//! Programmatic MIDI generation (`sonara.midi.generators`): chord progressions, chord voicings,
//! arpeggios, basslines and rule-based melodies. Drum grooves are in [`crate::drums`] and note
//! transformations in [`crate::transforms`].
//!
//! All generators are deterministic for a given seed and reproduce the prototype's output
//! exactly, including the order in which they draw note IDs and random numbers. They return
//! plain [`MidiNote`]s positioned in beats relative to the clip start.

use std::collections::BTreeSet;
use std::str::FromStr;

use sonara_core::EPSILON;
use sonara_core::error::{DomainError, Result};
use sonara_core::ids::IdGenerator;
use sonara_core::midi::MidiNote;
use sonara_core::pyfmt::str_repr;

use crate::num::{py_max, py_min, py_mod, to_f64};
use crate::pyrandom::PyRandom;
use crate::theory::{Chord, Key, bass_note, chord_from, voice};

/// A chord placed on the timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct ChordSlot {
    /// The chord.
    pub chord: Chord,
    /// Start in beats.
    pub start: f64,
    /// Length in beats.
    pub length: f64,
}

impl ChordSlot {
    /// End position in beats.
    #[must_use]
    pub fn end(&self) -> f64 {
        self.start + self.length
    }
}

/// How long each chord of a progression lasts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BeatsPerChord<'a> {
    /// Every chord lasts this many beats.
    Uniform(f64),
    /// One length per chord; the count must match the number of chords.
    Each(&'a [f64]),
}

/// Lays chords end to end from `start`. The chords are Roman numerals (when `key` is given) or
/// chord symbols.
///
/// # Errors
///
/// `INVALID_ARGUMENT` for an empty progression (`progression needs at least one chord`), for
/// lengths that do not match the chords (`beats_per_chord must match the number of chords`), and
/// the errors of [`chord_from`].
pub fn progression<S: AsRef<str>>(
    chords: &[S],
    key: Option<Key>,
    beats_per_chord: BeatsPerChord<'_>,
    start: f64,
) -> Result<Vec<ChordSlot>> {
    if chords.is_empty() {
        return Err(DomainError::invalid("progression needs at least one chord"));
    }
    let lengths: Vec<f64> = match beats_per_chord {
        BeatsPerChord::Uniform(length) => vec![length; chords.len()],
        BeatsPerChord::Each(lengths) => lengths.to_vec(),
    };
    if lengths.len() != chords.len() {
        return Err(DomainError::invalid(
            "beats_per_chord must match the number of chords",
        ));
    }
    let mut slots = Vec::with_capacity(chords.len());
    let mut t = start;
    for (symbol, length) in chords.iter().zip(lengths) {
        slots.push(ChordSlot {
            chord: chord_from(symbol.as_ref(), key)?,
            start: t,
            length,
        });
        t += length;
    }
    Ok(slots)
}

/// The rhythm of [`chords_to_notes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChordRhythm {
    /// One long chord per slot.
    Sustain,
    /// A short chord on every beat.
    Stabs,
    /// A short chord on the "and" of every beat.
    Offbeat,
    /// Eighth notes.
    Pulse8,
    /// A chord every two beats.
    Half,
}

impl ChordRhythm {
    /// The name the prototype uses.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sustain => "sustain",
            Self::Stabs => "stabs",
            Self::Offbeat => "offbeat",
            Self::Pulse8 => "pulse8",
            Self::Half => "half",
        }
    }
}

impl FromStr for ChordRhythm {
    type Err = DomainError;

    /// # Errors
    ///
    /// `INVALID_ARGUMENT`: `unknown chord rhythm '…'`.
    fn from_str(name: &str) -> Result<Self> {
        match name {
            "sustain" => Ok(Self::Sustain),
            "stabs" => Ok(Self::Stabs),
            "offbeat" => Ok(Self::Offbeat),
            "pulse8" => Ok(Self::Pulse8),
            "half" => Ok(Self::Half),
            other => Err(DomainError::invalid(format!(
                "unknown chord rhythm {}",
                str_repr(other)
            ))),
        }
    }
}

/// Options of [`chords_to_notes`].
#[derive(Debug, Clone, PartialEq)]
pub struct ChordNotesOptions {
    /// Pitch the voicing centres on.
    pub center: i64,
    /// Note velocity.
    pub velocity: i64,
    /// When the chord sounds within its slot.
    pub rhythm: ChordRhythm,
    /// Length of a stab in beats (capped by the rhythm's step).
    pub stab_length: f64,
    /// Drop the lowest voice an octave.
    pub spread: bool,
}

