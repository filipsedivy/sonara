//! The track headers and the mixer: the plugin pickers, and the edits the
//! headers and the channel strips send, through the edit boundary with the prototype's
//! undo steps.
//!
//! these prototype tests here (R), beside their
//! page tests (P): `header_buttons_rename_and_reorder` (the edits of M, S, R, the rename and the
//! reorder), `fader_drag_is_one_undo_step` and `pan_knob` (the moves of one gesture are one step
//! until the seal), `sends_add_level_pre_remove` (a send's edits, and a bus offers no send to
//! itself), `add_effect_from_menu_and_bypass_and_reorder` (the picker, the new device's ID, the
//! bypass and the reorder) and `mute_solo_arm_and_typed_db`. The tracks view, the refresh of a
//! mixer edit, equals the full view's tracks.

use serde_json::{Value, json};
use sonara_app::catalog::{PluginGroup, category_title, plugin_menu};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::edit::{self, Edit, EditOutcome};
use sonara_app::events::ErrorNotice;
use sonara_app::ids::IdSource;
use sonara_app::projects::NewProject;
use sonara_app::settings::Settings;
use sonara_app::view::{TrackView, project_view, tracks_view};
use sonara_core::ids::SequentialIds;
use sonara_core::plugins::PluginKind;
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
                name: "Mixer".to_owned(),
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

    fn track(&self, id: &str) -> TrackView {
        let view = project_view(self.app.session(), self.app.plugins());
        let project = view.project.expect("an open project");
        std::iter::once(project.master)
            .chain(project.tracks)
            .find(|track| track.id == id)
            .expect("the track")
    }

    fn order(&self) -> Vec<String> {
        let view = project_view(self.app.session(), self.app.plugins());
        view.project
            .expect("an open project")
            .tracks
            .into_iter()
            .map(|track| track.id)
            .collect()
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
}

fn titles(groups: &[PluginGroup]) -> Vec<(&str, Vec<&str>)> {
    groups
        .iter()
        .map(|group| {
            (
                group.title.as_str(),
                group.plugins.iter().map(|p| p.name.as_str()).collect(),
            )
        })
        .collect()
}

// -- the pickers -----------------------------------------------------------------------

#[test]
fn the_effect_picker_groups_every_effect_by_category() {
    let gui = Gui::new();
    let groups = plugin_menu(gui.app.plugins(), PluginKind::Effect);
    assert_eq!(
        titles(&groups),
        vec![
            ("Delay", vec!["Delay"]),
            ("Distortion", vec!["Saturation"]),
            ("Dynamics", vec!["Compressor", "Gate / Expander", "Limiter"]),
            ("EQ", vec!["Parametric EQ"]),
            ("Filter", vec!["Filter"]),
            ("Reverb", vec!["Convolution Reverb", "Reverb"]),
            ("Utility", vec!["Utility"]),
        ]
    );
    // Every effect of the registry, once, with its ID and description for the tooltip.
    let mut ids: Vec<&str> = groups
        .iter()
        .flat_map(|g| g.plugins.iter().map(|p| p.id.as_str()))
        .collect();
    ids.sort_unstable();
    let mut registered: Vec<&str> = gui
        .app
        .plugins()
        .all(Some(PluginKind::Effect))
        .iter()
        .map(|d| d.id.as_str())
        .collect();
    registered.sort_unstable();
    assert_eq!(ids, registered);
    let eq = &groups[3].plugins[0];
    assert_eq!(eq.id, "sonara.eq.parametric");
    assert_ne!(eq.description, "");
    assert_eq!(groups[2].category, "dynamics");
}

#[test]
fn the_instrument_picker_has_the_synthesizers_and_the_drums() {
    let gui = Gui::new();
    let groups = plugin_menu(gui.app.plugins(), PluginKind::Instrument);
    assert_eq!(
        titles(&groups),
        vec![
            ("Drums", vec!["Sonara Drum Kit"]),
            ("Synthesizers", vec!["Sonara Poly Synth"]),
        ]
    );
}

