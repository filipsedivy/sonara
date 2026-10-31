//! Live graphs: rebuilds that take over the playing graph's state,
//! live input, meters, the hand-off to the render thread and deferred setup.
//!
//! The oracles, from strongest to weakest:
//!
//! - **The uninterrupted render.** A rebuild for an edit that does not change the sound (a
//!   renamed track) must continue bit for bit like the graph that was never rebuilt: processor
//!   state, held notes (no chase), compensation delays, smoothers, the next frame.
//! - **An equivalent project.** A parameter edit at a block equals a lane that steps the
//!   parameter at that block; deleting a held note equals a note that ends at that block.
//! - **The prototype.** The meters equal the prototype's `graph.meters()` for the same blocks
//!   (host virtualenv, NumPy 2.5.4, 2026-10-22), bit for bit.
//! - **`SmoothedValue`.** A volume edit ramps over 256 frames exactly as the smoother computes.
//!
//! The binary installs `AllocDisabler`: every block, adoption, swap and install of the render
//! side runs inside `assert_no_alloc`, which aborts on an allocation or a deallocation (RT-02,
//! RT-09). Media is read outside it; these projects have no audio clips.

use std::cell::Cell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use serde_json::{Value, json};
use sonara_core::model::JsonMap;
use sonara_core::model::project::Project;
use sonara_core::model::track::db_to_gain;
use sonara_core::plugins::{PluginRegistry, builtin_registry};
use sonara_dsp::filters::{Smoothed, SmoothedValue};
use sonara_dsp::processor::{EventKind, InstrumentEvent};
use sonara_engine::{
    Applied, DEFAULT_BLOCK, GraphBuilder, GraphOptions, LIVE_EVENTS, MUTE_RAMP, RenderGraph,
    SetupTicket, Update, apply,
};
use sonara_media::MediaProvider;
use sonara_project::serializer::{loads, project_from_value};
use sonara_rt::{handoff, queue};
use sonara_testkit::Golden;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const BASE: &str = "workflows/sidechain_preview/final.project.json";
const KICK: &str = "trk_000000000003";
const BASS: &str = "trk_000000000005";
const BUS: &str = "trk_000000000900";
const REVERB: &str = "dev_000000000913";
const BLOCK: i64 = 512;

fn project(edit: impl FnOnce(&mut JsonMap)) -> Project {
    let golden = Golden::open().expect("golden data is available (set SONARA_GOLDEN)");
    let mut data = loads(&golden.bytes(BASE).expect("the base project")).expect("project JSON");
    edit(&mut data);
    project_from_value(&data).expect("the edited project loads")
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

fn device(id: &str, plugin: &str, params: &Value) -> Value {
    json!({"id": id, "plugin_id": plugin, "name": plugin, "enabled": true, "params": params,
           "sidechain_id": null, "preset": null, "state": {}})
}

fn lane(id: &str, device: &str, param: &str, points: &[(f64, f64, &str)]) -> Value {
    let points: Vec<Value> = points
        .iter()
        .map(|&(beat, value, curve)| json!({"beat": beat, "value": value, "curve": curve}))
        .collect();
    json!({"id": id, "target": {"device_id": device, "param": param}, "enabled": true,
           "points": points})
}

fn inserts<'a>(data: &'a mut JsonMap, id: &str) -> &'a mut Vec<Value> {
    track(data, id)["inserts"]
        .as_array_mut()
        .expect("an insert list")
}

/// The base project with a delay on the bass, a send from the bass to a reverb bus, and a
/// limiter on the kick, whose 102 frames of latency the outputs of the bass and the bus
/// compensate.
fn rich(data: &mut JsonMap) {
    inserts(data, BASS).push(device(
        "dev_000000000910",
        "sonara.delay",
        &json!({"sync": false, "time_ms": 120.0, "feedback": 0.6, "mix": 0.5}),
    ));
    track(data, BASS)["sends"] = json!([{"id": "snd_000000000911", "target_id": BUS,
        "level_db": -6.0, "pre_fader": false, "enabled": true}]);
    inserts(data, KICK).push(device(
        "dev_000000000912",
        "sonara.dynamics.limiter",
        &json!({}),
    ));
    let reverb = device(
        REVERB,
        "sonara.reverb",
        &json!({"decay_s": 0.8, "mix": 1.0}),
    );
    data.get_mut("tracks")
        .and_then(Value::as_array_mut)
        .expect("the track list")
        .push(
            json!({"id": BUS, "name": "Bus", "kind": "bus", "color": null,
            "output_id": "master", "parent_id": null,
            "mixer": {"volume_db": -2.0, "pan": 0.0, "mute": false, "solo": false,
                      "solo_safe": false},
            "input": {"channels": [0], "armed": false, "monitor": "auto"}, "instrument": null,
            "inserts": [reverb], "sends": [], "automation": [], "clips": []}),
        );
}

