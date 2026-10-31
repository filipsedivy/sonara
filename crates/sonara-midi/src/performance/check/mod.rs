//! Rule predicates: they check the notes of an orchestra's parts against the orchestration rules
//! ([`RULES`](crate::orchestration::rules::RULES)) and report each place a rule finds as a
//! [`Hit`] under the rule's id.
//!
//! [`check`] runs the twenty rules whose inputs a project holds today ([`CHECKED_RULES`]): each
//! part's instrument, its notes with their velocities and articulations, and the tempo and metre.
//! [`check_rule`] runs one of them. The parts come as [`Line`]s; a note's dynamic is read from its
//! velocity ([`velocity_band`]), a muted note carries its mute as its articulation, and a
//! pizzicato note carries `pizzicato`. The rules that also need a part's role, the harmony or the
//! notated dynamic are applied by the voicing helpers while they write
//! ([`voicing`](super::voicing)), and the rest wait for inputs the project does not hold.
//!
//! Two derivations work out what a player has to set for a passage: [`harp_pedals`] the harp's
//! pedals and [`timpani_tunings`] the timpani's tuning, each with the places no setting covers.
//!
//! How a rule reads a source's words, how long a rest, how fast a run, is stated by the rule's
//! check text and by the named thresholds it compares with, each with its provenance
//! ([`orchestration::rules`](crate::orchestration::rules)). A source's span of moderate time
//! converts at the moderate tempo ([`Span::millis`]), and the time a piece leaves is measured at
//! the piece's own tempo.
//!
//! Everything is a pure function of its inputs: the same lines and score give the same hits in
//! the same order (TST-04). Nothing here runs on a real-time thread (RT-01).

mod doubling;
mod harp;
mod range;
mod spacing;
mod technique;
mod texture;
mod timpani;
mod wind;

use std::collections::BTreeSet;

use sonara_core::midi::MidiNote;
use thiserror::Error;

use super::{PartId, ScoreContext};
use crate::orchestration::rules::{DynamicBand, Level, RuleId};
use crate::orchestration::{Articulation, InstrumentType, SoundingPitch, Span};

pub use harp::{Accidental, HarpPedals, PedalPassage, PedalSetting, harp_pedals};
pub(crate) use spacing::{below_low_limit, is_low_dissonance};
pub use timpani::{TimpaniTunings, TuningPassage, timpani_tunings};

/// How close two beats must be to count as one.
const BEAT_EPSILON: f64 = 1e-9;

/// The rules [`check`] runs, in the catalogue's order: those whose inputs a project holds today.
pub const CHECKED_RULES: [RuleId; 20] = [
    RuleId::Rng01,
    RuleId::Rng02,
    RuleId::Rng03,
    RuleId::Spc01,
    RuleId::Spc02,
    RuleId::Dbl02,
    RuleId::Dbl03,
    RuleId::Dbl04,
    RuleId::Dbl09,
    RuleId::Voi12,
    RuleId::Ply01,
    RuleId::Ply02,
    RuleId::Ply04,
    RuleId::Ply05,
    RuleId::Ply06,
    RuleId::Ply12,
    RuleId::Ply14,
    RuleId::Ply15,
    RuleId::Tex01,
    RuleId::Tex02,
];

/// Why a check or a derivation cannot run.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum CheckError {
    /// The same part is given twice.
    #[error("part {0} is given twice")]
    DuplicatePart(PartId),
    /// A note starts before beat 0 or at a beat that is not finite, lasts no time or a length
    /// that is not finite, or has a pitch above 127 or a velocity of 0 or above 127.
    #[error("note {0} is not a valid note")]
    InvalidNote(String),
    /// The rule has no predicate here: it is not one of [`CHECKED_RULES`].
    #[error("rule {} is not checked from the notes", .0.code())]
    Unchecked(RuleId),
    /// A tuning was asked for no drums.
    #[error("the timpani need at least one drum")]
    NoDrums,
}

/// One part as the checks read it: the part, the instrument it plays and its notes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line<'a> {
    /// The part.
    pub part: &'a PartId,
    /// The instrument the part plays.
    pub instrument: InstrumentType,
    /// The notes at sounding pitch, in content beats of the clip that the score places, in any
    /// order. A note's velocity stands for its dynamic ([`velocity_band`]); its articulation is
    /// its own or, for a passage state, the mute it is played with.
    pub notes: &'a [MidiNote],
}

