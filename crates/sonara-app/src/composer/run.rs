//! Running a headless agent CLI (Claude Code, or another provider's) and streaming what it
//! reports.
//!
//! **Threads.** The process is read by four threads, none of them real-time: one per output pipe,
//! one that writes the prompt, and one that waits for the exit. Their events go through one
//! channel to an emitter thread that owns the caller's sink, so the sink needs no lock and the
//! events arrive in order, with [`Event::Exited`] last.
//!
//! **Stopping.** [`Run::stop`] sends `SIGINT` to Claude Code, which ends the turn and still
//! writes its `result` (a `SIGTERM` would exit with 143 and no result). A run that ignores it is
//! sent `SIGTERM` and then `SIGKILL` as a process group, which also ends the `sonara mcp` child.
//! The escalation runs on a timer thread, which an exiting process never lets finish, so a host
//! that is about to quit uses [`Run::stop_now`], which does the same on the calling thread
//! within a bound.
//!
//! **The end.** [`Event::Exited`] follows the exit of the process and the end of its output,
//! which the readers see when every holder of the pipes has closed them. A helper that left the
//! process group (`setsid`) can keep a pipe open for as long as it lives, so the exit thread
//! waits for the readers at most [`READER_GRACE`]; after that it reports the exit anyway, and
//! the output that arrives later is dropped.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{Sender, unbounded};
use rustix::process::{Pid, Signal, kill_process, kill_process_group};

use super::invocation::{Invocation, Workspace};
use super::stop::StopPolicy;
use super::stream::{ClaudeParser, Event, LineParser};

/// How long the exit thread waits for the readers of the output pipes after the process has been
/// reaped, before it reports the exit without the rest of the output.
pub const READER_GRACE: Duration = Duration::from_secs(2);

/// How often a wait for the process or a reader looks at its flag.
const POLL: Duration = Duration::from_millis(10);

/// How long [`Run::stop_now`] gives `SIGTERM` before it sends `SIGKILL`.
const TERMINATE_GRACE: Duration = Duration::from_millis(300);

/// How long [`Run::stop_now`] waits for the exit after `SIGKILL`.
const KILL_REAP: Duration = Duration::from_secs(1);

/// The gate of the event channel. The readers send through it, and the exit thread closes it
/// as it sends [`Event::Exited`], so that nothing a straggling reader says comes after the exit.
#[derive(Debug, Clone)]
struct Gate {
    sender: Sender<Event>,
    closed: Arc<Mutex<bool>>,
}

impl Gate {
    /// Sends `event` unless the gate is closed; `false` when the event was not delivered.
    fn send(&self, event: Event) -> bool {
        let closed = self.closed.lock().unwrap_or_else(PoisonError::into_inner);
        !*closed && self.sender.send(event).is_ok()
    }

    /// Closes the gate, then sends `event` as the last one.
    fn close_with(&self, event: Event) -> bool {
        let mut closed = self.closed.lock().unwrap_or_else(PoisonError::into_inner);
        *closed = true;
        self.sender.send(event).is_ok()
    }
}

/// A running agent process (Claude Code, or another provider's CLI).
#[derive(Debug)]
pub struct Run {
    pid: u32,
    stop_policy: StopPolicy,
    stop_requested: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
}

