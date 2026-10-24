//! pydantic-core's lax coercions of strings and floats to `bool`, `int` and `float`, ported from
//! its `src/input/shared.rs` and checked against the prototype's pydantic-core 2.50.1 by the
//! validation fixtures. pydantic-core is written in Rust, so the port keeps its algorithms and
//! its use of `str::trim` and `str::parse::<f64>`, and reproduces its results exactly.
//!
//! One deliberate difference: a valid integer outside the `i64` range is
//! [`IntError::Size`] here, where Python would keep an arbitrary-precision `int`. The normalized
//! arguments are JSON values (`serde_json` without arbitrary precision), so such a number cannot
//! be carried on.

use std::borrow::Cow;

/// Why a value is not an integer (pydantic's error types).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntError {
    /// `int_parsing`: the string is not an integer.
    Parsing,
    /// `int_parsing_size`: the number is outside the supported range.
    Size,
    /// `int_from_float`: the float has a fractional part.
    FromFloat,
    /// `finite_number`: the float is infinite or NaN.
    Finite,
}

/// `str_as_bool`: the accepted spellings, without trimming.
pub(crate) fn str_as_bool(text: &str) -> Option<bool> {
    const FALSE: [&str; 5] = ["f", "n", "no", "off", "false"];
    const TRUE: [&str; 5] = ["t", "y", "on", "yes", "true"];
    if text == "0" || FALSE.iter().any(|word| text.eq_ignore_ascii_case(word)) {
        Some(false)
    } else if text == "1" || TRUE.iter().any(|word| text.eq_ignore_ascii_case(word)) {
        Some(true)
    } else {
        None
    }
}

/// `int_as_bool`: only 0 and 1 are booleans.
pub(crate) fn int_as_bool(value: i64) -> Option<bool> {
    match value {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// `i64::MIN as f64`: -2^63.
const I64_MIN_AS_F64: f64 = -9_223_372_036_854_775_808.0;
/// `i64::MAX as f64`: 2^63 - 1 rounds to 2^63.
const I64_MAX_AS_F64: f64 = 9_223_372_036_854_775_808.0;

/// `float_as_int`: an integral float strictly inside the `i64` range (`i64::MIN as f64 < value <
/// i64::MAX as f64`, as pydantic-core compares).
pub(crate) fn float_as_int(value: f64) -> Result<i64, IntError> {
    if value.is_infinite() || value.is_nan() {
        Err(IntError::Finite)
    } else if value.fract() != 0.0 {
        Err(IntError::FromFloat)
    } else if I64_MIN_AS_F64 < value && value < I64_MAX_AS_F64 {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the value is integral and inside the i64 range"
        )]
        Ok(value as i64)
    } else {
        Err(IntError::Size)
    }
}

/// `str_as_int`: the string as a JSON integer, or else after `clean_int_str`.
pub(crate) fn str_as_int(text: &str) -> Result<i64, IntError> {
    if let Some(parsed) = json_int(text) {
        return parsed;
    }
    match clean_int_str(text) {
        Some(cleaned) => json_int(&cleaned).unwrap_or(Err(IntError::Parsing)),
        None => Err(IntError::Parsing),
    }
}

/// `str_as_float`: `str::parse` on the trimmed string, then on the untrimmed string without its
/// underscores.
pub(crate) fn str_as_float(text: &str) -> Option<f64> {
    text.trim()
        .parse()
        .ok()
        .or_else(|| strip_underscores(text).and_then(|stripped| stripped.parse().ok()))
}

/// jiter's `NumberInt::try_from` on the whole string: `None` unless it is a JSON integer
/// (`-?(0|[1-9][0-9]*)`).
fn json_int(text: &str) -> Option<Result<i64, IntError>> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    let valid = match digits.as_bytes() {
        [] => false,
        [b'0'] => true,
        [first, rest @ ..] => (b'1'..=b'9').contains(first) && rest.iter().all(u8::is_ascii_digit),
    };
    valid.then(|| text.parse::<i64>().map_err(|_| IntError::Size))
}

/// `strip_underscores`: Python's rule for digit separators, loosely (no leading, trailing or
/// doubled underscore).
fn strip_underscores(text: &str) -> Option<String> {
    if text.starts_with('_') || text.ends_with('_') || !text.contains('_') || text.contains("__") {
        None
    } else {
        Some(text.replace('_', ""))
    }
}

