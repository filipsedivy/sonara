//! The block-tagged playback ring (`native/src/engine.cpp`).
//!
//! The render thread pushes whole blocks of interleaved `f32` frames, each tagged with the
//! timeline frame of its first sample and a [`Generation`]. The device callback plays them frame
//! by frame. [`flush`](crate::StreamControl::flush) makes every block of an older generation
//! stale: the callback fades the playing audio out over [`FADE_FRAMES`](crate::FADE_FRAMES),
//! discards the stale blocks and fades the new material in.
//!
//! Two `rtrb` rings carry a block: its samples, then its metadata. The producer pushes the
//! samples first, so a consumer that sees the metadata finds the samples too. The queue depth
//! comes from a ledger the producer writes and a read position the consumer publishes, so no
//! thread ever reads a slot another thread owns.
//!
//! Differences from the prototype:
//! - D12: the first block of a newer generation always fades in, even when the flush found no
//!   stale block and the ring never ran empty.
//! - D13: a fade-out that outlasts its stale block continues into the next stale block; it ends
//!   early only when no stale audio is left.
//! - D11: the player's `playing` flag is armed for one generation
//!   ([`BlockProducer::set_playing_generation`]). A flush to a newer generation disarms it at
//!   once, so the control thread never has to clear the flag itself, and a render thread that
//!   arms a generation the control thread has just flushed arms nothing: no underrun is counted
//!   while a new start or a seek waits for its first block.

use std::num::{NonZeroU64, NonZeroUsize};

use rtrb::{Consumer, Producer};

use crate::stream::Shared;
use crate::sync::{Arc, AtomicU32, AtomicU64, Ordering};
use crate::{FADE_FRAMES, Generation, TimelineFrame};

/// Metadata of one block in the ring.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BlockMeta {
    tag: TimelineFrame,
    generation: Generation,
    frames: u32,
}

/// Why [`BlockProducer::push_block`] rejected a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PushError {
    /// Every slot is taken, stale blocks included. The block was not written; retry once the
    /// callback has played or discarded a block.
    #[error("the playback ring is full")]
    Full,
    /// The samples are not whole frames of the stream's channel count, or more frames than a
    /// block holds.
    #[error("block shape does not match the stream")]
    ShapeMismatch,
}

/// The playback protocol's shared atomics. Only the producer writes `written` and the ledger;
/// only the consumer writes `read_position`.
pub(crate) struct Protocol {
    /// Oldest generation that is still current; older blocks are stale.
    min_generation: AtomicU32,
    /// The player's flag that arms underrun counting, for one generation: 0 when disarmed,
    /// otherwise `ARMED` with the generation in the low half. It counts only while that
    /// generation is current.
    playing: AtomicU64,
    /// Blocks pushed since the stream started.
    written: AtomicU64,
    /// The consumer's position: the low 32 bits of the blocks it has finished (high half) and
    /// the frames it has played of the head block (low half).
    read_position: AtomicU64,
    /// Generation (high half) and frame count (low half) of the last pushed blocks, at the
    /// block's index modulo the ring size.
    ledger: Box<[AtomicU64]>,
    /// The ring size in blocks (the ledger's length).
    blocks: NonZeroU64,
}

impl Protocol {
    /// Setup: allocates the ledger for a ring of `blocks` blocks.
    pub(crate) fn new(blocks: NonZeroU64) -> Self {
        let ledger = (0..blocks.get()).map(|_| AtomicU64::new(0)).collect();
        Self {
            min_generation: AtomicU32::new(0),
            playing: AtomicU64::new(DISARMED),
            written: AtomicU64::new(0),
            read_position: AtomicU64::new(0),
            ledger,
            blocks,
        }
    }

    pub(crate) fn flush(&self, generation: Generation) {
        // Release ↔ the Acquire load at the start of `BlockReader::fill`, and in
        // `queued_frames`.
        self.min_generation
            .store(generation.get(), Ordering::Release);
    }

    /// Arms the flag for the generation that is current now (`true`), or disarms it.
    pub(crate) fn set_playing(&self, playing: bool) {
        let armed = if playing {
            // Acquire ↔ the Release store in `flush`.
            arm(Generation::new(self.min_generation.load(Ordering::Acquire)))
        } else {
            DISARMED
        };
        self.store_playing(armed);
    }

    /// Arms the flag for `generation`, or disarms it (`None`).
    pub(crate) fn set_playing_generation(&self, generation: Option<Generation>) {
        self.store_playing(generation.map_or(DISARMED, arm));
    }

