//! The piano roll's data: the drum map of its keyboard, its gestures' undo
//! steps on the edit boundary, and the clips view that refreshes a note edit (PERF-01: a drag's
//! edit per display frame refreshes its clip, not the whole view).
//!
//! these prototype tests here (R):
//! `drum_track_shows_gm_labels` (the names, recorded from the prototype's `drum_label`),
//! `drag_moves_note_in_time_and_pitch_as_one_undo_step` and
//! `velocity_lane_sets_velocity_in_one_step` (the steps the page's gestures make).

use serde_json::{Value, json};
use sonara_app::catalog::{drum_label, drum_map};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::edit::{self, Edit, EditOutcome};
use sonara_app::ids::IdSource;
use sonara_app::projects::NewProject;
use sonara_app::settings::Settings;
use sonara_app::view::{ClipView, clips_view, project_view};
use sonara_core::ids::SequentialIds;
use tempfile::TempDir;

/// An application on the null backend with sequential IDs and a new project.
struct Gui {
    app: AppContext,
    _dir: TempDir,
}

impl Gui {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let app = AppContext::create(AppOptions {
            workspaces: Some(dir.path().join("ws")),
            settings: Some(Settings::default()),
            persist_settings: false,
            audio_backend: Some("null".to_owned()),
            ids: Some(IdSource::new(Box::new(SequentialIds::new(0)))),
            ..AppOptions::default()
        })
        .expect("the application");
        app.projects()
            .new_project(&NewProject {
                name: "Piano Roll".to_owned(),
                ..NewProject::default()
            })
            .expect("a project");
        Self { app, _dir: dir }
    }

    fn edit(&self, edit: Value) -> EditOutcome {
        let shown = edit.to_string();
        Edit::from_json(edit)
            .and_then(|edit| edit::apply(&self.app, edit))
            .unwrap_or_else(|error| panic!("{shown}: {error:?}"))
    }

    /// A track with the instrument `plugin` and a clip of `test_piano_roll.py`'s three notes at
    /// beat 4: `(track, clip, notes)`.
    fn chords(&self, plugin: &str) -> (String, String, Vec<String>) {
        let track = json!({"op": "create_track", "name": "Keys", "kind": "instrument",
                           "instrument": plugin});
        let track = self.edit(track).result["track_id"]
            .as_str()
            .expect("a track")
            .to_owned();
        let clip = json!({"op": "create_midi_clip", "track_id": track, "start": 4.0,
                          "length": 8.0, "name": "Chords"});
        let clip = self.edit(clip);
        let clip = created(&clip, "clip")[0].clone();
        let notes = self.edit(json!({"op": "add_notes", "clip_id": clip, "notes": [
            {"pitch": 60, "start": 0.0, "length": 1.0, "velocity": 100},
            {"pitch": 64, "start": 1.0, "length": 1.0, "velocity": 90},
            {"pitch": 67, "start": 2.0, "length": 0.5, "velocity": 80}]}));
        (track, clip, created(&notes, "note"))
    }

    /// The clip's notes as `(id, pitch, start, length, velocity)`.
    fn notes(&self, clip: &str) -> Vec<(String, u8, f64, f64, u8)> {
        let view = project_view(self.app.session(), self.app.plugins());
        let project = view.project.expect("an open project");
        for track in &project.tracks {
            for c in &track.clips {
                if let ClipView::Midi(midi) = c
                    && midi.id == clip
                {
                    return midi
                        .notes
                        .iter()
                        .map(|n| (n.id.clone(), n.pitch, n.start, n.length, n.velocity))
                        .collect();
                }
            }
        }
        panic!("no clip {clip}")
    }

    fn undo_labels(&self) -> Vec<String> {
        let session = self.app.session().lock();
        session
            .history()
            .labels()
            .0
            .into_iter()
            .map(str::to_owned)
            .collect()
    }
}

fn created(outcome: &EditOutcome, kind: &str) -> Vec<String> {
    outcome
        .changes
        .created
        .iter()
        .filter(|object| object.kind == kind)
        .map(|object| object.id.clone())
        .collect()
}

/// The prototype's `drum_label(pitch)` for every pitch it names, recorded on the host with
/// `.venv/bin/python -I -c "from sonara.ui.piano_roll.widgets import drum_label; …"`.
const PROTOTYPE_LABELS: [(u8, &str); 34] = [
    (35, "Kick"),
    (36, "Kick"),
    (37, "Rim"),
    (38, "Snare"),
    (39, "Clap"),
    (40, "Snare"),
    (41, "Tom Floor Low"),
    (42, "Hat Closed"),
    (43, "Tom Floor High"),
    (44, "Hat Pedal"),
    (45, "Tom Low"),
    (46, "Hat Open"),
    (47, "Tom Mid Low"),
    (48, "Tom Mid High"),
    (49, "Crash"),
    (50, "Tom High"),
    (51, "Ride"),
    (52, "China"),
    (53, "Ride Bell"),
    (54, "Tambourine"),
    (55, "Splash"),
    (56, "Cowbell"),
    (57, "Crash"),
    (59, "Ride"),
    (60, "Bongo High"),
    (61, "Bongo Low"),
    (62, "Conga Mute"),
    (63, "Conga High"),
    (64, "Conga Low"),
    (69, "Shaker"),
    (70, "Shaker"),
    (75, "Claves"),
    (76, "Block High"),
    (77, "Block Low"),
];

