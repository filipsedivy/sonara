//! Orchestrating a reduction: a melody, the chords under it and a bass, spread over the parts of
//! an ensemble as lines ready to perform.
//!
//! **Reading a sketch.** [`analyse`] reads a sketch, such as a piano part, as a reduction: the
//! highest note of each onset is the melody (the skyline, [`skyline`]), and the notes under it
//! are read as chords against the chord table of [`theory`](crate::theory), the lowest note of
//! each giving the chord's bass ([`ChordFit`]). [`find_key`] matches the pitch classes of the
//! whole reduction against every major and minor key ([`KeyFit`]). Both readings are estimates:
//! where two chords or two keys fit equally they say so, so that a caller can ask, and the choice
//! made is the same every time.
//!
//! **Distributing it.** [`orchestrate`] gives every part of an ensemble a function, the melody,
//! the harmony or the bass, and its notes, family by family, through the voicing helpers of
//! [`voicing`](super::voicing), which apply the orchestration rules as they write:
//!
//! 1. **Functions.** A function the request assigns to named parts goes to exactly those parts
//!    ([`Assignment`]). The others take parts by the function table and the order in which
//!    Rimsky-Korsakov adds the groups of the orchestra in a crescendo, strings, woodwind, then
//!    brass (Principles of Orchestration, Ch. IV).
//! 2. **Melody.** [`melody_doublings`] picks the part that carries the melody from the function
//!    table and doubles it at the density asked, never with the family that voices the chords
//!    first. When the doubling still leaves no family to voice them, the melody is written one
//!    density thinner ([`Remark::Voicing`] with [`VoicingRemark::Thinned`]).
//! 3. **Harmony.** The families that are left voice the chords, the strings first, then the
//!    woodwinds, the horns and the other brass: one family at [`Density::Thin`], two at
//!    [`Density::Medium`] and every family at [`Density::Full`]. Each family writes the whole
//!    chord with its own helper ([`strings_divisi`], [`woodwind_pairs`], [`horns`],
//!    [`brass_chorale`]). Under a melody on top, the middle of its range at C4 or above, the
//!    chords stay below the melody so that it stays on top of the weight (BAL-03): a part that
//!    cannot play below the melody's lowest note rests, a family after the first whose chords
//!    would reach above the melody rests, and a chord of the first family that fits under it
//!    nowhere is reported ([`VoicingRemark::Crosses`]).
//! 4. **Bass.** One family carries the bass in its lowest voice, the one that holds the first
//!    instrument of the function table's bass row: so the cellos play the bass and the double
//!    basses double it an octave below, the usual construction of the bass. A bass the request
//!    assigns, a bass line the reduction gives, or a harmony the request assigns gets parts of
//!    its own instead: the first instrument of the bass row among the parts left (never a double
//!    bass or a double bassoon on its own, DBL-08) with the other bass-row instruments of its
//!    family, a lower one an octave below.
//! 5. **Timpani.** The timpani play two drums on the tonic and the dominant of the key
//!    ([`timpani`]) when the request assigns them any function, the melody included, which they
//!    cannot carry, and at [`Density::Full`] otherwise.
//!
//! Every part given comes back as a [`PartLine`], in the order given; a part with nothing to play
//! has no notes, and a remark says why ([`Remark::Rests`]). The lines are ready for
//! [`perform`](super::perform) once the application has filled in what each part's sound can do
//! ([`PartLine::caps`] comes back as [`SoundCaps::default`]).
//!
//! Everything is a pure function of its inputs: the same reduction, parts and options give the
//! same lines with the same note ids from the same id generator (TST-04). Nothing here runs on a
//! real-time thread (RT-01).

mod analyse;

use std::collections::BTreeSet;

use sonara_core::error::DomainError;
use sonara_core::ids::{IdGenerator, SequentialIds};
use sonara_core::midi::{MidiNote, sort_notes};
use thiserror::Error;

use super::voicing::{
    Carrier, ChordOptions, DEFAULT_VELOCITY, Density, RegisterFit, Remark as VoicingRemark,
    TimpaniOptions, Voiced, VoicingError, WindLayout, WindOptions, brass_chorale, comfortable,
    fit_register, horns, in_chorale, melody_carrier, melody_doublings, strings_divisi, timpani,
    upper_sections, woodwind_pairs,
};
use super::{DynamicsRequest, HumanizeRequest, PartId, PartLine, Role, ScoreContext, SoundCaps};
use crate::generators::ChordSlot;
use crate::orchestration::rules::{DynamicBand, RegisterBand, RuleId, WOODWIND_ORDER, carriers};
use crate::orchestration::{InstrumentType, PitchRange, SoundingPitch};
use crate::theory::{Chord, Key, bass_note};

