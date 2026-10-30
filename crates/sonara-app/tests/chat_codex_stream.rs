//! The Codex stream parser against recorded-shape transcripts of `codex exec --json`.
//!
//! **README for the fixtures** (JSONL has no comments, so the note lives here). The files
//! `fixtures/codex-exec-reconstructed-0.166.0-*.jsonl` are RECONSTRUCTED FROM SOURCE, not recorded
//! from a real run: no Codex is signed in on the machine that wrote them, and a real turn spends
//! quota. They follow `codex-rs/exec/src/exec_events.rs` and
//! `event_processor_with_jsonl_output.rs` of openai/codex `rust-v0.166.0`. Ids and numbers are
//! invented, and the text of the denial error is the sentence of `mcp_tool_call.rs`, which has
//! not been seen in a real run. One file per scenario, because the controller keeps the last
//! result of a run:
//!
//! - `-chat`: a turn that only answers, in two messages;
//! - `-tools`: several Sonara calls: one that succeeds, one that fails with a Sonara error
//!   envelope, one that Codex denies (a destructive tool under approval `never`), one that is
//!   seen only when completed; a plan and a warning around them;
//! - `-failed`: a failed turn after a retry notice;
//! - `-boundary`: a shell command, a file change and a web search, which the boundary must have
//!   made impossible, and which must be shown if they appear anyway;
//! - `-unknown-item`: an item type, a top-level event and fields that this version does not know.
//!
//! When a real recording exists, add it next to these under a name without `reconstructed`.
//! These tests feed the lines to the parser one by one and assert the exact event sequences
//! (TST-10); runs through a process are in `chat_codex.rs`.

use serde_json::{Value, json};
use sonara_app::chat::providers::codex::CodexParser;
use sonara_app::composer::{Event, LineParser};

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/tests/fixtures/codex-exec-reconstructed-0.166.0-{name}.jsonl",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn events_of(name: &str) -> Vec<Event> {
    let mut parser = CodexParser::new();
    fixture(name)
        .lines()
        .flat_map(|line| parser.parse(line))
        .collect()
}

fn last_line(name: &str) -> Value {
    let text = fixture(name);
    let line = text.lines().last().expect("a line");
    serde_json::from_str(line).expect("JSON")
}

fn text(text: &str) -> Event {
    Event::Text { text: text.into() }
}

fn stderr(line: &str) -> Event {
    Event::Stderr { line: line.into() }
}

fn init(thread: &str) -> Event {
    Event::Init {
        session_id: Some(thread.into()),
        model: None,
        tool_count: 0,
        mcp_servers: Vec::new(),
        mcp_errors: Vec::new(),
    }
}

fn tool_use(id: &str, tool: &str, summary: &str) -> Event {
    Event::ToolUse {
        id: id.into(),
        tool: tool.into(),
        summary: summary.into(),
        nested: false,
    }
}

fn tool_result(id: &str, ok: bool, preview: &str) -> Event {
    Event::ToolResult {
        id: id.into(),
        ok,
        preview: preview.into(),
    }
}

/// The result of a turn that completed; `raw` is the whole `turn.completed` line, pretty-printed.
fn completed(thread: &str, answer: &str, usage: Value, denials: &[&str], last: &Value) -> Event {
    Event::Result {
        success: true,
        subtype: "success".into(),
        text: answer.into(),
        duration_ms: None,
        num_turns: None,
        cost_usd: None,
        session_id: Some(thread.into()),
        usage,
        permission_denials: denials.iter().map(|name| (*name).to_owned()).collect(),
        raw: serde_json::to_string_pretty(last).expect("a JSON value prints"),
    }
}

#[test]
fn every_fixture_line_is_json() {
    for name in ["chat", "tools", "failed", "boundary", "unknown-item"] {
        for line in fixture(name).lines() {
            assert!(
                serde_json::from_str::<Value>(line).is_ok(),
                "{name}: {line}"
            );
        }
    }
}

#[test]
fn a_chat_turn_is_two_texts_with_a_break_and_one_result() {
    let thread = "0199a213-81c0-7800-8aa1-bbab2a035a53";
    let usage = last_line("chat")["usage"].clone();
    assert_eq!(usage["input_tokens"], 24763, "the oracle is the file");
    let answer = "Dorian is a minor mode with a raised sixth degree.";
    let closing = "I changed nothing; the answer is above.";
    assert_eq!(
        events_of("chat"),
        [
            init(thread),
            text(answer),
            text(&format!("\n\n{closing}")),
            completed(
                thread,
                &format!("{answer}\n\n{closing}"),
                usage,
                &[],
                &last_line("chat")
            ),
        ]
    );
}

