//! Real-time: runs on the audio render thread (see docs/audio-engine.md); [`LiveInput::new`] is
//! setup.
//!
//! What a live graph does on the render thread beyond an offline one, in the prototype's
//! terms (`audio/engine/graph.py`):
//!
//! - **Adoption** ([`RenderGraph::adopt`]): a rebuilt graph takes over the state of the graph
//!   it replaces by the plan its builder made (`previous` and `carry_live_state`), with moves
//!   only:
//!   - each reused processor is swapped in for the new graph's stand-in and given the new
//!     parameters (the prototype's reused processor gets `set_state`, then `set_params`; the
//!     state is unchanged by the reuse rule, and `set_state` is setup, RT-02);
//!   - volume, pan, mute and send smoothers are carried and ramp over 256 frames to their new
//!     targets;
//!   - compensation delay lines with the same edge and length are carried;
//!   - the next frame is carried, so the next block does not chase and held notes keep
//!     sounding. A note the old schedule holds at that frame and the edited one does
//!     not gets a note-off at offset 0 of the next block. A note that only the edited schedule
//!     holds does not start until its next note-on (no chase). Start, seek and loop wraps
//!     still chase: they process a frame that does not continue the last block;
//!   - pending live events move over.
//! - **Live input** ([`RenderGraph::inject`]): a fixed buffer of [`LIVE_EVENTS`] per instrument,
//!   kept in stable order by offset; overflow is counted. Each block merges it as the
//!   prototype's stable `sorted([*schedule, *live], key=offset)` does: on equal offsets the
//!   scheduled events come first. Pending releases come before both. When the transport does
//!   not play, the live events are still delivered, as in the prototype.
//! - **Meters** ([`RenderGraph::meters`]): per node, the post-fader block before its output's
//!   compensation delay, NumPy's `max(abs(post), axis=0)` and `sqrt(mean(post * post,
//!   axis=0))`. NumPy sums the axis 0 of a C-ordered `(n, 2)` array sequentially per
//!   column, then divides by `n`.
//! - **Reports** ([`RenderGraph::reports`]): each processor's non-empty report, in the
//!   prototype's `device_reports` order.
//! - **Prefetched clip audio** (`RenderGraph::install_media`): the player's prefetch thread
//!   reads the clips ahead, and the render thread copies a block of it in.
//! - **Deferred setup** ([`RenderGraph::post_setup_requests`],
//!   [`RenderGraph::install_setup`]): requests go to the builder thread once per change, and
//!   prepared setup is routed by processor instance, so it finds a processor that a rebuild
//!   moved.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

use std::fmt;

use sonara_dsp::filters::SmoothedValue;
use sonara_dsp::processor::{EventKind, InstrumentEvent, Report};

use super::layout::{DeviceAddr, Plan, SlotPosition};
use super::{DeviceIndex, DeviceSlot, GraphId, MediaKey, NodeIndex, RenderGraph};
use crate::schedule::NoteBits;
use crate::timeline::Timeline;
use crate::update::{SetupDelivery, SetupTicket};

/// Live events an instrument holds for its next block; more are dropped and counted.
pub const LIVE_EVENTS: usize = 256;

/// The most notes a rebuild can release at once: every channel and pitch.
pub(super) const MAX_RELEASES: usize = 16 * 128;

/// The filler of unused event slots.
pub(super) const NO_EVENT: InstrumentEvent = InstrumentEvent {
    offset: 0,
    kind: EventKind::AllNotesOff,
    pitch: 0,
    velocity: 0,
    channel: 0,
    value: 0,
    number: 0,
    articulation: None,
};

/// The post-fader level of a node over the last block, per channel (left, right).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NodeMeter {
    /// The largest absolute sample.
    pub peak: [f64; 2],
    /// The root mean square.
    pub rms: [f64; 2],
}

