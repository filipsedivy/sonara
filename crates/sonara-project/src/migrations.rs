//! Versioned project-document migrations (`sonara.persistence.migrations`).
//!
//! Each migration is a pure function that upgrades a `project.json` document by exactly one
//! schema version. [`upgrade`] validates a document against the schema of its own version, so an
//! old document gets exactly the verdict it always got, and then runs the migrations. Documents
//! from a newer schema version than this build understands are rejected;
//! [`crate::store::required_features`] is the finer-grained, forward-compatible mechanism.

use serde_json::Value;
use sonara_core::error::{DomainError, ErrorCode, Result};
use sonara_core::ids::is_vocabulary_id;
use sonara_core::model::JsonMap;

use crate::pyjson::repr;
use crate::schema::{Schema, is_valid_part, validate_document};
use crate::serializer::SCHEMA_VERSION;

/// Upgrades a document by one schema version.
pub type Migration = fn(JsonMap) -> JsonMap;

/// `MIGRATIONS[n - 1]` upgrades a version-`n` document to version `n + 1`.
pub const MIGRATIONS: &[Migration] = &[v1_to_v2];

/// The key a version-1 track keeps a `part` under that version 2 does not accept.
pub const PART_V1_KEY: &str = "part_v1";

/// Version 1 to 2. Version 2 only adds an optional track `part` and an optional note
/// `articulation`, so the document is relabelled. A version-1 document can already hold keys
/// of those names, which version 1 ignored; each stays only where version 2 accepts it:
///
/// - a track's `part` stays when the track is an instrument track and the part is valid
///   ([`is_valid_part`]); otherwise it is renamed [`PART_V1_KEY`], an unknown key the track
///   keeps (replacing one of that name);
/// - a MIDI note's `articulation` stays when it is a vocabulary id; otherwise it is removed, as
///   version 1 dropped every unknown note key.
///
/// Nothing is derived from other values, such as a sampler's instrument type: a migration is
/// pure, and a fact is stored once. The document has not been validated against version 2
/// yet, so every step tolerates any JSON.
fn v1_to_v2(mut doc: JsonMap) -> JsonMap {
    if let Some(tracks) = doc.get_mut("tracks").and_then(Value::as_array_mut) {
        tracks.iter_mut().for_each(relabel_track);
    }
    if let Some(master) = doc.get_mut("master") {
        relabel_track(master);
    }
    doc
}

/// [`v1_to_v2`] for one track.
fn relabel_track(track: &mut Value) {
    let Some(track) = track.as_object_mut() else {
        return;
    };
    let instrument = track.get("kind").and_then(Value::as_str) == Some("instrument");
    if track
        .get("part")
        .is_some_and(|part| !(instrument && is_valid_part(part)))
        && let Some(part) = track.shift_remove("part")
    {
        track.insert(PART_V1_KEY.to_owned(), part);
    }
    let clips = track.get_mut("clips").and_then(Value::as_array_mut);
    for clip in clips.into_iter().flatten().filter_map(Value::as_object_mut) {
        if clip.get("type").and_then(Value::as_str) != Some("midi") {
            continue;
        }
        let notes = clip.get_mut("notes").and_then(Value::as_array_mut);
        for note in notes.into_iter().flatten().filter_map(Value::as_object_mut) {
            if note
                .get("articulation")
                .is_some_and(|id| !id.as_str().is_some_and(is_vocabulary_id))
            {
                note.shift_remove("articulation");
            }
        }
    }
}

/// Prepares a parsed `project.json` for reading: checks its version, validates it against the
/// schema of that version and upgrades it to [`SCHEMA_VERSION`]. Returns the document and the
/// version it was migrated from, which is `None` unless a migration changed more than the
/// version number: an unchanged version-1 document opens as it always did, reporting no
/// migration (GLD-01).
///
/// # Errors
///
/// - the errors of [`migrate`] for a missing, invalid or newer version;
/// - [`ErrorCode::FormatError`] when the document breaks the schema of its version (see
///   [`validate_document`]);
/// - [`ErrorCode::Internal`] when a version has no schema.
pub fn upgrade(doc: JsonMap) -> Result<(JsonMap, Option<i64>)> {
    let version = source_version(&doc, SCHEMA_VERSION)?;
    let schema = Schema::project(version).ok_or_else(|| {
        DomainError::new(
            ErrorCode::Internal,
            format!("no schema for project version {version}"),
        )
    })?;
    let value = Value::Object(doc);
    validate_document(&value, schema)?;
    let Value::Object(doc) = value else {
        // Wrapped just above.
        return Err(DomainError::new(
            ErrorCode::FormatError,
            "project.json must contain a JSON object",
        ));
    };
    if version == SCHEMA_VERSION {
        return Ok((doc, None));
    }
    let before = doc.clone();
    let (doc, _) = migrate(doc)?;
    let changed = !same_but_version(&before, &doc);
    Ok((doc, changed.then_some(version)))
}

/// Whether two documents that both have a `schema_version` hold the same other keys and values.
fn same_but_version(a: &JsonMap, b: &JsonMap) -> bool {
    a.len() == b.len()
        && a.iter()
            .filter(|(key, _)| *key != "schema_version")
            .all(|(key, value)| b.get(key) == Some(value))
}

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
    let mut version = source_version(&doc, target)?;
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

