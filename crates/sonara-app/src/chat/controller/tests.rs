//! The controller against fake providers: shell scripts that behave like a Claude Code run.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::*;
use crate::chat::task::{QueueMode, TaskState, TaskView};
use crate::composer::{ClaudeParser, Invocation, LineParser, Located, StopPolicy, Workspace};

/// The fake agent. It reads the whole of stdin, notes it in `$OUT/in-<first word of the last
/// line>.txt`, and acts on the first word of the last line (the user's message):
///
/// - `FAIL`: writes to stderr and exits 1 without a result;
/// - `NORESULT`: exits 0 without a result;
/// - `EXIT3`: a successful result, then exits 3;
/// - `DIE130`: exits 130 by itself, without a result;
/// - `SLOW`: waits for `SIGINT`, then exits 130;
/// - `SLOWOK`: waits for `SIGINT`, then reports a success and exits 0;
/// - `SLOWEXIT`: like `SLOW`, but takes 0.5 s to end;
/// - `MCPFAIL`: reports the `sonara` MCP server as failed and waits for `SIGINT`;
/// - anything else: streams two text pieces, takes 0.1 s, and reports `answer to: <message>`.
///
/// Every run appends `start` and `end` lines to `$OUT/log`, so that overlapping runs show.
const SCRIPT: &str = r#"
input=$(cat)
last=$(printf '%s' "$input" | tail -n 1)
word=${last%% *}
printf '%s' "$input" > "$OUT/in-$word.txt"
echo "start $word" >> "$OUT/log"
case "$word" in
  SLOW|SLOWOK|MCPFAIL) trap 'echo "end $word" >> "$OUT/log"; exit 130' INT ;;
  SLOWEXIT) trap 'sleep 0.5; echo "end $word" >> "$OUT/log"; exit 130' INT ;;
esac
if [ "$word" = SLOWOK ]; then
  trap 'echo "end $word" >> "$OUT/log"; printf "%s\n" "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"stopped but fine\"}"; exit 0' INT
fi
if [ "$word" = MCPFAIL ]; then
  echo '{"type":"system","subtype":"init","session_id":"s","model":"m","tools":[],"mcp_servers":[{"name":"sonara","status":"failed"}],"mcp_server_errors":[{"name":"sonara","message":"no session"}]}'
else
  echo '{"type":"system","subtype":"init","session_id":"s","model":"m","tools":[],"mcp_servers":[{"name":"sonara","status":"connected"}]}'
fi
case "$word" in
  FAIL) echo boom >&2; echo "end $word" >> "$OUT/log"; exit 1 ;;
  NORESULT) echo "end $word" >> "$OUT/log"; exit 0 ;;
  DIE130) echo "end $word" >> "$OUT/log"; exit 130 ;;
  SLOW|SLOWOK|SLOWEXIT|MCPFAIL)
    echo '{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"working"}}}'
    while true; do sleep 0.05; done ;;
  EXIT3)
    printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"result":"fine"}'
    echo "end $word" >> "$OUT/log"; exit 3 ;;
esac
echo '{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"Hel"}}}'
echo '{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"lo"}}}'
sleep 0.1
printf '{"type":"result","subtype":"success","is_error":false,"result":"answer to: %s","duration_ms":5,"num_turns":1,"usage":{"input_tokens":3,"output_tokens":4}}\n' "$last"
echo "end $word" >> "$OUT/log"
"#;

/// A provider that runs [`SCRIPT`] through `/bin/sh`.
struct Fake {
    script: PathBuf,
    out: PathBuf,
    ready: bool,
    program: PathBuf,
}

impl Provider for Fake {
    fn id(&self) -> ProviderId {
        ProviderId::Claude
    }

    fn name(&self) -> &'static str {
        "Fake Agent"
    }

    fn detect(&self) -> ProviderStatus {
        let located = self.ready.then(|| Located {
            path: self.program.clone(),
            version: "1.0.0".parse_version(),
        });
        ProviderStatus {
            id: ProviderId::Claude,
            name: "Fake Agent".to_owned(),
            availability: if self.ready {
                Availability::Ready
            } else {
                Availability::NotFound
            },
            version: None,
            path: None,
            hint: "install the fake agent".to_owned(),
            beta: false,
            min_version: "1.0.0".to_owned(),
            located,
        }
    }

    fn build(
        &self,
        located: &Located,
        _sonara_cli: &Path,
        workspace: &Workspace,
        turn: &Turn,
    ) -> Invocation {
        Invocation {
            program: located.path.clone(),
            args: vec![self.script.clone().into_os_string()],
            cwd: workspace.cwd(),
            env: vec![("OUT".into(), self.out.clone().into_os_string())],
            stdin: turn.stdin_text(),
        }
    }

    fn parser(&self) -> Box<dyn LineParser> {
        Box::new(ClaudeParser)
    }

    fn stop_policy(&self) -> StopPolicy {
        StopPolicy {
            interrupt_grace: Duration::from_secs(5),
            terminate_grace: Duration::from_secs(5),
        }
    }
}

