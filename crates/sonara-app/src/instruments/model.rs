//! The catalog's data: identifiers, presets with what is claimed about them and who claims it,
//! libraries, availability and the problems a scan reports.

use std::fmt;
use std::path::PathBuf;

use serde::Serialize;
use sonara_midi::orchestration::InstrumentType;

use super::facts::PatchFacts;
use super::family::FamilyClaim;
use super::manifest::{
    ArticulationEntry, ArticulationList, DynamicsMode, EnsembleClaim, KeymapEntry, LegatoMode,
    Verdict, VerdictState, serialize_instrument,
};

/// The identifier of a sample library: a lower-case slug, `[a-z0-9][a-z0-9_-]*`, at most 64
/// characters. It is the library's `id` in its manifest; else the `id` of the library profile
/// Sonara ships for it, when the library holds that profile's detect files; else a slug of its
/// directory name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct LibraryId(String);

/// A library ID that is not a valid slug.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid library id {0:?}: use lower-case letters, digits, '-' and '_' (1 to 64)")]
pub struct InvalidLibraryId(pub String);

impl LibraryId {
    /// The longest library ID.
    pub const MAX_LEN: usize = 64;

    /// Validates `text` as a library ID (TYP-06).
    ///
    /// # Errors
    ///
    /// [`InvalidLibraryId`] when `text` is empty, too long, starts with `-` or `_`, or holds a
    /// character other than `a-z`, `0-9`, `-` and `_`.
    pub fn parse(text: &str) -> Result<Self, InvalidLibraryId> {
        let mut chars = text.chars();
        let valid_start = chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        let valid_rest =
            chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
        if valid_start && valid_rest && text.len() <= Self::MAX_LEN {
            Ok(Self(text.to_owned()))
        } else {
            Err(InvalidLibraryId(text.to_owned()))
        }
    }

    /// A slug of a directory name: letters and digits lower-cased, every other run becomes one
    /// `-`; `library` when nothing is left.
    #[must_use]
    pub(crate) fn from_directory_name(name: &str) -> Self {
        let mut slug = String::new();
        for c in name.chars() {
            if c.is_ascii_alphanumeric() {
                slug.push(c.to_ascii_lowercase());
            } else if !slug.is_empty() && !slug.ends_with('-') {
                slug.push('-');
            }
        }
        let slug = slug.trim_end_matches('-');
        let slug: String = slug.chars().take(Self::MAX_LEN).collect();
        if slug.is_empty() {
            Self("library".to_owned())
        } else {
            Self(slug)
        }
    }

    /// The slug.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LibraryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The stable identifier of an instrument preset: `<library id>/<patch path below the library
/// root, without the extension>`, for example `vsco2ce/Strings/Violin Solo`, or for a
/// multi-patch set `<library id>/@<set>`, for example `vsco2ce/@horn-solo`.
///
/// It does not hold the absolute path, so it stays the same when a library is installed in
/// another directory or on another machine.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct InstrumentId(String);

impl InstrumentId {
    pub(crate) fn new(library: &LibraryId, relative_path: &str) -> Self {
        let stem = relative_path
            .rsplit_once('.')
            .map_or(relative_path, |(stem, _)| stem);
        Self(format!("{library}/{stem}"))
    }

    /// The ID of the multi-patch set `slug` of `library`: `<library>/@<slug>`.
    pub(crate) fn of_set(library: &LibraryId, slug: &str) -> Self {
        Self(format!("{library}/@{slug}"))
    }

    /// The ID as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InstrumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whether a preset can be played on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum Availability {
    /// The patch file exists and is readable. A deeper check, [`super::PatchLoader`], may still
    /// find it invalid.
    Available,
    /// The library's manifest lists the patch but the file is not there.
    Missing,
    /// The file exists but cannot be used.
    Invalid {
        /// Why, in words for the user.
        reason: String,
    },
}

impl Availability {
    /// Whether the preset can be played.
    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }
}

/// Who says what the catalog shows about a preset. Serialized in lower case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ClaimSource {
    /// The library's own manifest: the library's word, trusted.
    Declared,
    /// The library profile Sonara ships for the library: Sonara's word, read from the library's
    /// patch files, not the library's.
    Profile,
    /// A guess from the words of the patch's path.
    Inferred,
}

