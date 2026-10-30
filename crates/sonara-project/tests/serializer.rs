//! The serializer and its model behaviour follow the prototype
//! (`tests/project_format/test_serializer.py`): exact round trips, deterministic bytes,
//! error paths and unknown-key preservation. The store parts of that module (schema
//! validation, migrations, required features) are ported with the store.
//!
//! The rich project is the prototype's `build_rich_project` fixture. Its media asset carries the
//! values the fixture reads from its 0.5-second WAV file, and the expected document hashes are
//! the prototype's `dumps` output.

use std::sync::Arc;

use serde_json::{Value, json};
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::ids::SequentialIds;
use sonara_core::midi::{ControlKind, MidiControlEvent, MidiNote};
use sonara_core::model::JsonMap;
use sonara_core::model::automation::{
    AutomationLane, AutomationLaneData, AutomationPoint, AutomationTarget, CurveKind,
};
use sonara_core::model::clip::{AudioClip, AudioClipData, Fade, FadeCurve, MidiClip, MidiClipData};
use sonara_core::model::device::{Device, DeviceData, ParamValue, Params};
use sonara_core::model::markers::{Marker, Section};
use sonara_core::model::media::{MediaAsset, MediaAssetData, MediaKind, MediaLocation};
use sonara_core::model::project::{Project, ProjectMetadata, ProjectSettings, TransportSettings};
use sonara_core::model::track::{
    InputSettings, MixerSettings, MonitorMode, Send, Track, TrackData, TrackKind,
};
use sonara_core::routing;
use sonara_core::time::{TempoMap, TempoPoint, TimeSignatureMap, TimeSignaturePoint};
use sonara_project::serializer::{
    FORMAT_ID, SCHEMA_VERSION, dumps, loads, lowest_schema_version, project_from_value,
    project_to_value,
};
use sonara_testkit::sha256_hex;

/// The prototype's `dumps(build_rich_project(...))`: length and SHA-256.
const RICH_DOCUMENT: (usize, &str) = (
    8534,
    "ffc18c9921cde88586d405ddbae1432269d86232141f8b19e8a2dcd6e4de719a",
);

fn map(value: Value) -> JsonMap {
    match value {
        Value::Object(map) => map,
        other => panic!("not a JSON object: {other}"),
    }
}

fn params(value: Value) -> Params {
    map(value)
        .into_iter()
        .map(|(key, value)| {
            let value = match value {
                Value::Bool(b) => ParamValue::Bool(b),
                Value::String(s) => ParamValue::Str(s),
                Value::Number(n) if n.is_f64() => ParamValue::Float(n.as_f64().expect("float")),
                Value::Number(n) => ParamValue::Int(n.as_i64().expect("integer")),
                other => panic!("not a parameter value: {other}"),
            };
            (key, value)
        })
        .collect()
}

fn device(id: &str, plugin_id: &str, name: &str, change: impl FnOnce(&mut DeviceData)) -> Device {
    let mut data = DeviceData {
        id: id.to_owned(),
        plugin_id: plugin_id.to_owned(),
        name: name.to_owned(),
        params: Params::new(),
        enabled: true,
        sidechain_id: None,
        preset: None,
        state: JsonMap::new(),
        extra: JsonMap::new(),
    };
    change(&mut data);
    Device::new(data).expect("valid device")
}

fn track(id: &str, name: &str, kind: TrackKind, change: impl FnOnce(&mut TrackData)) -> Track {
    let mut data = TrackData::new(id, name, kind);
    change(&mut data);
    Track::new(data).expect("valid track")
}

fn point(beat: f64, value: f64, curve: CurveKind) -> AutomationPoint {
    AutomationPoint::new(beat, value, curve).expect("valid point")
}

fn lane(
    id: &str,
    device_id: Option<&str>,
    param: &str,
    points: Vec<AutomationPoint>,
    enabled: bool,
) -> AutomationLane {
    AutomationLane::new(AutomationLaneData {
        id: id.to_owned(),
        target: AutomationTarget::new(device_id.map(str::to_owned), param).expect("valid target"),
        points,
        enabled,
    })
    .expect("valid lane")
}

