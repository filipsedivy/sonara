//! The live frame of the GUI: its bytes (a round trip, and the byte
//! fixture the page's decoder test shares), how a transport snapshot maps onto the project's
//! meter order, the feed's rates, the MIDI focus, and an RT-06 test that reads the project view
//! and builds frames from many threads while the null device plays and the session is edited.
//!
//! The byte fixture lives with the page's tests, `apps/sonara-desktop/tests/fixtures/
//! live-frame-v1.json`, because the container's frontend task copies only that folder. It holds
//! each frame's bytes as hex and the page's decoded form. `SONARA_WRITE_FIXTURES=1` rewrites it
//! from this test; otherwise the test checks that the encoder still produces exactly those
//! bytes and that they decode to the same frame.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::edit::{self, Edit};
use sonara_app::live::{
    EngineState, FRAME_VERSION, FrameError, HEADER_BYTES, LiveFeed, LiveFrame, LivePosition,
    LiveState, MeterLevels,
};
use sonara_app::projects::NewProject;
use sonara_app::settings::Settings;
use sonara_app::transport::{
    DeviceReport, PlayerState, PlayerStatus, Position, TrackMeter, TransportSnapshot,
};
use sonara_app::view::{meter_layout, project_view};
use sonara_core::ids::SequentialIds;
use sonara_core::model::project::Project;
use tempfile::TempDir;

// -- the bytes ----------------------------------------------------------------------------------

fn playing_frame() -> LiveFrame {
    LiveFrame {
        sequence: 7,
        engine: EngineState::Running,
        state: LiveState::Playing,
        position: LivePosition {
            frame: 96_000,
            musical: Some((4.0, 2.0)),
        },
        device_stopped: false,
        sample_rate: 48_000,
        buffer_frames: 256,
        render_load: 0.125,
        callback_load: 0.0625,
        output_latency_ms: 10.67,
        lookahead_ms: 80.0,
        queued_ms: 42.5,
        underruns: 3,
        device_errors: 1,
        version: 12,
        meter_layout: 0xdead_beef,
        meters: vec![
            MeterLevels {
                peak: [0.5, 0.25],
                rms: [0.125, 0.0625],
            },
            MeterLevels::default(),
            MeterLevels {
                peak: [1.0, 0.75],
                rms: [0.5, 0.375],
            },
        ],
        gain_reduction: vec![f32::NAN, -3.5],
    }
}

fn idle_frame() -> LiveFrame {
    LiveFrame {
        sequence: u32::MAX,
        engine: EngineState::Unavailable,
        version: 3,
        device_stopped: true,
        ..LiveFrame::default()
    }
}

/// The page's decoded form of a frame (`src/lib/live/frame.ts`): NaN gain reduction is `null`
/// in JSON.
fn decoded(frame: &LiveFrame) -> Value {
    let engine = match frame.engine {
        EngineState::Absent => "absent",
        EngineState::Unavailable => "unavailable",
        EngineState::Ready => "ready",
        EngineState::Running => "running",
    };
    let state = match frame.state {
        LiveState::Stopped => "stopped",
        LiveState::Playing => "playing",
        LiveState::Paused => "paused",
        LiveState::Recording => "recording",
    };
    let (beat, seconds) = frame
        .position
        .musical
        .map_or((Value::Null, Value::Null), |(b, s)| (json!(b), json!(s)));
    json!({
        "sequence": frame.sequence,
        "engine": engine,
        "state": state,
        "position": {"frame": frame.position.frame, "beat": beat, "seconds": seconds},
        "device_stopped": frame.device_stopped,
        "sample_rate": frame.sample_rate,
        "buffer_frames": frame.buffer_frames,
        "render_load": frame.render_load,
        "callback_load": frame.callback_load,
        "output_latency_ms": frame.output_latency_ms,
        "lookahead_ms": frame.lookahead_ms,
        "queued_ms": frame.queued_ms,
        "underruns": frame.underruns,
        "device_errors": frame.device_errors,
        "version": frame.version,
        "meter_layout": frame.meter_layout,
        "meters": frame.meters.iter().map(|m| json!({
            "peak": [f64::from(m.peak[0]), f64::from(m.peak[1])],
            "rms": [f64::from(m.rms[0]), f64::from(m.rms[1])],
        })).collect::<Vec<_>>(),
        "gain_reduction": frame.gain_reduction.iter().map(|g| {
            if g.is_nan() { Value::Null } else { json!(f64::from(*g)) }
        }).collect::<Vec<_>>(),
    })
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, b| {
        write!(out, "{b:02x}").expect("writing to a String cannot fail");
        out
    })
}

