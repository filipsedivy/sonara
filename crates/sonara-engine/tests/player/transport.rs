//! The transport on a manual player, deterministically (TST-04): the underrun fixes of D11, the
//! answers of D15, the end of a play range, edits during playback, and the control calls
//! under concurrency (RT-06).
//!
//! The prototype counted an underrun when its ring ran dry at the end of a play range before
//! the render thread noticed, and when a restart while playing flushed the ring without
//! clearing `playing` (7 of 10 short ranges and 1 to 2 of 10 restarts on the
//! running prototype). Here the device plays the ring empty *before* the render thread steps
//! again, which is exactly the window in which the prototype counted them.

use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::json;
use sonara_core::plugins::builtin_registry;
use sonara_engine::player::{
    Player, PlayerSettings, PlayerSetup, PlayerState, Position, ProjectSnapshot, RecordPlan, Step,
    TakeTarget, click,
};
use sonara_media::export::SampleFormat;
use sonara_rt::midi_channel;
use sonara_testkit::Golden;

use crate::support::{
    Harness, ManualDevice, concat, first_difference, golden_project, no_media, tracks,
};

fn harness(scenario: &str) -> Harness {
    let golden = Golden::open().expect("golden data");
    let project = golden_project(
        &golden,
        &format!("transport/{scenario}/project.json"),
        |_| {},
    );
    Harness::new(project, no_media(), PlayerSettings::default())
}

fn beat_of(position: &Position) -> f64 {
    match position {
        Position::Musical { beat, .. } => *beat,
        Position::Frame(_) => panic!("a project position"),
    }
}

/// D11 at the end of a play range: once the last block is written, the ring drains without an
/// underrun, and the transport stops at the range's end frame.
#[test]
fn no_underrun_at_range_end() {
    let mut h = harness("play-range");
    let snapshot = h.snapshot.clone();
    // Half a beat at 96 BPM: 15 000 frames, 30 blocks.
    h.player
        .play(&snapshot, Some(1.5), Some(2.0), false)
        .expect("play");
    h.render(30);
    assert!(!h.control().is_playing(), "disarmed after the last block");
    for _ in 0..80 {
        h.callback(256);
    }
    assert_eq!(h.control().queued_frames(), 0);
    assert_eq!(h.control().stats().underruns, 0);
    assert_eq!(
        h.player.state(),
        PlayerState::Playing,
        "the render thread has not stepped"
    );
    assert_eq!(h.step(), Step::Idle);
    let status = h.player.status(Some(&snapshot.project));
    assert_eq!(status.state, PlayerState::Stopped);
    assert_eq!(
        beat_of(&status.position),
        2.0,
        "the position is the range's end"
    );
    assert_eq!(h.player.current_frame(), 60_000);
    assert_eq!(h.played.len(), 30);
    let stopped = h.player.stop(Some(&snapshot.project));
    assert!(!stopped.was_playing);
}

/// D11 on a restart and a seek while playing: the flush disarms `playing` until the new
/// generation's first block is written, so a device that plays the stale blocks out and runs
/// dry meanwhile counts nothing.
#[test]
fn no_underrun_on_restart_while_playing() {
    let mut h = harness("play-position");
    let snapshot = h.snapshot.clone();
    h.player
        .play(&snapshot, Some(0.0), None, false)
        .expect("play");
    for round in 0..6 {
        h.render(12);
        for _ in 0..4 {
            h.callback(256);
        }
        if round % 2 == 0 {
            h.player
                .play(&snapshot, Some(f64::from(round)), None, false)
                .expect("restart");
        } else {
            let _ = h.player.seek(&snapshot.project, f64::from(round) + 0.5);
        }
        // The device fades the stale blocks out and runs dry before the render thread steps.
        for _ in 0..60 {
            h.callback(256);
        }
        assert_eq!(h.control().queued_frames(), 0);
        assert_eq!(h.control().stats().underruns, 0, "round {round}");
    }
    h.render(12);
    assert!(h.control().is_playing());
    // The prototype's starts: generations 2 (first play), then one per restart and seek.
    assert_eq!(h.played.last().expect("a block").generation, 8);
}

