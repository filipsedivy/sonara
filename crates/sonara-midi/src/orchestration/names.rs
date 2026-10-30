//! Names and aliases: what an instrument, a category of instruments or a family is called.
//!
//! [`Names`] are for display, in five languages, and are never identifiers. An [`Alias`] is one
//! form a person may write to mean a target, such as the Czech "lesních rohů" for the horns.
//! Aliases are matching data, not display text:
//!
//! - **Stored folded.** The text is what [`fold`] makes of the form: lower-case ASCII letters
//!   and digits, words separated by one space. "Lesní rohy" is stored as `lesni rohy`, "Hörner"
//!   as `horner`, "Clarinet in B♭" as `clarinet in bb`. Diacritics are dropped; `ß`, `œ`, `æ`,
//!   `ø`, `ł`, `♭` and `♯` are written `ss`, `oe`, `ae`, `o`, `l`, `b` and `is`; and every run of
//!   other characters is one space.
//! - **Written out.** Every form is listed: singular and plural, the Czech cases singular and
//!   plural (nominative, genitive, dative, accusative, locative and instrumental), synonyms and
//!   score abbreviations. No form is derived by code.
//! - **At least three letters.** One- and two-letter abbreviations (`Vn.`, `Hn.`, `S.`) collide
//!   with ordinary words, such as the Czech "a" or the German "ob", and are never aliases.
//! - **Ambiguity is data.** One text may name several targets, in one language or across
//!   languages (`bass`: the double bass and the bass voice; `alto`: the French viola and the
//!   English voice). Every target is listed; nothing picks one.
//!
//! [`resolve`] finds the aliases in a text: it folds the text and, reading left to right, takes
//! the longest alias that starts at each word, in the languages asked for. Each find is a
//! [`Mention`] with every target its words name.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::LazyLock;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use super::data;
use super::id::{CategoryId, InstrumentType, same_text};
use super::instrument::{Family, InstrumentError};

/// The fewest letters an alias holds; digits and spaces do not count.
const MIN_ALIAS_LETTERS: usize = 3;

/// The characters that compatibility decomposition (NFKD) leaves whole, in lower case, with the
/// ASCII they fold to. `♯` folds to the German `is`, so "F♯" folds like "Fis".
const SPECIAL_LETTERS: [(char, &str); 7] = [
    ('ß', "ss"),
    ('œ', "oe"),
    ('æ', "ae"),
    ('ø', "o"),
    ('ł', "l"),
    ('♭', "b"),
    ('♯', "is"),
];

/// `text` in the folded form aliases are stored in: lower-case ASCII letters and digits, words
/// separated by one space, no space at either end.
///
/// The text is decomposed (Unicode NFKD) and its combining marks are dropped, so "Lesní rohy"
/// becomes `lesni rohy` and "Hörner" `horner`. Every character is lower-cased, and the seven
/// that decomposition leaves whole are spelled out: `ß` as `ss`, `œ` as `oe`, `æ` as `ae`, `ø`
/// as `o`, `ł` as `l`, `♭` as `b` and `♯` as `is` ("Clarinet in B♭" becomes `clarinet in bb`).
/// Every run of other characters (spaces, punctuation, letters of other scripts) separates two
/// words. Folding a folded text gives it back unchanged; a text with nothing that folds to an
/// ASCII letter or digit folds to the empty string.
///
/// The decomposition tables are those of `unicode-normalization`. Not for real-time code: the
/// result is allocated (RT-02).
#[must_use]
pub fn fold(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    let mut separated = false;
    for c in text.nfkd().flat_map(char::to_lowercase) {
        if is_combining_mark(c) {
            continue;
        }
        let special = SPECIAL_LETTERS
            .iter()
            .find_map(|&(letter, ascii)| (letter == c).then_some(ascii));
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || special.is_some()) {
            separated = true;
            continue;
        }
        if separated && !folded.is_empty() {
            folded.push(' ');
        }
        separated = false;
        match special {
            Some(ascii) => folded.push_str(ascii),
            None => folded.push(c),
        }
    }
    folded
}

/// A run of words of a text that names one or more targets: the result of [`resolve`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mention {
    words: Range<usize>,
    text: &'static str,
    matches: Vec<(Target, &'static Alias)>,
}

