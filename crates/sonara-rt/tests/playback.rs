//! The playback ring against the prototype's semantics (`native/src/engine.cpp`;
//! `tests/engine/test_realtime.py`) and the corrected fades:
//! D12 (`new_generation_always_fades_in`) and D13 (`stale_fade_continues_across_blocks`).
//!
//! The consumer is driven by hand, so every expectation is exact: the fades step by 1/128,
//! which `f32` represents exactly, as in the prototype.
#![cfg(not(loom))]

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use sonara_rt::{
    BLOCK_FRAMES, ConfigError, FADE_FRAMES, Generation, PushError, Stream, StreamConfig,
    TimelineFrame,
};

/// RT-02: the calls of the real-time threads (the device callback, the render thread's producer,
/// the stream's atomics) run inside `assert_no_alloc`, which aborts on any allocation or free.
#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const GEN1: Generation = Generation::new(1);
const GEN2: Generation = Generation::new(2);

fn config(channels: usize, ring_blocks: usize) -> StreamConfig {
    StreamConfig {
        output_channels: channels,
        input_channels: 0,
        block_frames: BLOCK_FRAMES,
        ring_blocks,
        capture_frames: 0,
    }
}

fn stereo(ring_blocks: usize) -> Stream {
    Stream::new(&config(2, ring_blocks)).expect("a valid configuration")
}

/// A stereo block of `frames` frames at `value`.
fn block(value: f32, frames: usize) -> Vec<f32> {
    vec![value; frames * 2]
}

fn push(stream: &mut Stream, value: f32, frames: usize, tag: i64, generation: Generation) {
    let data = block(value, frames);
    assert_no_alloc(|| {
        stream
            .producer
            .push_block(&data, TimelineFrame::new(tag), generation)
    })
    .expect("a free slot");
}

/// One callback of `frames` stereo frames; returns the left channel (both are equal).
fn fill(stream: &mut Stream, frames: usize) -> Vec<f32> {
    let mut out = vec![f32::NAN; frames * 2];
    assert_no_alloc(|| stream.callback.fill(&mut out));
    let (frames, _) = out.as_chunks::<2>();
    assert!(frames.iter().all(|[l, r]| l.to_bits() == r.to_bits()));
    frames.iter().map(|[l, _]| *l).collect()
}

/// The fade gain after `steps` steps of 1/128.
fn gain(steps: usize) -> f32 {
    f32::from(u16::try_from(steps).expect("a fade step")) / 128.0
}

fn bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|v| v.to_bits()).collect()
}

/// `value` faded in over the first 128 frames, then at full gain.
fn faded_in(value: f32, frames: usize) -> Vec<f32> {
    (1..=frames)
        .map(|k| value * gain(k.min(FADE_FRAMES)))
        .collect()
}

#[test]
fn ring_rejects_write_when_full() {
    let mut stream = stereo(8);
    for i in 0..8 {
        push(&mut stream, 0.25, 512, i * 512, GEN1);
    }
    assert_eq!(assert_no_alloc(|| stream.producer.free_blocks()), 0);
    let data = block(0.25, 512);
    let rejected = assert_no_alloc(|| {
        stream
            .producer
            .push_block(&data, TimelineFrame::new(4096), GEN1)
    });
    assert_eq!(rejected, Err(PushError::Full));
    assert_eq!(assert_no_alloc(|| stream.producer.queued_frames()), 8 * 512);

    fill(&mut stream, 512);
    assert_eq!(assert_no_alloc(|| stream.producer.free_blocks()), 1);
    push(&mut stream, 0.25, 512, 4096, GEN1);
    assert_eq!(assert_no_alloc(|| stream.producer.free_blocks()), 0);
}

#[test]
fn consumer_never_runs_ahead_of_writes() {
    let mut stream = stereo(8);
    assert_eq!(assert_no_alloc(|| stream.control.position()), None);
    for i in 0..8 {
        push(&mut stream, 0.25, 512, i * 512, GEN1);
    }
    assert_no_alloc(|| stream.control.set_playing(true));
    let written_end = 8 * 512;
    let mut previous = 0;
    for _ in 0..20 {
        fill(&mut stream, 256);
        let position = assert_no_alloc(|| stream.control.position()).unwrap().get();
        assert!(
            position >= previous && position <= written_end,
            "{position}"
        );
        previous = position;
    }
    // Everything was played, and an empty ring does not move the position.
    assert_eq!(previous, written_end);
    assert_eq!(assert_no_alloc(|| stream.control.stats()).underruns, 4);
}

