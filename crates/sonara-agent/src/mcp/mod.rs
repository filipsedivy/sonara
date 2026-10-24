//! The MCP server (`sonara.agent.mcp_server`): the tools of
//! a [`ToolBackend`] over the Model Context Protocol on stdio, through the official Rust SDK
//! (`rmcp`). `sonara mcp` runs it.
//!
//! The JSON the server sends is built here, as the prototype's server builds it, and checked
//! against the golden data (`contracts/mcp-tools-list.json`):
//!
//! - `initialize`: [`server_info`], [`capabilities`] and [`INSTRUCTIONS`]; the protocol version
//!   is the client's when the SDK supports it (`rmcp` and the prototype's SDK know the same
//!   versions and fall back to the same one);
//! - `tools/list`: every tool of the backend ([`ToolBackend::tools`]) as [`listed_tool`] shows
//!   it;
//! - `tools/call`: [`call_result`] of the backend's envelope, with the envelope as JSON text
//!   ([`dumps`] with `indent=1`), as structured content, and `isError` when it is not `ok`.
//!
//! Only [`run_stdio`] is asynchronous: it owns a `tokio` runtime, and tool calls run on its
//! blocking threads (the prototype's `anyio.to_thread.run_sync`), so pings are answered while a
//! long tool runs.

mod server;

use serde_json::{Map, Value, json};
use sonara_core::error::DomainError;

use crate::contract::{Safety, TOOLS, ToolSpec};
use crate::ipc::IpcClient;
use crate::service::AgentService;

pub use server::{McpError, run_stdio};
pub use sonara_app::pyjson::dumps;

/// The server name reported in `initialize`.
pub const SERVER_NAME: &str = "sonara";

/// The server title reported in `initialize`.
pub const SERVER_TITLE: &str = "Sonara DAW";

/// The instructions reported in `initialize` (verbatim from the prototype).
pub const INSTRUCTIONS: &str = "\
Sonara is a digital audio workstation. Workflow: create_project (or open_project) ->
set_tempo / set_time_signature / add_section -> create_track (instrument tracks take a
plugin_id + preset from list_plugins) -> generate_midi or create_midi_clip/add_midi_notes ->
add_effect / configure_routing (sends, sidechain) / create_automation -> render_preview
(returns loudness, peaks, clipping, spectrum and issues) -> revise -> export_audio /
export_midi -> save_project. Inspect real state with get_project_state; positions are beats
(0 = song start), 'bar:beat' strings or '12.5s'. Destructive actions return
CONFIRMATION_REQUIRED with a token: ask the user, then repeat the call with the token.
All edits are undoable (undo/redo).";

/// What the MCP server and the `sonara` command run tools on (the prototype's `ToolBackend`
/// protocol): the in-process [`AgentService`], or an [`IpcClient`] of a running session.
pub trait ToolBackend: Send + Sync {
    /// Calls the tool `name` with `arguments` and returns its envelope (`{"ok": …, "tool": …}`);
    /// a failed tool is an envelope too.
    ///
    /// # Errors
    ///
    /// The error that kept the call from reaching the tools: a running session that cannot be
    /// asked (the prototype's `IPCClient.call` raises it as a `SonaraError`). An in-process
    /// service never fails here.
    fn call(&self, name: &str, arguments: Option<&Value>) -> Result<Value, DomainError>;

    /// The tools the backend runs, as the tool contract lists them (`ToolSpec.to_json`:
    /// `{name, title, description, safety, input_schema}`).
    ///
    /// # Errors
    ///
    /// As [`ToolBackend::call`].
    fn tools(&self) -> Result<Vec<Value>, DomainError>;
}

impl ToolBackend for AgentService {
    fn call(&self, name: &str, arguments: Option<&Value>) -> Result<Value, DomainError> {
        Ok(AgentService::call(self, name, arguments))
    }

    fn tools(&self) -> Result<Vec<Value>, DomainError> {
        Ok(AgentService::tools(self))
    }
}

