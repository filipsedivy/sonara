//! The agent context's pure parts against the prototype (CTR-01, GLD-01): the file sandbox, the confirmations, the audit log and the argument
//! conversion, checked against
//!
//! - the generated fixtures `tests/fixtures/context.json`
//!   (`scripts/golden/agent_context_fixtures.py`), with the prototype's results;
//! - the `PERMISSION_DENIED` and `CONFIRMATION_REQUIRED` steps of the golden coverage
//!   transcript, including its refused token.

use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use serde_json::{Value, json};
use sonara_core::error::DomainError;
use sonara_testkit::Golden;

static FIXTURE: LazyLock<Value> = LazyLock::new(|| {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/context.json");
    let text = fs::read_to_string(path).expect("the fixture is readable");
    serde_json::from_str(&text).expect("the fixture is JSON")
});

static TRANSCRIPT: LazyLock<Value> = LazyLock::new(|| {
    Golden::open()
        .and_then(|golden| golden.json("coverage/transcript.json"))
        .expect("the golden coverage transcript is available")
});

/// The prototype's `SonaraError.to_dict()`.
fn error_json(error: &DomainError) -> Value {
    let mut out = json!({"code": error.code.as_str(), "message": error.message});
    if !error.details.is_empty() {
        out["details"] = Value::Object(error.details.clone());
    }
    out
}

/// The outcome a case expects: the Rust port's documented result (`rust`) where the prototype
/// crashes or diverges, else the prototype's (`ok` or `error`).
fn expected(case: &Value) -> Value {
    let source = case.get("rust").unwrap_or(case);
    match source.get("ok") {
        Some(ok) => json!({"ok": ok}),
        None => json!({"error": source["error"]}),
    }
}

fn outcome(result: Result<Value, DomainError>) -> Value {
    match result {
        Ok(value) => json!({"ok": value}),
        Err(error) => json!({"error": error_json(&error)}),
    }
}

fn items(value: &Value) -> &Vec<Value> {
    value.as_array().expect("a fixture array")
}

fn text(value: &Value) -> &str {
    value.as_str().expect("a fixture string")
}

