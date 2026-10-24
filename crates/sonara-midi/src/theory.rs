//! Music theory helpers: keys, scales, chords, Roman numerals and voice leading
//! (`sonara.midi.theory`).
//!
//! Pitches and pitch classes are `i64` so that out-of-range results of the arithmetic reach the
//! note validation, which reports them with the prototype's messages.

use std::fmt;

use sonara_core::error::{DomainError, Result};
use sonara_core::pyfmt::str_repr;

use crate::num::{is_py_space, py_strip, to_f64, to_f64_len};

/// A scale of the prototype's `SCALES` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scale {
    /// `major`
    Major,
    /// `minor`
    Minor,
    /// `harmonic_minor`
    HarmonicMinor,
    /// `melodic_minor`
    MelodicMinor,
    /// `dorian`
    Dorian,
    /// `phrygian`
    Phrygian,
    /// `lydian`
    Lydian,
    /// `mixolydian`
    Mixolydian,
    /// `locrian`
    Locrian,
    /// `pentatonic_major`
    PentatonicMajor,
    /// `pentatonic_minor`
    PentatonicMinor,
    /// `blues`
    Blues,
    /// `whole_tone`
    WholeTone,
    /// `chromatic`
    Chromatic,
}

impl Scale {
    /// Every scale.
    pub const ALL: [Self; 14] = [
        Self::Major,
        Self::Minor,
        Self::HarmonicMinor,
        Self::MelodicMinor,
        Self::Dorian,
        Self::Phrygian,
        Self::Lydian,
        Self::Mixolydian,
        Self::Locrian,
        Self::PentatonicMajor,
        Self::PentatonicMinor,
        Self::Blues,
        Self::WholeTone,
        Self::Chromatic,
    ];

    /// The scale's name as the prototype spells it, for example `"harmonic_minor"`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Major => "major",
            Self::Minor => "minor",
            Self::HarmonicMinor => "harmonic_minor",
            Self::MelodicMinor => "melodic_minor",
            Self::Dorian => "dorian",
            Self::Phrygian => "phrygian",
            Self::Lydian => "lydian",
            Self::Mixolydian => "mixolydian",
            Self::Locrian => "locrian",
            Self::PentatonicMajor => "pentatonic_major",
            Self::PentatonicMinor => "pentatonic_minor",
            Self::Blues => "blues",
            Self::WholeTone => "whole_tone",
            Self::Chromatic => "chromatic",
        }
    }

    /// The scale with exactly this name (no aliases), or `None`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|scale| scale.name() == name)
    }

    /// Semitone offsets from the root, ascending.
    #[must_use]
    pub const fn intervals(self) -> &'static [i64] {
        match self {
            Self::Major => &[0, 2, 4, 5, 7, 9, 11],
            Self::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Self::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            Self::MelodicMinor => &[0, 2, 3, 5, 7, 9, 11],
            Self::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Self::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Self::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            Self::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            Self::Locrian => &[0, 1, 3, 5, 6, 8, 10],
            Self::PentatonicMajor => &[0, 2, 4, 7, 9],
            Self::PentatonicMinor => &[0, 3, 5, 7, 10],
            Self::Blues => &[0, 3, 5, 6, 7, 10],
            Self::WholeTone => &[0, 2, 4, 6, 8, 10],
            Self::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        }
    }
}

/// A tonic and a scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    /// Pitch class of the tonic, 0–11.
    pub root: i64,
    /// The scale.
    pub scale: Scale,
}

impl Key {
    /// Pitch class of the 1-based scale `degree`; degrees wrap around the octave.
    #[must_use]
    pub fn degree_pc(&self, degree: i64) -> i64 {
        let intervals = self.scale.intervals();
        let len = i64::try_from(intervals.len()).unwrap_or(i64::MAX);
        let index = usize::try_from((degree - 1).rem_euclid(len)).unwrap_or(0);
        (self.root + intervals[index]).rem_euclid(12)
    }

