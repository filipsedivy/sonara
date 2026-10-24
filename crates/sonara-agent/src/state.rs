//! Agent-facing views of project state (`sonara.agent.state`). Every position is given in beats
//! *and* seconds, with a 1-based `bar:beat` label, so agents never do tempo math themselves.
//!
//! Keys appear in the prototype's order, and values are rounded as it rounds them: positions to
//! 6 decimals in beats and 4 in seconds, clip lengths to 6, asset durations to 3, each like
//! Python's `round(x, n)`.

use serde_json::{Map, Value, json};
use sonara_core::midi::note_name;
use sonara_core::model::automation::AutomationLane;
use sonara_core::model::clip::Clip;
use sonara_core::model::device::{Device, Params};
use sonara_core::model::project::{Project, ProjectMetadata};
use sonara_core::model::track::{Track, TrackKind};
use sonara_core::plugins::PluginRegistry;
use sonara_core::pyfmt::py_round;

/// What [`project_state`] reports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StateOptions<'a> {
    /// `detail="full"`: device parameters, units and states, and automation points.
    pub full: bool,
    /// Every MIDI note of every clip.
    pub include_notes: bool,
    /// Only these tracks (`None`: all). The master is always reported.
    pub track_ids: Option<&'a [String]>,
}

/// `_t`: a position as `{beat, seconds, bar_beat}`.
fn position(project: &Project, beat: f64) -> Map<String, Value> {
    let pos = project.time_signatures.position(beat);
    let mut out = Map::new();
    out.insert("beat".to_owned(), Value::from(py_round(beat, 6)));
    out.insert(
        "seconds".to_owned(),
        Value::from(py_round(project.beat_to_seconds(beat), 4)),
    );
    out.insert(
        "bar_beat".to_owned(),
        Value::from(format!("{}:{}", pos.bar, pos.beat)),
    );
    out
}

fn params_json(params: &Params) -> Value {
    Value::Object(
        params
            .iter()
            .map(|(name, value)| (name.clone(), Value::from(value)))
            .collect(),
    )
}

/// `device_json`: an instrument or effect. A device whose plugin is not installed reports
/// `available: false`; with `full`, its stored parameters are reported unresolved.
#[must_use]
pub fn device_json(device: &Device, plugins: &PluginRegistry, full: bool) -> Value {
    let mut data = Map::new();
    data.insert("device_id".to_owned(), Value::from(device.id.as_str()));
    data.insert(
        "plugin_id".to_owned(),
        Value::from(device.plugin_id.as_str()),
    );
    data.insert("name".to_owned(), Value::from(device.name.as_str()));
    data.insert("enabled".to_owned(), Value::from(device.enabled));
    data.insert("preset".to_owned(), Value::from(device.preset.as_deref()));
    if let Some(sidechain) = device.sidechain_id.as_deref().filter(|id| !id.is_empty()) {
        data.insert("sidechain_source".to_owned(), Value::from(sidechain));
    }
    let descriptor = plugins.find(&device.plugin_id);
    if descriptor.is_none() {
        data.insert("available".to_owned(), Value::Bool(false));
    }
    if full {
        let params = match descriptor {
            Some(descriptor) => descriptor.resolve(&device.params),
            None => device.params.clone(),
        };
        data.insert("params".to_owned(), params_json(&params));
        if let Some(descriptor) = descriptor {
            let units: Map<String, Value> = descriptor
                .params
                .iter()
                .filter(|param| !param.unit.is_empty())
                .map(|param| (param.id.clone(), Value::from(param.unit.as_str())))
                .collect();
            data.insert("units".to_owned(), Value::Object(units));
        }
        if !device.state.is_empty() {
            // `isinstance(v, str | int | float | bool)`: lists, objects and null are left out.
            let state: Map<String, Value> = device
                .state
                .iter()
                .filter(|(_, value)| {
                    matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_))
                })
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            data.insert("state".to_owned(), Value::Object(state));
        }
    }
    Value::Object(data)
}

