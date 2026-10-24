//! `sonara batch` on every `examples/` workflow against the golden transcripts
//! (`golden/workflows/<name>/transcript.json`, recorded by running each example through the
//! prototype's `run_batch`; GLD-01).
//!
//! - [`workflows_run_to_the_end`]: [`sonara::batch::run_batch`] on an agent service set up as
//!   the recorder set it up (sandbox, sequential IDs and tokens, frozen clock and UTC preview
//!   names, null audio, no audit log): every step's ID, tool and full envelope equal the
//!   recording. Analysis floats (and the size of a FLAC file of samples the render residual
//!   changed) are compared within the allowances that the transcript replay of `sonara-agent`
//!   declares and justifies step by step (`crates/sonara-agent/tests/replay/allowances.rs`,
//!   included here); every other value compares exactly.
//! - [`batch_summaries_equal_the_transcripts`]: the built binary on the same files: the printed
//!   summary (`{id, tool, ok}` per step) equals the recording, and every step succeeds.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, UNIX_EPOCH};

use serde_json::{Map, Value, json};
use sonara::batch::run_batch;
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget};
use sonara_app::clock::LocalZone;
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_app::settings::Settings;
use sonara_core::ids::IdGenerator;
use sonara_testkit::Golden;

use crate::support::Sandbox;

/// The tolerances of the golden batches, shared with `sonara-agent`'s transcript replay.
#[path = "../../../../crates/sonara-agent/tests/replay/allowances.rs"]
mod allowances;
/// The recorder's placeholder for wall-clock values, and the keys that carry them
/// (`golden_batch.VOLATILE_KEYS`).
const VOLATILE: &str = "<VOLATILE>";
const VOLATILE_KEYS: [&str; 7] = [
    "elapsed_seconds",
    "started",
    "finished",
    "created",
    "duration_seconds",
    "render_seconds",
    "realtime_factor",
];
/// 2026-01-01T12:00:00Z, the recorder's frozen clock.
const FROZEN_UNIX_SECONDS: u64 = 1_767_268_800;

/// The recorded workflows.
const WORKFLOWS: [&str; 5] = [
    "cinematic_orchestral",
    "electronic_124bpm",
    "halloween_thirteenth_hour",
    "mix_analysis",
    "sidechain_preview",
];

/// One workflow: the example's steps and the recorded transcript steps.
struct Workflow {
    name: &'static str,
    example: PathBuf,
    steps: Vec<Value>,
    recorded: Vec<Value>,
}

impl Workflow {
    fn load(golden: &Golden, name: &'static str) -> Self {
        let transcript: Value = golden
            .json(&format!("workflows/{name}/transcript.json"))
            .expect("the workflow transcript");
        let example = golden
            .root()
            .parent()
            .expect("golden/ lies in the repository")
            .join(transcript["workflow"].as_str().expect("the workflow path"));
        let spec: Value = serde_json::from_slice(&std::fs::read(&example).expect("the example"))
            .expect("the example is JSON");
        let steps = spec["steps"]
            .as_array()
            .expect("the example lists steps")
            .clone();
        let recorded = transcript["steps"]
            .as_array()
            .expect("the recorded steps")
            .clone();
        assert_eq!(
            steps.len(),
            recorded.len(),
            "{name}: every step was recorded"
        );
        Self {
            name,
            example,
            steps,
            recorded,
        }
    }
}

/// Sequential IDs (`deterministic_ids(seed=0)`).
struct Counter(Arc<AtomicU64>);

impl IdGenerator for Counter {
    fn new_id(&mut self, prefix: &str) -> String {
        // The counter only orders IDs.
        let n = self.0.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{prefix}_{n:012}")
    }
}

/// Whether an example step is a `generate_midi` that humanizes its notes (unless `humanize` is
/// false), through `random.gauss` and so the platform's libm.
fn humanizing_generate_midi(step: &Value) -> bool {
    step["tool"] == "generate_midi" && step["args"].get("humanize") != Some(&Value::Bool(false))
}

/// The recorder's session (`area_workflows._record_workflow`) in `root`.
fn recorder_session(root: &Path) -> (Arc<AppContext>, AgentService) {
    for sub in ["home", "projects", "work", "workspaces"] {
        std::fs::create_dir(root.join(sub)).expect("a sandbox folder");
    }
    let app = Arc::new(
        AppContext::create(AppOptions {
            workspaces: Some(root.join("workspaces")),
            settings: Some(Settings::default()),
            persist_settings: false,
            audio_backend: Some("null".to_owned()),
            ids: Some(IdSource::new(Box::new(Counter(Arc::new(AtomicU64::new(
                0,
            )))))),
            wall_clock: Some(Arc::new(|| {
                UNIX_EPOCH + Duration::from_secs(FROZEN_UNIX_SECONDS)
            })),
            // The recorder's frozen `time.strftime` formats its moment in UTC.
            local_zone: LocalZone::Utc,
            undo_clock: Some(Arc::new(|| 0.0)),
            ..AppOptions::default()
        })
        .expect("the application starts"),
    );
    let tokens = AtomicU64::new(0);
    let service = AgentService::new(
        Arc::clone(&app),
        AgentOptions {
            base_dir: Some(root.join("work")),
            audit: AuditTarget::Disabled,
            allowed_paths: vec![root.to_path_buf()],
            tokens: Some(Arc::new(move || {
                format!(
                    "golden-token-{:04}",
                    tokens.fetch_add(1, Ordering::Relaxed) + 1
                )
            })),
            environment: Some(PolicyEnvironment {
                allowed: None,
                projects_dir: root.join("projects"),
            }),
            ..AgentOptions::default()
        },
    )
    .expect("the agent service starts");
    (app, service)
}