    /// Whether `pitch` belongs to the scale.
    #[must_use]
    pub fn contains(&self, pitch: i64) -> bool {
        self.scale
            .intervals()
            .contains(&(pitch - self.root).rem_euclid(12))
    }

    /// The pitches of the scale from `low` to `high`, both included, ascending.
    #[must_use]
    pub fn pitches(&self, low: i64, high: i64) -> Vec<i64> {
        (low..=high).filter(|&pitch| self.contains(pitch)).collect()
    }

    /// The nearest in-key pitch; a tie resolves downward.
    #[must_use]
    pub fn snap(&self, pitch: i64) -> i64 {
        for distance in 0..12 {
            if self.contains(pitch - distance) {
                return pitch - distance;
            }
            if self.contains(pitch + distance) {
                return pitch + distance;
            }
        }
        pitch
    }
}

impl fmt::Display for Key {
    /// `"C major"`, `"Eb minor"`: the tonic spelled with flats, then the scale name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const NAMES: [&str; 12] = [
            "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
        ];
        let name = usize::try_from(self.root.rem_euclid(12)).map_or("C", |i| NAMES[i]);
        write!(f, "{name} {}", self.scale.name())
    }
}

/// Pitch class of a note letter (`A`–`G`, either case) and an optional `#` or `b`, the way the
/// prototype's `PITCH_CLASSES` table spells it (`Cb` is 11, `B#` is 0).
fn pitch_class(letter: char, accidental: Option<char>) -> i64 {
    let natural: i64 = match letter.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        _ => 11,
    };
    let shift = match accidental {
        Some('#') => 1,
        Some('b') => -1,
        _ => 0,
    };
    (natural + shift).rem_euclid(12)
}

fn is_note_letter(c: char) -> bool {
    matches!(c, 'A'..='G' | 'a'..='g')
}

fn is_accidental(c: char) -> bool {
    matches!(c, '#' | 'b')
}

/// Parses `"C major"`, `"F# minor"`, `"Am"`, `"D dorian"` or `"Bb"`. The scale name is
/// case-insensitive, spaces and dashes act as underscores, and `ionian`, `aeolian`, `maj`, `min`
/// and `m` are aliases; no scale means major.
///
/// # Errors
///
/// `INVALID_ARGUMENT` with `cannot parse key '…'` when the text does not start with a note, and
/// `unknown scale '…'` (with the sorted `available` scale names) for an unknown scale.
pub fn parse_key(text: &str) -> Result<Key> {
    let stripped = py_strip(text);
    let mut chars = stripped.chars();
    let letter = chars
        .next()
        .filter(|&c| is_note_letter(c))
        .ok_or_else(|| DomainError::invalid(format!("cannot parse key {}", str_repr(text))))?;
    let mut rest = chars.as_str();
    let accidental = rest.chars().next().filter(|&c| is_accidental(c));
    if let Some(accidental) = accidental {
        rest = &rest[accidental.len_utf8()..];
    }
    // The scale part may not contain a line break, except as leading white space (`\s*(.*)$`).
    let rest = rest.trim_start_matches(is_py_space);
    if rest.contains('\n') {
        return Err(DomainError::invalid(format!(
            "cannot parse key {}",
            str_repr(text)
        )));
    }
    let name = py_strip(rest).to_lowercase().replace([' ', '-'], "_");
    let scale = match name.as_str() {
        "" | "major" | "maj" | "ionian" => Scale::Major,
        "minor" | "min" | "m" | "aeolian" => Scale::Minor,
        other => Scale::from_name(other).ok_or_else(|| {
            let mut available: Vec<&str> = Scale::ALL.iter().map(|s| s.name()).collect();
            available.sort_unstable();
            DomainError::invalid(format!("unknown scale {}", str_repr(other)))
                .with_detail("available", available)
        })?,
    };
    Ok(Key {
        root: pitch_class(letter, accidental),
        scale,
    })
}

