//! The project model and `project.json` (`sonara.persistence.serializer`,
//! docs/project-format.md).
//!
//! Writing is deterministic: keys in the prototype's order, Python float formatting
//! ([`crate::canonical`]). Unknown keys read from a file are kept in the owning object's `extra`
//! map and written back after the known keys, so files from newer Sonara versions survive a
//! round trip.
//!
//! A document is written in the lowest schema version that holds it
//! ([`lowest_schema_version`]): version 1, byte for byte as before, unless a track has a part or
//! a note has an articulation, which only version 2 can store.
//!
//! Reading mirrors the prototype's reader: values are coerced the way Python's `float()`,
//! `int()` and `bool()` coerce them, and errors carry the JSON path of the failing object
//! (`tracks/0/clips/1: clip length must be > 0`). Documents are schema-validated before they
//! are read, so the coercions only matter for documents built in memory.

use std::fmt::Display;
use std::sync::Arc;

use serde_json::{Map, Value};
use sonara_core::error::{DomainError, ErrorCode, Result};
use sonara_core::midi::{ControlKind, MidiControlEvent, MidiNote};
use sonara_core::model::JsonMap;
use sonara_core::model::automation::{
    AutomationLane, AutomationLaneData, AutomationPoint, AutomationTarget, CurveKind,
};
use sonara_core::model::clip::{
    AudioClip, AudioClipData, Clip, Fade, FadeCurve, MidiClip, MidiClipData,
};
use sonara_core::model::device::{Device, DeviceData, ParamValue, Params};
use sonara_core::model::markers::{Marker, Section};
use sonara_core::model::media::{MediaAsset, MediaAssetData, MediaKind, MediaLocation};
use sonara_core::model::part::{Clef, Notation, Part};
use sonara_core::model::project::{
    Project, ProjectData, ProjectMetadata, ProjectSettings, TransportSettings,
};
use sonara_core::model::track::{
    InputSettings, MixerSettings, MonitorMode, Send, Track, TrackData, TrackKind,
};
use sonara_core::pyfmt::{float_repr, str_repr};
use sonara_core::time::{TempoMap, TempoPoint, TimeSignatureMap, TimeSignaturePoint};

use crate::canonical::to_canonical_bytes;

/// The `format` value of a project document.
pub const FORMAT_ID: &str = "sonara.project";
/// The newest schema version: the one this build reads, and migrates older documents to.
pub const SCHEMA_VERSION: i64 = 2;

/// Whether a track of `project` has a part (schema version 2).
#[must_use]
pub fn has_parts(project: &Project) -> bool {
    project.all_tracks().any(|track| track.part.is_some())
}

/// Whether a note of `project` has an articulation (schema version 2).
#[must_use]
pub fn has_note_articulations(project: &Project) -> bool {
    project.all_tracks().any(|track| {
        track
            .midi_clips()
            .any(|clip| clip.notes.iter().any(|note| note.articulation.is_some()))
    })
}

/// The lowest schema version that can hold `project`, which is the version it is written in:
/// 2 when a track has a part or a note has an articulation, else 1. A project that uses nothing
/// of version 2 is written exactly as before it existed (GLD-01).
#[must_use]
pub fn lowest_schema_version(project: &Project) -> i64 {
    if has_parts(project) || has_note_articulations(project) {
        2
    } else {
        1
    }
}

// -- writing ------------------------------------------------------------------------------

/// A JSON object from `(key, value)` pairs, in order.
fn object<const N: usize>(entries: [(&str, Value); N]) -> Map<String, Value> {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
}

/// Appends the unknown keys that the known keys do not shadow (Python's `dict.setdefault`).
fn with_extra(mut base: Map<String, Value>, extra: &JsonMap) -> Value {
    for (key, value) in extra {
        if !base.contains_key(key) {
            base.insert(key.clone(), value.clone());
        }
    }
    Value::Object(base)
}

fn opt_str(value: Option<&str>) -> Value {
    value.map_or(Value::Null, Value::from)
}

/// The JSON form of a device.
#[must_use]
pub fn device_to_value(d: &Device) -> Value {
    let params: Map<String, Value> = d
        .params
        .iter()
        .map(|(key, value)| (key.clone(), Value::from(value)))
        .collect();
    with_extra(
        object([
            ("id", d.id.as_str().into()),
            ("plugin_id", d.plugin_id.as_str().into()),
            ("name", d.name.as_str().into()),
            ("enabled", d.enabled.into()),
            ("params", Value::Object(params)),
            ("sidechain_id", opt_str(d.sidechain_id.as_deref())),
            ("preset", opt_str(d.preset.as_deref())),
            ("state", Value::Object(d.state.clone())),
        ]),
        &d.extra,
    )
}

/// The JSON form of an automation lane.
#[must_use]
pub fn lane_to_value(lane: &AutomationLane) -> Value {
    let points: Vec<Value> = lane
        .points
        .iter()
        .map(|p| {
            Value::Object(object([
                ("beat", p.beat.into()),
                ("value", p.value.into()),
                ("curve", p.curve.as_str().into()),
            ]))
        })
        .collect();
    Value::Object(object([
        ("id", lane.id.as_str().into()),
        (
            "target",
            Value::Object(object([
                ("device_id", opt_str(lane.target.device_id.as_deref())),
                ("param", lane.target.param.as_str().into()),
            ])),
        ),
        ("enabled", lane.enabled.into()),
        ("points", Value::Array(points)),
    ]))
}

