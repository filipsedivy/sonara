//! Golden parity of the offline renderer (GLD-01, steps
//! 8 to 10).
//!
//! `golden/workflows/<name>/render.wav` and `golden/coverage/render.wav` are the prototype's
//! `OfflineRenderer.render(final, RenderRequest())` of each batch's final project
//! (`scripts/golden/area_workflows.py`, `area_coverage.py`), written as float32. Each test
//! renders `final.project.json` with the defaults and checks:
//!
//! - every recorded statistic of `transcript.json → render` exactly: sample rate, frame count
//!   (which depends on the auto tail's silence threshold), start frame, latency, tail frames
//!   and warnings;
//! - the audio, rounded to float32, by its null-test residual against the recording.
//!
//! The coverage batch's media resolve as the recorder's sandbox saw them: every asset is a byte
//! copy of a file in `golden/fixtures` with the asset's name (embedded assets were copied into
//! the bundle at import, the linked one was moved by a harness step), checked by its SHA-256.
//! The decode cache lives in a temporary directory, so the 44.1 kHz and the compressed media go
//! through decoding and libsoxr as in the prototype.
//!
//! **Residuals** (GLD-01). Every batch passes the shared −120 dBFS null-test threshold,
//! so none has a tolerance of its own. Measured in the development container (aarch64 Linux,
//! glibc) on 2026-10-21, with every statistic exact:
//!
//! | Batch | Residual | Cause |
//! |---|---|---|
//! | `sidechain_preview` | bit-exact | — |
//! | `mix_analysis` | −168.6 dBFS | one float32 rounding flip from a libm ulp (F1: glibc, not macOS libm) |
//! | `cinematic_orchestral` | −138.5 dBFS | the reverb's float32 wet path |
//! | `halloween_thirteenth_hour` | −136.5 dBFS | the same |
//! | `electronic_124bpm` | −135.0 dBFS | the same; −150.5 dBFS with the reverb bypassed in both |
//! | `coverage` | −125.5 dBFS | all inside the MP3 clip (symphonia against mpg123); −307 dBFS elsewhere |
//!
//! In `coverage`, the resampled 44.1 kHz WAV (libsoxr), the FLAC clips and the convolver with
//! its impulse response are bit-exact. On macOS, whose libm the oracle used, the libm part
//! is expected to vanish; that is not measured here.
//!
//! [`the_processing_path_allocates_nothing`] drives every graph block by block inside
//! `assert_no_alloc` (RT-02); the test binary installs `AllocDisabler`, which aborts on an
//! allocation inside the closure.
//!
//! Run with `--nocapture` to see each render's residual and timing. In a debug build the three
//! long workflows (2 to 4 minutes of audio, reverbs) take 3.5 to 4.5 minutes each; the tests run
//! in parallel.

use std::path::PathBuf;
use std::time::Instant;

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use serde_json::Value;
use sonara_core::model::project::Project;
use sonara_core::plugins::builtin_registry;
use sonara_engine::{GraphOptions, OfflineRenderer, RenderGraph, RenderRequest, range_frames};
use sonara_media::MediaProvider;
use sonara_project::serializer::{loads, project_from_value};
use sonara_testkit::{Golden, peak_residual_dbfs, sha256_hex};
use tempfile::TempDir;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

/// The golden null-test threshold: the peak residual of a render, in dBFS.
const NULL_TEST_DBFS: f64 = -120.0;

/// The recorded batches with a render.
const BATCHES: [&str; 6] = [
    "workflows/cinematic_orchestral",
    "workflows/electronic_124bpm",
    "workflows/halloween_thirteenth_hour",
    "workflows/mix_analysis",
    "workflows/sidechain_preview",
    "coverage",
];

/// A batch's final project and the media provider of its sandbox.
struct Batch {
    golden: Golden,
    project: Project,
    media: MediaProvider,
    _cache: TempDir,
}

impl Batch {
    fn open(dir: &str) -> Self {
        let golden = Golden::open().expect("golden data is available (set SONARA_GOLDEN)");
        let raw = golden
            .bytes(&format!("{dir}/final.project.json"))
            .expect("the final project");
        let project = project_from_value(&loads(&raw).expect("project JSON"))
            .expect("the final project loads");
        let fixtures = golden.root().join("fixtures");
        for asset in &project.assets {
            let bytes = golden
                .bytes(&format!("fixtures/{}", asset.name))
                .unwrap_or_else(|e| panic!("asset {} has no fixture: {e}", asset.id));
            if let Some(sha) = &asset.sha256 {
                assert_eq!(&sha256_hex(&bytes), sha, "fixture of asset {}", asset.id);
            }
        }
        let cache = TempDir::new().expect("a cache directory");
        let media = MediaProvider::new(
            move |asset| -> PathBuf { fixtures.join(&asset.name) },
            Some(cache.path().to_owned()),
        );
        Self {
            golden,
            project,
            media,
            _cache: cache,
        }
    }
}

fn int(value: &Value) -> u64 {
    value
        .as_u64()
        .unwrap_or_else(|| panic!("{value} is not an integer"))
}

