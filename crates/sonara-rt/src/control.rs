//! The stream's control handle: everything other threads do to a running stream.
//!
//! Every call is an atomic store, load or read-modify-write: none waits for the device thread
//! (RT-06).

use crate::stream::Shared;
use crate::sync::{Arc, Ordering};
use crate::{Generation, LatencyFrames, StatsSnapshot, TakeId, TimelineFrame};

/// The handle for the player's control thread, the agent and GUI threads, and the device
/// notifications. Clones share one stream.
#[derive(Clone)]
pub struct StreamControl {
    shared: Arc<Shared>,
}

impl StreamControl {
    pub(crate) fn new(shared: Arc<Shared>) -> Self {
        Self { shared }
    }

    /// Makes every block older than `generation` stale. Stale blocks stop counting as queued at
    /// once; the callback fades the playing one out and discards them.
    pub fn flush(&self, generation: Generation) {
        self.shared.playback.flush(generation);
    }

    /// Sets the player's flag: only while it is set does an empty ring count as an underrun.
    /// `true` arms it for the generation that is current now, so a later
    /// [`flush`](Self::flush) disarms it; the player arms its generations itself
    /// ([`BlockProducer::set_playing_generation`](crate::BlockProducer::set_playing_generation)).
    pub fn set_playing(&self, playing: bool) {
        self.shared.playback.set_playing(playing);
    }

    /// The player's flag: armed for a generation that is still current.
    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.shared.playback.is_playing()
    }

    /// Sets the direct monitor gain. Negative gains and NaN become 0, as in the prototype.
    pub fn set_monitor(&self, gain: f32) {
        // Relaxed ↔ the Relaxed load in `DeviceCallback::monitor_gain`: a standalone value.
        self.shared
            .monitor_gain
            .store(gain.max(0.0).to_bits(), Ordering::Relaxed);
    }

    /// Arms a new capture take and returns its id. The take starts at the first frame of the
    /// current generation that a callback plays; flush and push that generation first.
    #[must_use]
    pub fn arm_capture(&self) -> TakeId {
        self.shared.capture.arm()
    }

    /// Ends the current capture take. Frames already captured stay readable.
    pub fn disarm_capture(&self) {
        self.shared.capture.disarm();
    }

    /// Every stats key. Resets the peaks: they are the maxima since the previous `stats()` call,
    /// and no other call resets them.
    #[must_use]
    pub fn stats(&self) -> StatsSnapshot {
        self.shared.stats.snapshot(&self.shared.playback)
    }

    /// The software position: one past the last timeline frame handed to the device, `None`
    /// before the first. It is not the hardware position (RT-07). Leaves the peaks alone.
    #[must_use]
    pub fn position(&self) -> Option<TimelineFrame> {
        self.shared.stats.timeline_frame()
    }

    /// The output and input latency estimates. Leaves the peaks alone.
    #[must_use]
    pub fn latency(&self) -> (LatencyFrames, LatencyFrames) {
        self.shared.stats.latency()
    }

    /// Frames of the current generation not yet played. Leaves the peaks alone. From a thread
    /// other than the producer's this is an estimate while blocks are being pushed.
    #[must_use]
    pub fn queued_frames(&self) -> u64 {
        self.shared.playback.queued_frames()
    }

    /// Records whether the device is started. Clear it before stopping the device, so that the
    /// stop is not counted as a device error.
    pub fn set_running(&self, running: bool) {
        self.shared.stats.set_running(running);
    }

    /// Records the opened device's sample rate and period (0 and 0 without a device).
    pub fn set_device_format(&self, sample_rate: u32, buffer_frames: u32) {
        self.shared
            .stats
            .set_device_format(sample_rate, buffer_frames);
    }

    /// Records the output and input latency estimates.
    pub fn set_latency(&self, output: LatencyFrames, input: LatencyFrames) {
        self.shared.stats.set_latency(output, input);
    }

    /// A device notification: the device stopped. Counted as a device error only while the
    /// stream is running.
    pub fn note_device_stopped(&self) {
        self.shared.stats.note_device_stopped();
    }

    /// A device notification: the default device was rerouted.
    pub fn note_rerouted(&self) {
        self.shared.stats.note_rerouted();
    }

    /// A device notification: an interruption began. Counted as a device error.
    pub fn note_interruption(&self) {
        self.shared.stats.note_interruption();
    }
}
