//! Project schema version 2: an optional track `part` and an optional note `articulation` on top
//! of version 1 (CTR-01). The writer writes the lowest version a project needs, so a project
//! without either is written byte for byte as before, and an unchanged version-1 document opens
//! without a reported migration (GLD-01). The base document is the canonical song fixture,
//! which is version 1.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::model::part::Clef;
use sonara_core::model::project::Project;
use sonara_project::migrations::{PART_V1_KEY, upgrade};
use sonara_project::schema::{
    PROJECT_SCHEMA_TEXT, PROJECT_V2_SCHEMA_TEXT, Schema, validate_document,
};
use sonara_project::serializer::{
    SCHEMA_VERSION, dumps, has_note_articulations, has_parts, lowest_schema_version,
    project_from_value, project_to_value,
};
use sonara_project::store::{
    MANIFEST, PROJECT, ProjectStore, SUPPORTED_FEATURES, StoreError, required_features,
};

const SONG: &[u8] = include_bytes!("fixtures/song.project.json");

/// A change made to a document.
type Change = fn(&mut Value);

fn song_document() -> Value {
    serde_json::from_slice(SONG).expect("the fixture is JSON")
}

fn read(doc: &Value) -> Result<Project, DomainError> {
    project_from_value(doc.as_object().expect("a JSON object"))
}

fn object(value: Value) -> serde_json::Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("not a JSON object: {other}"),
    }
}

/// A full part, written in the writer's key order.
fn violins() -> Value {
    json!({"instrument": "violin", "name": "Violins I", "players": 16, "solo": false,
           "player": "Violin 1", "notation": {"transposition": 0, "clef": "treble"}})
}

/// The song as version 2: the pad track plays a part and its first note is staccato.
fn song_v2_document() -> Value {
    let mut doc = song_document();
    doc["schema_version"] = json!(2);
    let pad = doc["tracks"][0].as_object_mut().expect("track");
    // The writer puts `part` right after `kind`.
    let mut ordered = serde_json::Map::new();
    for (key, value) in std::mem::take(pad) {
        let after_kind = key == "kind";
        ordered.insert(key, value);
        if after_kind {
            ordered.insert("part".into(), violins());
        }
    }
    *pad = ordered;
    doc["tracks"][0]["clips"][0]["notes"][0]["articulation"] = json!("staccato");
    doc
}

// -- the schema --------------------------------------------------------------------------

#[test]
fn version_2_is_version_1_plus_exactly_the_two_keys() {
    let v1: Value = serde_json::from_str(PROJECT_SCHEMA_TEXT).expect("JSON");
    let mut v2: Value = serde_json::from_str(PROJECT_V2_SCHEMA_TEXT).expect("JSON");
    assert_eq!(
        v2["$id"],
        "https://sonara.dev/schemas/project-v2.schema.json"
    );
    assert_eq!(v2["properties"]["schema_version"], json!({"const": 2}));
    let id = json!({"type": "string", "pattern": "^[a-z][a-z0-9_]*$"});
    assert_eq!(v2["$defs"]["vocabulary_id"], id);
    let part = v2["$defs"]["part"].clone();
    assert_eq!(part["required"], json!(["instrument"]));
    assert_eq!(
        part["properties"]["notation"]["properties"]["clef"]["enum"],
        json!(Clef::ALL.map(Clef::as_str))
    );
    // Remove the additions, and what is left is version 1.
    let defs = v2["$defs"].as_object_mut().expect("defs");
    defs.shift_remove("vocabulary_id");
    defs.shift_remove("part");
    let note = v2["$defs"]["note"]["properties"]
        .as_object_mut()
        .expect("note");
    assert_eq!(
        note.shift_remove("articulation"),
        Some(json!({"$ref": "#/$defs/vocabulary_id"}))
    );
    let track = v2["$defs"]["track"]["properties"]
        .as_object_mut()
        .expect("track");
    assert_eq!(
        track.shift_remove("part"),
        Some(json!({"$ref": "#/$defs/part"}))
    );
    for key in ["$id", "title"] {
        v2[key] = v1[key].clone();
    }
    v2["properties"]["schema_version"] = json!({"const": 1});
    assert_eq!(v2, v1);
    assert_eq!(Schema::project(1), Some(Schema::Project));
    assert_eq!(Schema::project(SCHEMA_VERSION), Some(Schema::ProjectV2));
    assert_eq!(Schema::project(SCHEMA_VERSION + 1), None);
    assert_eq!(Schema::ProjectV2.file_name(), "project-v2.schema.json");
}

