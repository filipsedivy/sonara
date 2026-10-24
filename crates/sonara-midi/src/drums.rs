//! Drum grooves on the General MIDI map (`sonara.midi.generators.drum_pattern`).

use sonara_core::error::{DomainError, Result};
use sonara_core::ids::IdGenerator;
use sonara_core::midi::MidiNote;
use sonara_core::pyfmt::str_repr;

use crate::gm::{
    CLAP, CRASH, HAT_CLOSED, HAT_OPEN, HAT_PEDAL, KICK, RIDE, RIM, SNARE, TOM_FLOOR, TOM_HIGH,
    TOM_LOW, TOM_MID,
};
use crate::pyrandom::PyRandom;

/// Steps per bar: sixteenth notes in 4/4.
const STEPS: usize = 16;

/// One drum row of a pattern: the GM pitch and a velocity for each of the 16 steps (0 is rest).
type Row = (i64, [i64; STEPS]);

/// A four-step cell repeated across the bar.
const fn x4(cell: [i64; 4]) -> [i64; STEPS] {
    let mut row = [0; STEPS];
    let mut i = 0;
    while i < STEPS {
        row[i] = cell[i % 4];
        i += 1;
    }
    row
}

/// An eight-step cell repeated twice.
const fn x2(cell: [i64; 8]) -> [i64; STEPS] {
    let mut row = [0; STEPS];
    let mut i = 0;
    while i < STEPS {
        row[i] = cell[i % 8];
        i += 1;
    }
    row
}

/// A two-step cell repeated eight times.
const fn x8(cell: [i64; 2]) -> [i64; STEPS] {
    let mut row = [0; STEPS];
    let mut i = 0;
    while i < STEPS {
        row[i] = cell[i % 2];
        i += 1;
    }
    row
}

