//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! `sonara.saturation`: waveshaping saturation and distortion with optional 2× or 4×
//! oversampling, as the prototype's `dsp/saturation.py` computes it. Oversampling zero-stuffs,
//! filters with the elliptic low-pass of [`crate::tables::ellip_oversampling`] (its own state
//! for the up- and the down-sampling), shapes at the high rate, filters again and keeps every
//! `factor`-th sample: no added latency, a slight phase shift.

use std::f64::consts::PI;

use super::params::{choice, float};
use super::pyops::clip;
use super::{DEFAULT_SPEC, ramp_length, store_params};
use crate::filters::{BiquadType, Smoothed, SmoothedValue, SosFilter, biquad, db_to_amp};
use crate::processor::{BlockContext, ParamSlots, ProcessSpec, Processor};
use crate::tables::{Oversampling, SupportedRate, ellip_oversampling};

/// The plugin this processor implements.
pub(crate) const PLUGIN_ID: &str = "sonara.saturation";

// Slot indices, in the descriptor's parameter order.
const MODE: usize = 0;
const DRIVE_DB: usize = 1;
const BIAS: usize = 2;
const TONE_HZ: usize = 3;
const MIX: usize = 4;
const OUTPUT_DB: usize = 5;
const OVERSAMPLING: usize = 6;

/// The tone low-pass is on below this frequency, in Hz.
const TONE_ON_BELOW: f64 = 19_999.0;
/// The tone low-pass's Q, the prototype's literal.
#[expect(
    clippy::approx_constant,
    reason = "RT-11: the prototype's 0.7071, not 1/√2 (0.70710678…)"
)]
const TONE_Q: f64 = 0.7071;
/// The tube curve's built-in bias, added to the `bias` parameter.
const TUBE_BIAS: f64 = 0.2;
/// The foldback's input clamp.
const FOLDBACK_LIMIT: f64 = 50.0;
/// Sections of the oversampling filters (`ellip(10, …)`).
const ELLIP_SECTIONS: usize = 5;
/// The highest oversampling factor.
const MAX_FACTOR: usize = 4;

/// The transfer curve (`shape`), the `mode` choices in descriptor order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    /// `tanh(x)`.
    Tanh,
    /// The cubic `(c − c³/3) · 1.5` of `c = clip(x, ±1)`.
    SoftClip,
    /// `clip(x, ±1)`.
    HardClip,
    /// `tanh(x + b) − tanh(b)` with `b = 0.2 + bias`.
    Tube,
    /// `sin(clip(x, ±50) · π/2)`.
    Foldback,
}

impl Shape {
    fn from_choice(index: u16) -> Option<Self> {
        match index {
            0 => Some(Self::Tanh),
            1 => Some(Self::SoftClip),
            2 => Some(Self::HardClip),
            3 => Some(Self::Tube),
            4 => Some(Self::Foldback),
            _ => None,
        }
    }

    /// One sample through the curve; `b` and `tanh_b` are the tube's bias and its `tanh`.
    fn apply(self, x: f64, b: f64, tanh_b: f64) -> f64 {
        match self {
            Self::Tanh => x.tanh(),
            Self::SoftClip => {
                let c = clip(x, -1.0, 1.0);
                // `c**3` is `pow(c, 3.0)`, not `c*c*c`.
                (c - c.powf(3.0) / 3.0) * 1.5
            }
            Self::HardClip => clip(x, -1.0, 1.0),
            Self::Tube => (x + b).tanh() - tanh_b,
            Self::Foldback => (clip(x, -FOLDBACK_LIMIT, FOLDBACK_LIMIT) * (PI / 2.0)).sin(),
        }
    }
}

/// The `oversampling` choices, in descriptor order: `1x`, `2x`, `4x`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Factor {
    /// No oversampling.
    One,
    /// Oversampling by 2 or 4.
    Over(Oversampling),
}

impl Factor {
    fn from_choice(index: u16) -> Option<Self> {
        match index {
            0 => Some(Self::One),
            1 => Some(Self::Over(Oversampling::X2)),
            2 => Some(Self::Over(Oversampling::X4)),
            _ => None,
        }
    }

