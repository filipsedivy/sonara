//! The midir input callback driven without a MIDI device (TST-01, TST-03, TST-10): what reaches
//! the ring, what is replaced by a system exclusive marker, what is ignored, and that nothing
//! allocates (RT-02; `assert_no_alloc` is this test binary's allocator, see `callback_tests`).

use assert_no_alloc::assert_no_alloc;
use sonara_rt::{MidiEvent, MidiKind, MidiReceiver, midi_channel};

use crate::midi::{MidiInputPort, MidiOutputPort};
use crate::midi_callback::{InputCallback, receive};

fn drain(receiver: &mut MidiReceiver) -> Vec<MidiEvent> {
    std::iter::from_fn(|| receiver.pop()).collect()
}

#[test]
fn messages_reach_the_ring_with_their_time_and_port() {
    let (sender, mut receiver) = midi_channel(16);
    let mut callback = InputCallback::new(sender, 3);
    assert_no_alloc(|| {
        receive(1_000, &[0x90, 64, 90], &mut callback);
        receive(1_250, &[0x80, 64, 0], &mut callback);
        receive(1_300, &[0xD0, 40], &mut callback);
        // A real-time message that RtMidi's filter lets through (start).
        receive(1_400, &[0xFA], &mut callback);
    });
    let events = drain(&mut receiver);
    let summary: Vec<(u64, u8, &[u8], MidiKind)> = events
        .iter()
        .map(|e| (e.time_us(), e.port(), e.bytes(), e.kind()))
        .collect();
    assert_eq!(
        summary,
        [
            (1_000, 3, &[0x90, 64, 90][..], MidiKind::NoteOn),
            (1_250, 3, &[0x80, 64, 0][..], MidiKind::NoteOff),
            (1_300, 3, &[0xD0, 40][..], MidiKind::ChannelPressure),
            (1_400, 3, &[0xFA][..], MidiKind::Other),
        ]
    );
}

#[test]
fn system_exclusive_is_dropped_but_counted_as_a_message() {
    let (sender, mut receiver) = midi_channel(16);
    let mut callback = InputCallback::new(sender, 1);
    assert_no_alloc(|| {
        receive(10, &[0xF0, 0x43, 0x10, 0x4C, 0x00, 0xF7], &mut callback);
        // A system exclusive message short enough to fit is a marker too.
        receive(20, &[0xF0, 0x7E, 0xF7], &mut callback);
        // Longer than three bytes without a 0xF0 status: cannot be stored either.
        receive(30, &[0x90, 60, 100, 0x40], &mut callback);
    });
    let events = drain(&mut receiver);
    assert_eq!(events.len(), 3);
    for (event, time) in events.iter().zip([10, 20, 30]) {
        assert!(event.is_sysex());
        assert_eq!((event.time_us(), event.port()), (time, 1));
        assert_eq!(event.kind(), MidiKind::Other);
    }
}

#[test]
fn an_empty_message_is_ignored() {
    let (sender, mut receiver) = midi_channel(4);
    let mut callback = InputCallback::new(sender, 0);
    assert_no_alloc(|| receive(5, &[], &mut callback));
    assert!(receiver.pop().is_none());
}

#[test]
fn a_full_ring_drops_and_counts_without_allocating() {
    let (sender, mut receiver) = midi_channel(2);
    let overflows = sender.overflows();
    let mut callback = InputCallback::new(sender, 0);
    assert_no_alloc(|| {
        for time in 0..5 {
            receive(time, &[0xB0, 1, 64], &mut callback);
        }
    });
    assert_eq!(overflows.get(), 3);
    let times: Vec<u64> = drain(&mut receiver)
        .iter()
        .map(MidiEvent::time_us)
        .collect();
    assert_eq!(times, [0, 1]);
}

/// The hub keeps open ports behind a mutex shared across threads, so they must be `Send`.
#[test]
fn open_ports_can_move_between_threads() {
    fn assert_send<T: Send>() {}
    assert_send::<MidiInputPort>();
    assert_send::<MidiOutputPort>();
}
