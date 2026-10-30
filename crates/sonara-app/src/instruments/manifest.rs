//! The optional library manifest, `sonara-library.json` in a library's root directory, and the
//! shape that the library profiles Sonara ships share with it ([`super::profiles`]).
//!
//! Version 1 declares, per patch, a display name, a family and articulation names:
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
//! Version 2 keeps those fields and adds, per entry, the instrument it plays, solo or section,
//! how each articulation is reached, what drives dynamics, the legato kind, a key offset, a key
//! map for percussion, whether the library was recorded in seating position, a verdict and a
//! licence of its own, and entries for multi-patch sets ([`v2`]). `docs/instrument-catalog.md`
//! has the format.
//!
//! Only `version` is required. A manifest is read as untrusted input (TYP-06): it is size-capped,
//! its library-level fields are validated, and an entry whose path is absolute, climbs out of
//! the library with `..`, does not name an `.sfz` file, repeats an earlier entry or (in version
//! 2) holds any value that does not validate is rejected on its own, without discarding the rest
//! of the manifest. A version 1 manifest is read in one pass, as it always was, so a value of the
//! wrong type in one of its entries rejects the whole manifest; version 2 reads each entry on
//! its own, so the same mistake there rejects only that entry.
//!
//! Every field of an entry keeps whether it was left out, written as `null` or given a value
//! ([`Field`]), and who claims it ([`Claimant`]), so that a user's manifest can override a
//! shipped profile field by field ([`LibraryClaims::over`]).

mod claims;
mod v2;

use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;
use sonara_midi::orchestration::InstrumentType;

pub(crate) use claims::serialize_instrument;
pub use claims::{
    ArticulationEntry, ArticulationList, ArticulationName, ArticulationVia, CheckMethod,
    DynamicsMode, EnsembleClaim, EnsembleKind, KeymapEntry, LegatoMode, LibrarySource,
    ProfileCheck, Verdict, VerdictState,
};

use super::family::InstrumentFamily;
use super::model::{ClaimSource, InvalidLibraryId, LibraryId};

/// The manifest's file name.
pub const MANIFEST_FILE: &str = "sonara-library.json";
/// The newest manifest format version this build reads. It reads every version from 1 to this
/// one; a manifest of another version is rejected as a whole.
pub const MANIFEST_VERSION: u32 = 2;
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
        "manifest version {found} is not supported (this build reads versions 1 to {MANIFEST_VERSION})"
    )]
    Version { found: u32 },
    #[error(transparent)]
    LibraryId(#[from] InvalidLibraryId),
    #[error("the manifest's {field} is not valid: {reason}")]
    Library { field: &'static str, reason: String },
}

/// Who makes a claim: the library's own manifest, or a library profile Sonara ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Claimant {
    /// The manifest in the library's root directory: the library's own word.
    Manifest,
    /// A profile Sonara ships for the library: Sonara's word, not the library's.
    Profile,
}

impl Claimant {
    /// The source the catalog shows for a claim of this claimant.
    pub(crate) const fn source(self) -> ClaimSource {
        match self {
            Self::Manifest => ClaimSource::Declared,
            Self::Profile => ClaimSource::Profile,
        }
    }
}

/// One field of a manifest: left out, written as `null`, or given a value with who claims it.
///
/// The difference between the first two matters only when manifests are merged: a field that
/// is left out keeps the value of the source below it, one written as `null` clears it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum Field<T> {
    /// Not written.
    #[default]
    Unset,
    /// Written as `null`.
    Cleared,
    /// Written with a value.
    Set(T, Claimant),
}

impl<T: Clone> Field<T> {
    /// This field merged over `lower`, the same field of a source of lower precedence: written,
    /// as a value or as `null`, it wins; left out, `lower` stays.
    #[must_use]
    pub fn over(&self, lower: &Self) -> Self {
        match self {
            Self::Unset => lower.clone(),
            Self::Cleared | Self::Set(..) => self.clone(),
        }
    }
}

