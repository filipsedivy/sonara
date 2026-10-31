//! Articulation: which articulation each note of a line plays, the passages in which the part's
//! sound holds one articulation, and the automation points that switch it.
//!
//! [`articulate()`] gives every note of a line one articulation. A note whose `articulation` field
//! names one asks for that one; every other note asks for what its length, the time to the next
//! note and its neighbours imply ([`Rule`]). The part's sound then decides what each note plays
//! ([`NoteArticulation::plays`]): only an articulation the sound lists
//! ([`SoundCaps::articulations`]) is chosen, and one it lacks falls back through the vocabulary,
//! to the articulation it refines ([`Articulation::parent`]), then to the stand-ins of its
//! substitution row ([`fallback::substitution`]), and last to the ordinary sound.
//!
//! The sampler holds one articulation at a time for a whole track, and a switch affects only the
//! notes that start after it. So the notes are grouped into passages ([`Passage`]), and each
//! passage is one `hold` point of the sampler's `articulation` lane whose value is the choice
//! index of the passage's articulation ([`articulation_choice`]), placed ahead of the passage's
//! first note-on because the engine reads a device parameter once per block.
//!
//! The rules read times in seconds at the piece's tempo, so they hold at any tempo. Their
//! thresholds are starting values chosen to separate the long and the short patches of sample
//! libraries, to be confirmed by listening. Everything is a pure function of the line and the
//! score: no random choice is made, so the same input gives the same result (TST-04). Nothing here
//! runs on a real-time thread (RT-01).

use std::ops::Range;

use sonara_core::midi::MidiNote;
use sonara_core::model::automation::{AutomationPoint, CurveKind};
use sonara_core::model::project::SUPPORTED_SAMPLE_RATES;
use sonara_core::plugins::{SAMPLER_AUTO, SAMPLER_ORDINARIO};

use super::check::{is_valid, note_order};
use super::{PartLine, PerformanceError, ScoreContext, SoundCaps};
use crate::orchestration::fallback::{self, Emulation, Scope, StandIn, Substitution};
use crate::orchestration::{Articulation, Family, Instrument};

/// How close two beats must be to count as one.
pub(super) const BEAT_EPSILON: f64 = 1e-9;

/// The fewest repeated strokes of one pitch that make a tremolo or a roll.
const TREMOLO_STROKES: usize = 4;

/// The longest time between the onsets of two strokes of a tremolo or a roll, in seconds.
const TREMOLO_ONSET_S: f64 = 0.09;

/// A note shorter than this many seconds is very short.
const VERY_SHORT_S: f64 = 0.12;

/// A note shorter than this many seconds is short.
const SHORT_S: f64 = 0.25;

/// A note that fills less than this share of the time to the next onset is detached.
const DETACHED_DUTY: f64 = 0.5;

/// A detached note shorter than this many seconds is short; an accented note shorter than it is
/// marcato.
const DETACHED_S: f64 = 0.4;

/// Short string notes whose onsets lie at most this many seconds apart are spiccato.
const FAST_ONSET_S: f64 = 0.2;

/// A note that fills at least [`DETACHED_DUTY`] but less than this share of the time to the next
/// onset is separated from it: tenuto or portato.
const HELD_DUTY: f64 = 0.85;

/// A note this many velocity steps above the median of its phrase is accented.
pub(super) const ACCENT_VELOCITY: f64 = 20.0;

/// A rest of at least this many beats after a note ends its phrase.
pub(super) const PHRASE_REST_BEATS: f64 = 1.0;

/// A lone note's passage shorter than this many beats joins its neighbour.
const PASSAGE_MIN_BEATS: f64 = 1.0;

/// The frames of one block of the audio engine, which reads device parameters once per block at
/// the block's first frame (`docs/audio-engine.md`).
const BLOCK_FRAMES: u32 = 512;

/// How many blocks ahead of a passage's first note-on its switch is placed.
pub(super) const SWITCH_LEAD_BLOCKS: u32 = 2;

/// The choices of the sampler's `articulation` that come before the vocabulary's ids, in the
/// order `sonara_core::plugins::sampler_descriptor` lists them: `auto`, then `ordinario`.
const LEADING_CHOICES: [&str; 2] = [SAMPLER_AUTO, SAMPLER_ORDINARIO];

/// The choice index of `ordinario`, the last of the leading choices.
const ORDINARIO_CHOICE: usize = LEADING_CHOICES.len() - 1;

// Every choice index fits a `u16`, so `articulation_choice` cannot fail (PRI-04).
const _: () = assert!(
    LEADING_CHOICES.len() + Articulation::ALL.len() <= u16::MAX as usize,
    "the articulation choices fit a u16"
);

