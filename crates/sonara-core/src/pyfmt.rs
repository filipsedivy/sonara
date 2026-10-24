//! Python-compatible text formatting.
//!
//! The prototype's error messages and its canonical project JSON format numbers and strings the
//! way Python's `repr` does. The golden data records those bytes, so the Rust application
//! reproduces them exactly.

use std::fmt::Write as _;

/// Formats `value` like Python's `repr(float)`.
///
/// The shortest digits that round-trip, positional notation for decimal exponents in `-4..16`
/// with at least one fractional digit (`4.0`), and scientific notation with a signed, two-digit
/// minimum exponent otherwise (`1e-05`, `1.5e+16`).
#[must_use]
pub fn float_repr(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf" } else { "-inf" }.to_owned();
    }
    // Rust's `{:e}` gives the shortest round-trip digits: `1.5e16`, `-3e-5`, `0e0`.
    let sci = format!("{value:e}");
    let (mantissa, exponent) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let (sign, mantissa) = mantissa
        .strip_prefix('-')
        .map_or(("", mantissa), |m| ("-", m));
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    if (-4..16).contains(&exponent) {
        let mut out = String::from(sign);
        if exponent < 0 {
            out.push_str("0.");
            for _ in 0..(-exponent - 1) {
                out.push('0');
            }
            out.push_str(&digits);
        } else {
            let int_len = usize::try_from(exponent).unwrap_or(0) + 1;
            if digits.len() <= int_len {
                out.push_str(&digits);
                for _ in digits.len()..int_len {
                    out.push('0');
                }
                out.push_str(".0");
            } else {
                let (int_part, frac_part) = digits.split_at(int_len);
                out.push_str(int_part);
                out.push('.');
                out.push_str(frac_part);
            }
        }
        out
    } else {
        let mut out = String::from(sign);
        let (first, rest) = digits.split_at(1);
        out.push_str(first);
        if !rest.is_empty() {
            out.push('.');
            out.push_str(rest);
        }
        let exp_sign = if exponent < 0 { '-' } else { '+' };
        // `fmt::Write` for `String` never fails.
        let _ = write!(out, "e{exp_sign}{:02}", exponent.abs());
        out
    }
}

/// Formats `value` like Python's `format(value, "g")`: six significant digits, trailing zeros
/// removed, positional notation for decimal exponents in `-4..6` and scientific notation with a
/// signed, two-digit minimum exponent otherwise (`20000`, `0.001`, `1e-05`, `1.23457e+06`).
#[must_use]
pub fn float_g(value: f64) -> String {
    const PRECISION: i32 = 6;
    if value.is_nan() {
        return "nan".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf" } else { "-inf" }.to_owned();
    }
    // Round to the precision first: the exponent after rounding picks the notation.
    let sci = format!("{value:.5e}");
    let (mantissa, exponent) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    if (-4..PRECISION).contains(&exponent) {
        let decimals = usize::try_from(PRECISION - 1 - exponent).unwrap_or(0);
        strip_fraction_zeros(&format!("{value:.decimals$}"))
    } else {
        let mut out = strip_fraction_zeros(mantissa);
        let exp_sign = if exponent < 0 { '-' } else { '+' };
        // `fmt::Write` for `String` never fails.
        let _ = write!(out, "e{exp_sign}{:02}", exponent.abs());
        out
    }
}

/// The most decimals for which Python's `round` can still change an `f64`: beyond 323 every
/// value is returned unchanged (`float.__round__`).
const PY_NDIGITS_MAX: u32 = 323;

/// Python's `round(x, ndigits)` for `ndigits >= 0`: the exact binary value of `x` correctly
/// rounded to `ndigits` decimals (ties to even), then parsed back. NaN and infinities round to
/// themselves, and so does every `x` for `ndigits > 323`. The sign is kept, so small negative
/// values round to `-0.0`.
///
/// Rust's `{:.N}` formatting is exact and rounds ties to even, like Python's `_Py_dg_dtoa` in
/// mode 3, and `str::parse` is correctly rounded like `_Py_dg_strtod`. The text goes into a
/// stack buffer, so nothing allocates.
///
/// Prototype: the rounded values of tool results and job states (`round(x, 3)` of seconds,
/// `round(x, 2)` of levels, `round(x, 4)` of progress, `round(x, 6)` of beats).
///
/// # Examples
///
/// ```
/// use sonara_core::pyfmt::py_round;
///
/// assert_eq!(py_round(2.675, 2), 2.67); // 2.675 is 2.67499999… in binary
/// assert_eq!(py_round(0.125, 2), 0.12); // an exact tie goes to even
/// assert!(py_round(-0.001, 2).is_sign_negative());
/// ```
#[must_use]
pub fn py_round(x: f64, ndigits: u32) -> f64 {
    if !x.is_finite() || ndigits > PY_NDIGITS_MAX {
        return x;
    }
    let mut text = DecimalBuffer::new();
    let precision = usize::try_from(ndigits).unwrap_or(usize::MAX);
    if write!(text, "{x:.precision$}").is_err() {
        // The buffer holds the longest possible text (see `DecimalBuffer`).
        return x;
    }
    text.as_str().and_then(|s| s.parse().ok()).unwrap_or(x)
}

/// A fixed buffer for one formatted float: a sign, the 309 integer digits of `f64::MAX`, the
/// point and [`PY_NDIGITS_MAX`] decimals fit in 634 bytes.
struct DecimalBuffer {
    bytes: [u8; 640],
    len: usize,
}

impl DecimalBuffer {
    fn new() -> Self {
        Self {
            bytes: [0; 640],
            len: 0,
        }
    }

