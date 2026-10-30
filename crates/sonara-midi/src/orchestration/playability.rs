//! Playability facts: what a rule needs to know about playing an instrument beyond its ranges and
//! articulations. How long a note can be held on one breath, how long a mute or a change from
//! pizzicato back to arco takes, how many notes of a chord a string player holds, how quickly
//! timpani retune, and how a harp's pedals and hands are limited. Each fact says where it comes
//! from.
//!
//! The facts describe the instrument and its players, not a sample library. Most records have
//! none.
//!
//! A source often measures a duration in bars of moderate time. Such a [`Span`] keeps the source's
//! unit, and [`MODERATO_BPM`], one estimate, converts it to time.

use super::articulation::{Articulation, Axis, Mute};
use super::id::InstrumentType;
use super::instrument::{Family, Provenance};

/// The tempo, in crotchets per minute, of the moderate time in which sources measure durations
/// in bars.
///
/// An estimate: no source fixes one number. Wikipedia's tempo list
/// (<https://en.wikipedia.org/wiki/Tempo>) gives moderato as 108–120, and Widor, The Technique of
/// the Modern Orchestra (1906), p. 70, times the trumpet's breath "in moderate tempo" at
/// crotchet = 120.
pub const MODERATO_BPM: u16 = 120;

/// A duration as a source states it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Span {
    /// Milliseconds.
    Millis(u32),
    /// Beats (crotchets) of a moderate 4/4: the time that many beats take at [`MODERATO_BPM`],
    /// whatever the tempo of the piece. Two bars are eight beats.
    ModerateBeats(u16),
}

impl Span {
    /// The duration in milliseconds. A beat of a moderate 4/4 lasts 60 000 / [`MODERATO_BPM`] ms.
    ///
    /// A source that measures in bars of moderate time states how long the player needs, so a span
    /// converts at the moderate tempo and never at the tempo of the piece. A rule measures the time
    /// a piece leaves with the piece's own tempo and compares the two.
    #[must_use]
    pub const fn millis(self) -> u32 {
        match self {
            Self::Millis(millis) => millis,
            // 65 535 beats of 60 000 ms stay below u32::MAX, so neither step overflows.
            Self::ModerateBeats(beats) => beats as u32 * 60_000 / MODERATO_BPM as u32,
        }
    }

    /// Whether the span lasts no time at all.
    pub(super) const fn is_zero(self) -> bool {
        self.millis() == 0
    }
}

/// The letter name of a note. A harp pedal is named by one and moves every string that bears it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LetterName {
    /// C.
    C,
    /// D.
    D,
    /// E.
    E,
    /// F.
    F,
    /// G.
    G,
    /// A.
    A,
    /// B.
    B,
}

impl LetterName {
    /// The seven letter names, from C.
    pub const ALL: [Self; 7] = [
        Self::C,
        Self::D,
        Self::E,
        Self::F,
        Self::G,
        Self::A,
        Self::B,
    ];
}

