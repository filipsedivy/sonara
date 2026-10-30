//! Choosing the part that carries a melody and the doubling it gets.

use std::cmp::Reverse;

use sonara_core::ids::{self, IdGenerator};
use sonara_core::midi::MidiNote;

use super::{
    Carrier, DEFAULT_VELOCITY, Density, PartId, Remark, Score, Voiced, VoicingError, check_inputs,
    comfortable,
};
use crate::orchestration::rules::{
    NAMED_DOUBLINGS, NamedDoubling, Playing, RegisterBand, Role, RuleId, Selector, Spacing,
    UNDOUBLED_HIGH_MELODY, UNDOUBLED_LOW_MELODY, WOODWIND_ORDER, carriers,
};
use crate::orchestration::{Family, InstrumentType, SoundingPitch};

/// Semitones in an octave.
const OCTAVE: i64 = 12;

/// A melody's doubling: the notes of every part given, and what was chosen.
#[derive(Clone, Debug, PartialEq)]
pub struct MelodyDoubling {
    /// Every part given with its notes; the doublings name the carrier as the part they double.
    pub voiced: Voiced,
    /// The part that plays the melody at its written pitch.
    pub carrier: PartId,
    /// The doubling of the rules that was written, or `None` for the carrier alone.
    pub doubling: Option<&'static NamedDoubling>,
    /// The density written: the one asked for, or a thinner one when none fits
    /// ([`Remark::Thinned`]).
    pub density: Density,
}

/// One line of a doubling: a part, by its place among the carriers, and the semitones it plays
/// above the melody.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Line {
    place: usize,
    offset: i64,
}

/// A doubling that fits the parts: the row, the lines, and whether it gives the melody the
/// doubling its register or instrument wants.
struct Candidate {
    row: usize,
    lines: Vec<Line>,
    meets_need: bool,
}

/// The span of a melody as written.
struct Melody {
    lowest: i64,
    highest: i64,
}

impl Melody {
    /// Whether `carrier` plays the melody `offset` semitones up inside its comfortable range.
    fn fits(&self, carrier: &Carrier, offset: i64) -> bool {
        comfortable(carrier.instrument).is_some_and(|range| {
            range.low().get() <= self.lowest + offset && self.highest + offset <= range.high().get()
        })
    }
}

