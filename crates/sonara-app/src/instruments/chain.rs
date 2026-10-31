//! The degradation chain: what plays a part when no installed preset plays it exactly as
//! written, and what the user is told about it.
//!
//! [`choose_sound`] answers a [`SoundRequest`] (an instrument type, solo or section, the
//! articulations of the part and the range of its notes) from the presets of the catalog. It
//! tries these steps in order and keeps the first that finds a sound:
//!
//! 1. **Exact.** A playable preset ([`InstrumentPreset::is_playable`]) that plays the instrument
//!    in the size asked for. A preset plays the instrument when its manifest or profile says so,
//!    or when its percussion key map has keys for it.
//! 2. **Nearest articulation.** An articulation the chosen preset does not play is played by the
//!    nearest one it does, or imitated by rewriting the notes ([`nearest_articulation`]).
//! 3. **Solo or section swap.** When no preset of the size asked for is installed, a preset of
//!    the other size, with step 2 again.
//! 4. **Family neighbour.** When no preset of the instrument is installed, a neighbour that can
//!    play its part ([`sonara_midi::orchestration::fallback::neighbours`]), with steps 1 to 3 for
//!    the neighbour. A neighbour whose playable range holds the part's notes comes first, in the
//!    order of preference; one that does not, or that changes the family, the pitch or the role of
//!    the part, is offered only with a question to the user. Nothing is moved by an octave
//!    without that question.
//! 5. **General MIDI.** A General MIDI program needs a General MIDI engine, which this build does
//!    not have, so the chain goes on from step 4 to step 6.
//! 6. **Built-in.** A preset of the Poly Synth, or keys of the Drum Kit, that approximates the
//!    instrument ([`Builtin`]).
//! 7. **Silence.** The track stays silent.
//!
//! Among several presets of one step, the one that keeps the size asked for wins (unless the
//! user prefers to keep the articulations, [`ChainPreferences`]), then the one that plays the
//! most requested articulations as asked, then the better verdict (as is, then partly, then not
//! judged), then the one whose sampled keys hold the part's notes, then the one whose claim is
//! the library's own word before Sonara's profile and before a guess from the path, then the
//! order of preference among the presets of the libraries Sonara has profiles for, then the ID.
//!
//! Every substitution comes back in plain words ([`Notice`]), and a choice the user should make
//! first as a [`Confirmation`] that carries the sound plan for the answer "no".
//!
//! The chain is a pure function over a snapshot of the catalog: it does no I/O, starts no audio
//! engine and never runs on the real-time thread (RT-01).

mod tables;
#[cfg(test)]
mod tests;
mod words;

use std::cmp::Ordering;

use sonara_midi::orchestration::fallback::{self, Emulation, Manner, Neighbour, Scope, StandIn};
use sonara_midi::orchestration::{
    Articulation, Axis, InstrumentType, Mute, PitchModel, PitchRange, Request, Vibrato,
};

use super::facts::PatchFacts;
use super::manifest::{
    ArticulationEntry, ArticulationList, ArticulationName, EnsembleKind, KeymapEntry, Verdict,
    VerdictState,
};
use super::model::{ClaimSource, InstrumentId, InstrumentPreset, PresetKind, same_path};

pub use tables::Builtin;

/// One step of the chain, in the order the steps are tried. Step 5, a General MIDI program, is
/// never taken: this build has no General MIDI engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Step {
    /// A preset of the instrument, in the size asked for.
    Exact,
    /// Another articulation of the chosen sound, or a rewrite of the notes, in place of one the
    /// sound does not play.
    NearestArticulation,
    /// A preset of the instrument in the other size: a section for a solo, or a solo for a
    /// section.
    EnsembleSwap,
    /// A preset of a neighbour of the instrument.
    FamilyNeighbour,
    /// A built-in instrument's preset.
    Builtin,
    /// Nothing: the track is silent.
    Silence,
}

/// What a part asks the chain for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundRequest {
    /// The instrument the part is written for.
    pub instrument: InstrumentType,
    /// Solo or section, or `None` when either will do. A section emulated from solo recordings
    /// counts as a section.
    pub ensemble: Option<EnsembleKind>,
    /// The articulations the part plays, each with the mute and the vibrato of its passage.
    /// Empty means the instrument's ordinary sound alone.
    pub articulations: Vec<Request>,
    /// The lowest and the highest sounding pitch of the part, or `None` when they are not known.
    /// Ignored for an unpitched instrument.
    pub span: Option<PitchRange>,
}

impl SoundRequest {
    /// A request for the ordinary sound of `instrument`, in either size, over any range.
    #[must_use]
    pub const fn new(instrument: InstrumentType) -> Self {
        Self {
            instrument,
            ensemble: None,
            articulations: Vec::new(),
            span: None,
        }
    }

