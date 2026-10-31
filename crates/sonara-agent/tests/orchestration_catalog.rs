//! The orchestrator's read of the instrument catalog: through a test double that holds a fixed
//! catalog (TST-08), and through the real agent service over a library on disk. Each expected
//! sound and capability is written from the library's manifest and the facts the double's check
//! records (TST-10).

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::orchestration::{CatalogReader, Orchestrator, ToolRunner};
use sonara_agent::permissions::sequential_tokens;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget, shared_tokens};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_app::instruments::{
    ChainPreferences, EnsembleKind, InstrumentCatalog, InstrumentPreset, KeyZone, PatchFacts,
    PatchLoadError, PatchLoader, RegionAttack, SoundFacts, SoundRequest, SoundSource, Step,
    VelocityRange, sound_caps,
};
use sonara_app::samples::LIBRARIES_KEY;
use sonara_app::settings::Settings;
use sonara_core::ids::SequentialIds;
use sonara_midi::orchestration::{Articulation, InstrumentType, PitchRange};
use sonara_midi::performance::{DynamicsKind, LegatoKind, SoundCaps};
use tempfile::TempDir;

/// A library whose manifest says that `Strings/Violins.sfz` is a violin section with two key
/// switches, pizzicato stating its attack, controller dynamics and overlap legato.
fn library(root: &Path) {
    fs::create_dir_all(root.join("Strings")).expect("folder");
    fs::write(root.join("Strings/Violins.sfz"), "<region> sample=a.wav\n").expect("patch");
    let manifest = json!({
        "version": 2,
        "id": "lib",
        "name": "Test Strings",
        "instruments": [{
            "path": "Strings/Violins.sfz",
            "name": "Violins",
            "instrument": "violin",
            "ensemble": {"kind": "section"},
            "articulations": [
                {"id": "sustain", "via": "keyswitch", "key": 24, "default": true},
                {"id": "pizzicato", "via": "keyswitch", "key": 25, "attack_ms": 12}
            ],
            "dynamics": {"via": "cc", "cc": 1},
            "legato": {"kind": "overlap", "crossfade_ms": 90, "start_offset_ms": 60, "gap_ms": 0}
        }]
    });
    fs::write(root.join("sonara-library.json"), manifest.to_string()).expect("manifest");
}

/// A request for a violin section's ordinary sound.
fn violins() -> SoundRequest {
    SoundRequest {
        ensemble: Some(EnsembleKind::Section),
        ..SoundRequest::new(InstrumentType::Violin)
    }
}

/// The articulations the library's violins play as asked: the two it maps, and legato, which the
/// sampler's overlap legato plays.
fn violin_articulations() -> Vec<Articulation> {
    vec![
        Articulation::Sustain,
        Articulation::Legato,
        Articulation::Pizzicato,
    ]
}

/// A runner that holds a fixed catalog and counts how often it is read; every tool call fails as
/// if no project were open, and is recorded.
struct Shelf {
    presets: Vec<InstrumentPreset>,
    reads: Cell<usize>,
    calls: RefCell<Vec<String>>,
}

