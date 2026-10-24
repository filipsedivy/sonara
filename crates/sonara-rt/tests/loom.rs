//! Loom model checks of the protocols around the rings (RT-04): the generation (flush), the
//! playing flag and its per-generation arming, the read-position snapshot and ledger behind `queued_frames`, the capture
//! request and take start, the peaks, and the graph hand-off (order, the published count of
//! received values, and no drop on the real-time thread). Run with `scripts/dev.sh rust-loom`
//! (`--cfg loom`, release).
//!
//! Loom explores every interleaving of Sonara's atomics, including stale reads their orderings
//! allow. `rtrb`'s own atomics are not instrumented, so each ring operation runs as one
//! sequentially consistent step; the assertions hold without relying on `rtrb`'s ordering.
#![cfg(loom)]

use std::cell::Cell;

use loom::thread;
use sonara_rt::handoff;
use sonara_rt::{Generation, Stream, StreamConfig, TimelineFrame};

const GEN1: Generation = Generation::new(1);
const GEN2: Generation = Generation::new(2);
const BLOCK: usize = 16;

fn check(model: impl Fn() + Sync + Send + 'static) {
    let mut builder = loom::model::Builder::new();
    builder.preemption_bound = Some(3);
    builder.check(model);
}

/// Mono output, optional mono input, a ring of two 16-frame blocks.
fn mono(input_channels: usize) -> Stream {
    Stream::new(&StreamConfig {
        output_channels: 1,
        input_channels,
        block_frames: BLOCK,
        ring_blocks: 2,
        capture_frames: 64,
    })
    .expect("a valid configuration")
}

fn fill(stream_callback: &mut sonara_rt::DeviceCallback, frames: usize) -> Vec<f32> {
    let mut out = vec![f32::NAN; frames];
    stream_callback.fill(&mut out);
    out
}

/// After `flush(2)`, generation 1 (negative samples) never plays again once generation 2
/// (positive samples) has started, and the generation-2 block is played in full.
#[test]
fn flush_races_write_and_consume() {
    check(|| {
        let Stream {
            mut producer,
            mut callback,
            control,
            ..
        } = mono(0);
        producer
            .push_block(&[-1.0; BLOCK], TimelineFrame::new(0), GEN1)
            .expect("a free slot");
        let mut played = fill(&mut callback, 8);

        let render = thread::spawn(move || {
            control.flush(GEN2);
            producer
                .push_block(&[1.0; BLOCK], TimelineFrame::new(1000), GEN2)
                .expect("a free slot");
            let queued = producer.queued_frames();
            assert!(queued <= 16, "only generation 2 counts: {queued}");
            producer
        });
        played.extend(fill(&mut callback, 8));
        played.extend(fill(&mut callback, 8));
        let producer = render.join().expect("the render thread");
        played.extend(fill(&mut callback, 32));

        assert!(played.iter().all(|v| v.is_finite()));
        let first_new = played.iter().position(|v| *v > 0.0);
        let first_new = first_new.expect("generation 2 plays");
        assert!(played[first_new..].iter().all(|v| *v >= 0.0));
        assert_eq!(played.iter().filter(|v| **v > 0.0).count(), BLOCK);
        assert_eq!(producer.queued_frames(), 0);
    });
}

/// The ledger behind `queued_frames` is published with `written`: a reader on another thread
/// sees whole blocks, never a block counted with a stale ledger entry.
#[test]
fn queued_frames_see_whole_blocks() {
    check(|| {
        let Stream {
            mut producer,
            control,
            ..
        } = mono(0);
        let render = thread::spawn(move || {
            producer
                .push_block(&[0.5; 5], TimelineFrame::new(0), GEN1)
                .expect("a free slot");
            producer
                .push_block(&[0.5; 7], TimelineFrame::new(5), GEN1)
                .expect("a free slot");
        });
        let queued = control.queued_frames();
        assert!([0, 5, 12].contains(&queued), "{queued}");
        render.join().expect("the render thread");
        assert_eq!(control.queued_frames(), 12);
    });
}

/// The read-position snapshot: the producer's own query never counts more than it pushed or
/// less than the callback can have played, and is exact once the threads have joined.
#[test]
fn queued_frames_follow_the_read_position() {
    check(|| {
        let Stream {
            mut producer,
            mut callback,
            ..
        } = mono(0);
        let render = thread::spawn(move || {
            producer
                .push_block(&[0.5; 5], TimelineFrame::new(0), GEN1)
                .expect("a free slot");
            let first = producer.queued_frames();
            producer
                .push_block(&[0.5; 7], TimelineFrame::new(5), GEN1)
                .expect("a free slot");
            let second = producer.queued_frames();
            assert!(first <= 5, "{first}");
            assert!((4..=12).contains(&second), "{second}");
            producer
        });
        let mut played = fill(&mut callback, 4);
        played.extend(fill(&mut callback, 4));
        let producer = render.join().expect("the render thread");
        let played =
            u64::try_from(played.iter().filter(|v| **v > 0.0).count()).expect("a short count");
        assert_eq!(producer.queued_frames(), 12 - played);
    });
}