impl Default for ChordNotesOptions {
    fn default() -> Self {
        Self {
            center: 60,
            velocity: 80,
            rhythm: ChordRhythm::Sustain,
            stab_length: 0.5,
            spread: false,
        }
    }
}

/// Voice-led chord notes: each chord is voiced near `center` with the smallest movement from
/// the previous chord, and played in the options' rhythm.
///
/// # Errors
///
/// The validation errors of [`MidiNote::create`] for out-of-range pitches or velocities.
pub fn chords_to_notes(
    ids: &mut dyn IdGenerator,
    slots: &[ChordSlot],
    options: &ChordNotesOptions,
) -> Result<Vec<MidiNote>> {
    let mut notes = Vec::new();
    let mut previous: Option<Vec<i64>> = None;
    for slot in slots {
        let pitches = voice(
            &slot.chord,
            options.center,
            previous.as_deref(),
            options.spread,
        );
        let hits = chord_hits(slot, options);
        previous = Some(pitches.clone());
        for &(t, length) in &hits {
            for &pitch in &pitches {
                notes.push(MidiNote::create(
                    ids,
                    pitch,
                    t,
                    py_max(length * 0.98, 0.05),
                    options.velocity,
                    0,
                )?);
            }
        }
    }
    Ok(notes)
}

/// The `(start, length)` of every chord hit within `slot`.
fn chord_hits(slot: &ChordSlot, options: &ChordNotesOptions) -> Vec<(f64, f64)> {
    let (step, offset) = match options.rhythm {
        ChordRhythm::Sustain => return vec![(slot.start, slot.length)],
        ChordRhythm::Stabs => (1.0, 0.0),
        ChordRhythm::Offbeat => (1.0, 0.5),
        ChordRhythm::Pulse8 => (0.5, 0.0),
        ChordRhythm::Half => (2.0, 0.0),
    };
    let length = if options.rhythm == ChordRhythm::Half {
        2.0 * 0.95
    } else {
        py_min(options.stab_length, step)
    };
    let end = slot.start + slot.length;
    let mut hits = Vec::new();
    let mut t = slot.start + offset;
    while t < end - EPSILON {
        hits.push((t, py_min(length, end - t)));
        t += step;
    }
    hits
}

/// The note order of [`arpeggio`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArpeggioPattern {
    /// Low to high.
    Up,
    /// High to low.
    Down,
    /// Up, then down without repeating the ends.
    UpDown,
    /// Down, then up without repeating the ends.
    DownUp,
    /// A random chord tone at every step.
    Random,
    /// Alternate each tone with the top note.
    Pinky,
}

impl ArpeggioPattern {
    /// The name the prototype uses.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::UpDown => "updown",
            Self::DownUp => "downup",
            Self::Random => "random",
            Self::Pinky => "pinky",
        }
    }
}

impl FromStr for ArpeggioPattern {
    type Err = DomainError;

    /// # Errors
    ///
    /// `INVALID_ARGUMENT`: `unknown arpeggio pattern '…'`.
    fn from_str(name: &str) -> Result<Self> {
        match name {
            "up" => Ok(Self::Up),
            "down" => Ok(Self::Down),
            "updown" => Ok(Self::UpDown),
            "downup" => Ok(Self::DownUp),
            "random" => Ok(Self::Random),
            "pinky" => Ok(Self::Pinky),
            other => Err(DomainError::invalid(format!(
                "unknown arpeggio pattern {}",
                str_repr(other)
            ))),
        }
    }
}

/// Options of [`arpeggio`].
#[derive(Debug, Clone, PartialEq)]
pub struct ArpeggioOptions {
    /// The note order.
    pub pattern: ArpeggioPattern,
    /// Step length in beats; must be positive.
    pub rate: f64,
    /// Number of octaves the arpeggio spans; must be at least 1.
    pub octaves: u32,
    /// Pitch the voicing centres on.
    pub center: i64,
    /// Note length as a fraction of the step.
    pub gate: f64,
    /// Base velocity.
    pub velocity: i64,
    /// Velocity added on the beat.
    pub accent: i64,
    /// Seed of the random pattern.
    pub seed: i64,
}

impl Default for ArpeggioOptions {
    fn default() -> Self {
        Self {
            pattern: ArpeggioPattern::Up,
            rate: 0.25,
            octaves: 1,
            center: 64,
            gate: 0.8,
            velocity: 90,
            accent: 15,
            seed: 0,
        }
    }
}

