//! The instrument catalog against generated library directories (TST-02, TST-03). No audio
//! engine and no real samples: patches are small text files, and deep validation goes through a
//! fake [`PatchLoader`].

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::json;
use sonara_app::instruments::{
    Attack, AttackSource, Availability, Builtin, CatalogError, ChainPreferences, ClaimSource,
    EnsembleClaim, EnsembleKind, FamilyClaim, InstalledInstruments, InstrumentCatalog,
    InstrumentClaim, InstrumentFamily, InstrumentPreset, KeyZone, LibraryId, LibraryIssue,
    MANIFEST_FILE, MediaPatchLoader, PatchFacts, PatchLoadError, PatchLoader, PresetKind, Reach,
    RegionAttack, SoundFacts, SoundPlan, SoundRequest, SoundSource, Step, VelocityRange, Verdict,
    VerdictState, choose_sound,
};
use sonara_app::settings::Settings;
use sonara_media::InstrumentSource;
use sonara_midi::orchestration::{Articulation, InstrumentType, Mute, Request, Vibrato};

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
    let root = dir.path().join("orchestra");
    patch(&root.join("Strings/Violin.sfz"));
    patch(&root.join("Percussion/Timpani.sfz"));
    manifest(
        &root,
        &json!({
            "version": 1, "id": "my-orchestra", "name": "My Orchestra", "license": "CC0 1.0",
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
            "my-orchestra/Percussion/Timpani",
            "my-orchestra/Strings/Violin",
            "my-orchestra/Winds/Flute"
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
    assert_eq!(violin.id.as_str(), "my-orchestra/Strings/Violin");
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
    assert_eq!(library.name, "My Orchestra");
    assert!(library.has_manifest);
    assert_eq!(catalog.available().count(), 2);
    assert!(matches!(
        catalog.issues(),
        [LibraryIssue::ManifestEntryRejected { entry, .. }] if entry == "../escape.sfz"
    ));
}

#[test]
fn a_version_2_manifest_rejects_a_bad_entry_alone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("lib");
    patch(&root.join("Strings/Violin.sfz"));
    patch(&root.join("Brass/Horn.sfz"));
    patch(&root.join("Organ.sfz"));
    manifest(
        &root,
        &json!({
            "version": 2, "id": "lib", "name": "Lib", "license": "CC0 1.0",
            "instruments": [
                {"path": "Strings/Violin.sfz", "name": "Violin Section", "family": "strings",
                 "instrument": "violin", "ensemble": {"kind": "section"},
                 "articulations": [
                     {"id": "sustain", "via": "keyswitch", "key": 36, "default": true},
                     {"id": "pizzicato", "via": "keyswitch", "key": 37}]},
                {"path": "Brass/Horn.sfz", "name": "Horn", "instrument": "horn_f",
                 "articulations": [{"id": "sustain", "via": "keyswitch", "key": 128}]},
                {"path": "Organ.sfz", "family": "keyboards", "license": "CC BY 4.0",
                 "dynamics": {"via": "fixed", "velocity": 100}},
                {"set": "horns", "instrument": "horn_f",
                 "articulations": [{"id": "sustain", "via": "patch", "path": "Brass/Horn.sfz"}]}
            ]
        }),
    );

    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));

    // The horn's entry is rejected and reported; the horn is still found by the scan, and the
    // other entries are used. A multi-patch set is listed as a preset of its own, beside its
    // patch.
    assert_eq!(
        preset_ids(&catalog),
        [
            "lib/@horns",
            "lib/Brass/Horn",
            "lib/Organ",
            "lib/Strings/Violin"
        ]
    );
    assert!(matches!(
        catalog.issues(),
        [LibraryIssue::ManifestEntryRejected { path, entry, reason }]
            if path == &root.join(MANIFEST_FILE)
                && entry == "Brass/Horn.sfz"
                && reason == "articulation \"sustain\": key 128 is not between 0 and 127"
    ));
    let preset = |id: &str| {
        catalog
            .presets()
            .iter()
            .find(|p| p.id.as_str() == id)
            .unwrap_or_else(|| panic!("{id}"))
    };
    let violin = preset("lib/Strings/Violin");
    assert_eq!(violin.display_name, "Violin Section");
    assert_eq!(
        violin.family,
        FamilyClaim::Declared(InstrumentFamily::Strings)
    );
    assert_eq!(violin.articulations, ["sustain", "pizzicato"]);
    assert_eq!(violin.license.as_deref(), Some("CC0 1.0"));
    let horn = preset("lib/Brass/Horn");
    assert_eq!(horn.display_name, "Horn");
    assert_eq!(horn.family, FamilyClaim::Inferred(InstrumentFamily::Brass));
    assert_eq!(horn.articulations, Vec::<String>::new());
    // An entry's own licence is that recording's.
    assert_eq!(preset("lib/Organ").license.as_deref(), Some("CC BY 4.0"));
    assert!(catalog.libraries()[0].has_manifest);
}