/// Doubles the melody `melody` among the parts `parts`, given in score order, at `density`.
///
/// 1. **Carrier.** The function table's row for a melody in the melody's register band (the band
///    of the middle of its range) lists the instruments that carry it, strings first; the first
///    given part of the first listed instrument that plays every note inside its comfortable
///    range carries the melody at pitch, and a part of an unlisted instrument comes after them.
///    A melody is never moved to fit.
/// 2. **Doubling.** [`Density::Thin`] writes the carrier alone. [`Density::Medium`] writes the
///    named doubling of two members, in unison or at the octave, that brings in the most parts:
///    every other part of a member's instruments that fits joins it, so the second violins play
///    with the first. [`Density::Full`] writes the three groups' unison of a tutti, strings,
///    woodwinds and brass. Among equal choices the table's order decides.
/// 3. **Rules.** No doubling inverts the woodwind order (DBL-02), puts two players of one
///    woodwind in octaves without strings (DBL-03), puts a woodwind an octave above a trombone
///    without a trumpet (DBL-04), or sets strings and brass in unison without woodwind (DBL-09).
///    A violin or woodwind melody above MIDI 90 is doubled an octave below, and one below MIDI 42
///    an octave above, even at [`Density::Thin`] (RNG-04); a double-bass melody gets cellos or a
///    bassoon (DBL-08). When no part can give that doubling, [`Remark::Undoubled`] says so.
///
/// Every part that plays, the carrier too, gets copies of the melody's notes under fresh ids,
/// moved by the part's octaves, with the timing, velocity, channel and articulation of the
/// melody's notes. A part given nothing rests.
///
/// # Errors
/// [`VoicingError::EmptyMelody`]; [`VoicingError::NoCarrier`] when no part plays the melody
/// inside its comfortable range; [`VoicingError::DuplicatePart`].
pub fn melody_doublings(
    melody: &[MidiNote],
    parts: &[Carrier],
    density: Density,
    ids: &mut dyn IdGenerator,
) -> Result<MelodyDoubling, VoicingError> {
    check_inputs(parts, DEFAULT_VELOCITY)?;
    let pitches = melody.iter().map(|note| i64::from(note.pitch));
    let (Some(lowest), Some(highest)) = (pitches.clone().min(), pitches.max()) else {
        return Err(VoicingError::EmptyMelody);
    };
    let line = Melody { lowest, highest };
    let main = main_carrier(&line, parts).ok_or(VoicingError::NoCarrier)?;
    let main_carrier = parts.get(main).ok_or(VoicingError::NoCarrier)?;
    let need = need(&line, main_carrier.instrument);
    let mut remarks = Vec::new();
    let mut written = density;
    let mut chosen: Option<Candidate> = None;
    if written == Density::Full {
        chosen = best(&line, parts, main, Density::Full, need);
        if chosen.is_none() {
            written = Density::Medium;
        }
    }
    if written == Density::Medium && chosen.is_none() {
        chosen = best(&line, parts, main, Density::Medium, need);
        if chosen.is_none() {
            written = Density::Thin;
        }
    }
    if density == Density::Thin && need.is_some() {
        // A thin melody takes only the doubling a rule wants.
        chosen =
            best(&line, parts, main, Density::Thin, need).filter(|candidate| candidate.meets_need);
    }
    if written != density {
        remarks.push(Remark::Thinned {
            asked: density,
            written,
        });
    }
    if let Some(rule) = need
        && !chosen
            .as_ref()
            .is_some_and(|candidate| candidate.meets_need)
    {
        remarks.push(Remark::Undoubled { rule });
    }
    let lines = chosen.as_ref().map_or_else(
        || {
            vec![Line {
                place: main,
                offset: 0,
            }]
        },
        |candidate| candidate.lines.clone(),
    );
    let named = chosen
        .as_ref()
        .and_then(|candidate| NAMED_DOUBLINGS.get(candidate.row));
    let mut score = Score::new(parts);
    for line in &lines {
        for note in melody {
            score.push(line.place, moved(note, line.offset, ids)?);
        }
        if line.place != main {
            let octaves = i8::try_from(line.offset / OCTAVE).unwrap_or_default();
            score.double_as(line.place, main, octaves, named);
        }
    }
    score.remarks = remarks;
    Ok(MelodyDoubling {
        voiced: score.finish(),
        carrier: main_carrier.part.clone(),
        doubling: named,
        density: written,
    })
}

/// The place of the part that carries the melody: the first part, in the function table's order
/// of instruments for the melody's band and then in the order given, that plays every note
/// inside its comfortable range.
fn main_carrier(melody: &Melody, parts: &[Carrier]) -> Option<usize> {
    let middle = SoundingPitch::new(i64::midpoint(melody.lowest, melody.highest))?;
    let order =
        carriers(Role::Melody, RegisterBand::of(middle)).map_or(&[][..], |row| row.instruments);
    let rank = |instrument: InstrumentType| {
        order
            .iter()
            .position(|&listed| listed == instrument)
            .unwrap_or(order.len())
    };
    parts
        .iter()
        .enumerate()
        .filter(|(_, carrier)| melody.fits(carrier, 0))
        .min_by_key(|&(place, carrier)| (rank(carrier.instrument), place))
        .map(|(place, _)| place)
}

/// The rule that wants the melody doubled whatever the density: RNG-04 for a violin or woodwind
/// melody above MIDI 90 or any melody below MIDI 42, DBL-08 for a double-bass melody.
fn need(melody: &Melody, instrument: InstrumentType) -> Option<RuleId> {
    let high = (instrument == InstrumentType::Violin || instrument.family() == Family::Woodwinds)
        && melody.highest > UNDOUBLED_HIGH_MELODY.value.get();
    if high || melody.lowest < UNDOUBLED_LOW_MELODY.value.get() {
        Some(RuleId::Rng04)
    } else if instrument == InstrumentType::DoubleBass {
        Some(RuleId::Dbl08)
    } else {
        None
    }
}

