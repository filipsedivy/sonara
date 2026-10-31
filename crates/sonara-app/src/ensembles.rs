//! Ensemble plans: what applying an ensemble template to a project builds, worked out once so that
//! every caller builds the same tracks.
//!
//! An ensemble template of the orchestration metadata
//! ([`sonara_midi::orchestration::Ensemble`]) names instrument types only. [`plan`] turns one into
//! the tracks to create, in the order to create them:
//!
//! - **The parts** the request selects (the required ones by default), one track each, in the
//!   order the template's score prints them, named as the score names them ("Violins I",
//!   "Piccolo (Fl. 3)"). Each carries the part it plays as project data ([`PartPlan::track_part`]):
//!   the instrument type, the name, the players (a string section's from the chosen string size)
//!   and, for a doubled instrument, the part of the player who plays it.
//! - **A folder per family** in use, when asked for, holding that family's parts. The families
//!   come in the order their first part appears, which for a full score is score order; a
//!   family's parts all go into its folder, so in a wind quintet the bassoon joins the flute,
//!   oboe and clarinet above the horn. Without folders the parts keep the template's order
//!   exactly.
//! - **A bus per family** in use, when asked for, which the family's parts feed instead of the
//!   master, so that a family can be balanced and processed as a group. The buses come first,
//!   because a track can feed only a bus that already exists.
//! - **The sound of each part**, from the degradation chain ([`choose_sound`]) over the catalog's
//!   presets, asked for solo or section by the part's players: the whole [`SoundPlan`], with what
//!   it substituted in words and any question to ask first.
//! - **Where each part sits**: its stage place in the chosen seating as a track pan, unless the
//!   chosen preset was recorded with the players in their places ([`Placement::InSitu`]), whose
//!   samples carry the stage image already.
//!
//! The plan creates nothing, reads no project and mints no ID. A caller applies it track by track
//! in its order: a project edit as commands of one undo step, or the orchestration planner as
//! agent tool calls, each with the IDs the earlier tracks got. A hall reverb, which depends on
//! what the project already has, is the caller's to add.
//!
//! It is a pure function over a snapshot of the catalog: no I/O, no audio engine, never on the
//! real-time thread (RT-01).

#[cfg(test)]
mod tests;

use sonara_core::model::part::{Notation, Part as TrackPart};
use sonara_midi::orchestration::{
    Ensemble, EnsembleId, Family, Part, PitchRange, Players, Request, Seating, StringSize,
};
use thiserror::Error;

use crate::instruments::{
    ChainPreferences, EnsembleKind, InstrumentPreset, Notice, SoundPlan, SoundRequest, SoundSource,
    choose_sound,
};

/// What to plan: an ensemble template and the choices its form offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnsembleRequest {
    /// The template.
    pub ensemble: EnsembleId,
    /// The seating, or `None` for the template's default
    /// ([`Ensemble::default_seating`]). Only a seating the template offers is accepted.
    pub seating: Option<Seating>,
    /// The slug of the string size, such as `medium`, or `None` for the template's default
    /// ([`Ensemble::default_string_size`]).
    pub string_size: Option<String>,
    /// The parts to create.
    pub parts: PartSelection,
    /// Whether the parts go into a folder per family.
    pub folders: bool,
    /// Whether the parts of each family feed a bus of their own.
    pub buses: bool,
    /// The preferences of the degradation chain.
    pub preferences: ChainPreferences,
}

impl EnsembleRequest {
    /// A request for the required parts of `ensemble` in its default seating and string size,
    /// with folders and buses by family.
    #[must_use]
    pub fn new(ensemble: EnsembleId) -> Self {
        Self {
            ensemble,
            seating: None,
            string_size: None,
            parts: PartSelection::Required,
            folders: true,
            buses: true,
            preferences: ChainPreferences::default(),
        }
    }
}