impl ToolBackend for IpcClient {
    fn call(&self, name: &str, arguments: Option<&Value>) -> Result<Value, DomainError> {
        IpcClient::call(self, name, arguments).map_err(|error| error.to_domain())
    }

    fn tools(&self) -> Result<Vec<Value>, DomainError> {
        IpcClient::tools(self).map_err(|error| error.to_domain())
    }
}

/// The `serverInfo` of the `initialize` result: name, title and the application version.
#[must_use]
pub fn server_info() -> Value {
    json!({
        "name": SERVER_NAME,
        "title": SERVER_TITLE,
        "version": env!("CARGO_PKG_VERSION"),
    })
}

/// The server capabilities of the `initialize` result: tools, with a fixed list.
#[must_use]
pub fn capabilities() -> Value {
    json!({"tools": {"listChanged": false}})
}

/// A JSON Schema in the key order of pydantic's `model_json_schema` (`GenerateJsonSchema.sort`),
/// which the prototype's tools carried on the wire: the keys of every object sorted, except the
/// direct children of `properties` and `default`, which keep their order (field definition
/// order, and a default value as written).
#[must_use]
pub fn pydantic_order(schema: &Value) -> Value {
    sort_keys(schema, None)
}

/// `GenerateJsonSchema._sort_recursive`: list items inherit their parent's key.
fn sort_keys(value: &Value, parent: Option<&str>) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            if !matches!(parent, Some("properties" | "default")) {
                entries.sort_by_key(|(key, _)| *key);
            }
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, item)| (key.clone(), sort_keys(item, Some(key))))
                    .collect(),
            )
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| sort_keys(item, parent)).collect())
        }
        other => other.clone(),
    }
}

/// One tool of the `tools/list` result: `name`, `title`, `description`, `inputSchema` (in
/// [`pydantic_order`]) and the annotations the prototype derives from the safety class.
/// `idempotentHint` and `openWorldHint` are always false, as in the prototype.
#[must_use]
pub fn tool(spec: &ToolSpec) -> Value {
    json!({
        "name": spec.name,
        "title": spec.title,
        "description": spec.description(),
        "inputSchema": pydantic_order(&spec.input.input_schema()),
        "annotations": {
            "title": spec.title,
            "readOnlyHint": spec.safety == Safety::Read,
            "destructiveHint": spec.safety == Safety::Destructive,
            "idempotentHint": false,
            "openWorldHint": false,
        },
    })
}

/// The `tools` of the `tools/list` result for every tool of the contract, in registration order.
#[must_use]
pub fn tools() -> Vec<Value> {
    TOOLS.iter().map(tool).collect()
}

/// One tool of the `tools/list` result from the tool as a backend lists it
/// ([`ToolBackend::tools`]), as the prototype's `list_tools` builds it: a missing safety class
/// counts as `write`, and a missing or `null` title is left out. The input schema is put in
/// [`pydantic_order`]. For a tool of the contract this equals [`tool`].
#[must_use]
pub fn listed_tool(spec: &Value) -> Value {
    let title = spec.get("title").filter(|title| !title.is_null());
    let safety = spec
        .get("safety")
        .and_then(Value::as_str)
        .unwrap_or("write");
    let mut annotations = Map::new();
    if let Some(title) = title {
        annotations.insert("title".to_owned(), title.clone());
    }
    annotations.insert("readOnlyHint".to_owned(), (safety == "read").into());
    annotations.insert(
        "destructiveHint".to_owned(),
        (safety == "destructive").into(),
    );
    annotations.insert("idempotentHint".to_owned(), false.into());
    annotations.insert("openWorldHint".to_owned(), false.into());
    let mut out = Map::new();
    out.insert(
        "name".to_owned(),
        spec.get("name").cloned().unwrap_or(Value::Null),
    );
    if let Some(title) = title {
        out.insert("title".to_owned(), title.clone());
    }
    out.insert(
        "description".to_owned(),
        spec.get("description").cloned().unwrap_or(Value::Null),
    );
    out.insert(
        "inputSchema".to_owned(),
        spec.get("input_schema").map_or(Value::Null, pydantic_order),
    );
    out.insert("annotations".to_owned(), Value::Object(annotations));
    Value::Object(out)
}

