//! The project store (`tests/project_format/test_store.py` and the store parts of
//! `test_serializer.py` in the prototype). The rich project is the prototype's
//! `build_rich_project` document (`fixtures/rich.project.json`); its asset is patched to the
//! WAV file each test writes.

use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use sonara_core::error::ErrorCode;
use sonara_core::ids::SequentialIds;
use sonara_core::model::JsonMap;
use sonara_core::model::media::{MediaAsset, MediaAssetData, MediaKind, MediaLocation};
use sonara_core::model::project::Project;
use sonara_project::atomic::{atomic_write_bytes, sha256_bytes, sha256_file};
use sonara_project::schema::{Schema, validate_document};
use sonara_project::serializer::{
    asset_to_value, dumps, loads, project_from_value, project_to_value,
};
use sonara_project::store::{
    Layout, MANIFEST, PROJECT, ProjectStore, StoreError, asset_path, detect_layout,
    required_features, safe_extract,
};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const AUDIO_REL: &str = "media/audio/med_vox-vox.wav";
/// 2026-01-01T12:00:00Z, the recorder's frozen clock.
const FROZEN_UNIX_SECONDS: u64 = 1_767_268_800;
/// Frames of the test WAV files: 0.5 s at 48 kHz, as in the prototype's `write_wav`.
const FRAMES: u32 = 24_000;

// -- fixtures -------------------------------------------------------------------------------

/// A stereo 24-bit WAV at 48 kHz with `frames` frames; `seed` varies the content.
fn write_wav(path: &Path, frames: u32, seed: i32) -> PathBuf {
    fs::create_dir_all(path.parent().expect("parent")).expect("directory");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("WAV writer");
    for frame in 0..frames {
        let value = (i32::try_from(frame % 4096).expect("small") - 2048) * (1000 + seed);
        writer.write_sample(value).expect("sample");
        writer.write_sample(-value).expect("sample");
    }
    writer.finalize().expect("WAV written");
    path.to_path_buf()
}

/// The prototype's `asset_for`: an embedded audio asset describing the file at `rel`.
fn asset_for(workspace: &Path, rel: &str, asset_id: &str) -> MediaAsset {
    let path = workspace.join(rel);
    let mut metadata = JsonMap::new();
    metadata.insert("title".into(), json!("Vox"));
    MediaAsset::new(MediaAssetData {
        id: asset_id.into(),
        kind: MediaKind::Audio,
        name: "vox.wav".into(),
        path: rel.into(),
        location: MediaLocation::Embedded,
        size_bytes: fs::metadata(&path).expect("asset file").len(),
        sha256: Some(sha256_file(&path).expect("hash")),
        sample_rate: 48_000,
        channels: 2,
        frames: u64::from(FRAMES),
        format: "WAV".into(),
        subtype: "PCM_24".into(),
        original_path: Some("/original/vox.wav".into()),
        relative_hint: None,
        metadata,
        extra: JsonMap::new(),
    })
    .expect("valid asset")
}

/// The prototype's `build_rich_project` with `asset` as its only asset.
fn build_rich_project(asset: &MediaAsset) -> Project {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rich.project.json");
    let mut doc = loads(&fs::read(fixture).expect("fixture")).expect("JSON object");
    doc.insert("assets".into(), json!([asset_to_value(asset)]));
    project_from_value(&doc).expect("rich project")
}

/// A temporary directory with `ws/` holding the vocal take, and the rich project.
struct Setup {
    dir: tempfile::TempDir,
    workspace: PathBuf,
    project: Project,
}

impl Setup {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let workspace = dir.path().join("ws");
        write_wav(&workspace.join(AUDIO_REL), FRAMES, 0);
        let project = build_rich_project(&asset_for(&workspace, AUDIO_REL, "med_vox"));
        Self {
            dir,
            workspace,
            project,
        }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    /// The store of the prototype's fixture (3 backups), with a clock that starts at the frozen
    /// time and advances by a microsecond per reading, so backup names stay distinct.
    fn store(&self) -> ProjectStore {
        let ticks = AtomicU64::new(0);
        ProjectStore::new(self.path("workspaces"))
            .with_backups(3)
            .with_clock(move || {
                UNIX_EPOCH
                    + Duration::from_secs(FROZEN_UNIX_SECONDS)
                    + Duration::from_micros(ticks.fetch_add(1, Ordering::Relaxed))
            })
    }
}

fn renamed(project: &Project, name: &str) -> Project {
    project.edit(|d| d.name = name.into()).expect("valid name")
}

fn code<T: std::fmt::Debug>(result: Result<T, StoreError>) -> ErrorCode {
    result.expect_err("an error").code()
}

fn domain<T: std::fmt::Debug>(result: Result<T, StoreError>) -> sonara_core::error::DomainError {
    match result.expect_err("an error") {
        StoreError::Domain(err) => err,
        other => panic!("not a domain error: {other}"),
    }
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("readable")).expect("JSON")
}

fn backups_of(bundle: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(bundle.join("backups"))
        .map(|entries| entries.map(|e| e.expect("entry").path()).collect())
        .unwrap_or_default();
    out.retain(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_prefix("project-"))
            .is_some_and(|rest| rest.strip_suffix(".json").is_some())
    });
    out.sort();
    out
}