/// Which parts of a template to create.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartSelection {
    /// The parts every work of the kind needs ([`Part::required`]), each asked for its ordinary
    /// sound over any range.
    Required,
    /// These parts, each with what it plays. The plan keeps the template's order, whatever the
    /// order here.
    Only(Vec<PartChoice>),
}

/// One part to create, and what it plays, for the degradation chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartChoice {
    /// The part's slug in the template, such as `horn_3`.
    pub slug: String,
    /// The articulations the part plays; empty for its ordinary sound alone.
    pub articulations: Vec<Request>,
    /// The lowest and the highest sounding pitch of the part, or `None` when not known.
    pub span: Option<PitchRange>,
}

impl PartChoice {
    /// The part `slug`, playing its ordinary sound over any range.
    #[must_use]
    pub fn new(slug: impl Into<String>) -> Self {
        Self {
            slug: slug.into(),
            articulations: Vec::new(),
            span: None,
        }
    }
}

/// Why a request cannot be planned.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EnsemblePlanError {
    /// The template does not offer the seating; a template without a choice of seating offers
    /// none.
    #[error("the ensemble '{}' does not offer the {seating:?} seating", .ensemble.id())]
    UnsupportedSeating {
        /// The template.
        ensemble: EnsembleId,
        /// The seating asked for.
        seating: Seating,
    },
    /// The template has no string size with the slug; a template without strings has none.
    #[error("the ensemble '{}' has no string size '{slug}'", .ensemble.id())]
    UnknownStringSize {
        /// The template.
        ensemble: EnsembleId,
        /// The slug asked for.
        slug: String,
    },
    /// The template has no part with the slug.
    #[error("the ensemble '{}' has no part '{slug}'", .ensemble.id())]
    UnknownPart {
        /// The template.
        ensemble: EnsembleId,
        /// The slug asked for.
        slug: String,
    },
    /// The selection names a part twice.
    #[error("the part '{0}' is selected twice")]
    DuplicatePart(String),
    /// The selection names no part.
    #[error("no part is selected")]
    NoParts,
}

/// The tracks that applying an ensemble template creates, in the order to create them.
#[derive(Debug, Clone, PartialEq)]
pub struct EnsemblePlan {
    /// The template.
    pub ensemble: &'static Ensemble,
    /// The seating the pans follow; `None` for a template without a choice of seating.
    pub seating: Option<Seating>,
    /// The size of the string body; `None` for a template without string sections.
    pub string_size: Option<&'static StringSize>,
    /// The tracks: the family buses, then each family's folder followed by its parts, or the
    /// parts alone without folders. A track refers to earlier tracks only, by their index here.
    pub tracks: Vec<PlannedTrack>,
}

impl EnsemblePlan {
    /// The planned parts, in the order of [`Self::tracks`].
    pub fn parts(&self) -> impl Iterator<Item = &PartPlan> {
        self.tracks.iter().filter_map(|track| match &track.kind {
            PlannedKind::Part(part) => Some(part.as_ref()),
            PlannedKind::Folder(_) | PlannedKind::Bus(_) => None,
        })
    }

    /// What the degradation chain substituted, part by part in the order of [`Self::parts`]: the
    /// part's name and each of its sound's notices, in their order.
    pub fn notices(&self) -> impl Iterator<Item = (&'static str, &Notice)> {
        self.parts().flat_map(|plan| {
            plan.sound
                .notices
                .iter()
                .map(|notice| (plan.part.name, notice))
        })
    }
}

/// One track to create.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedTrack {
    /// The track's name: the part's name, or the family's folder or bus name ([`folder_name`],
    /// [`bus_name`]).
    pub name: &'static str,
    /// The folder the track belongs to, as the index of an earlier folder in
    /// [`EnsemblePlan::tracks`]; `None` at the top level.
    pub parent: Option<usize>,
    /// The bus the track feeds, as the index of an earlier bus in [`EnsemblePlan::tracks`];
    /// `None` for the master. A folder, which carries no audio, feeds the master.
    pub output: Option<usize>,
    /// What the track is.
    pub kind: PlannedKind,
}

