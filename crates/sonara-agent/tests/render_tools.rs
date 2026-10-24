//! The render, export and analysis tools on the paths the golden batches do not take (TST-03): the port of the render, export
//! and analysis tests of the prototype's `tests/agent/test_tools.py`, plus every branch
//! of `get_audio_analysis`, the naming of outputs and the failure paths. The golden batches are
//! replayed in `golden_transcripts.rs`.

use std::f64::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::permissions::sequential_tokens;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget, shared_tokens};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_app::settings::Settings;
use sonara_core::ids::SequentialIds;
use tempfile::TempDir;

struct Fixture {
    _dir: TempDir,
    root: PathBuf,
    app: Arc<AppContext>,
    service: AgentService,
}

impl Fixture {
    /// An agent whose sandbox is a temporary directory, with a 120 BPM project open.
    fn new() -> Self {
        let fx = Self::empty();
        fx.ok("create_project", &json!({"name": "Agent Test", "bpm": 120}));
        fx
    }

    /// An agent without a project.
    fn empty() -> Self {
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
            Arc::clone(&app),
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
            app,
            service,
        }
    }

    fn work(&self, rel: &str) -> PathBuf {
        self.root.join("work").join(rel)
    }

    fn call(&self, tool: &str, args: &Value) -> Value {
        self.service.call(tool, Some(args))
    }

    /// Calls a tool that must succeed.
    fn ok(&self, tool: &str, args: &Value) -> Value {
        let result = self.call(tool, args);
        assert_eq!(result["ok"], true, "{tool}: {result}");
        result
    }

    /// Calls a tool that must fail with `code`; returns the error.
    fn err(&self, tool: &str, args: &Value, code: &str) -> Value {
        let result = self.call(tool, args);
        assert_eq!(result["ok"], false, "{tool}: {result}");
        assert_eq!(result["error"]["code"], code, "{tool}: {result}");
        result["error"].clone()
    }

    /// Expects `CONFIRMATION_REQUIRED`, then repeats the call with the token.
    fn confirm(&self, tool: &str, args: &Value) -> Value {
        let error = self.err(tool, args, "CONFIRMATION_REQUIRED");
        let mut args = args.clone();
        args["confirmation_token"] = error["details"]["confirmation_token"].clone();
        self.ok(tool, &args)
    }

    /// Drums and bass for 2 bars: enough to render quickly but not silent
    /// (`small_song`). Returns the track IDs.
    fn small_song(&self) -> (String, String) {
        let track = |name: &str, extra: Value| -> String {
            let mut args = json!({"name": name, "kind": "instrument"});
            for (k, v) in extra.as_object().expect("an object") {
                args[k] = v.clone();
            }
            self.ok("create_track", &args)["result"]["track_id"]
                .as_str()
                .expect("a track ID")
                .to_owned()
        };
        let drums = track(
            "Drums",
            json!({"instrument": "sonara.drums", "preset": "Punchy 909"}),
        );
        let bass = track("Bass", json!({"preset": "Analog Bass"}));
        self.ok(
            "generate_midi",
            &json!({"track_id": drums, "kind": "drums", "style": "house", "bars": 2}),
        );
        self.ok(
            "generate_midi",
            &json!({"track_id": bass, "kind": "bass", "key": "A minor",
                    "progression": ["i", "VI"], "beats_per_chord": 4}),
        );
        (drums, bass)
    }
}

/// A 16-bit stereo WAV of `samples` per channel (both channels alike), as `soundfile` writes
/// one.
fn write_wav(path: &Path, rate: u32, samples: &[f64]) {
    let data: Vec<u8> = samples
        .iter()
        .flat_map(|&x| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the value is rounded and clamped to 16 bits first"
            )]
            let v = (x * 32768.0).round().clamp(-32768.0, 32767.0) as i16;
            [v.to_le_bytes(), v.to_le_bytes()].concat()
        })
        .collect();
    let len = u32::try_from(data.len()).expect("a small file");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 4).to_le_bytes());
    bytes.extend_from_slice(&4_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend(data);
    fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
    fs::write(path, bytes).expect("the WAV file");
}

/// `seconds` of a sine at `amp` and `freq` Hz, 48 kHz (`write_tone`).
fn tone(seconds: f64, freq: f64, amp: f64) -> Vec<f64> {
    (0..48_000_u32 * 2)
        .take_while(|&i| f64::from(i) < seconds * 48_000.0)
        .map(|i| amp * (2.0 * PI * freq * f64::from(i) / 48_000.0).sin())
        .collect()
}

