//! The render graph (`audio/engine/graph.py`): a project snapshot compiled into nodes in
//! processing order, shared by offline rendering and real-time playback, so both
//! produce the same mix. The numerics follow
//!
//! Per node, in the order of `routing::processing_order` (sidechain edges included):
//!
//! ```text
//! input (upstream outputs and sends) + audio clips + instrument
//!   → inserts (set_params every block) → finite-value guard → pre-fader tap
//!   → fader and balance pan (sample-accurate mixer automation) → sends → output
//! ```
//!
//! **Plugin delay compensation.** Every node knows the latency of the signal at its input (the
//! largest output latency of its incoming outputs and sends, disabled sends included) and at
//! its output (plus the latencies of its instrument and inserts). Each output and send edge
//! delays its signal by the target's input latency minus the source's output latency, and the
//! sources render [`RenderGraph::latency`] frames ahead of the master output. Sidechain taps
//! are not compensated.
//!
//! **What runs where.** Building the graph, [`RenderGraph::reset`] and
//! [`RenderGraph::read_media`] are setup or non-real-time work; [`RenderGraph::process`] is the
//! real-time processing path. It allocates, locks and indexes nothing; the clip audio of a
//! block is read before it by `read_media`, or, in real-time playback, read ahead by the
//! player's prefetch thread and installed with `RenderGraph::install_media`. Every method the render thread calls lives in the real-time modules `process` and
//! `live`, the lookups among them ([`RenderGraph::id`], [`RenderGraph::latency`],
//! [`RenderGraph::timeline`], `NodeIndex::new`); this module is setup and non-real-time code
//! (.claude/rules/rust/realtime.md, "Placement").
//!
//! **Known prototype defect D3** is fixed: a fresh graph starts every smoother at
//! its target, so a disabled send is silent and a bypassed master is at unity from the first
//! frame (the prototype ramps both over the first 256 frames).
//!
//! **Live graphs** are built by a
//! [`GraphBuilder`](crate::GraphBuilder) on the builder thread for real-time playback. Besides
//! what an offline graph has, they carry the IDs of their tracks, devices and sends, meters,
//! live MIDI input and a plan for taking over the state of the graph they replace
//! ([`RenderGraph::adopt`]). Their processors defer parameter-driven setup work
//! ([`sonara_dsp::processor`], "Deferred setup"). An offline graph has none of it, so offline
//! renders are unchanged (GLD-01).

mod layout;
mod live;
mod process;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::ids::MASTER_ID;
use sonara_core::model::device::{Device, ParamValue};
use sonara_core::model::project::Project;
use sonara_core::model::track::{Track, TrackKind, db_to_gain};
use sonara_core::plugins::{PluginDescriptor, PluginRegistry, SAMPLER_ID};
use sonara_core::pyfmt::str_repr;
use sonara_core::routing;
use sonara_dsp::filters::{DelayLine, SmoothedValue};
use sonara_dsp::processor::{
    DeviceState, ImpulseResponse, InstrumentEvent, NoteSet, ParamSlots, ProcessSpec, Processor,
    SetupRequest,
};
use sonara_dsp::processors::{create, param_slots, reusable};
use sonara_media::{FullAudio, MediaProvider};

pub use layout::GraphLayout;
pub(crate) use layout::Plan;
pub(crate) use live::is_clipping_peak;
pub use live::{LIVE_EVENTS, NodeMeter, StalePlan};

use crate::EngineError;
use crate::automation::{DeviceLane, Lane, TempoCurve};
use crate::clips::{AudioClipSource, ClipsSignature};
use crate::schedule::NoteSchedule;
use crate::timeline::Timeline;
use layout::{DeviceAddr, DeviceLayout, NodeLayout, SendLayout, SlotPosition};
use live::LiveInput;

/// Mute, solo and smoothing ramps in samples.
pub const MUTE_RAMP: u32 = 256;
/// The longest impulse response loaded for a convolver, in seconds (`load_full`).
const MAX_IR_SECONDS: f64 = 30.0;

/// What a graph renders besides the full mix.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphOptions {
    /// Renders only this track or bus and the path from it to the master, like soloing it.
    /// Audio that does not pass through it is blocked (a source's dry path to the master when a
    /// reverb bus is rendered alone).
    pub isolate: Option<String>,
    /// Skips the master's inserts and sets its fader and pan to unity (stems without master
    /// processing). The master's own volume and pan automation still applies.
    pub bypass_master: bool,
    /// The notes each instrument is prepared for ([`DeviceState::playable_notes`]).
    pub playable_notes: PlayableNotes,
}

/// Which notes the instruments of a graph are prepared for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlayableNotes {
    /// Every note, because live input can play any and the render thread must not do setup
    /// work (RT-02): the default, and what real-time playback needs.
    #[default]
    All,
    /// Only the notes of the track's MIDI schedule, for a graph whose only notes are its
    /// schedule's: an offline render. Instruments may skip the setup work for the other notes
    /// (the drum kit synthesizes only the sounds that play), which changes no output.
    Scheduled,
}

/// The numbers of live graphs and processor instances: unique in the process, so that a plan or
/// a setup made by one [`GraphBuilder`](crate::GraphBuilder) is never taken for another
/// builder's (a player that replaces its builder keeps rejecting what was planned for the old
/// one). A process-wide counter is the one way to make that hold across builders (PRI-02); 0
/// is left for offline graphs.
static NEXT_LIVE_ID: AtomicU64 = AtomicU64::new(1);

