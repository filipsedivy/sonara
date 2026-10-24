//! Real-time: runs on the audio render thread (see docs/audio-engine.md); `prepare` is setup.
//!
//! `sonara.dynamics.limiter`: a look-ahead brickwall limiter with optional true-peak
//! detection, as the prototype's `dsp/dynamics/limiter.py` computes it. Per block of `n` frames
//! with look-ahead `la`:
//!
//! 1. `x = buffer · input_gain` (only when the gain is not 1) and the sample peak
//!    `max(|l|, |r|)`. In true-peak mode the peak is `max(tp, sample peak delayed by 6)`, where
//!    `tp` is the largest magnitude of the 4× interpolated signal (`_Upsampler`: 12 frames of
//!    history, zero-stuffing, the 48 taps of [`limiter_taps`], the largest of each group of 4).
//! 2. The required gain `min(1, ceiling / max(peak, 1e-12))`, the ceiling lowered by 0.1 dB in
//!    true-peak mode.
//! 3. The held gain: the minimum over the backward window `[i − la, i]` of the required gains,
//!    the last `la` of them carried as history (`minimum_filter1d(…, size = la + 1, origin =
//!    la // 2)[la:]`).
//! 4. A boxcar over the same window, as differences of a cumulative sum that restarts every
//!    block from the held history (`csum = [0, cumsum(held_ext)]`), so its rounding depends on
//!    the block size like the prototype's.
//! 5. `gain = ar_smooth(avg, release, 0)`: an instant attack and a one-pole release.
//! 6. The audio delayed by the latency, times the gain, clipped to ±ceiling.
//!
//! Exactness: everything except the true-peak FIR is plain
//! arithmetic in the prototype's order and the kernels' FMA forms. The FIR is `SciPy`'s
//! `lfilter(taps, [1])`, which runs `np.convolve` on Accelerate's SIMD dot product; no
//! sequential order reproduces it, so true-peak values are ulp-level and platform
//! dependent. Here each interpolated sample sums its 12 non-zero products in the dot product's
//! nominal order.
//!
//! Memory: `prepare` sizes every buffer for the longest look-ahead the parameter range allows
//! (10 ms, `round(0.01 · sr)` frames, plus the detector's delay of 6). A look-ahead or
//! true-peak change in `set_params` then resets the buffers in place, without allocating, where
//! the prototype reallocates them. Kept quirk (section 5): that reset happens mid-render without
//! a new delay compensation in the render graph.

use super::params::{flag, float};
use super::pyops::{clip, maximum, minimum, py_max};
use super::{DEFAULT_SPEC, store_params};
use crate::filters::{db_to_amp, time_coefficient};
use crate::kernels::ar_smooth;
use crate::numpy::math::cumsum;
use crate::processor::{BlockContext, ParamSlots, ProcessSpec, Processor, Report};
use crate::tables::limiter_taps;

/// The plugin this processor implements.
pub(crate) const PLUGIN_ID: &str = "sonara.dynamics.limiter";

// Slot indices, in the descriptor's parameter order.
const CEILING_DB: usize = 0;
const INPUT_GAIN_DB: usize = 1;
const RELEASE_MS: usize = 2;
const LOOKAHEAD_MS: usize = 3;
const TRUE_PEAK: usize = 4;

/// The longest look-ahead the descriptor allows, in milliseconds (the range of `lookahead_ms`).
const MAX_LOOKAHEAD_MS: f64 = 10.0;
/// The look-ahead before the first parameters, in frames (`_lookahead`).
const INITIAL_LOOKAHEAD: usize = 96;
/// The true-peak detector's oversampling (`OVERSAMPLE`).
const OVERSAMPLE: usize = 4;
/// The true-peak FIR's length.
const TAPS: usize = 48;
/// Input frames the interpolator keeps (`len(taps) // OVERSAMPLE`).
const HISTORY: usize = TAPS / OVERSAMPLE;
/// The interpolator's delay in input frames, `round((48 − 1) / 2 / 4)` (`_Upsampler.delay`).
const TRUE_PEAK_DELAY: usize = 6;
/// How far below the ceiling true-peak mode aims, in dB (`TRUE_PEAK_MARGIN_DB`).
const TRUE_PEAK_MARGIN_DB: f64 = 0.1;
/// The peak floor before the division, and the gain floor before the logarithm.
const FLOOR: f64 = 1e-12;

