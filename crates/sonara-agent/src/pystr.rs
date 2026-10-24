//! The two pieces of Python string semantics that the argument conversion and the sandbox
//! reproduce: `str.strip()` and `float(str)`.
//!
//! These differ from pydantic's coercions in `contract::coerce`, which follow pydantic-core's
//! own (Rust) rules: the handlers run plain Python on values that validation already accepted.

/// Python's `str.isspace`: Unicode white space plus the separators U+001C to U+001F.
pub(crate) fn is_py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Python's `str.strip()`.
pub(crate) fn py_strip(text: &str) -> &str {
    text.trim_matches(is_py_space)
}

/// Python's `float(text)` for a `str`; `None` where Python raises `ValueError`.
///
/// Python strips white space, allows `_` only between two digits (PEP 515) and then reads the
/// same grammar as `str::parse::<f64>`: an optional sign, then decimal digits with an optional
/// point and exponent, or `inf`, `infinity` or `nan` in any case. Out-of-range values become
/// infinite in both. One difference remains: Python also reads non-ASCII decimal digits
/// (`float('١٢')` is `12.0`), which this function rejects.
pub(crate) fn py_float(text: &str) -> Option<f64> {
    let text = py_strip(text);
    if !text.contains('_') {
        return text.parse().ok();
    }
    let chars: Vec<char> = text.chars().collect();
    let digit_at = |i: Option<usize>| {
        i.and_then(|i| chars.get(i))
            .is_some_and(char::is_ascii_digit)
    };
    let well_placed = chars
        .iter()
        .enumerate()
        .all(|(i, c)| *c != '_' || (digit_at(i.checked_sub(1)) && digit_at(i.checked_add(1))));
    if !well_placed {
        return None;
    }
    let digits: String = chars.iter().filter(|c| **c != '_').collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{py_float, py_strip};

    #[test]
    fn strip_removes_python_white_space() {
        assert_eq!(py_strip("\u{1c} 3:1\t\u{2003}\u{1f}"), "3:1");
        assert_eq!(py_strip("\u{200b}x"), "\u{200b}x"); // not white space in Python either
    }

    #[test]
    fn floats_follow_python() {
        // Expected values from CPython 3.13 `float()`.
        let cases: [(&str, Option<f64>); 20] = [
            ("1", Some(1.0)),
            (" -2.5 ", Some(-2.5)),
            ("+.5", Some(0.5)),
            ("5.", Some(5.0)),
            ("1e3", Some(1000.0)),
            ("1E-2", Some(0.01)),
            ("1_000.000_1", Some(1000.0001)),
            ("1_0e1_0", Some(1e11)),
            ("1e400", Some(f64::INFINITY)),
            ("-Infinity", Some(f64::NEG_INFINITY)),
            ("iNf", Some(f64::INFINITY)),
            ("1__0", None),
            ("_1", None),
            ("1_", None),
            ("1_.5", None),
            ("1._5", None),
            ("", None),
            (".", None),
            ("1 0", None),
            ("0x10", None),
        ];
        for (text, expected) in cases {
            assert_eq!(py_float(text), expected, "{text:?}");
        }
        assert!(py_float("-nan").is_some_and(f64::is_nan));
        assert!(py_float("n_an").is_none());
    }
}