/// Rejects a non-positive or non-finite step, which would never advance the timeline. The
/// prototype loops forever in that case.
fn require_step(step: f64, name: &str) -> Result<()> {
    if step > 0.0 && step.is_finite() {
        Ok(())
    } else {
        Err(DomainError::invalid(format!("{name} must be > 0")))
    }
}

/// The note sequence of one arpeggiated chord.
fn arpeggio_sequence(pattern: ArpeggioPattern, pool: &[i64]) -> Vec<i64> {
    let reversed = || pool.iter().rev().copied();
    match pattern {
        ArpeggioPattern::Up | ArpeggioPattern::Random => pool.to_vec(),
        ArpeggioPattern::Down => reversed().collect(),
        ArpeggioPattern::UpDown => {
            let inner = pool.len().saturating_sub(1);
            let mut sequence = pool.to_vec();
            sequence.extend((1..inner).rev().map(|i| pool[i]));
            sequence
        }
        ArpeggioPattern::DownUp => {
            let inner = pool.len().saturating_sub(1);
            let mut sequence: Vec<i64> = reversed().collect();
            sequence.extend((1..inner).map(|i| pool[i]));
            sequence
        }
        ArpeggioPattern::Pinky => pool
            .split_last()
            .map(|(&top, rest)| rest.iter().flat_map(|&p| [p, top]).collect())
            .unwrap_or_default(),
    }
}

/// Arpeggiates chords, one note per `rate` beats.
///
/// # Errors
///
/// `INVALID_ARGUMENT` for a non-positive `rate`, for `octaves` below 1 and for a pattern that
/// yields no notes (the prototype fails with a Python exception or loops forever there), plus the
/// validation errors of [`MidiNote::create`].
pub fn arpeggio(
    ids: &mut dyn IdGenerator,
    slots: &[ChordSlot],
    options: &ArpeggioOptions,
) -> Result<Vec<MidiNote>> {
    require_step(options.rate, "arpeggio rate")?;
    if options.octaves < 1 {
        return Err(DomainError::invalid("arpeggio octaves must be >= 1"));
    }
    let mut rng = PyRandom::new(options.seed);
    let mut notes = Vec::new();
    let mut previous: Option<Vec<i64>> = None;
    for slot in slots {
        let base = voice(&slot.chord, options.center, previous.as_deref(), false);
        let pool: Vec<i64> = (0..i64::from(options.octaves))
            .flat_map(|octave| base.iter().map(move |p| p + 12 * octave))
            .collect::<BTreeSet<i64>>()
            .into_iter()
            .collect();
        previous = Some(base);
        let sequence = arpeggio_sequence(options.pattern, &pool);
        if sequence.is_empty() {
            return Err(DomainError::invalid(
                "arpeggio pattern produced no notes for the chord",
            ));
        }
        let end = slot.start + slot.length;
        let mut t = slot.start;
        let mut i = 0_usize;
        while t < end - EPSILON {
            let pitch = if options.pattern == ArpeggioPattern::Random {
                rng.choice(&pool).copied().unwrap_or_default()
            } else {
                sequence[i % sequence.len()]
            };
            let on_beat = py_mod(t - slot.start, 1.0).abs() < EPSILON;
            let velocity = (options.velocity + if on_beat { options.accent } else { 0 }).min(127);
            notes.push(MidiNote::create(
                ids,
                pitch,
                t,
                options.rate * options.gate,
                velocity,
                0,
            )?);
            t += options.rate;
            i += 1;
        }
    }
    Ok(notes)
}

/// The bass pattern of [`bassline`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BassStyle {
    /// One note per chord.
    Root,
    /// The root repeated every `rate` beats.
    Pulse,
    /// Alternate root and octave.
    Octaves,
    /// On the "and" of each beat.
    Offbeat,
    /// Alternate root and fifth.
    Fifths,
    /// Quarter-note chord tones with half-step approaches to the next root.
    Walking,
    /// A fixed syncopated one-bar pattern.
    Syncopated,
}

impl BassStyle {
    /// The name the prototype uses.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Pulse => "pulse",
            Self::Octaves => "octaves",
            Self::Offbeat => "offbeat",
            Self::Fifths => "fifths",
            Self::Walking => "walking",
            Self::Syncopated => "syncopated",
        }
    }
}

impl FromStr for BassStyle {
    type Err = DomainError;