/// The document's schema version, which `target` must not exceed.
///
/// # Errors
///
/// As [`migrate`].
fn source_version(doc: &JsonMap, target: i64) -> Result<i64> {
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
    i64::try_from(found).map_err(|_| newer())
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

    use super::{MIGRATIONS, Migration, PART_V1_KEY, migrate, migrate_with, upgrade, v1_to_v2};
    use crate::serializer::SCHEMA_VERSION;

    fn doc(value: Value) -> JsonMap {
        match value {
            Value::Object(map) => map,
            _ => unreachable!("tests pass objects"),
        }
    }

    #[test]
    fn current_documents_pass_unchanged() {
        let original = doc(json!({"schema_version": 2, "a": 1}));
        assert_eq!(
            migrate(original.clone()).expect("current"),
            (original, Vec::new())
        );
        // `True` is a Python int equal to 1, and is migrated as version 1; the version-1 schema
        // rejects it before (see `upgrade`).
        assert_eq!(
            migrate(doc(json!({"schema_version": true})))
                .expect("version 1")
                .1,
            [1]
        );
    }

    #[test]
    fn newer_and_invalid_versions_are_rejected() {
        let err = migrate(doc(json!({"schema_version": 99}))).expect_err("newer");
        assert_eq!(err.code, ErrorCode::Unsupported);
        assert_eq!(
            err.message,
            "project was saved by a newer Sonara (schema 99); this version reads up to 2"
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
            let err = migrate(doc(bad.clone())).expect_err("invalid");
            assert_eq!(err.code, ErrorCode::FormatError);
            assert_eq!(err.message, format!("invalid schema_version {text}"));
            // `upgrade` checks the version before anything else.
            assert_eq!(upgrade(doc(bad)).expect_err("invalid"), err);
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

    fn track(kind: &str, part: Option<Value>) -> Value {
        let mut track = json!({"id": "trk_1", "name": "T", "kind": kind, "clips": []});
        if let Some(part) = part {
            track["part"] = part;
        }
        track
    }

    #[test]
    fn v1_to_v2_keeps_a_valid_part_of_an_instrument_track() {
        let part = json!({"instrument": "violin", "name": "Violins I", "players": 14,
                          "solo": false, "notation": {"clef": "treble"}});
        let original = doc(json!({"tracks": [track("instrument", Some(part))],
                                  "master": track("master", None)}));
        assert_eq!(v1_to_v2(original.clone()), original);
    }

    #[test]
    fn v1_to_v2_moves_every_other_part_aside() {
        let valid = json!({"instrument": "violin"});
        for (kind, part) in [
            ("instrument", json!({"instrument": "Violin"})),
            ("instrument", json!({"name": "Violins I"})),
            ("instrument", json!({"instrument": "violin", "players": 0})),
            (
                "instrument",
                json!({"instrument": "violin", "notation": {"clef": "soprano"}}),
            ),
            ("instrument", json!("violin")),
            ("instrument", Value::Null),
            ("audio", valid.clone()),
            ("bus", valid.clone()),
        ] {
            let mut track = track(kind, Some(part.clone()));
            track[PART_V1_KEY] = json!("replaced");
            let out = v1_to_v2(doc(json!({"tracks": [track]})));
            let moved = out["tracks"][0].as_object().expect("track");
            assert!(!moved.contains_key("part"), "{kind} {part}");
            assert_eq!(moved[PART_V1_KEY], part, "{kind}");
        }
        let out = v1_to_v2(doc(json!({"master": track("master", Some(valid.clone()))})));
        assert_eq!(out["master"][PART_V1_KEY], valid);
    }

    #[test]
    fn v1_to_v2_drops_note_articulations_that_are_not_ids() {
        let notes = json!([
            {"id": "n_1", "articulation": "staccato"},
            {"id": "n_2", "articulation": "Staccato"},
            {"id": "n_3", "articulation": 5},
            {"id": "n_4"},
        ]);
        let midi = json!({"type": "midi", "id": "clp_1", "notes": notes});
        // An audio clip's `notes` are unknown keys the clip keeps, never read as notes.
        let audio = json!({"type": "audio", "id": "clp_2", "notes": [{"articulation": 5}]});
        let original = json!({"tracks": [{"kind": "instrument", "clips": [midi, audio]}]});
        let out = Value::Object(v1_to_v2(doc(original.clone())));
        let mut expected = original;
        expected["tracks"][0]["clips"][0]["notes"] = json!([
            {"id": "n_1", "articulation": "staccato"},
            {"id": "n_2"},
            {"id": "n_3"},
            {"id": "n_4"},
        ]);
        assert_eq!(out, expected);
    }

    #[test]
    fn v1_to_v2_tolerates_any_json() {
        let malformed = [
            json!({}),
            json!({"tracks": {}, "master": 1}),
            json!({"tracks": [1, "x", null, {"clips": {}}, {"clips": [1, {"type": 2}]}]}),
            json!({"tracks": [{"clips": [{"type": "midi", "notes": {}}]}]}),
        ];
        for original in malformed {
            assert_eq!(Value::Object(v1_to_v2(doc(original.clone()))), original);
        }
        let notes =
            json!({"tracks": [{"clips": [{"type": "midi", "notes": [1, {"articulation": []}]}]}]});
        assert_eq!(
            Value::Object(v1_to_v2(doc(notes))),
            json!({"tracks": [{"clips": [{"type": "midi", "notes": [1, {}]}]}]})
        );
    }
}