/// `play` and `seek` answer the new position, although the device still plays the
/// position from before.
#[test]
fn seek_and_restart_answer_the_new_position() {
    let mut h = harness("play-position");
    let snapshot = h.snapshot.clone();
    let project = &snapshot.project;
    h.player
        .play(&snapshot, Some(2.0), None, false)
        .expect("play");
    h.render(16);
    for _ in 0..8 {
        h.callback(256);
    }
    let restarted = h
        .player
        .play(&snapshot, Some(8.0), None, false)
        .expect("play");
    assert_eq!(beat_of(&restarted.position), 8.0);
    assert!(
        h.player.current_frame() < 96_000,
        "the device still plays the old position"
    );
    let seeked = h.player.seek(project, 12.0);
    assert_eq!(beat_of(&seeked), 12.0);
    h.render(4);
    assert_eq!(h.played.last().expect("a block").tag, 288_000 + 3 * 512);
}

/// Deterministically: an edit during playback rebuilds the graph on the
/// maintenance thread, the render thread swaps it in, and the player reports its version.
#[test]
fn edit_during_playback_swaps_the_graph() {
    let mut h = harness("play-position");
    let snapshot = h.snapshot.clone();
    h.player
        .play(&snapshot, Some(0.0), None, false)
        .expect("play");
    h.render(10);
    assert_eq!(h.player.graph_version(), Some(1));
    h.edit(|data| tracks(data)[0]["mixer"]["volume_db"] = json!(-12.0));
    h.player.service();
    h.render(2);
    assert_eq!(h.player.graph_version(), Some(2));
    h.render(10);
    assert_eq!(h.control().stats().underruns, 0);
    // The meters follow the rebuilt graph's tracks.
    let meters = h.player.meters_at(i64::MAX);
    assert!(meters.iter().any(|m| m.track_id == "master"));
}

/// A play after an edit made while paused takes over the paused graph's processors (a rebuild
/// adopted on the control thread) and resets them, as the prototype's `graph_for` with
/// `previous` and `reset` do: renaming a track changes no sound, so the resumed blocks equal the
/// golden `play-position` resume bit for bit, synth note counter included.
#[test]
fn a_play_after_an_edit_resets_the_reused_processors() {
    let golden = Golden::open().expect("golden data");
    let mut h = harness("play-position");
    let snapshot = h.snapshot.clone();
    h.player
        .play(&snapshot, Some(6.0), None, false)
        .expect("play");
    h.render(48);
    let _ = h.player.pause(Some(&snapshot.project));
    h.edit(|data| tracks(data)[2]["name"] = json!("Keys (renamed)"));
    h.player.service();
    let _ = h.player.seek(&h.snapshot.project, 1.25);
    let edited = h.snapshot.clone();
    h.player.play(&edited, None, None, false).expect("resume");
    h.render(32);
    assert_eq!(h.player.graph_version(), Some(2));
    let recorded = golden
        .f32_array("transport/play-position/blocks")
        .expect("the recorded blocks");
    let resumed = &recorded.data[48 * 1024..80 * 1024];
    let played = concat(&h.played[48..80]);
    assert_eq!(first_difference(&played, resumed), None);
}

