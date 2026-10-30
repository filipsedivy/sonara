//! The optional library manifest, `sonara-library.json` in a library's root directory.
//!
//! ```json
//! {
//!   "version": 1,
//!   "id": "vsco2ce",
//!   "name": "VSCO 2 Community Edition",
//!   "license": "CC0 1.0",
//!   "instruments": [
//!     { "path": "Strings/Violin Solo.sfz", "name": "Solo Violin", "family": "strings",
//!       "articulations": ["sustain", "pizzicato"] }
//!   ]
//! }
//! ```
//!
//! Only `version` is required. A manifest is read as untrusted input (TYP-06): it is size-capped,
//! its library ID is validated, and an entry whose path is absolute, climbs out of the library
//! with `..`, does not name an `.sfz` file or repeats an earlier entry is rejected on its own,
//! without discarding the rest of the manifest.

use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;

use super::family::InstrumentFamily;
use super::model::{InvalidLibraryId, LibraryId};

/// The manifest's file name.
pub const MANIFEST_FILE: &str = "sonara-library.json";
/// The manifest format version this build reads.
pub const MANIFEST_VERSION: u32 = 1;
/// The largest manifest read, in bytes.
const MAX_MANIFEST_BYTES: u64 = 1 << 20;

/// A manifest that could not be used as a whole.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ManifestError {
    #[error("cannot read the manifest: {0}")]
    Io(#[source] io::Error),
    #[error("the manifest is larger than {MAX_MANIFEST_BYTES} bytes")]
    TooLarge,
    #[error("the manifest is not valid: {0}")]
    Parse(#[source] serde_json::Error),
    #[error(
        "manifest version {found} is not supported (this build reads version {MANIFEST_VERSION})"
    )]
    Version { found: u32 },
    #[error(transparent)]
    LibraryId(#[from] InvalidLibraryId),
}

#[derive(Deserialize)]
struct RawManifest {
    version: u32,
    id: Option<String>,
    name: Option<String>,
    license: Option<String>,
    #[serde(default)]
    instruments: Vec<RawEntry>,
}

#[derive(Deserialize)]
struct RawEntry {
    path: String,
    name: Option<String>,
    family: Option<String>,
    #[serde(default)]
    articulations: Vec<String>,
}

/// A validated manifest.
#[derive(Debug)]
pub(crate) struct Manifest {
    pub id: Option<LibraryId>,
    pub name: Option<String>,
    pub license: Option<String>,
    pub entries: Vec<ManifestEntry>,
    /// Entries ignored, as `(path as written, reason)`.
    pub rejected: Vec<(String, String)>,
}

/// A validated entry: a patch path below the library root with the metadata declared for it.
#[derive(Debug)]
pub(crate) struct ManifestEntry {
    pub relative_path: String,
    pub name: Option<String>,
    pub family: Option<InstrumentFamily>,
    pub articulations: Vec<String>,
}

/// Reads the manifest of the library at `root`; `Ok(None)` when it has none.
pub(crate) fn read(root: &Path) -> Result<Option<Manifest>, ManifestError> {
    let path = root.join(MANIFEST_FILE);
    let size = match fs::metadata(&path) {
        Ok(meta) => meta.len(),
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(ManifestError::Io(err)),
    };
    if size > MAX_MANIFEST_BYTES {
        return Err(ManifestError::TooLarge);
    }
    let bytes = fs::read(&path).map_err(ManifestError::Io)?;
    parse(&bytes).map(Some)
}

fn parse(bytes: &[u8]) -> Result<Manifest, ManifestError> {
    let raw: RawManifest = serde_json::from_slice(bytes).map_err(ManifestError::Parse)?;
    if raw.version != MANIFEST_VERSION {
        return Err(ManifestError::Version { found: raw.version });
    }
    let id = raw.id.as_deref().map(LibraryId::parse).transpose()?;
    let mut entries: Vec<ManifestEntry> = Vec::new();
    let mut rejected = Vec::new();
    for entry in raw.instruments {
        match validate_entry(&entry) {
            Ok(_)
                if entries
                    .iter()
                    .any(|e| e.relative_path.eq_ignore_ascii_case(&entry.path)) =>
            {
                rejected.push((entry.path, "listed twice".to_owned()));
            }
            Ok(family) => entries.push(ManifestEntry {
                relative_path: entry.path,
                name: entry.name.filter(|name| !name.trim().is_empty()),
                family,
                articulations: clean_articulations(entry.articulations),
            }),
            Err(reason) => rejected.push((entry.path, reason)),
        }
    }
    Ok(Manifest {
        id,
        name: raw.name.filter(|name| !name.trim().is_empty()),
        license: raw.license.filter(|text| !text.trim().is_empty()),
        entries,
        rejected,
    })
}

