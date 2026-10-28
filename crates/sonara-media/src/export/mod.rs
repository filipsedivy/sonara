//! Audio export: the prototype's `import_export/audio_export.py`, the
//! encoding and writing half; the render belongs to `sonara-engine` and the service to
//! `sonara-app`.
//!
//! [`Exporter::export`] runs the prototype's two passes over a stream of rendered blocks
//! (interleaved stereo `f64`):
//!
//! 1. **Render pass:** every block is fed to the [`Analyzer`] and stored in a temporary
//!    `f64` file (the prototype uses an RF64 `DOUBLE` file; the intermediate is internal).
//! 2. **Encoding pass**, per output: 65536-frame chunks of the intermediate get the
//!    normalization gain `10^(gain_db / 20)` (skipped when it is 1), a mono mix `(l + r) / 2`,
//!    libsoxr VHQ resampling in `f64` ([`crate::ResampleStream64`]), then quantization with
//!    TPDF dither (integer PCM) or a cast to `f32` (float and lossy outputs; `f64` for
//!    `DOUBLE`), and are written. The file is written as `.<name>.partial` and renamed at the
//!    end; a BWF output gets its `bext` chunk from an analysis of the written file; every
//!    output is read back and checked ([`OutputResult::verified`]).
//!
//! Normalization (`_gain`) is a pure gain: none, sample peak, true peak, or integrated
//! loudness capped at `ceiling − true peak` with a warning; a silent input skips it. The final
//! loudness and true peak are the analysis plus the gain, not measured again.
//!
//! **Formats** ([`Container`]): WAV, BWF, RF64, Wave64, AIFF and CAF through the own writer
//! ([`PcmFileWriter`], byte-identical to libsndfile 1.2.2), FLAC through libFLAC 1.4.3 and MP3
//! through LAME, both loaded at run time (R2 and the FLAC decision of 2026-10-22), Ogg Vorbis
//! through `vorbis_rs` and Ogg Opus through libopus (the `opus` crate). The FLAC and lossy
//! encoders reproduce how libsndfile configures and drives the codecs, and the metadata strings
//! follow libsndfile's rules for each container (`metadata_written`).

mod bext;
mod error;
mod flac;
mod mp3;
mod ogg;
mod pcm;
mod quantize;
mod strings;
mod verify;

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};
use sonara_core::pyfmt::float_g;
use sonara_dsp::analysis::{AnalysisReport, Analyzer};
use sonara_dsp::numpy::math::py_round;
use tempfile::NamedTempFile;

use crate::codec_ffi::{FlacLibrary, LameLibrary};
use crate::format::Subtype;
use crate::resample::ResampleStream64;

pub use bext::{BextInfo, read_bext};
pub use error::ExportError;
pub use pcm::{PcmContainer, PcmFileWriter, SampleFormat};

use bext::{BextLoudness, bext_chunk, insert_bext};
use flac::FlacWriter;
use mp3::Mp3Writer;
use ogg::{OpusWriter, VorbisWriter};
use quantize::Quantizer;
use strings::Strings;

/// Frames per chunk of the encoding pass (`audio_export.CHUNK`); dither and resampling depend
/// on these boundaries.
pub const CHUNK: usize = 65536;

/// A source of the wall-clock time: the `PEAK` chunks' time stamps, the `bext` date and time,
/// and the default Ogg stream serial. Tests inject a fixed clock.
pub type ExportClock = Arc<dyn Fn() -> SystemTime + Send + Sync>;

/// The system clock.
#[must_use]
pub fn system_clock() -> ExportClock {
    Arc::new(SystemTime::now)
}

/// An export container (the prototype's `Container`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Container {
    /// RIFF WAVE.
    Wav,
    /// Broadcast WAV: WAV with a `bext` chunk.
    Bwf,
    /// RF64.
    Rf64,
    /// Sony Wave64.
    W64,
    /// AIFF (AIFF-C for float).
    Aiff,
    /// Core Audio Format.
    Caf,
    /// FLAC.
    Flac,
    /// MPEG-1/2 layer III.
    Mp3,
    /// Ogg Vorbis.
    Ogg,
    /// Ogg Opus.
    Opus,
}

/// The `(compression_level, kbps)` tables the prototype measured (`FORMATS`).
const MP3_CBR: &[(f64, f64)] = &[
    (0.0, 320.0),
    (0.25, 256.0),
    (0.5, 160.0),
    (0.75, 96.0),
    (0.9, 56.0),
    (0.99, 32.0),
];
const MP3_VBR: &[(f64, f64)] = &[
    (0.0, 268.0),
    (0.1, 214.0),
    (0.25, 179.0),
    (0.5, 126.0),
    (0.75, 104.0),
    (0.9, 79.0),
    (0.99, 64.0),
];
const OGG_VBR: &[(f64, f64)] = &[
    (0.0, 560.0),
    (0.1, 364.0),
    (0.25, 255.0),
    (0.5, 171.0),
    (0.75, 105.0),
    (0.9, 85.0),
    (0.99, 70.0),
];
const OPUS_VBR: &[(f64, f64)] = &[
    (0.0, 514.0),
    (0.1, 466.0),
    (0.25, 390.0),
    (0.5, 265.0),
    (0.75, 139.0),
    (0.9, 64.0),
    (0.99, 14.0),
];
const MP3_RATES: &[u32] = &[
    8000, 11_025, 12_000, 16_000, 22_050, 24_000, 32_000, 44_100, 48_000,
];
const OPUS_RATES: &[u32] = &[8000, 12_000, 16_000, 24_000, 48_000];

impl Container {
    /// Every container, in the prototype's order.
    pub const ALL: [Self; 10] = [
        Self::Wav,
        Self::Bwf,
        Self::Rf64,
        Self::W64,
        Self::Aiff,
        Self::Caf,
        Self::Flac,
        Self::Mp3,
        Self::Ogg,
        Self::Opus,
    ];

