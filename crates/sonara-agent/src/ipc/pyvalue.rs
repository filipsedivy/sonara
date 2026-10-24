//! The Python semantics that the prototype's IPC code applies to JSON values it received:
//! truthiness (`x or {}`), `str()` and `repr()`, `int()` and `float()`, attribute lookups on
//! values that are not dicts, and `json.dumps` as the bytes on the wire.
//!
//! [`dumps`] repeats the layout of `sonara-project`'s private Python writer (DES-01: that writer
//! is crate-private and this step's file set does not include `sonara-project`; the shared parts,
//! `float_repr` and `str_repr`, already come from `sonara_core::pyfmt`).

use std::io;

use serde::Serialize as _;
use serde_json::ser::Formatter;
use serde_json::{Number, Serializer, Value};
use sonara_core::pyfmt::{float_repr, str_repr};

use crate::pystr::{is_py_space, py_float};

/// Python's truthiness of a JSON value: `null`, `false`, `0`, `0.0`, `""`, `[]` and `{}` are
/// false.
pub(crate) fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|n| n != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}

/// `value.get(key)` on what `x or {}` left: `Ok(None)` for a missing key (or a falsy `x`,
/// which became `{}`), `Err(type name)` where Python raises `AttributeError` because the
/// value is not a dict.
pub(crate) fn dict_get<'v>(
    value: Option<&'v Value>,
    key: &str,
) -> Result<Option<&'v Value>, &'static str> {
    match value {
        None => Ok(None),
        Some(Value::Object(map)) => Ok(map.get(key)),
        Some(other) => Err(type_name(other)),
    }
}

/// `x or {}` as a lookup source: `None` stands for the empty dict.
pub(crate) fn or_empty(value: Option<&Value>) -> Option<&Value> {
    value.filter(|v| is_truthy(v))
}

/// The Python type name of a JSON value (`type(x).__name__`).
pub(crate) fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(number) if number.is_f64() => "float",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

/// Python's `str()` of a JSON value: a string as it is, anything else as its `repr()`.
pub(crate) fn py_str(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => py_repr(other),
    }
}

/// Python's `repr()` of a JSON value: `None`, `True`, `1`, `1.5`, `'text'`, `[1, 2]`,
/// `{'key': 'value'}`.
pub(crate) fn py_repr(value: &Value) -> String {
    let mut out = String::new();
    write_repr(&mut out, value);
    out
}

fn write_repr(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("None"),
        Value::Bool(true) => out.push_str("True"),
        Value::Bool(false) => out.push_str("False"),
        Value::Number(number) => out.push_str(&number_repr(number)),
        Value::String(text) => out.push_str(&str_repr(text)),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_repr(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            for (i, (key, item)) in map.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&str_repr(key));
                out.push_str(": ");
                write_repr(out, item);
            }
            out.push('}');
        }
    }
}

/// Integers as integers, floats like Python's `repr(float)`. `serde_json` reads an integer
/// beyond the 64-bit range as a float, so such a value prints as `1e+30` where Python prints
/// all of its digits.
fn number_repr(number: &Number) -> String {
    if let Some(int) = number.as_i64() {
        int.to_string()
    } else if let Some(uint) = number.as_u64() {
        uint.to_string()
    } else {
        float_repr(number.as_f64().unwrap_or(f64::NAN))
    }
}

/// The result of Python's `int()`, as far as the IPC code needs it: compared with small
/// values (the protocol version) or used as a process id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PyInt {
    /// A value in the 64-bit range.
    Small(i64),
    /// A valid integer beyond the 64-bit range, which equals no small value.
    Large,
}

