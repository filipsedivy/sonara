//! Motifs: a rhythm plus diatonic scale degrees (SON-P05).
//!
//! A motif stores *scale degrees relative to a reference degree*, not MIDI pitches, so it can be
//! realized in any [`Key`] and transformed without a key. Degree `0` is the key's tonic, `1` the
//! next scale degree up, `-1` the one below the tonic; degrees past the end of the scale
//! continue into the next octave (in `C major`, `7` is the C above). Realizing needs an anchor
//! pitch that is a tonic of the key.
//!
//! All transforms are pure and exact (integers only). Inversion mirrors degrees around the
//! first note, which is a *diatonic* inversion: intervals keep their step size, not their
//! semitone size. There is no random variation here.

use sonara_core::error::{DomainError, Result};

use super::rhythm::RhythmPattern;
use crate::theory::Key;

/// One realized note: a MIDI pitch and its position in beats from the motif start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RealizedNote {
    /// MIDI pitch, 0–127.
    pub pitch: u8,
    /// Start in beats.
    pub start: f64,
    /// Length in beats.
    pub length: f64,
}

/// A rhythm with one scale degree per onset.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Motif {
    rhythm: RhythmPattern,
    degrees: Vec<i64>,
}

impl Motif {
    /// A motif; `degrees[i]` is the degree of the `i`-th onset of `rhythm`.
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` when the number of degrees differs from the number of onsets.
    pub fn new(rhythm: RhythmPattern, degrees: Vec<i64>) -> Result<Self> {
        if rhythm.onsets().len() != degrees.len() {
            return Err(DomainError::invalid(format!(
                "motif has {} onsets but {} degrees",
                rhythm.onsets().len(),
                degrees.len()
            )));
        }
        Ok(Self { rhythm, degrees })
    }

    /// The rhythm.
    #[must_use]
    pub fn rhythm(&self) -> &RhythmPattern {
        &self.rhythm
    }

    /// The scale degrees, one per onset.
    #[must_use]
    pub fn degrees(&self) -> &[i64] {
        &self.degrees
    }

    /// The motif moved by `steps` scale degrees (negative: down).
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` when a degree would overflow `i64`.
    pub fn transposed(&self, steps: i64) -> Result<Self> {
        let degrees: Option<Vec<i64>> = self.degrees.iter().map(|d| d.checked_add(steps)).collect();
        let degrees = degrees.ok_or_else(|| DomainError::invalid("degree overflow"))?;
        Ok(Self {
            rhythm: self.rhythm.clone(),
            degrees,
        })
    }

    /// The motif mirrored around its first degree; an empty motif is returned unchanged.
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` when a degree would overflow `i64`.
    pub fn inverted(&self) -> Result<Self> {
        let Some(&axis) = self.degrees.first() else {
            return Ok(self.clone());
        };
        let degrees: Option<Vec<i64>> = self
            .degrees
            .iter()
            .map(|d| axis.checked_mul(2)?.checked_sub(*d))
            .collect();
        let degrees = degrees.ok_or_else(|| DomainError::invalid("degree overflow"))?;
        Ok(Self {
            rhythm: self.rhythm.clone(),
            degrees,
        })
    }

    /// The motif played backwards in both rhythm and pitch.
    #[must_use]
    pub fn retrograde(&self) -> Self {
        Self {
            rhythm: self.rhythm.reversed(),
            degrees: self.degrees.iter().rev().copied().collect(),
        }
    }

    /// The rhythm stretched by `factor`; degrees are unchanged.
    ///
    /// # Errors
    ///
    /// The errors of [`RhythmPattern::augmented`].
    pub fn augmented(&self, factor: u32) -> Result<Self> {
        Ok(Self {
            rhythm: self.rhythm.augmented(factor)?,
            degrees: self.degrees.clone(),
        })
    }

    /// The rhythm compressed by `divisor`; degrees are unchanged.
    ///
    /// # Errors
    ///
    /// The errors of [`RhythmPattern::diminished`].
    pub fn diminished(&self, divisor: u32) -> Result<Self> {
        Ok(Self {
            rhythm: self.rhythm.diminished(divisor)?,
            degrees: self.degrees.clone(),
        })
    }

    /// The motif for the notes `pitches` in `key`, with `anchor` as degree 0.
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` when `anchor` is not a tonic of `key`, a pitch is not in the key
    /// (a motif holds scale degrees only), or the counts differ (see [`Motif::new`]).
    pub fn from_pitches(
        rhythm: RhythmPattern,
        key: &Key,
        anchor: i64,
        pitches: &[i64],
    ) -> Result<Self> {
        check_anchor(key, anchor)?;
        let degrees = pitches
            .iter()
            .map(|&pitch| {
                degree_of(key, anchor, pitch)
                    .ok_or_else(|| DomainError::invalid(format!("pitch {pitch} is not in {key}")))
            })
            .collect::<Result<Vec<_>>>()?;
        Self::new(rhythm, degrees)
    }