/// `max(1, round(ms · 0.001 · sr))`: a look-ahead in frames (Python's `round`).
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Python's round() of a non-negative frame count; NaN and negatives give 1"
)]
fn lookahead_frames(ms: f64, sample_rate: f64) -> usize {
    ((ms * 0.001 * sample_rate).round_ties_even() as usize).max(1)
}

/// The decoded parameter set.
struct Settings {
    ceiling_db: f64,
    input_gain_db: f64,
    release_ms: f64,
    lookahead_ms: f64,
    true_peak: bool,
}

impl Settings {
    fn from_slots(slots: &ParamSlots) -> Option<Self> {
        Some(Self {
            ceiling_db: float(slots, CEILING_DB)?,
            input_gain_db: float(slots, INPUT_GAIN_DB)?,
            release_ms: float(slots, RELEASE_MS)?,
            lookahead_ms: float(slots, LOOKAHEAD_MS)?,
            true_peak: flag(slots, TRUE_PEAK)?,
        })
    }
}

/// The limiter (the prototype's `Limiter`).
pub(crate) struct Limiter {
    spec: ProcessSpec,
    params: ParamSlots,
    /// Whether parameters were applied; the prototype cannot process before.
    configured: bool,
    /// The look-ahead in frames (`_lookahead`), at most `capacity`.
    lookahead: usize,
    true_peak: bool,
    ceiling: f64,
    input_gain: f64,
    release: f64,
    /// The largest gain reduction of the last block, in dB (`_max_gr`).
    max_gr: f64,
    /// The release smoother's state (`_gain_state`).
    gain_state: f64,
    /// The longest look-ahead at this rate, in frames: the buffers' sizing.
    capacity: usize,
    /// The audio delay as a ring (`_delay`); its first `delay_len` frames are in use.
    delay: Vec<[f64; 2]>,
    /// The delay in frames, the latency at the last buffer reset (`_delay.shape[0]`).
    delay_len: usize,
    /// The oldest frame of the ring.
    delay_pos: usize,
    /// `req_ext`: the last `la` required gains (`_req_hist`), then this block's.
    required: Vec<f64>,
    /// `held_ext`: the last `la` held gains (`_held_hist`), then this block's.
    held: Vec<f64>,
    /// `[0, cumsum(held_ext)]`.
    csum: Vec<f64>,
    /// The sliding minimum's candidates: a ring of indices into `required`, `capacity + 1`.
    window: Vec<usize>,
    /// The interpolator's input: 12 frames of history (`_Upsampler.history`), then the block.
    padded: Vec<[f64; 2]>,
    /// The sample peaks the interpolator delays (`_peak_hist`), as a ring.
    peak_history: [f64; TRUE_PEAK_DELAY],
    peak_pos: usize,
    /// The block after the input gain (`x`).
    scaled: Vec<[f64; 2]>,
    /// The peaks, then the boxcar average.
    level: Vec<f64>,
    /// The gain per frame.
    gain: Vec<f64>,
}