/// Why a reduction cannot be read or orchestrated.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum ReductionError {
    /// The sketch has no notes.
    #[error("the sketch has no notes")]
    EmptySketch,
    /// The reduction has neither a melody nor chords.
    #[error("the reduction has neither a melody nor chords")]
    EmptyReduction,
    /// A note starts before beat 0 or at a beat that is not finite, lasts no time or a length that
    /// is not finite, or has a pitch above 127 or a velocity of 0 or above 127.
    #[error("note {0} is not a valid note")]
    InvalidNote(String),
    /// The same part is given twice.
    #[error("part {0} is given twice")]
    DuplicatePart(PartId),
    /// An assignment names a part the ensemble does not have.
    #[error("part {0} is not a part of the ensemble")]
    UnknownPart(PartId),
    /// Two assignments name the same part.
    #[error("part {0} is assigned twice")]
    DuplicateAssignment(PartId),
    /// An assignment gives a function a reduction does not hold: only the melody, the harmony
    /// and the bass.
    #[error("a reduction has no {} for a part to play", .0.slug())]
    UnsupportedRole(Role),
    /// A voicing helper cannot write the parts; the source says why. The melody's part cannot be
    /// found ([`VoicingError::NoCarrier`]) when no part plays it inside its comfortable range: a
    /// melody is never moved to fit.
    #[error("the parts cannot be voiced")]
    Voicing(#[from] VoicingError),
    /// A note could not be written; the source says why.
    #[error("a note cannot be written")]
    Note(#[from] DomainError),
}

/// A reduction: the melody, the chords under it and, when it has one, a bass line of its own.
#[derive(Clone, Debug, PartialEq)]
pub struct Reduction {
    /// The melody at sounding pitch, one line, in content beats of the clip.
    pub melody: Vec<MidiNote>,
    /// The chords in time order, each with its bass as the chord's slash bass when the bass is
    /// not its root.
    pub harmony: Vec<ChordSlot>,
    /// A bass line, when the reduction has its own; `None` takes the bass from the chords.
    pub bass: Option<Vec<MidiNote>>,
}

/// A sketch split into its melody and what lies under it.
#[derive(Clone, Debug, PartialEq)]
pub struct Skyline {
    /// The melody, in the project's note order.
    pub melody: Vec<MidiNote>,
    /// Every other note, in the project's note order.
    pub accompaniment: Vec<MidiNote>,
}

/// How well the chord of a [`ChordSlot`] fits the notes it was read from.
#[derive(Clone, Debug, PartialEq)]
pub struct ChordFit {
    /// Whether the chord's tones are exactly the pitch classes read; otherwise the chord is the
    /// nearest of the table, such as a triad read from its root and third alone.
    pub exact: bool,
    /// Other chords of the table that fit the notes exactly as well, in the order they were
    /// weighed; empty when the reading is unambiguous.
    pub others: Vec<Chord>,
}

/// The key of a reduction, and the keys that fit it equally.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyFit {
    /// The key: a major key or a natural minor key.
    pub key: Key,
    /// Other keys that fit exactly as well, from C upwards, the major before the minor; empty
    /// when the reading is unambiguous.
    pub others: Vec<Key>,
}

/// A sketch read as a reduction.
#[derive(Clone, Debug, PartialEq)]
pub struct Analysis {
    /// The melody and the chords; the sketch's own bass is the chords' bass.
    pub reduction: Reduction,
    /// How well each chord of [`Reduction::harmony`] fits, in the same order.
    pub chords: Vec<ChordFit>,
    /// The key.
    pub key: KeyFit,
}

impl Analysis {
    /// Whether a chord or the key could be read another way that fits equally well, which a
    /// caller may want to ask about.
    #[must_use]
    pub fn is_ambiguous(&self) -> bool {
        !self.key.others.is_empty() || self.chords.iter().any(|fit| !fit.others.is_empty())
    }
}

/// A function the request gives a part, such as the melody for the violins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    /// The part.
    pub part: PartId,
    /// Its function: [`Role::Melody`], [`Role::Harmony`] or [`Role::Bass`].
    pub role: Role,
}

/// What a request asks of an orchestration.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// How thickly to write: the melody's doubling and how many families voice the chords.
    pub density: Density,
    /// The key, for the timpani's drums; `None` reads it from the reduction ([`find_key`]).
    pub key: Option<Key>,
    /// The functions the request gives named parts; every other part takes the functions left.
    pub assignments: Vec<Assignment>,
    /// The dynamics of every line; a part that accompanies plays a marking below the melody.
    pub dynamics: DynamicsRequest,
    /// The humanization of every line.
    pub humanize: HumanizeRequest,
}

impl Default for Options {
    /// [`Density::Medium`], the key read from the reduction, no assignments, and the default
    /// dynamics and humanization.
    fn default() -> Self {
        Self {
            density: Density::Medium,
            key: None,
            assignments: Vec::new(),
            dynamics: DynamicsRequest::default(),
            humanize: HumanizeRequest::default(),
        }
    }
}

/// Why a part plays nothing.
#[derive(Clone, Debug, PartialEq)]
pub enum RestReason {
    /// A rule keeps it silent, such as a lone horn among trumpets (BAL-02) or double basses
    /// without cellos (DBL-08).
    Rule(RuleId),
    /// Its function needs no more parts: the melody's doubling or its family's chords leave it
    /// out, or the reduction has nothing for it to play.
    Unused,
    /// The density asked leaves its family out of the harmony.
    Density,
    /// No voicing helper writes for its instrument, such as a harp, a keyboard, a percussion
    /// instrument other than the timpani, a voice or a saxophone.
    NoHelper,
    /// Its family cannot voice the chords with the parts it has; the error says why.
    Unvoiced(VoicingError),
    /// It cannot play under the melody, which lies on top of the chords: its comfortable range
    /// starts at or above the melody's lowest note, or its family, voicing the chords after
    /// another one already does, would reach above the melody.
    AboveMelody,
}

/// Something the orchestration could not do, or a fact about what it wrote that a report should
/// state.
#[derive(Clone, Debug, PartialEq)]
pub enum Remark {
    /// A voicing helper's remark, such as a section divided or a doubling thinned. Silent parts
    /// are reported as [`Remark::Rests`] instead.
    Voicing(VoicingRemark),
    /// What the request asks breaks a rule and the orchestration writes it anyway: double basses
    /// or a double bassoon alone on a bass the request gives them, without the cellos or bassoons
    /// that should support them (DBL-08).
    Breaks(RuleId),
    /// The part plays nothing.
    Rests {
        /// The part.
        part: PartId,
        /// Why.
        reason: RestReason,
    },
}

/// A reduction spread over an ensemble.
#[derive(Clone, Debug, PartialEq)]
pub struct Orchestrated {
    /// The key the timpani were tuned in: the one asked for, or the reduction's.
    pub key: Key,
    /// Every part given, in the order given, with its function and notes; a part that plays
    /// nothing has no notes and the function the request gave it, or the harmony.
    pub lines: Vec<PartLine>,
    /// What the orchestration could not do, in the order it met it, then one [`Remark::Rests`]
    /// per silent part in the order of the parts.
    pub remarks: Vec<Remark>,
}