    /// The requested articulations; the ordinary sound when none is named.
    fn requests(&self) -> Vec<Request> {
        if self.articulations.is_empty() {
            vec![Request::default()]
        } else {
            self.articulations.clone()
        }
    }

    /// The span, when the instrument has pitches.
    fn pitched_span(&self) -> Option<PitchRange> {
        self.span.filter(|_| playable(self.instrument).is_some())
    }
}

/// The user's preferences for the chain.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChainPreferences {
    /// Keep the articulations rather than the size asked for: a preset of the other size that
    /// plays more of the requested articulations as asked wins over one of the size asked for
    /// that plays them through substitutes. This swaps steps 2 and 3. Off by default.
    pub keep_articulations: bool,
}

/// What plays the part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoundSource {
    /// A sampler preset of the catalog.
    Sampler {
        /// The preset.
        preset: InstrumentId,
    },
    /// A built-in instrument's preset.
    Builtin(Builtin),
    /// Nothing: the track is silent.
    Silence,
}

/// How the chosen sound reaches one articulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reach {
    /// An articulation the preset maps, as its manifest or its profile states it: how it is
    /// reached and, in a multi-patch set, which member plays it.
    Mapped(ArticulationEntry),
    /// The keys of the preset's percussion key map that play the instrument this way.
    Keys(Vec<u8>),
    /// The sound's own, with nothing to switch, as far as the catalog knows: a preset that maps
    /// no articulation, or whose version 1 manifest names its articulations without saying how
    /// each is reached, or a built-in instrument.
    Own,
}

/// What plays in place of a requested articulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Substitute {
    /// The articulation, the mute and the vibrato that play; the default request is the
    /// sound's ordinary sound.
    pub plays: Request,
    /// The rewrites of the notes as they play, in order: those that bring the stand-in nearer
    /// the articulation asked for, or imitate it, then the one that stands in for a missing mute.
    pub rewrites: Vec<Emulation>,
    /// The kind of passage the stand-in suits, or `None` for any. For a passage of another kind
    /// the caller asks the user before using it.
    pub only: Option<Scope>,
}

/// How the chosen sound plays one requested articulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArticulationChoice {
    /// The articulation the part asks for.
    pub requested: Request,
    /// How the sound reaches what plays.
    pub reach: Reach,
    /// What plays in its place, or `None` when the sound plays it as asked.
    pub substitute: Option<Substitute>,
}

impl ArticulationChoice {
    /// What the user is told about this choice when `instrument` plays it, or `None` when it
    /// plays as asked; for example "Spiccato is not available for Violin; playing staccato."
    #[must_use]
    pub fn message(&self, instrument: InstrumentType) -> Option<String> {
        self.substitute
            .as_ref()
            .map(|substitute| words::substitution(self.requested, substitute, instrument))
    }
}

/// A substitution the chain made, in words for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// The step that made it.
    pub step: Step,
    /// What the user is told.
    pub message: String,
}

/// A question to put to the user before a sound plan is used, and the sound plan to use when
/// the answer is no.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirmation {
    /// The question, in words for the user.
    pub question: String,
    /// What plays when the user declines.
    pub otherwise: Box<SoundPlan>,
}

/// The chain's answer to a [`SoundRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundPlan {
    /// The step whose sound plays the part: [`Step::Exact`], [`Step::EnsembleSwap`],
    /// [`Step::FamilyNeighbour`], [`Step::Builtin`] or [`Step::Silence`].
    pub step: Step,
    /// What plays the part.
    pub source: SoundSource,
    /// The neighbour that plays the part, with how it plays it, when the instrument has no
    /// preset (step 4); `None` otherwise.
    pub neighbour: Option<Neighbour>,
    /// How the source plays each requested articulation, in the order of the request (the
    /// ordinary sound when the request names none); empty for silence.
    pub articulations: Vec<ArticulationChoice>,
    /// What the user is told, in order: the step that chose the source, a swap of the size, the
    /// losses of a preset that plays partly, notes it has no samples for, then each substituted
    /// articulation.
    pub notices: Vec<Notice>,
    /// A question to ask first, or `None` when the sound plan can be used as it is.
    pub confirm: Option<Confirmation>,
}

