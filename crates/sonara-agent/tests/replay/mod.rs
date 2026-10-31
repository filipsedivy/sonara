//! Transcript replay: runs the golden agent batches (`golden/coverage/` and every
//! `golden/workflows/<name>/`) step by step through the Rust [`AgentService`] and compares each
//! result with the prototype's (GLD-01).
//!
//! The replay rebuilds the recorder's setup (`scripts/golden/golden_batch.py`, `area_coverage.py`,
//! `area_workflows.py`): a sandbox root with `home`, `projects`, `work`, `workspaces` and the
//! input fixtures, sequential IDs and confirmation tokens, a frozen wall clock, pinned undo
//! merging, the null audio backend, the recording host's MIDI ports ([`RecordingHostMidi`]),
//! `base_dir = <root>/work`, the root itself allowed and no audit log.
//!
//! Every step is either **live** or **fast-forwarded**:
//!
//! - A step whose tool has a handler in the service's registry is live: its arguments (with
//!   `<ROOT>` put back) go through [`AgentService::call`]; the envelope is normalized as the
//!   recorder normalizes it (`<ROOT>` for the sandbox, `<VOLATILE>` for wall-clock keys) and
//!   compared with the recorded result. The recording speaks API 1.0: a `get_capabilities`
//!   answer is compared as its 1.0 view (`contract::versions::recorded_view`).
//! - Any other step is fast-forwarded: its recorded effects on the session are applied from the
//!   command log (`commands.json`): each committed command as a [`Recorded`] command that
//!   produces the recorded snapshot with the recorded label, change set and merge key, and each
//!   load, undo and redo as the session call itself. Effects the log does not show are modelled
//!   too: saves mark the session clean, file tools admit the open project's folders through the
//!   sandbox (`ctx.path`), and the ID and token counters move past what the step drew.
//!
//! Either way the session's project must then hash to the step's recorded snapshot, so every
//! live step starts from the prototype's exact session: project, version, history, dirty flag,
//! location and sandbox. The one allowance, off macOS only ([`LIBM_ALLOWED`]): a project that
//! does not hash alike may still equal the snapshot with floats within [`LIBM`] (humanized notes,
//! whose random deviations depend on the platform's libm); the report lists those steps. Each
//! step whose result carries a project `state` (create, open, get state) also has the view of
//! its project checked.
//!
//! Every tool of the contract has a handler, so every step of
//! the golden batches runs live, and [`Report::fast_forwarded`] stays empty; the tests assert
//! it. The fast-forward models remain for a registry without some handlers. Their known limits:
//! `create_project` with a `path` is not modelled; an archive save keeps the old workspace; IDs
//! that a step drew without recording them (an `analyze_audio` render names its preview file
//! with a new ID) are not skipped; job tools leave no operation behind.

pub mod allowances;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, UNIX_EPOCH};

use regex::Regex;
use serde_json::{Value, json};
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::contract::versions::recorded_view;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget};
use sonara_agent::state::{StateOptions, project_state};
use sonara_app::clock::LocalZone;
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_app::jobs::WorkerHold;
use sonara_app::midi::{MidiDevices, MidiError, OpenInput};
use sonara_app::settings::Settings;
use sonara_core::commands::{
    ChangeSet, Command, CommandContext, Flag, ObjectKind, ObjectRef, Outcome,
};
use sonara_core::error::Result as DomainResult;
use sonara_core::ids::IdGenerator;
use sonara_core::model::project::Project;
use sonara_core::session::ProjectLocation;
use sonara_project::serializer::{dumps, loads, project_from_value};
use sonara_testkit::{Golden, sha256_hex};
use tempfile::TempDir;

use self::allowances::Allowance;

/// The recorder's placeholder for the sandbox root.
pub const ROOT: &str = "<ROOT>";
/// The recorder's placeholder for wall-clock values.
pub const VOLATILE: &str = "<VOLATILE>";
/// Result keys that measure wall-clock time (`golden_batch.VOLATILE_KEYS`).
pub const VOLATILE_KEYS: [&str; 7] = [
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

static ID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[a-z][a-z0-9-]*_([0-9]{12})\b").expect("the ID pattern is valid")
});
static REF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{\s*([A-Za-z0-9_\-]+)((?:\.[A-Za-z0-9_\-]+|\[\d+\])*)\s*\}\}")
        .expect("the reference pattern is valid")
});
static REF_PART: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\.([A-Za-z0-9_\-]+)|\[(\d+)\]").expect("the reference part pattern is valid")
});

// -- the recorded batch ----------------------------------------------------------------------

/// What a step does.
#[derive(Debug, Clone)]
pub enum Action {
    /// A tool call with its resolved arguments (`<ROOT>` still in place). `confirm` repeats a
    /// call that needs confirmation once with the issued token (`run_batch`).
    Tool {
        /// The tool.
        name: String,
        /// The arguments.
        args: Value,
        /// Whether the batch pre-approved a confirmation.
        confirm: bool,
    },
    /// A coverage harness step (`hold_jobs`, `release_jobs`, `move_file`).
    Harness {
        /// The harness action.
        name: String,
        /// Its arguments.
        args: Value,
    },
}

/// One recorded step.
#[derive(Debug, Clone)]
pub struct Step {
    /// Its index in the transcript.
    pub index: usize,
    /// The batch's step ID.
    pub id: String,
    /// What it does.
    pub action: Action,
    /// The recorded (normalized) envelope; `None` for harness steps.
    pub result: Option<Value>,
    /// The hash of the project after the step.
    pub project: Option<String>,
}

impl Step {
    /// The tool of a tool step.
    pub fn tool(&self) -> Option<&str> {
        match &self.action {
            Action::Tool { name, .. } => Some(name),
            Action::Harness { .. } => None,
        }
    }
}

