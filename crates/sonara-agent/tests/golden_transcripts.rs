//! The golden agent batches replayed through the Rust agent service (CTR-01, GLD-01): `golden/coverage/` and every
//! `golden/workflows/<name>/`.
//!
//! Every step runs live and must give the recorded envelope (see `replay`). After every step
//! the session's project must hash to the recorded snapshot, and every recorded `state` view
//! (from `create_project`, `open_project` and `get_project_state`) must equal the view of that
//! project. After each batch, the files its steps wrote are compared with the recorded outputs
//! (`outputs/`): previews, their analysis sidecars, exports, stems, MIDI files and saved
//! projects.
//!
//! **Allowances.** The analyses in the results are of `float64` renders, before any storage, and
//! no render reproduces the prototype's `float64` samples bit for bit, even where its `float32`
//! file does. `replay::allowances` says, step by step, how far each analysis (and the size of a
//! FLAC file of changed samples) may differ and why, with the residual measured on macOS, the
//! recording platform; every other field compares exactly. A step-scoped allowance that a step
//! no longer needs fails the test.
//!
//! Unix only: the golden data spells sandbox paths the POSIX way (`<ROOT>/work`), as the file
//! sandbox tests of `tests/context.rs` do.
#![cfg(unix)]

mod replay;

use std::fs;

use serde_json::{Map, Value};
use sonara_agent::mcp::dumps;
use sonara_project::canonical::to_canonical_bytes;
use sonara_testkit::Golden;

use replay::allowances::{Allowance, allowances};
use replay::{Replay, Report};

/// Whether FLAC files are compared byte for byte: on macOS, where libFLAC 1.4.3 is the
/// recording's build; elsewhere another libFLAC writes other bytes for the same samples
/// (`FLAC_BYTE_EXACT` of `crates/sonara-media/tests/golden_export.rs`, which pins their lossless
/// content).
const FLAC_BYTE_EXACT: bool = cfg!(target_os = "macos");

/// Whether rendered audio files (WAV, MP3) compare byte for byte: on macOS, the recording
/// platform. Elsewhere the processors' transcendental functions come from another libm (`svf` and `osc_bank` follow the platform's `tan`, `sin` and
/// `exp2`), so the `float64` renders differ in ulps, which can move a quantized sample (in the
/// development container: the mix's 24-bit `fixed-mix.wav`). There the files must keep their
/// size, which the frame counts and the constant bitrates fix.
const AUDIO_BYTE_EXACT: bool = cfg!(target_os = "macos");

/// The recorded output files a batch does not reproduce byte for byte, because the render
/// residual (`replay::allowances`: the reverb's `float32` path, NumPy's SIMD transcendentals,
/// the MP3 decoder) reaches their samples. Every other output file is byte-identical (WAV files
/// without libsndfile's `PEAK` chunk, which the recorder strips for its wall-clock time; FLAC
/// files where [`FLAC_BYTE_EXACT`]).
fn residual_files(batch: &str) -> &'static [&'static str] {
    match batch {
        "coverage" => &[
            "previews/full.wav",
            "stems/Coverage---06-saw-44k1-stereo.flac",
        ],
        "cinematic_orchestral" => &[
            "examples/output/cinematic_orchestral/ascension.flac",
            "examples/output/cinematic_orchestral/ascension.wav",
            "examples/output/cinematic_orchestral/climax.wav",
            "examples/output/cinematic_orchestral/preview.wav",
        ],
        "electronic_124bpm" => &[
            "examples/output/electronic_124bpm/drop1-preview.wav",
            "examples/output/electronic_124bpm/night-drive.flac",
            "examples/output/electronic_124bpm/night-drive.mp3",
            "examples/output/electronic_124bpm/night-drive.wav",
        ],
        "halloween_thirteenth_hour" => &[
            "examples/output/halloween/preview.wav",
            "examples/output/halloween/trinacta_hodina.mp3",
            "examples/output/halloween/trinacta_hodina.wav",
        ],
        "mix_analysis" => &["examples/output/mix_analysis/fixed.wav"],
        _ => &[],
    }
}

