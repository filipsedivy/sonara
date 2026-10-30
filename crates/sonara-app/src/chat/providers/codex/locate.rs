//! Finding the Codex CLI, checking its version and asking whether it is signed in.
//!
//! The places are the `PATH`, Homebrew's and `/usr/local`'s `bin`, and the places an npm or a
//! `curl` installation uses under the home directory (`~/.local/bin`, `~/.npm-global/bin`). A
//! `~/.codex/bin` is not searched: neither the decision record nor the CLI's own documentation
//! names it as an installation place.
//!
//! Two bounded questions are asked of the CLI (a hanging CLI must not hang the window):
//! `codex --version`, which prints `codex-cli 0.166.0`, and `codex login status`, which prints
//! to standard error and exits 0 when signed in and 1 when not. Credentials are never read: not
//! `~/.codex/auth.json`, not any token file, and the value of `CODEX_API_KEY` is never looked at,
//! only whether it is set (`codex exec` honours it, `login status` does not).

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::chat::provider::{Availability, ProviderStatus};
use crate::chat::providers::probe::run_bounded;
use crate::chat::task::ProviderId;
use crate::composer::invocation::search_path;
use crate::composer::locate::{LocateError, Version, candidates_named, locate_named_in};

/// The name of the provider in the window.
pub const NAME: &str = "Codex CLI";

/// The oldest Codex CLI that has every key of the boundary (`features.view_image` is the newest,
/// from 0.147.0; found by bisecting the release tags, so an upper bound of the true minimum).
pub const MIN_VERSION: Version = Version {
    major: 0,
    minor: 147,
    patch: 0,
};

/// How long `codex --version` gets.
pub const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// How long `codex login status` gets.
pub const LOGIN_TIMEOUT: Duration = Duration::from_secs(10);

/// What the user does when the CLI is not signed in.
const SIGN_IN_HINT: &str = "Run codex login in a terminal, then check again.";

/// What the hint of a ready Codex adds: a chat run cannot leave out the user's own instructions
/// (`$CODEX_HOME/AGENTS.md`), because Codex has no switch for that (ADR 0011).
const GLOBAL_INSTRUCTIONS_NOTE: &str =
    " Your global Codex instructions (AGENTS.md in your Codex home) also apply to chat runs.";

/// The environment variable `codex exec` reads an API key from.
const API_KEY_VARIABLE: &str = "CODEX_API_KEY";

/// What `codex login status` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignIn {
    /// Exit status 0.
    SignedIn,
    /// Exit status 1: not logged in (or the status could not be read).
    SignedOut,
}

/// The places to look for `codex`, in order and without repeats.
#[must_use]
pub fn candidates(home: Option<&Path>, path_var: Option<&OsStr>) -> Vec<PathBuf> {
    candidates_named("codex", &[".local/bin", ".npm-global/bin"], home, path_var)
}

/// The version in the output of `codex --version` (`codex-cli 0.166.0`): its last word, without a
/// pre-release or build suffix (`0.167.0-alpha.3` counts as `0.167.0`). `None` when that is not
/// three numbers.
#[must_use]
pub fn parse_version(output: &str) -> Option<Version> {
    let word = output.split_whitespace().last()?;
    let core = word.split(['-', '+']).next()?;
    Version::parse(core)
}

/// Whether `CODEX_API_KEY` is set to something. The value is not read beyond that.
#[must_use]
pub fn api_key_present() -> bool {
    std::env::var_os(API_KEY_VARIABLE).is_some_and(|value| !value.is_empty())
}

/// What a failed probe said on standard error, as a suffix for its error message: its first
/// line, or nothing.
fn said(stderr: &str) -> String {
    stderr
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map_or_else(String::new, |line| {
            format!(": {}", line.chars().take(200).collect::<String>())
        })
}

/// The output of `<program> --version`, run with the `PATH` of a chat run and at most `timeout`.
///
/// # Errors
///
/// The error of starting or bounding the process, or one that says the exit status was not
/// success.
pub fn probe_version(
    program: &Path,
    path_var: Option<&OsStr>,
    timeout: Duration,
) -> io::Result<String> {
    let answer = run_bounded(
        program,
        &["--version"],
        &search_path(program, path_var),
        timeout,
    )?;
    if answer.status.success() {
        Ok(answer.stdout)
    } else {
        Err(io::Error::other(format!(
            "`--version` exited with {}{}",
            answer.status,
            said(&answer.stderr)
        )))
    }
}

