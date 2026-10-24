//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! `sonara.dynamics.compressor`: a feed-forward soft-knee compressor with stereo-linked
//! detection, a filtered key, parallel mix and an external sidechain, as the prototype's
//! `dsp/dynamics/compressor.py` computes it (after Giannoulis, Massberg and Reiss, 2012).

use super::params::{choice, flag, float};
use super::pyops::{max_initial, maximum};
use super::{DEFAULT_SPEC, ramp_length, scale, store_params};
use crate::filters::{
    BiquadType, ButterworthSlope, Lfilter, Section, Smoothed, SmoothedValue, SosFilter,
    butterworth, db_to_amp, time_coefficient,
};
use crate::kernels::ar_smooth;
use crate::processor::{BlockContext, ParamSlots, ProcessSpec, Processor, Report};

/// The plugin this processor implements.
pub(crate) const PLUGIN_ID: &str = "sonara.dynamics.compressor";

// Slot indices, in the descriptor's parameter order.
const THRESHOLD_DB: usize = 0;
const RATIO: usize = 1;
const ATTACK_MS: usize = 2;
const RELEASE_MS: usize = 3;
const KNEE_DB: usize = 4;
const MAKEUP_DB: usize = 5;
const AUTO_MAKEUP: usize = 6;
const MIX: usize = 7;
const DETECTOR: usize = 8;
const SIDECHAIN: usize = 9;
const SIDECHAIN_HPF_HZ: usize = 10;
const SIDECHAIN_LPF_HZ: usize = 11;

/// The RMS detector's time constant, in milliseconds (`RMS_WINDOW_MS`).
const RMS_WINDOW_MS: f64 = 10.0;
/// The detector's floor before the logarithm, as an amplitude (−200 dBFS).
const LEVEL_FLOOR: f64 = 1e-10;
/// The key high-pass is on above this frequency, in Hz.
const KEY_HPF_ON_ABOVE: f64 = 21.0;
/// The key low-pass is on below this frequency, in Hz.
const KEY_LPF_ON_BELOW: f64 = 19_999.0;
/// The key filter's largest design: a 12 dB high-pass (one section) and a 24 dB low-pass (two).
const KEY_SECTIONS: usize = 3;

/// The static curve (`gain_computer`): the output level in dB for an input level `x_db`, in
/// the prototype's order of operations.
fn gain_computer(x_db: f64, threshold: f64, ratio: f64, knee: f64) -> f64 {
    let over = x_db - threshold;
    if knee > 0.0 {
        if (2.0 * over).abs() <= knee {
            let s = over + knee / 2.0;
            // `(1/r − 1) · (over + knee/2)**2 / (2·knee)`; `**2` is `s * s`.
            x_db + (1.0 / ratio - 1.0) * (s * s) / (2.0 * knee)
        } else if 2.0 * over > knee {
            threshold + over / ratio
        } else {
            x_db
        }
    } else if over > 0.0 {
        threshold + over / ratio
    } else {
        x_db
    }
}

/// The decoded parameter set.
struct Settings {
    threshold: f64,
    ratio: f64,
    attack_ms: f64,
    release_ms: f64,
    knee: f64,
    makeup_db: f64,
    auto_makeup: bool,
    mix: f64,
    rms: bool,
    use_sidechain: bool,
    hpf: f64,
    lpf: f64,
}

impl Settings {
    fn from_slots(slots: &ParamSlots) -> Option<Self> {
        Some(Self {
            threshold: float(slots, THRESHOLD_DB)?,
            ratio: float(slots, RATIO)?,
            attack_ms: float(slots, ATTACK_MS)?,
            release_ms: float(slots, RELEASE_MS)?,
            knee: float(slots, KNEE_DB)?,
            makeup_db: float(slots, MAKEUP_DB)?,
            auto_makeup: flag(slots, AUTO_MAKEUP)?,
            mix: float(slots, MIX)?,
            // The `detector` choices are `peak` and `rms`.
            rms: match choice(slots, DETECTOR)? {
                0 => false,
                1 => true,
                _ => return None,
            },
            use_sidechain: flag(slots, SIDECHAIN)?,
            hpf: float(slots, SIDECHAIN_HPF_HZ)?,
            lpf: float(slots, SIDECHAIN_LPF_HZ)?,
        })
    }
}

