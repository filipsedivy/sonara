//! The project service (`tests/project_format/test_store.py`, "project service" section), and
//! the service results the golden workflows record for `create_project` and `save_project`.
//!
//! The rich project is the prototype's `build_rich_project` document, shared with the store
//! tests of `sonara-project` (`tests/fixtures/rich.project.json` there).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use sonara_app::clock::WallClock;
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::error::{AppError, AppResult};
use sonara_app::ids::IdSource;
use sonara_app::projects::{MediaProbe, NewProject, ProbedAudio, normalize_project_path};
use sonara_app::settings::Settings;
use sonara_core::commands::project::{AddAsset, RenameProject};
use sonara_core::commands::track::CreateTrack;
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::ids::{self, IdGenerator as _, SequentialIds};
use sonara_core::model::JsonMap;
use sonara_core::model::media::{MediaAsset, MediaAssetData, MediaKind, MediaLocation};
use sonara_core::model::project::Project;
use sonara_core::model::track::TrackKind;
use sonara_core::session::ProjectLocation;
use sonara_project::atomic::sha256_file;
use sonara_project::serializer::{asset_to_value, dumps, loads, project_from_value};
use sonara_project::store::{Layout, PROJECT, detect_layout};
use sonara_testkit::Golden;

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
fn asset_for(workspace: &Path) -> MediaAsset {
    let path = workspace.join(AUDIO_REL);
    let mut metadata = JsonMap::new();
    metadata.insert("title".into(), json!("Vox"));
    MediaAsset::new(MediaAssetData {
        id: "med_vox".into(),
        kind: MediaKind::Audio,
        name: "vox.wav".into(),
        path: AUDIO_REL.into(),
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
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sonara-project/tests/fixtures/rich.project.json");
    let mut doc = loads(&fs::read(fixture).expect("fixture")).expect("JSON object");
    doc.insert("assets".into(), json!([asset_to_value(asset)]));
    project_from_value(&doc).expect("rich project")
}

/// Reads WAV headers: the test stand-in for the media service's probe.
struct WavProbe;

impl MediaProbe for WavProbe {
    fn probe(&self, path: &Path) -> AppResult<ProbedAudio> {
        let reader = hound::WavReader::open(path).map_err(|err| {
            AppError::from(DomainError::new(ErrorCode::FormatError, err.to_string()))
        })?;
        let spec = reader.spec();
        Ok(ProbedAudio {
            frames: u64::from(reader.duration()),
            sample_rate: spec.sample_rate,
            channels: u32::from(spec.channels),
        })
    }
}

/// The frozen time, advancing a microsecond per reading so that backup names stay distinct.
fn ticking_clock() -> WallClock {
    let ticks = AtomicU64::new(0);
    Arc::new(move || {
        UNIX_EPOCH
            + Duration::from_secs(FROZEN_UNIX_SECONDS)
            + Duration::from_micros(ticks.fetch_add(1, Ordering::Relaxed))
    })
}

/// The prototype's `app` fixture: workspaces in `dir`, default settings kept in memory,
/// sequential IDs, the frozen clock, and the WAV probe unless `probe` is false.
fn app(dir: &Path, probe: bool) -> AppContext {
    AppContext::create(AppOptions {
        workspaces: Some(dir.join("workspaces")),
        settings: Some(Settings::default()),
        persist_settings: false,
        audio_backend: Some("null".into()),
        ids: Some(IdSource::new(Box::new(SequentialIds::new(0)))),
        wall_clock: Some(ticking_clock()),
        media_probe: probe.then(|| Arc::new(WavProbe) as Arc<dyn MediaProbe>),
        ..AppOptions::default()
    })
    .expect("context")
}

fn named(name: &str) -> NewProject {
    NewProject {
        name: name.into(),
        ..NewProject::default()
    }
}

fn add_vox(app: &AppContext, workspace: &Path) {
    write_wav(&workspace.join(AUDIO_REL), FRAMES, 0);
    let mut session = app.session().lock();
    session
        .execute(
            &AddAsset {
                asset: asset_for(workspace),
            },
            false,
            "user",
        )
        .expect("asset added");
}

fn execute(app: &AppContext, command: &dyn sonara_core::commands::Command) {
    app.session()
        .lock()
        .execute(command, false, "user")
        .expect("command applies");
}

fn location(app: &AppContext) -> ProjectLocation {
    app.session()
        .lock()
        .require_location()
        .expect("open project")
        .clone()
}

fn is_dirty(app: &AppContext) -> bool {
    app.session().lock().is_dirty()
}

fn code<T: std::fmt::Debug>(result: AppResult<T>) -> ErrorCode {
    result.expect_err("an error").code()
}

fn audio_track(app: &AppContext, name: &str) -> CreateTrack {
    let mut source = app.ids();
    CreateTrack {
        name: name.into(),
        kind: TrackKind::Audio,
        index: None,
        color: None,
        output_id: ids::MASTER_ID.into(),
        instrument_plugin: None,
        instrument_preset: None,
        parent_id: None,
        track_id: source.new_id(ids::TRACK),
        instrument_id: source.new_id(ids::DEVICE),
    }
}

// -- the prototype's service tests ----------------------------------------------------------

#[test]
fn service_new_save_save_as() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tmp = fs::canonicalize(dir.path()).expect("resolved");
    let app = app(&tmp, true);
    let svc = app.projects();
    let project = svc
        .new_project(&NewProject {
            name: "Demo".into(),
            bpm: 100.0,
            time_signature: (3, 4),
            template: "electronic".into(),
            length_beats: Some(48.0),
            ..NewProject::default()
        })
        .expect("created");
    let loc = location(&app);
    assert!(loc.path.is_none() && loc.workspace.exists());
    assert!(
        loc.workspace
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("new-"))
    );
    let names: Vec<&str> = project.tracks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names[..4], ["Drums", "Bass", "Chords", "Lead"]);
    assert_eq!(project.time_signatures.points()[0].numerator, 3);
    assert_eq!(project.settings.length_beats, Some(48.0));
    assert!(!app.session().lock().history().can_undo());
    // Media that lives only in the scratch workspace.
    let scratch = loc.workspace;
    add_vox(&app, &scratch);
    assert_eq!(code(svc.save(None, None)), ErrorCode::InvalidArgument);
    let res = svc.save(Some(&tmp.join("Demo")), None).expect("saved");
    assert_eq!(res.path, tmp.join("Demo.sonara"));
    assert_eq!(res.layout, Layout::Bundle);
    assert!(res.path.join(AUDIO_REL).exists());
    assert!(
        !scratch.exists(),
        "scratch workspace removed after the first save"
    );
    let new_loc = location(&app);
    assert_eq!(new_loc.path.as_ref(), Some(&res.path));
    assert_eq!(new_loc.workspace, res.path);
    assert!(!is_dirty(&app));
    assert!(
        svc.recent_projects()
            .contains(&res.path.to_string_lossy().into_owned())
    );
    // Edit and save in place: backup created.
    execute(
        &app,
        &RenameProject {
            name: "Demo 2".into(),
        },
    );
    assert!(is_dirty(&app));
    let res2 = svc.save(None, None).expect("saved in place");
    assert!(res2.backup.is_some() && !is_dirty(&app));
    // Save As moves the open project; save_copy does not.
    let copy = svc
        .save_copy(&tmp.join("Copy.sonara"), false)
        .expect("copied");
    assert_eq!(location(&app).path.as_ref(), Some(&res.path));
    assert!(copy.path.join(AUDIO_REL).exists());
    let moved = svc
        .save(Some(&tmp.join("Moved.sonara")), None)
        .expect("saved as");
    assert_eq!(location(&app).path.as_ref(), Some(&moved.path));
    let reopened = svc.open_project(&moved.path, false).expect("opened");
    assert_eq!(reopened.project.name, "Demo 2");
    assert_eq!(svc.last_load().map(|l| l.path), Some(moved.path.clone()));
    let err = svc.save_copy(&moved.path, false).expect_err("itself");
    assert_eq!(err.code(), ErrorCode::Conflict);
    assert_eq!(
        err.to_string(),
        "[CONFLICT] the copy target is the open project itself"
    );
}

