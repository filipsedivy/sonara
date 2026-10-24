//! Golden parity of the media reading side (GLD-01).
//!
//! `golden/media/` was recorded by `scripts/golden/area_media.py` through the prototype's own
//! code: `audio/engine/media.py`, `import_export/audio_import.py`, soundfile (libsndfile 1.2.2)
//! and python-soxr 1.1. `media/inputs/` holds generated files for every container and encoding
//! libsndfile writes; `fixtures/` holds the N1 media fixtures.
//!
//! Parity targets:
//! - inspection, the direct reader, PCM, A-law, µ-law and FLAC decoding: exact (bit for bit);
//! - the decode cache: byte-identical files and sidecars where the decoding is exact and the
//!   rate is unchanged; the soxr streams: exact (see [`soxr_streams`]); resampled entries: exact
//!   on macOS, elsewhere within the documented noise allowance (see [`NOISE_ULPS`]);
//! - MP3 and Vorbis: frame counts exact, sample values reported (symphonia and mpg123 or
//!   libvorbis round differently).
//!
//! Known gaps are listed in [`GAPS`]; a test fails when a gap closes or a new one opens.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use sonara_core::model::JsonMap;
use sonara_core::model::media::{MediaAsset, MediaAssetData, MediaKind, MediaLocation};
use sonara_media::{
    CacheInfo, DECODE_CHUNK, DirectReader, MediaProvider, MediaReader, ResampleStream,
    frames_at_rate, import_path, import_warnings, inspect_audio, metadata_strings, needs_cache,
    path_stem, path_suffix, recording_path, safe_stem, sound_info,
};
use sonara_testkit::Golden;

/// Files libsndfile reads that this crate does not, with the reason. Empty since the format
/// gaps closed (CAF 8-bit and ALAC, Ogg Opus).
const GAPS: [(&str, &str); 0] = [];

/// Lossy decoders: symphonia's MP3 and Vorbis differ from mpg123 and libvorbis.
fn is_lossy(rel: &str) -> bool {
    Path::new(rel)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mp3") || ext.eq_ignore_ascii_case("ogg"))
}

fn golden() -> Golden {
    Golden::open().expect("golden data is available (run `git lfs pull` or set SONARA_GOLDEN)")
}

fn json(golden: &Golden, rel: &str) -> Value {
    golden
        .json(rel)
        .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn f64s(golden: &Golden, rel: &str) -> Vec<f64> {
    golden
        .f64_array(rel)
        .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
        .data
}

fn f32s(golden: &Golden, rel: &str) -> Vec<f32> {
    let header = json(golden, &format!("{rel}.json"));
    assert_eq!(header["dtype"], "f32le", "{rel}");
    let bytes = golden
        .bytes(&format!("{rel}.f32"))
        .expect("recorded golden value");
    let (samples, rest) = bytes.as_chunks::<4>();
    assert!(rest.is_empty(), "{rel}: partial sample");
    samples.iter().map(|b| f32::from_le_bytes(*b)).collect()
}

fn path(golden: &Golden, rel: &str) -> PathBuf {
    golden.root().join(rel)
}

/// The recorder's `<ROOT>` was its temporary directory, holding `fixtures/` and `inputs/`.
fn recorded_path(golden: &Golden, recorded: &str) -> String {
    let root = golden.root();
    recorded
        .replace(
            "<ROOT>/inputs/",
            &format!("{}/media/inputs/", root.display()),
        )
        .replace("<ROOT>/", &format!("{}/", root.display()))
}

fn gap(rel: &str) -> Option<&'static str> {
    GAPS.iter().find(|(r, _)| *r == rel).map(|(_, why)| *why)
}

/// Whether this crate reads the file at all.
fn opens(rel: &str) -> bool {
    gap(rel).is_none()
}

/// Every file the recorder inspected, with its record.
fn files(golden: &Golden) -> Map<String, Value> {
    json(golden, "media/info.json")
        .as_object()
        .expect("info.json is an object")
        .clone()
}

fn bits_equal(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn max_abs(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a sample count far below 2^53, for a statistic"
)]
fn rms(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().max(1) as f64;
    (a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f64>() / n).sqrt()
}

