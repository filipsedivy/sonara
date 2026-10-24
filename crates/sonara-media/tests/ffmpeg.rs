//! The `FFmpeg` import fallback: discovery on `PATH`, the
//! `ffprobe` inspection and the `ffmpeg` transcode, with their errors.
//!
//! Oracle: `tests/fixtures/formats/oracle.json`, recorded from the prototype by
//! `scripts/golden/media_format_oracles.py` (GLD-01): `shutil.which` over a layout of
//! directories (`which`), and `inspect_audio` with stand-in `ffprobe` and `ffmpeg` scripts on
//! `PATH` (`ffmpeg.inspect`), which print canned output and record their arguments. The same
//! scripts run here, so the tests are deterministic and need no `FFmpeg` (TST-04, TST-08). The
//! import itself (the asset, the original's copy, the warnings) is tested in `sonara-app`.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use sonara_core::error::ErrorCode;
use sonara_media::{Ffmpeg, inspect_audio_with, which};

fn oracle() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/formats/oracle.json");
    serde_json::from_str(&fs::read_to_string(path).expect("the oracle")).expect("JSON")
}

fn executable(path: &Path, text: &str) {
    fs::write(path, text).expect("a script");
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("executable");
}

/// The recorder's stand-in `ffprobe`: records its arguments, prints `out` and `err`, exits
/// with `rc`.
fn fake_ffprobe(bin: &Path, out: &str, err: &str, rc: i64) {
    let d = bin.display();
    fs::write(bin.join("ffprobe.out"), out).expect("output");
    fs::write(bin.join("ffprobe.err"), err).expect("error output");
    executable(
        &bin.join("ffprobe"),
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{d}/ffprobe.args'\n/bin/cat '{d}/ffprobe.out'\n\
             /bin/cat '{d}/ffprobe.err' >&2\nexit {rc}\n"
        ),
    );
}

/// The recorder's stand-in `ffmpeg`: records its arguments, then prints `err` and exits with
/// `rc`, or (with `rc` 0) copies `flac` to its last argument.
fn fake_ffmpeg(bin: &Path, flac: &Path, err: &str, rc: i64) {
    let d = bin.display();
    let body = if rc == 0 {
        format!(
            "for last; do :; done\n/bin/cp '{}' \"$last\"\n",
            flac.display()
        )
    } else {
        fs::write(bin.join("ffmpeg.err"), err).expect("error output");
        format!("/bin/cat '{d}/ffmpeg.err' >&2\nexit {rc}\n")
    };
    executable(
        &bin.join("ffmpeg"),
        &format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{d}/ffmpeg.args'\n{body}"),
    );
}

fn args(bin: &Path, tool: &str, root: &Path) -> Option<Vec<String>> {
    let text = fs::read_to_string(bin.join(format!("{tool}.args"))).ok()?;
    Some(
        text.lines()
            .map(|line| line.replace(&root.display().to_string(), "<ROOT>"))
            .collect(),
    )
}

