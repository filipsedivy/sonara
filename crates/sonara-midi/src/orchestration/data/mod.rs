//! The shipped dataset: one record for every instrument type.
//!
//! One file per family holds that family's records as named constants; [`INSTRUMENTS`] lists them
//! in score order. Ranges are sounding MIDI notes (middle C = 60). Each range, articulation list
//! and count of players says whether a source that was read states it (`Cited`) or not
//! (`Estimate`). A `Cited` value names its sources in full from the list in `sources`, each
//! followed by what it states; Wikipedia is used for facts only (see
//! `docs/orchestration-metadata.md`).
//!
//! Each record also holds its display names in five languages and its aliases, and the categories
//! and families hold the words that name them as a whole, in `categories`.
//!
//! Each record states its General MIDI voice. A GM1 program or key is exact when its GM1 name
//! names the instrument or its class; otherwise the record names the nearest program or keys as a
//! substitute, an `Estimate` whose reason cites the GM1 list it was read from and says why that
//! target is the nearest, or states that GM1 has no equivalent.

/// A list of aliases, written folded: `aliases![En Plural: "horns"; Cs Plural: "lesni rohy",
/// "lesnich rohu";]` gives each text with the language and the number before its colon. Defined
/// before the modules below, so they all see it.
macro_rules! aliases {
    ($($lang:ident $number:ident: $($text:literal),+;)+) => {
        &[$($($crate::orchestration::names::Alias {
            text: $text,
            lang: $crate::orchestration::names::Lang::$lang,
            number: $crate::orchestration::names::Number::$number,
        }),+),+]
    };
}

/// A list of GM1 channel-10 keys as a `&'static [GmNote]`, the preferred key first:
/// `gm_keys![81, 80]`. A key outside the GM1 percussion key map (35–81) does not compile.
macro_rules! gm_keys {
    ($($key:literal),+ $(,)?) => {{
        const KEYS: &[$crate::orchestration::general_midi::GmNote] =
            &[$($crate::orchestration::data::gm_key($key)),+];
        KEYS
    }};
}

mod brass;
mod categories;
mod ensembles;
mod keyboards;
mod percussion;
mod plucked;
pub(super) mod rules;
mod sources;
mod strings;
mod voices;
mod woodwinds;

pub(super) use categories::{FAMILY_ALIASES, category, family_aliases};
pub use ensembles::ENSEMBLES;
pub(super) use ensembles::{MEDIUM as MEDIUM_STRINGS, ensemble};
pub(super) use rules::rule_record;

use super::ensemble_profile::{EraPlayers, PlayerRange};
use super::general_midi::{GmNote, GmProgram, GmTarget, GmVoice, SubstituteFit};
use super::id::InstrumentType;
use super::instrument::{Instrument, PitchModel, Pitched, Provenance, SourcedRange};
use super::pitch::{PitchRange, Transposition};