    /// # Errors
    ///
    /// `INVALID_ARGUMENT`: `unknown bass style '…'`.
    fn from_str(name: &str) -> Result<Self> {
        match name {
            "root" => Ok(Self::Root),
            "pulse" => Ok(Self::Pulse),
            "octaves" => Ok(Self::Octaves),
            "offbeat" => Ok(Self::Offbeat),
            "fifths" => Ok(Self::Fifths),
            "walking" => Ok(Self::Walking),
            "syncopated" => Ok(Self::Syncopated),
            other => Err(DomainError::invalid(format!(
                "unknown bass style {}",
                str_repr(other)
            ))),
        }
    }
}

/// Options of [`bassline`].
#[derive(Debug, Clone, PartialEq)]
pub struct BasslineOptions {
    /// The pattern.
    pub style: BassStyle,
    /// Octave of the bass notes (middle C is octave 4).
    pub octave: i64,
    /// Step length in beats; must be positive.
    pub rate: f64,
    /// Base velocity.
    pub velocity: i64,
    /// Seed of the walking line.
    pub seed: i64,
    /// Key that keeps walking approach notes diatonic.
    pub key: Option<Key>,
}

impl Default for BasslineOptions {
    fn default() -> Self {
        Self {
            style: BassStyle::Root,
            octave: 2,
            rate: 0.5,
            velocity: 100,
            seed: 0,
            key: None,
        }
    }
}

/// A bass line from a progression.
///
/// # Errors
///
/// `INVALID_ARGUMENT` for a non-positive `rate` (the prototype loops forever there), plus the
/// validation errors of [`MidiNote::create`].
pub fn bassline(
    ids: &mut dyn IdGenerator,
    slots: &[ChordSlot],
    options: &BasslineOptions,
) -> Result<Vec<MidiNote>> {
    if matches!(
        options.style,
        BassStyle::Pulse | BassStyle::Octaves | BassStyle::Fifths
    ) {
        require_step(options.rate, "bass rate")?;
    }
    let mut rng = PyRandom::new(options.seed);
    let mut notes = Vec::new();
    for (index, slot) in slots.iter().enumerate() {
        let root = bass_note(&slot.chord, options.octave);
        match options.style {
            BassStyle::Root => notes.push(MidiNote::create(
                ids,
                root,
                slot.start,
                slot.length * 0.95,
                options.velocity,
                0,
            )?),
            BassStyle::Walking => {
                let next = bass_note(&slots[(index + 1) % slots.len()].chord, options.octave);
                walking_bass(ids, &mut notes, &mut rng, slot, root, next, options)?;
            }
            BassStyle::Offbeat => offbeat_bass(ids, &mut notes, slot, root, options)?,
            _ => stepped_bass(ids, &mut notes, slot, root, options)?,
        }
    }
    Ok(notes)
}

/// Quarter notes over chord tones, approaching the next root by a half step in the last beat.
fn walking_bass(
    ids: &mut dyn IdGenerator,
    notes: &mut Vec<MidiNote>,
    rng: &mut PyRandom,
    slot: &ChordSlot,
    root: i64,
    next: i64,
    options: &BasslineOptions,
) -> Result<()> {
    let second = slot.chord.intervals.get(1).map_or(4, |&interval| interval);
    let tones = [root, root + second, root + 7, root + 12];
    let end = slot.end();
    let mut t = slot.start;
    let mut i = 0_usize;
    while t < end - EPSILON {
        let remaining = end - t;
        let pitch = if remaining <= 1.0 + EPSILON {
            // Approach the next root by a half step from above or below.
            let mut pitch = next + if rng.random() < 0.5 { 1 } else { -1 };
            if options
                .key
                .is_some_and(|key| !key.contains(pitch) && key.contains(next - 2))
            {
                pitch = next - 2;
            }
            pitch
        } else if i < 2 {
            tones[i % tones.len()]
        } else {
            rng.choice(&tones).copied().unwrap_or(root)
        };
        let velocity = options.velocity - if i.is_multiple_of(2) { 0 } else { 10 };
        notes.push(MidiNote::create(ids, pitch, t, 0.9, velocity, 0)?);
        t += 1.0;
        i += 1;
    }
    Ok(())
}

