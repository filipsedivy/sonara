//! The seam between the chat and the agent CLIs: one [`Provider`] per agent (Claude Code, Codex
//! CLI), each knowing how to find its CLI, what command line to run, how to read its output and
//! how to stop it.
//!
//! The chat itself ([`super::controller`]) knows none of that: it asks a provider whether it is
//! ready, builds the run through it and feeds the run's lines to the parser the provider gives
//! back. Every provider runs under the same safety boundary: the Sonara tools through the pinned
//! `sonara mcp`, and none of [`super::boundary::DENIED_SONARA_TOOLS`].

use std::path::Path;

use serde::Serialize;

use super::task::ProviderId;
use crate::composer::{Invocation, LineParser, Located, StopPolicy, Workspace};

/// What a chat run is told on top of the composer's system prompt: this is a chat, answer
/// questions without touching the project, edit only when asked.
pub const SYSTEM_PROMPT_ADDENDUM: &str = include_str!("system_prompt_chat.md");

/// Whether a provider can run now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    /// The CLI is installed and recent enough.
    Ready,
    /// There is no such CLI.
    NotFound,
    /// The CLI is there, but not signed in.
    NotSignedIn,
    /// The CLI is older than the oldest version that has what the chat needs.
    TooOld,
    /// The CLI is there, but this application cannot use it.
    Unsupported,
}

/// What the window shows about one provider before the first message.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderStatus {
    /// The provider.
    pub id: ProviderId,
    /// Its name for the window.
    pub name: String,
    /// Whether it can run.
    pub availability: Availability,
    /// The CLI's version, when it is known.
    pub version: Option<String>,
    /// Where the CLI is, when it was found.
    pub path: Option<String>,
    /// How to fix what is wrong, or a short note when ready.
    pub hint: String,
    /// Whether the provider itself is experimental.
    pub beta: bool,
    /// The oldest CLI version the chat supports.
    pub min_version: String,
    /// The installation to run, set only when `availability` is [`Availability::Ready`]. Not
    /// part of what the window receives.
    #[serde(skip)]
    pub located: Option<Located>,
}

/// One message to the agent: what the user asks, and the earlier conversation for context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    /// The user's message, already validated by `check_prompt`.
    pub prompt: String,
    /// The earlier conversation, as [`super::replay::transcript`] builds it; empty for the
    /// first message.
    pub transcript: String,
}

impl Turn {
    /// The text the agent reads on stdin: the message alone for the first one, otherwise the
    /// transcript in an `<earlier_conversation>` block (which the chat's system prompt calls
    /// context only) followed by the new request.
    #[must_use]
    pub fn stdin_text(&self) -> String {
        if self.transcript.is_empty() {
            self.prompt.clone()
        } else {
            format!(
                "<earlier_conversation>\n{}\n</earlier_conversation>\n\nNew request from the user:\n{}",
                self.transcript, self.prompt
            )
        }
    }
}

/// An agent CLI the chat can run.
pub trait Provider: Send + Sync {
    /// Which provider this is.
    fn id(&self) -> ProviderId;

    /// Its name for the window.
    fn name(&self) -> &str;

    /// Finds the CLI and checks its version. Runs a process, so call it off the UI thread. It
    /// never reads credentials or token files.
    fn detect(&self) -> ProviderStatus;

    /// The command line of one run of `turn` in `workspace`, with the CLI of `located`.
    /// `sonara_cli` is the bundled `sonara` the agent's tools run through. The session is not
    /// persisted by the provider: the chat keeps the conversation itself.
    fn build(
        &self,
        located: &Located,
        sonara_cli: &Path,
        workspace: &Workspace,
        turn: &Turn,
    ) -> Invocation;

    /// A new parser for the output of one run.
    fn parser(&self) -> Box<dyn LineParser>;

    /// How a run of this provider is stopped.
    fn stop_policy(&self) -> StopPolicy;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::providers::codex::invocation::developer_instructions;
    use crate::composer::invocation::system_prompt;

    /// The most characters the whole system prompt of a chat run may hold: every task carries
    /// it, so it stays a fixed, tested cost.
    const PROMPT_BUDGET_CHARS: usize = 24_000;

    #[test]
    fn the_whole_chat_prompt_fits_the_budget() {
        // What both providers receive: the composer's prompt with the orchestration reference,
        // then this addendum (Codex's developer instructions are exactly that; the Claude
        // provider's test pins the same text in `--append-system-prompt`).
        let whole = format!("{}\n{SYSTEM_PROMPT_ADDENDUM}", system_prompt());
        assert_eq!(whole, developer_instructions());
        let count = whole.chars().count();
        assert!(
            count <= PROMPT_BUDGET_CHARS,
            "the chat's system prompt holds {count} characters, more than {PROMPT_BUDGET_CHARS}"
        );
    }

    #[test]
    fn the_first_message_goes_to_the_agent_as_it_is() {
        let turn = Turn {
            prompt: "make a beat".into(),
            transcript: String::new(),
        };
        assert_eq!(turn.stdin_text(), "make a beat");
    }

    #[test]
    fn earlier_messages_arrive_in_a_block_before_the_new_request() {
        let turn = Turn {
            prompt: "now add bass".into(),
            transcript: "User: make a beat\n\nAssistant: done".into(),
        };
        let text = turn.stdin_text();
        assert_eq!(
            text,
            "<earlier_conversation>\nUser: make a beat\n\nAssistant: done\n</earlier_conversation>\n\nNew request from the user:\nnow add bass"
        );
        assert!(SYSTEM_PROMPT_ADDENDUM.contains("<earlier_conversation>"));
        assert!(SYSTEM_PROMPT_ADDENDUM.contains("New request from the user:"));
    }

    #[test]
    fn the_status_serializes_for_the_window_without_the_installation() {
        let status = ProviderStatus {
            id: ProviderId::Claude,
            name: "Claude Code".into(),
            availability: Availability::NotSignedIn,
            version: Some("2.1.308".into()),
            path: Some("/bin/claude".into()),
            hint: "sign in".into(),
            beta: false,
            min_version: "2.1.259".into(),
            located: None,
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["id"], "claude");
        assert_eq!(json["availability"], "not_signed_in");
        assert_eq!(json["min_version"], "2.1.259");
        assert!(json.get("located").is_none());
    }

    #[test]
    fn the_chat_prompt_keeps_the_boundaries_and_says_what_a_chat_is() {
        // The wording flows over lines; compare without line breaks.
        let flat = SYSTEM_PROMPT_ADDENDUM
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        for needle in [
            "do not touch the project",
            "only when asked to",
            "context only",
            "cannot save, export, open or create",
            "language of the person",
            "what you did",
        ] {
            assert!(flat.contains(needle), "{needle}");
        }
    }
}
