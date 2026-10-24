//! JSON Schema validation of `project.json` and `manifest.json` (`validate_document` in
//! `sonara.persistence.store`).
//!
//! The two schemas (`schemas/*.schema.json`, Draft 2020-12) are part of the file-format contract
//! (CTR-01). They are embedded byte for byte and compiled once.
//!
//! Validation uses the [`jsonschema`] crate (QG-13), built without its HTTP and file resolvers:
//! the schemas reference only their own `$defs`. The crate reports the same errors as the
//! prototype's python-jsonschema 4.26 (the same count, and the same order for errors at one
//! location), but not the same wording. The message is therefore rebuilt from the crate's
//! structured error kind and Python's `repr` of the failing value, exactly as python-jsonschema
//! words it. The prototype reports the error with the smallest instance path (paths compared as
//! Python lists, ties in validation order) and the total count, so this module does too.
//!
//! One difference in matching remains: Python's `re` lets a final `$` match before a trailing
//! newline, so the prototype accepts an ID, colour or hash that ends in `"\n"`, while the
//! crate's ECMA-262 regexes reject it. Only malformed files are affected.

use std::cmp::Ordering;
use std::sync::LazyLock;

use jsonschema::error::{TypeKind, ValidationErrorKind};
use jsonschema::{Draft, ValidationError, Validator};
use serde_json::Value;
use sonara_core::error::{DomainError, ErrorCode, Result};
use sonara_core::pyfmt::str_repr;

use crate::pyjson::repr;

/// The project document schema (`project.json`).
pub const PROJECT_SCHEMA_TEXT: &str = include_str!("../schemas/project-v1.schema.json");
/// The manifest schema (`manifest.json`).
pub const MANIFEST_SCHEMA_TEXT: &str = include_str!("../schemas/manifest-v1.schema.json");

/// One of the two document schemas of the project format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schema {
    /// `project-v1.schema.json`.
    Project,
    /// `manifest-v1.schema.json`.
    Manifest,
}

impl Schema {
    /// The schema's file name, which prefixes validation messages.
    #[must_use]
    pub const fn file_name(self) -> &'static str {
        match self {
            Self::Project => "project-v1.schema.json",
            Self::Manifest => "manifest-v1.schema.json",
        }
    }

    /// The schema's text.
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::Project => PROJECT_SCHEMA_TEXT,
            Self::Manifest => MANIFEST_SCHEMA_TEXT,
        }
    }

    fn compiled(self) -> &'static Compiled {
        static PROJECT: LazyLock<Compiled> = LazyLock::new(|| Compiled::new(Schema::Project));
        static MANIFEST: LazyLock<Compiled> = LazyLock::new(|| Compiled::new(Schema::Manifest));
        match self {
            Self::Project => &PROJECT,
            Self::Manifest => &MANIFEST,
        }
    }
}

/// A parsed schema and its validator.
struct Compiled {
    schema: Value,
    validator: Validator,
}

impl Compiled {
    /// # Panics
    ///
    /// Never for the embedded schemas: the unit tests compile both.
    fn new(which: Schema) -> Self {
        let schema: Value = serde_json::from_str(which.text())
            .expect("the embedded schema is valid JSON (unit-tested)");
        let validator = jsonschema::options()
            .with_draft(Draft::Draft202012)
            .build(&schema)
            .expect("the embedded schema is a valid Draft 2020-12 schema (unit-tested)");
        Self { schema, validator }
    }
}

