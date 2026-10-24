//! The main window of the desktop application: the title and the Undo and
//! Redo items, the transport bar's calls, the start of live MIDI, the timeline's file drops as
//! an import job, and the time fixture the page's tempo and bar-position code shares.
//!
//! `test_dropping_audio_files_imports_them` here
//! (R, the import job; the Tauri drop event is the page's), and the time fixture is the oracle
//! of the page's display arithmetic (its position, its time and the waveform's frames).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use sonara_app::chrome::WindowChrome;
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::edit::{self, Edit};
use sonara_app::ids::IdSource;
use sonara_app::media::{
    AUDIO_IMPORT_SUFFIXES, IMPORT_AUDIO_JOB, ImportMessage, ImportReport, is_audio_import,
};
use sonara_app::midi::{MidiDevices, OpenInput};
use sonara_app::projects::NewProject;
use sonara_app::settings::Settings;
use sonara_app::transport_control;
use sonara_app::view::{ClipView, project_view};
use sonara_core::commands::project::{SetTempo, SetTimeSignature};
use sonara_core::ids::SequentialIds;
use sonara_core::model::track::TrackKind;
use sonara_io::MidiError;
use sonara_rt::midi_channel;
use tempfile::TempDir;

/// An application on `backend` with a new project "Song", and optional MIDI devices.
struct Window {
    app: AppContext,
    dir: TempDir,
}

impl Window {
    fn new(backend: &str) -> Self {
        Self::with(backend, Settings::default(), None)
    }

    fn with(backend: &str, settings: Settings, midi: Option<Arc<dyn MidiDevices>>) -> Self {
        Self::build(backend, settings, midi, None)
    }

    /// The application with sequential IDs, for a fixture that must not change between runs.
    fn sequential() -> Self {
        let ids = IdSource::new(Box::new(SequentialIds::new(0)));
        Self::build("null", Settings::default(), None, Some(ids))
    }

    fn build(
        backend: &str,
        settings: Settings,
        midi: Option<Arc<dyn MidiDevices>>,
        ids: Option<IdSource>,
    ) -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let app = AppContext::create(AppOptions {
            workspaces: Some(dir.path().join("ws")),
            settings: Some(settings),
            persist_settings: false,
            audio_backend: Some(backend.to_owned()),
            midi_devices: midi,
            ids,
            ..AppOptions::default()
        })
        .expect("the application");
        app.projects()
            .new_project(&NewProject {
                name: "Song".to_owned(),
                ..NewProject::default()
            })
            .expect("a project");
        Self { app, dir }
    }

    fn edit(&self, edit: Value) -> edit::EditOutcome {
        edit::apply(&self.app, Edit::from_json(edit).expect("an edit")).expect("the edit")
    }

    fn chrome(&self) -> WindowChrome {
        WindowChrome::of(self.app.session())
    }
}

// -- the title and the Undo and Redo items ----------------------------------------------------------

#[test]
fn the_title_marks_unsaved_changes_and_names_the_place() {
    let window = Window::new("null");
    let fresh = window.chrome();
    assert_eq!(fresh.title, "Song — not saved — Sonara");
    assert!(!fresh.dirty);
    assert_eq!((fresh.undo.as_str(), fresh.can_undo), ("Undo", false));
    assert_eq!((fresh.redo.as_str(), fresh.can_redo), ("Redo", false));

    window.edit(json!({"op": "create_track", "name": "Lead", "kind": "audio"}));
    let edited = window.chrome();
    assert_eq!(edited.title, "Song • — not saved — Sonara");
    assert!(edited.dirty);
    assert_eq!(
        (edited.undo.as_str(), edited.can_undo),
        ("Undo Create Track", true)
    );

    edit::undo(&window.app).expect("an undo");
    let undone = window.chrome();
    assert_eq!((undone.undo.as_str(), undone.can_undo), ("Undo", false));
    assert_eq!(
        (undone.redo.as_str(), undone.can_redo),
        ("Redo Create Track", true)
    );

    let path = window.dir.path().join("Song.sonara");
    window
        .app
        .projects()
        .save(Some(&path), None)
        .expect("saved");
    let saved = window.chrome();
    let shown = window
        .app
        .session()
        .lock()
        .location()
        .and_then(|location| location.path.clone())
        .expect("a path");
    assert_eq!(saved.title, format!("Song — {} — Sonara", shown.display()));
    assert!(!saved.dirty);

    window.app.projects().close(true).expect("closed");
    assert_eq!(window.chrome().title, "Sonara");
}