impl<'a> Line<'a> {
    /// The line of `part`, played by `instrument`, with `notes`.
    #[must_use]
    pub const fn new(part: &'a PartId, instrument: InstrumentType, notes: &'a [MidiNote]) -> Self {
        Self {
            part,
            instrument,
            notes,
        }
    }
}

/// One place where a rule finds something.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// The rule.
    pub rule: RuleId,
    /// How serious the finding is, by the rule's severity for this case.
    pub level: Level,
    /// The parts concerned, in the order the lines were given.
    pub parts: Vec<PartId>,
    /// The ids of the notes concerned, by start, then pitch, then id; empty for a finding about a
    /// whole piece.
    pub notes: Vec<String>,
    /// Where the finding starts, in content beats.
    pub start: f64,
    /// Where it ends, in content beats.
    pub end: f64,
    /// What was found, with its numbers, in words.
    pub detail: String,
}

/// Notes a derivation cannot cover, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Uncovered {
    /// Where the trouble starts, in content beats: the notes' start, or for a change the last note
    /// before it.
    pub start: f64,
    /// Where it ends: the notes' end, or for a change the first note that needs it.
    pub end: f64,
    /// The ids of the notes not covered, by start, then pitch, then id.
    pub notes: Vec<String>,
    /// Why they are not covered.
    pub shortfall: Shortfall,
}

impl Uncovered {
    /// The notes `notes` struck together, which `shortfall` leaves uncovered.
    fn of(notes: &[&MidiNote], shortfall: Shortfall) -> Self {
        let start = notes
            .iter()
            .map(|note| note.start)
            .reduce(f64::min)
            .unwrap_or(0.0);
        Self {
            start,
            end: latest_end(notes).unwrap_or(start),
            notes: notes.iter().map(|note| note.id.clone()).collect(),
            shortfall,
        }
    }
}

/// Why a derivation cannot cover some notes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shortfall {
    /// The notes struck together need more pitches than one setting or tuning gives.
    Pitches,
    /// A change before the notes takes longer than the music leaves.
    Time {
        /// The time the change takes, in milliseconds.
        needed_ms: u32,
        /// The time the music leaves at the piece's tempo, in milliseconds.
        free_ms: f64,
    },
}

/// The dynamic band a velocity stands for while a project stores no notated dynamics: 1-54 is pp
/// and p, 55-84 mp and mf, and 85-127 f and ff.
///
/// The bands are an estimate: six markings from pp to ff spread evenly over the velocities, pp
/// 1-39, p 40-54, mp 55-69, mf 70-84, f 85-104 and ff 105-127, grouped as the rules compare
/// loudness ([`DynamicBand`]). A velocity of 0 is no note and reads as the soft band.
#[must_use]
pub const fn velocity_band(velocity: u8) -> DynamicBand {
    match velocity {
        0..=54 => DynamicBand::Soft,
        55..=84 => DynamicBand::Medium,
        _ => DynamicBand::Loud,
    }
}