/// What `<program> login status` says, run with the `PATH` of a chat run and at most `timeout`.
///
/// # Errors
///
/// The error of starting or bounding the process, or one that names an exit status other than 0
/// and 1 (a CLI that does not know `login status`).
pub fn probe_login(
    program: &Path,
    path_var: Option<&OsStr>,
    timeout: Duration,
) -> io::Result<SignIn> {
    let answer = run_bounded(
        program,
        &["login", "status"],
        &search_path(program, path_var),
        timeout,
    )?;
    match answer.status.code() {
        Some(0) => Ok(SignIn::SignedIn),
        Some(1) => Ok(SignIn::SignedOut),
        _ => Err(io::Error::other(format!(
            "`login status` exited with {}{}",
            answer.status,
            said(&answer.stderr)
        ))),
    }
}

/// What the window shows about Codex, from the answers of the probes. `exists` and the two
/// probes are injected so that tests need no CLI; `api_key` says whether `CODEX_API_KEY` is set.
///
/// A CLI that does not answer a probe (or answers it in a way this application does not know)
/// is [`Availability::Unsupported`], with the reason in the hint: a signed-out CLI and a CLI
/// that could not be asked are different things to fix.
pub fn detect_in(
    places: &[PathBuf],
    exists: impl Fn(&Path) -> bool,
    version: impl Fn(&Path) -> io::Result<String>,
    login: impl Fn(&Path) -> io::Result<SignIn>,
    api_key: bool,
) -> ProviderStatus {
    let status =
        |availability, version: Option<String>, path: Option<&Path>, hint: String| ProviderStatus {
            id: ProviderId::Codex,
            name: NAME.to_owned(),
            availability,
            version,
            path: path.map(|path| path.display().to_string()),
            hint,
            beta: true,
            min_version: MIN_VERSION.to_string(),
            located: None,
        };
    let located = locate_named_in(places, MIN_VERSION, exists, |path| {
        // `Version::parse` reads the first word; Codex prints its version last.
        let output = version(path)?;
        let word = parse_version(&output).map(|version| version.to_string());
        Ok(word.unwrap_or(output))
    });
    let found = match located {
        Ok(found) => found,
        Err(LocateError::NotFound { .. }) => {
            return status(
                Availability::NotFound,
                None,
                None,
                "Install Codex CLI (`brew install --cask codex` or `npm install -g @openai/codex`), check that `codex` runs in a terminal, then check again."
                    .to_owned(),
            );
        }
        Err(LocateError::TooOld {
            path,
            found,
            required,
        }) => {
            return status(
                Availability::TooOld,
                Some(found.to_string()),
                Some(&path),
                format!(
                    "Codex CLI {found} is older than {required}. Update it (`brew upgrade --cask codex` or `npm install -g @openai/codex`), then check again."
                ),
            );
        }
        Err(LocateError::Unusable { path, reason }) => {
            return status(
                Availability::Unsupported,
                None,
                Some(&path),
                format!(
                    "{} does not report a Codex CLI version ({reason}). Reinstall Codex CLI, then check again.",
                    path.display()
                ),
            );
        }
    };
    let ready = |note: &str| ProviderStatus {
        located: Some(found.clone()),
        ..status(
            Availability::Ready,
            Some(found.version.to_string()),
            Some(&found.path),
            format!(
                "Codex CLI {}. {note}{GLOBAL_INSTRUCTIONS_NOTE}",
                found.version
            ),
        )
    };
    match login(&found.path) {
        Ok(SignIn::SignedIn) => ready("Signed in."),
        Ok(SignIn::SignedOut) if api_key => ready(
            "Not signed in with `codex login`, but CODEX_API_KEY is set and Codex CLI uses it for chat runs.",
        ),
        Ok(SignIn::SignedOut) => status(
            Availability::NotSignedIn,
            Some(found.version.to_string()),
            Some(&found.path),
            SIGN_IN_HINT.to_owned(),
        ),
        Err(error) => status(
            Availability::Unsupported,
            Some(found.version.to_string()),
            Some(&found.path),
            format!(
                "Cannot tell whether Codex CLI is signed in ({error}). Run `codex login status` in a terminal, then check again."
            ),
        ),
    }
}

