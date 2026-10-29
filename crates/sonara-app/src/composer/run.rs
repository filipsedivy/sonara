//! Running a headless Claude Code and streaming what it reports.
//!
//! **Threads.** The process is read by four threads, none of them real-time: one per output pipe,
//! one that writes the prompt, and one that waits for the exit. Their events go through one
//! channel to an emitter thread that owns the caller's sink, so the sink needs no lock and the
//! events arrive in order, with [`Event::Exited`] last.
//!
//! **Stopping.** [`Run::stop`] sends `SIGINT` to Claude Code, which ends the turn and still
//! writes its `result` (a `SIGTERM` would exit with 143 and no result). A run that ignores it is
//! sent `SIGTERM` and then `SIGKILL` as a process group, which also ends the `sonara mcp` child.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{Sender, unbounded};
use rustix::process::{Pid, Signal, kill_process, kill_process_group};

use super::invocation::{Invocation, Workspace};
use super::stream::{Event, parse_line};

/// How long a run gets to finish after `SIGINT`, before `SIGTERM`.
const INTERRUPT_GRACE: Duration = Duration::from_secs(10);

/// How long it gets after `SIGTERM`, before `SIGKILL`.
const TERMINATE_GRACE: Duration = Duration::from_secs(5);

/// A running Claude Code.
#[derive(Debug)]
pub struct Run {
    pid: u32,
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
    /// Starts `invocation` and delivers its events to `sink` on a thread of its own, until
    /// [`Event::Exited`]. The `workspace` is removed after the process has ended.
    ///
    /// # Errors
    ///
    /// [`RunError`] when the process or one of the threads cannot be started.
    pub fn start<F>(
        invocation: &Invocation,
        workspace: Workspace,
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

        let (events, receiver) = unbounded::<Event>();
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
            let out = spawn_reader("composer-stdout", stdout, events.clone(), |line| {
                parse_line(&line)
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
                    // The pipes close with the process; read them to the end first.
                    for reader in [out, err] {
                        if reader.join().is_err() {
                            log::warn!("composer: a reader thread panicked");
                        }
                    }
                    let stopped = stopped.load(Ordering::Acquire);
                    if events.send(Event::Exited { code, stopped }).is_err() {
                        log::debug!("composer: nobody listens for the exit");
                    }
                    drop(workspace);
                })?;
            thread::Builder::new()
                .name("composer-events".to_owned())
                .spawn(move || {
                    for event in receiver {
                        sink(event);
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
        let finished = Arc::clone(&self.finished);
        let escalation = thread::Builder::new()
            .name("composer-stop".to_owned())
            .spawn(move || {
                for (grace, signal) in [
                    (INTERRUPT_GRACE, Signal::TERM),
                    (TERMINATE_GRACE, Signal::KILL),
                ] {
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

    /// Whether the process has ended.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}

/// Reads `source` line by line, sending what `parse` makes of each.
fn spawn_reader<R, P>(
    name: &str,
    source: R,
    events: Sender<Event>,
    parse: P,
) -> io::Result<JoinHandle<()>>
where
    R: io::Read + Send + 'static,
    P: Fn(String) -> Vec<Event> + Send + 'static,
{
    thread::Builder::new().name(name.to_owned()).spawn(move || {
        for line in BufReader::new(source).lines() {
            let Ok(line) = line else { break };
            for event in parse(line) {
                if events.send(event).is_err() {
                    return;
                }
            }
        }
    })
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
