//! The command line of a Codex chat run: `codex exec --json` with every tool source but the
//! Sonara MCP server switched off.
//!
//! Codex has no single "only these tools" switch, so the boundary is a list of keys, each of
//! which was read from the source of rust-v0.166.0 and checked against the installed 0.166.0
//! (`codex debug prompt-input` lists the tools the model is offered; `--strict-config` rejects a
//! key this version does not know, so a rename fails the run loudly instead of silently
//! reopening a tool). The Codex column of the boundary table of ADR 0011 is what this builds:
//!
//! - **No shell:** `features.shell_tool=false` (no shell or unified-exec tool), `-s read-only`,
//!   and the approval policy `never` that `codex exec` forces.
//! - **No file write:** `-s read-only`. `apply_patch` cannot be removed by configuration: the
//!   model still sees it and Codex rejects the call (a `file_change` item with status `failed`).
//! - **No web:** `web_search="disabled"`.
//! - **No other tool source:** `features.apps`, `plugins`, `multi_agent`, `view_image`, `hooks`,
//!   `memories`, `tool_suggest`, `request_permissions_tool` off; `agents.enabled=false`
//!   (measured: with only `features.multi_agent=false` the default model is still offered the
//!   sub-agent tools of `multi_agent_v2`); `--ignore-user-config` and `--ignore-rules`.
//! - **Only our MCP server:** `mcp_servers.sonara` with `required=true` (a server that does not
//!   start aborts the run before any event), `enabled_tools` (every tool of the Agent API that is
//!   not denied) and, again, `disabled_tools` ([`DENIED_SONARA_TOOLS`]).
//! - **No prompts or escalation:** `approvals_reviewer="user"`, and never any `--yolo`,
//!   `--dangerously-bypass-*`, `--approve-for-me`, `--full-auto`, `--oss`, `--add-dir` or `-m`.
//! - **No kept session:** `--ephemeral`. The conversation is replayed in the prompt instead of
//!   resumed.
//!
//! Every `-c` value travels as one argument of its own (no shell is involved) and is a TOML
//! value: `-c` parses its value as TOML and silently falls back to a plain string when that
//! fails, so a badly escaped string would not be an error but a different value. [`toml_string`]
//! is the one place that writes a string.
//!
//! The prompt goes through stdin (`-`), never through the arguments.

use std::ffi::OsString;
use std::path::Path;

use super::tools::allowed_sonara_tools;
use crate::chat::boundary::DENIED_SONARA_TOOLS;
use crate::chat::provider::{SYSTEM_PROMPT_ADDENDUM, Turn};
use crate::composer::invocation::{SYSTEM_PROMPT, search_path};
use crate::composer::{Invocation, Workspace};

/// The name of the Sonara MCP server in `-c mcp_servers.<name>`.
const SERVER: &str = "sonara";

/// How long Codex waits for `sonara mcp` to start, in seconds.
const STARTUP_TIMEOUT_SEC: u32 = 30;

/// The features that are off in a chat run (`-c features.<name>=false`).
pub const DISABLED_FEATURES: [&str; 9] = [
    "shell_tool",
    "apps",
    "plugins",
    "multi_agent",
    "view_image",
    "hooks",
    "memories",
    "tool_suggest",
    "request_permissions_tool",
];

/// `text` as a TOML basic string, with its quotes.
///
/// `"` and `\` are escaped, the control characters have their short escapes (`\n`, `\t`, `\r`,
/// `\b`, `\f`) or `\u00XX`, and so has DEL (U+007F), which TOML does not allow raw (a JSON
/// encoder leaves it alone). Everything else, including characters outside the Basic
/// Multilingual Plane, is written as it is.
#[must_use]
pub fn toml_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            control if control.is_control() && control.is_ascii() => {
                // `\u00XX`: the code is below 0x80, so two hex digits are enough.
                out.push_str("\\u00");
                let code = u32::from(control);
                out.extend(
                    [code >> 4, code & 0xf]
                        .into_iter()
                        .filter_map(|nibble| char::from_digit(nibble, 16)),
                );
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// A TOML array of strings.
fn toml_strings<'a>(items: impl IntoIterator<Item = &'a str>) -> String {
    let items: Vec<String> = items.into_iter().map(toml_string).collect();
    format!("[{}]", items.join(","))
}

