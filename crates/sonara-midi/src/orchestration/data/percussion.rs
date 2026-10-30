//! Percussion records.

use super::{ARTICULATION_ESTIMATE, estimate, pitched};
use crate::orchestration::articulation::Articulation::{self, Marcato, Roll, Staccato, Tremolo};
use crate::orchestration::id::InstrumentType;
use crate::orchestration::instrument::{Instrument, PitchModel};

/// The articulations a mallet instrument idiomatically plays.
const MALLETS: &[Articulation] = &[Staccato, Marcato, Tremolo];
/// The articulations a snare drum idiomatically plays.
const SNARE: &[Articulation] = &[Marcato, Roll];

pub(super) const SNARE_DRUM: Instrument = Instrument::new(
    InstrumentType::SnareDrum,
    "Snare drum",
    PitchModel::Unpitched,
    SNARE,
    ARTICULATION_ESTIMATE,
);

pub(super) const GLOCKENSPIEL: Instrument = Instrument::new(
    InstrumentType::Glockenspiel,
    "Glockenspiel",
    pitched(
        24,
        estimate(
            79,
            108,
            "sounds two octaves above written (https://en.wikipedia.org/wiki/Glockenspiel); the range assumes written G3-C6 from general practice, which the page does not state in text",
        ),
        None,
    ),
    MALLETS,
    ARTICULATION_ESTIMATE,
);

pub(super) const XYLOPHONE: Instrument = Instrument::new(
    InstrumentType::Xylophone,
    "Xylophone",
    pitched(
        12,
        estimate(
            65,
            108,
            "sounds an octave above written (https://en.wikipedia.org/wiki/Xylophone); the range assumes written F3-C7 from general practice, which the page does not state in text",
        ),
        None,
    ),
    MALLETS,
    ARTICULATION_ESTIMATE,
);