/// Whether `lines` give the melody the doubling `rule` wants.
fn meets(rule: RuleId, melody: &Melody, lines: &[Line], parts: &[Carrier]) -> bool {
    match rule {
        RuleId::Rng04 => {
            let high = melody.highest > UNDOUBLED_HIGH_MELODY.value.get();
            let wanted = if high { -OCTAVE } else { OCTAVE };
            lines.iter().any(|line| line.offset == wanted)
        }
        RuleId::Dbl08 => lines.iter().any(|line| {
            line.offset.abs() <= OCTAVE
                && parts.get(line.place).is_some_and(|carrier| {
                    matches!(
                        carrier.instrument,
                        InstrumentType::Cello | InstrumentType::Bassoon
                    )
                })
        }),
        _ => true,
    }
}

/// The doubling of `density` that fits the parts best: the one that meets the melody's need,
/// then the one with the most lines, then the earliest in the table.
fn best(
    melody: &Melody,
    parts: &[Carrier],
    main: usize,
    density: Density,
    need: Option<RuleId>,
) -> Option<Candidate> {
    let main_instrument = parts.get(main)?.instrument;
    let mut candidates = Vec::new();
    for (row_index, row) in NAMED_DOUBLINGS.iter().enumerate() {
        if row.builds_bass {
            continue;
        }
        for &spacing in row.spacings {
            if !allows(density, row, spacing) {
                continue;
            }
            let matching =
                row.members.iter().enumerate().filter(|(_, member)| {
                    member.selector.matches(Playing::ordinary(main_instrument))
                });
            for (member, _) in matching {
                let joins = density != Density::Thin;
                let Some(lines) = assign(melody, parts, row, spacing, (main, member), joins) else {
                    continue;
                };
                if breaks_rules(&lines, parts) {
                    continue;
                }
                let meets_need = need.is_none_or(|rule| meets(rule, melody, &lines, parts));
                candidates.push(Candidate {
                    row: row_index,
                    lines,
                    meets_need,
                });
            }
        }
    }
    // The first of the best: `min_by_key` keeps the earliest of equal keys.
    candidates.into_iter().min_by_key(|candidate| {
        (
            Reverse(candidate.meets_need),
            Reverse(candidate.lines.len()),
            candidate.row,
        )
    })
}

/// Whether `density` writes the doubling `row` at `spacing`.
fn allows(density: Density, row: &NamedDoubling, spacing: Spacing) -> bool {
    match density {
        Density::Thin | Density::Medium => {
            row.members.len() == 2 && matches!(spacing, Spacing::Unison | Spacing::Octaves(1))
        }
        Density::Full => spacing == Spacing::Unison && is_three_groups(row),
    }
}

/// Whether the members of `row` are strings, woodwinds and brass, one group each.
fn is_three_groups(row: &NamedDoubling) -> bool {
    let family = |selector: Selector| match selector {
        Selector::Family(family) => Some(family),
        other => other.ids().first().map(|instrument| instrument.family()),
    };
    let families: Vec<Option<Family>> = row
        .members
        .iter()
        .map(|member| family(member.selector))
        .collect();
    families.len() == 3
        && [Family::Strings, Family::Woodwinds, Family::Brass]
            .iter()
            .all(|group| families.contains(&Some(*group)))
}

