//! Names and aliases: what an instrument, a category of instruments or a family is called.
//!
//! [`Names`] are for display, in five languages, and are never identifiers. An [`Alias`] is one
//! form a person may write to mean a target, such as the Czech "lesních rohů" for the horns.
//! Aliases are matching data, not display text:
//!
//! - **Stored folded.** The text is lower-case ASCII letters and digits, words separated by one
//!   space: "Lesní rohy" is stored as `lesni rohy`, "Hörner" as `horner`, "Clarinet in B♭" as
//!   `clarinet in bb`. Diacritics are dropped, `ß` is written `ss`, `œ` `oe`, `♭` `b`, and every
//!   run of other characters is one space.
//! - **Written out.** Every form is listed: singular and plural, the Czech cases singular and
//!   plural (nominative, genitive, dative, accusative, locative and instrumental), synonyms and
//!   score abbreviations. No form is derived by code.
//! - **At least three letters.** One- and two-letter abbreviations (`Vn.`, `Hn.`, `S.`) collide
//!   with ordinary words, such as the Czech "a" or the German "ob", and are never aliases.
//! - **Ambiguity is data.** One text may name several targets, in one language or across
//!   languages (`bass`: the double bass and the bass voice; `alto`: the French viola and the
//!   English voice). Every target is listed; nothing picks one.

use super::data;
use super::id::{CategoryId, InstrumentType, same_text};
use super::instrument::{Family, InstrumentError};

/// The fewest letters an alias holds; digits and spaces do not count.
const MIN_ALIAS_LETTERS: usize = 3;

/// The display names of an instrument in the five languages of [`Lang`], and the abbreviation
/// a score prints. Display text only, never an identifier.
///
/// A name is the first form the sources give, in their spelling and capitalization, without the
/// alternates they add in parentheses: those, such as the German "Geige" beside "Violine", are
/// aliases of the record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Names {
    /// English.
    pub en: &'static str,
    /// Czech.
    pub cs: &'static str,
    /// German.
    pub de: &'static str,
    /// French.
    pub fr: &'static str,
    /// Italian, the language of most score labels.
    pub it: &'static str,
    /// The abbreviation a score prints, such as `Vn.` or `Cl. (B♭)`.
    pub abbreviation: &'static str,
}

impl Names {
    /// The display name in `lang`.
    #[must_use]
    pub const fn get(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::En => self.en,
            Lang::Cs => self.cs,
            Lang::De => self.de,
            Lang::Fr => self.fr,
            Lang::It => self.it,
        }
    }

    /// Whether any of the five names or the abbreviation is empty or blank.
    pub(super) const fn has_blank(&self) -> bool {
        is_blank(self.en)
            || is_blank(self.cs)
            || is_blank(self.de)
            || is_blank(self.fr)
            || is_blank(self.it)
            || is_blank(self.abbreviation)
    }
}

/// A language the names and aliases cover.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    /// English.
    En,
    /// Czech.
    Cs,
    /// German.
    De,
    /// French.
    Fr,
    /// Italian.
    It,
}

impl Lang {
    /// Every language, in declaration order.
    pub const ALL: [Self; 5] = [Self::En, Self::Cs, Self::De, Self::Fr, Self::It];

    /// The ISO 639-1 code, for example `cs`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Cs => "cs",
            Self::De => "de",
            Self::Fr => "fr",
            Self::It => "it",
        }
    }
}

/// The grammatical number of an alias form. It tells a solo voice from a choir section ("tenor"
/// is the solo voice, "tenors" the section) and one instrument from several.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Number {
    /// Names one, such as "horn".
    Singular,
    /// Names several, such as "horns" or the Czech "lesních rohů".
    Plural,
    /// The form is both: a word with no singular, such as the Czech "housle", a form shared by a
    /// singular and a plural case, such as the Czech "flétny", or a score abbreviation.
    Unmarked,
}

/// One form of a name, stored folded (see the module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Alias {
    /// The folded text, for example `lesnich rohu`.
    pub text: &'static str,
    /// The language the form belongs to.
    pub lang: Lang,
    /// Its grammatical number.
    pub number: Number,
}

/// What an alias names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    /// One instrument type.
    Instrument(InstrumentType),
    /// A category: several types one word names, with a rule for choosing among them.
    Category(CategoryId),
    /// A family: the parts of that family in the ensemble in use.
    Family(Family),
}

/// Every alias of the dataset with the target it names, in data order: the instruments in score
/// order, then the categories in the order of [`CategoryId::ALL`], then the families. Within a
/// target the aliases keep their listed order.
pub fn all_aliases() -> impl Iterator<Item = (Target, &'static Alias)> {
    let instruments = data::INSTRUMENTS.iter().flat_map(|record| {
        let target = Target::Instrument(record.instrument_type());
        record.aliases().iter().map(move |alias| (target, alias))
    });
    let categories = CategoryId::ALL.into_iter().flat_map(|id| {
        let target = Target::Category(id);
        id.get().aliases().iter().map(move |alias| (target, alias))
    });
    let families = data::FAMILY_ALIASES.iter().flat_map(|&(family, aliases)| {
        aliases
            .iter()
            .map(move |alias| (Target::Family(family), alias))
    });
    instruments.chain(categories).chain(families)
}

/// The first fault of `aliases`, checked alias by alias in list order and then for duplicates:
/// [`AliasNotFolded`](InstrumentError::AliasNotFolded) for a text that is not in folded form,
/// [`AliasTooShort`](InstrumentError::AliasTooShort) for one with fewer than three letters, and
/// [`DuplicateAlias`](InstrumentError::DuplicateAlias) for a text listed twice in one language.
pub(super) const fn alias_fault(aliases: &[Alias]) -> Option<InstrumentError> {
    let mut i = 0;
    while i < aliases.len() {
        let text = aliases[i].text;
        if !is_folded(text) {
            return Some(InstrumentError::AliasNotFolded(text));
        }
        if letter_count(text) < MIN_ALIAS_LETTERS {
            return Some(InstrumentError::AliasTooShort(text));
        }
        i += 1;
    }
    let mut i = 0;
    while i < aliases.len() {
        let mut j = i + 1;
        while j < aliases.len() {
            if aliases[i].lang as u8 == aliases[j].lang as u8
                && same_text(aliases[i].text, aliases[j].text)
            {
                return Some(InstrumentError::DuplicateAlias(aliases[i].text));
            }
            j += 1;
        }
        i += 1;
    }
    None
}

/// Whether `text` is in folded form: non-empty, only `a-z`, `0-9` and spaces, no space at either
/// end and never two in a row.
const fn is_folded(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.is_empty() || bytes[0] == b' ' || bytes[bytes.len() - 1] == b' ' {
        return false;
    }
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b' ' {
            if bytes[i - 1] == b' ' {
                return false;
            }
        } else if !(b.is_ascii_lowercase() || b.is_ascii_digit()) {
            return false;
        }
        i += 1;
    }
    true
}

/// The number of letters `a-z` in `text`.
const fn letter_count(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_lowercase() {
            count += 1;
        }
        i += 1;
    }
    count
}

/// Whether `text` is empty or holds only ASCII whitespace.
const fn is_blank(text: &str) -> bool {
    text.trim_ascii().is_empty()
}
