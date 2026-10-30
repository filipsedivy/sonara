//! What a version 2 manifest, or a library profile Sonara ships, claims about a preset and its
//! library, as validated domain values (TYP-06). The manifest's spelling of each value is in
//! `docs/instrument-catalog.md`.

use serde::Deserialize;
use sonara_midi::orchestration::{Articulation, InstrumentType, Mute, Vibrato};

/// The articulation an articulation entry names: the instrument's ordinary sound, or an id of
/// the articulation vocabulary ([`Articulation`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArticulationName {
    /// `ordinario`: the patch's ordinary sound, for a patch that no better id describes. The
    /// vocabulary has no such id; a request without an articulation asks for this sound.
    Ordinario,
    /// An id of the vocabulary, for example `pizzicato`.
    Vocabulary(Articulation),
}

impl ArticulationName {
    /// The manifest's spelling: the id parsed by [`Self::parse`].
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ordinario => "ordinario",
            Self::Vocabulary(articulation) => articulation.id(),
        }
    }

    /// The name with manifest spelling `id`: `ordinario` or an exact vocabulary id
    /// ([`Articulation::from_id`]). `None` for anything else.
    #[must_use]
    pub fn parse(id: &str) -> Option<Self> {
        if id == "ordinario" {
            Some(Self::Ordinario)
        } else {
            Articulation::from_id(id).map(Self::Vocabulary)
        }
    }
}

/// How the sampler reaches an articulation of a patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArticulationVia {
    /// The whole patch is this articulation; in a multi-patch set, the member patch is.
    Patch,
    /// A key switch: the key, 0 to 127, selects it.
    Keyswitch {
        /// The switch key.
        key: u8,
    },
    /// A velocity zone of the patch, `lo` to `hi` with 1 ≤ `lo` ≤ `hi` ≤ 127.
    Velocity {
        /// The lowest velocity of the zone.
        lo: u8,
        /// The highest velocity of the zone.
        hi: u8,
    },
    /// A controller value: controller `cc` set to `value`, both 0 to 127.
    Cc {
        /// The controller number.
        cc: u8,
        /// The value that selects the articulation.
        value: u8,
    },
}

/// One articulation that a version 2 entry maps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArticulationEntry {
    /// The articulation.
    pub name: ArticulationName,
    /// How it is reached.
    pub via: ArticulationVia,
    /// In a multi-patch set, the member patch that plays it, a path below the library root.
    /// Always `None` in an entry of a single patch, whose own patch plays it.
    pub member: Option<String>,
    /// The library's own name for it, for example `Sustain Vibrato`.
    pub label: Option<String>,
    /// Whether it is the patch's ordinary sound. At most one articulation of an entry is.
    pub is_default: bool,
    /// The mute the recording carries; [`Mute::Open`] unless the entry names one.
    pub mute: Mute,
    /// The vibrato the recording carries; [`Vibrato::Ordinary`] unless the entry names one.
    pub vibrato: Vibrato,
    /// The onset of its samples in milliseconds, 0 to 2000, when the entry states it.
    pub attack_ms: Option<u16>,
}

/// The articulations an entry declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArticulationList {
    /// A version 1 manifest's names, trimmed, without empty or repeated ones. They are free
    /// text, never checked against the vocabulary, and say nothing about how each is reached.
    Names(Vec<String>),
    /// A version 2 entry's articulations, each with an id the vocabulary knows, unique with its
    /// mute and vibrato.
    Mapped(Vec<ArticulationEntry>),
}

impl ArticulationList {
    /// The articulation ids, in the declared order, each once: the names of a version 1
    /// manifest, the ids of a version 2 one.
    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = Vec::new();
        let mut push = |id| {
            if !ids.contains(&id) {
                ids.push(id);
            }
        };
        match self {
            Self::Names(names) => names.iter().for_each(|name| push(name.as_str())),
            Self::Mapped(entries) => entries.iter().for_each(|entry| push(entry.name.id())),
        }
        ids
    }
}

/// Whether a preset plays one player or a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnsembleKind {
    /// One player.
    Solo,
    /// A section recorded together.
    Section,
    /// A section emulated from solo recordings.
    SectionEmulated,
}

