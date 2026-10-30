//! The events of a Codex run, parsed from `codex exec --json`: one JSON object per line
//! (`thread.started`, `turn.started`, `item.started`, `item.updated`, `item.completed`,
//! `turn.completed`, `turn.failed` and `error`).
//!
//! The line parser is stateful. Codex delivers whole messages and no text deltas, and a tool call
//! is two lines (`item.started`, `item.completed`) with the same item id, so the parser remembers
//! which calls it has announced and which messages the turn has produced.
//!
//! **Mapping** (the normalised events are those of the composer, unchanged):
//!
//! | Codex line | Events |
//! |---|---|
//! | `thread.started` | [`Event::Init`] with the thread id as session id; the model, the tool count and the MCP servers are not reported, so they stay empty. |
//! | `item.completed` `agent_message` | [`Event::Text`]; a second and later message of the turn starts with a blank line, so that consecutive messages do not run together in the window. |
//! | `item.started` and `item.completed` `mcp_tool_call` of `sonara` | [`Event::ToolUse`] once (at the start, or before the result when no start was seen), then [`Event::ToolResult`]. |
//! | `reasoning`, `todo_list`, `item.updated` | Nothing. The reasoning summary is not an answer, and the plan is the agent's own bookkeeping; the steps that matter are the tool calls. |
//! | `error` item, top-level `error` | [`Event::Stderr`]. Neither is fatal by itself: Codex reports retries (`Reconnecting... 2/5`) and configuration warnings this way. |
//! | `turn.completed` | [`Event::Result`] with `success`, the joined messages of the turn, the token usage and the tools Codex denied. |
//! | `turn.failed` | [`Event::Result`] with `success: false` and the error's message. |
//! | `command_execution`, `file_change`, `web_search`, `collab_tool_call`, an MCP call of another server, and any item type this version does not know | A **boundary violation**: [`Event::Stderr`] starting with `boundary:`, and a failed tool call, so that the window shows it. |
//! | a line that is not JSON | [`Event::Output`] |
//!
//! A run that ends without `turn.completed` or `turn.failed` (a stop, a crash) is not completed
//! here: the controller decides, from the host's stop flag and the exit.
//!
//! Nothing here panics on input: every field is read through `serde_json::Value`, so a missing
//! field, a field of another type or a type of a later Codex release is tolerated.

use std::collections::HashSet;

use serde_json::{Value, json};

use crate::composer::{Event, LineParser};

/// The name of the Sonara MCP server in a Codex run (`mcp_servers.sonara`).
pub const SERVER: &str = "sonara";

/// The longest argument summary or result preview, in characters. The Claude parser cuts at the
/// same length (`composer::stream`); its helper is private to that module.
const PREVIEW_CHARS: usize = 240;

/// What separates two messages of one turn.
const MESSAGE_BREAK: &str = "\n\n";

/// Parts of the error text of a tool call that Codex's approval policy refused, in lower case.
/// The source says `MCP tool call requires approval, but approval policy is never`
/// (`mcp_tool_call.rs`, rust-v0.166.0) and `user rejected MCP tool call`; a substring match
/// survives a wrapper around the sentence.
const DENIAL_MARKERS: [&str; 3] = ["requires approval", "approval policy", "rejected"];

/// The parser of `codex exec --json` output; one per run.
#[derive(Debug, Default)]
pub struct CodexParser {
    thread_id: Option<String>,
    /// The non-empty agent messages of this turn, in order.
    messages: Vec<String>,
    /// The ids of the items whose [`Event::ToolUse`] was sent.
    announced: HashSet<String>,
    /// The ids of the items whose [`Event::ToolResult`] was sent.
    resolved: HashSet<String>,
    /// The tools of the calls that the approval policy denied, once each.
    denied: Vec<String>,
}

