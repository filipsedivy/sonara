//! The recorded golden data is intact and every file parses.

use std::path::Path;

use sonara_testkit::Golden;

fn has_extension(rel: &str, extension: &str) -> bool {
    Path::new(rel).extension().is_some_and(|e| e == extension)
}

#[test]
fn every_golden_file_matches_its_manifest_hash() {
    let golden = Golden::open().expect("golden data");
    assert_ne!(
        golden.manifest().files.len(),
        0,
        "the manifest lists no files"
    );
    let differing = golden.verify().expect("readable golden data");
    assert_eq!(
        differing,
        Vec::<String>::new(),
        "files differ from the manifest"
    );
}

#[test]
fn every_vector_matches_its_header() {
    let golden = Golden::open().expect("golden data");
    let vectors: Vec<&String> = golden
        .manifest()
        .files
        .keys()
        .filter(|rel| has_extension(rel, "f64"))
        .collect();
    assert_ne!(vectors.len(), 0, "no vectors recorded");
    for rel in vectors {
        let base = rel.trim_end_matches(".f64");
        let array = golden.f64_array(base).expect("vector");
        assert_eq!(
            array.data.len(),
            array.shape.iter().product::<usize>(),
            "{rel}"
        );
        // `numpy/math/` records NumPy's reductions and rounding on deliberately awkward inputs,
        // NaN and infinities included; every other vector is finite.
        if !rel.starts_with("numpy/math/") {
            assert!(
                array.data.iter().all(|v| v.is_finite()),
                "{rel} has non-finite values"
            );
        }
    }
}

/// Renders are 32-bit float stereo. Exported files (`*/outputs/`), the media fixtures
/// (`fixtures/`: inputs in other formats, and one deliberately invalid `.wav`) and the media
/// decoding inputs (`media/inputs/`) are not renders.
#[test]
fn every_render_is_a_float_wav() {
    let golden = Golden::open().expect("golden data");
    let renders: Vec<&String> = golden
        .manifest()
        .files
        .keys()
        .filter(|rel| {
            has_extension(rel, "wav")
                && !rel.contains("/outputs/")
                && !rel.starts_with("fixtures/")
                && !rel.starts_with("media/inputs/")
        })
        .collect();
    assert_ne!(renders.len(), 0, "no renders recorded");
    for rel in renders {
        let audio = golden.wav(rel).expect("wav");
        assert_eq!(audio.channels, 2, "{rel}");
        assert!(audio.sample_rate > 0, "{rel}");
        assert!(
            audio.samples.iter().all(|v| v.is_finite()),
            "{rel} has non-finite samples"
        );
    }
}

#[test]
fn every_json_file_parses() {
    let golden = Golden::open().expect("golden data");
    for rel in golden.manifest().files.keys() {
        if has_extension(rel, "json") {
            let _: serde_json::Value = golden.json(rel).expect("json");
        } else if has_extension(rel, "gz") {
            let _: serde_json::Value = golden.json_gz(rel).expect("json.gz");
        }
    }
}
