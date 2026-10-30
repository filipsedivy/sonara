//! Discovery: merges library manifests with a scan of the library directories.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::family::{FamilyClaim, InstrumentFamily, infer_family};
use super::manifest::{self, ArticulationList, EntryTarget, LibraryClaims};
use super::model::{
    Availability, CatalogError, InstrumentId, InstrumentPreset, Library, LibraryId, LibraryIssue,
};
use super::profiles::{self, Detected, Profile};
use super::verify::PatchLoader;
use crate::samples::{MAX_PATCHES, library_roots, scan_patches};
use crate::settings::Settings;

/// The instrument presets of the libraries installed on this machine, their libraries and the
/// problems found while reading them. A snapshot: build a new one to see changes on disk.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct InstrumentCatalog {
    libraries: Vec<Library>,
    presets: Vec<InstrumentPreset>,
    issues: Vec<LibraryIssue>,
}

impl InstrumentCatalog {
    /// Discovers the presets below the library directories `roots`.
    ///
    /// Never fails: a directory that is missing, is not a directory or has a broken manifest is
    /// reported in [`Self::issues`] and the rest is still read. Hidden entries, symbolic links and
    /// folders named `includes` are not followed ([`crate::samples::scan_patches`]); a manifest
    /// entry that names a file in an `includes` folder is still listed. The scan reads directory
    /// entries and the small manifests only; it opens no sample and parses no patch, so a patch
    /// without regions is available until [`Self::verify`] loads it.
    #[must_use]
    pub fn discover(roots: &[PathBuf]) -> Self {
        Self::discover_with(roots, profiles::shipped())
    }

    /// [`Self::discover`] with the library profiles `shipped` in place of those Sonara ships.
    pub(crate) fn discover_with(roots: &[PathBuf], shipped: &[Profile]) -> Self {
        let mut catalog = Self::default();
        for root in roots {
            catalog.add_library(root, shipped);
        }
        catalog.presets.sort_by(|a, b| a.id.cmp(&b.id));
        catalog
    }

    /// Discovers the presets of the libraries in the settings key `sample_libraries`.
    #[must_use]
    pub fn from_settings(settings: &Settings) -> Self {
        Self::discover(&library_roots(settings))
    }

    /// The libraries that were read, in the order of the configured directories.
    #[must_use]
    pub fn libraries(&self) -> &[Library] {
        &self.libraries
    }

    /// Every preset, sorted by ID, available or not.
    #[must_use]
    pub fn presets(&self) -> &[InstrumentPreset] {
        &self.presets
    }

    /// The problems found while discovering.
    #[must_use]
    pub fn issues(&self) -> &[LibraryIssue] {
        &self.issues
    }

    /// The preset with this ID.
    #[must_use]
    pub fn get(&self, id: &InstrumentId) -> Option<&InstrumentPreset> {
        self.presets
            .binary_search_by(|preset| preset.id.cmp(id))
            .ok()
            .and_then(|index| self.presets.get(index))
    }

    /// The presets that can be played now.
    pub fn available(&self) -> impl Iterator<Item = &InstrumentPreset> {
        self.presets
            .iter()
            .filter(|preset| preset.availability.is_available())
    }

    /// The presets whose family is `family`, declared or inferred.
    pub fn in_family(&self, family: InstrumentFamily) -> impl Iterator<Item = &InstrumentPreset> {
        self.presets
            .iter()
            .filter(move |preset| preset.family.family() == Some(family))
    }

    /// Loads the preset's patch through `loader` and records the verdict: a patch that does not
    /// load becomes [`Availability::Invalid`]. A preset that is not available is not loaded and
    /// keeps its state. Returns the availability after the check.
    ///
    /// # Errors
    ///
    /// [`CatalogError::UnknownInstrument`] when no preset has this ID.
    pub fn verify(
        &mut self,
        id: &InstrumentId,
        loader: &dyn PatchLoader,
    ) -> Result<&Availability, CatalogError> {
        let index = self
            .presets
            .binary_search_by(|preset| preset.id.cmp(id))
            .map_err(|_| CatalogError::UnknownInstrument(id.to_string()))?;
        let preset = self
            .presets
            .get_mut(index)
            .ok_or_else(|| CatalogError::UnknownInstrument(id.to_string()))?;
        if preset.availability.is_available()
            && let Err(err) = loader.load(&preset.patch_path)
        {
            preset.availability = Availability::Invalid {
                reason: err.to_string(),
            };
        }
        Ok(&preset.availability)
    }