/// A chord: a root, its intervals and an optional slash bass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chord {
    /// Pitch class of the root, 0–11.
    pub root: i64,
    /// Semitone offsets from the root; may exceed an octave for extensions (14 is the ninth).
    pub intervals: Vec<i64>,
    /// Pitch class of a slash bass (`G/B`), if any.
    pub bass: Option<i64>,
    /// The text the chord was parsed from.
    pub symbol: String,
}

impl Chord {
    /// Pitch class of every chord tone, in interval order.
    #[must_use]
    pub fn pitch_classes(&self) -> Vec<i64> {
        self.intervals
            .iter()
            .map(|interval| (self.root + interval).rem_euclid(12))
            .collect()
    }
}

/// Chord qualities of the prototype's `CHORD_QUALITIES` table, in its order.
const CHORD_QUALITIES: [(&str, &[i64]); 26] = [
    ("", &[0, 4, 7]),
    ("maj", &[0, 4, 7]),
    ("m", &[0, 3, 7]),
    ("min", &[0, 3, 7]),
    ("dim", &[0, 3, 6]),
    ("aug", &[0, 4, 8]),
    ("sus2", &[0, 2, 7]),
    ("sus4", &[0, 5, 7]),
    ("5", &[0, 7]),
    ("6", &[0, 4, 7, 9]),
    ("m6", &[0, 3, 7, 9]),
    ("7", &[0, 4, 7, 10]),
    ("maj7", &[0, 4, 7, 11]),
    ("m7", &[0, 3, 7, 10]),
    ("mmaj7", &[0, 3, 7, 11]),
    ("m7b5", &[0, 3, 6, 10]),
    ("dim7", &[0, 3, 6, 9]),
    ("add9", &[0, 4, 7, 14]),
    ("madd9", &[0, 3, 7, 14]),
    ("9", &[0, 4, 7, 10, 14]),
    ("maj9", &[0, 4, 7, 11, 14]),
    ("m9", &[0, 3, 7, 10, 14]),
    ("7sus4", &[0, 5, 7, 10]),
    ("11", &[0, 4, 7, 10, 14, 17]),
    ("m11", &[0, 3, 7, 10, 14, 17]),
    ("13", &[0, 4, 7, 10, 14, 21]),
];

/// The intervals of a chord quality such as `"m7b5"`, or `None` for an unknown quality.
#[must_use]
pub fn chord_quality(name: &str) -> Option<&'static [i64]> {
    CHORD_QUALITIES
        .iter()
        .find(|(quality, _)| *quality == name)
        .map(|&(_, intervals)| intervals)
}

