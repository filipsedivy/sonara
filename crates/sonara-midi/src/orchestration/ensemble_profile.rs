//! How an instrument appears in ensembles: as a soloist, as a section or both, and how many
//! players typical scores of four kinds give it.

use super::instrument::{InstrumentError, Provenance};

/// Whether scores and libraries treat the instrument as one player or as a section.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Forms {
    /// One player per part, such as the oboe or the harp.
    Solo,
    /// A group playing one part in unison, such as a choir section.
    Section,
    /// Either, such as the violin (a section in the orchestra, one player in a quartet).
    Both,
}

/// A kind of score whose typical player counts the profile records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Era {
    /// The orchestra of the Classical era (Haydn, Mozart, early Beethoven).
    Classical,
    /// The full orchestra of the Romantic era.
    Romantic,
    /// A film or studio orchestra, as sample libraries record it.
    Film,
    /// Chamber music: small ensembles and chamber orchestras.
    Chamber,
}

impl Era {
    /// Every era, in declaration order.
    pub const ALL: [Self; 4] = [Self::Classical, Self::Romantic, Self::Film, Self::Chamber];
}

/// An inclusive number of players, `min <= max`. `0..=0` ([`PlayerRange::NOT_USED`]) means the
/// instrument is not used in that kind of score.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlayerRange {
    min: u8,
    max: u8,
}

impl PlayerRange {
    /// No players: the instrument is not used.
    pub const NOT_USED: Self = Self { min: 0, max: 0 };

    /// The range from `min` to `max` players, both inclusive.
    ///
    /// # Errors
    /// [`InstrumentError::InvertedPlayerRange`] when `min > max`.
    pub const fn try_new(min: u8, max: u8) -> Result<Self, InstrumentError> {
        if min > max {
            return Err(InstrumentError::InvertedPlayerRange { min, max });
        }
        Ok(Self { min, max })
    }

    /// Like [`PlayerRange::try_new`]; an inverted range in a `const` item is a compile error.
    ///
    /// # Panics
    /// When [`PlayerRange::try_new`] would fail.
    #[must_use]
    pub const fn new(min: u8, max: u8) -> Self {
        match Self::try_new(min, max) {
            Ok(range) => range,
            Err(_) => panic!("invalid player range"),
        }
    }

    /// The fewest players.
    #[must_use]
    pub const fn min(self) -> u8 {
        self.min
    }

    /// The most players.
    #[must_use]
    pub const fn max(self) -> u8 {
        self.max
    }
}

/// The players one kind of score typically gives the instrument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EraPlayers {
    /// The number of players; `None` when no number is stated.
    pub players: Option<PlayerRange>,
    /// The qualifier the number comes with (for example "6-8 in large scores"); may be empty.
    pub note: &'static str,
    /// Where the number comes from.
    pub provenance: Provenance,
}

/// How the instrument appears in ensembles: its forms and the typical players per kind of score.
///
/// The section sizes of a particular orchestra are ensemble data, not part of this profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnsembleProfile {
    /// Solo, section or both.
    pub forms: Forms,
    /// Typical players in a Classical-era orchestra.
    pub classical: EraPlayers,
    /// Typical players in a full Romantic orchestra.
    pub romantic: EraPlayers,
    /// Typical players in a film or studio orchestra.
    pub film: EraPlayers,
    /// Typical players in chamber music.
    pub chamber: EraPlayers,
}

impl EnsembleProfile {
    /// The typical players of `era`.
    #[must_use]
    pub const fn players(&self, era: Era) -> &EraPlayers {
        match era {
            Era::Classical => &self.classical,
            Era::Romantic => &self.romantic,
            Era::Film => &self.film,
            Era::Chamber => &self.chamber,
        }
    }
}