/// The root on the "and" of every beat (or of every `rate` step when slower than a beat).
fn offbeat_bass(
    ids: &mut dyn IdGenerator,
    notes: &mut Vec<MidiNote>,
    slot: &ChordSlot,
    root: i64,
    options: &BasslineOptions,
) -> Result<()> {
    let step = py_max(options.rate, 1.0);
    let end = slot.end();
    let mut t = slot.start;
    let mut i = 0_usize;
    while t < end - EPSILON {
        let at = t + step / 2.0;
        if at < end - EPSILON {
            let velocity = options.velocity - if i % 2 == 1 { 8 } else { 0 };
            notes.push(MidiNote::create(
                ids,
                root,
                at,
                py_min(step * 0.4, end - at),
                velocity,
                0,
            )?);
        }
        t += step;
        i += 1;
    }
    Ok(())
}

/// The styles that step through the slot at `rate` (pulse, octaves, fifths) and the syncopated
/// bar pattern.
fn stepped_bass(
    ids: &mut dyn IdGenerator,
    notes: &mut Vec<MidiNote>,
    slot: &ChordSlot,
    root: i64,
    options: &BasslineOptions,
) -> Result<()> {
    let end = slot.end();
    let mut t = slot.start;
    let mut i = 0_usize;
    while t < end - EPSILON {
        let length = py_min(options.rate * 0.85, end - t);
        let odd = i % 2 == 1;
        let pitch = match options.style {
            BassStyle::Octaves => root + if odd { 12 } else { 0 },
            BassStyle::Fifths => root + if odd { 7 } else { 0 },
            BassStyle::Syncopated => {
                // One bar (4 beats) of pattern, repeated across the whole slot.
                let mut bar = slot.start;
                while bar < end - EPSILON {
                    // (offset in the bar, octave shift, velocity drop)
                    for (offset, shift, drop) in [
                        (0.0, 0, 0),
                        (0.75, 0, 10),
                        (1.5, 0, 10),
                        (2.5, 12, 10),
                        (3.0, 0, 10),
                    ] {
                        let at = bar + offset;
                        if at < end - EPSILON {
                            notes.push(MidiNote::create(
                                ids,
                                root + shift,
                                at,
                                py_min(0.4, end - at),
                                options.velocity - drop,
                                0,
                            )?);
                        }
                    }
                    bar += 4.0;
                }
                break;
            }
            // Pulse; root, walking and offbeat lines never get here.
            _ => root,
        };
        if t < end - EPSILON && length > 0.0 {
            let velocity = options.velocity - if odd { 12 } else { 0 };
            notes.push(MidiNote::create(ids, pitch, t, length, velocity, 0)?);
        }
        t += options.rate;
        i += 1;
    }
    Ok(())
}

/// Options of [`melody`].
#[derive(Debug, Clone, PartialEq)]
pub struct MelodyOptions {
    /// Lowest pitch of the register.
    pub low: i64,
    /// Highest pitch of the register.
    pub high: i64,
    /// Probability of a note on a weak position.
    pub density: f64,
    /// Basic step in beats; must be positive.
    pub rhythm_unit: f64,
    /// Base velocity.
    pub velocity: i64,
    /// Seed of the melody.
    pub seed: i64,
    /// Length of a phrase in beats.
    pub phrase_beats: f64,
}

impl Default for MelodyOptions {
    fn default() -> Self {
        Self {
            low: 60,
            high: 79,
            density: 0.6,
            rhythm_unit: 0.5,
            velocity: 95,
            seed: 0,
            phrase_beats: 8.0,
        }
    }
}