/// Parses chord symbols such as `C`, `Am7`, `F#m7b5`, `Bbmaj9` or the slash chord `G/B`.
///
/// # Errors
///
/// `INVALID_ARGUMENT` with `cannot parse chord '…'` for malformed text, and
/// `unknown chord quality '…' in '…'` (with the sorted `available` qualities) for an unknown
/// quality.
pub fn parse_chord(symbol: &str) -> Result<Chord> {
    let malformed = || DomainError::invalid(format!("cannot parse chord {}", str_repr(symbol)));
    let stripped = py_strip(symbol);
    let mut chars = stripped.chars();
    let letter = chars
        .next()
        .filter(|&c| is_note_letter(c))
        .ok_or_else(malformed)?;
    let mut rest = chars.as_str();
    let accidental = rest.chars().next().filter(|&c| is_accidental(c));
    if let Some(accidental) = accidental {
        rest = &rest[accidental.len_utf8()..];
    }
    // `([^/]*)(?:/([A-Ga-g][#b]?))?$`: the quality runs up to the first slash, and what follows
    // the slash must be exactly a note.
    let (quality, bass) = match rest.split_once('/') {
        None => (rest, None),
        Some((quality, bass_text)) => {
            let mut bass_chars = bass_text.chars();
            let bass_letter = bass_chars
                .next()
                .filter(|&c| is_note_letter(c))
                .ok_or_else(malformed)?;
            let bass_accidental = match bass_chars.as_str() {
                "" => None,
                tail => {
                    let mut tail_chars = tail.chars();
                    match (tail_chars.next(), tail_chars.next()) {
                        (Some(c), None) if is_accidental(c) => Some(c),
                        _ => return Err(malformed()),
                    }
                }
            };
            (quality, Some(pitch_class(bass_letter, bass_accidental)))
        }
    };
    let intervals = chord_quality(quality).ok_or_else(|| {
        let mut available: Vec<&str> = CHORD_QUALITIES
            .iter()
            .map(|&(name, _)| name)
            .filter(|name| !name.is_empty())
            .collect();
        available.sort_unstable();
        DomainError::invalid(format!(
            "unknown chord quality {} in {}",
            str_repr(quality),
            str_repr(symbol)
        ))
        .with_detail("available", available)
    })?;
    Ok(Chord {
        root: pitch_class(letter, accidental),
        intervals: intervals.to_vec(),
        bass,
        symbol: stripped.to_owned(),
    })
}

/// 1-based degree of a lower-case Roman numeral `i`–`vii`.
fn roman_degree(numeral: &str) -> Option<i64> {
    match numeral {
        "i" => Some(1),
        "ii" => Some(2),
        "iii" => Some(3),
        "iv" => Some(4),
        "v" => Some(5),
        "vi" => Some(6),
        "vii" => Some(7),
        _ => None,
    }
}

/// Intervals of a Roman numeral suffix; `upper` is whether the numeral is in upper case.
fn roman_suffix_intervals(suffix: &str, upper: bool) -> Option<&'static [i64]> {
    Some(match (suffix, upper) {
        ("", true) => &[0, 4, 7],
        ("", false) => &[0, 3, 7],
        ("7", true) => &[0, 4, 7, 10],
        ("7", false) => &[0, 3, 7, 10],
        ("maj7", true) => &[0, 4, 7, 11],
        ("maj7", false) => &[0, 3, 7, 11],
        ("9", true) => &[0, 4, 7, 10, 14],
        ("9", false) => &[0, 3, 7, 10, 14],
        ("add9", true) => &[0, 4, 7, 14],
        ("add9", false) => &[0, 3, 7, 14],
        ("dim", _) => &[0, 3, 6],
        ("dim7", _) => &[0, 3, 6, 9],
        ("m7b5", _) => &[0, 3, 6, 10],
        ("aug", _) => &[0, 4, 8],
        ("sus2", _) => &[0, 2, 7],
        ("sus4", _) => &[0, 5, 7],
        ("6", true) => &[0, 4, 7, 9],
        ("6", false) => &[0, 3, 7, 9],
        _ => return None,
    })
}

/// Splits a Roman numeral into its accidental (`b` or `#`), the run of `i`/`v` letters in either
/// case, and the suffix (`^([b#]?)([ivIV]+)(.*)$`).
fn split_roman(text: &str) -> Option<(Option<char>, &str, &str)> {
    let (accidental, body) = match text.chars().next() {
        Some(c @ ('b' | '#')) => (Some(c), &text[1..]),
        _ => (None, text),
    };
    let letters = body
        .find(|c: char| !matches!(c, 'i' | 'v' | 'I' | 'V'))
        .unwrap_or(body.len());
    if letters == 0 {
        return None;
    }
    let (numeral, suffix) = body.split_at(letters);
    // `.` does not match a line break.
    (!suffix.contains('\n')).then_some((accidental, numeral, suffix))
}