/// A path as a TOML string. A path that is not UTF-8 is written lossily, which makes the server
/// fail to start, and `required=true` turns that into a failed run.
fn toml_path(path: &Path) -> String {
    toml_string(&path.to_string_lossy())
}

/// What the agent is told on top of Codex's own instructions: the composer's system prompt and
/// the chat's addendum, the same text Claude Code gets.
#[must_use]
pub fn developer_instructions() -> String {
    format!("{SYSTEM_PROMPT}\n{SYSTEM_PROMPT_ADDENDUM}")
}

/// The `-c key=value` overrides of a run, as `key=value` texts, in the order they are passed.
fn overrides(sonara_cli: &Path, cwd: &Path, home: &Path) -> Vec<String> {
    let mut keys: Vec<String> = DISABLED_FEATURES
        .iter()
        .map(|feature| format!("features.{feature}=false"))
        .collect();
    keys.extend([
        "agents.enabled=false".to_owned(),
        format!("web_search={}", toml_string("disabled")),
        format!("approvals_reviewer={}", toml_string("user")),
        format!(
            "developer_instructions={}",
            toml_string(&developer_instructions())
        ),
        format!("mcp_servers.{SERVER}.command={}", toml_path(sonara_cli)),
        format!(
            "mcp_servers.{SERVER}.args=[{}]",
            [
                toml_string("mcp"),
                toml_string("--connect"),
                toml_string("always"),
                toml_string("--base-dir"),
                toml_path(cwd),
                toml_string("--log-level"),
                toml_string("WARNING"),
            ]
            .join(",")
        ),
        format!(
            "mcp_servers.{SERVER}.env={{SONARA_HOME={}}}",
            toml_path(home)
        ),
        format!("mcp_servers.{SERVER}.required=true"),
        format!(
            "mcp_servers.{SERVER}.enabled_tools={}",
            toml_strings(allowed_sonara_tools())
        ),
        format!(
            "mcp_servers.{SERVER}.disabled_tools={}",
            toml_strings(DENIED_SONARA_TOOLS)
        ),
        format!("mcp_servers.{SERVER}.startup_timeout_sec={STARTUP_TIMEOUT_SEC}"),
    ]);
    keys
}

/// The arguments of `codex`, without the program: `exec`, the flags of the boundary, the `-c`
/// overrides and `-` (the prompt is read from stdin).
#[must_use]
pub fn args(sonara_cli: &Path, cwd: &Path, home: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "exec",
        "--json",
        "--ephemeral",
        "--ignore-user-config",
        "--ignore-rules",
        "--strict-config",
        "--skip-git-repo-check",
        "--color",
        "never",
        "-s",
        "read-only",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    for text in overrides(sonara_cli, cwd, home) {
        args.push("-c".into());
        args.push(text.into());
    }
    args.push("-".into());
    args
}