fn rename_kick(data: &mut JsonMap) {
    track(data, KICK)["name"] = json!("Kick (renamed)");
}

/// The bass plays `notes` at 120 BPM (24 000 frames per beat).
fn bass_notes(data: &mut JsonMap, notes: &Value) {
    data.insert(
        "tempo_map".to_owned(),
        json!([{"beat": 0.0, "bpm": 120.0, "ramp": false}]),
    );
    track(data, BASS)["clips"][0]["notes"] = notes.clone();
}

/// The builder thread's tools.
struct Studio {
    builder: GraphBuilder,
    registry: PluginRegistry,
    media: MediaProvider,
}

impl Studio {
    fn new() -> Self {
        Self {
            builder: GraphBuilder::new(DEFAULT_BLOCK),
            registry: builtin_registry(),
            media: MediaProvider::new(|_| PathBuf::from("/nonexistent/sonara-media"), None),
        }
    }

    fn fresh(&mut self, project: &Project) -> Box<RenderGraph> {
        self.builder
            .fresh(project, &self.registry, &self.media)
            .expect("the graph builds")
    }

    fn rebuild(&mut self, project: &Project) -> Box<RenderGraph> {
        self.builder
            .rebuild(project, &self.registry, &self.media)
            .expect("the graph builds")
    }
}

/// Plays `blocks` blocks from `*frame`: the clip audio is read outside `assert_no_alloc`, the
/// processing runs inside it.
fn play(graph: &mut RenderGraph, frame: &mut i64, blocks: usize, out: &mut Vec<[f64; 2]>) {
    for _ in 0..blocks {
        graph.read_media(*frame).expect("no media to read");
        assert_no_alloc(|| {
            graph.process(*frame, true);
        });
        out.extend_from_slice(graph.output());
        *frame += BLOCK;
    }
}

/// Swaps `next` in for `current` as the render thread does, inside `assert_no_alloc`, and
/// returns the graph it replaced.
fn swap_in(
    current: &mut Option<Box<RenderGraph>>,
    next: Box<RenderGraph>,
) -> Option<Box<RenderGraph>> {
    let mut update = Update::Graph(Some(next));
    let applied = assert_no_alloc(|| apply(current, &mut update));
    assert_eq!(applied, Applied::Swapped);
    match update {
        Update::Graph(old) => old,
        Update::Setup(_) => panic!("a graph update stays one"),
    }
}

fn bits(frames: &[[f64; 2]]) -> Vec<u64> {
    frames.iter().flatten().map(|x| x.to_bits()).collect()
}

/// Renders `blocks` blocks of `project` with one graph, never rebuilt.
fn uninterrupted(project: &Project, blocks: usize) -> Vec<[f64; 2]> {
    let mut graph = Studio::new().fresh(project);
    let (mut frame, mut out) = (0, Vec::new());
    play(&mut graph, &mut frame, blocks, &mut out);
    out
}

/// Renders `blocks` blocks of `before`, switching at block `at` to a rebuild of `after` that
/// adopts the playing graph.
fn rebuilt(before: &Project, after: &Project, at: usize, blocks: usize) -> Vec<[f64; 2]> {
    let mut studio = Studio::new();
    let mut current = None;
    assert!(swap_in(&mut current, studio.fresh(before)).is_none());
    let (mut frame, mut out) = (0, Vec::new());
    let graph = current.as_mut().expect("a graph");
    play(graph, &mut frame, at, &mut out);
    let old = swap_in(&mut current, studio.rebuild(after)).expect("the old graph comes back");
    assert!(!old.awaits_adoption());
    drop(old);
    let graph = current.as_mut().expect("a graph");
    assert!(!graph.awaits_adoption());
    play(graph, &mut frame, blocks - at, &mut out);
    out
}

