//! Wind and brass players: breath (PLY-01), brass endurance (PLY-02), fast repeated notes
//! (PLY-14) and the clarinet's octave leaps (PLY-15).

use sonara_core::midi::MidiNote;

use super::{Context, Hit, HitBuilder, count, latest_end, named, onsets, pitch, velocity_band};
use crate::orchestration::rules::{
    DynamicBand, FLUTE_LOW_NOTES, LOUD_BRASS_PHRASE, RAPID_LEAP, RUN_NOTES, RuleId,
    SINGLE_TONGUING, WIND_PHRASE, WIND_REST, register_factor,
};
use crate::orchestration::{
    Articulation, FactKind, Family, InstrumentType, PitchModel, SoundingPitch, Span,
};

/// Semitones in an octave.
const OCTAVE: i64 = 12;

/// The clarinets the leap rule names: the B♭ and A clarinets, which the handbooks call simply
/// the clarinet.
const CLARINETS: [InstrumentType; 2] = [InstrumentType::ClarinetBb, InstrumentType::ClarinetA];

/// PLY-01: a wind part that plays longer than [`WIND_PHRASE`] without a rest of [`WIND_REST`]; a
/// note held longer than the instrument's breath; the flute playing more than
/// [`FLUTE_LOW_NOTES`] notes in its lowest register without such a rest.
pub(super) fn breath(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Ply01);
    let mut hits = Vec::new();
    for part in context.parts_where(is_wind) {
        for phrase in phrases(context, &part.notes, |_| true) {
            let millis = phrase_millis(context, &phrase);
            if millis > f64::from(WIND_PHRASE.value.millis()) {
                let detail = format!(
                    "{} without a rest for {:.1} s",
                    count(&phrase),
                    millis / 1000.0
                );
                hits.extend(builder.notes(builder.fixed(), &[part], &phrase, detail));
            }
        }
        if let Some(held) = held_note(part.instrument) {
            for &note in &part.notes {
                let millis = context.millis(note.start, note.end());
                if millis > f64::from(held.millis()) {
                    let detail = format!(
                        "one note held for {:.1} s, longer than the {}'s breath of {:.1} s",
                        millis / 1000.0,
                        part.instrument.get().name(),
                        f64::from(held.millis()) / 1000.0,
                    );
                    hits.extend(builder.notes(builder.fixed(), &[part], &[note], detail));
                }
            }
        }
        if part.instrument == InstrumentType::Flute {
            for run in phrases(context, &part.notes, is_low_flute) {
                if run.len() > usize::from(FLUTE_LOW_NOTES.value) {
                    let detail = format!(
                        "{} in the flute's lowest register without a rest",
                        count(&run)
                    );
                    hits.extend(builder.notes(builder.fixed(), &[part], &run, detail));
                }
            }
        }
    }
    hits
}

/// PLY-02: a brass part that plays loud or high notes for longer than [`LOUD_BRASS_PHRASE`]
/// without a rest of [`WIND_REST`], or [`RUN_NOTES`] repeated notes or more outside its middle
/// register without such a rest. The middle register is the middle third of the comfortable
/// range, as the trumpet's compass divides into a low, a middle and a high register.
pub(super) fn brass_endurance(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Ply02);
    let mut hits = Vec::new();
    for part in context.parts_where(|instrument| instrument.family() == Family::Brass) {
        let Some((low, high)) = middle_register(part.instrument) else {
            continue;
        };
        let strenuous = |note: &MidiNote| {
            velocity_band(note.velocity) == DynamicBand::Loud || pitch(note) > high
        };
        for phrase in phrases(context, &part.notes, strenuous) {
            let millis = phrase_millis(context, &phrase);
            if millis > f64::from(LOUD_BRASS_PHRASE.value.millis()) {
                let detail = format!(
                    "{} loud or high without a rest for {:.1} s",
                    count(&phrase),
                    millis / 1000.0
                );
                hits.extend(builder.notes(builder.fixed(), &[part], &phrase, detail));
            }
        }
        let outside = |note: &MidiNote| pitch(note) < low || pitch(note) > high;
        for phrase in phrases(context, &part.notes, outside) {
            for run in repeated(&phrase) {
                if run.len() >= usize::from(RUN_NOTES.value) {
                    let detail = format!(
                        "{} repeated on {} outside the middle register {} to {}",
                        count(&run),
                        run.first()
                            .map(|note| named(pitch(note)))
                            .unwrap_or_default(),
                        named(low),
                        named(high),
                    );
                    hits.extend(builder.notes(builder.fixed(), &[part], &run, detail));
                }
            }
        }
    }
    hits
}

/// PLY-14: [`RUN_NOTES`] repeated notes or more, each starting less than [`SINGLE_TONGUING`] after
/// the one before, on a woodwind or brass instrument whose articulations do not include multiple
/// tonguing.
pub(super) fn fast_repetitions(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Ply14);
    let mut hits = Vec::new();
    let single_tongued = |instrument: InstrumentType| {
        is_wind(instrument)
            && !instrument
                .get()
                .articulations()
                .contains(&Articulation::MultiTongue)
    };
    for part in context.parts_where(single_tongued) {
        for run in fast_runs(context, &part.notes, SINGLE_TONGUING.value, |a, b| {
            a.pitch == b.pitch
        }) {
            if run.len() >= usize::from(RUN_NOTES.value) {
                let detail = format!(
                    "{} repeated on {} faster than single tonguing; the {} does not multi-tongue",
                    count(&run),
                    run.first()
                        .map(|note| named(pitch(note)))
                        .unwrap_or_default(),
                    part.instrument.get().name(),
                );
                hits.extend(builder.notes(builder.fixed(), &[part], &run, detail));
            }
        }
    }
    hits
}