#[test]
fn sound_info_matches_libsndfile() {
    let golden = golden();
    let mut failures = BTreeSet::new();
    for (rel, entry) in files(&golden) {
        let file = path(&golden, &rel);
        if let Some(message) = entry.get("sf_error") {
            let err = sound_info(&file).expect_err(&rel);
            let expected = recorded_path(&golden, message.as_str().unwrap());
            assert_eq!(err.to_string(), expected, "{rel}");
            continue;
        }
        let expected = &entry["sf_info"];
        match sound_info(&file) {
            Ok(info) => {
                assert_eq!(info.format.name(), expected["format"], "{rel}");
                assert_eq!(info.subtype.name(), expected["subtype"], "{rel}");
                assert_eq!(info.sample_rate, expected["samplerate"], "{rel}");
                assert_eq!(info.channels, expected["channels"], "{rel}");
                assert_eq!(info.frames, expected["frames"], "{rel}");
            }
            Err(err) => {
                let kind = match err {
                    sonara_media::MediaError::Unsupported { .. } => "unsupported",
                    _ => "decode",
                };
                assert_eq!(gap(&rel), Some(kind), "{rel}: {err}");
                failures.insert(rel);
            }
        }
    }
    let expected: BTreeSet<String> = GAPS
        .iter()
        .map(|(rel, _)| (*rel).to_owned())
        .filter(|rel| !opens(rel))
        .collect();
    assert_eq!(failures, expected);
}

/// Metadata strings where libsndfile's reading is not reproduced (see the report).
const STRING_GAPS: [(&str, &str); 1] = [
    // libsndfile appends two bytes after the AIFF `APPL` text ("…)00").
    ("media/inputs/aiff-pcm16-tagged.aiff", "software"),
];

#[test]
fn metadata_strings_match_libsndfile() {
    let golden = golden();
    let mut mismatches = BTreeSet::new();
    for (rel, entry) in files(&golden) {
        let Some(strings) = entry.get("strings").and_then(Value::as_object) else {
            continue;
        };
        if !opens(&rel) {
            continue;
        }
        let ours = metadata_strings(&path(&golden, &rel)).unwrap();
        for (field, value) in strings {
            let value = value.as_str().unwrap();
            let got = ours
                .iter()
                .find(|(f, _)| f == field)
                .map_or("", |(_, v)| v.as_str());
            if got != value {
                mismatches.insert((rel.clone(), field.clone(), got.to_owned()));
            }
        }
    }
    let known: BTreeSet<(String, String)> = STRING_GAPS
        .iter()
        .map(|(r, f)| ((*r).to_owned(), (*f).to_owned()))
        .collect();
    let found: BTreeSet<(String, String)> = mismatches
        .iter()
        .map(|(r, f, _)| (r.clone(), f.clone()))
        .collect();
    assert_eq!(found, known, "{mismatches:#?}");
}

#[test]
fn inspect_audio_matches_the_importer() {
    let golden = golden();
    for (rel, entry) in files(&golden) {
        let file = path(&golden, &rel);
        if let Some(error) = entry.get("inspect_error") {
            let err = inspect_audio(&file).expect_err(&rel);
            assert_eq!(err.code.as_str(), error["code"], "{rel}");
            assert_eq!(
                err.message,
                recorded_path(&golden, error["message"].as_str().unwrap()),
                "{rel}"
            );
            continue;
        }
        if !opens(&rel) {
            continue;
        }
        let mut expected = entry["inspect"].clone();
        let mut ours = inspect_audio(&file).unwrap().to_json();
        assert_eq!(
            ours["path"],
            Value::String(file.display().to_string()),
            "{rel}"
        );
        for value in [&mut expected, &mut ours] {
            let object = value.as_object_mut().unwrap();
            object.remove("path");
            // Compared field by field in `metadata_strings_match_libsndfile`.
            object.remove("metadata");
        }
        assert_eq!(ours, expected, "{rel}");
    }
}

/// Opens a file with the direct reader and reads all of it.
fn decode(file: &Path) -> (usize, Vec<f64>) {
    let reader = DirectReader::open(file).expect("recorded golden value");
    let mut reader = MediaReader::Direct(reader);
    let frames = usize::try_from(reader.frames()).expect("recorded golden value");
    (
        reader.channels(),
        reader.read(0, frames).expect("recorded golden value"),
    )
}

