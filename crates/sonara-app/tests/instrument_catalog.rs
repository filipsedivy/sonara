//! The instrument catalog against generated library directories (TST-02, TST-03). No audio
//! engine and no real samples: patches are small text files, and deep validation goes through a
//! fake [`PatchLoader`].

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;
use sonara_app::instruments::{
    Availability, CatalogError, FamilyClaim, InstrumentCatalog, InstrumentFamily, LibraryId,
    LibraryIssue, MANIFEST_FILE, MediaPatchLoader, PatchLoadError, PatchLoader,
};
use sonara_app::settings::Settings;

fn patch(path: &Path) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
    fs::write(path, "<region> sample=a.wav\n").expect("write");
}

fn manifest(root: &Path, body: &serde_json::Value) {
    fs::write(root.join(MANIFEST_FILE), body.to_string()).expect("write manifest");
}

fn preset_ids(catalog: &InstrumentCatalog) -> Vec<&str> {
    catalog.presets().iter().map(|p| p.id.as_str()).collect()
}

#[test]
fn a_scan_without_a_manifest_infers_and_never_declares() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("My Library");
    patch(&root.join("Strings/Violin Solo.sfz"));
    patch(&root.join("Brass/Horn.sfz"));
    patch(&root.join("Misc/Pad 7.sfz"));
    patch(&root.join("Misc/notes.txt"));
    patch(&root.join(".hidden/Harp.sfz"));

    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));

    assert_eq!(
        preset_ids(&catalog),
        [
            "my-library/Brass/Horn",
            "my-library/Misc/Pad 7",
            "my-library/Strings/Violin Solo"
        ]
    );
    let library = &catalog.libraries()[0];
    assert_eq!(library.id.as_str(), "my-library");
    assert!(!library.has_manifest);
    assert_eq!(library.license, None);

    let violin = &catalog.presets()[2];
    assert_eq!(violin.display_name, "Violin Solo");
    assert_eq!(
        violin.family,
        FamilyClaim::Inferred(InstrumentFamily::Strings)
    );
    assert_eq!(violin.articulations, Vec::<String>::new());
    assert_eq!(violin.license, None);
    assert_eq!(violin.availability, Availability::Available);
    assert_eq!(violin.patch_path, root.join("Strings/Violin Solo.sfz"));
    assert_eq!(catalog.presets()[1].family, FamilyClaim::Unknown);
    assert_eq!(catalog.in_family(InstrumentFamily::Brass).count(), 1);
    assert_eq!(catalog.available().count(), 3);
    assert_eq!(catalog.issues(), []);
}

#[test]
fn a_manifest_declares_metadata_and_reports_missing_patches() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("vsco");
    patch(&root.join("Strings/Violin.sfz"));
    patch(&root.join("Percussion/Timpani.sfz"));
    manifest(
        &root,
        &json!({
            "version": 1, "id": "vsco2ce", "name": "VSCO 2 CE", "license": "CC0 1.0",
            "instruments": [
                {"path": "strings/violin.sfz", "name": "Solo Violin", "family": "strings",
                 "articulations": ["sustain", "pizzicato"]},
                {"path": "Winds/Flute.sfz", "family": "woodwinds"},
                {"path": "../escape.sfz"}
            ]
        }),
    );

    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));

    // The manifest's lower-case spelling matches the file on disk; the disk's spelling wins.
    assert_eq!(
        preset_ids(&catalog),
        [
            "vsco2ce/Percussion/Timpani",
            "vsco2ce/Strings/Violin",
            "vsco2ce/Winds/Flute"
        ]
    );
    let by_name = |name: &str| {
        catalog
            .presets()
            .iter()
            .find(|p| p.display_name == name)
            .unwrap_or_else(|| panic!("{name}"))
    };
    let violin = by_name("Solo Violin");
    assert_eq!(violin.id.as_str(), "vsco2ce/Strings/Violin");
    assert_eq!(
        violin.family,
        FamilyClaim::Declared(InstrumentFamily::Strings)
    );
    assert_eq!(violin.articulations, ["sustain", "pizzicato"]);
    assert_eq!(violin.license.as_deref(), Some("CC0 1.0"));
    assert_eq!(violin.availability, Availability::Available);

    let flute = by_name("Flute");
    assert_eq!(flute.availability, Availability::Missing);
    assert!(!flute.availability.is_available());

    // Not in the manifest: found by the scan, inferred, no articulations.
    let timpani = by_name("Timpani");
    assert_eq!(
        timpani.family,
        FamilyClaim::Inferred(InstrumentFamily::Percussion)
    );
    assert_eq!(timpani.articulations.len(), 0);

    let library = &catalog.libraries()[0];
    assert_eq!(library.name, "VSCO 2 CE");
    assert!(library.has_manifest);
    assert_eq!(catalog.available().count(), 2);
    assert!(matches!(
        catalog.issues(),
        [LibraryIssue::ManifestEntryRejected { entry, .. }] if entry == "../escape.sfz"
    ));
}