/// The JSON form of a MIDI note. The `articulation` key is written only when the note has one.
#[must_use]
pub fn note_to_value(n: &MidiNote) -> Value {
    let mut note = object([
        ("id", n.id.as_str().into()),
        ("pitch", n.pitch.into()),
        ("start", n.start.into()),
        ("length", n.length.into()),
        ("velocity", n.velocity.into()),
        ("channel", n.channel.into()),
    ]);
    if let Some(articulation) = &n.articulation {
        note.insert("articulation".to_owned(), articulation.as_str().into());
    }
    Value::Object(note)
}

/// The JSON form of a part: only the fields it states (the schema has no `null` for them).
#[must_use]
pub fn part_to_value(part: &Part) -> Value {
    let mut out = object([("instrument", part.instrument.as_str().into())]);
    if let Some(name) = &part.name {
        out.insert("name".to_owned(), name.as_str().into());
    }
    if let Some(players) = part.players {
        out.insert("players".to_owned(), players.into());
    }
    if let Some(solo) = part.solo {
        out.insert("solo".to_owned(), solo.into());
    }
    if let Some(player) = &part.player {
        out.insert("player".to_owned(), player.as_str().into());
    }
    if !part.notation.is_empty() {
        let mut notation = Map::new();
        if let Some(semitones) = part.notation.transposition {
            notation.insert("transposition".to_owned(), semitones.into());
        }
        if let Some(clef) = part.notation.clef {
            notation.insert("clef".to_owned(), clef.as_str().into());
        }
        out.insert("notation".to_owned(), Value::Object(notation));
    }
    Value::Object(out)
}

fn fade_to_value(fade: Fade) -> Value {
    Value::Object(object([
        ("seconds", fade.seconds.into()),
        ("curve", fade.curve.as_str().into()),
    ]))
}

/// The JSON form of a clip.
#[must_use]
pub fn clip_to_value(clip: &Clip) -> Value {
    match clip {
        Clip::Audio(c) => with_extra(
            object([
                ("type", "audio".into()),
                ("id", c.id.as_str().into()),
                ("name", c.name.as_str().into()),
                ("asset_id", c.asset_id.as_str().into()),
                ("start", c.start.into()),
                ("length", c.length.into()),
                ("source_offset", c.source_offset.into()),
                ("gain_db", c.gain_db.into()),
                ("fade_in", fade_to_value(c.fade_in)),
                ("fade_out", fade_to_value(c.fade_out)),
                ("muted", c.muted.into()),
                ("color", opt_str(c.color.as_deref())),
            ]),
            &c.extra,
        ),
        Clip::Midi(c) => {
            let controls: Vec<Value> = c
                .controls
                .iter()
                .map(|e| {
                    Value::Object(object([
                        ("beat", e.beat.into()),
                        ("kind", e.kind.as_str().into()),
                        ("number", e.number.into()),
                        ("value", e.value.into()),
                        ("channel", e.channel.into()),
                    ]))
                })
                .collect();
            with_extra(
                object([
                    ("type", "midi".into()),
                    ("id", c.id.as_str().into()),
                    ("name", c.name.as_str().into()),
                    ("start", c.start.into()),
                    ("length", c.length.into()),
                    ("content_offset", c.content_offset.into()),
                    (
                        "loop_length",
                        c.loop_length.map_or(Value::Null, Value::from),
                    ),
                    ("muted", c.muted.into()),
                    ("color", opt_str(c.color.as_deref())),
                    ("notes", c.notes.iter().map(note_to_value).collect()),
                    ("controls", Value::Array(controls)),
                ]),
                &c.extra,
            )
        }
    }
}

/// The JSON form of a track. The `part` key follows `kind`, and is written only when the track
/// has a part.
#[must_use]
pub fn track_to_value(t: &Track) -> Value {
    let m = &t.mixer;
    let sends: Vec<Value> = t
        .sends
        .iter()
        .map(|s| {
            Value::Object(object([
                ("id", s.id.as_str().into()),
                ("target_id", s.target_id.as_str().into()),
                ("level_db", s.level_db.into()),
                ("pre_fader", s.pre_fader.into()),
                ("enabled", s.enabled.into()),
            ]))
        })
        .collect();
    let mut track = object([
        ("id", t.id.as_str().into()),
        ("name", t.name.as_str().into()),
        ("kind", t.kind.as_str().into()),
    ]);
    if let Some(part) = &t.part {
        track.insert("part".to_owned(), part_to_value(part));
    }
    track.extend(object([
        ("color", opt_str(t.color.as_deref())),
        ("output_id", opt_str(t.output_id.as_deref())),
        ("parent_id", opt_str(t.parent_id.as_deref())),
        (
            "mixer",
            Value::Object(object([
                ("volume_db", m.volume_db.into()),
                ("pan", m.pan.into()),
                ("mute", m.mute.into()),
                ("solo", m.solo.into()),
                ("solo_safe", m.solo_safe.into()),
            ])),
        ),
        (
            "input",
            Value::Object(object([
                ("channels", t.input.channels.iter().copied().collect()),
                ("armed", t.input.armed.into()),
                ("monitor", t.input.monitor.as_str().into()),
            ])),
        ),
        (
            "instrument",
            t.instrument.as_ref().map_or(Value::Null, device_to_value),
        ),
        ("inserts", t.inserts.iter().map(device_to_value).collect()),
        ("sends", Value::Array(sends)),
        (
            "automation",
            t.automation.iter().map(lane_to_value).collect(),
        ),
        ("clips", t.clips.iter().map(clip_to_value).collect()),
    ]));
    with_extra(track, &t.extra)
}

