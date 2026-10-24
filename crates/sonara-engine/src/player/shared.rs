//! Real-time: the render thread writes these atomics (see docs/audio-engine.md); the control
//! thread reads them.
//!
//! What the render thread reports to the control thread (RT-08, RT-12): its counters, the end
//! of a play range, the progress of the playing sequence (where a new media feed starts) and the
//! project's metronome switch. Each value is standalone unless its comment names a pair.
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

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use sonara_rt::Generation;

/// The high half of a packed value that is set.
const SET: u64 = 1 << 32;

/// The render thread's reports.
#[derive(Debug, Default)]
pub(crate) struct Shared {
    /// Time spent in `_next_block` (the graph and the carry), in nanoseconds.
    render_nanos: AtomicU64,
    /// Blocks rendered by the player loop (the idle step excluded).
    blocks_rendered: AtomicU64,
    /// MIDI messages received while there were targets.
    messages_received: AtomicU64,
    /// `SET | generation` once the play range of `generation` has ended and drained; 0 before.
    ended: AtomicU64,
    /// The playing sequence's generation (high half) and the index of its next block (low
    /// half, wrapping).
    progress: AtomicU64,
    /// The project's metronome switch, set by the control thread.
    metronome: AtomicBool,
    /// Played blocks whose clip audio had not been read ahead (silent clips, RT-12).
    media_late: AtomicU64,
    /// Meter and report entries a full queue dropped.
    meters_dropped: AtomicU64,
    /// Live MIDI events beyond one step's buffer.
    events_dropped: AtomicU64,
    /// Blocks the playback ring refused although it had room (none, by the step's check).
    push_errors: AtomicU64,
    /// Clicks dropped because too many sounded at once.
    clicks_dropped: AtomicU64,
    /// Recorded MIDI messages a full queue dropped.
    recorded_midi_dropped: AtomicU64,
}

/// A snapshot of the render thread's counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderCounters {
    /// Blocks rendered by the player loop.
    pub blocks_rendered: u64,
    /// Time spent rendering them (the graph and the carry), in nanoseconds.
    pub render_nanos: u64,
    /// Played blocks whose clip audio was not ready: the clips were silent (RT-12).
    pub media_late: u64,
    /// Meter and report entries dropped because the control thread fell behind.
    pub meters_dropped: u64,
    /// Live MIDI events dropped because one step received more than an instrument holds.
    pub events_dropped: u64,
    /// Blocks the playback ring refused.
    pub push_errors: u64,
    /// Metronome clicks dropped because too many sounded at once.
    pub clicks_dropped: u64,
    /// MIDI messages a recording could not keep because the control thread fell behind.
    pub recorded_midi_dropped: u64,
}

fn unpack(value: u64) -> (u32, u32) {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "keeps exactly the low 32 bits"
    )]
    let low = value as u32;
    ((value >> 32) as u32, low)
}

impl Shared {
    // Writers on the render thread. Relaxed: standalone counters and values; nothing else is
    // published through them.

    pub(crate) fn add_render(&self, elapsed: Duration) {
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.render_nanos.fetch_add(nanos, Ordering::Relaxed);
        self.blocks_rendered.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_messages(&self, count: u64) {
        self.messages_received.fetch_add(count, Ordering::Relaxed);
    }

    pub(crate) fn note_ended(&self, generation: Generation) {
        self.ended
            .store(SET | u64::from(generation.get()), Ordering::Relaxed);
    }

    pub(crate) fn set_progress(&self, generation: Generation, index: u64) {
        let low = index & u64::from(u32::MAX);
        self.progress
            .store((u64::from(generation.get()) << 32) | low, Ordering::Relaxed);
    }

    pub(crate) fn note_media_late(&self) {
        self.media_late.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn note_meters_dropped(&self, count: u64) {
        self.meters_dropped.fetch_add(count, Ordering::Relaxed);
    }

    pub(crate) fn note_events_dropped(&self, count: u64) {
        self.events_dropped.fetch_add(count, Ordering::Relaxed);
    }

    pub(crate) fn note_push_error(&self) {
        self.push_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn note_recorded_midi_dropped(&self, count: u64) {
        self.recorded_midi_dropped
            .fetch_add(count, Ordering::Relaxed);
    }

    pub(crate) fn set_clicks_dropped(&self, total: u64) {
        self.clicks_dropped.store(total, Ordering::Relaxed);
    }

    // The control thread's side. Relaxed ↔ the Relaxed writers above.

    pub(crate) fn set_metronome(&self, on: bool) {
        self.metronome.store(on, Ordering::Relaxed);
    }

    pub(crate) fn metronome(&self) -> bool {
        self.metronome.load(Ordering::Relaxed)
    }

    pub(crate) fn messages_received(&self) -> u64 {
        self.messages_received.load(Ordering::Relaxed)
    }

    /// The generation whose play range ended last, if any.
    pub(crate) fn ended(&self) -> Option<Generation> {
        let (set, generation) = unpack(self.ended.load(Ordering::Relaxed));
        (set == 1).then_some(Generation::new(generation))
    }

    /// The generation of the playing sequence and the low 32 bits of its next block's index.
    pub(crate) fn progress(&self) -> (Generation, u32) {
        let (generation, index) = unpack(self.progress.load(Ordering::Relaxed));
        (Generation::new(generation), index)
    }

    pub(crate) fn counters(&self) -> RenderCounters {
        RenderCounters {
            blocks_rendered: self.blocks_rendered.load(Ordering::Relaxed),
            render_nanos: self.render_nanos.load(Ordering::Relaxed),
            media_late: self.media_late.load(Ordering::Relaxed),
            meters_dropped: self.meters_dropped.load(Ordering::Relaxed),
            events_dropped: self.events_dropped.load(Ordering::Relaxed),
            push_errors: self.push_errors.load(Ordering::Relaxed),
            clicks_dropped: self.clicks_dropped.load(Ordering::Relaxed),
            recorded_midi_dropped: self.recorded_midi_dropped.load(Ordering::Relaxed),
        }
    }
}
