//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! `sonara.filter`: a resonant multimode state-variable filter with a cutoff LFO, as the
//! prototype's `dsp/filters/multimode.py` computes it. The cutoff glides in the log domain over
//! `max(32, block)` samples; the LFO phase advances once per block and freezes while the depth
//! is 0 (a kept quirk, section 5).

use std::f64::consts::{LN_2, PI};

use super::params::{choice, float};
use super::pyops::{py_max, py_min, py_mod};
use super::{DEFAULT_SPEC, ramp_length, store_params};
use crate::filters::{Smoothed, SmoothedValue};
use crate::kernels::{SvfMode, clamp_cutoff, clamp_q, svf};
use crate::processor::{BlockContext, ParamSlots, ProcessSpec, Processor};

/// The plugin this processor implements.
pub(crate) const PLUGIN_ID: &str = "sonara.filter";

// Slot indices, in the descriptor's parameter order.
const MODE: usize = 0;
const CUTOFF_HZ: usize = 1;
const RESONANCE: usize = 2;
const LFO_RATE_HZ: usize = 3;
const LFO_DEPTH_OCT: usize = 4;
const MIX: usize = 5;

/// Shortest ramp of the cutoff and mix smoothers, in samples.
const MIN_RAMP: usize = 32;

/// The `mode` choices, in descriptor order (`_MODES`).
fn mode(index: u16) -> Option<SvfMode> {
    match index {
        0 => Some(SvfMode::Lowpass),
        1 => Some(SvfMode::Highpass),
        2 => Some(SvfMode::Bandpass),
        3 => Some(SvfMode::Notch),
        _ => None,
    }
}

/// `q_from_resonance`: a 0..1 resonance to a Q of 0.5 to 25, exponentially.
fn q_from_resonance(resonance: f64) -> f64 {
    let r = py_min(py_max(resonance, 0.0), 1.0);
    0.5 * (r * 50.0_f64.ln()).exp()
}

/// The decoded parameter set.
struct Settings {
    mode: SvfMode,
    cutoff_hz: f64,
    resonance: f64,
    lfo_rate: f64,
    lfo_depth: f64,
    mix: f64,
}

impl Settings {
    fn from_slots(slots: &ParamSlots) -> Option<Self> {
        Some(Self {
            mode: mode(choice(slots, MODE)?)?,
            cutoff_hz: float(slots, CUTOFF_HZ)?,
            resonance: float(slots, RESONANCE)?,
            lfo_rate: float(slots, LFO_RATE_HZ)?,
            lfo_depth: float(slots, LFO_DEPTH_OCT)?,
            mix: float(slots, MIX)?,
        })
    }
}

/// The multimode filter (the prototype's `MultimodeFilter`).
pub(crate) struct MultimodeFilter {
    spec: ProcessSpec,
    params: ParamSlots,
    /// The cutoff's natural logarithm.
    log_cutoff: SmoothedValue,
    mix: SmoothedValue,
    mode: SvfMode,
    q: f64,
    lfo_rate: f64,
    lfo_depth: f64,
    /// The LFO phase at the block start, in cycles.
    lfo_phase: f64,
    svf_state: [[f64; 2]; 2],
    /// Per-sample log cutoff, then the cutoff in Hz, one block long.
    cutoff: Vec<f64>,
    /// The filtered block.
    wet: Vec<[f64; 2]>,
    /// The values of a running mix ramp, one block long.
    mix_ramp: Vec<f64>,
}

impl MultimodeFilter {
    /// Setup: an unprepared processor.
    pub(crate) fn new() -> Self {
        Self {
            spec: DEFAULT_SPEC,
            params: ParamSlots::EMPTY,
            log_cutoff: SmoothedValue::new(),
            mix: SmoothedValue::new(),
            mode: SvfMode::Lowpass,
            q: 0.5,
            lfo_rate: 0.0,
            lfo_depth: 0.0,
            lfo_phase: 0.0,
            svf_state: [[0.0; 2]; 2],
            cutoff: Vec::new(),
            wet: Vec::new(),
            mix_ramp: Vec::new(),
        }
    }

    /// The prototype's `on_params_changed`.
    fn apply_params(&mut self) {
        let Some(p) = Settings::from_slots(&self.params) else {
            return;
        };
        let ramp = ramp_length(MIN_RAMP.max(self.spec.block_size));
        self.log_cutoff.set_ramp(ramp);
        self.log_cutoff.set_target(p.cutoff_hz.ln());
        self.mix.set_ramp(ramp);
        self.mix.set_target(p.mix);
        self.mode = p.mode;
        self.q = q_from_resonance(p.resonance);
        self.lfo_rate = p.lfo_rate;
        self.lfo_depth = p.lfo_depth;
    }
}

impl Processor for MultimodeFilter {
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    fn prepare(&mut self, spec: ProcessSpec) {
        self.spec = spec;
        self.cutoff = vec![0.0; spec.block_size];
        self.wet = vec![[0.0; 2]; spec.block_size];
        self.mix_ramp = vec![0.0; spec.block_size];
        self.reset();
        self.apply_params();
    }

    fn set_params(&mut self, params: &ParamSlots) {
        if store_params(&mut self.params, params) {
            self.apply_params();
        }
    }

