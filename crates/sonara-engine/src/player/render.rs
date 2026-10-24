//! Real-time: runs on the audio render thread (see docs/audio-engine.md); [`RenderLoop::new`]
//! is setup.
//!
//! The player loop (`_step`, `_next_block`, `_idle_step` and `_poll_midi` of the prototype's
//! `player.py`). One [`RenderLoop::step`] does, in order:
//!
//! 1. **Apply** every waiting message of the control thread (starts, seeks, halts, graphs,
//!    setup, the stream, MIDI inputs, targets, GUI preview notes), swapping each new value in
//!    and the old one out to be dropped on the control thread (RT-09).
//! 2. **GUI previews:** inject each pending preview note (the piano roll keyboard's click,
//!    [`Message::Audition`]) into its own node, for the graph it was posted for; a stale graph
//!    drops it. On a stopped transport this alone is enough to run the idle step, the same as
//!    live MIDI input.
//! 3. **Live MIDI:** drain every input's ring, merged by arrival time (ties in input order, as
//!    the prototype's stable sort). While there are targets, count the messages
//!    (`messages_received`, system-exclusive markers included) and inject note, controller,
//!    pitch-bend and channel-pressure events at offset 0 into every target instrument. While a
//!    recording with MIDI takes plays, the messages also go to the control thread with the
//!    timeline frame they were heard at: each input's newest message of the drain at
//!    the device's software position minus the output latency, the others earlier by their
//!    difference in arrival time (`_poll_midi`'s anchoring).
//! 4. **Render**, while playing: one block per step until the lookahead target is queued
//!    (80 ms, or 25 ms while live input is active) or the ring is full. A block is
//!    assembled from the carry of whole graph blocks; a loop end cuts the source block
//!    sample-accurately and rendering continues at the loop start. Each source block's clip
//!    audio comes from the prefetch thread's feed; a block that is not ready plays its clips
//!    silent and is counted, never waited for (RT-03, RT-12). The metronome clicks are added
//!    per part, then the block is cast to `f32` (round to nearest) and written with its
//!    tag and generation. The first block of a generation arms `playing`; the last block of a
//!    play range disarms it, and once the ring has drained the range ends.
//!    Stopped or paused with live input active, the idle step renders the live input alone at
//!    the resting position: no scheduled notes, no clip audio.
//! 5. **Post** the processors' setup requests to the builder thread.
//!
//! `render_load` times only the graph and the carry (`_next_block`), as the prototype does.
//! Reading the monotonic clock is the one system call RT-03 allows.
//!
//! Nothing here allocates, frees, locks or waits: every buffer is preallocated by
//! [`RenderLoop::new`], every value arrives and leaves through a ring, and the loop that sleeps
//! between steps lives in the non-real-time `threads` module.
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

use std::sync::Arc;
use std::time::Instant;

use sonara_dsp::processor::{EventKind, InstrumentEvent};
use sonara_rt::{Generation, MidiEvent, MidiKind, TimelineFrame, handoff, queue};

use super::clicks::{Click, MAX_PARTS, Metronome, Part};
use super::frames::{BLOCK, BLOCK_I64, LoopRange, advance};
use super::message::{
    Cue, LiveTargets, MAX_MIDI_INPUTS, MediaFeed, Message, MeterEntry, MidiChange, MidiSlot,
    RecordedMidi, ReportEntry, StreamPart,
};
use super::shared::Shared;
use crate::graph::{GraphId, LIVE_EVENTS, NodeIndex, RenderGraph};
use crate::update::{Applied, SetupTicket, apply};

/// The filler of unused live-event slots.
const NO_EVENT: InstrumentEvent = InstrumentEvent {
    offset: 0,
    kind: EventKind::AllNotesOff,
    pitch: 0,
    velocity: 0,
    channel: 0,
    value: 0,
    number: 0,
};

/// The most MIDI events one step takes from the inputs; the rest wait for the next step.
const MAX_MIDI_PER_STEP: usize = 4096;
/// The most GUI preview notes pending at once (`Message::Audition`, drained every step); a
/// click and its release are the common case, this covers a quick run across keys too. One
/// arriving past this is dropped, as a note preview is best-effort.
const AUDITION_CAPACITY: usize = 8;
/// The filler of unused slots of the recorded-message scratch.
const NO_MESSAGE: (usize, MidiEvent) = (0, MidiEvent::sysex(0, 0));
/// The most chunks the carry holds: the rest of a block, a loop's cut block, the next block.
const MAX_CHUNKS: usize = 4;

