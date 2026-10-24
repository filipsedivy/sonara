//! The GUI's data layer: the project view, the edit boundary with its
//! error codes, undo, redo and the gesture seal, quantize, the event mapping and the notice
//! pump across threads.
//!
//! these prototype tests here (R):
//! `quantize_and_strength` (the quantize command), `output_routing_and_cycle_error` (the error
//! mapping), `sidechain_selection_enables_external_key` (one transaction),
//! `agent_edits_over_ipc_appear_in_gui_and_share_undo`,
//! `agent_edit_from_other_thread_updates_strip`, `agent_param_edit_from_thread`,
//! `agent_edit_from_another_thread_appears` and
//! `agent_edits_from_background_thread_appear` (the delivery of other threads' edits to the
//! window, with the change set that decides between rebuilding and refreshing), and
//! `gui_edit_is_visible_to_agent` (a GUI edit is in the one session).

use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::edit::{self, Edit, EditOutcome};
use sonara_app::events::{ErrorNotice, EventPump, Notice, Subscriptions};
use sonara_app::ids::IdSource;
use sonara_app::media::AudioImport;
use sonara_app::projects::NewProject;
use sonara_app::settings::Settings;
use sonara_app::view::{ClipView, ProjectView, project_view};
use sonara_core::ids::{IdGenerator as _, SequentialIds};
use sonara_core::model::project::Project;
use sonara_core::model::track::Track;
use tempfile::TempDir;

/// An application on the null backend with sequential IDs and a new 120 BPM project.
struct Gui {
    app: AppContext,
    dir: TempDir,
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
                name: "GUI".to_owned(),
                ..NewProject::default()
            })
            .expect("a project");
        Self { app, dir }
    }

    fn try_edit(&self, edit: Value) -> Result<EditOutcome, ErrorNotice> {
        edit::apply(&self.app, Edit::from_json(edit)?)
    }

    fn edit(&self, edit: Value) -> EditOutcome {
        let shown = edit.to_string();
        self.try_edit(edit)
            .unwrap_or_else(|error| panic!("{shown}: {error:?}"))
    }

    fn code(&self, edit: Value) -> &'static str {
        let shown = edit.to_string();
        match self.try_edit(edit) {
            Ok(outcome) => panic!("{shown} succeeded: {outcome:?}"),
            Err(error) => error.code,
        }
    }

    fn project(&self) -> Project {
        self.app.session().project().expect("an open project")
    }

    fn track(&self, id: &str) -> Track {
        self.project().track(id).expect("the track").clone()
    }

    fn view(&self) -> ProjectView {
        project_view(self.app.session(), self.app.plugins())
    }

    /// A new track's ID.
    fn create(&self, name: &str, kind: &str, instrument: Option<&str>) -> String {
        let outcome = self.edit(json!({"op": "create_track", "name": name, "kind": kind,
                                       "instrument": instrument}));
        outcome.result["track_id"]
            .as_str()
            .expect("a track ID")
            .to_owned()
    }

    fn effect(&self, track: &str, plugin: &str) -> String {
        let outcome =
            self.edit(json!({"op": "add_device", "track_id": track, "plugin_id": plugin}));
        outcome.result["device_id"]
            .as_str()
            .expect("a device ID")
            .to_owned()
    }

    fn midi_clip(&self, track: &str, start: f64, length: f64) -> String {
        let outcome = self.edit(json!({"op": "create_midi_clip", "track_id": track,
                                       "start": start, "length": length, "name": "Clip"}));
        created(&outcome, "clip")[0].clone()
    }

    /// A 2-second stereo sine imported as an audio track with a clip at beat `start`.
    fn audio_clip(&self, start: f64) -> (String, String, String) {
        let path = self.dir.path().join("tone.wav");
        write_tone(&path);
        let result = self
            .app
            .media()
            .import_audio(
                &path,
                &AudioImport {
                    start_beat: start,
                    create_clip: true,
                    ..AudioImport::default()
                },
            )
            .expect("the import");
        let text = |key: &str| result[key].as_str().expect("an ID").to_owned();
        (text("track_id"), text("clip_id"), text("asset_id"))
    }
}

fn write_tone(path: &Path) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("a WAV writer");
    for i in 0..96_000_u32 {
        let t = f64::from(i) / 48_000.0;
        #[expect(clippy::cast_possible_truncation, reason = "a 16-bit test tone")]
        let sample = ((2.0 * std::f64::consts::PI * 220.0 * t).sin() * 8000.0) as i16;
        writer.write_sample(sample).expect("a sample");
        writer.write_sample(-sample).expect("a sample");
    }
    writer.finalize().expect("the WAV");
}

