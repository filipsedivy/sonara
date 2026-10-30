//! The chord layout shared by the family helpers: which pitch each voice of a family plays for one
//! chord, under the spacing rules, with the smoothest motion from the chord before.
//!
//! A layout is searched exhaustively over the chord tones inside each voice's range, from the
//! bottom voice up, and the rules prune the search as it goes:
//!
//! - every voice lies in its range, above the voice below it (no two voices share a pitch) and at
//!   most an octave above it: the bass rarely lies more than an octave below the part above it
//!   (SPC-03) and the upper parts leave no void (SPC-04);
//! - a family that carries the bass puts the chord's bass note in its lowest voice and spaces its
//!   lowest four voices after the harmonic series, each interval no wider than the one below it
//!   (SPC-05);
//! - every chord tone sounds, the fifth left out first and then the upper extensions when there
//!   are fewer voices than tones; only the root, the fifth and the bass note are doubled;
//! - no two voices form an interval below its low limit (SPC-01) or a dissonance below D3
//!   (SPC-02, the stricter grade, since the voices of a helper are one family);
//! - the caller's own check, such as a family's colour rules, accepts the whole layout.
//!
//! Among the layouts that pass, the first is taken by these costs in turn: the fewest voices that
//! leap more than an octave (PLY-13 for the horns), the fewest consecutive octaves between upper
//! voices (SPC-07), the least total motion voice by voice, the fewest doublings of a tone other
//! than the root or the bass, the mean nearest the middle of the voices' ranges, and last the
//! lowest pitches, so equal inputs always give the same layout (TST-04).

use crate::orchestration::rules::{LOW_DISSONANCE, LOW_INTERVAL_LIMITS, Limit};
use crate::orchestration::{InstrumentType, PitchRange};
use crate::theory::Chord;

/// Semitones in an octave.
const OCTAVE: i64 = 12;

/// How many of the lowest voices follow the harmonic series when a family carries the bass:
/// Rimsky-Korsakov's rule is about the bottom of the chord.
const SERIES_VOICES: usize = 4;

/// The semitones modulo the octave of the intervals the low-bass rule calls dissonant: seconds,
/// fourths, the tritone and sevenths; the ninths are seconds an octave up.
const DISSONANT: [i64; 6] = [1, 2, 5, 6, 10, 11];

/// The widest interval the low-bass rule names, a major ninth.
const WIDEST_DISSONANCE: i64 = 14;

/// One voice of a layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Voice {
    /// The pitches the voice may sound.
    pub(super) range: PitchRange,
    /// The instrument that plays it, which the caller's check may read.
    pub(super) colour: InstrumentType,
}

/// What a layout must satisfy besides the voices' ranges.
pub(super) struct Rules<'a> {
    /// Whether the lowest voice is the chord's bass, spaced after the harmonic series.
    pub(super) carries_bass: bool,
    /// The caller's check of a whole layout, pitches from the bottom.
    pub(super) accept: &'a dyn Fn(&[i64]) -> bool,
}

/// The pitch classes of a chord as a layout reads them.
struct Tones {
    /// The chord's bass pitch class: the slash bass, or the root.
    bass: i64,
    /// The root's pitch class.
    root: i64,
    /// The pitch classes every layout must sound.
    required: Vec<i64>,
    /// The pitch classes a layout may sound.
    allowed: Vec<i64>,
    /// The pitch classes a layout may double: the root, the fifth and, when the layout carries
    /// it, the bass.
    doublable: Vec<i64>,
}

impl Tones {
    /// The tones of `chord` for `voices` voices.
    fn of(chord: &Chord, voices: usize, carries_bass: bool) -> Self {
        let root = chord.root.rem_euclid(OCTAVE);
        let bass = chord.bass.unwrap_or(chord.root).rem_euclid(OCTAVE);
        let class = |interval: i64| (root + interval).rem_euclid(OCTAVE);
        let mut tones: Vec<i64> = Vec::new();
        let mut push = |pitch_class: i64| {
            if !tones.contains(&pitch_class) {
                tones.push(pitch_class);
            }
        };
        if carries_bass {
            push(bass);
        }
        push(root);
        // The third, then the seventh, then the extensions from the highest, the fifth last.
        let intervals = &chord.intervals;
        for &interval in intervals.iter().filter(|&&i| i == 3 || i == 4) {
            push(class(interval));
        }
        for &interval in intervals.iter().filter(|&&i| (9..=11).contains(&i)) {
            push(class(interval));
        }
        let mut extensions: Vec<i64> = intervals
            .iter()
            .copied()
            .filter(|&i| ![0, 3, 4, 7, 9, 10, 11].contains(&i))
            .collect();
        extensions.sort_unstable_by(|a, b| b.cmp(a));
        for interval in extensions {
            push(class(interval));
        }
        for &interval in intervals.iter().filter(|&&i| i == 7) {
            push(class(interval));
        }
        let allowed = tones.clone();
        let required: Vec<i64> = tones.into_iter().take(voices).collect();
        let allowed = if voices < allowed.len() {
            required.clone()
        } else {
            allowed
        };
        let mut doublable = vec![root];
        if carries_bass {
            doublable.push(bass);
        }
        if intervals.contains(&7) {
            doublable.push(class(7));
        }
        Self {
            bass,
            root,
            required,
            allowed,
            doublable,
        }
    }
}

