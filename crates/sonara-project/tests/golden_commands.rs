//! Replays every command the prototype's workflows executed (`golden/workflows/*/commands.json`)
//! and compares labels, change sets, results and project hashes, then walks the undo and redo
//! history ("Commands").

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use sonara_core::commands::automation::SetAutomation;
use sonara_core::commands::clip::{CreateMidiClip, ReplaceNotes};
use sonara_core::commands::device::{AddDevice, SetDeviceParams, SetInstrument};
use sonara_core::commands::project::{
    AddMarker, AddSection, SetMetadata, SetProjectLength, SetTempo, SetTimeSignature,
};
use sonara_core::commands::track::{AddSend, CreateTrack, SetTrackMixer};
use sonara_core::commands::{Command, Transaction};
use sonara_core::midi::MidiNote;
use sonara_core::model::JsonMap;
use sonara_core::model::automation::{AutomationPoint, CurveKind};
use sonara_core::model::project::Project;
use sonara_core::model::track::TrackKind;
use sonara_core::plugins::builtin_registry;
use sonara_core::session::{ProjectLocation, ProjectSession};
use sonara_project::serializer::{dumps, loads, project_from_value};
use sonara_testkit::{Golden, sha256_hex};

// -- reading recorded command fields --------------------------------------------------------

fn field<'a>(fields: &'a Value, key: &str) -> &'a Value {
    fields
        .get(key)
        .unwrap_or_else(|| panic!("recorded field {key} is missing in {fields}"))
}

fn string(fields: &Value, key: &str) -> String {
    field(fields, key)
        .as_str()
        .unwrap_or_else(|| panic!("{key} is a string"))
        .to_owned()
}

fn opt_string(fields: &Value, key: &str) -> Option<String> {
    field(fields, key).as_str().map(str::to_owned)
}

fn number(fields: &Value, key: &str) -> f64 {
    field(fields, key)
        .as_f64()
        .unwrap_or_else(|| panic!("{key} is a number"))
}

fn opt_number(fields: &Value, key: &str) -> Option<f64> {
    field(fields, key).as_f64()
}

fn integer(fields: &Value, key: &str) -> i64 {
    field(fields, key)
        .as_i64()
        .unwrap_or_else(|| panic!("{key} is an integer"))
}

fn opt_integer(fields: &Value, key: &str) -> Option<i64> {
    field(fields, key).as_i64()
}

fn flag(fields: &Value, key: &str) -> bool {
    field(fields, key)
        .as_bool()
        .unwrap_or_else(|| panic!("{key} is a boolean"))
}

fn opt_flag(fields: &Value, key: &str) -> Option<bool> {
    field(fields, key).as_bool()
}

fn map(fields: &Value, key: &str) -> JsonMap {
    field(fields, key)
        .as_object()
        .cloned()
        .unwrap_or_else(|| panic!("{key} is an object"))
}

fn notes(fields: &Value) -> Vec<MidiNote> {
    field(fields, "notes")
        .as_array()
        .expect("notes is a list")
        .iter()
        .map(|n| {
            MidiNote::new(
                string(n, "id"),
                integer(n, "pitch"),
                number(n, "start"),
                number(n, "length"),
                integer(n, "velocity"),
                integer(n, "channel"),
            )
            .expect("recorded notes are valid")
        })
        .collect()
}

fn points(fields: &Value) -> Vec<AutomationPoint> {
    field(fields, "points")
        .as_array()
        .expect("points is a list")
        .iter()
        .map(|p| {
            let curve = CurveKind::parse(&string(p, "curve")).expect("known curve");
            AutomationPoint::new(number(p, "beat"), number(p, "value"), curve)
                .expect("recorded points are valid")
        })
        .collect()
}

