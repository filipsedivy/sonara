//! The golden `transport` area's control transcripts replayed through the agent tools and the transport service on miniaudio's null device,
//! as the recorder ran them (`scripts/golden/area_transport.py`).
//!
//! Each scenario loads its recorded project into a fresh application on `native-null`, with the
//! ID counter where the recorder's setup left it, and the recording host's MIDI (`"core"`, no
//! ports). Every tool and service call must give the recorded answer, except the keys it marks
//! `<VOLATILE>` (the device clock's positions, the loads and the queue). The render steps let
//! the null device play for as long as the recorded blocks last, or until a play range stops by
//! itself; the played blocks themselves are compared by the player's golden test
//! (`sonara-engine`, `tests/player/golden.rs`), which drives the render thread by hand.
//!
//! **D4.** The recording's take summary is compared too: the asset and the clip are the
//! recorded ones, except the clip's `source_offset`, which the corrected capture start makes one
//! device period (256 frames) longer.
#![cfg(unix)]

use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde_json::{Value, json};
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::contract::versions::recorded_view;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget};
use sonara_app::clock::LocalZone;
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_app::midi::{MidiDevices, MidiError, OpenInput};
use sonara_app::settings::Settings;
use sonara_core::ids::SequentialIds;
use sonara_core::session::ProjectLocation;
use sonara_project::serializer::{loads, project_from_value, project_to_value};
use sonara_testkit::Golden;
use tempfile::TempDir;

const ROOT: &str = "<ROOT>";
const VOLATILE: &str = "<VOLATILE>";
/// 2026-01-01T12:00:00Z, the recorder's frozen clock.
const FROZEN_UNIX_SECONDS: u64 = 1_767_268_800;

/// The recording host's MIDI system: CoreMIDI without ports (the transcripts' `"core"`).
struct RecordingHostMidi;

impl MidiDevices for RecordingHostMidi {
    fn api_name(&self) -> &'static str {
        "core"
    }

    fn input_ports(&self) -> Result<Vec<String>, MidiError> {
        Ok(Vec::new())
    }

    fn output_ports(&self) -> Result<Vec<String>, MidiError> {
        Ok(Vec::new())
    }

    fn open_input(&self, name: &str, _port: u8) -> Result<OpenInput, MidiError> {
        Err(MidiError::InputNotFound {
            name: name.to_owned(),
            available: Vec::new(),
        })
    }

    fn open_virtual_input(&self, name: &str, _port: u8) -> Result<OpenInput, MidiError> {
        Err(MidiError::Connect {
            name: name.to_owned(),
            reason: "the recording host's MIDI ports are pinned".to_owned(),
        })
    }
}

/// One scenario's application, agent service and sandbox.
struct Scenario {
    name: String,
    app: Arc<AppContext>,
    service: AgentService,
    root: String,
    _sandbox: TempDir,
}

/// The highest `_NNNNNNNNNNNN` ID number in `text`.
fn max_id(text: &str) -> u64 {
    text.match_indices('_')
        .filter_map(|(i, _)| {
            let digits = text.get(i + 1..i + 13)?;
            digits
                .bytes()
                .all(|b| b.is_ascii_digit())
                .then(|| digits.parse().ok())
                .flatten()
        })
        .max()
        .unwrap_or(0)
}

