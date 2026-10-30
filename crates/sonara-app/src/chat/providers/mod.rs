//! The providers of the chat, one module per agent CLI.

use std::sync::Arc;

use super::provider::Provider;

pub mod claude;
pub mod codex;
pub(crate) mod probe;

pub use claude::ClaudeProvider;
pub use codex::{CodexParser, CodexProvider};

/// Every provider the chat offers, in the order the window lists them.
#[must_use]
pub fn default_providers() -> Vec<Arc<dyn Provider>> {
    vec![Arc::new(ClaudeProvider), Arc::new(CodexProvider::new())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::task::ProviderId;

    #[test]
    fn the_window_lists_claude_first_and_codex_second() {
        // `detect()` is not called: it runs the CLIs of this machine.
        let ids: Vec<ProviderId> = default_providers()
            .iter()
            .map(|provider| provider.id())
            .collect();
        assert_eq!(ids, [ProviderId::Claude, ProviderId::Codex]);
    }
}