/// The compressor (the prototype's `Compressor`).
pub(crate) struct Compressor {
    spec: ProcessSpec,
    params: ParamSlots,
    attack: f64,
    release: f64,
    threshold: f64,
    ratio: f64,
    knee: f64,
    rms: bool,
    use_sidechain: bool,
    makeup: SmoothedValue,
    mix: SmoothedValue,
    /// The gain-reduction smoother's state (`_env`).
    env: f64,
    /// The largest gain reduction of the last block, in dB (`_max_gr`).
    max_gr: f64,
    /// The mono key filter (`_sc_filter`), an identity while both filters are off.
    key_filter: SosFilter,
    /// `lfilter([1 − c], [1, −c], level², zi)` with `c = time_coefficient(10 ms)`. Its
    /// coefficients depend on the sample rate only, so `prepare` builds it.
    rms_filter: Option<Lfilter>,
    /// The detector chain, in place: key, level, power, level, dB, then the reduction target.
    detect: Vec<f64>,
    /// The smoothed reduction in dB.
    gr: Vec<f64>,
    makeup_ramp: Vec<f64>,
    mix_ramp: Vec<f64>,
}

impl Compressor {
    /// Setup: an unprepared processor.
    pub(crate) fn new() -> Self {
        Self {
            spec: DEFAULT_SPEC,
            params: ParamSlots::EMPTY,
            attack: 0.0,
            release: 0.0,
            threshold: 0.0,
            // A ratio of 1 reduces nothing until parameters arrive.
            ratio: 1.0,
            knee: 0.0,
            rms: false,
            use_sidechain: false,
            makeup: SmoothedValue::new(),
            mix: SmoothedValue::new(),
            env: 0.0,
            max_gr: 0.0,
            key_filter: SosFilter::new(1, KEY_SECTIONS),
            rms_filter: None,
            detect: Vec::new(),
            gr: Vec::new(),
            makeup_ramp: Vec::new(),
            mix_ramp: Vec::new(),
        }
    }

    /// The prototype's `on_params_changed`.
    fn apply_params(&mut self) {
        let Some(p) = Settings::from_slots(&self.params) else {
            return;
        };
        let sr = f64::from(self.spec.sample_rate);
        self.attack = time_coefficient(p.attack_ms, sr);
        self.release = time_coefficient(p.release_ms, sr);
        self.threshold = p.threshold;
        self.ratio = p.ratio;
        self.knee = p.knee;
        let mut makeup = p.makeup_db;
        if p.auto_makeup {
            // Compensate half the reduction a 0 dBFS signal would receive.
            makeup += -0.5 * gain_computer(0.0, self.threshold, self.ratio, self.knee);
        }
        let ramp = ramp_length(self.spec.block_size);
        self.makeup.set_ramp(ramp);
        self.makeup.set_target(db_to_amp(makeup));
        self.mix.set_ramp(ramp);
        self.mix.set_target(p.mix);
        self.rms = p.rms;
        // The key filter: a high-pass keeps bass from over-triggering, a low-pass keys on the
        // kick of a full drum bus.
        let mut design = [[0.0; 6]; KEY_SECTIONS];
        let mut count = 0;
        let mut add = |sections: &[Section]| {
            for section in sections {
                if let Some(slot) = design.get_mut(count) {
                    *slot = *section;
                    count += 1;
                }
            }
        };
        if p.hpf > KEY_HPF_ON_ABOVE {
            add(butterworth(BiquadType::Highpass, p.hpf, sr, ButterworthSlope::Db12).sections());
        }
        if p.lpf < KEY_LPF_ON_BELOW {
            add(butterworth(BiquadType::Lowpass, p.lpf, sr, ButterworthSlope::Db24).sections());
        }
        // ERR-06: cannot fail, the filter holds KEY_SECTIONS, the largest design; an empty
        // design switches to identity (`set_sos(None)`).
        let _ = self
            .key_filter
            .set_sections(design.get(..count).unwrap_or_default());
        self.use_sidechain = p.use_sidechain;
    }

