//! User settings (the settings part of `tests/unit/test_config_plugins.py`). The paths part
//! is a unit test in `src/paths.rs`, because it needs a controlled environment.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use sonara_app::settings::{
    AudioSettings, MAX_RECENT, SETTINGS_FILE, Settings, load_settings, save_settings, settings_path,
};
use sonara_core::model::JsonMap;

fn object(value: Value) -> JsonMap {
    match value {
        Value::Object(map) => map,
        other => panic!("not an object: {other}"),
    }
}

#[test]
fn settings_defaults_and_round_trip() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tmp = dir.path();
    assert_eq!(
        load_settings(&tmp.join("missing.json")),
        Settings::default()
    );
    let mut s = Settings::default();
    s.audio.buffer_frames = 512;
    s.theme = "light".into();
    s.extra = object(json!({"future_option": {"x": 1}}));
    for i in 0..MAX_RECENT + 5 {
        s.add_recent(&tmp.join(format!("p{i}.sonara")));
    }
    s.add_recent(&tmp.join("p3.sonara"));
    assert_eq!(s.recent_projects.len(), MAX_RECENT);
    assert!(s.recent_projects[0].ends_with("p3.sonara"));
    let mut unique = s.recent_projects.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), MAX_RECENT);
    let target = tmp.join(SETTINGS_FILE);
    save_settings(&s, &target).expect("saved");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = fs::metadata(&target).expect("file").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let back = load_settings(&target);
    assert_eq!(back.audio.buffer_frames, 512);
    assert_eq!(back.theme, "light");
    assert_eq!(back.extra, object(json!({"future_option": {"x": 1}})));
    assert_eq!(back.recent_projects, s.recent_projects);
    assert_eq!(back, s);
    let raw: Value = serde_json::from_slice(&fs::read(&target).expect("file")).expect("JSON");
    assert_eq!(raw["future_option"], json!({"x": 1}));
    if let Ok(path) = settings_path() {
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some("settings.json")
        );
    }
}

#[test]
fn settings_unreadable_files_fall_back() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = dir.path().join("s.json");
    for content in ["{broken", "[1, 2]", "null", "\"text\""] {
        fs::write(&file, content).expect("written");
        assert_eq!(load_settings(&file), Settings::default(), "{content}");
    }
    fs::write(&file, b"{\"theme\": \"\xff\"}").expect("written");
    assert_eq!(load_settings(&file), Settings::default(), "not UTF-8");
    assert_eq!(
        load_settings(dir.path()),
        Settings::default(),
        "a directory"
    );
}

#[test]
fn settings_ignore_wrong_types() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = dir.path().join("s.json");
    let data = json!({
        "autosave_seconds": "soon", "undo_limit": true, "theme": 5, "agent_require_confirmation": "yes",
        "recent_projects": "not a list", "agent_allowed_paths": ["/ok", 3], "ffmpeg_path": "/usr/bin/ffmpeg",
        "audio": {"sample_rate": 44100, "lookahead_ms": 50, "buffer_frames": "big", "output_device": 7},
    });
    fs::write(&file, data.to_string()).expect("written");
    let s = load_settings(&file);
    let d = Settings::default();
    assert_eq!(
        (s.autosave_seconds, s.undo_limit, &s.theme),
        (d.autosave_seconds, d.undo_limit, &d.theme)
    );
    assert!(s.agent_require_confirmation);
    assert!(s.recent_projects.is_empty() && s.agent_allowed_paths.is_empty());
    assert_eq!(s.ffmpeg_path.as_deref(), Some("/usr/bin/ffmpeg"));
    assert_eq!(s.audio.sample_rate, 44_100);
    assert_eq!(s.audio.lookahead_ms, 50.0);
    assert_eq!(s.audio.buffer_frames, d.audio.buffer_frames);
    assert_eq!(s.audio.output_device, None);
    fs::write(&file, json!({"audio": [1, 2]}).to_string()).expect("written");
    assert_eq!(load_settings(&file).audio, d.audio);
}

#[test]
fn nulls_floats_and_unknown_keys_load_like_the_prototype() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = dir.path().join("s.json");
    let data = json!({
        "zz_later": [1, 2],
        "extra": {"dropped": true},
        "theme": null,
        "ffmpeg_path": null,
        "undo_limit": 5.0,
        "version": 2,
        "audio": {"midi_inputs": ["Keys"], "input_device": null, "future": 1, "backend": "null"},
    });
    fs::write(&file, data.to_string()).expect("written");
    let s = load_settings(&file);
    assert_eq!(s.theme, "dark", "null is not a string");
    assert_eq!(s.ffmpeg_path, None);
    assert_eq!(s.undo_limit, 500, "5.0 is not an integer");
    assert_eq!(s.version, 2);
    assert_eq!(
        s.audio,
        AudioSettings {
            midi_inputs: Some(vec!["Keys".into()]),
            backend: "null".into(),
            ..AudioSettings::default()
        }
    );
    // `extra` is a field name, never an unknown key; unknown audio keys are dropped.
    assert_eq!(s.extra, object(json!({"zz_later": [1, 2]})));
}

#[test]
fn saved_bytes_match_the_prototype() {
    // The prototype's `save_settings` of this value (Python 3.13), byte for byte.
    let expected = r#"{
  "zz": {
    "x": 1.5,
    "u": "T\u0159in\u00e1ct\u00e1 \u2713 \ud83d\ude00"
  },
  "a": [
    1,
    1e+16,
    2.0
  ],
  "version": 1,
  "audio": {
    "backend": "auto",
    "output_device": null,
    "input_device": null,
    "sample_rate": 48000,
    "buffer_frames": 256,
    "lookahead_ms": 80.0,
    "live_lookahead_ms": 25.0,
    "midi_inputs": null,
    "output_channels": 2,
    "input_channels": 2
  },
  "autosave_seconds": 60,
  "backup_retention": 10,
  "undo_limit": 500,
  "theme": "sv\u011btl\u00e1",
  "recent_projects": [
    "/tmp/p.sonara"
  ],
  "agent_allowed_paths": [],
  "agent_require_confirmation": true,
  "ffmpeg_path": null
}
"#;
    let mut s = Settings {
        extra: object(json!({"zz": {"x": 1.5, "u": "Třináctá ✓ 😀"}, "a": [1, 1e16, 2.0]})),
        theme: "světlá".into(),
        ..Settings::default()
    };
    s.add_recent(Path::new("/tmp/p.sonara"));
    let dir = tempfile::tempdir().expect("temp dir");
    let target = dir.path().join("config").join(SETTINGS_FILE);
    save_settings(&s, &target).expect("saved, creating the directory");
    assert_eq!(fs::read_to_string(&target).expect("file"), expected);
    assert_eq!(load_settings(&target), s);
}
