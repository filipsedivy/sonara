//! String chords, divided where they have more notes than sections.

use sonara_core::ids::IdGenerator;

use super::{
    Carrier, ChordOptions, ChordRules, Remark, Score, VoiceGroup, Voiced, VoicingError,
    check_inputs, sorted_slots, write_chords, wrong,
};
use crate::generators::ChordSlot;
use crate::orchestration::InstrumentType;
use crate::orchestration::rules::RuleId;

/// The most notes one string part holds at once: a section divides at most in two, since only the
/// upper two notes of a chord on one part can be held (VOI-12), which also keeps every section
/// below the four ways that sound thin at forte (VOI-11).
const MOST_WAYS: usize = 2;

/// Semitones in an octave.
const OCTAVE: i64 = 12;

/// Voices `chords` in `voices` notes for the string sections `strings`, dividing the upper
/// sections when the notes outnumber them.
///
/// - The upper sections are the violins, the violas and, when the strings do not carry the bass,
///   the cellos, from the top in that order and each instrument's sections in the order given
///   (first violins before second violins).
/// - With `options.carries_bass`, the first cello section plays the chord's bass and every double
///   bass section doubles it an octave below, the usual construction of the bass. Without cellos
///   the lowest upper section takes the bass and the double basses rest: a double-bass line is
///   never left without cellos (DBL-08). Without the bass, the double basses rest.
/// - Every upper section divides the same number of ways (VOI-10), at most two, so a part holds
///   at most two notes at once (VOI-11, VOI-12); the two halves of a section never sound an
///   octave or a unison apart (DBL-05, and DBL-06 for the violins). A divided section is
///   reported with [`Remark::Divided`].
/// - When the upper notes are fewer than the upper sections, the lowest sections play them, the
///   second violins with the violas rather than the first violins, which stay free for a melody;
///   the others rest. When they are more, every upper section divides, so `voices` rounds up to a
///   whole number of notes per section: four upper notes over three sections become six.
///
/// The chords follow the shared rules of the family helpers. Every given part appears in the
/// result in the order given.
///
/// # Errors
/// [`VoicingError::WrongInstrument`] for a part that is not a violin, viola, cello or double
/// bass; [`VoicingError::TooFewVoices`] for no voice, or when no section can carry the notes;
/// [`VoicingError::TooManyVoices`] when the notes need a section to divide more than two ways;
/// [`VoicingError::DuplicatePart`]; [`VoicingError::InvalidVelocity`];
/// [`VoicingError::InvalidSlot`].
pub fn strings_divisi(
    chords: &[ChordSlot],
    strings: &[Carrier],
    voices: usize,
    options: ChordOptions,
    ids: &mut dyn IdGenerator,
) -> Result<Voiced, VoicingError> {
    use InstrumentType::{Cello, DoubleBass, Viola, Violin};
    check_inputs(strings, options.velocity)?;
    if let Some(carrier) = strings
        .iter()
        .find(|carrier| !matches!(carrier.instrument, Violin | Viola | Cello | DoubleBass))
    {
        return Err(wrong(carrier));
    }
    if voices == 0 {
        return Err(VoicingError::TooFewVoices {
            needed: 1,
            formed: 0,
        });
    }
    let Sections {
        bass,
        upper,
        basses,
    } = Sections::of(strings, options.carries_bass);
    let upper_voices = voices - usize::from(bass.is_some());
    if bass.is_none() && upper.is_empty() {
        return Err(VoicingError::TooFewVoices {
            needed: voices,
            formed: 0,
        });
    }
    let most = upper.len() * MOST_WAYS + usize::from(bass.is_some());
    if voices > most {
        return Err(VoicingError::TooManyVoices {
            asked: voices,
            most,
        });
    }
    let ways = if upper.is_empty() {
        1
    } else {
        upper_voices.div_ceil(upper.len()).max(1)
    };
    // Fewer notes than sections: the lowest sections play them.
    let playing: Vec<usize> = upper
        .iter()
        .copied()
        .skip(upper.len().saturating_sub(upper_voices))
        .collect();
    let mut score = Score::new(strings);
    let mut groups = Vec::new();
    if let Some(bass) = bass {
        let mut voice = VoiceGroup::solo(strings, bass)?;
        if strings.get(bass).map(|carrier| carrier.instrument) == Some(Cello) {
            for &place in &basses {
                voice.join(strings, place, -OCTAVE);
            }
        } else {
            for &place in &basses {
                score.silence(place, RuleId::Dbl08);
            }
        }
        groups.push(voice);
    }
    // From the bottom: the lowest section's voices first.
    for &place in playing.iter().rev() {
        for _ in 0..ways {
            groups.push(VoiceGroup::solo(strings, place)?);
        }
        if let Some(carrier) = strings.get(place).filter(|_| ways > 1) {
            score.remarks.push(Remark::Divided {
                part: carrier.part.clone(),
                ways: u8::try_from(ways).unwrap_or(u8::MAX),
            });
        }
    }
    let sections: Vec<usize> = groups
        .iter()
        .map(|voice| {
            voice
                .members
                .first()
                .map_or(usize::MAX, |&(place, _)| place)
        })
        .collect();
    let accept = |pitches: &[i64]| halves_apart(&sections, pitches);
    let rules = ChordRules {
        carries_bass: options.carries_bass,
        accept: &accept,
        velocity: options.velocity,
    };
    let slots = sorted_slots(chords)?;
    write_chords(&mut score, &slots, &groups, &rules, ids)?;
    Ok(score.finish())
}

/// The string sections of a chord by their places among the parts.
struct Sections {
    /// The section that plays the bass, when the strings carry it.
    bass: Option<usize>,
    /// The upper sections from the top.
    upper: Vec<usize>,
    /// The double-bass sections.
    basses: Vec<usize>,
}

impl Sections {
    /// The sections of `strings`; `carries_bass` gives the bass to the first cello section, or
    /// to the lowest upper section when there is no cello.
    fn of(strings: &[Carrier], carries_bass: bool) -> Self {
        use InstrumentType::{Cello, DoubleBass, Viola, Violin};
        let places = |instrument: InstrumentType| -> Vec<usize> {
            strings
                .iter()
                .enumerate()
                .filter(|(_, carrier)| carrier.instrument == instrument)
                .map(|(place, _)| place)
                .collect()
        };
        let mut cellos = places(Cello);
        let mut upper = places(Violin);
        upper.extend(places(Viola));
        let bass = if !carries_bass {
            None
        } else if cellos.is_empty() {
            upper.pop()
        } else {
            Some(cellos.remove(0))
        };
        upper.extend(cellos);
        Self {
            bass,
            upper,
            basses: places(DoubleBass),
        }
    }
}

/// Whether no two voices of one section, `sections` giving each voice's section, lie a unison or
/// an octave apart (DBL-05, DBL-06).
fn halves_apart(sections: &[usize], pitches: &[i64]) -> bool {
    pitches.iter().enumerate().all(|(index, &lower)| {
        pitches
            .iter()
            .enumerate()
            .skip(index + 1)
            .all(|(other, &upper)| {
                sections.get(index) != sections.get(other) || (upper - lower) % OCTAVE != 0
            })
    })
}