#[test]
fn flush_zeroes_queued_frames() {
    let mut stream = stereo(16);
    for i in 0..16 {
        push(&mut stream, 0.1, 512, i * 512, GEN1);
    }
    assert_no_alloc(|| stream.control.flush(GEN2));
    // Stale blocks no longer count as queued audio...
    assert_eq!(assert_no_alloc(|| stream.producer.queued_frames()), 0);
    assert_eq!(assert_no_alloc(|| stream.control.stats()).queued_frames, 0);
    // ...but keep their slots until the callback discards them.
    assert_eq!(assert_no_alloc(|| stream.producer.free_blocks()), 0);
    let out = fill(&mut stream, 256);
    assert!(out.iter().all(|v| *v == 0.0));
    assert_eq!(assert_no_alloc(|| stream.producer.free_blocks()), 16);
    // New-generation audio plays after the stale material was dropped.
    push(&mut stream, 0.1, 512, 100_000, GEN2);
    fill(&mut stream, 512);
    assert_eq!(
        assert_no_alloc(|| stream.control.position()),
        Some(TimelineFrame::new(100_512))
    );
}

#[test]
fn stale_blocks_fade_out_over_128_frames_then_discard() {
    let mut stream = stereo(4);
    for i in 0..3 {
        push(&mut stream, 0.5, 512, i * 512, GEN1);
    }
    assert_eq!(bits(&fill(&mut stream, 512)), bits(&faded_in(0.5, 512)));
    assert_eq!(fill(&mut stream, 256), vec![0.5; 256]);

    assert_no_alloc(|| stream.control.flush(GEN2));
    let out = fill(&mut stream, 512);
    let fade: Vec<f32> = (0..FADE_FRAMES).map(|i| 0.5 * gain(127 - i)).collect();
    assert_eq!(bits(&out[..FADE_FRAMES]), bits(&fade));
    assert_eq!(out[FADE_FRAMES - 1], 0.0);
    assert!(out[FADE_FRAMES..].iter().all(|v| *v == 0.0));
    // Both stale blocks are gone.
    assert_eq!(assert_no_alloc(|| stream.producer.free_blocks()), 4);
    assert_eq!(assert_no_alloc(|| stream.producer.queued_frames()), 0);
}

#[test]
fn new_generation_fades_in() {
    let mut stream = stereo(4);
    for i in 0..3 {
        push(&mut stream, 0.5, 512, i * 512, GEN1);
    }
    fill(&mut stream, 768);
    assert_no_alloc(|| stream.control.flush(GEN2));
    push(&mut stream, 0.25, 512, 9000, GEN2);
    let out = fill(&mut stream, 512);
    // The stale block fades out over 128 frames, then the new generation fades in.
    let fade: Vec<f32> = (0..FADE_FRAMES).map(|i| 0.5 * gain(127 - i)).collect();
    assert_eq!(bits(&out[..FADE_FRAMES]), bits(&fade));
    assert_eq!(bits(&out[FADE_FRAMES..]), bits(&faded_in(0.25, 384)));
    assert_eq!(
        assert_no_alloc(|| stream.control.position()),
        Some(TimelineFrame::new(9384))
    );
}

/// The flush finds no stale block and the ring never runs empty. The prototype played
/// the new generation at full gain; it must fade in.
#[test]
fn new_generation_always_fades_in() {
    let mut stream = stereo(4);
    push(&mut stream, 0.5, 512, 0, GEN1);
    // The block ends exactly with the callback: the ring did not run empty.
    fill(&mut stream, 512);
    assert_no_alloc(|| stream.control.flush(GEN2));
    push(&mut stream, 0.25, 512, 4000, GEN2);
    let out = fill(&mut stream, 256);
    assert_eq!(bits(&out), bits(&faded_in(0.25, 256)));
    assert_ne!(out[0], 0.25, "the prototype's defect: no fade-in");

    // Without a flush, too: a newer generation directly behind the old one fades in.
    let mut stream = stereo(4);
    push(&mut stream, 0.5, 512, 0, GEN1);
    push(&mut stream, 0.25, 512, 512, GEN2);
    let out = fill(&mut stream, 768);
    assert_eq!(bits(&out[..512]), bits(&faded_in(0.5, 512)));
    assert_eq!(bits(&out[512..]), bits(&faded_in(0.25, 256)));
}

