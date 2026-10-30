//! Codex CLI as a chat provider: `codex exec --json` with every tool source but the Sonara MCP
//! server switched off, run read-only and ephemeral.
//!
//! - [`locate`]: finds `codex`, checks its version and asks `codex login status`.
//! - [`invocation`]: the command line, which is the Codex column of the boundary of ADR 0011.
//! - [`stream`]: the parser of the `--json` output.
//! - [`tools`]: the Sonara tools the run may call.
//!
//! Codex handles only the first `SIGINT` (it interrupts the turn, writes no `turn.*` event and
//! exits 1) and has no `SIGTERM` handler, so `SIGTERM` simply ends it. A stop is `SIGINT`, then
//! [`STOP_POLICY`]'s 4 s, then `SIGTERM` to the process group (which also ends `sonara mcp`),
//! then 2 s, then `SIGKILL` to the group. Whether the task is cancelled is the host's stop flag,
//! never the exit.
//!
//! **Not closed:** the user's global `$CODEX_HOME/AGENTS.md` still reaches the model, because no
//! switch of Codex 0.166.0 leaves it out (see [`invocation`]). The availability hint says so.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::chat::provider::{Provider, ProviderStatus, Turn};
use crate::chat::task::ProviderId;
use crate::composer::{Invocation, LineParser, Located, StopPolicy, Workspace};

pub mod invocation;
pub mod locate;
pub mod stream;
pub mod tools;

pub use stream::CodexParser;

/// How a Codex run is stopped: `SIGINT`, then 4 s for Codex to interrupt the turn and exit,
/// then `SIGTERM` to the process group (Codex has no handler, so it ends the process and its
/// `sonara mcp`), and `SIGKILL` 2 s after that. The graces are constants chosen for a quick
/// answer to a stop button, not measured on a real turn.
pub const STOP_POLICY: StopPolicy = StopPolicy {
    interrupt_grace: Duration::from_secs(4),
    terminate_grace: Duration::from_secs(2),
};

/// Codex CLI, found on this machine, or at one path.
#[derive(Debug, Clone, Default)]
pub struct CodexProvider {
    /// The one installation to use instead of searching, for tests and tools.
    installation: Option<PathBuf>,
}

impl CodexProvider {
    /// The provider that searches for `codex` in the usual places.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The provider for the `codex` at `path`, without a search: detection asks that one
    /// executable and no other.
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            installation: Some(path.into()),
        }
    }
}

impl Provider for CodexProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Codex
    }

    fn name(&self) -> &str {
        locate::NAME
    }

    fn detect(&self) -> ProviderStatus {
        match &self.installation {
            Some(path) => locate::detect_places(std::slice::from_ref(path)),
            None => locate::detect(),
        }
    }

    fn build(
        &self,
        located: &Located,
        sonara_cli: &Path,
        workspace: &Workspace,
        turn: &Turn,
    ) -> Invocation {
        invocation::build(&located.path, sonara_cli, workspace, turn)
    }

    fn parser(&self) -> Box<dyn LineParser> {
        Box::new(CodexParser::new())
    }

    fn stop_policy(&self) -> StopPolicy {
        STOP_POLICY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::provider::Availability;
    use crate::composer::Event;

    #[test]
    fn the_provider_describes_itself() {
        let provider = CodexProvider::new();
        assert_eq!(provider.id(), ProviderId::Codex);
        assert_eq!(provider.name(), "Codex CLI");
        assert_eq!(provider.stop_policy(), STOP_POLICY);
        assert_eq!(STOP_POLICY.interrupt_grace, Duration::from_secs(4));
        assert_eq!(STOP_POLICY.terminate_grace, Duration::from_secs(2));
        // Not `detect()`: that would run the `codex` of the machine, which reads `~/.codex`.
        let mut parser = provider.parser();
        assert_eq!(
            parser.parse("not json"),
            [Event::Output {
                line: "not json".into()
            }]
        );
    }

    #[test]
    fn a_provider_at_a_missing_path_is_not_found_without_running_anything() {
        let status = CodexProvider::at("/nonexistent/dir/codex").detect();
        assert_eq!(status.availability, Availability::NotFound);
        assert_eq!(status.id, ProviderId::Codex);
        assert!(status.beta);
        assert!(status.located.is_none());
    }

    #[test]
    fn the_stop_graces_end_a_run_well_before_the_claude_ones() {
        assert!(STOP_POLICY.interrupt_grace < StopPolicy::CLAUDE.interrupt_grace);
        assert!(STOP_POLICY.terminate_grace < StopPolicy::CLAUDE.terminate_grace);
    }
}
