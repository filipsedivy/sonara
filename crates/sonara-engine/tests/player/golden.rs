//! The played blocks against the golden `transport` area: the
//! recorder's control calls are replayed on a manual player, and every block it
//! writes is compared by index with the prototype's.
//!
//! **Driving.** The recorder never started the prototype's render thread; it called the
//! player's own render step (`_step`) until the scenario's number of blocks was written, and
//! slept 1 ms while the null device played whenever the step had nothing to do
//! (`scripts/golden/area_transport.py`, `_Run.render`). The harness does the same: it steps the
//! render loop, and when a step writes nothing, it plays one 256-frame period of the device and
//! runs the maintenance. The block sequence depends only on the project, the start, the loop
//! and the control calls, so this reproduces it exactly; only the timing of the writes differs.
//!
//! **Comparison**, per block: the tag, the generation and the frame count exactly; the carry
//! chunks of a loop wrap exactly; the metronome clicks exactly, except where the prototype's
//! blocks carry D8: there the corrected placement (`per_chunk_clicks`) is asserted,
//! and the samples are compared with the recording after the prototype's clicks are replaced by
//! the corrected ones; the samples by the null test of (peak residual ≤ −120 dBFS).
//!
//! **The control answers** (the transport service turns them into JSON): every key the recording does not mark
//! `<VOLATILE>` is compared, and after each step the stream's `playing` flag equals the last
//! `set_playing` call the prototype made in it (D11 moves none of those calls).
//!
//! **`record-count-in`** is a recording: `record` from beat 3 with a one-bar count-in
//! on a duplex stream that captures silence, as the null device does. Its blocks start at the
//! count-in with the clicks forced on. The stop's take is aligned with the corrected capture
//! start: the first frame of the count-in, where the prototype stamped one period later.

use serde_json::{Value, json};
use sonara_core::model::project::Project;
use sonara_core::time::parse_position;
use sonara_engine::player::{
    PlayerSettings, PlayerStatus, Position, RecordPlan, TakeTarget, click,
};
use sonara_media::export::SampleFormat;
use sonara_testkit::{Golden, peak_residual_dbfs};

use crate::support::{Harness, Played, golden_project, no_media};

/// The golden null-test threshold.
const NULL_TEST_DBFS: f64 = -120.0;
const VOLATILE: &str = "<VOLATILE>";
const BLOCK: usize = 512;

/// A position argument of the agent tools: beats, or `"bar:beat"`.
fn beats(project: &Project, value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(text) => {
            Some(parse_position(text, &project.time_signatures).expect("a bar:beat position"))
        }
        _ => None,
    }
}

fn position_keys(position: &Position) -> Vec<(&'static str, Value)> {
    match position {
        Position::Musical {
            beat,
            seconds,
            bar,
            beat_in_bar,
            ..
        } => vec![
            ("position_beat", json!(beat)),
            ("position_seconds", json!(seconds)),
            ("bar_beat", json!(format!("{bar}:{beat_in_bar}"))),
        ],
        Position::Frame(frame) => vec![("position_frame", json!(frame))],
    }
}

fn status_keys(status: &PlayerStatus) -> Vec<(&'static str, Value)> {
    let mut keys = vec![
        ("state", json!(status.state.as_str())),
        ("device_sample_rate", json!(status.device_sample_rate)),
        ("buffer_frames", json!(status.buffer_frames)),
        ("queued_ms", json!(status.queued_ms)),
        ("lookahead_ms", json!(status.lookahead_ms)),
        (
            "output_latency_estimate_ms",
            json!(status.output_latency_estimate_ms),
        ),
        ("underruns", json!(status.underruns)),
        ("callback_load", json!(status.callback_load)),
        ("render_load", json!(status.render_load)),
        ("loop", json!(status.looped.map(|(a, b)| [a, b]))),
        ("device_errors", json!(status.device_errors)),
    ];
    keys.extend(position_keys(&status.position));
    keys
}

