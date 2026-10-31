//! `sonara.sampler` inside an offline render (ADR 0010): the instrument of a track is an SFZ
//! patch named by the device's `instrument` parameter. The library is generated in a tempdir; nothing licensed is
//! needed. These checks prove the integration (patch loaded, notes reach the sampler, output is
//! audible and does not clip); they say nothing about how a real library sounds.
//!
//! **The chase** (a seek, a loop wrap, a render that starts later): a sampler gets the clip's
//! last keyswitch and its last modulation, expression and sustain values before the new
//! position, so it plays there as playing up to it does. The oracle is the same graph played
//! without the jump, bit for bit. The binary installs `AllocDisabler`, and every block of these
//! checks is processed inside `assert_no_alloc` (RT-02). The Poly Synth and the Drum Kit keep
//! the plain chase: `tests/golden_render.rs` pins their renders byte for byte (GLD-01).
//!
//! **The `articulation` parameter** of a profiled preset selects the keyswitch its mapping
//! names, from a stored choice or an automation lane. The oracle is the same clip in `auto` with
//! keyswitch notes that select the same switches; a seek and every pass of a loop must play as
//! straight play does, with the clip's own keyswitches winning after the parameter's.
//!
//! **A note's own articulation** plays that note from its own frame over the device's
//! `articulation`, and the notes after it play the device's again. The oracle is the same notes
//! in `auto` with keyswitch notes at the marked note's frame and after it; a seek into the marked
//! note chases it with its articulation. Other instruments ignore it: the base project's Drum
//! Kit and Poly Synth render bit for bit as without.
//!
//! **The `dynamics` parameter** scales the velocity of later notes from a lane, as notes
//! written softer play; on a preset whose dynamics drive expression it sets CC 11 as a clip's
//! event does, also after a seek and on every pass of a loop.
//!
//! **The overlap legato** of a preset that claims it joins clip notes that abut (the engine
//! sends the note-off of a frame before its note-ons): the oracle is the same notes in
//! `sustain`, equal before the first join and after each crossfade, and different inside it. A
//! seek into the phrase starts the note there with its attack, as in `sustain`, and the next
//! note joins it.
//!
//! **Live rebuilds** (an edit during playback) keep a sampler whose instrument is unchanged, so
//! its sounding notes ring on: the oracle is the graph that was never rebuilt, bit for bit, and
//! a new `articulation` reaches the kept sampler exactly as a lane that steps there does.
//! Another preset, or the same preset with a changed mapping, builds the sampler again, and its
//! notes stop. The render thread's swap and adoption run inside `assert_no_alloc`, and the
//! replaced graph is dropped outside it, as the player drops it off the render thread (RT-02,
//! RT-09).

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use serde_json::{Value, json};
use sonara_core::model::project::Project;
use sonara_core::plugins::{PluginRegistry, SamplerChoices, builtin_registry, sampler_descriptor};
use sonara_dsp::bank::{ArticulationTarget, DynamicsDrive, LegatoSpec, SamplerMapping, Selector};
use sonara_engine::{
    Applied, DEFAULT_BLOCK, GraphBuilder, GraphOptions, OfflineRenderer, RenderGraph,
    RenderRequest, RenderedAudio, Tail, Update, apply,
};
use sonara_media::{InstrumentSource, MediaProvider};
use sonara_project::serializer::{loads, project_from_value, project_to_value};
use sonara_testkit::Golden;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const KICK: &str = "trk_000000000003";

#[expect(
    clippy::cast_possible_truncation,
    reason = "the amplitude 16000 fits an i16"
)]
/// A 16-bit mono WAV of a sine at `hz`, one second long at 44.1 kHz.
fn write_wav(path: &Path, hz: f64) {
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
        let x = (f64::from(i) / 44_100.0 * hz * std::f64::consts::TAU).sin();
        let value = (x * 16_000.0).round() as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(path, bytes).expect("write the wav");
}

fn patch_dir() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a tempdir");
    write_wav(&dir.path().join("tone.wav"), 110.0);
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
    let params = instrument.map_or_else(|| json!({}), |name| json!({"instrument": name}));
    render_params(installed, &params, media)
}

/// The sampler's choices: the `installed` presets, two instrument types and three
/// articulations, as a stand-in for the application's lists. The `articulation` choices are
/// `auto` (0), `ordinario` (1), `sustain` (2), `staccato` (3) and `legato` (4).
fn choices(installed: &[String]) -> SamplerChoices {
    SamplerChoices {
        presets: installed.to_vec(),
        instrument_types: vec!["horn_f".to_owned(), "violin".to_owned()],
        articulations: vec![
            "sustain".to_owned(),
            "staccato".to_owned(),
            "legato".to_owned(),
        ],
    }
}

