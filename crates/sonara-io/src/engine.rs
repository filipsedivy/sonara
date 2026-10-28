//! The device engine: the miniaudio context, the device list, and starting
//! and stopping a stream on a device.
//!
//! Not real-time: every method runs on a control thread, and some block while miniaudio opens or
//! closes a device. The calls a running stream needs from other threads go through the
//! [`StreamHandles`] that [`Engine::start`] returns: they are atomics and lock-free rings, so they
//! never wait for the device thread or for the engine (RT-04, RT-06).
//!
//! How the prototype's `Engine` (`native/src/engine.cpp`) maps here:
//!
//! | Prototype | Rust | Thread |
//! |---|---|---|
//! | `init_context(backend)` | [`Engine::init_context`] with a [`ContextKind`] | control |
//! | `backend_name` | [`Engine::backend_name`] | control |
//! | `devices()` | [`Engine::devices`] | control |
//! | `start(…)` | [`Engine::start`], returning the [`StreamHandles`] | control |
//! | `stop()`, `shutdown()` | [`Engine::stop`], [`Engine::shutdown`], and `Drop` | control |
//! | `write_block`, `free_blocks`, `queued_frames` | [`BlockProducer`] | render |
//! | `flush`, `set_playing`, `set_monitor`, `stats` | [`StreamControl`] | any |
//! | `set_capture(true/false)` | [`StreamControl::arm_capture`], [`disarm_capture`](StreamControl::disarm_capture) | any |
//! | `read_capture` | [`CaptureReader::read`] | take writer |
//!
//! The prototype's engine held every piece itself; here each thread owns its handle, so the
//! single-producer, single-consumer rings cannot be misused (PRI-04, RT-04).

use std::num::NonZeroUsize;

use sonara_rt::{
    BlockProducer, CaptureReader, ConfigError, LatencyFrames, Stream, StreamConfig, StreamControl,
};

use crate::callback::DeviceProcess;
use crate::device::{DeviceId, DeviceInfo};
use crate::error::EngineError;
use crate::ffi::{Context, DeviceRequest};

/// Which audio backends a context uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextKind {
    /// miniaudio's default list: Core Audio on macOS, ending with the null backend.
    Default,
    /// The null backend only: a simulated real-time device driven by a timer thread, for tests
    /// (the prototype's `"null"` context, the `native-null` audio backend).
    Null,
}

/// What [`Engine::start`] opens: the arguments of the prototype's `Engine::start`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceConfig {
    /// The stream's sample rate; miniaudio converts when the device runs at another rate.
    pub sample_rate: u32,
    /// The device period in frames: every callback gets exactly this many frames (256 in the
    /// prototype's settings).
    pub buffer_frames: u32,
    /// The channel counts and ring sizes. `capture_frames` replaces the prototype's
    /// `capture_seconds` × sample rate.
    pub stream: StreamConfig,
    /// The output device's saved id (`settings.audio.output_device`). `None` or an empty id
    /// selects the system default, as the prototype's empty string does.
    pub output_device: Option<String>,
    /// The input device's saved id, as `output_device`. Ignored without input channels.
    pub input_device: Option<String>,
}

/// What a started device obtained: the prototype's `start` result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamFormat {
    /// The device's sample rate: the requested one.
    pub sample_rate: u32,
    /// The playback period in frames.
    pub buffer_frames: u32,
    /// The playback periods (miniaudio's default is 3).
    pub periods: u32,
    /// The playback hardware's sample rate.
    pub internal_sample_rate: u32,
    /// The output channels of the callback.
    pub output_channels: u32,
}

/// A started stream's handles, one per thread (see [`sonara_rt::Stream`]).
pub struct StreamHandles {
    /// What the device obtained.
    pub format: StreamFormat,
    /// The render thread's handle: it pushes the played blocks.
    pub producer: BlockProducer,
    /// The take writer's handle: it reads the captured input.
    pub capture: CaptureReader,
    /// The handle for every other thread: flush, playing flag, monitor, capture, stats. It
    /// stays readable after the stream stops; its counters are kept.
    pub control: StreamControl,
}

