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
//! [`grammar`] reads requests that give instruments something to play, in English and Czech
//! ("strings play a legato melody, horns and timpani underneath"), into clauses and the questions
//! they leave open; [`Orchestrator::plan`] reads only the three kinds above.
//!
//! A step of a plan can take a value from the reply of an earlier step, such as the id of the
//! track that `create_track` makes ([`StepRef`]). Its argument holds a placeholder until the
//! earlier step has run.
//!
//! # Safety
//!
//! - A request that cannot be resolved to exactly one plan (an unknown phrase, a missing or
//!   out-of-range number, a track or instrument that is not found or not unique, the tempo set
//!   twice) becomes a list of questions ([`Status::NeedsClarification`]). Nothing is called that
//!   changes the project.
//! - A plan with a destructive step is only proposed ([`Status::NeedsConfirmation`]) until the
//!   caller runs it with [`Approval::Confirmed`]. Confirming applies to the plan as shown.
//! - Validation runs in stages, because a step that uses a value of an earlier step cannot be
//!   dry-run before that step has committed. Before the first commit, every reference must name
//!   an earlier step and sit in its argument, every step with references must pass its tool's
//!   input validation with the placeholders in place, and every step without references must
//!   pass the tool's `dry_run` against the project as it is. A refusal here changes nothing
//!   ([`Status::Rejected`]).
//! - The plan then commits stage by stage. A stage ends before the first step that needs a value
//!   from a step of the same stage. Every stage after the first has its references filled from
//!   the earlier replies and is dry-run as a whole against the project as the earlier stages
//!   left it, before any of its steps commits.
//! - When a step fails after that (a dry run of a later stage, a reference whose value is not in
//!   the reply, or a commit), the steps already applied are undone with the `undo` tool, newest
//!   first ([`Status::Failed`], [`Report::rollback`]). Before each `undo`, `get_project_state`
//!   must show the label of that step's own undo step on top, and a project version that only
//!   the plan and the rollback have moved on since that step committed. Otherwise the rollback
//!   stops, so that an edit someone else made meanwhile is never undone, and the report lists
//!   what is still applied.
//! - The rollback undoes one undo step per applied step, so every step of a plan must be an
//!   undoable edit, and a plan must not set the same value twice in a row: the undo history
//!   merges two edits of the same value made within 1.5 s (the tempo, or the pan of one track)
//!   into one undo step. This is why a request that sets the tempo twice is a question.
//! - When the tool layer asks for its own confirmation token for a confirmed destructive step,
//!   the step is repeated with that token: the caller's approval is the user's agreement.

pub mod grammar;
mod refs;

use std::ops::Range;

use serde::Serialize;
use serde_json::{Value, json};
use sonara_core::error::{DomainError, ErrorCode};

pub use self::refs::StepRef;
use crate::contract;
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
    /// The arguments of the call, without `dry_run`. An argument that a reference fills holds
    /// the reference's placeholder until the step runs.
    pub args: Value,
    /// What the step does, in a sentence.
    pub description: String,
    /// Whether the step deletes or discards something.
    pub destructive: bool,
    /// The values the step takes from the replies of earlier steps; empty for most steps.
    pub refs: Vec<StepRef>,
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
    /// Validation refused a step before the first commit; nothing was changed.
    Rejected,
    /// A step failed after the plan began to commit. The steps already applied were undone as
    /// far as [`Report::rollback`] says.
    Failed,
}

/// The outcome of one step that ran or was refused.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepResult {
    /// The step as it ran: its references replaced by the values they named.
    pub step: Step,
    /// Whether the tool call succeeded.
    pub ok: bool,
    /// The error code when it did not: the tool's, `INVALID_ARGUMENT` for a reference that
    /// cannot be used, or `NOT_FOUND` for a tool that does not exist.
    pub error: Option<String>,
    /// The error message when it did not.
    pub message: Option<String>,
    /// The tool's reply: the commit's, or the dry run's for a step that validation refused.
    /// `None` when the step was refused without a call.
    pub reply: Option<Value>,
    /// Whether the rollback after a later failure undid the step.
    pub rolled_back: bool,
}

