//! The context every tool handler runs in (`AgentContext` of `sonara.agent.tools.registry`):
//! the application, the client's file sandbox and confirmations, the shared audit log and the
//! client's name.
//!
//! Handlers reach the project only through this context: [`AgentContext::project`] reads a
//! snapshot, [`AgentContext::commit`] runs a command through the one session (ARC-11), and
//! [`AgentContext::path`] admits a file path through the sandbox. The argument conversions
//! (positions, lengths, pitches, notes) are the functions of [`crate::conversion`].

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Map, Value};
use sonara_app::context::AppContext;
use sonara_app::jobs::{Job, JobStatus};
use sonara_app::paths::{NoHomeDirectory, default_projects_dir};
use sonara_core::commands::Command;
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::model::project::Project;

use crate::audit::AuditLog;
use crate::contract::args;
use crate::permissions::{Access, Confirmations, ENV_ALLOWED, PathPolicy};

/// What the default file sandbox reads from the environment besides the settings: the value of
/// [`ENV_ALLOWED`] and the default projects folder.
///
/// The prototype reads both from the process environment; golden runs patch the environment,
/// which Rust tests cannot do safely, so the values are passed in (Rust addition).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyEnvironment {
    /// The value of `SONARA_AGENT_ALLOWED_PATHS`, split like `PATH`.
    pub allowed: Option<OsString>,
    /// The default projects folder (`sonara_app::paths::default_projects_dir`).
    pub projects_dir: PathBuf,
}

impl PolicyEnvironment {
    /// The process environment and the platform's default projects folder.
    ///
    /// # Errors
    ///
    /// [`NoHomeDirectory`] when neither `SONARA_PROJECTS_DIR` nor the home directory is known
    /// (the prototype fails there too).
    pub fn system() -> Result<Self, NoHomeDirectory> {
        Ok(Self {
            allowed: std::env::var_os(ENV_ALLOWED),
            projects_dir: default_projects_dir()?,
        })
    }
}

/// The prototype's `default_policy(app, base)`: the settings' allowed paths, then those of
/// [`ENV_ALLOWED`], then the default projects folder, then `base` (see
/// [`PathPolicy::from_environment`]).
///
/// # Errors
///
/// The error of reading the working directory when `base` is `None`.
pub fn default_policy(
    app: &AppContext,
    base: Option<&Path>,
    environment: &PolicyEnvironment,
) -> io::Result<PathPolicy> {
    // Copied out so that the settings lock is not held while paths are resolved.
    let configured = app.settings().agent_allowed_paths.clone();
    PathPolicy::from_environment(
        &configured,
        environment.allowed.as_deref(),
        base,
        std::slice::from_ref(&environment.projects_dir),
    )
}

/// The arguments of a destructive tool: they carry `dry_run` and `confirmation_token`
/// (the prototype's `Confirmable` model).
pub trait Confirmable: Serialize {
    /// Whether the call only describes the change.
    fn dry_run(&self) -> bool;
    /// The token of an earlier `CONFIRMATION_REQUIRED` response.
    fn confirmation_token(&self) -> Option<&str>;
}

macro_rules! confirmable {
    ($($args:ident),* $(,)?) => {$(
        impl Confirmable for args::$args {
            fn dry_run(&self) -> bool {
                self.dry_run
            }

            fn confirmation_token(&self) -> Option<&str> {
                self.confirmation_token.as_deref()
            }
        }
    )*};
}

confirmable!(
    CreateProject,
    OpenProject,
    SaveProject,
    DeleteTrack,
    ExportAudio,
    ExportStems,
    ExportMidi,
);

/// The prototype's `model_dump(args)` (`mode="json", exclude_none=True`): the validated
/// arguments as JSON, without the fields that are `null`, in nested objects too. Confirmation
/// tokens are bound to this form of a call.
///
/// # Errors
///
/// [`ErrorCode::Internal`] if the arguments do not serialize to a JSON object, a programming
/// error (every typed argument struct does).
pub fn model_dump(args: &impl Serialize) -> Result<Map<String, Value>, DomainError> {
    match serde_json::to_value(args) {
        Ok(Value::Object(map)) => Ok(without_nulls(map)),
        Ok(_) => Err(DomainError::new(
            ErrorCode::Internal,
            "tool arguments are not a JSON object",
        )),
        Err(error) => Err(DomainError::new(
            ErrorCode::Internal,
            format!("cannot serialize tool arguments: {error}"),
        )),
    }
}