impl<T> Field<T> {
    /// The value, whoever claims it.
    #[must_use]
    pub const fn value(&self) -> Option<&T> {
        match self {
            Self::Set(value, _) => Some(value),
            Self::Unset | Self::Cleared => None,
        }
    }

    /// The value with who claims it.
    #[must_use]
    pub const fn claimed(&self) -> Option<(&T, Claimant)> {
        match self {
            Self::Set(value, claimant) => Some((value, *claimant)),
            Self::Unset | Self::Cleared => None,
        }
    }

    /// The value when the library's own manifest claims it.
    #[must_use]
    pub const fn declared(&self) -> Option<&T> {
        match self {
            Self::Set(value, Claimant::Manifest) => Some(value),
            Self::Set(_, Claimant::Profile) | Self::Unset | Self::Cleared => None,
        }
    }
}

/// What a manifest entry describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EntryTarget {
    /// One patch: its path below the library root, `/`-separated, as written.
    Path(String),
    /// A multi-patch set of one instrument, by its slug (`[a-z0-9-]`); its members are the
    /// paths of its articulations.
    Set(String),
}

impl EntryTarget {
    /// The key two entries are matched by: a path ignoring case, as discovery matches entries
    /// to the files on disk, or a set's slug.
    fn key(&self) -> (bool, String) {
        match self {
            Self::Path(path) => (false, path.to_lowercase()),
            Self::Set(slug) => (true, slug.clone()),
        }
    }

    /// How a rejected entry is named to the user: the path, or `@<slug>` for a set.
    fn label(&self) -> String {
        match self {
            Self::Path(path) => path.clone(),
            Self::Set(slug) => format!("@{slug}"),
        }
    }
}

/// A validated entry: a patch below the library root, or a multi-patch set, with what is claimed
/// about it. A version 1 entry sets only `name`, `family` and `articulations`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManifestEntry {
    pub target: EntryTarget,
    pub name: Field<String>,
    pub family: Field<InstrumentFamily>,
    pub instrument: Field<InstrumentType>,
    pub ensemble: Field<EnsembleClaim>,
    pub articulations: Field<ArticulationList>,
    pub keymap: Field<Vec<KeymapEntry>>,
    pub key_offset: Field<i8>,
    pub dynamics: Field<DynamicsMode>,
    pub legato: Field<LegatoMode>,
    pub in_situ: Field<bool>,
    pub verdict: Field<Verdict>,
    pub license: Field<String>,
}

impl ManifestEntry {
    /// An entry for `target` that claims nothing.
    pub(crate) fn new(target: EntryTarget) -> Self {
        Self {
            target,
            name: Field::Unset,
            family: Field::Unset,
            instrument: Field::Unset,
            ensemble: Field::Unset,
            articulations: Field::Unset,
            keymap: Field::Unset,
            key_offset: Field::Unset,
            dynamics: Field::Unset,
            legato: Field::Unset,
            in_situ: Field::Unset,
            verdict: Field::Unset,
            license: Field::Unset,
        }
    }

    /// This entry merged over `lower`, an entry for the same target from a source of lower
    /// precedence, field by field ([`Field::over`]). The articulations and the key map are
    /// replaced whole, never merged item by item. The target keeps this entry's spelling.
    #[must_use]
    pub fn over(&self, lower: &Self) -> Self {
        Self {
            target: self.target.clone(),
            name: self.name.over(&lower.name),
            family: self.family.over(&lower.family),
            instrument: self.instrument.over(&lower.instrument),
            ensemble: self.ensemble.over(&lower.ensemble),
            articulations: self.articulations.over(&lower.articulations),
            keymap: self.keymap.over(&lower.keymap),
            key_offset: self.key_offset.over(&lower.key_offset),
            dynamics: self.dynamics.over(&lower.dynamics),
            legato: self.legato.over(&lower.legato),
            in_situ: self.in_situ.over(&lower.in_situ),
            verdict: self.verdict.over(&lower.verdict),
            license: self.license.over(&lower.license),
        }
    }
}

