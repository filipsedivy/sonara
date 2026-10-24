//! MIDI notes and controller events of the project model (`sonara.domain.midi`).

use std::cmp::Ordering;

use crate::EPSILON;
use crate::error::{DomainError, Result, require, require_finite};
use crate::ids::{self, IdGenerator};
use crate::pyfmt::str_repr;

/// A MIDI note in a clip. Positions are in beats relative to the clip start.
#[derive(Debug, Clone, PartialEq)]
pub struct MidiNote {
    /// Stable ID (`n_…`).
    pub id: String,
    /// MIDI pitch, 0–127.
    pub pitch: u8,
    /// Start in beats (≥ 0).
    pub start: f64,
    /// Length in beats (> 0).
    pub length: f64,
    /// Velocity, 1–127.
    pub velocity: u8,
    /// MIDI channel, 0–15.
    pub channel: u8,
}

impl MidiNote {
    /// A validated note with an existing ID.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::InvalidArgument`] with the prototype's messages for a pitch,
    /// velocity or channel out of range, or a non-finite, negative or empty position.
    pub fn new(
        id: impl Into<String>,
        pitch: i64,
        start: f64,
        length: f64,
        velocity: i64,
        channel: i64,
    ) -> Result<Self> {
        require((0..=127).contains(&pitch), || {
            format!("note pitch must be 0-127, got {pitch}")
        })?;
        require((1..=127).contains(&velocity), || {
            format!("note velocity must be 1-127, got {velocity}")
        })?;
        require((0..=15).contains(&channel), || {
            "MIDI channel must be 0-15".to_owned()
        })?;
        require_finite(start, "note start")?;
        require_finite(length, "note length")?;
        require(start >= -EPSILON, || "note start must be >= 0".to_owned())?;
        require(length > EPSILON, || "note length must be > 0".to_owned())?;
        Ok(Self {
            id: id.into(),
            pitch: u8::try_from(pitch).unwrap_or(0),
            start,
            length,
            velocity: u8::try_from(velocity).unwrap_or(1),
            channel: u8::try_from(channel).unwrap_or(0),
        })
    }

    /// A new note with a fresh ID; a negative start is clamped to 0 (the prototype's
    /// `MidiNote.create`).
    ///
    /// # Errors
    ///
    /// The validation errors of [`MidiNote::new`].
    pub fn create(
        ids: &mut dyn IdGenerator,
        pitch: i64,
        start: f64,
        length: f64,
        velocity: i64,
        channel: i64,
    ) -> Result<Self> {
        Self::new(
            ids.new_id(ids::NOTE),
            pitch,
            start.max(0.0),
            length,
            velocity,
            channel,
        )
    }

    /// End position in beats.
    #[must_use]
    pub fn end(&self) -> f64 {
        self.start + self.length
    }

    /// The note shifted in time and pitch; the start is clamped to 0.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::InvalidArgument`] when the new pitch leaves 0–127.
    pub fn moved(&self, delta_beats: f64, delta_pitch: i64) -> Result<Self> {
        Self::new(
            self.id.clone(),
            i64::from(self.pitch) + delta_pitch,
            (self.start + delta_beats).max(0.0),
            self.length,
            i64::from(self.velocity),
            i64::from(self.channel),
        )
    }
}

/// Lowest pitch-bend value.
pub const PITCH_BEND_MIN: i64 = -8192;
/// Highest pitch-bend value.
pub const PITCH_BEND_MAX: i64 = 8191;
/// Pitch-class names with sharps, C first.
pub const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// The kind of a MIDI controller event.
///
/// The variant order is the order of the stored strings: the prototype sorts controller events
/// by the string value of their kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ControlKind {
    /// Control change (`number` is the CC number).
    Cc,
    /// Channel pressure (aftertouch).
    ChannelPressure,
    /// Pitch bend, −8192..8191.
    PitchBend,
}

impl ControlKind {
    /// The kind as stored in project files.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cc => "cc",
            Self::PitchBend => "pitch_bend",
            Self::ChannelPressure => "channel_pressure",
        }
    }

    /// Parses the stored form.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "cc" => Some(Self::Cc),
            "pitch_bend" => Some(Self::PitchBend),
            "channel_pressure" => Some(Self::ChannelPressure),
            _ => None,
        }
    }
}

/// A controller value at `beat` (relative to the clip content).
#[derive(Debug, Clone, PartialEq)]
pub struct MidiControlEvent {
    /// Position in beats (≥ 0).
    pub beat: f64,
    /// The kind of event.
    pub kind: ControlKind,
    /// CC and pressure: 0–127; pitch bend: −8192..8191.
    pub value: i64,
    /// CC number (ignored for other kinds).
    pub number: i64,
    /// MIDI channel, 0–15.
    pub channel: i64,
}

impl MidiControlEvent {
    /// A validated event.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::InvalidArgument`] with the prototype's messages.
    pub fn new(
        beat: f64,
        kind: ControlKind,
        value: i64,
        number: i64,
        channel: i64,
    ) -> Result<Self> {
        require_finite(beat, "controller event beat")?;
        require(beat >= -EPSILON, || {
            "controller event beat must be >= 0".to_owned()
        })?;
        require((0..=15).contains(&channel), || {
            "MIDI channel must be 0-15".to_owned()
        })?;
        if kind == ControlKind::PitchBend {
            require((PITCH_BEND_MIN..=PITCH_BEND_MAX).contains(&value), || {
                "pitch bend must be -8192..8191".to_owned()
            })?;
        } else {
            require((0..=127).contains(&value), || {
                "controller value must be 0-127".to_owned()
            })?;
            require((0..=127).contains(&number), || {
                "CC number must be 0-127".to_owned()
            })?;
        }
        Ok(Self {
            beat,
            kind,
            value,
            number,
            channel,
        })
    }
}