fn sanitized(value: &Value, root: &Path) -> Value {
    match value {
        Value::String(text) => Value::String(text.replace(&root.display().to_string(), "<ROOT>")),
        Value::Array(items) => Value::Array(items.iter().map(|v| sanitized(v, root)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), sanitized(v, root)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn root() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let root = fs::canonicalize(dir.path()).expect("resolves");
    (dir, root)
}

#[test]
fn discovery_finds_executables_as_shutil_which_does() {
    let (_dir, root) = root();
    for name in ["plain", "folder", "tool", "link", "empty"] {
        fs::create_dir(root.join(name)).expect("a folder");
    }
    fs::write(root.join("plain/ffmpeg"), "#!/bin/sh\n").expect("a plain file");
    fs::create_dir(root.join("folder/ffmpeg")).expect("a folder named ffmpeg");
    executable(&root.join("tool/ffmpeg"), "#!/bin/sh\n");
    std::os::unix::fs::symlink(root.join("tool/ffmpeg"), root.join("link/ffmpeg")).expect("a link");
    let cases = oracle()["which"].as_array().expect("cases").clone();
    assert!(cases.len() >= 8);
    for case in cases {
        let dirs: Vec<String> = case["dirs"]
            .as_array()
            .expect("dirs")
            .iter()
            .map(|d| {
                let d = d.as_str().expect("a name");
                if d.is_empty() {
                    String::new()
                } else {
                    root.join(d).display().to_string()
                }
            })
            .collect();
        let search = dirs.join(":");
        let found = which("ffmpeg", Some(search.as_ref())).map(|p| {
            p.display()
                .to_string()
                .replace(&root.display().to_string(), "<ROOT>")
        });
        assert_eq!(
            found.as_deref(),
            case["found"].as_str(),
            "{:?}",
            case["dirs"]
        );
    }
}

#[test]
fn discovery_needs_ffmpeg_and_takes_ffprobe_from_the_same_search() {
    let (_dir, root) = root();
    let bin = root.join("bin");
    fs::create_dir(&bin).expect("bin");
    assert_eq!(Ffmpeg::discover_in(Some(bin.as_os_str())), None);
    fake_ffprobe(&bin, "{}", "", 0);
    assert_eq!(
        Ffmpeg::discover_in(Some(bin.as_os_str())),
        None,
        "ffprobe alone"
    );
    fake_ffmpeg(&bin, &root.join("x.flac"), "", 0);
    let found = Ffmpeg::discover_in(Some(bin.as_os_str())).expect("ffmpeg");
    assert_eq!(found.ffmpeg(), bin.join("ffmpeg"));
    assert_eq!(found.ffprobe(), Some(bin.join("ffprobe").as_path()));
    assert_eq!(
        Ffmpeg::discover_in(Some("".as_ref())),
        None,
        "an empty PATH"
    );
}

#[test]
fn inspection_matches_the_prototypes_ffprobe_fallback() {
    let oracle = oracle();
    let not_audio = oracle["ffmpeg"]["not_audio"]
        .as_str()
        .expect("the stand-in");
    let scenarios = oracle["ffmpeg"]["inspect"]
        .as_array()
        .expect("scenarios")
        .clone();
    assert!(scenarios.len() >= 9);
    for scenario in scenarios {
        let name = scenario["name"].as_str().expect("a name");
        let (_dir, root) = root();
        let bin = root.join(name).join("bin");
        fs::create_dir_all(&bin).expect("bin");
        if let Some(out) = scenario["probe"].as_str() {
            fake_ffprobe(
                &bin,
                out,
                scenario["probe_stderr"].as_str().expect("stderr"),
                scenario["probe_rc"].as_i64().expect("rc"),
            );
        }
        fake_ffmpeg(&bin, &root.join("unused.flac"), "", 0);
        let source = root
            .join(name)
            .join("in")
            .join(scenario["file"].as_str().expect("a file"));
        fs::create_dir_all(source.parent().expect("a folder")).expect("in");
        fs::write(&source, not_audio).expect("the stand-in");
        let ffmpeg = Ffmpeg::discover_in(Some(bin.as_os_str()));
        assert!(ffmpeg.is_some(), "{name}");
        match inspect_audio_with(&source, ffmpeg.as_ref()) {
            Ok(info) => {
                let json = sanitized(&info.to_json(), &root);
                assert_eq!(json, scenario["inspect"], "{name}");
                let order: Vec<&str> = info.metadata.iter().map(|(k, _)| k.as_str()).collect();
                let expected: Vec<&str> = scenario["metadata_order"]
                    .as_array()
                    .expect("an order")
                    .iter()
                    .map(|k| k.as_str().expect("a key"))
                    .collect();
                assert_eq!(order, expected, "{name}");
            }
            Err(err) => {
                assert_eq!(err.code.as_str(), scenario["error"]["code"], "{name}");
                assert_eq!(
                    Value::String(err.message.replace(&root.display().to_string(), "<ROOT>")),
                    scenario["error"]["message"],
                    "{name}"
                );
            }
        }
        let recorded: Option<Vec<String>> = scenario["ffprobe_args"].as_array().map(|a| {
            a.iter()
                .map(|v| v.as_str().expect("an argument").to_owned())
                .collect()
        });
        assert_eq!(
            args(&bin, "ffprobe", &root),
            recorded,
            "{name}: ffprobe's arguments"
        );
    }
}

#[test]
fn a_failed_transcode_reports_ffmpegs_error_and_leaves_no_partial_file() {
    let oracle = oracle();
    let scenario = oracle["ffmpeg"]["import"]
        .as_array()
        .expect("scenarios")
        .iter()
        .find(|s| s["name"] == "transcode_fails")
        .expect("the failing transcode")
        .clone();
    let (_dir, root) = root();
    let bin = root.join("transcode_fails").join("bin");
    fs::create_dir_all(&bin).expect("bin");
    fake_ffmpeg(
        &bin,
        &root.join("unused.flac"),
        scenario["ffmpeg_stderr"].as_str().expect("stderr"),
        scenario["ffmpeg_rc"].as_i64().expect("rc"),
    );
    let source = root.join("transcode_fails/in/Song Take 1.M4A");
    fs::create_dir_all(source.parent().expect("a folder")).expect("in");
    fs::write(&source, "x").expect("a source");
    let dest = root.join("transcode_fails/ws/media/audio/med_test0005-Song-Take-1.flac");
    let ffmpeg = Ffmpeg::discover_in(Some(bin.as_os_str())).expect("ffmpeg");
    let err = ffmpeg.transcode_to_flac(&source, &dest).expect_err("fails");
    assert_eq!(err.code, ErrorCode::IoError);
    assert_eq!(Value::String(err.message), scenario["error"]["message"]);
    let recorded: Vec<String> = scenario["ffmpeg_args"]
        .as_array()
        .expect("arguments")
        .iter()
        .map(|v| v.as_str().expect("an argument").to_owned())
        .collect();
    assert_eq!(args(&bin, "ffmpeg", &root), Some(recorded));
    assert!(!dest.exists());
    assert!(!dest.with_extension("tmp.flac").exists());
    assert!(
        dest.parent().expect("a folder").is_dir(),
        "the folder is made first"
    );
}

#[test]
fn a_successful_transcode_lands_under_the_final_name() {
    let (_dir, root) = root();
    let bin = root.join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let flac = root.join("made.flac");
    fs::write(&flac, b"fLaC stand-in").expect("a stand-in");
    fake_ffmpeg(&bin, &flac, "", 0);
    let dest = root.join("ws/media/audio/med_1-x.flac");
    let ffmpeg = Ffmpeg::discover_in(Some(bin.as_os_str())).expect("ffmpeg");
    ffmpeg
        .transcode_to_flac(&root.join("x.m4a"), &dest)
        .expect("transcodes");
    assert_eq!(fs::read(&dest).expect("the result"), b"fLaC stand-in");
    assert!(!dest.with_extension("tmp.flac").exists());
}

#[test]
fn a_tool_that_overruns_its_time_is_stopped() {
    let (_dir, root) = root();
    let bin = root.join("bin");
    fs::create_dir_all(&bin).expect("bin");
    executable(&bin.join("ffprobe"), "#!/bin/sh\nexec /bin/sleep 30\n");
    executable(&bin.join("ffmpeg"), "#!/bin/sh\nexec /bin/sleep 30\n");
    let ffmpeg = Ffmpeg::discover_in(Some(bin.as_os_str()))
        .expect("ffmpeg")
        .with_timeouts(Duration::from_secs(1), Duration::from_secs(1));
    let source = root.join("slow.m4a");
    fs::write(&source, "x").expect("a source");
    let started = std::time::Instant::now();
    let err = ffmpeg.inspect(&source, 1).expect_err("times out");
    assert!(started.elapsed() < Duration::from_secs(10));
    assert_eq!(err.code, ErrorCode::Internal);
    let probe = bin.join("ffprobe").display().to_string();
    assert_eq!(
        err.message,
        format!(
            "TimeoutExpired: Command '['{probe}', '-v', 'error', '-select_streams', 'a:0', \
             '-show_entries', \
             'stream=sample_rate,channels,codec_name,duration_ts,time_base,duration:format_tags', \
             '-of', 'json', '{}']' timed out after 1 seconds",
            source.display()
        )
    );
    let dest = root.join("out/x.flac");
    let err = ffmpeg
        .transcode_to_flac(&source, &dest)
        .expect_err("times out");
    assert_eq!(err.code, ErrorCode::Internal);
    assert!(
        err.message.starts_with("TimeoutExpired: Command '['"),
        "{}",
        err.message
    );
    assert!(!dest.with_extension("tmp.flac").exists());
}

#[test]
fn a_tool_that_cannot_start_is_an_internal_error() {
    let (_dir, root) = root();
    let bin = root.join("bin");
    fs::create_dir_all(&bin).expect("bin");
    executable(&bin.join("ffprobe"), "#!/nonexistent/interpreter\n");
    executable(&bin.join("ffmpeg"), "#!/nonexistent/interpreter\n");
    let ffmpeg = Ffmpeg::discover_in(Some(bin.as_os_str())).expect("ffmpeg");
    let source = root.join("x.m4a");
    fs::write(&source, "x").expect("a source");
    let err = ffmpeg.inspect(&source, 1).expect_err("cannot start");
    assert_eq!(err.code, ErrorCode::Internal);
    assert_eq!(
        err.message,
        format!(
            "FileNotFoundError: [Errno 2] No such file or directory: '{}'",
            bin.join("ffprobe").display()
        )
    );
}

#[test]
fn output_that_is_not_ffprobes_json_is_an_internal_error() {
    let (_dir, root) = root();
    let bin = root.join("bin");
    fs::create_dir_all(&bin).expect("bin");
    fake_ffmpeg(&bin, &root.join("unused.flac"), "", 0);
    let source = root.join("x.m4a");
    fs::write(&source, "x").expect("a source");
    for (out, prefix) in [
        ("not json", "JSONDecodeError: "),
        ("[1, 2]", "AttributeError: "),
        (r#"{"streams": [{"sample_rate": "fast"}]}"#, "ValueError: "),
        (
            r#"{"streams": [{"duration": "inf", "sample_rate": "1"}]}"#,
            "ValueError: ",
        ),
    ] {
        fake_ffprobe(&bin, out, "", 0);
        let ffmpeg = Ffmpeg::discover_in(Some(bin.as_os_str())).expect("ffmpeg");
        let err = ffmpeg.inspect(&source, 1).expect_err(out);
        assert_eq!(err.code, ErrorCode::Internal, "{out}");
        assert!(err.message.starts_with(prefix), "{out}: {}", err.message);
    }
}