#[test]
fn a_manifest_of_a_newer_version_is_invalid_and_the_library_is_scanned() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("lib");
    patch(&root.join("Cello.sfz"));
    manifest(
        &root,
        &json!({"version": 3, "id": "lib-3", "instruments": []}),
    );
    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    assert_eq!(preset_ids(&catalog), ["lib/Cello"]);
    assert!(!catalog.libraries()[0].has_manifest);
    assert!(matches!(
        catalog.issues(),
        [LibraryIssue::ManifestInvalid { path, reason }]
            if path == &root.join(MANIFEST_FILE)
                && reason == "manifest version 3 is not supported (this build reads versions 1 to 2)"
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
    fn load(&self, patch: &Path) -> Result<Option<SoundFacts>, PatchLoadError> {
        self.asked.borrow_mut().push(patch.to_owned());
        if patch.ends_with(self.reject) {
            Err(PatchLoadError("sample Violin_C4.wav is missing".to_owned()))
        } else {
            Ok(None)
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
    // A loader that does not inspect patches records no facts.
    assert!(
        catalog
            .presets()
            .iter()
            .all(|preset| preset.facts.is_none())
    );
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
    tone(&root);
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
    tone(&root);
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

/// Writes `tone.wav`, 64 frames of mono 16-bit audio, into `root`.
fn tone(root: &Path) {
    fs::create_dir_all(root).expect("mkdir");
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
}

/// The deep check records what the patch holds, read from the file and the sample headers,
/// and those facts beat the manifest's claims: a key switch the manifest maps that the patch
/// does not switch on lowers its verdict (TST-02, TST-03).
#[test]
fn verifying_records_the_patch_facts_and_they_beat_the_claims() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("lib");
    tone(&root);
    fs::write(
        root.join("Switch.sfz"),
        "<global> sw_lokey=36 sw_hikey=37 sw_default=36\n\
         <region> sample=tone.wav lokey=60 hikey=62 lovel=1 hivel=63 sw_last=36\n\
         <region> sample=tone.wav lokey=60 hikey=62 lovel=64 hivel=127 sw_last=36\n\
         <region> sample=tone.wav lokey=60 hikey=62 lovel=1 hivel=127 sw_last=37\n",
    )
    .expect("write");
    manifest(
        &root,
        &json!({"version": 2, "instruments": [
            {"path": "Switch.sfz", "instrument": "violin",
             "articulations": [
                 {"id": "sustain", "via": "keyswitch", "key": 36, "default": true},
                 {"id": "pizzicato", "via": "keyswitch", "key": 37},
                 {"id": "tremolo", "via": "keyswitch", "key": 40}],
             "verdict": {"state": "as_is", "lost": []}}
        ]}),
    );

    let mut catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    let id = catalog.presets()[0].id.clone();
    assert_eq!(id.as_str(), "lib/Switch");
    assert_eq!(catalog.presets()[0].facts, None, "discovery opens no patch");

    assert_eq!(
        catalog.verify(&id, &MediaPatchLoader).expect("known"),
        &Availability::Available
    );
    let preset = catalog.get(&id).expect("present");
    assert_eq!(
        preset.facts,
        Some(PatchFacts {
            regions: 3,
            playable_regions: 3,
            sampled: vec![KeyZone {
                lo: 60,
                hi: 62,
                layers: vec![
                    VelocityRange { lo: 1, hi: 63 },
                    VelocityRange { lo: 64, hi: 127 }
                ],
            }],
            switch_keys: vec![36, 37],
            default_switch: Some(36),
            ignored_opcodes: Vec::new(),
            missing_samples: Vec::new(),
            unreadable_samples: Vec::new(),
            notes: Vec::new(),
            // 64 frames, one channel, four bytes a frame, the file counted once.
            decoded_bytes: 256,
            loadable: true,
            // The ramp of `tone.wav` comes within 20 dB of its peak after 7 frames, 0.16 ms;
            // its 64 frames last 1.45 ms at key 60, its key centre.
            sound: Some(SoundFacts {
                attacks: vec![
                    RegionAttack {
                        switch: Some(36),
                        velocities: VelocityRange { lo: 1, hi: 63 },
                        attack_ms: 0,
                    },
                    RegionAttack {
                        switch: Some(36),
                        velocities: VelocityRange { lo: 64, hi: 127 },
                        attack_ms: 0,
                    },
                    RegionAttack {
                        switch: Some(37),
                        velocities: VelocityRange { lo: 1, hi: 127 },
                        attack_ms: 0,
                    },
                ],
                max_hold_ms: Some(2),
            }),
        })
    );
    assert_eq!(
        preset.instrument,
        Some(InstrumentClaim {
            instrument: InstrumentType::Violin,
            source: ClaimSource::Declared
        })
    );
    // The claim is kept as declared; the effective verdict is lowered by the facts.
    assert_eq!(
        preset
            .verdict
            .as_ref()
            .map(|claim| (&claim.value.state, claim.source)),
        Some((&VerdictState::AsIs, ClaimSource::Declared))
    );
    assert_eq!(
        preset.effective_verdict(),
        Some(Verdict {
            state: VerdictState::Partly,
            lost: vec!["tremolo: key 40 is not a switch key of the patch".to_owned()],
        })
    );
}

/// A patch that loses a sample is invalid for the deep check, and its facts say why; with
/// nothing claimed, the facts alone make it `partly`.
#[test]
fn verifying_records_the_samples_a_patch_leaves_out() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("lib");
    tone(&root);
    fs::write(
        root.join("Partial.sfz"),
        "<region> sample=tone.wav key=60\n<region> sample=nope.wav key=61\n",
    )
    .expect("write");
    let mut catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    let id = catalog.presets()[0].id.clone();

    let availability = catalog
        .verify(&id, &MediaPatchLoader)
        .expect("known")
        .clone();

    assert!(
        matches!(availability, Availability::Invalid { .. }),
        "{availability:?}"
    );
    let preset = catalog.get(&id).expect("present");
    let facts = preset.facts.as_ref().expect("inspected");
    assert_eq!(
        facts.sound, None,
        "a patch that does not load measures nothing"
    );
    assert_eq!(facts.missing_samples, [root.join("nope.wav")]);
    assert_eq!((facts.regions, facts.playable_regions), (2, 1));
    assert!(facts.loadable, "the sampler plays the other region");
    let note = format!(
        "sample {} is missing (1 region left out)",
        root.join("nope.wav").display()
    );
    assert_eq!(facts.notes, std::slice::from_ref(&note));
    assert_eq!(
        preset.effective_verdict(),
        Some(Verdict {
            state: VerdictState::Partly,
            lost: vec![note],
        })
    );
}

/// Writes a mono 16-bit WAV at 44.1 kHz into `root`: `silent` frames of silence, then `loud`
/// frames at full scale. 441 frames are 10 ms.
fn step(root: &Path, name: &str, silent: u32, loud: u32) {
    fs::create_dir_all(root).expect("mkdir");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut wav = hound::WavWriter::create(root.join(name), spec).expect("wav");
    for frame in 0..silent + loud {
        let level = if frame < silent { 0 } else { i16::MAX };
        wav.write_sample(level).expect("sample");
    }
    wav.finalize().expect("finalize");
}

/// The deep check decodes the samples and measures, on generated samples of known onset and
/// length, each articulation's attack and the longest a held note lasts; an articulation whose
/// entry states its `attack_ms` keeps that instead (TST-02, TST-10).
#[test]
fn verify_measures_onset_and_longest_unlooped_sample() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("lib");
    // A sustain that speaks after 30 ms and lasts 2 s, a staccato that speaks after 10 ms and
    // lasts 0.2 s, and a tremolo that speaks at once.
    step(&root, "sus.wav", 1_323, 86_877);
    step(&root, "stac.wav", 441, 8_379);
    step(&root, "trem.wav", 0, 4_410);
    fs::write(
        root.join("Violin.sfz"),
        "<global> sw_lokey=24 sw_hikey=26 sw_default=24 lokey=55 hikey=60 pitch_keycenter=60\n\
         <region> sample=sus.wav sw_last=24\n\
         <region> sample=sus.wav sw_last=24 offset=441 lovel=100\n\
         <region> sample=stac.wav sw_last=25 ampeg_attack=0.005\n\
         <region> sample=trem.wav sw_last=26\n",
    )
    .expect("write");
    manifest(
        &root,
        &json!({"version": 2, "instruments": [
            {"path": "Violin.sfz", "instrument": "violin",
             "articulations": [
                 {"id": "sustain", "via": "keyswitch", "key": 24, "default": true},
                 {"id": "staccato", "via": "keyswitch", "key": 25},
                 {"id": "tremolo", "via": "keyswitch", "key": 26, "attack_ms": 120},
                 {"id": "pizzicato", "via": "cc", "cc": 32, "value": 10}]}
        ]}),
    );
    let stated = Attack {
        ms: 120,
        source: AttackSource::Stated(ClaimSource::Declared),
    };
    let mut catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    let id = catalog.presets()[0].id.clone();
    let unchecked = catalog.get(&id).expect("present");
    assert_eq!(unchecked.max_hold_ms(), None);
    assert_eq!(unchecked.patch_attack_ms(), None);
    assert_eq!(
        unchecked
            .articulation_attacks()
            .into_iter()
            .map(|(entry, attack)| (entry.name.id(), attack))
            .collect::<Vec<_>>(),
        [("tremolo", stated)],
        "before the check only a stated attack is known"
    );

    assert_eq!(
        catalog.verify(&id, &MediaPatchLoader).expect("known"),
        &Availability::Available
    );
    let preset = catalog.get(&id).expect("present");
    let sound = preset
        .facts
        .as_ref()
        .and_then(|facts| facts.sound.as_ref())
        .expect("measured");
    let attacks: Vec<_> = sound
        .attacks
        .iter()
        .map(|region| (region.switch, region.attack_ms))
        .collect();
    // The offset of 441 frames starts the second sustain region 10 ms into its silence.
    assert_eq!(
        attacks,
        [
            (Some(24), 30),
            (Some(24), 20),
            (Some(25), 15),
            (Some(26), 0)
        ]
    );
    // Key 55 lies five semitones below the key centre: 88200 frames last 2 s at 44.1 kHz, and
    // 2 s × 2^(5/12) = 2669.7 ms played that much slower, rounded up.
    assert_eq!(sound.max_hold_ms, Some(2_670));
    assert_eq!(preset.max_hold_ms(), Some(2_670));

    let measured = |ms| Attack {
        ms,
        source: AttackSource::Measured,
    };
    let attacks: Vec<_> = preset
        .articulation_attacks()
        .into_iter()
        .map(|(entry, attack)| (entry.name.id(), attack))
        .collect();
    assert_eq!(
        attacks,
        [
            // The median of 30 and 20 ms.
            ("sustain", measured(25)),
            ("staccato", measured(15)),
            ("tremolo", stated),
        ],
        "a stated attack overrides the measured one, and a controller articulation has none"
    );
    // The whole patch: the median of 0, 15, 20 and 30 ms.
    assert_eq!(preset.patch_attack_ms(), Some(18));
}

/// A patch that loops while a key is held has no longest hold: a note sounds as long as it is
/// held (TST-03).
#[test]
fn a_looping_patch_has_no_longest_hold() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("lib");
    step(&root, "sus.wav", 0, 4_410);
    fs::write(
        root.join("Organ.sfz"),
        "<region> sample=sus.wav key=60\n\
         <region> sample=sus.wav key=61 loop_mode=loop_continuous loop_start=100 loop_end=4000\n",
    )
    .expect("write");
    let mut catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    let id = catalog.presets()[0].id.clone();
    catalog.verify(&id, &MediaPatchLoader).expect("known");
    let preset = catalog.get(&id).expect("present");
    assert!(
        preset
            .facts
            .as_ref()
            .is_some_and(|facts| facts.sound.is_some())
    );
    assert_eq!(preset.max_hold_ms(), None);
    assert_eq!(preset.patch_attack_ms(), Some(0));
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

    let mut catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
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
    // What the structural check could read is recorded, and lowers the verdict to `not`; a
    // patch it cannot read records no facts.
    let checked = |name: &str| {
        let preset = catalog
            .presets()
            .iter()
            .find(|preset| preset.display_name == name)
            .unwrap_or_else(|| panic!("{name}"));
        (preset.facts.clone(), preset.effective_verdict())
    };
    assert_eq!(checked("Garbage"), (None, None));
    assert_eq!(checked("Loop"), (None, None), "the include is too deep");
    let (facts, verdict) = checked("NoRegion");
    assert_eq!(
        facts.map(|facts| (facts.regions, facts.loadable)),
        Some((0, false))
    );
    assert_eq!(
        verdict,
        Some(Verdict {
            state: VerdictState::Not,
            lost: vec!["the patch has no region".to_owned()]
        })
    );
    let (facts, verdict) = checked("BadWav");
    let facts = facts.expect("inspected");
    assert_eq!(
        (facts.regions, facts.playable_regions, facts.loadable),
        (1, 0, false)
    );
    assert_eq!(facts.unreadable_samples, [dir.path().join("lib/bad.wav")]);
    let verdict = verdict.expect("lowered by the facts");
    assert_eq!(verdict.state, VerdictState::Not);
    assert_eq!(
        verdict.lost[..2],
        [
            "no region of the patch can play",
            "the patch does not load while a sample file cannot be read"
        ]
    );
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
    assert_eq!(available_ids(&catalog), ids);
    let library = &catalog.libraries()[0];
    assert_eq!(library.id.as_str(), "vsco2ce");
    assert_eq!(library.name, "VSCO 2 Community Edition");
    assert!(!library.has_manifest);
    assert!(catalog.issues().is_empty(), "{:?}", catalog.issues());
    // The profile lists the 75 patches of the library's branch `SFZ` and its 15 multi-patch
    // sets: those not on disk are missing, so the user sees what a partial copy lacks.
    assert_eq!(catalog.presets().len(), 90);
    let flute = preset(&catalog, "vsco2ce/FluteSusVib");
    assert_eq!(flute.availability, Availability::Missing);
    assert_eq!(flute.display_name, "Flute, sustain vibrato");
    // The profile names the presets, but what it claims is Sonara's word, never the library's:
    // the presets declare no licence, articulations or family, and every claim of the profile
    // is shown as the profile's.
    let horn = preset(&catalog, "vsco2ce/FHornSus");
    assert_eq!(horn.display_name, "Horn, sustain");
    assert_eq!(horn.license, None);
    assert_eq!(horn.articulations, Vec::<String>::new());
    assert_eq!(horn.family, FamilyClaim::Profile(InstrumentFamily::Brass));
    assert_eq!(
        horn.instrument,
        Some(InstrumentClaim {
            instrument: InstrumentType::HornF,
            source: ClaimSource::Profile
        })
    );
    let map = horn
        .articulation_map
        .as_ref()
        .expect("mapped by the profile");
    assert_eq!(
        (map.value.ids(), map.source),
        (vec!["sustain"], ClaimSource::Profile)
    );
    assert_eq!(
        horn.license_claim
            .as_ref()
            .map(|claim| (claim.value.as_str(), claim.source)),
        Some(("CC0 1.0", ClaimSource::Profile))
    );
    let section = preset(&catalog, "vsco2ce/ViolinEns-KS");
    assert_eq!(
        section
            .ensemble
            .as_ref()
            .map(|claim| (claim.value, claim.source)),
        Some((
            EnsembleClaim {
                kind: EnsembleKind::Section,
                players: None
            },
            ClaimSource::Profile
        ))
    );
    assert_eq!(
        section
            .articulation_map
            .as_ref()
            .map(|claim| claim.value.ids()),
        Some(vec!["sustain", "tremolo", "spiccato", "pizzicato"])
    );
    // The percussion patch plays several instruments, so it claims none.
    assert_eq!(preset(&catalog, "vsco2ce/GM-StylePerc").instrument, None);
    let after = dir.path().join("Samples/vsco");
    fs::create_dir_all(after.parent().expect("a parent")).expect("mkdir");
    fs::rename(&before, &after).expect("rename");
    assert_eq!(available_ids(&InstrumentCatalog::discover(&[after])), ids);
}