/// The playing flag publishes the block written before it: a callback that sees it set never
/// counts the generation's first block as an underrun.
#[test]
fn playing_flag_follows_the_first_block() {
    check(|| {
        let Stream {
            mut producer,
            mut callback,
            control,
            ..
        } = mono(0);
        let render = thread::spawn(move || {
            producer
                .push_block(&[0.5; BLOCK], TimelineFrame::new(0), GEN1)
                .expect("a free slot");
            control.set_playing(true);
            control
        });
        let first = callback.fill(&mut [0.0; BLOCK]);
        let second = callback.fill(&mut [0.0; BLOCK]);
        let control = render.join().expect("the render thread");
        assert!(!first.is_starved());
        assert!(!second.is_starved() || first.first_current().is_some());
        let underruns = u64::from(second.is_starved());
        assert_eq!(control.stats().underruns, underruns);
    });
}

/// The render thread arms the flag for the generation it has just written while the
/// control thread flushes to the next one (a seek or a restart). In no interleaving does the
/// callback count an underrun: it sees either the armed generation with its block, or a flushed
/// ring with a stale flag.
#[test]
fn a_flush_disarms_the_playing_flag() {
    check(|| {
        let Stream {
            mut producer,
            mut callback,
            control,
            ..
        } = mono(0);
        let render = thread::spawn(move || {
            producer
                .push_block(&[0.5; BLOCK], TimelineFrame::new(0), GEN1)
                .expect("a free slot");
            producer.set_playing_generation(Some(GEN1));
            producer
        });
        let seek = {
            let control = control.clone();
            thread::spawn(move || control.flush(GEN2))
        };
        let first = callback.fill(&mut [0.0; BLOCK]);
        seek.join().expect("the control thread");
        let second = callback.fill(&mut [0.0; BLOCK]);
        let _producer = render.join().expect("the render thread");
        let third = callback.fill(&mut [0.0; BLOCK]);
        assert!(!first.is_starved() && !second.is_starved() && !third.is_starved());
        assert_eq!(control.stats().underruns, 0);
    });
}

/// Captures with a take armed concurrently, `callbacks` callbacks of 8 frames that each play
/// the current generation from their first frame (tags from 100), with input samples numbered
/// by their timeline frame minus 100. Returns every chunk read after the threads joined.
fn capture_while(
    arm: impl FnOnce(&sonara_rt::StreamControl) + Send + 'static,
    callbacks: u16,
) -> Vec<(sonara_rt::CaptureTake, Vec<f32>)> {
    let Stream {
        mut producer,
        mut callback,
        mut capture,
        control,
    } = mono(1);
    producer
        .push_block(&[0.5; BLOCK], TimelineFrame::new(100), GEN1)
        .expect("a free slot");
    producer
        .push_block(&[0.5; BLOCK], TimelineFrame::new(116), GEN1)
        .expect("a free slot");
    let arming = control.clone();
    let armer = thread::spawn(move || arm(&arming));
    for k in 0..callbacks {
        let input: Vec<f32> = (0..8_u16).map(|i| f32::from(k * 8 + i)).collect();
        let fill = callback.fill(&mut [0.0; 8]);
        callback.capture(&input, &fill);
    }
    armer.join().expect("the control thread");
    let mut chunks = Vec::new();
    for _ in 0..8 {
        let mut buffer = [0.0; 64];
        let chunk = capture.read(&mut buffer);
        if chunk.frames > 0 {
            let take = chunk.take.expect("captured frames belong to a take");
            chunks.push((take, buffer[..chunk.frames].to_vec()));
        }
    }
    chunks
}

/// Every captured frame of a take is the input played at the take's start plus its index.
fn assert_aligned(chunks: &[(sonara_rt::CaptureTake, Vec<f32>)]) {
    let mut index = 0;
    let mut current = None;
    for (take, frames) in chunks {
        if current != Some(take.id) {
            current = Some(take.id);
            index = 0;
        }
        for value in frames {
            let expected = u16::try_from(take.start.get() - 100 + index).expect("a short run");
            assert_eq!(
                value.to_bits(),
                f32::from(expected).to_bits(),
                "take {take:?}"
            );
            index += 1;
        }
    }
}

