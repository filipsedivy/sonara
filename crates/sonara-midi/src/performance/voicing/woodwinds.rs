//! Woodwind chords in pairs: superposed, or interlocked for a background.

use sonara_core::ids::IdGenerator;

use super::{
    Carrier, ChordOptions, ChordRules, Score, VoiceGroup, Voiced, VoicingError, check_inputs,
    sorted_slots, write_chords, wrong,
};
use crate::generators::ChordSlot;
use crate::orchestration::InstrumentType;
use crate::orchestration::SoundingPitch;
use crate::orchestration::rules::{
    DynamicBand, RuleId, STRONG_REGISTER, WEAK_REGISTER, WOODWIND_ORDER, register_factor,
};

/// Semitones in an octave.
const OCTAVE: i64 = 12;

/// The intervals modulo the octave that are concords: thirds, sixths and the octave.
const CONCORDS: [i64; 5] = [0, 3, 4, 8, 9];

/// How the pairs of a woodwind chord lie against each other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WindLayout {
    /// Each instrument's players stacked on the next instrument's in the natural order of the
    /// woodwinds, flutes on top (superposition), the usual layout.
    #[default]
    Superposed,
    /// Two pairs alternating from the top: the first of the higher pair, the first of the lower,
    /// the second of the higher and the second of the lower, as Forsyth interlocks 1st oboe, 1st
    /// clarinet, 2nd oboe and 2nd clarinet for a background in the treble, the way the two
    /// pairs of horns interlock.
    Interlocked,
}

/// How [`woodwind_pairs`] writes its chords.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindOptions {
    /// Whether the woodwinds carry the bass, and their velocity.
    pub chord: ChordOptions,
    /// How the pairs lie against each other.
    pub layout: WindLayout,
    /// The dynamic the chords are played at, which decides which registers are weak or strong.
    pub dynamic: DynamicBand,
}

impl Default for WindOptions {
    /// Superposed pairs at mp to mf with the default chord options.
    fn default() -> Self {
        Self {
            chord: ChordOptions::default(),
            layout: WindLayout::Superposed,
            dynamic: DynamicBand::Medium,
        }
    }
}

/// Voices `chords` for the woodwind parts `winds`, each instrument's players in player order.
///
/// The players of one instrument form a colour, and the colours lie in the natural order of the
/// woodwinds from the top (flutes, oboes, clarinets, bassoons, [`WOODWIND_ORDER`]); within a
/// branch, in the order the branch lists them (piccolo above flute). Each player plays one voice.
///
/// - [`WindLayout::Superposed`] stacks the colours, each colour's players from the top in the
///   order given; [`WindLayout::Interlocked`] alternates exactly two pairs. Neither encloses a
///   pair inside one colour (VOI-03).
/// - Two neighbouring players of one colour sound a concord, a third, a sixth or an octave;
///   seconds, fourths, fifths and sevenths go to two colours (VOI-05).
/// - No voice in a weak register sits next to one in a strong register at `options.dynamic`,
///   such as a low second flute under oboes (VOI-06).
/// - Four colours of one player each are never laid out in close position, within an octave
///   (VOI-02); three players need two of one colour (VOI-01).
///
/// The chords follow the shared rules of the family helpers; the root is doubled before any other
/// tone, the same way in every chord (VOI-04). Every given part appears in the result in the order
/// given.
///
/// # Errors
/// [`VoicingError::WrongInstrument`] for a part that is not a woodwind of the natural order (a
/// saxophone has no place in it); [`VoicingError::Breaks`] with VOI-01 for three players of three
/// colours; [`VoicingError::NotTwoPairs`] for an interlocked layout of anything but two players
/// each of two instruments; [`VoicingError::TooFewVoices`] for no part;
/// [`VoicingError::DuplicatePart`]; [`VoicingError::InvalidVelocity`];
/// [`VoicingError::InvalidSlot`].
pub fn woodwind_pairs(
    chords: &[ChordSlot],
    winds: &[Carrier],
    options: WindOptions,
    ids: &mut dyn IdGenerator,
) -> Result<Voiced, VoicingError> {
    check_inputs(winds, options.chord.velocity)?;
    let mut ranked = Vec::with_capacity(winds.len());
    for (place, carrier) in winds.iter().enumerate() {
        let branch = WOODWIND_ORDER
            .rank(carrier.instrument)
            .ok_or_else(|| wrong(carrier))?;
        let within = WOODWIND_ORDER
            .branches
            .get(branch)
            .and_then(|members| members.iter().position(|&m| m == carrier.instrument))
            .unwrap_or_default();
        ranked.push(((branch, within), place));
    }
    // The colours from the top, each with its players in the order given.
    let mut colours: Vec<(InstrumentType, Vec<usize>)> = Vec::new();
    ranked.sort_by_key(|&(rank, place)| (rank, place));
    for (_, place) in ranked {
        let Some(carrier) = winds.get(place) else {
            continue;
        };
        match colours
            .iter_mut()
            .find(|(colour, _)| *colour == carrier.instrument)
        {
            Some((_, players)) => players.push(place),
            None => colours.push((carrier.instrument, vec![place])),
        }
    }
    if winds.is_empty() {
        return Err(VoicingError::TooFewVoices {
            needed: 1,
            formed: 0,
        });
    }
    if winds.len() == 3 && colours.len() == 3 {
        return Err(VoicingError::Breaks(RuleId::Voi01));
    }
    let from_the_top: Vec<usize> = match options.layout {
        WindLayout::Superposed => colours
            .iter()
            .flat_map(|(_, players)| players.iter().copied())
            .collect(),
        WindLayout::Interlocked => match colours.as_slice() {
            [(_, upper), (_, lower)] => match (upper.as_slice(), lower.as_slice()) {
                (&[a1, a2], &[b1, b2]) => vec![a1, b1, a2, b2],
                _ => return Err(VoicingError::NotTwoPairs),
            },
            _ => return Err(VoicingError::NotTwoPairs),
        },
    };
    let groups = from_the_top
        .iter()
        .rev()
        .map(|&place| VoiceGroup::solo(winds, place))
        .collect::<Result<Vec<_>, _>>()?;
    // The voices from the bottom: each one's instrument and its place among its colour's players.
    let voices: Vec<(InstrumentType, usize)> = from_the_top
        .iter()
        .rev()
        .filter_map(|&place| {
            let carrier = winds.get(place)?;
            let colour = colours
                .iter()
                .find(|(colour, _)| *colour == carrier.instrument)?;
            let player = colour.1.iter().position(|&p| p == place)?;
            Some((carrier.instrument, player))
        })
        .collect();
    let four_colours = winds.len() == 4 && colours.len() == 4;
    let accept = |pitches: &[i64]| {
        concords_within_colours(&voices, pitches)
            && registers_correspond(&voices, pitches, options.dynamic)
            && !(four_colours && is_close(pitches))
    };
    let rules = ChordRules {
        carries_bass: options.chord.carries_bass,
        accept: &accept,
        velocity: options.chord.velocity,
    };
    let slots = sorted_slots(chords)?;
    let mut score = Score::new(winds);
    write_chords(&mut score, &slots, &groups, &rules, ids)?;
    Ok(score.finish())
}

