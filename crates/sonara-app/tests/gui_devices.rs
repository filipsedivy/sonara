//! The device editors: the descriptors the editors are generated from, and
//! the edits the cards send, through the edit boundary with the prototype's undo
//! steps.
//!
//! The page builds every editor from the project view's `PluginView`s and tests that against
//! `golden/plugins/catalog.json` (`apps/sonara-desktop/tests/unit/devices.test.ts` and
//! `tests/page/devices.spec.ts`, which read its copy `tests/fixtures/plugin-catalog.json`). Here
//! the copy is checked to be the golden file, and the view's descriptor of every built-in plugin
//! to be the golden descriptor mapped as the page's `tests/support/plugins.ts` maps it.
//!
//! these prototype tests here (R), beside their page
//! tests (P): `every_builtin_plugin_has_a_complete_editor` (the descriptors),
//! `param_edit_preset_bypass_and_remove` (the knob, the choice, the preset, the bypass, the remove,
//! with their undo steps) and `knob_drag_and_int_quantize` (an int parameter stores its whole
//! number). The devices view, the refresh of a device edit, equals the full view's devices, and
//! the edits it refreshes carry the change set the page's `changedDevices` takes.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Map, Value, json};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::edit::{self, Edit, EditOutcome};
use sonara_app::events::ErrorNotice;
use sonara_app::ids::IdSource;
use sonara_app::projects::NewProject;
use sonara_app::settings::Settings;
use sonara_app::view::{DeviceView, OpenProject, TrackView, devices_view, project_view};
use sonara_core::ids::SequentialIds;
use sonara_core::plugins::PluginKind;
use sonara_testkit::Golden;
use tempfile::TempDir;

/// An application on the null backend with sequential IDs and a new project.
struct Gui {
    app: AppContext,
    _dir: TempDir,
}

impl Gui {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let app = AppContext::create(AppOptions {
            workspaces: Some(dir.path().join("ws")),
            settings: Some(Settings::default()),
            persist_settings: false,
            audio_backend: Some("null".to_owned()),
            ids: Some(IdSource::new(Box::new(SequentialIds::new(0)))),
            ..AppOptions::default()
        })
        .expect("the application");
        app.projects()
            .new_project(&NewProject {
                name: "Devices".to_owned(),
                ..NewProject::default()
            })
            .expect("a project");
        Self { app, _dir: dir }
    }

    fn try_edit(&self, edit: Value) -> Result<EditOutcome, ErrorNotice> {
        edit::apply(&self.app, Edit::from_json(edit)?)
    }

    fn edit(&self, edit: Value) -> EditOutcome {
        let shown = edit.to_string();
        self.try_edit(edit)
            .unwrap_or_else(|error| panic!("{shown}: {error:?}"))
    }

    fn create(&self, name: &str, kind: &str, instrument: Option<&str>) -> String {
        let outcome = self.edit(json!({"op": "create_track", "name": name, "kind": kind,
                                       "instrument": instrument}));
        outcome.result["track_id"]
            .as_str()
            .expect("a track ID")
            .to_owned()
    }

    /// "+ Effect"; the new device's ID, which the panel makes the current device.
    fn effect(&self, track: &str, plugin: &str) -> String {
        let outcome =
            self.edit(json!({"op": "add_device", "track_id": track, "plugin_id": plugin}));
        assert_eq!(outcome.label, "Add Effect");
        outcome.result["device_id"]
            .as_str()
            .expect("a device ID")
            .to_owned()
    }

    fn open(&self) -> OpenProject {
        project_view(self.app.session(), self.app.plugins())
            .project
            .expect("an open project")
    }

    fn track(&self, id: &str) -> TrackView {
        let project = self.open();
        std::iter::once(project.master)
            .chain(project.tracks)
            .find(|track| track.id == id)
            .expect("the track")
    }

    fn device(&self, track: &str, id: &str) -> DeviceView {
        let track = self.track(track);
        track
            .instrument
            .into_iter()
            .chain(track.inserts)
            .find(|device| device.id == id)
            .expect("the device")
    }

    /// The undo stack's labels, oldest first.
    fn steps(&self) -> Vec<String> {
        let session = self.app.session().lock();
        session
            .history()
            .labels()
            .0
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    fn seal(&self) {
        edit::seal_undo(&self.app);
    }

    fn undo(&self) {
        edit::undo(&self.app).expect("a step to undo");
    }
}

fn instrument_id(track: &TrackView) -> String {
    track.instrument.as_ref().expect("an instrument").id.clone()
}