impl Orchestrated {
    /// The line of part `part`, or `None` when it was not given.
    #[must_use]
    pub fn line(&self, part: &str) -> Option<&PartLine> {
        self.lines.iter().find(|line| line.part.as_str() == part)
    }
}

/// Reads `sketch` as a reduction.
///
/// - **Melody.** The highest note of each onset is the melody; notes starting within a sixteenth
///   of a beat of each other share an onset. An onset whose highest note lies under a melody note
///   that still sounds a quarter of a beat or more has no melody note, and a melody note still
///   sounding that long when the next melody note starts is cut there, so the melody is one line
///   ([`skyline`]).
/// - **Chords.** The other notes are read wherever one of them starts or ends: the pitch classes
///   sounding there give the chord of the table that fits them best, the lowest note its bass. The
///   melody's sounding note joins them when they are fewer than three, or when only it makes a
///   chord exact, such as the fifth of a seventh chord. A reading that holds only tones of the
///   chord before over the same bass continues it ([`ChordFit`]).
/// - **Key.** [`find_key`] of the melody and the chords; the chords are then read once more in
///   that key, so that a tie goes to the chord whose tones lie in it.
///
/// The reduction has no bass line of its own: the chords' bass is the sketch's lowest notes.
///
/// # Errors
/// [`ReductionError::EmptySketch`] for no notes; [`ReductionError::InvalidNote`] for the first
/// note that is not valid.
pub fn analyse(sketch: &[MidiNote]) -> Result<Analysis, ReductionError> {
    if sketch.is_empty() {
        return Err(ReductionError::EmptySketch);
    }
    analyse::analyse(sketch)
}

/// Splits `notes`, in any order, into the melody on top and the accompaniment under it, as
/// [`analyse`] reads them; each note keeps its fields, except a melody note cut short where the
/// next melody note starts.
///
/// # Errors
/// [`ReductionError::InvalidNote`] for the first note that is not valid.
pub fn skyline(notes: &[MidiNote]) -> Result<Skyline, ReductionError> {
    analyse::skyline(notes)
}

/// The key of `reduction`, or `None` when it has no notes and no chords.
///
/// Each pitch class weighs the beats it sounds: the melody's and the bass line's notes, and each
/// chord's tones and bass for the chord's length. Every major and natural minor key is weighed by
/// the pitch classes it holds; among the keys that hold the most, the one whose tonic is the root
/// of the last chord, then of the first chord (the last and first melody notes when there are no
/// chords), is taken. Keys equal in both are listed as [`KeyFit::others`], and the first of
/// them, from C upwards with the major before the minor, is taken.
#[must_use]
pub fn find_key(reduction: &Reduction) -> Option<KeyFit> {
    analyse::find_key(reduction)
}

/// The groups of parts that voice a chord with one helper.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Family {
    /// Violins, violas, cellos and double basses ([`strings_divisi`]).
    Strings,
    /// The woodwinds of the natural order ([`woodwind_pairs`]).
    Woodwinds,
    /// Two to four horns ([`horns`]).
    Horns,
    /// The other brass, and horns that do not form a section of two to four
    /// ([`brass_chorale`]).
    Brass,
}

impl Family {
    /// The family of `instrument` when it voices chords, before the horns are counted.
    fn of(instrument: InstrumentType) -> Option<Self> {
        use InstrumentType::{Cello, DoubleBass, HornF, Viola, Violin};
        match instrument {
            Violin | Viola | Cello | DoubleBass => Some(Self::Strings),
            HornF => Some(Self::Horns),
            _ if WOODWIND_ORDER.rank(instrument).is_some() => Some(Self::Woodwinds),
            _ if in_chorale(instrument) => Some(Self::Brass),
            _ => None,
        }
    }
}

/// Semitones in an octave.
const OCTAVE: i64 = 12;

/// The families that voice chords, each with the places of its parts.
type Families = Vec<(Family, Vec<usize>)>;

/// The octave a bass line taken from the chords is written in before it is fitted to the parts
/// that play it: C2 to B2.
const BASS_OCTAVE: i64 = 2;

/// The fewest notes the strings write above the bass.
const UPPER_VOICES: usize = 3;

/// The fewest and most horns that form a section of their own.
const HORN_SECTION: (usize, usize) = (2, 4);

/// The parts of the ensemble by place, and what each was given.
struct Draft<'a> {
    parts: &'a [Carrier],
    notes: Vec<Vec<MidiNote>>,
    roles: Vec<Option<Role>>,
    rests: Vec<Option<RestReason>>,
    remarks: Vec<Remark>,
}

impl<'a> Draft<'a> {
    fn new(parts: &'a [Carrier]) -> Self {
        Self {
            parts,
            notes: vec![Vec::new(); parts.len()],
            roles: vec![None; parts.len()],
            rests: vec![None; parts.len()],
            remarks: Vec::new(),
        }
    }

    /// The carriers at `places`, in that order.
    fn carriers(&self, places: &[usize]) -> Vec<Carrier> {
        places
            .iter()
            .filter_map(|&place| self.parts.get(place).cloned())
            .collect()
    }

