//! How far the library profiles Sonara ships cover the instrument types of the orchestration
//! metadata ([`ProfileCoverage`]): for each type, whether a copy of every profiled library would
//! give a part of that type a sampled sound, and how close that sound is.
//!
//! The answer is read from the profiles' claims, so it needs no installed library and opens no
//! file. A patch plays a type when its profile says it plays that type, or when its percussion
//! key map has keys for it, and its stated verdict is not [`VerdictState::Not`]. A multi-patch
//! set counts the same way; without a verdict of its own it takes the worst of its members'. The
//! structural check of an installed patch ([`super::InstrumentCatalog::verify`]) can only lower a
//! verdict, so these counts are an upper bound for any one machine.
//!
//! The classes follow the degradation chain ([`super::choose_sound`]) for a part of the type
//! that asks for nothing but its ordinary sound: a patch of the type in a size its ensemble
//! profile has (step 1), only in the other size (step 3), a neighbour's patch (step 4), or
//! nothing sampled (steps 6 and 7).

use sonara_midi::orchestration::fallback::{self, Neighbour};
use sonara_midi::orchestration::{Forms, InstrumentType};

use super::manifest::{
    ArticulationList, EnsembleKind, EntryTarget, LibraryClaims, ManifestEntry, VerdictState,
};
use super::model::same_path;
use super::profiles::{self, Profile};

/// How the shipped library profiles cover one instrument type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    /// A profiled patch plays the type in a size its ensemble profile has: one player for an
    /// instrument scores give to one player, a section for one they give to a section, either
    /// for one that is both. A patch that states no size counts as the right one, as the chain
    /// does not weigh it as the other size.
    Exact,
    /// Profiled patches play the type, but every one in the other size: a section for a
    /// soloist, or one player for a section.
    OtherEnsemble,
    /// No profiled patch plays the type, but one plays a neighbour that the chain offers in its
    /// place: the first of its neighbours ([`fallback::neighbours`]) that has one.
    Substitute(Neighbour),
    /// Neither the type nor any of its neighbours has a profiled patch: the chain falls to a
    /// built-in sound or to silence.
    Unsampled,
}

impl Coverage {
    /// Whether a sampled sound plays the type, its own or a neighbour's.
    #[must_use]
    pub const fn is_sampled(self) -> bool {
        !matches!(self, Self::Unsampled)
    }
}

/// How many instrument types fall into each class of [`Coverage`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CoverageCounts {
    /// Types a profiled patch plays in the right size ([`Coverage::Exact`]).
    pub exact: usize,
    /// Types profiled patches play only in the other size ([`Coverage::OtherEnsemble`]).
    pub other_ensemble: usize,
    /// Types only a neighbour's profiled patch plays ([`Coverage::Substitute`]).
    pub substitutes: usize,
    /// Types without any sampled sound ([`Coverage::Unsampled`]).
    pub unsampled: usize,
}

impl CoverageCounts {
    /// The loose count: the types any sampled sound plays, in the other size and through a
    /// neighbour included.
    #[must_use]
    pub const fn loose(self) -> usize {
        self.exact + self.other_ensemble + self.substitutes
    }

    /// Every type counted, sampled or not.
    #[must_use]
    pub const fn total(self) -> usize {
        self.loose() + self.unsampled
    }
}

/// The coverage of every instrument type by a set of library profiles, in score order (the order
/// of [`InstrumentType::ALL`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileCoverage {
    rows: Vec<(InstrumentType, Coverage)>,
}

impl ProfileCoverage {
    /// The coverage by the library profiles Sonara ships (VSCO 2 Community Edition and VCSL).
    #[must_use]
    pub fn shipped() -> Self {
        Self::of(profiles::shipped())
    }

