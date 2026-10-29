//! Conversational orchestration (SON-P13): a request in plain language becomes a plan of calls to
//! the existing agent tools, and the plan becomes a report of what changed.
//!
//! It is a library layer over the tool contract and adds no tool, schema or error code (CTR-01).
//! It never touches the project directly: every effect is one tool call through a [`ToolRunner`],
//! so the tools' validation, permissions and audit log apply to each step. No language model is
//! involved; the planner is deterministic and understands three kinds of request:
//!
//! - **tempo**: "set the tempo to 96 bpm" calls `set_tempo`;
//! - **instrument**: "use Poly Synth on Keys" calls `set_instrument` for the one instrument
//!   plugin and the one track the request names;
//! - **delete a track**: "delete track Drums" calls `delete_track`, which is destructive.
//!
//! Several requests can be joined with `and`, `then`, `,` or `;`.
//!
//! # Safety
//!
//! - A request that cannot be resolved to exactly one plan (an unknown phrase, a missing or
//!   out-of-range number, a track or instrument that is not found or not unique) becomes a list
//!   of questions ([`Status::NeedsClarification`]). Nothing is called that changes the project.
//! - A plan with a destructive step is only proposed ([`Status::NeedsConfirmation`]) until the
//!   caller runs it with [`Approval::Confirmed`]. Confirming applies to the plan as shown.
//! - Before any step is committed, every step is validated with the tool's `dry_run`. A plan that
//!   fails validation changes nothing ([`Status::Rejected`]); a step that fails while the plan
//!   runs stops it and the report lists what was already done ([`Status::Failed`]).
//! - When the tool layer asks for its own confirmation token for a confirmed destructive step,
//!   the step is repeated with that token: the caller's approval is the user's agreement.

use serde::Serialize;
use serde_json::{Value, json};

use crate::service::AgentService;

/// Where the orchestrator's tool calls go: [`AgentService`] in the application, a stub in tests.
pub trait ToolRunner {
    /// Calls the tool `tool`; always a result envelope (`ok`, and `error` on failure).
    fn call(&self, tool: &str, args: &Value) -> Value;
}

impl ToolRunner for AgentService {
    fn call(&self, tool: &str, args: &Value) -> Value {
        AgentService::call(self, tool, Some(args))
    }
}

/// The tempo range `set_tempo` accepts (BPM). A number outside it is a question, not a call.
const TEMPO_RANGE: std::ops::RangeInclusive<f64> = 10.0..=999.0;

/// Whether the caller has agreed to the destructive steps of a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approval {
    /// The caller has not confirmed: a destructive plan is only proposed.
    Pending,
    /// The caller has seen the plan and confirmed it.
    Confirmed,
}

/// One planned tool call.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Step {
    /// The tool to call.
    pub tool: String,
    /// The arguments of the call, without `dry_run`.
    pub args: Value,
    /// What the step does, in a sentence.
    pub description: String,
    /// Whether the step deletes or discards something.
    pub destructive: bool,
}

/// What a request resolves to: steps to take, or questions to ask first.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Plan {
    /// The request as given.
    pub request: String,
    /// The calls to make, in order. Empty when there are questions.
    pub steps: Vec<Step>,
    /// What the request left open. Not empty means the plan is not to be run.
    pub questions: Vec<String>,
}

impl Plan {
    /// Whether any step is destructive.
    #[must_use]
    pub fn is_destructive(&self) -> bool {
        self.steps.iter().any(|step| step.destructive)
    }
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Every step was applied.
    Done,
    /// The request is ambiguous; nothing was changed.
    NeedsClarification,
    /// The plan is destructive and not yet confirmed; nothing was changed.
    NeedsConfirmation,
    /// Validation refused a step; nothing was changed.
    Rejected,
    /// A step failed while the plan ran; the earlier steps stay applied (and are undoable).
    Failed,
}

/// The outcome of one executed step.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepResult {
    /// The step.
    pub step: Step,
    /// Whether the tool call succeeded.
    pub ok: bool,
    /// The error code when it did not.
    pub error: Option<String>,
}

/// The report of a run: the plan, what was executed and a summary for the user.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// How the run ended.
    pub status: Status,
    /// The plan the request resolved to.
    pub plan: Plan,
    /// The steps that were committed or that failed, in order.
    pub executed: Vec<StepResult>,
    /// A plain-language summary of the plan and what happened.
    pub summary: String,
}

/// Plans requests and runs the plans through a [`ToolRunner`].
pub struct Orchestrator<R> {
    runner: R,
}

impl<R: ToolRunner> Orchestrator<R> {
    /// An orchestrator that calls its tools through `runner`.
    #[must_use]
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    /// Resolves `request` against the open project. Only read tools are called.
    #[must_use]
    pub fn plan(&self, request: &str) -> Plan {
        let mut plan = Plan {
            request: request.to_owned(),
            steps: Vec::new(),
            questions: Vec::new(),
        };
        let clauses = split_clauses(request);
        if clauses.is_empty() {
            plan.questions
                .push("What would you like to change?".to_owned());
            return plan;
        }
        let tracks = self.read("list_tracks", "tracks");
        let plugins = self.read("list_plugins", "plugins");
        for clause in &clauses {
            match interpret(clause, &tracks, &plugins) {
                Ok(step) => plan.steps.push(step),
                Err(question) => plan.questions.push(question),
            }
        }
        if !plan.questions.is_empty() {
            plan.steps.clear();
        }
        plan
    }

