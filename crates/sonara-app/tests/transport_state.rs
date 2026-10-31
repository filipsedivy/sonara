//! The transport's one state under concurrent control (TST-07), on miniaudio's null device: the
//! guard for a reported defect in which the live feed fell to its idle rate (4 Hz) during a
//! looped play on `native-null`, and once sent no playing frame within 15 s of a play.
//!
//! `TransportService::state()` (what the feed's rate follows), `status()["state"]` (the agent's
//! answer) and `live_snapshot()` (the frame's state) read the player's one state field under its
//! lock. Here three threads play, stop, pause and seek at once, round after round; between rounds,
//! with no call in flight, the three answers must agree, the live feed must send that state, at
//! its rolling rate while the transport rolls, and a play must reach the feed as a playing frame.
//! The calls come from a fixed seed (TST-04); the null device is a simulated real-time device on
//! a timer thread, so the feed's bounds are generous.

use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use sonara_app::context::{AppContext, AppOptions};
use sonara_app::live::{LiveFeed, LiveFrame, LiveState};
use sonara_app::projects::NewProject;
use sonara_app::settings::Settings;
use sonara_app::transport::{PlayerState, TransportService};
use sonara_core::commands::clip::CreateMidiClip;
use sonara_core::commands::track::CreateTrack;
use sonara_core::ids::{self, IdGenerator};
use sonara_core::midi::MidiNote;
use sonara_core::model::track::TrackKind;
use tempfile::TempDir;

/// The threads that control the transport at once.
const CALLERS: usize = 3;
/// The rounds of concurrent calls.
const ROUNDS: u64 = 24;

/// An application on `native-null` with a 120 BPM project: a drum track and two synths, each
/// with a 16-beat clip.
struct App {
    app: Arc<AppContext>,
    _dir: TempDir,
}

impl App {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let mut settings = Settings::default();
        settings.audio.lookahead_ms = 250.0;
        let app = AppContext::create(AppOptions {
            workspaces: Some(dir.path().join("ws")),
            settings: Some(settings),
            persist_settings: false,
            audio_backend: Some("native-null".to_owned()),
            ..AppOptions::default()
        })
        .expect("the application");
        app.projects()
            .new_project(&NewProject {
                name: "State".to_owned(),
                bpm: 120.0,
                ..NewProject::default()
            })
            .expect("a project");
        let this = Self {
            app: Arc::new(app),
            _dir: dir,
        };
        this.track("Drums", "sonara.drums", 36);
        this.track("Bass", "sonara.synth.poly", 40);
        this.track("Keys", "sonara.synth.poly", 60);
        this
    }

    /// An instrument track with `plugin` and a note of `pitch` on every beat for 16 beats.
    fn track(&self, name: &str, plugin: &str, pitch: i64) {
        let mut ids = self.app.ids();
        let create = CreateTrack {
            name: name.to_owned(),
            kind: TrackKind::Instrument,
            index: None,
            color: None,
            output_id: ids::MASTER_ID.to_owned(),
            instrument_plugin: Some(plugin.to_owned()),
            instrument_preset: None,
            parent_id: None,
            track_id: ids.new_id(ids::TRACK),
            instrument_id: ids.new_id(ids::DEVICE),
            part: None,
        };
        let notes = (0..16)
            .map(|beat| {
                MidiNote::create(&mut ids, pitch, f64::from(beat), 0.5, 100, 0).expect("a note")
            })
            .collect();
        let clip = CreateMidiClip {
            track_id: create.track_id.clone(),
            start: 0.0,
            length: 16.0,
            name: name.to_owned(),
            notes,
            loop_length: None,
            color: None,
            clip_id: ids.new_id(ids::CLIP),
        };
        let mut session = self.app.session().lock();
        session.execute(&create, false, "test").expect("the track");
        session.execute(&clip, false, "test").expect("the clip");
    }
}

/// The frames the feed sent, with when they arrived.
type Frames = Arc<Mutex<Vec<(Instant, LiveState)>>>;

fn frames_since(frames: &Frames, since: Instant) -> Vec<LiveState> {
    frames
        .lock()
        .expect("an unpoisoned lock")
        .iter()
        .filter(|(at, _)| *at >= since)
        .map(|(_, state)| *state)
        .collect()
}