/// The IDs of the objects of `kind` an outcome created, in order.
fn created(outcome: &EditOutcome, kind: &str) -> Vec<String> {
    outcome
        .changes
        .created
        .iter()
        .filter(|object| object.kind == kind)
        .map(|object| object.id.clone())
        .collect()
}

fn note_starts(gui: &Gui, clip: &str) -> Vec<(u8, f64)> {
    let project = gui.project();
    let (_, clip) = project.clip(clip).expect("the clip");
    clip.as_midi()
        .expect("a MIDI clip")
        .notes
        .iter()
        .map(|n| (n.pitch, n.start))
        .collect()
}

// -- the view ---------------------------------------------------------------------------------

#[test]
fn the_view_shows_tracks_devices_routing_and_history() {
    let gui = Gui::new();
    let synth = gui.create("Lead", "instrument", Some("sonara.synth.poly"));
    let bus_a = gui.create("Bus A", "bus", None);
    let bus_b = gui.create("Bus B", "bus", None);
    let folder = gui.create("Strings", "folder", None);
    let eq = gui.effect(&synth, "sonara.eq.parametric");
    gui.edit(json!({"op": "set_device_params", "device_id": eq,
                    "values": {"mid1_gain": 4.5}}));
    gui.edit(json!({"op": "add_send", "track_id": synth, "target_id": bus_a, "level_db": -12.0}));
    let clip = gui.midi_clip(&synth, 4.0, 4.0);
    gui.edit(json!({"op": "add_notes", "clip_id": clip,
                    "notes": [{"pitch": 60, "start": 0.0, "length": 1.0, "velocity": 100}]}));
    gui.app
        .session()
        .lock()
        .execute(
            &sonara_core::commands::track::SetTrackParent {
                track_id: synth.clone(),
                parent_id: Some(folder.clone()),
            },
            false,
            "agent",
        )
        .expect("into the folder");

    let view = gui.view();
    assert_eq!(view.version, gui.app.session().lock().version());
    let project = view.project.expect("an open project");
    assert_eq!(project.name, "GUI");
    assert!(project.dirty && project.path.is_none());
    assert_eq!(project.history.undo.as_deref(), Some("Move Into Folder"));
    assert_eq!(project.history.redo, None);
    let names: Vec<&str> = project.tracks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Lead", "Bus A", "Bus B", "Strings"]);
    assert_eq!(project.master.id, "master");
    assert_eq!(project.master.output_targets.len(), 0);

    let lead = &project.tracks[0];
    assert_eq!((lead.kind, lead.depth), ("instrument", 1));
    assert_eq!(lead.parent_id.as_deref(), Some(folder.as_str()));
    let outputs: Vec<&str> = lead.output_targets.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(outputs, ["master", bus_a.as_str(), bus_b.as_str()]);
    // Bus A is targeted already, so only Bus B is offered for a new send.
    let sends: Vec<&str> = lead.send_targets.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(sends, [bus_b.as_str()]);
    assert_eq!(lead.sends[0].level_db, -12.0);
    // A bus never feeds itself.
    let bus = &project.tracks[1];
    assert!(bus.output_targets.iter().all(|t| t.id != bus_a));
    assert!(bus.send_targets.iter().all(|t| t.id != bus_a));

    // Devices with their parameters resolved in descriptor order, and the descriptors.
    let instrument = lead.instrument.as_ref().expect("the synth");
    assert_eq!(instrument.plugin_id, "sonara.synth.poly");
    assert!(instrument.available);
    let eq_view = &lead.inserts[0];
    assert_eq!(eq_view.params["mid1_gain"], json!(4.5));
    assert_eq!(
        eq_view.params["hp_on"],
        json!(false),
        "a default fills the gap"
    );
    let descriptor = &project.plugins["sonara.eq.parametric"];
    let order: Vec<&String> = eq_view.params.keys().collect();
    let spec_order: Vec<&String> = descriptor.params.iter().map(|p| &p.id).collect();
    assert_eq!(order, spec_order);
    let freq = descriptor
        .params
        .iter()
        .find(|p| p.id == "mid1_freq")
        .expect("mid1_freq");
    assert!(freq.log_scale && freq.unit == "Hz");
    assert_eq!(
        project.plugins.keys().collect::<Vec<_>>(),
        ["sonara.eq.parametric", "sonara.synth.poly"]
    );

    let ClipView::Midi(midi) = &lead.clips[0] else {
        panic!("a MIDI clip");
    };
    assert_eq!((midi.start, midi.length, midi.notes.len()), (4.0, 4.0, 1));
    assert_eq!(midi.notes[0].pitch, 60);

    assert_eq!(project.time_signatures[0].beats_per_bar, 4.0);
    assert_eq!(project.tempo[0].bpm, 120.0);
    assert_eq!(project.transport.loop_end, 16.0);
    assert_eq!(project.end_beat, 8.0);

    // The JSON keeps the clip's type tag and the plugin map.
    let json = serde_json::to_value(gui.view()).expect("serializable");
    assert_eq!(json["project"]["tracks"][0]["clips"][0]["type"], "midi");
    assert_eq!(
        json["project"]["tracks"][0]["inserts"][0]["params"]["mid1_gain"],
        4.5
    );
}

