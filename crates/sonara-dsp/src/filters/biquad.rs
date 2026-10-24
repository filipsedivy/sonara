//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! Biquad designs of the RBJ Audio EQ Cookbook and Butterworth cascades built from them, as
//! `dsp/filters/biquad.py` computes them in Python floats (F17: plain arithmetic in the same
//! order). Processors redesign while parameters glide, so these run on the render thread and
//! allocate nothing.

use std::f64::consts::PI;

use super::Section;

/// The most sections a [`Cascade`] holds: a 48 dB/octave Butterworth filter.
pub const MAX_CASCADE_SECTIONS: usize = 4;

/// Response of an RBJ biquad (the prototype's `BiquadType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BiquadType {
    /// Low-pass.
    Lowpass,
    /// High-pass.
    Highpass,
    /// Band-pass with a constant 0 dB peak.
    Bandpass,
    /// Notch.
    Notch,
    /// Peaking EQ.
    Peak,
    /// Low shelf.
    LowShelf,
    /// High shelf.
    HighShelf,
    /// All-pass.
    Allpass,
}

/// Python's `max(a, b)`: `a` unless `b > a` (so a NaN `a` stays NaN).
fn py_max(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// Python's `min(a, b)`: `a` unless `b < a`.
fn py_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// One normalized section `[b0, b1, b2, 1, a1, a2]` of the RBJ cookbook (the prototype's
/// `biquad`).
///
/// `freq` is clamped to `[1, 0.499 · sample_rate]` Hz and `q` to at least 0.001; `gain_db` is
/// used by the peak and shelf responses only. The arithmetic is the prototype's, so the result
/// is exact wherever the platform's `sin`, `cos` and `pow` agree with macOS libm.
///
/// # Examples
///
/// ```
/// use sonara_dsp::filters::{BiquadType, biquad};
///
/// // A 0 dB peak passes the signal unchanged.
/// let [b0, b1, b2, a0, a1, a2] = biquad(BiquadType::Peak, 1000.0, 48_000.0, 2.0, 0.0);
/// assert_eq!((b0, b1, b2, a0), (1.0, a1, a2, 1.0));
/// ```
#[must_use]
#[expect(
    clippy::manual_midpoint,
    reason = "RT-11: the prototype's `(1 + cw) / 2`, written as it computes it"
)]
pub fn biquad(kind: BiquadType, freq: f64, sample_rate: f64, q: f64, gain_db: f64) -> Section {
    let f = py_min(py_max(freq, 1.0), 0.499 * sample_rate);
    let w0 = 2.0 * PI * f / sample_rate;
    let (cw, sw) = (w0.cos(), w0.sin());
    let q = py_max(q, 1e-3);
    let alpha = sw / (2.0 * q);
    let a = 10.0_f64.powf(gain_db / 40.0);
    let (b, den) = match kind {
        BiquadType::Lowpass => (
            [(1.0 - cw) / 2.0, 1.0 - cw, (1.0 - cw) / 2.0],
            [1.0 + alpha, -2.0 * cw, 1.0 - alpha],
        ),
        BiquadType::Highpass => (
            [(1.0 + cw) / 2.0, -(1.0 + cw), (1.0 + cw) / 2.0],
            [1.0 + alpha, -2.0 * cw, 1.0 - alpha],
        ),
        BiquadType::Bandpass => ([alpha, 0.0, -alpha], [1.0 + alpha, -2.0 * cw, 1.0 - alpha]),
        BiquadType::Notch => ([1.0, -2.0 * cw, 1.0], [1.0 + alpha, -2.0 * cw, 1.0 - alpha]),
        BiquadType::Allpass => (
            [1.0 - alpha, -2.0 * cw, 1.0 + alpha],
            [1.0 + alpha, -2.0 * cw, 1.0 - alpha],
        ),
        BiquadType::Peak => (
            [1.0 + alpha * a, -2.0 * cw, 1.0 - alpha * a],
            [1.0 + alpha / a, -2.0 * cw, 1.0 - alpha / a],
        ),
        BiquadType::LowShelf => {
            let sa = 2.0 * a.sqrt() * alpha;
            (
                [
                    a * ((a + 1.0) - (a - 1.0) * cw + sa),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cw),
                    a * ((a + 1.0) - (a - 1.0) * cw - sa),
                ],
                [
                    (a + 1.0) + (a - 1.0) * cw + sa,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cw),
                    (a + 1.0) + (a - 1.0) * cw - sa,
                ],
            )
        }
        BiquadType::HighShelf => {
            let sa = 2.0 * a.sqrt() * alpha;
            (
                [
                    a * ((a + 1.0) + (a - 1.0) * cw + sa),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cw),
                    a * ((a + 1.0) + (a - 1.0) * cw - sa),
                ],
                [
                    (a + 1.0) - (a - 1.0) * cw + sa,
                    2.0 * ((a - 1.0) - (a + 1.0) * cw),
                    (a + 1.0) - (a - 1.0) * cw - sa,
                ],
            )
        }
    };
    let [b0, b1, b2] = b;
    let [a0, a1, a2] = den;
    [b0 / a0, b1 / a0, b2 / a0, 1.0, a1 / a0, a2 / a0]
}

/// Slope of a Butterworth high- or low-pass cascade (the prototype's `slope_db`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ButterworthSlope {
    /// 12 dB/octave: order 2, one section.
    Db12,
    /// 24 dB/octave: order 4, two sections.
    Db24,
    /// 36 dB/octave: order 6, three sections.
    Db36,
    /// 48 dB/octave: order 8, four sections.
    Db48,
}