/// Why a run could not start.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// The process could not be started.
    #[error("cannot start {program}: {source}")]
    Spawn {
        /// The executable.
        program: String,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// A thread of the run could not be started.
    #[error("cannot start a thread of the run: {0}")]
    Thread(#[source] io::Error),
}

impl Run {
    /// Starts a run of Claude Code: [`Run::start_with`] with Claude's parser and
    /// [`StopPolicy::CLAUDE`].
    ///
    /// # Errors
    ///
    /// [`RunError`] when the process or one of the threads cannot be started.
    pub fn start<F>(
        invocation: &Invocation,
        workspace: Workspace,
        sink: F,
    ) -> Result<Self, RunError>
    where
        F: FnMut(Event) + Send + 'static,
    {
        Self::start_with(
            invocation,
            workspace,
            Box::new(ClaudeParser),
            StopPolicy::CLAUDE,
            sink,
        )
    }

    /// Starts `invocation` and delivers its events to `sink` on a thread of its own, until
    /// [`Event::Exited`]. The lines of its standard output go through `parser`; a stop follows
    /// `stop_policy`. The `workspace` is removed after the process has ended.
    ///
    /// # Errors
    ///
    /// [`RunError`] when the process or one of the threads cannot be started.
    pub fn start_with<F>(
        invocation: &Invocation,
        workspace: Workspace,
        mut parser: Box<dyn LineParser>,
        stop_policy: StopPolicy,
        mut sink: F,
    ) -> Result<Self, RunError>
    where
        F: FnMut(Event) + Send + 'static,
    {
        let mut command = Command::new(&invocation.program);
        command
            .args(&invocation.args)
            .current_dir(&invocation.cwd)
            .envs(invocation.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Its own process group, so that a stop can end `sonara mcp` with it.
            .process_group(0);
        let mut child = command.spawn().map_err(|source| RunError::Spawn {
            program: invocation.program.display().to_string(),
            source,
        })?;
        let pid = child.id();
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            end(&mut child);
            return Err(RunError::Thread(io::Error::other("a pipe is missing")));
        };

        let (sender, receiver) = unbounded::<Event>();
        let events = Gate {
            sender,
            closed: Arc::new(Mutex::new(false)),
        };
        let stop_requested = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));

        let spawned = (|| -> io::Result<()> {
            let prompt = invocation.stdin.clone();
            thread::Builder::new()
                .name("composer-stdin".to_owned())
                .spawn(move || {
                    let mut stdin = stdin;
                    // A process that exited early closes the pipe: nothing more to tell it.
                    if let Err(error) = stdin.write_all(prompt.as_bytes()) {
                        log::debug!("composer: cannot write the prompt: {error}");
                    }
                })?;
            let out = spawn_reader("composer-stdout", stdout, events.clone(), move |line| {
                parser.parse(&line)
            })?;
            let err = spawn_reader("composer-stderr", stderr, events.clone(), |line| {
                let line = line.trim_end().to_owned();
                if line.is_empty() {
                    Vec::new()
                } else {
                    vec![Event::Stderr { line }]
                }
            })?;
            let stopped = Arc::clone(&stop_requested);
            let done = Arc::clone(&finished);
            thread::Builder::new()
                .name("composer-wait".to_owned())
                .spawn(move || {
                    let code = match child.wait() {
                        Ok(status) => status.code(),
                        Err(error) => {
                            log::warn!("composer: cannot wait for the process: {error}");
                            None
                        }
                    };
                    done.store(true, Ordering::Release);
                    // The pipes close with the process; read them to the end first, but not
                    // for longer than a helper outside the process group keeps them open.
                    let deadline = Instant::now() + READER_GRACE;
                    for reader in [out, err] {
                        if !join_before(reader, deadline) {
                            log::warn!(
                                "composer: the output of the process is still open after {READER_GRACE:?}; reporting the exit without it"
                            );
                        }
                    }
                    let stopped = stopped.load(Ordering::Acquire);
                    if !events.close_with(Event::Exited { code, stopped }) {
                        log::debug!("composer: nobody listens for the exit");
                    }
                    drop(workspace);
                })?;
            thread::Builder::new()
                .name("composer-events".to_owned())
                .spawn(move || {
                    for event in receiver {
                        let last = matches!(event, Event::Exited { .. });
                        sink(event);
                        // A reader that outlived the grace still holds the channel open.
                        if last {
                            break;
                        }
                    }
                })?;
            Ok(())
        })();
        if let Err(error) = spawned {
            // The process is already running; without its reader it would block on a full pipe.
            signal_group(pid, Signal::KILL);
            return Err(RunError::Thread(error));
        }
        Ok(Self {
            pid,
            stop_policy,
            stop_requested,
            finished,
        })
    }

    /// Asks the run to stop: `SIGINT` now, then `SIGTERM` and `SIGKILL` if it does not end.
    /// Returns at once; [`Event::Exited`] says when it is over, with `stopped` set.
    pub fn stop(&self) {
        if self.finished.load(Ordering::Acquire) {
            return;
        }
        self.stop_requested.store(true, Ordering::Release);
        signal(self.pid, Signal::INT);
        let pid = self.pid;
        let policy = self.stop_policy;
        let finished = Arc::clone(&self.finished);
        let escalation = thread::Builder::new()
            .name("composer-stop".to_owned())
            .spawn(move || {
                let steps: [(Duration, Signal); 2] = [
                    (policy.interrupt_grace, Signal::TERM),
                    (policy.terminate_grace, Signal::KILL),
                ];
                for (grace, signal) in steps {
                    thread::sleep(grace);
                    if finished.load(Ordering::Acquire) {
                        return;
                    }
                    signal_group(pid, signal);
                }
            });
        if let Err(error) = escalation {
            log::warn!("composer: cannot start the stop timer ({error}); ending the run");
            signal_group(self.pid, Signal::KILL);
        }
    }

    /// Ends the run before this thread returns, for an application that is about to quit (a
    /// timer thread would not outlive it): `SIGINT` at once (unless [`Run::stop`] sent it
    /// already), then, if the process has not ended after `grace`, `SIGTERM` and a moment later
    /// `SIGKILL` to its whole process group. Returns when the process has ended, or about
    /// `grace` plus a second at the most when it has not.
    ///
    /// It waits on the calling thread, so call it where waiting is allowed (never from a
    /// real-time thread, RT-06).
    pub fn stop_now(&self, grace: Duration) {
        if self.finished.load(Ordering::Acquire) {
            return;
        }
        if !self.stop_requested.swap(true, Ordering::AcqRel) {
            signal(self.pid, Signal::INT);
        }
        if self.wait_finished(grace) {
            return;
        }
        signal_group(self.pid, Signal::TERM);
        if self.wait_finished(TERMINATE_GRACE) {
            return;
        }
        signal_group(self.pid, Signal::KILL);
        if !self.wait_finished(KILL_REAP) {
            log::warn!("composer: the process did not end after SIGKILL");
        }
    }

    /// Waits until the process has ended or `limit` has passed; whether it has ended.
    fn wait_finished(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        while !self.finished.load(Ordering::Acquire) {
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(POLL);
        }
        true
    }

    /// Whether the process has ended.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}

