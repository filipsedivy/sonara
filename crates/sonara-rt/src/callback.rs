//! The device callback's handle: the consumer of the playback ring, the producer of the
//! capture ring, and the meters and counters.

use crate::capture::CaptureWriter;
use crate::playback::{BlockReader, Fill};
use crate::stream::Shared;
use crate::sync::{Arc, Ordering};

/// The device callback's half of a stream.
///
/// Real-time: every method neither allocates nor blocks nor panics. Drop it off the real-time
/// thread once the device has stopped (RT-09).
pub struct DeviceCallback {
    shared: Arc<Shared>,
    playback: BlockReader,
    capture: Option<CaptureWriter>,
}

impl DeviceCallback {
    pub(crate) fn new(
        shared: Arc<Shared>,
        playback: BlockReader,
        capture: Option<CaptureWriter>,
    ) -> Self {
        Self {
            shared,
            playback,
            capture,
        }
    }

    /// Fills `out` (interleaved, the stream's output channel count) from the playback ring:
    /// plays the current generation, fades stale audio out and new material in, and silences
    /// what the ring cannot fill. Counts at most one underrun, and only while the player is
    /// playing; updates the software position.
    ///
    /// A pure function of the ring's contents, so tests drive the callback without a device.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn fill(&mut self, out: &mut [f32]) -> Fill {
        self.playback.fill(&self.shared, out)
    }

    /// Captures `input` (interleaved, the stream's input channel count) while a take is armed.
    /// `fill` is this callback's [`fill`](Self::fill): a take starts at its first
    /// current-generation frame, and the input before it is dropped. Counts at most one
    /// overrun when the frames do not fit; those are dropped.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn capture(&mut self, input: &[f32], fill: &Fill) {
        if let Some(capture) = self.capture.as_mut() {
            capture.write(&self.shared, input, fill);
        }
    }

    /// The direct monitor gain the callback adds input to the output with.
    #[must_use]
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn monitor_gain(&self) -> f32 {
        // Relaxed ↔ the Relaxed store in `StreamControl::set_monitor`: a standalone value.
        f32::from_bits(self.shared.monitor_gain.load(Ordering::Relaxed))
    }

    /// Merges the output's peaks (after the direct monitor) into the stats.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn meter_output(&self, out: &[f32]) {
        self.shared.stats.meter_output(out);
    }

    /// Merges the input's peaks into the stats.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn meter_input(&self, input: &[f32]) {
        self.shared.stats.meter_input(input);
    }

    /// Counts a finished callback of `frames` frames and records its load: the processing
    /// time over the period, measured by the caller.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn end_callback(&self, frames: usize, load: f32) {
        let frames = u64::try_from(frames).unwrap_or(u64::MAX);
        self.shared.stats.end_callback(frames, load);
    }
}