/// What one [`RenderLoop::step`] did, which sets how long the render thread rests before the
/// next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// A block was written: step again at once.
    Wrote,
    /// Playing, but the lookahead target is queued or the ring is full: rest briefly.
    Waiting,
    /// Not playing and nothing to render: rest longer.
    Idle,
}

/// The transport as the render thread sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Stopped or paused: only the idle step renders, with live input.
    Halted,
    /// Playing a sequence.
    Playing,
}

/// The rendered-but-unsent frames of the loop (`_carry`): at most two blocks.
struct Carry {
    frames: Vec<[f64; 2]>,
    len: usize,
    chunks: [Part; MAX_CHUNKS],
    count: usize,
}

impl Carry {
    /// Setup: two blocks of frames.
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    fn new() -> Self {
        Self {
            frames: vec![[0.0; 2]; 2 * BLOCK],
            len: 0,
            chunks: [Part::default(); MAX_CHUNKS],
            count: 0,
        }
    }

    fn clear(&mut self) {
        self.len = 0;
        self.count = 0;
    }

    /// Appends `data`, the kept frames of a source block tagged `tag`. False when it does not
    /// fit, which the loop rules out (a chunk is added only while less than a block is held).
    fn push(&mut self, data: &[[f64; 2]], tag: i64) -> bool {
        let end = self.len.saturating_add(data.len());
        let (Some(target), Some(chunk)) = (
            self.frames.get_mut(self.len..end),
            self.chunks.get_mut(self.count),
        ) else {
            return false;
        };
        for (y, x) in target.iter_mut().zip(data) {
            *y = *x;
        }
        *chunk = Part {
            tag,
            frames: data.len(),
        };
        self.len = end;
        self.count += 1;
        true
    }

    /// Moves the first block of frames into `out` and the timeline parts they came from,
    /// contiguous chunks merged, into `parts`; returns the number of parts. A chunk that is
    /// split keeps its remainder, tagged `tag + used`. Needs a whole block held.
    fn take_block(&mut self, out: &mut [[f64; 2]], parts: &mut [Part; MAX_PARTS]) -> usize {
        if self.len < BLOCK {
            return 0;
        }
        for (y, x) in out.iter_mut().zip(&self.frames) {
            *y = *x;
        }
        let mut need = BLOCK;
        let mut written = 0_usize;
        let mut consumed = 0_usize;
        for chunk in self.chunks.iter_mut().take(self.count) {
            if need == 0 {
                break;
            }
            let take = chunk.frames.min(need);
            let merged = written
                .checked_sub(1)
                .and_then(|last| parts.get_mut(last))
                .filter(|part| {
                    part.tag
                        .saturating_add(i64::try_from(part.frames).unwrap_or(BLOCK_I64))
                        == chunk.tag
                });
            match merged {
                Some(part) => part.frames += take,
                None => {
                    if let Some(part) = parts.get_mut(written) {
                        *part = Part {
                            tag: chunk.tag,
                            frames: take,
                        };
                        written += 1;
                    }
                }
            }
            need -= take;
            if take == chunk.frames {
                consumed += 1;
            } else {
                chunk.tag = chunk
                    .tag
                    .saturating_add(i64::try_from(take).unwrap_or(BLOCK_I64));
                chunk.frames -= take;
            }
        }
        if let Some(chunks) = self.chunks.get_mut(..self.count) {
            chunks.rotate_left(consumed.min(chunks.len()));
        }
        self.count = self.count.saturating_sub(consumed);
        if let Some(held) = self.frames.get_mut(..self.len) {
            held.rotate_left(BLOCK);
        }
        self.len -= BLOCK;
        written
    }
}

/// What the last written block was, for tests and diagnostics ([`RenderLoop::last_block`]).
#[derive(Debug, Clone, Copy)]
struct Written {
    tag: i64,
    generation: Generation,
    parts: [Part; MAX_PARTS],
    parts_len: usize,
    clicked: bool,
    idle: bool,
}

/// The last block a [`RenderLoop`] wrote: its samples and how it was assembled.
#[derive(Debug, Clone, Copy)]
pub struct BlockView<'a> {
    written: &'a Written,
    samples: &'a [f32],
    clicks: &'a [Click],
}

