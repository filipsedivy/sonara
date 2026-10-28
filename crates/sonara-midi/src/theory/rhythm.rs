//! Rhythm patterns on an integer step grid (SON-P05).
//!
//! A pattern divides time into `steps_per_beat` equal steps and places onsets on that grid, so
//! every position and duration is an exact integer. Conversion to beats (`f64`, the project time
//! base) happens only in [`RhythmPattern::to_beats`]. Patterns are pure values: every transform
//! returns a new pattern and the same input always gives the same output.
//!
//! Limits: onsets do not overlap (monophonic), and there are no tuplets other than those a
//! chosen grid expresses (a grid of 3 gives eighth-note triplets, 6 gives sixteenth triplets).

use sonara_core::error::{DomainError, Result};

/// Largest accepted grid resolution; keeps step arithmetic far from `u32` overflow.
pub const MAX_STEPS_PER_BEAT: u32 = 96;

/// Largest accepted pattern length in steps.
pub const MAX_LENGTH_STEPS: u32 = 65_536;

/// A note onset on the step grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Onset {
    /// First step of the note, counted from the pattern start.
    pub step: u32,
    /// Length in steps (≥ 1).
    pub duration: u32,
}

/// A validated rhythm: a grid, a length and non-overlapping onsets sorted by step.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RhythmPattern {
    steps_per_beat: u32,
    length_steps: u32,
    onsets: Vec<Onset>,
}

