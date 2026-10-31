//! What a preset can do, in the form the performance layer reads it ([`SoundCaps`]): the claims of
//! the library's manifest or Sonara's profile and the facts of the patch check, copied into the
//! plain data that `sonara_midi::performance` takes. That layer cannot see the catalog (ARC-11),
//! so the catalog adapts to it (ARC-08).
//!
//! [`sound_caps`] fills every field from one preset. [`SoundSource::caps`] gives the capabilities
//! of whatever the degradation chain chose for a part: a preset, a built-in sound or silence.
//!
//! Nothing here does I/O or runs on the real-time thread (RT-01): it reads a snapshot of the
//! catalog, and a fact the check has not recorded yet is left unknown, never guessed.

use sonara_midi::orchestration::{
    Articulation, InstrumentType, Mute, PitchRange, Request, Vibrato,
};
use sonara_midi::performance::{DynamicsKind, LegatoKind, SoundCaps};

use super::chain::{Reach, SoundSource, named, nearest_articulation};
use super::facts::PatchFacts;
use super::manifest::{
    ArticulationEntry, ArticulationList, ArticulationName, ArticulationVia, DynamicsMode,
    LegatoMode,
};
use super::model::{InstrumentPreset, PresetKind};

/// What `preset` can do, field by field:
///
/// - **`articulations`**: the articulations the sampler's `articulation` parameter plays as
///   asked on the preset, in the order of the vocabulary ([`Articulation::ALL`]). A choice plays
///   as asked when step 2 of the degradation chain ([`nearest_articulation`]) reaches it through
///   an articulation the preset maps, without a substitute; a mute or a vibrato choice
///   (`con_sordino`, `non_vibrato`) asks for the ordinary sound under that mute or vibrato. The
///   parameter selects only on a preset that names its instrument type and maps how its
///   articulations are reached (a version 2 entry), so a preset that does not, a version 1
///   manifest's names, and the keys of a percussion key map give none. `legato` is listed as
///   well when the preset claims legato by overlap or by its own legato regions, which play
///   legato whether or not it maps a legato articulation.
/// - **`legato`**: the preset's legato claim; none when nothing is claimed.
/// - **`layers`**: the highest velocity of each layer, from the velocity layers of every key the
///   check found ([`PatchFacts::sampled`]), for a set of every member's. Where the keys differ,
///   each boundary of any key is kept, so a velocity range inside one listed layer plays the same
///   regions at every key; a velocity range no region plays at counts as a layer too. One layer,
///   `[127]`, until the check has read the patch (for a set, every member).
/// - **`dynamics`**: what the preset's dynamics claim drives; velocity when nothing is claimed,
///   as the sampler plays it.
/// - **`attack_ms`**: the attack of the ordinary sound (`None`) and of each listed articulation,
///   where it is known: the `attack_ms` the articulation states, else the attack the check
///   measured ([`InstrumentPreset::articulation_attacks`]). The ordinary sound is what step 2 of
///   the chain plays for no articulation; on a preset the parameter does not select on, it is the
///   patch as it plays without a switch (its default switch's regions, or every region of a
///   patch without switches). The overlap legato of `legato` starts from the ordinary sound, so
///   it has the ordinary sound's attack; the legato regions of a patch, the attack of the legato
///   articulation it maps, else the ordinary sound's.
/// - **`max_hold_s`**: [`InstrumentPreset::max_hold_ms`] in seconds.
/// - **`sampled`**: the notes from the lowest to the highest key that a press sounds on (for a
///   set, that every member sounds on), less the preset's key offset, so that the range is in the
///   notes a clip writes; within MIDI's 0 to 127. A key between the two that sounds nothing is
///   not told apart: the chain's choice and the preset's verdict report such gaps. `None` until
///   the check has read the patch, and when no key sounds.
///
/// Every value of a preset the catalog built passes [`SoundCaps::check`]: the manifest reader
/// accepts only a legato crossfade of 10 to 500 ms and a fixed velocity of 1 to 127, and every
/// other field is built to hold.
#[must_use]
pub fn sound_caps(preset: &InstrumentPreset) -> SoundCaps {
    let legato = legato(preset);
    let (articulations, attack_ms) = articulations(preset, legato);
    let checked = checked_facts(preset);
    SoundCaps {
        articulations,
        legato,
        layers: checked.as_deref().map_or_else(|| vec![127], layers),
        dynamics: dynamics(preset),
        attack_ms,
        max_hold_s: preset.max_hold_ms().map(|ms| f64::from(ms) / 1000.0),
        sampled: checked
            .as_deref()
            .and_then(|facts| sampled(facts, key_offset(preset))),
    }
}