/// What a planned track is.
#[derive(Debug, Clone, PartialEq)]
pub enum PlannedKind {
    /// The folder of a family.
    Folder(Family),
    /// The bus of a family, feeding the master.
    Bus(Family),
    /// An instrument track playing one part.
    Part(Box<PartPlan>),
}

/// One part of the plan: the part, what it is as project data, its sound and its place.
#[derive(Debug, Clone, PartialEq)]
pub struct PartPlan {
    /// The part of the template.
    pub part: &'static Part,
    /// Solo or section, as the sound was asked for: a section for a string section or a part of
    /// several players of its own (a choir section), else solo.
    pub size: EnsembleKind,
    /// The part as the track stores it.
    pub track_part: TrackPart,
    /// What plays the part, by the degradation chain.
    pub sound: SoundPlan,
    /// The part's stage place in the plan's seating, as a track pan from −1 to +1, to four
    /// decimal places.
    pub stage_pan: f64,
    /// Where the track sits.
    pub placement: Placement,
}

impl PartPlan {
    /// Takes the answer "no" to the question of the part's sound
    /// ([`SoundPlan::confirm`]): the sound becomes the one that plays instead, and the placement
    /// follows it, with the catalog's `presets`. Returns `false`, changing nothing, when the
    /// sound asks no question.
    pub fn decline(&mut self, presets: &[InstrumentPreset]) -> bool {
        let Some(confirmation) = self.sound.confirm.take() else {
            return false;
        };
        self.sound = *confirmation.otherwise;
        self.placement = placement(self.stage_pan, &self.sound.source, presets);
        true
    }
}

/// Where a part's track sits in the stereo image.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    /// Pan the track to this value, −1 (left) to +1 (right), seen from the audience: the part's
    /// stage place, for a dry or centred source (a built-in sound, a preset not recorded in
    /// place, silence).
    Pan(f64),
    /// Leave the track centred: the preset was recorded with the players in their places, so its
    /// samples carry the stage image.
    InSitu,
}

/// The name of a family's folder: "Woodwinds", "Brass", "Percussion", "Plucked", "Keyboards",
/// "Voices", "Strings" or "Other".
#[must_use]
pub const fn folder_name(family: Family) -> &'static str {
    match family {
        Family::Woodwinds => "Woodwinds",
        Family::Brass => "Brass",
        Family::Percussion => "Percussion",
        Family::Plucked => "Plucked",
        Family::Keyboards => "Keyboards",
        Family::Choir => "Voices",
        Family::Strings => "Strings",
        Family::Other => "Other",
    }
}

/// The name of a family's bus: the folder's name followed by "Bus", such as "Strings Bus", so
/// that the two tracks are told apart.
#[must_use]
pub const fn bus_name(family: Family) -> &'static str {
    match family {
        Family::Woodwinds => "Woodwinds Bus",
        Family::Brass => "Brass Bus",
        Family::Percussion => "Percussion Bus",
        Family::Plucked => "Plucked Bus",
        Family::Keyboards => "Keyboards Bus",
        Family::Choir => "Voices Bus",
        Family::Strings => "Strings Bus",
        Family::Other => "Other Bus",
    }
}

