//! Real-time: runs on the audio device thread (see docs/audio-engine.md).
//!
//! Sonara's real-time primitives: the block-tagged playback ring with
//! flush and fade, the capture ring, the stream's stats atomics, and the fixed-size rings for
//! commands, MIDI events, meters and the graph hand-off. They are the only channels between
//! real-time and other threads (RT-04). Each open MIDI input has its own [`midi_channel`].
//!
//! # A stream
//!
//! [`Stream::new`] is setup: it allocates every ring before the device starts (RT-02) and
//! splits the stream into one handle per thread.
//!
//! | Handle | Thread | Role |
//! |---|---|---|
//! | [`BlockProducer`] | render thread | pushes rendered blocks, queries the queue depth |
//! | [`DeviceCallback`] | device callback | [`fill`](DeviceCallback::fill)s the output, captures input, meters, counts |
//! | [`CaptureReader`] | take writer thread | reads captured frames take by take |
//! | [`StreamControl`] | any thread | flush, playing, monitor, capture arming, stats, device notifications |
//!
//! The device callback runs, in the prototype's order (`native/src/engine.cpp`):
//! [`fill`](DeviceCallback::fill), then the direct monitor with
//! [`monitor_gain`](DeviceCallback::monitor_gain) (`sonara-io`),
//! [`meter_input`](DeviceCallback::meter_input), [`capture`](DeviceCallback::capture),
//! [`meter_output`](DeviceCallback::meter_output) after monitoring, and finally
//! [`end_callback`](DeviceCallback::end_callback) with the measured load.
//!
//! # Rings
//!
//! Every ring is an [`rtrb`] single-producer, single-consumer ring. `rtrb` expresses the
//! prototype's block ring with two rings (samples and block metadata) plus a small atomic
//! ledger for the queue depth, so no custom ring and no `unsafe` are needed (QG-13, RT-10).
//! `rtrb` has no loom support; loom checks Sonara's protocol atomics around it through the
//! `sync` shim (`--cfg loom`).
#![forbid(unsafe_code)]
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

mod callback;
mod capture;
mod control;
mod generation;
pub mod handoff;
mod midi;
mod playback;
mod position;
pub mod queue;
mod stats;
mod stream;
mod sync;

pub use callback::DeviceCallback;
pub use capture::{CaptureChunk, CaptureReader, CaptureTake, TakeId};
pub use control::StreamControl;
pub use generation::Generation;
pub use midi::{MidiEvent, MidiKind, MidiOverflows, MidiReceiver, MidiSender, midi_channel};
pub use playback::{BlockProducer, Fill, FirstCurrentFrame, PushError};
pub use position::{LatencyFrames, TimelineFrame};
pub use stats::{Peaks, StatsSnapshot};
pub use stream::{ConfigError, Stream, StreamConfig};

/// Frames per playback block: the render thread's block size (`BLOCK` in the prototype's
/// player, `block_frames` of `NativeBackend`).
pub const BLOCK_FRAMES: usize = 512;

/// Smallest block a stream accepts (the prototype's `start` rejects smaller ones).
pub const MIN_BLOCK_FRAMES: usize = 16;

/// Smallest playback ring, in blocks.
pub const MIN_RING_BLOCKS: usize = 2;

/// Largest output or input channel count of a stream.
pub const MAX_CHANNELS: usize = 32;

/// Length of the flush fades: a flushed block fades out, and new material fades in, over this
/// many frames (`kFadeFrames`).
pub const FADE_FRAMES: usize = 128;

/// Channels whose peaks are metered: the first eight of the output and of the input.
pub const METERED_CHANNELS: usize = 8;