/// What a playability fact states, with its values.
///
/// Each kind describes one sort of instrument ([`FactKind::fits`]): a breath describes a wind,
/// brass or voice part, a mute change strings or brass, a change back to arco and the held notes
/// of a chord the bowed strings, a retuning the timpani, pedals the harp, and notes per hand a
/// plucked or keyboard instrument. [`Instrument::try_new`](super::Instrument::try_new) rejects a
/// fact on any other instrument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FactKind {
    /// How long a player's breath lasts.
    Breath {
        /// The longest one note can be held in the medium register at piano. A louder, higher or
        /// lower note lasts less: the trumpet holds half as long at forte.
        held_note: Span,
    },
    /// How long putting on and taking off a mute takes.
    MuteChange {
        /// The time to put the mute on.
        on: Span,
        /// The time to take it off.
        off: Span,
    },
    /// How long a string player needs to change from pizzicato back to arco: the bow must come
    /// back to the string. The change to pizzicato needs very little.
    ArcoPizzSwitch {
        /// The time from the last pizzicato to the first note played with the bow.
        to_arco: Span,
    },
    /// How many notes of a chord a string player can hold. A chord of more notes is played
    /// quickly, and its lower notes are let go.
    SustainedStops {
        /// The number of notes that can be held.
        notes: u8,
    },
    /// How quickly the timpani retune.
    Retune {
        /// The time to retune one drum by a whole tone; a larger change takes longer.
        whole_tone: Span,
        /// The number of drums in a usual set.
        drums: u8,
    },
    /// A pedal harp's pedals: one per letter name, each moving every string of that name.
    HarpPedals {
        /// The pedals under the left foot, from the player's left to right.
        left: &'static [LetterName],
        /// The pedals under the right foot, from the player's left to right.
        right: &'static [LetterName],
        /// The time one foot needs to change one pedal; the two feet change theirs independently.
        change: Span,
    },
    /// The most notes one hand plays at once.
    NotesPerHand {
        /// The number of notes.
        notes: u8,
    },
}

impl FactKind {
    /// Whether a fact of this kind can describe an instrument of `instrument_type` (see
    /// [`FactKind`]).
    #[must_use]
    pub const fn fits(self, instrument_type: InstrumentType) -> bool {
        let family = instrument_type.family();
        match self {
            Self::Breath { .. } => {
                matches!(family, Family::Woodwinds | Family::Brass | Family::Choir)
            }
            Self::MuteChange { .. } => matches!(family, Family::Strings | Family::Brass),
            Self::ArcoPizzSwitch { .. } | Self::SustainedStops { .. } => {
                matches!(family, Family::Strings)
            }
            Self::Retune { .. } => matches!(instrument_type, InstrumentType::Timpani),
            Self::HarpPedals { .. } => matches!(instrument_type, InstrumentType::Harp),
            Self::NotesPerHand { .. } => matches!(family, Family::Plucked | Family::Keyboards),
        }
    }

    /// The same number for two facts of one kind, whatever their values.
    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::Breath { .. } => 0,
            Self::MuteChange { .. } => 1,
            Self::ArcoPizzSwitch { .. } => 2,
            Self::SustainedStops { .. } => 3,
            Self::Retune { .. } => 4,
            Self::HarpPedals { .. } => 5,
            Self::NotesPerHand { .. } => 6,
        }
    }

    /// Whether a duration or a count of the fact is zero.
    pub(super) const fn has_zero(self) -> bool {
        match self {
            Self::Breath { held_note } => held_note.is_zero(),
            Self::MuteChange { on, off } => on.is_zero() || off.is_zero(),
            Self::ArcoPizzSwitch { to_arco } => to_arco.is_zero(),
            Self::SustainedStops { notes } | Self::NotesPerHand { notes } => notes == 0,
            Self::Retune { whole_tone, drums } => whole_tone.is_zero() || drums == 0,
            Self::HarpPedals { change, .. } => change.is_zero(),
        }
    }

    /// Whether harp pedals name each letter name once between the two feet; `true` for every
    /// other kind.
    pub(super) const fn has_valid_pedals(self) -> bool {
        let Self::HarpPedals { left, right, .. } = self else {
            return true;
        };
        if left.len() + right.len() != LetterName::ALL.len() {
            return false;
        }
        let mut i = 0;
        while i < LetterName::ALL.len() {
            let letter = LetterName::ALL[i];
            if !names(left, letter) && !names(right, letter) {
                return false;
            }
            i += 1;
        }
        true
    }

    /// Whether `articulations`, a record's list, holds the articulation the fact rests on: a mute
    /// put on and taken off (`con_sordino` or a named mute, not a hand) for a mute change, and
    /// `pizzicato` for a change back to arco; `true` for every other kind.
    pub(super) const fn has_its_articulation(self, articulations: &[Articulation]) -> bool {
        let needs_mute = matches!(self, Self::MuteChange { .. });
        let needs_pizzicato = matches!(self, Self::ArcoPizzSwitch { .. });
        if !needs_mute && !needs_pizzicato {
            return true;
        }
        let mut i = 0;
        while i < articulations.len() {
            let articulation = articulations[i];
            if (needs_mute && is_mute_device(articulation))
                || (needs_pizzicato && matches!(articulation, Articulation::Pizzicato))
            {
                return true;
            }
            i += 1;
        }
        false
    }
}