    /// The prototype's value, for example `"wav"`.
    #[must_use]
    pub const fn value(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Bwf => "bwf",
            Self::Rf64 => "rf64",
            Self::W64 => "w64",
            Self::Aiff => "aiff",
            Self::Caf => "caf",
            Self::Flac => "flac",
            Self::Mp3 => "mp3",
            Self::Ogg => "ogg",
            Self::Opus => "opus",
        }
    }

    /// The file extension with its dot (`FORMATS[c].extension`).
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Wav | Self::Bwf | Self::Rf64 => ".wav",
            Self::W64 => ".w64",
            Self::Aiff => ".aiff",
            Self::Caf => ".caf",
            Self::Flac => ".flac",
            Self::Mp3 => ".mp3",
            Self::Ogg => ".ogg",
            Self::Opus => ".opus",
        }
    }

    /// Whether the format is lossy (MP3, Vorbis, Opus).
    #[must_use]
    pub const fn is_lossy(self) -> bool {
        matches!(self, Self::Mp3 | Self::Ogg | Self::Opus)
    }

    /// The integer bit depths the container writes.
    #[must_use]
    pub const fn pcm_depths(self) -> &'static [u32] {
        match self {
            Self::Flac => &[16, 24],
            Self::Mp3 | Self::Ogg | Self::Opus => &[],
            _ => &[16, 24, 32],
        }
    }

    /// Whether the container writes float samples.
    #[must_use]
    pub const fn float_ok(self) -> bool {
        matches!(
            self,
            Self::Wav | Self::Bwf | Self::Rf64 | Self::W64 | Self::Aiff | Self::Caf
        )
    }

    /// The sample rates the format allows; `None` for any.
    #[must_use]
    pub const fn rates(self) -> Option<&'static [u32]> {
        match self {
            Self::Mp3 => Some(MP3_RATES),
            Self::Opus => Some(OPUS_RATES),
            _ => None,
        }
    }

    const fn cbr_table(self) -> &'static [(f64, f64)] {
        match self {
            Self::Mp3 => MP3_CBR,
            _ => &[],
        }
    }

    const fn vbr_table(self) -> &'static [(f64, f64)] {
        match self {
            Self::Mp3 => MP3_VBR,
            Self::Ogg => OGG_VBR,
            Self::Opus => OPUS_VBR,
            _ => &[],
        }
    }

    /// Whether this build can write the container (`sf_supported_containers`): every one
    /// except MP3 and FLAC, which need libmp3lame and libFLAC at run time.
    #[must_use]
    pub fn is_available(self) -> bool {
        match self {
            Self::Mp3 => LameLibrary::shared().is_ok(),
            Self::Flac => FlacLibrary::shared().is_ok(),
            _ => true,
        }
    }
}

impl FromStr for Container {
    type Err = ExportError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|c| c.value() == s)
            .ok_or_else(|| ExportError::InvalidArgument(format!("unknown container {s}")))
    }
}

/// The version of the loaded MP3 encoder (LAME's `get_lame_version`, for example `"3.100"`).
///
/// # Errors
///
/// [`ExportError::Codec`] when libmp3lame cannot be loaded.
pub fn mp3_encoder_version() -> Result<String, ExportError> {
    Ok(LameLibrary::shared()?.version())
}

/// Analyzes an audio file as the prototype's `_analyze_file` and the render service's
/// `analyze_file` do: the decoded file in 65536-frame blocks of `float64`, with libsndfile's
/// integer scaling. Reads every container this crate decodes.
///
/// # Errors
///
/// [`ExportError::Media`] when the file cannot be opened or decoded, and
/// [`ExportError::InvalidArgument`] for a rate or channel count the analyzer rejects.
pub fn analyze_file(path: &Path) -> Result<AnalysisReport, ExportError> {
    verify::analyze_stream(path)
}

/// The descriptive analysis of a reference recording (SON-P15): tempo with confidence, energy and
/// segments ([`crate::reference::ReferenceReport`]). Reads every container this crate decodes;
/// nothing about the recording leaves the report.
///
/// # Errors
///
/// [`ExportError::Media`] when the file cannot be opened or decoded, and
/// [`ExportError::InvalidArgument`] for a rate or channel count the analysis rejects, an empty
/// file or one shorter than [`crate::reference::MIN_SECONDS`].
pub fn analyze_reference_file(
    path: &Path,
) -> Result<crate::reference::ReferenceReport, ExportError> {
    verify::analyze_reference_stream(path)
}

/// The containers this build can write (the agent capability `export_formats`).
#[must_use]
pub fn available_containers() -> Vec<Container> {
    Container::ALL
        .into_iter()
        .filter(|c| c.is_available())
        .collect()
}

/// TPDF dither or none (`Dither`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dither {
    /// No dither.
    None,
    /// Triangular dither of ±1 LSB.
    #[default]
    Tpdf,
}

/// The normalization reference (`Normalize`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Normalize {
    /// No gain.
    #[default]
    None,
    /// Sample peak to the target in dBFS.
    Peak,
    /// True peak to the target in dBTP.
    TruePeak,
    /// Integrated loudness to the target in LUFS, capped by the true-peak ceiling.
    Loudness,
}

/// One requested output file (`OutputSpec`).
#[derive(Debug, Clone, PartialEq)]
pub struct OutputSpec {
    /// The output file.
    pub path: PathBuf,
    /// The container.
    pub container: Container,
    /// Integer bit depth; for float, more than 32 selects `DOUBLE`.
    pub bit_depth: u32,
    /// Float samples.
    pub floating_point: bool,
    /// The output rate; `None` keeps the render's.
    pub sample_rate: Option<u32>,
    /// 1 or 2; `None` is 2.
    pub channels: Option<u32>,
    /// Target bitrate of lossy formats in kbps.
    pub bitrate_kbps: Option<u32>,
    /// Variable bitrate (MP3) or the variable-bitrate table.
    pub variable_bitrate: bool,
    /// Dither for integer PCM below 32 bits.
    pub dither: Dither,
}

