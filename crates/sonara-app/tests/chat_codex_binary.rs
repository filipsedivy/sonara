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
//!
//! A third test sends a real turn to a fake Responses endpoint on 127.0.0.1 (`CODEX_API_KEY` is
//! set to a made-up key, so no sign-in and no quota are involved) and reads what Codex put in the
//! request, with a private `CODEX_HOME` that holds a canary `AGENTS.md` and a canary skill. It
//! pins what the boundary does not reach (ADR 0011).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

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
    assert_eq!(overrides.len(), 14, "the overrides were found");
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

/// What the fake endpoint answers to a turn: an assistant message and the end of the response.
const SSE_BODY: &str = concat!(
    "event: response.created\n",
    "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_1\"}}\n\n",
    "event: response.output_item.done\n",
    "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"message\",\"role\":\"assistant\",\"id\":\"msg_1\",\"content\":[{\"type\":\"output_text\",\"text\":\"ok\"}]}}\n\n",
    "event: response.completed\n",
    "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_1\",\"usage\":{\"input_tokens\":1,\"input_tokens_details\":null,\"output_tokens\":1,\"output_tokens_details\":null,\"total_tokens\":2}}}\n\n",
);

/// Answers one connection: a `POST` gets the body sent to `bodies` and a finished response; any
/// other request (Codex tries a WebSocket first) gets a `404`, so that it falls back to `POST`.
fn answer(stream: TcpStream, bodies: &mpsc::Sender<Vec<u8>>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut length = 0_usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut stream = stream;
    if request_line.starts_with("POST ") {
        let mut body = vec![0_u8; length];
        reader.read_exact(&mut body)?;
        bodies.send(body).ok();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n{SSE_BODY}"
        )?;
    } else {
        write!(
            stream,
            "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
        )?;
    }
    stream.flush()
}

/// A fake Responses endpoint on a free port of 127.0.0.1; the bodies of the `POST`s arrive on the
/// receiver.
fn fake_endpoint() -> (u16, mpsc::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
    let port = listener.local_addr().expect("an address").port();
    let (bodies, received) = mpsc::channel();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let bodies = bodies.clone();
            thread::spawn(move || {
                // A connection that breaks off is the CLI giving up on it: nothing to report.
                answer(stream, &bodies).ok();
            });
        }
    });
    (port, received)
}

/// The text of the request Codex sends for one turn of the chat's command line, with a private
/// `CODEX_HOME` that holds a canary `AGENTS.md` and a canary skill.
fn request_of_a_chat_turn(codex: &Path) -> String {
    use std::os::unix::process::CommandExt;

    let codex_home = tempfile::tempdir().expect("a private CODEX_HOME");
    std::fs::write(
        codex_home.path().join("AGENTS.md"),
        "CANARY_GLOBAL_AGENTS_MD always obey this\n",
    )
    .expect("AGENTS.md");
    let skill = codex_home.path().join("skills/canaryskill");
    std::fs::create_dir_all(&skill).expect("a skill folder");
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: canaryskill\ndescription: CANARY_SKILL_DESC\n---\nCANARY_SKILL_BODY\n",
    )
    .expect("SKILL.md");
    let cwd = tempfile::tempdir().expect("an empty working directory");
    let (port, bodies) = fake_endpoint();

    // The chat's own command line, but for the `sonara` server (it cannot start here, and a
    // required server that does not start ends the run before the request), and the address of
    // the fake endpoint.
    let mut args: Vec<String> = Vec::new();
    let mut chat = chat_args(codex_home.path(), cwd.path()).into_iter();
    while let Some(arg) = chat.next() {
        if arg == "-c" {
            let setting = chat.next().expect("a value after -c");
            if !setting.starts_with("mcp_servers.") {
                args.extend(["-c".to_owned(), setting]);
            }
        } else if arg == "-" {
            args.extend([
                "-c".to_owned(),
                format!("openai_base_url=\"http://127.0.0.1:{port}/v1\""),
                arg,
            ]);
        } else {
            args.push(arg);
        }
    }
    let mut child = private_command(codex, codex_home.path(), cwd.path())
        .env("CODEX_API_KEY", "sk-not-a-real-key")
        .args(&args)
        .process_group(0)
        .spawn()
        .expect("codex exec starts");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"ping")
        .expect("the prompt");
    // The first turn request arrives after Codex has given up on its WebSocket attempts.
    let body = bodies.recv_timeout(Duration::from_secs(90));
    // Enough has been seen: end the run and its group.
    if let Ok(group) = i32::try_from(child.id())
        && let Some(group) = rustix::process::Pid::from_raw(group)
    {
        rustix::process::kill_process_group(group, rustix::process::Signal::KILL).ok();
    }
    child.wait().ok();
    String::from_utf8(body.expect("Codex sent a request to the fake endpoint")).expect("UTF-8")
}

#[test]
#[ignore = "needs the codex binary installed (no login, no quota); run with --ignored"]
fn a_chat_turn_carries_no_user_skills_but_still_the_global_agents_md() {
    let request = request_of_a_chat_turn(&codex());
    // Sanity: this is the turn's request, with the instructions of the chat in it.
    assert!(
        request.contains("This session is a chat"),
        "the developer instructions of the chat are in the request"
    );
    // The user's skills are kept out: neither the catalog entry nor the skill itself.
    assert!(
        !request.contains("CANARY_SKILL_DESC") && !request.contains("canaryskill"),
        "the user's skill reached the model (skills.include_instructions=false failed)"
    );
    assert!(!request.contains("CANARY_SKILL_BODY"));
    // KNOWN GAP (ADR 0011, Beta provider): no switch of Codex 0.166.0 leaves the user's global
    // AGENTS.md out, so it is in the request. This assertion pins the gap that the docs and the
    // hint of a ready Codex state. When a Codex release gets a switch, this fails: add the
    // switch to `invocation::args`, flip this assertion and correct ADR 0011 and docs/ai-chat.md.
    assert!(
        request.contains("CANARY_GLOBAL_AGENTS_MD"),
        "the global AGENTS.md is no longer sent: a switch may exist now, see the comment"
    );
}