/// [`detect_in`] with the real probes, for the CLI at `places`. Runs processes (at most
/// [`VERSION_TIMEOUT`] and [`LOGIN_TIMEOUT`]), so call it off the UI thread.
#[must_use]
pub fn detect_places(places: &[PathBuf]) -> ProviderStatus {
    let path_var: Option<OsString> = std::env::var_os("PATH");
    detect_in(
        places,
        Path::is_file,
        |path| probe_version(path, path_var.as_deref(), VERSION_TIMEOUT),
        |path| probe_login(path, path_var.as_deref(), LOGIN_TIMEOUT),
        api_key_present(),
    )
}

/// [`detect_places`] over the usual places on this machine.
#[must_use]
pub fn detect() -> ProviderStatus {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let path_var = std::env::var_os("PATH");
    detect_places(&candidates(home.as_deref(), path_var.as_deref()))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::Instant;

    use super::*;

    fn places() -> Vec<PathBuf> {
        vec![PathBuf::from("/x/codex")]
    }

    fn ok(output: &'static str) -> impl Fn(&Path) -> io::Result<String> {
        move |_| Ok(output.to_owned())
    }

    fn signed(state: SignIn) -> impl Fn(&Path) -> io::Result<SignIn> {
        move |_| Ok(state)
    }

    fn detect_fake(version: &'static str, state: SignIn, api_key: bool) -> ProviderStatus {
        detect_in(&places(), |_| true, ok(version), signed(state), api_key)
    }

    #[test]
    fn the_version_is_the_last_word_of_the_output() {
        let expect = |output: &str| parse_version(output).map(|version| version.to_string());
        assert_eq!(expect("codex-cli 0.166.0\n").as_deref(), Some("0.166.0"));
        assert_eq!(expect("0.166.0").as_deref(), Some("0.166.0"));
        assert_eq!(
            expect("codex-cli 0.167.0-alpha.3").as_deref(),
            Some("0.167.0")
        );
        assert_eq!(
            expect("codex-cli 0.167.0+build5").as_deref(),
            Some("0.167.0")
        );
        assert_eq!(expect("codex-cli"), None);
        assert_eq!(expect("codex-cli 0.166"), None);
        assert_eq!(expect("codex-cli 0.166.0.1"), None);
        assert_eq!(expect(""), None);
        assert_eq!(expect("hello"), None);
    }

    #[test]
    fn the_search_order_is_path_then_the_installers_places_without_repeats() {
        let places = candidates(
            Some(Path::new("/Users/a")),
            Some(OsStr::new("/opt/homebrew/bin:/x/bin")),
        );
        assert_eq!(
            places,
            [
                "/opt/homebrew/bin/codex",
                "/x/bin/codex",
                "/Users/a/.local/bin/codex",
                "/Users/a/.npm-global/bin/codex",
                "/usr/local/bin/codex",
            ]
            .map(PathBuf::from)
        );
        assert!(
            !places
                .iter()
                .any(|place| place.to_string_lossy().contains(".codex")),
            "no `~/.codex/bin`: no document names it"
        );
    }

    #[test]
    fn a_signed_in_codex_is_ready_with_its_installation_and_is_beta() {
        let status = detect_fake("codex-cli 0.166.0\n", SignIn::SignedIn, false);
        assert_eq!(status.availability, Availability::Ready);
        assert_eq!(status.id, ProviderId::Codex);
        assert_eq!(status.name, "Codex CLI");
        assert!(status.beta);
        assert_eq!(status.version.as_deref(), Some("0.166.0"));
        assert_eq!(status.path.as_deref(), Some("/x/codex"));
        assert_eq!(status.min_version, "0.147.0");
        assert!(status.hint.contains("0.166.0"), "{}", status.hint);
        assert!(
            status.hint.contains("global Codex instructions"),
            "a ready Codex says that the user's own instructions apply: {}",
            status.hint
        );
        let located = status.located.expect("an installation");
        assert_eq!(located.path, Path::new("/x/codex"));
        assert_eq!(located.version.to_string(), "0.166.0");
    }

    #[test]
    fn a_missing_codex_is_not_found_and_says_how_to_install_it() {
        let status = detect_in(
            &places(),
            |_| false,
            ok(""),
            signed(SignIn::SignedIn),
            false,
        );
        assert_eq!(status.availability, Availability::NotFound);
        assert_eq!((status.version, status.path), (None, None));
        assert!(status.located.is_none());
        assert!(status.hint.contains("Install Codex CLI"), "{}", status.hint);
        assert!(status.beta);
    }

    #[test]
    fn an_old_codex_is_too_old_and_is_never_asked_about_sign_in() {
        let asked = RefCell::new(false);
        let status = detect_in(
            &places(),
            |_| true,
            ok("codex-cli 0.146.9"),
            |_| {
                *asked.borrow_mut() = true;
                Ok(SignIn::SignedIn)
            },
            false,
        );
        assert_eq!(status.availability, Availability::TooOld);
        assert_eq!(status.version.as_deref(), Some("0.146.9"));
        assert_eq!(status.path.as_deref(), Some("/x/codex"));
        assert!(status.located.is_none(), "an old Codex is never run");
        assert!(status.hint.contains("0.147.0"), "{}", status.hint);
        assert!(!*asked.borrow());
        // The floor itself is fine.
        assert_eq!(
            detect_fake("codex-cli 0.147.0", SignIn::SignedIn, false).availability,
            Availability::Ready
        );
    }

    #[test]
    fn a_codex_without_a_version_is_unsupported_with_the_reason() {
        let garbled = detect_fake("hello", SignIn::SignedIn, false);
        let broken = detect_in(
            &places(),
            |_| true,
            |_| Err(io::Error::other("boom")),
            signed(SignIn::SignedIn),
            false,
        );
        for (status, reason) in [(garbled, "hello"), (broken, "boom")] {
            assert_eq!(status.availability, Availability::Unsupported);
            assert_eq!(status.path.as_deref(), Some("/x/codex"));
            assert!(status.hint.contains(reason), "{}", status.hint);
            assert!(status.located.is_none());
        }
    }

    #[test]
    fn a_signed_out_codex_is_not_signed_in_with_the_login_hint() {
        let status = detect_fake("codex-cli 0.166.0", SignIn::SignedOut, false);
        assert_eq!(status.availability, Availability::NotSignedIn);
        assert_eq!(
            status.hint,
            "Run codex login in a terminal, then check again."
        );
        assert_eq!(status.version.as_deref(), Some("0.166.0"));
        assert_eq!(status.path.as_deref(), Some("/x/codex"));
        assert!(status.located.is_none(), "a signed-out Codex is never run");
    }

    #[test]
    fn a_signed_out_codex_with_an_api_key_in_the_environment_is_ready() {
        let status = detect_fake("codex-cli 0.166.0", SignIn::SignedOut, true);
        assert_eq!(status.availability, Availability::Ready);
        assert!(status.located.is_some());
        assert!(status.hint.contains("CODEX_API_KEY"), "{}", status.hint);
    }

    #[test]
    fn a_login_probe_that_fails_is_unsupported_not_signed_out() {
        for error in [
            io::Error::new(io::ErrorKind::TimedOut, "did not answer within 10 s"),
            io::Error::other("`login status` exited with exit status: 2"),
        ] {
            let message = error.to_string();
            let status = detect_in(
                &places(),
                |_| true,
                ok("codex-cli 0.166.0"),
                move |_| Err(io::Error::new(error.kind(), message.clone())),
                // The key is no help when the CLI cannot be asked.
                true,
            );
            assert_eq!(status.availability, Availability::Unsupported);
            assert_eq!(status.version.as_deref(), Some("0.166.0"));
            assert!(status.located.is_none());
            assert!(status.hint.contains("login status"), "{}", status.hint);
        }
    }

    #[cfg(unix)]
    mod processes {
        use super::*;
        /// A fake `codex`: a script that answers the two questions.
        fn fake(body: &str) -> (tempfile::TempDir, PathBuf) {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("codex");
            fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            (directory, path)
        }

        fn detect_script(path: &Path, timeout: Duration, api_key: bool) -> ProviderStatus {
            detect_in(
                &[path.to_path_buf()],
                Path::is_file,
                |path| probe_version(path, None, timeout),
                |path| probe_login(path, None, timeout),
                api_key,
            )
        }

        const ANSWERS: &str = r#"case "$1" in
  --version) echo "codex-cli 0.166.0" ;;
  login) echo "$LOGIN_TEXT" >&2; exit "${LOGIN_EXIT:-0}" ;;
  *) echo "unexpected: $*" >&2; exit 9 ;;
