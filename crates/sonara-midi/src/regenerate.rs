//! Localized regeneration of MIDI notes (SON-P11).
//!
//! [`propose`] resolves a bounded region of one MIDI clip, asks a [`RegionGenerator`] for
//! replacement notes, validates the candidate and returns a [`RegenerationPlan`]: a patch that
//! names the notes it removes and adds. Proposing never touches the project. [`apply`] commits
//! a plan as one undoable `Transaction` (`DeleteNotes` and `AddNotes`) through
//! [`ProjectSession::execute`], so the user can undo it like any other edit. [`regenerate`] does
//! both steps.
//!
//! # Region and write scope
//!
//! A region is a half-open interval `[start, end)` of timeline beats, or the span of a
//! section. It must lie inside the clip and the clip must not loop. Positions of notes are
//! relative to the clip's content (see `MidiClipData`), so the region is converted once to
//! content beats.
//!
//! - A note **belongs to the region** when its start lies in `[start, end)`. Only those notes
//!   are removed.
//! - A note that starts before `start` is preserved even when it sounds into the region. A note
//!   that starts in the region is replaced even when it sounds past `end`. Nothing is ever
//!   truncated. Preserved notes that cross a boundary are listed in
//!   [`RegenerationPlan::crossing`].
//! - A candidate note must start in `[start, end)` and end no later than `end`.
//!
//! The generator also receives read-only context (the preserved notes of the clip and the
//! notes of other MIDI tracks). Context never widens the write scope.
//!
//! # Identity, staleness and repeats
//!
//! The generator returns raw notes without IDs. Fresh IDs come from the [`IdGenerator`], so
//! retained notes keep theirs and new notes cannot collide. A plan records the session version
//! it was built from, and [`apply`] rejects it with [`RegenError::Stale`] when the session has
//! changed since. That also makes a second application of the same plan impossible, and a
//! retry after a failed application safe: a failed command leaves the session, and so the
//! version, unchanged.
//!
//! # Generation, cancellation and timeouts
//!
//! The generator interface is synchronous. Cancelling means not calling [`apply`]; it does not
//! stop a generator that is already running, and there is no timeout in this layer.

use std::collections::HashSet;
use std::fmt;

use sonara_core::EPSILON;
use sonara_core::commands::clip::{AddNotes, DeleteNotes};
use sonara_core::commands::{Command, Transaction};
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::ids::{self, IdGenerator};
use sonara_core::midi::MidiNote;
use sonara_core::model::clip::{Clip, MidiClip};
use sonara_core::model::project::Project;
use sonara_core::session::{CommandResult, ProjectSession};

/// The undo label of an applied regeneration.
pub const UNDO_LABEL: &str = "Regenerate Notes";

/// Why a regeneration did not happen.
#[derive(Debug, thiserror::Error)]
pub enum RegenError {
    /// The request is malformed or names an unsupported region.
    #[error("invalid regeneration request: {0}")]
    InvalidRequest(String),
    /// The clip, track or section does not exist, or is not a MIDI clip.
    #[error("regeneration target not found: {0}")]
    NotFound(String),
    /// The generator failed.
    #[error("generator failed: {0}")]
    Generation(#[from] GenerationError),
    /// The candidate broke hard rules; the project was not touched.
    #[error("generated candidate rejected: {}", .0.join("; "))]
    Rejected(Vec<String>),
    /// The session changed after the plan was made.
    #[error("the project changed since the plan was made (version {planned}, now {current})")]
    Stale {
        /// The version the plan was made from.
        planned: u64,
        /// The current version.
        current: u64,
    },
    /// The project command failed; the project was not touched.
    #[error("applying the regeneration failed: {0}")]
    Apply(#[from] DomainError),
}

impl RegenError {
    /// The agent-facing error code (`docs/agent-api.md`).
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidRequest(_) | Self::Rejected(_) | Self::Generation(_) => {
                ErrorCode::InvalidArgument
            }
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::Stale { .. } => ErrorCode::Conflict,
            Self::Apply(error) => error.code,
        }
    }
}

/// A failure inside a [`RegionGenerator`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct GenerationError(pub String);

/// What to regenerate.
#[derive(Debug, Clone, PartialEq)]
pub enum RegionTarget {
    /// `[start, end)` in timeline beats.
    Beats {
        /// First beat of the region.
        start: f64,
        /// First beat after the region.
        end: f64,
    },
    /// The span of a section, by ID.
    Section(String),
}