#[test]
fn the_view_has_audio_clips_assets_and_nothing_without_a_project() {
    let gui = Gui::new();
    let (track, clip, asset) = gui.audio_clip(2.0);
    gui.edit(
        json!({"op": "set_clip_properties", "clip_id": clip, "gain_db": -3.0,
                    "fade_in": {"seconds": 0.5, "curve": "equal_power"}}),
    );
    let project = gui.view().project.expect("a project");
    let audio_track = project
        .tracks
        .iter()
        .find(|t| t.id == track)
        .expect("the track");
    let ClipView::Audio(audio) = &audio_track.clips[0] else {
        panic!("an audio clip");
    };
    assert_eq!(audio.asset_id, asset);
    assert_eq!(audio.gain_db, -3.0);
    assert_eq!(
        (audio.fade_in.seconds, audio.fade_in.curve),
        (0.5, "equal_power")
    );
    assert_eq!(audio.fade_out.curve, "linear");
    let media = &project.assets[0];
    assert_eq!(
        (media.id.as_str(), media.kind, media.channels, media.frames),
        (asset.as_str(), "audio", 2, 96_000)
    );
    assert_eq!(media.duration_seconds, 2.0);

    gui.app.projects().close(true).expect("closed");
    let view = gui.view();
    assert!(view.project.is_none());
    assert_eq!(serde_json::to_value(&view).unwrap()["project"], Value::Null);
}

/// The meter layout follows the order of tracks and devices, not their values, so meters keep
/// flowing while a fader moves.
#[test]
fn the_meter_layout_changes_with_the_order_only() {
    let gui = Gui::new();
    let a = gui.create("A", "audio", None);
    let layout = || gui.view().project.unwrap().meter_layout;
    let first = layout();
    gui.edit(json!({"op": "set_track_mixer", "track_id": a, "volume_db": -6.0}));
    gui.edit(json!({"op": "rename_track", "track_id": a, "name": "Vocals"}));
    assert_eq!(layout(), first);
    let b = gui.create("B", "audio", None);
    let second = layout();
    assert_ne!(second, first);
    gui.edit(json!({"op": "move_track", "track_id": b, "index": 0}));
    assert_ne!(layout(), second);
    let before = layout();
    gui.effect(&a, "sonara.dynamics.compressor");
    assert_ne!(layout(), before);
}

// -- edits ------------------------------------------------------------------------------------