/// The diatonic chord of a Roman numeral in `key`.
///
/// Case selects the quality (`IV` is major, `iv` minor) unless a suffix overrides it: `°` or
/// `dim`, `+` or `aug`, `7`, `maj7`, `m7b5` or `ø`, `sus2`, `sus4`, `9`, `add9`, `6`, `dim7`.
/// A `b` or `#` before the numeral alters the root (`bVII`, `#iv°`). Scales without seven notes
/// count their degrees in the parallel major or minor (chosen by the scale's third), so `III`
/// in A minor pentatonic is C.
///
/// # Errors
///
/// `INVALID_ARGUMENT` with `cannot parse Roman numeral '…'` or `unknown numeral suffix '…' in
/// '…'`.
pub fn roman_chord(numeral: &str, key: Key) -> Result<Chord> {
    let malformed =
        || DomainError::invalid(format!("cannot parse Roman numeral {}", str_repr(numeral)));
    let (accidental, letters, suffix) = split_roman(py_strip(numeral)).ok_or_else(malformed)?;
    let degree = roman_degree(&letters.to_lowercase()).ok_or_else(malformed)?;
    let intervals = key.scale.intervals();
    let mut root = if intervals.len() == 7 {
        key.degree_pc(degree)
    } else {
        let parent = if intervals.contains(&3) && !intervals.contains(&4) {
            Scale::Minor
        } else {
            Scale::Major
        };
        let index = usize::try_from(degree - 1).unwrap_or(0);
        (key.root + parent.intervals()[index]).rem_euclid(12)
    };
    match accidental {
        Some('b') => root = (root - 1).rem_euclid(12),
        Some('#') => root = (root + 1).rem_euclid(12),
        _ => {}
    }
    let upper = letters.chars().all(char::is_uppercase);
    let suffix = suffix
        .replace('°', "dim")
        .replace('ø', "m7b5")
        .replace('+', "aug");
    let intervals = roman_suffix_intervals(&suffix, upper).ok_or_else(|| {
        DomainError::invalid(format!(
            "unknown numeral suffix {} in {}",
            str_repr(&suffix),
            str_repr(numeral)
        ))
    })?;
    Ok(Chord {
        root,
        intervals: intervals.to_vec(),
        bass: None,
        symbol: numeral.to_owned(),
    })
}

/// A Roman-numeral chord when a key is given and the text looks like a numeral, else a chord
/// symbol.
///
/// # Errors
///
/// The errors of [`roman_chord`] or [`parse_chord`].
pub fn chord_from(text: &str, key: Option<Key>) -> Result<Chord> {
    if let Some(key) = key {
        let stripped = py_strip(text);
        let body = stripped.strip_prefix(['b', '#']).unwrap_or(stripped);
        if body.starts_with(['i', 'v', 'I', 'V']) {
            return roman_chord(text, key);
        }
    }
    parse_chord(text)
}

