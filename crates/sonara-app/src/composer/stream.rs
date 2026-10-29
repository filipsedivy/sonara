//! The events of a Claude Code run, parsed from its `--output-format stream-json` output: one
//! JSON object per line (`system/init`, partial `stream_event`s, `assistant` and `user`
//! messages, `system/api_retry` and the final `result`).
//!
//! Only what the composer's panel shows is extracted. Unknown lines are ignored, because the
//! stream grows with each Claude Code release; a line that is not JSON at all becomes
//! [`Event::Output`], so that nothing the process prints is lost.

use serde::Serialize;
use serde_json::Value;

/// The prefix of the Sonara tools' names in a Claude Code run (`mcp__<server>__<tool>`).
const TOOL_PREFIX: &str = "mcp__sonara__";

/// The longest argument summary or result preview, in characters.
const PREVIEW_CHARS: usize = 240;

/// One MCP server in `system/init`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpServer {
    /// The server's name.
    pub name: String,
    /// Its status: `connected`, `failed`, `pending`, …
    pub status: String,
}

/// What a run reports, in the order it happens.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// The session started (`system/init`).
    Init {
        /// The Claude Code session id.
        session_id: Option<String>,
        /// The model.
        model: Option<String>,
        /// How many tools the run has.
        tool_count: usize,
        /// The MCP servers and whether they connected.
        mcp_servers: Vec<McpServer>,
        /// The `--mcp-config` entries that Claude Code skipped.
        mcp_errors: Vec<String>,
    },
    /// A piece of the assistant's text, as it is written.
    Text {
        /// The text.
        text: String,
    },
    /// The assistant calls a tool.
    ToolUse {
        /// The call's id, which its [`Event::ToolResult`] repeats.
        id: String,
        /// The tool's name without the `mcp__sonara__` prefix.
        tool: String,
        /// The arguments as short JSON.
        summary: String,
        /// Whether a subagent made the call.
        nested: bool,
    },
    /// A tool answered.
    ToolResult {
        /// The id of the call.
        id: String,
        /// `false` for a tool error or a Sonara envelope with `"ok": false`.
        ok: bool,
        /// The start of the answer.
        preview: String,
    },
    /// Claude Code retries a failed API request.
    Retry {
        /// The attempt, from 1.
        attempt: u64,
        /// The attempts allowed for this cause.
        max_retries: u64,
        /// The wait before the next attempt.
        delay_ms: u64,
        /// The error category.
        error: String,
    },
    /// The run is over (`result`).
    Result {
        /// Whether the run ended well.
        success: bool,
        /// `success`, `error_max_turns`, `error_during_execution`, …
        subtype: String,
        /// The assistant's final text.
        text: String,
        /// The duration of the run.
        duration_ms: Option<u64>,
        /// The number of turns.
        num_turns: Option<u64>,
        /// The estimated cost in US dollars.
        cost_usd: Option<f64>,
        /// The Claude Code session id.
        session_id: Option<String>,
        /// The token usage, as Claude Code reports it.
        usage: Value,
        /// The tools that were denied.
        permission_denials: Vec<String>,
        /// The whole `result` line, pretty-printed: everything Claude Code returned.
        raw: String,
    },
    /// A line of the process's standard error.
    Stderr {
        /// The line.
        line: String,
    },
    /// A line of the process's standard output that is not JSON.
    Output {
        /// The line.
        line: String,
    },
    /// The process ended. Always the last event.
    Exited {
        /// The exit code; `None` when a signal ended it.
        code: Option<i32>,
        /// Whether the user asked to stop.
        stopped: bool,
    },
}

/// The events of one line of the stream; none for a line the panel has no use for.
#[must_use]
pub fn parse_line(line: &str) -> Vec<Event> {
    let line = line.trim();
    if line.is_empty() {
        return Vec::new();
    }
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return vec![Event::Output {
            line: line.to_owned(),
        }];
    };
    match value["type"].as_str() {
        Some("system") => system(&value),
        Some("stream_event") => text_delta(&value).into_iter().collect(),
        Some("assistant") => tool_uses(&value),
        Some("user") => tool_results(&value),
        Some("result") => vec![result(&value)],
        _ => Vec::new(),
    }
}

