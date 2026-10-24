//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! `sonara.utility`: trim gain, stereo width (mid/side), mono, channel swap, polarity and DC
//! removal, as the prototype's `dsp/spatial/utility.py` computes them.

use std::f64::consts::PI;

use super::params::{flag, float};
use super::{DEFAULT_SPEC, ramp_length, scale, store_params};
use crate::filters::{Lfilter, Smoothed, SmoothedValue, db_to_amp};
use crate::processor::{BlockContext, ParamSlots, ProcessSpec, Processor};

/// The plugin this processor implements.
pub(crate) const PLUGIN_ID: &str = "sonara.utility";

// Slot indices, in the descriptor's parameter order.
const GAIN_DB: usize = 0;
const WIDTH: usize = 1;
const MONO: usize = 2;
const SWAP: usize = 3;
const INVERT_LEFT: usize = 4;
const INVERT_RIGHT: usize = 5;
const DC_BLOCK: usize = 6;

/// Corner frequency of the one-pole DC blocker, in Hz.
const DC_CORNER_HZ: f64 = 10.0;
/// Shortest ramp of the gain and width smoothers, in samples.
const MIN_RAMP: usize = 64;

/// The decoded parameter set.
#[expect(
    clippy::struct_excessive_bools,
    reason = "the descriptor's five independent switches"
)]
struct Settings {
    gain_db: f64,
    width: f64,
    mono: bool,
    swap: bool,
    invert_left: bool,
    invert_right: bool,
    dc_block: bool,
}

impl Settings {
    fn from_slots(slots: &ParamSlots) -> Option<Self> {
        Some(Self {
            gain_db: float(slots, GAIN_DB)?,
            width: float(slots, WIDTH)?,
            mono: flag(slots, MONO)?,
            swap: flag(slots, SWAP)?,
            invert_left: flag(slots, INVERT_LEFT)?,
            invert_right: flag(slots, INVERT_RIGHT)?,
            dc_block: flag(slots, DC_BLOCK)?,
        })
    }
}

/// The utility processor (the prototype's `Utility`).
pub(crate) struct Utility {
    spec: ProcessSpec,
    params: ParamSlots,
    gain: SmoothedValue,
    width: SmoothedValue,
    swap: bool,
    /// Polarity factors `[left, right]`, applied after the swap.
    invert: [f64; 2],
    dc_block: bool,
    /// `lfilter([1, -1], [1, -r], axis=0, zi)` with `r = 1 − 2π·10/sr`. Its coefficients depend
    /// on the sample rate only, so `prepare` builds it.
    dc_filter: Option<Lfilter>,
    /// The values of a running ramp, one block long.
    ramp: Vec<f64>,
}

impl Utility {
    /// Setup: an unprepared processor.
    pub(crate) fn new() -> Self {
        Self {
            spec: DEFAULT_SPEC,
            params: ParamSlots::EMPTY,
            gain: SmoothedValue::new(),
            width: SmoothedValue::new(),
            swap: false,
            invert: [1.0, 1.0],
            dc_block: false,
            dc_filter: None,
            ramp: Vec::new(),
        }
    }

    /// The prototype's `on_params_changed`.
    fn apply_params(&mut self) {
        let Some(p) = Settings::from_slots(&self.params) else {
            return;
        };
        let ramp = ramp_length(MIN_RAMP.max(self.spec.block_size));
        self.gain.set_ramp(ramp);
        self.width.set_ramp(ramp);
        self.gain.set_target(db_to_amp(p.gain_db));
        self.width.set_target(if p.mono { 0.0 } else { p.width });
        self.swap = p.swap;
        self.invert = [polarity(p.invert_left), polarity(p.invert_right)];
        self.dc_block = p.dc_block;
    }
}

/// The factor of an inverted (`-1`) or untouched (`1`) channel.
fn polarity(invert: bool) -> f64 {
    if invert { -1.0 } else { 1.0 }
}

impl Processor for Utility {
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    fn prepare(&mut self, spec: ProcessSpec) {
        self.spec = spec;
        let r = 1.0 - (2.0 * PI * DC_CORNER_HZ / f64::from(spec.sample_rate));
        // A first-order recursive filter with a0 = 1 is always accepted.
        self.dc_filter = Lfilter::new(&[1.0, -1.0], &[1.0, -r], 2).ok();
        self.ramp = vec![0.0; spec.block_size];
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
        reason = "the prototype skips work exactly when a steady value equals 1.0"
    )]
    fn process(&mut self, buffer: &mut [[f64; 2]], _ctx: &BlockContext<'_>) {
        let frames = buffer.len().min(self.ramp.len());
        let (Some(buffer), Some(ramp)) = (buffer.get_mut(..frames), self.ramp.get_mut(..frames))
        else {
            return;
        };
        if self.dc_block
            && let Some(dc) = &mut self.dc_filter
        {
            dc.process_in_place(buffer.as_flattened_mut());
        }
        let [invert_left, invert_right] = self.invert;
        for frame in buffer.iter_mut() {
            let [first, second] = *frame;
            let [left, right] = if self.swap {
                [second, first]
            } else {
                [first, second]
            };
            *frame = [left * invert_left, right * invert_right];
        }
        match self.width.next_block(ramp) {
            Smoothed::Steady(1.0) => {}
            Smoothed::Steady(w) => {
                for frame in buffer.iter_mut() {
                    *frame = mid_side(*frame, w);
                }
            }
            Smoothed::Ramp => {
                for (frame, &w) in buffer.iter_mut().zip(ramp.iter()) {
                    *frame = mid_side(*frame, w);
                }
            }
        }
        match self.gain.next_block(ramp) {
            Smoothed::Ramp => {
                for (frame, &g) in buffer.iter_mut().zip(ramp.iter()) {
                    *frame = scale(*frame, g);
                }
            }
            Smoothed::Steady(g) if g != 1.0 => {
                for frame in buffer.iter_mut() {
                    *frame = scale(*frame, g);
                }
            }
            Smoothed::Steady(_) => {}
        }
    }

    fn reset(&mut self) {
        if let Some(dc) = &mut self.dc_filter {
            dc.reset();
        }
    }
}

