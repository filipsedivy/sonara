//! Codex CLI as a chat provider: `codex exec --json` with every tool source but the Sonara MCP
//! server switched off, run read-only and ephemeral.

pub mod stream;

pub use stream::CodexParser;
