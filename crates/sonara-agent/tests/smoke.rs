//! The main end-to-end path through the application wiring (SON-P20): the agent service on the
//! application context, the same composition `sonara` and the desktop app use, with the null
//! audio backend. A project is created, edited, generated into, checked, exported as MIDI and as
//! WAV, analyzed and saved, and every step is one tool call. The test adds no logic of its own:
//! it only drives the public tools and reads what they return.
//!
//! `docs/smoke-tests.md` lists which task modules are part of this path and which are not
//! wired yet.

use std::fs;
use std::sync::Arc;

use serde_json::{Value, json};
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::permissions::sequential_tokens;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget, shared_tokens};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_app::settings::Settings;
use sonara_core::ids::SequentialIds;

/// The application with the null backend and an agent whose sandbox is a temporary directory:
/// the directory (kept alive), its resolved path, the audit log's path and the service.
fn world() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    AgentService,
) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let root = fs::canonicalize(dir.path()).expect("the directory resolves");
    for sub in ["work", "projects", "workspaces"] {
        fs::create_dir(root.join(sub)).expect("a folder");
    }
    let app = Arc::new(
        AppContext::create(AppOptions {
            workspaces: Some(root.join("workspaces")),
            settings: Some(Settings::default()),
            persist_settings: false,
            audio_backend: Some("null".to_owned()),
            ids: Some(IdSource::new(Box::new(SequentialIds::new(0)))),
            ..AppOptions::default()
        })
        .expect("the application"),
    );
    let log = root.join("audit.jsonl");
    let service = AgentService::new(
        app,
        AgentOptions {
            base_dir: Some(root.join("work")),
            audit: AuditTarget::File(log.clone()),
            tokens: Some(shared_tokens(sequential_tokens())),
            environment: Some(PolicyEnvironment {
                allowed: None,
                projects_dir: root.join("projects"),
            }),
            ..AgentOptions::default()
        },
    )
    .expect("the service");
    (dir, root, log, service)
}

#[test]
fn the_main_path_runs_from_an_empty_project_to_files_on_disk() {
    let (_dir, root, log, service) = world();
    let call = |tool: &str, args: &Value| -> Value {
        let reply = service.call(tool, Some(args));
        assert_eq!(reply["ok"], true, "{tool}: {reply}");
        reply
    };

    call("create_project", &json!({"name": "Smoke", "bpm": 120}));
    let drums = call(
        "create_track",
        &json!({"name": "Drums", "kind": "instrument", "instrument": "sonara.drums"}),
    )["result"]["track_id"]
        .clone();
    let bass = call(
        "create_track",
        &json!({"name": "Bass", "kind": "instrument"}),
    )["result"]["track_id"]
        .clone();
    call("set_tempo", &json!({"bpm": 100}));
    call(
        "generate_midi",
        &json!({"track_id": drums, "kind": "drums", "style": "house", "bars": 2}),
    );
    call(
        "generate_midi",
        &json!({"track_id": bass, "kind": "bass", "key": "A minor", "progression": ["i", "VI"], "beats_per_chord": 4}),
    );
    assert_eq!(call("verify_project", &json!({}))["intact"], true);

    // MIDI: a Standard MIDI File with a header and one track chunk per source plus the tempo track.
    let midi = call("export_midi", &json!({"path": "out/song.mid"}));
    assert_eq!(midi["ok"], true);
    let bytes = fs::read(root.join("work/out/song.mid")).expect("the MIDI file");
    assert_eq!(&bytes[..4], b"MThd");

    // WAV: 2 bars at 100 BPM are 4.8 s; the analysis of the written file agrees.
    let wav = call(
        "export_audio",
        &json!({"path": "out/song.wav", "tail": "none",
                "formats": [{"format": "wav", "bit_depth": 16, "dither": "none"}]}),
    );
    assert_eq!(wav["result"]["outputs"][0]["verified"], true);
    let wav_bytes = fs::read(root.join("work/out/song.wav")).expect("the WAV file");
    assert_eq!(&wav_bytes[..4], b"RIFF");
    let analysis = call("analyze_audio", &json!({"path": "out/song.wav"}));
    let seconds = analysis["analysis"]["duration_seconds"]
        .as_f64()
        .expect("a duration");
    assert!((seconds - 4.8).abs() < 0.05, "{seconds}");
    assert!(analysis["issues"].is_array());

    // Save, and the history shows what the agent did.
    let saved = root.join("projects/smoke.sonara");
    call(
        "save_project",
        &json!({"path": saved.to_str().expect("utf-8")}),
    );
    assert!(saved.exists());
    let history = call("get_history", &json!({}));
    let tools: Vec<&str> = history["recent_agent_actions"]
        .as_array()
        .expect("recent actions")
        .iter()
        .filter_map(|a| a["tool"].as_str())
        .collect();
    for expected in [
        "create_track",
        "set_tempo",
        "generate_midi",
        "export_midi",
        "export_audio",
        "save_project",
    ] {
        assert!(
            tools.contains(&expected),
            "{expected} missing from {tools:?}"
        );
    }
    let audited = fs::read_to_string(&log).expect("the audit log");
    assert!(audited.lines().count() >= 12, "{audited}");
}
