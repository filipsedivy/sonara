//! The providers of the chat, one module per agent CLI.

use std::sync::Arc;

use super::provider::Provider;

pub mod claude;

pub use claude::ClaudeProvider;

/// Every provider the chat offers, in the order the window lists them.
#[must_use]
pub fn default_providers() -> Vec<Arc<dyn Provider>> {
    vec![Arc::new(ClaudeProvider)]
}