/// The stale block ends 50 frames into the fade. The prototype cut the gain to 0;
/// the fade must continue into the next stale block.
#[test]
fn stale_fade_continues_across_blocks() {
    let mut stream = stereo(4);
    push(&mut stream, 0.5, 512, 0, GEN1);
    push(&mut stream, 0.25, 512, 512, GEN1);
    push(&mut stream, 0.125, 512, 1024, GEN1);
    fill(&mut stream, 462);
    assert_no_alloc(|| stream.control.flush(GEN2));
    let out = fill(&mut stream, 256);
    let fade: Vec<f32> = (0..FADE_FRAMES)
        .map(|i| {
            let value = if i < 50 { 0.5 } else { 0.25 };
            value * gain(127 - i)
        })
        .collect();
    assert_eq!(bits(&out[..FADE_FRAMES]), bits(&fade));
    assert!(out[FADE_FRAMES..].iter().all(|v| *v == 0.0));
    assert_eq!(assert_no_alloc(|| stream.producer.free_blocks()), 4);
}

/// The fade-out ends early only when no stale audio is left; then the new generation
/// fades in from silence.
#[test]
fn stale_fade_ends_when_no_stale_audio_is_left() {
    let mut stream = stereo(4);
    push(&mut stream, 0.5, 512, 0, GEN1);
    fill(&mut stream, 462);
    assert_no_alloc(|| stream.control.flush(GEN2));
    push(&mut stream, 0.25, 512, 7000, GEN2);
    let out = fill(&mut stream, 256);
    let fade: Vec<f32> = (0..50).map(|i| 0.5 * gain(127 - i)).collect();
    assert_eq!(bits(&out[..50]), bits(&fade));
    assert_eq!(bits(&out[50..]), bits(&faded_in(0.25, 206)));
}

#[test]
fn underrun_counted_only_while_playing() {
    let mut stream = stereo(4);
    for _ in 0..3 {
        fill(&mut stream, 256);
    }
    assert_eq!(assert_no_alloc(|| stream.control.stats()).underruns, 0);
    assert_no_alloc(|| stream.control.set_playing(true));
    fill(&mut stream, 256);
    assert_eq!(assert_no_alloc(|| stream.control.stats()).underruns, 1);
    assert_no_alloc(|| stream.control.set_playing(false));
    fill(&mut stream, 256);
    assert_eq!(assert_no_alloc(|| stream.control.stats()).underruns, 1);
}

/// The player arms the flag for one generation. A flush to a newer generation disarms it
/// at once, so neither a restart nor a seek counts an underrun while the new generation's first
/// block is on its way, and arming a generation that is already stale arms nothing.
#[test]
fn playing_is_armed_per_generation() {
    let mut stream = stereo(4);
    push(&mut stream, 0.5, 512, 0, GEN1);
    assert_no_alloc(|| stream.producer.set_playing_generation(Some(GEN1)));
    assert!(assert_no_alloc(|| stream.control.is_playing()));
    fill(&mut stream, 512);
    // The flush disarms: the stale-free, empty ring is no underrun.
    assert_no_alloc(|| stream.control.flush(GEN2));
    assert!(!assert_no_alloc(|| stream.control.is_playing()));
    fill(&mut stream, 256);
    assert_eq!(assert_no_alloc(|| stream.control.stats()).underruns, 0);
    // A render thread that arms the flushed generation late arms nothing.
    assert_no_alloc(|| stream.producer.set_playing_generation(Some(GEN1)));
    assert!(!assert_no_alloc(|| stream.control.stats()).playing);
    fill(&mut stream, 256);
    assert_eq!(assert_no_alloc(|| stream.control.stats()).underruns, 0);
    // The current generation counts again once armed, and a disarm (the end of a play range)
    // stops counting while the ring drains.
    assert_no_alloc(|| stream.producer.set_playing_generation(Some(GEN2)));
    fill(&mut stream, 256);
    assert_eq!(assert_no_alloc(|| stream.control.stats()).underruns, 1);
    assert_no_alloc(|| stream.producer.set_playing_generation(None));
    fill(&mut stream, 256);
    assert_eq!(assert_no_alloc(|| stream.control.stats()).underruns, 1);
    // `set_playing(true)` arms the generation that is current when it is called.
    assert_no_alloc(|| stream.control.set_playing(true));
    assert!(assert_no_alloc(|| stream.control.is_playing()));
    assert_no_alloc(|| stream.control.flush(Generation::new(3)));
    assert!(!assert_no_alloc(|| stream.control.is_playing()));
}