/// The lines of the doubling `row` at `spacing` with the part at `main.0` as its member
/// `main.1`, or `None` when the parts cannot fill every member inside their comfortable ranges.
/// With `joins`, every other part that matches a member, fits and is not counted out joins it.
fn assign(
    melody: &Melody,
    parts: &[Carrier],
    row: &NamedDoubling,
    spacing: Spacing,
    (main, main_member): (usize, usize),
    joins: bool,
) -> Option<Vec<Line>> {
    let step = match spacing {
        Spacing::Unison => 0,
        Spacing::Octaves(octaves) => OCTAVE * i64::from(octaves),
        Spacing::Chord => return None,
    };
    // Each member lies `step` above the next one in the list.
    let offset = |member: usize| {
        let (main_member, member) = (
            i64::try_from(main_member).ok()?,
            i64::try_from(member).ok()?,
        );
        Some((main_member - member) * step)
    };
    let mut taken = vec![false; parts.len()];
    *taken.get_mut(main)? = true;
    let mut lines = vec![Line {
        place: main,
        offset: 0,
    }];
    let mut counts = vec![0_usize; row.members.len()];
    *counts.get_mut(main_member)? = 1;
    let candidate = |taken: &[bool], member: Selector, offset: i64| {
        parts.iter().enumerate().position(|(place, carrier)| {
            !taken.get(place).copied().unwrap_or(true)
                && member.matches(Playing::ordinary(carrier.instrument))
                && melody.fits(carrier, offset)
        })
    };
    for (index, member) in row.members.iter().enumerate() {
        let fewest = member
            .players
            .map_or(1, |players| usize::from(players.min()));
        let offset = offset(index)?;
        while counts.get(index).copied()? < fewest {
            let place = candidate(&taken, member.selector, offset)?;
            *taken.get_mut(place)? = true;
            *counts.get_mut(index)? += 1;
            lines.push(Line { place, offset });
        }
    }
    if joins {
        for (index, member) in row.members.iter().enumerate() {
            let most = member
                .players
                .map_or(usize::MAX, |players| usize::from(players.max()));
            let offset = offset(index)?;
            while counts.get(index).copied()? < most {
                let Some(place) = candidate(&taken, member.selector, offset) else {
                    break;
                };
                *taken.get_mut(place)? = true;
                *counts.get_mut(index)? += 1;
                lines.push(Line { place, offset });
            }
        }
    }
    Some(lines)
}

/// Whether the lines break a doubling rule: the woodwind order inverted (DBL-02), two players of
/// one woodwind in octaves without strings (DBL-03), a woodwind an octave above a trombone
/// without a trumpet (DBL-04), or strings and brass in unison without woodwind (DBL-09).
fn breaks_rules(lines: &[Line], parts: &[Carrier]) -> bool {
    use InstrumentType::{TromboneBass, TromboneTenor, TrumpetBb, TrumpetC};
    let played: Vec<(InstrumentType, i64)> = lines
        .iter()
        .filter_map(|line| {
            parts
                .get(line.place)
                .map(|carrier| (carrier.instrument, line.offset))
        })
        .collect();
    let woodwind = |instrument: InstrumentType| instrument.family() == Family::Woodwinds;
    let has_strings = played
        .iter()
        .any(|&(instrument, _)| instrument.family() == Family::Strings);
    let has_trumpet = played
        .iter()
        .any(|&(instrument, _)| matches!(instrument, TrumpetBb | TrumpetC));
    played.iter().any(|&(upper, high)| {
        played.iter().any(|&(lower, low)| {
            let inverted = high > low
                && woodwind(upper)
                && woodwind(lower)
                && WOODWIND_ORDER.rank(upper) > WOODWIND_ORDER.rank(lower);
            let same_colour = high != low && upper == lower && woodwind(upper) && !has_strings;
            let above_trombone = high - low == OCTAVE
                && woodwind(upper)
                && matches!(lower, TromboneTenor | TromboneBass)
                && !has_trumpet;
            let unblended = high == low
                && upper.family() == Family::Strings
                && lower.family() == Family::Brass
                && !played
                    .iter()
                    .any(|&(other, at)| at == high && woodwind(other));
            inverted || same_colour || above_trombone || unblended
        })
    })
}

/// `note` moved by `offset` semitones under a fresh id, every other field kept.
fn moved(
    note: &MidiNote,
    offset: i64,
    ids: &mut dyn IdGenerator,
) -> Result<MidiNote, VoicingError> {
    Ok(MidiNote::new(
        ids.new_id(ids::NOTE),
        i64::from(note.pitch) + offset,
        note.start,
        note.length,
        i64::from(note.velocity),
        i64::from(note.channel),
    )?
    .with_articulation(note.articulation.clone())?)
}