/// One entry of the command log, in recorded order.
#[derive(Debug, Clone)]
enum LogItem {
    Command {
        step: usize,
        command: String,
        fields: Value,
        label: String,
        changes: Value,
        dry_run: bool,
        committed: bool,
        project: Option<String>,
    },
    Load {
        step: usize,
        project: Option<String>,
        saved: bool,
        path: Option<String>,
        workspace: String,
    },
    Moved {
        step: usize,
        undo: bool,
        label: String,
    },
}

impl LogItem {
    fn step(&self) -> usize {
        match self {
            Self::Command { step, .. } | Self::Load { step, .. } | Self::Moved { step, .. } => {
                *step
            }
        }
    }
}

fn text(value: &Value, key: &str) -> String {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("{key} is a string in {value}"))
        .to_owned()
}

fn opt_text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn index(value: &Value, key: &str) -> usize {
    value[key]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or_else(|| panic!("{key} is an index in {value}"))
}

fn items<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key} is a list"))
}

/// The command log in recorded order: the events logged before command `k` (`at == k`), then
/// command `k`.
fn command_log(log: &Value) -> Vec<LogItem> {
    let commands = items(log, "commands");
    let events = items(log, "events");
    let mut out = Vec::with_capacity(commands.len() + events.len());
    for k in 0..=commands.len() {
        for event in events.iter().filter(|e| index(e, "at") == k) {
            let step = index(event, "step");
            let project = opt_text(event, "project");
            out.push(match event["event"].as_str() {
                Some("load") => LogItem::Load {
                    step,
                    project,
                    saved: event["saved"].as_bool().expect("saved is a flag"),
                    path: opt_text(event, "path"),
                    workspace: text(event, "workspace"),
                },
                Some(kind @ ("undo" | "redo")) => LogItem::Moved {
                    step,
                    undo: kind == "undo",
                    label: text(event, "label"),
                },
                other => panic!("unknown command log event {other:?}"),
            });
        }
        if let Some(entry) = commands.get(k) {
            out.push(LogItem::Command {
                step: index(entry, "step"),
                command: text(entry, "command"),
                fields: entry["fields"].clone(),
                label: text(entry, "label"),
                changes: entry["changes"].clone(),
                dry_run: entry["dry_run"].as_bool().expect("dry_run is a flag"),
                committed: entry["committed"].as_bool().expect("committed is a flag"),
                project: opt_text(entry, "project"),
            });
        }
    }
    out
}

/// `run_batch`'s `_lookup`: `name` then `.key` and `[i]` parts into the earlier results.
fn lookup(results: &HashMap<String, Value>, name: &str, path: &str) -> Value {
    let mut current = results
        .get(name)
        .unwrap_or_else(|| panic!("no earlier result {name}"))
        .clone();
    for part in REF_PART.captures_iter(path) {
        current = match (part.get(1), part.get(2)) {
            (Some(key), _) => current[key.as_str()].clone(),
            (None, Some(i)) => current[i.as_str().parse::<usize>().expect("an index")].clone(),
            (None, None) => current,
        };
    }
    current
}

