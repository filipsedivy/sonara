//! The decode cache (`audio/engine/media.py`): media that cannot be read directly at the
//! project rate is decoded once, resampled with libsoxr VHQ, and stored as raw interleaved
//! little-endian `f32` in `cache/decoded/{asset}-{rate}.f32`, with a JSON sidecar
//! `{asset}-{rate}.json` holding `{"version": 1, "channels", "frames", "sample_rate",
//! "sha256"}`. The file layout and the sidecar are the prototype's, so a cache written by either
//! application is valid for the other.

use std::borrow::Cow;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::MediaError;
use crate::resample::ResampleStream;
use crate::stream::AudioStream;

/// The sidecar format version (`CACHE_VERSION`).
pub const CACHE_VERSION: u32 = 1;

/// Frames per decode block (`DECODE_CHUNK`); a shorter block marks the end of the input.
pub const DECODE_CHUNK: usize = 65_536;

/// The two files of one cache entry (`media.CacheInfo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheInfo {
    /// The raw `f32` data, `{asset}-{rate}.f32`.
    pub raw: PathBuf,
    /// The JSON sidecar, `{asset}-{rate}.json`.
    pub meta: PathBuf,
}

impl CacheInfo {
    /// The entry of `asset_id` at `sample_rate` in `cache_dir` (`media._cache_paths`).
    #[must_use]
    pub fn new(cache_dir: &Path, asset_id: &str, sample_rate: u32) -> Self {
        let stem = format!("{asset_id}-{sample_rate}");
        Self {
            raw: cache_dir.join(format!("{stem}.f32")),
            meta: cache_dir.join(format!("{stem}.json")),
        }
    }

    /// Whether the entry exists and was made for a file with `sha256`
    /// (`MediaProvider.cache_ready` for an asset that needs the cache): both files exist, the
    /// sidecar parses, its `version` equals 1 and its `sha256` equals the asset's (a missing or
    /// `null` value equals a missing hash).
    ///
    /// The comparisons are Python's: `version` matches 1, 1.0 and `true`. The frame and channel
    /// counts are not checked here. A sidecar that is valid JSON but not an object is not ready
    /// (the prototype raised `AttributeError` there).
    #[must_use]
    pub fn is_ready(&self, sha256: Option<&str>) -> bool {
        if !(self.raw.exists() && self.meta.exists()) {
            return false;
        }
        let Ok(text) = fs::read_to_string(&self.meta) else {
            return false;
        };
        let Ok(Value::Object(meta)) = serde_json::from_str::<Value>(&text) else {
            return false;
        };
        let version_ok = match meta.get("version") {
            Some(Value::Number(n)) => n.as_f64() == Some(f64::from(CACHE_VERSION)),
            Some(Value::Bool(b)) => *b,
            _ => false,
        };
        let sha_ok = match (meta.get("sha256"), sha256) {
            (None | Some(Value::Null), None) => true,
            (Some(Value::String(stored)), Some(expected)) => stored == expected,
            _ => false,
        };
        version_ok && sha_ok
    }

    /// The channel and frame counts from the sidecar (`int(meta["channels"])`,
    /// `int(meta["frames"])`).
    ///
    /// # Errors
    ///
    /// [`MediaError::Io`] when the sidecar cannot be read, and [`MediaError::Cache`] when it is
    /// not JSON or lacks a non-negative integer `channels` or `frames`.
    pub fn read_layout(&self) -> Result<(usize, u64), MediaError> {
        let text =
            fs::read_to_string(&self.meta).map_err(|e| MediaError::io("read", &self.meta, e))?;
        let cache_error = |message: &str| MediaError::Cache {
            path: self.meta.clone(),
            message: message.to_owned(),
        };
        let meta: Value = serde_json::from_str(&text).map_err(|_| cache_error("not JSON"))?;
        let count = |key: &str| {
            meta.get(key)
                .and_then(Value::as_u64)
                .ok_or_else(|| cache_error(&format!("no integer {key}")))
        };
        let channels = usize::try_from(count("channels")?)
            .ok()
            .filter(|&c| c > 0)
            .ok_or_else(|| cache_error("invalid channel count"))?;
        Ok((channels, count("frames")?))
    }
}

/// The sidecar text exactly as the prototype writes it (`json.dumps` with its default
/// separators and key order).
fn sidecar_text(channels: usize, frames: u64, sample_rate: u32, sha256: Option<&str>) -> String {
    let sha = sha256.map_or_else(|| "null".to_owned(), |s| Value::from(s).to_string());
    format!(
        "{{\"version\": {CACHE_VERSION}, \"channels\": {channels}, \"frames\": {frames}, \
         \"sample_rate\": {sample_rate}, \"sha256\": {sha}}}"
    )
}

