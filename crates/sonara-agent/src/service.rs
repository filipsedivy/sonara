//! The in-process agent facade (`AgentService` of `sonara.agent.service`): the tool registry
//! bound to an application. Used directly (Rust API, headless MCP) and by the IPC server, which
//! serves each client through a [`AgentService::for_client`] view.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use serde_json::{Value, json};
use sonara_app::context::AppContext;
use sonara_app::paths::{NoHomeDirectory, logs_dir};

use crate::audit::AuditLog;
use crate::context::{AgentContext, PolicyEnvironment, default_policy};
use crate::contract::API_VERSION;
use crate::permissions::{Confirmations, PathPolicy, TokenSource, random_tokens};
use crate::registry::ToolRegistry;

/// Where tool calls are audited (the prototype's `audit_path`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AuditTarget {
    /// `<logs>/agent-audit.jsonl` in the application's logs directory (`audit_path=True`).
    #[default]
    Default,
    /// Nothing is recorded (`audit_path=False` or `None`).
    Disabled,
    /// This file.
    File(PathBuf),
}

/// A source of confirmation tokens shared by every view of a service. The prototype draws all
/// tokens from `secrets.token_urlsafe`, one source for the whole process, which golden runs
/// replace by one counter; sharing the source keeps that order across clients.
pub type TokenFactory = Arc<dyn Fn() -> String + Send + Sync>;

/// `source` behind a lock, as a [`TokenFactory`] (for example [`crate::permissions::sequential_tokens`]
/// in golden runs).
#[must_use]
pub fn shared_tokens(source: TokenSource) -> TokenFactory {
    let source = Mutex::new(source);
    // A token source keeps no state that a panicking holder could break.
    Arc::new(move || (source.lock().unwrap_or_else(PoisonError::into_inner))())
}

/// How [`AgentService::new`] builds the service (the prototype's keyword arguments, plus the
/// sources it replaces globally in golden runs).
#[derive(Clone)]
pub struct AgentOptions {
    /// The working directory of relative paths; it is also allowed (default: the process's
    /// working directory, which is not).
    pub base_dir: Option<PathBuf>,
    /// The client's name in the audit log and the undo history.
    pub source: String,
    /// Where calls are audited.
    pub audit: AuditTarget,
    /// Extra allowed locations.
    pub allowed_paths: Vec<PathBuf>,
    /// The handlers (default: [`crate::handlers::builtin`]).
    pub registry: Option<Arc<ToolRegistry>>,
    /// The confirmation tokens (default: random). Rust addition: golden runs use sequential
    /// tokens.
    pub tokens: Option<TokenFactory>,
    /// What the default file sandbox reads from the environment (default:
    /// [`PolicyEnvironment::system`]). Rust addition: tests cannot patch the environment.
    pub environment: Option<PolicyEnvironment>,
}

impl Default for AgentOptions {
    fn default() -> Self {
        Self {
            base_dir: None,
            source: "agent".to_owned(),
            audit: AuditTarget::Default,
            allowed_paths: Vec::new(),
            registry: None,
            tokens: None,
            environment: None,
        }
    }
}

impl fmt::Debug for AgentOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentOptions")
            .field("base_dir", &self.base_dir)
            .field("source", &self.source)
            .field("audit", &self.audit)
            .field("allowed_paths", &self.allowed_paths)
            .field("registry", &self.registry)
            .field("tokens", &self.tokens.is_some())
            .field("environment", &self.environment)
            .finish()
    }
}

/// [`AgentService::new`] could not build the service.
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    /// The default projects folder or the logs directory is unknown.
    #[error(transparent)]
    NoHome(#[from] NoHomeDirectory),
    /// The working directory, the base of relative paths, cannot be read.
    #[error("cannot read the working directory: {0}")]
    WorkingDirectory(#[source] io::Error),
}

/// Runs tools against an application.
pub struct AgentService {
    registry: Arc<ToolRegistry>,
    tokens: TokenFactory,
    ctx: AgentContext,
}

impl fmt::Debug for AgentService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentService")
            .field("registry", &self.registry)
            .field("ctx", &self.ctx)
            .finish_non_exhaustive()
    }
}

