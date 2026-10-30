//! The harp's pedals: a setting for each passage of a harp part, and the places no setting
//! covers.
//!
//! A pedal harp has seven strings to the octave, one per letter name, and one pedal per letter
//! that sets every string of that name flat, natural or sharp. A setting therefore sounds at most
//! seven pitch classes, and a pitch class can often be spelled two ways: C♯ or D♭, E♯ or F.

use sonara_core::midi::MidiNote;

use super::{CheckError, Context, Shortfall, Uncovered, is_valid, note_order, onsets, pitch};
use crate::orchestration::{FactKind, InstrumentType, LetterName, Span};
use crate::performance::ScoreContext;

/// Semitones in an octave.
const OCTAVE: i64 = 12;

/// The number of pedal settings: three positions for each of seven pedals.
const SETTINGS: usize = 3_usize.pow(7);

/// The position of one harp pedal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Accidental {
    /// The pedal up: the strings sound a semitone below natural.
    Flat,
    /// The pedal in the middle notch.
    Natural,
    /// The pedal down: the strings sound a semitone above natural.
    Sharp,
}

impl Accidental {
    /// The three positions, from the top notch.
    pub const ALL: [Self; 3] = [Self::Flat, Self::Natural, Self::Sharp];

    /// The slug, such as `flat`.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::Natural => "natural",
            Self::Sharp => "sharp",
        }
    }

    /// The semitones the position moves a string from natural.
    const fn offset(self) -> i64 {
        match self {
            Self::Flat => -1,
            Self::Natural => 0,
            Self::Sharp => 1,
        }
    }
}

/// The positions of the seven pedals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PedalSetting([Accidental; 7]);

impl PedalSetting {
    /// Every pedal natural: the C major setting.
    pub const NATURAL: Self = Self([Accidental::Natural; 7]);

    /// The setting with `pedals` for the letters C, D, E, F, G, A and B, in the order of
    /// [`LetterName::ALL`].
    #[must_use]
    pub const fn new(pedals: [Accidental; 7]) -> Self {
        Self(pedals)
    }

    /// The position of the pedal of `letter`.
    #[must_use]
    pub const fn get(self, letter: LetterName) -> Accidental {
        self.0[letter as usize]
    }

    /// The pitch class, 0 for C to 11 for B, that the strings of `letter` sound.
    #[must_use]
    pub fn pitch_class(self, letter: LetterName) -> u8 {
        let class = (natural(letter) + self.get(letter).offset()).rem_euclid(OCTAVE);
        // The remainder lies in 0-11, so the conversion never falls back.
        u8::try_from(class).unwrap_or(0)
    }

    /// Whether a string sounds pitch class `class` (0-11).
    #[must_use]
    pub fn sounds(self, class: u8) -> bool {
        LetterName::ALL
            .into_iter()
            .any(|letter| self.pitch_class(letter) == class)
    }

    /// The pedals set differently in `other`, in the order of [`LetterName::ALL`].
    #[must_use]
    pub fn changed_from(self, other: Self) -> Vec<LetterName> {
        LetterName::ALL
            .into_iter()
            .filter(|&letter| self.get(letter) != other.get(letter))
            .collect()
    }

    /// The pitch classes the setting sounds, one bit per class from C.
    fn mask(self) -> u16 {
        LetterName::ALL
            .into_iter()
            .fold(0, |mask, letter| mask | 1 << self.pitch_class(letter))
    }

    /// How many pedals are not natural.
    fn altered(self) -> usize {
        self.0
            .iter()
            .filter(|&&position| position != Accidental::Natural)
            .count()
    }

    /// Whether the setting has both a flat and a sharp.
    fn is_mixed(self) -> bool {
        self.0.contains(&Accidental::Flat) && self.0.contains(&Accidental::Sharp)
    }
}

/// One passage of a harp part with the pedal setting that plays it.
#[derive(Clone, Debug, PartialEq)]
pub struct PedalPassage {
    /// Where the passage starts, in content beats: its first note's start.
    pub start: f64,
    /// Where it ends: its last note's end.
    pub end: f64,
    /// The setting.
    pub setting: PedalSetting,
    /// The pedals moved from the setting of the passage before; none for the first passage,
    /// whose setting the player makes before playing.
    pub changed: Vec<LetterName>,
}

/// The harp's pedal settings for a part.
#[derive(Clone, Debug, PartialEq)]
pub struct HarpPedals {
    /// The passages in order, each with its setting.
    pub passages: Vec<PedalPassage>,
    /// The notes no setting covers ([`Shortfall::Pitches`]) and the changes the music leaves too
    /// little time for ([`Shortfall::Time`]), in order.
    pub uncovered: Vec<Uncovered>,
}

