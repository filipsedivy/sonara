//! A stream's shared state and its setup: one allocation of every ring, split into per-thread
//! handles.

use std::num::{NonZeroU64, NonZeroUsize};

use rtrb::RingBuffer;

use crate::capture::{self, CaptureWriter, PENDING_STARTS};
use crate::playback::{self, BlockReader};
use crate::stats::Stats;
use crate::sync::{Arc, AtomicU32};
use crate::{
    BlockProducer, CaptureReader, DeviceCallback, MAX_CHANNELS, MIN_BLOCK_FRAMES, MIN_RING_BLOCKS,
    StreamControl,
};

/// The sizes of a stream (the prototype's `Engine::start` arguments; `capture_frames` replaces
/// `capture_seconds` × sample rate).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamConfig {
    /// Output channels: 1 to [`MAX_CHANNELS`].
    pub output_channels: usize,
    /// Input channels: 0 (no capture) to [`MAX_CHANNELS`].
    pub input_channels: usize,
    /// Frames per playback block; at least [`MIN_BLOCK_FRAMES`]. The player uses
    /// [`BLOCK_FRAMES`](crate::BLOCK_FRAMES).
    pub block_frames: usize,
    /// Playback ring size in blocks; at least [`MIN_RING_BLOCKS`].
    pub ring_blocks: usize,
    /// Capture ring size in frames; at least 1 when there is input.
    pub capture_frames: usize,
}

/// Why [`Stream::new`] rejected a [`StreamConfig`]. The messages are the prototype's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// No output, or more than [`MAX_CHANNELS`] output or input channels.
    #[error("unsupported channel count")]
    UnsupportedChannelCount,
    /// Blocks shorter than [`MIN_BLOCK_FRAMES`], fewer than [`MIN_RING_BLOCKS`] blocks, or no
    /// capture frames with input.
    #[error("ring too small")]
    RingTooSmall,
    /// A ring too large to allocate or to count.
    #[error("ring too large")]
    RingTooLarge,
}

/// State shared by a stream's handles.
pub(crate) struct Shared {
    pub(crate) playback: playback::Protocol,
    pub(crate) capture: capture::Protocol,
    pub(crate) stats: Stats,
    /// The direct monitor gain as `f32` bits.
    pub(crate) monitor_gain: AtomicU32,
}

/// A stream, split into one handle per thread (see the [crate docs](crate)).
pub struct Stream {
    /// The render thread's handle.
    pub producer: BlockProducer,
    /// The device callback's handle.
    pub callback: DeviceCallback,
    /// The take writer's handle.
    pub capture: CaptureReader,
    /// The handle for every other thread; it can be cloned.
    pub control: StreamControl,
}

/// The largest ring, in samples: its byte size fits `isize` with room to spare.
const MAX_RING_SAMPLES: usize = usize::MAX >> 4;

impl Stream {
    /// Allocates the rings of a stream with the sizes of `config`.
    ///
    /// Setup: it runs before the device starts, off the real-time thread (RT-02).
    ///
    /// # Errors
    ///
    /// [`ConfigError`] when `config` is out of range.
    pub fn new(config: &StreamConfig) -> Result<Self, ConfigError> {
        let sizes = Sizes::new(config)?;
        let shared = Arc::new(Shared {
            playback: playback::Protocol::new(sizes.ring_blocks),
            capture: capture::Protocol::new(),
            stats: Stats::new(sizes.output_channels, config.input_channels),
            monitor_gain: AtomicU32::new(0),
        });
        let (samples_in, samples_out) = RingBuffer::new(sizes.ring_samples);
        let (meta_in, meta_out) = RingBuffer::new(config.ring_blocks);
        let producer = BlockProducer::new(
            Arc::clone(&shared),
            samples_in,
            meta_in,
            sizes.output_channels,
            config.block_frames,
        );
        let reader = BlockReader::new(samples_out, meta_out, sizes.output_channels);
        let (writer, capture) = match NonZeroUsize::new(config.input_channels) {
            Some(channels) => {
                let (frames_in, frames_out) = RingBuffer::new(sizes.capture_samples);
                let (starts_in, starts_out) = RingBuffer::new(PENDING_STARTS);
                (
                    Some(CaptureWriter::new(frames_in, starts_in, channels)),
                    CaptureReader::new(frames_out, starts_out, channels),
                )
            }
            None => (None, CaptureReader::without_input()),
        };
        Ok(Self {
            producer,
            callback: DeviceCallback::new(Arc::clone(&shared), reader, writer),
            capture,
            control: StreamControl::new(shared),
        })
    }
}

/// A validated [`StreamConfig`].
struct Sizes {
    output_channels: NonZeroUsize,
    ring_blocks: NonZeroU64,
    ring_samples: usize,
    capture_samples: usize,
}

impl Sizes {
    fn new(config: &StreamConfig) -> Result<Self, ConfigError> {
        let output_channels = NonZeroUsize::new(config.output_channels)
            .filter(|channels| channels.get() <= MAX_CHANNELS)
            .ok_or(ConfigError::UnsupportedChannelCount)?;
        if config.input_channels > MAX_CHANNELS {
            return Err(ConfigError::UnsupportedChannelCount);
        }
        if config.block_frames < MIN_BLOCK_FRAMES
            || config.ring_blocks < MIN_RING_BLOCKS
            || (config.input_channels > 0 && config.capture_frames == 0)
        {
            return Err(ConfigError::RingTooSmall);
        }
        // Block frame counts are `u32` in the ring, and block counters are compared in 32 bits.
        if u32::try_from(config.block_frames).is_err()
            || config.ring_blocks > usize::try_from(u32::MAX >> 1).unwrap_or(usize::MAX)
        {
            return Err(ConfigError::RingTooLarge);
        }
        let ring_samples = config
            .block_frames
            .checked_mul(config.ring_blocks)
            .and_then(|frames| frames.checked_mul(output_channels.get()))
            .filter(|samples| *samples <= MAX_RING_SAMPLES)
            .ok_or(ConfigError::RingTooLarge)?;
        let capture_samples = config
            .capture_frames
            .checked_mul(config.input_channels)
            .filter(|samples| *samples <= MAX_RING_SAMPLES)
            .ok_or(ConfigError::RingTooLarge)?;
        let ring_blocks = u64::try_from(config.ring_blocks)
            .ok()
            .and_then(NonZeroU64::new)
            .ok_or(ConfigError::RingTooSmall)?;
        Ok(Self {
            output_channels,
            ring_blocks,
            ring_samples,
            capture_samples,
        })
    }
}