impl ButterworthSlope {
    /// The slope for `slope_db` of 12, 24, 36 or 48; `None` for any other value.
    #[must_use]
    pub fn from_db(slope_db: i64) -> Option<Self> {
        match slope_db {
            12 => Some(Self::Db12),
            24 => Some(Self::Db24),
            36 => Some(Self::Db36),
            48 => Some(Self::Db48),
            _ => None,
        }
    }

    /// The filter order, `slope_db // 6`.
    #[must_use]
    pub fn order(self) -> u32 {
        match self {
            Self::Db12 => 2,
            Self::Db24 => 4,
            Self::Db36 => 6,
            Self::Db48 => 8,
        }
    }
}

/// The section Qs of an even-order Butterworth filter, `1 / (2 sin((2k − 1) π / (2 · order)))`
/// for `k = 1 ..= order / 2` (the prototype's `butterworth_qs`).
pub fn butterworth_qs(slope: ButterworthSlope) -> impl Iterator<Item = f64> {
    let order = slope.order();
    (1..=order / 2)
        .map(move |k| 1.0 / (2.0 * (f64::from(2 * k - 1) * PI / f64::from(2 * order)).sin()))
}

/// A cascade of up to [`MAX_CASCADE_SECTIONS`] sections, held inline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cascade {
    sections: [Section; MAX_CASCADE_SECTIONS],
    len: usize,
}

impl Cascade {
    /// The sections in filtering order.
    #[must_use]
    pub fn sections(&self) -> &[Section] {
        self.sections.get(..self.len).unwrap_or_default()
    }
}

/// A Butterworth cascade: one [`biquad`] per Q of [`butterworth_qs`], stacked in order (the
/// prototype's `butterworth`). The prototype uses high- and low-pass responses.
///
/// # Examples
///
/// ```
/// use sonara_dsp::filters::{BiquadType, ButterworthSlope, butterworth};
///
/// let cascade = butterworth(BiquadType::Highpass, 80.0, 48_000.0, ButterworthSlope::Db24);
/// assert_eq!(cascade.sections().len(), 2);
/// ```
#[must_use]
pub fn butterworth(
    kind: BiquadType,
    freq: f64,
    sample_rate: f64,
    slope: ButterworthSlope,
) -> Cascade {
    let mut cascade = Cascade {
        sections: [[0.0; 6]; MAX_CASCADE_SECTIONS],
        len: 0,
    };
    for (section, q) in cascade.sections.iter_mut().zip(butterworth_qs(slope)) {
        *section = biquad(kind, freq, sample_rate, q, 0.0);
        cascade.len += 1;
    }
    cascade
}

#[cfg(test)]
mod tests {
    use super::{BiquadType, ButterworthSlope, biquad, butterworth, butterworth_qs};

    const SR: f64 = 48_000.0;

    #[test]
    fn slopes_map_to_orders() {
        for (db, order) in [(12, 2), (24, 4), (36, 6), (48, 8)] {
            let slope = ButterworthSlope::from_db(db).unwrap();
            assert_eq!(slope.order(), order);
            assert_eq!(
                butterworth_qs(slope).count(),
                usize::try_from(order / 2).unwrap()
            );
            let sections = butterworth(BiquadType::Lowpass, 1000.0, SR, slope);
            assert_eq!(
                sections.sections().len(),
                usize::try_from(order / 2).unwrap()
            );
        }
        assert_eq!(ButterworthSlope::from_db(18), None);
    }

    #[test]
    fn second_order_q_is_one_over_root_two() {
        let q: Vec<f64> = butterworth_qs(ButterworthSlope::Db12).collect();
        assert_eq!(q.len(), 1);
        assert!(
            q.iter()
                .all(|q| (q - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-15)
        );
    }

    #[test]
    fn lowpass_has_unit_dc_gain_and_highpass_zero() {
        for kind in [BiquadType::Lowpass, BiquadType::Highpass] {
            let [b0, b1, b2, _, a1, a2] = biquad(kind, 2000.0, SR, 0.75, 0.0);
            let dc = (b0 + b1 + b2) / (1.0 + a1 + a2);
            let expected = if kind == BiquadType::Lowpass {
                1.0
            } else {
                0.0
            };
            assert!((dc - expected).abs() < 1e-12, "{kind:?}: {dc}");
        }
    }

    #[test]
    fn clamps_frequency_and_q() {
        let low = biquad(BiquadType::Bandpass, -5.0, SR, 1.0, 0.0);
        assert_eq!(low, biquad(BiquadType::Bandpass, 1.0, SR, 1.0, 0.0));
        let high = biquad(BiquadType::Notch, 1e9, SR, 1.0, 0.0);
        assert_eq!(high, biquad(BiquadType::Notch, 0.499 * SR, SR, 1.0, 0.0));
        let flat = biquad(BiquadType::Allpass, 500.0, SR, 0.0, 0.0);
        assert_eq!(flat, biquad(BiquadType::Allpass, 500.0, SR, 1e-3, 0.0));
    }

    #[test]
    fn shelves_and_peaks_reach_their_gain() {
        // DC gain of a low shelf and Nyquist gain of a high shelf are A² = 10^(g/20).
        let g = 6.0_f64;
        let [b0, b1, b2, _, a1, a2] = biquad(BiquadType::LowShelf, 200.0, SR, 0.75, g);
        let dc = (b0 + b1 + b2) / (1.0 + a1 + a2);
        assert!((dc - 10.0_f64.powf(g / 20.0)).abs() < 1e-9, "{dc}");
        let [b0, b1, b2, _, a1, a2] = biquad(BiquadType::HighShelf, 8000.0, SR, 0.75, g);
        let nyquist = (b0 - b1 + b2) / (1.0 - a1 + a2);
        assert!(
            (nyquist - 10.0_f64.powf(g / 20.0)).abs() < 1e-9,
            "{nyquist}"
        );
    }
}
