//! The editing tools (tracks, clips, MIDI, devices, routing, automation) against the prototype on
//! the paths the golden batches leave out (TST-03): every
//! handler's errors, the IDs drawn before a call fails, the transforms of `edit_midi_notes`, the
//! generators and the existing-clip mode of `generate_midi`, every automation target form and the
//! confirmation of `delete_track`.
//!
//! `tests/fixtures/edit_tools.json` is generated from the prototype by
//! `scripts/golden/agent_edit_fixtures.py`: one session, its steps with resolved arguments, and
//! each envelope in the prototype's key order. The test replays the steps through the Rust
//! service, set up as the recorder sets the prototype up (sequential IDs and tokens, a frozen
//! clock, pinned undo merging, the null backend), and compares every envelope, key order
//! included.
//!
//! Unix only: the fixture spells sandbox paths the POSIX way, as `golden_transcripts.rs` does.
#![cfg(unix)]

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, UNIX_EPOCH};

use serde_json::Value;
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_app::settings::Settings;
use sonara_core::ids::SequentialIds;

/// The recorder's placeholder for the sandbox root.
const ROOT: &str = "<ROOT>";
/// The recorder's placeholder for wall-clock values.
const VOLATILE: &str = "<VOLATILE>";
/// 2026-01-01T12:00:00Z, the recorder's frozen clock (`golden_common.FIXED_NOW`).
const FROZEN_UNIX_SECONDS: u64 = 1_767_268_800;

/// The fixture's normalization: the sandbox root becomes `<ROOT>`, and a `created` timestamp (a
/// string; the `created` lists of change sets stay) becomes `<VOLATILE>`.
fn normalize(value: &Value, root: &str) -> Value {
    match value {
        Value::String(s) => Value::String(s.replace(root, ROOT)),
        Value::Array(items) => Value::Array(items.iter().map(|v| normalize(v, root)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| {
                    let v = if k == "created" && v.is_string() {
                        Value::from(VOLATILE)
                    } else {
                        normalize(v, root)
                    };
                    (k.clone(), v)
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

/// `<ROOT>` back to the sandbox root in every string.
fn rooted(value: &Value, root: &str) -> Value {
    match value {
        Value::String(s) => Value::String(s.replace(ROOT, root)),
        Value::Array(items) => Value::Array(items.iter().map(|v| rooted(v, root)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.replace(ROOT, root), rooted(v, root)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The first difference between two JSON values, key order included, as a JSON pointer with
/// both values. Numbers compare as JSON numbers: `1` and `1.0` differ, as they do in Python.
fn difference(expected: &Value, actual: &Value, at: &str) -> Option<String> {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            let keys_e: Vec<&String> = e.keys().collect();
            let keys_a: Vec<&String> = a.keys().collect();
            if keys_e != keys_a {
                return Some(format!("{at}: keys {keys_e:?}, got {keys_a:?}"));
            }
            e.iter()
                .find_map(|(k, ev)| difference(ev, &a[k.as_str()], &format!("{at}/{k}")))
        }
        (Value::Array(e), Value::Array(a)) => e
            .iter()
            .zip(a)
            .enumerate()
            .find_map(|(i, (ev, av))| difference(ev, av, &format!("{at}/{i}")))
            .or_else(|| {
                (e.len() != a.len())
                    .then(|| format!("{at}: expected {} items, got {}", e.len(), a.len()))
            }),
        _ if expected == actual => None,
        _ => Some(format!("{at}: expected {expected}, got {actual}")),
    }
}

fn service(root: &Path) -> AgentService {
    for sub in ["home", "projects", "work", "workspaces"] {
        fs::create_dir(root.join(sub)).expect("a sandbox folder");
    }
    let app = Arc::new(
        AppContext::create(AppOptions {
            workspaces: Some(root.join("workspaces")),
            settings: Some(Settings::default()),
            persist_settings: false,
            audio_backend: Some("null".to_owned()),
            ids: Some(IdSource::new(Box::new(SequentialIds::new(0)))),
            wall_clock: Some(Arc::new(|| {
                UNIX_EPOCH + Duration::from_secs(FROZEN_UNIX_SECONDS)
            })),
            undo_clock: Some(Arc::new(|| 0.0)),
            ..AppOptions::default()
        })
        .expect("the application"),
    );
    let counter = Arc::new(AtomicU64::new(0));
    AgentService::new(
        app,
        AgentOptions {
            base_dir: Some(root.join("work")),
            audit: AuditTarget::Disabled,
            allowed_paths: vec![root.to_path_buf()],
            tokens: Some(Arc::new(move || {
                // Orders tokens only.
                let n = counter.fetch_add(1, Ordering::Relaxed) + 1;
                format!("golden-token-{n:04}")
            })),
            environment: Some(PolicyEnvironment {
                allowed: None,
                projects_dir: root.join("projects"),
            }),
            ..AgentOptions::default()
        },
    )
    .expect("the agent service")
}

#[test]
fn editing_tools_match_the_prototype() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/edit_tools.json");
    let fixture: Value =
        serde_json::from_slice(&fs::read(&path).expect("the fixture")).expect("fixture JSON");
    let steps = fixture["steps"]
        .as_array()
        .expect("the fixture lists steps");
    let dir = tempfile::tempdir().expect("a sandbox");
    // Resolved, as the sandbox resolves its roots (`/var` is a link on macOS).
    let root = fs::canonicalize(dir.path()).expect("the sandbox resolves");
    let root_text = root.to_str().expect("a UTF-8 sandbox path");
    let service = service(&root);
    let failures: Vec<String> = steps
        .iter()
        .enumerate()
        .filter_map(|(i, step)| {
            let tool = step["tool"].as_str().expect("a tool name");
            let args = rooted(&step["args"], root_text);
            let actual = normalize(&service.call(tool, Some(&args)), root_text);
            difference(&step["result"], &actual, "")
                .map(|difference| format!("step {i} ({tool}): {difference}"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(steps.len(), 125);
}