/// What plays the part that `request` describes, from the presets of the catalog (`presets`,
/// usually [`super::InstrumentCatalog::presets`]), with `preferences`; see the module
/// documentation for the steps. Never fails: the last step is silence.
#[must_use]
pub fn choose_sound(
    presets: &[InstrumentPreset],
    request: &SoundRequest,
    preferences: ChainPreferences,
) -> SoundPlan {
    let requests = request.requests();
    let instrument = request.instrument;
    let own = candidates(
        presets,
        instrument,
        &requests,
        request.ensemble,
        request.pitched_span(),
    );
    if let Some(candidate) = best(own, preferences) {
        return sampler_plan(candidate, request, &requests, None);
    }
    let mut fallbacks = neighbour_fallbacks(presets, request, &requests, preferences);
    fallbacks.extend(tables::builtin(instrument).map(Fallback::Builtin));
    plan_from(&mut fallbacks.into_iter(), request, &requests)
}

/// How `preset` plays `request` when it plays `instrument` (step 2 of the chain): as asked when
/// it can, else the nearest it plays.
///
/// The note's own articulation is kept longest: the passage's vibrato gives way first, then its
/// mute (a mute's more general mute first, then no mute, a dynamic step softer for a mute the
/// substitution data imitates that way), and only then the articulation, which falls back to
/// the articulation it refines, then to its stand-ins in order of preference
/// ([`sonara_midi::orchestration::fallback::substitutes`]), then to the ordinary sound with the
/// notes rewritten to imitate it, then to the ordinary sound as it is. A stand-in that another
/// preset or another instrument plays is not used: one sound plan has one sound, and the caller
/// can ask for that sound in a request of its own.
#[must_use]
pub fn nearest_articulation(
    preset: &InstrumentPreset,
    instrument: InstrumentType,
    request: Request,
) -> ArticulationChoice {
    Sound::of(preset, instrument).nearest(request)
}

/// What a preset offers for one instrument, as the chain reads it.
#[derive(Default)]
struct Sound<'a> {
    /// The rows of its percussion key map that play the instrument.
    keymap: Vec<&'a KeymapEntry>,
    /// Its mapped articulations, each with the request it plays.
    entries: Vec<(Request, &'a ArticulationEntry)>,
    /// The articulations a version 1 manifest names for it, where they are known ids.
    names: Option<Vec<ArticulationName>>,
}

impl<'a> Sound<'a> {
    fn of(preset: &'a InstrumentPreset, instrument: InstrumentType) -> Self {
        let keymap = preset
            .keymap
            .as_ref()
            .map(|claim| {
                claim
                    .value
                    .iter()
                    .filter(|row| row.instrument == instrument)
                    .collect()
            })
            .unwrap_or_default();
        let (entries, names) = match preset.articulation_map.as_ref().map(|claim| &claim.value) {
            Some(ArticulationList::Mapped(entries)) => (
                entries
                    .iter()
                    .map(|entry| (named(entry.name, entry.mute, entry.vibrato), entry))
                    .collect(),
                None,
            ),
            Some(ArticulationList::Names(names)) => (
                Vec::new(),
                Some(
                    names
                        .iter()
                        .filter_map(|name| ArticulationName::parse(name))
                        .collect(),
                ),
            ),
            None => (Vec::new(), None),
        };
        Self {
            keymap,
            entries,
            names,
        }
    }

    /// Whether the catalog says nothing about how the sound is reached.
    fn is_own(&self) -> bool {
        self.keymap.is_empty() && self.entries.is_empty()
    }

    /// How the sound plays `request` as asked, if it does.
    fn find(&self, request: Request) -> Option<Reach> {
        if request.mute() == Mute::Open && request.vibrato() == Vibrato::Ordinary {
            let keys: Vec<u8> = self
                .keymap
                .iter()
                .filter(|row| row_primary(row) == request.primary())
                .flat_map(|row| row.keys.iter().copied())
                .collect();
            if !keys.is_empty() {
                return Some(Reach::Keys(keys));
            }
        }
        if let Some(entry) = self.find_entry(request) {
            return Some(Reach::Mapped(entry.clone()));
        }
        if !self.is_own() {
            return None;
        }
        let listed = |articulation: Option<Articulation>| {
            articulation.is_none_or(|articulation| {
                self.names.as_ref().is_some_and(|names| {
                    names.contains(&ArticulationName::Vocabulary(articulation))
                })
            })
        };
        let plays = request == Request::default()
            || (listed(request.primary())
                && listed(request.mute().articulation())
                && listed(request.vibrato().articulation()));
        plays.then_some(Reach::Own)
    }

