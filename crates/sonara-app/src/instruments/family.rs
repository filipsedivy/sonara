//! Instrument families, and the guess of a family from a patch's path.

use serde::{Deserialize, Serialize};

/// The orchestral family of an instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstrumentFamily {
    /// Bowed strings: violin, viola, cello, double bass, string ensembles.
    Strings,
    /// Woodwinds: flute, oboe, clarinet, bassoon and relatives.
    Woodwinds,
    /// Brass: horn, trumpet, trombone, tuba.
    Brass,
    /// Tuned and untuned percussion: timpani, drums, bells, cymbals.
    Percussion,
    /// Piano, organ, harpsichord, celesta.
    Keyboards,
    /// Plucked strings: harp, guitar.
    Plucked,
    /// Voices.
    Choir,
    /// Anything else that a manifest declares.
    Other,
}

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

/// Words that name a family. The groups are tried in this order, so `bass drum` is percussion
/// and `english horn` a woodwind before `bass` or `horn` can claim them.
const KEYWORDS: &[(InstrumentFamily, &[&str])] = &[
    (
        InstrumentFamily::Percussion,
        &[
            "timpani",
            "percussion",
            "drum",
            "drums",
            "snare",
            "cymbal",
            "cymbals",
            "gong",
            "bells",
            "glockenspiel",
            "xylophone",
            "marimba",
            "vibraphone",
            "triangle",
            "tamtam",
        ],
    ),
    (
        InstrumentFamily::Woodwinds,
        &[
            "woodwind",
            "woodwinds",
            "flute",
            "piccolo",
            "oboe",
            "clarinet",
            "bassoon",
            "contrabassoon",
            "anglais",
            "english",
            "recorder",
            "saxophone",
        ],
    ),
    (
        InstrumentFamily::Brass,
        &[
            "brass",
            "horn",
            "horns",
            "trumpet",
            "trumpets",
            "trombone",
            "trombones",
            "tuba",
            "cornet",
        ],
    ),
    (
        InstrumentFamily::Strings,
        &[
            "strings",
            "violin",
            "violins",
            "viola",
            "violas",
            "cello",
            "cellos",
            "contrabass",
            "bass",
            "basses",
            "fiddle",
        ],
    ),
    (
        InstrumentFamily::Keyboards,
        &["piano", "organ", "harpsichord", "celesta", "keyboard"],
    ),
    (InstrumentFamily::Plucked, &["harp", "guitar", "mandolin"]),
    (
        InstrumentFamily::Choir,
        &["choir", "voice", "vocal", "chorus"],
    ),
];

/// Guesses the family from the words of `relative_path` (`/`-separated, below the library root).
///
/// The file name is read first, then the folders from the nearest to the farthest, so
/// `Strings/Solo Violin.sfz` is decided by the file name and `Brass/Patch 1.sfz` by the folder.
/// Words are whole alphanumeric runs in lower case; a name that holds no known word gives
/// [`FamilyClaim::Unknown`].
#[must_use]
pub(crate) fn infer_family(relative_path: &str) -> FamilyClaim {
    relative_path
        .rsplit('/')
        .find_map(family_of_segment)
        .map_or(FamilyClaim::Unknown, FamilyClaim::Inferred)
}

fn family_of_segment(segment: &str) -> Option<InstrumentFamily> {
    let words: Vec<String> = segment
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect();
    KEYWORDS
        .iter()
        .find(|(_, names)| words.iter().any(|word| names.contains(&word.as_str())))
        .map(|(family, _)| *family)
}

#[cfg(test)]
mod tests {
    use super::{FamilyClaim, InstrumentFamily, infer_family};

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

    #[test]
    fn an_unrecognised_path_is_unknown_not_guessed() {
        assert_eq!(inferred("Pad 7.sfz"), FamilyClaim::Unknown);
        assert_eq!(inferred(""), FamilyClaim::Unknown);
        assert_eq!(FamilyClaim::Unknown.family(), None);
    }
}
