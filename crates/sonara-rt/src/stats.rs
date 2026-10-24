//! The stream's stats: every key of the prototype's `Engine::stats()` as an atomic (RT-08).
//!
//! The callback and the device notifications write them; any thread reads them through
//! [`StreamControl::stats`](crate::StreamControl::stats). Peaks are maxima since the previous
//! `stats()` call, and only that call resets them: the producer's own queries are separate, so
//! internal polling never consumes them.

use std::num::NonZeroUsize;

use crate::playback::Protocol;
use crate::sync::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use crate::{LatencyFrames, METERED_CHANNELS, TimelineFrame};

/// The software position before the first frame is played (the prototype's −1).
const NO_FRAME: i64 = i64::MIN;

/// Per-channel peaks of up to [`METERED_CHANNELS`] channels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Peaks {
    values: [f32; METERED_CHANNELS],
    len: usize,
}

impl Peaks {
    /// One peak per metered channel: the first `min(channels, 8)` channels.
    #[must_use]
    pub fn as_slice(&self) -> &[f32] {
        self.values.get(..self.len).unwrap_or(&[])
    }
}

/// One reading of every stats key (the prototype's `stats()` dict, `native/src/engine.cpp`).
///
/// Field names equal the keys. The software position and the latency estimates have their
/// own types and are never mixed (RT-07).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatsSnapshot {
    /// The device is started.
    pub running: bool,
    /// The player's flag that arms underrun counting.
    pub playing: bool,
    /// The device's sample rate; 0 without a device.
    pub sample_rate: u32,
    /// The device's period in frames; 0 without a device.
    pub buffer_frames: u32,
    /// Frames the device requested since the stream started.
    pub callback_frames: u64,
    /// Device callbacks since the stream started.
    pub callbacks: u64,
    /// Callbacks that found the playback ring empty while the player was playing.
    pub underruns: u64,
    /// Callbacks whose capture did not fit into the capture ring.
    pub overruns: u64,
    /// Frames of the current generation not yet played.
    pub queued_frames: u64,
    /// One past the last timeline frame handed to the device; `None` until the first frame is
    /// played (the prototype's −1). A software position, not the hardware position (RT-07).
    pub timeline_frame: Option<TimelineFrame>,
    /// Output latency estimate: the backend's period × periods.
    pub latency_frames: LatencyFrames,
    /// Input latency estimate; 0 without input.
    pub input_latency_frames: LatencyFrames,
    /// Output peaks since the previous `stats()` call, after the direct monitor.
    pub peak: Peaks,
    /// Input peaks since the previous `stats()` call.
    pub input_peak: Peaks,
    /// The last callback's processing time over its period.
    pub callback_load: f32,
    /// Device stops while running, and interruptions.
    pub device_errors: u64,
    /// The device stopped while running.
    pub stopped_by_device: bool,
    /// Default-device reroutes (unlike the prototype's `StreamStats`).
    pub rerouted: u64,
}

/// The atomics behind [`StatsSnapshot`].
///
/// Every counter and value here is standalone: no other data is published with it, so the
/// accesses are Relaxed. Each read-modify-write is atomic on its own, so no count and no peak
/// is ever lost, whatever the ordering.
pub(crate) struct Stats {
    output_channels: NonZeroUsize,
    input_channels: usize,
    running: AtomicBool,
    sample_rate: AtomicU32,
    buffer_frames: AtomicU32,
    callback_frames: AtomicU64,
    callbacks: AtomicU64,
    underruns: AtomicU64,
    overruns: AtomicU64,
    timeline_frame: AtomicI64,
    latency_frames: AtomicU32,
    input_latency_frames: AtomicU32,
    peak: [AtomicU32; METERED_CHANNELS],
    input_peak: [AtomicU32; METERED_CHANNELS],
    callback_load: AtomicU32,
    device_errors: AtomicU64,
    stopped_by_device: AtomicBool,
    rerouted: AtomicU64,
}

impl Stats {
    pub(crate) fn new(output_channels: NonZeroUsize, input_channels: usize) -> Self {
        Self {
            output_channels,
            input_channels,
            running: AtomicBool::new(false),
            sample_rate: AtomicU32::new(0),
            buffer_frames: AtomicU32::new(0),
            callback_frames: AtomicU64::new(0),
            callbacks: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            overruns: AtomicU64::new(0),
            timeline_frame: AtomicI64::new(NO_FRAME),
            latency_frames: AtomicU32::new(0),
            input_latency_frames: AtomicU32::new(0),
            peak: std::array::from_fn(|_| AtomicU32::new(0)),
            input_peak: std::array::from_fn(|_| AtomicU32::new(0)),
            callback_load: AtomicU32::new(0),
            device_errors: AtomicU64::new(0),
            stopped_by_device: AtomicBool::new(false),
            rerouted: AtomicU64::new(0),
        }
    }

    // Writers on the callback thread. Relaxed ↔ the Relaxed loads in `snapshot`: standalone
    // values (see the type's comment).