#[test]
fn each_version_validates_only_its_own_documents() {
    let v1 = song_document();
    let v2 = song_v2_document();
    validate_document(&v1, Schema::Project).expect("v1");
    validate_document(&v2, Schema::ProjectV2).expect("v2");
    let err = validate_document(&v2, Schema::Project).expect_err("v2 is not v1");
    assert_eq!(
        err.message,
        "project-v1.schema.json: schema_version: 1 was expected"
    );
    let err = validate_document(&v1, Schema::ProjectV2).expect_err("v1 is not v2");
    assert_eq!(
        err.message,
        "project-v2.schema.json: schema_version: 2 was expected"
    );
}

#[test]
fn the_schema_checks_parts_and_articulations() {
    let cases: [(Change, &str); 9] = [
        (
            |d| d["tracks"][0]["part"] = json!({"name": "Violins I"}),
            "tracks/0/part: 'instrument' is a required property",
        ),
        (
            |d| d["tracks"][0]["part"]["instrument"] = json!("Violin"),
            "tracks/0/part/instrument: 'Violin' does not match '^[a-z][a-z0-9_]*$'",
        ),
        (
            |d| d["tracks"][0]["part"]["players"] = json!(0),
            "tracks/0/part/players: 0 is less than the minimum of 1",
        ),
        (
            |d| d["tracks"][0]["part"]["players"] = json!(129),
            "tracks/0/part/players: 129 is greater than the maximum of 128",
        ),
        (
            |d| d["tracks"][0]["part"]["name"] = json!(""),
            "tracks/0/part/name: '' should be non-empty",
        ),
        (
            |d| d["tracks"][0]["part"]["solo"] = json!("yes"),
            "tracks/0/part/solo: 'yes' is not of type 'boolean'",
        ),
        (
            |d| d["tracks"][0]["part"]["notation"]["transposition"] = json!(-37),
            "tracks/0/part/notation/transposition: -37 is less than the minimum of -36",
        ),
        (
            |d| d["tracks"][0]["part"]["notation"]["clef"] = json!("soprano"),
            "tracks/0/part/notation/clef: 'soprano' is not one of ['treble', 'treble_8vb', \
             'bass', 'alto', 'tenor', 'percussion', 'single_line']",
        ),
        (
            |d| d["tracks"][0]["clips"][0]["notes"][0]["articulation"] = json!("Staccato"),
            "tracks/0/clips/0/notes/0/articulation: 'Staccato' does not match \
             '^[a-z][a-z0-9_]*$'",
        ),
    ];
    for (change, message) in cases {
        let mut doc = song_v2_document();
        change(&mut doc);
        let err = validate_document(&doc, Schema::ProjectV2).expect_err(message);
        assert_eq!(err.code, ErrorCode::FormatError);
        assert_eq!(err.message, format!("project-v2.schema.json: {message}"));
    }
}

// -- writing and reading -----------------------------------------------------------------