/// The next live graph or instance number.
fn next_live_id() -> u64 {
    // Relaxed: the counter only has to hand out each number once.
    NEXT_LIVE_ID.fetch_add(1, Ordering::Relaxed)
}

/// Identifies a live graph: unique in the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphId(u64);

impl GraphId {
    /// The ID of every graph built by [`RenderGraph::new`] (offline).
    pub const OFFLINE: Self = Self(0);

    /// The ID as a number: 0 offline, increasing with every live graph built.
    #[must_use]
    pub fn get(self) -> u64 {
        self.0
    }

    /// A new, unique live graph ID.
    pub(crate) fn next() -> Self {
        Self(next_live_id())
    }
}

/// Identifies the clip sources a live graph plays. Graphs whose audio
/// tracks play the same clips share a key, so the clip audio the prefetch thread read for one
/// of them is valid for the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MediaKey(u64);

impl MediaKey {
    /// The key numbered `value`.
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
}

/// The audio tracks of a live graph in processing order, with the sources of their clips: what
/// the player's prefetch thread reads for the graph.
#[derive(Debug)]
pub(crate) struct ClipSources {
    /// Each audio node's track ID and clip source, in processing order.
    pub(crate) tracks: Vec<(String, AudioClipSource)>,
}

impl ClipSources {
    /// What the sources play: equal signatures render equal clip audio.
    pub(crate) fn signature(&self) -> Vec<(String, ClipsSignature)> {
        self.tracks
            .iter()
            .map(|(track, source)| (track.clone(), source.signature()))
            .collect()
    }
}

/// A node's position in processing order: the index of its meters ([`RenderGraph::meters`])
/// and of its track in the [`GraphLayout`]. Valid for one graph only.
///
/// Its methods are in the real-time module `live`: the render thread numbers its meters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeIndex(usize);

/// A device's position among a graph's processors in report order: per node in processing
/// order, the instrument, then the inserts. It indexes [`RenderGraph::reports`] and the
/// [`GraphLayout`]. Valid for one graph only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceIndex(usize);

impl DeviceIndex {
    /// The position in report order.
    #[must_use]
    pub fn get(self) -> usize {
        self.0
    }
}

/// Identifies a processor across the live graphs that reuse it, so that setup prepared for it
/// finds it after a rebuild ([`crate::SetupTicket`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstanceId(u64);

impl InstanceId {
    /// The instance of every processor of an offline graph.
    pub const OFFLINE: Self = Self(0);

    /// A new, unique instance (it shares the numbers of [`GraphId`]).
    fn next() -> Self {
        Self(next_live_id())
    }
}

/// A compiled project: nodes in processing order with their processors and buffers.
pub struct RenderGraph {
    sample_rate: u32,
    block_size: usize,
    spec: ProcessSpec,
    tempo: TempoCurve,
    timeline: Timeline,
    nodes: Vec<Node>,
    /// Per node: its input while upstream nodes run, then its pre-fader signal.
    signal: Vec<Vec<[f64; 2]>>,
    scratch: Scratch,
    /// The master output of the last block.
    output: Vec<[f64; 2]>,
    latency: usize,
    warnings: Vec<String>,
    missing_media: Vec<String>,
    /// The frame the next block must start at to continue without a chase.
    next_frame: Option<i64>,
    /// The frame whose clip audio [`RenderGraph::read_media`] read last.
    media_frame: Option<i64>,
    /// Blocks processed without their clip audio (RT-12: counted, not handled).
    blocks_without_media: u64,
    /// Whether the last block was processed with `playing` (its schedule events were sent).
    last_playing: bool,
    id: GraphId,
    /// Per node, the post-fader meters of the last block; empty in an offline graph.
    meters: Vec<NodeMeter>,
    /// The clip sources it plays, in a live graph whose sources the player's prefetch thread
    /// reads ([`RenderGraph::install_media`]).
    media_key: Option<MediaKey>,
    /// How to take over the state of the graph this one replaces (a live rebuild).
    plan: Option<Plan>,
    /// Whether [`RenderGraph::adopt`] ran.
    adopted: bool,
    counters: LiveCounters,
}

/// What a live graph counts (RT-08, RT-12).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct LiveCounters {
    /// Live events that did not fit the instrument's buffer or the block.
    live_events_dropped: u64,
    /// Setup requests a full queue did not take (posted again with the next call).
    setup_requests_unsent: u64,
    /// Prepared setups installed.
    setups_installed: u64,
}

impl std::fmt::Debug for RenderGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderGraph")
            .field("id", &self.id)
            .field("sample_rate", &self.sample_rate)
            .field("block_size", &self.block_size)
            .field("nodes", &self.nodes.len())
            .field("latency", &self.latency)
            .field("warnings", &self.warnings)
            .finish_non_exhaustive()
    }
}

/// One track, bus or the master. Its methods are in the real-time module `process`.
struct Node {
    /// The track's ID (`master` for the master).
    track_id: String,
    is_master: bool,
    /// The clips of an audio track and the buffer `read_media` fills for the block.
    audio: Option<AudioInput>,
    /// The MIDI schedule of an instrument track.
    schedule: Option<NoteSchedule>,
    /// The enabled instrument of an instrument track.
    instrument: Option<DeviceSlot>,
    /// The enabled, available inserts in signal order.
    inserts: Vec<DeviceSlot>,
    volume: SmoothedValue,
    pan: SmoothedValue,
    mute: SmoothedValue,
    volume_lane: Option<Lane>,
    pan_lane: Option<Lane>,
    sends: Vec<SendTap>,
    /// The node index of the output target, if it is a node of the graph.
    output: Option<usize>,
    /// Whether isolation lets the main output through.
    output_carries: bool,
    output_compensation: Option<DelayLine>,
    input_latency: usize,
    insert_latency: usize,
    /// Live MIDI input and pending releases of an instrument node in a live graph.
    live: Option<LiveInput>,
}

