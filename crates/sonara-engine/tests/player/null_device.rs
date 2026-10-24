//! The prototype's player tests on miniaudio's null device (`tests/engine/test_realtime.py`),
//! with the player's threads running and the prototype's test
//! lookahead of 250 ms ("headroom: CI runs suites in parallel on shared CPUs"); and the live-MIDI
//! test's stopped half (`test_midi_live.py`, 150 ms) with a MIDI ring the test feeds, the
//! container twin of CoreMIDI test.
//!
//! The null device is a simulated real-time device on a timer thread, so these tests wait for
//! it, with the prototype's timings and tolerances. Step 10 runs the same tests through the
//! transport service.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;
use sonara_core::model::JsonMap;
use sonara_core::plugins::builtin_registry;
use sonara_engine::player::{
    AudioDevice, DeviceError, DeviceStream, Player, PlayerSettings, PlayerSetup, PlayerState,
    ProjectSnapshot, StreamRequest,
};
use sonara_io::{ContextKind, DeviceConfig, Engine};
use sonara_project::serializer::{project_from_value, project_to_value};
use sonara_rt::{MidiEvent, StreamConfig, midi_channel};
use sonara_testkit::Golden;

use crate::support::{golden_project, no_media, tracks};

/// `sonara-io`'s engine on the null backend: the prototype's `native-null`. The
/// capture-drain measurement can put it on the system's devices instead (`with_context`).
struct NullDevice {
    engine: Engine,
}

impl NullDevice {
    fn new() -> Self {
        Self::with_context(ContextKind::Null)
    }

    fn with_context(context: ContextKind) -> Self {
        let mut engine = Engine::new();
        engine
            .init_context(context)
            .expect("the audio context (the null backend is always available)");
        Self { engine }
    }
}

impl AudioDevice for NullDevice {
    fn start(&mut self, request: &StreamRequest) -> Result<DeviceStream, DeviceError> {
        let handles = self
            .engine
            .start(&DeviceConfig {
                sample_rate: request.sample_rate,
                buffer_frames: request.buffer_frames,
                stream: StreamConfig {
                    output_channels: request.output_channels,
                    input_channels: request.input_channels,
                    block_frames: 512,
                    ring_blocks: request.ring_blocks,
                    capture_frames: request.capture_frames,
                },
                output_device: request.output_device.clone(),
                input_device: request.input_device.clone(),
            })
            .map_err(|e| DeviceError(e.to_string()))?;
        Ok(DeviceStream {
            sample_rate: handles.format.sample_rate,
            producer: handles.producer,
            capture: handles.capture,
            control: handles.control,
        })
    }

    fn stop(&mut self) {
        self.engine.stop();
    }
}

/// A player on the null device with its threads, and the session it follows.
struct App {
    player: Player,
    session: Arc<Mutex<ProjectSnapshot>>,
}

impl App {
    fn new(edit: impl FnOnce(&mut JsonMap), lookahead_ms: f64) -> Self {
        Self::on(NullDevice::new(), edit, lookahead_ms)
    }

    fn on(device: NullDevice, edit: impl FnOnce(&mut JsonMap), lookahead_ms: f64) -> Self {
        let golden = Golden::open().expect("golden data");
        let project = golden_project(&golden, "transport/play-position/project.json", edit);
        let session = Arc::new(Mutex::new(ProjectSnapshot {
            project,
            version: 1,
            media: no_media(),
        }));
        let source = {
            let session = Arc::clone(&session);
            move || Some(session.lock().expect("an unpoisoned lock").clone())
        };
        let player = Player::new(PlayerSetup {
            registry: Arc::new(builtin_registry()),
            device: Box::new(device),
            source: Arc::new(source),
            settings: PlayerSettings {
                lookahead_ms,
                ..PlayerSettings::default()
            },
        })
        .expect("the player's threads");
        Self { player, session }
    }

    fn snapshot(&self) -> ProjectSnapshot {
        self.session.lock().expect("an unpoisoned lock").clone()
    }

    fn beats(&self) -> f64 {
        self.player.position_beats(Some(&self.snapshot().project))
    }
}

