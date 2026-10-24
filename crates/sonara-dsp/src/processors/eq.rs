//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! `sonara.eq.parametric`: a Butterworth high-pass, a low band, two mid bands, a high band and a
//! Butterworth low-pass in one cascade, as the prototype's `dsp/filters/eq.py` computes them.
//! Band parameters glide toward their targets once per block (frequency and Q geometrically,
//! gain linearly), and the coefficients are rebuilt only while they glide, so the result depends
//! on the block size.

use super::params::{choice, flag, float};
use super::{DEFAULT_SPEC, ramp_length, scale, store_params};
use crate::filters::{
    BiquadType, ButterworthSlope, IDENTITY_SECTION, MAX_CASCADE_SECTIONS, Section, Smoothed,
    SmoothedValue, SosFilter, biquad, butterworth, db_to_amp,
};
use crate::processor::{BlockContext, ParamSlots, ProcessSpec, Processor};

/// The plugin this processor implements.
pub(crate) const PLUGIN_ID: &str = "sonara.eq.parametric";

// Slot indices, in the descriptor's parameter order.
const HP_ON: usize = 0;
const HP_FREQ: usize = 1;
const HP_SLOPE: usize = 2;
const LOW_ON: usize = 3;
const LOW_TYPE: usize = 4;
const LOW_FREQ: usize = 5;
const LOW_GAIN: usize = 6;
const LOW_Q: usize = 7;
const MID1_ON: usize = 8;
const MID1_FREQ: usize = 9;
const MID1_GAIN: usize = 10;
const MID1_Q: usize = 11;
const MID2_ON: usize = 12;
const MID2_FREQ: usize = 13;
const MID2_GAIN: usize = 14;
const MID2_Q: usize = 15;
const HIGH_ON: usize = 16;
const HIGH_TYPE: usize = 17;
const HIGH_FREQ: usize = 18;
const HIGH_GAIN: usize = 19;
const HIGH_Q: usize = 20;
const LP_ON: usize = 21;
const LP_FREQ: usize = 22;
const LP_SLOPE: usize = 23;
const OUTPUT_DB: usize = 24;

/// The fraction of the remaining distance a band covers per block (`GLIDE`).
const GLIDE: f64 = 0.35;
/// A frequency or Q glides while `|ln(current / target)|` exceeds this.
const LOG_EPSILON: f64 = 1e-4;
/// A gain glides while it is farther than this from its target, in dB.
const GAIN_EPSILON: f64 = 1e-3;
/// Bands in the cascade; they are always present (0 dB when off), so toggling one keeps the
/// section count and with it the filter state.
const BANDS: usize = 4;
/// The largest design: a 48 dB high-pass, the four bands and a 48 dB low-pass.
const MAX_SECTIONS: usize = 2 * MAX_CASCADE_SECTIONS + BANDS;

/// One band (`_Band`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Band {
    kind: BiquadType,
    freq: f64,
    gain: f64,
    q: f64,
    on: bool,
}

impl Band {
    /// Moves toward `target` by one block's glide; returns whether it is still moving
    /// (`glide_to`). The response and the switch follow at once.
    fn glide_to(&mut self, target: &Self) -> bool {
        self.kind = target.kind;
        self.on = target.on;
        let mut moving = false;
        if (self.freq / target.freq).ln().abs() > LOG_EPSILON {
            self.freq *= (target.freq / self.freq).powf(GLIDE);
            moving = true;
        } else {
            self.freq = target.freq;
        }
        if (self.gain - target.gain).abs() > GAIN_EPSILON {
            self.gain += (target.gain - self.gain) * GLIDE;
            moving = true;
        } else {
            self.gain = target.gain;
        }
        if (self.q / target.q).ln().abs() > LOG_EPSILON {
            self.q *= (target.q / self.q).powf(GLIDE);
            moving = true;
        } else {
            self.q = target.q;
        }
        moving
    }

    /// The band's section: its gain, or 0 dB when it is off.
    fn section(&self, sample_rate: f64) -> Section {
        let gain = if self.on { self.gain } else { 0.0 };
        biquad(self.kind, self.freq, sample_rate, self.q, gain)
    }
}