    /// The mapped articulation that plays `request`: the one named so, with the passage's mute,
    /// and with its vibrato unless the passage's is the ordinary one, which any vibrato serves
    /// (the ordinary one first). The ordinary sound is an entry named `ordinario` or after a mute
    /// or a vibrato, else the entry marked as the default; for a passage with a mute or a
    /// vibrato, any entry that has them.
    fn find_entry(&self, request: Request) -> Option<&'a ArticulationEntry> {
        let passing = || {
            self.entries.iter().filter(move |(plays, _)| {
                plays.mute() == request.mute()
                    && (request.vibrato() == Vibrato::Ordinary
                        || plays.vibrato() == request.vibrato())
            })
        };
        let found = if let Some(articulation) = request.primary() {
            let named =
                || passing().filter(move |(plays, _)| plays.primary() == Some(articulation));
            named()
                .find(|(plays, _)| plays.vibrato() == request.vibrato())
                .or_else(|| named().next())
        } else {
            let ordinary = passing()
                .find(|(plays, _)| plays.primary().is_none())
                .or_else(|| passing().find(|(_, entry)| entry.is_default));
            if ordinary.is_none() && request != Request::default() {
                passing().next()
            } else {
                ordinary
            }
        };
        found.map(|(_, entry)| *entry)
    }

    /// The sound's ordinary sound and what it plays: the key map's keys without an articulation
    /// (else its first row), else the mapped articulation named `ordinario` (else the default,
    /// else the first), else the sound's own.
    fn ordinary(&self) -> (Reach, Request) {
        if !self.keymap.is_empty() {
            let keys: Vec<u8> = self
                .keymap
                .iter()
                .filter(|row| row_primary(row).is_none())
                .flat_map(|row| row.keys.iter().copied())
                .collect();
            if !keys.is_empty() {
                return (Reach::Keys(keys), Request::default());
            }
            if let Some(row) = self.keymap.first() {
                let name = row.articulation.unwrap_or(ArticulationName::Ordinario);
                return (
                    Reach::Keys(row.keys.clone()),
                    named(name, Mute::Open, Vibrato::Ordinary),
                );
            }
        }
        let entry = self
            .entries
            .iter()
            .find(|(_, entry)| entry.name == ArticulationName::Ordinario)
            .or_else(|| self.entries.iter().find(|(_, entry)| entry.is_default))
            .or_else(|| self.entries.first());
        match entry {
            Some((plays, entry)) => (Reach::Mapped((*entry).clone()), *plays),
            None => (Reach::Own, Request::default()),
        }
    }

    /// Step 2 of the chain for `requested`; see [`nearest_articulation`].
    fn nearest(&self, requested: Request) -> ArticulationChoice {
        let mutes = mute_tries(requested.mute());
        for (i, primary) in primary_tries(requested.primary()).into_iter().enumerate() {
            for (j, (mute, mute_rewrite)) in mutes.iter().enumerate() {
                let vibratos = vibrato_tries(requested.vibrato(), primary.vibrato);
                for (k, vibrato) in vibratos.into_iter().enumerate() {
                    let Ok(tried) = Request::new(primary.articulation, *mute, vibrato) else {
                        continue;
                    };
                    let Some(reach) = self.find(tried) else {
                        continue;
                    };
                    let plays = played(&reach, tried);
                    let substitute = (i, j, k) != (0, 0, 0);
                    return ArticulationChoice {
                        requested,
                        reach,
                        substitute: substitute.then(|| Substitute {
                            plays,
                            rewrites: primary.rewrite.into_iter().chain(*mute_rewrite).collect(),
                            only: primary.only,
                        }),
                    };
                }
            }
        }
        let (reach, plays) = self.ordinary();
        ArticulationChoice {
            requested,
            reach,
            substitute: Some(Substitute {
                plays,
                rewrites: Vec::new(),
                only: None,
            }),
        }
    }
}

/// The request an articulation entry or a key map row plays: its primary articulation, else the
/// ordinary sound, with the mute and the vibrato it names or carries.
fn named(name: ArticulationName, mute: Mute, vibrato: Vibrato) -> Request {
    let (primary, mute, vibrato) = match name {
        ArticulationName::Ordinario => (None, mute, vibrato),
        ArticulationName::Vocabulary(articulation) => match articulation.axis() {
            Axis::Primary => (Some(articulation), mute, vibrato),
            Axis::Mute(named_mute) if mute == Mute::Open => (None, named_mute, vibrato),
            Axis::Vibrato(named_vibrato) if vibrato == Vibrato::Ordinary => {
                (None, mute, named_vibrato)
            }
            Axis::Mute(_) | Axis::Vibrato(_) => (None, mute, vibrato),
        },
    };
    // The primary is of the primary axis, so the request is valid; the fallback is unreachable.
    Request::new(primary, mute, vibrato).unwrap_or_default()
}

/// What plays when `reach` serves the request `tried`: the mapped articulation as its entry
/// names it, else the request.
fn played(reach: &Reach, tried: Request) -> Request {
    match reach {
        Reach::Mapped(entry) => named(entry.name, entry.mute, entry.vibrato),
        Reach::Keys(_) | Reach::Own => tried,
    }
}

