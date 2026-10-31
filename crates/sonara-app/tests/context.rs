//! The composition root (`sonara.application.context`), and the session lock it owns
//! (`test_concurrent_execution_is_serialized` of `tests/unit/test_commands.py`).

use std::fs;
use std::num::NonZeroUsize;
use std::path::Path;
use std::thread;

use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_app::projects::NewProject;
use sonara_app::settings::{SETTINGS_FILE, Settings, load_settings, save_settings};
use sonara_core::commands::project::SetTempo;
use sonara_core::commands::track::{CreateTrack, SetTrackMixer};
use sonara_core::error::ErrorCode;
use sonara_core::ids::{self, IdGenerator as _, SequentialIds};
use sonara_core::model::JsonMap;
use sonara_core::model::track::TrackKind;

fn options(dir: &Path) -> AppOptions {
    AppOptions {
        workspaces: Some(dir.join("workspaces")),
        settings: Some(Settings::default()),
        persist_settings: false,
        ids: Some(IdSource::new(Box::new(SequentialIds::new(0)))),
        ..AppOptions::default()
    }
}

fn new_project(app: &AppContext) {
    app.projects()
        .new_project(&NewProject::default())
        .expect("created");
}

fn track(app: &AppContext, name: &str) -> String {
    let mut source = app.ids();
    let command = CreateTrack {
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
        part: None,
    };
    app.session()
        .lock()
        .execute(&command, false, "user")
        .expect("track created");
    command.track_id
}

#[test]
fn concurrent_execution_is_serialized() {
    let dir = tempfile::tempdir().expect("temp dir");
    let app = AppContext::create(options(dir.path())).expect("context");
    new_project(&app);
    let tids: Vec<String> = (0..4).map(|i| track(&app, &format!("T{i}"))).collect();
    let start_version = app.session().lock().version();
    let versions = std::sync::Mutex::new(Vec::new());
    thread::scope(|scope| {
        for tid in &tids {
            let (app, versions) = (&app, &versions);
            scope.spawn(move || {
                for i in 0..50_u8 {
                    let command = SetTrackMixer {
                        track_id: tid.clone(),
                        pan: Some(f64::from(i % 10) / 10.0),
                        ..SetTrackMixer::default()
                    };
                    let res = app
                        .session()
                        .lock()
                        .execute(&command, false, "user")
                        .expect("mixer change");
                    versions.lock().expect("not poisoned").push(res.version);
                }
            });
        }
    });
    let mut versions = versions.into_inner().expect("not poisoned");
    versions.sort_unstable();
    let expected: Vec<u64> = (start_version + 1..=start_version + 200).collect();
    assert_eq!(versions, expected);
    let session = app.session().lock();
    assert_eq!(session.version(), start_version + 200);
    let project = session.project().expect("open");
    for tid in &tids {
        assert_eq!(project.track(tid).expect("track").mixer.pan, 0.9);
    }
}

#[test]
fn the_context_applies_the_settings_and_options() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut opts = options(dir.path());
    opts.settings = Some(Settings {
        undo_limit: 2,
        backup_retention: -1,
        ..Settings::default()
    });
    opts.job_workers = NonZeroUsize::new(3).expect("non-zero");
    let app = AppContext::create(opts).expect("context");
    assert_eq!(app.store().backup_retention(), 0, "negative keeps none");
    assert_eq!(app.store().workspaces_root(), dir.path().join("workspaces"));
    assert_eq!(app.jobs().workers().get(), 3);
    assert_eq!(app.audio_backend(), "auto");
    new_project(&app);
    for (beat, bpm) in [(0.0, 90.0), (4.0, 100.0), (8.0, 110.0)] {
        let tempo = SetTempo {
            bpm,
            beat,
            ramp: false,
            clear_changes: false,
        };
        let mut session = app.session().lock();
        session.execute(&tempo, false, "user").expect("tempo");
        session.seal_undo();
    }
    assert_eq!(app.session().lock().history().labels().0.len(), 2);
    let mut opts = options(dir.path());
    opts.audio_backend = Some("null".into());
    let app = AppContext::create(opts).expect("context");
    app.settings().audio.backend = "native".into();
    assert_eq!(
        app.audio_backend(),
        "null",
        "the option overrides the settings"
    );
}

#[test]
fn settings_load_from_and_persist_to_the_settings_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tmp = fs::canonicalize(dir.path()).expect("resolved");
    let file = tmp.join("config").join(SETTINGS_FILE);
    save_settings(
        &Settings {
            theme: "light".into(),
            ..Settings::default()
        },
        &file,
    )
    .expect("saved");
    let app = AppContext::create(AppOptions {
        workspaces: Some(tmp.join("workspaces")),
        settings_path: Some(file.clone()),
        ..AppOptions::default()
    })
    .expect("context");
    assert_eq!(app.settings().theme, "light");
    new_project(&app);
    let saved = app
        .projects()
        .save(Some(&tmp.join("Song")), None)
        .expect("saved");
    let on_disk = load_settings(&file);
    assert_eq!(
        on_disk.recent_projects,
        [saved.path.to_string_lossy().into_owned()]
    );
    assert_eq!(on_disk.theme, "light");
}

#[test]
fn jobs_draw_from_the_shared_ids_and_stop_on_close() {
    let dir = tempfile::tempdir().expect("temp dir");
    let app = AppContext::create(options(dir.path())).expect("context");
    assert_eq!(app.ids().new_id(ids::TRACK), "trk_000000000001");
    let job = app
        .jobs()
        .submit("decode", "Decode", |_| Ok(JsonMap::new()))
        .expect("submitted");
    assert_eq!(job.id(), "op_000000000002");
    assert!(job.wait(None));
    app.close();
    let err = app
        .jobs()
        .submit("decode", "Decode", |_| Ok(JsonMap::new()))
        .expect_err("closed");
    assert_eq!(err.code(), ErrorCode::Internal);
}