/// A loop that is not aligned to beats clicks on each pass's own beats. The golden loop
/// (95 960 to 143 900 frames: 40 frames before the downbeat of bar 2, 100 frames before beat 6)
/// is played with and without the metronome; the difference of the two is the clicks alone,
/// which must be the accented downbeat 40 frames into every pass and the plain beat 5 a beat
/// later, also across the wraps (the prototype clicked beat 6 past the loop end instead, and
/// lost the accent).
#[test]
fn clicks_follow_loop_wrap() {
    let golden = Golden::open().expect("golden data");
    let render = |metronome: bool| {
        let project = golden_project(&golden, "transport/loop-metronome/project.json", |data| {
            data["transport"]["metronome"] = json!(metronome);
        });
        let mut h = Harness::new(project, no_media(), PlayerSettings::default());
        let snapshot = h.snapshot.clone();
        h.player
            .play(
                &snapshot,
                Some(95_960.0 / 24_000.0),
                Some(143_900.0 / 24_000.0),
                true,
            )
            .expect("play the loop");
        h.render(200);
        concat(&h.played)
    };
    let (on, off) = (render(true), render(false));
    let pass = 143_900 - 95_960;
    let (accent, beat) = (click(48_000, true), click(48_000, false));
    let mut expected = vec![0.0_f64; on.len() / 2];
    let mut place = |at: usize, samples: &[f64]| {
        for (slot, value) in expected.iter_mut().skip(at).zip(samples) {
            *slot += value;
        }
    };
    for k in 0..3 {
        place(40 + k * pass, &accent);
        place(40 + 24_000 + k * pass, &beat);
    }
    let (on, _) = on.as_chunks::<2>();
    let (off, _) = off.as_chunks::<2>();
    let worst = on
        .iter()
        .zip(off)
        .zip(&expected)
        .flat_map(|((a, b), c)| {
            (0..2).map(move |ch| (f64::from(a[ch]) - f64::from(b[ch]) - c).abs())
        })
        .fold(0.0, f64::max);
    // Each block is rounded to f32 once with and once without the clicks.
    assert!(worst < 5e-7, "the clicks differ by {worst}");
}

/// One of the control calls of the RT-06 test, chosen by caller and call number.
fn control_call(
    player: &Player,
    snapshot: &ProjectSnapshot,
    takes: &std::path::Path,
    caller: u32,
    call: u32,
) {
    let project = &snapshot.project;
    match (call + caller) % 20 {
        0 => {
            let _ = player.play(snapshot, Some(f64::from(call % 8)), None, false);
        }
        1 => {
            let _ = player.play(snapshot, Some(1.0), Some(3.0), true);
        }
        2 => {
            let _ = player.seek(project, f64::from(call % 5));
        }
        3 => {
            let _ = player.pause(Some(project));
        }
        4 => {
            let _ = player.stop(Some(project));
        }
        5 => {
            let _ = player.status(Some(project));
        }
        6 => {
            let _ = player.current_frame();
        }
        7 => player.set_midi_focus(Some("trk_000000000002".to_owned())),
        8 => {
            let _ = player.midi_targets(project);
        }
        9 => {
            let _ = (player.meters(), player.device_reports());
        }
        10 => {
            let _ = player.graph_version();
        }
        11 => player.service(),
        12 => {
            let _ = player.open_stream(project, Some(2));
            let _ = player.is_recording();
        }
        13 => {
            let plan = RecordPlan {
                start_beat: Some(f64::from(call % 4)),
                takes: vec![TakeTarget {
                    track_id: "trk_000000000002".to_owned(),
                    path: takes.join(format!("{caller}-{call}.wav")),
                    channels: vec![0],
                    format: SampleFormat::Pcm24,
                }],
                midi_tracks: vec!["trk_000000000002".to_owned()],
            };
            let _ = player.record(snapshot, plan);
        }
        14 => {
            let _ = (player.state(), player.position_beats(Some(project)));
        }
        15 => {
            let _ = (
                player.messages_received(),
                player.midi_ports(),
                player.media_errors(),
            );
        }
        16 => {
            let _ = (
                player.meters_at(i64::from(call) * 512),
                player.output_peaks(),
            );
        }
        17 => {
            let _ = player.counters();
        }
        18 => {
            let _ = player.prepare_live(snapshot);
        }
        _ => {
            let (_keys, input) = midi_channel(8);
            let port = u8::try_from(call % 4).expect("a port");
            let _ = player.add_midi_inputs(vec![(port, input)], Some(snapshot));
            player.remove_midi_inputs(&[port]);
        }
    }
}

