//! The instrument record: family, pitch capabilities with provenance, and idiomatic articulations.

use thiserror::Error;

use super::articulation::{Articulation, ArticulationAvailability};
use super::id::InstrumentType;
use super::pitch::{PitchRange, SoundingPitch, Transposition, WrittenPitch};

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
    /// Stated by a source that was read; `source` names it (URL and what it states).
    Cited {
        /// The source.
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
    /// The written endpoints of the playable range do not fit MIDI notes.
    #[error("the transposition moves the playable range outside MIDI notes 0-127")]
    WrittenOutOfMidi,
    /// An articulation is listed twice.
    #[error("an articulation is listed twice")]
    DuplicateArticulation,
}

/// Metadata of one instrument type, such as `violin` or `clarinet_bb`.
///
/// The record is keyed by its [`InstrumentType`], which gives its id and family. The type names
/// the acoustic instrument, not a sample preset: it is independent of the preset ids of the
/// instrument catalog, which may map to it later. The data does not depend on any audio engine,
/// library or UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instrument {
    kind: InstrumentType,
    name: &'static str,
    pitch: PitchModel,
    articulations: &'static [Articulation],
    articulations_provenance: Provenance,
}

impl Instrument {
    /// Builds the record after checking its invariants.
    ///
    /// # Errors
    /// [`InstrumentError`] for a comfortable range outside the playable one, a transposition that
    /// moves the playable range outside MIDI, or a duplicate articulation.
    pub const fn try_new(
        instrument_type: InstrumentType,
        name: &'static str,
        pitch: PitchModel,
        articulations: &'static [Articulation],
        articulations_provenance: Provenance,
    ) -> Result<Self, InstrumentError> {
        if let PitchModel::Pitched(pitched) = &pitch {
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
        Ok(Self {
            kind: instrument_type,
            name,
            pitch,
            articulations,
            articulations_provenance,
        })
    }

    /// Like [`Instrument::try_new`]; invalid data in a `const` item is a compile error.
    ///
    /// # Panics
    /// When [`Instrument::try_new`] would fail.
    #[must_use]
    pub const fn new(
        instrument_type: InstrumentType,
        name: &'static str,
        pitch: PitchModel,
        articulations: &'static [Articulation],
        articulations_provenance: Provenance,
    ) -> Self {
        match Self::try_new(
            instrument_type,
            name,
            pitch,
            articulations,
            articulations_provenance,
        ) {
            Ok(instrument) => instrument,
            Err(_) => panic!("invalid instrument metadata"),
        }
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
        self.name
    }

    /// The family, that of [`Instrument::instrument_type`].
    #[must_use]
    pub const fn family(&self) -> Family {
        self.kind.family()
    }

    /// The pitch capabilities.
    #[must_use]
    pub const fn pitch(&self) -> &PitchModel {
        &self.pitch
    }

    /// The articulations the instrument idiomatically plays (general capability, not any
    /// library's).
    #[must_use]
    pub const fn articulations(&self) -> &'static [Articulation] {
        self.articulations
    }

    /// The provenance of [`Instrument::articulations`].
    #[must_use]
    pub const fn articulations_provenance(&self) -> Provenance {
        self.articulations_provenance
    }

    /// The transposition, or `None` for an unpitched instrument.
    #[must_use]
    pub const fn transposition(&self) -> Option<Transposition> {
        match &self.pitch {
            PitchModel::Pitched(pitched) => Some(pitched.transposition),
            PitchModel::Unpitched => None,
        }
    }

    /// The playable range as written pitches, derived from the sounding range; `None` for an
    /// unpitched instrument.
    #[must_use]
    pub fn written_playable(&self) -> Option<(WrittenPitch, WrittenPitch)> {
        let PitchModel::Pitched(pitched) = &self.pitch else {
            return None;
        };
        let range = pitched.playable.range;
        Some((
            pitched.transposition.to_written(range.low())?,
            pitched.transposition.to_written(range.high())?,
        ))
    }

    /// Where the sounding `pitch` lies relative to the instrument's ranges.
    ///
    /// # Errors
    /// [`CheckError::Unpitched`] for an unpitched instrument.
    pub fn check_sounding(&self, pitch: SoundingPitch) -> Result<RangeCheck, CheckError> {
        let PitchModel::Pitched(pitched) = &self.pitch else {
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
        let PitchModel::Pitched(pitched) = &self.pitch else {
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
        } else if self.articulations.contains(&articulation) {
            ArticulationAvailability::IdiomaticUnverified
        } else {
            ArticulationAvailability::NotIdiomatic
        }
    }
}
