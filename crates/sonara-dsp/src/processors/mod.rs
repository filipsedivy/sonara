//! Real-time: the built-in processors run on the audio render thread (see
//! docs/audio-engine.md); [`create`] and [`param_slots`] are setup.
//!
//! The prototype's built-in effects (`sonara/dsp/**`) and instruments
//! (`sonara/instruments/**`) behind the [`Processor`] trait. Each one reproduces its Python wrapper, including its clamps, smoothers and
//! block-size-dependent state, on top of the raw kernels and filters of this crate
//! (section 1):
//!
//! | Plugin ID | Prototype |
//! |---|---|
//! | `sonara.utility` | `dsp/spatial/utility.py` |
//! | `sonara.dynamics.gate` | `dsp/dynamics/gate.py` |
//! | `sonara.dynamics.compressor` | `dsp/dynamics/compressor.py` |
//! | `sonara.eq.parametric` | `dsp/filters/eq.py` |
//! | `sonara.filter` | `dsp/filters/multimode.py` |
//! | `sonara.saturation` | `dsp/saturation.py` |
//! | `sonara.delay` | `dsp/time_frequency/delay.py` |
//! | `sonara.dynamics.limiter` | `dsp/dynamics/limiter.py` |
//! | `sonara.convolver` | `ConvolutionReverb` in `dsp/time_frequency/reverb.py` |
//! | `sonara.reverb` | `Reverb` in `dsp/time_frequency/reverb.py` |
//! | `sonara.synth.poly` | `instruments/synth.py` |
//! | `sonara.drums` | `instruments/drums.py` |
//!
//! How a processor is driven (the contract of [`crate::processor`]):
//!
//! 1. [`create`] builds it by plugin ID (setup).
//! 2. [`param_slots`] converts the device's parameter values into a [`ParamSlots`] in
//!    descriptor order (setup).
//! 3. [`Processor::prepare`] sizes its buffers for the block size, resets it and applies the
//!    stored parameters, like the prototype's `prepare` (`reset()`, then
//!    `on_params_changed()`).
//! 4. [`Processor::set_params`] takes the full set every block and recomputes only when it
//!    differs from the stored one, like the prototype's dict comparison. A set that does not
//!    decode (a missing slot, a slot of another type or an unknown choice) is stored and
//!    otherwise ignored: the processor keeps its previous settings.
//! 5. [`Processor::process`] works on at most the prepared block size: frames beyond it are left
//!    untouched, so a sizing mistake cannot panic the callback (the convention of
//!    [`crate::filters`]). Before `prepare` that size is zero.
//!
//! Python's and NumPy's element semantics (`np.maximum`, `np.clip`, `max(initial=…)`, the
//! scalar-versus-array paths of `SmoothedValue.next_block`) are kept where they change a result.
//!
//! A live graph reuses processors across rebuilds and switches them to deferred setup
//! ([`Processor::defer_setup`], [`reusable`]); the module docs of [`crate::processor`] describe
//! it.

mod compressor;
mod convolver;
mod delay;
mod drums;
mod eq;
mod gate;
mod limiter;
mod multimode;
mod params;
mod pyops;
mod reverb;
mod sampler;
mod saturation;
pub(crate) mod setup;
mod synth;
mod utility;

pub use eq::{EqDesign, parametric_eq_design};
pub use params::{ParamSlotsError, param_slots};

use crate::processor::{ParamSlots, ProcessSpec, Processor};

/// The format a processor assumes before its first `prepare` (the prototype's
/// `ProcessSpec(48000, 512)`).
const DEFAULT_SPEC: ProcessSpec = ProcessSpec {
    sample_rate: 48_000,
    block_size: 512,
    channels: 2,
};

/// The plugin IDs [`create`] knows.
pub const PORTED: &[&str] = &[
    utility::PLUGIN_ID,
    gate::PLUGIN_ID,
    compressor::PLUGIN_ID,
    eq::PLUGIN_ID,
    multimode::PLUGIN_ID,
    saturation::PLUGIN_ID,
    delay::PLUGIN_ID,
    limiter::PLUGIN_ID,
    convolver::PLUGIN_ID,
    reverb::PLUGIN_ID,
    synth::PLUGIN_ID,
    drums::PLUGIN_ID,
];