impl StepResult {
    /// The result of `step` from the tool's reply.
    fn from_reply(step: Step, reply: Value) -> Self {
        Self {
            ok: is_ok(&reply),
            error: error_code(&reply),
            message: reply["error"]["message"].as_str().map(str::to_owned),
            step,
            reply: Some(reply),
            rolled_back: false,
        }
    }

    /// `step`, refused without a call.
    fn refused(step: Step, code: ErrorCode, message: String) -> Self {
        Self {
            step,
            ok: false,
            error: Some(code.as_str().to_owned()),
            message: Some(message),
            reply: None,
            rolled_back: false,
        }
    }

    /// Whether the step changed the project and so made an undo step: it succeeded, and its
    /// reply does not say that nothing was committed (a value set to what it already was).
    fn applied(&self) -> bool {
        self.ok
            && self
                .reply
                .as_ref()
                .is_some_and(|reply| reply["committed"] != Value::Bool(false))
    }

    /// The error code and message, for a summary.
    fn reason(&self) -> String {
        match (self.error.as_deref(), self.message.as_deref()) {
            (Some(code), Some(message)) => format!("{code}: {message}"),
            (Some(code), None) => code.to_owned(),
            (None, _) => "unknown error".to_owned(),
        }
    }
}

/// What the rollback after a failed step did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rollback {
    /// Whether every applied step was undone, so that the project is as it was before the run.
    pub complete: bool,
    /// Why the rollback stopped before it had undone every applied step; `None` when it is
    /// complete.
    pub stopped: Option<String>,
}