trait ParseVersion {
    fn parse_version(&self) -> crate::composer::locate::Version;
}

impl ParseVersion for &str {
    fn parse_version(&self) -> crate::composer::locate::Version {
        crate::composer::locate::Version::parse(self).unwrap()
    }
}

/// What the sink received.
#[derive(Clone, Default)]
struct Recorder {
    events: Arc<Mutex<Vec<ChatEvent>>>,
}

impl Recorder {
    fn sink(&self) -> Sink {
        let events = Arc::clone(&self.events);
        Arc::new(move |event| events.lock().unwrap().push(event))
    }

    fn all(&self) -> Vec<ChatEvent> {
        self.events.lock().unwrap().clone()
    }

    fn states(&self) -> Vec<ChatState> {
        self.all()
            .into_iter()
            .filter_map(|event| match event {
                ChatEvent::Changed { state } => Some(state),
                ChatEvent::Run { .. } => None,
            })
            .collect()
    }

    fn run_events(&self, task: TaskId) -> Vec<Event> {
        self.all()
            .into_iter()
            .filter_map(|event| match event {
                ChatEvent::Run { task_id, event, .. } if task_id == task => Some(event),
                _ => None,
            })
            .collect()
    }
}

struct Fixture {
    _dir: tempfile::TempDir,
    out: PathBuf,
    recorder: Recorder,
    chat: ChatController,
}

fn discovery(dir: &Path) -> PathBuf {
    let file = dir.join("session-1.json");
    fs::write(&file, "{}").unwrap();
    file
}

fn fixture_with(ready: bool, program: Option<&str>) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    fs::create_dir(&out).unwrap();
    let script = dir.path().join("agent.sh");
    fs::write(&script, SCRIPT).unwrap();
    let provider = Fake {
        script,
        out: out.clone(),
        ready,
        program: PathBuf::from(program.unwrap_or("/bin/sh")),
    };
    let recorder = Recorder::default();
    let chat = ChatController::new(
        vec![Arc::new(provider)],
        ChatConfig {
            sonara_cli: PathBuf::from("/App/sonara"),
            discovery_file: discovery(dir.path()),
        },
        recorder.sink(),
    );
    Fixture {
        _dir: dir,
        out,
        recorder,
        chat,
    }
}

fn fixture() -> Fixture {
    fixture_with(true, None)
}

impl Fixture {
    fn log(&self) -> Vec<String> {
        fs::read_to_string(self.out.join("log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn input(&self, word: &str) -> String {
        fs::read_to_string(self.out.join(format!("in-{word}.txt"))).unwrap()
    }

    fn task(&self, id: TaskId) -> TaskView {
        self.chat
            .snapshot()
            .state
            .tasks
            .into_iter()
            .find(|task| task.id == id)
            .unwrap_or_else(|| panic!("no task {id}"))
    }

    fn wait_for(&self, what: &str, condition: impl Fn(&ChatState) -> bool) -> ChatState {
        wait_until(what, || {
            let state = self.chat.snapshot().state;
            condition(&state).then_some(state)
        })
    }

    fn wait_state(&self, id: TaskId, state: TaskState) -> TaskView {
        self.wait_for(&format!("task {id} to be {state:?}"), |s| {
            s.tasks.iter().any(|t| t.id == id && t.state == state)
        });
        self.task(id)
    }

    /// Waits until the run of `id` has reported its `Init` (the script has its trap set).
    fn wait_started(&self, id: TaskId) {
        wait_until(&format!("task {id} to start"), || {
            self.recorder
                .run_events(id)
                .iter()
                .any(|event| matches!(event, Event::Init { .. }))
                .then_some(())
        });
    }

    fn wait_idle(&self) {
        wait_until("the chat to be idle", || {
            (!self.chat.is_busy()).then_some(())
        });
    }
}

/// Polls `check` until it returns a value; the test fails after 30 s.
fn wait_until<T>(what: &str, check: impl Fn() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(value) = check() {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(10));
    }
}

fn prompts(state: &ChatState) -> Vec<&str> {
    state.tasks.iter().map(|t| t.prompt.as_str()).collect()
}

fn states(state: &ChatState) -> Vec<TaskState> {
    state.tasks.iter().map(|t| t.state).collect()
}

// ---- running tasks -----------------------------------------------------------------------

#[test]
fn queued_tasks_run_strictly_one_after_the_other_in_order() {
    let fx = fixture();
    let ids: Vec<TaskId> = ["alpha", "beta", "gamma"]
        .iter()
        .map(|prompt| fx.chat.enqueue(prompt).unwrap())
        .collect();
    let state = fx.wait_for("three done", |s| {
        s.tasks.len() == 3 && s.tasks.iter().all(|t| t.state == TaskState::Done)
    });
    assert_eq!(state.mode, QueueMode::Running);
    assert_eq!(
        fx.log(),
        [
            "start alpha",
            "end alpha",
            "start beta",
            "end beta",
            "start gamma",
            "end gamma"
        ],
        "never two runs at once, and in queue order"
    );
    for (id, word) in ids.iter().zip(["alpha", "beta", "gamma"]) {
        let task = fx.task(*id);
        let result = task.result.unwrap();
        assert_eq!(result.text, format!("answer to: {word}"));
        assert_eq!(result.input_tokens, Some(3));
        assert_eq!(result.output_tokens, Some(4));
        assert_eq!(task.error, None);
    }
    // The window saw the run's events live, in order, with the exit last.
    let events = fx.recorder.run_events(ids[0]);
    assert!(matches!(events.first(), Some(Event::Init { .. })));
    assert_eq!(
        events.last(),
        Some(&Event::Exited {
            code: Some(0),
            stopped: false
        })
    );
    let texts: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            Event::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["Hel", "lo"]);
    // Every state the window got is consistent, and the generation never goes back.
    let mut generation = 0;
    for state in fx.recorder.states() {
        assert!(
            state
                .tasks
                .iter()
                .filter(|t| t.state == TaskState::Running)
                .count()
                <= 1
        );
        assert!(state.generation >= generation);
        generation = state.generation;
    }
    assert!(
        fx.recorder
            .states()
            .iter()
            .any(|s| s.tasks.iter().any(|t| t.state == TaskState::Running)),
        "the window saw a running task"
    );
}