/// The clips of an audio track.
struct AudioInput {
    /// The clips; `None` once a live graph has handed them to the player's prefetch thread
    /// ([`RenderGraph::take_clip_sources`]).
    source: Option<AudioClipSource>,
    /// The clip audio of the block `read_media` read last.
    buffer: Vec<[f64; 2]>,
}

/// An instrument or insert with its processor.
struct DeviceSlot {
    processor: Box<dyn Processor>,
    /// The device's parameter values in descriptor order.
    params: ParamSlots,
    /// The enabled lanes automating its parameters, in the track's lane order.
    lanes: Vec<DeviceLane>,
    /// The node index of the sidechain source, if it is a node processed before this one.
    sidechain: Option<usize>,
    /// The processor's identity across rebuilds ([`InstanceId::OFFLINE`] offline).
    instance: InstanceId,
    /// The setup request posted last and not yet answered (deferred setup).
    requested: Option<SetupRequest>,
}

/// A send of a node.
struct SendTap {
    gain: SmoothedValue,
    pre_fader: bool,
    lane: Option<Lane>,
    /// The node index of the target, if it is a node of the graph.
    target: Option<usize>,
    /// Whether isolation lets this send through.
    carries: bool,
    compensation: Option<DelayLine>,
}

/// Per-block working memory of the processing path, one block long.
struct Scratch {
    /// An instrument's events merged with its live input (live graphs; empty offline).
    events: Vec<InstrumentEvent>,
    instrument: Vec<[f64; 2]>,
    post: Vec<[f64; 2]>,
    contribution: Vec<[f64; 2]>,
    beats: Vec<f64>,
    volume: Vec<f64>,
    mute: Vec<f64>,
    pan: Vec<f64>,
    left: Vec<f64>,
    right: Vec<f64>,
    send: Vec<f64>,
}

impl Scratch {
    /// Buffers for blocks of `n` frames and up to `events` merged events.
    fn new(n: usize, events: usize) -> Self {
        Self {
            events: vec![live::NO_EVENT; events],
            instrument: vec![[0.0; 2]; n],
            post: vec![[0.0; 2]; n],
            contribution: vec![[0.0; 2]; n],
            beats: vec![0.0; n],
            volume: vec![0.0; n],
            mute: vec![0.0; n],
            pan: vec![0.0; n],
            left: vec![0.0; n],
            right: vec![0.0; n],
            send: vec![0.0; n],
        }
    }
}

impl RenderGraph {
    /// Compiles `project` for blocks of `block_size` frames at the project's sample rate.
    ///
    /// Every processor is created, prepared, given its device state (an impulse response is
    /// loaded through `media`; an instrument's playable notes follow
    /// [`GraphOptions::playable_notes`]) and its parameters, as the prototype's graph does. A
    /// plugin that is not available is bypassed with a warning, and so is an impulse response
    /// whose asset is not in the project; clips whose media is missing play silence and are
    /// listed in [`RenderGraph::missing_media`].
    ///
    /// Setup: allocates and reads media, so it runs off the real-time thread.
    ///
    /// # Errors
    ///
    /// - [`EngineError::Domain`]: `ROUTING_CYCLE` for a routing feedback loop, and
    ///   `INVALID_ARGUMENT` for an automation lane on a parameter its plugin does not have
    ///   (the prototype fails the same way when it evaluates the lane);
    /// - [`EngineError::Params`] when a device's values do not fit its plugin's parameters;
    /// - [`EngineError::Media`] when an impulse response cannot be read.
    pub fn new(
        project: &Project,
        registry: &PluginRegistry,
        media: &MediaProvider,
        block_size: NonZeroUsize,
        options: &GraphOptions,
    ) -> Result<Self, EngineError> {
        Self::build(project, registry, media, block_size, options, None).map(|(graph, _)| graph)
    }

    /// Compiles `project` as a live graph ([`crate::GraphBuilder`]): the full mix with every
    /// note playable, meters, live input, deferred setup, and a plan against `live.previous`.
    ///
    /// Setup, like [`RenderGraph::new`], with the same errors.
    pub(crate) fn new_live(
        project: &Project,
        registry: &PluginRegistry,
        media: &MediaProvider,
        block_size: NonZeroUsize,
        live: LiveBuild<'_>,
    ) -> Result<(Self, GraphLayout), EngineError> {
        let options = GraphOptions::default();
        let (graph, layout) =
            Self::build(project, registry, media, block_size, &options, Some(live))?;
        let layout = layout.ok_or_else(|| {
            DomainError::new(ErrorCode::Internal, "a live build without its layout")
        })?;
        Ok((graph, layout))
    }