/// The report of a run: the plan, what was executed and a summary for the user.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// How the run ended.
    pub status: Status,
    /// The plan the request resolved to.
    pub plan: Plan,
    /// The steps that were committed, then the step that failed or was refused, in plan order.
    pub executed: Vec<StepResult>,
    /// What the rollback did when the run [`Status::Failed`]; `None` otherwise.
    pub rollback: Option<Rollback>,
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
        // Two tempo changes would merge into one undo step, which the rollback cannot take apart.
        let tempos: Vec<&str> = plan
            .steps
            .iter()
            .filter(|step| step.tool == "set_tempo")
            .map(|step| step.description.as_str())
            .collect();
        if tempos.len() > 1 {
            let question = format!(
                "The request sets the tempo more than once ({}). Which tempo do you want?",
                tempos.join(", ")
            );
            plan.questions.push(question);
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

    /// Applies a plan from [`Orchestrator::plan`] (or one built by hand) under the rules in the
    /// module documentation.
    #[must_use]
    pub fn execute(&self, plan: Plan, approval: Approval) -> Report {
        if !plan.questions.is_empty() {
            let summary = format!(
                "I need to know more before changing anything: {}",
                plan.questions.join(" ")
            );
            return finish(Status::NeedsClarification, plan, Vec::new(), None, summary);
        }
        if plan.is_destructive() && approval != Approval::Confirmed {
            let summary = format!(
                "This plan removes something, so nothing was changed. Please confirm: {}",
                describe(&plan.steps)
            );
            return finish(Status::NeedsConfirmation, plan, Vec::new(), None, summary);
        }
        if let Some(refused) = self.validate(&plan.steps) {
            let summary = format!(
                "Nothing was changed: '{}' was refused ({}).",
                refused.step.description,
                refused.reason()
            );
            return finish(Status::Rejected, plan, vec![refused], None, summary);
        }
        let mut executed = Vec::with_capacity(plan.steps.len());
        for (number, stage) in refs::stages(&plan.steps).into_iter().enumerate() {
            if let Some(failed) = self.run_stage(&plan.steps, stage, number > 0, &mut executed) {
                let rollback = self.roll_back(&mut executed);
                let summary = failure_summary(&failed, &executed, &rollback);
                executed.push(failed);
                return finish(Status::Failed, plan, executed, Some(rollback), summary);
            }
        }
        let summary = format!(
            "Done: {}.",
            descriptions(executed.iter().filter(|result| result.ok))
        );
        finish(Status::Done, plan, executed, None, summary)
    }

    /// The validation before the first commit. Returns the first step it refuses.
    fn validate(&self, steps: &[Step]) -> Option<StepResult> {
        for (index, step) in steps.iter().enumerate() {
            if let Err(error) = refs::check(step, index) {
                let message = format!("it cannot take a value from an earlier step: {error}");
                return Some(StepResult::refused(
                    step.clone(),
                    ErrorCode::InvalidArgument,
                    message,
                ));
            }
        }
        steps.iter().find_map(|step| self.validate_step(step))
    }

    /// Validates one step before the first commit: a step without references with its tool's
    /// `dry_run`, a step with references, which cannot be dry-run yet, against its tool's input
    /// model with the placeholders in place. Returns the step when it is refused.
    fn validate_step(&self, step: &Step) -> Option<StepResult> {
        if step.refs.is_empty() {
            let reply = self.dry_run(step);
            return (!is_ok(&reply)).then(|| StepResult::from_reply(step.clone(), reply));
        }
        let Some(spec) = contract::find(&step.tool) else {
            let message = format!("unknown tool '{}'", step.tool);
            return Some(StepResult::refused(
                step.clone(),
                ErrorCode::NotFound,
                message,
            ));
        };
        contract::validate(spec, &step.args)
            .err()
            .map(|error| StepResult::refused(step.clone(), error.code, validation_message(&error)))
    }

    /// Runs the steps `range` of `steps`: fills their references from the replies in
    /// `executed`, dry-runs all of them first when `validate` is set, then commits them in order
    /// and appends each to `executed`. Returns the step that failed, which is not appended.
    fn run_stage(
        &self,
        steps: &[Step],
        range: Range<usize>,
        validate: bool,
        executed: &mut Vec<StepResult>,
    ) -> Option<StepResult> {
        let stage = steps.get(range).unwrap_or_default();
        let mut ready = Vec::with_capacity(stage.len());
        for step in stage {
            let reply = |index: usize| executed.get(index).and_then(|done| done.reply.as_ref());
            match refs::resolve(step, reply) {
                Ok(args) => ready.push(Step {
                    args,
                    ..step.clone()
                }),
                Err(error) => {
                    let message = format!("it cannot take a value from an earlier step: {error}");
                    return Some(StepResult::refused(
                        step.clone(),
                        ErrorCode::InvalidArgument,
                        message,
                    ));
                }
            }
        }
        if validate {
            for step in &ready {
                let reply = self.dry_run(step);
                if !is_ok(&reply) {
                    return Some(StepResult::from_reply(step.clone(), reply));
                }
            }
        }
        for step in ready {
            let reply = self.commit(&step);
            let result = StepResult::from_reply(step, reply);
            if !result.ok {
                return Some(result);
            }
            executed.push(result);
        }
        None
    }

    /// The step's call with `dry_run` set.
    fn dry_run(&self, step: &Step) -> Value {
        self.runner
            .call(&step.tool, &with_flag(&step.args, "dry_run", json!(true)))
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

    /// Undoes the applied steps of `executed`, newest first, and marks each one it undid. Stops
    /// at the first step whose undo step it cannot show to be that step's own.
    fn roll_back(&self, executed: &mut [StepResult]) -> Rollback {
        let applied: Vec<usize> = executed
            .iter()
            .enumerate()
            .filter(|(_, result)| result.applied())
            .map(|(index, _)| index)
            .collect();
        for (later, &index) in applied.iter().rev().enumerate() {
            let Some(result) = executed.get_mut(index) else {
                continue;
            };
            if let Err(reason) = self.undo_step(result, later) {
                return Rollback {
                    complete: false,
                    stopped: Some(reason),
                };
            }
            result.rolled_back = true;
        }
        Rollback {
            complete: true,
            stopped: None,
        }
    }

    /// Undoes the undo step of `result`, after the `later` applied steps that followed it were
    /// undone. Before the `undo`, the project must show this step's label on top of the undo
    /// history, and the version this step's commit left, moved on by one for each later applied
    /// step and one for each undo since.
    ///
    /// # Errors
    ///
    /// Why the newest undo step cannot be shown to be this step's, or why it could not be undone.
    fn undo_step(&self, result: &StepResult, later: usize) -> Result<(), String> {
        let step = &result.step.description;
        let reply = result.reply.as_ref();
        let (Some(label), Some(version)) = (
            reply.and_then(|reply| reply["operation"].as_str()),
            reply.and_then(|reply| reply["project_version"].as_u64()),
        ) else {
            return Err(format!(
                "the reply of '{step}' names no undo step and project version"
            ));
        };
        let moves = u64::try_from(later).unwrap_or(u64::MAX).saturating_mul(2);
        let expected = version.saturating_add(moves);
        let state = self.runner.call(
            "get_project_state",
            &json!({"detail": "summary", "include_notes": false, "track_ids": []}),
        );
        if !is_ok(&state) {
            return Err(format!(
                "the project could not be read before undoing '{step}' ({})",
                error_code(&state).as_deref().unwrap_or("unknown error")
            ));
        }
        if state["project_version"].as_u64() != Some(expected) {
            return Err(format!(
                "another edit changed the project after '{step}', and the rollback keeps it"
            ));
        }
        if state["undo"].as_str() != Some(label) {
            let newest = state["undo"]
                .as_str()
                .map_or_else(|| "none".to_owned(), |label| format!("'{label}'"));
            return Err(format!(
                "the newest undo step is {newest}, not the one '{step}' made ('{label}')"
            ));
        }
        let undo = self.runner.call("undo", &json!({}));
        if is_ok(&undo) {
            Ok(())
        } else {
            Err(format!(
                "undoing '{step}' failed ({})",
                error_code(&undo).as_deref().unwrap_or("unknown error")
            ))
        }
    }

    /// The list under `key` of a read tool's reply; empty when the call fails (no project).
    fn read(&self, tool: &str, key: &str) -> Vec<Value> {
        let reply = self.runner.call(tool, &json!({}));
        reply[key].as_array().cloned().unwrap_or_default()
    }
}

fn finish(
    status: Status,
    plan: Plan,
    executed: Vec<StepResult>,
    rollback: Option<Rollback>,
    summary: String,
) -> Report {
    Report {
        status,
        plan,
        executed,
        rollback,
        summary,
    }
}

/// The summary of a failed run: what failed, what was undone and what is still applied.
fn failure_summary(failed: &StepResult, executed: &[StepResult], rollback: &Rollback) -> String {
    let head = format!("'{}' failed ({})", failed.step.description, failed.reason());
    let undone = descriptions(executed.iter().filter(|result| result.rolled_back));
    match &rollback.stopped {
        None if executed.iter().any(|result| result.rolled_back) => {
            format!("{head}, so the steps already applied were undone: {undone}.")
        }
        None => format!("{head}. Nothing was changed."),
        Some(why) => {
            let kept = descriptions(
                executed
                    .iter()
                    .filter(|result| result.applied() && !result.rolled_back),
            );
            format!(
                "{head}. Undone: {undone}. Still applied: {kept}. The rollback stopped because {why}."
            )
        }
    }
}

/// A validation error's message with the fields it names.
fn validation_message(error: &DomainError) -> String {
    let fields: Vec<String> = error
        .details
        .get("errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|field| {
            Some(format!(
                "{}: {}",
                field["field"].as_str()?,
                field["message"].as_str()?
            ))
        })
        .collect();
    if fields.is_empty() {
        error.message.clone()
    } else {
        format!("{} ({})", error.message, fields.join("; "))
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

/// The descriptions of `results`, joined; "nothing" when there are none.
fn descriptions<'a>(results: impl Iterator<Item = &'a StepResult>) -> String {
    let done: Vec<&str> = results
        .map(|result| result.step.description.as_str())
        .collect();
    if done.is_empty() {
        "nothing".to_owned()
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
            refs: Vec::new(),
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
        refs: Vec::new(),
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
        refs: Vec::new(),
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