#[test]
fn the_window_can_resync_from_a_snapshot_with_the_coalesced_log() {
    let fx = fixture();
    let id = fx.chat.enqueue("alpha").unwrap();
    fx.wait_state(id, TaskState::Done);
    let snapshot = fx.chat.snapshot();
    assert_eq!(snapshot.logs.len(), 1);
    let log = &snapshot.logs[0];
    assert_eq!(log.task_id, id);
    let texts: Vec<&str> = log
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["Hello"], "adjacent text pieces are one");
    assert!(matches!(log.events.first(), Some(Event::Init { .. })));
    assert!(matches!(log.events.last(), Some(Event::Exited { .. })));
}

#[test]
fn a_task_log_is_bounded_and_coalesces_text() {
    let mut logs = Logs::default();
    let id = TaskId::from_raw(1);
    for _ in 0..3 {
        logs.push(id, &Event::Text { text: "ab".into() });
    }
    assert_eq!(
        logs.views()[0].events,
        [Event::Text {
            text: "ababab".into()
        }]
    );
    for n in 0..(MAX_LOG_EVENTS + 500) {
        logs.push(
            id,
            &Event::Stderr {
                line: n.to_string(),
            },
        );
    }
    let events = &logs.views()[0].events;
    assert_eq!(events.len(), MAX_LOG_EVENTS);
    assert_eq!(
        events.last(),
        Some(&Event::Stderr {
            line: (MAX_LOG_EVENTS + 499).to_string()
        }),
        "the newest events are kept"
    );
    assert_eq!(events.first(), Some(&Event::Stderr { line: "500".into() }));
}

#[test]
fn a_failing_task_pauses_the_queue_and_resume_goes_on() {
    let fx = fixture();
    let failing = fx.chat.enqueue("FAIL now").unwrap();
    let next = fx.chat.enqueue("after").unwrap();
    let task = fx.wait_state(failing, TaskState::Failed);
    let reason = task.error.unwrap();
    assert!(reason.contains("Fake Agent"), "{reason}");
    assert!(reason.contains("without an answer"), "{reason}");
    assert!(reason.contains("code 1"), "{reason}");
    assert!(reason.contains("boom"), "the stderr is quoted: {reason}");
    let state = fx.chat.snapshot().state;
    assert_eq!(state.mode, QueueMode::Paused);
    // The queue stays paused: the second task does not start by itself.
    thread::sleep(Duration::from_millis(300));
    assert_eq!(fx.task(next).state, TaskState::Queued);
    assert_eq!(fx.log(), ["start FAIL", "end FAIL"]);

    let resumed = fx.chat.resume();
    assert_eq!(resumed.mode, QueueMode::Running);
    fx.wait_state(next, TaskState::Done);
    assert_eq!(
        fx.log(),
        ["start FAIL", "end FAIL", "start after", "end after"]
    );
}