impl OutputSpec {
    /// The prototype's defaults: WAV, 24-bit PCM, the render's rate, stereo, TPDF dither.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            container: Container::Wav,
            bit_depth: 24,
            floating_point: false,
            sample_rate: None,
            channels: None,
            bitrate_kbps: None,
            variable_bitrate: false,
            dither: Dither::Tpdf,
        }
    }

    /// The rate written for a render at `source_rate`: the request, or the lowest allowed
    /// rate at or above it, else the highest allowed one (`resolved_rate`).
    #[must_use]
    pub fn resolved_rate(&self, source_rate: u32) -> u32 {
        let rate = self.sample_rate.filter(|&r| r != 0).unwrap_or(source_rate);
        match self.container.rates() {
            Some(allowed) if !allowed.contains(&rate) => allowed
                .iter()
                .copied()
                .filter(|&r| r >= rate)
                .min()
                .unwrap_or_else(|| allowed.iter().copied().max().unwrap_or(rate)),
            _ => rate,
        }
    }

    /// The libsndfile encoding (`_subtype`).
    fn subtype(&self) -> Result<Subtype, ExportError> {
        let c = self.container;
        match c {
            Container::Mp3 => return Ok(Subtype::MpegLayerIII),
            Container::Ogg => return Ok(Subtype::Vorbis),
            Container::Opus => return Ok(Subtype::Opus),
            _ => {}
        }
        if self.floating_point {
            if !c.float_ok() {
                return Err(ExportError::Unsupported(format!(
                    "{} does not support floating point",
                    c.value()
                )));
            }
            return Ok(if self.bit_depth <= 32 {
                Subtype::Float
            } else {
                Subtype::Double
            });
        }
        match self.bit_depth {
            16 | 24 | 32 if c.pcm_depths().contains(&self.bit_depth) => Ok(match self.bit_depth {
                16 => Subtype::Pcm16,
                24 => Subtype::Pcm24,
                _ => Subtype::Pcm32,
            }),
            other => {
                let depths: Vec<String> = c.pcm_depths().iter().map(u32::to_string).collect();
                Err(ExportError::Unsupported(format!(
                    "{} supports bit depths [{}], not {other}",
                    c.value(),
                    depths.join(", ")
                )))
            }
        }
    }

    /// The compression level of a lossy output: interpolated from the bitrate tables when a
    /// bitrate is set, else 0 for MP3 (320 kbps CBR) and 0.25 for Vorbis and Opus; at most
    /// 0.99.
    fn compression_level(&self) -> f64 {
        let c = self.container;
        let level = match self.bitrate_kbps {
            Some(kbps) => {
                let table = if self.variable_bitrate || c.cbr_table().is_empty() {
                    c.vbr_table()
                } else {
                    c.cbr_table()
                };
                interp_level(table, f64::from(kbps))
            }
            None if c == Container::Mp3 => 0.0,
            None => 0.25,
        };
        level.clamp(0.0, 0.99)
    }
}

/// `_interp_level`: the compression level for `kbps` from a `(level, kbps)` table whose
/// bitrates fall with the level; `kbps` is clamped to the table's range and interpolated
/// linearly as `np.interp` does.
#[must_use]
#[expect(
    clippy::float_cmp,
    reason = "np.interp returns the table value for an exact match of a bitrate"
)]
pub fn interp_level(table: &[(f64, f64)], kbps: f64) -> f64 {
    let (Some(&(first_level, max_rate)), Some(&(last_level, min_rate))) =
        (table.first(), table.last())
    else {
        return 0.0;
    };
    let x = kbps.clamp(min_rate, max_rate);
    // `xp` (the bitrates) increase with the index from the end of the table.
    let points: Vec<(f64, f64)> = table.iter().rev().map(|&(l, r)| (r, l)).collect();
    if x >= max_rate {
        return first_level;
    }
    for pair in points.windows(2) {
        let [(x0, y0), (x1, y1)] = pair else {
            continue;
        };
        if x == *x0 {
            return *y0;
        }
        if x > *x0 && x < *x1 {
            let slope = (y1 - y0) / (x1 - x0);
            // `slope * (x - xp[j]) + fp[j]`, fused: NumPy's `interp` on the recording host
            // (macOS arm64, FMA-contracted). The unfused form misses the recorded levels by an
            // ulp (golden `export/tables.json`).
            return slope.mul_add(x - x0, *y0);
        }
    }
    last_level
}

/// The normalization request (`NormalizeSpec`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalizeSpec {
    /// The reference.
    pub mode: Normalize,
    /// The target in dBFS, dBTP or LUFS.
    pub target_db: f64,
    /// The true-peak ceiling of loudness normalization; `None` for no cap.
    pub true_peak_ceiling_db: Option<f64>,
}

impl Default for NormalizeSpec {
    fn default() -> Self {
        Self {
            mode: Normalize::None,
            target_db: -14.0,
            true_peak_ceiling_db: Some(-1.0),
        }
    }
}

/// One written file (`OutputResult`).
#[derive(Debug, Clone, PartialEq)]
pub struct OutputResult {
    /// The output path.
    pub path: PathBuf,
    /// The container.
    pub container: Container,
    /// The written rate.
    pub sample_rate: u32,
    /// The written channel count.
    pub channels: u32,
    /// The bit depth; `None` for lossy formats.
    pub bit_depth: Option<u32>,
    /// Float samples.
    pub floating_point: bool,
    /// Frames written.
    pub frames: u64,
    /// File size in bytes.
    pub size_bytes: u64,
    /// The average bitrate of lossy files from the size, in kbps (1 decimal).
    pub bitrate_kbps: Option<f64>,
    /// Samples clipped by the quantizer.
    pub clipped_samples: u64,
    /// Whether TPDF dither was applied.
    pub dithered: bool,
    /// The render's rate when the output was resampled.
    pub resampled_from: Option<u32>,
    /// The metadata keys the container accepted, plus `bext` for BWF.
    pub metadata_written: Vec<String>,
    /// The true peak of the decoded lossy file in dBTP (2 decimals).
    pub decoded_true_peak_dbtp: Option<f64>,
    /// Whether the file read back as written.
    pub verified: bool,
}

