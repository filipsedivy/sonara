//! Reading a sketch: the melody on top, the chords under it and the key.

use std::collections::BTreeSet;

use sonara_core::midi::MidiNote;

use super::{Analysis, ChordFit, KeyFit, Reduction, ReductionError, Skyline};
use crate::generators::ChordSlot;
use crate::performance::check::{is_valid, note_order};
use crate::theory::{CHORD_QUALITIES, Chord, Key, Scale};

/// Semitones in an octave.
const OCTAVE: i64 = 12;

/// Notes whose starts lie within this many beats (a sixty-fourth note in a quarter-note beat) of
/// an onset's first start sound at that onset, so a chord played by hand with a slight spread is
/// still one chord.
pub(super) const ONSET_BEATS: f64 = 0.0625;

/// A melody note that sounds on for at least this many beats (a sixteenth note in a quarter-note
/// beat) after the next onset holds over it: a lower note starting there accompanies it, and a
/// note at least as high starting there cuts it short. A shorter overlap is a legato join and is
/// kept.
const HELD_BEATS: f64 = 0.25;

/// The fewest pitch classes of the accompaniment that make a chord without the melody's note.
const FULL_CHORD: usize = 3;

/// The pitch-class names chord symbols are spelled with, flats for the black keys, as
/// [`Key`]'s display spells a tonic.
const NAMES: [&str; 12] = [
    "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
];

/// Reads `sketch` (see [`analyse`](super::analyse)).
pub(super) fn analyse(sketch: &[MidiNote]) -> Result<Analysis, ReductionError> {
    let Skyline {
        melody,
        accompaniment,
    } = skyline(sketch)?;
    // The chords are read once without a key, to find the key, and then again in it, so that a
    // tie between chords that fit equally goes to the one whose tones lie in the key.
    let provisional = Reduction {
        harmony: harmony(&accompaniment, &melody, None)
            .into_iter()
            .map(|(slot, _)| slot)
            .collect(),
        melody,
        bass: None,
    };
    let key = find_key(&provisional).ok_or(ReductionError::EmptySketch)?;
    let (harmony, chords): (Vec<ChordSlot>, Vec<ChordFit>) =
        harmony(&accompaniment, &provisional.melody, Some(key.key))
            .into_iter()
            .unzip();
    Ok(Analysis {
        reduction: Reduction {
            melody: provisional.melody,
            harmony,
            bass: None,
        },
        chords,
        key,
    })
}

/// Splits `notes` into the melody on top and the accompaniment under it (see
/// [`skyline`](super::skyline)).
pub(super) fn skyline(notes: &[MidiNote]) -> Result<Skyline, ReductionError> {
    if let Some(note) = notes.iter().find(|note| !is_valid(note)) {
        return Err(ReductionError::InvalidNote(note.id.clone()));
    }
    let mut sorted = notes.to_vec();
    sorted.sort_by(note_order);
    let mut melody: Vec<MidiNote> = Vec::new();
    let mut accompaniment = Vec::new();
    for group in onsets(&sorted) {
        let Some(onset) = group.first().map(|note| note.start) else {
            continue;
        };
        // The highest note of the onset; the first of equal ones in note order.
        let top = group
            .iter()
            .enumerate()
            .max_by(|(a_index, a), (b_index, b)| a.pitch.cmp(&b.pitch).then(b_index.cmp(a_index)))
            .map(|(index, _)| index);
        let Some(top) = top else {
            continue;
        };
        let held = |previous: &MidiNote| previous.end() - onset >= HELD_BEATS;
        let under_held = melody.last().is_some_and(|previous| {
            group
                .get(top)
                .is_some_and(|note| note.pitch < previous.pitch && held(previous))
        });
        for (index, note) in group.iter().enumerate() {
            if index == top && !under_held {
                if let Some(previous) = melody.last_mut().filter(|previous| held(previous)) {
                    previous.length = onset - previous.start;
                }
                melody.push(note.clone());
            } else {
                accompaniment.push(note.clone());
            }
        }
    }
    Ok(Skyline {
        melody,
        accompaniment,
    })
}

/// The notes of `sorted` (in note order) grouped by onset: a note joins the group whose first
/// start lies less than [`ONSET_BEATS`] before its own.
fn onsets(sorted: &[MidiNote]) -> Vec<&[MidiNote]> {
    let mut groups = Vec::new();
    let mut first = 0;
    for (index, note) in sorted.iter().enumerate() {
        let opens = sorted
            .get(first)
            .is_none_or(|start| note.start - start.start >= ONSET_BEATS);
        if opens && index > first {
            if let Some(group) = sorted.get(first..index) {
                groups.push(group);
            }
            first = index;
        }
    }
    if let Some(group) = sorted.get(first..).filter(|group| !group.is_empty()) {
        groups.push(group);
    }
    groups
}

