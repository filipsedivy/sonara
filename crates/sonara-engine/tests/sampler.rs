//! `sonara.sampler` inside an offline render (ADR 0010): the instrument of a track is an SFZ
//! patch named by the device's `instrument` parameter. The library is generated in a tempdir; nothing licensed is
//! needed. These checks prove the integration (patch loaded, notes reach the sampler, output is
//! audible and does not clip); they say nothing about how a real library sounds.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sonara_core::plugins::{builtin_registry, sampler_descriptor};
use sonara_engine::{OfflineRenderer, RenderRequest, RenderedAudio, Tail};
use sonara_media::MediaProvider;
use sonara_project::serializer::{loads, project_from_value};
use sonara_testkit::Golden;

const KICK: &str = "trk_000000000003";

#[expect(
    clippy::cast_possible_truncation,
    reason = "the amplitude 16000 fits an i16"
)]
/// A 16-bit mono WAV of a 110 Hz sine, one second long at 44.1 kHz.
fn write_wav(path: &Path) {
    let frames = 44_100_u32;
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
        let x = (f64::from(i) / 44_100.0 * 110.0 * std::f64::consts::TAU).sin();
        let value = (x * 16_000.0).round() as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(path, bytes).expect("write the wav");
}

fn patch_dir() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a tempdir");
    write_wav(&dir.path().join("tone.wav"));
    let sfz = dir.path().join("tone.sfz");
    fs::write(
        &sfz,
        "<region> sample=tone.wav lokey=36 hikey=36 pitch_keycenter=36 ampeg_release=0.1\n",
    )
    .expect("write the patch");
    (dir, sfz)
}

/// The base project with the kick track's drum kit replaced by the sampler, which offers the
/// installed `patch` and plays `instrument` (`None`: the default, the first installed one).
fn render(
    installed: &[String],
    instrument: Option<&str>,
) -> Result<RenderedAudio, sonara_engine::EngineError> {
    let golden = Golden::open().expect("golden data is available (set SONARA_GOLDEN)");
    let bytes = golden
        .bytes("workflows/sidechain_preview/final.project.json")
        .expect("the base project");
    let mut data = loads(&bytes).expect("project JSON");
    let track = data
        .get_mut("tracks")
        .and_then(Value::as_array_mut)
        .and_then(|tracks| tracks.iter_mut().find(|t| t["id"] == KICK))
        .expect("the kick track");
    let params = instrument.map_or_else(|| json!({}), |name| json!({"instrument": name}));
    track["instrument"] = json!({"id": "dev_000000000004", "plugin_id": "sonara.sampler",
        "name": "Sonara Sampler", "enabled": true, "params": params, "sidechain_id": null,
        "preset": null, "state": {}});
    let project = project_from_value(&data).expect("the edited project loads");
    let mut registry = builtin_registry();
    if let Some(descriptor) = sampler_descriptor(installed) {
        registry.register(descriptor).expect("registers once");
    }
    let media = MediaProvider::new(|_| PathBuf::from("/nonexistent/sonara-media"), None);
    OfflineRenderer::new(&registry, &media).render(
        &project,
        &RenderRequest {
            end_beat: Some(2.0),
            tail: Tail::None,
            ..RenderRequest::default()
        },
        None,
    )
}

/// The loudest absolute sample.
fn peak(audio: &RenderedAudio) -> f64 {
    audio
        .data
        .iter()
        .flatten()
        .fold(0.0, |p, v| f64::max(p, v.abs()))
}

fn path(sfz: &Path) -> String {
    sfz.to_str().expect("a UTF-8 path").to_owned()
}

#[test]
fn a_sampler_track_renders_its_patch() {
    let (_dir, sfz) = patch_dir();
    let audio = render(&[path(&sfz)], None).expect("renders");
    assert!(
        audio.stats.warnings.is_empty(),
        "{:?}",
        audio.stats.warnings
    );
    assert_eq!(audio.sample_rate, 48_000);
    // Two beats at 124 bpm is about 0.97 s; the sampler adds the kick track's note at beat 0.
    assert!(
        audio.seconds() > 0.9 && audio.seconds() < 1.1,
        "{}",
        audio.seconds()
    );
    let drums = render_drums();
    assert_ne!(audio.data, drums.data, "the sampler is not the drum kit");
    assert!(
        peak(&audio) > 0.01,
        "the patch is audible: {}",
        peak(&audio)
    );
    assert!(peak(&audio) <= 1.0, "no clipping: {}", peak(&audio));
}

/// The same project with no instrument tone at all: the base project's own render.
fn render_drums() -> RenderedAudio {
    let golden = Golden::open().expect("golden data");
    let bytes = golden
        .bytes("workflows/sidechain_preview/final.project.json")
        .expect("the base project");
    let project = project_from_value(&loads(&bytes).expect("project JSON")).expect("loads");
    let media = MediaProvider::new(|_| PathBuf::from("/nonexistent/sonara-media"), None);
    OfflineRenderer::new(&builtin_registry(), &media)
        .render(
            &project,
            &RenderRequest {
                end_beat: Some(2.0),
                tail: Tail::None,
                ..RenderRequest::default()
            },
            None,
        )
        .expect("renders")
}

#[test]
fn a_missing_patch_warns_and_plays_silence() {
    let (_dir, sfz) = patch_dir();
    let gone = path(&sfz.with_file_name("gone.sfz"));
    let audio = render(&[gone], None).expect("renders");
    assert!(
        audio
            .stats
            .warnings
            .iter()
            .any(|w| w.contains("cannot be loaded")),
        "{:?}",
        audio.stats.warnings
    );
    let (_other, working) = patch_dir();
    let heard = render(&[path(&working)], None).expect("renders");
    assert!(
        peak(&heard) > peak(&audio),
        "the missing patch adds nothing"
    );
}

#[test]
fn an_instrument_that_is_not_installed_warns_instead_of_failing() {
    let (_dir, sfz) = patch_dir();
    let audio = render(&[path(&sfz)], Some("/elsewhere/Violin.sfz")).expect("renders");
    assert!(
        audio
            .stats
            .warnings
            .iter()
            .any(|w| w.contains("is not installed")),
        "{:?}",
        audio.stats.warnings
    );
}

#[test]
fn an_unregistered_sampler_is_bypassed_with_a_warning() {
    let audio = render(&[], None).expect("renders");
    assert!(
        audio
            .stats
            .warnings
            .iter()
            .any(|w| w.contains("unavailable; bypassed")),
        "{:?}",
        audio.stats.warnings
    );
}
