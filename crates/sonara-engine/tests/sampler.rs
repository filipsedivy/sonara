//! `sonara.sampler` inside an offline render (ADR 0010): the instrument of a track is an SFZ
//! patch named by the device's `instrument` parameter. The library is generated in a tempdir; nothing licensed is
//! needed. These checks prove the integration (patch loaded, notes reach the sampler, output is
//! audible and does not clip); they say nothing about how a real library sounds.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sonara_core::plugins::{builtin_registry, sampler_descriptor};
use sonara_engine::{OfflineRenderer, RenderRequest, RenderedAudio, Tail};
use sonara_media::{InstrumentSource, MediaProvider};
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

/// A provider of no project media.
fn no_media() -> MediaProvider {
    MediaProvider::new(|_| PathBuf::from("/nonexistent/sonara-media"), None)
}

/// The base project with the kick track's drum kit replaced by the sampler, which offers the
/// `installed` choices and plays `instrument` (`None`: the default, the first installed one).
/// Without an instrument resolver, as here, a choice is a patch path.
fn render(
    installed: &[String],
    instrument: Option<&str>,
) -> Result<RenderedAudio, sonara_engine::EngineError> {
    render_with(installed, instrument, &no_media())
}

/// [`render`] with the instruments that `media` resolves.
fn render_with(
    installed: &[String],
    instrument: Option<&str>,
    media: &MediaProvider,
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
    OfflineRenderer::new(&registry, media).render(
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

#[test]
fn a_patch_with_a_missing_sample_plays_the_rest_and_warns() {
    // The sampler leaves out only the regions whose sample is missing or a generator it does
    // not play; the rest of the patch sounds, and the warning names what was left out.
    let (dir, _) = patch_dir();
    let sfz = dir.path().join("partial.sfz");
    fs::write(
        &sfz,
        "<region> sample=tone.wav lokey=36 hikey=36 pitch_keycenter=36 ampeg_release=0.1\n\
         <region> sample=gone.wav lokey=36 hikey=36\n\
         <region> sample=*sine key=37\n",
    )
    .expect("write the patch");
    let audio = render(&[path(&sfz)], None).expect("renders");
    let expected = format!(
        "instrument {} plays without the regions it cannot load: sample {} is missing \
         (1 region left out); the sampler does not play the generator *sine (1 region left out)",
        sonara_core::pyfmt::str_repr(&path(&sfz)),
        dir.path().join("gone.wav").display()
    );
    assert_eq!(audio.stats.warnings, [expected]);
    let (_other, whole) = patch_dir();
    let heard = render(&[path(&whole)], None).expect("renders");
    assert_eq!(
        audio.data, heard.data,
        "the region that loads plays as the same patch without the others"
    );
}

#[test]
fn what_the_sampler_does_not_play_is_one_warning() {
    // Unknown opcodes, unsupported values of supported ones and skipped headers are listed
    // together; the regions still play.
    let (dir, _) = patch_dir();
    let sfz = dir.path().join("legato.sfz");
    fs::write(
        &sfz,
        "<curve> curve_index=7 v000=0\n\
         <region> sample=tone.wav lokey=36 hikey=36 pitch_keycenter=36 ampeg_release=0.1 \
         trigger=legato cutoff=500\n",
    )
    .expect("write the patch");
    let audio = render(&[path(&sfz)], None).expect("renders");
    assert_eq!(
        audio.stats.warnings,
        [format!(
            "instrument {} uses SFZ features the sampler does not play: <curve>, cutoff, \
             trigger=legato",
            sonara_core::pyfmt::str_repr(&path(&sfz))
        )]
    );
    assert!(peak(&audio) > 0.01, "the legato region plays as attack");
}

/// A provider whose resolver knows one preset, `lib/tone`, by its ID and by `old_path`, the
/// path a project from before preset IDs stored, and plays it from `patch`.
fn resolving(patch: &Path, old_path: &str) -> MediaProvider {
    let patch = patch.to_owned();
    let old_path = old_path.to_owned();
    no_media().with_instruments(move |value| {
        (value == "lib/tone" || value == old_path).then(|| InstrumentSource {
            id: "lib/tone".to_owned(),
            patch: patch.clone(),
        })
    })
}

#[test]
fn a_preset_id_plays_the_patch_its_resolver_names() {
    let (_dir, sfz) = patch_dir();
    // The oracle: the same patch named by its path, as before preset IDs.
    let by_path = render(&[path(&sfz)], None).expect("renders");
    let media = resolving(&sfz, "/elsewhere/tone.sfz");
    let installed = ["lib/tone".to_owned()];
    for instrument in [Some("lib/tone"), None] {
        let audio = render_with(&installed, instrument, &media).expect("renders");
        assert!(
            audio.stats.warnings.is_empty(),
            "{:?}",
            audio.stats.warnings
        );
        assert_eq!(audio.data, by_path.data, "{instrument:?}");
    }
    assert!(peak(&by_path) > 0.01, "the patch is audible");
}

#[test]
fn an_old_patch_path_plays_as_its_preset() {
    // A project from before preset IDs stores the patch's path. It is no longer a choice, but
    // the resolver maps it to the preset, so the project opens and plays without a warning.
    let (_dir, sfz) = patch_dir();
    let old_path = "/Users/someone/Libraries/Lib/tone.sfz";
    let media = resolving(&sfz, old_path);
    let installed = ["lib/tone".to_owned()];
    let by_id = render_with(&installed, Some("lib/tone"), &media).expect("renders");
    let by_old_path = render_with(&installed, Some(old_path), &media).expect("renders");
    assert!(
        by_old_path.stats.warnings.is_empty(),
        "{:?}",
        by_old_path.stats.warnings
    );
    assert_eq!(by_old_path.data, by_id.data);
    assert!(peak(&by_old_path) > 0.01, "the patch is audible");
}

#[test]
fn a_preset_that_is_not_installed_warns_and_plays_silence() {
    let (_dir, sfz) = patch_dir();
    let media = resolving(&sfz, "/elsewhere/tone.sfz");
    let installed = ["lib/tone".to_owned()];
    // The kick track without any instrument: the oracle for a silent sampler.
    let bypassed = render(&[], None).expect("renders");
    for unknown in ["lib/cello", "/elsewhere/cello.sfz", &path(&sfz)] {
        let audio = render_with(&installed, Some(unknown), &media).expect("renders");
        assert_eq!(
            audio.stats.warnings,
            [format!(
                "instrument {} of 'Sonara Sampler' is not installed; it plays silence",
                sonara_core::pyfmt::str_repr(unknown)
            )],
            "a resolver never falls back to reading the value as a path"
        );
        assert_eq!(audio.data, bypassed.data, "{unknown}");
    }
}

#[test]
fn a_resolved_preset_that_is_not_offered_is_not_installed() {
    // The resolver and the choices come from one snapshot in the application; if they ever
    // disagree, the sampler plays silence with a warning instead of failing the build.
    let (_dir, sfz) = patch_dir();
    let media = resolving(&sfz, "/elsewhere/tone.sfz");
    let audio = render_with(&["lib/other".to_owned()], Some("lib/tone"), &media).expect("renders");
    assert_eq!(
        audio.stats.warnings,
        ["instrument 'lib/tone' of 'Sonara Sampler' is not installed; it plays silence"]
    );
    assert_eq!(audio.data, render(&[], None).expect("renders").data);
}