/// Rules the candidate must meet, in addition to the model's own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Constraints {
    /// Lowest and highest allowed pitch, both inclusive.
    pub pitch_range: Option<(u8, u8)>,
    /// Most notes the candidate may hold.
    pub max_notes: Option<usize>,
}

/// A request to regenerate part of one MIDI clip.
#[derive(Debug, Clone, PartialEq)]
pub struct RegenerationRequest {
    /// The MIDI clip to change.
    pub clip_id: String,
    /// The region to replace.
    pub target: RegionTarget,
    /// Rules for the candidate.
    pub constraints: Constraints,
    /// Seed for a deterministic generator.
    pub seed: Option<u64>,
}

/// A region in content beats of the clip: `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContentRegion {
    /// First beat.
    pub start: f64,
    /// First beat after the region.
    pub end: f64,
}

impl ContentRegion {
    fn owns(self, note: &MidiNote) -> bool {
        note.start >= self.start - EPSILON && note.start < self.end - EPSILON
    }
}

/// A note of another track, in content beats of the clip being regenerated.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextNote {
    /// The track the note plays on.
    pub track_id: String,
    /// MIDI pitch.
    pub pitch: u8,
    /// Start in content beats of the target clip.
    pub start: f64,
    /// Length in beats.
    pub length: f64,
}

/// A generated note, not yet validated and without an ID.
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateNote {
    /// MIDI pitch, 0–127.
    pub pitch: i64,
    /// Start in content beats.
    pub start: f64,
    /// Length in beats.
    pub length: f64,
    /// Velocity, 1–127.
    pub velocity: i64,
    /// MIDI channel, 0–15.
    pub channel: i64,
}

/// What a generator may read. Everything here is read-only context.
#[derive(Debug)]
pub struct GenerationInput<'a> {
    /// The track of the clip.
    pub track_id: &'a str,
    /// The clip.
    pub clip_id: &'a str,
    /// The region to fill, in content beats.
    pub region: ContentRegion,
    /// The notes of the clip that stay, including those that cross a boundary.
    pub preserved: &'a [MidiNote],
    /// Notes of other MIDI tracks that sound in the region.
    pub other_tracks: &'a [ContextNote],
    /// The caller's constraints.
    pub constraints: &'a Constraints,
    /// The caller's seed.
    pub seed: Option<u64>,
}

/// A source of replacement notes. Implementations own the musical algorithm; this module owns
/// validation and application.
pub trait RegionGenerator {
    /// Notes for the region of `input`.
    ///
    /// # Errors
    ///
    /// [`GenerationError`] when no candidate can be made.
    fn generate(&self, input: &GenerationInput<'_>) -> Result<Vec<CandidateNote>, GenerationError>;
}

/// An advisory finding; it does not stop the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning(pub String);

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated patch for one clip. It changes nothing until [`apply`] commits it.
#[derive(Debug, Clone)]
pub struct RegenerationPlan {
    /// The clip.
    pub clip_id: String,
    /// The track of the clip.
    pub track_id: String,
    /// The region, in content beats.
    pub region: ContentRegion,
    /// The session version the plan was made from.
    pub base_version: u64,
    /// The notes that are removed.
    pub removed: Vec<MidiNote>,
    /// The notes that are added, with their new IDs.
    pub added: Vec<MidiNote>,
    /// How many notes of the clip stay untouched.
    pub preserved: usize,
    /// IDs of preserved notes that start before the region and sound into it.
    pub crossing: Vec<String>,
    /// Advisory findings.
    pub warnings: Vec<Warning>,
}

impl RegenerationPlan {
    /// The project command that applies the plan as one undo step.
    #[must_use]
    pub fn command(&self) -> Transaction {
        let mut commands: Vec<Box<dyn Command>> = Vec::new();
        if !self.removed.is_empty() {
            commands.push(Box::new(DeleteNotes {
                clip_id: self.clip_id.clone(),
                note_ids: self.removed.iter().map(|n| n.id.clone()).collect(),
            }));
        }
        commands.push(Box::new(AddNotes {
            clip_id: self.clip_id.clone(),
            notes: self.added.clone(),
        }));
        Transaction {
            commands,
            name: UNDO_LABEL.to_owned(),
        }
    }
}

/// The outcome of a committed regeneration.
#[derive(Debug)]
pub struct RegenerationReport {
    /// What the session committed.
    pub command: CommandResult,
    /// IDs of the removed notes.
    pub removed: Vec<String>,
    /// IDs of the added notes.
    pub added: Vec<String>,
    /// How many notes of the clip stayed untouched.
    pub preserved: usize,
    /// Advisory findings.
    pub warnings: Vec<Warning>,
}

fn midi_clip<'a>(
    project: &'a Project,
    clip_id: &str,
) -> Result<(&'a str, &'a MidiClip), RegenError> {
    let (track, clip) = project
        .clip(clip_id)
        .map_err(|_| RegenError::NotFound(format!("clip {clip_id:?}")))?;
    match clip {
        Clip::Midi(clip) => Ok((track.id.as_str(), clip)),
        Clip::Audio(_) => Err(RegenError::NotFound(format!(
            "clip {clip_id:?} is not a MIDI clip"
        ))),
    }
}

