//! The sampler mapping of a preset ([`SamplerMapping`]): what each choice of the sampler's
//! `articulation` parameter selects in the preset's patch under each choice of its `mute` and
//! `vibrato` parameters, what its `dynamics` parameter drives, how the preset plays legato and
//! how far incoming notes are moved, compiled from what the library's manifest or Sonara's
//! profile claims about the preset.
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
//! - **Under each passage state**: the `mute` and `vibrato` parameters (`auto`, then every
//!   [`Mute`] or [`Vibrato`] in its order) state the mute and the vibrato a passage is played
//!   with. A state other than `auto` replaces the one the articulation choice names, if any, and
//!   the choice asks for its articulation under that state: an articulation the profile records
//!   with that mute or vibrato (`{"id": "sustain", "mute": "con_sordino"}`) is chosen when the
//!   state matches, and a combination the preset does not record falls back as step 2 does,
//!   dropping the vibrato first, then the mute (a more general mute first), and only then the
//!   articulation. With `auto` as the articulation, a stated mute or vibrato plays the ordinary
//!   sound under it when the preset records the state (or the more general mute step 2 puts in
//!   its place); otherwise `auto` stays without a target and forces nothing. A preset that
//!   records no mute (or no vibrato) plays every choice of that parameter as `auto`, so its
//!   mapping keeps one column for it.
//! - **How a target is reached** is how the preset maps the articulation: a key switch, a
//!   velocity zone or a controller value, or nothing to select for a patch that is the
//!   articulation. The keys of a percussion key map are chosen by the notes, and a sound whose
//!   articulations the catalog does not know how to reach plays as it is, so neither gives a
//!   target.
//! - **Multi-patch sets.** A target of a set also names the patch that plays it
//!   ([`ArticulationTarget::member`]): the position of the patch the articulation names among
//!   the set's patches ([`PresetKind::Set`]), the order in which [`super::InstalledInstruments`]
//!   hands them to the loader, so the sampler switches patches by the articulation. `auto`
//!   plays the patch of the ordinary sound. A patch is patch 0.
//! - **Legato.** The `legato` choice follows the preset's legato claim. A preset that plays
//!   legato by overlapping its notes plays its ordinary sound (under the passage state) with the
//!   sampler's overlap legato. A preset whose own regions play legato (SFZ `trigger=legato`)
//!   plays the legato articulation it maps (under the state when it records one), else its
//!   ordinary sound, whose regions do the work. Otherwise step 2 chooses, as for any other
//!   choice. Every other choice stays polyphonic, so a chord is never joined into one line by
//!   accident.
//! - **Dynamics, legato times and the key offset** are the preset's claims, or velocity
//!   dynamics, no overlap legato and no offset when nothing is claimed.
//!
//! A preset about which nothing is claimed (no mapped articulations, or a version 1 manifest's
//! names, which do not say how each is reached) compiles to the default mapping, and the sampler
//! plays its patch as it is. A preset that names no single instrument type, such as a key-mapped
//! kit, gets no articulation targets either: the chain chooses articulations for an instrument
//! type.

use sonara_dsp::bank::{
    ArticulationTarget, DynamicsDrive, LegatoSpec, Passage, SamplerMapping, Selector, TargetTable,
};
use sonara_midi::orchestration::{Articulation, Axis, InstrumentType, Mute, Request, Vibrato};

use super::chain::{ArticulationChoice, Reach, nearest_articulation};
use super::manifest::{
    ArticulationEntry, ArticulationList, ArticulationName, ArticulationVia, DynamicsMode,
    LegatoMode,
};
use super::model::{InstrumentPreset, PresetKind, same_path};

/// The choices of the `articulation` parameter before the vocabulary: `auto` and `ordinario`.
const LEADING_CHOICES: usize = 2;
/// The choices of the `articulation` parameter: `auto`, `ordinario`, then the vocabulary.
const ARTICULATION_CHOICES: usize = LEADING_CHOICES + Articulation::ALL.len();
/// The choices of the `mute` parameter: `auto`, then every mute.
const MUTE_CHOICES: usize = 1 + Mute::ALL.len();
/// The choices of the `vibrato` parameter: `auto`, then every vibrato.
const VIBRATO_CHOICES: usize = 1 + Vibrato::ALL.len();

