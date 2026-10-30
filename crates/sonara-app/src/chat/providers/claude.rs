//! Claude Code as a chat provider: `claude -p` with the composer's command line, a session that
//! is not kept on disk, and the chat's system prompt on top of the composer's.

use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::time::Duration;

use crate::chat::provider::{Availability, Provider, ProviderStatus, SYSTEM_PROMPT_ADDENDUM, Turn};
use crate::chat::providers::probe::run_bounded;
use crate::chat::task::ProviderId;
use crate::composer::invocation::{BuildOptions, search_path};
use crate::composer::locate::MIN_VERSION;
use crate::composer::{
    ClaudeParser, Invocation, LineParser, LocateError, Located, StopPolicy, Workspace, build_with,
    locate,
};

/// The name of the provider in the window.
const NAME: &str = "Claude Code";

/// The flag that keeps a chat run from leaving a session on disk. The chat passes it to every run,
/// so a Claude Code that does not list it in `--help` cannot be used.
pub const STATELESS_FLAG: &str = "--no-session-persistence";

/// How long `claude --help` gets.
pub const HELP_TIMEOUT: Duration = Duration::from_secs(15);

/// Claude Code, found through [`locate()`]. The same executable and boundaries as the composer.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaudeProvider;

impl ClaudeProvider {
    /// What `located`, the outcome of a search for Claude Code, means for the window.
    ///
    /// Whether the user is signed in is not probed (that would mean reading credentials or
    /// running a request), so a signed-out Claude Code shows as ready and the first message
    /// fails with Claude Code's own message. A `claude` that does not report a version at all
    /// is [`Availability::Unsupported`].
    #[must_use]
    pub fn status(located: Result<Located, LocateError>) -> ProviderStatus {
        let status = |availability, version: Option<String>, path: Option<&Path>, hint: String| {
            ProviderStatus {
                id: ProviderId::Claude,
                name: NAME.to_owned(),
                availability,
                version,
                path: path.map(|path| path.display().to_string()),
                hint,
                beta: false,
                min_version: MIN_VERSION.to_string(),
                located: None,
            }
        };
        match located {
            Ok(found) => ProviderStatus {
                located: Some(found.clone()),
                ..status(
                    Availability::Ready,
                    Some(found.version.to_string()),
                    Some(&found.path),
                    format!(
                        "Claude Code {}. If a message fails because you are signed out, run `claude` once in a terminal and log in.",
                        found.version
                    ),
                )
            },
            Err(LocateError::NotFound { .. }) => status(
                Availability::NotFound,
                None,
                None,
                "Install Claude Code, check that `claude` runs in a terminal, then check again."
                    .to_owned(),
            ),
            Err(LocateError::TooOld {
                path,
                found,
                required,
            }) => status(
                Availability::TooOld,
                Some(found.to_string()),
                Some(&path),
                format!(
                    "Claude Code {found} is older than {required}. Update it (`claude update`), then check again."
                ),
            ),
            Err(LocateError::Unusable { path, reason }) => status(
                Availability::Unsupported,
                None,
                Some(&path),
                format!(
                    "{} does not report a Claude Code version ({reason}). Reinstall Claude Code, then check again.",
                    path.display()
                ),
            ),
        }
    }
}

/// The help text of `<program> --help`, run with the `PATH` of a chat run and at most `timeout`.
///
/// # Errors
///
/// The error of starting or bounding the process, or one that says the exit status was not
/// success.
pub fn probe_help(
    program: &Path,
    path_var: Option<&OsStr>,
    timeout: Duration,
) -> io::Result<String> {
    let answer = run_bounded(
        program,
        &["--help"],
        &search_path(program, path_var),
        timeout,
    )?;
    if answer.status.success() {
        Ok(answer.stdout)
    } else {
        Err(io::Error::other(format!(
            "`--help` exited with {}",
            answer.status
        )))
    }
}

