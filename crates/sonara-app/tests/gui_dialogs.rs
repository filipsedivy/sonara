//! The browser, the inspector's edits and the dialogs of the desktop application, against the
//! prototype's own dialogs.
//!
//! The oracle is `apps/sonara-desktop/tests/fixtures/dialogs-v1.json`, which the frozen
//! prototype wrote on the host (`.venv/bin/python -I scripts/golden/gui_dialog_oracles.py`,
//! GLD-01): the settings files `SettingsDialog` wrote for given choices, `NewProjectDialog`'s
//! fields and values, `ExportDialog`'s presets and outputs, the browser's Devices tree, and the
//! Verify report. `test_export_dialog.py` and the
//! About dialog's author check here.
//!
//! The fixed prototype defects are asserted as Rust behaves: G1, G2,
//! G3, G4 (the edit boundary keeps the other fade), G11, G12 and
//! G13.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sonara_app::about::{self, agent_connection, bundled_cli};
use sonara_app::browser::{self, BrowserItem};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::edit::{self, Edit};
use sonara_app::events::JobNotice;
use sonara_app::export_dialog::{
    self, BitDepth, ExportForm, ExportTail, NamedTail, NormalizeChoice, preset_names,
};
use sonara_app::jobs::JobStatus;
use sonara_app::media::AudioImport;
use sonara_app::projects::{NewProject, NewProjectForm, new_project_dialog};
use sonara_app::prompts;
use sonara_app::render::{Container, Dither, Normalize, StemOptions, Tail, stem_paths};
use sonara_app::settings::{Settings, load_settings};
use sonara_app::settings_dialog::{
    self, BACKENDS, BUFFERS, DeviceListing, SettingsDialogView, SettingsForm, THEMES,
};
use sonara_core::model::clip::{Clip, FadeCurve};
use tempfile::TempDir;

/// The prototype's oracle.
fn fixture() -> &'static Value {
    static FIXTURE: OnceLock<Value> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/sonara-desktop/tests/fixtures/dialogs-v1.json");
        let text = fs::read_to_string(&path).expect("the dialogs fixture");
        serde_json::from_str(&text).expect("JSON")
    })
}

/// An application whose settings live in `dir/settings.json`, saved there (as the desktop's).
fn app_in(dir: &Path) -> AppContext {
    AppContext::create(AppOptions {
        workspaces: Some(dir.join("ws")),
        settings_path: Some(dir.join("settings.json")),
        persist_settings: true,
        audio_backend: Some("null".to_owned()),
        ..AppOptions::default()
    })
    .expect("the application")
}

/// An application with settings in memory and a new project `name` from `template`.
fn project_app(dir: &Path, name: &str, template: &str) -> AppContext {
    let app = AppContext::create(AppOptions {
        workspaces: Some(dir.join("ws")),
        settings: Some(Settings::default()),
        persist_settings: false,
        audio_backend: Some("null".to_owned()),
        ..AppOptions::default()
    })
    .expect("the application");
    app.projects()
        .new_project(&NewProject {
            name: name.to_owned(),
            template: template.to_owned(),
            ..NewProject::default()
        })
        .expect("a project");
    app
}

/// Half a second of a quiet sine at 48 kHz, mono.
fn write_tone(path: &Path) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("a WAV writer");
    for n in 0..24_000_u32 {
        #[expect(clippy::cast_possible_truncation, reason = "a float test signal")]
        let sample = (0.3 * (f64::from(n) / 10.0).sin()) as f32;
        writer.write_sample(sample).expect("a sample");
    }
    writer.finalize().expect("the WAV");
}

fn text(value: &Value) -> &str {
    value.as_str().expect("a string")
}

// -- the Settings dialog ----------------------------------------------------

