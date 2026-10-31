//! Argument validation against the prototype (CTR-01,
//! GLD-01):
//!
//! - the validation errors of the golden coverage transcript, and the arguments of every golden
//!   workflow, which the prototype accepted;
//! - the generated fixtures `tests/fixtures/validation.json`
//!   (`scripts/golden/agent_validation_fixtures.py`), with the prototype's normalized values or
//!   errors for every tool;
//! - the typed argument structs: every normalized value deserializes into its struct and
//!   serializes back unchanged, so the table and the structs cannot drift.

use std::fmt::Write as _;
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use sonara_agent::contract::args::{self, ToolArgs};
use sonara_agent::contract::versions::recorded_arguments;
use sonara_agent::contract::{Model, TOOLS, find, parse, validate};
use sonara_core::error::{DomainError, ErrorCode};
use sonara_testkit::Golden;

type RoundTrip = fn(&Value) -> Result<Value, String>;

/// Deserializes `value` into `T` and serializes it back.
fn round_trip<T: ToolArgs + Serialize + DeserializeOwned>(value: &Value) -> Result<Value, String> {
    let typed: T = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    serde_json::to_value(&typed).map_err(|e| e.to_string())
}

macro_rules! structs {
    ($($name:ident),* $(,)?) => {
        [$((<args::$name as ToolArgs>::MODEL, round_trip::<args::$name> as RoundTrip)),*]
    };
}

/// Every typed argument struct with its model.
fn structs() -> Vec<(&'static Model, RoundTrip)> {
    structs![
        CreateProject,
        OpenProject,
        SaveProject,
        GetProjectState,
        Empty,
        SetTempo,
        SetTimeSignature,
        SetProjectLength,
        AddSection,
        AddMarker,
        SetMetadata,
        SetLoop,
        CreateTrack,
        RenameTrack,
        DeleteTrack,
        SetTrackVolume,
        SetTrackPan,
        SetTrackState,
        SetInstrument,
        ConfigureRouting,
        CreateMidiClip,
        AddMidiNotes,
        EditMidiNotes,
        GenerateMidi,
        ImportAudio,
        ImportMidi,
        MoveClip,
        EditClip,
        SplitClip,
        DuplicateClip,
        DeleteClips,
        ListPlugins,
        AddEffect,
        SetEffectParameter,
        RemoveEffect,
        LoadImpulseResponse,
        CreateAutomation,
        RenderPreview,
        PlayPreview,
        ExportAudio,
        ExportStems,
        ExportMidi,
        AnalyzeAudio,
        GetAudioAnalysis,
        SetMidiInput,
        StartRecording,
        OperationId,
        Relink,
        WriteMidiControllers,
    ]
    .to_vec()
}

fn struct_of(model: &Model) -> RoundTrip {
    let matching: Vec<RoundTrip> = structs()
        .into_iter()
        .filter(|(m, _)| std::ptr::eq(*m, model))
        .map(|(_, f)| f)
        .collect();
    assert_eq!(matching.len(), 1, "{} needs exactly one struct", model.name);
    matching[0]
}

/// Checks that `values` round-trips through the tool's struct unchanged.
fn check_struct(tool: &str, values: &Map<String, Value>) -> Result<(), String> {
    let spec = find(tool).ok_or_else(|| format!("unknown tool {tool}"))?;
    let value = Value::Object(values.clone());
    let back = struct_of(spec.input)(&value)?;
    if back == value {
        Ok(())
    } else {
        Err(format!("round trip changed {value} into {back}"))
    }
}

/// The `details.errors` of a validation failure.
fn errors_of(tool: &str, error: &DomainError) -> Value {
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert_eq!(error.message, format!("invalid arguments for {tool}"));
    error.details["errors"].clone()
}

#[test]
fn every_tool_has_one_struct() {
    for tool in &TOOLS {
        struct_of(tool.input);
    }
    for (model, _) in structs() {
        assert!(
            TOOLS.iter().any(|tool| std::ptr::eq(tool.input, model)),
            "{} is not a tool input",
            model.name
        );
    }
}

#[test]
fn coverage_transcript_validation_errors_match() {
    let transcript: Value = Golden::open()
        .and_then(|golden| golden.json("coverage/transcript.json"))
        .unwrap();
    let mut checked = 0;
    for step in transcript["steps"].as_array().unwrap() {
        let Some(tool) = step["tool"].as_str() else {
            continue; // a harness step
        };
        let spec = find(tool).unwrap();
        // The registry maps a missing or empty argument value to `{}` (`arguments or {}`).
        let args = &step["args"];
        let recorded = &step["result"]["error"];
        match validate(spec, args) {
            Ok(values) => {
                assert!(
                    recorded["details"].get("errors").is_none(),
                    "{tool} {args}: the prototype rejected it with {recorded}"
                );
                check_struct(tool, &values).unwrap();
            }
            Err(error) => {
                assert_eq!(
                    &errors_of(tool, &error),
                    &recorded["details"]["errors"],
                    "{tool} {args}"
                );
                assert_eq!(recorded["code"], "INVALID_ARGUMENT");
                checked += 1;
            }
        }
    }
    // The coverage batch's argument-validation steps (scripts/golden/area_coverage.py).
    assert_eq!(checked, 10);
}