#[test]
fn decoding_matches_libsndfile() {
    let golden = golden();
    for (rel, entry) in files(&golden) {
        if entry.get("sf_info").is_none() || gap(&rel).is_some() {
            continue;
        }
        let key = rel.strip_prefix("media/").unwrap_or(&rel);
        let expected = f64s(&golden, &format!("media/decoded/{key}"));
        let (_, ours) = decode(&path(&golden, &rel));
        assert_eq!(ours.len(), expected.len(), "{rel}: sample count");
        if is_lossy(&rel) {
            let (max, rms) = (max_abs(&ours, &expected), rms(&ours, &expected));
            println!("{rel}: max |Δ| {max:.3e}, rms {rms:.3e}");
            // Measured on the recording: about 1e-6 at most for MP3, 5e-7 for Vorbis, against
            // signals of up to -2 dBFS. A failure here means the decoders drifted apart.
            assert!(max < 1e-5, "{rel}: max |Δ| {max:e}");
            continue;
        }
        assert!(
            bits_equal(&ours, &expected),
            "{rel}: max |Δ| {:e}",
            max_abs(&ours, &expected)
        );
        if rel.starts_with("media/inputs/") {
            // libsndfile's `float` read equals the f64 values rounded to f32.
            let floats = f32s(&golden, &format!("media/decoded32/{key}"));
            #[expect(
                clippy::cast_possible_truncation,
                reason = "libsndfile's float read is the f64 value rounded to f32"
            )]
            let ours: Vec<f32> = ours.iter().map(|&x| x as f32).collect();
            assert!(
                ours.iter()
                    .zip(&floats)
                    .all(|(a, b)| a.to_bits() == b.to_bits()),
                "{rel}: f32"
            );
        }
    }
}

#[test]
fn direct_reader_windows_match() {
    let golden = golden();
    let mut checked = 0;
    for (rel, entry) in files(&golden) {
        let Some(windows) = entry.get("direct_windows").and_then(Value::as_array) else {
            continue;
        };
        if gap(&rel).is_some() {
            continue;
        }
        let key = rel.strip_prefix("media/").unwrap_or(&rel);
        let mut reader = MediaReader::Direct(DirectReader::open(&path(&golden, &rel)).unwrap());
        for (k, window) in windows.iter().enumerate() {
            let start = window["start"].as_i64().unwrap();
            let count = usize::try_from(window["count"].as_u64().unwrap()).unwrap();
            let expected = f64s(&golden, &format!("media/direct/{key}/w{k}"));
            let ours = reader.read(start, count).unwrap();
            assert!(bits_equal(&ours, &expected), "{rel} window {k}");
            checked += 1;
        }
    }
    assert!(checked > 200, "only {checked} windows");
}

fn asset(id: &str, file: &Path, kind: MediaKind) -> MediaAsset {
    let info = sound_info(file).expect("recorded golden value");
    let sha = sonara_testkit::sha256_hex(&std::fs::read(file).expect("recorded golden value"));
    MediaAsset::new(MediaAssetData {
        id: id.to_owned(),
        kind,
        name: file
            .file_name()
            .expect("recorded golden value")
            .to_string_lossy()
            .into_owned(),
        path: file.display().to_string(),
        location: MediaLocation::Linked,
        size_bytes: std::fs::metadata(file)
            .expect("recorded golden value")
            .len(),
        sha256: Some(sha),
        sample_rate: info.sample_rate,
        channels: info.channels,
        frames: info.frames,
        format: info.format.name().to_owned(),
        subtype: info.subtype.name().to_owned(),
        original_path: None,
        relative_hint: None,
        metadata: JsonMap::new(),
        extra: JsonMap::new(),
    })
    .expect("recorded golden value")
}

fn asset_from(id: &str, kind: MediaKind, rate: u32, channels: u32, frames: u64) -> MediaAsset {
    MediaAsset::new(MediaAssetData {
        id: id.to_owned(),
        kind,
        name: "f".to_owned(),
        path: "/f.wav".to_owned(),
        location: MediaLocation::Linked,
        size_bytes: 1,
        sha256: None,
        sample_rate: rate,
        channels,
        frames,
        format: "WAV".to_owned(),
        subtype: "PCM_16".to_owned(),
        original_path: None,
        relative_hint: None,
        metadata: JsonMap::new(),
        extra: JsonMap::new(),
    })
    .expect("recorded golden value")
}

/// One decode-cache entry built by this crate, next to the recorded one.
struct CacheCase {
    stem: String,
    fixture: String,
    resampled: bool,
    exact: bool,
    ours: Vec<f64>,
    expected: Vec<f64>,
}