    fn process(&mut self, buffer: &mut [[f64; 2]], _ctx: &BlockContext<'_>) {
        let n = buffer.len().min(self.cutoff.len());
        let (Some(buffer), Some(cutoff), Some(wet), Some(mix)) = (
            buffer.get_mut(..n),
            self.cutoff.get_mut(..n),
            self.wet.get_mut(..n),
            self.mix_ramp.get_mut(..n),
        ) else {
            return;
        };
        let sr = f64::from(self.spec.sample_rate);
        if let Smoothed::Steady(lc) = self.log_cutoff.next_block(cutoff) {
            cutoff.fill(lc);
        }
        if self.lfo_depth > 0.0 {
            let inc = self.lfo_rate / sr;
            // `phase = lfo_phase + inc * arange(n)`; `k` counts exactly up to 2^53.
            let mut k = 0.0;
            for lc in cutoff.iter_mut() {
                let phase = self.lfo_phase + inc * k;
                *lc += (2.0 * PI * phase).sin() * self.lfo_depth * LN_2;
                k += 1.0;
            }
            self.lfo_phase = py_mod(self.lfo_phase + inc * k, 1.0);
        }
        // `np.exp(log_cut)`, then the kernel wrapper's clamps.
        for c in cutoff.iter_mut() {
            *c = clamp_cutoff(c.exp(), sr);
        }
        svf(
            buffer.as_flattened(),
            wet.as_flattened_mut(),
            cutoff,
            clamp_q(self.q),
            self.mode,
            sr,
            &mut self.svf_state,
        );
        match self.mix.next_block(mix) {
            Smoothed::Steady(m) => {
                for (frame, w) in buffer.iter_mut().zip(wet.iter()) {
                    *frame = blend(*w, *frame, m);
                }
            }
            Smoothed::Ramp => {
                for ((frame, w), &m) in buffer.iter_mut().zip(wet.iter()).zip(mix.iter()) {
                    *frame = blend(*w, *frame, m);
                }
            }
        }
    }

    fn reset(&mut self) {
        self.svf_state = [[0.0; 2]; 2];
        self.lfo_phase = 0.0;
    }
}

/// `wet * m + dry * (1 − m)` for one frame.
fn blend([wl, wr]: [f64; 2], [dl, dr]: [f64; 2], m: f64) -> [f64; 2] {
    let d = 1.0 - m;
    [wl * m + dl * d, wr * m + dr * d]
}

#[cfg(test)]
mod tests {
    use sonara_core::model::device::{ParamValue, Params};
    use sonara_core::plugins::builtin_registry;

    use super::{
        CUTOFF_HZ, LFO_DEPTH_OCT, LFO_RATE_HZ, MIX, MODE, MultimodeFilter, PLUGIN_ID, RESONANCE,
        q_from_resonance,
    };
    use crate::processor::{BlockContext, ParamSlots, ProcessSpec, Processor};
    use crate::processors::param_slots;

    fn slots(values: &[(&str, ParamValue)]) -> ParamSlots {
        let registry = builtin_registry();
        let values: Params = values
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        param_slots(registry.get(PLUGIN_ID).unwrap(), &values).unwrap()
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

    fn prepared(values: &[(&str, ParamValue)]) -> MultimodeFilter {
        let mut filter = MultimodeFilter::new();
        filter.prepare(ProcessSpec {
            sample_rate: 48_000,
            block_size: 64,
            channels: 2,
        });
        filter.set_params(&slots(values));
        filter
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
            (MODE, "mode"),
            (CUTOFF_HZ, "cutoff_hz"),
            (RESONANCE, "resonance"),
            (LFO_RATE_HZ, "lfo_rate_hz"),
            (LFO_DEPTH_OCT, "lfo_depth_oct"),
            (MIX, "mix"),
        ];
        assert_eq!(ids.len(), slots.len());
        for (index, id) in slots {
            assert_eq!(ids.get(index), Some(&id));
        }
    }

    #[test]
    fn resonance_maps_exponentially_and_is_clamped() {
        assert_eq!(q_from_resonance(0.0), 0.5);
        assert!((q_from_resonance(1.0) - 25.0).abs() < 1e-12);
        assert_eq!(q_from_resonance(-1.0), 0.5);
        assert_eq!(q_from_resonance(2.0), q_from_resonance(1.0));
    }

    #[test]
    fn the_lfo_advances_per_block_and_freezes_at_depth_zero() {
        let mut filter = prepared(&[]);
        let mut block = [[0.0; 2]; 64];
        filter.process(&mut block, &ctx());
        assert_eq!(filter.lfo_phase, 0.0, "depth 0: the phase stands still");
        let mut filter = prepared(&[
            ("lfo_depth_oct", ParamValue::Float(1.0)),
            ("lfo_rate_hz", ParamValue::Float(15.0)),
        ]);
        filter.process(&mut block, &ctx());
        assert_eq!(filter.lfo_phase, 15.0 / 48_000.0 * 64.0);
        filter.reset();
        assert_eq!(filter.lfo_phase, 0.0);
    }

    #[test]
    fn a_dry_mix_returns_the_input() {
        let mut filter = prepared(&[("mix", ParamValue::Float(0.0))]);
        let input = [[0.5, -0.25]; 64];
        let mut block = input;
        filter.process(&mut block, &ctx());
        assert_eq!(block, input);
    }
}
