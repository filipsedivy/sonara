//! Note transformations (`sonara.midi.generators`: quantize, humanize, transpose, scale
//! velocity, offset, legato, strum, euclid). Each returns new notes with the original IDs and
//! validates them again, as the prototype's `dataclasses.replace` does.

use std::collections::HashMap;

use sonara_core::EPSILON;
use sonara_core::error::{DomainError, Result};
use sonara_core::midi::MidiNote;

use crate::num::{py_cmp, py_max, py_min, py_mod, py_round, py_round6};
use crate::pyrandom::PyRandom;

/// `note` with new timing and velocity, validated like a fresh note; its articulation is kept.
fn with_timing(note: &MidiNote, start: f64, length: f64, velocity: i64) -> Result<MidiNote> {
    note.replaced(
        i64::from(note.pitch),
        start,
        length,
        velocity,
        i64::from(note.channel),
    )
}

/// Options of [`quantize`].
#[derive(Debug, Clone, PartialEq)]
pub struct QuantizeOptions {
    /// How far starts move toward the grid, 0..1.
    pub strength: f64,
    /// Delay of every second grid position as a fraction of half a grid step, 0..1.
    pub swing: f64,
    /// Quantize note ends too.
    pub ends: bool,
}

impl Default for QuantizeOptions {
    fn default() -> Self {
        Self {
            strength: 1.0,
            swing: 0.0,
            ends: false,
        }
    }
}

/// Moves note starts toward a grid of `grid` beats.
///
/// # Errors
///
/// `INVALID_ARGUMENT`: `quantize grid must be > 0`; plus the note validation errors.
pub fn quantize(notes: &[MidiNote], grid: f64, options: &QuantizeOptions) -> Result<Vec<MidiNote>> {
    if grid.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
        return Err(DomainError::invalid("quantize grid must be > 0"));
    }
    notes
        .iter()
        .map(|note| {
            let k = (note.start / grid).round_ties_even();
            let swung = if py_mod(k, 2.0) > 0.5 {
                options.swing * grid * 0.5
            } else {
                0.0
            };
            let target = k * grid + swung;
            let start = note.start + (target - note.start) * options.strength;
            let length = if options.ends {
                let end_k = py_max(
                    k + 1.0,
                    ((note.start + note.length) / grid).round_ties_even(),
                );
                py_max(grid * 0.25, end_k * grid - start)
            } else {
                note.length
            };
            with_timing(note, py_max(0.0, start), length, i64::from(note.velocity))
        })
        .collect()
}

/// Options of [`humanize`].
#[derive(Debug, Clone, PartialEq)]
pub struct HumanizeOptions {
    /// Standard deviation of the start shift in beats.
    pub timing: f64,
    /// Maximum velocity shift in either direction.
    pub velocity: i64,
    /// Maximum relative length change.
    pub length: f64,
    /// Seed of the deviations.
    pub seed: i64,
}

impl Default for HumanizeOptions {
    fn default() -> Self {
        Self {
            timing: 0.01,
            velocity: 8,
            length: 0.0,
            seed: 0,
        }
    }
}

/// Reproducible random deviations of start, velocity and length.
///
/// # Errors
///
/// The note validation errors.
pub fn humanize(notes: &[MidiNote], options: &HumanizeOptions) -> Result<Vec<MidiNote>> {
    let mut rng = PyRandom::new(options.seed);
    notes
        .iter()
        .map(|note| {
            let dt = if options.timing > 0.0 {
                rng.gauss(0.0, options.timing)
            } else {
                0.0
            };
            let dv = if options.velocity > 0 {
                rng.randint(-options.velocity, options.velocity)
            } else {
                0
            };
            let dl = 1.0
                + if options.length > 0.0 {
                    rng.uniform(-options.length, options.length)
                } else {
                    0.0
                };
            with_timing(
                note,
                py_max(0.0, note.start + dt),
                py_max(0.01, note.length * dl),
                (i64::from(note.velocity) + dv).clamp(1, 127),
            )
        })
        .collect()
}

/// Shifts every pitch by `semitones`.
///
/// # Errors
///
/// `INVALID_ARGUMENT`: `transposing note <id> leaves the MIDI range`.
pub fn transpose(notes: &[MidiNote], semitones: i64) -> Result<Vec<MidiNote>> {
    notes
        .iter()
        .map(|note| {
            let pitch = i64::from(note.pitch) + semitones;
            if !(0..=127).contains(&pitch) {
                return Err(DomainError::invalid(format!(
                    "transposing note {} leaves the MIDI range",
                    note.id
                )));
            }
            note.replaced(
                pitch,
                note.start,
                note.length,
                i64::from(note.velocity),
                i64::from(note.channel),
            )
        })
        .collect()
}

