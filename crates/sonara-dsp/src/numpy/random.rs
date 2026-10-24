//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! NumPy's `np.random.default_rng(seed)`: [`SeedSequence`] → [`Pcg64`] → [`Generator`], bit for
//! bit (NumPy 2.5.4, `numpy/random/bit_generator.pyx`, `src/pcg64/pcg64.h`,
//! `src/distributions/distributions.c`, `_bounded_integers.pyx.in`, `_generator.pyx`).
//!
//! Nothing here allocates, locks or panics: a generator is a few words on the stack, so the synth
//! can reseed one per block segment on the audio thread (RT-02, RT-05). Arrays are filled into
//! caller-owned slices; NumPy draws an array element by element in C order, so a `(n, 2)` array
//! is the interleaved buffer of `2 n` values.
//!
//! Prototype call sites:
//!
//! - `dsp/time_frequency/reverb.py` (`synthesize_ir`): `default_rng(1234)`, then
//!   `standard_normal((n, 2))`, then per early reflection `random()` and, only when the delay
//!   fits, `choice([-1.0, 1.0], size=2)`.
//! - `instruments/drums.py` (`synthesize`): `default_rng(zlib.crc32(name))`, then
//!   `standard_normal(len)` once or twice per voice.
//! - `instruments/synth.py`: initial phases `default_rng(order).random(count)`; noise
//!   `default_rng((seed * 1_000_003 + age) & 0xFFFFFFFF).standard_normal(m)`, reseeded per block
//!   segment.
//! - `import_export/audio_export.py` (`_Quantizer`): one `default_rng(0x5A0A)` per file, and per
//!   chunk `random(shape) - random(shape)`: two full C-order arrays, drawn one after the other.

use super::ziggurat::{FI_DOUBLE, KI_DOUBLE, WI_DOUBLE, ZIGGURAT_NOR_INV_R, ZIGGURAT_NOR_R};

// SeedSequence constants (`bit_generator.pyx`).
const POOL_SIZE: usize = 4;
const INIT_A: u32 = 0x43b0_d7e5;
const MULT_A: u32 = 0x931e_8875;
const INIT_B: u32 = 0x8b51_f9dd;
const MULT_B: u32 = 0x58f3_8ded;
const MIX_MULT_L: u32 = 0xca01_f9dd;
const MIX_MULT_R: u32 = 0x4973_f715;
const XSHIFT: u32 = 16;

/// `PCG_DEFAULT_MULTIPLIER_128` of `pcg64.h` (high word 2549297995355413924, low word
/// 4865540595714422341).
const PCG_MULTIPLIER: u128 = 0x2360_ed05_1fc6_5da4_4385_df64_9fcc_f645;

/// `next_double`: 53 random bits scaled by 2^-53.
const DOUBLE_SCALE: f64 = 1.0 / 9_007_199_254_740_992.0;

/// Errors of [`Generator::integers`] and [`Generator::choice`], where NumPy raises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RandomError {
    /// `integers(low, high)` with `low >= high` (NumPy: `ValueError: low >= high`).
    EmptyRange,
    /// The range has more than 2^32 values. NumPy then uses its 64-bit Lemire path, which is not
    /// ported because no prototype call reaches it.
    RangeTooWide,
    /// `choice` from an empty population with samples requested (NumPy: `ValueError: a cannot
    /// be empty unless no samples are taken`).
    EmptyPopulation,
}

impl core::fmt::Display for RandomError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::EmptyRange => "low >= high",
            Self::RangeTooWide => "range wider than 2^32 values (64-bit path not ported)",
            Self::EmptyPopulation => "a cannot be empty unless no samples are taken",
        })
    }
}

impl core::error::Error for RandomError {}

/// NumPy's `SeedSequence(seed)` for a non-negative integer seed: no spawn key, pool size 4.
///
/// The seed is split into 32-bit words, lowest first (`0` is one zero word), and hashed into the
/// pool. Seeds below 2^32 give one word, larger ones two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeedSequence {
    pool: [u32; POOL_SIZE],
}

