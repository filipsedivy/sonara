//! The importer's multichannel warning and the exporter's guards on disk against the prototype
//! (gaps M1 to M3, TST-03): the ports of
//! `test_import_warns_about_multichannel_and_lossy`, `test_existing_output_requires_overwrite`
//! and `test_cancelled_export_leaves_no_files` of the prototype's
//! `tests/integration/test_import_export.py`.
//!
//! `tests/fixtures/export_guards.json` is generated from the prototype by
//! `scripts/golden/media_export_guards.py`. `golden/export/errors.json` pins the codes and
//! messages of the conflict and the cancellation (`golden_export.rs`); this oracle adds the
//! 4-channel file and its import report, the bytes a refused export leaves, how often an export
//! polls for a cancellation, and every path a cancelled export leaves, in the render pass and
//! in the encoding pass.

use std::f64::consts::PI;
use std::fs;
use std::path::Path;

use serde_json::Value;
use sonara_core::model::media::MediaKind;
use sonara_media::export::{ExportOptions, Exporter, OutputSpec};
use sonara_media::{import_path, import_warnings, inspect_audio};
use sonara_testkit::sha256_hex;

/// The generator's placeholder for its temporary root.
const ROOT: &str = "<ROOT>";

fn fixture() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/export_guards.json");
    serde_json::from_slice(&fs::read(path).expect("the fixture")).expect("fixture JSON")
}

fn text(value: &Value) -> &str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("a string: {value}"))
}

fn count(value: &Value) -> usize {
    usize::try_from(value.as_u64().unwrap_or_else(|| panic!("a count: {value}")))
        .expect("a small count")
}

fn hex(digits: &str) -> Vec<u8> {
    assert!(digits.len().is_multiple_of(2), "whole bytes");
    (0..digits.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&digits[i..i + 2], 16).expect("hex digits"))
        .collect()
}

/// Every path under `folder`, relative and sorted; folders end with `/` (the generator's
/// `listing`).
fn listing(folder: &Path) -> Vec<String> {
    fn walk(base: &Path, folder: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(folder).expect("a readable folder") {
            let path = entry.expect("an entry").path();
            let rel = path
                .strip_prefix(base)
                .expect("under the base")
                .to_string_lossy()
                .into_owned();
            if path.is_dir() {
                out.push(format!("{rel}/"));
                walk(base, &path, out);
            } else {
                out.push(rel);
            }
        }
    }
    let mut out = Vec::new();
    walk(folder, folder, &mut out);
    out.sort();
    out
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("a list: {value}"))
        .iter()
        .map(|v| text(v).to_owned())
        .collect()
}

/// `music` of `test_import_export.py` at `rate`: harmonic, slowly modulated stereo material,
/// interleaved, in blocks of `block` frames. Only its length matters to these tests.
fn music(seconds: f64, rate: u32, block: usize) -> Vec<Vec<f64>> {
    let amp = 0.25;
    let left = [220.0, 277.18, 329.63, 440.0];
    let right = [220.0, 329.63, 554.37];
    // `int(seconds * sr)` frames.
    let frames = (0..u32::MAX)
        .take_while(|&i| f64::from(i) < seconds * f64::from(rate))
        .map(|i| {
            let t = f64::from(i) / f64::from(rate);
            let env = 0.6 + 0.4 * (2.0 * PI * 0.7 * t).sin();
            let sum = |freqs: &[f64], phase: f64| -> f64 {
                freqs
                    .iter()
                    .zip(1_u32..)
                    .map(|(f, n)| (2.0 * PI * f * t + phase).sin() / f64::from(n))
                    .sum()
            };
            [
                amp * sum(&left, 0.0) * env / 2.0,
                amp * sum(&right, 0.3) * env / 2.0,
            ]
        })
        .collect::<Vec<_>>();
    frames
        .chunks(block)
        .map(|chunk| chunk.iter().flatten().copied().collect())
        .collect()
}

/// M1: the prototype's 4-channel WAV imports with its warning: the same inspection, copy path
/// and warnings as `import_audio`.
#[test]
fn import_warns_about_multichannel() {
    let oracle = &fixture()["multichannel"];
    let file = &oracle["file"];
    let mut bytes = hex(text(&file["head_hex"]));
    bytes.resize(bytes.len() + count(&file["zero_bytes"]), 0);
    bytes.extend(hex(text(&file["tail_hex"])));
    assert_eq!(sha256_hex(&bytes), file["sha256"], "the prototype's file");
    let dir = tempfile::tempdir().expect("a folder");
    let name = text(&file["name"]);
    let path = dir.path().join(name);
    fs::write(&path, &bytes).expect("the file");

    let info = inspect_audio(&path).expect("a readable WAV");
    let expected = &oracle["inspect"];
    assert_eq!(info.format, expected["format"]);
    assert_eq!(info.subtype, expected["subtype"]);
    assert_eq!(info.sample_rate, expected["sample_rate"]);
    assert_eq!(info.channels, expected["channels"]);
    assert_eq!(info.frames, expected["frames"]);
    assert_eq!(info.size_bytes, expected["size_bytes"]);
    assert_eq!(
        info.bit_depth.map(u64::from),
        expected["bit_depth"].as_u64()
    );
    assert_eq!(info.lossy, expected["lossy"]);
    assert_eq!(
        import_path(text(&oracle["asset_id"]), MediaKind::Audio, name),
        oracle["path"]
    );
    assert_eq!(import_warnings(name, &info), strings(&oracle["warnings"]));
}

