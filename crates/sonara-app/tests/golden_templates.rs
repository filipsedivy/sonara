//! A new project built from the `create_project` arguments of each recorded workflow equals the
//! prototype's snapshot after that step, templates included.

use std::fs;

use serde_json::Value;
use sonara_app::projects::{NewProject, build_new_project};
use sonara_core::commands::CommandContext;
use sonara_core::ids::SequentialIds;
use sonara_core::plugins::builtin_registry;
use sonara_project::serializer::{dumps, loads, project_from_value};
use sonara_testkit::Golden;

#[test]
fn new_projects_match_the_recorded_create_project_step() {
    let golden = Golden::open().expect("golden data is available");
    let plugins = builtin_registry();
    let ctx = CommandContext { plugins: &plugins };
    let mut checked = 0;
    for entry in fs::read_dir(golden.root().join("workflows")).expect("workflows") {
        let entry = entry.expect("entry");
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let workflow = entry.file_name().to_string_lossy().into_owned();
        let transcript: Value = golden
            .json(&format!("workflows/{workflow}/transcript.json"))
            .expect("transcript");
        let first = &transcript["steps"][0];
        assert_eq!(first["tool"], "create_project");
        let hash = first["project"].as_str().expect("snapshot hash");
        let raw = golden
            .bytes_gz(&format!("workflows/{workflow}/snapshots/{hash}.json.gz"))
            .expect("snapshot");
        let recorded = project_from_value(&loads(&raw).expect("JSON")).expect("valid");

        // The workflow file names the template; everything else is read back from the snapshot.
        let spec_path = golden
            .root()
            .join("..")
            .join(transcript["workflow"].as_str().expect("workflow path"));
        let spec: Value = serde_json::from_slice(&fs::read(spec_path).expect("workflow file"))
            .expect("workflow JSON");
        let steps = spec.get("steps").unwrap_or(&spec);
        let template = steps[0]["args"]["template"].as_str().unwrap_or("empty");
        let signature = recorded.time_signatures.points()[0];
        let options = NewProject {
            name: recorded.name.clone(),
            sample_rate: i64::from(recorded.settings.sample_rate),
            bpm: recorded.tempo_map.bpm_at(0.0),
            time_signature: (signature.numerator, signature.denominator),
            template: template.to_owned(),
            length_beats: recorded.settings.length_beats,
        };
        // The prototype's IDs count up from the project's own number.
        let number: u64 = recorded
            .id
            .trim_start_matches("prj_")
            .parse()
            .expect("number");
        let mut ids = SequentialIds::new(number - 1);
        let built = build_new_project(&options, &recorded.metadata.created, &ctx, &mut ids)
            .expect("valid project");
        assert!(dumps(&built) == raw, "{workflow}: the new project differs");
        checked += 1;
    }
    assert_eq!(checked, 5);
}