impl SeedSequence {
    /// `SeedSequence(entropy)`.
    #[must_use]
    pub fn new(entropy: u64) -> Self {
        let [b0, b1, b2, b3, b4, b5, b6, b7] = entropy.to_le_bytes();
        let low = u32::from_le_bytes([b0, b1, b2, b3]);
        let high = u32::from_le_bytes([b4, b5, b6, b7]);
        // `_int_to_uint32_array`: as many words as the integer needs, at least one.
        let words = [low, high];
        let word_count = if high == 0 { 1 } else { 2 };

        let mut hash_const = INIT_A;
        let mut pool = [0_u32; POOL_SIZE];
        // `mix_entropy`: hash the entropy into the pool, running the hash out on zeros.
        for (i, slot) in pool.iter_mut().enumerate() {
            let word = if i < word_count {
                words.get(i).copied().unwrap_or(0)
            } else {
                0
            };
            *slot = hashmix(word, &mut hash_const);
        }
        // Mix all bits together so late bits can affect earlier bits. Two words never exceed
        // the pool, so the "remaining entropy" loop of NumPy has nothing to do.
        for i_src in 0..POOL_SIZE {
            let Some(&src) = pool.get(i_src) else {
                continue;
            };
            for (i_dst, dst) in pool.iter_mut().enumerate() {
                if i_dst != i_src {
                    *dst = mix(*dst, hashmix(src, &mut hash_const));
                }
            }
        }
        Self { pool }
    }

    /// The mixed entropy pool (`SeedSequence.pool`).
    #[must_use]
    pub fn pool(&self) -> [u32; POOL_SIZE] {
        self.pool
    }

    /// `generate_state(out.len(), np.uint32)`.
    pub fn generate_state_u32(&self, out: &mut [u32]) {
        let mut words = StateWords::new(self.pool);
        for slot in out {
            *slot = words.next_word();
        }
    }

    /// `generate_state(out.len(), np.uint64)`: twice as many 32-bit words, paired little-endian
    /// (the first word is the low half).
    pub fn generate_state_u64(&self, out: &mut [u64]) {
        let mut words = StateWords::new(self.pool);
        for slot in out {
            let low = words.next_word();
            let high = words.next_word();
            *slot = u64::from(low) | (u64::from(high) << 32);
        }
    }
}

/// `hashmix` of `bit_generator.pyx`; `hash_const` is input and output.
fn hashmix(value: u32, hash_const: &mut u32) -> u32 {
    let mut value = value ^ *hash_const;
    *hash_const = hash_const.wrapping_mul(MULT_A);
    value = value.wrapping_mul(*hash_const);
    value ^ (value >> XSHIFT)
}

/// `mix` of `bit_generator.pyx`.
fn mix(x: u32, y: u32) -> u32 {
    let result = MIX_MULT_L
        .wrapping_mul(x)
        .wrapping_sub(MIX_MULT_R.wrapping_mul(y));
    result ^ (result >> XSHIFT)
}

/// The word stream of `generate_state`: the pool cycled and hashed with `INIT_B`/`MULT_B`.
struct StateWords {
    pool: [u32; POOL_SIZE],
    position: usize,
    hash_const: u32,
}

impl StateWords {
    fn new(pool: [u32; POOL_SIZE]) -> Self {
        Self {
            pool,
            position: 0,
            hash_const: INIT_B,
        }
    }

    fn next_word(&mut self) -> u32 {
        let mut value = self.pool.get(self.position).copied().unwrap_or(0);
        self.position = (self.position + 1) % POOL_SIZE;
        value ^= self.hash_const;
        self.hash_const = self.hash_const.wrapping_mul(MULT_B);
        value = value.wrapping_mul(self.hash_const);
        value ^ (value >> XSHIFT)
    }
}

/// The state of a [`Pcg64`], as NumPy's `bit_generator.state` reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pcg64State {
    /// The 128-bit LCG state (`state["state"]["state"]`).
    pub state: u128,
    /// The LCG increment, always odd (`state["state"]["inc"]`).
    pub inc: u128,
    /// Whether the high half of the last 64-bit draw is buffered for the next 32-bit draw.
    pub has_uint32: bool,
    /// The buffered high half. NumPy keeps the last value after it is consumed.
    pub uinteger: u32,
}

/// NumPy's `PCG64` bit generator: a 128-bit LCG with the XSL-RR output function, and the
/// buffered high half of a 64-bit draw that 32-bit draws use first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pcg64 {
    state: u128,
    inc: u128,
    has_uint32: bool,
    uinteger: u32,
}

