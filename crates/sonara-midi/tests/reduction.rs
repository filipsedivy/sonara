//! Contract tests of reading a sketch and orchestrating a reduction through the public API.

use std::collections::BTreeSet;

use proptest::prelude::*;
use sonara_core::ids::SequentialIds;
use sonara_core::midi::MidiNote;
use sonara_core::time::{TempoMap, TimeSignatureMap};
use sonara_midi::generators::ChordSlot;
use sonara_midi::orchestration::rules::RuleId;
use sonara_midi::orchestration::{Family, InstrumentType, PitchModel, ensemble};
use sonara_midi::performance::reduction::{
    Analysis, Assignment, Options, Orchestrated, Reduction, ReductionError, Remark, RestReason,
    analyse, find_key, orchestrate, skyline,
};
use sonara_midi::performance::voicing::{Carrier, Density, Remark as VoicingRemark, VoicingError};
use sonara_midi::performance::{PartId, PartLine, Role, ScoreContext, Seed, perform};
use sonara_midi::theory::{Key, Scale, parse_chord};

fn note(id: &str, pitch: i64, start: f64, length: f64) -> MidiNote {
    MidiNote::new(id, pitch, start, length, 90, 0).expect("a valid note")
}

fn part(slug: &str) -> PartId {
    PartId::new(slug).expect("a valid part id")
}

fn score() -> ScoreContext {
    ScoreContext::new(
        TempoMap::constant(72.0).expect("a valid tempo"),
        TimeSignatureMap::default(),
        0.0,
    )
    .expect("a valid origin")
}

fn ids() -> SequentialIds {
    SequentialIds::new(0)
}

/// The theme's chords, one per bar of 4/4.
const THEME: [&str; 8] = ["Dm", "Bb", "Gm", "A", "Dm", "Gm", "A7", "Dm"];

/// The theme's melody, bar by bar: (pitch, beats).
const THEME_MELODY: [&[(i64, f64)]; 8] = [
    &[(69, 1.0), (74, 1.0), (77, 1.0), (76, 1.0)],
    &[(74, 2.0), (77, 1.0), (74, 1.0)],
    &[(79, 1.0), (77, 1.0), (74, 2.0)],
    &[(76, 1.0), (73, 1.0), (69, 2.0)],
    &[(77, 1.0), (81, 1.0), (77, 1.0), (74, 1.0)],
    &[(79, 2.0), (74, 1.0), (70, 1.0)],
    &[(81, 2.0), (79, 1.0), (76, 1.0)],
    &[(74, 4.0)],
];

/// A piano sketch of the theme: a melody between A4 and A5 (MIDI 69-81) in the right hand over a
/// close triad in root position held through each bar in the left.
fn theme_sketch() -> Vec<MidiNote> {
    let chords: [&[i64]; 8] = [
        &[50, 53, 57],
        &[46, 50, 53],
        &[43, 46, 50],
        &[45, 49, 52],
        &[50, 53, 57],
        &[43, 46, 50],
        &[45, 49, 52, 55],
        &[50, 53, 57],
    ];
    let mut notes = Vec::new();
    for ((bar, line), chord) in (0_u32..).zip(THEME_MELODY).zip(chords) {
        let bar_start = 4.0 * f64::from(bar);
        let mut beat = bar_start;
        for (index, &(pitch, length)) in line.iter().enumerate() {
            notes.push(note(&format!("m{bar}_{index}"), pitch, beat, length));
            beat += length;
        }
        for (index, &pitch) in chord.iter().enumerate() {
            notes.push(note(&format!("c{bar}_{index}"), pitch, bar_start, 4.0));
        }
    }
    notes
}

/// The (start, pitch) of every note of the theme's melody.
fn theme_melody() -> Vec<(f64, i64)> {
    let mut melody = Vec::new();
    for (bar, line) in (0_u32..).zip(THEME_MELODY) {
        let mut beat = 4.0 * f64::from(bar);
        for &(pitch, length) in line {
            melody.push((beat, pitch));
            beat += length;
        }
    }
    melody
}

/// A four-bar chorale in C major in half notes, soprano, alto, tenor and bass, the chords C, Am,
/// F, G, C, F, G7, C; the lower voices of the first chord have no third, which the soprano gives.
const CHORALE: [[i64; 4]; 8] = [
    [76, 72, 67, 48],
    [76, 72, 64, 45],
    [77, 72, 69, 41],
    [74, 71, 67, 43],
    [76, 72, 67, 48],
    [77, 72, 69, 53],
    [74, 71, 65, 43],
    [72, 67, 64, 48],
];

fn chorale_sketch() -> Vec<MidiNote> {
    let mut notes = Vec::new();
    for (index, chord) in (0_u32..).zip(CHORALE) {
        for (voice, &pitch) in chord.iter().enumerate() {
            let start = 2.0 * f64::from(index);
            notes.push(note(&format!("v{voice}_{index}"), pitch, start, 2.0));
        }
    }
    notes
}

/// The parts of `ensemble_id` in score order, those of `families` only when any are given.
fn parts_of(ensemble_id: &str, families: &[Family]) -> Vec<Carrier> {
    ensemble(ensemble_id)
        .expect("the ensemble exists")
        .parts()
        .iter()
        .filter(|template| families.is_empty() || families.contains(&template.instrument.family()))
        .map(|template| Carrier::try_from(template).expect("a template slug is a part id"))
        .collect()
}

