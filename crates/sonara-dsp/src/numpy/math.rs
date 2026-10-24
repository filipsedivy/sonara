//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! NumPy reductions and helpers with NumPy's order of operations (NumPy 2.5.4), plus Python's
//! `round(x, ndigits)`. Every function works on caller-owned slices and allocates nothing.
//!
//! Which reduction a prototype expression uses depends on the axis:
//!
//! - a 1-D `sum`/`mean`, or a reduction over the last, contiguous axis (such as
//!   `np.mean(seg * seg, axis=1)` on a `(k, hop)` array, one row at a time), is pairwise:
//!   [`sum`], [`mean`];
//! - `sum(axis=0)` over a C-order `(n, 2)` array and a mean over a middle axis are sequential per
//!   column, and `mean(axis=1)` over two channels is `(l + r) / 2`. These are plain loops, not
//!   pairwise: for example the reverb IR normalization `np.sum(ir**2, axis=0)` and the
//!   analyzer's `np.sum(x * x, axis=0)`;
//! - `cumsum` is sequential: [`cumsum`].

/// NumPy's `PW_BLOCKSIZE`.
const PW_BLOCKSIZE: usize = 128;

/// NumPy's `pairwise_sum` kernel (`DOUBLE_pairwise_sum` in `loops_utils.h.src`).
///
/// Below 8 elements it adds sequentially, starting from `-0.0`. Up to 128 it keeps 8
/// accumulators (element `i` goes to `i % 8`), combines them as
/// `((r0 + r1) + (r2 + r3)) + ((r4 + r5) + (r6 + r7))` and adds the `n % 8` remainder
/// sequentially. Above 128 it splits at `n / 2` rounded down to a multiple of 8 and adds the two
/// halves' sums.
///
/// Use [`sum`] for `np.sum`; this is the kernel without the reduction's initial value.
#[must_use]
pub fn pairwise_sum(values: &[f64]) -> f64 {
    let n = values.len();
    if n < 8 {
        return values.iter().fold(-0.0, |acc, &v| acc + v);
    }
    if n <= PW_BLOCKSIZE {
        let (blocks, remainder) = values.as_chunks::<8>();
        let mut blocks = blocks.iter();
        // `n >= 8`, so there is a first block; it initializes the accumulators.
        let mut r = blocks.next().copied().unwrap_or([0.0; 8]);
        for block in blocks {
            for (acc, &v) in r.iter_mut().zip(block) {
                *acc += v;
            }
        }
        let [r0, r1, r2, r3, r4, r5, r6, r7] = r;
        let head = ((r0 + r1) + (r2 + r3)) + ((r4 + r5) + (r6 + r7));
        return remainder.iter().fold(head, |acc, &v| acc + v);
    }
    let half = n / 2;
    let split = half - half % 8;
    match values.split_at_checked(split) {
        Some((left, right)) => pairwise_sum(left) + pairwise_sum(right),
        // `split <= n / 2`, so the split always succeeds.
        None => values.iter().fold(-0.0, |acc, &v| acc + v),
    }
}

/// `np.sum` of a contiguous 1-D float64 array: the reduction starts from `0.0` and adds
/// [`pairwise_sum`] of all elements, so a sum of only `-0.0` is `0.0`, as in NumPy.
///
/// Prototype: the analyzer's spectrum totals (`psd[1:].sum()`, `psd[mask].sum()`,
/// `(psd * freqs).sum()`), and [`mean`].
///
/// # Examples
///
/// ```
/// use sonara_dsp::numpy::math::sum;
///
/// // Eight accumulators: ((0.2 + 0.2) + 0.4) + 0.1 + 0.1. A sequential sum is 0.9999999999999999.
/// assert_eq!(sum(&[0.1; 10]), 1.0);
/// assert!(sum(&[-0.0; 3]).is_sign_positive());
/// ```
#[must_use]
pub fn sum(values: &[f64]) -> f64 {
    0.0 + pairwise_sum(values)
}

