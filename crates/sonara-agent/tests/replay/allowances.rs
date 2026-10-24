//! The documented tolerances of the golden agent batches (GLD-01): which values of which
//! recorded results the Rust build reproduces only within a measured bound, and why. One source
//! for the transcript replay (`crates/sonara-agent/tests/golden_transcripts.rs`), the batch
//! test of the `sonara` command (`apps/sonara/tests/commands/workflows.rs`) and the tool-path
//! sessions (`crates/sonara-agent/tests/tool_paths.rs`); the last two include this file.
//!
//! **Why analyses differ.** Results carry the analyzer's report of `float64` renders, before any
//! storage. No render reproduces the prototype's `float64` samples bit for bit, even where the
//! `float32` file does, and the analyzer itself differs in its FFT. Each allowance below names
//! the steps it covers, the cause, and the residual measured on macOS, the recording platform:
//! the largest `|expected − actual| / max(1, |expected|)` over the covered fields. A value within
//! the tolerance compares as the recorded one; anything beyond it, and every other field of every
//! result, compares exactly.

use serde_json::{Value, json};

/// Whether the platform's libm is macOS libm, the recording's.
const MACOS: bool = cfg!(target_os = "macos");

/// The unrounded floats of an analysis report (`AnalysisReport.to_json`): levels, loudness,
/// stereo and spectrum values. Every other field is a count, a key, or a value the prototype
/// rounds (`dc_offset`, the silence regions and times, the loudness series), and compares
/// exactly.
const ANALYSIS_FLOATS: [&str; 15] = [
    "sample_peak_dbfs",
    "true_peak_dbtp",
    "rms_dbfs",
    "crest_factor_db",
    "integrated_lufs",
    "momentary_max_lufs",
    "short_term_max_lufs",
    "loudness_range_lu",
    "plr_db",
    "correlation",
    "min_correlation_400ms",
    "side_to_mid_db",
    "balance_db",
    "octave_bands_db",
    "spectral_centroid_hz",
];

/// What an [`Allowance`] covers.
#[derive(Debug, Clone, Copy)]
pub enum Fields {
    /// These fields of every analysis object of a result (under `analysis` or
    /// `analysis_before_gain`); the octave bands are compared band by band.
    Analysis(&'static [&'static str]),
    /// The number at this JSON pointer of the result, relative to the larger value.
    Pointer(&'static str),
    /// `size_bytes` of every FLAC output (an object with `"container": "flac"`), relative to the
    /// larger size.
    FlacSize,
}

/// A documented tolerance of some recorded values.
#[derive(Debug, Clone)]
pub struct Allowance {
    /// A short name for reports.
    pub name: &'static str,
    /// The step indices it covers; empty for every step of the batch.
    pub steps: &'static [usize],
    /// The fields it covers.
    pub fields: Fields,
    /// The largest scaled difference allowed.
    pub tolerance: f64,
    /// The measured cause.
    pub reason: &'static str,
}

impl Allowance {
    /// Whether it covers step `step`.
    pub fn covers(&self, step: usize) -> bool {
        self.steps.is_empty() || self.steps.contains(&step)
    }

    /// Replaces each covered value of `actual` that differs from `expected` within the
    /// tolerance by the expected value; returns the largest such difference, if any.
    pub fn apply(&self, expected: &Value, actual: &mut Value) -> Option<f64> {
        let mut used = None;
        match self.fields {
            Fields::Pointer(pointer) => {
                if let (Some(e), Some(a)) = (expected.pointer(pointer), actual.pointer_mut(pointer))
                {
                    self.substitute(e, a, true, &mut used);
                }
            }
            Fields::Analysis(_) | Fields::FlacSize => self.walk(expected, actual, false, &mut used),
        }
        used
    }

    fn walk(&self, expected: &Value, actual: &mut Value, analysis: bool, used: &mut Option<f64>) {
        match (expected, actual) {
            (Value::Object(e), Value::Object(a)) => {
                if matches!(self.fields, Fields::FlacSize)
                    && e.get("container") == Some(&json!("flac"))
                    && a.get("container") == Some(&json!("flac"))
                    && let (Some(ev), Some(av)) = (e.get("size_bytes"), a.get_mut("size_bytes"))
                {
                    self.substitute(ev, av, true, used);
                }
                for (key, ev) in e {
                    let Some(av) = a.get_mut(key) else { continue };
                    match self.fields {
                        Fields::Analysis(fields) if analysis && fields.contains(&key.as_str()) => {
                            self.substitute(ev, av, false, used);
                        }
                        _ => {
                            let inner = key == "analysis" || key == "analysis_before_gain";
                            self.walk(ev, av, inner, used);
                        }
                    }
                }
            }
            (Value::Array(e), Value::Array(a)) if e.len() == a.len() => {
                for (ev, av) in e.iter().zip(a) {
                    self.walk(ev, av, analysis, used);
                }
            }
            _ => {}
        }
    }