/// The JSON form of a media asset.
#[must_use]
pub fn asset_to_value(a: &MediaAsset) -> Value {
    with_extra(
        object([
            ("id", a.id.as_str().into()),
            ("kind", a.kind.as_str().into()),
            ("name", a.name.as_str().into()),
            ("path", a.path.as_str().into()),
            ("location", a.location.as_str().into()),
            ("size_bytes", a.size_bytes.into()),
            ("sha256", opt_str(a.sha256.as_deref())),
            ("sample_rate", a.sample_rate.into()),
            ("channels", a.channels.into()),
            ("frames", a.frames.into()),
            ("format", a.format.as_str().into()),
            ("subtype", a.subtype.as_str().into()),
            ("original_path", opt_str(a.original_path.as_deref())),
            ("relative_hint", opt_str(a.relative_hint.as_deref())),
            ("metadata", Value::Object(a.metadata.clone())),
        ]),
        &a.extra,
    )
}

fn tempo_map_value(map: &TempoMap) -> Value {
    map.points()
        .iter()
        .map(|t| {
            Value::Object(object([
                ("beat", t.beat.into()),
                ("bpm", t.bpm.into()),
                ("ramp", t.ramp.into()),
            ]))
        })
        .collect()
}

fn time_signatures_value(map: &TimeSignatureMap) -> Value {
    map.points()
        .iter()
        .map(|t| {
            Value::Object(object([
                ("bar", t.bar.into()),
                ("numerator", t.numerator.into()),
                ("denominator", t.denominator.into()),
            ]))
        })
        .collect()
}

fn markers_value(markers: &[Marker]) -> Value {
    markers
        .iter()
        .map(|m| {
            Value::Object(object([
                ("id", m.id.as_str().into()),
                ("beat", m.beat.into()),
                ("name", m.name.as_str().into()),
                ("color", opt_str(m.color.as_deref())),
            ]))
        })
        .collect()
}

fn sections_value(sections: &[Section]) -> Value {
    sections
        .iter()
        .map(|x| {
            Value::Object(object([
                ("id", x.id.as_str().into()),
                ("name", x.name.as_str().into()),
                ("start", x.start.into()),
                ("length", x.length.into()),
                ("color", opt_str(x.color.as_deref())),
            ]))
        })
        .collect()
}

fn transport_value(tr: &TransportSettings) -> Value {
    Value::Object(object([
        ("loop_enabled", tr.loop_enabled.into()),
        ("loop_start", tr.loop_start.into()),
        ("loop_end", tr.loop_end.into()),
        ("punch_enabled", tr.punch_enabled.into()),
        ("punch_in", tr.punch_in.into()),
        ("punch_out", tr.punch_out.into()),
        ("metronome", tr.metronome.into()),
        ("count_in_bars", tr.count_in_bars.into()),
    ]))
}

fn metadata_value(md: &ProjectMetadata) -> Value {
    ProjectMetadata::FIELDS
        .iter()
        .map(|field| {
            (
                (*field).to_owned(),
                Value::from(md.get(field).unwrap_or_default()),
            )
        })
        .collect::<Map<String, Value>>()
        .into()
}

/// The JSON document of a project.
#[must_use]
pub fn project_to_value(p: &Project) -> Value {
    let s = &p.settings;
    let mut assets: Vec<&MediaAsset> = p.assets.iter().collect();
    assets.sort_by(|a, b| a.id.cmp(&b.id));
    with_extra(
        object([
            ("format", FORMAT_ID.into()),
            ("schema_version", lowest_schema_version(p).into()),
            ("id", p.id.as_str().into()),
            ("name", p.name.as_str().into()),
            (
                "settings",
                Value::Object(object([
                    ("sample_rate", s.sample_rate.into()),
                    (
                        "length_beats",
                        s.length_beats.map_or(Value::Null, Value::from),
                    ),
                    ("record_subtype", s.record_subtype.as_str().into()),
                ])),
            ),
            ("tempo_map", tempo_map_value(&p.tempo_map)),
            ("time_signatures", time_signatures_value(&p.time_signatures)),
            ("tracks", p.tracks.iter().map(track_to_value).collect()),
            ("master", track_to_value(&p.master)),
            ("assets", assets.into_iter().map(asset_to_value).collect()),
            ("markers", markers_value(&p.markers)),
            ("sections", sections_value(&p.sections)),
            ("transport", transport_value(&p.transport)),
            ("metadata", metadata_value(&p.metadata)),
            ("editor", Value::Object(p.editor.clone())),
        ]),
        &p.extra,
    )
}

/// The deterministic UTF-8 bytes of `project.json`.
#[must_use]
pub fn dumps(p: &Project) -> Vec<u8> {
    to_canonical_bytes(&project_to_value(p))
}

/// Parses `project.json` into a JSON object.
///
/// # Errors
///
/// [`ErrorCode::FormatError`] for invalid UTF-8 or JSON, or a document that is not an object.
pub fn loads(raw: &[u8]) -> Result<JsonMap> {
    let value: Value = serde_json::from_slice(raw).map_err(|err| {
        DomainError::new(
            ErrorCode::FormatError,
            format!("project.json is not valid JSON: {err}"),
        )
    })?;
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(DomainError::new(
            ErrorCode::FormatError,
            "project.json must contain a JSON object",
        )),
    }
}

// -- reading ------------------------------------------------------------------------------

/// A failure while reading: a domain rule, or a malformed value (Python's `KeyError`,
/// `TypeError` or `ValueError`, whose text is kept).
enum ReadError {
    Domain(DomainError),
    Value(String),
}

impl From<DomainError> for ReadError {
    fn from(err: DomainError) -> Self {
        Self::Domain(err)
    }
}

