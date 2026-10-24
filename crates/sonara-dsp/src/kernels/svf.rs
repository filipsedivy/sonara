//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! Topology-preserving-transform state-variable filter.

use std::f64::consts::PI;

/// Lowest cutoff the prototype's wrapper passes to the kernel, in Hz.
const MIN_CUTOFF_HZ: f64 = 5.0;
/// Highest cutoff the prototype's wrapper passes to the kernel, as a fraction of the sample rate.
const MAX_CUTOFF_RATIO: f64 = 0.49;
/// Lowest Q the prototype's wrapper passes to the kernel.
const MIN_Q: f64 = 0.05;

/// Response of the state-variable filter (the `mode` parameter of [`svf`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SvfMode {
    /// Low-pass.
    Lowpass,
    /// High-pass.
    Highpass,
    /// Band-pass.
    Bandpass,
    /// Notch (band-reject).
    Notch,
    /// Peak (low-pass subtracted twice from the notch).
    Peak,
    /// All-pass.
    Allpass,
}

impl SvfMode {
    /// Maps the raw mode the way the prototype's kernel does (golden kernel vectors):
    /// 0–4 select a response, and every other value selects `Allpass`.
    #[must_use]
    pub fn from_raw(mode: i32) -> Self {
        match mode {
            0 => Self::Lowpass,
            1 => Self::Highpass,
            2 => Self::Bandpass,
            3 => Self::Notch,
            4 => Self::Peak,
            _ => Self::Allpass,
        }
    }
}

/// Clamps a cutoff to the range the prototype's Python wrapper feeds to the native kernel:
/// 5 Hz up to 0.49 × the sample rate.
///
/// [`svf`] itself does not clamp (like the native kernel). A cutoff at or above Nyquist makes
/// the filter unstable, so callers apply this to modulated cutoffs first.
#[must_use]
pub fn clamp_cutoff(cutoff_hz: f64, sample_rate: f64) -> f64 {
    cutoff_hz
        .max(MIN_CUTOFF_HZ)
        .min(MAX_CUTOFF_RATIO * sample_rate)
}

/// Clamps Q to at least 0.05, as the prototype's Python wrapper does before calling the native
/// kernel. [`svf`] itself does not clamp.
#[must_use]
pub fn clamp_q(q: f64) -> f64 {
    q.max(MIN_Q)
}

/// Topology-preserving-transform state-variable filter (Simper form) with a per-sample cutoff.
///
/// - `x` and `out` are interleaved `(frames, channels)` buffers, row-major, where `channels` is
///   `state.len()`.
/// - `cutoff_hz` holds one cutoff in Hz per frame. Cutoff and `q` are used as given (see
///   [`clamp_cutoff`] and [`clamp_q`]). A `q` of zero or less, or a cutoff at or above Nyquist,
///   gives unstable or non-finite output.
/// - `state` holds the two integrator states of every channel. It is read at the start and
///   updated, so consecutive calls continue the same signal.
///
/// The frames processed are the common length of `x`, `out` and `cutoff_hz` (a trailing partial
/// frame is ignored; see the [module docs](super)). With an empty `state` nothing is processed.
///
/// # Examples
///
/// ```
/// use sonara_dsp::kernels::{SvfMode, svf};
///
/// // One channel: a unit step through a low-pass settles at 1.
/// let x = [1.0; 4096];
/// let cutoff = [1000.0; 4096];
/// let mut out = [0.0; 4096];
/// let mut state = [[0.0; 2]];
/// svf(&x, &mut out, &cutoff, 0.707, SvfMode::Lowpass, 48_000.0, &mut state);
/// assert!(out.last().is_some_and(|y| (y - 1.0).abs() < 1e-9));
/// ```
pub fn svf(
    x: &[f64],
    out: &mut [f64],
    cutoff_hz: &[f64],
    q: f64,
    mode: SvfMode,
    sample_rate: f64,
    state: &mut [[f64; 2]],
) {
    let channels = state.len();
    if channels == 0 {
        return;
    }
    let k = 1.0 / q;
    let frames = x
        .chunks_exact(channels)
        .zip(out.chunks_exact_mut(channels))
        .zip(cutoff_hz);
    for ((x_frame, out_frame), &cutoff) in frames {
        // The coefficients depend on the frame only. The native kernel recomputes them per
        // channel; the values are identical.
        let g = (PI * cutoff / sample_rate).tan();
        let a1 = 1.0 / g.mul_add(g + k, 1.0);
        let a2 = g * a1;
        let a3 = g * a2;
        for ((&v0, y), ic) in x_frame
            .iter()
            .zip(out_frame.iter_mut())
            .zip(state.iter_mut())
        {
            let [ic1, ic2] = ic;
            let v3 = v0 - *ic2;
            let v1 = a1.mul_add(*ic1, a2 * v3);
            let v2 = a3.mul_add(v3, a2.mul_add(*ic1, *ic2));
            // `2 · v` is exact, so the oracle's fused `fma(2, v, -ic)` equals this.
            *ic1 = 2.0 * v1 - *ic1;
            *ic2 = 2.0 * v2 - *ic2;
            *y = response(mode, v0, v1, v2, k);
        }
    }
}

/// The output of one mode, in the oracle's contracted form: `v0 − k·v1` is fused; the
/// further `− 2·v2` of the peak is exact as written, since `2·v2` is.
#[inline]
fn response(mode: SvfMode, v0: f64, v1: f64, v2: f64, k: f64) -> f64 {
    match mode {
        SvfMode::Lowpass => v2,
        SvfMode::Highpass => (-k).mul_add(v1, v0) - v2,
        SvfMode::Bandpass => v1,
        SvfMode::Notch => (-k).mul_add(v1, v0),
        SvfMode::Peak => (-k).mul_add(v1, v0) - 2.0 * v2,
        SvfMode::Allpass => (-2.0 * k).mul_add(v1, v0),
    }
}

