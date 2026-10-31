//! The questions a request can leave open, and the "did you mean" suggestions for a word that
//! names no instrument.

use std::fmt;

use sonara_midi::orchestration::{Alias, CategoryId, Target, all_aliases};

use super::words::LANGUAGES;

/// The most suggestions one unknown word gets.
pub const MAX_SUGGESTIONS: usize = 3;

/// The fewest letters a word needs before suggestions are looked for: shorter words are within an
/// edit or two of too many aliases ("tim", from the Czech "pod tím", is one edit from "timp").
const MIN_SUGGESTION_LETTERS: usize = 4;

/// What a request leaves open. Nothing is to be changed while a question is open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Question {
    /// Words where an instrument belongs name none: after a count ("two saxhorns"), beside an
    /// instrument in a list ("horns and trumpts"), after "with", or before the verb or the
    /// "underneath" of a clause that names no instrument ("saxhorns play the melody").
    UnknownInstrument {
        /// The words, folded.
        words: String,
        /// Aliases close to the words, nearest first; empty when none is close.
        suggestions: Vec<Suggestion>,
    },
    /// The words name several instruments, categories or families ("bass": the double bass or
    /// the bass voice).
    Ambiguous {
        /// The words, folded.
        words: &'static str,
        /// Every target they name, in data order.
        targets: Vec<Target>,
    },
    /// The words name a category whose members have no usual one ("bells", "solo voice"): the one
    /// who asked chooses.
    ChooseMember {
        /// The words, folded.
        words: &'static str,
        /// The category.
        category: CategoryId,
    },
    /// "horn" or "horns" in a request that names nothing else orchestral: the French horn, or the
    /// brass and saxophone section that popular music calls "horns"?
    HornSense {
        /// The words, folded.
        words: &'static str,
    },
    /// A clause gives a role or a manner of playing but names no instrument, and there is no
    /// earlier clause it continues.
    NoInstruments {
        /// The clause as written.
        clause: String,
    },
    /// One clause asks for two different things of one kind ("legato staccato", "pp ff").
    Conflict {
        /// What the two words set.
        aspect: Aspect,
        /// The words that set it, folded, in the order written.
        said: Vec<String>,
    },
    /// A stretch of bars that is not a range: a bar 0, or a last bar before the first.
    InvalidBars {
        /// The first bar as written.
        first: u32,
        /// The last bar as written.
        last: u32,
    },
    /// A count of no players ("0 horns").
    InvalidCount {
        /// The instrument's words, folded.
        words: &'static str,
    },
}

/// What a word of a clause sets, for [`Question::Conflict`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Aspect {
    /// The role: melody, harmony, bass, …
    Role,
    /// The articulation of each note.
    Articulation,
    /// The mute.
    Mute,
    /// The vibrato.
    Vibrato,
    /// The notated dynamic.
    Dynamic,
    /// The change of dynamic.
    Hairpin,
    /// How thickly the line is doubled.
    Density,
    /// The bars.
    Bars,
    /// The key.
    Key,
}

impl Aspect {
    /// The aspect in words, plural: "roles", "dynamics".
    #[must_use]
    pub const fn plural(self) -> &'static str {
        match self {
            Self::Role => "roles",
            Self::Articulation => "articulations",
            Self::Mute => "mutes",
            Self::Vibrato => "vibrato settings",
            Self::Dynamic => "dynamics",
            Self::Hairpin => "changes of dynamic",
            Self::Density => "densities",
            Self::Bars => "bars",
            Self::Key => "keys",
        }
    }
}

/// An alias close to a word that names nothing: a "did you mean" offer, never a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Suggestion {
    /// The alias, folded.
    pub alias: &'static Alias,
    /// What the alias names.
    pub target: Target,
    /// The edit distance between the word and the alias (Damerau-Levenshtein: an insertion,
    /// deletion, substitution or swap of two neighbouring letters is one edit).
    pub distance: usize,
}

