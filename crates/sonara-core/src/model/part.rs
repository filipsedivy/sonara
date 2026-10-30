//! The part a track plays (project schema version 2): the instrument type that plays it, its
//! printed name, its players and how it is notated where that differs from the instrument.
//!
//! The rules here are exactly those of `$defs/part` in `project-v2.schema.json`, so a part that
//! passes the schema always reads. The domain layer checks only the form of the instrument id
//! ([`crate::ids::is_vocabulary_id`]); the application checks it against the orchestration
//! metadata, which this crate cannot see.

use std::fmt;

use crate::error::{Result, require};
use crate::ids::is_vocabulary_id;
use crate::pyfmt::str_repr;

/// The most players a part can have.
pub const MAX_PLAYERS: u8 = 128;
/// The largest notated transposition, in semitones either way (three octaves).
pub const MAX_TRANSPOSITION: i8 = 36;

/// A clef a part can be notated in. The stored strings are the clef ids of the orchestration
/// metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Clef {
    /// The treble (G) clef.
    Treble,
    /// The treble clef with an 8 below it: the notes sound an octave lower than written.
    Treble8vb,
    /// The bass (F) clef.
    Bass,
    /// The alto (C) clef.
    Alto,
    /// The tenor (C) clef.
    Tenor,
    /// The neutral percussion clef.
    Percussion,
    /// A one-line staff, which shows rhythm but no pitch.
    SingleLine,
}

impl Clef {
    /// Every clef, in the order the schema lists them.
    pub const ALL: [Self; 7] = [
        Self::Treble,
        Self::Treble8vb,
        Self::Bass,
        Self::Alto,
        Self::Tenor,
        Self::Percussion,
        Self::SingleLine,
    ];

    /// The clef as stored in project files, for example `treble_8vb`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Treble => "treble",
            Self::Treble8vb => "treble_8vb",
            Self::Bass => "bass",
            Self::Alto => "alto",
            Self::Tenor => "tenor",
            Self::Percussion => "percussion",
            Self::SingleLine => "single_line",
        }
    }

    /// Parses the stored form.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|clef| clef.as_str() == text)
    }
}

impl fmt::Display for Clef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a part is notated where it differs from its instrument's default. Both fields unset
/// means the instrument's notation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Notation {
    /// Sounding minus written pitch, in semitones (−36 to 36).
    pub transposition: Option<i8>,
    /// The clef.
    pub clef: Option<Clef>,
}

impl Notation {
    /// Whether nothing is overridden.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.transposition.is_none() && self.clef.is_none()
    }
}

/// The part a track plays. Only `instrument` is required; an unset field means "not stated",
/// and readers fall back to the instrument's defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    /// The instrument type that plays the part, for example `violin` or `horn_f`.
    pub instrument: String,
    /// The part's printed name, for example `Violins I` or `Horn 3`.
    pub name: Option<String>,
    /// How many players play the part (1–128).
    pub players: Option<u8>,
    /// Whether one player plays the part (`false`: a section).
    pub solo: Option<bool>,
    /// The player who plays the part when one player doubles several instruments, for example
    /// `Flute 2` for a piccolo part.
    pub player: Option<String>,
    /// Notation overrides.
    pub notation: Notation,
}

impl Part {
    /// A part of `instrument` that states nothing else.
    ///
    /// # Errors
    ///
    /// The instrument error of [`Part::validate`].
    pub fn new(instrument: impl Into<String>) -> Result<Self> {
        let part = Self {
            instrument: instrument.into(),
            name: None,
            players: None,
            solo: None,
            player: None,
            notation: Notation::default(),
        };
        part.validate()?;
        Ok(part)
    }