/// One frame through the width stage: `mid = (l + r)·0.5`, `side = (l − r)·0.5·w`, out
/// `[mid + side, mid − side]`.
#[expect(
    clippy::manual_midpoint,
    reason = "RT-11: the prototype's `(l + r) * 0.5`, written as it computes it"
)]
fn mid_side([l, r]: [f64; 2], w: f64) -> [f64; 2] {
    let mid = (l + r) * 0.5;
    let side = (l - r) * 0.5 * w;
    [mid + side, mid - side]
}

#[cfg(test)]
mod tests {
    use sonara_core::model::device::{ParamValue, Params};
    use sonara_core::plugins::builtin_registry;

    use super::{
        DC_BLOCK, GAIN_DB, INVERT_LEFT, INVERT_RIGHT, MONO, PLUGIN_ID, SWAP, Utility, WIDTH,
    };
    use crate::filters::db_to_amp;
    use crate::processor::{BlockContext, ProcessSpec, Processor};
    use crate::processors::param_slots;

    const SPEC: ProcessSpec = ProcessSpec {
        sample_rate: 48_000,
        block_size: 4,
        channels: 2,
    };

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

    fn prepared(values: &[(&str, ParamValue)]) -> Utility {
        let registry = builtin_registry();
        let descriptor = registry.get(PLUGIN_ID).unwrap();
        let values: Params = values
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        let mut utility = Utility::new();
        utility.prepare(SPEC);
        utility.set_params(&param_slots(descriptor, &values).unwrap());
        utility
    }

    #[test]
    fn slots_follow_the_descriptor_order() {
        let registry = builtin_registry();
        let ids: Vec<&str> = registry
            .get(PLUGIN_ID)
            .unwrap()
            .params
            .iter()
            .map(|p| p.id.as_str())
            .collect();
        let slots = [
            (GAIN_DB, "gain_db"),
            (WIDTH, "width"),
            (MONO, "mono"),
            (SWAP, "swap"),
            (INVERT_LEFT, "invert_left"),
            (INVERT_RIGHT, "invert_right"),
            (DC_BLOCK, "dc_block"),
        ];
        assert_eq!(ids.len(), slots.len());
        for (index, id) in slots {
            assert_eq!(ids.get(index), Some(&id));
        }
    }

    #[test]
    fn default_settings_pass_the_signal_unchanged() {
        let mut utility = prepared(&[]);
        let input = [[0.5, -0.25], [0.1, 0.2], [-1.0, 1.0], [0.0, 0.3]];
        let mut block = input;
        utility.process(&mut block, &ctx());
        assert_eq!(block, input);
    }

    #[test]
    fn swap_then_invert_then_gain() {
        let mut utility = prepared(&[
            ("swap", ParamValue::Bool(true)),
            ("invert_left", ParamValue::Bool(true)),
            ("gain_db", ParamValue::Float(-6.0)),
        ]);
        let g = db_to_amp(-6.0);
        let mut block = [[0.5, -0.25]; 4];
        utility.process(&mut block, &ctx());
        // The swapped left (the old right) is inverted; the first target applies immediately.
        assert_eq!(block, [[0.25 * g, 0.5 * g]; 4]);
    }

    #[test]
    fn mono_sums_to_the_mid_signal_and_ramps_on_a_change() {
        let mut utility = prepared(&[("mono", ParamValue::Bool(true))]);
        let mut block = [[0.5, -0.25]; 4];
        utility.process(&mut block, &ctx());
        assert_eq!(block, [[0.125, 0.125]; 4]);
        // Back to width 1: a ramp over max(64, block) samples, so the block is not yet stereo.
        let registry = builtin_registry();
        let descriptor = registry.get(PLUGIN_ID).unwrap();
        let slots = param_slots(descriptor, &Params::new()).unwrap();
        utility.set_params(&slots);
        let mut block = [[0.5, -0.25]; 4];
        utility.process(&mut block, &ctx());
        let first = block.first().copied().unwrap();
        assert!(first[0] > 0.125 && first[0] < 0.5, "{first:?}");
        // An equal set is a no-op and does not restart the ramp.
        utility.set_params(&slots);
        assert!(utility.width.is_smoothing());
    }

    #[test]
    fn unprepared_or_oversized_blocks_never_panic() {
        let mut utility = Utility::new();
        let mut block = [[1.0, 2.0]; 8];
        utility.process(&mut block, &ctx());
        assert_eq!(
            block,
            [[1.0, 2.0]; 8],
            "nothing is processed before prepare"
        );
        let mut utility = prepared(&[("gain_db", ParamValue::Float(-6.0))]);
        utility.process(&mut block, &ctx());
        assert_eq!(
            block.get(4..),
            Some(&[[1.0, 2.0]; 4][..]),
            "frames beyond the block size are left alone"
        );
    }
}