/// Every edit runs as `gui` and keeps the prototype's undo labels, composite gestures included.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one row per edit of the boundary; splitting the table would hide which are covered"
)]
fn every_edit_runs_with_the_gui_source() {
    let gui = Gui::new();
    let events = gui.app.session().lock().subscribe();
    let vox = gui.create("Vox", "audio", None);
    let bus = gui.create("Bus", "bus", None);
    let kick = gui.create("Kick", "instrument", Some("sonara.drums"));
    let bass = gui.create("Bass", "instrument", Some("sonara.synth.poly"));
    let comp = gui.effect(&bass, "sonara.dynamics.compressor");
    let eq = gui.effect(&bass, "sonara.eq.parametric");
    let clip = gui.midi_clip(&bass, 0.0, 4.0);
    let added = gui.edit(json!({"op": "add_notes", "clip_id": clip, "notes": [
        {"pitch": 60, "start": 0.1, "length": 0.5, "velocity": 90},
        {"pitch": 64, "start": 1.0, "length": 0.5, "velocity": 90, "channel": 1}]}));
    let notes = created(&added, "note");
    assert_eq!(notes.len(), 2);
    let send = gui.edit(json!({"op": "add_send", "track_id": vox, "target_id": bus,
                               "level_db": -12.0}));
    let send = send.result["send_id"].as_str().unwrap().to_owned();

    let steps: Vec<(Value, &str)> = vec![
        (
            json!({"op": "rename_track", "track_id": vox, "name": "Lead Vox"}),
            "Rename Track",
        ),
        (
            json!({"op": "set_track_color", "track_id": vox, "color": "#ff8800"}),
            "Track Color",
        ),
        (
            json!({"op": "set_track_color", "track_id": vox, "color": null}),
            "Track Color",
        ),
        (
            json!({"op": "move_track", "track_id": vox, "index": 2}),
            "Move Track",
        ),
        (
            json!({"op": "set_track_mixer", "track_id": vox, "mute": true, "solo": true}),
            "Mute",
        ),
        (
            json!({"op": "set_track_mixer", "track_id": "master", "volume_db": -1.5}),
            "Track Volume",
        ),
        (
            json!({"op": "set_track_input", "track_id": vox, "armed": true, "monitor": "on",
                "channels": [0, 1]}),
            "Track Input",
        ),
        (
            json!({"op": "set_track_output", "track_id": vox, "output_id": bus}),
            "Route Output",
        ),
        (
            json!({"op": "update_send", "track_id": vox, "send_id": send, "level_db": -6.0}),
            "Send Level",
        ),
        (
            json!({"op": "update_send", "track_id": vox, "send_id": send, "pre_fader": true}),
            "Send Level",
        ),
        (
            json!({"op": "remove_send", "track_id": vox, "send_id": send}),
            "Remove Send",
        ),
        (
            json!({"op": "move_device", "device_id": eq, "index": 0}),
            "Reorder Effects",
        ),
        (
            json!({"op": "set_device_enabled", "device_id": eq, "enabled": false}),
            "Bypass Effect",
        ),
        (
            json!({"op": "set_device_params", "device_id": eq, "values": {"low_gain": 3.0}}),
            "Set Parameter",
        ),
        (
            json!({"op": "reset_device", "device_id": eq}),
            "Set Parameter",
        ),
        (
            json!({"op": "apply_preset", "device_id": eq, "preset": "Air"}),
            "Load Preset",
        ),
        (
            json!({"op": "set_sidechain", "device_id": comp, "source_track_id": kick}),
            "Set Sidechain",
        ),
        (
            json!({"op": "set_instrument", "track_id": kick, "plugin_id": "sonara.synth.poly"}),
            "Set Instrument",
        ),
        (
            json!({"op": "edit_notes", "clip_id": clip, "edits": [
            {"note_id": notes[0], "pitch": 62, "start": 0.5}]}),
            "Edit Notes",
        ),
        (
            json!({"op": "quantize_notes", "clip_id": clip, "grid": 1.0, "strength": 1.0}),
            "Quantize Notes",
        ),
        (
            json!({"op": "delete_notes", "clip_id": clip, "note_ids": [notes[1]]}),
            "Delete Notes",
        ),
        (
            json!({"op": "set_clip_properties", "clip_id": clip, "name": "Riff", "muted": true,
                "loop_length": 2.0}),
            "Clip Properties",
        ),
        (
            json!({"op": "set_clip_properties", "clip_id": clip, "loop_length": null}),
            "Clip Properties",
        ),
        (
            json!({"op": "move_clip", "clip_id": clip, "start": 8.0}),
            "Move Clip",
        ),
        (
            json!({"op": "trim_clip", "clip_id": clip, "end": 11.0}),
            "Trim Clip",
        ),
        (
            json!({"op": "move_clips", "clip_ids": [clip], "delta_beats": -4.0}),
            "Move Clips",
        ),
        (
            json!({"op": "split_clips", "clip_ids": [clip], "at": 5.0}),
            "Split Clips",
        ),
        (
            json!({"op": "duplicate_clips", "clip_ids": [clip]}),
            "Duplicate Clips",
        ),
        (
            json!({"op": "set_transport", "loop_enabled": true, "loop_start": 4.0,
                "loop_end": 8.0}),
            "Transport Settings",
        ),
        (
            json!({"op": "set_transport", "metronome": true, "count_in_bars": 2}),
            "Transport Settings",
        ),
        (json!({"op": "set_tempo", "bpm": 98.5}), "Set Tempo"),
        (
            json!({"op": "set_time_signature", "numerator": 6, "denominator": 8}),
            "Set Time Signature",
        ),
        (
            json!({"op": "rename_project", "name": "Song"}),
            "Rename Project",
        ),
        (
            json!({"op": "set_project_length", "length_beats": 64.0}),
            "Set Project Length",
        ),
        (
            json!({"op": "set_project_length", "length_beats": null}),
            "Set Project Length",
        ),
        (
            json!({"op": "set_metadata", "values": {"artist": "Me", "genre": "Pop"}}),
            "Edit Project Info",
        ),
        (
            json!({"op": "remove_device", "device_id": eq}),
            "Remove Effect",
        ),
        (
            json!({"op": "delete_clips", "clip_ids": [clip]}),
            "Delete Clips",
        ),
        (
            json!({"op": "delete_track", "track_id": kick}),
            "Delete Track",
        ),
    ];
    for (step, label) in steps {
        let shown = step.to_string();
        let outcome = gui.edit(step);
        assert!(outcome.committed, "{shown}");
        assert_eq!(outcome.label, label, "{shown}");
        // Faders and parameters merge into one step until the gesture is sealed.
        edit::seal_undo(&gui.app);
    }
    let sources: Vec<String> = events.try_iter().map(|e| e.source).collect();
    assert!(sources.len() > 40);
    assert!(sources.iter().all(|s| s == "gui"), "{sources:?}");

    let project = gui.project();
    assert_eq!(project.name, "Song");
    assert_eq!(project.metadata.artist, "Me");
    assert_eq!(project.tempo_map.bpm_at(0.0), 98.5);
    assert_eq!(project.transport.count_in_bars, 2);
    assert_eq!(gui.track("master").mixer.volume_db, -1.5);
    let vox = gui.track(&vox);
    assert_eq!(vox.name, "Lead Vox");
    assert!(vox.input.armed && vox.mixer.mute && vox.sends.is_empty());
    assert_eq!(vox.output_id.as_deref(), Some(bus.as_str()));
    let bass = gui.track(&bass);
    let compressor = bass.find_device(&comp).expect("the compressor");
    assert!(project.find_track(&kick).is_none());
    assert_eq!(
        compressor.sidechain_id, None,
        "deleting the kick repaired the sidechain"
    );
}