/// Builds every recorded decode-cache entry and checks its sidecar, length and reader windows.
fn build_cache_entries(golden: &Golden, dir: &Path) -> Vec<CacheCase> {
    let provider_json = json(golden, "media/provider.json");
    let mut cases = Vec::new();
    for case in provider_json["cache"]
        .as_array()
        .expect("recorded golden value")
    {
        let fixture = case["fixture"].as_str().expect("recorded golden value");
        let id = case["asset_id"].as_str().expect("recorded golden value");
        let rate = u32::try_from(case["sample_rate"].as_u64().expect("recorded golden value"))
            .expect("recorded golden value");
        let file = path(golden, fixture);
        let asset = asset(id, &file, MediaKind::Audio);
        assert_eq!(
            asset.sha256.as_deref(),
            case["sha256"].as_str(),
            "{fixture}"
        );
        let provider = MediaProvider::new(|a| PathBuf::from(&a.path), Some(dir.into()));
        provider
            .prepare(&asset, rate, None)
            .expect("recorded golden value");
        assert!(provider.cache_ready(&asset, rate));
        let entry = CacheInfo::new(dir, id, rate);
        let stem = format!("{id}-{rate}");
        let expected_meta = golden
            .bytes(&format!("media/cache/{stem}.json"))
            .expect("recorded golden value");
        assert_eq!(
            String::from_utf8(std::fs::read(&entry.meta).expect("recorded golden value"))
                .expect("recorded golden value"),
            String::from_utf8(expected_meta).expect("recorded golden value"),
            "{stem}: sidecar"
        );
        let expected_raw = golden
            .bytes(&format!("media/cache/{stem}.f32"))
            .expect("recorded golden value");
        let ours_raw = std::fs::read(&entry.raw).expect("recorded golden value");
        assert_eq!(ours_raw.len(), expected_raw.len(), "{stem}: length");
        let to_f64 = |bytes: &[u8]| -> Vec<f64> {
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f64::from(f32::from_le_bytes(*b)))
                .collect()
        };
        let (ours, expected) = (to_f64(&ours_raw), to_f64(&expected_raw));
        let exact = ours_raw == expected_raw;
        let resampled = case["resampled"].as_bool().expect("recorded golden value");
        // Reader windows over our own entry.
        let shared = provider.reader(&asset, rate);
        let mut reader = shared.lock().expect("recorded golden value");
        assert!(matches!(*reader, MediaReader::Cached(_)), "{stem}");
        assert_eq!(
            reader.frames(),
            case["frames"].as_u64().expect("recorded golden value")
        );
        assert_eq!(
            reader.channels() as u64,
            case["channels"].as_u64().expect("recorded golden value")
        );
        for (k, window) in case["windows"]
            .as_array()
            .expect("recorded golden value")
            .iter()
            .enumerate()
        {
            let start = window["start"].as_i64().expect("recorded golden value");
            let count = usize::try_from(window["count"].as_u64().expect("recorded golden value"))
                .expect("recorded golden value");
            let expected = f64s(golden, &format!("media/cache/{stem}/w{k}"));
            let ours = reader.read(start, count).expect("recorded golden value");
            if exact {
                assert!(bits_equal(&ours, &expected), "{stem} window {k}");
            } else {
                assert_eq!(ours.len(), expected.len(), "{stem} window {k}");
            }
        }
        println!(
            "{stem}: {} (max |Δ| {:.3e}, rms {:.3e})",
            if exact { "identical" } else { "differs" },
            max_abs(&ours, &expected),
            rms(&ours, &expected)
        );
        cases.push(CacheCase {
            stem,
            fixture: fixture.to_owned(),
            resampled,
            exact,
            ours,
            expected,
        });
    }
    cases
}

/// The decode cache without resampling, and the lossy decodes: data, sidecar, reader windows.
#[test]
fn decode_cache_matches() {
    let golden = golden();
    let dir = tempfile::tempdir().unwrap();
    for case in build_cache_entries(&golden, dir.path()) {
        if is_lossy(&case.fixture) {
            // The MP3 decoder bound of `decoding_matches_libsndfile`, through soxr when resampled.
            let max = max_abs(&case.ours, &case.expected);
            assert!(max < 1e-5, "{}: max |Δ| {max:e}", case.stem);
        } else if !case.resampled {
            assert!(case.exact, "{}: decoded data differs", case.stem);
        }
    }
}