/// Whether every two neighbouring players of one colour sound a concord (VOI-05): `voices` gives
/// each voice's instrument and its place among that instrument's players, `pitches` its pitch.
fn concords_within_colours(voices: &[(InstrumentType, usize)], pitches: &[i64]) -> bool {
    voices
        .iter()
        .zip(pitches)
        .all(|(&(colour, player), &pitch)| {
            voices
                .iter()
                .zip(pitches)
                .filter(|&(&(other, other_player), _)| {
                    other == colour && other_player == player + 1
                })
                .all(|(_, &next)| CONCORDS.contains(&(pitch - next).abs().rem_euclid(OCTAVE)))
        })
}

/// Whether no voice in a weak register sits next to a voice in a strong one at `band` (VOI-06).
fn registers_correspond(
    voices: &[(InstrumentType, usize)],
    pitches: &[i64],
    band: DynamicBand,
) -> bool {
    let factor = |instrument: InstrumentType, pitch: i64| {
        SoundingPitch::new(pitch)
            .and_then(|pitch| register_factor(instrument, pitch, band, false))
            .map_or(1.0, |row| row.factor)
    };
    let factors: Vec<f32> = voices
        .iter()
        .zip(pitches)
        .map(|(&(instrument, _), &pitch)| factor(instrument, pitch))
        .collect();
    factors.windows(2).all(|pair| match pair {
        [a, b] => {
            let weak = |f: f32| f <= WEAK_REGISTER.value;
            let strong = |f: f32| f >= STRONG_REGISTER.value;
            !((weak(*a) && strong(*b)) || (strong(*a) && weak(*b)))
        }
        _ => true,
    })
}

/// Whether the layout lies within an octave: close position.
fn is_close(pitches: &[i64]) -> bool {
    match (pitches.first(), pitches.last()) {
        (Some(low), Some(high)) => high - low <= OCTAVE,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::{concords_within_colours, is_close, registers_correspond};
    use crate::orchestration::InstrumentType::{Flute, Oboe};
    use crate::orchestration::rules::DynamicBand;

    #[test]
    fn two_players_of_one_colour_sound_a_concord() {
        // VOI-05: from the bottom the second flute, then the first.
        let pair = [(Flute, 1), (Flute, 0)];
        let cases = [
            (75, true),
            (76, true),
            (80, true),
            (81, true),
            (84, true),
            (74, false),
            (77, false),
            (79, false),
            (83, false),
        ];
        for (upper, concord) in cases {
            assert_eq!(
                concords_within_colours(&pair, &[72, upper]),
                concord,
                "{upper}"
            );
        }
        // Two colours may sound a discord.
        assert!(concords_within_colours(&[(Oboe, 0), (Flute, 0)], &[72, 79]));
    }

    #[test]
    fn a_weak_register_never_sits_next_to_a_strong_one() {
        // VOI-06: an oboe low (62, strong) under a flute low (64, weak) is refused; both above
        // their low registers are fine, and so is a voice of factor 1 between them.
        let voices = [(Oboe, 0), (Flute, 0)];
        assert!(!registers_correspond(
            &voices,
            &[62, 64],
            DynamicBand::Medium
        ));
        assert!(registers_correspond(
            &voices,
            &[70, 76],
            DynamicBand::Medium
        ));
        assert!(registers_correspond(
            &voices,
            &[62, 72],
            DynamicBand::Medium
        ));
        assert!(is_close(&[60, 64, 67, 72]));
        assert!(!is_close(&[60, 64, 67, 76]));
    }
}