impl RhythmPattern {
    /// A pattern from explicit onsets.
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` when the grid or length is zero or above its maximum, an onset has a
    /// zero duration, onsets are unsorted or overlap, or an onset ends after the pattern.
    pub fn new(steps_per_beat: u32, length_steps: u32, onsets: Vec<Onset>) -> Result<Self> {
        if !(1..=MAX_STEPS_PER_BEAT).contains(&steps_per_beat) {
            return Err(DomainError::invalid(format!(
                "steps_per_beat must be 1-{MAX_STEPS_PER_BEAT}, got {steps_per_beat}"
            )));
        }
        if !(1..=MAX_LENGTH_STEPS).contains(&length_steps) {
            return Err(DomainError::invalid(format!(
                "length_steps must be 1-{MAX_LENGTH_STEPS}, got {length_steps}"
            )));
        }
        let mut free_from = 0_u32;
        for (index, onset) in onsets.iter().enumerate() {
            if onset.duration == 0 {
                return Err(DomainError::invalid(format!(
                    "onset {index} has a zero duration"
                )));
            }
            if onset.step < free_from {
                return Err(DomainError::invalid(format!(
                    "onset {index} at step {} starts before the previous onset ends or is out of order",
                    onset.step
                )));
            }
            let end = onset.step.saturating_add(onset.duration);
            if end > length_steps {
                return Err(DomainError::invalid(format!(
                    "onset {index} ends at step {end}, after the pattern length {length_steps}"
                )));
            }
            free_from = end;
        }
        Ok(Self {
            steps_per_beat,
            length_steps,
            onsets,
        })
    }

    /// A pattern from a grid string: `x` starts a note, `-` extends the previous note, `.` is a
    /// rest. Spaces and `|` are ignored, so `"x-.x | x.x."` is two bars of one-beat steps.
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` for another character, a `-` with no note to extend (at the start or
    /// after a rest), an empty grid, or the errors of [`RhythmPattern::new`].
    pub fn from_grid(steps_per_beat: u32, grid: &str) -> Result<Self> {
        let mut onsets: Vec<Onset> = Vec::new();
        let mut sustaining = false;
        let mut length = 0_u32;
        for symbol in grid.chars().filter(|c| !c.is_whitespace() && *c != '|') {
            match symbol {
                'x' | 'X' => {
                    onsets.push(Onset {
                        step: length,
                        duration: 1,
                    });
                    sustaining = true;
                }
                '-' => match onsets.last_mut().filter(|_| sustaining) {
                    Some(last) => last.duration += 1,
                    None => {
                        return Err(DomainError::invalid(format!(
                            "'-' at step {length} has no note to extend"
                        )));
                    }
                },
                '.' => sustaining = false,
                other => {
                    return Err(DomainError::invalid(format!(
                        "unknown grid symbol '{other}' at step {length}"
                    )));
                }
            }
            length += 1;
        }
        Self::new(steps_per_beat, length, onsets)
    }

    /// Steps in one beat.
    #[must_use]
    pub fn steps_per_beat(&self) -> u32 {
        self.steps_per_beat
    }

    /// Pattern length in steps.
    #[must_use]
    pub fn length_steps(&self) -> u32 {
        self.length_steps
    }

    /// The onsets, sorted by step.
    #[must_use]
    pub fn onsets(&self) -> &[Onset] {
        &self.onsets
    }

    /// Pattern length in beats.
    #[must_use]
    pub fn length_beats(&self) -> f64 {
        f64::from(self.length_steps) / f64::from(self.steps_per_beat)
    }

    /// `(start, length)` of every onset in beats from the pattern start.
    #[must_use]
    pub fn to_beats(&self) -> Vec<(f64, f64)> {
        let grid = f64::from(self.steps_per_beat);
        self.onsets
            .iter()
            .map(|o| (f64::from(o.step) / grid, f64::from(o.duration) / grid))
            .collect()
    }

    /// Whether the pattern fills exactly a whole number of bars of `beats_per_bar` beats, in the
    /// project's beat unit (what `TimeSignaturePoint::beats_per_bar` returns). A bar that is not
    /// a whole number of grid steps never matches.
    #[must_use]
    pub fn fills_whole_bars(&self, beats_per_bar: f64) -> bool {
        // Compare in steps: bar steps must be an exact integer for the answer to be exact.
        let bar_steps = beats_per_bar * f64::from(self.steps_per_beat);
        bar_steps >= 1.0
            && bar_steps.fract() == 0.0
            && f64::from(self.length_steps) % bar_steps == 0.0
    }

    /// The pattern shifted later by `steps` (negative: earlier), wrapping around the end. A note
    /// that would cross the wrap point is cut at the end of the pattern, and a note whose start
    /// wraps keeps its duration only up to the pattern end.
    #[must_use]
    pub fn rotated(&self, steps: i64) -> Self {
        let length = i64::from(self.length_steps);
        let mut onsets: Vec<Onset> = self
            .onsets
            .iter()
            .map(|o| {
                let step = (i64::from(o.step) + steps).rem_euclid(length);
                let step = u32::try_from(step).unwrap_or(0);
                Onset {
                    step,
                    duration: o.duration.min(self.length_steps - step),
                }
            })
            .collect();
        onsets.sort_by_key(|o| o.step);
        trim_overlaps(&mut onsets);
        Self {
            steps_per_beat: self.steps_per_beat,
            length_steps: self.length_steps,
            onsets,
        }
    }

    /// The pattern played backwards: each note keeps its duration and ends where it began.
    #[must_use]
    pub fn reversed(&self) -> Self {
        let onsets = self
            .onsets
            .iter()
            .rev()
            .map(|o| Onset {
                step: self.length_steps - o.step - o.duration,
                duration: o.duration,
            })
            .collect();
        Self {
            steps_per_beat: self.steps_per_beat,
            length_steps: self.length_steps,
            onsets,
        }
    }

    /// Every position and duration multiplied by `factor` on the same beat grid (augmentation).
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` for a factor of 0 or a result above [`MAX_LENGTH_STEPS`].
    pub fn augmented(&self, factor: u32) -> Result<Self> {
        if factor == 0 {
            return Err(DomainError::invalid(
                "augmentation factor must be at least 1",
            ));
        }
        let scale = |value: u32| value.checked_mul(factor);
        let onsets: Option<Vec<Onset>> = self
            .onsets
            .iter()
            .map(|o| {
                Some(Onset {
                    step: scale(o.step)?,
                    duration: scale(o.duration)?,
                })
            })
            .collect();
        match (onsets, scale(self.length_steps)) {
            (Some(onsets), Some(length)) => Self::new(self.steps_per_beat, length, onsets),
            _ => Err(DomainError::invalid("augmented pattern is too long")),
        }
    }

    /// Every position and duration divided by `divisor` (diminution).
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` for a divisor of 0, or when any position, duration or the length is
    /// not an exact multiple of it (nothing is rounded).
    pub fn diminished(&self, divisor: u32) -> Result<Self> {
        let exact =
            |value: u32| (divisor != 0 && value.is_multiple_of(divisor)).then(|| value / divisor);
        let onsets: Option<Vec<Onset>> = self
            .onsets
            .iter()
            .map(|o| {
                Some(Onset {
                    step: exact(o.step)?,
                    duration: exact(o.duration)?,
                })
            })
            .collect();
        match (onsets, exact(self.length_steps)) {
            (Some(onsets), Some(length)) => Self::new(self.steps_per_beat, length, onsets),
            _ => Err(DomainError::invalid(format!(
                "pattern does not divide exactly by {divisor}"
            ))),
        }
    }
}

/// Cuts each note at the start of the next so rotation never produces an overlap.
fn trim_overlaps(onsets: &mut [Onset]) {
    for index in 1..onsets.len() {
        let next_step = onsets[index].step;
        if let Some(previous) = onsets.get_mut(index - 1) {
            previous.duration = previous.duration.min(next_step - previous.step).max(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Onset, RhythmPattern};

    fn on(step: u32, duration: u32) -> Onset {
        Onset { step, duration }
    }

    #[test]
    fn grid_strings_parse_to_exact_onsets() {
        let cases = [
            ("x...", vec![on(0, 1)]),
            ("x-.x", vec![on(0, 2), on(3, 1)]),
            ("x--- | x.x.", vec![on(0, 4), on(4, 1), on(6, 1)]),
            ("....", vec![]),
        ];
        for (grid, expected) in cases {
            let pattern = RhythmPattern::from_grid(1, grid).expect(grid);
            assert_eq!(pattern.onsets(), expected.as_slice(), "{grid}");
        }
    }

    #[test]
    fn bad_grids_are_rejected_with_a_reason() {
        for (grid, needle) in [
            ("-x", "no note to extend"),
            ("x.-", "no note to extend"),
            ("x?", "unknown grid symbol"),
            ("", "length_steps"),
        ] {
            let err = RhythmPattern::from_grid(1, grid).expect_err(grid);
            assert!(err.message.contains(needle), "{grid}: {}", err.message);
        }
    }

    #[test]
    fn constructor_rejects_invalid_structure() {
        assert!(RhythmPattern::new(0, 4, vec![]).is_err());
        assert!(RhythmPattern::new(97, 4, vec![]).is_err());
        assert!(RhythmPattern::new(4, 4, vec![on(0, 0)]).is_err());
        assert!(RhythmPattern::new(4, 4, vec![on(0, 3), on(2, 1)]).is_err());
        assert!(RhythmPattern::new(4, 4, vec![on(2, 1), on(0, 1)]).is_err());
        assert!(RhythmPattern::new(4, 4, vec![on(3, 2)]).is_err());
        assert!(RhythmPattern::new(4, 4, vec![on(u32::MAX, 2)]).is_err());
        assert!(RhythmPattern::new(4, 4, vec![on(0, 2), on(2, 2)]).is_ok());
    }

    #[test]
    fn beats_are_exact_on_the_grid() {
        let triplets = RhythmPattern::from_grid(3, "xxx").expect("valid");
        let beats = triplets.to_beats();
        assert_eq!(beats[1], (1.0 / 3.0, 1.0 / 3.0));
        assert!((triplets.length_beats() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn whole_bar_check() {
        let two_bars = RhythmPattern::from_grid(2, "x.......").expect("valid");
        assert!(two_bars.fills_whole_bars(2.0));
        assert!(!two_bars.fills_whole_bars(3.0));
        assert!(
            !two_bars.fills_whole_bars(0.25),
            "a bar of half a step is not on the grid"
        );
        assert!(!two_bars.fills_whole_bars(0.0));
    }

    #[test]
    fn reversal_keeps_durations_and_is_an_involution() {
        let pattern = RhythmPattern::from_grid(1, "x--.x.x-").expect("valid");
        let reversed = pattern.reversed();
        assert_eq!(reversed.onsets(), [on(0, 2), on(3, 1), on(5, 3)].as_slice());
        assert_eq!(reversed.reversed(), pattern);
    }

    #[test]
    fn rotation_wraps_and_inverse_rotation_restores_whole_steps() {
        let pattern = RhythmPattern::from_grid(1, "x.x.").expect("valid");
        assert_eq!(pattern.rotated(1).onsets(), [on(1, 1), on(3, 1)].as_slice());
        assert_eq!(
            pattern.rotated(-1).onsets(),
            [on(1, 1), on(3, 1)].as_slice()
        );
        assert_eq!(pattern.rotated(4), pattern);
        assert_eq!(pattern.rotated(1).rotated(-1), pattern);
        // A held note that would cross the end is cut there.
        let held = RhythmPattern::from_grid(1, "xxx-")
            .expect("valid")
            .rotated(1);
        assert_eq!(held.onsets(), [on(1, 1), on(2, 1), on(3, 1)].as_slice());
    }

    #[test]
    fn augmentation_and_diminution_are_exact() {
        let pattern = RhythmPattern::from_grid(2, "x-.x").expect("valid");
        let doubled = pattern.augmented(2).expect("fits");
        assert_eq!(doubled.length_steps(), 8);
        assert_eq!(doubled.onsets(), [on(0, 4), on(6, 2)].as_slice());
        assert_eq!(doubled.diminished(2).expect("exact"), pattern);
        assert!(
            pattern.diminished(2).is_err(),
            "an odd length does not halve"
        );
        assert!(pattern.diminished(0).is_err());
        assert!(pattern.augmented(0).is_err());
        assert!(pattern.augmented(u32::MAX).is_err());
    }
}
