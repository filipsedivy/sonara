//! Register, voicing and doubling helpers by family: they give each part of an ensemble its notes
//! for a melody or a run of chords, applying the orchestration rules while they write.
//!
//! Each helper is a pure function of a melody or chords ([`ChordSlot`]s), the parts that may play
//! them ([`Carrier`]s) and its options, and returns the notes of every part ([`Voiced`]) with the
//! doublings it wrote and what it could not do ([`Remark`]):
//!
//! - [`horns`]: the horns in close harmony, ordered from the top 1, 3, 2, 4 (VOI-07), each player
//!   moving as little as the chords allow (PLY-13), the bass doubled by a bassoon when one is
//!   given.
//! - [`brass_chorale`]: trumpets on top, trombones below and the bass on the lowest trombone with
//!   the tuba an octave under it; horns join only in unison pairs, so no single horn holds a part
//!   against trumpets or trombones (BAL-02).
//! - [`fit_register`]: moves a harmony line by octaves into the comfortable range (RNG-01,
//!   RNG-02); a melody is never moved silently, the caller asks instead.
//!
//! The chords of a family are laid out by one search shared by every helper: chord tones inside
//! each part's comfortable range, wide intervals low and close ones high, no interval below its
//! low limit and no void in the middle (SPC-01 to SPC-05), the smoothest motion between chords
//! and no consecutive octaves between upper parts (SPC-07). A chord no layout fits is left to
//! the other families and reported ([`Remark::Unvoiced`]).
//!
//! The helpers make no random choice: the same inputs give the same notes, ties broken by the
//! order the parts are given in (TST-04). New notes take their ids from the caller's
//! [`IdGenerator`]. Nothing here runs on a real-time thread (RT-01).

mod brass;
mod layout;
mod register;

use std::collections::BTreeSet;

use sonara_core::error::DomainError;
use sonara_core::ids::{self, IdGenerator};
use sonara_core::midi::{MidiNote, sort_notes};
use thiserror::Error;

use super::{PartId, PerformanceError};
use crate::generators::ChordSlot;
use crate::orchestration::rules::{NAMED_DOUBLINGS, NamedDoubling, Playing, RuleId, Spacing};
use crate::orchestration::{InstrumentType, Part, PitchModel, PitchRange};

pub use brass::{brass_chorale, horns};
pub use register::{RegisterFit, fit_register};

/// The velocity a helper writes unless told otherwise, the MIDI generators' default.
pub const DEFAULT_VELOCITY: u8 = 80;

/// Why a helper cannot voice its input.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum VoicingError {
    /// A chord slot starts before beat 0 or at a beat that is not finite, or its length is not a
    /// finite number above 0.
    #[error(
        "a chord slot must start at a finite beat >= 0 and last a finite length > 0, got start {start} and length {length}"
    )]
    InvalidSlot {
        /// The slot's start in beats.
        start: f64,
        /// The slot's length in beats.
        length: f64,
    },
    /// A velocity is 0 or above 127.
    #[error("the velocity must be 1-127, got {0}")]
    InvalidVelocity(u8),
    /// The same part is given twice.
    #[error("part {0} is given twice")]
    DuplicatePart(PartId),
    /// A part plays an instrument the helper does not voice, such as a flute given to the horns,
    /// or an instrument without pitch.
    #[error("part {part} plays {}, which this helper does not voice", .instrument.id())]
    WrongInstrument {
        /// The part.
        part: PartId,
        /// The instrument it plays.
        instrument: InstrumentType,
    },
    /// The helper needs between `min` and `max` parts of the kinds it voices.
    #[error("this helper needs {min} to {max} parts, got {given}")]
    PartCount {
        /// The fewest parts it voices.
        min: usize,
        /// The most parts it voices.
        max: usize,
        /// The parts given.
        given: usize,
    },
    /// The parts given form fewer voices than the helper needs for a chord.
    #[error("the parts given form {formed} voices, and a chord needs at least {needed}")]
    TooFewVoices {
        /// The fewest voices the helper writes a chord in.
        needed: usize,
        /// The voices the parts form.
        formed: usize,
    },
    /// A note could not be made from the input; the source says why.
    #[error("a note cannot be written")]
    Note(#[from] DomainError),
}

/// One part that may play a line: its id and the instrument it plays.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Carrier {
    /// The part.
    pub part: PartId,
    /// The instrument the part plays.
    pub instrument: InstrumentType,
}

