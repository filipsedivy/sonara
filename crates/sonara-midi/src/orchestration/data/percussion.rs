//! Percussion records.

use super::{ARTICULATION_ESTIMATE, estimate, pitched};
use crate::orchestration::articulation::Articulation::{self, Marcato, Roll, Staccato, Tremolo};
use crate::orchestration::id::InstrumentType;
use crate::orchestration::instrument::{Instrument, InstrumentSpec, PitchModel};

/// The articulations a mallet instrument idiomatically plays.
const MALLETS: &[Articulation] = &[Staccato, Marcato, Tremolo];
/// The articulations a snare drum idiomatically plays.
const SNARE: &[Articulation] = &[Marcato, Roll];

pub(super) const SNARE_DRUM: Instrument = Instrument::new(
    InstrumentType::SnareDrum,
    InstrumentSpec {
        name: "Snare drum",
        pitch: PitchModel::Unpitched,
        articulations: SNARE,
        articulations_provenance: ARTICULATION_ESTIMATE,
        ..InstrumentSpec::EMPTY
    },
);

pub(super) const GLOCKENSPIEL: Instrument = Instrument::new(
    InstrumentType::Glockenspiel,
    InstrumentSpec {
        name: "Glockenspiel",
        pitch: pitched(
            24,
            estimate(
                79,
                108,
                "sounds two octaves above written (https://en.wikipedia.org/wiki/Glockenspiel); the range assumes written G3-C6 from general practice, which the page does not state in text",
            ),
            None,
        ),
        articulations: MALLETS,
        articulations_provenance: ARTICULATION_ESTIMATE,
        ..InstrumentSpec::EMPTY
    },
);

pub(super) const XYLOPHONE: Instrument = Instrument::new(
    InstrumentType::Xylophone,
    InstrumentSpec {
        name: "Xylophone",
        pitch: pitched(
            12,
            estimate(
                65,
                108,
                "sounds an octave above written (https://en.wikipedia.org/wiki/Xylophone); the range assumes written F3-C7 from general practice, which the page does not state in text",
            ),
            None,
        ),
        articulations: MALLETS,
        articulations_provenance: ARTICULATION_ESTIMATE,
        ..InstrumentSpec::EMPTY
    },
);
