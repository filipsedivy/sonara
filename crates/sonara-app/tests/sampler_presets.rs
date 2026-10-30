//! The sampler's `instrument` through the whole application (TST-02): the choices are preset
//! IDs, a project stores the ID, a project from before preset IDs that stores the patch's
//! absolute path still opens and plays the same patch, and an edit that passes a path stores the
//! ID. Also its `instrument_type`, `articulation` and `dynamics`: what a new device stores, which
//! of them automation may change, and that a choice saved by a newer version opens and plays as
//! `auto`. The library is generated in a temporary directory; nothing licensed is needed.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::edit::{self, Edit, EditOutcome};
use sonara_app::ids::IdSource;
use sonara_app::projects::NewProject;
use sonara_app::samples::LIBRARIES_KEY;
use sonara_app::settings::Settings;
use sonara_core::commands::automation::SetAutomation;
use sonara_core::error::ErrorCode;
use sonara_core::ids::SequentialIds;
use sonara_core::model::automation::{AutomationPoint, CurveKind};
use sonara_core::model::device::ParamValue;
use sonara_engine::{RenderRequest, RenderedAudio, Tail};
use tempfile::TempDir;

/// A 16-bit mono WAV of a 220 Hz sine, half a second long at 44.1 kHz.
fn write_wav(path: &Path) {
    let frames = 22_050_u32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + frames * 2).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    for field in [16_u32, 0x0001_0001, 44_100, 88_200] {
        bytes.extend_from_slice(&field.to_le_bytes());
    }
    bytes.extend_from_slice(&0x0010_0002_u32.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(frames * 2).to_le_bytes());
    for i in 0..frames {
        let x = (f64::from(i) / 44_100.0 * 220.0 * std::f64::consts::TAU).sin();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the amplitude 16000 fits an i16"
        )]
        let value = (x * 16_000.0).round() as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(path, bytes).expect("write the wav");
}

/// A library `Lib` with two patches, `Keys/Tone.sfz` and `Keys/Other.sfz`, of one sample.
fn library(dir: &Path) -> PathBuf {
    let root = dir.join("Lib");
    fs::create_dir_all(root.join("Keys")).expect("mkdir");
    write_wav(&root.join("Keys/tone.wav"));
    for name in ["Tone", "Other"] {
        fs::write(
            root.join(format!("Keys/{name}.sfz")),
            "<region> sample=tone.wav lokey=0 hikey=127 pitch_keycenter=57 ampeg_release=0.05\n",
        )
        .expect("write the patch");
    }
    root
}

/// An application on the null backend whose settings list `root` as a sample library, with a
/// new project.
struct App {
    app: AppContext,
    dir: TempDir,
}

