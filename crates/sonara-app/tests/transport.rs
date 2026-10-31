//! The transport service on miniaudio's null device: the
//! prototype's engine tests through the service (`tests/engine/test_realtime.py`,
//! rows 4–9, with its 250 ms test lookahead), the container twin of its live-MIDI test (row 12b,
//! `test_midi_live.py` with 150 ms) on injected MIDI devices whose rings the test feeds, the
//! controllers a MIDI take keeps, the backends' answers, the shutdown that finishes a
//! recording, and every service call from several threads during playback and edits (RT-06).
//!
//! The null device is a simulated real-time device on a timer thread, so these tests wait for
//! it with the prototype's timings and tolerances.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::midi::{MidiDevices, OpenInput};
use sonara_app::projects::NewProject;
use sonara_app::settings::Settings;
use sonara_app::transport::TransportService;
use sonara_core::commands::Command;
use sonara_core::commands::clip::CreateMidiClip;
use sonara_core::commands::track::{CreateTrack, SetTrackInput, SetTrackMixer};
use sonara_core::error::ErrorCode;
use sonara_core::ids::{self, IdGenerator};
use sonara_core::midi::{ControlKind, MidiNote};
use sonara_core::model::clip::Clip;
use sonara_core::model::track::TrackKind;
use sonara_dsp::processor::{EventKind, InstrumentEvent};
use sonara_io::MidiError;
use sonara_rt::{MidiEvent, MidiSender, midi_channel};
use tempfile::TempDir;

/// An application on `native-null` with a new 120 BPM project, as the prototype's `rt_app`.
struct Rt {
    app: AppContext,
    _dir: TempDir,
}

impl Rt {
    fn new(lookahead_ms: f64, midi: Option<Arc<dyn MidiDevices>>) -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let mut settings = Settings::default();
        settings.audio.lookahead_ms = lookahead_ms;
        let app = AppContext::create(AppOptions {
            workspaces: Some(dir.path().join("ws")),
            settings: Some(settings),
            persist_settings: false,
            audio_backend: Some("native-null".to_owned()),
            midi_devices: midi,
            ..AppOptions::default()
        })
        .expect("the application");
        app.projects()
            .new_project(&NewProject {
                name: "RT".to_owned(),
                bpm: 120.0,
                ..NewProject::default()
            })
            .expect("a project");
        Self { app, _dir: dir }
    }

    fn execute(&self, command: &dyn Command) {
        self.app
            .session()
            .lock()
            .execute(command, false, "test")
            .expect("the command");
    }

    fn track(&self, name: &str, kind: TrackKind, plugin: Option<(&str, Option<&str>)>) -> String {
        let mut ids = self.app.ids();
        let create = CreateTrack {
            name: name.to_owned(),
            kind,
            index: None,
            color: None,
            output_id: ids::MASTER_ID.to_owned(),
            instrument_plugin: plugin.map(|(p, _)| p.to_owned()),
            instrument_preset: plugin.and_then(|(_, preset)| preset.map(str::to_owned)),
            parent_id: None,
            track_id: ids.new_id(ids::TRACK),
            instrument_id: ids.new_id(ids::DEVICE),
            part: None,
        };
        self.execute(&create);
        create.track_id
    }

    fn arm(&self, track_id: &str, channels: Option<Vec<i64>>) {
        self.execute(&SetTrackInput {
            track_id: track_id.to_owned(),
            armed: Some(true),
            monitor: None,
            channels,
        });
    }

    /// `_drum_project`: a drum track with a kick on every beat for 16 beats.
    fn drums(&self) -> String {
        let track = self.track("Drums", TrackKind::Instrument, Some(("sonara.drums", None)));
        let mut ids = self.app.ids();
        let notes = (0..16)
            .map(|beat| {
                MidiNote::create(&mut ids, 36, f64::from(beat), 0.25, 110, 0).expect("a note")
            })
            .collect();
        self.execute(&CreateMidiClip {
            track_id: track.clone(),
            start: 0.0,
            length: 16.0,
            name: "Beat".to_owned(),
            notes,
            loop_length: None,
            color: None,
            clip_id: ids.new_id(ids::CLIP),
        });
        track
    }

    fn status(&self) -> Value {
        self.app.transport().status()
    }
}

