//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! Level conversions and sample guards of `dsp/base.py`.

/// Decibels to amplitude: `10 ** (db / 20)` (libm `pow`, exact where it agrees with macOS libm).
///
/// # Examples
///
/// ```
/// use sonara_dsp::filters::db_to_amp;
///
/// assert_eq!(db_to_amp(0.0), 1.0);
/// assert_eq!(db_to_amp(-20.0), 0.1);
/// ```
#[must_use]
pub fn db_to_amp(db: f64) -> f64 {
    10.0_f64.powf(db / 20.0)
}

/// Amplitude to decibels: `20 · log10(amp)`, or `floor` when `amp` is not positive (the
/// prototype's default floor is −200 dB).
///
/// # Examples
///
/// ```
/// use sonara_dsp::filters::amp_to_db;
///
/// assert_eq!(amp_to_db(10.0, -200.0), 20.0);
/// assert_eq!(amp_to_db(0.0, -200.0), -200.0);
/// ```
#[must_use]
pub fn amp_to_db(amp: f64, floor: f64) -> f64 {
    if amp > 0.0 { 20.0 * amp.log10() } else { floor }
}

/// One-pole smoothing coefficient for a time constant of `ms` milliseconds:
/// `exp(−1 / (ms · 0.001 · sample_rate))`, and 0 for `ms <= 0`.
///
/// # Examples
///
/// ```
/// use sonara_dsp::filters::time_coefficient;
///
/// assert_eq!(time_coefficient(0.0, 48_000.0), 0.0);
/// assert_eq!(time_coefficient(1.0, 1000.0), (-1.0_f64).exp());
/// ```
#[must_use]
pub fn time_coefficient(ms: f64, sample_rate: f64) -> f64 {
    if ms <= 0.0 {
        return 0.0;
    }
    (-1.0 / (ms * 0.001 * sample_rate)).exp()
}

/// Replaces NaN and ±infinity with 0.0 in place (the guard at node boundaries) and returns how
/// many samples it replaced.
///
/// # Examples
///
/// ```
/// use sonara_dsp::filters::ensure_finite;
///
/// let mut block = [1.0, f64::NAN, f64::INFINITY, -2.0, f64::NEG_INFINITY];
/// assert_eq!(ensure_finite(&mut block), 3);
/// assert_eq!(block, [1.0, 0.0, 0.0, -2.0, 0.0]);
/// ```
pub fn ensure_finite(buffer: &mut [f64]) -> usize {
    let mut replaced = 0_usize;
    for v in buffer.iter_mut().filter(|v| !v.is_finite()) {
        *v = 0.0;
        replaced = replaced.saturating_add(1);
    }
    replaced
}

#[cfg(test)]
mod tests {
    use super::{amp_to_db, db_to_amp, ensure_finite, time_coefficient};

    #[test]
    fn negative_and_nan_inputs() {
        assert_eq!(amp_to_db(-1.0, -120.0), -120.0);
        assert!(amp_to_db(f64::NAN, -120.0).total_cmp(&-120.0).is_eq());
        assert_eq!(time_coefficient(-5.0, 48_000.0), 0.0);
        assert!(time_coefficient(f64::NAN, 48_000.0).is_nan());
        assert_eq!(db_to_amp(f64::NEG_INFINITY), 0.0);
        let mut finite = [0.0, -0.0, f64::MAX];
        assert_eq!(ensure_finite(&mut finite), 0);
        assert_eq!(finite.get(1).map(|v| v.is_sign_negative()), Some(true));
    }
}