/// Compares the answer's keys with the recording's, except those it marks volatile.
fn check_answer(scenario: &str, index: usize, recorded: &Value, keys: &[(&str, Value)]) {
    for (key, value) in keys {
        let expected = &recorded[*key];
        if expected == VOLATILE {
            continue;
        }
        assert_eq!(
            value, expected,
            "{scenario} step {index}: `{key}` differs from the recording"
        );
    }
}

/// The last `set_playing` value among a step's backend calls.
fn last_set_playing(step: &Value) -> Option<bool> {
    step["backend"]
        .as_array()?
        .iter()
        .filter(|event| event[0] == "set_playing")
        .filter_map(|event| event[1].as_bool())
        .next_back()
}

/// Replays `scenario` and returns the harness with every played block.
fn replay(golden: &Golden, scenario: &str) -> Harness {
    let base = format!("transport/{scenario}");
    let transcript: Value = golden
        .json(&format!("{base}/transcript.json"))
        .expect("the transcript");
    let project = golden_project(golden, &format!("{base}/project.json"), |_| {});
    let settings = &transcript["settings"];
    let mut harness = Harness::new(
        project,
        no_media(),
        PlayerSettings {
            lookahead_ms: settings["lookahead_ms"].as_f64().expect("the lookahead"),
            live_lookahead_ms: settings["live_lookahead_ms"]
                .as_f64()
                .expect("the live lookahead"),
            buffer_frames: 256,
            output_device: None,
            input_device: None,
        },
    );
    let steps = transcript["steps"].as_array().expect("the steps");
    for (index, step) in steps.iter().enumerate() {
        replay_step(&mut harness, scenario, index, step, &transcript);
    }
    harness
}

/// Replays one step of the transcript: a control call with its answer, or a render step.
fn replay_step(
    harness: &mut Harness,
    scenario: &str,
    index: usize,
    step: &Value,
    transcript: &Value,
) {
    let recorded = &step["result"];
    let snapshot = harness.snapshot.clone();
    let project = &snapshot.project;
    match (step["via"].as_str(), step["call"].as_str()) {
        (Some("render"), _) => {
            let range = step["blocks"].as_array().expect("a block range");
            let block_index = |value: &Value| {
                usize::try_from(value.as_u64().expect("a block index")).expect("a short run")
            };
            let (first, last) = (block_index(&range[0]), block_index(&range[1]));
            assert_eq!(
                harness.played.len(),
                first,
                "{scenario}: blocks before step {index}"
            );
            if step["until"] == "stopped" {
                harness.render_until_stopped();
            } else {
                harness.render(last - first);
            }
            assert_eq!(
                harness.played.len(),
                last,
                "{scenario}: blocks of step {index}"
            );
        }
        (Some("tool"), Some("play_preview")) => {
            let args = &step["args"];
            let status = harness
                .player
                .play(
                    &snapshot,
                    beats(project, &args["start"]),
                    beats(project, &args["end"]),
                    args["loop"].as_bool().unwrap_or(false),
                )
                .expect("play");
            check_answer(scenario, index, recorded, &status_keys(&status));
        }
        (Some("tool"), Some("start_recording")) => {
            start_recording(harness, scenario, index, step);
        }
        (Some("tool"), Some("stop_playback")) => {
            stop_playback(harness, scenario, index, step, transcript);
        }
        (Some("service"), Some("status")) => {
            let status = harness.player.status(Some(project));
            check_answer(scenario, index, recorded, &status_keys(&status));
        }
        (Some("service"), Some("pause")) => {
            let paused = harness.player.pause(Some(project));
            let mut keys = vec![("paused", json!(paused.paused))];
            if let Some(position) = &paused.position {
                keys.extend(position_keys(position));
            }
            check_answer(scenario, index, recorded, &keys);
        }
        (Some("service"), Some("seek")) => {
            let beat = step["args"][0].as_f64().expect("a beat");
            let position = harness.player.seek(project, beat);
            check_answer(scenario, index, recorded, &position_keys(&position));
        }
        other => panic!("{scenario}: unexpected step {other:?}"),
    }
    if let Some(playing) = last_set_playing(step) {
        assert_eq!(
            harness.control().is_playing(),
            playing,
            "{scenario} step {index}: the playing flag"
        );
    }
}