impl OutputResult {
    /// Duration in seconds.
    #[must_use]
    pub fn seconds(&self) -> f64 {
        to_f64(self.frames) / f64::from(self.sample_rate)
    }

    /// The prototype's `to_json`, in its key order.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("path".into(), Value::from(self.path.display().to_string()));
        m.insert("container".into(), Value::from(self.container.value()));
        m.insert("sample_rate".into(), Value::from(self.sample_rate));
        m.insert("channels".into(), Value::from(self.channels));
        m.insert(
            "bit_depth".into(),
            self.bit_depth.map_or(Value::Null, Value::from),
        );
        m.insert("floating_point".into(), Value::from(self.floating_point));
        m.insert("frames".into(), Value::from(self.frames));
        m.insert("seconds".into(), Value::from(self.seconds()));
        m.insert("size_bytes".into(), Value::from(self.size_bytes));
        m.insert(
            "bitrate_kbps".into(),
            self.bitrate_kbps.map_or(Value::Null, Value::from),
        );
        m.insert("clipped_samples".into(), Value::from(self.clipped_samples));
        m.insert("dithered".into(), Value::from(self.dithered));
        m.insert(
            "resampled_from".into(),
            self.resampled_from.map_or(Value::Null, Value::from),
        );
        m.insert(
            "metadata_written".into(),
            Value::from(self.metadata_written.clone()),
        );
        m.insert(
            "decoded_true_peak_dbtp".into(),
            self.decoded_true_peak_dbtp.map_or(Value::Null, Value::from),
        );
        m.insert("verified".into(), Value::from(self.verified));
        Value::Object(m)
    }
}

/// The result of an export (`ExportResult`).
#[derive(Debug, Clone, PartialEq)]
pub struct ExportResult {
    /// One entry per output, in request order.
    pub outputs: Vec<OutputResult>,
    /// The normalization gain in dB.
    pub gain_db: f64,
    /// The analysis of the render before the gain.
    pub source_analysis: AnalysisReport,
    /// Integrated loudness after the gain; `None` for silence.
    pub final_integrated_lufs: Option<f64>,
    /// True peak after the gain; `None` for silence.
    pub final_true_peak_dbtp: Option<f64>,
    /// Whether the normalization target was reached; `None` without normalization.
    pub normalization_target_reached: Option<bool>,
    /// Warnings: the cap of loudness normalization, skipped normalization, clipping,
    /// resampling.
    pub warnings: Vec<String>,
}

impl ExportResult {
    /// The prototype's `to_json`, in its key order.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert(
            "outputs".into(),
            Value::Array(self.outputs.iter().map(OutputResult::to_json).collect()),
        );
        m.insert("gain_db".into(), Value::from(py_round(self.gain_db, 3)));
        m.insert(
            "final_integrated_lufs".into(),
            self.final_integrated_lufs
                .map_or(Value::Null, |v| Value::from(py_round(v, 2))),
        );
        m.insert(
            "final_true_peak_dbtp".into(),
            self.final_true_peak_dbtp
                .map_or(Value::Null, |v| Value::from(py_round(v, 2))),
        );
        m.insert(
            "normalization_target_reached".into(),
            self.normalization_target_reached
                .map_or(Value::Null, Value::from),
        );
        m.insert(
            "analysis_before_gain".into(),
            self.source_analysis.to_json(false),
        );
        m.insert("warnings".into(), Value::from(self.warnings.clone()));
        Value::Object(m)
    }
}

/// A progress callback: the fraction done and a message.
pub type Progress = dyn Fn(f64, &str);

/// How one export runs.
#[derive(Clone, Copy, Default)]
pub struct ExportOptions<'a> {
    /// Normalization.
    pub normalize: NormalizeSpec,
    /// Metadata in insertion order; a repeated key behaves like a `dict` update.
    pub metadata: &'a [(String, String)],
    /// Replace existing output files.
    pub overwrite: bool,
    /// The render's first frame (the BWF time reference at the render rate).
    pub time_reference: u64,
    /// Polled before every block and chunk; `true` cancels the export.
    pub cancelled: Option<&'a dyn Fn() -> bool>,
    /// Progress from 0.6 (render done) to 1.0, with a message.
    pub progress: Option<&'a Progress>,
    /// The Ogg stream serial; `None` derives one from the clock, as libsndfile does.
    pub ogg_serial: Option<u32>,
}

impl std::fmt::Debug for ExportOptions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportOptions")
            .field("normalize", &self.normalize)
            .field("metadata", &self.metadata)
            .field("overwrite", &self.overwrite)
            .field("time_reference", &self.time_reference)
            .finish_non_exhaustive()
    }
}

/// `validate_output`: the encoding, the channel count, the bitrate and the availability.
///
/// # Errors
///
/// [`ExportError::Unsupported`] for an encoding the container does not write, a channel count
/// other than 1 or 2, or a container this build cannot write (MP3 without libmp3lame, FLAC
/// without libFLAC), and
/// [`ExportError::InvalidArgument`] for a bitrate on a lossless format.
pub fn validate_output(spec: &OutputSpec) -> Result<(), ExportError> {
    spec.subtype()?;
    if !matches!(spec.channels, None | Some(1 | 2)) {
        return Err(ExportError::Unsupported(
            "exports support 1 or 2 channels".to_owned(),
        ));
    }
    if spec.bitrate_kbps.is_some() && !spec.container.is_lossy() {
        return Err(ExportError::InvalidArgument(
            "bitrate applies to lossy formats only".to_owned(),
        ));
    }
    if !spec.container.is_available() {
        return Err(ExportError::Unsupported(format!(
            "{} encoding is not available in this build",
            spec.container.value()
        )));
    }
    Ok(())
}