fn wait_until(timeout: Duration, mut predicate: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        if predicate() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

/// Waits until the render thread has actually produced a block for the new recording
/// generation: `record()` arms the state synchronously, but the render thread only picks up
/// the new cue and starts capturing MIDI once it runs its play step for it. `position_beat`
/// cannot tell this apart from the idle step that already ran before `record()` (both tag
/// their blocks with the resting frame), so this polls `render_load`, which the idle step is
/// excluded from (`blocks_rendered`, `shared.rs`) and so stays `0.0` until a real block of the
/// new generation has rendered (TST-04: a fixed sleep here races a CPU-starved runner instead).
fn wait_until_rolling(transport: &TransportService) -> bool {
    wait_until(Duration::from_secs(10), || {
        transport.status()["render_load"]
            .as_f64()
            .is_some_and(|load| load > 0.0)
    })
}

fn number(value: &Value) -> f64 {
    value.as_f64().expect("a number")
}

/// Row 4: plays from 0; the position advances with the device; nothing underruns.
#[test]
fn player_plays_without_underruns_and_tracks_position() {
    let rt = Rt::new(250.0, None);
    rt.drums();
    let transport = rt.app.transport();
    let started = Instant::now();
    let status = transport.play(Some(0.0), None, false).expect("play");
    assert_eq!(status["state"], "playing");
    thread::sleep(Duration::from_millis(600));
    let status = rt.status();
    let elapsed = started.elapsed().as_secs_f64();
    assert_eq!(status["underruns"], 0);
    // 0.6 s at 120 BPM is 1.2 beats; allow 0.2 beats for lookahead and scheduling jitter. The
    // null device runs on wall-clock time and a loaded runner can oversleep, so the upper bound
    // follows the measured elapsed time, not the requested sleep (TST-04).
    let beat = number(&status["position_beat"]);
    let upper = elapsed * 2.0 + 0.2;
    assert!(
        0.6 < beat && beat < upper,
        "position {beat}, elapsed {elapsed:.3} s"
    );
    assert!(number(&status["render_load"]) < 1.0);
    let stopped = transport.stop().expect("stop");
    assert_eq!(stopped["was_playing"], true);
    assert_eq!(rt.status()["state"], "stopped");
}

/// Row 5: a seek while playing moves the position; a two-beat loop keeps it inside the loop.
#[test]
fn player_seek_and_loop() {
    let rt = Rt::new(250.0, None);
    rt.drums();
    let transport = rt.app.transport();
    transport.play(Some(0.0), None, false).expect("play");
    transport.seek(8.0).expect("seek");
    thread::sleep(Duration::from_millis(200));
    assert!(transport.position_beats() >= 8.0);
    transport.stop().expect("stop");
    transport.play(Some(0.0), Some(2.0), true).expect("a loop");
    thread::sleep(Duration::from_millis(1500));
    let beat = transport.position_beats();
    assert!((0.0..2.0).contains(&beat), "position {beat}");
    assert_eq!(rt.status()["underruns"], 0);
    transport.stop().expect("stop");
}

/// Row 6: a play range stops by itself at its end.
#[test]
fn play_range_stops_at_end() {
    let rt = Rt::new(250.0, None);
    rt.drums();
    let transport = rt.app.transport();
    transport.play(Some(0.0), Some(1.0), false).expect("play");
    assert!(wait_until(Duration::from_secs(3), || rt.status()["state"] == "stopped"));
    assert!((transport.position_beats() - 1.0).abs() <= 0.05);
}

/// Row 7: an edit during playback is picked up without underruns.
#[test]
fn edit_during_playback_is_picked_up() {
    let rt = Rt::new(250.0, None);
    let drums = rt.drums();
    let transport = rt.app.transport();
    transport.play(Some(0.0), None, false).expect("play");
    thread::sleep(Duration::from_millis(200));
    rt.execute(&SetTrackMixer {
        track_id: drums,
        volume_db: Some(-12.0),
        ..SetTrackMixer::default()
    });
    thread::sleep(Duration::from_millis(300));
    let version = rt.app.session().lock().version();
    assert_eq!(transport.graph_version(), Some(version));
    assert_eq!(rt.status()["underruns"], 0);
    transport.stop().expect("stop");
}

/// Row 8: the null device captures silence; the pipeline (capture ring, writer thread, WAV
/// take, asset and clip with latency compensation, one undo step) is what is under test.
#[test]
fn recording_creates_take_clip_on_armed_track() {
    let rt = Rt::new(250.0, None);
    rt.app.settings().audio.input_channels = 1;
    let vocal = rt.track("Vocal", TrackKind::Audio, None);
    rt.arm(&vocal, Some(vec![0]));
    let transport = rt.app.transport();
    let status = transport.record(Some(4.0), None).expect("record");
    assert_eq!(status["recording"], true);
    assert_eq!(status["tracks"], json!([vocal]));
    assert_eq!(
        status["state"],
        Value::Null,
        "the record answer has no state"
    );
    assert_eq!(rt.status()["state"], "recording");
    thread::sleep(Duration::from_millis(800));
    let result = transport.stop().expect("stop");
    let recording = &result["recording"];
    let keys: Vec<&String> = recording.as_object().expect("an object").keys().collect();
    assert_eq!(
        keys,
        ["takes", "latency_compensation_frames", "capture_overruns"]
    );
    // The null device's latency estimates: 256 × 3 frames out and in.
    assert_eq!(recording["latency_compensation_frames"], 1536);
    assert_eq!(recording["capture_overruns"], 0);
    let takes = recording["takes"].as_array().expect("the takes");
    assert_eq!(takes.len(), 1);
    let take = &takes[0];
    let keys: Vec<&String> = take.as_object().expect("an object").keys().collect();
    assert_eq!(keys, ["track_id", "asset_id", "clip_id", "seconds"]);
    let seconds = number(&take["seconds"]);
    assert!(0.4 < seconds && seconds < 1.2, "{seconds} s");
    let project = rt.app.session().project().expect("the project");
    let (track, clip) = project
        .clip(take["clip_id"].as_str().expect("a clip ID"))
        .expect("the clip");
    assert_eq!(track.id, vocal);
    // Recording started at beat 4 (no count-in); latency compensation may only trim.
    let Clip::Audio(clip) = clip else {
        panic!("an audio clip");
    };
    assert!((clip.start - 4.0).abs() <= 0.05, "clip at {}", clip.start);
    assert_eq!(clip.source_offset, 1536, "aligned at the capture start");
    assert_eq!(clip.name, "Vocal take");
    let asset = project
        .asset(take["asset_id"].as_str().expect("an asset ID"))
        .expect("the asset");
    assert_eq!(
        (asset.name.as_str(), asset.subtype.as_str()),
        ("Vocal take", "PCM_24")
    );
    let workspace = rt
        .app
        .session()
        .lock()
        .location()
        .expect("a location")
        .workspace
        .clone();
    let file = workspace.join(&asset.path);
    let reader = hound::WavReader::open(&file).expect("the take");
    assert_eq!(reader.spec().channels, 1);
    assert!(u64::from(reader.duration()) >= asset.frames && asset.frames > 0);
    // The whole take is one undoable step.
    rt.app.session().lock().undo("test").expect("undo");
    let project = rt.app.session().project().expect("the project");
    assert!(
        project
            .find_clip(take["clip_id"].as_str().unwrap())
            .is_none()
    );
    assert!(project.find_asset(asset.id.as_str()).is_none());
}

/// Row 9: nothing armed, nothing recorded.
#[test]
fn record_without_armed_tracks_is_rejected() {
    let rt = Rt::new(250.0, None);
    let error = rt
        .app
        .transport()
        .record(None, None)
        .expect_err("no armed track");
    let sonara_app::error::AppError::Domain(error) = error else {
        panic!("a domain error: {error}");
    };
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert!(error.message.contains("armed"), "{}", error.message);
    assert_eq!(rt.status()["state"], "stopped");
}

/// A keyboard whose ports the test plays: injected MIDI devices (TST-08) with one input, whose
/// ring the test feeds with real arrival times.
struct Keyboard {
    name: String,
    start: Instant,
    senders: Mutex<Vec<(u8, MidiSender)>>,
}

impl Keyboard {
    fn new(name: &str) -> Arc<Self> {
        Arc::new(Self {
            name: name.to_owned(),
            start: Instant::now(),
            senders: Mutex::new(Vec::new()),
        })
    }

    fn send(&self, bytes: &[u8]) {
        let time = u64::try_from(self.start.elapsed().as_micros()).expect("a short test");
        let mut senders = self.senders.lock().expect("an unpoisoned lock");
        let (port, sender) = senders.last_mut().expect("an open input");
        assert!(sender.push(MidiEvent::new(time, *port, bytes).expect("a message")));
    }
}

impl MidiDevices for Keyboard {
    fn api_name(&self) -> &'static str {
        "core"
    }

    fn input_ports(&self) -> Result<Vec<String>, MidiError> {
        Ok(vec![self.name.clone()])
    }

    fn output_ports(&self) -> Result<Vec<String>, MidiError> {
        Ok(Vec::new())
    }

    fn open_input(&self, name: &str, port: u8) -> Result<OpenInput, MidiError> {
        if name != self.name {
            return Err(MidiError::InputNotFound {
                name: name.to_owned(),
                available: vec![self.name.clone()],
            });
        }
        let (sender, events) = midi_channel(4096);
        let overflows = sender.overflows();
        self.senders
            .lock()
            .expect("an unpoisoned lock")
            .push((port, sender));
        Ok(OpenInput {
            events,
            overflows,
            connection: Box::new(()),
        })
    }

    fn open_virtual_input(&self, name: &str, _port: u8) -> Result<OpenInput, MidiError> {
        Err(MidiError::Connect {
            name: name.to_owned(),
            reason: "no virtual ports here".to_owned(),
        })
    }
}

