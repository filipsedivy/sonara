//! The generators and transforms reproduce the prototype's output (golden `midi/generators.json`).
//!
//! The recorder ran every call inside one `deterministic(seed=1000)` block, so note IDs count up
//! across all calls. The test therefore makes the same calls in the recorder's order
//! (`scripts/golden/area_midi.py`) with one shared ID generator. The JSON object keys are sorted
//! in the file, so the order of calls comes from this test, not from the file.

use std::cmp::Ordering;

use serde_json::Value;
use sonara_core::ids::SequentialIds;
use sonara_core::midi::MidiNote;
use sonara_midi::drums::{DRUM_STYLES, DrumOptions, drum_pattern};
use sonara_midi::generators::{
    ArpeggioOptions, BasslineOptions, BeatsPerChord, ChordNotesOptions, ChordSlot, MelodyOptions,
    arpeggio, bassline, chords_to_notes, melody, progression,
};
use sonara_midi::theory::{Key, parse_key};
use sonara_midi::transforms::{
    HumanizeOptions, QuantizeOptions, VelocityScale, euclid, humanize, legato, offset, quantize,
    scale_velocity, strum, transpose,
};
use sonara_testkit::Golden;

const SEEDS: [i64; 3] = [0, 1, 42];
const PROGRESSIONS: [&str; 3] = ["a-minor-pop", "c-major-jazz", "symbols"];
const RHYTHMS: [&str; 5] = ["sustain", "stabs", "offbeat", "pulse8", "half"];
const PATTERNS: [&str; 6] = ["up", "down", "updown", "downup", "random", "pinky"];
const STYLES: [&str; 7] = [
    "root",
    "walking",
    "offbeat",
    "pulse",
    "octaves",
    "fifths",
    "syncopated",
];

/// Largest relative difference allowed in a float field. Everything except `humanize` matches
/// bit for bit; `humanize` goes through `gauss`, whose `sin`, `cos` and `ln` come from the
/// platform's libm and may differ from the recording platform's in the last bit.
const EXACT: f64 = 0.0;
const LIBM: f64 = 1e-12;

fn same_float(expected: f64, actual: f64, tolerance: f64) -> bool {
    expected.partial_cmp(&actual) == Some(Ordering::Equal)
        || (expected - actual).abs() <= tolerance * expected.abs().max(actual.abs())
}

fn expect_notes(what: &str, expected: &Value, actual: &[MidiNote], tolerance: f64) {
    let expected = expected.as_array().expect("golden notes are an array");
    assert_eq!(expected.len(), actual.len(), "{what}: note count");
    for (index, (want, got)) in expected.iter().zip(actual).enumerate() {
        let at = format!("{what} #{index}");
        assert_eq!(want["id"].as_str(), Some(got.id.as_str()), "{at}: id");
        assert_eq!(
            want["pitch"].as_u64(),
            Some(u64::from(got.pitch)),
            "{at}: pitch"
        );
        assert_eq!(
            want["velocity"].as_u64(),
            Some(u64::from(got.velocity)),
            "{at}: velocity"
        );
        assert_eq!(
            want["channel"].as_u64(),
            Some(u64::from(got.channel)),
            "{at}: channel"
        );
        for (field, value) in [("start", got.start), ("length", got.length)] {
            let want = want[field].as_f64().expect("golden float");
            assert!(
                same_float(want, value, tolerance),
                "{at}: {field} expected {want:e}, got {value:e}"
            );
        }
    }
}

/// Checks `section[name]` and counts it, so that unchecked golden entries fail the test.
fn check(section: &Value, name: &str, actual: &[MidiNote], tolerance: f64, checked: &mut usize) {
    let expected = section
        .get(name)
        .unwrap_or_else(|| panic!("golden has no entry {name}"));
    expect_notes(name, expected, actual, tolerance);
    *checked += 1;
}

fn slots_of(case: &Value, key: Option<Key>) -> Vec<ChordSlot> {
    let chords: Vec<&str> = case["chords"]
        .as_array()
        .expect("chords")
        .iter()
        .map(|chord| chord.as_str().expect("chord symbol"))
        .collect();
    progression(&chords, key, BeatsPerChord::Uniform(4.0), 0.0).expect("valid progression")
}

