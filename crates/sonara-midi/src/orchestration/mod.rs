//! Orchestral instrument metadata: names and aliases in five languages, categories, pitch ranges,
//! transposition, families, articulations, ensemble profiles, General MIDI voices, keyboard
//! mechanics and playability facts (SON-P06, `contracts/orchestration-metadata.md`), the
//! ensemble templates that assemble the instruments into orchestras and smaller groups, the
//! orchestration rules with the tables they read ([`rules`]), what to play when no installed
//! sound plays an instrument or an articulation ([`fallback`]), and a compact plain-text
//! reference of the instruments for an agent's system prompt ([`mod@reference`]).
//!
//! The data is about instruments in general, not about any sample library, audio engine or UI. A
//! value is either sourced or marked as an estimate, and an articulation counts as supported only
//! when a library declared it. See `docs/orchestration-metadata.md` for the conventions.

mod articulation;
mod data;
mod ensemble_profile;
mod ensembles;
pub mod fallback;
mod general_midi;
mod id;
mod instrument;
mod keyboard;
mod names;
mod pitch;
mod playability;
pub mod reference;
pub mod rules;

pub use articulation::{
    Articulation, ArticulationAvailability, Axis, Kind, Mute, Request, RequestAvailability,
    RequestError, Vibrato,
};
pub use data::{ENSEMBLES, INSTRUMENTS};
pub use ensemble_profile::{EnsembleProfile, Era, EraPlayers, Forms, PlayerRange};
pub use ensembles::{
    Ensemble, EnsembleError, EnsembleId, EnsembleSpec, Pan, Part, PartOrder, Players, Seating,
    StringSection, StringSize,
};
pub use general_midi::{
    GmNote, GmProgram, GmTarget, GmVoice, ORCHESTRAL_PERCUSSION_MAP, SubstituteFit,
    percussion_instrument,
};
pub(crate) use id::is_snake_case;
pub use id::{Category, CategoryId, InstrumentType, Selection};
pub use instrument::{
    CheckError, Family, Instrument, InstrumentError, InstrumentSpec, PitchModel, Pitched,
    Provenance, RangeCheck, SourcedRange,
};
pub use keyboard::{
    Action, Controller, Decay, KeyboardMechanics, Noise, Pedal, PedalKind, Resonance, TimbreState,
};
pub use names::{Alias, Lang, Mention, Names, Number, Target, all_aliases, fold, resolve};
pub use pitch::{Clef, PitchRange, RangeError, SoundingPitch, Transposition, WrittenPitch};
pub use playability::{Fact, FactKind, LetterName, MODERATO_BPM, Span};

/// The instrument with id `id`, or `None` for an unknown id. The id must match exactly
/// ([`InstrumentType::from_id`]).
#[must_use]
pub fn instrument(id: &str) -> Option<&'static Instrument> {
    InstrumentType::from_id(id).map(InstrumentType::get)
}

/// Every instrument record, in score order (the order of [`InstrumentType::ALL`]).
#[must_use]
pub fn instruments() -> &'static [Instrument] {
    &INSTRUMENTS
}

/// The record of category `id`.
#[must_use]
pub fn category(id: CategoryId) -> &'static Category {
    id.get()
}

/// The ensemble template with id `id`, or `None` for an unknown id. The id must match exactly
/// ([`EnsembleId::from_id`]).
#[must_use]
pub fn ensemble(id: &str) -> Option<&'static Ensemble> {
    EnsembleId::from_id(id).map(EnsembleId::get)
}

/// Every ensemble template, in the order of [`EnsembleId::ALL`].
#[must_use]
pub fn ensembles() -> &'static [Ensemble] {
    &ENSEMBLES
}