/// Reads `source` line by line, sending what `parse` makes of each.
fn spawn_reader<R, P>(name: &str, source: R, events: Gate, parse: P) -> io::Result<JoinHandle<()>>
where
    R: io::Read + Send + 'static,
    P: FnMut(String) -> Vec<Event> + Send + 'static,
{
    thread::Builder::new().name(name.to_owned()).spawn(move || {
        let mut parse = parse;
        for line in BufReader::new(source).lines() {
            let Ok(line) = line else { break };
            for event in parse(line) {
                if !events.send(event) {
                    return;
                }
            }
        }
    })
}

/// Waits for `reader` until `deadline`; `false` when it is still running then (it ends on its own
/// when the pipe closes, and nobody waits for it any more).
fn join_before(reader: JoinHandle<()>, deadline: Instant) -> bool {
    while !reader.is_finished() {
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(POLL);
    }
    if reader.join().is_err() {
        log::warn!("composer: a reader thread panicked");
    }
    true
}

fn pid_of(pid: u32) -> Option<Pid> {
    Pid::from_raw(i32::try_from(pid).ok()?)
}

/// Signals the process only.
fn signal(pid: u32, signal: Signal) {
    if let Some(pid) = pid_of(pid)
        && let Err(error) = kill_process(pid, signal)
    {
        log::debug!("composer: cannot signal {pid:?}: {error}");
    }
}

/// Signals the process group the run leads.
fn signal_group(pid: u32, signal: Signal) {
    if let Some(pid) = pid_of(pid)
        && let Err(error) = kill_process_group(pid, signal)
    {
        log::debug!("composer: cannot signal the group of {pid:?}: {error}");
    }
}

