//! Real-time: `process`, `reset` and `set_params` with an unchanged impulse-response setting run
//! on the audio render thread (see docs/audio-engine.md); `prepare`, `set_state` and the loading
//! of an impulse response are setup.
//!
//! `sonara.convolver`: convolution with an impulse response from the project's media, as the
//! prototype's `ConvolutionReverb` (`dsp/time_frequency/reverb.py`) computes it on its
//! `PartitionedConvolver` in double precision ([`crate::convolution`]): the input, or its mono
//! sum on both channels, through the convolver, times the wet gain, mixed with the dry signal.
//! Without an impulse response the block passes unchanged.
//!
//! **Loading** (`_load_ir`, setup): the device state's response (an [`ImpulseResponse`] at the
//! project rate) is cut to 30 s and to two channels, normalized by its largest per-channel
//! energy `sqrt(Σ x²)` when "Normalize IR" is on and the energy is positive, and installed,
//! crossfading over one block when a response was loaded before. The energy sums follow NumPy:
//! sequential per column for two channels, pairwise for one.
//!
//! **Defect D2 is fixed**. The prototype loads the
//! response in `set_state` and normalizes it only when its parameters are already set; the
//! render graph calls `set_state` before the first `set_params`, so "Normalize IR" never takes
//! effect there, and the response is not reloaded when the parameters arrive. Here the response
//! is loaded once both the response and the parameters are known, and again whenever
//! "Normalize IR" changes, so it is normalized whenever the switch is on, in either call order.
//!
//! **When loading runs.** It allocates (cutting, normalizing, partitioning and transforming the
//! response), so it is setup. `set_state` loads a new response (`Arc` identity stands for the
//! prototype's `id(ir)`, so the same response twice does not reload). `set_params` loads only in
//! two cases, both part of the device setup that the render graph does off the real-time thread
//! (prepare, `set_state`, `set_params`): the first parameters after a response arrived, and a
//! change of "Normalize IR", which the descriptor marks not automatable, so the parameter sets
//! the render thread sends every block never change it. Every other `set_params` allocates
//! nothing.
//!
//! **Deferred setup** (a live graph, [`Processor::defer_setup`]): `set_params` never loads.
//! A live graph reuses a convolver only while its response and its "Normalize IR" stay the
//! same ([`same_setup`], `processors::reusable`); otherwise a new convolver, loaded on the
//! builder thread, replaces it, so the deferred convolver never has a load to wait for.

use std::sync::Arc;

use super::params::{choice, flag, float};
use super::pyops::maximum;
use super::{DEFAULT_SPEC, ramp_length, store_params};
use crate::convolution::{
    InputMode, LayoutMismatch, OutputPrecision, PartitionedConvolver, PreparedIr, Retired,
    SpectrumFloat,
};
use crate::filters::{Smoothed, SmoothedValue};
use crate::numpy::math::sum;
use crate::processor::{
    BlockContext, DeviceState, ImpulseResponse, ParamSlots, ProcessSpec, Processor,
};

/// The plugin this processor implements.
pub(crate) const PLUGIN_ID: &str = "sonara.convolver";

// Slot indices, in the descriptor's parameter order.
const MIX: usize = 0;
const GAIN_DB: usize = 1;
const INPUT: usize = 2;
const NORMALIZE: usize = 3;

/// The longest impulse response, in seconds (`MAX_IR_SECONDS`).
pub(super) const MAX_IR_SECONDS: f64 = 30.0;

/// The decoded parameter set.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Settings {
    mix: f64,
    gain_db: f64,
    /// `input == "stereo"`; the other choice, `mono_sum`, feeds `(l + r) / 2` to both channels.
    stereo_input: bool,
    normalize: bool,
}