    /// Takes what a helper wrote for the parts at `places` (given to it in that order): each
    /// part's notes and `role`, [`Role::Bass`] for a part named in `bass`; for a part given
    /// nothing, a rest with the rule the helper named; and the helper's other remarks.
    fn take(&mut self, places: &[usize], voiced: Voiced, role: Role, bass: &BTreeSet<PartId>) {
        let mut silenced: Vec<(PartId, Option<RuleId>)> = Vec::new();
        for remark in voiced.remarks {
            match remark {
                VoicingRemark::Tacet { part, rule } => silenced.push((part, rule)),
                other => self.remarks.push(Remark::Voicing(other)),
            }
        }
        for (&place, part) in places.iter().zip(voiced.parts) {
            let line_role = if bass.contains(&part.part) {
                Role::Bass
            } else {
                role
            };
            if part.notes.is_empty() {
                let silencing = silenced
                    .iter()
                    .find(|(silent, _)| *silent == part.part)
                    .and_then(|(_, rule)| *rule);
                self.rest(
                    place,
                    silencing.map_or(RestReason::Unused, RestReason::Rule),
                );
            } else if let Some(notes) = self.notes.get_mut(place) {
                *notes = part.notes;
            }
            if let Some(slot) = self.roles.get_mut(place) {
                *slot = Some(line_role);
            }
        }
    }

    /// Records why the part at `place` rests, unless it already has a reason.
    fn rest(&mut self, place: usize, reason: RestReason) {
        if let Some(slot) = self.rests.get_mut(place)
            && slot.is_none()
        {
            *slot = Some(reason);
        }
    }

    /// Forgets what the melody's helper said of the part at `place`, which it left free.
    fn clear(&mut self, place: usize) {
        if let Some(slot) = self.rests.get_mut(place) {
            *slot = None;
        }
        if let Some(slot) = self.roles.get_mut(place) {
            *slot = None;
        }
    }

    /// Whether the part at `place` plays any note.
    fn plays(&self, place: usize) -> bool {
        self.notes.get(place).is_some_and(|notes| !notes.is_empty())
    }
}

/// Spreads `reduction` over the parts `parts`, given in score order, inside `score`, as the
/// request `options` asks (see the [module documentation](self) for the steps).
///
/// The melody's parts get copies of its notes, moved by octaves where they double it, with their
/// timing, velocity, channel and articulation; every other note is new. Every note takes a fresh
/// id from `ids`, part by part in the order given and each part's notes in the project's note
/// order, and only the notes returned take one, so the same request always gives the same ids
/// from the same generator. The key is `options.key`, or the reduction's ([`find_key`]).
///
/// # Errors
/// - [`ReductionError::EmptyReduction`] for a reduction with neither a melody nor chords;
/// - [`ReductionError::InvalidNote`] for a melody or bass note that is not valid;
/// - [`ReductionError::DuplicatePart`], [`ReductionError::UnknownPart`],
///   [`ReductionError::DuplicateAssignment`] and [`ReductionError::UnsupportedRole`] for parts
///   and assignments that do not fit together;
/// - [`ReductionError::Voicing`] when a chord slot starts before beat 0 or lasts no time
///   ([`VoicingError::InvalidSlot`]), or no part plays the melody inside its comfortable range
///   ([`VoicingError::NoCarrier`]);
/// - [`ReductionError::Note`] when a bass line doubled an octave below would leave the MIDI
///   notes.
pub fn orchestrate(
    reduction: &Reduction,
    parts: &[Carrier],
    score: &ScoreContext,
    options: &Options,
    ids: &mut dyn IdGenerator,
) -> Result<Orchestrated, ReductionError> {
    let request = Request::new(reduction, parts, score, options)?;
    let pool = request.melody_pool();
    let mut density = options.density;
    loop {
        // The steps write with ids of their own, so a draft that is thrown away, a thinner melody
        // tried or a family that would cross the melody, takes none from `ids`.
        let mut drafts = SequentialIds::new(0);
        let (mut draft, starved) = request.write(&pool, density, &mut drafts)?;
        // A melody doubled so widely that no family is left for the chords is thinned.
        if starved && let Some(thinner) = thinner(density) {
            density = thinner;
            continue;
        }
        if density != options.density {
            draft.remarks.insert(
                0,
                Remark::Voicing(VoicingRemark::Thinned {
                    asked: options.density,
                    written: density,
                }),
            );
        }
        return Ok(finish(draft, request.key, options, ids));
    }
}

/// An orchestration request, checked: the reduction, the parts and the functions the request
/// assigns them.
struct Request<'a> {
    reduction: &'a Reduction,
    parts: &'a [Carrier],
    score: &'a ScoreContext,
    options: &'a Options,
    /// The key the timpani are tuned in.
    key: Key,
    /// The function the request assigns each part, by place.
    assigned: Vec<Option<Role>>,
    /// The parts the request gives the melody, the harmony and the bass, the timpani left out.
    melody: Vec<usize>,
    harmony: Vec<usize>,
    bass: Vec<usize>,
    /// The parts the request assigns nothing.
    free: Vec<usize>,
    /// The melody the chords stay under, when it lies on top.
    under: Option<&'a [MidiNote]>,
}

impl<'a> Request<'a> {
    fn new(
        reduction: &'a Reduction,
        parts: &'a [Carrier],
        score: &'a ScoreContext,
        options: &'a Options,
    ) -> Result<Self, ReductionError> {
        let assigned = check(reduction, parts, &options.assignments)?;
        let key = match options.key {
            Some(key) => key,
            None => {
                find_key(reduction)
                    .ok_or(ReductionError::EmptyReduction)?
                    .key
            }
        };
        let explicit = |role: Role| -> Vec<usize> {
            (0..parts.len())
                .filter(|&place| {
                    assigned.get(place).copied().flatten() == Some(role)
                        && !is_timpani(parts, place)
                })
                .collect()
        };
        let (melody, harmony, bass) = (
            explicit(Role::Melody),
            explicit(Role::Harmony),
            explicit(Role::Bass),
        );
        let free = (0..parts.len())
            .filter(|&place| assigned.get(place).copied().flatten().is_none())
            .collect();
        Ok(Self {
            reduction,
            parts,
            score,
            options,
            key,
            assigned,
            melody,
            harmony,
            bass,
            free,
            under: on_top(&reduction.melody).then_some(reduction.melody.as_slice()),
        })
    }

