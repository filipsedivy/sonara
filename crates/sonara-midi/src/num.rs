//! Numeric helpers that reproduce Python semantics the generators rely on.

use std::cmp::Ordering;

/// Python's `str.isspace` (and the regex `\s` on `str`): Unicode white space plus the
/// information separators U+001C to U+001F.
pub(crate) fn is_py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Python's `str.strip()`.
pub(crate) fn py_strip(text: &str) -> &str {
    text.trim_matches(is_py_space)
}

/// An integer as a float. MIDI pitches, velocities and counters are far below 2**53, so the
/// conversion is exact.
#[expect(
    clippy::cast_precision_loss,
    reason = "values are small MIDI quantities, exactly representable"
)]
pub(crate) const fn to_f64(value: i64) -> f64 {
    value as f64
}

/// A length as a float, exactly for any realistic length.
#[expect(
    clippy::cast_precision_loss,
    reason = "lengths are far below 2**53, exactly representable"
)]
pub(crate) const fn to_f64_len(len: usize) -> f64 {
    len as f64
}

/// Python's `round(x)` for an integral result: ties go to the even integer. Out-of-range
/// values saturate, which the later note validation rejects.
#[expect(
    clippy::cast_possible_truncation,
    reason = "saturating conversion of an already rounded value is intended"
)]
pub(crate) fn py_round(value: f64) -> i64 {
    value.round_ties_even() as i64
}

/// Python's `round(x, 6)`: the decimal string is correctly rounded from the exact binary value
/// (ties to even), then parsed back.
pub(crate) fn py_round6(value: f64) -> f64 {
    format!("{value:.6}").parse().unwrap_or(value)
}

/// Python's float `%`: the result has the sign of the divisor.
pub(crate) fn py_mod(value: f64, divisor: f64) -> f64 {
    let remainder = value % divisor;
    if remainder == 0.0 {
        0.0_f64.copysign(divisor)
    } else if (remainder < 0.0) != (divisor < 0.0) {
        remainder + divisor
    } else {
        remainder
    }
}

/// Python's float floor division `a // b` for a finite, non-zero `b`.
pub(crate) fn py_floordiv(a: f64, b: f64) -> f64 {
    let modulo = a % b;
    let mut div = (a - modulo) / b;
    if modulo != 0.0 && (b < 0.0) != (modulo < 0.0) {
        div -= 1.0;
    }
    if div == 0.0 {
        return 0.0_f64.copysign(a / b);
    }
    let floored = div.floor();
    if div - floored > 0.5 {
        floored + 1.0
    } else {
        floored
    }
}

/// Python's `min(a, b)`: `a` unless `b` is smaller (the first of equal values, so `-0.0` and
/// `0.0` keep Python's choice).
pub(crate) fn py_min(a: f64, b: f64) -> f64 {
    if b.partial_cmp(&a) == Some(Ordering::Less) {
        b
    } else {
        a
    }
}

/// Python's `max(a, b)`: `a` unless `b` is larger.
pub(crate) fn py_max(a: f64, b: f64) -> f64 {
    if b.partial_cmp(&a) == Some(Ordering::Greater) {
        b
    } else {
        a
    }
}

/// Orders finite floats like Python's `<` does; equal values (including `-0.0` and `0.0`) keep
/// their order in a stable sort.
pub(crate) fn py_cmp(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use super::{py_floordiv, py_max, py_min, py_mod, py_round, py_round6};

    #[test]
    fn rounding_matches_python() {
        assert_eq!(py_round(0.5), 0);
        assert_eq!(py_round(1.5), 2);
        assert_eq!(py_round(2.5), 2);
        assert_eq!(py_round(-0.5), 0);
        assert_eq!(py_round(-1.5), -2);
        assert_eq!(py_round(7.2), 7);
        // round(x, 6) rounds the exact binary value half to even: 1/128 and 3/128 are ties.
        assert_eq!(py_round6(0.007_812_5), 0.007_812);
        assert_eq!(py_round6(0.023_437_5), 0.023_438);
        assert_eq!(py_round6(1.234_567_49), 1.234_567);
        assert_eq!(py_round6(2.0), 2.0);
    }

    #[test]
    fn modulo_takes_the_sign_of_the_divisor() {
        assert_eq!(py_mod(5.5, 2.0), 1.5);
        assert_eq!(py_mod(-0.5, 1.0), 0.5);
        assert_eq!(py_mod(0.5, -1.0), -0.5);
        assert_eq!(py_mod(3.0, 1.0), 0.0);
        assert!(py_mod(3.0, -1.0).is_sign_negative());
    }

    #[test]
    fn floor_division_follows_python() {
        assert_eq!(py_floordiv(7.5, 4.0), 1.0);
        assert_eq!(py_floordiv(-7.5, 4.0), -2.0);
        assert_eq!(py_floordiv(8.0, 4.0), 2.0);
        assert_eq!(py_floordiv(0.0, 4.0), 0.0);
        assert_eq!(py_floordiv(-0.5, 4.0), -1.0);
    }

    #[test]
    fn min_and_max_keep_the_first_of_equal_values() {
        assert!(py_max(0.0, -0.0).is_sign_positive());
        assert!(py_min(-0.0, 0.0).is_sign_negative());
        assert_eq!(py_min(0.3, 0.2), 0.2);
        assert_eq!(py_max(0.3, 0.2), 0.3);
    }
}