/// Validates `doc` against `schema`.
///
/// # Errors
///
/// [`ErrorCode::FormatError`] for an invalid document:
/// `"<schema file>: <path or (root)>: <message>"` for the first error, with the total count as
/// the `error_count` detail, for example
/// `"project-v1.schema.json: tracks: {} is not of type 'array'"`.
pub fn validate_document(doc: &Value, schema: Schema) -> Result<()> {
    let compiled = schema.compiled();
    let mut errors: Vec<(Vec<PathPart>, ValidationError<'_>)> = compiled
        .validator
        .iter_errors(doc)
        .map(|error| (path_parts(doc, error.instance_path().as_str()), error))
        .collect();
    // Stable, like Python's `sorted`: errors at one location keep their validation order.
    errors.sort_by(|a, b| a.0.cmp(&b.0));
    let count = errors.len();
    let Some((path, first)) = errors.first() else {
        return Ok(());
    };
    let joined = path
        .iter()
        .map(PathPart::to_string)
        .collect::<Vec<_>>()
        .join("/");
    let location = if joined.is_empty() {
        "(root)"
    } else {
        joined.as_str()
    };
    Err(DomainError::new(
        ErrorCode::FormatError,
        format!(
            "{}: {location}: {}",
            schema.file_name(),
            message(&compiled.schema, first)
        ),
    )
    .with_detail("error_count", count))
}

/// One step of an instance path, typed by the container it indexes (python-jsonschema's
/// `absolute_path` holds `int` for array items and `str` for object keys).
#[derive(Debug, Clone, PartialEq, Eq)]
enum PathPart {
    Index(usize),
    Key(String),
}

impl PartialOrd for PathPart {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PathPart {
    /// Python's comparison of `int` with `int` and `str` with `str` (code point order, which
    /// UTF-8 byte order preserves). Two paths that share a prefix index the same container, so
    /// an index never meets a key; that case only gets some fixed order.
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Index(a), Self::Index(b)) => a.cmp(b),
            (Self::Key(a), Self::Key(b)) => a.cmp(b),
            (Self::Index(_), Self::Key(_)) => Ordering::Less,
            (Self::Key(_), Self::Index(_)) => Ordering::Greater,
        }
    }
}

impl std::fmt::Display for PathPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Index(index) => write!(f, "{index}"),
            Self::Key(key) => f.write_str(key),
        }
    }
}

/// The steps of a JSON pointer into `doc`. The document decides whether a step is an index or
/// a key, so an object key such as `"0"` stays a key.
fn path_parts(doc: &Value, pointer: &str) -> Vec<PathPart> {
    let mut node = Some(doc);
    pointer
        .split('/')
        .skip(1)
        .map(|raw| {
            let key = raw.replace("~1", "/").replace("~0", "~");
            match node {
                Some(Value::Array(items)) => {
                    if let Ok(index) = key.parse::<usize>() {
                        node = items.get(index);
                        return PathPart::Index(index);
                    }
                    node = None;
                }
                Some(Value::Object(map)) => node = map.get(&key),
                _ => node = None,
            }
            PathPart::Key(key)
        })
        .collect()
}

/// python-jsonschema's message for `error` (`jsonschema/_keywords.py`, version 4.26).
fn message(schema: &Value, error: &ValidationError<'_>) -> String {
    let instance = repr(error.instance());
    match error.kind() {
        ValidationErrorKind::Required { property } => {
            format!("{} is a required property", repr(property))
        }
        ValidationErrorKind::Constant { expected_value } => {
            format!("{} was expected", repr(expected_value))
        }
        ValidationErrorKind::Enum { options } => {
            format!("{instance} is not one of {}", repr(options))
        }
        ValidationErrorKind::Type { kind } => {
            format!(
                "{instance} is not of type {}",
                type_names(schema, error, kind)
            )
        }
        ValidationErrorKind::Minimum { limit } => {
            format!("{instance} is less than the minimum of {}", repr(limit))
        }
        ValidationErrorKind::Maximum { limit } => {
            format!("{instance} is greater than the maximum of {}", repr(limit))
        }
        ValidationErrorKind::ExclusiveMinimum { limit } => {
            format!(
                "{instance} is less than or equal to the minimum of {}",
                repr(limit)
            )
        }
        ValidationErrorKind::ExclusiveMaximum { limit } => {
            format!(
                "{instance} is greater than or equal to the maximum of {}",
                repr(limit)
            )
        }
        ValidationErrorKind::MinItems { limit } | ValidationErrorKind::MinLength { limit } => {
            let text = if *limit == 1 {
                "should be non-empty"
            } else {
                "is too short"
            };
            format!("{instance} {text}")
        }
        ValidationErrorKind::MaxItems { limit } | ValidationErrorKind::MaxLength { limit } => {
            let text = if *limit == 0 {
                "is expected to be empty"
            } else {
                "is too long"
            };
            format!("{instance} {text}")
        }
        ValidationErrorKind::MinProperties { limit } => {
            let text = if *limit == 1 {
                "should be non-empty"
            } else {
                "does not have enough properties"
            };
            format!("{instance} {text}")
        }
        ValidationErrorKind::MaxProperties { limit } => {
            let text = if *limit == 0 {
                "is expected to be empty"
            } else {
                "has too many properties"
            };
            format!("{instance} {text}")
        }
        ValidationErrorKind::Pattern { pattern } => {
            format!("{instance} does not match {}", str_repr(pattern))
        }
        ValidationErrorKind::UniqueItems => format!("{instance} has non-unique elements"),
        ValidationErrorKind::AnyOf { .. } | ValidationErrorKind::OneOfNotValid { .. } => {
            format!("{instance} is not valid under any of the given schemas")
        }
        // Keywords that neither schema uses keep the crate's own wording.
        _ => error.to_string(),
    }
}