#[test]
fn every_workflow_validates() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut steps = 0;
    for name in [
        "cinematic_orchestral",
        "electronic_124bpm",
        "halloween_thirteenth_hour",
        "mix_analysis",
        "sidechain_preview",
    ] {
        let text = std::fs::read_to_string(examples.join(format!("{name}.json"))).unwrap();
        let workflow: Value = serde_json::from_str(&text).unwrap();
        for step in workflow["steps"].as_array().unwrap() {
            let tool = step["tool"].as_str().unwrap();
            let args = step.get("args").cloned().unwrap_or_else(|| json!({}));
            let values = validate(find(tool).unwrap(), &args)
                .unwrap_or_else(|error| panic!("{name}: {tool} {args}: {error:?}"));
            check_struct(tool, &values).unwrap_or_else(|e| panic!("{name}: {tool}: {e}"));
            steps += 1;
        }
    }
    assert_eq!(steps, 277);
}

fn repeated(value: &Value, count: usize) -> Value {
    let first = value.as_array().and_then(|items| items.first()).cloned();
    Value::Array(vec![first.unwrap_or(Value::Null); count])
}

#[test]
fn fixtures_match_the_prototype() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/validation.json");
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    let mut failures = Vec::new();
    let (mut accepted, mut rejected) = (0, 0);
    for (index, case) in cases.iter().enumerate() {
        let tool = case["tool"].as_str().unwrap();
        let mut input = case["input"].clone();
        let mut ok = case.get("ok").and_then(Value::as_object).cloned();
        if let Some(repeat) = case.get("repeat") {
            let field = repeat["field"].as_str().unwrap();
            let count = usize::try_from(repeat["count"].as_u64().unwrap()).unwrap();
            input[field] = repeated(&input[field], count);
            if let Some(changed) = ok.as_mut()
                && let Some(list) = changed.get(field).cloned()
            {
                changed.insert(field.to_owned(), repeated(&list, count));
            }
        }
        let spec = find(tool).unwrap();
        let result = validate(spec, &input);
        let problem = match (ok, result) {
            (Some(changed), Ok(values)) => {
                accepted += 1;
                // The prototype normalized the recorded version's arguments, without the
                // fields a later version adds (CTR-01); the struct takes them all.
                if let Some(problem) = check_struct(tool, &values).err() {
                    failures.push((index, case, problem));
                    continue;
                }
                let values = recorded_arguments(spec.input, &values);
                let mut expected = fixture["tools"][tool]["base_output"]
                    .as_object()
                    .unwrap()
                    .clone();
                expected.extend(changed);
                if values == expected {
                    None
                } else {
                    Some(format!(
                        "expected {}, got {}",
                        Value::Object(expected),
                        Value::Object(values)
                    ))
                }
            }
            (None, Err(error)) => {
                rejected += 1;
                let errors = errors_of(tool, &error);
                (errors != case["errors"])
                    .then(|| format!("expected errors {}, got {errors}", case["errors"]))
            }
            (Some(_), Err(error)) => Some(format!("expected ok, got {}", error.details["errors"])),
            (None, Ok(values)) => Some(format!(
                "expected errors {}, got ok {}",
                case["errors"],
                Value::Object(values)
            )),
        };
        if let Some(problem) = problem {
            failures.push((index, case, problem));
        }
    }
    let mut report = String::new();
    for (index, case, problem) in failures.iter().take(25) {
        let note = case["note"].as_str().unwrap_or_default();
        let _ = writeln!(
            report,
            "#{index} {} ({note}) {}: {problem}",
            case["tool"], case["input"]
        );
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{report}",
        failures.len(),
        cases.len()
    );
    assert!(
        accepted > 4000 && rejected > 5000,
        "{accepted} ok, {rejected} errors"
    );
}

#[test]
fn parse_returns_the_typed_struct() {
    let tool = find("add_midi_notes").unwrap();
    let parsed: args::AddMidiNotes = parse(
        tool,
        &json!({"clip_id": " clp_1 ", "notes": [{"pitch": "C4", "start": 0, "length": "1.5"}]}),
    )
    .unwrap();
    assert_eq!(parsed.clip_id, "clp_1");
    assert_eq!(parsed.notes[0].pitch, args::Pitch::Name("C4".to_owned()));
    assert!((parsed.notes[0].length - 1.5).abs() < f64::EPSILON);
    assert_eq!(parsed.notes[0].velocity, 100);
    assert!(!parsed.dry_run);
}

#[test]
fn parse_reports_invalid_arguments_and_mismatched_structs() {
    let tool = find("add_midi_notes").unwrap();
    let error =
        parse::<args::AddMidiNotes>(tool, &json!({"clip_id": "c", "notes": []})).unwrap_err();
    assert_eq!(
        errors_of("add_midi_notes", &error),
        json!([{
            "field": "notes",
            "message": "List should have at least 1 item after validation, not 0",
        }])
    );
    let error = parse::<args::Empty>(tool, &json!({})).unwrap_err();
    assert_eq!(error.code, ErrorCode::Internal);
}