#[test]
fn undo_redo_and_the_gesture_seal() {
    let gui = Gui::new();
    let vox = gui.create("Vox", "audio", None);
    // A fader drag: one edit per move, one undo step.
    for db in [-1.0, -2.0, -3.0, -4.0] {
        gui.edit(json!({"op": "set_track_mixer", "track_id": vox, "volume_db": db}));
    }
    edit::seal_undo(&gui.app);
    // The next drag is a step of its own.
    gui.edit(json!({"op": "set_track_mixer", "track_id": vox, "volume_db": -9.0}));
    edit::seal_undo(&gui.app);
    let undo = edit::undo(&gui.app).expect("undo");
    assert_eq!(undo.label, "Undo Track Volume");
    assert!(undo.changes.structure_changed, "an undo rebuilds");
    assert_eq!(gui.track(&vox).mixer.volume_db, -4.0);
    edit::undo(&gui.app).expect("undo the first drag");
    assert_eq!(gui.track(&vox).mixer.volume_db, 0.0);
    let redo = edit::redo(&gui.app).expect("redo");
    assert_eq!(redo.label, "Redo Track Volume");
    assert_eq!(gui.track(&vox).mixer.volume_db, -4.0);
    assert_eq!(
        gui.view().project.unwrap().history.redo.as_deref(),
        Some("Track Volume")
    );
    edit::undo(&gui.app).expect("undo");
    edit::undo(&gui.app).expect("undo the track");
    assert_eq!(
        edit::undo(&gui.app).expect_err("nothing left").code,
        "NOTHING_TO_UNDO"
    );
    while edit::redo(&gui.app).is_ok() {}
    assert_eq!(
        edit::redo(&gui.app).expect_err("nothing left").code,
        "NOTHING_TO_REDO"
    );
    gui.app.projects().close(true).expect("closed");
    assert_eq!(edit::undo(&gui.app).unwrap_err().code, "NO_PROJECT");
    assert_eq!(edit::redo(&gui.app).unwrap_err().code, "NO_PROJECT");
}

