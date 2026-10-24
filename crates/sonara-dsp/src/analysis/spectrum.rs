//! Not real-time: the Analyzer's Welch spectrum of the mono sum (see the parent module).

use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};

use crate::numpy::math::sum;
use crate::tables::spectrum_window;

use super::loudness::len_f64;

/// Welch segment length (the prototype's `SPECTRUM_FFT`).
pub(super) const SPECTRUM_FFT: usize = 8192;
/// Hop between segments: half the segment.
const HOP: usize = SPECTRUM_FFT / 2;
/// Octave-band centres with the prototype's keys (`f"{c:g}"`).
const OCTAVES: [(f64, &str); 10] = [
    (31.5, "31.5"),
    (63.0, "63"),
    (125.0, "125"),
    (250.0, "250"),
    (500.0, "500"),
    (1000.0, "1000"),
    (2000.0, "2000"),
    (4000.0, "4000"),
    (8000.0, "8000"),
    (16000.0, "16000"),
];
/// A band without energy, dB.
const EMPTY_BAND: f64 = -120.0;

/// Accumulates `|rfft(segment · hann)|²` over 8192-sample segments every 4096 samples; the
/// remainder at the end is dropped, as in the prototype.
pub(super) struct SpectrumMeter {
    fft: Arc<dyn RealToComplex<f64>>,
    frame: Vec<f64>,
    bins: Vec<Complex<f64>>,
    scratch: Vec<Complex<f64>>,
    /// Mono samples not yet consumed by a segment.
    buffer: Vec<f64>,
    /// Sum of the power spectra.
    power: Vec<f64>,
    segments: u64,
}

impl std::fmt::Debug for SpectrumMeter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpectrumMeter")
            .field("segments", &self.segments)
            .finish_non_exhaustive()
    }
}

impl Clone for SpectrumMeter {
    fn clone(&self) -> Self {
        Self {
            fft: Arc::clone(&self.fft),
            frame: self.frame.clone(),
            bins: self.bins.clone(),
            scratch: self.scratch.clone(),
            buffer: self.buffer.clone(),
            power: self.power.clone(),
            segments: self.segments,
        }
    }
}

impl SpectrumMeter {
    #[expect(
        clippy::disallowed_macros,
        reason = "not real-time: sizes the analysis buffers"
    )]
    pub(super) fn new() -> Self {
        let fft = RealFftPlanner::<f64>::new().plan_fft_forward(SPECTRUM_FFT);
        Self {
            frame: fft.make_input_vec(),
            bins: fft.make_output_vec(),
            scratch: fft.make_scratch_vec(),
            fft,
            buffer: Vec::new(),
            power: vec![0.0; SPECTRUM_FFT / 2 + 1],
            segments: 0,
        }
    }

    /// Appends `mono` and accumulates every complete segment.
    pub(super) fn feed(&mut self, mono: &[f64]) {
        self.buffer.extend_from_slice(mono);
        let window = spectrum_window();
        let mut start = 0;
        while let Some(segment) = self.buffer.get(start..start + SPECTRUM_FFT) {
            for ((x, &s), &w) in self.frame.iter_mut().zip(segment).zip(window) {
                *x = s * w;
            }
            // Sized in `new`; realfft rejects nothing else.
            if self
                .fft
                .process_with_scratch(&mut self.frame, &mut self.bins, &mut self.scratch)
                .is_ok()
            {
                for (p, z) in self.power.iter_mut().zip(&self.bins) {
                    let magnitude = numpy_abs(*z);
                    *p += magnitude * magnitude;
                }
                self.segments += 1;
            }
            start += HOP;
        }
        self.buffer.drain(..start.min(self.buffer.len()));
    }

    /// The octave bands (dB relative to the total, in band order) and the spectral centroid:
    /// the prototype's `finish`. Without a segment, or without energy above DC, there are no
    /// bands and the centroid is 0.
    #[expect(
        clippy::disallowed_methods,
        reason = "not real-time: builds the band list on the analysis thread"
    )]
    pub(super) fn bands(&self, sample_rate: u32) -> (Vec<(&'static str, f64)>, f64) {
        let mut bands = Vec::new();
        if self.segments == 0 {
            return (bands, 0.0);
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "segment counts are far below 2^53, exactly representable"
        )]
        let count = self.segments as f64;
        let psd: Vec<f64> = self.power.iter().map(|p| p / count).collect();
        let sr = f64::from(sample_rate);
        // `np.fft.rfftfreq(n, 1 / sr)`: `arange(n // 2 + 1) * (1 / (n · d))`.
        let step = 1.0 / (len_f64(SPECTRUM_FFT) * (1.0 / sr));
        let freqs: Vec<f64> = (0..psd.len()).map(|i| len_f64(i) * step).collect();
        let total = psd.get(1..).map_or(0.0, sum);
        if total <= 0.0 {
            return (bands, 0.0);
        }
        let weighted: Vec<f64> = psd.iter().zip(&freqs).map(|(p, f)| p * f).collect();
        let centroid = sum(&weighted) / sum(&psd);
        let root2 = 2.0_f64.sqrt();
        for (centre, name) in OCTAVES {
            let (low, high) = (centre / root2, centre * root2);
            if low >= sr / 2.0 {
                break;
            }
            // The mask `(freqs >= low) & (freqs < high)` selects a contiguous run of bins.
            let first = freqs.partition_point(|&f| f < low);
            let end = freqs.partition_point(|&f| f < high);
            let energy = psd.get(first..end.max(first)).map_or(0.0, sum);
            let db = if energy > 0.0 {
                10.0 * (energy / total).log10()
            } else {
                EMPTY_BAND
            };
            bands.push((name, db));
        }
        (bands, centroid)
    }
}