impl Question {
    /// The question in English, for the one who asked.
    #[must_use]
    pub fn text(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for Question {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownInstrument { words, suggestions } if suggestions.is_empty() => write!(
                f,
                "I don't know the instrument '{words}'. Which instrument do you mean?"
            ),
            Self::UnknownInstrument { words, suggestions } => {
                let offers: Vec<String> = suggestions
                    .iter()
                    .map(|s| format!("'{}' ({})", s.alias.text, target_name(s.target)))
                    .collect();
                write!(
                    f,
                    "I don't know the instrument '{words}'. Did you mean {}?",
                    either(&offers)
                )
            }
            Self::Ambiguous { words, targets } => {
                let names: Vec<String> = targets.iter().map(|&t| target_label(t)).collect();
                write!(
                    f,
                    "'{words}' can mean {}. Which one do you mean?",
                    either(&names)
                )
            }
            Self::ChooseMember { words, category } => {
                let members: Vec<String> = category
                    .get()
                    .members()
                    .map(|member| member.get().name().to_owned())
                    .collect();
                write!(
                    f,
                    "'{words}' does not say which one. Do you mean {}?",
                    either(&members)
                )
            }
            Self::HornSense { words } => write!(
                f,
                "By '{words}', do you mean French horns, or a brass and saxophone section?"
            ),
            Self::NoInstruments { clause } => {
                write!(
                    f,
                    "'{clause}' names no instrument. Which instruments should play it?"
                )
            }
            Self::Conflict { aspect, said } => {
                let quoted: Vec<String> = said.iter().map(|word| format!("'{word}'")).collect();
                write!(
                    f,
                    "{} ask for different {}. Which one do you want?",
                    both(&quoted),
                    aspect.plural()
                )
            }
            Self::InvalidBars { first, last } => write!(
                f,
                "Bars {first} to {last} are not a stretch of bars: bars count from 1, and the last \
                 comes after the first. Which bars do you mean?"
            ),
            Self::InvalidCount { words } => {
                write!(
                    f,
                    "How many '{words}' do you want? None is no part to play."
                )
            }
        }
    }
}

/// The English name of a target: an instrument's display name, a category's or a family's slug in
/// words ("solo voice", "strings").
#[must_use]
pub fn target_name(target: Target) -> String {
    match target {
        Target::Instrument(instrument) => instrument.get().name().to_owned(),
        Target::Category(category) => category.slug().replace('_', " "),
        Target::Family(family) => family.slug().to_owned(),
    }
}

/// A target's name with what tells it apart from a target of a similar name: an instrument's id
/// ("Bass (`voice_bass`)", "Double bass (`double_bass`)"), or whether it is a category or a
/// family.
fn target_label(target: Target) -> String {
    let name = target_name(target);
    match target {
        Target::Instrument(instrument) => format!("{name} ({})", instrument.id()),
        Target::Category(_) => format!("{name} (category)"),
        Target::Family(_) => format!("{name} (family)"),
    }
}

/// `items` joined as alternatives: "a", "a or b", "a, b or c".
fn either(items: &[String]) -> String {
    join(items, "or")
}

/// `items` joined as a list: "a and b", "a, b and c".
fn both(items: &[String]) -> String {
    join(items, "and")
}

fn join(items: &[String], last: &str) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [init @ .., tail] => format!("{} {last} {tail}", init.join(", ")),
    }
}

/// The aliases (English and Czech) close to `words`, nearest first, one per target, at most
/// [`MAX_SUGGESTIONS`].
///
/// Words of fewer than four letters get none. Words of four or five letters take aliases one edit
/// away, longer words aliases up to two edits away. Equal distances keep the data order of
/// `all_aliases` (TST-04). The distance is only ever used to suggest, never to match: "viola" and
/// "violin" are one edit apart.
pub(super) fn suggestions(words: &str) -> Vec<Suggestion> {
    let letters = words.bytes().filter(u8::is_ascii_lowercase).count();
    if letters < MIN_SUGGESTION_LETTERS {
        return Vec::new();
    }
    let max = if letters <= 5 { 1 } else { 2 };
    let mut close: Vec<(usize, Suggestion)> = all_aliases()
        .filter(|(_, alias)| LANGUAGES.contains(&alias.lang))
        .enumerate()
        .filter_map(|(order, (target, alias))| {
            let distance = strsim::damerau_levenshtein(words, alias.text);
            (distance <= max).then_some((
                order,
                Suggestion {
                    alias,
                    target,
                    distance,
                },
            ))
        })
        .collect();
    close.sort_by_key(|&(order, suggestion)| (suggestion.distance, order));
    let mut chosen: Vec<Suggestion> = Vec::with_capacity(MAX_SUGGESTIONS);
    for (_, suggestion) in close {
        if chosen.len() == MAX_SUGGESTIONS {
            break;
        }
        if !chosen.iter().any(|kept| kept.target == suggestion.target) {
            chosen.push(suggestion);
        }
    }
    chosen
}
