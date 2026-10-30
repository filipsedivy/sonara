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

// -- undo groups: one step for a whole AI composer run ----------------------------------------

fn labels(fx: &Fx) -> Vec<String> {
    fx.session
        .history()
        .labels()
        .0
        .into_iter()
        .map(str::to_owned)
        .collect()
}

#[test]
fn a_group_of_agent_steps_is_one_undo_step_that_restores_the_project_before_it() {
    let mut fx = Fx::new();
    fx.exec(&fx.rename("Mine"));
    let before = fx.project().clone();
    fx.session.begin_undo_group();
    for name in ["A", "B", "C"] {
        fx.session
            .execute(&fx.rename(name), false, "agent")
            .expect("agent edit");
    }
    let after = fx.project().clone();
    assert_eq!(
        labels(&fx).len(),
        4,
        "the steps stay separate while the run goes"
    );
    assert!(fx.session.end_undo_group("AI Composer"));
    assert_eq!(labels(&fx), ["Rename Track", "AI Composer"]);

    fx.session.undo("user").expect("undo the run");
    assert_eq!(*fx.project(), before);
    fx.session.redo("user").expect("redo the run");
    assert_eq!(*fx.project(), after);
    fx.session.undo("user").expect("undo the run again");
    fx.session.undo("user").expect("undo the earlier edit");
    assert_eq!(
        fx.session.undo("user").unwrap_err().code,
        ErrorCode::NothingToUndo
    );
}

#[test]
fn an_undo_made_inside_the_group_undoes_one_step_not_the_run() {
    let mut fx = Fx::new();
    fx.session.begin_undo_group();
    fx.session
        .execute(&fx.rename("A"), false, "agent")
        .expect("a");
    let after_a = fx.project().clone();
    fx.session
        .execute(&fx.rename("B"), false, "agent")
        .expect("b");
    fx.session
        .undo("agent")
        .expect("the agent undoes its last step");
    assert_eq!(*fx.project(), after_a);
    assert!(fx.session.end_undo_group("AI Composer"));
    assert_eq!(labels(&fx), ["AI Composer"]);
}

#[test]
fn an_edit_of_the_user_inside_the_group_stays_its_own_step() {
    let mut fx = Fx::new();
    let start = fx.project().clone();
    fx.session.begin_undo_group();
    fx.session
        .execute(&fx.rename("A"), false, "agent")
        .expect("a");
    fx.session
        .execute(&fx.rename("B"), false, "agent")
        .expect("b");
    fx.session
        .execute(&fx.rename("Mine"), false, "gui")
        .expect("user");
    fx.session
        .execute(&fx.rename("C"), false, "agent")
        .expect("c");
    assert!(fx.session.end_undo_group("AI Composer"));
    assert_eq!(labels(&fx), ["AI Composer", "Rename Track", "AI Composer"]);
    fx.session.undo("user").expect("last run part");
    assert_eq!(fx.session.history().undo_label(), Some("Rename Track"));
    fx.session.undo("user").expect("the user's edit");
    fx.session.undo("user").expect("first run part");
    assert_eq!(*fx.project(), start);
}

#[test]
fn steps_inside_a_group_never_merge_into_an_earlier_step() {
    let mut fx = Fx::new();
    // The same merge key within the merge window (the test clock stands still).
    fx.session
        .execute(&fx.rename("Mine"), false, "gui")
        .expect("user");
    fx.session.begin_undo_group();
    fx.session
        .execute(&fx.rename("A"), false, "agent")
        .expect("agent");
    fx.session.end_undo_group("AI Composer");
    assert_eq!(labels(&fx), ["Rename Track", "AI Composer"]);
    // The next edit does not merge into the run's step either.
    fx.session
        .execute(&fx.rename("After"), false, "gui")
        .expect("user");
    assert_eq!(labels(&fx), ["Rename Track", "AI Composer", "Rename Track"]);
}

#[test]
fn an_empty_group_changes_nothing_and_makes_no_event() {
    let mut fx = Fx::new();
    fx.exec(&fx.rename("Mine"));
    let events = fx.session.subscribe();
    fx.session.begin_undo_group();
    assert!(!fx.session.end_undo_group("AI Composer"));
    assert_eq!(labels(&fx), ["Rename Track"]);
    assert!(events.try_recv().is_err());
}

#[test]
fn collapsing_a_group_tells_observers_to_refresh_the_history() {
    let mut fx = Fx::new();
    fx.session.begin_undo_group();
    fx.session
        .execute(&fx.rename("A"), false, "agent")
        .expect("agent");
    let events = fx.session.subscribe();
    fx.session.end_undo_group("AI Composer");
    let event = events.try_recv().expect("an event");
    assert_eq!(
        event.source, "user",
        "no status-bar message for the collapse"
    );
    assert!(event.changes.flags.is_empty());
}

#[test]
fn opening_another_project_drops_an_open_group() {
    let mut fx = Fx::new();
    fx.session.begin_undo_group();
    fx.session
        .execute(&fx.rename("A"), false, "agent")
        .expect("agent");
    let project = fx.project().clone();
    let location = ProjectLocation {
        path: None,
        workspace: PathBuf::from("/tmp/ws2"),
        packaged: false,
    };
    fx.session.load(project, location, true);
    fx.session
        .execute(&fx.rename("B"), false, "agent")
        .expect("agent");
    assert!(
        !fx.session.end_undo_group("AI Composer"),
        "the group did not survive the load"
    );
    assert_eq!(labels(&fx), ["Rename Track"]);
}
