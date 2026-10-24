//! Real-time: `process`, `reset` and `set_params` with an unchanged impulse-response key run on
//! the audio render thread (see docs/audio-engine.md); `prepare` and the synthesis of an impulse
//! response are setup.
//!
//! `sonara.reverb`: an algorithmic reverb that convolves the mono sum with a synthesized stereo
//! impulse response, as the prototype's `Reverb` and `synthesize_ir`
//! (`dsp/time_frequency/reverb.py`) compute it on the single-precision `PartitionedConvolver`
//! ([`crate::convolution`] with `f32`).
//!
//! **The impulse response** is a pure function of the rounded parameters, the key
//! `(sr, round(decay, 3), round(predelay, 1), round(damping, 3), round(lowcut, 1),
//! round(highcut, 1), round(width, 3), character)` with Python's `round`. The prototype
//! memoizes `synthesize_ir` over the key (`lru_cache`), which changes nothing but its speed;
//! here the processor keeps the key of the installed response and installs a response only
//! when the key changes, and it memoizes the last synthesized response with its key (one entry
//! of the `lru_cache`). [`synthesize_ir`] reproduces it step by step: `default_rng(1234)`
//! noise, the band filters of [`reverb_bands`] from zero state, per-band
//! decays, the onset, the early reflections (whose sign is drawn only when the reflection lies
//! inside the response), width, the Butterworth band limits and the normalization to unit
//! energy of the louder channel (sums sequential per column).
//!
//! **When the synthesis runs.** It allocates (up to 30 s of stereo noise, bands and response)
//! and partitions the response, so it is setup. A response is installed in `prepare`, when the
//! parameters are known, and in the `set_params` call whose key differs from the installed one:
//! in the render graph that is the device setup (prepare, `set_state`, `set_params`), off the
//! real-time thread. `prepare` builds a new convolver and installs the response again, as the
//! prototype's `reset` and `on_params_changed` do, but takes it from the memo when the key is
//! unchanged, so the render graph's build and its reset before an offline render (a second
//! `prepare`) synthesize the response once. The memo holds the samples of one response, about
//! 3 MB for a 2.5 s decay at 48 kHz. The prototype synthesizes in the block as well when an
//! automated decay, pre-delay, damping, band limit, width or character changes the key; that
//! would run this synthesis on the render thread, which RT-02 forbids; a live graph therefore
//! defers it (below). Every `set_params` with an unchanged key allocates nothing.
//!
//! **Deferred setup** (a live graph after [`Processor::defer_setup`]): a `set_params`
//! whose key differs from the installed one keeps the installed response, mixes and reports
//! its tail with the new parameters at once, and names the key it waits for in
//! [`Processor::setup_request`]. The builder thread synthesizes and partitions that response
//! ([`prepare_ir`]: the same steps as the offline install), and
//! [`Processor::install_setup`] moves it in with the same crossfade, without allocating. An
//! install at a block therefore equals an offline `set_params` at that block; until it
//! arrives, the old response plays on. At a sample rate without band tables nothing is
//! synthesized, so the key is installed at once, as offline.
//!
//! **Processing:** the mono sum `(l + r) / 2` on both channels through the convolver, then
//! `buffer · (1 − m) + wet · m`. Both channels carry the same signal, so the convolver shares
//! one input spectrum between them ([`InputMode::Shared`]), with the same output. With a steady
//! mix and an ordinary block the wet signal is float32, and NumPy (NEP 50) computes `wet · m` in
//! float32 with `m` rounded to float32; a crossfade block, a ramping mix and the silence before
//! the first response are float64.
//! Exactness: the synthesis and the mixing are the prototype's arithmetic; the only inexact
//! step is the convolver's float32 inverse FFT, a few float32 ulps of the wet peak.
//!
//! At a sample rate the prototype does not support there are no band tables, so there is no
//! impulse response and the reverb is silent (the dry part of the mix passes).

use super::convolver::{ConvolutionPath, MAX_IR_SECONDS};
use super::params::{choice, float};
use super::pyops::{maximum, py_max, py_min};
use super::setup::{Prepared, Request};
use super::{DEFAULT_SPEC, ramp_length, store_params};
use crate::convolution::{InputMode, LayoutMismatch, OutputPrecision, PreparedIr};
use crate::filters::{
    BiquadType, ButterworthSlope, Smoothed, SmoothedValue, SosFilter, butterworth,
};
use crate::numpy::math::py_round;
use crate::numpy::random::default_rng;
use crate::processor::{
    BlockContext, ParamSlots, PreparedSetup, ProcessSpec, Processor, SetupRequest,
};
use crate::tables::{SupportedRate, reverb_bands};