impl NodeMeter {
    /// The meter of `block` with NumPy's formulas (see the module docs).
    pub(super) fn measure(block: &[[f64; 2]]) -> Self {
        let mut peak = [0.0_f64; 2];
        let mut sum = [0.0_f64; 2];
        for frame in block {
            for ((p, s), &x) in peak.iter_mut().zip(sum.iter_mut()).zip(frame) {
                let a = x.abs();
                // `np.max` propagates NaN.
                if a > *p || a.is_nan() {
                    *p = a;
                }
                *s += x * x;
            }
        }
        let n = frames_f64(block.len());
        Self {
            peak,
            rms: [(sum[0] / n).sqrt(), (sum[1] / n).sqrt()],
        }
    }

    /// Whether the block clipped (SON-P17): a channel's peak reached full scale (1.0, the level
    /// the GUI's clip indicator latches at) or is NaN. The meter is post-fader, so this is the
    /// level the next stage would receive.
    #[must_use]
    pub fn is_clipping(&self) -> bool {
        is_clipping_peak(&self.peak)
    }
}

/// Whether a peak (left, right) is at or above full scale, or NaN.
#[must_use]
pub(crate) fn is_clipping_peak(peak: &[f64; 2]) -> bool {
    peak.iter().any(|p| p.is_nan() || *p >= 1.0)
}

/// A frame count as NumPy's `mean` divides by it.
#[expect(
    clippy::cast_precision_loss,
    reason = "block lengths are far below 2^53, where the conversion is exact"
)]
fn frames_f64(n: usize) -> f64 {
    n as f64
}

/// [`RenderGraph::adopt`] refused: the graph was planned against another graph than the one
/// it was to replace. Nothing changed; the caller keeps playing `previous`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StalePlan {
    /// The graph the plan was made against.
    pub planned_against: GraphId,
    /// The graph offered for adoption.
    pub previous: GraphId,
}

impl fmt::Display for StalePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the graph was planned against graph {}, not graph {}",
            self.planned_against.get(),
            self.previous.get()
        )
    }
}

impl std::error::Error for StalePlan {}

/// The live input of an instrument node: injected events in stable order by offset, and the
/// notes a rebuild released.
pub(super) struct LiveInput {
    events: Vec<InstrumentEvent>,
    len: usize,
    releases: NoteBits,
}

impl LiveInput {
    /// Setup: allocates the event buffer.
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs on the builder thread, off the real-time thread"
    )]
    pub(super) fn new() -> Self {
        Self {
            events: vec![NO_EVENT; LIVE_EVENTS],
            len: 0,
            releases: NoteBits::EMPTY,
        }
    }

    /// Whether there is nothing to merge.
    pub(super) fn is_idle(&self) -> bool {
        self.len == 0 && self.releases.is_empty()
    }

    /// Adds `event` after every held event with the same or a smaller offset (a stable sort
    /// by offset); false when the buffer is full.
    fn push(&mut self, event: InstrumentEvent) -> bool {
        let len = self.len;
        let at = self.events.get(..len).map_or(len, |held| {
            held.partition_point(|e| e.offset <= event.offset)
        });
        let Some(window) = self.events.get_mut(at..=len) else {
            return false;
        };
        window.rotate_right(1);
        if let Some(first) = window.first_mut() {
            *first = event;
        }
        self.len = len.saturating_add(1);
        true
    }

    /// Takes over the pending events of `old` (the prototype's `carry_live_state`).
    fn take_events(&mut self, old: &mut Self) {
        let pending = old.events.get(..old.len).unwrap_or_default();
        for (slot, event) in self.events.iter_mut().zip(pending) {
            *slot = *event;
        }
        self.len = old.len.min(self.events.len());
        old.len = 0;
    }

    /// Writes the block's events into `out`: the releases, then the stable merge of
    /// `scheduled` with the live events; both are consumed. Returns the written part.
    pub(super) fn merge<'s>(
        &mut self,
        scheduled: &[InstrumentEvent],
        out: &'s mut [InstrumentEvent],
    ) -> &'s [InstrumentEvent] {
        let mut written = 0_usize;
        let mut write = |out: &mut [InstrumentEvent], event: InstrumentEvent| {
            if let Some(slot) = out.get_mut(written) {
                *slot = event;
                written = written.saturating_add(1);
            }
        };
        for event in self.releases.note_offs() {
            write(out, event);
        }
        let live = self.events.get(..self.len).unwrap_or_default();
        let mut scheduled = scheduled.iter().peekable();
        let mut live = live.iter().peekable();
        loop {
            let next = match (scheduled.peek(), live.peek()) {
                (Some(s), Some(l)) if s.offset <= l.offset => scheduled.next(),
                (Some(_) | None, Some(_)) => live.next(),
                (Some(_), None) => scheduled.next(),
                (None, None) => None,
            };
            match next {
                Some(&event) => write(out, event),
                None => break,
            }
        }
        self.len = 0;
        self.releases = NoteBits::EMPTY;
        let out: &'s [InstrumentEvent] = out;
        out.get(..written).unwrap_or_default()
    }

    /// Forgets the pending releases (a reset silenced every voice).
    pub(super) fn clear_releases(&mut self) {
        self.releases = NoteBits::EMPTY;
    }
}