/// The base project with the kick track's drum kit replaced by the sampler, which offers the
/// `installed` choices and stores `params`.
fn render_params(
    installed: &[String],
    params: &Value,
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
    track["instrument"] = json!({"id": "dev_000000000004", "plugin_id": "sonara.sampler",
        "name": "Sonara Sampler", "enabled": true, "params": params, "sidechain_id": null,
        "preset": null, "state": {}});
    let project = project_from_value(&data).expect("the edited project loads");
    let mut registry = builtin_registry();
    if let Some(descriptor) = sampler_descriptor(&choices(installed)) {
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
    let sfz = dir.path().join("unsupported.sfz");
    fs::write(
        &sfz,
        "<curve> curve_index=7 v000=0\n\
         <region> sample=tone.wav lokey=36 hikey=36 pitch_keycenter=36 ampeg_release=0.1 \
         trigger=pedal cutoff=500\n",
    )
    .expect("write the patch");
    let audio = render(&[path(&sfz)], None).expect("renders");
    assert_eq!(
        audio.stats.warnings,
        [format!(
            "instrument {} uses SFZ features the sampler does not play: <curve>, cutoff, \
             trigger=pedal",
            sonara_core::pyfmt::str_repr(&path(&sfz))
        )]
    );
    assert!(
        peak(&audio) > 0.01,
        "the region with an unknown trigger plays as attack"
    );
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
            mapping: Arc::default(),
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

#[test]
fn a_preset_without_claims_plays_every_articulation_choice_as_the_patch_is() {
    // A preset about which nothing is claimed resolves with the default mapping: no
    // articulation target, velocity dynamics, no legato and no key offset. Every articulation
    // choice then plays the patch as it is. The oracles: the render with `auto`, and the patch
    // named by its path without a resolver, as before preset IDs.
    let (_dir, sfz) = patch_dir();
    let media = resolving(&sfz, "/elsewhere/tone.sfz");
    let installed = ["lib/tone".to_owned()];
    let as_is = render(&[path(&sfz)], None).expect("renders");
    let auto = render_params(
        &installed,
        &json!({"instrument": "lib/tone", "articulation": "auto"}),
        &media,
    )
    .expect("renders");
    assert_eq!(auto.data, as_is.data);
    assert!(peak(&auto) > 0.01, "the patch is audible");
    for articulation in ["ordinario", "sustain", "staccato"] {
        let audio = render_params(
            &installed,
            &json!({"instrument": "lib/tone", "articulation": articulation, "dynamics": 1.0}),
            &media,
        )
        .expect("renders");
        assert!(
            audio.stats.warnings.is_empty(),
            "{:?}",
            audio.stats.warnings
        );
        assert_eq!(audio.data, auto.data, "{articulation}");
    }
}

#[test]
fn an_unknown_instrument_type_or_articulation_plays_as_auto_with_a_warning() {
    // TST-03: a project saved by a newer version of Sonara can store a choice that this one
    // does not offer. The device plays as `auto` with a warning instead of failing the build
    // (ADR 0010).
    let (_dir, sfz) = patch_dir();
    let installed = [path(&sfz)];
    let auto = render_params(
        &installed,
        &json!({"instrument_type": "auto", "articulation": "auto"}),
        &no_media(),
    )
    .expect("renders");
    assert!(auto.stats.warnings.is_empty(), "{:?}", auto.stats.warnings);
    assert!(peak(&auto) > 0.01, "the patch is audible");

    let newer = render_params(
        &installed,
        &json!({"instrument_type": "ophicleide", "articulation": "col_legno"}),
        &no_media(),
    )
    .expect("renders instead of failing");
    assert_eq!(
        newer.stats.warnings,
        [
            "instrument_type 'ophicleide' of 'Sonara Sampler' is unknown to this version of \
             Sonara; it plays as 'auto'",
            "articulation 'col_legno' of 'Sonara Sampler' is unknown to this version of \
             Sonara; it plays as 'auto'"
        ]
    );
    assert_eq!(newer.data, auto.data, "it plays exactly as auto");
}

#[test]
fn offered_choices_and_dynamics_play_without_a_warning() {
    // A stored choice, given by its text or its index, is kept: nothing falls back.
    let (_dir, sfz) = patch_dir();
    let installed = [path(&sfz)];
    for params in [
        json!({"instrument_type": "violin", "articulation": "staccato", "dynamics": 0.5}),
        json!({"instrument_type": 1, "articulation": 1, "dynamics": 0.0}),
    ] {
        let audio = render_params(&installed, &params, &no_media()).expect("renders");
        assert!(
            audio.stats.warnings.is_empty(),
            "{params}: {:?}",
            audio.stats.warnings
        );
    }
}

#[test]
fn a_stored_choice_that_is_not_text_still_fails_the_build() {
    // Only an unknown text is read as a choice of a newer version; any other invalid value
    // fails as it does for every device.
    let (_dir, sfz) = patch_dir();
    let installed = [path(&sfz)];
    for params in [
        json!({"articulation": true}),
        json!({"articulation": 99}),
        json!({"instrument_type": 2.5}),
    ] {
        let error = render_params(&installed, &params, &no_media()).expect_err("an invalid value");
        assert!(
            matches!(
                &error,
                sonara_engine::EngineError::Params { device, .. } if device == "dev_000000000004"
            ),
            "{params}: {error}"
        );
    }
}

/// A patch with three articulations of the keys 36 to 72, a 110, 220 and 330 Hz tone, selected
/// by the keyswitch keys C1 (24, the default), C#1 (25) and D1 (26). The release is 5 ms, so a
/// note is silent soon after its end.
fn keyswitch_patch() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a tempdir");
    let mut sfz = String::from(
        "<global> sw_lokey=24 sw_hikey=26 sw_default=24 ampeg_release=0.005 \
         lokey=36 hikey=72 pitch_keycenter=60\n",
    );
    for (switch, hz) in [(24, 110.0), (25, 220.0), (26, 330.0)] {
        write_wav(&dir.path().join(format!("{switch}.wav")), hz);
        writeln!(sfz, "<region> sw_last={switch} sample={switch}.wav").expect("writes a string");
    }
    let patch = dir.path().join("articulations.sfz");
    fs::write(&patch, sfz).expect("write the patch");
    (dir, patch)
}

/// The frames of a beat at 120 bpm and the base project's 48 kHz.
const BEAT: i64 = 24_000;
/// The frames of a block.
const BLOCK: i64 = 512;
/// A block start after the switch to C#1 and before the note it selects (beat 0.896).
const AFTER_SWITCH: i64 = 42 * BLOCK;
/// A block start after the last note, which is silent there (beat 4.5013).
const AFTER_LAST_NOTE: i64 = 211 * BLOCK;
/// The end of the checks (beat 5.12).
const END: i64 = 240 * BLOCK;

/// A clip note: `pitch` from beat `start`, `length` beats long, at velocity 100.
fn note(id: &str, pitch: u8, start: f64, length: f64) -> Value {
    json!({"id": id, "pitch": pitch, "start": start, "length": length, "velocity": 100,
           "channel": 0})
}

/// The base project reduced to one sampler track that plays `instrument` at 120 bpm, without
/// the master's limiter, so the output is the sampler's alone. Its clip sets the expression
/// (CC 11) at beat 0 and plays C4 with the default articulation; switches to C#1 at beat 0.75
/// and plays C4 at beat 1; switches to D1 at beat 3, lowers the expression at beat 3.25 and
/// plays D4 at beat 3.5. Each note is half a beat long.
fn keyswitch_project(instrument: &str) -> (Project, PluginRegistry) {
    switched_project(instrument, 0)
}

/// [`keyswitch_project`] with every note of its clip `moved` semitones up, keyswitch notes
/// included.
fn switched_project(instrument: &str, moved: u8) -> (Project, PluginRegistry) {
    let expression = |beat: f64, value: u8| {
        json!({"beat": beat, "kind": "cc", "value": value, "number": 11,
               "channel": 0})
    };
    sampler_project(
        &json!({"instrument": instrument}),
        &[
            note("n_000000000001", 60 + moved, 0.0625, 0.5),
            note("n_000000000002", 25 + moved, 0.75, 0.125),
            note("n_000000000003", 60 + moved, 1.0, 0.5),
            note("n_000000000004", 26 + moved, 3.0, 0.125),
            note("n_000000000005", 62 + moved, 3.5, 0.5),
        ],
        &[expression(0.0, 100), expression(3.25, 50)],
        &[],
    )
}

/// The base project reduced to one sampler track at 120 bpm, without the master's limiter, so
/// the output is the sampler's alone. The sampler stores `params` and offers their
/// `instrument`; its clip, eight beats long, plays `notes` and `controls`, and the track has the
/// automation lanes `automation`.
fn sampler_project(
    params: &Value,
    notes: &[Value],
    controls: &[Value],
    automation: &[Value],
) -> (Project, PluginRegistry) {
    let golden = Golden::open().expect("golden data is available (set SONARA_GOLDEN)");
    let bytes = golden
        .bytes("workflows/sidechain_preview/final.project.json")
        .expect("the base project");
    let mut data = loads(&bytes).expect("project JSON");
    let instrument = params["instrument"]
        .as_str()
        .expect("the instrument's choice")
        .to_owned();
    let clip = json!({"type": "midi", "id": "clp_000000000008", "name": "Articulations",
        "start": 0.0, "length": 8.0, "content_offset": 0.0, "loop_length": null, "muted": false,
        "color": null, "notes": notes, "controls": controls});
    let mut track = data
        .get("tracks")
        .and_then(Value::as_array)
        .and_then(|tracks| tracks.iter().find(|t| t["id"] == KICK))
        .expect("the kick track")
        .clone();
    track["instrument"] = json!({"id": SAMPLER, "plugin_id": "sonara.sampler",
        "name": "Sonara Sampler", "enabled": true, "params": params,
        "sidechain_id": null, "preset": null, "state": {}});
    track["clips"] = json!([clip]);
    track["automation"] = json!(automation);
    data.insert("tracks".to_owned(), json!([track]));
    data.insert(
        "tempo_map".to_owned(),
        json!([{"beat": 0.0, "bpm": 120.0, "ramp": false}]),
    );
    data.get_mut("master").expect("the master")["inserts"] = json!([]);
    let project = project_from_value(&data).expect("the edited project loads");
    let mut registry = builtin_registry();
    let descriptor = sampler_descriptor(&choices(&[instrument])).expect("one instrument");
    registry.register(descriptor).expect("registers once");
    (project, registry)
}

/// The sampler device of [`sampler_project`].
const SAMPLER: &str = "dev_000000000004";

/// A fresh graph of [`keyswitch_project`], reset as for play from stop.
fn keyswitch_graph(patch: &Path) -> RenderGraph {
    graph_of(&keyswitch_project(&path(patch)), &no_media())
}

/// A fresh graph of `project` with the instruments of `media`, reset as for play from stop.
fn graph_of((project, registry): &(Project, PluginRegistry), media: &MediaProvider) -> RenderGraph {
    let mut graph = RenderGraph::new(
        project,
        registry,
        media,
        DEFAULT_BLOCK,
        &GraphOptions::default(),
    )
    .expect("the graph builds");
    assert!(graph.warnings().is_empty(), "{:?}", graph.warnings());
    assert_eq!(graph.block_size(), 512);
    assert_eq!(graph.latency(), 0);
    graph.reset();
    graph
}

/// Plays the blocks from `from` up to `to`, each processed inside `assert_no_alloc` (RT-02).
fn play(graph: &mut RenderGraph, from: i64, to: i64) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    let mut frame = from;
    while frame < to {
        graph.read_media(frame).expect("no media to read");
        out.extend_from_slice(assert_no_alloc(|| graph.process(frame, true)));
        frame += BLOCK;
    }
    out
}