// -- the transport bar ------------------------------------------------------------------------------

#[test]
fn without_a_backend_play_fails_and_the_label_says_why() {
    let window = Window::new("null");
    assert_eq!(transport_control::engine_status(&window.app), None);
    // A stop with nothing playing creates no transport (TB-03 asks first).
    let stopped = transport_control::stop(&window.app).expect("a stop");
    assert!(!stopped.was_playing);
    assert_eq!((stopped.takes, stopped.message), (None, None));
    assert!(!window.app.has_transport());

    let error = transport_control::play(&window.app, 0.0, false).expect_err("no backend");
    assert_eq!(error.code, "ENGINE_UNAVAILABLE");
    assert_eq!(error.message, "audio backend disabled (null backend)");
    let status = transport_control::engine_status(&window.app).expect("a transport now");
    assert_eq!(status["available"], false);
    assert_eq!(status["reason"], "audio backend disabled (null backend)");
    let error = transport_control::seek(&window.app, 4.0).expect_err("no backend");
    assert_eq!(error.code, "ENGINE_UNAVAILABLE");
}

#[test]
fn play_seek_and_stop_on_the_simulated_device() {
    let window = Window::new("native-null");
    let played = transport_control::play(&window.app, 2.0, false).expect("playing");
    assert_eq!(played["state"], "playing");
    assert_eq!(played["position_beat"], 2.0);
    let status = transport_control::engine_status(&window.app).expect("a transport");
    assert_eq!(status["available"], true);
    assert!(
        status["device_sample_rate"]
            .as_u64()
            .is_some_and(|rate| rate > 0)
    );

    let sought = transport_control::seek(&window.app, 8.0).expect("a seek");
    assert_eq!(sought["position_beat"], 8.0);
    let stopped = transport_control::stop(&window.app).expect("a stop");
    assert!(stopped.was_playing);
    assert_eq!(stopped.takes, None, "no recording ended");
    assert_eq!(stopped.message, None);
    let again = transport_control::stop(&window.app).expect("a second stop");
    assert!(!again.was_playing);

    // TB-04 without an armed track: the error the status bar shows.
    let error = transport_control::record(&window.app, 0.0).expect_err("nothing armed");
    assert_eq!(error.code, "INVALID_ARGUMENT");
    assert_eq!(
        error.message,
        "no armed audio or instrument tracks to record"
    );
}

#[test]
fn a_recording_that_ends_reports_its_takes() {
    let window = Window::new("native-null");
    let track = window
        .edit(json!({"op": "create_track", "name": "Keys",
                                   "kind": "instrument", "instrument": "sonara.synth.poly"}))
        .result["track_id"]
        .as_str()
        .expect("a track")
        .to_owned();
    window.edit(json!({"op": "set_track_input", "track_id": track, "armed": true}));
    let recording = transport_control::record(&window.app, 0.0).expect("recording");
    assert_eq!(recording["recording"], true);
    std::thread::sleep(Duration::from_millis(200));
    let stopped = transport_control::stop(&window.app).expect("a stop");
    assert!(stopped.was_playing);
    // No MIDI arrived, so the instrument track gives no take.
    assert_eq!(stopped.takes, Some(0));
    let message = stopped.message.expect("a message");
    assert_eq!(message.text, "Recorded 0 take(s)");
    assert!(!message.error);
}

// -- live MIDI at startup ------------------------------------------------------------------------

/// MIDI devices with fixed input ports that never send.
struct Ports(Mutex<Vec<String>>);

