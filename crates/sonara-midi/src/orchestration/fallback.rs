//! What to play when no installed sound plays an instrument or an articulation.
//!
//! When no sound source plays a part as written, the application falls back step by step and
//! tells the user what it did: first the nearest articulation of the same instrument, then, when
//! no sound of the instrument is installed at all, a neighbour of its family. This module holds
//! the musical data of those two steps; it names no sample library, preset or plugin, so the
//! steps that do (an exact preset, a built-in synthesizer sound) live with the application.
//!
//! - [`NEIGHBOURS`] lists, for every instrument type, the instruments that can play its part in
//!   order of preference ([`neighbours`]). A neighbour plays the part only when every note lies in
//!   its playable range, which the caller checks against the neighbour's record; it never
//!   transposes octaves. A neighbour of another family, or one that loses the part's pitch, is
//!   always confirmed by the user first ([`Neighbour::asks_first`]).
//! - [`SUBSTITUTIONS`] gives, for each of the fifteen original articulations
//!   ([`Articulation::is_stable`]), the articulations that can stand in for it in order of
//!   preference, then the [`Emulation`] that imitates it by rewriting the notes when none of them
//!   is available ([`substitution`], [`substitutes`]). An articulation that refines another
//!   ([`Articulation::parent`]) has no row: the caller tries its parent first and then the
//!   parent's row. A stand-in the instrument does not play is skipped by the caller, so a row
//!   lists string, wind and percussion stand-ins alike.
//!
//! Every value is a musical judgement unless a source that was read states it, and its
//! provenance says which. The statics are checked at compile time, so invalid data does not
//! build; the row types offer the same checks (`check`) for data built at run time.
//!
//! Nothing here runs on a real-time thread (RT-01).

use thiserror::Error;

use super::articulation::{Articulation, Axis, Vibrato};
use super::data::INSTRUMENTS;
use super::id::InstrumentType;
use super::instrument::{PitchModel, Provenance, is_stated};
use super::pitch::PitchRange;

pub use super::data::fallback::{NEIGHBOURS, SUBSTITUTIONS};

/// Why a fallback row is invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum FallbackError {
    /// An instrument is listed as its own neighbour.
    #[error("an instrument is listed as its own neighbour")]
    OwnNeighbour,
    /// A neighbour or a stand-in is listed twice in one row.
    #[error("a neighbour or a stand-in is listed twice in one row")]
    Repeated,
    /// A neighbour of another family plays without asking the user first.
    #[error("a neighbour of another family plays without asking first")]
    FamilyChangeUnasked,
    /// An unpitched neighbour plays a pitched part, which loses its pitch, without asking first.
    #[error("an unpitched neighbour plays a pitched part without asking first")]
    PitchLostUnasked,
    /// A pitched neighbour plays an unpitched part without naming the note it plays.
    #[error("a pitched neighbour of an unpitched part names no note")]
    NoteUnnamed,
    /// A manner does not fit the pair: a note named for a part that has pitches, or an
    /// articulation that is a passage state or that the neighbour does not play.
    #[error("the manner does not fit the instrument and its neighbour")]
    InvalidManner,
    /// Two pitched instruments share no note, so the neighbour could never play the part.
    #[error("a pitched neighbour shares no note with the instrument")]
    NoSharedRange,
    /// A provenance is empty or blank.
    #[error("a provenance is empty or blank")]
    EmptyProvenance,
    /// An articulation is listed as its own substitute.
    #[error("an articulation is listed as its own substitute")]
    OwnSubstitute,
    /// A substitute names a passage state, a mute or a vibrato, not a note's articulation.
    #[error("a substitute names a passage state, not a note's articulation")]
    NotPrimary,
    /// An emulation has a number of zero or numbers in the wrong order.
    #[error("an emulation has a zero or an inverted number")]
    InvalidEmulation,
}