/// RT-06: every control call returns while the render thread does not run at all; what the
/// calls post waits for it, in order.
#[test]
fn control_calls_never_wait_for_the_render_thread() {
    let golden = Golden::open().expect("golden data");
    let project = golden_project(&golden, "transport/play-position/project.json", |_| {});
    let snapshot = ProjectSnapshot {
        project,
        version: 1,
        media: no_media(),
    };
    let source = {
        let snapshot = snapshot.clone();
        move || Some(snapshot.clone())
    };
    let (device, _callback) = ManualDevice::new();
    let (player, mut render, prefetch) = Player::manual(PlayerSetup {
        registry: Arc::new(builtin_registry()),
        device: Box::new(device),
        source: Arc::new(source),
        settings: PlayerSettings::default(),
    });
    let player = Arc::new(player);
    let takes = Arc::new(tempfile::tempdir().expect("a directory for takes"));
    let (done, finished) = mpsc::channel();
    for caller in 0..4_u32 {
        let player = Arc::clone(&player);
        let snapshot = snapshot.clone();
        let done = done.clone();
        let takes = Arc::clone(&takes);
        thread::spawn(move || {
            for call in 0..50_u32 {
                control_call(&player, &snapshot, takes.path(), caller, call);
            }
            done.send(caller).expect("the test waits");
        });
    }
    drop(done);
    for _ in 0..4 {
        finished
            .recv_timeout(Duration::from_secs(60))
            .expect("every caller finishes without the render thread");
    }
    // The render thread catches up with everything the calls posted.
    for _ in 0..2_000 {
        prefetch.fill();
        player.service();
        render.step();
    }
    let _ = player.stop(None);
}

/// RT-06 with the threads running: control calls from several threads while the render thread
/// renders and the maintenance thread rebuilds.
#[test]
fn control_calls_run_concurrently_with_playback() {
    let golden = Golden::open().expect("golden data");
    let project = golden_project(&golden, "transport/play-position/project.json", |_| {});
    let snapshot = ProjectSnapshot {
        project,
        version: 1,
        media: no_media(),
    };
    let source = {
        let snapshot = snapshot.clone();
        move || Some(snapshot.clone())
    };
    let (device, callback) = ManualDevice::new();
    let player = Arc::new(
        Player::new(PlayerSetup {
            registry: Arc::new(builtin_registry()),
            device: Box::new(device),
            source: Arc::new(source),
            settings: PlayerSettings::default(),
        })
        .expect("the player's threads"),
    );
    player
        .play(&snapshot, Some(0.0), None, false)
        .expect("play");
    let device = {
        let callback = Arc::clone(&callback);
        thread::spawn(move || {
            let mut out = vec![0.0; 512];
            for _ in 0..400 {
                if let Some((callback, _, _)) = callback.lock().unwrap().as_mut() {
                    callback.fill(&mut out);
                }
                thread::sleep(Duration::from_micros(500));
            }
        })
    };
    let callers: Vec<_> = (0..3_u32)
        .map(|caller| {
            let player = Arc::clone(&player);
            let snapshot = snapshot.clone();
            thread::spawn(move || {
                for call in 0..30_u32 {
                    match (call + caller) % 5 {
                        0 => {
                            let _ = player.seek(&snapshot.project, f64::from(call % 7));
                        }
                        1 => {
                            let _ = player.status(Some(&snapshot.project));
                        }
                        2 => {
                            let _ = player.play(&snapshot, Some(f64::from(call % 3)), None, false);
                        }
                        3 => {
                            let _ = player.meters();
                        }
                        _ => {
                            let _ = player.pause(Some(&snapshot.project));
                        }
                    }
                    thread::sleep(Duration::from_millis(1));
                }
            })
        })
        .collect();
    for caller in callers {
        caller.join().expect("a caller");
    }
    device.join().expect("the device");
    let _ = player.stop(Some(&snapshot.project));
    drop(player);
}

