//! The Codex adapter end to end, against a FAKE `codex`: a shell script that answers
//! `--version` and `login status` and, for `exec`, prints the reconstructed transcripts of
//! `tests/fixtures/codex-exec-reconstructed-0.166.0-*.jsonl` (see `chat_codex_stream.rs` for what
//! those are). The provider under test is the real [`CodexProvider`], pointed at the script
//! through [`CodexProvider::at`], so detection, the command line, the parser and the stop policy
//! are the ones the chat uses; only the model is missing. A real `codex` is in
//! `chat_codex_binary.rs` (config loading) and `apps/sonara/tests/chat_live.rs` (a real turn).
//!
//! The fake acts on the first word of the last line of its stdin, which is the user's message:
//! `chat`, `tools`, `boundary` and `failed` print that fixture (`failed` exits 1); `garbage`
//! prints lines that are not JSON; `mcpfail` prints the stderr of a required MCP server that did
//! not start and exits 1; `slow` interrupts like Codex (exit 1 and no `turn.*` event) on the first
//! `SIGINT`; `stubborn` ignores `SIGINT` and `SIGTERM`; anything else echoes its whole stdin in
//! an `agent_message`. Every run keeps its stdin, arguments and working directory in the
//! output folder.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use sonara_app::chat::providers::codex::STOP_POLICY;
use sonara_app::chat::{
    Availability, ChatConfig, ChatController, ChatEvent, ChatState, CodexProvider, Provider,
    QueueMode, TaskId, TaskState, Turn,
};
use sonara_app::composer::{Event, Run, RunError, Workspace};

const SCRIPT: &str = r#"#!/bin/sh
OUT='@OUT@'
FIX='@FIX@'
case "$1" in
  --version) echo "codex-cli 0.166.0"; exit 0 ;;
  login) echo "Logged in using ChatGPT" >&2; exit 0 ;;
  exec) ;;
  *) echo "unexpected: $*" >&2; exit 9 ;;
esac
n=$(( $(cat "$OUT/count" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$OUT/count"
cat > "$OUT/stdin-$n.txt"
printf '%s\0' "$@" > "$OUT/argv-$n.bin"
pwd -P > "$OUT/cwd-$n.txt"
word=$(tail -n 1 "$OUT/stdin-$n.txt")
word=${word%% *}
case "$word" in
  chat|tools|boundary) cat "$FIX-$word.jsonl"; exit 0 ;;
  failed) cat "$FIX-failed.jsonl"; exit 1 ;;
  garbage)
    echo 'this is not json'
    echo '{"type":'
    echo '{"type":"thread.started","thread_id":"g"}'
    echo '{"type":"turn.completed","usage":{}}'
    exit 0 ;;
  mcpfail)
    echo '2026-10-29T23:33:57Z ERROR codex_core::session: Failed to create session: required MCP servers failed to initialize: sonara: handshaking with MCP server failed' >&2
    echo 'Error: thread/start: thread/start failed (code -32603)' >&2
    exit 1 ;;
  slow)
    trap 'exit 1' INT
    echo '{"type":"thread.started","thread_id":"s"}'
    echo '{"type":"turn.started"}'
    echo '{"type":"item.started","item":{"id":"item_0","type":"mcp_tool_call","server":"sonara","tool":"get_project_state","arguments":{},"result":null,"error":null,"status":"in_progress"}}'
    while true; do sleep 0.05; done ;;
  stubborn)
    trap '' INT TERM
    echo '{"type":"thread.started","thread_id":"s"}'
    while true; do sleep 0.05; done ;;