/// `test_drum_track_shows_gm_labels`: the keyboard's names are the prototype's, for every note.
#[test]
fn the_drum_map_names_every_gm_note_as_the_prototype() {
    let map = drum_map();
    assert_eq!(map.plugin_id, "sonara.drums");
    let names: Vec<(u8, &str)> = map
        .names
        .iter()
        .map(|n| (n.pitch, n.label.as_str()))
        .collect();
    assert_eq!(names, PROTOTYPE_LABELS);
    for pitch in 0..=127 {
        let expected = PROTOTYPE_LABELS
            .iter()
            .find(|(p, _)| *p == pitch)
            .map(|(_, l)| (*l).to_owned());
        assert_eq!(drum_label(pitch), expected, "pitch {pitch}");
    }
    // A drum track is an instrument track with the drum kit, as the view shows it.
    let gui = Gui::new();
    let (track, _, _) = gui.chords("sonara.drums");
    let view = project_view(gui.app.session(), gui.app.plugins());
    let project = view.project.expect("an open project");
    let drums = project
        .tracks
        .iter()
        .find(|t| t.id == track)
        .expect("the track");
    let instrument = drums.instrument.as_ref().expect("an instrument");
    assert_eq!(instrument.plugin_id, map.plugin_id);
}

/// `test_drag_moves_note_in_time_and_pitch_as_one_undo_step` and
/// `test_velocity_lane_sets_velocity_in_one_step`, on the edit boundary: a drag sends an
/// "Edit Notes" of the dragged notes on every move, which merge into one step until the page
/// seals it on release; the velocity lane's one edit and seal is the next step; undo restores.
#[test]
fn a_note_drag_is_one_step_until_the_seal() {
    let gui = Gui::new();
    let (_, clip, notes) = gui.chords("sonara.synth.poly");
    let depth = gui.undo_labels().len();
    for (start, pitch) in [(0.25, 60), (0.5, 61), (0.75, 61), (1.0, 62)] {
        let outcome = gui.edit(json!({"op": "edit_notes", "clip_id": clip,
                                      "edits": [{"note_id": notes[0], "pitch": pitch, "start": start}]}));
        assert_eq!(outcome.label, "Edit Notes");
        assert_eq!(outcome.changes.flags, ["notes"]);
        assert!(!outcome.changes.structure_changed);
    }
    edit::seal_undo(&gui.app);
    assert_eq!(gui.undo_labels().len(), depth + 1, "the drag is one step");
    let moved = gui.notes(&clip);
    assert!(
        moved.contains(&(notes[0].clone(), 62, 1.0, 1.0, 100)),
        "{moved:?}"
    );
    // The velocity lane: one edit on release, then the seal.
    gui.edit(json!({"op": "edit_notes", "clip_id": clip,
                    "edits": [{"note_id": notes[1], "velocity": 127}]}));
    edit::seal_undo(&gui.app);
    assert_eq!(gui.undo_labels().len(), depth + 2);
    // A resize of the same note after the seal is a step of its own.
    gui.edit(json!({"op": "edit_notes", "clip_id": clip,
                    "edits": [{"note_id": notes[0], "length": 1.5}]}));
    edit::seal_undo(&gui.app);
    assert_eq!(gui.undo_labels().len(), depth + 3);
    edit::undo(&gui.app).expect("undo the resize");
    edit::undo(&gui.app).expect("undo the velocity");
    edit::undo(&gui.app).expect("undo the drag");
    let restored = gui.notes(&clip);
    assert!(
        restored.contains(&(notes[0].clone(), 60, 0.0, 1.0, 100)),
        "{restored:?}"
    );
    assert!(
        restored.contains(&(notes[1].clone(), 64, 1.0, 1.0, 90)),
        "{restored:?}"
    );
}

/// The clips view of a note edit equals the full view's clips, with the version, the dirty state,
/// the history and the meter layout; an unknown clip is left out, an audio clip is a clip too.
#[test]
fn a_clips_view_equals_the_full_views_clips() {
    let gui = Gui::new();
    let (keys, clip, notes) = gui.chords("sonara.synth.poly");
    let (drums, beat, _) = gui.chords("sonara.drums");
    gui.edit(json!({"op": "edit_notes", "clip_id": clip,
                    "edits": [{"note_id": notes[2], "pitch": 69, "start": 2.5}]}));
    let full = project_view(gui.app.session(), gui.app.plugins());
    let open = full.project.expect("an open project");
    let asked = [beat.clone(), "nope".to_owned(), clip.clone()];
    let delta = clips_view(gui.app.session(), &asked);
    assert_eq!(delta.version, full.version);
    let clips = delta.project.expect("an open project");
    assert_eq!(clips.id, open.id);
    assert_eq!(clips.dirty, open.dirty);
    assert_eq!(clips.history, open.history);
    assert_eq!(clips.history.undo.as_deref(), Some("Edit Notes"));
    assert_eq!(clips.meter_layout, open.meter_layout);
    // In the project's order, each with its track.
    let placed: Vec<(&str, &ClipView)> = clips
        .clips
        .iter()
        .map(|p| (p.track_id.as_str(), &p.clip))
        .collect();
    let in_view = |track: &str| {
        let t = open
            .tracks
            .iter()
            .find(|t| t.id == track)
            .expect("the track");
        t.clips[0].clone()
    };
    assert_eq!(
        placed,
        [
            (keys.as_str(), &in_view(&keys)),
            (drums.as_str(), &in_view(&drums))
        ]
    );
    gui.app.projects().close(true).expect("closed");
    assert_eq!(clips_view(gui.app.session(), &asked).project, None);
}