fn replay(mut batch: Replay) -> Report {
    let name = batch.name().to_owned();
    let base = batch.base().to_owned();
    let declared = allowances(&name);
    for allowance in &declared {
        batch.allow(allowance.clone());
    }
    let report = batch.run();
    let failures = report.failures(&name);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(
        report.fast_forwarded,
        [] as [usize; 0],
        "{name}: every step runs live"
    );
    // A step-scoped allowance that a step no longer needs is stale.
    for allowance in &declared {
        for step in allowance.steps {
            assert!(
                report
                    .allowances_used
                    .iter()
                    .any(|(s, used, _)| s == step && *used == allowance.name),
                "{name} step {step}: {} is not needed any more ({})",
                allowance.name,
                allowance.reason
            );
        }
    }
    check_outputs(&name, &base, &report, &declared);
    // The measured differences, for the record (`--nocapture`).
    let mut largest: Vec<(&str, usize, f64)> = Vec::new();
    for &(step, allowance, used) in &report.allowances_used {
        match largest.iter_mut().find(|(name, _, _)| *name == allowance) {
            Some(entry) if entry.2 < used => *entry = (allowance, step, used),
            Some(_) => {}
            None => largest.push((allowance, step, used)),
        }
    }
    for (allowance, step, used) in largest {
        println!("{name}: {allowance} at most {used:.2e} (step {step})");
    }
    report
}

/// Every recorded output file of the batch against the file the replay wrote: sidecars as the
/// recorder normalized them (`<ROOT>`, `<VOLATILE>`, sorted keys), with the allowances of the
/// step that wrote them; every other file byte for byte unless [`residual_files`] lists it.
fn check_outputs(name: &str, base: &str, report: &Report, declared: &[Allowance]) {
    let golden = Golden::open().expect("golden data is available");
    let prefix = format!("{base}/outputs/");
    let files: Vec<String> = golden
        .manifest()
        .files
        .keys()
        .filter_map(|rel| rel.strip_prefix(&prefix).map(str::to_owned))
        .collect();
    assert!(!files.is_empty(), "{name}: recorded outputs");
    let residual = residual_files(name);
    for rel in &files {
        let ours = fs::read(report.work_file(rel))
            .unwrap_or_else(|e| panic!("{name}: {rel} was not written: {e}"));
        let recorded = golden
            .bytes(&format!("{prefix}{rel}"))
            .expect("a recorded output");
        let recorded = if rel.ends_with("manifest.json") {
            patch_manifest_version(&recorded)
        } else {
            recorded
        };
        if rel.ends_with(".analysis.json") {
            check_sidecar(name, rel, &ours, &recorded, report, declared);
            continue;
        }
        let ours = if has_extension(rel, "wav") {
            strip_peak_chunk(&ours)
        } else {
            ours
        };
        if has_extension(rel, "flac") && !FLAC_BYTE_EXACT {
            continue;
        }
        if (has_extension(rel, "wav") || has_extension(rel, "mp3")) && !AUDIO_BYTE_EXACT {
            assert_eq!(ours.len(), recorded.len(), "{name}: the size of {rel}");
            continue;
        }
        if !report.libm_steps.is_empty() && rel.contains(".sonara/") {
            check_saved_project(name, rel, &ours, &recorded);
            continue;
        }
        if residual.contains(&rel.as_str()) {
            assert_ne!(
                ours, recorded,
                "{name}: {rel} is exact now; remove it from the list"
            );
        } else {
            assert!(ours == recorded, "{name}: {rel} differs from the recording");
        }
    }
    for rel in residual {
        assert!(
            files.iter().any(|f| f == rel),
            "{name}: {rel} is not recorded"
        );
    }
}

/// GLD-01 deviation: the recorder pinned the prototype's own version (0.1.0) into
/// `sonara_version`. The field reports the current application version (VER-01), so it moves
/// with every release; replace the recording's value with the build's before any comparison.
fn patch_manifest_version(manifest: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(manifest).replace(
        "\"sonara_version\": \"0.1.0\"",
        &format!("\"sonara_version\": \"{}\"", env!("CARGO_PKG_VERSION")),
    );
    text.into_bytes()
}