#[test]
fn the_outcome_follows_the_result_and_the_exit_never_a_guess() {
    let fx = fixture();
    // A clean result but a bad exit code: failed, with the reason.
    let exit3 = fx.chat.enqueue("EXIT3").unwrap();
    let task = fx.wait_state(exit3, TaskState::Failed);
    assert!(task.error.unwrap().contains("exited with code 3"));
    assert_eq!(
        task.result.unwrap().text,
        "fine",
        "what it did report is kept"
    );
    // A clean exit without any result: failed.
    assert_eq!(fx.chat.resume().mode, QueueMode::Running);
    let none = fx.chat.enqueue("NORESULT").unwrap();
    let task = fx.wait_state(none, TaskState::Failed);
    assert!(task.error.unwrap().contains("without an answer"));
    assert_eq!(task.result, None);
}

#[test]
fn stop_cancels_the_running_task_and_pauses_the_queue() {
    let fx = fixture();
    let slow = fx.chat.enqueue("SLOW one").unwrap();
    let queued = fx.chat.enqueue("later").unwrap();
    fx.wait_started(slow);
    assert_eq!(fx.task(slow).state, TaskState::Running);
    let state = fx.chat.stop();
    assert_eq!(
        state.tasks[0].state,
        TaskState::Running,
        "the call does not wait"
    );
    let task = fx.wait_state(slow, TaskState::Cancelled);
    assert_eq!(task.error, None, "a stop is not a failure");
    let state = fx.chat.snapshot().state;
    assert_eq!(state.mode, QueueMode::Paused);
    assert_eq!(fx.task(queued).state, TaskState::Queued);
    fx.wait_idle();
    assert_eq!(
        fx.recorder.run_events(slow).last(),
        Some(&Event::Exited {
            code: Some(130),
            stopped: true
        })
    );
}

#[test]
fn cancelled_is_decided_by_the_stop_not_by_the_exit_code() {
    let fx = fixture();
    // This run exits 0 with a successful result after the stop: still cancelled.
    let id = fx.chat.enqueue("SLOWOK").unwrap();
    fx.wait_started(id);
    let _ = fx.chat.stop();
    let task = fx.wait_state(id, TaskState::Cancelled);
    assert_eq!(task.result.unwrap().text, "stopped but fine");
    assert_eq!(
        fx.recorder.run_events(id).last(),
        Some(&Event::Exited {
            code: Some(0),
            stopped: true
        })
    );
    // And a run that ends by itself with 130, without any stop, is failed, not cancelled.
    let _ = fx.chat.resume();
    let died = fx.chat.enqueue("DIE130").unwrap();
    let task = fx.wait_state(died, TaskState::Failed);
    assert!(task.error.unwrap().contains("exited with code 130"));
}

#[test]
fn stop_with_nothing_running_does_nothing() {
    let fx = fixture();
    let before = fx.chat.snapshot();
    let state = fx.chat.stop();
    assert_eq!(state, before.state);
    assert!(fx.recorder.all().is_empty(), "no event for a no-op");
    let id = fx.chat.enqueue("alpha").unwrap();
    fx.wait_state(id, TaskState::Done);
    fx.wait_idle();
    let events = fx.recorder.all().len();
    let state = fx.chat.stop();
    assert_eq!(state.mode, QueueMode::Running, "a done task pauses nothing");
    assert_eq!(fx.recorder.all().len(), events);
}

#[test]
fn a_run_that_cannot_reach_the_sonara_tools_is_ended_and_fails_with_that_reason() {
    let fx = fixture();
    let id = fx.chat.enqueue("MCPFAIL").unwrap();
    let task = fx.wait_state(id, TaskState::Failed);
    let reason = task.error.unwrap();
    assert!(reason.contains("Sonara tools"), "{reason}");
    assert!(
        reason.contains("no session"),
        "the cause is quoted: {reason}"
    );
    assert_ne!(
        task.state,
        TaskState::Cancelled,
        "the host's own stop is a failure"
    );
    assert_eq!(fx.chat.snapshot().state.mode, QueueMode::Paused);
    fx.wait_idle();
    assert_eq!(
        fx.recorder.run_events(id).last(),
        Some(&Event::Exited {
            code: Some(130),
            stopped: true
        }),
        "the process was stopped at once"
    );
}

// ---- the queue before it runs -------------------------------------------------------------