/// A rebuild keeps every processor's state: an unrelated edit continues bit for bit, and a
/// parameter edit reaches the reused processor exactly as a lane stepping at that block does.
#[test]
fn processor_state_survives_rebuild() {
    const BLOCKS: usize = 220;
    const AT: usize = 120;
    let original = project(rich);
    let expected = uninterrupted(&original, BLOCKS);
    let renamed = project(|d| {
        rich(d);
        rename_kick(d);
    });
    let actual = rebuilt(&original, &renamed, AT, BLOCKS);
    assert_eq!(bits(&actual), bits(&expected), "an unrelated edit");

    // Without adoption the edit point starts from scratch: the comparison discriminates.
    let mut cold = Studio::new().fresh(&renamed);
    let at = i64::try_from(AT).expect("a block") * BLOCK;
    cold.process(at, true);
    let block = AT * 512..(AT + 1) * 512;
    assert_ne!(bits(cold.output()), bits(&expected[block]));

    // The delay's feedback goes from 0.6 to 0.9 at block AT: by an edit and rebuild, or by a
    // hold lane whose step lies between the device clocks of blocks AT - 1 and AT.
    let step = Studio::new()
        .fresh(&original)
        .timeline()
        .frame_to_beat(at - BLOCK / 2);
    let stepped = project(|d| {
        rich(d);
        track(d, BASS)["automation"] = json!([lane(
            "aut_000000000920",
            "dev_000000000910",
            "feedback",
            &[(0.0, 0.6, "hold"), (step, 0.9, "hold")]
        )]);
    });
    let edited = project(|d| {
        rich(d);
        inserts(d, BASS)[1]["params"]["feedback"] = json!(0.9);
    });
    let expected = uninterrupted(&stepped, BLOCKS);
    let actual = rebuilt(&original, &edited, AT, BLOCKS);
    assert_eq!(bits(&actual), bits(&expected), "a parameter edit");
    assert_ne!(
        bits(&actual),
        bits(&uninterrupted(&original, BLOCKS)),
        "the new feedback is heard"
    );
}

/// A rebuild does not chase, so held notes keep sounding; a held note the edit removes
/// gets its note-off at the rebuild, exactly where a note ending there would get it.
#[test]
fn held_notes_survive_rebuild() {
    const BLOCKS: usize = 425;
    // Beat 8 at 120 BPM is frame 192 000, the start of block 375.
    const AT: usize = 375;
    let note = |length: f64| {
        json!([{"id": "n_000000000930", "pitch": 48, "start": 0.0, "length": length,
                "velocity": 100, "channel": 0}])
    };
    let held = project(|d| bass_notes(d, &note(16.0)));
    let expected = uninterrupted(&held, BLOCKS);
    let renamed = project(|d| {
        bass_notes(d, &note(16.0));
        rename_kick(d);
    });
    let actual = rebuilt(&held, &renamed, AT, BLOCKS);
    assert_eq!(
        bits(&actual),
        bits(&expected),
        "the held note keeps sounding"
    );

    // A chase at the rebuild (the prototype's behaviour) would start the note again.
    let mut chased = Studio::new().fresh(&renamed);
    chased.process(i64::try_from(AT).expect("a block") * BLOCK, true);
    assert_ne!(
        bits(chased.output()),
        bits(&expected[AT * 512..(AT + 1) * 512])
    );

    let deleted = project(|d| bass_notes(d, &json!([])));
    let ends_there = project(|d| bass_notes(d, &note(8.0)));
    let actual = rebuilt(&held, &deleted, AT, BLOCKS);
    assert_eq!(
        bits(&actual),
        bits(&uninterrupted(&ends_there, BLOCKS)),
        "the deleted note is released at the rebuild"
    );
    assert_ne!(bits(&actual), bits(&expected));
}

/// A compensation delay line whose edge and length are unchanged is carried, so the
/// compensated outputs (the bass and the bus behind the limited kick) have no gap.
#[test]
fn compensation_delay_survives_rebuild() {
    const BLOCKS: usize = 160;
    const AT: usize = 90;
    let original = project(rich);
    let graph = Studio::new().fresh(&original);
    assert_eq!(graph.latency(), 204, "the kick's and the master's limiters");
    let expected = uninterrupted(&original, BLOCKS);
    let renamed = project(|d| {
        rich(d);
        rename_kick(d);
    });
    let actual = rebuilt(&original, &renamed, AT, BLOCKS);
    let block = AT * 512..(AT + 1) * 512;
    assert_eq!(bits(&actual[block.clone()]), bits(&expected[block]));
    assert_eq!(bits(&actual), bits(&expected));
}