fn f64_of(value: &Value) -> f64 {
    value
        .as_f64()
        .unwrap_or_else(|| panic!("a number: {value}"))
}

/// The frames of a WAV file from its `data` chunk (16-bit or float, 2 channels).
fn wav_frames(path: &Path, bytes_per_frame: u64) -> u64 {
    let bytes = fs::read(path).expect("the WAV file");
    let at = bytes
        .windows(4)
        .position(|w| w == b"data")
        .expect("a data chunk");
    let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().expect("four bytes"));
    u64::from(size) / bytes_per_frame
}

#[test]
fn render_preview_waits_runs_in_the_background_and_keeps_its_analysis() {
    let fx = Fixture::new();
    fx.small_song();
    let res = fx.ok("render_preview", &json!({"per_track_levels": true}));
    assert_eq!(res["status"], "succeeded");
    assert!(res["operation_id"].as_str().unwrap().starts_with("op_"));
    let out = &res["result"];
    let path = PathBuf::from(out["path"].as_str().unwrap());
    // The default file: the workspace's previews, named by local time and a new ID.
    assert!(path.is_file());
    assert!(path.with_extension("analysis.json").is_file());
    let name = path.file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with("preview-"), "{name}");
    assert_eq!(path.extension().unwrap(), "wav");
    assert_eq!(
        name.len(),
        "preview-20260101-120000-000000.wav".len(),
        "{name}"
    );
    assert!(path.parent().unwrap().ends_with("cache/previews"));
    let analysis = &out["analysis"];
    assert!(analysis["integrated_lufs"].is_f64() && analysis["true_peak_dbtp"].is_f64());
    if f64_of(&analysis["true_peak_dbtp"]) > -1.0 {
        // A hot mix without a master limiter: the agent is told.
        let codes: Vec<&Value> = out["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| &i["code"])
            .collect();
        assert!(codes.contains(&&json!("TRUE_PEAK")), "{codes:?}");
    }
    // 2 bars at 120 BPM plus the tail.
    assert!(f64_of(&analysis["duration_seconds"]) >= 4.0);
    let mut kind: Vec<&String> = analysis["kind"].as_object().unwrap().keys().collect();
    kind.sort_unstable();
    assert_eq!(kind, ["estimated", "measured"]);
    assert!(
        analysis.get("short_term_lufs").is_none(),
        "the series stays in the sidecar"
    );
    let levels = out["track_levels"].as_object().unwrap();
    let names: Vec<&Value> = levels.values().map(|v| &v["name"]).collect();
    assert_eq!(names, [&json!("Drums"), &json!("Bass")]);
    assert!(levels.values().all(|v| v["integrated_lufs"].is_f64()));
    // A range of one track, without a tail, at a given path: exactly one second.
    let track_id = levels.keys().next().unwrap();
    let one = fx.ok(
        "render_preview",
        &json!({"start": "1:3", "end": "2:1", "tail": "none", "track_id": track_id,
                "path": "previews/bass.wav"}),
    );
    assert_eq!(one["result"]["seconds"], json!(1.0));
    assert_eq!(one["result"]["range_beats"], json!([2.0, 4.0]));
    assert_eq!(wav_frames(&fx.work("previews/bass.wav"), 8), 48_000);
    assert!(fx.work("previews/bass.analysis.json").is_file());
    assert!(!fx.work("previews/.bass.wav.partial").exists());
}

