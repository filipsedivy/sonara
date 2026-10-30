//! Claude Code as a chat provider: `claude -p` with the composer's command line, a session that
//! is not kept on disk, and the chat's system prompt on top of the composer's.

use std::path::Path;

use crate::chat::provider::{Availability, Provider, ProviderStatus, SYSTEM_PROMPT_ADDENDUM, Turn};
use crate::chat::task::ProviderId;
use crate::composer::invocation::BuildOptions;
use crate::composer::locate::MIN_VERSION;
use crate::composer::{
    ClaudeParser, Invocation, LineParser, LocateError, Located, StopPolicy, Workspace, build_with,
    locate,
};

/// The name of the provider in the window.
const NAME: &str = "Claude Code";

/// Claude Code, found through [`locate`]. The same executable and boundaries as the composer.
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

impl Provider for ClaudeProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Claude
    }

    fn name(&self) -> &str {
        NAME
    }

    fn detect(&self) -> ProviderStatus {
        Self::status(locate())
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