/// A value claimed about a preset, and who claims it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Claim<T> {
    /// What is claimed.
    pub value: T,
    /// Who claims it.
    pub source: ClaimSource,
}

/// The instrument type a preset plays, and who says so. Serialized as
/// `{"instrument": "violin", "source": "profile"}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct InstrumentClaim {
    /// The instrument type, an ID of the orchestration metadata.
    #[serde(serialize_with = "serialize_instrument")]
    pub instrument: InstrumentType,
    /// Who says so.
    pub source: ClaimSource,
}

/// Whether a preset is one patch or a multi-patch set. Serialized into the preset as
/// `"kind": "patch"`, or as `"kind": "set"` beside the set's `members`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PresetKind {
    /// One patch file, [`InstrumentPreset::patch_path`].
    Patch,
    /// A multi-patch set of one instrument, which a version 2 manifest or a profile defines:
    /// several patches, each playing the articulations that name it
    /// ([`ArticulationEntry::member`](super::ArticulationEntry::member)).
    Set {
        /// The patches, in the order the set's articulations first name them, each once.
        members: Vec<SetMember>,
    },
}

/// One patch of a multi-patch set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetMember {
    /// The patch path below the library root, `/`-separated: spelled as on disk when the scan
    /// found the file, else as the manifest or the profile writes it.
    pub path: String,
    /// The patch file on this machine.
    pub patch_path: PathBuf,
    /// Whether the patch can be played now, by the same checks as a preset's.
    pub availability: Availability,
    /// What the structural check found in the patch, once
    /// [`super::InstrumentCatalog::verify`] ran on the set with a loader that inspects patches;
    /// `None` before.
    pub facts: Option<PatchFacts>,
}

/// One instrument preset (`contracts/instrument-catalog.md`).
///
/// Every claim keeps its source ([`ClaimSource`]): the library's manifest declares, the profile
/// Sonara ships for the library states Sonara's reading of it, and a guess from the path is
/// inferred. The fields `articulations` and `license` keep their first meaning, what the
/// library's own manifest declares; the claims beside them add the profile's word, marked as
/// such.
///
/// A preset is a patch, or a multi-patch set ([`PresetKind`]) whose own patches are also listed
/// as the presets they are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstrumentPreset {
    /// The stable ID.
    pub id: InstrumentId,
    /// The name to show: the manifest's, else that of the shipped profile that applies to the
    /// library, else the patch's file name, or a set's slug.
    pub display_name: String,
    /// The family and where that claim comes from.
    pub family: FamilyClaim,
    /// The library the preset belongs to.
    pub library: LibraryId,
    /// The patch file on this machine. SFZ presets have no bank or program: the patch is the
    /// preset. For a set, the patch of its default articulation, else of its first one; the
    /// library directory for a set that names no patch.
    pub patch_path: PathBuf,
    /// One patch, or a set with its members.
    #[serde(flatten)]
    pub kind: PresetKind,
    /// Articulations the library's manifest declares for the patch: the names of a version 1
    /// manifest, the articulation IDs of a version 2 one. **Empty means not verified**, not
    /// "none": an articulation that is not listed is never presented as supported.
    pub articulations: Vec<String>,
    /// The licence the library's manifest declares for this recording, else for the library;
    /// `None` means **unknown**.
    pub license: Option<String>,
    /// Whether the preset can be played now. A set is available when every member is; it is
    /// missing when a member is missing, else invalid when a member is, with that member's
    /// reason, and invalid when it names no patch.
    pub availability: Availability,
    /// The instrument type the preset plays: declared, stated by the profile, or inferred from
    /// the path, or a set's slug, when neither says. `None` when nothing names one type, or when the manifest or
    /// the profile says that the patch plays several (`instrument: null`, a key map). An
    /// inferred type is kept only when its family is the preset's family.
    pub instrument: Option<InstrumentClaim>,
    /// Solo or section, with the player count when it is stated.
    pub ensemble: Option<Claim<EnsembleClaim>>,
    /// The articulations and how each is reached, as the manifest or the profile maps them:
    /// version 1 names say only which, version 2 entries also how. `None` means not verified,
    /// as for [`Self::articulations`]; nothing is guessed.
    pub articulation_map: Option<Claim<ArticulationList>>,
    /// For a percussion patch with one sound per key, the instrument each key plays.
    pub keymap: Option<Claim<Vec<KeymapEntry>>>,
    /// Semitones added to incoming notes, for a patch keyed away from the pitch it sounds.
    pub key_offset: Option<Claim<i8>>,
    /// What drives dynamics.
    pub dynamics: Option<Claim<DynamicsMode>>,
    /// How the preset plays legato.
    pub legato: Option<Claim<LegatoMode>>,
    /// Whether the library was recorded in seating position; `None` when unknown.
    pub in_situ: Option<Claim<bool>>,
    /// The licence of this recording, else of its library, as the manifest declares it or the
    /// profile states it.
    pub license_claim: Option<Claim<String>>,
    /// How well the preset plays as the manifest or the profile judges it. What the structural
    /// check finds may lower it: [`Self::effective_verdict`].
    pub verdict: Option<Claim<Verdict>>,
    /// What the structural check found in the patch, once [`super::InstrumentCatalog::verify`]
    /// ran with a loader that inspects patches; `None` before, and always for a set, whose
    /// members hold their own ([`SetMember::facts`]).
    pub facts: Option<PatchFacts>,
}

