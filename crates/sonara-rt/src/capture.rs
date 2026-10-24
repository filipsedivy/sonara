//! The capture ring: device input for recording.
//!
//! The device callback copies input frames into an `rtrb` ring of interleaved `f32` samples;
//! the take writer thread reads them. [`StreamControl::arm_capture`](crate::StreamControl)
//! starts a take. The callback starts it at the first frame of the current generation that the
//! same callback plays and drops the input before it; the take's capture start is that
//! frame's timeline frame. Each start travels to the reader through a small ring, ahead of the
//! take's frames and with the index of its first frame, so the reader splits the frames into
//! takes exactly, however quickly takes follow each other.
//!
//! The overrun rule is the prototype's: frames that do not fit are dropped, never overwritten,
//! and a callback counts at most one overrun.

use std::num::NonZeroUsize;

use rtrb::{Consumer, Producer};

use crate::TimelineFrame;
use crate::playback::Fill;
use crate::stream::Shared;
use crate::sync::{AtomicU64, Ordering};

/// Identifies one armed take; every [`arm_capture`](crate::StreamControl::arm_capture) returns
/// a new one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TakeId(u64);

impl TakeId {
    /// The raw take number; the first take is 1.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A take as the reader sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureTake {
    /// The take, as [`arm_capture`](crate::StreamControl::arm_capture) returned it.
    pub id: TakeId,
    /// Timeline frame of the take's first frame: the first current-generation frame the
    /// callback played when the take started. The take aligns at this frame minus the
    /// output and input latency.
    pub start: TimelineFrame,
}

/// Announces a take to the reader.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TakeStart {
    take: CaptureTake,
    /// Index of the take's first frame among all frames the callback ever captured.
    first_frame: u64,
}

/// Capacity of the ring of take starts. A start that does not fit waits for the next callback.
pub(crate) const PENDING_STARTS: usize = 4;

/// Bit 0 of the request: armed.
const ARMED: u64 = 1;

/// The capture request: the current take number (shifted left by one) and whether it is armed.
pub(crate) struct Protocol {
    request: AtomicU64,
}

impl Protocol {
    pub(crate) fn new() -> Self {
        Self {
            request: AtomicU64::new(0),
        }
    }

    /// Arms a new take and returns its id.
    pub(crate) fn arm(&self) -> TakeId {
        // A compare-exchange loop on a control thread (not real-time).
        // Acquire: the first guess for the compare-exchange below, which pairs the stores.
        let mut request = self.request.load(Ordering::Acquire);
        loop {
            let next = ((request >> 1).wrapping_add(1) << 1) | ARMED;
            // AcqRel ↔ the Acquire load in `CaptureWriter::write`.
            match self.request.compare_exchange_weak(
                request,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return TakeId(next >> 1),
                Err(actual) => request = actual,
            }
        }
    }

    /// Disarms the current take; the frames captured before stay readable.
    pub(crate) fn disarm(&self) {
        // Release ↔ the Acquire load in `CaptureWriter::write`.
        self.request.fetch_and(!ARMED, Ordering::Release);
    }

    fn armed_take(&self) -> Option<TakeId> {
        // Acquire ↔ the stores in `arm` and `disarm`.
        let request = self.request.load(Ordering::Acquire);
        (request & ARMED != 0).then_some(TakeId(request >> 1))
    }
}

/// The device callback's half of the capture ring.
pub(crate) struct CaptureWriter {
    frames: Producer<f32>,
    starts: Producer<TakeStart>,
    channels: NonZeroUsize,
    /// The take being captured.
    take: Option<TakeId>,
    /// Frames captured since the stream started.
    written: u64,
}

impl CaptureWriter {
    pub(crate) fn new(
        frames: Producer<f32>,
        starts: Producer<TakeStart>,
        channels: NonZeroUsize,
    ) -> Self {
        Self {
            frames,
            starts,
            channels,
            take: None,
            written: 0,
        }
    }