/// The device engine: a miniaudio context and at most one running stream.
///
/// `Engine` is `Send`; share it between control threads behind a lock. A stream's real-time
/// calls do not need the engine: they go through its [`StreamHandles`].
#[derive(Default)]
pub struct Engine {
    context: Option<Context>,
    backend_name: String,
    /// The current or last stream's control handle.
    control: Option<StreamControl>,
}

impl Engine {
    /// An engine without a context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates the audio context. Does nothing when a context exists, whatever its kind, as in
    /// the prototype.
    ///
    /// # Errors
    ///
    /// [`EngineError::ContextInit`] when miniaudio cannot create the context.
    pub fn init_context(&mut self, kind: ContextKind) -> Result<(), EngineError> {
        if self.context.is_some() {
            return Ok(());
        }
        let context = Context::init(kind).map_err(EngineError::ContextInit)?;
        context.backend_name().clone_into(&mut self.backend_name);
        self.context = Some(context);
        Ok(())
    }

    /// miniaudio's name of the context's backend, for example `"Core Audio"` or `"Null"`; empty
    /// before the first context. It is kept after [`shutdown`](Self::shutdown).
    #[must_use]
    pub fn backend_name(&self) -> &str {
        &self.backend_name
    }

    /// Lists the devices: playback devices first, then capture devices, in miniaudio's order.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoContext`] without a context, [`EngineError::Enumeration`] when
    /// miniaudio cannot list the devices.
    pub fn devices(&mut self) -> Result<Vec<DeviceInfo>, EngineError> {
        let context = self.context.as_mut().ok_or(EngineError::NoContext)?;
        context.devices().map_err(EngineError::Enumeration)
    }

    /// Stops the running stream, then opens and starts a stream with `config`, with the
    /// prototype's miniaudio configuration: a duplex device with input channels, otherwise a
    /// playback device; `f32`; the period of `buffer_frames` with miniaudio's default 3 periods;
    /// the low-latency profile; shared mode; fixed-size callbacks.
    ///
    /// The output latency estimate becomes period × periods, the input estimate the capture
    /// period × periods (0 without input). They are estimates, never positions (RT-07).
    ///
    /// Blocks while miniaudio opens and starts the device.
    ///
    /// # Errors
    ///
    /// In the prototype's order, each with its message:
    /// - [`EngineError::NoContext`] without a context (nothing is stopped);
    /// - [`EngineError::Config`] for out-of-range channel counts or ring sizes;
    /// - [`EngineError::InvalidOutputDevice`], [`EngineError::InvalidInputDevice`] for an id
    ///   that is not a device id;
    /// - [`EngineError::Open`], [`EngineError::Start`] when miniaudio fails, for example on an
    ///   unknown device ("cannot open audio device: No device").
    ///
    /// The running stream is stopped in every case but the first.
    pub fn start(&mut self, config: &DeviceConfig) -> Result<StreamHandles, EngineError> {
        if self.context.is_none() {
            return Err(EngineError::NoContext);
        }
        self.stop();
        let stream = Stream::new(&config.stream)?;
        let output_id =
            selected(config.output_device.as_deref()).map_err(EngineError::InvalidOutputDevice)?;
        let input_id = if config.stream.input_channels > 0 {
            selected(config.input_device.as_deref()).map_err(EngineError::InvalidInputDevice)?
        } else {
            None
        };
        // `Stream::new` has validated both counts: 1 to 32 outputs, 0 to 32 inputs.
        let output_channels = NonZeroUsize::new(config.stream.output_channels)
            .ok_or(ConfigError::UnsupportedChannelCount)?;
        let request = DeviceRequest {
            sample_rate: config.sample_rate,
            period_frames: config.buffer_frames,
            output_channels: channel_count(config.stream.output_channels)?,
            input_channels: channel_count(config.stream.input_channels)?,
            output_id: output_id.as_ref(),
            input_id: input_id.as_ref(),
        };

        let Stream {
            producer,
            callback,
            capture,
            control,
        } = stream;
        let process = Box::new(DeviceProcess::new(
            callback,
            output_channels,
            config.stream.input_channels,
        ));
        let context = self.context.as_mut().ok_or(EngineError::NoContext)?;
        let obtained = context
            .open_device(&request, process, Box::new(control.clone()))
            .map_err(EngineError::Open)?;
        control.set_device_format(obtained.sample_rate, obtained.period_frames);
        if let Err(error) = context.start_device() {
            control.set_device_format(0, 0);
            return Err(EngineError::Start(error));
        }
        control.set_running(true);
        let input_latency = if config.stream.input_channels > 0 {
            latency(obtained.capture_period_frames, obtained.capture_periods)
        } else {
            LatencyFrames::new(0)
        };
        control.set_latency(
            latency(obtained.period_frames, obtained.periods),
            input_latency,
        );
        self.control = Some(control.clone());
        Ok(StreamHandles {
            format: StreamFormat {
                sample_rate: obtained.sample_rate,
                buffer_frames: obtained.period_frames,
                periods: obtained.periods,
                internal_sample_rate: obtained.internal_sample_rate,
                output_channels: obtained.output_channels,
            },
            producer,
            capture,
            control,
        })
    }