#[test]
fn the_settings_dialog_writes_the_prototypes_bytes() {
    let cases = fixture()["settings"]["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 5);
    for case in cases {
        let name = text(&case["name"]);
        let dir = tempfile::tempdir().expect("a temporary directory");
        let file = dir.path().join("settings.json");
        if let Some(before) = case["before"].as_str() {
            fs::write(&file, before).expect("the old file");
        }
        let app = app_in(dir.path());
        let form: SettingsForm =
            serde_json::from_value(case["form"].clone()).expect("the dialog's fields");
        if case["typed"]
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
        {
            // The dialog opens with the prototype's values.
            assert_eq!(SettingsForm::of(&load_settings(&file)), form, "{name}");
        }
        let saved = settings_dialog::apply_and_save(&app, &form).expect("saved");
        let after = fs::read_to_string(&file).expect("the new file");
        assert_eq!(
            after,
            text(&case["after"]),
            "{name}: the bytes of settings.json"
        );
        assert_eq!(saved.autosave_seconds, form.autosave_seconds.clamp(0, 3600));
        assert_eq!(app.settings().theme, form.theme, "{name}: in memory too");
        app.close();
    }
}

#[test]
fn the_settings_dialog_offers_the_prototypes_choices() {
    let choices = &fixture()["settings"]["choices"];
    let pairs = |items: &Value| -> Vec<(String, Value)> {
        items
            .as_array()
            .expect("items")
            .iter()
            .map(|item| (text(&item[0]).to_owned(), item[1].clone()))
            .collect()
    };
    let view = SettingsDialogView::new(&Settings::default(), DeviceListing::default());
    let shown = |choices: &[settings_dialog::Choice<String>]| -> Vec<(String, Value)> {
        choices
            .iter()
            .map(|c| (c.label.clone(), json!(c.value)))
            .collect()
    };
    assert_eq!(shown(&view.backends), pairs(&choices["backends"]));
    assert_eq!(shown(&view.themes), pairs(&choices["themes"]));
    let buffers: Vec<(String, Value)> = view
        .buffers
        .iter()
        .map(|c| (c.label.clone(), json!(c.value)))
        .collect();
    assert_eq!(buffers, pairs(&choices["buffers"]));
    assert_eq!(BACKENDS.len(), 4);
    assert_eq!(BUFFERS.len(), 6);
    assert_eq!(THEMES.len(), 2);
    assert_eq!(
        json!([view.lookahead_ms.0, view.lookahead_ms.1, 2]),
        choices["lookahead"]
    );
    assert_eq!(
        json!([view.autosave_seconds.0, view.autosave_seconds.1, "off"]),
        choices["autosave"]
    );
    assert_eq!(
        json!([view.backup_retention.0, view.backup_retention.1]),
        choices["backups"]
    );
    assert_eq!(
        json!([view.undo_limit.0, view.undo_limit.1]),
        choices["undo"]
    );
    // Without devices the combos hold the system default only.
    assert_eq!(view.outputs.len(), 1);
    assert_eq!(view.outputs[0].label, "System default");
    assert_eq!(view.outputs[0].value, None);
}

/// A stored value that is none of a combo's choices is shown and kept.
#[test]
fn a_stored_value_outside_the_choices_is_kept() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("settings.json");
    let mut stored = Settings::default();
    stored.audio.buffer_frames = 480;
    stored.audio.backend = "coreaudio".to_owned();
    stored.theme = "solarized".to_owned();
    stored.audio.output_device = Some("dev-out-7".to_owned());
    sonara_app::settings::save_settings(&stored, &file).expect("saved");
    let app = app_in(dir.path());
    let listing = DeviceListing::from_json(&json!({"devices": [
        {"id": "dev-out-1", "name": "Speakers", "kind": "playback", "default": true},
        {"id": "dev-in-1", "name": "Mic", "kind": "capture", "default": false},
    ]}));
    let view = SettingsDialogView::new(&app.settings(), listing);
    assert!(
        view.buffers
            .iter()
            .any(|c| c.value == 480 && c.label == "480 frames")
    );
    assert_eq!(
        view.buffers.iter().map(|c| c.value).collect::<Vec<_>>(),
        vec![64, 128, 256, 480, 512, 1024, 2048]
    );
    assert_eq!(
        view.backends.last().map(|c| c.value.as_str()),
        Some("coreaudio")
    );
    assert_eq!(
        view.themes.last().map(|c| c.value.as_str()),
        Some("solarized")
    );
    let outputs: Vec<(&str, Option<&str>)> = view
        .outputs
        .iter()
        .map(|c| (c.label.as_str(), c.value.as_deref()))
        .collect();
    assert_eq!(
        outputs,
        vec![
            ("System default", None),
            ("Speakers (default)", Some("dev-out-1")),
            ("dev-out-7 (not connected)", Some("dev-out-7")),
        ]
    );
    assert_eq!(view.inputs[1].label, "Mic");
    // OK as the dialog opened: every stored value stays.
    settings_dialog::apply_and_save(&app, &view.form).expect("saved");
    let back = load_settings(&file);
    assert_eq!(back.audio.buffer_frames, 480);
    assert_eq!(back.audio.backend, "coreaudio");
    assert_eq!(back.theme, "solarized");
    assert_eq!(back.audio.output_device.as_deref(), Some("dev-out-7"));
    // A value the dialog never offered is refused, and nothing changes.
    let before = fs::read(&file).expect("the file");
    let mut odd = view.form.clone();
    odd.buffer_frames = 333;
    odd.theme = "light".to_owned();
    let error = settings_dialog::apply_and_save(&app, &odd).expect_err("refused");
    assert_eq!(
        sonara_app::events::ErrorNotice::from(&error).code,
        "INVALID_ARGUMENT"
    );
    assert_eq!(fs::read(&file).expect("the file"), before);
    assert_eq!(app.settings().theme, "solarized");
    app.close();
}

// -- New Project --------------------------------------------------------------------------

#[test]
fn the_new_project_dialog_matches_the_prototype() {
    let oracle = &fixture()["new_project"];
    let fields = &oracle["fields"];
    let view = new_project_dialog();
    assert_eq!(view.form.name, text(&fields["name"]));
    assert_eq!(view.form.template, text(&fields["template"]));
    assert_eq!(json!(view.form.time_signature), fields["signature"]);
    assert_eq!(json!(view.form.sample_rate), fields["rate"]);
    assert_eq!(
        json!([view.form.bpm, view.bpm.0, view.bpm.1, 2]),
        fields["bpm"]
    );
    let templates: Vec<Value> = view
        .templates
        .iter()
        .map(|t| json!([t.id, t.name, t.description]))
        .collect();
    assert_eq!(Value::Array(templates), fields["templates"]);
    assert_eq!(json!(view.signatures), fields["signatures"]);
    let rates: Vec<Value> = view.rates.iter().map(|(l, r)| json!([l, r])).collect();
    assert_eq!(Value::Array(rates), fields["rates"]);
    for case in oracle["values"].as_array().expect("cases") {
        let form = NewProjectForm {
            name: text(&case["name"]).to_owned(),
            template: "empty".to_owned(),
            bpm: case["bpm"].as_f64().expect("a tempo"),
            time_signature: text(&case["signature"]).to_owned(),
            sample_rate: 48_000,
        };
        let values = &case["values"];
        let made = form.to_new_project().expect("valid");
        assert_eq!(made.name, text(&values["name"]));
        assert_eq!(json!(made.bpm), values["bpm"], "{}", case["bpm"]);
        assert_eq!(
            json!([made.time_signature.0, made.time_signature.1]),
            values["time_signature"]
        );
        assert_eq!(json!(made.sample_rate), values["sample_rate"]);
        assert_eq!(made.template, text(&values["template"]));
    }
}

