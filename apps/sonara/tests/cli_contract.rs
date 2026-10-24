//! The CLI contract (CTR-01): the `clap` tree of `sonara`
//! equals the prototype's argparse tree in `golden/contracts/cli.json`, built the way the recorder
//! built it (`scripts/golden/area_contracts.py`, `_parser_tree`): every command with its `prog`
//! and `description`, and every argument with its option strings, `dest`, `nargs`, `required`,
//! `default`, `choices`, action class and `help`. Help arguments are left out, as the recorder
//! leaves them out.
//!
//! How `clap` settings read as argparse (see `sonara::cli`):
//!
//! - `description`: the root's `about`; a subcommand's `long_about` (`about` is its line in the
//!   parent's listing, argparse's `help`);
//! - actions: `Set` is `_StoreAction`, `SetTrue` `_StoreTrueAction`, `Append` `_AppendAction`,
//!   `Version` `_VersionAction`;
//! - `nargs`: `"0"` for flags, `"..."` for a trailing var-arg, `"?"` for an optional positional,
//!   otherwise `null`;
//! - defaults and choices take the type of the argument's value parser (`int`, `float` or `str`);
//!   flags default to `false`, `--version` to argparse's `==SUPPRESS==`.
//!
//! One thing the `clap` tree cannot show: an `append` option without values is `[]` in both
//! programs, except `render --format`, whose argparse default is `None` (`main` fills in
//! `["wav"]` afterwards). The Rust field is `Option<Vec<String>>`, which the tree does not
//! record, so [`NONE_DEFAULT_APPENDS`] names it.

use std::any::TypeId;

use clap::{Arg, ArgAction, Command};
use serde_json::{Map, Value, json};
use sonara_testkit::Golden;

/// `append` options whose argparse default is `None` instead of `[]`.
const NONE_DEFAULT_APPENDS: [(&str, &str); 1] = [("sonara render", "format")];

/// A default or choice text as the argparse value of the argument's type.
fn typed(arg: &Arg, text: &str) -> Value {
    let parser = arg.get_value_parser().type_id();
    if parser == TypeId::of::<i64>() {
        json!(text.parse::<i64>().expect("an integer value"))
    } else if parser == TypeId::of::<f64>() {
        json!(text.parse::<f64>().expect("a float value"))
    } else {
        json!(text)
    }
}

fn argument(prog: &str, arg: &Arg) -> Value {
    let id = arg.get_id().as_str();
    let mut options = Vec::new();
    if let Some(short) = arg.get_short() {
        options.push(format!("-{short}"));
    }
    if let Some(long) = arg.get_long() {
        options.push(format!("--{long}"));
    }
    let action = match arg.get_action() {
        ArgAction::Set => "_StoreAction",
        ArgAction::SetTrue => "_StoreTrueAction",
        ArgAction::Append => "_AppendAction",
        ArgAction::Version => "_VersionAction",
        other => panic!("{prog} {id}: no argparse action for {other:?}"),
    };
    let is_flag = matches!(arg.get_action(), ArgAction::SetTrue | ArgAction::Version);
    let nargs = if is_flag {
        json!("0")
    } else if arg.is_trailing_var_arg_set() {
        json!("...")
    } else if arg.is_positional() && !arg.is_required_set() {
        json!("?")
    } else {
        Value::Null
    };
    let default = match arg.get_action() {
        ArgAction::Version => json!("==SUPPRESS=="),
        ArgAction::SetTrue => json!(false),
        ArgAction::Append if NONE_DEFAULT_APPENDS.contains(&(prog, id)) => Value::Null,
        ArgAction::Append => json!([]),
        _ => arg
            .get_default_values()
            .first()
            .map_or(Value::Null, |text| {
                typed(arg, text.to_str().expect("a UTF-8 default"))
            }),
    };
    let choices: Vec<Value> = if is_flag {
        Vec::new()
    } else {
        arg.get_possible_values()
            .iter()
            .map(|value| typed(arg, value.get_name()))
            .collect()
    };
    json!({
        "options": options,
        "dest": id,
        "nargs": nargs,
        "required": arg.is_required_set(),
        "default": default,
        "choices": if choices.is_empty() { Value::Null } else { Value::Array(choices) },
        "action": action,
        "help": arg.get_help().map(ToString::to_string),
    })
}

