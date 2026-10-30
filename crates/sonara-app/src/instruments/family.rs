//! Instrument families, and the guess of a family from a patch's path.
//!
//! The family is the orchestration metadata's own enum, re-exported here as [`InstrumentFamily`],
//! so a preset and an instrument type name their family with one type. Its serialized form is
//! the lowercase slug (`"strings"`, `"woodwinds"`, …), which manifests declare and family claims
//! write. The guess reads a path with the metadata's own names and aliases ([`resolve`]), so a
//! patch path names a family through the same aliases as any other text read with them.

use serde::Serialize;
use sonara_midi::orchestration::{Lang, Mention, Target, resolve};

/// The orchestral family of an instrument: [`sonara_midi::orchestration::Family`].
pub use sonara_midi::orchestration::Family as InstrumentFamily;

/// What is known about the family of a preset. A manifest's word is trusted; a guess from the
/// path is not, so the two stay apart (`contracts/orchestration-metadata.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "source", content = "family", rename_all = "lowercase")]
pub enum FamilyClaim {
    /// Declared by the library's manifest.
    Declared(InstrumentFamily),
    /// Guessed from the words of the patch's path.
    Inferred(InstrumentFamily),
    /// Neither declared nor recognised.
    Unknown,
}

impl FamilyClaim {
    /// The family, whatever its source; `None` when unknown.
    #[must_use]
    pub const fn family(self) -> Option<InstrumentFamily> {
        match self {
            Self::Declared(family) | Self::Inferred(family) => Some(family),
            Self::Unknown => None,
        }
    }
}

/// The languages path words are read in. Library paths are written in English, and the aliases
/// of other languages collide with English words: the French `alto` is the viola, the English one
/// a voice.
const PATH_LANGUAGES: &[Lang] = &[Lang::En];

/// Guesses the family from the words of `relative_path` (`/`-separated, below the library root).
///
/// Each segment is split into words where a new one starts inside a run of letters and digits
/// (`TubularBells`, `Horn2`; see [`split_words`]) and read with the English names and aliases of
/// the orchestration metadata ([`resolve`]). A word that names instruments of more than one family
/// is no clue: `bass` is the double bass and the bass voice. A segment decides when its clues agree
/// on one family. The file name is read first, then the folders from the nearest to the farthest,
/// so `Strings/Solo Violin.sfz` is decided by the file name, and `Brass/Patch 1.sfz` and
/// `Strings/Bass.sfz` by the folder. A path that no segment decides gives
/// [`FamilyClaim::Unknown`]; what one decides is only ever [`FamilyClaim::Inferred`], a guess.
#[must_use]
pub(crate) fn infer_family(relative_path: &str) -> FamilyClaim {
    relative_path
        .rsplit('/')
        .find_map(family_of_segment)
        .map_or(FamilyClaim::Unknown, FamilyClaim::Inferred)
}

/// The family the clues of `segment` agree on; `None` when it holds no clue or its clues name
/// different families (`Contrabass Clarinet`: the double bass and a clarinet).
fn family_of_segment(segment: &str) -> Option<InstrumentFamily> {
    let mut agreed = None;
    for family in resolve(&split_words(segment), PATH_LANGUAGES)
        .iter()
        .filter_map(family_of_mention)
    {
        if !joins(&mut agreed, family) {
            return None;
        }
    }
    agreed
}

/// The one family every target of `mention` belongs to; `None` when they span several. A
/// category belongs to its members' family, and a family alias to that family.
fn family_of_mention(mention: &Mention) -> Option<InstrumentFamily> {
    let mut shared = None;
    for &(target, _) in mention.matches() {
        let fits = match target {
            Target::Instrument(instrument) => joins(&mut shared, instrument.family()),
            Target::Category(category) => category
                .get()
                .members()
                .all(|member| joins(&mut shared, member.family())),
            Target::Family(family) => joins(&mut shared, family),
        };
        if !fits {
            return None;
        }
    }
    shared
}

/// Records `family` in `agreed` when nothing is recorded yet, and tells whether the two agree.
fn joins(agreed: &mut Option<InstrumentFamily>, family: InstrumentFamily) -> bool {
    *agreed.get_or_insert(family) == family
}