    /// [`RenderGraph::new`], or a live graph with its layout.
    fn build(
        project: &Project,
        registry: &PluginRegistry,
        media: &MediaProvider,
        block_size: NonZeroUsize,
        options: &GraphOptions,
        live: Option<LiveBuild<'_>>,
    ) -> Result<(Self, Option<GraphLayout>), EngineError> {
        let sample_rate = project.settings.sample_rate;
        let n = block_size.get();
        let spec = ProcessSpec {
            sample_rate,
            block_size: n,
            channels: 2,
        };
        let order = routing::processing_order(project)?;
        let index: HashMap<&str, usize> = order
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        let graph_id = live.as_ref().map_or(GraphId::OFFLINE, |l| l.id);
        let mut builder = Builder {
            project,
            registry,
            media,
            spec,
            options,
            audible: audible(project, options),
            isolation: options
                .isolate
                .as_deref()
                .map(|target| Isolation::new(project, target)),
            index: &index,
            warnings: Vec::new(),
            missing: BTreeSet::new(),
            live: live.map(|l| LiveBuilder::new(l.previous, spec)),
        };
        let mut nodes = Vec::with_capacity(order.len());
        for (position, id) in order.iter().enumerate() {
            nodes.push(builder.node(project.track(id)?, position)?);
        }
        let Builder {
            mut warnings,
            missing,
            live,
            ..
        } = builder;
        let master = index.get(MASTER_ID).copied().ok_or_else(|| {
            DomainError::new(
                ErrorCode::Internal,
                "master node missing from processing order",
            )
        })?;
        let latency = compensate_latency(&mut nodes, master);
        let missing_media: Vec<String> = missing.into_iter().collect();
        if !missing_media.is_empty() {
            warnings.push(format!(
                "{} media file(s) missing; rendering silence for them",
                missing_media.len()
            ));
        }
        let (layout, plan) = match live {
            Some(live) => {
                let (layout, plan) = live.finish(graph_id, &nodes);
                (Some(layout), plan)
            }
            None => (None, None),
        };
        let events = merged_events(&nodes);
        let meters = if layout.is_some() {
            vec![NodeMeter::default(); nodes.len()]
        } else {
            Vec::new()
        };
        let graph = Self {
            sample_rate,
            block_size: n,
            spec,
            tempo: TempoCurve::new(&project.tempo_map),
            timeline: Timeline::new(project, sample_rate),
            signal: vec![vec![[0.0; 2]; n]; nodes.len()],
            nodes,
            scratch: Scratch::new(n, events),
            output: vec![[0.0; 2]; n],
            latency,
            warnings,
            missing_media,
            next_frame: None,
            media_frame: None,
            blocks_without_media: 0,
            last_playing: false,
            id: graph_id,
            meters,
            media_key: None,
            plan,
            adopted: false,
            counters: LiveCounters::default(),
        };
        Ok((graph, layout))
    }

    /// Whether it is a live graph ([`crate::GraphBuilder`]).
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.id != GraphId::OFFLINE
    }

    /// The project's sample rate.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Frames per block.
    #[must_use]
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Problems found while building, in the prototype's wording and order: unavailable
    /// plugins, missing impulse responses, then the count of missing media files.
    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// The IDs of the assets that play silence because their media is missing, sorted.
    #[must_use]
    pub fn missing_media(&self) -> &[String] {
        &self.missing_media
    }

    /// The master output of the last [`RenderGraph::process`] call (silence before the first).
    #[must_use]
    pub fn output(&self) -> &[[f64; 2]] {
        &self.output
    }

    /// Blocks that [`RenderGraph::process`] rendered without their clip audio because
    /// [`RenderGraph::read_media`] had not read it for that frame.
    #[must_use]
    pub fn blocks_without_media(&self) -> u64 {
        self.blocks_without_media
    }

    /// How long the graph keeps sounding after its sources stop: the largest sum of a node's
    /// instrument and insert tails, in seconds.
    #[must_use]
    pub fn tail_seconds(&self) -> f64 {
        let mut tail = 0.0;
        for node in &self.nodes {
            let mut node_tail = 0.0;
            for slot in &node.inserts {
                node_tail += slot.processor.tail_seconds();
            }
            if let Some(slot) = &node.instrument {
                node_tail += slot.processor.tail_seconds();
            }
            if node_tail > tail {
                tail = node_tail;
            }
        }
        tail
    }

    /// Clears all DSP state, as on stop and before an offline render: every processor is
    /// reset and its parameters applied again (`reset()` then `on_params_changed()` in the
    /// prototype, which [`Processor::prepare`] performs), the compensation delays are cleared,
    /// and the next block chases its notes.
    ///
    /// Setup: processors may reallocate their buffers, so it runs off the real-time thread.
    pub fn reset(&mut self) {
        for node in &mut self.nodes {
            for slot in node.inserts.iter_mut().chain(node.instrument.as_mut()) {
                slot.processor.prepare(self.spec);
            }
            if let Some(line) = &mut node.output_compensation {
                line.reset();
            }
            for send in &mut node.sends {
                if let Some(line) = &mut send.compensation {
                    line.reset();
                }
            }
            if let Some(live) = &mut node.live {
                live.clear_releases();
            }
        }
        self.next_frame = None;
        self.media_frame = None;
        self.last_playing = false;
    }

    /// Reads the audio clips of the block whose sources start at `frame`, for the next
    /// [`RenderGraph::process`] of that frame.
    ///
    /// Not real-time: the media readers do file I/O behind a lock.
    ///
    /// # Errors
    ///
    /// [`EngineError::Media`] when a reader fails; the block then plays without clip audio.
    pub fn read_media(&mut self, frame: i64) -> Result<(), EngineError> {
        self.media_frame = None;
        for audio in self.nodes.iter_mut().filter_map(|node| node.audio.as_mut()) {
            audio.buffer.fill([0.0; 2]);
            if let Some(source) = audio.source.as_mut() {
                source.render(frame, &mut audio.buffer)?;
            }
        }
        self.media_frame = Some(frame);
        Ok(())
    }

    /// Takes the clip sources of the audio nodes out, in processing order, for the player's
    /// prefetch thread; afterwards [`RenderGraph::read_media`] reads nothing, and the clip audio
    /// comes from `RenderGraph::install_media`.
    ///
    /// Setup: runs on the builder thread before the graph is sent.
    pub(crate) fn take_clip_sources(&mut self) -> ClipSources {
        let tracks = self
            .nodes
            .iter_mut()
            .filter_map(|node| {
                let source = node.audio.as_mut()?.source.take()?;
                Some((node.track_id.clone(), source))
            })
            .collect();
        ClipSources { tracks }
    }

    /// Records the key of the clip sources the graph plays.
    pub(crate) fn set_media_key(&mut self, key: MediaKey) {
        self.media_key = Some(key);
    }
}