impl MidiDevices for Ports {
    fn api_name(&self) -> &'static str {
        "core"
    }

    fn input_ports(&self) -> Result<Vec<String>, MidiError> {
        Ok(self.0.lock().expect("an unpoisoned lock").clone())
    }

    fn output_ports(&self) -> Result<Vec<String>, MidiError> {
        Ok(Vec::new())
    }

    fn open_input(&self, _name: &str, _port: u8) -> Result<OpenInput, MidiError> {
        let (sender, events) = midi_channel(64);
        Ok(OpenInput {
            events,
            overflows: sender.overflows(),
            connection: Box::new(sender),
        })
    }

    fn open_virtual_input(&self, name: &str, _port: u8) -> Result<OpenInput, MidiError> {
        Err(MidiError::Connect {
            name: name.to_owned(),
            reason: "no virtual ports here".to_owned(),
        })
    }
}

fn ports(names: &[&str]) -> Arc<dyn MidiDevices> {
    Arc::new(Ports(Mutex::new(
        names.iter().map(|&name| name.to_owned()).collect(),
    )))
}

#[test]
fn live_midi_starts_only_with_a_backend_and_input_ports() {
    let keys = Window::with("native-null", Settings::default(), Some(ports(&["Keys"])));
    assert_eq!(transport_control::start_live_midi(&keys.app), Ok(true));
    assert!(keys.app.has_transport());
    assert_eq!(keys.app.transport().midi_status()["ports"], json!(["Keys"]));

    let none = Window::with("native-null", Settings::default(), Some(ports(&[])));
    assert_eq!(transport_control::start_live_midi(&none.app), Ok(false));
    assert!(!none.app.has_transport(), "no ports: no transport either");

    let off = Window::with("null", Settings::default(), Some(ports(&["Keys"])));
    assert_eq!(transport_control::start_live_midi(&off.app), Ok(false));
    assert!(!off.app.has_transport());

    // The prototype also checks the settings' backend, whatever the launch option says.
    let mut settings = Settings::default();
    settings.audio.backend = "null".to_owned();
    let configured_off = Window::with("native-null", settings, Some(ports(&["Keys"])));
    assert_eq!(
        transport_control::start_live_midi(&configured_off.app),
        Ok(false)
    );
    assert!(!configured_off.app.has_transport());
}

// -- file drops -----------------------------------------------------------------------

/// The prototype test's file: half a second of `0.3 sin(n / 10)` at 48 kHz, mono.
fn write_drop(path: &Path) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("a WAV writer");
    for n in 0..24_000_u32 {
        #[expect(clippy::cast_possible_truncation, reason = "a float test signal")]
        let sample = (0.3 * (f64::from(n) / 10.0).sin()) as f32;
        writer.write_sample(sample).expect("a sample");
    }
    writer.finalize().expect("the WAV");
}

fn run_import(window: &Window, paths: Vec<PathBuf>, track: Option<String>, beat: f64) -> Value {
    let job = window
        .app
        .media()
        .import_audio_job(paths, track, beat)
        .expect("the import job");
    assert_eq!(job.kind(), IMPORT_AUDIO_JOB);
    assert!(
        job.wait(Some(Duration::from_secs(20))),
        "the import finished"
    );
    let snapshot = job.snapshot();
    assert_eq!(snapshot.status.as_str(), "succeeded", "{snapshot:?}");
    Value::Object(snapshot.result.expect("a result"))
}

/// `test_dropping_audio_files_imports_them`: a drop off every audio track imports the file into
/// a new audio track at the drop's beat, on a job worker.
#[test]
fn dropping_audio_files_imports_them() {
    let window = Window::new("null");
    let path = window.dir.path().join("drop.wav");
    write_drop(&path);
    let result = run_import(&window, vec![path.clone()], None, 4.0);
    let project = window.app.session().project().expect("a project");
    let track = project
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Audio)
        .expect("a new audio track");
    assert_eq!(track.name, "drop");
    assert_eq!(track.clips.len(), 1);
    assert_eq!(track.clips[0].start(), 4.0);
    assert_eq!(result["imported"][0]["track_id"], track.id.as_str());
    assert_eq!(result["failed"], json!([]));
    assert_eq!(
        ImportReport::of(&result).messages,
        [ImportMessage {
            text: "Imported drop.wav".to_owned(),
            error: false
        }]
    );
    // The window's undo takes the import back in one step.
    let undone = edit::undo(&window.app).expect("an undo");
    assert_eq!(undone.label, "Undo Import Audio");
}

