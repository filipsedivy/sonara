//! Every project document the prototype wrote re-serializes to the same bytes, both as plain
//! JSON and through the project model.

use std::fs;
use std::path::{Path, PathBuf};

use sonara_project::canonical::to_canonical_bytes;
use sonara_project::serializer::{dumps, loads, project_from_value};
use sonara_testkit::Golden;

/// Relative paths of all recorded `project.json` documents: snapshots and final projects.
fn project_documents(golden: &Golden) -> Vec<String> {
    let workflows = golden.root().join("workflows");
    let mut out = Vec::new();
    for entry in fs::read_dir(&workflows).expect("golden workflows exist") {
        let entry = entry.expect("readable entry");
        if !entry.file_type().expect("file type").is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let base = PathBuf::from("workflows").join(name.as_ref());
        out.push(format!("{}/final.project.json", base.display()));
        for snap in
            fs::read_dir(workflows.join(name.as_ref()).join("snapshots")).expect("snapshots exist")
        {
            let file = snap.expect("readable entry").file_name();
            out.push(format!(
                "{}/snapshots/{}",
                base.display(),
                file.to_string_lossy()
            ));
        }
    }
    out.sort();
    out
}

/// The first differing line, with its number, for a readable failure.
fn first_difference(expected: &[u8], actual: &[u8]) -> String {
    let expected = String::from_utf8_lossy(expected);
    let actual = String::from_utf8_lossy(actual);
    for (n, (e, a)) in expected.lines().zip(actual.lines()).enumerate() {
        if e != a {
            return format!("line {}:\n  expected: {e}\n  actual:   {a}", n + 1);
        }
    }
    format!(
        "lengths differ: expected {} lines, actual {} lines",
        expected.lines().count(),
        actual.lines().count()
    )
}

/// Every recorded document: its relative path and its bytes.
fn recorded_documents() -> Vec<(String, Vec<u8>)> {
    let golden = Golden::open().expect("golden data is available");
    let documents = project_documents(&golden);
    assert!(documents.len() > 200, "found {} documents", documents.len());
    documents
        .into_iter()
        .map(|rel| {
            let raw = if Path::new(&rel).extension().is_some_and(|ext| ext == "gz") {
                golden.bytes_gz(&rel)
            } else {
                golden.bytes(&rel)
            }
            .expect("readable document");
            (rel, raw)
        })
        .collect()
}

#[test]
fn recorded_documents_round_trip_as_json() {
    for (rel, raw) in recorded_documents() {
        let value: serde_json::Value = serde_json::from_slice(&raw).expect("valid JSON");
        let written = to_canonical_bytes(&value);
        assert!(
            written == raw,
            "{rel}: {}",
            first_difference(&raw, &written)
        );
    }
}

#[test]
fn recorded_documents_round_trip_through_the_model() {
    for (rel, raw) in recorded_documents() {
        let doc = loads(&raw).expect("JSON object");
        let project = project_from_value(&doc).unwrap_or_else(|err| panic!("{rel}: {err}"));
        let written = dumps(&project);
        assert!(
            written == raw,
            "{rel}: {}",
            first_difference(&raw, &written)
        );
    }
}