impl Limiter {
    /// Setup: an unprepared processor.
    pub(crate) fn new() -> Self {
        Self {
            spec: DEFAULT_SPEC,
            params: ParamSlots::EMPTY,
            configured: false,
            lookahead: INITIAL_LOOKAHEAD,
            true_peak: false,
            ceiling: 1.0,
            input_gain: 1.0,
            release: 0.0,
            max_gr: 0.0,
            gain_state: 1.0,
            capacity: lookahead_frames(MAX_LOOKAHEAD_MS, f64::from(DEFAULT_SPEC.sample_rate)),
            delay: Vec::new(),
            delay_len: 0,
            delay_pos: 0,
            required: Vec::new(),
            held: Vec::new(),
            csum: Vec::new(),
            window: Vec::new(),
            padded: Vec::new(),
            peak_history: [0.0; TRUE_PEAK_DELAY],
            peak_pos: 0,
            scaled: Vec::new(),
            level: Vec::new(),
            gain: Vec::new(),
        }
    }

    /// The prototype's `on_params_changed`.
    fn apply_params(&mut self) {
        let Some(p) = Settings::from_slots(&self.params) else {
            return;
        };
        let sr = f64::from(self.spec.sample_rate);
        // Validated parameters stay within the capacity; the clamp keeps others in bounds.
        let lookahead = lookahead_frames(p.lookahead_ms, sr).min(self.capacity);
        if lookahead != self.lookahead
            || p.true_peak != self.true_peak
            || self.delay_len != self.latency()
        {
            self.lookahead = lookahead;
            self.true_peak = p.true_peak;
            self.reset_upsampler();
            self.reset_buffers();
        }
        // In true-peak mode aim 0.1 dB under the ceiling: the 4× estimate can under-read
        // inter-sample peaks slightly.
        let margin = if p.true_peak {
            TRUE_PEAK_MARGIN_DB
        } else {
            0.0
        };
        self.ceiling = db_to_amp(p.ceiling_db - margin);
        self.input_gain = db_to_amp(p.input_gain_db);
        self.release = time_coefficient(p.release_ms, sr);
        self.configured = true;
    }

    /// `_reset_buffers`: a silent delay of the current latency, histories of unity gain and a
    /// unity release state. In place: the buffers have the capacity of the longest look-ahead.
    fn reset_buffers(&mut self) {
        let la = self.lookahead;
        self.delay_len = self.latency().min(self.delay.len());
        self.delay.fill([0.0; 2]);
        self.delay_pos = 0;
        for history in [&mut self.required, &mut self.held] {
            for v in history.iter_mut().take(la) {
                *v = 1.0;
            }
        }
        self.gain_state = 1.0;
    }

    /// A new `_Upsampler`: zero history and zero delayed peaks.
    fn reset_upsampler(&mut self) {
        for frame in self.padded.iter_mut().take(HISTORY) {
            *frame = [0.0; 2];
        }
        self.peak_history = [0.0; TRUE_PEAK_DELAY];
        self.peak_pos = 0;
    }

    /// `_Upsampler.peaks`: replaces the sample peaks in `level[..n]` by `max(tp, the sample
    /// peak 6 frames earlier)`, where `tp` is the largest magnitude of the 4× interpolation of
    /// frame `t` (`scaled[..n]`) over both channels.
    fn true_peaks(&mut self, n: usize) {
        let taps = limiter_taps();
        let (Some(input), Some(levels)) = (self.scaled.get(..n), self.level.get_mut(..n)) else {
            return;
        };
        for (slot, &frame) in self.padded.iter_mut().skip(HISTORY).zip(input) {
            *slot = frame;
        }
        // Window `t` is `padded[t + 1 ..= t + 12]`: input frames `t − 11 ..= t`.
        let windows = self.padded.windows(HISTORY).skip(1);
        for (window, level) in windows.zip(levels.iter_mut()) {
            let mut tp = f64::NEG_INFINITY;
            for phase in 0..OVERSAMPLE {
                // Interpolated sample `4t + phase`: `sum_j taps[j] · up[48 + 4t + phase − j]`,
                // whose non-zero terms are `taps[phase + 4i] · x[t − i]`; the dot product runs
                // over the input from the oldest frame on.
                let mut acc = [0.0; 2];
                let phase_taps = taps.iter().skip(phase).step_by(OVERSAMPLE).rev();
                for (&[l, r], &tap) in window.iter().zip(phase_taps) {
                    acc = [acc[0] + tap * l, acc[1] + tap * r];
                }
                let magnitude = maximum(acc[0].abs(), acc[1].abs());
                tp = if phase == 0 {
                    magnitude
                } else {
                    maximum(tp, magnitude)
                };
            }
            let delayed = self.peak_history.get(self.peak_pos).copied().unwrap_or(0.0);
            if let Some(slot) = self.peak_history.get_mut(self.peak_pos) {
                *slot = *level;
            }
            self.peak_pos = (self.peak_pos + 1) % TRUE_PEAK_DELAY;
            *level = maximum(tp, delayed);
        }
        // `history = padded[-12:]`.
        if let Some(end) = n.checked_add(HISTORY).filter(|&e| e <= self.padded.len()) {
            self.padded.copy_within(n..end, 0);
        }
    }