/// The plan of `request` over the catalog's `presets` (usually
/// [`crate::instruments::InstrumentCatalog::presets`]); see the module documentation for what
/// it holds.
///
/// # Errors
///
/// [`EnsemblePlanError`], checked in this order:
/// [`UnsupportedSeating`](EnsemblePlanError::UnsupportedSeating) for a seating the template does
/// not offer; [`UnknownStringSize`](EnsemblePlanError::UnknownStringSize) for a string size it
/// does not have; then, choice by choice, [`UnknownPart`](EnsemblePlanError::UnknownPart) and
/// [`DuplicatePart`](EnsemblePlanError::DuplicatePart); last
/// [`NoParts`](EnsemblePlanError::NoParts) for an empty selection.
pub fn plan(
    presets: &[InstrumentPreset],
    request: &EnsembleRequest,
) -> Result<EnsemblePlan, EnsemblePlanError> {
    let ensemble = request.ensemble.get();
    let seating = seating(ensemble, request.seating)?;
    let string_size = string_size(ensemble, request.string_size.as_deref())?;
    let selected = selected_parts(ensemble, &request.parts)?;

    let mut families: Vec<Family> = Vec::new();
    for (part, _) in &selected {
        let family = part.instrument.family();
        if !families.contains(&family) {
            families.push(family);
        }
    }

    let mut tracks = Vec::new();
    let mut buses = Vec::new();
    if request.buses {
        for &family in &families {
            buses.push((family, tracks.len()));
            tracks.push(PlannedTrack {
                name: bus_name(family),
                parent: None,
                output: None,
                kind: PlannedKind::Bus(family),
            });
        }
    }
    let bus_of = |family: Family| {
        buses
            .iter()
            .find(|(bus, _)| *bus == family)
            .map(|(_, index)| *index)
    };
    let part_track = |part: &'static Part, choice: Option<&PartChoice>, parent: Option<usize>| {
        let plan = part_plan(
            presets,
            request.preferences,
            ensemble,
            seating,
            string_size,
            part,
            choice,
        );
        PlannedTrack {
            name: part.name,
            parent,
            output: bus_of(part.instrument.family()),
            kind: PlannedKind::Part(Box::new(plan)),
        }
    };
    if request.folders {
        for &family in &families {
            let folder = tracks.len();
            tracks.push(PlannedTrack {
                name: folder_name(family),
                parent: None,
                output: None,
                kind: PlannedKind::Folder(family),
            });
            for &(part, choice) in &selected {
                if part.instrument.family() == family {
                    tracks.push(part_track(part, choice, Some(folder)));
                }
            }
        }
    } else {
        for &(part, choice) in &selected {
            tracks.push(part_track(part, choice, None));
        }
    }
    Ok(EnsemblePlan {
        ensemble,
        seating,
        string_size,
        tracks,
    })
}

/// The seating of the plan: the one asked for when the template offers it, else its default.
fn seating(
    ensemble: &Ensemble,
    asked: Option<Seating>,
) -> Result<Option<Seating>, EnsemblePlanError> {
    match asked {
        None => Ok(ensemble.default_seating()),
        Some(seating) if ensemble.seatings().contains(&seating) => Ok(Some(seating)),
        Some(seating) => Err(EnsemblePlanError::UnsupportedSeating {
            ensemble: ensemble.id(),
            seating,
        }),
    }
}

/// The string size of the plan: the one with the slug asked for, else the template's default.
fn string_size(
    ensemble: &Ensemble,
    asked: Option<&str>,
) -> Result<Option<&'static StringSize>, EnsemblePlanError> {
    match asked {
        None => Ok(ensemble.default_string_size()),
        Some(slug) => ensemble.string_size(slug).map(Some).ok_or_else(|| {
            EnsemblePlanError::UnknownStringSize {
                ensemble: ensemble.id(),
                slug: slug.to_owned(),
            }
        }),
    }
}