impl Settings {
    fn from_slots(slots: &ParamSlots) -> Option<Self> {
        Some(Self {
            mix: float(slots, MIX)?,
            gain_db: float(slots, GAIN_DB)?,
            // The `input` choices are `mono_sum` and `stereo`.
            stereo_input: match choice(slots, INPUT)? {
                0 => false,
                1 => true,
                _ => return None,
            },
            normalize: flag(slots, NORMALIZE)?,
        })
    }
}

/// The partitioned convolver of a convolution effect with its interleaved stereo block buffers,
/// shared by the convolver and the reverb.
///
/// It also tracks whether a crossfaded swap of the impulse response still waits for its block,
/// so that [`reset`](Self::reset) can reproduce the prototype, whose `reset` builds a new
/// convolver with the latest response and nothing pending.
pub(super) struct ConvolutionPath<T: SpectrumFloat> {
    /// What feeds the convolver's channels: the reverb's mono sum is shared.
    input_mode: InputMode,
    convolver: Option<PartitionedConvolver<T>>,
    /// The block the convolver reads.
    input: Vec<[f64; 2]>,
    /// The block it writes.
    output: Vec<[f64; 2]>,
    /// A swap was installed with a crossfade and no block has played it yet.
    swap_pending: bool,
}

impl<T: SpectrumFloat> ConvolutionPath<T> {
    /// Setup: no convolver until [`prepare`](Self::prepare), which builds one with
    /// `input_mode`.
    pub(super) fn new(input_mode: InputMode) -> Self {
        Self {
            input_mode,
            convolver: None,
            input: Vec::new(),
            output: Vec::new(),
            swap_pending: false,
        }
    }

    /// Setup: a new stereo convolver for blocks of `block_size` frames, without a response
    /// (the prototype's `PartitionedConvolver(block_size, 2)`). A zero block size leaves none.
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    pub(super) fn prepare(&mut self, block_size: usize) {
        self.convolver = PartitionedConvolver::with_input(block_size, 2, self.input_mode).ok();
        self.input = vec![[0.0; 2]; block_size];
        self.output = vec![[0.0; 2]; block_size];
        self.swap_pending = false;
    }

    /// Whether a convolver exists (the prototype's `_conv is not None`).
    pub(super) fn is_prepared(&self) -> bool {
        self.convolver.is_some()
    }

    /// Frames per block, 0 before `prepare`.
    pub(super) fn block_size(&self) -> usize {
        self.convolver
            .as_ref()
            .map_or(0, PartitionedConvolver::block_size)
    }

    /// Partitions of the current response, 0 without one.
    pub(super) fn partitions(&self) -> usize {
        self.convolver
            .as_ref()
            .map_or(0, PartitionedConvolver::partitions)
    }

    /// Setup: installs the interleaved `(taps, channels)` response `ir`, crossfading over the
    /// next block when `crossfade` and a response is current (`set_ir`). Returns whether it was
    /// installed: not before `prepare`, nor for a response the convolver cannot use.
    pub(super) fn set_ir(&mut self, ir: &[f64], channels: usize, crossfade: bool) -> bool {
        let Some(convolver) = self.convolver.as_mut() else {
            return false;
        };
        let had_response = convolver.partitions() > 0;
        if convolver.set_ir(ir, channels, crossfade).is_err() {
            return false;
        }
        self.swap_pending = crossfade && had_response;
        true
    }

    /// Moves the prepared response `ir` in, crossfading over the next block when `crossfade`
    /// and a response is current: [`set_ir`](Self::set_ir) with the preparation done elsewhere.
    ///
    /// Real-time: allocates and frees nothing; the displaced responses come back to be dropped
    /// off the real-time thread.
    ///
    /// # Errors
    ///
    /// [`LayoutMismatch`], carrying `ir` back, before `prepare` or for a response prepared for
    /// another block size or channel count; nothing changes.
    pub(super) fn install(
        &mut self,
        ir: PreparedIr<T>,
        crossfade: bool,
    ) -> Result<Retired<T>, LayoutMismatch<T>> {
        let Some(convolver) = self.convolver.as_mut() else {
            return Err(LayoutMismatch(ir));
        };
        let had_response = convolver.partitions() > 0;
        let retired = convolver.install(ir, crossfade)?;
        self.swap_pending = crossfade && had_response;
        Ok(retired)
    }