/// Solo or section, with the player count when it is stated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnsembleClaim {
    /// Solo or section.
    pub kind: EnsembleKind,
    /// The number of players, 1 to 64, when it is stated; 1 for a solo.
    pub players: Option<u8>,
}

/// What the sampler's dynamics drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicsMode {
    /// The note velocity picks the layer and the gain.
    Velocity,
    /// The controller with this number, 0 to 127, crossfades the layers.
    Cc(u8),
    /// Every note plays at this velocity, 1 to 127, whatever its own; for patches whose layers
    /// should not follow velocity.
    Fixed(u8),
}

/// How a patch plays legato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegatoMode {
    /// It does not.
    None,
    /// The sampler overlaps and crossfades successive sustained notes.
    Overlap {
        /// The crossfade, 10 to 500 ms.
        crossfade_ms: u16,
        /// Where the next note's sample starts, 0 to 500 ms into it.
        start_offset_ms: u16,
        /// The silence allowed between two notes that still connect, 0 to 100 ms.
        gap_ms: u16,
    },
    /// The patch's own legato regions do it (SFZ `trigger=first` and `trigger=legato`).
    Trigger,
}

/// One row of a percussion patch's key map: the keys that play one instrument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeymapEntry {
    /// The keys, 0 to 127, never empty; no key is in two rows of a map.
    pub keys: Vec<u8>,
    /// The instrument the keys play.
    pub instrument: InstrumentType,
    /// The articulation they play, when it is not the instrument's ordinary sound.
    pub articulation: Option<ArticulationName>,
    /// The pad's name to show, when the entry gives one.
    pub name: Option<String>,
}

/// How well a preset plays as its library intends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictState {
    /// As the library intends.
    AsIs,
    /// With something lost.
    Partly,
    /// Not usefully.
    Not,
}

/// A verdict on a preset: its state and what is lost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// The state.
    pub state: VerdictState,
    /// What is lost, in words for the user; trimmed, without empty lines.
    pub lost: Vec<String>,
}

/// Where a library comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibrarySource {
    /// Where to get it; never empty.
    pub url: String,
    /// The revision the claims describe, for example a tag or a branch.
    pub revision: Option<String>,
}

/// How a library's claims were checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckMethod {
    /// By reading the patch files.
    CodeReading,
    /// By rendering and listening.
    Render,
}

/// When and how a library's claims were checked; it dates the verdicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileCheck {
    /// The day of the check.
    pub checked: jiff::civil::Date,
    /// How it was checked.
    pub method: CheckMethod,
}

#[cfg(test)]
mod tests {
    use sonara_midi::orchestration::{Articulation, Mute, Vibrato};

    use super::{ArticulationEntry, ArticulationList, ArticulationName, ArticulationVia};

    #[test]
    fn an_articulation_name_reads_ordinario_and_exact_vocabulary_ids() {
        assert_eq!(
            ArticulationName::parse("ordinario"),
            Some(ArticulationName::Ordinario)
        );
        assert_eq!(
            Articulation::from_id("ordinario"),
            None,
            "not a vocabulary id"
        );
        for articulation in Articulation::ALL {
            let name = ArticulationName::Vocabulary(articulation);
            assert_eq!(ArticulationName::parse(name.id()), Some(name));
        }
        for text in ["Pizzicato", "pizz", " sustain", "", "auto"] {
            assert_eq!(ArticulationName::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn the_ids_of_a_list_keep_the_declared_order_once_each() {
        let names = ArticulationList::Names(vec!["pizz".to_owned(), "sustain".to_owned()]);
        assert_eq!(names.ids(), ["pizz", "sustain"]);
        let entry = |name, mute| ArticulationEntry {
            name,
            via: ArticulationVia::Patch,
            member: None,
            label: None,
            is_default: false,
            mute,
            vibrato: Vibrato::Ordinary,
            attack_ms: None,
        };
        let mapped = ArticulationList::Mapped(vec![
            entry(
                ArticulationName::Vocabulary(Articulation::Staccato),
                Mute::Open,
            ),
            entry(ArticulationName::Ordinario, Mute::Open),
            entry(
                ArticulationName::Vocabulary(Articulation::Staccato),
                Mute::Generic,
            ),
        ]);
        assert_eq!(mapped.ids(), ["staccato", "ordinario"]);
    }
}