/// A shipped profile's set is listed beside its patches. With only its default patch on disk,
/// the set is missing, and each member says which of its patches are here (TST-03).
#[test]
fn a_partly_installed_set_is_missing_and_names_its_members() {
    let dir = tempfile::tempdir().expect("tempdir");
    let before = dir.path().join("VSCO-2-CE");
    library(&before, &VSCO_DETECT);
    library(&before, &["FHornSus.sfz"]);
    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&before));
    let set = preset(&catalog, "vsco2ce/@horn");
    assert_eq!(set.availability, Availability::Missing);
    assert_eq!(set.patch_path, before.join("FHornSus.sfz"));
    let PresetKind::Set { members } = &set.kind else {
        panic!("a set: {:?}", set.kind);
    };
    assert_eq!(
        members
            .iter()
            .map(|member| (member.path.as_str(), member.availability.is_available()))
            .collect::<Vec<_>>(),
        [
            ("FHornSus.sfz", true),
            ("FHornStac.sfz", false),
            ("FHornMute.sfz", false)
        ]
    );
    assert_eq!(
        (set.family, set.instrument.map(|claim| claim.source)),
        (
            FamilyClaim::Profile(InstrumentFamily::Brass),
            Some(ClaimSource::Profile)
        )
    );
    assert_eq!(
        catalog
            .playing(InstrumentType::HornF)
            .map(|preset| preset.id.as_str())
            .collect::<Vec<_>>(),
        [
            "vsco2ce/@horn",
            "vsco2ce/FHornMute",
            "vsco2ce/FHornStac",
            "vsco2ce/FHornSus"
        ]
    );
    // With its other patches installed, the set is available.
    library(&before, &["FHornStac.sfz", "FHornMute.sfz"]);
    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&before));
    assert_eq!(
        preset(&catalog, "vsco2ce/@horn").availability,
        Availability::Available
    );
}