/// An armed synth and a keyboard: the application with its MIDI input open through the hub
/// (`set_midi_input`). The prototype's test attached its virtual port "Sonara Test Keys" to the
/// player directly; the hub never opens ports named "Sonara…", so the keyboard here is "Test
/// Keys".
fn live_rt(keyboard: &Arc<Keyboard>) -> (Rt, String) {
    let devices: Arc<dyn MidiDevices> = Arc::clone(keyboard) as Arc<dyn MidiDevices>;
    let rt = Rt::new(150.0, Some(devices));
    let synth = rt.track(
        "Keys",
        TrackKind::Instrument,
        Some(("sonara.synth.poly", Some("Pluck"))),
    );
    rt.arm(&synth, None);
    let opened = rt
        .app
        .transport()
        .enable_midi(Some(vec![keyboard.name.clone()]))
        .expect("MIDI input");
    assert_eq!(opened, json!({"enabled": true, "ports": [keyboard.name]}));
    (rt, synth)
}

/// Row 12b, the container twin of `test_live_midi_plays_while_stopped_and_records_into_a_clip`:
/// while stopped, a note reaches the armed synth; recording from 0 gives one MIDI take with the
/// two notes played, placed and sized as they were played.
#[test]
fn live_midi_injected_records_into_clip() {
    let keyboard = Keyboard::new("Test Keys");
    let (rt, synth) = live_rt(&keyboard);
    let transport = rt.app.transport();
    // Stopped transport: live input must still reach the instrument.
    keyboard.send(&[0x90, 60, 110]);
    assert!(wait_until(Duration::from_secs(2), || {
        transport.midi_status()["messages_received"]
            .as_u64()
            .is_some_and(|n| n >= 1)
    }));
    let status = transport.midi_status();
    assert_eq!(status["targets"], json!([synth]));
    assert_eq!(status["ports"], json!([keyboard.name]));
    keyboard.send(&[0x80, 60, 0]);
    // Record two notes from beat 0.
    transport.record(Some(0.0), None).expect("record");
    assert!(
        wait_until_rolling(&transport),
        "the render thread never advanced past the record start"
    );
    thread::sleep(Duration::from_millis(300));
    keyboard.send(&[0x90, 62, 100]);
    thread::sleep(Duration::from_millis(250));
    keyboard.send(&[0x80, 62, 0]);
    keyboard.send(&[0x90, 65, 80]);
    thread::sleep(Duration::from_millis(200));
    keyboard.send(&[0x80, 65, 0]);
    thread::sleep(Duration::from_millis(200));
    let result = transport.stop().expect("stop");
    // HW-01/TST-04: the null device's timeline only advances with frames it actually rendered.
    // On a CPU-starved runner it underruns and falls behind wall clock, so the absolute pacing
    // below (which assumes the device kept real time) is not asserted there.
    let real_time = transport.status()["underruns"].as_u64() == Some(0);
    let takes = result["recording"]["takes"].as_array().expect("the takes");
    assert_eq!(takes.len(), 1);
    assert_eq!(takes[0]["notes"], 2);
    assert_eq!(takes[0]["controllers"], 0);
    let keys: Vec<&String> = takes[0].as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        ["track_id", "clip_id", "notes", "controllers", "beats"]
    );
    let project = rt.app.session().project().expect("the project");
    let (track, clip) = project
        .clip(takes[0]["clip_id"].as_str().unwrap())
        .expect("the clip");
    assert_eq!(track.id, synth);
    let Clip::Midi(clip) = clip else {
        panic!("a MIDI clip");
    };
    let pitches: Vec<u8> = clip.notes.iter().map(|n| n.pitch).collect();
    assert_eq!(pitches, [62, 65]);
    assert_eq!(clip.notes[1].velocity, 80);
    assert_eq!(clip.name, "Keys take");
    if !real_time {
        eprintln!("the device underran while recording; skipping the pacing check");
        return;
    }
    let first = &clip.notes[0];
    // About 0.3 s after the start at 120 BPM, about 0.6 beats (allow for scheduling and the
    // latency estimates).
    assert!(
        0.2 < first.start && first.start < 1.2,
        "start {}",
        first.start
    );
    assert!(
        0.2 < first.length && first.length < 1.0,
        "length {}",
        first.length
    );
}