/// A new, unprepared processor for the plugin `plugin_id`, or `None` when no processor exists
/// for it. Besides the ported built-ins ([`PORTED`]) this builds `sonara.sampler`, Sonara's own
/// sample player, which has no prototype and is not in the golden catalog.
///
/// Setup: allocates, so it runs before the stream starts, off the real-time thread. Call
/// [`Processor::prepare`] before the first block.
///
/// # Examples
///
/// ```
/// use sonara_dsp::processors::create;
///
/// assert!(create("sonara.utility").is_some());
/// assert!(create("sonara.unknown").is_none());
/// ```
#[must_use]
#[expect(
    clippy::disallowed_methods,
    reason = "setup: runs before the stream starts, off the real-time thread"
)]
pub fn create(plugin_id: &str) -> Option<Box<dyn Processor>> {
    let processor: Box<dyn Processor> = match plugin_id {
        utility::PLUGIN_ID => Box::new(utility::Utility::new()),
        gate::PLUGIN_ID => Box::new(gate::Gate::new()),
        compressor::PLUGIN_ID => Box::new(compressor::Compressor::new()),
        eq::PLUGIN_ID => Box::new(eq::ParametricEq::new()),
        multimode::PLUGIN_ID => Box::new(multimode::MultimodeFilter::new()),
        saturation::PLUGIN_ID => Box::new(saturation::Saturation::new()),
        delay::PLUGIN_ID => Box::new(delay::Delay::new()),
        limiter::PLUGIN_ID => Box::new(limiter::Limiter::new()),
        convolver::PLUGIN_ID => Box::new(convolver::Convolver::new()),
        reverb::PLUGIN_ID => Box::new(reverb::Reverb::new()),
        synth::PLUGIN_ID => Box::new(synth::PolySynth::new()),
        drums::PLUGIN_ID => Box::new(drums::DrumKit::new()),
        sampler::PLUGIN_ID => Box::new(sampler::Sampler::new()),
        _ => return None,
    };
    Some(processor)
}

/// Whether a processor of `plugin_id`, set up with the parameters `old` and switched to
/// deferred setup ([`Processor::defer_setup`]), takes `new` through `set_params` as the
/// prototype's reused processor does, so a live graph can reuse it.
///
/// False only for a convolver whose "Normalize IR" switch changes: that reloads its impulse
/// response, which is setup work, and the convolver does not defer it (the switch cannot be
/// automated). Every other change is either cheap or deferred (the reverb's impulse response).
///
/// # Examples
///
/// ```
/// use sonara_dsp::processor::ParamSlots;
/// use sonara_dsp::processors::reusable;
///
/// assert!(reusable("sonara.utility", &ParamSlots::EMPTY, &ParamSlots::EMPTY));
/// ```
#[must_use]
pub fn reusable(plugin_id: &str, old: &ParamSlots, new: &ParamSlots) -> bool {
    plugin_id != convolver::PLUGIN_ID || convolver::same_setup(old, new)
}

/// The prototype's `Processor.set_params`: stores `params` and returns `true` when they differ
/// from `stored` (the caller then recomputes, `on_params_changed`); an equal set changes
/// nothing.
fn store_params(stored: &mut ParamSlots, params: &ParamSlots) -> bool {
    if stored == params {
        return false;
    }
    *stored = *params;
    true
}

/// One frame times a gain: NumPy's `x * g` on a `(frames, 2)` block.
fn scale([l, r]: [f64; 2], g: f64) -> [f64; 2] {
    [l * g, r * g]
}

/// The block size as a ramp length for [`crate::filters::SmoothedValue::set_ramp`].
fn ramp_length(samples: usize) -> u32 {
    u32::try_from(samples).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use sonara_core::plugins::builtin_registry;

    use super::{PORTED, create};

    #[test]
    fn creates_every_ported_plugin_and_nothing_else() {
        let registry = builtin_registry();
        for &id in PORTED {
            assert!(registry.contains(id), "{id} is a built-in plugin");
            assert!(create(id).is_some(), "{id}");
        }
        for id in ["sonara.unknown", ""] {
            assert!(create(id).is_none(), "{id} is not a built-in plugin");
        }
        assert_eq!(
            PORTED.len(),
            registry.all(None).len(),
            "every built-in is ported"
        );
    }
}