/// The primary articulation a key map row plays; `None` for the ordinary sound.
fn row_primary(row: &KeymapEntry) -> Option<Articulation> {
    row.articulation
        .map_or_else(Request::default, |name| {
            named(name, Mute::Open, Vibrato::Ordinary)
        })
        .primary()
}

/// One primary articulation step 2 tries, with what comes with it.
struct PrimaryTry {
    articulation: Option<Articulation>,
    rewrite: Option<Emulation>,
    vibrato: Option<Vibrato>,
    only: Option<Scope>,
}

/// The primary articulations step 2 tries for `requested`, in order: itself, the articulation
/// it refines, the stand-ins of its substitution row (or of the row of the articulation it
/// refines) that the same sound can play, then the ordinary sound with the row's emulation.
fn primary_tries(requested: Option<Articulation>) -> Vec<PrimaryTry> {
    let plain = |articulation| PrimaryTry {
        articulation,
        rewrite: None,
        vibrato: None,
        only: None,
    };
    let mut tries = vec![plain(requested)];
    let Some(articulation) = requested else {
        return tries;
    };
    let parent = articulation.parent();
    if parent.is_some() {
        tries.push(plain(parent));
    }
    let row =
        fallback::substitution(articulation).or_else(|| parent.and_then(fallback::substitution));
    if let Some(row) = row {
        for substitute in row.substitutes {
            if let StandIn::Articulation(stand_in) = substitute.stand_in {
                tries.push(PrimaryTry {
                    articulation: Some(stand_in),
                    rewrite: substitute.rewrite,
                    vibrato: substitute.vibrato,
                    only: substitute.only,
                });
            }
        }
        if let Some(emulation) = row.emulation {
            tries.push(PrimaryTry {
                articulation: None,
                rewrite: Some(emulation),
                vibrato: None,
                only: None,
            });
        }
    }
    tries
}

/// The mutes step 2 tries for `requested`, each with the rewrite it brings: the mute itself,
/// the more general mutes it refines, then no mute with the emulation of the nearest of those
/// that has a substitution row.
fn mute_tries(requested: Mute) -> Vec<(Mute, Option<Emulation>)> {
    let mut tries = vec![(requested, None)];
    if requested == Mute::Open {
        return tries;
    }
    let lineage: Vec<Articulation> =
        std::iter::successors(requested.articulation(), |articulation| {
            articulation.parent()
        })
        .take(Articulation::ALL.len())
        .collect();
    for articulation in lineage.iter().skip(1) {
        if let Axis::Mute(mute) = articulation.axis() {
            tries.push((mute, None));
        }
    }
    let emulation = lineage
        .iter()
        .find_map(|articulation| fallback::substitution(*articulation))
        .and_then(|row| row.emulation);
    tries.push((Mute::Open, emulation));
    tries
}

/// The vibratos step 2 tries: the stand-in's own, when it names one, then the passage's, then
/// the ordinary one, each once.
fn vibrato_tries(requested: Vibrato, stand_in: Option<Vibrato>) -> Vec<Vibrato> {
    let mut tries: Vec<Vibrato> = Vec::new();
    for vibrato in [stand_in.unwrap_or(requested), requested, Vibrato::Ordinary] {
        if !tries.contains(&vibrato) {
            tries.push(vibrato);
        }
    }
    tries
}

/// A playable preset of an instrument, with how it plays the part.
#[derive(Debug, Clone)]
struct Candidate<'a> {
    preset: &'a InstrumentPreset,
    instrument: InstrumentType,
    choices: Vec<ArticulationChoice>,
    verdict: Option<Verdict>,
    /// Whether its stated size is the other one than the size asked for.
    other_size: bool,
    /// Whether its sampled keys hold the part's notes, or are not known.
    sampled: bool,
    /// Whether a playable preset of the size asked for was among the candidates; set by
    /// [`best`].
    size_installed: bool,
}