impl BlockView<'_> {
    /// The timeline tag: the frame of its first sample on the latency-compensated timeline.
    #[must_use]
    pub fn tag(&self) -> i64 {
        self.written.tag
    }

    /// Its generation.
    #[must_use]
    pub fn generation(&self) -> Generation {
        self.written.generation
    }

    /// The interleaved stereo `f32` samples, a block long.
    #[must_use]
    pub fn samples(&self) -> &[f32] {
        self.samples
    }

    /// The timeline parts it was assembled from, as `(tag, frames)`: one, or two or three
    /// where a loop wraps inside it.
    pub fn parts(&self) -> impl Iterator<Item = (i64, usize)> + '_ {
        self.written
            .parts
            .iter()
            .take(self.written.parts_len)
            .map(|part| (part.tag, part.frames))
    }

    /// The metronome clicks it started as `(offset, accent)`, or `None` when the metronome
    /// was off.
    #[must_use]
    pub fn clicks(&self) -> Option<impl Iterator<Item = (i64, bool)> + '_> {
        self.written
            .clicked
            .then(|| self.clicks.iter().map(|click| (click.offset, click.accent)))
    }

    /// Whether it is an idle block: live input on a stopped transport.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.written.idle
    }
}

/// The render thread's state: everything [`RenderLoop::step`] works on.
struct State {
    shared: Arc<Shared>,
    graph: Option<Box<RenderGraph>>,
    stream: Option<Box<StreamPart>>,
    mode: Mode,
    generation: Generation,
    /// The next source frame of the playing sequence.
    src: i64,
    end: Option<i64>,
    looped: Option<LoopRange>,
    /// The resting position: the idle step's tag.
    rest: i64,
    awaiting_first: bool,
    force_clicks: bool,
    /// Whether live MIDI input is recorded (a recording with MIDI takes is playing).
    record_midi: bool,
    pass: u32,
    carry: Carry,
    /// The assembled block in `f64`.
    mix: Vec<[f64; 2]>,
    parts: [Part; MAX_PARTS],
    parts_len: usize,
    /// The block in `f32`, interleaved.
    block: Vec<f32>,
    metronome: Metronome,
    feed: Option<MediaFeed>,
    /// The position of the next source block in the playing sequence.
    media_index: u64,
    /// The open MIDI inputs by slot.
    midi: [MidiSlot; MAX_MIDI_INPUTS],
    targets: LiveTargets,
    events: Vec<InstrumentEvent>,
    /// Pending GUI preview notes (`Message::Audition`), each with the graph it was posted for.
    audition: [Option<(GraphId, NodeIndex, InstrumentEvent)>; AUDITION_CAPACITY],
    /// The messages of one drain to record, with the slot of their input.
    heard: Vec<(usize, MidiEvent)>,
    recorded: queue::Sender<RecordedMidi>,
    tickets: queue::Sender<SetupTicket>,
    meters: queue::Sender<MeterEntry>,
    reports: queue::Sender<ReportEntry>,
    blocks_written: u64,
    written: Option<Written>,
}

/// The render thread's half of a player: call [`step`](RenderLoop::step) in a loop, resting
/// between steps as the [`Step`] says. Drop it off the render thread (RT-09).
pub struct RenderLoop {
    messages: handoff::Receiver<Message>,
    state: State,
}

impl RenderLoop {
    /// Setup: preallocates every buffer.
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    pub(crate) fn new(
        messages: handoff::Receiver<Message>,
        tickets: queue::Sender<SetupTicket>,
        meters: queue::Sender<MeterEntry>,
        reports: queue::Sender<ReportEntry>,
        recorded: queue::Sender<RecordedMidi>,
        shared: Arc<Shared>,
    ) -> Self {
        Self {
            messages,
            state: State {
                shared,
                graph: None,
                stream: None,
                mode: Mode::Halted,
                generation: Generation::new(1),
                src: 0,
                end: None,
                looped: None,
                rest: 0,
                awaiting_first: false,
                force_clicks: false,
                record_midi: false,
                pass: 0,
                carry: Carry::new(),
                mix: vec![[0.0; 2]; BLOCK],
                parts: [Part::default(); MAX_PARTS],
                parts_len: 0,
                block: vec![0.0; 2 * BLOCK],
                metronome: Metronome::new(),
                feed: None,
                media_index: 0,
                midi: std::array::from_fn(|_| None),
                targets: LiveTargets::default(),
                events: vec![NO_EVENT; LIVE_EVENTS],
                audition: [None; AUDITION_CAPACITY],
                heard: vec![NO_MESSAGE; MAX_MIDI_PER_STEP],
                recorded,
                tickets,
                meters,
                reports,
                blocks_written: 0,
                written: None,
            },
        }
    }

