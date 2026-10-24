//! MIDI events: the prototype's message decoding (`MidiMessage` in `src/sonara/midi/devices.py`,
//! `tests/engine/test_midi_live.py::test_message_decoding`) and the ring of one MIDI input
//! (TST-01, TST-03, TST-10).
#![cfg(not(loom))]

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use sonara_rt::{MidiEvent, MidiKind, midi_channel};

/// RT-02: the calls of the real-time threads (the MIDI input callback's push, the render
/// thread's pop and decoding) run inside `assert_no_alloc`, which aborts on any allocation or
/// free.
#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

fn message(bytes: &[u8]) -> MidiEvent {
    assert_no_alloc(|| MidiEvent::new(0, 0, bytes)).expect("a message of one to three bytes")
}

/// Bytes, then the expected channel, kind, note, velocity and value.
type Case = (&'static [u8], u8, MidiKind, u8, u8, i32);

/// The prototype's `test_message_decoding`, case by case.
#[test]
fn midi_message_decoding() {
    let on = message(&[0x91, 60, 100]);
    assert_eq!(on.kind(), MidiKind::NoteOn);
    assert_eq!(on.channel(), 1);
    assert_eq!(on.note(), 60);
    assert_eq!(on.velocity(), 100);
    // Velocity 0 is a note off.
    assert_eq!(message(&[0x90, 60, 0]).kind(), MidiKind::NoteOff);
    // Pitch bend: the centre is 0, the top 8191.
    assert_eq!(message(&[0xE0, 0x00, 0x40]).value(), 0);
    assert_eq!(message(&[0xE0, 0x7F, 0x7F]).value(), 8191);
    assert_eq!(message(&[0xE0, 0x00, 0x00]).value(), -8192);
    assert_eq!(message(&[0xB0, 64, 127]).kind(), MidiKind::Cc);
}

/// Every kind and value the prototype's accessors give, values computed by hand from
/// `MidiMessage` (status, channel, kind, note, velocity, value).
#[test]
fn every_kind_decodes_as_in_the_prototype() {
    let cases: &[Case] = &[
        (&[0x8F, 64, 30], 15, MidiKind::NoteOff, 64, 30, 30),
        (&[0x93, 64, 1], 3, MidiKind::NoteOn, 64, 1, 1),
        (&[0xA2, 60, 77], 2, MidiKind::PolyPressure, 60, 77, 77),
        // A control change: `note` is the controller, `value` its value.
        (&[0xB5, 7, 100], 5, MidiKind::Cc, 7, 100, 100),
        (&[0xC4, 12], 4, MidiKind::Program, 12, 0, 0),
        // Channel pressure: the value is the second byte.
        (&[0xD6, 90], 6, MidiKind::ChannelPressure, 90, 0, 90),
        (&[0xE7, 0x01, 0x40], 7, MidiKind::PitchBend, 1, 0x40, 1),
        // System messages are "other"; their channel is the status's low nibble.
        (&[0xF8], 8, MidiKind::Other, 0, 0, 0),
        (&[0xF2, 3, 4], 2, MidiKind::Other, 3, 4, 4),
        (&[0xFF], 15, MidiKind::Other, 0, 0, 0),
    ];
    for &(bytes, channel, kind, note, velocity, value) in cases {
        let event = message(bytes);
        assert_eq!(event.status(), bytes[0], "{bytes:02X?}");
        assert_eq!(event.channel(), channel, "{bytes:02X?}");
        assert_eq!(event.kind(), kind, "{bytes:02X?}");
        assert_eq!(event.note(), note, "{bytes:02X?}");
        assert_eq!(event.velocity(), velocity, "{bytes:02X?}");
        assert_eq!(event.value(), value, "{bytes:02X?}");
    }
}

/// Short and malformed messages: the prototype's length guards, value by value.
#[test]
fn short_messages_decode_as_in_the_prototype() {
    // A note on without its velocity fails the `len >= 3` guard and is not in the table.
    let short_on = message(&[0x90, 60]);
    assert_eq!(short_on.kind(), MidiKind::Other);
    assert_eq!((short_on.note(), short_on.velocity()), (60, 0));
    // A short note off is still a note off.
    assert_eq!(message(&[0x80, 60]).kind(), MidiKind::NoteOff);
    assert_eq!(message(&[0x80]).kind(), MidiKind::NoteOff);
    // A pitch bend without its high byte has the value 0 (the fallback reads byte 2).
    let short_bend = message(&[0xE0, 0x7F]);
    assert_eq!(short_bend.kind(), MidiKind::PitchBend);
    assert_eq!(short_bend.value(), 0);
    // Channel pressure without its byte.
    assert_eq!(message(&[0xD0]).value(), 0);
    // A control change without its value.
    assert_eq!(message(&[0xB0, 64]).value(), 0);
    // Running status (data bytes without a status byte) never reaches the ring: midir, like
    // RtMidi, delivers whole messages. Bytes without a status byte decode as "other".
    let running = message(&[0x3C, 100]);
    assert_eq!(running.kind(), MidiKind::Other);
    assert_eq!(running.status(), 0x3C);
    assert_eq!(running.channel(), 0x0C);
}

#[test]
fn a_sysex_marker_keeps_time_and_port_but_no_bytes() {
    let marker = MidiEvent::sysex(1_500, 3);
    assert!(marker.is_sysex());
    assert_eq!(marker.bytes(), [] as [u8; 0]);
    assert_eq!((marker.time_us(), marker.port()), (1_500, 3));
    assert_eq!(marker.kind(), MidiKind::Other);
    assert_eq!(
        (marker.status(), marker.channel(), marker.value()),
        (0, 0, 0)
    );
    assert!(!message(&[0xF0, 0x7E, 0xF7]).is_sysex());
}

#[test]
fn kinds_have_the_prototype_names() {
    let names: Vec<&str> = [
        MidiKind::NoteOn,
        MidiKind::NoteOff,
        MidiKind::Cc,
        MidiKind::PitchBend,
        MidiKind::ChannelPressure,
        MidiKind::PolyPressure,
        MidiKind::Program,
        MidiKind::Other,
    ]
    .iter()
    .map(|kind| kind.as_str())
    .collect();
    assert_eq!(
        names,
        [
            "note_on",
            "note_off",
            "cc",
            "pitch_bend",
            "channel_pressure",
            "poly_pressure",
            "program",
            "other"
        ]
    );
}

#[test]
fn the_midi_ring_delivers_in_order_and_counts_what_does_not_fit() {
    let (mut sender, mut receiver) = midi_channel(2);
    let overflows = sender.overflows();
    assert!(assert_no_alloc(|| receiver.is_empty()));
    assert!(assert_no_alloc(|| sender.push(message(&[0x90, 60, 100]))));
    assert!(assert_no_alloc(|| sender.push(MidiEvent::sysex(5, 0))));
    // Full: dropped and counted.
    assert!(!assert_no_alloc(|| sender.push(message(&[0x80, 60, 0]))));
    assert!(!assert_no_alloc(|| sender.push(message(&[0x80, 61, 0]))));
    assert_eq!(overflows.get(), 2);
    assert_eq!(assert_no_alloc(|| receiver.len()), 2);
    assert_eq!(
        assert_no_alloc(|| receiver.pop()).map(|e| e.kind()),
        Some(MidiKind::NoteOn)
    );
    assert!(assert_no_alloc(|| receiver.pop()).unwrap().is_sysex());
    assert_eq!(assert_no_alloc(|| receiver.pop()), None);
    // Room again.
    assert!(assert_no_alloc(|| sender.push(message(&[0x80, 60, 0]))));
    assert_eq!(
        assert_no_alloc(|| receiver.pop()).map(|e| e.kind()),
        Some(MidiKind::NoteOff)
    );
    assert_eq!(overflows.get(), 2);
}