/// `segment` with a space before every word that starts inside a run of letters and digits: at a
/// lower-case letter followed by a capital (`TubularBells`), before the last capital of a run of
/// capitals that a lower-case letter follows (`SViolin`, `GMStyle`), and between a letter and a
/// digit either way (`Horn2`, `3d`). Everything else is kept as it is; [`resolve`] folds the
/// result, so punctuation and spaces already separate words.
fn split_words(segment: &str) -> String {
    let mut words = String::with_capacity(segment.len().saturating_mul(2));
    let mut chars = segment.chars().peekable();
    let mut previous: Option<char> = None;
    while let Some(current) = chars.next() {
        if previous.is_some_and(|previous| starts_word(previous, current, chars.peek().copied())) {
            words.push(' ');
        }
        words.push(current);
        previous = Some(current);
    }
    words
}

/// Whether `current`, between `previous` and `next`, starts a new word (see [`split_words`]).
fn starts_word(previous: char, current: char, next: Option<char>) -> bool {
    (previous.is_lowercase() && current.is_uppercase())
        || (previous.is_uppercase()
            && current.is_uppercase()
            && next.is_some_and(char::is_lowercase))
        || (previous.is_alphabetic() && current.is_numeric())
        || (previous.is_numeric() && current.is_alphabetic())
}

#[cfg(test)]
mod tests {
    use sonara_midi::orchestration::{InstrumentType, Lang, Target, all_aliases, resolve};

    use super::{FamilyClaim, InstrumentFamily, infer_family, split_words};

    fn inferred(path: &str) -> FamilyClaim {
        infer_family(path)
    }

    #[test]
    fn the_file_name_decides_before_the_folders() {
        assert_eq!(
            inferred("Strings/Solo Violin.sfz"),
            FamilyClaim::Inferred(InstrumentFamily::Strings)
        );
        assert_eq!(
            inferred("Brass/Cello_Oddity.sfz"),
            FamilyClaim::Inferred(InstrumentFamily::Strings)
        );
    }

    #[test]
    fn a_folder_decides_when_the_file_name_says_nothing() {
        assert_eq!(
            inferred("Library/Brass/Patch 1.sfz"),
            FamilyClaim::Inferred(InstrumentFamily::Brass)
        );
    }

    #[test]
    fn compound_names_go_to_the_right_family() {
        let cases = [
            ("Bass Drum.sfz", InstrumentFamily::Percussion),
            ("English Horn.sfz", InstrumentFamily::Woodwinds),
            ("Bassoon.sfz", InstrumentFamily::Woodwinds),
            ("Double Bass.sfz", InstrumentFamily::Strings),
            ("French-Horn-Solo.sfz", InstrumentFamily::Brass),
            ("Harp.sfz", InstrumentFamily::Plucked),
            ("Grand Piano.sfz", InstrumentFamily::Keyboards),
        ];
        for (path, family) in cases {
            assert_eq!(inferred(path), FamilyClaim::Inferred(family), "{path}");
        }
    }

    #[test]
    fn words_are_matched_whole() {
        // `hornet` contains `horn` and `bassline` contains `bass`: neither counts.
        assert_eq!(inferred("Hornet.sfz"), FamilyClaim::Unknown);
        assert_eq!(inferred("Bassline.sfz"), FamilyClaim::Unknown);
    }

    /// Electric keyboards and tuned percussion are named by the metadata's aliases, nicknames
    /// included. Sonatina keeps its celesta in `Percussion/`: the file name now decides, where
    /// the folder decided before (TST-06).
    #[test]
    fn keyboard_and_tuned_percussion_names_are_recognised() {
        let cases = [
            ("Rhodes.sfz", InstrumentFamily::Keyboards),
            (
                "Wurlitzer EP200/Wurlitzer EP200.sfz",
                InstrumentFamily::Keyboards,
            ),
            ("Clavinet.sfz", InstrumentFamily::Keyboards),
            ("Clav.sfz", InstrumentFamily::Keyboards),
            ("Percussion/Celeste.sfz", InstrumentFamily::Keyboards),
            ("Chimes.sfz", InstrumentFamily::Percussion),
            ("Crotales.sfz", InstrumentFamily::Percussion),
            ("Carillon.sfz", InstrumentFamily::Percussion),
            ("Vibes.sfz", InstrumentFamily::Percussion),
            ("Glock.sfz", InstrumentFamily::Percussion),
        ];
        for (path, family) in cases {
            assert_eq!(inferred(path), FamilyClaim::Inferred(family), "{path}");
        }
    }