/// TST-03: every edit's failure reports its code for the status bar, and leaves the project
/// unchanged.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one row per failure of every edit; splitting the table would hide which are covered"
)]
fn every_edit_reports_its_error_code() {
    let gui = Gui::new();
    let vox = gui.create("Vox", "audio", None);
    let bus_a = gui.create("Bus A", "bus", None);
    let bus_b = gui.create("Bus B", "bus", None);
    let synth = gui.create("Synth", "instrument", Some("sonara.synth.poly"));
    let eq = gui.effect(&vox, "sonara.eq.parametric");
    let clip = gui.midi_clip(&synth, 0.0, 4.0);
    let (_, audio, _) = gui.audio_clip(0.0);
    gui.edit(json!({"op": "set_track_output", "track_id": bus_a, "output_id": bus_b}));
    let version = gui.app.session().lock().version();

    let cases: Vec<(Value, &str)> = vec![
        // The shape of the edit (TYP-06).
        (json!({"op": "explode"}), "INVALID_ARGUMENT"),
        (
            json!({"op": "rename_track", "track_id": vox}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "rename_track", "track_id": vox, "name": 3}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "rename_track", "track_id": vox, "name": "x", "extra": 1}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "create_track", "name": "M", "kind": "master"}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "set_track_input", "track_id": vox, "monitor": "loud"}),
            "INVALID_ARGUMENT",
        ),
        // Tracks.
        (
            json!({"op": "create_track", "name": "A", "kind": "audio",
                "instrument": "sonara.synth.poly"}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "create_track", "name": "I", "kind": "instrument",
                "instrument": "sonara.nope"}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "delete_track", "track_id": "trk_nope"}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "delete_track", "track_id": "master"}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "rename_track", "track_id": "trk_nope", "name": "x"}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "set_track_color", "track_id": vox, "color": "orange"}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "move_track", "track_id": "trk_nope", "index": 0}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "set_track_mixer", "track_id": vox, "pan": 3.0}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "set_track_mixer", "track_id": "trk_nope", "mute": true}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "set_track_input", "track_id": vox, "channels": [-1]}),
            "INVALID_ARGUMENT",
        ),
        // A routing loop is refused with its own code, and a message naming the loop.
        (
            json!({"op": "set_track_output", "track_id": bus_b, "output_id": bus_a}),
            "ROUTING_CYCLE",
        ),
        (
            json!({"op": "set_track_output", "track_id": vox, "output_id": "trk_nope"}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "add_send", "track_id": bus_a, "target_id": bus_a, "level_db": -12.0}),
            "ROUTING_CYCLE",
        ),
        (
            json!({"op": "add_send", "track_id": vox, "target_id": "trk_nope",
                "level_db": -12.0}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "update_send", "track_id": vox, "send_id": "snd_nope",
                "level_db": -1.0}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "remove_send", "track_id": vox, "send_id": "snd_nope"}),
            "NOT_FOUND",
        ),
        // Devices.
        (
            json!({"op": "add_device", "track_id": vox, "plugin_id": "sonara.nope"}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "add_device", "track_id": vox, "plugin_id": "sonara.synth.poly"}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "remove_device", "device_id": "dev_nope"}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "move_device", "device_id": "dev_nope", "index": 0}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "set_device_enabled", "device_id": "dev_nope", "enabled": true}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "set_device_params", "device_id": eq, "values": {"nope": 1}}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "set_device_params", "device_id": eq, "values": {"mid1_gain": 99.0}}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "reset_device", "device_id": "dev_nope"}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "apply_preset", "device_id": eq, "preset": "Nope"}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "set_instrument", "track_id": vox, "plugin_id": "sonara.synth.poly"}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "set_sidechain", "device_id": "dev_nope", "source_track_id": vox}),
            "NOT_FOUND",
        ),
        // Clips.
        (
            json!({"op": "create_midi_clip", "track_id": vox, "start": 0.0, "length": 4.0,
                "name": "x"}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "create_midi_clip", "track_id": synth, "start": 0.0, "length": 0.0,
                "name": "x"}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "move_clip", "clip_id": "clp_nope", "start": 1.0}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "move_clip", "clip_id": clip, "track_id": vox}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "move_clips", "clip_ids": ["clp_nope"], "delta_beats": 1.0}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "trim_clip", "clip_id": clip, "end": -1.0}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "split_clips", "clip_ids": ["clp_nope"], "at": 1.0}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "duplicate_clips", "clip_ids": ["clp_nope"]}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "delete_clips", "clip_ids": ["clp_nope"]}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "set_clip_properties", "clip_id": audio,
                "fade_in": {"seconds": -1.0, "curve": "linear"}}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "set_clip_properties", "clip_id": clip, "gain_db": -3.0}),
            "INVALID_ARGUMENT",
        ),
        // Notes.
        (
            json!({"op": "add_notes", "clip_id": clip, "notes": [
            {"pitch": 128, "start": 0.0, "length": 1.0, "velocity": 100}]}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "add_notes", "clip_id": audio, "notes": [
            {"pitch": 60, "start": 0.0, "length": 1.0, "velocity": 100}]}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "edit_notes", "clip_id": clip, "edits": [{"note_id": "n_nope"}]}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "delete_notes", "clip_id": "clp_nope", "note_ids": []}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "quantize_notes", "clip_id": clip, "grid": 0.0, "strength": 1.0}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "quantize_notes", "clip_id": clip, "grid": 0.25, "strength": 2.0}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "quantize_notes", "clip_id": clip, "note_ids": ["n_nope"],
                "grid": 0.25, "strength": 1.0}),
            "NOT_FOUND",
        ),
        (
            json!({"op": "quantize_notes", "clip_id": audio, "grid": 0.25, "strength": 1.0}),
            "INVALID_ARGUMENT",
        ),
        // Project.
        (
            json!({"op": "set_transport", "loop_start": 8.0, "loop_end": 4.0}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "set_transport", "count_in_bars": 9}),
            "INVALID_ARGUMENT",
        ),
        (json!({"op": "set_tempo", "bpm": 0.0}), "INVALID_ARGUMENT"),
        (
            json!({"op": "set_time_signature", "numerator": 4, "denominator": 3}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "rename_project", "name": "  "}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "set_project_length", "length_beats": -4.0}),
            "INVALID_ARGUMENT",
        ),
        (
            json!({"op": "set_metadata", "values": {"composer": "x"}}),
            "INVALID_ARGUMENT",
        ),
    ];
    for (case, code) in cases {
        let shown = case.to_string();
        assert_eq!(gui.code(case), code, "{shown}");
    }
    assert_eq!(
        gui.app.session().lock().version(),
        version,
        "nothing changed"
    );

    let cycle = gui
        .try_edit(json!({"op": "set_track_output", "track_id": bus_b, "output_id": bus_a}))
        .unwrap_err();
    assert!(
        cycle.message.to_lowercase().contains("loop"),
        "{}",
        cycle.message
    );
    assert_eq!(gui.track(&bus_b).output_id.as_deref(), Some("master"));

    // Without a project, every edit is NO_PROJECT.
    gui.app.projects().close(true).expect("closed");
    assert_eq!(
        gui.code(json!({"op": "rename_project", "name": "x"})),
        "NO_PROJECT"
    );
    assert_eq!(
        gui.code(json!({"op": "split_clips", "clip_ids": [clip], "at": 1.0})),
        "NO_PROJECT"
    );
}