    /// The input block to fill.
    pub(super) fn input_mut(&mut self) -> &mut [[f64; 2]] {
        &mut self.input
    }

    /// The last output block.
    pub(super) fn output(&self) -> &[[f64; 2]] {
        &self.output
    }

    /// Convolves the input block into the output block; the dtype the prototype's output had,
    /// or `None` without a convolver.
    pub(super) fn process(&mut self) -> Option<OutputPrecision> {
        let convolver = self.convolver.as_mut()?;
        // The buffers are sized with the convolver in `prepare`, so the block always fits.
        let precision = convolver
            .process(self.input.as_flattened(), self.output.as_flattened_mut())
            .ok();
        self.swap_pending = false;
        precision
    }

    /// The prototype's `reset`, which builds a new convolver holding the latest response:
    /// silent history, an empty delay line, nothing pending. Without allocating: a pending swap
    /// first plays into one silent block, which makes its response current, and then the
    /// history and the delay line are cleared. The displaced response stays inside the
    /// convolver until the next swap, which is setup, frees it (RT-09).
    pub(super) fn reset(&mut self) {
        let Some(convolver) = self.convolver.as_mut() else {
            return;
        };
        if self.swap_pending {
            self.input.fill([0.0; 2]);
            // ERR-06: the block is sized for the convolver; its output is discarded.
            let _ = convolver.process(self.input.as_flattened(), self.output.as_flattened_mut());
            self.swap_pending = false;
        }
        convolver.reset();
    }
}

/// `float(np.sqrt(np.sum(arr**2, axis=0)).max())` of an interleaved `(frames, channels)`
/// response with one or two channels: a contiguous `(n, 1)` column is summed pairwise and the
/// columns of `(n, 2)` sequentially (verified with NumPy 2.5.4). Setup: the mono case
/// allocates the squares.
fn energy(samples: &[f64], channels: usize) -> f64 {
    if channels == 1 {
        let squares: Vec<f64> = samples.iter().map(|v| v * v).collect();
        return sum(&squares).sqrt();
    }
    let mut columns = [0.0; 2];
    for frame in samples.chunks_exact(channels) {
        for (acc, &v) in columns.iter_mut().zip(frame) {
            *acc += v * v;
        }
    }
    let [left, right] = columns;
    maximum(left.sqrt(), right.sqrt())
}

/// Whether a convolver set up with `old` keeps its loaded response under `new`: "Normalize IR"
/// is the same (the only parameter that reloads); two sets without the switch count as the
/// same.
pub(super) fn same_setup(old: &ParamSlots, new: &ParamSlots) -> bool {
    flag(old, NORMALIZE) == flag(new, NORMALIZE)
}

/// The convolution reverb (the prototype's `ConvolutionReverb`).
pub(crate) struct Convolver {
    spec: ProcessSpec,
    params: ParamSlots,
    settings: Option<Settings>,
    path: ConvolutionPath<f64>,
    /// The device state's response (`state['ir']`).
    ir: Option<Arc<ImpulseResponse>>,
    /// The response in the convolver and whether it was normalized (the prototype's `_ir_id`,
    /// plus the switch D2 needs).
    loaded: Option<(Arc<ImpulseResponse>, bool)>,
    mix: SmoothedValue,
    gain: f64,
    /// Seconds of the loaded response (`_tail`).
    tail: f64,
    mix_ramp: Vec<f64>,
    /// Deferred setup ([`Processor::defer_setup`]): `set_params` never loads.
    deferred: bool,
}