/// A rewrite of the notes that imitates an articulation no sound plays, or that brings a
/// stand-in nearer the articulation asked for.
///
/// A dynamic step is one step of notated dynamic, the step the balance rules weigh
/// ([`super::rules::DYNAMIC_STEP`]); how many velocity units it is depends on the sound and is
/// the caller's to decide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Emulation {
    /// Each note overlaps the next by a few milliseconds, so that a sound which joins
    /// overlapping notes plays them legato. How long the overlap is, is the caller's to decide.
    Overlap,
    /// Each note is shortened to `1 / divisor` of its written length: staccato from long notes.
    Shorten {
        /// The divisor, 2 or more: 4 keeps a quarter of each note.
        divisor: u8,
    },
    /// Each note is played a dynamic step louder: an accent.
    Accent,
    /// Each note is played a dynamic step softer.
    Soften,
    /// Each note is played as repeated notes of one value with a small variation in velocity: a
    /// tremolo or a roll written out.
    Repeat {
        /// The value of the repeated notes as notes per whole note: 32 for 32nd notes. A power
        /// of two, 4 or more.
        per_whole_note: u8,
    },
    /// The written note alternates with its upper auxiliary note: a trill written out.
    Alternate {
        /// The slowest rate, in notes per second; at least 1.
        slowest: u8,
        /// The fastest rate, in notes per second; at least `slowest`.
        fastest: u8,
    },
}

impl Emulation {
    /// Whether the numbers of the emulation are valid.
    ///
    /// # Errors
    /// [`FallbackError::InvalidEmulation`] for a divisor below 2, a note value that is not a
    /// power of two of 4 or more, or a rate of 0 or a slowest rate above the fastest.
    pub const fn check(self) -> Result<(), FallbackError> {
        let valid = match self {
            Self::Overlap | Self::Accent | Self::Soften => true,
            Self::Shorten { divisor } => divisor >= 2,
            Self::Repeat { per_whole_note } => {
                per_whole_note >= 4 && per_whole_note.is_power_of_two()
            }
            Self::Alternate { slowest, fastest } => slowest >= 1 && slowest <= fastest,
        };
        if valid {
            Ok(())
        } else {
            Err(FallbackError::InvalidEmulation)
        }
    }
}

/// The kind of passage a stand-in suits. For a passage of another kind the caller does not use
/// the stand-in without asking the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    /// Short notes, such as the short notes of a marcato passage.
    ShortNotes,
    /// A single line, one note at a time.
    SingleLines,
    /// A passage played tremolo.
    TremoloPassages,
    /// Accented notes, for their weight.
    Accents,
    /// Chords.
    Chords,
    /// Arpeggios and broken chords.
    Arpeggios,
    /// Single strokes, not rolls or swells.
    SingleStrokes,
}

/// How a neighbour plays the part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Manner {
    /// The part's notes and articulations, unchanged.
    AsWritten,
    /// Every note a dynamic step softer ([`Emulation::Soften`]), nearer a quieter instrument.
    Softer,
    /// Every note with this articulation in place of the part's own, such as a suspended
    /// cymbal's roll for the long wash of a tam-tam. The neighbour plays it idiomatically.
    With(Articulation),
    /// Every stroke of an unpitched part on the neighbour's highest playable note.
    OnHighestNote,
    /// Every stroke of an unpitched part on the neighbour's lowest playable note.
    OnLowestNote,
}

/// An instrument that can play a part written for another when no sound of the other is
/// installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Neighbour {
    /// The instrument that plays the part instead.
    pub instrument: InstrumentType,
    /// How it plays the part.
    pub manner: Manner,
    /// The kind of passage it suits, or `None` for any passage.
    pub only: Option<Scope>,
    /// Whether the user confirms the neighbour even when every note lies in its playable range:
    /// it belongs to another family or loses the part's pitch, or it changes the part's role,
    /// as a choir does in place of a soloist. Notes outside the range always need confirmation.
    pub asks_first: bool,
    /// Why the neighbour was chosen.
    pub provenance: Provenance,
}