/// Concrete pitches for `chord` near `center`.
///
/// Every inversion and octave shift that stays within MIDI 36–96 is a candidate. With `previous`
/// the candidate with the smallest total movement wins (smooth voice leading); without it, the
/// one whose mean is closest to `center`. `spread` drops the lowest tone an octave when there
/// are at least three; a slash bass is added below everything.
#[must_use]
pub fn voice(chord: &Chord, center: i64, previous: Option<&[i64]>, spread: bool) -> Vec<i64> {
    let pitch_classes = chord.pitch_classes();
    let floor = center - 12;
    let base: Vec<i64> = pitch_classes
        .iter()
        .map(|pc| floor + (pc - floor).rem_euclid(12))
        .collect();
    let mut sorted_base = base.clone();
    sorted_base.sort_unstable();
    let mut candidates: Vec<Vec<i64>> = Vec::new();
    for inversion in 0..pitch_classes.len() {
        let mut notes = sorted_base.clone();
        for _ in 0..inversion {
            let lowest = notes.remove(0);
            notes.push(lowest + 12);
        }
        for shift in [-12, 0, 12] {
            let candidate: Vec<i64> = notes.iter().map(|n| n + shift).collect();
            let low = candidate.iter().copied().min().unwrap_or(0);
            let high = candidate.iter().copied().max().unwrap_or(0);
            if low >= 36 && high <= 96 {
                candidates.push(candidate);
            }
        }
    }
    if candidates.is_empty() {
        candidates.push(sorted_base);
    }
    let cost = |candidate: &[i64]| -> f64 {
        let sum: i64 = candidate.iter().sum();
        let mean = to_f64(sum) / to_f64_len(candidate.len());
        let drift = (mean - to_f64(center)).abs();
        match previous.filter(|p| !p.is_empty()) {
            Some(previous) => {
                let movement: i64 = candidate
                    .iter()
                    .map(|p| {
                        previous
                            .iter()
                            .map(|q| (p - q).abs())
                            .min()
                            .unwrap_or_default()
                    })
                    .sum();
                to_f64(movement) + drift * 0.3
            }
            None => drift,
        }
    };
    // `min` keeps the first of equal costs, like Python's `min(key=...)`.
    let mut best = candidates
        .into_iter()
        .map(|candidate| (cost(&candidate), candidate))
        .reduce(|best, next| if next.0 < best.0 { next } else { best })
        .map(|(_, candidate)| candidate)
        .unwrap_or_default();
    if spread && best.len() >= 3 {
        best[0] -= 12;
    }
    if let Some(bass) = chord.bass {
        let lowest = best.iter().copied().min().unwrap_or(0);
        let gap = (lowest - bass).rem_euclid(12);
        best.insert(0, lowest - if gap == 0 { 12 } else { gap });
    }
    best
}

/// The MIDI pitch of a chord's bass note in `octave` (middle C is octave 4): the slash bass if
/// there is one, else the root.
#[must_use]
pub fn bass_note(chord: &Chord, octave: i64) -> i64 {
    (octave + 1) * 12 + chord.bass.unwrap_or(chord.root)
}

#[cfg(test)]
mod tests {
    use super::{
        Chord, Key, Scale, bass_note, chord_from, parse_chord, parse_key, pitch_class, roman_chord,
        voice,
    };
    use serde_json::json;

    fn key(text: &str) -> Key {
        parse_key(text).expect("valid key")
    }

    fn chord(symbol: &str) -> Chord {
        parse_chord(symbol).expect("valid chord")
    }

    #[test]
    fn keys_parse_like_the_prototype() {
        let cases = [
            ("C major", 0, Scale::Major, "C major"),
            ("F# minor", 6, Scale::Minor, "F# minor"),
            ("Am", 9, Scale::Minor, "A minor"),
            ("D dorian", 2, Scale::Dorian, "D dorian"),
            ("Bb", 10, Scale::Major, "Bb major"),
            (
                "  eb  Harmonic-Minor ",
                3,
                Scale::HarmonicMinor,
                "Eb harmonic_minor",
            ),
            ("c\nmajor", 0, Scale::Major, "C major"),
            ("C ionian", 0, Scale::Major, "C major"),
            (
                "G pentatonic minor",
                7,
                Scale::PentatonicMinor,
                "G pentatonic_minor",
            ),
            ("Ab m", 8, Scale::Minor, "Ab minor"),
            ("e#", 5, Scale::Major, "F major"),
            ("Cb min", 11, Scale::Minor, "B minor"),
        ];
        for (text, root, scale, shown) in cases {
            let parsed = key(text);
            assert_eq!((parsed.root, parsed.scale), (root, scale), "{text:?}");
            assert_eq!(parsed.to_string(), shown, "{text:?}");
        }
    }

