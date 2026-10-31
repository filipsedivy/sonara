//! The renderer and the graph beyond the golden renders (TST-02, TST-03, TST-05, TST-07).
//!
//! - **Oracle fingerprints.** The recorded projects use no plugin delay compensation below the
//!   master, no pre-fader send, no pan or send automation, no isolation, no partial range, no
//!   pre-roll and no fixed tail. Each scenario below edits
//!   `golden/workflows/sidechain_preview/final.project.json` the way the prototype was given it
//!   and compares the render with the prototype's (`OfflineRenderer.render` in the host
//!   virtualenv, 2026-10-21): every statistic exactly; `Σ|x|` over the float32 samples within a
//!   relative 1e-9; the peak and the sampled frames (twelve, or 48 in `mixer-v1.json`) within
//!   1e-6 (the −120 dBFS null-test threshold).
//! - **Mixer and routing**: mute, solo, solo-safe, a
//!   soloed bus, a track routed into a bus, nested buses, pre- and post-fader sends and delay
//!   compensation through a bus, recorded from the prototype by
//!   `scripts/golden/engine_oracles.py` into `tests/fixtures/mixer-v1.json`:
//!   - the projects of `tests/audio/test_render.py` and the cases they leave out, on DC and
//!     impulse clips: the whole render equals the prototype's bit for bit, and the Python
//!     test's own check holds;
//!   - a graph rebuilt during playback as solo, solo-safe and mute change: the 256-frame ramps
//!     of the mute smoother, bit for bit;
//!   - edits of the base project above, by the fingerprints of the first point.
//! - **D3**: a disabled send and a bypassed master do not ramp in the first block.
//! - **Failure paths** with the prototype's codes, messages and warnings.
//! - **Invariants:** the stream equals the render before trimming; a pre-roll that covers the
//!   start renders exactly the slice of a full render; a looping clip's notes, bends and pedal
//!   on the Poly Synth render as the clip written out without a loop.
//!
//! The binary installs `AllocDisabler`, so the processing path of the edited project runs
//! inside `assert_no_alloc` too (RT-02).

use std::path::{Path, PathBuf};

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use serde_json::{Value, json};
use sonara_core::error::ErrorCode;
use sonara_core::model::JsonMap;
use sonara_core::model::project::Project;
use sonara_core::model::track::db_to_gain;
use sonara_core::plugins::builtin_registry;
use sonara_engine::{
    Applied, DEFAULT_BLOCK, EngineError, GraphBuilder, GraphOptions, OfflineRenderer,
    PlayableNotes, RenderGraph, RenderRequest, RenderStats, RenderedAudio, Tail, Update, apply,
    range_frames,
};
use sonara_media::MediaProvider;
use sonara_media::export::{PcmContainer, PcmFileWriter, SampleFormat, system_clock};
use sonara_project::serializer::{loads, project_from_value};
use sonara_testkit::Golden;
use tempfile::TempDir;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const BASE: &str = "workflows/sidechain_preview/final.project.json";
const KICK: &str = "trk_000000000003";
const BASS: &str = "trk_000000000005";
const BUS: &str = "trk_000000000900";

/// The base project as JSON, edited by `edit`, then loaded.
fn project(edit: impl FnOnce(&mut JsonMap)) -> Project {
    let golden = Golden::open().expect("golden data is available (set SONARA_GOLDEN)");
    let mut data = loads(&golden.bytes(BASE).expect("the base project")).expect("project JSON");
    edit(&mut data);
    project_from_value(&data).expect("the edited project loads")
}

/// A provider for projects without media: every file is missing.
fn no_media() -> MediaProvider {
    MediaProvider::new(|_| PathBuf::from("/nonexistent/sonara-media"), None)
}

fn render(project: &Project, request: &RenderRequest) -> Result<RenderedAudio, EngineError> {
    let registry = builtin_registry();
    let media = no_media();
    OfflineRenderer::new(&registry, &media).render(project, request, None)
}

fn track<'a>(data: &'a mut JsonMap, id: &str) -> &'a mut Value {
    if id == "master" {
        return data.get_mut("master").expect("the master");
    }
    data.get_mut("tracks")
        .and_then(Value::as_array_mut)
        .and_then(|tracks| tracks.iter_mut().find(|t| t["id"] == id))
        .unwrap_or_else(|| panic!("track {id}"))
}

fn lane(id: &str, device: Option<&str>, param: &str, points: &[(f64, f64, &str)]) -> Value {
    let points: Vec<Value> = points
        .iter()
        .map(|&(beat, value, curve)| json!({"beat": beat, "value": value, "curve": curve}))
        .collect();
    json!({"id": id, "target": {"device_id": device, "param": param}, "enabled": true, "points": points})
}

fn device(id: &str, plugin: &str, name: &str, params: &Value, state: &Value) -> Value {
    json!({"id": id, "plugin_id": plugin, "name": name, "enabled": true, "params": params,
           "sidechain_id": null, "preset": null, "state": state})
}

fn bus(inserts: &Value, volume_db: f64, pan: f64) -> Value {
    json!({"id": BUS, "name": "Bus", "kind": "bus", "color": null, "output_id": "master",
           "parent_id": null,
           "mixer": {"volume_db": volume_db, "pan": pan, "mute": false, "solo": false, "solo_safe": false},
           "input": {"channels": [0], "armed": false, "monitor": "auto"}, "instrument": null,
           "inserts": inserts, "sends": [], "automation": [], "clips": []})
}

fn add_track(data: &mut JsonMap, track: Value) {
    data.get_mut("tracks")
        .and_then(Value::as_array_mut)
        .expect("the track list")
        .push(track);
}

/// Plugin delay compensation (a limiter on the bass, 102 frames), a pre-fader send with a pan
/// lane on the kick, a post-fader send with a level lane on the bass, and a bus with an EQ, a
/// fader and a pan.
fn variant(data: &mut JsonMap) {
    let kick = track(data, KICK);
    kick["mixer"]["volume_db"] = json!(-3.0);
    kick["mixer"]["pan"] = json!(-0.4);
    kick["sends"] = json!([{"id": "snd_000000000901", "target_id": BUS, "level_db": -6.0,
                            "pre_fader": true, "enabled": true}]);
    kick["automation"] = json!([lane(
        "aut_000000000902",
        None,
        "pan",
        &[
            (0.0, -0.4, "linear"),
            (4.0, 0.6, "smooth"),
            (8.0, 0.0, "hold"),
            (12.0, 0.3, "linear")
        ]
    )]);
    let bass = track(data, BASS);
    bass["inserts"]
        .as_array_mut()
        .expect("inserts")
        .push(device(
            "dev_000000000904",
            "sonara.dynamics.limiter",
            "Limiter",
            &json!({}),
            &json!({}),
        ));
    bass["sends"] = json!([{"id": "snd_000000000903", "target_id": BUS, "level_db": -10.0,
                            "pre_fader": false, "enabled": true}]);
    bass["automation"] = json!([lane(
        "aut_000000000905",
        None,
        "send.snd_000000000903.level_db",
        &[(0.0, -20.0, "linear"), (16.0, 0.0, "linear")]
    )]);
    let eq = device(
        "dev_000000000906",
        "sonara.eq.parametric",
        "EQ",
        &json!({"low_gain": 4.0}),
        &json!({}),
    );
    add_track(data, bus(&json!([eq]), -2.0, 0.25));
}