#[test]
fn service_packaged_round_trip() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tmp = fs::canonicalize(dir.path()).expect("resolved");
    let app = app(&tmp, true);
    let svc = app.projects();
    svc.new_project(&named("Pkg")).expect("created");
    add_vox(&app, &location(&app).workspace);
    let res = svc
        .save(Some(&tmp.join("Pkg.sonara")), Some(true))
        .expect("saved");
    assert_eq!(res.layout, Layout::Archive);
    assert!(location(&app).packaged);
    let loaded = svc.open_project(&res.path, false).expect("opened");
    assert_eq!(loaded.layout, Layout::Archive);
    assert!(!is_dirty(&app));
    let track = audio_track(&app, "Added");
    execute(&app, &track);
    svc.save(None, None).expect("stays an archive");
    assert_eq!(detect_layout(&res.path), Some(Layout::Archive));
    let reopened = svc.open_project(&res.path, false).expect("reopened");
    let names: Vec<&str> = reopened
        .project
        .tracks
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(names, ["Added"]);
    assert!(reopened.project.find_asset("med_vox").is_some());
    assert_eq!(reopened.missing_assets, Vec::<String>::new());
}

#[test]
fn service_close_and_autosave() {
    let dir = tempfile::tempdir().expect("temp dir");
    let app = app(dir.path(), true);
    let svc = app.projects();
    svc.new_project(&named("Close")).expect("created");
    assert_eq!(svc.autosave().expect("no error"), None, "clean");
    execute(
        &app,
        &RenameProject {
            name: "Dirty".into(),
        },
    );
    let path = svc.autosave().expect("written").expect("dirty");
    assert!(path.exists());
    let workspace = location(&app).workspace;
    let err = svc.close(false).expect_err("unsaved changes");
    assert_eq!(err.code(), ErrorCode::ConfirmationRequired);
    assert_eq!(
        err.to_string(),
        "[CONFIRMATION_REQUIRED] the project has unsaved changes"
    );
    svc.close(true).expect("discarded");
    assert!(app.session().project().is_none());
    svc.close(false).expect("no project: no-op");
    // The scratch workspace of a dirty project survives for crash recovery.
    assert!(workspace.exists());
    let recovered = svc.recover_session(&workspace).expect("recovered");
    assert_eq!(recovered.name, "Dirty");
    assert!(is_dirty(&app));
    assert_eq!(location(&app).workspace, workspace);
}

