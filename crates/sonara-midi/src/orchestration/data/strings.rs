//! Bowed string records.

use super::sources::cite;
use super::{ARTICULATION_ESTIMATE, cited, estimate, pitched};
use crate::orchestration::articulation::Articulation::{
    self, ColLegno, ConSordino, Harmonics, Legato, Marcato, Pizzicato, Spiccato, Staccato,
    SulPonticello, Sustain, Tenuto, Tremolo, Trill,
};
use crate::orchestration::id::InstrumentType;
use crate::orchestration::instrument::{Instrument, InstrumentSpec};
use crate::orchestration::pitch::Clef;

/// The articulations a bowed string instrument idiomatically plays.
const BOWED: &[Articulation] = &[
    Sustain,
    Legato,
    Staccato,
    Spiccato,
    Tenuto,
    Marcato,
    Pizzicato,
    Tremolo,
    Trill,
    ColLegno,
    Harmonics,
    SulPonticello,
    ConSordino,
];

const WIKI_VIOLIN: &str =
    cite!(wikipedia_violin: "lowest G3; E7 the limit for orchestral parts; up to C8 by stopping");

pub(super) const VIOLIN: Instrument = Instrument::new(
    InstrumentType::Violin,
    InstrumentSpec {
        name: "Violin",
        pitch: pitched(
            0,
            cited(55, 108, WIKI_VIOLIN),
            Some(cited(55, 100, WIKI_VIOLIN)),
        ),
        clefs: &[Clef::Treble],
        articulations: BOWED,
        articulations_provenance: ARTICULATION_ESTIMATE,
        ..InstrumentSpec::EMPTY
    },
);

pub(super) const CELLO: Instrument = Instrument::new(
    InstrumentType::Cello,
    InstrumentSpec {
        name: "Cello",
        pitch: pitched(
            0,
            estimate(
                36,
                93,
                "lowest C2 is the open C string (https://en.wikipedia.org/wiki/Cello); the upper limit is not stated by a source that was read",
            ),
            None,
        ),
        clefs: &[Clef::Bass, Clef::Tenor, Clef::Treble],
        articulations: BOWED,
        articulations_provenance: ARTICULATION_ESTIMATE,
        ..InstrumentSpec::EMPTY
    },
);

pub(super) const DOUBLE_BASS: Instrument = Instrument::new(
    InstrumentType::DoubleBass,
    InstrumentSpec {
        name: "Double bass",
        pitch: pitched(
            -12,
            estimate(
                24,
                67,
                "lowest C1 with a C extension and E1-G3 orchestral parts are from https://en.wikipedia.org/wiki/Double_bass; the upper limit is not stated by a source that was read",
            ),
            Some(cited(
                28,
                55,
                cite!(wikipedia_double_bass: "orchestral parts rarely exceed E1-G3 (sounding)"),
            )),
        ),
        clefs: &[Clef::Bass, Clef::Tenor, Clef::Treble],
        articulations: BOWED,
        articulations_provenance: ARTICULATION_ESTIMATE,
        ..InstrumentSpec::EMPTY
    },
);