impl Scenario {
    fn new(golden: &Golden, name: &str) -> Self {
        let sandbox = tempfile::Builder::new()
            .prefix("sonara-golden-")
            .tempdir()
            .expect("a sandbox");
        let root_path = std::fs::canonicalize(sandbox.path()).expect("the sandbox resolves");
        for sub in ["home", "projects", "work", "workspaces"] {
            std::fs::create_dir(root_path.join(sub)).expect("a sandbox folder");
        }
        let raw = golden
            .bytes(&format!("transport/{name}/project.json"))
            .expect("the scenario's project");
        let project = project_from_value(&loads(&raw).expect("JSON")).expect("the project");
        let text = String::from_utf8(raw).expect("UTF-8");
        // The recorder's settings: the defaults, with the backend it ran on.
        let mut settings = Settings::default();
        "native-null".clone_into(&mut settings.audio.backend);
        let app = Arc::new(
            AppContext::create(AppOptions {
                workspaces: Some(root_path.join("workspaces")),
                settings: Some(settings),
                persist_settings: false,
                audio_backend: Some("native-null".to_owned()),
                ids: Some(IdSource::new(Box::new(SequentialIds::new(max_id(&text))))),
                wall_clock: Some(Arc::new(|| {
                    UNIX_EPOCH + Duration::from_secs(FROZEN_UNIX_SECONDS)
                })),
                local_zone: LocalZone::Utc,
                undo_clock: Some(Arc::new(|| 0.0)),
                midi_devices: Some(Arc::new(RecordingHostMidi)),
                // The recorder treated FFmpeg as absent (`audio_import.ffmpeg_path`).
                ffmpeg: Some(Arc::new(|| None)),
                ..AppOptions::default()
            })
            .expect("the application"),
        );
        let workspace = root_path.join("workspaces").join(name);
        std::fs::create_dir_all(&workspace).expect("the workspace");
        app.session().lock().load(
            project,
            ProjectLocation {
                path: None,
                workspace,
                packaged: false,
            },
            false,
        );
        let service = AgentService::new(
            Arc::clone(&app),
            AgentOptions {
                base_dir: Some(root_path.join("work")),
                audit: AuditTarget::Disabled,
                allowed_paths: vec![root_path.clone()],
                environment: Some(PolicyEnvironment {
                    allowed: None,
                    projects_dir: root_path.join("projects"),
                }),
                ..AgentOptions::default()
            },
        )
        .expect("the agent service");
        Self {
            name: name.to_owned(),
            app,
            service,
            root: root_path.to_str().expect("a UTF-8 sandbox").to_owned(),
            _sandbox: sandbox,
        }
    }