esac
esc=$(sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' "$OUT/stdin-$n.txt" | awk 'BEGIN{ORS=""} NR>1{print "\\n"} {print}')
echo '{"type":"thread.started","thread_id":"echo"}'
echo '{"type":"turn.started"}'
printf '{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"%s"}}\n' "$esc"
echo '{"type":"turn.completed","usage":{"input_tokens":7,"cached_input_tokens":0,"output_tokens":3,"reasoning_output_tokens":0}}'
"#;

struct Harness {
    dir: tempfile::TempDir,
    script: PathBuf,
    out: PathBuf,
}

fn harness() -> Harness {
    let dir = tempfile::tempdir().expect("a directory");
    let out = dir.path().join("out");
    fs::create_dir(&out).expect("the output folder");
    let fixtures = format!(
        "{}/tests/fixtures/codex-exec-reconstructed-0.166.0",
        env!("CARGO_MANIFEST_DIR")
    );
    let script = dir.path().join("codex");
    let text = SCRIPT
        .replace("@OUT@", &out.display().to_string())
        .replace("@FIX@", &fixtures);
    fs::write(&script, text).expect("the script");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("executable");
    fs::write(dir.path().join("session-1.json"), "{}").expect("a discovery file");
    Harness { dir, script, out }
}

impl Harness {
    fn provider(&self) -> CodexProvider {
        CodexProvider::at(&self.script)
    }

    fn discovery(&self) -> PathBuf {
        self.dir.path().join("session-1.json")
    }

    fn file(&self, name: &str) -> String {
        fs::read_to_string(self.out.join(name)).unwrap_or_else(|error| panic!("{name}: {error}"))
    }

    /// Starts one run of `prompt` through the provider: the real detection, command line,
    /// parser and stop policy.
    fn start(
        &self,
        prompt: &str,
    ) -> (
        Result<Run, RunError>,
        mpsc::Receiver<Event>,
        sonara_app::composer::Invocation,
        PathBuf,
    ) {
        let provider = self.provider();
        let status = provider.detect();
        let located = status.located.expect("the fake codex is ready");
        let workspace = Workspace::create(&self.discovery()).expect("a workspace");
        let cwd = workspace.cwd();
        let turn = Turn {
            prompt: prompt.to_owned(),
            transcript: String::new(),
        };
        let invocation = provider.build(&located, Path::new("/App/sonara"), &workspace, &turn);
        let (tx, rx) = mpsc::channel();
        let run = Run::start_with(
            &invocation,
            workspace,
            provider.parser(),
            provider.stop_policy(),
            move |event| {
                tx.send(event).ok();
            },
        );
        (run, rx, invocation, cwd)
    }
}

fn until_exit(rx: &mpsc::Receiver<Event>) -> Vec<Event> {
    let mut events = Vec::new();
    loop {
        let event = rx.recv_timeout(Duration::from_secs(30)).expect("an event");
        let exited = matches!(event, Event::Exited { .. });
        events.push(event);
        if exited {
            return events;
        }
    }
}

fn wait_for_init(rx: &mpsc::Receiver<Event>) {
    loop {
        let event = rx.recv_timeout(Duration::from_secs(30)).expect("an event");
        if matches!(event, Event::Init { .. }) {
            return;
        }
    }
}

// ---- the run ----------------------------------------------------------------------------

#[test]
fn the_fake_is_ready_and_the_provider_runs_it_with_the_chat_command_line() {
    let h = harness();
    let status = h.provider().detect();
    assert_eq!(status.availability, Availability::Ready, "{}", status.hint);
    assert_eq!(status.version.as_deref(), Some("0.166.0"));
    assert!(status.beta);

    let (run, rx, invocation, cwd) = h.start("chat what is dorian?");
    let _run = run.expect("the run starts");
    let events = until_exit(&rx);

    // What the process was given: the arguments as separate elements, the prompt on stdin, the
    // empty workspace as its working directory.
    let argv = fs::read(h.out.join("argv-1.bin")).expect("argv");
    let seen: Vec<&[u8]> = argv.split(|byte| *byte == 0).collect();
    // The recording ends with a NUL, so the split leaves one empty tail.
    assert_eq!(seen.last().map(|tail| tail.len()), Some(0));
    let seen: Vec<String> = seen[..seen.len() - 1]
        .iter()
        .map(|arg| String::from_utf8(arg.to_vec()).expect("UTF-8"))
        .collect();
    let sent: Vec<String> = invocation
        .args
        .iter()
        .map(|arg| arg.to_str().expect("UTF-8").to_owned())
        .collect();
    assert_eq!(seen, sent, "every argument arrived as one element");
    assert_eq!(seen.first().map(String::as_str), Some("exec"));
    assert_eq!(seen.last().map(String::as_str), Some("-"));
    assert_eq!(h.file("stdin-1.txt"), "chat what is dorian?");
    assert_eq!(
        h.file("cwd-1.txt").trim(),
        fs::canonicalize(&cwd).unwrap_or(cwd).display().to_string(),
        "it ran in the empty workspace"
    );

    // The events: the normalised stream of the fixture, then the exit.
    let kinds: Vec<&str> = events
        .iter()
        .map(|event| match event {
            Event::Init { .. } => "init",
            Event::Text { .. } => "text",
            Event::Result { .. } => "result",
            Event::Exited { .. } => "exited",
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(kinds, ["init", "text", "text", "result", "exited"]);
    assert_eq!(
        events.last(),
        Some(&Event::Exited {
            code: Some(0),
            stopped: false
        })
    );
}

#[test]
fn several_sonara_calls_arrive_as_uses_and_results_with_the_denial_reported() {
    let h = harness();
    let (run, rx, _, _) = h.start("tools please");
    let _run = run.expect("starts");
    let events = until_exit(&rx);
    let calls: Vec<(String, bool)> = events
        .iter()
        .filter_map(|event| match event {
            Event::ToolResult { id, ok, .. } => Some((id.clone(), *ok)),
            _ => None,
        })
        .collect();
    assert_eq!(
        calls,
        [
            ("item_2".to_owned(), true),
            ("item_3".to_owned(), false),
            ("item_4".to_owned(), false),
            ("item_5".to_owned(), true),
        ]
    );
    let uses = events
        .iter()
        .filter(|event| matches!(event, Event::ToolUse { .. }))
        .count();
    assert_eq!(uses, 4);
    let Some(Event::Result {
        success,
        permission_denials,
        ..
    }) = events
        .iter()
        .find(|event| matches!(event, Event::Result { .. }))
    else {
        panic!("a result");
    };
    assert!(*success);
    assert_eq!(permission_denials, &["delete_track".to_owned()]);
}

#[test]
fn a_boundary_violation_in_the_stream_is_shown_with_the_run_still_ending() {
    let h = harness();
    let (run, rx, _, _) = h.start("boundary now");
    let _run = run.expect("starts");
    let events = until_exit(&rx);
    let notes: Vec<&String> = events
        .iter()
        .filter_map(|event| match event {
            Event::Stderr { line } if line.starts_with("boundary:") => Some(line),
            _ => None,
        })
        .collect();
    assert_eq!(notes.len(), 3, "{events:?}");
    let failed_tools = events
        .iter()
        .filter(|event| matches!(event, Event::ToolResult { ok: false, .. }))
        .count();
    assert_eq!(failed_tools, 3);
    assert!(matches!(
        events.last(),
        Some(Event::Exited { code: Some(0), .. })
    ));
}

#[test]
fn a_stop_interrupts_like_codex_does_with_no_result_and_exit_one() {
    let h = harness();
    let (run, rx, _, _) = h.start("slow task");
    let run = run.expect("starts");
    wait_for_init(&rx);
    thread::sleep(Duration::from_millis(300));
    let started = Instant::now();
    run.stop();
    let events = until_exit(&rx);
    assert!(
        started.elapsed() < STOP_POLICY.interrupt_grace,
        "the first SIGINT was enough: {:?}",
        started.elapsed()
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Result { .. })),
        "an interrupted Codex turn reports no result: {events:?}"
    );
    assert_eq!(
        events.last(),
        Some(&Event::Exited {
            code: Some(1),
            stopped: true
        }),
        "exit 1, and the host's flag says why"
    );
}

#[test]
fn a_run_that_ignores_sigint_and_sigterm_is_killed_after_the_policys_graces() {
    assert_eq!(STOP_POLICY.interrupt_grace, Duration::from_secs(4));
    assert_eq!(STOP_POLICY.terminate_grace, Duration::from_secs(2));
    let h = harness();
    let (run, rx, _, _) = h.start("stubborn task");
    let run = run.expect("starts");
    wait_for_init(&rx);
    thread::sleep(Duration::from_millis(300));
    let started = Instant::now();
    run.stop();
    let events = until_exit(&rx);
    let took = started.elapsed();
    assert_eq!(
        events.last(),
        Some(&Event::Exited {
            code: None,
            stopped: true
        }),
        "ended by SIGKILL, on request"
    );
    let floor = STOP_POLICY.interrupt_grace + STOP_POLICY.terminate_grace;
    assert!(
        took + Duration::from_millis(200) >= floor,
        "both graces were waited: {took:?}"
    );
    assert!(
        took < floor + Duration::from_secs(6),
        "killed on time: {took:?}"
    );
    assert!(run.is_finished());
}

#[test]
fn a_program_that_does_not_exist_is_a_start_error() {
    let h = harness();
    let provider = h.provider();
    let located = provider.detect().located.expect("ready");
    let workspace = Workspace::create(&h.discovery()).expect("a workspace");
    let turn = Turn {
        prompt: "chat".into(),
        transcript: String::new(),
    };
    let mut invocation = provider.build(&located, Path::new("/App/sonara"), &workspace, &turn);
    invocation.program = h.dir.path().join("no-such-codex");
    let error = Run::start_with(
        &invocation,
        workspace,
        provider.parser(),
        provider.stop_policy(),
        drop,
    )
    .expect_err("no process");
    assert!(matches!(error, RunError::Spawn { .. }), "{error}");
}

#[test]
fn output_that_is_not_json_becomes_output_events_and_the_run_still_ends() {
    let h = harness();
    let (run, rx, _, _) = h.start("garbage in");
    let _run = run.expect("starts");
    let events = until_exit(&rx);
    assert!(events.contains(&Event::Output {
        line: "this is not json".into()
    }));
    assert!(events.contains(&Event::Output {
        line: r#"{"type":"#.into()
    }));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Init { .. }))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Result { success: true, .. }))
    );
    assert_eq!(
        events.last(),
        Some(&Event::Exited {
            code: Some(0),
            stopped: false
        })
    );
}

