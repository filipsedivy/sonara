//! The import formats added for full parity: CAF 8-bit, odd-length 24-bit
//! mono, µ-law, A-law, little-endian and truncated files, CAF ALAC at 16 to 32 bits and with six
//! channels, and Ogg Opus at every decoding rate, with six channels, an output gain, comments and
//! truncation.
//!
//! The oracle is `tests/fixtures/formats/`, recorded from the prototype's soundfile 0.14
//! (libsndfile 1.2.2) by `scripts/golden/media_format_oracles.py` (GLD-01): `sf.info`, the
//! metadata strings, `inspect_audio` and `sf.read` of every input. The golden media inputs
//! (`golden/media/inputs/`) are checked by `golden_media.rs`.
//!
//! Lossless decoding is exact. Ogg Opus is compared within the allowance `golden_media.rs` uses
//! for the lossy decoders, 1e-5: this crate links libopus 1.6.1 (`opusic-sys`), the recording's
//! libsndfile linked libopus 1.4, and the two round differently (measured at most 5.1e-7 on the
//! golden `ogg-opus.ogg`).

use std::path::{Path, PathBuf};

use serde_json::Value;
use sonara_media::{
    DirectReader, MediaError, MediaReader, inspect_audio, metadata_strings, sound_info,
};

/// The allowance for Ogg Opus samples (see the module documentation).
const OPUS_ALLOWANCE: f64 = 1e-5;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/formats")
}

fn oracle() -> Value {
    let text = std::fs::read_to_string(fixtures().join("oracle.json")).expect("the oracle");
    serde_json::from_str(&text).expect("the oracle is JSON")
}

fn input(name: &str) -> PathBuf {
    fixtures().join("inputs").join(name)
}

/// The recorder's `<ROOT>` was the inputs directory.
fn recorded(text: &str) -> String {
    text.replace("<ROOT>", &fixtures().join("inputs").display().to_string())
}

/// `sf.read` of `name` in float64, interleaved (stored as f64, or as f32 for Opus).
fn decoded(name: &str) -> Vec<f64> {
    let dir = fixtures().join("decoded");
    let header: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join(format!("{name}.json"))).expect("a decoded header"),
    )
    .expect("JSON");
    match header["dtype"].as_str() {
        Some("f64le") => std::fs::read(dir.join(format!("{name}.f64")))
            .expect("decoded samples")
            .as_chunks::<8>()
            .0
            .iter()
            .map(|b| f64::from_le_bytes(*b))
            .collect(),
        Some("f32le") => std::fs::read(dir.join(format!("{name}.f32")))
            .expect("decoded samples")
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f64::from(f32::from_le_bytes(*b)))
            .collect(),
        other => panic!("{name}: unknown dtype {other:?}"),
    }
}

