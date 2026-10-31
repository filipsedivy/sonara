//! The performance layer: turns the notes of one orchestral part into a performance of them.
//!
//! [`perform`] takes one part's line ([`PartLine`]): its notes at sounding pitch, the instrument
//! that plays it, the part's role in the texture, what the chosen sound can do ([`SoundCaps`])
//! and what the request asks of its dynamics ([`DynamicsRequest`]). It returns the notes to write,
//! the articulation of each note and the passages of the sampler's `articulation` lane that play
//! them ([`Performed`], [`articulate()`]), and the velocity each note is written at with the points
//! of the sampler's `dynamics` lane that shape its phrases ([`dynamics`]).
//!
//! The lines come from [`voicing`], whose helpers give the parts of an ensemble their notes for a
//! melody or a run of chords, family by family, applying the orchestration rules as they write.
//!
//! A performance is a pure function of its inputs and a [`Seed`]: no clock, no environment, no
//! hash-ordered output and no state shared between calls, so the same line, score and seed always
//! give the same performance (TST-04). Each part draws its random numbers from a stream of its own,
//! keyed by the part's id rather than by its position among the parts ([`Seed::stream`]), so
//! adding, removing or reordering parts never changes another part's result.
//!
//! What a sound can do is known to the application's instrument catalog, which this crate cannot
//! see (ARC-11). The application copies those facts into a [`SoundCaps`] (ARC-08), and
//! [`SoundCaps::check`] says whether the copy is coherent.
//!
//! Nothing here runs on a real-time thread (RT-01).

pub mod articulate;
pub mod check;
pub mod dynamics;
mod seed;
pub mod voicing;

use std::fmt;
use std::ops::RangeInclusive;

use sonara_core::midi::MidiNote;
use sonara_core::model::automation::AutomationPoint;
use sonara_core::time::{TempoMap, TimeSignatureMap};
use thiserror::Error;

use crate::orchestration::{Articulation, Instrument, PitchRange, is_snake_case};

pub use crate::orchestration::rules::{Dynamic, Role};
pub use articulate::{
    Articulated, NoteArticulation, Passage, Rule, Source, articulate, articulation_choice,
};
pub use dynamics::{Dynamics, DynamicsRequest};
pub use seed::Seed;

/// The most bytes a [`PartId`] holds.
pub const MAX_PART_ID_BYTES: usize = 64;

/// Why a performance cannot be made, or one of its inputs cannot be built.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum PerformanceError {
    /// A part id is empty, longer than [`MAX_PART_ID_BYTES`], or holds a character other than
    /// `a-z`, `0-9` and `_`.
    #[error(
        "part id {0:?} must be 1-{MAX_PART_ID_BYTES} bytes of lower-case letters, digits and \
         underscores"
    )]
    InvalidPartId(String),
    /// The timeline beat of a clip's content beat 0 is not finite or lies before beat 0.
    #[error("the clip origin must be a finite beat >= 0, got {0}")]
    InvalidOrigin(f64),
    /// The timeline beat where a section starts is not finite or lies before beat 0.
    #[error("a section must start at a finite beat >= 0, got {0}")]
    InvalidSection(f64),
    /// The sound's capabilities contradict themselves; the source says how.
    #[error("the sound's capabilities are invalid")]
    InvalidCaps(#[from] CapsError),
    /// A note starts before beat 0 or at a beat that is not finite, lasts no time or a length
    /// that is not finite, or has a pitch above 127 or a velocity of 0 or above 127.
    #[error("note {0} is not a valid note")]
    InvalidNote(String),
    /// A note's articulation is no id of the articulation vocabulary.
    #[error("note {note} has the articulation {id:?}, which is not in the vocabulary")]
    UnknownArticulation {
        /// The id of the note.
        note: String,
        /// The articulation it names.
        id: String,
    },
}

/// The id of one part of a performance, such as `violins_1` or `horn_3`: the slug of an
/// ensemble's [`Part`](crate::orchestration::Part), or of a part named the same way.
///
/// It holds 1 to [`MAX_PART_ID_BYTES`] bytes of `a-z`, `0-9` and `_`, the form the ensemble
/// templates give their part slugs. The id, not the part's position, keys the part's random
/// stream ([`Seed::stream`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PartId(String);