impl Neighbour {
    /// Whether the neighbour fits a part written for `instrument`.
    ///
    /// # Errors
    /// - [`FallbackError::OwnNeighbour`] when the neighbour is `instrument` itself;
    /// - [`FallbackError::EmptyProvenance`] when the provenance is blank;
    /// - [`FallbackError::FamilyChangeUnasked`] when it is of another family and does not ask
    ///   first;
    /// - [`FallbackError::InvalidManner`] when the manner names a note for a part that has
    ///   pitches or for an unpitched neighbour, or an articulation that is a passage state or
    ///   that the neighbour's record does not list;
    /// - [`FallbackError::NoteUnnamed`] when a pitched neighbour of an unpitched part names no
    ///   note;
    /// - [`FallbackError::PitchLostUnasked`] when an unpitched neighbour of a pitched part does
    ///   not ask first;
    /// - [`FallbackError::NoSharedRange`] when both are pitched and their playable ranges share
    ///   no note.
    pub const fn check(&self, instrument: InstrumentType) -> Result<(), FallbackError> {
        if self.instrument as u8 == instrument as u8 {
            return Err(FallbackError::OwnNeighbour);
        }
        if !is_stated(self.provenance) {
            return Err(FallbackError::EmptyProvenance);
        }
        if self.instrument.family() as u8 != instrument.family() as u8 && !self.asks_first {
            return Err(FallbackError::FamilyChangeUnasked);
        }
        let part = playable(instrument);
        let neighbour = playable(self.instrument);
        match self.manner {
            Manner::OnHighestNote | Manner::OnLowestNote => {
                if part.is_some() || neighbour.is_none() {
                    return Err(FallbackError::InvalidManner);
                }
            }
            Manner::With(articulation) => {
                if !matches!(articulation.axis(), Axis::Primary)
                    || !lists(self.instrument, articulation)
                {
                    return Err(FallbackError::InvalidManner);
                }
            }
            Manner::AsWritten | Manner::Softer => {}
        }
        match (part, neighbour) {
            (None, Some(_)) => {
                if !matches!(self.manner, Manner::OnHighestNote | Manner::OnLowestNote) {
                    return Err(FallbackError::NoteUnnamed);
                }
            }
            (Some(_), None) => {
                if !self.asks_first {
                    return Err(FallbackError::PitchLostUnasked);
                }
            }
            (Some(part), Some(neighbour)) => {
                if part.high().get() < neighbour.low().get()
                    || neighbour.high().get() < part.low().get()
                {
                    return Err(FallbackError::NoSharedRange);
                }
            }
            (None, None) => {}
        }
        Ok(())
    }
}

/// The neighbours of one instrument type, best first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeighbourList {
    /// The instrument whose part the neighbours play.
    pub instrument: InstrumentType,
    /// Its neighbours in order of preference; empty when no instrument comes near enough.
    pub neighbours: &'static [Neighbour],
}

impl NeighbourList {
    /// Whether every neighbour fits the instrument ([`Neighbour::check`]) and none is listed
    /// twice.
    ///
    /// # Errors
    /// The first error of [`Neighbour::check`], or [`FallbackError::Repeated`] when an
    /// instrument is listed twice.
    pub const fn check(&self) -> Result<(), FallbackError> {
        let mut i = 0;
        while i < self.neighbours.len() {
            if let Err(error) = self.neighbours[i].check(self.instrument) {
                return Err(error);
            }
            let mut j = i + 1;
            while j < self.neighbours.len() {
                if self.neighbours[i].instrument as u8 == self.neighbours[j].instrument as u8 {
                    return Err(FallbackError::Repeated);
                }
                j += 1;
            }
            i += 1;
        }
        Ok(())
    }
}

/// What plays in place of an articulation no sound plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StandIn {
    /// Another articulation of the same instrument.
    Articulation(Articulation),
    /// The same articulation from the instrument's other ensemble size: a section's for a solo,
    /// a solo's for a section.
    OtherEnsemble,
    /// Another instrument, with its ordinary sound. It changes the part's family, so the user
    /// always confirms it first.
    Instrument(InstrumentType),
}

/// One stand-in for an articulation, with what is changed as it plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArticulationSubstitute {
    /// What plays instead.
    pub stand_in: StandIn,
    /// The rewrite of the notes as it plays, such as long notes shortened for a staccato, or
    /// `None` to play the notes as written.
    pub rewrite: Option<Emulation>,
    /// The vibrato it plays with in place of the passage's, or `None` to keep the passage's.
    pub vibrato: Option<Vibrato>,
    /// The kind of passage it suits, or `None` for any passage.
    pub only: Option<Scope>,
}

impl ArticulationSubstitute {
    /// Whether the substitute can stand in for `articulation`.
    ///
    /// # Errors
    /// - [`FallbackError::OwnSubstitute`] when it is `articulation` itself;
    /// - [`FallbackError::NotPrimary`] when it is a mute or vibrato id;
    /// - [`FallbackError::InvalidEmulation`] when its rewrite is invalid.
    pub const fn check(&self, articulation: Articulation) -> Result<(), FallbackError> {
        if let StandIn::Articulation(stand_in) = self.stand_in {
            if stand_in as u8 == articulation as u8 {
                return Err(FallbackError::OwnSubstitute);
            }
            if !matches!(stand_in.axis(), Axis::Primary) {
                return Err(FallbackError::NotPrimary);
            }
        }
        match self.rewrite {
            Some(rewrite) => rewrite.check(),
            None => Ok(()),
        }
    }
}