/// `run_batch`'s `_substitute`: `{{id.path}}` references to earlier results. A whole-string
/// reference takes the referenced value; references inside a string are replaced by their text.
fn substitute(value: &Value, results: &HashMap<String, Value>) -> Value {
    match value {
        Value::String(s) => {
            if let Some(full) = REF
                .captures(s.trim())
                .filter(|c| c[0].len() == s.trim().len())
            {
                return lookup(results, &full[1], &full[2]);
            }
            Value::String(
                REF.replace_all(s, |c: &regex::Captures<'_>| {
                    match lookup(results, &c[1], &c[2]) {
                        Value::String(text) => text,
                        other => other.to_string(),
                    }
                })
                .into_owned(),
            )
        }
        Value::Array(list) => Value::Array(list.iter().map(|v| substitute(v, results)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), substitute(v, results)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The coverage transcript: arguments are recorded, resolved.
fn coverage_steps(transcript: &Value) -> Vec<Step> {
    items(transcript, "steps")
        .iter()
        .enumerate()
        .map(|(i, step)| {
            let action = match (opt_text(step, "tool"), opt_text(step, "harness")) {
                (Some(name), _) => Action::Tool {
                    name,
                    args: step["args"].clone(),
                    confirm: false,
                },
                (None, Some(name)) => Action::Harness {
                    name,
                    args: step["args"].clone(),
                },
                (None, None) => panic!("step {i} is neither a tool nor a harness step"),
            };
            Step {
                index: i,
                id: text(step, "id"),
                action,
                result: step.get("result").cloned(),
                project: opt_text(step, "project"),
            }
        })
        .collect()
}

/// A workflow transcript: arguments come from the example batch, with references resolved
/// against the recorded results as `run_batch` resolved them.
fn workflow_steps(transcript: &Value, example: &Value) -> Vec<Step> {
    let batch = example.get("steps").unwrap_or(example);
    let batch = batch.as_array().expect("the example lists steps");
    let recorded = items(transcript, "steps");
    assert_eq!(
        batch.len(),
        recorded.len(),
        "every example step was recorded"
    );
    let mut results = HashMap::new();
    batch
        .iter()
        .zip(recorded)
        .enumerate()
        .map(|(i, (spec, step))| {
            let name = text(spec, "tool");
            assert_eq!(name, text(step, "tool"), "step {i} runs the example's tool");
            let args = substitute(spec.get("args").unwrap_or(&json!({})), &results);
            let id = text(step, "id");
            results.insert(id.clone(), step["result"].clone());
            Step {
                index: i,
                id,
                action: Action::Tool {
                    name,
                    args,
                    confirm: spec.get("confirm").and_then(Value::as_bool) == Some(true),
                },
                result: Some(step["result"].clone()),
                project: opt_text(step, "project"),
            }
        })
        .collect()
}

// -- normalization and comparison ------------------------------------------------------------

/// `golden_batch.normalize`: the sandbox root becomes `<ROOT>` in every string and the values
/// of `volatile` keys become `<VOLATILE>`.
pub fn normalize(value: &Value, root: &str, volatile: &[&str]) -> Value {
    match value {
        Value::String(s) => Value::String(s.replace(root, ROOT)),
        Value::Array(list) => {
            Value::Array(list.iter().map(|v| normalize(v, root, volatile)).collect())
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| {
                    let v = if volatile.contains(&k.as_str()) {
                        Value::from(VOLATILE)
                    } else {
                        normalize(v, root, volatile)
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
        Value::Array(list) => Value::Array(list.iter().map(|v| rooted(v, root)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), rooted(v, root)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The first difference between two JSON values, as a JSON pointer with both values; `None`
/// when they are equal. Numbers compare as JSON numbers: `1` and `1.0` differ, as they do for
/// Python's `json` (int and float).
pub fn difference(expected: &Value, actual: &Value) -> Option<String> {
    fn walk(expected: &Value, actual: &Value, at: &str) -> Option<String> {
        match (expected, actual) {
            (Value::Object(e), Value::Object(a)) => {
                for key in e.keys().chain(a.keys().filter(|k| !e.contains_key(*k))) {
                    let here = format!("{at}/{key}");
                    let found = match (e.get(key), a.get(key)) {
                        (Some(ev), Some(av)) => walk(ev, av, &here),
                        (ev, av) => Some(format!("{here}: expected {ev:?}, got {av:?}")),
                    };
                    if found.is_some() {
                        return found;
                    }
                }
                None
            }
            (Value::Array(e), Value::Array(a)) => {
                for (i, (ev, av)) in e.iter().zip(a).enumerate() {
                    if let Some(found) = walk(ev, av, &format!("{at}/{i}")) {
                        return Some(found);
                    }
                }
                (e.len() != a.len())
                    .then(|| format!("{at}: expected {} items, got {}", e.len(), a.len()))
            }
            _ if expected == actual => None,
            _ => Some(format!("{at}: expected {expected}, got {actual}")),
        }
    }
    if expected == actual {
        return None;
    }
    walk(expected, actual, "").or_else(|| Some("(values differ)".to_owned()))
}

/// Largest relative difference allowed between a float of a recorded project snapshot and the
/// replayed project's, when the two do not hash alike (GLD-01, decided by the orchestrator like
/// NumPy's `log1p` and libsoxr).
///
/// Justification: `generate_midi` and `edit_midi_notes` humanize notes through Python's
/// `random.gauss`, whose `sin`, `cos` and `ln` come from the platform's libm. glibc's differ
/// from macOS libm (the recording platform) by one ulp on some inputs, which moves a humanized
/// note start by one ulp (cinematic step 10: `0.008696673652198627` recorded,
/// `0.008696673652198625` under glibc). `sonara-midi` checks the same generator with the same
/// allowance (`crates/sonara-midi/tests/golden_generators.rs`, `LIBM`). It applies only where
/// [`LIBM_ALLOWED`] is set; `golden_transcripts.rs` asserts that it covers only steps from a
/// humanizing `generate_midi` on.
pub const LIBM: f64 = 1e-12;

/// Whether [`LIBM`] applies: not on macOS, the product and recording platform, where every step
/// must reach its exact recorded hash.
pub const LIBM_ALLOWED: bool = !cfg!(target_os = "macos");

/// Whether two JSON documents are equal except for floats within [`LIBM`]: every key, array
/// length, string, boolean, null and integer must be identical.
pub fn within_libm(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            e.len() == a.len()
                && e.iter()
                    .all(|(k, ev)| a.get(k).is_some_and(|av| within_libm(ev, av)))
        }
        (Value::Array(e), Value::Array(a)) => {
            e.len() == a.len() && e.iter().zip(a).all(|(ev, av)| within_libm(ev, av))
        }
        (Value::Number(e), Value::Number(a)) if e.is_f64() && a.is_f64() => {
            match (e.as_f64(), a.as_f64()) {
                (Some(e), Some(a)) => (e - a).abs() <= LIBM * e.abs().max(a.abs()),
                _ => false,
            }
        }
        _ => expected == actual,
    }
}

// -- fast-forward commands -------------------------------------------------------------------

/// A recorded command: produces the recorded snapshot with the recorded label, change set and
/// merge key, so the session's version and history evolve exactly as the prototype's did.
///
/// Snapshots list assets sorted by ID (the serializer sorts them), but a project keeps them in
/// the order `with_asset` leaves: an added or replaced asset moves to the end. The state views
/// report that order, so the command restores it from the project it is applied to.
#[derive(Debug)]
struct Recorded {
    label: String,
    after: Project,
    changes: ChangeSet,
    merge_key: Option<String>,
    undoable: bool,
}

impl Command for Recorded {
    fn name(&self) -> &'static str {
        "Recorded"
    }

    fn label(&self) -> &'static str {
        "Recorded"
    }

    fn undoable(&self) -> bool {
        self.undoable
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> DomainResult<Outcome> {
        let changed: Vec<&str> = self
            .changes
            .created
            .iter()
            .chain(&self.changes.updated)
            .filter(|r| r.kind == ObjectKind::Asset)
            .map(|r| r.id.as_str())
            .collect();
        let mut order: Vec<&str> = project
            .assets
            .iter()
            .map(|a| a.id.as_str())
            .filter(|id| !changed.contains(id))
            .collect();
        order.extend(changed);
        let after = self.after.edit(|data| {
            // Stable: assets the order does not know keep their snapshot order, at the end.
            data.assets.sort_by_key(|a| {
                order
                    .iter()
                    .position(|id| *id == a.id)
                    .unwrap_or(usize::MAX)
            });
        })?;
        Ok(Outcome::new(after, self.changes.clone()))
    }

    fn merge_key(&self) -> Option<String> {
        self.merge_key.clone()
    }

    fn describe(&self) -> String {
        self.label.clone()
    }
}

const OBJECT_KINDS: [ObjectKind; 10] = [
    ObjectKind::Project,
    ObjectKind::Track,
    ObjectKind::Clip,
    ObjectKind::Note,
    ObjectKind::Device,
    ObjectKind::Send,
    ObjectKind::AutomationLane,
    ObjectKind::Marker,
    ObjectKind::Section,
    ObjectKind::Asset,
];

const FLAGS: [Flag; 15] = [
    Flag::Tracks,
    Flag::Clips,
    Flag::Notes,
    Flag::Mixer,
    Flag::Devices,
    Flag::Routing,
    Flag::Automation,
    Flag::Tempo,
    Flag::Media,
    Flag::Markers,
    Flag::Transport,
    Flag::Settings,
    Flag::Metadata,
    Flag::Editor,
    Flag::Project,
];

/// A change set from its JSON form (`ChangeSet.to_json`).
fn change_set(json: &Value) -> ChangeSet {
    let refs = |key: &str| -> Vec<ObjectRef> {
        items(json, key)
            .iter()
            .map(|r| {
                let kind = text(r, "kind");
                let kind = OBJECT_KINDS
                    .into_iter()
                    .find(|k| k.as_str() == kind)
                    .unwrap_or_else(|| panic!("unknown object kind {kind}"));
                ObjectRef::new(kind, text(r, "id"))
            })
            .collect()
    };
    ChangeSet {
        created: refs("created"),
        updated: refs("updated"),
        deleted: refs("deleted"),
        flags: items(json, "flags")
            .iter()
            .map(|f| {
                let name = f.as_str().expect("a flag name");
                FLAGS
                    .into_iter()
                    .find(|flag| flag.as_str() == name)
                    .unwrap_or_else(|| panic!("unknown flag {name}"))
            })
            .collect(),
    }
}

/// The merge key of a recorded command, from its fields. Only equality matters, so values are
/// written as JSON, which tells the same values apart as the prototype's f-strings do.
fn merge_key(command: &str, f: &Value) -> Option<String> {
    let set = |key: &str| !f[key].is_null();
    let sorted_keys = |key: &str| {
        let mut keys: Vec<&str> = f[key]
            .as_object()
            .map(|m| m.keys().map(String::as_str).collect())
            .unwrap_or_default();
        keys.sort_unstable();
        keys.join(",")
    };
    match command {
        "SetTrackMixer" => {
            let changed: Vec<&str> = ["volume_db", "pan"]
                .into_iter()
                .filter(|k| set(k))
                .collect();
            let toggles = ["mute", "solo", "solo_safe"].into_iter().any(set);
            (!changed.is_empty() && !toggles)
                .then(|| format!("mixer:{}:{}", f["track_id"], changed.join(",")))
        }
        "UpdateSend" => (set("level_db") && !set("pre_fader") && !set("enabled"))
            .then(|| format!("send:{}", f["send_id"])),
        "SetDeviceParams" => Some(format!(
            "params:{}:{}",
            f["device_id"],
            sorted_keys("values")
        )),
        "MoveClip" => Some(format!("move:{}", f["clip_id"])),
        "TrimClip" => Some(format!("trim:{}", f["clip_id"])),
        "SetClipProperties" => (set("gain_db") && !set("fade_in") && !set("fade_out"))
            .then(|| format!("clipgain:{}", f["clip_id"])),
        "EditNotes" => {
            let mut ids: Vec<String> = items(f, "edits")
                .iter()
                .map(|e| e["note_id"].to_string())
                .collect();
            ids.sort_unstable();
            Some(format!("notes:{}:{}", f["clip_id"], ids.join(",")))
        }
        "SetTempo" => Some(format!("tempo:{}", f["beat"])),
        "SetTransportSettings" => Some(format!("transport:{}", sorted_keys("values"))),
        "SetEditorState" => Some("editor".to_owned()),
        _ => None,
    }
}

// -- the replay ------------------------------------------------------------------------------

/// A sequential ID source whose position the replay can move (`deterministic_ids(seed=0)`).
#[derive(Debug, Clone)]
struct Counter(Arc<AtomicU64>);

impl IdGenerator for Counter {
    fn new_id(&mut self, prefix: &str) -> String {
        // The counter only orders IDs; no other memory depends on it.
        let n = self.0.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{prefix}_{n:012}")
    }
}

/// The highest number of a `golden-token-NNNN` token in `value`.
fn max_token(value: &Value) -> u64 {
    match value {
        Value::String(s) => s
            .strip_prefix("golden-token-")
            .and_then(|n| n.parse().ok())
            .unwrap_or(0),
        Value::Array(list) => list.iter().map(max_token).max().unwrap_or(0),
        Value::Object(map) => map.values().map(max_token).max().unwrap_or(0),
        _ => 0,
    }
}

/// The highest ID number in `text`.
fn max_id(text: &str) -> u64 {
    ID.captures_iter(text)
        .filter_map(|c| c[1].parse().ok())
        .max()
        .unwrap_or(0)
}

/// A live step and whether its result matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Live {
    /// The step index.
    pub index: usize,
    /// The batch's step ID.
    pub id: String,
    /// The tool.
    pub tool: String,
    /// `None` when the result matched; otherwise the first difference.
    pub difference: Option<String>,
    /// The normalized envelope.
    pub actual: Value,
}

/// What a replay found.
#[derive(Debug, Default)]
pub struct Report {
    /// Every live step, in order.
    pub live: Vec<Live>,
    /// The steps whose recorded `state` view was checked, and the first difference of each.
    pub states: Vec<(usize, Option<String>)>,
    /// The allowances that were applied: step, allowance name, largest scaled difference.
    pub allowances_used: Vec<(usize, &'static str, f64)>,
    /// Why the replay stopped early: the session left the recorded chain.
    pub diverged: Option<String>,
    /// The steps whose project equals the recorded snapshot only within [`LIBM`].
    pub libm_steps: Vec<usize>,
    /// The tool steps that ran without a handler, from the command log.
    pub fast_forwarded: Vec<usize>,
    /// The sandbox the batch ran in, with the files its steps wrote (removed when dropped).
    pub sandbox: Option<TempDir>,
}

impl Report {
    /// The sandbox root, resolved, as the recorder's `<ROOT>` stands for it.
    pub fn root(&self) -> String {
        let sandbox = self.sandbox.as_ref().expect("the replay keeps its sandbox");
        fs::canonicalize(sandbox.path())
            .expect("the sandbox resolves")
            .to_str()
            .expect("a UTF-8 sandbox path")
            .to_owned()
    }

    /// The file `rel` under the sandbox's `work` folder (the batch's base directory).
    pub fn work_file(&self, rel: &str) -> PathBuf {
        Path::new(&self.root()).join("work").join(rel)
    }

    /// Every difference found, one line each.
    pub fn failures(&self, name: &str) -> Vec<String> {
        let mut out: Vec<String> = self
            .live
            .iter()
            .filter_map(|l| {
                l.difference
                    .as_ref()
                    .map(|d| format!("{name} step {} {} ({}): {d}", l.index, l.id, l.tool))
            })
            .collect();
        out.extend(
            self.states
                .iter()
                .filter_map(|(i, d)| d.as_ref().map(|d| format!("{name} step {i} state: {d}"))),
        );
        out.extend(self.diverged.iter().map(|d| format!("{name}: {d}")));
        out
    }

    /// How many live steps ran per tool.
    pub fn live_counts(&self) -> HashMap<&str, usize> {
        let mut counts = HashMap::new();
        for live in &self.live {
            *counts.entry(live.tool.as_str()).or_insert(0) += 1;
        }
        counts
    }
}

/// The tools that pass their `path`-like argument through `ctx.path` before changing anything,
/// with that argument (fast-forward model of the sandbox side effect).
const PATH_TOOLS: [(&str, &str); 12] = [
    ("open_project", "path"),
    ("save_project", "path"),
    ("import_audio", "path"),
    ("import_midi", "path"),
    ("load_impulse_response", "path"),
    ("render_preview", "path"),
    ("export_audio", "path"),
    ("export_stems", "directory"),
    ("export_midi", "path"),
    ("analyze_audio", "path"),
    ("get_audio_analysis", "path"),
    ("relink_media", "path"),
];

/// One golden batch, replayed.
pub struct Replay {
    name: String,
    golden: Golden,
    base: String,
    steps: Vec<Step>,
    log: Vec<LogItem>,
    sandbox: TempDir,
    root: String,
    root_json: String,
    service: AgentService,
    app: Arc<AppContext>,
    ids: Arc<AtomicU64>,
    tokens: Arc<AtomicU64>,
    volatile: Vec<&'static str>,
    allowances: Vec<Allowance>,
    snapshots: HashMap<String, Project>,
    held: Option<WorkerHold>,
}

impl Replay {
    /// The coverage batch (`golden/coverage/`), whose `project_sha256` values are volatile too.
    pub fn coverage() -> Self {
        let golden = Golden::open().expect("golden data is available");
        let transcript: Value = golden
            .json("coverage/transcript.json")
            .expect("the coverage transcript");
        let steps = coverage_steps(&transcript);
        let mut volatile = VOLATILE_KEYS.to_vec();
        volatile.push("project_sha256");
        Self::new("coverage", golden, "coverage".to_owned(), steps, volatile)
    }

    /// The workflow `name` (`golden/workflows/<name>/`, from `examples/<name>.json`).
    pub fn workflow(name: &str) -> Self {
        let golden = Golden::open().expect("golden data is available");
        let base = format!("workflows/{name}");
        let transcript: Value = golden
            .json(&format!("{base}/transcript.json"))
            .expect("the workflow transcript");
        let example_path = golden
            .root()
            .parent()
            .expect("golden/ lies in the repository")
            .join(text(&transcript, "workflow"));
        let example: Value = serde_json::from_slice(
            &fs::read(&example_path).unwrap_or_else(|e| panic!("{}: {e}", example_path.display())),
        )
        .expect("the example is JSON");
        let steps = workflow_steps(&transcript, &example);
        Self::new(name, golden, base, steps, VOLATILE_KEYS.to_vec())
    }

    /// The names of every recorded workflow.
    pub fn workflows() -> Vec<String> {
        let golden = Golden::open().expect("golden data is available");
        let mut names: Vec<String> = fs::read_dir(golden.root().join("workflows"))
            .expect("golden/workflows")
            .map(|entry| entry.expect("a directory entry").file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn new(
        name: &str,
        golden: Golden,
        base: String,
        steps: Vec<Step>,
        volatile: Vec<&'static str>,
    ) -> Self {
        let log: Value = golden
            .json(&format!("{base}/commands.json"))
            .expect("the command log");
        let log = command_log(&log);
        let sandbox = tempfile::Builder::new()
            .prefix("sonara-golden-")
            .tempdir()
            .expect("a sandbox");
        let root_path = fs::canonicalize(sandbox.path()).expect("the sandbox resolves");
        for sub in ["home", "projects", "work", "workspaces"] {
            fs::create_dir(root_path.join(sub)).expect("a sandbox folder");
        }
        install_fixtures(&golden, &root_path.join("inputs"));
        let root = root_path.to_str().expect("a UTF-8 sandbox path").to_owned();
        let root_json = serde_json::to_string(&root).expect("a path string");
        let root_json = root_json
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .expect("a quoted string")
            .to_owned();
        let ids = Arc::new(AtomicU64::new(0));
        let tokens = Arc::new(AtomicU64::new(0));
        let app = Arc::new(
            AppContext::create(AppOptions {
                workspaces: Some(root_path.join("workspaces")),
                settings: Some(Settings::default()),
                persist_settings: false,
                audio_backend: Some("null".to_owned()),
                ids: Some(IdSource::new(Box::new(Counter(Arc::clone(&ids))))),
                wall_clock: Some(Arc::new(|| {
                    UNIX_EPOCH + Duration::from_secs(FROZEN_UNIX_SECONDS)
                })),
                // The recorder's frozen `time.strftime` formats its moment in UTC.
                local_zone: LocalZone::Utc,
                undo_clock: Some(Arc::new(|| 0.0)),
                midi_devices: Some(Arc::new(RecordingHostMidi)),
                // The recorder treated FFmpeg as absent (`audio_import.ffmpeg_path`).
                ffmpeg: Some(Arc::new(|| None)),
                ..AppOptions::default()
            })
            .expect("the application"),
        );
        let token_counter = Arc::clone(&tokens);
        let service = AgentService::new(
            Arc::clone(&app),
            AgentOptions {
                base_dir: Some(root_path.join("work")),
                audit: AuditTarget::Disabled,
                allowed_paths: vec![root_path.clone()],
                tokens: Some(Arc::new(move || {
                    // Orders tokens only.
                    let n = token_counter.fetch_add(1, Ordering::Relaxed) + 1;
                    format!("golden-token-{n:04}")
                })),
                environment: Some(PolicyEnvironment {
                    allowed: None,
                    projects_dir: root_path.join("projects"),
                }),
                ..AgentOptions::default()
            },
        )
        .expect("the agent service");
        Self {
            name: name.to_owned(),
            golden,
            base,
            steps,
            log,
            sandbox,
            root,
            root_json,
            service,
            app,
            ids,
            tokens,
            volatile,
            allowances: Vec::new(),
            snapshots: HashMap::new(),
            held: None,
        }
    }

    /// The batch's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The recorded steps.
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// Declares a documented tolerance of some recorded floats ([`Allowance`]).
    pub fn allow(&mut self, allowance: Allowance) {
        self.allowances.push(allowance);
    }

    /// The golden folder of the batch (`coverage` or `workflows/<name>`).
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Replays every step.
    pub fn run(mut self) -> Report {
        let mut report = Report::default();
        for i in 0..self.steps.len() {
            let step = self.steps[i].clone();
            let live = match &step.action {
                Action::Harness { name, args } => {
                    self.harness(name, &rooted(args, &self.root));
                    false
                }
                Action::Tool {
                    name,
                    args,
                    confirm,
                } => {
                    let args = rooted(args, &self.root);
                    if self.service.registry().contains(name) {
                        let (difference, actual) =
                            self.live(&step, name, &args, *confirm, &mut report);
                        report.live.push(Live {
                            index: i,
                            id: step.id.clone(),
                            tool: name.clone(),
                            difference,
                            actual,
                        });
                        true
                    } else {
                        self.fast_forward(&step, name, &args);
                        report.fast_forwarded.push(i);
                        false
                    }
                }
            };
            match self.check_project(&step) {
                Ok(true) => {}
                Ok(false) => report.libm_steps.push(i),
                Err(diverged) => {
                    assert!(live, "fast-forward left the recorded chain: {diverged}");
                    report.diverged = Some(diverged);
                    break;
                }
            }
            if let Some(state) = self.state_check(&step) {
                report.states.push((i, state.err()));
            }
            if !live {
                self.sync_counters(&step);
            }
        }
        // The app's jobs are done (every job step waited or was cancelled); the files stay.
        self.app.close();
        report.sandbox = Some(self.sandbox);
        report
    }

    // -- live steps ------------------------------------------------------------------------

    fn live(
        &self,
        step: &Step,
        tool: &str,
        args: &Value,
        confirm: bool,
        report: &mut Report,
    ) -> (Option<String>, Value) {
        let mut result = self.service.call(tool, Some(args));
        if confirm && result["error"]["code"] == "CONFIRMATION_REQUIRED" {
            let mut args = args.clone();
            args["confirmation_token"] = result["error"]["details"]["confirmation_token"].clone();
            result = self.service.call(tool, Some(&args));
        }
        let mut actual = normalize(&result, &self.root, &self.volatile);
        if tool == "get_capabilities" {
            // The recording speaks API 1.0; compare the 1.0 view of this build's answer, which
            // differs from it by exactly the recorded additions of `contract::versions` (CTR-01).
            actual = recorded_view(&actual);
        }
        let expected = step.result.as_ref().expect("a tool step has a result");
        // A saved project that equals the snapshot only within LIBM cannot hash alike (off macOS
        // only: there `check_project` is strict).
        if let (Some(recorded), Some(slot)) = (
            expected.get("project_sha256"),
            actual.get_mut("project_sha256"),
        ) && recorded != slot
            && self.check_project(step) == Ok(false)
        {
            *slot = recorded.clone();
        }
        for allowance in self.allowances.iter().filter(|a| a.covers(step.index)) {
            if let Some(used) = allowance.apply(expected, &mut actual) {
                report
                    .allowances_used
                    .push((step.index, allowance.name, used));
            }
        }
        (difference(expected, &actual), actual)
    }

    // -- fast-forward ----------------------------------------------------------------------

    fn fast_forward(&mut self, step: &Step, tool: &str, args: &Value) {
        let result = step.result.as_ref().expect("a tool step has a result");
        let ok = result["ok"] == true;
        let validated = !result["error"]["message"]
            .as_str()
            .is_some_and(|m| m.starts_with("invalid arguments for "));
        let path_arg = PATH_TOOLS
            .iter()
            .find(|(name, _)| *name == tool)
            .is_some_and(|(_, key)| args.get(*key).is_some_and(|v| !v.is_null()));
        if path_arg && validated {
            self.admit_project_folders();
        }
        let items: Vec<LogItem> = self
            .log
            .iter()
            .filter(|item| item.step() == step.index)
            .cloned()
            .collect();
        for item in items {
            self.apply(item);
        }
        match tool {
            "save_project"
                if ok
                    && result["committed"] == true
                    && args.get("as_copy") != Some(&Value::Bool(true)) =>
            {
                self.mark_saved(result);
            }
            "create_project" if ok && args.get("path").is_some_and(|p| !p.is_null()) => {
                panic!(
                    "{} step {}: create_project with a path is not modelled; attach its handler",
                    self.name, step.index
                );
            }
            _ => {}
        }
    }

    /// `ctx.path`'s side effect: the open project's workspace and folder become allowed.
    fn admit_project_folders(&self) {
        let location = self.app.session().lock().location().cloned();
        if let Some(location) = location {
            let ctx = self.service.context();
            ctx.allow(&location.workspace);
            if let Some(path) = &location.path {
                ctx.allow(path.parent().unwrap_or(path));
            }
        }
    }

    fn apply(&mut self, item: LogItem) {
        match item {
            LogItem::Command {
                step,
                command,
                fields,
                label,
                changes,
                dry_run,
                committed,
                project,
            } => {
                if dry_run || !committed {
                    return;
                }
                let hash = project.expect("a committed command has a snapshot");
                let recorded = Recorded {
                    merge_key: merge_key(&command, &fields),
                    undoable: command != "SetEditorState",
                    after: self.snapshot(&hash),
                    changes: change_set(&changes),
                    label,
                };
                let result = self
                    .app
                    .session()
                    .lock()
                    .execute(&recorded, false, "agent")
                    .unwrap_or_else(|e| panic!("step {step}: recorded {command} failed: {e}"));
                assert!(
                    result.committed,
                    "step {step}: recorded {command} committed"
                );
            }
            LogItem::Load {
                project,
                saved,
                path,
                workspace,
                ..
            } => {
                let project = self.snapshot(&project.expect("a load has a snapshot"));
                let path = path.map(|p| PathBuf::from(p.replace(ROOT, &self.root)));
                let workspace = PathBuf::from(workspace.replace(ROOT, &self.root));
                let packaged = path.as_ref().is_some_and(|p| *p != workspace);
                let location = ProjectLocation {
                    path,
                    workspace,
                    packaged,
                };
                self.app.session().lock().load(project, location, saved);
            }
            LogItem::Moved { step, undo, label } => {
                let mut session = self.app.session().lock();
                let result = if undo {
                    session.undo("agent")
                } else {
                    session.redo("agent")
                };
                let result =
                    result.unwrap_or_else(|e| panic!("step {step}: recorded move failed: {e}"));
                assert_eq!(result.label, label, "step {step}: the recorded move");
            }
        }
    }

    /// `ProjectService.save`'s effect on the session: clean, at the saved location.
    fn mark_saved(&self, result: &Value) {
        let path = PathBuf::from(text(result, "path").replace(ROOT, &self.root));
        let archive = result["layout"] == "archive";
        let mut session = self.app.session().lock();
        let project = session.require_project().expect("a saved project").clone();
        let workspace = if archive {
            session
                .location()
                .map(|l| l.workspace.clone())
                .expect("an open project has a location")
        } else {
            path.clone()
        };
        session.mark_saved(
            project,
            ProjectLocation {
                path: Some(path),
                workspace,
                packaged: archive,
            },
        );
    }

    /// Moves the ID and token counters past everything recorded up to this step.
    fn sync_counters(&mut self, step: &Step) {
        let mut seen = 0;
        if let Some(result) = &step.result {
            seen = seen.max(max_id(&result.to_string()));
            self.tokens.fetch_max(max_token(result), Ordering::Relaxed);
        }
        let (Action::Tool { args, .. } | Action::Harness { args, .. }) = &step.action;
        seen = seen.max(max_id(&args.to_string()));
        for item in self.log.iter().filter(|item| item.step() == step.index) {
            seen = seen.max(max_id(&format!("{item:?}")));
        }
        if let Some(hash) = &step.project {
            let bytes = self.snapshot_bytes(hash);
            seen = seen.max(max_id(&String::from_utf8_lossy(&bytes)));
        }
        self.ids.fetch_max(seen, Ordering::Relaxed);
    }

    // -- harness steps ---------------------------------------------------------------------

    fn harness(&mut self, name: &str, args: &Value) {
        match name {
            "hold_jobs" => self.held = Some(self.app.jobs().hold_workers()),
            "release_jobs" => {
                if let Some(held) = self.held.take() {
                    held.release();
                }
            }
            "move_file" => {
                let from = PathBuf::from(text(args, "from"));
                let to = PathBuf::from(text(args, "to"));
                fs::create_dir_all(to.parent().expect("a target folder")).expect("the folder");
                fs::rename(&from, &to).expect("the file moves");
            }
            other => panic!("unknown harness step {other}"),
        }
    }

    // -- snapshots -------------------------------------------------------------------------

    fn snapshot_bytes(&self, hash: &str) -> Vec<u8> {
        self.golden
            .bytes_gz(&format!("{}/snapshots/{hash}.json.gz", self.base))
            .unwrap_or_else(|e| panic!("snapshot {hash}: {e}"))
    }

    /// The recorded project `hash`, with `<ROOT>` put back. Each call returns a new snapshot
    /// object: a command that rebuilt an equal project still committed in the prototype, and a
    /// shared object would look unchanged to the session.
    fn snapshot(&mut self, hash: &str) -> Project {
        if let Some(project) = self.snapshots.get(hash) {
            return Project::new(project.to_data()).expect("a recorded snapshot stays valid");
        }
        let text = String::from_utf8(self.snapshot_bytes(hash)).expect("UTF-8 JSON");
        let text = text.replace(ROOT, &self.root_json);
        let data = loads(text.as_bytes()).expect("a project document");
        let project = project_from_value(&data).expect("a valid project");
        self.snapshots.insert(hash.to_owned(), project.clone());
        project
    }

    /// The hash the recorder gives `project` (`Snapshots.digest`).
    fn digest(&self, project: &Project) -> String {
        let text = String::from_utf8(dumps(project)).expect("UTF-8 JSON");
        sha256_hex(text.replace(&self.root_json, ROOT).as_bytes())
    }

    /// Whether the session's project is the step's recorded snapshot: `Ok(true)` when it hashes
    /// to it, `Ok(false)` when it equals it within [`LIBM`] (never on macOS, [`LIBM_ALLOWED`]).
    fn check_project(&self, step: &Step) -> Result<bool, String> {
        let project = self.app.session().project();
        let actual = project.as_ref().map(|p| self.digest(p));
        if actual == step.project {
            return Ok(true);
        }
        if LIBM_ALLOWED && let (Some(project), Some(hash)) = (&project, &step.project) {
            let text = String::from_utf8(dumps(project)).expect("UTF-8 JSON");
            let ours: Value = serde_json::from_str(&text.replace(&self.root_json, ROOT))
                .expect("a project document");
            let recorded: Value =
                serde_json::from_slice(&self.snapshot_bytes(hash)).expect("a recorded snapshot");
            if within_libm(&recorded, &ours) {
                return Ok(false);
            }
        }
        Err(format!(
            "after step {} ({:?}) the project hashes to {actual:?}, recorded {:?}",
            step.index,
            step.tool(),
            step.project
        ))
    }

    /// The state view of the step's project against a recorded `result.state`, if any. The
    /// project is the session's, which [`Self::check_project`] has just matched with the
    /// step's snapshot; it also keeps the asset order that the snapshot cannot carry.
    fn state_check(&self, step: &Step) -> Option<Result<(), String>> {
        let result = step.result.as_ref()?;
        let recorded = result.get("state")?;
        let project = self
            .app
            .session()
            .project()
            .expect("a recorded state describes the open project");
        let (full, include_notes, track_ids) = match &step.action {
            Action::Tool { name, args, .. } if name == "get_project_state" => (
                args.get("detail") == Some(&json!("full")),
                args.get("include_notes") == Some(&json!(true)),
                args.get("track_ids").and_then(Value::as_array).map(|ids| {
                    ids.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                }),
            ),
            _ => (false, false, None),
        };
        let view = project_state(
            &project,
            self.app.plugins(),
            StateOptions {
                full,
                include_notes,
                track_ids: track_ids.as_deref(),
            },
        );
        Some(
            difference(recorded, &normalize(&view, &self.root, &self.volatile)).map_or(Ok(()), Err),
        )
    }
}

/// The coverage batch's input files (`fixtures.install`): every file of `golden/fixtures`
/// except its index.
fn install_fixtures(golden: &Golden, inputs: &Path) {
    fs::create_dir_all(inputs).expect("the inputs folder");
    let source = golden.root().join("fixtures");
    for entry in fs::read_dir(&source).expect("golden/fixtures") {
        let entry = entry.expect("a fixture");
        let name = entry.file_name();
        if name == "index.json" {
            continue;
        }
        let rel = format!("fixtures/{}", name.to_string_lossy());
        fs::write(inputs.join(&name), golden.bytes(&rel).expect("the fixture"))
            .expect("the fixture is installed");
    }
}

/// The MIDI system of the machine that recorded the golden batches: CoreMIDI (`"core"`) with no
/// ports. The recorder pinned the port listings to empty (`area_coverage.py`, as
/// `area_transport.py` does), so the replay pins them too, like the frozen clock and the UTC
/// zone: a machine with MIDI devices still replays the recorded answers.
struct RecordingHostMidi;

impl MidiDevices for RecordingHostMidi {
    fn api_name(&self) -> &'static str {
        "core"
    }

    fn input_ports(&self) -> Result<Vec<String>, MidiError> {
        Ok(Vec::new())
    }

    fn output_ports(&self) -> Result<Vec<String>, MidiError> {
        Ok(Vec::new())
    }

    fn open_input(&self, name: &str, _port: u8) -> Result<OpenInput, MidiError> {
        Err(MidiError::InputNotFound {
            name: name.to_owned(),
            available: Vec::new(),
        })
    }

    fn open_virtual_input(&self, name: &str, _port: u8) -> Result<OpenInput, MidiError> {
        Err(MidiError::Connect {
            name: name.to_owned(),
            reason: "the recording host's MIDI ports are pinned".to_owned(),
        })
    }
}