/// Runs every rule of [`CHECKED_RULES`] on `lines` within `score`.
///
/// The hits come by rule in the catalogue's order, then by where they start and end, then in the
/// order each predicate finds them. The same lines and score always give the same hits.
///
/// # Errors
/// [`CheckError::DuplicatePart`] when two lines name one part; [`CheckError::InvalidNote`] for
/// the first invalid note, line by line.
pub fn check(lines: &[Line<'_>], score: &ScoreContext) -> Result<Vec<Hit>, CheckError> {
    let context = Context::new(lines, score)?;
    let mut hits = Vec::new();
    for rule in CHECKED_RULES {
        hits.extend(run(rule, &context)?);
    }
    Ok(hits)
}

/// Runs the rule `rule` on `lines` within `score`; the hits come in the order of [`check`].
///
/// # Errors
/// [`CheckError::Unchecked`] for a rule that is not one of [`CHECKED_RULES`]; otherwise the
/// errors of [`check`].
pub fn check_rule(
    rule: RuleId,
    lines: &[Line<'_>],
    score: &ScoreContext,
) -> Result<Vec<Hit>, CheckError> {
    if !CHECKED_RULES.contains(&rule) {
        return Err(CheckError::Unchecked(rule));
    }
    run(rule, &Context::new(lines, score)?)
}

/// The hits of `rule`, sorted by where they start and end.
fn run(rule: RuleId, context: &Context<'_>) -> Result<Vec<Hit>, CheckError> {
    let mut hits = match rule {
        RuleId::Rng01 => range::outside_playable(context),
        RuleId::Rng02 => range::outside_comfortable(context),
        RuleId::Rng03 => range::sustained_high_brass(context),
        RuleId::Spc01 => spacing::low_interval_limits(context),
        RuleId::Spc02 => spacing::low_dissonances(context),
        RuleId::Dbl02 => doubling::inverted_woodwind_order(context),
        RuleId::Dbl03 => doubling::winds_in_octaves_without_strings(context),
        RuleId::Dbl04 => doubling::woodwind_above_trombone(context),
        RuleId::Dbl09 => doubling::strings_and_brass_in_unison(context),
        RuleId::Voi12 => technique::held_multi_stops(context),
        RuleId::Ply01 => wind::breath(context),
        RuleId::Ply02 => wind::brass_endurance(context),
        RuleId::Ply04 => technique::mute_changes(context),
        RuleId::Ply05 => technique::pizzicato_to_arco(context),
        RuleId::Ply06 => technique::pizzicato_speed(context),
        RuleId::Ply12 => spacing::timpani_thirds(context),
        RuleId::Ply14 => wind::fast_repetitions(context),
        RuleId::Ply15 => wind::clarinet_leaps(context),
        RuleId::Tex01 => texture::characteristic_colours(context),
        RuleId::Tex02 => texture::percussion_and_brass_rests(context),
        other => return Err(CheckError::Unchecked(other)),
    };
    hits.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.end.total_cmp(&b.end)));
    Ok(hits)
}

/// The parts of a check, validated, with their notes in order.
struct Context<'a> {
    score: &'a ScoreContext,
    parts: Vec<Part<'a>>,
}

/// One part of a check.
struct Part<'a> {
    /// The part's place among the lines given.
    place: usize,
    /// The part.
    id: &'a PartId,
    /// The instrument it plays.
    instrument: InstrumentType,
    /// Its notes by start, then pitch, then id.
    notes: Vec<&'a MidiNote>,
}

impl<'a> Context<'a> {
    fn new(lines: &'a [Line<'a>], score: &'a ScoreContext) -> Result<Self, CheckError> {
        let mut seen = BTreeSet::new();
        let mut parts = Vec::with_capacity(lines.len());
        for (place, line) in lines.iter().enumerate() {
            if !seen.insert(line.part) {
                return Err(CheckError::DuplicatePart(line.part.clone()));
            }
            if let Some(note) = line.notes.iter().find(|note| !is_valid(note)) {
                return Err(CheckError::InvalidNote(note.id.clone()));
            }
            let mut notes: Vec<&MidiNote> = line.notes.iter().collect();
            notes.sort_by(|a, b| note_order(a, b));
            parts.push(Part {
                place,
                id: line.part,
                instrument: line.instrument,
                notes,
            });
        }
        Ok(Self { score, parts })
    }

    /// The milliseconds from content beat `from` to content beat `to` at the piece's tempo;
    /// negative when `to` comes first.
    fn millis(&self, from: f64, to: f64) -> f64 {
        let tempo = self.score.tempo();
        let origin = self.score.origin();
        (tempo.beat_to_seconds(origin + to) - tempo.beat_to_seconds(origin + from)) * 1000.0
    }

    /// Whether the time from `from` to `to` is shorter than `span`.
    fn shorter_than(&self, from: f64, to: f64, span: Span) -> bool {
        self.millis(from, to) < f64::from(span.millis())
    }

    /// The parts that play an instrument for which `wanted` holds.
    fn parts_where(
        &self,
        wanted: impl Fn(InstrumentType) -> bool,
    ) -> impl Iterator<Item = &Part<'a>> {
        self.parts
            .iter()
            .filter(move |part| wanted(part.instrument))
    }
}

/// Whether `note` holds valid values: its fields are public, so a caller can build any.
pub(super) fn is_valid(note: &MidiNote) -> bool {
    note.start.is_finite()
        && note.start >= 0.0
        && note.length.is_finite()
        && note.length > 0.0
        && note.pitch <= 127
        && (1..=127).contains(&note.velocity)
}