    #[test]
    fn words_split_at_capitals_and_digits() {
        let cases = [
            ("TubularBells", "Tubular Bells"),
            ("FHornSus", "F Horn Sus"),
            ("SViolin-KS", "S Violin-KS"),
            ("GM-StylePerc", "GM-Style Perc"),
            ("VSUpright1", "VS Upright 1"),
            ("jRhodes3d-keyswitch", "j Rhodes 3 d-keyswitch"),
            ("Wurlitzer EP200.sfz", "Wurlitzer EP 200.sfz"),
            ("Solo Violin", "Solo Violin"),
            ("TIMPANI", "TIMPANI"),
            ("", ""),
        ];
        for (segment, words) in cases {
            assert_eq!(split_words(segment), words, "{segment}");
        }
    }

    /// File names written as one run, as VSCO 2 CE writes them, are read word by word.
    #[test]
    fn run_together_names_are_read_word_by_word() {
        let cases = [
            ("TubularBells.sfz", InstrumentFamily::Percussion),
            ("FHornSus.sfz", InstrumentFamily::Brass),
            ("TrumpetHarmonMuteSus.sfz", InstrumentFamily::Brass),
            ("ViolinEns-KS.sfz", InstrumentFamily::Strings),
            ("SViolin-KS.sfz", InstrumentFamily::Strings),
            ("jRhodes3d-keyswitch.sfz", InstrumentFamily::Keyboards),
            ("Horn2.sfz", InstrumentFamily::Brass),
        ];
        for (path, family) in cases {
            assert_eq!(inferred(path), FamilyClaim::Inferred(family), "{path}");
        }
    }

    /// The splitter separates digits from letters, which is safe only while no alias holds a
    /// digit.
    #[test]
    fn no_alias_holds_a_digit() {
        for (target, alias) in all_aliases() {
            assert!(
                !alias.text.bytes().any(|byte| byte.is_ascii_digit()),
                "{target:?}: {}",
                alias.text
            );
        }
    }

    /// Only English counts: `Geige` and `housle` name the violin in German and Czech, and the
    /// French `alto` is the viola, but a path read in English finds no violin in the first two
    /// and the alto voice in the third.
    #[test]
    fn only_english_names_count() {
        let violin = [Target::Instrument(InstrumentType::Violin)];
        assert_eq!(resolve("Geige", &[Lang::De])[0].targets(), violin);
        assert_eq!(resolve("housle", &[Lang::Cs])[0].targets(), violin);
        assert_eq!(inferred("Geige.sfz"), FamilyClaim::Unknown);
        assert_eq!(inferred("housle.sfz"), FamilyClaim::Unknown);
        assert_eq!(
            inferred("Alto.sfz"),
            FamilyClaim::Inferred(InstrumentFamily::Choir)
        );
    }

    /// `bass` names the double bass and the bass voice, so it is no clue. A bare `Bass.sfz` was
    /// a string instrument before; it is unknown now, because the metadata lists two families
    /// for the word and the guess would be wrong for every bass voice.
    #[test]
    fn a_word_of_two_families_is_no_clue() {
        assert_eq!(inferred("Bass.sfz"), FamilyClaim::Unknown);
        assert_eq!(
            inferred("Strings/Bass.sfz"),
            FamilyClaim::Inferred(InstrumentFamily::Strings)
        );
        assert_eq!(
            inferred("Choir/Basses.sfz"),
            FamilyClaim::Inferred(InstrumentFamily::Choir)
        );
        assert_eq!(
            inferred("Bass Guitar.sfz"),
            FamilyClaim::Inferred(InstrumentFamily::Plucked)
        );
    }

