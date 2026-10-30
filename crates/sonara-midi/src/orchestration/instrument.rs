//! The instrument record: family, pitch capabilities with provenance, and idiomatic articulations.

use thiserror::Error;

use super::articulation::{Articulation, ArticulationAvailability};
use super::ensemble_profile::{EnsembleProfile, Era};
use super::id::InstrumentType;
use super::pitch::{Clef, PitchRange, SoundingPitch, Transposition, WrittenPitch};

/// The instrument family. The slugs equal the family slugs of the instrument catalog
/// (`sonara_app::instruments`), so the two map by string. Voices, solo and choral, are `Choir`;
/// `Other` exists for that mapping, and no instrument type has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    /// Bowed strings.
    Strings,
    /// Woodwinds.
    Woodwinds,
    /// Brass.
    Brass,
    /// Percussion, pitched or not.
    Percussion,
    /// Keyboards.
    Keyboards,
    /// Plucked strings.
    Plucked,
    /// Voices.
    Choir,
    /// Anything else.
    Other,
}

impl Family {
    /// The slug, for example `woodwinds`.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Strings => "strings",
            Self::Woodwinds => "woodwinds",
            Self::Brass => "brass",
            Self::Percussion => "percussion",
            Self::Keyboards => "keyboards",
            Self::Plucked => "plucked",
            Self::Choir => "choir",
            Self::Other => "other",
        }
    }
}

/// Where a value comes from. Every range and every articulation list carries one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provenance {
    /// Stated by a source that was read; `source` names each source in full (author or
    /// publisher, title, and a URL or a page) and what it states.
    Cited {
        /// The sources and what they state.
        source: &'static str,
    },
    /// Not confirmed by a source that was read; `reason` says what the value rests on.
    Estimate {
        /// What the value rests on.
        reason: &'static str,
    },
}

/// A range of sounding pitches and where it comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourcedRange {
    /// The range.
    pub range: PitchRange,
    /// Its provenance.
    pub provenance: Provenance,
}

/// The pitch capabilities of a pitched instrument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pitched {
    /// Sounding minus written pitch.
    pub transposition: Transposition,
    /// The outer limits of what the instrument can play (sounding pitch).
    pub playable: SourcedRange,
    /// The range of ordinary orchestral writing, inside `playable`; `None` when unknown.
    pub comfortable: Option<SourcedRange>,
}

/// An instrument either has pitches or it does not; unpitched percussion has no range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PitchModel {
    /// Plays definite pitches.
    Pitched(Pitched),
    /// Plays no definite pitch (for example a snare drum).
    Unpitched,
}

/// Where a note lies relative to an instrument's ranges. Nothing is clamped or changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeCheck {
    /// Inside the comfortable range (or inside the playable range when no comfortable range is
    /// known).
    Within,
    /// Playable, but outside the comfortable range: a warning, not an error.
    OutsideComfortable,
    /// Below the lowest playable note.
    BelowPlayable,
    /// Above the highest playable note.
    AbovePlayable,
}

/// A range check that cannot give an answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CheckError {
    /// The instrument has no pitch.
    #[error("the instrument is unpitched and has no range")]
    Unpitched,
    /// The written pitch converts to a sounding pitch outside MIDI notes 0-127.
    #[error("written pitch {0} does not convert to a MIDI note")]
    ConversionOutOfRange(i64),
}

/// Why instrument metadata is invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum InstrumentError {
    /// The comfortable range is not inside the playable range.
    #[error("comfortable range is not inside the playable range")]
    ComfortableOutsidePlayable,
    /// The written endpoints of the playable range do not fit MIDI notes, with the
    /// transposition alone or in one of the listed clefs.
    #[error(
        "the playable range, written with the transposition or in a listed clef, falls outside MIDI notes 0-127"
    )]
    WrittenOutOfMidi,
    /// An articulation is listed twice.
    #[error("an articulation is listed twice")]
    DuplicateArticulation,
    /// A clef is listed twice.
    #[error("a clef is listed twice")]
    DuplicateClef,
    /// A pitched instrument lists a clef that shows no pitch: the percussion clef or a one-line
    /// staff.
    #[error("a pitched instrument lists a clef that shows no pitch")]
    PitchlessClef,
    /// A transposing instrument lists a clef with an octave of its own ([`Clef::octave`] not 0).
    /// The octave is stated once, in the clef or in the transposition: a tenor voice is at
    /// transposition 0 and takes its octave from `treble_8vb`, so a −12 transposition as well
    /// would write it two octaves above the sound.
    #[error("a transposing instrument lists a clef with an octave of its own")]
    OctaveClefWithTransposition,
    /// The display name is empty or blank.
    #[error("the display name is empty or blank")]
    EmptyName,
    /// A provenance is a `Cited` source or an `Estimate` reason that is empty or blank.
    #[error("a cited source or an estimate reason is empty or blank")]
    EmptyProvenance,
    /// An instrument outside the percussion family has no pitch. Every wind, brass, keyboard,
    /// plucked, voice and string instrument plays definite pitches.
    #[error("only percussion instruments may be unpitched")]
    UnpitchedOutsidePercussion,
    /// A player range has more players at its low end than at its high end.
    #[error("player range {min}-{max} is inverted")]
    InvertedPlayerRange {
        /// The low end.
        min: u8,
        /// The high end.
        max: u8,
    },
}

