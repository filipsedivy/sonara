//! The shipped dataset: a representative set of orchestral instruments.
//!
//! Ranges are sounding MIDI notes (middle C = 60). Each range says whether a source that was read
//! states it (`Cited`) or not (`Estimate`). Sources were read on 2026-10-28; Wikipedia is used for
//! facts only (see `docs/orchestration-metadata.md`).

use super::articulation::Articulation::{
    self, ColLegno, ConSordino, FlutterTongue, Harmonics, Legato, Marcato, Pizzicato, Roll,
    Spiccato, Staccato, SulPonticello, Sustain, Tenuto, Tremolo, Trill,
};
use super::instrument::{Family, Instrument, PitchModel, Pitched, Provenance, SourcedRange};
use super::pitch::{PitchRange, Transposition};

const ARTICULATION_ESTIMATE: Provenance = Provenance::Estimate {
    reason: "general orchestration practice; no source was read for the articulation list",
};

const fn cited(low: i64, high: i64, source: &'static str) -> SourcedRange {
    SourcedRange {
        range: PitchRange::new(low, high),
        provenance: Provenance::Cited { source },
    }
}

const fn estimate(low: i64, high: i64, reason: &'static str) -> SourcedRange {
    SourcedRange {
        range: PitchRange::new(low, high),
        provenance: Provenance::Estimate { reason },
    }
}

const fn pitched(
    semitones: i8,
    playable: SourcedRange,
    comfortable: Option<SourcedRange>,
) -> PitchModel {
    PitchModel::Pitched(Pitched {
        transposition: Transposition::new(semitones),
        playable,
        comfortable,
    })
}

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
const WIND: &[Articulation] = &[
    Sustain,
    Legato,
    Staccato,
    Tenuto,
    Marcato,
    Trill,
    FlutterTongue,
];
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
const MALLETS: &[Articulation] = &[Staccato, Marcato, Tremolo];
const SNARE: &[Articulation] = &[Marcato, Roll];

const WIKI_VIOLIN: &str = "https://en.wikipedia.org/wiki/Violin: lowest G3; E7 the limit for orchestral parts; up to C8 by stopping";
const WIKI_CLARINET: &str =
    "https://en.wikipedia.org/wiki/Clarinet: Bb clarinet written E3-C7, sounding D3-Bb6";

/// Every instrument of the dataset, in a fixed order.
pub static INSTRUMENTS: [Instrument; 16] = [
    Instrument::new(
        "violin",
        "Violin",
        Family::Strings,
        pitched(
            0,
            cited(55, 108, WIKI_VIOLIN),
            Some(cited(55, 100, WIKI_VIOLIN)),
        ),
        BOWED,
        ARTICULATION_ESTIMATE,
    ),
    Instrument::new(
        "cello",
        "Cello",
        Family::Strings,
        pitched(
            0,
            estimate(
                36,
                93,
                "lowest C2 is the open C string (https://en.wikipedia.org/wiki/Cello); the upper limit is not stated by a source that was read",
            ),
            None,
        ),
        BOWED,
        ARTICULATION_ESTIMATE,
    ),
    Instrument::new(
        "double_bass",
        "Double bass",
        Family::Strings,
        pitched(
            -12,
            estimate(
                24,
                67,
                "lowest C1 with a C extension and E1-G3 orchestral parts are from https://en.wikipedia.org/wiki/Double_bass; the upper limit is not stated by a source that was read",
            ),
            Some(cited(
                28,
                55,
                "https://en.wikipedia.org/wiki/Double_bass: orchestral parts rarely exceed E1-G3 (sounding)",
            )),
        ),
        BOWED,
        ARTICULATION_ESTIMATE,
    ),
    Instrument::new(
        "flute",
        "Flute",
        Family::Woodwinds,
        pitched(
            0,
            cited(
                59,
                96,
                "https://en.wikipedia.org/wiki/Flute: three octaves from middle C, a semitone lower with a B foot",
            ),
            None,
        ),
        WIND,
        ARTICULATION_ESTIMATE,
    ),
    Instrument::new(
        "piccolo",
        "Piccolo",
        Family::Woodwinds,
        pitched(
            12,
            cited(
                74,
                108,
                "https://en.wikipedia.org/wiki/Piccolo: tessitura D5-C8 (sounding, an octave above written); some instruments reach C5",
            ),
            None,
        ),
        WIND,
        ARTICULATION_ESTIMATE,
    ),
    Instrument::new(
        "oboe",
        "Oboe",
        Family::Woodwinds,
        pitched(
            0,
            cited(
                58,
                91,
                "https://en.wikipedia.org/wiki/Oboe: commonly accepted range Bb3 to about G6",
            ),
            None,
        ),
        WIND,
        ARTICULATION_ESTIMATE,
    ),
    Instrument::new(
        "english_horn",
        "English horn",
        Family::Woodwinds,
        pitched(
            -7,
            cited(
                52,
                84,
                "https://en.wikipedia.org/wiki/Cor_anglais: pitched in F, a fifth below the oboe; sounding range E3 to C6",
            ),
            None,
        ),
        WIND,
        ARTICULATION_ESTIMATE,
    ),
    Instrument::new(
        "clarinet_bb",
        "Clarinet in Bb",
        Family::Woodwinds,
        pitched(-2, cited(50, 94, WIKI_CLARINET), None),
        WIND,
        ARTICULATION_ESTIMATE,
    ),
    Instrument::new(
        "bassoon",
        "Bassoon",
        Family::Woodwinds,
        pitched(
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
        WIND,
        ARTICULATION_ESTIMATE,
    ),
    Instrument::new(
        "contrabassoon",
        "Contrabassoon",
        Family::Woodwinds,
        pitched(
            -12,
            cited(
                22,
                62,
                "https://en.wikipedia.org/wiki/Contrabassoon: sounding range from Bb0 (A0 on some) to D4; notated an octave above sounding",
            ),
            None,
        ),
        WIND,
        ARTICULATION_ESTIMATE,
    ),
    Instrument::new(
        "horn_f",
        "Horn in F",
        Family::Brass,
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
    ),
    Instrument::new(
        "trumpet_bb",
        "Trumpet in Bb",
        Family::Brass,
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
    ),
    Instrument::new(
        "trombone_tenor",
        "Tenor trombone",
        Family::Brass,
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
    ),
    Instrument::new(
        "glockenspiel",
        "Glockenspiel",
        Family::Percussion,
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
    ),
    Instrument::new(
        "xylophone",
        "Xylophone",
        Family::Percussion,
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
    ),
    Instrument::new(
        "snare_drum",
        "Snare drum",
        Family::Percussion,
        PitchModel::Unpitched,
        SNARE,
        ARTICULATION_ESTIMATE,
    ),
];