/// Resolves the target to content beats of `clip`.
fn resolve_region(
    project: &Project,
    clip: &MidiClip,
    target: &RegionTarget,
) -> Result<ContentRegion, RegenError> {
    if clip.loop_length.is_some() {
        return Err(RegenError::InvalidRequest(
            "looping clips are not supported".to_owned(),
        ));
    }
    let (start, end) = match target {
        RegionTarget::Beats { start, end } => (*start, *end),
        RegionTarget::Section(id) => {
            let section = project
                .sections
                .iter()
                .find(|s| s.id == *id)
                .ok_or_else(|| RegenError::NotFound(format!("section {id:?}")))?;
            (section.start, section.start + section.length)
        }
    };
    if !(start.is_finite() && end.is_finite()) || end - start <= EPSILON {
        return Err(RegenError::InvalidRequest(
            "the region must be finite and not empty".to_owned(),
        ));
    }
    if start < clip.start - EPSILON || end > clip.start + clip.length + EPSILON {
        return Err(RegenError::InvalidRequest(format!(
            "the region [{start}, {end}) is not inside the clip [{}, {})",
            clip.start,
            clip.start + clip.length
        )));
    }
    let shift = clip.content_offset - clip.start;
    Ok(ContentRegion {
        start: start + shift,
        end: end + shift,
    })
}

fn other_track_notes(
    project: &Project,
    clip_id: &str,
    clip: &MidiClip,
    region: ContentRegion,
) -> Vec<ContextNote> {
    let shift = clip.content_offset - clip.start;
    let mut out = Vec::new();
    for track in &project.tracks {
        for other in &track.clips {
            let Clip::Midi(other) = other else { continue };
            if other.id == clip_id {
                continue;
            }
            for (timeline_start, note) in other.playback_notes() {
                let start = timeline_start + shift;
                if start < region.end - EPSILON && start + note.length > region.start + EPSILON {
                    out.push(ContextNote {
                        track_id: track.id.clone(),
                        pitch: note.pitch,
                        start,
                        length: note.length,
                    });
                }
            }
        }
    }
    out
}

fn check_request(request: &RegenerationRequest) -> Result<(), RegenError> {
    if let Some((low, high)) = request.constraints.pitch_range
        && (low > high || high > 127)
    {
        return Err(RegenError::InvalidRequest(format!(
            "pitch range {low}..={high} is not within 0..=127"
        )));
    }
    if request.constraints.max_notes == Some(0) {
        return Err(RegenError::InvalidRequest(
            "max_notes must be at least 1".to_owned(),
        ));
    }
    Ok(())
}

/// Validates `candidate` and turns it into notes with fresh IDs.
fn validate_candidate(
    candidate: &[CandidateNote],
    region: ContentRegion,
    constraints: &Constraints,
    taken: &mut HashSet<String>,
    ids: &mut dyn IdGenerator,
) -> Result<Vec<MidiNote>, RegenError> {
    let mut issues = Vec::new();
    if candidate.is_empty() {
        issues.push("the generator returned no notes".to_owned());
    }
    if let Some(max) = constraints.max_notes
        && candidate.len() > max
    {
        issues.push(format!(
            "{} notes exceed the limit of {max}",
            candidate.len()
        ));
    }
    let mut notes = Vec::with_capacity(candidate.len());
    for (index, c) in candidate.iter().enumerate() {
        let mut id = ids.new_id(ids::NOTE);
        while !taken.insert(id.clone()) {
            id = ids.new_id(ids::NOTE);
        }
        let note = match MidiNote::new(id, c.pitch, c.start, c.length, c.velocity, c.channel) {
            Ok(note) => note,
            Err(error) => {
                issues.push(format!("note {index}: {}", error.message));
                continue;
            }
        };
        if !region.owns(&note) || note.end() > region.end + EPSILON {
            issues.push(format!(
                "note {index} [{}, {}) is outside the region [{}, {})",
                note.start,
                note.end(),
                region.start,
                region.end
            ));
        }
        if let Some((low, high)) = constraints.pitch_range
            && !(low..=high).contains(&note.pitch)
        {
            issues.push(format!(
                "note {index}: pitch {} is outside {low}..={high}",
                note.pitch
            ));
        }
        notes.push(note);
    }
    if issues.is_empty() {
        Ok(notes)
    } else {
        Err(RegenError::Rejected(issues))
    }
}