impl CodexParser {
    /// A parser for a new run.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl LineParser for CodexParser {
    fn parse(&mut self, line: &str) -> Vec<Event> {
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
            Some("thread.started") => self.thread_started(&value),
            Some("item.started") => self.item_started(&value["item"]),
            Some("item.completed") => self.item_completed(&value["item"]),
            Some("turn.completed") => self.turn_completed(&value),
            Some("turn.failed") => self.turn_failed(&value),
            Some("error") => stderr(string(&value["message"]).unwrap_or_default()),
            // `turn.started`, `item.updated` (only the plan changes) and what a later Codex adds.
            _ => Vec::new(),
        }
    }
}

impl CodexParser {
    fn thread_started(&mut self, value: &Value) -> Vec<Event> {
        self.thread_id = string(&value["thread_id"]);
        vec![Event::Init {
            session_id: self.thread_id.clone(),
            model: None,
            tool_count: 0,
            mcp_servers: Vec::new(),
            mcp_errors: Vec::new(),
        }]
    }

    fn item_started(&mut self, item: &Value) -> Vec<Event> {
        match classify(item) {
            Kind::Message | Kind::Reasoning | Kind::Plan | Kind::Warning => Vec::new(),
            Kind::Sonara | Kind::Violation(_) => self.announce(item),
        }
    }

    fn item_completed(&mut self, item: &Value) -> Vec<Event> {
        match classify(item) {
            Kind::Message => self.message(item),
            Kind::Reasoning | Kind::Plan => Vec::new(),
            Kind::Warning => stderr(string(&item["message"]).unwrap_or_default()),
            Kind::Sonara => {
                let mut events = self.announce(item);
                events.extend(self.sonara_result(item));
                events
            }
            Kind::Violation(_) => {
                let mut events = self.announce(item);
                events.extend(self.violation_result(item));
                events
            }
        }
    }

    fn message(&mut self, item: &Value) -> Vec<Event> {
        let text = item["text"].as_str().unwrap_or_default();
        if text.trim().is_empty() {
            return Vec::new();
        }
        let shown = if self.messages.is_empty() {
            text.to_owned()
        } else {
            format!("{MESSAGE_BREAK}{text}")
        };
        self.messages.push(text.to_owned());
        vec![Event::Text { text: shown }]
    }

    /// The [`Event::ToolUse`] of `item` (and the boundary note of a violation), unless it was
    /// sent already.
    fn announce(&mut self, item: &Value) -> Vec<Event> {
        let id = item_id(item);
        if !self.announced.insert(id.clone()) {
            return Vec::new();
        }
        let (tool, summary) = describe(item);
        let mut events = Vec::new();
        if let Kind::Violation(what) = classify(item) {
            events.push(Event::Stderr {
                line: format!(
                    "boundary: Codex tried to use {what}: {} (status {})",
                    shorten(&summary),
                    string(&item["status"]).as_deref().unwrap_or("unknown")
                ),
            });
        }
        events.push(Event::ToolUse {
            id,
            tool,
            summary: shorten(&summary),
            nested: false,
        });
        events
    }

    fn sonara_result(&mut self, item: &Value) -> Vec<Event> {
        let id = item_id(item);
        if !self.resolved.insert(id.clone()) {
            return Vec::new();
        }
        let status_ok = item["status"].as_str() == Some("completed");
        let error = item["error"]["message"].as_str();
        let text = error.map_or_else(|| result_text(&item["result"]), str::to_owned);
        let failed_envelope = envelopes(&item["result"]).any(|envelope| envelope["ok"] == false);
        let ok = status_ok && error.is_none() && !failed_envelope;
        if let Some(message) = error
            && DENIAL_MARKERS
                .iter()
                .any(|marker| message.to_lowercase().contains(marker))
        {
            let tool = string(&item["tool"]).unwrap_or_default();
            if !self.denied.contains(&tool) {
                self.denied.push(tool);
            }
        }
        vec![Event::ToolResult {
            id,
            ok,
            preview: shorten(&text),
        }]
    }

