//! Golden parity of the built-in plugins (GLD-01,
//! and 6).
//!
//! `plugins/` and `plugins-44100/` are recorded by `scripts/golden/area_plugins.py` and
//! `area_plugins_rates.py`: every built-in effect and instrument in its default configuration and
//! in each preset, driven block by block over the recorded signals (the instruments over their
//! recorded note patterns). The shared runner in `support`
//! replays the recorder exactly and compares every render by its null-test residual, plus the
//! recorded `report()`, `latency` and `tail_seconds` (see its module docs). One test per ported
//! plugin; a newly ported plugin adds one here.
//!
//! Exactness: the filters and kernels are FMA-exact,
//! and the rest is plain arithmetic in the prototype's order. What remains is libm (F1:
//! exact with macOS libm, ulp-level elsewhere) and NumPy's own `tanh` (F2: within one ulp), far
//! below the −120 dBFS threshold. Two steps are inexact by design: the limiter's true-peak FIR
//! (Accelerate's dot product, ulp-level inside, its renders are bit-exact in float32) and the
//! convolvers' FFTs (`realfft` against pocketfft), which matter only for the reverb's
//! float32 wet path (see [`reverb`]). Run with `--nocapture` to see each render's residual.

mod support;

use assert_no_alloc::AllocDisabler;
use sonara_core::plugins::builtin_registry;
use sonara_dsp::processors::{PORTED, create};
use support::{NULL_TEST_DBFS, check_convolver_ir, check_plugin};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

/// The plugins with a test below.
const CHECKED: &[&str] = &[
    "sonara.utility",
    "sonara.dynamics.gate",
    "sonara.dynamics.compressor",
    "sonara.eq.parametric",
    "sonara.filter",
    "sonara.saturation",
    "sonara.delay",
    "sonara.dynamics.limiter",
    "sonara.convolver",
    "sonara.reverb",
    "sonara.synth.poly",
    "sonara.drums",
];

#[test]
fn every_ported_plugin_is_checked() {
    assert_eq!(PORTED, CHECKED);
    let registry = builtin_registry();
    for descriptor in registry.all(None) {
        let ported = CHECKED.contains(&descriptor.id.as_str());
        assert_eq!(
            create(&descriptor.id).is_some(),
            ported,
            "{}",
            descriptor.id
        );
    }
}

#[test]
fn utility() {
    check_plugin("sonara.utility", NULL_TEST_DBFS);
}

#[test]
fn gate() {
    check_plugin("sonara.dynamics.gate", NULL_TEST_DBFS);
}

#[test]
fn compressor() {
    check_plugin("sonara.dynamics.compressor", NULL_TEST_DBFS);
}

#[test]
fn parametric_eq() {
    check_plugin("sonara.eq.parametric", NULL_TEST_DBFS);
}

#[test]
fn multimode_filter() {
    check_plugin("sonara.filter", NULL_TEST_DBFS);
}

#[test]
fn saturation() {
    check_plugin("sonara.saturation", NULL_TEST_DBFS);
}

#[test]
fn delay() {
    check_plugin("sonara.delay", NULL_TEST_DBFS);
}

#[test]
fn limiter() {
    check_plugin("sonara.dynamics.limiter", NULL_TEST_DBFS);
}

/// The default configuration has no impulse response; the configurations of
/// `plugins/convolver-ir.json` load the recorded fixture, and the one that records defect D2 is
/// checked against the correct, normalized result (see `support::check_convolver_ir`).
#[test]
fn convolver() {
    check_plugin("sonara.convolver", NULL_TEST_DBFS);
    check_convolver_ir(NULL_TEST_DBFS);
}

/// The shared threshold holds for the reverb's float32 wet path (GLD-01: no own tolerance). Its
/// residual is the float32 inverse FFT, `realfft` against NumPy's pocketfft; the impulse
/// response synthesis and the NEP 50 float32 mix follow the prototype. Measured in the
/// development container (aarch64 Linux, glibc) on 2026-10-21: at worst −128.9 dBFS (the
/// `clipped` renders of `preset-small-room` and `preset-concert-hall`), −135.0 dBFS at 44.1 kHz,
/// silence exact. `realfft`'s twiddles come from the platform libm (`sin`, `cos`), so the margin on
/// macOS and Windows is not measured here.
#[test]
fn reverb() {
    check_plugin("sonara.reverb", NULL_TEST_DBFS);
}

/// The `single`, `chord` and `legato-line` patterns of every configuration. The
/// patterns use no pedal, controller, bend, noise or stealing; those paths are unit-tested in
/// `processors::synth`.
#[test]
fn poly_synth() {
    check_plugin("sonara.synth.poly", NULL_TEST_DBFS);
}

/// The `kit` pattern of every configuration: eight of the 29 sounds, no choke and no
/// all-notes-off. The kit's first `set_params` synthesizes its samples into the buffers of
/// `prepare`, inside `assert_no_alloc`.
#[test]
fn drum_kit() {
    check_plugin("sonara.drums", NULL_TEST_DBFS);
}