/// Derives a pedal setting for each passage of the harp notes `notes`, whose beats are content
/// beats of the clip that `score` places.
///
/// - **Passages.** From the first note, a passage takes in each next group of notes struck
///   together for as long as one setting sounds every pitch class of the passage; the next group
///   starts a new passage. So the part changes its pedals as seldom as passages of this kind
///   allow. A group of notes that no setting plays together, such as six semitones in a row, is
///   reported and ends the passage before it.
/// - **Time.** Each foot changes one pedal at a time: the left foot D, C and B, the right foot E,
///   F, G and A, each change taking the time the harp's record states. A foot's changes fit
///   between the last pluck of a string it moves, at its old tuning, and the first note that needs
///   one at its new tuning; when the music leaves less, the change is reported.
/// - **Setting.** Of the settings that play a passage, the one whose changes fit the time on the
///   most feet, then with the fewest pedals moved from the setting before, then the fewest pedals
///   off natural, then one without both flats and sharps, then sharps before flats: which
///   spelling a harpist would choose is an estimate.
///
/// # Errors
/// [`CheckError::InvalidNote`] for the first invalid note.
pub fn harp_pedals(notes: &[MidiNote], score: &ScoreContext) -> Result<HarpPedals, CheckError> {
    if let Some(note) = notes.iter().find(|note| !is_valid(note)) {
        return Err(CheckError::InvalidNote(note.id.clone()));
    }
    let context = Context {
        score,
        parts: Vec::new(),
    };
    let mut sorted: Vec<&MidiNote> = notes.iter().collect();
    sorted.sort_by(|a, b| note_order(a, b));
    let settings: Vec<(PedalSetting, u16)> = (0..SETTINGS)
        .map(setting_at)
        .map(|setting| (setting, setting.mask()))
        .collect();
    let covers = |mask: u16| settings.iter().any(|&(_, sounds)| sounds & mask == mask);
    let (groups, mut uncovered) = passages_of(&sorted, covers);

    let feet = pedal_feet();
    let mut passages: Vec<PedalPassage> = Vec::with_capacity(groups.len());
    let mut last_pluck: [Option<f64>; 7] = [None; 7];
    let mut previous: Option<PedalSetting> = None;
    for passage in &groups {
        let mask = passage.iter().fold(0, |mask, group| mask | group.mask);
        let late = |setting: PedalSetting| -> Vec<Uncovered> {
            let (Some(before), Some((left, right, change))) = (previous, feet) else {
                return Vec::new();
            };
            let changed = setting.changed_from(before);
            [left, right]
                .into_iter()
                .filter_map(|foot| {
                    let moved: Vec<LetterName> = changed
                        .iter()
                        .copied()
                        .filter(|letter| foot.contains(letter))
                        .collect();
                    late_change(&context, passage, setting, &moved, &last_pluck, change)
                })
                .collect()
        };
        let before = previous.unwrap_or(PedalSetting::NATURAL);
        let Some((setting, lates)) = settings
            .iter()
            .enumerate()
            .filter(|(_, (_, sounds))| sounds & mask == mask)
            .map(|(index, &(setting, _))| (index, setting, late(setting)))
            .min_by_key(|(index, setting, lates)| {
                (
                    lates.len(),
                    setting.changed_from(before).len(),
                    setting.altered(),
                    setting.is_mixed(),
                    *index,
                )
            })
            .map(|(_, setting, lates)| (setting, lates))
        else {
            continue;
        };
        uncovered.extend(lates);
        let changed = if previous.is_some() {
            setting.changed_from(before)
        } else {
            Vec::new()
        };
        for group in passage {
            for letter in LetterName::ALL {
                if group.mask & 1 << setting.pitch_class(letter) != 0
                    && let Some(slot) = last_pluck.get_mut(letter as usize)
                {
                    *slot = Some(group.start);
                }
            }
        }
        let start = passage.first().map_or(0.0, |group| group.start);
        let end = passage.iter().map(|group| group.end).fold(start, f64::max);
        passages.push(PedalPassage {
            start,
            end,
            setting,
            changed,
        });
        previous = Some(setting);
    }
    uncovered.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.end.total_cmp(&b.end)));
    Ok(HarpPedals {
        passages,
        uncovered,
    })
}

/// The notes in passages of groups struck together: each passage as long as one setting covers
/// it (`covers`), and the groups no setting covers on their own, each ending the passage before
/// it.
fn passages_of<'a>(
    sorted: &[&'a MidiNote],
    covers: impl Fn(u16) -> bool,
) -> (Vec<Vec<Group<'a>>>, Vec<Uncovered>) {
    let mut groups: Vec<Vec<Group<'_>>> = Vec::new();
    let mut uncovered = Vec::new();
    let mut current: Vec<Group<'_>> = Vec::new();
    let mut current_mask = 0_u16;
    for onset in onsets(sorted) {
        let group = Group::new(onset);
        if !covers(group.mask) {
            groups.push(std::mem::take(&mut current));
            current_mask = 0;
            uncovered.push(Uncovered::of(&group.notes, Shortfall::Pitches));
            continue;
        }
        if !covers(current_mask | group.mask) {
            groups.push(std::mem::take(&mut current));
            current_mask = 0;
        }
        current_mask |= group.mask;
        current.push(group);
    }
    groups.push(current);
    groups.retain(|passage| !passage.is_empty());
    (groups, uncovered)
}