    /// A violation never counts as a success, whatever its status says.
    fn violation_result(&mut self, item: &Value) -> Vec<Event> {
        let id = item_id(item);
        if !self.resolved.insert(id.clone()) {
            return Vec::new();
        }
        let preview = match item["status"].as_str() {
            Some(status) => format!("not allowed in the chat (status {status})"),
            None => "not allowed in the chat".to_owned(),
        };
        vec![Event::ToolResult {
            id,
            ok: false,
            preview,
        }]
    }

    fn turn_completed(&mut self, value: &Value) -> Vec<Event> {
        let event = Event::Result {
            success: true,
            subtype: "success".to_owned(),
            text: self.messages.join(MESSAGE_BREAK),
            duration_ms: None,
            num_turns: None,
            cost_usd: None,
            session_id: self.thread_id.clone(),
            usage: value["usage"].clone(),
            permission_denials: std::mem::take(&mut self.denied),
            raw: pretty(value),
        };
        self.end_turn();
        vec![event]
    }

    fn turn_failed(&mut self, value: &Value) -> Vec<Event> {
        let message = string(&value["error"]["message"])
            .filter(|message| !message.trim().is_empty())
            .unwrap_or_else(|| "turn failed".to_owned());
        let event = Event::Result {
            success: false,
            subtype: "error".to_owned(),
            text: message,
            duration_ms: None,
            num_turns: None,
            cost_usd: None,
            session_id: self.thread_id.clone(),
            usage: json!({}),
            permission_denials: std::mem::take(&mut self.denied),
            raw: pretty(value),
        };
        self.end_turn();
        vec![event]
    }

    fn end_turn(&mut self) {
        self.messages.clear();
        self.announced.clear();
        self.resolved.clear();
    }
}

/// What an item is, for the parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// `agent_message`.
    Message,
    /// `reasoning`.
    Reasoning,
    /// `todo_list`.
    Plan,
    /// An `error` item: a warning, not a failure.
    Warning,
    /// An `mcp_tool_call` of the `sonara` server.
    Sonara,
    /// Anything the chat does not allow; the text names it for the boundary note.
    Violation(&'static str),
}

fn classify(item: &Value) -> Kind {
    match item["type"].as_str() {
        Some("agent_message") => Kind::Message,
        Some("reasoning") => Kind::Reasoning,
        Some("todo_list") => Kind::Plan,
        Some("error") => Kind::Warning,
        Some("mcp_tool_call") if item["server"].as_str() == Some(SERVER) => Kind::Sonara,
        Some("mcp_tool_call") => Kind::Violation("a tool of another MCP server"),
        Some("command_execution") => Kind::Violation("a shell command"),
        Some("file_change") => Kind::Violation("a file change"),
        Some("web_search") => Kind::Violation("a web search"),
        Some("collab_tool_call") => Kind::Violation("a sub-agent"),
        _ => Kind::Violation("an item of an unknown type"),
    }
}