    /// One step of the player loop (see the module docs).
    ///
    /// Real-time: allocates, frees, locks and waits for nothing.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn step(&mut self) -> Step {
        let Self { messages, state } = self;
        let mut applied = false;
        while messages.receive(|message| state.apply(message)).is_some() {
            applied = true;
        }
        let audition = state.drain_auditions();
        let live = state.drain_midi();
        let step = match state.mode {
            Mode::Playing => state.play_step(live),
            Mode::Halted if live || audition => state.idle_step(),
            Mode::Halted => Step::Idle,
        };
        state.post_setup_requests();
        // A message often comes with more (a recalled graph comes back as a start): rest
        // briefly.
        if applied && step == Step::Idle {
            Step::Waiting
        } else {
            step
        }
    }

    /// The last block written, until the next step writes one.
    #[must_use]
    pub fn last_block(&self) -> Option<BlockView<'_>> {
        let state = &self.state;
        state.written.as_ref().map(|written| BlockView {
            written,
            samples: &state.block,
            clicks: state.metronome.started(),
        })
    }
}

impl State {
    /// Applies one message, leaving in it what it replaced.
    fn apply(&mut self, message: &mut Message) {
        match message {
            Message::Graph {
                update,
                feed,
                targets,
            } => {
                if apply(&mut self.graph, update) == Applied::Swapped {
                    if feed.is_some() {
                        std::mem::swap(&mut self.feed, feed);
                    }
                    std::mem::swap(&mut self.targets, targets);
                }
            }
            Message::Setup(update) | Message::Recall(update) => {
                apply(&mut self.graph, update);
            }
            Message::Start {
                graph,
                feed,
                targets,
                cue,
            } => {
                apply(&mut self.graph, graph);
                std::mem::swap(&mut self.feed, feed);
                std::mem::swap(&mut self.targets, targets);
                self.start(*cue);
            }
            Message::Seek {
                generation,
                frame,
                feed,
            } => {
                std::mem::swap(&mut self.feed, feed);
                self.start(Cue {
                    generation: *generation,
                    start: *frame,
                    end: self.end,
                    looped: self.looped,
                    force_clicks: self.force_clicks,
                    record_midi: self.record_midi,
                });
            }
            Message::Halt {
                generation,
                rest,
                feed,
            } => {
                std::mem::swap(&mut self.feed, feed);
                self.mode = Mode::Halted;
                self.generation = *generation;
                self.rest = *rest;
                self.record_midi = false;
            }
            Message::Rest(frame) => self.rest = *frame,
            Message::Stream(part) => std::mem::swap(&mut self.stream, part),
            Message::Midi(change) => change_inputs(&mut self.midi, change),
            Message::Targets(targets) => std::mem::swap(&mut self.targets, targets),
            Message::Audition { graph, node, event } => {
                if let Some(slot) = self.audition.iter_mut().find(|slot| slot.is_none()) {
                    *slot = Some((*graph, *node, *event));
                }
            }
        }
    }

    /// Injects pending GUI preview notes (`Message::Audition`) into their node, for the graph
    /// they were posted for; a stale graph (rebuilt since) drops its note silently. Returns
    /// whether anything was injected, so a stopped transport renders this step too.
    fn drain_auditions(&mut self) -> bool {
        let mut injected = false;
        for slot in &mut self.audition {
            let Some((id, node, event)) = slot.take() else {
                continue;
            };
            if let Some(graph) = self.graph.as_deref_mut()
                && graph.id() == id
            {
                injected |= graph.inject(node, std::slice::from_ref(&event));
            }
        }
        injected
    }

    /// Starts the sequence of `cue` (`_start_locked`, or a seek while playing): a new
    /// generation from its first frame, with an empty carry and no sounding clicks.
    fn start(&mut self, cue: Cue) {
        self.mode = Mode::Playing;
        self.generation = cue.generation;
        self.src = cue.start;
        self.rest = cue.start;
        self.end = cue.end;
        self.looped = cue.looped;
        self.force_clicks = cue.force_clicks;
        self.record_midi = cue.record_midi;
        self.awaiting_first = true;
        self.carry.clear();
        self.metronome.clear();
        self.media_index = 0;
        self.pass = self.pass.wrapping_add(1);
        self.shared.set_progress(self.generation, 0);
    }

    /// The live-input targets of the current graph.
    fn targets(&self) -> &[NodeIndex] {
        match (&self.graph, self.targets.graph) {
            (Some(graph), Some(id)) if graph.id() == id => &self.targets.nodes,
            _ => &[],
        }
    }

