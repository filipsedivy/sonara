//! Media off the render thread: the prefetch thread reads the clips
//! ahead along the player's frame sequence, and the render thread only copies what was read.
//!
//! The oracles:
//!
//! - **Offline renders of the same frame ranges.** A project of audio clips alone (the coverage
//!   batch's audio tracks without their convolver) renders every frame from its media alone, so
//!   the played blocks must equal the offline renderer's output for the same frames, bit for
//!   bit: a straight play over WAV, FLAC, resampled and MP3 clips, and a loop that is not
//!   aligned to blocks, whose passes each equal an offline render of the loop range.
//! - **The graph driven through the same sequence.** With instruments and an effect (the full
//!   coverage batch: notes chase at the wrap, the convolver keeps its tail), the played frames
//!   must equal a live graph of the same project that reads its media itself (`read_media`)
//!   while it is driven through the loop's frame sequence: the prefetched clip audio is the
//!   audio the graph would have read.
//!
//! The harness reads ahead before each step, as the prefetch thread does; one test stops doing
//! that and checks that late clip audio is silent and counted, never waited for. Every render
//! step runs inside `assert_no_alloc` (the harness), and under `--features rtsan` the step is
//! `#[nonblocking]`, so RealtimeSanitizer finds any file I/O or lock in it.

use std::num::NonZeroUsize;

use serde_json::{Value, json};
use sonara_core::model::JsonMap;
use sonara_core::model::project::Project;
use sonara_core::plugins::builtin_registry;
use sonara_engine::player::{PlayerSettings, Step};
use sonara_engine::{GraphBuilder, OfflineRenderer, RenderRequest, Tail};
use sonara_rt::{MidiEvent, midi_channel};
use sonara_testkit::Golden;

use crate::support::{
    Harness, concat, coverage_media, first_difference, golden_project, to_f32, tracks,
};

const COVERAGE: &str = "coverage/final.project.json";
/// The coverage batch's audio tracks: the tone clips (WAV, and a resampled 44.1 kHz mono WAV),
/// the chord (FLAC) and the saw (MP3).
const AUDIO_TRACKS: [&str; 3] = ["trk_000000000027", "trk_000000000034", "trk_000000000039"];

/// The coverage batch with only its audio tracks, without inserts.
fn clips_only(data: &mut JsonMap) {
    let tracks = tracks(data);
    tracks.retain(|t| AUDIO_TRACKS.contains(&t["id"].as_str().unwrap_or_default()));
    for track in tracks.iter_mut() {
        track["inserts"] = json!([]);
    }
}

fn harness(project: Project, golden: &Golden) -> (Harness, tempfile::TempDir) {
    let (media, cache) = coverage_media(golden, &project);
    (
        Harness::new(project, media, PlayerSettings::default()),
        cache,
    )
}

/// The offline render of beats `[start, end)`, without a tail, as `f32`.
fn offline(harness: &Harness, start: f64, end: f64) -> Vec<f32> {
    let registry = builtin_registry();
    let rendered = OfflineRenderer::new(&registry, &harness.snapshot.media)
        .render(
            harness.project(),
            &RenderRequest {
                start_beat: start,
                end_beat: Some(end),
                tail: Tail::None,
                ..RenderRequest::default()
            },
            None,
        )
        .expect("the offline render");
    to_f32(&rendered.data)
}

#[test]
fn prefetched_playback_equals_offline_renders() {
    let golden = Golden::open().expect("golden data");
    let project = golden_project(&golden, COVERAGE, clips_only);
    let (mut harness, _cache) = harness(project, &golden);
    let snapshot = harness.snapshot.clone();
    harness
        .player
        .play(&snapshot, Some(3.5), None, false)
        .expect("play");
    // Beats 3.5 to 26.5: every clip of the three tracks.
    harness.render(1078);
    let played = concat(&harness.played);
    let expected = offline(&harness, 3.5, 3.5 + 1078.0 * 512.0 / 24_000.0);
    assert_eq!(expected.len(), played.len());
    assert!(expected.iter().any(|x| x.abs() > 0.1), "the clips sound");
    assert_eq!(
        first_difference(&played, &expected),
        None,
        "the played blocks equal the offline render"
    );
    assert_eq!(harness.player.counters().media_late, 0);
    assert_eq!(harness.player.media_errors(), 0);
}