/// The mixer meters of a frame are those of the block heard in the current loop pass. On a
/// two-beat loop, the bass is turned down 24 dB after the first pass; in the second pass the
/// meters at a frame are the quiet ones. The prototype scanned its history from the oldest
/// entry and stopped at the first larger tag, so it returned the first pass's loud entry.
#[test]
fn meters_follow_the_current_loop_pass() {
    let mut h = harness("play-position");
    let snapshot = h.snapshot.clone();
    let bass = snapshot.project.tracks[1].id.clone();
    h.player
        .play(&snapshot, Some(0.0), Some(2.0), true)
        .expect("play a loop");
    // Two beats at 120 BPM: 48 000 frames, a pass of 93.75 blocks.
    h.render(95);
    let frame = 20 * 512 + 100;
    let peak = |meters: Vec<sonara_engine::player::TrackMeter>| {
        meters
            .iter()
            .find(|m| m.track_id == bass)
            .map(|m| m.peak[0].max(m.peak[1]))
            .expect("the bass's meter")
    };
    // The first pass's meter at the frame, from a harness that has not wrapped yet.
    let loud = {
        // A fresh harness gives the first pass's entry without a second pass in the history.
        let mut first = harness("play-position");
        let snapshot = first.snapshot.clone();
        first
            .player
            .play(&snapshot, Some(0.0), Some(2.0), true)
            .expect("play a loop");
        first.render(40);
        peak(first.player.meters_at(frame))
    };
    assert!(loud > 0.01, "the bass sounds at the frame: {loud}");
    h.edit(|data| tracks(data)[1]["mixer"]["volume_db"] = json!(-24.0));
    h.player.service();
    h.render(40);
    let quiet = peak(h.player.meters_at(frame));
    // About 24 dB down: the wrap chases, so the bass's voices restart and its level matches
    // the first pass only to a few dB (measured −21.8 dB). The first pass's entry is 0 dB.
    let db = 20.0 * (quiet / loud).log10();
    assert!(
        (-30.0..-18.0).contains(&db),
        "the second pass's meter: {quiet} against {loud} ({db:.1} dB)"
    );
}

/// Asserts that the transport's two answers agree on `expected`: `Player::state` (what the live
/// feed's rate follows) and `Player::status` (the frame's state and the agent's `status()`).
fn assert_state(h: &Harness, expected: PlayerState, at: &str) {
    assert_eq!(h.player.state(), expected, "{at}: state()");
    assert_eq!(
        h.player.status(Some(h.project())).state,
        expected,
        "{at}: status()"
    );
}

/// The generation of the last block written.
fn last_generation(h: &Harness) -> u32 {
    h.played.last().expect("a block").generation
}

/// Plays the one-beat loop from `start` (the resting position when `None`) and checks that both
/// answers say "playing" at once, before the render thread has seen anything.
fn play_loop(h: &mut Harness, start: Option<f64>) {
    let snapshot = h.snapshot.clone();
    let status = h
        .player
        .play(&snapshot, start, Some(1.0), true)
        .expect("play the loop");
    assert_eq!(status.state, PlayerState::Playing);
    assert_state(h, PlayerState::Playing, "the play's answer");
}