    /// Drains the MIDI inputs (`_poll_midi`); returns whether live input is active: there are
    /// targets and an open input.
    fn drain_midi(&mut self) -> bool {
        let has_targets = !self.targets().is_empty();
        let recording = has_targets && self.record_midi && self.mode == Mode::Playing;
        let mut received = 0_u64;
        let mut events = 0_usize;
        let mut heard = 0_usize;
        let mut dropped = 0_u64;
        // Each input's newest arrival time in this drain: the anchor of its recorded messages.
        let mut newest = [0_u64; MAX_MIDI_INPUTS];
        for _ in 0..MAX_MIDI_PER_STEP {
            let Some((slot, message)) = next_message(&mut self.midi) else {
                break;
            };
            received += 1;
            if !has_targets {
                continue;
            }
            if recording {
                if let Some(time) = newest.get_mut(slot) {
                    *time = (*time).max(message.time_us());
                }
                if !message.is_sysex()
                    && let Some(entry) = self.heard.get_mut(heard)
                {
                    *entry = (slot, message);
                    heard += 1;
                }
            }
            let Some(event) = to_event(message) else {
                continue;
            };
            match self.events.get_mut(events) {
                Some(entry) => {
                    *entry = event;
                    events += 1;
                }
                None => dropped += 1,
            }
        }
        if has_targets && received > 0 {
            self.shared.add_messages(received);
        }
        if dropped > 0 {
            self.shared.note_events_dropped(dropped);
        }
        if heard > 0 {
            self.record_heard(heard, &newest);
        }
        if events > 0
            && let (Some(graph), Some(batch)) =
                (self.graph.as_deref_mut(), self.events.get(..events))
        {
            let targets = match self.targets.graph {
                Some(id) if graph.id() == id => self.targets.nodes.as_slice(),
                _ => &[],
            };
            for &node in targets {
                graph.inject(node, batch);
            }
        }
        has_targets && self.midi.iter().any(Option::is_some)
    }

    /// Sends the first `count` messages of the drain to the control thread for the MIDI takes,
    /// each with the frame it was heard at: `heard_now − round((newest − t) · sr)`, where
    /// `heard_now` is the device's software position (the next source frame before the first
    /// callback) minus the output latency, and `newest` its input's newest arrival time in this
    /// drain (`_poll_midi`, which anchors a port's newest message at the poll).
    fn record_heard(&mut self, count: usize, newest: &[u64; MAX_MIDI_INPUTS]) {
        let Some(stream) = self.stream.as_deref() else {
            return;
        };
        let rate = f64::from(stream.sample_rate);
        let (latency, _) = stream.control.latency();
        let heard_now = stream
            .control
            .position()
            .map_or(self.src, TimelineFrame::get)
            .saturating_sub(i64::from(latency.get()));
        let mut dropped = 0_u64;
        for &(slot, event) in self.heard.iter().take(count) {
            let anchor = newest.get(slot).copied().unwrap_or(event.time_us());
            let behind = micros_to_frames(anchor.saturating_sub(event.time_us()), rate);
            let entry = RecordedMidi {
                frame: heard_now.saturating_sub(behind),
                event,
            };
            if self.recorded.push(entry).is_err() {
                dropped += 1;
            }
        }
        if dropped > 0 {
            self.shared.note_recorded_midi_dropped(dropped);
        }
    }

    /// The playing step (`_step`): lookahead, end of range, one block.
    fn play_step(&mut self, live: bool) -> Step {
        let (Some(stream), true) = (self.stream.as_deref(), self.graph.is_some()) else {
            return Step::Waiting;
        };
        let target = if live {
            stream.live_target_blocks
        } else {
            stream.target_blocks
        };
        let queued = stream.producer.queued_frames();
        if queued / 512 >= target || stream.producer.free_blocks() == 0 {
            return Step::Waiting;
        }
        if let Some(end) = self.end
            && self.src >= end
        {
            if queued == 0 {
                // The range has drained: the transport stops at its end.
                self.mode = Mode::Halted;
                self.rest = end;
                self.shared.note_ended(self.generation);
                return Step::Idle;
            }
            return Step::Waiting;
        }
        let started = Instant::now();
        let Some(tag) = self.next_block() else {
            return Step::Waiting;
        };
        self.shared.add_render(started.elapsed());
        let clicked = self.force_clicks || self.shared.metronome();
        if clicked
            && let (Some(graph), Some(stream)) = (self.graph.as_deref(), self.stream.as_deref())
        {
            let parts = self.parts.get(..self.parts_len).unwrap_or(&[]);
            self.metronome
                .add(graph.timeline(), parts, &stream.clicks, &mut self.mix);
            self.shared.set_clicks_dropped(self.metronome.dropped());
        }
        to_f32(&self.mix, &mut self.block);
        self.write(tag, false, clicked);
        if let Some(stream) = self.stream.as_deref() {
            if self.awaiting_first {
                // Only now can an empty ring be an underrun.
                stream
                    .producer
                    .set_playing_generation(Some(self.generation));
                self.awaiting_first = false;
            }
            if self.end.is_some_and(|end| self.src >= end) {
                // The last block of the range is written; the ring drains without
                // counting underruns.
                stream.producer.set_playing_generation(None);
            }
        }
        self.push_meters(tag);
        Step::Wrote
    }

