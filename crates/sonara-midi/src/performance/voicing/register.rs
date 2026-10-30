//! Moving a line by octaves into its instrument's comfortable range.

use sonara_core::midi::MidiNote;

use super::comfortable;
use crate::orchestration::rules::Role;
use crate::orchestration::{Instrument, PitchModel, PitchRange};

/// Semitones in an octave.
const OCTAVE: i64 = 12;

/// The furthest a whole line is moved, in octaves either way.
const MAX_OCTAVES: i64 = 10;

/// What [`fit_register`] did with a line.
#[derive(Clone, Debug, PartialEq)]
pub enum RegisterFit {
    /// Every note lies in the comfortable range, or the instrument has no pitch: nothing moved.
    Fits,
    /// The line moved: the whole line by `shift` semitones, a whole number of octaves, and then
    /// each note in `folded` by further octaves of its own.
    Moved {
        /// The line's notes after the move, each keeping its id, timing, velocity and
        /// articulation, in the order given.
        notes: Vec<MidiNote>,
        /// The semitones the whole line moved; 0 when only single notes moved.
        shift: i64,
        /// The ids of the notes moved on their own after the shift, in the order given.
        folded: Vec<String>,
    },
    /// A melody lies outside the comfortable range. A melody is never moved silently: the line
    /// is unchanged, and the caller asks whether to move it by `shift` semitones.
    Ask {
        /// The whole-octave move that brings the most notes inside the range; 0 when none helps.
        shift: i64,
        /// How many notes lie outside the range as written.
        outside: usize,
    },
}

/// Fits `notes`, a line that `instrument` plays in the role `role`, into the instrument's
/// comfortable range (its playable range when no comfortable range is known), by octaves.
///
/// - A harmony, bass, rhythm, colour or pedal line moves as a whole by the number of octaves that
///   leaves the fewest notes outside the range (the smallest move among equals, then the one
///   nearest the range's middle), which keeps its shape; a note still outside then moves on its
///   own by the fewest octaves that bring it inside (RNG-02), or inside the playable range when no
///   octave of it is comfortable (RNG-01).
/// - A melody or counter-melody is never moved: when it leaves the range the result is
///   [`RegisterFit::Ask`] with the move to propose.
///
/// A note no octave brings into the playable range stays where it is.
#[must_use]
pub fn fit_register(notes: &[MidiNote], instrument: &Instrument, role: Role) -> RegisterFit {
    let PitchModel::Pitched(pitched) = instrument.pitch() else {
        return RegisterFit::Fits;
    };
    let Some(range) = comfortable(instrument.instrument_type()) else {
        return RegisterFit::Fits;
    };
    let outside = |shift: i64| {
        notes
            .iter()
            .filter(|note| !holds(range, i64::from(note.pitch) + shift))
            .count()
    };
    let written = outside(0);
    if written == 0 {
        return RegisterFit::Fits;
    }
    let shift = best_shift(notes, range, outside);
    if matches!(role, Role::Melody | Role::CounterMelody) {
        return RegisterFit::Ask {
            shift,
            outside: written,
        };
    }
    let mut folded = Vec::new();
    let mut moved = Vec::with_capacity(notes.len());
    for note in notes {
        let shifted = i64::from(note.pitch) + shift;
        let pitch = if holds(range, shifted) {
            shifted
        } else {
            let fold = nearest_octave(shifted, range)
                .or_else(|| nearest_octave(shifted, pitched.playable.range))
                .unwrap_or(shifted);
            if fold != shifted {
                folded.push(note.id.clone());
            }
            fold
        };
        let mut moved_note = note.clone();
        // Every pitch here is the note moved by a shift that `best_shift` keeps inside MIDI
        // notes 0-127, or a pitch of an instrument's range, so the conversion keeps it.
        moved_note.pitch = u8::try_from(pitch).unwrap_or(note.pitch);
        moved.push(moved_note);
    }
    RegisterFit::Moved {
        notes: moved,
        shift,
        folded,
    }
}

/// Whether `range` holds MIDI note `pitch`.
fn holds(range: PitchRange, pitch: i64) -> bool {
    range.low().get() <= pitch && pitch <= range.high().get()
}

/// The whole-octave move of `notes` that leaves the fewest notes outside `range` and keeps every
/// note a MIDI note: the smallest among equals, then the one that brings the line's middle nearest
/// the range's.
fn best_shift(notes: &[MidiNote], range: PitchRange, outside: impl Fn(i64) -> usize) -> i64 {
    let lowest = notes.iter().map(|note| i64::from(note.pitch)).min();
    let highest = notes.iter().map(|note| i64::from(note.pitch)).max();
    let (Some(lowest), Some(highest)) = (lowest, highest) else {
        return 0;
    };
    let middle = range.low().get() + range.high().get();
    (-MAX_OCTAVES..=MAX_OCTAVES)
        .map(|octaves| octaves * OCTAVE)
        .filter(|shift| lowest + shift >= 0 && highest + shift <= 127)
        .min_by_key(|&shift| {
            (
                outside(shift),
                shift.abs(),
                (lowest + highest + 2 * shift - middle).abs(),
            )
        })
        .unwrap_or(0)
}

/// The octave of `pitch` inside `range` nearest to it, or `None` when no octave of it is inside.
fn nearest_octave(pitch: i64, range: PitchRange) -> Option<i64> {
    (-MAX_OCTAVES..=MAX_OCTAVES)
        .map(|octaves| pitch + octaves * OCTAVE)
        .filter(|&candidate| holds(range, candidate))
        .min_by_key(|candidate| (candidate - pitch).abs())
}