/// The chords of the accompaniment, each with how well it fits, in time order.
///
/// The accompaniment is read at each beat where one of its notes starts or ends (times closer
/// than [`ONSET_BEATS`] count as one): the pitch classes sounding there, and the melody's
/// sounding note where it helps ([`read_chord`]), are matched against the chord table
/// ([`match_chord`]), the accompaniment's lowest note giving the bass. A reading joins the chord
/// before it when it continues it: only tones of that chord over the same bass. A time where the
/// accompaniment is silent has no chord.
fn harmony(
    accompaniment: &[MidiNote],
    melody: &[MidiNote],
    key: Option<Key>,
) -> Vec<(ChordSlot, ChordFit)> {
    // The starts, then the ends that no start lies near, so a chord read at a change starts
    // where its notes do.
    let mut times: Vec<f64> = accompaniment.iter().map(|note| note.start).collect();
    times.sort_by(f64::total_cmp);
    times.dedup_by(|later, earlier| *later - *earlier < ONSET_BEATS);
    let ends: Vec<f64> = accompaniment
        .iter()
        .map(MidiNote::end)
        .filter(|end| times.iter().all(|start| (start - end).abs() >= ONSET_BEATS))
        .collect();
    times.extend(ends);
    times.sort_by(f64::total_cmp);
    times.dedup_by(|later, earlier| *later - *earlier < ONSET_BEATS);
    let sounding = |notes: &[MidiNote], beat: f64| -> Vec<i64> {
        notes
            .iter()
            .filter(|note| note.start < beat + ONSET_BEATS && beat < note.end())
            .map(|note| i64::from(note.pitch))
            .collect()
    };
    let mut slots: Vec<(ChordSlot, ChordFit)> = Vec::new();
    for (&start, &end) in times.iter().zip(times.iter().skip(1)) {
        let pitches = sounding(accompaniment, start);
        let Some(&lowest) = pitches.iter().min() else {
            continue;
        };
        let classes: BTreeSet<i64> = pitches.iter().map(|p| p.rem_euclid(OCTAVE)).collect();
        let bass = lowest.rem_euclid(OCTAVE);
        if let Some((last, _)) = slots.last_mut()
            && (last.end() - start).abs() < ONSET_BEATS
            && continues(&last.chord, &classes, bass)
        {
            last.length = end - last.start;
            continue;
        }
        let mut with_melody = classes.clone();
        with_melody.extend(sounding(melody, start).iter().map(|p| p.rem_euclid(OCTAVE)));
        if let Some((chord, fit)) = read_chord(&classes, &with_melody, bass, key) {
            slots.push((
                ChordSlot {
                    chord,
                    start,
                    length: end - start,
                },
                fit,
            ));
        }
    }
    slots
}

/// The chord of the accompaniment's pitch classes `classes`, or of `with_melody`, the same with
/// the melody's sounding note: the accompaniment alone when it holds a chord of three or more
/// tones exactly, so a melody's passing note never colours the chord; with the melody when the
/// accompaniment holds fewer than three pitch classes, or when only the melody's note makes the
/// chord exact (a seventh chord whose fifth the melody plays).
fn read_chord(
    classes: &BTreeSet<i64>,
    with_melody: &BTreeSet<i64>,
    bass: i64,
    key: Option<Key>,
) -> Option<(Chord, ChordFit)> {
    let alone = match_chord(classes, bass, key);
    let full = classes.len() >= FULL_CHORD;
    if full && alone.as_ref().is_some_and(|(_, fit)| fit.exact) || with_melody == classes {
        return alone;
    }
    let joined = match_chord(with_melody, bass, key);
    if !full || joined.as_ref().is_some_and(|(_, fit)| fit.exact) {
        joined
    } else {
        alone
    }
}

/// Whether a reading of `classes` over `bass` continues `chord`: the reading's tones are the
/// chord's and its bass is the chord's bass.
fn continues(chord: &Chord, classes: &BTreeSet<i64>, bass: i64) -> bool {
    let tones: BTreeSet<i64> = chord.pitch_classes().into_iter().collect();
    chord.bass.unwrap_or(chord.root).rem_euclid(OCTAVE) == bass && classes.is_subset(&tones)
}

/// How well a chord of the table fits a set of pitch classes; each field compares so that lower
/// is better, in field order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Fit {
    /// Whether the chord's tones differ from the pitch classes.
    inexact: bool,
    /// How many pitch classes the chord does not hold plus how many of its tones are not among
    /// them: how far its tones lie from the pitch classes.
    distance: usize,
    /// The chord's tones that are not among the pitch classes: a chord that adds no tone the
    /// notes do not have is the better reading of two equally distant ones.
    missing: usize,
    /// Whether the chord's root is not the bass: a chord in root position is the plainer reading.
    inverted: bool,
    /// The chord's tones outside the key, its minor's raised seventh counted as inside.
    outside: usize,
}