/// The plugin this processor implements.
pub(crate) const PLUGIN_ID: &str = "sonara.reverb";

// Slot indices, in the descriptor's parameter order.
const CHARACTER: usize = 0;
const DECAY_S: usize = 1;
const PREDELAY_MS: usize = 2;
const DAMPING: usize = 3;
const LOWCUT_HZ: usize = 4;
const HIGHCUT_HZ: usize = 5;
const WIDTH: usize = 6;
const MIX: usize = 7;

/// The decay before the first parameters, in seconds (`_decay`).
const INITIAL_DECAY_S: f64 = 2.0;
/// The noise seed of `synthesize_ir`.
const SEED: u64 = 1234;
/// The shortest response, in frames.
const MIN_FRAMES: usize = 64;
/// `ln(1000)` as the prototype writes it: the decay to −60 dB over the RT60.
const RT60_LN: f64 = 6.907_755;
/// The band-limit high-pass is on above this frequency, in Hz.
const LOWCUT_ON_ABOVE: f64 = 21.0;
/// The band-limit low-pass is on below `min(19999, 0.98 · nyquist)`, in Hz.
const HIGHCUT_ON_BELOW: f64 = 19_999.0;

/// The `character` choices, in descriptor order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Character {
    Room,
    Chamber,
    Hall,
    Plate,
}

impl Character {
    fn from_choice(index: u16) -> Option<Self> {
        match index {
            0 => Some(Self::Room),
            1 => Some(Self::Chamber),
            2 => Some(Self::Hall),
            3 => Some(Self::Plate),
            _ => None,
        }
    }

    /// `_CHARACTERS`: the onset of the diffuse tail in ms, the number of early reflections and
    /// their spread in ms.
    fn shape(self) -> (f64, usize, f64) {
        match self {
            Self::Room => (4.0, 8, 25.0),
            Self::Hall => (18.0, 14, 80.0),
            Self::Plate => (1.0, 0, 0.0),
            Self::Chamber => (8.0, 10, 45.0),
        }
    }
}

/// The decoded parameter set.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Settings {
    character: Character,
    decay_s: f64,
    predelay_ms: f64,
    damping: f64,
    lowcut_hz: f64,
    highcut_hz: f64,
    width: f64,
    mix: f64,
}

impl Settings {
    fn from_slots(slots: &ParamSlots) -> Option<Self> {
        Some(Self {
            character: Character::from_choice(choice(slots, CHARACTER)?)?,
            decay_s: float(slots, DECAY_S)?,
            predelay_ms: float(slots, PREDELAY_MS)?,
            damping: float(slots, DAMPING)?,
            lowcut_hz: float(slots, LOWCUT_HZ)?,
            highcut_hz: float(slots, HIGHCUT_HZ)?,
            width: float(slots, WIDTH)?,
            mix: float(slots, MIX)?,
        })
    }
}

/// The arguments of `synthesize_ir`: the processor's rounded key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct IrKey {
    sample_rate: u32,
    decay: f64,
    predelay_ms: f64,
    damping: f64,
    lowcut: f64,
    highcut: f64,
    width: f64,
    character: Character,
}

impl IrKey {
    /// The key of `on_params_changed`, rounded with Python's `round` (stack-only, so it can be
    /// computed on the render thread).
    fn new(sample_rate: u32, p: &Settings) -> Self {
        Self {
            sample_rate,
            decay: py_round(p.decay_s, 3),
            predelay_ms: py_round(p.predelay_ms, 1),
            damping: py_round(p.damping, 3),
            lowcut: py_round(p.lowcut_hz, 1),
            highcut: py_round(p.highcut_hz, 1),
            width: py_round(p.width, 3),
            character: p.character,
        }
    }
}