impl ClaudeProvider {
    /// [`ClaudeProvider::status`], and for a Claude Code that is ready, a check that `help` (the
    /// output of `claude --help`) lists [`STATELESS_FLAG`].
    ///
    /// The minimum version of the composer says which release has the flags of the composer's
    /// run; the chat adds this flag to it, and a Claude Code that does not list it would refuse
    /// every chat run. So that one is [`Availability::TooOld`], with a hint that names the flag.
    /// A Claude Code that cannot be asked is [`Availability::Unsupported`], with the reason: the
    /// flag cannot be verified. `help` is not called for a Claude Code that is not ready anyway.
    pub fn status_checked(
        located: Result<Located, LocateError>,
        help: impl FnOnce(&Located) -> io::Result<String>,
    ) -> ProviderStatus {
        let status = Self::status(located);
        let Some(found) = status.located.clone() else {
            return status;
        };
        let refuse = |availability, hint: String| ProviderStatus {
            availability,
            hint,
            located: None,
            ..status.clone()
        };
        match help(&found) {
            Ok(text) if text.contains(STATELESS_FLAG) => status,
            Ok(_) => refuse(
                Availability::TooOld,
                format!(
                    "Claude Code {} does not list {STATELESS_FLAG}, which the chat needs so that no session is kept. Update it (`claude update`), then check again.",
                    found.version
                ),
            ),
            Err(error) => refuse(
                Availability::Unsupported,
                format!(
                    "Cannot check that Claude Code {} supports {STATELESS_FLAG} ({error}). Run `claude --help` in a terminal, then check again.",
                    found.version
                ),
            ),
        }
    }
}

