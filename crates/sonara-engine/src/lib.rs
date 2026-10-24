//! Sonara's render engine: the render
//! sources, the render graph and the offline renderer, ported from the prototype's
//! `audio/engine/graph.py` and `renderer.py`; and the graph
//! side of real-time playback.
//!
//! - [`RenderGraph`] compiles a project into nodes in processing order and renders it block by
//!   block. Building it is setup; [`RenderGraph::process`] is the real-time processing path
//!   (.claude/rules/rust/realtime.md), which allocates, locks and indexes nothing. Audio clips
//!   are read before each block by [`RenderGraph::read_media`], because reading media is file
//!   I/O.
//! - [`OfflineRenderer`] renders a [`RenderRequest`] faster than real time: the latency skip,
//!   pre-roll, the tail rules and the trim of [`OfflineRenderer::render`]. [`RenderStream`]
//!   hands out the untrimmed blocks one by one, for previews and exports.
//! - **Live playback**: a [`GraphBuilder`] builds live graphs on
//!   the builder thread; an [`Update`] carries each to the render thread through
//!   `sonara_rt::handoff`, where [`apply`] lets it adopt the playing graph's state
//!   ([`RenderGraph::adopt`]) and sends the old graph back to be dropped (RT-09). A live graph
//!   takes live MIDI input ([`RenderGraph::inject`]), meters its nodes
//!   ([`RenderGraph::meters`]), reports its devices ([`RenderGraph::reports`]), carries a
//!   [`Timeline`] snapshot, and asks the builder thread for the setup work its processors
//!   defer ([`RenderGraph::post_setup_requests`], [`SetupTicket`]).
//!
//! - **The player** ([`player`]): the player loop on the render thread, the
//!   prefetch thread that reads the audio clips ahead of it, and the control calls.
//!
//! The real-time modules are `automation`, `schedule`, `timeline`, `update`, `graph::process`,
//! `graph::live`, and the player's `render`, `clicks`, `frames`, `message` and `shared`;
//! `clips`, `builder`, the graph's setup, the renderer and the player's control, prefetch and
//! thread modules are not real-time code. Everything the render step reaches is in a real-time
//! module, apart from the queries of `sonara_core::time` that `timeline` and
//! `automation` call: binary and linear searches over the tempo and time-signature points,
//! which allocate, lock and block on nothing (the render paths run under `assert_no_alloc` and
//! RealtimeSanitizer) and index only within the bounds their constructors guarantee.
#![forbid(unsafe_code)]

mod automation;
mod builder;
mod clips;
mod error;
mod graph;
pub mod player;
mod renderer;
mod schedule;
mod timeline;
mod update;

pub use builder::GraphBuilder;
pub use error::EngineError;
pub use graph::{
    DeviceIndex, GraphId, GraphLayout, GraphOptions, InstanceId, LIVE_EVENTS, MUTE_RAMP, NodeIndex,
    NodeMeter, PlayableNotes, RenderGraph, StalePlan,
};
pub use renderer::{
    DEFAULT_BLOCK, MAX_AUTO_TAIL_SECONDS, MAX_RENDER_SECONDS, OfflineRenderer, RenderRequest,
    RenderStats, RenderStream, RenderedAudio, SILENCE, SILENCE_HOLD_SECONDS, Tail, range_frames,
};
pub use timeline::Timeline;
pub use update::{Applied, SetupDelivery, SetupTicket, Update, apply};