// -- the descriptors -------------------------------------------------------------------------------

/// The desktop's copy of the golden catalog, which the page tests read.
fn desktop_catalog() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/sonara-desktop/tests/fixtures/plugin-catalog.json");
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[test]
fn the_desktops_catalog_is_the_golden_one() {
    let golden = Golden::open().expect("the golden data");
    let bytes = golden
        .bytes("plugins/catalog.json")
        .expect("the plugin catalog");
    assert!(
        bytes == desktop_catalog(),
        "apps/sonara-desktop/tests/fixtures/plugin-catalog.json differs from golden/plugins/catalog.json"
    );
}

/// A golden descriptor as the project view carries it: `tests/support/plugins.ts`' `pluginView`.
fn plugin_view(descriptor: &Value) -> Value {
    let params: Vec<Value> = descriptor["params"]
        .as_array()
        .expect("params")
        .iter()
        .map(|param| {
            json!({
                "id": param["id"],
                "name": param["name"],
                "kind": param["kind"],
                "default": param["default"],
                "min": param["minimum"],
                "max": param["maximum"],
                "unit": param["unit"],
                "choices": param["choices"],
                "log_scale": param["scale"] == "log",
                "automatable": param["automatable"],
                "description": param["description"],
            })
        })
        .collect();
    let presets: Vec<Value> = descriptor["presets"]
        .as_array()
        .expect("presets")
        .iter()
        .map(|preset| preset["name"].clone())
        .collect();
    json!({
        "id": descriptor["id"],
        "name": descriptor["name"],
        "kind": descriptor["kind"],
        "category": descriptor["category"],
        "description": descriptor["description"],
        "sidechain": descriptor["sidechain"],
        "presets": presets,
        "params": params,
    })
}

/// `test_every_builtin_plugin_has_a_complete_editor`: with every built-in plugin in use, the view
/// carries each one's descriptor, and it is the golden catalog's (the editors' fixture), field by
/// field; every device's parameters are resolved, so each editor control has its value.
#[test]
fn the_views_descriptors_are_the_golden_catalogs() {
    let gui = Gui::new();
    let fx = gui.create("FX", "audio", None);
    let effects: Vec<String> = gui
        .app
        .plugins()
        .all(Some(PluginKind::Effect))
        .into_iter()
        .map(|d| d.id.clone())
        .collect();
    assert_eq!(effects.len(), 10, "10 effects");
    for plugin in &effects {
        gui.effect(&fx, plugin);
    }
    gui.create("Synth", "instrument", Some("sonara.synth.poly"));
    gui.create("Drums", "instrument", Some("sonara.drums"));
    let open = gui.open();
    let golden: Vec<Value> = Golden::open()
        .expect("the golden data")
        .json("plugins/catalog.json")
        .expect("the plugin catalog");
    let ids: BTreeSet<&str> = golden
        .iter()
        .map(|d| d["id"].as_str().expect("an ID"))
        .collect();
    assert_eq!(
        open.plugins
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        ids,
        "every built-in plugin is in use"
    );
    for descriptor in &golden {
        let id = descriptor["id"].as_str().expect("an ID");
        let view = serde_json::to_value(&open.plugins[id]).expect("serializable");
        assert_eq!(view, plugin_view(descriptor), "{id}");
    }
    // Each device's parameters: every descriptor parameter, in order, at its default.
    let devices = open
        .tracks
        .iter()
        .flat_map(|track| track.instrument.iter().chain(&track.inserts));
    for device in devices {
        let descriptor = golden
            .iter()
            .find(|d| d["id"] == device.plugin_id.as_str())
            .expect("a built-in plugin");
        let expected: Map<String, Value> = descriptor["params"]
            .as_array()
            .expect("params")
            .iter()
            .map(|p| {
                (
                    p["id"].as_str().expect("an ID").to_owned(),
                    p["default"].clone(),
                )
            })
            .collect();
        assert_eq!(device.params, expected, "{}", device.plugin_id);
        assert!(device.available);
    }
}

// -- the cards' edits ------------------------------------------------------------------