impl Carrier {
    /// The part `part` playing `instrument`.
    #[must_use]
    pub const fn new(part: PartId, instrument: InstrumentType) -> Self {
        Self { part, instrument }
    }
}

impl TryFrom<&Part> for Carrier {
    type Error = PerformanceError;

    /// The part of an ensemble template, under its slug.
    ///
    /// # Errors
    /// [`PerformanceError::InvalidPartId`] when the slug is not a part id; never for a shipped
    /// template.
    fn try_from(part: &Part) -> Result<Self, Self::Error> {
        Ok(Self::new(PartId::new(part.slug)?, part.instrument))
    }
}

/// The notes one part plays.
#[derive(Clone, Debug, PartialEq)]
pub struct PartNotes {
    /// The part.
    pub part: PartId,
    /// Its notes in the project's note order: by start, then pitch, then id. Empty when the part
    /// rests.
    pub notes: Vec<MidiNote>,
}

/// A part that doubles another's line, and the doubling of the rules it realises.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Doubling<'a> {
    /// The doubling part.
    pub part: &'a PartId,
    /// The part whose line it doubles.
    pub of: &'a PartId,
    /// How many octaves above the line it plays: 0 in unison, −1 an octave below.
    pub octaves: i8,
    /// The doubling of [`NAMED_DOUBLINGS`] it realises, when the table names one.
    pub named: Option<&'static NamedDoubling>,
}

/// A doubling as a helper records it, by the places of the two parts among its carriers.
#[derive(Clone, Copy, Debug, PartialEq)]
struct DoublingAt {
    part: usize,
    of: usize,
    octaves: i8,
    named: Option<&'static NamedDoubling>,
}

/// Something a helper could not do, or a fact about what it wrote that a report should state.
#[derive(Clone, Debug, PartialEq)]
pub enum Remark {
    /// The part was given nothing to play; `rule` is the rule that kept it silent, if one did.
    Tacet {
        /// The part.
        part: PartId,
        /// The rule that kept it silent.
        rule: Option<RuleId>,
    },
    /// No layout of the chord starting at `start` meets the family's rules within its ranges, so
    /// the family rests through that chord.
    Unvoiced {
        /// The chord's start in beats.
        start: f64,
    },
}

impl Remark {
    /// The rule the remark concerns, if any.
    #[must_use]
    pub const fn rule(&self) -> Option<RuleId> {
        match self {
            Self::Tacet { rule, .. } => *rule,
            Self::Unvoiced { .. } => None,
        }
    }
}

/// What a helper wrote: the notes of every part it was given, in the order given, the doublings
/// among them and its remarks.
#[derive(Clone, Debug, PartialEq)]
pub struct Voiced {
    /// Every part the helper was given, in the order given, with its notes.
    pub parts: Vec<PartNotes>,
    doublings: Vec<DoublingAt>,
    /// What the helper could not do, in the order it met it.
    pub remarks: Vec<Remark>,
}

impl Voiced {
    /// The notes of part `part`, or `None` when the helper was not given it.
    #[must_use]
    pub fn notes(&self, part: &str) -> Option<&[MidiNote]> {
        self.parts
            .iter()
            .find(|notes| notes.part.as_str() == part)
            .map(|notes| notes.notes.as_slice())
    }

    /// The doublings among the parts: which part doubles which, and at what octave.
    pub fn doublings(&self) -> impl Iterator<Item = Doubling<'_>> {
        self.doublings.iter().filter_map(|at| {
            Some(Doubling {
                part: &self.parts.get(at.part)?.part,
                of: &self.parts.get(at.of)?.part,
                octaves: at.octaves,
                named: at.named,
            })
        })
    }
}

/// How a family helper writes its chords.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChordOptions {
    /// Whether the family plays the chord's bass note as its lowest voice; otherwise another
    /// family has the bass and this one may invert the chord.
    pub carries_bass: bool,
    /// The velocity of every note, 1-127.
    pub velocity: u8,
}

impl Default for ChordOptions {
    /// The family carries the bass, at [`DEFAULT_VELOCITY`].
    fn default() -> Self {
        Self {
            carries_bass: true,
            velocity: DEFAULT_VELOCITY,
        }
    }
}

/// The range of ordinary writing for `instrument`: its comfortable range, or its playable range
/// when no comfortable range is known; `None` for an instrument without pitch.
pub(super) fn comfortable(instrument: InstrumentType) -> Option<PitchRange> {
    match instrument.get().pitch() {
        PitchModel::Pitched(pitched) => Some(
            pitched
                .comfortable
                .map_or(pitched.playable.range, |range| range.range),
        ),
        PitchModel::Unpitched => None,
    }
}

