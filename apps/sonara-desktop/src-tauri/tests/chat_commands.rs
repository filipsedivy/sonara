//! The AI Chat's window and the desktop agree on the commands (TST-05, TST-10): every command the
//! window's TypeScript client invokes is registered in `generate_handler!`, takes the arguments
//! the client sends (Tauri maps a camelCase argument of the page to the `snake_case` parameter of
//! the command), and is answered by the page tests' mock; the event, the window's label and its
//! capability match as well. A rename on one side alone fails here instead of silently breaking
//! the window, which the page tests (mocked commands) and the Rust tests (no window) cannot see.
//!
//! The sources are read as text: the desktop crate is a binary with a window system, so the
//! lists cannot be asked of the running application.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

fn desktop() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read(relative: &str) -> String {
    let path = desktop().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// `snake_case` as the camelCase argument name Tauri expects from the page.
fn camel(name: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for ch in name.chars() {
        if ch == '_' {
            upper = true;
        } else if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// The command names registered in `generate_handler!`: the `chat::name,` lines of main.rs.
fn registered() -> BTreeSet<String> {
    read("src-tauri/src/main.rs")
        .lines()
        .filter_map(|line| line.trim().strip_prefix("chat::"))
        .filter_map(|rest| rest.strip_suffix(','))
        .map(str::to_owned)
        .collect()
}

/// The commands of chat.rs with the page-facing argument names of their parameters, State and
/// `AppHandle` (which the page never sends) left out.
fn defined() -> BTreeMap<String, BTreeSet<String>> {
    let source = read("src-tauri/src/chat.rs");
    let mut commands = BTreeMap::new();
    let mut rest = source.as_str();
    while let Some(at) = rest.find("#[tauri::command]") {
        rest = &rest[at + "#[tauri::command]".len()..];
        let fn_at = rest.find("fn ").expect("a function follows the attribute");
        let after = &rest[fn_at + 3..];
        let open = after.find('(').expect("a parameter list");
        let name = after[..open].trim().to_owned();
        // The parameter list ends at the parenthesis that closes the first one.
        let mut depth = 0_usize;
        let mut end = open;
        for (index, ch) in after.char_indices().skip(open) {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = index;
                        break;
                    }
                }
                _ => {}
            }
        }
        let params = &after[open + 1..end];
        // Split at the commas outside generics, so that `State<'_, ChatHost>` stays whole.
        let mut parts = Vec::new();
        let mut nesting = 0_i32;
        let mut start = 0;
        for (index, ch) in params.char_indices() {
            match ch {
                '<' | '(' | '[' => nesting += 1,
                '>' | ')' | ']' => nesting -= 1,
                ',' if nesting == 0 => {
                    parts.push(&params[start..index]);
                    start = index + 1;
                }
                _ => {}
            }
        }
        parts.push(&params[start..]);
        let args = parts
            .into_iter()
            .filter_map(|part| part.split_once(':'))
            .filter(|(_, ty)| !ty.contains("State<") && !ty.contains("AppHandle"))
            .map(|(param, _)| camel(param.trim()))
            .collect();
        commands.insert(name, args);
    }
    commands
}