    /// Steps 3 and 4 for `n` frames: the held gains into `held[la..la + n]` and their boxcar
    /// average into `level[..n]`; then both histories move to the block's last `la` values.
    #[expect(
        clippy::cast_precision_loss,
        reason = "a window of at most 1921 frames is exact in f64"
    )]
    fn hold_and_average(&mut self, n: usize) {
        let la = self.lookahead;
        let Some(extended) = la.checked_add(n) else {
            return;
        };
        let (Some(required), Some(held), Some(csum)) = (
            self.required.get(..extended),
            self.held.get_mut(..extended),
            self.csum.get_mut(..=extended),
        ) else {
            return;
        };
        sliding_minimum(
            required,
            la,
            held.get_mut(la..).unwrap_or_default(),
            &mut self.window,
        );
        // `csum = concat([0], cumsum(held_ext))`.
        let Some((zero, sums)) = csum.split_first_mut() else {
            return;
        };
        *zero = 0.0;
        for (s, &h) in sums.iter_mut().zip(held.iter()) {
            *s = h;
        }
        cumsum(sums);
        // `avg = (csum[la + 1:] − csum[:n]) / (la + 1)`.
        let width = (la + 1) as f64;
        let upper = csum.iter().skip(la + 1);
        for ((avg, &hi), &lo) in self.level.iter_mut().take(n).zip(upper).zip(csum.iter()) {
            *avg = (hi - lo) / width;
        }
        for history in [&mut self.required, &mut self.held] {
            if let Some(end) = n.checked_add(la).filter(|&e| e <= history.len()) {
                history.copy_within(n..end, 0);
            }
        }
    }
}

/// `out[i] = min(ext[i ..= i + la])` for every `i < out.len()`: a monotonic deque over `ext`,
/// with `window` as its ring of candidate indices (at least `la + 1` long). A minimum is one of
/// its inputs, so the values are `ndimage`'s exactly (for finite inputs).
fn sliding_minimum(ext: &[f64], la: usize, out: &mut [f64], window: &mut [usize]) {
    let capacity = window.len();
    if capacity <= la {
        return;
    }
    let wrap = |index: usize| {
        if index >= capacity {
            index - capacity
        } else {
            index
        }
    };
    let (mut head, mut len) = (0, 0);
    let mut outputs = out.iter_mut();
    for (j, &value) in ext.iter().enumerate() {
        // Drop the candidates that left the window `[j − la, j]`.
        while len > 0 {
            match window.get(head) {
                Some(&front) if j - front > la => {
                    head = wrap(head + 1);
                    len -= 1;
                }
                _ => break,
            }
        }
        // Drop the candidates the new value dominates; at most `la` remain.
        while len > 0 {
            let back = window.get(wrap(head + len - 1)).copied().unwrap_or(j);
            if ext.get(back).is_some_and(|&candidate| candidate >= value) {
                len -= 1;
            } else {
                break;
            }
        }
        if let Some(slot) = window.get_mut(wrap(head + len)) {
            *slot = j;
            len += 1;
        }
        if j >= la
            && let Some(y) = outputs.next()
        {
            *y = window
                .get(head)
                .and_then(|&front| ext.get(front))
                .copied()
                .unwrap_or(value);
        }
    }
}

