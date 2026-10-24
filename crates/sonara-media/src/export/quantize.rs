//! Integer quantization with TPDF dither, the prototype's `_Quantizer` (`audio_export.py`).
//!
//! Per chunk of interleaved samples `x`: `y = x * 2^(bits-1)`; with dither,
//! `y = y + (a - b)` where `a` and `b` are two full `rng.random(shape)` draws of NumPy's
//! `default_rng(0x5A0A)`, the first array completely before the second (C order); then
//! `rint` (ties to even), clipping to `[-2^(bits-1), 2^(bits-1) - 1]` with a count of the
//! clipped samples, and the cast. A new quantizer, and so a new generator, is made for every
//! output file, so equal chunk boundaries give bit-identical dither.

use sonara_dsp::numpy::math::rint;
use sonara_dsp::numpy::random::{Generator, default_rng};

/// The dither seed of every exported file.
pub(crate) const DITHER_SEED: u64 = 0x5A0A;

/// The quantizer of one output file.
pub(crate) struct Quantizer {
    scale: f64,
    dither: bool,
    rng: Generator,
    clipped: u64,
    a: Vec<f64>,
    b: Vec<f64>,
}

impl Quantizer {
    /// A quantizer to `bits` (16, 24 or 32) with TPDF dither when `dither` is true.
    pub(crate) fn new(bits: u32, dither: bool) -> Self {
        Self {
            // `float(2 ** (bits - 1))`: exact for every supported depth.
            scale: 2f64.powi(i32::try_from(bits).unwrap_or(32) - 1),
            dither,
            rng: default_rng(DITHER_SEED),
            clipped: 0,
            a: Vec::new(),
            b: Vec::new(),
        }
    }

    /// Whether this quantizer dithers.
    pub(crate) fn dithers(&self) -> bool {
        self.dither
    }

    /// Samples clipped at full scale so far.
    pub(crate) fn clipped(&self) -> u64 {
        self.clipped
    }

    /// Quantizes one chunk of interleaved samples into `out` (replaced), as integers in the
    /// target range (not shifted: a 24-bit sample is in `-2^23..2^23`).
    pub(crate) fn quantize(&mut self, x: &[f64], out: &mut Vec<i32>) {
        out.clear();
        if self.dither {
            self.a.resize(x.len(), 0.0);
            self.b.resize(x.len(), 0.0);
            self.rng.random_fill(&mut self.a);
            self.rng.random_fill(&mut self.b);
        }
        let hi = self.scale - 1.0;
        let lo = -self.scale;
        for (i, &v) in x.iter().enumerate() {
            let mut y = v * self.scale;
            if self.dither {
                let a = self.a.get(i).copied().unwrap_or_default();
                let b = self.b.get(i).copied().unwrap_or_default();
                y += a - b;
            }
            let mut q = rint(y);
            if q > hi || q < lo {
                self.clipped += 1;
                q = q.clamp(lo, hi);
            }
            out.push(to_int(q));
        }
    }
}

/// The cast after clipping. A NaN (which the renderer never produces) becomes 0.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the value is integral and clipped to the target range, which fits an i32"
)]
fn to_int(q: f64) -> i32 {
    if q.is_nan() { 0 } else { q as i32 }
}

#[cfg(test)]
mod tests {
    use super::Quantizer;
    use sonara_dsp::numpy::random::default_rng;

    #[test]
    fn without_dither_values_round_to_even_and_clip_with_a_count() {
        let mut q = Quantizer::new(16, false);
        let mut out = Vec::new();
        let step = 1.0 / 32768.0;
        q.quantize(
            &[
                0.5 * step,
                1.5 * step,
                -0.5 * step,
                1.0,
                -1.0,
                2.0,
                32767.0 * step,
            ],
            &mut out,
        );
        assert_eq!(out, [0, 2, 0, 32767, -32768, 32767, 32767]);
        assert_eq!(
            q.clipped(),
            2,
            "1.0 and 2.0 exceed 32767; -1.0 is exactly the minimum"
        );
        let mut q24 = Quantizer::new(24, false);
        q24.quantize(&[-1.0, 0.25], &mut out);
        assert_eq!(out, [-8_388_608, 2_097_152]);
        let mut q32 = Quantizer::new(32, false);
        q32.quantize(&[-1.0, 0.5, 1.0], &mut out);
        assert_eq!(out, [i32::MIN, 1 << 30, i32::MAX]);
        assert_eq!(q32.clipped(), 1);
    }

    #[test]
    fn dither_draws_two_whole_arrays_per_chunk() {
        let mut q = Quantizer::new(16, true);
        let mut out = Vec::new();
        let x = [0.1, -0.2, 0.3, 0.0];
        q.quantize(&x, &mut out);
        let mut rng = default_rng(0x5A0A);
        let mut a = [0.0; 4];
        let mut b = [0.0; 4];
        rng.random_fill(&mut a);
        rng.random_fill(&mut b);
        let expected: Vec<i32> = x
            .iter()
            .zip(a.iter().zip(&b))
            .map(|(v, (a, b))| super::to_int((v * 32768.0 + (a - b)).round_ties_even()))
            .collect();
        assert_eq!(out, expected);
        assert!(q.dithers());
    }
}