/// The cost of a layout, compared in field order; lower is better.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Cost {
    leaps: usize,
    parallels: usize,
    motion: i64,
    doublings: usize,
    drift: i64,
    pitches: Vec<i64>,
}

/// The search for the best layout of one chord.
struct Search<'a> {
    voices: &'a [Voice],
    rules: &'a Rules<'a>,
    tones: Tones,
    previous: Option<&'a [i64]>,
    /// The sum of every voice's lowest and highest pitch: twice the middle of the ranges.
    centre: i64,
    best: Option<Cost>,
}

/// The best layout of `chord` for `voices` (from the bottom) under `rules`, moving as smoothly as
/// the rules allow from `previous` (the layout of the chord before, with as many voices); `None`
/// when no layout passes.
pub(super) fn lay_out(
    chord: &Chord,
    voices: &[Voice],
    rules: &Rules<'_>,
    previous: Option<&[i64]>,
) -> Option<Vec<i64>> {
    if voices.is_empty() {
        return None;
    }
    let centre = voices
        .iter()
        .map(|voice| voice.range.low().get() + voice.range.high().get())
        .sum();
    let mut search = Search {
        voices,
        rules,
        tones: Tones::of(chord, voices.len(), rules.carries_bass),
        previous: previous.filter(|previous| previous.len() == voices.len()),
        centre,
        best: None,
    };
    let mut pitches = Vec::with_capacity(voices.len());
    search.extend(&mut pitches);
    search.best.map(|cost| cost.pitches)
}

impl Search<'_> {
    /// Tries every pitch for the next voice above `pitches`, then recurses.
    fn extend(&mut self, pitches: &mut Vec<i64>) {
        let index = pitches.len();
        let Some(voice) = self.voices.get(index) else {
            self.finish(pitches);
            return;
        };
        let below = pitches.last().copied();
        let low = below.map_or(voice.range.low().get(), |below| {
            voice.range.low().get().max(below + 1)
        });
        let high = below.map_or(voice.range.high().get(), |below| {
            voice.range.high().get().min(below + OCTAVE)
        });
        for pitch in low..=high {
            if self.admits(pitches, pitch) {
                pitches.push(pitch);
                self.extend(pitches);
                pitches.pop();
            }
        }
    }

    /// Whether `pitch` may follow `pitches` as the next voice up.
    fn admits(&self, pitches: &[i64], pitch: i64) -> bool {
        let class = pitch.rem_euclid(OCTAVE);
        if !self.tones.allowed.contains(&class) {
            return false;
        }
        let index = pitches.len();
        if index == 0 && self.rules.carries_bass && class != self.tones.bass {
            return false;
        }
        if !self.tones.doublable.contains(&class)
            && pitches.iter().any(|&p| p.rem_euclid(OCTAVE) == class)
        {
            return false;
        }
        if self.rules.carries_bass
            && (2..SERIES_VOICES).contains(&index)
            && let [.., lower, upper] = pitches
            && pitch - upper > upper - lower
        {
            return false;
        }
        // Enough voices must remain for the tones not yet sounding.
        let missing = self
            .tones
            .required
            .iter()
            .filter(|&&tone| tone != class && pitches.iter().all(|p| p.rem_euclid(OCTAVE) != tone))
            .count();
        missing < self.voices.len() - index
            && pitches.iter().all(|&lower| clears_limits(lower, pitch))
    }

    /// Records the complete layout `pitches` when it passes the remaining checks and costs less
    /// than the best so far.
    fn finish(&mut self, pitches: &[i64]) {
        let sounds = |tone: &i64| pitches.iter().any(|p| p.rem_euclid(OCTAVE) == *tone);
        if !self.tones.required.iter().all(sounds) || !(self.rules.accept)(pitches) {
            return;
        }
        let cost = self.cost(pitches);
        if self.best.as_ref().is_none_or(|best| cost < *best) {
            self.best = Some(cost);
        }
    }

    /// The cost of the layout `pitches`.
    fn cost(&self, pitches: &[i64]) -> Cost {
        let (leaps, parallels, motion) = self.previous.map_or((0, 0, 0), |previous| {
            let moves: Vec<i64> = pitches.iter().zip(previous).map(|(p, q)| p - q).collect();
            let leaps = moves.iter().filter(|step| step.abs() > OCTAVE).count();
            let motion = moves.iter().map(|step| step.abs()).sum();
            let first_upper = usize::from(self.rules.carries_bass);
            let mut parallels = 0;
            for i in first_upper..pitches.len() {
                for j in i + 1..pitches.len() {
                    let (Some(&p_i), Some(&p_j), Some(&q_i), Some(&q_j)) = (
                        pitches.get(i),
                        pitches.get(j),
                        previous.get(i),
                        previous.get(j),
                    ) else {
                        continue;
                    };
                    let (step_i, step_j) = (p_i - q_i, p_j - q_j);
                    if (p_j - p_i) % OCTAVE == 0
                        && (q_j - q_i) % OCTAVE == 0
                        && step_i != 0
                        && step_i.signum() == step_j.signum()
                    {
                        parallels += 1;
                    }
                }
            }
            (leaps, parallels, motion)
        });
        let doublings = pitches
            .iter()
            .enumerate()
            .filter(|&(index, &pitch)| {
                let class = pitch.rem_euclid(OCTAVE);
                class != self.tones.root
                    && class != self.tones.bass
                    && pitches
                        .iter()
                        .take(index)
                        .any(|p| p.rem_euclid(OCTAVE) == class)
            })
            .count();
        let sum: i64 = pitches.iter().sum();
        Cost {
            leaps,
            parallels,
            motion,
            doublings,
            drift: (2 * sum - self.centre).abs(),
            pitches: pitches.to_vec(),
        }
    }
}

