//! The command line of a composer run, and the private folder it runs in.
//!
//! The run is a headless Claude Code (`claude -p`) that gets one thing: the Sonara tools, through
//! the bundled `sonara mcp`, attached to **this** window's session so that every edit shows on the
//! timeline. The tool lists below are the harness's safety boundary:
//!
//! - **Allowed:** the `sonara` MCP server, plus `WebSearch` and `WebFetch` for musical reference.
//! - **Denied:** every tool that writes a file or runs a program (a web page can carry a prompt
//!   injection, so there is no shell and no file write), and the Sonara tools that save, export,
//!   open or replace a project. The user saves and exports.
//! - `--permission-mode dontAsk` and `--permission-prompts none` deny whatever is not allowed
//!   instead of waiting for an answer nobody can give.
//!
//! **Isolation.** `--strict-mcp-config` keeps the user's own MCP servers out. The working
//! directory is an empty folder of the application's, so no repository `CLAUDE.md`, `.mcp.json` or
//! project hook is read. (Hooks and `CLAUDE.md` of `~/.claude` still load; `--bare` would skip
//! them, but it also skips the subscription login, which this user has instead of an API key.)
//!
//! **Pinning the session.** `sonara mcp` attaches to the *newest* running Sonara. To make sure it
//! is this window and not another instance, the run gets a `SONARA_HOME` of its own whose `run/`
//! folder holds a copy of this window's discovery file only, and `--connect always`, which fails
//! instead of falling back to a headless session that nobody can see.

use std::ffi::{OsStr, OsString};
use std::fs::{self, DirBuilder};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::json;

/// The harness's system prompt: what Sonara is, what the agent may do and how it should work.
pub const SYSTEM_PROMPT: &str = include_str!("system_prompt.md");

/// The name the Sonara tools have in the run (`mcp__sonara__…`).
const SERVER: &str = "sonara";

/// The longest prompt the panel accepts, in characters.
pub const MAX_PROMPT_CHARS: usize = 16_000;

/// A guard against a run that never ends; a song takes far fewer turns.
const MAX_TURNS: u32 = 200;

/// What the run may call without asking.
pub const ALLOWED_TOOLS: [&str; 3] = ["mcp__sonara", "WebSearch", "WebFetch"];

/// What the run may never call. Deny rules win over the allow rule of the whole server.
pub const DENIED_TOOLS: [&str; 18] = [
    "Bash",
    "Edit",
    "Write",
    "NotebookEdit",
    "AskUserQuestion",
    "mcp__sonara__save_project",
    "mcp__sonara__open_project",
    "mcp__sonara__create_project",
    "mcp__sonara__export_audio",
    "mcp__sonara__export_midi",
    "mcp__sonara__export_stems",
    "mcp__sonara__render_preview",
    "mcp__sonara__relink_media",
    "mcp__sonara__start_recording",
    "mcp__sonara__set_midi_input",
    "mcp__sonara__verify_project",
    "mcp__sonara__analyze_audio",
    "mcp__sonara__get_audio_analysis",
];

static NEXT_WORKSPACE: AtomicU32 = AtomicU32::new(0);

/// A private folder for one run, removed when it drops: an empty working directory, and a
/// `SONARA_HOME` whose `run/` holds a copy of one window's discovery file.
#[derive(Debug)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    /// Creates the folder (mode `0700`) with a copy of `discovery_file`, which is the
    /// `session-<pid>.json` of the window the run must edit.
    ///
    /// # Errors
    ///
    /// The I/O error of creating the folders or copying the file, and `InvalidInput` when the
    /// file's name is not that of a discovery file (clients find sessions by it).
    pub fn create(discovery_file: &Path) -> io::Result<Self> {
        let name = discovery_file
            .file_name()
            .filter(|name| {
                let name = name.to_string_lossy();
                name.starts_with("session-") && name.ends_with(".json")
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{} is not a discovery file", discovery_file.display()),
                )
            })?;
        let serial = NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("sonara-composer-{}-{serial}", std::process::id()));
        let workspace = Self { root };
        // From here on a failure drops `workspace`, which removes what was created.
        private_dir(&workspace.root)?;
        private_dir(&workspace.cwd())?;
        private_dir(&workspace.home().join("run"))?;
        fs::copy(discovery_file, workspace.home().join("run").join(name))?;
        Ok(workspace)
    }

    /// The `SONARA_HOME` of the run.
    #[must_use]
    pub fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    /// The working directory of the run.
    #[must_use]
    pub fn cwd(&self) -> PathBuf {
        self.root.join("cwd")
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root)
            && error.kind() != io::ErrorKind::NotFound
        {
            log::warn!("cannot remove {}: {error}", self.root.display());
        }
    }
}