/// The loudest absolute sample of `frames`.
fn loudest(frames: &[[f64; 2]]) -> f64 {
    frames
        .iter()
        .flatten()
        .fold(0.0, |p, v| f64::max(p, v.abs()))
}

#[test]
fn a_chase_restores_the_clip_key_switch() {
    let (_dir, patch) = keyswitch_patch();
    let mut straight = keyswitch_graph(&patch);
    let heard = play(&mut straight, 0, END);
    // The three notes: beats 0.0625, 1 and 3.5, each half a beat long.
    let notes = [
        (BEAT / 16, BEAT * 9 / 16),
        (BEAT, BEAT * 3 / 2),
        (BEAT * 7 / 2, BEAT * 4),
    ];
    for (on, off) in notes {
        let at = |frame: i64| usize::try_from(frame).unwrap();
        assert!(loudest(&heard[at(on)..at(off)]) > 0.1, "a note at {on}");
    }
    // Each seek comes from after the last note, where D1 and the lower expression are active.
    // Back to beat 0, before every switch, the default articulation C1 plays again; to beat
    // 0.896, the switch C#1 and the expression set before it.
    for to in [0, AFTER_SWITCH] {
        let mut jumping = keyswitch_graph(&patch);
        play(&mut jumping, 0, AFTER_LAST_NOTE);
        let after = play(&mut jumping, to, END);
        let from = usize::try_from(to).unwrap();
        assert!(
            after == heard[from..],
            "a seek to frame {to} plays as played through"
        );
    }
}

#[test]
fn every_pass_of_a_loop_sounds_the_same() {
    // A loop wrap chases like a seek. The first pass starts as playback starts there (a reset
    // and a chase); the switch to D1 and the lower expression inside the loop must not reach
    // the next pass.
    let (_dir, patch) = keyswitch_patch();
    for start in [0, AFTER_SWITCH] {
        let mut graph = keyswitch_graph(&patch);
        let first = play(&mut graph, start, AFTER_LAST_NOTE);
        assert!(loudest(&first) > 0.1, "the loop from frame {start} sounds");
        for pass in 2..=3 {
            let again = play(&mut graph, start, AFTER_LAST_NOTE);
            assert!(again == first, "pass {pass} of the loop from frame {start}");
        }
    }
}

#[test]
fn a_render_from_a_later_beat_is_the_slice_of_the_full_render() {
    // An offline render chases at its first block, so the articulation and the expression
    // that the clip selected before the start apply.
    let (_dir, patch) = keyswitch_patch();
    let (project, registry) = keyswitch_project(&path(&patch));
    let media = no_media();
    let renderer = OfflineRenderer::new(&registry, &media);
    let render = |start_beat: f64| {
        renderer
            .render(
                &project,
                &RenderRequest {
                    start_beat,
                    end_beat: Some(5.0),
                    tail: Tail::None,
                    ..RenderRequest::default()
                },
                None,
            )
            .expect("renders")
    };
    let full = render(0.0);
    // Beat 0.875 is after the switch note ends and before the note it selects.
    let part = render(0.875);
    assert_eq!(part.start_frame, BEAT * 7 / 8);
    assert!(part.stats.warnings.is_empty(), "{:?}", part.stats.warnings);
    assert!(loudest(&part.data) > 0.1, "the note at beat 1 sounds");
    let from = usize::try_from(part.start_frame).unwrap();
    assert!(part.data == full.data[from..], "the render from beat 0.875");
}