// -- bundles --------------------------------------------------------------------------------

#[test]
fn bundle_save_and_open() {
    let s = Setup::new();
    let store = s.store();
    let target = s.path("Song.sonara");
    let res = store
        .save(&s.project, &s.workspace, &target, None)
        .expect("save");
    assert_eq!(res.layout, Layout::Bundle);
    assert_eq!(res.workspace, target);
    assert_eq!(res.assets, 1);
    assert_eq!(res.backup, None);
    assert_eq!(
        sha256_file(&target.join(AUDIO_REL)).expect("copied"),
        sha256_file(&s.workspace.join(AUDIO_REL)).expect("source")
    );
    let manifest = read_json(&target.join(MANIFEST));
    assert_eq!(
        manifest["project_file"]["sha256"],
        sha256_bytes(&fs::read(target.join(PROJECT)).expect("project"))
    );
    assert_eq!(res.project_sha256, manifest["project_file"]["sha256"]);
    assert_eq!(manifest["assets"][0]["path"], AUDIO_REL);
    assert_eq!(manifest["project_id"], s.project.id.as_str());
    assert_eq!(manifest["saved"], "2026-01-01T12:00:00Z");
    assert!(
        manifest["required_features"]
            .as_array()
            .expect("list")
            .contains(&json!("sidechain"))
    );
    assert_eq!(detect_layout(&target), Some(Layout::Bundle));
    let loaded = store.open(&target, false).expect("open");
    assert_eq!(loaded.project, s.project);
    assert_eq!(loaded.layout, Layout::Bundle);
    assert_eq!(loaded.workspace, target);
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    assert_eq!(loaded.recovered_from, None);
    assert_eq!(loaded.migrated_from, None);
    assert!(!loaded.autosave_available);
    assert_eq!(loaded.missing_assets, Vec::<String>::new());
}

#[test]
fn backups_rotate_and_skip_identical() {
    let s = Setup::new();
    let store = s.store();
    let target = s.path("Song.sonara");
    store
        .save(&s.project, &s.workspace, &target, None)
        .expect("first save");
    let same = store
        .save(&s.project, &target, &target, None)
        .expect("unchanged");
    assert_eq!(same.backup, None);
    for i in 0..5 {
        let res = store
            .save(
                &renamed(&s.project, &format!("v{i}")),
                &target,
                &target,
                None,
            )
            .expect("save");
        let backup = res.backup.expect("a backup");
        let name = backup
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        assert!(name.starts_with("project-20260101T120000"), "{name}");
        assert_eq!(name.len(), "project-20260101T120000000000Z.json".len());
    }
    let backups = backups_of(&target);
    assert_eq!(backups.len(), 3);
    let names: Vec<Value> = backups
        .iter()
        .map(|b| read_json(b)["name"].clone())
        .collect();
    assert_eq!(names, [json!("v1"), json!("v2"), json!("v3")]);
    let none = ProjectStore::new(s.path("w2")).with_backups(0);
    let res = none
        .save(&renamed(&s.project, "v9"), &target, &target, None)
        .expect("save");
    assert_eq!(res.backup, None);
}

#[test]
fn interrupted_save_leaves_a_stale_manifest() {
    let s = Setup::new();
    let store = s.store();
    let target = s.path("Song.sonara");
    store
        .save(&s.project, &s.workspace, &target, None)
        .expect("save");
    // A crash after project.json was replaced but before the manifest was rewritten.
    atomic_write_bytes(&target.join(PROJECT), &dumps(&renamed(&s.project, "Newer")))
        .expect("write");
    let loaded = store.open(&target, false).expect("open");
    assert_eq!(loaded.project.name, "Newer");
    assert_eq!(
        loaded.recovered_from.as_deref(),
        Some("project.json (manifest was stale after an interrupted save)")
    );
    assert_eq!(
        loaded.warnings,
        [
            "the previous save was interrupted after project.json was written; the manifest will \
          be rewritten"
        ]
    );
}

#[test]
fn a_damaged_project_recovers_the_newest_valid_backup() {
    let s = Setup::new();
    let store = s.store();
    let target = s.path("Song.sonara");
    store
        .save(&renamed(&s.project, "v1"), &s.workspace, &target, None)
        .expect("v1");
    store
        .save(&renamed(&s.project, "v2"), &target, &target, None)
        .expect("v2");
    store
        .save(&renamed(&s.project, "v3"), &target, &target, None)
        .expect("v3");
    let newest = backups_of(&target).pop().expect("a backup");
    fs::write(
        target.join("backups/project-99999999T999999999999Z.json"),
        "{broken",
    )
    .expect("write");
    fs::write(target.join(PROJECT), b"\x00garbage").expect("write");
    let loaded = store.open(&target, false).expect("open");
    assert_eq!(loaded.project.name, "v2");
    let newest_name = newest
        .file_name()
        .expect("name")
        .to_string_lossy()
        .into_owned();
    assert_eq!(loaded.recovered_from.as_deref(), Some(newest_name.as_str()));
    assert_eq!(loaded.warnings.len(), 2, "{:?}", loaded.warnings);
    assert!(
        loaded.warnings[0].starts_with("project.json is damaged: project.json is not valid JSON")
    );
    assert_eq!(
        loaded.warnings[1],
        format!("recovered from backup {newest_name}")
    );
    fs::remove_dir_all(target.join("backups")).expect("remove");
    let err = domain(store.open(&target, false));
    assert_eq!(err.code, ErrorCode::FormatError);
    assert_eq!(
        err.message,
        "project.json is missing or damaged and no valid backup exists"
    );
}