/// The commands the window's client invokes, with the argument names it sends: every
/// `invoke<…>("chat_…", { … })` of `src/lib/chat/api.ts`.
fn invoked() -> BTreeMap<String, BTreeSet<String>> {
    let mut commands = BTreeMap::new();
    for line in read("src/lib/chat/api.ts").lines() {
        let Some(at) = line.find("invoke<") else {
            continue;
        };
        let call = &line[at..];
        let Some(quote) = call.find("(\"") else {
            continue;
        };
        let call = &call[quote + 2..];
        let name = call[..call.find('"').expect("a closing quote")].to_owned();
        let args = call
            .split_once('{')
            .and_then(|(_, tail)| tail.split_once('}'))
            .map(|(list, _)| {
                list.split(',')
                    .map(|arg| arg.split(':').next().unwrap_or_default().trim().to_owned())
                    .filter(|arg| !arg.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        commands.insert(name, args);
    }
    commands
}

/// The commands the chat page tests answer: the `chat_…:` keys of `tests/page/chat.spec.ts`.
fn mocked() -> BTreeSet<String> {
    read("tests/page/chat.spec.ts")
        .lines()
        .filter_map(|line| {
            let name = line.trim().strip_prefix("chat_")?;
            let (name, _) = name.split_once(':')?;
            name.chars()
                .all(|ch| ch.is_ascii_lowercase() || ch == '_')
                .then(|| format!("chat_{name}"))
        })
        .collect()
}

#[test]
fn every_command_the_window_invokes_is_registered_and_defined() {
    let invoked = invoked();
    assert!(
        invoked.len() >= 10,
        "the client's invokes were found: {invoked:?}"
    );
    let registered = registered();
    let defined = defined();
    for name in invoked.keys() {
        assert!(
            registered.contains(name),
            "{name} is invoked by api.ts but not in generate_handler! (main.rs)"
        );
        assert!(
            defined.contains_key(name),
            "{name} is invoked by api.ts but no #[tauri::command] of chat.rs has that name"
        );
    }
}

#[test]
fn every_registered_chat_command_is_called_by_the_window() {
    let invoked = invoked();
    let registered = registered();
    let defined = defined();
    assert_eq!(
        registered,
        defined.keys().cloned().collect::<BTreeSet<_>>(),
        "main.rs registers exactly the commands chat.rs defines"
    );
    for name in &registered {
        assert!(
            invoked.contains_key(name),
            "{name} is registered but the window never invokes it"
        );
    }
}

#[test]
fn the_window_sends_the_arguments_the_commands_take() {
    let defined = defined();
    for (name, sent) in invoked() {
        assert_eq!(
            Some(&sent),
            defined.get(&name),
            "{name}: the arguments of api.ts (left) and the parameters of the command (right)"
        );
    }
    // The one that needs the conversion: `toIndex` reaches `to_index`.
    assert_eq!(
        defined.get("chat_move"),
        Some(&BTreeSet::from(["id".to_owned(), "toIndex".to_owned()]))
    );
}

#[test]
fn the_page_tests_answer_every_command_the_window_invokes() {
    let mocked = mocked();
    for name in invoked().keys() {
        assert!(
            mocked.contains(name),
            "{name} is invoked by api.ts but tests/page/chat.spec.ts does not answer it"
        );
    }
    for name in &mocked {
        assert!(
            invoked().contains_key(name),
            "{name} is answered by tests/page/chat.spec.ts but api.ts never invokes it"
        );
    }
}

#[test]
fn the_event_the_window_listens_to_and_its_label_match_the_desktop() {
    let chat = read("src-tauri/src/chat.rs");
    assert!(
        chat.contains("const EVENT: &str = \"chat-event\";"),
        "the event the desktop emits"
    );
    assert!(
        read("src/lib/chat/api.ts").contains("listen<ChatEvent>(\"chat-event\""),
        "the event the window listens to"
    );
    assert!(
        read("src-tauri/src/window.rs").contains("pub const CHAT: &str = \"chat\";"),
        "the window's label"
    );
    assert!(
        read("src/main.ts").contains("label === \"chat\" ? ChatWindow"),
        "the page mounts the chat for that label"
    );
    let capability: serde_json::Value =
        serde_json::from_str(&read("src-tauri/capabilities/default.json")).expect("JSON");
    let windows: Vec<&str> = capability["windows"]
        .as_array()
        .expect("the windows list")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(
        windows.contains(&"chat"),
        "the chat window may listen (capabilities/default.json): {windows:?}"
    );
}

#[test]
fn the_conversion_of_argument_names_is_tauris() {
    assert_eq!(camel("to_index"), "toIndex");
    assert_eq!(camel("id"), "id");
    assert_eq!(camel("a_b_c"), "aBC");
}