/// Options of [`scale_velocity`].
#[derive(Debug, Clone, PartialEq)]
pub struct VelocityScale {
    /// Multiplier of every velocity.
    pub factor: f64,
    /// Added after the multiplication.
    pub offset: i64,
    /// A linear ramp `(from, to)` of the multiplier over `span` beats (crescendo, decrescendo).
    pub curve: Option<(f64, f64)>,
    /// Length of the ramp in beats; the ramp applies only for a non-zero span.
    pub span: Option<f64>,
}

impl Default for VelocityScale {
    fn default() -> Self {
        Self {
            factor: 1.0,
            offset: 0,
            curve: None,
            span: None,
        }
    }
}

/// Scales velocities, optionally along a ramp.
///
/// # Errors
///
/// The note validation errors.
pub fn scale_velocity(notes: &[MidiNote], options: &VelocityScale) -> Result<Vec<MidiNote>> {
    notes
        .iter()
        .map(|note| {
            let mut factor = options.factor;
            if let (Some((from, to)), Some(span)) = (options.curve, options.span)
                && span != 0.0
            {
                let x = py_min(py_max(note.start / span, 0.0), 1.0);
                factor *= from + (to - from) * x;
            }
            let velocity =
                (py_round(f64::from(note.velocity) * factor) + options.offset).clamp(1, 127);
            with_timing(note, note.start, note.length, velocity)
        })
        .collect()
}

/// Moves every note by `beats`, dropping notes that would start before the clip.
///
/// # Errors
///
/// The note validation errors.
pub fn offset(notes: &[MidiNote], beats: f64) -> Result<Vec<MidiNote>> {
    notes
        .iter()
        .filter(|note| note.start + beats >= -EPSILON)
        .map(|note| {
            with_timing(
                note,
                note.start + beats,
                note.length,
                i64::from(note.velocity),
            )
        })
        .collect()
}

/// Extends each note to the next note start (monophonic lines), plus `overlap` beats. Notes are
/// returned ordered by start.
///
/// # Errors
///
/// The note validation errors, for example for an `overlap` that makes a note empty.
pub fn legato(notes: &[MidiNote], overlap: f64) -> Result<Vec<MidiNote>> {
    let mut ordered: Vec<&MidiNote> = notes.iter().collect();
    ordered.sort_by(|a, b| py_cmp(a.start, b.start));
    let mut out = Vec::with_capacity(ordered.len());
    for (i, note) in ordered.iter().enumerate() {
        let mut length = note.length;
        let mut changed = false;
        if let Some(next) = ordered.get(i + 1) {
            let gap = next.start - note.start;
            if gap > EPSILON {
                length = gap + overlap;
                changed = true;
            }
        }
        out.push(if changed {
            with_timing(note, note.start, length, i64::from(note.velocity))?
        } else {
            (*note).clone()
        });
    }
    Ok(out)
}

/// Offsets simultaneous notes (chords) upward by `spread` beats per voice, lowest pitch first.
/// Notes whose starts agree to 6 decimals form one chord; chords keep their first-seen order.
///
/// # Errors
///
/// The note validation errors.
pub fn strum(notes: &[MidiNote], spread: f64) -> Result<Vec<MidiNote>> {
    // Insertion-ordered groups keyed by the rounded start; -0.0 and 0.0 are one key.
    let mut groups: Vec<(f64, Vec<&MidiNote>)> = Vec::new();
    let mut index: HashMap<u64, usize> = HashMap::new();
    for note in notes {
        let start = py_round6(note.start);
        let slot = *index.entry((start + 0.0).to_bits()).or_insert_with(|| {
            groups.push((start, Vec::new()));
            groups.len() - 1
        });
        groups[slot].1.push(note);
    }
    let mut out = Vec::with_capacity(notes.len());
    for (start, mut group) in groups {
        group.sort_by_key(|note| note.pitch);
        for (i, note) in (0_u32..).zip(group) {
            let shift = f64::from(i) * spread;
            out.push(with_timing(
                note,
                start + shift,
                py_max(0.02, note.length - shift),
                i64::from(note.velocity),
            )?);
        }
    }
    Ok(out)
}