/// Python's `int(value)` of a JSON value; `None` where Python raises (`null`, lists, dicts and
/// strings that are no decimal integer).
///
/// Strings follow `int(str)`: white space around, an optional sign, decimal digits with single
/// underscores between them. One difference remains: Python also reads non-ASCII decimal digits
/// (`int('١')` is `1`), which this function rejects.
pub(crate) fn py_int(value: &Value) -> Option<PyInt> {
    match value {
        Value::Bool(flag) => Some(PyInt::Small(i64::from(*flag))),
        Value::Number(number) => Some(number_int(number)),
        Value::String(text) => text_int(text),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

/// `int()` of a JSON number: floats are truncated toward zero.
fn number_int(number: &Number) -> PyInt {
    if let Some(int) = number.as_i64() {
        return PyInt::Small(int);
    }
    if number.is_u64() {
        return PyInt::Large;
    }
    let truncated = number.as_f64().unwrap_or(0.0).trunc();
    // The range check makes the conversion exact: every integral f64 in this range is an i64.
    if (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&truncated) {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "an integral value inside the i64 range converts exactly"
        )]
        let int = truncated as i64;
        PyInt::Small(int)
    } else {
        PyInt::Large
    }
}

/// `int(str)` in base 10.
fn text_int(text: &str) -> Option<PyInt> {
    let text = text.trim_matches(is_py_space);
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let well_formed = !digits.is_empty()
        && !digits.starts_with('_')
        && !digits.ends_with('_')
        && !digits.contains("__")
        && digits.chars().all(|c| c == '_' || c.is_ascii_digit());
    if !well_formed {
        return None;
    }
    let mut value: i64 = 0;
    for digit in digits.chars().filter_map(|c| c.to_digit(10)) {
        match value
            .checked_mul(10)
            .and_then(|v| v.checked_add(i64::from(digit)))
        {
            Some(next) => value = next,
            None => return Some(PyInt::Large),
        }
    }
    Some(PyInt::Small(if negative { -value } else { value }))
}

