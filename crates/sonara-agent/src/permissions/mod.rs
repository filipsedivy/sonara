//! Agent safety (`sonara.agent.permissions`): the file sandbox ([`PathPolicy`]) and the
//! confirmation tokens of destructive actions ([`Confirmations`]).

mod confirmations;
mod paths;

pub use confirmations::{
    Confirmations, MonotonicClock, TOKEN_TTL, TokenSource, random_tokens, sequential_tokens,
};
pub use paths::{Access, ENV_ALLOWED, PathPolicy};