/// `clip_json`: an audio or MIDI clip; `include_notes` adds every note of a MIDI clip.
#[must_use]
pub fn clip_json(project: &Project, clip: &Clip, include_notes: bool) -> Value {
    let mut base = Map::new();
    base.insert("clip_id".to_owned(), Value::from(clip.id()));
    base.insert("name".to_owned(), Value::from(clip.name()));
    let kind = if clip.as_audio().is_some() {
        "audio"
    } else {
        "midi"
    };
    base.insert("type".to_owned(), Value::from(kind));
    base.insert(
        "start".to_owned(),
        Value::Object(position(project, clip.start())),
    );
    base.insert(
        "end".to_owned(),
        Value::Object(position(project, clip.end())),
    );
    base.insert(
        "length_beats".to_owned(),
        Value::from(py_round(clip.length(), 6)),
    );
    base.insert("muted".to_owned(), Value::from(clip.muted()));
    if let Some(audio) = clip.as_audio() {
        base.insert("asset_id".to_owned(), Value::from(audio.asset_id.as_str()));
        base.insert(
            "source_offset_frames".to_owned(),
            Value::from(audio.source_offset),
        );
        base.insert("gain_db".to_owned(), Value::from(audio.gain_db));
        base.insert(
            "fade_in_seconds".to_owned(),
            Value::from(audio.fade_in.seconds),
        );
        base.insert(
            "fade_out_seconds".to_owned(),
            Value::from(audio.fade_out.seconds),
        );
    } else if let Some(midi) = clip.as_midi() {
        base.insert("note_count".to_owned(), Value::from(midi.notes.len()));
        // Non-destructive trims and splits keep hidden notes; this counts what actually plays.
        base.insert(
            "audible_note_count".to_owned(),
            Value::from(midi.playback_notes().len()),
        );
        let lowest = midi.notes.iter().map(|note| note.pitch).min();
        let highest = midi.notes.iter().map(|note| note.pitch).max();
        if let (Some(lowest), Some(highest)) = (lowest, highest) {
            base.insert(
                "pitch_range".to_owned(),
                json!([note_name(lowest), note_name(highest)]),
            );
        }
        if let Some(loop_length) = midi.loop_length {
            base.insert("loop_length_beats".to_owned(), Value::from(loop_length));
        }
        if !midi.controls.is_empty() {
            base.insert(
                "controller_events".to_owned(),
                Value::from(midi.controls.len()),
            );
        }
        if include_notes {
            let notes: Vec<Value> = midi
                .notes
                .iter()
                .map(|note| {
                    let mut entry = Map::new();
                    entry.insert("note_id".to_owned(), Value::from(note.id.as_str()));
                    entry.insert("pitch".to_owned(), Value::from(note.pitch));
                    entry.insert("name".to_owned(), Value::from(note_name(note.pitch)));
                    entry.insert("start".to_owned(), Value::from(note.start));
                    entry.insert("length".to_owned(), Value::from(note.length));
                    entry.insert("velocity".to_owned(), Value::from(note.velocity));
                    if note.channel != 0 {
                        entry.insert("channel".to_owned(), Value::from(note.channel));
                    }
                    Value::Object(entry)
                })
                .collect();
            base.insert("notes".to_owned(), Value::Array(notes));
        }
    }
    Value::Object(base)
}

/// The points of a lane that [`lane_json`] reports.
const MAX_LANE_POINTS: usize = 200;

/// `lane_json`: an automation lane with its first 200 points. The target is `param` for a
/// mixer lane and `<device_id>:<param>` for a device lane.
#[must_use]
pub fn lane_json(lane: &AutomationLane) -> Value {
    let target = match &lane.target.device_id {
        None => lane.target.param.clone(),
        Some(device_id) => format!("{device_id}:{}", lane.target.param),
    };
    let points: Vec<Value> = lane
        .points
        .iter()
        .take(MAX_LANE_POINTS)
        .map(|point| json!({"beat": point.beat, "value": point.value, "curve": point.curve.as_str()}))
        .collect();
    json!({
        "lane_id": lane.id,
        "target": target,
        "enabled": lane.enabled,
        "points": points,
        "point_count": lane.points.len(),
    })
}