/// Whether two notes `lower` and `upper` sounding together clear the low-interval limit of their
/// interval (SPC-01) and, for a dissonance, lie at or above D3 (SPC-02).
pub(super) fn clears_limits(lower: i64, upper: i64) -> bool {
    let semitones = upper - lower;
    let above_limit = u8::try_from(semitones)
        .ok()
        .and_then(|semitones| LOW_INTERVAL_LIMITS.of(semitones))
        .is_none_or(|limit| match limit {
            Limit::Lowest(pitch) => lower >= pitch.get(),
            Limit::Unlimited => true,
        });
    let dissonant =
        semitones <= WIDEST_DISSONANCE && DISSONANT.contains(&semitones.rem_euclid(OCTAVE));
    above_limit && !(dissonant && lower < LOW_DISSONANCE.value.get())
}

#[cfg(test)]
mod tests {
    use super::{Rules, Voice, clears_limits, lay_out};
    use crate::orchestration::{InstrumentType, PitchRange};
    use crate::theory::parse_chord;

    fn voices(count: usize, low: i64, high: i64) -> Vec<Voice> {
        vec![
            Voice {
                range: PitchRange::new(low, high),
                colour: InstrumentType::HornF,
            };
            count
        ]
    }

    fn layout(symbol: &str, voices: &[Voice], bass: bool, previous: Option<&[i64]>) -> Vec<i64> {
        let chord = parse_chord(symbol).expect("a valid chord");
        let rules = Rules {
            carries_bass: bass,
            accept: &|_| true,
        };
        lay_out(&chord, voices, &rules, previous).expect("a layout exists")
    }

    #[test]
    fn a_bass_layout_follows_the_harmonic_series() {
        // TST-10: D minor in four voices over D3 is root, fifth, root and third (7, 5, 3).
        assert_eq!(
            layout("Dm", &voices(4, 41, 72), true, None),
            [50, 57, 62, 65]
        );
        // From that chord, G minor moves every voice up a fourth rather than down a fifth.
        assert_eq!(
            layout("Gm", &voices(4, 41, 72), true, Some(&[50, 57, 62, 65])),
            [55, 62, 67, 70]
        );
    }

    #[test]
    fn fewer_voices_than_tones_leave_out_the_fifth_first() {
        // A dominant ninth in four voices: root, third, seventh and ninth.
        let pitches = layout("G9", &voices(4, 43, 79), true, None);
        let classes: Vec<i64> = pitches.iter().map(|p| p.rem_euclid(12)).collect();
        assert_eq!(classes[0], 7);
        for tone in [7, 11, 5, 9] {
            assert!(classes.contains(&tone), "{pitches:?}");
        }
        assert!(!classes.contains(&2), "{pitches:?}");
    }

    #[test]
    fn a_dissonance_never_lies_below_d3_and_limits_hold() {
        // A seventh over A2 is muddy (SPC-02); a fifth over B♭1 is the lowest clear fifth.
        assert!(!clears_limits(45, 55));
        assert!(clears_limits(50, 60));
        assert!(clears_limits(34, 41));
        assert!(!clears_limits(33, 40));
        // A minor third needs C3 under it (SPC-01), an octave clears anywhere.
        assert!(!clears_limits(47, 50));
        assert!(clears_limits(24, 36));
        // A seventh chord over A2 keeps its seventh well above the bass.
        let pitches = layout("A7", &voices(4, 41, 72), true, None);
        for (index, &lower) in pitches.iter().enumerate() {
            for &upper in &pitches[index + 1..] {
                assert!(clears_limits(lower, upper), "{pitches:?}");
            }
        }
    }

    #[test]
    fn no_layout_is_none() {
        // TST-03: four distinct voices of a triad cannot fit in a fifth.
        let chord = parse_chord("C").expect("a valid chord");
        let rules = Rules {
            carries_bass: true,
            accept: &|_| true,
        };
        assert_eq!(lay_out(&chord, &voices(4, 60, 67), &rules, None), None);
        assert_eq!(lay_out(&chord, &[], &rules, None), None);
        // A check that refuses everything leaves no layout either.
        let refuse = Rules {
            carries_bass: false,
            accept: &|_| false,
        };
        assert_eq!(lay_out(&chord, &voices(3, 48, 84), &refuse, None), None);
    }
}
