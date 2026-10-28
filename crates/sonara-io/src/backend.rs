//! The engine-agnostic seam over the device engine: [`AudioBackend`], implemented by
//! [`Engine`](crate::Engine) and by the scripted [`MockBackend`].
//!
//! Not real-time: every method runs on a control thread (RT-06). Callers that must not depend
//! on miniaudio (the transport, the catalog and the tests of other crates) take an
//! `impl AudioBackend` or `&mut dyn AudioBackend` instead of the concrete engine (DES-06).
//!
//! The lifecycle is `init_context` → `start` ⇄ `stop` → `shutdown`, and `shutdown` may be
//! followed by a new `init_context`. Every call is valid in every state: `stop` and `shutdown`
//! are idempotent, and a call that needs a context returns [`EngineError::NoContext`] without one.

use sonara_rt::{Stream, StreamControl};

use crate::{ContextKind, DeviceConfig, DeviceInfo, EngineError, StreamFormat, StreamHandles};

/// The lifecycle and diagnostics of an audio device engine.
pub trait AudioBackend {
    /// Creates the audio context; does nothing when one exists.
    ///
    /// # Errors
    ///
    /// [`EngineError::ContextInit`] when the context cannot be created.
    fn init_context(&mut self, kind: ContextKind) -> Result<(), EngineError>;

    /// The name of the context's backend, empty before the first context.
    fn backend_name(&self) -> &str;

    /// Lists the playback devices, then the capture devices.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoContext`] without a context, [`EngineError::Enumeration`] when the
    /// devices cannot be listed.
    fn devices(&mut self) -> Result<Vec<DeviceInfo>, EngineError>;

    /// Stops the running stream, then opens and starts a stream with `config`.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoContext`] without a context; otherwise the errors of
    /// [`Engine::start`](crate::Engine::start).
    fn start(&mut self, config: &DeviceConfig) -> Result<StreamHandles, EngineError>;

    /// Stops the running stream; does nothing without one. Blocks until the device callback has
    /// returned.
    fn stop(&mut self);

    /// Stops the running stream and releases the context; does nothing without one.
    fn shutdown(&mut self);

    /// Whether a stream is open and started.
    fn is_running(&self) -> bool;
}

/// A scripted [`AudioBackend`] for tests: no device, no miniaudio, the real lifecycle rules.
///
/// [`start`](AudioBackend::start) allocates a real stream (the rings and the handles) that no
/// callback drains, so a test can push blocks, flush and read the stats.
#[derive(Default)]
pub struct MockBackend {
    has_context: bool,
    control: Option<StreamControl>,
    start_failure: Option<EngineError>,
    devices: Vec<DeviceInfo>,
    /// Calls to [`stop`](AudioBackend::stop) and [`shutdown`](AudioBackend::shutdown) that
    /// closed a running stream.
    closed_streams: usize,
}

impl MockBackend {
    /// A mock without a context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes [`devices`](AudioBackend::devices) return `devices`.
    #[must_use]
    pub fn with_devices(mut self, devices: Vec<DeviceInfo>) -> Self {
        self.devices = devices;
        self
    }

    /// Makes the next [`start`](AudioBackend::start) fail with `error`, after it has stopped the
    /// running stream as the real engine does.
    pub fn fail_next_start(&mut self, error: EngineError) {
        self.start_failure = Some(error);
    }

    /// How many running streams `stop` and `shutdown` have closed.
    #[must_use]
    pub fn closed_streams(&self) -> usize {
        self.closed_streams
    }
}

impl AudioBackend for MockBackend {
    fn init_context(&mut self, _kind: ContextKind) -> Result<(), EngineError> {
        self.has_context = true;
        Ok(())
    }

    fn backend_name(&self) -> &str {
        if self.has_context { "Mock" } else { "" }
    }

    fn devices(&mut self) -> Result<Vec<DeviceInfo>, EngineError> {
        if self.has_context {
            Ok(self.devices.clone())
        } else {
            Err(EngineError::NoContext)
        }
    }

    fn start(&mut self, config: &DeviceConfig) -> Result<StreamHandles, EngineError> {
        if !self.has_context {
            return Err(EngineError::NoContext);
        }
        self.stop();
        if let Some(error) = self.start_failure.take() {
            return Err(error);
        }
        let Stream {
            producer,
            capture,
            control,
            ..
        } = Stream::new(&config.stream)?;
        control.set_device_format(config.sample_rate, config.buffer_frames);
        control.set_running(true);
        self.control = Some(control.clone());
        Ok(StreamHandles {
            format: StreamFormat {
                sample_rate: config.sample_rate,
                buffer_frames: config.buffer_frames,
                periods: 3,
                internal_sample_rate: config.sample_rate,
                output_channels: u32::try_from(config.stream.output_channels).unwrap_or(u32::MAX),
            },
            producer,
            capture,
            control,
        })
    }

    fn stop(&mut self) {
        if let Some(control) = self.control.take() {
            control.set_running(false);
            control.set_device_format(0, 0);
            control.set_playing(false);
            self.closed_streams += 1;
        }
    }

    fn shutdown(&mut self) {
        self.stop();
        self.has_context = false;
    }

    fn is_running(&self) -> bool {
        self.control.is_some()
    }
}
