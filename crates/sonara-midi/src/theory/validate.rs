//! Validators for pitches, keys, chords, motifs and rhythms (SON-P05).
//!
//! Every validator takes explicit input, never mutates it and returns a [`Report`]. Findings
//! come in two severities:
//!
//! - [`Severity::Error`]: the data is malformed (a pitch outside 0–127, a pitch class outside
//!   0–11, an empty chord). Callers should refuse it.
//! - [`Severity::Advisory`]: valid, but worth a look (a note outside the key, a note that is not
//!   a chord tone, a large leap). These are heuristics of common-practice theory, not rules:
//!   chromatic notes and wide leaps are legitimate, so an advisory never makes a report invalid.
//!
//! Positions in findings are zero-based indices into the checked slice.

use super::motif::{Motif, pitch_of};
use super::rhythm::RhythmPattern;
use crate::theory::{Chord, Key};

/// Highest MIDI pitch.
const MAX_PITCH: i64 = 127;

/// A leap above this many semitones (an octave) is reported as an advisory.
pub const LARGE_LEAP_SEMITONES: i64 = 12;

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    /// Malformed data.
    Error,
    /// Valid but unusual.
    Advisory,
}

/// What a finding is about; stable for callers that react to a kind of finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FindingKind {
    /// A pitch outside 0–127.
    PitchOutOfRange,
    /// A pitch class outside 0–11.
    PitchClassOutOfRange,
    /// A chord with no intervals, or whose intervals are negative or repeated.
    MalformedChord,
    /// A note outside the key's scale.
    OutOfKey,
    /// A note whose pitch class is not in the chord.
    NotChordTone,
    /// A melodic leap larger than an octave.
    LargeLeap,
    /// A rhythm that does not fill a whole number of bars.
    PartialBar,
}

/// One finding of a validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Error or advisory.
    pub severity: Severity,
    /// The kind of finding.
    pub kind: FindingKind,
    /// Index of the offending element in the checked input, if it has one.
    pub index: Option<usize>,
    /// A message naming the value and the rule.
    pub message: String,
}

/// The findings of one validation, in input order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Every finding.
    pub findings: Vec<Finding>,
}

impl Report {
    /// Whether there is no [`Severity::Error`] finding; advisories do not count.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.findings.iter().any(|f| f.severity == Severity::Error)
    }

    /// The findings of the given severity.
    pub fn of(&self, severity: Severity) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(move |f| f.severity == severity)
    }

    fn push(
        &mut self,
        severity: Severity,
        kind: FindingKind,
        index: Option<usize>,
        message: String,
    ) {
        self.findings.push(Finding {
            severity,
            kind,
            index,
            message,
        });
    }
}

/// Structural check: every pitch is a MIDI note number, 0–127.
#[must_use]
pub fn validate_pitches(pitches: &[i64]) -> Report {
    let mut report = Report::default();
    for (index, &pitch) in pitches.iter().enumerate() {
        if !(0..=MAX_PITCH).contains(&pitch) {
            report.push(
                Severity::Error,
                FindingKind::PitchOutOfRange,
                Some(index),
                format!("pitch {pitch} is outside 0-127"),
            );
        }
    }
    report
}

/// Structural check: the key's tonic is a pitch class, 0–11.
#[must_use]
pub fn validate_key(key: &Key) -> Report {
    let mut report = Report::default();
    if !(0..12).contains(&key.root) {
        report.push(
            Severity::Error,
            FindingKind::PitchClassOutOfRange,
            None,
            format!("key tonic {} is outside 0-11", key.root),
        );
    }
    report
}

/// Structural check of a chord: root and bass are pitch classes; the intervals are non-empty,
/// non-negative and distinct. Any chord shape that passes is accepted (not only triads), and
/// intervals need not be sorted or start at 0.
#[must_use]
pub fn validate_chord(chord: &Chord) -> Report {
    let mut report = Report::default();
    for (name, value) in [("root", Some(chord.root)), ("bass", chord.bass)] {
        if let Some(value) = value.filter(|v| !(0..12).contains(v)) {
            report.push(
                Severity::Error,
                FindingKind::PitchClassOutOfRange,
                None,
                format!("chord {name} {value} is outside 0-11"),
            );
        }
    }
    if chord.intervals.is_empty() {
        report.push(
            Severity::Error,
            FindingKind::MalformedChord,
            None,
            "chord has no intervals".to_owned(),
        );
    }
    for (index, &interval) in chord.intervals.iter().enumerate() {
        if interval < 0 {
            report.push(
                Severity::Error,
                FindingKind::MalformedChord,
                Some(index),
                format!("chord interval {interval} is negative"),
            );
        } else if chord.intervals[..index].contains(&interval) {
            report.push(
                Severity::Error,
                FindingKind::MalformedChord,
                Some(index),
                format!("chord interval {interval} is repeated"),
            );
        }
    }
    report
}

