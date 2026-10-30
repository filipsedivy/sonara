//! Per-part random streams: one seed for a whole request, an independent stream for each part.
//!
//! A part's stream is a [`PyRandom`] seeded through MT19937's key array ([`PyRandom::with_key`])
//! with the words `[seed low, seed high, id length, id bytes…]`: the seed's 64-bit two's
//! complement, least significant word first; the part id's length in bytes; then the id's bytes
//! packed little-endian into 32-bit words, the last one padded with zero bytes.
//!
//! - **Injective.** Two keys are equal only for the same seed and part id: the seed words differ
//!   for different seeds, and the length word separates ids whose packed words could otherwise
//!   coincide. A negative seed differs from its absolute value, which [`PyRandom::new`] does not
//!   tell apart.
//! - **Independent of the other parts.** The key depends on the part's id, never on its position
//!   among the parts, so adding or removing a part leaves every other part's stream as it was.
//! - **No hash of our own (QG-13).** The mixing is MT19937's key seeding in `rand_mt`. A part id
//!   of at most [`MAX_PART_ID_BYTES`](super::MAX_PART_ID_BYTES) bytes gives a key of at most 19 words, well inside the 624
//!   words the key seeding reads.

use crate::pyrandom::PyRandom;

use super::PartId;

/// The seed of a whole performance request; 0 by default, as in the MIDI generators.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Seed(i64);

impl Seed {
    /// The seed `value`; every value is valid.
    #[must_use]
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    /// The seed's value.
    #[must_use]
    pub const fn value(self) -> i64 {
        self.0
    }

    /// The random stream of `part` under this seed: the same seed and part always give the same
    /// stream, and it does not depend on any other part.
    #[must_use]
    pub fn stream(self, part: &PartId) -> PyRandom {
        PyRandom::with_key(&key(self, part))
    }
}

impl From<i64> for Seed {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

/// The key array of `part`'s stream under `seed` (see the module documentation).
fn key(seed: Seed, part: &PartId) -> Vec<u32> {
    let [b0, b1, b2, b3, b4, b5, b6, b7] = seed.0.to_le_bytes();
    let id = part.as_str().as_bytes();
    let length = u32::try_from(id.len())
        .expect("a part id holds at most MAX_PART_ID_BYTES bytes, far below u32::MAX");
    let mut key = Vec::with_capacity(3 + id.len().div_ceil(4));
    key.push(u32::from_le_bytes([b0, b1, b2, b3]));
    key.push(u32::from_le_bytes([b4, b5, b6, b7]));
    key.push(length);
    key.extend(id.chunks(4).map(|chunk| {
        let mut word = [0_u8; 4];
        for (slot, &byte) in word.iter_mut().zip(chunk) {
            *slot = byte;
        }
        u32::from_le_bytes(word)
    }));
    key
}

#[cfg(test)]
mod tests {
    use super::{Seed, key};
    use crate::performance::PartId;
    use crate::pyrandom::PyRandom;

    fn part(slug: &str) -> PartId {
        PartId::new(slug).expect("a valid part id")
    }

    fn draws(seed: Seed, slug: &str, count: usize) -> Vec<f64> {
        let mut stream = seed.stream(&part(slug));
        (0..count).map(|_| stream.random()).collect()
    }

    #[test]
    fn the_key_holds_the_seed_words_the_length_and_the_packed_id() {
        assert_eq!(
            key(Seed::new(-2), &part("ab_cd")),
            [
                0xFFFF_FFFE,
                0xFFFF_FFFF,
                5,
                u32::from_le_bytes(*b"ab_c"),
                u32::from_le_bytes([b'd', 0, 0, 0]),
            ]
        );
        assert_eq!(
            key(Seed::new((7 << 32) + 3), &part("horn")),
            [3, 7, 4, u32::from_le_bytes(*b"horn")]
        );
        assert_eq!(Seed::default(), Seed::new(0));
        assert_eq!(Seed::from(-5).value(), -5);
    }

    #[test]
    fn a_part_stream_matches_cpython_for_its_key() {
        // The key's last word is never zero, so `CPython`'s `random.Random(n)`, with `n` the
        // integer whose 32-bit words (least significant first) are the key, draws the same
        // numbers.
        assert_eq!(
            draws(Seed::new(0), "horn_3", 3),
            [
                0.401_987_896_764_506_3,
                0.050_583_598_136_088_925,
                0.733_631_604_964_545
            ]
        );
        assert_eq!(
            draws(Seed::new(-1), "violins_1", 3),
            [
                0.036_343_870_527_661_415,
                0.167_562_917_340_842_83,
                0.016_221_311_826_412_288
            ]
        );
        assert_eq!(
            draws(Seed::new(0), "horn_1", 3),
            [
                0.730_486_450_760_971_4,
                0.478_657_431_433_854_4,
                0.948_943_563_874_075_1
            ]
        );
    }

    #[test]
    fn a_negative_seed_differs_from_its_absolute_value() {
        // `PyRandom::new` drops the sign; a part stream keeps it.
        assert_eq!(PyRandom::new(3).random(), PyRandom::new(-3).random());
        assert_eq!(draws(Seed::new(3), "horn_1", 1), [0.941_161_545_995_736_5]);
        assert_eq!(
            draws(Seed::new(-3), "horn_1", 1),
            [0.418_527_847_255_055_65]
        );
    }

    #[test]
    fn same_seed_and_part_give_the_same_stream_whatever_else_is_drawn() {
        let alone = draws(Seed::new(11), "violas", 64);
        // Other parts' streams, created and drawn first, change nothing.
        for other in ["violins_1", "violins_2", "cellos", "horn_1"] {
            let _ = draws(Seed::new(11), other, 100);
        }
        assert_eq!(draws(Seed::new(11), "violas", 64), alone);
    }

    #[test]
    fn streams_differ_between_parts_and_between_seeds() {
        let parts = [
            "horn_1",
            "horn_2",
            "horn_3",
            "horn_13",
            "violins_1",
            "violins_2",
        ];
        let mut seen: Vec<Vec<f64>> = Vec::new();
        for seed in [0, 1, -1, 1 << 32, i64::MIN, i64::MAX] {
            for slug in parts {
                let drawn = draws(Seed::new(seed), slug, 8);
                assert!(!seen.contains(&drawn), "{seed} {slug} repeats a stream");
                seen.push(drawn);
            }
        }
    }
}