/// The project's note order: by start, then pitch, then id.
pub(super) fn note_order(a: &MidiNote, b: &MidiNote) -> std::cmp::Ordering {
    a.start
        .total_cmp(&b.start)
        .then(a.pitch.cmp(&b.pitch))
        .then_with(|| a.id.cmp(&b.id))
}

/// The sounding pitch of `note`; a valid note's pitch is 0-127.
fn pitch(note: &MidiNote) -> i64 {
    i64::from(note.pitch)
}

/// The articulation `note` names, or `None` for the ordinary sound and for an id the vocabulary
/// does not know.
fn articulation(note: &MidiNote) -> Option<Articulation> {
    note.articulation.as_deref().and_then(Articulation::from_id)
}

/// Whether `note` is plucked: `pizzicato`, or an articulation that refines it.
fn is_pizzicato(note: &MidiNote) -> bool {
    articulation(note).is_some_and(|articulation| {
        articulation == Articulation::Pizzicato
            || articulation.parent() == Some(Articulation::Pizzicato)
    })
}

/// The notes grouped by start: each group the notes that start together, in note order.
fn onsets<'n>(notes: &[&'n MidiNote]) -> Vec<Vec<&'n MidiNote>> {
    let mut groups: Vec<Vec<&MidiNote>> = Vec::new();
    for &note in notes {
        match groups.last_mut() {
            Some(group)
                if group
                    .first()
                    .is_some_and(|first| (note.start - first.start).abs() < BEAT_EPSILON) =>
            {
                group.push(note);
            }
            _ => groups.push(vec![note]),
        }
    }
    groups
}

/// The latest end of `notes`, or `None` when there are none.
fn latest_end(notes: &[&MidiNote]) -> Option<f64> {
    notes.iter().map(|note| note.end()).reduce(f64::max)
}

/// "1 note" or "3 notes".
fn count(notes: &[&MidiNote]) -> String {
    match notes.len() {
        1 => "1 note".to_owned(),
        n => format!("{n} notes"),
    }
}

/// The name of a pitch with its MIDI number, such as `C4 (60)`.
fn named(pitch: i64) -> String {
    let name = u8::try_from(pitch).map_or_else(|_| pitch.to_string(), sonara_core::midi::note_name);
    format!("{name} ({pitch})")
}

/// The name of a sounding pitch with its MIDI number.
fn named_pitch(pitch: SoundingPitch) -> String {
    named(pitch.get())
}

/// Builds hits for one rule.
struct HitBuilder {
    rule: RuleId,
}

impl HitBuilder {
    const fn new(rule: RuleId) -> Self {
        Self { rule }
    }

    /// The hit at `level` on `parts` for `notes`, spanning them; `None` when `notes` is empty.
    fn notes(
        &self,
        level: Level,
        parts: &[&Part<'_>],
        notes: &[&MidiNote],
        detail: String,
    ) -> Option<Hit> {
        let start = notes.iter().map(|note| note.start).reduce(f64::min)?;
        let end = latest_end(notes)?;
        Some(self.span(level, parts, notes, start, end, detail))
    }

    /// The hit at `level` on `parts` for `notes`, from `start` to `end`.
    fn span(
        &self,
        level: Level,
        parts: &[&Part<'_>],
        notes: &[&MidiNote],
        start: f64,
        end: f64,
        detail: String,
    ) -> Hit {
        let mut places: Vec<(usize, &PartId)> =
            parts.iter().map(|part| (part.place, part.id)).collect();
        places.sort_by_key(|&(place, _)| place);
        places.dedup_by_key(|&mut (place, _)| place);
        let mut sorted: Vec<&MidiNote> = notes.to_vec();
        sorted.sort_by(|a, b| note_order(a, b));
        sorted.dedup_by(|a, b| std::ptr::eq(*a, *b));
        Hit {
            rule: self.rule,
            level,
            parts: places.into_iter().map(|(_, id)| id.clone()).collect(),
            notes: sorted.into_iter().map(|note| note.id.clone()).collect(),
            start,
            end,
            detail,
        }
    }

    /// The level the rule's severity gives in `band`, in the raised case when `in_case` holds.
    fn level(&self, band: DynamicBand, in_case: bool) -> Level {
        self.rule.get().severity().level(band, in_case)
    }

    /// The level of a rule whose severity is the same in every case.
    fn fixed(&self) -> Level {
        self.level(DynamicBand::Medium, false)
    }
}