/// The schema's `type` value as python-jsonschema prints it: `'array'`, or `'string', 'null'`
/// in schema order. The crate keeps several types as an unordered set, so the order comes from
/// the schema itself, at the error's keyword location.
fn type_names(schema: &Value, error: &ValidationError<'_>, kind: &TypeKind) -> String {
    let keyword = error
        .absolute_keyword_location()
        .and_then(jsonschema::Uri::fragment)
        .and_then(|fragment| schema.pointer(fragment.as_str()));
    match keyword {
        Some(Value::String(name)) => str_repr(name),
        Some(Value::Array(names)) => names.iter().map(repr).collect::<Vec<_>>().join(", "),
        _ => match kind {
            TypeKind::Single(name) => str_repr(&name.to_string()),
            TypeKind::Multiple(names) => names
                .iter()
                .map(|name| str_repr(&name.to_string()))
                .collect::<Vec<_>>()
                .join(", "),
        },
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use sonara_core::error::ErrorCode;

    use super::{PathPart, Schema, path_parts, validate_document};

    #[test]
    fn embedded_schemas_compile() {
        for schema in [Schema::Project, Schema::Manifest] {
            let compiled = schema.compiled();
            assert_eq!(
                compiled.schema["$schema"],
                "https://json-schema.org/draft/2020-12/schema"
            );
        }
    }

    #[test]
    fn paths_are_typed_by_the_document() {
        let doc = json!({"tracks": [{"0": {"a/b": 1}}], "": 2});
        assert_eq!(
            path_parts(&doc, "/tracks/0/0/a~1b"),
            [
                PathPart::Key("tracks".into()),
                PathPart::Index(0),
                PathPart::Key("0".into()),
                PathPart::Key("a/b".into())
            ]
        );
        assert_eq!(path_parts(&doc, ""), []);
        assert_eq!(path_parts(&doc, "/"), [PathPart::Key(String::new())]);
    }

    fn first_error(doc: &Value, schema: Schema) -> (String, usize) {
        let err = validate_document(doc, schema).expect_err("invalid");
        assert_eq!(err.code, ErrorCode::FormatError);
        let count = err.details["error_count"].as_u64().expect("count");
        (err.message, usize::try_from(count).expect("small"))
    }

    #[test]
    fn several_types_keep_the_schema_order() {
        // Python: "1 is not of type 'string', 'null'" (the crate's set order is null, string).
        let doc = json!({"format": "sonara.manifest", "format_version": 1, "project_id": "p",
                         "project_file": {"path": "project.json", "sha256": "0".repeat(64), "size": 1},
                         "assets": [{"id": "a", "path": "p", "location": "linked", "size": 1, "sha256": 1}],
                         "required_features": []});
        assert_eq!(
            first_error(&doc, Schema::Manifest),
            (
                "manifest-v1.schema.json: assets/0/sha256: 1 is not of type 'string', 'null'"
                    .to_owned(),
                1
            )
        );
    }
}