    /// A file name whose clues name two families decides nothing, and its folders may.
    #[test]
    fn a_name_whose_clues_disagree_leaves_it_to_the_folders() {
        assert_eq!(inferred("Contrabass Clarinet.sfz"), FamilyClaim::Unknown);
        assert_eq!(
            inferred("Woodwinds/Contrabass Clarinet.sfz"),
            FamilyClaim::Inferred(InstrumentFamily::Woodwinds)
        );
        assert_eq!(
            inferred("Chordophones/Zithers/Harpsichord, English - Lute.sfz"),
            FamilyClaim::Unknown
        );
    }

    /// Every English alias, as a file name, infers the family its targets share, and a text
    /// whose targets span families infers nothing (oracle: the targets in `all_aliases`).
    #[test]
    fn every_english_alias_infers_the_family_of_its_targets() {
        let english: Vec<_> = all_aliases()
            .filter(|(_, alias)| alias.lang == Lang::En)
            .collect();
        assert!(english.len() > 100, "{} English aliases", english.len());
        for (_, alias) in &english {
            let mut families: Vec<InstrumentFamily> = english
                .iter()
                .filter(|(_, other)| other.text == alias.text)
                .flat_map(|&(target, _)| match target {
                    Target::Instrument(instrument) => vec![instrument.family()],
                    Target::Category(category) => category
                        .get()
                        .members()
                        .map(InstrumentType::family)
                        .collect(),
                    Target::Family(family) => vec![family],
                })
                .collect();
            families.sort();
            families.dedup();
            let expected = match families[..] {
                [family] => FamilyClaim::Inferred(family),
                _ => FamilyClaim::Unknown,
            };
            assert_eq!(
                inferred(&format!("{}.sfz", alias.text)),
                expected,
                "{}",
                alias.text
            );
        }
    }

    #[test]
    fn an_unrecognised_path_is_unknown_not_guessed() {
        assert_eq!(inferred("Pad 7.sfz"), FamilyClaim::Unknown);
        assert_eq!(inferred(""), FamilyClaim::Unknown);
        assert_eq!(FamilyClaim::Unknown.family(), None);
    }

    /// A family serializes as its lowercase slug, and a claim as its source with that slug, the
    /// form the catalog has always written (TST-05).
    #[test]
    fn every_claim_keeps_its_json_form() {
        let families = [
            (InstrumentFamily::Strings, "strings"),
            (InstrumentFamily::Woodwinds, "woodwinds"),
            (InstrumentFamily::Brass, "brass"),
            (InstrumentFamily::Percussion, "percussion"),
            (InstrumentFamily::Keyboards, "keyboards"),
            (InstrumentFamily::Plucked, "plucked"),
            (InstrumentFamily::Choir, "choir"),
            (InstrumentFamily::Other, "other"),
        ];
        for (family, slug) in families {
            assert_eq!(
                serde_json::to_string(&family).expect("a family serializes"),
                format!("\"{slug}\"")
            );
            assert_eq!(
                serde_json::to_string(&FamilyClaim::Declared(family)).expect("a claim serializes"),
                format!(r#"{{"source":"declared","family":"{slug}"}}"#)
            );
            assert_eq!(
                serde_json::to_string(&FamilyClaim::Inferred(family)).expect("a claim serializes"),
                format!(r#"{{"source":"inferred","family":"{slug}"}}"#)
            );
        }
        assert_eq!(
            serde_json::to_string(&FamilyClaim::Unknown).expect("a claim serializes"),
            r#"{"source":"unknown"}"#
        );
    }

    /// The families sort in the catalog's order, strings first, not in the score order of the
    /// instrument types, which starts with the woodwinds.
    #[test]
    fn families_sort_in_the_catalog_order() {
        let mut families = [
            InstrumentFamily::Other,
            InstrumentFamily::Choir,
            InstrumentFamily::Plucked,
            InstrumentFamily::Keyboards,
            InstrumentFamily::Percussion,
            InstrumentFamily::Brass,
            InstrumentFamily::Woodwinds,
            InstrumentFamily::Strings,
        ];
        families.sort();
        assert_eq!(
            families,
            [
                InstrumentFamily::Strings,
                InstrumentFamily::Woodwinds,
                InstrumentFamily::Brass,
                InstrumentFamily::Percussion,
                InstrumentFamily::Keyboards,
                InstrumentFamily::Plucked,
                InstrumentFamily::Choir,
                InstrumentFamily::Other,
            ]
        );
    }
}