/// The command a recorded entry describes.
fn command(name: &str, f: &Value) -> Box<dyn Command> {
    match name {
        "AddDevice" => Box::new(AddDevice {
            track_id: string(f, "track_id"),
            plugin_id: string(f, "plugin_id"),
            index: opt_integer(f, "index"),
            params: map(f, "params"),
            preset: opt_string(f, "preset"),
            sidechain_id: opt_string(f, "sidechain_id"),
            name: opt_string(f, "name"),
            device_id: string(f, "device_id"),
        }),
        "AddMarker" => Box::new(AddMarker {
            beat: number(f, "beat"),
            name: string(f, "name"),
            color: opt_string(f, "color"),
            marker_id: string(f, "marker_id"),
        }),
        "AddSection" => Box::new(AddSection {
            name: string(f, "name"),
            start: number(f, "start"),
            length: number(f, "length"),
            color: opt_string(f, "color"),
            section_id: string(f, "section_id"),
        }),
        "CreateMidiClip" => Box::new(CreateMidiClip {
            track_id: string(f, "track_id"),
            start: number(f, "start"),
            length: number(f, "length"),
            name: string(f, "name"),
            notes: notes(f),
            loop_length: opt_number(f, "loop_length"),
            color: opt_string(f, "color"),
            clip_id: string(f, "clip_id"),
        }),
        "CreateTrack" => Box::new(CreateTrack {
            name: string(f, "name"),
            kind: TrackKind::parse(&string(f, "kind")).expect("known track kind"),
            index: opt_integer(f, "index"),
            color: opt_string(f, "color"),
            output_id: string(f, "output_id"),
            instrument_plugin: opt_string(f, "instrument_plugin"),
            instrument_preset: opt_string(f, "instrument_preset"),
            parent_id: opt_string(f, "parent_id"),
            track_id: string(f, "track_id"),
            instrument_id: string(f, "instrument_id"),
        }),
        "SetAutomation" => Box::new(SetAutomation {
            track_id: string(f, "track_id"),
            device_id: opt_string(f, "device_id"),
            param: string(f, "param"),
            points: points(f),
            start: opt_number(f, "start"),
            end: opt_number(f, "end"),
            lane_id: string(f, "lane_id"),
        }),
        "SetInstrument" => Box::new(SetInstrument {
            track_id: string(f, "track_id"),
            plugin_id: string(f, "plugin_id"),
            preset: opt_string(f, "preset"),
            params: map(f, "params"),
            device_id: string(f, "device_id"),
        }),
        "SetTrackMixer" => Box::new(SetTrackMixer {
            track_id: string(f, "track_id"),
            volume_db: opt_number(f, "volume_db"),
            pan: opt_number(f, "pan"),
            mute: opt_flag(f, "mute"),
            solo: opt_flag(f, "solo"),
            solo_safe: opt_flag(f, "solo_safe"),
        }),
        "Transaction" => {
            let steps = field(f, "commands").as_array().expect("commands is a list");
            // Newer recordings name the nested commands in `types`; older ones are recognized
            // by their field names.
            let types: Vec<Option<&str>> = match f.get("types").and_then(Value::as_array) {
                Some(types) => types.iter().map(Value::as_str).collect(),
                None => vec![None; steps.len()],
            };
            Box::new(Transaction {
                commands: steps
                    .iter()
                    .zip(types)
                    .map(|(step, kind)| kind.map_or_else(|| nested(step), |k| command(k, step)))
                    .collect(),
                name: string(f, "name"),
            })
        }
        other => project_command(other, f),
    }
}

/// A recorded project-level command.
fn project_command(name: &str, f: &Value) -> Box<dyn Command> {
    match name {
        "SetMetadata" => Box::new(SetMetadata {
            values: map(f, "values")
                .into_iter()
                .map(|(k, v)| (k, v.as_str().expect("metadata is text").to_owned()))
                .collect(),
        }),
        "SetProjectLength" => Box::new(SetProjectLength {
            length_beats: opt_number(f, "length_beats"),
        }),
        "SetTempo" => Box::new(SetTempo {
            bpm: number(f, "bpm"),
            beat: number(f, "beat"),
            ramp: flag(f, "ramp"),
            clear_changes: flag(f, "clear_changes"),
        }),
        "SetTimeSignature" => Box::new(SetTimeSignature {
            numerator: integer(f, "numerator"),
            denominator: integer(f, "denominator"),
            bar: integer(f, "bar"),
        }),
        "SetDeviceParams" => Box::new(SetDeviceParams {
            device_id: string(f, "device_id"),
            values: map(f, "values"),
        }),
        "AddSend" => Box::new(AddSend {
            track_id: string(f, "track_id"),
            target_id: string(f, "target_id"),
            level_db: number(f, "level_db"),
            pre_fader: flag(f, "pre_fader"),
            send_id: string(f, "send_id"),
        }),
        "ReplaceNotes" => Box::new(ReplaceNotes {
            clip_id: string(f, "clip_id"),
            notes: notes(f),
            name: string(f, "name"),
        }),
        other => panic!("the replay does not know command {other}"),
    }
}