/// The bass alone, at unity gain and centre pan, into a master without inserts: the master
/// output is the bass's pre-fader signal, passed through unchanged.
fn bass_only(data: &mut JsonMap) {
    let tracks = data
        .get_mut("tracks")
        .and_then(Value::as_array_mut)
        .expect("the track list");
    tracks.retain(|t| t["id"] != KICK);
    inserts(data, BASS)[0]["sidechain_id"] = Value::Null;
    track(data, BASS)["mixer"]["volume_db"] = json!(0.0);
    track(data, BASS)["mixer"]["pan"] = json!(0.0);
    track(data, "master")["inserts"] = json!([]);
}

/// A volume edit ramps the carried fader over 256 frames from its old value, as the
/// prototype's reused `SmoothedValue` does, instead of jumping.
#[test]
fn smoothers_ramp_after_rebuild() {
    const BLOCKS: usize = 60;
    const AT: usize = 40;
    let unity = project(bass_only);
    let quieter = project(|d| {
        bass_only(d);
        track(d, BASS)["mixer"]["volume_db"] = json!(-12.0);
    });
    let expected = uninterrupted(&unity, BLOCKS);
    let actual = rebuilt(&unity, &quieter, AT, BLOCKS);
    assert_eq!(bits(&actual[..AT * 512]), bits(&expected[..AT * 512]));
    let mut fader = SmoothedValue::with_value(1.0, MUTE_RAMP);
    fader.set_target(db_to_gain(-12.0));
    let mut ramp = [0.0; 512];
    assert_eq!(fader.next_block(&mut ramp), Smoothed::Ramp);
    assert!(
        ramp[256..]
            .iter()
            .all(|g| g.to_bits() == db_to_gain(-12.0).to_bits()),
        "256 frames of ramp"
    );
    assert!(
        ramp[0] < 1.0 && ramp[0] > ramp[255],
        "{} {}",
        ramp[0],
        ramp[255]
    );
    assert_eq!(
        bits(&actual[(AT + 1) * 512..]),
        bits(
            &expected[(AT + 1) * 512..]
                .iter()
                .map(|f| [f[0] * db_to_gain(-12.0), f[1] * db_to_gain(-12.0)])
                .collect::<Vec<_>>()
        ),
        "steady at the target afterwards"
    );
    let start = AT * 512;
    for (i, &g) in ramp.iter().enumerate() {
        let (a, e) = (actual[start + i], expected[start + i]);
        assert_eq!(
            [a[0].to_bits(), a[1].to_bits()],
            [(e[0] * g).to_bits(), (e[1] * g).to_bits()],
            "frame {i}"
        );
    }
    assert!(
        expected[start..start + 512].iter().any(|f| f[0] != 0.0),
        "the bass plays"
    );
}

fn note_on(pitch: u8, offset: usize) -> InstrumentEvent {
    InstrumentEvent {
        offset,
        kind: EventKind::NoteOn,
        pitch,
        velocity: 100,
        channel: 0,
        value: 0,
        number: 0,
        articulation: None,
    }
}

