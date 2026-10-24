//! `sonara batch`: a sequence of tool calls in one session (`run_batch` of `sonara.cli.main`).
//!
//! A batch is `[{"id": "x", "tool": "...", "args": {...}}, ...]`. `{{x.path}}` in the arguments of
//! a later step refers to an earlier step's full envelope (`{{bass.result.track_id}}`,
//! `{{gen.notes[0]}}`): a string that is exactly one reference takes the referenced value; a
//! reference inside a longer string is replaced by the value's Python `str()`.
//!
//! A step with a truthy `confirm` is pre-approved by the author of the batch file: when it
//! returns `CONFIRMATION_REQUIRED`, it is repeated once with the issued token. A failed step stops
//! the batch unless the batch keeps going or the step has a truthy `allow_error`.
//!
//! The prototype lets malformed batches end in a Python exception (a step that is not an object or
//! has no `tool`, a reference to a missing result or key); here they end with a [`BatchError`].
//! The steps before it have run. A call that cannot reach the tools (a running session that does
//! not answer) ends the batch too, as its `SonaraError` ends the prototype's.

use std::sync::LazyLock;

use regex::{Captures, Regex};
use serde_json::{Map, Value};
use sonara_agent::mcp::ToolBackend;
use sonara_core::error::DomainError;
use sonara_core::pyfmt::{float_repr, str_repr};

/// `{{ name(.key|[index])* }}` (`_REF`).
static REF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{\s*([A-Za-z0-9_\-]+)((?:\.[A-Za-z0-9_\-]+|\[\d+\])*)\s*\}\}")
        .expect("the reference pattern is a valid literal")
});

/// One part of a reference path.
static REF_PART: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\.([A-Za-z0-9_\-]+)|\[(\d+)\]")
        .expect("the reference part pattern is a valid literal")
});

/// A batch that cannot run on (where the prototype raises).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum BatchError {
    /// A step is not a JSON object.
    #[error("step {index} is not an object")]
    NotAnObject {
        /// The step's position, from 0.
        index: usize,
    },
    /// A step names no tool, or not as a string.
    #[error("step {index} has no tool name")]
    NoTool {
        /// The step's position, from 0.
        index: usize,
    },
    /// A reference does not resolve.
    #[error("step {index}: cannot resolve {reference}: {reason}")]
    Reference {
        /// The step's position, from 0.
        index: usize,
        /// The reference as written, for example `{{bass.result.track_id}}`.
        reference: String,
        /// What is missing.
        reason: String,
    },
    /// A confirmation cannot be repeated: the arguments are not an object, or the error carries
    /// no token.
    #[error("step {index}: cannot repeat the call with its confirmation token: {reason}")]
    Confirmation {
        /// The step's position, from 0.
        index: usize,
        /// What is missing.
        reason: String,
    },
    /// A call did not reach the tools ([`ToolBackend::call`]).
    #[error(transparent)]
    Call(#[from] DomainError),
}

/// What one step reports to the caller while the batch runs: the step ID, the tool and the
/// envelope.
pub type StepLog<'a> = dyn FnMut(&Value, &str, &Value) + 'a;