impl InstrumentPreset {
    /// How well the preset plays: the claimed [`Self::verdict`], lowered by what the
    /// structural check found ([`Self::facts`], or each member's for a set). Facts beat claims:
    /// a patch that does not load is [`VerdictState::Not`], and a sample left out or a claimed
    /// key switch that is not a switch key of the patch makes it at best
    /// [`VerdictState::Partly`]; each finding is added to what is lost. A set takes the
    /// findings of every member, each led by the member's path, and checks a key switch against
    /// the member that its articulation names; a member that does not load makes the whole set
    /// [`VerdictState::Not`]. `None` when nothing is claimed and the check found nothing wrong
    /// or did not run.
    #[must_use]
    pub fn effective_verdict(&self) -> Option<Verdict> {
        let mapped: &[ArticulationEntry] = match self.articulation_map.as_ref() {
            Some(Claim {
                value: ArticulationList::Mapped(entries),
                ..
            }) => entries,
            _ => &[],
        };
        let findings = match &self.kind {
            PresetKind::Patch => self
                .facts
                .as_ref()
                .map_or_else(Vec::new, |facts| facts.findings(mapped)),
            PresetKind::Set { members } => members
                .iter()
                .flat_map(|member| {
                    let played: Vec<&ArticulationEntry> = mapped
                        .iter()
                        .filter(|entry| {
                            entry
                                .member
                                .as_deref()
                                .is_some_and(|path| same_path(path, &member.path))
                        })
                        .collect();
                    member
                        .facts
                        .as_ref()
                        .map_or_else(Vec::new, |facts| facts.findings(played))
                        .into_iter()
                        .map(|(state, finding)| (state, format!("{}: {finding}", member.path)))
                })
                .collect(),
        };
        let claimed = self.verdict.as_ref().map(|claim| &claim.value);
        if claimed.is_none() && findings.is_empty() {
            return None;
        }
        let mut verdict = claimed.cloned().unwrap_or(Verdict {
            state: VerdictState::AsIs,
            lost: Vec::new(),
        });
        for (state, finding) in findings {
            verdict.state = verdict.state.worse(state);
            if !verdict.lost.contains(&finding) {
                verdict.lost.push(finding);
            }
        }
        Some(verdict)
    }

    /// Whether the preset can play a part, by everything known about it: the catalog finds it
    /// [`Availability::Available`]; where the structural check ran ([`Self::facts`], or each
    /// member's for a set), it found a region that can play and no sample file missing; and its
    /// [`effective verdict`](Self::effective_verdict) is not [`VerdictState::Not`].
    ///
    /// Facts that are not known yet do not count against it: a preset discovery lists and no
    /// check has read is playable, and a check that runs later can only take that back.
    #[must_use]
    pub fn is_playable(&self) -> bool {
        let sound = |facts: Option<&PatchFacts>| {
            facts.is_none_or(|facts| facts.playable_regions > 0 && facts.missing_samples.is_empty())
        };
        let checked = match &self.kind {
            PresetKind::Patch => sound(self.facts.as_ref()),
            PresetKind::Set { members } => {
                members.iter().all(|member| sound(member.facts.as_ref()))
            }
        };
        self.availability.is_available()
            && checked
            && self
                .effective_verdict()
                .is_none_or(|verdict| verdict.state != VerdictState::Not)
    }
}