/// The resampled decode-cache entries match python-soxr's: bit-exact on
/// macOS, and within [`NOISE_ULPS`] at the filter's rounding noise elsewhere.
#[test]
fn decode_cache_resampling_matches() {
    let golden = golden();
    let dir = tempfile::tempdir().unwrap();
    for case in build_cache_entries(&golden, dir.path()) {
        if case.resampled && !is_lossy(&case.fixture) {
            assert_soxr_parity(&case.stem, &case.ours, &case.expected);
        }
    }
}

/// Off macOS, the largest difference allowed in resampled output, in `f32` ulps, and only for
/// samples whose recorded magnitude is below [`NOISE_FLOOR`] (GLD-01).
///
/// Justification: where the ideal output is zero (the exact zero crossings of the fixture's
/// 440 Hz and 660 Hz sines at 44.1 kHz), libsoxr's VHQ result is only the double-precision
/// rounding noise of its filter, and that noise depends on how libsoxr was compiled. Three
/// builds disagree there, each against the recording (python-soxr 1.1 on macOS, libsoxr commit
/// `a66f3ee`, Apple Clang), for the 48 → 44.1 kHz tone entry (176 400 samples):
/// - Debian trixie's `libsoxr0` 0.1.3 (GCC, arm64), the development container: 50 samples;
/// - python-soxr 1.1's Linux wheel, the same commit `a66f3ee` (GCC, arm64): 56 samples;
/// - the two GCC builds against each other: 54 samples.
///
/// Every one of those samples differs by exactly one `f32` ulp at a magnitude below 1.3e-8
/// (largest |Δ| 8.9e-16, about −301 dBFS). Every other sample, the frame count, the other
/// resampled entries and all 115 streams of [`soxr_streams`] are bit-exact in the container.
/// macOS, the product and recording platform, ships libsoxr built like the recording's and
/// stays exact.
const NOISE_ULPS: u32 = 1;

/// The magnitude below which a recorded sample counts as rounding noise (−140 dBFS). The
/// differing samples measured above are all below 1.3e-8; the floor leaves a margin of about
/// eight, and stays far below any audible or analysed level.
const NOISE_FLOOR: f64 = 1e-7;

/// The most samples per entry that may use the allowance: twice the largest count measured
/// (56, python-soxr's own Linux build). A build that disturbs more noise samples than that is
/// a different arithmetic, not a different compiler, and must be investigated.
const MAX_NOISE_SAMPLES: usize = 112;

/// The distance of two `f32` values in ulps, across zero as well.
fn ulps(a: f32, b: f32) -> u64 {
    let ordered = |x: f32| {
        let bits = i64::from(x.to_bits());
        if bits & 0x8000_0000 == 0 {
            bits
        } else {
            -(bits & 0x7FFF_FFFF)
        }
    };
    ordered(a).abs_diff(ordered(b))
}

/// soxr parity of an `f32` result (passed widened to `f64`). The recording's libsoxr is
/// python-soxr's bundled build (`index.json` → `libsoxr`); this test loads the system libsoxr.
/// On macOS every sample must be bit-exact; elsewhere [`NOISE_ULPS`] applies below
/// [`NOISE_FLOOR`], at most [`MAX_NOISE_SAMPLES`] times. The frame count is always exact.
fn assert_soxr_parity(case: &str, ours: &[f64], expected: &[f64]) {
    let golden = golden();
    let index = json(&golden, "media/index.json");
    let running = ResampleStream::libsoxr_version().unwrap_or_default();
    let recorded = &index["libsoxr"];
    assert_eq!(ours.len(), expected.len(), "{case}: sample count");
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the values are f32 samples widened to f64; narrowing is exact"
    )]
    let narrow = |x: f64| x as f32;
    let (noise, other): (Vec<_>, Vec<_>) = ours
        .iter()
        .zip(expected)
        .enumerate()
        .filter(|(_, (a, b))| a.to_bits() != b.to_bits())
        .map(|(i, (a, b))| (i, *a, *b))
        .partition(|&(_, a, b)| {
            !cfg!(target_os = "macos")
                && b.abs() < NOISE_FLOOR
                && ulps(narrow(a), narrow(b)) <= u64::from(NOISE_ULPS)
        });
    assert!(
        other.is_empty(),
        "{case}: soxr output differs from python-soxr in {} of {} samples outside the noise \
         allowance (first {:?}): max |Δ| {:e}, rms {:e}; recorded with libsoxr {recorded}, \
         running {running}",
        other.len(),
        ours.len(),
        &other[..other.len().min(8)],
        max_abs(ours, expected),
        rms(ours, expected),
    );
    if !noise.is_empty() {
        let largest = noise.iter().map(|&(_, _, b)| b.abs()).fold(0.0, f64::max);
        println!(
            "{case}: {} of {} samples within the {NOISE_ULPS}-ulp noise allowance below \
             {NOISE_FLOOR:e}, the largest at |x| = {largest:e} (recorded with libsoxr \
             {recorded}, running {running})",
            noise.len(),
            ours.len()
        );
    }
    assert!(
        noise.len() <= MAX_NOISE_SAMPLES,
        "{case}: {} samples need the noise allowance, more than {MAX_NOISE_SAMPLES} \
         (first {:?}); recorded with libsoxr {recorded}, running {running}",
        noise.len(),
        &noise[..noise.len().min(8)],
    );
}