/// Carries `old` into `new` and ramps it to the target `new` was built with (the prototype
/// reuses the old `SmoothedValue` and calls `set_target`).
fn carry(new: &mut SmoothedValue, old: SmoothedValue) {
    let target = new.target();
    *new = old;
    new.set_target(target);
}

impl NodeIndex {
    /// The node at `index` in processing order.
    pub(crate) const fn new(index: usize) -> Self {
        Self(index)
    }

    /// The position in processing order.
    #[must_use]
    pub fn get(self) -> usize {
        self.0
    }
}

impl RenderGraph {
    /// The graph's ID: [`GraphId::OFFLINE`] for [`RenderGraph::new`], unique in the process for
    /// a live graph ([`GraphBuilder`](crate::GraphBuilder)).
    #[must_use]
    pub fn id(&self) -> GraphId {
        self.id
    }

    /// The tempo and time-signature maps of the project the graph was built from, for the
    /// metronome and the MIDI takes of the player.
    #[must_use]
    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    /// The number of audio nodes: the clip buffers `RenderGraph::install_media` takes.
    pub(crate) fn audio_nodes(&self) -> usize {
        self.nodes
            .iter()
            .filter(|node| node.audio.is_some())
            .count()
    }

    /// The clip sources the graph plays, once the builder has keyed them.
    pub(crate) fn media_key(&self) -> Option<MediaKey> {
        self.media_key
    }