fn pattern_options(pattern: &str, seed: i64) -> ArpeggioOptions {
    ArpeggioOptions {
        pattern: pattern.parse().expect("known pattern"),
        octaves: 2,
        seed,
        ..ArpeggioOptions::default()
    }
}

#[test]
fn progressions_reproduce_the_prototype() {
    let golden = Golden::open().expect("golden data");
    let data: Value = golden
        .json("midi/generators.json")
        .expect("generators.json");
    let mut ids = SequentialIds::new(1000);
    let mut checked = 0;

    for name in PROGRESSIONS {
        let case = &data[name];
        let key = case["key"]
            .as_str()
            .map(|text| parse_key(text).expect("valid key"));
        let slots = slots_of(case, key);

        for rhythm in RHYTHMS {
            let options = ChordNotesOptions {
                rhythm: rhythm.parse().expect("known rhythm"),
                ..ChordNotesOptions::default()
            };
            let notes = chords_to_notes(&mut ids, &slots, &options).expect("chord notes");
            check(
                &case["chords_to_notes"],
                rhythm,
                &notes,
                EXACT,
                &mut checked,
            );
        }
        for pattern in PATTERNS {
            for seed in SEEDS {
                let notes =
                    arpeggio(&mut ids, &slots, &pattern_options(pattern, seed)).expect("arpeggio");
                let name = format!("{pattern}-seed{seed}");
                check(&case["arpeggio"], &name, &notes, EXACT, &mut checked);
            }
        }
        for style in STYLES {
            for seed in SEEDS {
                let options = BasslineOptions {
                    style: style.parse().expect("known style"),
                    seed,
                    key,
                    ..BasslineOptions::default()
                };
                let notes = bassline(&mut ids, &slots, &options).expect("bassline");
                let name = format!("{style}-seed{seed}");
                check(&case["bassline"], &name, &notes, EXACT, &mut checked);
            }
        }
        if let Some(key) = key {
            for seed in SEEDS {
                let options = MelodyOptions {
                    seed,
                    ..MelodyOptions::default()
                };
                let notes = melody(&mut ids, key, &slots, &options).expect("melody");
                check(
                    &case["melody"],
                    &format!("seed{seed}"),
                    &notes,
                    EXACT,
                    &mut checked,
                );
            }
        } else {
            assert!(
                case.get("melody").is_none(),
                "{name}: golden has a melody without a key"
            );
        }
    }

    for style in DRUM_STYLES {
        for seed in SEEDS {
            let options = DrumOptions {
                swing: 0.1,
                crash_on_first: true,
                seed,
                ..DrumOptions::default()
            };
            let notes = drum_pattern(&mut ids, style, &options).expect("drum pattern");
            let name = format!("{style}-seed{seed}");
            check(&data["drum_pattern"], &name, &notes, EXACT, &mut checked);
        }
    }

    // 3 progressions x (5 + 18 + 21) + 2 melody keys x 3 + 11 styles x 3.
    assert_eq!(checked, 3 * (5 + 18 + 21) + 2 * 3 + 11 * 3);
    let in_golden: usize = PROGRESSIONS
        .iter()
        .map(|name| {
            ["chords_to_notes", "arpeggio", "bassline", "melody"]
                .iter()
                .filter_map(|section| data[*name].get(*section))
                .map(|section| section.as_object().expect("section").len())
                .sum::<usize>()
        })
        .sum::<usize>()
        + data["drum_pattern"]
            .as_object()
            .expect("drum_pattern")
            .len();
    assert_eq!(checked, in_golden, "every golden entry is checked");
}