/// The pitches both ranges hold, or `None` when they do not meet.
fn intersect(a: PitchRange, b: PitchRange) -> Option<PitchRange> {
    let low = a.low().max(b.low()).get();
    let high = a.high().min(b.high()).get();
    PitchRange::try_new(low, high).ok()
}

/// `range` moved by `semitones`, or `None` when it would leave MIDI notes 0-127.
fn shifted(range: PitchRange, semitones: i64) -> Option<PitchRange> {
    PitchRange::try_new(
        range.low().get() + semitones,
        range.high().get() + semitones,
    )
    .ok()
}

/// The named doubling of an `upper` and a `lower` instrument, `octaves` apart (0 for a unison, in
/// either order), when the table names it as a pair; for a doubling of the bass, a row that
/// builds the bass comes first.
fn named_pair(
    upper: InstrumentType,
    lower: InstrumentType,
    octaves: u8,
    bass: bool,
) -> Option<&'static NamedDoubling> {
    let spacing = if octaves == 0 {
        Spacing::Unison
    } else {
        Spacing::Octaves(octaves)
    };
    let pair = |row: &NamedDoubling, a: InstrumentType, b: InstrumentType| {
        matches!(row.members, [first, second]
            if first.selector.matches(Playing::ordinary(a))
                && second.selector.matches(Playing::ordinary(b)))
    };
    let names = |row: &&NamedDoubling| {
        row.spacings.contains(&spacing)
            && (pair(row, upper, lower) || (octaves == 0 && pair(row, lower, upper)))
    };
    let mut rows = NAMED_DOUBLINGS.iter().filter(names);
    let first = rows.clone().next();
    if bass {
        rows.find(|row| row.builds_bass).or(first)
    } else {
        first
    }
}

/// Checks that every part is given once and the velocity is 1-127.
fn check_inputs<'a>(
    carriers: impl IntoIterator<Item = &'a Carrier>,
    velocity: u8,
) -> Result<(), VoicingError> {
    if !(1..=127).contains(&velocity) {
        return Err(VoicingError::InvalidVelocity(velocity));
    }
    let mut seen = BTreeSet::new();
    for carrier in carriers {
        if !seen.insert(&carrier.part) {
            return Err(VoicingError::DuplicatePart(carrier.part.clone()));
        }
    }
    Ok(())
}

/// The slots in the order they sound (by start, ties in the order given), after checking each.
fn sorted_slots(slots: &[ChordSlot]) -> Result<Vec<&ChordSlot>, VoicingError> {
    for slot in slots {
        let valid = slot.start.is_finite()
            && slot.start >= 0.0
            && slot.length.is_finite()
            && slot.length > 0.0;
        if !valid {
            return Err(VoicingError::InvalidSlot {
                start: slot.start,
                length: slot.length,
            });
        }
    }
    let mut sorted: Vec<&ChordSlot> = slots.iter().collect();
    sorted.sort_by(|a, b| a.start.total_cmp(&b.start));
    Ok(sorted)
}

/// A new note of `pitch` from `start` for `length` beats at `velocity` on channel 0.
fn new_note(
    ids: &mut dyn IdGenerator,
    pitch: i64,
    start: f64,
    length: f64,
    velocity: u8,
) -> Result<MidiNote, VoicingError> {
    Ok(MidiNote::new(
        ids.new_id(ids::NOTE),
        pitch,
        start,
        length,
        i64::from(velocity),
        0,
    )?)
}

/// The notes a helper writes, part by part, before it hands them back as [`Voiced`].
struct Score<'a> {
    carriers: &'a [Carrier],
    notes: Vec<Vec<MidiNote>>,
    doublings: Vec<DoublingAt>,
    remarks: Vec<Remark>,
    /// The rule that keeps a part silent when it is given nothing, by the part's place.
    silenced: Vec<Option<RuleId>>,
}

impl<'a> Score<'a> {
    fn new(carriers: &'a [Carrier]) -> Self {
        Self {
            carriers,
            notes: vec![Vec::new(); carriers.len()],
            doublings: Vec::new(),
            remarks: Vec::new(),
            silenced: vec![None; carriers.len()],
        }
    }

    /// Adds a note to the part at `place`.
    fn push(&mut self, place: usize, note: MidiNote) {
        if let Some(notes) = self.notes.get_mut(place) {
            notes.push(note);
        }
    }