// ---- the chat -----------------------------------------------------------------------------

struct Chat {
    h: Harness,
    controller: ChatController,
    events: Arc<Mutex<Vec<ChatEvent>>>,
}

fn chat() -> Chat {
    let h = harness();
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink_events = Arc::clone(&events);
    let controller = ChatController::new(
        vec![Arc::new(h.provider())],
        ChatConfig {
            sonara_cli: PathBuf::from("/App/sonara"),
            discovery_file: h.discovery(),
        },
        Arc::new(move |event| sink_events.lock().expect("lock").push(event)),
    );
    Chat {
        h,
        controller,
        events,
    }
}

fn wait_until<T>(what: &str, check: impl Fn() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(value) = check() {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(10));
    }
}

impl Chat {
    fn wait_task(&self, id: TaskId, state: TaskState) -> sonara_app::chat::TaskView {
        wait_until(&format!("task {id} to be {state:?}"), || {
            self.controller
                .snapshot()
                .state
                .tasks
                .into_iter()
                .find(|task| task.id == id && task.state == state)
        })
    }

    fn state(&self) -> ChatState {
        self.controller.snapshot().state
    }

    fn run_events(&self, id: TaskId) -> Vec<Event> {
        self.events
            .lock()
            .expect("lock")
            .iter()
            .filter_map(|event| match event {
                ChatEvent::Run { task_id, event, .. } if *task_id == id => Some(event.clone()),
                _ => None,
            })
            .collect()
    }
}