/// `_gain`: the gain in dB, whether the target was reached, and the warnings.
#[must_use]
pub fn normalization_gain(
    report: &AnalysisReport,
    spec: &NormalizeSpec,
) -> (f64, Option<bool>, Vec<String>) {
    let mut warnings = Vec::new();
    let reference = match spec.mode {
        Normalize::None => return (0.0, None, warnings),
        Normalize::Peak => report.sample_peak_dbfs,
        Normalize::TruePeak => report.true_peak_dbtp,
        Normalize::Loudness => report.integrated_lufs,
    };
    if !reference.is_finite() {
        warnings.push("signal is silent; normalization skipped".to_owned());
        return (0.0, Some(false), warnings);
    }
    let mut gain = spec.target_db - reference;
    let mut reached = true;
    if spec.mode == Normalize::Loudness
        && let Some(ceiling) = spec.true_peak_ceiling_db
    {
        let limit = ceiling - report.true_peak_dbtp;
        if gain > limit {
            warnings.push(format!(
                "loudness target {} LUFS not reached: the true-peak ceiling {} dBTP limits the \
                 gain to {limit:+.2} dB (result {:.1} LUFS). Add a limiter to get louder.",
                float_g(spec.target_db),
                float_g(ceiling),
                report.integrated_lufs + limit
            ));
            gain = limit;
            reached = false;
        }
    }
    (gain, Some(reached), warnings)
}

/// Runs the two-pass export (`Exporter`).
pub struct Exporter {
    temp_dir: PathBuf,
    clock: ExportClock,
}

impl std::fmt::Debug for Exporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Exporter")
            .field("temp_dir", &self.temp_dir)
            .finish_non_exhaustive()
    }
}

/// The encoder of one output.
enum Sink {
    Pcm(PcmFileWriter),
    Flac(FlacWriter),
    Mp3(Mp3Writer),
    /// Boxed: the Vorbis encoder state is much larger than the other sinks.
    Vorbis(Box<VorbisWriter>),
    Opus(OpusWriter),
}

impl Sink {
    fn write_pcm(&mut self, q: &[i32]) -> Result<(), ExportError> {
        match self {
            Self::Pcm(w) => w.write_pcm(q),
            Self::Flac(w) => w.write(q),
            _ => Err(ExportError::InvalidArgument(
                "integer samples for a lossy format".to_owned(),
            )),
        }
    }

    fn write_f32(&mut self, x: &[f32]) -> Result<(), ExportError> {
        match self {
            Self::Pcm(w) => w.write_f32(x),
            Self::Mp3(w) => w.write(x),
            Self::Vorbis(w) => w.write(x),
            Self::Opus(w) => w.write(x),
            Self::Flac(_) => Err(ExportError::InvalidArgument(
                "float samples for FLAC".to_owned(),
            )),
        }
    }

    fn finish(self) -> Result<(), ExportError> {
        match self {
            Self::Pcm(w) => w.finish(),
            Self::Flac(w) => w.finish(),
            Self::Mp3(w) => w.finish(),
            Self::Vorbis(w) => w.finish(),
            Self::Opus(w) => w.finish(),
        }
    }
}

impl Exporter {
    /// An exporter whose intermediate files go to `temp_dir` (created when needed), with the
    /// system clock.
    #[must_use]
    pub fn new(temp_dir: impl Into<PathBuf>) -> Self {
        Self {
            temp_dir: temp_dir.into(),
            clock: system_clock(),
        }
    }

    /// The same exporter with another clock.
    #[must_use]
    pub fn with_clock(mut self, clock: ExportClock) -> Self {
        self.clock = clock;
        self
    }

    /// Exports `blocks` (interleaved stereo `f64` at `sample_rate`) to every output.
    ///
    /// # Errors
    ///
    /// The errors of [`validate_output`]; [`ExportError::InvalidArgument`] for no outputs,
    /// a block with a partial frame or nothing rendered; [`ExportError::Conflict`] for an
    /// existing output without `overwrite`, or a directory in its place;
    /// [`ExportError::Cancelled`]; [`ExportError::Verification`] when a file does not read
    /// back as written; and the I/O, codec and encoder errors of writing.
    pub fn export<I, B>(
        &self,
        blocks: I,
        sample_rate: u32,
        outputs: &[OutputSpec],
        options: &ExportOptions<'_>,
    ) -> Result<ExportResult, ExportError>
    where
        I: IntoIterator<Item = B>,
        B: AsRef<[f64]>,
    {
        check_outputs(outputs, options)?;
        let (intermediate, report) = self.render_pass(blocks, sample_rate, options)?;
        if let Some(progress) = options.progress {
            progress(0.6, "encoding");
        }
        let (gain_db, reached, mut warnings) = normalization_gain(&report, &options.normalize);
        let mut results = Vec::with_capacity(outputs.len());
        for (i, spec) in outputs.iter().enumerate() {
            let job = OutputJob {
                intermediate: intermediate.path(),
                source_rate: sample_rate,
                spec,
                gain_db,
                options,
                clock: &self.clock,
            };
            results.push(job.run()?);
            if let Some(progress) = options.progress {
                progress(
                    0.6 + 0.4 * to_f64(to_u64(i + 1)) / to_f64(to_u64(outputs.len())),
                    "encoding",
                );
            }
        }
        warnings.extend(results.iter().flat_map(output_warnings));
        let final_integrated_lufs = report
            .integrated_lufs
            .is_finite()
            .then_some(report.integrated_lufs + gain_db);
        let final_true_peak_dbtp = report
            .true_peak_dbtp
            .is_finite()
            .then_some(report.true_peak_dbtp + gain_db);
        Ok(ExportResult {
            outputs: results,
            gain_db,
            source_analysis: report,
            final_integrated_lufs,
            final_true_peak_dbtp,
            normalization_target_reached: reached,
            warnings,
        })
    }