/// The choice index of `articulation` in the sampler's `articulation` parameter, the value its
/// automation lane holds: `ordinario` for `None`, the instrument's ordinary sound, and for an
/// articulation its place after `auto` and `ordinario` in the vocabulary's frozen order
/// ([`Articulation::ALL`]), which the parameter lists unchanged. It is never `auto`, which would
/// leave the patch's own default in charge.
///
/// # Panics
/// Never: every index fits a `u16`, which a compile-time check proves.
#[must_use]
pub fn articulation_choice(articulation: Option<Articulation>) -> u16 {
    let index = match articulation {
        None => ORDINARIO_CHOICE,
        // `Articulation::ALL` lists the variants in declaration order, which a compile-time check
        // of the vocabulary proves, so a variant's discriminant is its place in the list.
        Some(articulation) => LEADING_CHOICES.len() + articulation as usize,
    };
    u16::try_from(index).expect("every articulation choice fits a u16 (compile-time check)")
}

/// The rule by which the notes imply a note's articulation. The rules are tried in this order and
/// the first that matches decides, so every note gets exactly one.
///
/// Which articulation a rule implies depends on the instrument's family:
///
/// | Rule | Strings | Woodwinds, brass | Voices, other | Percussion | Keyboards, plucked |
/// |---|---|---|---|---|---|
/// | [`Rule::Strokes`] | `tremolo` | – | – | `roll`, else `tremolo` | – |
/// | [`Rule::Joined`] | `legato` | `legato` | `legato` | – | – |
/// | [`Rule::Accented`] | `marcato` | `marcato` | `marcato` | `marcato` | `marcato` |
/// | [`Rule::VeryShort`] | `spiccato` | `staccatissimo` | `staccato` | – | – |
/// | [`Rule::Short`] | `staccato`, `spiccato` when fast | `staccato` | `staccato` | – | – |
/// | [`Rule::Separated`] | `portato`, else `tenuto` | `tenuto` | `tenuto` | – | – |
/// | [`Rule::Ordinary`] | ordinario | ordinario | ordinario | ordinario | ordinario |
///
/// A struck or plucked note decays on its own and its note-off damps it, so its length plays its
/// shape and only rolls and accents switch the sound. A percussion instrument rolls with `roll`
/// when its record lists `roll` (drums, timpani, cymbals) and with `tremolo` when it lists only
/// that (mallet instruments).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rule {
    /// At least four strokes of one pitch, each the only note at its onset, whose onsets lie at
    /// most 0.09 s apart, played by a sound that has the instrument's tremolo or roll: the strokes
    /// are merged into one held note. Without that articulation the strokes stay as written,
    /// which is how a tremolo is imitated, and each is read by the other rules.
    Strokes,
    /// The note and the next note of the line join: the next starts before this one ends, at
    /// another pitch, and the two are the only notes sounding when it starts. Both notes are
    /// legato, the one that joins and the one joined into.
    Joined,
    /// The note is at least 20 velocity steps above the median of its phrase and shorter than
    /// 0.4 s, and the sound has `marcato`. Without `marcato` the velocity alone accents it.
    Accented,
    /// The note is shorter than 0.12 s.
    VeryShort,
    /// The note is shorter than 0.25 s, or it fills less than half the time to the next onset
    /// and is shorter than 0.4 s. A string note whose next onset follows within 0.2 s is
    /// spiccato.
    Short,
    /// The note fills at least half but less than 85 % of the time to the next onset.
    Separated,
    /// No other rule matches: the ordinary sound.
    Ordinary,
}

/// Why a note asks for its articulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// The note's own `articulation` field names it.
    Written,
    /// The notes imply it by this rule.
    Inferred(Rule),
}

/// The articulation of one note.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteArticulation {
    /// The id of the note.
    pub note: String,
    /// What the note asks for; `None` is the ordinary sound.
    pub asked: Option<Articulation>,
    /// Why it asks for it.
    pub source: Source,
    /// What the sound plays for it: the articulation asked for when the sound lists it, else the
    /// nearest the sound lists, else `None`, the ordinary sound.
    pub plays: Option<Articulation>,
    /// The rewrite of the notes that would bring what it plays nearer what it asks for: the
    /// rewrite the substitution row gives the stand-in, or the row's emulation when the ordinary
    /// sound plays. It is reported, not applied: the notes keep the lengths and velocities they
    /// were given, which for an implied articulation already have its shape.
    pub rewrite: Option<Emulation>,
    /// The ids of the repeated strokes merged into the note, its own first; empty when it is no
    /// merged tremolo or roll.
    pub merged: Vec<String>,
    /// The index of the note's passage in [`Articulated::passages`].
    pub passage: usize,
    /// Whether the passage plays another articulation than [`NoteArticulation::plays`]: a lone
    /// note that joined its neighbour's passage, or a note that starts together with a higher
    /// note that needs another articulation (a divisi on one track).
    pub overruled: bool,
}