/// A saved project of a batch whose project equals the recording only within `replay::LIBM`
/// (off macOS, after a humanizing `generate_midi`): `project.json` within that tolerance, and
/// `manifest.json` exactly except the hash and size of `project.json`, which follow from it (a
/// float one ulp away can take another digit: 60,258 bytes instead of 60,257 for cinematic in
/// the development container).
fn check_saved_project(name: &str, rel: &str, ours: &[u8], recorded: &[u8]) {
    let ours: Value = serde_json::from_slice(ours).expect("a JSON file");
    let recorded: Value = serde_json::from_slice(recorded).expect("a JSON file");
    if rel.ends_with("/project.json") {
        assert!(
            replay::within_libm(&recorded, &ours),
            "{name}: {rel} differs beyond LIBM"
        );
        return;
    }
    let mut ours = ours;
    for key in ["/project_file/sha256", "/project_file/size"] {
        if let (Some(slot), Some(value)) = (ours.pointer_mut(key), recorded.pointer(key)) {
            slot.clone_from(value);
        }
    }
    assert_eq!(ours, recorded, "{name}: {rel}");
}

/// A preview's analysis sidecar: written as `json.dumps(stored, indent=1)` in the prototype's
/// key order, and equal to the recording, normalized as the recorder normalized it, byte for
/// byte, with the analysis floats within the allowances of the step that rendered it.
fn check_sidecar(
    name: &str,
    rel: &str,
    ours: &[u8],
    recorded: &[u8],
    report: &Report,
    declared: &[Allowance],
) {
    let text = std::str::from_utf8(ours).expect("an ASCII sidecar");
    let stored: Value = serde_json::from_str(text).expect("a JSON sidecar");
    assert_eq!(
        dumps(&stored, 1),
        text,
        "{name}: {rel} is Python's json.dumps(indent=1)"
    );
    let keys: Vec<&str> = stored
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    let mut order = vec![
        "path",
        "seconds",
        "sample_rate",
        "range_beats",
        "render_seconds",
        "realtime_factor",
        "latency_samples",
        "tail_seconds",
        "warnings",
        "missing_media",
        "analysis",
        "issues",
    ];
    if stored.get("track_levels").is_some() {
        order.push("track_levels");
    }
    assert_eq!(
        keys, order,
        "{name}: {rel} keys in PreviewResult.to_json order"
    );
    assert!(
        stored["analysis"].get("short_term_lufs").is_some(),
        "{name}: {rel} keeps the loudness series"
    );
    // The step that rendered the preview: its result names the WAV.
    let wav = format!(
        "{}/work/{}",
        replay::ROOT,
        rel.replace(".analysis.json", ".wav")
    );
    let step = report
        .live
        .iter()
        .find(|live| live.actual.pointer("/result/path") == Some(&Value::from(wav.clone())))
        .unwrap_or_else(|| panic!("{name}: no step rendered {wav}"))
        .index;
    let root = report.root();
    let mut actual = replay::normalize(&stored, &root, &replay::VOLATILE_KEYS);
    let expected: Value = serde_json::from_slice(recorded).expect("a recorded sidecar");
    for allowance in declared.iter().filter(|a| a.covers(step)) {
        allowance.apply(&expected, &mut actual);
    }
    let ours = to_canonical_bytes(&sorted(&actual));
    if ours != recorded {
        let found = replay::difference(&expected, &actual).unwrap_or_default();
        panic!("{name}: {rel} differs from the recording (step {step}): {found}");
    }
}

/// Whether the file `rel` has the extension `extension`, in any case.
fn has_extension(rel: &str, extension: &str) -> bool {
    std::path::Path::new(rel)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
}

