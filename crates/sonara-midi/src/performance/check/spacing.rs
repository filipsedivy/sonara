//! Intervals of notes that sound together: the low-interval limits (SPC-01), dissonance in the
//! low bass (SPC-02) and timpani struck a third apart (PLY-12).

use sonara_core::midi::MidiNote;

use super::{BEAT_EPSILON, Context, Hit, HitBuilder, Part, named, onsets, pitch, velocity_band};
use crate::orchestration::rules::{
    DynamicBand, LOW_DISSONANCE, LOW_INTERVAL_LIMITS, Limit, RuleId,
};
use crate::orchestration::{Family, InstrumentType, PitchModel};

/// Semitones in an octave.
const OCTAVE: i64 = 12;

/// The semitones modulo the octave of the intervals the low-bass rule calls dissonant: seconds,
/// fourths, the tritone and sevenths; the ninths are seconds an octave up.
const DISSONANT: [i64; 6] = [1, 2, 5, 6, 10, 11];

/// The widest interval the low-bass rule names, a major ninth.
const WIDEST_DISSONANCE: i64 = 14;

/// The minor and the major third.
const THIRDS: [i64; 2] = [3, 4];

/// Whether `lower` and `upper` sounding together lie below the low limit of their interval
/// (SPC-01): the lower note under the lowest note [`LOW_INTERVAL_LIMITS`] gives the interval. A
/// unison, an octave and an interval wider than the table never do.
pub(crate) fn below_low_limit(lower: i64, upper: i64) -> bool {
    u8::try_from(upper - lower)
        .ok()
        .and_then(|semitones| LOW_INTERVAL_LIMITS.of(semitones))
        .is_some_and(|limit| match limit {
            Limit::Lowest(pitch) => lower < pitch.get(),
            Limit::Unlimited => false,
        })
}

/// Whether `lower` and `upper` form a dissonance in the low bass (SPC-02): a second, fourth,
/// tritone, seventh or ninth whose lower note lies below [`LOW_DISSONANCE`].
pub(crate) fn is_low_dissonance(lower: i64, upper: i64) -> bool {
    let semitones = upper - lower;
    (1..=WIDEST_DISSONANCE).contains(&semitones)
        && DISSONANT.contains(&semitones.rem_euclid(OCTAVE))
        && lower < LOW_DISSONANCE.value.get()
}

/// SPC-01: each pair of pitched notes that sound together, in one part or two, whose interval lies
/// below its low limit. A warning when either note is loud or a brass instrument plays it, a hint
/// otherwise.
pub(super) fn low_interval_limits(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Spc01);
    let mut hits = Vec::new();
    for pair in sounding_pairs(context) {
        if !below_low_limit(pair.lower_pitch(), pair.upper_pitch()) {
            continue;
        }
        let loud = [pair.lower, pair.upper]
            .iter()
            .any(|note| velocity_band(note.velocity) == DynamicBand::Loud);
        let brass = [pair.lower_part, pair.upper_part]
            .iter()
            .any(|part| part.instrument.family() == Family::Brass);
        let detail = format!(
            "{} semitones from {} up to {} lie below the interval's low limit",
            pair.upper_pitch() - pair.lower_pitch(),
            named(pair.lower_pitch()),
            named(pair.upper_pitch()),
        );
        hits.extend(builder.notes(
            builder.level(DynamicBand::Medium, loud || brass),
            &[pair.lower_part, pair.upper_part],
            &[pair.lower, pair.upper],
            detail,
        ));
    }
    hits
}

/// SPC-02: each pair of pitched notes of two parts that sound together as a dissonance in the low
/// bass. A warning when both parts belong to one family, a hint otherwise.
pub(super) fn low_dissonances(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Spc02);
    let mut hits = Vec::new();
    for pair in sounding_pairs(context) {
        if pair.lower_part.place == pair.upper_part.place
            || !is_low_dissonance(pair.lower_pitch(), pair.upper_pitch())
        {
            continue;
        }
        let one_family = pair.lower_part.instrument.family() == pair.upper_part.instrument.family();
        let detail = format!(
            "a dissonance of {} semitones from {} up to {} lies below {}",
            pair.upper_pitch() - pair.lower_pitch(),
            named(pair.lower_pitch()),
            named(pair.upper_pitch()),
            named(LOW_DISSONANCE.value.get()),
        );
        hits.extend(builder.notes(
            builder.level(DynamicBand::Medium, one_family),
            &[pair.lower_part, pair.upper_part],
            &[pair.lower, pair.upper],
            detail,
        ));
    }
    hits
}

/// PLY-12: each group of timpani notes struck together that holds two notes a third apart, across
/// every timpani part.
pub(super) fn timpani_thirds(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Ply12);
    let parts: Vec<&Part<'_>> = context
        .parts_where(|instrument| instrument == InstrumentType::Timpani)
        .collect();
    let mut notes: Vec<&MidiNote> = parts
        .iter()
        .flat_map(|part| part.notes.iter().copied())
        .collect();
    notes.sort_by(|a, b| super::note_order(a, b));
    let mut hits = Vec::new();
    for group in onsets(&notes) {
        let thirds: Vec<&MidiNote> = group
            .iter()
            .copied()
            .filter(|a| {
                group
                    .iter()
                    .any(|b| THIRDS.contains(&(pitch(a) - pitch(b)).abs()))
            })
            .collect();
        if thirds.is_empty() {
            continue;
        }
        let struck: Vec<&Part<'_>> = parts
            .iter()
            .copied()
            .filter(|part| {
                part.notes
                    .iter()
                    .any(|note| thirds.iter().any(|third| std::ptr::eq(*note, *third)))
            })
            .collect();
        let pitches: Vec<String> = thirds.iter().map(|note| named(pitch(note))).collect();
        let detail = format!("timpani struck a third apart: {}", pitches.join(", "));
        hits.extend(builder.notes(builder.fixed(), &struck, &thirds, detail));
    }
    hits
}

/// Two pitched notes that sound together, the lower first.
struct Pair<'p, 'a> {
    lower: &'a MidiNote,
    lower_part: &'p Part<'a>,
    upper: &'a MidiNote,
    upper_part: &'p Part<'a>,
}

impl Pair<'_, '_> {
    fn lower_pitch(&self) -> i64 {
        pitch(self.lower)
    }

    fn upper_pitch(&self) -> i64 {
        pitch(self.upper)
    }
}

/// Every pair of notes of pitched instruments whose sounding times overlap, by the start of the
/// earlier note: in one part or in two, each pair once.
fn sounding_pairs<'p, 'a>(context: &'p Context<'a>) -> Vec<Pair<'p, 'a>> {
    let mut notes: Vec<(&'a MidiNote, &'p Part<'a>)> = context
        .parts
        .iter()
        .filter(|part| matches!(part.instrument.get().pitch(), PitchModel::Pitched(_)))
        .flat_map(|part| part.notes.iter().map(move |&note| (note, part)))
        .collect();
    notes.sort_by(|(a, a_part), (b, b_part)| {
        super::note_order(a, b).then(a_part.place.cmp(&b_part.place))
    });
    let mut pairs = Vec::new();
    for (index, &(first, first_part)) in notes.iter().enumerate() {
        for &(second, second_part) in notes.iter().skip(index + 1) {
            if second.start >= first.end() - BEAT_EPSILON {
                break;
            }
            let (lower, lower_part, upper, upper_part) = if second.pitch < first.pitch {
                (second, second_part, first, first_part)
            } else {
                (first, first_part, second, second_part)
            };
            pairs.push(Pair {
                lower,
                lower_part,
                upper,
                upper_part,
            });
        }
    }
    pairs
}
