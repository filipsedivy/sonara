//! The agent tools against the prototype on the paths of its `tests/agent/test_tools.py` that no
//! golden step takes (gaps T1, T3 to T7 and T8, TST-03):
//! `create_project` at 44.1 kHz in 3/4 with a bar length (T4), the templates (T6), tempo, meter,
//! length and automation-target paths (T7), the dry runs of every editing tool (T1), saving
//! packaged and reopening the archive (T5), Ogg and AIFF files through `import_audio` and a
//! render (T3), and the mixer's range errors, an unknown preset and the routing loops (T8).
//!
//! `tests/fixtures/tool_paths.json` is generated from the prototype by
//! `scripts/golden/agent_tool_fixtures.py`: one session per prototype test, each with its steps'
//! resolved arguments and envelopes in the prototype's key order. Each test here replays one
//! session through the Rust service on a fresh sandbox, set up as the generator sets the
//! prototype up (sequential IDs and tokens, a frozen clock in UTC, pinned undo merging, the null
//! backend, the golden input files under `<ROOT>/inputs/`), and compares every envelope, key
//! order included. A few tests add the prototype test's own invariants on top.
//!
//! **Allowances.** Analyses in the results compare within the documented allowances of
//! `replay/allowances.rs` (one source with the transcript replay), under the session's name:
//! the analyzer's own everywhere, the platform libm's off macOS, and the Vorbis decoder's in the
//! two previews of `import_audio_formats`. Every other value compares exactly, and a
//! step-scoped allowance that a step no longer needs fails the test.
//!
//! Unix only: the fixture spells sandbox paths the POSIX way, as `golden_transcripts.rs` does.
#![cfg(unix)]

#[path = "replay/allowances.rs"]
mod allowances;

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, UNIX_EPOCH};

use serde_json::Value;
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget};
use sonara_app::clock::LocalZone;
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_app::settings::Settings;
use sonara_core::ids::SequentialIds;
use sonara_testkit::Golden;
use tempfile::TempDir;

use allowances::{Allowance, allowances};

/// The generator's placeholder for the sandbox root.
const ROOT: &str = "<ROOT>";
/// The generator's placeholder for wall-clock values.
const VOLATILE: &str = "<VOLATILE>";
/// The keys whose string or number values the generator replaces (`golden_batch.VOLATILE_KEYS`).
const VOLATILE_KEYS: [&str; 7] = [
    "elapsed_seconds",
    "started",
    "finished",
    "created",
    "duration_seconds",
    "render_seconds",
    "realtime_factor",
];
/// 2026-01-01T12:00:00Z, the recorder's frozen clock (`golden_common.FIXED_NOW`).
const FROZEN_UNIX_SECONDS: u64 = 1_767_268_800;