/// A run of notes during which the part's sound holds one articulation.
#[derive(Clone, Debug, PartialEq)]
pub struct Passage {
    /// The articulation the passage plays; `None` is the ordinary sound.
    pub articulation: Option<Articulation>,
    /// The content beat of its first note-on.
    pub start: f64,
    /// The content beat of its last note-on.
    pub last_onset: f64,
    /// The point of the sampler's `articulation` lane that switches to the passage: a `hold` of
    /// [`articulation_choice`] at a timeline beat. The first passage's point sits at the clip's
    /// content beat 0, the score's origin. Every later point sits two engine blocks before the
    /// passage's first note-on, or halfway between the previous passage's last note-on and this
    /// one's first when those lie less than four blocks apart, so that it never switches a note
    /// of the previous passage. A block is taken at the slowest sample rate a project supports,
    /// where it lasts longest.
    pub switch: AutomationPoint,
    /// Whether the switch leads the passage's first note-on by less than one block, so that the
    /// block holding the note-on may have read the lane before the switch: the note may start
    /// with the previous passage's articulation.
    pub possibly_late: bool,
}

/// The articulation of a line.
#[derive(Clone, Debug, PartialEq)]
pub struct Articulated {
    /// The notes to write, in the project's note order (start, then pitch, then id): the line's
    /// notes with every field kept, except that the strokes of a tremolo or a roll the sound
    /// plays are merged into their first stroke, which lasts until the last stroke ends.
    pub notes: Vec<MidiNote>,
    /// The articulation of each note, in the order of [`Articulated::notes`].
    pub articulations: Vec<NoteArticulation>,
    /// The passages in time order; empty for a line without notes.
    pub passages: Vec<Passage>,
}

impl Articulated {
    /// The points of the sampler's `articulation` lane, one per passage, in time order.
    #[must_use]
    pub fn lane(&self) -> Vec<AutomationPoint> {
        lane(&self.passages)
    }
}

/// The points of an `articulation` lane: each passage's switch, in order.
pub(super) fn lane(passages: &[Passage]) -> Vec<AutomationPoint> {
    passages.iter().map(|passage| passage.switch).collect()
}

/// The articulation of `line` within `score`; see the module documentation.
///
/// # Errors
/// - [`PerformanceError::InvalidCaps`] when `line.caps` fails [`SoundCaps::check`];
/// - [`PerformanceError::InvalidNote`] for a note that starts before beat 0 or at a beat that is
///   not finite, lasts no time or a length that is not finite, or has a pitch above 127 or a
///   velocity of 0 or above 127 (its fields are public, so a caller can build any);
/// - [`PerformanceError::UnknownArticulation`] for a note whose `articulation` is no id of the
///   vocabulary, such as one a newer version wrote.
pub fn articulate(line: &PartLine, score: &ScoreContext) -> Result<Articulated, PerformanceError> {
    line.caps.check()?;
    let mut entries = Vec::with_capacity(line.notes.len());
    for note in &line.notes {
        if !is_valid(note) {
            return Err(PerformanceError::InvalidNote(note.id.clone()));
        }
        let written = match note.articulation.as_deref() {
            None => None,
            Some(id) => Some(Articulation::from_id(id).ok_or_else(|| {
                PerformanceError::UnknownArticulation {
                    note: note.id.clone(),
                    id: id.to_owned(),
                }
            })?),
        };
        entries.push(Entry {
            note: note.clone(),
            written,
            merged: Vec::new(),
        });
    }
    entries.sort_by(|a, b| note_order(&a.note, &b.note));
    let clock = Clock::new(score);
    let class = Class::of(line.instrument.family());
    let strokes = stroke_articulation(line.instrument, class)
        .filter(|articulation| line.caps.articulations.contains(articulation));
    if strokes.is_some() {
        entries = merge_strokes(entries, &clock);
    }
    let view = LineView::new(&entries, &clock);
    let mut articulations: Vec<NoteArticulation> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| view.articulation(index, entry, class, strokes, &line.caps))
        .collect();
    let drafts = view.passages(&articulations);
    for (passage_index, draft) in drafts.iter().enumerate() {
        for group in draft.groups.clone() {
            for index in view.groups.get(group).cloned().unwrap_or_default() {
                if let Some(articulation) = articulations.get_mut(index) {
                    articulation.passage = passage_index;
                    articulation.overruled = articulation.plays != draft.articulation;
                }
            }
        }
    }
    let passages = view.switches(&drafts);
    Ok(Articulated {
        notes: entries.into_iter().map(|entry| entry.note).collect(),
        articulations,
        passages,
    })
}

/// One note of the line as the rules read it.
struct Entry {
    note: MidiNote,
    /// The articulation the note's own field names.
    written: Option<Articulation>,
    /// The ids of the strokes merged into the note, its own first; empty when none was.
    merged: Vec<String>,
}

/// How an instrument's family shapes its notes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Class {
    /// Bowed strings.
    Bowed,
    /// Woodwinds and brass.
    Blown,
    /// Voices and any other sustaining instrument.
    Sustained,
    /// Percussion, keyboards and plucked strings: a note decays on its own.
    Struck,
}

impl Class {
    pub(super) const fn of(family: Family) -> Self {
        match family {
            Family::Strings => Self::Bowed,
            Family::Woodwinds | Family::Brass => Self::Blown,
            Family::Choir | Family::Other => Self::Sustained,
            Family::Percussion | Family::Keyboards | Family::Plucked => Self::Struck,
        }
    }

