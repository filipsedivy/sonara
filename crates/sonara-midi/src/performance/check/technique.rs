//! Changes of technique and what a string player can hold: sustained multiple stops (VOI-12),
//! time for a mute (PLY-04), the bow's return after pizzicato (PLY-05) and the speed of
//! pizzicato (PLY-06).

use sonara_core::midi::MidiNote;

use super::{
    BEAT_EPSILON, Context, Hit, HitBuilder, articulation, count, is_pizzicato, latest_end, onsets,
    velocity_band,
};
use crate::orchestration::rules::{
    DOUBLE_BASS_PIZZICATO, DynamicBand, PIZZICATO_SPEED, RUN_NOTES, RuleId, SHORT_CHORD,
};
use crate::orchestration::{Articulation, Axis, FactKind, Family, InstrumentType, Mute, Span};

/// VOI-12: a string part sounding more notes at once than its players hold, for longer than
/// [`SHORT_CHORD`] or with every note soft.
pub(super) fn held_multi_stops(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Voi12);
    let mut hits = Vec::new();
    for part in context.parts_where(|instrument| instrument.family() == Family::Strings) {
        let Some(held) = sustained_stops(part.instrument) else {
            continue;
        };
        for (start, end) in crowded_spans(&part.notes, usize::from(held)) {
            let notes: Vec<&MidiNote> = part
                .notes
                .iter()
                .copied()
                .filter(|note| note.start < end - BEAT_EPSILON && note.end() > start + BEAT_EPSILON)
                .collect();
            let long = context.millis(start, end) > f64::from(SHORT_CHORD.value.millis());
            let soft = notes
                .iter()
                .all(|note| velocity_band(note.velocity) == DynamicBand::Soft);
            if long || soft {
                let detail = format!(
                    "{} at once for {:.0} ms{}; only the upper {held} can be held",
                    count(&notes),
                    context.millis(start, end),
                    if soft { " at p" } else { "" },
                );
                hits.push(builder.span(builder.fixed(), &[part], &notes, start, end, detail));
            }
        }
    }
    hits
}

/// PLY-04: a part putting on a mute, taking it off or changing it with less free time than the
/// instrument's mute change needs: the time to put it on, to take it off, or both to change one
/// mute for another. The free time runs from the end of the last note before the change to the
/// start of the first note after it. Only a mute the player puts on and takes off counts
/// ([`Mute::is_device`]).
pub(super) fn mute_changes(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Ply04);
    let mut hits = Vec::new();
    for part in &context.parts {
        let Some((on, off)) = mute_change(part.instrument) else {
            continue;
        };
        let mut before: Vec<&MidiNote> = Vec::new();
        let mut state = Mute::Open;
        for &note in &part.notes {
            let mute = mute_of(note);
            if let (Some(&last), Some(end)) = (before.last(), latest_end(&before))
                && mute != state
            {
                let needed = match (state, mute) {
                    (Mute::Open, _) => on.millis(),
                    (_, Mute::Open) => off.millis(),
                    _ => on.millis().saturating_add(off.millis()),
                };
                let free = context.millis(end, note.start).max(0.0);
                if free < f64::from(needed) {
                    let action = match (state, mute) {
                        (Mute::Open, _) => "put on",
                        (_, Mute::Open) => "taken off",
                        _ => "changed",
                    };
                    let detail = format!(
                        "the mute is {action} with {:.1} s free; the {} needs {:.1} s",
                        free / 1000.0,
                        part.instrument.get().name(),
                        f64::from(needed) / 1000.0,
                    );
                    hits.extend(builder.notes(builder.fixed(), &[part], &[last, note], detail));
                }
            }
            state = mute;
            before.push(note);
        }
    }
    hits
}

/// PLY-05: a string part changing from pizzicato to arco with less time than the bow needs to
/// come back. The time runs from the last pizzicato note's start, when the string is plucked and
/// the hand is free, to the first bowed note's start.
pub(super) fn pizzicato_to_arco(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Ply05);
    let mut hits = Vec::new();
    for part in &context.parts {
        let Some(to_arco) = to_arco(part.instrument) else {
            continue;
        };
        let mut last_pizzicato: Option<&MidiNote> = None;
        for &note in &part.notes {
            if is_pizzicato(note) {
                last_pizzicato = Some(note);
                continue;
            }
            if let Some(plucked) = last_pizzicato.take() {
                let free = context.millis(plucked.start, note.start);
                if free < f64::from(to_arco.millis()) {
                    let detail = format!(
                        "arco {:.2} s after the last pizzicato; the bow needs {:.2} s",
                        free / 1000.0,
                        f64::from(to_arco.millis()) / 1000.0,
                    );
                    hits.extend(builder.notes(builder.fixed(), &[part], &[plucked, note], detail));
                }
            }
        }
    }
    hits
}