#[test]
fn manifest_problems() {
    let s = Setup::new();
    let store = s.store();
    let target = s.path("Song.sonara");
    store
        .save(&s.project, &s.workspace, &target, None)
        .expect("save");
    let manifest = read_json(&target.join(MANIFEST));
    fs::remove_file(target.join(MANIFEST)).expect("remove");
    let loaded = store.open(&target, false).expect("open");
    assert_eq!(loaded.project, s.project);
    assert_eq!(
        loaded.warnings,
        ["manifest.json is missing; it will be recreated on save"]
    );
    for junk in ["null", "[1]"] {
        fs::write(target.join(MANIFEST), junk).expect("write");
        assert_eq!(
            store.open(&target, false).expect("open").warnings,
            ["manifest is unreadable (manifest is not a JSON object); continuing without it"]
        );
    }
    fs::write(target.join(MANIFEST), "{").expect("write");
    let warnings = store.open(&target, false).expect("open").warnings;
    assert!(
        warnings[0].starts_with("manifest is unreadable ("),
        "{warnings:?}"
    );
    let mut invalid = manifest.clone();
    invalid["format"] = json!("other");
    fs::write(target.join(MANIFEST), invalid.to_string()).expect("write");
    assert_eq!(
        store.open(&target, false).expect("open").warnings,
        [
            "manifest is unreadable ([FORMAT_ERROR] manifest-v1.schema.json: format: \
          'sonara.manifest' was expected); continuing without it"
        ]
    );

    let mut future = manifest.clone();
    future["required_features"] = json!(["core", "time-travel"]);
    fs::write(target.join(MANIFEST), future.to_string()).expect("write");
    let err = domain(store.open(&target, false));
    assert_eq!(err.code, ErrorCode::Unsupported);
    assert_eq!(
        err.message,
        "project requires features this version does not support: ['time-travel']"
    );
    assert_eq!(err.details["features"], json!(["time-travel"]));

    let mut newer = manifest.clone();
    newer["format_version"] = json!(99);
    fs::write(target.join(MANIFEST), newer.to_string()).expect("write");
    let err = domain(store.open(&target, false));
    assert_eq!(err.code, ErrorCode::Unsupported);
    assert_eq!(
        err.message,
        "project was saved with a newer container format"
    );

    let mut other = manifest;
    other["project_id"] = json!("prj_other");
    fs::write(target.join(MANIFEST), other.to_string()).expect("write");
    assert_eq!(
        store.open(&target, false).expect("open").warnings,
        ["manifest project id does not match project.json"]
    );
}

#[test]
fn a_newer_schema_is_rejected_not_replaced_by_a_backup() {
    let s = Setup::new();
    let store = s.store();
    let target = s.path("Song.sonara");
    store
        .save(&s.project, &s.workspace, &target, None)
        .expect("save");
    store
        .save(&renamed(&s.project, "Later"), &target, &target, None)
        .expect("leaves a backup");
    let mut doc = read_json(&target.join(PROJECT));
    doc["schema_version"] = json!(7);
    fs::write(target.join(PROJECT), doc.to_string()).expect("write");
    fs::remove_file(target.join(MANIFEST)).expect("remove");
    let err = domain(store.open(&target, false));
    assert_eq!(err.code, ErrorCode::Unsupported);
    assert_eq!(
        err.message,
        "project was saved by a newer Sonara (schema 7); this version reads up to 1"
    );
    assert_eq!(err.details["schema_version"], 7);
    assert!(target.join(PROJECT).exists());
}

#[test]
fn open_errors_and_layout_detection() {
    let s = Setup::new();
    let store = s.store();
    let missing = s.path("missing.sonara");
    let err = domain(store.open(&missing, false));
    assert_eq!(err.code, ErrorCode::NotFound);
    assert_eq!(
        err.message,
        format!("project {} does not exist", missing.display())
    );
    let empty = s.path("empty.sonara");
    fs::create_dir(&empty).expect("mkdir");
    assert_eq!(detect_layout(&empty), None);
    let err = domain(store.open(&empty, false));
    assert_eq!(err.code, ErrorCode::FormatError);
    assert_eq!(
        err.message,
        format!(
            "{} is not a Sonara project (no manifest/project.json)",
            empty.display()
        )
    );
    fs::write(s.path("text.sonara"), "hello").expect("write");
    assert_eq!(detect_layout(&s.path("text.sonara")), None);
    // Python's `Path()` spelling: no trailing separator or `.` segments in messages.
    let spelled = PathBuf::from(format!("{}/./missing.sonara/", s.dir.path().display()));
    assert_eq!(
        domain(store.open(&spelled, false)).message,
        format!("project {} does not exist", missing.display())
    );
}