type Read<T> = std::result::Result<T, ReadError>;

/// Python's `repr` of a JSON value, for error messages.
fn py_repr(value: &Value) -> String {
    match value {
        Value::Null => "None".to_owned(),
        Value::Bool(true) => "True".to_owned(),
        Value::Bool(false) => "False".to_owned(),
        Value::Number(n) => n
            .as_i64()
            .map(|i| i.to_string())
            .or_else(|| n.as_u64().map(|u| u.to_string()))
            .unwrap_or_else(|| float_repr(n.as_f64().unwrap_or(f64::NAN))),
        Value::String(s) => str_repr(s),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

/// Python's type name of a JSON value.
fn py_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_f64() => "float",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

fn as_object(value: &Value) -> Read<&JsonMap> {
    value
        .as_object()
        .ok_or_else(|| ReadError::Value(format!("'{}' object is not a mapping", py_type(value))))
}

/// `d[key]`.
fn req<'a>(d: &'a JsonMap, key: &str) -> Read<&'a Value> {
    d.get(key).ok_or_else(|| ReadError::Value(str_repr(key)))
}

/// Python's `float(value)`.
fn py_float(value: &Value) -> Read<f64> {
    match value {
        Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| ReadError::Value(format!("invalid number {n}"))),
        Value::String(s) => s.trim().parse::<f64>().map_err(|_| {
            ReadError::Value(format!(
                "could not convert string to float: {}",
                str_repr(s)
            ))
        }),
        _ => Err(ReadError::Value(format!(
            "float() argument must be a string or a real number, not '{}'",
            py_type(value)
        ))),
    }
}

/// Python's `int(value)`: floats truncate toward zero.
fn py_int(value: &Value) -> Read<i64> {
    match value {
        Value::Bool(b) => Ok(i64::from(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i)
            } else if let Some(f) = n.as_f64().filter(|f| f.is_finite()) {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "Python's int(float) truncates toward zero"
                )]
                Ok(f.trunc() as i64)
            } else {
                Err(ReadError::Value(format!("cannot convert {n} to integer")))
            }
        }
        Value::String(s) => s.trim().parse::<i64>().map_err(|_| {
            ReadError::Value(format!(
                "invalid literal for int() with base 10: {}",
                str_repr(s)
            ))
        }),
        _ => Err(ReadError::Value(format!(
            "int() argument must be a string, a bytes-like object or a real number, not '{}'",
            py_type(value)
        ))),
    }
}

/// Python's `bool(value)` (truthiness).
fn py_bool(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Python's `str(value)`.
fn py_str(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => py_repr(other),
    }
}

fn string(value: &Value) -> Read<String> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| ReadError::Value(format!("expected a string, not '{}'", py_type(value))))
}

/// `d.get(key)` for an optional string.
fn opt_string(d: &JsonMap, key: &str) -> Read<Option<String>> {
    match d.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => string(value).map(Some),
    }
}

/// `d.get(key, default)` converted with `convert`.
fn get_or<T>(
    d: &JsonMap,
    key: &str,
    default: T,
    convert: impl FnOnce(&Value) -> Read<T>,
) -> Read<T> {
    d.get(key).map_or(Ok(default), convert)
}

fn get_bool(d: &JsonMap, key: &str, default: bool) -> bool {
    d.get(key).map_or(default, py_bool)
}

fn get_string(d: &JsonMap, key: &str, default: &str) -> Read<String> {
    get_or(d, key, default.to_owned(), string)
}

/// `d.get(key, {})` as an object (the prototype's `freeze` of a mapping).
fn get_map(d: &JsonMap, key: &str) -> Read<JsonMap> {
    get_or(d, key, JsonMap::new(), |v| as_object(v).cloned())
}

/// `d.get(key, [])` as a list.
fn get_list<'a>(d: &'a JsonMap, key: &str) -> Read<&'a [Value]> {
    match d.get(key) {
        None => Ok(&[]),
        Some(Value::Array(items)) => Ok(items),
        Some(other) => Err(ReadError::Value(format!(
            "'{}' object is not iterable",
            py_type(other)
        ))),
    }
}

/// Python's `SomeEnum(value)`.
fn py_enum<T>(value: &Value, name: &str, parse: impl FnOnce(&str) -> Option<T>) -> Read<T> {
    value
        .as_str()
        .and_then(parse)
        .ok_or_else(|| ReadError::Value(format!("{} is not a valid {name}", py_repr(value))))
}