impl Mention {
    /// The positions of the words the mention covers among the words of the folded text, that
    /// is among `fold(text).split(' ')` for the text given to [`resolve`].
    #[must_use]
    pub fn words(&self) -> Range<usize> {
        self.words.clone()
    }

    /// The folded alias text the words spell, for example `lesni rohy`.
    #[must_use]
    pub const fn text(&self) -> &'static str {
        self.text
    }

    /// Every target the words name with the alias that names it, in the order of
    /// [`all_aliases`]; never empty. A target is listed once per language its alias belongs to,
    /// and each alias tells the language and the grammatical number of the form.
    #[must_use]
    pub fn matches(&self) -> &[(Target, &'static Alias)] {
        &self.matches
    }

    /// The targets the words name, each once, in the order of [`all_aliases`]. More than one
    /// target means the words are ambiguous, and the one who wrote them should be asked.
    #[must_use]
    pub fn targets(&self) -> Vec<Target> {
        let mut targets: Vec<Target> = Vec::with_capacity(self.matches.len());
        for &(target, _) in &self.matches {
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
        targets
    }
}

/// The instruments, categories and families that `text` names in the given languages, left to
/// right.
///
/// The text is folded ([`fold`]) and read word by word. At each word the longest alias that
/// starts there wins ("bass clarinet" over "bass", the Czech "anglický roh" over a shorter
/// form); its words become one [`Mention`] and reading continues after them. A word that starts
/// no alias is skipped. Only aliases in `languages` count, so an empty slice finds nothing; pass
/// [`Lang::ALL`] for every language. A text that names several targets yields one mention with
/// all of them, in data order; nothing picks one. No form is guessed: there is no stemming and
/// no edit distance, so a misspelt or uninflected word that is not listed names nothing.
///
/// Not for real-time code: the first call builds an index of every alias behind a lazy lock
/// (RT-03), and every call allocates (RT-02).
#[must_use]
pub fn resolve(text: &str, languages: &[Lang]) -> Vec<Mention> {
    let index = &*ALIAS_INDEX;
    let folded = fold(text);
    let words = word_spans(&folded);
    let mut mentions = Vec::new();
    let mut start = 0;
    while start < words.len() {
        let longest = index.max_words.min(words.len() - start);
        match (1..=longest)
            .rev()
            .find_map(|count| index.mention(&folded, &words, start..start + count, languages))
        {
            Some(mention) => {
                start = mention.words.end;
                mentions.push(mention);
            }
            None => start += 1,
        }
    }
    mentions
}

/// The byte ranges of the words of a folded text.
fn word_spans(folded: &str) -> Vec<Range<usize>> {
    let mut offset = 0;
    folded
        .split(' ')
        .filter_map(|word| {
            let span = offset..offset + word.len();
            offset = span.end + 1;
            (!word.is_empty()).then_some(span)
        })
        .collect()
}

/// Every alias of the dataset by its text, built on first use.
static ALIAS_INDEX: LazyLock<AliasIndex> = LazyLock::new(AliasIndex::build);

/// The aliases by text, each text with its targets in the order of [`all_aliases`].
struct AliasIndex {
    by_text: HashMap<&'static str, Vec<(Target, &'static Alias)>>,
    /// The number of words of the longest alias.
    max_words: usize,
}

impl AliasIndex {
    fn build() -> Self {
        let mut by_text: HashMap<&'static str, Vec<(Target, &'static Alias)>> = HashMap::new();
        let mut max_words = 0;
        for (target, alias) in all_aliases() {
            max_words = max_words.max(alias.text.split(' ').count());
            by_text.entry(alias.text).or_default().push((target, alias));
        }
        Self { by_text, max_words }
    }

    /// The mention of the words at positions `words` of `folded` (whose word spans are `spans`),
    /// or `None` when they spell no alias in `languages`.
    fn mention(
        &self,
        folded: &str,
        spans: &[Range<usize>],
        words: Range<usize>,
        languages: &[Lang],
    ) -> Option<Mention> {
        let first = spans.get(words.start)?;
        let last = spans.get(words.end.checked_sub(1)?)?;
        let (&text, entries) = self
            .by_text
            .get_key_value(folded.get(first.start..last.end)?)?;
        let matches: Vec<(Target, &'static Alias)> = entries
            .iter()
            .filter(|(_, alias)| languages.contains(&alias.lang))
            .copied()
            .collect();
        (!matches.is_empty()).then_some(Mention {
            words,
            text,
            matches,
        })
    }
}

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