/// A MIDI take keeps its controllers (the prototype counted them in `"controllers"` and
/// dropped them): controller, pitch bend and channel pressure land in the clip's `controls`, in
/// the same undo step as the clip.
#[test]
fn midi_take_keeps_controllers() {
    let keyboard = Keyboard::new("Test Keys");
    let (rt, synth) = live_rt(&keyboard);
    let transport = rt.app.transport();
    transport.record(Some(0.0), None).expect("record");
    assert!(
        wait_until_rolling(&transport),
        "the render thread never advanced past the record start"
    );
    thread::sleep(Duration::from_millis(200));
    keyboard.send(&[0x90, 64, 90]);
    thread::sleep(Duration::from_millis(50));
    keyboard.send(&[0xB1, 1, 99]);
    thread::sleep(Duration::from_millis(50));
    keyboard.send(&[0xE0, 0x00, 0x60]);
    thread::sleep(Duration::from_millis(50));
    keyboard.send(&[0xD2, 42]);
    thread::sleep(Duration::from_millis(50));
    keyboard.send(&[0x80, 64, 0]);
    thread::sleep(Duration::from_millis(150));
    let result = transport.stop().expect("stop");
    let take = &result["recording"]["takes"][0];
    assert_eq!(
        (take["notes"].as_u64(), take["controllers"].as_u64()),
        (Some(1), Some(3))
    );
    let project = rt.app.session().project().expect("the project");
    let (_, clip) = project.clip(take["clip_id"].as_str().unwrap()).unwrap();
    let Clip::Midi(clip) = clip else {
        panic!("a MIDI clip");
    };
    // Compared as a set (TST-04): the clip orders controllers by beat and then by kind, and two
    // messages drained in the same period share a beat, so the send order is not the stored
    // order. The beat order is checked below.
    let mut controls: Vec<(ControlKind, i64, i64, i64)> = clip
        .controls
        .iter()
        .map(|c| (c.kind, c.value, c.number, c.channel))
        .collect();
    controls.sort();
    assert_eq!(
        controls,
        [
            (ControlKind::Cc, 99, 1, 1),
            (ControlKind::ChannelPressure, 42, 0, 2),
            (ControlKind::PitchBend, 4096, 0, 0),
        ]
    );
    let note_on = clip.notes[0].start;
    // `>=`, not `>` (TST-04): a message is timed by the device's software position, which
    // advances only per callback period, so a controller drained in the same period as the
    // note-on shares its frame on a slow runner (RTSan debug build).
    assert!(
        clip.controls
            .iter()
            .all(|c| c.beat >= note_on && c.beat < clip.length)
    );
    // Non-decreasing, not strictly increasing (TST-04), for the same reason: two messages
    // drained before the next period share a frame. Only the beat order is pinned.
    assert!(clip.controls.windows(2).all(|w| w[0].beat <= w[1].beat));
    // One undo step removes the clip with its controllers.
    let clip_id = clip.id.clone();
    rt.app.session().lock().undo("test").expect("undo");
    let project = rt.app.session().project().unwrap();
    assert!(project.find_clip(&clip_id).is_none());
    assert_eq!(project.track(&synth).unwrap().clips.len(), 0);
}