/// The values of an instrument record, checked by [`Instrument::try_new`].
///
/// A record is written as a literal that ends in `..InstrumentSpec::EMPTY`, so a field added
/// later gets its default without editing every record. [`InstrumentSpec::EMPTY`] is not a valid
/// record: see there which fields a record must state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstrumentSpec {
    /// The English display name. Not an identifier.
    pub name: &'static str,
    /// The pitch capabilities.
    pub pitch: PitchModel,
    /// The clefs the instrument's parts are written in, in order of use (the usual one first),
    /// each at most once; empty when not recorded.
    pub clefs: &'static [Clef],
    /// The articulations the instrument idiomatically plays (general capability, not any
    /// library's), each at most once.
    pub articulations: &'static [Articulation],
    /// The provenance of `articulations`.
    pub articulations_provenance: Provenance,
    /// How the instrument appears in ensembles; `None` when not recorded.
    pub ensemble: Option<EnsembleProfile>,
}

impl InstrumentSpec {
    /// The base of every record literal.
    ///
    /// A record must state `name` and `articulations_provenance`: here they are empty, which
    /// [`Instrument::try_new`] rejects, so a record that forgets one does not build. `pitch` is
    /// [`PitchModel::Unpitched`], which only percussion may keep; a pitched percussion instrument
    /// states its pitch. The other fields hold their "not known" value: no clefs, no
    /// articulations and no ensemble profile.
    pub const EMPTY: Self = Self {
        name: "",
        pitch: PitchModel::Unpitched,
        clefs: &[],
        articulations: &[],
        articulations_provenance: Provenance::Estimate { reason: "" },
        ensemble: None,
    };
}

/// Metadata of one instrument type, such as `violin` or `clarinet_bb`.
///
/// The record is keyed by its [`InstrumentType`], which gives its id and family, and holds the
/// checked [`InstrumentSpec`]. The type names the acoustic instrument, not a sample preset: it is
/// independent of the preset ids of the instrument catalog, which may map to it later. The data
/// does not depend on any audio engine, library or UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instrument {
    kind: InstrumentType,
    spec: InstrumentSpec,
}

impl Instrument {
    /// Builds the record of `instrument_type` after checking the invariants of `spec`.
    ///
    /// # Errors
    /// [`InstrumentError`]: [`EmptyName`](InstrumentError::EmptyName) for an empty or blank
    /// name; [`UnpitchedOutsidePercussion`](InstrumentError::UnpitchedOutsidePercussion) for an
    /// unpitched instrument outside the percussion family;
    /// [`ComfortableOutsidePlayable`](InstrumentError::ComfortableOutsidePlayable) and
    /// [`WrittenOutOfMidi`](InstrumentError::WrittenOutOfMidi) for ranges that do not fit;
    /// [`DuplicateArticulation`](InstrumentError::DuplicateArticulation) for an articulation
    /// listed twice; [`DuplicateClef`](InstrumentError::DuplicateClef) for a clef listed twice;
    /// then, for a pitched instrument and clef by clef in list order,
    /// [`PitchlessClef`](InstrumentError::PitchlessClef) for a clef that shows no pitch,
    /// [`OctaveClefWithTransposition`](InstrumentError::OctaveClefWithTransposition) for a clef
    /// with an octave of its own on a transposing instrument and
    /// [`WrittenOutOfMidi`](InstrumentError::WrittenOutOfMidi) when the playable range written
    /// in the clef leaves MIDI; [`EmptyProvenance`](InstrumentError::EmptyProvenance) when any
    /// provenance of the record (ranges, articulations, ensemble profile) is empty or blank. The
    /// first failing check in that order is reported.
    pub const fn try_new(
        instrument_type: InstrumentType,
        spec: InstrumentSpec,
    ) -> Result<Self, InstrumentError> {
        if spec.name.trim_ascii().is_empty() {
            return Err(InstrumentError::EmptyName);
        }
        match &spec.pitch {
            PitchModel::Pitched(pitched) => {
                if let Some(comfortable) = &pitched.comfortable
                    && !pitched.playable.range.encloses(comfortable.range)
                {
                    return Err(InstrumentError::ComfortableOutsidePlayable);
                }
                let range = pitched.playable.range;
                if pitched.transposition.to_written(range.low()).is_none()
                    || pitched.transposition.to_written(range.high()).is_none()
                {
                    return Err(InstrumentError::WrittenOutOfMidi);
                }
            }
            PitchModel::Unpitched => {
                if !matches!(instrument_type.family(), Family::Percussion) {
                    return Err(InstrumentError::UnpitchedOutsidePercussion);
                }
            }
        }
        let articulations = spec.articulations;
        let mut i = 0;
        while i < articulations.len() {
            let mut j = i + 1;
            while j < articulations.len() {
                if articulations[i] as u8 == articulations[j] as u8 {
                    return Err(InstrumentError::DuplicateArticulation);
                }
                j += 1;
            }
            i += 1;
        }
        if let Err(error) = check_clefs(&spec) {
            return Err(error);
        }
        if !provenances_are_stated(&spec) {
            return Err(InstrumentError::EmptyProvenance);
        }
        Ok(Self {
            kind: instrument_type,
            spec,
        })
    }

