//! The sampler mapping of a preset ([`SamplerMapping`]): what each choice of the sampler's
//! `articulation` parameter selects in the preset's patch, what its `dynamics` parameter
//! drives, how the preset plays legato and how far incoming notes are moved, compiled from what
//! the library's manifest or Sonara's profile claims about the preset.
//!
//! [`super::InstalledInstruments`] compiles the mapping of every playable preset when it is
//! built, and the media provider hands it to the sampler with the loaded patch
//! ([`sonara_media::MediaProvider::load_instrument`]). Compiling runs off the real-time thread
//! and allocates; the render thread only looks a choice up (RT-02, RT-09).
//!
//! - **One target per choice**, in the order of the parameter's choices: `auto`, `ordinario`,
//!   then the articulation vocabulary in its frozen order ([`Articulation::ALL`]), as
//!   [`crate::samples`] lists them. `auto` has no target. `ordinario` is the preset's ordinary
//!   sound: the articulation it maps as `ordinario`, else its default one. Every other choice is
//!   the articulation the preset maps for it, and for one it does not map, the nearest it plays
//!   by step 2 of the degradation chain ([`nearest_articulation`]), chosen here so that the
//!   render thread never searches. A mute or vibrato choice (`con_sordino`, `non_vibrato`) asks
//!   for the ordinary sound under that mute or vibrato.
//! - **How a target is reached** is how the preset maps the articulation: a key switch, a
//!   velocity zone or a controller value, or nothing to select for a patch that is the
//!   articulation. The keys of a percussion key map are chosen by the notes, and a sound whose
//!   articulations the catalog does not know how to reach plays as it is, so neither gives a
//!   target.
//! - **Legato.** The `legato` choice follows the preset's legato claim. A preset that plays
//!   legato by overlapping its notes plays its ordinary sound with the sampler's overlap legato.
//!   A preset whose own regions play legato (SFZ `trigger=legato`) plays the legato articulation
//!   it maps, else its ordinary sound, whose regions do the work. Otherwise step 2 chooses, as
//!   for any other choice. Every other choice stays polyphonic, so a chord is never joined into
//!   one line by accident.
//! - **Dynamics, legato times and the key offset** are the preset's claims, or velocity
//!   dynamics, no overlap legato and no offset when nothing is claimed.
//!
//! A preset about which nothing is claimed (no mapped articulations, or a version 1 manifest's
//! names, which do not say how each is reached) compiles to the default mapping, and the sampler
//! plays its patch as it is. A preset that names no single instrument type, such as a key-mapped
//! kit, gets no articulation targets either: the chain chooses articulations for an instrument
//! type.

use sonara_dsp::bank::{ArticulationTarget, DynamicsDrive, LegatoSpec, SamplerMapping, Selector};
use sonara_midi::orchestration::{Articulation, Axis, InstrumentType, Mute, Request, Vibrato};

use super::chain::{Reach, nearest_articulation};
use super::manifest::{ArticulationList, ArticulationVia, DynamicsMode, LegatoMode};
use super::model::InstrumentPreset;

/// The choices of the `articulation` parameter before the vocabulary: `auto` and `ordinario`.
const LEADING_CHOICES: usize = 2;

/// The sampler mapping of `preset`, as the module documentation describes it. A multi-patch
/// set compiles as the patch it plays: a member patch is not selected (the sampler plays one
/// patch per device).
#[must_use]
pub(crate) fn sampler_mapping(preset: &InstrumentPreset) -> SamplerMapping {
    let legato = preset.legato.as_ref().map(|claim| claim.value);
    let dynamics = match preset.dynamics.as_ref().map(|claim| claim.value) {
        None | Some(DynamicsMode::Velocity) => DynamicsDrive::Velocity,
        Some(DynamicsMode::Cc(number)) => DynamicsDrive::Controller(number),
        Some(DynamicsMode::Fixed(velocity)) => DynamicsDrive::Fixed(velocity),
    };
    let spec = match legato {
        Some(LegatoMode::Overlap {
            crossfade_ms,
            start_offset_ms,
            gap_ms,
        }) => Some(LegatoSpec {
            crossfade_ms,
            start_offset_ms,
            gap_ms,
        }),
        Some(LegatoMode::None | LegatoMode::Trigger) | None => None,
    };
    let key_offset = preset.key_offset.as_ref().map_or(0, |claim| claim.value);
    SamplerMapping::new(targets(preset, legato), dynamics, spec, key_offset)
}