    pub(crate) fn count_underrun(&self) {
        self.underruns.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn count_overrun(&self) {
        self.overruns.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn set_timeline_frame(&self, frame: TimelineFrame) {
        self.timeline_frame.store(frame.get(), Ordering::Relaxed);
    }

    pub(crate) fn end_callback(&self, frames: u64, load: f32) {
        self.callback_frames.fetch_add(frames, Ordering::Relaxed);
        self.callbacks.fetch_add(1, Ordering::Relaxed);
        self.callback_load.store(load.to_bits(), Ordering::Relaxed);
    }

    pub(crate) fn meter_output(&self, out: &[f32]) {
        merge_peaks(&self.peak, out, self.output_channels);
    }

    pub(crate) fn meter_input(&self, input: &[f32]) {
        if let Some(channels) = NonZeroUsize::new(self.input_channels) {
            merge_peaks(&self.input_peak, input, channels);
        }
    }

    // Writers on control and notification threads.

    pub(crate) fn set_running(&self, running: bool) {
        self.running.store(running, Ordering::Relaxed);
    }

    pub(crate) fn set_device_format(&self, sample_rate: u32, buffer_frames: u32) {
        self.sample_rate.store(sample_rate, Ordering::Relaxed);
        self.buffer_frames.store(buffer_frames, Ordering::Relaxed);
    }

    pub(crate) fn set_latency(&self, output: LatencyFrames, input: LatencyFrames) {
        self.latency_frames.store(output.get(), Ordering::Relaxed);
        self.input_latency_frames
            .store(input.get(), Ordering::Relaxed);
    }

    pub(crate) fn note_device_stopped(&self) {
        // Our own stop clears `running` first, so it is not counted (the prototype's rule).
        if self.running.load(Ordering::Relaxed) {
            self.stopped_by_device.store(true, Ordering::Relaxed);
            self.device_errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn note_rerouted(&self) {
        self.rerouted.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn note_interruption(&self) {
        self.device_errors.fetch_add(1, Ordering::Relaxed);
    }

    // Readers, on any thread. Relaxed ↔ the Relaxed writers above: standalone values.

    pub(crate) fn timeline_frame(&self) -> Option<TimelineFrame> {
        let frame = self.timeline_frame.load(Ordering::Relaxed);
        (frame != NO_FRAME).then_some(TimelineFrame::new(frame))
    }

    pub(crate) fn latency(&self) -> (LatencyFrames, LatencyFrames) {
        (
            LatencyFrames::new(self.latency_frames.load(Ordering::Relaxed)),
            LatencyFrames::new(self.input_latency_frames.load(Ordering::Relaxed)),
        )
    }

    /// Reads every key and resets the peaks (the only reader that does: D9).
    pub(crate) fn snapshot(&self, playback: &Protocol) -> StatsSnapshot {
        let (latency_frames, input_latency_frames) = self.latency();
        StatsSnapshot {
            running: self.running.load(Ordering::Relaxed),
            playing: playback.is_playing(),
            sample_rate: self.sample_rate.load(Ordering::Relaxed),
            buffer_frames: self.buffer_frames.load(Ordering::Relaxed),
            callback_frames: self.callback_frames.load(Ordering::Relaxed),
            callbacks: self.callbacks.load(Ordering::Relaxed),
            underruns: self.underruns.load(Ordering::Relaxed),
            overruns: self.overruns.load(Ordering::Relaxed),
            queued_frames: playback.queued_frames(),
            timeline_frame: self.timeline_frame(),
            latency_frames,
            input_latency_frames,
            peak: take_peaks(&self.peak, self.output_channels.get()),
            input_peak: take_peaks(&self.input_peak, self.input_channels),
            callback_load: f32::from_bits(self.callback_load.load(Ordering::Relaxed)),
            device_errors: self.device_errors.load(Ordering::Relaxed),
            stopped_by_device: self.stopped_by_device.load(Ordering::Relaxed),
            rerouted: self.rerouted.load(Ordering::Relaxed),
        }
    }
}

/// Merges the per-channel maxima of `|x|` over the interleaved `data` into the peaks of its
/// first [`METERED_CHANNELS`] channels.
fn merge_peaks(peaks: &[AtomicU32; METERED_CHANNELS], data: &[f32], channels: NonZeroUsize) {
    let mut maxima = [0.0_f32; METERED_CHANNELS];
    for frame in data.chunks_exact(channels.get()) {
        for (max, sample) in maxima.iter_mut().zip(frame) {
            let level = sample.abs();
            // NaN compares false and is skipped, as `std::max` skips it in the prototype.
            if level > *max {
                *max = level;
            }
        }
    }
    for (peak, max) in peaks.iter().zip(maxima) {
        if max > 0.0 {
            // For non-negative floats (NaN excluded above) the order of the bit patterns is the
            // order of the values, so `fetch_max` on the bits is the prototype's CAS max loop.
            // Relaxed ↔ the Relaxed `swap` in `take_peaks`: both are read-modify-writes of this
            // one atomic, so no peak is lost between two readings.
            peak.fetch_max(max.to_bits(), Ordering::Relaxed);
        }
    }
}

/// Reads and resets the peaks of the first `min(channels, 8)` channels.
fn take_peaks(peaks: &[AtomicU32; METERED_CHANNELS], channels: usize) -> Peaks {
    let len = channels.min(METERED_CHANNELS);
    let mut values = [0.0_f32; METERED_CHANNELS];
    for (value, peak) in values.iter_mut().zip(peaks).take(len) {
        // Relaxed ↔ the Relaxed `fetch_max` in `merge_peaks`.
        *value = f32::from_bits(peak.swap(0, Ordering::Relaxed));
    }
    Peaks { values, len }
}