    /// Records that the part at `part` doubles the part at `of`, `octaves` above it; `bass` says
    /// whether the line doubled is the bass.
    fn double(&mut self, part: usize, of: usize, octaves: i8, bass: bool) {
        let (Some(upper), Some(lower)) = (self.carriers.get(part), self.carriers.get(of)) else {
            return;
        };
        let (high, low) = if octaves >= 0 {
            (upper.instrument, lower.instrument)
        } else {
            (lower.instrument, upper.instrument)
        };
        let named = named_pair(high, low, octaves.unsigned_abs(), bass);
        if !self
            .doublings
            .iter()
            .any(|at| at.part == part && at.of == of && at.octaves == octaves)
        {
            self.doublings.push(DoublingAt {
                part,
                of,
                octaves,
                named,
            });
        }
    }

    /// Names the rule that keeps the part at `place` silent if it ends up with no notes.
    fn silence(&mut self, place: usize, rule: RuleId) {
        if let Some(slot) = self.silenced.get_mut(place) {
            *slot = Some(rule);
        }
    }

    /// The parts with their notes in the project's order, a tacet remark for each silent part.
    fn finish(self) -> Voiced {
        let mut remarks = self.remarks;
        let mut parts = Vec::with_capacity(self.carriers.len());
        for ((carrier, mut notes), rule) in self.carriers.iter().zip(self.notes).zip(self.silenced)
        {
            if notes.is_empty() {
                remarks.push(Remark::Tacet {
                    part: carrier.part.clone(),
                    rule,
                });
            }
            sort_notes(&mut notes);
            parts.push(PartNotes {
                part: carrier.part.clone(),
                notes,
            });
        }
        Voiced {
            parts,
            doublings: self.doublings,
            remarks,
        }
    }
}

/// One voice of a family's chords: who plays it and where.
#[derive(Clone, Debug, PartialEq)]
struct VoiceGroup {
    /// The pitches the voice may sound: every member's range, moved by its offset, intersected.
    range: PitchRange,
    /// The instrument of the voice's first member.
    colour: InstrumentType,
    /// The members by place among the carriers, each with the semitones it plays above the
    /// voice: 0 in unison, −12 an octave below.
    members: Vec<(usize, i64)>,
}

impl VoiceGroup {
    /// A voice played by the carrier at `place` alone.
    fn solo(carriers: &[Carrier], place: usize) -> Result<Self, VoicingError> {
        let carrier = carriers.get(place).ok_or(VoicingError::PartCount {
            min: place + 1,
            max: place + 1,
            given: carriers.len(),
        })?;
        let range = comfortable(carrier.instrument).ok_or_else(|| wrong(carrier))?;
        Ok(Self {
            range,
            colour: carrier.instrument,
            members: vec![(place, 0)],
        })
    }

    /// Adds the carrier at `place` playing `offset` semitones above the voice, when the voice's
    /// range and the member's meet; returns whether it joined.
    fn join(&mut self, carriers: &[Carrier], place: usize, offset: i64) -> bool {
        let joined = carriers
            .get(place)
            .and_then(|carrier| comfortable(carrier.instrument))
            .and_then(|range| shifted(range, -offset))
            .and_then(|range| intersect(self.range, range));
        if let Some(range) = joined {
            self.range = range;
            self.members.push((place, offset));
        }
        joined.is_some()
    }
}

/// The error for a carrier the helper does not voice.
fn wrong(carrier: &Carrier) -> VoicingError {
    VoicingError::WrongInstrument {
        part: carrier.part.clone(),
        instrument: carrier.instrument,
    }
}

/// How [`write_chords`] lays out a family's chords.
struct ChordRules<'a> {
    /// Whether the lowest voice is the chord's bass.
    carries_bass: bool,
    /// The family's own check of a layout, pitches from the bottom.
    accept: &'a dyn Fn(&[i64]) -> bool,
    /// The velocity of every note.
    velocity: u8,
}

/// Whether every two notes that the members of `groups` sound for the voices `pitches` clear their
/// low-interval limits (SPC-01, SPC-02), octave doublings included.
fn members_clear(groups: &[VoiceGroup], pitches: &[i64]) -> bool {
    let mut sounding: Vec<i64> = groups
        .iter()
        .zip(pitches)
        .flat_map(|(voice, &pitch)| voice.members.iter().map(move |&(_, offset)| pitch + offset))
        .collect();
    sounding.sort_unstable();
    sounding.dedup();
    sounding.iter().enumerate().all(|(index, &lower)| {
        sounding
            .iter()
            .skip(index + 1)
            .all(|&upper| layout::clears_limits(lower, upper))
    })
}