    /// The parts the melody is given to: those the request names, or every free part but the
    /// timpani, without the family that voices the chords first, so that the melody's doublings
    /// never take it.
    fn melody_pool(&self) -> Vec<usize> {
        if !self.melody.is_empty() {
            return self.melody.clone();
        }
        let pool: Vec<usize> = self
            .free
            .iter()
            .copied()
            .filter(|&place| !is_timpani(self.parts, place))
            .collect();
        if self.reduction.harmony.is_empty() || !self.harmony.is_empty() {
            return pool;
        }
        let carriers: Vec<Carrier> = pool
            .iter()
            .filter_map(|&place| self.parts.get(place).cloned())
            .collect();
        // Without a carrier, the melody's helper reports that no part plays it.
        let Some(&carrier) =
            melody_carrier(&self.reduction.melody, &carriers).and_then(|i| pool.get(i))
        else {
            return pool;
        };
        let others: Vec<usize> = pool
            .iter()
            .copied()
            .filter(|&place| place != carrier)
            .collect();
        let (families, _) = families_of(self.parts, &others, self.under);
        let reserved = families
            .into_iter()
            .next()
            .map(|(_, members)| members)
            .unwrap_or_default();
        pool.into_iter()
            .filter(|place| !reserved.contains(place))
            .collect()
    }

    /// Writes every part at `density`, the melody on `pool`; returns the draft and whether the
    /// melody's doublings left no family to voice the chords, so that a thinner melody should be
    /// tried.
    fn write(
        &self,
        pool: &[usize],
        density: Density,
        ids: &mut dyn IdGenerator,
    ) -> Result<(Draft<'a>, bool), ReductionError> {
        let reduction = self.reduction;
        let mut draft = Draft::new(self.parts);
        draft.roles.clone_from(&self.assigned);
        // 1. The melody.
        if !reduction.melody.is_empty() && !pool.is_empty() {
            let written = melody_doublings(&reduction.melody, &draft.carriers(pool), density, ids)?;
            draft.take(pool, written.voiced, Role::Melody, &BTreeSet::new());
        }
        let melody_playing = pool.iter().filter(|&&place| draft.plays(place)).count();
        // The free parts the melody does not play serve the other functions.
        let left: Vec<usize> = self
            .free
            .iter()
            .copied()
            .filter(|&place| !draft.plays(place))
            .collect();
        for &place in &left {
            draft.clear(place);
        }
        // 2. A bass of its own, when the request or the reduction asks for one.
        let bass: Vec<usize> = if !self.bass.is_empty() {
            self.bass.clone()
        } else if reduction.bass.is_some() || !self.harmony.is_empty() {
            default_bass(self.parts, &left)
        } else {
            Vec::new()
        };
        if reduction.bass.is_some() || !reduction.harmony.is_empty() {
            write_bass(&mut draft, &bass, reduction, ids)?;
        }
        // 3. The harmony and the timpani.
        if reduction.harmony.is_empty() {
            return Ok((draft, false));
        }
        let voiced = self.write_chords(&mut draft, &left, &bass, density, ids)?;
        let starved =
            !voiced && self.harmony.is_empty() && self.melody.is_empty() && melody_playing > 1;
        Ok((draft, starved))
    }

    /// Writes the harmony on the parts the request gives it, or on the parts `left` free but
    /// those of `bass`, family by family as `density` allows, then the timpani; returns whether
    /// a family voiced the chords.
    fn write_chords(
        &self,
        draft: &mut Draft<'a>,
        left: &[usize],
        bass: &[usize],
        density: Density,
        ids: &mut dyn IdGenerator,
    ) -> Result<bool, ReductionError> {
        let chords = &self.reduction.harmony;
        let (harmony, limit) = if self.harmony.is_empty() {
            let harmony: Vec<usize> = left
                .iter()
                .copied()
                .filter(|&place| !bass.contains(&place) && !is_timpani(self.parts, place))
                .collect();
            (harmony, families(density))
        } else {
            (self.harmony.clone(), usize::MAX)
        };
        let marking = super::dynamics::part_marking(self.options.dynamics.marking, Role::Harmony);
        let rules = HarmonyRules {
            limit,
            carries_bass: bass.is_empty(),
            band: marking.band(),
            under: self.under,
        };
        let voiced = write_harmony(draft, &harmony, chords, &rules, ids)?;
        // The timpani the request names play; free timpani join a full texture.
        for place in (0..self.parts.len()).filter(|&place| is_timpani(self.parts, place)) {
            let named = self.assigned.get(place).copied().flatten().is_some();
            if !named && density != Density::Full {
                draft.rest(place, RestReason::Density);
                continue;
            }
            let carriers = draft.carriers(&[place]);
            let Some(carrier) = carriers.first() else {
                continue;
            };
            let options = TimpaniOptions::new(self.key);
            let written = timpani(chords, carrier, self.score, options, ids)?;
            let on_bass = BTreeSet::from([carrier.part.clone()]);
            draft.take(&[place], written.voiced, Role::Bass, &on_bass);
        }
        Ok(voiced)
    }
}

/// Whether the part at `place` plays the timpani.
fn is_timpani(parts: &[Carrier], place: usize) -> bool {
    parts
        .get(place)
        .is_some_and(|carrier| carrier.instrument == InstrumentType::Timpani)
}

/// Whether `melody` lies on top of the texture, so that the chords stay under it: the middle of its
/// range lies in the alto band (C4) or higher. A lower melody, a cello's or a bassoon's, has the
/// chords around and above it.
fn on_top(melody: &[MidiNote]) -> bool {
    let pitches = melody.iter().map(|note| i64::from(note.pitch));
    let (Some(lowest), Some(highest)) = (pitches.clone().min(), pitches.max()) else {
        return false;
    };
    SoundingPitch::new(i64::midpoint(lowest, highest))
        .is_some_and(|middle| RegisterBand::of(middle) >= RegisterBand::Alto)
}