/// Every bad field is reported, before anything else happens.
#[test]
fn a_bad_new_project_reports_its_field() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let app = project_app(dir.path(), "Open", "empty");
    let form = |signature: &str| NewProjectForm {
        time_signature: signature.to_owned(),
        ..NewProjectForm::default()
    };
    for (signature, message) in [
        ("x/4", "invalid time signature 'x/4'"),
        ("4", "invalid time signature '4'"),
        ("4/4/4", "invalid time signature '4/4/4'"),
        ("", "invalid time signature ''"),
    ] {
        let error = form(signature).to_new_project().expect_err(signature);
        assert_eq!(
            (error.field, error.code, error.message.as_str()),
            ("time_signature", "INVALID_ARGUMENT", message)
        );
    }
    let error = form("3/5")
        .to_new_project()
        .expect_err("a denominator of 5");
    assert_eq!(
        (error.field, error.code),
        ("time_signature", "INVALID_ARGUMENT")
    );
    assert!(error.message.contains("denominator"), "{}", error.message);
    let spaced = form(" 7 / 8 ")
        .to_new_project()
        .expect("Python's int takes spaces");
    assert_eq!(spaced.time_signature, (7, 8));
    let error = NewProjectForm {
        template: "jazz".to_owned(),
        ..NewProjectForm::default()
    }
    .to_new_project()
    .expect_err("an unknown template");
    assert_eq!((error.field, error.code), ("template", "NOT_FOUND"));
    let error = NewProjectForm {
        sample_rate: 22_050,
        ..NewProjectForm::default()
    }
    .to_new_project()
    .expect_err("a rate the dialog does not offer");
    assert_eq!(
        (error.field, error.code),
        ("sample_rate", "INVALID_ARGUMENT")
    );
    let error = NewProjectForm {
        bpm: f64::NAN,
        ..NewProjectForm::default()
    }
    .to_new_project()
    .expect_err("no tempo");
    assert_eq!(error.field, "bpm");
    // The tempo is clamped as the spin box clamps it.
    let fast = NewProjectForm {
        bpm: 2000.0,
        ..NewProjectForm::default()
    }
    .to_new_project()
    .expect("clamped");
    assert!((fast.bpm - 999.0).abs() < 1e-12);
    // A valid form is built in memory without touching the open project.
    let options = NewProjectForm {
        name: "Next".to_owned(),
        template: "band".to_owned(),
        ..NewProjectForm::default()
    }
    .to_new_project()
    .expect("valid");
    app.projects()
        .check_new_project(&options)
        .expect("buildable");
    assert_eq!(
        app.session().project().map(|p| p.name.clone()).as_deref(),
        Some("Open")
    );
    app.close();
}

// -- Export Audio ------------------------------------------------------------------

/// The electronic project of the export oracle, its tracks by name, and the temporary folder
/// that stands for the oracle's `<tmp>`.
struct ExportProject {
    app: AppContext,
    dir: TempDir,
}

impl ExportProject {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let app = project_app(dir.path(), "Dialog", "electronic");
        Self { app, dir }
    }

    fn project(&self) -> sonara_core::model::project::Project {
        self.app.session().project().expect("the project").clone()
    }

    /// The oracle's text with `<tmp>` and `<home>` replaced by this run's.
    fn local(&self, value: &str) -> String {
        let home = sonara_app::paths::expand_user(Path::new("~"));
        value
            .replace("<tmp>", &self.dir.path().display().to_string())
            .replace("<home>", home.display().to_string().trim_end_matches('/'))
    }

    fn track_id(&self, name: &str) -> String {
        self.project()
            .tracks
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.id.clone())
            .expect(name)
    }

    fn track_name(&self, id: &str) -> String {
        self.project()
            .tracks
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.name.clone())
            .expect(id)
    }

    /// The oracle's form, its paths and track names made this run's.
    fn form(&self, oracle: &Value) -> ExportForm {
        let mut oracle = oracle.clone();
        oracle["path"] = json!(self.local(text(&oracle["path"])));
        let ids: Vec<Value> = oracle["stem_tracks"]
            .as_array()
            .expect("tracks")
            .iter()
            .map(|name| json!(self.track_id(text(name))))
            .collect();
        oracle["stem_tracks"] = Value::Array(ids);
        serde_json::from_value(oracle).expect("the dialog's fields")
    }
}