#[test]
fn a_category_without_a_title_is_title_cased_as_python_does() {
    assert_eq!(category_title("eq"), "EQ");
    assert_eq!(category_title("synth"), "Synthesizers");
    assert_eq!(category_title("multi band"), "Multi Band");
    assert_eq!(category_title("lo-fi"), "Lo-Fi");
    assert_eq!(category_title("MODULATION"), "Modulation");
    assert_eq!(category_title("x2y"), "X2Y");
}

// -- the track headers ----------------------------------------------------------------------

#[test]
fn the_header_buttons_rename_reorder_and_colour() {
    let gui = Gui::new();
    let a = gui.create("A", "instrument", Some("sonara.synth.poly"));
    let b = gui.create("B", "instrument", Some("sonara.synth.poly"));
    let c = gui.create("C", "instrument", Some("sonara.synth.poly"));
    // M, S and R, each a step of its own.
    gui.edit(json!({"op": "set_track_mixer", "track_id": a, "mute": true}));
    gui.edit(json!({"op": "set_track_mixer", "track_id": a, "solo": true}));
    gui.edit(json!({"op": "set_track_input", "track_id": a, "armed": true}));
    let track = gui.track(&a);
    assert!(track.mixer.mute && track.mixer.solo && track.input.armed);
    assert_eq!(gui.steps()[3..], ["Mute", "Solo", "Track Input"]);
    // The inline rename.
    gui.edit(json!({"op": "rename_track", "track_id": b, "name": "Bassline"}));
    assert_eq!(gui.track(&b).name, "Bassline");
    // C dragged above A is inserted before index 0 (`reorderIndex(2, 0)` is 0).
    gui.edit(json!({"op": "move_track", "track_id": c, "index": 0}));
    assert_eq!(gui.order(), [c.clone(), a.clone(), b.clone()]);
    // …and A dragged below the last row (target 3) lands last (`reorderIndex(1, 3)` is 2).
    gui.edit(json!({"op": "move_track", "track_id": a, "index": 2}));
    assert_eq!(gui.order(), [c.clone(), b.clone(), a.clone()]);
    // Colour… and Automatic Colour.
    gui.edit(json!({"op": "set_track_color", "track_id": c, "color": "#aabbcc"}));
    assert_eq!(gui.track(&c).color.as_deref(), Some("#aabbcc"));
    gui.edit(json!({"op": "set_track_color", "track_id": c, "color": null}));
    assert_eq!(gui.track(&c).color, None);
    // Add MIDI Clip at Playhead names the clip after the track; Delete Track.
    let at_playhead =
        json!({"op": "create_midi_clip", "track_id": b, "start": 4.0, "length": 4.0, "name": "B"});
    let clip = gui.edit(at_playhead);
    assert_eq!(clip.label, "Create MIDI Clip");
    gui.edit(json!({"op": "delete_track", "track_id": a}));
    assert_eq!(gui.order(), [c, b]);
    assert_eq!(
        gui.steps()[6..],
        [
            "Rename Track",
            "Move Track",
            "Move Track",
            "Track Color",
            "Track Color",
            "Create MIDI Clip",
            "Delete Track"
        ]
    );
}

#[test]
fn an_empty_or_unknown_header_edit_reports_its_code() {
    let gui = Gui::new();
    let a = gui.create("A", "audio", None);
    let code = |edit: Value| gui.try_edit(edit).expect_err("refused").code;
    assert_eq!(
        code(json!({"op": "rename_track", "track_id": "nope", "name": "X"})),
        "NOT_FOUND"
    );
    assert_eq!(
        code(json!({"op": "move_track", "track_id": a, "index": "first"})),
        "INVALID_ARGUMENT"
    );
    assert_eq!(
        code(json!({"op": "set_track_input", "track_id": "master", "armed": true})),
        "INVALID_ARGUMENT"
    );
}

// -- the channel strips ---------------------------------------------------------------------