/// The unknown keys of `d`.
fn extra(d: &JsonMap, known: &[&str]) -> JsonMap {
    d.iter()
        .filter(|(key, _)| !known.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn param_value(value: &Value) -> Read<ParamValue> {
    match value {
        Value::Bool(b) => Ok(ParamValue::Bool(*b)),
        Value::Number(n) => match n.as_i64() {
            Some(i) if !n.is_f64() => Ok(ParamValue::Int(i)),
            _ => py_float(value).map(ParamValue::Float),
        },
        Value::String(s) => Ok(ParamValue::Str(s.clone())),
        _ => Err(ReadError::Value(format!(
            "parameter values must be numbers, booleans or strings, not '{}'",
            py_type(value)
        ))),
    }
}

fn note(value: &Value) -> Read<MidiNote> {
    let n = as_object(value)?;
    let id = string(req(n, "id")?)?;
    let pitch = py_int(req(n, "pitch")?)?;
    let start = py_float(req(n, "start")?)?;
    let length = py_float(req(n, "length")?)?;
    let velocity = get_or(n, "velocity", 100, py_int)?;
    let channel = get_or(n, "channel", 0, py_int)?;
    let articulation = opt_string(n, "articulation")?;
    Ok(MidiNote::new(id, pitch, start, length, velocity, channel)?
        .with_articulation(articulation)?)
}

/// A part. Unknown keys inside it are not kept, as in the other small value objects.
fn part(value: &Value) -> Read<Part> {
    let p = as_object(value)?;
    let notation = get_map(p, "notation")?;
    let part = Part {
        instrument: string(req(p, "instrument")?)?,
        name: opt_string(p, "name")?,
        players: opt_int(p, "players")?,
        solo: p.get("solo").filter(|v| !v.is_null()).map(py_bool),
        player: opt_string(p, "player")?,
        notation: Notation {
            transposition: opt_int(&notation, "transposition")?,
            clef: match notation.get("clef") {
                None | Some(Value::Null) => None,
                Some(clef) => Some(py_enum(clef, "Clef", Clef::parse)?),
            },
        },
    };
    part.validate()?;
    Ok(part)
}

/// `d.get(key)` for an optional integer of type `T`.
fn opt_int<T: TryFrom<i64>>(d: &JsonMap, key: &str) -> Read<Option<T>> {
    match d.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            let number = py_int(value)?;
            T::try_from(number)
                .map(Some)
                .map_err(|_| ReadError::Value(format!("{key} {number} is out of range")))
        }
    }
}

fn section(value: &Value) -> Read<Section> {
    let x = as_object(value)?;
    let id = string(req(x, "id")?)?;
    let name = string(req(x, "name")?)?;
    let start = py_float(req(x, "start")?)?;
    let length = py_float(req(x, "length")?)?;
    Ok(Section::new(
        id,
        name,
        start,
        length,
        opt_string(x, "color")?,
    )?)
}

/// Builds model objects from a document, prefixing errors with the JSON path.
#[derive(Default)]
struct Reader {
    path: Vec<String>,
}

impl Reader {
    /// Runs `read` with `key` appended to the path; non-format errors become
    /// [`ErrorCode::FormatError`] with the path prefixed.
    fn at<T>(&mut self, key: impl Display, read: impl FnOnce(&mut Self) -> Read<T>) -> Read<T> {
        self.path.push(key.to_string());
        let result = read(self);
        let path = self.path.join("/");
        self.path.pop();
        result.map_err(|err| {
            let message = match err {
                ReadError::Domain(err) if err.code == ErrorCode::FormatError => {
                    return ReadError::Domain(err);
                }
                ReadError::Domain(err) => format!("{path}: {}", err.message),
                ReadError::Value(text) => format!("{path}: invalid value ({text})"),
            };
            ReadError::Domain(DomainError::new(ErrorCode::FormatError, message))
        })
    }

    /// `read` applied to every item, errors prefixed with `prefix/index` (or `index`).
    fn each<T>(
        &mut self,
        prefix: &str,
        items: &[Value],
        mut read: impl FnMut(&mut Self, &Value) -> Read<T>,
    ) -> Read<Vec<T>> {
        items
            .iter()
            .enumerate()
            .map(|(k, item)| {
                let key = if prefix.is_empty() {
                    k.to_string()
                } else {
                    format!("{prefix}/{k}")
                };
                self.at(key, |r| read(r, item))
            })
            .collect()
    }

    fn device(value: &Value) -> Read<Device> {
        const KNOWN: [&str; 8] = [
            "id",
            "plugin_id",
            "name",
            "enabled",
            "params",
            "sidechain_id",
            "preset",
            "state",
        ];
        let d = as_object(value)?;
        let id = string(req(d, "id")?)?;
        let plugin_id = string(req(d, "plugin_id")?)?;
        let name = match d.get("name") {
            Some(name) if py_bool(name) => string(name)?,
            _ => plugin_id.clone(),
        };
        let mut params = Params::new();
        for (key, value) in get_map(d, "params")? {
            params.insert(key, param_value(&value)?);
        }
        Ok(Device::new(DeviceData {
            id,
            plugin_id,
            name,
            params,
            enabled: get_bool(d, "enabled", true),
            sidechain_id: opt_string(d, "sidechain_id")?,
            preset: opt_string(d, "preset")?,
            state: get_map(d, "state")?,
            extra: extra(d, &KNOWN),
        })?)
    }

    fn lane(value: &Value) -> Read<AutomationLane> {
        let d = as_object(value)?;
        let target = as_object(req(d, "target")?)?;
        let id = string(req(d, "id")?)?;
        let target = AutomationTarget::new(
            opt_string(target, "device_id")?,
            string(req(target, "param")?)?,
        )?;
        let points = get_list(d, "points")?
            .iter()
            .map(|p| {
                let p = as_object(p)?;
                let beat = py_float(req(p, "beat")?)?;
                let value = py_float(req(p, "value")?)?;
                let curve = get_or(p, "curve", CurveKind::Linear, |v| {
                    py_enum(v, "CurveKind", CurveKind::parse)
                })?;
                Ok(AutomationPoint::new(beat, value, curve)?)
            })
            .collect::<Read<Vec<_>>>()?;
        Ok(AutomationLane::new(AutomationLaneData {
            id,
            target,
            points,
            enabled: get_bool(d, "enabled", true),
        })?)
    }

    fn fade(d: &JsonMap, key: &str) -> Read<Fade> {
        let Some(value) = d.get(key) else {
            return Ok(Fade::NONE);
        };
        let f = as_object(value)?;
        let seconds = get_or(f, "seconds", 0.0, py_float)?;
        let curve = get_or(f, "curve", FadeCurve::Linear, |v| {
            py_enum(v, "FadeCurve", FadeCurve::parse)
        })?;
        Ok(Fade::new(seconds, curve)?)
    }