/// Live input plays when the transport stops (the prototype's idle step), is bounded and
/// counted, reaches only instruments of live graphs, and survives a rebuild.
#[test]
fn live_events_play_and_move_to_a_rebuilt_graph() {
    let silent = project(|d| {
        bass_only(d);
        bass_notes(d, &json!([]));
    });
    let mut studio = Studio::new();
    let mut graph = studio.fresh(&silent);
    let bass = graph.node_index(BASS).expect("the bass");
    let master = graph.node_index("master").expect("the master");
    let peak = |graph: &RenderGraph| {
        graph
            .output()
            .iter()
            .flatten()
            .fold(0.0_f64, |p, x| p.max(x.abs()))
    };
    graph.process(0, false);
    assert_eq!(peak(&graph), 0.0, "nothing plays");
    let injected = assert_no_alloc(|| {
        let accepted = graph.inject(bass, &[note_on(48, 0)]);
        graph.process(BLOCK, false);
        accepted
    });
    assert!(injected);
    assert!(
        peak(&graph) > 0.0,
        "the live note plays on a stopped transport"
    );
    assert!(
        !graph.inject(master, &[note_on(48, 0)]),
        "the master has no instrument"
    );
    let mut offline = RenderGraph::new(
        &silent,
        &studio.registry,
        &studio.media,
        DEFAULT_BLOCK,
        &GraphOptions::default(),
    )
    .expect("builds");
    assert!(
        !offline.inject(bass, &[note_on(48, 0)]),
        "offline graphs take no live input"
    );
    offline.process(0, true);
    assert_eq!(offline.meters().len(), 0, "offline graphs have no meters");

    let flood: Vec<InstrumentEvent> = (0..LIVE_EVENTS + 5).map(|_| note_on(50, 3)).collect();
    assert_no_alloc(|| {
        graph.inject(bass, &flood);
        graph.inject(bass, &[note_on(52, 512)]);
        graph.process(2 * BLOCK, true);
    });
    assert_eq!(
        graph.live_events_dropped(),
        6,
        "5 beyond the buffer, 1 beyond the block"
    );

    // Pending events move to a rebuilt graph.
    let mut current = Some(studio.fresh(&silent));
    let graph = current.as_mut().expect("a graph");
    graph.process(0, true);
    graph.process(BLOCK, true);
    assert_eq!(peak(graph), 0.0);
    assert!(graph.inject(bass, &[note_on(55, 0)]));
    drop(swap_in(&mut current, studio.rebuild(&silent)));
    let graph = current.as_mut().expect("a graph");
    assert_no_alloc(|| {
        graph.process(2 * BLOCK, true);
    });
    assert!(peak(graph) > 0.0, "the pending note plays in the new graph");
}