/// M2: an existing output is refused with the prototype's error and keeps its bytes; with
/// `overwrite` the export replaces it with the prototype's file shape.
#[test]
fn existing_output_requires_overwrite() {
    let oracle = &fixture()["conflict"];
    let rate = 48_000;
    let dir = tempfile::tempdir().expect("a folder");
    let root = fs::canonicalize(dir.path()).expect("the folder resolves");
    let case = root.join("conflict");
    fs::create_dir(&case).expect("the case folder");
    let path = case.join("e.wav");
    fs::write(&path, text(&oracle["existing"])).expect("the existing file");
    let signal = music(oracle["seconds"].as_f64().expect("seconds"), rate, 4096);
    let exporter = Exporter::new(case.join("tmp"));

    let error = exporter
        .export(
            &signal,
            rate,
            &[OutputSpec::new(&path)],
            &ExportOptions::default(),
        )
        .expect_err("an existing output")
        .to_domain_error();
    let root_text = root.to_str().expect("a UTF-8 path");
    assert_eq!(error.code.as_str(), oracle["error"]["code"]);
    assert_eq!(
        error.message.replace(root_text, ROOT),
        oracle["error"]["message"]
    );
    assert_eq!(
        fs::read_to_string(&path).expect("the kept file"),
        text(&oracle["kept"])
    );

    let result = exporter
        .export(
            &signal,
            rate,
            &[OutputSpec::new(&path)],
            &ExportOptions {
                overwrite: true,
                ..ExportOptions::default()
            },
        )
        .expect("the export replaces the file");
    let info = inspect_audio(&path).expect("the written file");
    let expected = &oracle["overwritten"];
    assert_eq!(info.format, expected["format"]);
    assert_eq!(info.subtype, expected["subtype"]);
    assert_eq!(info.sample_rate, expected["samplerate"]);
    assert_eq!(info.channels, expected["channels"]);
    assert_eq!(info.frames, expected["frames"]);
    assert_eq!(result.outputs[0].frames, info.frames);
    assert_eq!(listing(&case), strings(&oracle["listing"]));
}

/// M3: an export polls for a cancellation as often as the prototype's; cancelled in the render
/// pass or in the encoding pass, it fails like the prototype's and leaves the same paths: no
/// output, no `.partial` file, an empty temporary folder.
#[test]
fn cancelled_export_leaves_no_files() {
    let oracle = &fixture()["cancel"];
    let rate = 48_000;
    let block = count(&oracle["block_frames"]);
    let signal = music(oracle["seconds"].as_f64().expect("seconds"), rate, block);
    assert_eq!(
        signal.len(),
        count(&oracle["render_polls"]),
        "one poll a block"
    );
    let dir = tempfile::tempdir().expect("a folder");
    let root = fs::canonicalize(dir.path()).expect("the folder resolves");
    let run = |name: &str, case: &Value| {
        let folder = root.join(name);
        fs::create_dir(&folder).expect("the case folder");
        let after = case["cancel_after_polls"].as_u64();
        let polls = std::cell::Cell::new(0_u64);
        let cancelled = || {
            polls.set(polls.get() + 1);
            after.is_some_and(|n| polls.get() > n)
        };
        let outcome = Exporter::new(folder.join("tmp")).export(
            &signal,
            rate,
            &[OutputSpec::new(folder.join("x.wav"))],
            &ExportOptions {
                cancelled: Some(&cancelled),
                ..ExportOptions::default()
            },
        );
        let what = format!("{name}: {case}");
        match (outcome, case.get("error")) {
            (Ok(_), None) => {}
            (Err(e), Some(expected)) => {
                let e = e.to_domain_error();
                assert_eq!(e.code.as_str(), expected["code"], "{what}");
                assert_eq!(e.message, expected["message"], "{what}");
            }
            (outcome, _) => panic!("{what}: {:?}", outcome.map(|_| ())),
        }
        assert_eq!(polls.get(), case["polls"], "{what}");
        assert_eq!(listing(&folder), strings(&case["listing"]), "{what}");
    };
    run("whole", &oracle["whole"]);
    let cases = oracle["cases"].as_array().expect("the cases");
    assert_eq!(cases.len(), 3);
    for (i, case) in cases.iter().enumerate() {
        run(&format!("case-{i}"), case);
    }
}