impl Processor for Limiter {
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    fn prepare(&mut self, spec: ProcessSpec) {
        self.spec = spec;
        let block = spec.block_size;
        self.capacity = lookahead_frames(MAX_LOOKAHEAD_MS, f64::from(spec.sample_rate));
        let extended = self.capacity.saturating_add(block);
        self.delay = vec![[0.0; 2]; self.capacity.saturating_add(TRUE_PEAK_DELAY)];
        self.required = vec![1.0; extended];
        self.held = vec![1.0; extended];
        self.csum = vec![0.0; extended.saturating_add(1)];
        self.window = vec![0; self.capacity.saturating_add(1)];
        self.padded = vec![[0.0; 2]; HISTORY.saturating_add(block)];
        self.scaled = vec![[0.0; 2]; block];
        self.level = vec![0.0; block];
        self.gain = vec![0.0; block];
        // Before the parameters the look-ahead must fit the new capacity too.
        self.lookahead = self.lookahead.min(self.capacity);
        self.reset();
        self.apply_params();
    }

    fn set_params(&mut self, params: &ParamSlots) {
        if store_params(&mut self.params, params) {
            self.apply_params();
        }
    }

    #[expect(
        clippy::float_cmp,
        reason = "the prototype skips the input gain exactly when it is 1.0"
    )]
    fn process(&mut self, buffer: &mut [[f64; 2]], _ctx: &BlockContext<'_>) {
        if !self.configured {
            return;
        }
        let n = buffer.len().min(self.scaled.len());
        let (Some(buffer), Some(scaled), Some(level)) = (
            buffer.get_mut(..n),
            self.scaled.get_mut(..n),
            self.level.get_mut(..n),
        ) else {
            return;
        };
        if n == 0 {
            return;
        }
        let g = self.input_gain;
        for ((x, &[l, r]), peak) in scaled.iter_mut().zip(buffer.iter()).zip(level.iter_mut()) {
            *x = if g == 1.0 { [l, r] } else { [l * g, r * g] };
            *peak = maximum(x[0].abs(), x[1].abs());
        }
        if self.true_peak {
            self.true_peaks(n);
        }
        // `req = minimum(1, ceiling / maximum(peak, 1e-12))` after the history.
        let la = self.lookahead;
        let required = self.required.iter_mut().skip(la).take(n);
        for (req, &peak) in required.zip(self.level.iter()) {
            *req = minimum(1.0, self.ceiling / maximum(peak, FLOOR));
        }
        self.hold_and_average(n);
        let (Some(avg), Some(gain)) = (self.level.get(..n), self.gain.get_mut(..n)) else {
            return;
        };
        ar_smooth(avg, gain, self.release, 0.0, &mut self.gain_state);
        let ceiling = self.ceiling;
        let len = self.delay_len;
        for ((frame, &x), &g) in buffer.iter_mut().zip(self.scaled.iter()).zip(gain.iter()) {
            // The delay line: the oldest frame out, this one in.
            let delayed = match self.delay.get_mut(self.delay_pos).filter(|_| len > 0) {
                Some(slot) => {
                    let out = *slot;
                    *slot = x;
                    self.delay_pos += 1;
                    if self.delay_pos >= len {
                        self.delay_pos = 0;
                    }
                    out
                }
                None => x,
            };
            *frame = [
                clip(delayed[0] * g, -ceiling, ceiling),
                clip(delayed[1] * g, -ceiling, ceiling),
            ];
        }
        // `gain.min()` propagates NaN, like NumPy's reduction.
        let smallest = gain.iter().copied().reduce(minimum).unwrap_or(1.0);
        self.max_gr = -20.0 * py_max(smallest, FLOOR).log10();
    }

    /// `_reset_buffers` and a new `_Upsampler`.
    fn reset(&mut self) {
        self.reset_buffers();
        self.reset_upsampler();
    }

    /// The look-ahead, plus the detector's delay of 6 in true-peak mode.
    fn latency(&self) -> usize {
        self.lookahead + if self.true_peak { TRUE_PEAK_DELAY } else { 0 }
    }

    fn report(&self) -> Report {
        Report::new(&[("gain_reduction_db", self.max_gr)])
    }
}

