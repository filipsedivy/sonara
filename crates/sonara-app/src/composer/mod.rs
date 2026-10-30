//! The AI composer: a harness that runs a headless Claude Code (`claude -p`) as an agentic
//! composer inside the open project.
//!
//! The agent edits the project that the window shows, through the bundled `sonara mcp` attached to
//! that window's agent session ([`invocation`] explains how the session is pinned), so every edit
//! appears on the timeline as it happens and has the window's undo history. It may search the web
//! for musical reference, and it may not write files, run programs, save or export: the user does
//! that ([`invocation::DENIED_TOOLS`]).
//!
//! - [`mod@locate`]: finds Claude Code and checks its version.
//! - [`invocation`]: the system prompt, the tool boundaries and the command line.
//! - [`stream`]: parses the `stream-json` output into [`stream::Event`]s for the panel.
//! - [`run`]: starts the process, streams the events and stops it (unix only). The parser of the
//!   output ([`LineParser`]) and the stop signals ([`StopPolicy`]) are the parts that differ
//!   between providers; the AI Chat ([`crate::chat`]) uses them for its own providers.
//!
//! Nothing here runs on a real-time thread, and no tool, schema or CLI contract changes (CTR-01):
//! the composer is a client of the existing agent API.

pub mod invocation;
pub mod locate;
#[cfg(unix)]
pub mod run;
pub mod stop;
pub mod stream;

pub use invocation::{
    BuildOptions, Invocation, PromptError, Workspace, build, build_with, check_prompt,
};
pub use locate::{ClaudeCli, LocateError, Located, locate};
#[cfg(unix)]
pub use run::{Run, RunError};
pub use stop::StopPolicy;
pub use stream::{ClaudeParser, Event, LineParser};