#[test]
fn a_version_2_document_round_trips_byte_for_byte() {
    let doc = song_v2_document();
    let project = read(&doc).expect("a valid document");
    let part = project.tracks[0].part.as_ref().expect("a part");
    assert_eq!(part.instrument, "violin");
    assert_eq!(
        (part.name.as_deref(), part.players, part.solo),
        (Some("Violins I"), Some(16), Some(false))
    );
    assert_eq!(part.player.as_deref(), Some("Violin 1"));
    assert_eq!(part.notation.transposition, Some(0));
    assert_eq!(part.notation.clef, Some(Clef::Treble));
    let notes = &project.tracks[0].clips[0].as_midi().expect("MIDI").notes;
    assert_eq!(notes[0].articulation.as_deref(), Some("staccato"));
    assert!(notes[1..].iter().all(|note| note.articulation.is_none()));
    assert_eq!(project_to_value(&project), doc);
    let bytes = dumps(&project);
    assert_eq!(
        read(&serde_json::from_slice(&bytes).expect("JSON")),
        Ok(project)
    );
}

#[test]
fn the_writer_writes_the_lowest_version_the_project_needs() {
    let song = read(&song_document()).expect("valid");
    assert_eq!(
        (has_parts(&song), has_note_articulations(&song)),
        (false, false)
    );
    assert_eq!(lowest_schema_version(&song), 1);
    assert_eq!(dumps(&song), SONG);

    let mut only_part = song_v2_document();
    only_part["tracks"][0]["clips"][0]["notes"][0]
        .as_object_mut()
        .expect("note")
        .shift_remove("articulation");
    let project = read(&only_part).expect("valid");
    assert_eq!(
        (has_parts(&project), has_note_articulations(&project)),
        (true, false)
    );
    assert_eq!(project_to_value(&project)["schema_version"], 2);

    let mut only_articulation = song_v2_document();
    only_articulation["tracks"][0]
        .as_object_mut()
        .expect("track")
        .shift_remove("part");
    let project = read(&only_articulation).expect("valid");
    assert_eq!(
        (has_parts(&project), has_note_articulations(&project)),
        (false, true)
    );
    assert_eq!(lowest_schema_version(&project), 2);
    let written = project_to_value(&project);
    assert_eq!(written, only_articulation);
    validate_document(&written, Schema::ProjectV2).expect("v2");

    // Taking both away again gives back the version-1 bytes.
    only_articulation["tracks"][0]["clips"][0]["notes"][0]
        .as_object_mut()
        .expect("note")
        .shift_remove("articulation");
    assert_eq!(dumps(&read(&only_articulation).expect("valid")), SONG);
}

#[test]
fn a_part_states_only_what_it_has() {
    let mut doc = song_v2_document();
    doc["tracks"][0]["part"] = json!({"instrument": "horn_f", "notation": {}});
    let project = read(&doc).expect("valid");
    let part = project.tracks[0].part.as_ref().expect("a part");
    assert_eq!(part.instrument, "horn_f");
    assert!(part.name.is_none() && part.players.is_none() && part.solo.is_none());
    assert!(part.player.is_none() && part.notation.is_empty());
    // An empty notation means the instrument's, so it is not written.
    assert_eq!(
        project_to_value(&project)["tracks"][0]["part"],
        json!({"instrument": "horn_f"})
    );
}

#[test]
fn the_reader_reports_the_domain_rules_with_the_path() {
    let cases: [(Change, &str); 5] = [
        (
            |d| d["tracks"][0]["part"]["players"] = json!(0),
            "tracks/0/part: part players must be 1-128, got 0",
        ),
        (
            |d| d["tracks"][0]["part"]["players"] = json!(300),
            "tracks/0/part: invalid value (players 300 is out of range)",
        ),
        (
            |d| d["tracks"][0]["part"]["instrument"] = json!("Violin"),
            "tracks/0/part: part instrument must be an id of lowercase letters, digits and \
             underscores, got 'Violin'",
        ),
        (
            |d| d["tracks"][0]["part"]["notation"]["clef"] = json!("soprano"),
            "tracks/0/part: invalid value ('soprano' is not a valid Clef)",
        ),
        (
            |d| d["tracks"][0]["clips"][0]["notes"][0]["articulation"] = json!("Staccato"),
            "tracks/0/clips/0/0: note articulation must be an id of lowercase letters, digits \
             and underscores, got 'Staccato'",
        ),
    ];
    for (change, message) in cases {
        let mut doc = song_v2_document();
        change(&mut doc);
        let err = read(&doc).expect_err(message);
        assert_eq!(err.code, ErrorCode::FormatError);
        assert_eq!(err.message, message);
    }
    // A part belongs to an instrument track.
    let mut doc = song_v2_document();
    doc["tracks"][0]["kind"] = json!("bus");
    doc["tracks"][0]["clips"] = json!([]);
    doc["tracks"][0]["instrument"] = Value::Null;
    let err = read(&doc).expect_err("a bus plays no part");
    assert_eq!(err.message, "tracks/0: only instrument tracks have a part");
}