    /// The oversampling, `None` at 1×.
    fn oversampling(self) -> Option<Oversampling> {
        match self {
            Self::One => None,
            Self::Over(factor) => Some(factor),
        }
    }
}

/// The decoded parameter set.
struct Settings {
    shape: Shape,
    drive_db: f64,
    bias: f64,
    tone_hz: f64,
    mix: f64,
    output_db: f64,
    oversampling: Option<Oversampling>,
}

impl Settings {
    fn from_slots(slots: &ParamSlots) -> Option<Self> {
        Some(Self {
            shape: Shape::from_choice(choice(slots, MODE)?)?,
            drive_db: float(slots, DRIVE_DB)?,
            bias: float(slots, BIAS)?,
            tone_hz: float(slots, TONE_HZ)?,
            mix: float(slots, MIX)?,
            output_db: float(slots, OUTPUT_DB)?,
            oversampling: Factor::from_choice(choice(slots, OVERSAMPLING)?)?.oversampling(),
        })
    }
}

/// The saturation (the prototype's `Saturation`).
pub(crate) struct Saturation {
    spec: ProcessSpec,
    params: ParamSlots,
    /// The project rate's oversampling tables; `None` for a rate the prototype does not
    /// support, which then runs without oversampling.
    rate: Option<SupportedRate>,
    drive: SmoothedValue,
    out: SmoothedValue,
    mix: SmoothedValue,
    shape: Shape,
    bias: f64,
    /// The running oversampling (`_resampler`): `None` at 1×.
    resampler: Option<Oversampling>,
    /// The anti-imaging filter after zero-stuffing (`zi_up`).
    up: SosFilter,
    /// The anti-aliasing filter before decimation (`zi_down`).
    down: SosFilter,
    tone: SosFilter,
    /// The block at the high rate, up to four blocks long.
    high: Vec<[f64; 2]>,
    /// The shaped block.
    wet: Vec<[f64; 2]>,
    drive_ramp: Vec<f64>,
    out_ramp: Vec<f64>,
    mix_ramp: Vec<f64>,
}

impl Saturation {
    /// Setup: an unprepared processor.
    pub(crate) fn new() -> Self {
        Self {
            spec: DEFAULT_SPEC,
            params: ParamSlots::EMPTY,
            rate: SupportedRate::from_hz(DEFAULT_SPEC.sample_rate),
            drive: SmoothedValue::new(),
            out: SmoothedValue::new(),
            mix: SmoothedValue::new(),
            shape: Shape::Tanh,
            bias: 0.0,
            resampler: None,
            up: SosFilter::new(2, ELLIP_SECTIONS),
            down: SosFilter::new(2, ELLIP_SECTIONS),
            tone: SosFilter::new(2, 1),
            high: Vec::new(),
            wet: Vec::new(),
            drive_ramp: Vec::new(),
            out_ramp: Vec::new(),
            mix_ramp: Vec::new(),
        }
    }

    /// The prototype's `on_params_changed`.
    fn apply_params(&mut self) {
        let Some(p) = Settings::from_slots(&self.params) else {
            return;
        };
        let ramp = ramp_length(self.spec.block_size);
        for s in [&mut self.drive, &mut self.out, &mut self.mix] {
            s.set_ramp(ramp);
        }
        self.drive.set_target(db_to_amp(p.drive_db));
        self.out.set_target(db_to_amp(p.output_db));
        self.mix.set_target(p.mix);
        self.shape = p.shape;
        self.bias = p.bias;
        if self.resampler.is_none() || self.resampler != p.oversampling {
            // A new resampler starts from zero state; at 1× there is none.
            self.resampler = None;
            if let (Some(factor), Some(rate)) = (p.oversampling, self.rate) {
                let sections = ellip_oversampling(rate, factor);
                // ERR-06: cannot fail, both filters hold the five sections of the design.
                let _ = self.up.set_sections(sections);
                let _ = self.down.set_sections(sections);
                self.up.reset();
                self.down.reset();
                self.resampler = Some(factor);
            }
        }
        if p.tone_hz < TONE_ON_BELOW {
            let sr = f64::from(self.spec.sample_rate);
            let section = biquad(BiquadType::Lowpass, p.tone_hz, sr, TONE_Q, 0.0);
            // ERR-06: cannot fail, the filter holds the one section.
            let _ = self.tone.set_sections(&[section]);
        } else {
            self.tone.set_identity();
        }
    }
}