/// Renders one batch and compares it; returns the residual in dBFS.
fn check_render(dir: &str, max_residual_dbfs: f64) -> f64 {
    let batch = Batch::open(dir);
    let transcript: Value = batch
        .golden
        .json(&format!("{dir}/transcript.json"))
        .expect("the transcript");
    let recorded = &transcript["render"];
    let registry = builtin_registry();
    let started = Instant::now();
    let rendered = OfflineRenderer::new(&registry, &batch.media)
        .render(&batch.project, &RenderRequest::default(), None)
        .unwrap_or_else(|e| panic!("{dir}: render failed: {e}"));
    let elapsed = started.elapsed().as_secs_f64();
    let stats = &rendered.stats;
    let warnings: Vec<String> = recorded["warnings"]
        .as_array()
        .expect("a warning list")
        .iter()
        .map(|w| w.as_str().expect("a warning").to_owned())
        .collect();
    assert_eq!(
        u64::from(rendered.sample_rate),
        int(&recorded["sample_rate"]),
        "{dir}: sample rate"
    );
    assert_eq!(
        rendered.start_frame,
        i64::try_from(int(&recorded["start_frame"])).expect("a frame"),
        "{dir}: start frame"
    );
    assert_eq!(
        stats.latency as u64,
        int(&recorded["latency"]),
        "{dir}: latency"
    );
    assert_eq!(
        stats.tail_frames as u64,
        int(&recorded["tail_frames"]),
        "{dir}: tail frames"
    );
    assert_eq!(
        stats.frames as u64,
        int(&recorded["frames"]),
        "{dir}: frames"
    );
    assert_eq!(
        rendered.data.len() as u64,
        int(&recorded["frames"]),
        "{dir}: data frames"
    );
    assert_eq!(stats.warnings, warnings, "{dir}: warnings");
    assert!(
        stats.missing_media.is_empty(),
        "{dir}: missing media {:?}",
        stats.missing_media
    );
    let expected = batch
        .golden
        .wav(&format!("{dir}/render.wav"))
        .expect("the render");
    assert_eq!(expected.channels, 2, "{dir}: channels");
    assert_eq!(
        expected.sample_rate, rendered.sample_rate,
        "{dir}: WAV rate"
    );
    #[expect(clippy::cast_possible_truncation, reason = "the recording is float32")]
    let actual: Vec<f32> = rendered.data.iter().flatten().map(|&x| x as f32).collect();
    let residual = peak_residual_dbfs(&expected.samples, &actual);
    println!(
        "{dir}: {} frames ({:.1} s audio) in {elapsed:.1} s, residual {residual:.1} dBFS",
        stats.frames,
        rendered.seconds()
    );
    assert!(
        residual <= max_residual_dbfs,
        "{dir}: residual {residual:.1} dBFS above {max_residual_dbfs} dBFS"
    );
    residual
}

#[test]
fn cinematic_orchestral() {
    check_render("workflows/cinematic_orchestral", NULL_TEST_DBFS);
}

#[test]
fn electronic_124bpm() {
    check_render("workflows/electronic_124bpm", NULL_TEST_DBFS);
}

#[test]
fn halloween_thirteenth_hour() {
    check_render("workflows/halloween_thirteenth_hour", NULL_TEST_DBFS);
}

#[test]
fn mix_analysis() {
    check_render("workflows/mix_analysis", NULL_TEST_DBFS);
}

#[test]
fn sidechain_preview() {
    check_render("workflows/sidechain_preview", NULL_TEST_DBFS);
}

#[test]
fn coverage() {
    check_render("coverage", NULL_TEST_DBFS);
}

/// RT-02: every block of every batch's graph, from several start positions (each one chases),
/// runs inside `assert_no_alloc`; the clip audio is read outside it, as the renderer does.
#[test]
fn the_processing_path_allocates_nothing() {
    const BLOCKS: usize = 64;
    let registry = builtin_registry();
    for dir in BATCHES {
        let batch = Batch::open(dir);
        let (start, end) =
            range_frames(&batch.project, &RenderRequest::default()).expect("a range");
        let request = RenderRequest::default();
        let mut graph = RenderGraph::new(
            &batch.project,
            &registry,
            &batch.media,
            request.block_size,
            &GraphOptions::default(),
        )
        .expect("the graph builds");
        graph.reset();
        let block = i64::try_from(graph.block_size()).unwrap();
        for quarter in 0..4 {
            let mut frame = start + (end - start) * quarter / 4;
            for _ in 0..BLOCKS {
                graph.read_media(frame).expect("media reads");
                let peak = assert_no_alloc(|| {
                    graph
                        .process(frame, true)
                        .iter()
                        .flatten()
                        .fold(0.0_f64, |peak, x| peak.max(x.abs()))
                });
                assert!(peak.is_finite(), "{dir}: frame {frame}");
                frame += block;
            }
        }
        assert_eq!(graph.blocks_without_media(), 0, "{dir}");
    }
}