    /// Like [`Instrument::try_new`]; invalid data in a `const` item is a compile error.
    ///
    /// # Panics
    /// When [`Instrument::try_new`] would fail.
    #[must_use]
    pub const fn new(instrument_type: InstrumentType, spec: InstrumentSpec) -> Self {
        match Self::try_new(instrument_type, spec) {
            Ok(instrument) => instrument,
            Err(_) => panic!("invalid instrument metadata"),
        }
    }

    /// The checked values of the record, as [`Instrument::try_new`] received them.
    #[must_use]
    pub const fn spec(&self) -> InstrumentSpec {
        self.spec
    }

    /// The instrument type the record describes.
    #[must_use]
    pub const fn instrument_type(&self) -> InstrumentType {
        self.kind
    }

    /// The stable identifier, the id of [`Instrument::instrument_type`].
    #[must_use]
    pub const fn id(&self) -> &'static str {
        self.kind.id()
    }

    /// The English display name. Not an identifier.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.spec.name
    }

    /// The family, that of [`Instrument::instrument_type`].
    #[must_use]
    pub const fn family(&self) -> Family {
        self.kind.family()
    }

    /// The pitch capabilities.
    #[must_use]
    pub const fn pitch(&self) -> &PitchModel {
        &self.spec.pitch
    }

    /// The articulations the instrument idiomatically plays (general capability, not any
    /// library's).
    #[must_use]
    pub const fn articulations(&self) -> &'static [Articulation] {
        self.spec.articulations
    }

    /// The provenance of [`Instrument::articulations`].
    #[must_use]
    pub const fn articulations_provenance(&self) -> Provenance {
        self.spec.articulations_provenance
    }

    /// How the instrument appears in ensembles, or `None` when not recorded.
    #[must_use]
    pub const fn ensemble(&self) -> Option<&EnsembleProfile> {
        self.spec.ensemble.as_ref()
    }

    /// The transposition, or `None` for an unpitched instrument.
    #[must_use]
    pub const fn transposition(&self) -> Option<Transposition> {
        match &self.spec.pitch {
            PitchModel::Pitched(pitched) => Some(pitched.transposition),
            PitchModel::Unpitched => None,
        }
    }

    /// The clefs the instrument's parts are written in, in order of use (the usual one first);
    /// empty when not recorded.
    #[must_use]
    pub const fn clefs(&self) -> &'static [Clef] {
        self.spec.clefs
    }

    /// The playable range as written pitches, derived from the sounding range with the
    /// transposition alone (no clef); `None` for an unpitched instrument.
    #[must_use]
    pub fn written_playable(&self) -> Option<(WrittenPitch, WrittenPitch)> {
        let PitchModel::Pitched(pitched) = &self.spec.pitch else {
            return None;
        };
        let range = pitched.playable.range;
        Some((
            pitched.transposition.to_written(range.low())?,
            pitched.transposition.to_written(range.high())?,
        ))
    }

    /// The pitch to write for the sounding `pitch` in `clef`: the instrument's transposition,
    /// then the clef's own octave ([`Clef::to_written`]). A tenor voice's sounding C3 is written
    /// C4 in [`Clef::Treble8vb`] and C3 in [`Clef::Bass`].
    ///
    /// The clef need not be one of [`Instrument::clefs`]: a part may be written in another, such
    /// as a tenor on a bass-clef staff shared with the basses. `None` for an unpitched
    /// instrument, for a clef that shows no pitch, and when the written pitch falls outside
    /// 0–127.
    #[must_use]
    pub const fn written_in(&self, clef: Clef, pitch: SoundingPitch) -> Option<WrittenPitch> {
        match &self.spec.pitch {
            PitchModel::Pitched(pitched) => clef.to_written(pitched.transposition, pitch),
            PitchModel::Unpitched => None,
        }
    }

    /// Where the sounding `pitch` lies relative to the instrument's ranges.
    ///
    /// # Errors
    /// [`CheckError::Unpitched`] for an unpitched instrument.
    pub fn check_sounding(&self, pitch: SoundingPitch) -> Result<RangeCheck, CheckError> {
        let PitchModel::Pitched(pitched) = &self.spec.pitch else {
            return Err(CheckError::Unpitched);
        };
        let playable = pitched.playable.range;
        Ok(if pitch < playable.low() {
            RangeCheck::BelowPlayable
        } else if pitch > playable.high() {
            RangeCheck::AbovePlayable
        } else if pitched
            .comfortable
            .is_some_and(|comfortable| !comfortable.range.contains(pitch))
        {
            RangeCheck::OutsideComfortable
        } else {
            RangeCheck::Within
        })
    }

    /// Where the written `pitch` lies, after converting it with the instrument's transposition.
    ///
    /// # Errors
    /// [`CheckError::Unpitched`] for an unpitched instrument; [`CheckError::ConversionOutOfRange`]
    /// when the converted pitch is outside MIDI notes 0-127.
    pub fn check_written(&self, pitch: WrittenPitch) -> Result<RangeCheck, CheckError> {
        let PitchModel::Pitched(pitched) = &self.spec.pitch else {
            return Err(CheckError::Unpitched);
        };
        let sounding = pitched
            .transposition
            .to_sounding(pitch)
            .ok_or(CheckError::ConversionOutOfRange(pitch.get()))?;
        self.check_sounding(sounding)
    }

    /// Whether the instrument can play articulation `id`. `library_declared` holds the
    /// articulation ids a sample library declared for it (empty when none or unverified); only
    /// those make the answer [`ArticulationAvailability::Supported`].
    #[must_use]
    pub fn articulation_availability(
        &self,
        id: &str,
        library_declared: &[&str],
    ) -> ArticulationAvailability {
        let Some(articulation) = Articulation::from_id(id) else {
            return ArticulationAvailability::UnknownArticulation;
        };
        if library_declared.contains(&id) {
            ArticulationAvailability::Supported
        } else if self.spec.articulations.contains(&articulation) {
            ArticulationAvailability::IdiomaticUnverified
        } else {
            ArticulationAvailability::NotIdiomatic
        }
    }
}