/// PLY-06: [`RUN_NOTES`] pizzicato notes or more on a bowed string, each plucked less than
/// [`PIZZICATO_SPEED`] after the one before, or less than [`DOUBLE_BASS_PIZZICATO`] on the
/// double bass. A chord is one pluck.
pub(super) fn pizzicato_speed(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Ply06);
    let mut hits = Vec::new();
    for part in context.parts_where(plucks) {
        let span = if part.instrument == InstrumentType::DoubleBass {
            DOUBLE_BASS_PIZZICATO.value
        } else {
            PIZZICATO_SPEED.value
        };
        let mut runs: Vec<Vec<&MidiNote>> = Vec::new();
        for group in onsets(&part.notes) {
            let Some(&note) = group.first() else {
                continue;
            };
            let plucked = group.iter().all(|note| is_pizzicato(note));
            match runs.last_mut() {
                Some(run)
                    if plucked
                        && run.last().is_some_and(|last| {
                            context.shorter_than(last.start, note.start, span)
                        }) =>
                {
                    run.push(note);
                }
                _ if plucked => runs.push(vec![note]),
                _ => runs.push(Vec::new()),
            }
        }
        for run in runs {
            if run.len() >= usize::from(RUN_NOTES.value) {
                let detail = format!(
                    "{} plucked faster than one every {} ms on the {}",
                    count(&run),
                    span.millis(),
                    part.instrument.get().name(),
                );
                hits.extend(builder.notes(builder.fixed(), &[part], &run, detail));
            }
        }
    }
    hits
}

/// Whether the instrument is a bowed string that plays pizzicato.
fn plucks(instrument: InstrumentType) -> bool {
    instrument.family() == Family::Strings
        && instrument
            .get()
            .articulations()
            .contains(&Articulation::Pizzicato)
}

/// The mute `note` is played with: the mute its articulation names when the player puts it on and
/// takes it off, otherwise none.
fn mute_of(note: &MidiNote) -> Mute {
    match articulation(note).map(Articulation::axis) {
        Some(Axis::Mute(mute)) if mute.is_device() => mute,
        _ => Mute::Open,
    }
}

/// How many notes of a chord a player of the instrument holds, when its record states it.
fn sustained_stops(instrument: InstrumentType) -> Option<u8> {
    instrument
        .get()
        .facts()
        .iter()
        .find_map(|fact| match fact.kind {
            FactKind::SustainedStops { notes } => Some(notes),
            _ => None,
        })
}

/// The time to put a mute on and to take it off, when the instrument's record states it.
fn mute_change(instrument: InstrumentType) -> Option<(Span, Span)> {
    instrument
        .get()
        .facts()
        .iter()
        .find_map(|fact| match fact.kind {
            FactKind::MuteChange { on, off } => Some((on, off)),
            _ => None,
        })
}

/// The time from pizzicato back to arco, when the instrument's record states it.
fn to_arco(instrument: InstrumentType) -> Option<Span> {
    instrument
        .get()
        .facts()
        .iter()
        .find_map(|fact| match fact.kind {
            FactKind::ArcoPizzSwitch { to_arco } => Some(to_arco),
            _ => None,
        })
}

/// The spans, in content beats, in which more than `held` of `notes` sound at once; spans that
/// touch are joined.
fn crowded_spans(notes: &[&MidiNote], held: usize) -> Vec<(f64, f64)> {
    let mut events: Vec<(f64, bool)> = notes
        .iter()
        .flat_map(|note| [(note.start, true), (note.end(), false)])
        .collect();
    // At one beat a note's end comes before another's start: the two do not sound together.
    events.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut spans: Vec<(f64, f64)> = Vec::new();
    let mut sounding = 0_usize;
    let mut opened: Option<f64> = None;
    for (beat, starts) in events {
        if starts {
            sounding = sounding.saturating_add(1);
        } else {
            sounding = sounding.saturating_sub(1);
        }
        match (sounding > held, opened) {
            (true, None) => opened = Some(beat),
            (false, Some(start)) => {
                match spans.last_mut() {
                    Some(last) if (start - last.1).abs() < BEAT_EPSILON => last.1 = beat,
                    _ => spans.push((start, beat)),
                }
                opened = None;
            }
            _ => {}
        }
    }
    spans.retain(|(start, end)| end - start > BEAT_EPSILON);
    spans
}

#[cfg(test)]
mod tests {
    use sonara_core::midi::MidiNote;

    use super::crowded_spans;

    fn note(start: f64, length: f64) -> MidiNote {
        MidiNote::new("n", 60, start, length, 80, 0).expect("a valid note")
    }

    #[test]
    fn crowded_spans_join_and_ignore_touching_notes() {
        // TST-10: three notes overlap from beat 1 to 2; a note that starts where another ends
        // does not sound with it.
        let notes = [
            note(0.0, 2.0),
            note(1.0, 2.0),
            note(1.0, 1.0),
            note(2.0, 1.0),
        ];
        let refs: Vec<&MidiNote> = notes.iter().collect();
        assert_eq!(crowded_spans(&refs, 2), vec![(1.0, 2.0)]);
        assert_eq!(crowded_spans(&refs, 1), vec![(1.0, 3.0)]);
        assert_eq!(crowded_spans(&refs, 3), vec![]);
    }
}