#[test]
fn a_chat_task_through_codex_is_done_with_its_text_tokens_and_session() {
    let c = chat();
    let id = c
        .controller
        .enqueue("chat what is dorian?")
        .expect("queued");
    let task = c.wait_task(id, TaskState::Done);
    let result = task.result.expect("a result");
    assert_eq!(
        result.text,
        "Dorian is a minor mode with a raised sixth degree.\n\nI changed nothing; the answer is above."
    );
    assert_eq!(result.input_tokens, Some(24763));
    assert_eq!(result.output_tokens, Some(122));
    assert_eq!(result.cost_usd, None);
    assert_eq!(
        result.session_id.as_deref(),
        Some("0199a213-81c0-7800-8aa1-bbab2a035a53")
    );
    assert_eq!(c.state().mode, QueueMode::Running);
    // The window got the streamed events of the run, ending with the exit.
    let events = c.run_events(id);
    assert!(matches!(events.first(), Some(Event::Init { .. })));
    assert!(matches!(
        events.last(),
        Some(Event::Exited {
            code: Some(0),
            stopped: false
        })
    ));
}

#[test]
fn a_failed_turn_fails_the_task_with_codexs_message_and_pauses_the_queue() {
    let c = chat();
    let first = c.controller.enqueue("failed now").expect("queued");
    let second = c.controller.enqueue("chat later").expect("queued");
    let task = c.wait_task(first, TaskState::Failed);
    let reason = task.error.expect("a reason");
    assert!(
        reason.contains("unexpected status 401 Unauthorized"),
        "{reason}"
    );
    assert!(reason.contains("Codex CLI"), "{reason}");
    let state = c.state();
    assert_eq!(state.mode, QueueMode::Paused);
    let queued = state
        .tasks
        .iter()
        .find(|task| task.id == second)
        .expect("task");
    assert_eq!(queued.state, TaskState::Queued, "it waits for the user");
    // Resume starts it, and it finishes.
    let _ = c.controller.resume();
    c.wait_task(second, TaskState::Done);
}