/// A rule-based melody: chord tones on strong beats, stepwise scale motion elsewhere, phrase
/// endings on long chord tones, the register bounded by `low`..=`high`.
///
/// # Errors
///
/// `INVALID_ARGUMENT`: `melody range contains no scale tones`; a non-positive `rhythm_unit`
/// or `phrase_beats` (the prototype loops forever or divides by zero there); plus the validation
/// errors of [`MidiNote::create`].
pub fn melody(
    ids: &mut dyn IdGenerator,
    key: Key,
    slots: &[ChordSlot],
    options: &MelodyOptions,
) -> Result<Vec<MidiNote>> {
    let mut rng = PyRandom::new(options.seed);
    let scale = key.pitches(options.low, options.high);
    if scale.is_empty() {
        return Err(DomainError::invalid("melody range contains no scale tones"));
    }
    require_step(options.rhythm_unit, "melody rhythm_unit")?;
    require_step(options.phrase_beats, "melody phrase_beats")?;
    let mut notes = Vec::new();
    let mut current = scale[scale.len() / 2];
    for slot in slots {
        let chord_pcs = slot.chord.pitch_classes();
        let end = slot.end();
        let mut t = slot.start;
        while t < end - EPSILON {
            let in_phrase = py_mod(t, options.phrase_beats) / options.phrase_beats;
            let strong = py_mod(t, 1.0).abs() < EPSILON;
            let phrase_end = in_phrase >= 0.75;
            if !strong && rng.random() > options.density {
                t += options.rhythm_unit;
                continue;
            }
            let pitch = if strong || phrase_end {
                let chord_tones: Vec<i64> = scale
                    .iter()
                    .copied()
                    .filter(|p| chord_pcs.contains(&p.rem_euclid(12)))
                    .collect();
                let candidates = if chord_tones.is_empty() {
                    &scale
                } else {
                    &chord_tones
                };
                nearest_with_jitter(&mut rng, candidates, current)
            } else {
                let nearest = nearest(&scale, current);
                let index = scale.iter().position(|&p| p == nearest).unwrap_or(0);
                let movement = rng.choice(&[-2_i64, -1, -1, 1, 1, 2]).copied().unwrap_or(1);
                let target = (to_index(index) + movement).clamp(0, to_index(scale.len() - 1));
                let candidate = scale[usize::try_from(target).unwrap_or(0)];
                nearest_with_jitter(&mut rng, &[candidate], current)
            };
            let mut length = options.rhythm_unit;
            if phrase_end && strong {
                let factor = rng.choice(&[2_i64, 3, 4]).copied().unwrap_or(2);
                length = py_min(end - t, options.rhythm_unit * to_f64(factor));
            } else if rng.random() < 0.25 {
                length = py_min(end - t, options.rhythm_unit * 2.0);
            }
            let velocity =
                (options.velocity + rng.randint(-10, 8) + if strong { 8 } else { 0 }).clamp(1, 127);
            notes.push(MidiNote::create(ids, pitch, t, length * 0.95, velocity, 0)?);
            current = pitch;
            t += length;
        }
    }
    Ok(notes)
}

fn to_index(index: usize) -> i64 {
    i64::try_from(index).unwrap_or(i64::MAX)
}

/// The first element of `pitches` closest to `current`.
fn nearest(pitches: &[i64], current: i64) -> i64 {
    pitches
        .iter()
        .copied()
        .reduce(|best, p| {
            if (p - current).abs() < (best - current).abs() {
                p
            } else {
                best
            }
        })
        .unwrap_or(current)
}

/// `min(candidates, key=lambda p: abs(p - current) + rng.random() * 3)`: one random draw per
/// candidate, in order; the first minimum wins.
fn nearest_with_jitter(rng: &mut PyRandom, candidates: &[i64], current: i64) -> i64 {
    let mut best: Option<(f64, i64)> = None;
    for &pitch in candidates {
        let key = to_f64((pitch - current).abs()) + rng.random() * 3.0;
        if best.is_none_or(|(best_key, _)| key < best_key) {
            best = Some((key, pitch));
        }
    }
    best.map_or(current, |(_, pitch)| pitch)
}

#[cfg(test)]
mod tests {
    use sonara_core::ids::SequentialIds;

    use super::{
        ArpeggioOptions, ArpeggioPattern, BassStyle, BasslineOptions, BeatsPerChord,
        ChordNotesOptions, ChordRhythm, MelodyOptions, arpeggio, bassline, chords_to_notes, melody,
        progression,
    };
    use crate::theory::parse_key;

    fn a_minor_slots() -> Vec<super::ChordSlot> {
        let key = parse_key("A minor").expect("valid key");
        progression(
            &["i", "VI", "III", "VII"],
            Some(key),
            BeatsPerChord::Uniform(4.0),
            0.0,
        )
        .expect("valid progression")
    }

    #[test]
    fn progression_lays_chords_end_to_end() {
        let slots = progression(
            &["Cmaj7", "Am7"],
            None,
            BeatsPerChord::Each(&[2.0, 6.0]),
            1.0,
        )
        .expect("valid progression");
        assert_eq!((slots[0].start, slots[0].length), (1.0, 2.0));
        assert_eq!((slots[1].start, slots[1].length), (3.0, 6.0));
        assert_eq!(slots[1].end(), 9.0);
        assert_eq!(slots[1].chord.symbol, "Am7");
    }