    /// Whether the note lengths of the family imply legato, short and separated notes.
    const fn shapes_notes(self) -> bool {
        !matches!(self, Self::Struck)
    }
}

/// The articulation of an instrument's tremolo or roll on one pitch, if it has one: `tremolo` for
/// bowed strings; for percussion `roll` when its record lists it, else `tremolo` when it lists
/// that.
fn stroke_articulation(instrument: &Instrument, class: Class) -> Option<Articulation> {
    match class {
        Class::Bowed => Some(Articulation::Tremolo),
        Class::Struck if instrument.family() == Family::Percussion => {
            let lists = |wanted: Articulation| instrument.articulations().contains(&wanted);
            if lists(Articulation::Roll) {
                Some(Articulation::Roll)
            } else if lists(Articulation::Tremolo) {
                Some(Articulation::Tremolo)
            } else {
                None
            }
        }
        Class::Blown | Class::Sustained | Class::Struck => None,
    }
}

/// Converts content beats of the line's clip to seconds at the piece's tempo.
pub(super) struct Clock<'a> {
    score: &'a ScoreContext,
}

impl<'a> Clock<'a> {
    /// The clock of `score`.
    pub(super) const fn new(score: &'a ScoreContext) -> Self {
        Self { score }
    }

    /// The time of content beat `beat`, in seconds from the timeline's start.
    pub(super) fn seconds(&self, beat: f64) -> f64 {
        self.score
            .tempo()
            .beat_to_seconds(self.score.origin() + beat)
    }

    /// The seconds from content beat `from` to content beat `to`.
    pub(super) fn span(&self, from: f64, to: f64) -> f64 {
        self.seconds(to) - self.seconds(from)
    }
}

/// The indices of notes grouped by onset, given their `starts` in note order: notes whose starts
/// lie within [`BEAT_EPSILON`] of a group's first start belong to it.
pub(super) fn onset_groups(starts: impl IntoIterator<Item = f64>) -> Vec<Range<usize>> {
    let mut groups: Vec<Range<usize>> = Vec::new();
    let mut first_start = f64::NEG_INFINITY;
    for (index, start) in starts.into_iter().enumerate() {
        match groups.last_mut() {
            Some(group) if start - first_start < BEAT_EPSILON => group.end = index + 1,
            _ => {
                first_start = start;
                groups.push(index..index + 1);
            }
        }
    }
    groups
}

/// The entries with every run of tremolo or roll strokes merged into its first stroke, which
/// then lasts until the last stroke ends ([`Rule::Strokes`]).
fn merge_strokes(entries: Vec<Entry>, clock: &Clock<'_>) -> Vec<Entry> {
    let groups = onset_groups(entries.iter().map(|entry| entry.note.start));
    // A stroke is the only note at its onset and names no articulation of its own.
    let strokes: Vec<Option<&Entry>> = groups
        .iter()
        .map(|group| match entries.get(group.clone()) {
            Some([entry]) if entry.written.is_none() => Some(entry),
            _ => None,
        })
        .collect();
    // `continues[g]`: the stroke of group `g + 1` repeats the stroke of group `g` within a run.
    let continues: Vec<bool> = strokes
        .windows(2)
        .map(|pair| match pair {
            [Some(first), Some(next)] => {
                first.note.pitch == next.note.pitch
                    && clock.span(first.note.start, next.note.start) <= TREMOLO_ONSET_S
                    && next.note.start < first.note.end() + PHRASE_REST_BEATS - BEAT_EPSILON
            }
            _ => false,
        })
        .collect();
    // The runs of at least `TREMOLO_STROKES` groups, as ranges of group indices.
    let mut runs: Vec<Range<usize>> = Vec::new();
    let mut start = 0;
    for group in 0..groups.len() {
        if !continues.get(group).copied().unwrap_or(false) {
            if group + 1 - start >= TREMOLO_STROKES {
                runs.push(start..group + 1);
            }
            start = group + 1;
        }
    }
    let mut merged = Vec::with_capacity(entries.len());
    let mut runs = runs.into_iter().peekable();
    let mut pending: Option<Entry> = None;
    let mut entries = entries.into_iter();
    for (group_index, group) in groups.iter().enumerate() {
        let in_run = runs.peek().is_some_and(|run| run.contains(&group_index));
        for _ in group.clone() {
            let Some(entry) = entries.next() else { break };
            if !in_run {
                merged.push(entry);
                continue;
            }
            if let Some(held) = pending.as_mut() {
                let end = held.note.end().max(entry.note.end());
                held.note.length = end - held.note.start;
                held.merged.push(entry.note.id);
            } else {
                let mut held = entry;
                held.merged.push(held.note.id.clone());
                pending = Some(held);
            }
        }
        if runs.peek().is_some_and(|run| run.end == group_index + 1) {
            runs.next();
            merged.extend(pending.take());
        }
    }
    merged
}