/// The substitution row of one articulation: what stands in for it, in order, and how it is
/// emulated when nothing does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Substitution {
    /// The articulation asked for.
    pub articulation: Articulation,
    /// Its stand-ins in order of preference.
    pub substitutes: &'static [ArticulationSubstitute],
    /// The rewrite that imitates it when no stand-in plays, or `None` when no rewrite comes
    /// near it; the notes are then played with the ordinary sound.
    pub emulation: Option<Emulation>,
    /// Where the row comes from.
    pub provenance: Provenance,
}

impl Substitution {
    /// Whether every substitute can stand in for the row's articulation
    /// ([`ArticulationSubstitute::check`]), none is listed twice, and the emulation and the
    /// provenance are valid.
    ///
    /// # Errors
    /// [`FallbackError::EmptyProvenance`] for a blank provenance, the first error of a
    /// substitute's check, [`FallbackError::Repeated`] when a stand-in is listed twice, or
    /// [`FallbackError::InvalidEmulation`] for an invalid emulation.
    pub const fn check(&self) -> Result<(), FallbackError> {
        if !is_stated(self.provenance) {
            return Err(FallbackError::EmptyProvenance);
        }
        let mut i = 0;
        while i < self.substitutes.len() {
            if let Err(error) = self.substitutes[i].check(self.articulation) {
                return Err(error);
            }
            let mut j = i + 1;
            while j < self.substitutes.len() {
                if same_stand_in(self.substitutes[i].stand_in, self.substitutes[j].stand_in) {
                    return Err(FallbackError::Repeated);
                }
                j += 1;
            }
            i += 1;
        }
        match self.emulation {
            Some(emulation) => emulation.check(),
            None => Ok(()),
        }
    }
}

/// The neighbours of `instrument`, best first; empty when no instrument comes near enough.
#[must_use]
pub fn neighbours(instrument: InstrumentType) -> &'static [Neighbour] {
    // The table holds each type's row at the type's place, which a compile-time check proves.
    NEIGHBOURS
        .get(instrument as usize)
        .map_or(&[], |row| row.neighbours)
}

/// The substitution row of `articulation`, or `None` for an articulation that has none: every
/// id but the fifteen original ones. A refinement falls back to its parent
/// ([`Articulation::parent`]) and then to the parent's row.
#[must_use]
pub fn substitution(articulation: Articulation) -> Option<&'static Substitution> {
    // Row `i` is the row of `Articulation::ALL[i]`, which a compile-time check proves.
    SUBSTITUTIONS
        .get(articulation as usize)
        .filter(|row| row.articulation == articulation)
}

/// The stand-ins of `articulation` in order of preference; empty when it has no row
/// ([`substitution`]).
#[must_use]
pub fn substitutes(articulation: Articulation) -> &'static [ArticulationSubstitute] {
    substitution(articulation).map_or(&[], |row| row.substitutes)
}

/// The playable range of `instrument`, or `None` when it is unpitched.
const fn playable(instrument: InstrumentType) -> Option<PitchRange> {
    // The dataset holds every type's record at the type's place in score order, which a
    // compile-time check in `data` proves, so the index is in bounds.
    match INSTRUMENTS[instrument as usize].pitch() {
        PitchModel::Pitched(pitched) => Some(pitched.playable.range),
        PitchModel::Unpitched => None,
    }
}

/// Whether the record of `instrument` lists `articulation`.
const fn lists(instrument: InstrumentType, articulation: Articulation) -> bool {
    let listed = INSTRUMENTS[instrument as usize].articulations();
    let mut i = 0;
    while i < listed.len() {
        if listed[i] as u8 == articulation as u8 {
            return true;
        }
        i += 1;
    }
    false
}