/// The prototype's render, reduced: statistics, `Σ|x|` and the peak over the float32 samples,
/// and sampled frames: twelve at `i · frames / 12` in the scenarios below, 48 at
/// `i · frames / 48` in `mixer-v1.json`'s edits.
struct Fingerprint {
    frames: usize,
    tail_frames: usize,
    latency: usize,
    start_frame: i64,
    abs_sum: f64,
    peak: f64,
    samples: Vec<(usize, [f64; 2])>,
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the prototype's render is float32"
)]
fn as_f32(x: f64) -> f64 {
    f64::from(x as f32)
}

fn check(name: &str, project: &Project, request: &RenderRequest, expected: &Fingerprint) {
    let rendered = render(project, request).unwrap_or_else(|e| panic!("{name}: {e}"));
    compare_fingerprint(name, &rendered, expected);
}

/// Compares a render with the prototype's fingerprint (see the module docs for the tolerances).
fn compare_fingerprint(name: &str, rendered: &RenderedAudio, expected: &Fingerprint) {
    let stats = &rendered.stats;
    assert_eq!(stats.frames, expected.frames, "{name}: frames");
    assert_eq!(rendered.data.len(), expected.frames, "{name}: data");
    assert_eq!(
        stats.tail_frames, expected.tail_frames,
        "{name}: tail frames"
    );
    assert_eq!(stats.latency, expected.latency, "{name}: latency");
    assert_eq!(
        rendered.start_frame, expected.start_frame,
        "{name}: start frame"
    );
    assert!(stats.warnings.is_empty(), "{name}: {:?}", stats.warnings);
    let mut abs_sum = 0.0;
    let mut peak = 0.0_f64;
    for x in rendered.data.iter().flatten().map(|&x| as_f32(x)) {
        abs_sum += x.abs();
        peak = peak.max(x.abs());
    }
    let relative = (abs_sum - expected.abs_sum).abs() / expected.abs_sum;
    assert!(
        relative <= 1e-9,
        "{name}: Σ|x| {abs_sum} vs {} ({relative:e})",
        expected.abs_sum
    );
    assert!(
        (peak - expected.peak).abs() <= 1e-6,
        "{name}: peak {peak} vs {}",
        expected.peak
    );
    for &(frame, [l, r]) in &expected.samples {
        let [al, ar] = rendered.data[frame];
        assert!(
            (as_f32(al) - l).abs() <= 1e-6 && (as_f32(ar) - r).abs() <= 1e-6,
            "{name}: frame {frame}: [{al}, {ar}] vs [{l}, {r}]"
        );
    }
}

#[test]
fn a_range_without_tail_starts_with_a_chase() {
    let request = RenderRequest {
        start_beat: 8.0,
        end_beat: Some(16.0),
        tail: Tail::None,
        ..RenderRequest::default()
    };
    check(
        "range_without_tail",
        &project(|_| {}),
        &request,
        &Fingerprint {
            frames: 182_857,
            tail_frames: 0,
            latency: 102,
            start_frame: 182_857,
            abs_sum: 47_267.614_647_813_45,
            peak: 0.556_478_261_947_631_8,
            samples: vec![
                (0, [0.009_371_810_592_710_972, 0.009_371_810_592_710_972]),
                (
                    15_238,
                    [-0.244_012_415_409_088_13, -0.244_012_415_409_088_13],
                ),
                (30_476, [0.066_080_413_758_754_73, 0.066_080_413_758_754_73]),
                (
                    45_714,
                    [0.009_384_242_817_759_514, 0.009_384_242_817_759_514],
                ),
                (
                    60_952,
                    [0.010_519_074_276_089_668, 0.010_519_074_276_089_668],
                ),
                (
                    76_190,
                    [-0.022_716_756_910_085_678, -0.022_716_756_910_085_678],
                ),
                (
                    91_428,
                    [-0.030_519_580_468_535_423, -0.030_519_580_468_535_423],
                ),
                (
                    106_666,
                    [0.108_695_484_697_818_76, 0.108_695_484_697_818_76],
                ),
                (
                    121_904,
                    [0.063_902_921_974_658_97, 0.063_902_921_974_658_97],
                ),
                (
                    137_142,
                    [-0.029_332_971_200_346_947, -0.029_332_971_200_346_947],
                ),
                (
                    152_380,
                    [0.262_308_150_529_861_45, 0.262_308_150_529_861_45],
                ),
                (
                    167_618,
                    [-0.070_537_999_272_346_5, -0.070_537_999_272_346_5],
                ),
            ],
        },
    );
}

#[test]
fn a_preroll_and_a_tail_in_seconds() {
    let request = RenderRequest {
        start_beat: 8.0,
        end_beat: Some(12.0),
        preroll_seconds: 0.5,
        tail: Tail::Seconds(1.0),
        ..RenderRequest::default()
    };
    check(
        "preroll_and_seconds_tail",
        &project(|_| {}),
        &request,
        &Fingerprint {
            frames: 139_429,
            tail_frames: 48_000,
            latency: 102,
            start_frame: 182_857,
            abs_sum: 36_237.919_460_873_156,
            peak: 0.556_478_261_947_631_8,
            samples: vec![
                (0, [0.009_384_242_817_759_514, 0.009_384_242_817_759_514]),
                (11_619, [0.207_703_247_666_358_95, 0.207_703_247_666_358_95]),
                (23_238, [0.076_689_563_691_616_06, 0.076_689_563_691_616_06]),
                (34_857, [0.220_301_434_397_697_45, 0.220_301_434_397_697_45]),
                (
                    46_476,
                    [-0.032_647_311_687_469_48, -0.032_647_311_687_469_48],
                ),
                (58_095, [-0.205_182_254_314_422_6, -0.205_182_254_314_422_6]),
                (
                    69_714,
                    [-0.156_103_670_597_076_42, -0.156_103_670_597_076_42],
                ),
                (
                    81_333,
                    [-0.155_588_403_344_154_36, -0.155_588_403_344_154_36],
                ),
                (92_952, [0.143_097_326_159_477_23, 0.143_097_326_159_477_23]),
                (
                    104_571,
                    [-0.247_578_039_765_357_97, -0.247_578_039_765_357_97],
                ),
                (
                    116_190,
                    [-0.121_346_175_670_623_78, -0.121_346_175_670_623_78],
                ),
                (
                    127_809,
                    [-0.234_781_384_468_078_6, -0.234_781_384_468_078_6],
                ),
            ],
        },
    );
}