impl App {
    fn new(root: &Path) -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let mut settings = Settings::default();
        settings.extra.insert(
            LIBRARIES_KEY.to_owned(),
            json!([root.to_str().expect("UTF-8")]),
        );
        let app = AppContext::create(AppOptions {
            workspaces: Some(dir.path().join("ws")),
            settings: Some(settings),
            persist_settings: false,
            audio_backend: Some("null".to_owned()),
            ids: Some(IdSource::new(Box::new(SequentialIds::new(0)))),
            ..AppOptions::default()
        })
        .expect("the application");
        app.projects()
            .new_project(&NewProject {
                name: "Sampler".to_owned(),
                ..NewProject::default()
            })
            .expect("a project");
        Self { app, dir }
    }

    fn edit(&self, edit: Value) -> EditOutcome {
        let shown = edit.to_string();
        edit::apply(&self.app, Edit::from_json(edit).expect("an edit"))
            .unwrap_or_else(|error| panic!("{shown}: {error:?}"))
    }

    /// A sampler track playing one note; the sampler's device ID.
    fn sampler_track(&self) -> String {
        let track = self.edit(json!({"op": "create_track", "name": "Keys",
                                     "kind": "instrument", "instrument": "sonara.sampler"}));
        let track_id = track.result["track_id"]
            .as_str()
            .expect("a track")
            .to_owned();
        let clip = self.edit(json!({"op": "create_midi_clip", "track_id": track_id,
                                    "start": 0.0, "length": 2.0, "name": "Note"}));
        let clip_id = clip.result["clip_id"].as_str().expect("a clip").to_owned();
        self.edit(json!({"op": "add_notes", "clip_id": clip_id,
                         "notes": [{"pitch": 57, "start": 0.0, "length": 1.0, "velocity": 100}]}));
        self.instrument_device()
    }

    /// The ID of the first track's instrument.
    fn instrument_device(&self) -> String {
        let session = self.app.session().lock();
        let project = session.require_project().expect("a project");
        project
            .tracks
            .iter()
            .find_map(|track| track.instrument.as_ref())
            .expect("an instrument")
            .id
            .clone()
    }

    /// The stored `instrument` value of `device_id`.
    fn stored_instrument(&self, device_id: &str) -> ParamValue {
        self.stored(device_id, "instrument")
    }

    /// The stored value of parameter `param` of `device_id`.
    fn stored(&self, device_id: &str, param: &str) -> ParamValue {
        let session = self.app.session().lock();
        let project = session.require_project().expect("a project");
        let (_, device) = project.device(device_id).expect("the device");
        device.params[param].clone()
    }

    /// Writes an automation lane of `param` on the sampler `device_id` with one point of
    /// `value` per beat, through the session's own command.
    fn automate(
        &self,
        device_id: &str,
        param: &str,
        values: &[f64],
    ) -> Result<(), sonara_core::error::DomainError> {
        let mut session = self.app.session().lock();
        let track_id = session
            .require_project()
            .expect("a project")
            .device(device_id)
            .expect("the device")
            .0
            .id
            .clone();
        let points = values
            .iter()
            .zip(0_u32..)
            .map(|(&value, beat)| {
                AutomationPoint::new(f64::from(beat), value, CurveKind::Hold).expect("a point")
            })
            .collect();
        let command = SetAutomation {
            track_id,
            device_id: Some(device_id.to_owned()),
            param: param.to_owned(),
            points,
            start: None,
            end: None,
            lane_id: format!("aut_{param}"),
        };
        session.execute(&command, false, "test").map(|_| ())
    }

    fn render(&self) -> RenderedAudio {
        self.app
            .render()
            .render_array(&RenderRequest {
                end_beat: Some(2.0),
                tail: Tail::None,
                ..RenderRequest::default()
            })
            .expect("renders")
    }
}

fn peak(audio: &RenderedAudio) -> f64 {
    audio
        .data
        .iter()
        .flatten()
        .fold(0.0, |p, v| f64::max(p, v.abs()))
}

fn path_text(path: &Path) -> String {
    path.to_str().expect("UTF-8").to_owned()
}

#[test]
fn the_sampler_offers_and_stores_preset_ids() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = library(dir.path());
    let app = App::new(&root);
    let sampler = app.app.plugins().get("sonara.sampler").expect("registered");
    let instrument = sampler.param("instrument").expect("the parameter");
    assert_eq!(instrument.choices, ["lib/Keys/Other", "lib/Keys/Tone"]);
    let device = app.sampler_track();
    assert_eq!(
        app.stored_instrument(&device),
        ParamValue::Str("lib/Keys/Other".into()),
        "a new sampler plays the first preset"
    );
    app.edit(json!({"op": "set_device_params", "device_id": device,
                    "values": {"instrument": "lib/Keys/Tone"}}));
    assert_eq!(
        app.stored_instrument(&device),
        ParamValue::Str("lib/Keys/Tone".into())
    );
    let audio = app.render();
    assert!(
        audio.stats.warnings.is_empty(),
        "{:?}",
        audio.stats.warnings
    );
    assert!(peak(&audio) > 0.01, "the preset plays: {}", peak(&audio));
}