#[test]
fn a_preview_in_the_background_keeps_its_analysis() {
    let fx = Fixture::new();
    fx.small_song();
    let pending = fx.ok(
        "render_preview",
        &json!({"wait": false, "per_track_levels": true}),
    );
    let operation_id = pending["operation_id"].clone();
    assert!(["queued", "running", "succeeded"].contains(&pending["status"].as_str().unwrap()));
    let done = fx.ok(
        "get_operation_status",
        &json!({"operation_id": operation_id, "wait_seconds": 60}),
    );
    assert_eq!(done["status"], "succeeded");
    assert_eq!(done["progress"], json!(1.0));
    assert_eq!(done["message"], "measuring Bass");
    let out = &done["result"];
    let analysis = &out["analysis"];
    let a1 = fx.ok("get_audio_analysis", &json!({"operation_id": operation_id}));
    assert_eq!(a1["ready"], true);
    assert_eq!(
        a1["analysis"]["integrated_lufs"],
        analysis["integrated_lufs"]
    );
    assert!(a1["analysis"].get("short_term_lufs").is_none());
    let series = fx.ok(
        "get_audio_analysis",
        &json!({"operation_id": operation_id, "include_series": true}),
    );
    assert!(series["analysis"]["short_term_lufs"].is_array());
    let a2 = fx.ok(
        "get_audio_analysis",
        &json!({"path": out["path"], "include_series": true}),
    );
    assert_eq!(a2["ready"], true);
    assert!(a2["analysis"]["short_term_lufs"].is_array());
    // Without the series, the sidecar's analysis loses it and keeps its key order.
    let a3 = fx.ok("get_audio_analysis", &json!({"path": out["path"]}));
    let keys: Vec<&String> = a3.as_object().unwrap().keys().collect();
    assert_eq!(keys.first().map(|k| k.as_str()), Some("ready"));
    assert!(a3["analysis"].get("short_term_lufs").is_none());
    assert_eq!(a3["track_levels"], out["track_levels"]);
    let ops = fx.ok("list_operations", &json!({}));
    assert!(
        ops["operations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["operation_id"] == operation_id)
    );
    fx.err(
        "get_operation_status",
        &json!({"operation_id": "op_missing"}),
        "NOT_FOUND",
    );
    let error = fx.err("get_audio_analysis", &json!({}), "INVALID_ARGUMENT");
    assert_eq!(error["message"], "give operation_id or path");
}

#[test]
fn render_preview_failure_paths() {
    // An empty project: nothing to render, reported by the job with its operation.
    let fx = Fixture::new();
    let error = fx.err("render_preview", &json!({}), "INVALID_ARGUMENT");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .starts_with("nothing to render")
    );
    let operation_id = error["details"]["operation_id"].clone();
    assert!(operation_id.is_string(), "{error}");
    // `get_audio_analysis` of a failed operation: no analysis, with its status and error.
    let missing = fx.err(
        "get_audio_analysis",
        &json!({"operation_id": operation_id}),
        "NOT_FOUND",
    );
    assert_eq!(missing["message"], "operation has no analysis");
    assert_eq!(missing["details"]["status"], "failed");
    assert_eq!(missing["details"]["error"]["code"], "INVALID_ARGUMENT");
    // No partial file is left behind.
    let previews = fx.work("never.wav");
    fx.err(
        "render_preview",
        &json!({"path": "never.wav"}),
        "INVALID_ARGUMENT",
    );
    assert!(!previews.exists());
    assert!(!fx.work(".never.wav.partial").exists());
    // Outside the sandbox.
    fx.small_song();
    let outside = tempfile::tempdir().unwrap();
    let path = fs::canonicalize(outside.path()).unwrap().join("p.wav");
    fx.err(
        "render_preview",
        &json!({"path": path.to_str().unwrap()}),
        "PERMISSION_DENIED",
    );
    // An unknown track, before any job.
    fx.err(
        "render_preview",
        &json!({"track_id": "trk_nope"}),
        "NOT_FOUND",
    );
    // Without a project.
    let none = Fixture::empty();
    none.err(
        "render_preview",
        &json!({"track_id": "trk_nope"}),
        "NO_PROJECT",
    );
    none.err("render_preview", &json!({}), "NO_PROJECT");
}