/// Whether two patch paths below a library name the same file, ignoring case, as discovery
/// matches manifest paths to the files on disk.
pub(crate) fn same_path(a: &str, b: &str) -> bool {
    a == b || a.to_lowercase() == b.to_lowercase()
}

/// A library root the catalog read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Library {
    /// The library's ID.
    pub id: LibraryId,
    /// The name to show: the manifest's, else that of the shipped profile that applies to the
    /// library, else the ID.
    pub name: String,
    /// The library directory.
    pub root: PathBuf,
    /// The licence the manifest declares. `None` means **unknown**: check the library's own
    /// terms before redistributing anything (`docs/instrument-catalog.md`).
    pub license: Option<String>,
    /// Whether a valid manifest was read.
    pub has_manifest: bool,
}

/// A problem found while discovering libraries. None of them stops the discovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "issue", rename_all = "snake_case")]
pub enum LibraryIssue {
    /// A configured directory does not exist.
    RootMissing {
        /// The directory.
        path: PathBuf,
    },
    /// A configured path is not a directory.
    RootNotDirectory {
        /// The path.
        path: PathBuf,
    },
    /// The manifest could not be read or parsed; the library is scanned without it.
    ManifestInvalid {
        /// The manifest file.
        path: PathBuf,
        /// Why.
        reason: String,
    },
    /// A manifest entry was ignored; the rest of the manifest is used.
    ManifestEntryRejected {
        /// The manifest file.
        path: PathBuf,
        /// The entry's `path` as written, `@<set>` for a multi-patch set, or `#<n>`, its
        /// position in the list counted from 1, when it names neither.
        entry: String,
        /// Why.
        reason: String,
    },
    /// More than one of Sonara's library profiles matches a library without a declared ID, so
    /// none applies and the library's ID is the slug of its directory name.
    ProfileAmbiguous {
        /// The library directory.
        path: PathBuf,
        /// The IDs of the matching profiles.
        profiles: Vec<LibraryId>,
    },
    /// Two libraries have the same ID; the second one is skipped.
    DuplicateLibrary {
        /// The shared ID.
        id: LibraryId,
        /// The skipped directory.
        path: PathBuf,
    },
    /// A library holds more patches than a scan reports; the rest are not listed.
    ScanTruncated {
        /// The library directory.
        path: PathBuf,
        /// The most patches reported per library.
        limit: usize,
    },
}

