//! Raw MIDI messages in a fixed-size, `Copy` form, their decoding, and the ring of one MIDI
//! input.
//!
//! Each open MIDI input has its own ring ([`midi_channel`]): its input callback (midir's
//! CoreMIDI or ALSA thread) is the only producer, and the render thread is the only consumer
//! (RT-04). The decoding mirrors the prototype's `MidiMessage` (`src/sonara/midi/devices.py`),
//! so the render thread can turn events into instrument events without allocating.

use std::fmt;

use crate::queue::{self, Full};
use crate::sync::{Arc, AtomicU64, Ordering};

/// What a MIDI message is: the prototype's `MessageKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MidiKind {
    /// Note on with a velocity above 0.
    NoteOn,
    /// Note off, or note on with velocity 0.
    NoteOff,
    /// Control change.
    Cc,
    /// Pitch bend.
    PitchBend,
    /// Channel pressure (aftertouch).
    ChannelPressure,
    /// Polyphonic key pressure.
    PolyPressure,
    /// Program change.
    Program,
    /// Anything else: system messages, a dropped system exclusive message, a message without a
    /// status byte, a note on without its velocity.
    Other,
}

impl MidiKind {
    /// The prototype's name of the kind, for example `"note_on"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoteOn => "note_on",
            Self::NoteOff => "note_off",
            Self::Cc => "cc",
            Self::PitchBend => "pitch_bend",
            Self::ChannelPressure => "channel_pressure",
            Self::PolyPressure => "poly_pressure",
            Self::Program => "program",
            Self::Other => "other",
        }
    }
}

/// One channel or system message of at most three bytes, with its arrival time.
///
/// A system exclusive message does not fit. The MIDI input callback replaces it with a
/// [`sysex`](Self::sysex) marker, which keeps its time and port but not its bytes, so that the
/// receiver still counts it (the prototype's `messages_received` counts every message).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MidiEvent {
    time_us: u64,
    port: u8,
    /// 0 for a [`sysex`](Self::sysex) marker; `new` never stores an empty message.
    len: u8,
    data: [u8; Self::MAX_LEN],
}

impl MidiEvent {
    /// The longest message an event holds, in bytes.
    pub const MAX_LEN: usize = 3;

    /// The message `bytes` from input port `port`, received at `time_us` microseconds (midir's
    /// timestamp). `None` when `bytes` is empty or longer than [`MAX_LEN`](Self::MAX_LEN).
    #[must_use]
    pub fn new(time_us: u64, port: u8, bytes: &[u8]) -> Option<Self> {
        let len = u8::try_from(bytes.len()).ok()?;
        if bytes.is_empty() || bytes.len() > Self::MAX_LEN {
            return None;
        }
        let mut data = [0; Self::MAX_LEN];
        for (dst, src) in data.iter_mut().zip(bytes) {
            *dst = *src;
        }
        Some(Self {
            time_us,
            port,
            len,
            data,
        })
    }

    /// The marker of a system exclusive message from input port `port`, received at `time_us`,
    /// whose bytes were dropped. It has no bytes and decodes as [`MidiKind::Other`], as the
    /// prototype's system exclusive messages do.
    #[must_use]
    pub const fn sysex(time_us: u64, port: u8) -> Self {
        Self {
            time_us,
            port,
            len: 0,
            data: [0; Self::MAX_LEN],
        }
    }

    /// Whether this is the marker of a dropped system exclusive message.
    #[must_use]
    pub const fn is_sysex(&self) -> bool {
        self.len == 0
    }