/// The presets `lib/articulations` and `lib/keyed-low`, played from the [`keyswitch_patch`] at
/// `patch` with the mapping of a profiled preset: `ordinario` on C1 (24, the patch's default),
/// `sustain` on C#1 (25) and `staccato` on D1 (26), the choices 1 to 3 of [`choices`].
/// `lib/keyed-low` reads the patch as keyed an octave below the pitch it sounds: its key offset
/// is -12.
fn articulations(patch: &Path) -> MediaProvider {
    let patch = patch.to_owned();
    let mapping = |key_offset| {
        Arc::new(SamplerMapping::new(
            vec![None, Some(switch(24)), Some(switch(25)), Some(switch(26))],
            DynamicsDrive::Velocity,
            None,
            key_offset,
        ))
    };
    let presets = [
        ("lib/articulations", mapping(0)),
        ("lib/keyed-low", mapping(-12)),
    ];
    no_media().with_instruments(move |value| {
        presets
            .iter()
            .find(|(id, _)| *id == value)
            .map(|(id, mapping)| InstrumentSource {
                id: (*id).to_owned(),
                patch: patch.clone(),
                mapping: Arc::clone(mapping),
            })
    })
}

/// The target of an articulation that the keyswitch `key` selects.
fn switch(key: u8) -> ArticulationTarget {
    ArticulationTarget {
        selector: Some(Selector::Keyswitch(key)),
        legato: false,
    }
}

/// An automation lane of the sampler's `articulation`, holding the choice index of each point
/// from its beat on.
fn articulation_lane(points: &[(f64, f64)]) -> Value {
    hold_lane("articulation", points)
}

/// An automation lane of the sampler's parameter `param`, holding the value of each point from
/// its beat on.
fn hold_lane(param: &str, points: &[(f64, f64)]) -> Value {
    let points: Vec<Value> = points
        .iter()
        .map(|&(beat, value)| json!({"beat": beat, "value": value, "curve": "hold"}))
        .collect();
    json!({"id": "aut_000000000009", "target": {"device_id": SAMPLER, "param": param},
           "enabled": true, "points": points})
}

/// Notes on C4 at beats 0.0625 and 1 and on D4 at beat 3.5, half a beat each: the notes of
/// [`keyswitch_project`] without its keyswitch notes and controllers.
fn three_notes() -> [Value; 3] {
    [
        note("n_000000000001", 60, 0.0625, 0.5),
        note("n_000000000003", 60, 1.0, 0.5),
        note("n_000000000005", 62, 3.5, 0.5),
    ]
}

/// A block start between the second and the third note, after the lane's change at beat 2
/// (beat 2.133).
const AFTER_CHANGE: i64 = 100 * BLOCK;

/// Checks that playing `project` straight through equals the clip-switched `oracle`, and that a
/// seek from after the last note to each of `seeks` and a loop from there play as straight
/// play does there.
fn plays_as_through(
    project: &(Project, PluginRegistry),
    oracle: &(Project, PluginRegistry),
    media: &MediaProvider,
    seeks: &[i64],
) {
    let heard = play(&mut graph_of(oracle, media), 0, END);
    assert!(loudest(&heard) > 0.1, "the oracle sounds");
    let mut straight = graph_of(project, media);
    assert!(play(&mut straight, 0, END) == heard, "straight play");
    for &to in seeks {
        let mut jumping = graph_of(project, media);
        play(&mut jumping, 0, AFTER_LAST_NOTE);
        let after = play(&mut jumping, to, END);
        let from = usize::try_from(to).expect("a frame of the render");
        assert!(after == heard[from..], "a seek to frame {to}");
        let mut looping = graph_of(project, media);
        let first = play(&mut looping, to, AFTER_LAST_NOTE);
        for pass in 2..=3 {
            assert!(
                play(&mut looping, to, AFTER_LAST_NOTE) == first,
                "pass {pass} of the loop from frame {to}"
            );
        }
    }
}

#[test]
fn a_seek_and_a_loop_keep_the_automated_articulation() {
    // A lane plays `sustain` (C#1) from beat 0 and `staccato` (D1) from beat 2. The oracle is
    // the same notes in `auto` with the keyswitch notes C#1 at beat 0 and D1 at beat 2, which
    // sound nothing: the lane must select exactly what they select, at the same notes, also
    // after a seek and on every pass of a loop.
    let (_dir, patch) = keyswitch_patch();
    let media = articulations(&patch);
    let lane = articulation_lane(&[(0.0, 2.0), (2.0, 3.0)]);
    let automated = sampler_project(
        &json!({"instrument": "lib/articulations"}),
        &three_notes(),
        &[],
        &[lane],
    );
    let mut switched = three_notes().to_vec();
    switched.push(note("n_000000000002", 25, 0.0, 0.03125));
    switched.push(note("n_000000000004", 26, 2.0, 0.125));
    let oracle = sampler_project(
        &json!({"instrument": "lib/articulations"}),
        &switched,
        &[],
        &[],
    );
    plays_as_through(
        &automated,
        &oracle,
        &media,
        &[0, AFTER_SWITCH, AFTER_CHANGE],
    );
}

#[test]
fn a_clip_keyswitch_wins_after_the_articulation_also_after_a_seek() {
    // `staccato` (D1) is the device's articulation, and the clip switches to C#1 at beat 0.75:
    // the first note plays D1, the later ones C#1, as when the clip itself switches to D1 at
    // beat 0. A seek's all notes off applies the articulation again, and the chased keyswitch
    // of the clip wins after it, so a seek before the clip's switch plays D1 and one after it
    // C#1, as straight play does.
    let (_dir, patch) = keyswitch_patch();
    let media = articulations(&patch);
    let [first, second, third] = three_notes();
    let clip_switch = note("n_000000000002", 25, 0.75, 0.125);
    let forced = sampler_project(
        &json!({"instrument": "lib/articulations", "articulation": "staccato"}),
        &[
            first.clone(),
            clip_switch.clone(),
            second.clone(),
            third.clone(),
        ],
        &[],
        &[],
    );
    let oracle = sampler_project(
        &json!({"instrument": "lib/articulations"}),
        &[
            note("n_000000000004", 26, 0.0, 0.03125),
            first,
            clip_switch,
            second,
            third,
        ],
        &[],
        &[],
    );
    plays_as_through(&forced, &oracle, &media, &[0, AFTER_SWITCH, AFTER_CHANGE]);
}