/// `clean_int_str`: trims, drops a unary plus, leading zeros, a `.0…` fraction and digit
/// separators. `None` when nothing could be cleaned or the input is malformed.
fn clean_int_str(text: &str) -> Option<Cow<'_, str>> {
    let len_before = text.len();
    let mut s = text.trim();
    if let Some(suffix) = s.strip_prefix('+') {
        if suffix.starts_with('-') {
            return None;
        }
        s = suffix;
    }
    let mut is_negative = false;
    if let Some(suffix) = s.strip_prefix('-') {
        if suffix.starts_with('-') || suffix.starts_with('+') {
            return None;
        }
        is_negative = true;
        s = suffix;
    }
    s = strip_leading_zeros(s)?;
    if let Some((integer, decimals)) = s.split_once('.')
        && !decimals.is_empty()
        && decimals.chars().all(|c| c == '0')
    {
        s = integer;
    }
    let sign = if is_negative { "-" } else { "" };
    if let Some(stripped) = strip_underscores(s) {
        return Some(Cow::Owned(format!("{sign}{stripped}")));
    }
    if len_before == s.len() {
        return None;
    }
    Some(if is_negative {
        Cow::Owned(format!("-{s}"))
    } else {
        Cow::Borrowed(s)
    })
}

/// `strip_leading_zeros`: keeps one zero for an all-zero number and before a decimal point;
/// underscores among the leading zeros are dropped too.
fn strip_leading_zeros(s: &str) -> Option<&str> {
    let mut chars = s.char_indices();
    match chars.next() {
        Some((_, '0')) => {}
        Some((_, c)) if c.is_ascii_digit() || c == '-' => return Some(s),
        _ => return None,
    }
    for (i, c) in chars {
        match c {
            '0' | '_' => {}
            '1'..='9' | '-' => return s.get(i..),
            // The previous character is the ASCII zero just before the point.
            '.' => return s.get(i - 1..),
            _ => return None,
        }
    }
    s.get(s.len() - 1..)
}

#[cfg(test)]
mod tests {
    use super::{IntError, float_as_int, str_as_bool, str_as_float, str_as_int};

    #[test]
    fn integers_from_strings_follow_pydantic_core() {
        let accepted = [
            ("3", 3),
            (" 3 ", 3),
            ("+3", 3),
            ("-3", -3),
            ("00012", 12),
            ("-00012", -12),
            ("1_000", 1000),
            ("1_000.0", 1000),
            ("1.00", 1),
            ("-0.0", 0),
            ("0_0", 0),
            ("9223372036854775807", i64::MAX),
        ];
        for (text, value) in accepted {
            assert_eq!(str_as_int(text), Ok(value), "{text:?}");
        }
        let rejected = [
            "1.", ".0", "0.", "3.5", "1e3", "1__0", "_1000", "1000_", "-_1", "+_1", "--1", "+-1",
            "1.000_0", "0x10", "", " ", "+", "-", "١٢",
        ];
        for text in rejected {
            assert_eq!(str_as_int(text), Err(IntError::Parsing), "{text:?}");
        }
        assert_eq!(str_as_int("9223372036854775808"), Err(IntError::Size));
    }

    #[test]
    fn floats_from_strings_follow_pydantic_core() {
        let accepted = [
            ("2.5", 2.5),
            (" 3.5 ", 3.5),
            ("1e3", 1000.0),
            ("1_000.5_5", 1000.55),
            ("1_.5", 1.5),
            (".5", 0.5),
            ("5.", 5.0),
            ("+1.5", 1.5),
        ];
        for (text, value) in accepted {
            assert_eq!(str_as_float(text), Some(value), "{text:?}");
        }
        for text in [" 1_000 ", "_1", "1_", "1__0", "0x10", "1e", "", "abc"] {
            assert_eq!(str_as_float(text), None, "{text:?}");
        }
        assert_eq!(str_as_float("in_f"), Some(f64::INFINITY));
        assert!(str_as_float("nan").is_some_and(f64::is_nan));
    }

    #[test]
    fn floats_to_integers() {
        assert_eq!(float_as_int(3.0), Ok(3));
        assert_eq!(float_as_int(-0.0), Ok(0));
        assert_eq!(float_as_int(3.5), Err(IntError::FromFloat));
        assert_eq!(float_as_int(9.3e18), Err(IntError::Size));
        assert_eq!(float_as_int(f64::NAN), Err(IntError::Finite));
        assert_eq!(float_as_int(-(2f64.powi(63))), Err(IntError::Size));
        assert_eq!(float_as_int(2f64.powi(63)), Err(IntError::Size));
        assert_eq!(float_as_int(-9.2e18), Ok(-9_200_000_000_000_000_000));
    }

    #[test]
    fn booleans_from_strings() {
        assert_eq!(str_as_bool("YES"), Some(true));
        assert_eq!(str_as_bool("off"), Some(false));
        assert_eq!(str_as_bool(" true "), None);
        assert_eq!(str_as_bool("1.0"), None);
    }
}