#[test]
fn a_drop_on_an_audio_track_imports_there_and_reports_each_file() {
    let window = Window::new("null");
    let track = window
        .edit(json!({"op": "create_track", "name": "Vox", "kind": "audio"}))
        .result["track_id"]
        .as_str()
        .expect("a track")
        .to_owned();
    let good = window.dir.path().join("take.wav");
    write_drop(&good);
    let missing = window.dir.path().join("gone.wav");
    let result = run_import(&window, vec![missing, good], Some(track.clone()), 2.0);
    let project = window.app.session().project().expect("a project");
    let clips = &project.track(&track).expect("the track").clips;
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].start(), 2.0);
    assert_eq!(result["imported"][0]["file"], "take.wav");
    assert_eq!(result["failed"][0]["file"], "gone.wav");
    assert_eq!(result["failed"][0]["code"], "NOT_FOUND");
    let messages = ImportReport::of(&result).messages;
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].text, "Imported take.wav");
    assert!(messages[1].error);
    assert!(
        messages[1].text.starts_with("Import of gone.wav failed: "),
        "{}",
        messages[1].text
    );
    // The view shows the clip the page draws.
    let view = project_view(window.app.session(), window.app.plugins());
    let shown = view
        .project
        .expect("a project")
        .tracks
        .into_iter()
        .find(|t| t.id == track)
        .expect("the track");
    assert!(matches!(&shown.clips[0], ClipView::Audio(clip) if (clip.start - 2.0).abs() < 1e-9));
}

#[test]
fn the_drop_suffixes_are_the_timelines() {
    assert_eq!(AUDIO_IMPORT_SUFFIXES.len(), 14);
    for path in [
        "a.wav",
        "B.AIFF",
        "c.d.flac",
        "song.Rf64",
        "x.aac",
        "voice.m4a",
    ] {
        assert!(is_audio_import(Path::new(path)), "{path}");
    }
    for path in ["a.sonara", "notes.txt", "wav", ".wav", "a.mid", "folder/"] {
        assert!(!is_audio_import(Path::new(path)), "{path}");
    }
}

// -- the time fixture of the page ----------------------------------------------------------------------

/// The tempo map, bar positions and MIDI playback mapping that the page computes for display
/// (the transport bar's position and time, the ruler, the waveform's frames, the mini notes):
/// `apps/sonara-desktop/tests/fixtures/time-v1.json`, which `tests/unit/time.test.ts` checks
/// the page against. `SONARA_WRITE_FIXTURES=1` rewrites it; otherwise this test checks that
/// `sonara-core` still gives exactly those values.
#[test]
fn the_page_shares_the_time_fixture() {
    let (window, plain, looped) = time_session();
    let document = time_document(&window, &plain, &looped);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/sonara-desktop/tests/fixtures/time-v1.json");
    if std::env::var_os("SONARA_WRITE_FIXTURES").is_some() {
        let text = serde_json::to_string_pretty(&document).expect("JSON") + "\n";
        fs::write(&path, text).expect("the fixture is written");
    }
    let fixture: Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("the shared fixture"))
            .expect("JSON");
    assert_eq!(
        fixture, document,
        "sonara-core still gives the fixture's values"
    );
}

