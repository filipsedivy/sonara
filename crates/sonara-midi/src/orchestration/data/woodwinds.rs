//! Woodwind records.

use super::{ARTICULATION_ESTIMATE, cited, pitched};
use crate::orchestration::articulation::Articulation::{
    self, FlutterTongue, Legato, Marcato, Staccato, Sustain, Tenuto, Trill,
};
use crate::orchestration::id::InstrumentType;
use crate::orchestration::instrument::{Instrument, InstrumentSpec};
use crate::orchestration::pitch::Clef;

/// The articulations a woodwind idiomatically plays.
const WIND: &[Articulation] = &[
    Sustain,
    Legato,
    Staccato,
    Tenuto,
    Marcato,
    Trill,
    FlutterTongue,
];

const WIKI_CLARINET: &str =
    "https://en.wikipedia.org/wiki/Clarinet: Bb clarinet written E3-C7, sounding D3-Bb6";

pub(super) const PICCOLO: Instrument = Instrument::new(
    InstrumentType::Piccolo,
    InstrumentSpec {
        name: "Piccolo",
        pitch: pitched(
            12,
            cited(
                74,
                108,
                "https://en.wikipedia.org/wiki/Piccolo: tessitura D5-C8 (sounding, an octave above written); some instruments reach C5",
            ),
            None,
        ),
        clefs: &[Clef::Treble],
        articulations: WIND,
        articulations_provenance: ARTICULATION_ESTIMATE,
        ..InstrumentSpec::EMPTY
    },
);

pub(super) const FLUTE: Instrument = Instrument::new(
    InstrumentType::Flute,
    InstrumentSpec {
        name: "Flute",
        pitch: pitched(
            0,
            cited(
                59,
                96,
                "https://en.wikipedia.org/wiki/Flute: three octaves from middle C, a semitone lower with a B foot",
            ),
            None,
        ),
        clefs: &[Clef::Treble],
        articulations: WIND,
        articulations_provenance: ARTICULATION_ESTIMATE,
        ..InstrumentSpec::EMPTY
    },
);

pub(super) const OBOE: Instrument = Instrument::new(
    InstrumentType::Oboe,
    InstrumentSpec {
        name: "Oboe",
        pitch: pitched(
            0,
            cited(
                58,
                91,
                "https://en.wikipedia.org/wiki/Oboe: commonly accepted range Bb3 to about G6",
            ),
            None,
        ),
        clefs: &[Clef::Treble],
        articulations: WIND,
        articulations_provenance: ARTICULATION_ESTIMATE,
        ..InstrumentSpec::EMPTY
    },
);

pub(super) const ENGLISH_HORN: Instrument = Instrument::new(
    InstrumentType::EnglishHorn,
    InstrumentSpec {
        name: "English horn",
        pitch: pitched(
            -7,
            cited(
                52,
                84,
                "https://en.wikipedia.org/wiki/Cor_anglais: pitched in F, a fifth below the oboe; sounding range E3 to C6",
            ),
            None,
        ),
        clefs: &[Clef::Treble],
        articulations: WIND,
        articulations_provenance: ARTICULATION_ESTIMATE,
        ..InstrumentSpec::EMPTY
    },
);

pub(super) const CLARINET_BB: Instrument = Instrument::new(
    InstrumentType::ClarinetBb,
    InstrumentSpec {
        name: "Clarinet in Bb",
        pitch: pitched(-2, cited(50, 94, WIKI_CLARINET), None),
        clefs: &[Clef::Treble],
        articulations: WIND,
        articulations_provenance: ARTICULATION_ESTIMATE,
        ..InstrumentSpec::EMPTY
    },
);

pub(super) const BASSOON: Instrument = Instrument::new(
    InstrumentType::Bassoon,
    InstrumentSpec {
        name: "Bassoon",
        pitch: pitched(
            0,
            cited(
                34,
                80,
                "https://en.wikipedia.org/wiki/Bassoon: infobox playing range Bb1-E5, Ab5 in parentheses",
            ),
            Some(cited(
                34,
                76,
                "https://en.wikipedia.org/wiki/Bassoon: infobox playing range Bb1-E5",
            )),
        ),
        clefs: &[Clef::Bass, Clef::Tenor, Clef::Treble],
        articulations: WIND,
        articulations_provenance: ARTICULATION_ESTIMATE,
        ..InstrumentSpec::EMPTY
    },
);

pub(super) const CONTRABASSOON: Instrument = Instrument::new(
    InstrumentType::Contrabassoon,
    InstrumentSpec {
        name: "Contrabassoon",
        pitch: pitched(
            -12,
            cited(
                22,
                62,
                "https://en.wikipedia.org/wiki/Contrabassoon: sounding range from Bb0 (A0 on some) to D4; notated an octave above sounding",
            ),
            None,
        ),
        clefs: &[Clef::Bass, Clef::Tenor],
        articulations: WIND,
        articulations_provenance: ARTICULATION_ESTIMATE,
        ..InstrumentSpec::EMPTY
    },
);