#[test]
fn a_key_offset_moves_the_clip_and_its_keyswitch_notes_also_after_a_seek() {
    // A preset keyed an octave below the pitch it sounds (key offset -12) plays a clip written
    // an octave above the patch's keys, its keyswitch notes C#2 and D2 included, exactly as
    // the patch played unmoved; the chase finds the moved keyswitch notes, so a seek and a loop
    // play as straight play does.
    let (_dir, patch) = keyswitch_patch();
    let media = articulations(&patch);
    plays_as_through(
        &switched_project("lib/keyed-low", 12),
        &switched_project("lib/articulations", 0),
        &media,
        &[0, AFTER_SWITCH, AFTER_CHANGE],
    );
}

/// The presets `lib/by-velocity`, whose dynamics drive the velocity, and `lib/on-expression`,
/// whose dynamics drive expression (CC 11), played from the [`keyswitch_patch`] at `patch`
/// without articulation targets.
fn dynamics_presets(patch: &Path) -> MediaProvider {
    let patch = patch.to_owned();
    no_media().with_instruments(move |value| {
        let drive = match value {
            "lib/by-velocity" => DynamicsDrive::Velocity,
            "lib/on-expression" => DynamicsDrive::Controller(11),
            _ => return None,
        };
        Some(InstrumentSource {
            id: value.to_owned(),
            patch: patch.clone(),
            mapping: Arc::new(SamplerMapping::new(Vec::new(), drive, None, 0)),
        })
    })
}

#[test]
fn an_automated_dynamics_scales_later_notes_also_after_a_seek() {
    // A lane holds `dynamics` at 1 from beat 0 and at 0.5 from beat 2, where no note sounds:
    // the notes before play as written, and the note at beat 3.5 plays at velocity
    // round(100 · 0.5) = 50, exactly as the same note written at 50, also after a seek and on
    // every pass of a loop.
    let (_dir, patch) = keyswitch_patch();
    let media = dynamics_presets(&patch);
    let automated = sampler_project(
        &json!({"instrument": "lib/by-velocity"}),
        &three_notes(),
        &[],
        &[hold_lane("dynamics", &[(0.0, 1.0), (2.0, 0.5)])],
    );
    let [first, second, mut third] = three_notes();
    third["velocity"] = json!(50);
    let oracle = sampler_project(
        &json!({"instrument": "lib/by-velocity"}),
        &[first, second, third],
        &[],
        &[],
    );
    plays_as_through(
        &automated,
        &oracle,
        &media,
        &[0, AFTER_SWITCH, AFTER_CHANGE],
    );
}

#[test]
fn controller_dynamics_set_the_expression_as_a_clip_event_does_also_after_a_seek() {
    // A preset whose dynamics drive expression, stored at 0.5, plays as the same clip on a
    // preset with velocity dynamics whose CC 11 is 64 (round(0.5 · 127)) from beat 0: the
    // clip's own CC 11 at beat 3.25 wins after the parameter. A jump's all notes off writes the
    // parameter's value again, as the chase restores the oracle's CC 11 of beat 0, so a seek
    // and every pass of a loop play as straight play does.
    let (_dir, patch) = keyswitch_patch();
    let media = dynamics_presets(&patch);
    let expression = |beat: f64, value: u8| json!({"beat": beat, "kind": "cc", "value": value, "number": 11, "channel": 0});
    let driven = sampler_project(
        &json!({"instrument": "lib/on-expression", "dynamics": 0.5}),
        &three_notes(),
        &[expression(3.25, 50)],
        &[],
    );
    let oracle = sampler_project(
        &json!({"instrument": "lib/by-velocity"}),
        &three_notes(),
        &[expression(0.0, 64), expression(3.25, 50)],
        &[],
    );
    plays_as_through(&driven, &oracle, &media, &[0, AFTER_SWITCH, AFTER_CHANGE]);
}

/// [`note`] with the note's own `articulation`.
fn marked_note(id: &str, pitch: u8, start: f64, length: f64, articulation: &str) -> Value {
    let mut note = note(id, pitch, start, length);
    note["articulation"] = json!(articulation);
    note
}

/// A block start inside the note of [`three_notes`] at beat 1 (beat 1.067).
const INSIDE_SECOND_NOTE: i64 = 50 * BLOCK;

/// Plays `project` and `oracle` from frame 0 to after the last note, seeks each to `to` and
/// plays on to [`END`], and checks that both play the same from there.
fn jumps_as(
    project: &(Project, PluginRegistry),
    oracle: &(Project, PluginRegistry),
    media: &MediaProvider,
    to: i64,
) {
    let mut expected = graph_of(oracle, media);
    play(&mut expected, 0, AFTER_LAST_NOTE);
    let expected = play(&mut expected, to, END);
    assert!(loudest(&expected) > 0.1, "the oracle sounds after the seek");
    let mut jumping = graph_of(project, media);
    play(&mut jumping, 0, AFTER_LAST_NOTE);
    assert!(
        play(&mut jumping, to, END) == expected,
        "a seek to frame {to}"
    );
}

#[test]
fn a_note_plays_its_own_articulation_from_its_frame_also_after_a_seek() {
    // The device plays `sustain` (C#1). D4 at beat 63/64 plays it; C4 at beat 1, in the same
    // block, plays its own `staccato` (D1) from its own frame; D4 at beat 3.5 plays `sustain`
    // again. The oracle is the same notes in `auto` with keyswitch notes, which sound nothing:
    // C#1 at beat 0, D1 at beat 1 (before C4, which starts with it) and C#1 again at beat 2,
    // after C4 has ended. Applied at the block start instead of the note's frame, `staccato`
    // would reach the D4 of beat 63/64 too.
    let (_dir, patch) = keyswitch_patch();
    let media = articulations(&patch);
    let [first, second, third] = three_notes();
    let early = note("n_000000000006", 62, 63.0 / 64.0, 0.5);
    let staccato = marked_note("n_000000000003", 60, 1.0, 0.5, "staccato");
    let marked = sampler_project(
        &json!({"instrument": "lib/articulations", "articulation": "sustain"}),
        &[first.clone(), early.clone(), staccato, third.clone()],
        &[],
        &[],
    );
    let oracle = sampler_project(
        &json!({"instrument": "lib/articulations"}),
        &[
            first,
            early,
            second,
            third,
            note("n_000000000002", 25, 0.0, 0.03125),
            note("n_000000000004", 26, 1.0, 0.03125),
            note("n_000000000007", 25, 2.0, 0.03125),
        ],
        &[],
        &[],
    );
    plays_as_through(&marked, &oracle, &media, &[0, AFTER_SWITCH, AFTER_CHANGE]);
}