/// How many families voice the chords at `density`: one, two, or all of them.
const fn families(density: Density) -> usize {
    match density {
        Density::Thin => 1,
        Density::Medium => 2,
        Density::Full => usize::MAX,
    }
}

/// The next thinner density, or `None` for [`Density::Thin`].
const fn thinner(density: Density) -> Option<Density> {
    match density {
        Density::Full => Some(Density::Medium),
        Density::Medium => Some(Density::Thin),
        Density::Thin => None,
    }
}

/// Checks the reduction, the parts and the assignments; returns each part's assigned function.
fn check(
    reduction: &Reduction,
    parts: &[Carrier],
    assignments: &[Assignment],
) -> Result<Vec<Option<Role>>, ReductionError> {
    if reduction.melody.is_empty() && reduction.harmony.is_empty() {
        return Err(ReductionError::EmptyReduction);
    }
    let notes = reduction
        .melody
        .iter()
        .chain(reduction.bass.iter().flatten());
    for note in notes {
        if !super::check::is_valid(note) {
            return Err(ReductionError::InvalidNote(note.id.clone()));
        }
    }
    for slot in &reduction.harmony {
        let valid = slot.start.is_finite()
            && slot.start >= 0.0
            && slot.length.is_finite()
            && slot.length > 0.0;
        if !valid {
            return Err(VoicingError::InvalidSlot {
                start: slot.start,
                length: slot.length,
            }
            .into());
        }
    }
    let mut seen = BTreeSet::new();
    for carrier in parts {
        if !seen.insert(&carrier.part) {
            return Err(ReductionError::DuplicatePart(carrier.part.clone()));
        }
    }
    let mut roles = vec![None; parts.len()];
    for assignment in assignments {
        if !matches!(assignment.role, Role::Melody | Role::Harmony | Role::Bass) {
            return Err(ReductionError::UnsupportedRole(assignment.role));
        }
        let place = parts
            .iter()
            .position(|carrier| carrier.part == assignment.part)
            .ok_or_else(|| ReductionError::UnknownPart(assignment.part.clone()))?;
        match roles.get_mut(place) {
            Some(slot @ None) => *slot = Some(assignment.role),
            _ => return Err(ReductionError::DuplicateAssignment(assignment.part.clone())),
        }
    }
    Ok(roles)
}

/// The parts among `left` that carry a bass of their own: the first instrument of the function
/// table's bass row among them, never a double bass or a double bassoon, which do not lead the
/// bass (DBL-08), with the other bass-row instruments of its family.
fn default_bass(parts: &[Carrier], left: &[usize]) -> Vec<usize> {
    let Some(row) = carriers(Role::Bass, RegisterBand::Bass) else {
        return Vec::new();
    };
    let instrument = |place: usize| parts.get(place).map(|carrier| carrier.instrument);
    let lead = row.instruments.iter().copied().find(|&listed| {
        !matches!(
            listed,
            InstrumentType::DoubleBass | InstrumentType::Contrabassoon
        ) && left.iter().any(|&place| instrument(place) == Some(listed))
    });
    let Some(lead) = lead else {
        return Vec::new();
    };
    left.iter()
        .copied()
        .filter(|&place| {
            instrument(place).is_some_and(|played| {
                row.instruments.contains(&played) && played.family() == lead.family()
            })
        })
        .collect()
}

/// Writes the bass line, the reduction's own or one from its chords' bass notes, for the parts at
/// `places`: the first in the order of the function table's bass row plays it inside its
/// comfortable range, and each other part doubles it, an octave below when that leaves no more
/// notes outside the part's comfortable range than the unison does (the bass is doubled downward,
/// Rimsky-Korsakov, Ch. III, p. 64). A line from the chords lies an octave higher when that lets
/// the lower instruments take the octave below inside their ranges.
fn write_bass(
    draft: &mut Draft<'_>,
    places: &[usize],
    reduction: &Reduction,
    ids: &mut dyn IdGenerator,
) -> Result<(), ReductionError> {
    let Some(row) = carriers(Role::Bass, RegisterBand::Bass) else {
        return Ok(());
    };
    let rank = |place: &usize| {
        draft
            .parts
            .get(*place)
            .and_then(|carrier| {
                row.instruments
                    .iter()
                    .position(|&i| i == carrier.instrument)
            })
            .unwrap_or(row.instruments.len())
    };
    let mut order = places.to_vec();
    order.sort_by_key(|place| (rank(place), *place));
    let Some(&lead) = order.first() else {
        return Ok(());
    };
    let line: Vec<MidiNote> = if let Some(line) = &reduction.bass {
        line.clone()
    } else {
        let mut line = Vec::with_capacity(reduction.harmony.len());
        for slot in &reduction.harmony {
            line.push(MidiNote::new(
                ids.new_id(sonara_core::ids::NOTE),
                bass_note(&slot.chord, BASS_OCTAVE),
                slot.start,
                slot.length,
                i64::from(DEFAULT_VELOCITY),
                0,
            )?);
        }
        line
    };
    let Some(lead_carrier) = draft.parts.get(lead) else {
        return Ok(());
    };
    let lead_instrument = lead_carrier.instrument;
    let mut fitted = fit(&line, lead_instrument);
    // A line taken from the chords goes up an octave when that keeps it in the lead's range and
    // lets the lower instruments double it an octave below in theirs, as the cellos over the
    // double basses.
    let lowest = |instrument: InstrumentType| comfortable(instrument).map(PitchRange::low);
    let lower: Vec<InstrumentType> = order
        .iter()
        .skip(1)
        .filter_map(|place| draft.parts.get(*place).map(|carrier| carrier.instrument))
        .filter(|&instrument| lowest(instrument) < lowest(lead_instrument))
        .collect();
    let misses = |shift: i64| -> usize {
        lower
            .iter()
            .map(|&instrument| outside(&fitted, instrument, shift))
            .sum()
    };
    if reduction.bass.is_none()
        && !lower.is_empty()
        && outside(&fitted, lead_instrument, OCTAVE) == 0
        && misses(0) < misses(-OCTAVE)
    {
        fitted = moved(&fitted, OCTAVE, ids)?;
    }
    let mut lines = vec![(lead, fitted.clone())];
    for &place in order.iter().skip(1) {
        let Some(carrier) = draft.parts.get(place) else {
            continue;
        };
        let outside = |shift: i64| outside(&fitted, carrier.instrument, shift);
        let shift = if outside(-OCTAVE) <= outside(0) {
            -OCTAVE
        } else {
            0
        };
        let doubled = moved(&fitted, shift, ids)?;
        lines.push((place, fit(&doubled, carrier.instrument)));
    }
    let only_basses = order.iter().all(|place| {
        draft.parts.get(*place).is_some_and(|carrier| {
            matches!(
                carrier.instrument,
                InstrumentType::DoubleBass | InstrumentType::Contrabassoon
            )
        })
    });
    if only_basses {
        draft.remarks.push(Remark::Breaks(RuleId::Dbl08));
    }
    for (place, mut notes) in lines {
        sort_notes(&mut notes);
        if let Some(slot) = draft.notes.get_mut(place) {
            *slot = notes;
        }
        if let Some(slot) = draft.roles.get_mut(place) {
            *slot = Some(Role::Bass);
        }
    }
    Ok(())
}