/// `start_recording`: the duplex stream the prototype opened, then the take of the armed audio
/// track.
fn start_recording(harness: &mut Harness, scenario: &str, index: usize, step: &Value) {
    let recorded = &step["result"];
    let snapshot = harness.snapshot.clone();
    let project = &snapshot.project;
    let requested = &step["backend"][0][1]["requested"];
    let inputs = requested["input_channels"]
        .as_u64()
        .expect("input channels");
    harness
        .player
        .open_stream(
            project,
            Some(usize::try_from(inputs).expect("a channel count")),
        )
        .expect("the duplex stream");
    let track = recorded["tracks"][0].as_str().expect("the recorded track");
    let position = harness
        .player
        .record(
            &snapshot,
            RecordPlan {
                start_beat: beats(project, &step["args"]["start"]),
                takes: vec![TakeTarget {
                    track_id: track.to_owned(),
                    path: harness.scratch.path().join("take.wav"),
                    channels: vec![0],
                    format: SampleFormat::Pcm24,
                }],
                midi_tracks: Vec::new(),
            },
        )
        .expect("record");
    check_answer(scenario, index, recorded, &position_keys(&position));
}

/// `stop_playback`, and for a recording the take's alignment inputs.
fn stop_playback(
    harness: &mut Harness,
    scenario: &str,
    index: usize,
    step: &Value,
    transcript: &Value,
) {
    let recorded = &step["result"];
    let project = harness.snapshot.project.clone();
    let stopped = harness.player.stop(Some(&project));
    let mut keys = vec![
        ("stopped", json!(true)),
        ("was_playing", json!(stopped.was_playing)),
    ];
    if let Some(position) = &stopped.position {
        keys.extend(position_keys(position));
    }
    check_answer(scenario, index, recorded, &keys);
    let Some(take) = recorded.get("recording") else {
        return;
    };
    let done = stopped.recording.expect("the recording's takes");
    let latency = done.latency_frames + done.input_latency_frames;
    assert_eq!(
        take["latency_compensation_frames"],
        json!(latency),
        "{scenario} step {index}: the latency compensation"
    );
    assert_eq!(take["capture_overruns"], json!(done.capture_overruns));
    assert_eq!(done.takes.len(), 1);
    assert!(done.takes[0].frames > 0 && done.takes[0].error.is_none());
    // The take starts at the count-in's first played frame (block 0, tag 0). The prototype
    // stamped the end of that callback, one 256-frame period later, so its clip's source offset
    // is 256 frames short of the corrected one.
    assert_eq!(done.capture_start, Some(0), "D4: the capture start");
    let summary = &transcript["take"];
    assert_eq!(summary["prototype_defect"], "D4");
    let aligned = done.capture_start.unwrap_or(done.record_from) - i64::from(latency);
    let trim = (done.record_from - aligned).max(0);
    assert_eq!(
        summary["clip"]["source_offset"].as_i64(),
        Some(trim - 256),
        "the prototype's offset is one period short"
    );
}

/// The prototype's clicks at 48 kHz (`transport/clicks`).
fn golden_clicks(golden: &Golden) -> (Vec<f64>, Vec<f64>) {
    let read = |name: &str| {
        golden
            .f64_array(&format!("transport/clicks/{name}"))
            .expect("a click vector")
            .data
    };
    (read("accent"), read("beat"))
}

fn clicks_of(value: &Value) -> Option<Vec<(i64, bool)>> {
    value.as_array().map(|clicks| {
        clicks
            .iter()
            .map(|c| {
                (
                    c[0].as_i64().expect("an offset"),
                    c[1].as_bool().expect("an accent"),
                )
            })
            .collect()
    })
}