    #[test]
    fn bad_keys_report_the_prototype_messages() {
        let err = parse_key("H").expect_err("no such note");
        assert_eq!(err.message, "cannot parse key 'H'");
        let err = parse_key("C lydian\nx").expect_err("line break inside the scale");
        assert_eq!(err.message, "cannot parse key 'C lydian\\nx'");
        let err = parse_key("Bbb").expect_err("unknown scale");
        assert_eq!(err.message, "unknown scale 'b'");
        assert_eq!(
            err.details["available"],
            json!([
                "blues",
                "chromatic",
                "dorian",
                "harmonic_minor",
                "locrian",
                "lydian",
                "major",
                "melodic_minor",
                "minor",
                "mixolydian",
                "pentatonic_major",
                "pentatonic_minor",
                "phrygian",
                "whole_tone"
            ])
        );
    }

    #[test]
    fn pitch_classes_follow_the_prototype_table() {
        let table = [
            ("C", 0),
            ("B#", 0),
            ("C#", 1),
            ("Db", 1),
            ("D", 2),
            ("D#", 3),
            ("Eb", 3),
            ("E", 4),
            ("Fb", 4),
            ("E#", 5),
            ("F", 5),
            ("F#", 6),
            ("Gb", 6),
            ("G", 7),
            ("G#", 8),
            ("Ab", 8),
            ("A", 9),
            ("A#", 10),
            ("Bb", 10),
            ("B", 11),
            ("Cb", 11),
        ];
        for (name, expected) in table {
            let mut chars = name.chars();
            let letter = chars.next().expect("letter");
            assert_eq!(pitch_class(letter, chars.next()), expected, "{name}");
        }
    }

    #[test]
    fn chords_parse_like_the_prototype() {
        let c = chord("F#m7b5");
        assert_eq!(
            (c.root, c.intervals.as_slice()),
            (6, [0, 3, 6, 10].as_slice())
        );
        assert_eq!(chord("Bbmaj9").intervals, [0, 4, 7, 11, 14]);
        assert_eq!(chord("Cb").root, 11);
        assert_eq!(chord(" D13 ").symbol, "D13");
        let slash = chord("G/B");
        assert_eq!((slash.root, slash.bass), (7, Some(11)));
        assert_eq!(chord("G/Bb").bass, Some(10));
        assert_eq!(chord("C/B#").bass, Some(0));
        assert_eq!(chord("Am7").pitch_classes(), [9, 0, 4, 7]);
    }

    #[test]
    fn bad_chords_report_the_prototype_messages() {
        for text in ["C/E/G", "C/H", "Xm", "", "C/Bbb"] {
            let err = parse_chord(text).expect_err("malformed");
            assert_eq!(err.message, format!("cannot parse chord '{text}'"));
        }
        let err = parse_chord("Cfoo").expect_err("unknown quality");
        assert_eq!(err.message, "unknown chord quality 'foo' in 'Cfoo'");
        let available = err.details["available"].as_array().expect("array");
        assert_eq!(available.len(), 25);
        assert_eq!(available[0], "11");
        let err = parse_chord("Dm\n7").expect_err("line break in the quality");
        assert_eq!(err.message, "unknown chord quality 'm\\n7' in 'Dm\\n7'");
    }

    #[test]
    fn roman_numerals_follow_the_key() {
        let a_minor = key("A minor");
        let cases = [
            ("i", 9, vec![0, 3, 7]),
            ("VI", 5, vec![0, 4, 7]),
            ("bVII", 6, vec![0, 4, 7]),
            ("#iv°", 3, vec![0, 3, 6]),
            ("viiø", 7, vec![0, 3, 6, 10]),
            ("Isus4", 9, vec![0, 5, 7]),
            ("IVadd9", 2, vec![0, 4, 7, 14]),
            ("V9", 4, vec![0, 4, 7, 10, 14]),
            ("Iv", 2, vec![0, 3, 7]),
            ("ii7", 11, vec![0, 3, 7, 10]),
            ("vi+", 5, vec![0, 4, 8]),
        ];
        for (numeral, root, intervals) in cases {
            let c = roman_chord(numeral, a_minor).expect("valid numeral");
            assert_eq!(
                (c.root, c.intervals, c.symbol.as_str()),
                (root, intervals, numeral)
            );
        }
        assert_eq!(
            roman_chord("III", key("A pentatonic minor"))
                .expect("valid")
                .root,
            0
        );
        assert_eq!(roman_chord("V", key("C blues")).expect("valid").root, 7);
        assert_eq!(
            roman_chord("III", key("C whole tone")).expect("valid").root,
            4
        );
    }