    /// Assembles the next block from the carry (`_next_block`); returns its tag.
    fn next_block(&mut self) -> Option<i64> {
        let graph = self.graph.as_deref_mut()?;
        let latency = i64::try_from(graph.latency()).unwrap_or(i64::MAX);
        while self.carry.len < BLOCK {
            let src = self.src;
            let installed = take_media(&mut self.feed, graph, self.media_index, src);
            if !installed && graph.audio_nodes() > 0 {
                self.shared.note_media_late();
            }
            let output = graph.process(src, true);
            let (kept, next) = advance(src, self.looped);
            if !self.carry.push(
                output.get(..kept).unwrap_or(&[]),
                src.saturating_sub(latency),
            ) {
                return None;
            }
            if next != src.saturating_add(BLOCK_I64) {
                // A loop wrap starts a new pass.
                self.pass = self.pass.wrapping_add(1);
            }
            self.src = next;
            self.media_index = self.media_index.wrapping_add(1);
            self.shared.set_progress(self.generation, self.media_index);
        }
        self.parts_len = self.carry.take_block(&mut self.mix, &mut self.parts);
        self.parts
            .first()
            .filter(|_| self.parts_len > 0)
            .map(|part| part.tag)
    }

    /// The idle step (`_idle_step`): live input alone at the resting position.
    fn idle_step(&mut self) -> Step {
        let (Some(stream), Some(graph)) = (self.stream.as_deref(), self.graph.as_deref_mut())
        else {
            return Step::Idle;
        };
        let queued = stream.producer.queued_frames();
        if queued / 512 >= stream.live_target_blocks || stream.producer.free_blocks() == 0 {
            return Step::Idle;
        }
        let output = graph.process_idle(self.rest);
        for (y, x) in self.mix.iter_mut().zip(output) {
            *y = *x;
        }
        self.parts_len = 0;
        to_f32(&self.mix, &mut self.block);
        self.write(self.rest, true, false);
        Step::Wrote
    }

    /// Writes the `f32` block with `tag` under the current generation.
    fn write(&mut self, tag: i64, idle: bool, clicked: bool) {
        let Some(stream) = self.stream.as_deref_mut() else {
            return;
        };
        if stream
            .producer
            .push_block(&self.block, TimelineFrame::new(tag), self.generation)
            .is_err()
        {
            self.shared.note_push_error();
        }
        self.blocks_written = self.blocks_written.wrapping_add(1);
        self.written = Some(Written {
            tag,
            generation: self.generation,
            parts: self.parts,
            parts_len: self.parts_len,
            clicked,
            idle,
        });
    }

    /// Posts the meters and reports of the block just written.
    fn push_meters(&mut self, tag: i64) {
        let Some(graph) = self.graph.as_deref() else {
            return;
        };
        let mut dropped = 0_u64;
        for (index, meter) in graph.meters().iter().enumerate() {
            let entry = MeterEntry {
                block: self.blocks_written,
                tag,
                pass: self.pass,
                graph: graph.id(),
                node: NodeIndex::new(index),
                meter: *meter,
            };
            if self.meters.push(entry).is_err() {
                dropped += 1;
            }
        }
        for (device, report) in graph.reports() {
            let entry = ReportEntry {
                block: self.blocks_written,
                graph: graph.id(),
                device,
                report,
            };
            if self.reports.push(entry).is_err() {
                dropped += 1;
            }
        }
        if dropped > 0 {
            self.shared.note_meters_dropped(dropped);
        }
    }

    fn post_setup_requests(&mut self) {
        let tickets = &mut self.tickets;
        if let Some(graph) = self.graph.as_deref_mut() {
            graph.post_setup_requests(|ticket| tickets.push(ticket).is_ok());
        }
    }
}

/// Installs the prefetched clip audio of the block at `index` of the playing sequence (source
/// frame `frame`) into `graph`. Older blocks are returned unused; a block that is not there yet
/// is not waited for. Returns whether the clip audio was installed.
fn take_media(
    feed: &mut Option<MediaFeed>,
    graph: &mut RenderGraph,
    index: u64,
    frame: i64,
) -> bool {
    let Some(feed) = feed.as_mut() else {
        return false;
    };
    loop {
        let Some(head) = feed.receiver.peek() else {
            return false;
        };
        if head.index > index {
            return false;
        }
        let wanted = head.index == index && head.frame == frame;
        let key = graph.media_key();
        let taken = feed.receiver.receive(|block| {
            wanted && Some(block.key) == key && graph.install_media(frame, &block.tracks)
        });
        match taken {
            // The block was used, or it was the right position but did not fit the graph.
            Some(installed) if wanted => return installed,
            // A stale block went back; look at the next one.
            Some(_) => {}
            // The return ring is full: the prefetch thread has not taken its blocks back.
            None => return false,
        }
    }
}