/// A Euclidean rhythm: `pulses` onsets spread as evenly as possible over `steps`, the first on
/// step 0 for rotation 0 (`E(3, 8)` is `x..x..x.`), then rotated right by `rotation` steps.
///
/// # Errors
///
/// `INVALID_ARGUMENT`: `euclid needs 0 <= pulses <= steps`.
pub fn euclid(pulses: i64, steps: i64, rotation: i64) -> Result<Vec<bool>> {
    if steps <= 0 || !(0..=steps).contains(&pulses) {
        return Err(DomainError::invalid("euclid needs 0 <= pulses <= steps"));
    }
    let mut pattern: Vec<bool> = (0..steps).map(|i| (i * pulses) % steps < pulses).collect();
    let shift = usize::try_from(rotation.rem_euclid(steps)).unwrap_or(0);
    pattern.rotate_right(shift);
    Ok(pattern)
}

#[cfg(test)]
mod tests {
    use sonara_core::midi::MidiNote;

    use super::{
        HumanizeOptions, QuantizeOptions, VelocityScale, euclid, humanize, legato, offset,
        quantize, scale_velocity, strum, transpose,
    };

    fn note(id: &str, pitch: i64, start: f64, length: f64, velocity: i64) -> MidiNote {
        MidiNote::new(id, pitch, start, length, velocity, 0).expect("valid note")
    }

    #[test]
    fn every_transform_keeps_the_articulation() {
        let notes: Vec<MidiNote> = [(60, 0.1), (64, 0.1), (67, 1.3)]
            .into_iter()
            .enumerate()
            .map(|(i, (pitch, start))| {
                note(&format!("n{i}"), pitch, start, 0.5, 100)
                    .with_articulation(Some("tenuto".to_owned()))
                    .expect("an id")
            })
            .collect();
        let velocity = VelocityScale {
            factor: 0.5,
            offset: 3,
            ..VelocityScale::default()
        };
        let outputs = [
            quantize(&notes, 0.5, &QuantizeOptions::default()).expect("valid"),
            humanize(&notes, &HumanizeOptions::default()).expect("valid"),
            transpose(&notes, 5).expect("valid"),
            scale_velocity(&notes, &velocity).expect("valid"),
            offset(&notes, 1.0).expect("valid"),
            legato(&notes, 0.0).expect("valid"),
            strum(&notes, 0.1).expect("valid"),
        ];
        for out in outputs {
            assert_eq!(out.len(), notes.len());
            assert!(
                out.iter()
                    .all(|n| n.articulation.as_deref() == Some("tenuto")),
                "{out:?}"
            );
        }
    }

    #[test]
    fn quantize_pulls_starts_toward_the_grid() {
        let notes = [note("a", 60, 0.1, 0.5, 100), note("b", 62, 0.9, 0.5, 100)];
        let options = QuantizeOptions {
            strength: 0.5,
            ..QuantizeOptions::default()
        };
        let out = quantize(&notes, 0.5, &options).expect("valid");
        assert!((out[0].start - 0.05).abs() < 1e-12);
        assert!((out[1].start - 0.95).abs() < 1e-12);
        assert_eq!(out[1].id, "b");
        let swung = QuantizeOptions {
            swing: 0.5,
            ..QuantizeOptions::default()
        };
        let out = quantize(&[note("c", 60, 0.52, 0.2, 90)], 0.5, &swung).expect("valid");
        assert!((out[0].start - 0.625).abs() < 1e-12);
    }

    #[test]
    fn quantizing_ends_snaps_the_note_end() {
        let options = QuantizeOptions {
            ends: true,
            ..QuantizeOptions::default()
        };
        let out = quantize(&[note("a", 60, 0.1, 0.7, 90)], 0.5, &options).expect("valid");
        assert!((out[0].start - 0.0).abs() < 1e-12);
        assert!((out[0].length - 1.0).abs() < 1e-12);
    }

    #[test]
    fn quantize_rejects_a_bad_grid() {
        for grid in [0.0, -0.25] {
            let err = quantize(&[], grid, &QuantizeOptions::default()).expect_err("grid");
            assert_eq!(err.message, "quantize grid must be > 0");
        }
    }

    #[test]
    fn humanize_is_reproducible_and_bounded() {
        let notes: Vec<MidiNote> = (0..20)
            .map(|i| note(&format!("n{i}"), 60, f64::from(i), 1.0, 100))
            .collect();
        let options = HumanizeOptions {
            timing: 0.02,
            velocity: 10,
            length: 0.1,
            seed: 5,
        };
        let a = humanize(&notes, &options).expect("valid");
        assert_eq!(a, humanize(&notes, &options).expect("valid"));
        assert!(a.iter().all(|n| (90..=110).contains(&n.velocity)));
        assert!(a.iter().all(|n| (0.9..=1.1).contains(&n.length)));
        assert!(
            a.iter()
                .zip(&notes)
                .any(|(x, y)| x.start.to_bits() != y.start.to_bits())
        );
    }