/// The notes struck together, with the pitch classes they sound.
struct Group<'a> {
    start: f64,
    end: f64,
    mask: u16,
    notes: Vec<&'a MidiNote>,
}

impl<'a> Group<'a> {
    fn new(notes: Vec<&'a MidiNote>) -> Self {
        let start = notes.first().map_or(0.0, |note| note.start);
        let end = notes.iter().map(|note| note.end()).fold(start, f64::max);
        let mask = notes.iter().fold(0_u16, |mask, note| {
            mask | 1 << pitch(note).rem_euclid(OCTAVE)
        });
        Self {
            start,
            end,
            mask,
            notes,
        }
    }
}

/// The change of the pedals `moved`, all under one foot, before `passage` with its `setting`, when
/// the music leaves the foot less time than the changes take one after another.
fn late_change(
    context: &Context<'_>,
    passage: &[Group<'_>],
    setting: PedalSetting,
    moved: &[LetterName],
    last_pluck: &[Option<f64>; 7],
    change: Span,
) -> Option<Uncovered> {
    let latest = moved
        .iter()
        .filter_map(|&letter| last_pluck.get(letter as usize).copied().flatten())
        .reduce(f64::max)?;
    let first_need = passage.iter().find(|group| {
        moved
            .iter()
            .any(|&letter| group.mask & 1 << setting.pitch_class(letter) != 0)
    })?;
    let count = u32::try_from(moved.len()).unwrap_or(u32::MAX);
    let needed = change.millis().saturating_mul(count);
    let free = context.millis(latest, first_need.start);
    (free < f64::from(needed)).then(|| Uncovered {
        start: latest,
        end: first_need.start,
        notes: first_need
            .notes
            .iter()
            .map(|note| note.id.clone())
            .collect(),
        shortfall: Shortfall::Time {
            needed_ms: needed,
            free_ms: free,
        },
    })
}

/// The harp's pedals under the left and the right foot and the time one change takes, from its
/// record; `None` if the record states none.
fn pedal_feet() -> Option<(&'static [LetterName], &'static [LetterName], Span)> {
    InstrumentType::Harp
        .get()
        .facts()
        .iter()
        .find_map(|fact| match fact.kind {
            FactKind::HarpPedals {
                left,
                right,
                change,
            } => Some((left, right, change)),
            _ => None,
        })
}

/// The setting numbered `index` among the 3^7: each letter from C a digit in base 3 from the
/// lowest, 0 natural, 1 sharp, 2 flat, so that a lower number prefers sharps and the lower
/// letters.
fn setting_at(index: usize) -> PedalSetting {
    let mut pedals = [Accidental::Natural; 7];
    let mut rest = index;
    for pedal in &mut pedals {
        *pedal = match rest % 3 {
            0 => Accidental::Natural,
            1 => Accidental::Sharp,
            _ => Accidental::Flat,
        };
        rest /= 3;
    }
    PedalSetting(pedals)
}

/// The pitch class of `letter`'s natural string.
const fn natural(letter: LetterName) -> i64 {
    match letter {
        LetterName::C => 0,
        LetterName::D => 2,
        LetterName::E => 4,
        LetterName::F => 5,
        LetterName::G => 7,
        LetterName::A => 9,
        LetterName::B => 11,
    }
}

#[cfg(test)]
mod tests {
    use super::{Accidental, PedalSetting, SETTINGS, setting_at};
    use crate::orchestration::LetterName;

    #[test]
    fn settings_are_numbered_once_each() {
        // TST-07: the 2187 numbers give 2187 different settings, the first all natural.
        let mut all: Vec<PedalSetting> = (0..SETTINGS).map(setting_at).collect();
        assert_eq!(all.first(), Some(&PedalSetting::NATURAL));
        all.sort_by_key(|setting| format!("{setting:?}"));
        all.dedup();
        assert_eq!(all.len(), SETTINGS);
    }

    #[test]
    fn pedals_move_strings_by_a_semitone() {
        // TST-10: C flat sounds B, B sharp sounds C, F sharp sounds F#.
        let mut pedals = [Accidental::Natural; 7];
        pedals[0] = Accidental::Flat;
        pedals[3] = Accidental::Sharp;
        pedals[6] = Accidental::Sharp;
        let setting = PedalSetting::new(pedals);
        assert_eq!(setting.pitch_class(LetterName::C), 11);
        assert_eq!(setting.pitch_class(LetterName::F), 6);
        assert_eq!(setting.pitch_class(LetterName::B), 0);
        assert_eq!(setting.pitch_class(LetterName::D), 2);
        assert!(setting.sounds(0) && setting.sounds(11) && !setting.sounds(5));
        assert_eq!(setting.altered(), 3);
        assert!(setting.is_mixed());
        assert_eq!(
            setting.changed_from(PedalSetting::NATURAL),
            vec![LetterName::C, LetterName::F, LetterName::B]
        );
    }
}