#[test]
fn a_seek_into_a_note_plays_its_own_articulation() {
    // A seek into the C4 of beat 1 chases it with its own `staccato` (D1) over the device's
    // `sustain` (C#1), exactly as a seek there into the oracle's C4, which its chased keyswitch
    // D1 selects; the later D4 plays `sustain` again. A chase that dropped the note's
    // articulation would play C4 on C#1.
    let (_dir, patch) = keyswitch_patch();
    let media = articulations(&patch);
    let [first, second, third] = three_notes();
    let marked = sampler_project(
        &json!({"instrument": "lib/articulations", "articulation": "sustain"}),
        &[
            first.clone(),
            marked_note("n_000000000003", 60, 1.0, 0.5, "staccato"),
            third.clone(),
        ],
        &[],
        &[],
    );
    let oracle = sampler_project(
        &json!({"instrument": "lib/articulations"}),
        &[
            first,
            second,
            third,
            note("n_000000000002", 25, 0.0, 0.03125),
            note("n_000000000004", 26, 1.0, 0.03125),
            note("n_000000000007", 25, 2.0, 0.03125),
        ],
        &[],
        &[],
    );
    jumps_as(&marked, &oracle, &media, INSIDE_SECOND_NOTE);
    // The oracle discriminates: without its articulation the chased C4 plays C#1.
    let (unmarked, registry) = sampler_project(
        &json!({"instrument": "lib/articulations", "articulation": "sustain"}),
        &three_notes(),
        &[],
        &[],
    );
    let mut sustained = graph_of(&(unmarked, registry), &media);
    play(&mut sustained, 0, AFTER_LAST_NOTE);
    let mut expected = graph_of(&oracle, &media);
    play(&mut expected, 0, AFTER_LAST_NOTE);
    assert!(
        play(&mut sustained, INSIDE_SECOND_NOTE, END)
            != play(&mut expected, INSIDE_SECOND_NOTE, END),
        "the note's articulation changes what the chase plays"
    );
}

#[test]
fn an_unknown_note_articulation_plays_the_device_articulation_with_a_warning() {
    // A project saved by a newer version can name an articulation this one does not offer:
    // the note plays the device's `staccato`, as the same note without an articulation does,
    // and the build warns once for the id.
    let (_dir, patch) = keyswitch_patch();
    let media = articulations(&patch);
    let [first, _, third] = three_notes();
    let (project, registry) = sampler_project(
        &json!({"instrument": "lib/articulations", "articulation": "staccato"}),
        &[
            marked_note("n_000000000001", 60, 0.0625, 0.5, "flutter_tongue"),
            marked_note("n_000000000003", 60, 1.0, 0.5, "flutter_tongue"),
            third,
        ],
        &[],
        &[],
    );
    let mut graph = RenderGraph::new(
        &project,
        &registry,
        &media,
        DEFAULT_BLOCK,
        &GraphOptions::default(),
    )
    .expect("the graph builds");
    assert_eq!(
        graph.warnings(),
        [
            "note articulation 'flutter_tongue' on 'Kick' is unknown to this version of Sonara; \
             the note plays the device's articulation"
        ]
    );
    graph.reset();
    let unmarked = sampler_project(
        &json!({"instrument": "lib/articulations", "articulation": "staccato"}),
        &[first, three_notes()[1].clone(), three_notes()[2].clone()],
        &[],
        &[],
    );
    let heard = play(&mut graph_of(&unmarked, &media), 0, END);
    assert!(loudest(&heard) > 0.1, "the notes sound");
    assert!(
        play(&mut graph, 0, END) == heard,
        "the notes play the device's articulation"
    );
}

#[test]
fn other_instruments_ignore_note_articulations() {
    // The base project's Drum Kit and Poly Synth tracks with an articulation on every note
    // render bit for bit as without, and without a warning: only a sampler plays them.
    let golden = Golden::open().expect("golden data");
    let bytes = golden
        .bytes("workflows/sidechain_preview/final.project.json")
        .expect("the base project");
    let mut data = loads(&bytes).expect("project JSON");
    let mut marked = 0;
    for track in data
        .get_mut("tracks")
        .and_then(Value::as_array_mut)
        .expect("tracks")
    {
        for clip in track["clips"].as_array_mut().expect("clips") {
            for note in clip["notes"].as_array_mut().into_iter().flatten() {
                note["articulation"] = json!("staccato");
                marked += 1;
            }
        }
    }
    assert_eq!(marked, 65, "the kick's note and the bass line");
    let project = project_from_value(&data).expect("the marked project loads");
    let media = no_media();
    let audio = OfflineRenderer::new(&builtin_registry(), &media)
        .render(
            &project,
            &RenderRequest {
                end_beat: Some(2.0),
                tail: Tail::None,
                ..RenderRequest::default()
            },
            None,
        )
        .expect("renders");
    let drums = render_drums();
    assert!(
        audio.stats.warnings.is_empty(),
        "{:?}",
        audio.stats.warnings
    );
    assert!(peak(&drums) > 0.01, "the base project sounds");
    assert!(audio.data == drums.data, "bit for bit");
}

/// The presets of [`articulations`], resolved as the application resolves them: every lookup
/// compiles the preset's mapping again, into a new [`Arc`] of an equal mapping. Once `revised`
/// is set, `lib/articulations` resolves with the mapping of a revised profile, which claims
/// `sustain` on D1 (26) and `staccato` on C#1 (25).
fn revisable(patch: &Path, revised: Arc<AtomicBool>) -> MediaProvider {
    let patch = patch.to_owned();
    no_media().with_instruments(move |value| {
        let key_offset = match value {
            "lib/articulations" => 0,
            "lib/keyed-low" => -12,
            _ => return None,
        };
        // Relaxed: the tests set the flag between two graph builds on their own thread.
        let (sustain, staccato) = if value == "lib/articulations" && revised.load(Ordering::Relaxed)
        {
            (26, 25)
        } else {
            (25, 26)
        };
        Some(InstrumentSource {
            id: value.to_owned(),
            patch: patch.clone(),
            mapping: Arc::new(SamplerMapping::new(
                vec![
                    None,
                    Some(switch(24)),
                    Some(switch(sustain)),
                    Some(switch(staccato)),
                ],
                DynamicsDrive::Velocity,
                None,
                key_offset,
            )),
        })
    })
}

/// The built-in plugins and the sampler, which offers both presets of [`revisable`].
fn both_presets() -> PluginRegistry {
    let mut registry = builtin_registry();
    let installed = ["lib/articulations".to_owned(), "lib/keyed-low".to_owned()];
    let descriptor = sampler_descriptor(&choices(&installed)).expect("two instruments");
    registry.register(descriptor).expect("registers once");
    registry
}