    fn clip(&mut self, value: &Value) -> Read<Clip> {
        const AUDIO_KNOWN: [&str; 12] = [
            "type",
            "id",
            "name",
            "asset_id",
            "start",
            "length",
            "source_offset",
            "gain_db",
            "fade_in",
            "fade_out",
            "muted",
            "color",
        ];
        const MIDI_KNOWN: [&str; 11] = [
            "type",
            "id",
            "name",
            "start",
            "length",
            "content_offset",
            "loop_length",
            "muted",
            "color",
            "notes",
            "controls",
        ];
        let d = as_object(value)?;
        let kind = req(d, "type")?;
        match kind.as_str() {
            Some("audio") => {
                let id = string(req(d, "id")?)?;
                let name = get_string(d, "name", "")?;
                let asset_id = string(req(d, "asset_id")?)?;
                let start = py_float(req(d, "start")?)?;
                let length = py_float(req(d, "length")?)?;
                let source_offset = get_or(d, "source_offset", 0, py_int)?;
                let gain_db = get_or(d, "gain_db", 0.0, py_float)?;
                let fade_in = Self::fade(d, "fade_in")?;
                let fade_out = Self::fade(d, "fade_out")?;
                Ok(Clip::Audio(AudioClip::new(AudioClipData {
                    id,
                    name,
                    asset_id,
                    start,
                    length,
                    source_offset,
                    gain_db,
                    fade_in,
                    fade_out,
                    muted: get_bool(d, "muted", false),
                    color: opt_string(d, "color")?,
                    extra: extra(d, &AUDIO_KNOWN),
                })?))
            }
            Some("midi") => {
                let id = string(req(d, "id")?)?;
                let name = get_string(d, "name", "")?;
                let start = py_float(req(d, "start")?)?;
                let length = py_float(req(d, "length")?)?;
                let notes = self.each("", get_list(d, "notes")?, |_, v| note(v))?;
                let controls = get_list(d, "controls")?
                    .iter()
                    .map(|e| {
                        let e = as_object(e)?;
                        let beat = py_float(req(e, "beat")?)?;
                        let kind = py_enum(req(e, "kind")?, "ControlKind", ControlKind::parse)?;
                        let value = py_int(req(e, "value")?)?;
                        let number = get_or(e, "number", 0, py_int)?;
                        let channel = get_or(e, "channel", 0, py_int)?;
                        Ok(MidiControlEvent::new(beat, kind, value, number, channel)?)
                    })
                    .collect::<Read<Vec<_>>>()?;
                let content_offset = get_or(d, "content_offset", 0.0, py_float)?;
                let loop_length = match d.get("loop_length") {
                    None | Some(Value::Null) => None,
                    Some(v) => Some(py_float(v)?),
                };
                Ok(Clip::Midi(MidiClip::new(MidiClipData {
                    id,
                    name,
                    start,
                    length,
                    notes: Arc::new(notes),
                    controls: Arc::new(controls),
                    content_offset,
                    loop_length,
                    muted: get_bool(d, "muted", false),
                    color: opt_string(d, "color")?,
                    extra: extra(d, &MIDI_KNOWN),
                })?))
            }
            _ => Err(DomainError::new(
                ErrorCode::Unsupported,
                format!("unknown clip type {}", py_repr(kind)),
            )
            .into()),
        }
    }

    fn track(&mut self, value: &Value) -> Read<Track> {
        const KNOWN: [&str; 14] = [
            "id",
            "name",
            "kind",
            "part",
            "color",
            "output_id",
            "parent_id",
            "mixer",
            "input",
            "instrument",
            "inserts",
            "sends",
            "automation",
            "clips",
        ];
        let d = as_object(value)?;
        let id = string(req(d, "id")?)?;
        let name = string(req(d, "name")?)?;
        let kind = py_enum(req(d, "kind")?, "TrackKind", TrackKind::parse)?;
        let clips = self.each("clips", get_list(d, "clips")?, Self::clip)?;
        let instrument = match d.get("instrument") {
            Some(inst) if py_bool(inst) => Some(self.at("instrument", |_| Self::device(inst))?),
            _ => None,
        };
        let inserts = self.each("inserts", get_list(d, "inserts")?, |_, v| Self::device(v))?;
        let m = get_map(d, "mixer")?;
        let mixer = MixerSettings::new(
            get_or(&m, "volume_db", 0.0, py_float)?,
            get_or(&m, "pan", 0.0, py_float)?,
            get_bool(&m, "mute", false),
            get_bool(&m, "solo", false),
            get_bool(&m, "solo_safe", false),
        )?;
        let output_id = opt_string(d, "output_id")?;
        let sends = get_list(d, "sends")?
            .iter()
            .map(|s| {
                let s = as_object(s)?;
                let id = string(req(s, "id")?)?;
                let target_id = string(req(s, "target_id")?)?;
                let level_db = get_or(s, "level_db", 0.0, py_float)?;
                Ok(Send::new(
                    id,
                    target_id,
                    level_db,
                    get_bool(s, "pre_fader", false),
                    get_bool(s, "enabled", true),
                )?)
            })
            .collect::<Read<Vec<_>>>()?;
        let automation = self.each("automation", get_list(d, "automation")?, |_, v| {
            Self::lane(v)
        })?;
        let i = get_map(d, "input")?;
        let channels = if i.contains_key("channels") {
            get_list(&i, "channels")?
                .iter()
                .map(py_int)
                .collect::<Read<Vec<i64>>>()?
        } else {
            vec![0]
        };
        let monitor = get_or(&i, "monitor", MonitorMode::Auto, |v| {
            py_enum(v, "MonitorMode", MonitorMode::parse)
        })?;
        let input = InputSettings::new(&channels, get_bool(&i, "armed", false), monitor)?;
        let part = match d.get("part") {
            None | Some(Value::Null) => None,
            Some(value) => Some(self.at("part", |_| part(value))?),
        };
        Ok(Track::new(TrackData {
            id,
            name,
            kind,
            clips,
            instrument,
            inserts,
            mixer,
            output_id,
            sends,
            automation,
            input,
            parent_id: opt_string(d, "parent_id")?,
            color: opt_string(d, "color")?,
            part,
            extra: extra(d, &KNOWN),
        })?)
    }