/// The sampler mapping of `preset`, as the module documentation describes it, for a patch or a
/// multi-patch set.
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

/// The target of every choice of the `articulation` parameter under every choice of `mute`
/// and `vibrato`. Only the states the preset records get columns of their own: a choice of a
/// state it does not record plays as `auto`, since step 2 drops that state.
fn targets(preset: &InstrumentPreset, legato: Option<LegatoMode>) -> TargetTable {
    let compiler = Compiler::of(preset);
    let (mutes, vibratos) = compiler.recorded_states();
    TargetTable::tabulate(
        count(ARTICULATION_CHOICES),
        if mutes { count(MUTE_CHOICES) } else { 1 },
        if vibratos { count(VIBRATO_CHOICES) } else { 1 },
        |choice, passage| compiler.target(choice, passage, legato),
    )
}

/// `choices` as a choice count; the lists are far shorter than `u16::MAX`.
fn count(choices: usize) -> u16 {
    u16::try_from(choices).unwrap_or(u16::MAX)
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
        Self {
            preset,
            instrument: preset
                .instrument
                .map(|claim| claim.instrument)
                .filter(|_| !mapped_entries(preset).is_empty()),
        }
    }

    /// Whether the preset records a mute, and whether it records a vibrato: an articulation it
    /// maps with that state, or one named after it. Step 2 drops a state that no articulation
    /// carries, so every choice of it then plays what `auto` plays.
    fn recorded_states(&self) -> (bool, bool) {
        if self.instrument.is_none() {
            return (false, false);
        }
        let entries = mapped_entries(self.preset);
        let axis = |entry: &ArticulationEntry| match entry.name {
            ArticulationName::Vocabulary(articulation) => articulation.axis(),
            ArticulationName::Ordinario => Axis::Primary,
        };
        let mutes = entries
            .iter()
            .any(|entry| entry.mute != Mute::Open || matches!(axis(entry), Axis::Mute(_)));
        let vibratos = entries.iter().any(|entry| {
            entry.vibrato != Vibrato::Ordinary || matches!(axis(entry), Axis::Vibrato(_))
        });
        (mutes, vibratos)
    }

    /// The target of the articulation choice `choice` under `passage`, as the module
    /// documentation describes it.
    fn target(
        &self,
        choice: u16,
        passage: Passage,
        legato: Option<LegatoMode>,
    ) -> Option<ArticulationTarget> {
        let state = State::of(passage);
        let ordinary = || self.nearest(state.applied(Request::default()));
        match usize::from(choice).checked_sub(LEADING_CHOICES) {
            // `auto`: the patch's own default and the keyswitch notes of clips decide, unless a
            // stated mute or vibrato has a recording of the ordinary sound.
            None if choice == SamplerMapping::AUTO => {
                self.auto_under(state.applied(Request::default()))
            }
            // `ordinario`.
            None => ordinary(),
            Some(index) => {
                let articulation = *Articulation::ALL.get(index)?;
                let request = state.applied(request_for(articulation));
                match (articulation, legato) {
                    // The ordinary sound, in its patch of a set, joined by the overlap legato.
                    (Articulation::Legato, Some(LegatoMode::Overlap { .. })) => {
                        Some(ArticulationTarget {
                            legato: true,
                            ..ordinary().unwrap_or(ArticulationTarget::select(None))
                        })
                    }
                    (Articulation::Legato, Some(LegatoMode::Trigger)) => {
                        self.mapped_legato(request).or_else(ordinary)
                    }
                    _ => self.nearest(request),
                }
            }
        }
    }

    /// What the chain plays for `request` on this preset; `None` when there is nothing to
    /// compile.
    fn choose(&self, request: Request) -> Option<ArticulationChoice> {
        let instrument = self.instrument?;
        Some(nearest_articulation(self.preset, instrument, request))
    }

    /// The target of `request`, or of the nearest articulation the preset plays.
    fn nearest(&self, request: Request) -> Option<ArticulationTarget> {
        target_of(self.preset, &self.choose(request)?.reach)
    }

    /// The target of a legato the preset maps, with the state of `request` when it records the
    /// legato that way, else with the nearest state it does; `None` when step 2 plays another
    /// articulation in its place.
    fn mapped_legato(&self, request: Request) -> Option<ArticulationTarget> {
        let choice = self.choose(request)?;
        (played(&choice).primary() == Some(Articulation::Legato))
            .then(|| target_of(self.preset, &choice.reach))
            .flatten()
    }

    /// The target of `auto` under the mute and the vibrato of `request`, the ordinary sound:
    /// the nearest the preset plays when it keeps some of the stated state (the mute, or a more
    /// general one; the vibrato), else none, so `auto` forces nothing.
    fn auto_under(&self, request: Request) -> Option<ArticulationTarget> {
        if request == Request::default() {
            return None;
        }
        let choice = self.choose(request)?;
        let plays = played(&choice);
        let keeps_mute = request.mute() != Mute::Open && plays.mute() != Mute::Open;
        let keeps_vibrato =
            request.vibrato() != Vibrato::Ordinary && plays.vibrato() == request.vibrato();
        (keeps_mute || keeps_vibrato)
            .then(|| target_of(self.preset, &choice.reach))
            .flatten()
    }
}