/// Whether two stand-ins are the same.
const fn same_stand_in(a: StandIn, b: StandIn) -> bool {
    match (a, b) {
        (StandIn::Articulation(a), StandIn::Articulation(b)) => a as u8 == b as u8,
        (StandIn::Instrument(a), StandIn::Instrument(b)) => a as u8 == b as u8,
        (StandIn::OtherEnsemble, StandIn::OtherEnsemble) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REASON: Provenance = Provenance::Estimate {
        reason: "a test reason",
    };

    const fn neighbour(instrument: InstrumentType) -> Neighbour {
        Neighbour {
            instrument,
            manner: Manner::AsWritten,
            only: None,
            asks_first: false,
            provenance: REASON,
        }
    }

    const fn plain(articulation: Articulation) -> ArticulationSubstitute {
        ArticulationSubstitute {
            stand_in: StandIn::Articulation(articulation),
            rewrite: None,
            vibrato: None,
            only: None,
        }
    }

    static TWICE_NEIGHBOURS: [Neighbour; 2] = [
        neighbour(InstrumentType::Viola),
        neighbour(InstrumentType::Viola),
    ];

    static OWN_NEIGHBOUR: [Neighbour; 1] = [neighbour(InstrumentType::Violin)];

    static STACCATO: [ArticulationSubstitute; 2] = [
        plain(Articulation::Spiccato),
        ArticulationSubstitute {
            stand_in: StandIn::Articulation(Articulation::Sustain),
            rewrite: Some(Emulation::Shorten { divisor: 4 }),
            vibrato: None,
            only: None,
        },
    ];

    static OWN_SUBSTITUTE: [ArticulationSubstitute; 1] = [plain(Articulation::Staccato)];

    static MUTE_SUBSTITUTE: [ArticulationSubstitute; 1] = [plain(Articulation::MuteCup)];

    static TWICE_SUBSTITUTES: [ArticulationSubstitute; 2] = [
        ArticulationSubstitute {
            stand_in: StandIn::OtherEnsemble,
            rewrite: None,
            vibrato: None,
            only: None,
        },
        ArticulationSubstitute {
            stand_in: StandIn::OtherEnsemble,
            rewrite: Some(Emulation::Soften),
            vibrato: None,
            only: None,
        },
    ];

    static BAD_REWRITE: [ArticulationSubstitute; 1] = [ArticulationSubstitute {
        stand_in: StandIn::Articulation(Articulation::Sustain),
        rewrite: Some(Emulation::Alternate {
            slowest: 0,
            fastest: 0,
        }),
        vibrato: None,
        only: None,
    }];

    #[test]
    fn neighbours_are_checked_against_the_instrument() {
        // TST-03: each rule of `Neighbour::check` rejects a fixture that breaks only it.
        assert_eq!(
            neighbour(InstrumentType::Viola).check(InstrumentType::Violin),
            Ok(())
        );
        assert_eq!(
            neighbour(InstrumentType::Violin).check(InstrumentType::Violin),
            Err(FallbackError::OwnNeighbour)
        );
        let blank = Neighbour {
            provenance: Provenance::Estimate { reason: " " },
            ..neighbour(InstrumentType::Viola)
        };
        assert_eq!(
            blank.check(InstrumentType::Violin),
            Err(FallbackError::EmptyProvenance)
        );
        // The harp is plucked, the piano a keyboard: another family must ask first.
        let piano = neighbour(InstrumentType::PianoConcertGrand);
        assert_eq!(
            piano.check(InstrumentType::Harp),
            Err(FallbackError::FamilyChangeUnasked)
        );
        let asking = Neighbour {
            asks_first: true,
            ..piano
        };
        assert_eq!(asking.check(InstrumentType::Harp), Ok(()));
        // Timpani are pitched, the bass drum is not: the pitch is lost unless the user agrees.
        let drum = neighbour(InstrumentType::BassDrum);
        assert_eq!(
            drum.check(InstrumentType::Timpani),
            Err(FallbackError::PitchLostUnasked)
        );
        // An unpitched triangle part on the glockenspiel needs the note it plays.
        let glockenspiel = neighbour(InstrumentType::Glockenspiel);
        assert_eq!(
            glockenspiel.check(InstrumentType::Triangle),
            Err(FallbackError::NoteUnnamed)
        );
        let on_top = Neighbour {
            manner: Manner::OnHighestNote,
            ..glockenspiel
        };
        assert_eq!(on_top.check(InstrumentType::Triangle), Ok(()));
        // A pitched part names no note, and an unpitched neighbour has none to name.
        assert_eq!(
            on_top.check(InstrumentType::Xylophone),
            Err(FallbackError::InvalidManner)
        );
        let on_drum = Neighbour {
            manner: Manner::OnLowestNote,
            ..neighbour(InstrumentType::SnareDrum)
        };
        assert_eq!(
            on_drum.check(InstrumentType::BassDrum),
            Err(FallbackError::InvalidManner)
        );
        // The piccolo (74-108) and the double bass (23-67) share no note.
        let wide_apart = Neighbour {
            asks_first: true,
            ..neighbour(InstrumentType::DoubleBass)
        };
        assert_eq!(
            wide_apart.check(InstrumentType::Piccolo),
            Err(FallbackError::NoSharedRange)
        );
    }

    #[test]
    fn a_manner_articulation_is_one_the_neighbour_plays() {
        let roll = Neighbour {
            manner: Manner::With(Articulation::Roll),
            ..neighbour(InstrumentType::SuspendedCymbal)
        };
        assert_eq!(roll.check(InstrumentType::TamTam), Ok(()));
        // The suspended cymbal's record does not list pizzicato.
        let pizzicato = Neighbour {
            manner: Manner::With(Articulation::Pizzicato),
            ..roll
        };
        assert_eq!(
            pizzicato.check(InstrumentType::TamTam),
            Err(FallbackError::InvalidManner)
        );
        // A mute is a passage state, never a note's articulation.
        let muted = Neighbour {
            manner: Manner::With(Articulation::ConSordino),
            ..neighbour(InstrumentType::Viola)
        };
        assert_eq!(
            muted.check(InstrumentType::Violin),
            Err(FallbackError::InvalidManner)
        );
    }

    #[test]
    fn a_list_names_each_neighbour_once() {
        let list = NeighbourList {
            instrument: InstrumentType::Violin,
            neighbours: &TWICE_NEIGHBOURS,
        };
        assert_eq!(list.check(), Err(FallbackError::Repeated));
        let own = NeighbourList {
            instrument: InstrumentType::Violin,
            neighbours: &OWN_NEIGHBOUR,
        };
        assert_eq!(own.check(), Err(FallbackError::OwnNeighbour));
        let empty = NeighbourList {
            instrument: InstrumentType::Violin,
            neighbours: &[],
        };
        assert_eq!(empty.check(), Ok(()));
    }

    #[test]
    fn emulations_reject_zero_and_inverted_numbers() {
        assert_eq!(Emulation::Shorten { divisor: 4 }.check(), Ok(()));
        assert_eq!(
            Emulation::Shorten { divisor: 1 }.check(),
            Err(FallbackError::InvalidEmulation)
        );
        assert_eq!(Emulation::Repeat { per_whole_note: 32 }.check(), Ok(()));
        for per_whole_note in [0, 2, 12] {
            assert_eq!(
                Emulation::Repeat { per_whole_note }.check(),
                Err(FallbackError::InvalidEmulation),
                "{per_whole_note}"
            );
        }
        assert_eq!(
            Emulation::Alternate {
                slowest: 12,
                fastest: 16
            }
            .check(),
            Ok(())
        );
        for (slowest, fastest) in [(0, 16), (16, 12)] {
            assert_eq!(
                Emulation::Alternate { slowest, fastest }.check(),
                Err(FallbackError::InvalidEmulation),
                "{slowest}-{fastest}"
            );
        }
    }

    #[test]
    fn substitutions_are_checked() {
        let row = Substitution {
            articulation: Articulation::Staccato,
            substitutes: &STACCATO,
            emulation: Some(Emulation::Shorten { divisor: 4 }),
            provenance: REASON,
        };
        assert_eq!(row.check(), Ok(()));
        assert_eq!(
            Substitution {
                provenance: Provenance::Cited { source: "" },
                ..row
            }
            .check(),
            Err(FallbackError::EmptyProvenance)
        );
        assert_eq!(
            Substitution {
                emulation: Some(Emulation::Shorten { divisor: 0 }),
                ..row
            }
            .check(),
            Err(FallbackError::InvalidEmulation)
        );
        assert_eq!(
            Substitution {
                substitutes: &OWN_SUBSTITUTE,
                ..row
            }
            .check(),
            Err(FallbackError::OwnSubstitute)
        );
        assert_eq!(
            Substitution {
                substitutes: &MUTE_SUBSTITUTE,
                ..row
            }
            .check(),
            Err(FallbackError::NotPrimary)
        );
        assert_eq!(
            Substitution {
                substitutes: &TWICE_SUBSTITUTES,
                ..row
            }
            .check(),
            Err(FallbackError::Repeated)
        );
        assert_eq!(
            Substitution {
                substitutes: &BAD_REWRITE,
                ..row
            }
            .check(),
            Err(FallbackError::InvalidEmulation)
        );
    }
}
