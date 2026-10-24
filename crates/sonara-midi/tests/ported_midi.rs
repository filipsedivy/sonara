//! Ports of the non-SMF tests of the prototype's `tests/unit/test_midi.py` (theory, generators
//! and transforms) and of the MIDI export test of `tests/integration/test_import_export.py`.
//! The SMF tests of `test_midi.py` are in `smf.rs`; golden parity is in `golden_generators.rs`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use midly::{Smf, Timing, TrackEventKind};
use sonara_core::ids::SequentialIds;
use sonara_core::midi::MidiNote;
use sonara_core::model::clip::{Clip, MidiClip, MidiClipData};
use sonara_core::model::device::{Device, Params};
use sonara_core::model::project::{Project, ProjectData};
use sonara_core::model::track::{Track, TrackData, TrackKind};
use sonara_core::model::{JsonMap, Valid};
use sonara_midi::drums::{DRUM_STYLES, DrumOptions, drum_pattern};
use sonara_midi::generators::{
    ArpeggioOptions, BasslineOptions, BeatsPerChord, ChordNotesOptions, ChordSlot, MelodyOptions,
    arpeggio, bassline, chords_to_notes, melody, progression,
};
use sonara_midi::gm::drum_sound;
use sonara_midi::smf::{encode_smf, parse_smf};
use sonara_midi::theory::{
    Key, Scale, bass_note, chord_from, parse_chord, parse_key, roman_chord, voice,
};
use sonara_midi::transforms::{
    HumanizeOptions, QuantizeOptions, VelocityScale, euclid, humanize, legato, offset, quantize,
    scale_velocity, strum, transpose,
};

fn key(text: &str) -> Key {
    parse_key(text).expect("valid key")
}

fn ids() -> SequentialIds {
    SequentialIds::new(0)
}

/// Note content independent of the (sequential) note IDs, like the prototype's `sig`.
fn sig(notes: &[MidiNote]) -> Vec<String> {
    let mut out: Vec<String> = notes
        .iter()
        .map(|n| {
            format!(
                "{:03} {:.9} {:.9} {}",
                n.pitch, n.start, n.length, n.velocity
            )
        })
        .collect();
    out.sort();
    out
}