#[test]
fn save_conflicts_and_missing_media() {
    let s = Setup::new();
    let store = s.store();
    let a_file = s.path("file.sonara");
    fs::write(&a_file, "x").expect("write");
    let err = domain(store.save(&s.project, &s.workspace, &a_file, Some(Layout::Bundle)));
    assert_eq!(err.code, ErrorCode::Conflict);
    assert_eq!(
        err.message,
        format!("{} exists and is not a project bundle", a_file.display())
    );
    let a_dir = s.path("dir.sonara");
    fs::create_dir(&a_dir).expect("mkdir");
    let err = domain(store.save(&s.project, &s.workspace, &a_dir, Some(Layout::Archive)));
    assert_eq!(err.code, ErrorCode::Conflict);
    assert_eq!(
        err.message,
        format!(
            "{} is a bundle directory; cannot overwrite it with an archive",
            a_dir.display()
        )
    );
    fs::remove_file(s.workspace.join(AUDIO_REL)).expect("remove");
    let err = domain(store.save(&s.project, &s.workspace, &s.path("x.sonara"), None));
    assert_eq!(err.code, ErrorCode::MediaMissing);
    assert_eq!(
        err.message,
        "cannot save: 1 embedded media file(s) are missing from the workspace"
    );
    assert_eq!(err.details["missing"], json!(["vox.wav"]));
}

#[test]
fn missing_media_is_reported_on_open() {
    let s = Setup::new();
    let store = s.store();
    let target = s.path("Song.sonara");
    store
        .save(&s.project, &s.workspace, &target, None)
        .expect("save");
    fs::remove_file(target.join(AUDIO_REL)).expect("remove");
    let loaded = store.open(&target, false).expect("open");
    assert_eq!(loaded.missing_assets, ["med_vox"]);
    assert_eq!(
        loaded.warnings,
        ["1 media file(s) are missing; relink them to restore audio"]
    );
}

// -- archives -------------------------------------------------------------------------------

#[test]
fn archive_save_and_open() {
    let s = Setup::new();
    let store = s.store();
    let target = s.path("Song.sonara");
    let res = store
        .save(&s.project, &s.workspace, &target, Some(Layout::Archive))
        .expect("save");
    assert_eq!(res.layout, Layout::Archive);
    assert_eq!(res.workspace, s.workspace);
    assert_eq!(res.backup, None);
    assert_eq!(detect_layout(&target), Some(Layout::Archive));
    {
        let mut zip = ZipArchive::new(File::open(&target).expect("archive")).expect("ZIP");
        let names: Vec<String> = zip
            .file_names()
            .map(|n| n.expect("name").into_owned())
            .collect();
        for name in [MANIFEST, PROJECT, AUDIO_REL] {
            assert!(names.iter().any(|n| n == name), "{names:?}");
        }
        assert!(!names.iter().any(|n| n.starts_with("backups")
            || n.starts_with(".autosave")
            || n.starts_with("cache")));
        let index = zip.index_for_name(AUDIO_REL).expect("audio member");
        assert_eq!(
            zip.by_index_raw(index).expect("member").compression(),
            CompressionMethod::Stored
        );
        let index = zip.index_for_name(PROJECT).expect("project member");
        assert_eq!(
            zip.by_index_raw(index).expect("member").compression(),
            CompressionMethod::Deflated
        );
    }
    let loaded = store.open(&target, false).expect("open");
    assert_eq!(loaded.project, s.project);
    assert_eq!(loaded.layout, Layout::Archive);
    assert_ne!(loaded.workspace, target);
    assert_eq!(loaded.workspace.parent(), Some(store.workspaces_root()));
    let name = loaded
        .workspace
        .file_name()
        .expect("name")
        .to_string_lossy()
        .into_owned();
    assert!(name.starts_with("pkg-") && name.len() == 16, "{name}");
    assert!(loaded.workspace.join(AUDIO_REL).exists());
    let info = read_json(&loaded.workspace.join("workspace.json"));
    assert_eq!(info["kind"], "extracted");
    assert_eq!(info["source"], target.to_string_lossy().as_ref());
    // Re-saving from the extraction workspace back into the archive keeps the layout and is
    // atomic.
    store
        .save(
            &renamed(&loaded.project, "Edited"),
            &loaded.workspace,
            &target,
            None,
        )
        .expect("re-save");
    assert_eq!(
        store.open(&target, false).expect("open").project.name,
        "Edited"
    );
    let leftovers: Vec<String> = fs::read_dir(s.dir.path())
        .expect("list")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| {
            n.strip_prefix(".Song.sonara.")
                .is_some_and(|rest| rest.strip_suffix(".tmp").is_some())
        })
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn archive_autosave_recovery_keeps_new_media() {
    let s = Setup::new();
    let store = s.store();
    let target = s.path("Song.sonara");
    store
        .save(&s.project, &s.workspace, &target, Some(Layout::Archive))
        .expect("save");
    let loaded = store.open(&target, false).expect("open");
    // Import new media into the extraction workspace, autosave, then "crash" and reopen.
    let new_rel = "media/audio/med_new-new.wav";
    write_wav(&loaded.workspace.join(new_rel), FRAMES, 7);
    let edited = loaded
        .project
        .with_asset(asset_for(&loaded.workspace, new_rel, "med_new"))
        .expect("asset");
    store
        .write_autosave(&edited, &loaded.workspace)
        .expect("autosave");
    let reopened = store.open(&target, false).expect("reopen");
    assert!(reopened.autosave_available);
    assert_eq!(reopened.project, s.project);
    let recovered = store.open(&target, true).expect("recover");
    assert!(recovered.project.find_asset("med_new").is_some());
    assert_eq!(recovered.missing_assets, Vec::<String>::new());
    assert!(recovered.workspace.join(new_rel).exists());
    assert!(recovered.workspace.join(AUDIO_REL).exists());
    let kept: Vec<String> = fs::read_dir(store.workspaces_root())
        .expect("list")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("autosave-"))
        .collect();
    assert!(kept.is_empty(), "{kept:?}");
}