#[test]
fn underrun_at_most_once_per_callback() {
    let mut stream = stereo(4);
    assert_no_alloc(|| stream.control.set_playing(true));
    // A block that ends exactly with the callback is no underrun.
    push(&mut stream, 0.5, 512, 0, GEN1);
    fill(&mut stream, 512);
    assert_eq!(assert_no_alloc(|| stream.control.stats()).underruns, 0);
    // A short block: the ring runs empty once in this callback.
    push(&mut stream, 0.5, 100, 512, GEN1);
    let out = fill(&mut stream, 256);
    assert_eq!(bits(&out[..100]), bits(&[0.5; 100]));
    assert!(out[100..].iter().all(|v| *v == 0.0));
    assert_eq!(assert_no_alloc(|| stream.control.stats()).underruns, 1);
    fill(&mut stream, 256);
    assert_eq!(assert_no_alloc(|| stream.control.stats()).underruns, 2);
    // Discarding stale blocks into an empty ring counts once, too, while the new generation
    // is armed (a flush alone disarms the flag: D11, `playing_is_armed_per_generation`).
    push(&mut stream, 0.5, 512, 0, GEN1);
    push(&mut stream, 0.5, 512, 512, GEN1);
    assert_no_alloc(|| stream.control.flush(GEN2));
    assert_no_alloc(|| stream.producer.set_playing_generation(Some(GEN2)));
    let fill_result = {
        let mut out = vec![0.0; 512];
        assert_no_alloc(|| stream.callback.fill(&mut out))
    };
    assert!(fill_result.is_starved());
    assert_eq!(assert_no_alloc(|| stream.control.stats()).underruns, 3);
}

#[test]
fn queued_frames_counts_only_the_current_generation() {
    let mut stream = stereo(4);
    push(&mut stream, 0.5, 512, 0, GEN1);
    push(&mut stream, 0.5, 512, 512, GEN1);
    assert_no_alloc(|| stream.control.flush(GEN2));
    push(&mut stream, 0.25, 300, 5000, GEN2);
    assert_eq!(assert_no_alloc(|| stream.producer.queued_frames()), 300);
    // The stale blocks are discarded silently (nothing was playing), then 100 frames play.
    fill(&mut stream, 100);
    assert_eq!(assert_no_alloc(|| stream.producer.queued_frames()), 200);
    assert_eq!(assert_no_alloc(|| stream.control.queued_frames()), 200);
}

#[test]
fn queued_frames_subtract_the_played_part_of_the_head() {
    let mut stream = stereo(4);
    push(&mut stream, 0.5, 512, 0, GEN1);
    push(&mut stream, 0.5, 512, 512, GEN1);
    assert_eq!(assert_no_alloc(|| stream.producer.queued_frames()), 1024);
    fill(&mut stream, 100);
    assert_eq!(assert_no_alloc(|| stream.producer.queued_frames()), 924);
    fill(&mut stream, 412);
    assert_eq!(assert_no_alloc(|| stream.producer.queued_frames()), 512);
    fill(&mut stream, 1);
    assert_eq!(assert_no_alloc(|| stream.producer.queued_frames()), 511);
}

#[test]
fn queued_frames_stay_exact_across_many_ring_cycles() {
    let mut stream = stereo(3);
    let mut tag = 0;
    for _ in 0..50 {
        while assert_no_alloc(|| stream.producer.free_blocks()) > 0 {
            push(&mut stream, 0.5, 512, tag, GEN1);
            tag += 512;
        }
        assert_eq!(assert_no_alloc(|| stream.producer.queued_frames()), 3 * 512);
        fill(&mut stream, 700);
        assert_eq!(
            assert_no_alloc(|| stream.producer.queued_frames()),
            3 * 512 - 700
        );
        fill(&mut stream, 324);
    }
}

#[test]
fn short_block_plays_only_its_frames() {
    let mut stream = stereo(4);
    push(&mut stream, 0.5, 100, 1000, GEN1);
    let out = fill(&mut stream, 256);
    assert_eq!(bits(&out[..100]), bits(&faded_in(0.5, 100)));
    assert!(out[100..].iter().all(|v| *v == 0.0));
    assert_eq!(
        assert_no_alloc(|| stream.control.position()),
        Some(TimelineFrame::new(1100))
    );
}

