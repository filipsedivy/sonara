//! The GUI's computed data: the waveform peak pyramids and the parametric
//! EQ's response curve, against oracles recorded from the prototype.
//!
//! **Oracles.** `tests/fixtures/peaks_level0.json` and `tests/fixtures/eq_response.json` were
//! written by the prototype, on the macOS host, with
//!
//! ```text
//! .venv/bin/python -I scripts/golden/gui_oracles.py
//! ```
//!
//! The script runs the prototype's own functions on the inputs of its GUI tests (GLD-01): for
//! the peaks, `test_waveform_cache_computes_off_thread_and_persists`'s tone written as a WAV by
//! `soundfile`, imported into a new project and reduced by `compute_level0` and `build_levels`
//! through the media provider; for the EQ, `eq_sections` and `EqCurve.set_params` with the
//! parameters of `test_eq_sections_match_processor` and `test_eq_curve_reflects_gain`.
//!
//! These prototype tests are covered here:
//! `waveform_cache_computes_off_thread_and_persists`, `eq_sections_match_processor` (with
//! `sonara-dsp`'s `the_public_design_is_the_processors_own_cascade`) and `eq_curve_reflects_gain`
//! (the response; the drawing is the page's).

use std::fs;
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use serde_json::Value;
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::edit::{self, Edit};
use sonara_app::eq_response::{self, EqResponse};
use sonara_app::media::AudioImport;
use sonara_app::peaks::{
    BIN_FRAMES, PeakPyramid, PeaksReady, PeaksState, TILE_HEADER, cache_file, encode_level0,
};
use sonara_app::projects::NewProject;
use sonara_app::settings::Settings;
use sonara_core::model::media::MediaAsset;
use sonara_core::plugins::builtin_registry;
use sonara_dsp::processors::{param_slots, parametric_eq_design};
use sonara_testkit::sha256_hex;
use tempfile::TempDir;

const SR: u32 = 48_000;

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    serde_json::from_str(&fs::read_to_string(&path).expect("the fixture")).expect("JSON")
}

struct App {
    app: AppContext,
    dir: TempDir,
}

impl App {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let app = AppContext::create(AppOptions {
            workspaces: Some(dir.path().join("ws")),
            settings: Some(Settings::default()),
            persist_settings: false,
            audio_backend: Some("null".to_owned()),
            ..AppOptions::default()
        })
        .expect("the application");
        app.projects()
            .new_project(&NewProject {
                name: "GUI".to_owned(),
                ..NewProject::default()
            })
            .expect("a project");
        Self { app, dir }
    }

    fn import(&self, path: &Path) -> MediaAsset {
        let result = self
            .app
            .media()
            .import_audio(
                path,
                &AudioImport {
                    create_clip: true,
                    ..AudioImport::default()
                },
            )
            .expect("the import");
        let id = result["asset_id"].as_str().expect("an asset");
        self.app
            .session()
            .project()
            .expect("a project")
            .asset(id)
            .expect("the asset")
            .clone()
    }

    fn workspace(&self) -> std::path::PathBuf {
        self.app
            .session()
            .lock()
            .location()
            .expect("a location")
            .workspace
            .clone()
    }
}

/// The signal of `test_waveform_cache_computes_off_thread_and_persists`, as NumPy computes it:
/// `t = arange(sr · 3) / sr`, `s = where(t < 1.5, 0.5, 0.1) · sin(2π · 110 · t)`, stereo
/// `[s, −s]` as float32, interleaved.
fn tone() -> Vec<f32> {
    let mut samples = Vec::with_capacity(2 * 3 * SR as usize);
    for n in 0..3 * SR {
        let t = f64::from(n) / f64::from(SR);
        let s = if t < 1.5 { 0.5 } else { 0.1 } * (2.0 * std::f64::consts::PI * 110.0 * t).sin();
        #[expect(clippy::cast_possible_truncation, reason = "astype(float32)")]
        samples.extend([s as f32, (-s) as f32]);
    }
    samples
}

/// libsndfile 1.2.2's float-to-16-bit conversion as `soundfile` wrote the oracle's file: the
/// float scaled to 32 bits with `lrint` (ties to even), then shifted right by 16 (checked on
/// every sample of the fixture's signal against the file's samples, `pcm16_sha256`).
fn pcm16(sample: f32) -> i16 {
    let wide = (f64::from(sample) * 2_147_483_648.0)
        .round_ties_even()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX));
    #[expect(clippy::cast_possible_truncation, reason = "clamped to the i32 range")]
    let wide = wide as i32;
    i16::try_from(wide >> 16).expect("an i32 shifted by 16 fits i16")
}

