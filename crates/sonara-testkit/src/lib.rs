//! Test support: loads the golden data recorded from the original Python prototype (`golden/`) and
//! compares results with it: canonical JSON, relative error, null-test residual in dBFS.
//! Used only as a dev-dependency.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

/// Environment variable that points at a golden directory other than `<repo>/golden`.
pub const GOLDEN_ENV: &str = "SONARA_GOLDEN";
const LFS_POINTER_PREFIX: &[u8] = b"version https://git-lfs.github.com/spec/v1";

/// Errors while reading golden data.
#[derive(Debug, thiserror::Error)]
pub enum GoldenError {
    /// No golden directory was found.
    #[error("golden data not found: run `git lfs pull` or set {GOLDEN_ENV}")]
    NotFound,
    /// A file could not be read.
    #[error("cannot read {path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: std::io::Error,
    },
    /// A JSON file did not parse into the requested type.
    #[error("invalid JSON in {path}: {source}")]
    Json {
        /// The file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: serde_json::Error,
    },
    /// The file is a Git LFS pointer, not its content.
    #[error("{path} is a Git LFS pointer; run `git lfs pull`")]
    LfsPointer {
        /// The file.
        path: PathBuf,
    },
    /// The file's content does not match its declared format.
    #[error("{path}: {message}")]
    Format {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        message: String,
    },
    /// A WAV file could not be decoded.
    #[error("cannot read WAV {path}: {source}")]
    Wav {
        /// The file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: hound::Error,
    },
}

/// `golden/MANIFEST.json`: provenance and the SHA-256 of every recorded file.
#[derive(Debug, Deserialize)]
pub struct Manifest {
    /// Format version of the golden data.
    pub format: u32,
    /// SHA-256 over the prototype's sources at recording time.
    pub source_sha256: String,
    /// The wall-clock time the recorder pinned.
    pub frozen_clock: String,
    /// Platform and library versions of the recording machine.
    pub environment: serde_json::Value,
    /// Per-area summaries.
    pub areas: serde_json::Value,
    /// Relative path → SHA-256 of every recorded file.
    pub files: BTreeMap<String, String>,
}

/// A float64 array recorded as `<name>.f64` with its `<name>.json` header.
#[derive(Debug, Clone, PartialEq)]
pub struct F64Array {
    /// Dimensions in C order.
    pub shape: Vec<usize>,
    /// The values, row-major.
    pub data: Vec<f64>,
    /// The header, including the recorder's metadata (parameters, states).
    pub header: serde_json::Value,
}

/// A float32 array recorded as `<name>.f32` with its `<name>.json` header: the layout of
/// `media/decoded32/`, also used for data the prototype holds as float32 (the played blocks of
/// the `transport` area).
#[derive(Debug, Clone, PartialEq)]
pub struct F32Array {
    /// Dimensions in C order.
    pub shape: Vec<usize>,
    /// The values, row-major.
    pub data: Vec<f32>,
    /// The header, including the recorder's metadata (for example each block's tag).
    pub header: serde_json::Value,
}

/// A decoded 32-bit float WAV.
#[derive(Debug, Clone, PartialEq)]
pub struct Audio {
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Number of channels.
    pub channels: u16,
    /// Interleaved samples.
    pub samples: Vec<f32>,
}

/// The golden data directory.
#[derive(Debug)]
pub struct Golden {
    root: PathBuf,
    manifest: Manifest,
}

impl Golden {
    /// Opens `$SONARA_GOLDEN`, or the `golden/` directory of the repository that contains this
    /// crate.
    ///
    /// # Errors
    ///
    /// [`GoldenError::NotFound`] when no directory with a `MANIFEST.json` exists, or any error
    /// from reading the manifest.
    pub fn open() -> Result<Self, GoldenError> {
        if let Some(dir) = std::env::var_os(GOLDEN_ENV) {
            return Self::at(dir);
        }
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = crate_dir
            .ancestors()
            .map(|dir| dir.join("golden"))
            .find(|dir| dir.join("MANIFEST.json").is_file())
            .ok_or(GoldenError::NotFound)?;
        Self::at(root)
    }

