//! The shipped dataset: a representative set of orchestral instruments.
//!
//! One file per family holds that family's records as named constants; [`INSTRUMENTS`] lists them
//! in score order. Ranges are sounding MIDI notes (middle C = 60). Each range says whether a
//! source that was read states it (`Cited`) or not (`Estimate`). A `Cited` value names its
//! sources in full from the list in `sources`, each followed by what it states; Wikipedia is used
//! for facts only (see `docs/orchestration-metadata.md`).

mod brass;
mod percussion;
mod sources;
mod strings;
mod woodwinds;

use super::id::InstrumentType;
use super::instrument::{Instrument, PitchModel, Pitched, Provenance, SourcedRange};
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

/// Every instrument record of the dataset, in score order (the order of
/// [`InstrumentType::ALL`]), at most one per type. A type without a record has no metadata yet.
pub static INSTRUMENTS: [Instrument; 16] = [
    woodwinds::PICCOLO,
    woodwinds::FLUTE,
    woodwinds::OBOE,
    woodwinds::ENGLISH_HORN,
    woodwinds::CLARINET_BB,
    woodwinds::BASSOON,
    woodwinds::CONTRABASSOON,
    brass::HORN_F,
    brass::TRUMPET_BB,
    brass::TROMBONE_TENOR,
    percussion::SNARE_DRUM,
    percussion::GLOCKENSPIEL,
    percussion::XYLOPHONE,
    strings::VIOLIN,
    strings::CELLO,
    strings::DOUBLE_BASS,
];

/// The position of each type's record in [`INSTRUMENTS`], indexed by the type's place in
/// [`InstrumentType::ALL`]; `None` for a type without a record.
const POSITIONS: [Option<usize>; InstrumentType::ALL.len()] = {
    let mut positions = [None; InstrumentType::ALL.len()];
    let mut i = 0;
    while i < INSTRUMENTS.len() {
        let instrument_type = INSTRUMENTS[i].instrument_type();
        // Records in strictly ascending score order: the list is sorted and holds no type twice.
        assert!(
            i == 0 || (INSTRUMENTS[i - 1].instrument_type() as u8) < instrument_type as u8,
            "INSTRUMENTS lists each type at most once, in score order"
        );
        positions[instrument_type as usize] = Some(i);
        i += 1;
    }
    // Every entry points at a record of its own type, so a lookup through the table cannot
    // return another instrument's record.
    let mut t = 0;
    while t < positions.len() {
        if let Some(position) = positions[t] {
            assert!(
                INSTRUMENTS[position].instrument_type() as usize == t,
                "the position table points at a record of the wrong type"
            );
        }
        t += 1;
    }
    positions
};

/// The record of `instrument_type`, or `None` when the dataset has none for it.
pub(super) fn record(instrument_type: InstrumentType) -> Option<&'static Instrument> {
    POSITIONS
        .get(instrument_type as usize)
        .copied()
        .flatten()
        .and_then(|position| INSTRUMENTS.get(position))
}