/// The mapped articulations of `preset`; none when it maps none, or names them without saying
/// how each is reached.
fn mapped_entries(preset: &InstrumentPreset) -> &[ArticulationEntry] {
    match preset.articulation_map.as_ref().map(|claim| &claim.value) {
        Some(ArticulationList::Mapped(entries)) => entries,
        Some(ArticulationList::Names(_)) | None => &[],
    }
}

/// The mute and the vibrato a passage states through the `mute` and `vibrato` parameters;
/// `None` for `auto`, which keeps what the articulation choice names, and for a choice beyond
/// the lists, which plays as `auto`.
#[derive(Clone, Copy)]
struct State {
    mute: Option<Mute>,
    vibrato: Option<Vibrato>,
}

impl State {
    /// The states of the choices of `passage`: choice `i + 1` is `Mute::ALL[i]` and
    /// `Vibrato::ALL[i]`, as [`crate::samples`] offers them.
    fn of(passage: Passage) -> Self {
        let stated = |choice: u16| usize::from(choice).checked_sub(1);
        Self {
            mute: stated(passage.mute).and_then(|index| Mute::ALL.get(index).copied()),
            vibrato: stated(passage.vibrato).and_then(|index| Vibrato::ALL.get(index).copied()),
        }
    }

    /// `request` with the stated mute and vibrato in place of its own.
    fn applied(self, request: Request) -> Request {
        // The primary is the request's own, so the request stays valid; the fallback is
        // unreachable.
        Request::new(
            request.primary(),
            self.mute.unwrap_or(request.mute()),
            self.vibrato.unwrap_or(request.vibrato()),
        )
        .unwrap_or(request)
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

/// What plays for the chain's `choice`: the request itself, or its substitute.
fn played(choice: &ArticulationChoice) -> Request {
    choice
        .substitute
        .as_ref()
        .map_or(choice.requested, |substitute| substitute.plays)
}

/// The target of what the chain reached on `preset`: the patch that plays a mapped articulation
/// ([`member_of`]) and its selector, or nothing to select for a whole patch; `None` for the keys
/// of a key map, for a sound whose articulations the catalog does not know how to reach, and for
/// an articulation of a set whose patch the set does not hold.
fn target_of(preset: &InstrumentPreset, reach: &Reach) -> Option<ArticulationTarget> {
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
        member: member_of(preset, entry)?,
        selector,
        legato: false,
    })
}

/// The number of the patch of `preset` that plays `entry`: in a multi-patch set, the position
/// among the set's patches of the patch the entry names, compared as discovery compares paths;
/// 0 for a preset of one patch. `None` for an articulation of a set that names no patch, or one
/// the set does not hold, and for a position past the most patches a bank holds.
fn member_of(preset: &InstrumentPreset, entry: &ArticulationEntry) -> Option<u8> {
    let PresetKind::Set { members } = &preset.kind else {
        return Some(0);
    };
    let path = entry.member.as_deref()?;
    let position = members
        .iter()
        .position(|member| same_path(&member.path, path))?;
    u8::try_from(position).ok()
}

#[cfg(test)]
mod tests;