/// The line's entries with what the rules read about them.
struct LineView<'a> {
    entries: &'a [Entry],
    clock: &'a Clock<'a>,
    /// The onset groups ([`onset_groups`]).
    groups: Vec<Range<usize>>,
    /// The group of each entry.
    group_of: Vec<usize>,
    /// The median velocity of each entry's phrase.
    median: Vec<f64>,
    /// For each entry, whether it joins the next note of the line ([`Rule::Joined`]).
    joins: Vec<bool>,
    /// For each entry, whether the previous note of the line joins it.
    joined: Vec<bool>,
}

impl<'a> LineView<'a> {
    fn new(entries: &'a [Entry], clock: &'a Clock<'a>) -> Self {
        let groups = onset_groups(entries.iter().map(|entry| entry.note.start));
        let mut group_of = vec![0; entries.len()];
        for (group_index, group) in groups.iter().enumerate() {
            for slot in group_of.get_mut(group.clone()).into_iter().flatten() {
                *slot = group_index;
            }
        }
        let median = phrase_medians(entries, &groups);
        let mut line = Self {
            entries,
            clock,
            groups,
            group_of,
            median,
            joins: vec![false; entries.len()],
            joined: vec![false; entries.len()],
        };
        for index in 0..entries.len() {
            if let Some(next) = line.joined_note(index) {
                if let Some(slot) = line.joins.get_mut(index) {
                    *slot = true;
                }
                if let Some(slot) = line.joined.get_mut(next) {
                    *slot = true;
                }
            }
        }
        line
    }