/// The `tools/call` result of a tool's `envelope`: the envelope as text
/// (`json.dumps(envelope, indent=1)`), as structured content, and `isError` unless the envelope
/// is `ok`.
#[must_use]
pub fn call_result(envelope: &Value) -> Value {
    let ok = envelope.get("ok").and_then(Value::as_bool) == Some(true);
    json!({
        "content": [{"type": "text", "text": dumps(envelope, 1)}],
        "structuredContent": envelope,
        "isError": !ok,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{call_result, listed_tool, pydantic_order, tool};
    use crate::contract::{TOOLS, find};

    /// Keys in order, at every level (`serde_json`'s equality ignores the order).
    fn ordered(value: &serde_json::Value) -> String {
        serde_json::to_string(value).unwrap()
    }

    #[test]
    fn listed_tools_of_the_contract_equal_the_contract_listing() {
        for spec in &TOOLS {
            let listed = listed_tool(&spec.to_json());
            assert_eq!(ordered(&listed), ordered(&tool(spec)), "{}", spec.name);
        }
    }

    #[test]
    fn schemas_take_pydantics_key_order() {
        // `json.dumps(registry.get("set_tempo").input_schema())` of the prototype, keys in its
        // order.
        let expected: serde_json::Value = serde_json::from_str(
            r#"{"additionalProperties": false, "properties": {"dry_run": {"default": false, "description": "Validate and describe the change without committing it", "title": "Dry Run", "type": "boolean"}, "bpm": {"maximum": 999, "minimum": 10, "title": "Bpm", "type": "number"}, "at": {"anyOf": [{"type": "number"}, {"type": "string"}], "default": 0.0, "description": "Where the tempo starts (0 = song tempo)", "title": "At"}, "ramp": {"default": false, "description": "Glide linearly from this point to the next tempo point", "title": "Ramp", "type": "boolean"}, "clear_changes": {"default": false, "description": "Replace all tempo changes with one constant tempo", "title": "Clear Changes", "type": "boolean"}}, "required": ["bpm"], "type": "object"}"#,
        )
        .unwrap();
        let spec = find("set_tempo").unwrap();
        let schema = pydantic_order(&spec.input.input_schema());
        assert_eq!(schema, expected);
        assert_eq!(ordered(&schema), ordered(&expected));
        // `default` values keep their own order.
        let value =
            json!({"b": 1, "default": {"z": 1, "a": {"y": 2, "x": 3}}, "a": [{"d": 1, "c": 2}]});
        assert_eq!(
            ordered(&pydantic_order(&value)),
            r#"{"a":[{"c":2,"d":1}],"b":1,"default":{"z":1,"a":{"x":3,"y":2}}}"#
        );
    }

    #[test]
    fn listed_tool_defaults_follow_the_prototype() {
        let listed = listed_tool(&json!({
            "name": "x",
            "title": null,
            "description": "d",
            "input_schema": {"type": "object"},
        }));
        assert_eq!(
            listed,
            json!({
                "name": "x",
                "description": "d",
                "inputSchema": {"type": "object"},
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": false,
                    "idempotentHint": false,
                    "openWorldHint": false,
                },
            })
        );
    }

    #[test]
    fn call_results_carry_the_envelope_twice() {
        let ok = json!({"ok": true, "tool": "undo", "label": "\u{e9}"});
        assert_eq!(
            call_result(&ok),
            json!({
                "content": [{
                    "type": "text",
                    "text": "{\n \"ok\": true,\n \"tool\": \"undo\",\n \"label\": \"\\u00e9\"\n}",
                }],
                "structuredContent": ok,
                "isError": false,
            })
        );
        let failed = json!({"ok": false, "tool": "x", "error": {"code": "NOT_FOUND"}});
        assert_eq!(call_result(&failed)["isError"], json!(true));
        assert_eq!(call_result(&json!({}))["isError"], json!(true));
    }
}