#[test]
fn a_running_preview_is_cancelled_and_one_still_queued_never_runs() {
    let fx = Fixture::new();
    let synth = fx.ok(
        "create_track",
        &json!({"name": "Pad", "kind": "instrument", "preset": "Supersaw Lead"}),
    )["result"]["track_id"]
        .clone();
    let notes: Vec<Value> = [60, 64, 67, 71]
        .iter()
        .map(|p| json!({"pitch": p, "start": 0, "length": 1200}))
        .collect();
    fx.ok(
        "create_midi_clip",
        &json!({"track_id": synth, "start": 0, "length": 1200, "notes": notes}),
    );
    fx.ok(
        "add_effect",
        &json!({"track_id": synth, "plugin_id": "sonara.reverb", "preset": "Cathedral"}),
    );
    let pending = fx.ok("render_preview", &json!({"wait": false}));
    let operation_id = pending["operation_id"].clone();
    // Wait until it runs, so cancellation reaches the render.
    let started = Instant::now();
    loop {
        let status = fx.ok(
            "get_operation_status",
            &json!({"operation_id": operation_id}),
        );
        if status["status"] == "running" {
            break;
        }
        assert_eq!(status["status"], "queued", "{status}");
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the preview never started"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let waiting = fx.ok("get_audio_analysis", &json!({"operation_id": operation_id}));
    assert_eq!(waiting["ready"], false);
    assert_eq!(waiting["status"], "running");
    fx.ok("cancel_operation", &json!({"operation_id": operation_id}));
    let last = fx.ok(
        "get_operation_status",
        &json!({"operation_id": operation_id, "wait_seconds": 60}),
    );
    assert_eq!(last["status"], "cancelled");
    assert_eq!(last["error"]["code"], "CANCELLED");
    // No WAV and no partial file.
    let previews = fx
        .app
        .session()
        .lock()
        .location()
        .unwrap()
        .workspace
        .join("cache/previews");
    let left: Vec<_> = fs::read_dir(&previews)
        .map(|entries| entries.map(|e| e.unwrap().file_name()).collect())
        .unwrap_or_default();
    assert_eq!(left, [] as [std::ffi::OsString; 0]);
    // Held workers: the job stays queued; `get_audio_analysis` says it is not ready.
    let hold = fx.app.jobs().hold_workers();
    let queued = fx.ok("render_preview", &json!({"wait": false, "end": 4}));
    let queued_id = queued["operation_id"].clone();
    assert_eq!(
        fx.ok("get_audio_analysis", &json!({"operation_id": queued_id})),
        json!({"status": "queued", "progress": 0.0, "ready": false, "ok": true,
               "tool": "get_audio_analysis"})
    );
    fx.ok("cancel_operation", &json!({"operation_id": queued_id}));
    hold.release();
    let cancelled = fx.ok(
        "get_operation_status",
        &json!({"operation_id": queued_id, "wait_seconds": 60}),
    );
    assert_eq!(cancelled["status"], "cancelled");
    assert!(cancelled.get("error").is_none());
}

#[test]
fn export_audio_writes_every_format_and_guards_existing_files() {
    let fx = Fixture::new();
    fx.small_song();
    fx.ok(
        "set_metadata",
        &json!({"title": "Export Me", "artist": "Sonara Tests"}),
    );
    let base = fx.work("out/mix.wav");
    let dry = fx.ok(
        "export_audio",
        &json!({"path": "out/mix.wav", "dry_run": true}),
    );
    assert_eq!(dry["committed"], false);
    assert_eq!(dry["requires_confirmation"], false);
    assert_eq!(dry["outputs"], json!([base.to_str().unwrap()]));
    // 2 bars at 120 BPM.
    assert_eq!(dry["seconds"], json!(4.0));
    assert!(!base.exists());
    let res = fx.ok(
        "export_audio",
        &json!({"path": "out/mix.wav", "normalize": "loudness", "target_db": -16,
                "formats": [{"format": "wav", "bit_depth": 24},
                            {"format": "flac", "bit_depth": 16},
                            {"format": "mp3", "bitrate_kbps": 192}]}),
    );
    assert_eq!(res["message"], "encoding");
    let out = &res["result"];
    let outputs = out["outputs"].as_array().unwrap();
    let suffixes: Vec<&str> = outputs
        .iter()
        .map(|o| o["path"].as_str().unwrap().rsplit('.').next().unwrap())
        .collect();
    assert_eq!(suffixes, ["wav", "flac", "mp3"]);
    assert!(outputs.iter().all(|o| o["verified"] == true));
    assert!((f64_of(&out["final_integrated_lufs"]) + 16.0).abs() <= 0.05);
    assert_eq!(out["normalization_target_reached"], true);
    assert_eq!(outputs[0]["bit_depth"], 24);
    assert_eq!(outputs[1]["bit_depth"], 16);
    assert_eq!(outputs[1]["dithered"], true);
    let kbps = f64_of(&outputs[2]["bitrate_kbps"]);
    assert!(kbps > 150.0 && kbps < 230.0, "{kbps}");
    // The project's title is the default title; `software` is always written.
    let written: Vec<&Value> = outputs[1]["metadata_written"]
        .as_array()
        .unwrap()
        .iter()
        .collect();
    assert!(written.contains(&&json!("title")) && written.contains(&&json!("software")));
    // Existing files: a conflict without `overwrite`, which leaves the file as it was (M2,
    // `crates/sonara-media/tests/export_guards.rs`), and a confirmation with it.
    let kept = fs::read(&base).unwrap();
    let error = fx.err("export_audio", &json!({"path": "out/mix.wav"}), "CONFLICT");
    assert_eq!(
        error["details"]["existing"],
        json!([base.to_str().unwrap()])
    );
    assert_eq!(fs::read(&base).unwrap(), kept);
    let dry = fx.ok(
        "export_audio",
        &json!({"path": "out/mix.wav", "overwrite": true, "dry_run": true}),
    );
    assert_eq!(dry["requires_confirmation"], true);
    fx.confirm(
        "export_audio",
        &json!({"path": "out/mix.wav", "overwrite": true}),
    );
    // A range of one track, resampled to mono 44.1 kHz.
    let part = fx.ok(
        "export_audio",
        &json!({"path": "out/part.wav", "start": 0, "end": 4, "tail": "none",
                "formats": [{"format": "wav", "bit_depth": 16, "sample_rate": 44100,
                             "channels": 1}]}),
    );
    let o = &part["result"]["outputs"][0];
    assert_eq!(o["sample_rate"], 44100);
    assert_eq!(o["channels"], 1);
    assert_eq!(o["seconds"], json!(2.0));
    assert_eq!(o["resampled_from"], 48000);
}

#[test]
fn export_audio_names_twin_outputs_and_keeps_its_analysis() {
    let fx = Fixture::new();
    fx.small_song();
    let res = fx.ok(
        "export_audio",
        &json!({"path": "out/mix.wav", "normalize": "peak",
                                           "target_db": -1}),
    );
    let out = &res["result"];
    // Two formats with one extension: the second gets its format in the name.
    let twins = fx.ok(
        "export_audio",
        &json!({"path": "out/twins", "tail": "none",
                "formats": [{"format": "wav"}, {"format": "bwf"}]}),
    );
    let paths: Vec<&Value> = twins["result"]["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| &o["path"])
        .collect();
    assert_eq!(
        paths,
        [
            &json!(fx.work("out/twins.wav").to_str().unwrap()),
            &json!(fx.work("out/twins-bwf.wav").to_str().unwrap())
        ]
    );
    // The analysis of an export: before the gain.
    let analysis = fx.ok(
        "get_audio_analysis",
        &json!({"operation_id": res["operation_id"]}),
    );
    let keys: Vec<&String> = analysis.as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        [
            "ready",
            "analysis_before_gain",
            "gain_db",
            "final_integrated_lufs",
            "final_true_peak_dbtp",
            "ok",
            "tool"
        ]
    );
    assert_eq!(analysis["gain_db"], out["gain_db"]);
}

#[test]
fn export_audio_failure_paths() {
    let fx = Fixture::new();
    fx.small_song();
    // A rate no resampler takes (a deviation: the prototype failed inside the resampler).
    let error = fx.err(
        "export_audio",
        &json!({"path": "x.wav", "formats": [{"format": "wav", "sample_rate": -1}]}),
        "INVALID_ARGUMENT",
    );
    assert_eq!(error["details"]["sample_rate"], -1);
    // Encodings a container does not write.
    fx.err(
        "export_audio",
        &json!({"path": "x.mp3", "formats": [{"format": "flac", "floating_point": true}]}),
        "UNSUPPORTED",
    );
    fx.err(
        "export_audio",
        &json!({"path": "x.wav", "formats": [{"format": "wav", "bitrate_kbps": 128}]}),
        "INVALID_ARGUMENT",
    );
    // An empty range, and an unknown track.
    fx.err(
        "export_audio",
        &json!({"path": "x.wav", "start": 8, "end": 8}),
        "INVALID_ARGUMENT",
    );
    fx.err(
        "export_audio",
        &json!({"path": "x.wav", "track_id": "trk_nope"}),
        "NOT_FOUND",
    );
    let outside = tempfile::tempdir().unwrap();
    let path = fs::canonicalize(outside.path()).unwrap().join("x.wav");
    fx.err(
        "export_audio",
        &json!({"path": path.to_str().unwrap()}),
        "PERMISSION_DENIED",
    );
    assert!(!fx.work("x.wav").exists());
}

#[test]
fn export_stems_one_file_per_track() {
    let fx = Fixture::new();
    let (drums, bass) = fx.small_song();
    let res = fx.ok(
        "export_stems",
        &json!({"directory": "stems", "format": {"format": "flac", "bit_depth": 24}}),
    );
    assert_eq!(res["message"], "stem Bass");
    let stems = res["result"]["stems"].as_array().unwrap();
    let names: Vec<&Value> = stems.iter().map(|s| &s["name"]).collect();
    assert_eq!(names, [&json!("Drums"), &json!("Bass")]);
    let files: Vec<&Value> = stems.iter().map(|s| &s["path"]).collect();
    assert_eq!(
        files,
        [
            &json!(
                fx.work("stems/Agent-Test---01-Drums.flac")
                    .to_str()
                    .unwrap()
            ),
            &json!(fx.work("stems/Agent-Test---02-Bass.flac").to_str().unwrap())
        ]
    );
    for stem in stems {
        assert!(Path::new(stem["path"].as_str().unwrap()).is_file());
        assert!(stem["integrated_lufs"].is_f64() && stem["peak_dbtp"].is_f64());
        assert_eq!(stem["container"], "flac");
    }
    assert_eq!(res["result"]["through_master"], false);
    let one = fx.ok(
        "export_stems",
        &json!({"directory": "stems2", "track_ids": [bass], "through_master": true}),
    );
    assert_eq!(one["result"]["stems"].as_array().unwrap().len(), 1);
    assert_eq!(one["result"]["through_master"], true);
    let dry = fx.ok(
        "export_stems",
        &json!({"directory": "stems", "overwrite": true, "dry_run": true}),
    );
    assert_eq!(dry["requires_confirmation"], true);
    assert_eq!(dry["directory"], json!(fx.work("stems").to_str().unwrap()));
    fx.confirm(
        "export_stems",
        &json!({"directory": "stems", "overwrite": true, "track_ids": [drums]}),
    );
    // Without `overwrite`, an existing stem is a conflict of the job.
    let conflict = fx.err(
        "export_stems",
        &json!({"directory": "stems", "track_ids": [drums],
                "format": {"format": "flac", "bit_depth": 24}}),
        "CONFLICT",
    );
    assert!(conflict["details"]["operation_id"].is_string());
    let error = fx.err(
        "export_stems",
        &json!({"directory": "none", "track_ids": []}),
        "INVALID_ARGUMENT",
    );
    assert_eq!(error["message"], "no tracks to export as stems");
    fx.err(
        "export_stems",
        &json!({"directory": "none", "track_ids": ["trk_nope"]}),
        "NOT_FOUND",
    );
}

#[test]
fn analyze_audio_measures_files_assets_and_renders() {
    let fx = Fixture::new();
    let tone_path = fx.work("audio/tone.wav");
    write_wav(&tone_path, 48_000, &tone(2.0, 440.0, 0.5));
    let by_path = fx.ok("analyze_audio", &json!({"path": "audio/tone.wav"}));
    let keys: Vec<&String> = by_path.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["path", "analysis", "issues", "ok", "tool"]);
    let analysis = &by_path["analysis"];
    assert!((f64_of(&analysis["sample_peak_dbfs"]) + 6.02).abs() < 0.05);
    assert!((f64_of(&analysis["correlation"]) - 1.0).abs() < 1e-6);
    let imported = fx.ok("import_audio", &json!({"path": "audio/tone.wav"}));
    let by_asset = fx.ok("analyze_audio", &json!({"asset_id": imported["asset_id"]}));
    assert_eq!(by_asset["asset_id"], imported["asset_id"]);
    assert_eq!(
        by_asset["analysis"]["integrated_lufs"],
        analysis["integrated_lufs"]
    );
    let render = fx.ok(
        "analyze_audio",
        &json!({"render": {"tail": "none"}, "per_track_levels": true}),
    );
    assert_eq!(render["seconds"], json!(2.0));
    assert!(!render["track_levels"].as_object().unwrap().is_empty());
    for args in [
        json!({}),
        json!({"path": "audio/tone.wav", "asset_id": imported["asset_id"]}),
    ] {
        let error = fx.err("analyze_audio", &args, "INVALID_ARGUMENT");
        assert_eq!(
            error["message"],
            "give exactly one of path, asset_id or render"
        );
    }
    let error = fx.err(
        "analyze_audio",
        &json!({"path": "audio/none.wav"}),
        "NOT_FOUND",
    );
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .ends_with("none.wav does not exist")
    );
    fx.err(
        "analyze_audio",
        &json!({"asset_id": "med_nope"}),
        "NOT_FOUND",
    );
    // An empty path is given but names nothing: the prototype's assertion fails.
    fx.err("analyze_audio", &json!({"path": ""}), "INTERNAL");
    // A clipped file.
    let clipped: Vec<f64> = tone(1.0, 100.0, 2.0)
        .into_iter()
        .map(|x| x.clamp(-1.0, 1.0))
        .collect();
    write_wav(&fx.work("audio/clip.wav"), 48_000, &clipped);
    let result = fx.ok("analyze_audio", &json!({"path": "audio/clip.wav"}));
    let codes: Vec<&Value> = result["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| &i["code"])
        .collect();
    assert!(codes.contains(&&json!("CLIPPING")), "{codes:?}");
    // A file without a sidecar is analyzed when asked for.
    let fresh = fx.ok("get_audio_analysis", &json!({"path": "audio/tone.wav"}));
    let keys: Vec<&String> = fresh.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["path", "analysis", "issues", "ready", "ok", "tool"]);
    // The result of an operation without an analysis is returned as it is.
    let resampled = fx.work("audio/low.wav");
    write_wav(&resampled, 44_100, &tone(0.5, 440.0, 0.5));
    let imported = fx.ok("import_audio", &json!({"path": "audio/low.wav"}));
    let operation_id = imported["decode_operation_id"].clone();
    assert!(operation_id.is_string(), "{imported}");
    fx.ok(
        "get_operation_status",
        &json!({"operation_id": operation_id, "wait_seconds": 60}),
    );
    let other = fx.ok("get_audio_analysis", &json!({"operation_id": operation_id}));
    assert_eq!(other["ready"], true);
    assert_eq!(other["result"]["decoded"], true);
}