impl Pcg64 {
    /// `PCG64(seed)`: seeded through `SeedSequence(seed)`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self::from_seed_sequence(&SeedSequence::new(seed))
    }

    /// `PCG64(seed_sequence)`: `generate_state(4, uint64)` gives the initial state (words 0
    /// and 1, high first) and the stream (words 2 and 3), then `pcg_setseq_128_srandom_r`.
    #[must_use]
    pub fn from_seed_sequence(seed: &SeedSequence) -> Self {
        let mut words = [0_u64; 4];
        seed.generate_state_u64(&mut words);
        let [s0, s1, i0, i1] = words;
        let init_state = (u128::from(s0) << 64) | u128::from(s1);
        let init_seq = (u128::from(i0) << 64) | u128::from(i1);
        let mut pcg = Self {
            state: 0,
            inc: (init_seq << 1) | 1,
            has_uint32: false,
            uinteger: 0,
        };
        pcg.step();
        pcg.state = pcg.state.wrapping_add(init_state);
        pcg.step();
        pcg
    }

    /// The state, as `bit_generator.state` reports it.
    #[must_use]
    pub fn state(&self) -> Pcg64State {
        Pcg64State {
            state: self.state,
            inc: self.inc,
            has_uint32: self.has_uint32,
            uinteger: self.uinteger,
        }
    }

    /// `bit_generator.random_raw()`: the next 64-bit output. It leaves the 32-bit buffer alone.
    pub fn random_raw(&mut self) -> u64 {
        self.next_uint64()
    }

    /// `bit_generator.random_raw(out.len())`.
    pub fn random_raw_fill(&mut self, out: &mut [u64]) {
        for slot in out {
            *slot = self.next_uint64();
        }
    }

    fn step(&mut self) {
        self.state = self
            .state
            .wrapping_mul(PCG_MULTIPLIER)
            .wrapping_add(self.inc);
    }

    /// `pcg64_next64`: step, then the XSL-RR output of the new state.
    fn next_uint64(&mut self) -> u64 {
        self.step();
        xsl_rr(self.state)
    }

    /// `pcg64_next32`: the buffered high half if there is one, else the low half of a new
    /// 64-bit draw, buffering its high half.
    fn next_uint32(&mut self) -> u32 {
        if self.has_uint32 {
            self.has_uint32 = false;
            return self.uinteger;
        }
        let [b0, b1, b2, b3, b4, b5, b6, b7] = self.next_uint64().to_le_bytes();
        self.has_uint32 = true;
        self.uinteger = u32::from_le_bytes([b4, b5, b6, b7]);
        u32::from_le_bytes([b0, b1, b2, b3])
    }

    /// `pcg64_next_double`: `(next64 >> 11) * 2^-53`.
    fn next_double(&mut self) -> f64 {
        exact_f64(self.next_uint64() >> 11) * DOUBLE_SCALE
    }
}

/// `pcg_output_xsl_rr_128_64`: the high half XOR the low half, rotated right by the top six bits.
#[expect(
    clippy::cast_possible_truncation,
    reason = "takes the 64-bit halves and the 6-bit rotation of the 128-bit state"
)]
fn xsl_rr(state: u128) -> u64 {
    let rotation = (state >> 122) as u32;
    (((state >> 64) as u64) ^ (state as u64)).rotate_right(rotation)
}

/// A 64-bit integer below 2^53 as a float, exactly.
#[expect(
    clippy::cast_precision_loss,
    reason = "callers pass values below 2^53, which convert exactly"
)]
fn exact_f64(value: u64) -> f64 {
    value as f64
}

/// NumPy's `Generator` over [`Pcg64`]; [`default_rng`] builds one from a seed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generator {
    bit_generator: Pcg64,
}

/// `np.random.default_rng(seed)` for a non-negative integer seed: `Generator(PCG64(seed))`.
///
/// Cheap and allocation-free, so it can run on the audio thread.
///
/// # Examples
///
/// ```
/// use sonara_dsp::numpy::random::default_rng;
///
/// let mut rng = default_rng(1234);
/// let u = rng.random();
/// assert!((0.0..1.0).contains(&u));
/// assert_eq!(default_rng(1234).random(), u);
/// ```
#[must_use]
pub fn default_rng(seed: u64) -> Generator {
    Generator::new(Pcg64::new(seed))
}