    fn entry(&self, index: usize) -> Option<&'a Entry> {
        self.entries.get(index)
    }

    /// The index of the note that entry `index` joins legato, if it joins one: the only note of
    /// the next onset, starting before the entry ends, at another pitch, with no third note
    /// sounding when it starts.
    fn joined_note(&self, index: usize) -> Option<usize> {
        let entry = self.entry(index)?;
        let group = *self.group_of.get(index)?;
        let next_group = self.groups.get(group + 1)?;
        let [next] = self.entries.get(next_group.clone())? else {
            return None;
        };
        let next_index = next_group.start;
        let joins = next.note.start < entry.note.end() - BEAT_EPSILON
            && next.note.pitch != entry.note.pitch
            && self.sounding_at(next_index).all(|other| other == index);
        joins.then_some(next_index)
    }

    /// The indices of the entries other than entry `index` that sound when it starts: started
    /// before it, or with it, and not yet ended.
    fn sounding_at(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        let start = self.entry(index).map_or(0.0, |entry| entry.note.start);
        self.entries
            .iter()
            .enumerate()
            .take_while(move |(_, other)| other.note.start < start + BEAT_EPSILON)
            .filter(move |(other_index, other)| {
                *other_index != index && other.note.end() > start + BEAT_EPSILON
            })
            .map(|(other_index, _)| other_index)
    }

    /// The seconds from entry `index`'s onset to the next onset of the line, when that onset
    /// comes before the entry's end plus a rest of [`PHRASE_REST_BEATS`]; `None` when the entry
    /// ends its phrase.
    fn onset_gap(&self, index: usize) -> Option<f64> {
        let entry = self.entry(index)?;
        let group = *self.group_of.get(index)?;
        let next = self.groups.get(group + 1)?;
        let next_start = self.entry(next.start)?.note.start;
        (next_start < entry.note.end() + PHRASE_REST_BEATS - BEAT_EPSILON)
            .then(|| self.clock.span(entry.note.start, next_start))
    }

    /// What the passage around entry `index` is, for the scopes of the substitution rows.
    fn context(&self, index: usize, seconds: f64, accented: bool) -> NoteContext {
        NoteContext {
            seconds,
            together: self
                .group_of
                .get(index)
                .and_then(|&group| self.groups.get(group))
                .map_or(1, ExactSizeIterator::len),
            sounding: self.sounding_at(index).count(),
            accented,
            merged: self
                .entry(index)
                .is_some_and(|entry| entry.merged.len() > 1),
        }
    }

    /// The articulation of `entry`, the entry at `index`, before its passage is known.
    fn articulation(
        &self,
        index: usize,
        entry: &Entry,
        class: Class,
        stroke: Option<Articulation>,
        caps: &SoundCaps,
    ) -> NoteArticulation {
        let seconds = self.clock.span(entry.note.start, entry.note.end());
        let median = self.median.get(index).copied().unwrap_or(0.0);
        let accented = f64::from(entry.note.velocity) >= median + ACCENT_VELOCITY;
        let context = self.context(index, seconds, accented);
        let (source, candidates) = if let Some(written) = entry.written {
            (Source::Written, vec![written])
        } else {
            let (rule, candidates) =
                self.infer(index, entry, class, stroke, caps, seconds, accented);
            (Source::Inferred(rule), candidates.to_vec())
        };
        let (plays, rewrite) = resolve(&candidates, caps, &context);
        NoteArticulation {
            note: entry.note.id.clone(),
            asked: candidates.first().copied(),
            source,
            plays,
            rewrite,
            merged: if entry.merged.len() > 1 {
                entry.merged.clone()
            } else {
                Vec::new()
            },
            passage: 0,
            overruled: false,
        }
    }

    /// The rule that decides `entry`, the entry at `index`, and the articulations it implies,
    /// best first; none for the ordinary sound.
    #[expect(
        clippy::too_many_arguments,
        reason = "the rules read the note, its family, the sound and two measures of the note"
    )]
    fn infer(
        &self,
        index: usize,
        entry: &Entry,
        class: Class,
        stroke: Option<Articulation>,
        caps: &SoundCaps,
        seconds: f64,
        accented: bool,
    ) -> (Rule, &'static [Articulation]) {
        use Articulation::{Legato, Marcato, Portato, Spiccato, Staccatissimo, Staccato, Tenuto};
        if entry.merged.len() > 1 {
            let candidates: &'static [Articulation] = match stroke {
                Some(Articulation::Roll) => &[Articulation::Roll],
                Some(_) => &[Articulation::Tremolo],
                None => &[],
            };
            return (Rule::Strokes, candidates);
        }
        let shapes = class.shapes_notes();
        let joined = self.joins.get(index).copied().unwrap_or(false)
            || self.joined.get(index).copied().unwrap_or(false);
        if shapes && joined {
            return (Rule::Joined, &[Legato]);
        }
        if accented && seconds < DETACHED_S && caps.articulations.contains(&Marcato) {
            return (Rule::Accented, &[Marcato]);
        }
        if !shapes {
            return (Rule::Ordinary, &[]);
        }
        let gap = self.onset_gap(index);
        let duty = gap.map_or(1.0, |gap| seconds / gap);
        if seconds < VERY_SHORT_S {
            let candidates: &'static [Articulation] = match class {
                Class::Bowed => &[Spiccato],
                Class::Blown => &[Staccatissimo],
                Class::Sustained | Class::Struck => &[Staccato],
            };
            return (Rule::VeryShort, candidates);
        }
        if seconds < SHORT_S || (duty < DETACHED_DUTY && seconds < DETACHED_S) {
            let fast = gap.is_some_and(|gap| gap <= FAST_ONSET_S);
            let candidates: &'static [Articulation] = match class {
                Class::Bowed if fast => &[Spiccato],
                Class::Bowed | Class::Blown | Class::Sustained | Class::Struck => &[Staccato],
            };
            return (Rule::Short, candidates);
        }
        if (DETACHED_DUTY..HELD_DUTY).contains(&duty) {
            let candidates: &'static [Articulation] = match class {
                Class::Bowed => &[Portato, Tenuto],
                Class::Blown | Class::Sustained | Class::Struck => &[Tenuto],
            };
            return (Rule::Separated, candidates);
        }
        (Rule::Ordinary, &[])
    }

    /// The passages of the line, each a run of onset groups, before their switches are placed.
    fn passages(&self, notes: &[NoteArticulation]) -> Vec<Draft> {
        // Each onset group plays what its highest note plays: the line's top voice.
        let states: Vec<GroupState> = self
            .groups
            .iter()
            .map(|group| {
                let top = group.end.checked_sub(1).and_then(|last| notes.get(last));
                let members = notes.get(group.clone()).unwrap_or_default();
                GroupState {
                    articulation: top.and_then(|note| note.plays),
                    // A lone note may join a neighbour only when the notes implied its
                    // articulation and no stroke was merged into it.
                    loose: members.iter().all(|note| {
                        matches!(note.source, Source::Inferred(rule) if rule != Rule::Strokes)
                    }),
                }
            })
            .collect();
        let mut runs: Vec<Draft> = Vec::new();
        for (group, state) in states.iter().enumerate() {
            match runs.last_mut() {
                Some(run) if run.articulation == state.articulation => run.groups.end = group + 1,
                _ => runs.push(Draft {
                    articulation: state.articulation,
                    groups: group..group + 1,
                }),
            }
        }
        let lone: Vec<bool> = runs
            .iter()
            .enumerate()
            .map(|(run_index, run)| {
                let next_start = runs
                    .get(run_index + 1)
                    .and_then(|next| self.group_start(next.groups.start));
                self.is_lone(run, next_start, &states)
            })
            .collect();
        let mut passages: Vec<Draft> = Vec::with_capacity(runs.len());
        for (run, lone) in runs.into_iter().zip(lone) {
            match passages.last_mut() {
                Some(previous)
                    if previous.articulation == run.articulation
                        || (lone && is_shape(previous.articulation)) =>
                {
                    previous.groups.end = run.groups.end;
                }
                _ => passages.push(run),
            }
        }
        // A lone first passage joins the passage after it.
        let lone_first = match passages.as_slice() {
            [first, second, ..] => {
                let next_start = self.group_start(second.groups.start);
                self.is_lone(first, next_start, &states) && is_shape(second.articulation)
            }
            _ => false,
        };
        if lone_first {
            let first = passages.remove(0);
            if let Some(next) = passages.first_mut() {
                next.groups.start = first.groups.start;
            }
        }
        passages
    }

    /// The start of onset group `group`.
    fn group_start(&self, group: usize) -> Option<f64> {
        let first = self.groups.get(group)?.start;
        Some(self.entry(first)?.note.start)
    }

    /// Whether `run` is a lone note that may join a neighbouring passage: one onset group whose
    /// articulation the notes implied and is one of the note shapes ([`is_shape`]), lasting less
    /// than [`PASSAGE_MIN_BEATS`] until `next_start`, the next run's start, or, for the last run,
    /// until its last note ends.
    fn is_lone(&self, run: &Draft, next_start: Option<f64>, states: &[GroupState]) -> bool {
        if run.groups.len() != 1 || !is_shape(run.articulation) {
            return false;
        }
        let group = run.groups.start;
        let Some(state) = states.get(group) else {
            return false;
        };
        let Some(start) = self.group_start(group) else {
            return false;
        };
        let end = next_start.unwrap_or_else(|| {
            self.groups
                .get(group)
                .and_then(|members| self.entries.get(members.clone()))
                .unwrap_or_default()
                .iter()
                .map(|entry| entry.note.end())
                .fold(start, f64::max)
        });
        state.loose && end - start < PASSAGE_MIN_BEATS - BEAT_EPSILON
    }

    /// The passages with their switches placed.
    fn switches(&self, drafts: &[Draft]) -> Vec<Passage> {
        let block_s = block_seconds();
        let lead_s = f64::from(SWITCH_LEAD_BLOCKS) * block_s;
        let tempo = self.clock.score.tempo();
        let origin = self.clock.score.origin();
        let mut passages: Vec<Passage> = Vec::with_capacity(drafts.len());
        for draft in drafts {
            let start = self.group_start(draft.groups.start).unwrap_or(0.0);
            let last_onset = draft
                .groups
                .end
                .checked_sub(1)
                .and_then(|group| self.group_start(group))
                .unwrap_or(start);
            let (beat, possibly_late) = match passages.last() {
                None => (origin, false),
                Some(previous) => {
                    let first_s = self.clock.seconds(start);
                    let previous_s = self.clock.seconds(previous.last_onset);
                    let at_s = (first_s - lead_s).max(f64::midpoint(previous_s, first_s));
                    (
                        tempo.seconds_to_beat(at_s).max(origin),
                        first_s - at_s < block_s,
                    )
                }
            };
            passages.push(Passage {
                articulation: draft.articulation,
                start,
                last_onset,
                switch: AutomationPoint {
                    beat,
                    value: f64::from(articulation_choice(draft.articulation)),
                    curve: CurveKind::Hold,
                },
                possibly_late,
            });
        }
        passages
    }
}