    #[test]
    fn progression_errors_use_the_prototype_messages() {
        let none: [&str; 0] = [];
        let err = progression(&none, None, BeatsPerChord::Uniform(4.0), 0.0).expect_err("empty");
        assert_eq!(err.message, "progression needs at least one chord");
        let err = progression(&["C", "G"], None, BeatsPerChord::Each(&[4.0]), 0.0)
            .expect_err("length mismatch");
        assert_eq!(
            err.message,
            "beats_per_chord must match the number of chords"
        );
        let err = progression(&["iv"], None, BeatsPerChord::Uniform(4.0), 0.0)
            .expect_err("numeral without a key");
        assert_eq!(err.message, "cannot parse chord 'iv'");
    }

    #[test]
    fn names_parse_and_report_the_prototype_messages() {
        assert_eq!("pulse8".parse::<ChordRhythm>(), Ok(ChordRhythm::Pulse8));
        assert_eq!(
            "pinky".parse::<ArpeggioPattern>(),
            Ok(ArpeggioPattern::Pinky)
        );
        assert_eq!("syncopated".parse::<BassStyle>(), Ok(BassStyle::Syncopated));
        for rhythm in [ChordRhythm::Sustain, ChordRhythm::Half] {
            assert_eq!(rhythm.name().parse::<ChordRhythm>(), Ok(rhythm));
        }
        assert_eq!(
            "wide".parse::<ChordRhythm>().expect_err("unknown").message,
            "unknown chord rhythm 'wide'"
        );
        assert_eq!(
            "zigzag"
                .parse::<ArpeggioPattern>()
                .expect_err("unknown")
                .message,
            "unknown arpeggio pattern 'zigzag'"
        );
        assert_eq!(
            "slap".parse::<BassStyle>().expect_err("unknown").message,
            "unknown bass style 'slap'"
        );
    }

    #[test]
    fn sustained_chords_fill_their_slots() {
        let mut ids = SequentialIds::new(0);
        let slots = a_minor_slots();
        let notes = chords_to_notes(&mut ids, &slots[..1], &ChordNotesOptions::default())
            .expect("valid notes");
        assert_eq!(notes.len(), 3);
        assert_eq!(notes[0].id, "n_000000000001");
        assert!(
            notes
                .iter()
                .all(|n| n.start == 0.0 && (n.length - 3.92).abs() < 1e-12)
        );
        let pitches: Vec<u8> = notes.iter().map(|n| n.pitch).collect();
        assert_eq!(pitches, [57, 60, 64]);
    }

    #[test]
    fn offbeat_stabs_sit_on_the_and_of_the_beat() {
        let mut ids = SequentialIds::new(0);
        let slots = a_minor_slots();
        let options = ChordNotesOptions {
            rhythm: ChordRhythm::Offbeat,
            ..ChordNotesOptions::default()
        };
        let notes = chords_to_notes(&mut ids, &slots[..1], &options).expect("valid notes");
        let starts: Vec<f64> = notes.iter().step_by(3).map(|n| n.start).collect();
        assert_eq!(starts, [0.5, 1.5, 2.5, 3.5]);
    }

    #[test]
    fn velocity_out_of_range_is_reported_like_the_prototype() {
        let mut ids = SequentialIds::new(0);
        let options = ChordNotesOptions {
            velocity: 0,
            ..ChordNotesOptions::default()
        };
        let err = chords_to_notes(&mut ids, &a_minor_slots(), &options).expect_err("velocity 0");
        assert_eq!(err.message, "note velocity must be 1-127, got 0");
    }

    #[test]
    fn up_arpeggio_climbs_and_accents_the_beat() {
        let mut ids = SequentialIds::new(0);
        let slots = a_minor_slots();
        let notes =
            arpeggio(&mut ids, &slots[..1], &ArpeggioOptions::default()).expect("valid notes");
        assert_eq!(notes.len(), 16);
        let pitches: Vec<u8> = notes.iter().take(4).map(|n| n.pitch).collect();
        assert_eq!(pitches, [60, 64, 69, 60]);
        assert_eq!(notes[0].velocity, 105);
        assert_eq!(notes[1].velocity, 90);
        assert_eq!(notes[4].velocity, 105);
        assert!((notes[0].length - 0.2).abs() < 1e-12);
    }

    #[test]
    fn arpeggio_rejects_inputs_that_never_terminate() {
        let mut ids = SequentialIds::new(0);
        let slots = a_minor_slots();
        let options = ArpeggioOptions {
            rate: 0.0,
            ..ArpeggioOptions::default()
        };
        assert_eq!(
            arpeggio(&mut ids, &slots, &options)
                .expect_err("zero rate")
                .message,
            "arpeggio rate must be > 0"
        );
        let options = ArpeggioOptions {
            octaves: 0,
            ..ArpeggioOptions::default()
        };
        assert_eq!(
            arpeggio(&mut ids, &slots, &options)
                .expect_err("no octave")
                .message,
            "arpeggio octaves must be >= 1"
        );
    }