/// `track_json`: a track with its mixer, devices, sends, clips (not for the master) and
/// automation.
#[must_use]
pub fn track_json(
    project: &Project,
    track: &Track,
    plugins: &PluginRegistry,
    full: bool,
    include_notes: bool,
) -> Value {
    let mut data = Map::new();
    data.insert("track_id".to_owned(), Value::from(track.id.as_str()));
    data.insert("name".to_owned(), Value::from(track.name.as_str()));
    data.insert("kind".to_owned(), Value::from(track.kind.as_str()));
    data.insert("volume_db".to_owned(), Value::from(track.mixer.volume_db));
    data.insert("pan".to_owned(), Value::from(track.mixer.pan));
    data.insert("mute".to_owned(), Value::from(track.mixer.mute));
    data.insert("solo".to_owned(), Value::from(track.mixer.solo));
    data.insert("output".to_owned(), Value::from(track.output_id.as_deref()));
    if let Some(color) = track.color.as_deref().filter(|color| !color.is_empty()) {
        data.insert("color".to_owned(), Value::from(color));
    }
    if let Some(parent) = track.parent_id.as_deref().filter(|id| !id.is_empty()) {
        data.insert("parent_id".to_owned(), Value::from(parent));
    }
    if let Some(instrument) = &track.instrument {
        data.insert(
            "instrument".to_owned(),
            device_json(instrument, plugins, full),
        );
    }
    if !track.inserts.is_empty() {
        let effects: Vec<Value> = track
            .inserts
            .iter()
            .map(|device| device_json(device, plugins, full))
            .collect();
        data.insert("effects".to_owned(), Value::Array(effects));
    }
    if !track.sends.is_empty() {
        let sends: Vec<Value> = track
            .sends
            .iter()
            .map(|send| {
                json!({
                    "send_id": send.id,
                    "target": send.target_id,
                    "level_db": send.level_db,
                    "pre_fader": send.pre_fader,
                    "enabled": send.enabled,
                })
            })
            .collect();
        data.insert("sends".to_owned(), Value::Array(sends));
    }
    if track.kind != TrackKind::Master {
        let clips: Vec<Value> = track
            .clips
            .iter()
            .map(|clip| clip_json(project, clip, include_notes))
            .collect();
        data.insert("clips".to_owned(), Value::Array(clips));
    }
    if !track.automation.is_empty() {
        let lanes: Vec<Value> = track
            .automation
            .iter()
            .map(|lane| {
                if full {
                    lane_json(lane)
                } else {
                    json!({
                        "lane_id": lane.id,
                        "target": lane.target.to_string(),
                        "points": lane.points.len(),
                    })
                }
            })
            .collect();
        data.insert("automation".to_owned(), Value::Array(lanes));
    }
    if track.input.armed {
        data.insert("armed".to_owned(), Value::Bool(true));
    }
    Value::Object(data)
}

/// `project_state`: tempo, meter, length, sections, markers, tracks, the master, media, the
/// loop and the metadata that is set.
#[must_use]
pub fn project_state(
    project: &Project,
    plugins: &PluginRegistry,
    options: StateOptions<'_>,
) -> Value {
    let StateOptions {
        full,
        include_notes,
        track_ids,
    } = options;
    let tempo: Vec<Value> = project
        .tempo_map
        .points()
        .iter()
        .map(|point| json!({"beat": point.beat, "bpm": point.bpm, "ramp": point.ramp}))
        .collect();
    let signatures: Vec<Value> = project
        .time_signatures
        .points()
        .iter()
        .map(|point| {
            json!({
                "bar": point.bar.saturating_add(1),
                "signature": format!("{}/{}", point.numerator, point.denominator),
                "beat": project.time_signatures.bar_to_beat(bar_number(point.bar)),
            })
        })
        .collect();
    let mut length = position(project, project.end_beat());
    length.insert(
        "explicit".to_owned(),
        Value::Bool(project.settings.length_beats.is_some()),
    );
    let sections: Vec<Value> = project
        .sections
        .iter()
        .map(|section| {
            json!({
                "section_id": section.id,
                "name": section.name,
                "start": position(project, section.start),
                "end": position(project, section.end()),
            })
        })
        .collect();
    let markers: Vec<Value> = project
        .markers
        .iter()
        .map(|marker| {
            let mut entry = Map::new();
            entry.insert("marker_id".to_owned(), Value::from(marker.id.as_str()));
            entry.insert("name".to_owned(), Value::from(marker.name.as_str()));
            entry.extend(position(project, marker.beat));
            Value::Object(entry)
        })
        .collect();
    let tracks: Vec<Value> = project
        .tracks
        .iter()
        .filter(|track| track_ids.is_none_or(|ids| ids.contains(&track.id)))
        .map(|track| track_json(project, track, plugins, full, include_notes))
        .collect();
    json!({
        "project_id": project.id,
        "name": project.name,
        "sample_rate": project.settings.sample_rate,
        "tempo": tempo,
        "time_signatures": signatures,
        "length": length,
        "content_end": position(project, project.content_end()),
        "sections": sections,
        "markers": markers,
        "tracks": tracks,
        "master": track_json(project, &project.master, plugins, full, false),
        "assets": assets_json(project),
        "loop": {
            "enabled": project.transport.loop_enabled,
            "start": project.transport.loop_start,
            "end": project.transport.loop_end,
        },
        "metadata": metadata_json(&project.metadata),
    })
}