/// `np.mean` of a contiguous 1-D float64 array: [`sum`] divided by the length. An empty slice
/// gives NaN, as in NumPy (which also warns).
///
/// Prototype: the analyzer's loudness (`gated.mean()`, `final.mean()`, `sub.mean()`,
/// `np.mean(10 ** ((lra + 0.691) / 10))`), its 50 ms silence frames and 100 ms sub-blocks (one
/// row of `np.mean(seg * seg, axis=1)` each), and `self._sum_sq.mean()` over the channels.
#[must_use]
pub fn mean(values: &[f64]) -> f64 {
    sum(values) / len_f64(values.len())
}

/// `np.cumsum` in place: `values[i] = values[i - 1] + values[i]`, sequentially. The first
/// element is kept as it is (`-0.0` stays `-0.0`).
///
/// Prototype: the drum sweep phase `np.cumsum(freq)`, the limiter's boxcar
/// `np.cumsum(held_ext)` and the analyzer's loudness blocks `np.cumsum(sub)`. The latter two
/// prepend a `0.0`: write the input from index 1 of a buffer whose element 0 is `0.0`, because
/// `0.0 + x0` is `x0` except for `-0.0`.
pub fn cumsum(values: &mut [f64]) {
    let mut acc: Option<f64> = None;
    for v in values {
        let next = acc.map_or(*v, |a| a + *v);
        *v = next;
        acc = Some(next);
    }
}

/// `np.linspace(start, stop, out.len())` with `endpoint=True`, into `out`.
///
/// `step = (stop - start) / (n - 1)` and element `i` is `i * step + start`, with the last
/// element set to `stop`. When the step is zero (a zero or denormal span), element `i` is
/// `(i / (n - 1)) * (stop - start) + start`, as NumPy's gh-5437 branch computes it. A single
/// element is `0 * (stop - start) + start`.
///
/// Prototype: the synth's unison offsets and pans `linspace(-d, d, U)`, the drum fade-out
/// `linspace(1, 0, fade)`, and the crossfades `linspace(0, 1, n)` of the delay and the
/// convolver.
///
/// # Examples
///
/// ```
/// use sonara_dsp::numpy::math::linspace;
///
/// let mut out = [0.0; 5];
/// linspace(0.0, 1.0, &mut out);
/// assert_eq!(out, [0.0, 0.25, 0.5, 0.75, 1.0]);
/// ```
pub fn linspace(start: f64, stop: f64, out: &mut [f64]) {
    let delta = stop - start;
    let Some(div) = out.len().checked_sub(1) else {
        return;
    };
    if div == 0 {
        for v in out.iter_mut() {
            *v = 0.0 * delta + start;
        }
        return;
    }
    let div = len_f64(div);
    // NumPy's `step`.
    let spacing = delta / div;
    for (i, v) in out.iter_mut().enumerate() {
        let i = len_f64(i);
        *v = if spacing == 0.0 {
            (i / div) * delta + start
        } else {
            i * spacing + start
        };
    }
    if let Some(last) = out.last_mut() {
        *last = stop;
    }
}

/// `np.rint`: rounds to the nearest integer, ties to even.
///
/// Prototype: the export quantizer (`np.rint(y)`). Python's `round(x)` without digits rounds the
/// same way (its integer result aside), for example the gate's hold time and the delay length.
#[must_use]
pub fn rint(x: f64) -> f64 {
    x.round_ties_even()
}

/// NumPy's `round(x, decimals)` (`np.round`, `ndarray.round`): `rint(x * 10^d) / 10^d` for
/// `d > 0`, `rint(x / 10^-d) * 10^-d` for `d < 0`, and `rint(x)` for `d = 0`. The power of ten
/// is NumPy's `power_of_ten`: a table up to 10^8, then repeated multiplication by 10.
///
/// This is not correctly rounded: `x * 10^d` rounds first. Python's `round` is [`py_round`].
///
/// Prototype: the analyzer's DC offset, `(sum / frames).round(6)`.
#[must_use]
pub fn round(x: f64, decimals: i32) -> f64 {
    let scale = power_of_ten(decimals.unsigned_abs());
    match decimals {
        0 => rint(x),
        d if d > 0 => rint(x * scale) / scale,
        _ => rint(x / scale) * scale,
    }
}