/// The IDs of the presets that can be played, sorted.
fn available_ids(catalog: &InstrumentCatalog) -> Vec<&str> {
    catalog.available().map(|p| p.id.as_str()).collect()
}

/// The preset with ID `id`.
fn preset<'a>(catalog: &'a InstrumentCatalog, id: &str) -> &'a InstrumentPreset {
    catalog
        .presets()
        .iter()
        .find(|p| p.id.as_str() == id)
        .unwrap_or_else(|| panic!("no preset {id}"))
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
fn a_manifest_that_declares_a_shipped_id_is_that_library() {
    // The library's own manifest names it VSCO 2 CE, so the shipped profile applies below it:
    // the name the manifest leaves out is the profile's.
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("My Copy");
    library(&root, &["Tuba-KS.sfz"]);
    manifest(&root, &json!({"version": 2, "id": "vsco2ce"}));
    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    let library = &catalog.libraries()[0];
    assert_eq!(library.id.as_str(), "vsco2ce");
    assert_eq!(library.name, "VSCO 2 Community Edition");
    assert_eq!(available_ids(&catalog), ["vsco2ce/Tuba-KS"]);
    assert_eq!(
        preset(&catalog, "vsco2ce/Tuba-KS").display_name,
        "Tuba, key switches"
    );
    assert_eq!(
        catalog.presets().len(),
        90,
        "the profile's patches and sets"
    );
    // `null` clears the profile's name, so the library is shown by its ID.
    manifest(&root, &json!({"version": 2, "id": "vsco2ce", "name": null}));
    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    assert_eq!(catalog.libraries()[0].name, "vsco2ce");
}