    /// The render pass: every block is analyzed and stored in a temporary `f64` file, which is
    /// removed when the returned handle drops.
    fn render_pass<I, B>(
        &self,
        blocks: I,
        sample_rate: u32,
        options: &ExportOptions<'_>,
    ) -> Result<(NamedTempFile, AnalysisReport), ExportError>
    where
        I: IntoIterator<Item = B>,
        B: AsRef<[f64]>,
    {
        fs::create_dir_all(&self.temp_dir)
            .map_err(|e| ExportError::io("create", &self.temp_dir, e))?;
        let intermediate = tempfile::Builder::new()
            .prefix("render-")
            .suffix(".f64")
            .tempfile_in(&self.temp_dir)
            .map_err(|e| ExportError::io("create", &self.temp_dir, e))?;
        let mut analyzer = Analyzer::new(sample_rate, 2)
            .map_err(|e| ExportError::InvalidArgument(e.to_string()))?;
        let mut frames = 0u64;
        let io = |e| ExportError::io("write", intermediate.path(), e);
        let mut out = BufWriter::with_capacity(1 << 20, intermediate.as_file());
        let mut bytes = Vec::new();
        for block in blocks {
            check_cancelled(options)?;
            let block = block.as_ref();
            if !block.len().is_multiple_of(2) {
                return Err(ExportError::InvalidArgument(
                    "render blocks must hold whole stereo frames".to_owned(),
                ));
            }
            analyzer.feed(block);
            bytes.clear();
            bytes.extend(block.iter().flat_map(|v| v.to_le_bytes()));
            out.write_all(&bytes).map_err(io)?;
            frames += to_u64(block.len() / 2);
        }
        out.flush().map_err(io)?;
        drop(out);
        if frames == 0 {
            return Err(ExportError::InvalidArgument(
                "nothing was rendered".to_owned(),
            ));
        }
        Ok((intermediate, analyzer.finish()))
    }
}

/// The checks before the render: outputs requested, each valid, and its path free (or
/// replaceable with `overwrite`, and not a directory).
fn check_outputs(outputs: &[OutputSpec], options: &ExportOptions<'_>) -> Result<(), ExportError> {
    if outputs.is_empty() {
        return Err(ExportError::InvalidArgument(
            "no export outputs requested".to_owned(),
        ));
    }
    for spec in outputs {
        validate_output(spec)?;
        if spec.path.exists() && !options.overwrite {
            return Err(ExportError::Conflict {
                message: format!("{} already exists", spec.path.display()),
                path: Some(spec.path.clone()),
            });
        }
        if spec.path.is_dir() {
            return Err(ExportError::Conflict {
                message: format!("{} is a directory", spec.path.display()),
                path: None,
            });
        }
    }
    Ok(())
}

/// The warnings of one written file: clipped samples, then resampling.
fn output_warnings(r: &OutputResult) -> Vec<String> {
    let name = r
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut warnings = Vec::new();
    if r.clipped_samples > 0 {
        warnings.push(format!(
            "{name}: {} samples clipped at full scale",
            r.clipped_samples
        ));
    }
    if let Some(from) = r.resampled_from {
        warnings.push(format!(
            "{name}: resampled from {from} Hz to {} Hz",
            r.sample_rate
        ));
    }
    warnings
}

fn check_cancelled(options: &ExportOptions<'_>) -> Result<(), ExportError> {
    if options.cancelled.is_some_and(|cancelled| cancelled()) {
        Err(ExportError::Cancelled)
    } else {
        Ok(())
    }
}

/// The encoding pass of one output (`Exporter._write`).
struct OutputJob<'a> {
    intermediate: &'a Path,
    source_rate: u32,
    spec: &'a OutputSpec,
    gain_db: f64,
    options: &'a ExportOptions<'a>,
    clock: &'a ExportClock,
}

/// What the encoding pass produced, before the file is renamed and checked.
struct Encoded {
    frames: u64,
    written: Vec<String>,
    clipped: u64,
    dithered: bool,
}