#[cfg(test)]
mod tests {
    use sonara_core::model::device::{ParamValue, Params};
    use sonara_core::plugins::builtin_registry;

    use super::{
        CEILING_DB, INPUT_GAIN_DB, LOOKAHEAD_MS, Limiter, MAX_LOOKAHEAD_MS, PLUGIN_ID, RELEASE_MS,
        TRUE_PEAK, lookahead_frames, sliding_minimum,
    };
    use crate::processor::{BlockContext, ParamSlots, ProcessSpec, Processor};
    use crate::processors::param_slots;

    const BLOCK: usize = 64;

    fn slots(values: &[(&str, ParamValue)]) -> ParamSlots {
        let registry = builtin_registry();
        let values: Params = values
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        param_slots(registry.get(PLUGIN_ID).unwrap(), &values).unwrap()
    }

    fn prepared(values: &[(&str, ParamValue)]) -> Limiter {
        let mut limiter = Limiter::new();
        limiter.prepare(ProcessSpec {
            sample_rate: 48_000,
            block_size: BLOCK,
            channels: 2,
        });
        limiter.set_params(&slots(values));
        limiter
    }

    fn gain_reduction(limiter: &Limiter) -> f64 {
        let report = limiter.report();
        let (name, value) = report.entries().first().copied().unwrap();
        assert_eq!(name, "gain_reduction_db");
        value
    }