    /// The coverage by `profiles`.
    pub(crate) fn of(profiles: &[Profile]) -> Self {
        let entries: Vec<&ManifestEntry> = profiles
            .iter()
            .flat_map(|profile| {
                let claims = &profile.claims;
                claims
                    .entries
                    .iter()
                    .filter(move |entry| !judged_not(entry, claims))
            })
            .collect();
        let playing = |instrument: InstrumentType| {
            entries
                .iter()
                .copied()
                .filter(move |entry| plays(entry, instrument))
        };
        let rows = InstrumentType::ALL
            .into_iter()
            .map(|instrument| {
                let coverage = if playing(instrument).any(|entry| right_size(entry, instrument)) {
                    Coverage::Exact
                } else if playing(instrument).next().is_some() {
                    Coverage::OtherEnsemble
                } else {
                    fallback::neighbours(instrument)
                        .iter()
                        .find(|neighbour| playing(neighbour.instrument).next().is_some())
                        .map_or(Coverage::Unsampled, |neighbour| {
                            Coverage::Substitute(*neighbour)
                        })
                };
                (instrument, coverage)
            })
            .collect();
        Self { rows }
    }

    /// Every instrument type with its coverage, in score order.
    #[must_use]
    pub fn rows(&self) -> &[(InstrumentType, Coverage)] {
        &self.rows
    }

    /// The types without any sampled sound, in score order.
    pub fn unsampled(&self) -> impl Iterator<Item = InstrumentType> + '_ {
        self.rows
            .iter()
            .filter(|(_, coverage)| !coverage.is_sampled())
            .map(|(instrument, _)| *instrument)
    }

    /// How many types fall into each class.
    #[must_use]
    pub fn counts(&self) -> CoverageCounts {
        self.rows
            .iter()
            .fold(CoverageCounts::default(), |mut counts, (_, coverage)| {
                match coverage {
                    Coverage::Exact => counts.exact += 1,
                    Coverage::OtherEnsemble => counts.other_ensemble += 1,
                    Coverage::Substitute(_) => counts.substitutes += 1,
                    Coverage::Unsampled => counts.unsampled += 1,
                }
                counts
            })
    }
}

/// Whether the verdict on `entry` is [`VerdictState::Not`]: its own, or for a set without one,
/// that of any member patch `claims` lists, as the chain judges a set.
fn judged_not(entry: &ManifestEntry, claims: &LibraryClaims) -> bool {
    let not = |entry: &ManifestEntry| {
        entry
            .verdict
            .value()
            .is_some_and(|verdict| verdict.state == VerdictState::Not)
    };
    match (&entry.target, entry.verdict.value()) {
        (EntryTarget::Path(_), _) | (EntryTarget::Set(_), Some(_)) => not(entry),
        (EntryTarget::Set(_), None) => {
            let Some(ArticulationList::Mapped(articulations)) = entry.articulations.value() else {
                return false;
            };
            articulations
                .iter()
                .filter_map(|articulation| articulation.member.as_deref())
                .any(|member| {
                    claims.entries.iter().any(|other| {
                        matches!(&other.target, EntryTarget::Path(path) if same_path(path, member))
                            && not(other)
                    })
                })
        }
    }
}

/// Whether `entry` plays `instrument`: it says so, or its key map has keys for it.
fn plays(entry: &ManifestEntry, instrument: InstrumentType) -> bool {
    entry.instrument.value() == Some(&instrument)
        || entry
            .keymap
            .value()
            .is_some_and(|rows| rows.iter().any(|row| row.instrument == instrument))
}

/// Whether the size `entry` states is one the ensemble profile of `instrument` has; a patch that
/// states none, or an instrument without a profile, fits either.
fn right_size(entry: &ManifestEntry, instrument: InstrumentType) -> bool {
    let Some(claim) = entry.ensemble.value() else {
        return true;
    };
    let section = matches!(
        claim.kind,
        EnsembleKind::Section | EnsembleKind::SectionEmulated
    );
    match instrument.get().ensemble().map(|profile| profile.forms) {
        Some(Forms::Solo) => !section,
        Some(Forms::Section) => section,
        Some(Forms::Both) | None => true,
    }
}

#[cfg(test)]
mod tests;