/// The nodes audible in this render: mute and solo, or with isolation the target, everything
/// downstream and upstream of it, and the master.
fn audible(project: &Project, options: &GraphOptions) -> BTreeSet<String> {
    let Some(target) = &options.isolate else {
        return routing::audible_nodes(project);
    };
    let mut audible = BTreeSet::from([target.clone(), MASTER_ID.to_owned()]);
    audible.extend(routing::downstream(project, target));
    audible.extend(routing::upstream(project, target));
    audible
}

/// The edges isolation lets through (`_edge_carries`): only audio that passes through the
/// isolated node reaches the master.
struct Isolation {
    target: String,
    /// The target and everything downstream of it.
    path: BTreeSet<String>,
    /// Everything that feeds the target.
    feeders: BTreeSet<String>,
}

impl Isolation {
    fn new(project: &Project, target: &str) -> Self {
        let mut path = routing::downstream(project, target);
        path.insert(target.to_owned());
        Self {
            target: target.to_owned(),
            path,
            feeders: routing::upstream(project, target),
        }
    }

    fn carries(&self, source: &str, target: &str) -> bool {
        if self.path.contains(source) {
            return self.path.contains(target);
        }
        self.feeders.contains(source) && (self.feeders.contains(target) || target == self.target)
    }
}

/// The state of a graph under construction.
struct Builder<'a> {
    project: &'a Project,
    registry: &'a PluginRegistry,
    media: &'a MediaProvider,
    spec: ProcessSpec,
    options: &'a GraphOptions,
    audible: BTreeSet<String>,
    isolation: Option<Isolation>,
    index: &'a HashMap<&'a str, usize>,
    warnings: Vec<String>,
    missing: BTreeSet<String>,
    /// What a live build records; `None` offline.
    live: Option<LiveBuilder<'a>>,
}

/// What a live build is given ([`crate::GraphBuilder`]).
pub(crate) struct LiveBuild<'a> {
    pub(crate) id: GraphId,
    /// The layout of the graph the new one replaces, when the new one is to take over its
    /// state; `None` for a fresh graph.
    pub(crate) previous: Option<&'a GraphLayout>,
}

/// What a live build records while the nodes are built.
struct LiveBuilder<'a> {
    spec: ProcessSpec,
    previous: Option<&'a GraphLayout>,
    /// The previous graph's devices by ID.
    old_devices: HashMap<&'a str, &'a DeviceLayout>,
    /// Old processors already taken over: one processor never goes to two slots.
    claimed: HashSet<DeviceAddr>,
    nodes: Vec<NodeLayout>,
    devices: Vec<DeviceLayout>,
}

impl<'a> LiveBuilder<'a> {
    /// A live build of graphs for `spec` that plans against `previous`.
    fn new(previous: Option<&'a GraphLayout>, spec: ProcessSpec) -> Self {
        // The prototype reuses only within the same format (`previous.spec == spec`).
        let previous = previous.filter(|layout| layout.spec == spec);
        Self {
            spec,
            previous,
            old_devices: previous
                .map(|layout| {
                    layout
                        .devices
                        .iter()
                        .map(|d| (d.device_id.as_str(), d))
                        .collect()
                })
                .unwrap_or_default(),
            claimed: HashSet::new(),
            nodes: Vec::new(),
            devices: Vec::new(),
        }
    }

    /// The old processor a new slot takes over: the same device and plugin, the same device
    /// state (impulse response and playable notes), and parameters it can take through
    /// `set_params` ([`reusable`]).
    fn reuse(
        &mut self,
        device: &Device,
        params: &ParamSlots,
        state: &DeviceState,
        ir_source: Option<&Arc<FullAudio>>,
    ) -> Option<&'a DeviceLayout> {
        let old = *self.old_devices.get(device.id.as_str())?;
        let same_source = match (old.ir_source.as_ref(), ir_source) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        // A sampler is rebuilt with every graph: its instrument is device state, which `reuse`
        // does not compare, so a reused one could keep playing a replaced patch. The loaded
        // bank is cached, so a rebuild costs no file reads; sounding notes restart (ADR 0010).
        let fits = old.plugin_id == device.plugin_id
            && device.plugin_id != SAMPLER_ID
            && same_source
            && old.playable_notes == state.playable_notes
            && reusable(&device.plugin_id, &old.params, params)
            && self.claimed.insert(old.address);
        fits.then_some(old)
    }

    /// The layout of the finished graph, and its plan against the previous one.
    fn finish(self, id: GraphId, nodes: &[Node]) -> (GraphLayout, Option<Plan>) {
        let mut layouts = self.nodes;
        for (layout, node) in layouts.iter_mut().zip(nodes) {
            if let Some((_, delay)) = &mut layout.output {
                *delay = node
                    .output_compensation
                    .as_ref()
                    .map_or(0, DelayLine::delay);
            }
            for (send, tap) in layout.sends.iter_mut().zip(&node.sends) {
                send.delay = tap.compensation.as_ref().map_or(0, DelayLine::delay);
            }
        }
        let layout = GraphLayout {
            id,
            spec: self.spec,
            nodes: layouts,
            devices: self.devices,
        };
        let plan = self.previous.map(|previous| Plan::new(previous, &layout));
        (layout, plan)
    }
}