fn files() -> Vec<(String, Value)> {
    oracle()["files"]
        .as_object()
        .expect("files")
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

fn is_opus(name: &str) -> bool {
    name.starts_with("ogg-opus")
}

fn read_all(path: &Path) -> Vec<f64> {
    let mut reader = MediaReader::Direct(DirectReader::open(path).expect("opens"));
    let frames = usize::try_from(reader.frames()).expect("frames");
    reader.read(0, frames).expect("reads")
}

fn max_abs(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}

#[test]
fn every_input_is_recorded_and_present() {
    let names: Vec<String> = files().into_iter().map(|(name, _)| name).collect();
    assert!(names.len() >= 20, "{names:?}");
    for name in &names {
        assert!(input(name).is_file(), "{name}");
    }
}

#[test]
fn sound_info_matches_libsndfile() {
    for (name, entry) in files() {
        let path = input(&name);
        if let Some(message) = entry.get("sf_error") {
            let err = sound_info(&path).expect_err(&name);
            assert_eq!(
                err.to_string(),
                recorded(message.as_str().expect("text")),
                "{name}"
            );
            assert!(
                matches!(err, MediaError::Rejected { .. }),
                "{name}: {err:?}"
            );
            continue;
        }
        let expected = &entry["sf_info"];
        let info = sound_info(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(info.format.name(), expected["format"], "{name}");
        assert_eq!(info.subtype.name(), expected["subtype"], "{name}");
        assert_eq!(info.sample_rate, expected["samplerate"], "{name}");
        assert_eq!(info.channels, expected["channels"], "{name}");
        assert_eq!(info.frames, expected["frames"], "{name}");
    }
}

#[test]
fn metadata_strings_match_libsndfile() {
    for (name, entry) in files() {
        let Some(strings) = entry.get("strings").and_then(Value::as_object) else {
            continue;
        };
        let ours = metadata_strings(&input(&name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let expected: Vec<(String, String)> = sonara_media::METADATA_FIELDS
            .iter()
            .filter_map(|field| {
                let value = strings.get(*field)?.as_str()?;
                (!value.is_empty()).then(|| ((*field).to_owned(), value.to_owned()))
            })
            .collect();
        let ours: Vec<(String, String)> =
            ours.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
        assert_eq!(ours, expected, "{name}");
    }
}

#[test]
fn inspect_audio_matches_the_importer() {
    for (name, entry) in files() {
        let path = input(&name);
        if let Some(error) = entry.get("inspect_error") {
            let err = inspect_audio(&path).expect_err(&name);
            assert_eq!(err.code.as_str(), error["code"], "{name}");
            assert_eq!(
                err.message,
                recorded(error["message"].as_str().expect("text")),
                "{name}"
            );
            continue;
        }
        let mut expected = entry["inspect"].clone();
        expected["path"] = Value::String(recorded(expected["path"].as_str().expect("path")));
        let ours = inspect_audio(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(ours.to_json(), expected, "{name}");
    }
}

#[test]
fn decoding_matches_libsndfile() {
    for (name, entry) in files() {
        if entry.get("sf_info").is_none() {
            continue;
        }
        if let Some(frames) = entry.get("decoded_frames") {
            // A large file, recorded without its samples (the last page is searched for from
            // the end of the file, beyond the first 64 KiB window).
            let path = input(&name);
            assert!(
                std::fs::metadata(&path).expect("the file").len() > 64 * 1024,
                "{name}"
            );
            let channels =
                usize::try_from(entry["sf_info"]["channels"].as_u64().expect("channels"))
                    .expect("channels");
            let ours = read_all(&path);
            assert_eq!(
                Some((ours.len() / channels) as u64),
                frames.as_u64(),
                "{name}"
            );
            continue;
        }
        let expected = decoded(&name);
        let ours = read_all(&input(&name));
        assert_eq!(ours.len(), expected.len(), "{name}: sample count");
        if is_opus(&name) {
            let max = max_abs(&ours, &expected);
            println!("{name}: max |Δ| {max:.3e}");
            assert!(max < OPUS_ALLOWANCE, "{name}: max |Δ| {max:e}");
            // libopus decodes to f32, which the reader widens exactly.
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the check of exact widening"
            )]
            let widened = ours
                .iter()
                .all(|&x| f64::from(x as f32).to_bits() == x.to_bits());
            assert!(widened, "{name}: not f32 values");
        } else {
            let exact = ours
                .iter()
                .zip(&expected)
                .all(|(a, b)| a.to_bits() == b.to_bits());
            assert!(exact, "{name}: max |Δ| {:e}", max_abs(&ours, &expected));
        }
    }
}

#[test]
fn reads_from_anywhere_equal_the_sequential_decode() {
    for name in [
        "caf-alac24-long.caf",
        "caf-pcm16-truncated.caf",
        "caf-ulaw.caf",
        "ogg-opus-long.ogg",
        "ogg-opus-16k.ogg",
    ] {
        let path = input(name);
        let whole = read_all(&path);
        let mut reader = MediaReader::Direct(DirectReader::open(&path).expect("opens"));
        let channels = reader.channels();
        let frames = i64::try_from(reader.frames()).expect("frames");
        // Backwards and forwards, across packet boundaries, and past both ends.
        for (start, count) in [
            (frames - 900, 600),
            (5000, 300),
            (4090, 20),
            (-50, 100),
            (8000, 2000),
            (0, 10),
            (frames - 5, 40),
        ] {
            let window = reader.read(start, count).expect("a window");
            for (k, value) in window.chunks_exact(channels).enumerate() {
                let frame = start + i64::try_from(k).expect("index");
                let expected: Vec<f64> = usize::try_from(frame)
                    .ok()
                    .and_then(|f| whole.get(f * channels..(f + 1) * channels))
                    .map_or_else(|| vec![0.0; channels], <[f64]>::to_vec);
                assert_eq!(value, expected.as_slice(), "{name}: frame {frame}");
            }
        }
    }
}