    fn add_library(&mut self, root: &Path, shipped: &[Profile]) {
        match fs::metadata(root) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => {
                self.issues.push(LibraryIssue::RootNotDirectory {
                    path: root.to_owned(),
                });
                return;
            }
            Err(_) => {
                self.issues.push(LibraryIssue::RootMissing {
                    path: root.to_owned(),
                });
                return;
            }
        }
        let manifest = match manifest::read(root) {
            Ok(manifest) => manifest,
            Err(err) => {
                self.issues.push(LibraryIssue::ManifestInvalid {
                    path: root.join(manifest::MANIFEST_FILE),
                    reason: err.to_string(),
                });
                None
            }
        };
        // The ID: the manifest's, else a shipped profile's whose detect files are all here,
        // else the folder's slug. A profile ID survives renaming the folder. A manifest that
        // declares a profile's ID names that library, so the profile applies to it as well.
        let declared = manifest.as_ref().and_then(|m| m.id.clone());
        let profile = match &declared {
            Some(id) => shipped.iter().find(|profile| profile.id == *id),
            None => match profiles::detect(root, shipped) {
                Detected::None => None,
                Detected::One(profile) => Some(profile),
                Detected::Ambiguous(ids) => {
                    self.issues.push(LibraryIssue::ProfileAmbiguous {
                        path: root.to_owned(),
                        profiles: ids,
                    });
                    None
                }
            },
        };
        let id = declared
            .or_else(|| profile.map(|profile| profile.id.clone()))
            .unwrap_or_else(|| {
                LibraryId::from_directory_name(
                    &root
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                )
            });
        if self.libraries.iter().any(|library| library.id == id) {
            self.issues.push(LibraryIssue::DuplicateLibrary {
                id,
                path: root.to_owned(),
            });
            return;
        }
        if let Some(manifest) = &manifest {
            for (entry, reason) in &manifest.rejected {
                self.issues.push(LibraryIssue::ManifestEntryRejected {
                    path: root.join(manifest::MANIFEST_FILE),
                    entry: entry.clone(),
                    reason: reason.clone(),
                });
            }
        }
        // The library's own manifest over the shipped profile, field by field.
        let claims = profiles::merge(
            profile.map(|profile| &profile.claims),
            manifest.as_ref().map(|manifest| &manifest.claims),
        );
        let name = claims
            .name
            .value()
            .cloned()
            .unwrap_or_else(|| id.to_string());
        self.libraries.push(Library {
            id: id.clone(),
            name,
            root: root.to_owned(),
            license: claims.license.declared().cloned(),
            has_manifest: manifest.is_some(),
        });
        self.add_presets(root, &id, &claims);
    }

    /// Lists the patches of the library at `root`: those its claims name, and every other patch
    /// the scan finds. A claim of the shipped profile is not the library's word, so only the
    /// library's own manifest declares a family, articulations or a licence here; the profile
    /// adds its entries (a patch it names that is not on disk is listed as missing) and their
    /// names. A multi-patch set is not a patch: its members are listed as the patches they are.
    fn add_presets(&mut self, root: &Path, library: &LibraryId, claims: &LibraryClaims) {
        let license = claims.license.declared().cloned();
        let canonical_root = fs::canonicalize(root).ok();
        let scanned = scan_patches(&[root.to_owned()]);
        if scanned.len() >= MAX_PATCHES {
            self.issues.push(LibraryIssue::ScanTruncated {
                path: root.to_owned(),
                limit: MAX_PATCHES,
            });
        }
        // Scanned patches by lower-case relative path: macOS volumes are usually
        // case-insensitive, so a manifest may spell a path differently from the disk.
        let mut found: BTreeMap<String, String> = scanned
            .iter()
            .filter_map(|path| relative_to(root, Path::new(path)))
            .map(|relative| (relative.to_lowercase(), relative))
            .collect();

        let mut presets = Vec::new();
        for entry in &claims.entries {
            let EntryTarget::Path(path) = &entry.target else {
                continue;
            };
            let relative = found
                .remove(&path.to_lowercase())
                .unwrap_or_else(|| path.clone());
            let patch_path = root.join(&relative);
            presets.push(InstrumentPreset {
                id: InstrumentId::new(library, &relative),
                display_name: entry
                    .name
                    .value()
                    .cloned()
                    .unwrap_or_else(|| file_stem(&relative)),
                family: entry
                    .family
                    .declared()
                    .map_or_else(|| infer_family(&relative), |f| FamilyClaim::Declared(*f)),
                library: library.clone(),
                availability: check_file(&patch_path, canonical_root.as_deref()),
                patch_path,
                articulations: entry
                    .articulations
                    .declared()
                    .map(ArticulationList::ids)
                    .unwrap_or_default()
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                license: entry
                    .license
                    .declared()
                    .cloned()
                    .or_else(|| license.clone()),
            });
        }
        for relative in found.into_values() {
            let patch_path = root.join(&relative);
            presets.push(InstrumentPreset {
                id: InstrumentId::new(library, &relative),
                display_name: file_stem(&relative),
                family: infer_family(&relative),
                library: library.clone(),
                availability: check_file(&patch_path, canonical_root.as_deref()),
                patch_path,
                articulations: Vec::new(),
                license: license.clone(),
            });
        }
        self.presets.extend(presets);
    }
}