#[test]
fn bad_roots_and_manifests_are_reported_and_do_not_stop_the_rest() {
    let dir = tempfile::tempdir().expect("tempdir");
    let good = dir.path().join("good");
    patch(&good.join("Cello.sfz"));
    let file_root = dir.path().join("a-file");
    fs::write(&file_root, "x").expect("write");
    let broken = dir.path().join("broken");
    patch(&broken.join("Flute.sfz"));
    fs::write(broken.join(MANIFEST_FILE), "{ not json").expect("write");
    let missing = dir.path().join("missing");

    let catalog = InstrumentCatalog::discover(&[
        missing.clone(),
        file_root.clone(),
        broken.clone(),
        good.clone(),
    ]);

    assert_eq!(preset_ids(&catalog), ["broken/Flute", "good/Cello"]);
    assert!(
        catalog
            .issues()
            .contains(&LibraryIssue::RootMissing { path: missing })
    );
    assert!(
        catalog
            .issues()
            .contains(&LibraryIssue::RootNotDirectory { path: file_root })
    );
    assert!(matches!(
        catalog
            .issues()
            .iter()
            .find(|i| matches!(i, LibraryIssue::ManifestInvalid { .. })),
        Some(LibraryIssue::ManifestInvalid { path, .. }) if path == &broken.join(MANIFEST_FILE)
    ));
    // The broken manifest's library is still scanned, without a manifest.
    assert!(!catalog.libraries()[0].has_manifest);
}

#[test]
fn an_empty_or_unconfigured_catalog_is_empty() {
    assert_eq!(
        InstrumentCatalog::discover(&[]),
        InstrumentCatalog::default()
    );
    let catalog = InstrumentCatalog::from_settings(&Settings::default());
    assert!(catalog.presets().is_empty() && catalog.issues().is_empty());
}

#[test]
fn settings_name_the_library_directories() {
    let dir = tempfile::tempdir().expect("tempdir");
    patch(&dir.path().join("Viola.sfz"));
    let mut settings = Settings::default();
    settings.extra.insert(
        "sample_libraries".to_owned(),
        json!([dir.path().to_str().expect("UTF-8")]),
    );
    let catalog = InstrumentCatalog::from_settings(&settings);
    assert_eq!(catalog.presets().len(), 1);
    assert_eq!(
        catalog.presets()[0].family,
        FamilyClaim::Inferred(InstrumentFamily::Strings)
    );
}

#[test]
fn two_libraries_with_one_id_keep_the_first() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (a, b) = (dir.path().join("one/lib"), dir.path().join("two/lib"));
    patch(&a.join("Oboe.sfz"));
    patch(&b.join("Flute.sfz"));
    let catalog = InstrumentCatalog::discover(&[a, b.clone()]);
    assert_eq!(preset_ids(&catalog), ["lib/Oboe"]);
    assert_eq!(catalog.libraries().len(), 1);
    assert!(matches!(
        catalog.issues(),
        [LibraryIssue::DuplicateLibrary { id, path }] if id.as_str() == "lib" && path == &b
    ));
}

#[test]
fn ids_do_not_depend_on_where_the_library_is_installed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (a, b) = (dir.path().join("x/Lib"), dir.path().join("y/z/Lib"));
    patch(&a.join("Strings/Violin.sfz"));
    patch(&b.join("Strings/Violin.sfz"));
    let first = InstrumentCatalog::discover(&[a]);
    let second = InstrumentCatalog::discover(&[b]);
    assert_eq!(first.presets()[0].id, second.presets()[0].id);
    assert_ne!(
        first.presets()[0].patch_path,
        second.presets()[0].patch_path
    );
}

