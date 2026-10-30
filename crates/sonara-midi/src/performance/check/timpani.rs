//! The timpani's tuning: the pitch of each drum for each passage of a timpani part, and the places
//! no tuning covers.

use std::collections::BTreeSet;

use sonara_core::midi::MidiNote;

use super::{CheckError, Context, Shortfall, Uncovered, is_valid, note_order, onsets};
use crate::orchestration::{FactKind, InstrumentType, SoundingPitch, Span};
use crate::performance::ScoreContext;

/// One passage of a timpani part with the drums' tuning that plays it.
#[derive(Clone, Debug, PartialEq)]
pub struct TuningPassage {
    /// Where the passage starts, in content beats: its first note's start.
    pub start: f64,
    /// Where it ends: its last note's end.
    pub end: f64,
    /// The pitch of each drum during the passage, by drum; `None` for a drum not yet tuned.
    pub drums: Vec<Option<SoundingPitch>>,
    /// The drums retuned since the passage before, by number.
    pub retuned: Vec<usize>,
}

/// The timpani's tuning for a part.
#[derive(Clone, Debug, PartialEq)]
pub struct TimpaniTunings {
    /// The passages in order, each with its tuning.
    pub passages: Vec<TuningPassage>,
    /// The notes struck together on more pitches than there are drums ([`Shortfall::Pitches`])
    /// and the retunings the music leaves too little time for ([`Shortfall::Time`]), in order.
    pub uncovered: Vec<Uncovered>,
}

/// Derives the tuning of `drums` drums for each passage of the timpani notes `notes`, whose beats
/// are content beats of the clip that `score` places.
///
/// - **Passages.** From the first note, a passage takes in each next group of notes struck
///   together for as long as the passage's pitches fit on the drums; the next group starts a new
///   passage, so the drums are retuned as seldom as passages of this kind allow. A group of more
///   pitches than drums is reported and ends the passage before it.
/// - **Drums.** A drum keeps its pitch while a passage still needs it. A new pitch takes a drum
///   not yet tuned, then the drum whose pitch, no longer needed, lies nearest. The drums are
///   numbered from the one first tuned lowest; no drum's own compass is modelled.
/// - **Time.** A drum is retuned between the end of its last note at the old pitch and the first
///   note at the new one. A whole tone takes the time the timpani's record states, and each
///   semitone half of it (Forsyth: proportionately longer for a bigger change); when the music
///   leaves less, the retuning is reported.
///
/// # Errors
/// [`CheckError::NoDrums`] when `drums` is 0; [`CheckError::InvalidNote`] for the first invalid
/// note.
pub fn timpani_tunings(
    notes: &[MidiNote],
    drums: u8,
    score: &ScoreContext,
) -> Result<TimpaniTunings, CheckError> {
    if drums == 0 {
        return Err(CheckError::NoDrums);
    }
    if let Some(note) = notes.iter().find(|note| !is_valid(note)) {
        return Err(CheckError::InvalidNote(note.id.clone()));
    }
    let context = Context {
        score,
        parts: Vec::new(),
    };
    let capacity = usize::from(drums);
    let mut sorted: Vec<&MidiNote> = notes.iter().collect();
    sorted.sort_by(|a, b| note_order(a, b));

    let mut groups: Vec<Vec<Vec<&MidiNote>>> = Vec::new();
    let mut uncovered = Vec::new();
    let mut current: Vec<Vec<&MidiNote>> = Vec::new();
    let mut pitches: BTreeSet<u8> = BTreeSet::new();
    for group in onsets(&sorted) {
        let own: BTreeSet<u8> = group.iter().map(|note| note.pitch).collect();
        if own.len() > capacity {
            groups.push(std::mem::take(&mut current));
            pitches.clear();
            uncovered.push(Uncovered::of(&group, Shortfall::Pitches));
            continue;
        }
        if pitches.union(&own).count() > capacity {
            groups.push(std::mem::take(&mut current));
            pitches.clear();
        }
        pitches.extend(own);
        current.push(group);
    }
    groups.push(current);
    groups.retain(|passage| !passage.is_empty());

    let whole_tone = retune_time();
    // Each drum's pitch and the end of its last note at that pitch.
    let mut state: Vec<Option<(u8, Option<f64>)>> = vec![None; capacity];
    let mut passages = Vec::with_capacity(groups.len());
    for passage in &groups {
        let needed: BTreeSet<u8> = passage.iter().flatten().map(|note| note.pitch).collect();
        let mut taken: Vec<bool> = state
            .iter()
            .map(|drum| drum.is_some_and(|(pitch, _)| needed.contains(&pitch)))
            .collect();
        let mut retuned = Vec::new();
        for &arriving in &needed {
            if state.iter().flatten().any(|&(pitch, _)| pitch == arriving) {
                continue;
            }
            let Some(drum) = choose_drum(&state, &taken, arriving) else {
                continue;
            };
            if let Some(slot) = taken.get_mut(drum) {
                *slot = true;
            }
            let old = state.get(drum).copied().flatten();
            if let Some((from, last_end)) = old {
                retuned.push(drum);
                if let (Some(whole_tone), Some(last_end)) = (whole_tone, last_end) {
                    uncovered.extend(late_retune(
                        &context, passage, from, arriving, last_end, whole_tone,
                    ));
                }
            }
            if let Some(slot) = state.get_mut(drum) {
                *slot = Some((arriving, None));
            }
        }
        for note in passage.iter().flatten() {
            if let Some((_, last_end)) = state
                .iter_mut()
                .flatten()
                .find(|(pitch, _)| *pitch == note.pitch)
            {
                *last_end = Some(last_end.map_or(note.end(), |end| end.max(note.end())));
            }
        }
        let start = passage
            .first()
            .and_then(|group| group.first())
            .map_or(0.0, |note| note.start);
        let end = passage
            .iter()
            .flatten()
            .map(|note| note.end())
            .fold(start, f64::max);
        passages.push(TuningPassage {
            start,
            end,
            drums: state
                .iter()
                .map(|drum| drum.and_then(|(pitch, _)| SoundingPitch::new(i64::from(pitch))))
                .collect(),
            retuned,
        });
    }
    uncovered.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.end.total_cmp(&b.end)));
    Ok(TimpaniTunings {
        passages,
        uncovered,
    })
}

