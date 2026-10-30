//! Running a short question to an agent CLI (`--version`, `--help`, `login status`) with a
//! deadline, so that detection never waits for a CLI that hangs.
//!
//! The question runs in a process group of its own and is killed with that group when the
//! deadline passes, so a helper it started cannot outlive the question. The output is read on
//! threads that this module never waits for unboundedly: a helper that keeps the pipe open after
//! the CLI has exited costs [`DRAIN_GRACE`] at most.

use std::io;
use std::path::Path;
use std::process::ExitStatus;
use std::time::Duration;

/// What a probe printed, and how it ended.
#[derive(Debug, Clone)]
pub struct Answer {
    /// How the process ended.
    pub status: ExitStatus,
    /// What it wrote to standard output (lossy UTF-8, cut at [`MAX_OUTPUT`] bytes).
    pub stdout: String,
    /// What it wrote to standard error (lossy UTF-8, cut at [`MAX_OUTPUT`] bytes).
    pub stderr: String,
}

/// The most bytes of one output stream a probe keeps.
pub const MAX_OUTPUT: usize = 256 * 1024;

/// How long the output pipes are read after the process has ended.
pub const DRAIN_GRACE: Duration = Duration::from_millis(250);

/// How often the process is asked whether it has ended.
#[cfg(unix)]
const POLL: Duration = Duration::from_millis(10);

/// Runs `program args` with stdin closed and `PATH` set to `path`, and waits at most `timeout`.
///
/// # Errors
///
/// The I/O error of starting the program; [`io::ErrorKind::TimedOut`] when it has not ended by
/// the deadline (it is killed with its process group), and an error on a platform without
/// process groups ([`io::ErrorKind::Unsupported`]).
#[cfg(unix)]
pub fn run_bounded(
    program: &Path,
    args: &[&str],
    path: &std::ffi::OsStr,
    timeout: Duration,
) -> io::Result<Answer> {
    use std::io::Read;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::sync::mpsc::{self, Receiver};
    use std::thread;
    use std::time::Instant;

    use rustix::process::{Pid, Signal, kill_process_group};

    /// Reads `source` to its end on a thread of its own, in chunks.
    fn drain<R: Read + Send + 'static>(mut source: R) -> io::Result<Receiver<Vec<u8>>> {
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name("provider-probe".to_owned())
            .spawn(move || {
                let mut chunk = [0_u8; 4096];
                while let Ok(read) = source.read(&mut chunk) {
                    let Some(bytes) = chunk.get(..read).filter(|bytes| !bytes.is_empty()) else {
                        break;
                    };
                    if tx.send(bytes.to_vec()).is_err() {
                        break;
                    }
                }
            })?;
        Ok(rx)
    }

    /// What `rx` delivers until it closes or `until` passes.
    fn collect(rx: &Receiver<Vec<u8>>, until: Instant) -> String {
        let mut bytes: Vec<u8> = Vec::new();
        while let Ok(chunk) = rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
            let room = MAX_OUTPUT.saturating_sub(bytes.len());
            bytes.extend(chunk.into_iter().take(room));
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }

    let mut child = Command::new(program)
        .args(args)
        .env("PATH", path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        return Err(io::Error::other("a pipe is missing"));
    };
    let (stdout, stderr) = (drain(stdout)?, drain(stderr)?);
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            if let Some(group) = i32::try_from(child.id()).ok().and_then(Pid::from_raw)
                && let Err(error) = kill_process_group(group, Signal::KILL)
            {
                log::debug!("chat: cannot kill the group of a probe: {error}");
            }
            if let Err(error) = child.kill().and_then(|()| child.wait().map(drop)) {
                log::debug!("chat: cannot end a probe: {error}");
            }
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "`{} {}` did not answer within {} s",
                    program.display(),
                    args.join(" "),
                    timeout.as_secs_f64()
                ),
            ));
        }
        thread::sleep(POLL);
    };
    let until = Instant::now() + DRAIN_GRACE;
    Ok(Answer {
        status,
        stdout: collect(&stdout, until),
        stderr: collect(&stderr, until),
    })
}

/// Without process groups there is no bounded probe.
///
/// # Errors
///
/// Always [`io::ErrorKind::Unsupported`].
#[cfg(not(unix))]
pub fn run_bounded(
    _program: &Path,
    _args: &[&str],
    _path: &std::ffi::OsStr,
    _timeout: Duration,
) -> io::Result<Answer> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "probing an agent CLI needs macOS",
    ))
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    mod processes {
        use std::ffi::OsStr;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;
        use std::time::Instant;

        use crate::chat::providers::probe::*;

        fn script(body: &str) -> (tempfile::TempDir, PathBuf) {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("agent");
            fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            (directory, path)
        }

        const PATH: &str = "/usr/bin:/bin";

        #[test]
        fn a_probe_returns_both_streams_and_the_status() {
            let (_dir, program) = script("echo out; echo err >&2; echo \"args: $*\"; exit 3");
            let answer = run_bounded(
                &program,
                &["--version", "x"],
                OsStr::new(PATH),
                Duration::from_secs(10),
            )
            .unwrap();
            assert_eq!(answer.status.code(), Some(3));
            assert_eq!(answer.stdout, "out\nargs: --version x\n");
            assert_eq!(answer.stderr, "err\n");
        }

        #[test]
        fn the_probe_gets_the_path_it_is_given_and_stdin_is_closed() {
            let (_dir, program) = script("echo \"$PATH\"; cat; echo eof");
            let answer = run_bounded(
                &program,
                &[],
                OsStr::new("/a:/b:/usr/bin:/bin"),
                Duration::from_secs(10),
            )
            .unwrap();
            assert_eq!(answer.stdout, "/a:/b:/usr/bin:/bin\neof\n");
        }

        #[test]
        fn a_hanging_probe_is_killed_at_the_deadline_with_its_helpers() {
            // The helper `sleep` holds the pipes open; the group kill ends both.
            let (_dir, program) = script("sleep 30\necho never");
            let started = Instant::now();
            let error = run_bounded(
                &program,
                &["--version"],
                OsStr::new(PATH),
                Duration::from_millis(300),
            )
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::TimedOut, "{error}");
            assert!(error.to_string().contains("--version"), "{error}");
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "{:?}",
                started.elapsed()
            );
        }

        #[test]
        fn a_helper_that_keeps_the_pipe_open_does_not_hold_the_answer_back() {
            let (_dir, program) = script("echo done\nsleep 3 &\nexit 0");
            let started = Instant::now();
            let answer =
                run_bounded(&program, &[], OsStr::new(PATH), Duration::from_secs(10)).unwrap();
            assert_eq!(answer.stdout, "done\n");
            assert!(answer.status.success());
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "{:?}",
                started.elapsed()
            );
        }

        #[test]
        fn a_missing_program_is_the_error_of_starting_it() {
            let error = run_bounded(
                Path::new("/nonexistent/codex"),
                &[],
                OsStr::new(PATH),
                Duration::from_secs(1),
            )
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::NotFound);
        }

        #[test]
        fn output_beyond_the_limit_is_cut() {
            let (_dir, program) = script("head -c 400000 /dev/zero | tr '\\0' 'x'");
            let answer =
                run_bounded(&program, &[], OsStr::new(PATH), Duration::from_secs(10)).unwrap();
            assert_eq!(answer.stdout.len(), MAX_OUTPUT);
        }
    }
}