/// The loop 5.9 to 10.3 beats (141 600 to 247 200 frames, 206.25 blocks): every pass equals the
/// offline render of the loop range, so the cut is sample-accurate and the prefetch thread
/// follows the wraps.
#[test]
fn loop_cut_is_sample_accurate() {
    let golden = Golden::open().expect("golden data");
    let project = golden_project(&golden, COVERAGE, clips_only);
    let (mut harness, _cache) = harness(project, &golden);
    let snapshot = harness.snapshot.clone();
    let status = harness
        .player
        .play(&snapshot, Some(5.9), Some(10.3), true)
        .expect("play");
    assert_eq!(status.looped, Some((141_600, 247_200)));
    harness.render(650);
    let pass = offline(&harness, 5.9, 10.3);
    assert_eq!(pass.len(), 2 * 105_600);
    let played = concat(&harness.played);
    let expected: Vec<f32> = pass.iter().copied().cycle().take(played.len()).collect();
    assert_eq!(
        first_difference(&played, &expected),
        None,
        "every pass equals the loop range"
    );
    // The wrap blocks carry their two parts.
    let wraps: Vec<&Vec<(i64, usize)>> = harness
        .played
        .iter()
        .map(|p| &p.parts)
        .filter(|parts| parts.len() > 1)
        .collect();
    assert_eq!(wraps.len(), 3, "three wraps in 650 blocks");
    assert_eq!(*wraps[0], vec![(247_072, 128), (141_600, 384)]);
    assert_eq!(harness.player.counters().media_late, 0);
}

/// With instruments, an insert and audio clips (the whole coverage batch), a loop over notes
/// that sound across both ends, so every wrap chases: the played frames equal a live graph
/// that reads its own media while it is driven through the same frame sequence.
#[test]
fn prefetched_loop_with_instruments_equals_the_graph() {
    let golden = Golden::open().expect("golden data");
    let project = golden_project(&golden, COVERAGE, |_| {});
    let (mut harness, _cache) = harness(project, &golden);
    let snapshot = harness.snapshot.clone();
    harness
        .player
        .play(&snapshot, Some(5.9), Some(10.3), true)
        .expect("play");
    harness.render(500);
    let played = concat(&harness.played);

    let registry = builtin_registry();
    let mut builder = GraphBuilder::new(NonZeroUsize::new(512).expect("a block"));
    let mut graph = builder
        .fresh(&snapshot.project, &registry, &snapshot.media)
        .expect("the reference graph");
    let (start, end) = (141_600_i64, 247_200_i64);
    let mut src = start;
    let mut reference: Vec<[f64; 2]> = Vec::new();
    while reference.len() * 2 < played.len() {
        graph.read_media(src).expect("the clips read");
        let output = graph.process(src, true);
        let kept = if src < end && end <= src + 512 {
            usize::try_from(end - src).expect("a short cut")
        } else {
            512
        };
        reference.extend_from_slice(&output[..kept]);
        src = if src + 512 >= end { start } else { src + 512 };
    }
    let expected = to_f32(&reference);
    assert!(expected.iter().any(|x| x.abs() > 0.1));
    assert_eq!(
        first_difference(&played, &expected[..played.len()]),
        None,
        "the prefetched clips equal the clips the graph reads"
    );
}

/// A restart resets the player's graph on the control thread (the step-5 amendment): the
/// coverage batch's audio tracks with the Tone track's convolver, whose impulse response a live
/// graph keeps in deferred mode, play the same frames on the second play as on the first, and
/// both equal the offline render.
#[test]
fn a_restart_keeps_the_convolver_response() {
    let golden = Golden::open().expect("golden data");
    let project = golden_project(&golden, COVERAGE, |data| {
        tracks(data).retain(|t| AUDIO_TRACKS.contains(&t["id"].as_str().unwrap_or_default()));
    });
    assert!(
        project.tracks.iter().any(|t| !t.inserts.is_empty()),
        "the convolver plays"
    );
    let (mut harness, _cache) = harness(project, &golden);
    let snapshot = harness.snapshot.clone();
    harness
        .player
        .play(&snapshot, Some(3.5), None, false)
        .expect("play");
    harness.render(160);
    let _ = harness.player.stop(Some(&snapshot.project));
    harness
        .player
        .play(&snapshot, Some(3.5), None, false)
        .expect("play again");
    harness.render(160);
    let first = concat(&harness.played[..160]);
    let second = concat(&harness.played[160..]);
    assert!(first.iter().any(|x| x.abs() > 0.1));
    assert_eq!(
        first_difference(&second, &first),
        None,
        "the restart plays the same"
    );
    let expected = offline(&harness, 3.5, 3.5 + 160.0 * 512.0 / 24_000.0);
    assert_eq!(
        first_difference(&first, &expected),
        None,
        "equal to the offline render"
    );
}