    pub(crate) fn write(&mut self, shared: &Shared, input: &[f32], fill: &Fill) {
        let Some(armed) = shared.capture.armed_take() else {
            self.take = None;
            return;
        };
        let from = if self.take == Some(armed) {
            0
        } else {
            // The take starts at the first current-generation frame this callback plays;
            // the input before it is dropped.
            let Some(first) = fill.first_current() else {
                return;
            };
            let start = TakeStart {
                take: CaptureTake {
                    id: armed,
                    start: first.timeline_frame,
                },
                first_frame: self.written,
            };
            // Announced before its frames (rtrb's Release ↔ the reader's Acquire). When the
            // reader is behind by several takes, the start waits for the next callback.
            if self.starts.push(start).is_err() {
                return;
            }
            self.take = Some(armed);
            first.offset
        };
        let channels = self.channels.get();
        let wanted = (input.len() / self.channels).saturating_sub(from);
        let n = wanted.min(self.frames.slots() / self.channels);
        if n < wanted {
            // At most one overrun per callback; the frames that do not fit are dropped.
            shared.stats.count_overrun();
        }
        if n == 0 {
            return;
        }
        let Some(src) = input.get(from * channels..(from + n) * channels) else {
            return;
        };
        // Cannot fail: the space was checked, and only this thread writes (SPSC).
        if self.frames.push_entire_slice(src).is_ok() {
            self.written = self
                .written
                .wrapping_add(u64::try_from(n).unwrap_or(u64::MAX));
        }
    }
}

/// The take writer's half of the capture ring.
///
/// Not real-time, but allocation-free: [`read`](Self::read) copies into the caller's buffer.
pub struct CaptureReader {
    rings: Option<ReaderRings>,
    take: Option<CaptureTake>,
    /// Frames read (or skipped) since the stream started.
    read: u64,
}

struct ReaderRings {
    frames: Consumer<f32>,
    starts: Consumer<TakeStart>,
    channels: NonZeroUsize,
}

impl CaptureReader {
    /// A reader without input channels: it never returns a frame.
    pub(crate) fn without_input() -> Self {
        Self {
            rings: None,
            take: None,
            read: 0,
        }
    }

    pub(crate) fn new(
        frames: Consumer<f32>,
        starts: Consumer<TakeStart>,
        channels: NonZeroUsize,
    ) -> Self {
        Self {
            rings: Some(ReaderRings {
                frames,
                starts,
                channels,
            }),
            take: None,
            read: 0,
        }
    }

    /// Copies captured frames into `out` (interleaved, the stream's input channel count) and
    /// says which take they belong to. One call never mixes two takes: frames left from an
    /// earlier take come first, labelled with that take, and the next call moves on.
    pub fn read(&mut self, out: &mut [f32]) -> CaptureChunk {
        let Some(rings) = self.rings.as_mut() else {
            return CaptureChunk {
                frames: 0,
                take: None,
            };
        };
        // Count the frames first: every start announced before them is then visible too.
        let available = rings.frames.slots() / rings.channels;
        // Move to the next take once every frame before its first one has been read.
        while let Ok(next) = rings.starts.peek() {
            if next.first_frame > self.read {
                break;
            }
            self.take = Some(next.take);
            if let Ok(chunk) = rings.starts.read_chunk(1) {
                chunk.commit_all();
            }
        }
        let until_next_take = rings.starts.peek().map_or(usize::MAX, |next| {
            usize::try_from(next.first_frame.saturating_sub(self.read)).unwrap_or(usize::MAX)
        });
        let n = (out.len() / rings.channels)
            .min(available)
            .min(until_next_take);
        let samples = n * rings.channels.get();
        let copied = out
            .get_mut(..samples)
            .is_some_and(|dst| rings.frames.pop_entire_slice(dst).is_ok());
        let frames = if copied { n } else { 0 };
        self.read = self
            .read
            .wrapping_add(u64::try_from(frames).unwrap_or(u64::MAX));
        CaptureChunk {
            frames,
            take: self.take,
        }
    }

    /// The stream's input channel count: the interleaving of [`read`](Self::read)'s frames.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.rings.as_ref().map_or(0, |rings| rings.channels.get())
    }
}

/// Frames returned by one [`CaptureReader::read`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureChunk {
    /// Whole frames copied to the start of the buffer.
    pub frames: usize,
    /// The take they belong to; `None` before the first take has started.
    pub take: Option<CaptureTake>,
}