/// The answers of each backend before anything plays: `null` has none and says why;
/// `native-null` names miniaudio's backend.
#[test]
fn backends_answer_like_the_prototype() {
    let dir = tempfile::tempdir().unwrap();
    let app = |backend: &str| {
        AppContext::create(AppOptions {
            workspaces: Some(dir.path().join("ws")),
            settings: Some(Settings::default()),
            persist_settings: false,
            audio_backend: Some(backend.to_owned()),
            ..AppOptions::default()
        })
        .unwrap()
    };
    let null = app("null");
    assert!(!null.has_transport());
    let transport = null.transport();
    assert!(null.has_transport());
    assert_eq!(
        transport.status(),
        json!({"available": false, "backend": "null", "reason": "audio backend disabled (null backend)"})
    );
    let unavailable = |error: sonara_app::error::AppError| match error {
        sonara_app::error::AppError::Domain(error) => (error.code, error.message),
        other => panic!("a domain error: {other}"),
    };
    assert_eq!(
        unavailable(transport.seek(1.0).unwrap_err()),
        (
            ErrorCode::EngineUnavailable,
            "audio backend disabled (null backend)".to_owned()
        )
    );
    assert_eq!(
        transport.stop().unwrap(),
        json!({"stopped": true, "was_playing": false})
    );
    assert_eq!(transport.pause(), json!({"paused": false}));
    assert_eq!(transport.disable_midi(), json!({"enabled": false}));
    assert_eq!(
        transport.midi_status(),
        json!({"enabled": false, "ports": []})
    );
    let native = app("native-null");
    let transport = native.transport();
    assert_eq!(
        transport.status(),
        json!({"available": true, "backend": "native/Null", "state": "stopped"})
    );
    // Without a project: `play` asks for one first.
    assert_eq!(
        unavailable(transport.play(None, None, false).unwrap_err()).0,
        ErrorCode::NoProject
    );
    assert_eq!(transport.meters(), json!({}));
    assert_eq!(transport.device_reports(), json!({}));
}