#[test]
fn sonara_calls_become_uses_and_results_and_a_denied_one_is_reported() {
    let thread = "0199a213-81c0-7800-8aa1-bbab2a035a54";
    let usage = json!({
        "input_tokens": 30100, "cached_input_tokens": 29000, "cache_write_input_tokens": 0,
        "output_tokens": 210, "reasoning_output_tokens": 0
    });
    let answer = "Tempo is 96 BPM. The track could not be renamed or deleted.";
    assert_eq!(
        events_of("tools"),
        [
            init(thread),
            tool_use("item_2", "set_tempo", r#"{"bpm":96}"#),
            tool_result("item_2", true, r#"{"ok": true, "tool": "set_tempo"}"#),
            tool_use(
                "item_3",
                "rename_track",
                r#"{"track_id":"trk_missing","name":"Drums"}"#
            ),
            tool_result(
                "item_3",
                false,
                r#"{"ok": false, "tool": "rename_track", "error": {"code": "NOT_FOUND", "message": "no such track"}}"#
            ),
            tool_use("item_4", "delete_track", r#"{"track_id":"trk_1"}"#),
            tool_result(
                "item_4",
                false,
                "MCP tool call requires approval, but approval policy is never"
            ),
            tool_use("item_5", "list_tracks", "{}"),
            tool_result(
                "item_5",
                true,
                r#"{"ok": true, "tool": "list_tracks", "tracks": []}"#
            ),
            stderr("model rerouted: gpt-x -> gpt-y (capacity)"),
            text(answer),
            completed(
                thread,
                answer,
                usage,
                &["delete_track"],
                &last_line("tools")
            ),
        ]
    );
}

#[test]
fn a_failed_turn_is_a_failed_result_and_a_retry_notice_is_only_stderr() {
    let thread = "0199a213-81c0-7800-8aa1-bbab2a035a55";
    let events = events_of("failed");
    assert_eq!(events.len(), 3, "{events:?}");
    assert_eq!(events[0], init(thread));
    assert_eq!(
        events[1],
        stderr("Reconnecting... 2/5 (stream disconnected)")
    );
    assert_eq!(
        events[2],
        Event::Result {
            success: false,
            subtype: "error".into(),
            text: "unexpected status 401 Unauthorized".into(),
            duration_ms: None,
            num_turns: None,
            cost_usd: None,
            session_id: Some(thread.into()),
            usage: json!({}),
            permission_denials: Vec::new(),
            raw: serde_json::to_string_pretty(&last_line("failed")).unwrap(),
        }
    );
}

#[test]
fn a_shell_command_a_file_change_and_a_web_search_are_boundary_violations_that_are_shown() {
    let thread = "0199a213-81c0-7800-8aa1-bbab2a035a56";
    let usage = last_line("boundary")["usage"].clone();
    let command = "/bin/zsh -lc 'touch /tmp/x'";
    assert_eq!(
        events_of("boundary"),
        [
            init(thread),
            stderr(&format!(
                "boundary: Codex tried to use a shell command: {command} (status in_progress)"
            )),
            tool_use("item_0", "command_execution", command),
            tool_result("item_0", false, "not allowed in the chat (status declined)"),
            stderr("boundary: Codex tried to use a file change: add /tmp/x.txt (status failed)"),
            tool_use("item_1", "file_change", "add /tmp/x.txt"),
            tool_result("item_1", false, "not allowed in the chat (status failed)"),
            stderr("boundary: Codex tried to use a web search: dorian mode (status unknown)"),
            tool_use("item_2", "web_search", "dorian mode"),
            tool_result("item_2", false, "not allowed in the chat"),
            text("I could not do that."),
            completed(
                thread,
                "I could not do that.",
                usage,
                &[],
                &last_line("boundary")
            ),
        ]
    );
}

#[test]
fn an_unknown_item_is_a_violation_and_unknown_events_and_fields_change_nothing() {
    let thread = "0199a213-81c0-7800-8aa1-bbab2a035a57";
    let item = r#"{"id":"item_0","type":"hologram","brightness":11,"extra":{"nested":true}}"#;
    let usage = last_line("unknown-item")["usage"].clone();
    assert_eq!(usage["another_counter"], 7, "the oracle is the file");
    assert_eq!(
        events_of("unknown-item"),
        [
            init(thread),
            stderr(&format!(
                "boundary: Codex tried to use an item of an unknown type: {item} (status unknown)"
            )),
            tool_use("item_0", "hologram", item),
            tool_result("item_0", false, "not allowed in the chat"),
            text("Done."),
            completed(thread, "Done.", usage, &[], &last_line("unknown-item")),
        ]
    );
}

#[test]
fn the_usage_feeds_the_task_result_like_claudes() {
    use sonara_app::chat::TaskResult;
    let events = events_of("chat");
    let result = TaskResult::from_event(events.last().expect("a result")).expect("a result");
    assert_eq!(result.input_tokens, Some(24763));
    assert_eq!(result.output_tokens, Some(122));
    assert_eq!(result.cost_usd, None);
    assert_eq!(result.num_turns, None);
    assert_eq!(result.duration_ms, None);
    assert_eq!(
        result.session_id.as_deref(),
        Some("0199a213-81c0-7800-8aa1-bbab2a035a53")
    );
}