#[test]
fn unusable_patch_files_are_invalid_not_a_crash() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("lib");
    fs::create_dir_all(&root).expect("mkdir");
    fs::write(root.join("Empty.sfz"), "").expect("write");
    // A directory that is named like a patch is not scanned as one (scan sees files only), so
    // reach the "not a file" path through a manifest entry.
    fs::create_dir_all(root.join("Folder.sfz")).expect("mkdir");
    manifest(
        &root,
        &json!({"version": 1, "instruments": [{"path": "Folder.sfz"}]}),
    );
    let catalog = InstrumentCatalog::discover(&[root]);
    let state = |name: &str| {
        catalog
            .presets()
            .iter()
            .find(|p| p.display_name == name)
            .map(|p| p.availability.clone())
    };
    assert_eq!(
        state("Empty"),
        Some(Availability::Invalid {
            reason: "the file is empty".to_owned()
        })
    );
    assert_eq!(
        state("Folder"),
        Some(Availability::Invalid {
            reason: "it is not a file".to_owned()
        })
    );
}

#[cfg(unix)]
#[test]
fn symbolic_links_are_never_followed() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("lib");
    let outside = dir.path().join("outside");
    patch(&root.join("Real.sfz"));
    patch(&outside.join("Secret.sfz"));
    // A loop, a linked folder and a linked file: the scan skips all of them.
    symlink(&root, root.join("loop")).expect("symlink");
    symlink(&outside, root.join("linked-dir")).expect("symlink");
    symlink(outside.join("Secret.sfz"), root.join("Linked.sfz")).expect("symlink");
    // A manifest entry that points at a link must not escape the library either.
    manifest(
        &root,
        &json!({"version": 1, "instruments": [
            {"path": "Linked.sfz"}, {"path": "linked-dir/Secret.sfz"}]}),
    );

    let catalog = InstrumentCatalog::discover(&[root]);

    let state = |name: &str| {
        catalog
            .presets()
            .iter()
            .find(|p| p.display_name == name)
            .map(|p| p.availability.clone())
    };
    assert_eq!(state("Real"), Some(Availability::Available));
    assert!(matches!(
        state("Linked"),
        Some(Availability::Invalid { .. })
    ));
    assert!(matches!(
        state("Secret"),
        Some(Availability::Invalid { .. })
    ));
    assert_eq!(catalog.presets().len(), 3, "Real, Linked and Secret only");
}

#[cfg(unix)]
#[test]
fn an_unreadable_patch_is_invalid() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("lib");
    patch(&root.join("Locked.sfz"));
    let file = root.join("Locked.sfz");
    fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).expect("chmod");
    let catalog = InstrumentCatalog::discover(&[root]);
    // Root can open anything; skip the assertion there rather than fake a result.
    if fs::File::open(&file).is_err() {
        assert!(matches!(
            catalog.presets()[0].availability,
            Availability::Invalid { ref reason } if reason.starts_with("cannot be opened")
        ));
    }
    fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).expect("chmod back");
}

/// A loader that accepts every patch except the ones named, and records what it was asked.
struct FakeLoader {
    reject: &'static str,
    asked: RefCell<Vec<PathBuf>>,
}

impl PatchLoader for FakeLoader {
    fn load(&self, patch: &Path) -> Result<(), PatchLoadError> {
        self.asked.borrow_mut().push(patch.to_owned());
        if patch.ends_with(self.reject) {
            Err(PatchLoadError("sample Violin_C4.wav is missing".to_owned()))
        } else {
            Ok(())
        }
    }
}

#[test]
fn deep_validation_marks_a_broken_patch_invalid_and_skips_missing_ones() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("lib");
    patch(&root.join("Violin.sfz"));
    patch(&root.join("Cello.sfz"));
    manifest(
        &root,
        &json!({"version": 1, "instruments": [{"path": "Gone.sfz"}]}),
    );
    let mut catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    let loader = FakeLoader {
        reject: "Violin.sfz",
        asked: RefCell::default(),
    };
    let ids: Vec<_> = catalog.presets().iter().map(|p| p.id.clone()).collect();

    let verdicts: Vec<_> = ids
        .iter()
        .map(|id| catalog.verify(id, &loader).expect("known id").clone())
        .collect();

    assert_eq!(
        verdicts,
        [
            Availability::Available,
            Availability::Missing,
            Availability::Invalid {
                reason: "sample Violin_C4.wav is missing".to_owned()
            },
        ]
    );
    assert_eq!(
        *loader.asked.borrow(),
        [root.join("Cello.sfz"), root.join("Violin.sfz")],
        "a missing patch is not loaded"
    );
    let violin = catalog.get(&ids[2]).expect("present");
    assert!(!violin.availability.is_available());
    assert_eq!(catalog.available().count(), 1);
}