#[test]
fn an_isolated_track_bypassing_the_master() {
    let request = RenderRequest {
        target: Some(BASS.to_owned()),
        bypass_master: true,
        end_beat: Some(8.0),
        tail: Tail::None,
        ..RenderRequest::default()
    };
    check(
        "isolated_track_bypassing_master",
        &project(|_| {}),
        &request,
        &Fingerprint {
            frames: 182_857,
            tail_frames: 0,
            latency: 0,
            start_frame: 0,
            abs_sum: 41_148.246_651_887_69,
            peak: 0.360_654_503_107_070_9,
            samples: vec![
                (0, [0.0, 0.0]),
                (15_238, [0.215_910_255_908_966_06, 0.215_910_255_908_966_06]),
                (
                    30_476,
                    [-0.046_481_680_124_998_09, -0.046_481_680_124_998_09],
                ),
                (
                    45_714,
                    [-1.071_924_014_200_25e-22, -1.071_924_014_200_25e-22],
                ),
                (60_952, [0.123_791_024_088_859_56, 0.123_791_024_088_859_56]),
                (76_190, [0.041_733_972_728_252_41, 0.041_733_972_728_252_41]),
                (
                    91_428,
                    [0.010_570_830_665_528_774, 0.010_570_830_665_528_774],
                ),
                (
                    106_666,
                    [-0.171_264_916_658_401_5, -0.171_264_916_658_401_5],
                ),
                (
                    121_904,
                    [-0.052_274_540_066_719_055, -0.052_274_540_066_719_055],
                ),
                (
                    137_142,
                    [-0.002_679_878_380_149_603, -0.002_679_878_380_149_603],
                ),
                (
                    152_380,
                    [-0.245_980_829_000_473_02, -0.245_980_829_000_473_02],
                ),
                (
                    167_618,
                    [-0.018_708_737_567_067_146, -0.018_708_737_567_067_146],
                ),
            ],
        },
    );
}

#[test]
fn compensation_pre_fader_sends_and_mixer_lanes() {
    check(
        "compensation_sends_and_lanes",
        &project(variant),
        &RenderRequest::default(),
        &Fingerprint {
            frames: 739_890,
            tail_frames: 8_461,
            latency: 204,
            start_frame: 0,
            abs_sum: 282_191.310_701_404_47,
            peak: 0.881_048_858_165_741,
            samples: vec![
                (0, [0.009_572_393_260_896_206, 0.007_998_691_871_762_276]),
                (
                    61_657,
                    [-0.343_314_349_651_336_67, -0.352_863_222_360_610_96],
                ),
                (
                    123_315,
                    [0.011_724_599_637_091_16, 0.009_391_904_808_580_875],
                ),
                (
                    184_972,
                    [0.116_520_680_487_155_91, 0.132_950_708_270_072_94],
                ),
                (246_630, [-0.269_625_991_582_870_5, -0.297_210_156_917_572]),
                (
                    308_287,
                    [-0.053_803_551_942_110_06, -0.061_251_323_670_148_85],
                ),
                (
                    369_945,
                    [0.160_186_842_083_930_97, 0.186_702_802_777_290_34],
                ),
                (
                    431_602,
                    [-0.355_193_912_982_940_7, -0.389_495_760_202_407_84],
                ),
                (
                    493_260,
                    [0.108_795_560_896_396_64, 0.115_619_920_194_149_02],
                ),
                (
                    554_917,
                    [-0.175_749_585_032_463_07, -0.209_309_279_918_670_65],
                ),
                (
                    616_575,
                    [-0.054_109_428_077_936_17, -0.062_586_382_031_440_73],
                ),
                (
                    678_232,
                    [0.237_904_056_906_700_13, 0.278_502_583_503_723_14],
                ),
            ],
        },
    );
}

#[test]
fn an_isolated_bus_hears_only_its_sends() {
    let request = RenderRequest {
        target: Some(BUS.to_owned()),
        end_beat: Some(8.0),
        tail: Tail::None,
        ..RenderRequest::default()
    };
    check(
        "isolated_bus",
        &project(variant),
        &request,
        &Fingerprint {
            frames: 182_857,
            tail_frames: 0,
            latency: 204,
            start_frame: 0,
            abs_sum: 11_830.169_800_509_615,
            peak: 0.179_897_025_227_546_7,
            samples: vec![
                (0, [0.002_937_659_155_577_421, 0.003_738_930_914_551_019_7]),
                (
                    15_238,
                    [0.016_187_276_691_198_35, 0.020_602_494_478_225_708],
                ),
                (
                    30_476,
                    [0.002_811_557_147_651_910_8, 0.003_578_433_534_130_454],
                ),
                (
                    45_714,
                    [0.003_054_904_052_987_694_7, 0.003_888_155_333_697_796],
                ),
                (60_952, [0.025_101_050_734_519_96, 0.031_947_575_509_548_19]),
                (
                    76_190,
                    [0.019_105_385_988_950_73, 0.024_316_545_575_857_162],
                ),
                (
                    91_428,
                    [-0.000_141_494_398_121_722_04, -0.000_180_088_216_438_889_5],
                ),
                (
                    106_666,
                    [-0.031_580_243_259_668_35, -0.040_194_023_400_545_12],
                ),
                (
                    121_904,
                    [-0.003_991_346_340_626_478, -0.005_080_020_055_174_828],
                ),
                (
                    137_142,
                    [-0.002_833_517_733_961_343_8, -0.003_606_383_921_578_526_5],
                ),
                (152_380, [-0.043_503_537_774_086, -0.055_369_500_070_810_32]),
                (
                    167_618,
                    [0.013_238_286_599_516_869, 0.016_849_143_430_590_63],
                ),
            ],
        },
    );
}