/// `power_of_ten` of `calculation.c`.
fn power_of_ten(n: u32) -> f64 {
    const TABLE: [f64; 9] = [1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8];
    if let Some(&p) = usize::try_from(n).ok().and_then(|i| TABLE.get(i)) {
        return p;
    }
    let mut ret = 1e9_f64;
    let mut k = 9;
    // The loop stops at infinity, which every later step would keep.
    while k < n && ret.is_finite() {
        ret *= 10.0;
        k += 1;
    }
    ret
}

/// Python's `NDIGITS_MAX`, `int((DBL_MANT_DIG - DBL_MIN_EXP) * 0.30103)`: with more digits a
/// float rounds to itself.
const PY_NDIGITS_MAX: u32 = 323;

/// Python's `round(x, ndigits)` for `ndigits >= 0`: the exact binary value of `x` correctly
/// rounded to `ndigits` decimals (ties to even), then parsed back. NaN and infinities round to
/// themselves, and so does every `x` for `ndigits > 323`. The sign is kept, so small negative
/// values round to `-0.0`.
///
/// Rust's `{:.N}` formatting is exact and rounds ties to even, like Python's `_Py_dg_dtoa` in
/// mode 3, and `str::parse` is correctly rounded like `_Py_dg_strtod`. The text goes into a
/// stack buffer, so nothing allocates.
///
/// Prototype: the reverb's IR key (`round(decay, 3)`, `round(predelay, 1)`, …), whose rounded
/// values are the arguments of `synthesize_ir`; the analyzer's loudness series (`round(lufs,
/// 2)`) and silence regions (`round(t, 3)`); the export report (`round(gain_db, 3)`,
/// `round(lufs, 2)`, `round(kbps, 1)`).
///
/// # Examples
///
/// ```
/// use sonara_dsp::numpy::math::py_round;
///
/// assert_eq!(py_round(2.675, 2), 2.67); // 2.675 is 2.67499999… in binary
/// assert_eq!(py_round(0.125, 2), 0.12); // an exact tie goes to even
/// assert!(py_round(-0.001, 2).is_sign_negative());
/// ```
#[must_use]
pub fn py_round(x: f64, ndigits: u32) -> f64 {
    if !x.is_finite() || ndigits > PY_NDIGITS_MAX {
        return x;
    }
    let mut text = DecimalBuffer::new();
    if core::fmt::write(
        &mut text,
        format_args!("{x:.prec$}", prec = ndigits as usize),
    )
    .is_err()
    {
        // The buffer holds the longest possible text (see `DecimalBuffer`).
        return x;
    }
    text.as_str().and_then(|s| s.parse().ok()).unwrap_or(x)
}

/// A fixed buffer for one formatted float: a sign, the 309 integer digits of `f64::MAX`, the
/// point and [`PY_NDIGITS_MAX`] decimals fit in 634 bytes.
struct DecimalBuffer {
    bytes: [u8; 640],
    len: usize,
}

impl DecimalBuffer {
    fn new() -> Self {
        Self {
            bytes: [0; 640],
            len: 0,
        }
    }

    fn as_str(&self) -> Option<&str> {
        self.bytes
            .get(..self.len)
            .and_then(|b| core::str::from_utf8(b).ok())
    }
}

impl core::fmt::Write for DecimalBuffer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len.checked_add(s.len()).ok_or(core::fmt::Error)?;
        let slot = self.bytes.get_mut(self.len..end).ok_or(core::fmt::Error)?;
        for (dst, &src) in slot.iter_mut().zip(s.as_bytes()) {
            *dst = src;
        }
        self.len = end;
        Ok(())
    }
}

