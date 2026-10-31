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
/// A run receives it followed by [`ORCHESTRATION_REFERENCE`] ([`system_prompt`]).
pub const SYSTEM_PROMPT: &str = include_str!("system_prompt.md");

/// The orchestration reference: how orchestral parts are written with the tools, every
/// instrument id with its ranges and the articulation ids a note may carry, and the presets of
/// the library profiles Sonara ships.
///
/// `system_prompt_orchestra.md` is generated, never edited by hand: the example
/// `gen_chat_reference` writes it from the orchestration metadata and the shipped profiles
/// (`sonara_app::instruments::agent_reference`), and that example's test fails while the
/// committed file differs from what they render.
pub const ORCHESTRATION_REFERENCE: &str = include_str!("system_prompt_orchestra.md");

/// The whole system prompt of a run: [`SYSTEM_PROMPT`], a line break and
/// [`ORCHESTRATION_REFERENCE`].
const FULL_SYSTEM_PROMPT: &str = concat!(
    include_str!("system_prompt.md"),
    "\n",
    include_str!("system_prompt_orchestra.md")
);

/// [`SYSTEM_PROMPT`] followed by [`ORCHESTRATION_REFERENCE`]: what a run is told before any
/// addendum of the caller.
#[must_use]
pub const fn system_prompt() -> &'static str {
    FULL_SYSTEM_PROMPT
}

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

/// How many names [`Workspace::create`] tries before it gives up on a crowded temporary folder.
const NAME_ATTEMPTS: u32 = 16;

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
    /// The root is created, never reused: a folder that already exists at the chosen name (left
    /// by a crashed run with the same process id, or planted by another user under a shared
    /// temporary directory) is not entered, and the next name is tried instead. The folders
    /// inside it are made new inside it.
    ///
    /// # Errors
    ///
    /// The I/O error of creating the folders or copying the file, and `InvalidInput` when the
    /// file's name is not that of a discovery file (clients find sessions by it). When every
    /// name that was tried exists already, [`io::ErrorKind::AlreadyExists`].
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
        let base = std::env::temp_dir();
        let mut taken = None;
        for _ in 0..NAME_ATTEMPTS {
            let serial = NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed);
            let root = base.join(format!("sonara-composer-{}-{serial}", std::process::id()));
            match Self::create_at(root, discovery_file, name) {
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => taken = Some(error),
                result => return result,
            }
        }
        Err(taken.unwrap_or_else(|| io::Error::from(io::ErrorKind::AlreadyExists)))
    }

    /// [`Workspace::create`] at exactly `root`, which must not exist.
    fn create_at(root: PathBuf, discovery_file: &Path, name: &OsStr) -> io::Result<Self> {
        // Not recursive: an existing path (a directory, or a symlink to one) is an error, and
        // the workspace is not built before this succeeds, so its `Drop` can never remove
        // what this call did not create.
        private_dir(&root)?;
        let workspace = Self { root };
        // From here on a failure drops `workspace`, which removes what was created.
        private_dir(&workspace.cwd())?;
        private_dir(&workspace.home())?;
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

/// Creates the one directory `path` (mode `0700`), whose parent exists; an error if `path`
/// exists.
fn private_dir(path: &Path) -> io::Result<()> {
    let mut builder = DirBuilder::new();
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

/// What a caller may change in the command line of a run of Claude Code. The default is the
/// composer panel's run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildOptions<'a> {
    /// Whether Claude Code keeps the session on disk (`false` adds `--no-session-persistence`).
    pub persist_session: bool,
    /// Text appended to [`system_prompt`] in the same `--append-system-prompt` value.
    pub system_prompt_addendum: Option<&'a str>,
}

impl Default for BuildOptions<'_> {
    fn default() -> Self {
        Self {
            persist_session: true,
            system_prompt_addendum: None,
        }
    }
}

