//! The tool registry and dispatcher (`ToolRegistry` of `sonara.agent.tools.registry`, ADR 0005):
//! one contract for the Rust API, the CLI, IPC and MCP.
//!
//! Handlers are attached one tool at a time with [`ToolRegistry::register`]. A tool of the
//! contract without a handler is unknown to the registry, exactly as an unregistered name was
//! to the prototype's.
//!
//! [`ToolRegistry::call`] validates the arguments, runs the handler and always returns a JSON
//! envelope, built as the prototype builds it:
//!
//! - success: the handler's result with `ok: true` and `tool` set (`{**result, "ok": True,
//!   "tool": name}`; a handler cannot fake either key);
//! - failure: `{"ok": false, "tool": name, "error": {code, message, details?}}`.
//!
//! Every call is recorded in the audit log, failed ones with their error code.

use std::any::Any;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

use serde_json::{Map, Value, json};
use sonara_app::error::AppError;
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::pyfmt::str_repr;

use crate::audit::{AuditError, AuditRecord};
use crate::context::AgentContext;
use crate::contract::{TOOLS, ToolArgs, ToolSpec, parse};

/// A handler's result: a JSON object, or the error the envelope reports.
pub type ToolResult = Result<Value, ToolError>;

/// A tool handler: the context of the calling client and the validated arguments.
pub type Handler<A> = fn(&AgentContext, A) -> ToolResult;

/// Why a tool call failed, and how the envelope reports it.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    /// An expected error (the prototype's `SonaraError`), reported with its code, message and
    /// details.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// A failure of an application service that is not a domain error (file system, worker
    /// threads), reported as `INTERNAL` with its text.
    #[error(transparent)]
    App(AppError),
    /// The audit log could not be read, reported as `INTERNAL`.
    #[error(transparent)]
    Audit(#[from] AuditError),
    /// Any other unexpected failure, reported as `INTERNAL` with this message.
    #[error("{0}")]
    Internal(String),
}

impl From<AppError> for ToolError {
    /// Domain errors of the services keep their code; everything else is `INTERNAL`.
    fn from(error: AppError) -> Self {
        match error {
            AppError::Domain(error) => Self::Domain(error),
            other => Self::App(other),
        }
    }
}

impl ToolError {
    /// The error code the envelope and the audit log report.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Domain(error) => error.code,
            Self::App(error) => error.code(),
            Self::Audit(_) | Self::Internal(_) => ErrorCode::Internal,
        }
    }

    /// The `error` of the envelope: the prototype's `SonaraError.to_dict()` (`code`, `message`,
    /// and `details` when there are any), or `INTERNAL` with the failure's text.
    ///
    /// The prototype words an unexpected failure as `"<exception type>: <message>"`; Rust
    /// reports the error's own text (a deviation in wording only).
    #[must_use]
    pub fn to_json(&self) -> Value {
        match self {
            Self::Domain(error) => error_json(error),
            Self::App(error) => error.to_json(),
            Self::Audit(error) => internal_json(&error.to_string()),
            Self::Internal(message) => internal_json(message),
        }
    }
}

/// The prototype's `SonaraError.to_dict()` of a domain error.
#[must_use]
pub fn error_json(error: &DomainError) -> Value {
    let mut out = json!({"code": error.code.as_str(), "message": error.message});
    if !error.details.is_empty() {
        out["details"] = Value::Object(error.details.clone());
    }
    out
}

fn internal_json(message: &str) -> Value {
    json!({"code": ErrorCode::Internal.as_str(), "message": message})
}

/// A handler can be attached only to a tool of the contract, once, with the argument struct of
/// that tool's input model.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    /// The name is not a tool of the contract.
    #[error("{0} is not a tool of the contract")]
    UnknownTool(String),
    /// The tool already has a handler.
    #[error("tool {0} registered twice")]
    Duplicate(&'static str),
    /// The handler takes the arguments of another input model.
    #[error("{tool} takes {expected} arguments, not {actual}")]
    WrongArguments {
        /// The tool.
        tool: &'static str,
        /// Its input model.
        expected: &'static str,
        /// The model of the handler's argument struct.
        actual: &'static str,
    },
}