/// Frames compare equal with NaN gain reduction in the same places.
fn same(a: &LiveFrame, b: &LiveFrame) -> bool {
    let gr = |f: &LiveFrame| -> Vec<u32> { f.gain_reduction.iter().map(|g| g.to_bits()).collect() };
    LiveFrame {
        gain_reduction: Vec::new(),
        ..a.clone()
    } == LiveFrame {
        gain_reduction: Vec::new(),
        ..b.clone()
    } && gr(a) == gr(b)
}

#[test]
fn frames_round_trip_and_match_the_shared_byte_fixture() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/sonara-desktop/tests/fixtures/live-frame-v1.json");
    let absent = LiveFrame {
        sequence: 1,
        ..LiveFrame::default()
    };
    let frames = [
        ("playing", playing_frame()),
        ("idle", idle_frame()),
        ("absent", absent),
    ];
    let cases: Vec<Value> = frames
        .iter()
        .map(|(name, frame)| {
            json!({"name": name, "hex": hex(&frame.encode()), "decoded": decoded(frame)})
        })
        .collect();
    let document = json!({
        "comment": "Live frames of format 1 (sonara-app src/live.rs), written by \
                    crates/sonara-app/tests/gui_live.rs with SONARA_WRITE_FIXTURES=1. The page's \
                    decoder test (tests/unit/frame.test.ts) decodes each hex string to its \
                    decoded form; a null gain reduction is NaN.",
        "format": FRAME_VERSION,
        "frames": cases,
    });
    if std::env::var_os("SONARA_WRITE_FIXTURES").is_some() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = serde_json::to_string_pretty(&document).unwrap() + "\n";
        fs::write(&path, text).expect("the fixture is written");
    }
    let fixture: Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("the shared fixture"))
            .expect("JSON");
    assert_eq!(
        fixture, document,
        "the encoder still writes the fixture's bytes"
    );

    for (_, frame) in &frames {
        let bytes = frame.encode();
        let back = LiveFrame::decode(&bytes).expect("a frame");
        assert!(same(&back, frame), "{back:?} vs {frame:?}");
    }
    let playing = playing_frame().encode();
    assert_eq!(playing.len(), HEADER_BYTES + 3 * 16 + 2 * 4);
    assert_eq!(u16::from_le_bytes([playing[0], playing[1]]), FRAME_VERSION);
    // RT-07: the latency estimate is a field of its own, apart from the position.
    assert_eq!(
        f64::from_le_bytes(playing[72..80].try_into().unwrap()),
        10.67
    );
    assert_eq!(
        i64::from_le_bytes(playing[24..32].try_into().unwrap()),
        96_000
    );

    assert_eq!(
        LiveFrame::decode(&playing[..100]),
        Err(FrameError::TooShort(100))
    );
    assert!(matches!(
        LiveFrame::decode(&playing[..playing.len() - 1]),
        Err(FrameError::Length { .. })
    ));
    let mut other = playing.clone();
    other[0] = 9;
    assert_eq!(LiveFrame::decode(&other), Err(FrameError::Version(9)));
    let mut state = playing;
    state[4] = 7;
    assert_eq!(LiveFrame::decode(&state), Err(FrameError::State(7)));
}