#[test]
fn a_fader_or_pan_drag_is_one_step_until_the_seal() {
    let gui = Gui::new();
    let vox = gui.create("Vox", "audio", None);
    // One edit per move of the gesture, one step; the seal ends it.
    for db in [-1.0, -2.5, -4.0, -6.5] {
        gui.edit(json!({"op": "set_track_mixer", "track_id": vox, "volume_db": db}));
    }
    gui.seal();
    // The pan knob.
    for pan in [0.1, 0.2, 0.35] {
        gui.edit(json!({"op": "set_track_mixer", "track_id": vox, "pan": pan}));
    }
    gui.seal();
    // A typed value is a gesture of one edit and its seal.
    gui.edit(json!({"op": "set_track_mixer", "track_id": vox, "volume_db": -144.0}));
    gui.seal();
    // The toggles are steps of their own.
    gui.edit(json!({"op": "set_track_mixer", "track_id": vox, "mute": true}));
    gui.edit(json!({"op": "set_track_mixer", "track_id": vox, "solo": true}));
    assert_eq!(
        gui.steps(),
        [
            "Create Track",
            "Track Volume",
            "Track Pan",
            "Track Volume",
            "Mute",
            "Solo"
        ]
    );
    let track = gui.track(&vox);
    assert_eq!(track.mixer.volume_db, -144.0);
    assert_eq!(track.mixer.pan, 0.35);
    edit::undo(&gui.app).expect("undo the solo");
    edit::undo(&gui.app).expect("undo the mute");
    edit::undo(&gui.app).expect("undo the typed value");
    assert_eq!(gui.track(&vox).mixer.volume_db, -6.5);
    edit::undo(&gui.app).expect("undo the pan");
    edit::undo(&gui.app).expect("undo the drag");
    let track = gui.track(&vox);
    assert_eq!((track.mixer.volume_db, track.mixer.pan), (0.0, 0.0));
}

#[test]
fn inserts_add_select_bypass_and_reorder() {
    let gui = Gui::new();
    let keys = gui.create("Keys", "instrument", Some("sonara.synth.poly"));
    // The new device's ID selects it and shows the Devices tab.
    let add =
        |plugin: &str| gui.edit(json!({"op": "add_device", "track_id": keys, "plugin_id": plugin}));
    let added = add("sonara.eq.parametric");
    assert_eq!(added.label, "Add Effect");
    let device_id = |outcome: &EditOutcome| {
        outcome.result["device_id"]
            .as_str()
            .expect("a device ID")
            .to_owned()
    };
    let eq = device_id(&added);
    let comp = device_id(&add("sonara.dynamics.compressor"));
    let track = gui.track(&keys);
    assert_eq!(
        track.instrument.as_ref().map(|d| d.plugin_id.as_str()),
        Some("sonara.synth.poly")
    );
    let inserts: Vec<&str> = track.inserts.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(inserts, [eq.as_str(), comp.as_str()]);
    // The checkbox bypasses (then the seal); Enable again.
    gui.edit(json!({"op": "set_device_enabled", "device_id": eq, "enabled": false}));
    gui.seal();
    assert!(!gui.track(&keys).inserts[0].enabled);
    gui.edit(json!({"op": "set_device_enabled", "device_id": eq, "enabled": true}));
    // The compressor dropped on the EQ's row (insert index 0) moves first.
    gui.edit(json!({"op": "move_device", "device_id": comp, "index": 0}));
    let order: Vec<String> = gui.track(&keys).inserts.into_iter().map(|d| d.id).collect();
    assert_eq!(order, [comp.clone(), eq.clone()]);
    // The instrument never moves, and a remove takes the device out.
    let instrument = gui.track(&keys).instrument.expect("the synth").id;
    assert_eq!(
        gui.try_edit(json!({"op": "move_device", "device_id": instrument, "index": 1}))
            .expect_err("refused")
            .code,
        "INVALID_ARGUMENT"
    );
    gui.edit(json!({"op": "remove_device", "device_id": comp}));
    assert_eq!(gui.track(&keys).inserts.len(), 1);
    assert_eq!(
        gui.steps()[1..],
        [
            "Add Effect",
            "Add Effect",
            "Bypass Effect",
            "Bypass Effect",
            "Reorder Effects",
            "Remove Effect"
        ]
    );
}