/// `synthesize_ir(*key)`: the interleaved `(taps, 2)` response, normalized to unit energy in
/// its louder channel and preceded by the pre-delay. Setup: allocates about `6 · taps` floats.
#[expect(
    clippy::disallowed_macros,
    reason = "setup: synthesizes at device setup, off the real-time thread"
)]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "Python's int() of positive lengths in frames, and frame indices far below 2^53"
)]
fn synthesize_ir(rate: SupportedRate, key: &IrKey) -> Vec<f64> {
    let sr = f64::from(rate.hz());
    let (onset_ms, early, spread_ms) = key.character.shape();
    let length = py_min(
        MAX_IR_SECONDS,
        key.decay * 1.5 + key.predelay_ms * 0.001 + 0.05,
    );
    let n = ((length * sr) as usize).max(MIN_FRAMES);
    let mut rng = default_rng(SEED);
    // `rng.standard_normal((n, 2))`, C order.
    let mut noise = vec![[0.0; 2]; n];
    rng.standard_normal_fill(noise.as_flattened_mut());
    let mut ir = vec![[0.0; 2]; n];
    let mut band = vec![[0.0; 2]; n];
    // `t = np.arange(n) / sr`.
    let t = |i: usize| i as f64 / sr;
    for spec in reverb_bands(rate) {
        let mut filter = SosFilter::new(2, spec.sections.len());
        // ERR-06: cannot fail, the filter holds the band's sections.
        let _ = filter.set_sections(spec.sections);
        filter.process(noise.as_flattened(), band.as_flattened_mut());
        let centre = (py_max(spec.lo, 60.0) * spec.hi).sqrt();
        // High bands decay faster: down to (1 − 0.85 · damping) of the RT60 at 16 kHz.
        let tilt = py_min(
            1.0,
            py_max(0.0, (py_max(centre, 500.0) / 500.0).log2() / 5.0),
        );
        let rt = py_max(0.05, key.decay * (1.0 - 0.85 * key.damping * tilt));
        // `ir += band * exp(−6.907755 · t / rt)`.
        for (i, (frame, b)) in ir.iter_mut().zip(&band).enumerate() {
            let envelope = (-RT60_LN * t(i) / rt).exp();
            for (y, &v) in frame.iter_mut().zip(b) {
                *y += v * envelope;
            }
        }
    }
    // `ir *= 1 − exp(−t / max(onset · 0.001, 1e-4))`.
    let onset = py_max(onset_ms * 0.001, 1e-4);
    for (i, frame) in ir.iter_mut().enumerate() {
        let envelope = 1.0 - (-t(i) / onset).exp();
        for y in frame {
            *y *= envelope;
        }
    }
    let early_gain = (sr / 48_000.0).sqrt();
    let divisor = early.max(1) as f64;
    for k in 0..early {
        let delay = ((0.002 + rng.random() * spread_ms * 0.001) * sr) as usize;
        if delay < n {
            let mut sign = [0.0; 2];
            // ERR-06: cannot fail, the population is not empty.
            let _ = rng.choice(&[-1.0, 1.0], &mut sign);
            let gain = 0.6 * (1.0 - k as f64 / divisor);
            if let Some(frame) = ir.get_mut(delay) {
                for (y, s) in frame.iter_mut().zip(sign) {
                    *y += gain * s * early_gain;
                }
            }
        }
    }
    // Width: `mid ± width · side`.
    for frame in &mut ir {
        let [left, right] = *frame;
        #[expect(
            clippy::manual_midpoint,
            reason = "RT-11: NumPy's two-channel mean, written as it computes it"
        )]
        let mid = (left + right) / 2.0;
        let side = (left - right) / 2.0;
        *frame = [mid + key.width * side, mid - key.width * side];
    }
    let nyquist = sr / 2.0;
    if key.lowcut > LOWCUT_ON_ABOVE {
        band_limit(&mut ir, BiquadType::Highpass, key.lowcut, sr);
    }
    if key.highcut < py_min(HIGHCUT_ON_BELOW, nyquist * 0.98) {
        band_limit(&mut ir, BiquadType::Lowpass, key.highcut, sr);
    }
    // `ir /= max(1e-12, sqrt(sum(ir**2, axis=0)).max())`.
    let mut energy = [0.0; 2];
    for frame in &ir {
        for (acc, &v) in energy.iter_mut().zip(frame) {
            *acc += v * v;
        }
    }
    let [left, right] = energy;
    let norm = py_max(1e-12, maximum(left.sqrt(), right.sqrt()));
    for v in ir.as_flattened_mut() {
        *v /= norm;
    }
    let pre = (key.predelay_ms * 0.001 * sr) as usize;
    if pre == 0 {
        return ir.into_flattened();
    }
    let mut delayed = vec![0.0; 2 * pre];
    delayed.extend_from_slice(ir.as_flattened());
    delayed
}