/// What a manifest claims about its library and its entries: the part that merges.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LibraryClaims {
    pub name: Field<String>,
    pub license: Field<String>,
    pub attribution: Field<String>,
    pub source: Field<LibrarySource>,
    pub checked: Field<ProfileCheck>,
    pub entries: Vec<ManifestEntry>,
}

impl LibraryClaims {
    /// These claims merged over `lower`, the claims of a source of lower precedence: each
    /// library field as [`Field::over`]; an entry of both is merged field by field
    /// ([`ManifestEntry::over`]), an entry only in `lower` stays, and an entry only here is
    /// added after those of `lower`. Entries match by path ignoring case, or by set slug.
    #[must_use]
    pub fn over(&self, lower: &Self) -> Self {
        let mut entries: Vec<ManifestEntry> = lower
            .entries
            .iter()
            .map(|below| {
                let key = below.target.key();
                self.entries
                    .iter()
                    .find(|entry| entry.target.key() == key)
                    .map_or_else(|| below.clone(), |entry| entry.over(below))
            })
            .collect();
        entries.extend(
            self.entries
                .iter()
                .filter(|entry| {
                    let key = entry.target.key();
                    !lower.entries.iter().any(|below| below.target.key() == key)
                })
                .cloned(),
        );
        Self {
            name: self.name.over(&lower.name),
            license: self.license.over(&lower.license),
            attribution: self.attribution.over(&lower.attribution),
            source: self.source.over(&lower.source),
            checked: self.checked.over(&lower.checked),
            entries,
        }
    }
}

/// A validated manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Manifest {
    /// The format version it is written in.
    pub version: u32,
    pub id: Option<LibraryId>,
    /// Paths below the library root that recognize a copy of the library; only a shipped
    /// profile uses them.
    pub detect: Vec<String>,
    pub claims: LibraryClaims,
    /// Entries ignored, as `(entry, reason)`: the path as written, `@<slug>` for a set, or
    /// `#<n>`, the entry's position counted from 1, when it names neither.
    pub rejected: Vec<(String, String)>,
}

/// Reads the manifest of the library at `root`; `Ok(None)` when it has none. Its claims are the
/// library's own ([`Claimant::Manifest`]).
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
    parse(&bytes, Claimant::Manifest).map(Some)
}

/// The field every version has.
#[derive(Deserialize)]
struct RawVersion {
    version: u32,
}

/// Parses a manifest of any version this build reads; `source` says who claims what it holds.
pub(crate) fn parse(bytes: &[u8], source: Claimant) -> Result<Manifest, ManifestError> {
    let RawVersion { version } = serde_json::from_slice(bytes).map_err(ManifestError::Parse)?;
    match version {
        1 => parse_v1(bytes, source),
        2 => v2::parse(bytes, source),
        found => Err(ManifestError::Version { found }),
    }
}