#[test]
fn queued_tasks_can_be_edited_removed_and_moved_before_they_start() {
    let fx = fixture();
    let slow = fx.chat.enqueue("SLOW first").unwrap();
    fx.wait_started(slow);
    let a = fx.chat.enqueue("a1").unwrap();
    let b = fx.chat.enqueue("b1").unwrap();
    fx.chat.enqueue("c1").unwrap();
    let d = fx.chat.enqueue("d1").unwrap();
    let state = fx.chat.edit(b, "b2").unwrap();
    assert_eq!(prompts(&state), ["SLOW first", "a1", "b2", "c1", "d1"]);
    let state = fx.chat.remove(a).unwrap();
    assert_eq!(prompts(&state), ["SLOW first", "b2", "c1", "d1"]);
    let state = fx.chat.move_task(d, 0).unwrap();
    assert_eq!(prompts(&state), ["SLOW first", "d1", "b2", "c1"]);
    // The running task cannot be touched, and unknown ids are refused.
    assert_eq!(
        fx.chat.edit(slow, "x").unwrap_err(),
        ChatError::TaskNotQueued(slow)
    );
    assert_eq!(
        fx.chat.remove(slow).unwrap_err(),
        ChatError::TaskNotQueued(slow)
    );
    assert_eq!(
        fx.chat.move_task(slow, 0).unwrap_err(),
        ChatError::TaskNotQueued(slow)
    );
    assert_eq!(
        fx.chat.remove(a).unwrap_err(),
        ChatError::TaskNotFound(a),
        "gone"
    );
    assert_eq!(fx.chat.edit(b, "  ").unwrap_err(), ChatError::PromptEmpty);
    // The window was told about each change.
    let last = fx.recorder.states().pop().unwrap();
    assert_eq!(prompts(&last), ["SLOW first", "d1", "b2", "c1"]);

    let _ = fx.chat.stop();
    fx.wait_state(slow, TaskState::Cancelled);
    let _ = fx.chat.resume();
    fx.wait_for("all done", |s| {
        s.tasks
            .iter()
            .filter(|t| t.state == TaskState::Done)
            .count()
            == 3
    });
    assert_eq!(
        fx.log()
            .iter()
            .filter(|l| l.starts_with("start"))
            .collect::<Vec<_>>(),
        ["start SLOW", "start d1", "start b2", "start c1"],
        "they ran in the edited and moved order"
    );
    assert_eq!(fx.task(b).result.unwrap().text, "answer to: b2");
}

#[test]
fn the_calls_refuse_what_the_state_does_not_allow() {
    let fx = fixture();
    assert_eq!(fx.chat.enqueue("  ").unwrap_err().code(), "PROMPT_EMPTY");
    let long = "x".repeat(16_001);
    assert_eq!(
        fx.chat.enqueue(&long).unwrap_err().code(),
        "PROMPT_TOO_LONG"
    );
    assert_eq!(
        fx.chat.edit(TaskId::from_raw(9), "x").unwrap_err().code(),
        "TASK_NOT_FOUND"
    );
    // Fill the queue behind a paused one: 50 wait, the 51st is refused.
    let failing = fx.chat.enqueue("FAIL x").unwrap();
    fx.wait_state(failing, TaskState::Failed);
    for n in 0..50 {
        fx.chat.enqueue(&format!("wait {n}")).unwrap();
    }
    assert_eq!(
        fx.chat.enqueue("one more").unwrap_err().code(),
        "QUEUE_FULL"
    );
    // The provider is locked once the thread has tasks.
    assert_eq!(
        fx.chat.set_provider(ProviderId::Claude).unwrap_err().code(),
        "PROVIDER_LOCKED"
    );
    let _ = fx.chat.clear();
}

#[test]
fn the_provider_can_only_be_one_that_exists_and_is_ready() {
    let fx = fixture();
    let error = fx.chat.set_provider(ProviderId::Codex).unwrap_err();
    assert_eq!(error.code(), "PROVIDER_UNAVAILABLE");
    assert!(error.to_string().contains("codex"), "{error}");
    assert!(fx.chat.set_provider(ProviderId::Claude).is_ok());
    assert_eq!(fx.chat.providers().len(), 1);
    assert_eq!(fx.chat.providers()[0].availability, Availability::Ready);

    let missing = fixture_with(false, None);
    let error = missing.chat.set_provider(ProviderId::Claude).unwrap_err();
    assert_eq!(error.code(), "PROVIDER_UNAVAILABLE");
    assert!(
        error.to_string().contains("install the fake agent"),
        "{error}"
    );
    assert_eq!(
        missing.chat.providers()[0].availability,
        Availability::NotFound
    );
}

// ---- failing to start ---------------------------------------------------------------------

#[test]
fn a_process_that_cannot_start_fails_the_task_with_the_reason() {
    let fx = fixture_with(true, Some("/nonexistent/agent"));
    let id = fx.chat.enqueue("alpha").unwrap();
    let task = fx.wait_state(id, TaskState::Failed);
    let reason = task.error.unwrap();
    assert!(reason.contains("cannot start"), "{reason}");
    assert_eq!(fx.chat.snapshot().state.mode, QueueMode::Paused);
    fx.wait_idle();
    assert!(fx.log().is_empty(), "nothing ran");
}