#[test]
fn the_export_dialog_opens_as_the_prototypes() {
    let export = ExportProject::new();
    let oracle = &fixture()["export"];
    let default_dir = PathBuf::from(export.local(text(&oracle["default_dir"])));
    let fresh = ExportForm::new(&export.project(), &default_dir);
    assert_eq!(fresh, export.form(&oracle["fresh"]));
    let view = export_dialog::dialog_view(&export.project(), &default_dir, &Container::ALL);
    let labels: Vec<Value> = view
        .formats
        .iter()
        .map(|c| json!([c.value, c.label]))
        .collect();
    assert_eq!(Value::Array(labels), oracle["format_labels"]);
    let presets: Vec<&str> = oracle["presets"]
        .as_object()
        .expect("presets")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(preset_names()[1..], presets[..]);
    assert_eq!(preset_names()[0], "Custom");
    let stems: Vec<String> = view.stem_tracks.iter().map(|c| c.label.clone()).collect();
    assert_eq!(
        stems,
        ["Drums", "Bass", "Chords", "Lead", "Reverb", "Delay"]
    );
    // MP3 and FLAC need their libraries: only the formats this build writes are offered.
    let some = [Container::Wav, Container::Ogg];
    let view = export_dialog::dialog_view(&export.project(), &default_dir, &some);
    assert_eq!(
        view.formats.iter().map(|c| c.value).collect::<Vec<_>>(),
        ["wav", "ogg"]
    );
    export.app.close();
}

#[test]
fn the_export_dialog_builds_the_prototypes_outputs() {
    let export = ExportProject::new();
    let project = export.project();
    let cases = fixture()["export"]["cases"].as_array().expect("cases");
    assert!(cases.len() >= 10);
    for case in cases {
        let name = text(&case["name"]);
        let form = export.form(&case["form"]);
        let outputs = form.outputs(&Container::ALL).expect(name);
        let got: Vec<Value> = outputs
            .iter()
            .map(|o| {
                json!({
                    "path": o.path.display().to_string(),
                    "container": o.container.value(),
                    "bit_depth": o.bit_depth,
                    "floating_point": o.floating_point,
                    "sample_rate": o.sample_rate,
                    "channels": o.channels,
                    "bitrate_kbps": o.bitrate_kbps,
                    "variable_bitrate": o.variable_bitrate,
                    "dither": match o.dither { Dither::Tpdf => "tpdf", Dither::None => "none" },
                })
            })
            .collect();
        let want: Vec<Value> = case["outputs"]
            .as_array()
            .expect("outputs")
            .iter()
            .map(|o| {
                let mut o = o.clone();
                o["path"] = json!(export.local(text(&o["path"])));
                o
            })
            .collect();
        assert_eq!(got, want, "{name}: outputs");
        let request = form.request(&project).expect(name);
        let tail = match request.tail {
            Tail::Auto => json!("auto"),
            Tail::None => json!("none"),
            Tail::Seconds(s) => json!(s),
        };
        assert_eq!(
            json!({"start_beat": request.start_beat, "end_beat": request.end_beat, "tail": tail}),
            case["request"],
            "{name}: request"
        );
        let spec = form.normalize_spec();
        let mode = match spec.mode {
            Normalize::None => "none",
            Normalize::Peak => "peak",
            Normalize::TruePeak => "true_peak",
            Normalize::Loudness => "loudness",
        };
        assert_eq!(
            json!({"mode": mode, "target_db": spec.target_db, "ceiling_db": spec.true_peak_ceiling_db}),
            case["normalize"],
            "{name}: normalization"
        );
        let stems = form.stem_track_ids().map(|ids| {
            ids.iter()
                .map(|id| export.track_name(id))
                .collect::<Vec<_>>()
        });
        assert_eq!(json!(stems), case["stem_tracks"], "{name}: stems");
        assert_eq!(
            json!({"through_master": form.through_master}),
            case["settings"]
        );
    }
    // No format, and a format this build does not write.
    let mut none = export.form(&cases[0]["form"]);
    none.formats.clear();
    let error = none.outputs(&Container::ALL).expect_err("no format");
    assert_eq!(
        sonara_app::events::ErrorNotice::from(&error).message,
        "choose at least one format"
    );
    let mp3 = ExportForm {
        formats: vec!["mp3".to_owned()],
        ..export.form(&cases[0]["form"])
    };
    assert!(mp3.outputs(&[Container::Wav]).is_err());
    export.app.close();
}

/// G1 and G13: every preset sets the prototype's fields, except the two fields the
/// oracle names as the prototype's defects (`export.prototype_defects`): the podcast preset's
/// 192 kbps and the archive preset's 32-bit float.
#[test]
fn the_presets_set_the_prototypes_fields_and_podcast_192() {
    let export = ExportProject::new();
    let default_dir = PathBuf::from(export.local(text(&fixture()["export"]["default_dir"])));
    let fresh = ExportForm::new(&export.project(), &default_dir);
    let defects = fixture()["export"]["prototype_defects"]
        .as_object()
        .expect("the defects");
    assert_eq!(defects.len(), 2);
    for (name, oracle) in fixture()["export"]["presets"].as_object().expect("presets") {
        let mut want = export.form(oracle);
        if let Some(defect) = defects.get(name) {
            let field = text(&defect["field"]);
            // The prototype's value, then Rust's.
            assert_ne!(json!(want)[field], defect["rust"], "{name}");
            let mut fixed = json!(want);
            fixed[field] = defect["rust"].clone();
            want = serde_json::from_value(fixed).expect("the fixed form");
        }
        assert_eq!(fresh.with_preset(name, &Container::ALL), want, "{name}");
    }
    let podcast = fresh.with_preset("Podcast/Web (MP3 192 + Opus)", &Container::ALL);
    let outputs = podcast.outputs(&Container::ALL).expect("outputs");
    assert_eq!(outputs[0].container, Container::Mp3);
    assert_eq!(outputs[0].bitrate_kbps, Some(192));
    assert_eq!(outputs[1].bitrate_kbps, Some(192));
    // Custom changes nothing; a preset checks only the formats this build writes.
    assert_eq!(
        fresh.with_preset("Custom", &Container::ALL).formats,
        fresh.formats
    );
    let streaming = fresh.with_preset("Streaming (-14 LUFS, WAV + MP3 320)", &[Container::Wav]);
    assert_eq!(streaming.formats, ["wav"]);
    assert_eq!(streaming.normalize, NormalizeChoice::Loudness);
    export.app.close();
}