    #[test]
    fn bad_numerals_report_the_prototype_messages() {
        let a_minor = key("A minor");
        let err = roman_chord("viii", a_minor).expect_err("no such degree");
        assert_eq!(err.message, "cannot parse Roman numeral 'viii'");
        let err = roman_chord("IVx", a_minor).expect_err("unknown suffix");
        assert_eq!(err.message, "unknown numeral suffix 'x' in 'IVx'");
        let err = roman_chord("x", a_minor).expect_err("not a numeral");
        assert_eq!(err.message, "cannot parse Roman numeral 'x'");
    }

    #[test]
    fn chord_from_picks_numerals_only_with_a_key() {
        let a_minor = key("A minor");
        assert_eq!(chord_from("Am", Some(a_minor)).expect("symbol").root, 9);
        assert_eq!(chord_from("iv", Some(a_minor)).expect("numeral").root, 2);
        assert_eq!(chord_from("bVII", Some(a_minor)).expect("numeral").root, 6);
        let err = chord_from("iv", None).expect_err("a numeral is no chord symbol");
        assert_eq!(err.message, "cannot parse chord 'iv'");
    }

    #[test]
    fn voicing_matches_the_prototype() {
        type Case = (&'static str, Option<Vec<i64>>, bool, i64, Vec<i64>);
        let cases: [Case; 8] = [
            ("C", None, false, 60, vec![55, 60, 64]),
            (
                "Am7",
                Some(vec![60, 64, 67]),
                false,
                60,
                vec![60, 64, 67, 69],
            ),
            ("G/B", None, false, 60, vec![47, 55, 59, 62]),
            (
                "Cmaj9",
                Some(vec![48, 55, 59]),
                true,
                64,
                vec![36, 50, 52, 55, 59],
            ),
            ("F", Some(vec![60, 64, 67]), false, 60, vec![57, 60, 65]),
            ("E13", None, true, 72, vec![54, 68, 71, 73, 74, 76]),
            ("C", None, false, 20, vec![12, 16, 19]),
            ("C", None, false, 110, vec![88, 91, 96]),
        ];
        for (symbol, previous, spread, center, expected) in cases {
            let voiced = voice(&chord(symbol), center, previous.as_deref(), spread);
            assert_eq!(voiced, expected, "{symbol} around {center}");
        }
    }

    #[test]
    fn bass_notes_use_the_slash_bass() {
        assert_eq!(bass_note(&chord("G/B"), 2), 47);
        assert_eq!(bass_note(&chord("Am"), 3), 57);
    }

    #[test]
    fn key_membership_and_snapping() {
        let c_major = key("C major");
        assert_eq!(c_major.snap(61), 60);
        assert_eq!(c_major.snap(66), 65);
        assert_eq!(c_major.snap(70), 69);
        assert_eq!(c_major.pitches(60, 72), [60, 62, 64, 65, 67, 69, 71, 72]);
        assert_eq!(c_major.degree_pc(8), 0);
        assert_eq!(c_major.degree_pc(0), 11);
        assert_eq!(c_major.degree_pc(-1), 9);
        assert!(c_major.contains(-1));
        assert!(!c_major.contains(-2));
    }

    #[test]
    fn scale_names_round_trip() {
        for scale in Scale::ALL {
            assert_eq!(Scale::from_name(scale.name()), Some(scale));
        }
        assert_eq!(Scale::from_name("ionian"), None);
    }
}
