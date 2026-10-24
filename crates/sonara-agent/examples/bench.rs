//! The Rust counterpart of the prototype's `scripts/bench.py`, for the offline render exit
//! criterion of (PERF-01, PERF-08, PERF-10).
//!
//! It builds the same projects through the same agent tools, renders each offline without a
//! tail and times only the render, then times save, open and a WAV, FLAC and MP3 export of the
//! 8-track 48 kHz project, as `bench.py` does. It prints JSON with the keys of `bench.py` plus
//! `"implementation": "rust"`. The numbers are measurements on the machine that runs it, not
//! guarantees.
//!
//! ```text
//! cargo run --release -p sonara-agent --example bench [-- --quick]
//! ```
//!
//! Like `bench.py`, it never touches the user's settings, logs or projects: the settings are the
//! defaults and are not persisted, the workspaces, the projects folder and every file live in a
//! temporary directory, and calls are not audited.
//!
//! Before each render it checks that the project is the one `bench.py` builds (the reverb bus
//! plus `tracks` tracks, `bars` bars of 4/4) and prints a summary of the project to stderr: the
//! length; each track's name, instrument, preset, effect and send counts, and its clips' note
//! counts and pitch ranges; and the master's effect count. The prototype's MIDI generators are
//! seeded (`seed` defaults to 0), so the summary equals the prototype's for the same case. The
//! JSON on stdout keeps the keys of `bench.py`, except that `machine` reports the Rust version
//! and the build profile instead of `python` and `native_kernels`.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use serde_json::{Map, Value, json};
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::render::{RenderRequest, Tail};
use sonara_app::settings::Settings;
use sonara_core::pyfmt::py_round;

type BenchResult<T> = Result<T, Box<dyn Error>>;

/// `(tracks, bars, sample rate)` of each case (`cases` in `bench.py`).
const CASES: [(usize, u32, u32); 5] = [
    (4, 8, 48_000),
    (8, 8, 48_000),
    (16, 8, 48_000),
    (8, 8, 44_100),
    (8, 8, 96_000),
];
/// The cases of `--quick`.
const QUICK_CASES: [(usize, u32, u32); 2] = [(4, 4, 48_000), (8, 4, 48_000)];
/// The case whose project is also saved, opened and exported: `(tracks, sample rate)`.
const FILE_CASE: (usize, u32) = (8, 48_000);
/// The synth presets, picked by track index.
const PRESETS: [&str; 6] = [
    "Analog Bass",
    "Warm Pad",
    "Pluck",
    "Strings",
    "Supersaw Lead",
    "Electric Keys",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("bench: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> BenchResult<()> {
    let mut quick = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--quick" => quick = true,
            other => {
                return Err(format!("unknown argument {other:?}; usage: bench [--quick]").into());
            }
        }
    }
    let cases: &[(usize, u32, u32)] = if quick { &QUICK_CASES } else { &CASES };
    let dir = tempfile::Builder::new()
        .prefix("sonara-bench-proj-")
        .tempdir()?;
    // The path policy compares resolved paths (`/var` is `/private/var` on macOS).
    let root = fs::canonicalize(dir.path())?;
    let mut render = Vec::with_capacity(cases.len());
    let mut files = Map::new();
    for &(tracks, bars, sample_rate) in cases {
        let bench = Bench::new(&root)?;
        build(&bench, tracks, bars, sample_rate)?;
        eprintln!("{}", check(&bench, tracks, bars)?);
        let label = format!("{tracks} tracks, {bars} bars, {sample_rate} Hz");
        render.push(bench_render(&bench, &label)?);
        if (tracks, sample_rate) == FILE_CASE {
            files = bench_files(&bench, &root)?;
        }
        bench.app.close();
    }
    let mut results = Map::new();
    results.insert("implementation".to_owned(), json!("rust"));
    results.insert("machine".to_owned(), machine());
    results.insert("render".to_owned(), Value::Array(render));
    results.extend(files);
    println!("{}", serde_json::to_string_pretty(&results)?);
    Ok(())
}

/// What the numbers were measured with. `bench.py`'s `python` and `native_kernels` have no
/// meaning here; the Rust version and the build profile take their place.
fn machine() -> Value {
    json!({
        "platform": format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        "processor": std::env::consts::ARCH,
        "rust": env!("CARGO_PKG_RUST_VERSION"),
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
    })
}

/// An application with the null audio backend and an agent over it, both confined to `root`.
struct Bench {
    app: Arc<AppContext>,
    agent: AgentService,
}