/// A passage before its switch is placed.
struct Draft {
    articulation: Option<Articulation>,
    /// Its onset groups.
    groups: Range<usize>,
}

/// What an onset group plays, as the passages read it.
struct GroupState {
    articulation: Option<Articulation>,
    /// Whether every note of the group asks for an articulation the notes implied, with no
    /// stroke merged into it.
    loose: bool,
}

/// The slowest sample rate a project supports, at which a block lasts longest.
const SLOWEST_RATE: u32 = {
    let mut slowest = u32::MAX;
    let mut i = 0;
    while i < SUPPORTED_SAMPLE_RATES.len() {
        if SUPPORTED_SAMPLE_RATES[i] < slowest {
            slowest = SUPPORTED_SAMPLE_RATES[i];
        }
        i += 1;
    }
    slowest
};

/// The seconds of one engine block at [`SLOWEST_RATE`].
pub(super) fn block_seconds() -> f64 {
    f64::from(BLOCK_FRAMES) / f64::from(SLOWEST_RATE)
}

/// Whether `articulation` only shapes the length, the joins or the accent of notes, so that a
/// lone note can play it in place of its own: the ordinary sound, `sustain`, `legato`, `tenuto`,
/// `portato`, `staccato`, `staccatissimo`, `spiccato` or `marcato`. A roll, a tremolo, an effect
/// or any other articulation keeps its notes to itself.
fn is_shape(articulation: Option<Articulation>) -> bool {
    use Articulation::{
        Legato, Marcato, Portato, Spiccato, Staccatissimo, Staccato, Sustain, Tenuto,
    };
    matches!(
        articulation,
        None | Some(
            Sustain | Legato | Tenuto | Portato | Staccato | Staccatissimo | Spiccato | Marcato
        )
    )
}