/// The prototype's `asset_for(workspace)`: a 0.5-second 48 kHz stereo `PCM_24` WAV.
fn rich_asset() -> MediaAsset {
    MediaAsset::new(MediaAssetData {
        id: "med_vox".to_owned(),
        kind: MediaKind::Audio,
        name: "vox.wav".to_owned(),
        path: "media/audio/med_vox-vox.wav".to_owned(),
        location: MediaLocation::Embedded,
        size_bytes: 144_044,
        sha256: Some("3ab030a4879fc982317fbfc28b75260f62a672db7ea2173b479bff216757baa1".to_owned()),
        sample_rate: 48_000,
        channels: 2,
        frames: 24_000,
        format: "WAV".to_owned(),
        subtype: "PCM_24".to_owned(),
        original_path: Some("/original/vox.wav".to_owned()),
        relative_hint: None,
        metadata: map(json!({"title": "Vox"})),
        extra: JsonMap::new(),
    })
    .expect("valid asset")
}

/// The prototype's `build_rich_project`: every feature the format carries, with awkward values
/// (1/3 beats, negative pan, −144 dB, Unicode).
#[expect(
    clippy::too_many_lines,
    reason = "mirrors conftest.py::build_rich_project"
)]
fn rich_project() -> Project {
    let asset = rich_asset();
    let synth = device("dev_syn", "sonara.synth.poly", "Synth", |d| {
        d.params = params(
            json!({"filter_cutoff": 1234.5, "unison": 3, "voice_mode": "mono", "glide": 0.0}),
        );
        d.preset = Some("Analog Bass".to_owned());
    });
    let comp = device("dev_cmp", "sonara.dynamics.compressor", "Comp", |d| {
        d.params = params(json!({"threshold_db": -24.0, "sidechain": true}));
        d.sidechain_id = Some("trk_drm".to_owned());
    });
    let eq = device("dev_eq", "sonara.eq.parametric", "EQ", |d| {
        d.params = params(json!({"low_gain": -1.5}));
        d.enabled = false;
    });
    let conv = device("dev_ir", "sonara.convolver", "IR", |d| {
        d.state = map(json!({"ir_asset_id": asset.id, "nested": {"a": [1, 2]}}));
    });
    let notes = vec![
        MidiNote::new("n_1", 36, 0.0, 0.5, 120, 0).expect("valid"),
        MidiNote::new("n_2", 43, 1.0 / 3.0, 2.0 / 3.0, 90, 2).expect("valid"),
        MidiNote::new("n_3", 48, 2.75, 1.25, 1, 0).expect("valid"),
    ];
    let controls = vec![
        MidiControlEvent::new(0.5, ControlKind::Cc, 64, 11, 0).expect("valid"),
        MidiControlEvent::new(1.0, ControlKind::PitchBend, -8192, 0, 0).expect("valid"),
        MidiControlEvent::new(2.0, ControlKind::ChannelPressure, 70, 0, 3).expect("valid"),
    ];
    let midi = MidiClip::new(MidiClipData {
        id: "clp_bass".to_owned(),
        name: "Bass Line".to_owned(),
        start: 4.0,
        length: 8.0,
        notes: Arc::new(notes),
        controls: Arc::new(controls),
        content_offset: 0.5,
        loop_length: Some(4.0),
        muted: false,
        color: Some("#123abc".to_owned()),
        extra: JsonMap::new(),
    })
    .expect("valid clip");
    let audio = AudioClip::new(AudioClipData {
        id: "clp_vox".to_owned(),
        name: "Vox".to_owned(),
        asset_id: asset.id.clone(),
        start: 2.0,
        length: 3.5,
        source_offset: 1200,
        gain_db: -3.25,
        fade_in: Fade::new(0.01, FadeCurve::EqualPower).expect("valid"),
        fade_out: Fade::new(0.2, FadeCurve::SCurve).expect("valid"),
        muted: true,
        color: None,
        extra: JsonMap::new(),
    })
    .expect("valid clip");
    let lane_vol = lane(
        "aut_vol",
        None,
        "volume_db",
        vec![
            point(0.0, -6.0, CurveKind::Smooth),
            point(8.0, 0.0, CurveKind::Linear),
        ],
        true,
    );
    let lane_cut = lane(
        "aut_cut",
        Some("dev_syn"),
        "filter_cutoff",
        vec![
            point(4.0, 200.0, CurveKind::Hold),
            point(6.0, 8000.0, CurveKind::Linear),
        ],
        false,
    );
    let lane_snd = lane(
        "aut_snd",
        None,
        "send.snd_1.level_db",
        vec![point(1.0, -12.0, CurveKind::Linear)],
        true,
    );
    let folder = track("trk_fld", "Rhythm", TrackKind::Folder, |d| {
        d.color = Some("#aa0000".to_owned());
    });
    let drums = track("trk_drm", "Drums", TrackKind::Instrument, |d| {
        d.instrument = Some(device("dev_drm", "sonara.drums", "Kit", |_| {}));
        d.parent_id = Some("trk_fld".to_owned());
    });
    let bass = track("trk_bas", "Bass", TrackKind::Instrument, |d| {
        d.clips = vec![midi.into()];
        d.instrument = Some(synth);
        d.inserts = vec![comp, eq];
        d.mixer = MixerSettings::new(-4.5, -0.25, false, true, false).expect("valid");
        d.output_id = Some("trk_bus".to_owned());
        d.sends = vec![Send::new("snd_1", "trk_bus", -9.0, true, false).expect("valid")];
        d.automation = vec![lane_vol, lane_cut, lane_snd];
        d.input = InputSettings::new(&[1], true, MonitorMode::On).expect("valid");
        d.parent_id = Some("trk_fld".to_owned());
    });
    let vox = track("trk_vox", "Vocals", TrackKind::Audio, |d| {
        d.clips = vec![audio.into()];
        d.mixer = MixerSettings::new(0.0, 0.0, true, false, true).expect("valid");
    });
    let bus = track("trk_bus", "Verb", TrackKind::Bus, |d| {
        d.inserts = vec![conv];
        d.mixer = MixerSettings::new(-144.0, 0.0, false, false, false).expect("valid");
    });
    let project = Project::create(&mut SequentialIds::new(0), "Rich ✓ Ünïcode", 44_100, 120.0)
        .expect("valid project");
    let master = project
        .master
        .edit(|d| {
            d.inserts = vec![device("dev_lim", "sonara.dynamics.limiter", "Lim", |_| {})];
            d.mixer = MixerSettings::new(-0.5, 0.0, false, false, false).expect("valid");
        })
        .expect("valid master");
    project
        .edit(|d| {
            d.settings = ProjectSettings::new(44_100, Some(64.0), "FLOAT").expect("valid");
            d.tempo_map = TempoMap::new(vec![
                TempoPoint::new(0.0, 128.0, true).expect("valid"),
                TempoPoint::new(16.0, 96.5, false).expect("valid"),
                TempoPoint::new(32.0, 140.0, false).expect("valid"),
            ])
            .expect("valid");
            d.time_signatures = TimeSignatureMap::new(vec![
                TimeSignaturePoint::new(0, 4, 4).expect("valid"),
                TimeSignaturePoint::new(4, 7, 8).expect("valid"),
            ])
            .expect("valid");
            d.tracks = vec![folder, drums, bass, vox, bus];
            d.master = master;
            d.assets = vec![asset];
            d.markers =
                vec![Marker::new("mrk_1", 16.0, "Drop", Some("#00ff00".to_owned())).expect("ok")];
            d.sections = vec![
                Section::new("sec_i", "Intro", 0.0, 16.0, None).expect("valid"),
                Section::new("sec_v", "Verse", 16.0, 16.0, Some("#0000ff".to_owned()))
                    .expect("valid"),
            ];
            d.transport = TransportSettings {
                loop_enabled: true,
                loop_start: 4.0,
                loop_end: 12.0,
                punch_enabled: true,
                punch_in: 2.0,
                punch_out: 6.0,
                metronome: true,
                count_in_bars: 2,
            };
            d.metadata = ProjectMetadata {
                title: "Title".to_owned(),
                artist: "Artist".to_owned(),
                album: "Album".to_owned(),
                genre: "Genre".to_owned(),
                composer: "Composer".to_owned(),
                comment: "Comment".to_owned(),
                created: "2026-10-20T00:00:00Z".to_owned(),
                modified: String::new(),
            };
            d.editor = map(json!({"zoom": 1.5, "snap": "1/16", "heights": {"trk_bas": 80}}));
        })
        .expect("valid project")
}