impl Candidate<'_> {
    fn exact(&self) -> usize {
        self.choices
            .iter()
            .filter(|choice| choice.substitute.is_none())
            .count()
    }

    /// As is, then partly, then not judged.
    fn verdict_rank(&self) -> u8 {
        match self.verdict.as_ref().map(|verdict| verdict.state) {
            Some(VerdictState::AsIs) => 0,
            Some(VerdictState::Partly) => 1,
            None | Some(VerdictState::Not) => 2,
        }
    }

    /// The library's own word, then Sonara's profile, then a guess from the path.
    fn source_rank(&self) -> u8 {
        let source = self
            .preset
            .instrument
            .filter(|claim| claim.instrument == self.instrument)
            .map(|claim| claim.source)
            .or_else(|| self.preset.keymap.as_ref().map(|claim| claim.source));
        match source {
            Some(ClaimSource::Declared) => 0,
            Some(ClaimSource::Profile) => 1,
            Some(ClaimSource::Inferred) | None => 2,
        }
    }

    fn table_rank(&self) -> usize {
        tables::preference(self.instrument, self.preset.id.as_str()).unwrap_or(usize::MAX)
    }

    /// The order of the module documentation: lower is better.
    fn compare(&self, other: &Self, preferences: ChainPreferences) -> Ordering {
        let size = self.other_size.cmp(&other.other_size);
        let exact = other.exact().cmp(&self.exact());
        let first = if preferences.keep_articulations {
            exact.then(size)
        } else {
            size.then(exact)
        };
        first
            .then_with(|| self.verdict_rank().cmp(&other.verdict_rank()))
            .then_with(|| other.sampled.cmp(&self.sampled))
            .then_with(|| self.source_rank().cmp(&other.source_rank()))
            .then_with(|| self.table_rank().cmp(&other.table_rank()))
            .then_with(|| self.preset.id.cmp(&other.preset.id))
    }
}

/// The playable presets of `presets` that play `instrument`, each with how it plays `requests`.
/// A preset whose verdict, with those of a set's members, is [`VerdictState::Not`] is left out.
fn candidates<'a>(
    presets: &'a [InstrumentPreset],
    instrument: InstrumentType,
    requests: &[Request],
    wish: Option<EnsembleKind>,
    span: Option<PitchRange>,
) -> Vec<Candidate<'a>> {
    presets
        .iter()
        .filter(|preset| plays(preset, instrument) && preset.is_playable())
        .filter_map(|preset| {
            let verdict = verdict_of(preset, presets);
            if verdict
                .as_ref()
                .is_some_and(|verdict| verdict.state == VerdictState::Not)
            {
                return None;
            }
            let sound = Sound::of(preset, instrument);
            let choices: Vec<ArticulationChoice> = requests
                .iter()
                .map(|request| sound.nearest(*request))
                .collect();
            let sampled = holds_span(preset, &choices, span);
            let other_size = match (wish, preset.ensemble.as_ref()) {
                (Some(wish), Some(claim)) => is_section(wish) != is_section(claim.value.kind),
                _ => false,
            };
            Some(Candidate {
                preset,
                instrument,
                choices,
                verdict,
                other_size,
                sampled,
                size_installed: !other_size,
            })
        })
        .collect()
}

