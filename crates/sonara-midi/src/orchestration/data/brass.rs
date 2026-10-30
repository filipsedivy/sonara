//! Brass records.

use super::{ARTICULATION_ESTIMATE, cited, estimate, pitched};
use crate::orchestration::articulation::Articulation::{
    self, ConSordino, FlutterTongue, Legato, Marcato, Staccato, Sustain, Tenuto, Trill,
};
use crate::orchestration::id::InstrumentType;
use crate::orchestration::instrument::Instrument;

/// The articulations a brass instrument idiomatically plays.
const BRASS: &[Articulation] = &[
    Sustain,
    Legato,
    Staccato,
    Tenuto,
    Marcato,
    Trill,
    FlutterTongue,
    ConSordino,
];

pub(super) const HORN_F: Instrument = Instrument::new(
    InstrumentType::HornF,
    "Horn in F",
    pitched(
        -7,
        estimate(
            35,
            77,
            "sounds a fifth below written (https://en.wikipedia.org/wiki/French_horn); the range assumes written F#2-C6 from general practice, which the page does not state in text",
        ),
        None,
    ),
    BRASS,
    ARTICULATION_ESTIMATE,
);

pub(super) const TRUMPET_BB: Instrument = Instrument::new(
    InstrumentType::TrumpetBb,
    "Trumpet in Bb",
    pitched(
        -2,
        cited(
            52,
            82,
            "https://en.wikipedia.org/wiki/Trumpet: standard written range F#3-C6, sounds a whole step lower",
        ),
        None,
    ),
    BRASS,
    ARTICULATION_ESTIMATE,
);

pub(super) const TROMBONE_TENOR: Instrument = Instrument::new(
    InstrumentType::TromboneTenor,
    "Tenor trombone",
    pitched(
        0,
        cited(
            40,
            77,
            "https://en.wikipedia.org/wiki/Trombone: lowest E2 (no pedal notes), practical top F5",
        ),
        Some(cited(
            40,
            74,
            "https://en.wikipedia.org/wiki/Trombone: more conservative top D5",
        )),
    ),
    BRASS,
    ARTICULATION_ESTIMATE,
);