/// The canvas splits the selected clips that contain the beat as one step, and
/// duplicates them as a block after their span.
#[test]
fn split_and_duplicate_keep_the_canvas_gestures() {
    let gui = Gui::new();
    let synth = gui.create("Synth", "instrument", Some("sonara.synth.poly"));
    let a = gui.midi_clip(&synth, 0.0, 4.0);
    let b = gui.midi_clip(&synth, 6.0, 2.0);
    let split = gui.edit(json!({"op": "split_clips", "clip_ids": [a, b], "at": 2.0}));
    assert_eq!(split.label, "Split Clips");
    assert_eq!(
        created(&split, "clip").len(),
        1,
        "only the clip under the beat"
    );
    // A beat at a clip's edge splits nothing, and nothing changes.
    let none = gui.edit(json!({"op": "split_clips", "clip_ids": [b], "at": 6.0}));
    assert!(!none.committed);

    let duplicate = gui.edit(json!({"op": "duplicate_clips", "clip_ids": [a, b]}));
    assert_eq!(duplicate.label, "Duplicate Clips");
    let copies = created(&duplicate, "clip");
    assert_eq!(copies.len(), 2);
    let project = gui.project();
    let starts: Vec<f64> = copies
        .iter()
        .map(|id| project.clip(id).unwrap().1.start())
        .collect();
    // The span is 0–8, so the block repeats from beat 8.
    assert_eq!(starts, [8.0, 14.0]);
    edit::undo(&gui.app).expect("one step");
    assert_eq!(gui.track(&synth).clips.len(), 3);
}

/// `test_quantize_and_strength`: half strength moves 3.1 to 3.05 on a 1/4 grid, full strength
/// to 3.0; only the chosen notes move, and the step is "Quantize Notes".
#[test]
fn quantize_moves_the_chosen_notes_at_a_strength() {
    let gui = Gui::new();
    let synth = gui.create("Synth", "instrument", Some("sonara.synth.poly"));
    let clip = gui.midi_clip(&synth, 0.0, 8.0);
    let added = gui.edit(json!({"op": "add_notes", "clip_id": clip, "notes": [
        {"pitch": 70, "start": 3.1, "length": 0.25, "velocity": 100},
        {"pitch": 72, "start": 5.1, "length": 0.25, "velocity": 100}]}));
    let notes = created(&added, "note");
    let half = gui.edit(
        json!({"op": "quantize_notes", "clip_id": clip, "note_ids": [notes[0]],
                               "grid": 0.25, "strength": 0.5}),
    );
    assert_eq!(half.label, "Quantize Notes");
    let starts = note_starts(&gui, &clip);
    let moved = starts.iter().find(|(p, _)| *p == 70).unwrap().1;
    assert!((moved - 3.05).abs() < 1e-12, "{moved}");
    assert!(starts.contains(&(72, 5.1)), "the other note stays");
    // Without a selection, every note moves.
    gui.edit(json!({"op": "quantize_notes", "clip_id": clip, "grid": 0.25, "strength": 1.0}));
    let starts = note_starts(&gui, &clip);
    assert!(
        starts.contains(&(70, 3.0)) && starts.contains(&(72, 5.0)),
        "{starts:?}"
    );
}

