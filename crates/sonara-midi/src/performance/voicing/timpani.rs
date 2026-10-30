//! The timpani: two drums on the tonic and the dominant, hits and rolls.

use sonara_core::ids::IdGenerator;

use super::{
    Carrier, DEFAULT_VELOCITY, Score, Voiced, VoicingError, check_inputs, comfortable, new_note,
    sorted_slots, wrong,
};
use crate::generators::ChordSlot;
use crate::orchestration::{Articulation, InstrumentType, PitchRange, SoundingPitch};
use crate::performance::ScoreContext;
use crate::theory::Key;

/// Semitones in an octave.
const OCTAVE: i64 = 12;

/// The dominant lies a fifth above the tonic.
const FIFTH: i64 = 7;

/// A fourth: the dominant tuned below the tonic.
const FOURTH: i64 = 5;

/// The length of a hit in beats; a hit near the end of its chord ends with the chord.
const HIT_BEATS: f64 = 1.0;

/// How close two beats must be to count as one.
const BEAT_EPSILON: f64 = 1e-9;

/// How [`timpani`] writes its part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimpaniOptions {
    /// The key of the passage: its tonic and the dominant a fifth above are the two drums.
    pub key: Key,
    /// The velocity of every hit and roll, 1-127.
    pub velocity: u8,
}

impl TimpaniOptions {
    /// The drums of `key` at [`DEFAULT_VELOCITY`].
    #[must_use]
    pub const fn new(key: Key) -> Self {
        Self {
            key,
            velocity: DEFAULT_VELOCITY,
        }
    }
}

/// The timpani part of a passage.
#[derive(Clone, Debug, PartialEq)]
pub struct TimpaniPart {
    /// The two drums, the lower first: the tonic and the dominant a fourth or a fifth apart.
    pub drums: [SoundingPitch; 2],
    /// The timpani's notes, and a remark when it rests throughout.
    pub voiced: Voiced,
}

/// Writes the timpani part `timpani` for the chords `chords` of a passage in `options.key`; the
/// slots' beats are content beats of the clip that `score` places.
///
/// - **Tuning.** Two drums on the tonic and the dominant, the main harmonic notes of the passage
///   (Forsyth): the lowest pair inside the timpani's comfortable range, the dominant a fifth above
///   the tonic or a fourth below it, never a third, whose upper partials clash (PLY-12). Nothing
///   else is played, so no drum is retuned (PLY-11).
/// - **Hits.** A chord whose bass note (its slash bass, or its root) is a drum's pitch gets a hit
///   on that drum on every downbeat it holds, one beat long; a chord with another bass gets none.
/// - **Rolls.** A bass held on a drum's pitch across two or more chords in a row, a pedal, and the
///   passage's last chord when its bass is on a drum, the held close, get a roll through their
///   length, written as one note with the articulation `roll`.
///
/// # Errors
/// [`VoicingError::WrongInstrument`] for a part that does not play the timpani;
/// [`VoicingError::InvalidVelocity`]; [`VoicingError::InvalidSlot`].
pub fn timpani(
    chords: &[ChordSlot],
    timpani: &Carrier,
    score: &ScoreContext,
    options: TimpaniOptions,
    ids: &mut dyn IdGenerator,
) -> Result<TimpaniPart, VoicingError> {
    let parts = std::slice::from_ref(timpani);
    check_inputs(parts, options.velocity)?;
    if timpani.instrument != InstrumentType::Timpani {
        return Err(wrong(timpani));
    }
    let range = comfortable(timpani.instrument).ok_or_else(|| wrong(timpani))?;
    let tonic = options.key.root.rem_euclid(OCTAVE);
    let drums = tuning(range, tonic).ok_or_else(|| wrong(timpani))?;
    let drum = |pitch_class: i64| {
        drums
            .iter()
            .find(|drum| drum.get().rem_euclid(OCTAVE) == pitch_class)
            .map(|drum| drum.get())
    };
    let slots = sorted_slots(chords)?;
    let mut written = Score::new(parts);
    let runs = runs(&slots);
    let last = runs.len().saturating_sub(1);
    for (index, run) in runs.iter().enumerate() {
        let Some(pitch) = drum(run.bass) else {
            continue;
        };
        if run.chords >= 2 || index == last {
            let note = new_note(ids, pitch, run.start, run.end - run.start, options.velocity)?
                .with_articulation(Some(Articulation::Roll.id().to_owned()))?;
            written.push(0, note);
        } else {
            for beat in downbeats(score, run.start, run.end) {
                let length = HIT_BEATS.min(run.end - beat);
                written.push(0, new_note(ids, pitch, beat, length, options.velocity)?);
            }
        }
    }
    Ok(TimpaniPart {
        drums,
        voiced: written.finish(),
    })
}

/// The two drums for `tonic` inside `range`: the pair with the lowest drum among the tonic with
/// the dominant a fifth above and the dominant with the tonic a fourth above.
fn tuning(range: PitchRange, tonic: i64) -> Option<[SoundingPitch; 2]> {
    let dominant = (tonic + FIFTH).rem_euclid(OCTAVE);
    let (low, high) = (range.low().get(), range.high().get());
    (low..=high).find_map(|pitch| {
        let class = pitch.rem_euclid(OCTAVE);
        let upper = if class == tonic {
            pitch + FIFTH
        } else if class == dominant {
            pitch + FOURTH
        } else {
            return None;
        };
        if upper > high {
            return None;
        }
        Some([SoundingPitch::new(pitch)?, SoundingPitch::new(upper)?])
    })
}

/// Chords in a row on one bass note.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Run {
    /// The bass's pitch class.
    bass: i64,
    /// The first chord's start in content beats.
    start: f64,
    /// The last chord's end in content beats.
    end: f64,
    /// How many chords the run holds.
    chords: usize,
}

/// The slots grouped into runs: a slot joins the run before it when it starts where that run
/// ends and has the same bass.
fn runs(slots: &[&ChordSlot]) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for slot in slots {
        let bass = slot
            .chord
            .bass
            .unwrap_or(slot.chord.root)
            .rem_euclid(OCTAVE);
        let end = slot.start + slot.length;
        match runs.last_mut() {
            Some(run) if run.bass == bass && (slot.start - run.end).abs() < BEAT_EPSILON => {
                run.end = end;
                run.chords += 1;
            }
            _ => runs.push(Run {
                bass,
                start: slot.start,
                end,
                chords: 1,
            }),
        }
    }
    runs
}

/// The content beats from `start` (inclusive) to `end` (exclusive) on which a bar begins, in the
/// score's metre and with its clip origin.
fn downbeats(score: &ScoreContext, start: f64, end: f64) -> Vec<f64> {
    let signatures = score.signatures();
    let origin = score.origin();
    let mut bar = (signatures.beat_to_bar(origin + start) - BEAT_EPSILON).ceil();
    let mut beats = Vec::new();
    loop {
        let beat = signatures.bar_to_beat(bar) - origin;
        if beat >= end - BEAT_EPSILON {
            return beats;
        }
        if beat >= start - BEAT_EPSILON {
            beats.push(beat.max(start));
        }
        bar += 1.0;
    }
}
