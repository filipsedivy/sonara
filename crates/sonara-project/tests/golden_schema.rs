//! Schema validation against the prototype: the recorded verdicts
//! (`golden/contracts/schema-verdicts.json`), every recorded project document, and further
//! verdicts of python-jsonschema 4.26 (`fixtures/schema-parity.json`, written by the prototype's
//! `validate_document`).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sonara_core::error::ErrorCode;
use sonara_project::schema::{
    MANIFEST_SCHEMA_TEXT, PROJECT_SCHEMA_TEXT, Schema, validate_document,
};
use sonara_testkit::Golden;

fn golden() -> Golden {
    Golden::open().expect("golden data is available")
}

fn schema_named(name: &str) -> Schema {
    match name {
        "project-v1.schema.json" => Schema::Project,
        "manifest-v1.schema.json" => Schema::Manifest,
        other => panic!("unknown schema {other}"),
    }
}

/// The verdict as the prototype records it: `None` when valid, else code, message and count.
fn verdict(doc: &Value, schema: Schema) -> Option<(String, String, u64)> {
    validate_document(doc, schema).err().map(|err| {
        let count = err.details["error_count"].as_u64().expect("error_count");
        (err.code.as_str().to_owned(), err.message, count)
    })
}

#[test]
fn embedded_schemas_are_the_recorded_contract() {
    let golden = golden();
    assert_eq!(
        golden
            .bytes("contracts/schemas/project-v1.schema.json")
            .expect("recorded"),
        PROJECT_SCHEMA_TEXT.as_bytes()
    );
    assert_eq!(
        golden
            .bytes("contracts/schemas/manifest-v1.schema.json")
            .expect("recorded"),
        MANIFEST_SCHEMA_TEXT.as_bytes()
    );
}

#[test]
fn recorded_verdicts_match() {
    let cases: Vec<Value> = golden()
        .json("contracts/schema-verdicts.json")
        .expect("recorded verdicts");
    assert_eq!(cases.len(), 7);
    for case in &cases {
        let name = case["case"].as_str().expect("case name");
        let actual = verdict(&case["document"], Schema::Project);
        if case["valid"] == true {
            assert_eq!(actual, None, "{name}");
        } else {
            let expected = (
                case["code"].as_str().expect("code").to_owned(),
                case["message"].as_str().expect("message").to_owned(),
                case["error_count"].as_u64().expect("count"),
            );
            assert_eq!(actual, Some(expected), "{name}");
        }
    }
}

/// Applies a fixture operation list: `set` (an empty path replaces the document) and `remove`.
fn apply(mut doc: Value, ops: &[Value]) -> Value {
    for op in ops {
        let path = op["path"].as_array().expect("path");
        let Some((last, parents)) = path.split_last() else {
            doc = op["value"].clone();
            continue;
        };
        let mut node = &mut doc;
        for key in parents {
            node = match key {
                Value::String(k) => &mut node[k.as_str()],
                Value::Number(i) => {
                    &mut node[usize::try_from(i.as_u64().expect("index")).expect("index")]
                }
                other => panic!("bad path element {other}"),
            };
        }
        match (op["op"].as_str().expect("op"), last) {
            ("set", Value::String(k)) => node[k.as_str()] = op["value"].clone(),
            ("set", Value::Number(i)) => {
                node[usize::try_from(i.as_u64().expect("index")).expect("index")] =
                    op["value"].clone();
            }
            ("remove", Value::String(k)) => {
                node.as_object_mut().expect("object").shift_remove(k);
            }
            other => panic!("unsupported operation {other:?}"),
        }
    }
    doc
}