/// The archive preset's WAV is 32-bit float, as its name says, and its FLAC 24-bit
/// integer; the prototype exported both at 24-bit integer.
#[test]
fn the_archive_preset_writes_a_float_wav_and_a_24_bit_flac() {
    let export = ExportProject::new();
    let fresh = ExportForm::new(&export.project(), export.dir.path());
    let name = "Archive (FLAC 24-bit + 32-bit float WAV)";
    let prototype = &fixture()["export"]["presets"][name];
    assert_eq!(
        prototype["bit_depth"], "24",
        "the prototype's combo stayed at 24-bit"
    );
    let recorded = fixture()["export"]["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["name"] == format!("preset {name}"))
        .expect("the preset's case");
    assert!(
        recorded["outputs"]
            .as_array()
            .expect("outputs")
            .iter()
            .all(|o| o["bit_depth"] == 24 && o["floating_point"] == false),
        "the prototype wrote both at 24-bit integer"
    );
    let archive = fresh.with_preset(name, &Container::ALL);
    assert_eq!(archive.bit_depth, BitDepth::Float32);
    assert_eq!(archive.formats, ["wav", "flac"]);
    let outputs: Vec<(Container, u32, bool)> = archive
        .outputs(&Container::ALL)
        .expect("outputs")
        .iter()
        .map(|o| (o.container, o.bit_depth, o.floating_point))
        .collect();
    assert_eq!(
        outputs,
        vec![(Container::Wav, 32, true), (Container::Flac, 24, false)]
    );
    // Without libFLAC the preset still makes its WAV float.
    let wav_only = fresh.with_preset(name, &[Container::Wav]);
    assert_eq!(wav_only.formats, ["wav"]);
    assert_eq!(wav_only.bit_depth, BitDepth::Float32);
    // The other presets keep their integer depths.
    for (preset, depth) in [
        ("Master (WAV 24-bit)", BitDepth::Int24),
        ("CD (WAV 16-bit 44.1 kHz)", BitDepth::Int16),
        ("Streaming (-14 LUFS, WAV + MP3 320)", BitDepth::Int24),
        ("Podcast/Web (MP3 192 + Opus)", BitDepth::Int24),
    ] {
        assert_eq!(
            fresh.with_preset(preset, &Container::ALL).bit_depth,
            depth,
            "{preset}"
        );
    }
    export.app.close();
}

/// Waits until every job has finished.
fn wait_for_jobs(app: &AppContext) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while app
        .jobs()
        .list(50)
        .iter()
        .any(|job| !job.status().is_finished())
    {
        assert!(Instant::now() < deadline, "the jobs did not finish");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The prompt names the stem files an export would replace, and stems replace files
/// only after the user agreed; the stem files are the ones the stems export writes.
#[test]
fn the_overwrite_prompt_names_existing_stems() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let app = project_app(dir.path(), "Stems", "empty");
    let tone = dir.path().join("tone.wav");
    write_tone(&tone);
    for _ in 0..2 {
        app.media()
            .import_audio(
                &tone,
                &AudioImport {
                    create_clip: true,
                    ..AudioImport::default()
                },
            )
            .expect("imported");
    }
    let project = app.session().project().expect("project").clone();
    let mut form = ExportForm::new(&project, &dir.path().join("out"));
    form.stems = true;
    form.tail = ExportTail::Named(NamedTail::None);
    let plan = form.plan(&project, &Container::ALL).expect("a plan");
    let stems = plan.stems.clone().expect("stems");
    assert_eq!(stems.directory, dir.path().join("out").join("Stems stems"));
    assert_eq!(
        stems.files,
        stem_paths(
            &project,
            &stems.directory,
            Some(&stems.track_ids),
            Container::Wav
        )
        .expect("names")
    );
    assert_eq!(plan.existing_files(), Vec::<PathBuf>::new());
    plan.clone()
        .submit(app.render(), false)
        .expect("the export jobs");
    wait_for_jobs(&app);
    let written: Vec<PathBuf> = {
        let mut files: Vec<PathBuf> = fs::read_dir(&stems.directory)
            .expect("the stems")
            .map(|e| e.expect("an entry").path())
            .collect();
        files.sort();
        files
    };
    let mut planned = stems.files.clone();
    planned.sort();
    assert_eq!(written, planned, "the files the stems export wrote");
    // A second export would replace the mix and both stems: the prompt names all of them.
    let existing = plan.existing_files();
    assert_eq!(existing.len(), 3);
    let prompt = prompts::overwrite_prompt(&existing);
    assert_eq!(prompt.title, "Overwrite?");
    for file in &stems.files {
        assert!(
            prompt.text.contains(&file.display().to_string()),
            "{}",
            prompt.text
        );
    }
    // Without the user's agreement the stems job refuses to replace them.
    for file in &stems.files {
        fs::write(file, b"precious").expect("written");
    }
    app.render()
        .export_stems_job(
            stems.directory.clone(),
            Some(stems.track_ids.clone()),
            stems.template.clone(),
            plan.request.clone(),
            StemOptions {
                through_master: false,
                overwrite: false,
            },
        )
        .expect("queued");
    wait_for_jobs(&app);
    for file in &stems.files {
        assert_eq!(fs::read(file).expect("kept"), b"precious");
    }
    let failed = app
        .jobs()
        .list(50)
        .into_iter()
        .find(|job| job.kind() == "export_stems" && job.status() == JobStatus::Failed)
        .expect("the refused stems job");
    assert_eq!(
        failed.snapshot().error.as_ref().map(|e| e.code.as_str()),
        Some("CONFLICT")
    );
    app.close();
}

#[test]
fn a_finished_export_reports_its_files_and_levels() {
    let job = |kind: &str, status: &'static str, result: Value| JobNotice {
        id: "op_1".to_owned(),
        kind: kind.to_owned(),
        label: "Export audio".to_owned(),
        status,
        finished: true,
        progress: 1.0,
        message: String::new(),
        result: Some(result),
        error: None,
    };
    let done = export_dialog::export_message(&job(
        "export_audio",
        "succeeded",
        json!({"outputs": [{}, {}], "final_integrated_lufs": -14.04, "final_true_peak_dbtp": -1.25}),
    ))
    .expect("a message");
    assert_eq!(done.text, "Exported 2 file(s) · -14.0 LUFS · -1.2 dBTP");
    assert_eq!(done.duration_ms, 8000);
    assert!(!done.error);
    let stems = export_dialog::export_message(&job(
        "export_stems",
        "succeeded",
        json!({"stems": [{}, {}, {}], "directory": "/x"}),
    ))
    .expect("a message");
    assert_eq!(stems.text, "Exported 3 file(s)");
    let silent = export_dialog::export_message(&job(
        "export_audio",
        "succeeded",
        json!({"outputs": [{}], "final_integrated_lufs": null, "final_true_peak_dbtp": -3.0}),
    ))
    .expect("a message");
    assert_eq!(silent.text, "Exported 1 file(s)");
    assert!(export_dialog::export_message(&job("export_audio", "failed", json!({}))).is_none());
    assert!(export_dialog::export_message(&job("import_audio", "succeeded", json!({}))).is_none());
}