/// `test_param_edit_preset_bypass_and_remove`, with the undo steps of each gesture.
#[test]
fn a_cards_edits_keep_the_prototypes_undo_steps() {
    let gui = Gui::new();
    let bass = gui.create("Bass", "instrument", Some("sonara.synth.poly"));
    let synth = instrument_id(&gui.track(&bass));
    // A knob's moves are one step until the seal.
    for hz in [2600.0, 2800.0, 3100.0, 3500.0] {
        let outcome = gui.edit(json!({"op": "set_device_params", "device_id": synth,
                                      "values": {"filter_cutoff": hz}}));
        assert_eq!(outcome.label, "Set Parameter");
    }
    gui.seal();
    // An int knob stores its whole number (`knob_drag_and_int_quantize`).
    gui.edit(json!({"op": "set_device_params", "device_id": synth, "values": {"polyphony": 8}}));
    gui.seal();
    // A choice is a step of its own.
    gui.edit(json!({"op": "set_device_params", "device_id": synth,
                    "values": {"voice_mode": "mono"}}));
    gui.seal();
    // A preset loads its values over the defaults.
    let outcome = gui.edit(json!({"op": "apply_preset", "device_id": synth,
                                  "preset": "Analog Bass"}));
    assert_eq!(outcome.label, "Load Preset");
    gui.seal();
    let device = gui.device(&bass, &synth);
    assert_eq!(device.preset.as_deref(), Some("Analog Bass"));
    assert_eq!(device.params["osc2_octave"], json!(-1));
    // On/Off.
    let outcome = gui.edit(json!({"op": "set_device_enabled", "device_id": synth,
                                  "enabled": false}));
    assert_eq!(outcome.label, "Bypass Effect");
    gui.seal();
    assert!(!gui.device(&bass, &synth).enabled);
    // DV-07 and DV-06: an effect added from "+ Effect", then removed from its card's menu.
    let delay = gui.effect(&bass, "sonara.delay");
    let outcome = gui.edit(json!({"op": "remove_device", "device_id": delay}));
    assert_eq!(outcome.label, "Remove Effect");
    assert_eq!(gui.track(&bass).inserts, []);
    assert_eq!(
        gui.steps(),
        [
            "Create Track",
            "Set Parameter",
            "Set Parameter",
            "Set Parameter",
            "Load Preset",
            "Bypass Effect",
            "Add Effect",
            "Remove Effect"
        ]
    );
    // One undo per step: the remove, the add, the bypass, the preset, the choice, the int knob,
    // then the whole drag.
    for _ in 0..4 {
        gui.undo();
    }
    let device = gui.device(&bass, &synth);
    assert!(device.enabled);
    assert_eq!(device.preset, None);
    assert_eq!(device.params["voice_mode"], json!("mono"));
    gui.undo();
    assert_eq!(
        gui.device(&bass, &synth).params["voice_mode"],
        json!("poly")
    );
    gui.undo();
    assert_eq!(gui.device(&bass, &synth).params["polyphony"], json!(16));
    assert_eq!(
        gui.device(&bass, &synth).params["filter_cutoff"],
        json!(3500.0)
    );
    gui.undo();
    assert_eq!(
        gui.device(&bass, &synth).params["filter_cutoff"],
        json!(2500.0)
    );
}

/// Reset to Defaults is one step that puts every parameter back; Replace Instrument puts a
/// new device of the chosen plugin first in the chain.
#[test]
fn reset_and_replace_instrument() {
    let gui = Gui::new();
    let vox = gui.create("Vox", "audio", None);
    let comp = gui.effect(&vox, "sonara.dynamics.compressor");
    gui.edit(json!({"op": "set_device_params", "device_id": comp,
                    "values": {"threshold_db": -30.0, "ratio": 8.0, "auto_makeup": true}}));
    gui.seal();
    let outcome = gui.edit(json!({"op": "reset_device", "device_id": comp}));
    assert_eq!(outcome.label, "Set Parameter");
    gui.seal();
    let device = gui.device(&vox, &comp);
    assert_eq!(device.params["threshold_db"], json!(-18.0));
    assert_eq!(device.params["ratio"], json!(4.0));
    assert_eq!(device.params["auto_makeup"], json!(false));
    gui.undo();
    assert_eq!(gui.device(&vox, &comp).params["threshold_db"], json!(-30.0));

    let keys = gui.create("Keys", "instrument", Some("sonara.synth.poly"));
    let before = instrument_id(&gui.track(&keys));
    let outcome = gui.edit(json!({"op": "set_instrument", "track_id": keys,
                                  "plugin_id": "sonara.drums"}));
    assert_eq!(outcome.label, "Set Instrument");
    let track = gui.track(&keys);
    let drums = track.instrument.as_ref().expect("an instrument");
    assert_eq!(drums.plugin_id, "sonara.drums");
    assert_ne!(
        drums.id, before,
        "a new device, so the panel makes a new card"
    );
    gui.undo();
    assert_eq!(instrument_id(&gui.track(&keys)), before);
    // A refused value keeps its code for the status bar.
    let refused = gui
        .try_edit(json!({"op": "set_device_params", "device_id": comp,
                         "values": {"ratio": 100.0}}))
        .expect_err("out of range");
    assert_eq!(refused.code, "INVALID_ARGUMENT");
}