fn tree(command: &Command, prog: &str, root: bool) -> Value {
    let description = if root {
        command.get_about()
    } else {
        command.get_long_about()
    };
    let arguments: Vec<Value> = command
        .get_arguments()
        .filter(|arg| !matches!(arg.get_action(), ArgAction::Help))
        .map(|arg| argument(prog, arg))
        .collect();
    let commands: Map<String, Value> = command
        .get_subcommands()
        .map(|sub| {
            let name = sub.get_name();
            (name.to_owned(), tree(sub, &format!("{prog} {name}"), false))
        })
        .collect();
    let mut out = Map::new();
    out.insert("prog".to_owned(), json!(prog));
    out.insert(
        "description".to_owned(),
        json!(description.map(ToString::to_string)),
    );
    out.insert("arguments".to_owned(), Value::Array(arguments));
    let has_commands = !commands.is_empty();
    out.insert("commands".to_owned(), Value::Object(commands));
    if has_commands {
        out.insert(
            "commands_required".to_owned(),
            json!(command.is_subcommand_required_set()),
        );
    }
    Value::Object(out)
}

/// Every difference between `expected` and `actual`, as JSON-pointer-like paths.
fn differences(expected: &Value, actual: &Value, path: &str, out: &mut Vec<String>) {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            for (key, value) in e {
                match a.get(key) {
                    Some(other) => differences(value, other, &format!("{path}/{key}"), out),
                    None => out.push(format!("{path}/{key}: missing")),
                }
            }
            for key in a.keys().filter(|key| !e.contains_key(*key)) {
                out.push(format!("{path}/{key}: unexpected {}", a[key]));
            }
        }
        (Value::Array(e), Value::Array(a)) if e.len() == a.len() => {
            for (index, (value, other)) in e.iter().zip(a).enumerate() {
                differences(value, other, &format!("{path}/{index}"), out);
            }
        }
        _ if expected == actual => {}
        _ => out.push(format!("{path}: expected {expected}, got {actual}")),
    }
}

#[test]
fn the_command_tree_equals_the_golden_argparse_tree() {
    let golden: Value = Golden::open()
        .and_then(|golden| golden.json("contracts/cli.json"))
        .unwrap();
    let mut command = sonara::cli::command();
    command.build();
    let actual = tree(&command, "sonara", true);
    let mut out = Vec::new();
    differences(&golden, &actual, "", &mut out);
    assert!(
        out.is_empty(),
        "{} differences:\n{}",
        out.len(),
        out.join("\n")
    );
    // The listing covers every command of the prototype, in its order.
    let names: Vec<&str> = command.get_subcommands().map(Command::get_name).collect();
    assert_eq!(
        names,
        [
            "gui", "serve", "mcp", "tools", "tool", "batch", "render", "info", "analyze",
            "devices", "verify"
        ]
    );
}

#[test]
fn the_mcp_tree_equals_the_prototypes_sonara_mcp_parser() {
    // `sonara.agent.mcp_server.main`'s parser (not recorded in the golden data; written out from
    // the prototype's source).
    let mut command = sonara::cli::mcp_command();
    command.build();
    let expected = json!({
        "prog": "sonara-mcp",
        "description": "Sonara MCP server (stdio)",
        "commands": {},
        "arguments": [
            {"options": ["--connect"], "dest": "connect", "nargs": null, "required": false,
             "default": "auto", "choices": ["auto", "always", "never"], "action": "_StoreAction",
             "help": "use a running Sonara session (auto: if one is found)"},
            {"options": ["--allow"], "dest": "allow", "nargs": null, "required": false,
             "default": [], "choices": null, "action": "_AppendAction",
             "help": "additional allowed path (repeatable)"},
            // argparse's default is `Path.cwd()` when the parser is built; Rust reads it when the
            // server starts.
            {"options": ["--base-dir"], "dest": "base_dir", "nargs": null, "required": false,
             "default": null, "choices": null, "action": "_StoreAction",
             "help": "directory relative paths resolve against"},
            {"options": ["--audio"], "dest": "audio", "nargs": null, "required": false,
             "default": null, "choices": ["auto", "native", "null"], "action": "_StoreAction",
             "help": "audio backend for headless mode"},
            {"options": ["--log-level"], "dest": "log_level", "nargs": null, "required": false,
             "default": "WARNING", "choices": null, "action": "_StoreAction", "help": null},
        ],
    });
    let mut out = Vec::new();
    differences(&expected, &tree(&command, "sonara-mcp", true), "", &mut out);
    assert!(out.is_empty(), "{}", out.join("\n"));
}
