//! The Standard MIDI File export and import reproduce the prototype's (golden
//! `workflows/*/final.mid` and `final.import.json`).
//!
//! `final.mid` is what the prototype exported from `final.project.json`, and `final.import.json`
//! is what it read back from that file inside `deterministic(seed=2000)`
//! (`scripts/golden/area_midi.py`, `record_smf`). The export is compared byte for byte.
//! `halloween_thirteenth_hour` has no `final.mid`: the prototype crashed there on non-Latin-1
//! names, and Rust must export, with UTF-8 names.

use serde_json::Value;
use sonara_core::ids::SequentialIds;
use sonara_core::model::project::Project;
use sonara_midi::smf::{ImportedMidi, ImportedTrack, encode_smf, parse_smf};
use sonara_project::serializer::{loads, project_from_value};
use sonara_testkit::Golden;

fn workflows(golden: &Golden) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(golden.root().join("workflows"))
        .expect("workflows directory")
        .map(|entry| {
            entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

fn project(golden: &Golden, name: &str) -> Project {
    let raw = golden
        .bytes(&format!("workflows/{name}/final.project.json"))
        .expect("final project");
    let data = loads(&raw).expect("project JSON");
    project_from_value(&data).expect("valid project")
}

fn transcript(golden: &Golden, name: &str) -> Value {
    golden
        .json(&format!("workflows/{name}/transcript.json"))
        .expect("transcript")
}

/// Compares what the importer read with `final.import.json`. Floats match exactly: the importer
/// does the same arithmetic as the prototype on the same integers.
fn expect_import(expected: &Value, actual: &ImportedMidi, what: &str) {
    let points = actual.tempo_map.points();
    let want = expected["tempo"].as_array().expect("tempo");
    assert_eq!(points.len(), want.len(), "{what}: tempo points");
    for (got, want) in points.iter().zip(want) {
        assert_eq!(
            got.beat,
            want["beat"].as_f64().expect("beat"),
            "{what}: tempo beat"
        );
        assert_eq!(
            got.bpm,
            want["bpm"].as_f64().expect("bpm"),
            "{what}: tempo bpm"
        );
        assert_eq!(
            got.ramp,
            want["ramp"].as_bool().expect("ramp"),
            "{what}: tempo ramp"
        );
    }

    let signatures = actual.time_signatures.points();
    let want = expected["time_signatures"].as_array().expect("signatures");
    assert_eq!(signatures.len(), want.len(), "{what}: time signatures");
    for (got, want) in signatures.iter().zip(want) {
        assert_eq!(Some(got.bar), want["bar"].as_i64(), "{what}: bar");
        assert_eq!(
            Some(u64::from(got.numerator)),
            want["numerator"].as_u64(),
            "{what}: numerator"
        );
        assert_eq!(
            Some(u64::from(got.denominator)),
            want["denominator"].as_u64(),
            "{what}: denominator"
        );
    }

    let want = expected["markers"].as_array().expect("markers");
    assert_eq!(actual.markers.len(), want.len(), "{what}: markers");
    for ((beat, text), want) in actual.markers.iter().zip(want) {
        assert_eq!(
            *beat,
            want[0].as_f64().expect("marker beat"),
            "{what}: marker beat"
        );
        assert_eq!(Some(text.as_str()), want[1].as_str(), "{what}: marker text");
    }

    let warnings: Vec<&str> = expected["warnings"]
        .as_array()
        .expect("warnings")
        .iter()
        .map(|w| w.as_str().expect("warning"))
        .collect();
    assert_eq!(actual.warnings, warnings, "{what}: warnings");

    let want = expected["tracks"].as_array().expect("tracks");
    assert_eq!(actual.tracks.len(), want.len(), "{what}: tracks");
    for (track, want) in actual.tracks.iter().zip(want) {
        expect_track(track, want, what);
    }
}

/// Compares one imported track with the recorded one.
fn expect_track(track: &ImportedTrack, want: &Value, what: &str) {
    let at = format!("{what} / {}", track.name);
    assert_eq!(
        Some(track.name.as_str()),
        want["name"].as_str(),
        "{at}: name"
    );
    assert_eq!(
        Some(u64::from(track.channel)),
        want["channel"].as_u64(),
        "{at}: channel"
    );
    assert_eq!(
        track.program.map(u64::from),
        want["program"].as_u64(),
        "{at}: program"
    );
    // None of the recorded files has controllers; their `repr` strings are not ported.
    assert!(
        want["controls"].as_array().expect("controls").is_empty(),
        "{at}"
    );
    assert!(track.controls.is_empty(), "{at}: controls");
    let notes = want["notes"].as_array().expect("notes");
    assert_eq!(track.notes.len(), notes.len(), "{at}: note count");
    for (index, (got, want)) in track.notes.iter().zip(notes).enumerate() {
        let note = format!("{at} #{index}");
        assert_eq!(Some(got.id.as_str()), want["id"].as_str(), "{note}: id");
        assert_eq!(
            Some(u64::from(got.pitch)),
            want["pitch"].as_u64(),
            "{note}: pitch"
        );
        assert_eq!(
            Some(u64::from(got.velocity)),
            want["velocity"].as_u64(),
            "{note}: velocity"
        );
        assert_eq!(
            Some(u64::from(got.channel)),
            want["channel"].as_u64(),
            "{note}: channel"
        );
        assert_eq!(
            got.start,
            want["start"].as_f64().expect("start"),
            "{note}: start"
        );
        assert_eq!(
            got.length,
            want["length"].as_f64().expect("length"),
            "{note}: length"
        );
    }
}

#[test]
fn export_and_import_reproduce_the_prototype() {
    let golden = Golden::open().expect("golden data");
    let mut exported = 0;
    for name in workflows(&golden) {
        let want_bytes = golden.bytes(&format!("workflows/{name}/final.mid"));
        let Ok(want_bytes) = want_bytes else {
            continue;
        };
        let project = project(&golden, &name);
        let encoded = encode_smf(&project, None).expect("export");

        // The summary the prototype recorded.
        let summary = &transcript(&golden, &name)["smf_export"];
        assert_eq!(
            Some(encoded.tracks as u64),
            summary["tracks"].as_u64(),
            "{name}: tracks"
        );
        assert_eq!(
            Some(encoded.notes as u64),
            summary["notes"].as_u64(),
            "{name}: notes"
        );
        assert_eq!(
            encoded.max_rounding_error_beats,
            summary["max_rounding_error_beats"]
                .as_f64()
                .expect("rounding error"),
            "{name}: rounding error"
        );

        // The bytes.
        assert_eq!(encoded.bytes.len(), want_bytes.len(), "{name}: file size");
        let first_difference = encoded
            .bytes
            .iter()
            .zip(&want_bytes)
            .position(|(got, want)| got != want);
        assert_eq!(first_difference, None, "{name}: first differing byte");

        // What the prototype reads back from its file.
        let expected: Value = golden
            .json(&format!("workflows/{name}/final.import.json"))
            .expect("final.import.json");
        let mut ids = SequentialIds::new(2000);
        let imported = parse_smf(&mut ids, &want_bytes, "final.mid").expect("import");
        expect_import(&expected, &imported, &name);

        // Our own export reads back the same.
        let mut ids = SequentialIds::new(2000);
        let again = parse_smf(&mut ids, &encoded.bytes, "final.mid").expect("import");
        assert_eq!(again, imported, "{name}: re-import");
        exported += 1;
    }
    assert_eq!(exported, 4, "four workflows have a final.mid");
}

#[test]
fn export_with_czech_names_succeeds_where_the_prototype_crashed() {
    let golden = Golden::open().expect("golden data");
    let name = "halloween_thirteenth_hour";
    assert!(
        golden
            .bytes(&format!("workflows/{name}/final.mid"))
            .is_err(),
        "the prototype wrote no file"
    );
    let summary = transcript(&golden, name);
    assert_eq!(summary["smf_export"]["prototype_defect"], "D1");
    assert!(
        summary["smf_export"]["error"]
            .as_str()
            .is_some_and(|e| e.starts_with("UnicodeEncodeError: 'latin-1' codec can't encode")),
    );

    let project = project(&golden, name);
    let encoded = encode_smf(&project, None).expect("export succeeds");
    assert!(encoded.notes > 0);
    let mut ids = SequentialIds::new(2000);
    let imported = parse_smf(&mut ids, &encoded.bytes, "final.mid").expect("import");
    // Every track name that has notes survives the round trip as UTF-8, among them names that
    // are not Latin-1 (drum tracks get the suffix ` (Drums)` when their name does not say drum).
    let names: Vec<&str> = imported.tracks.iter().map(|t| t.name.as_str()).collect();
    let with_notes: Vec<&str> = project
        .tracks
        .iter()
        .filter(|t| t.midi_clips().any(|c| !c.notes.is_empty()))
        .map(|t| t.name.as_str())
        .collect();
    assert_ne!(with_notes.len(), 0);
    for track_name in &with_notes {
        let drums = format!("{track_name} (Drums)");
        assert!(
            names.contains(track_name) || names.contains(&drums.as_str()),
            "{track_name:?} not in {names:?}"
        );
    }
    assert!(
        with_notes
            .iter()
            .any(|n| n.chars().any(|c| u32::from(c) > 0xFF)),
        "the project has a track name outside Latin-1"
    );
}