/// `close()` finishes a running recording as the prototype's `close` does: its take is added.
#[test]
fn shutdown_finishes_a_recording() {
    let rt = Rt::new(250.0, None);
    rt.app.settings().audio.input_channels = 1;
    let vocal = rt.track("Vocal", TrackKind::Audio, None);
    rt.arm(&vocal, Some(vec![0]));
    rt.app.transport().record(Some(0.0), None).expect("record");
    thread::sleep(Duration::from_millis(300));
    rt.app.close();
    let project = rt.app.session().project().expect("the project");
    let track = project.track(&vocal).expect("the track");
    assert_eq!(track.clips.len(), 1, "the take was added on close");
    let workspace = rt
        .app
        .session()
        .lock()
        .location()
        .unwrap()
        .workspace
        .clone();
    let Clip::Audio(clip) = &track.clips[0] else {
        panic!("an audio clip");
    };
    let asset = project.asset(&clip.asset_id).unwrap();
    assert!(Path::new(&workspace).join(&asset.path).is_file());
}

/// One call of the RT-06 service test, chosen by caller and call number: every public call of
/// the transport service. Returns whether it started playing or recording.
fn service_call(
    transport: &TransportService,
    keys: &str,
    synth: &str,
    caller: u32,
    call: u32,
) -> bool {
    match (call + caller) % 17 {
        0 => {
            return transport
                .play(Some(f64::from(call % 8)), None, false)
                .is_ok();
        }
        1 => return transport.play(Some(1.0), Some(3.0), true).is_ok(),
        2 => {
            let _ = transport.seek(f64::from(call % 5));
        }
        3 => {
            let _ = transport.pause();
        }
        4 => {
            let _ = transport.stop();
        }
        5 => {
            let _ = (transport.status(), transport.position_beats());
        }
        6 => return transport.record(Some(f64::from(call % 4)), None).is_ok(),
        7 => {
            let _ = transport.enable_midi(Some(vec![keys.to_owned()]));
        }
        8 => {
            let _ = transport.disable_midi();
        }
        9 => transport.set_midi_focus(call.is_multiple_of(2).then(|| synth.to_owned())),
        10 => {
            let _ = transport.midi_status();
        }
        11 => {
            let _ = (transport.meters(), transport.device_reports());
        }
        12 => {
            let _ = (transport.output_peaks(), transport.graph_version());
        }
        13 => {
            let _ = (transport.available(), transport.devices());
        }
        14 => {
            let _ = transport.midi_devices();
        }
        15 => {
            let event = InstrumentEvent {
                offset: 0,
                kind: if call.is_multiple_of(2) {
                    EventKind::NoteOn
                } else {
                    EventKind::NoteOff
                },
                pitch: 60,
                velocity: 100,
                channel: 0,
                value: 0,
                number: 0,
                articulation: None,
            };
            let _ = transport.audition_note(synth, event);
        }
        _ => {
            let _ = transport.status();
        }
    }
    false
}