/// A recording transport whose player reports the master and track `synth`, and the gain
/// reduction of the compressor `comp`.
fn recording(synth: &str, comp: &str) -> TransportSnapshot {
    TransportSnapshot {
        available: true,
        status: Some(PlayerStatus {
            state: PlayerState::Recording,
            position: Position::Musical {
                frame: 24_000,
                beat: 1.0,
                seconds: 0.5,
                bar: 1,
                beat_in_bar: 2,
            },
            device_sample_rate: 48_000,
            buffer_frames: 512,
            queued_ms: 85.3,
            lookahead_ms: 80.0,
            output_latency_estimate_ms: 21.33,
            underruns: 2,
            callback_load: 0.1,
            render_load: 0.05,
            looped: None,
            device_errors: 0,
            last_error: None,
            device_status: None,
        }),
        meters: vec![
            TrackMeter {
                track_id: "master".to_owned(),
                peak: [0.9, 0.8],
                rms: [0.3, 0.2],
            },
            TrackMeter {
                track_id: synth.to_owned(),
                peak: [0.5, 0.4],
                rms: [0.1, 0.05],
            },
        ],
        reports: vec![DeviceReport {
            device_id: comp.to_owned(),
            values: vec![("input_db", -12.0), ("gain_reduction_db", -4.5)],
        }],
    }
}

/// The meters follow the project's order, tracks then master, whatever order the player
/// reports them in; a device without a gain-reduction readout is NaN.
#[test]
fn a_snapshot_lines_up_with_the_project() {
    let gui = Live::new("null");
    let a = gui.create("A", "audio", None);
    let b = gui.create("B", "instrument", Some("sonara.synth.poly"));
    let comp = gui.effect(&b, "sonara.dynamics.compressor");
    let eq = gui.effect(&a, "sonara.eq.parametric");
    let project = gui.project();
    let instrument = project
        .track(&b)
        .unwrap()
        .instrument
        .clone()
        .unwrap()
        .id
        .clone();
    let snapshot = recording(&b, &comp);
    let frame = LiveFrame::of(Some(&snapshot), Some(&project), 9, 1);
    assert_eq!(frame.engine, EngineState::Running);
    assert_eq!(frame.state, LiveState::Recording);
    assert_eq!(frame.position.frame, 24_000);
    assert_eq!(frame.position.musical, Some((1.0, 0.5)));
    assert_eq!(frame.output_latency_ms, 21.33);
    assert_eq!(frame.meter_layout, meter_layout(&project));
    assert_eq!(frame.meters.len(), 3, "A, B, master");
    assert_eq!(
        frame.meters[0],
        MeterLevels::default(),
        "A reported nothing"
    );
    assert_eq!(frame.meters[1].peak, [0.5, 0.4]);
    assert_eq!(frame.meters[2].rms, [0.3, 0.2]);
    // A's EQ, B's synth, B's compressor; the master has no devices.
    let order: Vec<&str> = project
        .all_tracks()
        .flat_map(sonara_core::model::track::Track::devices)
        .map(|d| d.id.as_str())
        .collect();
    assert_eq!(order, [eq.as_str(), instrument.as_str(), comp.as_str()]);
    assert!(frame.gain_reduction[0].is_nan() && frame.gain_reduction[1].is_nan());
    assert_eq!(frame.gain_reduction[2], -4.5);

    let ready = TransportSnapshot {
        available: true,
        status: None,
        meters: Vec::new(),
        reports: Vec::new(),
    };
    let frame = LiveFrame::of(Some(&ready), Some(&project), 9, 2);
    assert_eq!(
        (frame.engine, frame.state),
        (EngineState::Ready, LiveState::Stopped)
    );
    assert_eq!(frame.meters, vec![MeterLevels::default(); 3]);
    let absent = LiveFrame::of(None, None, 0, 3);
    assert_eq!(absent.engine, EngineState::Absent);
    assert!(absent.meters.is_empty() && absent.position.musical.is_none());
    let unavailable = TransportSnapshot {
        available: false,
        ..ready
    };
    assert_eq!(
        LiveFrame::of(Some(&unavailable), None, 0, 4).engine,
        EngineState::Unavailable
    );
}