fn line<'a>(result: &'a Orchestrated, slug: &str) -> &'a PartLine {
    result
        .line(slug)
        .unwrap_or_else(|| panic!("{slug} is a part of the result"))
}

/// The (start, pitch) of every note of `slug`, in note order.
fn notes_of(result: &Orchestrated, slug: &str) -> Vec<(f64, i64)> {
    line(result, slug)
        .notes
        .iter()
        .map(|note| (note.start, i64::from(note.pitch)))
        .collect()
}

/// The pitches of `slug`, in note order.
fn pitches_of(result: &Orchestrated, slug: &str) -> Vec<i64> {
    notes_of(result, slug)
        .into_iter()
        .map(|(_, pitch)| pitch)
        .collect()
}

/// The pitches of `slug` sounding at `beat`, from the lowest.
fn chord_at(result: &Orchestrated, slug: &str, beat: f64) -> Vec<i64> {
    let mut pitches: Vec<i64> = line(result, slug)
        .notes
        .iter()
        .filter(|note| note.start <= beat && beat < note.end())
        .map(|note| i64::from(note.pitch))
        .collect();
    pitches.sort_unstable();
    pitches
}

/// Why `slug` rests, from the result's remarks.
fn rest_of(result: &Orchestrated, slug: &str) -> Option<RestReason> {
    result.remarks.iter().find_map(|remark| match remark {
        Remark::Rests { part, reason } if part.as_str() == slug => Some(reason.clone()),
        _ => None,
    })
}

fn assign(slugs: &[&str], role: Role) -> Vec<Assignment> {
    slugs
        .iter()
        .map(|slug| Assignment {
            part: part(slug),
            role,
        })
        .collect()
}

fn symbols(analysis: &Analysis) -> Vec<String> {
    analysis
        .reduction
        .harmony
        .iter()
        .map(|slot| slot.chord.symbol.clone())
        .collect()
}

#[test]
fn a_theme_reads_as_its_melody_over_its_chords_in_d_minor() {
    // TST-10: the sketch was written from these chords; the key is the one they and the melody
    // spell, D minor, whose relative F major holds the same pitch classes but does not end the
    // piece.
    let analysis = analyse(&theme_sketch()).expect("a valid sketch");
    let melody: Vec<(f64, i64)> = analysis
        .reduction
        .melody
        .iter()
        .map(|note| (note.start, i64::from(note.pitch)))
        .collect();
    assert_eq!(melody, theme_melody());
    assert_eq!(symbols(&analysis), THEME);
    for (slot, bar) in analysis.reduction.harmony.iter().zip(0_u32..) {
        assert_eq!((slot.start, slot.length), (4.0 * f64::from(bar), 4.0));
        // A symbol reads back as the chord it names.
        let parsed = parse_chord(&slot.chord.symbol).expect("a chord symbol");
        assert_eq!(
            (parsed.root, &parsed.intervals, parsed.bass),
            (slot.chord.root, &slot.chord.intervals, slot.chord.bass)
        );
    }
    assert!(
        analysis
            .chords
            .iter()
            .all(|fit| fit.exact && fit.others.is_empty())
    );
    assert_eq!(
        analysis.key.key,
        Key {
            root: 2,
            scale: Scale::Minor
        }
    );
    assert_eq!(analysis.key.others, Vec::<Key>::new());
    assert!(!analysis.is_ambiguous());
    assert_eq!(analysis.reduction.bass, None);
}

#[test]
fn a_chorale_reads_its_chords_with_the_soprano_only_where_it_helps() {
    // TST-10: the chords the chorale was written from. The first chord's alto, tenor and bass
    // sound only C and G, so the soprano's E makes it C major; the G7's lower voices sound G, B
    // and F, so the soprano's D makes it exact; every other chord is read from the lower voices
    // alone.
    let analysis = analyse(&chorale_sketch()).expect("a valid sketch");
    assert_eq!(
        symbols(&analysis),
        ["C", "Am", "F", "G", "C", "F", "G7", "C"]
    );
    assert!(
        analysis
            .chords
            .iter()
            .all(|fit| fit.exact && fit.others.is_empty())
    );
    let soprano: Vec<i64> = analysis
        .reduction
        .melody
        .iter()
        .map(|note| i64::from(note.pitch))
        .collect();
    assert_eq!(soprano, CHORALE.map(|chord| chord[0]));
    // C major and A minor hold the same pitch classes; the last chord's root decides.
    assert_eq!(
        analysis.key.key,
        Key {
            root: 0,
            scale: Scale::Major
        }
    );
    assert_eq!(analysis.key.others, Vec::<Key>::new());
}

#[test]
fn an_inversion_reads_as_a_slash_chord() {
    let sketch = [
        note("e", 52, 0.0, 4.0),
        note("g", 55, 0.0, 4.0),
        note("c", 60, 0.0, 4.0),
        note("top", 67, 0.0, 4.0),
    ];
    let analysis = analyse(&sketch).expect("a valid sketch");
    let chord = &analysis.reduction.harmony[0].chord;
    assert_eq!(chord.symbol, "C/E");
    assert_eq!((chord.root, chord.bass), (0, Some(4)));
}