/// Applies a change of the MIDI inputs by moves only: the inputs of removed ports go into the
/// change's `removed` slots, the added inputs into free slots (one that finds no free slot
/// stays in the change and goes back).
fn change_inputs(slots: &mut [MidiSlot; MAX_MIDI_INPUTS], change: &mut MidiChange) {
    let remove = change.remove;
    for (slot, removed) in slots.iter_mut().zip(change.removed.iter_mut()) {
        if slot
            .as_ref()
            .is_some_and(|(port, _)| MidiChange::marks(&remove, *port))
        {
            std::mem::swap(slot, removed);
        }
    }
    for added in change.add.iter_mut().filter(|added| added.is_some()) {
        if let Some(free) = slots.iter_mut().find(|slot| slot.is_none()) {
            std::mem::swap(free, added);
        }
    }
}

/// The next MIDI message of all inputs by arrival time, with the slot of its input; on equal
/// times the earlier input comes first (a stable merge).
fn next_message(inputs: &mut [MidiSlot]) -> Option<(usize, MidiEvent)> {
    let mut best: Option<(usize, u64)> = None;
    for (index, input) in inputs.iter().enumerate() {
        if let Some((_, input)) = input
            && let Some(event) = input.peek()
            && best.is_none_or(|(_, time)| event.time_us() < time)
        {
            best = Some((index, event.time_us()));
        }
    }
    let (index, _) = best?;
    let event = inputs.get_mut(index)?.as_mut()?.1.pop()?;
    Some((index, event))
}

/// `round(Δt · sr)` for a difference of `micros` microseconds, rounded half to even as Python's
/// `round`.
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "MIDI arrival differences are far below 2^53 µs, and their frame counts fit an i64"
)]
fn micros_to_frames(micros: u64, rate: f64) -> i64 {
    (micros as f64 * 1e-6 * rate).round_ties_even() as i64
}

/// A live MIDI message as an instrument event at the start of the next block (`_to_event`);
/// `None` for the kinds instruments do not take.
fn to_event(message: MidiEvent) -> Option<InstrumentEvent> {
    let channel = message.channel();
    let event = |kind, pitch, velocity, value, number| InstrumentEvent {
        offset: 0,
        kind,
        pitch,
        velocity,
        channel,
        value,
        number,
    };
    match message.kind() {
        MidiKind::NoteOn => Some(event(
            EventKind::NoteOn,
            message.note(),
            message.velocity(),
            0,
            0,
        )),
        MidiKind::NoteOff => Some(event(EventKind::NoteOff, message.note(), 0, 0, 0)),
        MidiKind::Cc => Some(event(EventKind::Cc, 0, 0, message.value(), message.note())),
        MidiKind::PitchBend => Some(event(EventKind::PitchBend, 0, 0, message.value(), 0)),
        MidiKind::ChannelPressure => {
            Some(event(EventKind::ChannelPressure, 0, 0, message.value(), 0))
        }
        MidiKind::PolyPressure | MidiKind::Program | MidiKind::Other => None,
    }
}

/// `out.astype(np.float32)`: each sample rounded to the nearest `f32`.
fn to_f32(mix: &[[f64; 2]], block: &mut [f32]) {
    let (frames, _) = block.as_chunks_mut::<2>();
    for (y, x) in frames.iter_mut().zip(mix) {
        *y = [narrow(x[0]), narrow(x[1])];
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "NumPy's float64 to float32 cast: round to nearest, ties to even"
)]
fn narrow(x: f64) -> f32 {
    x as f32
}

#[cfg(test)]
mod tests {
    use sonara_dsp::processor::EventKind;
    use sonara_rt::{MidiEvent, midi_channel};

    use super::{Carry, Part, change_inputs, next_message, to_event};
    use crate::player::clicks::MAX_PARTS;
    use crate::player::message::{MAX_MIDI_INPUTS, MidiChange, MidiSlot};