/// The selected parts in the template's order, each with its choice when the selection names
/// it.
fn selected_parts<'a>(
    ensemble: &Ensemble,
    selection: &'a PartSelection,
) -> Result<Vec<(&'static Part, Option<&'a PartChoice>)>, EnsemblePlanError> {
    let parts = ensemble.parts();
    let selected: Vec<_> = match selection {
        PartSelection::Required => parts
            .iter()
            .filter(|part| part.required)
            .map(|part| (part, None))
            .collect(),
        PartSelection::Only(choices) => {
            for (i, choice) in choices.iter().enumerate() {
                if ensemble.part(&choice.slug).is_none() {
                    return Err(EnsemblePlanError::UnknownPart {
                        ensemble: ensemble.id(),
                        slug: choice.slug.clone(),
                    });
                }
                if choices
                    .iter()
                    .take(i)
                    .any(|earlier| earlier.slug == choice.slug)
                {
                    return Err(EnsemblePlanError::DuplicatePart(choice.slug.clone()));
                }
            }
            parts
                .iter()
                .filter_map(|part| {
                    choices
                        .iter()
                        .find(|choice| choice.slug == part.slug)
                        .map(|choice| (part, Some(choice)))
                })
                .collect()
        }
    };
    if selected.is_empty() {
        return Err(EnsemblePlanError::NoParts);
    }
    Ok(selected)
}

/// The plan of one part.
fn part_plan(
    presets: &[InstrumentPreset],
    preferences: ChainPreferences,
    ensemble: &Ensemble,
    seating: Option<Seating>,
    string_size: Option<&StringSize>,
    part: &'static Part,
    choice: Option<&PartChoice>,
) -> PartPlan {
    let size = match part.players {
        Players::Section(_) => EnsembleKind::Section,
        Players::Own(range) if range.max() > 1 => EnsembleKind::Section,
        Players::Own(_) | Players::Doubling { .. } => EnsembleKind::Solo,
    };
    let request = SoundRequest {
        instrument: part.instrument,
        ensemble: Some(size),
        articulations: choice.map_or_else(Vec::new, |choice| choice.articulations.clone()),
        span: choice.and_then(|choice| choice.span),
    };
    let sound = choose_sound(presets, &request, preferences);
    let stage_pan = track_pan(part.pan.at(seating));
    PartPlan {
        part,
        size,
        track_part: track_part(ensemble, string_size, part, size),
        placement: placement(stage_pan, &sound.source, presets),
        sound,
        stage_pan,
    }
}

/// The part as a track stores it: its instrument type and name; its players (one for a part of
/// its own or a doubling, a string section's from the string size, none stated for a range such
/// as a choir section's 16 to 20); solo or section; and for a doubling, the name of the part of
/// the player who plays it. The notation is the instrument's.
fn track_part(
    ensemble: &Ensemble,
    string_size: Option<&StringSize>,
    part: &Part,
    size: EnsembleKind,
) -> TrackPart {
    let players = match part.players {
        Players::Own(range) => (range.min() == range.max()).then_some(range.min()),
        Players::Section(section) => string_size.map(|size| size.count(section)),
        Players::Doubling { .. } => Some(1),
    };
    TrackPart {
        instrument: part.instrument.id().to_owned(),
        name: Some(part.name.to_owned()),
        players,
        solo: Some(size == EnsembleKind::Solo),
        player: ensemble
            .player_of(part)
            .map(|player| player.name.to_owned()),
        notation: Notation::default(),
    }
}

/// Where a track whose stage place is `stage_pan` sits when `source` plays it: in its place,
/// unless the source is a preset of `presets` that was recorded in place.
fn placement(stage_pan: f64, source: &SoundSource, presets: &[InstrumentPreset]) -> Placement {
    let in_situ = match source {
        SoundSource::Sampler { preset } => presets
            .iter()
            .find(|candidate| candidate.id == *preset)
            .and_then(|preset| preset.in_situ.as_ref())
            .is_some_and(|claim| claim.value),
        SoundSource::Builtin(_) | SoundSource::Silence => false,
    };
    if in_situ {
        Placement::InSitu
    } else {
        Placement::Pan(stage_pan)
    }
}

/// A pan of the metadata as a track pan: the data writes at most two decimal places, so rounding
/// to four keeps the value written (−0.55, not the widened single-precision −0.550000011920929).
fn track_pan(pan: f32) -> f64 {
    (f64::from(pan) * 10_000.0).round() / 10_000.0
}