/// PLY-15: a clarinet leaping an octave or more between two notes that start less than
/// [`RAPID_LEAP`] apart; leaps that follow one another are one hit.
pub(super) fn clarinet_leaps(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Ply15);
    let mut hits = Vec::new();
    for part in context.parts_where(|instrument| CLARINETS.contains(&instrument)) {
        let leap = |a: &MidiNote, b: &MidiNote| (pitch(a) - pitch(b)).abs() >= OCTAVE;
        for run in fast_runs(context, &part.notes, RAPID_LEAP.value, leap) {
            if run.len() >= 2 {
                let detail = format!("{} rapid leaps of an octave or more", run.len() - 1);
                hits.extend(builder.notes(builder.fixed(), &[part], &run, detail));
            }
        }
    }
    hits
}

/// Whether the instrument is a wind: a woodwind or brass instrument.
fn is_wind(instrument: InstrumentType) -> bool {
    matches!(instrument.family(), Family::Woodwinds | Family::Brass)
}

/// The longest note the instrument holds on one breath, when its record states it.
fn held_note(instrument: InstrumentType) -> Option<Span> {
    instrument
        .get()
        .facts()
        .iter()
        .find_map(|fact| match fact.kind {
            FactKind::Breath { held_note } => Some(held_note),
            _ => None,
        })
}

/// Whether `note` lies in the flute's lowest register: the weak low register of its register
/// factors.
fn is_low_flute(note: &MidiNote) -> bool {
    SoundingPitch::new(pitch(note)).is_some_and(|sounding| {
        register_factor(
            InstrumentType::Flute,
            sounding,
            velocity_band(note.velocity),
            false,
        )
        .is_some_and(|row| row.factor < 1.0)
    })
}

/// The middle third of the instrument's comfortable range, lowest and highest pitch; `None` when
/// it is unpitched or has no comfortable range.
fn middle_register(instrument: InstrumentType) -> Option<(i64, i64)> {
    let PitchModel::Pitched(pitched) = instrument.get().pitch() else {
        return None;
    };
    let range = pitched.comfortable?.range;
    let (low, high) = (range.low().get(), range.high().get());
    let third = (high - low) / 3;
    Some((low + third, high - third))
}

/// The runs of notes for which `wanted` holds, in note order, with no rest of [`WIND_REST`]
/// between them; a note for which it does not hold ends a run.
fn phrases<'a>(
    context: &Context<'_>,
    notes: &[&'a MidiNote],
    wanted: impl Fn(&MidiNote) -> bool,
) -> Vec<Vec<&'a MidiNote>> {
    let mut phrases: Vec<Vec<&MidiNote>> = Vec::new();
    let mut current: Vec<&MidiNote> = Vec::new();
    for &note in notes {
        let rested = latest_end(&current)
            .is_some_and(|end| !context.shorter_than(end, note.start, WIND_REST.value));
        if !wanted(note) || rested {
            phrases.push(std::mem::take(&mut current));
        }
        if wanted(note) {
            current.push(note);
        }
    }
    phrases.push(current);
    phrases.retain(|phrase| !phrase.is_empty());
    phrases
}

/// The time from a phrase's first start to its last end, in milliseconds.
fn phrase_millis(context: &Context<'_>, phrase: &[&MidiNote]) -> f64 {
    match (phrase.first(), latest_end(phrase)) {
        (Some(first), Some(end)) => context.millis(first.start, end),
        _ => 0.0,
    }
}

/// The runs of notes in a row on one pitch.
fn repeated<'a>(notes: &[&'a MidiNote]) -> Vec<Vec<&'a MidiNote>> {
    let mut runs: Vec<Vec<&MidiNote>> = Vec::new();
    for &note in notes {
        match runs.last_mut() {
            Some(run) if run.last().is_some_and(|last| last.pitch == note.pitch) => run.push(note),
            _ => runs.push(vec![note]),
        }
    }
    runs
}

/// The runs of notes, one per onset (the lowest of a chord), in which each note starts less than
/// `span` after the one before and `joins` holds for the two.
fn fast_runs<'a>(
    context: &Context<'_>,
    notes: &[&'a MidiNote],
    span: Span,
    joins: impl Fn(&MidiNote, &MidiNote) -> bool,
) -> Vec<Vec<&'a MidiNote>> {
    let mut runs: Vec<Vec<&MidiNote>> = Vec::new();
    for group in onsets(notes) {
        let Some(&note) = group.first() else {
            continue;
        };
        match runs.last_mut() {
            Some(run)
                if run.last().is_some_and(|&last| {
                    joins(last, note) && context.shorter_than(last.start, note.start, span)
                }) =>
            {
                run.push(note);
            }
            _ => runs.push(vec![note]),
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::middle_register;
    use crate::orchestration::InstrumentType;

    #[test]
    fn the_middle_register_is_the_middle_third_of_the_comfortable_range() {
        // TST-10: the horn's comfortable range is F2-C5 (41-72): 31 semitones, thirds of 10.
        assert_eq!(middle_register(InstrumentType::HornF), Some((51, 62)));
        // The C trumpet's 54-84 gives 64-74, close to VSL's middle register G4-F#5.
        assert_eq!(middle_register(InstrumentType::TrumpetC), Some((64, 74)));
        assert_eq!(middle_register(InstrumentType::SnareDrum), None);
    }
}