/// The sampler track of [`sampler_project`] with `params`: C4 from beat 0.0625, two beats long,
/// so it sounds through [`REBUILD_AT`], and D4 at [`LATER_NOTE`], half a beat long.
fn held_note(params: &Value, automation: &[Value]) -> Project {
    let notes = [
        note("n_000000000001", 60, 0.0625, 2.0),
        note("n_000000000005", 62, 3.5, 0.5),
    ];
    sampler_project(params, &notes, &[], automation).0
}

/// `project` with its sampler track renamed: an edit that changes nothing the graph plays.
fn renamed(project: &Project) -> Project {
    let mut data = project_to_value(project);
    data["tracks"][0]["name"] = json!("Horn");
    let data = data.as_object().expect("a project is an object");
    project_from_value(data).expect("the renamed project loads")
}

/// The block start of the rebuilds, while the first note of [`held_note`] sounds (beat 0.853).
const REBUILD_AT: i64 = 40 * BLOCK;
/// The start of the second note of [`held_note`] (beat 3.5), long after the first one is
/// silent.
const LATER_NOTE: usize = 84_000;

/// A fresh live graph of `project`, which the builder thread of a player built.
fn fresh(
    builder: &mut GraphBuilder,
    project: &Project,
    registry: &PluginRegistry,
    media: &MediaProvider,
) -> Box<RenderGraph> {
    let graph = builder
        .fresh(project, registry, media)
        .expect("the graph builds");
    assert!(graph.warnings().is_empty(), "{:?}", graph.warnings());
    graph
}

/// Swaps `next` in for `current` as the render thread does, inside `assert_no_alloc`, and
/// returns the graph it replaced.
fn swap_in(
    current: &mut Option<Box<RenderGraph>>,
    next: Box<RenderGraph>,
) -> Option<Box<RenderGraph>> {
    let mut update = Update::Graph(Some(next));
    let applied = assert_no_alloc(|| apply(current, &mut update));
    assert_eq!(applied, Applied::Swapped);
    match update {
        Update::Graph(old) => old,
        Update::Setup(_) => panic!("a graph update stays one"),
    }
}

/// Plays `project` live from frame 0 to [`END`], never rebuilt.
fn played_through(
    project: &Project,
    registry: &PluginRegistry,
    media: &MediaProvider,
) -> Vec<[f64; 2]> {
    let mut graph = fresh(
        &mut GraphBuilder::new(DEFAULT_BLOCK),
        project,
        registry,
        media,
    );
    play(&mut graph, 0, END)
}

/// Plays `project` live from frame 0, switches at [`REBUILD_AT`] to a rebuild of `edited`
/// that adopts the playing graph, and plays on to [`END`]. `edit` runs just before the rebuild
/// is built, for an edit outside the project (a library whose mapping is compiled again).
/// Returns the output and the voices the sampler sounds after the first block of the rebuilt
/// graph.
fn rebuilt(
    project: &Project,
    edited: &Project,
    registry: &PluginRegistry,
    media: &MediaProvider,
    edit: impl FnOnce(),
) -> (Vec<[f64; 2]>, f64) {
    let mut builder = GraphBuilder::new(DEFAULT_BLOCK);
    let mut current = None;
    let first = fresh(&mut builder, project, registry, media);
    assert!(swap_in(&mut current, first).is_none());
    let graph = current.as_mut().expect("a graph plays");
    let mut out = play(graph, 0, REBUILD_AT);
    edit();
    let next = builder
        .rebuild(edited, registry, media)
        .expect("the graph builds");
    assert!(next.warnings().is_empty(), "{:?}", next.warnings());
    let old = swap_in(&mut current, next).expect("the playing graph comes back");
    // Off the render thread, as the player drops a replaced graph (RT-09).
    drop(old);
    let graph = current.as_mut().expect("a graph plays");
    out.extend(play(graph, REBUILD_AT, REBUILD_AT + BLOCK));
    let voices = voices(graph);
    out.extend(play(graph, REBUILD_AT + BLOCK, END));
    (out, voices)
}

/// The voices the samplers of `graph` sound after its last block, from their reports.
fn voices(graph: &RenderGraph) -> f64 {
    graph
        .reports()
        .filter_map(|(_, report)| {
            report
                .entries()
                .iter()
                .find(|(name, _)| *name == "voices")
                .map(|&(_, voices)| voices)
        })
        .sum()
}

/// The index of `frame` in an output played from frame 0.
fn at(frame: i64) -> usize {
    usize::try_from(frame).expect("a frame of the render")
}

#[test]
fn an_unchanged_sampler_keeps_its_voices_across_a_rebuild() {
    // An edit that leaves the sampler's instrument alone (a renamed track) keeps the sampler,
    // so the held note rings on: the render equals the graph that was never rebuilt, bit for
    // bit. The resolver compiles a new, equal mapping for every build, and the provider still
    // hands out the same instrument.
    let (_dir, patch) = keyswitch_patch();
    let media = revisable(&patch, Arc::default());
    let registry = both_presets();
    let project = held_note(
        &json!({"instrument": "lib/articulations", "articulation": "sustain"}),
        &[],
    );
    let renamed = renamed(&project);
    let heard = played_through(&project, &registry, &media);
    let (played, voices) = rebuilt(&project, &renamed, &registry, &media, || {});
    assert!(
        played == heard,
        "the held note rings on through the rebuild"
    );
    assert_eq!(voices, 1.0, "the held note's voice is kept");
    let block = at(REBUILD_AT)..at(REBUILD_AT + BLOCK);
    assert!(
        loudest(&heard[block]) > 0.1,
        "the held note sounds at the rebuild, so the comparison discriminates"
    );
}

#[test]
fn an_articulation_edit_reaches_the_kept_sampler_as_a_lane_step_does() {
    // The articulation goes from `sustain` (C#1) to `staccato` (D1) at the rebuild: by an edit
    // and a rebuild, or by a hold lane whose step lies between the clocks of the blocks before
    // and at REBUILD_AT (beat 0.84). The held note rings on as `sustain`; the later note plays
    // `staccato`.
    let (_dir, patch) = keyswitch_patch();
    let media = revisable(&patch, Arc::default());
    let registry = both_presets();
    let sustain = json!({"instrument": "lib/articulations", "articulation": "sustain"});
    let project = held_note(&sustain, &[]);
    let edited = held_note(
        &json!({"instrument": "lib/articulations", "articulation": "staccato"}),
        &[],
    );
    let stepped = held_note(&sustain, &[articulation_lane(&[(0.0, 2.0), (0.84, 3.0)])]);
    let (played, voices) = rebuilt(&project, &edited, &registry, &media, || {});
    assert!(
        played == played_through(&stepped, &registry, &media),
        "the edit plays as the lane's step"
    );
    assert_eq!(voices, 1.0, "the held note's voice is kept");
    let unedited = played_through(&project, &registry, &media);
    assert!(
        played[..LATER_NOTE] == unedited[..LATER_NOTE],
        "the held note rings on as it was"
    );
    assert!(
        played[LATER_NOTE..] != unedited[LATER_NOTE..],
        "the later note plays the new articulation"
    );
    assert!(
        loudest(&played[LATER_NOTE..]) > 0.1,
        "the later note sounds"
    );
}