    /// The sandbox root as `<ROOT>` in every string.
    fn rooted(&self, value: &Value) -> Value {
        match value {
            Value::String(text) => Value::String(text.replace(&self.root, ROOT)),
            Value::Array(items) => Value::Array(items.iter().map(|v| self.rooted(v)).collect()),
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), self.rooted(v)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    /// Runs one recorded step and compares its answer.
    fn step(&self, index: usize, step: &Value) {
        let what = format!("{} step {index}", self.name);
        let args = &step["args"];
        let actual = match (step["via"].as_str(), step["call"].as_str()) {
            (Some("render"), _) => {
                self.render(step);
                return;
            }
            // The recording speaks API 1.0: a `get_capabilities` answer is compared as its 1.0
            // view, which differs from it by exactly the recorded additions (CTR-01).
            (Some("tool"), Some("get_capabilities")) => {
                recorded_view(&self.service.call("get_capabilities", Some(args)))
            }
            (Some("tool"), Some(tool)) => self.service.call(tool, Some(args)),
            (Some("service"), Some("status")) => self.app.transport().status(),
            (Some("service"), Some("pause")) => self.app.transport().pause(),
            (Some("service"), Some("seek")) => self
                .app
                .transport()
                .seek(args[0].as_f64().expect("a beat"))
                .expect("seek"),
            other => panic!("{what}: unexpected step {other:?}"),
        };
        let recorded = &step["result"];
        compare(&what, recorded, &self.rooted(&actual));
    }

    /// Lets the null device play: as long as the recorded blocks last, or until the transport
    /// stops by itself.
    fn render(&self, step: &Value) {
        if step["until"] == "stopped" {
            let end = Instant::now() + Duration::from_secs(10);
            while self.app.transport().status()["state"] != "stopped" {
                assert!(Instant::now() < end, "{}: the range never ended", self.name);
                thread::sleep(Duration::from_millis(10));
            }
            return;
        }
        let range = step["blocks"].as_array().expect("a block range");
        let blocks = range[1].as_u64().expect("an index") - range[0].as_u64().expect("an index");
        thread::sleep(Duration::from_micros(blocks * 512 * 1_000_000 / 48_000));
    }
}

/// Compares `actual` with `recorded`: equal, except where the recording says `<VOLATILE>`.
fn compare(what: &str, recorded: &Value, actual: &Value) {
    match (recorded, actual) {
        (Value::String(text), _) if text == VOLATILE => {}
        (Value::Object(expected), Value::Object(got)) => {
            let keys: Vec<&String> = got.keys().collect();
            let wanted: Vec<&String> = expected.keys().collect();
            let mut sorted_keys = keys.clone();
            sorted_keys.sort();
            let mut sorted_wanted = wanted.clone();
            sorted_wanted.sort();
            assert_eq!(sorted_keys, sorted_wanted, "{what}: the keys");
            for (key, value) in expected {
                compare(&format!("{what}.{key}"), value, &got[key]);
            }
        }
        (Value::Array(expected), Value::Array(got)) => {
            assert_eq!(expected.len(), got.len(), "{what}: the length");
            for (i, (e, g)) in expected.iter().zip(got).enumerate() {
                compare(&format!("{what}[{i}]"), e, g);
            }
        }
        _ => assert_eq!(recorded, actual, "{what}"),
    }
}

/// The fields of `recorded` (the recorder's dump of a clip or asset object) as the project file
/// stores them in `stored`; the file leaves out an empty `extra`, which the dump writes as `{}`.
fn stored_fields(recorded: &Value, stored: &Value) -> Value {
    let fields = recorded
        .as_object()
        .expect("an object")
        .iter()
        .map(|(key, value)| {
            let value = match stored.get(key) {
                Some(stored) => stored.clone(),
                None if key == "extra" && value == &json!({}) => json!({}),
                None => Value::Null,
            };
            (key.clone(), value)
        })
        .collect();
    Value::Object(fields)
}

fn replay(name: &str) -> Scenario {
    let golden = Golden::open().expect("golden data");
    let transcript: Value = golden
        .json(&format!("transport/{name}/transcript.json"))
        .expect("the transcript");
    let scenario = Scenario::new(&golden, name);
    assert_eq!(
        scenario.app.transport().status()["backend"],
        transcript["backend"]
    );
    for (index, step) in transcript["steps"]
        .as_array()
        .expect("the steps")
        .iter()
        .enumerate()
    {
        scenario.step(index, step);
    }
    scenario
}

#[test]
fn controls_match_the_golden_transport_area() {
    replay("controls");
}

#[test]
fn play_position_matches_the_golden_transport_area() {
    replay("play-position");
}

#[test]
fn play_range_matches_the_golden_transport_area() {
    replay("play-range");
}

#[test]
fn loop_metronome_matches_the_golden_transport_area() {
    replay("loop-metronome");
}

/// The recording: the answers, and the take's asset and clip as the project stores them (D4:
/// the source offset is one device period longer than the prototype's).
#[test]
fn record_count_in_matches_the_golden_transport_area() {
    let scenario = replay("record-count-in");
    let golden = Golden::open().expect("golden data");
    let transcript: Value = golden
        .json("transport/record-count-in/transcript.json")
        .expect("the transcript");
    let take = &transcript["take"];
    assert_eq!(take["prototype_defect"], "D4");
    let project = project_to_value(&scenario.app.session().project().expect("the project"));
    let track = project["tracks"]
        .as_array()
        .expect("tracks")
        .iter()
        .find(|t| t["id"] == take["track_id"])
        .expect("the take's track");
    let clip = track["clips"]
        .as_array()
        .expect("clips")
        .iter()
        .find(|c| c["id"] == take["clip"]["id"])
        .expect("the take's clip");
    let asset = project["assets"]
        .as_array()
        .expect("assets")
        .iter()
        .find(|a| a["id"] == take["asset"]["id"])
        .expect("the take's asset");
    let mut expected_clip = take["clip"].clone();
    let recorded_offset = expected_clip["source_offset"].as_i64().expect("an offset");
    // The prototype's capture start was one 256-frame period late.
    expected_clip["source_offset"] = json!(recorded_offset + 256);
    compare(
        "the take's clip",
        &expected_clip,
        &stored_fields(&expected_clip, clip),
    );
    assert_eq!(
        clip["source_offset"], 59_136,
        "57 600 + 1 536 frames of latency"
    );
    compare(
        "the take's asset",
        &take["asset"],
        &stored_fields(&take["asset"], asset),
    );
    let workspace = scenario
        .app
        .session()
        .lock()
        .location()
        .expect("a location")
        .workspace
        .clone();
    let file = Path::new(&workspace).join(asset["path"].as_str().expect("a path"));
    assert!(file.is_file(), "the take file {}", file.display());
}