#[test]
fn capabilities_list_the_export_formats() {
    let fx = Fixture::empty();
    let caps = fx.ok("get_capabilities", &json!({}));
    let formats: Vec<&str> = caps["export_formats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap())
        .collect();
    let mut sorted = formats.clone();
    sorted.sort_unstable();
    assert_eq!(formats, sorted);
    for format in [
        "aiff", "bwf", "caf", "flac", "ogg", "opus", "rf64", "w64", "wav",
    ] {
        assert!(formats.contains(&format), "{format}");
    }
}

#[test]
fn stems_whose_names_are_cut_alike_are_told_apart() {
    let fx = Fixture::empty();
    // 118 characters: `<name> - 01 Drums` and `<name> - 02 Bass` are cut to 120 characters.
    let long = "L".repeat(118);
    fx.ok("create_project", &json!({"name": long, "bpm": 120}));
    fx.small_song();
    let res = fx.ok(
        "export_stems",
        &json!({"directory": "stems", "tail": "none"}),
    );
    let paths: Vec<&Value> = res["result"]["stems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["path"])
        .collect();
    let first = format!("stems/{long}--.wav");
    let second = format!("stems/{long}--_.wav");
    assert_eq!(
        paths,
        [
            &json!(fx.work(&first).to_str().unwrap()),
            &json!(fx.work(&second).to_str().unwrap())
        ]
    );
}

#[test]
fn sidecars_are_python_json_with_ascii_escapes() {
    let fx = Fixture::new();
    let track = fx.ok(
        "create_track",
        &json!({"name": "Třináctá", "kind": "instrument", "instrument": "sonara.drums"}),
    )["result"]["track_id"]
        .clone();
    fx.ok(
        "generate_midi",
        &json!({"track_id": track, "kind": "drums", "bars": 1}),
    );
    fx.ok(
        "render_preview",
        &json!({"path": "previews/cz.wav", "per_track_levels": true, "tail": "none"}),
    );
    let bytes = fs::read(fx.work("previews/cz.analysis.json")).expect("the sidecar");
    assert!(bytes.is_ascii());
    let text = String::from_utf8(bytes).expect("ASCII");
    // `json.dumps(stored, indent=1)`: one-space indents, `ensure_ascii`, no final newline.
    assert!(text.starts_with("{\n \"path\": "), "{text}");
    assert!(
        text.contains("\"name\": \"T\\u0159in\\u00e1ct\\u00e1\""),
        "{text}"
    );
    assert!(text.ends_with('}'));
}