#[test]
fn an_isolated_send_source_keeps_its_send() {
    let request = RenderRequest {
        target: Some(KICK.to_owned()),
        bypass_master: true,
        end_beat: Some(8.0),
        tail: Tail::None,
        ..RenderRequest::default()
    };
    check(
        "isolated_send_source",
        &project(variant),
        &request,
        &Fingerprint {
            frames: 182_857,
            tail_frames: 0,
            latency: 102,
            start_frame: 0,
            abs_sum: 13_914.516_832_262_194,
            peak: 0.372_733_712_196_350_1,
            samples: vec![
                (0, [0.009_572_393_260_896_206, 0.007_998_691_871_762_276]),
                (
                    15_238,
                    [0.001_044_872_449_710_965_2, 0.000_982_738_099_992_275_2],
                ),
                (
                    30_476,
                    [0.028_310_319_408_774_376, 0.030_089_581_385_254_86],
                ),
                (
                    45_714,
                    [0.009_031_669_236_719_608, 0.010_372_516_699_135_303],
                ),
                (
                    60_952,
                    [0.000_875_802_244_991_064_1, 0.001_129_159_587_435_424_3],
                ),
                (
                    76_190,
                    [0.021_190_093_830_227_852, 0.031_067_220_494_151_115],
                ),
                (
                    91_428,
                    [-3.971_147_634_729_277e-6, -1.148_575_734_077_894_6e-6],
                ),
                (
                    106_666,
                    [0.000_650_689_762_551_337_5, 0.001_111_717_778_258_025_6],
                ),
                (
                    121_904,
                    [0.020_741_175_860_166_55, 0.030_859_874_561_429_024],
                ),
                (
                    137_142,
                    [-2.264_264_139_739_68e-6, -2.397_292_064_415_523_8e-6],
                ),
                (
                    152_380,
                    [0.000_938_774_202_950_298_8, 0.001_111_717_778_258_025_6],
                ),
                (
                    167_618,
                    [0.027_545_582_503_080_368, 0.030_859_874_561_429_024],
                ),
            ],
        },
    );
}

/// RT-02 for the paths the golden projects do not use: compensation lines, pre-fader sends,
/// pan and send lanes, a bus with inserts.
#[test]
fn the_edited_graph_allocates_nothing() {
    let project = project(variant);
    let registry = builtin_registry();
    let media = no_media();
    let mut graph = RenderGraph::new(
        &project,
        &registry,
        &media,
        RenderRequest::default().block_size,
        &GraphOptions::default(),
    )
    .expect("the graph builds");
    graph.reset();
    assert_eq!(graph.latency(), 204);
    let block = i64::try_from(graph.block_size()).expect("a block size");
    let mut frame = 0;
    while frame < 400_000 {
        graph.read_media(frame).expect("no media to read");
        assert_no_alloc(|| graph.process(frame, true));
        frame += block;
    }
}

/// The tracks of the D3 checks: the base project with an empty bus.
fn with_bus(data: &mut JsonMap) {
    add_track(data, bus(&json!([]), 0.0, 0.0));
}

fn two_beats() -> RenderRequest {
    RenderRequest {
        end_beat: Some(2.0),
        tail: Tail::None,
        ..RenderRequest::default()
    }
}

/// A disabled send adds nothing, from the first frame on; the prototype ramps it from its
/// level to 0 over the first 256 frames.
#[test]
fn a_disabled_send_is_silent_from_the_first_frame() {
    let send = |enabled: bool| {
        move |data: &mut JsonMap| {
            with_bus(data);
            track(data, KICK)["sends"] = json!([{"id": "snd_000000000901", "target_id": BUS,
                "level_db": 0.0, "pre_fader": false, "enabled": enabled}]);
        }
    };
    let without = render(&project(with_bus), &two_beats()).expect("renders");
    let disabled = render(&project(send(false)), &two_beats()).expect("renders");
    let enabled = render(&project(send(true)), &two_beats()).expect("renders");
    assert_eq!(disabled.data, without.data);
    assert_ne!(enabled.data, without.data, "the enabled send is audible");
}

/// A bypassed master is at unity gain and centre pan from the first frame; the prototype
/// ramps from its fader to unity over the first 256 frames.
#[test]
fn a_bypassed_master_is_at_unity_from_the_first_frame() {
    let bypassed = project(|data| {
        let master = track(data, "master");
        master["mixer"]["volume_db"] = json!(-6.0);
        master["mixer"]["pan"] = json!(0.3);
    });
    let plain = project(|data| {
        track(data, "master")["inserts"] = json!([]);
    });
    let request = RenderRequest {
        bypass_master: true,
        ..two_beats()
    };
    let bypassed = render(&bypassed, &request).expect("renders");
    let plain = render(&plain, &two_beats()).expect("renders");
    assert_eq!(bypassed.stats.latency, 0);
    assert_eq!(bypassed.data, plain.data);
}

/// A pre-roll that reaches back to frame 0 renders on the same block grid as a full render, so
/// its output is exactly the full render's slice.
#[test]
fn a_full_preroll_renders_the_slice_of_a_full_render() {
    let project = project(|_| {});
    let full = render(
        &project,
        &RenderRequest {
            tail: Tail::None,
            ..RenderRequest::default()
        },
    )
    .expect("renders");
    let part = render(
        &project,
        &RenderRequest {
            start_beat: 8.0,
            end_beat: Some(12.0),
            preroll_seconds: 60.0,
            tail: Tail::None,
            ..RenderRequest::default()
        },
    )
    .expect("renders");
    let start = usize::try_from(part.start_frame).expect("a frame");
    assert_eq!(part.data, full.data[start..start + part.data.len()]);
}

/// The block stream is the render before the auto tail's trim; a block that crosses the range
/// end comes in two parts.
#[test]
fn the_stream_is_the_untrimmed_render() {
    let project = project(|_| {});
    let registry = builtin_registry();
    let media = no_media();
    let renderer = OfflineRenderer::new(&registry, &media);
    for tail in [Tail::Seconds(0.3), Tail::Auto] {
        let request = RenderRequest {
            end_beat: Some(4.0),
            tail,
            ..RenderRequest::default()
        };
        let mut stream = renderer.stream(&project, &request).expect("starts");
        let mut data: Vec<[f64; 2]> = Vec::new();
        let mut sizes = Vec::new();
        while let Some(block) = stream.next_block(None).expect("renders") {
            sizes.push(block.len());
            data.extend_from_slice(block);
        }
        assert_eq!(stream.progress(), 1.0);
        let (start, end) = range_frames(&project, &request).expect("a range");
        let main = usize::try_from(end - start).expect("frames");
        let stats: RenderStats = stream.into_stats();
        assert_eq!(stats.frames, data.len());
        assert_eq!(stats.frames, main + stats.tail_frames);
        assert!(sizes.iter().all(|&n| (1..=512).contains(&n)), "{sizes:?}");
        let trimmed = renderer.render(&project, &request, None).expect("renders");
        if tail == Tail::Auto {
            assert!(trimmed.data.len() <= data.len());
            assert_eq!(trimmed.data, data[..trimmed.data.len()]);
            assert_eq!(trimmed.stats.frames, trimmed.data.len());
        } else {
            assert_eq!(stats.tail_frames, 14_400);
            assert_eq!(trimmed.data, data);
            assert_eq!(trimmed.stats, stats);
        }
    }
}