/// A registered handler behind its argument parsing.
type Erased = Box<dyn Fn(&AgentContext, &'static ToolSpec, &Value) -> ToolResult + Send + Sync>;

/// The tools that have handlers, in contract order.
pub struct ToolRegistry {
    /// One slot per tool of [`TOOLS`], in the same order.
    handlers: Vec<Option<Erased>>,
}

impl fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.specs().iter().map(|spec| spec.name).collect();
        f.debug_struct("ToolRegistry")
            .field("tools", &names)
            .finish()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// A registry without handlers: every tool is unknown.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: TOOLS.iter().map(|_| None).collect(),
        }
    }

    /// Attaches `handler` to the tool `name`. Its arguments are validated against the tool's
    /// input model and handed over as `A`.
    ///
    /// # Errors
    ///
    /// [`RegistryError`] when `name` is not a tool of the contract, already has a handler, or
    /// takes arguments other than `A`.
    pub fn register<A: ToolArgs + 'static>(
        &mut self,
        name: &str,
        handler: Handler<A>,
    ) -> Result<(), RegistryError> {
        let Some(index) = TOOLS.iter().position(|spec| spec.name == name) else {
            return Err(RegistryError::UnknownTool(name.to_owned()));
        };
        // `index` comes from `TOOLS`, and there is one slot per tool.
        let (Some(spec), Some(slot)) = (TOOLS.get(index), self.handlers.get_mut(index)) else {
            return Err(RegistryError::UnknownTool(name.to_owned()));
        };
        if slot.is_some() {
            return Err(RegistryError::Duplicate(spec.name));
        }
        if !std::ptr::eq(spec.input, A::MODEL) {
            return Err(RegistryError::WrongArguments {
                tool: spec.name,
                expected: spec.input.name,
                actual: A::MODEL.name,
            });
        }
        *slot = Some(Box::new(move |ctx, spec, arguments| {
            let args = parse::<A>(spec, arguments)?;
            handler(ctx, args)
        }));
        Ok(())
    }

    /// Whether the tool `name` has a handler.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.entry(name).is_some()
    }

    /// The tools with handlers, in contract order (the prototype's registration order).
    #[must_use]
    pub fn specs(&self) -> Vec<&'static ToolSpec> {
        TOOLS
            .iter()
            .zip(&self.handlers)
            .filter(|(_, handler)| handler.is_some())
            .map(|(spec, _)| spec)
            .collect()
    }

    /// The tool `name`.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NotFound`]: `"unknown tool '<name>'"`, with the sorted names of the tools
    /// that have handlers as detail `available`.
    pub fn get(&self, name: &str) -> Result<&'static ToolSpec, DomainError> {
        self.entry(name)
            .map(|(spec, _)| spec)
            .ok_or_else(|| self.unknown(name))
    }

    /// The prototype's error for a tool without a handler.
    fn unknown(&self, name: &str) -> DomainError {
        let mut available: Vec<&str> = self.specs().iter().map(|spec| spec.name).collect();
        available.sort_unstable();
        DomainError::new(
            ErrorCode::NotFound,
            format!("unknown tool {}", str_repr(name)),
        )
        .with_detail("available", available)
    }

    fn entry(&self, name: &str) -> Option<(&'static ToolSpec, &Erased)> {
        TOOLS
            .iter()
            .zip(&self.handlers)
            .find(|(spec, _)| spec.name == name)
            .and_then(|(spec, handler)| handler.as_ref().map(|handler| (spec, handler)))
    }

    /// Validates, runs and audits one tool call for the client of `ctx`, and returns the
    /// envelope. Never fails: every error, a panicking handler included, becomes an error
    /// envelope.
    ///
    /// `arguments` is read as the prototype reads `arguments or {}`: missing, `null` and every
    /// other falsy JSON value (`false`, `0`, `""`, `[]`) count as `{}`.
    pub fn call(&self, ctx: &AgentContext, name: &str, arguments: Option<&Value>) -> Value {
        let started = Instant::now();
        let args = effective_arguments(arguments);
        let outcome = self.run(ctx, name, &args);
        let error = outcome.as_ref().err().map(ToolError::code);
        let envelope = match outcome {
            Ok(result) => success(result, name),
            Err(error) => {
                if error.code() == ErrorCode::Internal {
                    log::error!("tool {name} failed: {error}");
                }
                json!({"ok": false, "tool": name, "error": error.to_json()})
            }
        };
        let duration = started.elapsed();
        let project_version = {
            let session = ctx.app().session().lock();
            session.project().map(|_| session.version())
        };
        let record = AuditRecord {
            tool: name,
            args: &args,
            error,
            duration,
            project_version,
            source: ctx.source(),
        };
        if let Err(error) = ctx.audit().record(&record) {
            // The prototype logs the failure and returns the tool's result.
            log::warn!("{error}");
        }
        envelope
    }

    /// Looks the tool up and runs its handler, turning a panic into an `INTERNAL` error (the
    /// prototype reports every unexpected exception that way; ERR-07: a safety net for
    /// defects, so that one broken tool cannot take the server down).
    fn run(&self, ctx: &AgentContext, name: &str, arguments: &Value) -> ToolResult {
        let (spec, handler) = self
            .entry(name)
            .ok_or_else(|| ToolError::Domain(self.unknown(name)))?;
        let result = catch_unwind(AssertUnwindSafe(|| handler(ctx, spec, arguments)))
            .map_err(|payload| ToolError::Internal(panic_message(payload.as_ref())))??;
        if result.is_object() {
            Ok(result)
        } else {
            // `{**result, …}` raises a `TypeError` in the prototype.
            Err(ToolError::Internal(format!(
                "tool {name} returned a non-object result"
            )))
        }
    }
}