/// Python's `float(value)` of a JSON value; `None` where Python raises.
pub(crate) fn py_float_value(value: &Value) -> Option<f64> {
    match value {
        Value::Bool(flag) => Some(if *flag { 1.0 } else { 0.0 }),
        Value::Number(number) => number.as_f64(),
        Value::String(text) => py_float(text),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

/// `json.dumps(value)`: Python's default layout (`", "` and `": "` separators) with
/// `ensure_ascii=True` (every character outside `' '..='~'` as a lowercase `\uXXXX` escape, a
/// surrogate pair above the BMP) and Python's float formatting.
///
/// # Panics
///
/// Never in practice: a [`Value`] has only string keys and finite numbers, and writing into
/// memory cannot fail.
pub(crate) fn dumps(value: &Value) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    let mut serializer = Serializer::with_formatter(&mut out, PythonJson);
    value
        .serialize(&mut serializer)
        .expect("serializing a JSON value into memory cannot fail");
    out
}

/// Python's default `json.dumps` layout and escapes.
struct PythonJson;

impl Formatter for PythonJson {
    fn write_f32<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: f32) -> io::Result<()> {
        self.write_f64(writer, f64::from(value))
    }

    fn write_f64<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        writer.write_all(float_repr(value).as_bytes())
    }

    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        writer.write_all(b": ")
    }

    /// `serde_json` already escapes quotes, backslashes and control characters exactly like
    /// Python; the unescaped runs it passes here may still hold non-ASCII characters and DEL.
    fn write_string_fragment<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        let mut plain = 0;
        for (at, ch) in fragment.char_indices() {
            if ch.is_ascii() && ch != '\x7f' {
                continue;
            }
            writer.write_all(fragment.get(plain..at).unwrap_or_default().as_bytes())?;
            let mut units = [0_u16; 2];
            for unit in ch.encode_utf16(&mut units) {
                write!(writer, "\\u{unit:04x}")?;
            }
            plain = at + ch.len_utf8();
        }
        writer.write_all(fragment.get(plain..).unwrap_or_default().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{PyInt, dict_get, dumps, is_truthy, py_float_value, py_int, py_repr, py_str};

    // Expected texts are CPython 3.13's `repr`, `str`, `int`, `float` and `json.dumps` of the
    // same values.

    #[test]
    fn dumps_matches_python_json_dumps() {
        let value = json!({"kind": "x", "name": "Třináctá ✓ 😀\u{7f}\u{1}\n\"\\", "n": [1, 2.0, 1e-7, 1e16, -0.0],
                           "big": 18_446_744_073_709_551_615_u64, "x": {}, "y": [], "z": null, "t": true});
        assert_eq!(
            String::from_utf8(dumps(&value)).expect("ASCII"),
            r#"{"kind": "x", "name": "T\u0159in\u00e1ct\u00e1 \u2713 \ud83d\ude00\u007f\u0001\n\"\\", "n": [1, 2.0, 1e-07, 1e+16, -0.0], "big": 18446744073709551615, "x": {}, "y": [], "z": null, "t": true}"#
        );
    }

    #[test]
    fn repr_and_str_match_python() {
        let value = json!({"a": [1, -2, 1.5, true, false, null], "it's": "x\"y\n"});
        assert_eq!(
            py_repr(&value),
            "{'a': [1, -2, 1.5, True, False, None], \"it's\": 'x\"y\\n'}"
        );
        assert_eq!(py_str(&json!("plain")), "plain");
        assert_eq!(py_str(&json!(5)), "5");
        assert_eq!(py_str(&json!(null)), "None");
        assert_eq!(py_str(&json!(["a"])), "['a']");
    }

    #[test]
    fn truthiness_matches_python() {
        for falsy in [
            json!(null),
            json!(false),
            json!(0),
            json!(0.0),
            json!(-0.0),
            json!(""),
            json!([]),
            json!({}),
        ] {
            assert!(!is_truthy(&falsy), "{falsy}");
        }
        for truthy in [
            json!(true),
            json!(1),
            json!(0.5),
            json!(" "),
            json!([0]),
            json!({"a": 0}),
        ] {
            assert!(is_truthy(&truthy), "{truthy}");
        }
    }

    #[test]
    fn dict_get_raises_on_non_dicts() {
        assert_eq!(dict_get(None, "k"), Ok(None));
        assert_eq!(dict_get(Some(&json!({"k": 1})), "k"), Ok(Some(&json!(1))));
        assert_eq!(dict_get(Some(&json!([1])), "k"), Err("list"));
        assert_eq!(dict_get(Some(&json!("x")), "k"), Err("str"));
        assert_eq!(dict_get(Some(&json!(5)), "k"), Err("int"));
        assert_eq!(dict_get(Some(&json!(1.5)), "k"), Err("float"));
        assert_eq!(dict_get(Some(&json!(true)), "k"), Err("bool"));
    }

    #[test]
    fn int_matches_python() {
        let cases = [
            (json!(1), Some(PyInt::Small(1))),
            (json!(1.9), Some(PyInt::Small(1))),
            (json!(-1.9), Some(PyInt::Small(-1))),
            (json!(true), Some(PyInt::Small(1))),
            (json!(false), Some(PyInt::Small(0))),
            (json!("1"), Some(PyInt::Small(1))),
            (json!(" \t1\n"), Some(PyInt::Small(1))),
            (json!("+1"), Some(PyInt::Small(1))),
            (json!("-0"), Some(PyInt::Small(0))),
            (json!("007"), Some(PyInt::Small(7))),
            (json!("1_0"), Some(PyInt::Small(10))),
            (json!("99999999999999999999"), Some(PyInt::Large)),
            (json!(18_446_744_073_709_551_615_u64), Some(PyInt::Large)),
            (json!(1e300), Some(PyInt::Large)),
            (json!("1__0"), None),
            (json!("_1"), None),
            (json!("1_"), None),
            (json!("+ 1"), None),
            (json!("1.0"), None),
            (json!("0x10"), None),
            (json!(""), None),
            (json!("abc"), None),
            (json!(null), None),
            (json!([1]), None),
            (json!({"a": 1}), None),
        ];
        for (value, expected) in cases {
            assert_eq!(py_int(&value), expected, "{value}");
        }
    }

    #[test]
    fn float_matches_python() {
        assert_eq!(py_float_value(&json!(2)), Some(2.0));
        assert_eq!(py_float_value(&json!(" 1.5 ")), Some(1.5));
        assert_eq!(py_float_value(&json!(true)), Some(1.0));
        assert_eq!(py_float_value(&json!(null)), None);
        assert_eq!(py_float_value(&json!("x")), None);
    }
}
