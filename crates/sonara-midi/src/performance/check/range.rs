//! The range rules: notes outside the playable or the comfortable range (RNG-01, RNG-02) and
//! high brass held too long (RNG-03).

use sonara_core::midi::MidiNote;

use super::{Context, Hit, HitBuilder, Part, count, latest_end, named, named_pitch, pitch};
use crate::orchestration::rules::{RuleId, SUSTAINED_HIGH_BRASS, WIND_REST};
use crate::orchestration::{Family, PitchModel, RangeCheck, SoundingPitch};

/// RNG-01: every run of notes in a row below or above a part's playable range.
pub(super) fn outside_playable(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Rng01);
    let mut hits = Vec::new();
    for part in &context.parts {
        for (outcome, run) in runs(part) {
            let side = match outcome {
                RangeCheck::BelowPlayable => "below",
                RangeCheck::AbovePlayable => "above",
                RangeCheck::Within | RangeCheck::OutsideComfortable => continue,
            };
            let detail = format!(
                "{} {side} the playable range of the {}, {}",
                count(&run),
                part.instrument.get().name(),
                playable(part).unwrap_or_default(),
            );
            hits.extend(builder.notes(builder.fixed(), &[part], &run, detail));
        }
    }
    hits
}

/// RNG-02: every run of notes in a row that a part can play but that lie outside its comfortable
/// range.
pub(super) fn outside_comfortable(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Rng02);
    let mut hits = Vec::new();
    for part in &context.parts {
        let Some(comfortable) = comfortable(part) else {
            continue;
        };
        for (outcome, run) in runs(part) {
            if outcome != RangeCheck::OutsideComfortable {
                continue;
            }
            let detail = format!(
                "{} outside the comfortable range of the {}, {}",
                count(&run),
                part.instrument.get().name(),
                comfortable,
            );
            hits.extend(builder.notes(builder.fixed(), &[part], &run, detail));
        }
    }
    hits
}

/// RNG-03: a brass part whose notes above its comfortable range follow each other, with no other
/// note and no rest of [`WIND_REST`] between them, for longer than [`SUSTAINED_HIGH_BRASS`].
pub(super) fn sustained_high_brass(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Rng03);
    let mut hits = Vec::new();
    for part in context.parts_where(|instrument| instrument.family() == Family::Brass) {
        let Some(top) = comfortable_range(part).map(|(_, high)| high.get()) else {
            continue;
        };
        let mut runs: Vec<Vec<&MidiNote>> = Vec::new();
        let mut current: Vec<&MidiNote> = Vec::new();
        for &note in &part.notes {
            let rested = latest_end(&current)
                .is_some_and(|end| !context.shorter_than(end, note.start, WIND_REST.value));
            if pitch(note) <= top || rested {
                runs.push(std::mem::take(&mut current));
            }
            if pitch(note) > top {
                current.push(note);
            }
        }
        runs.push(current);
        for run in runs {
            let (Some(first), Some(end)) = (run.first(), latest_end(&run)) else {
                continue;
            };
            let millis = context.millis(first.start, end);
            if millis > f64::from(SUSTAINED_HIGH_BRASS.value.millis()) {
                let detail = format!(
                    "{} above the comfortable top {} for {:.1} s without a rest",
                    count(&run),
                    named(top),
                    millis / 1000.0,
                );
                hits.extend(builder.notes(builder.fixed(), &[part], &run, detail));
            }
        }
    }
    hits
}

/// The part's notes in runs of one outcome of the range check, in note order; an unpitched
/// instrument has none.
fn runs<'a>(part: &Part<'a>) -> Vec<(RangeCheck, Vec<&'a MidiNote>)> {
    let record = part.instrument.get();
    let mut runs: Vec<(RangeCheck, Vec<&MidiNote>)> = Vec::new();
    for &note in &part.notes {
        let Some(outcome) = SoundingPitch::new(pitch(note))
            .and_then(|sounding| record.check_sounding(sounding).ok())
        else {
            continue;
        };
        match runs.last_mut() {
            Some((last, run)) if *last == outcome => run.push(note),
            _ => runs.push((outcome, vec![note])),
        }
    }
    runs
}

/// The comfortable range of the part's instrument, or `None` when it is unpitched or none is
/// known.
fn comfortable_range(part: &Part<'_>) -> Option<(SoundingPitch, SoundingPitch)> {
    match part.instrument.get().pitch() {
        PitchModel::Pitched(pitched) => pitched
            .comfortable
            .map(|range| (range.range.low(), range.range.high())),
        PitchModel::Unpitched => None,
    }
}

/// The comfortable range in words, such as `C3 (48) to C6 (84)`.
fn comfortable(part: &Part<'_>) -> Option<String> {
    comfortable_range(part)
        .map(|(low, high)| format!("{} to {}", named_pitch(low), named_pitch(high)))
}

/// The playable range in words.
fn playable(part: &Part<'_>) -> Option<String> {
    match part.instrument.get().pitch() {
        PitchModel::Pitched(pitched) => Some(format!(
            "{} to {}",
            named_pitch(pitched.playable.range.low()),
            named_pitch(pitched.playable.range.high()),
        )),
        PitchModel::Unpitched => None,
    }
}