    /// Takes over the state of `previous`, the graph this one replaces, by the plan its
    /// [`GraphBuilder`](crate::GraphBuilder) made (see the module docs): reused processors,
    /// smoothers, compensation delay lines, the next frame, releases of held notes, pending
    /// live events and the live counters. `previous` keeps the new graph's stand-ins and
    /// emptied buffers; drop it off the real-time thread (RT-09).
    ///
    /// A fresh graph ([`GraphBuilder::fresh`](crate::GraphBuilder::fresh)) has no plan and
    /// takes over nothing.
    ///
    /// Real-time: moves pointers and copies small values; allocates and frees nothing.
    ///
    /// # Errors
    ///
    /// [`StalePlan`] when the graph was planned against another graph than `previous`, or
    /// adopted already; nothing changes.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn adopt(&mut self, previous: &mut RenderGraph) -> Result<(), StalePlan> {
        let Some(plan) = self.plan.take() else {
            return Ok(());
        };
        if plan.previous != previous.id || self.adopted {
            let stale = StalePlan {
                planned_against: plan.previous,
                previous: previous.id,
            };
            self.plan = Some(plan);
            return Err(stale);
        }
        for &(new, old) in &plan.processors {
            if let (Some(slot), Some(old_slot)) = (self.slot_mut(new), previous.slot_mut(old)) {
                std::mem::swap(&mut slot.processor, &mut old_slot.processor);
                slot.requested = old_slot.requested.take();
                slot.processor.set_params(&slot.params);
            }
        }
        self.adopt_nodes(&plan, previous);
        self.adopt_sends(&plan, previous);
        self.next_frame = previous.next_frame;
        self.last_playing = previous.last_playing;
        self.counters.live_events_dropped = self
            .counters
            .live_events_dropped
            .wrapping_add(previous.counters.live_events_dropped);
        self.counters.setup_requests_unsent = self
            .counters
            .setup_requests_unsent
            .wrapping_add(previous.counters.setup_requests_unsent);
        self.counters.setups_installed = self
            .counters
            .setups_installed
            .wrapping_add(previous.counters.setups_installed);
        self.adopted = true;
        self.plan = Some(plan);
        Ok(())
    }

    /// The node part of [`RenderGraph::adopt`]: smoothers, releases, live events, output
    /// compensation lines.
    fn adopt_nodes(&mut self, plan: &Plan, previous: &mut RenderGraph) {
        let held_from = previous.next_frame.filter(|_| previous.last_playing);
        for &(new, old) in &plan.nodes {
            let (Some(node), Some(old_node)) =
                (self.nodes.get_mut(new), previous.nodes.get_mut(old))
            else {
                continue;
            };
            carry(&mut node.volume, old_node.volume);
            carry(&mut node.pan, old_node.pan);
            carry(&mut node.mute, old_node.mute);
            let Some(input) = node.live.as_mut() else {
                continue;
            };
            let mut releases = old_node.live.as_mut().map_or(NoteBits::EMPTY, |old_input| {
                std::mem::replace(&mut old_input.releases, NoteBits::EMPTY)
            });
            if let (Some(frame), Some(old_schedule), Some(schedule)) =
                (held_from, &old_node.schedule, &node.schedule)
            {
                let gone = old_schedule.held_at(frame).without(schedule.held_at(frame));
                releases = releases.union(gone);
            }
            input.releases = releases;
            if let Some(old_input) = old_node.live.as_mut() {
                input.take_events(old_input);
            }
        }
        for &(node, old_node) in &plan.output_lines {
            let new = self
                .nodes
                .get_mut(node)
                .and_then(|n| n.output_compensation.as_mut());
            let old = previous
                .nodes
                .get_mut(old_node)
                .and_then(|n| n.output_compensation.as_mut());
            if let (Some(new), Some(old)) = (new, old)
                && new.delay() == old.delay()
            {
                std::mem::swap(new, old);
            }
        }
    }

    /// The send part of [`RenderGraph::adopt`]: send-level smoothers and compensation lines.
    fn adopt_sends(&mut self, plan: &Plan, previous: &mut RenderGraph) {
        for &((node, send), (old_node, old_send)) in &plan.sends {
            let old = previous
                .nodes
                .get(old_node)
                .and_then(|n| n.sends.get(old_send))
                .map(|s| s.gain);
            let new = self.nodes.get_mut(node).and_then(|n| n.sends.get_mut(send));
            if let (Some(new), Some(old)) = (new, old) {
                carry(&mut new.gain, old);
            }
        }
        for &((node, send), (old_node, old_send)) in &plan.send_lines {
            let new = self
                .nodes
                .get_mut(node)
                .and_then(|n| n.sends.get_mut(send))
                .and_then(|s| s.compensation.as_mut());
            let old = previous
                .nodes
                .get_mut(old_node)
                .and_then(|n| n.sends.get_mut(old_send))
                .and_then(|s| s.compensation.as_mut());
            if let (Some(new), Some(old)) = (new, old)
                && new.delay() == old.delay()
            {
                std::mem::swap(new, old);
            }
        }
    }

    /// Whether [`RenderGraph::adopt`] still has a plan to apply: a rebuilt live graph before
    /// its adoption.
    #[must_use]
    pub fn awaits_adoption(&self) -> bool {
        self.plan.is_some() && !self.adopted
    }

    /// The node of a track, if the graph has one.
    ///
    /// Real-time: compares strings, allocates nothing.
    #[must_use]
    pub fn node_index(&self, track_id: &str) -> Option<NodeIndex> {
        self.nodes
            .iter()
            .position(|node| node.track_id == track_id)
            .map(NodeIndex)
    }

    /// Queues live events (MIDI input) for the next block of an instrument node
    /// (`inject_events`). Offsets must lie within the block; events beyond it, and events that
    /// do not fit the node's [`LIVE_EVENTS`], are dropped and counted
    /// ([`RenderGraph::live_events_dropped`]). Returns false, taking nothing, when the node
    /// has no instrument or the graph is not live.
    ///
    /// Real-time.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn inject(&mut self, node: NodeIndex, events: &[InstrumentEvent]) -> bool {
        let block = self.block_size;
        let Some(input) = self.nodes.get_mut(node.0).and_then(|n| n.live.as_mut()) else {
            return false;
        };
        let mut dropped = 0_u64;
        for &event in events {
            if event.offset >= block || !input.push(event) {
                dropped = dropped.wrapping_add(1);
            }
        }
        self.counters.live_events_dropped = self.counters.live_events_dropped.wrapping_add(dropped);
        true
    }

    /// The post-fader meters of the last block, by node in processing order
    /// ([`GraphLayout::track_id`](crate::GraphLayout::track_id)); empty for an offline graph.
    #[must_use]
    pub fn meters(&self) -> &[NodeMeter] {
        &self.meters
    }

    /// The processors' non-empty reports after the last block, by device in report order
    /// ([`GraphLayout::device_id`](crate::GraphLayout::device_id)): the prototype's
    /// `device_reports`. The reports describe the most recently rendered block, not the
    /// playhead.
    ///
    /// Real-time.
    pub fn reports(&self) -> impl Iterator<Item = (DeviceIndex, Report)> + '_ {
        self.slots()
            .enumerate()
            .map(|(index, slot)| (DeviceIndex(index), slot.processor.report()))
            .filter(|(_, report)| !report.entries().is_empty())
    }

    /// Hands every new setup request of the processors to `post`, which returns whether it
    /// took it (a real-time queue to the builder thread). A request is posted once per change;
    /// one that `post` refuses is counted ([`RenderGraph::setup_requests_unsent`]) and offered
    /// again on the next call. Call it after [`RenderGraph::process`].
    ///
    /// Real-time when `post` is. It is not marked `nonblocking` for RealtimeSanitizer itself,
    /// because tests pass a `post` that collects into a `Vec`; the player's render step, which
    /// is marked, calls it with a real-time queue.
    pub fn post_setup_requests(&mut self, mut post: impl FnMut(SetupTicket) -> bool) {
        let mut unsent = 0_u64;
        for slot in self.slots_mut() {
            match slot.processor.setup_request() {
                None => slot.requested = None,
                Some(request) if slot.requested == Some(request) => {}
                Some(request) => {
                    let ticket = SetupTicket {
                        instance: slot.instance,
                        request,
                    };
                    if post(ticket) {
                        slot.requested = Some(request);
                    } else {
                        unsent = unsent.wrapping_add(1);
                    }
                }
            }
        }
        self.counters.setup_requests_unsent =
            self.counters.setup_requests_unsent.wrapping_add(unsent);
    }

    /// Installs prepared setup into the processor it was prepared for, wherever a rebuild put
    /// it. Afterwards `delivery` holds what the install displaced, or the unused setup when the
    /// processor is gone or did not take it (false); drop it off the real-time thread.
    ///
    /// Real-time.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn install_setup(&mut self, delivery: &mut SetupDelivery) -> bool {
        let instance = delivery.instance;
        let Some(slot) = self.slots_mut().find(|slot| slot.instance == instance) else {
            return false;
        };
        let installed = slot.processor.install_setup(&mut delivery.setup);
        if installed {
            self.counters.setups_installed = self.counters.setups_installed.wrapping_add(1);
        }
        installed
    }

    /// Installs the clip audio that the player's prefetch thread read for the block whose
    /// sources start at `frame`: one buffer per audio node in processing
    /// order, a block long. The next [`RenderGraph::process`] of `frame` plays it, as after
    /// [`RenderGraph::read_media`]. Returns false and installs nothing when the buffers do not
    /// fit the graph's audio nodes.
    ///
    /// Real-time: copies; allocates nothing.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub(crate) fn install_media(&mut self, frame: i64, tracks: &[Vec<[f64; 2]>]) -> bool {
        let block = self.block_size;
        if tracks.len() != self.audio_nodes() || tracks.iter().any(|t| t.len() != block) {
            return false;
        }
        let buffers = self
            .nodes
            .iter_mut()
            .filter_map(|node| node.audio.as_mut())
            .map(|audio| &mut audio.buffer);
        for (buffer, track) in buffers.zip(tracks) {
            for (y, x) in buffer.iter_mut().zip(track) {
                *y = *x;
            }
        }
        self.media_frame = Some(frame);
        true
    }

    /// Live events dropped since the graph was built, including those of the graphs it
    /// adopted: beyond the block or beyond [`LIVE_EVENTS`].
    #[must_use]
    pub fn live_events_dropped(&self) -> u64 {
        self.counters.live_events_dropped
    }

    /// Setup requests a full queue refused, including those of adopted graphs.
    #[must_use]
    pub fn setup_requests_unsent(&self) -> u64 {
        self.counters.setup_requests_unsent
    }

    /// Prepared setups installed, including those of adopted graphs.
    #[must_use]
    pub fn setups_installed(&self) -> u64 {
        self.counters.setups_installed
    }

    /// Every processor slot in report order.
    fn slots(&self) -> impl Iterator<Item = &DeviceSlot> {
        self.nodes
            .iter()
            .flat_map(|node| node.instrument.iter().chain(node.inserts.iter()))
    }

    fn slots_mut(&mut self) -> impl Iterator<Item = &mut DeviceSlot> {
        self.nodes
            .iter_mut()
            .flat_map(|node| node.instrument.iter_mut().chain(node.inserts.iter_mut()))
    }

    fn slot_mut(&mut self, at: DeviceAddr) -> Option<&mut DeviceSlot> {
        let node = self.nodes.get_mut(at.node)?;
        match at.slot {
            SlotPosition::Instrument => node.instrument.as_mut(),
            SlotPosition::Insert(index) => node.inserts.get_mut(index),
        }
    }
}

