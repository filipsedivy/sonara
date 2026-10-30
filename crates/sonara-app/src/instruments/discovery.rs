//! Discovery: merges library manifests with a scan of the library directories.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::family::{FamilyClaim, InstrumentFamily, infer_family};
use super::manifest::{self, Manifest};
use super::model::{
    Availability, CatalogError, InstrumentId, InstrumentPreset, Library, LibraryId, LibraryIssue,
};
use super::profiles::{self, Detected};
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
        let mut catalog = Self::default();
        for root in roots {
            catalog.add_library(root);
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

    fn add_library(&mut self, root: &Path) {
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
        // else the folder's slug. A profile ID survives renaming the folder.
        let declared = manifest.as_ref().and_then(|m| m.id.clone());
        let profile = if declared.is_some() {
            None
        } else {
            match profiles::detect(root, profiles::shipped()) {
                Detected::None => None,
                Detected::One(profile) => Some(profile),
                Detected::Ambiguous(ids) => {
                    self.issues.push(LibraryIssue::ProfileAmbiguous {
                        path: root.to_owned(),
                        profiles: ids,
                    });
                    None
                }
            }
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
        let name = manifest
            .as_ref()
            .and_then(|m| m.name.clone())
            .or_else(|| profile.map(|profile| profile.name.clone()))
            .unwrap_or_else(|| id.to_string());
        self.libraries.push(Library {
            id: id.clone(),
            name,
            root: root.to_owned(),
            license: manifest.as_ref().and_then(|m| m.license.clone()),
            has_manifest: manifest.is_some(),
        });
        self.add_presets(root, &id, manifest.as_ref());
    }

    fn add_presets(&mut self, root: &Path, library: &LibraryId, manifest: Option<&Manifest>) {
        let license = self
            .libraries
            .iter()
            .find(|l| l.id == *library)
            .and_then(|l| l.license.clone());
        if let Some(manifest) = manifest {
            for (entry, reason) in &manifest.rejected {
                self.issues.push(LibraryIssue::ManifestEntryRejected {
                    path: root.join(manifest::MANIFEST_FILE),
                    entry: entry.clone(),
                    reason: reason.clone(),
                });
            }
        }
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
        for entry in manifest.map_or(&[][..], |m| &m.entries[..]) {
            let relative = found
                .remove(&entry.relative_path.to_lowercase())
                .unwrap_or_else(|| entry.relative_path.clone());
            let patch_path = root.join(&relative);
            presets.push(InstrumentPreset {
                id: InstrumentId::new(library, &relative),
                display_name: entry.name.clone().unwrap_or_else(|| file_stem(&relative)),
                family: entry
                    .family
                    .map_or_else(|| infer_family(&relative), FamilyClaim::Declared),
                library: library.clone(),
                availability: check_file(&patch_path, canonical_root.as_deref()),
                patch_path,
                articulations: entry.articulations.clone(),
                license: license.clone(),
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