    /// Opens the golden directory at `root`.
    ///
    /// # Errors
    ///
    /// Any error from reading or parsing `root/MANIFEST.json`.
    pub fn at(root: impl Into<PathBuf>) -> Result<Self, GoldenError> {
        let root = root.into();
        let path = root.join("MANIFEST.json");
        if !path.is_file() {
            return Err(GoldenError::NotFound);
        }
        let manifest = parse_json(&path, &read(&path)?)?;
        Ok(Self { root, manifest })
    }

    /// The golden directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The parsed manifest.
    #[must_use]
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The raw bytes of a recorded file.
    ///
    /// # Errors
    ///
    /// [`GoldenError::Io`], or [`GoldenError::LfsPointer`] when Git LFS content was not pulled.
    pub fn bytes(&self, rel: &str) -> Result<Vec<u8>, GoldenError> {
        let path = self.root.join(rel);
        let data = read(&path)?;
        if data.starts_with(LFS_POINTER_PREFIX) {
            return Err(GoldenError::LfsPointer { path });
        }
        Ok(data)
    }

    /// A JSON file, deserialized.
    ///
    /// # Errors
    ///
    /// Any error from [`Golden::bytes`], or [`GoldenError::Json`].
    pub fn json<T: DeserializeOwned>(&self, rel: &str) -> Result<T, GoldenError> {
        parse_json(&self.root.join(rel), &self.bytes(rel)?)
    }

    /// The decompressed bytes of a gzip-compressed file (`*.gz`).
    ///
    /// # Errors
    ///
    /// Any error from [`Golden::bytes`], or a decompression error.
    pub fn bytes_gz(&self, rel: &str) -> Result<Vec<u8>, GoldenError> {
        let mut data = Vec::new();
        GzDecoder::new(self.bytes(rel)?.as_slice())
            .read_to_end(&mut data)
            .map_err(|source| GoldenError::Io {
                path: self.root.join(rel),
                source,
            })?;
        Ok(data)
    }

    /// A gzip-compressed JSON file (`*.json.gz`, such as project snapshots), deserialized.
    ///
    /// # Errors
    ///
    /// Any error from [`Golden::bytes_gz`], or [`GoldenError::Json`].
    pub fn json_gz<T: DeserializeOwned>(&self, rel: &str) -> Result<T, GoldenError> {
        parse_json(&self.root.join(rel), &self.bytes_gz(rel)?)
    }

    /// A float64 array: `<rel>.json` (header) and `<rel>.f64` (little-endian values).
    ///
    /// # Errors
    ///
    /// Read or parse errors, or [`GoldenError::Format`] when the data does not match the shape.
    pub fn f64_array(&self, rel: &str) -> Result<F64Array, GoldenError> {
        let (shape, bytes, header) = self.raw_array(rel, "f64", "f64le", 8)?;
        let (words, _) = bytes.as_chunks::<8>();
        let data = words.iter().map(|word| f64::from_le_bytes(*word)).collect();
        Ok(F64Array {
            shape,
            data,
            header,
        })
    }

    /// A float32 array: `<rel>.json` (header, dtype `f32le`) and `<rel>.f32` (little-endian
    /// values), the header convention of `media/decoded32/`.
    ///
    /// # Errors
    ///
    /// Read or parse errors, or [`GoldenError::Format`] when the data does not match the shape.
    pub fn f32_array(&self, rel: &str) -> Result<F32Array, GoldenError> {
        let (shape, bytes, header) = self.raw_array(rel, "f32", "f32le", 4)?;
        let (words, _) = bytes.as_chunks::<4>();
        let data = words.iter().map(|word| f32::from_le_bytes(*word)).collect();
        Ok(F32Array {
            shape,
            data,
            header,
        })
    }

