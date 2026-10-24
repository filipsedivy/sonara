//! Media off the render thread: the prefetch thread reads the audio
//! clips ahead of the render thread.
//!
//! Not real-time code: the clip readers do file I/O behind their locks (`sonara-media`).
//!
//! - **Sources.** The builder thread hands over the clip sources of the graph that plays
//!   ([`GraphBuilder::key_media`](crate::GraphBuilder)), keyed: a rebuild that plays the same
//!   clips keeps the key, and the audio already read ahead stays valid.
//! - **Feeds.** Every start and seek opens a feed for its generation: a `sonara_rt::handoff`
//!   channel of [`MediaBlock`]s along the player's source-frame sequence, the same sequence the
//!   render thread walks (`frames::advance`: loop wraps included). The control thread reads the
//!   first blocks itself before it starts the render thread on the generation (priming), so a
//!   start or a seek never begins with silent clips; the prefetch thread then keeps the feed
//!   [`PREFETCH_BLOCKS`] blocks ahead. A rebuild with other clips opens a feed from the render
//!   thread's current position.
//! - **Recycling.** The render thread copies a block's audio into the graph and returns the
//!   block through the hand-off's return ring; it is refilled here, so the steady state
//!   allocates nothing on either side.
//! - **Errors.** A clip that cannot be read plays silent in that block; the error is counted
//!   and kept for the status (RT-12).

use sonara_rt::{Generation, handoff};

use super::frames::{BLOCK, LoopRange, advance};
use super::message::{MediaBlock, MediaFeed};
use crate::graph::{ClipSources, MediaKey};

/// How many blocks a feed reads ahead of the render thread (about 340 ms at 48 kHz).
pub(crate) const PREFETCH_BLOCKS: usize = 32;

/// Where a feed starts: a position in a generation's source-frame sequence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FeedStart {
    pub(crate) generation: Generation,
    /// The position of the first block in the sequence.
    pub(crate) index: u64,
    /// Its source frame.
    pub(crate) frame: i64,
    pub(crate) looped: Option<LoopRange>,
    /// Where a play range ends; no block at or after it is read.
    pub(crate) end: Option<i64>,
}

/// The sending end of the current feed and its position in the sequence.
struct Writer {
    sender: handoff::Sender<MediaBlock>,
    key: MediaKey,
    start: FeedStart,
    /// Blocks sent.
    sent: u64,
}

/// What the prefetch thread and the control thread share (behind a lock; both are
/// non-real-time): the clip sources and the feed being filled.
#[derive(Default)]
pub(crate) struct Prefetch {
    sources: Option<(MediaKey, ClipSources)>,
    writer: Option<Writer>,
    /// Blocks the render thread returned, to be refilled.
    pool: Vec<MediaBlock>,
    errors: u64,
    last_error: Option<String>,
}

impl std::fmt::Debug for Prefetch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prefetch")
            .field(
                "tracks",
                &self.sources.as_ref().map(|(_, s)| s.tracks.len()),
            )
            .field("feeding", &self.writer.is_some())
            .field("errors", &self.errors)
            .finish_non_exhaustive()
    }
}

impl Prefetch {
    /// Replaces the clip sources with the sources of media key `key`. The feed of the old
    /// sources stops; the control thread opens a feed for the new ones.
    pub(crate) fn set_sources(&mut self, key: MediaKey, sources: ClipSources) {
        let tracks = sources.tracks.len();
        self.pool.retain(|block| block.tracks.len() == tracks);
        self.sources = Some((key, sources));
    }

    /// Opens the feed of a sequence from `start`, reading its first `primed` blocks now.
    /// Returns the render thread's end, or `None` when the graph plays no clips. The previous
    /// feed stops.
    pub(crate) fn begin(&mut self, start: FeedStart, primed: usize) -> Option<MediaFeed> {
        self.writer = None;
        let key = match &self.sources {
            Some((key, sources)) if !sources.tracks.is_empty() => *key,
            _ => return None,
        };
        let (sender, receiver) = handoff::channel(PREFETCH_BLOCKS);
        self.writer = Some(Writer {
            sender,
            key,
            start,
            sent: 0,
        });
        for _ in 0..primed.min(PREFETCH_BLOCKS) {
            if !self.produce() {
                break;
            }
        }
        Some(MediaFeed { receiver })
    }

    /// Stops the current feed (a pause or a stop).
    pub(crate) fn end(&mut self) {
        self.writer = None;
    }

    /// Takes back the blocks the render thread returned and reads the feed's next block, unless
    /// the feed holds [`PREFETCH_BLOCKS`] blocks or its sequence has ended. Returns whether it
    /// read one. The prefetch thread calls it in a loop and releases the lock between blocks,
    /// so a control call that opens a feed waits for one block at most.
    pub(crate) fn fill_one(&mut self) -> bool {
        let tracks = self.sources.as_ref().map_or(0, |(_, s)| s.tracks.len());
        if let Some(writer) = &mut self.writer {
            while let Some(block) = writer.sender.take_returned() {
                if block.tracks.len() == tracks {
                    self.pool.push(block);
                }
            }
        }
        self.produce()
    }

    /// Clip reads that failed (their blocks played silent), and the last error.
    pub(crate) fn errors(&self) -> (u64, Option<&str>) {
        (self.errors, self.last_error.as_deref())
    }

    /// Reads and sends the feed's next block; false when there is no feed, the feed is full or
    /// its sequence has ended.
    fn produce(&mut self) -> bool {
        let Self {
            sources,
            writer,
            pool,
            errors,
            last_error,
        } = self;
        let (Some(writer), Some((key, sources))) = (writer.as_mut(), sources.as_mut()) else {
            return false;
        };
        if *key != writer.key
            || writer
                .start
                .end
                .is_some_and(|end| writer.start.frame >= end)
        {
            return false;
        }
        let in_flight = writer.sent.saturating_sub(writer.sender.received());
        if in_flight >= PREFETCH_BLOCKS as u64 {
            return false;
        }
        let mut block = pool.pop().unwrap_or_else(|| MediaBlock {
            key: writer.key,
            generation: writer.start.generation,
            index: 0,
            frame: 0,
            tracks: vec![vec![[0.0; 2]; BLOCK]; sources.tracks.len()],
        });
        block.key = writer.key;
        block.generation = writer.start.generation;
        block.index = writer.start.index;
        block.frame = writer.start.frame;
        for (buffer, (_, source)) in block.tracks.iter_mut().zip(&mut sources.tracks) {
            buffer.fill([0.0; 2]);
            if let Err(error) = source.render(writer.start.frame, buffer) {
                buffer.fill([0.0; 2]);
                *errors = errors.wrapping_add(1);
                *last_error = Some(error.to_string());
            }
        }
        match writer.sender.send(block) {
            Ok(()) => {
                writer.sent += 1;
                writer.start.index = writer.start.index.wrapping_add(1);
                writer.start.frame = advance(writer.start.frame, writer.start.looped).1;
                true
            }
            Err(sonara_rt::queue::Full(block)) => {
                pool.push(block);
                false
            }
        }
    }
}