impl Processor for Saturation {
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    fn prepare(&mut self, spec: ProcessSpec) {
        self.spec = spec;
        self.rate = SupportedRate::from_hz(spec.sample_rate);
        self.high = vec![[0.0; 2]; spec.block_size.saturating_mul(MAX_FACTOR)];
        self.wet = vec![[0.0; 2]; spec.block_size];
        self.drive_ramp = vec![0.0; spec.block_size];
        self.out_ramp = vec![0.0; spec.block_size];
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
        let n = buffer.len().min(self.wet.len());
        let (Some(buffer), Some(wet), Some(drive), Some(out), Some(mix)) = (
            buffer.get_mut(..n),
            self.wet.get_mut(..n),
            self.drive_ramp.get_mut(..n),
            self.out_ramp.get_mut(..n),
            self.mix_ramp.get_mut(..n),
        ) else {
            return;
        };
        // The smoothers' scalar and array paths compute the same products here, so a steady
        // value is spread over its block.
        for (smoother, values) in [
            (&mut self.drive, &mut *drive),
            (&mut self.out, &mut *out),
            (&mut self.mix, &mut *mix),
        ] {
            if let Smoothed::Steady(v) = smoother.next_block(values) {
                values.fill(v);
            }
        }
        // `x = buffer * drive`.
        for ((x, &[l, r]), &d) in wet.iter_mut().zip(buffer.iter()).zip(drive.iter()) {
            *x = [l * d, r * d];
        }
        let b = TUBE_BIAS + self.bias;
        let tanh_b = b.tanh();
        let shape = self.shape;
        let shape_frame = |frame: &mut [f64; 2]| {
            for v in frame.iter_mut() {
                *v = shape.apply(*v, b, tanh_b);
            }
        };
        let high = self
            .resampler
            .map(Oversampling::factor)
            .and_then(|f| Some((f, self.high.get_mut(..n.checked_mul(f)?)?)));
        if let Some((factor, high)) = high {
            // Up: `z[::f] = x * f`, then the anti-imaging filter.
            #[expect(
                clippy::cast_precision_loss,
                reason = "the factor is 2 or 4, exact in f64"
            )]
            let gain = factor as f64;
            // The factor is 2 or 4, so the chunks are never empty.
            for (chunk, &[l, r]) in high.chunks_exact_mut(factor.max(1)).zip(wet.iter()) {
                if let Some((first, zeros)) = chunk.split_first_mut() {
                    *first = [l * gain, r * gain];
                    zeros.fill([0.0; 2]);
                }
            }
            self.up.process_in_place(high.as_flattened_mut());
            high.iter_mut().for_each(shape_frame);
            // Down: the anti-aliasing filter, then every `factor`-th sample.
            self.down.process_in_place(high.as_flattened_mut());
            for (w, h) in wet.iter_mut().zip(high.iter().step_by(factor.max(1))) {
                *w = *h;
            }
        } else {
            wet.iter_mut().for_each(shape_frame);
        }
        self.tone.process_in_place(wet.as_flattened_mut());
        // `(wet * m + buffer * (1 − m)) * o`.
        let gains = out.iter().zip(mix.iter());
        for ((frame, w), (&o, &m)) in buffer.iter_mut().zip(wet.iter()).zip(gains) {
            let dry = 1.0 - m;
            let [wl, wr] = *w;
            let [dl, dr] = *frame;
            *frame = [(wl * m + dl * dry) * o, (wr * m + dr * dry) * o];
        }
    }

    fn reset(&mut self) {
        // The prototype drops the resampler and makes a new identity tone filter, then applies
        // the parameters again, which brings the resampler back with zero state.
        self.resampler = None;
        self.tone.reset();
        self.tone.set_identity();
        self.apply_params();
    }
}