#[test]
fn verifying_an_unknown_preset_is_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    patch(&dir.path().join("lib/Violin.sfz"));
    let mut catalog = InstrumentCatalog::discover(&[dir.path().join("lib")]);
    let other = tempfile::tempdir().expect("tempdir");
    patch(&other.path().join("zzz/Tuba.sfz"));
    let stranger = InstrumentCatalog::discover(&[other.path().join("zzz")]).presets()[0]
        .id
        .clone();
    let loader = FakeLoader {
        reject: "",
        asked: RefCell::default(),
    };
    assert_eq!(
        catalog.verify(&stranger, &loader).expect_err("unknown"),
        CatalogError::UnknownInstrument("zzz/Tuba".to_owned())
    );
    assert!(loader.asked.borrow().is_empty());
}

#[test]
fn the_media_loader_accepts_a_playable_patch_and_rejects_a_missing_sample() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("lib");
    fs::create_dir_all(&root).expect("mkdir");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut wav = hound::WavWriter::create(root.join("tone.wav"), spec).expect("wav");
    for i in 0..64_i16 {
        wav.write_sample(i * 100).expect("sample");
    }
    wav.finalize().expect("finalize");
    fs::write(root.join("Good.sfz"), "<region> sample=tone.wav key=60\n").expect("write");
    fs::write(root.join("Bad.sfz"), "<region> sample=nope.wav key=60\n").expect("write");

    let mut catalog = InstrumentCatalog::discover(&[root]);
    let ids: Vec<_> = catalog.presets().iter().map(|p| p.id.clone()).collect();
    assert_eq!(ids.len(), 2);
    let (bad, good) = (&ids[0], &ids[1]);

    assert_eq!(
        catalog.verify(good, &MediaPatchLoader).expect("known"),
        &Availability::Available
    );
    let verdict = catalog
        .verify(bad, &MediaPatchLoader)
        .expect("known")
        .clone();
    assert!(
        matches!(&verdict, Availability::Invalid { reason } if reason.contains("nope.wav")),
        "{verdict:?}"
    );
}

#[test]
fn the_media_loader_refuses_a_patch_that_plays_without_a_missing_sample() {
    // The sampler plays such a patch without the regions of the missing sample; the deep check
    // stays strict and names the file.
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("lib");
    fs::create_dir_all(&root).expect("mkdir");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut wav = hound::WavWriter::create(root.join("tone.wav"), spec).expect("wav");
    for i in 0..64_i16 {
        wav.write_sample(i * 100).expect("sample");
    }
    wav.finalize().expect("finalize");
    fs::write(
        root.join("Partial.sfz"),
        "<region> sample=tone.wav key=60\n<region> sample=nope.wav key=61\n",
    )
    .expect("write");
    fs::write(
        root.join("Silent.sfz"),
        "<region> sample=tone.wav key=60\n<region> sample=*silence key=61 group=1\n",
    )
    .expect("write");

    let mut catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    let ids: Vec<_> = catalog.presets().iter().map(|p| p.id.clone()).collect();
    assert_eq!(ids.len(), 2);
    let (partial, silent) = (&ids[0], &ids[1]);
    let verdict = catalog
        .verify(partial, &MediaPatchLoader)
        .expect("known")
        .clone();
    assert_eq!(
        verdict,
        Availability::Invalid {
            reason: format!(
                "the patch leaves out regions it cannot load: sample {} is missing \
                 (1 region left out)",
                root.join("nope.wav").display()
            )
        }
    );
    assert_eq!(
        catalog.verify(silent, &MediaPatchLoader).expect("known"),
        &Availability::Available,
        "a silent region is part of the patch"
    );
}

#[test]
fn include_fragments_are_listed_only_when_a_manifest_names_them() {
    // TST-06: Sonatina keeps the pieces its patches include in `includes` folders. The scan
    // leaves them out; a manifest that names one explicitly still lists it.
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("sso");
    patch(&root.join("Strings/1st Violins.sfz"));
    patch(&root.join("Strings/includes/1st-violins-sustain.sfz"));
    patch(&root.join("Strings/Includes/vibrato.sfz"));
    manifest(
        &root,
        &json!({"version": 1, "instruments": [
            {"path": "Strings/includes/1st-violins-sustain.sfz", "name": "Violins sustain"}]}),
    );

    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));

    assert_eq!(
        preset_ids(&catalog),
        [
            "sso/Strings/1st Violins",
            "sso/Strings/includes/1st-violins-sustain"
        ]
    );
    let listed = &catalog.presets()[1];
    assert_eq!(listed.display_name, "Violins sustain");
    assert_eq!(listed.availability, Availability::Available);
    assert_eq!(
        listed.patch_path,
        root.join("Strings/includes/1st-violins-sustain.sfz")
    );
    assert_eq!(catalog.issues(), []);
}