/// Copies of `notes` under new ids, `shift` semitones higher, every other field kept.
fn moved(
    notes: &[MidiNote],
    shift: i64,
    ids: &mut dyn IdGenerator,
) -> Result<Vec<MidiNote>, ReductionError> {
    let mut moved = Vec::with_capacity(notes.len());
    for note in notes {
        moved.push(
            MidiNote::new(
                ids.new_id(sonara_core::ids::NOTE),
                i64::from(note.pitch) + shift,
                note.start,
                note.length,
                i64::from(note.velocity),
                i64::from(note.channel),
            )?
            .with_articulation(note.articulation.clone())?,
        );
    }
    Ok(moved)
}

/// `notes`, a bass line, moved by octaves into the comfortable range of `instrument`.
fn fit(notes: &[MidiNote], instrument: InstrumentType) -> Vec<MidiNote> {
    match fit_register(notes, instrument.get(), Role::Bass) {
        RegisterFit::Moved { notes, .. } => notes,
        RegisterFit::Fits | RegisterFit::Ask { .. } => notes.to_vec(),
    }
}

/// How many of `notes`, moved by `shift` semitones, lie outside the comfortable range of
/// `instrument` (its playable range when none is known).
fn outside(notes: &[MidiNote], instrument: InstrumentType, shift: i64) -> usize {
    let Some(range) = comfortable(instrument) else {
        return notes.len();
    };
    notes
        .iter()
        .filter(|note| {
            let pitch = i64::from(note.pitch) + shift;
            pitch < range.low().get() || pitch > range.high().get()
        })
        .count()
}

/// How the harmony's families are chosen and written.
struct HarmonyRules<'a> {
    /// The most families that voice the chords.
    limit: usize,
    /// Whether one family carries the bass.
    carries_bass: bool,
    /// The dynamic band the harmony plays at.
    band: DynamicBand,
    /// The melody the chords stay under, when it lies on top.
    under: Option<&'a [MidiNote]>,
}

/// The parts at `places` grouped into the families that voice chords, in the order strings,
/// woodwinds, horns, brass, each with its parts in the order given; and the parts that take no
/// part in the harmony, with the reason: an instrument no helper writes for, and, under a melody
/// on top (`under`), one that cannot play below the melody's lowest note. Horns form a family of
/// their own when there are two to four of them, and join the brass otherwise.
fn families_of(
    parts: &[Carrier],
    places: &[usize],
    under: Option<&[MidiNote]>,
) -> (Families, Vec<(usize, RestReason)>) {
    let instrument = |place: usize| parts.get(place).map(|carrier| carrier.instrument);
    let lowest = under.and_then(|melody| melody.iter().map(|note| i64::from(note.pitch)).min());
    let above = |instrument: InstrumentType| {
        lowest.is_some_and(|lowest| {
            comfortable(instrument).is_some_and(|range| range.low().get() >= lowest)
        })
    };
    let mut families: Families = Vec::new();
    let mut outside = Vec::new();
    for &place in places {
        let Some(played) = instrument(place) else {
            continue;
        };
        let Some(kind) = Family::of(played) else {
            outside.push((place, RestReason::NoHelper));
            continue;
        };
        if above(played) {
            outside.push((place, RestReason::AboveMelody));
            continue;
        }
        match families.iter_mut().find(|(other, _)| *other == kind) {
            Some((_, members)) => members.push(place),
            None => families.push((kind, vec![place])),
        }
    }
    let horns = families
        .iter()
        .position(|(kind, members)| {
            *kind == Family::Horns && !(HORN_SECTION.0..=HORN_SECTION.1).contains(&members.len())
        })
        .map(|index| families.remove(index));
    if let Some((_, horns)) = horns {
        match families.iter_mut().find(|(kind, _)| *kind == Family::Brass) {
            Some((_, members)) => {
                members.extend(horns);
                members.sort_unstable();
            }
            None => families.push((Family::Brass, horns)),
        }
    }
    families.sort_by_key(|(kind, _)| *kind);
    (families, outside)
}

