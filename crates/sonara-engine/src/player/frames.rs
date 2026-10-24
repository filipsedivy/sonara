//! Real-time: runs on the audio render thread (see docs/audio-engine.md); the prefetch thread
//! follows the same sequence.
//!
//! The player's source-frame sequence (`_next_block` in the prototype's `player.py`): the graph
//! always renders whole blocks; at a loop end the block is cut sample-accurately in source time
//! and rendering continues from the loop start. The render thread and the prefetch thread both
//! walk this sequence, so the clip audio read ahead is the audio the graph asks for (DES-01).
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

use sonara_rt::BLOCK_FRAMES;

/// Frames per player block (`BLOCK` in the prototype's player).
pub(crate) const BLOCK: usize = BLOCK_FRAMES;

/// [`BLOCK`] as a frame offset.
#[expect(
    clippy::cast_possible_wrap,
    reason = "512 fits every integer type; checked by the assertion below"
)]
pub(crate) const BLOCK_I64: i64 = BLOCK as i64;
const _: () = assert!(BLOCK_I64 == 512);

/// A loop in source frames, `[start, end)`, longer than one block (the prototype rejects
/// shorter loops with "loop range is too short").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopRange {
    start: i64,
    end: i64,
}

impl LoopRange {
    /// The loop `[start, end)`; `None` unless `end > start + 512`.
    #[must_use]
    pub fn new(start: i64, end: i64) -> Option<Self> {
        (end > start.saturating_add(BLOCK_I64)).then_some(Self { start, end })
    }

    /// The first frame of the loop.
    #[must_use]
    pub fn start(self) -> i64 {
        self.start
    }

    /// The frame after the loop, where playback wraps to [`LoopRange::start`].
    #[must_use]
    pub fn end(self) -> i64 {
        self.end
    }
}

/// One step of the sequence from the block at source frame `src`: how many of its frames are
/// kept, and the next source frame. Without a loop, all 512 frames and `src + 512`. A block that
/// contains the loop end keeps the frames before it and continues at the loop start; a block
/// that starts at or after the loop end (after a seek past it) is kept whole and also continues
/// at the loop start, as in the prototype.
pub(crate) fn advance(src: i64, looped: Option<LoopRange>) -> (usize, i64) {
    let next = src.saturating_add(BLOCK_I64);
    let Some(range) = looped else {
        return (BLOCK, next);
    };
    if src < range.end && range.end <= next {
        let kept = usize::try_from(range.end.saturating_sub(src)).unwrap_or(BLOCK);
        (kept.min(BLOCK), range.start)
    } else if src >= range.end {
        (BLOCK, range.start)
    } else {
        (BLOCK, next)
    }
}

/// The source frame at position `index` of the sequence that starts at `start` (the prefetch
/// thread's restart point after a rebuild). Walks the sequence; arithmetic only.
pub(crate) fn frame_at(start: i64, looped: Option<LoopRange>, index: u64) -> i64 {
    let mut frame = start;
    for _ in 0..index {
        frame = advance(frame, looped).1;
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::{BLOCK, LoopRange, advance, frame_at};

    #[test]
    fn loops_must_be_longer_than_a_block() {
        assert_eq!(LoopRange::new(0, 512), None);
        let range = LoopRange::new(100, 613).expect("513 frames");
        assert_eq!((range.start(), range.end()), (100, 613));
    }

    /// The prototype's `_next_block` rules, on the loop of the golden `loop-metronome`
    /// scenario (95 960 to 143 900): the block at 143 576 keeps 324 frames and wraps.
    #[test]
    fn the_sequence_cuts_at_the_loop_end_in_source_time() {
        let range = LoopRange::new(95_960, 143_900);
        assert_eq!(advance(0, None), (BLOCK, 512));
        assert_eq!(advance(143_064, range), (BLOCK, 143_576));
        assert_eq!(advance(143_576, range), (324, 95_960));
        // Exactly at the end: the whole block is kept.
        assert_eq!(advance(143_388, range), (BLOCK, 95_960));
        // At or past the end (a seek): kept whole, then the loop start.
        assert_eq!(advance(143_900, range), (BLOCK, 95_960));
        assert_eq!(advance(200_000, range), (BLOCK, 95_960));
        // 47 940 frames per pass: 93 full blocks, then the cut block.
        assert_eq!(frame_at(95_960, range, 93), 143_576);
        assert_eq!(frame_at(95_960, range, 94), 95_960);
        assert_eq!(frame_at(95_960, range, 0), 95_960);
    }
}
