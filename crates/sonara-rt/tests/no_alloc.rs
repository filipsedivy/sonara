//! RT-02: the real-time paths neither allocate nor free. `assert_no_alloc`'s allocator aborts
//! the test on any allocation inside an `assert_no_alloc` block (debug builds).
#![cfg(not(loom))]

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use sonara_rt::midi_channel;
use sonara_rt::{
    BLOCK_FRAMES, Generation, MidiEvent, MidiKind, Stream, StreamConfig, TimelineFrame,
};
use sonara_rt::{handoff, queue};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

#[test]
fn the_callback_and_the_producer_do_not_allocate() {
    let mut stream = Stream::new(&StreamConfig {
        output_channels: 2,
        input_channels: 2,
        block_frames: BLOCK_FRAMES,
        ring_blocks: 4,
        capture_frames: 4096,
    })
    .unwrap();
    let block = vec![0.5_f32; 2 * BLOCK_FRAMES];
    let input = vec![0.25_f32; 2 * 256];
    let mut out = vec![0.0_f32; 2 * 256];
    let mut read = vec![0.0_f32; 2 * 1024];
    stream.control.set_playing(true);
    let _ = stream.control.arm_capture();

    let mut tag = 0;
    let mut generation = Generation::new(1);
    let mut captured = 0;
    assert_no_alloc(|| {
        for callback in 0..64 {
            // The render thread.
            while stream.producer.free_blocks() > 0 && stream.producer.queued_frames() < 1024 {
                let pushed =
                    stream
                        .producer
                        .push_block(&block, TimelineFrame::new(tag), generation);
                assert!(pushed.is_ok());
                tag += 512;
            }
            if callback % 16 == 15 {
                generation = generation.next();
                stream.control.flush(generation);
            }
            // The device callback, in the prototype's order.
            let fill = stream.callback.fill(&mut out);
            let _ = stream.callback.monitor_gain();
            stream.callback.meter_input(&input);
            stream.callback.capture(&input, &fill);
            stream.callback.meter_output(&out);
            stream.callback.end_callback(256, 0.01);
            // The take writer.
            captured += stream.capture.read(&mut read).frames;
        }
    });
    assert!(captured > 0);
    assert_eq!(stream.control.stats().callbacks, 64);
}

#[test]
fn queues_and_the_handoff_do_not_allocate() {
    let (mut tx, mut rx) = queue::channel::<MidiEvent>(8);
    let (mut graphs, mut render) = handoff::channel::<Vec<f32>>(2);
    graphs.send(vec![1.0; 16]).unwrap();
    let mut current = vec![0.0_f32; 16];
    let swapped = assert_no_alloc(|| {
        for time in 0..20 {
            if let Some(event) = MidiEvent::new(time, 0, &[0x90, 60, 100]) {
                let _ = tx.push(event);
            }
            let _ = rx.pop();
        }
        render.swap(&mut current)
    });
    assert!(swapped);
    assert_eq!(current, vec![1.0; 16]);
    assert_eq!(graphs.take_returned(), Some(vec![0.0; 16]));
}

#[test]
fn the_midi_ring_and_the_decoding_do_not_allocate() {
    let (mut sender, mut receiver) = midi_channel(4);
    let overflows = sender.overflows();
    let (pushed, notes) = assert_no_alloc(|| {
        let mut pushed = 0_usize;
        let mut notes = 0_usize;
        for time in 0..20 {
            let event = MidiEvent::new(time, 1, &[0x90, 60, 100])
                .unwrap_or_else(|| MidiEvent::sysex(time, 1));
            // The ring fills up: the overflow is counted, not allocated.
            if sender.push(event) {
                pushed += 1;
            }
            if time % 6 == 0 {
                while let Some(event) = receiver.pop() {
                    if event.kind() == MidiKind::NoteOn && event.value() == 100 {
                        notes += 1;
                    }
                }
            }
        }
        (pushed, notes)
    });
    assert_eq!(u64::try_from(pushed).unwrap() + overflows.get(), 20);
    assert!(overflows.get() > 0);
    assert_eq!(notes + receiver.len(), pushed);
}