fn system(value: &Value) -> Vec<Event> {
    match value["subtype"].as_str() {
        Some("init") => vec![Event::Init {
            session_id: string(&value["session_id"]),
            model: string(&value["model"]),
            tool_count: value["tools"].as_array().map_or(0, Vec::len),
            mcp_servers: value["mcp_servers"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|server| McpServer {
                    name: string(&server["name"]).unwrap_or_default(),
                    status: string(&server["status"]).unwrap_or_default(),
                })
                .collect(),
            mcp_errors: value["mcp_server_errors"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|error| {
                    format!(
                        "{}: {}",
                        error["name"].as_str().unwrap_or("?"),
                        error["message"].as_str().unwrap_or("skipped")
                    )
                })
                .collect(),
        }],
        Some("api_retry") => vec![Event::Retry {
            attempt: value["attempt"].as_u64().unwrap_or(0),
            max_retries: value["max_retries"].as_u64().unwrap_or(0),
            delay_ms: value["retry_delay_ms"].as_u64().unwrap_or(0),
            error: string(&value["error"]).unwrap_or_else(|| "unknown".to_owned()),
        }],
        _ => Vec::new(),
    }
}

fn text_delta(value: &Value) -> Option<Event> {
    let delta = &value["event"]["delta"];
    (delta["type"].as_str() == Some("text_delta")).then(|| Event::Text {
        text: delta["text"].as_str().unwrap_or_default().to_owned(),
    })
}