fn roundtrip(p: &Project) -> Project {
    project_from_value(&loads(&dumps(p)).expect("a JSON object")).expect("a valid document")
}

/// The document of `p` as a mutable JSON value.
fn document(p: &Project) -> Value {
    project_to_value(p)
}

fn read(doc: &Value) -> Result<Project, DomainError> {
    project_from_value(doc.as_object().expect("a JSON object"))
}

fn assert_format_error(result: Result<Project, DomainError>, message: &str) {
    let err = result.expect_err(message);
    assert_eq!(
        (err.code, err.message.as_str()),
        (ErrorCode::FormatError, message)
    );
}

#[test]
fn rich_project_round_trips_exactly() {
    let rich = rich_project();
    routing::validate(&rich).expect("valid routing");
    let back = roundtrip(&rich);
    assert_eq!(back, rich);
    // Spot-check values that are easy to lose.
    let bass = back.track("trk_bas").expect("exists");
    let notes = &bass.clips[0].as_midi().expect("a MIDI clip").notes;
    assert!((notes[1].start - 1.0 / 3.0).abs() < f64::EPSILON);
    assert!(bass.input.armed && bass.sends[0].pre_fader && !bass.sends[0].enabled);
    assert!(back.tempo_map.points()[0].ramp);
    let verb = back.track("trk_bus").expect("exists");
    assert_eq!(verb.inserts[0].state["nested"], json!({"a": [1, 2]}));
    assert!((verb.mixer.volume_db + 144.0).abs() < f64::EPSILON);
}