#[cfg(unix)]
mod sandbox {
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};

    use serde_json::{Value, json};
    use sonara_agent::permissions::{Access, PathPolicy};
    use sonara_core::error::ErrorCode;
    use tempfile::TempDir;

    use super::{FIXTURE, TRANSCRIPT, error_json, expected, items, outcome, text};

    /// The fixture's temporary tree, rebuilt under a resolved temporary directory (`<D>`).
    struct Tree {
        _dir: TempDir,
        root: String,
    }

    impl Tree {
        fn new(layout: &Value) -> Self {
            let dir = tempfile::tempdir().expect("a temporary directory");
            let root = fs::canonicalize(dir.path()).expect("the temporary directory exists");
            let tree = Self {
                _dir: dir,
                root: root.to_str().expect("a UTF-8 temporary path").to_owned(),
            };
            for dir in items(&layout["dirs"]) {
                fs::create_dir_all(tree.path(text(dir))).expect("create a directory");
            }
            for file in items(&layout["files"]) {
                fs::write(tree.path(text(file)), b"").expect("create a file");
            }
            for link in items(&layout["symlinks"]) {
                symlink(tree.local(text(&link[1])), tree.path(text(&link[0])))
                    .expect("create a symbolic link");
            }
            tree
        }

        fn path(&self, relative: &str) -> PathBuf {
            Path::new(&self.root).join(relative)
        }

        fn local(&self, text: &str) -> String {
            text.replace("<D>", &self.root)
        }

        fn out(&self, value: Value) -> Value {
            match value {
                Value::String(text) => Value::String(text.replace(&self.root, "<D>")),
                Value::Array(items) => items.into_iter().map(|v| self.out(v)).collect(),
                Value::Object(map) => Value::Object(
                    map.into_iter()
                        .map(|(key, value)| (key, self.out(value)))
                        .collect(),
                ),
                other => other,
            }
        }

        fn policy(&self, roots: &Value, base: &Value) -> PathPolicy {
            let roots: Vec<String> = items(roots).iter().map(|r| self.local(text(r))).collect();
            PathPolicy::new(&roots, Path::new(&self.local(text(base))))
        }
    }

    fn access(name: &str) -> Access {
        match name {
            "read" => Access::Read,
            "write" => Access::Write,
            other => panic!("unknown access {other}"),
        }
    }

    #[test]
    fn resolution_matches_the_prototype() {
        let section = &FIXTURE["sandbox"];
        let tree = Tree::new(&section["layout"]);
        let mut crashes = 0;
        for case in items(&section["resolve"]) {
            let policy = tree.policy(&case["roots"], &case["base"]);
            let path = tree.local(text(&case["path"]));
            let result = policy
                .resolve(&path, access(text(&case["access"])))
                .map(|resolved| json!(resolved.to_str().unwrap()));
            assert_eq!(tree.out(outcome(result)), expected(case), "{case}");
            crashes += usize::from(case.get("crash").is_some());
        }
        // `~unknown-user/…`, `~x.wav` and a NUL character crash the prototype.
        assert_eq!(crashes, 3);
        assert_eq!(items(&section["resolve"]).len(), 44);
    }

    #[test]
    fn roots_are_resolved_and_deduplicated_in_order() {
        let section = &FIXTURE["sandbox"];
        let tree = Tree::new(&section["layout"]);
        for case in items(&section["allow"]) {
            let mut policy = tree.policy(&case["roots"], &case["base"]);
            for step in items(&case["steps"]) {
                // A root the prototype crashes on is skipped (`rust: skipped`), so the roots stay
                // as the prototype left them.
                policy.allow(tree.local(text(&step["allow"])));
                assert_eq!(tree.out(json!(policy.roots())), step["roots"], "{step}");
            }
            assert_eq!(
                policy.roots(),
                policy
                    .roots_paths()
                    .iter()
                    .map(|p| p.to_str().unwrap().to_owned())
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn environment_roots_match_the_prototype() {
        let section = &FIXTURE["sandbox"];
        let tree = Tree::new(&section["layout"]);
        for case in items(&section["environment"]) {
            let configured: Vec<String> = items(&case["configured"])
                .iter()
                .map(|c| tree.local(text(c)))
                .collect();
            let extra: Vec<PathBuf> = items(&case["extra"])
                .iter()
                .map(|e| PathBuf::from(tree.local(text(e))))
                .collect();
            let env = OsString::from(tree.local(text(&case["env"])));
            let base = PathBuf::from(tree.local(text(&case["base"])));
            let policy =
                PathPolicy::from_environment(&configured, Some(&env), Some(&base), &extra).unwrap();
            assert_eq!(tree.out(json!(policy.roots())), case["roots"], "{case}");
            assert_eq!(
                tree.out(json!(policy.base().to_str().unwrap())),
                case["resolved_base"]
            );
        }
    }

    #[test]
    fn without_a_base_the_working_directory_is_the_base_but_not_a_root() {
        let cwd = fs::canonicalize(std::env::current_dir().unwrap()).unwrap();
        let policy = PathPolicy::from_environment(&["/".to_owned()], None, None, &[]).unwrap();
        assert_eq!(policy.base(), cwd);
        assert_eq!(policy.roots(), ["/"]);
    }

    #[test]
    fn relative_roots_and_bases_resolve_against_the_working_directory() {
        let cwd = fs::canonicalize(std::env::current_dir().unwrap()).unwrap();
        let mut policy = PathPolicy::new(["sonara-relative-root"], Path::new("sonara-base"));
        policy.allow("./sonara-relative-root/");
        assert_eq!(policy.roots_paths(), [cwd.join("sonara-relative-root")]);
        assert_eq!(policy.base(), cwd.join("sonara-base"));
    }

    #[test]
    fn a_leading_tilde_is_the_home_directory() {
        let home = fs::canonicalize(std::env::home_dir().unwrap()).unwrap();
        let policy = PathPolicy::new([&home], Path::new("/"));
        assert_eq!(
            policy
                .resolve("~/sonara-agent-test/x.wav", Access::Read)
                .unwrap(),
            home.join("sonara-agent-test/x.wav")
        );
        assert_eq!(policy.resolve("~", Access::Write).unwrap(), home);
        // Another user's home directory is not looked up (the prototype expands an existing
        // user's, and crashes on an unknown one).
        let err = policy.resolve("~root/x", Access::Read).unwrap_err();
        assert_eq!(err.code, ErrorCode::PermissionDenied);
        assert_eq!(
            err.message,
            "cannot resolve ~root/x: Could not determine home directory."
        );
        let mut roots = policy.clone();
        roots.allow("~root");
        assert_eq!(roots, policy);
    }

    /// The path argument and access of the tools whose denials the transcript records.
    const PATH_ARGUMENTS: [(&str, &str, Access); 3] = [
        ("import_audio", "path", Access::Read),
        ("export_stems", "directory", Access::Write),
        ("open_project", "path", Access::Read),
    ];

    #[test]
    fn transcript_denials_match() {
        let dir = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        let root = root.to_str().unwrap();
        let mut checked = Vec::new();
        for step in items(&TRANSCRIPT["steps"]) {
            let Some(recorded) = step["result"].get("error") else {
                continue;
            };
            if recorded["code"] != "PERMISSION_DENIED" {
                continue;
            }
            let tool = text(&step["tool"]);
            let (_, argument, access) = PATH_ARGUMENTS
                .iter()
                .find(|(name, ..)| *name == tool)
                .unwrap_or_else(|| panic!("no path argument known for {tool}"));
            // The roots the session had allowed by then (the recorder's base is `<ROOT>/work`).
            let roots: Vec<String> = items(&recorded["details"]["allowed_roots"])
                .iter()
                .map(|r| text(r).replace("<ROOT>", root))
                .collect();
            let policy = PathPolicy::new(&roots, &Path::new(root).join("work"));
            let path = text(&step["args"][*argument]).replace("<ROOT>", root);
            let error = policy.resolve(&path, *access).unwrap_err();
            let actual = error_json(&error).to_string().replace(root, "<ROOT>");
            assert_eq!(
                serde_json::from_str::<Value>(&actual).unwrap(),
                *recorded,
                "{}",
                step["id"]
            );
            checked.push(text(&step["id"]).to_owned());
        }
        assert_eq!(checked, ["step80", "step147", "step166"]);
    }
}

mod confirmations {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    use serde_json::{Value, json};
    use sonara_agent::contract::{find, validate};
    use sonara_agent::permissions::{
        Confirmations, MonotonicClock, TOKEN_TTL, random_tokens, sequential_tokens,
    };

    use super::{FIXTURE, TRANSCRIPT, error_json, expected, items, outcome, text};

    /// A monotonic clock that moves only when the returned counter (milliseconds) does.
    fn manual_clock() -> (Arc<AtomicU64>, MonotonicClock) {
        let elapsed_ms = Arc::new(AtomicU64::new(0));
        let start = Instant::now();
        let elapsed = Arc::clone(&elapsed_ms);
        let clock = Box::new(move || start + Duration::from_millis(elapsed.load(Ordering::SeqCst)));
        (elapsed_ms, clock)
    }

    #[test]
    fn scenarios_match_the_prototype() {
        let mut calls = 0;
        for scenario in items(&FIXTURE["confirmations"]["scenarios"]) {
            let (elapsed, clock) = manual_clock();
            let enabled = scenario["enabled"].as_bool().unwrap();
            let confirmations = Confirmations::with_sources(enabled, sequential_tokens(), clock);
            assert_eq!(confirmations.is_enabled(), enabled);
            for step in items(&scenario["steps"]) {
                if let Some(ms) = step.get("advance_ms") {
                    elapsed.fetch_add(ms.as_u64().unwrap(), Ordering::SeqCst);
                    continue;
                }
                let result = confirmations
                    .require(
                        text(&step["action"]),
                        step["args"].as_object().unwrap(),
                        step["token"].as_str(),
                        text(&step["description"]),
                    )
                    .map(|()| Value::Null);
                assert_eq!(
                    outcome(result),
                    expected(step),
                    "{}: {step}",
                    scenario["name"]
                );
                calls += 1;
            }
        }
        assert_eq!(calls, 34);
        assert_eq!(TOKEN_TTL.as_secs(), 600);
    }

    #[test]
    fn random_tokens_are_fresh() {
        let confirmations = Confirmations::new(true);
        let args = json!({"track_id": "trk_1"});
        let args = args.as_object().unwrap();
        let token = |result: Result<(), sonara_core::error::DomainError>| {
            error_json(&result.unwrap_err())["details"]["confirmation_token"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        let first = token(confirmations.require("delete_track", args, None, "x"));
        let second = token(confirmations.require("delete_track", args, None, "x"));
        assert_ne!(first, second);
        confirmations
            .require("delete_track", args, Some(&first), "x")
            .unwrap();
        let mut tokens = random_tokens();
        assert_eq!(tokens().len(), 36);
    }

    const SUFFIX: &str =
        ". Ask the user; if they agree, repeat the identical call with this confirmation_token.";

    /// Replays every call of the coverage transcript that issued or presented a token, through
    /// one `Confirmations` with golden tokens, as the recorder's session did.
    #[test]
    fn transcript_round_trips_match() {
        let confirmations =
            Confirmations::with_sources(true, sequential_tokens(), Box::new(Instant::now));
        let mut replayed = Vec::new();
        for step in items(&TRANSCRIPT["steps"]) {
            let Some(tool) = step["tool"].as_str() else {
                continue; // a harness step
            };
            let result = &step["result"];
            let required = result
                .get("error")
                .is_some_and(|error| error["code"] == "CONFIRMATION_REQUIRED");
            let token = step["args"]
                .get("confirmation_token")
                .and_then(Value::as_str);
            if !required && token.is_none() {
                continue;
            }
            // The handlers bind the validated arguments (`model_dump`), not the raw ones.
            let args = validate(find(tool).unwrap(), &step["args"]).unwrap();
            let description = if required {
                text(&result["error"]["message"])
                    .strip_suffix(SUFFIX)
                    .unwrap()
            } else {
                "accepted"
            };
            match confirmations.require(tool, &args, token, description) {
                Ok(()) => assert_eq!(result["ok"], true, "{}", step["id"]),
                Err(error) => assert_eq!(error_json(&error), result["error"], "{}", step["id"]),
            }
            replayed.push(text(&step["id"]).to_owned());
        }
        assert_eq!(
            replayed,
            [
                "stems_confirm",
                "step146",
                "open_confirm",
                "step164",
                "step170",
                "step171"
            ]
        );
    }
}

mod audit {
    use std::fs;
    use std::time::{Duration, UNIX_EPOCH};

    use serde_json::{Value, json};
    use sonara_agent::audit::{AuditLog, AuditRecord, FILE_NAME, MAX_BYTES, WallClock};
    use sonara_core::error::ErrorCode;

    use super::{FIXTURE, items, text};

    /// `golden_common.FIXED_NOW`, 2026-01-01T12:00:00Z.
    fn fixed_clock() -> WallClock {
        Box::new(|| UNIX_EPOCH + Duration::from_hours(490_908))
    }

    fn code(name: &Value) -> Option<ErrorCode> {
        name.as_str().map(|name| {
            *ErrorCode::ALL
                .iter()
                .find(|code| code.as_str() == name)
                .expect("a known error code")
        })
    }

    fn keys(value: &Value) -> Vec<&str> {
        value
            .as_object()
            .expect("an entry object")
            .keys()
            .map(String::as_str)
            .collect()
    }

    const CALL: AuditRecord<'static> = AuditRecord {
        tool: "list_tracks",
        args: &Value::Null,
        error: None,
        duration: Duration::from_millis(2),
        project_version: Some(1),
        source: "agent",
    };

    #[test]
    fn entries_match_the_prototype() {
        let section = &FIXTURE["audit"];
        let dir = tempfile::tempdir().unwrap();
        let path = AuditLog::default_path(&dir.path().join("logs"));
        assert!(path.ends_with("logs/agent-audit.jsonl"));
        let log = AuditLog::with_clock(Some(path.clone()), fixed_clock());
        for record in items(&section["records"]) {
            log.record(&AuditRecord {
                tool: text(&record["tool"]),
                args: &record["args"],
                error: code(&record["error"]),
                duration: Duration::from_nanos(record["nanos"].as_u64().unwrap()),
                project_version: record["version"].as_u64(),
                source: text(&record["source"]),
            })
            .unwrap();
        }
        let written = fs::read_to_string(&path).unwrap();
        let entries: Vec<Value> = written
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let recorded = items(&section["entries"]);
        assert_eq!(entries.len(), recorded.len());
        for (entry, recorded) in entries.iter().zip(recorded) {
            assert_eq!(entry, recorded);
            assert_eq!(keys(entry), keys(recorded));
            assert_eq!(entry["ts"], section["timestamp"]);
        }
        assert_eq!(Value::Array(log.tail(3).unwrap()), section["tail_3"]);
    }

    #[test]
    fn the_log_rotates_past_ten_mebibytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        let rotated = dir.path().join("agent-audit.1.jsonl");
        let log = AuditLog::with_clock(Some(path.clone()), fixed_clock());
        fs::File::create(&path).unwrap().set_len(MAX_BYTES).unwrap();
        log.record(&CALL).unwrap();
        assert!(!rotated.exists(), "a file of exactly MAX_BYTES is kept");
        let size = fs::metadata(&path).unwrap().len();
        assert!(size > MAX_BYTES);
        fs::write(&rotated, "older rotation\n").unwrap();
        log.record(&CALL).unwrap();
        assert_eq!(fs::metadata(&rotated).unwrap().len(), size);
        let fresh = fs::read_to_string(&path).unwrap();
        assert_eq!(fresh.lines().count(), 1);
        let entry: Value = serde_json::from_str(fresh.trim_end()).unwrap();
        assert_eq!(entry["tool"], "list_tracks");
        assert_eq!(entry["ms"], 2.0);
    }

    #[test]
    fn tail_skips_lines_that_are_not_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        fs::write(&path, "{\"a\": 1}\nnot json\n\n[2]\n{\"b\": 3}\n").unwrap();
        let log = AuditLog::new(Some(path));
        // As `splitlines()[-3:]`: the blank line counts, then fails to parse.
        assert_eq!(log.tail(3).unwrap(), [json!([2]), json!({"b": 3})]);
        assert_eq!(
            log.tail(20).unwrap(),
            [json!({"a": 1}), json!([2]), json!({"b": 3})]
        );
        let none = log.tail(0).unwrap();
        assert_eq!(none, Vec::<Value>::new());
        let missing = AuditLog::new(Some(dir.path().join("none.jsonl")));
        let none = missing.tail(20).unwrap();
        assert_eq!(none, Vec::<Value>::new());
    }

    #[test]
    fn a_disabled_log_writes_nothing() {
        let log = AuditLog::new(None);
        log.record(&CALL).unwrap();
        assert!(log.path().is_none());
        let none = log.tail(20).unwrap();
        assert_eq!(none, Vec::<Value>::new());
    }

    #[test]
    fn failures_name_the_file_and_keep_the_cause() {
        let dir = tempfile::tempdir().unwrap();
        // The path is a directory, so appending fails.
        let log = AuditLog::new(Some(dir.path().to_owned()));
        let error = log.record(&CALL).unwrap_err();
        let prefix = format!("cannot write audit log {}: ", dir.path().display());
        assert!(error.to_string().starts_with(&prefix), "{error}");
        assert!(std::error::Error::source(&error).is_some());
        let error = log.tail(1).unwrap_err();
        assert!(
            error.to_string().starts_with("cannot read audit log "),
            "{error}"
        );
    }
}

mod conversion {
    use std::sync::Arc;

    use serde_json::{Value, json};
    use sonara_agent::contract::args::{Length, NoteInput, Pitch, Position};
    use sonara_agent::conversion::{beats, length_beats, notes, pitch};
    use sonara_core::ids::SequentialIds;
    use sonara_core::model::project::Project;
    use sonara_core::plugins::PluginRegistry;
    use sonara_core::session::ProjectSession;
    use sonara_core::time::{TempoMap, TempoPoint, TimeSignatureMap, TimeSignaturePoint};

    use super::{FIXTURE, expected, items, outcome};

    /// The fixture's project: a tempo map and time signatures with changes.
    fn project(shape: &Value) -> Project {
        let points = |key: &str| {
            items(&shape[key])
                .iter()
                .map(|p| p.as_array().expect("a point"))
        };
        let tempo = TempoMap::new(
            points("tempo")
                .map(|p| {
                    TempoPoint::new(
                        p[0].as_f64().expect("a beat"),
                        p[1].as_f64().expect("a tempo"),
                        p[2].as_bool().expect("a ramp flag"),
                    )
                    .expect("a valid tempo point")
                })
                .collect(),
        )
        .expect("a valid tempo map");
        let signatures = TimeSignatureMap::new(
            points("signatures")
                .map(|p| {
                    let part = |i: usize| {
                        u32::try_from(p[i].as_u64().expect("a count")).expect("a small count")
                    };
                    TimeSignaturePoint::new(p[0].as_i64().expect("a bar"), part(1), part(2))
                        .expect("a valid signature")
                })
                .collect(),
        )
        .expect("valid time signatures");
        let mut ids = SequentialIds::new(0);
        Project::create(
            &mut ids,
            "Song",
            48_000,
            shape["bpm"].as_f64().expect("a tempo"),
        )
        .expect("a valid project")
        .edit(|data| {
            data.tempo_map = tempo;
            data.time_signatures = signatures;
        })
        .expect("the edited project is valid")
    }

    /// A float as the fixture writes it.
    fn number(value: f64) -> Value {
        if value.is_nan() {
            json!({"non_finite": "nan"})
        } else if value.is_infinite() {
            json!({"non_finite": if value > 0.0 { "inf" } else { "-inf" }})
        } else {
            json!(value)
        }
    }

    fn typed<T: serde::de::DeserializeOwned>(value: &Value) -> T {
        serde_json::from_value(value.clone()).expect("a typed argument")
    }

    #[test]
    fn positions_match_the_prototype() {
        let section = &FIXTURE["conversion"];
        let project = project(&section["project"]);
        let cases = items(&section["positions"]);
        for case in cases {
            let value: Position = typed(&case["value"]);
            let result = beats(&value, || Ok(&project)).map(number);
            assert_eq!(outcome(result), expected(case), "{}", case["value"]);
        }
        assert_eq!(cases.len(), 56);
    }

    #[test]
    fn numbers_convert_without_an_open_project() {
        let session = ProjectSession::new(Arc::new(PluginRegistry::new()));
        for case in items(&FIXTURE["conversion"]["positions_without_project"]) {
            let value: Position = typed(&case["value"]);
            let result = beats(&value, || session.require_project()).map(number);
            assert_eq!(outcome(result), expected(case), "{}", case["value"]);
        }
    }

    #[test]
    fn lengths_match_the_prototype() {
        let section = &FIXTURE["conversion"];
        let project = project(&section["project"]);
        let mut ids = SequentialIds::new(100);
        let bpm = section["override_project"]["bpm"].as_f64().unwrap();
        let shape = Project::create(&mut ids, "New", 48_000, bpm).unwrap();
        for (key, project) in [("lengths", &project), ("lengths_with_override", &shape)] {
            for case in items(&section[key]) {
                let value: Length = typed(&case["value"]);
                let start = case["start"].as_f64().unwrap();
                let result = length_beats(&value, start, project).map(number);
                assert_eq!(outcome(result), expected(case), "{key} {case}");
            }
        }
    }

    #[test]
    fn pitches_match_the_prototype() {
        for case in items(&FIXTURE["conversion"]["pitches"]) {
            let value: Pitch = typed(&case["value"]);
            assert_eq!(
                outcome(pitch(&value).map(Value::from)),
                expected(case),
                "{}",
                case["value"]
            );
        }
    }

    #[test]
    fn notes_match_the_prototype() {
        for case in items(&FIXTURE["conversion"]["notes"]) {
            let inputs: Vec<NoteInput> = typed(&case["notes"]);
            let mut ids = SequentialIds::new(0);
            let result = notes(&inputs, &mut ids).map(|notes| {
                notes
                    .iter()
                    .map(|n| {
                        json!({"id": n.id, "pitch": n.pitch, "start": n.start, "length": n.length,
                               "velocity": n.velocity, "channel": n.channel})
                    })
                    .collect()
            });
            assert_eq!(outcome(result), expected(case), "{}", case["notes"]);
        }
    }
}