/// `path` below `root` as a `/`-separated string; `None` when it is not below it or not UTF-8.
pub(super) fn relative_to(root: &Path, path: &Path) -> Option<String> {
    let parts: Option<Vec<&str>> = path
        .strip_prefix(root)
        .ok()?
        .components()
        .map(|part| part.as_os_str().to_str())
        .collect();
    parts.map(|parts| parts.join("/"))
}

fn file_stem(relative: &str) -> String {
    let name = relative.rsplit('/').next().unwrap_or(relative);
    name.rsplit_once('.')
        .map_or(name, |(stem, _)| stem)
        .to_owned()
}

/// The light availability check: a regular, non-empty, readable file inside the library.
fn check_file(path: &Path, canonical_root: Option<&Path>) -> Availability {
    let invalid = |reason: String| Availability::Invalid { reason };
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Availability::Missing,
        Err(err) => return invalid(format!("cannot be read: {err}")),
    };
    if meta.file_type().is_symlink() {
        return invalid("it is a symbolic link, which Sonara does not follow".to_owned());
    }
    if !meta.is_file() {
        return invalid("it is not a file".to_owned());
    }
    if meta.len() == 0 {
        return invalid("the file is empty".to_owned());
    }
    // A manifest path may pass through a linked folder; the file must still be in the library.
    let inside = match (fs::canonicalize(path), canonical_root) {
        (Ok(real), Some(root)) => real.starts_with(root),
        (Err(_), _) | (_, None) => false,
    };
    if !inside {
        return invalid("it is outside the library directory".to_owned());
    }
    match fs::File::open(path) {
        Ok(_) => Availability::Available,
        Err(err) => invalid(format!("cannot be opened: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use serde_json::json;

    use super::InstrumentCatalog;
    use crate::instruments::profiles::{self, Profile};
    use crate::instruments::{
        Availability, FamilyClaim, InstrumentFamily, InstrumentPreset, LibraryIssue, MANIFEST_FILE,
    };

    fn patch(path: &Path) {
        fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
        fs::write(path, "<region> sample=a.wav\n").expect("write");
    }

    /// A profile of the library `lib`, recognized by `Detect.sfz`, with claims of every kind
    /// about one patch on disk and one that is not.
    fn profile() -> Profile {
        profiles::parse(
            &json!({
                "version": 2, "id": "lib", "name": "Profiled Library", "license": "CC0 1.0",
                "detect": ["Detect.sfz"],
                "instruments": [
                    {"path": "Strings/Violin.sfz", "name": "Profile Violin", "family": "brass",
                     "instrument": "violin", "license": "Profile licence",
                     "articulations": [{"id": "sustain", "via": "keyswitch", "key": 36}]},
                    {"path": "Gone.sfz", "name": "Gone Patch"},
                    {"set": "violin-set", "instrument": "violin",
                     "articulations": [{"id": "sustain", "via": "patch",
                                        "path": "Strings/Violin.sfz"}]}
                ]
            })
            .to_string(),
        )
        .expect("a valid profile")
    }

    fn preset<'a>(catalog: &'a InstrumentCatalog, id: &str) -> &'a InstrumentPreset {
        catalog
            .presets()
            .iter()
            .find(|preset| preset.id.as_str() == id)
            .unwrap_or_else(|| panic!("no preset {id}"))
    }

    fn ids(catalog: &InstrumentCatalog) -> Vec<&str> {
        catalog.presets().iter().map(|p| p.id.as_str()).collect()
    }

    /// A profile adds its entries and their names, but its family, articulations and licence
    /// are not the library's word: without a manifest, nothing is declared (provenance).
    #[test]
    fn a_profile_lists_its_patches_but_declares_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("Some Folder");
        patch(&root.join("Detect.sfz"));
        patch(&root.join("Strings/Violin.sfz"));

        let catalog = InstrumentCatalog::discover_with(std::slice::from_ref(&root), &[profile()]);

        let library = &catalog.libraries()[0];
        assert_eq!(library.id.as_str(), "lib");
        assert_eq!(library.name, "Profiled Library");
        assert_eq!(library.license, None);
        assert!(!library.has_manifest);
        // A set is not a patch; its member is listed as the patch it is.
        assert_eq!(
            ids(&catalog),
            ["lib/Detect", "lib/Gone", "lib/Strings/Violin"]
        );
        let violin = preset(&catalog, "lib/Strings/Violin");
        assert_eq!(violin.display_name, "Profile Violin");
        assert_eq!(
            violin.family,
            FamilyClaim::Inferred(InstrumentFamily::Strings)
        );
        assert_eq!(violin.articulations, Vec::<String>::new());
        assert_eq!(violin.license, None);
        assert_eq!(violin.availability, Availability::Available);
        // A patch the profile names that is not on disk is listed as missing, by its name.
        let gone = preset(&catalog, "lib/Gone");
        assert_eq!(gone.availability, Availability::Missing);
        assert_eq!(gone.display_name, "Gone Patch");
        assert_eq!(catalog.issues(), []);
    }

    /// The library's own manifest declares over the profile, field by field; what it leaves
    /// out keeps the profile's value, and a manifest that declares the profile's ID takes the
    /// profile without its detect files.
    #[test]
    fn a_manifest_declares_over_the_profile() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("Renamed");
        patch(&root.join("Strings/Violin.sfz"));
        fs::write(
            root.join(MANIFEST_FILE),
            json!({
                "version": 2, "id": "lib", "license": "CC0 1.0",
                "instruments": [
                    {"path": "strings/violin.sfz", "family": "strings",
                     "articulations": [{"id": "pizzicato", "via": "patch"},
                                       {"id": "ordinario", "via": "patch", "default": true}]},
                    {"path": "Gone.sfz", "name": null},
                    {"path": "Bad.sfz", "instrument": "kazoo"}
                ]
            })
            .to_string(),
        )
        .expect("write manifest");

        let catalog = InstrumentCatalog::discover_with(std::slice::from_ref(&root), &[profile()]);

        let library = &catalog.libraries()[0];
        assert_eq!(library.id.as_str(), "lib");
        assert_eq!(library.name, "Profiled Library");
        assert_eq!(library.license.as_deref(), Some("CC0 1.0"));
        assert!(library.has_manifest);
        let violin = preset(&catalog, "lib/Strings/Violin");
        assert_eq!(violin.display_name, "Profile Violin");
        assert_eq!(
            violin.family,
            FamilyClaim::Declared(InstrumentFamily::Strings)
        );
        assert_eq!(violin.articulations, ["pizzicato", "ordinario"]);
        // The profile's licence for the recording is not declared; the library's is.
        assert_eq!(violin.license.as_deref(), Some("CC0 1.0"));
        // `null` clears the profile's name, so the file name shows.
        assert_eq!(preset(&catalog, "lib/Gone").display_name, "Gone");
        // The bad entry is reported alone; the rest of the manifest is used.
        assert!(matches!(
            catalog.issues(),
            [LibraryIssue::ManifestEntryRejected { path, entry, reason }]
                if path == &root.join(MANIFEST_FILE)
                    && entry == "Bad.sfz"
                    && reason == "unknown instrument \"kazoo\""
        ));
        assert_eq!(ids(&catalog), ["lib/Gone", "lib/Strings/Violin"]);
    }

    /// A manifest with another ID takes no profile, even when the profile's detect files are
    /// there.
    #[test]
    fn a_manifest_with_another_id_takes_no_profile() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("Lib");
        patch(&root.join("Detect.sfz"));
        fs::write(
            root.join(MANIFEST_FILE),
            json!({"version": 1, "id": "mine"}).to_string(),
        )
        .expect("write manifest");
        let catalog = InstrumentCatalog::discover_with(std::slice::from_ref(&root), &[profile()]);
        assert_eq!(catalog.libraries()[0].name, "mine");
        assert_eq!(ids(&catalog), ["mine/Detect"]);
    }
}