/// The prototype's `_drum_project`: the drum track alone, at 120 BPM.
fn drums_only(data: &mut JsonMap) {
    tracks(data).retain(|t| t["name"] == "Drums");
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

/// Row 4: plays from 0, the position advances with the device, nothing underruns.
#[test]
fn player_plays_without_underruns_and_tracks_position() {
    let app = App::new(drums_only, 250.0);
    let snapshot = app.snapshot();
    let status = app
        .player
        .play(&snapshot, Some(0.0), None, false)
        .expect("play");
    assert_eq!(status.state, PlayerState::Playing);
    thread::sleep(Duration::from_millis(600));
    let status = app.player.status(Some(&snapshot.project));
    assert_eq!(status.underruns, 0);
    // 0.6 s at 120 BPM is 1.2 beats; allow for lookahead and scheduling jitter.
    let beat = app.beats();
    assert!(0.6 < beat && beat < 1.4, "position {beat}");
    assert!(
        status.render_load < 1.0,
        "render load {}",
        status.render_load
    );
    let stopped = app.player.stop(Some(&snapshot.project));
    assert!(stopped.was_playing);
    assert_eq!(app.player.status(None).state, PlayerState::Stopped);
}

/// Row 5: a seek while playing moves the position; a two-beat loop keeps it inside the loop
/// over more than one pass, without underruns.
#[test]
fn player_seek_and_loop() {
    let app = App::new(drums_only, 250.0);
    let snapshot = app.snapshot();
    app.player
        .play(&snapshot, Some(0.0), None, false)
        .expect("play");
    let _ = app.player.seek(&snapshot.project, 8.0);
    thread::sleep(Duration::from_millis(200));
    assert!(app.beats() >= 8.0, "position {}", app.beats());
    let _ = app.player.stop(Some(&snapshot.project));
    app.player
        .play(&snapshot, Some(0.0), Some(2.0), true)
        .expect("play a loop");
    thread::sleep(Duration::from_millis(1500));
    let beat = app.beats();
    assert!((0.0..2.0).contains(&beat), "position {beat}");
    assert_eq!(app.player.status(None).underruns, 0);
    let _ = app.player.stop(Some(&snapshot.project));
}

/// Row 6: a play range stops by itself at its end.
#[test]
fn play_range_stops_at_end() {
    let app = App::new(drums_only, 250.0);
    let snapshot = app.snapshot();
    app.player
        .play(&snapshot, Some(0.0), Some(1.0), false)
        .expect("play");
    assert!(wait_until(Duration::from_secs(3), || app.player.state()
        == PlayerState::Stopped));
    let beat = app.beats();
    assert!((beat - 1.0).abs() <= 0.05, "position {beat}");
    assert_eq!(
        app.player.status(None).underruns,
        0,
        "D11: none at the range end"
    );
}

/// Row 7: an edit during playback is picked up (the graph of the new version plays) without
/// underruns.
#[test]
fn edit_during_playback_is_picked_up() {
    let app = App::new(drums_only, 250.0);
    let snapshot = app.snapshot();
    app.player
        .play(&snapshot, Some(0.0), None, false)
        .expect("play");
    thread::sleep(Duration::from_millis(200));
    {
        let mut session = app.session.lock().expect("an unpoisoned lock");
        let mut value = project_to_value(&session.project);
        let data = value.as_object_mut().expect("a project");
        tracks(data)[0]["mixer"]["volume_db"] = json!(-12.0);
        session.project = project_from_value(data).expect("the edited project");
        session.version += 1;
    }
    thread::sleep(Duration::from_millis(300));
    assert_eq!(app.player.graph_version(), Some(2));
    assert_eq!(app.player.status(None).underruns, 0);
    let _ = app.player.stop(None);
}

/// `test_live_midi_plays_while_stopped_and_records_into_a_clip`, its first half (the take is
/// tested separately): while stopped, a note reaches the armed synth and sounds; the message counts
/// because the track is a target.
#[test]
fn live_midi_plays_while_stopped() {
    let app = App::new(
        |data| {
            tracks(data).retain(|t| t["name"] == "Keys");
            tracks(data)[0]["input"]["armed"] = json!(true);
            tracks(data)[0]["clips"] = json!([]);
            tracks(data)[0]["inserts"] = json!([]);
        },
        150.0,
    );
    let snapshot = app.snapshot();
    let keys = snapshot.project.tracks[0].id.clone();
    let (mut keyboard, input) = midi_channel(64);
    app.player
        .add_midi_inputs(vec![(0, input)], Some(&snapshot))
        .expect("live input");
    assert_eq!(app.player.midi_targets(&snapshot.project), [keys]);
    assert!(keyboard.push(MidiEvent::new(1, 0, &[0x90, 62, 100]).expect("a note on")));
    assert!(wait_until(Duration::from_secs(2), || app
        .player
        .messages_received()
        >= 1));
    // The device plays the note.
    assert!(wait_until(Duration::from_secs(2), || {
        app.player.output_peaks().iter().any(|peak| *peak > 0.01)
    }));
    assert!(keyboard.push(MidiEvent::new(2, 0, &[0x80, 62, 0]).expect("a note off")));
    assert!(wait_until(Duration::from_secs(2), || app
        .player
        .messages_received()
        >= 2));
    assert_eq!(app.player.status(None).state, PlayerState::Stopped);
}

/// Measured locally (HW-01): how long the take writer leaves the capture ring undrained
/// while it records two input channels at 48 kHz for 20 s, with another thread writing and
/// syncing 32 MB files to the same disk and four threads spinning on the CPU. The capture ring
/// holds `CAPTURE_SECONDS`; the test fails if it overran.
///
/// ```text
/// cargo test --release -p sonara-engine --test player take_writer_stall -- --ignored --nocapture
/// ```
///
/// With `SONARA_CAPTURE_CONTEXT=default` it records from the system's default input device
/// instead of the null device (the terminal needs the microphone permission). The master is at
/// −144 dB, so the drums are inaudible, but a recording forces the metronome on and its clicks
/// are added after the master, as in the prototype: they sound on the default output.
#[test]
#[ignore = "measurement: 20 s of recording under disk and CPU load (HW-01)"]
fn take_writer_stall() {
    use std::io::Write as _;
    use std::sync::atomic::{AtomicBool, Ordering};

    use sonara_engine::player::{CAPTURE_SECONDS, RecordPlan, TakeTarget};
    use sonara_media::export::SampleFormat;

    let system = std::env::var("SONARA_CAPTURE_CONTEXT").is_ok_and(|context| context == "default");
    let app = if system {
        let inaudible = |data: &mut JsonMap| {
            drums_only(data);
            data["master"]["mixer"]["volume_db"] = json!(-144.0);
        };
        App::on(
            NullDevice::with_context(ContextKind::Default),
            inaudible,
            80.0,
        )
    } else {
        App::new(drums_only, 80.0)
    };
    let snapshot = app.snapshot();
    let dir = tempfile::tempdir().expect("a directory");
    app.player
        .open_stream(&snapshot.project, Some(2))
        .expect("a duplex stream");
    let busy = Arc::new(AtomicBool::new(true));
    let load: Vec<_> = (0..5)
        .map(|n| {
            let busy = Arc::clone(&busy);
            let path = dir.path().join(format!("load-{n}.bin"));
            thread::spawn(move || {
                let chunk = vec![0x5A_u8; 32 << 20];
                let mut spins = 0_u64;
                while busy.load(Ordering::Relaxed) {
                    if n == 0 {
                        let mut file = std::fs::File::create(&path).expect("a load file");
                        file.write_all(&chunk).expect("written");
                        file.sync_all().expect("synced");
                    } else {
                        spins = spins.wrapping_add(1);
                    }
                }
                drop(std::fs::remove_file(&path));
                spins
            })
        })
        .collect();
    let take = dir.path().join("take.wav");
    app.player
        .record(
            &snapshot,
            RecordPlan {
                start_beat: Some(0.0),
                takes: vec![TakeTarget {
                    track_id: snapshot.project.tracks[0].id.clone(),
                    path: take.clone(),
                    channels: vec![0, 1],
                    format: SampleFormat::Pcm24,
                }],
                midi_tracks: Vec::new(),
            },
        )
        .expect("record");
    thread::sleep(Duration::from_secs(20));
    let done = app
        .player
        .stop(Some(&snapshot.project))
        .recording
        .expect("the take");
    busy.store(false, Ordering::Relaxed);
    for thread in load {
        drop(thread.join());
    }
    let stats = done.writer;
    println!(
        "capture: {} drains, longest gap {:.2} ms, longest drain {:.2} ms, {} frames, {} overruns; \
         the ring holds {CAPTURE_SECONDS} s",
        stats.drains,
        stats.longest_gap.as_secs_f64() * 1e3,
        stats.longest_drain.as_secs_f64() * 1e3,
        stats.frames,
        done.capture_overruns
    );
    assert_eq!(done.capture_overruns, 0);
    assert!(stats.frames >= 19 * 48_000, "{} frames", stats.frames);
    assert!(std::fs::metadata(&take).expect("the take").len() > 0);
}
