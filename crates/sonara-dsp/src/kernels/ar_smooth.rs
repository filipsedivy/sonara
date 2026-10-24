//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! Attack/release one-pole smoother.

/// Attack/release one-pole smoother: `y[n] = c * y[n-1] + (1 - c) * x[n]`, where `c` is `attack`
/// when `x[n] > y[n-1]` and `release` otherwise. As the oracle computes it:
/// `y = fma(c, y, (1 - c) * x)`.
///
/// `state` is `y[-1]`; it is read at the start and updated to the last output, so consecutive
/// calls continue the same signal. If `x` and `out` differ in length, the common length is
/// processed (see the [module docs](super)).
///
/// # Examples
///
/// ```
/// use sonara_dsp::kernels::ar_smooth;
///
/// let mut out = [0.0; 3];
/// let mut state = 0.0;
/// ar_smooth(&[1.0, 1.0, 0.0], &mut out, 0.5, 0.25, &mut state);
/// assert_eq!(out, [0.5, 0.75, 0.1875]);
/// assert_eq!(state, 0.1875);
/// ```
pub fn ar_smooth(x: &[f64], out: &mut [f64], attack: f64, release: f64, state: &mut f64) {
    let mut y = *state;
    for (&v, o) in x.iter().zip(out.iter_mut()) {
        let c = if v > y { attack } else { release };
        // F8: the oracle fuses `c * y` into the addition.
        y = c.mul_add(y, (1.0 - c) * v);
        *o = y;
    }
    *state = y;
}

#[cfg(test)]
mod tests {
    use super::ar_smooth;

    #[test]
    fn rises_with_attack_and_falls_with_release() {
        let mut out = [0.0; 4];
        let mut state = 0.0;
        ar_smooth(&[1.0, 1.0, 0.0, 0.0], &mut out, 0.5, 0.75, &mut state);
        assert_eq!(out, [0.5, 0.75, 0.5625, 0.421_875]);
        assert_eq!(state, 0.421_875);
    }

    #[test]
    fn equal_input_uses_release() {
        // `x[n] > y[n-1]` is strict: an equal sample takes the release coefficient.
        let mut out = [0.0];
        let mut state = 1.0;
        ar_smooth(&[1.0], &mut out, 0.0, 0.5, &mut state);
        assert_eq!(out, [1.0]);
    }

    #[test]
    fn shorter_slice_sets_the_length() {
        let mut out = [9.0; 3];
        let mut state = 0.0;
        ar_smooth(&[1.0, 1.0], &mut out, 0.0, 0.0, &mut state);
        assert_eq!(out, [1.0, 1.0, 9.0]);

        let mut out = [9.0; 1];
        let mut state = 0.0;
        ar_smooth(&[1.0, 1.0], &mut out, 0.0, 0.0, &mut state);
        assert_eq!(out, [1.0]);
        assert_eq!(state, 1.0);
    }

    #[test]
    fn empty_input_keeps_the_state() {
        let mut state = 0.25;
        ar_smooth(&[], &mut [], 0.5, 0.5, &mut state);
        assert_eq!(state, 0.25);
    }
}