/// A ZIP with STORED members, optionally a symbolic link to `/etc/passwd`.
fn make_zip(path: &Path, members: &[(&str, &[u8])], symlink: Option<&str>) -> PathBuf {
    let mut zip = ZipWriter::new(File::create(path).expect("create"));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, data) in members {
        zip.start_file(*name, options).expect("member");
        zip.write_all(data).expect("data");
    }
    if let Some(link) = symlink {
        zip.add_symlink(link, "/etc/passwd", options).expect("link");
    }
    zip.finish().expect("finish");
    path.to_path_buf()
}

#[test]
fn safe_extract_rejects_unsafe_members() {
    for member in [
        "../evil.txt",
        "/abs/evil.txt",
        "media/../../evil.txt",
        "unknown/top.txt",
        "media\\win.txt",
        "C:/windows.txt",
    ] {
        let dir = tempfile::tempdir().expect("temp dir");
        let archive = make_zip(
            &dir.path().join("bad.sonara"),
            &[(PROJECT, b"{}"), (member, b"x")],
            None,
        );
        let dest = dir.path().join("out");
        fs::create_dir(&dest).expect("mkdir");
        let err = domain(safe_extract(&archive, &dest));
        assert_eq!(err.code, ErrorCode::FormatError, "{member}");
        let quoted = if member.contains('\\') {
            "'media\\\\win.txt'".to_owned()
        } else {
            format!("'{member}'")
        };
        assert_eq!(
            err.message,
            format!("archive member {quoted} is not allowed"),
            "{member}"
        );
        assert_eq!(err.details["member"], member);
        assert!(!dir.path().join("evil.txt").exists());
    }
}

#[test]
fn safe_extract_rejects_symlinks_and_bad_zips() {
    let dir = tempfile::tempdir().expect("temp dir");
    let archive = make_zip(
        &dir.path().join("link.sonara"),
        &[(PROJECT, b"{}")],
        Some("media/link"),
    );
    let dest = dir.path().join("out");
    fs::create_dir(&dest).expect("mkdir");
    let err = domain(safe_extract(&archive, &dest));
    assert_eq!(err.code, ErrorCode::FormatError);
    assert_eq!(err.message, "archive member 'media/link' is not allowed");
    fs::write(dir.path().join("notzip.sonara"), b"PK\x03\x04garbage").expect("write");
    let err = domain(safe_extract(&dir.path().join("notzip.sonara"), &dest));
    assert_eq!(err.code, ErrorCode::FormatError);
    assert_eq!(err.message, "notzip.sonara is not a valid project archive");
}

#[test]
fn safe_extract_accepts_the_valid_layout() {
    let dir = tempfile::tempdir().expect("temp dir");
    let archive = make_zip(
        &dir.path().join("ok.sonara"),
        &[
            (MANIFEST, b"{}"),
            (PROJECT, b"{}"),
            ("media/audio/a.wav", b"RIFF"),
            ("presets/p.json", b"{}"),
            ("thumbnails/t.png", b"png"),
        ],
        None,
    );
    let dest = dir.path().join("out");
    fs::create_dir(&dest).expect("mkdir");
    safe_extract(&archive, &dest).expect("extract");
    assert_eq!(
        fs::read(dest.join("media/audio/a.wav")).expect("member"),
        b"RIFF"
    );
    assert!(dest.join("presets/p.json").exists());
    assert!(dest.join("thumbnails/t.png").exists());
}

#[test]
fn a_corrupt_archive_open_cleans_up() {
    let s = Setup::new();
    let store = s.store();
    let archive = make_zip(
        &s.path("evil.sonara"),
        &[(PROJECT, b"{}"), ("../x", b"x")],
        None,
    );
    assert_eq!(code(store.open(&archive, false)), ErrorCode::FormatError);
    let leftovers: Vec<String> = fs::read_dir(store.workspaces_root())
        .map(|entries| {
            entries
                .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("pkg-"))
                .collect()
        })
        .unwrap_or_default();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

// -- autosave -------------------------------------------------------------------------------

