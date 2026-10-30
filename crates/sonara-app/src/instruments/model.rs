//! The catalog's data: identifiers, presets, libraries, availability and the problems a scan
//! reports.

use std::fmt;
use std::path::PathBuf;

use serde::Serialize;

use super::family::FamilyClaim;

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
/// root, without the extension>`, for example `vsco2ce/Strings/Violin Solo`.
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

/// One instrument preset (`contracts/instrument-catalog.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstrumentPreset {
    /// The stable ID.
    pub id: InstrumentId,
    /// The name to show: the manifest's, else the patch's file name.
    pub display_name: String,
    /// The family and where that claim comes from.
    pub family: FamilyClaim,
    /// The library the preset belongs to.
    pub library: LibraryId,
    /// The patch file on this machine. SFZ presets have no bank or program: the patch is the
    /// preset.
    pub patch_path: PathBuf,
    /// Articulations a manifest declares for the patch. **Empty means not verified**, not "none":
    /// an articulation that is not listed is never presented as supported.
    pub articulations: Vec<String>,
    /// The licence note of the preset's library; `None` means **unknown**.
    pub license: Option<String>,
    /// Whether the preset can be played now.
    pub availability: Availability,
}

/// A library root the catalog read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Library {
    /// The library's ID.
    pub id: LibraryId,
    /// The name to show: the manifest's, else that of the shipped profile that gave the ID,
    /// else the ID.
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
    /// A manifest entry was ignored.
    ManifestEntryRejected {
        /// The manifest file.
        path: PathBuf,
        /// The entry's `path` as written.
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
    use super::{InstrumentId, LibraryId};

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
}