/// A take armed concurrently starts at the first current frame of the callback that
/// starts it, and captures every later frame.
#[test]
fn capture_start_comes_from_the_same_callback() {
    check(|| {
        let chunks = capture_while(
            |control| {
                let _ = control.arm_capture();
            },
            2,
        );
        assert_aligned(&chunks);
        if let Some((take, _)) = chunks.first() {
            assert!([100, 108].contains(&take.start.get()), "{take:?}");
            let frames: usize = chunks.iter().map(|(_, f)| f.len()).sum();
            assert_eq!(
                frames,
                usize::try_from(116 - take.start.get()).expect("a count")
            );
        }
    });
}

/// Disarming and re-arming while the callback captures: each take holds only its own frames,
/// in order, aligned with its start.
#[test]
fn rearming_splits_takes_exactly() {
    check(|| {
        let chunks = capture_while(
            |control| {
                let first = control.arm_capture();
                control.disarm_capture();
                let second = control.arm_capture();
                assert_ne!(first, second);
            },
            3,
        );
        assert_aligned(&chunks);
        let mut takes: Vec<u64> = chunks.iter().map(|(take, _)| take.id.get()).collect();
        takes.dedup();
        assert!(takes.windows(2).all(|pair| pair[0] < pair[1]), "{takes:?}");
    });
}

/// The take writer reads while the callback captures and the control thread re-arms: a read
/// never mixes two takes, and every frame stays aligned with its take's start.
#[test]
fn the_reader_never_mixes_takes() {
    check(|| {
        let Stream {
            mut producer,
            mut callback,
            mut capture,
            control,
        } = mono(1);
        producer
            .push_block(&[0.5; BLOCK], TimelineFrame::new(100), GEN1)
            .expect("a free slot");
        producer
            .push_block(&[0.5; BLOCK], TimelineFrame::new(116), GEN1)
            .expect("a free slot");
        let first = control.arm_capture();
        let rearm = thread::spawn(move || {
            control.disarm_capture();
            let _ = control.arm_capture();
        });
        let writer = thread::spawn(move || {
            let mut chunks = Vec::new();
            for _ in 0..2 {
                let mut buffer = [0.0; 64];
                let chunk = capture.read(&mut buffer);
                if chunk.frames > 0 {
                    chunks.push((chunk.take.expect("a take"), buffer[..chunk.frames].to_vec()));
                }
                thread::yield_now();
            }
            (capture, chunks)
        });
        for k in 0..3_u16 {
            let input: Vec<f32> = (0..8_u16).map(|i| f32::from(k * 8 + i)).collect();
            let fill = callback.fill(&mut [0.0; 8]);
            callback.capture(&input, &fill);
        }
        rearm.join().expect("the control thread");
        let (mut capture, mut chunks) = writer.join().expect("the take writer");
        loop {
            let mut buffer = [0.0; 64];
            let chunk = capture.read(&mut buffer);
            if chunk.frames == 0 {
                break;
            }
            chunks.push((chunk.take.expect("a take"), buffer[..chunk.frames].to_vec()));
        }
        assert_aligned(&chunks);
        // Takes come in order; the first may never start when the re-arm wins the race.
        let takes: Vec<u64> = chunks.iter().map(|(take, _)| take.id.get()).collect();
        assert!(takes.windows(2).all(|pair| pair[0] <= pair[1]), "{takes:?}");
        assert!(takes.iter().all(|id| *id >= first.get()));
    });
}

/// Peaks merged concurrently with `stats()` reads are reported exactly once, never lost.
#[test]
fn peaks_are_never_lost() {
    check(|| {
        let Stream {
            callback, control, ..
        } = mono(0);
        let meter = thread::spawn(move || {
            callback.meter_output(&[0.5]);
            callback.meter_output(&[-0.25]);
        });
        let first = control.stats().peak.as_slice()[0];
        let second = control.stats().peak.as_slice()[0];
        meter.join().expect("the callback thread");
        let last = control.stats().peak.as_slice()[0];
        let readings = [first, second, last];
        assert_eq!(
            readings.iter().filter(|p| **p == 0.5).count(),
            1,
            "{readings:?}"
        );
        assert!(readings.iter().all(|p| [0.0, 0.25, 0.5].contains(p)));
    });
}

loom::thread_local! {
    /// Set on the thread that plays the real-time receiver of a hand-off.
    static ON_RECEIVER: Cell<bool> = Cell::new(false);
}