#[cfg(test)]
mod tests {
    use sonara_dsp::processor::{EventKind, InstrumentEvent};

    use super::{LiveInput, NodeMeter};

    fn event(offset: usize, kind: EventKind, pitch: u8) -> InstrumentEvent {
        InstrumentEvent {
            offset,
            kind,
            pitch,
            velocity: if kind == EventKind::NoteOn { 100 } else { 0 },
            channel: 0,
            value: 0,
            number: 0,
            articulation: None,
        }
    }

    fn kinds(events: &[InstrumentEvent]) -> Vec<(usize, EventKind, u8)> {
        events.iter().map(|e| (e.offset, e.kind, e.pitch)).collect()
    }

    /// The prototype's `sorted([*schedule, *live], key=lambda e: e.offset)` is stable: on equal
    /// offsets the scheduled events come first, then the live ones in injection order; a live
    /// event injected out of order goes by its offset.
    #[test]
    fn live_events_merge_after_schedule_at_offset_0() {
        let scheduled = [
            event(0, EventKind::NoteOff, 60),
            event(0, EventKind::NoteOn, 64),
            event(5, EventKind::NoteOn, 67),
        ];
        let mut input = LiveInput::new();
        assert!(input.push(event(5, EventKind::NoteOn, 72)));
        assert!(input.push(event(0, EventKind::NoteOn, 48)));
        assert!(input.push(event(0, EventKind::NoteOff, 50)));
        let mut out = [super::NO_EVENT; 16];
        let merged = input.merge(&scheduled, &mut out);
        assert_eq!(
            kinds(merged),
            [
                (0, EventKind::NoteOff, 60),
                (0, EventKind::NoteOn, 64),
                (0, EventKind::NoteOn, 48),
                (0, EventKind::NoteOff, 50),
                (5, EventKind::NoteOn, 67),
                (5, EventKind::NoteOn, 72),
            ]
        );
        assert!(input.is_idle(), "the live events are consumed");
        let mut out = [super::NO_EVENT; 4];
        assert_eq!(kinds(input.merge(&scheduled, &mut out)), kinds(&scheduled));
    }

