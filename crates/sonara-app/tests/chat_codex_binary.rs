//! The Codex command line of the chat against the installed `codex` (local only, HW-01: a
//! machine with the CLI installed; no sign-in and no quota are needed).
//!
//! Run it after every Codex upgrade and every change of the boundary keys:
//! `cargo test -p sonara-app --test chat_codex_binary -- --ignored --nocapture`.
//! Set `CODEX_BIN` to test another executable than the first `codex` found.
//!
//! Every run uses a private, empty `CODEX_HOME`, so it never reads or writes `~/.codex` and cannot
//! be signed in; nothing here reaches the model. Two things are proved against the real CLI:
//!
//! 1. `codex debug prompt-input` (which renders what the model is offered) accepts the `-c`
//!    overrides of the chat (but those of the `sonara` server), hands the developer instructions back exactly as they were written
//!    (so the TOML escaping is right: `-c` falls back to a plain string for a value that does
//!    not parse, which would be silent), and offers none of the tools the boundary removes.
//! 2. `codex exec` with the whole command line (`--strict-config` included) gets through the
//!    loading of the configuration and fails only because the required `sonara` MCP server
//!    cannot start (its command is `/usr/bin/false`), before any turn.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;
use sonara_app::chat::providers::codex::{invocation, locate};
use sonara_app::composer::invocation::SYSTEM_PROMPT;

fn codex() -> PathBuf {
    if let Some(path) = std::env::var_os("CODEX_BIN") {
        return PathBuf::from(path);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let path_var = std::env::var_os("PATH");
    locate::candidates(home.as_deref(), path_var.as_deref())
        .into_iter()
        .find(|path| path.is_file())
        .expect("a `codex` executable (or CODEX_BIN)")
}

/// The arguments of the chat's run with a Sonara command that cannot start.
fn chat_args(home: &Path, cwd: &Path) -> Vec<String> {
    invocation::args(Path::new("/usr/bin/false"), cwd, home)
        .into_iter()
        .map(|arg| arg.into_string().expect("UTF-8 arguments"))
        .collect()
}

fn private_command(codex: &Path, codex_home: &Path, cwd: &Path) -> Command {
    let mut command = Command::new(codex);
    command
        .env("CODEX_HOME", codex_home)
        .env_remove("CODEX_API_KEY")
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn names_of_tools(input: &Value) -> Vec<String> {
    fn walk(tool: &Value, found: &mut Vec<String>) {
        if let Some(name) = tool["name"].as_str() {
            found.push(name.to_owned());
        }
        for nested in tool["tools"].as_array().into_iter().flatten() {
            walk(nested, found);
        }
    }
    let mut found = Vec::new();
    for item in input.as_array().into_iter().flatten() {
        for tool in item["tools"].as_array().into_iter().flatten() {
            walk(tool, &mut found);
        }
    }
    found
}

#[test]
#[ignore = "needs the codex binary installed (no login, no quota); run with --ignored"]
fn the_installed_codex_takes_the_boundary_and_reads_the_instructions_back() {
    let codex = codex();
    let codex_home = tempfile::tempdir().expect("a private CODEX_HOME");
    let cwd = tempfile::tempdir().expect("an empty working directory");
    let args = chat_args(codex_home.path(), cwd.path());

    // 1. `debug prompt-input` with the `-c` overrides of the chat.
    let mut command = private_command(&codex, codex_home.path(), cwd.path());
    command.args(["debug", "prompt-input"]);
    let mut overrides = Vec::new();
    // Without the `sonara` server: `debug prompt-input` starts a required server, and the one of
    // this test cannot start (part 2 covers those keys).
    for pair in args
        .windows(2)
        .filter(|pair| pair[0] == "-c" && !pair[1].starts_with("mcp_servers."))
    {
        command.args(["-c", &pair[1]]);
        overrides.push(pair[1].clone());
    }
    assert_eq!(overrides.len(), 13, "the overrides were found");
    let output = command.arg("ping").output().expect("codex runs");
    assert!(
        output.status.success(),
        "debug prompt-input failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let input: Value = serde_json::from_slice(&output.stdout).expect("JSON on stdout");

    // The developer instructions come back whole, with every escape undone.
    let instructions = invocation::developer_instructions();
    assert!(instructions.starts_with(SYSTEM_PROMPT));
    let messages: Vec<&str> = input
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| item["type"] == "message")
        .flat_map(|item| item["content"].as_array().into_iter().flatten())
        .filter_map(|part| part["text"].as_str())
        .collect();
    assert!(
        messages.iter().any(|text| *text == instructions),
        "the developer instructions were not handed back exactly ({} messages)",
        messages.len()
    );

    // The tools the boundary removes are not offered (the nested ones are in the description of
    // the code-mode `exec` tool).
    let tools = names_of_tools(&input);
    let description: String = input
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|item| item["tools"].as_array().into_iter().flatten())
        .flat_map(|namespace| namespace["tools"].as_array().into_iter().flatten())
        .filter_map(|tool| tool["description"].as_str())
        .collect();
    for gone in [
        "exec_command",
        "write_stdin",
        "view_image",
        "spawn_agent",
        "send_message",
        "followup_task",
    ] {
        assert!(
            !tools.iter().any(|name| name == gone),
            "{gone} is offered: {tools:?}"
        );
        assert!(
            !description.contains(&format!("### `{gone}`")),
            "{gone} is offered inside exec"
        );
    }
    eprintln!("tools offered at the top level: {tools:?}");
    eprintln!(
        "still offered inside exec (cannot be removed by configuration): {:?}",
        description
            .lines()
            .filter_map(|line| line.strip_prefix("### `")?.strip_suffix('`'))
            .collect::<Vec<_>>()
    );

    // 2. `exec` with the whole command line: the configuration loads, the required server does
    // not start, and the run ends before any event.
    let mut exec = private_command(&codex, codex_home.path(), cwd.path());
    let mut child = exec.args(&args).spawn().expect("codex exec starts");
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().expect("stdin");
        // The CLI may exit before it reads the prompt.
        stdin.write_all(b"ping").ok();
    }
    let output = child.wait_with_output().expect("codex exec ends");
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("exec stderr: {stderr}");
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        !stderr.contains("Error loading config"),
        "the configuration was rejected: {stderr}"
    );
    assert!(
        stderr.contains("required MCP servers failed to initialize"),
        "expected the required-server failure: {stderr}"
    );
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("\"turn.started\""),
        "no turn may start without the Sonara tools"
    );
}

#[test]
#[ignore = "needs the codex binary installed (no login, no quota); run with --ignored"]
fn the_installed_codex_rejects_an_unknown_key_under_strict_config() {
    // The reason `--strict-config` is in the command line: a renamed key must fail the run.
    let codex = codex();
    let codex_home = tempfile::tempdir().expect("a private CODEX_HOME");
    let cwd = tempfile::tempdir().expect("an empty working directory");
    let mut args = chat_args(codex_home.path(), cwd.path());
    let at = args.len() - 1;
    args.splice(
        at..at,
        ["-c".to_owned(), "features.shell_toool=false".to_owned()],
    );
    let mut child = private_command(&codex, codex_home.path(), cwd.path())
        .args(&args)
        .spawn()
        .expect("codex exec starts");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("codex exec ends");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("unknown configuration field"),
        "expected the strict-config error: {stderr}"
    );
}