/// Decodes `source`, resamples it to `sample_rate` when its rate differs, and writes the cache
/// entry `target` (`media.decode_to_cache`). Returns `(channels, frames)`.
///
/// The source is read in [`DECODE_CHUNK`]-frame blocks of `f32` (libsndfile's `float` read);
/// the first shorter block, possibly empty, is the last one and flushes the resampler. The data
/// goes to `{raw}.tmp` and is renamed into place before the sidecar is written. Call it from a
/// worker thread; `cancelled` is polled before every block.
///
/// # Errors
///
/// [`MediaError::Cancelled`] when `cancelled` returns `true`, and the errors of opening,
/// decoding, resampling and writing. The temporary file is removed on every error.
pub fn decode_to_cache(
    source: &Path,
    target: &CacheInfo,
    sample_rate: u32,
    expected_sha256: Option<&str>,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<(usize, u64), MediaError> {
    if let Some(dir) = target.raw.parent() {
        fs::create_dir_all(dir).map_err(|e| MediaError::io("create", dir, e))?;
    }
    let tmp = target.raw.with_extension("f32.tmp");
    let mut stream = AudioStream::open(source)?.stream;
    stream.check_decodable()?;
    let result = write_decoded(&mut stream, &tmp, sample_rate, cancelled);
    let (channels, frames) = match result {
        Ok(layout) => layout,
        Err(err) => {
            // Best effort: the error being reported is the decode failure, not the cleanup.
            let _ = fs::remove_file(&tmp);
            return Err(err);
        }
    };
    fs::rename(&tmp, &target.raw).map_err(|e| MediaError::io("rename", &tmp, e))?;
    fs::write(
        &target.meta,
        sidecar_text(channels, frames, sample_rate, expected_sha256),
    )
    .map_err(|e| MediaError::io("write", &target.meta, e))?;
    Ok((channels, frames))
}

fn write_decoded(
    stream: &mut AudioStream,
    tmp: &Path,
    sample_rate: u32,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<(usize, u64), MediaError> {
    let channels = stream.channels();
    let file = File::create(tmp).map_err(|e| MediaError::io("create", tmp, e))?;
    let mut out = BufWriter::new(file);
    let mut resampler = if stream.sample_rate() == sample_rate {
        None
    } else {
        Some(ResampleStream::new(
            stream.sample_rate(),
            sample_rate,
            channels,
        )?)
    };
    let mut decoded = vec![0.0f64; DECODE_CHUNK * channels];
    let mut block: Vec<f32> = Vec::with_capacity(DECODE_CHUNK * channels);
    let mut bytes = Vec::new();
    let mut frames = 0u64;
    loop {
        if cancelled.is_some_and(|f| f()) {
            return Err(MediaError::Cancelled);
        }
        let read = stream.read(&mut decoded)?;
        let last = read < DECODE_CHUNK;
        block.clear();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "libsndfile's float read: the f64 value rounded to f32 (exact for <= 24 bits)"
        )]
        block.extend(decoded[..read * channels].iter().map(|&x| x as f32));
        let samples: Cow<'_, [f32]> = match &mut resampler {
            Some(r) => Cow::Owned(r.resample_chunk(&block, last)?),
            None => Cow::Borrowed(&block),
        };
        if !samples.is_empty() {
            bytes.clear();
            bytes.extend(samples.iter().flat_map(|s| s.to_le_bytes()));
            out.write_all(&bytes)
                .map_err(|e| MediaError::io("write", tmp, e))?;
            frames += (samples.len() / channels) as u64;
        }
        if last {
            break;
        }
    }
    out.flush().map_err(|e| MediaError::io("write", tmp, e))?;
    Ok((channels, frames))
}

#[cfg(test)]
mod tests {
    use super::{CacheInfo, decode_to_cache, sidecar_text};
    use crate::error::MediaError;

    #[test]
    fn entries_are_named_by_asset_and_rate() {
        let info = CacheInfo::new(std::path::Path::new("/p/cache/decoded"), "med_a1", 48_000);
        assert_eq!(info.raw.to_str(), Some("/p/cache/decoded/med_a1-48000.f32"));
        assert_eq!(
            info.meta.to_str(),
            Some("/p/cache/decoded/med_a1-48000.json")
        );
    }