    /// `detector_db` followed by `x_db − gain_computer(x_db)`: the reduction target per sample
    /// into `detect`.
    #[expect(
        clippy::manual_midpoint,
        reason = "RT-11: NumPy's two-channel mean is `(l + r) / 2`, written as it computes it"
    )]
    fn detect(&mut self, buffer: &[[f64; 2]], sidechain: Option<&[[f64; 2]]>, n: usize) {
        let Some(detect) = self.detect.get_mut(..n) else {
            return;
        };
        let key = match sidechain {
            Some(sidechain) if self.use_sidechain => sidechain,
            _ => buffer,
        };
        // Frames the key does not have count as silence.
        let mut frames = key.iter();
        if self.key_filter.is_identity() {
            // `np.max(np.abs(key), axis=1)`.
            for level in detect.iter_mut() {
                *level = frames
                    .next()
                    .map_or(0.0, |&[l, r]| maximum(l.abs(), r.abs()));
            }
        } else {
            // `np.mean(key, axis=1, keepdims=True)` through the key filter, then `abs`.
            for k in detect.iter_mut() {
                *k = frames.next().map_or(0.0, |&[l, r]| (l + r) / 2.0);
            }
            self.key_filter.process_in_place(detect);
            for level in detect.iter_mut() {
                *level = level.abs();
            }
        }
        if self.rms
            && let Some(rms) = &mut self.rms_filter
        {
            for level in detect.iter_mut() {
                *level *= *level;
            }
            rms.process_in_place(detect);
            for level in detect.iter_mut() {
                *level = maximum(*level, 0.0).sqrt();
            }
        }
        for v in detect.iter_mut() {
            let x_db = 20.0 * maximum(*v, LEVEL_FLOOR).log10();
            *v = x_db - gain_computer(x_db, self.threshold, self.ratio, self.knee);
        }
    }
}

impl Processor for Compressor {
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    fn prepare(&mut self, spec: ProcessSpec) {
        self.spec = spec;
        let c = time_coefficient(RMS_WINDOW_MS, f64::from(spec.sample_rate));
        // A first-order recursive filter with a0 = 1 is always accepted.
        self.rms_filter = Lfilter::new(&[1.0 - c], &[1.0, -c], 1).ok();
        self.detect = vec![0.0; spec.block_size];
        self.gr = vec![0.0; spec.block_size];
        self.makeup_ramp = vec![0.0; spec.block_size];
        self.mix_ramp = vec![0.0; spec.block_size];
        self.reset();
        self.apply_params();
    }

    fn set_params(&mut self, params: &ParamSlots) {
        if store_params(&mut self.params, params) {
            self.apply_params();
        }
    }

    fn process(&mut self, buffer: &mut [[f64; 2]], ctx: &BlockContext<'_>) {
        let n = buffer.len().min(self.detect.len());
        let Some(buffer) = buffer.get_mut(..n) else {
            return;
        };
        self.detect(buffer, ctx.sidechain, n);
        let (Some(detect), Some(gr), Some(makeup), Some(mix)) = (
            self.detect.get(..n),
            self.gr.get_mut(..n),
            self.makeup_ramp.get_mut(..n),
            self.mix_ramp.get_mut(..n),
        ) else {
            return;
        };
        ar_smooth(detect, gr, self.attack, self.release, &mut self.env);
        self.max_gr = max_initial(gr, 0.0);
        // `gain = 10**(−gr/20) * makeup`, then `wet = buffer * gain`.
        match self.makeup.next_block(makeup) {
            Smoothed::Steady(m) => {
                for g in gr.iter_mut() {
                    *g = 10.0_f64.powf(-*g / 20.0) * m;
                }
            }
            Smoothed::Ramp => {
                for (g, &m) in gr.iter_mut().zip(makeup.iter()) {
                    *g = 10.0_f64.powf(-*g / 20.0) * m;
                }
            }
        }
        match self.mix.next_block(mix) {
            Smoothed::Steady(m) if m >= 1.0 => {
                for (frame, &g) in buffer.iter_mut().zip(gr.iter()) {
                    *frame = scale(*frame, g);
                }
            }
            Smoothed::Steady(m) => {
                for (frame, &g) in buffer.iter_mut().zip(gr.iter()) {
                    *frame = blend(*frame, g, m);
                }
            }
            Smoothed::Ramp => {
                for ((frame, &g), &m) in buffer.iter_mut().zip(gr.iter()).zip(mix.iter()) {
                    *frame = blend(*frame, g, m);
                }
            }
        }
    }

    fn reset(&mut self) {
        self.env = 0.0;
        if let Some(rms) = &mut self.rms_filter {
            rms.reset();
        }
        self.key_filter.reset();
    }

    fn report(&self) -> Report {
        Report::new(&[("gain_reduction_db", self.max_gr)])
    }
}

