//! The `sonara` CLI: tool listing, single tool calls and batch workflows (the port of the
//! prototype's `tests/agent/test_cli.py`). Each test runs the built binary in a sandbox.
//!
//! The prototype checks the CLI's batch logic with `export_audio`
//! (`batch_with_references`, `batch_confirm_steps`); variants on other tools cover the same logic
//! (`batch_with_references_without_export`, `batch_confirm_steps_on_unsaved_changes`).
//! `render_exports_a_project` covers `sonara render`, which the prototype's suite did not test.

use serde_json::{Value, json};
use sonara_agent::handlers::builtin;
use sonara_agent::mcp::dumps;

use crate::support::Sandbox;

/// The names of the registered tools, in registration order (the prototype's
/// `registry.specs()`).
fn registered() -> Vec<String> {
    builtin()
        .specs()
        .iter()
        .map(|spec| spec.name.to_owned())
        .collect()
}

fn names(listing: &Value, key: &str) -> Vec<String> {
    listing
        .as_array()
        .expect("a list")
        .iter()
        .map(|item| item[key].as_str().expect("a string field").to_owned())
        .collect()
}

#[test]
fn tools_listing_and_schema() {
    let sandbox = Sandbox::new();
    let run = sandbox.run(&["tools"]);
    assert_eq!(run.code, 0);
    assert_eq!(names(&run.json, "name"), registered());
    // `_print`: `json.dumps(obj, indent=2)` and a newline, keys in the prototype's order.
    let expected: Vec<Value> = builtin()
        .specs()
        .iter()
        .map(|spec| {
            json!({
                "name": spec.name,
                "title": spec.title,
                "safety": spec.safety.as_str(),
                "description": spec.description(),
            })
        })
        .collect();
    assert_eq!(run.stdout, dumps(&Value::Array(expected), 2) + "\n");
    // One tool's contract entry (the prototype checks `export_audio`, see below).
    let run = sandbox.run(&["tools", "create_project"]);
    assert_eq!(run.code, 0);
    assert_eq!(run.json["safety"], "destructive");
    assert!(run.json["input_schema"]["properties"].get("bpm").is_some());
    let run = sandbox.run(&["tools", "no_such_tool"]);
    assert_eq!(run.code, 1);
    assert_eq!(run.json["ok"], false);
    assert_eq!(run.json["error"]["code"], "NOT_FOUND");
    assert!(run.json.get("tool").is_none());
}

#[test]
fn tools_listing_and_schema_of_export_audio() {
    let run = Sandbox::new().run(&["tools", "export_audio"]);
    assert_eq!(run.code, 0);
    assert_eq!(run.json["safety"], "destructive");
    assert!(
        run.json["input_schema"]["properties"]
            .get("formats")
            .is_some()
    );
}

#[test]
fn tool_call_in_process() {
    let sandbox = Sandbox::new();
    let run = sandbox.run(&[
        "tool",
        "create_project",
        r#"{"name": "CLI", "bpm": 99}"#,
        "--connect",
        "never",
    ]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.json["ok"], true);
    assert_eq!(run.json["state"]["tempo"][0]["bpm"], json!(99.0));
    // Each in-process call is a fresh session.
    let run = sandbox.run(&["tool", "list_tracks", "--connect", "never"]);
    assert_eq!(run.code, 1);
    assert_eq!(run.json["error"]["code"], "NO_PROJECT");
}

#[test]
fn tool_with_project_and_save() {
    let sandbox = Sandbox::new();
    let target = sandbox.root().join("p");
    let path_arg = format!("path={}", target.display());
    let run = sandbox.run(&[
        "tool",
        "create_project",
        "--connect",
        "never",
        "-a",
        "name=Persisted",
        "-a",
        &path_arg,
    ]);
    assert_eq!(run.code, 0, "{}", run.stdout);
    let project = run.json["path"].as_str().unwrap().to_owned();
    let run = sandbox.run(&[
        "tool",
        "create_track",
        "-a",
        "name=Bass Bus",
        "-a",
        "kind=bus",
        "--project",
        &project,
        "--save",
        "--connect",
        "never",
    ]);
    assert_eq!(run.code, 0, "{}", run.stdout);
    assert_eq!(run.json["saved"]["ok"], true);
    // `saved` is added last, after the tool's own keys.
    let keys: Vec<&String> = run.json.as_object().unwrap().keys().collect();
    assert_eq!(keys.last().map(|k| k.as_str()), Some("saved"));
    let run = sandbox.run(&[
        "tool",
        "list_tracks",
        "--project",
        &project,
        "--connect",
        "never",
    ]);
    assert_eq!(run.code, 0);
    assert_eq!(names(&run.json["tracks"], "name"), ["Bass Bus"]);
}

