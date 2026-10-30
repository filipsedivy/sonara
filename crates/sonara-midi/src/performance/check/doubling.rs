//! Doublings: two parts that start their notes together at one interval for [`RUN_NOTES`] notes
//! or more. The woodwind order inverted (DBL-02), two winds of one colour in octaves without the
//! strings (DBL-03), a woodwind an octave above a trombone (DBL-04), and strings in unison with
//! brass (DBL-09).

use sonara_core::midi::MidiNote;

use super::{BEAT_EPSILON, Context, Hit, HitBuilder, Part, onsets, pitch};
use crate::orchestration::rules::{RUN_NOTES, RuleId, WOODWIND_ORDER};
use crate::orchestration::{Family, InstrumentType};

/// Semitones in an octave.
const OCTAVE: i64 = 12;

/// The intervals of a doubling in octaves: one, two or three octaves.
const OCTAVES: [i64; 3] = [OCTAVE, 2 * OCTAVE, 3 * OCTAVE];

/// The intervals of a doubling in thirds or sixths, in either size.
const THIRDS_AND_SIXTHS: [i64; 4] = [3, 4, 8, 9];

/// The trombones a woodwind should not double an octave above.
const TROMBONES: [InstrumentType; 2] =
    [InstrumentType::TromboneTenor, InstrumentType::TromboneBass];

/// The trumpets that suit an octave above a trombone better.
const TRUMPETS: [InstrumentType; 3] = [
    InstrumentType::TrumpetPiccolo,
    InstrumentType::TrumpetC,
    InstrumentType::TrumpetBb,
];

/// A run of notes that two parts start together, each pair the upper part's note first.
type Run<'a> = Vec<(&'a MidiNote, &'a MidiNote)>;

/// DBL-02: a woodwind of a lower branch of the natural order doubling one of a higher branch an
/// octave, a third or a sixth above it, such as a bassoon above a clarinet.
pub(super) fn inverted_woodwind_order(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Dbl02);
    let mut hits = Vec::new();
    for (upper, lower) in ordered_pairs(context) {
        let (Some(upper_rank), Some(lower_rank)) = (
            WOODWIND_ORDER.rank(upper.instrument),
            WOODWIND_ORDER.rank(lower.instrument),
        ) else {
            continue;
        };
        if upper_rank <= lower_rank {
            continue;
        }
        let interval =
            |semitones: i64| OCTAVES.contains(&semitones) || THIRDS_AND_SIXTHS.contains(&semitones);
        for run in runs(upper, lower, interval) {
            let detail = format!(
                "the {} doubles the {} from above for {} notes",
                upper.instrument.get().name(),
                lower.instrument.get().name(),
                run.len(),
            );
            hits.extend(builder.notes(builder.fixed(), &[upper, lower], &flatten(&run), detail));
        }
    }
    hits
}

/// DBL-03: two parts of one woodwind instrument in octaves while no string part plays their
/// pitch class with every note of the run.
pub(super) fn winds_in_octaves_without_strings(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Dbl03);
    let strings: Vec<&Part<'_>> = context
        .parts_where(|instrument| instrument.family() == Family::Strings)
        .collect();
    let mut hits = Vec::new();
    for (upper, lower) in ordered_pairs(context) {
        if upper.instrument != lower.instrument || upper.instrument.family() != Family::Woodwinds {
            continue;
        }
        for run in runs(upper, lower, |semitones| OCTAVES.contains(&semitones)) {
            if strings.iter().any(|part| doubles_run(part, &run, |_| true)) {
                continue;
            }
            let detail = format!(
                "two {} parts in octaves for {} notes with no string part doubling them",
                upper.instrument.get().name(),
                run.len(),
            );
            hits.extend(builder.notes(builder.fixed(), &[upper, lower], &flatten(&run), detail));
        }
    }
    hits
}

/// DBL-04: a woodwind an octave above a trombone while no trumpet doubles the trombone an octave
/// above with every note of the run.
pub(super) fn woodwind_above_trombone(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Dbl04);
    let trumpets: Vec<&Part<'_>> = context
        .parts_where(|instrument| TRUMPETS.contains(&instrument))
        .collect();
    let mut hits = Vec::new();
    for (upper, lower) in ordered_pairs(context) {
        if upper.instrument.family() != Family::Woodwinds || !TROMBONES.contains(&lower.instrument)
        {
            continue;
        }
        for run in runs(upper, lower, |semitones| semitones == OCTAVE) {
            let octave_above = |pair: (&MidiNote, &MidiNote), note: &MidiNote| {
                pitch(note) == pitch(pair.1) + OCTAVE
            };
            if trumpets
                .iter()
                .any(|part| doubles_run(part, &run, |(pair, note)| octave_above(pair, note)))
            {
                continue;
            }
            let detail = format!(
                "the {} doubles the {} an octave above for {} notes with no trumpet",
                upper.instrument.get().name(),
                lower.instrument.get().name(),
                run.len(),
            );
            hits.extend(builder.notes(builder.fixed(), &[upper, lower], &flatten(&run), detail));
        }
    }
    hits
}