impl Shelf {
    fn new(presets: Vec<InstrumentPreset>) -> Self {
        Self {
            presets,
            reads: Cell::new(0),
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl ToolRunner for &Shelf {
    fn call(&self, tool: &str, _args: &Value) -> Value {
        self.calls.borrow_mut().push(tool.to_owned());
        json!({"ok": false, "tool": tool, "error": {"code": "NO_PROJECT", "message": "no project"}})
    }
}

impl CatalogReader for &Shelf {
    fn presets(&self) -> Vec<InstrumentPreset> {
        self.reads.set(self.reads.get() + 1);
        self.presets.clone()
    }
}

/// A check that finds the violins sampled over keys 21 to 108 in two velocity layers, switched by
/// keys 24 and 25 with 24 the default, and measures the sustain regions' onset at 40 ms and the
/// longest hold at 11.2 s.
struct Checked;

impl PatchLoader for Checked {
    fn load(&self, _patch: &Path) -> Result<Option<SoundFacts>, PatchLoadError> {
        Ok(Some(SoundFacts {
            attacks: vec![RegionAttack {
                switch: Some(24),
                velocities: VelocityRange { lo: 1, hi: 127 },
                attack_ms: 40,
            }],
            max_hold_ms: Some(11_200),
        }))
    }

    fn inspect(&self, _patch: &Path) -> Option<PatchFacts> {
        Some(PatchFacts {
            regions: 2,
            playable_regions: 2,
            sampled: vec![KeyZone {
                lo: 21,
                hi: 108,
                layers: vec![
                    VelocityRange { lo: 1, hi: 62 },
                    VelocityRange { lo: 63, hi: 127 },
                ],
            }],
            switch_keys: vec![24, 25],
            default_switch: Some(24),
            ignored_opcodes: Vec::new(),
            missing_samples: Vec::new(),
            unreadable_samples: Vec::new(),
            notes: Vec::new(),
            decoded_bytes: 0,
            loadable: true,
            sound: None,
        })
    }
}

/// The orchestrator reads the catalog once through its runner and calls no tool; each part's sound
/// comes back with what it can do, from the claims and from the facts the check recorded.
#[test]
fn the_planner_reads_what_plays_a_part_and_what_it_can_do() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("Lib");
    library(&root);
    let mut catalog = InstrumentCatalog::discover(std::slice::from_ref(&root));
    let id = catalog.presets()[0].id.clone();
    catalog.verify(&id, &Checked).expect("the preset is listed");
    let shelf = Shelf::new(catalog.into_presets());

    let snapshot = Orchestrator::new(&shelf).catalog();
    assert_eq!(shelf.reads.get(), 1);
    assert_eq!(snapshot.presets(), shelf.presets.as_slice());

    let sound = snapshot.sound(&violins(), ChainPreferences::default());
    assert_eq!(sound.plan.step, Step::Exact);
    assert_eq!(
        sound.plan.source,
        SoundSource::Sampler { preset: id.clone() }
    );
    let range = PitchRange::try_new(21, 108).expect("a valid range");
    assert_eq!(
        sound.caps,
        SoundCaps {
            articulations: violin_articulations(),
            legato: LegatoKind::Overlap { crossfade_ms: 90.0 },
            layers: vec![62, 127],
            dynamics: DynamicsKind::Cc,
            attack_ms: vec![
                (None, 40.0),
                (Some(Articulation::Sustain), 40.0),
                (Some(Articulation::Legato), 40.0),
                (Some(Articulation::Pizzicato), 12.0),
            ],
            max_hold_s: Some(11.2),
            sampled: Some(range),
        }
    );
    assert_eq!(sound.caps, sound_caps(&shelf.presets[0]));

    // A part no installed preset plays gets a built-in sound, about which nothing is known.
    let tuba = snapshot.sound(
        &SoundRequest::new(InstrumentType::Tuba),
        ChainPreferences::default(),
    );
    assert!(
        matches!(tuba.plan.source, SoundSource::Builtin(_)),
        "{:?}",
        tuba.plan.source
    );
    assert_eq!(tuba.caps, SoundCaps::default());
    assert_eq!(snapshot.caps(&SoundSource::Silence), SoundCaps::default());

    assert_eq!(shelf.reads.get(), 1, "the snapshot answers every part");
    assert!(
        shelf.calls.borrow().is_empty(),
        "{:?}",
        shelf.calls.borrow()
    );
}

/// An empty catalog answers every part with a built-in sound or silence (TST-03).
#[test]
fn an_empty_catalog_plays_nothing_from_a_library() {
    let shelf = Shelf::new(Vec::new());
    let sound = Orchestrator::new(&shelf)
        .catalog()
        .sound(&violins(), ChainPreferences::default());
    assert!(
        matches!(
            sound.plan.source,
            SoundSource::Builtin(_) | SoundSource::Silence
        ),
        "{:?}",
        sound.plan.source
    );
    assert_eq!(sound.caps, SoundCaps::default());
}

/// The agent service over an application whose settings list `library` as a sample library.
struct Service {
    _dir: TempDir,
    service: AgentService,
}

impl Service {
    fn new(library: &Path) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
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
        Self { _dir: dir, service }
    }
}

fn ids(presets: &[InstrumentPreset]) -> Vec<&str> {
    presets.iter().map(|preset| preset.id.as_str()).collect()
}

/// The agent service reads the configured libraries as they are on disk at each read. Nothing
/// has checked their patches, so a sound's capabilities hold what the manifest claims and none of
/// the facts the check records: one velocity layer, no sampled keys, no measured attack or hold.
#[test]
fn the_agent_service_reads_the_configured_libraries_unchecked() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root: PathBuf = dir.path().join("Lib");
    library(&root);
    let world = Service::new(&root);

    let presets = CatalogReader::presets(&world.service);
    assert_eq!(ids(&presets), ["lib/Strings/Violins"]);
    assert!(presets.iter().all(|preset| preset.facts.is_none()));

    let orchestrator = Orchestrator::new(world.service);
    let sound = orchestrator
        .catalog()
        .sound(&violins(), ChainPreferences::default());
    assert_eq!(
        sound.caps,
        SoundCaps {
            articulations: violin_articulations(),
            legato: LegatoKind::Overlap { crossfade_ms: 90.0 },
            dynamics: DynamicsKind::Cc,
            attack_ms: vec![(Some(Articulation::Pizzicato), 12.0)],
            ..SoundCaps::default()
        }
    );

    // A patch added on disk is read at the next read.
    fs::write(root.join("Strings/Violas.sfz"), "<region> sample=a.wav\n").expect("patch");
    assert_eq!(
        ids(orchestrator.catalog().presets()),
        ["lib/Strings/Violas", "lib/Strings/Violins"]
    );
}