/// The chord of the table that best fits the pitch classes `classes` over the bass pitch class
/// `bass`, in `key` when one is known, with how well it fits; `None` for no pitch class.
///
/// Every root with every quality of the table is a candidate (each quality spelled by its first
/// name, so `C` rather than `Cmaj`). The best is the one whose tones are exactly the pitch
/// classes, then the one whose tones differ from them in the fewest pitch classes, then the one
/// that adds the fewest tones they do not have, with its root on the bass, and with the fewest
/// tones outside the key; the chords equal to it in all of these fit equally
/// ([`ChordFit::others`]). Among those the earlier quality of the table, then the
/// root nearest above the bass, is taken, so the choice is always the same. The chord has a
/// slash bass when the bass is not its root.
pub(super) fn match_chord(
    classes: &BTreeSet<i64>,
    bass: i64,
    key: Option<Key>,
) -> Option<(Chord, ChordFit)> {
    if classes.is_empty() {
        return None;
    }
    let bass = bass.rem_euclid(OCTAVE);
    let mut candidates: Vec<(Fit, usize, i64)> = Vec::new();
    for (quality, &(_, intervals)) in CHORD_QUALITIES.iter().enumerate() {
        let first = CHORD_QUALITIES
            .iter()
            .position(|&(_, other)| other == intervals);
        if first != Some(quality) {
            continue;
        }
        for root in 0..OCTAVE {
            let tones: BTreeSet<i64> = intervals
                .iter()
                .map(|interval| (root + interval).rem_euclid(OCTAVE))
                .collect();
            let fit = Fit {
                inexact: tones != *classes,
                distance: classes.symmetric_difference(&tones).count(),
                missing: tones.difference(classes).count(),
                inverted: root != bass,
                outside: key.map_or(0, |key| {
                    tones.iter().filter(|&&tone| !in_key(key, tone)).count()
                }),
            };
            candidates.push((fit, quality, (root - bass).rem_euclid(OCTAVE)));
        }
    }
    candidates.sort();
    let mut equal = candidates.iter();
    let &(best, quality, distance) = equal.next()?;
    let best_chord = chord(quality, (bass + distance).rem_euclid(OCTAVE), bass)?;
    let others = equal
        .take_while(|(fit, _, _)| *fit == best)
        .filter_map(|&(_, quality, distance)| {
            chord(quality, (bass + distance).rem_euclid(OCTAVE), bass)
        })
        .collect();
    Some((
        best_chord,
        ChordFit {
            exact: !best.inexact,
            others,
        },
    ))
}

/// The chord of the table's quality `quality` on `root` over `bass`, spelled as a symbol that
/// [`parse_chord`](crate::theory::parse_chord) reads back.
fn chord(quality: usize, root: i64, bass: i64) -> Option<Chord> {
    let &(name, intervals) = CHORD_QUALITIES.get(quality)?;
    let spell = |pitch_class: i64| {
        usize::try_from(pitch_class.rem_euclid(OCTAVE))
            .ok()
            .and_then(|index| NAMES.get(index))
            .copied()
    };
    let slash = if bass == root {
        String::new()
    } else {
        format!("/{}", spell(bass)?)
    };
    Some(Chord {
        root,
        intervals: intervals.to_vec(),
        bass: (bass != root).then_some(bass),
        symbol: format!("{}{name}{slash}", spell(root)?),
    })
}

/// Whether the pitch class `tone` lies in `key`, a minor key's raised seventh, its leading tone,
/// counted as inside.
fn in_key(key: Key, tone: i64) -> bool {
    key.contains(tone) || (key.scale == Scale::Minor && tone == (key.root - 1).rem_euclid(OCTAVE))
}

