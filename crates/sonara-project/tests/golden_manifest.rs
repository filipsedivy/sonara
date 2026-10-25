//! Saving the projects the prototype's workflows saved
//! (`golden/workflows/*/outputs/examples/output/*/*.sonara/`) with the recorder's frozen clock
//! (2026-01-01T12:00:00Z) writes the recorded `project.json` and `manifest.json` byte for byte,
//! and reports what the recorded `save_project` result reports.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use serde_json::Value;
use sonara_project::serializer::{loads, project_from_value};
use sonara_project::store::{Layout, MANIFEST, PROJECT, ProjectStore};
use sonara_testkit::Golden;

/// 2026-01-01T12:00:00Z, the recorder's frozen `datetime.now`.
const FROZEN_UNIX_SECONDS: u64 = 1_767_268_800;

/// `(workflow, bundle directory)` for every saved bundle.
fn recorded_bundles(golden: &Golden) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let workflows = golden.root().join("workflows");
    let mut names: Vec<PathBuf> = fs::read_dir(&workflows)
        .expect("golden workflows exist")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.is_dir())
        .collect();
    names.sort();
    for workflow in names {
        let output = workflow.join("outputs/examples/output");
        let Ok(groups) = fs::read_dir(&output) else {
            continue;
        };
        for group in groups {
            let group = group.expect("entry").path();
            for entry in fs::read_dir(&group).expect("output group") {
                let path = entry.expect("entry").path();
                if path.extension().is_some_and(|ext| ext == "sonara") && path.is_dir() {
                    let name = workflow.file_name().expect("name").to_string_lossy();
                    out.push((name.into_owned(), path));
                }
            }
        }
    }
    out.sort();
    out
}

/// GLD-01 deviation: the recorder pinned the prototype's own version (0.1.0) into
/// `sonara_version`. The field reports the current application version (VER-01), so it moves
/// with every release; replace the recording's value with the build's before a byte comparison.
fn with_current_version(manifest: &[u8]) -> String {
    String::from_utf8_lossy(manifest).replace(
        "\"sonara_version\": \"0.1.0\"",
        &format!("\"sonara_version\": \"{}\"", env!("CARGO_PKG_VERSION")),
    )
}

/// The recorded `save_project` result whose path names `bundle`.
fn save_result(golden: &Golden, workflow: &str, bundle: &Path) -> Value {
    let transcript: Value = golden
        .json(&format!("workflows/{workflow}/transcript.json"))
        .expect("transcript");
    let bundle_name = bundle.file_name().expect("name").to_string_lossy();
    let steps = transcript["steps"].as_array().expect("transcript steps");
    steps
        .iter()
        .filter(|step| step["tool"] == "save_project")
        .map(|step| step["result"].clone())
        .find(|result| {
            result["path"]
                .as_str()
                .is_some_and(|path| path.ends_with(&format!("/{bundle_name}")))
        })
        .unwrap_or_else(|| panic!("no save_project result for {bundle_name}"))
}

#[test]
fn saving_a_recorded_project_writes_the_recorded_bundle() {
    let golden = Golden::open().expect("golden data is available");
    let bundles = recorded_bundles(&golden);
    assert_eq!(bundles.len(), 4, "{bundles:?}");
    let store = ProjectStore::new("unused-workspaces")
        .with_clock(|| UNIX_EPOCH + Duration::from_secs(FROZEN_UNIX_SECONDS));
    for (workflow, bundle) in bundles {
        let recorded_project = fs::read(bundle.join(PROJECT)).expect("project.json");
        let recorded_manifest = fs::read(bundle.join(MANIFEST)).expect("manifest.json");
        let project =
            project_from_value(&loads(&recorded_project).expect("JSON object")).expect("project");

        let dir = tempfile::tempdir().expect("temp dir");
        let workspace = dir.path().join("workspace");
        fs::create_dir_all(&workspace).expect("workspace");
        let target = dir.path().join(bundle.file_name().expect("bundle name"));
        let saved = store
            .save(&project, &workspace, &target, None)
            .expect("save");

        assert_eq!(
            fs::read(target.join(PROJECT)).expect("written"),
            recorded_project,
            "{workflow}: project.json"
        );
        let written_manifest = fs::read(target.join(MANIFEST)).expect("written");
        assert_eq!(
            String::from_utf8_lossy(&written_manifest),
            with_current_version(&recorded_manifest),
            "{workflow}: manifest.json"
        );

        let recorded = save_result(&golden, &workflow, &bundle);
        assert_eq!(
            saved.project_sha256, recorded["project_sha256"],
            "{workflow}"
        );
        assert_eq!(saved.layout, Layout::Bundle);
        assert_eq!(saved.layout.as_str(), recorded["layout"]);
        assert_eq!(Value::from(saved.assets), recorded["assets"], "{workflow}");
        assert_eq!(saved.backup, None);
        assert!(recorded["backup"].is_null());
    }
}