#[test]
fn the_export_forms_choices_are_checked() {
    let export = ExportProject::new();
    let project = export.project();
    let mut form = ExportForm::new(&project, export.dir.path());
    form.tail = ExportTail::Seconds(3.0);
    assert!(form.request(&project).is_err(), "3 s is not offered");
    form.tail = ExportTail::Seconds(5.0);
    assert_eq!(
        form.request(&project).expect("5 s").tail,
        Tail::Seconds(5.0)
    );
    form.channels = 6;
    assert!(form.outputs(&Container::ALL).is_err());
    form.channels = 1;
    form.sample_rate = Some(22_050);
    assert!(form.outputs(&Container::ALL).is_err());
    form.sample_rate = None;
    form.bitrate_kbps = 100;
    assert!(form.outputs(&Container::ALL).is_err());
    form.bitrate_kbps = 128;
    form.path = "  ".to_owned();
    assert!(form.outputs(&Container::ALL).is_err());
    form.path = export.dir.path().join("x").display().to_string();
    form.bit_depth = BitDepth::Float32;
    form.target = -75.0;
    form.ceiling = 3.0;
    let spec = form.normalize_spec();
    assert!(
        (spec.target_db + 60.0).abs() < 1e-12,
        "clamped as the spin box"
    );
    assert_eq!(spec.true_peak_ceiling_db, Some(0.0));
    let parsed: ExportForm = serde_json::from_value(json!({
        "path": "/x", "preset": "Custom", "formats": ["wav"], "bit_depth": "32f",
        "sample_rate": null, "channels": 2, "bitrate_kbps": 320, "dither": true,
        "range": "loop", "tail": 2.0, "normalize": "true_peak", "target": -2.0,
        "ceiling": -0.5, "stems": false, "through_master": false, "stem_tracks": []
    }))
    .expect("the page's form");
    assert_eq!(parsed.bit_depth, BitDepth::Float32);
    assert_eq!(parsed.tail, ExportTail::Seconds(2.0));
    assert!(serde_json::from_value::<ExportForm>(json!({"path": "/x"})).is_err());
    export.app.close();
}

// -- the browser --------------------------------------------------------------------

#[test]
fn the_browser_tree_is_the_prototypes() {
    fn item(node: &BrowserItem) -> Value {
        json!({
            "text": node.text,
            "plugin": node.plugin_id,
            "preset": node.preset,
            "tooltip": node.tooltip,
            "children": node.children.iter().map(item).collect::<Vec<_>>(),
        })
    }
    let registry = sonara_core::plugins::builtin_registry();
    let tree = browser::device_tree(&registry);
    let got: Vec<Value> = tree.iter().map(item).collect();
    assert_eq!(Value::Array(got), fixture()["browser"]);
    // Each plugin and preset says which kind it is, for the double-click's action.
    let instruments = &tree[0];
    assert!(
        instruments
            .children
            .iter()
            .all(|p| p.kind == Some("instrument"))
    );
    assert!(
        tree[1]
            .children
            .iter()
            .all(|category| category.kind.is_none())
    );
    assert!(
        tree[1]
            .children
            .iter()
            .flat_map(|category| &category.children)
            .all(
                |p| p.kind == Some("effect") && p.children.iter().all(|c| c.kind == Some("effect"))
            )
    );
}

