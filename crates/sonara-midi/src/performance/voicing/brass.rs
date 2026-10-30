//! The horns in close harmony and the brass chorale.

use sonara_core::ids::IdGenerator;

use super::layout::clears_limits;
use super::{
    Carrier, ChordOptions, ChordRules, Score, VoiceGroup, Voiced, VoicingError, check_inputs,
    comfortable, new_note, sorted_slots, write_chords, wrong,
};
use crate::generators::ChordSlot;
use crate::orchestration::InstrumentType;
use crate::orchestration::rules::RuleId;

/// The fewest and most horns [`horns`] voices.
const HORNS: (usize, usize) = (2, 4);

/// The fewest voices of a brass chorale.
const CHORALE_VOICES: usize = 3;

/// The real parts of a chorale; horn pairs add parts until there are this many.
const CHORALE_PARTS: usize = 4;

/// Places `0..count` in the order the horns sit from the top: the odd-numbered horns, then the
/// even ones, so four horns read 1, 3, 2, 4 (VOI-07).
fn from_the_top(count: usize) -> impl Iterator<Item = usize> {
    (0..count).step_by(2).chain((1..count).step_by(2))
}

/// Voices `chords` for the horns `horns`, given in player order (Horn 1 first), and doubles their
/// bass with `bassoon` when one is given.
///
/// - Each horn plays one voice of the chord inside the horn's comfortable range. From the top the
///   voices go to Horns 1, 3, 2 and 4: the odd horns play the upper parts and the even ones the
///   lower, the 3rd above the 2nd as in Forsyth's account of the Eroica (VOI-07). Three horns read
///   1, 3, 2 and two horns 1, 2.
/// - The layout follows the shared rules of the family helpers; carrying the bass, four horns
///   voice D minor over D3 as F4, D4, A3, D3. Between chords the voices move as little as the
///   rules allow and never leap an octave when another layout avoids it, so each player keeps a
///   tessitura (PLY-13).
/// - With `options.carries_bass`, `bassoon` doubles the lowest horn an octave below, or in
///   unison when the octave lies outside the bassoon's comfortable range; without it the bassoon
///   rests. The doubling is reported with the rules' named doubling.
///
/// Every given part appears in the result, the horns first in the order given; a chord no layout
/// fits is left out and reported.
///
/// # Errors
/// [`VoicingError::PartCount`] for fewer than two or more than four horns;
/// [`VoicingError::WrongInstrument`] for a horn part that does not play the horn in F or a
/// bassoon part that does not play the bassoon; [`VoicingError::DuplicatePart`];
/// [`VoicingError::InvalidVelocity`]; [`VoicingError::InvalidSlot`].
pub fn horns(
    chords: &[ChordSlot],
    horns: &[Carrier],
    bassoon: Option<&Carrier>,
    options: ChordOptions,
    ids: &mut dyn IdGenerator,
) -> Result<Voiced, VoicingError> {
    let mut carriers = horns.to_vec();
    carriers.extend(bassoon.cloned());
    check_inputs(&carriers, options.velocity)?;
    if !(HORNS.0..=HORNS.1).contains(&horns.len()) {
        return Err(VoicingError::PartCount {
            min: HORNS.0,
            max: HORNS.1,
            given: horns.len(),
        });
    }
    if let Some(carrier) = horns
        .iter()
        .find(|carrier| carrier.instrument != InstrumentType::HornF)
    {
        return Err(wrong(carrier));
    }
    if let Some(carrier) = bassoon.filter(|carrier| carrier.instrument != InstrumentType::Bassoon) {
        return Err(wrong(carrier));
    }
    let slots = sorted_slots(chords)?;
    // From the bottom: the reverse of the order from the top.
    let mut order: Vec<usize> = from_the_top(horns.len()).collect();
    order.reverse();
    let groups = order
        .iter()
        .map(|&place| VoiceGroup::solo(&carriers, place))
        .collect::<Result<Vec<_>, _>>()?;
    let mut score = Score::new(&carriers);
    let rules = ChordRules {
        carries_bass: options.carries_bass,
        accept: &|_| true,
        velocity: options.velocity,
    };
    let layouts = write_chords(&mut score, &slots, &groups, &rules, ids)?;
    let (Some(bassoon), Some(&lowest)) = (bassoon, order.first()) else {
        return Ok(score.finish());
    };
    let place = horns.len();
    if !options.carries_bass {
        return Ok(score.finish());
    }
    let range = comfortable(bassoon.instrument).ok_or_else(|| wrong(bassoon))?;
    for (slot, layout) in slots.iter().zip(&layouts) {
        let Some((&bass, horn_pitches)) = layout.as_ref().and_then(|pitches| pitches.split_first())
        else {
            continue;
        };
        let holds = |pitch: i64| range.low().get() <= pitch && pitch <= range.high().get();
        // The octave below must clear its low-interval limits against every horn (SPC-01).
        let clear = |pitch: i64| horn_pitches.iter().all(|&horn| clears_limits(pitch, horn));
        let (pitch, octaves) = if holds(bass - 12) && clear(bass - 12) {
            (bass - 12, -1)
        } else if holds(bass) {
            (bass, 0)
        } else {
            continue;
        };
        let note = new_note(ids, pitch, slot.start, slot.length, options.velocity)?;
        score.push(place, note);
        score.double(place, lowest, octaves, true);
    }
    Ok(score.finish())
}

/// What a part plays in a brass chorale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// The trumpets and their kin: the top parts.
    Trumpet,
    /// The horns: in unison pairs only.
    Horn,
    /// The trombones and the euphonium: the parts below the trumpets, the lowest on the bass.
    Trombone,
    /// The tuba and the cimbasso: an octave under the bass.
    Tuba,
}