/// The median velocity of each entry's phrase. A phrase ends where the next onset comes at least
/// [`PHRASE_REST_BEATS`] after every earlier note has ended.
fn phrase_medians(entries: &[Entry], groups: &[Range<usize>]) -> Vec<f64> {
    let mut medians = vec![0.0; entries.len()];
    let mut phrase: Range<usize> = 0..0;
    let mut latest_end = f64::NEG_INFINITY;
    let mut close = |phrase: &Range<usize>| {
        let mut velocities: Vec<u8> = entries
            .get(phrase.clone())
            .unwrap_or_default()
            .iter()
            .map(|entry| entry.note.velocity)
            .collect();
        velocities.sort_unstable();
        let middle = |at: usize| velocities.get(at).copied().map_or(0.0, f64::from);
        let count = velocities.len();
        let median = if count == 0 {
            0.0
        } else {
            f64::midpoint(middle((count - 1) / 2), middle(count / 2))
        };
        for slot in medians.get_mut(phrase.clone()).into_iter().flatten() {
            *slot = median;
        }
    };
    for group in groups {
        let Some(start) = entries.get(group.start).map(|entry| entry.note.start) else {
            continue;
        };
        if !phrase.is_empty() && start >= latest_end + PHRASE_REST_BEATS - BEAT_EPSILON {
            close(&phrase);
            phrase = group.start..group.start;
        }
        phrase.end = group.end;
        for entry in entries.get(group.clone()).unwrap_or_default() {
            latest_end = latest_end.max(entry.note.end());
        }
    }
    close(&phrase);
    medians
}

/// What the passage around a note is, for the scopes of the substitution rows.
#[derive(Clone, Copy, Debug)]
struct NoteContext {
    /// The note's length in seconds.
    seconds: f64,
    /// The notes that start together with it, itself included.
    together: usize,
    /// The other notes that sound when it starts.
    sounding: usize,
    /// Its velocity accents it ([`Rule::Accented`]).
    accented: bool,
    /// Strokes were merged into it.
    merged: bool,
}

impl NoteContext {
    /// Whether a stand-in that suits only `scope` suits the note: short notes are shorter than
    /// [`DETACHED_S`], a single line starts one note at a time with at most the note before still
    /// sounding, a chord starts several notes together, and a tremolo passage is a merged run of
    /// strokes. An arpeggio is not recognized, so a stand-in for arpeggios is never used.
    const fn fits(&self, scope: Scope) -> bool {
        match scope {
            Scope::ShortNotes => self.seconds < DETACHED_S,
            Scope::SingleLines => self.together == 1 && self.sounding <= 1,
            Scope::TremoloPassages => self.merged,
            Scope::Accents => self.accented,
            Scope::Chords => self.together > 1,
            Scope::Arpeggios => false,
            Scope::SingleStrokes => !self.merged,
        }
    }
}

/// What the sound plays for a note that asks for `candidates`, best first (empty: the ordinary
/// sound), with the rewrite reported for it.
///
/// The candidates the sound lists come first, in order. Then, candidate by candidate, the
/// articulation it refines, and the stand-ins of its substitution row (or of the row of the
/// articulation it refines) that are articulations, suit the note and are listed. Last comes the
/// ordinary sound, with the emulation of the first candidate's row.
fn resolve(
    candidates: &[Articulation],
    caps: &SoundCaps,
    context: &NoteContext,
) -> (Option<Articulation>, Option<Emulation>) {
    let lists = |articulation: Articulation| caps.articulations.contains(&articulation);
    if let Some(&listed) = candidates.iter().find(|&&candidate| lists(candidate)) {
        return (Some(listed), None);
    }
    for &candidate in candidates {
        if let Some(parent) = candidate.parent().filter(|&parent| lists(parent)) {
            return (Some(parent), None);
        }
        let substitutes = row_of(candidate).map_or(&[][..], |row| row.substitutes);
        for substitute in substitutes {
            if let StandIn::Articulation(stand_in) = substitute.stand_in
                && substitute.only.is_none_or(|scope| context.fits(scope))
                && lists(stand_in)
            {
                return (Some(stand_in), substitute.rewrite);
            }
        }
    }
    let emulation = candidates
        .first()
        .and_then(|&candidate| row_of(candidate))
        .and_then(|row| row.emulation);
    (None, emulation)
}

/// The substitution row of `articulation`, or of the articulation it refines.
fn row_of(articulation: Articulation) -> Option<&'static Substitution> {
    fallback::substitution(articulation)
        .or_else(|| articulation.parent().and_then(fallback::substitution))
}

#[cfg(test)]
mod tests {
    use super::{BLOCK_FRAMES, SLOWEST_RATE};
    use sonara_core::model::project::SUPPORTED_SAMPLE_RATES;

    #[test]
    fn the_switch_lead_counts_the_engines_blocks_at_the_slowest_rate() {
        assert_eq!(
            usize::try_from(BLOCK_FRAMES).expect("fits"),
            sonara_rt::BLOCK_FRAMES
        );
        assert_eq!(SLOWEST_RATE, 22_050);
        assert!(
            SUPPORTED_SAMPLE_RATES
                .iter()
                .all(|&rate| rate >= SLOWEST_RATE)
        );
        assert!(SUPPORTED_SAMPLE_RATES.contains(&SLOWEST_RATE));
    }
}
