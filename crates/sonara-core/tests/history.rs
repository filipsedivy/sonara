//! Session-level guarantees of commands and history (SON-P09): a rejected command leaves the
//! project, the version and both history stacks untouched; a no-op creates no step; undoing a
//! note edit restores exactly that edit and keeps every other note, clip and track.

use std::path::PathBuf;
use std::sync::Arc;

use sonara_core::commands::clip::{CreateMidiClip, DeleteNotes, EditNotes, NoteEdit};
use sonara_core::commands::track::{CreateTrack, RenameTrack};
use sonara_core::commands::{Command, Transaction};
use sonara_core::error::ErrorCode;
use sonara_core::ids::{self, IdGenerator, MASTER_ID, SequentialIds};
use sonara_core::midi::MidiNote;
use sonara_core::model::clip::MidiClip;
use sonara_core::model::project::Project;
use sonara_core::model::track::TrackKind;
use sonara_core::plugins::builtin_registry;
use sonara_core::session::{ProjectLocation, ProjectSession};

struct Fx {
    session: ProjectSession,
    track_id: String,
    clip_id: String,
}

fn note(id: &str, pitch: i64, start: f64) -> MidiNote {
    MidiNote::new(id, pitch, start, 0.5, 100, 0).expect("valid note")
}

impl Fx {
    /// A project with one instrument track holding one MIDI clip of three notes. The setup
    /// history is cleared, so every test starts with empty stacks.
    fn new() -> Self {
        let mut ids = SequentialIds::new(0);
        let project = Project::create(&mut ids, "Song", 48_000, 120.0).expect("valid project");
        let mut session =
            ProjectSession::with_options(Arc::new(builtin_registry()), 500, Box::new(|| 0.0));
        let location = ProjectLocation {
            path: None,
            workspace: PathBuf::from("/tmp/ws"),
            packaged: false,
        };
        session.load(project, location, true);
        let track_id = ids.new_id(ids::TRACK);
        let create = CreateTrack {
            name: "Lead".into(),
            kind: TrackKind::Instrument,
            index: None,
            color: None,
            output_id: MASTER_ID.to_owned(),
            instrument_plugin: Some("sonara.synth.poly".into()),
            instrument_preset: None,
            parent_id: None,
            track_id: track_id.clone(),
            instrument_id: ids.new_id(ids::DEVICE),
        };
        session.execute(&create, false, "user").expect("track");
        let clip_id = ids.new_id(ids::CLIP);
        let clip = CreateMidiClip {
            track_id: track_id.clone(),
            start: 0.0,
            length: 8.0,
            name: "MIDI".into(),
            notes: vec![
                note("n_a", 60, 0.0),
                note("n_b", 64, 1.0),
                note("n_c", 67, 2.0),
            ],
            loop_length: None,
            color: None,
            clip_id: clip_id.clone(),
        };
        session.execute(&clip, false, "user").expect("clip");
        session.clear_history();
        Self {
            session,
            track_id,
            clip_id,
        }
    }

    fn project(&self) -> &Project {
        self.session.project().expect("a project is open")
    }

    fn clip(&self) -> &MidiClip {
        self.project()
            .clip(&self.clip_id)
            .expect("clip exists")
            .1
            .as_midi()
            .expect("a MIDI clip")
    }

    fn exec(&mut self, command: &dyn Command) {
        self.session
            .execute(command, false, "user")
            .unwrap_or_else(|err| panic!("{} failed: {err}", command.name()));
    }

    fn edit(&self, note_id: &str, pitch: i64) -> EditNotes {
        EditNotes {
            clip_id: self.clip_id.clone(),
            edits: vec![NoteEdit {
                note_id: note_id.into(),
                pitch: Some(pitch),
                ..NoteEdit::default()
            }],
        }
    }

    fn rename(&self, name: &str) -> RenameTrack {
        RenameTrack {
            track_id: self.track_id.clone(),
            name: name.into(),
        }
    }
}

