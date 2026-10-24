//! Python renderings of JSON values that the store's files and messages carry.
//!
//! - [`repr`] and [`py_str`]: Python's `repr()` and `str()` of a value that `json.loads`
//!   returned, for error messages (`'48000' is not one of [22050, …]`, `{'a': [1, True]}`).
//! - [`dumps_indented`]: `json.dumps(value, indent=1) + "\n"`, the manifest's bytes.
//! - [`dumps_compact`]: `json.dumps(value)`, the bytes of `workspace.json` and the autosave's
//!   `info.json`.
//!
//! Both writers keep Python's default `ensure_ascii=True`: every character outside `' '..='~'`
//! is written as a lowercase `\uXXXX` escape, with a surrogate pair above the BMP. The
//! indented writer reuses [`crate::canonical`] (which writes non-ASCII text as UTF-8) and
//! escapes its output; `serde_json` already escapes quotes, backslashes and control characters
//! exactly like Python, so only the remaining non-ASCII characters and DEL need escaping.

use std::fmt::Write as _;
use std::io;

use serde::Serialize;
use serde_json::ser::Formatter;
use serde_json::{Number, Serializer, Value};
use sonara_core::pyfmt::{float_repr, str_repr};

use crate::canonical::to_canonical_bytes;

/// Python's `repr` of a JSON value: `None`, `True`, `1`, `1.5`, `'text'`, `[1, 2]`,
/// `{'key': 'value'}`.
#[must_use]
pub(crate) fn repr(value: &Value) -> String {
    let mut out = String::new();
    write_repr(&mut out, value);
    out
}

/// Python's `str` of a JSON value: a string as it is, anything else as its `repr`.
#[must_use]
pub(crate) fn py_str(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => repr(other),
    }
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

/// Integers as integers, floats like Python's `repr(float)`.
///
/// `serde_json` reads an integer beyond the 64-bit range as a float, so such a value prints as
/// `1e+30` where Python prints all of its digits.
fn number_repr(number: &Number) -> String {
    if let Some(int) = number.as_i64() {
        int.to_string()
    } else if let Some(uint) = number.as_u64() {
        uint.to_string()
    } else {
        float_repr(number.as_f64().unwrap_or(f64::NAN))
    }
}

/// `json.dumps(value, indent=1) + "\n"`.
#[must_use]
pub(crate) fn dumps_indented(value: &Value) -> Vec<u8> {
    ascii_escape(&to_canonical_bytes(value))
}

/// `json.dumps(value)`: `", "` and `": "` separators, no newline.
///
/// # Panics
///
/// Never in practice: a [`Value`] has only string keys and finite numbers, and writing into
/// memory cannot fail.
#[must_use]
pub(crate) fn dumps_compact(value: &Value) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    let mut serializer = Serializer::with_formatter(&mut out, PythonCompact);
    value
        .serialize(&mut serializer)
        .expect("serializing a JSON value into memory cannot fail");
    ascii_escape(&out)
}

/// Escapes every character outside `' '..='~'` that `serde_json` left as it is: non-ASCII
/// characters and DEL. Non-ASCII text only occurs inside JSON strings, so the escapes are
/// always valid.
fn ascii_escape(json: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(json);
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_ascii() && ch != '\x7f' {
            out.push(ch);
        } else {
            let mut units = [0_u16; 2];
            for unit in ch.encode_utf16(&mut units) {
                // `fmt::Write` for `String` never fails.
                let _ = write!(out, "\\u{unit:04x}");
            }
        }
    }
    out.into_bytes()
}

/// Python's default `json.dumps` layout, with Python float formatting.
struct PythonCompact;

impl Formatter for PythonCompact {
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
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{dumps_compact, dumps_indented, py_str, repr};

    // Expected texts are the output of Python 3.13's `repr`, `json.dumps(v)` and
    // `json.dumps(v, indent=1)` for the same values.

    #[test]
    fn repr_matches_python() {
        let value = json!({"a": [1, -2, 1.5, 1e-07, 1e16, -0.0, true, false, null],
                           "it's": "x\"y\n\u{1}", "nested": {"k": []}, "empty": {}});
        assert_eq!(
            repr(&value),
            "{'a': [1, -2, 1.5, 1e-07, 1e+16, -0.0, True, False, None], \"it's\": 'x\"y\\n\\x01', \
             'nested': {'k': []}, 'empty': {}}"
        );
        assert_eq!(
            repr(&json!(18_446_744_073_709_551_615_u64)),
            "18446744073709551615"
        );
        assert_eq!(py_str(&json!("plain")), "plain");
        assert_eq!(py_str(&json!(null)), "None");
    }

    #[test]
    fn compact_matches_python_json_dumps() {
        let value =
            json!({"kind": "scratch", "name": "Třináctá ✓ 😀\u{7f}", "n": [1, 2.0], "x": {}});
        assert_eq!(
            String::from_utf8(dumps_compact(&value)).expect("ASCII"),
            r#"{"kind": "scratch", "name": "T\u0159in\u00e1ct\u00e1 \u2713 \ud83d\ude00\u007f", "n": [1, 2.0], "x": {}}"#
        );
    }

    #[test]
    fn indented_matches_python_json_dumps() {
        let value = json!({"path": "media/audio/med_1-Ünï.wav", "list": ["a"], "empty": []});
        assert_eq!(
            String::from_utf8(dumps_indented(&value)).expect("ASCII"),
            "{\n \"path\": \"media/audio/med_1-\\u00dcn\\u00ef.wav\",\n \"list\": [\n  \"a\"\n ],\n \
             \"empty\": []\n}\n"
        );
    }
}