/// NumPy's SIMD `absolute` of a complex128 (`loops_unary_complex`), bit for bit on the
/// recording host: with `larger = max(|re|, |im|)` and `smaller = min(|re|, |im|)`, `sqrt(fma(r,
/// r, 1)) · larger` for `r = smaller / larger`, and 0 when `larger` is 0 (it is neither libm's
/// `hypot` nor `sqrt(re² + im²)`).
fn numpy_abs(z: Complex<f64>) -> f64 {
    let (a, b) = (z.re.abs(), z.im.abs());
    let (larger, smaller) = if a >= b { (a, b) } else { (b, a) };
    if larger == 0.0 {
        return 0.0;
    }
    if smaller.is_infinite() {
        return f64::INFINITY;
    }
    let ratio = smaller / larger;
    ratio.mul_add(ratio, 1.0).sqrt() * larger
}

#[cfg(test)]
mod tests {
    use super::{SPECTRUM_FFT, SpectrumMeter, numpy_abs};
    use realfft::num_complex::Complex;

    #[test]
    fn numpy_abs_scales_by_the_larger_component() {
        assert_eq!(numpy_abs(Complex::new(3.0, -4.0)), 5.0);
        assert_eq!(numpy_abs(Complex::new(0.0, -0.0)), 0.0);
        assert_eq!(numpy_abs(Complex::new(-2.0, 0.0)), 2.0);
        assert_eq!(
            numpy_abs(Complex::new(1e-300, 1e-300)),
            1e-300 * 2.0_f64.sqrt()
        );
    }

    #[test]
    fn a_sine_puts_its_energy_into_its_octave() {
        let sr = 48000.0;
        let mono: Vec<f64> = (0..SPECTRUM_FFT * 3)
            .map(|i| {
                (2.0 * std::f64::consts::PI * 1000.0 * f64::from(u32::try_from(i).unwrap()) / sr)
                    .sin()
            })
            .collect();
        let mut meter = SpectrumMeter::new();
        // Uneven pieces: the remainder carries over.
        for piece in mono.chunks(5000) {
            meter.feed(piece);
        }
        assert_eq!(meter.segments, 5);
        let (bands, centroid) = meter.bands(48000);
        assert_eq!(bands.len(), 10);
        let &(name, db) = bands.get(5).unwrap();
        assert_eq!(name, "1000");
        assert!(db > -0.01, "{db}");
        assert!((centroid - 1000.0).abs() < 1.0, "{centroid}");
        // At 22.05 kHz the 16 kHz band starts above Nyquist.
        assert_eq!(meter.bands(22050).0.len(), 9);
    }
}