#[test]
fn autosave_cycle() {
    let s = Setup::new();
    let store = s.store();
    let target = s.path("Song.sonara");
    store
        .save(&s.project, &s.workspace, &target, None)
        .expect("save");
    assert_eq!(store.autosave_info(&target), None);
    let changed = renamed(&s.project, "Unsaved");
    store.write_autosave(&changed, &target).expect("autosave");
    let info = store.autosave_info(&target).expect("info");
    assert_eq!(
        info.keys().map(String::as_str).collect::<Vec<_>>(),
        ["saved_at", "sha256", "project_id", "name"]
    );
    assert_eq!(info["name"], "Unsaved");
    assert_eq!(info["project_id"], s.project.id.as_str());
    assert_eq!(info["saved_at"], "2026-01-01T12:00:00Z");
    assert_eq!(info["sha256"], sha256_bytes(&dumps(&changed)));
    assert_eq!(store.read_autosave(&target).expect("read"), changed);
    let plain = store.open(&target, false).expect("open");
    assert!(plain.autosave_available);
    assert_eq!(plain.project, s.project);
    let recovered = store.open(&target, true).expect("recover");
    assert_eq!(recovered.project, changed);
    assert_eq!(
        recovered.recovered_from.as_deref(),
        Some("autosave 2026-01-01T12:00:00Z")
    );
    assert_eq!(
        recovered.warnings,
        ["restored unsaved changes from the autosave"]
    );
    // An autosave identical to the saved project is not offered.
    store.write_autosave(&s.project, &target).expect("autosave");
    assert!(!store.open(&target, false).expect("open").autosave_available);
    // Saving discards the autosave.
    store.write_autosave(&changed, &target).expect("autosave");
    store.save(&changed, &target, &target, None).expect("save");
    assert_eq!(store.autosave_info(&target), None);
}

#[test]
fn recoverable_sessions() {
    let s = Setup::new();
    let store = s.store();
    assert_eq!(store.recoverable_sessions().expect("list"), Vec::new());
    let mut ids = SequentialIds::new(0);
    let project = Project::create(&mut ids, "Scratch", 48_000, 120.0).expect("project");
    let ws = store.scratch_workspace(&project.id).expect("scratch");
    assert!(ws.join("media").join("audio").is_dir());
    assert_eq!(
        String::from_utf8(fs::read(ws.join("workspace.json")).expect("info")).expect("ASCII"),
        format!(
            r#"{{"kind": "scratch", "project_id": "{}", "created": "2026-01-01T12:00:00Z"}}"#,
            project.id
        )
    );
    assert_eq!(store.scratch_workspace(&project.id).expect("again"), ws);
    store.write_autosave(&project, &ws).expect("autosave");
    let sessions = store.recoverable_sessions().expect("list");
    assert_eq!(sessions.len(), 1);
    assert_eq!(
        sessions[0].keys().map(String::as_str).collect::<Vec<_>>(),
        ["workspace", "saved_at", "sha256", "project_id", "name"]
    );
    assert_eq!(sessions[0]["workspace"], ws.to_string_lossy().as_ref());
    assert_eq!(sessions[0]["name"], "Scratch");
    store.discard_autosave(&ws);
    assert_eq!(store.recoverable_sessions().expect("list"), Vec::new());
}

#[test]
fn autosave_info_is_written_like_python_json_dumps() {
    let s = Setup::new();
    let store = s.store();
    let ws = s.path("ascii");
    let project = renamed(&s.project, "Třináctá ✓");
    store.write_autosave(&project, &ws).expect("autosave");
    let text =
        String::from_utf8(fs::read(ws.join(".autosave/info.json")).expect("info")).expect("ASCII");
    let expected = format!(
        "{{\"saved_at\": \"2026-01-01T12:00:00Z\", \"sha256\": \"{}\", \"project_id\": \"{}\", \
         \"name\": \"T\\u0159in\\u00e1ct\\u00e1 \\u2713\"}}",
        sha256_bytes(&dumps(&project)),
        project.id
    );
    assert_eq!(text, expected);
}

// -- integrity ------------------------------------------------------------------------------