impl Convolver {
    /// Setup: an unprepared processor.
    pub(crate) fn new() -> Self {
        Self {
            spec: DEFAULT_SPEC,
            params: ParamSlots::EMPTY,
            settings: None,
            path: ConvolutionPath::new(InputMode::PerChannel),
            ir: None,
            loaded: None,
            mix: SmoothedValue::new(),
            gain: 1.0,
            tail: 0.0,
            mix_ramp: Vec::new(),
            deferred: false,
        }
    }

    /// The prototype's `on_params_changed`, then, when `load` allows it, the load of a response
    /// that waited for the parameters or whose normalization changed (see the module docs).
    fn apply_params(&mut self, load: bool) {
        let Some(p) = Settings::from_slots(&self.params) else {
            return;
        };
        self.mix.set_ramp(ramp_length(self.spec.block_size));
        self.mix.set_target(p.mix);
        self.gain = 10.0_f64.powf(p.gain_db / 20.0);
        self.settings = Some(p);
        if load && self.needs_load() {
            self.load_ir();
        }
    }

    /// Whether the convolver lacks the device state's response with the current
    /// normalization. False until a convolver, a response and parameters all exist.
    fn needs_load(&self) -> bool {
        let (Some(ir), Some(settings)) = (&self.ir, self.settings) else {
            return false;
        };
        if !self.path.is_prepared() {
            return false;
        }
        match &self.loaded {
            Some((loaded, normalized)) => {
                !Arc::ptr_eq(loaded, ir) || *normalized != settings.normalize
            }
            None => true,
        }
    }

    /// `_load_ir` with the D2 fix: the device state's response, cut to 30 s and two channels,
    /// normalized when the switch is on, installed with a crossfade when one was loaded before.
    /// A response the convolver cannot use (no channels, or a partial frame) is ignored.
    #[expect(
        clippy::disallowed_methods,
        reason = "setup: loads an impulse response at device setup, off the real-time thread"
    )]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "Python's int() of 30 · sr, and a frame count far below 2^53"
    )]
    fn load_ir(&mut self) {
        let (Some(ir), Some(settings)) = (self.ir.clone(), self.settings) else {
            return;
        };
        if ir.channels == 0 {
            return;
        }
        let sr = f64::from(self.spec.sample_rate);
        // `arr[: int(MAX_IR_SECONDS * sr)]`, then `arr[:, :2]`.
        let max_frames = (MAX_IR_SECONDS * sr) as usize;
        let frames = (ir.samples.len() / ir.channels).min(max_frames);
        let channels = ir.channels.min(2);
        let mut arr = Vec::with_capacity(frames.saturating_mul(channels));
        for frame in ir.samples.chunks_exact(ir.channels).take(frames) {
            arr.extend(frame.iter().take(channels));
        }
        let energy = energy(&arr, channels);
        if settings.normalize && energy > 0.0 {
            for v in &mut arr {
                *v /= energy;
            }
        }
        if self.path.set_ir(&arr, channels, self.loaded.is_some()) {
            self.loaded = Some((ir, settings.normalize));
            self.tail = frames as f64 / sr;
        }
    }
}

impl Processor for Convolver {
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    fn prepare(&mut self, spec: ProcessSpec) {
        self.spec = spec;
        // `reset`: a new convolver, nothing loaded (`_ir_id = None`); the response loads with
        // the parameters below.
        self.path.prepare(spec.block_size);
        self.loaded = None;
        self.mix_ramp = vec![0.0; spec.block_size];
        self.apply_params(true);
    }

    fn set_params(&mut self, params: &ParamSlots) {
        if store_params(&mut self.params, params) {
            self.apply_params(!self.deferred);
        }
    }

    /// Setup: adopts the device state's response and loads it when the parameters are known.
    /// A state without a response keeps the loaded one, as in the prototype.
    fn set_state(&mut self, state: &DeviceState) {
        self.ir.clone_from(&state.impulse_response);
        if self.needs_load() {
            self.load_ir();
        }
    }