/// soxr VHQ streams of a sine and of noise between every pair of supported rates, in the
/// decode cache's chunking.
#[test]
fn soxr_streams() {
    let golden = golden();
    let index = json(&golden, "media/index.json");
    let mut worst = (0.0f64, String::new());
    let mut differing = 0;
    let cases = index["soxr"].as_array().unwrap();
    for case in cases {
        let input = f32s(&golden, case["input"].as_str().unwrap());
        let expected = f32s(&golden, case["output"].as_str().unwrap());
        let channels = usize::try_from(case["channels"].as_u64().unwrap()).unwrap();
        let in_rate = u32::try_from(case["in_rate"].as_u64().unwrap()).unwrap();
        let out_rate = u32::try_from(case["out_rate"].as_u64().unwrap()).unwrap();
        let mut stream = ResampleStream::new(in_rate, out_rate, channels).unwrap();
        let header = json(
            &golden,
            &format!("{}.json", case["output"].as_str().unwrap()),
        );
        assert_eq!(stream.engine(), header["engine"], "{}", case["output"]);
        let mut ours = Vec::new();
        let mut sizes = Vec::new();
        let mut pos = 0;
        loop {
            let end = (pos + DECODE_CHUNK * channels).min(input.len());
            let block = &input[pos..end];
            pos = end;
            let last = block.len() < DECODE_CHUNK * channels;
            let out = stream.resample_chunk(block, last).unwrap();
            sizes.push(Value::from(out.len() / channels));
            ours.extend(out);
            if last {
                break;
            }
        }
        let rel = case["output"].as_str().unwrap();
        assert_eq!(ours.len(), expected.len(), "{rel}: frames");
        assert_eq!(
            Value::Array(sizes),
            header["chunk_frames"],
            "{rel}: frames per chunk"
        );
        let ours: Vec<f64> = ours.into_iter().map(f64::from).collect();
        let expected: Vec<f64> = expected.into_iter().map(f64::from).collect();
        if !bits_equal(&ours, &expected) {
            differing += 1;
            let max = max_abs(&ours, &expected);
            if max > worst.0 {
                worst = (max, rel.to_owned());
            }
        }
    }
    println!(
        "soxr: {differing} of {} streams differ; worst max |Δ| {:e} in {}",
        cases.len(),
        worst.0,
        worst.1
    );
    assert_eq!(
        differing,
        0,
        "soxr output differs from python-soxr in {differing} streams (worst max |Δ| {:e} in {}); \
         recorded with libsoxr {}, running {}",
        worst.0,
        worst.1,
        index["libsoxr"],
        ResampleStream::libsoxr_version().unwrap_or_default()
    );
}

#[test]
fn needs_cache_matches() {
    let golden = golden();
    let provider_json = json(&golden, "media/provider.json");
    for row in provider_json["needs_cache"].as_array().unwrap() {
        let asset_rate = u32::try_from(row["asset_rate"].as_u64().unwrap()).unwrap();
        let rate = u32::try_from(row["rate"].as_u64().unwrap()).unwrap();
        assert_eq!(
            needs_cache(
                row["format"].as_str().unwrap(),
                row["subtype"].as_str().unwrap(),
                asset_rate,
                rate
            ),
            row["needs_cache"].as_bool().unwrap(),
            "{row}"
        );
    }
}

