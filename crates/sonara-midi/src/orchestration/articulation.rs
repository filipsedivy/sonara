//! The articulation vocabulary: stable identifiers of general musical articulations.
//!
//! An identifier names a musical idea, not a sample patch, key switch or controller: how a library
//! produces it is outside this metadata. The slugs match the spelling in a sample library's
//! `sonara-library.json` manifest (`sustain`, `pizzicato`), so a later mapping is a string match.

/// A general musical articulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Articulation {
    /// A held note with no special attack.
    Sustain,
    /// Notes joined smoothly.
    Legato,
    /// Short, detached notes.
    Staccato,
    /// Bowed, bouncing detached notes (strings).
    Spiccato,
    /// Notes held for their full value.
    Tenuto,
    /// Strongly accented notes.
    Marcato,
    /// Plucked strings.
    Pizzicato,
    /// Rapid repetition of one note (bowed or mallet).
    Tremolo,
    /// Rapid alternation with the note above.
    Trill,
    /// Strings struck with the wood of the bow.
    ColLegno,
    /// Natural or artificial harmonics.
    Harmonics,
    /// Bowing near the bridge.
    SulPonticello,
    /// Played with a mute.
    ConSordino,
    /// Flutter tongue (winds and brass).
    FlutterTongue,
    /// A drum roll.
    Roll,
}

impl Articulation {
    /// Every articulation of the vocabulary.
    pub const ALL: [Self; 15] = [
        Self::Sustain,
        Self::Legato,
        Self::Staccato,
        Self::Spiccato,
        Self::Tenuto,
        Self::Marcato,
        Self::Pizzicato,
        Self::Tremolo,
        Self::Trill,
        Self::ColLegno,
        Self::Harmonics,
        Self::SulPonticello,
        Self::ConSordino,
        Self::FlutterTongue,
        Self::Roll,
    ];

    /// The stable identifier, for example `col_legno`.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Sustain => "sustain",
            Self::Legato => "legato",
            Self::Staccato => "staccato",
            Self::Spiccato => "spiccato",
            Self::Tenuto => "tenuto",
            Self::Marcato => "marcato",
            Self::Pizzicato => "pizzicato",
            Self::Tremolo => "tremolo",
            Self::Trill => "trill",
            Self::ColLegno => "col_legno",
            Self::Harmonics => "harmonics",
            Self::SulPonticello => "sul_ponticello",
            Self::ConSordino => "con_sordino",
            Self::FlutterTongue => "flutter_tongue",
            Self::Roll => "roll",
        }
    }

    /// The articulation with identifier `id`, or `None` when it is not in the vocabulary.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|articulation| articulation.id() == id)
    }
}

/// Whether an instrument can play an articulation, as far as is known.
///
/// Only [`ArticulationAvailability::Supported`] means a library can play it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArticulationAvailability {
    /// The library declared it for this instrument.
    Supported,
    /// The instrument plays it, but no library declaration confirms it.
    IdiomaticUnverified,
    /// The instrument does not normally play it and no library declared it.
    NotIdiomatic,
    /// The identifier is not in the vocabulary.
    UnknownArticulation,
}

#[cfg(test)]
mod tests {
    use super::Articulation;

    #[test]
    fn ids_are_unique_and_round_trip() {
        for articulation in Articulation::ALL {
            assert_eq!(Articulation::from_id(articulation.id()), Some(articulation));
        }
        let mut ids: Vec<&str> = Articulation::ALL.iter().map(|a| a.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), Articulation::ALL.len());
    }

    #[test]
    fn unknown_ids_are_not_guessed() {
        assert_eq!(Articulation::from_id("Pizzicato"), None);
        assert_eq!(Articulation::from_id("pizz"), None);
        assert_eq!(Articulation::from_id(""), None);
        assert_eq!(
            Articulation::from_id("pizzicato"),
            Some(Articulation::Pizzicato)
        );
    }
}