#[cfg(test)]
mod tests {
    use super::{SvfMode, clamp_cutoff, clamp_q, svf};

    const SR: f64 = 48_000.0;

    #[test]
    fn unknown_raw_modes_select_allpass() {
        let known = [
            SvfMode::Lowpass,
            SvfMode::Highpass,
            SvfMode::Bandpass,
            SvfMode::Notch,
            SvfMode::Peak,
            SvfMode::Allpass,
        ];
        for (raw, mode) in (0..).zip(known) {
            assert_eq!(SvfMode::from_raw(raw), mode);
        }
        for raw in [-1, 6, 7, i32::MAX, i32::MIN] {
            assert_eq!(SvfMode::from_raw(raw), SvfMode::Allpass);
        }
    }

    #[test]
    fn clamps_match_the_wrapper() {
        assert_eq!(clamp_cutoff(1.0, SR), 5.0);
        assert_eq!(clamp_cutoff(1000.0, SR), 1000.0);
        assert_eq!(clamp_cutoff(30_000.0, SR), 0.49 * SR);
        assert_eq!(clamp_q(0.0), 0.05);
        assert_eq!(clamp_q(0.707), 0.707);
    }

    /// One unit impulse through each mode; the first output sample is known in closed form:
    /// with zero state, `v1 = a2`, `v2 = a3`, so every response follows from `g` and `k` (in the
    /// oracle's contracted forms).
    #[test]
    fn impulse_response_first_sample() {
        let (cutoff, q) = (1000.0, 2.0);
        let g = (std::f64::consts::PI * cutoff / SR).tan();
        let k = 1.0 / q;
        let a1 = 1.0 / g.mul_add(g + k, 1.0);
        let (v1, v2) = (g * a1, g * (g * a1));
        let notch = (-k).mul_add(v1, 1.0);
        let cases = [
            (SvfMode::Lowpass, v2),
            (SvfMode::Highpass, notch - v2),
            (SvfMode::Bandpass, v1),
            (SvfMode::Notch, notch),
            (SvfMode::Peak, notch - 2.0 * v2),
            (SvfMode::Allpass, (-2.0 * k).mul_add(v1, 1.0)),
        ];
        for (mode, expected) in cases {
            let mut out = [0.0];
            svf(&[1.0], &mut out, &[cutoff], q, mode, SR, &mut [[0.0; 2]]);
            assert_eq!(out, [expected], "{mode:?}");
        }
    }

    #[test]
    fn channels_are_independent_and_interleaved() {
        const FRAMES: usize = 32;
        let (mut left, mut right) = ([0.0; FRAMES], [0.0; FRAMES]);
        for ((i, l), r) in (0..).zip(left.iter_mut()).zip(right.iter_mut()) {
            *l = f64::from(i).sin();
            *r = f64::from(i * 3).cos();
        }
        let cutoff = [2000.0; FRAMES];

        let (mut mono_l, mut mono_r) = ([0.0; FRAMES], [0.0; FRAMES]);
        svf(
            &left,
            &mut mono_l,
            &cutoff,
            1.5,
            SvfMode::Bandpass,
            SR,
            &mut [[0.0; 2]],
        );
        svf(
            &right,
            &mut mono_r,
            &cutoff,
            1.5,
            SvfMode::Bandpass,
            SR,
            &mut [[0.0; 2]],
        );

        let mut interleaved = [0.0; 2 * FRAMES];
        let mut expected = [0.0; 2 * FRAMES];
        let frames = interleaved
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip(expected.as_chunks_mut::<2>().0.iter_mut())
            .zip(left.iter().zip(&right))
            .zip(mono_l.iter().zip(&mono_r));
        for (((x_frame, y_frame), (l, r)), (mono_l, mono_r)) in frames {
            *x_frame = [*l, *r];
            *y_frame = [*mono_l, *mono_r];
        }
        let mut stereo = [0.0; 2 * FRAMES];
        svf(
            &interleaved,
            &mut stereo,
            &cutoff,
            1.5,
            SvfMode::Bandpass,
            SR,
            &mut [[0.0; 2]; 2],
        );
        assert_eq!(stereo, expected);
    }

    #[test]
    fn shorter_slice_sets_the_length_and_empty_state_is_a_no_op() {
        // Stereo: the cutoff covers 2 frames, x and out 3 frames.
        let mut out = [9.0; 6];
        let mut state = [[0.0; 2]; 2];
        svf(
            &[1.0; 6],
            &mut out,
            &[1000.0; 2],
            1.0,
            SvfMode::Lowpass,
            SR,
            &mut state,
        );
        assert!(out.iter().take(4).all(|y| y.total_cmp(&9.0).is_ne()));
        assert!(out.iter().skip(4).all(|y| y.total_cmp(&9.0).is_eq()));

        // A trailing partial frame is ignored.
        let mut out = [9.0; 5];
        svf(
            &[1.0; 5],
            &mut out,
            &[1000.0; 3],
            1.0,
            SvfMode::Lowpass,
            SR,
            &mut state,
        );
        assert_eq!(out.last(), Some(&9.0));

        let mut out = [9.0; 2];
        svf(
            &[1.0; 2],
            &mut out,
            &[1000.0; 2],
            1.0,
            SvfMode::Lowpass,
            SR,
            &mut [],
        );
        assert_eq!(out, [9.0; 2]);
    }
}