    /// Releases come first, by channel and pitch; a full buffer refuses.
    #[test]
    fn releases_come_first_and_the_buffer_is_bounded() {
        let mut input = LiveInput::new();
        input.releases.insert(1, 40);
        input.releases.insert(0, 62);
        assert!(input.push(event(0, EventKind::NoteOn, 70)));
        let mut out = [super::NO_EVENT; 8];
        let scheduled = [event(0, EventKind::NoteOn, 64)];
        let merged = input.merge(&scheduled, &mut out);
        assert_eq!(
            merged
                .iter()
                .map(|e| (e.kind, e.channel, e.pitch))
                .collect::<Vec<_>>(),
            [
                (EventKind::NoteOff, 0, 62),
                (EventKind::NoteOff, 1, 40),
                (EventKind::NoteOn, 0, 64),
                (EventKind::NoteOn, 0, 70),
            ]
        );
        for _ in 0..super::LIVE_EVENTS {
            assert!(input.push(event(1, EventKind::Cc, 0)));
        }
        assert!(!input.push(event(1, EventKind::Cc, 0)), "full");
    }

    /// The prototype's `np.max(np.abs(post), axis=0)` and `np.sqrt(np.mean(post * post,
    /// axis=0))` of a `(512, 2)` block whose values both languages compute exactly
    /// (`((i·7919 + c·104729) mod 2003) / 2003 − 0.5`), recorded with NumPy 2.5.4 in the host
    /// virtualenv (2026-10-22): `[peak_l, peak_r, rms_l, rms_r]`. NumPy's sum over axis 0 is
    /// sequential per column; a pairwise sum would end one ulp off in the left RMS
    /// (`0x3fd2_6b5e_382d_3a08`).
    #[test]
    fn clipping_is_a_peak_at_full_scale_or_nan() {
        // Just below full scale does not clip; exactly 1.0 and above does, on either channel.
        assert!(!NodeMeter::measure(&[[0.999, -0.5], [0.2, 0.9999]]).is_clipping());
        assert!(NodeMeter::measure(&[[1.0, 0.0]]).is_clipping());
        assert!(NodeMeter::measure(&[[0.0, 0.0], [0.1, -1.7]]).is_clipping());
        assert!(NodeMeter::measure(&[[f64::NAN, 0.0]]).is_clipping());
        assert!(!NodeMeter::measure(&[[0.0, 0.0]]).is_clipping());
    }