/// Checks an entry's path and family; returns the declared family.
fn validate_entry(entry: &RawEntry) -> Result<Option<InstrumentFamily>, String> {
    validate_relative_path(&entry.path)?;
    entry
        .family
        .as_deref()
        .map(|name| {
            serde_json::from_value::<InstrumentFamily>(serde_json::Value::String(name.to_owned()))
                .map_err(|_| format!("unknown family {name:?}"))
        })
        .transpose()
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    if path.contains('\\') || path.contains('\0') {
        return Err("the path must use '/' separators".to_owned());
    }
    if path.starts_with('/') {
        return Err("the path must be relative to the library".to_owned());
    }
    if path
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("the path must not hold empty, '.' or '..' parts".to_owned());
    }
    let is_sfz = Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("sfz"));
    if is_sfz {
        Ok(())
    } else {
        Err("the path must name an .sfz file".to_owned())
    }
}

/// Trims the names and drops empty and repeated ones, keeping the declared order.
fn clean_articulations(names: Vec<String>) -> Vec<String> {
    let mut clean: Vec<String> = Vec::new();
    for name in names {
        let name = name.trim().to_owned();
        if !name.is_empty() && !clean.contains(&name) {
            clean.push(name);
        }
    }
    clean
}

#[cfg(test)]
mod tests {
    use super::{ManifestError, parse};
    use crate::instruments::InstrumentFamily;

    #[test]
    fn a_full_manifest_is_read() {
        let manifest = parse(
            br#"{"version":1,"id":"lib-1","name":"Lib","license":"CC0",
            "instruments":[{"path":"S/Violin.sfz","name":"Violin","family":"strings",
            "articulations":[" sustain ","sustain","","pizz"]}]}"#,
        )
        .expect("valid");
        assert_eq!(manifest.id.expect("id").as_str(), "lib-1");
        assert_eq!(manifest.license.as_deref(), Some("CC0"));
        let entry = &manifest.entries[0];
        assert_eq!(entry.relative_path, "S/Violin.sfz");
        assert_eq!(entry.family, Some(InstrumentFamily::Strings));
        assert_eq!(entry.articulations, ["sustain", "pizz"]);
        assert_eq!(manifest.rejected.len(), 0);
    }

    #[test]
    fn unsafe_or_odd_entries_are_rejected_one_by_one() {
        let manifest = parse(
            br#"{"version":1,"instruments":[
            {"path":"/etc/x.sfz"},{"path":"../x.sfz"},{"path":"a/../x.sfz"},{"path":"a//x.sfz"},
            {"path":"a\\x.sfz"},{"path":"readme.txt"},{"path":"ok.sfz","family":"kazoo"},
            {"path":"fine.sfz"},{"path":"fine.sfz"}]}"#,
        )
        .expect("the manifest itself is valid");
        let kept: Vec<_> = manifest
            .entries
            .iter()
            .map(|e| e.relative_path.as_str())
            .collect();
        assert_eq!(kept, ["fine.sfz"]);
        let reasons: Vec<_> = manifest.rejected.iter().map(|(_, r)| r.as_str()).collect();
        assert_eq!(manifest.rejected.len(), 8);
        assert!(reasons.contains(&"unknown family \"kazoo\""));
        assert!(reasons.contains(&"listed twice"));
    }

    /// Every family slug is read as its family; the match is exact, so a capitalised or padded
    /// slug rejects the entry (TST-03, TST-05).
    #[test]
    fn a_family_is_read_from_its_exact_slug() {
        let families = [
            ("strings", InstrumentFamily::Strings),
            ("woodwinds", InstrumentFamily::Woodwinds),
            ("brass", InstrumentFamily::Brass),
            ("percussion", InstrumentFamily::Percussion),
            ("keyboards", InstrumentFamily::Keyboards),
            ("plucked", InstrumentFamily::Plucked),
            ("choir", InstrumentFamily::Choir),
            ("other", InstrumentFamily::Other),
        ];
        for (slug, family) in families {
            let manifest = parse(
                format!(r#"{{"version":1,"instruments":[{{"path":"a.sfz","family":"{slug}"}}]}}"#)
                    .as_bytes(),
            )
            .expect("valid");
            assert_eq!(manifest.entries[0].family, Some(family), "{slug}");
        }
        for text in ["Strings", "STRINGS", "strings ", "string", ""] {
            let manifest = parse(
                format!(r#"{{"version":1,"instruments":[{{"path":"a.sfz","family":"{text}"}}]}}"#)
                    .as_bytes(),
            )
            .expect("the manifest itself is valid");
            assert!(manifest.entries.is_empty(), "{text:?}");
            assert_eq!(
                manifest.rejected,
                [("a.sfz".to_owned(), format!("unknown family {text:?}"))]
            );
        }
    }

    #[test]
    fn a_broken_manifest_is_an_error() {
        assert!(matches!(parse(b"not json"), Err(ManifestError::Parse(_))));
        assert!(matches!(
            parse(br#"{"version":2}"#),
            Err(ManifestError::Version { found: 2 })
        ));
        assert!(matches!(
            parse(br#"{"version":1,"id":"Bad Id"}"#),
            Err(ManifestError::LibraryId(_))
        ));
        assert!(matches!(parse(br"{}"), Err(ManifestError::Parse(_))));
    }
}