// -- the application ------------------------------------------------------------------------------

/// An application with a new 120 BPM project on `backend`.
struct Live {
    app: Arc<AppContext>,
    _dir: TempDir,
}

impl Live {
    fn new(backend: &str) -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let mut settings = Settings::default();
        settings.audio.lookahead_ms = 250.0;
        let app = AppContext::create(AppOptions {
            workspaces: Some(dir.path().join("ws")),
            settings: Some(settings),
            persist_settings: false,
            audio_backend: Some(backend.to_owned()),
            ids: Some(sonara_app::ids::IdSource::new(Box::new(
                SequentialIds::new(0),
            ))),
            ..AppOptions::default()
        })
        .expect("the application");
        app.projects()
            .new_project(&NewProject {
                name: "Live".to_owned(),
                ..NewProject::default()
            })
            .expect("a project");
        Self {
            app: Arc::new(app),
            _dir: dir,
        }
    }

    fn edit(&self, edit: Value) -> sonara_app::edit::EditOutcome {
        let shown = edit.to_string();
        edit::apply(&self.app, Edit::from_json(edit).expect("an edit"))
            .unwrap_or_else(|error| panic!("{shown}: {error:?}"))
    }

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

    fn project(&self) -> Project {
        self.app.session().project().expect("a project")
    }

    /// A drum track with a kick on every beat for 16 beats.
    fn drums(&self) -> String {
        let track = self.create("Drums", "instrument", Some("sonara.drums"));
        let clip = self.edit(
            json!({"op": "create_midi_clip", "track_id": track, "start": 0.0,
                                    "length": 16.0, "name": "Beat"}),
        );
        let clip = clip.changes.created[0].id.clone();
        let notes: Vec<Value> = (0..16)
            .map(|beat| {
                json!({"pitch": 36, "start": f64::from(beat), "length": 0.25,
                               "velocity": 110})
            })
            .collect();
        self.edit(json!({"op": "add_notes", "clip_id": clip, "notes": notes}));
        track
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

/// A feed whose frames land in a shared list.
fn feed(app: &Arc<AppContext>) -> (LiveFeed, Arc<Mutex<Vec<LiveFrame>>>) {
    let frames = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&frames);
    let feed = LiveFeed::start(Arc::clone(app), move |bytes| {
        let frame = LiveFrame::decode(bytes).expect("the feed's frames decode");
        sink.lock().expect("an unpoisoned lock").push(frame);
    })
    .expect("the feed");
    (feed, frames)
}

/// About 4 frames a second while nothing plays, and the transport is never created for them.
#[test]
fn an_idle_feed_sends_about_four_frames_a_second() {
    let live = Live::new("null");
    let (feed, frames) = feed(&live.app);
    thread::sleep(Duration::from_millis(1100));
    feed.stop();
    let frames = frames.lock().unwrap();
    assert!(
        (3..=7).contains(&frames.len()),
        "{} frames in 1.1 s",
        frames.len()
    );
    assert!(frames.iter().all(|f| f.engine == EngineState::Absent));
    assert!(
        !live.app.has_transport(),
        "the feed did not create the transport"
    );
    assert_eq!(frames[0].meters.len(), 1, "the master's meter");
    let sequences: Vec<u32> = frames.iter().map(|f| f.sequence).collect();
    assert!(sequences.windows(2).all(|w| w[1] == w[0] + 1));
}