impl OutputJob<'_> {
    fn run(&self) -> Result<OutputResult, ExportError> {
        let spec = self.spec;
        let rate = spec.resolved_rate(self.source_rate);
        let channels = usize::try_from(spec.channels.unwrap_or(2)).unwrap_or(2);
        let subtype = spec.subtype()?;
        let parent = spec.path.parent().unwrap_or(Path::new("."));
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| ExportError::io("create", parent, e))?;
        }
        let name = spec
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let partial = spec.path.with_file_name(format!(".{name}.partial"));
        let encoded = match self.encode(&partial, rate, channels, subtype) {
            Ok(encoded) => encoded,
            Err(e) => {
                let _ = fs::remove_file(&partial);
                return Err(e);
            }
        };
        fs::rename(&partial, &spec.path).map_err(|e| {
            let _ = fs::remove_file(&partial);
            ExportError::io("replace", &spec.path, e)
        })?;
        let size_bytes = fs::metadata(&spec.path)
            .map_err(|e| ExportError::io("inspect", &spec.path, e))?
            .len();
        let lossy = spec.container.is_lossy();
        let seconds = to_f64(encoded.frames) / f64::from(rate);
        let mut result = OutputResult {
            path: spec.path.clone(),
            container: spec.container,
            sample_rate: rate,
            channels: u32::try_from(channels).unwrap_or(2),
            bit_depth: (!lossy).then_some(spec.bit_depth),
            floating_point: matches!(subtype, Subtype::Float | Subtype::Double),
            frames: encoded.frames,
            size_bytes,
            bitrate_kbps: lossy
                .then(|| py_round(to_f64(size_bytes) * 8.0 / seconds.max(1e-9) / 1000.0, 1)),
            clipped_samples: encoded.clipped,
            dithered: encoded.dithered,
            resampled_from: (rate != self.source_rate).then_some(self.source_rate),
            metadata_written: encoded.written,
            decoded_true_peak_dbtp: None,
            verified: false,
        };
        verify_output(&mut result)?;
        Ok(result)
    }

    /// Writes `partial`: the chunks of the intermediate through gain, mix, resampling and
    /// quantization into the encoder; for BWF, the `bext` chunk afterwards.
    fn encode(
        &self,
        partial: &Path,
        rate: u32,
        channels: usize,
        subtype: Subtype,
    ) -> Result<Encoded, ExportError> {
        let spec = self.spec;
        let gain = 10f64.powf(self.gain_db / 20.0);
        let mut quantizer = match subtype {
            Subtype::Pcm16 | Subtype::Pcm24 | Subtype::Pcm32 => Some(Quantizer::new(
                spec.bit_depth,
                spec.dither == Dither::Tpdf && spec.bit_depth < 32,
            )),
            _ => None,
        };
        let mut resampler = if rate == self.source_rate {
            None
        } else {
            Some(ResampleStream64::new(self.source_rate, rate, channels)?)
        };
        let (mut sink, mut written) = self.open_sink(partial, rate, channels, subtype)?;
        let mut source = ChunkReader::open(self.intermediate)?;
        let mut chunk: Vec<f64> = Vec::with_capacity(CHUNK * 2);
        let mut scratch = Scratch::default();
        let mut frames_out = 0u64;
        loop {
            check_cancelled(self.options)?;
            let last = source.next(&mut chunk)?;
            apply_gain(&mut chunk, gain);
            if channels == 1 {
                chunk = chunk
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|[l, r]| f64::midpoint(*l, *r))
                    .collect();
            }
            if let Some(r) = resampler.as_mut() {
                chunk = r.resample_chunk(&chunk, last)?;
            }
            if !chunk.is_empty() {
                write_chunk(&mut sink, quantizer.as_mut(), subtype, &chunk, &mut scratch)?;
                frames_out += to_u64(chunk.len() / channels);
            }
            if last {
                break;
            }
        }
        sink.finish()?;
        if spec.container == Container::Bwf {
            self.add_bext(partial, rate, channels)?;
            written.push("bext".to_owned());
        }
        Ok(Encoded {
            frames: frames_out,
            written,
            clipped: quantizer.as_ref().map_or(0, Quantizer::clipped),
            dithered: quantizer.as_ref().is_some_and(Quantizer::dithers),
        })
    }

    /// `write_bext`: the loudness fields describe the file as written (after gain, mix and
    /// resampling), so the written file is analyzed first.
    fn add_bext(&self, partial: &Path, rate: u32, channels: usize) -> Result<(), ExportError> {
        let report = verify::analyze_stream(partial)?;
        // `metadata.get("title", "")[:256]`: the last value of the key, as in a dict.
        let title: String = self
            .options
            .metadata
            .iter()
            .rev()
            .find(|(k, _)| k == "title")
            .map(|(_, v)| v.chars().take(256).collect())
            .unwrap_or_default();
        let time_reference =
            scale_time_reference(self.options.time_reference, rate, self.source_rate);
        let history = format!(
            "A=PCM,F={rate},W={},M={},T=Sonara\r\n",
            self.spec.bit_depth,
            if channels == 1 { "mono" } else { "stereo" }
        );
        let chunk = bext_chunk(
            &title,
            "Sonara",
            time_reference,
            Some(&BextLoudness::from_report(&report)),
            &history,
            (self.clock)(),
        );
        insert_bext(partial, &chunk)
    }

    /// Opens the encoder for the container and returns it with `metadata_written`.
    fn open_sink(
        &self,
        partial: &Path,
        rate: u32,
        channels: usize,
        subtype: Subtype,
    ) -> Result<(Sink, Vec<String>), ExportError> {
        let spec = self.spec;
        let metadata = self.options.metadata;
        if let Some(container) = PcmContainer::of(spec.container) {
            let format = match subtype {
                Subtype::Pcm16 => SampleFormat::Pcm16,
                Subtype::Pcm24 => SampleFormat::Pcm24,
                Subtype::Pcm32 => SampleFormat::Pcm32,
                Subtype::Double => SampleFormat::Double,
                _ => SampleFormat::Float,
            };
            let writer = PcmFileWriter::create(
                partial,
                container,
                format,
                u16::try_from(channels).unwrap_or(2),
                rate,
                metadata,
                Arc::clone(self.clock),
            )?;
            let written = writer.metadata_written().to_vec();
            return Ok((Sink::Pcm(writer), written));
        }
        let (strings, written) = Strings::set(spec.container, metadata);
        let level = spec.compression_level();
        let serial = self
            .options
            .ogg_serial
            .unwrap_or_else(|| clock_serial(self.clock));
        let sink = match spec.container {
            Container::Flac => Sink::Flac(FlacWriter::create(
                partial,
                rate,
                channels,
                spec.bit_depth,
                strings,
            )?),
            Container::Mp3 => Sink::Mp3(Mp3Writer::create(
                partial,
                rate,
                channels,
                level,
                spec.variable_bitrate,
                strings,
            )?),
            Container::Ogg => Sink::Vorbis(Box::new(VorbisWriter::create(
                partial, rate, channels, level, &strings, serial,
            )?)),
            _ => Sink::Opus(OpusWriter::create(
                partial, rate, channels, level, strings, serial,
            )?),
        };
        Ok((sink, written))
    }
}

/// Reads the intermediate render in 65536-frame chunks.
struct ChunkReader<'a> {
    path: &'a Path,
    reader: BufReader<File>,
    raw: Vec<u8>,
}

impl<'a> ChunkReader<'a> {
    fn open(path: &'a Path) -> Result<Self, ExportError> {
        let file = File::open(path).map_err(|e| ExportError::io("open", path, e))?;
        Ok(Self {
            path,
            reader: BufReader::with_capacity(1 << 20, file),
            raw: vec![0u8; CHUNK * 16],
        })
    }

    /// The next chunk into `chunk`; true when it is the last (shorter than 65536 frames,
    /// possibly empty), as `src.read(CHUNK)` and `chunk.shape[0] < CHUNK`.
    fn next(&mut self, chunk: &mut Vec<f64>) -> Result<bool, ExportError> {
        let got = read_full(&mut self.reader, &mut self.raw)
            .map_err(|e| ExportError::io("read", self.path, e))?;
        let frames = got / 16;
        chunk.clear();
        let bytes = self.raw.get(..frames * 16).unwrap_or_default();
        chunk.extend(
            bytes
                .as_chunks::<8>()
                .0
                .iter()
                .map(|b| f64::from_le_bytes(*b)),
        );
        Ok(frames < CHUNK)
    }
}