    /// Plans `request` and, when it is clear, validated and approved, applies it.
    #[must_use]
    pub fn run(&self, request: &str, approval: Approval) -> Report {
        let plan = self.plan(request);
        self.execute(plan, approval)
    }

    /// Applies a plan from [`Orchestrator::plan`] under the rules in the module documentation.
    #[must_use]
    pub fn execute(&self, plan: Plan, approval: Approval) -> Report {
        if !plan.questions.is_empty() {
            let summary = format!(
                "I need to know more before changing anything: {}",
                plan.questions.join(" ")
            );
            return finish(Status::NeedsClarification, plan, Vec::new(), summary);
        }
        if plan.is_destructive() && approval != Approval::Confirmed {
            let summary = format!(
                "This plan removes something, so nothing was changed. Please confirm: {}",
                describe(&plan.steps)
            );
            return finish(Status::NeedsConfirmation, plan, Vec::new(), summary);
        }
        // Validate the whole plan before the first commit.
        for step in &plan.steps {
            let reply = self
                .runner
                .call(&step.tool, &with_flag(&step.args, "dry_run", json!(true)));
            if !is_ok(&reply) {
                let code = error_code(&reply);
                let summary = format!(
                    "Nothing was changed: '{}' was refused ({}).",
                    step.description,
                    code.as_deref().unwrap_or("unknown error")
                );
                let failed = StepResult {
                    step: step.clone(),
                    ok: false,
                    error: code,
                };
                return finish(Status::Rejected, plan, vec![failed], summary);
            }
        }
        let mut executed = Vec::new();
        for step in &plan.steps {
            let reply = self.commit(step);
            let ok = is_ok(&reply);
            executed.push(StepResult {
                step: step.clone(),
                ok,
                error: error_code(&reply),
            });
            if !ok {
                let summary = format!(
                    "Stopped at '{}' ({}). Applied before that: {}.",
                    step.description,
                    executed
                        .last()
                        .and_then(|r| r.error.as_deref())
                        .unwrap_or("unknown error"),
                    applied(&executed),
                );
                return finish(Status::Failed, plan, executed, summary);
            }
        }
        let summary = format!("Done: {}.", applied(&executed));
        finish(Status::Done, plan, executed, summary)
    }

    /// One committed call; a destructive step repeats with the tool layer's token.
    fn commit(&self, step: &Step) -> Value {
        let reply = self.runner.call(&step.tool, &step.args);
        if step.destructive
            && error_code(&reply).as_deref() == Some("CONFIRMATION_REQUIRED")
            && let Some(token) = reply["error"]["details"]["confirmation_token"].as_str()
        {
            return self.runner.call(
                &step.tool,
                &with_flag(&step.args, "confirmation_token", json!(token)),
            );
        }
        reply
    }

    /// The list under `key` of a read tool's reply; empty when the call fails (no project).
    fn read(&self, tool: &str, key: &str) -> Vec<Value> {
        let reply = self.runner.call(tool, &json!({}));
        reply[key].as_array().cloned().unwrap_or_default()
    }
}

fn finish(status: Status, plan: Plan, executed: Vec<StepResult>, summary: String) -> Report {
    Report {
        status,
        plan,
        executed,
        summary,
    }
}

fn is_ok(reply: &Value) -> bool {
    reply["ok"] == Value::Bool(true)
}

fn error_code(reply: &Value) -> Option<String> {
    reply["error"]["code"].as_str().map(str::to_owned)
}

fn with_flag(args: &Value, key: &str, value: Value) -> Value {
    let mut out = args.clone();
    out[key] = value;
    out
}