fn domain_error(result: Result<RenderedAudio, EngineError>) -> (ErrorCode, String, Value) {
    match result {
        Err(EngineError::Domain(err)) => (err.code, err.message, Value::Object(err.details)),
        Err(other) => panic!("not a domain error: {other}"),
        Ok(_) => panic!("rendered"),
    }
}

/// The prototype's errors (host virtualenv, 2026-10-21).
#[test]
fn requests_fail_like_the_prototype() {
    let project = project(|_| {});
    let (code, message, details) = domain_error(render(
        &project,
        &RenderRequest {
            start_beat: 8.0,
            end_beat: Some(8.0),
            ..RenderRequest::default()
        },
    ));
    assert_eq!(code, ErrorCode::InvalidArgument);
    assert_eq!(
        message,
        "nothing to render: the range is empty (add content or set an end position)"
    );
    assert_eq!(details, json!({"start_beat": 8.0, "end_beat": 8.0}));
    let (code, message, _) = domain_error(render(
        &project,
        &RenderRequest {
            max_seconds: 10.0,
            ..RenderRequest::default()
        },
    ));
    assert_eq!(code, ErrorCode::LimitExceeded);
    assert_eq!(message, "render length 15s exceeds the limit of 10s");
    let (code, message, details) = domain_error(render(
        &project,
        &RenderRequest {
            target: Some("trk_missing".to_owned()),
            ..RenderRequest::default()
        },
    ));
    assert_eq!(code, ErrorCode::NotFound);
    assert_eq!(message, "track 'trk_missing' not found");
    assert_eq!(details, json!({"kind": "track", "id": "trk_missing"}));
    let registry = builtin_registry();
    let media = no_media();
    let cancelled = OfflineRenderer::new(&registry, &media).render(
        &project,
        &RenderRequest::default(),
        Some(&|| true),
    );
    let (code, message, _) = domain_error(cancelled);
    assert_eq!(code, ErrorCode::Cancelled);
    assert_eq!(message, "render cancelled");
}

/// An automation lane on a parameter its plugin does not have fails the render, as the
/// prototype's `params_at` does.
#[test]
fn a_lane_on_an_unknown_parameter_fails() {
    let project = project(|data| {
        track(data, KICK)["automation"] = json!([lane(
            "aut_000000000930",
            Some("dev_000000000004"),
            "nope",
            &[(0.0, 1.0, "linear")]
        )]);
    });
    let err = render(&project, &two_beats()).expect_err("fails");
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
    assert_eq!(
        err.to_string(),
        "[INVALID_ARGUMENT] Sonara Drum Kit has no parameter 'nope'"
    );
}

/// Unavailable plugins and impulse responses are bypassed and missing media plays silence,
/// with the prototype's warnings (host virtualenv, 2026-10-21).
#[test]
fn missing_plugins_and_media_are_warnings() {
    let project = project(|data| {
        track(data, KICK)["inserts"] = json!([
            device(
                "dev_000000000910",
                "vst3.serum",
                "Serum",
                &json!({}),
                &json!({})
            ),
            device(
                "dev_000000000911",
                "sonara.convolver",
                "Convolution Reverb",
                &json!({}),
                &json!({"ir_asset_id": "med_000000000999"})
            ),
        ]);
        data.insert(
            "assets".to_owned(),
            json!([{
            "id": "med_000000000920", "kind": "audio", "name": "gone.wav",
            "path": "media/audio/med_000000000920-gone.wav", "location": "embedded",
            "size_bytes": 10, "sha256": null, "sample_rate": 48000, "channels": 2,
            "frames": 4800, "format": "WAV", "subtype": "PCM_16", "original_path": null,
            "relative_hint": null, "metadata": {}}]),
        );
        let clip = |id: &str, asset: &str, start: f64| {
            json!({
            "type": "audio", "id": id, "name": "gone", "asset_id": asset, "start": start,
            "length": 1.0, "source_offset": 0, "gain_db": 0.0,
            "fade_in": {"seconds": 0.0, "curve": "linear"},
            "fade_out": {"seconds": 0.0, "curve": "linear"}, "muted": false, "color": null})
        };
        add_track(
            data,
            json!({
            "id": "trk_000000000921", "name": "Audio", "kind": "audio", "color": null,
            "output_id": "master", "parent_id": null,
            "mixer": {"volume_db": 0.0, "pan": 0.0, "mute": false, "solo": false, "solo_safe": false},
            "input": {"channels": [0], "armed": false, "monitor": "auto"}, "instrument": null,
            "inserts": [], "sends": [], "automation": [],
            "clips": [clip("clp_000000000922", "med_000000000920", 0.0),
                      clip("clp_000000000923", "med_000000000924", 1.0)]}),
        );
    });
    let request = RenderRequest {
        end_beat: Some(4.0),
        tail: Tail::None,
        ..RenderRequest::default()
    };
    let rendered = render(&project, &request).expect("renders");
    assert_eq!(rendered.stats.frames, 91_429);
    assert_eq!(
        rendered.stats.warnings,
        [
            "plugin 'vst3.serum' on 'Kick' is unavailable; bypassed",
            "impulse response 'med_000000000999' for 'Convolution Reverb' is missing",
            "2 media file(s) missing; rendering silence for them",
        ]
    );
    assert_eq!(
        rendered.stats.missing_media,
        ["med_000000000920", "med_000000000924"]
    );
}