/// RT-06 at the service level: every public call of the transport service from four threads
/// while the null device plays, a keyboard plays live input, and a fifth thread edits the
/// session, so the maintenance thread rebuilds the graph and the scan thread rescans the
/// keyboard meanwhile. The service takes its lock before the session's, never after, and drops
/// the player's threads outside both: no call deadlocks against the player's threads, the scan
/// thread or the edits, every call returns, and the transport still stops and plays
/// afterwards. That no control call waits for the render thread is the player's own test
/// (`control_calls_never_wait_for_the_render_thread` in sonara-engine), which runs it without a
/// render thread.
#[test]
fn service_calls_run_concurrently_with_playback_and_edits() {
    let (finished, watchdog) = mpsc::channel();
    thread::spawn(move || {
        service_calls_concurrently();
        finished.send(()).expect("the test waits");
    });
    watchdog
        .recv_timeout(Duration::from_secs(120))
        .expect("the concurrent service calls finish (no deadlock)");
}

fn service_calls_concurrently() {
    let keyboard = Keyboard::new("Test Keys");
    let (rt, synth) = live_rt(&keyboard);
    rt.app.settings().audio.input_channels = 1;
    let drums = rt.drums();
    let vocal = rt.track("Vocal", TrackKind::Audio, None);
    rt.arm(&vocal, Some(vec![0]));
    let transport = rt.app.transport();
    transport.play(Some(0.0), None, false).expect("play");
    let done = Arc::new(AtomicBool::new(false));

    let callers: Vec<_> = (0..4_u32)
        .map(|caller| {
            let transport = Arc::clone(&transport);
            let keys = keyboard.name.clone();
            let synth = synth.clone();
            thread::spawn(move || {
                let mut started = 0_u32;
                for call in 0..40_u32 {
                    started += u32::from(service_call(&transport, &keys, &synth, caller, call));
                    thread::sleep(Duration::from_millis(1));
                }
                started
            })
        })
        .collect();
    let editor = {
        let session = rt.app.session().clone();
        let done = Arc::clone(&done);
        thread::spawn(move || {
            let mut edits = 0_u32;
            while !done.load(Ordering::Relaxed) {
                let volume = if edits.is_multiple_of(2) { -6.0 } else { 0.0 };
                session
                    .lock()
                    .execute(
                        &SetTrackMixer {
                            track_id: drums.clone(),
                            volume_db: Some(volume),
                            ..SetTrackMixer::default()
                        },
                        false,
                        "test",
                    )
                    .expect("the edit");
                edits += 1;
                thread::sleep(Duration::from_millis(5));
            }
            edits
        })
    };
    let player = {
        let keyboard = Arc::clone(&keyboard);
        let done = Arc::clone(&done);
        thread::spawn(move || {
            let mut note = 0_u8;
            while !done.load(Ordering::Relaxed) {
                // A ring opened by `enable_midi` exists once the first caller has run it.
                if !keyboard
                    .senders
                    .lock()
                    .expect("an unpoisoned lock")
                    .is_empty()
                {
                    keyboard.send(&[0x90, 60 + note % 12, 90]);
                    keyboard.send(&[0x80, 60 + note % 12, 0]);
                    note = note.wrapping_add(1);
                }
                thread::sleep(Duration::from_millis(20));
            }
        })
    };

    let started: u32 = callers
        .into_iter()
        .map(|caller| caller.join().expect("a caller finishes"))
        .sum();
    // The plays and recordings did start; the calls did not only take their error paths.
    assert!(started > 10, "{started} plays and recordings started");
    done.store(true, Ordering::Relaxed);
    assert!(editor.join().expect("the editor finishes") > 0);
    player.join().expect("the keyboard finishes");

    // The transport is consistent afterwards: it stops, and a new play runs on the device.
    transport.stop().expect("stop");
    assert_eq!(rt.status()["state"], "stopped");
    assert_eq!(rt.status()["device_errors"], 0);
    transport.play(Some(0.0), None, false).expect("play again");
    let start = rt.status()["position_beat"].as_f64();
    // A liveness check, not a timing one: after the stress phase the maintenance thread may
    // still be working off rebuilds, and under RTSan in the container the first blocks took
    // over 3 s once in seven gate runs (2026-10-22). Ten seconds keeps a real hang failing.
    assert!(
        wait_until(Duration::from_secs(10), || {
            rt.status()["position_beat"].as_f64() > start
        }),
        "playback advances after the stress phase: {}",
        rt.status()
    );
    transport.stop().expect("stop");
    drop(transport);
    rt.app.close();
}