/// The media assets, in the project's order, with their duration rounded to 3 decimals.
fn assets_json(project: &Project) -> Vec<Value> {
    project
        .assets
        .iter()
        .map(|asset| {
            json!({
                "asset_id": asset.id,
                "name": asset.name,
                "location": asset.location.as_str(),
                "seconds": py_round(asset.duration_seconds(), 3),
                "sample_rate": asset.sample_rate,
                "channels": asset.channels,
            })
        })
        .collect()
}

/// The metadata fields that are set, in file order.
fn metadata_json(metadata: &ProjectMetadata) -> Map<String, Value> {
    ProjectMetadata::FIELDS
        .iter()
        .filter_map(|field| {
            metadata
                .get(field)
                .filter(|value| !value.is_empty())
                .map(|value| ((*field).to_owned(), Value::from(value)))
        })
        .collect()
}

/// A 0-based bar index as the float the time base takes.
#[expect(
    clippy::cast_precision_loss,
    reason = "bar indices are far below 2^53; Python passes the int to the same float math"
)]
fn bar_number(bar: i64) -> f64 {
    bar as f64
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use sonara_core::plugins::builtin_registry;
    use sonara_project::serializer::{loads, project_from_value};
    use sonara_testkit::Golden;

    use super::{StateOptions, project_state};

    // The key order of each object, from the dict literals of `sonara.agent.state`; optional
    // keys sit where the prototype inserts them. Audio and MIDI clip keys never meet, nor do the
    // summary and full forms of a lane, so one list covers both.
    const PROJECT: &[&str] = &[
        "project_id",
        "name",
        "sample_rate",
        "tempo",
        "time_signatures",
        "length",
        "content_end",
        "sections",
        "markers",
        "tracks",
        "master",
        "assets",
        "loop",
        "metadata",
    ];
    const POSITION: &[&str] = &["beat", "seconds", "bar_beat", "explicit"];
    const TEMPO: &[&str] = &["beat", "bpm", "ramp"];
    const SIGNATURE: &[&str] = &["bar", "signature", "beat"];
    const SECTION: &[&str] = &["section_id", "name", "start", "end"];
    const MARKER: &[&str] = &["marker_id", "name", "beat", "seconds", "bar_beat"];
    const TRACK: &[&str] = &[
        "track_id",
        "name",
        "kind",
        "volume_db",
        "pan",
        "mute",
        "solo",
        "output",
        "color",
        "parent_id",
        "instrument",
        "effects",
        "sends",
        "clips",
        "automation",
        "armed",
    ];
    const DEVICE: &[&str] = &[
        "device_id",
        "plugin_id",
        "name",
        "enabled",
        "preset",
        "sidechain_source",
        "available",
        "params",
        "units",
        "state",
    ];
    const SEND: &[&str] = &["send_id", "target", "level_db", "pre_fader", "enabled"];
    const CLIP: &[&str] = &[
        "clip_id",
        "name",
        "type",
        "start",
        "end",
        "length_beats",
        "muted",
        "asset_id",
        "source_offset_frames",
        "gain_db",
        "fade_in_seconds",
        "fade_out_seconds",
        "note_count",
        "audible_note_count",
        "pitch_range",
        "loop_length_beats",
        "controller_events",
        "notes",
    ];
    const NOTE: &[&str] = &[
        "note_id", "pitch", "name", "start", "length", "velocity", "channel",
    ];
    const LANE: &[&str] = &["lane_id", "target", "enabled", "points", "point_count"];
    const POINT: &[&str] = &["beat", "value", "curve"];
    const ASSET: &[&str] = &[
        "asset_id",
        "name",
        "location",
        "seconds",
        "sample_rate",
        "channels",
    ];
    const LOOP: &[&str] = &["enabled", "start", "end"];
    const METADATA: &[&str] = &[
        "title", "artist", "album", "genre", "composer", "comment", "created", "modified",
    ];

    /// Asserts that `value`'s keys are `order` without the keys it lacks.
    fn check(value: &Value, order: &[&str], what: &str, seen: &mut usize) {
        let map = value
            .as_object()
            .unwrap_or_else(|| panic!("{what} is an object"));
        let actual: Vec<&str> = map.keys().map(String::as_str).collect();
        let expected: Vec<&str> = order
            .iter()
            .copied()
            .filter(|key| map.contains_key(*key))
            .collect();
        assert_eq!(actual, expected, "{what}");
        *seen += 1;
    }

    fn each<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
        value
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
    }

    fn check_track(track: &Value, seen: &mut usize) {
        check(track, TRACK, "track", seen);
        for device in track
            .get("instrument")
            .into_iter()
            .chain(each(track, "effects"))
        {
            check(device, DEVICE, "device", seen);
        }
        for send in each(track, "sends") {
            check(send, SEND, "send", seen);
        }
        for clip in each(track, "clips") {
            check(clip, CLIP, "clip", seen);
            check(&clip["start"], POSITION, "clip start", seen);
            check(&clip["end"], POSITION, "clip end", seen);
            for note in each(clip, "notes") {
                check(note, NOTE, "note", seen);
            }
        }
        for lane in each(track, "automation") {
            check(lane, LANE, "lane", seen);
            for point in each(lane, "points") {
                check(point, POINT, "point", seen);
            }
        }
    }

    #[test]
    fn keys_follow_the_prototypes_order() {
        let golden = Golden::open().expect("golden data is available");
        let plugins = builtin_registry();
        let finals = [
            "coverage",
            "workflows/cinematic_orchestral",
            "workflows/electronic_124bpm",
            "workflows/halloween_thirteenth_hour",
            "workflows/mix_analysis",
            "workflows/sidechain_preview",
        ];
        let mut seen = 0;
        for base in finals {
            let raw = golden
                .bytes(&format!("{base}/final.project.json"))
                .expect("a final project");
            let project = project_from_value(&loads(&raw).expect("JSON")).expect("a project");
            for full in [false, true] {
                let view = project_state(
                    &project,
                    &plugins,
                    StateOptions {
                        full,
                        include_notes: true,
                        track_ids: None,
                    },
                );
                check(&view, PROJECT, "project", &mut seen);
                check(&view["length"], POSITION, "length", &mut seen);
                check(&view["content_end"], POSITION, "content end", &mut seen);
                check(&view["loop"], LOOP, "loop", &mut seen);
                check(&view["metadata"], METADATA, "metadata", &mut seen);
                for point in each(&view, "tempo") {
                    check(point, TEMPO, "tempo", &mut seen);
                }
                for signature in each(&view, "time_signatures") {
                    check(signature, SIGNATURE, "time signature", &mut seen);
                }
                for section in each(&view, "sections") {
                    check(section, SECTION, "section", &mut seen);
                }
                for marker in each(&view, "markers") {
                    check(marker, MARKER, "marker", &mut seen);
                }
                for asset in each(&view, "assets") {
                    check(asset, ASSET, "asset", &mut seen);
                }
                for track in each(&view, "tracks").chain([&view["master"]]) {
                    check_track(track, &mut seen);
                }
            }
        }
        assert!(seen > 10_000, "{seen} objects checked");
    }
}
