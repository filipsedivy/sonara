//! The real-time player (the prototype's `RealtimePlayer` in
//! `audio/transport/player.py`): playback with lookahead, sample-accurate loops, the metronome,
//! live MIDI input and recording, on a stream that a [`AudioDevice`] opens.
//!
//! # Threads
//!
//! | Thread | Code | Role |
//! |---|---|---|
//! | any (agent, GUI, CLI) | [`Player`] | the control calls; none waits for the render thread (RT-06) |
//! | `sonara-player` (non-real-time) | [`Player::service`] | takes back what the render thread returns and drops it (RT-09), answers setup requests, rebuilds the graph when the project changes, collects meters and reports |
//! | `sonara-prefetch` (non-real-time) | [`PrefetchWorker::fill`] | reads the audio clips ahead of the render thread |
//! | `sonara-render` (real-time) | [`RenderLoop::step`] | the player loop |
//! | `sonara-takes` (non-real-time) | the take writer | drains the capture ring into the take files while a recording runs |
//!
//! [`Player::new`] starts the three threads; [`Player::manual`] returns their parts instead, so
//! a test drives them step by step, as the golden recorder drove the prototype's `_step`.
//! [`Player::with_render_start`] also gives the render thread a [`RenderThreadStart`] hook,
//! which it runs once on itself before its loop. On a real device the application joins the
//! output device's audio workgroup there with a time-constraint policy, falling back to the
//! user-interactive `QoS` class. The render thread drops the hook's guard after its last
//! step.
//!
//! # Control
//!
//! Every call that changes the transport bumps the generation and flushes the stream at once
//! (atomics), then posts its message to the render thread through one ordered hand-off. A start
//! builds a fresh graph, as the prototype resets its graph (`GraphBuilder::fresh`); a seek keeps
//! the graph and lets the next block chase. The flush disarms `playing` until the new
//! generation's first block is written. `play` and `seek` answer the new position.
//!
//! The project comes from the caller ([`ProjectSnapshot`]); the `sonara-player` thread also
//! polls a [`ProjectSource`] to pick up edits during playback, as the prototype's render step
//! compared `session.version`. It never holds the player's lock while it asks the source, so a
//! caller that holds its session lock while it calls the player cannot deadlock with it.
//!
//! # Recording
//!
//! [`Player::record`] starts like a play from the count-in, with the clicks forced on, and
//! records MIDI input with its frames through a ring from the render thread; the device callback
//! captures the input into the capture ring, which the take writer thread drains into the take
//! files. [`Player::stop`] returns the [`Recorded`] takes; the transport service (`sonara-app`)
//! aligns them, adds the assets and clips in one "Record" transaction, and writes the JSON. The
//! status and position values here are already rounded as the prototype's JSON rounds them.

mod clicks;
mod control;
mod frames;
mod message;
mod prefetch;
mod render;
mod shared;
mod takes;
mod threads;

use std::fmt;
use std::sync::Arc;

use sonara_core::model::project::Project;
use sonara_media::MediaProvider;
use sonara_rt::{BlockProducer, CaptureReader, StreamControl};

pub use clicks::click;
pub use control::{
    CAPTURE_SECONDS, DeviceReport, Paused, Player, PlayerSetup, PlayerState, PlayerStatus,
    Position, RecordPlan, Recorded, Stopped, TrackMeter,
};
pub use frames::LoopRange;
pub use message::MAX_MIDI_INPUTS;
pub use render::{BlockView, RenderLoop, Step};
pub use shared::RenderCounters;
pub use takes::{DRAIN_REST, RecordedTake, TakeTarget, WriterStats};
pub use threads::{PrefetchWorker, RenderThreadStart};

/// The player's audio settings (`AudioSettings` in the prototype).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerSettings {
    /// How far ahead of the device the player renders, in milliseconds (80).
    pub lookahead_ms: f64,
    /// The lookahead while live MIDI input is active (25); it also governs playback then.
    pub live_lookahead_ms: f64,
    /// The device period in frames (256).
    pub buffer_frames: u32,
    /// The saved output device id; `None` for the system default.
    pub output_device: Option<String>,
    /// The saved input device id; `None` for the system default.
    pub input_device: Option<String>,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            lookahead_ms: 80.0,
            live_lookahead_ms: 25.0,
            buffer_frames: 256,
            output_device: None,
            input_device: None,
        }
    }
}

/// A project the player plays: its snapshot, the session version it has, and the media
/// provider of its location.
#[derive(Debug, Clone)]
pub struct ProjectSnapshot {
    /// The project (cheap to clone).
    pub project: Project,
    /// The session's version of it; a new version rebuilds the graph during playback.
    pub version: u64,
    /// Where its media is read from.
    pub media: Arc<MediaProvider>,
}

/// Where the `sonara-player` thread looks for edits during playback: the session.
pub trait ProjectSource: Send + Sync {
    /// The open project, or `None` without one. Called without any player lock held.
    fn snapshot(&self) -> Option<ProjectSnapshot>;
}

impl<F> ProjectSource for F
where
    F: Fn() -> Option<ProjectSnapshot> + Send + Sync,
{
    fn snapshot(&self) -> Option<ProjectSnapshot> {
        self()
    }
}

/// What the player asks of a device: the prototype's `StreamConfig` with the ring in blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRequest {
    /// The project's sample rate; the device layer converts when the device differs.
    pub sample_rate: u32,
    /// The device period in frames.
    pub buffer_frames: u32,
    /// Output channels (2).
    pub output_channels: usize,
    /// Input channels (0 without recording).
    pub input_channels: usize,
    /// Blocks of 512 frames in the playback ring: `max(4, ceil(ring_frames / 512))` with
    /// `ring_frames = max(2048, int(lookahead_ms · 0.001 · sr · 2))`.
    pub ring_blocks: usize,
    /// Frames of the capture ring: [`CAPTURE_SECONDS`] of input, 0 without input.
    pub capture_frames: usize,
    /// The saved output device id.
    pub output_device: Option<String>,
    /// The saved input device id.
    pub input_device: Option<String>,
}

/// A started stream's handles (`sonara_io::StreamHandles`).
pub struct DeviceStream {
    /// The device's sample rate.
    pub sample_rate: u32,
    /// The render thread's handle.
    pub producer: BlockProducer,
    /// The take writer's handle.
    pub capture: CaptureReader,
    /// The control handle.
    pub control: StreamControl,
}

impl fmt::Debug for DeviceStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceStream")
            .field("sample_rate", &self.sample_rate)
            .finish_non_exhaustive()
    }
}

/// A device that could not be opened or started, with the device layer's message (the
/// player reports it as `DEVICE_ERROR "cannot open the audio device: <message>"`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct DeviceError(pub String);

/// The device layer as the player uses it: `sonara-io`'s engine on a real or the null device,
/// or a test device whose callback the test drives (DES-06).
pub trait AudioDevice: Send {
    /// Stops the running stream, then opens and starts one for `request`. Blocks while the
    /// device opens; the player calls it on a control thread.
    ///
    /// # Errors
    ///
    /// [`DeviceError`] with the device layer's message when the device cannot be opened or
    /// started.
    fn start(&mut self, request: &StreamRequest) -> Result<DeviceStream, DeviceError>;

    /// Stops the running stream; blocks until its callback has returned.
    fn stop(&mut self);
}