/// TST-07, after a reported live feed at 4 Hz during playback: the transport has
/// one state on every path. `state()` and `status()` answer from the same field under the
/// player's lock, so they agree at every step, also on the reset-on-play path: a play that finds
/// a graph takes it back from the render thread (`Message::Recall`), resets it and starts it
/// when it is back, and says "playing" from the call on, whatever comes between (a seek, a
/// stop, another play, a pause). A looped play never stops by itself.
#[test]
fn the_transport_has_one_state_on_every_path() {
    let mut h = harness("play-position");
    let project = h.snapshot.project.clone();
    // A loop of one beat (24 000 frames, 46.9 blocks) plays its first graph at once: three
    // passes later it still plays (no end of range for a loop).
    play_loop(&mut h, Some(0.0));
    h.render(150);
    assert!(
        h.played.windows(2).any(|w| w[1].tag < w[0].tag),
        "the loop wrapped"
    );
    assert_state(&h, PlayerState::Playing, "after three passes");
    assert_eq!(last_generation(&h), 2);

    // Stop, then play: the graph comes back from the render thread before the start.
    let _ = h.player.stop(Some(&project));
    assert_state(&h, PlayerState::Stopped, "stopped");
    play_loop(&mut h, Some(0.0));
    h.step();
    assert_state(&h, PlayerState::Playing, "the recall applied");
    h.player.service();
    assert_state(&h, PlayerState::Playing, "the recalled graph started");
    h.render(4);
    assert_eq!(last_generation(&h), 4, "the play's generation renders");

    // A seek before the recalled graph is back starts the play at the seek.
    let _ = h.player.stop(Some(&project));
    play_loop(&mut h, Some(0.0));
    let _ = h.player.seek(&project, 0.5);
    assert_state(&h, PlayerState::Playing, "a seek while the graph is away");
    let before = h.played.len();
    h.render(2);
    let first = &h.played[before];
    assert_eq!((first.generation, first.tag), (7, 12_000));
    assert_state(&h, PlayerState::Playing, "playing from the seek");

    // A stop while the graph is away sends it back unchanged; the next play recalls it again.
    let _ = h.player.stop(Some(&project));
    play_loop(&mut h, Some(0.0));
    h.step();
    let _ = h.player.stop(Some(&project));
    assert_state(&h, PlayerState::Stopped, "stopped while the graph is away");
    play_loop(&mut h, Some(0.25));
    for _ in 0..3 {
        h.step();
        assert_state(&h, PlayerState::Playing, "two recalls in flight");
        h.player.service();
        assert_state(&h, PlayerState::Playing, "two recalls in flight");
    }
    h.render(2);
    assert_eq!(last_generation(&h), 11);

    // A play while playing, and a resume after a pause, take the same path.
    play_loop(&mut h, Some(0.5));
    h.render(2);
    assert_eq!(last_generation(&h), 12);
    assert!(h.player.pause(Some(&project)).paused);
    assert_state(&h, PlayerState::Paused, "paused");
    play_loop(&mut h, None);
    h.render(2);
    assert_eq!(last_generation(&h), 14);
    assert_state(&h, PlayerState::Playing, "resumed");
    assert_eq!(h.player.status(Some(&project)).last_error, None);
}

/// TST-07: the two ways the transport stops without a call agree in both answers. A play range
/// that ends says "stopped" at the first look after the render thread noticed its end, whichever
/// answer is asked first; a play whose recalled graph cannot be rebuilt stops with its error,
/// and the render thread plays none of it.
#[test]
fn a_range_end_and_a_failed_start_stop_both_answers() {
    let mut h = harness("play-position");
    let snapshot = h.snapshot.clone();
    let project = snapshot.project.clone();
    play_loop(&mut h, Some(0.0));
    h.render(4);

    // A quarter beat from 0, on the recalled graph: 6 000 frames.
    h.player
        .play(&snapshot, Some(0.0), Some(0.25), false)
        .expect("play a range");
    let mut rounds = 0;
    loop {
        let step = h.step();
        // Asked before the maintenance looks: `state()` first, then `status()`.
        let state = h.player.state();
        assert_eq!(
            h.player.status(Some(&project)).state,
            state,
            "round {rounds}"
        );
        if state == PlayerState::Stopped {
            break;
        }
        if step != Step::Wrote {
            h.callback(256);
            h.player.service();
        }
        rounds += 1;
        assert!(rounds < 10_000, "the range never ended");
    }
    assert_state(&h, PlayerState::Stopped, "the range ended");
    assert_eq!(h.player.current_frame(), 6_000, "at the range's end");

    // The next play finds a graph, which cannot be rebuilt for the edited project.
    h.edit(|data| tracks(data)[1]["instrument"]["params"]["voice_mode"] = json!("nope"));
    play_loop(&mut h, Some(0.0));
    h.step();
    h.player.service();
    assert_state(&h, PlayerState::Stopped, "the start failed");
    let error = h.player.status(Some(&project)).last_error;
    assert!(
        error.as_deref().is_some_and(|e| e.contains("voice_mode")),
        "the failure is reported: {error:?}"
    );
    let before = h.played.len();
    for _ in 0..20 {
        assert_ne!(h.step(), Step::Wrote, "the failed play renders nothing");
        h.callback(256);
        h.player.service();
    }
    assert_eq!(h.played.len(), before);
    assert_state(&h, PlayerState::Stopped, "after the failed start");
}