/// Errors of the catalog's operations on a preset.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CatalogError {
    /// No preset has this ID.
    #[error("no instrument preset {0}")]
    UnknownInstrument(String),
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use sonara_media::sfz::MAX_BANK_BYTES;
    use sonara_midi::orchestration::{Articulation, InstrumentType, Mute, Vibrato};

    use super::{
        Availability, Claim, ClaimSource, InstrumentClaim, InstrumentId, InstrumentPreset,
        LibraryId, PresetKind, SetMember,
    };
    use crate::instruments::{
        ArticulationEntry, ArticulationList, ArticulationName, ArticulationVia, FamilyClaim,
        InstrumentFamily, PatchFacts, Verdict, VerdictState,
    };

    #[test]
    fn library_ids_are_validated() {
        for good in ["vsco2ce", "a", "sso_2-0", "0day"] {
            assert_eq!(LibraryId::parse(good).expect(good).as_str(), good);
        }
        let too_long = "a".repeat(65);
        for bad in ["", "VSCO", "-x", "_x", "a b", "a/b", "é", too_long.as_str()] {
            assert!(LibraryId::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_directory_name_becomes_a_slug() {
        let slug = |name| LibraryId::from_directory_name(name).to_string();
        assert_eq!(slug("VSCO-2-CE"), "vsco-2-ce");
        assert_eq!(slug("  My  Library!! "), "my-library");
        assert_eq!(slug("日本語"), "library");
        assert_eq!(slug(""), "library");
        assert_eq!(slug(&"x".repeat(100)).len(), LibraryId::MAX_LEN);
    }

    #[test]
    fn an_instrument_id_drops_the_extension_and_the_machine() {
        let library = LibraryId::parse("vsco2ce").expect("valid");
        assert_eq!(
            InstrumentId::new(&library, "Strings/Violin Solo.sfz").as_str(),
            "vsco2ce/Strings/Violin Solo"
        );
        assert_eq!(InstrumentId::new(&library, "Harp").as_str(), "vsco2ce/Harp");
    }

    /// A violin preset of the library `lib` that the profile judges `verdict`, with
    /// `articulations` and the structural check's `facts`.
    fn preset(
        verdict: Option<Verdict>,
        articulations: Option<ArticulationList>,
        facts: Option<PatchFacts>,
    ) -> InstrumentPreset {
        let library = LibraryId::parse("lib").expect("valid");
        let profile = |value| Claim {
            value,
            source: ClaimSource::Profile,
        };
        InstrumentPreset {
            id: InstrumentId::new(&library, "Violin.sfz"),
            display_name: "Violin".to_owned(),
            family: FamilyClaim::Profile(InstrumentFamily::Strings),
            library,
            patch_path: PathBuf::from("/libraries/lib/Violin.sfz"),
            kind: PresetKind::Patch,
            articulations: Vec::new(),
            license: None,
            availability: Availability::Available,
            instrument: Some(InstrumentClaim {
                instrument: InstrumentType::Violin,
                source: ClaimSource::Profile,
            }),
            ensemble: None,
            articulation_map: articulations.map(|value| Claim {
                value,
                source: ClaimSource::Profile,
            }),
            keymap: None,
            key_offset: None,
            dynamics: None,
            legato: None,
            in_situ: None,
            license_claim: None,
            verdict: verdict.map(profile),
            facts,
        }
    }

    /// The facts of a patch that loads: switch keys 36 and 37, one unsupported opcode.
    fn sound_facts() -> PatchFacts {
        PatchFacts {
            regions: 4,
            playable_regions: 4,
            sampled: Vec::new(),
            switch_keys: vec![36, 37],
            default_switch: Some(36),
            ignored_opcodes: vec!["ampeg_dynamic".to_owned()],
            missing_samples: Vec::new(),
            unreadable_samples: Vec::new(),
            notes: Vec::new(),
            decoded_bytes: 1024,
            loadable: true,
        }
    }

    fn verdict(state: VerdictState, lost: &[&str]) -> Verdict {
        Verdict {
            state,
            lost: lost.iter().map(|line| (*line).to_owned()).collect(),
        }
    }

    fn switch(articulation: Articulation, key: u8) -> ArticulationEntry {
        ArticulationEntry {
            name: ArticulationName::Vocabulary(articulation),
            via: ArticulationVia::Keyswitch { key },
            member: None,
            label: None,
            is_default: false,
            mute: Mute::Open,
            vibrato: Vibrato::Ordinary,
            attack_ms: None,
        }
    }

    #[test]
    fn without_a_claim_or_a_finding_the_verdict_is_unknown() {
        assert_eq!(preset(None, None, None).effective_verdict(), None);
        // An unsupported opcode alone lowers nothing: what it costs is the claim's to say.
        assert_eq!(
            preset(None, None, Some(sound_facts())).effective_verdict(),
            None
        );
        let claimed = verdict(VerdictState::AsIs, &["ampeg_dynamic"]);
        assert_eq!(
            preset(Some(claimed.clone()), None, Some(sound_facts())).effective_verdict(),
            Some(claimed)
        );
    }

    /// Facts beat claims: a claimed key switch that does not switch in the patch lowers the
    /// claim to `partly` and says which (TST-03).
    #[test]
    fn a_claimed_key_switch_the_patch_lacks_lowers_the_verdict() {
        let mapped = ArticulationList::Mapped(vec![
            switch(Articulation::Sustain, 36),
            switch(Articulation::Pizzicato, 37),
            switch(Articulation::Tremolo, 40),
        ]);
        let claimed = verdict(VerdictState::AsIs, &["ampeg_dynamic"]);
        let violin = preset(Some(claimed.clone()), Some(mapped), Some(sound_facts()));
        assert_eq!(
            violin.effective_verdict(),
            Some(verdict(
                VerdictState::Partly,
                &[
                    "ampeg_dynamic",
                    "tremolo: key 40 is not a switch key of the patch"
                ]
            ))
        );
        // The claim itself is kept as it was made.
        assert_eq!(violin.verdict.map(|claim| claim.value), Some(claimed));
        // Names of a version 1 manifest say nothing about keys, so nothing is checked.
        let names = ArticulationList::Names(vec!["tremolo".to_owned()]);
        assert_eq!(
            preset(None, Some(names), Some(sound_facts())).effective_verdict(),
            None
        );
    }

    /// A sample left out makes a preset at best `partly`, with the reader's note; a patch that
    /// does not load is `not`, whatever is claimed, and a finding the claim already names is
    /// not repeated.
    #[test]
    fn samples_left_out_and_a_patch_that_does_not_load_lower_the_verdict() {
        let note = "sample /lib/b.wav is missing (1 region left out)";
        let partial = PatchFacts {
            missing_samples: vec![PathBuf::from("/lib/b.wav")],
            notes: vec![note.to_owned()],
            ..sound_facts()
        };
        assert_eq!(
            preset(None, None, Some(partial.clone())).effective_verdict(),
            Some(verdict(VerdictState::Partly, &[note]))
        );
        assert_eq!(
            preset(
                Some(verdict(VerdictState::Partly, &[note])),
                None,
                Some(partial)
            )
            .effective_verdict(),
            Some(verdict(VerdictState::Partly, &[note]))
        );

        let as_is = || Some(verdict(VerdictState::AsIs, &[]));
        let not_loading = [
            (
                PatchFacts {
                    regions: 0,
                    playable_regions: 0,
                    loadable: false,
                    ..sound_facts()
                },
                "the patch has no region".to_owned(),
            ),
            (
                PatchFacts {
                    playable_regions: 0,
                    loadable: false,
                    ..sound_facts()
                },
                "no region of the patch can play".to_owned(),
            ),
            (
                PatchFacts {
                    unreadable_samples: vec![PathBuf::from("/lib/c.wav")],
                    loadable: false,
                    ..sound_facts()
                },
                "the patch does not load while a sample file cannot be read".to_owned(),
            ),
            (
                PatchFacts {
                    decoded_bytes: MAX_BANK_BYTES + 1,
                    loadable: false,
                    ..sound_facts()
                },
                // One byte over the 3 GiB cap is a started MiB over it.
                "its samples take 3073 MiB decoded, more than the 3072 MiB a patch may take"
                    .to_owned(),
            ),
        ];
        for (facts, reason) in not_loading {
            assert_eq!(
                preset(as_is(), None, Some(facts)).effective_verdict(),
                Some(verdict(VerdictState::Not, &[reason.as_str()])),
                "{reason}"
            );
        }
    }

    /// A set takes each member's findings, led by the member's path, and checks a key switch
    /// against the member its articulation names, ignoring case; a member that does not load
    /// makes the whole set `not`. Its JSON form names the kind and the members (TST-05).
    #[test]
    fn a_set_takes_the_findings_of_each_member() {
        let member = |path: &str, facts| SetMember {
            path: path.to_owned(),
            patch_path: PathBuf::from("/libraries/lib").join(path),
            availability: Availability::Available,
            facts,
        };
        let played_by = |articulation, path: &str| ArticulationEntry {
            member: Some(path.to_owned()),
            ..switch(articulation, 36)
        };
        let mapped = ArticulationList::Mapped(vec![
            played_by(Articulation::Sustain, "a.sfz"),
            played_by(Articulation::Tremolo, "B.sfz"),
        ]);
        let mut set = preset(None, Some(mapped), None);
        // `a.sfz` switches on key 36; `b.sfz` has no switch key and no region that can play.
        set.kind = PresetKind::Set {
            members: vec![
                member("a.sfz", Some(sound_facts())),
                member(
                    "b.sfz",
                    Some(PatchFacts {
                        switch_keys: Vec::new(),
                        playable_regions: 0,
                        loadable: false,
                        ..sound_facts()
                    }),
                ),
                member("c.sfz", None),
            ],
        };
        assert_eq!(
            set.effective_verdict(),
            Some(verdict(
                VerdictState::Not,
                &[
                    "b.sfz: no region of the patch can play",
                    "b.sfz: tremolo: key 36 is not a switch key of the patch"
                ]
            ))
        );
        let value = serde_json::to_value(&set).expect("serializes");
        assert_eq!(value["kind"], json!("set"));
        assert_eq!(
            value["members"][2],
            json!({"path": "c.sfz", "patch_path": "/libraries/lib/c.sfz",
                   "availability": {"state": "available"}, "facts": null})
        );
        assert_eq!(value["members"][0]["facts"]["switch_keys"], json!([36, 37]));
    }

    #[test]
    fn the_worse_of_two_states_wins() {
        use VerdictState::{AsIs, Not, Partly};
        let cases = [
            (AsIs, AsIs, AsIs),
            (AsIs, Partly, Partly),
            (Partly, AsIs, Partly),
            (Partly, Not, Not),
            (Not, AsIs, Not),
            (Partly, Partly, Partly),
        ];
        for (a, b, worse) in cases {
            assert_eq!(a.worse(b), worse, "{a:?} {b:?}");
        }
    }

    /// The JSON form of a preset's claims and facts (TST-05): each claim is its value and its
    /// source, the instrument type is its orchestration ID, and absent claims are `null`.
    #[test]
    fn claims_and_facts_keep_their_json_form() {
        for (source, text) in [
            (ClaimSource::Declared, "declared"),
            (ClaimSource::Profile, "profile"),
            (ClaimSource::Inferred, "inferred"),
        ] {
            assert_eq!(
                serde_json::to_value(source).expect("serializes"),
                json!(text)
            );
        }
        assert_eq!(
            serde_json::to_value(Claim {
                value: -12,
                source: ClaimSource::Declared
            })
            .expect("serializes"),
            json!({"value": -12, "source": "declared"})
        );
        let facts = PatchFacts {
            sampled: vec![crate::instruments::KeyZone {
                lo: 55,
                hi: 60,
                layers: vec![crate::instruments::VelocityRange { lo: 1, hi: 127 }],
            }],
            ..sound_facts()
        };
        let preset = preset(
            Some(verdict(VerdictState::Partly, &["rt_decay"])),
            Some(ArticulationList::Mapped(vec![switch(
                Articulation::Sustain,
                36,
            )])),
            Some(facts),
        );
        let value = serde_json::to_value(&preset).expect("serializes");
        assert_eq!(value["kind"], json!("patch"));
        assert_eq!(value.get("members"), None);
        assert_eq!(
            value["instrument"],
            json!({"instrument": "violin", "source": "profile"})
        );
        assert_eq!(
            value["family"],
            json!({"source": "profile", "family": "strings"})
        );
        assert_eq!(
            value["articulation_map"],
            json!({"value": [{"id": "sustain", "via": "keyswitch", "key": 36}],
                   "source": "profile"})
        );
        assert_eq!(
            value["verdict"],
            json!({"value": {"state": "partly", "lost": ["rt_decay"]}, "source": "profile"})
        );
        for absent in [
            "ensemble",
            "keymap",
            "key_offset",
            "dynamics",
            "legato",
            "in_situ",
            "license_claim",
        ] {
            assert_eq!(value[absent], json!(null), "{absent}");
        }
        assert_eq!(
            value["facts"],
            json!({
                "regions": 4, "playable_regions": 4,
                "sampled": [{"lo": 55, "hi": 60, "layers": [{"lo": 1, "hi": 127}]}],
                "switch_keys": [36, 37], "default_switch": 36,
                "ignored_opcodes": ["ampeg_dynamic"], "missing_samples": [],
                "unreadable_samples": [], "notes": [], "decoded_bytes": 1024, "loadable": true
            })
        );
    }
}