/// Python's `arguments or {}`.
fn effective_arguments(arguments: Option<&Value>) -> Value {
    let falsy = match arguments {
        None | Some(Value::Null) => true,
        Some(Value::Bool(value)) => !value,
        Some(Value::Number(number)) => number.as_f64() == Some(0.0),
        Some(Value::String(text)) => text.is_empty(),
        Some(Value::Array(items)) => items.is_empty(),
        Some(Value::Object(map)) => map.is_empty(),
    };
    match arguments {
        Some(value) if !falsy => value.clone(),
        _ => Value::Object(Map::new()),
    }
}

/// `{**result, "ok": True, "tool": name}`: keys keep the result's order, and `ok` and `tool`
/// replace the result's own values in place.
fn success(result: Value, name: &str) -> Value {
    let mut map = match result {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    map.insert("ok".to_owned(), Value::Bool(true));
    map.insert("tool".to_owned(), Value::from(name));
    Value::Object(map)
}

/// The text of a panic payload (`panic!` with a literal or a formatted message).
fn panic_message(payload: &(dyn Any + Send)) -> String {
    let text = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown cause");
    format!("panic: {text}")
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{effective_arguments, panic_message, success};

    #[test]
    fn falsy_arguments_count_as_an_empty_object() {
        for falsy in [
            Value::Null,
            json!(false),
            json!(0),
            json!(0.0),
            json!(""),
            json!([]),
            json!({}),
        ] {
            assert_eq!(effective_arguments(Some(&falsy)), json!({}), "{falsy}");
        }
        assert_eq!(effective_arguments(None), json!({}));
        for truthy in [
            json!(true),
            json!(1),
            json!("x"),
            json!([0]),
            json!({"a": null}),
        ] {
            assert_eq!(effective_arguments(Some(&truthy)), truthy);
        }
    }

    #[test]
    fn success_keeps_result_order_and_overrides_ok_and_tool() {
        let envelope = success(json!({"b": 1, "ok": false, "a": 2, "tool": "fake"}), "real");
        let keys: Vec<&str> = envelope
            .as_object()
            .map(|map| map.keys().map(String::as_str).collect())
            .unwrap_or_default();
        assert_eq!(keys, ["b", "ok", "a", "tool"]);
        assert_eq!(
            envelope,
            json!({"b": 1, "ok": true, "a": 2, "tool": "real"})
        );
    }

    #[test]
    fn panic_payloads_become_messages() {
        assert_eq!(panic_message(&"boom"), "panic: boom");
        assert_eq!(panic_message(&String::from("bad")), "panic: bad");
        assert_eq!(panic_message(&3_i32), "panic: unknown cause");
    }
}