impl Generator {
    /// `Generator(bit_generator)`.
    #[must_use]
    pub fn new(bit_generator: Pcg64) -> Self {
        Self { bit_generator }
    }

    /// `Generator.bit_generator`.
    #[must_use]
    pub fn bit_generator(&self) -> &Pcg64 {
        &self.bit_generator
    }

    /// `Generator.bit_generator`, mutable (for `random_raw`).
    pub fn bit_generator_mut(&mut self) -> &mut Pcg64 {
        &mut self.bit_generator
    }

    /// `random()`: a float in `[0, 1)` from the top 53 bits of one 64-bit draw.
    pub fn random(&mut self) -> f64 {
        self.bit_generator.next_double()
    }

    /// `random(shape)`, filled in C order (`random_standard_uniform_fill`).
    pub fn random_fill(&mut self, out: &mut [f64]) {
        for slot in out {
            *slot = self.bit_generator.next_double();
        }
    }

    /// `integers(low, high)` with the default `int64` dtype and `endpoint=False`: an integer in
    /// `[low, high)`.
    ///
    /// NumPy's `random_bounded_uint64_fill` with `rng = high - 1 - low`: a single value needs no
    /// draw; `rng = 2^32 - 1` takes one buffered 32-bit draw; a smaller `rng` uses Lemire's method
    /// on buffered 32-bit draws (`buffered_bounded_lemire_uint32`). The buffer is the bit
    /// generator's, so a call can use the high half of a 64-bit value drawn by an earlier call.
    /// `integers(0, n)` with `size` draws the same values as `n` scalar calls.
    ///
    /// # Errors
    ///
    /// [`RandomError::EmptyRange`] when `low >= high`; [`RandomError::RangeTooWide`] when the
    /// range has more than 2^32 values (NumPy's 64-bit path is not ported).
    pub fn integers(&mut self, low: i64, high: i64) -> Result<i64, RandomError> {
        if low >= high {
            return Err(RandomError::EmptyRange);
        }
        // Two's complement: the true difference lies in [0, 2^64 - 1], so wrapping gives it.
        let range = high.wrapping_sub(1).wrapping_sub(low).cast_unsigned();
        let offset = self.bounded_u32(range)?;
        Ok(low.wrapping_add_unsigned(offset))
    }

    /// A draw in `[0, range]` for `range <= 2^32 - 1`, as `random_bounded_uint64_fill` makes it.
    fn bounded_u32(&mut self, range: u64) -> Result<u64, RandomError> {
        let Ok(range) = u32::try_from(range) else {
            return Err(RandomError::RangeTooWide);
        };
        Ok(match range {
            0 => 0,
            u32::MAX => u64::from(self.bit_generator.next_uint32()),
            _ => u64::from(self.lemire_u32(range)),
        })
    }

    /// `buffered_bounded_lemire_uint32` for `range < 2^32 - 1`.
    fn lemire_u32(&mut self, range: u32) -> u32 {
        let range_excl = u64::from(range) + 1;
        let mut m = u64::from(self.bit_generator.next_uint32()) * range_excl;
        let mut leftover = m & 0xffff_ffff;
        if leftover < range_excl {
            // `(UINT32_MAX - rng) % rng_excl`; `range_excl` is at least 1 (checked form, RT-05).
            let threshold = u64::from(u32::MAX - range)
                .checked_rem(range_excl)
                .unwrap_or(0);
            while leftover < threshold {
                m = u64::from(self.bit_generator.next_uint32()) * range_excl;
                leftover = m & 0xffff_ffff;
            }
        }
        // m < 2^64 and its high half is below range_excl <= 2^32.
        u32::try_from(m >> 32).unwrap_or(range)
    }