/// The Files tab lists folders and the audio files the import takes, `.rf64` and
/// `.aac` included, under its root only.
#[test]
fn the_browser_lists_folders_and_the_import_formats() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let root = dir.path().join("Music");
    fs::create_dir_all(root.join("Loops/Drums")).expect("folders");
    for name in [
        "take 10.wav",
        "Take 2.WAV",
        "long.rf64",
        "song.aac",
        "notes.txt",
        ".hidden.wav",
        "voice.m4a",
        "cover.jpg",
    ] {
        fs::write(root.join(name), b"x").expect("a file");
    }
    fs::create_dir(root.join(".git")).expect("a hidden folder");
    let listed = browser::list_directory(&root, &root).expect("listed");
    let names: Vec<(&str, bool)> = listed.iter().map(|e| (e.name.as_str(), e.folder)).collect();
    assert_eq!(
        names,
        vec![
            ("long.rf64", false),
            ("Loops", true),
            ("song.aac", false),
            ("Take 2.WAV", false),
            ("take 10.wav", false),
            ("voice.m4a", false),
        ]
    );
    let loops = listed.iter().find(|e| e.name == "Loops").expect("a folder");
    let inner = browser::list_directory(&root, Path::new(&loops.path)).expect("listed");
    assert_eq!(inner.len(), 1);
    assert!(inner[0].folder);
    let code =
        |error: &sonara_app::error::AppError| sonara_app::events::ErrorNotice::from(error).code;
    let outside = browser::list_directory(&root, dir.path()).expect_err("outside the root");
    assert_eq!(code(&outside), "PERMISSION_DENIED");
    let up = browser::list_directory(&root, &root.join("Loops/../..")).expect_err("climbing out");
    assert_eq!(code(&up), "PERMISSION_DENIED");
    let file = browser::list_directory(&root, &root.join("long.rf64")).expect_err("a file");
    assert_eq!(code(&file), "NOT_FOUND");
    // The root is ~/Music when it exists.
    let shown = browser::browser_root();
    assert!(shown.is_dir() || shown == Path::new("~"));
}

// -- the inspector's edits ---------------------------------------------------------------------

/// The inspector sends one fade per edit, and the edit keeps the other fade's curve.
#[test]
fn a_fade_in_edit_keeps_the_fade_out_curve() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let app = project_app(dir.path(), "Fades", "empty");
    let tone = dir.path().join("tone.wav");
    write_tone(&tone);
    let imported = app
        .media()
        .import_audio(
            &tone,
            &AudioImport {
                create_clip: true,
                ..AudioImport::default()
            },
        )
        .expect("imported");
    let clip_id = text(&imported["clip_id"]).to_owned();
    let apply = |value: Value| {
        edit::apply(&app, Edit::from_json(value).expect("an edit")).expect("applied");
    };
    apply(json!({"op": "set_clip_properties", "clip_id": clip_id,
                 "fade_in": {"seconds": 0.1, "curve": "linear"},
                 "fade_out": {"seconds": 0.2, "curve": "s_curve"}}));
    apply(json!({"op": "set_clip_properties", "clip_id": clip_id,
                 "fade_in": {"seconds": 0.25, "curve": "linear"}}));
    let project = app.session().project().expect("project").clone();
    let (_, clip) = project.clip(&clip_id).expect("the clip");
    let Clip::Audio(clip) = clip else {
        panic!("an audio clip")
    };
    assert!((clip.fade_in.seconds - 0.25).abs() < 1e-12);
    assert_eq!(clip.fade_out.curve, FadeCurve::SCurve);
    assert!((clip.fade_out.seconds - 0.2).abs() < 1e-12);
    app.close();
}

// -- the prompts and reports -----------------------------------------------------------------

#[test]
fn the_verify_report_is_the_prototypes() {
    let oracle = &fixture()["verify"];
    let dir = tempfile::tempdir().expect("a temporary directory");
    let app = project_app(dir.path(), "Verified", "empty");
    let shown = |app: &AppContext| {
        let report = prompts::verify_report(&app.projects().verify(true).expect("verified"));
        format!("{}\n{}", report.title, report.text)
    };
    assert_eq!(shown(&app), text(&oracle["consistent"]));
    let tone = dir.path().join("verified.wav");
    write_tone(&tone);
    let imported = app
        .media()
        .import_audio(&tone, &AudioImport::default())
        .expect("imported");
    let project = app.session().project().expect("project").clone();
    let asset = project
        .assets
        .iter()
        .find(|a| a.id == text(&imported["asset_id"]))
        .expect("the asset")
        .clone();
    let workspace = app
        .session()
        .lock()
        .location()
        .expect("a location")
        .workspace
        .clone();
    fs::remove_file(workspace.join(&asset.path)).expect("the media removed");
    assert_eq!(shown(&app), text(&oracle["missing"]));
    app.close();
}