impl Bench {
    /// `AppContext.create(AppOptions(workspaces=tmp / "ws", settings=Settings(),
    /// persist_settings=False, audio_backend="null"))` and
    /// `AgentService(app, audit_path=False, allowed_paths=[tmp])`. `bench.py` keeps the user's
    /// directories out through a temporary `SONARA_HOME`; here the options do it: the settings
    /// are given and not persisted, auditing is off, and the sandbox's projects folder and
    /// environment are fixed.
    fn new(root: &Path) -> BenchResult<Self> {
        let app = Arc::new(AppContext::create(AppOptions {
            workspaces: Some(root.join("ws")),
            settings: Some(Settings::default()),
            persist_settings: false,
            audio_backend: Some("null".to_owned()),
            ..AppOptions::default()
        })?);
        let agent = AgentService::new(
            Arc::clone(&app),
            AgentOptions {
                base_dir: Some(root.to_path_buf()),
                audit: AuditTarget::Disabled,
                allowed_paths: vec![root.to_path_buf()],
                environment: Some(PolicyEnvironment {
                    allowed: None,
                    projects_dir: root.join("projects"),
                }),
                ..AgentOptions::default()
            },
        )?;
        Ok(Self { app, agent })
    }

    /// Calls `tool`; returns its envelope whether it succeeded or not.
    fn call(&self, tool: &str, args: &Value) -> Value {
        self.agent.call(tool, Some(args))
    }

    /// Calls `tool`, which must succeed. `bench.py` checks only `create_project`; a call that
    /// failed silently would make the projects differ, so every call is checked here.
    fn ok(&self, tool: &str, args: &Value) -> BenchResult<Value> {
        let response = self.call(tool, args);
        succeeded(tool, response)
    }

    /// Creates a track and returns its ID.
    fn track(&self, args: &Value) -> BenchResult<String> {
        let response = self.ok("create_track", args)?;
        response["result"]["track_id"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("create_track returned no track ID: {response}").into())
    }
}

/// `response` when its `ok` is true, else an error naming `tool`.
fn succeeded(tool: &str, response: Value) -> BenchResult<Value> {
    if response["ok"] == true {
        Ok(response)
    } else {
        Err(format!("{tool} failed: {response}").into())
    }
}

/// `build` of `bench.py`, call for call: a reverb bus, then drums on every fourth track and
/// synths with an EQ, a compressor and a send to the bus on the others, and a limiter on the
/// master.
fn build(bench: &Bench, tracks: usize, bars: u32, sample_rate: u32) -> BenchResult<()> {
    bench.ok(
        "create_project",
        &json!({"name": "Bench", "bpm": 124, "sample_rate": sample_rate}),
    )?;
    let verb = bench.track(&json!({"name": "Reverb", "kind": "bus"}))?;
    bench.ok(
        "add_effect",
        &json!({"track_id": verb, "plugin_id": "sonara.reverb",
                "params": {"mix": 1.0, "decay_s": 2.5}}),
    )?;
    for i in 0..tracks {
        if i % 4 == 0 {
            let track = bench
                .track(&json!({"name": format!("Drums {i}"), "instrument": "sonara.drums"}))?;
            bench.ok(
                "generate_midi",
                &json!({"track_id": track, "kind": "drums", "style": "house", "bars": bars}),
            )?;
        } else {
            let preset = PRESETS[i % PRESETS.len()];
            let track = bench.track(&json!({"name": format!("Synth {i}"), "preset": preset}))?;
            let kind = match i % 4 {
                1 => "bass",
                2 => "chords",
                _ => "arpeggio",
            };
            let octave = if kind == "bass" { 2 } else { 4 };
            bench.ok(
                "generate_midi",
                &json!({"track_id": track, "kind": kind, "key": "A minor",
                        "progression": ["i", "VI", "III", "VII"],
                        "beats_per_chord": bars, "octave": octave}),
            )?;
            bench.ok(
                "add_effect",
                &json!({"track_id": track, "plugin_id": "sonara.eq.parametric",
                        "params": {"mid1_gain": 2.0}}),
            )?;
            bench.ok(
                "add_effect",
                &json!({"track_id": track, "plugin_id": "sonara.dynamics.compressor"}),
            )?;
            bench.ok(
                "configure_routing",
                &json!({"track_id": track, "add_sends": [{"target": verb, "level_db": -12}]}),
            )?;
        }
    }
    bench.ok(
        "add_effect",
        &json!({"track_id": "master", "plugin_id": "sonara.dynamics.limiter"}),
    )?;
    Ok(())
}