/// `np.percentile(values, q)` with the default `linear` method, for `0 <= q <= 100`.
///
/// **Sorts `values` in place.** The virtual index is `(n - 1) * (q / 100)`; with `prev =
/// floor(index)`, `next = prev + 1` and `t = index - prev`, the result is NumPy's `_lerp`: `a +
/// (b - a) * t` for `t < 0.5` and `b - (b - a) * (1 - t)` for `t >= 0.5`. An index at or past the
/// last element uses the last element for both ends, with `t = index + 1` as NumPy computes it
/// (the end index is `-1`). Any NaN in `values` gives NaN.
///
/// Returns `None` for an empty slice or a `q` outside `[0, 100]` (NumPy raises).
///
/// Prototype: the analyzer's loudness range, `percentile(lra, 95) - percentile(lra, 10)`.
///
/// # Examples
///
/// ```
/// use sonara_dsp::numpy::math::percentile;
///
/// let mut values = [4.0, 1.0, 3.0, 2.0];
/// assert_eq!(percentile(&mut values, 50.0), Some(2.5));
/// assert_eq!(values, [1.0, 2.0, 3.0, 4.0]);
/// ```
pub fn percentile(values: &mut [f64], q: f64) -> Option<f64> {
    if values.is_empty() || !(0.0..=100.0).contains(&q) {
        return None;
    }
    if values.iter().any(|v| v.is_nan()) {
        return Some(f64::NAN);
    }
    // NaN is excluded above, so `total_cmp` orders like `<` (it puts -0.0 before 0.0, where
    // NumPy's partition leaves equal values in an unspecified order).
    values.sort_unstable_by(f64::total_cmp);
    let last = values.len() - 1;
    let index = len_f64(last) * (q / 100.0);
    let (a, b, t) = if index >= len_f64(last) {
        let end = values.last().copied()?;
        (end, end, index + 1.0)
    } else {
        let prev = index.floor();
        let lower = floor_index(prev);
        let a = values.get(lower).copied()?;
        let b = values.get(lower + 1).copied()?;
        (a, b, index - prev)
    };
    Some(lerp(a, b, t))
}

/// NumPy's `_lerp(a, b, t)`.
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    let diff = b - a;
    if t >= 0.5 {
        b - diff * (1.0 - t)
    } else {
        a + diff * t
    }
}

/// A non-negative, integral float below the slice length as an index.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is floor((n - 1) * q) with 0 <= q < 1, an exact index below n - 1"
)]
fn floor_index(value: f64) -> usize {
    value as usize
}

/// A length or an index as a float, exactly for every length below 2^53.
#[expect(
    clippy::cast_precision_loss,
    reason = "slice lengths are far below 2^53, exactly representable"
)]
fn len_f64(len: usize) -> f64 {
    len as f64
}

#[cfg(test)]
mod tests {
    use super::{
        cumsum, linspace, mean, pairwise_sum, percentile, power_of_ten, py_round, rint, round, sum,
    };

    /// A sequential reference for the small cases: `-0.0 + v0 + v1 + …`.
    fn sequential(values: &[f64]) -> f64 {
        values.iter().fold(-0.0, |acc, &v| acc + v)
    }

    #[test]
    fn pairwise_sum_small_and_unrolled() {
        let values: Vec<f64> = (0..20).map(|i| 0.1 * f64::from(i) + 1e-17).collect();
        let (seven, _) = values.split_at(7);
        assert_eq!(pairwise_sum(seven), sequential(seven));
        // 20 elements: two blocks of 8 into 8 accumulators, then 4 sequentially.
        let (first, rest) = values.split_at(8);
        let (second, remainder) = rest.split_at(8);
        let mut r = [0.0; 8];
        for ((acc, a), b) in r.iter_mut().zip(first).zip(second) {
            *acc = a + b;
        }
        let [r0, r1, r2, r3, r4, r5, r6, r7] = r;
        let head = ((r0 + r1) + (r2 + r3)) + ((r4 + r5) + (r6 + r7));
        let expected = remainder.iter().fold(head, |acc, v| acc + v);
        assert_eq!(pairwise_sum(&values), expected);
    }