    #[test]
    fn meters_match_prototype_formula() {
        let mut block = [[0.0; 2]; 512];
        for (i, frame) in (0_u32..).zip(block.iter_mut()) {
            for (c, x) in (0_u32..).zip(frame.iter_mut()) {
                *x = f64::from((i * 7919 + c * 104_729) % 2003) / 2003.0 - 0.5;
            }
        }
        let meter = NodeMeter::measure(&block);
        let bits = [meter.peak[0], meter.peak[1], meter.rms[0], meter.rms[1]].map(f64::to_bits);
        assert_eq!(
            bits,
            [
                0x3fe0_0000_0000_0000,
                0x3fe0_0000_0000_0000,
                0x3fd2_6b5e_382d_3a07,
                0x3fd2_8d7a_8aee_963c
            ]
        );
    }

    /// The peak propagates NaN like NumPy's reduction; the mean divides a sequential sum.
    #[test]
    fn meters_follow_numpy() {
        let block = [[0.5, -0.25], [-1.0, 0.0], [0.25, 0.75]];
        let meter = NodeMeter::measure(&block);
        assert_eq!(meter.peak, [1.0, 0.75]);
        let left = ((0.25 + 1.0) + 0.0625) / 3.0_f64;
        let right = ((0.0625 + 0.0) + 0.5625) / 3.0_f64;
        assert_eq!(meter.rms, [left.sqrt(), right.sqrt()]);
        let nan = NodeMeter::measure(&[[f64::NAN, 0.0], [1.0, 0.0]]);
        assert!(nan.peak[0].is_nan());
    }
}