    /// One covered value: a number within the tolerance becomes the expected number; objects
    /// (the octave bands) are compared key by key.
    fn substitute(
        &self,
        expected: &Value,
        actual: &mut Value,
        relative: bool,
        used: &mut Option<f64>,
    ) {
        if expected == actual {
            return;
        }
        match (expected, &mut *actual) {
            (Value::Object(e), Value::Object(a)) => {
                for (key, ev) in e {
                    if let Some(av) = a.get_mut(key) {
                        self.substitute(ev, av, relative, used);
                    }
                }
            }
            (Value::Number(e), Value::Number(a)) => {
                let (Some(e), Some(a)) = (e.as_f64(), a.as_f64()) else {
                    return;
                };
                let scale = if relative {
                    e.abs().max(a.abs())
                } else {
                    e.abs().max(1.0)
                };
                let difference = (e - a).abs() / scale;
                if difference <= self.tolerance {
                    *actual = expected.clone();
                    *used = Some(used.map_or(difference, |u: f64| u.max(difference)));
                }
            }
            _ => {}
        }
    }
}

/// The analyzer's own differences, in every analysis of every step (the causes of
/// `crates/sonara-dsp/tests/golden_analysis.rs`): the spectrum goes through
/// `realfft`, which rounds differently from NumPy's pocketfft, and the
/// side-to-mid ratio cannot reproduce the prototype's alignment-dependent BLAS `ddot` order.
/// Measured on macOS where the analyzed audio itself equals the prototype's (coverage
/// step 83, a WAV file; sidechain step 6, a render): at most 3.1e-14, an octave band at
/// −100.8 dB. The tolerance leaves a margin for the FFT's rounding of other inputs.
const ANALYZER: Allowance = Allowance {
    name: "ANALYZER",
    steps: &[],
    fields: Fields::Analysis(&["octave_bands_db", "spectral_centroid_hz", "side_to_mid_db"]),
    tolerance: 1e-12,
    reason: "realfft vs pocketfft and the ddot order of side-to-mid",
};

/// Off macOS only: the analyzer's levels end in `log10`, `pow` and `tan` of the platform's libm,
/// which round some values differently from macOS libm
/// (`golden_analysis.rs`, `LIBM_EPSILONS`), and the renders' processors call the platform's
/// `exp`, `sin` and `tan` too. Measured in the development container (aarch64 Linux, glibc): at
/// most 6.2e-15 (sidechain step 6), where no step-scoped allowance applies first.
const LIBM: Allowance = Allowance {
    name: "LIBM",
    steps: &[],
    fields: Fields::Analysis(&ANALYSIS_FLOATS),
    tolerance: 1e-12,
    reason: "the platform's libm differs from macOS libm",
};

/// Renders whose `float64` samples differ from the prototype's by a few ulps, while their
/// `float32` files are byte-identical (sidechain, mix "hot") or differ in 18 bytes (mix
/// "fixed"): the prototype's processors use NumPy's own SIMD transcendental functions, the Rust
/// ones libm. Measured on macOS: at most 6.6e-14
/// (`loudness_range_lu`, mix step 16).
const fn ulp_render(steps: &'static [usize]) -> Allowance {
    Allowance {
        name: "ULP_RENDER",
        steps,
        fields: Fields::Analysis(&ANALYSIS_FLOATS),
        tolerance: 1e-12,
        reason: "NumPy's SIMD transcendentals in the prototype's processors",
    }
}

/// Renders with the render residual of (−125.5 to −168.6 dBFS): the reverb's
/// `float32` wet path (cinematic, electronic, halloween) and the MP3 decoder of a media track
/// (coverage; : within 1.3e-6). Their analyses differ from the 8th significant
/// digit on. Measured on macOS: at most 2.2e-8 (cinematic step 24); per batch 1.9e-8
/// (coverage), 2.2e-8 (cinematic), 8.8e-9 (electronic), 1.4e-8 (halloween).
const fn render_residual(steps: &'static [usize]) -> Allowance {
    Allowance {
        name: "RENDER_RESIDUAL",
        steps,
        fields: Fields::Analysis(&ANALYSIS_FLOATS),
        tolerance: 1e-7,
        reason: "the render residual of (reverb float32 path, MP3 decoder)",
    }
}

/// Coverage step 82 analyzes a media asset decoded from MP3: symphonia's decoder differs from
/// the prototype's mpg123 within 1.3e-6. Measured on macOS: at most 2.4e-6
/// (the 8 kHz octave band at −105 dB), 6.9e-7 on the levels.
const MP3_DECODE: Allowance = Allowance {
    name: "MP3_DECODE",
    steps: &[82],
    fields: Fields::Analysis(&ANALYSIS_FLOATS),
    tolerance: 1e-5,
    reason: "symphonia's MP3 decoder vs mpg123",
};

