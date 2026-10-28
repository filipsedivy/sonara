//! Pitch conventions of the orchestration metadata: written and sounding pitch as separate types,
//! the transposition between them, and ranges.
//!
//! Pitches are MIDI note numbers with middle C = 60 = C4, the convention of `theory`
//! (`bass_note`: octave 4 starts at 60). The model is in semitones, so enharmonic spelling is
//! out of scope. Endpoints of a range are inclusive.

use thiserror::Error;

/// Highest MIDI note number.
const MAX_PITCH: i64 = 127;

/// A pitch as it is notated for the player. For a transposing instrument it differs from the pitch
/// that sounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WrittenPitch(u8);

/// A pitch as it sounds (concert pitch).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SoundingPitch(u8);

macro_rules! pitch_impl {
    ($name:ident) => {
        impl $name {
            /// The pitch of MIDI note `note`, or `None` outside 0–127.
            #[must_use]
            pub const fn new(note: i64) -> Option<Self> {
                if note < 0 || note > MAX_PITCH {
                    return None;
                }
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "checked to be 0..=127 above"
                )]
                Some(Self(note as u8))
            }

            /// The MIDI note number.
            #[must_use]
            pub const fn get(self) -> i64 {
                self.0 as i64
            }
        }
    };
}

pitch_impl!(WrittenPitch);
pitch_impl!(SoundingPitch);

/// The interval from written to sounding pitch in semitones: `sounding = written + semitones`.
///
/// A Bb clarinet is −2, a horn in F −7, a piccolo +12 and a double bass −12. Metadata stores
/// sounding pitches, so applying a transposition twice needs a conversion from the wrong type and
/// does not compile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Transposition(i8);

impl Transposition {
    /// No transposition: written and sounding pitch are the same.
    pub const NONE: Self = Self(0);

    /// A transposition of `semitones` (sounding minus written).
    #[must_use]
    pub const fn new(semitones: i8) -> Self {
        Self(semitones)
    }

    /// Sounding minus written, in semitones.
    #[must_use]
    pub const fn semitones(self) -> i8 {
        self.0
    }

    /// Whether the instrument is written at the pitch it sounds.
    #[must_use]
    pub const fn is_none(self) -> bool {
        self.0 == 0
    }

    /// The pitch that sounds when `written` is played, or `None` when it falls outside 0–127.
    #[must_use]
    pub const fn to_sounding(self, written: WrittenPitch) -> Option<SoundingPitch> {
        SoundingPitch::new(written.get() + self.0 as i64)
    }

    /// The pitch to write for `sounding`, or `None` when it falls outside 0–127.
    #[must_use]
    pub const fn to_written(self, sounding: SoundingPitch) -> Option<WrittenPitch> {
        WrittenPitch::new(sounding.get() - self.0 as i64)
    }
}

/// An inclusive range of sounding pitches with `low <= high`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PitchRange {
    low: SoundingPitch,
    high: SoundingPitch,
}

/// A range that cannot be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum RangeError {
    /// An endpoint is outside MIDI notes 0–127.
    #[error("range endpoint {0} is outside MIDI notes 0-127")]
    OutOfMidi(i64),
    /// The low endpoint is above the high one.
    #[error("range low {low} is above high {high}")]
    Inverted {
        /// The low endpoint.
        low: i64,
        /// The high endpoint.
        high: i64,
    },
}

impl PitchRange {
    /// The range from `low` to `high`, both inclusive.
    ///
    /// # Errors
    /// [`RangeError`] when an endpoint is outside 0–127 or `low > high`.
    pub const fn try_new(low: i64, high: i64) -> Result<Self, RangeError> {
        let Some(low_pitch) = SoundingPitch::new(low) else {
            return Err(RangeError::OutOfMidi(low));
        };
        let Some(high_pitch) = SoundingPitch::new(high) else {
            return Err(RangeError::OutOfMidi(high));
        };
        if low > high {
            return Err(RangeError::Inverted { low, high });
        }
        Ok(Self {
            low: low_pitch,
            high: high_pitch,
        })
    }