#[test]
fn a_manifest_in_a_detected_library_overrides_the_profile() {
    // A VSCO 2 CE checkout found by its files, with a manifest of its own that declares no ID:
    // the manifest's claims lie over the shipped profile's, field by field.
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("VSCO-2-CE");
    library(&root, &VSCO_DETECT);
    library(&root, &["FHornSus.sfz", "Extra/Celesta.sfz"]);
    manifest(
        &root,
        &json!({"version": 2, "license": "CC0 1.0", "instruments": [
            {"path": "Tuba-KS.sfz", "name": "My Tuba", "family": "brass",
             "articulations": [{"id": "staccato", "via": "keyswitch", "key": 85}]},
            {"path": "fhornsus.sfz", "name": null},
            {"path": "Extra/Celesta.sfz", "name": "Celesta"}
        ]}),
    );
    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    let library = &catalog.libraries()[0];
    assert_eq!(
        library.id.as_str(),
        "vsco2ce",
        "detected; the manifest has no ID"
    );
    assert_eq!(library.name, "VSCO 2 Community Edition");
    assert_eq!(library.license.as_deref(), Some("CC0 1.0"));
    assert!(catalog.issues().is_empty(), "{:?}", catalog.issues());

    // A field the manifest writes replaces the profile's; its articulation list replaces the
    // profile's whole, and what it declares is shown as declared.
    let tuba = preset(&catalog, "vsco2ce/Tuba-KS");
    assert_eq!(tuba.display_name, "My Tuba");
    assert_eq!(tuba.family, FamilyClaim::Declared(InstrumentFamily::Brass));
    assert_eq!(tuba.articulations, ["staccato"]);
    assert_eq!(tuba.license.as_deref(), Some("CC0 1.0"));
    // `null` clears the profile's name, so the file name shows; the manifest's spelling of the
    // path matches the profile's entry ignoring case.
    assert_eq!(
        preset(&catalog, "vsco2ce/FHornSus").display_name,
        "FHornSus"
    );
    // A field the manifest leaves out keeps the profile's.
    assert_eq!(
        preset(&catalog, "vsco2ce/ViolinEns-KS").display_name,
        "Violin Section, key switches"
    );
    // An entry only in the manifest is added to the profile's 75 patches and 15 sets.
    assert_eq!(
        preset(&catalog, "vsco2ce/Extra/Celesta").display_name,
        "Celesta"
    );
    assert_eq!(catalog.presets().len(), 91);
    assert_eq!(
        available_ids(&catalog),
        [
            "vsco2ce/Extra/Celesta",
            "vsco2ce/FHornSus",
            "vsco2ce/GM-StylePerc",
            "vsco2ce/Tuba-KS",
            "vsco2ce/ViolinEns-KS",
        ]
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

/// Two libraries: `Lib One` with a manifest that lists a missing patch, and `VSCO-2-CE`, a
/// detected checkout with an empty patch file.
fn two_libraries(dir: &Path) -> (PathBuf, PathBuf) {
    let one = dir.join("Lib One");
    library(&one, &["Strings/Violin.sfz", "Brass/Horn.sfz"]);
    manifest(
        &one,
        &json!({"version": 1, "instruments": [{"path": "Strings/Gone.sfz"}]}),
    );
    let vsco = dir.join("VSCO-2-CE");
    library(&vsco, &VSCO_DETECT);
    fs::write(vsco.join("Empty.sfz"), "").expect("write");
    (one, vsco)
}

#[test]
fn installed_instruments_are_the_playable_presets_by_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (one, vsco) = two_libraries(dir.path());
    // Both patches of the tuba's set, so the set is available.
    library(&vsco, &["TubaSus.sfz", "TubaStac.sfz"]);
    let catalog = InstrumentCatalog::discover(&[one, vsco]);
    // The catalog lists all of them: three of `Lib One` (one missing), and the 75 patches the
    // VSCO 2 CE profile names (70 of them missing), its 15 sets (all but the tuba's missing)
    // and the empty patch the scan finds.
    assert_eq!(catalog.presets().len(), 94, "the catalog lists all of them");
    let tuba = preset(&catalog, "vsco2ce/@tuba");
    assert_eq!(tuba.availability, Availability::Available);
    assert!(matches!(&tuba.kind, PresetKind::Set { members } if members.len() == 2));
    let installed = InstalledInstruments::from_catalog(&catalog);
    // One sampler device plays a whole set: the available set is offered beside its patches.
    assert_eq!(
        installed.ids(),
        [
            "lib-one/Brass/Horn",
            "lib-one/Strings/Violin",
            "vsco2ce/@tuba",
            "vsco2ce/GM-StylePerc",
            "vsco2ce/Tuba-KS",
            "vsco2ce/TubaStac",
            "vsco2ce/TubaSus",
            "vsco2ce/ViolinEns-KS",
        ],
        "a missing or an invalid preset, or a set with a missing patch, is not offered"
    );
    assert_eq!(installed.preset_id("vsco2ce/@tuba"), Some("vsco2ce/@tuba"));
    let set = installed
        .resolve("vsco2ce/@tuba")
        .expect("the set resolves");
    let PresetKind::Set { members } = &tuba.kind else {
        panic!("a set");
    };
    assert_eq!(
        set.patches,
        members
            .iter()
            .map(|member| member.patch_path.clone())
            .collect::<Vec<_>>(),
        "the set's patches, in the order its targets number them"
    );
    assert_eq!(
        installed.resolve("vsco2ce/@horn"),
        None,
        "a patch is missing"
    );
    assert!(!installed.is_empty());
    assert!(InstalledInstruments::default().is_empty());
    assert_eq!(
        InstalledInstruments::from_settings(&Settings::default()),
        InstalledInstruments::default()
    );
}

#[test]
fn an_old_patch_path_resolves_to_its_preset() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (one, vsco) = two_libraries(dir.path());
    let installed = InstalledInstruments::from_catalog(&InstrumentCatalog::discover(&[
        one.clone(),
        vsco.clone(),
    ]));
    let text = |path: PathBuf| path.to_str().expect("UTF-8").to_owned();
    let violin = Some(InstrumentSource {
        id: "lib-one/Strings/Violin".to_owned(),
        patches: vec![one.join("Strings/Violin.sfz")],
        mapping: Arc::default(),
    });
    // By its ID, and by the absolute path a project from before preset IDs stored: the scan's
    // path, the library directory joined with the patch.
    assert_eq!(installed.resolve("lib-one/Strings/Violin"), violin);
    assert_eq!(
        installed.resolve(&text(one.join("Strings/Violin.sfz"))),
        violin
    );
    // The patch part ignores case, as discovery does; the library directory is as configured.
    assert_eq!(
        installed.resolve(&text(one.join("strings/VIOLIN.SFZ"))),
        violin
    );
    assert_eq!(
        installed.preset_id(&text(vsco.join("Tuba-KS.sfz"))),
        Some("vsco2ce/Tuba-KS"),
        "a detected library maps its old paths to the profile ID"
    );
    for nothing in [
        "lib-one/Strings/Gone".to_owned(),
        text(one.join("Strings/Gone.sfz")),
        "vsco2ce/Empty".to_owned(),
        text(vsco.join("Empty.sfz")),
        text(dir.path().join("Elsewhere/Strings/Violin.sfz")),
        "/Lib One/Strings/Violin.sfz".to_owned(),
        "lib-one/strings/violin".to_owned(),
        "Strings/Violin.sfz".to_owned(),
        String::new(),
    ] {
        assert_eq!(installed.resolve(&nothing), None, "{nothing:?}");
    }
}

