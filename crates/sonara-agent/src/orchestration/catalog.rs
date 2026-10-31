//! The planner's read-only view of the instrument catalog: which preset plays a part, and what
//! that sound can do.
//!
//! No agent tool reads the catalog, and adding one would change the tool contract (CTR-01), so
//! the planner reads it as a library through [`CatalogReader`]: the application's catalog for
//! [`AgentService`], a fixed list of presets in tests (DES-06). Reading changes nothing; every
//! edit still goes through the tools.
//!
//! [`Orchestrator::catalog`] reads the catalog once per plan. The snapshot ([`Catalog`]) answers,
//! part by part, what plays it by the degradation chain (`sonara_app::instruments::choose_sound`)
//! and what that sound can do (`sonara_app::instruments::sound_caps`): the `SoundCaps` that the
//! performance layer of `sonara-midi` reads ([`PartSound`]).

use sonara_app::instruments::{
    ChainPreferences, InstrumentCatalog, InstrumentPreset, SoundPlan, SoundRequest, SoundSource,
    choose_sound,
};
use sonara_midi::performance::SoundCaps;

use super::Orchestrator;
use crate::service::AgentService;

/// Where the planner reads the instrument catalog: [`AgentService`] in the application, a fixed
/// list of presets in tests.
pub trait CatalogReader {
    /// The presets of the instrument catalog as they are now, available or not.
    fn presets(&self) -> Vec<InstrumentPreset>;
}

/// The service reads the catalog of the libraries in the application's `sample_libraries`
/// setting, as they are on disk at each call (`InstrumentCatalog::from_settings`). Discovery
/// reads directory entries and manifests and checks no patch, so these presets carry the claims
/// of their manifests and of Sonara's profiles but none of the facts of the patch check
/// (`InstrumentPreset::facts`): the velocity layers, sampled keys, measured attacks and longest
/// hold of their sounds are unknown, and only an attack an articulation states is known.
impl CatalogReader for AgentService {
    fn presets(&self) -> Vec<InstrumentPreset> {
        let settings = self.app().settings().clone();
        InstrumentCatalog::from_settings(&settings).into_presets()
    }
}

/// The instrument catalog as one plan reads it: a snapshot, so that every part of the plan is
/// chosen from the same presets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    presets: Vec<InstrumentPreset>,
}

impl Catalog {
    /// A snapshot of `presets`.
    #[must_use]
    pub fn new(presets: Vec<InstrumentPreset>) -> Self {
        Self { presets }
    }

    /// The presets of the snapshot.
    #[must_use]
    pub fn presets(&self) -> &[InstrumentPreset] {
        &self.presets
    }

    /// What plays the part that `request` describes, with `preferences`, and what that sound can
    /// do. Never fails: the chain's last step is silence.
    #[must_use]
    pub fn sound(&self, request: &SoundRequest, preferences: ChainPreferences) -> PartSound {
        let plan = choose_sound(&self.presets, request, preferences);
        let caps = self.caps(&plan.source);
        PartSound { plan, caps }
    }

    /// What `source` can do: a preset of the snapshot's own capabilities, and nothing known for a
    /// built-in sound, silence or a preset the snapshot does not hold (`SoundSource::caps`). For
    /// the sound a plan's question offers when the user declines, as for the plan's own.
    #[must_use]
    pub fn caps(&self, source: &SoundSource) -> SoundCaps {
        source.caps(&self.presets)
    }
}

/// What plays one part, and what that sound can do.
#[derive(Debug, Clone, PartialEq)]
pub struct PartSound {
    /// The degradation chain's answer: the sound, what it substituted in words, and a question
    /// to ask first, if any.
    pub plan: SoundPlan,
    /// What the plan's sound can do.
    pub caps: SoundCaps,
}

impl<R: CatalogReader> Orchestrator<R> {
    /// Reads the instrument catalog through the runner, once: the snapshot every part of a plan
    /// is chosen from. Calls no tool.
    #[must_use]
    pub fn catalog(&self) -> Catalog {
        Catalog::new(self.runner.presets())
    }
}