    fn store_playing(&self, armed: u64) {
        // Release ↔ the Acquire load in `playing_for`: a callback that sees the flag armed also
        // sees every block pushed before it, so the first block of a generation is never
        // counted as an underrun.
        self.playing.store(armed, Ordering::Release);
    }

    /// Whether the flag is armed for a generation that `min_generation` still keeps current.
    fn playing_for(&self, min_generation: Generation) -> bool {
        // Acquire ↔ the Release store in `store_playing`.
        let armed = self.playing.load(Ordering::Acquire);
        let (flag, generation) = unpack_halves(armed);
        flag == ARMED_FLAG && !Generation::new(generation).is_older_than(min_generation)
    }

    pub(crate) fn is_playing(&self) -> bool {
        // Acquire ↔ the Release store in `flush`.
        let min_generation = Generation::new(self.min_generation.load(Ordering::Acquire));
        self.playing_for(min_generation)
    }

    /// Frames of the current generation not yet played: the ledger entries of the blocks still
    /// in the ring, minus the played part of the head block.
    ///
    /// The read position is published at most once per callback and before each finished
    /// block, as in the prototype, so the value can be one callback behind. From the producer's
    /// thread it never counts a block twice. From another thread, a block pushed during the
    /// call can stand in for the one it replaced; the value is then an estimate.
    pub(crate) fn queued_frames(&self) -> u64 {
        // Acquire ↔ the Release stores in `BlockReader::publish_position`.
        let position = self.read_position.load(Ordering::Acquire);
        // Acquire ↔ the Release store in `BlockProducer::push_block`: the ledger entries of the
        // blocks counted by `written` are visible.
        let written = self.written.load(Ordering::Acquire);
        // Acquire ↔ the Release store in `flush`.
        let min_generation = Generation::new(self.min_generation.load(Ordering::Acquire));

        let (finished, offset) = unpack_halves(position);
        let in_ring = low_half(written).wrapping_sub(finished);
        if in_ring > u32::MAX >> 1 {
            // The read position is ahead of `written` as this thread loaded it: nothing queued
            // can be counted reliably.
            return 0;
        }
        // A stale read position can make the distance exceed the ring; the ring then holds the
        // last `blocks` pushed blocks, and its head is not the one the offset refers to.
        let (count, offset_applies) = if u64::from(in_ring) > self.blocks.get() {
            (self.blocks.get(), false)
        } else {
            (u64::from(in_ring), true)
        };
        let start = written.wrapping_sub(count);
        let mut total = 0_u64;
        let mut head_is_current = false;
        for index in start..written {
            let (generation, frames) = self.ledger_entry(index);
            if !generation.is_older_than(min_generation) {
                total = total.saturating_add(u64::from(frames));
                head_is_current |= index == start;
            }
        }
        if offset_applies && head_is_current {
            total = total.saturating_sub(u64::from(offset));
        }
        total
    }

    fn ledger_slot(&self, index: u64) -> Option<&AtomicU64> {
        usize::try_from(index % self.blocks)
            .ok()
            .and_then(|slot| self.ledger.get(slot))
    }

    fn ledger_entry(&self, index: u64) -> (Generation, u32) {
        // Relaxed: ordered by the Acquire load of `written` in `queued_frames`.
        self.ledger_slot(index)
            .map_or((Generation::default(), 0), |entry| {
                let (generation, frames) = unpack_halves(entry.load(Ordering::Relaxed));
                (Generation::new(generation), frames)
            })
    }

    fn record(&self, index: u64, generation: Generation, frames: u32) {
        if let Some(entry) = self.ledger_slot(index) {
            // Relaxed: published by the Release store of `written` that follows.
            entry.store(pack_halves(generation.get(), frames), Ordering::Relaxed);
        }
    }
}

/// The high half of an armed `playing` value.
const ARMED_FLAG: u32 = 1;
/// The `playing` value of a disarmed flag.
const DISARMED: u64 = 0;

/// The `playing` value armed for `generation`.
fn arm(generation: Generation) -> u64 {
    pack_halves(ARMED_FLAG, generation.get())
}

fn pack_halves(high: u32, low: u32) -> u64 {
    (u64::from(high) << 32) | u64::from(low)
}

fn unpack_halves(value: u64) -> (u32, u32) {
    (high_half(value), low_half(value))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "keeps exactly the low 32 bits"
)]
fn low_half(value: u64) -> u32 {
    value as u32
}

fn high_half(value: u64) -> u32 {
    (value >> 32) as u32
}