impl Builder<'_> {
    fn carries(&self, source: &str, target: &str) -> bool {
        self.isolation
            .as_ref()
            .is_none_or(|iso| iso.carries(source, target))
    }

    /// The node of `track` at `position` in processing order (`_build_node`).
    fn node(&mut self, track: &Track, position: usize) -> Result<Node, EngineError> {
        let is_master = track.kind == TrackKind::Master;
        let bypass = is_master && self.options.bypass_master;
        let n = self.spec.block_size;
        let audio = if track.kind == TrackKind::Audio {
            let (source, missing) = AudioClipSource::new(track, self.project, self.media, n);
            self.missing.extend(missing);
            Some(AudioInput {
                source: Some(source),
                buffer: vec![[0.0; 2]; n],
            })
        } else {
            None
        };
        let mut schedule = None;
        let mut instrument = None;
        if track.kind == TrackKind::Instrument {
            let track_schedule = NoteSchedule::build(
                track.midi_clips(),
                &self.project.tempo_map,
                self.spec.sample_rate,
            );
            let playable = match self.options.playable_notes {
                PlayableNotes::All => NoteSet::ALL,
                PlayableNotes::Scheduled => track_schedule.pitches(),
            };
            schedule = Some(track_schedule);
            if let Some(device) = track.instrument.as_ref().filter(|d| d.enabled) {
                instrument =
                    self.slot(track, device, position, SlotPosition::Instrument, playable)?;
            }
        }
        let mut inserts = Vec::new();
        if !bypass {
            for device in track.inserts.iter().filter(|d| d.enabled) {
                let at = SlotPosition::Insert(inserts.len());
                if let Some(slot) = self.slot(track, device, position, at, NoteSet::ALL)? {
                    inserts.push(slot);
                }
            }
        }
        let mixer = &track.mixer;
        let audible = if self.audible.contains(&track.id) {
            1.0
        } else {
            0.0
        };
        // A fresh graph starts at the targets, so nothing ramps in the first block. A live
        // rebuild carries the old smoothers and ramps them to these targets
        // (`RenderGraph::adopt`).
        let (volume, pan) = if bypass {
            (1.0, 0.0)
        } else {
            (db_to_gain(mixer.volume_db), mixer.pan)
        };
        let sends = self.sends(track);
        // The master has no output. A track without one adds nothing (the prototype would stop
        // processing at it; routing validation reports such a track).
        let output_id = if is_master {
            None
        } else {
            track.output_id.as_deref()
        };
        let output = output_id.and_then(|id| self.index.get(id).copied());
        let live = self.record_node(
            track,
            output_id.filter(|_| output.is_some()),
            instrument.is_some(),
        );
        Ok(Node {
            track_id: track.id.clone(),
            is_master,
            audio,
            schedule,
            instrument,
            inserts,
            volume: SmoothedValue::with_value(volume, MUTE_RAMP),
            pan: SmoothedValue::with_value(pan, MUTE_RAMP),
            mute: SmoothedValue::with_value(audible, MUTE_RAMP),
            volume_lane: mixer_lane(track, "volume_db"),
            pan_lane: mixer_lane(track, "pan"),
            sends,
            output,
            output_carries: output_id.is_some_and(|id| self.carries(&track.id, id)),
            output_compensation: None,
            input_latency: 0,
            insert_latency: 0,
            live,
        })
    }

    /// The sends of `track`, each starting at its level.
    fn sends(&self, track: &Track) -> Vec<SendTap> {
        track
            .sends
            .iter()
            .map(|send| {
                let lane = mixer_lane(track, &format!("send.{}.level_db", send.id));
                let level = if send.enabled {
                    db_to_gain(send.level_db)
                } else {
                    0.0
                };
                SendTap {
                    gain: SmoothedValue::with_value(level, MUTE_RAMP),
                    pre_fader: send.pre_fader,
                    lane,
                    target: self.index.get(send.target_id.as_str()).copied(),
                    carries: self.carries(&track.id, &send.target_id),
                    compensation: None,
                }
            })
            .collect()
    }

    /// Records the node of `track` in a live build's layout (the compensation delays follow in
    /// `LiveBuilder::finish`) and gives a node with an instrument its live input; nothing
    /// offline.
    fn record_node(
        &mut self,
        track: &Track,
        output_id: Option<&str>,
        has_instrument: bool,
    ) -> Option<LiveInput> {
        let builder = self.live.as_mut()?;
        builder.nodes.push(NodeLayout {
            track_id: track.id.clone(),
            output: output_id.map(|id| (id.to_owned(), 0)),
            sends: track
                .sends
                .iter()
                .map(|send| SendLayout {
                    send_id: send.id.clone(),
                    target_id: send.target_id.clone(),
                    delay: 0,
                })
                .collect(),
        });
        has_instrument.then(LiveInput::new)
    }

    /// The processor slot of `device` on `track` (`_slot`), prepared for `playable_notes`, or
    /// `None` with a warning when its plugin is not available.
    ///
    /// A live build that reuses the previous graph's processor builds a cheap stand-in instead
    /// (deferred, without an impulse response or playable notes, so the reverb synthesizes
    /// nothing and the drum kit no sound), which gives the new parameters' latency for the
    /// delay compensation. [`RenderGraph::adopt`] swaps the reused processor in and sends the
    /// stand-in back with the old graph.
    fn slot(
        &mut self,
        track: &Track,
        device: &Device,
        node: usize,
        position: SlotPosition,
        playable_notes: NoteSet,
    ) -> Result<Option<DeviceSlot>, EngineError> {
        let descriptor = self.registry.find(&device.plugin_id);
        let processor = descriptor.and_then(|_| create(&device.plugin_id));
        let (Some(descriptor), Some(mut processor)) = (descriptor, processor) else {
            self.warnings.push(format!(
                "plugin {} on {} is unavailable; bypassed",
                str_repr(&device.plugin_id),
                str_repr(&track.name)
            ));
            return Ok(None);
        };
        let params = if device.plugin_id == SAMPLER_ID {
            self.sampler_params(descriptor, device)?
        } else {
            param_slots(descriptor, &device.params).map_err(|source| EngineError::Params {
                device: device.id.clone(),
                source,
            })?
        };
        let (state, ir_source) = self.device_state(device)?;
        let state = DeviceState {
            playable_notes,
            ..state
        };
        let reused = self.live.as_mut().and_then(|live| {
            live.reuse(device, &params, &state, ir_source.as_ref())
                .map(|old| (old.address, old.instance))
        });
        if reused.is_some() {
            processor.defer_setup();
            processor.set_state(&DeviceState {
                impulse_response: None,
                playable_notes: NoteSet::EMPTY,
                sample_bank: None,
            });
            processor.prepare(self.spec);
            processor.set_params(&params);
        } else {
            processor.prepare(self.spec);
            processor.set_state(&state);
            processor.set_params(&params);
            if self.live.is_some() {
                processor.defer_setup();
            }
        }
        let instance = match (reused, self.live.as_mut()) {
            (Some((_, instance)), _) => instance,
            (None, Some(_)) => InstanceId::next(),
            (None, None) => InstanceId::OFFLINE,
        };
        if let Some(live) = self.live.as_mut() {
            live.devices.push(DeviceLayout {
                device_id: device.id.clone(),
                plugin_id: device.plugin_id.clone(),
                address: DeviceAddr {
                    node,
                    slot: position,
                },
                params,
                ir_source,
                playable_notes: state.playable_notes,
                instance,
                reused: reused.map(|(address, _)| address),
            });
        }
        let lanes = device_lanes(track, device, descriptor)?;
        let sidechain = device
            .sidechain_id
            .as_deref()
            .and_then(|id| self.index.get(id).copied())
            // Processing order puts a sidechain source first; a later one has no signal yet.
            .filter(|&source| source < node);
        Ok(Some(DeviceSlot {
            processor,
            params,
            lanes,
            sidechain,
            instance,
            requested: None,
        }))
    }

    /// The parameters of a sampler. Its `instrument` names a patch of the libraries installed
    /// on this machine; a project that names one that is not installed here (it was made
    /// elsewhere, or the library moved) still renders, with a warning and silence from that
    /// device, instead of failing on the choice.
    fn sampler_params(
        &mut self,
        descriptor: &PluginDescriptor,
        device: &Device,
    ) -> Result<ParamSlots, EngineError> {
        let mut params = device.params.clone();
        let installed = descriptor
            .params
            .iter()
            .find(|spec| spec.id == "instrument")
            .map(|spec| &spec.choices);
        let stale = params
            .get("instrument")
            .and_then(text)
            .filter(|name| installed.is_some_and(|choices| !choices.iter().any(|c| c == name)));
        if let Some(name) = stale {
            self.warnings.push(format!(
                "instrument {} of {} is not installed; it plays silence",
                str_repr(name),
                str_repr(&device.name)
            ));
            params.shift_remove("instrument");
        }
        param_slots(descriptor, &params).map_err(|source| EngineError::Params {
            device: device.id.clone(),
            source,
        })
    }

    /// The installed patch `device` plays: its `instrument` parameter, or the descriptor's
    /// default when the parameter was never set; `None` when the choice is not installed.
    fn sampler_instrument(&self, device: &Device) -> Option<String> {
        let spec = self
            .registry
            .find(SAMPLER_ID)?
            .params
            .iter()
            .find(|spec| spec.id == "instrument")?;
        let name = match device.params.get("instrument") {
            Some(value) => text(value)?.to_owned(),
            None => text(&spec.default)?.to_owned(),
        };
        spec.choices.contains(&name).then_some(name)
    }

    /// The state of a sampler: the instrument of the SFZ patch its `instrument` parameter
    /// names. A patch that cannot be read is a warning, and the sampler plays silence, as a
    /// missing impulse response does not stop a render (ADR 0010).
    fn sampler_state(&mut self, device: &Device) -> DeviceState {
        let Some(path) = self.sampler_instrument(device) else {
            return DeviceState::default();
        };
        let path = path.as_str();
        match self.media.load_sample_bank(std::path::Path::new(path)) {
            Ok(bank) => {
                if !bank.ignored_opcodes.is_empty() {
                    self.warnings.push(format!(
                        "instrument {} uses opcodes the sampler does not play: {}",
                        str_repr(path),
                        bank.ignored_opcodes.join(", ")
                    ));
                }
                DeviceState {
                    sample_bank: Some(bank),
                    ..DeviceState::default()
                }
            }
            Err(error) => {
                self.warnings.push(format!(
                    "instrument {} of {} cannot be loaded: {error}",
                    str_repr(path),
                    str_repr(&device.name)
                ));
                DeviceState::default()
            }
        }
    }

    /// The non-parameter state of `device` (`_device_state`): its impulse response, loaded at
    /// the project rate, when the device names one, with the loaded audio it came from; every
    /// note playable.
    fn device_state(
        &mut self,
        device: &Device,
    ) -> Result<(DeviceState, Option<Arc<FullAudio>>), EngineError> {
        if device.plugin_id == SAMPLER_ID {
            return Ok((self.sampler_state(device), None));
        }
        let Some(ir_id) = ir_asset_id(device) else {
            return Ok((DeviceState::default(), None));
        };
        let Some(asset) = self.project.find_asset(ir_id) else {
            self.warnings.push(format!(
                "impulse response {} for {} is missing",
                str_repr(ir_id),
                str_repr(&device.name)
            ));
            return Ok((DeviceState::default(), None));
        };
        let full = self
            .media
            .load_full(asset, self.spec.sample_rate, MAX_IR_SECONDS)
            .map_err(|source| EngineError::Media {
                asset: asset.id.clone(),
                source,
            })?;
        let state = DeviceState {
            impulse_response: Some(Arc::new(ImpulseResponse {
                channels: full.channels,
                samples: full.samples.clone(),
            })),
            ..DeviceState::default()
        };
        Ok((state, Some(full)))
    }
}