/// `test_sidechain_selection_enables_external_key`: one step sets the source and turns the
/// compressor's `sidechain` switch on; clearing the source leaves the switch.
#[test]
fn a_sidechain_choice_is_one_step_with_its_switch() {
    let gui = Gui::new();
    let kick = gui.create("Kick", "instrument", Some("sonara.drums"));
    let bass = gui.create("Bass", "instrument", Some("sonara.synth.poly"));
    let comp = gui.effect(&bass, "sonara.dynamics.compressor");
    let outcome = gui.edit(json!({"op": "set_sidechain", "device_id": comp,
                                  "source_track_id": kick}));
    assert_eq!(outcome.label, "Set Sidechain");
    let device = gui.track(&bass).find_device(&comp).unwrap().clone();
    assert_eq!(device.sidechain_id.as_deref(), Some(kick.as_str()));
    let view = gui.view().project.unwrap();
    let comp_view = &view.tracks[1].inserts[0];
    assert_eq!(comp_view.sidechain_id.as_deref(), Some(kick.as_str()));
    assert_eq!(comp_view.params["sidechain"], json!(true));
    edit::undo(&gui.app).expect("one step");
    let device = gui.track(&bass).find_device(&comp).unwrap().clone();
    assert_eq!(device.sidechain_id, None);
    assert_eq!(
        view.tracks[1].inserts[0].params["sidechain"],
        json!(true),
        "the view before the undo"
    );
    gui.edit(json!({"op": "set_sidechain", "device_id": comp, "source_track_id": null}));
    assert_eq!(
        gui.track(&bass).find_device(&comp).unwrap().sidechain_id,
        None
    );
}

// -- events -----------------------------------------------------------------------------------

/// The window hears every edit, whichever thread made it, with the change set that chooses
/// between rebuilding and refreshing; the jobs' changes come on the same pump; and the GUI's
/// own edit is in the one session the agent reads.
#[test]
fn notices_reach_the_window_from_every_thread() {
    let gui = Gui::new();
    let subscriptions = Subscriptions::new(gui.app.session(), gui.app.jobs())
        .with_peaks(gui.app.peaks().subscribe());
    let (sender, received) = mpsc::channel();
    let pump = EventPump::start_notices(subscriptions, move |notice| {
        let _ = sender.send(notice);
    })
    .expect("the pump");

    // An agent's edits on another thread.
    let session = gui.app.session().clone();
    let ids = gui.app.ids();
    thread::spawn(move || {
        let mut ids = ids;
        let create = sonara_core::commands::track::CreateTrack {
            name: "Agent Track".to_owned(),
            kind: sonara_core::model::track::TrackKind::Audio,
            index: None,
            color: None,
            output_id: "master".to_owned(),
            instrument_plugin: None,
            instrument_preset: None,
            parent_id: None,
            track_id: ids.new_id("trk"),
            instrument_id: ids.new_id("dev"),
        };
        session
            .lock()
            .execute(&create, false, "agent")
            .expect("the agent's track");
        let mixer = sonara_core::commands::track::SetTrackMixer {
            track_id: create.track_id.clone(),
            volume_db: Some(-10.0),
            mute: Some(true),
            ..Default::default()
        };
        session
            .lock()
            .execute(&mixer, false, "agent")
            .expect("the agent's mixer edit");
    })
    .join()
    .expect("the agent thread");
    // The GUI's own edit, and a job.
    let track = gui.project().tracks[0].id.clone();
    gui.edit(json!({"op": "rename_track", "track_id": track, "name": "Renamed"}));
    let job = gui
        .app
        .jobs()
        .submit("test", "Test job", |_| {
            Ok(sonara_core::model::JsonMap::new())
        })
        .expect("a job");
    assert!(job.wait(Some(Duration::from_secs(5))));

    let mut sessions = Vec::new();
    let mut jobs = Vec::new();
    while sessions.len() < 3
        || !jobs
            .iter()
            .any(|j: &sonara_app::events::JobNotice| j.finished)
    {
        match received
            .recv_timeout(Duration::from_secs(5))
            .expect("a notice")
        {
            Notice::Session(notice) => sessions.push(notice),
            Notice::Job(notice) => jobs.push(notice),
            Notice::Peaks(_) => {}
        }
    }
    pump.stop();
    let create = &sessions[0];
    assert_eq!(
        (create.source.as_str(), create.label.as_str()),
        ("agent", "Create Track")
    );
    assert!(create.changes.structure_changed);
    assert_eq!(create.changes.created[0].kind, "track");
    let mixer = &sessions[1];
    assert_eq!(mixer.changes.flags, ["mixer"]);
    assert!(
        !mixer.changes.structure_changed,
        "a mixer edit refreshes in place"
    );
    assert_eq!(mixer.foreign_edit().as_deref(), Some("agent: Track Volume"));
    let rename = &sessions[2];
    assert_eq!(
        (rename.source.as_str(), rename.foreign_edit()),
        ("gui", None)
    );
    assert!(sessions.windows(2).all(|w| w[0].version < w[1].version));
    let done = jobs.iter().find(|j| j.finished).unwrap();
    assert_eq!(
        (done.label.as_str(), done.status),
        ("Test job", "succeeded")
    );
    // The GUI's edit is in the session the agent reads.
    assert_eq!(gui.project().tracks[0].name, "Renamed");
}