/// The clef checks of [`Instrument::try_new`], in its documented order: no clef twice; then, for
/// a pitched instrument and clef by clef, a clef that shows pitch, no octave in both the clef and
/// the transposition, and a playable range that stays inside MIDI when written in the clef.
const fn check_clefs(spec: &InstrumentSpec) -> Result<(), InstrumentError> {
    let clefs = spec.clefs;
    let mut i = 0;
    while i < clefs.len() {
        let mut j = i + 1;
        while j < clefs.len() {
            if clefs[i] as u8 == clefs[j] as u8 {
                return Err(InstrumentError::DuplicateClef);
            }
            j += 1;
        }
        i += 1;
    }
    let PitchModel::Pitched(pitched) = &spec.pitch else {
        // An unpitched instrument may be written on any staff (a snare drum on a treble staff).
        return Ok(());
    };
    let range = pitched.playable.range;
    let mut i = 0;
    while i < clefs.len() {
        let clef = clefs[i];
        if !clef.shows_pitch() {
            return Err(InstrumentError::PitchlessClef);
        }
        if clef.octave() != 0 && !pitched.transposition.is_none() {
            return Err(InstrumentError::OctaveClefWithTransposition);
        }
        if clef
            .to_written(pitched.transposition, range.low())
            .is_none()
            || clef
                .to_written(pitched.transposition, range.high())
                .is_none()
        {
            return Err(InstrumentError::WrittenOutOfMidi);
        }
        i += 1;
    }
    Ok(())
}

/// Whether every provenance of `spec` names a source or a reason: not empty, not blank.
const fn provenances_are_stated(spec: &InstrumentSpec) -> bool {
    if let PitchModel::Pitched(pitched) = &spec.pitch {
        if !is_stated(pitched.playable.provenance) {
            return false;
        }
        if let Some(comfortable) = &pitched.comfortable
            && !is_stated(comfortable.provenance)
        {
            return false;
        }
    }
    if !is_stated(spec.articulations_provenance) {
        return false;
    }
    if let Some(profile) = &spec.ensemble {
        let mut i = 0;
        while i < Era::ALL.len() {
            if !is_stated(profile.players(Era::ALL[i]).provenance) {
                return false;
            }
            i += 1;
        }
    }
    true
}

/// Whether `provenance` names a source or a reason.
const fn is_stated(provenance: Provenance) -> bool {
    match provenance {
        Provenance::Cited { source } => !source.trim_ascii().is_empty(),
        Provenance::Estimate { reason } => !reason.trim_ascii().is_empty(),
    }
}