#[test]
fn readings_that_fit_equally_are_reported() {
    // TST-03: E, A, C and G over E are both C6 and Am7, neither in root position; the earlier
    // quality of the table, the sixth chord, is taken and the other reported.
    let sketch = [
        note("e", 52, 0.0, 4.0),
        note("a", 57, 0.0, 4.0),
        note("c", 60, 0.0, 4.0),
        note("g", 67, 0.0, 4.0),
        note("top", 76, 0.0, 4.0),
    ];
    let analysis = analyse(&sketch).expect("a valid sketch");
    assert_eq!(symbols(&analysis), ["C6/E"]);
    let others: Vec<&str> = analysis.chords[0]
        .others
        .iter()
        .map(|chord| chord.symbol.as_str())
        .collect();
    assert_eq!(others, ["Am7/E"]);
    assert!(analysis.is_ambiguous());
    // An open fifth fits C major and C minor alike; the major comes first.
    let fifth = [
        note("c", 48, 0.0, 4.0),
        note("g", 55, 0.0, 4.0),
        note("top", 60, 0.0, 4.0),
    ];
    let analysis = analyse(&fifth).expect("a valid sketch");
    assert_eq!(symbols(&analysis), ["C5"]);
    assert_eq!(
        (analysis.key.key, analysis.key.others),
        (
            Key {
                root: 0,
                scale: Scale::Major
            },
            vec![Key {
                root: 0,
                scale: Scale::Minor
            }]
        )
    );
}

#[test]
fn the_skyline_keeps_one_line_on_top() {
    let sketch = [
        // A half note held over two notes below: the second has no melody note.
        note("held", 76, 0.0, 2.0),
        note("low_1", 60, 0.0, 1.0),
        note("low_2", 62, 1.0, 1.0),
        // A short legato overlap into a lower note is kept.
        note("legato_from", 74, 2.0, 1.05),
        note("legato_to", 72, 3.0, 1.0),
        // A note at least as high starting while the melody holds cuts it there.
        note("cut", 71, 4.0, 4.0),
        note("over", 79, 5.0, 1.0),
        // A chord spread by less than a sixteenth of a beat is one onset.
        note("spread_low", 55, 8.0, 1.0),
        note("spread_top", 77, 8.04, 1.0),
    ];
    let split = skyline(&sketch).expect("valid notes");
    let melody: Vec<(&str, f64)> = split
        .melody
        .iter()
        .map(|note| (note.id.as_str(), note.length))
        .collect();
    assert_eq!(
        melody,
        [
            ("held", 2.0),
            ("legato_from", 1.05),
            ("legato_to", 1.0),
            ("cut", 1.0),
            ("over", 1.0),
            ("spread_top", 1.0)
        ]
    );
    let accompaniment: BTreeSet<&str> = split
        .accompaniment
        .iter()
        .map(|note| note.id.as_str())
        .collect();
    assert_eq!(
        accompaniment,
        BTreeSet::from(["low_1", "low_2", "spread_low"])
    );
}

#[test]
fn the_key_of_a_bare_melody_is_where_it_ends() {
    // A C major scale up and back to C: every key holding all seven notes weighs the same, and
    // the last note names C major.
    let melody: Vec<MidiNote> = [60, 62, 64, 65, 67, 69, 71, 72, 67, 64, 60]
        .into_iter()
        .zip(0_u32..)
        .map(|(pitch, index)| note(&format!("m{index}"), pitch, f64::from(index), 1.0))
        .collect();
    let reduction = Reduction {
        melody,
        harmony: Vec::new(),
        bass: None,
    };
    let fit = find_key(&reduction).expect("a key");
    assert_eq!(
        (fit.key, fit.others),
        (
            Key {
                root: 0,
                scale: Scale::Major
            },
            Vec::new()
        )
    );
    let empty = Reduction {
        melody: Vec::new(),
        harmony: Vec::new(),
        bass: None,
    };
    assert_eq!(find_key(&empty), None);
}

#[test]
fn a_sketch_that_is_empty_or_invalid_is_rejected() {
    assert_eq!(analyse(&[]), Err(ReductionError::EmptySketch));
    let mut bad = note("bad", 60, 0.0, 1.0);
    bad.length = 0.0;
    assert_eq!(
        analyse(&[note("good", 64, 0.0, 1.0), bad.clone()]),
        Err(ReductionError::InvalidNote("bad".to_owned()))
    );
    assert_eq!(
        skyline(&[bad]),
        Err(ReductionError::InvalidNote("bad".to_owned()))
    );
}

/// A small orchestra: four horns, the timpani and the strings of the Romantic orchestra.
fn small_orchestra() -> Vec<Carrier> {
    let mut parts: Vec<Carrier> = parts_of("romantic_orchestra", &[Family::Brass])
        .into_iter()
        .filter(|carrier| carrier.instrument == InstrumentType::HornF)
        .collect();
    parts.push(Carrier::new(part("timpani"), InstrumentType::Timpani));
    parts.extend(parts_of("romantic_orchestra", &[Family::Strings]));
    parts
}