/// The key of `reduction` (see [`find_key`](super::find_key)).
pub(super) fn find_key(reduction: &Reduction) -> Option<KeyFit> {
    let mut weights = [0.0_f64; 12];
    let mut add = |pitch_class: i64, length: f64| {
        if let Some(weight) = usize::try_from(pitch_class.rem_euclid(OCTAVE))
            .ok()
            .and_then(|index| weights.get_mut(index))
        {
            *weight += length;
        }
    };
    let lines = reduction
        .melody
        .iter()
        .chain(reduction.bass.iter().flatten());
    for note in lines {
        add(i64::from(note.pitch), note.length);
    }
    for slot in &reduction.harmony {
        let mut tones: BTreeSet<i64> = slot.chord.pitch_classes().into_iter().collect();
        tones.extend(slot.chord.bass.map(|bass| bass.rem_euclid(OCTAVE)));
        for pitch_class in tones {
            add(pitch_class, slot.length);
        }
    }
    let total: f64 = weights.iter().sum();
    if total <= 0.0 {
        return None;
    }
    // The tonic evidence: the root of the last chord and of the first, or the last and first
    // melody notes when there are no chords.
    let roots = |slot: Option<&ChordSlot>, note: Option<&MidiNote>| {
        slot.map(|slot| slot.chord.root.rem_euclid(OCTAVE))
            .or_else(|| note.map(|note| i64::from(note.pitch).rem_euclid(OCTAVE)))
    };
    let mut sorted_slots: Vec<&ChordSlot> = reduction.harmony.iter().collect();
    sorted_slots.sort_by(|a, b| a.start.total_cmp(&b.start));
    let mut sorted_melody: Vec<&MidiNote> = reduction.melody.iter().collect();
    sorted_melody.sort_by(|a, b| note_order(a, b));
    let last = roots(sorted_slots.last().copied(), sorted_melody.last().copied());
    let first = roots(
        sorted_slots.first().copied(),
        sorted_melody.first().copied(),
    );
    let keys: Vec<(Key, f64, [bool; 2])> = (0..OCTAVE)
        .flat_map(|root| [Scale::Major, Scale::Minor].map(|scale| Key { root, scale }))
        .map(|key| {
            // Summed in pitch-class order, so two keys holding the same pitch classes weigh
            // exactly the same.
            let weight: f64 = (0..OCTAVE)
                .zip(weights)
                .filter(|&(pitch_class, _)| key.contains(pitch_class))
                .map(|(_, weight)| weight)
                .sum();
            let tonic = |evidence: Option<i64>| evidence == Some(key.root);
            (key, weight, [tonic(last), tonic(first)])
        })
        .collect();
    let most = keys
        .iter()
        .map(|&(_, weight, _)| weight)
        .fold(0.0, f64::max);
    let fitting: Vec<&(Key, f64, [bool; 2])> = keys
        .iter()
        .filter(|(_, weight, _)| most - weight <= total * 1e-9)
        .collect();
    let evidence = fitting.iter().map(|(_, _, evidence)| *evidence).max()?;
    let mut equal = fitting
        .iter()
        .filter(|(_, _, found)| *found == evidence)
        .map(|(key, _, _)| *key);
    let key = equal.next()?;
    Some(KeyFit {
        key,
        others: equal.collect(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{in_key, match_chord};
    use crate::theory::{Key, Scale};

    fn classes(pitch_classes: &[i64]) -> BTreeSet<i64> {
        pitch_classes.iter().copied().collect()
    }

    fn symbol(pitch_classes: &[i64], bass: i64, key: Option<Key>) -> (String, Vec<String>, bool) {
        let (chord, fit) = match_chord(&classes(pitch_classes), bass, key).expect("a chord");
        let others = fit.others.into_iter().map(|chord| chord.symbol).collect();
        (chord.symbol, others, fit.exact)
    }

    const D_MINOR: Key = Key {
        root: 2,
        scale: Scale::Minor,
    };

    #[test]
    fn a_tie_goes_to_the_chord_whose_tones_lie_in_the_key() {
        // D and F over D are both D minor and D diminished without their fifth; without a key
        // they fit equally, in D minor the A of the minor triad lies in the key and the A flat
        // does not. B flat major also holds both but has its root off the bass.
        assert_eq!(
            symbol(&[2, 5], 2, None),
            ("Dm".to_owned(), vec!["Ddim".to_owned()], false)
        );
        assert_eq!(
            symbol(&[2, 5], 2, Some(D_MINOR)),
            ("Dm".to_owned(), Vec::new(), false)
        );
    }

    #[test]
    fn exact_readings_come_before_fuller_ones() {
        // C, E, G and B flat are exactly C7; C9 would hold them too but misses its ninth.
        assert_eq!(
            symbol(&[0, 4, 7, 10], 0, None),
            ("C7".to_owned(), Vec::new(), true)
        );
        // C, E, G and F match no chord exactly. The major triad and the suspended fourth leave
        // one of them out; F major ninth over C holds them all but adds an A the notes lack,
        // so the triads are the nearer readings, the major one first.
        assert_eq!(
            symbol(&[0, 4, 5, 7], 0, None),
            ("C".to_owned(), vec!["Csus4".to_owned()], false)
        );
        assert_eq!(match_chord(&BTreeSet::new(), 0, None), None);
    }

    #[test]
    fn a_minor_keys_leading_tone_counts_as_inside() {
        assert!(in_key(D_MINOR, 1));
        assert!(in_key(D_MINOR, 10));
        assert!(!in_key(D_MINOR, 11));
        let c_major = Key {
            root: 0,
            scale: Scale::Major,
        };
        assert!(!in_key(c_major, 10));
    }
}