    /// `choice(population, size=out.len())` with `replace=True` and no `p`: each element of `out`
    /// is `population[integers(0, len)]`, drawn in order.
    ///
    /// The prototype draws `choice([-1.0, 1.0], size=2)`: two 32-bit draws, which are the two
    /// halves of one 64-bit value unless a half was already buffered.
    ///
    /// # Errors
    ///
    /// [`RandomError::EmptyPopulation`] when `population` is empty and `out` is not;
    /// [`RandomError::RangeTooWide`] for a population above 2^32 elements.
    pub fn choice(&mut self, population: &[f64], out: &mut [f64]) -> Result<(), RandomError> {
        if out.is_empty() {
            return Ok(());
        }
        if population.is_empty() {
            return Err(RandomError::EmptyPopulation);
        }
        let range = u64::try_from(population.len() - 1).map_err(|_| RandomError::RangeTooWide)?;
        if range > u64::from(u32::MAX) {
            return Err(RandomError::RangeTooWide);
        }
        for slot in out {
            let drawn = self.bounded_u32(range)?;
            // `drawn <= range < population.len()`, so the lookup always succeeds.
            if let Some(&value) = usize::try_from(drawn)
                .ok()
                .and_then(|index| population.get(index))
            {
                *slot = value;
            }
        }
        Ok(())
    }

    /// `standard_normal()`: NumPy's 256-layer ziggurat (`random_standard_normal`).
    ///
    /// One 64-bit draw gives the layer (low 8 bits), the sign (bit 8) and a 52-bit magnitude
    /// `rabs`; `x = rabs * wi[idx]` is returned when `rabs < ki[idx]` (about 99 % of draws).
    /// Layer 0 otherwise samples the tail, `r + xx` with `xx = -inv_r * log1p(-u1)`, accepted
    /// when `2 * (-log1p(-u2)) > xx²`; its sign is bit 8 of `rabs`. The other layers accept `x`
    /// when `fma(fi[idx-1] - fi[idx], u, fi[idx]) < exp(-x²/2)`, else they start over.
    ///
    /// The wedge test is fused on purpose. Clang contracts that C expression into a fused
    /// multiply-add on the recording platform (macOS arm64), unlike NumPy's ufuncs, which never
    /// fuse. Crafted PCG64 states where the fused and the unfused test disagree showed NumPy
    /// following the fused one in 7 of 7 cases. `mul_add` is correctly rounded everywhere, so
    /// the decision matches on every platform. `log1p` and `exp` come from the platform libm
    /// (RT-11).
    pub fn standard_normal(&mut self) -> f64 {
        loop {
            let r = self.bit_generator.next_uint64();
            let [low_byte, ..] = r.to_le_bytes();
            let idx = usize::from(low_byte);
            let r = r >> 8;
            let negative = r & 1 == 1;
            let rabs = (r >> 1) & 0x000f_ffff_ffff_ffff;
            // `idx < 256` indexes the 256-entry tables, so these lookups always succeed.
            let (Some(&ki), Some(&wi), Some(&fi)) =
                (KI_DOUBLE.get(idx), WI_DOUBLE.get(idx), FI_DOUBLE.get(idx))
            else {
                continue;
            };
            let mut x = exact_f64(rabs) * wi;
            if negative {
                x = -x;
            }
            if rabs < ki {
                return x;
            }
            let Some(&fi_prev) = idx.checked_sub(1).and_then(|i| FI_DOUBLE.get(i)) else {
                return self.normal_tail(rabs);
            };
            let u = self.bit_generator.next_double();
            if (fi_prev - fi).mul_add(u, fi) < (-0.5 * x * x).exp() {
                return x;
            }
        }
    }

    /// The tail of layer 0 (`|x| > r`), Marsaglia's method as NumPy writes it.
    fn normal_tail(&mut self, rabs: u64) -> f64 {
        loop {
            // `1 - U` avoids `log(0)` (NumPy GH 13361).
            let xx = -ZIGGURAT_NOR_INV_R * (-self.bit_generator.next_double()).ln_1p();
            let yy = -(-self.bit_generator.next_double()).ln_1p();
            if yy + yy > xx * xx {
                return if (rabs >> 8) & 1 == 1 {
                    -(ZIGGURAT_NOR_R + xx)
                } else {
                    ZIGGURAT_NOR_R + xx
                };
            }
        }
    }