    /// The carry of the golden `loop-metronome` wrap: 324 frames before the loop end, then 512
    /// from the loop start. The block takes 188 of them; the next block starts at 96 148 and
    /// merges the contiguous chunks.
    #[test]
    fn the_carry_cuts_and_tags_like_the_prototype() {
        let mut carry = Carry::new();
        let mut out = [[0.0; 2]; 512];
        let mut parts = [Part::default(); MAX_PARTS];
        assert!(carry.push(&[[1.0; 2]; 324], 143_576));
        assert!(carry.push(&[[2.0; 2]; 512], 95_960));
        assert_eq!(carry.take_block(&mut out, &mut parts), 2);
        assert_eq!(
            parts[..2],
            [
                Part {
                    tag: 143_576,
                    frames: 324
                },
                Part {
                    tag: 95_960,
                    frames: 188
                }
            ]
        );
        assert_eq!((out[323], out[324]), ([1.0; 2], [2.0; 2]));
        assert_eq!(carry.len, 324);
        assert!(carry.push(&[[3.0; 2]; 512], 96_472));
        assert_eq!(carry.take_block(&mut out, &mut parts), 1);
        assert_eq!(
            parts[0],
            Part {
                tag: 96_148,
                frames: 512
            }
        );
        assert_eq!((out[323], out[324]), ([2.0; 2], [3.0; 2]));
    }

    /// Several inputs merge by time, ties in input order (the prototype's stable sort).
    #[test]
    fn midi_inputs_merge_by_time() {
        let (mut a_in, a) = midi_channel(8);
        let (mut b_in, b) = midi_channel(8);
        let note = |time, pitch| MidiEvent::new(time, 0, &[0x90, pitch, 100]).unwrap();
        assert!(a_in.push(note(5, 1)) && a_in.push(note(9, 2)));
        assert!(b_in.push(note(5, 3)) && b_in.push(note(7, 4)));
        let mut inputs = [Some((0, a)), None, Some((1, b))];
        let notes: Vec<u8> = std::iter::from_fn(|| next_message(&mut inputs))
            .map(|(_, e)| e.note())
            .collect();
        assert_eq!(notes, [1, 3, 4, 2]);
    }

    /// Inputs come and go by moves: a removed port's ring goes into the change, an added one
    /// into a free slot; when every slot is taken, the input stays in the change and goes back.
    #[test]
    fn midi_inputs_come_and_go_by_moves() {
        let mut slots: [MidiSlot; MAX_MIDI_INPUTS] = std::array::from_fn(|_| None);
        let mut change = MidiChange::new();
        for (port, slot) in (0_u8..).zip(change.add.iter_mut().take(3)) {
            *slot = Some((port, midi_channel(4).1));
        }
        change_inputs(&mut slots, &mut change);
        assert!(change.add.iter().all(Option::is_none));
        assert_eq!(
            slots.iter().flatten().map(|(p, _)| *p).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        let mut change = MidiChange::new();
        change.remove_port(1);
        change_inputs(&mut slots, &mut change);
        assert_eq!(
            change
                .removed
                .iter()
                .flatten()
                .map(|(p, _)| *p)
                .collect::<Vec<_>>(),
            [1]
        );
        assert_eq!(
            slots.iter().flatten().map(|(p, _)| *p).collect::<Vec<_>>(),
            [0, 2]
        );
        let mut change = MidiChange::new();
        for (port, slot) in (10_u8..).zip(change.add.iter_mut()) {
            *slot = Some((port, midi_channel(4).1));
        }
        change_inputs(&mut slots, &mut change);
        assert_eq!(slots.iter().flatten().count(), MAX_MIDI_INPUTS);
        assert_eq!(
            change.add.iter().flatten().count(),
            2,
            "no free slot for two of them"
        );
    }

    /// `_to_event`: notes, controllers with their number, bend and pressure with their value;
    /// other messages are dropped.
    #[test]
    fn midi_messages_become_events_at_offset_zero() {
        let event = |bytes: &[u8]| to_event(MidiEvent::new(0, 0, bytes).unwrap());
        let on = event(&[0x91, 60, 100]).unwrap();
        assert_eq!(
            (on.kind, on.pitch, on.velocity, on.channel, on.offset),
            (EventKind::NoteOn, 60, 100, 1, 0)
        );
        let off = event(&[0x90, 60, 0]).unwrap();
        assert_eq!(
            (off.kind, off.pitch, off.velocity),
            (EventKind::NoteOff, 60, 0)
        );
        let cc = event(&[0xB0, 7, 99]).unwrap();
        assert_eq!((cc.kind, cc.number, cc.value), (EventKind::Cc, 7, 99));
        let bend = event(&[0xE0, 0x7F, 0x7F]).unwrap();
        assert_eq!((bend.kind, bend.value), (EventKind::PitchBend, 8191));
        let pressure = event(&[0xD0, 42]).unwrap();
        assert_eq!(
            (pressure.kind, pressure.value),
            (EventKind::ChannelPressure, 42)
        );
        assert!(event(&[0xC0, 5]).is_none());
        assert!(to_event(MidiEvent::sysex(0, 0)).is_none());
    }
}