#[test]
fn push_rejects_a_wrong_shape() {
    let mut stream = stereo(4);
    let tag = TimelineFrame::new(0);
    let odd = [0.5_f32; 3];
    assert_eq!(
        assert_no_alloc(|| stream.producer.push_block(&odd, tag, GEN1)),
        Err(PushError::ShapeMismatch)
    );
    let long = block(0.5, 513);
    assert_eq!(
        assert_no_alloc(|| stream.producer.push_block(&long, tag, GEN1)),
        Err(PushError::ShapeMismatch)
    );
    assert_eq!(assert_no_alloc(|| stream.producer.free_blocks()), 4);
    assert_eq!(
        PushError::ShapeMismatch.to_string(),
        "block shape does not match the stream"
    );
    let whole = block(0.5, 512);
    assert!(assert_no_alloc(|| stream.producer.push_block(&whole, tag, GEN1)).is_ok());
}

#[test]
fn timeline_frame_is_one_past_the_last_frame() {
    let mut stream = stereo(4);
    // The player tags blocks with the source frame minus the graph latency: possibly negative.
    push(&mut stream, 0.5, 512, -300, GEN1);
    fill(&mut stream, 256);
    assert_eq!(
        assert_no_alloc(|| stream.control.position()),
        Some(TimelineFrame::new(-44))
    );
    assert_eq!(
        assert_no_alloc(|| stream.control.stats()).timeline_frame,
        Some(TimelineFrame::new(-44))
    );
}

#[test]
fn channels_stay_interleaved() {
    let mut stream = Stream::new(&config(3, 4)).unwrap();
    let frames = 200;
    let samples: Vec<f32> = (0..frames).flat_map(|_| [0.25, 0.5, 0.75]).collect();
    assert_no_alloc(|| {
        stream
            .producer
            .push_block(&samples, TimelineFrame::new(0), GEN1)
    })
    .unwrap();
    let mut out = vec![f32::NAN; 256 * 3];
    assert_no_alloc(|| stream.callback.fill(&mut out));
    for (k, frame) in out.as_chunks::<3>().0.iter().enumerate() {
        let g = if k < frames {
            gain((k + 1).min(FADE_FRAMES))
        } else {
            0.0
        };
        assert_eq!(
            bits(frame),
            bits(&[0.25 * g, 0.5 * g, 0.75 * g]),
            "frame {k}"
        );
    }
}

#[test]
fn a_partial_trailing_frame_is_silenced() {
    let mut stream = stereo(4);
    push(&mut stream, 0.5, 512, 0, GEN1);
    let mut out = vec![f32::NAN; 5];
    let result = assert_no_alloc(|| stream.callback.fill(&mut out));
    assert_eq!(result.frames(), 2);
    assert_eq!(out[4], 0.0);
}

#[test]
fn generations_compare_across_the_wrap() {
    let last = Generation::new(u32::MAX);
    assert_eq!(last.next(), Generation::new(0));
    assert!(last.is_older_than(last.next()));
    assert!(GEN1.is_older_than(GEN2));
    assert!(!GEN2.is_older_than(GEN2));
    assert!(!GEN2.is_older_than(GEN1));
    assert!(!Generation::new(0).is_older_than(last));
}

#[test]
fn stream_config_is_validated() {
    let check = |config: StreamConfig| Stream::new(&config).err();
    let base = config(2, 4);
    let channels = Some(ConfigError::UnsupportedChannelCount);
    let small = Some(ConfigError::RingTooSmall);
    assert_eq!(
        check(StreamConfig {
            output_channels: 0,
            ..base
        }),
        channels
    );
    assert_eq!(
        check(StreamConfig {
            output_channels: 33,
            ..base
        }),
        channels
    );
    assert_eq!(
        check(StreamConfig {
            input_channels: 33,
            ..base
        }),
        channels
    );
    assert_eq!(
        check(StreamConfig {
            block_frames: 15,
            ..base
        }),
        small
    );
    assert_eq!(
        check(StreamConfig {
            ring_blocks: 1,
            ..base
        }),
        small
    );
    assert_eq!(
        check(StreamConfig {
            input_channels: 1,
            ..base
        }),
        small
    );
    let huge = StreamConfig {
        block_frames: usize::MAX / 2,
        ..base
    };
    assert_eq!(check(huge), Some(ConfigError::RingTooLarge));
    assert_eq!(
        check(StreamConfig {
            output_channels: 32,
            ..base
        }),
        None
    );
    assert_eq!(
        ConfigError::UnsupportedChannelCount.to_string(),
        "unsupported channel count"
    );
    assert_eq!(ConfigError::RingTooSmall.to_string(), "ring too small");
}