/// The timpani hit the downbeats of the theme's bars on D and A and roll the closing D, two drums
/// on the tonic and the dominant (Forsyth).
fn assert_timpani_on_d_and_a(result: &Orchestrated) {
    let timpani: Vec<(f64, f64, i64, Option<&str>)> = line(result, "timpani")
        .notes
        .iter()
        .map(|n| {
            (
                n.start,
                n.length,
                i64::from(n.pitch),
                n.articulation.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        timpani,
        [
            (0.0, 1.0, 38, None),
            (12.0, 1.0, 45, None),
            (16.0, 1.0, 38, None),
            (24.0, 1.0, 45, None),
            (28.0, 4.0, 38, Some("roll"))
        ]
    );
}

#[test]
fn strings_play_a_legato_melody_horns_and_timpani_underneath() {
    // TST-10: "strings play the melody, horns and timpani underneath". The first and second
    // violins take the melody in unison and the violas and cellos an octave below
    // (Rimsky-Korsakov, Ch. II, p. 43, for a melody in the middle range); the double basses have
    // nothing to double and rest. The horns voice the chords in four parts, from the top Horns 1,
    // 3, 2 and 4 (VOI-07), and carry the bass, since no other part does; the timpani are tuned to
    // the tonic and the dominant, D and A.
    let analysis = analyse(&theme_sketch()).expect("a valid sketch");
    let mut assignments = assign(
        &["violins_1", "violins_2", "violas", "cellos", "basses"],
        Role::Melody,
    );
    assignments.extend(assign(
        &["horn_1", "horn_2", "horn_3", "horn_4", "timpani"],
        Role::Harmony,
    ));
    let options = Options {
        assignments,
        ..Options::default()
    };
    let result = orchestrate(
        &analysis.reduction,
        &small_orchestra(),
        &score(),
        &options,
        &mut ids(),
    )
    .expect("the theme is orchestrated");
    assert_eq!(
        result.key,
        Key {
            root: 2,
            scale: Scale::Minor
        }
    );
    let melody = theme_melody();
    let octave_below: Vec<(f64, i64)> = melody.iter().map(|&(beat, p)| (beat, p - 12)).collect();
    assert_eq!(notes_of(&result, "violins_1"), melody);
    assert_eq!(notes_of(&result, "violins_2"), melody);
    assert_eq!(notes_of(&result, "violas"), octave_below);
    assert_eq!(notes_of(&result, "cellos"), octave_below);
    assert_eq!(line(&result, "basses").notes, Vec::<MidiNote>::new());
    // The horns, bar by bar from the top: Horns 1, 3, 2 and 4. D minor is F4, D4, A3, D3 and G
    // minor in bar 3 B♭4, G4, D4, G3; in bar 6 the melody falls to B♭4, so the chord moves
    // under it to B♭3, G3, D3, G2.
    let expected: [[i64; 4]; 8] = [
        [65, 62, 57, 50],
        [65, 62, 58, 46],
        [70, 67, 62, 55],
        [64, 61, 57, 45],
        [65, 62, 57, 50],
        [58, 55, 50, 43],
        [67, 64, 61, 57],
        [65, 62, 57, 50],
    ];
    for (bar, chord) in (0_u32..).zip(expected) {
        let beat = 4.0 * f64::from(bar);
        let horns =
            ["horn_1", "horn_3", "horn_2", "horn_4"].map(|slug| chord_at(&result, slug, beat));
        assert_eq!(horns, chord.map(|pitch| vec![pitch]), "bar {}", bar + 1);
        // Every horn note lies under the lowest melody note of its bar.
        let lowest = melody
            .iter()
            .filter(|(start, _)| (beat..beat + 4.0).contains(start))
            .map(|&(_, pitch)| pitch)
            .min()
            .expect("the melody sounds in every bar");
        assert!(chord.iter().all(|&pitch| pitch < lowest), "bar {}", bar + 1);
    }
    let roles: Vec<(&str, Role)> = result
        .lines
        .iter()
        .map(|line| (line.part.as_str(), line.role))
        .collect();
    assert_eq!(
        roles,
        [
            ("horn_1", Role::Harmony),
            ("horn_2", Role::Harmony),
            ("horn_3", Role::Harmony),
            ("horn_4", Role::Bass),
            ("timpani", Role::Bass),
            ("violins_1", Role::Melody),
            ("violins_2", Role::Melody),
            ("violas", Role::Melody),
            ("cellos", Role::Melody),
            ("basses", Role::Melody),
        ]
    );
    assert_timpani_on_d_and_a(&result);
    assert_eq!(
        result.remarks,
        [Remark::Rests {
            part: part("basses"),
            reason: RestReason::Unused
        }]
    );
}

#[test]
fn every_line_is_ready_to_perform() {
    // Each line performs as it comes back, its notes kept: the application only adds what the
    // part's sound can do.
    let analysis = analyse(&theme_sketch()).expect("a valid sketch");
    let result = orchestrate(
        &analysis.reduction,
        &small_orchestra(),
        &score(),
        &Options::default(),
        &mut ids(),
    )
    .expect("the theme is orchestrated");
    for line in &result.lines {
        let performed = perform(line, &score(), Seed::default()).expect("the line performs");
        let written: Vec<(String, u8)> =
            line.notes.iter().map(|n| (n.id.clone(), n.pitch)).collect();
        let mut played: Vec<(String, u8)> = performed
            .notes
            .iter()
            .map(|n| (n.id.clone(), n.pitch))
            .collect();
        played.sort();
        let mut expected = written;
        expected.sort();
        assert_eq!(played, expected, "{}", line.part);
    }
}

/// Whether `pitch` is a tone of `slot`'s chord or its bass.
fn in_chord(slot: &ChordSlot, pitch: i64) -> bool {
    let class = pitch.rem_euclid(12);
    slot.chord.pitch_classes().contains(&class) || slot.chord.bass == Some(class)
}

#[test]
fn a_chorale_reduction_scored_for_strings() {
    // TST-10: the chorale's soprano goes to the first violins. The second violins and the violas
    // divide in two (VOI-10) for three notes above the bass, the cellos play the bass and the
    // double basses double it an octave below. Checked by hand, chord by chord: every note is a
    // tone of its chord under the soprano, the lowest four voices widen downward (SPC-05), each
    // voice lies within an octave of the one below, and no section's halves lie a unison or an
    // octave apart (DBL-05).
    let analysis = analyse(&chorale_sketch()).expect("a valid sketch");
    let strings = parts_of("romantic_orchestra", &[Family::Strings]);
    let thin = Options {
        density: Density::Thin,
        ..Options::default()
    };
    let result = orchestrate(&analysis.reduction, &strings, &score(), &thin, &mut ids())
        .expect("the chorale is orchestrated");
    let soprano: Vec<(f64, i64)> = (0_u32..)
        .zip(CHORALE)
        .map(|(index, chord)| (2.0 * f64::from(index), chord[0]))
        .collect();
    assert_eq!(notes_of(&result, "violins_1"), soprano);
    // C, Am, F, G, C, F, G7, C.
    let violins_2: [[i64; 2]; 8] = [
        [67, 72],
        [69, 72],
        [69, 72],
        [67, 71],
        [64, 72],
        [65, 69],
        [65, 71],
        [64, 67],
    ];
    let violas: [[i64; 2]; 8] = [
        [60, 64],
        [57, 64],
        [60, 65],
        [55, 62],
        [55, 60],
        [53, 60],
        [55, 62],
        [55, 60],
    ];
    let cellos: [i64; 8] = [48, 45, 53, 43, 48, 41, 43, 48];
    for (index, slot) in analysis.reduction.harmony.iter().enumerate() {
        let beat = slot.start;
        assert_eq!(chord_at(&result, "violins_2", beat), violins_2[index]);
        assert_eq!(chord_at(&result, "violas", beat), violas[index]);
        assert_eq!(chord_at(&result, "cellos", beat), [cellos[index]]);
        assert_eq!(chord_at(&result, "basses", beat), [cellos[index] - 12]);
        // The invariants the hand check rests on, verified again.
        let soprano = CHORALE[index][0];
        for slug in ["violins_2", "violas", "cellos", "basses"] {
            for pitch in chord_at(&result, slug, beat) {
                assert!(in_chord(slot, pitch), "{slug} {pitch} at {beat}");
                assert!(pitch < soprano, "{slug} {pitch} at {beat}");
            }
        }
        let bass = slot.chord.bass.unwrap_or(slot.chord.root);
        assert_eq!(cellos[index].rem_euclid(12), bass);
    }
    let roles: Vec<Role> = result.lines.iter().map(|line| line.role).collect();
    assert_eq!(
        roles,
        [
            Role::Melody,
            Role::Harmony,
            Role::Harmony,
            Role::Bass,
            Role::Bass
        ]
    );
    assert_eq!(
        result.remarks,
        [
            Remark::Voicing(VoicingRemark::Divided {
                part: part("violas"),
                ways: 2
            }),
            Remark::Voicing(VoicingRemark::Divided {
                part: part("violins_2"),
                ways: 2
            }),
        ]
    );
    // Asked for more, the strings give the same parts: the second violins voice the chords, so no
    // string part is left to double the melody, and the result says it is thinner.
    let medium = orchestrate(
        &analysis.reduction,
        &strings,
        &score(),
        &Options::default(),
        &mut ids(),
    )
    .expect("the chorale is orchestrated");
    assert_eq!(medium.lines, result.lines);
    assert_eq!(
        medium.remarks.first(),
        Some(&Remark::Voicing(VoicingRemark::Thinned {
            asked: Density::Medium,
            written: Density::Thin
        }))
    );
}

/// The parts of `result` with notes, by the role of their line.
fn playing(result: &Orchestrated, role: Role) -> Vec<&str> {
    result
        .lines
        .iter()
        .filter(|line| line.role == role && !line.notes.is_empty())
        .map(|line| line.part.as_str())
        .collect()
}

/// Every harmony and bass note is a tone of the chord sounding at its start and lies under the
/// lowest melody note of that chord, unless the chord is reported as crossing.
fn assert_harmony_under_melody(result: &Orchestrated, analysis: &Analysis) {
    let crossing: Vec<f64> = result
        .remarks
        .iter()
        .filter_map(|remark| match remark {
            Remark::Voicing(VoicingRemark::Crosses { start }) => Some(*start),
            _ => None,
        })
        .collect();
    let harmony = &analysis.reduction.harmony;
    for line in &result.lines {
        if line.role == Role::Melody || line.instrument.instrument_type() == InstrumentType::Timpani
        {
            continue;
        }
        for note in &line.notes {
            let slot = harmony
                .iter()
                .find(|slot| slot.start <= note.start && note.start < slot.end())
                .expect("every chord note lies in a chord");
            assert!(
                in_chord(slot, i64::from(note.pitch)),
                "{} {note:?}",
                line.part
            );
            let lowest = analysis
                .reduction
                .melody
                .iter()
                .filter(|m| m.start < slot.end() && slot.start < m.end())
                .map(|m| m.pitch)
                .min();
            let under = lowest.is_none_or(|lowest| note.pitch < lowest);
            assert!(
                under || crossing.contains(&slot.start),
                "{} {note:?}",
                line.part
            );
        }
    }
}

#[test]
fn the_default_plan_adds_families_by_density() {
    // Without assignments, the function table gives the melody to the first violins and the
    // strings voice the chords first, the cellos on the bass with the double basses an octave
    // below; each density adds families in Rimsky-Korsakov's order, strings, woodwind, brass.
    let analysis = analyse(&theme_sketch()).expect("a valid sketch");
    let orchestra = parts_of("romantic_orchestra", &[]);
    let at = |density: Density| {
        let options = Options {
            density,
            ..Options::default()
        };
        orchestrate(
            &analysis.reduction,
            &orchestra,
            &score(),
            &options,
            &mut ids(),
        )
        .expect("the theme is orchestrated")
    };
    let thin = at(Density::Thin);
    assert_eq!(playing(&thin, Role::Melody), ["violins_1"]);
    assert_eq!(playing(&thin, Role::Harmony), ["violins_2", "violas"]);
    assert_eq!(playing(&thin, Role::Bass), ["cellos", "basses"]);
    assert_eq!(rest_of(&thin, "oboe_1"), Some(RestReason::Density));
    assert_eq!(rest_of(&thin, "timpani"), Some(RestReason::Density));
    // Medium: the flutes double the violins' melody in unison; the woodwinds that are left cannot
    // voice the chords under the melody, so the horns are the second family.
    let medium = at(Density::Medium);
    assert_eq!(
        playing(&medium, Role::Melody),
        ["flute_1", "flute_2", "flute_3", "violins_1"]
    );
    assert_eq!(
        playing(&medium, Role::Harmony),
        [
            "horn_1",
            "horn_2",
            "horn_3",
            "horn_4",
            "violins_2",
            "violas"
        ]
    );
    assert_eq!(playing(&medium, Role::Bass), ["cellos", "basses"]);
    for slug in ["piccolo", "oboe_1", "clarinet_2", "contrabassoon"] {
        assert_eq!(
            rest_of(&medium, slug),
            Some(RestReason::AboveMelody),
            "{slug}"
        );
    }
    for slug in ["trumpet_1", "trombone_3", "tuba"] {
        assert_eq!(rest_of(&medium, slug), Some(RestReason::Density), "{slug}");
    }
    for slug in ["bass_drum", "cymbals", "tam_tam", "harp"] {
        assert_eq!(rest_of(&medium, slug), Some(RestReason::NoHelper), "{slug}");
    }
    // Full: the three groups take the melody in unison, every family voices the chords and the
    // timpani join.
    let full = at(Density::Full);
    for slug in ["violins_1", "oboe_1", "clarinet_1", "trumpet_1"] {
        assert_eq!(line(&full, slug).role, Role::Melody, "{slug}");
        assert!(!line(&full, slug).notes.is_empty(), "{slug}");
    }
    for slug in ["bassoon_1", "horn_1", "trombone_1", "tuba"] {
        assert_eq!(line(&full, slug).role, Role::Harmony, "{slug}");
        assert!(!line(&full, slug).notes.is_empty(), "{slug}");
    }
    assert_eq!(line(&full, "timpani").role, Role::Bass);
    assert_eq!(notes_of(&full, "timpani").len(), 5);
    // Whatever the density, the harmony stays under the melody except where a remark says it
    // crosses, and every note is a tone of the chord sounding at its start.
    for result in [&thin, &medium, &full] {
        assert_harmony_under_melody(result, &analysis);
    }
}

/// The theme read as a reduction.
fn theme_reduction() -> Reduction {
    analyse(&theme_sketch()).expect("a valid sketch").reduction
}

#[test]
fn a_bass_the_request_assigns_has_parts_of_its_own() {
    // The cellos and the double basses play the chords' bass notes in octaves, the usual
    // construction of the bass (Rimsky-Korsakov, Ch. II, p. 43), each inside its comfortable
    // range; the free strings voice the chords without the bass.
    let reduction = theme_reduction();
    let strings = parts_of("romantic_orchestra", &[Family::Strings]);
    let mut assignments = assign(&["violins_1"], Role::Melody);
    assignments.extend(assign(&["cellos", "basses"], Role::Bass));
    let options = Options {
        assignments,
        density: Density::Thin,
        ..Options::default()
    };
    let result = orchestrate(&reduction, &strings, &score(), &options, &mut ids())
        .expect("the theme is orchestrated");
    let roots: Vec<i64> = THEME
        .iter()
        .map(|symbol| parse_chord(symbol).expect("a chord").root)
        .collect();
    let cellos = pitches_of(&result, "cellos");
    let classes: Vec<i64> = cellos.iter().map(|p| p.rem_euclid(12)).collect();
    assert_eq!(classes, roots);
    // The roots in the octave of C3, so the double basses play the octave of C2 below.
    assert_eq!(cellos, [50, 58, 55, 57, 50, 55, 57, 50]);
    assert_eq!(
        pitches_of(&result, "basses"),
        [38, 46, 43, 45, 38, 43, 45, 38]
    );
    assert_eq!(line(&result, "cellos").role, Role::Bass);
    assert_eq!(line(&result, "basses").role, Role::Bass);
    assert_eq!(playing(&result, Role::Harmony), ["violins_2", "violas"]);
    assert_harmony_under_melody(&result, &analyse(&theme_sketch()).expect("a sketch"));
    // Double basses alone on the bass break Rimsky-Korsakov's rule that they never play without
    // cellos or bassoons (DBL-08), which the result reports.
    let options = Options {
        assignments: assign(&["basses"], Role::Bass),
        ..Options::default()
    };
    let result = orchestrate(&reduction, &strings, &score(), &options, &mut ids())
        .expect("the theme is orchestrated");
    assert!(result.remarks.contains(&Remark::Breaks(RuleId::Dbl08)));
}

#[test]
fn a_bass_line_of_its_own_goes_to_the_bass_row() {
    // A reduction with a bass line of its own: without assignments, the first instrument of the
    // function table's bass row, the cello, plays it with the double basses an octave below.
    let mut reduction = theme_reduction();
    let walk = [50, 52, 53, 55];
    let line_notes: Vec<MidiNote> = (0_u32..16)
        .zip(walk.iter().cycle())
        .map(|(index, &pitch)| note(&format!("b{index}"), pitch, 2.0 * f64::from(index), 2.0))
        .collect();
    reduction.bass = Some(line_notes);
    let strings = parts_of("romantic_orchestra", &[Family::Strings]);
    let result = orchestrate(
        &reduction,
        &strings,
        &score(),
        &Options::default(),
        &mut ids(),
    )
    .expect("the theme is orchestrated");
    assert_eq!(pitches_of(&result, "cellos"), walk.repeat(4));
    let below: Vec<i64> = walk.iter().map(|pitch| pitch - 12).collect();
    assert_eq!(pitches_of(&result, "basses"), below.repeat(4));
}

#[test]
fn a_wind_quintet_whose_woodwinds_break_the_three_colour_rule_rests_its_chords() {
    // TST-03: the flute carries the melody; oboe, clarinet and bassoon are three colours of one
    // player each, which cannot voice a chord (VOI-01). They rest, and the result says why.
    let reduction = theme_reduction();
    let quintet = parts_of("wind_quintet", &[]);
    let result = orchestrate(
        &reduction,
        &quintet,
        &score(),
        &Options::default(),
        &mut ids(),
    )
    .expect("the theme is orchestrated");
    assert_eq!(playing(&result, Role::Melody), ["flute"]);
    for slug in ["oboe", "clarinet", "bassoon"] {
        assert_eq!(
            rest_of(&result, slug),
            Some(RestReason::Unvoiced(VoicingError::Breaks(RuleId::Voi01))),
            "{slug}"
        );
    }
}

#[test]
fn requests_that_do_not_fit_are_rejected() {
    // TST-03: one case per error.
    let reduction = theme_reduction();
    let strings = parts_of("romantic_orchestra", &[Family::Strings]);
    let run = |reduction: &Reduction, parts: &[Carrier], assignments: Vec<Assignment>| {
        let options = Options {
            assignments,
            ..Options::default()
        };
        orchestrate(reduction, parts, &score(), &options, &mut ids())
    };
    let empty = Reduction {
        melody: Vec::new(),
        harmony: Vec::new(),
        bass: None,
    };
    assert_eq!(
        run(&empty, &strings, Vec::new()),
        Err(ReductionError::EmptyReduction)
    );
    let mut twice = strings.clone();
    twice.push(strings[0].clone());
    assert_eq!(
        run(&reduction, &twice, Vec::new()),
        Err(ReductionError::DuplicatePart(part("violins_1")))
    );
    assert_eq!(
        run(&reduction, &strings, assign(&["flute_1"], Role::Melody)),
        Err(ReductionError::UnknownPart(part("flute_1")))
    );
    let mut double = assign(&["violas"], Role::Melody);
    double.extend(assign(&["violas"], Role::Bass));
    assert_eq!(
        run(&reduction, &strings, double),
        Err(ReductionError::DuplicateAssignment(part("violas")))
    );
    assert_eq!(
        run(&reduction, &strings, assign(&["violas"], Role::Rhythm)),
        Err(ReductionError::UnsupportedRole(Role::Rhythm))
    );
    // A melody no part plays inside its comfortable range is never moved to fit.
    let mut high = reduction.clone();
    for note in &mut high.melody {
        note.pitch = 127;
    }
    assert_eq!(
        run(&high, &strings, Vec::new()),
        Err(ReductionError::Voicing(VoicingError::NoCarrier))
    );
    let mut bad_note = reduction.clone();
    bad_note.melody[0].velocity = 0;
    let id = bad_note.melody[0].id.clone();
    assert_eq!(
        run(&bad_note, &strings, Vec::new()),
        Err(ReductionError::InvalidNote(id))
    );
    let mut bad_slot = reduction.clone();
    bad_slot.harmony[2].length = 0.0;
    assert_eq!(
        run(&bad_slot, &strings, Vec::new()),
        Err(ReductionError::Voicing(VoicingError::InvalidSlot {
            start: 8.0,
            length: 0.0
        }))
    );
}

#[test]
fn the_same_request_gives_the_same_lines_and_ids() {
    // TST-04: no hidden state; every note takes a fresh id, part by part in score order, and
    // drafts thrown away on the way take none.
    let reduction = theme_reduction();
    let orchestra = parts_of("romantic_orchestra", &[]);
    let options = Options {
        density: Density::Medium,
        ..Options::default()
    };
    let first = orchestrate(&reduction, &orchestra, &score(), &options, &mut ids())
        .expect("the theme is orchestrated");
    let second = orchestrate(&reduction, &orchestra, &score(), &options, &mut ids())
        .expect("the theme is orchestrated");
    assert_eq!(first, second);
    let all: Vec<&str> = first
        .lines
        .iter()
        .flat_map(|line| line.notes.iter().map(|note| note.id.as_str()))
        .collect();
    let mut expected: Vec<String> = (1..=all.len()).map(|n| format!("n_{n:012}")).collect();
    let mut sorted: Vec<String> = all.iter().map(|id| (*id).to_owned()).collect();
    sorted.sort_unstable();
    expected.sort_unstable();
    assert_eq!(sorted, expected);
}

/// A random sketch: a melody of quarter and half notes in C4-B5 over a chord of three or four
/// notes in C2-B3 held through each bar.
fn sketch_strategy() -> impl Strategy<Value = Vec<MidiNote>> {
    let bar = (
        proptest::collection::vec((60_i64..84, 1_u32..3), 2..5),
        proptest::collection::btree_set(36_i64..60, 3..5),
    );
    proptest::collection::vec(bar, 1..5).prop_map(|bars| {
        let mut notes = Vec::new();
        for (bar, (melody, chord)) in (0_u32..).zip(bars) {
            let start = 4.0 * f64::from(bar);
            let mut beat = start;
            for (index, (pitch, length)) in melody.into_iter().enumerate() {
                let length = f64::from(length).min(start + 4.0 - beat);
                if length <= 0.0 {
                    break;
                }
                notes.push(note(&format!("m{bar}_{index}"), pitch, beat, length));
                beat += length;
            }
            for (index, pitch) in chord.into_iter().enumerate() {
                notes.push(note(&format!("c{bar}_{index}"), pitch, start, 4.0));
            }
        }
        notes
    })
}

proptest! {
    /// Any sketch reads and orchestrates into valid lines (TST-07): every note valid, with a
    /// unique id and inside its instrument's playable range; the melody's parts play the melody
    /// at pitch or in octaves; the chord notes are tones of their chord, under the melody unless
    /// reported.
    #[test]
    fn any_sketch_orchestrates_into_valid_lines(
        sketch in sketch_strategy(),
        density in prop_oneof![Just(Density::Thin), Just(Density::Medium), Just(Density::Full)],
    ) {
        let analysis = analyse(&sketch).expect("a valid sketch");
        let orchestra = parts_of("romantic_orchestra", &[]);
        let options = Options { density, ..Options::default() };
        let result = orchestrate(&analysis.reduction, &orchestra, &score(), &options, &mut ids())
            .expect("the sketch is orchestrated");
        prop_assert_eq!(result.lines.len(), orchestra.len());
        let mut seen = BTreeSet::new();
        let melody: Vec<(f64, i64)> = analysis
            .reduction
            .melody
            .iter()
            .map(|note| (note.start, i64::from(note.pitch)))
            .collect();
        for line in &result.lines {
            let PitchModel::Pitched(pitched) = line.instrument.pitch() else {
                prop_assert!(line.notes.is_empty());
                continue;
            };
            let range = pitched.playable.range;
            for note in &line.notes {
                prop_assert!(seen.insert(note.id.clone()), "{} repeats", note.id);
                prop_assert!(note.length > 0.0 && note.start >= 0.0);
                let pitch = i64::from(note.pitch);
                prop_assert!(
                    range.low().get() <= pitch && pitch <= range.high().get(),
                    "{:?}",
                    note
                );
            }
            if line.role == Role::Melody && !line.notes.is_empty() {
                let played: Vec<(f64, i64)> = line
                    .notes
                    .iter()
                    .map(|note| (note.start, i64::from(note.pitch)))
                    .collect();
                let shift = played[0].1 - melody[0].1;
                prop_assert_eq!(shift.rem_euclid(12), 0);
                let moved: Vec<(f64, i64)> =
                    melody.iter().map(|&(beat, pitch)| (beat, pitch + shift)).collect();
                prop_assert_eq!(played, moved);
            }
        }
        assert_harmony_under_melody(&result, &analysis);
    }
}
