//! Crate dependency rules (MOD-07, ARC-08), checked over
//! `cargo metadata`.
//!
//! Only normal and build dependencies are checked; tests may use any crate. The exception is
//! the desktop application: no crate depends on it in any way.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// The workspace crates each crate may depend on.
const ALLOWED: &[(&str, &[&str])] = &[
    ("sonara-core", &[]),
    ("sonara-project", &["sonara-core"]),
    ("sonara-midi", &["sonara-core"]),
    ("sonara-dsp", &["sonara-core"]),
    ("sonara-rt", &[]),
    ("sonara-io", &["sonara-core", "sonara-rt"]),
    ("sonara-media", &["sonara-core", "sonara-dsp"]),
    (
        "sonara-engine",
        &[
            "sonara-core",
            "sonara-dsp",
            "sonara-media",
            "sonara-midi",
            "sonara-rt",
        ],
    ),
    (
        "sonara-app",
        &[
            "sonara-core",
            "sonara-dsp",
            "sonara-engine",
            "sonara-io",
            "sonara-media",
            "sonara-midi",
            "sonara-project",
            "sonara-rt",
        ],
    ),
    (
        "sonara-agent",
        &["sonara-app", "sonara-core", "sonara-midi", "sonara-project"],
    ),
    ("sonara-testkit", &[]),
    (
        "sonara",
        &[
            "sonara-agent",
            "sonara-app",
            "sonara-core",
            "sonara-project",
        ],
    ),
    // The desktop reaches the core only through the application and the agent.
    ("sonara-desktop", &["sonara-agent", "sonara-app"]),
];

/// The real-time crates (RT-01, `.claude/rules/rust/realtime.md`).
const REAL_TIME: &[&str] = &["sonara-dsp", "sonara-rt", "sonara-io", "sonara-engine"];

/// The external crates the pure domain crate may use: no I/O, audio, GUI or agent code.
const CORE_EXTERNAL: &[&str] = &[
    "getrandom",
    "indexmap",
    "regex",
    "serde",
    "serde_json",
    "thiserror",
];

/// Normal and build dependencies of every workspace crate.
fn dependencies() -> BTreeMap<String, BTreeSet<String>> {
    dependencies_of_kinds(|kind| kind.is_null() || kind == "build")
}

/// Every dependency of every workspace crate, development dependencies included.
fn all_dependencies() -> BTreeMap<String, BTreeSet<String>> {
    dependencies_of_kinds(|_| true)
}

/// The dependencies of every workspace crate whose kind (`null`, `"dev"`, `"build"`) passes
/// `keep`.
fn dependencies_of_kinds(keep: impl Fn(&Value) -> bool) -> BTreeMap<String, BTreeSet<String>> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let output = Command::new(cargo)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
        ])
        .arg("--manifest-path")
        .arg(manifest)
        .output()
        .expect("cargo metadata runs");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("metadata is JSON");
    metadata["packages"]
        .as_array()
        .expect("package list")
        .iter()
        .map(|package| {
            let name = package["name"].as_str().expect("package name").to_owned();
            let deps = package["dependencies"]
                .as_array()
                .expect("dependency list")
                .iter()
                .filter(|dep| keep(&dep["kind"]))
                .map(|dep| dep["name"].as_str().expect("dependency name").to_owned())
                .collect();
            (name, deps)
        })
        .collect()
}

#[test]
fn every_workspace_crate_has_rules() {
    let rules: BTreeSet<&str> = ALLOWED.iter().map(|(name, _)| *name).collect();
    for name in dependencies().keys() {
        assert!(
            rules.contains(name.as_str()),
            "no dependency rules for {name}"
        );
    }
}

#[test]
fn crates_depend_only_on_allowed_layers() {
    let allowed: BTreeMap<&str, &[&str]> = ALLOWED.iter().copied().collect();
    let mut violations = Vec::new();
    for (name, deps) in dependencies() {
        let permitted = allowed.get(name.as_str()).copied().unwrap_or_default();
        for dep in deps.iter().filter(|d| d.starts_with("sonara")) {
            if !permitted.contains(&dep.as_str()) {
                violations.push(format!("{name} depends on {dep}"));
            }
        }
    }
    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn the_domain_crate_stays_pure() {
    let deps = dependencies();
    let core = deps
        .get("sonara-core")
        .expect("sonara-core is a workspace crate");
    let extra: Vec<&String> = core
        .iter()
        .filter(|d| !CORE_EXTERNAL.contains(&d.as_str()))
        .collect();
    assert_eq!(
        extra,
        Vec::<&String>::new(),
        "sonara-core gained dependencies"
    );
}

#[test]
fn no_crate_depends_on_the_desktop_application() {
    let dependents: Vec<String> = all_dependencies()
        .into_iter()
        .filter(|(_, deps)| deps.contains("sonara-desktop"))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(dependents, Vec::<String>::new());
}

#[test]
fn no_real_time_crate_reaches_tauri() {
    // RT-01: neither a real-time crate nor a workspace crate it depends on may use Tauri, so
    // the callback cannot reach GUI code. Their workspace dependencies are followed.
    let deps = dependencies();
    let mut reached: BTreeSet<&str> = BTreeSet::new();
    let mut pending: Vec<&str> = REAL_TIME.to_vec();
    while let Some(name) = pending.pop() {
        if !reached.insert(name) {
            continue;
        }
        let crate_deps = deps.get(name).expect("a workspace crate");
        pending.extend(
            crate_deps
                .iter()
                .filter(|dep| deps.contains_key(dep.as_str()))
                .map(String::as_str),
        );
    }
    let mut violations = Vec::new();
    for name in &reached {
        for dep in &deps[*name] {
            if dep == "tauri" || dep.starts_with("tauri-") {
                violations.push(format!("{name} depends on {dep}"));
            }
        }
    }
    assert!(reached.contains("sonara-core"), "{reached:?}");
    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn the_desktop_reaches_the_core_only_through_the_application_and_the_agent() {
    let deps = dependencies();
    let desktop = deps
        .get("sonara-desktop")
        .expect("sonara-desktop is a workspace crate");
    let workspace: Vec<&str> = desktop
        .iter()
        .filter(|dep| deps.contains_key(dep.as_str()))
        .map(String::as_str)
        .collect();
    assert_eq!(workspace, ["sonara-agent", "sonara-app"]);
}