#[test]
fn sends_add_level_pre_and_remove() {
    let gui = Gui::new();
    let vox = gui.create("Vox", "audio", None);
    let verb = gui.create("Verb", "bus", None);
    let delay = gui.create("Delay", "bus", None);
    // The "+" menu: the buses, never the track itself nor one it already sends to.
    let targets = |id: &str| -> Vec<String> {
        gui.track(id)
            .send_targets
            .into_iter()
            .map(|t| t.name)
            .collect()
    };
    assert_eq!(targets(&vox), ["Verb", "Delay"]);
    assert_eq!(targets(&verb), ["Delay"]);
    assert_eq!(targets("master"), Vec::<String>::new());
    // A new send at −12 dB.
    gui.edit(json!({"op": "add_send", "track_id": vox, "target_id": verb, "level_db": -12.0}));
    let send = gui.track(&vox).sends[0].clone();
    assert_eq!(
        (send.target_id.as_str(), send.level_db),
        (verb.as_str(), -12.0)
    );
    assert_eq!(targets(&vox), ["Delay"]);
    // The level knob's moves are one step until the seal; Pre is a step and a seal.
    for db in [-10.0, -8.0, -6.0] {
        gui.edit(
            json!({"op": "update_send", "track_id": vox, "send_id": send.id,
                        "level_db": db}),
        );
    }
    gui.seal();
    gui.edit(
        json!({"op": "update_send", "track_id": vox, "send_id": send.id,
                    "pre_fader": true}),
    );
    gui.seal();
    let updated = gui.track(&vox).sends[0].clone();
    assert_eq!((updated.level_db, updated.pre_fader), (-6.0, true));
    gui.edit(json!({"op": "remove_send", "track_id": vox, "send_id": send.id}));
    assert_eq!(gui.track(&vox).sends, Vec::new());
    assert_eq!(
        gui.steps()[3..],
        ["Add Send", "Send Level", "Send Level", "Remove Send"]
    );
    // A bus cannot send to itself.
    assert_eq!(
        gui.try_edit(
            json!({"op": "add_send", "track_id": delay, "target_id": delay,
                            "level_db": -12.0})
        )
        .expect_err("refused")
        .code,
        "ROUTING_CYCLE"
    );
}

// -- the tracks view (the refresh of a mixer edit) -----------------------------------------------

#[test]
fn a_tracks_view_equals_the_full_views_tracks() {
    let gui = Gui::new();
    let vox = gui.create("Vox", "audio", None);
    let verb = gui.create("Verb", "bus", None);
    gui.edit(json!({"op": "add_send", "track_id": vox, "target_id": verb, "level_db": -12.0}));
    gui.edit(json!({"op": "set_track_mixer", "track_id": vox, "volume_db": -7.5}));
    gui.edit(json!({"op": "set_track_mixer", "track_id": "master", "mute": true}));
    let full = project_view(gui.app.session(), gui.app.plugins());
    let open = full.project.expect("an open project");
    // The master last, whatever the order asked; an unknown ID is left out.
    let ids = ["master".to_owned(), "nope".to_owned(), vox.clone()];
    let delta = tracks_view(gui.app.session(), gui.app.plugins(), &ids);
    assert_eq!(delta.version, full.version);
    let tracks = delta.project.expect("an open project");
    assert_eq!(tracks.id, open.id);
    assert_eq!(tracks.dirty, open.dirty);
    assert_eq!(tracks.history, open.history);
    assert_eq!(tracks.history.undo.as_deref(), Some("Mute"));
    assert_eq!(tracks.meter_layout, open.meter_layout);
    let vox_view = open.tracks.iter().find(|t| t.id == vox).expect("Vox");
    assert_eq!(tracks.tracks, [vox_view.clone(), open.master.clone()]);
    assert_eq!(tracks.tracks[0].mixer.volume_db, -7.5);
    assert!(tracks.tracks[1].mixer.mute);
    // Without a project there is nothing to show.
    gui.app.projects().close(true).expect("closed");
    let closed = tracks_view(gui.app.session(), gui.app.plugins(), &ids);
    assert_eq!(closed.project, None);
}