#[test]
fn closing_a_clean_scratch_project_removes_its_workspace() {
    let dir = tempfile::tempdir().expect("temp dir");
    let app = app(dir.path(), true);
    let svc = app.projects();
    svc.new_project(&named("First")).expect("created");
    let first = location(&app).workspace;
    svc.new_project(&named("Second")).expect("replaced");
    assert!(!first.exists(), "a clean scratch workspace is discarded");
    let second = location(&app).workspace;
    svc.close(false).expect("clean");
    assert!(!second.exists());
}

#[test]
fn service_relink_and_unused_media() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tmp = fs::canonicalize(dir.path()).expect("resolved");
    let app = app(&tmp, true);
    let svc = app.projects();
    svc.new_project(&named("Media")).expect("created");
    let ws = location(&app).workspace;
    add_vox(&app, &ws);
    fs::remove_file(ws.join(AUDIO_REL)).expect("removed");
    assert_eq!(svc.missing_assets().expect("open"), ["med_vox"]);
    let replacement = write_wav(&tmp.join("found").join("vox.wav"), FRAMES, 0);
    let out = svc
        .relink("med_vox", &replacement, false)
        .expect("relinked");
    let asset = |app: &AppContext| {
        app.session()
            .project()
            .and_then(|p| p.find_asset("med_vox").cloned())
            .expect("asset")
    };
    assert_eq!(asset(&app).location, MediaLocation::Linked);
    assert_eq!(asset(&app).path, replacement.to_string_lossy());
    assert!(out.warnings.is_empty() && out.committed);
    assert_eq!(
        out.to_json(),
        json!({"asset_id": "med_vox", "path": replacement.to_string_lossy(),
               "location": "linked", "warnings": [], "committed": true})
    );
    assert_eq!(svc.missing_assets().expect("open"), Vec::<String>::new());
    let other = write_wav(&tmp.join("found").join("other.wav"), 48_000, 1);
    let out = svc.relink("med_vox", &other, false).expect("relinked");
    assert_eq!(
        out.warnings,
        [
            "the new file's content differs from the original (hash mismatch)",
            "the new file differs in length or sample rate; clips may shift or be cut",
        ]
    );
    assert_eq!(asset(&app).frames, 48_000);
    let embedded = svc.embed_linked_media().expect("embedded");
    assert_eq!(embedded, ["med_vox"]);
    let vox = asset(&app);
    assert_eq!(vox.location, MediaLocation::Embedded);
    assert_eq!(vox.path, "media/audio/med_vox-other.wav");
    assert!(ws.join(&vox.path).exists());
    let err = svc
        .relink("med_vox", &tmp.join("nope.wav"), false)
        .expect_err("missing file");
    assert_eq!(err.code(), ErrorCode::NotFound);
    assert_eq!(
        err.to_string(),
        format!(
            "[NOT_FOUND] {} does not exist",
            tmp.join("nope.wav").display()
        )
    );
    let err = svc
        .relink("med_missing", &other, false)
        .expect_err("unknown asset");
    assert_eq!(err.to_string(), "[NOT_FOUND] asset 'med_missing' not found");
    let stray = write_wav(&ws.join("media/audio/stray.wav"), FRAMES, 2);
    assert!(svc.unused_media().expect("open").contains(&stray));
    assert_eq!(
        svc.delete_unused_media().expect("deleted"),
        [stray.to_string_lossy()]
    );
    assert!(!stray.exists() && ws.join(&vox.path).exists());
    assert!(svc.verify(true).expect("verified").ok);
}