#[test]
fn the_prompts_word_as_the_prototype() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let app = project_app(dir.path(), "Prompts", "electronic");
    let project = app.session().project().expect("project").clone();
    let drums = project
        .tracks
        .iter()
        .find(|t| t.name == "Drums")
        .expect("drums");
    assert!(
        prompts::delete_track_prompt(&project, &drums.id).is_none(),
        "no clips"
    );
    assert!(prompts::delete_track_prompt(&project, "master").is_none());
    assert!(!prompts::can_delete_track(&project, "master"));
    assert!(prompts::can_delete_track(&project, &drums.id));
    edit::apply(
        &app,
        Edit::from_json(
            json!({"op": "create_midi_clip", "track_id": drums.id, "start": 0.0,
                               "length": 4.0, "name": "Beat"}),
        )
        .expect("an edit"),
    )
    .expect("a clip");
    let project = app.session().project().expect("project").clone();
    let prompt = prompts::delete_track_prompt(&project, &drums.id).expect("a prompt");
    assert_eq!(prompt.title, "Delete Track");
    assert_eq!(
        prompt.text,
        "Delete “Drums” and its 1 clip(s)? (Undo is available.)"
    );
    let none = prompts::unused_media_prompt(&[]).expect_err("nothing to remove");
    assert_eq!(
        (none.title.as_str(), none.text.as_str()),
        ("Remove Unused Media", "No unused media files.")
    );
    let files: Vec<PathBuf> = (0..25)
        .map(|i| PathBuf::from(format!("/w/media/audio/f{i}.wav")))
        .collect();
    let ask = prompts::unused_media_prompt(&files).expect("a confirmation");
    assert!(
        ask.text
            .starts_with("Delete 25 unused file(s) from the project folder?\n\nf0.wav\n")
    );
    assert!(ask.text.contains("f19.wav\n\nThis cannot be undone."));
    assert!(!ask.text.contains("f20.wav"));
    assert_eq!(prompts::removed_media(3).text, "Removed 3 file(s)");
    assert_eq!(prompts::embedded_media(2).text, "Embedded 2 linked file(s)");
    let packaged = prompts::packaged_copy(Path::new("/p/Song.sonara"));
    assert_eq!(
        (packaged.text.as_str(), packaged.duration_ms),
        ("Packaged copy written to /p/Song.sonara", 5000)
    );
    assert_eq!(
        prompts::imported_midi(&json!({"tracks": [{}, {}]})).text,
        "Imported 2 MIDI track(s)"
    );
    assert_eq!(
        prompts::exported_midi(&json!({"notes": 12, "path": "/p/Song.mid"})).text,
        "Exported 12 notes to /p/Song.mid"
    );
    app.close();
}

// -- About and Agent Connection --------------------------------------------------------

/// `about_dialog_shows_author_and_contact`: the author, the address, the version and the system
/// rows that "Copy System Info" copies.
#[test]
fn about_shows_the_author_the_version_and_the_system() {
    let shown = about::about(
        vec![
            ("Audio".to_owned(), "test".to_owned()),
            ("Agent IPC".to_owned(), "off".to_owned()),
        ],
        None,
    );
    assert_eq!(shown.author, "Filip Sedivy");
    assert_eq!(shown.email, "mail@filipsedivy.cz");
    assert_eq!(
        shown.version,
        format!("Version {} (alpha)", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(shown.copyright, "© 2026 Filip Sedivy. All rights reserved.");
    let info = about::system_text(&shown.system);
    assert!(
        info.starts_with(&format!("Sonara: {} (alpha)\n", env!("CARGO_PKG_VERSION"))),
        "{info}"
    );
    assert!(info.contains("Audio engine: miniaudio "), "{info}");
    assert!(info.ends_with("Audio: test\nAgent IPC: off"), "{info}");
    assert!(
        shown
            .credits
            .iter()
            .any(|c| c.name == "LAME" && c.license.starts_with("LGPL"))
    );
    assert!(shown.licenses.is_none());
    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join(about::LICENSES_FILE);
    fs::create_dir_all(file.parent().expect("the licences folder")).expect("created");
    fs::write(&file, "Sonara 0.1.0: third-party licences").expect("written");
    assert_eq!(
        about::about(Vec::new(), Some(&file)).licenses.as_deref(),
        Some("Sonara 0.1.0: third-party licences")
    );
    let connection = agent_connection(
        Some("/tmp/sonara-501/gui.sock"),
        Some("/tmp/sonara-501/gui.json"),
        &bundled_cli(Path::new(
            "/Applications/Sonara.app/Contents/MacOS/sonara-desktop",
        )),
    );
    assert_eq!(
        connection.command,
        "claude mcp add sonara -- /Applications/Sonara.app/Contents/MacOS/sonara mcp"
    );
}

/// `test_brand_svgs_ship_with_the_package_and_are_valid`: the About dialog's brand assets are the
/// brand masters in `resources/brand/svg`, every
/// element closed, the wordmark outlined (no `<text>`, no font dependency).
#[test]
fn the_brand_svgs_ship_with_the_about_dialog() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let brand = root.join("apps/sonara-desktop/src/lib/dialogs/brand");
    let masters = root.join("resources/brand/svg");
    for name in [
        "icon.svg",
        "icon-small.svg",
        "mark.svg",
        "logo-dark.svg",
        "logo-light.svg",
    ] {
        let svg = fs::read_to_string(brand.join(name)).expect(name);
        let svg = svg.trim();
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{name}");
        assert!(svg.contains("viewBox=\"0 0 "), "{name}");
        assert!(!svg.contains("<text"), "{name}: the wordmark is outlined");
        let opened =
            svg.matches('<').count() - svg.matches("</").count() - svg.matches("<!").count();
        let self_closed = svg.matches("/>").count();
        assert_eq!(
            opened - self_closed,
            svg.matches("</").count(),
            "{name}: every element is closed"
        );
        // The dialog's copies equal the masters.
        let master = fs::read(masters.join(name)).expect(name);
        assert_eq!(fs::read(brand.join(name)).expect(name), master, "{name}");
    }
}