#[test]
fn tool_argument_errors() {
    let sandbox = Sandbox::new();
    let run = sandbox.run(&["tool", "create_project", "-a", "bpm"]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr.contains("-a expects key=value, got 'bpm'"),
        "{}",
        run.stderr
    );
    assert_eq!(run.stdout, "");
    let run = sandbox.run(&["tool", "create_project", "[1]", "--connect", "never"]);
    assert_eq!(run.code, 1);
    assert_eq!(run.stdout, "");
    // `-a` values are JSON when they parse, text otherwise; they override the JSON object.
    let run = sandbox.run(&[
        "tool",
        "create_project",
        r#"{"name": "A", "bpm": 90}"#,
        "-a",
        "bpm=101.5",
        "-a",
        "name=[x",
        "--connect",
        "never",
    ]);
    assert_eq!(run.code, 0, "{}", run.stdout);
    assert_eq!(run.json["state"]["name"], "[x");
    assert_eq!(run.json["state"]["tempo"][0]["bpm"], json!(101.5));
    // Usage errors exit with 2.
    assert_eq!(sandbox.run(&["tool"]).code, 2);
    assert_eq!(
        sandbox.run(&["tool", "x", "--connect", "sometimes"]).code,
        2
    );
}

#[test]
fn connect_always_needs_a_running_session() {
    // Attaching needs the IPC client; no session can be found until then.
    let run = Sandbox::new().run(&["tool", "list_tracks", "--connect", "always"]);
    assert_eq!(run.code, 1);
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr.contains("no running Sonara session found"),
        "{}",
        run.stderr
    );
}

fn write_steps(sandbox: &Sandbox, steps: &Value) -> String {
    let file = sandbox.root().join("steps.json");
    std::fs::write(&file, serde_json::to_vec(steps).expect("JSON steps")).expect("the steps file");
    file.to_str().expect("a UTF-8 sandbox path").to_owned()
}

/// The references batch of the prototype's test, without its last step when `export` is false.
fn references_batch(export: bool) -> Value {
    let mut steps = json!([
        {"id": "p", "tool": "create_project", "args": {"name": "Batch", "bpm": 120}},
        {"id": "drums", "tool": "create_track",
         "args": {"name": "Drums", "instrument": "sonara.drums"}},
        {"id": "gen", "tool": "generate_midi",
         "args": {"track_id": "{{drums.result.track_id}}", "kind": "drums", "bars": 1}},
        {"id": "ren", "tool": "rename_track",
         "args": {"track_id": "{{drums.result.track_id}}", "name": "Kit for {{p.state.name}}"}},
        {"id": "bad", "tool": "rename_track", "args": {"track_id": "trk_missing", "name": "x"},
         "allow_error": true},
        {"id": "st", "tool": "list_tracks", "args": {}},
    ]);
    if export {
        steps.as_array_mut().expect("a step list").push(json!(
            {"id": "exp", "tool": "export_audio", "args": {"path": "out/batch.wav", "tail": "none"}}
        ));
    }
    json!({"steps": steps})
}