#[test]
fn verify_and_unused_media() {
    let s = Setup::new();
    let store = s.store();
    let ws = &s.workspace;
    let report = store.verify(&s.project, ws, true).expect("verify");
    assert!(report.ok);
    assert_eq!(
        report.to_json(),
        json!({"ok": true, "missing": [], "size_mismatch": [], "hash_mismatch": [],
               "unreferenced_files": [], "routing_issues": []})
    );
    let stray = write_wav(&ws.join("media/audio/stray.wav"), FRAMES, 3);
    let unused = store.unused_media(&s.project, ws, std::iter::empty::<&str>());
    assert_eq!(unused, std::slice::from_ref(&stray));
    assert_eq!(
        store.unused_media(&s.project, ws, ["media/audio/stray.wav"]),
        Vec::<PathBuf>::new()
    );
    assert_eq!(
        store
            .verify(&s.project, ws, true)
            .expect("verify")
            .unreferenced_files,
        [Path::new("media")
            .join("audio")
            .join("stray.wav")
            .to_string_lossy()
            .into_owned()]
    );
    // Same size, different content: a hash mismatch, found only when hashing.
    let mut data = fs::read(ws.join(AUDIO_REL)).expect("read");
    *data.last_mut().expect("non-empty") ^= 0xFF;
    fs::write(ws.join(AUDIO_REL), &data).expect("write");
    assert_eq!(
        store
            .verify(&s.project, ws, true)
            .expect("verify")
            .hash_mismatch,
        ["med_vox"]
    );
    assert!(store.verify(&s.project, ws, false).expect("verify").ok);
    fs::write(ws.join(AUDIO_REL), b"short").expect("write");
    let report = store.verify(&s.project, ws, true).expect("verify");
    assert_eq!(report.size_mismatch, ["med_vox"]);
    assert!(!report.ok);
    fs::remove_file(ws.join(AUDIO_REL)).expect("remove");
    assert_eq!(
        store.verify(&s.project, ws, true).expect("verify").missing,
        ["med_vox"]
    );
    fs::remove_file(&stray).expect("remove");
    let broken = s
        .project
        .try_edit(|d| {
            for track in &mut d.tracks {
                if track.id == "trk_vox" {
                    *track = track.edit(|t| t.output_id = Some("trk_gone".into()))?;
                }
            }
            Ok(())
        })
        .expect("broken routing");
    assert_ne!(
        store
            .verify(&broken, ws, false)
            .expect("verify")
            .routing_issues,
        Vec::<String>::new()
    );
}

#[test]
fn linked_assets_resolve_outside() {
    let s = Setup::new();
    let external = write_wav(&s.path("external/take.wav"), FRAMES, 5);
    let linked = asset_for(&s.workspace, AUDIO_REL, "med_vox")
        .edit(|a| {
            a.path = external.to_string_lossy().into_owned();
            a.location = MediaLocation::Linked;
        })
        .expect("linked asset");
    assert_eq!(asset_path(&linked, &s.workspace), external);
    let project = s
        .project
        .edit(|d| d.assets = vec![linked])
        .expect("project");
    assert!(required_features(&project).contains(&"linked-media".to_owned()));
    let store = ProjectStore::new(s.path("w"));
    let target = s.path("Linked.sonara");
    store
        .save(&project, &s.workspace, &target, None)
        .expect("save");
    assert!(!target.join("media/audio/take.wav").exists());
    assert_eq!(
        store.open(&target, false).expect("open").missing_assets,
        Vec::<String>::new()
    );
}

#[test]
fn the_manifest_escapes_non_ascii_like_python() {
    let dir = tempfile::tempdir().expect("temp dir");
    let workspace = dir.path().join("ws");
    let embedded_rel = "media/audio/med_a-Třináctá ✓.wav";
    fs::create_dir_all(workspace.join("media/audio")).expect("mkdir");
    fs::write(workspace.join(embedded_rel), b"RIFFdata").expect("write");
    let asset = |id: &str, path: &str, location, size, sha256: Option<&str>| {
        MediaAsset::new(MediaAssetData {
            id: id.into(),
            kind: MediaKind::Audio,
            name: "a.wav".into(),
            path: path.into(),
            location,
            size_bytes: size,
            sha256: sha256.map(str::to_owned),
            sample_rate: 48_000,
            channels: 2,
            frames: 2,
            format: "WAV".into(),
            subtype: "PCM_16".into(),
            original_path: None,
            relative_hint: None,
            metadata: JsonMap::new(),
            extra: JsonMap::new(),
        })
        .expect("asset")
    };
    let sha = "c8f2762e738e8fd7ac652eb5ff052ebf2e3b62c00243db99e8835ab12ddfc5bc";
    assert_eq!(sha256_bytes(b"RIFFdata"), sha);
    let mut ids = SequentialIds::new(0);
    // Assets are listed by ID, whatever their order in the project.
    let project = Project::create(&mut ids, "Ü", 48_000, 120.0)
        .and_then(|p| {
            p.with_asset(asset(
                "med_b",
                "/nonexistent/Ünï 😀\u{7f}.wav",
                MediaLocation::Linked,
                7,
                None,
            ))
        })
        .and_then(|p| {
            p.with_asset(asset(
                "med_a",
                embedded_rel,
                MediaLocation::Embedded,
                99,
                Some(sha),
            ))
        })
        .expect("project");
    let store = ProjectStore::new(dir.path().join("w"))
        .with_clock(|| UNIX_EPOCH + Duration::from_secs(FROZEN_UNIX_SECONDS));
    let target = dir.path().join("Ascii.sonara");
    store
        .save(&project, &workspace, &target, None)
        .expect("save");
    let project_bytes = fs::read(target.join(PROJECT)).expect("project");
    // Python: json.dumps(manifest, indent=1) + "\n" for the same values. The embedded size is
    // the file's (8), the linked one falls back to `size_bytes` (7).
    let expected = format!(
        "{{\n \"format\": \"sonara.manifest\",\n \"format_version\": 1,\n \"sonara_version\": \
         \"{version}\",\n \"project_id\": \"{id}\",\n \"saved\": \"2026-01-01T12:00:00Z\",\n \
         \"required_features\": [\n  \"core\",\n  \"linked-media\"\n ],\n \"project_file\": \
         {{\n  \"path\": \"project.json\",\n  \"sha256\": \"{project_sha}\",\n  \"size\": \
         {size}\n }},\n \"assets\": [\n  {{\n   \"id\": \"med_a\",\n   \"path\": \
         \"media/audio/med_a-T\\u0159in\\u00e1ct\\u00e1 \\u2713.wav\",\n   \"location\": \
         \"embedded\",\n   \"size\": 8,\n   \"sha256\": \"{sha}\",\n   \"kind\": \"audio\"\n  \
         }},\n  {{\n   \"id\": \"med_b\",\n   \"path\": \"/nonexistent/\\u00dcn\\u00ef \
         \\ud83d\\ude00\\u007f.wav\",\n   \"location\": \"linked\",\n   \"size\": 7,\n   \
         \"sha256\": null,\n   \"kind\": \"audio\"\n  }}\n ],\n \"compatibility\": {{\n  \
         \"min_reader_version\": 1\n }}\n}}\n",
        version = env!("CARGO_PKG_VERSION"),
        id = project.id,
        project_sha = sha256_bytes(&project_bytes),
        size = project_bytes.len(),
    );
    assert_eq!(
        String::from_utf8(fs::read(target.join(MANIFEST)).expect("manifest")).expect("ASCII"),
        expected
    );
}

