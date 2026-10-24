//! Two-phase confirmation of destructive actions (`Confirmations` of
//! `sonara.agent.permissions`): a destructive call without a valid token fails with
//! `CONFIRMATION_REQUIRED` and a single-use token bound to that exact call; repeating the
//! identical call with the token lets it through.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Map, Value};
use sonara_core::error::{DomainError, ErrorCode};

/// How long an issued token stays valid.
pub const TOKEN_TTL: Duration = Duration::from_secs(600);

/// Arguments that do not change what a call does, so they are not part of its identity.
const UNBOUND: [&str; 4] = ["confirmation_token", "dry_run", "wait", "timeout_seconds"];

/// Where new tokens come from: [`random_tokens`] normally, [`sequential_tokens`] for golden
/// runs.
pub type TokenSource = Box<dyn FnMut() -> String + Send>;

/// The monotonic clock that token lifetimes are measured on ([`Instant::now`] normally).
pub type MonotonicClock = Box<dyn FnMut() -> Instant + Send>;

/// Tokens of 18 random bytes from the operating system, written as 36 hexadecimal digits (the
/// prototype's `secrets.token_urlsafe(18)` writes the same bytes as 24 base64 characters; the
/// token is opaque to agents).
///
/// # Panics
///
/// The source panics if the operating system's random source fails: a predictable token would
/// defeat the confirmation.
pub fn random_tokens() -> TokenSource {
    Box::new(|| {
        let mut bytes = [0_u8; 18];
        getrandom::fill(&mut bytes).expect("the operating system random source is available");
        // Two lowercase hexadecimal digits per byte; every nibble is a valid digit.
        bytes
            .iter()
            .flat_map(|byte| [byte >> 4, byte & 0x0f])
            .filter_map(|nibble| char::from_digit(u32::from(nibble), 16))
            .collect()
    })
}

/// `golden-token-0001`, `golden-token-0002`, …: the stand-in for random tokens that
/// `scripts/golden/golden_common.py` (`deterministic()`) pins in golden runs.
pub fn sequential_tokens() -> TokenSource {
    let mut issued: u64 = 0;
    Box::new(move || {
        issued = issued.wrapping_add(1);
        format!("golden-token-{issued:04}")
    })
}

/// An issued token: the call it is bound to and when it expires.
struct Pending {
    action: String,
    fingerprint: String,
    expires: Instant,
}

/// The tokens issued and not yet used or expired, with the sources of new ones.
struct State {
    pending: HashMap<String, Pending>,
    tokens: TokenSource,
    clock: MonotonicClock,
}

/// The confirmation state of one agent client. It is shared by the threads that serve the
/// client, so it locks internally, as the prototype does.
pub struct Confirmations {
    enabled: bool,
    state: Mutex<State>,
}

impl fmt::Debug for Confirmations {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pending = self.state.lock().map_or(0, |state| state.pending.len());
        f.debug_struct("Confirmations")
            .field("enabled", &self.enabled)
            .field("pending", &pending)
            .finish_non_exhaustive()
    }
}

impl Confirmations {
    /// Confirmations with random tokens and the system's monotonic clock. With `enabled` false
    /// (the user turned confirmations off), every call goes through.
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self::with_sources(enabled, random_tokens(), Box::new(Instant::now))
    }

    /// Confirmations with the given token source and clock (golden runs and tests).
    #[must_use]
    pub fn with_sources(enabled: bool, tokens: TokenSource, clock: MonotonicClock) -> Self {
        Self {
            enabled,
            state: Mutex::new(State {
                pending: HashMap::new(),
                tokens,
                clock,
            }),
        }
    }

    /// Whether destructive calls need a token (`get_capabilities` reports it).
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// The identity of a call: `action` with `args` minus `confirmation_token`, `dry_run`,
    /// `wait` and `timeout_seconds`, as JSON with sorted keys. Equal calls have equal
    /// fingerprints; key order does not matter, value types do (`1`, `1.0` and `true` differ).
    ///
    /// The prototype hashes the same object written by `json.dumps(sort_keys=True)`; the
    /// fingerprint never leaves this type, so only that equivalence matters.
    #[must_use]
    pub fn fingerprint(action: &str, args: &Map<String, Value>) -> String {
        let bound: Map<String, Value> = args
            .iter()
            .filter(|(key, _)| !UNBOUND.contains(&key.as_str()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        let mut call = Map::new();
        call.insert("action".to_owned(), Value::from(action));
        call.insert("args".to_owned(), Value::Object(bound));
        sorted(&Value::Object(call)).to_string()
    }

    /// Lets the call through if `token` was issued for this exact call (`action` and `args`, the
    /// validated arguments as the typed argument struct serializes them) and has not expired;
    /// otherwise issues a new token bound to this call. A presented token is used up either
    /// way, and expired tokens are dropped first.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::ConfirmationRequired`]: `"<description>. Ask the user; if they agree,
    /// repeat the identical call with this confirmation_token."`, with the details
    /// `confirmation_token` (the new token), `action` and `expires_in_seconds` (600).
    pub fn require(
        &self,
        action: &str,
        args: &Map<String, Value>,
        token: Option<&str>,
        description: &str,
    ) -> Result<(), DomainError> {
        if !self.enabled {
            return Ok(());
        }
        let fingerprint = Self::fingerprint(action, args);
        // The state stays consistent if a holder panicked: each step is a single map update.
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let now = (state.clock)();
        state.pending.retain(|_, pending| pending.expires >= now);
        if let Some(pending) = token.and_then(|token| state.pending.remove(token))
            && pending.fingerprint == fingerprint
            && pending.action == action
        {
            return Ok(());
        }
        let issued = (state.tokens)();
        state.pending.insert(
            issued.clone(),
            Pending {
                action: action.to_owned(),
                fingerprint,
                expires: now + TOKEN_TTL,
            },
        );
        drop(state);
        Err(DomainError::new(
            ErrorCode::ConfirmationRequired,
            format!(
                "{description}. Ask the user; if they agree, repeat the identical call with this \
                 confirmation_token."
            ),
        )
        .with_detail("confirmation_token", issued)
        .with_detail("action", action)
        .with_detail("expires_in_seconds", TOKEN_TTL.as_secs()))
    }
}

/// `value` with the keys of every object in sorted order.
fn sorted(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key.clone(), sorted(value)))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Confirmations, random_tokens, sequential_tokens};

    #[test]
    fn fingerprints_ignore_key_order_and_unbound_arguments() {
        let a = json!({"b": {"y": 1, "x": [{"q": 1, "p": 2}]}, "a": 1, "dry_run": true});
        let b = json!({"a": 1, "wait": false, "b": {"x": [{"p": 2, "q": 1}], "y": 1}});
        let (Some(a), Some(b)) = (a.as_object(), b.as_object()) else {
            unreachable!("objects")
        };
        assert_eq!(
            Confirmations::fingerprint("save_project", a),
            Confirmations::fingerprint("save_project", b)
        );
        assert_ne!(
            Confirmations::fingerprint("save_project", a),
            Confirmations::fingerprint("export_midi", a)
        );
    }

    #[test]
    fn token_sources() {
        let mut golden = sequential_tokens();
        assert_eq!(golden(), "golden-token-0001");
        assert_eq!(golden(), "golden-token-0002");
        let mut random = random_tokens();
        let (first, second) = (random(), random());
        assert_eq!(first.len(), 36);
        assert!(first.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }
}