/// Lays out every chord for the voices of `groups` (from the bottom) and writes each member's
/// notes into `score`; returns the layout of each slot in the order the slots sound, `None` for
/// a slot no layout fits.
fn write_chords(
    score: &mut Score<'_>,
    slots: &[&ChordSlot],
    groups: &[VoiceGroup],
    rules: &ChordRules<'_>,
    ids: &mut dyn IdGenerator,
) -> Result<Vec<Option<Vec<i64>>>, VoicingError> {
    let voices: Vec<layout::Voice> = groups
        .iter()
        .map(|voice| layout::Voice {
            range: voice.range,
            colour: voice.colour,
        })
        .collect();
    let accept = |pitches: &[i64]| (rules.accept)(pitches) && members_clear(groups, pitches);
    let layout_rules = layout::Rules {
        carries_bass: rules.carries_bass,
        accept: &accept,
    };
    let velocity = rules.velocity;
    let mut layouts = Vec::with_capacity(slots.len());
    let mut previous: Option<Vec<i64>> = None;
    for slot in slots {
        let pitches = layout::lay_out(&slot.chord, &voices, &layout_rules, previous.as_deref());
        if let Some(pitches) = &pitches {
            for (voice, &pitch) in groups.iter().zip(pitches) {
                for &(place, offset) in &voice.members {
                    let note = new_note(ids, pitch + offset, slot.start, slot.length, velocity)?;
                    score.push(place, note);
                }
            }
            previous = Some(pitches.clone());
        } else {
            score.remarks.push(Remark::Unvoiced { start: slot.start });
        }
        layouts.push(pitches);
    }
    for (index, voice) in groups.iter().enumerate() {
        if let Some(&(lead, _)) = voice.members.first() {
            let bass = index == 0 && rules.carries_bass;
            for &(place, offset) in voice.members.iter().skip(1) {
                let octaves = i8::try_from(offset / 12).unwrap_or_default();
                score.double(place, lead, octaves, bass);
            }
        }
    }
    Ok(layouts)
}

#[cfg(test)]
mod tests {
    use sonara_core::ids::SequentialIds;

    use super::{
        Carrier, ChordRules, PartId, Remark, Score, VoiceGroup, sorted_slots, write_chords,
    };
    use crate::generators::ChordSlot;
    use crate::orchestration::{InstrumentType, PitchRange};
    use crate::theory::parse_chord;

    #[test]
    fn a_chord_no_layout_fits_is_left_out_and_reported() {
        // TST-03: three voices in C4-C5 lay out C major but not B diminished over its bass, the
        // only B being the top note; the family rests there and the next chord leads on from
        // the last one it played.
        let carriers: Vec<Carrier> = ["a", "b", "c"]
            .into_iter()
            .map(|slug| Carrier::new(PartId::new(slug).unwrap(), InstrumentType::Viola))
            .collect();
        let groups: Vec<VoiceGroup> = (0..3)
            .map(|place| VoiceGroup {
                range: PitchRange::new(60, 72),
                colour: InstrumentType::Viola,
                members: vec![(place, 0)],
            })
            .collect();
        let slots: Vec<ChordSlot> = ["C", "Bdim", "F"]
            .into_iter()
            .zip(0_u32..)
            .map(|(symbol, index)| ChordSlot {
                chord: parse_chord(symbol).unwrap(),
                start: f64::from(index) * 2.0,
                length: 2.0,
            })
            .collect();
        let sorted = sorted_slots(&slots).unwrap();
        let mut score = Score::new(&carriers);
        let rules = ChordRules {
            carries_bass: true,
            accept: &|_| true,
            velocity: 64,
        };
        let layouts = write_chords(
            &mut score,
            &sorted,
            &groups,
            &rules,
            &mut SequentialIds::new(0),
        )
        .unwrap();
        assert_eq!(
            layouts,
            [Some(vec![60, 64, 67]), None, Some(vec![65, 69, 72])]
        );
        let voiced = score.finish();
        assert_eq!(voiced.remarks, [Remark::Unvoiced { start: 2.0 }]);
        let starts: Vec<f64> = voiced.parts[0].notes.iter().map(|n| n.start).collect();
        assert_eq!(starts, [0.0, 4.0]);
    }
}