/// Advisory check: each pitch that is not in the key's scale. Out-of-key notes are often
/// intentional (chromaticism, borrowed chords), so they are never errors.
#[must_use]
pub fn check_in_key(key: &Key, pitches: &[i64]) -> Report {
    let mut report = Report::default();
    for (index, &pitch) in pitches.iter().enumerate() {
        if !key.contains(pitch) {
            report.push(
                Severity::Advisory,
                FindingKind::OutOfKey,
                Some(index),
                format!("pitch {pitch} is not in {key}"),
            );
        }
    }
    report
}

/// Advisory check: each pitch whose pitch class is not a tone of `chord` (a slash bass counts
/// as a chord tone). Passing tones and extensions outside the chord are normal.
#[must_use]
pub fn check_chord_tones(chord: &Chord, pitches: &[i64]) -> Report {
    let mut tones = chord.pitch_classes();
    tones.extend(chord.bass);
    let mut report = Report::default();
    for (index, &pitch) in pitches.iter().enumerate() {
        if !tones.contains(&pitch.rem_euclid(12)) {
            report.push(
                Severity::Advisory,
                FindingKind::NotChordTone,
                Some(index),
                format!("pitch {pitch} is not a tone of {}", chord.symbol),
            );
        }
    }
    report
}

/// Advisory check: a rhythm that does not fill a whole number of bars of `beats_per_bar` beats.
#[must_use]
pub fn check_rhythm_bars(pattern: &RhythmPattern, beats_per_bar: f64) -> Report {
    let mut report = Report::default();
    if !pattern.fills_whole_bars(beats_per_bar) {
        report.push(
            Severity::Advisory,
            FindingKind::PartialBar,
            None,
            format!(
                "rhythm of {} beats is not a whole number of {beats_per_bar}-beat bars",
                pattern.length_beats()
            ),
        );
    }
    report
}