/// Voices `chords` with the parts at `places`, family by family in the order strings, woodwinds,
/// horns, brass, up to `rules.limit` families that voice them; returns whether any did.
fn write_harmony(
    draft: &mut Draft<'_>,
    places: &[usize],
    chords: &[ChordSlot],
    rules: &HarmonyRules<'_>,
    ids: &mut dyn IdGenerator,
) -> Result<bool, ReductionError> {
    let instrument = |place: usize| draft.parts.get(place).map(|carrier| carrier.instrument);
    let (families, outside) = families_of(draft.parts, places, rules.under);
    for (place, reason) in outside {
        draft.rest(place, reason);
    }
    // The family that carries the bass: the one with the first instrument of the bass row.
    let bass_family = rules
        .carries_bass
        .then(|| {
            let row = carriers(Role::Bass, RegisterBand::Bass)?;
            let holds = |members: &[usize], listed: InstrumentType| {
                members
                    .iter()
                    .any(|&place| instrument(place) == Some(listed))
            };
            row.instruments
                .iter()
                .find_map(|&listed| {
                    families
                        .iter()
                        .find(|(_, members)| holds(members, listed))
                        .map(|(kind, _)| *kind)
                })
                .or_else(|| families.first().map(|(kind, _)| *kind))
        })
        .flatten();
    let mut voiced = 0;
    for (kind, members) in families {
        if voiced >= rules.limit {
            for place in members {
                draft.rest(place, RestReason::Density);
            }
            continue;
        }
        let carries_bass = bass_family == Some(kind);
        let carriers = draft.carriers(&members);
        let options = ChordOptions {
            carries_bass,
            velocity: DEFAULT_VELOCITY,
            under: rules.under,
        };
        let written = match kind {
            Family::Strings => {
                // Three notes above the bass, so a minor triad over its root can be spaced
                // after the harmonic series; fewer sections divide to give them.
                let upper = upper_sections(&carriers, carries_bass);
                let upper_voices = upper.max(UPPER_VOICES).min(upper * 2);
                strings_divisi(
                    chords,
                    &carriers,
                    upper_voices + usize::from(carries_bass),
                    options,
                    ids,
                )
            }
            Family::Woodwinds => woodwind_pairs(
                chords,
                &carriers,
                WindOptions {
                    chord: options,
                    layout: WindLayout::Superposed,
                    dynamic: rules.band,
                },
                ids,
            ),
            Family::Horns => horns(chords, &carriers, None, options, ids),
            Family::Brass => brass_chorale(chords, &carriers, options, ids),
        };
        match written {
            // A family after the first that cannot stay under the melody rests: the chords
            // already sound under it, and it would cover the melody (BAL-03).
            Ok(written) if voiced > 0 && !carries_bass && crosses(&written) => {
                for place in members {
                    draft.rest(place, RestReason::AboveMelody);
                }
            }
            Ok(written) => {
                let bass = if carries_bass {
                    on_the_bass(&written)
                } else {
                    BTreeSet::new()
                };
                draft.take(&members, written, Role::Harmony, &bass);
                if members.iter().any(|&place| draft.plays(place)) {
                    voiced += 1;
                }
            }
            Err(error) if cannot_voice(&error) => {
                for place in members {
                    draft.rest(place, RestReason::Unvoiced(error.clone()));
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(voiced > 0)
}

/// Whether a family's chords reach above the melody somewhere ([`VoicingRemark::Crosses`]).
fn crosses(voiced: &Voiced) -> bool {
    voiced
        .remarks
        .iter()
        .any(|remark| matches!(remark, VoicingRemark::Crosses { .. }))
}

/// Whether `error` says only that a family's parts cannot voice the chords, not that the input
/// is wrong.
const fn cannot_voice(error: &VoicingError) -> bool {
    matches!(
        error,
        VoicingError::WrongInstrument { .. }
            | VoicingError::PartCount { .. }
            | VoicingError::TooFewVoices { .. }
            | VoicingError::TooManyVoices { .. }
            | VoicingError::NotTwoPairs
            | VoicingError::Breaks(_)
    )
}

/// The parts of a family that play its bass: those whose every note is the lowest the family
/// sounds at its start, and the parts they double from an octave below.
fn on_the_bass(voiced: &Voiced) -> BTreeSet<PartId> {
    let lowest = |beat: f64| {
        voiced
            .parts
            .iter()
            .flat_map(|part| &part.notes)
            .filter(|note| note.start <= beat && beat < note.end())
            .map(|note| note.pitch)
            .min()
    };
    let mut bass: BTreeSet<PartId> = voiced
        .parts
        .iter()
        .filter(|part| {
            !part.notes.is_empty()
                && part
                    .notes
                    .iter()
                    .all(|note| lowest(note.start) == Some(note.pitch))
        })
        .map(|part| part.part.clone())
        .collect();
    let doubled: Vec<PartId> = voiced
        .doublings()
        .filter(|doubling| doubling.octaves < 0 && bass.contains(doubling.part))
        .map(|doubling| doubling.of.clone())
        .collect();
    bass.extend(doubled);
    bass
}

/// The lines and remarks of a finished draft.
fn finish(
    draft: Draft<'_>,
    key: Key,
    options: &Options,
    ids: &mut dyn IdGenerator,
) -> Orchestrated {
    let Draft {
        parts,
        notes,
        roles,
        rests,
        mut remarks,
    } = draft;
    let mut lines = Vec::with_capacity(parts.len());
    for (((carrier, mut notes), role), rest) in parts.iter().zip(notes).zip(roles).zip(rests) {
        for note in &mut notes {
            note.id = ids.new_id(sonara_core::ids::NOTE);
        }
        sort_notes(&mut notes);
        if notes.is_empty() {
            let reason = rest.unwrap_or_else(|| {
                if Family::of(carrier.instrument).is_some()
                    || carrier.instrument == InstrumentType::Timpani
                {
                    RestReason::Unused
                } else {
                    RestReason::NoHelper
                }
            });
            remarks.push(Remark::Rests {
                part: carrier.part.clone(),
                reason,
            });
        }
        lines.push(PartLine {
            part: carrier.part.clone(),
            instrument: carrier.instrument.get(),
            role: role.unwrap_or(Role::Harmony),
            notes,
            caps: SoundCaps::default(),
            dynamics: options.dynamics,
            humanize: options.humanize,
        });
    }
    Orchestrated {
        key,
        lines,
        remarks,
    }
}
