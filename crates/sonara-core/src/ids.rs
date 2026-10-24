//! Stable object identifiers (`sonara.domain.ids`).
//!
//! IDs look like `trk_8f3k2m9q4x1z`: a short type prefix plus 12 Crockford base-32 characters
//! (60 random bits). They are never derived from names or positions. Commands receive their IDs
//! when they are constructed, from an [`IdGenerator`], so applying a command is deterministic.

/// Prefix of project IDs.
pub const PROJECT: &str = "prj";
/// Prefix of track IDs.
pub const TRACK: &str = "trk";
/// Prefix of clip IDs.
pub const CLIP: &str = "clp";
/// Prefix of MIDI note IDs.
pub const NOTE: &str = "n";
/// Prefix of device (instrument and effect) IDs.
pub const DEVICE: &str = "dev";
/// Prefix of send IDs.
pub const SEND: &str = "snd";
/// Prefix of automation lane IDs.
pub const LANE: &str = "aut";
/// Prefix of media asset IDs.
pub const ASSET: &str = "med";
/// Prefix of marker IDs.
pub const MARKER: &str = "mrk";
/// Prefix of section IDs.
pub const SECTION: &str = "sec";
/// The track ID reserved for the master bus.
pub const MASTER_ID: &str = "master";

const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";
/// Number of characters after the prefix.
pub const ID_LENGTH: usize = 12;

/// A source of new IDs.
pub trait IdGenerator: Send {
    /// A new, unique ID with the given type prefix.
    fn new_id(&mut self, prefix: &str) -> String;
}

/// Random IDs (60 bits from the operating system), for normal operation.
#[derive(Debug, Default, Clone, Copy)]
pub struct RandomIds;

impl IdGenerator for RandomIds {
    fn new_id(&mut self, prefix: &str) -> String {
        let mut bytes = [0_u8; 8];
        // The OS random source only fails on broken systems; an all-zero fallback would create
        // colliding IDs, so failing loudly is the safe choice here.
        getrandom::fill(&mut bytes).expect("the operating system random source is available");
        let mut value = u64::from_le_bytes(bytes);
        let mut out = String::with_capacity(prefix.len() + 1 + ID_LENGTH);
        out.push_str(prefix);
        out.push('_');
        for _ in 0..ID_LENGTH {
            out.push(char::from(
                ALPHABET[usize::try_from(value & 31).unwrap_or(0)],
            ));
            value >>= 5;
        }
        out
    }
}

/// Sequential IDs (`trk_000000000001`, …) for tests and golden data; the prototype's
/// `deterministic_ids(seed)` starts at `seed + 1`.
#[derive(Debug, Clone)]
pub struct SequentialIds {
    next: u64,
}

impl SequentialIds {
    /// IDs counting up from `seed + 1`, like the prototype's `deterministic_ids(seed)`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { next: seed + 1 }
    }
}

impl IdGenerator for SequentialIds {
    fn new_id(&mut self, prefix: &str) -> String {
        let id = format!("{prefix}_{:0width$}", self.next, width = ID_LENGTH);
        self.next += 1;
        id
    }
}

/// The type prefix of an ID (`"trk"` for `trk_…`).
#[must_use]
pub fn id_prefix(object_id: &str) -> &str {
    object_id
        .split_once('_')
        .map_or(object_id, |(prefix, _)| prefix)
}

/// Whether `value` is a well-formed ID: `master`, or an alphanumeric prefix, `_`, and 1–64
/// alphanumeric characters or dashes.
#[must_use]
pub fn is_valid_id(value: &str) -> bool {
    if value == MASTER_ID {
        return true;
    }
    let Some((prefix, body)) = value.split_once('_') else {
        return false;
    };
    let body_chars = body.chars().count();
    is_alnum(prefix)
        && (1..=64).contains(&body_chars)
        && is_alnum(&body.chars().filter(|c| *c != '-').collect::<String>())
}

/// Python's `str.isalnum`: non-empty and every character alphanumeric.
fn is_alnum(text: &str) -> bool {
    !text.is_empty() && text.chars().all(char::is_alphanumeric)
}

#[cfg(test)]
mod tests {
    use super::{IdGenerator, RandomIds, SequentialIds, id_prefix, is_valid_id};

    #[test]
    fn sequential_ids_match_the_prototype() {
        let mut ids = SequentialIds::new(0);
        assert_eq!(ids.new_id("trk"), "trk_000000000001");
        assert_eq!(ids.new_id("n"), "n_000000000002");
        let mut seeded = SequentialIds::new(1000);
        assert_eq!(seeded.new_id("n"), "n_000000001001");
    }

    #[test]
    fn random_ids_are_well_formed_and_distinct() {
        let mut ids = RandomIds;
        let a = ids.new_id("clp");
        let b = ids.new_id("clp");
        assert!(is_valid_id(&a), "{a}");
        assert_eq!(a.len(), 4 + 12);
        assert_ne!(a, b);
        assert_eq!(id_prefix(&a), "clp");
    }

    #[test]
    fn validation_follows_the_prototype() {
        assert!(is_valid_id("master"));
        assert!(is_valid_id("trk_abc-123"));
        assert!(!is_valid_id("trk_"));
        assert!(!is_valid_id("trk"));
        assert!(!is_valid_id("t-k_abc"));
        assert!(!is_valid_id(&format!("trk_{}", "a".repeat(65))));
    }
}