#[test]
fn relinking_needs_a_media_probe() {
    let dir = tempfile::tempdir().expect("temp dir");
    let app = app(dir.path(), false);
    // A service built without a probe; the context's own has `sonara-media`'s.
    let svc = sonara_app::projects::ProjectService::new(
        app.session().clone(),
        app.store().clone(),
        Arc::new(std::sync::Mutex::new(Settings::default())),
        app.ids(),
    );
    svc.new_project(&named("Media")).expect("created");
    add_vox(&app, &location(&app).workspace);
    let replacement = write_wav(&dir.path().join("found").join("vox.wav"), FRAMES, 0);
    let err = svc
        .relink("med_vox", &replacement, false)
        .expect_err("no probe");
    assert_eq!(err.code(), ErrorCode::Unsupported);
    let unchanged = app.session().project().expect("open");
    assert_eq!(
        unchanged.asset("med_vox").expect("asset").location,
        MediaLocation::Embedded
    );
}

#[test]
fn the_context_relinks_with_the_media_crates_probe() {
    let dir = tempfile::tempdir().expect("temp dir");
    let app = app(dir.path(), false);
    let svc = app.projects();
    svc.new_project(&named("Media")).expect("created");
    add_vox(&app, &location(&app).workspace);
    let longer = write_wav(&dir.path().join("found").join("vox.wav"), FRAMES * 2, 0);
    let relinked = svc.relink("med_vox", &longer, false).expect("relinked");
    assert_eq!(
        relinked.warnings,
        [
            "the new file's content differs from the original (hash mismatch)",
            "the new file differs in length or sample rate; clips may shift or be cut",
        ]
    );
    let project = app.session().project().expect("open");
    let asset = project.asset("med_vox").expect("asset");
    assert_eq!(
        (asset.frames, asset.sample_rate, asset.channels),
        (u64::from(FRAMES * 2), 48_000, 2)
    );
}

#[test]
fn service_keeps_media_reachable_by_undo() {
    let dir = tempfile::tempdir().expect("temp dir");
    let app = app(dir.path(), true);
    let svc = app.projects();
    svc.new_project(&named("Undo")).expect("created");
    add_vox(&app, &location(&app).workspace);
    app.session().lock().undo("user").expect("undone");
    assert_eq!(app.session().project().expect("open").assets.len(), 0);
    assert_eq!(svc.unused_media().expect("open"), Vec::<PathBuf>::new());
}

#[test]
fn normalize_project_path_follows_the_prototype() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tmp = fs::canonicalize(dir.path()).expect("resolved");
    // Python 3.13's `normalize_project_path` of each name.
    let cases = [
        ("a", "a.sonara"),
        ("b.SONARA", "b.SONARA"),
        ("c.wav", "c.wav.sonara"),
        ("d.", "d..sonara"),
        (".sonara", ".sonara.sonara"),
        ("x/.sonara", "x/.sonara.sonara"),
        ("dir/", "dir.sonara"),
        ("e.sonara.bak", "e.sonara.bak.sonara"),
        ("f..sonara", "f..sonara"),
        ("g/./h", "g/h.sonara"),
    ];
    for (input, expected) in cases {
        let path = normalize_project_path(&tmp.join(input)).expect("named");
        assert_eq!(path, tmp.join(expected), "{input}");
    }
    let parent = normalize_project_path(&tmp.join("sub").join("..")).expect("named");
    // `with_name` replaces the `..` itself: Python gives `<tmp>/sub/...sonara`.
    assert_eq!(parent, tmp.join("sub").join("...sonara"));
    let root = normalize_project_path(Path::new("/")).expect_err("no name");
    assert_eq!(root.code(), ErrorCode::Internal);
}