/// The prototype's meters after a number of blocks: per track, the bits of
/// `[peak_l, peak_r, rms_l, rms_r]`.
type MeterOracle = (usize, [(&'static str, [u64; 4]); 3]);

/// `graph.meters()` of the prototype's `sidechain_preview` graph (no media) after 40 and 45
/// blocks (host virtualenv, NumPy 2.5.4, 2026-10-22).
const METER_ORACLE: [MeterOracle; 2] = [
    (
        40,
        [
            (
                KICK,
                [
                    0x3f38_8951_480d_c890,
                    0x3f38_8951_480d_c88e,
                    0x3f30_cef8_ee48_661a,
                    0x3f30_cef8_ee48_6619,
                ],
            ),
            (
                BASS,
                [
                    0x3fd2_35a9_2e71_fa96,
                    0x3fd2_35a9_2e71_fa96,
                    0x3fc8_d688_593b_e48b,
                    0x3fc8_d688_593b_e48b,
                ],
            ),
            (
                "master",
                [
                    0x3fd2_3ba3_ad1e_eb52,
                    0x3fd2_3ba3_ad1e_eb52,
                    0x3fca_5b76_15a7_3722,
                    0x3fca_5b76_15a7_3722,
                ],
            ),
        ],
    ),
    (
        45,
        [
            (
                KICK,
                [
                    0x3fd5_18ca_2bac_ab1a,
                    0x3fd5_18ca_2bac_ab1a,
                    0x3fc2_e7b8_15be_1e9c,
                    0x3fc2_e7b8_15be_1e9c,
                ],
            ),
            (
                BASS,
                [
                    0x3fd0_7478_2d62_56f7,
                    0x3fd0_7478_2d62_56f7,
                    0x3fb9_ce23_5447_3a78,
                    0x3fb9_ce23_5447_3a78,
                ],
            ),
            (
                "master",
                [
                    0x3fd8_6054_0119_cc34,
                    0x3fd8_6054_0119_cc32,
                    0x3fbb_dfd4_1de6_d8a3,
                    0x3fbb_dfd4_1de6_d8a2,
                ],
            ),
        ],
    ),
];

/// The meters follow the prototype's `graph.meters()` after the same blocks of the
/// `sidechain_preview` project (no media): `[peak_l, peak_r, rms_l, rms_r]`. The formula is
/// checked bit for bit in the unit test `meters_match_prototype_formula`; here the signals
/// themselves differ by an ulp or two in float64 (the synth's and the drums' libm calls), so
/// the comparison allows a relative 1e-12. Measured on macOS, 2026-10-22: the bass's and the
/// master's peaks are exact, the kick's differs by one ulp, the RMS values by up to two.
#[test]
fn meters_follow_the_prototype_graph() {
    let base = project(|_| {});
    let mut studio = Studio::new();
    let mut graph = studio.fresh(&base);
    let layout = studio.builder.layout().expect("a layout").clone();
    assert_eq!(layout.id(), graph.id());
    assert_eq!(graph.meters().len(), layout.node_count());
    let mut frame = 0;
    let mut blocks = 0;
    for (after, expected) in METER_ORACLE {
        let mut out = Vec::new();
        play(&mut graph, &mut frame, after - blocks, &mut out);
        blocks = after;
        for (track_id, bits) in expected {
            let node = layout.node_index(track_id).expect("a node");
            let meter = graph.meters()[node.get()];
            let actual = [meter.peak[0], meter.peak[1], meter.rms[0], meter.rms[1]];
            for (a, e) in actual.into_iter().zip(bits.map(f64::from_bits)) {
                assert!(
                    (a - e).abs() <= 1e-12 * e.abs(),
                    "{track_id} after {after} blocks: {a} != {e}"
                );
            }
        }
    }
    let reports: HashMap<String, Vec<&str>> = graph
        .reports()
        .map(|(device, report)| {
            let id = layout.device_id(device).expect("a device").to_owned();
            (id, report.entries().iter().map(|(name, _)| *name).collect())
        })
        .collect();
    assert_eq!(
        reports.len(),
        4,
        "drums, synth, compressor and limiter report: {reports:?}"
    );
}

/// The builder thread's side of a simulated playback: serves the setup tickets (the latest per
/// processor), sends graphs and setups, and drops what comes back.
struct Builder {
    studio: Studio,
    to_render: handoff::Sender<Update>,
    tickets: queue::Receiver<SetupTicket>,
    served: usize,
}

impl Builder {
    fn serve(&mut self) {
        let mut latest: Vec<SetupTicket> = Vec::new();
        while let Some(ticket) = self.tickets.pop() {
            latest.retain(|t| t.instance != ticket.instance);
            latest.push(ticket);
        }
        for ticket in latest {
            assert!(
                self.to_render.send(Update::Setup(ticket.prepare())).is_ok(),
                "room"
            );
            self.served += 1;
        }
        while let Some(back) = self.to_render.take_returned() {
            drop(back);
        }
    }

    fn send(&mut self, graph: Box<RenderGraph>) {
        assert!(
            self.to_render.send(Update::Graph(Some(graph))).is_ok(),
            "room"
        );
    }
}

/// The reverb's decay is automated (a lane from 0.4 s to 1.6 s): its impulse response is
/// synthesized on the builder thread and installed on the render thread, and nothing on the
/// render side allocates, neither the installs nor the swap of a rebuild.
#[test]
fn automated_reverb_decay_is_set_up_off_the_render_thread() {
    const BLOCKS: usize = 240;
    let automated = |d: &mut JsonMap| {
        rich(d);
        track(d, BUS)["automation"] = json!([lane(
            "aut_000000000940",
            REVERB,
            "decay_s",
            &[(0.0, 0.4, "linear"), (8.0, 1.6, "linear")]
        )]);
    };
    let original = project(automated);
    let renamed = project(|d| {
        automated(d);
        rename_kick(d);
    });
    let run = |serve: bool| {
        let (to_render, mut from_builder) = handoff::channel::<Update>(8);
        let (mut requests, tickets) = queue::channel::<SetupTicket>(16);
        let mut builder = Builder {
            studio: Studio::new(),
            to_render,
            tickets,
            served: 0,
        };
        let fresh = builder.studio.fresh(&original);
        builder.send(fresh);
        let mut current: Option<Box<RenderGraph>> = None;
        let mut out = Vec::new();
        for block in 0..BLOCKS {
            if serve {
                builder.serve();
            }
            if block == 100 {
                let rebuilt = builder.studio.rebuild(&renamed);
                builder.send(rebuilt);
            }
            let frame = i64::try_from(block).expect("a block") * BLOCK;
            assert_no_alloc(|| {
                while from_builder
                    .receive(|update| apply(&mut current, update))
                    .is_some()
                {}
                if let Some(graph) = current.as_mut() {
                    graph.process(frame, true);
                    graph.post_setup_requests(|ticket| requests.push(ticket).is_ok());
                }
            });
            out.extend_from_slice(current.as_ref().expect("a graph").output());
        }
        let graph = current.expect("a graph");
        (
            out,
            graph.setups_installed(),
            graph.setup_requests_unsent(),
            builder.served,
        )
    };
    let (served_out, installed, unsent, served) = run(true);
    assert!(installed >= 10, "{installed} installs");
    assert!(served as u64 >= installed);
    assert_eq!(unsent, 0);
    // Unserved, the reverb keeps its first response: the installs are heard.
    let (held_out, none, _, _) = run(false);
    assert_eq!(none, 0);
    assert_ne!(bits(&served_out), bits(&held_out));
    assert!(served_out.iter().flatten().all(|x| x.is_finite()));
}

thread_local! {
    static ON_RENDER: Cell<bool> = const { Cell::new(false) };
}

/// Updates dropped on the render thread, which must stay 0, and on any thread.
static DROPPED_ON_RENDER: AtomicUsize = AtomicUsize::new(0);
static DROPPED: AtomicUsize = AtomicUsize::new(0);

/// An update whose drop is recorded: the probe of RT-09.
#[derive(Debug)]
struct Probe(Update);

impl Drop for Probe {
    fn drop(&mut self) {
        if ON_RENDER.with(Cell::get) {
            DROPPED_ON_RENDER.fetch_add(1, Ordering::Relaxed);
        }
        DROPPED.fetch_add(1, Ordering::Relaxed);
    }
}

/// The render thread of the drop-probe test: applies updates, plays blocks and posts setup
/// requests inside `assert_no_alloc`, until the graph is taken away. It counts its blocks and
/// hands its channel ends back, so they are dropped on the caller's thread.
fn render_thread(
    mut from_builder: handoff::Receiver<Probe>,
    mut requests: queue::Sender<SetupTicket>,
    blocks: Arc<AtomicUsize>,
) -> std::thread::JoinHandle<(handoff::Receiver<Probe>, queue::Sender<SetupTicket>)> {
    std::thread::spawn(move || {
        ON_RENDER.with(|on| on.set(true));
        let mut current: Option<Box<RenderGraph>> = None;
        let mut had_graph = false;
        let mut frame = 0;
        loop {
            let playing = assert_no_alloc(|| {
                while from_builder
                    .receive(|probe| apply(&mut current, &mut probe.0))
                    .is_some()
                {}
                let Some(graph) = current.as_mut() else {
                    return false;
                };
                graph.process(frame, true);
                graph.post_setup_requests(|ticket| requests.push(ticket).is_ok());
                true
            });
            if playing {
                had_graph = true;
                frame += BLOCK;
                blocks.fetch_add(1, Ordering::Relaxed);
            } else if had_graph {
                break;
            }
            std::thread::sleep(Duration::from_micros(100));
        }
        (from_builder, requests)
    })
}

/// Sends `update` in a probe, taking returned updates while the ring is full; counts it.
fn deliver(to_render: &mut handoff::Sender<Probe>, update: Update, delivered: &mut usize) {
    let mut probe = Probe(update);
    loop {
        match to_render.send(probe) {
            Ok(()) => break,
            Err(full) => {
                probe = full.0;
                while let Some(back) = to_render.take_returned() {
                    drop(back);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
    *delivered += 1;
}

/// A builder thread and a render thread: rebuilds and setups arrive while the render thread
/// plays, inside `assert_no_alloc` throughout; every graph and setup comes back and is dropped
/// on the builder thread, none on the render thread.
#[test]
fn no_graph_is_dropped_on_the_render_thread() {
    const REBUILDS: usize = 12;
    let automated = |volume: f64| {
        project(move |d| {
            rich(d);
            track(d, BUS)["automation"] = json!([lane(
                "aut_000000000940",
                REVERB,
                "decay_s",
                &[(0.0, 0.5, "linear"), (6.0, 1.2, "linear")]
            )]);
            track(d, BASS)["mixer"]["volume_db"] = json!(volume);
        })
    };
    let (mut to_render, from_builder) = handoff::channel::<Probe>(4);
    let (requests, mut tickets) = queue::channel::<SetupTicket>(32);
    let blocks = Arc::new(AtomicUsize::new(0));
    let render = render_thread(from_builder, requests, Arc::clone(&blocks));
    let mut studio = Studio::new();
    let mut delivered = 0;
    let fresh = studio.fresh(&automated(0.0));
    deliver(&mut to_render, Update::Graph(Some(fresh)), &mut delivered);
    for rebuild in 0..REBUILDS {
        while blocks.load(Ordering::Relaxed) < 20 * (rebuild + 1) {
            while let Some(ticket) = tickets.pop() {
                deliver(
                    &mut to_render,
                    Update::Setup(ticket.prepare()),
                    &mut delivered,
                );
            }
            while let Some(back) = to_render.take_returned() {
                drop(back);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let volume = if rebuild % 2 == 0 { -3.0 } else { -6.0 };
        let rebuilt = studio.rebuild(&automated(volume));
        deliver(&mut to_render, Update::Graph(Some(rebuilt)), &mut delivered);
    }
    while blocks.load(Ordering::Relaxed) < 20 * (REBUILDS + 2) {
        std::thread::sleep(Duration::from_millis(1));
    }
    // Takes the graph away: the render thread stops and the last graph comes back. The
    // render thread receives only while the return ring has room, so keep taking returns.
    deliver(&mut to_render, Update::Graph(None), &mut delivered);
    while !render.is_finished() {
        while let Some(back) = to_render.take_returned() {
            drop(back);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let ends = render.join().expect("the render thread");
    while let Some(back) = to_render.take_returned() {
        drop(back);
    }
    drop(ends);
    assert_eq!(
        DROPPED_ON_RENDER.load(Ordering::Relaxed),
        0,
        "no drop on the render thread"
    );
    assert_eq!(
        DROPPED.load(Ordering::Relaxed),
        delivered,
        "every update came back"
    );
    assert_eq!(
        to_render.received(),
        u64::try_from(delivered).expect("a count")
    );
}

/// The failure paths of the hand-off that the player meets (TST-03): a rebuild planned against
/// another graph is rejected and goes back unadopted, while the playing graph plays on; a setup
/// for a processor that is gone goes back unused; a graph adopts only once.
#[test]
fn stale_plans_and_orphan_setups_go_back_unchanged() {
    let original = project(rich);
    let mut studio = Studio::new();
    let first = studio.fresh(&original);
    let planned = studio.rebuild(&original);
    let mut current = Some(Studio::new().fresh(&original));
    let playing = current.as_ref().expect("a graph").id();
    let mut update = Update::Graph(Some(planned));
    let applied = assert_no_alloc(|| apply(&mut current, &mut update));
    assert_eq!(applied, Applied::Rejected);
    assert_eq!(current.as_ref().expect("a graph").id(), playing);
    let Update::Graph(Some(mut rejected)) = update else {
        panic!("the rejected graph goes back");
    };
    assert!(rejected.awaits_adoption());
    let mut other = Studio::new().fresh(&original);
    let stale = rejected
        .adopt(&mut other)
        .expect_err("planned against the first graph");
    assert_eq!(stale.planned_against, first.id());

    // Adopted once, a graph refuses a second adoption.
    let mut first = first;
    assert!(rejected.adopt(&mut first).is_ok());
    assert!(!rejected.awaits_adoption());
    assert!(rejected.adopt(&mut first).is_err());

    // A setup for an instance that is not in the playing graph comes back untouched.
    let automated = project(|d| {
        rich(d);
        track(d, BUS)["automation"] = json!([lane(
            "aut_000000000940",
            REVERB,
            "decay_s",
            &[(0.0, 0.4, "linear"), (8.0, 1.6, "linear")]
        )]);
    });
    let mut source = Studio::new().fresh(&automated);
    let mut tickets = Vec::new();
    for block in 0..4 {
        source.process(block * BLOCK, true);
        source.post_setup_requests(|ticket| {
            tickets.push(ticket);
            true
        });
    }
    let ticket = *tickets
        .last()
        .expect("the automated decay asks for a response");
    let mut update = Update::Setup(ticket.prepare());
    let installed = current.as_ref().expect("a graph").setups_installed();
    let applied = assert_no_alloc(|| apply(&mut current, &mut update));
    assert_eq!(applied, Applied::Unused);
    let Update::Setup(delivery) = &update else {
        panic!("a setup update stays one");
    };
    assert!(delivery.setup.is_pending(), "nothing was installed");
    assert_eq!(
        current.as_ref().expect("a graph").setups_installed(),
        installed
    );
    // Its own graph installs it.
    let Update::Setup(mut delivery) = update else {
        panic!("a setup update stays one");
    };
    assert!(assert_no_alloc(|| source.install_setup(&mut delivery)));
    assert!(!delivery.setup.is_pending());
}