/// Kills and reaps a child that never got its threads.
fn end(child: &mut Child) {
    if let Err(error) = child.kill().and_then(|()| child.wait().map(drop)) {
        log::debug!("composer: cannot end the child: {error}");
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::time::Instant;

    use super::*;

    /// A fake `claude`: a shell script, started like the real one.
    fn script(directory: &Path, body: &str) -> Invocation {
        let path = directory.join("claude");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        Invocation {
            program: path,
            args: Vec::new(),
            cwd: directory.to_path_buf(),
            env: Vec::<(OsString, OsString)>::new(),
            stdin: "make music".to_owned(),
        }
    }

    fn workspace(directory: &Path) -> Workspace {
        let file = directory.join("session-1.json");
        fs::write(&file, "{}").unwrap();
        Workspace::create(&file).unwrap()
    }

    fn collect(invocation: &Invocation, workspace: Workspace) -> (Run, mpsc::Receiver<Event>) {
        let (tx, rx) = mpsc::channel();
        let run = Run::start(invocation, workspace, move |event| {
            tx.send(event).ok();
        })
        .unwrap();
        (run, rx)
    }

    fn until_exit(rx: &mpsc::Receiver<Event>) -> Vec<Event> {
        let mut events = Vec::new();
        loop {
            let event = rx.recv_timeout(Duration::from_secs(20)).expect("an event");
            let exited = matches!(event, Event::Exited { .. });
            events.push(event);
            if exited {
                return events;
            }
        }
    }

    #[test]
    fn the_prompt_reaches_stdin_and_the_stream_comes_back_in_order() {
        let directory = tempfile::tempdir().unwrap();
        // Echoes the prompt it read inside a `result`, with a line of text before and of
        // stderr beside it.
        let run = script(
            directory.path(),
            r#"read prompt
echo '{"type":"system","subtype":"init","session_id":"s","model":"m","tools":[]}'
echo 'warning on stderr' >&2
echo 'not json'
printf '{"type":"result","subtype":"success","is_error":false,"result":"%s"}\n' "$prompt"
exit 3"#,
        );
        let (_run, rx) = collect(&run, workspace(directory.path()));
        let events = until_exit(&rx);
        assert!(matches!(
            events.first(),
            Some(Event::Init { .. } | Event::Stderr { .. })
        ));
        assert!(events.contains(&Event::Stderr {
            line: "warning on stderr".into()
        }));
        assert!(events.contains(&Event::Output {
            line: "not json".into()
        }));
        let result = events
            .iter()
            .find_map(|event| match event {
                Event::Result { text, success, .. } => Some((text.clone(), *success)),
                _ => None,
            })
            .expect("a result");
        assert_eq!(result, ("make music".to_owned(), true));
        assert_eq!(
            events.last(),
            Some(&Event::Exited {
                code: Some(3),
                stopped: false
            }),
            "the exit comes last and carries the code"
        );
    }

    #[test]
    fn a_stop_interrupts_the_run_and_the_result_is_still_delivered() {
        let directory = tempfile::tempdir().unwrap();
        // Like Claude Code: SIGINT ends the turn, and the result is written before the exit.
        let run = script(
            directory.path(),
            r#"trap 'echo "{\"type\":\"result\",\"subtype\":\"error_during_execution\",\"is_error\":true,\"result\":\"interrupted\"}"; exit 130' INT
echo '{"type":"system","subtype":"init","tools":[]}'
while true; do sleep 0.05; done"#,
        );
        let (handle, rx) = collect(&run, workspace(directory.path()));
        // Wait until the script is running its loop before interrupting it.
        match rx.recv_timeout(Duration::from_secs(10)).unwrap() {
            Event::Init { .. } => {}
            other => panic!("{other:?}"),
        }
        thread::sleep(Duration::from_millis(200));
        let started = Instant::now();
        handle.stop();
        let events = until_exit(&rx);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "it stopped promptly"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Result { success: false, .. }))
        );
        assert_eq!(
            events.last(),
            Some(&Event::Exited {
                code: Some(130),
                stopped: true
            })
        );
        assert!(handle.is_finished());
    }

    #[test]
    fn the_workspace_is_removed_once_the_process_has_ended() {
        let directory = tempfile::tempdir().unwrap();
        let space = workspace(directory.path());
        let root: PathBuf = space.cwd().parent().unwrap().to_path_buf();
        let run = script(directory.path(), "exit 0");
        let (_run, rx) = collect(&run, space);
        until_exit(&rx);
        let deadline = Instant::now() + Duration::from_secs(5);
        while root.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(!root.exists());
    }

    #[test]
    fn a_missing_program_is_a_start_error_not_a_panic() {
        let directory = tempfile::tempdir().unwrap();
        let mut run = script(directory.path(), "exit 0");
        run.program = directory.path().join("no-such-claude");
        let error = Run::start(&run, workspace(directory.path()), drop).unwrap_err();
        assert!(matches!(error, RunError::Spawn { .. }), "{error}");
    }
}

