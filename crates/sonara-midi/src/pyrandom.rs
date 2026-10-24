//! A bit-exact port of `CPython`'s `random.Random` for the methods the generators use.
//!
//! The prototype's generators draw from `random.Random(seed)`, and the golden data records their
//! output. To reproduce it, this module follows `CPython` 3.13
//! (`Modules/_randommodule.c` and `Lib/random.py`): MT19937 seeded through `init_by_array` with
//! the absolute seed split into 32-bit words (least significant first), a 53-bit `random()`,
//! `getrandbits`, the rejection loop of `_randbelow`, `randint`, `choice`, `uniform` and the
//! Box-Muller `gauss` with its cached second value.

use rand_mt::Mt;

/// `2**-53`, the scale of the 53-bit `random()`.
const RECIP_2_53: f64 = 1.0 / 9_007_199_254_740_992.0;
/// `2 * pi`, as `TWOPI` in `Lib/random.py`.
const TWO_PI: f64 = 2.0 * std::f64::consts::PI;

/// `CPython`'s `random.Random`: the same seed gives the same sequence of numbers.
#[derive(Debug, Clone)]
pub struct PyRandom {
    mt: Mt,
    /// The cached second Box-Muller value (`Random.gauss_next`).
    gauss_next: Option<f64>,
}

impl PyRandom {
    /// Equivalent of `random.Random(seed)` for an integer seed. The sign is ignored, as in
    /// `CPython` (`abs(seed)`).
    #[must_use]
    pub fn new(seed: i64) -> Self {
        let magnitude = seed.unsigned_abs().to_le_bytes();
        let low = u32::from_le_bytes([magnitude[0], magnitude[1], magnitude[2], magnitude[3]]);
        let high = u32::from_le_bytes([magnitude[4], magnitude[5], magnitude[6], magnitude[7]]);
        // `CPython` passes the minimal number of 32-bit words, and one zero word for the seed 0.
        let mt = if high == 0 {
            Mt::new_with_key([low])
        } else {
            Mt::new_with_key([low, high])
        };
        Self {
            mt,
            gauss_next: None,
        }
    }

    /// The next float in `[0.0, 1.0)` with 53 random bits (`random()`).
    pub fn random(&mut self) -> f64 {
        let a = f64::from(self.mt.next_u32() >> 5);
        let b = f64::from(self.mt.next_u32() >> 6);
        (a * 67_108_864.0 + b) * RECIP_2_53
    }

    /// A non-negative integer with `k` random bits (`getrandbits(k)`); `0` for `k == 0`.
    ///
    /// # Panics
    ///
    /// When `k > 64`; the generators never need more.
    pub fn getrandbits(&mut self, k: u32) -> u64 {
        assert!(k <= 64, "getrandbits supports at most 64 bits, got {k}");
        if k == 0 {
            0
        } else if k <= 32 {
            u64::from(self.mt.next_u32() >> (32 - k))
        } else {
            let low = u64::from(self.mt.next_u32());
            let high = u64::from(self.mt.next_u32() >> (64 - k));
            low | (high << 32)
        }
    }

    /// A random integer in `[0, n)` by rejection sampling (`_randbelow`). Returns 0 for `n == 0`
    /// without consuming random numbers (`CPython` does not define that case).
    pub fn randbelow(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        let bits = n.bit_width();
        loop {
            let r = self.getrandbits(bits);
            if r < n {
                return r;
            }
        }
    }

    /// A random integer in `[a, b]` (`randint(a, b)`).
    ///
    /// # Panics
    ///
    /// When `a > b` (`CPython` raises `ValueError`) or the range holds 2**64 values or more.
    pub fn randint(&mut self, a: i64, b: i64) -> i64 {
        assert!(a <= b, "empty range for randint({a}, {b})");
        let width = u64::try_from(i128::from(b) - i128::from(a) + 1)
            .expect("randint ranges hold fewer than 2**64 values");
        let offset = i128::from(self.randbelow(width));
        i64::try_from(i128::from(a) + offset).expect("a + offset lies in [a, b]")
    }

    /// A random element of `items` (`choice`), or `None` for an empty slice (`CPython` raises
    /// `IndexError`).
    pub fn choice<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            return None;
        }
        let index = self.randbelow(u64::try_from(items.len()).ok()?);
        items.get(usize::try_from(index).ok()?)
    }

    /// A random float between `a` and `b` (`uniform`): `a + (b - a) * random()`.
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }

    /// A normally distributed float with mean `mu` and standard deviation `sigma` (`gauss`).
    ///
    /// Box-Muller yields two values per pair of draws; the second one is cached for the next
    /// call, exactly as `CPython` does.
    pub fn gauss(&mut self, mu: f64, sigma: f64) -> f64 {
        let z = if let Some(cached) = self.gauss_next.take() {
            cached
        } else {
            let x2pi = self.random() * TWO_PI;
            let g2rad = (-2.0 * (1.0 - self.random()).ln()).sqrt();
            self.gauss_next = Some(x2pi.sin() * g2rad);
            x2pi.cos() * g2rad
        };
        mu + z * sigma
    }
}

#[cfg(test)]
mod tests {
    use super::PyRandom;

    // Expected values come from `CPython` 3.13: `random.Random(seed)` run in the prototype's
    // environment.