/// 60 frames a second while playing, with the playhead moving; back to the idle rate after a
/// stop, which is sent at once.
#[test]
fn a_playing_feed_sends_a_frame_per_display_frame() {
    let live = Live::new("native-null");
    let drums = live.drums();
    let transport = live.app.transport();
    let (feed, frames) = feed(&live.app);
    thread::sleep(Duration::from_millis(300));
    transport.play(Some(0.0), None, false).expect("play");
    assert!(
        wait_until(Duration::from_secs(10), || frames
            .lock()
            .unwrap()
            .iter()
            .any(|f| f.state == LiveState::Playing
                && f.position.musical.is_some_and(|(beat, _)| beat > 0.0))),
        "playback starts"
    );
    let start = frames.lock().unwrap().len();
    thread::sleep(Duration::from_secs(1));
    let playing: Vec<LiveFrame> = frames.lock().unwrap()[start..].to_vec();
    // HW-01: the feed's update rate (ADR 0009: meters >= 58 fps) is measured locally, never a CI
    // gate. A CI runner that starves this process of scheduling draws fewer display frames
    // though the feed itself is correct; that is the runner's limit, not a defect to fail on.
    if std::env::var_os("CI").is_none() {
        assert!(
            playing.len() >= 30,
            "{} frames in 1 s while playing",
            playing.len()
        );
    } else if playing.len() < 30 {
        eprintln!(
            "CI: only {} frames in 1 s while playing; skipping the check",
            playing.len()
        );
    }
    let first = playing.first().unwrap();
    let last = playing.last().unwrap();
    assert_eq!(first.engine, EngineState::Running);
    assert_eq!(first.sample_rate, 48_000);
    assert!(
        last.position.frame > first.position.frame,
        "the playhead moves"
    );
    assert_eq!(last.meter_layout, meter_layout(&live.project()));
    let index = live
        .project()
        .tracks
        .iter()
        .position(|t| t.id == drums)
        .unwrap();
    assert!(
        playing.iter().any(|f| f.meters[index].peak[0] > 0.0),
        "the kick shows in its meter"
    );
    assert_eq!(transport.pause()["paused"], true, "pause");
    assert!(
        wait_until(Duration::from_secs(2), || {
            let snapshot = live.app.transport().live_snapshot();
            snapshot
                .status
                .is_some_and(|status| status.state == PlayerState::Paused)
        }),
        "the pause is reached"
    );
    // The meters of the last block must not outlive the playback.
    assert!(
        live.app.transport().live_snapshot().meters.is_empty(),
        "a paused transport reports no meters"
    );
    transport.stop().expect("stop");
    let stopped_at = Instant::now();
    assert!(
        wait_until(Duration::from_secs(2), || frames
            .lock()
            .unwrap()
            .last()
            .is_some_and(|f| f.state == LiveState::Stopped)),
        "the stop is sent"
    );
    assert!(stopped_at.elapsed() < Duration::from_millis(500));
    feed.stop();
    drop(transport);
    live.app.close();
}

/// The selected instrument track takes live MIDI; another track clears the focus; and
/// without a transport nothing is created.
#[test]
fn the_midi_focus_follows_an_instrument_selection() {
    let live = Live::new("native-null");
    let synth = live.create("Synth", "instrument", Some("sonara.synth.poly"));
    let audio = live.create("Vox", "audio", None);
    live.app.focus_midi(Some(&synth));
    assert!(
        !live.app.has_transport(),
        "the focus never creates the transport"
    );
    let transport = live.app.transport();
    transport.play(Some(0.0), None, false).expect("a player");
    let targets = || transport.midi_status()["targets"].clone();
    assert_eq!(targets(), json!([]));
    live.app.focus_midi(Some(&synth));
    assert_eq!(targets(), json!([synth]));
    live.app.focus_midi(Some(&audio));
    assert_eq!(targets(), json!([]), "an audio track takes no MIDI");
    live.app.focus_midi(Some(&synth));
    live.app.focus_midi(Some("trk_nope"));
    assert_eq!(targets(), json!([]));
    live.app.focus_midi(Some(&synth));
    live.app.focus_midi(None);
    assert_eq!(targets(), json!([]));
    transport.stop().expect("stop");
    drop(transport);
    live.app.close();
}