/// The best of `candidates`, in the order of the module documentation.
fn best(candidates: Vec<Candidate<'_>>, preferences: ChainPreferences) -> Option<Candidate<'_>> {
    let size_installed = candidates.iter().any(|candidate| !candidate.other_size);
    candidates
        .into_iter()
        .min_by(|a, b| a.compare(b, preferences))
        .map(|candidate| Candidate {
            size_installed,
            ..candidate
        })
}

/// Whether `preset` plays `instrument`: its manifest or profile says so, or its key map has keys
/// for it.
fn plays(preset: &InstrumentPreset, instrument: InstrumentType) -> bool {
    preset
        .instrument
        .is_some_and(|claim| claim.instrument == instrument)
        || preset
            .keymap
            .as_ref()
            .is_some_and(|claim| claim.value.iter().any(|row| row.instrument == instrument))
}

const fn is_section(kind: EnsembleKind) -> bool {
    matches!(kind, EnsembleKind::Section | EnsembleKind::SectionEmulated)
}

/// The verdict on `preset`: its own effective verdict; for a set that has none, the verdicts of
/// its members' own presets in `presets`, the worst state with each loss led by the member's
/// path. `None` when nothing is judged.
fn verdict_of(preset: &InstrumentPreset, presets: &[InstrumentPreset]) -> Option<Verdict> {
    if let Some(verdict) = preset.effective_verdict() {
        return Some(verdict);
    }
    let PresetKind::Set { members } = &preset.kind else {
        return None;
    };
    let mut merged: Option<Verdict> = None;
    for member in members {
        let id = InstrumentId::new(&preset.library, &member.path);
        let Some(verdict) = presets
            .iter()
            .find(|other| other.id == id)
            .and_then(InstrumentPreset::effective_verdict)
        else {
            continue;
        };
        let merged = merged.get_or_insert_with(|| Verdict {
            state: VerdictState::AsIs,
            lost: Vec::new(),
        });
        merged.state = merged.state.worse(verdict.state);
        for loss in verdict.lost {
            let loss = format!("{}: {loss}", member.path);
            if !merged.lost.contains(&loss) {
                merged.lost.push(loss);
            }
        }
    }
    merged
}

/// Whether the keys of `preset` that play `choices` sound every note of `span`, the notes moved
/// by the preset's key offset; `true` when there is no span or the keys are not known. A set
/// checks the members that play the choices, or all of them when no choice names one.
fn holds_span(
    preset: &InstrumentPreset,
    choices: &[ArticulationChoice],
    span: Option<PitchRange>,
) -> bool {
    let Some(span) = span else {
        return true;
    };
    let offset = preset
        .key_offset
        .as_ref()
        .map_or(0, |claim| i64::from(claim.value));
    let holds = |facts: &PatchFacts| {
        (span.low().get()..=span.high().get()).all(|pitch| {
            let key = pitch.saturating_add(offset);
            facts
                .sampled
                .iter()
                .any(|zone| i64::from(zone.lo) <= key && key <= i64::from(zone.hi))
        })
    };
    match &preset.kind {
        PresetKind::Patch => preset.facts.as_ref().is_none_or(holds),
        PresetKind::Set { members } => {
            let playing: Vec<&str> = choices
                .iter()
                .filter_map(|choice| match &choice.reach {
                    Reach::Mapped(entry) => entry.member.as_deref(),
                    Reach::Keys(_) | Reach::Own => None,
                })
                .collect();
            members
                .iter()
                .filter(|member| {
                    playing.is_empty() || playing.iter().any(|path| same_path(path, &member.path))
                })
                .all(|member| member.facts.as_ref().is_none_or(holds))
        }
    }
}

/// The playable range of `instrument`, or `None` when it is unpitched.
fn playable(instrument: InstrumentType) -> Option<PitchRange> {
    match instrument.get().pitch() {
        PitchModel::Pitched(pitched) => Some(pitched.playable.range),
        PitchModel::Unpitched => None,
    }
}

/// The requests a neighbour plays in `manner`: each in the articulation the manner names, or
/// as they are.
fn adjusted(requests: &[Request], manner: Manner) -> Vec<Request> {
    match manner {
        Manner::With(articulation) => requests
            .iter()
            .map(|request| {
                Request::new(Some(articulation), request.mute(), request.vibrato())
                    .unwrap_or(*request)
            })
            .collect(),
        Manner::AsWritten | Manner::Softer | Manner::OnHighestNote | Manner::OnLowestNote => {
            requests.to_vec()
        }
    }
}

/// A sound below steps 1 to 3, in the order they are offered.
enum Fallback<'a> {
    Neighbour {
        neighbour: Neighbour,
        candidate: Candidate<'a>,
        out_of_range: bool,
    },
    Builtin(&'static tables::BuiltinRow),
}

/// The neighbours with a playable preset (step 4): those whose playable range holds the part's
/// notes, in order of preference, then the first one whose range does not.
fn neighbour_fallbacks<'a>(
    presets: &'a [InstrumentPreset],
    request: &SoundRequest,
    requests: &[Request],
    preferences: ChainPreferences,
) -> Vec<Fallback<'a>> {
    let span = request.pitched_span();
    let mut fitting = Vec::new();
    let mut beyond = None;
    for neighbour in fallback::neighbours(request.instrument) {
        let range = playable(neighbour.instrument);
        let neighbour_span = span.filter(|_| range.is_some());
        let found = best(
            candidates(
                presets,
                neighbour.instrument,
                &adjusted(requests, neighbour.manner),
                request.ensemble,
                neighbour_span,
            ),
            preferences,
        );
        let Some(candidate) = found else {
            continue;
        };
        let fits = match (neighbour_span, range) {
            (Some(span), Some(range)) => range.encloses(span),
            _ => true,
        };
        if fits {
            fitting.push(Fallback::Neighbour {
                neighbour: *neighbour,
                candidate,
                out_of_range: false,
            });
        } else if beyond.is_none() {
            beyond = Some(Fallback::Neighbour {
                neighbour: *neighbour,
                candidate,
                out_of_range: true,
            });
        }
    }
    fitting.extend(beyond);
    fitting
}