    fn as_str(&self) -> Option<&str> {
        self.bytes
            .get(..self.len)
            .and_then(|b| std::str::from_utf8(b).ok())
    }
}

impl std::fmt::Write for DecimalBuffer {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let end = self.len.checked_add(s.len()).ok_or(std::fmt::Error)?;
        let slot = self.bytes.get_mut(self.len..end).ok_or(std::fmt::Error)?;
        slot.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

/// `1.500` becomes `1.5` and `2.000` becomes `2`; text without a decimal point is unchanged.
fn strip_fraction_zeros(text: &str) -> String {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text.to_owned()
    }
}

/// Formats strings like Python's `repr` of a list of strings: `['a', 'b']`.
#[must_use]
pub fn str_list_repr<S: AsRef<str>>(items: &[S]) -> String {
    let parts: Vec<String> = items.iter().map(|s| str_repr(s.as_ref())).collect();
    format!("[{}]", parts.join(", "))
}

/// Formats `value` like Python's `repr(str)`: single quotes unless the text contains a single
/// quote and no double quote, with backslash escapes for quotes, backslashes and non-printable
/// characters.
#[must_use]
pub fn str_repr(value: &str) -> String {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(value.len() + 2);
    out.push(quote);
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if is_printable(c) => out.push(c),
            c => {
                let code = u32::from(c);
                let escaped = if code < 0x100 {
                    format!("\\x{code:02x}")
                } else if code < 0x1_0000 {
                    format!("\\u{code:04x}")
                } else {
                    format!("\\U{code:08x}")
                };
                out.push_str(&escaped);
            }
        }
    }
    out.push(quote);
    out
}

/// Python's `str.isprintable` for one character, approximated by the Unicode categories that
/// Python treats as non-printable: control characters, separators other than the ASCII space,
/// and unassigned or private-use code points are escaped.
fn is_printable(c: char) -> bool {
    if c == ' ' {
        return true;
    }
    !(c.is_control() || c.is_whitespace() || matches!(u32::from(c), 0xE000..=0xF8FF | 0xF_0000..))
}

#[cfg(test)]
mod tests {
    use super::{float_g, float_repr, py_round, str_list_repr, str_repr};

    #[test]
    fn round_matches_python() {
        // Values from CPython 3.12's `round(x, n)`.
        let cases = [
            (2.675, 2, 2.67),
            (0.125, 2, 0.12),
            (0.375, 2, 0.38),
            (15.645_875, 3, 15.646),
            (-0.000_4, 3, -0.0),
            (1.0e300, 2, 1.0e300),
            (33.856_000_000_000_01, 3, 33.856),
            (123.456, 0, 123.0),
            (0.1 + 0.2, 400, 0.1 + 0.2),
            (0.123_45, 4, 0.1235),
            (1.0005, 3, 1.0),
            (30.967_741_935_483_87, 4, 30.9677),
            (1.000_000_5, 6, 1.000_001),
            (-0.000_000_1, 6, -0.0),
        ];
        for (x, digits, expected) in cases {
            let rounded = py_round(x, digits);
            assert_eq!(rounded, expected, "round({x}, {digits})");
            assert_eq!(rounded.is_sign_negative(), expected.is_sign_negative());
        }
        assert!(py_round(f64::NAN, 2).is_nan());
        assert_eq!(py_round(f64::NEG_INFINITY, 2), f64::NEG_INFINITY);
        // The longest text: every integer digit of f64::MAX and 323 decimals.
        assert_eq!(py_round(-f64::MAX, 323), -f64::MAX);
    }

    #[test]
    fn g_format_matches_python() {
        let cases = [
            (20.0, "20"),
            (0.001, "0.001"),
            (20000.0, "20000"),
            (1e-05, "1e-05"),
            (1_234_567.0, "1.23457e+06"),
            (-144.0, "-144"),
            (0.5, "0.5"),
            (123_456.0, "123456"),
            (999_999.5, "1e+06"),
            (0.0, "0"),
            (-0.0, "-0"),
            (1.0 / 3.0, "0.333333"),
            (f64::INFINITY, "inf"),
        ];
        for (value, expected) in cases {
            assert_eq!(float_g(value), expected, "{value:e}");
        }
        assert_eq!(str_list_repr(&["n_1", "it's"]), r#"['n_1', "it's"]"#);
        assert_eq!(str_list_repr::<&str>(&[]), "[]");
    }

    #[test]
    fn floats_match_python_repr() {
        let cases = [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (4.0, "4.0"),
            (120.0, "120.0"),
            (0.1, "0.1"),
            (1.0 / 3.0, "0.3333333333333333"),
            (1e-4, "0.0001"),
            (1e-5, "1e-05"),
            (1.5e-7, "1.5e-07"),
            (123_456_789.125, "123456789.125"),
            (1e15, "1000000000000000.0"),
            (1e16, "1e+16"),
            (2.5e300, "2.5e+300"),
            (-7.25, "-7.25"),
            (f64::INFINITY, "inf"),
            (f64::NEG_INFINITY, "-inf"),
            (f64::NAN, "nan"),
        ];
        for (value, expected) in cases {
            assert_eq!(float_repr(value), expected, "{value:e}");
        }
    }

    #[test]
    fn strings_match_python_repr() {
        assert_eq!(str_repr("trk_1"), "'trk_1'");
        assert_eq!(str_repr("it's"), "\"it's\"");
        assert_eq!(str_repr("a'b\"c"), "'a\\'b\"c'");
        assert_eq!(str_repr("line\nbreak\\"), "'line\\nbreak\\\\'");
        assert_eq!(str_repr("Třináctá"), "'Třináctá'");
        assert_eq!(str_repr("\u{7}"), "'\\x07'");
    }
}