/// Checks that the open project is the one `bench.py` builds for `tracks` and `bars`: the bus
/// plus `tracks` tracks, `4 * bars` beats long. Returns a summary from `get_project_state`:
/// the length, then per track its name, instrument, preset, effect and send counts, and each
/// clip's note count and pitch range, then the master's effect count.
fn check(bench: &Bench, tracks: usize, bars: u32) -> BenchResult<String> {
    let response = bench.ok("get_project_state", &json!({}))?;
    let state = &response["state"];
    let listed = state["tracks"].as_array().map_or(0, Vec::len);
    let beats = state["length"]["beat"].as_f64();
    if listed != tracks + 1 || beats != Some(f64::from(4 * bars)) {
        return Err(format!(
            "the project is not bench.py's: {listed} tracks and {beats:?} beats, expected {} \
             tracks and {} beats",
            tracks + 1,
            4 * bars
        )
        .into());
    }
    let count = |value: &Value| value.as_array().map_or(0, Vec::len);
    let summaries: Vec<String> = state["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|track| {
            let clips: Vec<String> = track["clips"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|clip| {
                    format!(
                        "{} {}-{}",
                        clip["note_count"], clip["pitch_range"][0], clip["pitch_range"][1]
                    )
                })
                .collect();
            format!(
                "{} {} {} fx={} sends={} [{}]",
                track["name"],
                track["instrument"]["plugin_id"],
                track["instrument"]["preset"],
                count(&track["effects"]),
                count(&track["sends"]),
                clips.join(", ")
            )
        })
        .collect();
    Ok(format!(
        "{} beats, {} s; {}; master fx={}",
        state["length"]["beat"],
        state["length"]["seconds"],
        summaries.join("; "),
        count(&state["master"]["effects"])
    ))
}

/// `bench_render` of `bench.py`: renders the open project offline without a tail and times the
/// render, from creating the renderer to the last block (`RenderService::render_array`).
fn bench_render(bench: &Bench, label: &str) -> BenchResult<Value> {
    let request = RenderRequest {
        tail: Tail::None,
        ..RenderRequest::default()
    };
    let started = Instant::now();
    let out = bench.app.render().render_array(&request)?;
    let elapsed = started.elapsed().as_secs_f64();
    let seconds = out.seconds();
    Ok(json!({
        "case": label,
        "audio_seconds": py_round(seconds, 2),
        "render_seconds": py_round(elapsed, 3),
        "realtime_factor": py_round(seconds / elapsed, 2),
    }))
}

/// Times saving the open project, opening it again and exporting it to WAV, FLAC and MP3 in one
/// call, as `bench.py` does. Returns `save_seconds`, `open_seconds` and `export_wav_flac_mp3`.
fn bench_files(bench: &Bench, root: &Path) -> BenchResult<Map<String, Value>> {
    let project = utf8(&root.join("bench.sonara"))?;
    let mut out = Map::new();

    let started = Instant::now();
    let saved = bench.call("save_project", &json!({"path": project}));
    out.insert(
        "save_seconds".to_owned(),
        json!(py_round(started.elapsed().as_secs_f64(), 4)),
    );
    succeeded("save_project", saved)?;

    let started = Instant::now();
    let opened = bench.call("open_project", &json!({"path": project}));
    out.insert(
        "open_seconds".to_owned(),
        json!(py_round(started.elapsed().as_secs_f64(), 4)),
    );
    succeeded("open_project", opened)?;

    let started = Instant::now();
    let exported = bench.call(
        "export_audio",
        &json!({"path": utf8(&root.join("bench.wav"))?,
                "formats": [{"format": "wav"}, {"format": "flac"}, {"format": "mp3"}]}),
    );
    let elapsed = started.elapsed().as_secs_f64();
    // `bench.py` reports `None` for a failed export instead of stopping.
    let seconds = if exported["ok"] == true {
        exported["result"]["outputs"][0]["seconds"].clone()
    } else {
        eprintln!("export_audio failed: {exported}");
        Value::Null
    };
    out.insert(
        "export_wav_flac_mp3".to_owned(),
        json!({"seconds": py_round(elapsed, 3), "audio_seconds": seconds}),
    );
    Ok(out)
}

/// `path` as a string for a tool argument.
fn utf8(path: &Path) -> BenchResult<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{} is not UTF-8", path.display()).into())
}