#[test]
fn verdicts_match_the_prototype_on_edge_cases() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let rich: Value =
        serde_json::from_slice(&fs::read(dir.join("rich.project.json")).expect("fixture"))
            .expect("JSON");
    let manifest: Value = golden()
        .json("workflows/halloween_thirteenth_hour/outputs/examples/output/halloween/Trinacta hodina.sonara/manifest.json")
        .expect("recorded manifest");
    let cases: Vec<Value> =
        serde_json::from_slice(&fs::read(dir.join("schema-parity.json")).expect("fixture"))
            .expect("JSON");
    assert!(cases.len() > 50);
    for case in &cases {
        let name = case["case"].as_str().expect("case name");
        let base = match case["base"].as_str() {
            Some("rich") => rich.clone(),
            Some("manifest") => manifest.clone(),
            other => panic!("unknown base {other:?}"),
        };
        let doc = apply(base, case["ops"].as_array().expect("ops"));
        let schema = schema_named(case["schema"].as_str().expect("schema"));
        let actual = verdict(&doc, schema);
        if case["valid"] == true {
            assert_eq!(actual, None, "{name}");
        } else {
            let expected = (
                case["code"].as_str().expect("code").to_owned(),
                case["message"].as_str().expect("message").to_owned(),
                case["error_count"].as_u64().expect("count"),
            );
            assert_eq!(actual, Some(expected), "{name}");
        }
    }
}

#[test]
fn a_long_array_is_reported_whole() {
    // Python: "tracks: [{'id': 'trk_0', 'name': 'T', 'kind': 'audio'}, …] is too long", once.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut doc: Value =
        serde_json::from_slice(&fs::read(dir.join("rich.project.json")).expect("fixture"))
            .expect("JSON");
    let tracks: Vec<Value> = (0..513)
        .map(|i| json!({"id": format!("trk_{i}"), "name": "T", "kind": "audio"}))
        .collect();
    doc["tracks"] = Value::Array(tracks);
    let (code, message, count) = verdict(&doc, Schema::Project).expect("too many tracks");
    assert_eq!(code, ErrorCode::FormatError.as_str());
    assert_eq!(count, 1);
    assert!(message.starts_with(
        "project-v1.schema.json: tracks: [{'id': 'trk_0', 'name': 'T', 'kind': 'audio'}, {'id': 'trk_1'"
    ));
    assert!(message.ends_with("{'id': 'trk_512', 'name': 'T', 'kind': 'audio'}] is too long"));
}

#[test]
fn a_trailing_newline_is_rejected_unlike_python() {
    // Documented difference: Python's `re` lets `$` match before a final newline, so the
    // prototype accepts these values; the ECMA-262 patterns of the crate do not.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut doc: Value =
        serde_json::from_slice(&fs::read(dir.join("rich.project.json")).expect("fixture"))
            .expect("JSON");
    doc["tracks"][0]["id"] = json!("trk_fld\n");
    let (_, message, count) = verdict(&doc, Schema::Project).expect("rejected");
    assert_eq!(count, 1);
    assert!(
        message.starts_with("project-v1.schema.json: tracks/0/id: 'trk_fld\\n' does not match")
    );
}

/// Every recorded `project.json`: workflow snapshots and final projects.
fn recorded_documents(golden: &Golden) -> Vec<(String, Vec<u8>)> {
    let workflows = golden.root().join("workflows");
    let mut names: Vec<PathBuf> = fs::read_dir(&workflows)
        .expect("golden workflows exist")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.is_dir())
        .collect();
    names.sort();
    let mut out = Vec::new();
    for dir in names {
        let workflow = dir
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        let rel = format!("workflows/{workflow}/final.project.json");
        out.push((rel.clone(), golden.bytes(&rel).expect("final project")));
        let mut snapshots: Vec<String> = fs::read_dir(dir.join("snapshots"))
            .expect("snapshots exist")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        snapshots.sort();
        for snapshot in snapshots {
            let rel = format!("workflows/{workflow}/snapshots/{snapshot}");
            out.push((rel.clone(), golden.bytes_gz(&rel).expect("snapshot")));
        }
    }
    out
}

#[test]
fn every_recorded_project_document_validates() {
    let golden = golden();
    let documents = recorded_documents(&golden);
    assert!(documents.len() > 200, "found {} documents", documents.len());
    for (rel, raw) in documents {
        let doc: Value = serde_json::from_slice(&raw).expect("valid JSON");
        assert_eq!(validate_document(&doc, Schema::Project), Ok(()), "{rel}");
    }
}
