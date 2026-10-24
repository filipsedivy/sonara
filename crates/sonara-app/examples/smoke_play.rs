//! Headless smoke playback for the hardware baselines (`docs/audio-engine.md`): the measurement
//! of the prototype's `sonara-gui --smoke-play SECONDS <project>`, without a GUI.
//! It is an example rather than a `sonara` subcommand, so the command tree that
//! `golden/contracts/cli.json` pins stays unchanged (CTR-01).
//!
//! ```text
//! cargo run --release -p sonara-app --example smoke_play -- \
//!     [--seconds 15] [--backend native] [--lookahead-ms 80] <project>
//! ```
//!
//! What it does, in the prototype's order:
//!
//! 1. opens `<project>` on the audio backend (default `native`: the system's default output
//!    device) with the default audio settings: 48 kHz, a 256-frame period, 80 ms lookahead
//!    (`--lookahead-ms` measures another, such as the 25 ms of live input);
//! 2. sets the master to −144 dB with `SetTrackMixer`, not the mute flag, so the whole graph
//!    still renders and nothing is audible;
//! 3. plays from beat 0 for `--seconds` (default 15), reads the transport's status, stops;
//! 4. prints the prototype's keys on stdout, as its `json.dumps` writes them: `state`,
//!    `last_error`, `device_status`, `underruns`, `render_load`, `callback_load`, `queued_ms`,
//!    `lookahead_ms`, `position_seconds`, `device_sample_rate` and `buffer_frames` (`null` when
//!    the status has no such key).
//!
//! `render_load` has the prototype's definition: the time spent in the graph and the carry
//! (`_next_block`) over the audio time of the blocks rendered, cumulative since the player was
//! created; `callback_load` is the last callback's processing time over its period.
//!
//! A second JSON line on stderr records the conditions, which the prototype's line lacks: the
//! backend, the default playback device, `output_latency_estimate_ms`, `device_errors` (for
//! the unplug and reroute runs), the seconds played, the build profile and the project.
//!
//! On Core Audio the render thread joins the output device's audio workgroup with a
//! time-constraint policy, from a terminal as from an app bundle (docs/audio-engine.md,
//! "The render thread's audio workgroup"). Only where it joins none does it fall back to the
//! user-interactive `QoS` class, which macOS squashes to the default class for a process without
//! an application role, such as this example started from a terminal; to measure with that class
//! in effect, start it from an app bundle with `open` (docs/audio-engine.md, "The render
//! thread's `QoS` class").
//!
//! It never saves anything: the settings are the defaults and are not persisted, the workspaces
//! live in a temporary directory, and the project is closed with its changes discarded. Opening
//! a bundle reads it in place, so measure a copy to be sure an example project stays untouched.
//! The measured numbers belong in docs/audio-engine.md (HW-01), never in a CI gate.

use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

use serde_json::{Map, Value, json};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::settings::Settings;
use sonara_core::commands::track::SetTrackMixer;
use sonara_core::ids::MASTER_ID;
use sonara_io::DeviceKind;

/// The keys `_smoke_play` prints, in its order.
const KEYS: [&str; 11] = [
    "state",
    "last_error",
    "device_status",
    "underruns",
    "render_load",
    "callback_load",
    "queued_ms",
    "lookahead_ms",
    "position_seconds",
    "device_sample_rate",
    "buffer_frames",
];

/// The prototype's measurement level for the master: inaudible, but rendered.
const MASTER_DB: f64 = -144.0;

/// The prototype's lookahead (`settings.audio.lookahead_ms`), set explicitly.
const LOOKAHEAD_MS: f64 = 80.0;

const USAGE: &str =
    "usage: smoke_play [--seconds SECONDS] [--backend NAME] [--lookahead-ms MS] <project>";

type SmokeResult<T> = Result<T, Box<dyn Error>>;

struct Args {
    project: PathBuf,
    seconds: f64,
    backend: String,
    lookahead_ms: f64,
}

fn parse_args() -> SmokeResult<Args> {
    let mut project = None;
    let mut seconds: f64 = 15.0;
    let mut backend = "native".to_owned();
    let mut lookahead_ms: f64 = LOOKAHEAD_MS;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seconds" => {
                seconds = args.next().ok_or("--seconds needs a value")?.parse()?;
            }
            "--backend" => backend = args.next().ok_or("--backend needs a value")?,
            "--lookahead-ms" => {
                lookahead_ms = args.next().ok_or("--lookahead-ms needs a value")?.parse()?;
            }
            "-h" | "--help" => return Err(USAGE.into()),
            _ if project.is_none() => project = Some(PathBuf::from(arg)),
            other => return Err(format!("unexpected argument: {other}").into()),
        }
    }
    let project = project.ok_or(USAGE)?;
    if !(seconds.is_finite() && seconds > 0.0) {
        return Err("--seconds must be positive".into());
    }
    if !(lookahead_ms.is_finite() && lookahead_ms > 0.0) {
        return Err("--lookahead-ms must be positive".into());
    }
    Ok(Args {
        project,
        seconds,
        backend,
        lookahead_ms,
    })
}

/// `json.dumps(value)` for a flat object: `", "` and `": "` separators, keys in order.
fn python_line(values: &Map<String, Value>) -> String {
    let fields: Vec<String> = values
        .iter()
        .map(|(key, value)| format!("{}: {value}", Value::String(key.clone())))
        .collect();
    format!("{{{}}}", fields.join(", "))
}

fn run(args: &Args) -> SmokeResult<bool> {
    let workspaces = tempfile::tempdir()?;
    let mut settings = Settings::default();
    settings.audio.lookahead_ms = args.lookahead_ms;
    let app = AppContext::create(AppOptions {
        workspaces: Some(workspaces.path().to_owned()),
        settings: Some(settings),
        persist_settings: false,
        audio_backend: Some(args.backend.clone()),
        ..AppOptions::default()
    })?;
    app.projects().open_project(&args.project, false)?;
    app.session().lock().execute(
        &SetTrackMixer {
            track_id: MASTER_ID.to_owned(),
            volume_db: Some(MASTER_DB),
            pan: None,
            mute: None,
            solo: None,
            solo_safe: None,
        },
        false,
        "smoke",
    )?;
    let transport = app.transport();
    let device = transport.devices().ok().and_then(|devices| {
        devices
            .into_iter()
            .find(|device| device.kind == DeviceKind::Playback && device.is_default)
            .map(|device| device.name)
    });
    let started = transport.play(Some(0.0), None, false)?;
    let playing = started.get("state") == Some(&json!("playing"));
    if playing {
        thread::sleep(Duration::from_secs_f64(args.seconds));
        let status = transport.status();
        transport.stop()?;
        let keys: Map<String, Value> = KEYS
            .iter()
            .map(|&key| {
                (
                    key.to_owned(),
                    status.get(key).cloned().unwrap_or(Value::Null),
                )
            })
            .collect();
        println!("{}", python_line(&keys));
        let conditions = json!({
            "backend": status.get("backend"),
            "default_playback_device": device,
            "output_latency_estimate_ms": status.get("output_latency_estimate_ms"),
            "device_errors": status.get("device_errors"),
            "seconds": args.seconds,
            "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "project": args.project.display().to_string(),
        });
        eprintln!("{conditions}");
    } else {
        println!(
            "{}",
            json!({"error": "playback did not start", "status": started})
        );
    }
    app.projects().close(true)?;
    app.close();
    Ok(playing)
}

fn main() -> ExitCode {
    let result = parse_args().and_then(|args| run(&args));
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("smoke_play: {error}");
            ExitCode::FAILURE
        }
    }
}