    /// The message's bytes; empty for a [`sysex`](Self::sysex) marker.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.data.get(..usize::from(self.len)).unwrap_or(&[])
    }

    /// The arrival time in microseconds.
    #[must_use]
    pub fn time_us(&self) -> u64 {
        self.time_us
    }

    /// The input port the message came from.
    #[must_use]
    pub fn port(&self) -> u8 {
        self.port
    }

    /// The status byte: the first byte, or 0 without bytes.
    #[must_use]
    pub fn status(&self) -> u8 {
        self.byte(0)
    }

    /// The channel, 0–15: the low nibble of the status byte.
    #[must_use]
    pub fn channel(&self) -> u8 {
        self.status() & 0x0F
    }

    /// What the message is. A note on needs all three bytes: with a velocity of 0 it is a note
    /// off, and without its velocity it is [`MidiKind::Other`], as in the prototype.
    #[must_use]
    pub fn kind(&self) -> MidiKind {
        let high = self.status() & 0xF0;
        if high == 0x90 && self.bytes().len() >= 3 {
            return if self.byte(2) > 0 {
                MidiKind::NoteOn
            } else {
                MidiKind::NoteOff
            };
        }
        match high {
            0x80 => MidiKind::NoteOff,
            0xB0 => MidiKind::Cc,
            0xE0 => MidiKind::PitchBend,
            0xD0 => MidiKind::ChannelPressure,
            0xA0 => MidiKind::PolyPressure,
            0xC0 => MidiKind::Program,
            _ => MidiKind::Other,
        }
    }

    /// The second byte (the note of a note message, the controller of a control change), or 0.
    #[must_use]
    pub fn note(&self) -> u8 {
        self.byte(1)
    }

    /// The third byte (the velocity of a note message), or 0.
    #[must_use]
    pub fn velocity(&self) -> u8 {
        self.byte(2)
    }

    /// The message's value, as the prototype's `MidiMessage.value`: a pitch bend in
    /// −8192..=8191, the pressure of a channel pressure message, otherwise the third byte (a
    /// controller's value); 0 where the bytes are missing.
    #[must_use]
    pub fn value(&self) -> i32 {
        match self.kind() {
            MidiKind::PitchBend if self.bytes().len() >= 3 => {
                (i32::from(self.byte(1)) | (i32::from(self.byte(2)) << 7)) - 8192
            }
            MidiKind::ChannelPressure => i32::from(self.byte(1)),
            _ => i32::from(self.byte(2)),
        }
    }

    /// Byte `index`, or 0 when the message is shorter.
    fn byte(&self, index: usize) -> u8 {
        self.bytes().get(index).copied().unwrap_or(0)
    }
}

/// The ring of one MIDI input, split into its two ends: the input callback pushes, the render
/// thread pops. It holds up to `capacity` events; when it is full, an event is dropped and
/// counted ([`MidiOverflows`], RT-08).
///
/// Setup: allocates, so it runs off the real-time thread, when the input is opened.
#[must_use]
pub fn midi_channel(capacity: usize) -> (MidiSender, MidiReceiver) {
    let (events, receiver) = queue::channel(capacity);
    (
        MidiSender {
            events,
            overflows: MidiOverflows(Arc::new(AtomicU64::new(0))),
        },
        MidiReceiver { events: receiver },
    )
}

/// The input callback's end of a [`midi_channel`].
pub struct MidiSender {
    events: queue::Sender<MidiEvent>,
    overflows: MidiOverflows,
}

impl MidiSender {
    /// Appends `event`. When the ring is full, the event is dropped and counted; returns
    /// whether it was queued.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn push(&mut self, event: MidiEvent) -> bool {
        let Err(Full(_)) = self.events.push(event) else {
            return true;
        };
        // A plain counter: nothing is published through it, so `Relaxed` pairs with the
        // `Relaxed` load in `MidiOverflows::get`.
        self.overflows.0.fetch_add(1, Ordering::Relaxed);
        false
    }

    /// A handle that reads this ring's overflow count from any thread. Setup: call it before
    /// the sender moves into the callback.
    #[must_use]
    pub fn overflows(&self) -> MidiOverflows {
        self.overflows.clone()
    }
}

impl fmt::Debug for MidiSender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MidiSender")
            .field("free_slots", &self.events.free_slots())
            .field("overflows", &self.overflows)
            .finish()
    }
}

/// The render thread's end of a [`midi_channel`].
pub struct MidiReceiver {
    events: queue::Receiver<MidiEvent>,
}

impl MidiReceiver {
    /// Takes the oldest event, if any.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn pop(&mut self) -> Option<MidiEvent> {
        self.events.pop()
    }

    /// The oldest event, without taking it: the render thread merges several inputs by time.
    #[must_use]
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn peek(&self) -> Option<MidiEvent> {
        self.events.peek()
    }

    /// Events waiting; more may arrive at any time.
    #[must_use]
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether no event is waiting.
    #[must_use]
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl fmt::Debug for MidiReceiver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MidiReceiver")
            .field("len", &self.events.len())
            .finish()
    }
}

/// The number of events a [`midi_channel`] dropped because it was full.
#[derive(Clone)]
pub struct MidiOverflows(Arc<AtomicU64>);

impl fmt::Debug for MidiOverflows {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MidiOverflows").field(&self.get()).finish()
    }
}

impl MidiOverflows {
    /// The count so far.
    #[must_use]
    pub fn get(&self) -> u64 {
        // Pairs with the `Relaxed` increment in `MidiSender::push`: a plain counter.
        self.0.load(Ordering::Relaxed)
    }
}