    /// The motif as MIDI notes in `key`, with `anchor` as the pitch of degree 0.
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` when `anchor` is not a tonic of `key` or a note leaves 0–127.
    pub fn realize(&self, key: &Key, anchor: i64) -> Result<Vec<RealizedNote>> {
        check_anchor(key, anchor)?;
        self.degrees
            .iter()
            .zip(self.rhythm.to_beats())
            .map(|(&degree, (start, length))| {
                let pitch = pitch_of(key, anchor, degree)
                    .and_then(|p| u8::try_from(p).ok().filter(|p| *p <= 127))
                    .ok_or_else(|| {
                        DomainError::invalid(format!("degree {degree} leaves the MIDI range"))
                    })?;
                Ok(RealizedNote {
                    pitch,
                    start,
                    length,
                })
            })
            .collect()
    }
}

fn check_anchor(key: &Key, anchor: i64) -> Result<()> {
    if (anchor - key.root).rem_euclid(12) == 0 {
        Ok(())
    } else {
        Err(DomainError::invalid(format!(
            "anchor pitch {anchor} is not a tonic of {key}"
        )))
    }
}

/// Pitch of `degree` above `anchor`, or `None` on overflow.
pub(crate) fn pitch_of(key: &Key, anchor: i64, degree: i64) -> Option<i64> {
    let intervals = key.scale.intervals();
    let len = i64::try_from(intervals.len()).ok()?;
    let index = usize::try_from(degree.rem_euclid(len)).ok()?;
    let octaves = degree.div_euclid(len).checked_mul(12)?;
    anchor
        .checked_add(octaves)?
        .checked_add(*intervals.get(index)?)
}

/// Degree of `pitch` relative to `anchor`, or `None` if the pitch is not in the key.
pub(crate) fn degree_of(key: &Key, anchor: i64, pitch: i64) -> Option<i64> {
    let intervals = key.scale.intervals();
    let len = i64::try_from(intervals.len()).ok()?;
    let offset = pitch.checked_sub(anchor)?;
    let index = intervals.iter().position(|&i| i == offset.rem_euclid(12))?;
    offset
        .div_euclid(12)
        .checked_mul(len)?
        .checked_add(i64::try_from(index).ok()?)
}

#[cfg(test)]
mod tests {
    use super::{Motif, RealizedNote};
    use crate::theory::rhythm::RhythmPattern;
    use crate::theory::{Key, parse_key};
    use proptest::prelude::*;

    fn key(text: &str) -> Key {
        parse_key(text).expect("valid key")
    }

    fn motif(grid: &str, degrees: &[i64]) -> Motif {
        Motif::new(
            RhythmPattern::from_grid(1, grid).expect("valid grid"),
            degrees.to_vec(),
        )
        .expect("matching counts")
    }

    fn pitches(motif: &Motif, key: &Key, anchor: i64) -> Vec<u8> {
        motif
            .realize(key, anchor)
            .expect("in range")
            .iter()
            .map(|n| n.pitch)
            .collect()
    }

    #[test]
    fn degrees_realize_in_major_across_octaves() {
        let m = motif("xxxxxxxx", &[0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(
            pitches(&m, &key("C major"), 60),
            [60, 62, 64, 65, 67, 69, 71, 72]
        );
        let below = motif("xxx", &[-1, -2, -7]);
        assert_eq!(pitches(&below, &key("C major"), 60), [59, 57, 48]);
    }

    #[test]
    fn minor_and_pentatonic_follow_their_scales() {
        let m = motif("xxxxx", &[0, 1, 2, 3, 4]);
        assert_eq!(pitches(&m, &key("A minor"), 57), [57, 59, 60, 62, 64]);
        assert_eq!(
            pitches(&m, &key("C pentatonic major"), 60),
            [60, 62, 64, 67, 69]
        );
    }

    #[test]
    fn realization_reports_beats_from_the_rhythm() {
        let m = motif("x-.x", &[0, 2]);
        assert_eq!(
            m.realize(&key("C major"), 60).expect("in range"),
            [
                RealizedNote {
                    pitch: 60,
                    start: 0.0,
                    length: 2.0
                },
                RealizedNote {
                    pitch: 64,
                    start: 3.0,
                    length: 1.0
                },
            ]
        );
    }

    #[test]
    fn transforms_match_hand_worked_examples() {
        let m = motif("xxx", &[0, 2, 1]);
        assert_eq!(m.transposed(3).expect("fits").degrees(), [3, 5, 4]);
        assert_eq!(m.inverted().expect("fits").degrees(), [0, -2, -1]);
        assert_eq!(m.retrograde().degrees(), [1, 2, 0]);
        let long = motif("x-x.", &[0, 1]);
        let back = long.retrograde();
        assert_eq!(back.rhythm().onsets()[0].step, 1);
        assert_eq!(back.degrees(), [1, 0]);
        assert_eq!(long.augmented(2).expect("fits").rhythm().length_steps(), 8);
        assert_eq!(
            long.augmented(2)
                .expect("fits")
                .diminished(2)
                .expect("exact"),
            long
        );
    }

    #[test]
    fn invalid_motifs_and_realizations_are_rejected() {
        let rhythm = RhythmPattern::from_grid(1, "xx").expect("valid");
        assert!(Motif::new(rhythm.clone(), vec![0]).is_err());
        let m = motif("x", &[0]);
        assert!(m.realize(&key("C major"), 61).is_err(), "61 is not a tonic");
        let high = motif("x", &[7 * 10]);
        assert!(high.realize(&key("C major"), 60).is_err(), "above 127");
        assert!(motif("x", &[i64::MAX]).transposed(1).is_err());
        assert!(
            motif("x", &[i64::MAX])
                .realize(&key("C major"), 60)
                .is_err()
        );
        assert!(Motif::from_pitches(rhythm.clone(), &key("C major"), 60, &[60, 61]).is_err());
        assert!(Motif::from_pitches(rhythm, &key("C major"), 61, &[60, 62]).is_err());
    }

    #[test]
    fn from_pitches_inverts_realize_for_in_key_notes() {
        let rhythm = RhythmPattern::from_grid(1, "xxxx").expect("valid");
        let k = key("D dorian");
        let m = Motif::from_pitches(rhythm, &k, 62, &[62, 65, 57, 74]).expect("in key");
        assert_eq!(m.degrees(), [0, 2, -3, 7]);
        assert_eq!(pitches(&m, &k, 62), [62, 65, 57, 74]);
    }

    proptest! {
        #[test]
        fn retrograde_twice_is_identity(degrees in proptest::collection::vec(-20_i64..20, 1..8)) {
            let grid: String = "x".repeat(degrees.len());
            let m = motif(&grid, &degrees);
            prop_assert_eq!(m.retrograde().retrograde(), m);
        }

        #[test]
        fn transposition_round_trips_and_shifts_realized_pitch_by_scale_steps(
            degrees in proptest::collection::vec(-7_i64..14, 1..8),
            steps in -14_i64..14,
        ) {
            let grid: String = "x".repeat(degrees.len());
            let m = motif(&grid, &degrees);
            prop_assert_eq!(m.transposed(steps).expect("fits").transposed(-steps).expect("fits"), m.clone());
            let k = key("G mixolydian");
            let moved = m.transposed(steps).expect("fits");
            for (a, b) in m.degrees().iter().zip(moved.degrees()) {
                let pa = super::pitch_of(&k, 67, *a).expect("no overflow");
                let pb = super::pitch_of(&k, 67, *b).expect("no overflow");
                prop_assert!(k.contains(pa) && k.contains(pb));
                prop_assert_eq!(super::degree_of(&k, 67, pb), Some(*b));
            }
        }

        #[test]
        fn inversion_twice_is_identity(degrees in proptest::collection::vec(-20_i64..20, 1..8)) {
            let grid: String = "x".repeat(degrees.len());
            let m = motif(&grid, &degrees);
            prop_assert_eq!(m.inverted().expect("fits").inverted().expect("fits"), m);
        }
    }
}
