//! Real-time: runs on the MIDI input thread (see docs/audio-engine.md).
//!
//! The midir input callback. midir calls [`receive`] for every message of
//! an open input, on CoreMIDI's thread (macOS) or on its own ALSA thread (Linux). It turns the
//! message into a fixed-size [`MidiEvent`] and pushes it into the input's ring, which the render
//! thread drains. It never allocates, locks or blocks (RT-02, RT-03):
//!
//! - a message of at most three bytes is pushed as it is;
//! - a system exclusive message (status `0xF0`, or longer than three bytes) is replaced by a
//!   [`MidiEvent::sysex`] marker, so the receiver still counts it (`messages_received`);
//! - an empty message is ignored (midir never delivers one);
//! - an event that does not fit is dropped and counted by the ring (RT-08).
//!
//! midir's own handler around this callback is outside Sonara's real-time modules: on CoreMIDI
//! it locks an uncontended mutex around the call and collects segmented system exclusive
//! messages in a `Vec<u8>`; on ALSA it collects every message in a `Vec<u8>`.
//! RealtimeSanitizer's `nonblocking` covers [`receive`] only.
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

use sonara_rt::{MidiEvent, MidiSender};

/// The status byte that starts a system exclusive message.
const SYSEX_START: u8 = 0xF0;

/// The state of one input's callback: the ring's sending end and the input's port index.
pub(crate) struct InputCallback {
    events: MidiSender,
    port: u8,
}

impl InputCallback {
    /// Setup: runs when the input is opened, off the real-time thread.
    pub(crate) fn new(events: MidiSender, port: u8) -> Self {
        Self { events, port }
    }
}

/// One message from midir: `time_us` is midir's timestamp in microseconds, `bytes` one whole
/// message (midir, like RtMidi, never delivers running status).
#[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
pub(crate) fn receive(time_us: u64, bytes: &[u8], callback: &mut InputCallback) {
    let event = match bytes.first() {
        None => return,
        Some(&SYSEX_START) => MidiEvent::sysex(time_us, callback.port),
        Some(_) => MidiEvent::new(time_us, callback.port, bytes)
            .unwrap_or_else(|| MidiEvent::sysex(time_us, callback.port)),
    };
    // A full ring drops the event and counts it (`MidiOverflows`).
    callback.events.push(event);
}