impl Provider for ClaudeProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Claude
    }

    fn name(&self) -> &str {
        NAME
    }

    fn detect(&self) -> ProviderStatus {
        let path_var = std::env::var_os("PATH");
        Self::status_checked(locate(), |found| {
            probe_help(&found.path, path_var.as_deref(), HELP_TIMEOUT)
        })
    }

    fn build(
        &self,
        located: &Located,
        sonara_cli: &Path,
        workspace: &Workspace,
        turn: &Turn,
    ) -> Invocation {
        // The deny list is the composer's (`DENIED_TOOLS`); `boundary` has the same Sonara
        // tools by bare name, and a test there keeps the two equal.
        build_with(
            &located.path,
            sonara_cli,
            workspace,
            &turn.stdin_text(),
            &BuildOptions {
                persist_session: false,
                system_prompt_addendum: Some(SYSTEM_PROMPT_ADDENDUM),
            },
        )
    }

    fn parser(&self) -> Box<dyn LineParser> {
        Box::new(ClaudeParser)
    }

    fn stop_policy(&self) -> StopPolicy {
        StopPolicy::CLAUDE
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::path::PathBuf;

    use super::*;
    use crate::composer::invocation::{DENIED_TOOLS, SYSTEM_PROMPT};
    use crate::composer::locate::{candidates, locate_in};

    fn workspace() -> (tempfile::TempDir, Workspace) {
        let source = tempfile::tempdir().unwrap();
        let file = source.path().join("session-7.json");
        fs::write(&file, "{}").unwrap();
        let workspace = Workspace::create(&file).unwrap();
        (source, workspace)
    }

    fn found(version: &str) -> Result<Located, LocateError> {
        let places = vec![PathBuf::from("/x/claude")];
        let output = format!("{version} (Claude Code)");
        locate_in(&places, |_| true, move |_| Ok(output.clone()))
    }

    fn arg_after<'a>(invocation: &'a Invocation, flag: &str) -> &'a str {
        let position = invocation.args.iter().position(|arg| arg == flag).unwrap();
        invocation.args[position + 1].to_str().unwrap()
    }

    #[test]
    fn a_recent_claude_is_ready_with_its_installation() {
        let status = ClaudeProvider::status(found("2.1.312"));
        assert_eq!(status.availability, Availability::Ready);
        assert_eq!(status.id, ProviderId::Claude);
        assert_eq!(status.version.as_deref(), Some("2.1.312"));
        assert_eq!(status.path.as_deref(), Some("/x/claude"));
        assert_eq!(status.min_version, "2.1.259");
        assert!(status.hint.contains("2.1.312"));
        let located = status.located.unwrap();
        assert_eq!(located.path, Path::new("/x/claude"));
        assert_eq!(located.version.to_string(), "2.1.312");
    }

    #[test]
    fn a_missing_claude_is_not_found_and_says_how_to_fix_it() {
        let places = candidates(None, None);
        let error = locate_in(&places, |_| false, |_| Ok(String::new())).unwrap_err();
        let status = ClaudeProvider::status(Err(error));
        assert_eq!(status.availability, Availability::NotFound);
        assert_eq!((status.version, status.path), (None, None));
        assert!(status.located.is_none());
        assert!(status.hint.contains("Install Claude Code"));
    }

    #[test]
    fn an_old_claude_is_too_old_with_its_version_and_path() {
        let status = ClaudeProvider::status(found("2.1.100"));
        assert_eq!(status.availability, Availability::TooOld);
        assert_eq!(status.version.as_deref(), Some("2.1.100"));
        assert_eq!(status.path.as_deref(), Some("/x/claude"));
        assert!(status.located.is_none(), "an old Claude is never run");
        assert!(status.hint.contains("2.1.259"), "{}", status.hint);
    }

    #[test]
    fn a_claude_without_a_version_is_unsupported_with_the_reason() {
        let places = vec![PathBuf::from("/x/claude")];
        let garbled = locate_in(&places, |_| true, |_| Ok("hello".to_owned())).unwrap_err();
        let broken = locate_in(&places, |_| true, |_| Err(io::Error::other("boom"))).unwrap_err();
        for (error, reason) in [(garbled, "hello"), (broken, "boom")] {
            let status = ClaudeProvider::status(Err(error));
            assert_eq!(status.availability, Availability::Unsupported);
            assert_eq!(status.path.as_deref(), Some("/x/claude"));
            assert!(status.hint.contains(reason), "{}", status.hint);
            assert!(status.located.is_none());
        }
    }

    const HELP_WITH_FLAG: &str =
        "Usage: claude [options]\n  --no-session-persistence  Disable session persistence\n";

    #[test]
    fn a_claude_that_lists_the_stateless_flag_is_ready_as_before() {
        let status =
            ClaudeProvider::status_checked(found("2.1.312"), |_| Ok(HELP_WITH_FLAG.into()));
        assert_eq!(status, ClaudeProvider::status(found("2.1.312")));
        assert_eq!(status.availability, Availability::Ready);
        assert!(status.located.is_some());
    }

    #[test]
    fn a_claude_without_the_stateless_flag_is_too_old_and_the_hint_names_the_flag() {
        let status = ClaudeProvider::status_checked(found("2.1.312"), |_| {
            Ok("Usage: claude [options]\n  --resume  Resume\n".into())
        });
        assert_eq!(status.availability, Availability::TooOld);
        assert_eq!(status.version.as_deref(), Some("2.1.312"));
        assert_eq!(status.path.as_deref(), Some("/x/claude"));
        assert!(status.located.is_none(), "such a Claude is never run");
        assert!(
            status.hint.contains("--no-session-persistence"),
            "{}",
            status.hint
        );
        assert!(status.hint.contains("claude update"), "{}", status.hint);
        assert_eq!(status.min_version, "2.1.259");
        assert!(!status.beta);
    }

    #[test]
    fn a_claude_that_cannot_be_asked_for_its_help_is_unsupported_with_the_reason() {
        let status = ClaudeProvider::status_checked(found("2.1.312"), |_| {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "did not answer within 15 s",
            ))
        });
        assert_eq!(status.availability, Availability::Unsupported);
        assert_eq!(status.version.as_deref(), Some("2.1.312"));
        assert!(status.located.is_none());
        assert!(status.hint.contains("did not answer"), "{}", status.hint);
        assert!(
            status.hint.contains("--no-session-persistence"),
            "{}",
            status.hint
        );
    }

    #[test]
    fn help_is_not_asked_of_a_claude_that_is_not_ready_anyway() {
        let missing = || {
            let places = candidates(None, None);
            locate_in(&places, |_| false, |_| Ok(String::new()))
        };
        let makers: [&dyn Fn() -> Result<Located, LocateError>; 2] =
            [&missing, &|| found("2.1.100")];
        for make in makers {
            let asked = std::cell::Cell::new(false);
            let status = ClaudeProvider::status_checked(make(), |_| {
                asked.set(true);
                Ok(HELP_WITH_FLAG.into())
            });
            assert_eq!(status, ClaudeProvider::status(make()));
            assert_ne!(status.availability, Availability::Ready);
            assert!(!asked.get());
        }
    }

    #[cfg(unix)]
    mod processes {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;
        use std::time::Instant;

        use super::*;

        fn fake(body: &str) -> (tempfile::TempDir, PathBuf) {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("claude");
            fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            (directory, path)
        }

        #[test]
        fn the_real_probe_reads_the_help_of_the_cli() {
            let (_dir, path) = fake(
                r#"case "$1" in --version) echo "2.1.312 (Claude Code)" ;; --help) printf 'Usage\n  --no-session-persistence  x\n' ;; esac"#,
            );
            let help = probe_help(&path, None, Duration::from_secs(10)).unwrap();
            assert!(help.contains(STATELESS_FLAG));
            let status = ClaudeProvider::status_checked(
                Ok(Located {
                    path: path.clone(),
                    version: "2.1.312".parse_version(),
                }),
                |found| probe_help(&found.path, None, Duration::from_secs(10)),
            );
            assert_eq!(status.availability, Availability::Ready);
        }

        #[test]
        fn a_cli_whose_help_lacks_the_flag_or_hangs_or_fails_is_not_ready() {
            let cases = [
                ("printf 'Usage\\n  --resume  x\\n'", Availability::TooOld),
                ("sleep 30", Availability::Unsupported),
                ("echo broken >&2; exit 2", Availability::Unsupported),
            ];
            for (body, expected) in cases {
                let (_dir, path) = fake(body);
                let started = Instant::now();
                let status = ClaudeProvider::status_checked(
                    Ok(Located {
                        path: path.clone(),
                        version: "2.1.312".parse_version(),
                    }),
                    |found| probe_help(&found.path, None, Duration::from_millis(400)),
                );
                assert_eq!(status.availability, expected, "{body}: {}", status.hint);
                assert!(status.located.is_none());
                assert!(started.elapsed() < Duration::from_secs(5));
            }
        }

        trait ParseVersion {
            fn parse_version(&self) -> crate::composer::locate::Version;
        }

        impl ParseVersion for &str {
            fn parse_version(&self) -> crate::composer::locate::Version {
                crate::composer::locate::Version::parse(self).unwrap()
            }
        }
    }

    #[test]
    fn the_provider_describes_itself() {
        let provider = ClaudeProvider;
        assert_eq!(provider.id(), ProviderId::Claude);
        assert_eq!(provider.name(), "Claude Code");
        assert_eq!(provider.stop_policy(), StopPolicy::CLAUDE);
        let mut parser = provider.parser();
        assert_eq!(
            parser.parse("not json"),
            [crate::composer::Event::Output {
                line: "not json".into()
            }]
        );
        // `detect` never panics, whatever this machine has.
        let status = provider.detect();
        assert_eq!(status.id, ProviderId::Claude);
        assert_eq!(
            status.located.is_some(),
            status.availability == Availability::Ready
        );
    }

    #[test]
    fn a_chat_run_is_the_composer_run_without_a_kept_session_and_with_the_chat_prompt() {
        let (_source, workspace) = workspace();
        let located = found("2.1.312").unwrap();
        let turn = Turn {
            prompt: "-- what key is this in?".into(),
            transcript: "User: hi\n\nAssistant: hello".into(),
        };
        let run = ClaudeProvider.build(&located, Path::new("/App/sonara"), &workspace, &turn);
        assert_eq!(run.program, Path::new("/x/claude"));
        assert_eq!(run.cwd, workspace.cwd());
        assert!(run.args.iter().any(|arg| arg == "--no-session-persistence"));
        let flags = run
            .args
            .iter()
            .filter(|arg| *arg == "--append-system-prompt")
            .count();
        assert_eq!(flags, 1);
        let system = arg_after(&run, "--append-system-prompt");
        assert!(system.starts_with(SYSTEM_PROMPT));
        assert!(system.ends_with(SYSTEM_PROMPT_ADDENDUM));
        assert!(system.contains("composer inside Sonara"));
        assert!(system.contains("This session is a chat"));
        assert_eq!(run.stdin, turn.stdin_text());
        assert!(
            !run.args.iter().any(|arg| arg
                .to_str()
                .is_some_and(|a| a.contains("what key") || a.contains("earlier_conversation="))),
            "the message travels on stdin"
        );
        assert_eq!(arg_after(&run, "--disallowedTools"), DENIED_TOOLS.join(","));
        assert_eq!(
            arg_after(&run, "--allowedTools"),
            "mcp__sonara,WebSearch,WebFetch"
        );
        assert_eq!(arg_after(&run, "--permission-mode"), "dontAsk");
        for flag in ["--strict-mcp-config", "-p", "--verbose"] {
            assert!(run.args.iter().any(|arg| arg == flag), "{flag}");
        }
        assert!(!run.args.iter().any(|arg| arg == "--bare"));
    }
}