#[test]
fn a_provider_that_is_not_ready_fails_the_task_with_its_hint() {
    let fx = fixture_with(false, None);
    let id = fx.chat.enqueue("alpha").unwrap();
    let task = fx.wait_state(id, TaskState::Failed);
    let reason = task.error.unwrap();
    assert!(reason.contains("install the fake agent"), "{reason}");
    assert!(
        fx.chat.enqueue("again").is_ok(),
        "the queue still takes tasks"
    );
    fx.wait_idle();
}

#[test]
fn an_unreadable_discovery_file_fails_the_task() {
    let dir = tempfile::tempdir().unwrap();
    let recorder = Recorder::default();
    let provider = Fake {
        script: dir.path().join("agent.sh"),
        out: dir.path().to_path_buf(),
        ready: true,
        program: PathBuf::from("/bin/sh"),
    };
    let chat = ChatController::new(
        vec![Arc::new(provider)],
        ChatConfig {
            sonara_cli: PathBuf::from("/App/sonara"),
            discovery_file: dir.path().join("not-a-discovery-file.json"),
        },
        recorder.sink(),
    );
    let id = chat.enqueue("alpha").unwrap();
    wait_until("the task to fail", || {
        let state = chat.snapshot().state;
        state
            .tasks
            .iter()
            .any(|t| t.id == id && t.state == TaskState::Failed)
            .then_some(())
    });
    let task = chat.snapshot().state.tasks.remove(0);
    assert!(task.error.unwrap().contains("cannot prepare the run"));
}

// ---- wiping -------------------------------------------------------------------------------

#[test]
fn clear_stops_the_run_and_leaves_an_empty_chat_in_a_new_generation() {
    let fx = fixture();
    let slow = fx.chat.enqueue("SLOW one").unwrap();
    let _ = fx.chat.enqueue("two").unwrap();
    fx.wait_started(slow);
    let before = fx.chat.snapshot().state.generation;
    let state = fx.chat.clear();
    assert_ne!(state.generation, before);
    assert_eq!(state.tasks.len(), 0);
    assert_eq!(state.mode, QueueMode::Running);
    assert!(!state.provider_locked);
    let snapshot = fx.chat.snapshot();
    assert_eq!(snapshot.state.tasks.len() + snapshot.logs.len(), 0);
    fx.wait_idle();
    assert_eq!(
        fx.log(),
        ["start SLOW", "end SLOW"],
        "the second never started"
    );
    let after = fx.chat.snapshot();
    assert_eq!(after.state.tasks.len() + after.logs.len(), 0);
    assert_eq!(after.state.generation, state.generation);
}

#[test]
fn events_of_a_wiped_generation_never_reach_the_window() {
    let fx = fixture();
    let old = fx.chat.enqueue("SLOWEXIT one").unwrap();
    fx.wait_started(old);
    let cleared = fx.chat.clear();
    fx.wait_idle();
    let events = fx.recorder.all();
    // Everything after the wipe's own `Changed` belongs to the new generation.
    let wipe_at = events
        .iter()
        .position(
            |e| matches!(e, ChatEvent::Changed { state } if state.generation == cleared.generation),
        )
        .unwrap();
    for event in &events[wipe_at..] {
        match event {
            ChatEvent::Run {
                generation,
                task_id,
                ..
            } => {
                assert_eq!(*generation, cleared.generation, "{task_id} is stale");
            }
            ChatEvent::Changed { state } => assert_eq!(state.generation, cleared.generation),
        }
    }
    assert!(
        events[wipe_at..]
            .iter()
            .all(|e| !matches!(e, ChatEvent::Run { task_id, .. } if *task_id == old)),
        "no event of the wiped task after the wipe"
    );
    // The stale process' end did not leave a trace in the state.
    assert_eq!(fx.chat.snapshot().state.tasks.len(), 0);
}

#[test]
fn nothing_starts_while_the_process_of_the_wiped_generation_is_still_ending() {
    let fx = fixture();
    let old = fx.chat.enqueue("SLOWEXIT one").unwrap();
    fx.wait_started(old);
    let _ = fx.chat.clear();
    // The old process takes 0.5 s to end. A new task must wait for it.
    let new = fx.chat.enqueue("fresh").unwrap();
    assert_eq!(fx.task(new).state, TaskState::Queued);
    assert!(fx.chat.is_busy(), "the old process is still ending");
    thread::sleep(Duration::from_millis(150));
    assert_eq!(fx.task(new).state, TaskState::Queued, "still waiting");
    assert!(
        !fx.log().iter().any(|line| line == "start fresh"),
        "the new run has not started: {:?}",
        fx.log()
    );
    fx.wait_state(new, TaskState::Done);
    assert_eq!(
        fx.log(),
        ["start SLOWEXIT", "end SLOWEXIT", "start fresh", "end fresh"],
        "the runs never overlapped"
    );
}