#[test]
fn rejected_commands_leave_project_version_and_both_stacks_untouched() {
    let mut fx = Fx::new();
    fx.exec(&fx.rename("One"));
    fx.exec(&fx.rename("Two"));
    fx.session.seal_undo();
    fx.session.undo("user").expect("undo Two");
    let (project, version) = (fx.project().clone(), fx.session.version());
    let stacks = |fx: &Fx| {
        let (undo, redo) = fx.session.history().labels();
        (
            undo.iter().map(ToString::to_string).collect::<Vec<_>>(),
            redo.iter().map(ToString::to_string).collect::<Vec<_>>(),
        )
    };
    let stacks_before = stacks(&fx);
    assert!(fx.session.history().can_redo());

    let rejected: Vec<Box<dyn Command>> = vec![
        // A missing target.
        Box::new(RenameTrack {
            track_id: "trk_missing".into(),
            name: "x".into(),
        }),
        // An invalid value.
        Box::new(fx.edit("n_a", 200)),
        // A missing note.
        Box::new(fx.edit("n_missing", 60)),
        // A transaction whose last step fails after earlier steps succeeded.
        Box::new(Transaction {
            commands: vec![
                Box::new(fx.edit("n_a", 61)),
                Box::new(fx.rename("Never")),
                Box::new(fx.edit("n_b", 128)),
            ],
            name: "Batch".into(),
        }),
    ];
    for command in &rejected {
        let err = fx
            .session
            .execute(command.as_ref(), false, "user")
            .expect_err("rejected");
        assert!(
            matches!(err.code, ErrorCode::NotFound | ErrorCode::InvalidArgument),
            "{}: {err}",
            command.name()
        );
        assert!(fx.project().ptr_eq(&project), "{}: project", command.name());
        assert_eq!(fx.session.version(), version, "{}: version", command.name());
        assert_eq!(stacks(&fx), stacks_before, "{}: history", command.name());
    }
    assert!(fx.session.history().can_redo(), "the redo branch survives");
    fx.session.redo("user").expect("redo Two still works");
    assert_eq!(fx.project().track(&fx.track_id).expect("track").name, "Two");
}

#[test]
fn a_no_op_creates_no_step_and_keeps_the_redo_branch() {
    let mut fx = Fx::new();
    fx.exec(&fx.rename("Renamed"));
    fx.session.undo("user").expect("undo");
    let (project, version) = (fx.project().clone(), fx.session.version());
    // A command that returns the input snapshot is a no-op. (A command that rewrites a value
    // with itself is not: it builds a new snapshot and is recorded, as in the prototype.)
    let same = Transaction {
        commands: Vec::new(),
        name: "Nothing".into(),
    };
    let res = fx.session.execute(&same, false, "user").expect("valid");
    assert!(!res.committed);
    assert!(fx.project().ptr_eq(&project) && fx.session.version() == version);
    assert!(!fx.session.history().can_undo());
    assert!(fx.session.history().can_redo());
}

#[test]
fn undoing_a_note_edit_restores_only_that_edit() {
    let mut fx = Fx::new();
    let before = fx.clip().clone();
    let track_before = fx.project().track(&fx.track_id).expect("track").clone();
    fx.exec(&fx.edit("n_b", 65));
    fx.session.seal_undo();
    fx.exec(&fx.rename("Renamed"));

    let edited = fx.clip().note("n_b").expect("note keeps its ID").clone();
    assert_eq!((edited.pitch, edited.start), (65, 1.0));
    assert_eq!(
        (edited.length, edited.velocity),
        (0.5, 100),
        "unrelated fields"
    );
    assert_eq!(fx.clip().note("n_a"), before.note("n_a"));
    assert_eq!(fx.clip().note("n_c"), before.note("n_c"));

    // Undo the rename first: the note edit stays.
    fx.session.undo("user").expect("undo rename");
    assert_eq!(fx.clip().note("n_b").expect("note").pitch, 65);
    assert_eq!(
        fx.project().track(&fx.track_id).expect("track").name,
        track_before.name
    );
    // Then undo the edit: the clip is exactly as it was.
    fx.session.undo("user").expect("undo edit");
    assert_eq!(fx.clip().notes, before.notes);
    assert_eq!(fx.clip().id, before.id);
}

#[test]
fn deleting_and_restoring_a_note_keeps_its_id_and_values() {
    let mut fx = Fx::new();
    let original = fx.clip().note("n_b").expect("note").clone();
    fx.exec(&DeleteNotes {
        clip_id: fx.clip_id.clone(),
        note_ids: vec!["n_b".into()],
    });
    assert!(fx.clip().note("n_b").is_none());
    fx.session.undo("user").expect("undo delete");
    assert_eq!(fx.clip().note("n_b"), Some(&original));
    fx.session.redo("user").expect("redo delete");
    assert!(fx.clip().note("n_b").is_none());
    assert_eq!(fx.clip().notes.len(), 2);
}