/// The generator's normalization: the sandbox root becomes `<ROOT>`, and the string or number
/// value of a [`VOLATILE_KEYS`] key becomes `<VOLATILE>` (the `created` lists of change sets
/// stay).
fn normalize(value: &Value, root: &str) -> Value {
    match value {
        Value::String(s) => Value::String(s.replace(root, ROOT)),
        Value::Array(items) => Value::Array(items.iter().map(|v| normalize(v, root)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| {
                    let wall_clock =
                        VOLATILE_KEYS.contains(&k.as_str()) && (v.is_string() || v.is_number());
                    let v = if wall_clock {
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

/// One recorded session on a fresh sandbox and application.
struct Session {
    name: String,
    _dir: TempDir,
    root: PathBuf,
    app: Arc<AppContext>,
    service: AgentService,
    steps: Vec<Value>,
    allowances: Vec<Allowance>,
    /// Every allowance used: the step, the allowance's name and the largest difference.
    used: RefCell<Vec<(usize, &'static str, f64)>>,
}

impl Session {
    fn new(name: &str) -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tool_paths.json");
        let fixture: Value =
            serde_json::from_slice(&fs::read(&path).expect("the fixture")).expect("fixture JSON");
        let steps = fixture["sessions"][name]
            .as_array()
            .unwrap_or_else(|| panic!("the fixture records the session {name}"))
            .clone();
        let dir = tempfile::Builder::new()
            .prefix("sonara-golden-")
            .tempdir()
            .expect("a sandbox");
        // Resolved, as the sandbox resolves its roots (`/var` is a link on macOS).
        let root = fs::canonicalize(dir.path()).expect("the sandbox resolves");
        for sub in ["home", "projects", "work", "workspaces"] {
            fs::create_dir(root.join(sub)).expect("a sandbox folder");
        }
        let golden = Golden::open().expect("golden data is available");
        for rel in fixture["inputs"].as_array().expect("the input files") {
            let rel = rel.as_str().expect("a golden path");
            let target = root.join("inputs").join(rel);
            fs::create_dir_all(target.parent().expect("a folder")).expect("the input folder");
            fs::write(&target, golden.bytes(rel).expect("the golden input")).expect("the input");
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
                // The recorder's frozen `time.strftime` formats its moment in UTC.
                local_zone: LocalZone::Utc,
                undo_clock: Some(Arc::new(|| 0.0)),
                ..AppOptions::default()
            })
            .expect("the application"),
        );
        let counter = Arc::new(AtomicU64::new(0));
        let service = AgentService::new(
            Arc::clone(&app),
            AgentOptions {
                base_dir: Some(root.join("work")),
                audit: AuditTarget::Disabled,
                allowed_paths: vec![root.clone()],
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
        .expect("the agent service");
        Self {
            name: name.to_owned(),
            _dir: dir,
            root,
            app,
            service,
            steps,
            allowances: allowances(name),
            used: RefCell::new(Vec::new()),
        }
    }

    fn root_text(&self) -> &str {
        self.root.to_str().expect("a UTF-8 sandbox path")
    }

    /// Runs step `index` and returns its difference from the recorded envelope, if any, with
    /// the normalized envelope.
    fn step(&self, index: usize) -> (Option<String>, Value) {
        let step = &self.steps[index];
        let tool = step["tool"].as_str().expect("a tool name");
        let args = rooted(&step["args"], self.root_text());
        let mut actual = normalize(&self.service.call(tool, Some(&args)), self.root_text());
        let expected = &step["result"];
        for allowance in self.allowances.iter().filter(|a| a.covers(index)) {
            if let Some(used) = allowance.apply(expected, &mut actual) {
                self.used.borrow_mut().push((index, allowance.name, used));
            }
        }
        let failure = difference(expected, &actual, "")
            .map(|difference| format!("step {index} ({tool}): {difference}"));
        (failure, actual)
    }

    /// Runs steps `range` and returns their failures.
    fn run(&self, range: std::ops::Range<usize>) -> Vec<String> {
        range.filter_map(|i| self.step(i).0).collect()
    }

    /// Fails when a step-scoped allowance was not needed by one of its steps; prints the
    /// largest difference each allowance absorbed (`--nocapture`).
    fn check_allowances(&self) {
        let used = self.used.borrow();
        for allowance in &self.allowances {
            for step in allowance.steps {
                assert!(
                    used.iter()
                        .any(|(s, name, _)| s == step && *name == allowance.name),
                    "{} step {step}: {} is not needed any more ({})",
                    self.name,
                    allowance.name,
                    allowance.reason
                );
            }
            if let Some((step, _, largest)) = used
                .iter()
                .filter(|(_, name, _)| *name == allowance.name)
                .max_by(|a, b| a.2.total_cmp(&b.2))
            {
                println!(
                    "{}: {} at most {largest:.2e} (step {step}; tolerance {:.0e})",
                    self.name, allowance.name, allowance.tolerance
                );
            }
        }
    }

    fn version(&self) -> u64 {
        self.app.session().lock().version()
    }

    /// The index of the first step at or after `from` that calls `tool`.
    fn find(&self, tool: &str, from: usize) -> usize {
        (from..self.steps.len())
            .find(|&i| self.steps[i]["tool"] == tool)
            .unwrap_or_else(|| panic!("a {tool} step after {from}"))
    }
}

/// Replays a whole session and asserts that it matches the prototype; returns the session.
fn replay(name: &str, steps: usize) -> Session {
    let session = Session::new(name);
    assert_eq!(session.steps.len(), steps, "{name}: recorded steps");
    let failures = session.run(0..steps);
    assert!(failures.is_empty(), "{name}:\n{}", failures.join("\n"));
    session.check_allowances();
    session
}

/// T4, `test_create_project_defaults_and_state`: 124 BPM in 3/4 at 44.1 kHz, "8 bars" long (24
/// beats, explicit), clean; a clean project is replaced without a confirmation.
#[test]
fn create_project_defaults_and_state() {
    replay("create_project_defaults_and_state", 4);
}

/// T6, `test_create_project_templates`: the electronic, band and orchestral templates' tracks,
/// buses and devices, each replacing the previous clean project.
#[test]
fn create_project_templates() {
    replay("create_project_templates", 3);
}

/// T7, `test_tempo_meter_length_sections_markers_metadata_loop`: a tempo change and a meter
/// change, `set_tempo` with `clear_changes`, `set_project_length` with a time and without a
/// length, a section, a marker, metadata and the loop, and a loop that ends before it starts.
#[test]
fn tempo_meter_length_sections_markers_metadata_loop() {
    replay("tempo_meter_length_sections_markers_metadata_loop", 17);
}

/// T7, `test_create_automation_targets`: volume, pan, a `send:<id>` level and a device
/// parameter, a replaced range of an existing lane, and the target errors.
#[test]
fn create_automation_targets() {
    replay("create_automation_targets", 16);
}

/// T1, `test_dry_run_never_commits`: 31 dry runs, among them the 10 tools no golden step
/// dry-runs (`add_marker`, `add_section`, `create_midi_clip`, `set_instrument`, `set_metadata`,
/// `set_project_length`, `set_tempo`, `set_time_signature`, `set_track_volume`, `split_clip`).
/// Every one reports the prototype's envelope (`committed: false`), the session's version stays
/// put, the full state view after them equals the one before, and the dry save writes nothing.
#[test]
fn dry_run_never_commits() {
    let session = Session::new("dry_run_never_commits");
    assert_eq!(session.steps.len(), 38);
    let before = session.find("get_project_state", 0);
    let after = session.find("get_project_state", before + 1);
    assert_eq!(after, session.steps.len() - 1);
    let mut failures = session.run(0..before);
    let (failure, state_before) = session.step(before);
    failures.extend(failure);
    let version = session.version();
    for i in before + 1..after {
        let (failure, envelope) = session.step(i);
        failures.extend(failure);
        assert_eq!(envelope["committed"], false, "step {i}: {envelope}");
        assert_eq!(session.version(), version, "step {i} committed");
    }
    let (failure, state_after) = session.step(after);
    failures.extend(failure);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    session.check_allowances();
    assert_eq!(state_after["state"], state_before["state"]);
    assert!(!session.root.join("work/dry.sonara").exists());
}

/// T5, `test_open_save_save_as_copy_and_packaged`: a save without a path, a first save, a save
/// in place with a backup, a copy that keeps the open project, a packaged archive, the
/// confirmation to overwrite another project, reopening the archive, a dry open and a missing
/// project.
#[test]
fn open_save_save_as_copy_and_packaged() {
    let session = replay("open_save_save_as_copy_and_packaged", 16);
    let work = session.root.join("work");
    assert!(
        work.join("Packed.sonara").is_file(),
        "an archive is one file"
    );
    assert!(work.join("Orig.sonara/project.json").is_file());
    assert!(work.join("Copy.sonara/project.json").is_file());
}

/// T3, `test_import_audio_formats`: the prototype's Ogg Vorbis file (decoded into the cache by
/// a job) and AIFF file (read directly) through `import_audio` and `render_preview`.
#[test]
fn import_audio_formats() {
    let session = replay("import_audio_formats", 7);
    let previews = session.root.join("work/previews");
    assert!(previews.join("ogg.wav").is_file());
    assert!(previews.join("ogg-aiff.wav").is_file());
}

/// T8, `test_track_create_rename_delete_and_mixer`: track kinds, a folder child at index 0, the
/// mixer and state setters, their range errors, and deleting an empty track without a
/// confirmation.
#[test]
fn track_create_rename_delete_and_mixer() {
    replay("track_create_rename_delete_and_mixer", 19);
}

/// T8, `test_set_instrument_and_preset`: an instrument with a preset, an effect refused as an
/// instrument, an unknown preset (`NOT_FOUND`) and an audio track refused.
#[test]
fn set_instrument_and_preset() {
    replay("set_instrument_and_preset", 7);
}

/// T8, `test_configure_routing_sends_output_cycles_and_sidechain`: a pre-fader send and its
/// level, a bus into a bus, the feedback loops of an output and of a send (`ROUTING_CYCLE`), a
/// send to a track that is no bus, nothing to change, a removed send, and a sidechain set,
/// cleared and refused.
#[test]
fn configure_routing_sends_output_cycles_and_sidechain() {
    replay("configure_routing_sends_output_cycles_and_sidechain", 21);
}