/// Adds `sign ×` the clicks to `signal`, a run of blocks that starts at the block that started
/// them.
fn add_clicks(
    signal: &mut [f64],
    clicks: &[(i64, bool)],
    tables: &(Vec<f64>, Vec<f64>),
    sign: f64,
) {
    for &(offset, accent) in clicks {
        let samples = if accent { &tables.0 } else { &tables.1 };
        for (i, sample) in samples.iter().enumerate() {
            let at = offset + i64::try_from(i).expect("a click frame");
            if let Some(slot) = usize::try_from(at).ok().and_then(|at| signal.get_mut(at)) {
                *slot += sign * sample;
            }
        }
    }
}

/// Compares every played block with the recording; returns the largest residual in dBFS
/// (negative infinity when every block is bit-exact).
fn compare(golden: &Golden, scenario: &str, played: &[Played]) -> f64 {
    let recorded = golden
        .f32_array(&format!("transport/{scenario}/blocks"))
        .expect("the recorded blocks");
    let meta = recorded.header["blocks"]
        .as_array()
        .expect("block metadata");
    assert_eq!(recorded.shape, [meta.len(), BLOCK, 2]);
    assert_eq!(played.len(), meta.len(), "{scenario}: block count");
    let tables = golden_clicks(golden);
    let mut worst = f64::NEG_INFINITY;
    let mut d8 = Vec::new();
    for (i, (block, meta)) in played.iter().zip(meta).enumerate() {
        let what = format!("{scenario} block {i}");
        assert_eq!(
            block.tag,
            meta["tag"].as_i64().expect("a tag"),
            "{what}: tag"
        );
        assert_eq!(
            u64::from(block.generation),
            meta["generation"].as_u64().expect("a generation"),
            "{what}: generation"
        );
        assert_eq!(meta["frames"], BLOCK, "{what}: frames");
        assert_eq!(block.samples.len(), 2 * BLOCK);
        let chunks: Vec<(i64, usize)> = meta["chunks"].as_array().map_or_else(
            || vec![(block.tag, BLOCK)],
            |chunks| {
                chunks
                    .iter()
                    .map(|c| {
                        let frames = c[1].as_u64().expect("a chunk length");
                        (
                            c[0].as_i64().expect("a chunk tag"),
                            usize::try_from(frames).expect("a short chunk"),
                        )
                    })
                    .collect()
            },
        );
        assert_eq!(block.parts, chunks, "{what}: carry chunks");
        let expected_clicks =
            clicks_of(&meta["per_chunk_clicks"]).or_else(|| clicks_of(&meta["clicks"]));
        assert_eq!(block.clicks, expected_clicks, "{what}: clicks");
        let samples = &recorded.data[i * 2 * BLOCK..(i + 1) * 2 * BLOCK];
        if meta["prototype_defect"] == "D8" {
            if meta.get("per_chunk_clicks").is_some() {
                d8.push(i);
            }
            continue;
        }
        let residual = peak_residual_dbfs(samples, &block.samples);
        assert!(
            residual <= NULL_TEST_DBFS,
            "{what}: residual {residual:.1} dBFS"
        );
        worst = worst.max(residual);
    }
    for &wrap in &d8 {
        worst = worst.max(compare_d8(
            scenario,
            wrap,
            meta,
            &recorded.data,
            played,
            &tables,
        ));
    }
    println!(
        "{scenario}: {} blocks, largest residual {worst:.1} dBFS",
        played.len()
    );
    worst
}