/// A command nested in a transaction. The recorder keeps only the fields of nested commands,
/// so the command is recognized by its field names.
fn nested(step: &Value) -> Box<dyn Command> {
    let has = |key: &str| step.get(key).is_some();
    let kind = if has("device_id") && has("values") {
        "SetDeviceParams"
    } else if has("send_id") && has("target_id") {
        "AddSend"
    } else if has("clip_id") && has("notes") && has("name") {
        "ReplaceNotes"
    } else {
        panic!("unexpected nested command {step}")
    };
    command(kind, step)
}

// -- replay ---------------------------------------------------------------------------------

fn snapshot(golden: &Golden, workflow: &str, hash: &str) -> Project {
    let raw = golden
        .bytes_gz(&format!("workflows/{workflow}/snapshots/{hash}.json.gz"))
        .expect("recorded snapshot");
    project_from_value(&loads(&raw).expect("JSON object")).expect("valid snapshot")
}

fn project_hash(session: &ProjectSession) -> String {
    sha256_hex(&dumps(session.project().expect("open project")))
}

fn workflows(golden: &Golden) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(golden.root().join("workflows"))
        .expect("golden workflows exist")
        .map(|e| e.expect("readable entry"))
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Undo (or redo) until the history is exhausted: labels and project hashes after each move.
fn walk(session: &mut ProjectSession, undo: bool) -> (Vec<String>, Vec<String>) {
    let mut labels = Vec::new();
    let mut hashes = Vec::new();
    loop {
        let moved = if undo {
            session.undo("golden")
        } else {
            session.redo("golden")
        };
        let Ok(result) = moved else { break };
        labels.push(result.label);
        hashes.push(project_hash(session));
    }
    (labels, hashes)
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("a list")
        .iter()
        .map(|v| v.as_str().expect("a string").to_owned())
        .collect()
}

#[test]
fn workflow_commands_replay_like_the_prototype() {
    let golden = Golden::open().expect("golden data is available");
    let plugins = Arc::new(builtin_registry());
    let mut replayed = 0;
    for workflow in workflows(&golden) {
        let base = format!("workflows/{workflow}");
        let transcript: Value = golden
            .json(&format!("{base}/transcript.json"))
            .expect("transcript");
        let recorded: Value = golden
            .json(&format!("{base}/commands.json"))
            .expect("commands");
        let first = &transcript["steps"][0];
        assert_eq!(first["tool"], "create_project", "{workflow}");
        let start = snapshot(&golden, &workflow, first["project"].as_str().expect("hash"));
        let mut session = ProjectSession::with_options(plugins.clone(), 500, Box::new(|| 0.0));
        let location = ProjectLocation {
            path: None,
            workspace: PathBuf::from("/golden"),
            packaged: false,
        };
        session.load(start, location, true);

        for (i, entry) in recorded["commands"]
            .as_array()
            .expect("command list")
            .iter()
            .enumerate()
        {
            let name = entry["command"].as_str().expect("command name");
            let where_ = format!("{workflow} #{i} {name}");
            let cmd = command(name, &entry["fields"]);
            assert_eq!(cmd.name(), name, "{where_}");
            let dry_run = entry["dry_run"].as_bool().expect("dry_run flag");
            let result = session
                .execute(cmd.as_ref(), dry_run, "golden")
                .unwrap_or_else(|err| panic!("{where_}: {err}"));
            assert_eq!(
                Value::from(result.committed),
                entry["committed"],
                "{where_}"
            );
            assert_eq!(
                result.label,
                entry["label"].as_str().expect("label"),
                "{where_}"
            );
            assert_eq!(
                result.changes.to_json(),
                entry["changes"],
                "{where_}: changes"
            );
            assert_eq!(
                Value::Object(result.result.clone()),
                entry["result"],
                "{where_}: result"
            );
            if let Some(hash) = entry["project"].as_str() {
                assert_eq!(project_hash(&session), hash, "{where_}: project");
            }
            replayed += 1;
        }
        assert_eq!(
            project_hash(&session),
            transcript["final_project"].as_str().expect("final hash"),
            "{workflow}: final project"
        );

        let (labels, hashes) = walk(&mut session, true);
        assert_eq!(
            labels,
            strings(&transcript["undo"]["labels"]),
            "{workflow}: undo labels"
        );
        assert_eq!(
            hashes,
            strings(&transcript["undo"]["hashes"]),
            "{workflow}: undo hashes"
        );
        let (labels, hashes) = walk(&mut session, false);
        assert_eq!(
            labels,
            strings(&transcript["redo"]["labels"]),
            "{workflow}: redo labels"
        );
        assert_eq!(
            hashes,
            strings(&transcript["redo"]["hashes"]),
            "{workflow}: redo hashes"
        );
    }
    assert!(replayed > 200, "replayed {replayed} commands");
}