    #[test]
    fn random_matches_cpython() {
        let mut r = PyRandom::new(0);
        assert_eq!(r.random(), 0.844_421_851_525_048_1);
        assert_eq!(r.random(), 0.757_954_402_940_302_5);
        assert_eq!(r.random(), 0.420_571_580_830_845);
        let mut r = PyRandom::new(42);
        assert_eq!(r.random(), 0.639_426_798_457_883_7);
        assert_eq!(r.random(), 0.025_010_755_222_666_936);
        assert_eq!(r.random(), 0.275_029_318_369_119_26);
        let mut r = PyRandom::new(5);
        assert_eq!(r.random(), 0.622_901_694_889_701_9);
    }

    #[test]
    fn negative_and_wide_seeds_match_cpython() {
        // The sign is dropped; seeds above 32 bits use two key words.
        assert_eq!(PyRandom::new(-3).random(), 0.237_964_627_091_891_37);
        assert_eq!(PyRandom::new(3).random(), PyRandom::new(-3).random());
        assert_eq!(
            PyRandom::new((1 << 32) + 7).random(),
            0.225_508_889_298_931_87
        );
        assert_eq!(
            PyRandom::new((1 << 40) + 12345).random(),
            0.310_362_222_412_872_26
        );
    }

    #[test]
    fn getrandbits_matches_cpython() {
        let mut r = PyRandom::new(0);
        let drawn: Vec<u64> = [1, 5, 32, 33, 64, 7]
            .iter()
            .map(|&k| r.getrandbits(k))
            .collect();
        assert_eq!(
            drawn,
            [
                1,
                12,
                3_255_389_356,
                3_823_568_514,
                4_776_171_008_201_404_212,
                123
            ]
        );
        let mut r = PyRandom::new(42);
        let drawn: Vec<u64> = [1, 5, 32, 33, 64, 7]
            .iter()
            .map(|&k| r.getrandbits(k))
            .collect();
        assert_eq!(
            drawn,
            [
                1,
                3,
                107_420_369,
                3_184_935_163,
                4_117_511_471_858_006_928,
                17
            ]
        );
        assert_eq!(PyRandom::new(1).getrandbits(0), 0);
    }

    #[test]
    fn randint_matches_cpython() {
        let mut r = PyRandom::new(0);
        let first: Vec<i64> = (0..5).map(|_| r.randint(-8, 8)).collect();
        let second: Vec<i64> = (0..3).map(|_| r.randint(0, 15)).collect();
        let single: Vec<i64> = (0..2).map(|_| r.randint(3, 3)).collect();
        assert_eq!(first, [4, 5, -7, 0, 8]);
        assert_eq!(second, [15, 12, 9]);
        assert_eq!(single, [3, 3]);
        let mut r = PyRandom::new(42);
        let first: Vec<i64> = (0..5).map(|_| r.randint(-8, 8)).collect();
        assert_eq!(first, [-5, -8, 0, -1, -1]);
    }

    #[test]
    fn choice_matches_cpython() {
        let items = [10, 20, 30, 40, 50, 60, 70];
        let mut r = PyRandom::new(0);
        let drawn: Vec<i32> = (0..6)
            .map(|_| *r.choice(&items).expect("non-empty"))
            .collect();
        assert_eq!(drawn, [70, 40, 70, 40, 10, 30]);
        let mut r = PyRandom::new(5);
        let drawn: Vec<i32> = (0..6)
            .map(|_| *r.choice(&items).expect("non-empty"))
            .collect();
        assert_eq!(drawn, [50, 30, 60, 30, 70, 60]);
        assert_eq!(PyRandom::new(0).choice::<i32>(&[]), None);
    }

    #[test]
    fn uniform_matches_cpython() {
        let mut r = PyRandom::new(0);
        assert_eq!(r.uniform(-0.1, 0.1), 0.068_884_370_305_009_63);
        assert_eq!(r.uniform(-0.1, 0.1), 0.051_590_880_588_060_495);
        assert_eq!(r.uniform(-0.1, 0.1), -0.015_885_683_833_831_002);
    }

    #[test]
    fn gauss_matches_cpython_including_the_cached_value() {
        // libm may differ from the recording platform in the last bit, so compare with a tight
        // relative tolerance instead of exactly.
        let close = |expected: f64, actual: f64| {
            assert!(
                (expected - actual).abs() <= 1e-15 * expected.abs().max(1e-300),
                "expected {expected:e}, got {actual:e}"
            );
        };
        let mut r = PyRandom::new(0);
        for expected in [
            0.009_417_154_046_806_644,
            -0.013_965_781_047_011_499,
            -0.006_797_144_480_784_211,
            0.003_705_035_674_606_598,
            -0.010_163_488_941_880_709,
        ] {
            close(expected, r.gauss(0.0, 0.01));
        }
        let mut r = PyRandom::new(5);
        assert_eq!(r.random(), 0.622_901_694_889_701_9);
        assert_eq!(r.randint(-8, 8), 3);
        close(0.006_694_689_143_859_696, r.gauss(0.0, 0.01));
        assert_eq!(r.choice(&[1, 2, 3]), Some(&3));
        assert_eq!(r.uniform(-0.1, 0.1), 0.030_406_339_350_827_01);
        close(-3.587_820_189_263_822, r.gauss(1.0, 2.0));
    }

    #[test]
    fn randbelow_of_zero_consumes_nothing() {
        let mut r = PyRandom::new(9);
        assert_eq!(r.randbelow(0), 0);
        assert_eq!(r.random(), PyRandom::new(9).random());
    }

    #[test]
    #[should_panic(expected = "empty range for randint(2, 1)")]
    fn randint_rejects_an_empty_range() {
        PyRandom::new(0).randint(2, 1);
    }
}