#[test]
fn a_replaced_project_wipes_the_chat_like_clear() {
    let fx = fixture();
    let done = fx.chat.enqueue("alpha").unwrap();
    fx.wait_state(done, TaskState::Done);
    let slow = fx.chat.enqueue("SLOW two").unwrap();
    let _ = fx.chat.enqueue("three").unwrap();
    fx.wait_started(slow);
    let before = fx.chat.snapshot().state.generation;
    let state = fx.chat.on_project_replaced();
    assert_ne!(state.generation, before);
    assert_eq!(state.tasks.len(), 0);
    assert_eq!(fx.chat.snapshot().logs.len(), 0);
    fx.wait_idle();
    assert_eq!(
        fx.log(),
        ["start alpha", "end alpha", "start SLOW", "end SLOW"]
    );
    // The new project's chat starts clean: no replay of the old conversation.
    let next = fx.chat.enqueue("fresh").unwrap();
    fx.wait_state(next, TaskState::Done);
    assert!(!fx.input("fresh").contains("earlier_conversation"));
    assert!(!fx.input("fresh").contains("alpha"));
}

#[test]
fn shutdown_ends_the_run_and_closes_the_chat() {
    let fx = fixture();
    let slow = fx.chat.enqueue("SLOW one").unwrap();
    fx.wait_started(slow);
    let sent = fx.recorder.all().len();
    fx.chat.shutdown();
    fx.wait_idle();
    assert_eq!(fx.log(), ["start SLOW", "end SLOW"]);
    assert_eq!(fx.chat.enqueue("more").unwrap_err().code(), "CONFLICT");
    assert_eq!(fx.chat.edit(slow, "x").unwrap_err().code(), "CONFLICT");
    assert_eq!(
        fx.recorder.all().len(),
        sent,
        "nothing is sent after the shutdown"
    );
    assert_eq!(fx.chat.snapshot().state.tasks.len(), 0);
}

// ---- the conversation replay -------------------------------------------------------------

#[test]
fn a_later_task_gets_the_finished_ones_as_context_and_the_first_gets_none() {
    let fx = fixture();
    let first = fx.chat.enqueue("alpha").unwrap();
    fx.wait_state(first, TaskState::Done);
    let failing = fx.chat.enqueue("FAIL x").unwrap();
    fx.wait_state(failing, TaskState::Failed);
    let _ = fx.chat.resume();
    let third = fx.chat.enqueue("gamma").unwrap();
    fx.wait_state(third, TaskState::Done);

    assert_eq!(
        fx.input("alpha"),
        "alpha",
        "the first message goes as it is"
    );
    let input = fx.input("gamma");
    assert!(input.starts_with("<earlier_conversation>\n"), "{input}");
    assert!(
        input.contains("User: alpha\n\nAssistant: answer to: alpha\n"),
        "{input}"
    );
    assert!(
        input.ends_with("</earlier_conversation>\n\nNew request from the user:\ngamma"),
        "{input}"
    );
    assert!(
        !input.contains("FAIL"),
        "a failed task is not replayed: {input}"
    );
}

#[test]
fn the_replay_is_bounded_and_a_long_chat_never_hits_the_prompt_limit() {
    let fx = fixture();
    // Each message and each answer is far longer than the replay allows (but one line: the
    // fake agent reads the last line).
    let mut last = None;
    for n in 0..12 {
        let prompt = format!("task{n:02} {}", "x".repeat(3_000));
        last = Some(fx.chat.enqueue(&prompt).unwrap());
    }
    fx.wait_state(last.unwrap(), TaskState::Done);
    let input = fx.input("task11");
    let block = input
        .strip_prefix("<earlier_conversation>\n")
        .unwrap()
        .split_once("\n</earlier_conversation>")
        .unwrap()
        .0;
    assert!(
        block.chars().count() <= replay::MAX_TOTAL_CHARS,
        "{}",
        block.chars().count()
    );
    assert!(block.contains("task10"), "the newest are kept");
    assert!(!block.contains("task00"), "the oldest are not");
    assert!(block.contains("[truncated]"));
    assert!(block.starts_with("[Earlier messages are left out.]"));
    // The new request itself is whole.
    assert!(input.ends_with(&format!("task11 {}", "x".repeat(3_000))));
}

// ---- concurrency (RT-06's spirit, for the host side) ---------------------------------------

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn check_state(state: &ChatState) {
    let running = state
        .tasks
        .iter()
        .filter(|t| t.state == TaskState::Running)
        .count();
    assert!(running <= 1, "{running} tasks run: {:?}", states(state));
    let first_queued = state
        .tasks
        .iter()
        .position(|t| t.state == TaskState::Queued)
        .unwrap_or(state.tasks.len());
    assert!(
        state.tasks[first_queued..]
            .iter()
            .all(|t| t.state == TaskState::Queued),
        "queued tasks are not a suffix: {:?}",
        states(state)
    );
    assert_eq!(state.provider_locked, !state.tasks.is_empty());
    let mut ids: Vec<_> = state.tasks.iter().map(|t| t.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), state.tasks.len(), "ids repeat");
}