// -- documents (test_serializer.py) ---------------------------------------------------------

#[test]
fn documents_validate_against_the_schema() {
    let s = Setup::new();
    validate_document(&project_to_value(&s.project), Schema::Project).expect("rich");
    let mut ids = SequentialIds::new(0);
    let empty = Project::create(&mut ids, "Empty", 48_000, 120.0).expect("project");
    validate_document(&project_to_value(&empty), Schema::Project).expect("empty");
}

#[test]
fn the_schema_rejects_malformed_documents() {
    let s = Setup::new();
    let mutations: [fn(&mut Value); 10] = [
        |d| {
            d.as_object_mut().expect("object").shift_remove("master");
        },
        |d| d["format"] = json!("other"),
        |d| d["settings"]["sample_rate"] = json!(12345),
        |d| d["tempo_map"] = json!([]),
        |d| d["tracks"][2]["clips"][0]["notes"][0]["pitch"] = json!(200),
        |d| d["tracks"][2]["kind"] = json!("banjo"),
        |d| {
            d["tracks"][3]["clips"][0]
                .as_object_mut()
                .expect("clip")
                .shift_remove("asset_id");
        },
        |d| d["tracks"][2]["id"] = json!("has space"),
        |d| d["assets"][0]["sha256"] = json!("xyz"),
        |d| d["tracks"][2]["mixer"]["pan"] = json!(3),
    ];
    for (i, mutate) in mutations.iter().enumerate() {
        let mut doc = project_to_value(&s.project);
        mutate(&mut doc);
        let err = validate_document(&doc, Schema::Project).expect_err("malformed");
        assert_eq!(err.code, ErrorCode::FormatError, "mutation {i}");
    }
}

#[test]
fn a_minimal_document_gets_the_defaults() {
    let doc = json!({
        "format": "sonara.project", "schema_version": 1, "id": "prj_x", "name": "Min",
        "settings": {"sample_rate": 48000}, "tempo_map": [{"beat": 0, "bpm": 100}],
        "time_signatures": [{"bar": 0, "numerator": 3, "denominator": 4}],
        "tracks": [], "master": {"id": "master", "name": "Master", "kind": "master"},
    });
    validate_document(&doc, Schema::Project).expect("valid");
    let project = project_from_value(doc.as_object().expect("object")).expect("project");
    assert!((project.tempo_map.points()[0].bpm - 100.0).abs() < f64::EPSILON);
    assert_eq!(project.time_signatures.points()[0].numerator, 3);
    assert_eq!(project.master.output_id, None);
    assert_eq!(project.assets.len(), 0);
}

#[test]
fn required_features_follow_the_content() {
    let s = Setup::new();
    let mut ids = SequentialIds::new(0);
    let plain = Project::create(&mut ids, "x", 48_000, 120.0).expect("project");
    assert_eq!(required_features(&plain), ["core"]);
    let features = required_features(&s.project);
    for feature in [
        "core",
        "audio-clips",
        "midi-clips",
        "sends",
        "automation",
        "sidechain",
        "tempo-ramps",
        "sections",
        "folders",
    ] {
        assert!(features.contains(&feature.to_owned()), "{feature}");
    }
    assert!(!features.contains(&"linked-media".to_owned()));
    let mut sorted = features.clone();
    sorted.sort();
    assert_eq!(features, sorted);
}

#[test]
fn time_is_read_from_the_injected_clock() {
    let s = Setup::new();
    let store = ProjectStore::new(s.path("w"))
        .with_clock(|| SystemTime::UNIX_EPOCH + Duration::from_secs(31_536_001));
    let target = s.path("Clock.sonara");
    store
        .save(&s.project, &s.workspace, &target, None)
        .expect("save");
    assert_eq!(
        read_json(&target.join(MANIFEST))["saved"],
        "1971-01-01T00:00:01Z"
    );
}