impl PartId {
    /// The part id `slug`.
    ///
    /// # Errors
    /// [`PerformanceError::InvalidPartId`] when `slug` is empty, longer than
    /// [`MAX_PART_ID_BYTES`], or holds a character other than `a-z`, `0-9` and `_`.
    pub fn new(slug: &str) -> Result<Self, PerformanceError> {
        if slug.len() <= MAX_PART_ID_BYTES && is_snake_case(slug) {
            Ok(Self(slug.to_owned()))
        } else {
            Err(PerformanceError::InvalidPartId(slug.to_owned()))
        }
    }

    /// The id as text, such as `horn_3`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PartId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// How a sound joins two notes when the second starts while the first is held.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LegatoKind {
    /// No legato: each note sounds on its own, with its full attack.
    None,
    /// The new note crossfades out of the held one over `crossfade_ms`, starting past its own
    /// sample's attack: the sampler's overlap legato on sustain samples.
    Overlap {
        /// The length of the crossfade, in milliseconds (finite, above 0).
        crossfade_ms: f64,
    },
    /// The patch plays its own legato transitions, from regions triggered by a note that starts
    /// while another is held.
    Trigger,
}

/// How a sound's level follows the sampler's `dynamics` parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DynamicsKind {
    /// The note's velocity, scaled by `dynamics` at note-on, picks the layer and the level; a held
    /// note follows later changes as a gain.
    Velocity,
    /// Every note-on plays this velocity (1-127) in place of the note's own, scaled by `dynamics`
    /// as in [`DynamicsKind::Velocity`].
    Fixed(u8),
    /// `dynamics` is an absolute level that drives the patch's controller crossfade.
    Cc,
}

/// What the chosen sound of a part can do: the facts the instrument catalog knows about a preset,
/// copied by the application.
///
/// [`SoundCaps::default`] is a sound nothing is known about: no articulations to switch to, no
/// legato, one velocity layer, velocity dynamics, and no attack, hold or key facts.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundCaps {
    /// The articulations the preset can switch to; the ordinary sound needs no entry.
    pub articulations: Vec<Articulation>,
    /// How the preset plays legato.
    pub legato: LegatoKind,
    /// The highest velocity of each velocity layer, from the softest: for example `[62, 127]` for
    /// two layers that split between 62 and 63. The values rise strictly and the last is 127.
    pub layers: Vec<u8>,
    /// How the level follows `dynamics`.
    pub dynamics: DynamicsKind,
    /// The time from note-on to the audible onset, in milliseconds (finite, at least 0), for each
    /// articulation whose onset is known; `None` is the ordinary sound. An articulation appears
    /// at most once, and an unknown onset has no entry.
    pub attack_ms: Vec<(Option<Articulation>, f64)>,
    /// The longest sample that does not loop, in seconds (finite, above 0), when known: a held
    /// note longer than it falls silent before its note-off.
    pub max_hold_s: Option<f64>,
    /// The keys the preset has samples for, when known.
    pub sampled: Option<PitchRange>,
}

impl Default for SoundCaps {
    fn default() -> Self {
        Self {
            articulations: Vec::new(),
            legato: LegatoKind::None,
            layers: vec![127],
            dynamics: DynamicsKind::Velocity,
            attack_ms: Vec::new(),
            max_hold_s: None,
            sampled: None,
        }
    }
}

/// How a [`SoundCaps`] contradicts itself.
#[derive(Clone, Copy, Debug, PartialEq, Error)]
pub enum CapsError {
    /// The velocity layers are empty, include 0, do not rise strictly, or do not end at 127.
    #[error("velocity layers must rise strictly within 1-127 and end at 127")]
    Layers,
    /// The overlap legato's crossfade is not finite or not above 0 ms.
    #[error("the legato crossfade must be a finite time above 0 ms, got {0}")]
    Crossfade(f64),
    /// The fixed velocity is 0 or above 127.
    #[error("the fixed velocity must be 1-127, got {0}")]
    FixedVelocity(u8),
    /// An attack time is not finite or below 0 ms.
    #[error("an attack time must be a finite time of at least 0 ms, got {0}")]
    Attack(f64),
    /// One articulation, or the ordinary sound, has two attack times.
    #[error("an articulation has two attack times")]
    DuplicateAttack,
    /// The longest hold is not finite or not above 0 s.
    #[error("the longest hold must be a finite time above 0 s, got {0}")]
    MaxHold(f64),
}