#[test]
fn serialization_is_deterministic() {
    let rich = rich_project();
    let a = dumps(&rich);
    assert_eq!(a, dumps(&rich));
    assert_eq!(dumps(&roundtrip(&rich)), a);
    let doc: Value = serde_json::from_slice(&a).expect("valid JSON");
    assert_eq!(doc["format"], FORMAT_ID);
    // No part and no note articulation: the lowest version that holds the project (GLD-01).
    assert_eq!(doc["schema_version"], 1);
    assert_eq!(lowest_schema_version(&rich), 1);
    assert_ne!(lowest_schema_version(&rich), SCHEMA_VERSION);
    // UTF-8, not escaped.
    assert!(a.ends_with(b"\n"));
    assert!(
        a.windows("Ünïcode".len())
            .any(|w| w == "Ünïcode".as_bytes())
    );
    // The prototype writes exactly these bytes.
    assert_eq!((a.len(), sha256_hex(&a).as_str()), RICH_DOCUMENT);
}

#[test]
fn semantic_errors_report_path() {
    let rich = rich_project();
    // An audio clip on an instrument track.
    let mut doc = document(&rich);
    doc["tracks"][1]["clips"] = json!([doc["tracks"][3]["clips"][0].clone()]);
    assert_format_error(
        read(&doc),
        "tracks/1: instrument track cannot hold audio clips",
    );
    let mut doc = document(&rich);
    doc["tracks"][2]["clips"][0]["type"] = json!("video");
    assert_format_error(read(&doc), "tracks/2/clips/0: unknown clip type 'video'");
    let mut doc = document(&rich);
    doc["tracks"][2]["clips"][0]["notes"][0]["start"] = json!("zero");
    assert_format_error(
        read(&doc),
        "tracks/2/clips/0/0: invalid value (could not convert string to float: 'zero')",
    );
}