/// A block processed without its clip audio is counted, not handled (RT-12).
#[test]
fn blocks_without_their_media_are_counted() {
    let project = project(|data| {
        data.insert("assets".to_owned(), json!([]));
        add_track(
            data,
            json!({
            "id": "trk_000000000921", "name": "Audio", "kind": "audio", "color": null,
            "output_id": "master", "parent_id": null,
            "mixer": {"volume_db": 0.0, "pan": 0.0, "mute": false, "solo": false, "solo_safe": false},
            "input": {"channels": [0], "armed": false, "monitor": "auto"}, "instrument": null,
            "inserts": [], "sends": [], "automation": [], "clips": []}),
        );
    });
    let registry = builtin_registry();
    let media = no_media();
    let mut graph = RenderGraph::new(
        &project,
        &registry,
        &media,
        RenderRequest::default().block_size,
        &GraphOptions::default(),
    )
    .expect("the graph builds");
    graph.reset();
    graph.read_media(0).expect("reads");
    graph.process(0, true);
    assert_eq!(graph.blocks_without_media(), 0);
    graph.process(512, true);
    assert_eq!(graph.blocks_without_media(), 1);
}

/// An offline render prepares each instrument for the notes of its schedule only
/// ([`PlayableNotes::Scheduled`]: the drum kit synthesizes only the sounds that play). The
/// output is bit-identical to a graph prepared for every note, here with five drum sounds,
/// including the hi-hat choke and a chase into the loop.
#[test]
fn scheduled_notes_render_like_every_note() {
    let project = project(|data| {
        track(data, KICK)["clips"][0]["notes"] = json!([
            {"id": "n_000000000007", "pitch": 36, "start": 0.0, "length": 0.25, "velocity": 120, "channel": 0},
            {"id": "n_000000000931", "pitch": 49, "start": 0.125, "length": 0.25, "velocity": 90, "channel": 0},
            {"id": "n_000000000932", "pitch": 46, "start": 0.25, "length": 0.25, "velocity": 100, "channel": 0},
            {"id": "n_000000000933", "pitch": 42, "start": 0.5, "length": 0.25, "velocity": 80, "channel": 0},
            {"id": "n_000000000934", "pitch": 38, "start": 0.75, "length": 0.25, "velocity": 110, "channel": 0}
        ]);
    });
    let registry = builtin_registry();
    let media = no_media();
    let render = |playable_notes| {
        let options = GraphOptions {
            playable_notes,
            ..GraphOptions::default()
        };
        let mut graph = RenderGraph::new(
            &project,
            &registry,
            &media,
            RenderRequest::default().block_size,
            &options,
        )
        .expect("the graph builds");
        graph.reset();
        let mut bits = Vec::new();
        // Two seconds from the start, then a jump into the loop, which chases the notes.
        for frame in (0..96_000)
            .step_by(512)
            .chain((150_037..198_037).step_by(512))
        {
            graph.read_media(frame).expect("no media to read");
            let out = graph.process(frame, true);
            bits.extend(out.as_flattened().iter().map(|v| v.to_bits()));
        }
        bits
    };
    let every = render(PlayableNotes::All);
    let scheduled = render(PlayableNotes::Scheduled);
    assert!(
        every.iter().any(|&b| f64::from_bits(b).abs() > 0.1),
        "the drums play"
    );
    assert_eq!(every, scheduled, "bit-identical");
}

/// The base project with the bass's clip (the Poly Synth) replaced by one of 8 beats: in each
/// 2-beat pass, C2 from beat 0 and E2 from beat 1, half a beat each, the pitch bend up to 4096
/// at beat 0.5 and down to -2048 at beat 1.5, and the sustain pedal down at beat 1.25 and up
/// at beat 1.75. As a clip that loops every 2 beats, or (`unrolled`) written out without a
/// loop; `with_controls: false` leaves the bends and the pedal out.
fn bass_loop(unrolled: bool, with_controls: bool) -> Project {
    let passes = if unrolled { 0..4 } else { 0..1 };
    let mut notes = Vec::new();
    let mut controls = Vec::new();
    for pass in passes {
        let base = 2.0 * f64::from(pass);
        for (id, pitch, beat) in [(1, 36, 0.0), (2, 40, 1.0)] {
            notes.push(
                json!({"id": format!("n_00000000{id}00{pass}"), "pitch": pitch,
                "start": base + beat, "length": 0.5, "velocity": 100, "channel": 0}),
            );
        }
        if with_controls {
            controls.extend([
                json!({"beat": base + 0.5, "kind": "pitch_bend", "value": 4096, "number": 0,
                       "channel": 0}),
                json!({"beat": base + 1.25, "kind": "cc", "value": 127, "number": 64,
                       "channel": 0}),
                json!({"beat": base + 1.5, "kind": "pitch_bend", "value": -2048, "number": 0,
                       "channel": 0}),
                json!({"beat": base + 1.75, "kind": "cc", "value": 0, "number": 64,
                       "channel": 0}),
            ]);
        }
    }
    let loop_length = (!unrolled).then_some(2.0);
    project(|data| {
        track(data, BASS)["clips"] = json!([{"type": "midi", "id": "clp_000000000073",
            "name": "Bass", "start": 0.0, "length": 8.0, "content_offset": 0.0,
            "loop_length": loop_length, "muted": false, "color": null, "notes": notes,
            "controls": controls}]);
    })
}

/// A looping clip plays its controller events on every pass, as its notes, also on the Poly
/// Synth: the bends and the pedal repeat with the loop, and the bend one pass leaves holds into
/// the next. The oracle is the same clip written out without a loop, bit for bit, played from
/// the start and from a later start in the third pass. The prototype dropped the controller
/// events of looping clips (TST-06 pins the change); no golden project has a clip with
/// controller events, so the golden renders stay the same (`tests/golden_render.rs`, GLD-01).
/// Every block is processed inside `assert_no_alloc` (RT-02).
#[test]
fn a_looping_clip_bends_the_poly_synth_on_every_pass() {
    let registry = builtin_registry();
    let media = no_media();
    // The blocks from frame `from` up to 200 000 (8 beats at 126 bpm are 182 857 frames).
    let render = |project: &Project, from: i64| {
        let mut graph = RenderGraph::new(
            project,
            &registry,
            &media,
            RenderRequest::default().block_size,
            &GraphOptions::default(),
        )
        .expect("the graph builds");
        graph.reset();
        let block = i64::try_from(graph.block_size()).expect("a block size");
        let mut out = Vec::new();
        let mut frame = from;
        while frame < 200_000 {
            graph.read_media(frame).expect("no media to read");
            out.extend_from_slice(assert_no_alloc(|| graph.process(frame, true)));
            frame += block;
        }
        out
    };
    let heard = render(&bass_loop(true, true), 0);
    let looped = render(&bass_loop(false, true), 0);
    assert!(looped == heard, "the looping clip plays as written out");
    let without = render(&bass_loop(false, false), 0);
    assert!(looped != without, "the bends and the pedal are heard");
    // Frame 100 352 lies early in the third pass (beat 4.39), before its first bend.
    let from = 196 * 512;
    let later = render(&bass_loop(false, true), from);
    let start = usize::try_from(from).expect("a frame");
    assert!(
        later == render(&bass_loop(true, true), from),
        "a start in the third pass plays as written out"
    );
    assert_eq!(later.len(), heard.len() - start);
}