fn tool_uses(value: &Value) -> Vec<Event> {
    let nested = !value["parent_tool_use_id"].is_null();
    value["message"]["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block["type"].as_str() == Some("tool_use"))
        .map(|block| {
            let name = block["name"].as_str().unwrap_or("?");
            Event::ToolUse {
                id: string(&block["id"]).unwrap_or_default(),
                tool: name.strip_prefix(TOOL_PREFIX).unwrap_or(name).to_owned(),
                summary: shorten(&block["input"].to_string()),
                nested,
            }
        })
        .collect()
}

fn tool_results(value: &Value) -> Vec<Event> {
    value["message"]["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block["type"].as_str() == Some("tool_result"))
        .map(|block| {
            let text = result_text(&block["content"]);
            let failed_envelope = serde_json::from_str::<Value>(&text)
                .is_ok_and(|envelope| envelope["ok"] == Value::Bool(false));
            Event::ToolResult {
                id: string(&block["tool_use_id"]).unwrap_or_default(),
                ok: !block["is_error"].as_bool().unwrap_or(false) && !failed_envelope,
                preview: shorten(&text),
            }
        })
        .collect()
}

/// The text of a tool result: a string, or the text blocks of a list.
fn result_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| block["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn result(value: &Value) -> Event {
    let subtype = string(&value["subtype"]).unwrap_or_default();
    Event::Result {
        success: !value["is_error"].as_bool().unwrap_or(false) && subtype == "success",
        subtype,
        text: string(&value["result"]).unwrap_or_default(),
        duration_ms: value["duration_ms"].as_u64(),
        num_turns: value["num_turns"].as_u64(),
        cost_usd: value["total_cost_usd"].as_f64(),
        session_id: string(&value["session_id"]),
        usage: value["usage"].clone(),
        permission_denials: value["permission_denials"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|denial| {
                denial["tool_name"].as_str().map_or_else(
                    || denial.to_string(),
                    |name| name.strip_prefix(TOOL_PREFIX).unwrap_or(name).to_owned(),
                )
            })
            .collect(),
        raw: serde_json::to_string_pretty(value).unwrap_or_default(),
    }
}

fn string(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

/// `text` cut to [`PREVIEW_CHARS`] characters, with an ellipsis when it was longer.
fn shorten(text: &str) -> String {
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(PREVIEW_CHARS).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_reports_the_model_and_whether_sonara_connected() {
        let events = parse_line(
            r#"{"type":"system","subtype":"init","session_id":"s1","model":"claude-x",
            "tools":["a","b","c"],
            "mcp_servers":[{"name":"sonara","status":"connected"}],
            "mcp_server_errors":[{"name":"other","type":"invalid_config","message":"bad"}]}"#,
        );
        assert_eq!(
            events,
            [Event::Init {
                session_id: Some("s1".into()),
                model: Some("claude-x".into()),
                tool_count: 3,
                mcp_servers: vec![McpServer {
                    name: "sonara".into(),
                    status: "connected".into()
                }],
                mcp_errors: vec!["other: bad".into()],
            }]
        );
    }

    #[test]
    fn only_text_deltas_of_partial_messages_become_text() {
        let text = parse_line(
            r#"{"type":"stream_event","event":{"type":"content_block_delta",
            "delta":{"type":"text_delta","text":"Hel"}}}"#,
        );
        assert_eq!(text, [Event::Text { text: "Hel".into() }]);
        for ignored in [
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"{"}}}"#,
            r#"{"type":"stream_event","event":{"type":"message_start"}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"hm"}}}"#,
        ] {
            assert_eq!(parse_line(ignored), [], "{ignored}");
        }
    }

    #[test]
    fn a_tool_call_names_the_tool_without_the_server_prefix() {
        let events = parse_line(
            r#"{"type":"assistant","parent_tool_use_id":null,"message":{"content":[
            {"type":"text","text":"Adding a bass."},
            {"type":"tool_use","id":"t1","name":"mcp__sonara__create_track",
             "input":{"name":"Bass","kind":"instrument"}},
            {"type":"tool_use","id":"t2","name":"WebSearch","input":{"query":"dorian"}}]}}"#,
        );
        assert_eq!(
            events,
            [
                Event::ToolUse {
                    id: "t1".into(),
                    tool: "create_track".into(),
                    // `serde_json` keeps the order of the keys.
                    summary: r#"{"name":"Bass","kind":"instrument"}"#.into(),
                    nested: false,
                },
                Event::ToolUse {
                    id: "t2".into(),
                    tool: "WebSearch".into(),
                    summary: r#"{"query":"dorian"}"#.into(),
                    nested: false,
                },
            ]
        );
    }

    #[test]
    fn a_sonara_error_envelope_is_a_failed_result_even_without_is_error() {
        let failed = parse_line(
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1",
            "content":[{"type":"text","text":"{\"ok\":false,\"error\":{\"code\":\"NOT_FOUND\"}}"}]}]}}"#,
        );
        let good = parse_line(
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t2",
            "content":"{\"ok\":true,\"tool\":\"create_track\"}"}]}}"#,
        );
        let errored = parse_line(
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t3",
            "is_error":true,"content":"denied"}]}}"#,
        );
        let ok_of = |events: &[Event]| match events {
            [Event::ToolResult { ok, .. }] => *ok,
            other => panic!("{other:?}"),
        };
        assert!(!ok_of(&failed));
        assert!(ok_of(&good));
        assert!(!ok_of(&errored));
    }

    #[test]
    fn a_long_answer_is_cut_on_a_character_boundary() {
        let long = "á".repeat(PREVIEW_CHARS + 50);
        let line = serde_json::json!({"type":"user","message":{"content":[
            {"type":"tool_result","tool_use_id":"t","content":long}]}})
        .to_string();
        let events = parse_line(&line);
        let [Event::ToolResult { preview, .. }] = events.as_slice() else {
            panic!("one result");
        };
        assert_eq!(preview.chars().count(), PREVIEW_CHARS + 1);
        assert!(preview.ends_with('…'));
    }

    #[test]
    fn the_result_keeps_everything_claude_code_returned() {
        let events = parse_line(
            r#"{"type":"result","subtype":"success","is_error":false,"result":"Done.",
            "duration_ms":4200,"num_turns":7,"total_cost_usd":0.12,"session_id":"s1",
            "usage":{"input_tokens":10,"output_tokens":20},
            "permission_denials":[{"tool_name":"mcp__sonara__save_project","tool_use_id":"x"}]}"#,
        );
        let [
            Event::Result {
                success,
                subtype,
                text,
                duration_ms,
                num_turns,
                cost_usd,
                session_id,
                usage,
                permission_denials,
                raw,
            },
        ] = events.as_slice()
        else {
            panic!("one result");
        };
        assert!(*success);
        assert_eq!(subtype, "success");
        assert_eq!(text, "Done.");
        assert_eq!(*duration_ms, Some(4200));
        assert_eq!(*num_turns, Some(7));
        assert_eq!(*cost_usd, Some(0.12));
        assert_eq!(session_id.as_deref(), Some("s1"));
        assert_eq!(usage["output_tokens"], 20);
        assert_eq!(permission_denials, &["save_project".to_owned()]);
        assert!(raw.contains("\"total_cost_usd\": 0.12"), "{raw}");
    }

    #[test]
    fn an_error_result_is_not_a_success() {
        let events = parse_line(
            r#"{"type":"result","subtype":"error_max_turns","is_error":true,"result":""}"#,
        );
        assert!(matches!(
            events.as_slice(),
            [Event::Result { success: false, subtype, .. }] if subtype == "error_max_turns"
        ));
    }

    #[test]
    fn retries_are_reported_and_unknown_or_blank_lines_are_not() {
        assert_eq!(
            parse_line(
                r#"{"type":"system","subtype":"api_retry","attempt":2,"max_retries":10,
                "retry_delay_ms":2000,"error":"rate_limit"}"#
            ),
            [Event::Retry {
                attempt: 2,
                max_retries: 10,
                delay_ms: 2000,
                error: "rate_limit".into()
            }]
        );
        assert_eq!(parse_line(""), []);
        assert_eq!(parse_line(r#"{"type":"rate_limit_event"}"#), []);
        assert_eq!(
            parse_line(r#"{"type":"system","subtype":"hook_started"}"#),
            []
        );
    }

    #[test]
    fn a_line_that_is_not_json_is_kept_as_output() {
        assert_eq!(
            parse_line("Error: not logged in"),
            [Event::Output {
                line: "Error: not logged in".into()
            }]
        );
    }
}