/// The high- or low-pass (`_hp`, `_lp`): on, frequency, slope.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pass {
    on: bool,
    freq: f64,
    slope: ButterworthSlope,
}

/// The `hp_slope`/`lp_slope` choices, in descriptor order: 12, 24, 36 and 48 dB/octave.
fn slope(index: u16) -> Option<ButterworthSlope> {
    match index {
        0 => Some(ButterworthSlope::Db12),
        1 => Some(ButterworthSlope::Db24),
        2 => Some(ButterworthSlope::Db36),
        3 => Some(ButterworthSlope::Db48),
        _ => None,
    }
}

/// The `low_type`/`high_type` choices, in descriptor order: `shelf` then `peak`.
fn band_kind(index: u16, shelf: BiquadType) -> Option<BiquadType> {
    match index {
        0 => Some(shelf),
        1 => Some(BiquadType::Peak),
        _ => None,
    }
}

/// The decoded parameter set.
struct Settings {
    bands: [Band; BANDS],
    hp: Pass,
    lp: Pass,
    output_db: f64,
}

impl Settings {
    fn from_slots(slots: &ParamSlots) -> Option<Self> {
        let band = |kind: BiquadType, on: usize, freq: usize, gain: usize, q: usize| {
            Some(Band {
                kind,
                freq: float(slots, freq)?,
                gain: float(slots, gain)?,
                q: float(slots, q)?,
                on: flag(slots, on)?,
            })
        };
        let low = band_kind(choice(slots, LOW_TYPE)?, BiquadType::LowShelf)?;
        let high = band_kind(choice(slots, HIGH_TYPE)?, BiquadType::HighShelf)?;
        Some(Self {
            bands: [
                band(low, LOW_ON, LOW_FREQ, LOW_GAIN, LOW_Q)?,
                band(BiquadType::Peak, MID1_ON, MID1_FREQ, MID1_GAIN, MID1_Q)?,
                band(BiquadType::Peak, MID2_ON, MID2_FREQ, MID2_GAIN, MID2_Q)?,
                band(high, HIGH_ON, HIGH_FREQ, HIGH_GAIN, HIGH_Q)?,
            ],
            hp: Pass {
                on: flag(slots, HP_ON)?,
                freq: float(slots, HP_FREQ)?,
                slope: slope(choice(slots, HP_SLOPE)?)?,
            },
            lp: Pass {
                on: flag(slots, LP_ON)?,
                freq: float(slots, LP_FREQ)?,
                slope: slope(choice(slots, LP_SLOPE)?)?,
            },
            output_db: float(slots, OUTPUT_DB)?,
        })
    }
}

/// The parametric EQ (the prototype's `ParametricEQ`).
pub(crate) struct ParametricEq {
    spec: ProcessSpec,
    params: ParamSlots,
    filter: SosFilter,
    /// The bands as they sound now; `None` until the first parameters after a reset, which
    /// apply without a glide.
    current: Option<[Band; BANDS]>,
    target: [Band; BANDS],
    hp: Pass,
    lp: Pass,
    out: SmoothedValue,
    /// The values of a running output ramp, one block long.
    ramp: Vec<f64>,
}

impl ParametricEq {
    /// Setup: an unprepared processor.
    pub(crate) fn new() -> Self {
        let flat = Band {
            kind: BiquadType::Peak,
            freq: 1000.0,
            gain: 0.0,
            q: 1.0,
            on: false,
        };
        Self {
            spec: DEFAULT_SPEC,
            params: ParamSlots::EMPTY,
            filter: SosFilter::new(2, MAX_SECTIONS),
            current: None,
            target: [flat; BANDS],
            hp: Pass {
                on: false,
                freq: 20.0,
                slope: ButterworthSlope::Db12,
            },
            lp: Pass {
                on: false,
                freq: 20_000.0,
                slope: ButterworthSlope::Db12,
            },
            out: SmoothedValue::new(),
            ramp: Vec::new(),
        }
    }

    /// The prototype's `on_params_changed`.
    fn apply_params(&mut self) {
        let Some(p) = Settings::from_slots(&self.params) else {
            return;
        };
        self.target = p.bands;
        self.hp = p.hp;
        self.lp = p.lp;
        self.out.set_ramp(ramp_length(self.spec.block_size));
        self.out.set_target(db_to_amp(p.output_db));
        if self.current.is_none() {
            self.current = Some(self.target);
        }
        self.rebuild();
    }