#[test]
fn an_edit_that_passes_a_patch_path_stores_the_preset_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = library(dir.path());
    let app = App::new(&root);
    let device = app.sampler_track();
    app.edit(json!({"op": "set_device_params", "device_id": device,
                    "values": {"instrument": path_text(&root.join("Keys/Tone.sfz"))}}));
    assert_eq!(
        app.stored_instrument(&device),
        ParamValue::Str("lib/Keys/Tone".into())
    );
    // A path that names nothing installed is refused with the choices (TST-03).
    let refused = edit::apply(
        &app.app,
        Edit::from_json(json!({"op": "set_device_params", "device_id": device,
                               "values": {"instrument": "/elsewhere/Lib/Keys/Tone.sfz"}}))
        .expect("an edit"),
    )
    .expect_err("not a choice");
    assert!(
        refused
            .message
            .contains("must be one of ['lib/Keys/Other', 'lib/Keys/Tone']"),
        "{refused:?}"
    );
    assert_eq!(
        app.stored_instrument(&device),
        ParamValue::Str("lib/Keys/Tone".into()),
        "the refused edit changed nothing"
    );
}

#[test]
fn a_project_that_stores_an_absolute_path_opens_and_plays_the_same_patch() {
    // A project saved before preset IDs stores the patch's absolute path. It opens as it is,
    // plays the preset of that patch without a warning, and keeps the path until an edit.
    let dir = tempfile::tempdir().expect("tempdir");
    let root = library(dir.path());
    let app = App::new(&root);
    let device = app.sampler_track();
    app.edit(json!({"op": "set_device_params", "device_id": device,
                    "values": {"instrument": "lib/Keys/Tone"}}));
    let by_id = app.render();
    assert!(peak(&by_id) > 0.01, "the preset plays");

    let bundle = app.dir.path().join("Old.sonara");
    app.app
        .projects()
        .save(Some(&bundle), Some(false))
        .expect("saved");
    let file = bundle.join("project.json");
    let old_path = path_text(&root.join("Keys/Tone.sfz"));
    let text = fs::read_to_string(&file).expect("the project file");
    assert!(text.contains("\"lib/Keys/Tone\""), "the ID is stored");
    fs::write(
        &file,
        text.replace(
            "\"lib/Keys/Tone\"",
            &serde_json::to_string(&old_path).expect("JSON"),
        ),
    )
    .expect("write the old project");

    app.app.projects().close(true).expect("closed");
    app.app
        .projects()
        .open_project(&bundle, false)
        .expect("opens");
    assert_eq!(
        app.stored_instrument(&device),
        ParamValue::Str(old_path.clone()),
        "opening does not rewrite the project"
    );
    let by_path = app.render();
    assert!(
        by_path.stats.warnings.is_empty(),
        "{:?}",
        by_path.stats.warnings
    );
    assert_eq!(
        by_path.data, by_id.data,
        "the old path plays the same patch"
    );

    // An instrument that is not installed here plays silence with a warning instead of
    // failing the render (ADR 0010).
    let text = fs::read_to_string(&file).expect("the project file");
    let elsewhere = "/Users/someone-else/Lib/Keys/Tone.sfz";
    fs::write(
        &file,
        text.replace(
            &serde_json::to_string(&old_path).expect("JSON"),
            &serde_json::to_string(elsewhere).expect("JSON"),
        ),
    )
    .expect("write the project");
    app.app.projects().close(true).expect("closed");
    app.app
        .projects()
        .open_project(&bundle, false)
        .expect("opens");
    let silent = app.render();
    assert_eq!(
        silent.stats.warnings,
        [format!(
            "instrument '{elsewhere}' of 'Sonara Sampler' is not installed; it plays silence"
        )]
    );
    assert!(peak(&silent) < 1e-9, "silence: {}", peak(&silent));
}

#[test]
fn a_new_sampler_stores_auto_and_full_dynamics() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app = App::new(&library(dir.path()));
    let device = app.sampler_track();
    assert_eq!(
        app.stored(&device, "instrument_type"),
        ParamValue::Str("auto".into())
    );
    assert_eq!(
        app.stored(&device, "articulation"),
        ParamValue::Str("auto".into())
    );
    assert_eq!(app.stored(&device, "dynamics"), ParamValue::Float(1.0));
    app.edit(json!({"op": "set_device_params", "device_id": device,
                    "values": {"instrument_type": "horn_f", "articulation": "pizzicato",
                               "dynamics": 0.25}}));
    assert_eq!(
        app.stored(&device, "articulation"),
        ParamValue::Str("pizzicato".into())
    );
    assert_eq!(
        app.stored(&device, "instrument_type"),
        ParamValue::Str("horn_f".into())
    );
    assert_eq!(app.stored(&device, "dynamics"), ParamValue::Float(0.25));
    // TST-03: a value that is not a choice is refused with the choices, as for every enum.
    let refused = edit::apply(
        &app.app,
        Edit::from_json(json!({"op": "set_device_params", "device_id": device,
                               "values": {"articulation": "pizz"}}))
        .expect("an edit"),
    )
    .expect_err("not a choice");
    assert_eq!(refused.code, ErrorCode::InvalidArgument.as_str());
    assert!(
        refused.message.starts_with(
            "parameter 'articulation' must be one of ['auto', 'ordinario', 'sustain',"
        ),
        "{refused:?}"
    );
}