/// The response for `key`, partitioned for a convolver of `block_size`-frame stereo blocks:
/// what [`Reverb::install_ir`] builds through `set_ir`, prepared off the real-time thread for
/// deferred setup. `None` at a sample rate without band tables (no response, as offline) or for
/// a block size the convolver rejects.
///
/// Setup: allocates.
pub(super) fn prepare_ir(key: &IrKey, block_size: usize) -> Option<PreparedIr<f32>> {
    let rate = SupportedRate::from_hz(key.sample_rate)?;
    let ir = synthesize_ir(rate, key);
    PreparedIr::with_input(block_size, 2, InputMode::Shared, &ir, 2).ok()
}

/// `sosfilt(butterworth(kind, freq, sr, 12), ir, axis=0)` from zero state, in place.
fn band_limit(ir: &mut [[f64; 2]], kind: BiquadType, freq: f64, sample_rate: f64) {
    let design = butterworth(kind, freq, sample_rate, ButterworthSlope::Db12);
    let mut filter = SosFilter::new(2, 1);
    // ERR-06: cannot fail, a 12 dB design is one section.
    let _ = filter.set_sections(design.sections());
    filter.process_in_place(ir.as_flattened_mut());
}

/// The reverb (the prototype's `Reverb`).
pub(crate) struct Reverb {
    spec: ProcessSpec,
    params: ParamSlots,
    path: ConvolutionPath<f32>,
    /// The key of the installed response (`_ir_key`).
    ir_key: Option<IrKey>,
    /// The last synthesized response with its key: one entry of the prototype's `lru_cache`
    /// over `synthesize_ir`. `prepare` keeps it.
    synthesized: Option<(IrKey, Vec<f64>)>,
    /// How many responses were synthesized (unit tests count the syntheses per setup).
    #[cfg(test)]
    syntheses: usize,
    mix: SmoothedValue,
    decay: f64,
    mix_ramp: Vec<f64>,
    /// Deferred setup ([`Processor::defer_setup`]): a key change waits for
    /// [`Processor::install_setup`].
    deferred: bool,
    /// The key the parameters ask for while it is not installed (deferred setup only).
    wanted: Option<IrKey>,
}

impl Reverb {
    /// Setup: an unprepared processor.
    pub(crate) fn new() -> Self {
        Self {
            spec: DEFAULT_SPEC,
            params: ParamSlots::EMPTY,
            path: ConvolutionPath::new(InputMode::Shared),
            ir_key: None,
            synthesized: None,
            #[cfg(test)]
            syntheses: 0,
            mix: SmoothedValue::new(),
            decay: INITIAL_DECAY_S,
            mix_ramp: Vec::new(),
            deferred: false,
            wanted: None,
        }
    }

    /// The prototype's `on_params_changed`: the mix and the tail, and a new response when the
    /// key changed (see the module docs for when that happens). With `install` false (deferred
    /// setup) a new key that needs a synthesis is only recorded in `wanted`.
    fn apply_params(&mut self, install: bool) {
        let Some(p) = Settings::from_slots(&self.params) else {
            return;
        };
        let key = IrKey::new(self.spec.sample_rate, &p);
        self.decay = p.decay_s;
        self.mix.set_ramp(ramp_length(self.spec.block_size));
        self.mix.set_target(p.mix);
        if self.ir_key == Some(key) {
            self.wanted = None;
        } else if self.path.is_prepared() {
            // Without band tables the install synthesizes nothing, so it never waits.
            if install || SupportedRate::from_hz(key.sample_rate).is_none() {
                self.install_ir(key);
                self.wanted = None;
            } else {
                self.wanted = Some(key);
            }
        }
    }

    /// Setup: installs the response for `key`, crossfading when one was installed before
    /// (`set_ir(ir, crossfade=_ir_key is not None)`). The response comes from the memo when it
    /// was synthesized for `key`; otherwise it is synthesized and replaces the memo
    /// (`synthesize_ir` is a pure function of the key, so both give the same samples).
    fn install_ir(&mut self, key: IrKey) {
        if let Some(rate) = SupportedRate::from_hz(key.sample_rate) {
            if self
                .synthesized
                .as_ref()
                .is_none_or(|(memo, _)| *memo != key)
            {
                let ir = synthesize_ir(rate, &key);
                #[cfg(test)]
                {
                    self.syntheses += 1;
                }
                self.synthesized = Some((key, ir));
            }
            if let Some((_, ir)) = &self.synthesized {
                self.path.set_ir(ir, 2, self.ir_key.is_some());
            }
        }
        self.ir_key = Some(key);
    }
}