    #[test]
    fn pairwise_sum_splits_at_a_multiple_of_eight() {
        let values: Vec<f64> = (0..300).map(|i| (f64::from(i) * 0.37).sin()).collect();
        // n = 300: n / 2 = 150, minus 150 % 8 = 144.
        let (left, right) = values.split_at(144);
        assert_eq!(
            pairwise_sum(&values),
            pairwise_sum(left) + pairwise_sum(right)
        );
    }

    #[test]
    fn signed_zeros_follow_numpy() {
        assert!(pairwise_sum(&[-0.0; 3]).is_sign_negative());
        assert!(sum(&[-0.0; 3]).is_sign_positive());
        assert!(sum(&[]).is_sign_positive());
        assert!(mean(&[]).is_nan());
        let mut values = [-0.0, -0.0];
        cumsum(&mut values);
        assert!(values.iter().all(|v| v.is_sign_negative()));
    }

    #[test]
    fn cumsum_is_sequential() {
        let mut values = [0.1, 0.2, 0.3, 1e16, -1e16];
        cumsum(&mut values);
        assert_eq!(values, [0.1, 0.1 + 0.2, 0.1 + 0.2 + 0.3, 1e16, 0.0]);
    }

    #[test]
    fn linspace_edge_cases() {
        let mut one = [9.0];
        linspace(-0.0, 1.0, &mut one);
        let [only] = one;
        assert!(only == 0.0 && only.is_sign_positive());
        let mut zero_span = [9.0; 3];
        linspace(-0.0, 0.0, &mut zero_span);
        assert_eq!(zero_span, [0.0, 0.0, 0.0]);
        let mut empty: [f64; 0] = [];
        linspace(0.0, 1.0, &mut empty);
        let mut denormal = [0.0; 4];
        linspace(0.0, 5e-324, &mut denormal);
        assert_eq!(denormal, [0.0, 0.0, 5e-324, 5e-324]);
    }

    #[test]
    fn rounding() {
        assert_eq!(rint(2.5), 2.0);
        assert_eq!(rint(-3.5), -4.0);
        // NumPy 2.5.4: np.round(0.1234565, 6) and np.round(0.015, 2); Python rounds 0.015 to 0.01.
        assert_eq!(round(0.123_456_5, 6), 0.123_456);
        assert_eq!(round(0.015, 2), 0.02);
        assert_eq!(py_round(0.015, 2), 0.01);
        assert_eq!(round(1250.0, -2), 1200.0);
        assert_eq!(round(1350.0, -2), 1400.0);
        assert_eq!(power_of_ten(22), 1e22);
        assert!(power_of_ten(400).is_infinite());
        assert!(power_of_ten(u32::MAX).is_infinite());
        assert_eq!(py_round(0.5, 0), 0.0);
        assert_eq!(py_round(1.5, 0), 2.0);
        assert_eq!(py_round(2.5, 0), 2.0);
        assert_eq!(py_round(0.007_812_5, 6), 0.007_812);
        assert_eq!(py_round(0.023_437_5, 6), 0.023_438);
        assert_eq!(py_round(f64::MAX, 323), f64::MAX);
        assert_eq!(py_round(-f64::MAX, 323), -f64::MAX);
        assert_eq!(py_round(5e-324, 323), 0.0);
        assert_eq!(py_round(1.234e-20, 22), 1.23e-20);
        assert!(py_round(f64::NAN, 2).is_nan());
        assert_eq!(py_round(f64::NEG_INFINITY, 2), f64::NEG_INFINITY);
    }

    #[test]
    fn percentile_bounds_and_nan() {
        assert_eq!(percentile(&mut [], 50.0), None);
        assert_eq!(percentile(&mut [1.0], 101.0), None);
        assert_eq!(percentile(&mut [1.0], f64::NAN), None);
        assert_eq!(percentile(&mut [7.0], 50.0), Some(7.0));
        assert_eq!(percentile(&mut [3.0, 1.0, 2.0], 100.0), Some(3.0));
        assert_eq!(percentile(&mut [3.0, 1.0, 2.0], 0.0), Some(1.0));
        assert!(percentile(&mut [1.0, f64::NAN], 10.0).unwrap().is_nan());
    }
}
