//! `sonara gui [PROJECT]`: it runs the `sonara-desktop` executable
//! next to `sonara`, with the project, and exits with its exit code. A copy of the built
//! `sonara` runs in a folder of its own, so that a stub stands in for the desktop application
//! (no window opens), or nothing does.
//!
//! Unix only: the stub is a shell script.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use crate::support::Sandbox;

/// A copy of the built `sonara` in `<sandbox>/bin`.
fn installed(sandbox: &Sandbox) -> PathBuf {
    let bin = sandbox.root().join("bin");
    fs::create_dir(&bin).expect("a bin folder");
    let sonara = bin.join("sonara");
    fs::copy(env!("CARGO_BIN_EXE_sonara"), &sonara).expect("sonara copied");
    sonara
}

/// A `sonara-desktop` stub next to `sonara` that records its arguments and exits with 7.
fn stub_desktop(sonara: &Path, record: &Path) {
    let stub = sonara.with_file_name("sonara-desktop");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexit 7\n",
        record.display()
    );
    fs::write(&stub, script).expect("the stub is written");
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).expect("executable");
}

#[test]
fn gui_runs_the_desktop_application_next_to_sonara() {
    let sandbox = Sandbox::new();
    let sonara = installed(&sandbox);
    let record = sandbox.root().join("args.txt");
    stub_desktop(&sonara, &record);
    let run = |args: &[&str]| {
        let status = std::process::Command::new(&sonara)
            .current_dir(sandbox.root())
            .env("SONARA_HOME", sandbox.root().join("home"))
            .args(args)
            .status()
            .expect("sonara runs");
        let recorded = fs::read_to_string(&record).expect("the desktop ran");
        fs::remove_file(&record).expect("reset");
        (status.code(), recorded)
    };
    assert_eq!(
        run(&["gui", "songs/My Song.sonara"]),
        (Some(7), "songs/My Song.sonara\n".to_owned()),
        "the project as given, and the desktop's exit code"
    );
    assert_eq!(run(&["gui"]), (Some(7), "\n".to_owned()), "no project");
}

#[test]
fn gui_without_the_desktop_application_fails_with_a_reason() {
    let sandbox = Sandbox::new();
    let sonara = installed(&sandbox);
    let output = std::process::Command::new(&sonara)
        .current_dir(sandbox.root())
        .env("SONARA_HOME", sandbox.root().join("home"))
        .arg("gui")
        .output()
        .expect("sonara runs");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"");
    let stderr = String::from_utf8(output.stderr).expect("UTF-8");
    assert!(
        stderr.contains("the desktop application is not installed")
            && stderr.contains("sonara-desktop is missing"),
        "{stderr}"
    );
}