/// `chunk * gain`, skipped when the gain is exactly 1 as in the prototype (`if gain != 1.0`).
#[expect(
    clippy::float_cmp,
    reason = "the prototype skips the multiplication only for a gain of exactly 1.0"
)]
fn apply_gain(chunk: &mut [f64], gain: f64) {
    if gain != 1.0 {
        for v in chunk {
            *v *= gain;
        }
    }
}

/// Reusable buffers of the encoding pass.
#[derive(Default)]
struct Scratch {
    pcm: Vec<i32>,
    floats: Vec<f32>,
}

/// Quantizes (integer PCM) or casts (float and lossy) one chunk and writes it.
fn write_chunk(
    sink: &mut Sink,
    quantizer: Option<&mut Quantizer>,
    subtype: Subtype,
    chunk: &[f64],
    scratch: &mut Scratch,
) -> Result<(), ExportError> {
    if let Some(q) = quantizer {
        q.quantize(chunk, &mut scratch.pcm);
        return sink.write_pcm(&scratch.pcm);
    }
    if subtype == Subtype::Double {
        return match sink {
            Sink::Pcm(w) => w.write_f64(chunk),
            _ => Err(ExportError::InvalidArgument(
                "double samples for a non-PCM container".to_owned(),
            )),
        };
    }
    scratch.floats.clear();
    scratch.floats.extend(chunk.iter().map(|&v| to_f32(v)));
    sink.write_f32(&scratch.floats)
}

/// `_verify`: the file reads back with the written rate and channels (and frames, for
/// lossless formats); lossy files are decoded and their true peak measured.
fn verify_output(result: &mut OutputResult) -> Result<(), ExportError> {
    let lossy = result.container.is_lossy();
    let shape = verify::shape(&result.path)?;
    if shape.sample_rate != result.sample_rate
        || u32::try_from(shape.channels).ok() != Some(result.channels)
    {
        return Err(ExportError::Verification(format!(
            "verification failed for {}",
            result.path.display()
        )));
    }
    if !lossy && shape.frames != result.frames {
        return Err(ExportError::Verification(format!(
            "verification failed for {}: frame count mismatch",
            result.path.display()
        )));
    }
    if lossy {
        let report = verify::analyze_stream(&result.path)?;
        result.decoded_true_peak_dbtp = report
            .true_peak_dbtp
            .is_finite()
            .then(|| py_round(report.true_peak_dbtp, 2));
    }
    result.verified = true;
    Ok(())
}

/// `round(time_reference * rate / source_rate)`: the exact quotient of the integers, rounded
/// to the nearest `f64`, then to the nearest integer with ties to even.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the rounded quotient is non-negative and below 2^64"
)]
fn scale_time_reference(time_reference: u64, rate: u32, source_rate: u32) -> u64 {
    if rate == source_rate || source_rate == 0 {
        return time_reference;
    }
    let product = u128::from(time_reference) * u128::from(rate);
    let quotient = to_f64_u128(product) / f64::from(source_rate);
    quotient.round_ties_even() as u64
}

/// A stream serial from the clock (libsndfile seeds its serial from the time of day).
fn clock_serial(clock: &ExportClock) -> u32 {
    let now = clock();
    let nanos = now.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let mixed = nanos.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 64;
    u32::try_from(mixed & 0x7FFF_FFFF).unwrap_or(0)
}

/// Reads until `buf` is full or the input ends; returns the bytes read.
fn read_full(reader: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        let Some(rest) = buf.get_mut(filled..) else {
            break;
        };
        match reader.read(rest) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

fn to_u64(v: usize) -> u64 {
    u64::try_from(v).unwrap_or(u64::MAX)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "frame counts and file sizes stay far below 2^53, where the conversion is exact"
)]
fn to_f64(v: u64) -> f64 {
    v as f64
}

#[expect(
    clippy::cast_precision_loss,
    reason = "Python divides the exact integer product; the conversion rounds it like Python's"
)]
fn to_f64_u128(v: u128) -> f64 {
    v as f64
}

/// `astype(np.float32)`: rounding to the nearest `f32`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the float32 output rounds each sample to the nearest f32"
)]
fn to_f32(v: f64) -> f32 {
    v as f32
}

#[cfg(test)]
mod tests {
    use super::{Container, MP3_CBR, OutputSpec, interp_level, scale_time_reference};

    #[test]
    fn rates_resolve_to_the_lowest_allowed_rate_at_or_above_the_request() {
        let mut spec = OutputSpec::new("x.mp3");
        spec.container = Container::Mp3;
        spec.sample_rate = Some(96_000);
        assert_eq!(spec.resolved_rate(48_000), 48_000);
        spec.sample_rate = Some(7000);
        assert_eq!(spec.resolved_rate(48_000), 8000);
        spec.sample_rate = None;
        assert_eq!(spec.resolved_rate(88_200), 48_000);
        spec.container = Container::Wav;
        assert_eq!(spec.resolved_rate(88_200), 88_200);
    }

    #[test]
    fn levels_interpolate_the_bitrate_tables() {
        assert_eq!(interp_level(MP3_CBR, 320.0), 0.0);
        assert_eq!(interp_level(MP3_CBR, 1000.0), 0.0);
        assert_eq!(interp_level(MP3_CBR, 1.0), 0.99);
        assert_eq!(interp_level(MP3_CBR, 160.0), 0.5);
        assert!((interp_level(MP3_CBR, 128.0) - 0.625).abs() < 1e-12);
    }

    #[test]
    fn time_references_scale_with_the_rate() {
        assert_eq!(scale_time_reference(48_000, 44_100, 48_000), 44_100);
        assert_eq!(
            scale_time_reference(1, 44_100, 88_200),
            0,
            "0.5 rounds to even"
        );
        assert_eq!(
            scale_time_reference(3, 44_100, 88_200),
            2,
            "1.5 rounds to even"
        );
        assert_eq!(scale_time_reference(7, 48_000, 48_000), 7);
    }
}