/// Runs `steps` on `backend` and returns one `{id, tool, ok[, error]}` entry per step that ran.
///
/// - `stop_on_error`: stop at the first failed step without `allow_error`;
/// - `results`: receives every step's full envelope under its ID (the prototype's `results`);
/// - `log`: called after every step (the prototype's `log`).
///
/// # Errors
///
/// [`BatchError`] for a malformed step, a reference that does not resolve, or a call that does
/// not reach the tools.
pub fn run_batch(
    backend: &dyn ToolBackend,
    steps: &[Value],
    stop_on_error: bool,
    results: &mut Map<String, Value>,
    log: &mut StepLog<'_>,
) -> Result<Vec<Value>, BatchError> {
    let mut outputs = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        let Value::Object(step) = step else {
            return Err(BatchError::NotAnObject { index });
        };
        // `step.get("id") or f"step{i}"`.
        let sid = step
            .get("id")
            .filter(|id| truthy(id))
            .cloned()
            .unwrap_or_else(|| Value::String(format!("step{index}")));
        let Some(tool) = step.get("tool").and_then(Value::as_str) else {
            return Err(BatchError::NoTool { index });
        };
        let empty = Value::Object(Map::new());
        let args = substitute(step.get("args").unwrap_or(&empty), results, index)?;
        let mut res = backend.call(tool, Some(&args))?;
        if !is_ok(&res)
            && step.get("confirm").is_some_and(truthy)
            && res.pointer("/error/code").and_then(Value::as_str) == Some("CONFIRMATION_REQUIRED")
        {
            let token = res
                .pointer("/error/details/confirmation_token")
                .cloned()
                .ok_or_else(|| BatchError::Confirmation {
                    index,
                    reason: "the error carries no confirmation_token".to_owned(),
                })?;
            let Value::Object(mut confirmed) = args else {
                return Err(BatchError::Confirmation {
                    index,
                    reason: "the arguments are not an object".to_owned(),
                });
            };
            confirmed.insert("confirmation_token".to_owned(), token);
            res = backend.call(tool, Some(&Value::Object(confirmed)))?;
        }
        results.insert(py_str(&sid), res.clone());
        let mut output = Map::new();
        output.insert("id".to_owned(), sid.clone());
        output.insert("tool".to_owned(), Value::from(tool));
        output.insert(
            "ok".to_owned(),
            res.get("ok").cloned().unwrap_or(Value::Null),
        );
        let failed = !is_ok(&res);
        if failed {
            output.insert(
                "error".to_owned(),
                res.get("error").cloned().unwrap_or(Value::Null),
            );
        }
        outputs.push(Value::Object(output));
        log(&sid, tool, &res);
        if failed && stop_on_error && !step.get("allow_error").is_some_and(truthy) {
            break;
        }
    }
    Ok(outputs)
}

/// Whether an envelope is `ok` (`res.get("ok")`, truthy).
#[must_use]
pub fn is_ok(envelope: &Value) -> bool {
    envelope.get("ok").is_some_and(truthy)
}

/// Python's truthiness of a JSON value: `null`, `false`, `0`, `""`, `[]` and `{}` are false.
#[must_use]
pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|n| n != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}

/// Python's `str()` of a JSON value: a string as it is, anything else as its `repr`.
#[must_use]
pub fn py_str(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => py_repr(other),
    }
}