impl AgentService {
    /// The service for `app`: the default file sandbox (settings, environment, the default
    /// projects folder and `base_dir`) plus `allowed_paths`, confirmations as the settings ask,
    /// and the audit log.
    ///
    /// # Errors
    ///
    /// - [`ServiceError::NoHome`] when the default projects folder or, for
    ///   [`AuditTarget::Default`], the logs directory is unknown;
    /// - [`ServiceError::WorkingDirectory`] without `base_dir` when the working directory
    ///   cannot be read.
    pub fn new(app: Arc<AppContext>, options: AgentOptions) -> Result<Self, ServiceError> {
        let audit = match options.audit {
            AuditTarget::Default => AuditLog::new(Some(AuditLog::default_path(&logs_dir()?))),
            AuditTarget::Disabled => AuditLog::new(None),
            AuditTarget::File(path) => AuditLog::new(Some(path)),
        };
        let environment = match options.environment {
            Some(environment) => environment,
            None => PolicyEnvironment::system()?,
        };
        let mut policy = default_policy(&app, options.base_dir.as_deref(), &environment)
            .map_err(ServiceError::WorkingDirectory)?;
        for path in &options.allowed_paths {
            policy.allow(path);
        }
        let tokens = options
            .tokens
            .unwrap_or_else(|| shared_tokens(random_tokens()));
        let enabled = app.settings().agent_require_confirmation;
        let confirmations = confirmations(enabled, &tokens);
        let registry = options
            .registry
            .unwrap_or_else(|| Arc::new(crate::handlers::builtin()));
        let ctx = AgentContext::new(app, policy, confirmations, Arc::new(audit), options.source);
        Ok(Self {
            registry,
            tokens,
            ctx,
        })
    }

    /// A view of this service for one IPC client: the same application, session, registry,
    /// audit log and token source, but the client's working directory for relative paths, and
    /// the client's allowed folders on top of this service's (clients are authenticated with
    /// the session token, so they are the same user). The view has its own confirmations.
    #[must_use]
    pub fn for_client(
        &self,
        base_dir: Option<&Path>,
        allowed_paths: &[PathBuf],
        source: &str,
    ) -> Self {
        let current = self.ctx.policy();
        let mut policy = PathPolicy::new(current.roots_paths(), base_dir.unwrap_or(current.base()));
        for path in allowed_paths {
            policy.allow(path);
        }
        if let Some(base) = base_dir {
            policy.allow(base);
        }
        let enabled = self.ctx.confirmations().is_enabled();
        let ctx = AgentContext::new(
            Arc::clone(self.ctx.shared_app()),
            policy,
            confirmations(enabled, &self.tokens),
            Arc::clone(self.ctx.shared_audit()),
            source,
        );
        Self {
            registry: Arc::clone(&self.registry),
            tokens: Arc::clone(&self.tokens),
            ctx,
        }
    }

    /// Calls the tool `name` ([`ToolRegistry::call`]): always an envelope.
    #[must_use]
    pub fn call(&self, name: &str, arguments: Option<&Value>) -> Value {
        self.registry.call(&self.ctx, name, arguments)
    }

    /// The tools with handlers as the tool contract lists them (`ToolSpec.to_json`), in
    /// contract order.
    #[must_use]
    pub fn tools(&self) -> Vec<Value> {
        self.registry
            .specs()
            .iter()
            .map(|spec| spec.to_json())
            .collect()
    }

    /// `{api_version, project, tools}`: the open project's name (or `null`) and the number of
    /// tools.
    #[must_use]
    pub fn info(&self) -> Value {
        let project = self
            .ctx
            .app()
            .session()
            .lock()
            .project()
            .map(|project| project.name.clone());
        json!({
            "api_version": API_VERSION,
            "project": project,
            "tools": self.registry.specs().len(),
        })
    }

    /// The registry.
    #[must_use]
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// The context the tools of this view run in.
    #[must_use]
    pub fn context(&self) -> &AgentContext {
        &self.ctx
    }

    /// The application.
    #[must_use]
    pub fn app(&self) -> &AppContext {
        self.ctx.app()
    }
}

/// Confirmations drawing their tokens from `tokens`, on the system's monotonic clock.
fn confirmations(enabled: bool, tokens: &TokenFactory) -> Confirmations {
    let tokens = Arc::clone(tokens);
    Confirmations::with_sources(enabled, Box::new(move || tokens()), Box::new(Instant::now))
}