/// The two previews of `tests/tool_paths.rs::import_audio_formats` (session steps 3 and 5) mix
/// a media asset decoded from Ogg Vorbis: symphonia's Vorbis decoder rounds differently from the
/// prototype's libvorbis (`crates/sonara-media/tests/golden_media.rs` compares the frame counts
/// exactly and reports the sample values). Their analyses differ from the 7th significant digit
/// on. Measured on macOS: at most 1.3e-7 (`true_peak_dbtp`). The tolerance leaves a
/// margin for another platform's rounding of the decoder's window and transform tables.
const VORBIS_DECODE: Allowance = Allowance {
    name: "VORBIS_DECODE",
    steps: &[3, 5],
    fields: Fields::Analysis(&ANALYSIS_FLOATS),
    tolerance: 1e-6,
    reason: "symphonia's Vorbis decoder vs libvorbis (golden_media.rs, lossy decoders)",
};

/// The size of one FLAC export whose 24-bit samples differ from the prototype's, because the
/// render residual above reaches them; the encoder is not the cause (libFLAC 1.4.3 writes every
/// FLAC output of identical samples byte for byte as the prototype did). Coverage step 142,
/// stem 5: the stem of the MP3 media track, whose decoded samples differ from the recording in
/// 35,359 of 3,250,176 samples by up to 5 LSB (measured on macOS by decoding both files); cinematic
/// step 26 and electronic step 49: the FLAC output of a render whose WAV export of the same
/// samples differs too. Measured on macOS: at most 4.4e-4 relative (electronic, 45.6 MB).
const fn residual_flac_size(steps: &'static [usize], pointer: &'static str) -> Allowance {
    Allowance {
        name: "RESIDUAL_FLAC_SIZE",
        steps,
        fields: Fields::Pointer(pointer),
        tolerance: 1e-3,
        reason: "a FLAC file of samples that the render residual changed",
    }
}

/// Off macOS only, mirroring `FLAC_BYTE_EXACT` of `crates/sonara-media/tests/golden_export.rs`:
/// the bytes of a FLAC file come from another libFLAC there. The development container has only
/// Debian trixie's libFLAC 1.5.0, whose LPC analysis is compiled by GCC (other floating-point
/// contraction) and, on the x86-64 CI runner, picks SSE/AVX/FMA routines at run time; any of
/// these can choose other predictor coefficients, and so another size, for the same samples.
/// Only the FLAC outputs of `export_stems` (coverage steps 142 and 146) are covered; the
/// lossless content is pinned by `golden_export.rs`. Measured in the development container
/// (aarch64 Linux): at most 9.5e-4 relative. The tolerance leaves room for the x86-64
/// runner's run-time SIMD routines, which were not measured; a wrong encoding (another bit
/// depth, no compression) changes a size by far more.
const FLAC_ENCODER: Allowance = Allowance {
    name: "FLAC_ENCODER",
    steps: &[142, 146],
    fields: Fields::FlacSize,
    tolerance: 1e-2,
    reason: "libFLAC 1.5.0 built by GCC off macOS (FLAC_BYTE_EXACT of golden_export.rs)",
};

/// Every allowance of the batch `batch` on this platform, in the order they apply: the
/// analyzer's (every step), the step-scoped ones, then the platform libm's (off macOS), so that
/// a step-scoped allowance is used, and checked for staleness, wherever it applies.
pub fn allowances(batch: &str) -> Vec<Allowance> {
    let mut out = vec![ANALYZER];
    match batch {
        "coverage" => {
            out.extend([
                MP3_DECODE,
                render_residual(&[127, 128, 129, 130, 140]),
                residual_flac_size(&[142], "/result/stems/5/size_bytes"),
            ]);
            if !MACOS {
                out.push(FLAC_ENCODER);
            }
        }
        "cinematic_orchestral" => out.extend([
            render_residual(&[24, 25, 26]),
            residual_flac_size(&[26], "/result/outputs/1/size_bytes"),
        ]),
        "electronic_124bpm" => out.extend([
            render_residual(&[48, 49]),
            residual_flac_size(&[49], "/result/outputs/1/size_bytes"),
        ]),
        "halloween_thirteenth_hour" => out.push(render_residual(&[159, 160, 161])),
        "mix_analysis" => out.push(ulp_render(&[10, 16, 17])),
        "sidechain_preview" => out.push(ulp_render(&[10, 11, 12])),
        // A session of `tests/tool_paths.rs` (the prototype's `test_import_audio_formats`).
        "import_audio_formats" => out.push(VORBIS_DECODE),
        _ => {}
    }
    if !MACOS {
        out.push(LIBM);
    }
    out
}