#[test]
fn cache_validity_matches() {
    let golden = golden();
    let provider_json = json(&golden, "media/provider.json");
    let dir = tempfile::tempdir().unwrap();
    for row in provider_json["cache_ready"].as_array().unwrap() {
        let entry = CacheInfo::new(dir.path(), "med_v", 48_000);
        let _ = std::fs::remove_file(&entry.raw);
        let _ = std::fs::remove_file(&entry.meta);
        if let Some(len) = row["raw"].as_u64() {
            std::fs::write(&entry.raw, vec![0u8; usize::try_from(len).unwrap()]).unwrap();
        }
        if let Some(sidecar) = row["sidecar"].as_str() {
            std::fs::write(&entry.meta, sidecar).unwrap();
        }
        let mut asset = asset_from("med_v", MediaKind::Audio, 44_100, 2, 10);
        let rate = u32::try_from(row["asset_rate"].as_u64().unwrap()).unwrap();
        asset = asset
            .edit(|a| {
                a.sha256 = row["asset_sha256"].as_str().map(str::to_owned);
                a.format = row["asset_format"].as_str().unwrap().to_owned();
                a.sample_rate = rate;
            })
            .unwrap();
        let cache_dir = row["cache_dir"]
            .as_bool()
            .unwrap()
            .then(|| dir.path().into());
        let provider = MediaProvider::new(|a| PathBuf::from(&a.path), cache_dir);
        assert_eq!(
            provider.cache_ready(&asset, 48_000),
            row["ready"].as_bool().unwrap(),
            "{}",
            row["case"]
        );
    }
}

#[test]
fn missing_media_is_silent_with_the_prototypes_length() {
    let golden = golden();
    let provider_json = json(&golden, "media/provider.json");
    let missing = tempfile::tempdir().unwrap();
    for row in provider_json["silent"].as_array().unwrap() {
        let kind = if row["asset_id"] == "med_midi" {
            MediaKind::Midi
        } else {
            MediaKind::Audio
        };
        let asset = asset_from(
            row["asset_id"].as_str().unwrap(),
            kind,
            u32::try_from(row["asset_rate"].as_u64().unwrap()).unwrap(),
            u32::try_from(row["asset_channels"].as_u64().unwrap()).unwrap(),
            row["asset_frames"].as_u64().unwrap(),
        );
        let rate = u32::try_from(row["rate"].as_u64().unwrap()).unwrap();
        let gone = missing.path().join("gone.wav");
        let provider = MediaProvider::new(move |_| gone.clone(), None);
        let shared = provider.reader(&asset, rate);
        let reader = shared.lock().unwrap();
        assert_eq!(reader.frames(), row["frames"].as_u64().unwrap(), "{row}");
        assert_eq!(reader.channels() as u64, row["channels"].as_u64().unwrap());
        assert!(reader.is_missing());
        match &*reader {
            MediaReader::Silent(s) => assert_eq!(s.reason(), row["reason"]),
            other => panic!("{other:?}"),
        }
        assert_eq!(
            frames_at_rate(asset.frames, asset.sample_rate, rate),
            reader.frames()
        );
    }
}

#[test]
fn unreadable_media_is_silent_with_the_prototypes_length() {
    let golden = golden();
    let provider_json = json(&golden, "media/provider.json");
    let dir = tempfile::tempdir().unwrap();
    let bad = path(&golden, "fixtures/not-media.wav");
    for row in provider_json["unreadable"].as_array().unwrap() {
        let asset = asset_from(
            "med_bad",
            MediaKind::Audio,
            u32::try_from(row["asset_rate"].as_u64().unwrap()).unwrap(),
            2,
            1000,
        )
        .edit(|a| {
            a.format = row["format"].as_str().unwrap().to_owned();
            a.subtype = row["subtype"].as_str().unwrap().to_owned();
        })
        .unwrap();
        let bad = bad.clone();
        let provider = MediaProvider::new(move |_| bad.clone(), Some(dir.path().join("bad")));
        let rate = u32::try_from(row["rate"].as_u64().unwrap()).unwrap();
        let shared = provider.reader(&asset, rate);
        let reader = shared.lock().unwrap();
        assert!(reader.is_missing(), "{row}");
        assert_eq!(reader.frames(), row["frames"].as_u64().unwrap(), "{row}");
        assert_eq!(reader.channels() as u64, row["channels"].as_u64().unwrap());
        let MediaReader::Silent(silent) = &*reader else {
            panic!("not silent")
        };
        assert_eq!(
            silent.reason(),
            recorded_path(&golden, row["reason"].as_str().unwrap()),
            "{row}"
        );
    }
}