/// Notes sorted by start, then pitch, then ID (the prototype's `sort_notes`). The sort is
/// stable and, like Python's `<`, treats `-0.0` and `0.0` as equal.
pub fn sort_notes(notes: &mut [MidiNote]) {
    notes.sort_by(|a, b| {
        a.start
            .partial_cmp(&b.start)
            .unwrap_or(Ordering::Equal)
            .then(a.pitch.cmp(&b.pitch))
            .then_with(|| a.id.cmp(&b.id))
    });
}

/// Scientific pitch notation with C4 = 60 (`"C#4"`).
#[must_use]
pub fn note_name(pitch: u8) -> String {
    let pitch = i32::from(pitch);
    let name = NOTE_NAMES
        .get(usize::try_from(pitch % 12).unwrap_or(0))
        .copied()
        .unwrap_or("C");
    format!("{name}{}", pitch / 12 - 1)
}

/// Parses `C4`, `F#3` or `Bb-1` (C4 = 60) into a MIDI pitch.
///
/// # Errors
///
/// [`crate::error::ErrorCode::InvalidArgument`]: `invalid note name '…'` or
/// `note '…' is outside the MIDI range`.
pub fn parse_note_name(name: &str) -> Result<u8> {
    let invalid = || DomainError::invalid(format!("invalid note name {}", str_repr(name)));
    let text = name.trim();
    if text.chars().count() < 2 {
        return Err(invalid());
    }
    let mut chars = text.chars();
    let letter = chars.next().ok_or_else(invalid)?.to_ascii_uppercase();
    let mut rest: String = chars.collect();
    let mut accidental = "";
    if rest.starts_with('#') {
        accidental = "#";
        rest.remove(0);
    } else if rest.starts_with('b') {
        accidental = "B";
        rest.remove(0);
    }
    let pitch_class = match (letter, accidental) {
        ('C', "") | ('B', "#") => 0,
        ('C', "#") | ('D', "B") => 1,
        ('D', "") => 2,
        ('D', "#") | ('E', "B") => 3,
        ('E', "") | ('F', "B") => 4,
        ('E', "#") | ('F', "") => 5,
        ('F', "#") | ('G', "B") => 6,
        ('G', "") => 7,
        ('G', "#") | ('A', "B") => 8,
        ('A', "") => 9,
        ('A', "#") | ('B', "B") => 10,
        ('B', "") | ('C', "B") => 11,
        _ => return Err(invalid()),
    };
    let octave: i64 = rest.trim().parse().map_err(|_| invalid())?;
    let mut pitch = (octave + 1) * 12 + pitch_class;
    if letter == 'C' && accidental == "B" {
        pitch -= 12;
    }
    if letter == 'B' && accidental == "#" {
        pitch += 12;
    }
    u8::try_from(pitch)
        .ok()
        .filter(|p| *p <= 127)
        .ok_or_else(|| {
            DomainError::invalid(format!("note {} is outside the MIDI range", str_repr(name)))
        })
}

#[cfg(test)]
mod tests {
    use super::{ControlKind, MidiNote, note_name, parse_note_name};
    use crate::ids::SequentialIds;

    #[test]
    fn control_kinds_order_like_their_stored_strings() {
        let mut kinds = [
            ControlKind::PitchBend,
            ControlKind::Cc,
            ControlKind::ChannelPressure,
        ];
        kinds.sort();
        let mut names = kinds.map(ControlKind::as_str);
        let sorted = names;
        names.sort_unstable();
        assert_eq!(names, sorted);
    }

    #[test]
    fn note_names_round_trip() {
        assert_eq!(note_name(60), "C4");
        assert_eq!(note_name(61), "C#4");
        assert_eq!(parse_note_name("C4").expect("valid"), 60);
        assert_eq!(parse_note_name("Bb-1").expect("valid"), 10);
        assert_eq!(parse_note_name("F#3").expect("valid"), 54);
        assert_eq!(parse_note_name("Cb4").expect("valid"), 59);
        assert_eq!(parse_note_name("B#3").expect("valid"), 60);
        assert_eq!(
            parse_note_name("H2").expect_err("invalid").message,
            "invalid note name 'H2'"
        );
    }

    #[test]
    fn create_assigns_ids_and_clamps_the_start() {
        let mut ids = SequentialIds::new(1000);
        let note = MidiNote::create(&mut ids, 60, -0.5, 1.0, 100, 0).expect("valid note");
        assert_eq!(note.id, "n_000000001001");
        assert!(note.start.abs() < f64::EPSILON);
        assert!((note.end() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn invalid_notes_report_the_prototype_messages() {
        let err = MidiNote::new("n_1", 128, 0.0, 1.0, 100, 0).expect_err("pitch out of range");
        assert_eq!(err.message, "note pitch must be 0-127, got 128");
        let err = MidiNote::new("n_1", 60, 0.0, 0.0, 100, 0).expect_err("empty note");
        assert_eq!(err.message, "note length must be > 0");
    }
}
