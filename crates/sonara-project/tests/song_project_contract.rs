//! SON-P04: the canonical song project, checked at the contract level a downstream consumer
//! sees: a JSON document, validated by the JSON Schema and the domain model, serialized
//! deterministically. The model itself is `sonara-core::model`; this file adds no second one.
//!
//! The fixture `fixtures/song.project.json` is a three-track song with a tempo change, a meter
//! change, two sections and overlapping notes. It is stored in canonical form, so reading and
//! writing it must reproduce its bytes.

use serde_json::{Value, json};
use sonara_core::error::ErrorCode;
use sonara_core::model::project::Project;
use sonara_core::routing;
use sonara_project::schema::{Schema, validate_document};
use sonara_project::serializer::{
    SCHEMA_VERSION, dumps, loads, project_from_value, project_to_value,
};

const SONG: &[u8] = include_bytes!("fixtures/song.project.json");

fn song_document() -> Value {
    serde_json::from_slice(SONG).expect("the fixture is JSON")
}

fn read(doc: &Value) -> Result<Project, sonara_core::error::DomainError> {
    project_from_value(doc.as_object().expect("a JSON object"))
}

fn song() -> Project {
    read(&song_document()).expect("the fixture is a valid project")
}

#[test]
fn the_fixture_passes_the_schema_the_model_and_routing() {
    let doc = song_document();
    validate_document(&doc, Schema::Project).expect("schema");
    let project = read(&doc).expect("model");
    routing::validate(&project).expect("routing");
    assert_eq!(project.tracks.len(), 3);
    assert_eq!(project.sections.len(), 2);
    assert_eq!(project.tempo_map.points().len(), 2);
    assert_eq!(project.time_signatures.points().len(), 2);
}

#[test]
fn the_fixture_is_canonical_and_round_trips_byte_for_byte() {
    let project = song();
    let bytes = dumps(&project);
    assert_eq!(bytes, SONG);
    let again = project_from_value(&loads(&bytes).expect("an object")).expect("valid");
    assert_eq!(again, project);
    assert_eq!(dumps(&again), bytes);
}

#[test]
fn an_empty_project_is_valid_and_deterministic() {
    let doc = json!({
        "format": "sonara.project", "schema_version": 1, "id": "prj_empty", "name": "Empty",
        "settings": {"sample_rate": 44100}, "tempo_map": [{"beat": 0, "bpm": 120}],
        "time_signatures": [{"bar": 0, "numerator": 4, "denominator": 4}],
        "tracks": [], "master": {"id": "master", "name": "Master", "kind": "master"},
    });
    let project = read(&doc).expect("valid");
    assert!(project.tracks.is_empty() && project.sections.is_empty());
    let bytes = dumps(&project);
    assert_eq!(bytes, dumps(&project));
    let canonical = serde_json::from_slice(&bytes).expect("JSON");
    validate_document(&canonical, Schema::Project).expect("the written document is schema-valid");
    assert_eq!(read(&canonical).expect("valid"), project);
}

#[test]
fn notes_may_overlap_and_share_a_pitch_range() {
    let project = song();
    let pad = project.track("trk_pad").expect("track");
    let notes = &pad.clips[0].as_midi().expect("a MIDI clip").notes;
    // n_pad_3 (beats 2..8) starts inside n_pad_1 and n_pad_2 (beats 0..4).
    assert!(notes[2].start < notes[0].start + notes[0].length);
    assert_eq!(notes.len(), 3);
}

#[test]
fn instruments_are_referenced_by_plugin_id_and_hold_no_machine_state() {
    let doc = song_document();
    for track in doc["tracks"].as_array().expect("tracks") {
        let instrument = &track["instrument"];
        assert!(
            instrument["plugin_id"]
                .as_str()
                .is_some_and(|id| id.starts_with("sonara."))
        );
        assert!(!doc.to_string().contains("/Users/"));
    }
}