#[test]
fn articulation_and_dynamics_are_automatable_and_the_instrument_type_is_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app = App::new(&library(dir.path()));
    let device = app.sampler_track();
    // A lane of an enum holds choice indices: 0 (`auto`) to 78, the last articulation.
    app.automate(&device, "articulation", &[0.0, 4.0, 78.0])
        .expect("an articulation lane");
    app.automate(&device, "dynamics", &[1.0, 0.5, 0.0])
        .expect("a dynamics lane");
    let beyond = app
        .automate(&device, "articulation", &[79.0])
        .expect_err("past the last choice");
    assert_eq!(beyond.code, ErrorCode::InvalidArgument);
    assert_eq!(
        beyond.message,
        "automation value 79 at beat 0 is outside 0..78"
    );
    let refused = app
        .automate(&device, "instrument_type", &[1.0])
        .expect_err("it names the instrument");
    assert_eq!(refused.code, ErrorCode::Unsupported);
    assert_eq!(
        refused.message,
        "parameter 'instrument_type' is not automatable"
    );
    // The graph builds the enum lane with the sampler and plays without a warning.
    let audio = app.render();
    assert!(
        audio.stats.warnings.is_empty(),
        "{:?}",
        audio.stats.warnings
    );
    assert!(peak(&audio) > 0.01, "the preset plays: {}", peak(&audio));
}

#[test]
fn a_choice_saved_by_a_newer_version_opens_and_plays_as_auto() {
    // A project saved by a newer version of Sonara can store an instrument type or an
    // articulation this version does not offer. It opens unchanged and plays as `auto`, with a
    // warning, instead of failing (ADR 0010).
    let dir = tempfile::tempdir().expect("tempdir");
    let app = App::new(&library(dir.path()));
    let device = app.sampler_track();
    app.edit(json!({"op": "set_device_params", "device_id": device,
                    "values": {"instrument_type": "violin", "articulation": "staccato"}}));
    let bundle = app.dir.path().join("Newer.sonara");
    app.app
        .projects()
        .save(Some(&bundle), Some(false))
        .expect("saved");
    let file = bundle.join("project.json");
    let text = fs::read_to_string(&file).expect("the project file");
    assert!(
        text.contains("\"staccato\""),
        "the choice is stored as text"
    );
    fs::write(
        &file,
        text.replace("\"violin\"", "\"ophicleide\"")
            .replace("\"staccato\"", "\"future_articulation\""),
    )
    .expect("write the newer project");
    let auto = {
        app.edit(json!({"op": "set_device_params", "device_id": device,
                        "values": {"instrument_type": "auto", "articulation": "auto"}}));
        app.render()
    };
    assert!(auto.stats.warnings.is_empty(), "{:?}", auto.stats.warnings);

    app.app.projects().close(true).expect("closed");
    app.app
        .projects()
        .open_project(&bundle, false)
        .expect("opens");
    assert_eq!(
        app.stored(&device, "articulation"),
        ParamValue::Str("future_articulation".into()),
        "opening does not rewrite the project"
    );
    let newer = app.render();
    assert_eq!(
        newer.stats.warnings,
        [
            "instrument_type 'ophicleide' of 'Sonara Sampler' is unknown to this version of \
             Sonara; it plays as 'auto'",
            "articulation 'future_articulation' of 'Sonara Sampler' is unknown to this version \
             of Sonara; it plays as 'auto'"
        ]
    );
    assert_eq!(newer.data, auto.data, "it plays as auto");
    assert!(peak(&newer) > 0.01, "the preset plays");
}