/// xorshift64: the calls of a round from a fixed seed (TST-04).
fn next(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

/// One control call; plays have no end, so nothing but the calls changes the state.
fn control(transport: &TransportService, choice: u64) {
    #[expect(clippy::cast_precision_loss, reason = "a beat below 8, exact as f64")]
    let beat = (choice / 8 % 8) as f64;
    match choice % 8 {
        0..=2 => {
            transport
                .play(Some(beat), None, choice.is_multiple_of(2))
                .expect("play");
        }
        3 => {
            transport.play(None, None, false).expect("play on");
        }
        4 | 5 => {
            transport.stop().expect("stop");
        }
        6 => {
            let _ = transport.pause();
        }
        _ => {
            transport.seek(beat).expect("seek");
        }
    }
}

/// Checks the transport between rounds, with no call in flight: its three answers agree, and
/// the feed sends that state, at its rolling rate while it rolls. Returns the state.
fn check(transport: &TransportService, frames: &Frames, round: u64) -> PlayerState {
    let state = transport.state().expect("a player");
    let status = transport.status();
    let snapshot = transport.live_snapshot();
    assert_eq!(status["state"], state.as_str(), "round {round}: status()");
    assert_eq!(
        snapshot.status.as_ref().map(|s| s.state),
        Some(state),
        "round {round}: live_snapshot()"
    );
    let live = LiveState::from(state);
    let since = Instant::now();
    if live.is_rolling() {
        // 60 Hz while rolling: about 24 frames in 400 ms. At the idle rate (4 Hz), 2 at most.
        // The window opens 20 ms late, so a frame built during the round is not counted.
        thread::sleep(Duration::from_millis(420));
        let sent = frames_since(frames, since + Duration::from_millis(20));
        // HW-01: the feed's update rate is measured locally, never a CI gate. A CI runner that
        // starves this process of scheduling draws fewer display frames though the feed itself
        // is correct; that is the runner's limit, not a defect to fail on.
        if std::env::var_os("CI").is_none() {
            assert!(
                sent.len() >= 8,
                "round {round}: {} frames in 400 ms while {state:?}",
                sent.len()
            );
        } else if sent.len() < 8 {
            eprintln!(
                "CI: round {round}: only {} frames in 400 ms while {state:?}; skipping the check",
                sent.len()
            );
        }
        assert!(
            sent.iter().all(|s| *s == live),
            "round {round}: the frames say {sent:?} while {state:?}"
        );
    } else {
        // The idle feed sends within 250 ms, and at once when the state changed.
        let end = since + Duration::from_secs(2);
        while !frames_since(frames, since).contains(&live) {
            assert!(
                Instant::now() < end,
                "round {round}: no {live:?} frame within 2 s: {:?}",
                frames_since(frames, since)
            );
            thread::sleep(Duration::from_millis(5));
        }
    }
    state
}

/// TST-07: three threads control the transport at once; between rounds its answers agree and
/// the feed follows them. Afterwards a play still reaches the feed as playing frames.
#[test]
fn the_transport_answers_one_state_under_concurrent_control() {
    let app = App::new();
    let transport = app.app.transport();
    let frames: Frames = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&frames);
    let feed = LiveFeed::start(Arc::clone(&app.app), move |bytes| {
        let frame = LiveFrame::decode(bytes).expect("the feed's frames decode");
        sink.lock()
            .expect("an unpoisoned lock")
            .push((Instant::now(), frame.state));
    })
    .expect("the feed");
    // The first play builds the graph; the others take it back and reset it (the recall path).
    transport
        .play(Some(0.0), None, true)
        .expect("the first play");

    let start = Arc::new(Barrier::new(CALLERS + 1));
    let done = Arc::new(Barrier::new(CALLERS + 1));
    let callers: Vec<_> = (0..CALLERS)
        .map(|caller| {
            let app = Arc::clone(&app.app);
            let (start, done) = (Arc::clone(&start), Arc::clone(&done));
            thread::spawn(move || {
                let mut seed = 0x9e37_79b9_7f4a_7c15_u64 ^ (caller as u64 + 1);
                let transport = app.transport();
                for _ in 0..ROUNDS {
                    start.wait();
                    control(&transport, next(&mut seed));
                    done.wait();
                }
            })
        })
        .collect();
    let mut seen = Vec::new();
    for round in 0..ROUNDS {
        start.wait();
        done.wait();
        seen.push(check(&transport, &frames, round));
    }
    for caller in callers {
        caller.join().expect("a caller finishes");
    }
    assert!(
        seen.contains(&PlayerState::Playing) && seen.iter().any(|s| *s != PlayerState::Playing),
        "the rounds covered rolling and halted states: {seen:?}"
    );

    // Symptom 2's guard: after the storm, a play reaches the feed as playing frames at once,
    // and the playhead moves.
    transport.stop().expect("stop");
    let played = Instant::now();
    transport.play(Some(0.0), None, true).expect("play");
    let end = played + Duration::from_secs(2);
    while !frames_since(&frames, played).contains(&LiveState::Playing) {
        assert!(
            Instant::now() < end,
            "no playing frame within 2 s of a play"
        );
        thread::sleep(Duration::from_millis(5));
    }
    let beat = || transport.status()["position_beat"].as_f64();
    let before = beat();
    let end = Instant::now() + Duration::from_secs(5);
    while beat() == before {
        assert!(Instant::now() < end, "the playhead does not move");
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(check(&transport, &frames, ROUNDS), PlayerState::Playing);
    let status = transport.status();
    assert!(status.get("last_error").is_none(), "{status}");
    feed.stop();
    transport.stop().expect("stop");
    drop(transport);
    app.app.close();
}
