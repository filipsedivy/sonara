//! Discovery: merges library manifests with a scan of the library directories.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sonara_midi::orchestration::InstrumentType;

use super::family::{FamilyClaim, InstrumentFamily, infer_family, infer_instrument};
use super::manifest::{
    self, ArticulationList, Claimant, EntryTarget, Field, LibraryClaims, ManifestEntry,
};
use super::model::{
    Availability, CatalogError, Claim, ClaimSource, InstrumentClaim, InstrumentId,
    InstrumentPreset, Library, LibraryId, LibraryIssue,
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

    /// The presets whose family is `family`, whoever claims it.
    pub fn in_family(&self, family: InstrumentFamily) -> impl Iterator<Item = &InstrumentPreset> {
        self.presets
            .iter()
            .filter(move |preset| preset.family.family() == Some(family))
    }

    /// The presets that play `instrument`, whoever claims it ([`InstrumentPreset::instrument`]).
    pub fn playing(&self, instrument: InstrumentType) -> impl Iterator<Item = &InstrumentPreset> {
        self.presets.iter().filter(move |preset| {
            preset
                .instrument
                .is_some_and(|claim| claim.instrument == instrument)
        })
    }

    /// Loads the preset's patch through `loader` and records the verdict: a patch that does not
    /// load becomes [`Availability::Invalid`]. It also records what the loader's structural check
    /// finds in the patch ([`PatchLoader::inspect`]) as the preset's
    /// [`facts`](InstrumentPreset::facts), loaded or not, which may lower its
    /// [`effective verdict`](InstrumentPreset::effective_verdict). A preset that is not
    /// available is neither loaded nor inspected and keeps its state. Returns the availability
    /// after the check.
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
        if preset.availability.is_available() {
            if let Err(err) = loader.load(&preset.patch_path) {
                preset.availability = Availability::Invalid {
                    reason: err.to_string(),
                };
            }
            if let Some(facts) = loader.inspect(&preset.patch_path) {
                preset.facts = Some(facts);
            }
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
    /// the scan finds. Each claim keeps who makes it: a claim of the shipped profile is not the
    /// library's word, so it is shown as the profile's, and `articulations` and `license` hold
    /// only what the library's own manifest declares. A patch the claims name that is not on
    /// disk is listed as missing. A multi-patch set is not a patch: its members are listed as
    /// the patches they are.
    fn add_presets(&mut self, root: &Path, library: &LibraryId, claims: &LibraryClaims) {
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
            presets.push(preset(
                library,
                root,
                canonical_root.as_deref(),
                &relative,
                entry,
                &claims.license,
            ));
        }
        for relative in found.into_values() {
            // A patch nothing claims anything about: its name and family come from its path.
            let unlisted = ManifestEntry::new(EntryTarget::Path(relative.clone()));
            presets.push(preset(
                library,
                root,
                canonical_root.as_deref(),
                &relative,
                &unlisted,
                &claims.license,
            ));
        }
        self.presets.extend(presets);
    }
}

/// The preset of the patch at `relative` (`/`-separated, spelled as on disk when it is there)
/// in the library `library` at `root`, whose real path is `canonical_root`, with what `entry`
/// claims about it; `license` is the library's licence. No structural check has run yet, so
/// the preset holds no facts.
fn preset(
    library: &LibraryId,
    root: &Path,
    canonical_root: Option<&Path>,
    relative: &str,
    entry: &ManifestEntry,
    license: &Field<String>,
) -> InstrumentPreset {
    let patch_path = root.join(relative);
    let family = family_claim(entry, relative);
    InstrumentPreset {
        id: InstrumentId::new(library, relative),
        display_name: entry
            .name
            .value()
            .cloned()
            .unwrap_or_else(|| file_stem(relative)),
        family,
        library: library.clone(),
        availability: check_file(&patch_path, canonical_root),
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
            .or_else(|| license.declared())
            .cloned(),
        instrument: instrument_claim(entry, relative, family),
        ensemble: claim(&entry.ensemble),
        articulation_map: claim(&entry.articulations),
        keymap: claim(&entry.keymap),
        key_offset: claim(&entry.key_offset),
        dynamics: claim(&entry.dynamics),
        legato: claim(&entry.legato),
        in_situ: claim(&entry.in_situ),
        license_claim: claim(&entry.license).or_else(|| claim(license)),
        verdict: claim(&entry.verdict),
        facts: None,
    }
}

/// The value of `field` with who claims it; `None` when it is left out or cleared.
fn claim<T: Clone>(field: &Field<T>) -> Option<Claim<T>> {
    field.claimed().map(|(value, claimant)| Claim {
        value: value.clone(),
        source: claimant.source(),
    })
}

