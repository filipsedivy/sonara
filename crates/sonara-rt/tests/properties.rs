//! Property tests of the playback consumer (TST-09).
//!
//! - `fill` is frame-sequential: the device's callback sizes do not change what plays. Filling
//!   over arbitrary callback sizes gives bit-identical output, position and queue depth to one
//!   large fill, through flushes, fades, short blocks and generation changes.
//! - A fade never amplifies: every output sample is at most the largest source sample.
#![cfg(not(loom))]

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use proptest::collection::vec;
use proptest::prelude::*;
use sonara_rt::{Generation, Stream, StreamConfig, TimelineFrame};

/// RT-02: the calls of the real-time threads (the device callback, the render thread's producer,
/// the stream's atomics) run inside `assert_no_alloc`, which aborts on any allocation or free.
#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const BLOCK: usize = 64;

#[derive(Clone, Debug)]
struct Block {
    generation: u32,
    frames: usize,
    level: f32,
}

impl Block {
    /// Stereo samples: a ramp on the left, its negation on the right, so a frame played twice,
    /// skipped or swapped changes the output.
    fn samples(&self) -> Vec<f32> {
        (0..u16::try_from(self.frames).expect("a short block"))
            .flat_map(|i| {
                let left = self.level + f32::from(i) / 1024.0;
                [left, -left]
            })
            .collect()
    }
}

fn blocks() -> impl Strategy<Value = Vec<Block>> {
    vec(
        (1_u32..=3, 0..=BLOCK, -0.9_f32..0.9).prop_map(|(generation, frames, level)| Block {
            generation,
            frames,
            level,
        }),
        1..10,
    )
}

#[derive(Debug, PartialEq)]
struct Outcome {
    output: Vec<u32>,
    position: Option<TimelineFrame>,
    queued: u64,
}

fn play(
    blocks: &[Block],
    playing: bool,
    prefix: usize,
    flush: Option<u32>,
    callbacks: &[usize],
) -> Outcome {
    let mut stream = Stream::new(&StreamConfig {
        output_channels: 2,
        input_channels: 0,
        block_frames: BLOCK,
        ring_blocks: blocks.len().max(2),
        capture_frames: 0,
    })
    .expect("a valid configuration");
    let mut tag = 0;
    for block in blocks {
        let samples = block.samples();
        assert_no_alloc(|| {
            stream.producer.push_block(
                &samples,
                TimelineFrame::new(tag),
                Generation::new(block.generation),
            )
        })
        .expect("a free slot");
        tag += i64::try_from(block.frames).expect("a short block");
    }
    assert_no_alloc(|| stream.control.set_playing(playing));
    let mut head = vec![0.0; prefix * 2];
    assert_no_alloc(|| stream.callback.fill(&mut head));
    if let Some(generation) = flush {
        assert_no_alloc(|| stream.control.flush(Generation::new(generation)));
    }
    let mut output = Vec::new();
    for frames in callbacks {
        let mut out = vec![f32::NAN; frames * 2];
        assert_no_alloc(|| stream.callback.fill(&mut out));
        output.extend(out.iter().map(|v| v.to_bits()));
    }
    Outcome {
        output,
        position: assert_no_alloc(|| stream.control.position()),
        queued: assert_no_alloc(|| stream.producer.queued_frames()),
    }
}

proptest! {
    #[test]
    fn fill_over_any_callback_sizes_equals_one_fill(
        blocks in blocks(),
        playing in any::<bool>(),
        prefix in 0_usize..200,
        flush in proptest::option::of(1_u32..=3),
        callbacks in vec(1_usize..=150, 1..12),
    ) {
        let total: usize = callbacks.iter().sum();
        let pieces = play(&blocks, playing, prefix, flush, &callbacks);
        let whole = play(&blocks, playing, prefix, flush, &[total]);
        prop_assert_eq!(&pieces, &whole);

        let loudest = blocks
            .iter()
            .flat_map(Block::samples)
            .fold(0.0_f32, |max, v| max.max(v.abs()));
        prop_assert!(pieces
            .output
            .iter()
            .all(|bits| f32::from_bits(*bits).abs() <= loudest));
    }
}