// -- Mixer and routing: mute, solo, solo-safe, buses -----

/// `tests/fixtures/mixer-v1.json`, recorded from the prototype by
/// `scripts/golden/engine_oracles.py` (see its docs).
fn mixer_fixture() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mixer-v1.json");
    let raw = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_slice(&raw).expect("the fixture is JSON")
}

fn index(value: &Value) -> usize {
    value
        .as_u64()
        .and_then(|v| usize::try_from(v).ok())
        .unwrap_or_else(|| panic!("{value} is not an index"))
}

fn float(value: &Value) -> f64 {
    value
        .as_f64()
        .unwrap_or_else(|| panic!("{value} is not a number"))
}

fn text(value: &Value) -> &str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("{value} is not a string"))
}

fn items(value: &Value) -> &[Value] {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{value} is not an array"))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the assets are float32 WAVs, as `soundfile` writes them with subtype FLOAT"
)]
fn to_f32(x: f64) -> f32 {
    x as f32
}

/// The fixture's media in a temporary workspace: float32 WAVs of the recorded DC levels and
/// impulses, as the prototype's `Rig.asset` writes them, and a provider that resolves an asset
/// by its workspace path (embedded media, as `asset_path` does).
struct Workspace {
    _dir: TempDir,
    media: MediaProvider,
}

fn workspace(fixture: &Value) -> Workspace {
    let dir = TempDir::new().expect("a temporary workspace");
    for asset in items(&fixture["media"]) {
        let (frames, channels) = (index(&asset["frames"]), index(&asset["channels"]));
        let mut samples = vec![0.0; frames * channels];
        match text(&asset["signal"]) {
            "dc" => samples.fill(float(&asset["level"])),
            "impulse" => {
                let at = index(&asset["at"]) * channels;
                samples[at..at + channels].fill(float(&asset["amp"]));
            }
            other => panic!("unknown signal {other}"),
        }
        let samples: Vec<f32> = samples.into_iter().map(to_f32).collect();
        let bytes: Vec<u8> = samples.iter().flat_map(|x| x.to_le_bytes()).collect();
        assert_eq!(
            sonara_testkit::sha256_hex(&bytes),
            text(&asset["samples_sha256"]),
            "{}: the prototype's samples",
            asset["path"]
        );
        let path = dir.path().join(text(&asset["path"]));
        std::fs::create_dir_all(path.parent().expect("a media folder")).expect("the media folder");
        let mut writer = PcmFileWriter::create(
            &path,
            PcmContainer::Wav,
            SampleFormat::Float,
            u16::try_from(channels).expect("a channel count"),
            48_000,
            &[],
            system_clock(),
        )
        .expect("the WAV is created");
        writer.write_f32(&samples).expect("the samples are written");
        writer.finish().expect("the WAV is finished");
    }
    let root = dir.path().to_owned();
    let media = MediaProvider::new(move |asset| root.join(&asset.path), None);
    Workspace { _dir: dir, media }
}

fn recorded_project(document: &Value) -> Project {
    project_from_value(document.as_object().expect("a project document"))
        .expect("the recorded project loads")
}

fn recorded_request(request: &Value) -> RenderRequest {
    let tail = match &request["tail"] {
        Value::String(tail) if tail == "none" => Tail::None,
        Value::String(tail) if tail == "auto" => Tail::Auto,
        seconds => Tail::Seconds(float(seconds)),
    };
    RenderRequest {
        start_beat: float(&request["start_beat"]),
        end_beat: request["end_beat"].as_f64(),
        tail,
        target: request["target"].as_str().map(str::to_owned),
        bypass_master: request["bypass_master"] == true,
        preroll_seconds: float(&request["preroll_seconds"]),
        ..RenderRequest::default()
    }
}

/// One channel of a run-length encoded render (`[value, count]` runs).
fn decode(runs: &Value) -> Vec<f64> {
    let mut out = Vec::new();
    for run in items(runs) {
        let run = items(run);
        out.extend(std::iter::repeat_n(float(&run[0]), index(&run[1])));
    }
    out
}

/// The whole output against the prototype's, frame for frame and bit for bit: these paths are
/// gains, sums, pans and delays in the prototype's order of operations.
fn compare_output(name: &str, data: &[[f64; 2]], recorded: &Value) {
    let channels = items(recorded);
    for (c, runs) in channels.iter().enumerate() {
        let expected = decode(runs);
        assert_eq!(data.len(), expected.len(), "{name}: frames");
        let mismatch = data
            .iter()
            .zip(&expected)
            .position(|(frame, e)| frame[c].to_bits() != e.to_bits());
        if let Some(i) = mismatch {
            panic!(
                "{name}: channel {c}, frame {i}: {} vs the prototype's {}",
                data[i][c], expected[i]
            );
        }
    }
}

/// Every recorded render of a mute, solo, solo-safe, bus or send configuration: the projects of
/// `tests/audio/test_render.py` (`test_mute_and_solo`, `test_soloing_a_bus_keeps_its_sources_audible`,
/// `test_pre_and_post_fader_sends`, `test_nested_buses_multiply_gains`,
/// `test_pdc_through_bus_and_sends`) and the cases they leave out. Each render equals the
/// prototype's bit for bit, and the Python test's own check holds too (TST-10: two oracles).
#[test]
fn mixer_renders_match_the_prototype() {
    let fixture = mixer_fixture();
    let workspace = workspace(&fixture);
    let registry = builtin_registry();
    let offline = OfflineRenderer::new(&registry, &workspace.media);
    let scenarios = items(&fixture["mixer"]);
    assert_eq!(scenarios.len(), 19, "the recorded scenarios");
    for scenario in scenarios {
        let name = text(&scenario["name"]);
        let project = recorded_project(&scenario["project"]);
        let request = recorded_request(&scenario["request"]);
        let rendered = offline
            .render(&project, &request, None)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let stats = &rendered.stats;
        assert_eq!(stats.frames, index(&scenario["frames"]), "{name}: frames");
        assert_eq!(
            stats.tail_frames,
            index(&scenario["tail_frames"]),
            "{name}: tail"
        );
        assert_eq!(
            stats.latency,
            index(&scenario["latency"]),
            "{name}: latency"
        );
        assert_eq!(
            rendered.start_frame,
            scenario["start_frame"].as_i64().expect("a start frame"),
            "{name}: start frame"
        );
        assert_eq!(
            json!(stats.warnings),
            scenario["warnings"],
            "{name}: warnings"
        );
        compare_output(name, &rendered.data, &scenario["output"]);
        // The Python test's check: `pytest.approx(rel=1e-6)` at one frame, and where it lists
        // them, the only frames above 1e-9.
        let expect = &scenario["expect"];
        let frame = index(&expect["frame"]);
        for (c, value) in items(&expect["value"]).iter().enumerate() {
            let (actual, value) = (rendered.data[frame][c], float(value));
            assert!(
                (actual - value).abs() <= 1e-6 * value.abs(),
                "{name}: frame {frame}, channel {c}: {actual} vs {value}"
            );
        }
        if let Some(nonzero) = expect.get("nonzero") {
            let loud: Vec<usize> = (0..rendered.data.len())
                .filter(|&i| rendered.data[i][0].abs() > 1e-9)
                .collect();
            let nonzero: Vec<usize> = items(nonzero).iter().map(index).collect();
            assert_eq!(loud, nonzero, "{name}: the impulse frames");
        }
    }
}