impl Kind {
    /// The kind of `instrument`, or `None` for an instrument a chorale does not use.
    const fn of(instrument: InstrumentType) -> Option<Self> {
        use InstrumentType::{
            Cimbasso, Cornet, Euphonium, Flugelhorn, HornF, TromboneBass, TromboneTenor, TrumpetBb,
            TrumpetC, TrumpetPiccolo, Tuba,
        };
        match instrument {
            TrumpetBb | TrumpetC | TrumpetPiccolo | Cornet | Flugelhorn => Some(Self::Trumpet),
            HornF => Some(Self::Horn),
            TromboneTenor | TromboneBass | Euphonium => Some(Self::Trombone),
            Tuba | Cimbasso => Some(Self::Tuba),
            _ => None,
        }
    }
}

/// Voices `chords` as a brass chorale for the parts `brass`, each kind given in player order.
///
/// The chorale is in close order with the trumpets on top (Rimsky-Korsakov, Principles of
/// Orchestration, Ch. III, Harmony in the brass):
///
/// - each trumpet takes a part from the top, then each trombone but the last;
/// - the last trombone takes the bass, the first tuba an octave below it; without a trombone the
///   tuba takes the bass;
/// - the horns play in unison pairs, in the order they sit from the top (Horns 1 and 3, then 2
///   and 4): a pair takes a part of its own below the trumpets while the chorale has fewer than
///   four, and otherwise doubles an inner part, from the top, so no single horn holds a part at
///   forte against trumpets or trombones (BAL-02). A horn left without a partner rests;
/// - a second tuba, and a pair with no inner part left, rest.
///
/// The voices follow the shared rules of the family helpers, so no interval of the chorale lies
/// below its low limit (SPC-01). Every given part appears in the result in the order given.
///
/// # Errors
/// [`VoicingError::WrongInstrument`] for a part that is not a trumpet, cornet, flugelhorn, horn,
/// trombone, euphonium, tuba or cimbasso; [`VoicingError::TooFewVoices`] when the parts form fewer
/// than three voices; [`VoicingError::DuplicatePart`]; [`VoicingError::InvalidVelocity`];
/// [`VoicingError::InvalidSlot`].
pub fn brass_chorale(
    chords: &[ChordSlot],
    brass: &[Carrier],
    options: ChordOptions,
    ids: &mut dyn IdGenerator,
) -> Result<Voiced, VoicingError> {
    check_inputs(brass, options.velocity)?;
    let mut kinds = Vec::with_capacity(brass.len());
    for carrier in brass {
        kinds.push(Kind::of(carrier.instrument).ok_or_else(|| wrong(carrier))?);
    }
    let places = |kind: Kind| -> Vec<usize> {
        kinds
            .iter()
            .enumerate()
            .filter(|&(_, &k)| k == kind)
            .map(|(place, _)| place)
            .collect()
    };
    let horns = places(Kind::Horn);
    let tubas = places(Kind::Tuba);
    let mut trombones = places(Kind::Trombone);
    // Parts from the top, each a list of (place, semitones above the voice).
    let mut upper: Vec<Vec<(usize, i64)>> = places(Kind::Trumpet)
        .into_iter()
        .map(|place| vec![(place, 0)])
        .collect();
    let lowest: Option<Vec<(usize, i64)>> = match (trombones.pop(), tubas.first()) {
        (Some(trombone), Some(&tuba)) => Some(vec![(trombone, 0), (tuba, -12)]),
        (Some(trombone), None) => Some(vec![(trombone, 0)]),
        (None, Some(&tuba)) => Some(vec![(tuba, 0)]),
        (None, None) => None,
    };
    let middle: Vec<Vec<(usize, i64)>> = trombones.into_iter().map(|p| vec![(p, 0)]).collect();
    let order: Vec<usize> = from_the_top(horns.len())
        .filter_map(|index| horns.get(index).copied())
        .collect();
    let (pairs, lone) = order.as_chunks::<2>();
    let lone_horn = lone.first().copied();
    let formed = upper.len() + middle.len() + usize::from(lowest.is_some());
    let mut extra_pairs = Vec::new();
    for (index, pair) in pairs.iter().enumerate() {
        if formed + index < CHORALE_PARTS {
            upper.push(pair.iter().map(|&p| (p, 0)).collect());
        } else {
            extra_pairs.push(pair);
        }
    }
    let mut parts: Vec<Vec<(usize, i64)>> = upper;
    parts.extend(middle);
    parts.extend(lowest);
    if parts.len() < CHORALE_VOICES {
        return Err(VoicingError::TooFewVoices {
            needed: CHORALE_VOICES,
            formed: parts.len(),
        });
    }
    // Pairs left over double the inner parts in unison, from the top.
    let inner = parts.len().saturating_sub(2);
    for (index, pair) in extra_pairs.into_iter().enumerate().take(inner) {
        if let Some(part) = parts.get_mut(index + 1) {
            part.extend(pair.iter().map(|&p| (p, 0)));
        }
    }
    let mut groups = Vec::with_capacity(parts.len());
    for part in parts.iter().rev() {
        let Some(&(lead, _)) = part.first() else {
            continue;
        };
        let mut voice = VoiceGroup::solo(brass, lead)?;
        for &(place, offset) in part.iter().skip(1) {
            voice.join(brass, place, offset);
        }
        groups.push(voice);
    }
    let slots = sorted_slots(chords)?;
    let mut score = Score::new(brass);
    if let Some(horn) = lone_horn {
        score.silence(horn, RuleId::Bal02);
    }
    let rules = ChordRules {
        carries_bass: options.carries_bass,
        accept: &|_| true,
        velocity: options.velocity,
    };
    write_chords(&mut score, &slots, &groups, &rules, ids)?;
    Ok(score.finish())
}