    fn process(&mut self, buffer: &mut [[f64; 2]], _ctx: &BlockContext<'_>) {
        // No response: the block passes (`partitions == 0`). The convolver takes exactly one
        // block, as the prototype asserts; another length passes too.
        if self.path.partitions() == 0 || buffer.len() != self.path.block_size() {
            return;
        }
        let stereo = self.settings.is_some_and(|s| s.stereo_input);
        for (x, &[l, r]) in self.path.input_mut().iter_mut().zip(buffer.iter()) {
            *x = if stereo {
                [l, r]
            } else {
                // `np.repeat(buffer.mean(axis=1, keepdims=True), 2, axis=1)`.
                #[expect(
                    clippy::manual_midpoint,
                    reason = "RT-11: NumPy's two-channel mean, written as it computes it"
                )]
                let mono = (l + r) / 2.0;
                [mono, mono]
            };
        }
        if self.path.process().is_none() {
            return;
        }
        let gain = self.gain;
        let Some(mix) = self.mix_ramp.get_mut(..buffer.len()) else {
            return;
        };
        let wet = self.path.output();
        // `buffer * (1 − m) + (conv(x) * gain) * m`.
        match self.mix.next_block(mix) {
            Smoothed::Steady(m) => {
                let dry = 1.0 - m;
                for (frame, w) in buffer.iter_mut().zip(wet) {
                    for (y, &v) in frame.iter_mut().zip(w) {
                        *y = *y * dry + (v * gain) * m;
                    }
                }
            }
            Smoothed::Ramp => {
                for ((frame, w), &m) in buffer.iter_mut().zip(wet).zip(mix.iter()) {
                    let dry = 1.0 - m;
                    for (y, &v) in frame.iter_mut().zip(w) {
                        *y = *y * dry + (v * gain) * m;
                    }
                }
            }
        }
    }

    /// Clears the convolution history and keeps the loaded response, which is what the
    /// prototype's `reset` (a new convolver that reloads the response) leaves behind.
    fn reset(&mut self) {
        self.path.reset();
    }

    /// The loaded response's length in seconds; 0 before the first one.
    fn tail_seconds(&self) -> f64 {
        self.tail
    }

    fn defer_setup(&mut self) {
        self.deferred = true;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sonara_core::model::device::{ParamValue, Params};
    use sonara_core::plugins::builtin_registry;

    use super::{Convolver, GAIN_DB, INPUT, MIX, NORMALIZE, PLUGIN_ID, energy};
    use crate::processor::{
        BlockContext, DeviceState, ImpulseResponse, ParamSlots, ProcessSpec, Processor,
    };
    use crate::processors::param_slots;

    const BLOCK: usize = 16;

    fn slots(values: &[(&str, ParamValue)]) -> ParamSlots {
        let registry = builtin_registry();
        let values: Params = values
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        param_slots(registry.get(PLUGIN_ID).unwrap(), &values).unwrap()
    }

    fn spec() -> ProcessSpec {
        ProcessSpec {
            sample_rate: 48_000,
            block_size: BLOCK,
            channels: 2,
        }
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

    /// A stereo response whose first frame is `[a, b]`, then silence: energies `|a|` and `|b|`.
    fn state(a: f64, b: f64) -> DeviceState {
        let samples = [a, b].into_iter().chain([0.0; 14]).collect();
        DeviceState {
            impulse_response: Some(Arc::new(ImpulseResponse {
                channels: 2,
                samples,
            })),
            ..DeviceState::default()
        }
    }

    /// The first output frame for an impulse input, fully wet.
    fn first_frame(convolver: &mut Convolver) -> [f64; 2] {
        let mut block = [[0.0; 2]; BLOCK];
        block[0] = [1.0, 1.0];
        convolver.process(&mut block, &ctx());
        block[0]
    }

    fn assert_close(actual: [f64; 2], expected: [f64; 2]) {
        for (a, e) in actual.into_iter().zip(expected) {
            assert!((a - e).abs() < 1e-12, "{actual:?} != {expected:?}");
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
            (MIX, "mix"),
            (GAIN_DB, "gain_db"),
            (INPUT, "input"),
            (NORMALIZE, "normalize"),
        ];
        assert_eq!(ids.len(), slots.len());
        for (index, id) in slots {
            assert_eq!(ids.get(index), Some(&id));
        }
        // The reload in `set_params` is setup only because automation never changes the switch.
        let normalize = registry.get(PLUGIN_ID).unwrap().params.get(NORMALIZE);
        assert!(!normalize.unwrap().automatable);
    }

    /// "Normalize IR" applies in the render graph's order (state first) as in the
    /// recorder's (parameters first).
    #[test]
    fn the_response_is_normalized_in_either_call_order() {
        let wet = [("mix", ParamValue::Float(1.0))];
        for state_first in [true, false] {
            let mut convolver = Convolver::new();
            convolver.prepare(spec());
            if state_first {
                convolver.set_state(&state(0.5, 0.25));
                convolver.set_params(&slots(&wet));
            } else {
                convolver.set_params(&slots(&wet));
                convolver.set_state(&state(0.5, 0.25));
            }
            // A mono-summed impulse of 1 through [0.5, 0.25] / 0.5.
            assert_close(first_frame(&mut convolver), [1.0, 0.5]);
            assert_eq!(convolver.tail_seconds(), 8.0 / 48_000.0);
        }
    }

    #[test]
    fn a_normalize_change_reloads_and_the_same_response_does_not() {
        let mut convolver = Convolver::new();
        convolver.prepare(spec());
        let state = state(0.5, 0.25);
        convolver.set_state(&state);
        convolver.set_params(&slots(&[
            ("mix", ParamValue::Float(1.0)),
            ("normalize", ParamValue::Bool(false)),
        ]));
        assert_close(first_frame(&mut convolver), [0.5, 0.25]);
        let loaded = convolver.loaded.clone();
        convolver.set_state(&state);
        assert!(
            Arc::ptr_eq(&loaded.unwrap().0, &convolver.loaded.clone().unwrap().0),
            "the same response is not reloaded"
        );
        convolver.reset();
        convolver.set_params(&slots(&[("mix", ParamValue::Float(1.0))]));
        assert_eq!(convolver.loaded.as_ref().map(|l| l.1), Some(true));
        // The switch crossfades: the first frame still has the raw response, so play it out.
        first_frame(&mut convolver);
        convolver.reset();
        assert_close(first_frame(&mut convolver), [1.0, 0.5]);
    }

    #[test]
    fn a_reset_settles_a_pending_swap_like_a_new_convolver() {
        let mut convolver = Convolver::new();
        convolver.prepare(spec());
        convolver.set_params(&slots(&[
            ("mix", ParamValue::Float(1.0)),
            ("normalize", ParamValue::Bool(false)),
        ]));
        convolver.set_state(&state(0.5, 0.25));
        first_frame(&mut convolver);
        // A new response crossfades in the next block; a reset in between makes it current.
        convolver.set_state(&state(0.125, 0.75));
        convolver.reset();
        assert_close(first_frame(&mut convolver), [0.125, 0.75]);
    }

    #[test]
    fn without_a_response_the_block_passes_and_energy_follows_numpy() {
        let mut convolver = Convolver::new();
        convolver.prepare(spec());
        convolver.set_params(&slots(&[]));
        let input = [[0.25, -0.5]; BLOCK];
        let mut block = input;
        convolver.process(&mut block, &ctx());
        assert_eq!(block, input);
        assert_eq!(convolver.tail_seconds(), 0.0);
        assert_eq!(energy(&[3.0, 4.0, 0.0, 0.0], 2), 4.0);
        assert_eq!(energy(&[3.0, 4.0], 1), 5.0);
        assert_eq!(energy(&[], 2), 0.0);
    }
}