impl SoundCaps {
    /// The velocities of the layer that plays `velocity`: from one above the top of the layer
    /// below it (1 for the softest) to the top of its own. `None` for a velocity of 0, which plays
    /// nothing, and for one above the loudest layer of caps that fail [`SoundCaps::check`].
    #[must_use]
    pub fn layer(&self, velocity: u8) -> Option<RangeInclusive<u8>> {
        if velocity == 0 {
            return None;
        }
        let mut lowest = 1;
        for &highest in &self.layers {
            if velocity <= highest {
                return Some(lowest..=highest);
            }
            lowest = highest.saturating_add(1);
        }
        None
    }

    /// Checks that the facts are coherent.
    ///
    /// # Errors
    /// The first failing check, in this order: [`CapsError::Layers`],
    /// [`CapsError::Crossfade`], [`CapsError::FixedVelocity`], then, entry by entry,
    /// [`CapsError::Attack`] and [`CapsError::DuplicateAttack`], and last
    /// [`CapsError::MaxHold`].
    pub fn check(&self) -> Result<(), CapsError> {
        let rising = self.layers.first().is_some_and(|&low| low >= 1)
            && self.layers.is_sorted_by(|lower, higher| lower < higher);
        if !rising || self.layers.last() != Some(&127) {
            return Err(CapsError::Layers);
        }
        if let LegatoKind::Overlap { crossfade_ms } = self.legato
            && !(crossfade_ms.is_finite() && crossfade_ms > 0.0)
        {
            return Err(CapsError::Crossfade(crossfade_ms));
        }
        if let DynamicsKind::Fixed(velocity) = self.dynamics
            && !(1..=127).contains(&velocity)
        {
            return Err(CapsError::FixedVelocity(velocity));
        }
        for (index, &(articulation, ms)) in self.attack_ms.iter().enumerate() {
            if !(ms.is_finite() && ms >= 0.0) {
                return Err(CapsError::Attack(ms));
            }
            if self
                .attack_ms
                .iter()
                .take(index)
                .any(|&(earlier, _)| earlier == articulation)
            {
                return Err(CapsError::DuplicateAttack);
            }
        }
        if let Some(seconds) = self.max_hold_s
            && !(seconds.is_finite() && seconds > 0.0)
        {
            return Err(CapsError::MaxHold(seconds));
        }
        Ok(())
    }
}

/// Where a line sits in the score: the project's tempo and metre, where its clip starts, and
/// where the piece's sections start.
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreContext {
    tempo: TempoMap,
    signatures: TimeSignatureMap,
    origin: f64,
    /// Timeline beats, ascending.
    sections: Vec<f64>,
}

impl ScoreContext {
    /// The context of a clip whose content beat 0 lies at timeline beat `origin` (the clip's
    /// start minus its content offset), in a project with `tempo` and `signatures` and no
    /// sections ([`ScoreContext::with_sections`]).
    ///
    /// # Errors
    /// [`PerformanceError::InvalidOrigin`] when `origin` is not finite or below 0.
    pub fn new(
        tempo: TempoMap,
        signatures: TimeSignatureMap,
        origin: f64,
    ) -> Result<Self, PerformanceError> {
        if origin.is_finite() && origin >= 0.0 {
            Ok(Self {
                tempo,
                signatures,
                origin,
                sections: Vec::new(),
            })
        } else {
            Err(PerformanceError::InvalidOrigin(origin))
        }
    }

    /// The project's tempo map.
    #[must_use]
    pub fn tempo(&self) -> &TempoMap {
        &self.tempo
    }

    /// The project's time signatures.
    #[must_use]
    pub fn signatures(&self) -> &TimeSignatureMap {
        &self.signatures
    }

    /// The timeline beat of the clip's content beat 0.
    #[must_use]
    pub fn origin(&self) -> f64 {
        self.origin
    }