/// DBL-09: a string part and a brass part in unison while no woodwind part plays their pitch
/// class with every note of the run.
pub(super) fn strings_and_brass_in_unison(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Dbl09);
    let woodwinds: Vec<&Part<'_>> = context
        .parts_where(|instrument| instrument.family() == Family::Woodwinds)
        .collect();
    let mut hits = Vec::new();
    for (upper, lower) in ordered_pairs(context) {
        if upper.instrument.family() != Family::Strings
            || lower.instrument.family() != Family::Brass
        {
            continue;
        }
        for run in runs(upper, lower, |semitones| semitones == 0) {
            if woodwinds
                .iter()
                .any(|part| doubles_run(part, &run, |_| true))
            {
                continue;
            }
            let detail = format!(
                "the {} and the {} in unison for {} notes with no woodwind",
                upper.instrument.get().name(),
                lower.instrument.get().name(),
                run.len(),
            );
            hits.extend(builder.notes(builder.fixed(), &[upper, lower], &flatten(&run), detail));
        }
    }
    hits
}

/// Every ordered pair of two different parts, the first taken as the upper.
fn ordered_pairs<'p, 'a>(context: &'p Context<'a>) -> Vec<(&'p Part<'a>, &'p Part<'a>)> {
    let mut pairs = Vec::new();
    for upper in &context.parts {
        for lower in &context.parts {
            if upper.place != lower.place {
                pairs.push((upper, lower));
            }
        }
    }
    pairs
}

/// The runs of at least [`RUN_NOTES`] onsets in a row at which both parts start a note together,
/// a note of `upper` lying `semitones` above one of `lower` for which `interval` holds. A run ends
/// at an onset of only one part, or of both at another interval.
fn runs<'a>(upper: &Part<'a>, lower: &Part<'a>, interval: impl Fn(i64) -> bool) -> Vec<Run<'a>> {
    let upper_onsets = onsets(&upper.notes);
    let lower_onsets = onsets(&lower.notes);
    let mut runs: Vec<Run<'a>> = Vec::new();
    let mut current: Run<'a> = Vec::new();
    let (mut u, mut l) = (0, 0);
    while let (Some(up), Some(low)) = (upper_onsets.get(u), lower_onsets.get(l)) {
        let (Some(up_start), Some(low_start)) = (
            up.first().map(|note| note.start),
            low.first().map(|note| note.start),
        ) else {
            break;
        };
        if (up_start - low_start).abs() < BEAT_EPSILON {
            let pair = up.iter().find_map(|&a| {
                low.iter()
                    .find(|&&b| interval(pitch(a) - pitch(b)))
                    .map(|&b| (a, b))
            });
            match pair {
                Some(pair) => current.push(pair),
                None => runs.push(std::mem::take(&mut current)),
            }
            u += 1;
            l += 1;
        } else {
            runs.push(std::mem::take(&mut current));
            if up_start < low_start {
                u += 1;
            } else {
                l += 1;
            }
        }
    }
    runs.push(current);
    runs.retain(|run| run.len() >= usize::from(RUN_NOTES.value));
    runs
}

/// Whether `part` starts a note with every pair of `run` for which `matches` holds, given the
/// pair and the note; by default, a note of the pair's pitch class.
fn doubles_run<'a>(
    part: &Part<'a>,
    run: &Run<'a>,
    matches: impl Fn(((&MidiNote, &MidiNote), &MidiNote)) -> bool,
) -> bool {
    run.iter().all(|&(a, b)| {
        part.notes.iter().any(|&note| {
            (note.start - a.start).abs() < BEAT_EPSILON
                && (pitch(note) - pitch(a)).rem_euclid(OCTAVE) == 0
                && matches(((a, b), note))
        })
    })
}

/// The notes of a run, both parts'.
fn flatten<'a>(run: &Run<'a>) -> Vec<&'a MidiNote> {
    run.iter().flat_map(|&(a, b)| [a, b]).collect()
}