    /// Designs the cascade from the current bands (`_rebuild`).
    fn rebuild(&mut self) {
        let Some(current) = &self.current else {
            return;
        };
        let design = Cascade::design(
            current,
            &self.hp,
            &self.lp,
            f64::from(self.spec.sample_rate),
        );
        // ERR-06: cannot fail, the filter holds MAX_SECTIONS, the largest design.
        let _ = self.filter.set_sections(design.sections());
    }
}

/// The sections of one design, held inline (at most [`MAX_SECTIONS`]).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cascade {
    sections: [Section; MAX_SECTIONS],
    len: usize,
}

impl Cascade {
    /// The cascade of `bands` between the high- and the low-pass, in filtering order: the one
    /// design of the processor and of [`parametric_eq_design`] (DES-01).
    fn design(bands: &[Band; BANDS], hp: &Pass, lp: &Pass, sample_rate: f64) -> Self {
        let mut cascade = Self {
            sections: [IDENTITY_SECTION; MAX_SECTIONS],
            len: 0,
        };
        let mut add = |section: Section| {
            if let Some(slot) = cascade.sections.get_mut(cascade.len) {
                *slot = section;
                cascade.len += 1;
            }
        };
        if hp.on {
            let design = butterworth(BiquadType::Highpass, hp.freq, sample_rate, hp.slope);
            design.sections().iter().copied().for_each(&mut add);
        }
        bands
            .iter()
            .map(|band| band.section(sample_rate))
            .for_each(&mut add);
        if lp.on {
            let design = butterworth(BiquadType::Lowpass, lp.freq, sample_rate, lp.slope);
            design.sections().iter().copied().for_each(&mut add);
        }
        cascade
    }

    fn sections(&self) -> &[Section] {
        self.sections.get(..self.len).unwrap_or_default()
    }
}

/// What the parametric EQ (`sonara.eq.parametric`) applies once its parameters have settled:
/// the cascade of second-order sections and the output gain after it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EqDesign {
    cascade: Cascade,
    /// The output gain in dB (`output_db`), applied after the cascade.
    pub output_db: f64,
}

impl EqDesign {
    /// The sections in filtering order: the high-pass (when on), the low, mid 1, mid 2 and high
    /// bands (0 dB when off), then the low-pass (when on).
    #[must_use]
    pub fn sections(&self) -> &[Section] {
        self.cascade.sections()
    }
}

/// The design the parametric EQ runs for the parameter set `params` at `sample_rate` once its
/// bands have stopped gliding: the same function as the processor's own rebuild, so a drawn
/// response equals what plays (the prototype's `eq_sections` in `ui/devices/panel.py`, which
/// copied the processor's code instead).
///
/// `params` are the device's values in descriptor order ([`super::param_slots`]); `None` when
/// they do not decode (a missing slot, a slot of another type or an unknown choice), the case
/// in which the processor keeps its previous settings.
///
/// # Examples
///
/// ```
/// use sonara_core::model::device::Params;
/// use sonara_core::plugins::builtin_registry;
/// use sonara_dsp::processors::{param_slots, parametric_eq_design};
///
/// let registry = builtin_registry();
/// let descriptor = registry.get("sonara.eq.parametric").unwrap();
/// let slots = param_slots(descriptor, &Params::new()).unwrap();
/// let design = parametric_eq_design(&slots, 48_000.0).unwrap();
/// // The four bands; the high- and low-pass are off by default.
/// assert_eq!(design.sections().len(), 4);
/// assert_eq!(design.output_db, 0.0);
/// ```
#[must_use]
pub fn parametric_eq_design(params: &ParamSlots, sample_rate: f64) -> Option<EqDesign> {
    let settings = Settings::from_slots(params)?;
    Some(EqDesign {
        cascade: Cascade::design(&settings.bands, &settings.hp, &settings.lp, sample_rate),
        output_db: settings.output_db,
    })
}