/// The wrap block `wrap` and the blocks its clicks reach, against the recording with the
/// prototype's clicks replaced by the corrected ones; returns the residual in dBFS.
fn compare_d8(
    scenario: &str,
    wrap: usize,
    meta: &[Value],
    recorded: &[f32],
    played: &[Played],
    tables: &(Vec<f64>, Vec<f64>),
) -> f64 {
    let span = (wrap..meta.len())
        .take_while(|&i| {
            i == wrap
                || (meta[i]["prototype_defect"] == "D8"
                    && meta[i].get("per_chunk_clicks").is_none())
        })
        .count();
    let mut expected: Vec<f64> = vec![0.0; span * BLOCK];
    add_clicks(
        &mut expected,
        &clicks_of(&meta[wrap]["per_chunk_clicks"]).expect("the corrected clicks"),
        tables,
        1.0,
    );
    add_clicks(
        &mut expected,
        &clicks_of(&meta[wrap]["clicks"]).expect("the prototype's clicks"),
        tables,
        -1.0,
    );
    let mut peak = 0.0_f64;
    for k in 0..span {
        let i = wrap + k;
        let samples = &recorded[i * 2 * BLOCK..(i + 1) * 2 * BLOCK];
        for (n, (&recorded, &played)) in samples.iter().zip(&played[i].samples).enumerate() {
            let want = f64::from(recorded) + expected[k * BLOCK + n / 2];
            peak = peak.max((f64::from(played) - want).abs());
        }
    }
    let residual = if peak == 0.0 {
        f64::NEG_INFINITY
    } else {
        20.0 * peak.log10()
    };
    println!(
        "{scenario}: D8 blocks {wrap}..{}: residual {residual:.1} dBFS against the corrected clicks",
        wrap + span
    );
    assert!(
        residual <= NULL_TEST_DBFS,
        "{scenario}: D8 blocks from {wrap}: residual {residual:.1} dBFS"
    );
    residual
}

#[test]
fn play_position_matches_the_golden_transport_area() {
    let golden = Golden::open().expect("golden data");
    let harness = replay(&golden, "play-position");
    compare(&golden, "play-position", &harness.played);
    assert_eq!(harness.control().stats().underruns, 0);
}

#[test]
fn play_range_matches_the_golden_transport_area() {
    let golden = Golden::open().expect("golden data");
    let harness = replay(&golden, "play-range");
    compare(&golden, "play-range", &harness.played);
}

/// The loop that is not aligned to beats, with the metronome: the carry, the sample-accurate
/// cut and, at both wraps, the corrected clicks.
#[test]
fn loop_metronome_matches_the_golden_transport_area() {
    let golden = Golden::open().expect("golden data");
    let harness = replay(&golden, "loop-metronome");
    compare(&golden, "loop-metronome", &harness.played);
    // The wraps the recorder marked: block 93 (324 + 188 frames) and 187 (136 + 376).
    for (block, offset) in [(93, 364), (187, 176)] {
        assert_eq!(harness.played[block].clicks, Some(vec![(offset, true)]));
    }
}

/// The recording with a count-in: the blocks of `record` (the count-in clicks forced on), and
/// the take's alignment.
#[test]
fn record_count_in_blocks_match_the_golden_transport_area() {
    let golden = Golden::open().expect("golden data");
    let harness = replay(&golden, "record-count-in");
    compare(&golden, "record-count-in", &harness.played);
}

/// `click()` of the prototype (`transport/clicks/*.f64`) as a kernel: the same
/// length, and the values within 4 ε of the click's peak. The difference is the libm's `sin` and
/// `exp`: on macOS, whose libm the oracle used, the vectors are bit for bit equal, which is
/// asserted there; glibc (the container) differs by up to 3 ulps of a value (measured
/// 2026-10-22), a fraction of an ε of the peak.
#[test]
fn click_vectors_equal_the_prototype() {
    let golden = Golden::open().expect("golden data");
    for (accent, name, peak) in [(true, "accent", 0.5), (false, "beat", 0.35)] {
        let recorded = golden
            .f64_array(&format!("transport/clicks/{name}"))
            .expect("a click vector");
        assert_eq!(recorded.header["accent"], accent);
        let computed = click(48_000, accent);
        assert_eq!(computed.len(), recorded.data.len(), "{name}: length");
        let worst_ulps = computed
            .iter()
            .zip(&recorded.data)
            .map(|(a, b)| a.to_bits().abs_diff(b.to_bits()))
            .max()
            .unwrap_or(0);
        let worst = computed
            .iter()
            .zip(&recorded.data)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        let epsilons = worst / (f64::EPSILON * peak);
        println!("click {name}: {worst_ulps} ulp, {epsilons:.2} ε of the peak from the prototype");
        assert!(epsilons <= 4.0, "{name}: {epsilons} ε");
        if cfg!(target_os = "macos") {
            assert_eq!(worst_ulps, 0, "{name}: bit-exact with the oracle's libm");
        }
    }
}