// -- opening -----------------------------------------------------------------------------

/// Upgrades a document as opening does, then reads it.
fn upgraded(doc: &Value) -> (Value, Option<i64>) {
    let (doc, from) = upgrade(object(doc.clone())).expect("upgraded");
    (Value::Object(doc), from)
}

#[test]
fn an_unchanged_version_1_document_reports_no_migration() {
    let (doc, from) = upgraded(&song_document());
    assert_eq!(from, None);
    let mut expected = song_document();
    expected["schema_version"] = json!(2);
    assert_eq!(doc, expected);
    // Written again, it is version 1 with the same bytes.
    assert_eq!(dumps(&read(&doc).expect("valid")), SONG);
}

#[test]
fn a_version_2_document_needs_no_migration() {
    assert_eq!(upgraded(&song_v2_document()), (song_v2_document(), None));
}

#[test]
fn a_valid_stray_part_of_version_1_becomes_the_part() {
    let mut v1 = song_document();
    v1["tracks"][0]["part"] = violins();
    let (doc, from) = upgraded(&v1);
    assert_eq!(from, None, "only the version number changed");
    let project = read(&doc).expect("valid");
    assert_eq!(
        project.tracks[0]
            .part
            .as_ref()
            .map(|p| p.instrument.as_str()),
        Some("violin")
    );
    // The sampler's instrument type is never lifted into a part.
    let mut sampler = song_document();
    sampler["tracks"][1]["instrument"]["params"]["instrument_type"] = json!("flute");
    let (doc, from) = upgraded(&sampler);
    assert_eq!(from, None);
    assert!(read(&doc).expect("valid").tracks[1].part.is_none());
}

#[test]
fn an_invalid_stray_part_is_kept_aside_and_reported() {
    for (kind, part) in [
        ("instrument", json!({"instrument": "Violin I"})),
        ("audio", json!({"instrument": "violin"})),
    ] {
        let mut v1 = song_document();
        v1["tracks"][0]["kind"] = json!(kind);
        v1["tracks"][0]["clips"] = json!([]);
        v1["tracks"][0]
            .as_object_mut()
            .expect("track")
            .shift_remove("instrument");
        v1["tracks"][0]["part"] = part.clone();
        let (doc, from) = upgraded(&v1);
        assert_eq!(from, Some(1), "{kind}");
        let project = read(&doc).expect("valid");
        assert!(project.tracks[0].part.is_none());
        assert_eq!(project.tracks[0].extra[PART_V1_KEY], part);
    }
}

#[test]
fn an_invalid_stray_note_articulation_is_dropped_and_reported() {
    let mut v1 = song_document();
    v1["tracks"][0]["clips"][0]["notes"][0]["articulation"] = json!(7);
    let (doc, from) = upgraded(&v1);
    assert_eq!(from, Some(1));
    assert_eq!(dumps(&read(&doc).expect("valid")), SONG);
}