    /// The context with sections of the piece starting at the timeline beats `starts`, in any
    /// order, in place of the sections it had: a phrase never runs across the start of a section.
    ///
    /// # Errors
    /// [`PerformanceError::InvalidSection`] for the first start that is not finite or lies
    /// before beat 0.
    pub fn with_sections(
        mut self,
        starts: impl IntoIterator<Item = f64>,
    ) -> Result<Self, PerformanceError> {
        let mut sections = Vec::new();
        for start in starts {
            if !(start.is_finite() && start >= 0.0) {
                return Err(PerformanceError::InvalidSection(start));
            }
            sections.push(start);
        }
        sections.sort_by(f64::total_cmp);
        self.sections = sections;
        Ok(self)
    }

    /// The timeline beats where the piece's sections start, ascending.
    #[must_use]
    pub fn sections(&self) -> &[f64] {
        &self.sections
    }
}

/// One part's line, ready to perform.
#[derive(Clone, Debug, PartialEq)]
pub struct PartLine {
    /// The part.
    pub part: PartId,
    /// The instrument that plays the part.
    pub instrument: &'static Instrument,
    /// What the part does in the texture.
    pub role: Role,
    /// The notes at sounding pitch, in content beats of the part's clip, in any order; chords are
    /// allowed.
    pub notes: Vec<MidiNote>,
    /// What the part's sound can do.
    pub caps: SoundCaps,
    /// What the request asks of the part's dynamics.
    pub dynamics: DynamicsRequest,
}

/// The performance of one part.
#[derive(Clone, Debug, PartialEq)]
pub struct Performed {
    /// The part performed.
    pub part: PartId,
    /// The notes to write, in the project's note order: by start, then pitch, then id, each
    /// written at the velocity its dynamics give it.
    pub notes: Vec<MidiNote>,
    /// The articulation of each note, in the order of [`Performed::notes`].
    pub articulations: Vec<NoteArticulation>,
    /// The passages of the part's sound, in time order, each with its switch on the sampler's
    /// `articulation` lane.
    pub passages: Vec<Passage>,
    /// The part's dynamics: its phrases, the level of each note and the points of the sampler's
    /// `dynamics` lane.
    pub dynamics: Dynamics,
}

impl Performed {
    /// The points of the sampler's `articulation` lane, one per passage, in time order: `hold`
    /// points of [`articulation_choice`] at timeline beats.
    #[must_use]
    pub fn articulation_lane(&self) -> Vec<AutomationPoint> {
        articulate::lane(&self.passages)
    }

    /// The points of the sampler's `dynamics` lane, at timeline beats in time order
    /// ([`Dynamics::lane`]).
    #[must_use]
    pub fn dynamics_lane(&self) -> Vec<AutomationPoint> {
        self.dynamics.lane.clone()
    }
}

/// Performs `line` within `score`. The same line, score and seed give the same result, whatever
/// other parts are performed (TST-04): a stage that makes a random choice draws it from the part's
/// own stream of `seed` ([`Seed::stream`]). Articulation and dynamics, the stages so far, make
/// none.
///
/// The notes come back in the project's note order (start, then pitch, then id; the order of
/// [`sort_notes`](sonara_core::midi::sort_notes)), each with its fields kept but its velocity, with
/// the articulation of each and the passages that play them ([`articulate()`]): only the strokes of
/// a tremolo or a roll that the sound plays are merged into one held note. Each note is then
/// written at the velocity its phrase's dynamics give it, and the dynamics hold the points of the
/// sampler's `dynamics` lane that shape the phrases ([`dynamics`]).
///
/// # Errors
/// - [`PerformanceError::InvalidCaps`] when `line.caps` fails [`SoundCaps::check`]; its source is
///   the [`CapsError`];
/// - [`PerformanceError::InvalidNote`] and [`PerformanceError::UnknownArticulation`] for a note
///   that [`articulate()`] rejects.
#[expect(
    unused_variables,
    reason = "the signature fixes what every stage of a performance reads, the part's own random \
              stream included; articulation and dynamics make no random choice"
)]
pub fn perform(
    line: &PartLine,
    score: &ScoreContext,
    seed: Seed,
) -> Result<Performed, PerformanceError> {
    let Articulated {
        notes,
        articulations,
        passages,
    } = articulate(line, score)?;
    let dynamics::Shaped { notes, dynamics } = dynamics::shape(line, score, notes, &articulations);
    Ok(Performed {
        part: line.part.clone(),
        notes,
        articulations,
        passages,
        dynamics,
    })
}