const FOUR_ON_THE_FLOOR: [Row; 4] = [
    (KICK, x4([120, 0, 0, 0])),
    (CLAP, x2([0, 0, 0, 0, 110, 0, 0, 0])),
    (HAT_CLOSED, x4([0, 0, 90, 0])),
    (HAT_OPEN, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 80, 0]),
];
const HOUSE: [Row; 4] = [
    (KICK, x4([120, 0, 0, 0])),
    (CLAP, x2([0, 0, 0, 0, 105, 0, 0, 0])),
    (HAT_OPEN, x4([0, 0, 85, 0])),
    (HAT_CLOSED, x4([60, 0, 0, 70])),
];
const TECHNO: [Row; 4] = [
    (KICK, x4([127, 0, 0, 0])),
    (HAT_CLOSED, x4([0, 0, 95, 0])),
    (RIM, x2([0, 0, 0, 0, 0, 0, 0, 70])),
    (CLAP, x2([0, 0, 0, 0, 90, 0, 0, 0])),
];
const BREAKBEAT: [Row; 3] = [
    (KICK, [120, 0, 0, 0, 0, 0, 0, 0, 0, 0, 110, 0, 0, 0, 0, 0]),
    (SNARE, [0, 0, 0, 0, 115, 0, 0, 0, 0, 0, 0, 0, 115, 0, 0, 60]),
    (HAT_CLOSED, x4([90, 0, 80, 0])),
];
const HIPHOP: [Row; 3] = [
    (KICK, [120, 0, 0, 0, 0, 0, 0, 90, 0, 0, 115, 0, 0, 0, 0, 0]),
    (SNARE, [0, 0, 0, 0, 115, 0, 0, 0, 0, 0, 0, 0, 115, 0, 0, 0]),
    (HAT_CLOSED, x4([85, 0, 70, 0])),
];
const TRAP: [Row; 3] = [
    (KICK, [125, 0, 0, 0, 0, 0, 0, 100, 0, 0, 0, 0, 0, 0, 0, 0]),
    (CLAP, [0, 0, 0, 0, 0, 0, 0, 0, 120, 0, 0, 0, 0, 0, 0, 0]),
    (HAT_CLOSED, x8([80, 60])),
];
const ROCK: [Row; 3] = [
    (KICK, [120, 0, 0, 0, 0, 0, 0, 0, 115, 0, 110, 0, 0, 0, 0, 0]),
    (SNARE, [0, 0, 0, 0, 120, 0, 0, 0, 0, 0, 0, 0, 120, 0, 0, 0]),
    (HAT_CLOSED, x4([95, 0, 75, 0])),
];
const HALFTIME: [Row; 3] = [
    (KICK, [120, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100, 0, 0, 0, 0, 0]),
    (SNARE, [0, 0, 0, 0, 0, 0, 0, 0, 120, 0, 0, 0, 0, 0, 0, 0]),
    (HAT_CLOSED, x4([85, 0, 70, 0])),
];
const DNB: [Row; 3] = [
    (KICK, [125, 0, 0, 0, 0, 0, 0, 0, 0, 0, 110, 0, 0, 0, 0, 0]),
    (SNARE, [0, 0, 0, 0, 120, 0, 0, 0, 0, 0, 0, 0, 120, 0, 0, 0]),
    (HAT_CLOSED, x4([80, 0, 70, 0])),
];
const RIDE_JAZZ: [Row; 3] = [
    (
        RIDE,
        [100, 0, 0, 0, 90, 0, 0, 70, 100, 0, 0, 0, 90, 0, 0, 70],
    ),
    (HAT_PEDAL, x2([0, 0, 0, 0, 80, 0, 0, 0])),
    (KICK, [60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
];
const CINEMATIC: [Row; 3] = [
    (
        TOM_FLOOR,
        [120, 0, 0, 0, 0, 0, 90, 0, 0, 0, 110, 0, 0, 0, 0, 0],
    ),
    (TOM_LOW, [0, 0, 0, 0, 0, 0, 0, 0, 100, 0, 0, 0, 0, 0, 95, 0]),
    (KICK, [127, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
];

/// The style names, in the prototype's order (`DRUM_STYLES`).
pub const DRUM_STYLES: [&str; 11] = [
    "four_on_the_floor",
    "house",
    "techno",
    "breakbeat",
    "hiphop",
    "trap",
    "rock",
    "halftime",
    "dnb",
    "ride_jazz",
    "cinematic",
];

/// The rows of a style, in the order the prototype's pattern dictionary lists them.
fn pattern(style: &str) -> Option<&'static [Row]> {
    Some(match style {
        "four_on_the_floor" => &FOUR_ON_THE_FLOOR,
        "house" => &HOUSE,
        "techno" => &TECHNO,
        "breakbeat" => &BREAKBEAT,
        "hiphop" => &HIPHOP,
        "trap" => &TRAP,
        "rock" => &ROCK,
        "halftime" => &HALFTIME,
        "dnb" => &DNB,
        "ride_jazz" => &RIDE_JAZZ,
        "cinematic" => &CINEMATIC,
        _ => return None,
    })
}

/// Options of [`drum_pattern`].
#[derive(Debug, Clone, PartialEq)]
pub struct DrumOptions {
    /// Number of bars, 1–512.
    pub bars: i64,
    /// Length of a bar in beats.
    pub beats_per_bar: f64,
    /// Amount (0..1) of ghost snares and extra kicks.
    pub variation: f64,
    /// Put a snare and tom fill in the last half bar.
    pub fill: bool,
    /// Delay of every second sixteenth, as a fraction of half a step.
    pub swing: f64,
    /// Add a crash cymbal on the first beat.
    pub crash_on_first: bool,
    /// Seed of the variation and of the velocity humanization.
    pub seed: i64,
}

impl Default for DrumOptions {
    fn default() -> Self {
        Self {
            bars: 4,
            beats_per_bar: 4.0,
            variation: 0.2,
            fill: true,
            swing: 0.0,
            crash_on_first: false,
            seed: 0,
        }
    }
}

/// A drum groove on the GM map. `variation` adds ghost notes and kick variations
/// deterministically; `fill` puts a snare and tom fill in the last half bar.
///
/// # Errors
///
/// `INVALID_ARGUMENT`: `unknown drum style '…'` (with the `available` styles) and `bars must be
/// 1..512`; plus the validation errors of [`MidiNote::create`].
pub fn drum_pattern(
    ids: &mut dyn IdGenerator,
    style: &str,
    options: &DrumOptions,
) -> Result<Vec<MidiNote>> {
    let rows = pattern(style).ok_or_else(|| {
        DomainError::invalid(format!("unknown drum style {}", str_repr(style)))
            .with_detail("available", DRUM_STYLES.to_vec())
    })?;
    let bars = u16::try_from(options.bars)
        .ok()
        .filter(|bars| (1..=512).contains(bars))
        .ok_or_else(|| DomainError::invalid("bars must be 1..512"))?;
    let mut rng = PyRandom::new(options.seed);
    let step = options.beats_per_bar / 16.0;
    let mut notes = Vec::new();
    for bar in 0..bars {
        let last = bar == bars - 1;
        let bar_start = f64::from(bar) * options.beats_per_bar;
        for &(pitch, row) in rows {
            for (s, &velocity) in (0_u8..).zip(row.iter()) {
                if options.fill && last && s >= 8 && matches!(pitch, SNARE | KICK | CLAP) {
                    continue;
                }
                let mut v = velocity;
                if v == 0
                    && options.variation > 0.0
                    && pitch == SNARE
                    && rng.random() < options.variation * 0.25
                {
                    v = 35 + rng.randint(0, 15); // ghost note
                }
                if v == 0
                    && options.variation > 0.0
                    && pitch == KICK
                    && s % 2 == 0
                    && rng.random() < options.variation * 0.12
                {
                    v = 85;
                }
                if v == 0 {
                    continue;
                }
                v = (v + rng.randint(-6, 6)).clamp(1, 127);
                let mut t = bar_start + f64::from(s) * step;
                if options.swing > 0.0 && s % 2 == 1 {
                    t += options.swing * step * 0.5;
                }
                notes.push(MidiNote::create(ids, pitch, t, step * 0.9, v, 0)?);
            }
        }
        if options.crash_on_first && bar == 0 {
            notes.push(MidiNote::create(ids, CRASH, 0.0, 1.0, 110, 0)?);
        }
        if options.fill && last {
            let toms = [TOM_HIGH, TOM_MID, TOM_LOW, TOM_FLOOR];
            for (k, s) in (0_u8..).zip(8_u8..16) {
                let pitch = if k < 4 {
                    SNARE
                } else {
                    toms[usize::from((k - 4) % 4)]
                };
                let at = bar_start + f64::from(s) * step;
                let velocity = 85 + i64::from(k) * 4;
                notes.push(MidiNote::create(ids, pitch, at, step * 0.9, velocity, 0)?);
            }
            notes.push(MidiNote::create(
                ids,
                KICK,
                bar_start + 8.0 * step,
                step * 0.9,
                110,
                0,
            )?);
        }
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use sonara_core::ids::SequentialIds;

    use super::{DRUM_STYLES, DrumOptions, drum_pattern, pattern};
    use crate::gm::{CRASH, KICK, SNARE};

    #[test]
    fn every_style_has_a_pattern_with_distinct_rows() {
        for style in DRUM_STYLES {
            let rows = pattern(style).expect("style has a pattern");
            let mut pitches: Vec<i64> = rows.iter().map(|&(pitch, _)| pitch).collect();
            pitches.sort_unstable();
            pitches.dedup();
            assert_eq!(pitches.len(), rows.len(), "{style} repeats a drum");
            assert!(
                rows.iter().all(|(_, row)| row.iter().any(|&v| v > 0)),
                "{style}"
            );
        }
        assert!(pattern("polka").is_none());
    }

    #[test]
    fn four_on_the_floor_without_variation_is_a_fixed_grid() {
        let mut ids = SequentialIds::new(0);
        let options = DrumOptions {
            bars: 1,
            variation: 0.0,
            fill: false,
            ..DrumOptions::default()
        };
        let notes = drum_pattern(&mut ids, "four_on_the_floor", &options).expect("valid groove");
        let kicks: Vec<f64> = notes
            .iter()
            .filter(|n| i64::from(n.pitch) == KICK)
            .map(|n| n.start)
            .collect();
        assert_eq!(kicks, [0.0, 1.0, 2.0, 3.0]);
        assert!(notes.iter().all(|n| (n.length - 0.225).abs() < 1e-12));
        assert!(notes.iter().all(|n| (1..=127).contains(&n.velocity)));
    }

    #[test]
    fn fill_replaces_the_last_half_bar_and_crash_opens_the_loop() {
        let mut ids = SequentialIds::new(0);
        let options = DrumOptions {
            bars: 2,
            variation: 0.0,
            crash_on_first: true,
            ..DrumOptions::default()
        };
        let notes = drum_pattern(&mut ids, "rock", &options).expect("valid groove");
        let crashes: Vec<f64> = notes
            .iter()
            .filter(|n| i64::from(n.pitch) == CRASH)
            .map(|n| n.start)
            .collect();
        assert_eq!(crashes, [0.0]);
        let snares_in_fill = notes
            .iter()
            .filter(|n| i64::from(n.pitch) == SNARE && n.start >= 6.0)
            .count();
        assert_eq!(snares_in_fill, 4);
    }

    #[test]
    fn invalid_arguments_are_reported_like_the_prototype() {
        let mut ids = SequentialIds::new(0);
        let err = drum_pattern(&mut ids, "polka", &DrumOptions::default()).expect_err("style");
        assert_eq!(err.message, "unknown drum style 'polka'");
        assert_eq!(err.details["available"][0], "four_on_the_floor");
        assert_eq!(err.details["available"].as_array().map(Vec::len), Some(11));
        for bars in [0, 513, -1] {
            let options = DrumOptions {
                bars,
                ..DrumOptions::default()
            };
            let err = drum_pattern(&mut ids, "house", &options).expect_err("bars");
            assert_eq!(err.message, "bars must be 1..512");
        }
    }
}
