//! Contract tests (SON-P19): one test per planning contract (`contracts/*.md` of the Sonora task
//! plan), against the public interfaces on this branch, so a failure names the contract that
//! broke. `docs/integration-tests.md` lists what each test covers, which contracts are checked
//! elsewhere, and which are not integrated yet.
//!
//! Run them alone with `cargo test -p sonara-agent --test contracts`.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::contract::tools::{Safety, TOOLS, ToolSpec};
use sonara_agent::permissions::sequential_tokens;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget, shared_tokens};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_app::instruments::{FamilyClaim, InstrumentCatalog};
use sonara_app::settings::Settings;
use sonara_core::ids::SequentialIds;
use sonara_core::model::track::MixerSettings;
use sonara_midi::orchestration::{ArticulationAvailability, INSTRUMENTS, Provenance};
use tempfile::TempDir;

struct World {
    _dir: TempDir,
    root: PathBuf,
    service: AgentService,
}

impl World {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = fs::canonicalize(dir.path()).expect("the directory resolves");
        for sub in ["work", "projects", "workspaces"] {
            fs::create_dir(root.join(sub)).expect("a folder");
        }
        let app = Arc::new(
            AppContext::create(AppOptions {
                workspaces: Some(root.join("workspaces")),
                settings: Some(Settings::default()),
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
                audit: AuditTarget::Disabled,
                tokens: Some(shared_tokens(sequential_tokens())),
                environment: Some(PolicyEnvironment {
                    allowed: None,
                    projects_dir: root.join("projects"),
                }),
                ..AgentOptions::default()
            },
        )
        .expect("the service");
        Self {
            _dir: dir,
            root,
            service,
        }
    }

    fn call(&self, tool: &str, args: &Value) -> Value {
        self.service.call(tool, Some(args))
    }

    fn ok(&self, tool: &str, args: &Value) -> Value {
        let reply = self.call(tool, args);
        assert_eq!(reply["ok"], true, "{tool}: {reply}");
        reply
    }

    fn version(&self) -> Value {
        self.ok("get_project_state", &json!({}))["project_version"].clone()
    }
}

/// contracts/agent-tools.md: tools are described and classified, read tools do not change the
/// project, and the project can only be changed through tools that report the change.
#[test]
fn contract_agent_tools() {
    let world = World::new();
    world.ok("create_project", &json!({"name": "Contract", "bpm": 120}));
    let specs = world.service.tools();
    assert_eq!(specs.len(), 58);
    let tools: Vec<&ToolSpec> = TOOLS.iter().collect();
    let mut reads = 0;
    for spec in &tools {
        assert!(
            !spec.title.is_empty() && !spec.description().is_empty(),
            "{}",
            spec.name
        );
        if spec.safety == Safety::Read {
            reads += 1;
            let before = world.version();
            // Arguments may be missing or invalid for a tool; the project must not change anyway.
            let _ = world.call(spec.name, &json!({}));
            assert_eq!(
                world.version(),
                before,
                "read tool {} changed the project",
                spec.name
            );
        }
    }
    assert!(reads > 10, "{reads} read tools");
    assert!(tools.iter().any(|t| t.safety == Safety::Write));
    assert!(tools.iter().any(|t| t.safety == Safety::Destructive));
}

/// contracts/project-commands.md: an invalid operation is rejected atomically, a valid one
/// reports an auditable change, and it can be undone.
#[test]
fn contract_project_commands() {
    let world = World::new();
    world.ok("create_project", &json!({"name": "Contract", "bpm": 120}));
    let before = world.version();
    let tracks = world.ok("list_tracks", &json!({}))["tracks"].clone();
    let refused = world.call("set_tempo", &json!({"bpm": -1.0}));
    assert_eq!(refused["ok"], false);
    assert_eq!(world.version(), before);
    assert_eq!(world.ok("list_tracks", &json!({}))["tracks"], tracks);

    let done = world.ok(
        "create_track",
        &json!({"name": "Keys", "kind": "instrument"}),
    );
    assert_eq!(done["committed"], true);
    assert!(
        done["changes"]["created"]
            .as_array()
            .is_some_and(|c| !c.is_empty()),
        "{done}"
    );
    assert_ne!(world.version(), before);
    world.ok("undo", &json!({}));
    assert_eq!(world.ok("list_tracks", &json!({}))["tracks"], tracks);
}