    fn ctx() -> BlockContext<'static> {
        BlockContext {
            frame: 0,
            sample_rate: 48_000,
            beat: 0.0,
            bpm: 120.0,
            playing: true,
            sidechain: None,
            events: &[],
        }
    }

    #[test]
    fn slots_follow_the_descriptor_order_and_the_capacity_its_range() {
        let registry = builtin_registry();
        let descriptor = registry.get(PLUGIN_ID).unwrap();
        let ids: Vec<&str> = descriptor.params.iter().map(|p| p.id.as_str()).collect();
        let slots = [
            (CEILING_DB, "ceiling_db"),
            (INPUT_GAIN_DB, "input_gain_db"),
            (RELEASE_MS, "release_ms"),
            (LOOKAHEAD_MS, "lookahead_ms"),
            (TRUE_PEAK, "true_peak"),
        ];
        assert_eq!(ids.len(), slots.len());
        for (index, id) in slots {
            assert_eq!(ids.get(index), Some(&id));
        }
        let lookahead = descriptor.params.get(LOOKAHEAD_MS).unwrap();
        assert_eq!(
            lookahead.maximum,
            Some(MAX_LOOKAHEAD_MS),
            "the buffers' sizing"
        );
        assert_eq!(lookahead_frames(MAX_LOOKAHEAD_MS, 192_000.0), 1920);
        assert_eq!(lookahead_frames(2.0, 48_000.0), 96);
        assert_eq!(lookahead_frames(0.0, 48_000.0), 1);
    }

    #[test]
    fn the_sliding_minimum_equals_the_direct_one() {
        let ext: Vec<f64> = (0..200)
            .map(|i| (f64::from(i) * 0.731).sin().abs())
            .collect();
        for la in [0, 1, 3, 17, 96] {
            let mut out: Vec<f64> = ext.iter().skip(la).map(|_| 0.0).collect();
            let mut window: Vec<usize> = (0..=la).collect();
            sliding_minimum(&ext, la, &mut out, &mut window);
            let direct: Vec<f64> = ext
                .windows(la + 1)
                .map(|w| w.iter().copied().fold(f64::INFINITY, f64::min))
                .collect();
            assert_eq!(out, direct, "la {la}");
        }
    }

    #[test]
    fn a_loud_block_is_delayed_and_held_under_the_ceiling() {
        let mut limiter = prepared(&[("true_peak", ParamValue::Bool(false))]);
        assert_eq!(limiter.latency(), 96);
        let ceiling = 10.0_f64.powf(-1.0 / 20.0);
        let mut output = Vec::new();
        for _ in 0..4 {
            let mut block = [[0.95, -0.95]; BLOCK];
            limiter.process(&mut block, &ctx());
            output.extend_from_slice(&block);
        }
        // The first 96 frames are the silent delay line.
        assert!(output.iter().take(96).flatten().all(|&v| v == 0.0));
        // The look-ahead reaches the required gain when the loud frames come out.
        assert!(
            output
                .iter()
                .skip(96)
                .all(|f| f[0] <= ceiling && f[0] > 0.0 && f[1] >= -ceiling)
        );
        let reduction = -20.0 * (ceiling / 0.95).log10();
        assert!((gain_reduction(&limiter) - reduction).abs() < 1e-9);
    }

    #[test]
    fn a_lookahead_change_resets_in_place_without_reallocating() {
        let mut limiter = prepared(&[]);
        assert_eq!(limiter.latency(), 96 + 6, "true peak by default");
        let capacity = limiter.required.capacity();
        let mut block = [[0.5, 0.5]; BLOCK];
        limiter.process(&mut block, &ctx());
        limiter.set_params(&slots(&[
            ("lookahead_ms", ParamValue::Float(10.0)),
            ("true_peak", ParamValue::Bool(false)),
        ]));
        assert_eq!(limiter.latency(), 480);
        assert_eq!(limiter.delay_len, 480);
        assert_eq!(limiter.required.capacity(), capacity);
        assert!(
            limiter
                .required
                .iter()
                .take(480)
                .all(|g| g.to_bits() == 1.0_f64.to_bits())
        );
        let mut block = [[0.5, 0.5]; BLOCK];
        limiter.process(&mut block, &ctx());
        assert_eq!(
            block, [[0.0; 2]; BLOCK],
            "the new, longer delay starts silent"
        );
    }

    #[test]
    fn true_peak_mode_catches_an_inter_sample_peak() {
        // A sine at fs/4 with a 45° phase peaks at 1 between samples of ±0.707.
        let mut sample_peak = prepared(&[
            ("true_peak", ParamValue::Bool(false)),
            ("ceiling_db", ParamValue::Float(-1.0)),
        ]);
        let mut true_peak = prepared(&[("ceiling_db", ParamValue::Float(-1.0))]);
        let frame = |i: usize| {
            let v = (std::f64::consts::FRAC_PI_2 * f64::from(u32::try_from(i).unwrap())
                + std::f64::consts::FRAC_PI_4)
                .sin();
            [v, v]
        };
        for b in 0..8 {
            let input: Vec<[f64; 2]> = (b * BLOCK..(b + 1) * BLOCK).map(frame).collect();
            let mut a = [[0.0; 2]; BLOCK];
            let mut t = [[0.0; 2]; BLOCK];
            a.copy_from_slice(&input);
            t.copy_from_slice(&input);
            sample_peak.process(&mut a, &ctx());
            true_peak.process(&mut t, &ctx());
        }
        let sample_gr = gain_reduction(&sample_peak);
        let true_gr = gain_reduction(&true_peak);
        assert!(sample_gr < 1e-9, "0.707 is under −1 dBFS: {sample_gr}");
        assert!(
            true_gr > 1.0,
            "the interpolated peak is near 0 dBFS: {true_gr}"
        );
    }
}