impl Processor for Reverb {
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    fn prepare(&mut self, spec: ProcessSpec) {
        self.spec = spec;
        // `reset`: a new single-precision convolver and no key; the parameters below
        // synthesize the response.
        self.path.prepare(spec.block_size);
        self.ir_key = None;
        self.mix_ramp = vec![0.0; spec.block_size];
        self.apply_params(true);
    }

    fn set_params(&mut self, params: &ParamSlots) {
        if store_params(&mut self.params, params) {
            self.apply_params(!self.deferred);
        }
    }

    fn process(&mut self, buffer: &mut [[f64; 2]], _ctx: &BlockContext<'_>) {
        // The convolver takes exactly one block, as the prototype asserts; another length
        // passes unchanged.
        if buffer.len() != self.path.block_size() {
            return;
        }
        for (x, &[l, r]) in self.path.input_mut().iter_mut().zip(buffer.iter()) {
            // `np.repeat(buffer.mean(axis=1, keepdims=True), 2, axis=1)`.
            #[expect(
                clippy::manual_midpoint,
                reason = "RT-11: NumPy's two-channel mean, written as it computes it"
            )]
            let mono = (l + r) / 2.0;
            *x = [mono, mono];
        }
        let Some(precision) = self.path.process() else {
            return;
        };
        let Some(mix) = self.mix_ramp.get_mut(..buffer.len()) else {
            return;
        };
        let wet = self.path.output();
        // `buffer * (1 − m) + wet * m`.
        match (self.mix.next_block(mix), precision) {
            (Smoothed::Steady(m), OutputPrecision::Single) => {
                // NEP 50: a float32 array times a Python float is float32.
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "NumPy casts the weak Python float to float32, rounding to nearest"
                )]
                let (m32, to_f32) = (m as f32, |v: f64| v as f32);
                let dry = 1.0 - m;
                for (frame, w) in buffer.iter_mut().zip(wet) {
                    for (y, &v) in frame.iter_mut().zip(w) {
                        *y = *y * dry + f64::from(to_f32(v) * m32);
                    }
                }
            }
            (Smoothed::Steady(m), OutputPrecision::Double) => {
                let dry = 1.0 - m;
                for (frame, w) in buffer.iter_mut().zip(wet) {
                    for (y, &v) in frame.iter_mut().zip(w) {
                        *y = *y * dry + v * m;
                    }
                }
            }
            (Smoothed::Ramp, _) => {
                for ((frame, w), &m) in buffer.iter_mut().zip(wet).zip(mix.iter()) {
                    let dry = 1.0 - m;
                    for (y, &v) in frame.iter_mut().zip(w) {
                        *y = *y * dry + v * m;
                    }
                }
            }
        }
    }

    /// Clears the convolution history and keeps the installed response: what the prototype's
    /// `reset` followed by `on_params_changed` (the render graph's reset) leaves behind.
    fn reset(&mut self) {
        self.path.reset();
    }

    /// `decay · 1.5 + 0.3`.
    fn tail_seconds(&self) -> f64 {
        self.decay * 1.5 + 0.3
    }

    fn defer_setup(&mut self) {
        self.deferred = true;
    }

    fn setup_request(&self) -> Option<SetupRequest> {
        let key = self.wanted.filter(|&key| self.ir_key != Some(key))?;
        Some(SetupRequest(Request::ReverbIr {
            key,
            block_size: self.path.block_size(),
        }))
    }

    /// Installs a prepared response unless its key is installed already, crossfading as
    /// [`Reverb::install_ir`] does. A response for a key the parameters have left meanwhile is
    /// installed too, as the closest one available; the parameters then ask again.
    fn install_setup(&mut self, setup: &mut PreparedSetup) -> bool {
        match &setup.0 {
            Prepared::ReverbIr { key, .. } if self.ir_key != Some(*key) => {}
            _ => return false,
        }
        let Prepared::ReverbIr { key, ir } = std::mem::replace(&mut setup.0, Prepared::Empty)
        else {
            return false;
        };
        match self.path.install(ir, self.ir_key.is_some()) {
            Ok(retired) => {
                setup.0 = Prepared::Displaced(retired);
                self.ir_key = Some(key);
                if self.wanted == Some(key) {
                    self.wanted = None;
                }
                true
            }
            Err(LayoutMismatch(ir)) => {
                setup.0 = Prepared::ReverbIr { key, ir };
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use sonara_core::model::device::{ParamValue, Params};
    use sonara_core::plugins::builtin_registry;

    use super::{
        CHARACTER, Character, DAMPING, DECAY_S, HIGHCUT_HZ, IrKey, LOWCUT_HZ, MIX, PLUGIN_ID,
        PREDELAY_MS, Request, Reverb, Settings, WIDTH, synthesize_ir,
    };
    use crate::processor::{BlockContext, ParamSlots, ProcessSpec, Processor, SetupRequest};
    use crate::processors::param_slots;
    use crate::tables::SupportedRate;

    const BLOCK: usize = 256;

    fn slots(values: &[(&str, ParamValue)]) -> ParamSlots {
        let registry = builtin_registry();
        let values: Params = values
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        param_slots(registry.get(PLUGIN_ID).unwrap(), &values).unwrap()
    }

    fn settings(values: &[(&str, ParamValue)]) -> Settings {
        Settings::from_slots(&slots(values)).unwrap()
    }

    #[test]
    fn slots_and_characters_follow_the_descriptor_order() {
        let registry = builtin_registry();
        let descriptor = registry.get(PLUGIN_ID).unwrap();
        let ids: Vec<&str> = descriptor.params.iter().map(|p| p.id.as_str()).collect();
        let slots = [
            (CHARACTER, "character"),
            (DECAY_S, "decay_s"),
            (PREDELAY_MS, "predelay_ms"),
            (DAMPING, "damping"),
            (LOWCUT_HZ, "lowcut_hz"),
            (HIGHCUT_HZ, "highcut_hz"),
            (WIDTH, "width"),
            (MIX, "mix"),
        ];
        assert_eq!(ids.len(), slots.len());
        for (index, id) in slots {
            assert_eq!(ids.get(index), Some(&id));
        }
        let choices = &descriptor.params.get(CHARACTER).unwrap().choices;
        let decoded: Vec<Character> = (0..4).map(|i| Character::from_choice(i).unwrap()).collect();
        assert_eq!(choices, &["room", "chamber", "hall", "plate"]);
        assert_eq!(
            decoded,
            [
                Character::Room,
                Character::Chamber,
                Character::Hall,
                Character::Plate
            ]
        );
        assert_eq!(Character::from_choice(4), None);
    }

    #[test]
    fn the_key_rounds_like_python() {
        let key = IrKey::new(
            48_000,
            &settings(&[
                ("decay_s", ParamValue::Float(2.0004)),
                ("predelay_ms", ParamValue::Float(15.05)),
                ("width", ParamValue::Float(0.12345)),
            ]),
        );
        assert_eq!((key.decay, key.width), (2.0, 0.123));
        // 15.05 is 15.05000000000000071… in binary, so Python rounds it up.
        assert_eq!(key.predelay_ms, 15.1);
        let same = IrKey::new(48_000, &settings(&[("decay_s", ParamValue::Float(1.9996))]));
        assert_eq!(same.decay, 2.0, "keys within the rounding are equal");
    }

    #[test]
    fn the_response_has_its_length_predelay_and_unit_energy() {
        let key = IrKey::new(
            48_000,
            &settings(&[
                ("character", ParamValue::from("plate")),
                ("decay_s", ParamValue::Float(0.1)),
                ("predelay_ms", ParamValue::Float(10.0)),
            ]),
        );
        let ir = synthesize_ir(SupportedRate::Hz48000, &key);
        // `int((0.1 · 1.5 + 0.01 + 0.05) · 48000)` frames after 480 frames of pre-delay.
        let frames = 480 + 10_080;
        assert_eq!(ir.len(), 2 * frames);
        assert!(ir.iter().take(2 * 480).all(|&v| v == 0.0));
        let energy = |c: usize| ir.iter().skip(c).step_by(2).map(|v| v * v).sum::<f64>();
        let louder = energy(0).max(energy(1));
        assert!((louder - 1.0).abs() < 1e-12, "{louder}");
        assert_eq!(
            synthesize_ir(SupportedRate::Hz48000, &key),
            ir,
            "deterministic"
        );
    }

    #[test]
    fn processing_is_wet_after_the_predelay_and_reset_clears() {
        let mut reverb = Reverb::new();
        reverb.prepare(ProcessSpec {
            sample_rate: 48_000,
            block_size: BLOCK,
            channels: 2,
        });
        reverb.set_params(&slots(&[
            ("mix", ParamValue::Float(1.0)),
            ("predelay_ms", ParamValue::Float(0.0)),
            ("decay_s", ParamValue::Float(0.2)),
        ]));
        assert!(reverb.ir_key.is_some());
        assert!((reverb.tail_seconds() - 0.6).abs() < 1e-15);
        let ctx = BlockContext {
            frame: 0,
            sample_rate: 48_000,
            beat: 0.0,
            bpm: 120.0,
            playing: true,
            sidechain: None,
            events: &[],
        };
        let mut block = [[0.0; 2]; BLOCK];
        block[0] = [1.0, 1.0];
        reverb.process(&mut block, &ctx);
        assert!(
            block.iter().any(|f| f[0] != 0.0),
            "the tail starts in the block"
        );
        reverb.reset();
        let mut silence = [[0.0; 2]; BLOCK];
        reverb.process(&mut silence, &ctx);
        assert_eq!(silence, [[0.0; 2]; BLOCK], "the reset cleared the history");
    }

    fn spec(sample_rate: u32) -> ProcessSpec {
        ProcessSpec {
            sample_rate,
            block_size: BLOCK,
            channels: 2,
        }
    }

    /// `count` blocks of a deterministic signal (a fixed-seed LCG) through `reverb`.
    fn render(reverb: &mut Reverb, count: usize) -> Vec<[f64; 2]> {
        let ctx = BlockContext {
            frame: 0,
            sample_rate: 48_000,
            beat: 0.0,
            bpm: 120.0,
            playing: true,
            sidechain: None,
            events: &[],
        };
        let mut state = 0x2545_f491_u64;
        let mut next = move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            f64::from(u32::try_from(state >> 40).unwrap()) / f64::from(1_u32 << 24) - 0.5
        };
        let mut out = Vec::new();
        for _ in 0..count {
            let mut block = [[0.0; 2]; BLOCK];
            for frame in &mut block {
                *frame = [next(), next()];
            }
            reverb.process(&mut block, &ctx);
            out.extend_from_slice(&block);
        }
        out
    }

    /// The render graph builds a device (prepare, `set_state`, `set_params`) and resets it
    /// before an offline render (a second `prepare`): one synthesis, not two.
    #[test]
    fn the_build_and_the_reset_synthesize_the_response_once() {
        let params = slots(&[("decay_s", ParamValue::Float(0.3))]);
        let mut reverb = Reverb::new();
        reverb.prepare(spec(48_000));
        assert_eq!(reverb.syntheses, 0, "no parameters yet");
        reverb.set_state(&crate::processor::DeviceState::default());
        reverb.set_params(&params);
        assert_eq!(reverb.syntheses, 1);
        render(&mut reverb, 3);
        reverb.prepare(spec(48_000));
        assert_eq!(
            reverb.syntheses, 1,
            "the reset installs the memoized response"
        );
        assert!(reverb.path.partitions() > 0, "a response is installed");
        reverb.set_params(&params);
        assert_eq!(reverb.syntheses, 1, "an unchanged set does nothing");
        reverb.set_params(&slots(&[("decay_s", ParamValue::Float(0.4))]));
        assert_eq!(reverb.syntheses, 2, "a new key synthesizes");
        reverb.prepare(spec(44_100));
        assert_eq!(reverb.syntheses, 3, "another rate is another key");
    }

    /// Deferred setup: `set_params` with a new key synthesizes nothing and keeps the old
    /// response while the mix follows at once, exactly like an offline reverb given the new mix
    /// with the old key; the prepared response installed at a block renders exactly what an
    /// offline `set_params` at that block renders.
    #[test]
    fn a_deferred_install_equals_an_offline_parameter_change() {
        let first = slots(&[
            ("mix", ParamValue::Float(0.5)),
            ("decay_s", ParamValue::Float(0.3)),
        ]);
        let second = slots(&[
            ("mix", ParamValue::Float(0.7)),
            ("decay_s", ParamValue::Float(0.5)),
            ("predelay_ms", ParamValue::Float(3.0)),
        ]);
        // The new mix with the old response: what the live reverb plays until the install.
        let waiting = slots(&[
            ("mix", ParamValue::Float(0.7)),
            ("decay_s", ParamValue::Float(0.3)),
        ]);
        let bits = |frames: &[[f64; 2]]| -> Vec<u64> {
            frames.as_flattened().iter().map(|v| v.to_bits()).collect()
        };
        let mut offline = Reverb::new();
        let mut held = Reverb::new();
        let mut live = Reverb::new();
        for reverb in [&mut offline, &mut held, &mut live] {
            reverb.prepare(spec(48_000));
            reverb.set_params(&first);
        }
        live.defer_setup();
        assert!(
            live.setup_request().is_none(),
            "the set-up key is installed"
        );
        let reference = render(&mut offline, 3);
        assert_eq!(bits(&render(&mut held, 3)), bits(&reference));
        assert_eq!(bits(&render(&mut live, 3)), bits(&reference));

        live.set_params(&second);
        held.set_params(&waiting);
        assert_eq!(live.syntheses, 1, "the deferred change synthesizes nothing");
        assert_eq!(
            bits(&render(&mut live, 2)),
            bits(&render(&mut held, 2)),
            "the old response plays on with the new mix"
        );
        let request = live.setup_request().expect("the new key is requested");
        let mut prepared = request.prepare();
        assert!(prepared.is_pending());

        // The install at a block equals the offline change at that block.
        offline.set_params(&second);
        held.set_params(&second);
        assert!(live.install_setup(&mut prepared));
        assert!(
            !prepared.is_pending(),
            "it now holds the displaced response"
        );
        assert!(live.setup_request().is_none());
        assert!(
            !live.install_setup(&mut prepared),
            "nothing left to install"
        );
        assert_eq!(bits(&render(&mut live, 20)), bits(&render(&mut held, 20)));
        assert_ne!(
            bits(&render(&mut offline, 1)),
            bits(&render(&mut held, 1)),
            "the two offline reverbs have different histories"
        );
    }

    /// The prepared response for an installed key is not installed again.
    #[test]
    fn an_installed_key_is_not_installed_again() {
        let params = slots(&[("decay_s", ParamValue::Float(0.2))]);
        let mut reverb = Reverb::new();
        reverb.prepare(spec(48_000));
        reverb.set_params(&params);
        reverb.defer_setup();
        let key = reverb.ir_key.expect("installed");
        let request = SetupRequest(Request::ReverbIr {
            key,
            block_size: BLOCK,
        });
        let mut prepared = request.prepare();
        assert!(prepared.is_pending());
        assert!(!reverb.install_setup(&mut prepared));
        assert!(prepared.is_pending(), "left unchanged");
    }

    /// A reset that installs the memoized response renders exactly what the code before the
    /// memo rendered, a reset that synthesizes it again: two reverbs with the same history, one
    /// of them without its memo.
    #[test]
    fn a_reset_from_the_memo_equals_a_fresh_synthesis() {
        let params = slots(&[
            ("mix", ParamValue::Float(0.4)),
            ("decay_s", ParamValue::Float(0.3)),
            ("predelay_ms", ParamValue::Float(5.0)),
        ]);
        let history = |reverb: &mut Reverb| {
            reverb.prepare(spec(48_000));
            reverb.set_params(&params);
            render(reverb, 4);
            // A new key and back: a crossfade is pending when the reset comes.
            reverb.set_params(&slots(&[("decay_s", ParamValue::Float(0.2))]));
            reverb.set_params(&params);
        };
        let (mut memoized, mut fresh) = (Reverb::new(), Reverb::new());
        history(&mut memoized);
        history(&mut fresh);
        fresh.synthesized = None;
        memoized.prepare(spec(48_000));
        fresh.prepare(spec(48_000));
        assert_eq!((memoized.syntheses, fresh.syntheses), (3, 4));
        let (a, b) = (render(&mut memoized, 40), render(&mut fresh, 40));
        assert!(a.iter().any(|f| f[0] != 0.0));
        let bits = |frames: &[[f64; 2]]| -> Vec<u64> {
            frames.as_flattened().iter().map(|v| v.to_bits()).collect()
        };
        assert_eq!(bits(&a), bits(&b), "bit-identical output");
    }
}