fn frames_of(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

/// The render thread's half of the playback ring.
///
/// Real-time: [`push_block`](Self::push_block), [`free_blocks`](Self::free_blocks) and
/// [`queued_frames`](Self::queued_frames) neither allocate nor block. The producer's own
/// queries never touch the stats, so polling them never resets the peaks. Drop it off the
/// real-time path, once the render loop has ended (RT-09).
pub struct BlockProducer {
    shared: Arc<Shared>,
    samples: Producer<f32>,
    meta: Producer<BlockMeta>,
    channels: NonZeroUsize,
    block_frames: usize,
    written: u64,
}

impl BlockProducer {
    pub(crate) fn new(
        shared: Arc<Shared>,
        samples: Producer<f32>,
        meta: Producer<BlockMeta>,
        channels: NonZeroUsize,
        block_frames: usize,
    ) -> Self {
        Self {
            shared,
            samples,
            meta,
            channels,
            block_frames,
            written: 0,
        }
    }

    /// Copies one block into the ring: `samples` holds whole interleaved frames, at most
    /// [`block_frames`](Self::block_frames) of them; `tag` is the timeline frame of its first
    /// frame. A short block plays only its own frames.
    ///
    /// # Errors
    ///
    /// [`PushError::Full`] when no slot is free (nothing is written), and
    /// [`PushError::ShapeMismatch`] when `samples` is not whole frames of the stream's channel
    /// count or holds more than a block.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn push_block(
        &mut self,
        samples: &[f32],
        tag: TimelineFrame,
        generation: Generation,
    ) -> Result<(), PushError> {
        let frames = samples.len() / self.channels;
        if samples.len() % self.channels != 0 || frames > self.block_frames {
            return Err(PushError::ShapeMismatch);
        }
        let frames = u32::try_from(frames).map_err(|_| PushError::ShapeMismatch)?;
        if self.meta.is_full() || self.samples.slots() < samples.len() {
            return Err(PushError::Full);
        }
        // Cannot fail: the space was checked, and only this thread writes (SPSC).
        self.samples
            .push_entire_slice(samples)
            .map_err(|_| PushError::Full)?;
        let protocol = &self.shared.playback;
        protocol.record(self.written, generation, frames);
        self.written = self.written.wrapping_add(1);
        // Release ↔ the Acquire load in `Protocol::queued_frames`: publishes the ledger entry.
        protocol.written.store(self.written, Ordering::Release);
        // Cannot fail: a metadata slot was free (checked above). rtrb's Release store ↔ the
        // callback's Acquire load publishes the samples and the metadata.
        self.meta
            .push(BlockMeta {
                tag,
                generation,
                frames,
            })
            .map_err(|_| PushError::Full)
    }

    /// Free slots, in blocks. Stale blocks occupy their slots until the callback discards
    /// them, as in the prototype.
    #[must_use]
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn free_blocks(&self) -> usize {
        self.meta.slots()
    }

    /// Arms underrun counting for `generation` (the player calls it after it has pushed the
    /// generation's first block), or disarms it (`None`: after the last block of a play range).
    /// The flag counts only while `generation` is current: a later
    /// [`flush`](crate::StreamControl::flush) disarms it, so arming a generation that was just
    /// flushed has no effect.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn set_playing_generation(&self, generation: Option<Generation>) {
        self.shared.playback.set_playing_generation(generation);
    }

    /// Frames of the current generation that are queued but not yet played. Stale blocks do not
    /// count, so the producer refills at once after a flush.
    #[must_use]
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn queued_frames(&self) -> u64 {
        self.shared.playback.queued_frames()
    }

    /// The most frames one block holds.
    #[must_use]
    pub fn block_frames(&self) -> usize {
        self.block_frames
    }

    /// The stream's output channel count: the interleaving of every block.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.channels.get()
    }
}

/// The first output frame of a callback that played the current generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FirstCurrentFrame {
    /// Its frame index in the callback's output buffer.
    pub offset: usize,
    /// Its timeline frame.
    pub timeline_frame: TimelineFrame,
}

/// What one [`fill`](crate::DeviceCallback::fill) did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fill {
    frames: usize,
    first_current: Option<FirstCurrentFrame>,
    starved: bool,
}

impl Fill {
    /// Whole frames in the output buffer.
    #[must_use]
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// The first frame of the current generation this callback played, if any. A capture take
    /// starts there.
    #[must_use]
    pub fn first_current(&self) -> Option<FirstCurrentFrame> {
        self.first_current
    }

