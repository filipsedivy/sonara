//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! The Python and NumPy element semantics the processors' wrappers rely on, where they differ
//! from Rust's `f64` methods: NumPy's `maximum`, `minimum`, `clip` and `max(initial=…)`
//! propagate NaN (`f64::max` drops it), Python's `max` and `min` keep the first argument on a
//! tie or NaN, and Python's float `%` takes the sign of the divisor.

/// `np.maximum(a, b)`: `a` when `a >= b` or `a` is NaN, else `b`, so a NaN in either
/// propagates.
pub(crate) fn maximum(a: f64, b: f64) -> f64 {
    if a >= b || a.is_nan() { a } else { b }
}

/// `np.minimum(a, b)`: `a` when `a <= b` or `a` is NaN, else `b`.
pub(crate) fn minimum(a: f64, b: f64) -> f64 {
    if a <= b || a.is_nan() { a } else { b }
}

/// `np.clip(x, lo, hi)`: `minimum(maximum(x, lo), hi)`; NaN stays NaN.
pub(crate) fn clip(x: f64, lo: f64, hi: f64) -> f64 {
    minimum(maximum(x, lo), hi)
}

/// `ndarray.max(initial=initial)`: the largest of `initial` and `values`, NaN propagating.
pub(crate) fn max_initial(values: &[f64], initial: f64) -> f64 {
    values.iter().fold(initial, |peak, &v| maximum(peak, v))
}

/// Python's `max(a, b)`: `a` unless `b > a`.
pub(crate) fn py_max(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// Python's `min(a, b)`: `a` unless `b < a`.
pub(crate) fn py_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// Python's float `x % y`: C's `fmod`, moved into the divisor's sign (`float_rem` in
/// `floatobject.c`). A zero result takes the divisor's sign.
pub(crate) fn py_mod(x: f64, y: f64) -> f64 {
    let rem = x % y;
    if rem == 0.0 {
        0.0_f64.copysign(y)
    } else if (y < 0.0) != (rem < 0.0) {
        rem + y
    } else {
        rem
    }
}

#[cfg(test)]
mod tests {
    use super::{clip, max_initial, maximum, minimum, py_max, py_min, py_mod};

    #[test]
    fn numpy_reductions_propagate_nan() {
        assert!(maximum(f64::NAN, 1.0).is_nan());
        assert!(maximum(1.0, f64::NAN).is_nan());
        assert!(minimum(f64::NAN, 1.0).is_nan());
        assert!(clip(f64::NAN, -1.0, 1.0).is_nan());
        assert!(max_initial(&[1.0, f64::NAN, 2.0], 0.0).is_nan());
        assert_eq!(max_initial(&[-3.0, -1.0], 0.0), 0.0);
        assert_eq!(max_initial(&[], 0.0), 0.0);
        assert_eq!(max_initial(&[0.5, 2.5, 1.0], 0.0), 2.5);
        assert_eq!(clip(4.0, -1.0, 1.0), 1.0);
        assert_eq!(clip(-4.0, -1.0, 1.0), -1.0);
        assert_eq!(clip(0.25, -1.0, 1.0), 0.25);
    }

    #[test]
    fn python_builtins() {
        assert_eq!(py_max(0.2, 0.0), 0.2);
        assert_eq!(py_max(-0.5, 0.0), 0.0);
        assert!(py_max(f64::NAN, 0.0).is_nan(), "the first argument is kept");
        assert_eq!(py_min(2.0, 1.0), 1.0);
        assert_eq!(py_mod(1.25, 1.0), 0.25);
        assert_eq!(py_mod(-0.25, 1.0), 0.75);
        assert_eq!(py_mod(0.25, -1.0), -0.75);
        assert!(
            py_mod(2.0, -1.0).is_sign_negative(),
            "a zero takes the divisor's sign"
        );
        assert!(py_mod(-2.0, 1.0).is_sign_positive());
    }
}