/// The target of every choice of the `articulation` parameter, by index.
fn targets(
    preset: &InstrumentPreset,
    legato: Option<LegatoMode>,
) -> Vec<Option<ArticulationTarget>> {
    let compiler = Compiler::of(preset);
    let ordinary = compiler.nearest(Request::default());
    let mut targets = Vec::with_capacity(LEADING_CHOICES + Articulation::ALL.len());
    // `auto`: the patch's own default and the keyswitch notes of clips decide.
    targets.push(None);
    targets.push(ordinary);
    for articulation in Articulation::ALL {
        let request = request_for(articulation);
        let target = match (articulation, legato) {
            (Articulation::Legato, Some(LegatoMode::Overlap { .. })) => Some(ArticulationTarget {
                selector: ordinary.and_then(|target| target.selector),
                legato: true,
            }),
            (Articulation::Legato, Some(LegatoMode::Trigger)) => {
                compiler.exact(request).or(ordinary)
            }
            _ => compiler.nearest(request),
        };
        targets.push(target);
    }
    targets
}

/// Step 2 of the chain over one preset.
struct Compiler<'a> {
    preset: &'a InstrumentPreset,
    /// The instrument type the preset plays, when it maps how its articulations are reached;
    /// `None` when there is nothing to compile.
    instrument: Option<InstrumentType>,
}

impl<'a> Compiler<'a> {
    fn of(preset: &'a InstrumentPreset) -> Self {
        let mapped = matches!(
            preset.articulation_map.as_ref().map(|claim| &claim.value),
            Some(ArticulationList::Mapped(entries)) if !entries.is_empty()
        );
        Self {
            preset,
            instrument: preset
                .instrument
                .map(|claim| claim.instrument)
                .filter(|_| mapped),
        }
    }

    /// The target of `request`, or of the nearest articulation the preset plays.
    fn nearest(&self, request: Request) -> Option<ArticulationTarget> {
        let instrument = self.instrument?;
        target_of(&nearest_articulation(self.preset, instrument, request).reach)
    }

    /// The target of `request` when the preset maps it as asked; `None` when it plays a
    /// substitute.
    fn exact(&self, request: Request) -> Option<ArticulationTarget> {
        let instrument = self.instrument?;
        let choice = nearest_articulation(self.preset, instrument, request);
        if choice.substitute.is_some() {
            return None;
        }
        target_of(&choice.reach)
    }
}

/// What the choice `articulation` asks for: the articulation itself, or the ordinary sound
/// under the mute or the vibrato it names.
fn request_for(articulation: Articulation) -> Request {
    let request = match articulation.axis() {
        Axis::Primary => Request::new(Some(articulation), Mute::Open, Vibrato::Ordinary),
        Axis::Mute(mute) => Request::new(None, mute, Vibrato::Ordinary),
        Axis::Vibrato(vibrato) => Request::new(None, Mute::Open, vibrato),
    };
    // Only an articulation of the primary axis is put in the primary place, so the request is
    // valid; the fallback is unreachable.
    request.unwrap_or_default()
}

/// The target of what the chain reached: a mapped articulation's selector, or nothing to select
/// for a whole patch; `None` for the keys of a key map and for a sound whose articulations the
/// catalog does not know how to reach.
fn target_of(reach: &Reach) -> Option<ArticulationTarget> {
    let Reach::Mapped(entry) = reach else {
        return None;
    };
    let selector = match entry.via {
        ArticulationVia::Patch => None,
        ArticulationVia::Keyswitch { key } => Some(Selector::Keyswitch(key)),
        ArticulationVia::Velocity { lo, hi } => Some(Selector::Velocity { lo, hi }),
        ArticulationVia::Cc { cc, value } => Some(Selector::Controller { number: cc, value }),
    };
    Some(ArticulationTarget {
        selector,
        legato: false,
    })
}

#[cfg(test)]
mod tests;
