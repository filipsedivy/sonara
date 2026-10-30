//! The providers of the chat, one module per agent CLI.

use std::sync::Arc;

use super::provider::Provider;

pub mod claude;
pub mod codex;

pub use claude::ClaudeProvider;
pub use codex::CodexParser;

/// Every provider the chat offers, in the order the window lists them.
#[must_use]
pub fn default_providers() -> Vec<Arc<dyn Provider>> {
    vec![Arc::new(ClaudeProvider)]
}