    /// Whether the ring ran empty while the player was playing: an underrun.
    #[must_use]
    pub fn is_starved(&self) -> bool {
        self.starved
    }
}

/// The fade gain, in steps of 1/128: exact in `f32`, so every gain equals the prototype's
/// accumulated `fade_gain`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Fade {
    steps: u8,
}

impl Fade {
    const FULL: u8 = 128;
    const SILENT: Self = Self { steps: 0 };

    fn is_full(self) -> bool {
        self.steps >= Self::FULL
    }

    fn is_silent(self) -> bool {
        self.steps == 0
    }

    /// Frames until a fade-out reaches silence.
    fn frames_to_silence(self) -> usize {
        usize::from(self.steps)
    }

    fn gain(self) -> f32 {
        f32::from(self.steps) / f32::from(Self::FULL)
    }

    /// One frame of fade-in: the step is taken before the gain is applied, so the first frame
    /// plays at 1/128.
    fn step_in(&mut self) -> f32 {
        if self.steps < Self::FULL {
            self.steps += 1;
        }
        self.gain()
    }

    /// One frame of fade-out: the step is taken before the gain is applied, so the last frame
    /// plays at 0.
    fn step_out(&mut self) -> f32 {
        self.steps = self.steps.saturating_sub(1);
        self.gain()
    }
}

// One source of truth for the fade length (DES-01).
const _: () = assert!(Fade::FULL as usize == FADE_FRAMES);

/// The device callback's half of the playback ring.
pub(crate) struct BlockReader {
    samples: Consumer<f32>,
    meta: Consumer<BlockMeta>,
    channels: NonZeroUsize,
    fade: Fade,
    /// Frames played of the head block.
    read_offset: u32,
    /// Blocks finished (played or discarded) since the stream started.
    finished: u64,
    /// Generation of the last current block that started playing.
    last_generation: Option<Generation>,
}

impl BlockReader {
    pub(crate) fn new(
        samples: Consumer<f32>,
        meta: Consumer<BlockMeta>,
        channels: NonZeroUsize,
    ) -> Self {
        Self {
            samples,
            meta,
            channels,
            fade: Fade::SILENT,
            read_offset: 0,
            finished: 0,
            last_generation: None,
        }
    }

    pub(crate) fn fill(&mut self, shared: &Shared, out: &mut [f32]) -> Fill {
        let protocol = &shared.playback;
        let channels = self.channels.get();
        let frames = out.len() / self.channels;
        // Acquire ↔ the Release store in `Protocol::flush`.
        let min_generation = Generation::new(protocol.min_generation.load(Ordering::Acquire));
        // Armed for a current generation (Acquire inside, ↔ the Release store of the flag).
        let playing = protocol.playing_for(min_generation);

        let mut done = 0;
        let mut starved = false;
        let mut first_current = None;
        let mut last_played = None;
        while done < frames {
            let Ok(&meta) = self.meta.peek() else {
                // Empty ring: silence, an underrun only while playing, and the next material
                // fades in.
                silence(out, done.saturating_mul(channels));
                starved = playing;
                self.fade = Fade::SILENT;
                break;
            };
            let wanted = frames - done;
            let left = frames_of(meta.frames.saturating_sub(self.read_offset));
            if meta.generation.is_older_than(min_generation) {
                if !self.fade.is_silent() && left > 0 {
                    // Fade the stale audio out (D13: across stale blocks).
                    let n = left.min(wanted).min(self.fade.frames_to_silence());
                    if !self.fade_out(out, done, n) {
                        break;
                    }
                    done += n;
                    self.advance(n);
                    if !self.fade.is_silent() && n < left {
                        // The callback's buffer is full; the fade resumes in the next one.
                        continue;
                    }
                }
                // The fade is over or the block ran out: discard the rest of it.
                self.finish_head(protocol, meta);
                if !self.fade.is_silent() && !self.head_is_stale(min_generation) {
                    // The fade-out carries into the next stale block, and ends early only
                    // when no stale audio is left.
                    self.fade = Fade::SILENT;
                }
                continue;
            }
            if self.read_offset == 0 && self.last_generation != Some(meta.generation) {
                // The first block of a newer generation always fades in.
                self.fade = Fade::SILENT;
                self.last_generation = Some(meta.generation);
            }
            let n = left.min(wanted);
            if n > 0 {
                first_current.get_or_insert(FirstCurrentFrame {
                    offset: done,
                    timeline_frame: meta.tag.advanced_by(self.read_offset),
                });
                if !self.play(out, done, n) {
                    break;
                }
                done += n;
                self.advance(n);
                // One past the last frame handed to the device.
                last_played = Some(meta.tag.advanced_by(self.read_offset));
            }
            if self.read_offset >= meta.frames {
                self.finish_head(protocol, meta);
            }
        }
        // A trailing partial frame (no device produces one) stays silent.
        silence(out, frames.saturating_mul(channels));
        self.publish_position(protocol);
        if starved {
            shared.stats.count_underrun();
        }
        if let Some(frame) = last_played {
            shared.stats.set_timeline_frame(frame);
        }
        Fill {
            frames,
            first_current,
            starved,
        }
    }