    #[test]
    fn transpose_checks_the_midi_range() {
        let out = transpose(&[note("a", 60, 0.0, 1.0, 100)], 7).expect("valid");
        assert_eq!(out[0].pitch, 67);
        let err = transpose(&[note("hi", 125, 0.0, 1.0, 100)], 7).expect_err("out of range");
        assert_eq!(err.message, "transposing note hi leaves the MIDI range");
    }

    #[test]
    fn velocity_scaling_rounds_half_to_even_and_clamps() {
        let notes = [note("a", 60, 0.0, 1.0, 101), note("b", 60, 4.0, 1.0, 100)];
        let options = VelocityScale {
            factor: 0.5,
            ..VelocityScale::default()
        };
        // 50.5 rounds to 50 (even), as Python's round().
        assert_eq!(
            scale_velocity(&notes, &options).expect("valid")[0].velocity,
            50
        );
        let ramp = VelocityScale {
            curve: Some((1.0, 0.5)),
            span: Some(4.0),
            ..VelocityScale::default()
        };
        let out = scale_velocity(&notes, &ramp).expect("valid");
        assert_eq!((out[0].velocity, out[1].velocity), (101, 50));
        let loud = VelocityScale {
            factor: 3.0,
            offset: 5,
            ..VelocityScale::default()
        };
        assert_eq!(
            scale_velocity(&notes, &loud).expect("valid")[0].velocity,
            127
        );
    }

    #[test]
    fn offset_drops_notes_that_start_before_zero() {
        let notes = [note("a", 60, 0.5, 1.0, 100), note("b", 60, 2.0, 1.0, 100)];
        let out = offset(&notes, -1.0).expect("valid");
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].id.as_str(), out[0].start), ("b", 1.0));
        assert_eq!(offset(&notes, 2.5).expect("valid")[1].start, 4.5);
    }

    #[test]
    fn legato_extends_notes_to_the_next_start() {
        let notes = [
            note("b", 62, 1.0, 0.25, 100),
            note("a", 60, 0.0, 0.25, 100),
            note("c", 64, 1.0, 0.25, 100),
        ];
        let out = legato(&notes, 0.05).expect("valid");
        let ids: Vec<&str> = out.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"]);
        assert!((out[0].length - 1.05).abs() < 1e-12);
        // Simultaneous notes (gap 0) and the last note keep their length.
        assert!((out[1].length - 0.25).abs() < 1e-12);
        assert!((out[2].length - 0.25).abs() < 1e-12);
    }

    #[test]
    fn strum_staggers_chord_notes_from_the_lowest_pitch() {
        let notes = [
            note("hi", 67, 1.0, 1.0, 100),
            note("lo", 60, 1.0, 1.0, 100),
            note("mid", 64, 1.000_000_1, 1.0, 100),
            note("solo", 72, 3.0, 1.0, 100),
        ];
        let out = strum(&notes, 0.1).expect("valid");
        let summary: Vec<(&str, f64)> = out.iter().map(|n| (n.id.as_str(), n.start)).collect();
        assert_eq!(summary[0].0, "lo");
        assert_eq!(summary[1].0, "mid");
        assert_eq!(summary[2].0, "hi");
        assert!((summary[1].1 - 1.1).abs() < 1e-12);
        assert!((summary[2].1 - 1.2).abs() < 1e-12);
        assert_eq!(summary[3], ("solo", 3.0));
        assert!((out[2].length - 0.8).abs() < 1e-12);
    }

    #[test]
    fn euclid_spreads_pulses_and_rotates() {
        let x = |pattern: &[bool]| -> String {
            pattern
                .iter()
                .map(|&on| if on { 'x' } else { '.' })
                .collect()
        };
        assert_eq!(x(&euclid(3, 8, 0).expect("valid")), "x..x..x.");
        assert_eq!(x(&euclid(5, 8, 2).expect("valid")), ".xx.x.xx");
        assert_eq!(x(&euclid(4, 12, 3).expect("valid")), "x..x..x..x..");
        assert_eq!(x(&euclid(0, 4, 1).expect("valid")), "....");
        assert_eq!(
            x(&euclid(3, 8, -1).expect("valid")),
            x(&euclid(3, 8, 7).expect("valid"))
        );
    }

    #[test]
    fn euclid_rejects_impossible_arguments() {
        for (pulses, steps) in [(1, 0), (-1, 4), (5, 4), (0, -2)] {
            let err = euclid(pulses, steps, 0).expect_err("impossible");
            assert_eq!(err.message, "euclid needs 0 <= pulses <= steps");
        }
    }
}