#[test]
fn open_through_service_reports_recovery() {
    let dir = tempfile::tempdir().expect("temp dir");
    let app = app(dir.path(), true);
    let ws = dir.path().join("ws");
    write_wav(&ws.join(AUDIO_REL), FRAMES, 0);
    let project = build_rich_project(&asset_for(&ws));
    let target = dir.path().join("Rich.sonara");
    let store = app.store();
    store.save(&project, &ws, &target, None).expect("saved");
    fs::write(target.join(PROJECT), b"broken").expect("damaged");
    assert!(app.projects().open_project(&target, false).is_err());
    store.save(&project, &target, &target, None).expect("saved");
    let second = project.edit(|d| d.name = "Second".into()).expect("renamed");
    store.save(&second, &target, &target, None).expect("saved");
    fs::write(target.join(PROJECT), b"broken").expect("damaged");
    let loaded = app
        .projects()
        .open_project(&target, false)
        .expect("recovered");
    assert_eq!(loaded.project, project);
    assert!(loaded.recovered_from.is_some());
    assert!(
        is_dirty(&app),
        "recovered from a backup: must be saved again"
    );
}

#[test]
fn recent_projects_list_only_existing_paths() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tmp = fs::canonicalize(dir.path()).expect("resolved");
    let app = app(&tmp, true);
    app.settings().add_recent(&tmp.join("gone.sonara"));
    app.projects().new_project(&named("R")).expect("created");
    let saved = app
        .projects()
        .save(Some(&tmp.join("R")), None)
        .expect("saved");
    let path = saved.path.to_string_lossy().into_owned();
    assert_eq!(
        app.projects().recent_projects(),
        std::slice::from_ref(&path)
    );
    assert_eq!(app.settings().recent_projects.len(), 2);
    assert_eq!(app.settings().recent_projects[0], path);
}

// -- golden results -------------------------------------------------------------------------