#[derive(Deserialize)]
struct RawManifest {
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

/// Reads a version 1 manifest in one pass, as this format was always read. Version 1 cannot
/// write `null` over a value: a field it leaves out or writes as `null`, a blank text and an
/// empty articulation list all leave the field unset.
fn parse_v1(bytes: &[u8], source: Claimant) -> Result<Manifest, ManifestError> {
    let raw: RawManifest = serde_json::from_slice(bytes).map_err(ManifestError::Parse)?;
    let id = raw.id.as_deref().map(LibraryId::parse).transpose()?;
    let mut entries: Vec<ManifestEntry> = Vec::new();
    let mut rejected = Vec::new();
    for entry in raw.instruments {
        let listed = entries.iter().any(|e| match &e.target {
            EntryTarget::Path(path) => path.eq_ignore_ascii_case(&entry.path),
            EntryTarget::Set(_) => false,
        });
        match validate_v1_entry(&entry) {
            Ok(_) if listed => rejected.push((entry.path, "listed twice".to_owned())),
            Ok(family) => {
                let articulations = clean_articulations(entry.articulations);
                let mut claims = ManifestEntry::new(EntryTarget::Path(entry.path));
                claims.name = text_field(entry.name, source);
                claims.family = family.map_or(Field::Unset, |family| Field::Set(family, source));
                if !articulations.is_empty() {
                    claims.articulations =
                        Field::Set(ArticulationList::Names(articulations), source);
                }
                entries.push(claims);
            }
            Err(reason) => rejected.push((entry.path, reason)),
        }
    }
    Ok(Manifest {
        version: 1,
        id,
        detect: Vec::new(),
        claims: LibraryClaims {
            name: text_field(raw.name, source),
            license: text_field(raw.license, source),
            entries,
            ..LibraryClaims::default()
        },
        rejected,
    })
}

/// A text field of version 1: set unless it is missing or blank.
fn text_field(text: Option<String>, source: Claimant) -> Field<String> {
    text.filter(|text| !text.trim().is_empty())
        .map_or(Field::Unset, |text| Field::Set(text, source))
}

/// Checks a version 1 entry's path and family; returns the declared family.
fn validate_v1_entry(entry: &RawEntry) -> Result<Option<InstrumentFamily>, String> {
    validate_relative_path(&entry.path)?;
    entry.family.as_deref().map(parse_family).transpose()
}

/// The family with slug `name`, exactly as written.
fn parse_family(name: &str) -> Result<InstrumentFamily, String> {
    InstrumentFamily::from_slug(name).ok_or_else(|| format!("unknown family {name:?}"))
}

/// Checks a patch path below a library root: relative, `/`-separated, without empty, `.` or `..`
/// parts, naming an `.sfz` file. The reason, in words for the user, when it is not.
pub(super) fn validate_relative_path(path: &str) -> Result<(), String> {
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
    use super::{
        ArticulationList, Claimant, EntryTarget, Field, LibraryClaims, ManifestEntry,
        ManifestError, parse,
    };
    use crate::instruments::InstrumentFamily;

    const MANIFEST: Claimant = Claimant::Manifest;

    fn path(entry: &ManifestEntry) -> &str {
        match &entry.target {
            EntryTarget::Path(path) => path,
            EntryTarget::Set(slug) => panic!("a set {slug}"),
        }
    }

    #[test]
    fn a_full_manifest_is_read() {
        let manifest = parse(
            br#"{"version":1,"id":"lib-1","name":"Lib","license":"CC0",
            "instruments":[{"path":"S/Violin.sfz","name":"Violin","family":"strings",
            "articulations":[" sustain ","sustain","","pizz"]}]}"#,
            MANIFEST,
        )
        .expect("valid");
        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.id.expect("id").as_str(), "lib-1");
        assert_eq!(manifest.claims.name, Field::Set("Lib".to_owned(), MANIFEST));
        assert_eq!(
            manifest.claims.license.declared().map(String::as_str),
            Some("CC0")
        );
        let entry = &manifest.claims.entries[0];
        assert_eq!(path(entry), "S/Violin.sfz");
        assert_eq!(entry.name.value().map(String::as_str), Some("Violin"));
        assert_eq!(entry.family.declared(), Some(&InstrumentFamily::Strings));
        // Version 1 names are free text, never checked against the vocabulary.
        assert_eq!(
            entry.articulations.declared(),
            Some(&ArticulationList::Names(vec![
                "sustain".to_owned(),
                "pizz".to_owned()
            ]))
        );
        assert_eq!(entry.instrument, Field::Unset);
        assert_eq!(manifest.rejected.len(), 0);
    }

    /// Version 1 cannot clear a field: what it leaves out, writes as `null` or leaves blank is
    /// unset, so a profile's value below it stays.
    #[test]
    fn a_version_1_manifest_never_clears_a_field() {
        let manifest = parse(
            br#"{"version":1,"name":null,"license":"  ",
            "instruments":[{"path":"a.sfz","name":"","family":null,"articulations":[" "]},
            {"path":"b.sfz","articulations":[]}]}"#,
            MANIFEST,
        )
        .expect("valid");
        assert_eq!(manifest.claims.name, Field::Unset);
        assert_eq!(manifest.claims.license, Field::Unset);
        assert_eq!(manifest.claims.entries.len(), 2);
        for entry in &manifest.claims.entries {
            assert_eq!(entry.name, Field::Unset);
            assert_eq!(entry.family, Field::Unset);
            assert_eq!(entry.articulations, Field::Unset);
        }
    }

    #[test]
    fn unsafe_or_odd_entries_are_rejected_one_by_one() {
        let manifest = parse(
            br#"{"version":1,"instruments":[
            {"path":"/etc/x.sfz"},{"path":"../x.sfz"},{"path":"a/../x.sfz"},{"path":"a//x.sfz"},
            {"path":"a\\x.sfz"},{"path":"readme.txt"},{"path":"ok.sfz","family":"kazoo"},
            {"path":"fine.sfz"},{"path":"fine.sfz"}]}"#,
            MANIFEST,
        )
        .expect("the manifest itself is valid");
        let kept: Vec<_> = manifest.claims.entries.iter().map(path).collect();
        assert_eq!(kept, ["fine.sfz"]);
        let reasons: Vec<_> = manifest.rejected.iter().map(|(_, r)| r.as_str()).collect();
        assert_eq!(manifest.rejected.len(), 8);
        assert!(reasons.contains(&"unknown family \"kazoo\""));
        assert!(reasons.contains(&"listed twice"));
    }

    /// Every family slug is read as its family, in both versions; the match is exact, so a
    /// capitalised or padded slug rejects the entry (TST-03, TST-05).
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
        for version in [1, 2] {
            let manifest_with = |family: &str| {
                parse(
                    format!(
                        r#"{{"version":{version},"instruments":[{{"path":"a.sfz","family":"{family}"}}]}}"#
                    )
                    .as_bytes(),
                    MANIFEST,
                )
                .expect("the manifest itself is valid")
            };
            for (slug, family) in families {
                let manifest = manifest_with(slug);
                assert_eq!(
                    manifest.claims.entries[0].family.declared(),
                    Some(&family),
                    "{slug}"
                );
            }
            for text in ["Strings", "STRINGS", "strings ", "string", ""] {
                let manifest = manifest_with(text);
                assert!(manifest.claims.entries.is_empty(), "{text:?}");
                assert_eq!(
                    manifest.rejected,
                    [("a.sfz".to_owned(), format!("unknown family {text:?}"))]
                );
            }
        }
    }

    #[test]
    fn a_broken_manifest_is_an_error() {
        assert!(matches!(
            parse(b"not json", MANIFEST),
            Err(ManifestError::Parse(_))
        ));
        for version in [0, 3, 100] {
            assert!(matches!(
                parse(format!(r#"{{"version":{version}}}"#).as_bytes(), MANIFEST),
                Err(ManifestError::Version { found }) if found == version
            ));
        }
        for version in [1, 2] {
            assert!(matches!(
                parse(
                    format!(r#"{{"version":{version},"id":"Bad Id"}}"#).as_bytes(),
                    MANIFEST
                ),
                Err(ManifestError::LibraryId(_))
            ));
        }
        assert!(matches!(
            parse(br"{}", MANIFEST),
            Err(ManifestError::Parse(_))
        ));
        assert!(matches!(
            parse(br#"{"version":"2"}"#, MANIFEST),
            Err(ManifestError::Parse(_))
        ));
        // Version 1 reads its entries in one pass: one entry of the wrong type rejects it all.
        assert!(matches!(
            parse(
                br#"{"version":1,"instruments":[{"path":"a.sfz"},{"path":5}]}"#,
                MANIFEST
            ),
            Err(ManifestError::Parse(_))
        ));
    }

    #[test]
    fn a_bare_version_2_manifest_is_empty() {
        let manifest = parse(br#"{"version":2}"#, MANIFEST).expect("valid");
        assert_eq!(manifest.version, 2);
        assert_eq!(manifest.id, None);
        assert_eq!(manifest.claims, LibraryClaims::default());
        assert!(manifest.rejected.is_empty() && manifest.detect.is_empty());
    }

    /// `value` as JSON.
    fn json(value: impl serde::Serialize) -> serde_json::Value {
        serde_json::to_value(value).expect("a claim serializes")
    }

    /// The catalog serializes a claimed value as the manifest writes it, so every value read
    /// from an entry serializes back to the JSON it was read from (TST-05; the manifest is the
    /// oracle).
    #[test]
    fn claimed_values_serialize_as_the_manifest_writes_them() {
        let entries = serde_json::json!([
            {"path": "a.sfz",
             "ensemble": {"kind": "section_emulated", "players": 12},
             "articulations": [
                 {"id": "sustain", "via": "keyswitch", "key": 36, "label": "Sus",
                  "default": true, "vibrato": "non_vibrato", "attack_ms": 40},
                 {"id": "sustain", "via": "velocity", "lo": 1, "hi": 63, "mute": "con_sordino"},
                 {"id": "staccato", "via": "cc", "cc": 32, "value": 5},
                 {"id": "ordinario", "via": "patch"}],
             "dynamics": {"via": "fixed", "velocity": 100},
             "legato": {"kind": "overlap", "crossfade_ms": 90, "start_offset_ms": 60,
                        "gap_ms": 0},
             "verdict": {"state": "partly", "lost": ["rt_decay"]}},
            {"path": "b.sfz", "ensemble": {"kind": "solo", "players": null},
             "keymap": [
                 {"keys": [36, 37], "instrument": "bass_drum", "articulation": "roll",
                  "name": "Kick"},
                 {"keys": [38], "instrument": "snare_drum"}],
             "dynamics": {"via": "cc", "cc": 11}, "legato": {"kind": "trigger"},
             "verdict": {"state": "not", "lost": []}},
            {"set": "horn", "dynamics": {"via": "velocity"}, "legato": {"kind": "none"},
             "articulations": [{"id": "sustain", "via": "patch", "path": "Horn.sfz"}],
             "verdict": {"state": "as_is", "lost": []}}
        ]);
        let manifest = parse(
            serde_json::json!({"version": 2, "instruments": entries})
                .to_string()
                .as_bytes(),
            MANIFEST,
        )
        .expect("valid");
        assert_eq!(manifest.rejected, []);
        let written = entries.as_array().expect("a list");
        assert_eq!(manifest.claims.entries.len(), written.len());
        for (entry, written) in manifest.claims.entries.iter().zip(written) {
            // A field the entry leaves out is unset, which serializes as `null`.
            let field = |name: &str| written.get(name).cloned().unwrap_or_default();
            assert_eq!(json(entry.ensemble.value()), field("ensemble"));
            assert_eq!(json(entry.articulations.value()), field("articulations"));
            assert_eq!(json(entry.keymap.value()), field("keymap"));
            assert_eq!(json(entry.dynamics.value()), field("dynamics"));
            assert_eq!(json(entry.legato.value()), field("legato"));
            assert_eq!(json(entry.verdict.value()), field("verdict"));
        }
        // Version 1 names stay a list of names.
        let names = ArticulationList::Names(vec!["pizz".to_owned(), "sustain".to_owned()]);
        assert_eq!(
            serde_json::to_value(&names).expect("serializes"),
            serde_json::json!(["pizz", "sustain"])
        );
    }

    #[test]
    fn a_field_over_another_keeps_or_replaces_it() {
        let shipped = Field::Set(1, Claimant::Profile);
        let user = Field::Set(2, Claimant::Manifest);
        assert_eq!(Field::Unset.over(&shipped), shipped);
        assert_eq!(Field::Cleared.over(&shipped), Field::Cleared);
        assert_eq!(user.over(&shipped), user);
        assert_eq!(Field::<u8>::Unset.over(&Field::Unset), Field::Unset);
        assert_eq!(shipped.value(), Some(&1));
        assert_eq!(
            shipped.declared(),
            None,
            "a profile's value is not declared"
        );
        assert_eq!(user.declared(), Some(&2));
        assert_eq!(Field::<u8>::Cleared.value(), None);
    }
}