/// `golden_batch.normalize`: `<ROOT>` for the sandbox, `<VOLATILE>` for wall-clock values.
fn normalize(value: &Value, root: &str) -> Value {
    match value {
        Value::String(text) => Value::String(text.replace(root, "<ROOT>")),
        Value::Array(items) => Value::Array(items.iter().map(|v| normalize(v, root)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| {
                    let v = if VOLATILE_KEYS.contains(&k.as_str()) {
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

/// The first difference between two JSON values, as a JSON pointer with both values.
fn difference(expected: &Value, actual: &Value, at: &str) -> Option<String> {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => e
            .keys()
            .chain(a.keys().filter(|k| !e.contains_key(*k)))
            .find_map(|key| match (e.get(key), a.get(key)) {
                (Some(ev), Some(av)) => difference(ev, av, &format!("{at}/{key}")),
                (ev, av) => Some(format!("{at}/{key}: expected {ev:?}, got {av:?}")),
            }),
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

/// Runs `workflow` through `run_batch` as the recorder did and returns each step's
/// `{id, tool, result}` (normalized) and the batch's summary.
fn run_recorded(workflow: &Workflow, steps: &[Value]) -> (Vec<Value>, Vec<Value>) {
    let sandbox = tempfile::Builder::new()
        .prefix("sonara-golden-")
        .tempdir()
        .expect("a sandbox");
    let root = std::fs::canonicalize(sandbox.path()).expect("the sandbox resolves");
    let root_text = root.to_str().expect("a UTF-8 sandbox path").to_owned();
    let (app, service) = recorder_session(&root);
    let mut transcript = Vec::new();
    let mut results = Map::new();
    let summary = run_batch(
        &service,
        steps,
        true,
        &mut results,
        &mut |sid, tool, res| {
            transcript.push(json!({"id": sid, "tool": tool, "result": normalize(res, &root_text)}));
        },
    )
    .unwrap_or_else(|error| panic!("{}: {error}", workflow.name));
    app.close();
    (transcript, summary)
}

#[test]
fn batch_summaries_equal_the_transcripts() {
    let golden = Golden::open().unwrap();
    for name in WORKFLOWS {
        let workflow = Workflow::load(&golden, name);
        let sandbox = Sandbox::new();
        let run = sandbox.run(&[
            "batch",
            workflow.example.to_str().unwrap(),
            "--connect",
            "never",
        ]);
        assert_eq!(run.code, 0, "{name}: {}", run.stderr);
        assert_eq!(run.json["ok"], true, "{name}");
        let steps = run.json["steps"].as_array().unwrap();
        let expected: Vec<Value> = workflow
            .recorded
            .iter()
            .map(|recorded| json!({"id": recorded["id"], "tool": recorded["tool"], "ok": true}))
            .collect();
        assert_eq!(steps, &expected, "{name}");
        assert_eq!(run.stderr.lines().count(), steps.len(), "{name}");
    }
}

#[test]
fn workflows_run_to_the_end() {
    let golden = Golden::open().unwrap();
    for name in WORKFLOWS {
        let workflow = Workflow::load(&golden, name);
        let (transcript, summary) = run_recorded(&workflow, &workflow.steps);
        assert_eq!(transcript.len(), workflow.recorded.len(), "{name}");
        let declared = allowances::allowances(name);
        for (i, (actual, recorded)) in transcript.iter().zip(&workflow.recorded).enumerate() {
            let mut actual = actual.clone();
            // Off macOS, notes humanized through the platform's `random.gauss` (libm) differ from
            // the recording by an ulp, so a later save hashes differently; `sonara-agent`'s
            // transcript replay checks every such project within `LIBM` (its `replay` module).
            if !cfg!(target_os = "macos")
                && workflow.steps[..i].iter().any(humanizing_generate_midi)
                && let (Some(slot), Some(recorded)) = (
                    actual.pointer_mut("/result/project_sha256"),
                    recorded.pointer("/result/project_sha256"),
                )
            {
                slot.clone_from(recorded);
            }
            let covering: Vec<_> = declared.iter().filter(|a| a.covers(i)).collect();
            for allowance in &covering {
                allowance.apply(&recorded["result"], &mut actual["result"]);
            }
            let expected = json!({
                "id": recorded["id"],
                "tool": recorded["tool"],
                "result": recorded["result"],
            });
            if let Some(found) = difference(&expected, &actual, "") {
                let allowed: Vec<String> = covering
                    .iter()
                    .map(|a| format!("{} ({})", a.name, a.reason))
                    .collect();
                panic!(
                    "{name} step {i}: {found}; allowances: {}",
                    allowed.join(", ")
                );
            }
        }
        assert!(summary.iter().all(|s| s["ok"] == true), "{name}");
    }
}
