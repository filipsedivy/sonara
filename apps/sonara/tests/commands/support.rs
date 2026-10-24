//! Running the built `sonara` binary in a sandbox: a temporary working directory with its own
//! Sonara home and projects folder, so nothing reaches the developer's real home (settings,
//! audit log, workspaces).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;
use tempfile::TempDir;

/// A sandbox: `<root>/home` is `SONARA_HOME`, `<root>/projects` is `SONARA_PROJECTS_DIR`,
/// `<root>` is the working directory, and `PATH` is empty, so the import's `FFmpeg` fallback
/// finds no `ffmpeg`, as in the golden recordings.
pub struct Sandbox {
    _dir: TempDir,
    root: PathBuf,
}

/// A finished run: exit code, stdout parsed as JSON (when it is), stdout and stderr as text.
#[derive(Debug)]
pub struct Run {
    pub code: i32,
    pub json: Value,
    pub stdout: String,
    pub stderr: String,
}

impl Sandbox {
    pub fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("sonara-cli-")
            .tempdir()
            .expect("a temporary directory");
        let root = std::fs::canonicalize(dir.path()).expect("the temporary directory resolves");
        for sub in ["home", "projects"] {
            std::fs::create_dir(root.join(sub)).expect("a sandbox folder");
        }
        Self { _dir: dir, root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `sonara` with the sandbox's environment and working directory.
    pub fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_sonara"));
        command
            .current_dir(&self.root)
            .env("SONARA_HOME", self.root.join("home"))
            .env("SONARA_PROJECTS_DIR", self.root.join("projects"))
            .env_remove("SONARA_AGENT_ALLOWED_PATHS")
            // No FFmpeg on PATH: the recorder treated it as absent (`audio_import.ffmpeg_path`).
            .env("PATH", "")
            .stdin(Stdio::null());
        command
    }

    /// Runs `sonara args...` to the end.
    pub fn run(&self, args: &[&str]) -> Run {
        let output = self
            .command()
            .args(args)
            .output()
            .expect("the sonara binary runs");
        let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
        let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
        Run {
            code: output.status.code().expect("sonara exits normally"),
            json: serde_json::from_str(&stdout).unwrap_or(Value::Null),
            stdout,
            stderr,
        }
    }
}