/// Python's `repr()` of a JSON value: `None`, `True`, `1`, `1.5`, `'text'`, `[1, 2]`,
/// `{'key': 'value'}`.
fn py_repr(value: &Value) -> String {
    match value {
        Value::Null => "None".to_owned(),
        Value::Bool(true) => "True".to_owned(),
        Value::Bool(false) => "False".to_owned(),
        Value::Number(number) => {
            if let Some(int) = number.as_i64() {
                int.to_string()
            } else if let Some(uint) = number.as_u64() {
                uint.to_string()
            } else {
                float_repr(number.as_f64().unwrap_or(f64::NAN))
            }
        }
        Value::String(text) => str_repr(text),
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().map(py_repr).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Object(map) => {
            let parts: Vec<String> = map
                .iter()
                .map(|(key, item)| format!("{}: {}", str_repr(key), py_repr(item)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

/// `_lookup`: the value at `path` (`.key` and `[index]` parts) of the result `name`.
fn lookup(
    results: &Map<String, Value>,
    name: &str,
    path: &str,
    reference: &str,
    index: usize,
) -> Result<Value, BatchError> {
    let missing = |reason: String| BatchError::Reference {
        index,
        reference: reference.to_owned(),
        reason,
    };
    let mut current = results
        .get(name)
        .ok_or_else(|| missing(format!("no earlier step {}", str_repr(name))))?;
    for part in REF_PART.captures_iter(path) {
        current = match (part.get(1), part.get(2)) {
            (Some(key), _) => current
                .as_object()
                .and_then(|map| map.get(key.as_str()))
                .ok_or_else(|| missing(format!("no key {}", str_repr(key.as_str()))))?,
            (None, Some(position)) => position
                .as_str()
                .parse::<usize>()
                .ok()
                .and_then(|i| current.as_array().and_then(|items| items.get(i)))
                .ok_or_else(|| missing(format!("no item {}", position.as_str())))?,
            (None, None) => current,
        };
    }
    Ok(current.clone())
}

/// `_substitute`: references in every string of `value`.
fn substitute(
    value: &Value,
    results: &Map<String, Value>,
    index: usize,
) -> Result<Value, BatchError> {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            if let Some(whole) = REF
                .captures(trimmed)
                .filter(|c| c.get(0).is_some_and(|m| m.len() == trimmed.len()))
            {
                return lookup(results, &whole[1], &whole[2], &whole[0], index);
            }
            let mut failure = None;
            let replaced = REF.replace_all(text, |c: &Captures<'_>| {
                match lookup(results, &c[1], &c[2], &c[0], index) {
                    Ok(found) => py_str(&found),
                    Err(error) => {
                        failure.get_or_insert(error);
                        String::new()
                    }
                }
            });
            match failure {
                Some(error) => Err(error),
                None => Ok(Value::String(replaced.into_owned())),
            }
        }
        Value::Array(items) => items
            .iter()
            .map(|item| substitute(item, results, index))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::Object(map) => map
            .iter()
            .map(|(key, item)| Ok((key.clone(), substitute(item, results, index)?)))
            .collect::<Result<Map<_, _>, _>>()
            .map(Value::Object),
        other => Ok(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::{Map, Value, json};
    use sonara_agent::mcp::ToolBackend;
    use sonara_core::error::{DomainError, ErrorCode};

    use super::{BatchError, py_str, run_batch, substitute, truthy};

    /// Records every call; `fail` fails with the given code, `confirm` needs a token.
    #[derive(Default)]
    struct Recorder {
        calls: Mutex<Vec<(String, Value)>>,
    }

    impl ToolBackend for Recorder {
        fn call(&self, name: &str, arguments: Option<&Value>) -> Result<Value, DomainError> {
            let arguments = arguments.cloned().unwrap_or(Value::Null);
            self.calls
                .lock()
                .unwrap()
                .push((name.to_owned(), arguments.clone()));
            if name == "lost" {
                return Err(DomainError::new(
                    ErrorCode::EngineUnavailable,
                    "connection closed by Sonara",
                ));
            }
            Ok(match name {
                "fail" => json!({"ok": false, "tool": name, "error": {"code": "NOT_FOUND"}}),
                "confirm" if arguments.get("confirmation_token").is_none() => json!({
                    "ok": false,
                    "tool": name,
                    "error": {
                        "code": "CONFIRMATION_REQUIRED",
                        "details": {"confirmation_token": "t-1"},
                    },
                }),
                _ => json!({"ok": true, "tool": name, "args": arguments, "list": [1.5, true]}),
            })
        }

        fn tools(&self) -> Result<Vec<Value>, DomainError> {
            Ok(Vec::new())
        }
    }

    fn run(backend: &Recorder, steps: &Value, stop: bool) -> Result<Vec<Value>, BatchError> {
        let mut results = Map::new();
        let mut seen = Vec::new();
        let out = run_batch(
            backend,
            steps.as_array().unwrap(),
            stop,
            &mut results,
            &mut |sid, tool, _| seen.push(format!("{}:{tool}", py_str(sid))),
        )?;
        assert_eq!(seen.len(), out.len());
        Ok(out)
    }

    #[test]
    fn references_resolve_whole_and_inside_strings() {
        let mut results = Map::new();
        results.insert(
            "a".to_owned(),
            json!({"result": {"id": "trk_1", "n": [3, 1.5e-05]}, "flag": true, "none": null}),
        );
        let value = json!({
            "whole": " {{ a.result.n }} ",
            "item": "{{a.result.n[1]}}",
            "text": "x={{a.result.id}} f={{a.result.n[1]}} b={{a.flag}} n={{a.none}} l={{a.result.n}}",
            "list": ["{{a.result.id}}", 4],
        });
        assert_eq!(
            substitute(&value, &results, 0).unwrap(),
            json!({
                "whole": [3, 1.5e-05],
                "item": 1.5e-05,
                "text": "x=trk_1 f=1.5e-05 b=True n=None l=[3, 1.5e-05]",
                "list": ["trk_1", 4],
            })
        );
        let error = substitute(&json!("{{b.x}}"), &results, 2).unwrap_err();
        assert!(matches!(error, BatchError::Reference { index: 2, .. }));
        assert!(substitute(&json!("{{a.result.n[5]}}"), &results, 0).is_err());
        assert!(substitute(&json!("v {{a.flag.x}}"), &results, 0).is_err());
    }

    #[test]
    fn ids_errors_and_stops_follow_the_prototype() {
        let backend = Recorder::default();
        let steps = json!([
            {"id": "", "tool": "ok"},
            {"id": "f", "tool": "fail", "allow_error": 1},
            {"tool": "ok", "args": {"v": "{{step0.tool}}"}},
            {"id": 7, "tool": "fail"},
            {"tool": "ok"},
        ]);
        let out = run(&backend, &steps, true).unwrap();
        assert_eq!(
            out,
            json!([
                {"id": "step0", "tool": "ok", "ok": true},
                {"id": "f", "tool": "fail", "ok": false, "error": {"code": "NOT_FOUND"}},
                {"id": "step2", "tool": "ok", "ok": true},
                {"id": 7, "tool": "fail", "ok": false, "error": {"code": "NOT_FOUND"}},
            ])
            .as_array()
            .unwrap()
            .clone()
        );
        assert_eq!(backend.calls.lock().unwrap()[2].1, json!({"v": "ok"}));
        let out = run(&Recorder::default(), &steps, false).unwrap();
        assert_eq!(out.len(), 5);
    }

    #[test]
    fn confirmed_steps_repeat_once_with_the_token() {
        let backend = Recorder::default();
        let steps = json!([
            {"tool": "confirm", "args": {"a": 1}, "confirm": true},
            {"tool": "confirm", "args": {"a": 2}},
        ]);
        let out = run(&backend, &steps, true).unwrap();
        let oks: Vec<&Value> = out.iter().map(|o| &o["ok"]).collect();
        assert_eq!(oks, [&json!(true), &json!(false)]);
        let calls = backend.calls.lock().unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[1].1, json!({"a": 1, "confirmation_token": "t-1"}));
    }

    #[test]
    fn malformed_steps_are_errors() {
        let backend = Recorder::default();
        assert_eq!(
            run(&backend, &json!([{"tool": "ok"}, 5]), true),
            Err(BatchError::NotAnObject { index: 1 })
        );
        assert_eq!(
            run(&backend, &json!([{"args": {}}]), true),
            Err(BatchError::NoTool { index: 0 })
        );
        assert!(matches!(
            run(&backend, &json!([{"tool": "lost"}]), false),
            Err(BatchError::Call(error)) if error.code == ErrorCode::EngineUnavailable
        ));
    }

    #[test]
    fn python_truthiness_and_str() {
        for falsy in [
            json!(null),
            json!(false),
            json!(0),
            json!(0.0),
            json!(""),
            json!([]),
            json!({}),
        ] {
            assert!(!truthy(&falsy), "{falsy}");
        }
        for value in [
            json!(true),
            json!(-1),
            json!("x"),
            json!([0]),
            json!({"a": null}),
        ] {
            assert!(truthy(&value), "{value}");
        }
        assert_eq!(
            py_str(&json!({"a": [1, 2.0, "it's", null]})),
            "{'a': [1, 2.0, \"it's\", None]}"
        );
    }
}