/// A session with a ramped tempo map, four time signatures and two MIDI clips, one plain with a
/// content offset and one looped; the IDs of the two clips.
fn time_session() -> (Window, String, String) {
    let window = Window::sequential();
    let execute = |command: &dyn sonara_core::commands::Command| {
        window
            .app
            .session()
            .lock()
            .execute(command, false, "test")
            .expect("the command");
    };
    // A ramp from 100 to 160 BPM over 16 beats, then steps.
    for (beat, bpm, ramp) in [
        (0.0, 100.0, true),
        (16.0, 160.0, false),
        (24.0, 90.0, false),
    ] {
        execute(&SetTempo {
            bpm,
            beat,
            ramp,
            clear_changes: false,
        });
    }
    for (bar, numerator, denominator) in [(2, 3, 4), (4, 7, 8), (6, 12, 8)] {
        execute(&SetTimeSignature {
            numerator,
            denominator,
            bar,
        });
    }
    let track = window
        .edit(json!({"op": "create_track", "name": "Keys",
                                   "kind": "instrument", "instrument": "sonara.synth.poly"}))
        .result["track_id"]
        .as_str()
        .expect("a track")
        .to_owned();
    let plain = window
        .edit(
            json!({"op": "create_midi_clip", "track_id": track, "start": 2.0,
                                   "length": 6.0, "name": "Plain"}),
        )
        .changes
        .created[0]
        .id
        .clone();
    let looped = window
        .edit(
            json!({"op": "create_midi_clip", "track_id": track, "start": 12.0,
                                    "length": 9.5, "name": "Looped"}),
        )
        .changes
        .created[0]
        .id
        .clone();
    for clip in [&plain, &looped] {
        window.edit(json!({"op": "add_notes", "clip_id": clip, "notes": [
            {"pitch": 60, "start": 0.0, "length": 0.5, "velocity": 100},
            {"pitch": 64, "start": 0.75, "length": 2.0, "velocity": 40},
            {"pitch": 67, "start": 1.5, "length": 0.25, "velocity": 64},
            {"pitch": 72, "start": 2.5, "length": 1.0, "velocity": 90},
            {"pitch": 48, "start": 5.75, "length": 1.0, "velocity": 127}
        ]}));
    }
    window.edit(json!({"op": "trim_clip", "clip_id": plain, "start": 2.5}));
    window.edit(json!({"op": "set_clip_properties", "clip_id": looped, "loop_length": 3.0}));
    window.edit(json!({"op": "trim_clip", "clip_id": looped, "start": 13.25}));
    (window, plain, looped)
}

/// The fixture's values of `window`'s session, as sonara-core computes them.
fn time_document(window: &Window, plain: &str, looped: &str) -> Value {
    let project = window.app.session().project().expect("a project");
    let view = project_view(window.app.session(), window.app.plugins())
        .project
        .expect("a project");
    let tempo = &project.tempo_map;
    let signatures = &project.time_signatures;
    let beats = [
        -2.0,
        0.0,
        0.25,
        1.0 / 1920.0,
        3.75,
        7.999_999_999_9,
        8.0,
        11.3,
        16.0,
        19.5,
        24.0,
        31.9,
        47.123,
    ];
    let seconds = [-1.0, 0.0, 0.3, 2.0, 7.5, 9.6, 12.25, 20.0];
    let bars: [f64; 10] = [0.0, 1.0, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 7.0, 9.25];
    let clips: Vec<Value> = [plain, looped]
        .iter()
        .map(|&id| {
            let (_, clip) = project.clip(id).expect("the clip");
            let midi = clip.as_midi().expect("a MIDI clip");
            let shown = view
                .tracks
                .iter()
                .flat_map(|t| &t.clips)
                .find(|c| match c {
                    ClipView::Midi(c) => c.id == id,
                    ClipView::Audio(_) => false,
                })
                .expect("the clip in the view");
            json!({
                "clip": shown,
                "playback": midi
                    .playback_notes()
                    .iter()
                    .map(|(start, note)| json!([start, note.id]))
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "note": "Written by crates/sonara-app/tests/gui_window.rs with SONARA_WRITE_FIXTURES=1 \
                 from sonara-core's TempoMap, TimeSignatureMap and playback_notes. The page's \
                 tests/unit/time.test.ts checks its own arithmetic against it.",
        "tempo": view.tempo,
        "time_signatures": view.time_signatures,
        "beat_to_seconds": beats.iter().map(|&b| json!([b, tempo.beat_to_seconds(b)]))
            .collect::<Vec<_>>(),
        "seconds_to_beat": seconds.iter().map(|&s| json!([s, tempo.seconds_to_beat(s)]))
            .collect::<Vec<_>>(),
        "positions": beats.iter().filter(|&&b| b >= 0.0).map(|&b| json!([
            b,
            signatures.position(b).to_string(),
            signatures.beat_to_bar(b),
        ])).collect::<Vec<_>>(),
        "bars": bars.iter().map(|&bar| {
            #[expect(clippy::cast_possible_truncation, reason = "small whole bars")]
            let whole = bar.floor() as i64;
            json!([bar, signatures.bar_to_beat(bar), signatures.bar_length(whole)])
        }).collect::<Vec<_>>(),
        "clips": clips,
    })
}