/// A document with an invalid value is a `FormatError`: the model's range check fails and the
/// document layer reports it with the path of the value.
#[test]
fn invalid_values_are_rejected_not_coerced() {
    let cases: [(&str, &[&str], Value, ErrorCode); 8] = [
        (
            "tempo below 10",
            &["tempo_map", "0", "bpm"],
            json!(9.9),
            ErrorCode::FormatError,
        ),
        (
            "tempo above 999",
            &["tempo_map", "0", "bpm"],
            json!(999.5),
            ErrorCode::FormatError,
        ),
        (
            "denominator 5",
            &["time_signatures", "0", "denominator"],
            json!(5),
            ErrorCode::FormatError,
        ),
        (
            "pitch 128",
            &["tracks", "0", "clips", "0", "notes", "0", "pitch"],
            json!(128),
            ErrorCode::FormatError,
        ),
        (
            "velocity 0",
            &["tracks", "0", "clips", "0", "notes", "0", "velocity"],
            json!(0),
            ErrorCode::FormatError,
        ),
        (
            "zero length",
            &["tracks", "0", "clips", "0", "notes", "0", "length"],
            json!(0.0),
            ErrorCode::FormatError,
        ),
        (
            "negative start",
            &["tracks", "0", "clips", "0", "notes", "0", "start"],
            json!(-1.0),
            ErrorCode::FormatError,
        ),
        (
            "channel 16",
            &["tracks", "0", "clips", "0", "notes", "0", "channel"],
            json!(16),
            ErrorCode::FormatError,
        ),
    ];
    for (what, path, value, code) in cases {
        let mut doc = song_document();
        let mut at = &mut doc;
        for key in path {
            at = match key.parse::<usize>() {
                Ok(i) => &mut at[i],
                Err(_) => &mut at[*key],
            };
        }
        *at = value;
        let err = read(&doc).expect_err(what);
        assert_eq!(err.code, code, "{what}: {}", err.message);
    }
}

#[test]
fn duplicate_ids_and_dangling_references_are_rejected() {
    let mut doc = song_document();
    doc["tracks"][1]["id"] = json!("trk_pad");
    let err = read(&doc).expect_err("duplicate track id");
    assert_eq!(err.code, ErrorCode::Conflict);

    let mut doc = song_document();
    doc["tracks"][0]["output_id"] = json!("trk_missing");
    let project = read(&doc).expect("the model reads it");
    let err = routing::validate(&project).expect_err("dangling output");
    assert_eq!(err.details["object_id"], "trk_pad");
}

#[test]
fn schema_versions_are_checked() {
    let mut newer = song_document();
    newer["schema_version"] = json!(SCHEMA_VERSION + 1);
    let err = sonara_project::migrations::migrate(newer.as_object().expect("object").clone())
        .expect_err("newer");
    assert_eq!(err.code, ErrorCode::Unsupported);

    let mut missing = song_document();
    missing
        .as_object_mut()
        .expect("object")
        .remove("schema_version");
    let err = sonara_project::migrations::migrate(missing.as_object().expect("object").clone())
        .expect_err("missing");
    assert_eq!(err.code, ErrorCode::FormatError);
    assert_eq!(SCHEMA_VERSION, 1);
}

#[test]
fn the_schema_rejects_malformed_documents() {
    let mut doc = song_document();
    doc["tracks"][0]["name"] = json!(7);
    assert!(validate_document(&doc, Schema::Project).is_err());
    let mut doc = song_document();
    doc.as_object_mut().expect("object").remove("tempo_map");
    assert!(validate_document(&doc, Schema::Project).is_err());
}

#[test]
fn serialization_does_not_depend_on_the_clock_or_on_input_key_order() {
    let a = dumps(&song());
    let mut shuffled = serde_json::Map::new();
    for (key, value) in song_document().as_object().expect("object").iter().rev() {
        shuffled.insert(key.clone(), value.clone());
    }
    let b = dumps(&project_from_value(&shuffled).expect("valid"));
    assert_eq!(a, b);
    assert_eq!(project_to_value(&song()), song_document());
}