fn workflows(golden: &Golden) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    for entry in fs::read_dir(golden.root().join("workflows")).expect("workflows") {
        let entry = entry.expect("entry");
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let transcript = golden
                .json(&format!("workflows/{name}/transcript.json"))
                .expect("transcript");
            out.push((name, transcript));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn snapshot(golden: &Golden, workflow: &str, hash: &str) -> (Vec<u8>, Project) {
    let raw = golden
        .bytes_gz(&format!("workflows/{workflow}/snapshots/{hash}.json.gz"))
        .expect("snapshot");
    let project = project_from_value(&loads(&raw).expect("JSON")).expect("valid");
    (raw, project)
}

/// `create_project` through the service, with the recorder's frozen clock and sequential IDs,
/// gives the recorded project, version 1 and a clean scratch workspace `new-<project id>`.
#[test]
fn new_projects_match_the_recorded_create_project_results() {
    let golden = Golden::open().expect("golden data");
    let mut checked = 0;
    for (workflow, transcript) in workflows(&golden) {
        let step = &transcript["steps"][0];
        assert_eq!(step["tool"], "create_project");
        let (raw, recorded) = snapshot(&golden, &workflow, step["project"].as_str().expect("hash"));
        let spec_path = golden
            .root()
            .join("..")
            .join(transcript["workflow"].as_str().expect("workflow path"));
        let spec: Value = serde_json::from_slice(&fs::read(spec_path).expect("workflow file"))
            .expect("workflow JSON");
        let steps = spec.get("steps").unwrap_or(&spec);
        let template = steps[0]["args"]["template"].as_str().unwrap_or("empty");
        let number: u64 = recorded
            .id
            .trim_start_matches("prj_")
            .parse()
            .expect("number");
        let dir = tempfile::tempdir().expect("temp dir");
        let frozen: WallClock = Arc::new(|| UNIX_EPOCH + Duration::from_secs(FROZEN_UNIX_SECONDS));
        let app = AppContext::create(AppOptions {
            workspaces: Some(dir.path().join("workspaces")),
            settings: Some(Settings::default()),
            persist_settings: false,
            ids: Some(IdSource::new(Box::new(SequentialIds::new(number - 1)))),
            wall_clock: Some(frozen),
            ..AppOptions::default()
        })
        .expect("context");
        let signature = recorded.time_signatures.points()[0];
        let project = app
            .projects()
            .new_project(&NewProject {
                name: recorded.name.clone(),
                sample_rate: i64::from(recorded.settings.sample_rate),
                bpm: recorded.tempo_map.bpm_at(0.0),
                time_signature: (signature.numerator, signature.denominator),
                template: template.to_owned(),
                length_beats: recorded.settings.length_beats,
            })
            .expect("created");
        assert!(
            dumps(&project) == raw,
            "{workflow}: the new project differs"
        );
        assert_eq!(step["result"]["project_id"], project.id.as_str());
        let session = app.session().lock();
        assert_eq!(
            Value::from(session.version()),
            step["result"]["project_version"]
        );
        assert!(!session.is_dirty());
        let loc = session.require_location().expect("open");
        assert_eq!(
            loc.workspace,
            dir.path()
                .join("workspaces")
                .join(format!("new-{}", project.id))
        );
        checked += 1;
    }
    assert_eq!(checked, 5);
}

/// `save_project` through the service: the recorded `path`, `layout`, `assets`,
/// `project_sha256`, `backup` and `dirty` of each workflow's save.
#[test]
fn saves_match_the_recorded_save_project_results() {
    let golden = Golden::open().expect("golden data");
    let mut checked = 0;
    for (workflow, transcript) in workflows(&golden) {
        let steps = transcript["steps"].as_array().expect("steps");
        let Some(step) = steps.iter().find(|s| s["tool"] == "save_project") else {
            continue;
        };
        let (_, project) = snapshot(&golden, &workflow, step["project"].as_str().expect("hash"));
        let dir = tempfile::tempdir().expect("temp dir");
        let root = fs::canonicalize(dir.path()).expect("resolved");
        let app = app(&root, true);
        let workspace = app.store().scratch_workspace(&project.id).expect("scratch");
        app.session().lock().load(
            project,
            ProjectLocation {
                path: None,
                workspace,
                packaged: false,
            },
            false,
        );
        let recorded = &step["result"];
        let path = recorded["path"]
            .as_str()
            .expect("path")
            .replace("<ROOT>", &root.to_string_lossy());
        let res = app
            .projects()
            .save(Some(Path::new(&path)), None)
            .expect("saved");
        assert_eq!(res.path.to_string_lossy(), path, "{workflow}");
        assert_eq!(res.layout.as_str(), recorded["layout"], "{workflow}");
        assert_eq!(Value::from(res.assets), recorded["assets"], "{workflow}");
        assert_eq!(res.project_sha256, recorded["project_sha256"], "{workflow}");
        assert_eq!(res.backup, None, "{workflow}");
        assert_eq!(recorded["backup"], Value::Null);
        assert_eq!(Value::from(is_dirty(&app)), recorded["dirty"], "{workflow}");
        checked += 1;
    }
    assert_eq!(checked, 4);
}

/// The coverage batch's second save in place names its backup after the frozen clock.
#[test]
fn a_save_in_place_backs_up_with_the_frozen_clock() {
    let golden = Golden::open().expect("golden data");
    let transcript: Value = golden.json("coverage/transcript.json").expect("transcript");
    let recorded = transcript["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .find(|s| s["id"] == "step172")
        .map(|s| s["result"]["backup"].clone())
        .expect("step 172");
    let dir = tempfile::tempdir().expect("temp dir");
    let root = fs::canonicalize(dir.path()).expect("resolved");
    let frozen: WallClock =
        Arc::new(|| SystemTime::UNIX_EPOCH + Duration::from_secs(FROZEN_UNIX_SECONDS));
    let app = AppContext::create(AppOptions {
        workspaces: Some(root.join("workspaces")),
        settings: Some(Settings::default()),
        persist_settings: false,
        wall_clock: Some(frozen),
        ..AppOptions::default()
    })
    .expect("context");
    let svc = app.projects();
    svc.new_project(&named("Coverage")).expect("created");
    // The step `saved`: `<ROOT>/work/Coverage` becomes `<ROOT>/work/Coverage.sonara`.
    let first = svc
        .save(Some(&root.join("work/Coverage")), None)
        .expect("saved");
    assert_eq!(first.path, root.join("work/Coverage.sonara"));
    assert_eq!(first.backup, None);
    execute(
        &app,
        &RenameProject {
            name: "Renamed".into(),
        },
    );
    let second = svc.save(None, None).expect("saved in place");
    let backup = second
        .backup
        .expect("backup")
        .to_string_lossy()
        .into_owned();
    assert_eq!(
        Value::from(backup.replace(&*root.to_string_lossy(), "<ROOT>")),
        recorded
    );
}
