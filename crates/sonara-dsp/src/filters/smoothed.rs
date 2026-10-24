//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! Linear parameter ramps: the prototype's `SmoothedValue` (`dsp/base.py`).

/// The prototype's default ramp length in samples.
const DEFAULT_RAMP: u32 = 512;

/// What [`SmoothedValue::next_block`] produced. The prototype returns a Python float when the
/// value is steady and an array while it ramps, and its callers take different arithmetic paths
/// for the two (a scalar product versus a per-sample one), so the distinction is kept.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Smoothed {
    /// The value is steady; the output slice was not written.
    Steady(f64),
    /// The output slice holds one value per sample.
    Ramp,
}

/// Linear ramp toward a target over a fixed number of samples, for gain-like parameters.
///
/// Semantics of the prototype, kept exactly:
///
/// - Created without a value ([`new`](Self::new)), the first [`set_target`](Self::set_target)
///   applies immediately. Created with a value ([`with_value`](Self::with_value)), it ramps from
///   that value.
/// - A new target (`!=` the current target, so a NaN target always restarts) starts a ramp of
///   the configured length from the current value: `step = (target − current) / ramp`. An equal
///   target changes nothing, even mid-ramp.
/// - Ramp sample `k` (from 1) is `start + step · k`, from the value at the start of the call, not
///   accumulated. When the ramp ends inside a block, the value becomes the target exactly, and
///   the rest of the block is the target; the last ramp sample itself is not forced to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmoothedValue {
    current: f64,
    target: f64,
    initialized: bool,
    /// Ramp length in samples, at least 1.
    ramp: u32,
    /// Samples left in the running ramp.
    remaining: u32,
    step: f64,
}

impl Default for SmoothedValue {
    fn default() -> Self {
        Self::new()
    }
}

impl SmoothedValue {
    /// A value without an initial value (`SmoothedValue()`): 0.0 until the first target, which
    /// applies immediately. The ramp is 512 samples.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: 0.0,
            target: 0.0,
            initialized: false,
            ramp: DEFAULT_RAMP,
            remaining: 0,
            step: 0.0,
        }
    }

    /// A value that starts at `value` and ramps over `ramp_samples` (at least 1) to new targets
    /// (`SmoothedValue(value, ramp_samples)`).
    #[must_use]
    pub fn with_value(value: f64, ramp_samples: u32) -> Self {
        Self {
            current: value,
            target: value,
            initialized: true,
            ramp: ramp_samples.max(1),
            remaining: 0,
            step: 0.0,
        }
    }

    /// Sets the length of later ramps (at least 1); a running ramp keeps its step.
    pub fn set_ramp(&mut self, samples: u32) {
        self.ramp = samples.max(1);
    }

    /// Ramps to `value`, or jumps to it when no value was set yet.
    #[expect(
        clippy::float_cmp,
        reason = "the prototype restarts the ramp exactly when `value != target`"
    )]
    pub fn set_target(&mut self, value: f64) {
        if !self.initialized {
            self.set_immediate(value);
            return;
        }
        if value != self.target {
            self.target = value;
            self.remaining = self.ramp;
            self.step = (value - self.current) / f64::from(self.ramp);
        }
    }

    /// Jumps to `value` and ends any ramp (`set_target(value, immediate=True)`).
    pub fn set_immediate(&mut self, value: f64) {
        self.initialized = true;
        self.current = value;
        self.target = value;
        self.remaining = 0;
    }

    /// The value after the last block.
    #[must_use]
    pub fn current(&self) -> f64 {
        self.current
    }

    /// The value the ramp moves to.
    #[must_use]
    pub fn target(&self) -> f64 {
        self.target
    }

    /// Whether a ramp is running.
    #[must_use]
    pub fn is_smoothing(&self) -> bool {
        self.remaining > 0
    }

    /// Advances by `out.len()` samples. Steady: returns [`Smoothed::Steady`] and leaves `out`
    /// alone. Ramping: writes one value per sample into `out` and returns [`Smoothed::Ramp`].
    ///
    /// An empty `out` while ramping (which the prototype does not support) returns
    /// [`Smoothed::Ramp`] and changes nothing.
    pub fn next_block(&mut self, out: &mut [f64]) -> Smoothed {
        if self.remaining == 0 {
            return Smoothed::Steady(self.current);
        }
        if out.is_empty() {
            return Smoothed::Ramp;
        }
        let start = self.current;
        let steps = u32::try_from(out.len()).map_or(self.remaining, |n| n.min(self.remaining));
        let mut last = start;
        for (k, y) in (1..=steps).zip(out.iter_mut()) {
            last = start + self.step * f64::from(k);
            *y = last;
        }
        self.remaining -= steps;
        if self.remaining == 0 {
            self.current = self.target;
            for y in out
                .iter_mut()
                .skip(usize::try_from(steps).unwrap_or(usize::MAX))
            {
                *y = self.current;
            }
        } else {
            self.current = last;
        }
        Smoothed::Ramp
    }
}

#[cfg(test)]
mod tests {
    use super::{Smoothed, SmoothedValue};

    #[test]
    fn first_target_applies_immediately_without_a_value() {
        let mut s = SmoothedValue::new();
        s.set_target(0.5);
        assert!(!s.is_smoothing());
        let mut out = [9.0; 4];
        assert_eq!(s.next_block(&mut out), Smoothed::Steady(0.5));
        assert_eq!(out, [9.0; 4], "a steady block leaves the slice alone");
    }

    #[test]
    fn ramps_from_the_start_value_and_lands_on_the_target() {
        let mut s = SmoothedValue::with_value(0.0, 4);
        s.set_target(1.0);
        let mut out = [0.0; 6];
        assert_eq!(s.next_block(&mut out), Smoothed::Ramp);
        assert_eq!(out, [0.25, 0.5, 0.75, 1.0, 1.0, 1.0]);
        assert_eq!(s.current(), 1.0);
        assert!(!s.is_smoothing());
        assert_eq!(s.next_block(&mut out), Smoothed::Steady(1.0));
    }

    #[test]
    fn a_ramp_continues_across_blocks_and_restarts_mid_ramp() {
        let mut s = SmoothedValue::with_value(1.0, 4);
        s.set_target(0.0);
        let mut out = [0.0; 2];
        s.next_block(&mut out);
        assert_eq!(out, [0.75, 0.5]);
        assert_eq!(s.current(), 0.5);
        s.set_target(0.0);
        assert!(s.is_smoothing(), "an equal target changes nothing");
        // A new target restarts the full ramp from the mid-ramp value.
        s.set_target(1.0);
        let mut out = [0.0; 4];
        s.next_block(&mut out);
        assert_eq!(out, [0.625, 0.75, 0.875, 1.0]);
    }

    #[test]
    fn immediate_and_ramp_settings() {
        let mut s = SmoothedValue::with_value(0.0, 0);
        s.set_target(2.0);
        let mut out = [0.0; 2];
        s.next_block(&mut out);
        assert_eq!(out, [2.0, 2.0], "a ramp of 0 samples is 1 sample");
        s.set_ramp(2);
        s.set_target(4.0);
        s.set_immediate(-1.0);
        assert_eq!((s.current(), s.target()), (-1.0, -1.0));
        assert!(!s.is_smoothing());
        s.set_target(1.0);
        assert_eq!(s.next_block(&mut []), Smoothed::Ramp);
        assert_eq!(s.current(), -1.0, "an empty block changes nothing");
        assert_eq!(SmoothedValue::default(), SmoothedValue::new());
    }
}