    #[test]
    fn the_sidecar_is_written_like_python_json_dumps() {
        assert_eq!(
            sidecar_text(2, 72_000, 48_000, Some("ab12")),
            r#"{"version": 1, "channels": 2, "frames": 72000, "sample_rate": 48000, "sha256": "ab12"}"#
        );
        assert_eq!(
            sidecar_text(1, 0, 44_100, None),
            r#"{"version": 1, "channels": 1, "frames": 0, "sample_rate": 44100, "sha256": null}"#
        );
    }

    /// A 16-bit mono WAV of `frames` frames at 44.1 kHz.
    fn wav(path: &std::path::Path, frames: u16) {
        let data = u32::from(frames) * 2;
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&(36 + data).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        for field in [1u16, 1] {
            bytes.extend_from_slice(&field.to_le_bytes());
        }
        bytes.extend_from_slice(&44_100u32.to_le_bytes());
        bytes.extend_from_slice(&88_200u32.to_le_bytes());
        for field in [2u16, 16] {
            bytes.extend_from_slice(&field.to_le_bytes());
        }
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data.to_le_bytes());
        for n in 0..frames {
            bytes.extend_from_slice(&(n.wrapping_mul(97).cast_signed()).to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn cancelling_or_failing_leaves_no_entry_behind() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("in.wav");
        wav(&source, 100);
        let target = CacheInfo::new(&dir.path().join("cache"), "med_c", 48_000);
        let cancelled = || true;
        let err = decode_to_cache(&source, &target, 48_000, None, Some(&cancelled)).unwrap_err();
        assert!(matches!(err, MediaError::Cancelled), "{err}");
        let not_media = dir.path().join("text.wav");
        std::fs::write(&not_media, b"plain text").unwrap();
        let err = decode_to_cache(&not_media, &target, 48_000, None, None).unwrap_err();
        assert!(matches!(err, MediaError::NotRecognised { .. }), "{err}");
        let err =
            decode_to_cache(&dir.path().join("none.wav"), &target, 48_000, None, None).unwrap_err();
        assert!(matches!(err, MediaError::Io { .. }), "{err}");
        let left: Vec<_> = std::fs::read_dir(dir.path().join("cache"))
            .unwrap()
            .collect();
        assert!(left.is_empty(), "{left:?}");
        assert!(!target.is_ready(None));
    }

    #[test]
    fn an_entry_at_the_source_rate_holds_the_decoded_samples() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("in.wav");
        wav(&source, 100);
        let target = CacheInfo::new(dir.path(), "med_s", 44_100);
        assert_eq!(
            decode_to_cache(&source, &target, 44_100, Some("ab"), None).unwrap(),
            (1, 100)
        );
        assert!(target.is_ready(Some("ab")));
        assert_eq!(target.read_layout().unwrap(), (1, 100));
        let raw = std::fs::read(&target.raw).unwrap();
        let samples: Vec<f32> = raw
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        let expected: Vec<f32> = (0..100u16)
            .map(|n| f32::from(n.wrapping_mul(97).cast_signed()) / 32_768.0)
            .collect();
        assert_eq!(samples, expected);
    }

    #[test]
    fn validity_follows_the_prototypes_comparisons() {
        let dir = tempfile::tempdir().unwrap();
        let info = CacheInfo::new(dir.path(), "med_v", 48_000);
        let sha = "ab".repeat(32);
        assert!(!info.is_ready(Some(&sha)), "no files");
        std::fs::write(&info.raw, [0u8; 8]).unwrap();
        for (sidecar, expected, ready) in [
            (
                format!(r#"{{"version": 1, "sha256": "{sha}"}}"#),
                Some(sha.as_str()),
                true,
            ),
            (
                format!(r#"{{"version": 1.0, "sha256": "{sha}"}}"#),
                Some(sha.as_str()),
                true,
            ),
            (
                format!(r#"{{"version": true, "sha256": "{sha}"}}"#),
                Some(sha.as_str()),
                true,
            ),
            (
                format!(r#"{{"version": 2, "sha256": "{sha}"}}"#),
                Some(sha.as_str()),
                false,
            ),
            (r#"{"version": 1}"#.to_owned(), None, true),
            (r#"{"version": 1, "sha256": null}"#.to_owned(), None, true),
            (r#"{"version": 1}"#.to_owned(), Some(sha.as_str()), false),
            (
                format!(r#"{{"version": 1, "sha256": "{sha}"}}"#),
                None,
                false,
            ),
            ("[1]".to_owned(), None, false),
            ("{not json".to_owned(), None, false),
        ] {
            std::fs::write(&info.meta, &sidecar).unwrap();
            assert_eq!(info.is_ready(expected), ready, "{sidecar} {expected:?}");
        }
    }
}