fn wait_ready(ready: &Receiver<PeaksReady>, asset: &str) -> PeaksReady {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        let left = end.saturating_duration_since(Instant::now());
        let notice = ready.recv_timeout(left).expect("the peaks are computed");
        if notice.asset_id == asset {
            return notice;
        }
    }
}

fn level(value: &Value) -> Vec<[f32; 2]> {
    value
        .as_array()
        .expect("bins")
        .iter()
        .map(|bin| {
            let pair = bin.as_array().expect("a bin");
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the fixture holds float32 values exactly"
            )]
            let value = |i: usize| pair[i].as_f64().expect("a number") as f32;
            [value(0), value(1)]
        })
        .collect()
}

fn bits(levels: &[Vec<[f32; 2]>]) -> Vec<Vec<[u32; 2]>> {
    levels
        .iter()
        .map(|l| l.iter().map(|[a, b]| [a.to_bits(), b.to_bits()]).collect())
        .collect()
}

#[test]
fn peaks_equal_the_prototypes_compute_level0() {
    let oracle = fixture("peaks_level0.json");
    let samples = tone();
    let float_bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    assert_eq!(
        sha256_hex(&float_bytes),
        oracle["samples_sha256"],
        "the regenerated signal is the prototype's"
    );
    let pcm: Vec<i16> = samples.iter().copied().map(pcm16).collect();
    let pcm_bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
    assert_eq!(
        sha256_hex(&pcm_bytes),
        oracle["pcm16_sha256"],
        "the file holds the prototype's samples"
    );

    let app = App::new();
    let path = app.dir.path().join("tone.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec).expect("a writer");
    for sample in &pcm {
        writer.write_sample(*sample).expect("a sample");
    }
    writer.finalize().expect("the WAV");
    let asset = app.import(&path);
    let ready = app.app.peaks().subscribe();

    // Never computed on the caller's thread: the first request schedules it.
    let asked = Instant::now();
    assert_eq!(app.app.peaks().request(&asset, SR), PeaksState::Pending);
    assert!(
        asked.elapsed() < Duration::from_millis(50),
        "{:?}",
        asked.elapsed()
    );
    assert_eq!(app.app.peaks().request(&asset, SR), PeaksState::Pending);
    let notice = wait_ready(&ready, &asset.id);
    assert_eq!((notice.state, notice.sample_rate), ("ready", SR));
    let PeaksState::Ready(pyramid) = app.app.peaks().request(&asset, SR) else {
        panic!("the pyramid is ready");
    };
    assert!(!pyramid.missing);
    assert_eq!(pyramid.frames, oracle["frames"].as_u64().unwrap());
    assert_eq!(pyramid.frames.div_ceil(BIN_FRAMES), 563);
    let expected: Vec<Vec<[f32; 2]>> = oracle["levels"]
        .as_array()
        .unwrap()
        .iter()
        .map(level)
        .collect();
    assert_eq!(pyramid.levels.len(), 3);
    assert_eq!(
        bits(&pyramid.levels),
        bits(&expected),
        "bit-equal to the oracle"
    );

    // The test's own checks: loud first half, quiet second half, and the envelope's bins at
    // 1000 frames per pixel (level 0) around frame 0 and frame 96 000.
    let level0 = &pyramid.levels[0];
    let third = level0.len() / 3;
    let max = |bins: &[[f32; 2]]| bins.iter().map(|b| b[1]).fold(f32::MIN, f32::max);
    assert!((max(&level0[..third]) - 0.5).abs() < 0.01);
    assert!((max(&level0[level0.len() - third..]) - 0.1).abs() < 0.01);
    assert!(max(&level0[0..3]) > 0.4);
    assert!(max(&level0[375..378]) < 0.15);

    // Persisted under the workspace's cache, and served as tiles.
    let file = cache_file(&app.workspace(), &asset, SR);
    assert!(file.is_file(), "{}", file.display());
    assert!(
        file.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(&format!("{}-", asset.id))
    );
    let tile = app.app.peaks().tile(&asset.id, SR, 0, 0).expect("tile 0");
    assert_eq!(tile.len(), TILE_HEADER + 563 * 8);
    assert_eq!(app.app.peaks().tile(&asset.id, SR, 0, 1), None);
    assert_eq!(app.app.peaks().tile(&asset.id, SR, 3, 0), None);
    assert_eq!(app.app.peaks().tile("med_nope", SR, 0, 0), None);
}

/// A valid cache file is read instead of the media; one for other media is ignored.
#[test]
fn a_persisted_pyramid_is_loaded_instead_of_recomputed() {
    let app = App::new();
    let path = app.dir.path().join("short.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SR,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec).expect("a writer");
    for i in 0..1000_i16 {
        writer.write_sample(i * 16).expect("a sample");
    }
    writer.finalize().expect("the WAV");
    let asset = app.import(&path);
    let ready = app.app.peaks().subscribe();
    app.app.peaks().request(&asset, SR);
    wait_ready(&ready, &asset.id);
    let computed = app.app.peaks().ready(&asset.id, SR).expect("ready");
    assert_eq!(computed.levels[0].len(), 4, "1000 frames in bins of 256");

    // Mark the file, forget the pyramid, and ask again: the marked bins come back.
    let file = cache_file(&app.workspace(), &asset, SR);
    let mut marked = computed.levels[0].clone();
    marked[0] = [-0.75, 0.75];
    fs::write(&file, encode_level0(SR, computed.frames, &marked)).expect("the marked file");
    app.app.peaks().clear();
    assert_eq!(app.app.peaks().request(&asset, SR), PeaksState::Pending);
    wait_ready(&ready, &asset.id);
    let loaded = app.app.peaks().ready(&asset.id, SR).expect("ready");
    assert_eq!(loaded.levels[0][0], [-0.75, 0.75]);

    // A file of another rate is not this pyramid's: it is computed again.
    fs::write(&file, encode_level0(44_100, computed.frames, &marked)).expect("the file");
    app.app.peaks().clear();
    app.app.peaks().request(&asset, SR);
    wait_ready(&ready, &asset.id);
    let recomputed = app.app.peaks().ready(&asset.id, SR).expect("ready");
    assert_eq!(recomputed.levels, computed.levels);
}

/// Missing media gives a silent pyramid flagged missing ("media missing"), never persisted.
#[test]
fn missing_media_gives_a_missing_pyramid() {
    let app = App::new();
    let path = app.dir.path().join("gone.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec).expect("a writer");
    for _ in 0..600 {
        writer.write_sample(1000_i16).expect("a sample");
    }
    writer.finalize().expect("the WAV");
    let asset = app.import(&path);
    let provider = app.app.media().provider();
    fs::remove_file(provider.path_for(&asset)).expect("the copy is removed");
    provider.forget(&asset.id);
    let ready = app.app.peaks().subscribe();
    app.app.peaks().request(&asset, SR);
    wait_ready(&ready, &asset.id);
    let pyramid = app.app.peaks().ready(&asset.id, SR).expect("ready");
    assert!(pyramid.missing);
    assert_eq!(pyramid.levels[0], vec![[0.0, 0.0]; 2]);
    assert!(!cache_file(&app.workspace(), &asset, SR).exists());
    assert_eq!(PeakPyramid::bin_frames(2), 4096);
}

// -- the EQ curve ------------------------------------------------------------------------------

fn close(actual: f64, expected: f64, tolerance: f64) -> bool {
    (actual - expected).abs() <= tolerance * expected.abs().max(1.0)
}

/// The design and the curve equal the prototype's `eq_sections` and `EqCurve` for the
/// parameters of its tests.
///
/// Tolerances: the sections come from `sin`, `cos` and `pow`, which are exact only where the
/// platform's libm agrees with macOS's (`biquad`), so they
/// are compared to 1e-12 relative; the curve evaluates `sosfreqz` with complex arithmetic in
/// another order than `SciPy`'s, so it is compared to 1e-9 dB.
#[test]
fn the_eq_curve_equals_the_prototypes() {
    let oracle = fixture("eq_response.json");
    let registry = builtin_registry();
    let descriptor = registry.get(eq_response::PLUGIN_ID).expect("the EQ");
    for case in oracle["cases"].as_array().expect("cases") {
        let values = case["params"].as_object().expect("params").clone();
        let params = descriptor
            .validate_params(&values)
            .expect("valid parameters");
        let rate = u32::try_from(case["sample_rate"].as_u64().unwrap()).unwrap();
        let slots = param_slots(descriptor, &params).expect("slots");
        let design = parametric_eq_design(&slots, f64::from(rate)).expect("a design");
        let sections = case["sections"].as_array().unwrap();
        assert_eq!(design.sections().len(), sections.len(), "{values:?}");
        for (ours, theirs) in design.sections().iter().zip(sections) {
            for (a, b) in ours.iter().zip(theirs.as_array().unwrap()) {
                assert!(
                    close(*a, b.as_f64().unwrap(), 1e-12),
                    "{values:?}: {a} vs {b}"
                );
            }
        }
        let curve = eq_response::response(&registry, &params, rate).expect("a curve");
        let freqs = case["freqs"].as_array().unwrap();
        let db = case["db"].as_array().unwrap();
        assert_eq!(curve.freqs.len(), freqs.len());
        for (i, (f, d)) in curve.freqs.iter().zip(&curve.db).enumerate() {
            let (f0, d0) = (freqs[i].as_f64().unwrap(), db[i].as_f64().unwrap());
            assert!(close(*f, f0, 1e-13), "point {i}: {f} Hz vs {f0} Hz");
            assert!(
                (d - d0).abs() < 1e-9,
                "{values:?} at {f} Hz: {d} vs {d0} dB"
            );
        }
    }
}

/// `EqCurve.response_at`: the curve interpolated on a log-frequency axis.
fn response_at(curve: &EqResponse, freq: f64) -> f64 {
    let x = freq.ln();
    let points: Vec<(f64, f64)> = curve
        .freqs
        .iter()
        .map(|f| f.ln())
        .zip(curve.db.iter().copied())
        .collect();
    let i = points
        .partition_point(|(f, _)| *f <= x)
        .clamp(1, points.len() - 1);
    let ((x0, y0), (x1, y1)) = (points[i - 1], points[i]);
    y0 + (y1 - y0) * (x - x0) / (x1 - x0)
}

/// `test_eq_curve_reflects_gain`: a mid band at 1 kHz shows in the curve, and the output gain
/// moves all of it.
#[test]
fn the_eq_curve_reflects_gain() {
    let app = App::new();
    let run = |edit: Value| {
        edit::apply(&app.app, Edit::from_json(edit).expect("an edit")).expect("the edit")
    };
    let track = run(serde_json::json!({"op": "create_track", "name": "Vox", "kind": "audio"}));
    let track = track.result["track_id"].as_str().unwrap().to_owned();
    let eq = run(serde_json::json!({"op": "add_device", "track_id": track,
                                    "plugin_id": eq_response::PLUGIN_ID}));
    let eq = eq.result["device_id"].as_str().unwrap().to_owned();
    let curve = || {
        let project = app.app.session().project().unwrap();
        eq_response::device_response(&project, app.app.plugins(), &eq).expect("a curve")
    };
    assert!(response_at(&curve(), 1000.0).abs() < 0.05);
    run(
        serde_json::json!({"op": "set_device_params", "device_id": eq,
                           "values": {"mid1_freq": 1000.0, "mid1_gain": 6.0, "mid1_q": 1.0}}),
    );
    let boosted = curve();
    assert!((response_at(&boosted, 1000.0) - 6.0).abs() < 0.3);
    let mid1 = &boosted.bands[1];
    assert_eq!((mid1.freq, mid1.gain_db, mid1.on), (1000.0, 6.0, true));
    run(
        serde_json::json!({"op": "set_device_params", "device_id": eq,
                           "values": {"output_db": -3.0}}),
    );
    assert!((response_at(&curve(), 1000.0) - 3.0).abs() < 0.3);

    let project = app.app.session().project().unwrap();
    let synth = run(serde_json::json!({"op": "add_device", "track_id": track,
                                       "plugin_id": "sonara.dynamics.compressor"}));
    let synth = synth.result["device_id"].as_str().unwrap().to_owned();
    let project_after = app.app.session().project().unwrap();
    let not_eq = eq_response::device_response(&project_after, app.app.plugins(), &synth)
        .expect_err("not an EQ");
    assert_eq!(not_eq.code.as_str(), "INVALID_ARGUMENT");
    let unknown =
        eq_response::device_response(&project, app.app.plugins(), "dev_nope").expect_err("unknown");
    assert_eq!(unknown.code.as_str(), "NOT_FOUND");
}