/// The sound plan of the first of `fallbacks`; when it asks first, the sound plan of the rest is
/// the answer "no". Silence when none is left.
fn plan_from<'a>(
    fallbacks: &mut impl Iterator<Item = Fallback<'a>>,
    request: &SoundRequest,
    requests: &[Request],
) -> SoundPlan {
    match fallbacks.next() {
        Some(Fallback::Neighbour {
            neighbour,
            candidate,
            out_of_range,
        }) => {
            let question = words::neighbour_question(
                request.instrument,
                &neighbour,
                &candidate.preset.id,
                if out_of_range {
                    request.pitched_span()
                } else {
                    None
                },
            );
            let mut plan = sampler_plan(candidate, request, requests, Some(neighbour));
            if let Some(question) = question {
                plan.confirm = Some(Confirmation {
                    question,
                    otherwise: Box::new(plan_from(fallbacks, request, requests)),
                });
            }
            plan
        }
        Some(Fallback::Builtin(row)) => {
            let instrument = request.instrument;
            let articulations: Vec<ArticulationChoice> = requests
                .iter()
                .map(|request| Sound::default().nearest(*request))
                .collect();
            let mut notices = vec![Notice {
                step: Step::Builtin,
                message: words::builtin(instrument, row.sound),
            }];
            notices.extend(articulation_notices(&articulations, instrument));
            let confirm = row.asks_first.then(|| Confirmation {
                question: words::builtin_question(instrument, row.sound),
                otherwise: Box::new(plan_from(fallbacks, request, requests)),
            });
            SoundPlan {
                step: Step::Builtin,
                source: SoundSource::Builtin(row.sound),
                neighbour: None,
                articulations,
                notices,
                confirm,
            }
        }
        None => SoundPlan {
            step: Step::Silence,
            source: SoundSource::Silence,
            neighbour: None,
            articulations: Vec::new(),
            notices: vec![Notice {
                step: Step::Silence,
                message: words::silence(request.instrument),
            }],
            confirm: None,
        },
    }
}

/// The sound plan of a sampler preset: of the instrument itself (steps 1 to 3), or of `neighbour`
/// (step 4), whose choices were made for the requests as its manner plays them and are given
/// back for the part's own `requests`.
fn sampler_plan(
    candidate: Candidate<'_>,
    request: &SoundRequest,
    requests: &[Request],
    neighbour: Option<Neighbour>,
) -> SoundPlan {
    let Candidate {
        preset,
        instrument,
        choices,
        verdict,
        other_size,
        sampled,
        size_installed,
    } = candidate;
    let mut notices = Vec::new();
    if let Some(neighbour) = &neighbour {
        notices.push(Notice {
            step: Step::FamilyNeighbour,
            message: words::neighbour(request.instrument, neighbour, &preset.id),
        });
    }
    if other_size && let Some(wish) = request.ensemble {
        notices.push(Notice {
            step: Step::EnsembleSwap,
            message: words::ensemble_swap(instrument, wish, size_installed),
        });
    }
    if let Some(verdict) = verdict.filter(|verdict| verdict.state == VerdictState::Partly) {
        notices.push(Notice {
            step: Step::Exact,
            message: words::partly(preset, &verdict),
        });
    }
    if let Some(span) = request.pitched_span().filter(|_| !sampled) {
        notices.push(Notice {
            step: Step::Exact,
            message: words::unsampled(preset, span),
        });
    }
    notices.extend(articulation_notices(&choices, instrument));
    let articulations = match neighbour {
        Some(neighbour) => choices
            .into_iter()
            .zip(requests)
            .map(|(choice, requested)| for_part(choice, *requested, neighbour.manner))
            .collect(),
        None => choices,
    };
    let step = if neighbour.is_some() {
        Step::FamilyNeighbour
    } else if other_size {
        Step::EnsembleSwap
    } else {
        Step::Exact
    };
    SoundPlan {
        step,
        source: SoundSource::Sampler {
            preset: preset.id.clone(),
        },
        neighbour,
        articulations,
        notices,
        confirm: None,
    }
}

/// A neighbour's `choice` given back for the part's own `requested`: what the manner changes
/// is a substitute, and a softer manner adds its rewrite.
fn for_part(choice: ArticulationChoice, requested: Request, manner: Manner) -> ArticulationChoice {
    let ArticulationChoice {
        requested: adjusted,
        reach,
        substitute,
    } = choice;
    let softer = matches!(manner, Manner::Softer);
    let substitute = if substitute.is_none() && adjusted == requested && !softer {
        None
    } else {
        let mut substitute = substitute.unwrap_or_else(|| Substitute {
            plays: played(&reach, adjusted),
            rewrites: Vec::new(),
            only: None,
        });
        if softer {
            substitute.rewrites.push(Emulation::Soften);
        }
        Some(substitute)
    };
    ArticulationChoice {
        requested,
        reach,
        substitute,
    }
}

/// The step 2 notices of `choices`, when `instrument` plays them.
fn articulation_notices(
    choices: &[ArticulationChoice],
    instrument: InstrumentType,
) -> impl Iterator<Item = Notice> {
    choices
        .iter()
        .filter_map(move |choice| choice.message(instrument))
        .map(|message| Notice {
            step: Step::NearestArticulation,
            message,
        })
}