/// `value` with every object's keys sorted (`json.dumps(sort_keys=True)`).
fn sorted(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            Value::Object(
                keys.into_iter()
                    .map(|k| (k.clone(), sorted(&map[k])))
                    .collect::<Map<String, Value>>(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

/// The recorder's `strip_peak_chunk`: a RIFF WAVE file without its `PEAK` chunk, which carries
/// libsndfile's wall-clock time.
fn strip_peak_chunk(wav: &[u8]) -> Vec<u8> {
    if wav.get(..4) != Some(b"RIFF".as_slice()) || wav.get(8..12) != Some(b"WAVE".as_slice()) {
        return wav.to_vec();
    }
    let mut body = b"WAVE".to_vec();
    let mut pos = 12;
    while let Some(header) = wav.get(pos..pos + 8) {
        let size = u32::from_le_bytes(header[4..8].try_into().expect("four bytes")) as usize;
        let end = (pos + 8 + size + (size & 1)).min(wav.len());
        if &header[..4] != b"PEAK" {
            body.extend_from_slice(&wav[pos..end]);
        }
        pos = end;
    }
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&u32::try_from(body.len()).expect("a WAV size").to_le_bytes());
    out.extend(body);
    out
}

/// The steps of `batch` that carry a project state view.
fn state_steps(batch: &Replay) -> usize {
    batch
        .steps()
        .iter()
        .filter(|step| {
            step.result
                .as_ref()
                .is_some_and(|r| r.get("state").is_some())
        })
        .count()
}

#[test]
fn coverage_batch_matches_the_prototype() {
    let batch = Replay::coverage();
    let states = state_steps(&batch);
    let report = replay(batch);
    let mut counts: Vec<(&str, usize)> = report.live_counts().into_iter().collect();
    counts.sort_unstable();
    assert_eq!(
        counts,
        [
            ("add_effect", 2),
            ("add_midi_notes", 5),
            ("analyze_audio", 2),
            ("cancel_operation", 3),
            ("create_midi_clip", 2),
            ("create_project", 5),
            ("create_track", 4),
            ("delete_clips", 4),
            ("delete_track", 2),
            ("duplicate_clip", 4),
            ("edit_clip", 14),
            ("edit_midi_notes", 10),
            ("export_stems", 6),
            ("get_audio_analysis", 3),
            ("get_capabilities", 3),
            ("get_history", 3),
            ("get_operation_status", 7),
            ("get_project_state", 1),
            ("import_audio", 9),
            ("import_midi", 4),
            ("list_midi_devices", 1),
            ("list_operations", 2),
            ("list_plugins", 5),
            ("list_tracks", 2),
            ("load_impulse_response", 4),
            ("move_clip", 8),
            ("open_project", 5),
            ("play_preview", 1),
            ("redo", 4),
            ("relink_media", 9),
            ("remove_effect", 3),
            ("rename_track", 4),
            ("render_preview", 2),
            ("save_project", 2),
            ("set_loop", 4),
            ("set_midi_input", 2),
            ("set_track_state", 5),
            ("split_clip", 3),
            ("start_recording", 1),
            ("stop_playback", 2),
            ("undo", 3),
            ("verify_project", 6),
        ]
    );
    // Nothing in the coverage batch humanizes notes: every project hashes alike.
    assert_eq!(report.libm_steps, [] as [usize; 0]);
    // The created project, the reopened project and the final full state.
    assert_eq!(report.states.len(), states);
    assert_eq!(states, 3);
}

/// The live steps of each workflow, per tool: every step of the transcript.
fn expected_live(name: &str) -> &'static [(&'static str, usize)] {
    match name {
        "cinematic_orchestral" => &[
            ("add_section", 4),
            ("configure_routing", 1),
            ("create_automation", 3),
            ("create_project", 1),
            ("create_track", 1),
            ("edit_midi_notes", 2),
            ("export_audio", 1),
            ("export_midi", 1),
            ("generate_midi", 7),
            ("render_preview", 2),
            ("save_project", 1),
            ("set_metadata", 1),
            ("set_tempo", 2),
            ("set_track_volume", 2),
        ],
        "electronic_124bpm" => &[
            ("add_effect", 6),
            ("add_section", 6),
            ("configure_routing", 2),
            ("create_automation", 2),
            ("create_project", 1),
            ("create_track", 6),
            ("export_audio", 1),
            ("export_midi", 1),
            ("generate_midi", 18),
            ("get_project_state", 1),
            ("render_preview", 1),
            ("save_project", 1),
            ("set_metadata", 1),
            ("set_track_pan", 1),
            ("set_track_volume", 4),
        ],
        "halloween_thirteenth_hour" => &[
            ("add_effect", 3),
            ("add_marker", 3),
            ("add_section", 10),
            ("configure_routing", 16),
            ("create_automation", 3),
            ("create_midi_clip", 50),
            ("create_project", 1),
            ("create_track", 19),
            ("export_audio", 1),
            ("get_audio_analysis", 1),
            ("render_preview", 1),
            ("save_project", 1),
            ("set_instrument", 17),
            ("set_metadata", 1),
            ("set_project_length", 1),
            ("set_tempo", 12),
            ("set_time_signature", 6),
            ("set_track_volume", 17),
        ],
        "mix_analysis" => &[
            ("add_effect", 2),
            ("create_project", 1),
            ("create_track", 3),
            ("export_audio", 1),
            ("generate_midi", 3),
            ("get_history", 1),
            ("render_preview", 2),
            ("set_track_volume", 6),
        ],
        "sidechain_preview" => &[
            ("add_effect", 2),
            ("analyze_audio", 2),
            ("create_midi_clip", 1),
            ("create_project", 1),
            ("create_track", 2),
            ("export_audio", 1),
            ("generate_midi", 1),
            ("render_preview", 1),
            ("save_project", 1),
            ("set_effect_parameter", 1),
            ("set_track_volume", 1),
        ],
        other => panic!("unexpected workflow {other}"),
    }
}