#[test]
fn a_patch_without_regions_is_invalid_once_verified() {
    // Discovery reads directory entries only, so a file with no `<region>` (a fragment outside
    // an `includes` folder) is listed as available; loading it marks it invalid.
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("lib");
    fs::create_dir_all(&root).expect("mkdir");
    fs::write(
        root.join("Vibrato.sfz"),
        "<global> sw_last=36\n<group> lovel=1 hivel=127 // no region follows\n",
    )
    .expect("write");

    let mut catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    let id = catalog.presets()[0].id.clone();
    assert_eq!(id.as_str(), "lib/Vibrato");
    assert_eq!(catalog.presets()[0].availability, Availability::Available);

    let verdict = catalog
        .verify(&id, &MediaPatchLoader)
        .expect("known")
        .clone();

    assert_eq!(
        verdict,
        Availability::Invalid {
            reason: format!(
                "{}: the patch has no playable region",
                root.join("Vibrato.sfz").display()
            )
        }
    );
    assert_eq!(catalog.available().count(), 0);
}

#[test]
fn the_media_loader_survives_hostile_patches() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("lib");
    fs::create_dir_all(&root).expect("mkdir");
    fs::write(
        root.join("Garbage.sfz"),
        [0xff_u8, 0xfe, 0x00, 0x9c, 0x80, 0x00, 0x01],
    )
    .expect("write");
    fs::write(root.join("NoRegion.sfz"), "<control>\ndefault_path=x/\n").expect("write");
    fs::write(root.join("Loop.sfz"), "#include \"Loop.sfz\"\n").expect("write");
    fs::write(root.join("BadWav.sfz"), "<region> sample=bad.wav key=60\n").expect("write");
    fs::write(root.join("bad.wav"), b"RIFF not really a wave file").expect("write");

    let mut catalog = InstrumentCatalog::discover(&[root]);
    let ids: Vec<_> = catalog.presets().iter().map(|p| p.id.clone()).collect();
    assert_eq!(ids.len(), 4);
    for id in &ids {
        let verdict = catalog
            .verify(id, &MediaPatchLoader)
            .expect("known")
            .clone();
        assert!(
            matches!(&verdict, Availability::Invalid { reason } if !reason.is_empty()),
            "{id}: {verdict:?}"
        );
    }
}

/// The files that identify a VSCO 2 CE checkout (branch `SFZ`), as Sonara's profile lists them.
const VSCO_DETECT: [&str; 3] = ["ViolinEns-KS.sfz", "Tuba-KS.sfz", "GM-StylePerc.sfz"];
/// The files that identify a VCSL checkout (tag `v1.2.2-RC`).
const VCSL_DETECT: [&str; 3] = [
    "Aerophones/Reed Aerophones/Tenor Saxophone - Keyswitch.sfz",
    "Idiophones/Struck Idiophones/Triangles.sfz",
    "Membranophones/Struck Membranophones/Timpani 1 - Keyswitch.sfz",
];

/// A library at `root` holding `files`.
fn library(root: &Path, files: &[&str]) {
    for file in files {
        patch(&root.join(file));
    }
}

#[test]
fn renaming_a_library_without_manifest_changes_its_ids() {
    // Pins the behaviour for a library Sonara has no profile of: its ID is its folder's slug.
    let dir = tempfile::tempdir().expect("tempdir");
    let before = dir.path().join("My Strings");
    library(&before, &["Violin.sfz"]);
    assert_eq!(
        preset_ids(&InstrumentCatalog::discover(std::slice::from_ref(&before))),
        ["my-strings/Violin"]
    );
    let after = dir.path().join("Strings 2");
    fs::rename(&before, &after).expect("rename");
    assert_eq!(
        preset_ids(&InstrumentCatalog::discover(&[after])),
        ["strings-2/Violin"]
    );
}