    /// Stops the running stream: clears `running` first, so the device's own stop
    /// notification is not counted as a device error, then closes the device. Blocks until the
    /// device's callback has returned (`ma_device_uninit`), so call it on a control thread,
    /// never from a callback (RT-06). The stream's counters stay readable through its
    /// [`StreamControl`]; its sample rate and period read 0, and `playing` is cleared.
    pub fn stop(&mut self) {
        let control = self.control.as_ref();
        if let Some(context) = self.context.as_mut()
            && context.has_device()
        {
            if let Some(control) = control {
                control.set_running(false);
            }
            context.close_device();
            if let Some(control) = control {
                control.set_device_format(0, 0);
            }
        }
        if let Some(control) = control {
            control.set_playing(false);
        }
    }

    /// Whether a stream is open and started.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.context.as_ref().is_some_and(Context::has_device)
    }

    /// Stops the running stream and releases the context. [`init_context`](Self::init_context)
    /// creates a new one.
    pub fn shutdown(&mut self) {
        self.stop();
        self.context = None;
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// The device a saved id selects: `None` (or an empty id) for the system default.
fn selected(id: Option<&str>) -> Result<Option<DeviceId>, crate::InvalidDeviceId> {
    match id {
        None | Some("") => Ok(None),
        Some(id) => id.parse().map(Some),
    }
}

fn channel_count(channels: usize) -> Result<u32, ConfigError> {
    u32::try_from(channels).map_err(|_| ConfigError::UnsupportedChannelCount)
}

/// The prototype's latency estimate: period × max(1, periods).
fn latency(period_frames: u32, periods: u32) -> LatencyFrames {
    LatencyFrames::new(period_frames.saturating_mul(periods.max(1)))
}

impl crate::AudioBackend for Engine {
    fn init_context(&mut self, kind: ContextKind) -> Result<(), EngineError> {
        Self::init_context(self, kind)
    }

    fn backend_name(&self) -> &str {
        Self::backend_name(self)
    }

    fn devices(&mut self) -> Result<Vec<DeviceInfo>, EngineError> {
        Self::devices(self)
    }

    fn start(&mut self, config: &DeviceConfig) -> Result<StreamHandles, EngineError> {
        Self::start(self, config)
    }

    fn stop(&mut self) {
        Self::stop(self);
    }

    fn shutdown(&mut self) {
        Self::shutdown(self);
    }

    fn is_running(&self) -> bool {
        Self::is_running(self)
    }
}