    fn head_is_stale(&self, min_generation: Generation) -> bool {
        self.meta
            .peek()
            .is_ok_and(|meta| meta.generation.is_older_than(min_generation))
    }

    fn advance(&mut self, frames: usize) {
        let frames = u32::try_from(frames).unwrap_or(u32::MAX);
        self.read_offset = self.read_offset.saturating_add(frames);
    }

    /// Plays `n` frames of the head block into `out` from frame `at`, fading in when the fade
    /// is not full. False when the ring or the buffer has fewer frames, which the protocol
    /// rules out; the rest of the buffer is then silent.
    fn play(&mut self, out: &mut [f32], at: usize, n: usize) -> bool {
        let channels = self.channels.get();
        let (Some(dst), Ok(chunk)) = (
            out.get_mut(at * channels..(at + n) * channels),
            self.samples.read_chunk(n * channels),
        ) else {
            silence(out, at.saturating_mul(channels));
            return false;
        };
        let (first, second) = chunk.as_slices();
        if self.fade.is_full() {
            for (d, s) in dst.iter_mut().zip(first.iter().chain(second)) {
                *d = *s;
            }
        } else {
            let frames = first
                .chunks_exact(channels)
                .chain(second.chunks_exact(channels));
            for (frame_out, frame_in) in dst.chunks_exact_mut(channels).zip(frames) {
                let gain = self.fade.step_in();
                for (d, s) in frame_out.iter_mut().zip(frame_in) {
                    *d = *s * gain;
                }
            }
        }
        chunk.commit_all();
        true
    }

    /// Fades `n` frames of the (stale) head block out into `out` from frame `at`.
    fn fade_out(&mut self, out: &mut [f32], at: usize, n: usize) -> bool {
        let channels = self.channels.get();
        let (Some(dst), Ok(chunk)) = (
            out.get_mut(at * channels..(at + n) * channels),
            self.samples.read_chunk(n * channels),
        ) else {
            silence(out, at.saturating_mul(channels));
            return false;
        };
        let (first, second) = chunk.as_slices();
        let frames = first
            .chunks_exact(channels)
            .chain(second.chunks_exact(channels));
        for (frame_out, frame_in) in dst.chunks_exact_mut(channels).zip(frames) {
            let gain = self.fade.step_out();
            for (d, s) in frame_out.iter_mut().zip(frame_in) {
                *d = *s * gain;
            }
        }
        chunk.commit_all();
        true
    }

    /// Finishes the head block: discards its unplayed frames and frees its slot.
    fn finish_head(&mut self, protocol: &Protocol, meta: BlockMeta) {
        let left = frames_of(meta.frames.saturating_sub(self.read_offset));
        self.finished = self.finished.wrapping_add(1);
        self.read_offset = 0;
        // Published before the slot is freed: a producer that sees the free slot also sees
        // this position (Release here, rtrb's Release on the slot after it).
        self.publish_position(protocol);
        if left > 0 {
            // Cannot fail: the producer pushed the block's samples before its metadata.
            if let Ok(chunk) = self.samples.read_chunk(left * self.channels.get()) {
                chunk.commit_all();
            }
        }
        // Cannot fail: the head was peeked, and only this thread pops (SPSC).
        if let Ok(chunk) = self.meta.read_chunk(1) {
            chunk.commit_all();
        }
    }

    fn publish_position(&self, protocol: &Protocol) {
        // Release ↔ the Acquire load in `Protocol::queued_frames`.
        protocol.read_position.store(
            pack_halves(low_half(self.finished), self.read_offset),
            Ordering::Release,
        );
    }
}

/// Silences `out` from sample `from` on.
fn silence(out: &mut [f32], from: usize) {
    if let Some(rest) = out.get_mut(from..) {
        rest.fill(0.0);
    }
}