/// The run of `turn` in `workspace` with the CLI at `codex`.
///
/// The environment adds only a `PATH` that starts with the directory of `codex` (the rest is
/// inherited, which keeps `CODEX_HOME` and `CODEX_API_KEY` for the CLI's own sign-in); the
/// working directory is the workspace's empty folder.
#[must_use]
pub fn build(codex: &Path, sonara_cli: &Path, workspace: &Workspace, turn: &Turn) -> Invocation {
    let cwd = workspace.cwd();
    Invocation {
        program: codex.to_path_buf(),
        args: args(sonara_cli, &cwd, &workspace.home()),
        cwd,
        env: vec![(
            OsString::from("PATH"),
            search_path(codex, std::env::var_os("PATH").as_deref()),
        )],
        stdin: turn.stdin_text(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    /// A reader of the TOML basic strings that [`toml_string`] writes: the oracle for the escaper
    /// in these tests. (The installed `codex` is the real oracle, in `tests/chat_codex_binary.rs`.)
    fn unescape(literal: &str) -> String {
        let inner = literal
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .expect("quoted");
        let mut out = String::new();
        let mut chars = inner.chars();
        while let Some(character) = chars.next() {
            assert_ne!(
                character, '"',
                "an unescaped quote ends the string: {literal}"
            );
            if character != '\\' {
                assert!(
                    !(character.is_ascii_control() && character != '\t'),
                    "a raw control character in {literal:?}"
                );
                out.push(character);
                continue;
            }
            match chars.next().expect("an escape") {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'b' => out.push('\u{8}'),
                'f' => out.push('\u{c}'),
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    out.push(
                        char::from_u32(u32::from_str_radix(&hex, 16).expect("hex"))
                            .expect("a scalar value"),
                    );
                }
                other => panic!("an escape TOML does not know: \\{other}"),
            }
        }
        out
    }

    #[test]
    fn a_toml_string_survives_quotes_backslashes_newlines_and_control_characters() {
        let tricky = "plain \"quoted\" back\\slash \\\" \\n\nline\r\n\ttab \u{8}\u{c}\u{0}\u{1b}[0m \u{7f} \
                      accent á ž 🎵 \u{2028} end\\";
        let literal = toml_string(tricky);
        assert_eq!(unescape(&literal), tricky);
        // DEL and NUL are not written raw: TOML rejects them, and a JSON encoder leaves DEL.
        assert!(!literal.contains('\u{7f}') && !literal.contains('\u{0}'));
        assert!(
            literal.contains("\\u007f") && literal.contains("\\u0000"),
            "{literal}"
        );
        assert!(literal.contains("\\u001b"));
        assert!(
            !literal.contains('\n') && !literal.contains('\r'),
            "one line"
        );
        assert_eq!(toml_string(""), "\"\"");
        assert_eq!(toml_string("a\"b"), r#""a\"b""#);
        assert_eq!(toml_string("a\\b"), r#""a\\b""#);
    }

    #[test]
    fn the_real_instructions_round_trip_through_the_escaper() {
        let text = developer_instructions();
        assert!(text.contains("composer inside Sonara"));
        assert!(text.contains("This session is a chat"));
        assert_eq!(unescape(&toml_string(&text)), text);
    }

    #[test]
    fn arrays_of_strings_are_toml_arrays() {
        assert_eq!(toml_strings(["a", "b\"c"]), r#"["a","b\"c"]"#);
        assert_eq!(toml_strings([]), "[]");
    }

    fn paths() -> (PathBuf, PathBuf, PathBuf) {
        (
            PathBuf::from("/App/Sonara.app/Contents/MacOS/sonara"),
            PathBuf::from("/tmp/sonara-composer-1-0/cwd"),
            PathBuf::from("/tmp/sonara-composer-1-0/home"),
        )
    }

    fn texts(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|arg| arg.to_str().expect("UTF-8").to_owned())
            .collect()
    }

    /// The `key` and the value of each `-c key=value` pair.
    fn overrides_of(args: &[String]) -> Vec<(String, String)> {
        args.windows(2)
            .filter(|pair| pair[0] == "-c")
            .map(|pair| {
                let (key, value) = pair[1].split_once('=').expect("key=value");
                (key.to_owned(), value.to_owned())
            })
            .collect()
    }

    fn value<'a>(overrides: &'a [(String, String)], key: &str) -> &'a str {
        overrides
            .iter()
            .find(|(name, _)| name == key)
            .unwrap_or_else(|| panic!("no -c {key}"))
            .1
            .as_str()
    }

    #[test]
    fn the_command_line_is_the_codex_column_of_the_boundary_table_literally() {
        let (cli, cwd, home) = paths();
        let args = texts(&args(&cli, &cwd, &home));
        let mut expected: Vec<String> = [
            "exec",
            "--json",
            "--ephemeral",
            "--ignore-user-config",
            "--ignore-rules",
            "--strict-config",
            "--skip-git-repo-check",
            "--color",
            "never",
            "-s",
            "read-only",
        ]
        .map(str::to_owned)
        .to_vec();
        let mut config = |text: String| {
            expected.push("-c".to_owned());
            expected.push(text);
        };
        for feature in [
            "shell_tool",
            "apps",
            "plugins",
            "multi_agent",
            "view_image",
            "hooks",
            "memories",
            "tool_suggest",
            "request_permissions_tool",
        ] {
            config(format!("features.{feature}=false"));
        }
        config("agents.enabled=false".to_owned());
        config(r#"web_search="disabled""#.to_owned());
        config(r#"approvals_reviewer="user""#.to_owned());
        config(format!(
            "developer_instructions={}",
            toml_string(&format!("{SYSTEM_PROMPT}\n{SYSTEM_PROMPT_ADDENDUM}"))
        ));
        config(r#"mcp_servers.sonara.command="/App/Sonara.app/Contents/MacOS/sonara""#.to_owned());
        config(
            r#"mcp_servers.sonara.args=["mcp","--connect","always","--base-dir","/tmp/sonara-composer-1-0/cwd","--log-level","WARNING"]"#
                .to_owned(),
        );
        config(
            r#"mcp_servers.sonara.env={SONARA_HOME="/tmp/sonara-composer-1-0/home"}"#.to_owned(),
        );
        config("mcp_servers.sonara.required=true".to_owned());
        config(format!(
            "mcp_servers.sonara.enabled_tools={}",
            toml_strings(allowed_sonara_tools())
        ));
        config(format!(
            "mcp_servers.sonara.disabled_tools={}",
            toml_strings(DENIED_SONARA_TOOLS)
        ));
        config("mcp_servers.sonara.startup_timeout_sec=30".to_owned());
        expected.push("-".to_owned());
        assert_eq!(args, expected);
    }

    #[test]
    fn the_enabled_and_denied_tools_in_the_arguments_are_disjoint_and_make_up_every_tool() {
        let (cli, cwd, home) = paths();
        let args = texts(&args(&cli, &cwd, &home));
        let overrides = overrides_of(&args);
        let names = |key: &str| -> Vec<String> {
            let array = value(&overrides, key);
            array
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
                .expect("an array")
                .split(',')
                .filter(|item| !item.is_empty())
                .map(|item| item.trim_matches('"').to_owned())
                .collect()
        };
        let enabled = names("mcp_servers.sonara.enabled_tools");
        let disabled = names("mcp_servers.sonara.disabled_tools");
        assert!(enabled.iter().all(|name| !disabled.contains(name)));
        assert_eq!(disabled.len(), 13);
        let mut union: Vec<String> = enabled.iter().chain(&disabled).cloned().collect();
        union.sort();
        let mut all: Vec<String> = super::super::tools::ALL_SONARA_TOOLS
            .map(str::to_owned)
            .to_vec();
        all.sort();
        assert_eq!(union, all);
        assert!(!enabled.contains(&"save_project".to_owned()));
        assert!(disabled.contains(&"save_project".to_owned()));
    }

    #[test]
    fn nothing_that_widens_the_boundary_can_appear_in_the_arguments() {
        // Flags that approve, bypass, widen the sandbox, add a model or a profile, resume a
        // session or write the answer to a file.
        const FORBIDDEN: [&str; 26] = [
            "--yolo",
            "--dangerously-bypass-approvals-and-sandbox",
            "--dangerously-bypass-hook-trust",
            "--approve-for-me",
            "--not-so-yolo",
            "--full-auto",
            "-a",
            "--ask-for-approval",
            "--oss",
            "--local-provider",
            "--add-dir",
            "-m",
            "--model",
            "-p",
            "--profile",
            "-C",
            "--cd",
            "--worktree",
            "-i",
            "--image",
            "-o",
            "--output-last-message",
            "--output-schema",
            "resume",
            "fork",
            "review",
        ];
        let (cli, cwd, home) = paths();
        let args = texts(&args(&cli, &cwd, &home));
        for flag in FORBIDDEN {
            assert!(
                !args.iter().any(|arg| arg == flag),
                "{flag} must never appear"
            );
        }
        for arg in &args {
            assert!(
                !arg.starts_with("--dangerously") && !arg.starts_with("--enable"),
                "{arg}"
            );
            assert!(!arg.starts_with("--disable"), "{arg}");
        }
        // The sandbox is read-only, once, and nothing else names a sandbox or an approval.
        let sandbox: Vec<&String> = args
            .iter()
            .enumerate()
            .filter(|(_, arg)| *arg == "-s" || *arg == "--sandbox")
            .map(|(index, _)| &args[index + 1])
            .collect();
        assert_eq!(sandbox, ["read-only"]);
        // The `-c` keys are a closed set.
        let keys: Vec<String> = overrides_of(&args)
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        let allowed: Vec<String> = DISABLED_FEATURES
            .iter()
            .map(|feature| format!("features.{feature}"))
            .chain(
                [
                    "agents.enabled",
                    "web_search",
                    "approvals_reviewer",
                    "developer_instructions",
                    "mcp_servers.sonara.command",
                    "mcp_servers.sonara.args",
                    "mcp_servers.sonara.env",
                    "mcp_servers.sonara.required",
                    "mcp_servers.sonara.enabled_tools",
                    "mcp_servers.sonara.disabled_tools",
                    "mcp_servers.sonara.startup_timeout_sec",
                ]
                .map(str::to_owned),
            )
            .collect();
        assert_eq!(
            keys, allowed,
            "no key outside the boundary's set, none missing"
        );
        for key in &keys {
            for bad in [
                "default_tools_approval_mode",
                "approval_policy",
                "sandbox",
                "shell_environment_policy",
                "model",
                "approval_mode",
            ] {
                assert!(!key.contains(bad), "{key} contains {bad}");
            }
        }
        // Every feature is switched off and never on.
        for (key, setting) in overrides_of(&args) {
            if key.starts_with("features.") {
                assert_eq!(setting, "false", "{key}");
            }
        }
    }

    #[test]
    fn the_sonara_server_is_pinned_to_the_workspace_and_must_start() {
        let (cli, cwd, home) = paths();
        let args = texts(&args(&cli, &cwd, &home));
        let overrides = overrides_of(&args);
        let server_args = value(&overrides, "mcp_servers.sonara.args");
        assert!(
            server_args.contains(r#""--connect","always""#),
            "{server_args}"
        );
        assert!(server_args.contains(r#""--base-dir","/tmp/sonara-composer-1-0/cwd""#));
        assert_eq!(value(&overrides, "mcp_servers.sonara.required"), "true");
        assert_eq!(value(&overrides, "web_search"), r#""disabled""#);
        assert_eq!(value(&overrides, "approvals_reviewer"), r#""user""#);
    }

    #[test]
    fn a_path_with_quotes_and_backslashes_stays_one_toml_string() {
        let cli = Path::new("/odd \"dir\"/back\\slash/sonara");
        let args = texts(&args(cli, Path::new("/c"), Path::new("/h")));
        let overrides = overrides_of(&args);
        let command = value(&overrides, "mcp_servers.sonara.command");
        assert_eq!(unescape(command), "/odd \"dir\"/back\\slash/sonara");
    }

    #[test]
    fn the_prompt_goes_through_stdin_and_never_through_the_arguments() {
        let source = tempfile::tempdir().expect("a directory");
        let file = source.path().join("session-7.json");
        fs::write(&file, "{}").expect("a discovery file");
        let workspace = Workspace::create(&file).expect("a workspace");
        let turn = Turn {
            prompt: "--yolo zebra-prompt-71 with \"quotes\"".into(),
            transcript: "User: giraffe-history-93\n\nAssistant: ok".into(),
        };
        let run = build(
            Path::new("/opt/homebrew/bin/codex"),
            Path::new("/App/sonara"),
            &workspace,
            &turn,
        );
        assert_eq!(run.program, Path::new("/opt/homebrew/bin/codex"));
        assert_eq!(run.cwd, workspace.cwd());
        assert_eq!(run.stdin, turn.stdin_text());
        assert!(run.stdin.contains("--yolo zebra-prompt-71"));
        assert!(run.stdin.contains("giraffe-history-93"));
        let args = texts(&run.args);
        assert_eq!(
            args.last().map(String::as_str),
            Some("-"),
            "stdin marker last"
        );
        assert!(
            !args
                .iter()
                .any(|arg| arg.contains("zebra-prompt-71") || arg.contains("giraffe-history-93")),
            "the prompt is not an argument"
        );
        assert!(
            args.iter()
                .any(|arg| arg.contains(&workspace.home().display().to_string()))
        );
        // The PATH starts with the directory of the CLI; nothing else is added.
        assert_eq!(run.env.len(), 1);
        let (name, path) = &run.env[0];
        assert_eq!(name, "PATH");
        assert_eq!(
            std::env::split_paths(path).next(),
            Some(PathBuf::from("/opt/homebrew/bin"))
        );
        // No credential or home of the CLI is set or overridden here.
        assert!(
            run.env
                .iter()
                .all(|(name, _)| name != "CODEX_HOME" && name != "CODEX_API_KEY")
        );
    }
}