impl Processor for ParametricEq {
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    fn prepare(&mut self, spec: ProcessSpec) {
        self.spec = spec;
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
        reason = "the prototype skips the output gain exactly when its steady value is 1.0"
    )]
    fn process(&mut self, buffer: &mut [[f64; 2]], _ctx: &BlockContext<'_>) {
        let mut moving = false;
        if let Some(current) = &mut self.current {
            // Every band glides, so no short-circuit.
            for (band, target) in current.iter_mut().zip(&self.target) {
                moving |= band.glide_to(target);
            }
        }
        if moving {
            self.rebuild();
        }
        let n = buffer.len().min(self.ramp.len());
        let (Some(buffer), Some(ramp)) = (buffer.get_mut(..n), self.ramp.get_mut(..n)) else {
            return;
        };
        self.filter.process_in_place(buffer.as_flattened_mut());
        match self.out.next_block(ramp) {
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
        // The prototype makes a new identity filter; the state is zero either way, and the
        // next design starts from it.
        self.filter.reset();
        self.filter.set_identity();
        self.current = None;
    }
}

#[cfg(test)]
mod tests {
    use sonara_core::model::device::{ParamValue, Params};
    use sonara_core::plugins::builtin_registry;

    use super::{
        Band, GLIDE, HIGH_FREQ, HIGH_GAIN, HIGH_ON, HIGH_Q, HIGH_TYPE, HP_FREQ, HP_ON, HP_SLOPE,
        LOW_FREQ, LOW_GAIN, LOW_ON, LOW_Q, LOW_TYPE, LP_FREQ, LP_ON, LP_SLOPE, MID1_FREQ,
        MID1_GAIN, MID1_ON, MID1_Q, MID2_FREQ, MID2_GAIN, MID2_ON, MID2_Q, OUTPUT_DB, PLUGIN_ID,
        ParametricEq,
    };
    use crate::filters::BiquadType;
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
            (HP_ON, "hp_on"),
            (HP_FREQ, "hp_freq"),
            (HP_SLOPE, "hp_slope"),
            (LOW_ON, "low_on"),
            (LOW_TYPE, "low_type"),
            (LOW_FREQ, "low_freq"),
            (LOW_GAIN, "low_gain"),
            (LOW_Q, "low_q"),
            (MID1_ON, "mid1_on"),
            (MID1_FREQ, "mid1_freq"),
            (MID1_GAIN, "mid1_gain"),
            (MID1_Q, "mid1_q"),
            (MID2_ON, "mid2_on"),
            (MID2_FREQ, "mid2_freq"),
            (MID2_GAIN, "mid2_gain"),
            (MID2_Q, "mid2_q"),
            (HIGH_ON, "high_on"),
            (HIGH_TYPE, "high_type"),
            (HIGH_FREQ, "high_freq"),
            (HIGH_GAIN, "high_gain"),
            (HIGH_Q, "high_q"),
            (LP_ON, "lp_on"),
            (LP_FREQ, "lp_freq"),
            (LP_SLOPE, "lp_slope"),
            (OUTPUT_DB, "output_db"),
        ];
        assert_eq!(ids.len(), slots.len());
        for (index, id) in slots {
            assert_eq!(ids.get(index), Some(&id));
        }
    }

    #[test]
    fn a_band_glides_geometrically_and_settles() {
        let target = Band {
            kind: BiquadType::Peak,
            freq: 1000.0,
            gain: 6.0,
            q: 2.0,
            on: true,
        };
        let mut band = Band {
            kind: BiquadType::LowShelf,
            freq: 100.0,
            gain: 0.0,
            q: 1.0,
            on: false,
        };
        assert!(band.glide_to(&target));
        assert_eq!((band.kind, band.on), (BiquadType::Peak, true));
        assert_eq!(band.freq, 100.0 * 10.0_f64.powf(GLIDE));
        assert_eq!(band.gain, 6.0 * GLIDE);
        let mut blocks = 1;
        while band.glide_to(&target) {
            blocks += 1;
        }
        assert_eq!(band, target, "lands on the target exactly");
        assert!(blocks > 10, "{blocks} blocks");
    }

    #[test]
    fn the_first_parameters_apply_at_once_and_changes_glide() {
        let mut eq = ParametricEq::new();
        eq.prepare(ProcessSpec {
            sample_rate: 48_000,
            block_size: 32,
            channels: 2,
        });
        let boost = slots(&[("mid1_gain", ParamValue::Float(12.0))]);
        eq.set_params(&boost);
        let current = eq.current.unwrap();
        assert_eq!(current.get(1).map(|b| b.gain), Some(12.0));
        // A change glides over blocks; an equal set keeps the glide going.
        eq.set_params(&slots(&[]));
        let mut block = [[0.0; 2]; 32];
        eq.process(&mut block, &ctx());
        let after_one = eq.current.unwrap().get(1).map(|b| b.gain);
        assert_eq!(after_one, Some(12.0 + (0.0 - 12.0) * GLIDE));
        eq.set_params(&slots(&[]));
        eq.process(&mut block, &ctx());
        assert!(eq.current.unwrap().get(1).is_some_and(|b| b.gain < 7.8));
        // A reset forgets the current bands: the next parameters apply without a glide.
        eq.reset();
        assert!(eq.current.is_none());
    }

    /// `test_eq_sections_match_processor` (`tests/gui/test_mixer_devices.py`): the design a GUI
    /// draws equals the cascade the prepared processor runs, section for section. Here both are
    /// one function, so the test pins that the processor really uses it, for the prototype
    /// test's parameters and for every slope and band type.
    #[test]
    fn the_public_design_is_the_processors_own_cascade() {
        let prototype: &[(&str, ParamValue)] = &[
            ("hp_on", ParamValue::Bool(true)),
            ("hp_freq", ParamValue::Float(60.0)),
            ("low_gain", ParamValue::Float(4.0)),
            ("mid2_gain", ParamValue::Float(-5.0)),
            ("lp_on", ParamValue::Bool(true)),
        ];
        let every_switch: &[(&str, ParamValue)] = &[
            ("hp_on", ParamValue::Bool(true)),
            ("hp_slope", ParamValue::from("48")),
            ("low_type", ParamValue::from("peak")),
            ("high_type", ParamValue::from("peak")),
            ("high_on", ParamValue::Bool(false)),
            ("mid1_gain", ParamValue::Float(9.0)),
            ("lp_on", ParamValue::Bool(true)),
            ("lp_slope", ParamValue::from("36")),
            ("output_db", ParamValue::Float(-3.0)),
        ];
        let cases = [prototype, every_switch, &[]];
        for (case, values) in cases.iter().enumerate() {
            let params = slots(values);
            for sample_rate in [44_100, 48_000, 96_000] {
                let mut eq = ParametricEq::new();
                eq.prepare(ProcessSpec {
                    sample_rate,
                    block_size: 512,
                    channels: 2,
                });
                eq.set_params(&params);
                let design = super::parametric_eq_design(&params, f64::from(sample_rate))
                    .expect("the parameters decode");
                assert_eq!(
                    design.sections(),
                    eq.filter.sections(),
                    "case {case} at {sample_rate} Hz"
                );
            }
        }
        let flat = super::parametric_eq_design(&slots(&[]), 48_000.0).unwrap();
        assert_eq!(flat.sections().len(), 4);
        let full = super::parametric_eq_design(&slots(every_switch), 48_000.0).unwrap();
        // A 48 dB high-pass (4 sections), the four bands and a 36 dB low-pass (3 sections).
        assert_eq!(full.sections().len(), 11);
        assert_eq!(full.output_db, -3.0);
    }

    #[test]
    fn flat_bands_pass_the_signal_through_the_cascade() {
        let mut eq = ParametricEq::new();
        eq.prepare(ProcessSpec {
            sample_rate: 48_000,
            block_size: 64,
            channels: 2,
        });
        eq.set_params(&slots(&[]));
        let mut block = [[0.0; 2]; 64];
        if let Some(first) = block.first_mut() {
            *first = [1.0, -0.5];
        }
        eq.process(&mut block, &ctx());
        // Four 0 dB peaks and shelves are identities up to rounding.
        let first = block.first().copied().unwrap();
        assert!((first[0] - 1.0).abs() < 1e-12 && (first[1] + 0.5).abs() < 1e-12);
        assert!(block.iter().skip(1).all(|f| f[0].abs() < 1e-12));
    }
}