#[test]
fn a_required_server_that_did_not_start_fails_with_what_codex_said() {
    let c = chat();
    let id = c.controller.enqueue("mcpfail").expect("queued");
    let task = c.wait_task(id, TaskState::Failed);
    let reason = task.error.expect("a reason");
    assert!(
        reason.contains("required MCP servers failed to initialize"),
        "{reason}"
    );
    assert!(reason.contains("exited with code 1"), "{reason}");
    assert_eq!(c.state().mode, QueueMode::Paused);
}

#[test]
fn stop_cancels_a_codex_task_although_it_exits_one_without_a_result() {
    let c = chat();
    let id = c.controller.enqueue("slow work").expect("queued");
    wait_until("the run to start", || {
        c.run_events(id)
            .iter()
            .any(|event| matches!(event, Event::ToolUse { .. }))
            .then_some(())
    });
    let started = Instant::now();
    let _ = c.controller.stop();
    let task = c.wait_task(id, TaskState::Cancelled);
    assert!(task.error.is_none(), "cancelled is not failed");
    assert!(task.result.is_none());
    assert!(started.elapsed() < STOP_POLICY.interrupt_grace);
    assert_eq!(c.state().mode, QueueMode::Paused);
}

#[test]
fn a_later_task_gets_the_replayed_conversation_on_stdin_and_the_agent_echoes_it() {
    let c = chat();
    let first = c
        .controller
        .enqueue("echo kumquat question")
        .expect("queued");
    let first_task = c.wait_task(first, TaskState::Done);
    // The first message goes as it is, and the fake echoed it back as its answer.
    assert_eq!(c.h.file("stdin-1.txt"), "echo kumquat question");
    assert_eq!(
        first_task.result.expect("a result").text,
        "echo kumquat question"
    );

    let second = c
        .controller
        .enqueue("echo persimmon \"quoted\" question")
        .expect("queued");
    let second_task = c.wait_task(second, TaskState::Done);
    let stdin = c.h.file("stdin-2.txt");
    assert!(
        stdin.starts_with("<earlier_conversation>\nUser: echo kumquat question\n\nAssistant: echo kumquat question\n</earlier_conversation>"),
        "{stdin}"
    );
    assert!(stdin.ends_with("New request from the user:\necho persimmon \"quoted\" question"));
    // The echo of stdin is the answer, with the JSON escaped and unescaped again by the parser.
    assert_eq!(second_task.result.expect("a result").text, stdin);
    // The prompt of neither task is in any argument.
    for n in 1..=2 {
        let argv = fs::read(c.h.out.join(format!("argv-{n}.bin"))).expect("argv");
        let argv = String::from_utf8_lossy(&argv);
        assert!(!argv.contains("kumquat"), "{argv}");
        assert!(!argv.contains("persimmon"), "{argv}");
    }
}