/// The provenance of a count of players that no source states: most counts summarize how the
/// instrument is usually scored.
const PLAYERS_ESTIMATE: Provenance = Provenance::Estimate {
    reason: "the usual scoring of the instrument in that kind of score; no source that was read \
             states the count",
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

/// `min` to `max` players, with the qualifier `note`.
const fn players(min: u8, max: u8, note: &'static str, provenance: Provenance) -> EraPlayers {
    EraPlayers {
        players: Some(PlayerRange::new(min, max)),
        note,
        provenance,
    }
}

/// No players: the instrument is not used in that kind of score.
const fn not_used(note: &'static str, provenance: Provenance) -> EraPlayers {
    EraPlayers {
        players: Some(PlayerRange::NOT_USED),
        note,
        provenance,
    }
}

/// A use stated without a number of players, such as "rare".
const fn unstated(note: &'static str, provenance: Provenance) -> EraPlayers {
    EraPlayers {
        players: None,
        note,
        provenance,
    }
}

/// The GM1 program `number`; one outside 1–128 in a `const` item is a compile error.
const fn gm_program_number(number: u8) -> GmProgram {
    match GmProgram::new(number) {
        Some(program) => program,
        None => panic!("GM1 programs are numbered 1-128"),
    }
}

/// The GM1 channel-10 key `key`, for [`gm_keys!`]; one outside 35–81 in a `const` item is a
/// compile error.
const fn gm_key(key: u8) -> GmNote {
    match GmNote::new(key) {
        Some(note) => note,
        None => panic!("GM1 channel-10 keys are 35-81"),
    }
}

/// The exact GM1 program `number`: its name names the instrument or its class.
const fn gm_program(number: u8) -> GmVoice {
    GmVoice::Program(gm_program_number(number))
}

/// The GM1 program `number` as a substitute that fits as `fit`, chosen for `reason`.
const fn gm_substitute(number: u8, fit: SubstituteFit, reason: &'static str) -> GmVoice {
    GmVoice::Substitute {
        target: GmTarget::Program(gm_program_number(number)),
        fit,
        provenance: Provenance::Estimate { reason },
    }
}

/// The GM1 channel-10 `keys` as a substitute that fits as `fit`, chosen for `reason`.
const fn gm_substitute_keys(
    keys: &'static [GmNote],
    fit: SubstituteFit,
    reason: &'static str,
) -> GmVoice {
    GmVoice::Substitute {
        target: GmTarget::Notes(keys),
        fit,
        provenance: Provenance::Estimate { reason },
    }
}

/// Every instrument record of the dataset, one per type, in score order: the record of a type is
/// at the type's place in [`InstrumentType::ALL`].
pub static INSTRUMENTS: [Instrument; InstrumentType::ALL.len()] = [
    woodwinds::PICCOLO,
    woodwinds::FLUTE,
    woodwinds::ALTO_FLUTE,
    woodwinds::BASS_FLUTE,
    woodwinds::OBOE,
    woodwinds::OBOE_DAMORE,
    woodwinds::ENGLISH_HORN,
    woodwinds::CLARINET_EB,
    woodwinds::CLARINET_BB,
    woodwinds::CLARINET_A,
    woodwinds::BASS_CLARINET,
    woodwinds::SAXOPHONE_SOPRANO,
    woodwinds::SAXOPHONE_ALTO,
    woodwinds::SAXOPHONE_TENOR,
    woodwinds::SAXOPHONE_BARITONE,
    woodwinds::BASSOON,
    woodwinds::CONTRABASSOON,
    brass::HORN_F,
    brass::WAGNER_TUBA_TENOR,
    brass::WAGNER_TUBA_BASS,
    brass::TRUMPET_PICCOLO,
    brass::TRUMPET_C,
    brass::TRUMPET_BB,
    brass::CORNET,
    brass::FLUGELHORN,
    brass::TROMBONE_TENOR,
    brass::TROMBONE_BASS,
    brass::EUPHONIUM,
    brass::TUBA,
    brass::CIMBASSO,
    percussion::TIMPANI,
    percussion::BASS_DRUM,
    percussion::SNARE_DRUM,
    percussion::CLASH_CYMBALS,
    percussion::SUSPENDED_CYMBAL,
    percussion::TAM_TAM,
    percussion::TRIANGLE,
    percussion::TAMBOURINE,
    percussion::CASTANETS,
    percussion::WOOD_BLOCK,
    percussion::TEMPLE_BLOCKS,
    percussion::SLEIGH_BELLS,
    percussion::MARACAS,
    percussion::SHAKER,
    percussion::WHIP,
    percussion::GLOCKENSPIEL,
    percussion::XYLOPHONE,
    percussion::VIBRAPHONE,
    percussion::MARIMBA,
    percussion::TUBULAR_BELLS,
    percussion::CROTALES,
    percussion::CARILLON,
    plucked::HARP,
    plucked::GUITAR_CLASSICAL,
    plucked::LUTE,
    plucked::MANDOLIN,
    keyboards::CELESTA,
    keyboards::PIANO_CONCERT_GRAND,
    keyboards::PIANO_STUDIO_GRAND,
    keyboards::PIANO_UPRIGHT,
    keyboards::PIANO_FELT,
    keyboards::PIANO_PREPARED,
    keyboards::PIANO_HONKY_TONK,
    keyboards::HARPSICHORD,
    keyboards::CLAVICHORD,
    keyboards::ELECTRIC_PIANO_RHODES,
    keyboards::ELECTRIC_PIANO_WURLITZER,
    keyboards::CLAVINET,
    keyboards::ORGAN_PIPE,
    keyboards::ORGAN_HAMMOND,
    keyboards::ACCORDION,
    voices::VOICE_SOPRANO,
    voices::VOICE_MEZZO_SOPRANO,
    voices::VOICE_ALTO,
    voices::VOICE_TENOR,
    voices::VOICE_BARITONE,
    voices::VOICE_BASS,
    voices::CHOIR_SOPRANO,
    voices::CHOIR_ALTO,
    voices::CHOIR_TENOR,
    voices::CHOIR_BASS,
    strings::VIOLIN,
    strings::VIOLA,
    strings::CELLO,
    strings::DOUBLE_BASS,
];

// Compile-time check: the record at each place of `INSTRUMENTS` is that of the type at the same
// place of `InstrumentType::ALL`, so the dataset covers every type once, in score order, and a
// lookup by place cannot return another type's record.
const _: () = {
    let mut i = 0;
    while i < INSTRUMENTS.len() {
        assert!(
            INSTRUMENTS[i].instrument_type() as usize == i,
            "INSTRUMENTS holds each type's record at the type's place in score order"
        );
        i += 1;
    }
};

/// The record of `instrument_type`. The dataset holds one for every type, which the check above
/// proves at compile time, so the answer is always `Some`.
pub(super) fn record(instrument_type: InstrumentType) -> Option<&'static Instrument> {
    INSTRUMENTS.get(instrument_type as usize)
}