fn describe(steps: &[Step]) -> String {
    steps
        .iter()
        .map(|s| s.description.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

fn applied(executed: &[StepResult]) -> String {
    let done: Vec<&str> = executed
        .iter()
        .filter(|r| r.ok)
        .map(|r| r.step.description.as_str())
        .collect();
    if done.is_empty() {
        "no changes".to_owned()
    } else {
        done.join("; ")
    }
}

// -- understanding a request ----------------------------------------------------------------

/// The request split into clauses at `and`, `then`, commas and semicolons.
fn split_clauses(request: &str) -> Vec<String> {
    let spaced = request
        .replace([',', ';'], " | ")
        .replace(" and then ", " | ");
    let mut clauses = vec![String::new()];
    for word in spaced.split_whitespace() {
        let lower = word.to_lowercase();
        if word == "|" || lower == "and" || lower == "then" {
            clauses.push(String::new());
        } else {
            let current = clauses.last_mut().expect("a clause");
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
    }
    clauses.retain(|c| !c.is_empty());
    clauses
}

fn interpret(clause: &str, tracks: &[Value], plugins: &[Value]) -> Result<Step, String> {
    let lower = clause.to_lowercase();
    let words = words(&lower);
    let has = |w: &str| words.iter().any(|x| x == w);
    if has("tempo") || has("bpm") {
        return tempo_step(clause, &words);
    }
    if (has("delete") || has("remove")) && has("track") {
        return delete_step(clause, &lower, tracks);
    }
    if let Some(plugin) = find_unique(&lower, plugins, "name", "instrument", |p| {
        p["kind"] == "instrument"
    })? {
        return instrument_step(clause, &lower, tracks, plugin);
    }
    Err(format!(
        "I did not understand '{clause}'. I can set the tempo, set a track's instrument or delete a track."
    ))
}

/// The lower-case words of a clause with surrounding punctuation removed.
fn words(lower: &str) -> Vec<String> {
    lower
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '-')
                .to_owned()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

fn tempo_step(clause: &str, words: &[String]) -> Result<Step, String> {
    let numbers: Vec<f64> = words
        .iter()
        .filter_map(|w| w.strip_suffix("bpm").unwrap_or(w).parse::<f64>().ok())
        .collect();
    match numbers.as_slice() {
        [bpm] if TEMPO_RANGE.contains(bpm) => Ok(Step {
            tool: "set_tempo".to_owned(),
            args: json!({"bpm": bpm}),
            description: format!("set the tempo to {bpm} BPM"),
            destructive: false,
        }),
        [bpm] => Err(format!(
            "{bpm} BPM is outside {}–{}. Which tempo do you want?",
            TEMPO_RANGE.start(),
            TEMPO_RANGE.end()
        )),
        [] => Err(format!("Which tempo, in BPM, do you want for '{clause}'?")),
        _ => Err(format!(
            "'{clause}' names several tempos. Which one do you want?"
        )),
    }
}

fn delete_step(clause: &str, lower: &str, tracks: &[Value]) -> Result<Step, String> {
    let track = find_unique(lower, tracks, "name", "track", |_| true)?
        .ok_or_else(|| format!("Which track should I delete for '{clause}'?"))?;
    let name = track["name"].as_str().unwrap_or_default();
    Ok(Step {
        tool: "delete_track".to_owned(),
        args: json!({"track_id": track["track_id"]}),
        description: format!("delete the track '{name}'"),
        destructive: true,
    })
}

fn instrument_step(
    clause: &str,
    lower: &str,
    tracks: &[Value],
    plugin: &Value,
) -> Result<Step, String> {
    let track = find_unique(lower, tracks, "name", "track", |t| {
        t["kind"] == "instrument"
    })?
    .ok_or_else(|| format!("Which instrument track should get that sound for '{clause}'?"))?;
    let (track_name, plugin_name) = (
        track["name"].as_str().unwrap_or_default(),
        plugin["name"].as_str().unwrap_or_default(),
    );
    Ok(Step {
        tool: "set_instrument".to_owned(),
        args: json!({"track_id": track["track_id"], "plugin_id": plugin["plugin_id"]}),
        description: format!("use {plugin_name} on the track '{track_name}'"),
        destructive: false,
    })
}

/// The one item of `items` (that passes `keep`) whose `key` the clause names. `None` when none
/// does; an error question when several unrelated ones do. A name inside a longer name that also
/// matches loses to the longer one ("Keys" and "Keys 2").
fn find_unique<'a>(
    lower: &str,
    items: &'a [Value],
    key: &str,
    what: &str,
    keep: impl Fn(&Value) -> bool,
) -> Result<Option<&'a Value>, String> {
    let mut found: Vec<(&Value, String)> = items
        .iter()
        .filter(|item| keep(item))
        .filter_map(|item| {
            let name = item[key].as_str()?.to_lowercase();
            names_phrase(lower, &name).then_some((item, name))
        })
        .collect();
    found.retain(|(_, name)| !found_longer(name, lower, items, key, &keep));
    match found.as_slice() {
        [] => Ok(None),
        [(item, _)] => Ok(Some(*item)),
        several => {
            let names: Vec<&str> = several
                .iter()
                .filter_map(|(i, _)| i[key].as_str())
                .collect();
            Err(format!(
                "The request matches several {what}s ({}). Which one do you mean?",
                names.join(", ")
            ))
        }
    }
}

/// Whether another matching item has a longer name that contains `name`.
fn found_longer(
    name: &str,
    lower: &str,
    items: &[Value],
    key: &str,
    keep: &impl Fn(&Value) -> bool,
) -> bool {
    items.iter().filter(|item| keep(item)).any(|item| {
        item[key]
            .as_str()
            .map(str::to_lowercase)
            .is_some_and(|other| {
                other.len() > name.len() && other.contains(name) && names_phrase(lower, &other)
            })
    })
}

/// Whether `phrase` occurs in `text` between word boundaries.
fn names_phrase(text: &str, phrase: &str) -> bool {
    if phrase.is_empty() {
        return false;
    }
    text.match_indices(phrase).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + phrase.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}