#[test]
fn concurrent_calls_neither_deadlock_nor_break_the_invariants() {
    let (done, finished) = mpsc::channel();
    let guard = thread::spawn(move || {
        let fx = fixture();
        let chat = fx.chat.clone();
        let workers: Vec<_> = (1..=4_u64)
            .map(|worker| {
                let chat = chat.clone();
                thread::spawn(move || {
                    let mut rng = Rng(worker.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
                    for _ in 0..60 {
                        let snapshot = chat.snapshot();
                        check_state(&snapshot.state);
                        let known: Vec<TaskId> =
                            snapshot.state.tasks.iter().map(|t| t.id).collect();
                        let pick = |rng: &mut Rng| {
                            if known.is_empty() {
                                TaskId::from_raw(1)
                            } else {
                                known[usize::try_from(rng.next()).unwrap_or(0) % known.len()]
                            }
                        };
                        match rng.next() % 9 {
                            0..=2 => {
                                let word = ["work", "SLOW", "FAIL", "NORESULT"]
                                    [usize::try_from(rng.next() % 4).unwrap()];
                                let _ = chat.enqueue(&format!("{word} {worker}"));
                            }
                            3 => {
                                let _ = chat.stop();
                            }
                            4 => {
                                let _ = chat.clear();
                            }
                            5 => {
                                let _ = chat.resume();
                            }
                            6 => {
                                let _ = chat.move_task(pick(&mut rng), 0);
                            }
                            7 => {
                                let _ = chat.edit(pick(&mut rng), "edited");
                            }
                            _ => {
                                let _ = chat.remove(pick(&mut rng));
                            }
                        }
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().expect("a worker panicked");
        }
        // After the storm: wipe, let the last process end, and the chat still works.
        let _ = chat.clear();
        fx.wait_idle();
        let _ = chat.resume();
        let id = chat.enqueue("final").unwrap();
        fx.wait_state(id, TaskState::Done);
        fx.wait_idle();

        // What the window was told is consistent: valid states, a generation that never goes
        // back, and run events only of the generation the window was in.
        let mut generation = 0;
        for event in fx.recorder.all() {
            match event {
                ChatEvent::Changed { state } => {
                    check_state(&state);
                    assert!(state.generation >= generation, "the generation went back");
                    generation = state.generation;
                }
                ChatEvent::Run { generation: g, .. } => {
                    assert_eq!(g, generation, "a stale run event reached the window");
                }
            }
        }
        done.send(()).unwrap();
    });
    // A deadlock fails the test instead of hanging it.
    match finished.recv_timeout(Duration::from_secs(120)) {
        Ok(()) => guard.join().unwrap(),
        Err(error) => panic!("the controller deadlocked or died: {error}"),
    }
}

#[test]
fn a_sink_may_call_back_into_the_controller() {
    // The sink runs outside the state lock, so it can read the snapshot.
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    fs::create_dir(&out).unwrap();
    let script = dir.path().join("agent.sh");
    fs::write(&script, SCRIPT).unwrap();
    let provider = Fake {
        script,
        out,
        ready: true,
        program: PathBuf::from("/bin/sh"),
    };
    let slot: Arc<Mutex<Option<ChatController>>> = Arc::default();
    let seen: Arc<Mutex<BTreeMap<u64, usize>>> = Arc::default();
    let sink: Sink = {
        let slot = Arc::clone(&slot);
        let seen = Arc::clone(&seen);
        Arc::new(move |_event| {
            let chat = slot.lock().unwrap().clone();
            if let Some(chat) = chat {
                let snapshot = chat.snapshot();
                *seen
                    .lock()
                    .unwrap()
                    .entry(snapshot.state.generation)
                    .or_default() += 1;
            }
        })
    };
    let chat = ChatController::new(
        vec![Arc::new(provider)],
        ChatConfig {
            sonara_cli: PathBuf::from("/App/sonara"),
            discovery_file: discovery(dir.path()),
        },
        sink,
    );
    *slot.lock().unwrap() = Some(chat.clone());
    let id = chat.enqueue("alpha").unwrap();
    wait_until("done", || {
        chat.snapshot()
            .state
            .tasks
            .iter()
            .any(|t| t.id == id && t.state == TaskState::Done)
            .then_some(())
    });
    assert!(
        !seen.lock().unwrap().is_empty(),
        "the sink ran and called back"
    );
    *slot.lock().unwrap() = None;
}