fn without_nulls(map: Map<String, Value>) -> Map<String, Value> {
    map.into_iter()
        .filter(|(_, value)| !value.is_null())
        .map(|(key, value)| match value {
            Value::Object(inner) => (key, Value::Object(without_nulls(inner))),
            Value::Array(items) => (
                key,
                Value::Array(
                    items
                        .into_iter()
                        .map(|item| match item {
                            Value::Object(inner) => Value::Object(without_nulls(inner)),
                            other => other,
                        })
                        .collect(),
                ),
            ),
            other => (key, other),
        })
        .collect()
}

/// What a tool handler works with. One context serves one agent client; the application, its
/// session and the audit log are shared with every other client.
pub struct AgentContext {
    app: Arc<AppContext>,
    policy: Mutex<PathPolicy>,
    confirmations: Confirmations,
    audit: Arc<AuditLog>,
    source: String,
}

impl fmt::Debug for AgentContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentContext")
            .field("source", &self.source)
            .field("policy", &*self.policy_guard())
            .field("confirmations", &self.confirmations)
            .field("audit", &self.audit)
            .finish_non_exhaustive()
    }
}

impl AgentContext {
    /// A context for the client `source` (`agent`, or an IPC client's name).
    #[must_use]
    pub fn new(
        app: Arc<AppContext>,
        policy: PathPolicy,
        confirmations: Confirmations,
        audit: Arc<AuditLog>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            app,
            policy: Mutex::new(policy),
            confirmations,
            audit,
            source: source.into(),
        }
    }

    /// The application.
    #[must_use]
    pub fn app(&self) -> &AppContext {
        &self.app
    }

    /// The application, shared.
    #[must_use]
    pub fn shared_app(&self) -> &Arc<AppContext> {
        &self.app
    }

    /// The client's name, recorded in the audit log and the undo history.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The client's confirmations.
    #[must_use]
    pub fn confirmations(&self) -> &Confirmations {
        &self.confirmations
    }

    /// The audit log shared by every client.
    #[must_use]
    pub fn audit(&self) -> &AuditLog {
        &self.audit
    }

    /// The audit log, shared.
    #[must_use]
    pub fn shared_audit(&self) -> &Arc<AuditLog> {
        &self.audit
    }

    /// A copy of the client's file sandbox as it is now.
    #[must_use]
    pub fn policy(&self) -> PathPolicy {
        self.policy_guard().clone()
    }

    /// Adds `root` to the client's allowed locations ([`PathPolicy::allow`]).
    pub fn allow(&self, root: impl AsRef<Path>) {
        self.policy_guard().allow(root);
    }

    fn policy_guard(&self) -> MutexGuard<'_, PathPolicy> {
        // Each change of the policy is a single push onto its root list, so a holder that
        // panicked leaves it consistent.
        self.policy.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A snapshot of the open project (`ctx.project`).
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`]: `"no project is open; create or open one first"`.
    pub fn project(&self) -> Result<Project, DomainError> {
        self.app.session().lock().require_project().cloned()
    }

    /// Runs `command` through the session for this client and returns the agent-facing result
    /// (`CommandResult.to_json()`): with `dry_run`, the project stays unchanged and the result
    /// reports what would change.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`], or the command's error (the project is left unchanged).
    pub fn commit(&self, command: &dyn Command, dry_run: bool) -> Result<Value, DomainError> {
        let result = self
            .app
            .session()
            .lock()
            .execute(command, dry_run, &self.source)?;
        Ok(result.to_json())
    }

    /// Admits `value` through the client's file sandbox for `access`. The open project's
    /// workspace and the folder of its `.sonara` path are allowed first, as in the prototype,
    /// so a project's own files are always reachable.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::PermissionDenied`] as [`PathPolicy::resolve`] reports it.
    pub fn path(&self, value: &str, access: Access) -> Result<PathBuf, DomainError> {
        // The location is copied out: the session lock is never held while paths are resolved.
        let location = self.app.session().lock().location().cloned();
        let mut policy = self.policy_guard();
        if let Some(location) = location {
            policy.allow(&location.workspace);
            if let Some(path) = &location.path {
                policy.allow(parent(path));
            }
        }
        policy.resolve(value, access)
    }

    /// The prototype's `wait_job`: waits up to `timeout_seconds` for `job` when `wait` is set,
    /// then returns its state (`Job.to_json()`).
    ///
    /// # Errors
    ///
    /// The job's own error when it failed, with the detail `operation_id`.
    pub fn wait_job(
        &self,
        job: &Job,
        wait: bool,
        timeout_seconds: f64,
    ) -> Result<Value, DomainError> {
        if wait {
            // `Event.wait(timeout)` without a usable bound waits until the job finishes.
            let timeout = Duration::try_from_secs_f64(timeout_seconds).ok();
            // Whether it finished in time shows in the status reported below.
            let _finished = job.wait(timeout);
        }
        let snapshot = job.snapshot();
        let data = snapshot.to_json();
        if snapshot.status == JobStatus::Failed
            && let Some(error) = snapshot.error
        {
            return Err(
                DomainError::new(error.code, error.message).with_detail("operation_id", job.id())
            );
        }
        Ok(data)
    }

    /// The prototype's `_confirm`: a call that `needed` confirmation and is not a dry run must
    /// carry a valid token for exactly these arguments ([`Confirmations::require`]). Dry runs
    /// never need one; they report whether the real call would. Returns `needed`.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::ConfirmationRequired`] with a new token.
    pub fn confirm<A: Confirmable>(
        &self,
        action: &str,
        args: &A,
        needed: bool,
        description: &str,
    ) -> Result<bool, DomainError> {
        if needed && !args.dry_run() {
            self.confirmations.require(
                action,
                &model_dump(args)?,
                args.confirmation_token(),
                description,
            )?;
        }
        Ok(needed)
    }

    /// The prototype's `_require_clean_or_confirm`: replacing an open project that has unsaved
    /// changes needs confirmation.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::ConfirmationRequired`] with a new token:
    /// `"the open project '<name>' has unsaved changes that will be discarded. …"`.
    pub fn require_clean_or_confirm<A: Confirmable>(
        &self,
        action: &str,
        args: &A,
    ) -> Result<(), DomainError> {
        let dirty_project = {
            let session = self.app.session().lock();
            session
                .project()
                .filter(|_| session.is_dirty())
                .map(|project| project.name.clone())
        };
        if let Some(name) = dirty_project {
            self.confirmations.require(
                action,
                &model_dump(args)?,
                args.confirmation_token(),
                &format!(
                    "the open project {} has unsaved changes that will be discarded",
                    sonara_core::pyfmt::str_repr(&name)
                ),
            )?;
        }
        Ok(())
    }
}