/// The prototype's JSON decoder words its syntax errors differently from `serde_json`, so the
/// two invalid-JSON cases check the code and the prototype's message prefix.
#[test]
fn wrong_format_and_bad_json() {
    assert_format_error(
        read(&json!({"format": "nope"})),
        "not a Sonara project document (format='nope')",
    );
    assert_format_error(
        read(&json!({})),
        "not a Sonara project document (format=None)",
    );
    for raw in [&b"{not json"[..], &b"\xff\xfe"[..]] {
        let err = loads(raw).expect_err("invalid JSON");
        assert_eq!(err.code, ErrorCode::FormatError);
        assert!(
            err.message.starts_with("project.json is not valid JSON: "),
            "{}",
            err.message
        );
    }
    let err = loads(b"[1, 2]").expect_err("not an object");
    assert_eq!(
        (err.code, err.message.as_str()),
        (
            ErrorCode::FormatError,
            "project.json must contain a JSON object"
        )
    );
}

#[test]
fn unknown_keys_survive_round_trip() {
    let mut doc = document(&rich_project());
    doc["future_top"] = json!({"x": [1, 2]});
    doc["tracks"][2]["future_track"] = json!("t");
    doc["tracks"][2]["clips"][0]["future_clip"] = json!(1.5);
    doc["tracks"][3]["clips"][0]["future_audio"] = json!(true);
    doc["tracks"][2]["inserts"][0]["future_device"] = json!({"deep": null});
    doc["assets"][0]["future_asset"] = json!("a");
    let bytes = dumps(&read(&doc).expect("a valid document"));
    let out: Value = serde_json::from_slice(&bytes).expect("valid JSON");
    assert_eq!(out["future_top"], json!({"x": [1, 2]}));
    assert_eq!(out["tracks"][2]["future_track"], "t");
    assert_eq!(out["tracks"][2]["clips"][0]["future_clip"], 1.5);
    assert_eq!(out["tracks"][3]["clips"][0]["future_audio"], true);
    assert_eq!(
        out["tracks"][2]["inserts"][0]["future_device"],
        json!({"deep": null})
    );
    assert_eq!(out["assets"][0]["future_asset"], "a");
    // Known keys always win over a colliding preserved value.
    assert_eq!(out["format"], "sonara.project");
    // The prototype writes the same bytes, with an extra top-level key after the known ones.
    doc["format_extra"] = json!(1);
    let bytes = dumps(&read(&doc).expect("a valid document"));
    assert_eq!(
        (bytes.len(), sha256_hex(&bytes).as_str()),
        (
            8750,
            "476cfd4381a9c330ac6b5583b80220698195f15f0c65a3c4067126af81deb285"
        )
    );
}

/// The prototype also validates this document against the schema; schema validation is ported
/// with the store.
#[test]
fn minimal_document_defaults() {
    let doc = json!({
        "format": "sonara.project", "schema_version": 1, "id": "prj_x", "name": "Min",
        "settings": {"sample_rate": 48000}, "tempo_map": [{"beat": 0, "bpm": 100}],
        "time_signatures": [{"bar": 0, "numerator": 3, "denominator": 4}],
        "tracks": [], "master": {"id": "master", "name": "Master", "kind": "master"},
    });
    let p = read(&doc).expect("a valid document");
    assert!((p.tempo_map.points()[0].bpm - 100.0).abs() < f64::EPSILON);
    assert_eq!(p.time_signatures.points()[0].numerator, 3);
    assert_eq!(p.master.output_id, None);
    assert_eq!(p.assets.len(), 0);
    // The defaults the prototype fills in, byte for byte.
    let bytes = dumps(&p);
    assert_eq!(
        (bytes.len(), sha256_hex(&bytes).as_str()),
        (
            1189,
            "26941050574419b16c0a459460c6a4c365fb0beaa1e1ceed7a2e35d63b7eccb5"
        )
    );
}
