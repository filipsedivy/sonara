//! Real-time: runs on the audio device thread (see docs/audio-engine.md).
//!
//! The device callback: what `Engine::process` does in the prototype
//! (`native/src/engine.cpp`), on top of the `sonara-rt` stream. In the prototype's order:
//!
//! 1. the start time, for the load;
//! 2. [`fill`](DeviceCallback::fill): the playback ring, its fades, the underrun count and the
//!    software position;
//! 3. with input: the direct monitor (`out[c] += in[c % in_ch] × gain`), the input peaks of the
//!    first eight channels, and the capture;
//! 4. the output peaks of the first eight channels, after the monitor;
//! 5. the callback count and the load of this callback: its processing time over its period.
//!
//! The device notifications only set atomics. Errors are counted, never handled here (RT-12).
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

use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use sonara_rt::{DeviceCallback, StreamControl};

/// A device notification (miniaudio's `ma_device_notification_type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Notification {
    Started,
    Stopped,
    Rerouted,
    InterruptionBegan,
    InterruptionEnded,
    Unlocked,
}

/// One callback's buffers, as the device hands them over.
pub(crate) struct Period<'a> {
    /// Interleaved output, `frames × output_channels` samples, pre-silenced by miniaudio.
    pub(crate) output: &'a mut [f32],
    pub(crate) output_channels: u32,
    /// Interleaved input, `frames × input_channels` samples; `None` without input.
    pub(crate) input: Option<&'a [f32]>,
    pub(crate) input_channels: u32,
    pub(crate) frames: u32,
    /// The device's sample rate.
    pub(crate) sample_rate: u32,
}

/// The device callback's state: the stream's callback handle and its channel counts.
pub(crate) struct DeviceProcess {
    callback: DeviceCallback,
    output_channels: NonZeroUsize,
    input_channels: Option<NonZeroUsize>,
}

impl DeviceProcess {
    /// Setup: runs before the device starts, off the real-time thread.
    pub(crate) fn new(
        callback: DeviceCallback,
        output_channels: NonZeroUsize,
        input_channels: usize,
    ) -> Self {
        Self {
            callback,
            output_channels,
            input_channels: NonZeroUsize::new(input_channels),
        }
    }

    /// Processes one callback. A pure function of the stream and the buffers, so tests drive it
    /// without a device.
    ///
    /// A period whose channel counts are not the stream's is left as miniaudio pre-silenced it.
    /// That cannot happen: the shim asks miniaudio for the stream's counts, and miniaudio
    /// converts to them.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub(crate) fn process(&mut self, period: Period<'_>) {
        let started = Instant::now();
        if !self.matches(&period) {
            return;
        }
        let Period {
            output,
            input,
            frames,
            sample_rate,
            ..
        } = period;
        let fill = self.callback.fill(output);
        if let (Some(input), Some(input_channels)) = (input, self.input_channels) {
            let gain = self.callback.monitor_gain();
            if gain > 0.0 {
                monitor(output, self.output_channels, input, input_channels, gain);
            }
            self.callback.meter_input(input);
            self.callback.capture(input, &fill);
        }
        self.callback.meter_output(output);
        let load = load(started.elapsed(), frames, sample_rate);
        self.callback
            .end_callback(usize::try_from(frames).unwrap_or(usize::MAX), load);
    }

    fn matches(&self, period: &Period<'_>) -> bool {
        let output =
            usize::try_from(period.output_channels).ok() == Some(self.output_channels.get());
        let input = period.input.is_none()
            || usize::try_from(period.input_channels).ok()
                == self.input_channels.map(NonZeroUsize::get);
        output && input
    }
}

/// The direct monitor: adds the input times `gain` to the output. Output channel `c` takes
/// input channel `c % input_channels`, so a mono input reaches every output channel.
fn monitor(
    output: &mut [f32],
    output_channels: NonZeroUsize,
    input: &[f32],
    input_channels: NonZeroUsize,
    gain: f32,
) {
    let frames = output
        .chunks_exact_mut(output_channels.get())
        .zip(input.chunks_exact(input_channels.get()));
    for (out_frame, in_frame) in frames {
        for (out, sample) in out_frame.iter_mut().zip(in_frame.iter().cycle()) {
            *out += *sample * gain;
        }
    }
}

/// The prototype's `callback_load`: the processing time over the duration of the period,
/// `frames / max(1, sample_rate)` seconds. A callback without frames (miniaudio sends none with
/// fixed-size callbacks) has load 0.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the prototype stores the load as a float"
)]
pub(crate) fn load(elapsed: Duration, frames: u32, sample_rate: u32) -> f32 {
    if frames == 0 {
        return 0.0;
    }
    let period = f64::from(frames) / f64::from(sample_rate.max(1));
    (elapsed.as_secs_f64() / period) as f32
}

/// Applies a device notification to the stream's stats: a device stop while running and an
/// interruption count as device errors, a reroute is counted (RT-08). Atomics only.
#[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
pub(crate) fn notify(control: &StreamControl, notification: Notification) {
    match notification {
        Notification::Stopped => control.note_device_stopped(),
        Notification::Rerouted => control.note_rerouted(),
        Notification::InterruptionBegan => control.note_interruption(),
        Notification::Started | Notification::InterruptionEnded | Notification::Unlocked => {}
    }
}