#[cfg(test)]
mod tests {
    use sonara_core::model::device::{ParamValue, Params};
    use sonara_core::plugins::builtin_registry;

    use super::{
        BIAS, DRIVE_DB, MIX, MODE, OUTPUT_DB, OVERSAMPLING, PLUGIN_ID, Saturation, Shape, TONE_HZ,
    };
    use crate::processor::{BlockContext, ParamSlots, ProcessSpec, Processor};
    use crate::processors::param_slots;
    use crate::tables::Oversampling;

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

    fn prepared(sample_rate: u32, values: &[(&str, ParamValue)]) -> Saturation {
        let mut saturation = Saturation::new();
        saturation.prepare(ProcessSpec {
            sample_rate,
            block_size: 64,
            channels: 2,
        });
        saturation.set_params(&slots(values));
        saturation
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
            (DRIVE_DB, "drive_db"),
            (BIAS, "bias"),
            (TONE_HZ, "tone_hz"),
            (MIX, "mix"),
            (OUTPUT_DB, "output_db"),
            (OVERSAMPLING, "oversampling"),
        ];
        assert_eq!(ids.len(), slots.len());
        for (index, id) in slots {
            assert_eq!(ids.get(index), Some(&id));
        }
    }

    #[test]
    fn curves() {
        let tube_b = 0.2;
        for shape in [
            Shape::Tanh,
            Shape::SoftClip,
            Shape::HardClip,
            Shape::Tube,
            Shape::Foldback,
        ] {
            assert_eq!(shape.apply(0.0, tube_b, tube_b.tanh()), 0.0, "{shape:?}");
        }
        assert_eq!(
            Shape::SoftClip.apply(5.0, 0.0, 0.0),
            1.0,
            "saturates at 2/3 · 1.5"
        );
        assert_eq!(Shape::HardClip.apply(-5.0, 0.0, 0.0), -1.0);
        assert_eq!(Shape::Foldback.apply(1.0, 0.0, 0.0), 1.0);
        assert!(Shape::Foldback.apply(3.0, 0.0, 0.0) < -0.99, "folds back");
    }

    #[test]
    fn oversampling_follows_the_factor_and_keeps_state_while_unchanged() {
        let mut saturation = prepared(48_000, &[]);
        assert_eq!(
            saturation.resampler,
            Some(Oversampling::X2),
            "2x by default"
        );
        let mut block = [[0.25, -0.25]; 64];
        saturation.process(&mut block, &ctx());
        let state = saturation.up.state(0).map(<[_]>::to_vec);
        // A change of another parameter keeps the resampler and its state.
        saturation.set_params(&slots(&[("drive_db", ParamValue::Float(12.0))]));
        assert_eq!(saturation.up.state(0).map(<[_]>::to_vec), state);
        saturation.set_params(&slots(&[("oversampling", ParamValue::from("1x"))]));
        assert_eq!(saturation.resampler, None);
        saturation.set_params(&slots(&[("oversampling", ParamValue::from("4x"))]));
        assert_eq!(saturation.resampler, Some(Oversampling::X4));
        assert!(
            saturation
                .up
                .state(0)
                .is_some_and(|s| s.iter().flatten().all(|z| *z == 0.0)),
            "a new resampler starts from zero"
        );
        // An unsupported rate has no tables and runs without oversampling.
        let saturation = prepared(8_000, &[]);
        assert_eq!(saturation.resampler, None);
    }

    #[test]
    fn a_dry_mix_returns_the_input_times_the_output_gain() {
        let mut saturation = prepared(
            48_000,
            &[
                ("mix", ParamValue::Float(0.0)),
                ("output_db", ParamValue::Float(0.0)),
            ],
        );
        let input = [[0.5, -0.25]; 64];
        let mut block = input;
        saturation.process(&mut block, &ctx());
        assert_eq!(block, input);
    }
}