/// A handed-off value (a render graph in the engine) whose drop asserts that it does not run on
/// the real-time receiver (RT-09).
#[derive(Debug)]
struct Probe(u32);

impl Drop for Probe {
    fn drop(&mut self) {
        ON_RECEIVER.with(|on| assert!(!on.get(), "value {} dropped on the receiver", self.0));
    }
}

fn send(tx: &mut handoff::Sender<Probe>, id: u32) {
    assert!(tx.send(Probe(id)).is_ok(), "room for value {id}");
}

/// Values sent while the receiver swaps them in come back in order and are never dropped on
/// the receiver, and the published count never runs ahead of the returned values.
#[test]
fn handoff_returns_values_in_order_without_dropping_them_on_the_receiver() {
    check(|| {
        let (mut tx, mut rx) = handoff::channel::<Probe>(2);
        send(&mut tx, 1);
        let receiver = thread::spawn(move || {
            ON_RECEIVER.with(|on| on.set(true));
            let mut current = Probe(0);
            let mut swapped = Vec::new();
            for _ in 0..2 {
                if rx.swap(&mut current) {
                    swapped.push(current.0);
                }
            }
            (rx, current, swapped)
        });
        send(&mut tx, 2);
        let published = tx.received();
        let mut returned = Vec::new();
        while let Some(value) = tx.take_returned() {
            returned.push(value.0);
        }
        assert!(
            u64::try_from(returned.len()).expect("a short list") >= published,
            "{published} published, {returned:?} returned"
        );
        let (rx, current, swapped) = receiver.join().expect("the receiver");
        assert!(swapped == [1, 2] || swapped == [1], "{swapped:?}");
        while let Some(value) = tx.take_returned() {
            returned.push(value.0);
        }
        let expected: Vec<u32> = (0..u32::try_from(swapped.len()).expect("two")).collect();
        assert_eq!(returned, expected, "returned in the order received");
        assert_eq!(current.0, *swapped.last().expect("one swap at least"));
        assert_eq!(
            tx.received(),
            u64::try_from(swapped.len()).expect("two"),
            "the count of received values"
        );
        drop((rx, current));
    });
}

/// Once the control thread sees the count of received values, the returned value is there to
/// take: the count is published after the return (Release, Acquire).
#[test]
fn the_received_count_publishes_the_returned_value() {
    check(|| {
        let (mut tx, mut rx) = handoff::channel::<Probe>(1);
        send(&mut tx, 1);
        let receiver = thread::spawn(move || {
            ON_RECEIVER.with(|on| on.set(true));
            let mut current = Probe(0);
            let swapped = rx.swap(&mut current);
            (rx, current, swapped)
        });
        if tx.received() == 1 {
            let back = tx
                .take_returned()
                .expect("the received value has come back");
            assert_eq!(back.0, 0);
        }
        let (rx, current, swapped) = receiver.join().expect("the receiver");
        assert!(swapped);
        assert_eq!((current.0, tx.received()), (1, 1));
        drop((rx, current));
    });
}

/// A full return ring makes the receiver keep its value and leave the sent one waiting, while
/// the control thread takes a returned value concurrently; nothing is dropped on the receiver
/// and nothing is lost.
#[test]
fn a_full_return_ring_keeps_every_value() {
    check(|| {
        let (mut tx, mut rx) = handoff::channel::<Probe>(1);
        let mut current = Probe(0);
        send(&mut tx, 1);
        assert!(rx.swap(&mut current));
        send(&mut tx, 2);
        assert!(rx.swap(&mut current));
        // The return ring (capacity 2) now holds 0 and 1.
        send(&mut tx, 3);
        let receiver = thread::spawn(move || {
            ON_RECEIVER.with(|on| on.set(true));
            let swapped = rx.swap(&mut current);
            (rx, current, swapped)
        });
        let first = tx.take_returned().expect("value 0 is back");
        assert_eq!(first.0, 0);
        let (mut rx, current, swapped) = receiver.join().expect("the receiver");
        let mut ids: Vec<u32> = std::iter::from_fn(|| tx.take_returned())
            .map(|v| v.0)
            .collect();
        if swapped {
            assert_eq!((current.0, ids.as_slice()), (3, [1, 2].as_slice()));
        } else {
            assert_eq!((current.0, ids.as_slice()), (2, [1].as_slice()));
            let mut current = current;
            assert!(rx.swap(&mut current), "room again");
            assert_eq!(current.0, 3);
            ids.extend(std::iter::from_fn(|| tx.take_returned()).map(|v| v.0));
            assert_eq!(ids, [1, 2]);
        }
        assert_eq!(tx.received(), 3);
    });
}