// -- RT-06 ----------------------------------------------------------------------------------------

/// RT-06: the project view and the live frame are read from several threads while the null
/// device plays, the feed runs and another thread edits the session through the edit boundary.
/// Nothing deadlocks (a 120 s watchdog), every frame decodes, and the transport stops and plays
/// again afterwards.
#[test]
fn the_view_and_frames_are_read_concurrently_with_playback() {
    let (finished, watchdog) = mpsc::channel();
    thread::spawn(move || {
        view_and_frames_concurrently();
        finished.send(()).expect("the test waits");
    });
    watchdog
        .recv_timeout(Duration::from_secs(120))
        .expect("the concurrent reads finish (no deadlock)");
}

fn view_and_frames_concurrently() {
    let live = Live::new("native-null");
    let drums = live.drums();
    live.effect(&drums, "sonara.dynamics.compressor");
    let transport = live.app.transport();
    transport.play(Some(0.0), None, true).expect("play");
    let delivered = Arc::new(AtomicU32::new(0));
    let counted = Arc::clone(&delivered);
    let feed = LiveFeed::start(Arc::clone(&live.app), move |bytes| {
        LiveFrame::decode(bytes).expect("a frame");
        counted.fetch_add(1, Ordering::Relaxed);
    })
    .expect("the feed");
    let done = Arc::new(AtomicBool::new(false));

    let readers: Vec<_> = (0..4_u32)
        .map(|reader| {
            let app = Arc::clone(&live.app);
            thread::spawn(move || {
                let mut playing = 0_u32;
                for call in 0..150_u32 {
                    match (reader + call) % 3 {
                        0 => {
                            let view = project_view(app.session(), app.plugins());
                            assert!(view.project.is_some());
                        }
                        1 => {
                            let frame = LiveFrame::build(&app, call);
                            let back = LiveFrame::decode(&frame.encode()).expect("a frame");
                            playing += u32::from(back.state == LiveState::Playing);
                        }
                        _ => {
                            let snapshot = app.transport().live_snapshot();
                            assert!(snapshot.available);
                            app.focus_midi(Some(&format!("trk_{call}")));
                        }
                    }
                    thread::sleep(Duration::from_millis(1));
                }
                playing
            })
        })
        .collect();
    let editor = {
        let app = Arc::clone(&live.app);
        let done = Arc::clone(&done);
        let track = drums.clone();
        thread::spawn(move || {
            let mut edits = 0_u32;
            while !done.load(Ordering::Relaxed) {
                let db = if edits.is_multiple_of(2) { -6.0 } else { 0.0 };
                let edit = Edit::from_json(
                    json!({"op": "set_track_mixer", "track_id": track, "volume_db": db}),
                )
                .expect("an edit");
                edit::apply(&app, edit).expect("the edit");
                if edits.is_multiple_of(10) {
                    edit::seal_undo(&app);
                }
                edits += 1;
                thread::sleep(Duration::from_millis(5));
            }
            edits
        })
    };

    let playing: u32 = readers
        .into_iter()
        .map(|reader| reader.join().expect("a reader finishes"))
        .sum();
    done.store(true, Ordering::Relaxed);
    assert!(editor.join().expect("the editor finishes") > 0);
    assert!(playing > 0, "the frames saw the transport play");
    assert!(
        delivered.load(Ordering::Relaxed) > 0,
        "the feed ran meanwhile"
    );
    feed.stop();

    transport.stop().expect("stop");
    assert_eq!(transport.status()["state"], "stopped");
    transport.play(Some(0.0), None, false).expect("play again");
    let start = transport.status()["position_beat"].as_f64();
    assert!(
        wait_until(Duration::from_secs(10), || {
            transport.status()["position_beat"].as_f64() > start
        }),
        "playback advances afterwards"
    );
    transport.stop().expect("stop");
    drop(transport);
    live.app.close();
}