/// RT-03, RT-12: without the prefetch thread, the clip audio read ahead at the start (the
/// lookahead target plus two blocks) plays, and then the clips are silent and counted; the
/// render thread never waits.
#[test]
fn late_media_plays_silent_clips_and_is_counted() {
    let golden = Golden::open().expect("golden data");
    let project = golden_project(&golden, COVERAGE, clips_only);
    let (mut harness, _cache) = harness(project, &golden);
    harness.prefetching = false;
    let snapshot = harness.snapshot.clone();
    harness
        .player
        .play(&snapshot, Some(4.5), None, false)
        .expect("play");
    harness.render(30);
    // 80 ms at 48 kHz: a target of 8 blocks, so 10 blocks are read before the start.
    let primed = 10;
    let played = concat(&harness.played);
    let expected = offline(&harness, 4.5, 4.5 + 30.0 * 512.0 / 24_000.0);
    let split = primed * 1024;
    assert_eq!(first_difference(&played[..split], &expected[..split]), None);
    assert!(
        expected[split..].iter().any(|x| *x != 0.0),
        "the clip goes on"
    );
    assert!(
        played[split..].iter().all(|x| *x == 0.0),
        "late clips are silent"
    );
    assert_eq!(harness.player.counters().media_late, 20);
    // Once the prefetch thread catches up, the next block has its clips again.
    harness.prefetching = true;
    harness.render(1);
    let last = harness.played.last().expect("a block");
    assert!(last.samples.iter().any(|x| *x != 0.0));
}

/// Live input on a stopped transport inside an audio clip renders the instrument alone:
/// the idle blocks equal those of the same project with the clip muted, and nothing counts as
/// late media.
#[test]
fn idle_step_renders_no_clip_audio() {
    let golden = Golden::open().expect("golden data");
    let idle_blocks = |mute: bool| {
        let project = golden_project(&golden, COVERAGE, |data| {
            clips_only(data);
            // The poly synth track of the `controls` scenario, armed.
            let keys: Value = {
                let controls = sonara_project::serializer::loads(
                    &golden
                        .bytes("transport/controls/project.json")
                        .expect("the controls project"),
                )
                .expect("project JSON");
                controls["tracks"][0].clone()
            };
            let mut keys = keys;
            keys["input"]["armed"] = json!(true);
            tracks(data).push(keys);
            if mute {
                for track in tracks(data) {
                    for clip in track["clips"].as_array_mut().into_iter().flatten() {
                        clip["muted"] = json!(true);
                    }
                }
            }
        });
        let (mut harness, _cache) = harness(project, &golden);
        let snapshot = harness.snapshot.clone();
        // The stopped transport rests inside the first tone clip (beats 4 to 6).
        let position = harness.player.seek(&snapshot.project, 4.5);
        assert_eq!(position.frame(), 108_000);
        let (mut keyboard, input) = midi_channel(16);
        assert!(keyboard.push(MidiEvent::new(0, 0, &[0x90, 60, 100]).expect("a note on")));
        harness
            .player
            .add_midi_inputs(vec![(0, input)], Some(&snapshot))
            .expect("live input");
        assert_eq!(
            harness.player.midi_targets(&snapshot.project),
            ["trk_000000000002"]
        );
        harness.render(6);
        assert!(harness.played.iter().all(|p| p.idle && p.tag == 108_000));
        assert_eq!(harness.player.messages_received(), 1);
        assert_eq!(harness.player.counters().media_late, 0);
        let blocks = concat(&harness.played);
        // Without live input, a stopped transport renders nothing.
        harness.player.remove_midi_inputs(&[0]);
        assert_eq!(harness.step(), Step::Waiting);
        assert_eq!(harness.step(), Step::Idle);
        blocks
    };
    let with_clip = idle_blocks(false);
    let muted = idle_blocks(true);
    assert!(with_clip.iter().any(|x| x.abs() > 1e-3), "the note sounds");
    assert_eq!(
        first_difference(&with_clip, &muted),
        None,
        "no clip audio while stopped"
    );
}
