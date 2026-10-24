//! `sonara info`, `analyze`, `verify` and `devices`, and the commands that cannot run here (the
//! GUI, and attaching without a running session).
//!
//! Oracles: the prototype's analysis of the workflow renders (`golden/analysis/workflows/`,
//! recorded with the same 65536-frame float64 blocks as `cmd_analyze`), its inspection of the
//! media fixtures (`golden/media/info.json`, `inspect_audio(path).to_json()`), and for projects
//! the state view the creating tool reported.

use std::fs;

use serde_json::{Value, json};
use sonara_dsp::analysis::{Analyzer, Issue};
use sonara_testkit::Golden;

use crate::support::Sandbox;

/// `<ROOT>` for the sandbox root in every string.
fn normalized(value: &Value, root: &str) -> Value {
    match value {
        Value::String(text) => Value::String(text.replace(root, "<ROOT>")),
        Value::Array(items) => Value::Array(items.iter().map(|v| normalized(v, root)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), normalized(v, root)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The report fields whose parity with the prototype holds within the documented tolerances of
/// `sonara-dsp`'s `golden_analysis` test (libm, summation order, FFT rounding); every other
/// field is exact.
const TOLERANT_FIELDS: [&str; 13] = [
    "sample_peak_dbfs",
    "true_peak_dbtp",
    "rms_dbfs",
    "crest_factor_db",
    "integrated_lufs",
    "momentary_max_lufs",
    "short_term_max_lufs",
    "loudness_range_lu",
    "plr_db",
    "balance_db",
    "side_to_mid_db",
    "spectral_centroid_hz",
    "octave_bands_db",
];

#[test]
fn analyze_equals_the_golden_reports() {
    let golden = Golden::open().unwrap();
    let sandbox = Sandbox::new();
    // The two shortest renders; `sonara-dsp` checks the analyzer on every render.
    for name in ["mix_analysis", "sidechain_preview"] {
        let rel = format!("workflows/{name}/render.wav");
        let recorded: Value = golden
            .json(&format!("analysis/workflows/{name}.json"))
            .unwrap();
        let render = golden.root().join(&rel);
        let render = render.to_str().unwrap();
        let run = sandbox.run(&["analyze", "--series", render]);
        assert_eq!(run.code, 0, "{}", run.stderr);
        // Exactly the analyzer's report on the samples the prototype fed (the float32 WAV read
        // as float64), in the same blocks: the command reads and feeds the file as
        // `cmd_analyze` did.
        let audio = golden.wav(&rel).unwrap();
        let channels = usize::from(audio.channels);
        let samples: Vec<f64> = audio.samples.iter().map(|&s| f64::from(s)).collect();
        let mut analyzer = Analyzer::new(audio.sample_rate, channels).unwrap();
        for block in samples.chunks(65_536 * channels) {
            analyzer.feed(block);
        }
        let report = analyzer.finish();
        let issues: Vec<Value> = report.issues().iter().map(Issue::to_json).collect();
        assert_eq!(
            run.json,
            json!({"analysis": report.to_json(true), "issues": issues}),
            "{name}"
        );
        // The recording: the same keys and issues, and every exact field equal.
        assert_eq!(run.json["issues"], recorded["issues"], "{name}");
        let actual = run.json["analysis"].as_object().unwrap();
        let expected = recorded["analysis"].as_object().unwrap();
        let mut keys: Vec<&String> = actual.keys().collect();
        let mut recorded_keys: Vec<&String> = expected.keys().collect();
        keys.sort();
        recorded_keys.sort();
        assert_eq!(keys, recorded_keys, "{name}");
        for (key, value) in expected {
            if !TOLERANT_FIELDS.contains(&key.as_str()) {
                assert_eq!(&actual[key], value, "{name}: {key}");
            }
        }
        // Without `--series` the short-term loudness series is left out.
        let run = sandbox.run(&["analyze", render]);
        let mut analysis = report.to_json(true);
        analysis
            .as_object_mut()
            .unwrap()
            .shift_remove("short_term_lufs");
        assert_eq!(run.json["analysis"], analysis, "{name}");
    }
}

#[test]
fn analyze_fails_on_files_it_cannot_read() {
    let sandbox = Sandbox::new();
    fs::write(sandbox.root().join("noise.wav"), b"not audio").unwrap();
    for file in ["noise.wav", "missing.wav"] {
        let run = sandbox.run(&["analyze", file]);
        assert_eq!(run.code, 1, "{file}");
        assert_eq!(run.stdout, "", "{file}");
        assert!(!run.stderr.is_empty(), "{file}");
    }
}

#[test]
fn info_of_audio_files_equals_the_golden_inspection() {
    let golden = Golden::open().unwrap();
    let recorded: Value = golden.json("media/info.json").unwrap();
    let sandbox = Sandbox::new();
    let root = sandbox.root().to_str().unwrap().to_owned();
    fs::create_dir(sandbox.root().join("fixtures")).unwrap();
    for name in [
        "chord-48k-stereo.flac",
        "ir-48k-stereo-float.wav",
        "saw-44k1-stereo.mp3",
        "tone-44k1-mono-16bit.wav",
        "tone-48k-stereo-24bit.wav",
        "not-media.wav",
    ] {
        let rel = format!("fixtures/{name}");
        fs::write(sandbox.root().join(&rel), golden.bytes(&rel).unwrap()).unwrap();
        let path = sandbox.root().join(&rel);
        let run = sandbox.run(&["info", path.to_str().unwrap()]);
        let entry = &recorded[&rel];
        if let Some(error) = entry.get("inspect_error") {
            // `SonaraError` from `inspect_audio`, caught by `main`.
            assert_eq!(run.code, 1, "{name}");
            assert_eq!(
                normalized(&run.json, &root),
                json!({"ok": false, "error": error}),
                "{name}"
            );
        } else {
            assert_eq!(run.code, 0, "{name}: {}", run.stderr);
            assert_eq!(normalized(&run.json, &root), entry["inspect"], "{name}");
        }
    }
}

#[test]
fn info_and_verify_of_a_project() {
    let sandbox = Sandbox::new();
    let target = sandbox.root().join("song");
    let created = sandbox.run(&[
        "tool",
        "create_project",
        "-a",
        "name=Info",
        "-a",
        &format!("path={}", target.display()),
        "--connect",
        "never",
    ]);
    assert_eq!(created.code, 0, "{}", created.stdout);
    let path = created.json["path"].as_str().unwrap().to_owned();
    let run = sandbox.run(&["info", &path]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.json["layout"], "bundle");
    assert_eq!(run.json["warnings"], json!([]));
    assert_eq!(run.json["state"], created.json["state"]);
    for quick in [false, true] {
        let args: &[&str] = if quick {
            &["verify", &path, "--quick"]
        } else {
            &["verify", &path]
        };
        let run = sandbox.run(args);
        assert_eq!(run.code, 0, "{}", run.stdout);
        assert_eq!(run.json["ok"], true);
        let keys: Vec<&String> = run.json.as_object().unwrap().keys().collect();
        assert_eq!(keys.first().map(|k| k.as_str()), Some("warnings"));
    }
    // A missing project: `SonaraError` caught by `main`.
    let run = sandbox.run(&["verify", "missing.sonara"]);
    assert_eq!(run.code, 1);
    assert_eq!(run.json["ok"], false);
    assert_eq!(run.json["error"]["code"], "NOT_FOUND");
}

/// `sonara devices` lists the system context's devices in the prototype's shape: the backend
/// as `native/<name>`, playback devices before capture devices, each with its saved id (hex),
/// name, kind, default flag, channels and the always empty `sample_rates`.
///
/// On the macOS host the output was byte-identical to the prototype's `sonara devices`
/// (2026-10-22: Core Audio, three playback and two capture devices). Setting
/// `SONARA_PROTOTYPE_DEVICES` to a file with the prototype's output repeats that comparison.
#[test]
fn devices_lists_the_native_backend() {
    let run = Sandbox::new().run(&["devices"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let listing = run.json.as_object().unwrap();
    assert_eq!(listing.keys().collect::<Vec<_>>(), ["backend", "devices"]);
    assert!(run.json["backend"].as_str().unwrap().starts_with("native/"));
    let devices = run.json["devices"].as_array().unwrap();
    let kinds: Vec<&str> = devices
        .iter()
        .map(|d| d["kind"].as_str().unwrap())
        .collect();
    let playback = kinds.iter().take_while(|k| **k == "playback").count();
    assert!(
        kinds[playback..].iter().all(|k| *k == "capture"),
        "{kinds:?}"
    );
    for device in devices {
        assert_eq!(
            device.as_object().unwrap().keys().collect::<Vec<_>>(),
            ["id", "name", "kind", "default", "channels", "sample_rates"]
        );
        let id = device["id"].as_str().unwrap();
        assert!(
            id.len() % 2 == 0
                && id
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
        assert_eq!(device["sample_rates"], json!([]));
    }
    if let Some(path) = std::env::var_os("SONARA_PROTOTYPE_DEVICES") {
        let prototype = fs::read_to_string(path).unwrap();
        assert_eq!(
            run.stdout, prototype,
            "the prototype's output, byte for byte"
        );
    }
}

#[test]
fn commands_that_cannot_run_fail_with_a_reason() {
    // `gui` without the desktop application next to `sonara` is in `gui.rs`: the built
    // `sonara-desktop` may sit next to this binary.
    let sandbox = Sandbox::new();
    // No session runs in the sandbox.
    let run = sandbox.run(&["mcp", "--connect", "always"]);
    assert_eq!(run.code, 1);
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr.contains("no running Sonara session found"),
        "{}",
        run.stderr
    );
}

#[test]
fn usage_errors_and_log_levels() {
    let sandbox = Sandbox::new();
    assert_eq!(sandbox.run(&[]).code, 2);
    assert_eq!(sandbox.run(&["help"]).code, 2);
    assert_eq!(sandbox.run(&["render", "p.sonara"]).code, 2);
    assert_eq!(sandbox.run(&["mcp", "--audio", "loud"]).code, 2);
    let run = sandbox.run(&["--log-level", "loud", "tools"]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr.contains("Unknown level: 'LOUD'"),
        "{}",
        run.stderr
    );
    assert_eq!(sandbox.run(&["--log-level", "debug", "tools"]).code, 0);
}