    /// Checks the rules of the schema's `$defs/part`.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::InvalidArgument`] for an instrument that is not a vocabulary
    /// id, an empty name or player, a player count outside 1–128, or a transposition outside
    /// −36 to 36.
    pub fn validate(&self) -> Result<()> {
        require(is_vocabulary_id(&self.instrument), || {
            format!(
                "part instrument must be an id of lowercase letters, digits and underscores, \
                 got {}",
                str_repr(&self.instrument)
            )
        })?;
        require(
            self.name.as_deref().is_none_or(|name| !name.is_empty()),
            || "part name must not be empty".to_owned(),
        )?;
        if let Some(players) = self.players {
            require((1..=MAX_PLAYERS).contains(&players), || {
                format!("part players must be 1-{MAX_PLAYERS}, got {players}")
            })?;
        }
        require(
            self.player
                .as_deref()
                .is_none_or(|player| !player.is_empty()),
            || "part player must not be empty".to_owned(),
        )?;
        if let Some(semitones) = self.notation.transposition {
            require(
                (-MAX_TRANSPOSITION..=MAX_TRANSPOSITION).contains(&semitones),
                || {
                    format!(
                        "part transposition must be -{MAX_TRANSPOSITION}..{MAX_TRANSPOSITION} \
                         semitones, got {semitones}"
                    )
                },
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Clef, MAX_PLAYERS, MAX_TRANSPOSITION, Notation, Part};
    use crate::error::ErrorCode;

    /// A change made to a part.
    type Change = fn(&mut Part);

    #[test]
    fn clefs_round_trip_through_their_stored_form() {
        for clef in Clef::ALL {
            assert_eq!(Clef::parse(clef.as_str()), Some(clef));
            assert_eq!(clef.to_string(), clef.as_str());
        }
        assert_eq!(Clef::parse("Treble"), None);
        assert_eq!(Clef::parse("soprano"), None);
    }

    #[test]
    fn a_part_needs_only_an_instrument() {
        let part = Part::new("horn_f").expect("valid");
        assert_eq!(part.instrument, "horn_f");
        assert!(part.name.is_none() && part.players.is_none() && part.solo.is_none());
        assert!(part.player.is_none() && part.notation.is_empty());
    }

    #[test]
    fn every_rule_of_the_schema_is_checked() {
        let err = Part::new("Violin").expect_err("not an id");
        assert_eq!(err.code, ErrorCode::InvalidArgument);
        assert_eq!(
            err.message,
            "part instrument must be an id of lowercase letters, digits and underscores, \
             got 'Violin'"
        );
        let base = Part::new("violin").expect("valid");
        let cases: [(Change, &str); 6] = [
            (
                |p| p.name = Some(String::new()),
                "part name must not be empty",
            ),
            (
                |p| p.player = Some(String::new()),
                "part player must not be empty",
            ),
            (|p| p.players = Some(0), "part players must be 1-128, got 0"),
            (
                |p| p.players = Some(129),
                "part players must be 1-128, got 129",
            ),
            (
                |p| p.notation.transposition = Some(37),
                "part transposition must be -36..36 semitones, got 37",
            ),
            (
                |p| p.notation.transposition = Some(-37),
                "part transposition must be -36..36 semitones, got -37",
            ),
        ];
        for (change, message) in cases {
            let mut part = base.clone();
            change(&mut part);
            let err = part.validate().expect_err(message);
            assert_eq!(err.code, ErrorCode::InvalidArgument);
            assert_eq!(err.message, message);
        }
    }

    #[test]
    fn the_limits_themselves_are_valid() {
        let mut part = Part::new("violin").expect("valid");
        part.name = Some(" ".to_owned());
        part.player = Some("Violin 1".to_owned());
        part.solo = Some(false);
        for players in [1, MAX_PLAYERS] {
            part.players = Some(players);
            for semitones in [-MAX_TRANSPOSITION, 0, MAX_TRANSPOSITION] {
                part.notation = Notation {
                    transposition: Some(semitones),
                    clef: Some(Clef::Treble8vb),
                };
                assert!(part.validate().is_ok(), "{players} {semitones}");
            }
        }
    }
}