/// The text of a string parameter value.
fn text(value: &ParamValue) -> Option<&str> {
    match value {
        ParamValue::Str(text) => Some(text),
        _ => None,
    }
}

/// The most events an instrument of a live graph merges in one block: its schedule's worst
/// case, the live buffer and one release per channel and pitch; 0 without live input.
fn merged_events(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .filter(|node| node.live.is_some())
        .filter_map(|node| node.schedule.as_ref())
        .map(|schedule| {
            schedule
                .capacity()
                .saturating_add(LIVE_EVENTS)
                .saturating_add(live::MAX_RELEASES)
        })
        .max()
        .unwrap_or(0)
}

/// The enabled mixer lane of `track` for `param`, if it has points (`lane_for` takes the first
/// lane with that target, enabled or not).
fn mixer_lane(track: &Track, param: &str) -> Option<Lane> {
    track
        .lane_for(None, param)
        .filter(|lane| lane.enabled)
        .and_then(Lane::new)
}

/// The enabled lanes with points that automate parameters of `device`, in lane order.
fn device_lanes(
    track: &Track,
    device: &Device,
    descriptor: &PluginDescriptor,
) -> Result<Vec<DeviceLane>, EngineError> {
    let mut lanes = Vec::new();
    for lane in track
        .automation
        .iter()
        .filter(|lane| lane.enabled && lane.target.device_id.as_deref() == Some(&device.id))
    {
        let Some(points) = Lane::new(lane) else {
            continue;
        };
        let spec = descriptor.param(&lane.target.param)?;
        let slot = descriptor
            .params
            .iter()
            .position(|p| p.id == spec.id)
            .unwrap_or_default();
        lanes.push(DeviceLane::new(points, slot, spec));
    }
    Ok(lanes)
}

