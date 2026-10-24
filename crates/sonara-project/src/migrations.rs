//! Versioned project-document migrations (`sonara.persistence.migrations`).
//!
//! Each migration is a pure function that upgrades a `project.json` document by exactly one
//! schema version. Migrations run before schema validation. Documents from a newer schema
//! version than this build understands are rejected; [`crate::store::required_features`] is the
//! finer-grained, forward-compatible mechanism.

use serde_json::Value;
use sonara_core::error::{DomainError, ErrorCode, Result};
use sonara_core::model::JsonMap;

use crate::pyjson::repr;
use crate::serializer::SCHEMA_VERSION;

/// Upgrades a document by one schema version.
pub type Migration = fn(JsonMap) -> JsonMap;

/// `MIGRATIONS[n - 1]` upgrades a version-`n` document to version `n + 1`. Empty: version 1 is
/// the only schema version so far.
pub const MIGRATIONS: &[Migration] = &[];

/// Upgrades `doc` to [`SCHEMA_VERSION`]. Returns the document and the versions it was upgraded
/// from, in order (empty when it was current).
///
/// # Errors
///
/// - [`ErrorCode::FormatError`]: `"invalid schema_version <repr>"` when the version is missing,
///   not an integer or below 1;
/// - [`ErrorCode::Unsupported`] for a newer document, with the `schema_version` detail;
/// - [`ErrorCode::Internal`] when a migration step is missing.
pub fn migrate(doc: JsonMap) -> Result<(JsonMap, Vec<i64>)> {
    migrate_with(doc, SCHEMA_VERSION, MIGRATIONS)
}

/// [`migrate`] towards `target` with the table `migrations`, where `migrations[n - 1]` upgrades
/// version `n`. Exposed so the upgrade loop can be tested with a table of its own.
///
/// # Errors
///
/// As [`migrate`].
pub fn migrate_with(
    mut doc: JsonMap,
    target: i64,
    migrations: &[Migration],
) -> Result<(JsonMap, Vec<i64>)> {
    let raw = doc.get("schema_version").cloned().unwrap_or(Value::Null);
    let Some(found) = python_int(&raw).filter(|v| *v >= 1) else {
        return Err(DomainError::new(
            ErrorCode::FormatError,
            format!("invalid schema_version {}", repr(&raw)),
        ));
    };
    let newer = || {
        DomainError::new(
            ErrorCode::Unsupported,
            format!(
                "project was saved by a newer Sonara (schema {found}); this version reads up \
                 to {target}"
            ),
        )
        .with_detail("schema_version", raw.clone())
    };
    if found > i128::from(target) {
        return Err(newer());
    }
    let mut version = i64::try_from(found).map_err(|_| newer())?;
    let mut applied = Vec::new();
    while version < target {
        let step = usize::try_from(version - 1)
            .ok()
            .and_then(|index| migrations.get(index))
            .ok_or_else(|| {
                DomainError::new(
                    ErrorCode::Internal,
                    format!("no migration from schema {version}"),
                )
            })?;
        doc = step(doc);
        applied.push(version);
        version += 1;
        doc.insert("schema_version".to_owned(), Value::from(version));
    }
    Ok((doc, applied))
}

/// The value as a Python `int` would see it: integers, and booleans as 0 and 1
/// (`isinstance(True, int)`). Floats are not integers, so `1.0` is invalid.
fn python_int(value: &Value) -> Option<i128> {
    match value {
        Value::Bool(flag) => Some(i128::from(*flag)),
        Value::Number(number) => number
            .as_i64()
            .map(i128::from)
            .or_else(|| number.as_u64().map(i128::from)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use sonara_core::error::ErrorCode;
    use sonara_core::model::JsonMap;

    use super::{MIGRATIONS, Migration, migrate, migrate_with};
    use crate::serializer::SCHEMA_VERSION;

    fn doc(value: Value) -> JsonMap {
        match value {
            Value::Object(map) => map,
            _ => unreachable!("tests pass objects"),
        }
    }

    #[test]
    fn current_documents_pass_unchanged() {
        let original = doc(json!({"schema_version": 1, "a": 1}));
        assert_eq!(
            migrate(original.clone()).expect("current"),
            (original, Vec::new())
        );
        // `True` is a Python int equal to 1; the schema rejects it later.
        assert!(migrate(doc(json!({"schema_version": true}))).is_ok());
    }

    #[test]
    fn newer_and_invalid_versions_are_rejected() {
        let err = migrate(doc(json!({"schema_version": 99}))).expect_err("newer");
        assert_eq!(err.code, ErrorCode::Unsupported);
        assert_eq!(
            err.message,
            "project was saved by a newer Sonara (schema 99); this version reads up to 1"
        );
        assert_eq!(err.details["schema_version"], 99);
        let err = migrate(doc(json!({"schema_version": u64::MAX}))).expect_err("newer");
        assert_eq!(err.code, ErrorCode::Unsupported);
        assert!(err.message.contains("(schema 18446744073709551615)"));
        for (bad, text) in [
            (json!({}), "None"),
            (json!({"schema_version": "1"}), "'1'"),
            (json!({"schema_version": 0}), "0"),
            (json!({"schema_version": 1.0}), "1.0"),
            (json!({"schema_version": false}), "False"),
        ] {
            let err = migrate(doc(bad)).expect_err("invalid");
            assert_eq!(err.code, ErrorCode::FormatError);
            assert_eq!(err.message, format!("invalid schema_version {text}"));
        }
    }

    #[test]
    fn steps_run_in_order() {
        fn add_b(mut d: JsonMap) -> JsonMap {
            d.insert("b".into(), json!(2));
            d
        }
        fn add_c(mut d: JsonMap) -> JsonMap {
            let b = d["b"].as_i64().expect("b");
            d.insert("c".into(), json!(b + 1));
            d
        }
        let table: [Migration; 2] = [add_b, add_c];
        let (upgraded, steps) =
            migrate_with(doc(json!({"schema_version": 1, "a": 1})), 3, &table).expect("upgrade");
        assert_eq!(
            Value::Object(upgraded),
            json!({"schema_version": 3, "a": 1, "b": 2, "c": 3})
        );
        assert_eq!(steps, [1, 2]);
        let err = migrate_with(doc(json!({"schema_version": 1})), 4, &table).expect_err("gap");
        assert_eq!(err.code, ErrorCode::Internal);
        assert_eq!(err.message, "no migration from schema 3");
    }

    #[test]
    fn every_schema_version_has_a_migration_path() {
        assert_eq!(
            i64::try_from(MIGRATIONS.len()).expect("small"),
            SCHEMA_VERSION - 1
        );
    }
}