    fn asset(value: &Value) -> Read<MediaAsset> {
        const KNOWN: [&str; 15] = [
            "id",
            "kind",
            "name",
            "path",
            "location",
            "size_bytes",
            "sha256",
            "sample_rate",
            "channels",
            "frames",
            "format",
            "subtype",
            "original_path",
            "relative_hint",
            "metadata",
        ];
        let d = as_object(value)?;
        let id = string(req(d, "id")?)?;
        let kind = py_enum(req(d, "kind")?, "MediaKind", MediaKind::parse)?;
        let name = get_string(d, "name", "")?;
        let path = string(req(d, "path")?)?;
        let location = py_enum(req(d, "location")?, "MediaLocation", MediaLocation::parse)?;
        let size_bytes = get_or(d, "size_bytes", 0, py_int)?;
        let sha256 = opt_string(d, "sha256")?;
        let sample_rate = get_or(d, "sample_rate", 0, py_int)?;
        let channels = get_or(d, "channels", 0, py_int)?;
        let frames = get_or(d, "frames", 0, py_int)?;
        let format = get_string(d, "format", "")?;
        let subtype = get_string(d, "subtype", "")?;
        // The prototype checks these before the rules of `MediaAssetData`.
        if !path.is_empty() && size_bytes < 0 {
            return Err(DomainError::invalid("asset size must be >= 0").into());
        }
        let audio = matches!(kind, MediaKind::Audio | MediaKind::Impulse);
        let asset = MediaAsset::new(MediaAssetData {
            id,
            kind,
            name,
            path,
            location,
            size_bytes: u64::try_from(size_bytes).unwrap_or(0),
            sha256,
            sample_rate: u32::try_from(sample_rate).unwrap_or(0),
            channels: u32::try_from(channels).unwrap_or(0),
            frames: u64::try_from(frames).unwrap_or(0),
            format,
            subtype,
            original_path: opt_string(d, "original_path")?,
            relative_hint: opt_string(d, "relative_hint")?,
            metadata: get_map(d, "metadata")?,
            extra: extra(d, &KNOWN),
        })?;
        // The prototype checks the frame count last.
        if audio && frames < 0 {
            return Err(DomainError::invalid("audio asset frame count must be >= 0").into());
        }
        Ok(asset)
    }

    fn project(&mut self, d: &JsonMap) -> Read<Project> {
        const KNOWN: [&str; 15] = [
            "format",
            "schema_version",
            "id",
            "name",
            "settings",
            "tempo_map",
            "time_signatures",
            "tracks",
            "master",
            "assets",
            "markers",
            "sections",
            "transport",
            "metadata",
            "editor",
        ];
        let id = string(req(d, "id")?)?;
        let name = string(req(d, "name")?)?;
        let settings = self.at("settings", |_| settings(d))?;
        let tempo_map = self.at("tempo_map", |_| tempo_map(d))?;
        let time_signatures = self.at("time_signatures", |_| time_signatures(d))?;
        let tracks = self.each("tracks", get_list(d, "tracks")?, Self::track)?;
        let master = self.at("master", |r| r.track(req(d, "master")?))?;
        let assets = self.each("assets", get_list(d, "assets")?, |_, v| Self::asset(v))?;
        let markers = get_list(d, "markers")?
            .iter()
            .map(marker)
            .collect::<Read<Vec<_>>>()?;
        let sections = self.each("sections", get_list(d, "sections")?, |_, v| section(v))?;
        let transport = self.at("transport", |_| transport(d))?;
        let metadata = metadata(d)?;
        Ok(Project::new(ProjectData {
            id,
            name,
            settings,
            tempo_map,
            time_signatures,
            tracks,
            master,
            assets,
            markers,
            sections,
            transport,
            metadata,
            editor: get_map(d, "editor")?,
            extra: extra(d, &KNOWN),
        })?)
    }
}

fn settings(d: &JsonMap) -> Read<ProjectSettings> {
    let s = get_map(d, "settings")?;
    let sample_rate = get_or(&s, "sample_rate", 48_000, py_int)?;
    let length_beats = match s.get("length_beats") {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => n.as_f64(),
        Some(other) => {
            return Err(ReadError::Value(format!(
                "must be real number, not {}",
                py_type(other)
            )));
        }
    };
    let subtype = get_string(&s, "record_subtype", "PCM_24")?;
    Ok(ProjectSettings::new(sample_rate, length_beats, subtype)?)
}

fn tempo_map(d: &JsonMap) -> Read<TempoMap> {
    let points = req_list(d, "tempo_map")?
        .iter()
        .map(|t| {
            let t = as_object(t)?;
            let beat = py_float(req(t, "beat")?)?;
            let bpm = py_float(req(t, "bpm")?)?;
            Ok(TempoPoint::new(beat, bpm, get_bool(t, "ramp", false))?)
        })
        .collect::<Read<Vec<_>>>()?;
    Ok(TempoMap::new(points)?)
}