// -- the devices view (the refresh of a device edit) ---------------------------------------------

/// The change set of `outcome` as the page's `changedDevices` reads it: whether only devices were
/// updated, with the flag `devices` alone.
fn devices_only(outcome: &EditOutcome) -> bool {
    let changes = &outcome.changes;
    changes.created.is_empty()
        && changes.deleted.is_empty()
        && changes.flags == ["devices"]
        && !changes.updated.is_empty()
        && changes.updated.iter().all(|object| object.kind == "device")
}

#[test]
fn a_devices_view_equals_the_full_views_devices() {
    let gui = Gui::new();
    let bass = gui.create("Bass", "instrument", Some("sonara.synth.poly"));
    let synth = instrument_id(&gui.track(&bass));
    let comp = gui.effect(&bass, "sonara.dynamics.compressor");
    let limit = gui.effect("master", "sonara.dynamics.limiter");
    // A knob, On/Off and a preset change only their device: the page refreshes just it.
    let knob = gui.edit(json!({"op": "set_device_params", "device_id": comp,
                               "values": {"threshold_db": -30.0}}));
    assert!(devices_only(&knob), "{:?}", knob.changes);
    assert!(
        knob.changes.structure_changed,
        "structural for the prototype's panels"
    );
    gui.seal();
    let preset =
        gui.edit(json!({"op": "apply_preset", "device_id": synth, "preset": "Analog Bass"}));
    assert!(devices_only(&preset), "{:?}", preset.changes);
    gui.seal();
    let bypass =
        gui.edit(json!({"op": "set_device_enabled", "device_id": limit, "enabled": false}));
    assert!(devices_only(&bypass), "{:?}", bypass.changes);
    gui.seal();
    let full = project_view(gui.app.session(), gui.app.plugins());
    let open = full.project.expect("an open project");
    let ids = [
        limit.clone(),
        "nope".to_owned(),
        comp.clone(),
        synth.clone(),
    ];
    let delta = devices_view(gui.app.session(), gui.app.plugins(), &ids);
    assert_eq!(delta.version, full.version);
    let devices = delta.project.expect("an open project");
    assert_eq!(devices.id, open.id);
    assert_eq!(devices.dirty, open.dirty);
    assert_eq!(devices.history, open.history);
    assert_eq!(devices.history.undo.as_deref(), Some("Bypass Effect"));
    assert_eq!(devices.meter_layout, open.meter_layout);
    // In the project's order: the instrument first, the master's last; an unknown ID left out.
    let placed: Vec<(&str, &str)> = devices
        .devices
        .iter()
        .map(|p| (p.track_id.as_str(), p.device.id.as_str()))
        .collect();
    assert_eq!(
        placed,
        [
            (bass.as_str(), synth.as_str()),
            (bass.as_str(), comp.as_str()),
            ("master", limit.as_str())
        ]
    );
    let bass_view = open.tracks.iter().find(|t| t.id == bass).expect("Bass");
    assert_eq!(
        Some(&devices.devices[0].device),
        bass_view.instrument.as_ref()
    );
    assert_eq!(devices.devices[1].device, bass_view.inserts[0]);
    assert_eq!(devices.devices[2].device, open.master.inserts[0]);
    assert_eq!(
        devices.devices[1].device.params["threshold_db"],
        json!(-30.0)
    );
    assert!(!devices.devices[2].device.enabled);
    // What adds, removes, moves or keys a device needs the whole view.
    let delay = gui.effect(&bass, "sonara.delay");
    let moved = gui.edit(json!({"op": "move_device", "device_id": delay, "index": 0}));
    assert!(!devices_only(&moved), "{:?}", moved.changes);
    let keys = gui.create("Keys", "instrument", Some("sonara.drums"));
    let keyed =
        gui.edit(json!({"op": "set_sidechain", "device_id": comp, "source_track_id": keys}));
    assert!(!devices_only(&keyed), "{:?}", keyed.changes);
    let removed = gui.edit(json!({"op": "remove_device", "device_id": delay}));
    assert!(!devices_only(&removed), "{:?}", removed.changes);
    // Without a project there is nothing to show.
    gui.app.projects().close(true).expect("closed");
    let closed = devices_view(gui.app.session(), gui.app.plugins(), &ids);
    assert_eq!(closed.project, None);
}