/// contracts/song-project.md: a saved project opens again with the same tracks and a schema
/// version on disk.
#[test]
fn contract_song_project_round_trips() {
    let world = World::new();
    world.ok("create_project", &json!({"name": "Round Trip", "bpm": 98}));
    world.ok(
        "create_track",
        &json!({"name": "Keys", "kind": "instrument"}),
    );
    let tracks = world.ok("list_tracks", &json!({}))["tracks"].clone();
    let path = world.root.join("projects").join("rt.sonara");
    world.ok(
        "save_project",
        &json!({"path": path.to_str().expect("utf-8")}),
    );
    assert!(path.exists());
    world.ok("create_project", &json!({"name": "Other"}));
    world.ok(
        "open_project",
        &json!({"path": path.to_str().expect("utf-8")}),
    );
    assert_eq!(world.ok("list_tracks", &json!({}))["tracks"], tracks);
    let all = fs::read_dir(&path).map_or_else(
        |_| String::new(),
        |entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(",")
        },
    );
    let manifest = fs::read_to_string(path.join("manifest.json")).unwrap_or_default();
    assert!(
        manifest.contains("version"),
        "no schema version in {all}: {manifest}"
    );
}

/// contracts/orchestration-metadata.md: every record has a source or an explicit estimate, ids
/// are unique, and an articulation is `Supported` only when a library declared it.
#[test]
fn contract_orchestration_metadata() {
    let mut ids = std::collections::HashSet::new();
    for instrument in INSTRUMENTS {
        assert!(
            ids.insert(instrument.id()),
            "duplicate id {}",
            instrument.id()
        );
        match instrument.articulations_provenance() {
            Provenance::Cited { source } => assert!(!source.is_empty(), "{}", instrument.id()),
            Provenance::Estimate { reason } => assert!(!reason.is_empty(), "{}", instrument.id()),
        }
        for articulation in instrument.articulations() {
            let availability = instrument.articulation_availability(articulation.id(), &[]);
            assert_ne!(
                availability,
                ArticulationAvailability::Supported,
                "{}",
                instrument.id()
            );
            let declared =
                instrument.articulation_availability(articulation.id(), &[articulation.id()]);
            assert_eq!(declared, ArticulationAvailability::Supported);
        }
        assert_eq!(
            instrument.articulation_availability("not_an_articulation", &["not_an_articulation"]),
            ArticulationAvailability::UnknownArticulation
        );
    }
}

/// contracts/instrument-catalog.md: an empty scan is an empty catalog, and a preset found
/// without a manifest never claims articulations, a licence or a confirmed family.
#[test]
fn contract_instrument_catalog() {
    assert_eq!(InstrumentCatalog::discover(&[]).presets().len(), 0);
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("Lib");
    fs::create_dir_all(root.join("Strings")).expect("folder");
    fs::write(root.join("Strings/Violin.sfz"), "<region> sample=a.wav\n").expect("patch");
    let catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    let preset = catalog.presets().first().expect("a preset");
    assert!(!preset.id.as_str().is_empty() && !preset.display_name.is_empty());
    assert!(
        preset.articulations.is_empty(),
        "articulations are unverified, not declared"
    );
    assert_eq!(preset.license, None);
    assert!(matches!(preset.family, FamilyClaim::Inferred(_)));
}

/// contracts/mix-control.md: gain (dB) and pan have documented, enforced ranges, independent of
/// any UI.
#[test]
fn contract_mix_control_ranges() {
    assert!(MixerSettings::new(-144.0, -1.0, false, false, false).is_ok());
    assert!(MixerSettings::new(12.0, 1.0, false, false, false).is_ok());
    assert!(MixerSettings::new(12.1, 0.0, false, false, false).is_err());
    assert!(MixerSettings::new(-144.1, 0.0, false, false, false).is_err());
    assert!(MixerSettings::new(0.0, 1.01, false, false, false).is_err());
    assert!(MixerSettings::new(f64::NAN, 0.0, false, false, false).is_err());
}