impl SoundSource {
    /// What this sound can do: [`sound_caps`] of the preset among `presets` that a sampler source
    /// names. A built-in sound and silence are sounds about which the catalog knows nothing
    /// ([`SoundCaps::default`]), and so is a preset that is not among `presets`.
    #[must_use]
    pub fn caps(&self, presets: &[InstrumentPreset]) -> SoundCaps {
        match self {
            Self::Sampler { preset } => presets
                .iter()
                .find(|candidate| candidate.id == *preset)
                .map_or_else(SoundCaps::default, sound_caps),
            Self::Builtin(_) | Self::Silence => SoundCaps::default(),
        }
    }
}

/// The preset's legato claim as the performance layer reads it.
fn legato(preset: &InstrumentPreset) -> LegatoKind {
    match preset.legato.as_ref().map(|claim| claim.value) {
        Some(LegatoMode::Overlap { crossfade_ms, .. }) => LegatoKind::Overlap {
            crossfade_ms: f64::from(crossfade_ms),
        },
        Some(LegatoMode::Trigger) => LegatoKind::Trigger,
        Some(LegatoMode::None) | None => LegatoKind::None,
    }
}

/// The preset's dynamics claim as the performance layer reads it; the controller's number is the
/// sampler's business, not the performance's.
fn dynamics(preset: &InstrumentPreset) -> DynamicsKind {
    match preset.dynamics.as_ref().map(|claim| claim.value) {
        Some(DynamicsMode::Velocity) | None => DynamicsKind::Velocity,
        Some(DynamicsMode::Cc(_)) => DynamicsKind::Cc,
        Some(DynamicsMode::Fixed(velocity)) => DynamicsKind::Fixed(velocity),
    }
}

/// The semitones the preset adds to incoming notes.
fn key_offset(preset: &InstrumentPreset) -> i8 {
    preset.key_offset.as_ref().map_or(0, |claim| claim.value)
}

/// The instrument type step 2 of the chain reads the preset as, when the `articulation`
/// parameter selects on it: the preset names its type and maps at least one articulation with how
/// it is reached.
fn switched_instrument(preset: &InstrumentPreset) -> Option<InstrumentType> {
    let mapped = matches!(
        preset.articulation_map.as_ref().map(|claim| &claim.value),
        Some(ArticulationList::Mapped(entries)) if !entries.is_empty()
    );
    preset
        .instrument
        .map(|claim| claim.instrument)
        .filter(|_| mapped)
}

/// What the choice `articulation` asks for: the articulation itself, or the ordinary sound under
/// the mute or the vibrato it names (the chain's own reading of an articulation id).
fn request_for(articulation: Articulation) -> Request {
    named(
        ArticulationName::Vocabulary(articulation),
        Mute::Open,
        Vibrato::Ordinary,
    )
}

/// The mapped articulation that `reach` plays, if it is one.
fn entry_of(reach: &Reach) -> Option<&ArticulationEntry> {
    match reach {
        Reach::Mapped(entry) => Some(entry),
        Reach::Keys(_) | Reach::Own => None,
    }
}