#[test]
fn installed_instruments_are_grouped_by_library_with_their_names() {
    // The device editor's instrument combo: names, never paths, under the library's name.
    let dir = tempfile::tempdir().expect("tempdir");
    let bare = dir.path().join("Bare");
    library(&bare, &["b/Horn.sfz", "a/Horn.sfz", "Tuba.sfz"]);
    let named = dir.path().join("Zeta Strings");
    library(
        &named,
        &["Solo/Violin.sfz", "Ens/Violins.sfz", "Solo/Cello.sfz"],
    );
    fs::write(named.join("Empty.sfz"), "").expect("write");
    manifest(
        &named,
        &json!({"version": 1, "id": "zeta", "name": "Alpha Strings", "instruments": [
            {"path": "Solo/Violin.sfz", "name": "Violin"},
            {"path": "Ens/Violins.sfz", "name": "violin"},
            {"path": "Gone.sfz", "name": "Gone"}
        ]}),
    );
    let installed =
        InstalledInstruments::from_catalog(&InstrumentCatalog::discover(&[bare, named]));
    // Libraries by name, not by ID or configured order; presets by label; a name that two
    // presets of a library share, ignoring case, gets the patch path below the library. A
    // missing or invalid preset is not offered, so it neither shows nor makes a name shared.
    assert_eq!(
        serde_json::to_value(installed.by_library()).expect("JSON"),
        json!([
            {"id": "zeta", "name": "Alpha Strings", "presets": [
                {"id": "zeta/Solo/Cello", "label": "Cello"},
                {"id": "zeta/Ens/Violins", "label": "violin (Ens/Violins)"},
                {"id": "zeta/Solo/Violin", "label": "Violin (Solo/Violin)"}
            ]},
            {"id": "bare", "name": "bare", "presets": [
                {"id": "bare/a/Horn", "label": "Horn (a/Horn)"},
                {"id": "bare/b/Horn", "label": "Horn (b/Horn)"},
                {"id": "bare/Tuba", "label": "Tuba"}
            ]}
        ])
    );
    // The groups hold exactly the sampler's choices (TST-07).
    let mut grouped: Vec<String> = installed
        .by_library()
        .iter()
        .flat_map(|group| group.presets.iter().map(|preset| preset.id.clone()))
        .collect();
    grouped.sort();
    assert_eq!(grouped, installed.ids());
    assert_eq!(InstalledInstruments::default().by_library(), []);
}