/// The tool name and the argument summary of a tool call item, or of a violation.
fn describe(item: &Value) -> (String, String) {
    let text = |key: &str| string(&item[key]).unwrap_or_default();
    match item["type"].as_str() {
        Some("mcp_tool_call") => {
            let tool = text("tool");
            let summary = item["arguments"].to_string();
            if item["server"].as_str() == Some(SERVER) {
                (tool, summary)
            } else {
                (format!("{}.{tool}", text("server")), summary)
            }
        }
        Some("command_execution") => ("command_execution".to_owned(), text("command")),
        Some("file_change") => (
            "file_change".to_owned(),
            item["changes"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|change| {
                    format!(
                        "{} {}",
                        change["kind"].as_str().unwrap_or("change"),
                        change["path"].as_str().unwrap_or("?")
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
        ),
        Some("web_search") => ("web_search".to_owned(), text("query")),
        Some("collab_tool_call") => ("collab_tool_call".to_owned(), text("tool")),
        other => (other.unwrap_or("unknown").to_owned(), item.to_string()),
    }
}

fn item_id(item: &Value) -> String {
    string(&item["id"]).unwrap_or_default()
}

/// The text blocks of an MCP result, one per line; the structured content when there are none.
fn result_text(result: &Value) -> String {
    let blocks: Vec<&str> = result["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|block| block["text"].as_str())
        .collect();
    if !blocks.is_empty() {
        return blocks.join("\n");
    }
    match &result["structured_content"] {
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// The Sonara envelopes in an MCP result: the structured content and every text block that is
/// itself JSON.
fn envelopes(result: &Value) -> impl Iterator<Item = Value> + '_ {
    let blocks = result["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|block| block["text"].as_str())
        .filter_map(|text| serde_json::from_str::<Value>(text).ok());
    std::iter::once(result["structured_content"].clone()).chain(blocks)
}

fn stderr(line: String) -> Vec<Event> {
    if line.trim().is_empty() {
        Vec::new()
    } else {
        vec![Event::Stderr { line }]
    }
}

fn string(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
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

    fn parse_all(lines: &[&str]) -> Vec<Event> {
        let mut parser = CodexParser::new();
        lines.iter().flat_map(|line| parser.parse(line)).collect()
    }

    fn tool_call(id: &str, status: &str, extra: &str) -> String {
        format!(
            r#"{{"type":"item.completed","item":{{"id":"{id}","type":"mcp_tool_call","server":"sonara","tool":"set_tempo","arguments":{{"bpm":90}},"status":"{status}",{extra}}}}}"#
        )
    }

    #[test]
    fn blank_and_unknown_lines_make_no_events_and_text_lines_become_output() {
        assert_eq!(parse_all(&["", "   "]), []);
        assert_eq!(parse_all(&[r#"{"type":"turn.started"}"#]), []);
        assert_eq!(parse_all(&[r#"{"type":"future.thing","x":1}"#]), []);
        assert_eq!(
            parse_all(&[r#"{"no_type":true}"#, "42", "[1,2]", "null"]),
            []
        );
        assert_eq!(
            parse_all(&["Error: not logged in"]),
            [Event::Output {
                line: "Error: not logged in".into()
            }]
        );
    }

    #[test]
    fn a_call_seen_only_when_completed_gets_its_use_before_its_result() {
        let line = tool_call(
            "item_1",
            "completed",
            r#""result":{"content":[{"type":"text","text":"{\"ok\":true}"}],"structured_content":null},"error":null"#,
        );
        let events = parse_all(&[&line]);
        assert_eq!(
            events,
            [
                Event::ToolUse {
                    id: "item_1".into(),
                    tool: "set_tempo".into(),
                    summary: r#"{"bpm":90}"#.into(),
                    nested: false
                },
                Event::ToolResult {
                    id: "item_1".into(),
                    ok: true,
                    preview: r#"{"ok":true}"#.into()
                },
            ]
        );
    }

    #[test]
    fn a_call_is_announced_once_even_when_the_lines_repeat() {
        let started = r#"{"type":"item.started","item":{"id":"i","type":"mcp_tool_call","server":"sonara","tool":"undo","arguments":{},"status":"in_progress"}}"#;
        let done = tool_call("i", "completed", r#""result":null,"error":null"#);
        let events = parse_all(&[started, started, &done, &done]);
        let uses = events
            .iter()
            .filter(|e| matches!(e, Event::ToolUse { .. }))
            .count();
        let results = events
            .iter()
            .filter(|e| matches!(e, Event::ToolResult { .. }))
            .count();
        assert_eq!((uses, results), (1, 1), "{events:?}");
    }

    #[test]
    fn a_failed_envelope_in_a_completed_call_is_a_failed_result() {
        // Whichever way the envelope arrives: as a text block or as structured content.
        let text = tool_call(
            "a",
            "completed",
            r#""result":{"content":[{"type":"text","text":"{\"ok\": false, \"error\": {\"code\": \"NOT_FOUND\"}}"}],"structured_content":null},"error":null"#,
        );
        let structured = tool_call(
            "b",
            "completed",
            r#""result":{"content":[],"structured_content":{"ok":false}},"error":null"#,
        );
        let events = parse_all(&[&text, &structured]);
        let oks: Vec<bool> = events
            .iter()
            .filter_map(|e| match e {
                Event::ToolResult { ok, .. } => Some(*ok),
                _ => None,
            })
            .collect();
        assert_eq!(oks, [false, false]);
    }

    #[test]
    fn a_long_result_is_cut_on_a_character_boundary() {
        let long = "á".repeat(PREVIEW_CHARS + 50);
        let line = tool_call(
            "x",
            "completed",
            &format!(
                r#""result":{{"content":[{{"type":"text","text":"{long}"}}],"structured_content":null}},"error":null"#
            ),
        );
        let events = parse_all(&[&line]);
        let Some(Event::ToolResult { preview, .. }) = events.last() else {
            panic!("a result");
        };
        assert_eq!(preview.chars().count(), PREVIEW_CHARS + 1);
        assert!(preview.ends_with('…'));
    }

    #[test]
    fn denials_are_matched_by_the_sentence_of_the_approval_policy() {
        for message in [
            "MCP tool call requires approval, but approval policy is never",
            "tool call failed: MCP tool call requires approval, but approval policy is never (code 1)",
            "user rejected MCP tool call",
        ] {
            let line = tool_call(
                "d",
                "failed",
                &format!(r#""result":null,"error":{{"message":"{message}"}}"#),
            );
            let events = parse_all(&[
                &line,
                r#"{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":2}}"#,
            ]);
            let Some(Event::Result {
                permission_denials, ..
            }) = events.last()
            else {
                panic!("a result");
            };
            assert_eq!(permission_denials, &["set_tempo".to_owned()], "{message}");
        }
        // A tool that failed for another reason is not a denial.
        let line = tool_call(
            "e",
            "failed",
            r#""result":null,"error":{"message":"transport closed"}"#,
        );
        let events = parse_all(&[&line, r#"{"type":"turn.completed","usage":{}}"#]);
        let Some(Event::Result {
            permission_denials, ..
        }) = events.last()
        else {
            panic!("a result");
        };
        assert_eq!(permission_denials.as_slice(), [] as [String; 0]);
    }

    #[test]
    fn empty_messages_and_blank_errors_make_no_events() {
        let events = parse_all(&[
            r#"{"type":"item.completed","item":{"id":"m","type":"agent_message","text":"  \n"}}"#,
            r#"{"type":"error","message":""}"#,
            r#"{"type":"item.completed","item":{"id":"e","type":"error","message":" "}}"#,
        ]);
        assert_eq!(events, []);
    }

    #[test]
    fn a_failed_turn_without_a_message_says_so() {
        let events = parse_all(&[r#"{"type":"turn.failed","error":{}}"#]);
        assert!(matches!(
            events.as_slice(),
            [Event::Result { success: false, subtype, text, .. }]
                if subtype == "error" && text == "turn failed"
        ));
    }

    #[test]
    fn the_state_of_a_turn_does_not_leak_into_the_next() {
        let mut parser = CodexParser::new();
        let message = |text: &str| {
            format!(
                r#"{{"type":"item.completed","item":{{"id":"m","type":"agent_message","text":"{text}"}}}}"#
            )
        };
        parser.parse(&message("first"));
        parser.parse(r#"{"type":"turn.completed","usage":{}}"#);
        let events = parser.parse(&message("second"));
        assert_eq!(
            events,
            [Event::Text {
                text: "second".into()
            }],
            "no leading break in a new turn"
        );
    }
}