    /// The shape, raw bytes and header of `<rel>.<extension>` with the header `<rel>.json`
    /// declaring `dtype`, checked against `width`-byte values.
    fn raw_array(
        &self,
        rel: &str,
        extension: &str,
        dtype: &str,
        width: usize,
    ) -> Result<(Vec<usize>, Vec<u8>, serde_json::Value), GoldenError> {
        let header: serde_json::Value = self.json(&format!("{rel}.json"))?;
        let data_rel = format!("{rel}.{extension}");
        let path = self.root.join(&data_rel);
        let format_error = |message: String| GoldenError::Format {
            path: path.clone(),
            message,
        };
        if header.get("dtype").and_then(serde_json::Value::as_str) != Some(dtype) {
            return Err(format_error(format!("header dtype is not {dtype}")));
        }
        let shape: Vec<usize> = header
            .get("shape")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format_error("header has no shape".to_owned()))?
            .iter()
            .map(|d| {
                d.as_u64()
                    .and_then(|d| usize::try_from(d).ok())
                    .ok_or_else(|| format_error(format!("bad dimension {d}")))
            })
            .collect::<Result<_, _>>()?;
        let bytes = self.bytes(&data_rel)?;
        let expected: usize = shape.iter().product();
        if bytes.len() != expected * width {
            return Err(format_error(format!(
                "{} bytes for shape {shape:?}",
                bytes.len()
            )));
        }
        Ok((shape, bytes, header))
    }

    /// A 32-bit float WAV file.
    ///
    /// # Errors
    ///
    /// Read errors, [`GoldenError::Wav`], or [`GoldenError::Format`] for a non-float WAV.
    pub fn wav(&self, rel: &str) -> Result<Audio, GoldenError> {
        let path = self.root.join(rel);
        let bytes = self.bytes(rel)?;
        let reader =
            hound::WavReader::new(bytes.as_slice()).map_err(|source| GoldenError::Wav {
                path: path.clone(),
                source,
            })?;
        let spec = reader.spec();
        if spec.sample_format != hound::SampleFormat::Float || spec.bits_per_sample != 32 {
            return Err(GoldenError::Format {
                path,
                message: "expected 32-bit float samples".to_owned(),
            });
        }
        let samples = reader
            .into_samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| GoldenError::Wav { path, source })?;
        Ok(Audio {
            sample_rate: spec.sample_rate,
            channels: spec.channels,
            samples,
        })
    }

    /// Checks every file against its SHA-256 in the manifest.
    ///
    /// Returns the relative paths whose content differs; an empty list means the golden data is
    /// intact.
    ///
    /// # Errors
    ///
    /// A file that cannot be read, or one that is still a Git LFS pointer.
    pub fn verify(&self) -> Result<Vec<String>, GoldenError> {
        let mut differing = Vec::new();
        for (rel, expected) in &self.manifest.files {
            if sha256_hex(&self.bytes(rel)?) != *expected {
                differing.push(rel.clone());
            }
        }
        Ok(differing)
    }
}

/// Lowercase hex SHA-256 of `data`.
#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
            hex
        })
}

/// The largest relative error `|a − e| / max(|e|, floor)` over two equally long slices.
///
/// `floor` keeps values near zero from dominating; the kernels use `1e-300`.
///
/// # Panics
///
/// When the slices differ in length.
#[must_use]
pub fn max_relative_error(expected: &[f64], actual: &[f64], floor: f64) -> f64 {
    assert_eq!(expected.len(), actual.len(), "length mismatch");
    expected
        .iter()
        .zip(actual)
        .map(|(e, a)| (a - e).abs() / e.abs().max(floor))
        .fold(0.0, f64::max)
}