#[test]
fn a_detected_library_groups_its_presets_under_the_profile_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (one, vsco) = two_libraries(dir.path());
    let installed = InstalledInstruments::from_catalog(&InstrumentCatalog::discover(&[one, vsco]));
    let groups: Vec<(&str, Vec<&str>)> = installed
        .by_library()
        .iter()
        .map(|group| {
            (
                group.name.as_str(),
                group.presets.iter().map(|p| p.label.as_str()).collect(),
            )
        })
        .collect();
    assert_eq!(
        groups,
        [
            ("lib-one", vec!["Horn", "Violin"]),
            (
                "VSCO 2 Community Edition",
                vec![
                    "GM-Style Percussion",
                    "Tuba, key switches",
                    "Violin Section, key switches"
                ]
            ),
        ],
        "a manifest without a name shows the ID; the profile names a detected checkout and its \
         presets"
    );
}

#[test]
fn only_a_sampler_path_is_rewritten_to_its_preset_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (one, _) = two_libraries(dir.path());
    let installed = InstalledInstruments::from_catalog(&InstrumentCatalog::discover(
        std::slice::from_ref(&one),
    ));
    let horn = one
        .join("Brass/Horn.sfz")
        .to_str()
        .expect("UTF-8")
        .to_owned();
    let values = |instrument: &str| {
        let mut map = serde_json::Map::new();
        map.insert("volume".to_owned(), json!(-3.0));
        map.insert("instrument".to_owned(), json!(instrument));
        map
    };
    let normalized = |plugin: &str, instrument: &str| {
        let mut map = values(instrument);
        installed.normalize_params(plugin, &mut map);
        map
    };
    assert_eq!(
        normalized("sonara.sampler", &horn),
        values("lib-one/Brass/Horn")
    );
    assert_eq!(
        normalized("sonara.sampler", "lib-one/Brass/Horn"),
        values("lib-one/Brass/Horn")
    );
    // Not installed: left for the descriptor to reject with its choices.
    assert_eq!(
        normalized("sonara.sampler", "/elsewhere/Horn.sfz"),
        values("/elsewhere/Horn.sfz")
    );
    assert_eq!(normalized("sonara.synth.poly", &horn), values(&horn));
    let mut number = serde_json::Map::new();
    number.insert("instrument".to_owned(), json!(1));
    let expected = number.clone();
    installed.normalize_params("sonara.sampler", &mut number);
    assert_eq!(number, expected, "an index is the descriptor's business");
}