#[cfg(test)]
mod provider_seam_tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::sync::mpsc;
    use std::time::Instant;

    use super::*;

    fn script(directory: &Path, body: &str) -> Invocation {
        let path = directory.join("agent");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        Invocation {
            program: "/bin/sh".into(),
            args: vec![path.into_os_string()],
            cwd: directory.to_path_buf(),
            env: Vec::new(),
            stdin: String::new(),
        }
    }

    fn workspace(directory: &Path) -> Workspace {
        let file = directory.join("session-1.json");
        fs::write(&file, "{}").unwrap();
        Workspace::create(&file).unwrap()
    }

    fn until_exit(rx: &mpsc::Receiver<Event>) -> Vec<Event> {
        let mut events = Vec::new();
        loop {
            let event = rx.recv_timeout(Duration::from_secs(20)).expect("an event");
            let exited = matches!(event, Event::Exited { .. });
            events.push(event);
            if exited {
                return events;
            }
        }
    }

    /// Numbers the lines it gets, which a stateless parser cannot do, and turns two special
    /// lines into several events and into none.
    struct Counting {
        seen: usize,
    }

    impl LineParser for Counting {
        fn parse(&mut self, line: &str) -> Vec<Event> {
            self.seen += 1;
            match line {
                "skip" => Vec::new(),
                "twice" => (1..=2)
                    .map(|n| Event::Text {
                        text: format!("{}:{line}:{n}", self.seen),
                    })
                    .collect(),
                _ => vec![Event::Text {
                    text: format!("{}:{line}", self.seen),
                }],
            }
        }
    }

    #[test]
    fn the_parser_of_the_provider_sees_every_stdout_line_in_order() {
        let directory = tempfile::tempdir().unwrap();
        let invocation = script(
            directory.path(),
            "echo first\necho skip\necho twice\necho 'err line' >&2\necho last",
        );
        let (tx, rx) = mpsc::channel();
        let _run = Run::start_with(
            &invocation,
            workspace(directory.path()),
            Box::new(Counting { seen: 0 }),
            StopPolicy::CLAUDE,
            move |event| {
                tx.send(event).ok();
            },
        )
        .unwrap();
        let events = until_exit(&rx);
        let texts: Vec<&str> = events
            .iter()
            .filter_map(|event| match event {
                Event::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["1:first", "3:twice:1", "3:twice:2", "4:last"]);
        assert!(
            events.contains(&Event::Stderr {
                line: "err line".into()
            }),
            "stderr bypasses the parser"
        );
        assert_eq!(
            events.last(),
            Some(&Event::Exited {
                code: Some(0),
                stopped: false
            })
        );
    }

    #[test]
    fn start_is_start_with_the_claude_parser() {
        let directory = tempfile::tempdir().unwrap();
        let invocation = script(
            directory.path(),
            r#"echo '{"type":"result","subtype":"success","is_error":false,"result":"hi"}'
echo 'plain text'"#,
        );
        let (tx, rx) = mpsc::channel();
        let _run = Run::start(&invocation, workspace(directory.path()), move |event| {
            tx.send(event).ok();
        })
        .unwrap();
        let events = until_exit(&rx);
        assert!(matches!(
            events.first(),
            Some(Event::Result { success: true, text, .. }) if text == "hi"
        ));
        assert!(events.contains(&Event::Output {
            line: "plain text".into()
        }));
    }

    /// A helper that leaves the process group of the run (`setsid`, which ships with macOS'
    /// perl) and keeps the inherited pipes open, as a detached daemon would.
    const DETACHED_HELPER: &str =
        "perl -MPOSIX -e 'POSIX::setsid(); sleep 3; print qq(late\\n); sleep 4' &";

    #[test]
    fn a_helper_outside_the_group_that_holds_the_pipes_does_not_hold_back_the_exit() {
        let directory = tempfile::tempdir().unwrap();
        let invocation = script(
            directory.path(),
            &format!("{DETACHED_HELPER}\necho first\nexit 0"),
        );
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let started = Instant::now();
        let (tx, rx) = mpsc::channel();
        let _run = Run::start_with(
            &invocation,
            workspace(directory.path()),
            Box::new(ClaudeParser),
            StopPolicy::CLAUDE,
            move |event| {
                sink.lock().unwrap().push(event.clone());
                tx.send(event).ok();
            },
        )
        .unwrap();
        let events = until_exit(&rx);
        let elapsed = started.elapsed();
        assert!(
            elapsed >= READER_GRACE.saturating_sub(Duration::from_millis(100)),
            "the readers were given their grace: {elapsed:?}"
        );
        assert!(
            elapsed < READER_GRACE + Duration::from_secs(2),
            "the exit was reported although the pipes are still open: {elapsed:?}"
        );
        assert!(events.contains(&Event::Output {
            line: "first".into()
        }));
        assert_eq!(
            events.last(),
            Some(&Event::Exited {
                code: Some(0),
                stopped: false
            })
        );
        // The helper prints after the exit was reported: that line is dropped, not delivered
        // behind the exit.
        thread::sleep(Duration::from_secs(3));
        assert_eq!(
            seen.lock().unwrap().last(),
            Some(&Event::Exited {
                code: Some(0),
                stopped: false
            }),
            "nothing follows the exit"
        );
    }

    /// Whether any process of the group led by `pid` still exists.
    fn group_exists(pid: u32) -> bool {
        let pid = Pid::from_raw(i32::try_from(pid).unwrap()).unwrap();
        rustix::process::test_kill_process_group(pid).is_ok()
    }

    #[test]
    fn stop_now_ends_a_run_that_ignores_sigint_and_sigterm_before_it_returns() {
        let directory = tempfile::tempdir().unwrap();
        let invocation = script(
            directory.path(),
            "trap '' INT TERM\necho ready\nwhile true; do sleep 0.05; done",
        );
        let (tx, rx) = mpsc::channel();
        let run = Run::start_with(
            &invocation,
            workspace(directory.path()),
            Box::new(ClaudeParser),
            StopPolicy::CLAUDE,
            move |event| {
                tx.send(event).ok();
            },
        )
        .unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            Event::Output { .. }
        ));
        let started = Instant::now();
        run.stop_now(Duration::from_millis(200));
        let elapsed = started.elapsed();
        assert!(run.is_finished(), "the process is reaped when it returns");
        assert!(
            elapsed >= Duration::from_millis(200),
            "SIGINT got its grace: {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(3),
            "the call is bounded, not the 10 s and 5 s of the timer: {elapsed:?}"
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while group_exists(run.pid) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(!group_exists(run.pid), "no process of the group is left");
        assert_eq!(
            until_exit(&rx).last(),
            Some(&Event::Exited {
                code: None,
                stopped: true
            })
        );
    }

    #[test]
    fn stop_now_lets_a_run_that_honours_sigint_end_by_itself() {
        let directory = tempfile::tempdir().unwrap();
        let invocation = script(
            directory.path(),
            "trap 'echo bye; exit 130' INT\necho ready\nwhile true; do sleep 0.05; done",
        );
        let (tx, rx) = mpsc::channel();
        let run = Run::start_with(
            &invocation,
            workspace(directory.path()),
            Box::new(ClaudeParser),
            StopPolicy::CLAUDE,
            move |event| {
                tx.send(event).ok();
            },
        )
        .unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            Event::Output { .. }
        ));
        let started = Instant::now();
        run.stop_now(Duration::from_secs(5));
        assert!(run.is_finished());
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "it did not wait out the grace: {:?}",
            started.elapsed()
        );
        assert_eq!(
            until_exit(&rx).last(),
            Some(&Event::Exited {
                code: Some(130),
                stopped: true
            }),
            "ended by its own SIGINT handler, not killed"
        );
    }

    #[test]
    fn the_stop_policy_of_the_provider_decides_how_fast_a_stubborn_run_is_ended() {
        let directory = tempfile::tempdir().unwrap();
        // Ignores both polite signals, so only SIGKILL ends it.
        let invocation = script(
            directory.path(),
            "trap '' INT TERM\necho ready\nwhile true; do sleep 0.05; done",
        );
        let (tx, rx) = mpsc::channel();
        let run = Run::start_with(
            &invocation,
            workspace(directory.path()),
            Box::new(ClaudeParser),
            StopPolicy {
                interrupt_grace: Duration::from_millis(150),
                terminate_grace: Duration::from_millis(150),
            },
            move |event| {
                tx.send(event).ok();
            },
        )
        .unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            Event::Output { .. }
        ));
        let started = Instant::now();
        run.stop();
        let events = until_exit(&rx);
        let elapsed = started.elapsed();
        assert_eq!(
            events.last(),
            Some(&Event::Exited {
                code: None,
                stopped: true
            }),
            "ended by a signal, on request"
        );
        assert!(
            elapsed >= Duration::from_millis(300),
            "both graces were waited: {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(8),
            "the 10 s and 5 s of Claude's policy were not used: {elapsed:?}"
        );
    }
}