    /// `standard_normal(shape)`, filled in C order (`random_standard_normal_fill`).
    pub fn standard_normal_fill(&mut self, out: &mut [f64]) {
        for slot in out {
            *slot = self.standard_normal();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Generator, Pcg64, RandomError, SeedSequence, default_rng};

    #[test]
    fn seed_words_follow_numpy() {
        // NumPy 2.5.4: SeedSequence(0).pool and SeedSequence(2**40 + 5).generate_state(2, uint64).
        assert_eq!(
            SeedSequence::new(0).pool(),
            [0xfe40_eb07, 0x4f36_3a36, 0x4eb2_009d, 0xc89a_7aa7]
        );
        let mut words = [0_u64; 2];
        SeedSequence::new((1 << 40) + 5).generate_state_u64(&mut words);
        let mut halves = [0_u32; 4];
        SeedSequence::new((1 << 40) + 5).generate_state_u32(&mut halves);
        let [h0, h1, h2, h3] = halves;
        assert_eq!(
            words,
            [
                u64::from(h0) | u64::from(h1) << 32,
                u64::from(h2) | u64::from(h3) << 32
            ]
        );
    }

    #[test]
    fn raw_stream_matches_numpy() {
        // NumPy 2.5.4: default_rng(1234).bit_generator.random_raw(3).
        let mut pcg = Pcg64::new(1234);
        let mut raw = [0_u64; 3];
        pcg.random_raw_fill(&mut raw);
        assert_eq!(
            raw,
            [
                0xfa08_fef3_fab6_bbba,
                0x6154_81f7_fce9_ac81,
                0xec59_dd7c_2bde_3cb4
            ]
        );
    }

    #[test]
    fn thirty_two_bit_draws_use_both_halves_low_first() {
        let mut rng = default_rng(7);
        let mut raw = default_rng(7);
        let value = raw.bit_generator_mut().random_raw();
        // rng = 2^32 - 1 takes one 32-bit draw directly.
        let low = rng.integers(0, 1 << 32).unwrap();
        let high = rng.integers(0, 1 << 32).unwrap();
        assert_eq!(low.cast_unsigned(), value & 0xffff_ffff);
        assert_eq!(high.cast_unsigned(), value >> 32);
        assert!(!rng.bit_generator().state().has_uint32);
    }

    #[test]
    fn the_buffer_survives_other_draws() {
        let mut a = default_rng(99);
        let mut b = default_rng(99);
        // integers buffers the high half; random() and random_raw() draw fresh 64-bit values.
        let first = a.integers(0, 1 << 32).unwrap();
        let u = a.random();
        let second = a.integers(0, 1 << 32).unwrap();
        let value = b.bit_generator_mut().random_raw();
        assert_eq!(first.cast_unsigned(), value & 0xffff_ffff);
        assert_eq!(u, b.random());
        assert_eq!(second.cast_unsigned(), value >> 32);
    }

    #[test]
    fn integers_rejects_what_numpy_rejects() {
        let mut rng = default_rng(1);
        assert_eq!(rng.integers(3, 3), Err(RandomError::EmptyRange));
        assert_eq!(
            rng.integers(0, (1 << 32) + 1),
            Err(RandomError::RangeTooWide)
        );
        assert_eq!(
            rng.integers(i64::MIN, i64::MAX),
            Err(RandomError::RangeTooWide)
        );
        let before = *rng.bit_generator();
        // A one-value range draws nothing.
        assert_eq!(rng.integers(-4, -3), Ok(-4));
        assert_eq!(*rng.bit_generator(), before);
        let mut out = [0.0; 2];
        assert_eq!(rng.choice(&[], &mut out), Err(RandomError::EmptyPopulation));
        assert_eq!(rng.choice(&[], &mut []), Ok(()));
    }

    #[test]
    fn integers_stay_in_range() {
        let mut rng = default_rng(5);
        for (low, high) in [(0, 2), (-7, 10), (0, 3 << 30), (i64::MAX - 5, i64::MAX)] {
            for _ in 0..1000 {
                let v = rng.integers(low, high).unwrap();
                assert!((low..high).contains(&v), "{v} not in {low}..{high}");
            }
        }
    }

    #[test]
    fn construction_is_deterministic_and_copyable() {
        let a: Generator = default_rng(0x5A0A);
        let mut b = a;
        let mut a = a;
        assert_eq!(a.random(), b.random());
        assert_eq!(a.standard_normal(), b.standard_normal());
    }
}