#[test]
fn batch_with_references() {
    let sandbox = Sandbox::new();
    let file = write_steps(&sandbox, &references_batch(true));
    let full = sandbox.root().join("full.json");
    let run = sandbox.run(&[
        "batch",
        &file,
        "--connect",
        "never",
        "--results",
        full.to_str().unwrap(),
    ]);
    let by_id = |id: &str| {
        run.json["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == id)
            .unwrap()
            .clone()
    };
    let full: Value = serde_json::from_slice(&std::fs::read(&full).unwrap()).unwrap();
    assert_eq!(names(&full["st"]["tracks"], "name"), ["Kit for Batch"]);
    assert_eq!(by_id("bad")["ok"], false);
    assert_eq!(by_id("bad")["error"]["code"], "NOT_FOUND");
    assert_eq!(by_id("exp")["ok"], true);
    assert_eq!(run.code, 1);
    assert_eq!(run.json["ok"], false);
    assert!(sandbox.root().join("out/batch.wav").exists());
}

#[test]
fn batch_with_references_without_export() {
    let sandbox = Sandbox::new();
    let file = write_steps(&sandbox, &references_batch(false));
    let full = sandbox.root().join("full.json");
    let run = sandbox.run(&[
        "batch",
        &file,
        "--connect",
        "never",
        "--results",
        full.to_str().unwrap(),
    ]);
    let full_text = std::fs::read_to_string(&full).unwrap();
    let full: Value = serde_json::from_str(&full_text).unwrap();
    // `{{p.state.name}}` was substituted inside a string.
    assert_eq!(names(&full["st"]["tracks"], "name"), ["Kit for Batch"]);
    // `--results`: every envelope by step ID, as `json.dumps(full, indent=1)` and a newline.
    assert_eq!(full_text, dumps(&full, 1) + "\n");
    let ids: Vec<&str> = full
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(ids, ["p", "drums", "gen", "ren", "bad", "st"]);
    let summary: Vec<Value> = run.json["steps"].as_array().unwrap().clone();
    assert_eq!(summary.len(), 6);
    assert_eq!(summary[4]["ok"], false);
    assert_eq!(summary[4]["error"]["code"], "NOT_FOUND");
    assert_eq!(summary[4]["error"], full["bad"]["error"]);
    assert!(
        summary
            .iter()
            .enumerate()
            .all(|(i, s)| (i == 4) != (s["ok"] == true))
    );
    assert!(summary.iter().all(|s| {
        let keys: Vec<&String> = s.as_object().unwrap().keys().collect();
        keys == ["id", "tool", "ok"] || keys == ["id", "tool", "ok", "error"]
    }));
    // An allowed error still marks the batch as not fully ok.
    assert_eq!(run.code, 1);
    assert_eq!(run.json["ok"], false);
    // Progress on stderr, one line per step.
    let lines: Vec<&str> = run.stderr.lines().collect();
    assert_eq!(lines.len(), 6, "{}", run.stderr);
    assert_eq!(lines[0], "[p] create_project: ok");
    assert!(
        lines[4].starts_with("[bad] rename_track: ERROR NOT_FOUND: "),
        "{}",
        lines[4]
    );
}

#[test]
fn batch_stops_on_first_error() {
    let sandbox = Sandbox::new();
    let file = write_steps(
        &sandbox,
        &json!([
            {"tool": "create_project", "args": {"name": "Stop"}},
            {"tool": "set_tempo", "args": {"bpm": 5000}},
            {"tool": "set_tempo", "args": {"bpm": 100}},
        ]),
    );
    let run = sandbox.run(&["batch", &file, "--connect", "never"]);
    let oks: Vec<&Value> = run.json["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["ok"])
        .collect();
    assert_eq!(run.code, 1);
    assert_eq!(oks, [&json!(true), &json!(false)]);
    assert_eq!(run.json["steps"][1]["id"], "step1");
    let run = sandbox.run(&["batch", &file, "--connect", "never", "--keep-going"]);
    let oks: Vec<&Value> = run.json["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["ok"])
        .collect();
    assert_eq!(oks, [&json!(true), &json!(false), &json!(true)]);
}

#[test]
fn batch_failures_before_and_during_the_run() {
    let sandbox = Sandbox::new();
    // An empty batch is fully ok (`all([])`).
    let file = write_steps(&sandbox, &json!({"steps": []}));
    let run = sandbox.run(&["batch", &file, "--connect", "never"]);
    assert_eq!(
        (run.code, run.json.clone()),
        (0, json!({"steps": [], "ok": true}))
    );
    // A missing file and a reference to a missing step end without a summary.
    let run = sandbox.run(&["batch", "missing.json", "--connect", "never"]);
    assert_eq!(run.code, 1);
    assert_eq!(run.stdout, "");
    let file = write_steps(
        &sandbox,
        &json!([
            {"tool": "create_project", "args": {"name": "R"}},
            {"tool": "rename_track", "args": {"track_id": "{{nope.result.track_id}}", "name": "x"}},
        ]),
    );
    let run = sandbox.run(&["batch", &file, "--connect", "never"]);
    assert_eq!(run.code, 1);
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr.contains("[step0] create_project: ok"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr.contains("{{nope.result.track_id}}"),
        "{}",
        run.stderr
    );
}

#[test]
fn module_entry_point_subprocess() {
    let sandbox = Sandbox::new();
    let run = sandbox.run(&["tools"]);
    assert_eq!(run.code, 0);
    assert_eq!(run.json.as_array().unwrap().len(), registered().len());
    let run = sandbox.run(&["--version"]);
    assert_eq!(run.code, 0);
    assert_eq!(
        run.stdout,
        format!("sonara {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn batch_confirm_steps() {
    let sandbox = Sandbox::new();
    let export = json!({"path": "out/c.wav", "tail": "none", "overwrite": true});
    let file = write_steps(
        &sandbox,
        &json!([
            {"tool": "create_project", "args": {"name": "Confirm"}},
            {"id": "d", "tool": "create_track", "args": {"name": "D", "instrument": "sonara.drums"}},
            {"tool": "generate_midi",
             "args": {"track_id": "{{d.result.track_id}}", "kind": "drums", "bars": 1}},
            {"tool": "export_audio", "args": export},
            {"tool": "export_audio", "args": export, "confirm": true},
            {"tool": "export_audio", "args": export},
        ]),
    );
    let run = sandbox.run(&["batch", &file, "--connect", "never"]);
    let oks: Vec<&Value> = run.json["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["ok"])
        .collect();
    // The first export creates the file; the pre-approved overwrite succeeds; the unapproved
    // one stops.
    assert_eq!(
        oks,
        [
            &json!(true),
            &json!(true),
            &json!(true),
            &json!(true),
            &json!(true),
            &json!(false)
        ]
    );
    assert_eq!(
        run.json["steps"][5]["error"]["code"],
        "CONFIRMATION_REQUIRED"
    );
    assert_eq!(run.code, 1);
}

#[test]
fn batch_confirm_steps_on_unsaved_changes() {
    // The same pre-approval on a destructive tool that is ported: replacing a project with
    // unsaved changes.
    let sandbox = Sandbox::new();
    let replace = json!({"name": "Next"});
    let file = write_steps(
        &sandbox,
        &json!([
            {"tool": "create_project", "args": {"name": "Confirm"}},
            {"tool": "create_track", "args": {"name": "D", "instrument": "sonara.drums"}},
            {"tool": "create_project", "args": replace, "confirm": true},
            {"tool": "create_track", "args": {"name": "E"}},
            {"tool": "create_project", "args": replace},
        ]),
    );
    let run = sandbox.run(&["batch", &file, "--connect", "never"]);
    let oks: Vec<&Value> = run.json["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["ok"])
        .collect();
    assert_eq!(
        oks,
        [
            &json!(true),
            &json!(true),
            &json!(true),
            &json!(true),
            &json!(false)
        ]
    );
    assert_eq!(
        run.json["steps"][4]["error"]["code"],
        "CONFIRMATION_REQUIRED"
    );
    assert_eq!(run.code, 1);
}

#[test]
fn render_exports_a_project() {
    let sandbox = Sandbox::new();
    let project = sandbox.root().join("song.sonara");
    let project_arg = project.to_str().unwrap();
    let file = write_steps(
        &sandbox,
        &json!([
            {"tool": "create_project", "args": {"name": "Render Me", "bpm": 120}},
            {"id": "d", "tool": "create_track", "args": {"name": "D", "instrument": "sonara.drums"}},
            {"tool": "generate_midi",
             "args": {"track_id": "{{d.result.track_id}}", "kind": "drums", "bars": 1}},
            {"tool": "save_project", "args": {"path": project_arg}},
        ]),
    );
    let saved = sandbox.run(&["batch", &file, "--connect", "never"]);
    assert_eq!(saved.code, 0, "{}", saved.stderr);
    let out = sandbox.root().join("out").join("mix.wav");
    let out_arg = out.to_str().unwrap();
    let run = sandbox.run(&[
        "render",
        project_arg,
        "-o",
        out_arg,
        "-f",
        "wav",
        "-f",
        "flac",
    ]);
    assert_eq!(run.code, 0, "{}", run.stdout);
    assert_eq!(run.json["ok"], true);
    assert_eq!(run.json["status"], "succeeded");
    let outputs = run.json["result"]["outputs"].as_array().unwrap();
    assert_eq!(
        names(&run.json["result"]["outputs"], "container"),
        ["wav", "flac"]
    );
    for output in outputs {
        assert_eq!(output["bit_depth"], 24);
        assert_eq!(output["verified"], true);
        assert!(std::path::Path::new(output["path"].as_str().unwrap()).is_file());
    }
    assert!(sandbox.root().join("out").join("mix.flac").is_file());
    // An existing output: a conflict without --overwrite …
    let again = sandbox.run(&["render", project_arg, "-o", out_arg]);
    assert_eq!(again.code, 1);
    assert_eq!(again.json["error"]["code"], "CONFLICT");
    assert_eq!(again.json["error"]["details"]["existing"], json!([out_arg]));
    // … and with it, the command confirms the overwrite itself.
    let replaced = sandbox.run(&[
        "render",
        project_arg,
        "-o",
        out_arg,
        "--overwrite",
        "--normalize",
        "peak",
        "--target",
        "-3",
    ]);
    assert_eq!(replaced.code, 0, "{}", replaced.stdout);
    assert_eq!(
        replaced.json["result"]["normalization_target_reached"],
        true
    );
    let peak = replaced.json["result"]["outputs"][0]["path"]
        .as_str()
        .unwrap();
    let analyzed = sandbox.run(&["analyze", peak]);
    let sample_peak = analyzed.json["analysis"]["sample_peak_dbfs"]
        .as_f64()
        .unwrap();
    assert!((sample_peak + 3.0).abs() < 0.01, "{sample_peak}");
}
