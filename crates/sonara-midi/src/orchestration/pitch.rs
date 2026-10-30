//! Pitch conventions of the orchestration metadata: written and sounding pitch as separate types,
//! the transposition between them, clefs, and ranges.
//!
//! Pitches are MIDI note numbers with middle C = 60 = C4, the convention of `theory`
//! (`bass_note`: octave 4 starts at 60). The model is in semitones, so enharmonic spelling is
//! out of scope. Endpoints of a range are inclusive.

use thiserror::Error;

/// Highest MIDI note number.
const MAX_PITCH: i64 = 127;

/// Semitones in an octave.
const SEMITONES_PER_OCTAVE: i64 = 12;

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

/// The clef a part is written in.
///
/// A clef can carry an octave of its own: [`Clef::Treble8vb`] sounds an octave below the written
/// note. That octave belongs to the staff, not to the instrument. A tenor voice is not a
/// transposing instrument: in `treble_8vb` its sounding C3 is written C4, and on a bass-clef
/// staff shared with the basses the same note is written C3. An instrument whose written pitch
/// differs from the sounding one in every clef, such as the piccolo or the double bass, keeps
/// that difference in its [`Transposition`] instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Clef {
    /// The treble (G) clef.
    Treble,
    /// The treble clef with an 8 below it: the notes sound an octave lower than written. The
    /// usual clef of tenor voices.
    Treble8vb,
    /// The bass (F) clef.
    Bass,
    /// The alto (C) clef, middle C on the middle line.
    Alto,
    /// The tenor (C) clef, middle C on the fourth line.
    Tenor,
    /// The neutral percussion clef: the lines stand for instruments, not for pitches.
    Percussion,
    /// A one-line staff, which shows rhythm but no pitch.
    SingleLine,
}

impl Clef {
    /// Every clef, in declaration order.
    pub const ALL: [Self; 7] = [
        Self::Treble,
        Self::Treble8vb,
        Self::Bass,
        Self::Alto,
        Self::Tenor,
        Self::Percussion,
        Self::SingleLine,
    ];

    /// The stable slug, for example `treble_8vb`. It is the external form and is never renamed.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Treble => "treble",
            Self::Treble8vb => "treble_8vb",
            Self::Bass => "bass",
            Self::Alto => "alto",
            Self::Tenor => "tenor",
            Self::Percussion => "percussion",
            Self::SingleLine => "single_line",
        }
    }

    /// The clef with slug `slug`, or `None` for anything else: the match is exact, with no case
    /// folding or trimming.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|clef| clef.slug() == slug)
    }

    /// The octave the clef itself adds, as sounding minus written in octaves (the direction of
    /// [`Transposition`]): −1 for [`Clef::Treble8vb`], whose notes sound an octave lower than
    /// written, and 0 for every other clef.
    #[must_use]
    pub const fn octave(self) -> i8 {
        match self {
            Self::Treble8vb => -1,
            Self::Treble
            | Self::Bass
            | Self::Alto
            | Self::Tenor
            | Self::Percussion
            | Self::SingleLine => 0,
        }
    }

    /// Whether a note's place on the staff names a pitch. The percussion clef and the one-line
    /// staff do not.
    #[must_use]
    pub const fn shows_pitch(self) -> bool {
        !matches!(self, Self::Percussion | Self::SingleLine)
    }

    /// The pitch to write in this clef for `sounding` on an instrument with `transposition`:
    /// the instrument's transposition, then the clef's own [`octave`](Clef::octave). In a clef
    /// with octave 0 this equals [`Transposition::to_written`].
    ///
    /// `None` when the clef shows no pitch ([`Clef::shows_pitch`]) or the written pitch falls
    /// outside 0–127.
    #[must_use]
    pub const fn to_written(
        self,
        transposition: Transposition,
        sounding: SoundingPitch,
    ) -> Option<WrittenPitch> {
        if !self.shows_pitch() {
            return None;
        }
        WrittenPitch::new(
            sounding.get() - transposition.0 as i64 - SEMITONES_PER_OCTAVE * self.octave() as i64,
        )
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
    use super::{Clef, PitchRange, RangeError, SoundingPitch, Transposition, WrittenPitch};

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
    fn clef_slugs_round_trip() {
        let slugs: Vec<&str> = Clef::ALL.into_iter().map(Clef::slug).collect();
        assert_eq!(
            slugs,
            [
                "treble",
                "treble_8vb",
                "bass",
                "alto",
                "tenor",
                "percussion",
                "single_line"
            ]
        );
        for (index, clef) in Clef::ALL.into_iter().enumerate() {
            assert_eq!(clef as usize, index, "{clef:?}");
            assert_eq!(Clef::from_slug(clef.slug()), Some(clef));
        }
    }

    /// `from_slug` takes the exact slug only (TST-03).
    #[test]
    fn clef_lookup_is_strict() {
        for text in [
            "",
            "Treble",
            "treble ",
            "treble-8vb",
            "treble8vb",
            "treble_8va",
            "g",
            "neutral",
            "single line",
        ] {
            assert_eq!(Clef::from_slug(text), None, "{text:?}");
        }
    }

    #[test]
    fn only_the_treble_8vb_clef_has_an_octave_and_two_clefs_show_no_pitch() {
        for clef in Clef::ALL {
            let octave = if clef == Clef::Treble8vb { -1 } else { 0 };
            assert_eq!(clef.octave(), octave, "{clef:?}");
            let shows_pitch = !matches!(clef, Clef::Percussion | Clef::SingleLine);
            assert_eq!(clef.shows_pitch(), shows_pitch, "{clef:?}");
        }
    }

    #[test]
    fn clefs_write_known_pitches() {
        // (clef, semitones, sounding, written): a tenor's C3 in treble_8vb and on a bass staff,
        // a Bb clarinet in treble, a double bass in bass clef.
        let table = [
            (Clef::Treble8vb, 0, 48, Some(60)),
            (Clef::Bass, 0, 48, Some(48)),
            (Clef::Treble, -2, 58, Some(60)),
            (Clef::Bass, -12, 28, Some(40)),
            (Clef::Alto, 0, 60, Some(60)),
            (Clef::Treble8vb, 0, 115, Some(127)),
            (Clef::Treble8vb, 0, 116, None),
            (Clef::Treble, 12, 11, None),
            (Clef::Percussion, 0, 60, None),
            (Clef::SingleLine, 0, 60, None),
        ];
        for (clef, semitones, s, w) in table {
            assert_eq!(
                clef.to_written(Transposition::new(semitones), sounding(s)),
                w.map(written),
                "{clef:?} {semitones} {s}"
            );
        }
    }

    /// Against `Transposition::to_written` as the oracle, for every sounding pitch: a clef with
    /// octave 0 writes what the transposition writes, `treble_8vb` what a transposition an
    /// octave further down writes, and a clef without pitch writes nothing.
    #[test]
    fn clef_writing_agrees_with_the_transposition() {
        for semitones in -36..=36 {
            let transposition = Transposition::new(semitones);
            let octave_lower = Transposition::new(semitones - 12);
            for note in 0..=127 {
                let pitch = sounding(note);
                for clef in Clef::ALL {
                    let expected = match clef {
                        Clef::Treble8vb => octave_lower.to_written(pitch),
                        Clef::Percussion | Clef::SingleLine => None,
                        Clef::Treble | Clef::Bass | Clef::Alto | Clef::Tenor => {
                            transposition.to_written(pitch)
                        }
                    };
                    assert_eq!(
                        clef.to_written(transposition, pitch),
                        expected,
                        "{clef:?} {semitones} {note}"
                    );
                }
            }
        }
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