/// Advisory findings: a new note that sounds over a preserved note of the same pitch and
/// channel.
fn overlap_warnings(added: &[MidiNote], preserved: &[MidiNote]) -> Vec<Warning> {
    let mut warnings = Vec::new();
    for new in added {
        for old in preserved {
            if new.pitch == old.pitch
                && new.channel == old.channel
                && new.start < old.end() - EPSILON
                && old.start < new.end() - EPSILON
            {
                warnings.push(Warning(format!(
                    "new note {} overlaps preserved note {} at pitch {}",
                    new.id, old.id, new.pitch
                )));
            }
        }
    }
    warnings
}

/// Builds a validated plan. The session is only read.
///
/// # Errors
///
/// [`RegenError::NotFound`], [`RegenError::InvalidRequest`] for a bad request,
/// [`RegenError::Generation`] when the generator fails, and [`RegenError::Rejected`] when the
/// candidate breaks a hard rule.
pub fn propose(
    session: &ProjectSession,
    request: &RegenerationRequest,
    generator: &dyn RegionGenerator,
    ids: &mut dyn IdGenerator,
) -> Result<RegenerationPlan, RegenError> {
    check_request(request)?;
    let project = session
        .require_project()
        .map_err(|_| RegenError::NotFound("no project is open".to_owned()))?;
    let (track_id, clip) = midi_clip(project, &request.clip_id)?;
    let region = resolve_region(project, clip, &request.target)?;

    let (removed, preserved): (Vec<MidiNote>, Vec<MidiNote>) =
        clip.notes.iter().cloned().partition(|n| region.owns(n));
    let crossing: Vec<String> = preserved
        .iter()
        .filter(|n| n.start < region.start - EPSILON && n.end() > region.start + EPSILON)
        .map(|n| n.id.clone())
        .collect();
    let other_tracks = other_track_notes(project, &request.clip_id, clip, region);

    let candidate = generator.generate(&GenerationInput {
        track_id,
        clip_id: &request.clip_id,
        region,
        preserved: &preserved,
        other_tracks: &other_tracks,
        constraints: &request.constraints,
        seed: request.seed,
    })?;

    let mut taken: HashSet<String> = clip.notes.iter().map(|n| n.id.clone()).collect();
    let added = validate_candidate(&candidate, region, &request.constraints, &mut taken, ids)?;
    let warnings = overlap_warnings(&added, &preserved);
    Ok(RegenerationPlan {
        clip_id: request.clip_id.clone(),
        track_id: track_id.to_owned(),
        region,
        base_version: session.version(),
        removed,
        preserved: preserved.len(),
        crossing,
        added,
        warnings,
    })
}

/// Commits `plan` as one undoable step.
///
/// # Errors
///
/// [`RegenError::Stale`] when the session changed since the plan was made, and
/// [`RegenError::Apply`] when the project command fails. The project is untouched in both
/// cases.
pub fn apply(
    session: &mut ProjectSession,
    plan: &RegenerationPlan,
    source: &str,
) -> Result<RegenerationReport, RegenError> {
    let current = session.version();
    if current != plan.base_version {
        return Err(RegenError::Stale {
            planned: plan.base_version,
            current,
        });
    }
    let command = session.execute(&plan.command(), false, source)?;
    Ok(RegenerationReport {
        command,
        removed: plan.removed.iter().map(|n| n.id.clone()).collect(),
        added: plan.added.iter().map(|n| n.id.clone()).collect(),
        preserved: plan.preserved,
        warnings: plan.warnings.clone(),
    })
}

/// [`propose`] and [`apply`] in one call.
///
/// # Errors
///
/// The errors of [`propose`] and [`apply`].
pub fn regenerate(
    session: &mut ProjectSession,
    request: &RegenerationRequest,
    generator: &dyn RegionGenerator,
    ids: &mut dyn IdGenerator,
    source: &str,
) -> Result<RegenerationReport, RegenError> {
    let plan = propose(session, request, generator, ids)?;
    apply(session, &plan, source)
}