/// Solo, solo-safe and mute changed while a graph plays: each edit rebuilds the graph, which
/// adopts the playing one (the prototype's `RenderGraph(..., previous=graph)`), so the mute
/// smoother ramps over 256 frames (`MUTE_RAMP`) from its old value, a post-fader send ramps
/// with its source, and a pre-fader send follows the smoother's value at the block's end
/// (`gain * node.mute.current`). Every block equals the prototype's bit for bit; the edits
/// and the blocks they land on are the recording's.
#[test]
fn mute_and_solo_ramp_like_the_prototype_during_playback() {
    let fixture = mixer_fixture();
    let workspace = workspace(&fixture);
    let live = &fixture["live"];
    let registry = builtin_registry();
    let block_size = index(&live["block_size"]);
    assert_eq!(block_size, DEFAULT_BLOCK.get());
    let rebuilds: Vec<(usize, Project)> = items(&live["rebuilds"])
        .iter()
        .map(|r| (index(&r["block"]), recorded_project(&r["project"])))
        .collect();
    let mut builder = GraphBuilder::new(DEFAULT_BLOCK);
    let mut current: Option<Box<RenderGraph>> = None;
    let mut data: Vec<[f64; 2]> = Vec::new();
    for block in 0..index(&live["blocks"]) {
        for (_, project) in rebuilds.iter().filter(|(at, _)| *at == block) {
            let next = if current.is_none() {
                builder.fresh(project, &registry, &workspace.media)
            } else {
                builder.rebuild(project, &registry, &workspace.media)
            }
            .expect("the graph builds");
            assert!(next.warnings().is_empty(), "{:?}", next.warnings());
            let mut update = Update::Graph(Some(next));
            assert_eq!(
                assert_no_alloc(|| apply(&mut current, &mut update)),
                Applied::Swapped
            );
        }
        let graph = current.as_mut().expect("a graph plays");
        let frame = i64::try_from(block * block_size).expect("a frame");
        graph.read_media(frame).expect("the media reads");
        data.extend_from_slice(assert_no_alloc(|| graph.process(frame, true)));
    }
    compare_output("live", &data, &live["output"]);
    // What the recording shows: after block 4 (A soloed), B and its send leave over 256
    // frames, the last of which is the target; after block 12 (A muted), A's pre-fader send
    // is gone from the block's first frame while A itself ramps out.
    let start = 4 * block_size;
    assert!(data[start - 1][0] > data[start][0] && data[start][0] > data[start + 254][0]);
    assert_eq!(
        data[start + 255][0].to_bits(),
        data[start + 511][0].to_bits()
    );
    let start = 12 * block_size;
    let pre_fader_send = 0.1 * db_to_gain(-6.0);
    assert!((data[start - 1][0] - data[start][0] - pre_fader_send).abs() < 2e-3);
}

/// The base project of the fingerprints above edited by the recorded operations: mute and solo
/// with the bass compressor keyed by the kick (whose pre-fader tap stays live when the kick is
/// muted or not soloed), solo-safe, a track into a bus with inserts, a soloed bus, nested
/// buses, and a limiter on a bus fed directly and by a send (R2: delay compensation through a
/// bus). Each equals the prototype's fingerprint at the tolerances of the module docs.
#[test]
fn mixer_edits_of_the_base_project_match_the_prototype() {
    let fixture = mixer_fixture();
    let golden = Golden::open().expect("golden data is available (set SONARA_GOLDEN)");
    let raw = golden.bytes(BASE).expect("the base project");
    assert_eq!(
        sonara_testkit::sha256_hex(&raw),
        text(&fixture["base_sha256"]),
        "the base project the fixture was recorded from"
    );
    let scenarios = items(&fixture["edits"]);
    assert_eq!(scenarios.len(), 7, "the recorded edits");
    for scenario in scenarios {
        let name = text(&scenario["name"]);
        let project = project(|data| apply_ops(data, items(&scenario["ops"])));
        let samples = items(&scenario["samples"])
            .iter()
            .map(|s| {
                let lr = items(&s[1]);
                (index(&s[0]), [float(&lr[0]), float(&lr[1])])
            })
            .collect();
        assert!(items(&scenario["warnings"]).is_empty(), "{name}: warnings");
        let expected = Fingerprint {
            frames: index(&scenario["frames"]),
            tail_frames: index(&scenario["tail_frames"]),
            latency: index(&scenario["latency"]),
            start_frame: scenario["start_frame"].as_i64().expect("a start frame"),
            abs_sum: float(&scenario["abs_sum"]),
            peak: float(&scenario["peak"]),
            samples,
        };
        check(
            name,
            &project,
            &recorded_request(&scenario["request"]),
            &expected,
        );
    }
}

/// The operations of an `edits` scenario (`engine_oracles.py`'s `apply_ops`): set a value at a
/// path of a track, or add a track.
fn apply_ops(data: &mut JsonMap, ops: &[Value]) {
    for op in ops {
        match text(&op["op"]) {
            "add_track" => add_track(data, op["track"].clone()),
            "set" => {
                let path = items(&op["path"]);
                let (last, parents) = path.split_last().expect("a path");
                let mut node = track(data, text(&op["track"]));
                for part in parents {
                    node = &mut node[text(part)];
                }
                node[text(last)] = op["value"].clone();
            }
            other => panic!("unknown operation {other}"),
        }
    }
}