#[test]
fn a_version_1_document_gets_the_version_1_verdict() {
    let mut v1 = song_document();
    v1["tracks"][0]["name"] = json!(7);
    let err = upgrade(object(v1)).expect_err("invalid");
    assert_eq!(
        err.message,
        "project-v1.schema.json: tracks/0/name: 7 is not of type 'string'"
    );
    // `True` passes the version check as a Python int, and the version-1 schema still
    // rejects it: the relabelling never turns it into a valid 2.
    let mut v1 = song_document();
    v1["schema_version"] = json!(true);
    let err = upgrade(object(v1)).expect_err("invalid");
    assert_eq!(
        err.message,
        "project-v1.schema.json: schema_version: 1 was expected"
    );
    let mut v2 = song_v2_document();
    v2["tracks"][0]["part"]["players"] = json!(0);
    let err = upgrade(object(v2)).expect_err("invalid");
    assert_eq!(
        err.message,
        "project-v2.schema.json: tracks/0/part/players: 0 is less than the minimum of 1"
    );
}

// -- the store ---------------------------------------------------------------------------

fn manifest_features(bundle: &Path) -> Value {
    let manifest: Value =
        serde_json::from_slice(&fs::read(bundle.join(MANIFEST)).expect("manifest")).expect("JSON");
    manifest["required_features"].clone()
}

#[test]
fn a_version_2_project_saves_and_opens_with_its_features() {
    assert!(SUPPORTED_FEATURES.contains(&"parts"));
    assert!(SUPPORTED_FEATURES.contains(&"note-articulations"));
    let dir = tempfile::tempdir().expect("temp dir");
    let store = ProjectStore::new(dir.path().join("workspaces"));
    let workspace = dir.path().join("ws");
    fs::create_dir_all(&workspace).expect("workspace");
    let project = read(&song_v2_document()).expect("valid");
    assert_eq!(
        required_features(&project),
        [
            "core",
            "midi-clips",
            "note-articulations",
            "parts",
            "sections"
        ]
    );
    let target = dir.path().join("Song.sonara");
    store
        .save(&project, &workspace, &target, None)
        .expect("saved");
    assert_eq!(
        manifest_features(&target),
        json!([
            "core",
            "midi-clips",
            "note-articulations",
            "parts",
            "sections"
        ])
    );
    let raw = fs::read(target.join(PROJECT)).expect("project.json");
    assert_eq!(
        serde_json::from_slice::<Value>(&raw).expect("JSON"),
        song_v2_document()
    );
    let loaded = store.open(&target, false).expect("opened");
    assert_eq!(loaded.project, project);
    assert_eq!(loaded.migrated_from, None);
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
}

#[test]
fn opening_reports_a_migration_only_when_content_changed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = ProjectStore::new(dir.path().join("workspaces"));
    let workspace = dir.path().join("ws");
    fs::create_dir_all(&workspace).expect("workspace");
    let target = dir.path().join("Song.sonara");
    let song = read(&song_document()).expect("valid");
    store.save(&song, &workspace, &target, None).expect("saved");
    assert_eq!(fs::read(target.join(PROJECT)).expect("bytes"), SONG);
    assert_eq!(
        manifest_features(&target),
        json!(["core", "midi-clips", "sections"])
    );
    let loaded = store.open(&target, false).expect("opened");
    assert_eq!((loaded.migrated_from, loaded.project), (None, song));

    let mut changed = song_document();
    changed["tracks"][2]["part"] = json!({"instrument": "Drums"});
    fs::write(target.join(PROJECT), changed.to_string()).expect("written");
    fs::remove_file(target.join(MANIFEST)).expect("removed");
    let loaded = store.open(&target, false).expect("opened");
    assert_eq!(loaded.migrated_from, Some(1));
    assert_eq!(
        loaded.project.tracks[2].extra[PART_V1_KEY],
        json!({"instrument": "Drums"})
    );

    // A newer version than this build reads is refused, not treated as damaged.
    let mut newer = song_v2_document();
    newer["schema_version"] = json!(SCHEMA_VERSION + 1);
    fs::write(target.join(PROJECT), newer.to_string()).expect("written");
    let err = store.open(&target, false).expect_err("newer");
    assert!(matches!(&err, StoreError::Domain(e) if e.code == ErrorCode::Unsupported));
}