fn private_dir(path: &Path) -> io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(path)
}

/// The `PATH` of a run: the directory of `claude` first, then the inherited one, then the usual
/// prefixes. A Finder launch has only launchd's minimal `PATH`, and Claude Code looks for helper
/// programs (`git`, `rg`, `node` for an npm install) on it.
#[must_use]
pub fn search_path(claude: &Path, inherited: Option<&OsStr>) -> OsString {
    let mut directories: Vec<PathBuf> = Vec::new();
    let mut add = |directory: PathBuf| {
        if !directory.as_os_str().is_empty() && !directories.contains(&directory) {
            directories.push(directory);
        }
    };
    if let Some(directory) = claude.parent() {
        add(directory.to_path_buf());
    }
    if let Some(inherited) = inherited {
        std::env::split_paths(inherited).for_each(&mut add);
    }
    for prefix in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"] {
        add(PathBuf::from(prefix));
    }
    std::env::join_paths(directories).unwrap_or_else(|_| inherited.unwrap_or_default().to_owned())
}

/// A process to start: the program, its arguments, where it runs and what it reads on stdin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// The executable.
    pub program: PathBuf,
    /// Its arguments.
    pub args: Vec<OsString>,
    /// Its working directory.
    pub cwd: PathBuf,
    /// Environment variables added to the inherited environment.
    pub env: Vec<(OsString, OsString)>,
    /// What the process reads on stdin: the user's prompt.
    pub stdin: String,
}

/// Why a prompt cannot start a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PromptError {
    /// There is nothing to compose from.
    #[error("describe the music you want first")]
    Empty,
    /// The prompt is longer than [`MAX_PROMPT_CHARS`].
    #[error("the description is longer than {MAX_PROMPT_CHARS} characters")]
    TooLong,
}

/// The trimmed prompt, if it is usable.
///
/// # Errors
///
/// [`PromptError`] for an empty or a too long prompt.
pub fn check_prompt(prompt: &str) -> Result<&str, PromptError> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        Err(PromptError::Empty)
    } else if prompt.chars().count() > MAX_PROMPT_CHARS {
        Err(PromptError::TooLong)
    } else {
        Ok(prompt)
    }
}

/// The `--mcp-config` document: the bundled `sonara mcp`, forced to attach to the session of
/// `workspace`'s discovery file.
#[must_use]
pub fn mcp_config(sonara_cli: &Path, workspace: &Workspace) -> String {
    json!({
        "mcpServers": {
            SERVER: {
                "command": sonara_cli,
                "args": [
                    "mcp",
                    "--connect", "always",
                    "--base-dir", workspace.cwd(),
                    "--log-level", "WARNING",
                ],
                "env": { "SONARA_HOME": workspace.home() },
            }
        }
    })
    .to_string()
}