/// [M] Row 12, `test_live_midi_plays_while_stopped_and_records_into_a_clip` through the
/// transport service on the system's MIDI (CoreMIDI): a virtual source plays an armed synth on
/// a stopped transport, then two notes into a recording. This needs real CoreMIDI or ALSA
/// virtual ports, so the test is ignored and runs on the host (HW-01):
///
/// ```text
/// cargo test -p sonara-app --test transport live_midi_plays -- --ignored
/// ```
///
/// The hub never opens ports named "Sonara…", so the source is "Test Keys Loop", where the
/// prototype attached its "Sonara Test Keys" port to the player directly.
#[test]
#[ignore = "hardware: CoreMIDI or ALSA virtual ports (HW-01)"]
fn live_midi_plays_while_stopped_and_records_into_a_clip() {
    use sonara_io::midi::{MidiOutputPort, input_ports};

    let name = "Test Keys Loop";
    let mut out = MidiOutputPort::open_virtual(name).expect("a virtual MIDI source");
    assert!(wait_until(Duration::from_secs(2), || {
        input_ports().is_ok_and(|ports| ports.iter().any(|p| p == name))
    }));
    let rt = Rt::new(150.0, None);
    let synth = rt.track(
        "Keys",
        TrackKind::Instrument,
        Some(("sonara.synth.poly", Some("Pluck"))),
    );
    rt.arm(&synth, None);
    let transport = rt.app.transport();
    let opened = transport
        .enable_midi(Some(vec![name.to_owned()]))
        .expect("MIDI input");
    assert_eq!(opened["ports"], json!([name]));
    thread::sleep(Duration::from_millis(50));
    // Stopped transport: live input must still reach the instrument.
    out.send(&[0x90, 60, 110]).expect("sent");
    assert!(wait_until(Duration::from_secs(2), || {
        transport.midi_status()["messages_received"]
            .as_u64()
            .is_some_and(|n| n >= 1)
    }));
    assert_eq!(transport.midi_status()["targets"], json!([synth]));
    out.send(&[0x80, 60, 0]).expect("sent");
    // Record two notes from beat 0.
    transport.record(Some(0.0), None).expect("record");
    thread::sleep(Duration::from_millis(300));
    out.send(&[0x90, 62, 100]).expect("sent");
    thread::sleep(Duration::from_millis(250));
    out.send(&[0x80, 62, 0]).expect("sent");
    out.send(&[0x90, 65, 80]).expect("sent");
    thread::sleep(Duration::from_millis(200));
    out.send(&[0x80, 65, 0]).expect("sent");
    thread::sleep(Duration::from_millis(200));
    let result = transport.stop().expect("stop");
    drop(out);
    let takes = result["recording"]["takes"].as_array().expect("the takes");
    assert_eq!(takes.len(), 1);
    assert_eq!(takes[0]["notes"], 2);
    let project = rt.app.session().project().expect("the project");
    let (_, clip) = project
        .clip(takes[0]["clip_id"].as_str().expect("a clip ID"))
        .expect("the clip");
    let Clip::Midi(clip) = clip else {
        panic!("a MIDI clip");
    };
    let pitches: Vec<u8> = clip.notes.iter().map(|n| n.pitch).collect();
    assert_eq!(pitches, [62, 65]);
    let first = &clip.notes[0];
    // About 0.3 s after the start at 120 BPM, about 0.6 beats.
    assert!(
        0.2 < first.start && first.start < 1.2,
        "start {}",
        first.start
    );
    assert!(
        0.2 < first.length && first.length < 1.0,
        "length {}",
        first.length
    );
    assert_eq!(clip.notes[1].velocity, 80);
}