/// `wet * m + buffer * (1 − m)` for one frame, with `wet = buffer * gain`.
fn blend([l, r]: [f64; 2], gain: f64, m: f64) -> [f64; 2] {
    let dry = 1.0 - m;
    [(l * gain) * m + l * dry, (r * gain) * m + r * dry]
}

#[cfg(test)]
mod tests {
    use sonara_core::model::device::{ParamValue, Params};
    use sonara_core::plugins::builtin_registry;

    use super::{
        ATTACK_MS, AUTO_MAKEUP, Compressor, DETECTOR, KNEE_DB, MAKEUP_DB, MIX, PLUGIN_ID, RATIO,
        RELEASE_MS, SIDECHAIN, SIDECHAIN_HPF_HZ, SIDECHAIN_LPF_HZ, THRESHOLD_DB, gain_computer,
    };
    use crate::processor::{BlockContext, ProcessSpec, Processor};
    use crate::processors::param_slots;

    fn prepared(values: &[(&str, ParamValue)]) -> Compressor {
        let registry = builtin_registry();
        let values: Params = values
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        let mut compressor = Compressor::new();
        compressor.prepare(ProcessSpec {
            sample_rate: 48_000,
            block_size: 256,
            channels: 2,
        });
        compressor.set_params(&param_slots(registry.get(PLUGIN_ID).unwrap(), &values).unwrap());
        compressor
    }

    fn ctx(sidechain: Option<&[[f64; 2]]>) -> BlockContext<'_> {
        BlockContext {
            frame: 0,
            sample_rate: 48_000,
            beat: 0.0,
            bpm: 120.0,
            playing: true,
            sidechain,
            events: &[],
        }
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
            (THRESHOLD_DB, "threshold_db"),
            (RATIO, "ratio"),
            (ATTACK_MS, "attack_ms"),
            (RELEASE_MS, "release_ms"),
            (KNEE_DB, "knee_db"),
            (MAKEUP_DB, "makeup_db"),
            (AUTO_MAKEUP, "auto_makeup"),
            (MIX, "mix"),
            (DETECTOR, "detector"),
            (SIDECHAIN, "sidechain"),
            (SIDECHAIN_HPF_HZ, "sidechain_hpf_hz"),
            (SIDECHAIN_LPF_HZ, "sidechain_lpf_hz"),
        ];
        assert_eq!(ids.len(), slots.len());
        for (index, id) in slots {
            assert_eq!(ids.get(index), Some(&id));
        }
    }

    #[test]
    fn the_static_curve() {
        // Hard knee: unity below the threshold, `thr + over/ratio` above it.
        assert_eq!(gain_computer(-30.0, -20.0, 4.0, 0.0), -30.0);
        assert_eq!(gain_computer(-12.0, -20.0, 4.0, 0.0), -18.0);
        // Soft knee of 10 dB: the curve is continuous at both knee edges.
        let edge = |x: f64| gain_computer(x, -20.0, 4.0, 10.0);
        assert_eq!(edge(-25.0), -25.0);
        assert!((edge(-15.0) - (-20.0 + 5.0 / 4.0)).abs() < 1e-12);
        assert_eq!(edge(-10.0), -20.0 + 10.0 / 4.0);
        assert_eq!(edge(-40.0), -40.0);
    }

    #[test]
    fn quiet_signals_pass_and_loud_ones_are_reduced() {
        let mut compressor = prepared(&[]);
        let mut quiet = [[0.01, -0.01]; 256];
        compressor.process(&mut quiet, &ctx(None));
        assert_eq!(quiet, [[0.01, -0.01]; 256], "−40 dBFS is below the knee");
        let mut loud = [[0.9, -0.9]; 256];
        compressor.process(&mut loud, &ctx(None));
        let last = loud.last().copied().unwrap();
        assert!(last[0] < 0.9 && last[0] > 0.0, "{last:?}");
        assert!(compressor.max_gr > 0.0);
    }

    #[test]
    fn a_sidechain_ducks_only_when_enabled() {
        let key = [[0.9, 0.9]; 256];
        for (enabled, ducked) in [(true, true), (false, false)] {
            let mut compressor = prepared(&[("sidechain", ParamValue::Bool(enabled))]);
            let mut quiet = [[0.01, 0.01]; 256];
            compressor.process(&mut quiet, &ctx(Some(&key)));
            assert_eq!(quiet != [[0.01, 0.01]; 256], ducked, "sidechain {enabled}");
        }
    }
}