esac"#;

        #[test]
        fn the_real_probes_run_the_cli_and_read_its_answers() {
            let (_dir, path) = fake(&ANSWERS.replace("$LOGIN_TEXT", "Logged in using ChatGPT"));
            let status = detect_script(&path, Duration::from_secs(10), false);
            assert_eq!(status.availability, Availability::Ready, "{}", status.hint);
            assert_eq!(status.version.as_deref(), Some("0.166.0"));
            assert_eq!(status.path.as_deref(), Some(path.to_str().unwrap()));
        }

        #[test]
        fn the_real_probe_of_a_signed_out_cli_reads_exit_one_from_stderr_text() {
            let body = ANSWERS
                .replace("$LOGIN_TEXT", "Not logged in")
                .replace("${LOGIN_EXIT:-0}", "1");
            let (_dir, path) = fake(&body);
            let signed_out = detect_script(&path, Duration::from_secs(10), false);
            assert_eq!(signed_out.availability, Availability::NotSignedIn);
            let with_key = detect_script(&path, Duration::from_secs(10), true);
            assert_eq!(with_key.availability, Availability::Ready);
        }

        #[test]
        fn the_real_probe_of_another_exit_status_is_unsupported() {
            let body = ANSWERS
                .replace("$LOGIN_TEXT", "usage error")
                .replace("${LOGIN_EXIT:-0}", "2");
            let (_dir, path) = fake(&body);
            let status = detect_script(&path, Duration::from_secs(10), false);
            assert_eq!(status.availability, Availability::Unsupported);
            assert!(status.hint.contains("exit"), "{}", status.hint);
        }

        #[test]
        fn a_cli_that_hangs_on_the_version_or_on_login_is_given_up_at_the_deadline() {
            let hang_version = "sleep 30";
            let hang_login =
                r#"case "$1" in --version) echo "codex-cli 0.166.0" ;; *) sleep 30 ;; esac"#;
            for (body, availability) in [
                (hang_version, Availability::Unsupported),
                (hang_login, Availability::Unsupported),
            ] {
                let (_dir, path) = fake(body);
                let started = Instant::now();
                let status = detect_script(&path, Duration::from_millis(400), true);
                assert!(
                    started.elapsed() < Duration::from_secs(5),
                    "{:?}",
                    started.elapsed()
                );
                assert_eq!(status.availability, availability, "{}", status.hint);
                assert!(status.hint.contains("did not answer"), "{}", status.hint);
                assert!(status.located.is_none());
            }
        }

        #[test]
        fn a_failing_version_command_is_unsupported() {
            let (_dir, path) = fake("echo broken >&2; exit 4");
            let status = detect_script(&path, Duration::from_secs(10), false);
            assert_eq!(status.availability, Availability::Unsupported);
            assert!(status.hint.contains("--version"), "{}", status.hint);
        }

        #[test]
        fn the_cli_gets_the_minimal_search_path_and_a_closed_stdin() {
            let (dir, path) = fake(
                r#"echo "$PATH" > "$(dirname "$0")/path.txt"
cat > /dev/null
case "$1" in --version) echo "codex-cli 0.166.0" ;; *) exit 0 ;; esac"#,
            );
            let status = detect_script(&path, Duration::from_secs(10), false);
            assert_eq!(status.availability, Availability::Ready);
            let seen = fs::read_to_string(dir.path().join("path.txt")).unwrap();
            let first = seen.trim().split(':').next().unwrap();
            assert_eq!(
                Path::new(first),
                dir.path(),
                "the directory of the CLI comes first"
            );
            assert!(seen.contains("/usr/bin"), "{seen}");
        }
    }
}