/// A loader whose structural check finds one sample missing in every patch.
struct SampleMissing;

impl PatchLoader for SampleMissing {
    fn load(&self, _patch: &Path) -> Result<Option<SoundFacts>, PatchLoadError> {
        Ok(None)
    }

    fn inspect(&self, _patch: &Path) -> Option<PatchFacts> {
        Some(PatchFacts {
            regions: 2,
            playable_regions: 1,
            sampled: Vec::new(),
            switch_keys: Vec::new(),
            default_switch: None,
            ignored_opcodes: Vec::new(),
            missing_samples: vec![PathBuf::from("/lib/a.wav")],
            unreadable_samples: Vec::new(),
            notes: vec!["sample /lib/a.wav is missing (1 region left out)".to_owned()],
            decoded_bytes: 64,
            loadable: true,
            sound: None,
        })
    }
}

/// The degradation chain over a discovered VSCO 2 CE checkout that holds a few patches (TST-02):
/// the profile's claims choose the preset, a missing patch is never chosen, a neighbour stands
/// in for an instrument the checkout lacks, a key map plays a drum, and a patch whose check
/// finds a sample missing gives way to a built-in sound.
#[test]
fn the_chain_chooses_from_a_discovered_library() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("VSCO-2-CE");
    library(&root, &VSCO_DETECT);
    library(&root, &["FHornSus.sfz", "FHornStac.sfz"]);
    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    let choose = |catalog: &InstrumentCatalog, request| {
        choose_sound(catalog.presets(), &request, ChainPreferences::default())
    };
    let part = |instrument, articulations: &[Articulation]| SoundRequest {
        articulations: articulations
            .iter()
            .map(|articulation| {
                Request::new(Some(*articulation), Mute::Open, Vibrato::Ordinary).expect("primary")
            })
            .collect(),
        ..SoundRequest::new(instrument)
    };
    let source = |plan: &SoundPlan| match &plan.source {
        SoundSource::Sampler { preset } => preset.as_str().to_owned(),
        other => format!("{other:?}"),
    };
    let messages = |plan: &SoundPlan| -> Vec<String> {
        plan.notices
            .iter()
            .map(|notice| notice.message.clone())
            .collect()
    };

    let violins = SoundRequest {
        ensemble: Some(EnsembleKind::Section),
        ..part(
            InstrumentType::Violin,
            &[Articulation::Sustain, Articulation::Pizzicato],
        )
    };
    let plan = choose(&catalog, violins.clone());
    assert_eq!(
        (plan.step, source(&plan), messages(&plan)),
        (Step::Exact, "vsco2ce/ViolinEns-KS".to_owned(), Vec::new())
    );

    // The horn set lacks its muted patch, so it is missing; of the two patches here, the
    // sustain comes first and plays the staccato shortened.
    let horns = part(
        InstrumentType::HornF,
        &[Articulation::Sustain, Articulation::Staccato],
    );
    let plan = choose(&catalog, horns.clone());
    assert_eq!(source(&plan), "vsco2ce/FHornSus");
    assert_eq!(
        messages(&plan),
        [
            "Staccato is not available for Horn in F; playing sustain, each note shortened to a \
          quarter of its length."
        ]
    );

    let plan = choose(&catalog, part(InstrumentType::Cimbasso, &[]));
    assert_eq!(plan.step, Step::FamilyNeighbour);
    assert_eq!(
        messages(&plan),
        ["No Cimbasso is installed; Tuba (vsco2ce/Tuba-KS) plays the part."]
    );

    let plan = choose(&catalog, part(InstrumentType::SnareDrum, &[]));
    assert_eq!(source(&plan), "vsco2ce/GM-StylePerc");
    assert_eq!(plan.articulations[0].reach, Reach::Keys(vec![38]));

    // With its muted patch installed, the set plays both articulations as asked.
    library(&root, &["FHornMute.sfz"]);
    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    let plan = choose(&catalog, horns);
    assert_eq!(
        (plan.step, source(&plan), messages(&plan)),
        (Step::Exact, "vsco2ce/@horn".to_owned(), Vec::new())
    );

    // A check that finds a sample missing rules the violin section out.
    let mut catalog = catalog;
    let section = preset(&catalog, "vsco2ce/ViolinEns-KS").id.clone();
    catalog.verify(&section, &SampleMissing).expect("a preset");
    let plan = choose(&catalog, violins);
    assert_eq!(plan.step, Step::Builtin);
    assert_eq!(
        plan.source,
        SoundSource::Builtin(Builtin::Synth { preset: "Strings" })
    );
}