/// A playability fact and where it comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fact {
    /// What the fact states.
    pub kind: FactKind,
    /// Where it comes from.
    pub provenance: Provenance,
}

/// Whether `letters` holds `letter`.
const fn names(letters: &[LetterName], letter: LetterName) -> bool {
    let mut i = 0;
    while i < letters.len() {
        if letters[i] as u8 == letter as u8 {
            return true;
        }
        i += 1;
    }
    false
}

/// Whether `articulation` names a mute the player puts on and takes off: a device, not a hand
/// stopping a horn, resting on a string or damping a harp string.
const fn is_mute_device(articulation: Articulation) -> bool {
    matches!(
        articulation.axis(),
        Axis::Mute(Mute::Generic | Mute::Cup | Mute::Harmon | Mute::Plunger | Mute::Bucket)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_convert_to_milliseconds() {
        // TST-10: a moderate beat lasts 500 ms at crotchet = 120, so two bars of 4/4 take 4 s.
        assert_eq!(Span::Millis(500).millis(), 500);
        assert_eq!(Span::Millis(0).millis(), 0);
        assert_eq!(Span::ModerateBeats(1).millis(), 500);
        assert_eq!(Span::ModerateBeats(8).millis(), 4_000);
        assert_eq!(Span::ModerateBeats(7).millis(), 3_500);
        // The longest span converts without overflow.
        assert_eq!(Span::ModerateBeats(u16::MAX).millis(), 32_767_500);
        assert!(Span::ModerateBeats(0).is_zero());
        assert!(!Span::ModerateBeats(1).is_zero());
    }

    #[test]
    fn moderato_lies_in_the_moderato_marks() {
        // Wikipedia's tempo list gives moderato as 108-120 crotchets per minute.
        assert!((108..=120).contains(&MODERATO_BPM), "{MODERATO_BPM}");
    }

    #[test]
    fn kinds_fit_their_instruments() {
        let breath = FactKind::Breath {
            held_note: Span::ModerateBeats(8),
        };
        let mute = FactKind::MuteChange {
            on: Span::ModerateBeats(8),
            off: Span::ModerateBeats(7),
        };
        let pizz = FactKind::ArcoPizzSwitch {
            to_arco: Span::Millis(500),
        };
        let stops = FactKind::SustainedStops { notes: 2 };
        let retune = FactKind::Retune {
            whole_tone: Span::Millis(400),
            drums: 4,
        };
        let pedals = FactKind::HarpPedals {
            left: &[LetterName::D, LetterName::C, LetterName::B],
            right: &[LetterName::E, LetterName::F, LetterName::G, LetterName::A],
            change: Span::ModerateBeats(1),
        };
        let hands = FactKind::NotesPerHand { notes: 4 };
        // TST-10: each row is the expected answer for one instrument of each sort, written from
        // the rules in the `FactKind` documentation.
        let rows: [(InstrumentType, [bool; 7]); 9] = [
            (
                InstrumentType::Flute,
                [true, false, false, false, false, false, false],
            ),
            (
                InstrumentType::TrumpetBb,
                [true, true, false, false, false, false, false],
            ),
            (
                InstrumentType::VoiceSoprano,
                [true, false, false, false, false, false, false],
            ),
            (
                InstrumentType::Violin,
                [false, true, true, true, false, false, false],
            ),
            (
                InstrumentType::Timpani,
                [false, false, false, false, true, false, false],
            ),
            (
                InstrumentType::Marimba,
                [false, false, false, false, false, false, false],
            ),
            (
                InstrumentType::Harp,
                [false, false, false, false, false, true, true],
            ),
            (
                InstrumentType::GuitarClassical,
                [false, false, false, false, false, false, true],
            ),
            (
                InstrumentType::PianoConcertGrand,
                [false, false, false, false, false, false, true],
            ),
        ];
        let kinds = [breath, mute, pizz, stops, retune, pedals, hands];
        for (instrument_type, expected) in rows {
            let fits = kinds.map(|kind| kind.fits(instrument_type));
            assert_eq!(fits, expected, "{instrument_type:?}");
        }
        // Each kind has its own tag.
        let mut tags = kinds.map(FactKind::tag);
        tags.sort_unstable();
        assert_eq!(tags, [0, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn harp_pedals_name_each_letter_once() {
        use LetterName::{A, B, C, D, E, F, G};
        let pedals = |left, right| FactKind::HarpPedals {
            left,
            right,
            change: Span::ModerateBeats(1),
        };
        assert!(pedals(&[D, C, B], &[E, F, G, A]).has_valid_pedals());
        // Any split of the seven between the feet is a layout.
        assert!(pedals(&[], &[C, D, E, F, G, A, B]).has_valid_pedals());
        // A letter missing, a letter twice, an eighth pedal.
        assert!(!pedals(&[D, C], &[E, F, G, A]).has_valid_pedals());
        assert!(!pedals(&[D, C, C], &[E, F, G, A]).has_valid_pedals());
        assert!(!pedals(&[D, C, B, B], &[E, F, G, A]).has_valid_pedals());
        // Other kinds have no pedals to check.
        assert!(FactKind::NotesPerHand { notes: 4 }.has_valid_pedals());
    }

    #[test]
    fn facts_rest_on_their_articulations() {
        use Articulation::{
            ConSordino, Legato, MuteCup, PalmMute, Pizzicato, Stopped, Sustain, Xylophonic,
        };
        let mute = FactKind::MuteChange {
            on: Span::ModerateBeats(8),
            off: Span::ModerateBeats(8),
        };
        let pizz = FactKind::ArcoPizzSwitch {
            to_arco: Span::Millis(500),
        };
        let stops = FactKind::SustainedStops { notes: 2 };
        // A mute device names the mute; a hand does not.
        assert!(mute.has_its_articulation(&[Sustain, ConSordino]));
        assert!(mute.has_its_articulation(&[MuteCup]));
        assert!(!mute.has_its_articulation(&[Sustain, Stopped, PalmMute, Xylophonic]));
        assert!(!mute.has_its_articulation(&[]));
        assert!(pizz.has_its_articulation(&[Legato, Pizzicato]));
        assert!(!pizz.has_its_articulation(&[Legato, ConSordino]));
        // Other kinds rest on no articulation.
        assert!(stops.has_its_articulation(&[]));
    }

    #[test]
    fn zero_values_are_found() {
        let zero = [
            FactKind::Breath {
                held_note: Span::Millis(0),
            },
            FactKind::MuteChange {
                on: Span::ModerateBeats(8),
                off: Span::ModerateBeats(0),
            },
            FactKind::ArcoPizzSwitch {
                to_arco: Span::Millis(0),
            },
            FactKind::SustainedStops { notes: 0 },
            FactKind::Retune {
                whole_tone: Span::Millis(400),
                drums: 0,
            },
            FactKind::HarpPedals {
                left: &[],
                right: &[],
                change: Span::ModerateBeats(0),
            },
            FactKind::NotesPerHand { notes: 0 },
        ];
        for kind in zero {
            assert!(kind.has_zero(), "{kind:?}");
        }
        assert!(
            !FactKind::Retune {
                whole_tone: Span::Millis(1),
                drums: 1,
            }
            .has_zero()
        );
    }
}