#[test]
fn a_detected_library_keeps_its_profile_id() {
    // A VSCO 2 CE checkout without a manifest takes the shipped profile's ID and name, so its
    // preset IDs survive renaming the folder; without the profile it would be `vsco-2-ce`.
    let dir = tempfile::tempdir().expect("tempdir");
    let before = dir.path().join("VSCO-2-CE");
    library(&before, &VSCO_DETECT);
    library(&before, &["FHornSus.sfz"]);
    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&before));
    let ids = [
        "vsco2ce/FHornSus",
        "vsco2ce/GM-StylePerc",
        "vsco2ce/Tuba-KS",
        "vsco2ce/ViolinEns-KS",
    ];
    assert_eq!(preset_ids(&catalog), ids);
    let library = &catalog.libraries()[0];
    assert_eq!(library.id.as_str(), "vsco2ce");
    assert_eq!(library.name, "VSCO 2 Community Edition");
    assert!(!library.has_manifest);
    assert!(catalog.issues().is_empty(), "{:?}", catalog.issues());
    // The profile gives the identity only: the presets claim no licence or articulations.
    let horn = &catalog.presets()[0];
    assert_eq!(horn.license, None);
    assert_eq!(horn.articulations, Vec::<String>::new());
    assert!(!matches!(horn.family, FamilyClaim::Declared(_)));

    let after = dir.path().join("Samples/vsco");
    fs::create_dir_all(after.parent().expect("a parent")).expect("mkdir");
    fs::rename(&before, &after).expect("rename");
    assert_eq!(preset_ids(&InstrumentCatalog::discover(&[after])), ids);
}

#[test]
fn a_detected_vcsl_checkout_is_vcsl() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("VCSL-1.2.2-RC");
    library(&root, &VCSL_DETECT);
    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    assert_eq!(catalog.libraries()[0].id.as_str(), "vcsl");
    assert_eq!(
        catalog.libraries()[0].name,
        "Versilian Community Sample Library"
    );
    assert!(preset_ids(&catalog).contains(&"vcsl/Idiophones/Struck Idiophones/Triangles"));
}

#[test]
fn a_profile_needs_every_detect_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("VSCO-2-CE");
    library(&root, &VSCO_DETECT[..2]);
    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    assert_eq!(catalog.libraries()[0].id.as_str(), "vsco-2-ce");
    assert_eq!(catalog.libraries()[0].name, "vsco-2-ce");
}

#[test]
fn a_declared_id_wins_over_a_profile_and_a_manifest_without_one_does_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    let declared = dir.path().join("declared");
    library(&declared, &VSCO_DETECT);
    manifest(
        &declared,
        &json!({"version": 1, "id": "my-vsco", "name": "Mine"}),
    );
    let undeclared = dir.path().join("undeclared");
    library(&undeclared, &VCSL_DETECT);
    manifest(&undeclared, &json!({"version": 1, "license": "CC0 1.0"}));
    let catalog = InstrumentCatalog::discover(&[declared, undeclared]);
    let libraries: Vec<(&str, &str)> = catalog
        .libraries()
        .iter()
        .map(|l| (l.id.as_str(), l.name.as_str()))
        .collect();
    assert_eq!(
        libraries,
        [
            ("my-vsco", "Mine"),
            ("vcsl", "Versilian Community Sample Library")
        ]
    );
    assert_eq!(
        catalog.libraries()[1].license.as_deref(),
        Some("CC0 1.0"),
        "the manifest's own claims stay"
    );
}

#[test]
fn a_folder_that_two_profiles_match_keeps_its_folder_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("Both Libraries");
    library(&root, &VSCO_DETECT);
    library(&root, &VCSL_DETECT);
    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    assert_eq!(catalog.libraries()[0].id.as_str(), "both-libraries");
    let profiles: Vec<&str> = match catalog.issues() {
        [LibraryIssue::ProfileAmbiguous { path, profiles }] if *path == root => {
            profiles.iter().map(LibraryId::as_str).collect()
        }
        other => panic!("{other:?}"),
    };
    assert_eq!(profiles, ["vsco2ce", "vcsl"]);
}

#[test]
fn two_detected_copies_of_one_library_keep_the_first() {
    // Both checkouts take the profile's ID, so the second is a duplicate, as two libraries
    // whose manifests declare one ID are.
    let dir = tempfile::tempdir().expect("tempdir");
    let (a, b) = (dir.path().join("VSCO-2-CE"), dir.path().join("vsco copy"));
    library(&a, &VSCO_DETECT);
    library(&b, &VSCO_DETECT);
    let catalog = InstrumentCatalog::discover(&[a.clone(), b.clone()]);
    assert_eq!(catalog.libraries().len(), 1);
    assert_eq!(catalog.libraries()[0].root, a);
    assert!(matches!(
        catalog.issues(),
        [LibraryIssue::DuplicateLibrary { id, path }] if id.as_str() == "vsco2ce" && *path == b
    ));
}