#[test]
fn transforms_reproduce_the_prototype() {
    let golden = Golden::open().expect("golden data");
    let data: Value = golden
        .json("midi/generators.json")
        .expect("generators.json");
    let expected = &data["transforms"];
    let mut checked = 0;

    // The recorder consumed the IDs of every earlier call before it built `base`: 3 progressions
    // of generator output (counted by replaying them) and the 33 drum patterns.
    let mut ids = SequentialIds::new(1000);
    replay_ids_of_earlier_calls(&data, &mut ids);
    let base = drum_pattern(
        &mut ids,
        DRUM_STYLES[0],
        &DrumOptions {
            seed: 3,
            ..DrumOptions::default()
        },
    )
    .expect("base pattern");

    let quantize_options = QuantizeOptions {
        strength: 0.5,
        swing: 0.2,
        ..QuantizeOptions::default()
    };
    let notes = quantize(&base, 0.25, &quantize_options).expect("quantize");
    check(expected, "quantize", &notes, EXACT, &mut checked);

    let humanize_options = HumanizeOptions {
        timing: 0.02,
        velocity: 10,
        length: 0.1,
        seed: 5,
    };
    let notes = humanize(&base, &humanize_options).expect("humanize");
    check(expected, "humanize", &notes, LIBM, &mut checked);

    let notes = transpose(&base, 7).expect("transpose");
    check(expected, "transpose", &notes, EXACT, &mut checked);

    let scaling = VelocityScale {
        factor: 0.7,
        offset: 5,
        ..VelocityScale::default()
    };
    let notes = scale_velocity(&base, &scaling).expect("scale_velocity");
    check(expected, "scale_velocity", &notes, EXACT, &mut checked);

    let notes = offset(&base, 2.5).expect("offset");
    check(expected, "offset", &notes, EXACT, &mut checked);

    let notes = legato(&base, 0.05).expect("legato");
    check(expected, "legato", &notes, EXACT, &mut checked);

    let notes = strum(&base, 0.03).expect("strum");
    check(expected, "strum", &notes, EXACT, &mut checked);

    let rhythms: Vec<Vec<bool>> = [(3, 8, 0), (5, 8, 2), (7, 16, 1), (4, 12, 3)]
        .into_iter()
        .map(|(pulses, steps, rotation)| euclid(pulses, steps, rotation).expect("euclid"))
        .collect();
    let want: Vec<Vec<bool>> = expected["euclid"]
        .as_array()
        .expect("euclid cases")
        .iter()
        .map(|row| {
            row.as_array()
                .expect("row")
                .iter()
                .map(|step| step.as_bool().expect("bool"))
                .collect()
        })
        .collect();
    assert_eq!(rhythms, want, "euclid");

    assert_eq!(checked, 7);
    assert_eq!(expected.as_object().expect("transforms").len(), checked + 1);
}

/// Advances `ids` past every note the recorder created before `base`.
fn replay_ids_of_earlier_calls(data: &Value, ids: &mut SequentialIds) {
    for name in PROGRESSIONS {
        let case = &data[name];
        let key = case["key"]
            .as_str()
            .map(|text| parse_key(text).expect("valid key"));
        let slots = slots_of(case, key);
        for rhythm in RHYTHMS {
            let options = ChordNotesOptions {
                rhythm: rhythm.parse().expect("known rhythm"),
                ..ChordNotesOptions::default()
            };
            chords_to_notes(ids, &slots, &options).expect("chord notes");
        }
        for pattern in PATTERNS {
            for seed in SEEDS {
                arpeggio(ids, &slots, &pattern_options(pattern, seed)).expect("arpeggio");
            }
        }
        for style in STYLES {
            for seed in SEEDS {
                let options = BasslineOptions {
                    style: style.parse().expect("known style"),
                    seed,
                    key,
                    ..BasslineOptions::default()
                };
                bassline(ids, &slots, &options).expect("bassline");
            }
        }
        if let Some(key) = key {
            for seed in SEEDS {
                let options = MelodyOptions {
                    seed,
                    ..MelodyOptions::default()
                };
                melody(ids, key, &slots, &options).expect("melody");
            }
        }
    }
    for style in DRUM_STYLES {
        for seed in SEEDS {
            let options = DrumOptions {
                swing: 0.1,
                crash_on_first: true,
                seed,
                ..DrumOptions::default()
            };
            drum_pattern(ids, style, &options).expect("drum pattern");
        }
    }
}