    /// The range from `low` to `high`; an invalid one in a `const` item is a compile error.
    ///
    /// # Panics
    /// When [`PitchRange::try_new`] would fail.
    #[must_use]
    pub const fn new(low: i64, high: i64) -> Self {
        match Self::try_new(low, high) {
            Ok(range) => range,
            Err(_) => panic!("invalid pitch range"),
        }
    }

    /// The lowest pitch.
    #[must_use]
    pub const fn low(self) -> SoundingPitch {
        self.low
    }

    /// The highest pitch.
    #[must_use]
    pub const fn high(self) -> SoundingPitch {
        self.high
    }

    /// Whether `pitch` is inside the range, endpoints included.
    #[must_use]
    pub const fn contains(self, pitch: SoundingPitch) -> bool {
        self.low.0 <= pitch.0 && pitch.0 <= self.high.0
    }

    /// Whether every pitch of `other` is inside this range.
    #[must_use]
    pub const fn encloses(self, other: Self) -> bool {
        self.low.0 <= other.low.0 && other.high.0 <= self.high.0
    }
}

#[cfg(test)]
mod tests {
    use super::{PitchRange, RangeError, SoundingPitch, Transposition, WrittenPitch};

    fn written(note: i64) -> WrittenPitch {
        WrittenPitch::new(note).expect("valid note")
    }

    fn sounding(note: i64) -> SoundingPitch {
        SoundingPitch::new(note).expect("valid note")
    }

    #[test]
    fn pitches_reject_notes_outside_midi() {
        assert!(WrittenPitch::new(-1).is_none());
        assert!(SoundingPitch::new(128).is_none());
        assert_eq!(WrittenPitch::new(0).map(WrittenPitch::get), Some(0));
        assert_eq!(SoundingPitch::new(127).map(SoundingPitch::get), Some(127));
    }

    #[test]
    fn transposition_converts_both_ways() {
        // (semitones, written, sounding): Bb clarinet C4 sounds Bb3, horn in F, piccolo, bass.
        let table = [
            (0, 60, 60),
            (-2, 60, 58),
            (-7, 60, 53),
            (12, 72, 84),
            (-12, 48, 36),
            (24, 60, 84),
        ];
        for (semitones, w, s) in table {
            let t = Transposition::new(semitones);
            assert_eq!(t.to_sounding(written(w)), Some(sounding(s)), "{semitones}");
            assert_eq!(t.to_written(sounding(s)), Some(written(w)), "{semitones}");
        }
    }

    #[test]
    fn transposition_fails_at_the_midi_boundaries() {
        let up = Transposition::new(12);
        assert_eq!(up.to_sounding(written(115)), Some(sounding(127)));
        assert_eq!(up.to_sounding(written(116)), None);
        let down = Transposition::new(-12);
        assert_eq!(down.to_sounding(written(12)), Some(sounding(0)));
        assert_eq!(down.to_sounding(written(11)), None);
        assert_eq!(down.to_written(sounding(115)), Some(written(127)));
        assert_eq!(down.to_written(sounding(116)), None);
    }

    #[test]
    fn range_endpoints_are_inclusive() {
        let range = PitchRange::new(55, 100);
        assert!(range.contains(sounding(55)));
        assert!(range.contains(sounding(100)));
        assert!(!range.contains(sounding(54)));
        assert!(!range.contains(sounding(101)));
        assert!(PitchRange::new(50, 60).encloses(PitchRange::new(50, 60)));
        assert!(!PitchRange::new(50, 60).encloses(PitchRange::new(49, 60)));
    }

    #[test]
    fn invalid_ranges_are_rejected() {
        assert_eq!(
            PitchRange::try_new(60, 59),
            Err(RangeError::Inverted { low: 60, high: 59 })
        );
        assert_eq!(PitchRange::try_new(-1, 5), Err(RangeError::OutOfMidi(-1)));
        assert_eq!(PitchRange::try_new(5, 128), Err(RangeError::OutOfMidi(128)));
        assert!(PitchRange::try_new(60, 60).is_ok());
    }
}