#[test]
fn workflows_match_the_prototype() {
    let names = Replay::workflows();
    assert_eq!(
        names,
        [
            "cinematic_orchestral",
            "electronic_124bpm",
            "halloween_thirteenth_hour",
            "mix_analysis",
            "sidechain_preview",
        ]
    );
    for name in names {
        let batch = Replay::workflow(&name);
        let states = state_steps(&batch);
        let batch_steps = batch.steps().to_vec();
        let report = replay(batch);
        assert_eq!(report.states.len(), states, "{name}");
        let mut live: Vec<(&str, usize)> = report.live_counts().into_iter().collect();
        live.sort_unstable();
        let expected = expected_live(&name);
        assert_eq!(live, expected, "{name}");
        check_libm_scope(&name, &report, &batch_steps);
    }
}

/// The steps of a workflow whose project equals the snapshot only within `replay::LIBM`: none on
/// macOS, which records the golden data, so every step reaches its exact hash. Elsewhere,
/// floats may differ only from a humanizing `generate_midi` on: every run of consecutive such
/// steps starts at one (later live steps keep the difference until a fast-forwarded step loads a
/// recorded snapshot).
fn check_libm_scope(name: &str, report: &Report, steps: &[replay::Step]) {
    if !replay::LIBM_ALLOWED {
        assert_eq!(report.libm_steps, [] as [usize; 0], "{name}");
    }
    let mut previous: Option<usize> = None;
    for &i in &report.libm_steps {
        if previous.is_none_or(|p| i != p + 1) {
            assert!(
                humanizing_generate_midi(&steps[i]),
                "{name} step {i} is within LIBM without a humanizing generate_midi"
            );
        }
        previous = Some(i);
    }
}

/// Whether `step` is a `generate_midi` that humanizes its notes through `random.gauss` (unless
/// `humanize` is false).
fn humanizing_generate_midi(step: &replay::Step) -> bool {
    matches!(
        &step.action,
        replay::Action::Tool { name, args, .. }
            if name == "generate_midi"
                && args.get("humanize") != Some(&serde_json::Value::Bool(false))
    )
}