/// Checks a motif realized in `key` from `anchor`: a note outside 0–127 or an anchor that is
/// not a tonic is an error (and ends the check); a leap above [`LARGE_LEAP_SEMITONES`] between
/// consecutive notes is an advisory.
#[must_use]
pub fn check_motif(motif: &Motif, key: &Key, anchor: i64) -> Report {
    let mut report = Report::default();
    let pitches: Option<Vec<i64>> = motif
        .degrees()
        .iter()
        .map(|&d| pitch_of(key, anchor, d))
        .collect();
    let pitches = pitches.unwrap_or_default();
    if (anchor - key.root).rem_euclid(12) != 0 || pitches.len() != motif.degrees().len() {
        report.push(
            Severity::Error,
            FindingKind::PitchOutOfRange,
            None,
            format!("anchor {anchor} is not a tonic of {key} or a degree overflows"),
        );
        return report;
    }
    report.findings.extend(validate_pitches(&pitches).findings);
    for (index, pair) in pitches.windows(2).enumerate() {
        if let [a, b] = pair
            && (b - a).abs() > LARGE_LEAP_SEMITONES
        {
            report.push(
                Severity::Advisory,
                FindingKind::LargeLeap,
                Some(index + 1),
                format!(
                    "leap of {} semitones into note {}",
                    (b - a).abs(),
                    index + 1
                ),
            );
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::{
        FindingKind, Severity, check_chord_tones, check_in_key, check_motif, check_rhythm_bars,
        validate_chord, validate_key, validate_pitches,
    };
    use crate::theory::motif::Motif;
    use crate::theory::rhythm::RhythmPattern;
    use crate::theory::{Chord, Key, parse_chord, parse_key};

    fn key(text: &str) -> Key {
        parse_key(text).expect("valid key")
    }

    fn kinds(report: &super::Report) -> Vec<(FindingKind, Option<usize>)> {
        report.findings.iter().map(|f| (f.kind, f.index)).collect()
    }

    #[test]
    fn pitch_boundaries_are_0_and_127() {
        assert_eq!(kinds(&validate_pitches(&[0, 60, 127])), []);
        let report = validate_pitches(&[-1, 60, 128]);
        assert!(!report.is_valid());
        assert_eq!(
            kinds(&report),
            [
                (FindingKind::PitchOutOfRange, Some(0)),
                (FindingKind::PitchOutOfRange, Some(2))
            ]
        );
        assert!(report.findings[0].message.contains("-1"));
    }

    #[test]
    fn out_of_key_notes_are_advisory_not_errors() {
        let report = check_in_key(&key("C major"), &[60, 61, 64, 66]);
        assert!(report.is_valid(), "a chromatic note does not invalidate");
        assert_eq!(
            kinds(&report),
            [
                (FindingKind::OutOfKey, Some(1)),
                (FindingKind::OutOfKey, Some(3))
            ]
        );
        assert!(report.of(Severity::Error).next().is_none());
        assert_eq!(report.of(Severity::Advisory).count(), 2);
        assert_eq!(kinds(&check_in_key(&key("C blues"), &[63, 66])), []);
    }

    #[test]
    fn chord_tones_use_pitch_classes_and_slash_bass() {
        let g_over_b = parse_chord("G/B").expect("valid");
        // 71 is B (the slash bass), 62 is D (fifth), 60 is C (not a tone).
        let report = check_chord_tones(&g_over_b, &[71, 62, 60, 55 + 12]);
        assert_eq!(kinds(&report), [(FindingKind::NotChordTone, Some(2))]);
        assert!(report.is_valid());
    }

    #[test]
    fn chord_structure_is_checked_but_shapes_are_not_restricted() {
        for symbol in ["C", "Gsus4", "C5", "C13", "F#m7b5"] {
            let chord = parse_chord(symbol).expect(symbol);
            assert_eq!(kinds(&validate_chord(&chord)), [], "{symbol}");
        }
        let bad = Chord {
            root: 12,
            intervals: vec![],
            bass: Some(-1),
            symbol: "?".into(),
        };
        let report = validate_chord(&bad);
        assert!(!report.is_valid());
        assert_eq!(report.findings.len(), 3);
        let repeated = Chord {
            root: 0,
            intervals: vec![0, 4, 4, -2],
            bass: None,
            symbol: "?".into(),
        };
        assert_eq!(
            kinds(&validate_chord(&repeated)),
            [
                (FindingKind::MalformedChord, Some(2)),
                (FindingKind::MalformedChord, Some(3))
            ]
        );
    }

    #[test]
    fn key_tonic_must_be_a_pitch_class() {
        assert!(validate_key(&key("F# minor")).is_valid());
        let bad = Key {
            root: 12,
            scale: key("C major").scale,
        };
        assert_eq!(
            kinds(&validate_key(&bad)),
            [(FindingKind::PitchClassOutOfRange, None)]
        );
    }

    #[test]
    fn partial_bars_are_advisory() {
        let pattern = RhythmPattern::from_grid(1, "x.x.x").expect("valid");
        let report = check_rhythm_bars(&pattern, 4.0);
        assert!(report.is_valid());
        assert_eq!(kinds(&report), [(FindingKind::PartialBar, None)]);
        assert_eq!(kinds(&check_rhythm_bars(&pattern, 5.0)), []);
    }

    #[test]
    fn motif_checks_range_and_leaps() {
        let rhythm = RhythmPattern::from_grid(1, "xxx").expect("valid");
        let c = key("C major");
        let ok = Motif::new(rhythm.clone(), vec![0, 2, 1]).expect("counts");
        assert_eq!(kinds(&check_motif(&ok, &c, 60)), []);
        let leap = Motif::new(rhythm.clone(), vec![0, 9, 8]).expect("counts");
        let report = check_motif(&leap, &c, 60);
        assert!(report.is_valid());
        assert_eq!(kinds(&report), [(FindingKind::LargeLeap, Some(1))]);
        let high = Motif::new(rhythm.clone(), vec![0, 70, 1]).expect("counts");
        assert!(!check_motif(&high, &c, 60).is_valid());
        assert!(
            !check_motif(&ok, &c, 61).is_valid(),
            "anchor must be a tonic"
        );
    }

    #[test]
    fn validators_do_not_mutate_and_are_deterministic() {
        let pitches = vec![61, 62];
        let c = key("C major");
        let first = check_in_key(&c, &pitches);
        assert_eq!(first, check_in_key(&c, &pitches));
        assert_eq!(pitches, [61, 62]);
    }
}