fn time_signatures(d: &JsonMap) -> Read<TimeSignatureMap> {
    let points = req_list(d, "time_signatures")?
        .iter()
        .map(|t| {
            let t = as_object(t)?;
            let bar = py_int(req(t, "bar")?)?;
            let numerator = py_int(req(t, "numerator")?)?;
            let denominator = py_int(req(t, "denominator")?)?;
            // Out-of-range values fail validation with the same messages as 0 does.
            Ok(TimeSignaturePoint::new(
                bar,
                u32::try_from(numerator).unwrap_or(0),
                u32::try_from(denominator).unwrap_or(0),
            )?)
        })
        .collect::<Read<Vec<_>>>()?;
    Ok(TimeSignatureMap::new(points)?)
}

fn marker(value: &Value) -> Read<Marker> {
    let m = as_object(value)?;
    let id = string(req(m, "id")?)?;
    let beat = py_float(req(m, "beat")?)?;
    let name = get_string(m, "name", "")?;
    Ok(Marker::new(id, beat, name, opt_string(m, "color")?)?)
}

fn transport(d: &JsonMap) -> Read<TransportSettings> {
    let tr = get_map(d, "transport")?;
    let count_in = get_or(&tr, "count_in_bars", 0, py_int)?;
    let transport = TransportSettings {
        loop_enabled: get_bool(&tr, "loop_enabled", false),
        loop_start: get_or(&tr, "loop_start", 0.0, py_float)?,
        loop_end: get_or(&tr, "loop_end", 16.0, py_float)?,
        punch_enabled: get_bool(&tr, "punch_enabled", false),
        punch_in: get_or(&tr, "punch_in", 0.0, py_float)?,
        punch_out: get_or(&tr, "punch_out", 16.0, py_float)?,
        metronome: get_bool(&tr, "metronome", false),
        // An invalid count-in fails validation after the loop and punch checks, as in the
        // prototype.
        count_in_bars: TransportSettings::count_in(count_in).unwrap_or(u32::MAX),
    };
    transport.validate()?;
    Ok(transport)
}

/// The eight metadata fields via Python's `str()`; other keys are dropped.
fn metadata(d: &JsonMap) -> Read<ProjectMetadata> {
    let md = get_map(d, "metadata")?;
    let mut metadata = ProjectMetadata::default();
    for field in ProjectMetadata::FIELDS {
        if let Some(slot) = metadata.get_mut(field) {
            *slot = md.get(field).map_or_else(String::new, py_str);
        }
    }
    Ok(metadata)
}

/// `d[key]` as a list.
fn req_list<'a>(d: &'a JsonMap, key: &str) -> Read<&'a [Value]> {
    match req(d, key)? {
        Value::Array(items) => Ok(items),
        other => Err(ReadError::Value(format!(
            "'{}' object is not iterable",
            py_type(other)
        ))),
    }
}

/// Builds a project from a (migrated, schema-validated) document.
///
/// # Errors
///
/// [`ErrorCode::FormatError`] for a document that is not a Sonara project or holds malformed
/// values (with the JSON path), or the domain error of a violated project-level rule.
pub fn project_from_value(data: &JsonMap) -> Result<Project> {
    if data.get("format").and_then(Value::as_str) != Some(FORMAT_ID) {
        let format = data
            .get("format")
            .map_or_else(|| "None".to_owned(), py_repr);
        return Err(DomainError::new(
            ErrorCode::FormatError,
            format!("not a Sonara project document (format={format})"),
        ));
    }
    Reader::default().project(data).map_err(|err| match err {
        ReadError::Domain(err) => err,
        ReadError::Value(text) => DomainError::new(
            ErrorCode::FormatError,
            format!("invalid project document: {text}"),
        ),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sonara_core::error::ErrorCode;
    use sonara_core::ids::SequentialIds;
    use sonara_core::model::project::Project;

    use super::{dumps, loads, project_from_value};

    #[test]
    fn a_new_project_round_trips() {
        let mut ids = SequentialIds::new(0);
        let project = Project::create(&mut ids, "Song", 48_000, 120.0).expect("valid");
        let bytes = dumps(&project);
        let back = project_from_value(&loads(&bytes).expect("JSON object")).expect("valid");
        assert_eq!(back, project);
        assert_eq!(dumps(&back), bytes);
    }

    #[test]
    fn errors_carry_the_json_path() {
        let mut ids = SequentialIds::new(0);
        let project = Project::create(&mut ids, "Song", 48_000, 120.0).expect("valid");
        let mut doc = loads(&dumps(&project)).expect("JSON object");
        doc.insert(
            "tracks".into(),
            json!([{"id": "trk_1", "name": "Keys", "kind": "instrument", "clips": [
                {"type": "midi", "id": "clp_1", "start": 0.0, "length": 4.0,
                 "notes": [{"id": "n_1", "pitch": 200, "start": 0.0, "length": 1.0}]}
            ]}]),
        );
        let err = project_from_value(&doc).expect_err("pitch out of range");
        assert_eq!(err.code, ErrorCode::FormatError);
        assert_eq!(
            err.message,
            "tracks/0/clips/0/0: note pitch must be 0-127, got 200"
        );
        doc.remove("id");
        let err = project_from_value(&doc).expect_err("missing id");
        assert_eq!(err.message, "invalid project document: 'id'");
    }
}