/// The family of a patch: what the library's manifest states, else what the shipped profile
/// states, else the guess from `relative`, its path. A source states the family itself or the
/// instrument type the patch plays, whose family it is; where one source states both, the
/// family it writes wins, as a written field is the user's word over a derived one.
fn family_claim(entry: &ManifestEntry, relative: &str) -> FamilyClaim {
    let stated_by = |claimant: Claimant| match (entry.family.claimed(), entry.instrument.claimed())
    {
        (Some((family, by)), _) if by == claimant => Some(*family),
        (_, Some((instrument, by))) if by == claimant => Some(instrument.family()),
        _ => None,
    };
    stated_by(Claimant::Manifest)
        .map(FamilyClaim::Declared)
        .or_else(|| stated_by(Claimant::Profile).map(FamilyClaim::Profile))
        .unwrap_or_else(|| infer_family(relative))
}

/// The instrument type of a patch: what the manifest or the shipped profile states, with its
/// source; else a guess from `relative`, its path, kept only when the guess is of `family`, the
/// patch's family, so a preset never claims a family and an instrument of another. A source
/// that writes `instrument: null`, or gives the patch a key map, says that it plays several
/// instruments, so nothing is guessed.
fn instrument_claim(
    entry: &ManifestEntry,
    relative: &str,
    family: FamilyClaim,
) -> Option<InstrumentClaim> {
    match &entry.instrument {
        Field::Set(instrument, claimant) => Some(InstrumentClaim {
            instrument: *instrument,
            source: claimant.source(),
        }),
        Field::Cleared => None,
        Field::Unset if entry.keymap.value().is_some() => None,
        Field::Unset => infer_instrument(relative)
            .filter(|instrument| family.family() == Some(instrument.family()))
            .map(|instrument| InstrumentClaim {
                instrument,
                source: ClaimSource::Inferred,
            }),
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
    use sonara_midi::orchestration::InstrumentType;

    use super::InstrumentCatalog;
    use crate::instruments::profiles::{self, Profile};
    use crate::instruments::{
        ArticulationList, Availability, Claim, ClaimSource, DynamicsMode, EnsembleClaim,
        EnsembleKind, FamilyClaim, InstrumentClaim, InstrumentFamily, InstrumentPreset, LegatoMode,
        LibraryIssue, MANIFEST_FILE, Verdict, VerdictState,
    };

    fn patch(path: &Path) {
        fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
        fs::write(path, "<region> sample=a.wav\n").expect("write");
    }

    /// A profile of the library `lib`, recognized by `Detect.sfz`: a violin entry with a claim
    /// of every kind, an entry that writes a family and an instrument of another, a percussion
    /// patch with a key map, a patch that is not on disk and a set.
    fn profile() -> Profile {
        profiles::parse(
            &json!({
                "version": 2, "id": "lib", "name": "Profiled Library", "license": "CC0 1.0",
                "detect": ["Detect.sfz"],
                "instruments": [
                    {"path": "Strings/Violin.sfz", "name": "Profile Violin",
                     "instrument": "violin", "ensemble": {"kind": "section", "players": 16},
                     "license": "Profile licence",
                     "articulations": [{"id": "sustain", "via": "keyswitch", "key": 36,
                                        "default": true}],
                     "key_offset": -12, "dynamics": {"via": "cc", "cc": 1},
                     "legato": {"kind": "trigger"}, "in_situ": true,
                     "verdict": {"state": "partly", "lost": ["rt_decay"]}},
                    {"path": "Odd.sfz", "family": "other", "instrument": "violin"},
                    {"path": "Snare Drum Kit.sfz", "family": "percussion",
                     "keymap": [{"keys": [38], "instrument": "snare_drum"}]},
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

    /// The library `lib` at `root` with the patches of [`profile`] on disk, apart from `Gone`.
    fn profiled_library(root: &Path) {
        for path in [
            "Detect.sfz",
            "Strings/Violin.sfz",
            "Odd.sfz",
            "Snare Drum Kit.sfz",
        ] {
            patch(&root.join(path));
        }
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

    const fn by_profile<T>(value: T) -> Claim<T> {
        Claim {
            value,
            source: ClaimSource::Profile,
        }
    }

    const fn instrument(instrument: InstrumentType, source: ClaimSource) -> InstrumentClaim {
        InstrumentClaim { instrument, source }
    }

    /// Without a manifest, everything the profile says about a patch is shown as the
    /// profile's, never as the library's: `articulations` and `license`, which hold what the
    /// library declares, stay empty (provenance; TST-05).
    #[test]
    fn a_detected_library_claims_with_the_profile_source() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("Some Folder");
        profiled_library(&root);

        let catalog = InstrumentCatalog::discover_with(std::slice::from_ref(&root), &[profile()]);

        let library = &catalog.libraries()[0];
        assert_eq!(library.id.as_str(), "lib");
        assert_eq!(library.name, "Profiled Library");
        assert_eq!(library.license, None);
        assert!(!library.has_manifest);
        // A set is not a patch; its member is listed as the patch it is.
        assert_eq!(
            ids(&catalog),
            [
                "lib/Detect",
                "lib/Gone",
                "lib/Odd",
                "lib/Snare Drum Kit",
                "lib/Strings/Violin"
            ]
        );
        let violin = preset(&catalog, "lib/Strings/Violin");
        assert_eq!(violin.display_name, "Profile Violin");
        // The profile states the instrument, so the family is the instrument's, as its claim.
        assert_eq!(
            violin.family,
            FamilyClaim::Profile(InstrumentFamily::Strings)
        );
        assert_eq!(
            violin.instrument,
            Some(instrument(InstrumentType::Violin, ClaimSource::Profile))
        );
        assert_eq!(
            violin.ensemble,
            Some(by_profile(EnsembleClaim {
                kind: EnsembleKind::Section,
                players: Some(16)
            }))
        );
        let map = violin.articulation_map.as_ref().expect("mapped");
        assert_eq!(map.source, ClaimSource::Profile);
        assert!(matches!(&map.value, ArticulationList::Mapped(list) if list.len() == 1));
        assert_eq!(map.value.ids(), ["sustain"]);
        assert_eq!(violin.key_offset, Some(by_profile(-12)));
        assert_eq!(violin.dynamics, Some(by_profile(DynamicsMode::Cc(1))));
        assert_eq!(violin.legato, Some(by_profile(LegatoMode::Trigger)));
        assert_eq!(violin.in_situ, Some(by_profile(true)));
        assert_eq!(violin.keymap, None);
        assert_eq!(
            violin.license_claim,
            Some(by_profile("Profile licence".to_owned()))
        );
        let verdict = Verdict {
            state: VerdictState::Partly,
            lost: vec!["rt_decay".to_owned()],
        };
        assert_eq!(violin.verdict, Some(by_profile(verdict.clone())));
        // No structural check ran, so the claimed verdict stands.
        assert_eq!(violin.facts, None);
        assert_eq!(violin.effective_verdict(), Some(verdict));
        // What the library declares stays empty.
        assert_eq!(violin.articulations, Vec::<String>::new());
        assert_eq!(violin.license, None);
        assert_eq!(violin.availability, Availability::Available);

        // Where the profile writes a family and an instrument, the written family wins.
        let odd = preset(&catalog, "lib/Odd");
        assert_eq!(odd.family, FamilyClaim::Profile(InstrumentFamily::Other));
        assert_eq!(
            odd.instrument,
            Some(instrument(InstrumentType::Violin, ClaimSource::Profile))
        );
        // A key map says the patch plays several instruments: none is guessed from the path.
        let kit = preset(&catalog, "lib/Snare Drum Kit");
        assert_eq!(
            kit.family,
            FamilyClaim::Profile(InstrumentFamily::Percussion)
        );
        assert_eq!(kit.instrument, None);
        assert!(kit.keymap.as_ref().is_some_and(|map| map.value.len() == 1));
        // A patch the profile does not name claims nothing but the library's licence.
        let detect = preset(&catalog, "lib/Detect");
        assert_eq!(detect.family, FamilyClaim::Unknown);
        assert_eq!(
            (detect.instrument, &detect.ensemble, &detect.verdict),
            (None, &None, &None)
        );
        assert_eq!(detect.license_claim, Some(by_profile("CC0 1.0".to_owned())));
        // A patch the profile names that is not on disk is listed as missing, by its name.
        let gone = preset(&catalog, "lib/Gone");
        assert_eq!(gone.availability, Availability::Missing);
        assert_eq!(gone.display_name, "Gone Patch");
        let violins: Vec<&str> = catalog
            .playing(InstrumentType::Violin)
            .map(|preset| preset.id.as_str())
            .collect();
        assert_eq!(violins, ["lib/Odd", "lib/Strings/Violin"]);
        assert_eq!(catalog.playing(InstrumentType::Flute).count(), 0);
        assert_eq!(catalog.issues(), []);
    }

    /// The library's own manifest declares over the profile, field by field; what it leaves
    /// out keeps the profile's value and source, and a manifest that declares the profile's ID
    /// takes the profile without its detect files.
    #[test]
    fn a_manifest_declares_over_the_profile() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("Renamed");
        for path in ["Strings/Violin.sfz", "Odd.sfz", "Snare Drum Kit.sfz"] {
            patch(&root.join(path));
        }
        fs::write(
            root.join(MANIFEST_FILE),
            json!({
                "version": 2, "id": "lib", "license": "CC0 1.0",
                "instruments": [
                    {"path": "strings/violin.sfz", "family": "strings",
                     "verdict": {"state": "as_is", "lost": []},
                     "articulations": [{"id": "pizzicato", "via": "patch"},
                                       {"id": "ordinario", "via": "patch", "default": true}]},
                    {"path": "Odd.sfz", "instrument": "flute"},
                    {"path": "Snare Drum Kit.sfz", "keymap": null},
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
        let map = violin.articulation_map.as_ref().expect("mapped");
        assert_eq!(map.source, ClaimSource::Declared);
        assert_eq!(map.value.ids(), ["pizzicato", "ordinario"]);
        assert_eq!(
            violin
                .verdict
                .as_ref()
                .map(|claim| (claim.value.state, claim.source)),
            Some((VerdictState::AsIs, ClaimSource::Declared))
        );
        // What the manifest leaves out keeps the profile's word, shown as the profile's.
        assert_eq!(
            violin.instrument,
            Some(instrument(InstrumentType::Violin, ClaimSource::Profile))
        );
        assert_eq!(violin.key_offset, Some(by_profile(-12)));
        // The profile's licence for the recording is not declared; the library's is. The
        // licence claim is the recording's, which the manifest leaves to the profile.
        assert_eq!(violin.license.as_deref(), Some("CC0 1.0"));
        assert_eq!(
            violin.license_claim,
            Some(by_profile("Profile licence".to_owned()))
        );
        // The library's own word beats the profile's: the declared instrument's family over
        // the family the profile writes.
        let odd = preset(&catalog, "lib/Odd");
        assert_eq!(
            odd.family,
            FamilyClaim::Declared(InstrumentFamily::Woodwinds)
        );
        assert_eq!(
            odd.instrument,
            Some(instrument(InstrumentType::Flute, ClaimSource::Declared))
        );
        // With the key map cleared, the instrument is guessed from the path, within the
        // profile's family.
        let kit = preset(&catalog, "lib/Snare Drum Kit");
        assert_eq!(kit.keymap, None);
        assert_eq!(
            kit.instrument,
            Some(instrument(InstrumentType::SnareDrum, ClaimSource::Inferred))
        );
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
        assert_eq!(
            ids(&catalog),
            [
                "lib/Gone",
                "lib/Odd",
                "lib/Snare Drum Kit",
                "lib/Strings/Violin"
            ]
        );
    }

    /// An instrument is guessed from the path only where no manifest or profile says what the
    /// patch plays, and only within the preset's family; `instrument: null` says the patch plays
    /// several, so nothing is guessed.
    #[test]
    fn an_instrument_is_inferred_only_where_nothing_states_one() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("lib");
        for path in ["Viola.sfz", "Violin.sfz", "Violin Pair.sfz", "Cello.sfz"] {
            patch(&root.join(path));
        }
        fs::write(
            root.join(MANIFEST_FILE),
            json!({
                "version": 2,
                "instruments": [
                    {"path": "Violin.sfz", "family": "other"},
                    {"path": "Violin Pair.sfz", "instrument": null},
                    {"path": "Cello.sfz", "family": "strings"}
                ]
            })
            .to_string(),
        )
        .expect("write manifest");

        let catalog = InstrumentCatalog::discover_with(std::slice::from_ref(&root), &[]);

        let claim = |id: &str| {
            let preset = preset(&catalog, id);
            (preset.family, preset.instrument)
        };
        assert_eq!(
            claim("lib/Viola"),
            (
                FamilyClaim::Inferred(InstrumentFamily::Strings),
                Some(instrument(InstrumentType::Viola, ClaimSource::Inferred))
            )
        );
        // The declared family is not the violin's, so the guess is dropped.
        assert_eq!(
            claim("lib/Violin"),
            (FamilyClaim::Declared(InstrumentFamily::Other), None)
        );
        assert_eq!(
            claim("lib/Violin Pair"),
            (FamilyClaim::Inferred(InstrumentFamily::Strings), None)
        );
        // A declared family that agrees keeps the guess, which stays a guess.
        assert_eq!(
            claim("lib/Cello"),
            (
                FamilyClaim::Declared(InstrumentFamily::Strings),
                Some(instrument(InstrumentType::Cello, ClaimSource::Inferred))
            )
        );
        assert_eq!(catalog.issues(), []);
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