/// The command line of a run of `claude` that composes from `prompt` in `workspace`.
///
/// `prompt` goes through stdin, so that a prompt which begins with `-` is not read as a flag.
#[must_use]
pub fn build(claude: &Path, sonara_cli: &Path, workspace: &Workspace, prompt: &str) -> Invocation {
    let args = [
        "-p".to_owned(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--verbose".to_owned(),
        "--include-partial-messages".to_owned(),
        "--strict-mcp-config".to_owned(),
        "--mcp-config".to_owned(),
        mcp_config(sonara_cli, workspace),
        "--permission-mode".to_owned(),
        "dontAsk".to_owned(),
        "--permission-prompts".to_owned(),
        "none".to_owned(),
        "--allowedTools".to_owned(),
        ALLOWED_TOOLS.join(","),
        "--disallowedTools".to_owned(),
        DENIED_TOOLS.join(","),
        "--max-turns".to_owned(),
        MAX_TURNS.to_string(),
        "--append-system-prompt".to_owned(),
        SYSTEM_PROMPT.to_owned(),
    ];
    Invocation {
        program: claude.to_path_buf(),
        args: args.into_iter().map(OsString::from).collect(),
        cwd: workspace.cwd(),
        env: vec![(
            OsString::from("PATH"),
            search_path(claude, std::env::var_os("PATH").as_deref()),
        )],
        stdin: prompt.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn discovery(directory: &Path) -> PathBuf {
        let file = directory.join("session-4242.json");
        fs::write(&file, r#"{"pid": 4242}"#).unwrap();
        file
    }

    fn arg_after<'a>(invocation: &'a Invocation, flag: &str) -> &'a str {
        let position = invocation
            .args
            .iter()
            .position(|arg| arg == flag)
            .unwrap_or_else(|| panic!("{flag} is missing"));
        invocation.args[position + 1].to_str().unwrap()
    }

    #[test]
    fn the_workspace_holds_only_this_windows_discovery_file_and_goes_away_with_it() {
        let source = tempfile::tempdir().unwrap();
        let other = source.path().join("session-1.json");
        fs::write(&other, "{}").unwrap();
        let file = discovery(source.path());
        let workspace = Workspace::create(&file).unwrap();
        let run = workspace.home().join("run");
        let names: Vec<_> = fs::read_dir(&run)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["session-4242.json"], "no other session is visible");
        assert_eq!(
            fs::read_to_string(run.join("session-4242.json")).unwrap(),
            r#"{"pid": 4242}"#
        );
        assert!(workspace.cwd().is_dir());
        let root = workspace.cwd().parent().unwrap().to_path_buf();
        drop(workspace);
        assert!(!root.exists(), "the folder is removed");
    }

    #[test]
    fn the_workspace_is_private_to_the_user() {
        use std::os::unix::fs::PermissionsExt;
        let source = tempfile::tempdir().unwrap();
        let workspace = Workspace::create(&discovery(source.path())).unwrap();
        for directory in [workspace.cwd(), workspace.home().join("run")] {
            let mode = fs::metadata(&directory).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0, "{}", directory.display());
        }
    }

    #[test]
    fn a_file_that_is_no_discovery_file_is_refused() {
        let source = tempfile::tempdir().unwrap();
        let file = source.path().join("settings.json");
        fs::write(&file, "{}").unwrap();
        let error = Workspace::create(&file).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn the_mcp_config_forces_attach_to_the_pinned_home() {
        let source = tempfile::tempdir().unwrap();
        let workspace = Workspace::create(&discovery(source.path())).unwrap();
        let config: serde_json::Value =
            serde_json::from_str(&mcp_config(Path::new("/App/sonara"), &workspace)).unwrap();
        let server = &config["mcpServers"]["sonara"];
        assert_eq!(server["command"], "/App/sonara");
        let args: Vec<&str> = server["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|arg| arg.as_str().unwrap())
            .collect();
        assert_eq!(args[..3], ["mcp", "--connect", "always"]);
        assert_eq!(
            server["env"]["SONARA_HOME"],
            workspace.home().to_str().unwrap()
        );
    }

    #[test]
    fn the_run_is_isolated_headless_streaming_and_denies_by_default() {
        let source = tempfile::tempdir().unwrap();
        let workspace = Workspace::create(&discovery(source.path())).unwrap();
        let run = build(
            Path::new("/bin/claude"),
            Path::new("/App/sonara"),
            &workspace,
            "-- make a house beat",
        );
        assert_eq!(run.program, Path::new("/bin/claude"));
        assert_eq!(run.cwd, workspace.cwd());
        assert_eq!(run.stdin, "-- make a house beat");
        for flag in [
            "-p",
            "--verbose",
            "--include-partial-messages",
            "--strict-mcp-config",
        ] {
            assert!(run.args.iter().any(|arg| arg == flag), "{flag}");
        }
        assert_eq!(arg_after(&run, "--output-format"), "stream-json");
        assert_eq!(arg_after(&run, "--permission-mode"), "dontAsk");
        assert_eq!(arg_after(&run, "--permission-prompts"), "none");
        assert_eq!(
            arg_after(&run, "--allowedTools"),
            "mcp__sonara,WebSearch,WebFetch"
        );
        assert!(
            !run.args.iter().any(|arg| arg == "--bare"),
            "--bare would skip the subscription login"
        );
        assert!(
            !run.args.iter().any(|arg| arg
                .to_str()
                .is_some_and(|a| a.contains("make a house beat"))),
            "the prompt travels on stdin, never as an argument"
        );
        assert!(arg_after(&run, "--append-system-prompt").contains("composer inside Sonara"));
    }

    #[test]
    fn nothing_that_saves_exports_or_runs_a_program_is_allowed() {
        let denied = DENIED_TOOLS.join(",");
        for tool in [
            "Bash",
            "Edit",
            "Write",
            "mcp__sonara__save_project",
            "mcp__sonara__export_audio",
            "mcp__sonara__export_midi",
            "mcp__sonara__export_stems",
            "mcp__sonara__open_project",
            "mcp__sonara__create_project",
        ] {
            assert!(denied.split(',').any(|t| t == tool), "{tool}");
        }
        // The denied Sonara tools exist in the registry, so a typo cannot leave one open.
        for tool in DENIED_TOOLS {
            if let Some(name) = tool.strip_prefix("mcp__sonara__") {
                assert!(
                    include_str!("../../../../docs/agent-api.md")
                        .contains(&format!("### `{name}`")),
                    "{name} is not a Sonara tool"
                );
            }
        }
        for tool in ALLOWED_TOOLS {
            assert!(!DENIED_TOOLS.contains(&tool));
        }
    }

    #[test]
    fn the_run_finds_its_helpers_even_from_a_finder_launch() {
        let path = search_path(
            Path::new("/Users/a/.local/bin/claude"),
            Some(OsStr::new("/usr/bin:/bin")),
        );
        let directories: Vec<PathBuf> = std::env::split_paths(&path).collect();
        assert_eq!(directories[0], Path::new("/Users/a/.local/bin"));
        assert!(directories.contains(&PathBuf::from("/opt/homebrew/bin")));
        assert_eq!(
            directories
                .iter()
                .filter(|d| **d == Path::new("/usr/bin"))
                .count(),
            1,
            "no directory twice"
        );
        let source = tempfile::tempdir().unwrap();
        let workspace = Workspace::create(&discovery(source.path())).unwrap();
        let run = build(
            Path::new("/x/claude"),
            Path::new("/App/sonara"),
            &workspace,
            "p",
        );
        assert!(run.env.iter().any(|(key, _)| key == "PATH"));
    }

    #[test]
    fn the_prompt_is_checked_before_a_run_starts() {
        assert_eq!(check_prompt("  \n "), Err(PromptError::Empty));
        assert_eq!(check_prompt("  a chill beat \n"), Ok("a chill beat"));
        let long = "x".repeat(MAX_PROMPT_CHARS + 1);
        assert_eq!(check_prompt(&long), Err(PromptError::TooLong));
        assert!(check_prompt(&long[..MAX_PROMPT_CHARS]).is_ok());
    }

    #[test]
    fn the_system_prompt_names_the_boundaries() {
        for needle in [
            "Sonara",
            "get_project_state",
            "generate_midi",
            "beats",
            "Do not save",
        ] {
            assert!(SYSTEM_PROMPT.contains(needle), "{needle}");
        }
    }
}