/// The command line of a run of `claude` that composes from `prompt` in `workspace`.
///
/// `prompt` goes through stdin, so that a prompt which begins with `-` is not read as a flag.
/// Same as [`build_with`] with the default [`BuildOptions`].
#[must_use]
pub fn build(claude: &Path, sonara_cli: &Path, workspace: &Workspace, prompt: &str) -> Invocation {
    build_with(
        claude,
        sonara_cli,
        workspace,
        prompt,
        &BuildOptions::default(),
    )
}

/// [`build`] with `options`: the tool boundaries, the isolation and the pinned session are the
/// same whatever the options say.
#[must_use]
pub fn build_with(
    claude: &Path,
    sonara_cli: &Path,
    workspace: &Workspace,
    prompt: &str,
    options: &BuildOptions<'_>,
) -> Invocation {
    let mut args = vec![
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
    ];
    if !options.persist_session {
        args.push("--no-session-persistence".to_owned());
    }
    // One flag only: a second `--append-system-prompt` would replace the first.
    args.push("--append-system-prompt".to_owned());
    args.push(match options.system_prompt_addendum {
        Some(addendum) => format!("{}\n{addendum}", system_prompt()),
        None => system_prompt().to_owned(),
    });
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
    use std::os::unix::fs::PermissionsExt;

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
        let source = tempfile::tempdir().unwrap();
        let workspace = Workspace::create(&discovery(source.path())).unwrap();
        let root = workspace.cwd().parent().unwrap().to_path_buf();
        for directory in [root, workspace.cwd(), workspace.home().join("run")] {
            let mode = fs::metadata(&directory).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0, "{}", directory.display());
        }
    }

    #[test]
    fn an_existing_folder_is_never_reused_and_never_removed() {
        let source = tempfile::tempdir().unwrap();
        let file = discovery(source.path());
        let name = file.file_name().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        // Another user's folder at the predictable name, with a file of theirs inside.
        let planted = scratch.path().join("sonara-composer-1-0");
        fs::create_dir(&planted).unwrap();
        fs::write(planted.join("theirs"), "keep").unwrap();
        let error = Workspace::create_at(planted.clone(), &file, name).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists, "{error}");
        assert_eq!(
            fs::read_to_string(planted.join("theirs")).unwrap(),
            "keep",
            "what was there stays, as the failed workspace removes only what it made"
        );
        assert!(
            !planted.join("home").exists() && !planted.join("cwd").exists(),
            "nothing was made inside it"
        );

        // A symlink to a directory is no better: it is not followed.
        let target = scratch.path().join("target");
        fs::create_dir(&target).unwrap();
        let link = scratch.path().join("sonara-composer-1-1");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let error = Workspace::create_at(link.clone(), &file, name).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists, "{error}");
        assert_eq!(
            fs::read_dir(&target).unwrap().count(),
            0,
            "nothing went through it"
        );
        assert!(link.is_symlink(), "and the link is left alone");

        // A fresh name works, and is private all the way down, the root included.
        let fresh = scratch.path().join("sonara-composer-1-2");
        let workspace = Workspace::create_at(fresh.clone(), &file, name).unwrap();
        for directory in [&fresh, &workspace.cwd(), &workspace.home()] {
            let mode = fs::metadata(directory).unwrap().permissions().mode();
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
    fn the_system_prompt_is_the_composer_prompt_then_the_orchestration_reference() {
        assert_eq!(
            system_prompt(),
            format!("{SYSTEM_PROMPT}\n{ORCHESTRATION_REFERENCE}")
        );
        assert!(ORCHESTRATION_REFERENCE.starts_with("# Orchestral parts\n"));
        assert!(ORCHESTRATION_REFERENCE.contains("\n## Instruments\n"));
        assert!(ORCHESTRATION_REFERENCE.contains("\n## Presets of the library profiles"));
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

#[cfg(test)]
mod build_options_tests {
    use super::*;

    fn workspace() -> (tempfile::TempDir, Workspace) {
        let source = tempfile::tempdir().unwrap();
        let file = source.path().join("session-4242.json");
        fs::write(&file, "{}").unwrap();
        let workspace = Workspace::create(&file).unwrap();
        (source, workspace)
    }

    fn strings(invocation: &Invocation) -> Vec<&str> {
        invocation
            .args
            .iter()
            .map(|arg| arg.to_str().unwrap())
            .collect()
    }

    #[test]
    fn the_composer_command_line_is_exactly_what_it_was() {
        let (_source, workspace) = workspace();
        let run = build(
            Path::new("/bin/claude"),
            Path::new("/App/sonara"),
            &workspace,
            "p",
        );
        let config = mcp_config(Path::new("/App/sonara"), &workspace);
        let allowed = ALLOWED_TOOLS.join(",");
        let denied = DENIED_TOOLS.join(",");
        let expected = [
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--strict-mcp-config",
            "--mcp-config",
            &config,
            "--permission-mode",
            "dontAsk",
            "--permission-prompts",
            "none",
            "--allowedTools",
            &allowed,
            "--disallowedTools",
            &denied,
            "--max-turns",
            "200",
            "--append-system-prompt",
            system_prompt(),
        ];
        assert_eq!(strings(&run), expected);
    }

    #[test]
    fn the_default_options_change_nothing() {
        let (_source, workspace) = workspace();
        let plain = build(
            Path::new("/bin/claude"),
            Path::new("/App/sonara"),
            &workspace,
            "p",
        );
        let with = build_with(
            Path::new("/bin/claude"),
            Path::new("/App/sonara"),
            &workspace,
            "p",
            &BuildOptions::default(),
        );
        assert_eq!(plain, with);
        assert!(BuildOptions::default().persist_session);
    }

    #[test]
    fn an_unpersisted_session_adds_one_flag_and_nothing_else() {
        let (_source, workspace) = workspace();
        let plain = build(
            Path::new("/bin/claude"),
            Path::new("/App/sonara"),
            &workspace,
            "p",
        );
        let chat = build_with(
            Path::new("/bin/claude"),
            Path::new("/App/sonara"),
            &workspace,
            "p",
            &BuildOptions {
                persist_session: false,
                system_prompt_addendum: None,
            },
        );
        let mut without = strings(&chat);
        let position = without
            .iter()
            .position(|arg| *arg == "--no-session-persistence")
            .expect("the flag is there");
        without.remove(position);
        assert_eq!(without, strings(&plain));
    }

    #[test]
    fn the_addendum_extends_the_one_system_prompt_flag() {
        let (_source, workspace) = workspace();
        let run = build_with(
            Path::new("/bin/claude"),
            Path::new("/App/sonara"),
            &workspace,
            "-- keep this off the command line",
            &BuildOptions {
                persist_session: false,
                system_prompt_addendum: Some("# Addendum marker"),
            },
        );
        let args = strings(&run);
        assert_eq!(
            args.iter()
                .filter(|arg| **arg == "--append-system-prompt")
                .count(),
            1,
            "a second flag would replace the first"
        );
        let position = args
            .iter()
            .position(|arg| *arg == "--append-system-prompt")
            .unwrap();
        let prompt = args[position + 1];
        assert_eq!(prompt, format!("{}\n# Addendum marker", system_prompt()));
        assert!(prompt.starts_with(SYSTEM_PROMPT));
        assert!(prompt.contains("composer inside Sonara"));
        assert!(!args.iter().any(|arg| arg.contains("keep this off")));
        assert_eq!(run.stdin, "-- keep this off the command line");
        assert!(!args.contains(&"--bare"));
        // The deny list and the isolation are intact.
        let denied = args[args.iter().position(|a| *a == "--disallowedTools").unwrap() + 1];
        assert_eq!(denied, DENIED_TOOLS.join(","));
        assert!(args.contains(&"--strict-mcp-config"));
        assert!(args.contains(&"dontAsk"));
    }
}