/// Sets every node's input and insert latency, adds the compensation delays to the output and
/// send edges (`_compute_latency`), and returns the master's output latency.
fn compensate_latency(nodes: &mut [Node], master: usize) -> usize {
    /// Records an incoming edge of `latency` frames at `target`.
    fn raise(incoming: &mut [Option<usize>], target: Option<usize>, latency: usize) {
        if let Some(slot) = target.and_then(|t| incoming.get_mut(t)) {
            *slot = Some(slot.map_or(latency, |l| l.max(latency)));
        }
    }
    let mut incoming: Vec<Option<usize>> = vec![None; nodes.len()];
    for (i, node) in nodes.iter_mut().enumerate() {
        node.input_latency = incoming.get(i).copied().flatten().unwrap_or(0);
        node.insert_latency = node
            .inserts
            .iter()
            .chain(node.instrument.as_ref())
            .map(|slot| slot.processor.latency())
            .sum();
        let out = node.output_latency();
        raise(&mut incoming, node.output, out);
        for send in &node.sends {
            raise(&mut incoming, send.target, out);
        }
    }
    let input_latency: Vec<usize> = nodes.iter().map(|n| n.input_latency).collect();
    let line = |target: Option<usize>, out: usize| {
        let need = target
            .and_then(|t| input_latency.get(t))
            .and_then(|input| input.checked_sub(out))
            .filter(|&need| need > 0)?;
        Some(DelayLine::new(need, 2))
    };
    for node in nodes.iter_mut() {
        let out = node.output_latency();
        node.output_compensation = line(node.output, out);
        for send in &mut node.sends {
            send.compensation = line(send.target, out);
        }
    }
    nodes.get(master).map_or(0, Node::output_latency)
}

/// The asset ID of a convolver's impulse response (`state["ir_asset_id"]`), if it is a string.
fn ir_asset_id(device: &Device) -> Option<&str> {
    device.state.get("ir_asset_id")?.as_str()
}
