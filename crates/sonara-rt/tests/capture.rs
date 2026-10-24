//! The capture ring: the take's capture start comes from the same callback, the
//! prototype's overrun rule, and exact take boundaries.
#![cfg(not(loom))]

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use sonara_rt::{BLOCK_FRAMES, Generation, Stream, StreamConfig, TimelineFrame};

/// RT-02: the calls of the real-time threads (the device callback, the render thread's producer,
/// the stream's atomics) run inside `assert_no_alloc`, which aborts on any allocation or free.
#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const GEN1: Generation = Generation::new(1);
const GEN2: Generation = Generation::new(2);
const PERIOD: usize = 256;

/// Stereo output, mono input.
fn duplex(capture_frames: usize) -> Stream {
    Stream::new(&StreamConfig {
        output_channels: 2,
        input_channels: 1,
        block_frames: BLOCK_FRAMES,
        ring_blocks: 4,
        capture_frames,
    })
    .expect("a valid configuration")
}

fn push(stream: &mut Stream, tag: i64, generation: Generation) {
    assert_no_alloc(|| {
        stream.producer.push_block(
            &[0.25; 2 * BLOCK_FRAMES],
            TimelineFrame::new(tag),
            generation,
        )
    })
    .expect("a free slot");
}

/// Mono input whose samples count up from `first`, so each captured sample shows where it came
/// from.
fn input(first: u16) -> Vec<f32> {
    (0..u16::try_from(PERIOD).expect("a short period"))
        .map(|i| f32::from(first + i))
        .collect()
}

/// One duplex callback: fill, then capture.
fn callback(stream: &mut Stream, input: &[f32]) -> sonara_rt::Fill {
    let mut out = vec![0.0; 2 * PERIOD];
    let fill = assert_no_alloc(|| stream.callback.fill(&mut out));
    assert_no_alloc(|| stream.callback.capture(input, &fill));
    fill
}

fn read_all(stream: &mut Stream) -> (Vec<f32>, Option<sonara_rt::CaptureTake>) {
    let mut buffer = vec![0.0; 4096];
    let chunk = assert_no_alloc(|| stream.capture.read(&mut buffer));
    buffer.truncate(chunk.frames);
    (buffer, chunk.take)
}

/// The prototype stamped a take with the position stored at the end of the previous
/// callback, the old playhead after an earlier play. The start must be the first
/// current-generation frame of the same callback, and the input before it is dropped.
#[test]
fn capture_start_alignment() {
    // After an earlier play.
    let mut stream = duplex(48_000);
    push(&mut stream, 0, GEN1);
    push(&mut stream, 512, GEN1);
    callback(&mut stream, &input(0));
    callback(&mut stream, &input(0));
    assert_eq!(
        assert_no_alloc(|| stream.control.position()),
        Some(TimelineFrame::new(512))
    );

    assert_no_alloc(|| stream.control.flush(GEN2));
    push(&mut stream, 48_000, GEN2);
    let take = assert_no_alloc(|| stream.control.arm_capture());
    // The stale block fades out over 128 frames; the new generation starts at frame 128.
    let fill = callback(&mut stream, &input(1000));
    let first = fill.first_current().unwrap();
    assert_eq!(first.offset, 128);
    assert_eq!(first.timeline_frame, TimelineFrame::new(48_000));
    let (frames, started) = read_all(&mut stream);
    let started = started.unwrap();
    assert_eq!(started.id, take);
    assert_eq!(started.start, TimelineFrame::new(48_000));
    assert_ne!(started.start, TimelineFrame::new(512), "the old playhead");
    assert_eq!(frames, input(1000)[128..]);
    // Later callbacks capture every frame.
    callback(&mut stream, &input(2000));
    assert_eq!(read_all(&mut stream).0, input(2000));

    // A fresh stream: nothing plays in the first callback, so its input is dropped.
    let mut stream = duplex(48_000);
    let take = assert_no_alloc(|| stream.control.arm_capture());
    callback(&mut stream, &input(0));
    assert_eq!(read_all(&mut stream), (vec![], None));
    push(&mut stream, 1000, GEN1);
    callback(&mut stream, &input(300));
    let (frames, started) = read_all(&mut stream);
    assert_eq!(frames, input(300));
    assert_eq!(
        started.map(|t| (t.id, t.start)),
        Some((take, TimelineFrame::new(1000)))
    );
}

#[test]
fn capture_overrun_counted_once_per_callback() {
    let mut stream = duplex(300);
    push(&mut stream, 0, GEN1);
    push(&mut stream, 512, GEN1);
    let _ = assert_no_alloc(|| stream.control.arm_capture());
    callback(&mut stream, &input(0));
    assert_eq!(assert_no_alloc(|| stream.control.stats()).overruns, 0);
    // 44 frames fit; the rest is dropped, and the callback counts one overrun.
    callback(&mut stream, &input(256));
    assert_eq!(assert_no_alloc(|| stream.control.stats()).overruns, 1);
    callback(&mut stream, &input(512));
    assert_eq!(assert_no_alloc(|| stream.control.stats()).overruns, 2);

    let (frames, _) = read_all(&mut stream);
    let expected: Vec<f32> = (0..300_u16).map(f32::from).collect();
    assert_eq!(frames, expected);
}

#[test]
fn takes_are_split_exactly() {
    let mut stream = duplex(48_000);
    for i in 0..4 {
        push(&mut stream, i * 512, GEN1);
    }
    let first = assert_no_alloc(|| stream.control.arm_capture());
    callback(&mut stream, &input(0));
    assert_no_alloc(|| stream.control.disarm_capture());
    // Disarmed: nothing is captured.
    callback(&mut stream, &input(100));
    let second = assert_no_alloc(|| stream.control.arm_capture());
    assert_ne!(first, second);
    callback(&mut stream, &input(200));

    // The reader has not read anything yet; each read stays within one take.
    let (frames, take) = read_all(&mut stream);
    assert_eq!(frames, input(0));
    assert_eq!(
        take.map(|t| (t.id, t.start)),
        Some((first, TimelineFrame::new(0)))
    );
    let (frames, take) = read_all(&mut stream);
    assert_eq!(frames, input(200));
    assert_eq!(
        take.map(|t| (t.id, t.start)),
        Some((second, TimelineFrame::new(512)))
    );
    assert_eq!(read_all(&mut stream).0, vec![]);
}

#[test]
fn nothing_is_captured_unless_armed() {
    let mut stream = duplex(48_000);
    push(&mut stream, 0, GEN1);
    callback(&mut stream, &input(0));
    assert_eq!(read_all(&mut stream), (vec![], None));
}

#[test]
fn a_stream_without_input_never_captures() {
    let mut stream = Stream::new(&StreamConfig {
        output_channels: 2,
        input_channels: 0,
        block_frames: BLOCK_FRAMES,
        ring_blocks: 4,
        capture_frames: 0,
    })
    .unwrap();
    push(&mut stream, 0, GEN1);
    let _ = assert_no_alloc(|| stream.control.arm_capture());
    callback(&mut stream, &[]);
    assert_eq!(stream.capture.channels(), 0);
    assert_eq!(read_all(&mut stream), (vec![], None));
    assert_eq!(
        assert_no_alloc(|| stream.control.stats())
            .input_peak
            .as_slice(),
        []
    );
}