#[test]
fn load_full_matches() {
    let golden = golden();
    let provider_json = json(&golden, "media/provider.json");
    let file = path(&golden, "fixtures/ir-48k-stereo-float.wav");
    let ir = asset("med_ir48kstereofloat", &file, MediaKind::Impulse);
    for row in provider_json["load_full"].as_array().unwrap() {
        let case = row["case"].as_str().unwrap();
        let rate = u32::try_from(row["sample_rate"].as_u64().unwrap()).unwrap();
        let cache = tempfile::tempdir().unwrap();
        let source = if row["present"].as_bool().unwrap() {
            file.clone()
        } else {
            cache.path().join("missing.wav")
        };
        let provider = MediaProvider::new(move |_| source.clone(), Some(cache.path().into()));
        let full = provider
            .load_full(&ir, rate, row["max_seconds"].as_f64().unwrap())
            .unwrap();
        let shape = row["shape"].as_array().unwrap();
        assert_eq!(full.frames as u64, shape[0].as_u64().unwrap(), "{case}");
        assert_eq!(full.channels as u64, shape[1].as_u64().unwrap(), "{case}");
        let expected = f64s(&golden, &format!("media/provider/load_full-{case}"));
        if rate == 48_000 || !row["present"].as_bool().unwrap() {
            assert!(bits_equal(&full.samples, &expected), "{case}");
        } else {
            assert_soxr_parity(case, &full.samples, &expected);
        }
    }
}

#[test]
fn names_match_the_importer() {
    let golden = golden();
    let names = json(&golden, "media/names.json");
    for row in names["safe_stem"].as_array().unwrap() {
        let limit = usize::try_from(row["limit"].as_u64().unwrap()).unwrap();
        assert_eq!(
            safe_stem(row["name"].as_str().unwrap(), limit),
            row["safe_stem"],
            "{row}"
        );
    }
    for row in names["paths"].as_array().unwrap() {
        let source = row["source"].as_str().unwrap();
        assert_eq!(path_stem(source), row["stem"], "{row}");
        assert_eq!(path_suffix(source), row["suffix"], "{row}");
        let kind = MediaKind::parse(row["kind"].as_str().unwrap()).unwrap();
        assert_eq!(
            import_path("med_x", kind, source),
            row["copy_path"],
            "{row}"
        );
    }
    for row in names["recordings"].as_array().unwrap() {
        assert_eq!(
            recording_path("med_rec", row["name"].as_str().unwrap()),
            row["rel"],
            "{row}"
        );
    }
    for row in names["imports"].as_array().unwrap() {
        let file = path(&golden, row["fixture"].as_str().unwrap());
        let kind = MediaKind::parse(row["kind"].as_str().unwrap()).unwrap();
        if let Some(error) = row.get("error") {
            let err = inspect_audio(&file).expect_err("not media");
            assert_eq!(err.code.as_str(), error["code"]);
            assert_eq!(
                err.message,
                recorded_path(&golden, error["message"].as_str().unwrap())
            );
            continue;
        }
        let expected = &row["asset"];
        let info = inspect_audio(&file).unwrap();
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        if row["mode"] == "copy" {
            assert_eq!(import_path("med_import", kind, &name), expected["path"]);
        }
        assert_eq!(info.format, expected["format"]);
        assert_eq!(info.subtype, expected["subtype"]);
        assert_eq!(info.sample_rate, expected["sample_rate"]);
        assert_eq!(info.channels, expected["channels"]);
        assert_eq!(info.frames, expected["frames"]);
        assert_eq!(info.size_bytes, expected["size_bytes"]);
        let warnings: Vec<Value> = import_warnings(&name, &info)
            .into_iter()
            .map(Value::String)
            .collect();
        assert_eq!(Value::Array(warnings), row["warnings"], "{row}");
        let metadata: Map<String, Value> = info
            .metadata
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect();
        assert_eq!(Value::Object(metadata), expected["metadata"]);
    }
}
