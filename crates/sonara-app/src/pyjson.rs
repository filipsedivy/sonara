//! Python's `json.dumps(value, indent=N)` with its defaults (`ensure_ascii=True`): the text of
//! preview analysis sidecars (`indent=1`, [`crate::render`]), of MCP tool results (`indent=1`)
//! and of everything the `sonara` command prints (`indent=2`) or writes (`batch --results`,
//! `indent=1`).
//!
//! The layout is `serde_json`'s pretty printer with an `N`-space indent, `": "` and `","`
//! separators, and empty containers as `[]` and `{}`, as Python writes them. Floats are formatted
//! like Python's `repr(float)` ([`float_repr`]); integers and floats stay distinct because
//! [`Value`] keeps them apart. `serde_json` already escapes quotes, backslashes and control
//! characters exactly like Python, so only the remaining non-ASCII characters and DEL are escaped
//! afterwards, as lowercase `\uXXXX` escapes with a surrogate pair above the BMP.

use std::fmt::Write as _;
use std::io;

use serde::Serialize;
use serde_json::ser::{Formatter, PrettyFormatter};
use serde_json::{Serializer, Value};
use sonara_core::pyfmt::float_repr;

/// `json.dumps(value, indent=indent)`, without a trailing newline.
///
/// # Panics
///
/// Never in practice: a [`Value`] has only string keys and finite numbers, and writing into
/// memory cannot fail.
#[must_use]
pub fn dumps(value: &Value, indent: usize) -> String {
    let indent = vec![b' '; indent];
    let mut out = Vec::with_capacity(1024);
    let mut serializer = Serializer::with_formatter(
        &mut out,
        PythonPretty {
            pretty: PrettyFormatter::with_indent(&indent),
        },
    );
    value
        .serialize(&mut serializer)
        .expect("serializing a JSON value into memory cannot fail");
    ascii_escape(&String::from_utf8_lossy(&out))
}

/// Escapes every character outside `' '..='~'` that `serde_json` left as it is: non-ASCII
/// characters and DEL. They only occur inside JSON strings, so the escapes are always valid.
fn ascii_escape(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    for ch in json.chars() {
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
    out
}

/// [`PrettyFormatter`] with Python float formatting.
struct PythonPretty<'a> {
    pretty: PrettyFormatter<'a>,
}

impl Formatter for PythonPretty<'_> {
    fn write_f32<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: f32) -> io::Result<()> {
        self.write_f64(writer, f64::from(value))
    }

    fn write_f64<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        writer.write_all(float_repr(value).as_bytes())
    }

    fn begin_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.begin_array(writer)
    }

    fn end_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.end_array(writer)
    }

    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.pretty.begin_array_value(writer, first)
    }

    fn end_array_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.end_array_value(writer)
    }

    fn begin_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.begin_object(writer)
    }

    fn end_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.end_object(writer)
    }

    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.pretty.begin_object_key(writer, first)
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.begin_object_value(writer)
    }

    fn end_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.end_object_value(writer)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::dumps;

    // Expected texts are the output of Python 3.13's `json.dumps(value, indent=N)` for the same
    // values.

    #[test]
    fn indent_two_matches_python() {
        let value = json!({
            "name": "T\u{159}in\u{e1}ct\u{e1} \"hodina\"\n\u{1}\u{7f} \u{1f600}",
            "empty_list": [],
            "empty_map": {},
            "ints": [-8192, 0, 18_446_744_073_709_551_615_u64],
            "floats": [4.0, 0.1, 1e-05, 1e16, -0.0, 123_456.789],
            "nested": {"a": [true, false, null]}
        });
        let expected = "{\n  \"name\": \"T\\u0159in\\u00e1ct\\u00e1 \\\"hodina\\\"\\n\\u0001\\u007f \
                        \\ud83d\\ude00\",\n  \"empty_list\": [],\n  \"empty_map\": {},\n  \
                        \"ints\": [\n    -8192,\n    0,\n    18446744073709551615\n  ],\n  \
                        \"floats\": [\n    4.0,\n    0.1,\n    1e-05,\n    1e+16,\n    -0.0,\n    \
                        123456.789\n  ],\n  \"nested\": {\n    \"a\": [\n      true,\n      \
                        false,\n      null\n    ]\n  }\n}";
        assert_eq!(dumps(&value, 2), expected);
    }

    #[test]
    fn indent_one_matches_python() {
        let value = json!({"ok": true, "list": [1.5], "text": "\u{e9}"});
        assert_eq!(
            dumps(&value, 1),
            "{\n \"ok\": true,\n \"list\": [\n  1.5\n ],\n \"text\": \"\\u00e9\"\n}"
        );
    }

    #[test]
    fn scalars_have_no_layout() {
        assert_eq!(dumps(&json!("x"), 2), "\"x\"");
        assert_eq!(dumps(&json!([]), 2), "[]");
        assert_eq!(dumps(&json!({}), 1), "{}");
    }
}