/// The drum for the new pitch `arriving`: the first drum not yet tuned, or else the free drum
/// whose pitch lies nearest, the lower number on a tie. A drum is free when no other pitch of the
/// passage is on it (`taken`).
fn choose_drum(state: &[Option<(u8, Option<f64>)>], taken: &[bool], arriving: u8) -> Option<usize> {
    let free = |drum: &usize| !taken.get(*drum).copied().unwrap_or(true);
    let untuned = (0..state.len())
        .filter(free)
        .find(|&drum| state.get(drum).is_some_and(Option::is_none));
    untuned.or_else(|| {
        (0..state.len())
            .filter(free)
            .filter_map(|drum| {
                let (pitch, _) = state.get(drum).copied().flatten()?;
                Some((drum, (i64::from(pitch) - i64::from(arriving)).abs()))
            })
            .min_by_key(|&(drum, distance)| (distance, drum))
            .map(|(drum, _)| drum)
    })
}

/// The retuning of a drum from `from` to `to` before `passage`, when the music leaves less time
/// than it takes after the drum's last note ends at `last_end`.
fn late_retune(
    context: &Context<'_>,
    passage: &[Vec<&MidiNote>],
    from: u8,
    to: u8,
    last_end: f64,
    whole_tone: Span,
) -> Option<Uncovered> {
    let first: Vec<&MidiNote> = passage
        .iter()
        .find(|group| group.iter().any(|note| note.pitch == to))?
        .iter()
        .copied()
        .filter(|note| note.pitch == to)
        .collect();
    let start = first.first()?.start;
    let semitones = u32::from(from.abs_diff(to));
    let needed = whole_tone.millis().saturating_mul(semitones) / 2;
    let free = context.millis(last_end, start);
    (free < f64::from(needed)).then(|| Uncovered {
        start: last_end,
        end: start,
        notes: first.iter().map(|note| note.id.clone()).collect(),
        shortfall: Shortfall::Time {
            needed_ms: needed,
            free_ms: free,
        },
    })
}

/// The time to retune a drum by a whole tone, from the timpani's record; `None` if it states none.
fn retune_time() -> Option<Span> {
    InstrumentType::Timpani
        .get()
        .facts()
        .iter()
        .find_map(|fact| match fact.kind {
            FactKind::Retune { whole_tone, .. } => Some(whole_tone),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::choose_drum;

    #[test]
    fn a_new_pitch_takes_an_untuned_drum_then_the_nearest_free_one() {
        // TST-10: drum 0 on D3 (50) is still needed; drum 1 on A2 (45) and drum 2 on E3 (52) are
        // free, and G3 (55) lies nearer E3.
        let state = [Some((50, None)), Some((45, None)), Some((52, None))];
        assert_eq!(choose_drum(&state, &[true, false, false], 55), Some(2));
        // A2 lies nearer C3 (48) than E3 does.
        assert_eq!(choose_drum(&state, &[true, false, false], 48), Some(1));
        // A tie goes to the lower drum: C#3 (49) lies three semitones from B♭2 (46) and E3.
        let tie = [Some((50, None)), Some((46, None)), Some((52, None))];
        assert_eq!(choose_drum(&tie, &[true, false, false], 49), Some(1));
        let with_untuned = [Some((50, None)), None, Some((52, None))];
        assert_eq!(
            choose_drum(&with_untuned, &[true, false, false], 55),
            Some(1)
        );
        assert_eq!(choose_drum(&state, &[true, true, true], 55), None);
    }
}
