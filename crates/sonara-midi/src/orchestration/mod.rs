//! Orchestral instrument metadata: pitch ranges, transposition, families and articulations
//! (SON-P06, `contracts/orchestration-metadata.md`).
//!
//! The data is about instruments in general, not about any sample library, audio engine or UI. A
//! value is either sourced or marked as an estimate, and an articulation counts as supported only
//! when a library declared it. See `docs/orchestration-metadata.md` for the conventions.

mod articulation;
mod data;
mod id;
mod instrument;
mod pitch;

pub use articulation::{Articulation, ArticulationAvailability};
pub use data::INSTRUMENTS;
pub use id::InstrumentType;
pub use instrument::{
    CheckError, Family, Instrument, InstrumentError, PitchModel, Pitched, Provenance, RangeCheck,
    SourcedRange,
};
pub use pitch::{PitchRange, RangeError, SoundingPitch, Transposition, WrittenPitch};

/// The instrument with id `id`, or `None` for an unknown id or a type without a record. The id
/// must match exactly ([`InstrumentType::from_id`]).
#[must_use]
pub fn instrument(id: &str) -> Option<&'static Instrument> {
    InstrumentType::from_id(id).and_then(InstrumentType::get)
}

/// Every instrument record, in score order (the order of [`InstrumentType::ALL`]).
#[must_use]
pub fn instruments() -> &'static [Instrument] {
    &INSTRUMENTS
}