fn slots(chords: &[&str], key: Option<Key>, beats: f64) -> Vec<ChordSlot> {
    progression(chords, key, BeatsPerChord::Uniform(beats), 0.0).expect("valid progression")
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

// -- theory -------------------------------------------------------------------------------

#[test]
fn midi_parse_key() {
    let cases = [
        ("C major", 0, Scale::Major),
        ("A minor", 9, Scale::Minor),
        ("Am", 9, Scale::Minor),
        ("F# major", 6, Scale::Major),
        ("Bb", 10, Scale::Major),
        ("D dorian", 2, Scale::Dorian),
        ("eb aeolian", 3, Scale::Minor),
        ("C harmonic minor", 0, Scale::HarmonicMinor),
        ("G pentatonic-minor", 7, Scale::PentatonicMinor),
    ];
    for (text, root, scale) in cases {
        let k = key(text);
        assert_eq!((k.root, k.scale), (root, scale), "{text}");
    }
}

#[test]
fn midi_parse_key_rejects() {
    for text in ["", "H major", "C weird", "#"] {
        assert!(parse_key(text).is_err(), "{text:?}");
    }
}

#[test]
fn midi_key_helpers() {
    let k = key("A minor");
    let degrees: Vec<i64> = (1..=8).map(|d| k.degree_pc(d)).collect();
    assert_eq!(degrees, [9, 11, 0, 2, 4, 5, 7, 9]);
    assert!(k.contains(60) && !k.contains(61));
    assert_eq!(k.pitches(57, 69), [57, 59, 60, 62, 64, 65, 67, 69]);
    assert_eq!(k.snap(61), 60);
    assert_eq!(k.snap(66), 65);
    assert_eq!(k.to_string(), "A minor");
    for scale in Scale::ALL {
        let iv = scale.intervals();
        let mut unique = iv.to_vec();
        unique.dedup();
        assert!(unique.len() == iv.len() && iv[0] == 0, "{}", scale.name());
    }
}

#[test]
fn midi_parse_chord() {
    let cases: [(&str, i64, &[i64], Option<i64>); 9] = [
        ("C", 0, &[0, 4, 7], None),
        ("Am7", 9, &[0, 3, 7, 10], None),
        ("F#m7b5", 6, &[0, 3, 6, 10], None),
        ("Bbmaj9", 10, &[0, 4, 7, 11, 14], None),
        ("G/B", 7, &[0, 4, 7], Some(11)),
        ("Dsus4", 2, &[0, 5, 7], None),
        ("E5", 4, &[0, 7], None),
        ("Cdim7", 0, &[0, 3, 6, 9], None),
        ("Abm/Eb", 8, &[0, 3, 7], Some(3)),
    ];
    for (symbol, root, intervals, bass) in cases {
        let c = parse_chord(symbol).expect("valid chord");
        assert_eq!(
            (c.root, c.intervals.as_slice(), c.bass),
            (root, intervals, bass),
            "{symbol}"
        );
    }
}

#[test]
fn midi_parse_chord_rejects() {
    for symbol in ["", "X", "Cfoo", "C/H", "maj7"] {
        assert!(parse_chord(symbol).is_err(), "{symbol:?}");
    }
}

#[test]
fn midi_roman_numerals() {
    let cases: [(&str, &str, &[i64]); 19] = [
        ("I", "C major", &[0, 4, 7]),
        ("vi", "C major", &[9, 0, 4]),
        ("V7", "C major", &[7, 11, 2, 5]),
        ("ii7", "C major", &[2, 5, 9, 0]),
        ("bVII", "C major", &[10, 2, 5]),
        ("vii°", "C major", &[11, 2, 5]),
        ("IVmaj7", "C major", &[5, 9, 0, 4]),
        ("i", "A minor", &[9, 0, 4]),
        ("VI", "A minor", &[5, 9, 0]),
        ("III", "A minor", &[0, 4, 7]),
        ("VII", "A minor", &[7, 11, 2]),
        ("V", "A minor", &[4, 8, 11]),
        ("iv", "A minor", &[2, 5, 9]),
        ("#iv°", "C major", &[6, 9, 0]),
        ("iiø", "A minor", &[11, 2, 5, 9]),
        ("Isus4", "D major", &[2, 7, 9]),
        ("III", "A pentatonic_minor", &[0, 4, 7]),
        ("IV", "C pentatonic_major", &[5, 9, 0]),
        ("VI", "C blues", &[8, 0, 3]),
    ];
    for (numeral, k, pcs) in cases {
        let chord = roman_chord(numeral, key(k)).expect("valid numeral");
        assert_eq!(chord.pitch_classes(), pcs, "{numeral} in {k}");
    }
}

#[test]
fn midi_roman_rejects() {
    for numeral in ["", "viii", "X", "Ifoo", "IVm9b13"] {
        assert!(roman_chord(numeral, key("C")).is_err(), "{numeral:?}");
    }
}

#[test]
fn midi_chord_from_dispatch() {
    let k = Some(key("C major"));
    assert_eq!(chord_from("IV", k).expect("numeral").root, 5);
    assert_eq!(chord_from("Bb", k).expect("symbol").root, 10);
    assert_eq!(chord_from("Am", None).expect("symbol").root, 9);
    // No key: parsed as a chord symbol and rejected.
    assert!(chord_from("IV", None).is_err());
}

#[test]
fn midi_voice_contains_chord_tones_near_center() {
    let c = parse_chord("Fmaj7").expect("valid chord");
    let v = voice(&c, 60, None, false);
    let mut pcs: Vec<i64> = v.iter().map(|p| p.rem_euclid(12)).collect();
    pcs.sort_unstable();
    pcs.dedup();
    let mut expected: Vec<i64> = c.intervals.iter().map(|i| (5 + i) % 12).collect();
    expected.sort_unstable();
    expected.dedup();
    assert_eq!(pcs, expected);
    let sum = i32::try_from(v.iter().sum::<i64>()).expect("small sum");
    let mean = f64::from(sum) / f64::from(i32::try_from(v.len()).expect("few notes"));
    assert!((mean - 60.0).abs() <= 6.0);
    assert!(v.is_sorted());
}

#[test]
fn midi_voice_leading_minimizes_motion() {
    let previous = [60, 64, 67]; // C major, root position
    let f = parse_chord("F").expect("valid chord");
    let led = voice(&f, 62, Some(&previous), false);
    let motion = |notes: &[i64]| -> i64 {
        notes
            .iter()
            .map(|p| {
                previous
                    .iter()
                    .map(|q| (p - q).abs())
                    .min()
                    .expect("non-empty")
            })
            .sum()
    };
    assert!(motion(&led) <= motion(&[65, 69, 72]));
    assert!(motion(&led) <= 3); // C-F-A from C-E-G: E->F and G->A
}

#[test]
fn midi_voice_slash_bass_and_spread() {
    let v = voice(&parse_chord("C/E").expect("valid chord"), 60, None, false);
    assert!(v[0] % 12 == 4 && v[0] < *v[1..].iter().min().expect("non-empty"));
    let spread = voice(&parse_chord("C").expect("valid chord"), 60, None, true);
    assert!(spread[0] <= *spread[1..].iter().min().expect("non-empty") - 5);
    assert_eq!(bass_note(&parse_chord("G/B").expect("valid chord"), 2), 47);
    assert_eq!(bass_note(&parse_chord("A").expect("valid chord"), 1), 33);
}

// -- generators ---------------------------------------------------------------------------

#[test]
fn midi_progression_layout() {
    let result = progression(
        &["i", "VI", "III", "VII"],
        Some(key("A minor")),
        BeatsPerChord::Each(&[4.0, 4.0, 2.0, 6.0]),
        8.0,
    )
    .expect("valid progression");
    let layout: Vec<(f64, f64)> = result.iter().map(|s| (s.start, s.length)).collect();
    assert_eq!(layout, [(8.0, 4.0), (12.0, 4.0), (16.0, 2.0), (18.0, 6.0)]);
    let none: [&str; 0] = [];
    assert!(progression(&none, None, BeatsPerChord::Uniform(4.0), 0.0).is_err());
    assert!(progression(&["C", "F"], None, BeatsPerChord::Each(&[4.0]), 0.0).is_err());
}

#[test]
fn midi_chords_rhythms_stay_in_slots() {
    let s = slots(&["C", "Am", "F", "G"], None, 4.0);
    for rhythm in ["sustain", "stabs", "offbeat", "pulse8", "half"] {
        let options = ChordNotesOptions {
            rhythm: rhythm.parse().expect("known rhythm"),
            ..ChordNotesOptions::default()
        };
        let notes = chords_to_notes(&mut ids(), &s, &options).expect("notes");
        assert_ne!(notes.len(), 0, "{rhythm}");
        for n in &notes {
            let slot = s
                .iter()
                .find(|slot| slot.start - 1e-9 <= n.start && n.start < slot.start + slot.length)
                .expect("a slot");
            assert!(n.end() <= slot.start + slot.length + 1e-9, "{rhythm}");
            assert!(
                slot.chord
                    .pitch_classes()
                    .contains(&i64::from(n.pitch % 12)),
                "{rhythm}"
            );
        }
    }
    assert!(
        "swing"
            .parse::<sonara_midi::generators::ChordRhythm>()
            .is_err()
    );
}

#[test]
fn midi_arpeggio_patterns() {
    let s = slots(&["Am", "F"], None, 4.0);
    for pattern in ["up", "down", "updown", "downup", "random", "pinky"] {
        let options = ArpeggioOptions {
            pattern: pattern.parse().expect("known pattern"),
            rate: 0.25,
            octaves: 2,
            seed: 3,
            ..ArpeggioOptions::default()
        };
        let notes = arpeggio(&mut ids(), &s, &options).expect("notes");
        assert_eq!(notes.len(), 32, "{pattern}");
        assert!(notes.iter().all(|n| (n.length - 0.2).abs() < 1e-9));
        assert_eq!(
            sig(&notes),
            sig(&arpeggio(&mut ids(), &s, &options).expect("notes"))
        );
        let first: Vec<u8> = notes.iter().take(4).map(|n| n.pitch).collect();
        let mut sorted = first.clone();
        sorted.sort_unstable();
        if pattern == "up" {
            assert_eq!(first, sorted);
        }
        if pattern == "down" {
            sorted.reverse();
            assert_eq!(first, sorted);
        }
    }
    assert!(
        "zigzag"
            .parse::<sonara_midi::generators::ArpeggioPattern>()
            .is_err()
    );
}

#[test]
fn midi_bassline_styles() {
    let k = key("A minor");
    let s = slots(&["i", "VI", "III", "VII"], Some(k), 8.0);
    for style in [
        "root",
        "pulse",
        "octaves",
        "offbeat",
        "fifths",
        "walking",
        "syncopated",
    ] {
        let options = BasslineOptions {
            style: style.parse().expect("known style"),
            octave: 2,
            seed: 1,
            key: Some(k),
            ..BasslineOptions::default()
        };
        let notes = bassline(&mut ids(), &s, &options).expect("notes");
        assert_ne!(notes.len(), 0, "{style}");
        for slot in &s {
            let inside: Vec<&MidiNote> = notes
                .iter()
                .filter(|n| slot.start - 1e-9 <= n.start && n.start < slot.start + slot.length)
                .collect();
            assert_ne!(
                inside.len(),
                0,
                "{style}: no notes in slot at {}",
                slot.start
            );
            // Each 8-beat chord is covered in both of its bars (not just the first).
            if style != "root" {
                assert!(
                    inside.iter().any(|n| n.start >= slot.start + 4.0),
                    "{style}"
                );
            }
            if style != "walking" {
                assert!(
                    inside
                        .iter()
                        .all(|n| n.end() <= slot.start + slot.length + 1e-9)
                );
            }
        }
        assert!(
            notes.iter().all(|n| (24..=60).contains(&n.pitch)),
            "{style}"
        );
        assert_eq!(
            sig(&notes),
            sig(&bassline(&mut ids(), &s, &options).expect("notes"))
        );
    }
    assert!(
        "slap"
            .parse::<sonara_midi::generators::BassStyle>()
            .is_err()
    );
}

#[test]
fn midi_drum_patterns() {
    for style in DRUM_STYLES {
        let options = DrumOptions {
            bars: 4,
            seed: 7,
            ..DrumOptions::default()
        };
        let notes = drum_pattern(&mut ids(), style, &options).expect("notes");
        assert_ne!(notes.len(), 0, "{style}");
        assert!(
            notes.iter().all(|n| drum_sound(n.pitch).is_some()),
            "{style}"
        );
        assert!(
            notes.iter().all(|n| (0.0..16.0).contains(&n.start)),
            "{style}"
        );
        assert_eq!(
            sig(&notes),
            sig(&drum_pattern(&mut ids(), style, &options).expect("notes"))
        );
        let toms = [41, 43, 45, 47, 48, 50];
        assert!(
            notes
                .iter()
                .any(|n| toms.contains(&n.pitch) && n.start >= 14.0),
            "{style}"
        );
    }
}

#[test]
fn midi_drum_pattern_options() {
    let plain = DrumOptions {
        bars: 2,
        variation: 0.0,
        fill: false,
        seed: 1,
        ..DrumOptions::default()
    };
    let notes = drum_pattern(&mut ids(), "rock", &plain).expect("notes");
    let mut kicks: Vec<f64> = notes
        .iter()
        .filter(|n| n.pitch == 36)
        .map(|n| n.start)
        .collect();
    kicks.sort_by(f64::total_cmp);
    assert_eq!(kicks, [0.0, 2.0, 2.5, 4.0, 6.0, 6.5]);
    assert!(notes.iter().all(|n| n.start < 8.0));

    let crash = DrumOptions {
        bars: 1,
        crash_on_first: true,
        ..DrumOptions::default()
    };
    let notes = drum_pattern(&mut ids(), "house", &crash).expect("notes");
    assert!(notes.iter().any(|n| n.pitch == 49 && n.start == 0.0));

    let swung = DrumOptions {
        bars: 1,
        variation: 0.0,
        fill: false,
        swing: 1.0,
        ..DrumOptions::default()
    };
    let notes = drum_pattern(&mut ids(), "house", &swung).expect("notes");
    assert!(notes.iter().filter(|n| n.pitch == 42).any(|n| {
        let steps = n.start / 0.25;
        (steps - steps.round()).abs() > 0.1
    }));

    let waltz = DrumOptions {
        bars: 2,
        beats_per_bar: 3.0,
        fill: false,
        variation: 0.0,
        ..DrumOptions::default()
    };
    let notes = drum_pattern(&mut ids(), "rock", &waltz).expect("notes");
    assert!(notes.iter().all(|n| n.start < 6.0));

    for bars in [0, 513] {
        let bad = DrumOptions {
            bars,
            ..DrumOptions::default()
        };
        assert!(
            drum_pattern(&mut ids(), "rock", &bad).is_err(),
            "bars {bars}"
        );
    }
    assert!(drum_pattern(&mut ids(), "polka", &DrumOptions::default()).is_err());
}

#[test]
fn midi_melody_in_key_and_range() {
    let k = key("D dorian");
    let s = slots(&["i", "IV", "i", "VII"], Some(k), 4.0);
    let options = MelodyOptions {
        low: 62,
        high: 81,
        seed: 5,
        ..MelodyOptions::default()
    };
    let notes = melody(&mut ids(), k, &s, &options).expect("notes");
    assert!(notes.len() >= 8);
    assert!(
        notes
            .iter()
            .all(|n| k.contains(i64::from(n.pitch)) && (62..=81).contains(&n.pitch))
    );
    assert!(notes.iter().all(|n| (0.0..16.0).contains(&n.start)));
    assert_eq!(
        sig(&notes),
        sig(&melody(&mut ids(), k, &s, &options).expect("notes"))
    );
    let other = MelodyOptions { seed: 6, ..options };
    assert_ne!(
        sig(&notes),
        sig(&melody(&mut ids(), k, &s, &other).expect("notes"))
    );
    let empty = MelodyOptions {
        low: 61,
        high: 61,
        ..MelodyOptions::default()
    };
    assert!(melody(&mut ids(), key("C major"), &s, &empty).is_err());
}

// -- transformations ----------------------------------------------------------------------

fn notes_at(starts: &[f64]) -> Vec<MidiNote> {
    starts
        .iter()
        .enumerate()
        .map(|(i, s)| MidiNote::new(format!("n_{i}"), 60, *s, 0.2, 100, 0).expect("valid note"))
        .collect()
}

fn starts_of(notes: &[MidiNote]) -> Vec<f64> {
    notes.iter().map(|n| n.start).collect()
}

fn all_near(actual: &[f64], expected: &[f64]) -> bool {
    actual.len() == expected.len() && actual.iter().zip(expected).all(|(a, b)| near(*a, *b))
}

#[test]
fn midi_quantize() {
    let notes = notes_at(&[0.1, 0.4, 0.6, 1.05]);
    let q = quantize(&notes, 0.5, &QuantizeOptions::default()).expect("valid");
    assert!(all_near(&starts_of(&q), &[0.0, 0.5, 0.5, 1.0]));
    let half = QuantizeOptions {
        strength: 0.5,
        ..QuantizeOptions::default()
    };
    let h = quantize(&notes, 0.5, &half).expect("valid");
    assert!(all_near(&starts_of(&h), &[0.05, 0.45, 0.55, 1.025]));
    let full_swing = QuantizeOptions {
        swing: 1.0,
        ..QuantizeOptions::default()
    };
    let swung = quantize(&notes_at(&[0.0, 0.5, 1.0, 1.5]), 0.5, &full_swing).expect("valid");
    assert!(all_near(&starts_of(&swung), &[0.0, 0.75, 1.0, 1.75]));
    let ends = QuantizeOptions {
        ends: true,
        ..QuantizeOptions::default()
    };
    let single = [MidiNote::new("n_x", 60, 0.1, 0.62, 100, 0).expect("valid note")];
    let e = quantize(&single, 0.25, &ends).expect("valid");
    assert!(e[0].start == 0.0 && near(e[0].length, 0.75));
    let ids_of = |notes: &[MidiNote]| notes.iter().map(|n| n.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids_of(&q), ids_of(&notes)); // identity is preserved
    assert!(quantize(&notes, 0.0, &QuantizeOptions::default()).is_err());
}

#[test]
fn midi_humanize_bounds_and_determinism() {
    let notes: Vec<MidiNote> = (0..200)
        .map(|i| {
            MidiNote::new(format!("n_{i}"), 60, f64::from(i) * 0.5, 0.25, 64, 0)
                .expect("valid note")
        })
        .collect();
    let options = HumanizeOptions {
        timing: 0.01,
        velocity: 8,
        length: 0.1,
        seed: 9,
    };
    let h = humanize(&notes, &options).expect("valid");
    assert!(
        notes
            .iter()
            .zip(&h)
            .all(|(a, b)| a.velocity.abs_diff(b.velocity) <= 8)
    );
    assert!(
        notes
            .iter()
            .zip(&h)
            .all(|(a, b)| (a.start - b.start).abs() < 0.06)
    );
    assert!(
        h.iter()
            .all(|b| (0.225 - 1e-9..=0.275 + 1e-9).contains(&b.length))
    );
    assert!(h.iter().all(|b| b.start >= 0.0));
    assert_eq!(h, humanize(&notes, &options).expect("valid"));
    let other = HumanizeOptions {
        seed: 10,
        ..options
    };
    assert_ne!(h, humanize(&notes, &other).expect("valid"));
    let none = HumanizeOptions {
        timing: 0.0,
        velocity: 0,
        length: 0.0,
        seed: 0,
    };
    assert_eq!(humanize(&notes, &none).expect("valid"), notes);
}

#[test]
fn midi_transpose_velocity_offset_legato_strum() {
    let notes = notes_at(&[0.0, 1.0]);
    let pitches: Vec<u8> = transpose(&notes, 12)
        .expect("valid")
        .iter()
        .map(|n| n.pitch)
        .collect();
    assert_eq!(pitches, [72, 72]);
    assert!(transpose(&notes, 70).is_err());
    let loud = VelocityScale {
        factor: 2.0,
        ..VelocityScale::default()
    };
    let velocities: Vec<u8> = scale_velocity(&notes, &loud)
        .expect("valid")
        .iter()
        .map(|n| n.velocity)
        .collect();
    assert_eq!(velocities, [127, 127]);
    let five: Vec<MidiNote> = (0..5)
        .map(|i| {
            MidiNote::new(format!("n_{i}"), 60, f64::from(i), 0.5, 100, 0).expect("valid note")
        })
        .collect();
    let ramp = VelocityScale {
        curve: Some((0.5, 1.0)),
        span: Some(4.0),
        ..VelocityScale::default()
    };
    let ramped: Vec<u8> = scale_velocity(&five, &ramp)
        .expect("valid")
        .iter()
        .map(|n| n.velocity)
        .collect();
    assert_eq!(ramped, [50, 62, 75, 88, 100]);
    assert_eq!(starts_of(&offset(&notes, 2.0).expect("valid")), [2.0, 3.0]);
    assert_eq!(offset(&notes, -0.5).expect("valid").len(), 1);
    let lengths: Vec<f64> = legato(&notes_at(&[0.0, 1.0, 3.0]), 0.0)
        .expect("valid")
        .iter()
        .map(|n| n.length)
        .collect();
    assert_eq!(lengths, [1.0, 2.0, 0.2]);
    let chord: Vec<MidiNote> = [67, 60, 64]
        .iter()
        .map(|p| MidiNote::new(format!("n_{p}"), *p, 0.0, 1.0, 100, 0).expect("valid note"))
        .collect();
    let mut strummed: Vec<(u8, String)> = strum(&chord, 0.05)
        .expect("valid")
        .iter()
        .map(|n| (n.pitch, format!("{:.6}", n.start)))
        .collect();
    strummed.sort();
    let expected = [(60, "0.000000"), (64, "0.050000"), (67, "0.100000")];
    assert_eq!(strummed, expected.map(|(p, s)| (p, s.to_owned())));
}

#[test]
fn midi_euclid() {
    let e = |p, s, r| euclid(p, s, r).expect("valid");
    assert_eq!(
        e(3, 8, 0),
        [true, false, false, true, false, false, true, false]
    );
    assert!(e(5, 8, 0).iter().filter(|b| **b).count() == 5 && e(5, 8, 0)[0]);
    assert_eq!(e(0, 4, 0), [false; 4]);
    assert_eq!(e(4, 4, 0), [true; 4]);
    assert_eq!(
        e(3, 8, 1),
        [false, true, false, false, true, false, false, true]
    );
    assert_eq!(e(3, 8, 8), e(3, 8, 0));
    for (p, s) in [(5, 4), (-1, 4), (1, 0)] {
        assert!(euclid(p, s, 0).is_err());
    }
}

#[test]
fn midi_key_dataclass_is_hashable() {
    let hash = |k: &Key| {
        let mut hasher = DefaultHasher::new();
        k.hash(&mut hasher);
        hasher.finish()
    };
    assert_eq!(
        hash(&Key {
            root: 0,
            scale: Scale::Major
        }),
        hash(&key("C"))
    );
}

// -- test_import_export.py ----------------------------------------------------------------

fn instrument(id: &str, name: &str, plugin: &str, notes: Vec<MidiNote>) -> Track {
    let clip = MidiClipData {
        id: format!("clp_{id}"),
        name: name.to_owned(),
        start: 0.0,
        length: 8.0,
        notes: Arc::new(notes),
        controls: Arc::new(Vec::new()),
        content_offset: 0.0,
        loop_length: None,
        muted: false,
        color: None,
        extra: JsonMap::new(),
    };
    let mut data = TrackData::new(id, name, TrackKind::Instrument);
    data.clips = vec![Clip::Midi(MidiClip::new(clip).expect("valid clip"))];
    data.instrument =
        Some(Device::create(&mut ids(), plugin, name, Params::default(), None).expect("device"));
    Valid::new(data).expect("valid track")
}

/// The export part of `test_midi_export_and_reimport_roundtrip` (the application service's
/// `CONFLICT` for an existing file belongs to `sonara-app`).
#[test]
fn import_export_midi_export_and_reimport_roundtrip() {
    let synth: Vec<MidiNote> = (0..8_i32)
        .map(|i| {
            let pitch = 60 + i64::from(i % 3) * 4;
            MidiNote::new(format!("s{i}"), pitch, f64::from(i), 0.9, 90, 0)
        })
        .collect::<Result<_, _>>()
        .expect("valid notes");
    let drums: Vec<MidiNote> = (0..8_i32)
        .map(|i| {
            let pitch = if i % 2 == 0 { 36 } else { 38 };
            MidiNote::new(format!("d{i}"), pitch, f64::from(i), 0.25, 110, 0)
        })
        .collect::<Result<_, _>>()
        .expect("valid notes");
    let mut data = ProjectData::new("prj_1", "Exp");
    data.tracks = vec![
        instrument("trk_k", "Keys", "sonara.synth.poly", synth),
        instrument("trk_d", "Drums", "sonara.drums", drums),
    ];
    let project: Project = Valid::new(data).expect("valid project");
    let encoded = encode_smf(&project, None).expect("export");
    assert_eq!(encoded.notes, 16);
    assert!(encoded.max_rounding_error_beats < 1.0 / 960.0);
    let raw = Smf::parse(&encoded.bytes).expect("standard file");
    assert_eq!(raw.header.timing, Timing::Metrical(960.into()));
    let drum_channels: Vec<u8> = raw
        .tracks
        .iter()
        .flatten()
        .filter_map(|e| match e.kind {
            TrackEventKind::Midi {
                channel,
                message: midly::MidiMessage::NoteOn { .. },
            } => Some(channel.as_int()),
            _ => None,
        })
        .collect();
    assert!(drum_channels.contains(&9)); // the drum kit exports on channel 10
    let imported = parse_smf(&mut ids(), &encoded.bytes, "song.mid").expect("import");
    let mut counts: Vec<usize> = imported.tracks.iter().map(|t| t.notes.len()).collect();
    counts.sort_unstable();
    assert_eq!(counts, [8, 8]);
    assert!(
        imported
            .tracks
            .iter()
            .any(sonara_midi::smf::ImportedTrack::is_drums)
    );
}