/// Python's `Path.parent`: the path without its last component, `.` for a bare name.
fn parent(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        Some(_) => Path::new("."),
        None => path,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{model_dump, without_nulls};
    use crate::contract::args::{CreateProject, Template};

    #[test]
    fn model_dump_drops_null_fields_like_exclude_none() {
        let args = CreateProject {
            dry_run: false,
            confirmation_token: None,
            name: "x".to_owned(),
            bpm: 120.0,
            time_signature: "4/4".to_owned(),
            sample_rate: 48_000,
            template: Template::Empty,
            length: None,
            path: Some("a.sonara".to_owned()),
        };
        let dumped = model_dump(&args).expect("an object");
        assert_eq!(
            serde_json::Value::Object(dumped),
            json!({"dry_run": false, "name": "x", "bpm": 120.0, "time_signature": "4/4",
                   "sample_rate": 48000, "template": "empty", "path": "a.sonara"})
        );
        let nested = json!({"a": null, "b": {"c": null, "d": 1}, "e": [{"f": null}, 2]});
        let serde_json::Value::Object(map) = nested else {
            unreachable!("an object literal")
        };
        assert_eq!(
            serde_json::Value::Object(without_nulls(map)),
            json!({"b": {"d": 1}, "e": [{}, 2]})
        );
    }
}