/// The `articulations` and `attack_ms` of [`sound_caps`].
fn articulations(
    preset: &InstrumentPreset,
    legato: LegatoKind,
) -> (Vec<Articulation>, Vec<(Option<Articulation>, f64)>) {
    let attacks = preset.articulation_attacks();
    let attack_of = |entry: &ArticulationEntry| {
        attacks
            .iter()
            .find(|(mapped, _)| *mapped == entry)
            .map(|(_, attack)| f64::from(attack.ms))
    };
    let instrument = switched_instrument(preset);
    let ordinary = match instrument {
        Some(instrument) => {
            entry_of(&nearest_articulation(preset, instrument, Request::default()).reach)
                .and_then(attack_of)
        }
        None => unswitched_attack(preset),
    };
    let mut listed = Vec::new();
    let mut attack_ms = Vec::new();
    if let Some(ms) = ordinary {
        attack_ms.push((None, ms));
    }
    for articulation in Articulation::ALL {
        let exact = instrument
            .map(|instrument| nearest_articulation(preset, instrument, request_for(articulation)))
            .filter(|choice| choice.substitute.is_none());
        let exact_entry = exact.as_ref().and_then(|choice| entry_of(&choice.reach));
        let attack = match (articulation, legato) {
            (Articulation::Legato, LegatoKind::Overlap { .. }) => Some(ordinary),
            (Articulation::Legato, LegatoKind::Trigger) => {
                Some(exact_entry.map_or(ordinary, attack_of))
            }
            _ => exact_entry.map(attack_of),
        };
        // `None`: the choice does not play the articulation as asked; `Some(None)`: it does, with
        // an attack that is not known.
        if let Some(attack) = attack {
            listed.push(articulation);
            if let Some(ms) = attack {
                attack_ms.push((Some(articulation), ms));
            }
        }
    }
    (listed, attack_ms)
}

/// The attack of the patch a preset plays as it is, with nothing switched: the regions of its
/// default switch, or every region of a patch without switches; for a set, of the patch it plays
/// ([`InstrumentPreset::patch_path`]). `None` before the patch was decoded, and for a patch with
/// switches but no default switch, whose first sound depends on the first switch a clip plays.
fn unswitched_attack(preset: &InstrumentPreset) -> Option<f64> {
    let facts = match &preset.kind {
        PresetKind::Patch => preset.facts.as_ref(),
        PresetKind::Set { members } => members
            .iter()
            .find(|member| member.patch_path == preset.patch_path)?
            .facts
            .as_ref(),
    }?;
    let via = if facts.switch_keys.is_empty() {
        ArticulationVia::Patch
    } else {
        ArticulationVia::Keyswitch {
            key: facts.default_switch?,
        }
    };
    facts.sound.as_ref()?.attack_ms(via).map(f64::from)
}

/// The facts of every patch the preset plays: its own, or each member's for a set; `None` until
/// the check has read all of them.
fn checked_facts(preset: &InstrumentPreset) -> Option<Vec<&PatchFacts>> {
    match &preset.kind {
        PresetKind::Patch => preset.facts.as_ref().map(|facts| vec![facts]),
        PresetKind::Set { members } => members.iter().map(|member| member.facts.as_ref()).collect(),
    }
}

/// The `layers` of [`sound_caps`]: every velocity at which some key of some patch ends a layer,
/// or ends the velocities no region plays at below a layer, then 127.
fn layers(facts: &[&PatchFacts]) -> Vec<u8> {
    let mut highest: Vec<u8> = facts
        .iter()
        .flat_map(|patch| &patch.sampled)
        .flat_map(|zone| &zone.layers)
        .flat_map(|layer| [layer.lo.checked_sub(1), Some(layer.hi)])
        .flatten()
        .filter(|velocity| (1..127).contains(velocity))
        .collect();
    highest.push(127);
    highest.sort_unstable();
    highest.dedup();
    highest
}

/// The `sampled` range of [`sound_caps`] for a preset whose key offset is `offset`.
fn sampled(facts: &[&PatchFacts], offset: i8) -> Option<PitchRange> {
    if facts.is_empty() {
        return None;
    }
    // Whether every patch sounds on the key, key by key.
    let mut every = [true; 128];
    for patch in facts {
        let mut sounds = [false; 128];
        for zone in &patch.sampled {
            for key in zone.lo..=zone.hi {
                if let Some(slot) = sounds.get_mut(usize::from(key)) {
                    *slot = true;
                }
            }
        }
        for (all, one) in every.iter_mut().zip(sounds) {
            *all = *all && one;
        }
    }
    let mut keys = (0..=127_i64)
        .zip(every)
        .filter_map(|(key, sounds)| sounds.then_some(key));
    let lowest = keys.next()?;
    let highest = keys.last().unwrap_or(lowest);
    let offset = i64::from(offset);
    PitchRange::try_new((lowest - offset).max(0), (highest - offset).min(127)).ok()
}

#[cfg(test)]
mod tests;