    #[test]
    fn arpeggio_patterns_order_the_pool() {
        let slots = a_minor_slots();
        let first_bar = |pattern| {
            let mut ids = SequentialIds::new(0);
            let options = ArpeggioOptions {
                pattern,
                rate: 0.5,
                octaves: 2,
                ..ArpeggioOptions::default()
            };
            arpeggio(&mut ids, &slots[..1], &options)
                .expect("valid notes")
                .iter()
                .map(|n| n.pitch)
                .collect::<Vec<u8>>()
        };
        // The pool is 60, 64, 69, 72, 76, 81 (the chord, then the chord an octave up).
        assert_eq!(
            first_bar(ArpeggioPattern::Up),
            [60, 64, 69, 72, 76, 81, 60, 64]
        );
        assert_eq!(
            first_bar(ArpeggioPattern::Down),
            [81, 76, 72, 69, 64, 60, 81, 76]
        );
        assert_eq!(
            first_bar(ArpeggioPattern::UpDown),
            [60, 64, 69, 72, 76, 81, 76, 72]
        );
        assert_eq!(
            first_bar(ArpeggioPattern::DownUp),
            [81, 76, 72, 69, 64, 60, 64, 69]
        );
        assert_eq!(
            first_bar(ArpeggioPattern::Pinky),
            [60, 81, 64, 81, 69, 81, 72, 81]
        );
    }

    #[test]
    fn root_bass_plays_one_note_per_chord() {
        let mut ids = SequentialIds::new(0);
        let notes =
            bassline(&mut ids, &a_minor_slots(), &BasslineOptions::default()).expect("valid notes");
        let pitches: Vec<u8> = notes.iter().map(|n| n.pitch).collect();
        assert_eq!(pitches, [45, 41, 36, 43]);
        assert!(notes.iter().all(|n| (n.length - 3.8).abs() < 1e-12));
    }

    #[test]
    fn syncopated_bass_repeats_one_bar_pattern() {
        let mut ids = SequentialIds::new(0);
        let options = BasslineOptions {
            style: BassStyle::Syncopated,
            ..BasslineOptions::default()
        };
        let notes = bassline(&mut ids, &a_minor_slots()[..1], &options).expect("valid notes");
        let starts: Vec<f64> = notes.iter().map(|n| n.start).collect();
        assert_eq!(starts, [0.0, 0.75, 1.5, 2.5, 3.0]);
        assert_eq!(notes[3].pitch, 57);
        assert_eq!(notes[0].velocity, 100);
        assert_eq!(notes[1].velocity, 90);
    }

    #[test]
    fn bass_rate_must_advance_the_timeline() {
        let mut ids = SequentialIds::new(0);
        let options = BasslineOptions {
            style: BassStyle::Pulse,
            rate: -1.0,
            ..BasslineOptions::default()
        };
        let err = bassline(&mut ids, &a_minor_slots(), &options).expect_err("negative rate");
        assert_eq!(err.message, "bass rate must be > 0");
    }

    #[test]
    fn melody_stays_in_the_register_and_the_key() {
        let key = parse_key("A minor").expect("valid key");
        let mut ids = SequentialIds::new(0);
        let notes = melody(&mut ids, key, &a_minor_slots(), &MelodyOptions::default())
            .expect("valid notes");
        assert!(notes.len() > 10, "the melody has notes");
        assert!(notes.iter().all(|n| (60..=79).contains(&n.pitch)));
        assert!(notes.iter().all(|n| key.contains(i64::from(n.pitch))));
        // Same seed, same melody (IDs aside).
        let mut again = SequentialIds::new(0);
        let repeated = melody(&mut again, key, &a_minor_slots(), &MelodyOptions::default())
            .expect("valid notes");
        assert_eq!(notes, repeated);
    }

    #[test]
    fn melody_rejects_an_empty_register() {
        let key = parse_key("C major").expect("valid key");
        let options = MelodyOptions {
            low: 61,
            high: 61,
            ..MelodyOptions::default()
        };
        let err = melody(&mut SequentialIds::new(0), key, &a_minor_slots(), &options)
            .expect_err("no scale tone between 61 and 61");
        assert_eq!(err.message, "melody range contains no scale tones");
    }
}