#[test]
fn another_preset_or_a_changed_mapping_rebuilds_the_sampler() {
    // A rebuilt sampler starts without voices and a rebuild does not chase, so the held note
    // stops at the rebuild; the later note plays the new instrument exactly as a graph that
    // played it from the start does.
    let (_dir, patch) = keyswitch_patch();
    let registry = both_presets();
    let project = held_note(
        &json!({"instrument": "lib/articulations", "articulation": "sustain"}),
        &[],
    );
    let another = held_note(
        &json!({"instrument": "lib/keyed-low", "articulation": "sustain"}),
        &[],
    );
    let revised = Arc::new(AtomicBool::new(false));
    let media = revisable(&patch, Arc::clone(&revised));
    // The edited project, and whether the library's profile is revised at the rebuild.
    let cases = [
        ("another preset", &another, false),
        ("a changed mapping of the same preset", &project, true),
    ];
    for (case, edited, revise) in cases {
        revised.store(false, Ordering::Relaxed);
        let (played, voices) = rebuilt(&project, edited, &registry, &media, || {
            revised.store(revise, Ordering::Relaxed);
        });
        assert_eq!(voices, 0.0, "{case}: the held note's voice is gone");
        assert!(
            loudest(&played[at(REBUILD_AT)..LATER_NOTE]) == 0.0,
            "{case}: the held note stops at the rebuild"
        );
        let heard = played_through(edited, &registry, &media);
        assert!(
            loudest(&heard[LATER_NOTE..]) > 0.1,
            "{case}: the later note sounds"
        );
        assert!(
            played[LATER_NOTE..] == heard[LATER_NOTE..],
            "{case}: the later note plays the new instrument"
        );
    }
}

/// The preset `lib/legato`, played from the [`keyswitch_patch`] at `patch` with the mapping of a
/// preset whose library says it plays legato by overlapping its notes, with a 20 ms crossfade,
/// no start offset and no gap: `sustain` and `legato` (the choices 2 and 4 of [`choices`]) both
/// play C1 (24), and only `legato` joins its notes.
fn legato_preset(patch: &Path) -> MediaProvider {
    let patch = patch.to_owned();
    let target = |legato| {
        Some(ArticulationTarget {
            selector: Some(Selector::Keyswitch(24)),
            legato,
        })
    };
    let mapping = Arc::new(SamplerMapping::new(
        vec![None, target(false), target(false), None, target(true)],
        DynamicsDrive::Velocity,
        Some(LegatoSpec {
            crossfade_ms: 20,
            start_offset_ms: 0,
            gap_ms: 0,
        }),
        0,
    ));
    no_media().with_instruments(move |value| {
        (value == "lib/legato").then(|| InstrumentSource {
            id: value.to_owned(),
            patch: patch.clone(),
            mapping: Arc::clone(&mapping),
        })
    })
}

/// The frames of the 20 ms crossfade at 48 kHz.
const CROSSFADE: usize = 960;
/// The frames after a join from which a joined note sounds as a note that attacks: past the
/// crossfade, with a margin for the last step of its ramp.
const JOINED: usize = CROSSFADE + 64;
/// A block start inside D4 of the legato phrase (beat 0.8107).
const INSIDE_D4: i64 = 38 * BLOCK;

#[test]
fn abutting_legato_notes_crossfade_also_after_a_seek() {
    // C4, D4 and E4 abut: each starts on the frame where the one before ends (13500 and 25500),
    // and the engine sends that note-off first. Under `legato` each later note joins the one
    // before over a 20 ms crossfade; under `sustain` the old note releases in 5 ms and the new
    // one attacks. With no start offset, the joined note then plays the same frames as the
    // attacked one.
    let (_dir, patch) = keyswitch_patch();
    let media = legato_preset(&patch);
    let notes = [
        note("n_000000000001", 60, 0.0625, 0.5),
        note("n_000000000002", 62, 0.5625, 0.5),
        note("n_000000000003", 64, 1.0625, 0.5),
    ];
    let project = |articulation: &str| {
        sampler_project(
            &json!({"instrument": "lib/legato", "articulation": articulation}),
            &notes,
            &[],
            &[],
        )
    };
    let (legato, sustain) = (project("legato"), project("sustain"));
    let joined = play(&mut graph_of(&legato, &media), 0, END);
    let attacked = play(&mut graph_of(&sustain, &media), 0, END);
    assert!(loudest(&joined) > 0.1, "the phrase sounds");
    let [d4, e4] = [13_500, 25_500];
    assert!(
        joined[..d4] == attacked[..d4],
        "the phrase starts with an attack"
    );
    for (join, until) in [(d4, e4), (e4, joined.len())] {
        assert!(
            joined[join..join + CROSSFADE] != attacked[join..join + CROSSFADE],
            "a crossfade at frame {join}"
        );
        assert!(
            joined[join + JOINED..until] == attacked[join + JOINED..until],
            "the note joined at frame {join} plays on as attacked"
        );
    }

    // A seek into D4 starts it with its attack, as in `sustain`, and E4 still joins it; every
    // pass of a loop from there plays the same.
    let seek = |project| {
        let mut graph = graph_of(project, &media);
        play(&mut graph, 0, AFTER_LAST_NOTE);
        play(&mut graph, INSIDE_D4, END)
    };
    let (after, oracle) = (seek(&legato), seek(&sustain));
    let e4_after = e4 - usize::try_from(INSIDE_D4).expect("a frame of the render");
    assert!(
        loudest(&after[..e4_after]) > 0.1,
        "D4 sounds after the seek"
    );
    assert!(
        after[..e4_after] == oracle[..e4_after],
        "D4 attacks after the seek"
    );
    assert!(
        after[e4_after..e4_after + CROSSFADE] != oracle[e4_after..e4_after + CROSSFADE],
        "E4 joins D4 after the seek"
    );
    assert!(
        after[e4_after + JOINED..] == joined[e4 + JOINED..],
        "E4 plays on as in straight play"
    );
    let mut looping = graph_of(&legato, &media);
    let first = play(&mut looping, INSIDE_D4, AFTER_LAST_NOTE);
    for pass in 2..=3 {
        assert!(
            play(&mut looping, INSIDE_D4, AFTER_LAST_NOTE) == first,
            "pass {pass} of the loop"
        );
    }
}