/// The null-test residual: the peak of `actual − expected` in dBFS, or negative infinity when
/// they are identical.
///
/// # Panics
///
/// When the slices differ in length.
#[must_use]
pub fn peak_residual_dbfs(expected: &[f32], actual: &[f32]) -> f64 {
    assert_eq!(expected.len(), actual.len(), "length mismatch");
    let peak = expected
        .iter()
        .zip(actual)
        .map(|(e, a)| (f64::from(*a) - f64::from(*e)).abs())
        .fold(0.0, f64::max);
    if peak == 0.0 {
        f64::NEG_INFINITY
    } else {
        20.0 * peak.log10()
    }
}

fn read(path: &Path) -> Result<Vec<u8>, GoldenError> {
    fs::read(path).map_err(|source| GoldenError::Io {
        path: path.to_owned(),
        source,
    })
}

fn parse_json<T: DeserializeOwned>(path: &Path, data: &[u8]) -> Result<T, GoldenError> {
    serde_json::from_slice(data).map_err(|source| GoldenError::Json {
        path: path.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{Golden, GoldenError, max_relative_error, peak_residual_dbfs, sha256_hex};

    /// A golden directory with one array of each width, in a temporary directory.
    fn arrays() -> (tempfile::TempDir, Golden) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("MANIFEST.json"),
            br#"{"format": 1, "source_sha256": "",
            "frozen_clock": "", "environment": {}, "areas": {}, "files": {}}"#,
        )
        .unwrap();
        let values: Vec<u8> = [0.5_f32, -0.25, 1.0, 3.0, -2.0, 0.125]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        std::fs::write(dir.path().join("blocks.f32"), values).unwrap();
        std::fs::write(
            dir.path().join("blocks.json"),
            br#"{"dtype": "f32le", "shape": [3, 2], "blocks": [{"tag": 0}]}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("short.f32"), [0_u8; 6]).unwrap();
        std::fs::write(
            dir.path().join("short.json"),
            br#"{"dtype": "f32le", "shape": [2]}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("wide.f64"), 1.5_f64.to_le_bytes()).unwrap();
        std::fs::write(
            dir.path().join("wide.json"),
            br#"{"dtype": "f64le", "shape": [1]}"#,
        )
        .unwrap();
        let golden = Golden::at(dir.path()).unwrap();
        (dir, golden)
    }

    #[test]
    fn f32_arrays_follow_the_decoded32_layout() {
        let (_dir, golden) = arrays();
        let array = golden.f32_array("blocks").unwrap();
        assert_eq!(array.shape, [3, 2]);
        assert_eq!(array.data, [0.5, -0.25, 1.0, 3.0, -2.0, 0.125]);
        assert_eq!(array.header["blocks"][0]["tag"], 0);
        assert_eq!(golden.f64_array("wide").unwrap().data, [1.5]);
        // The wrong width, a short file and a missing file are errors.
        assert!(matches!(
            golden.f32_array("wide"),
            Err(GoldenError::Format { .. })
        ));
        assert!(matches!(
            golden.f32_array("short"),
            Err(GoldenError::Format { .. })
        ));
        assert!(matches!(
            golden.f64_array("blocks"),
            Err(GoldenError::Format { .. })
        ));
    }

    #[test]
    fn sha256_matches_a_known_digest() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn identical_signals_have_no_residual() {
        assert_eq!(
            peak_residual_dbfs(&[0.5, -0.25], &[0.5, -0.25]),
            f64::NEG_INFINITY
        );
    }

    #[test]
    fn residual_is_reported_in_dbfs() {
        let residual = peak_residual_dbfs(&[0.0, 0.0], &[0.0, 0.001]);
        assert!((residual - (-60.0)).abs() < 1e-3, "{residual}");
    }

    #[test]
    fn relative_error_uses_the_floor_near_zero() {
        assert!((max_relative_error(&[2.0, 0.0], &[2.002, 1e-12], 1e-9) - 1e-3).abs() < 1e-12);
    }
}
