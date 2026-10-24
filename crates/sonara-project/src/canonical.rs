//! Canonical JSON text.
//!
//! The prototype writes `project.json` with `json.dumps(indent=1, ensure_ascii=False)` plus a
//! trailing newline. The bytes are part of the file-format contract (content hashes, golden
//! data), so this writer reproduces them: one-space indentation, `": "` and `","` separators,
//! non-ASCII text written as UTF-8, and floats formatted like Python's `repr(float)`
//! ([`sonara_core::pyfmt::float_repr`]). Integers and floats stay distinct because
//! [`serde_json::Value`] keeps them apart.

use std::io;

use serde::Serialize;
use serde_json::ser::{Formatter, PrettyFormatter};
use serde_json::{Serializer, Value};
use sonara_core::pyfmt::float_repr;

/// The canonical bytes of `value`, ending with a newline.
///
/// # Panics
///
/// Never in practice: a [`Value`] has only string keys and finite numbers, and writing into
/// memory cannot fail.
#[must_use]
pub fn to_canonical_bytes(value: &Value) -> Vec<u8> {
    let mut out = Vec::with_capacity(4096);
    let mut serializer = Serializer::with_formatter(&mut out, CanonicalFormatter::new());
    value
        .serialize(&mut serializer)
        .expect("serializing a JSON value into memory cannot fail");
    out.push(b'\n');
    out
}

/// [`PrettyFormatter`] with a one-space indent and Python float formatting.
struct CanonicalFormatter<'a> {
    pretty: PrettyFormatter<'a>,
}

impl CanonicalFormatter<'_> {
    fn new() -> Self {
        Self {
            pretty: PrettyFormatter::with_indent(b" "),
        }
    }
}

impl Formatter for CanonicalFormatter<'_> {
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

    use super::to_canonical_bytes;

    #[test]
    fn matches_python_json_dumps() {
        let value = json!({
            "name": "Třináctá \"hodina\"\n",
            "empty_list": [],
            "empty_map": {},
            "ints": [-8192, 0, 127],
            "floats": [4.0, 0.1, 1e-05, 1e16, -0.0],
            "null": null,
            "flag": true
        });
        let expected = "{\n \"name\": \"Třináctá \\\"hodina\\\"\\n\",\n \"empty_list\": [],\n \
                        \"empty_map\": {},\n \"ints\": [\n  -8192,\n  0,\n  127\n ],\n \
                        \"floats\": [\n  4.0,\n  0.1,\n  1e-05,\n  1e+16,\n  -0.0\n ],\n \
                        \"null\": null,\n \"flag\": true\n}\n";
        assert_eq!(
            String::from_utf8(to_canonical_bytes(&value)).expect("UTF-8"),
            expected
        );
    }
}
