//! The sampler's `instrument` through the agent tools: `list_plugins` offers preset IDs, and
//! `set_instrument` and `set_effect_parameter` store the preset ID when an agent or script
//! passes the patch's path, as it did before preset IDs (CTR-01: the tools and their schemas
//! are unchanged). The library is a generated directory of patch files; nothing is played.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::permissions::sequential_tokens;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget, shared_tokens};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_app::samples::LIBRARIES_KEY;
use sonara_app::settings::Settings;
use sonara_core::ids::SequentialIds;

/// The agent on an application whose settings list the library `Lib` with the patches
/// `Strings/Violin.sfz` and `Strings/Cello.sfz`: the directory (kept alive), the library root
/// and the service.
fn world() -> (tempfile::TempDir, std::path::PathBuf, AgentService) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let root = fs::canonicalize(dir.path()).expect("the directory resolves");
    for sub in ["work", "projects", "workspaces"] {
        fs::create_dir(root.join(sub)).expect("a folder");
    }
    let library = root.join("Lib");
    for patch in ["Strings/Violin.sfz", "Strings/Cello.sfz"] {
        let path = library.join(patch);
        fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
        fs::write(path, "<region> sample=a.wav\n").expect("write the patch");
    }
    let mut settings = Settings::default();
    settings.extra.insert(
        LIBRARIES_KEY.to_owned(),
        json!([library.to_str().expect("UTF-8")]),
    );
    let app = Arc::new(
        AppContext::create(AppOptions {
            workspaces: Some(root.join("workspaces")),
            settings: Some(settings),
            persist_settings: false,
            audio_backend: Some("null".to_owned()),
            ids: Some(IdSource::new(Box::new(SequentialIds::new(0)))),
            ..AppOptions::default()
        })
        .expect("the application"),
    );
    let service = AgentService::new(
        app,
        AgentOptions {
            base_dir: Some(root.join("work")),
            audit: AuditTarget::File(root.join("audit.jsonl")),
            tokens: Some(shared_tokens(sequential_tokens())),
            environment: Some(PolicyEnvironment {
                allowed: None,
                projects_dir: root.join("projects"),
            }),
            ..AgentOptions::default()
        },
    )
    .expect("the service");
    (dir, library, service)
}

fn call(service: &AgentService, tool: &str, args: &Value) -> Value {
    let reply = service.call(tool, Some(args));
    assert_eq!(reply["ok"], true, "{tool}: {reply}");
    reply
}

fn path_text(path: &Path) -> String {
    path.to_str().expect("UTF-8").to_owned()
}

/// A new project with a sampler track; the track's ID.
fn sampler_project(service: &AgentService) -> Value {
    call(service, "create_project", &json!({"name": "Strings"}));
    call(
        service,
        "create_track",
        &json!({"name": "Strings", "kind": "instrument", "instrument": "sonara.sampler"}),
    )["result"]["track_id"]
        .clone()
}

#[test]
fn list_plugins_offers_preset_ids() {
    let (_dir, _library, service) = world();
    let listed = call(
        &service,
        "list_plugins",
        &json!({"plugin_id": "sonara.sampler"}),
    );
    let instrument = listed["plugin"]["parameters"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|param| param["id"] == "instrument")
        .unwrap_or_else(|| panic!("the instrument parameter: {listed}"));
    assert_eq!(
        instrument["choices"],
        json!(["lib/Strings/Cello", "lib/Strings/Violin"])
    );
}

#[test]
fn set_instrument_with_a_patch_path_stores_the_preset_id() {
    let (_dir, library, service) = world();
    let track = sampler_project(&service);
    let reply = call(
        &service,
        "set_instrument",
        &json!({"track_id": track, "plugin_id": "sonara.sampler",
                "params": {"instrument": path_text(&library.join("Strings/Violin.sfz"))}}),
    );
    assert_eq!(
        reply["result"]["params"]["instrument"], "lib/Strings/Violin",
        "{reply}"
    );
}

#[test]
fn set_effect_parameter_with_a_patch_path_stores_the_preset_id() {
    let (_dir, library, service) = world();
    let track = sampler_project(&service);
    let device = call(
        &service,
        "set_instrument",
        &json!({"track_id": track, "plugin_id": "sonara.sampler",
                "params": {"instrument": "lib/Strings/Violin"}}),
    )["result"]["device_id"]
        .clone();
    let reply = call(
        &service,
        "set_effect_parameter",
        &json!({"device_id": device, "param": "instrument",
                "value": path_text(&library.join("Strings/Cello.sfz"))}),
    );
    assert_eq!(
        reply["params"]["instrument"], "lib/Strings/Cello",
        "{reply}"
    );
}

#[test]
fn a_path_outside_the_libraries_is_refused_with_the_choices() {
    let (_dir, _library, service) = world();
    let track = sampler_project(&service);
    let reply = service.call(
        "set_instrument",
        Some(&json!({"track_id": track, "plugin_id": "sonara.sampler",
                     "params": {"instrument": "/elsewhere/Lib/Strings/Violin.sfz"}})),
    );
    assert_eq!(reply["ok"], false, "{reply}");
    assert_eq!(reply["error"]["code"], "INVALID_ARGUMENT", "{reply}");
    assert_eq!(
        reply["error"]["details"]["choices"],
        json!(["lib/Strings/Cello", "lib/Strings/Violin"]),
        "{reply}"
    );
}
