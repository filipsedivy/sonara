//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! `sonara.dynamics.gate`: a noise gate with hysteresis and hold, or a downward expander, with
//! an optional external key, as the prototype's `dsp/dynamics/gate.py` computes it.

use super::params::{choice, flag, float};
use super::pyops::{max_initial, maximum, minimum};
use super::{DEFAULT_SPEC, scale, store_params};
use crate::filters::time_coefficient;
use crate::kernels::{GateState, ar_smooth, gate_hold};
use crate::processor::{BlockContext, ParamSlots, ProcessSpec, Processor, Report};

/// The plugin this processor implements.
pub(crate) const PLUGIN_ID: &str = "sonara.dynamics.gate";

// Slot indices, in the descriptor's parameter order.
const MODE: usize = 0;
const THRESHOLD_DB: usize = 1;
const RANGE_DB: usize = 2;
const RATIO: usize = 3;
const HYSTERESIS_DB: usize = 4;
const ATTACK_MS: usize = 5;
const HOLD_MS: usize = 6;
const RELEASE_MS: usize = 7;
const SIDECHAIN: usize = 8;

/// The detector's floor before the logarithm, as an amplitude (−200 dBFS).
const LEVEL_FLOOR: f64 = 1e-10;

/// The `mode` choices, in descriptor order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// Gate: open or closed, with hysteresis and hold.
    Gate,
    /// Downward expander below the threshold.
    Expander,
}

impl Mode {
    fn from_choice(index: u16) -> Option<Self> {
        match index {
            0 => Some(Self::Gate),
            1 => Some(Self::Expander),
            _ => None,
        }
    }
}

/// The decoded parameter set, with the prototype's derived values.
struct Settings {
    mode: Mode,
    threshold: f64,
    hysteresis: f64,
    range: f64,
    ratio: f64,
    hold: i64,
    attack: f64,
    release: f64,
    use_sidechain: bool,
}

impl Settings {
    fn from_slots(slots: &ParamSlots, sample_rate: u32) -> Option<Self> {
        let sr = f64::from(sample_rate);
        Some(Self {
            mode: Mode::from_choice(choice(slots, MODE)?)?,
            threshold: float(slots, THRESHOLD_DB)?,
            hysteresis: float(slots, HYSTERESIS_DB)?,
            range: float(slots, RANGE_DB)?,
            ratio: float(slots, RATIO)?,
            hold: hold_samples(float(slots, HOLD_MS)?, sr),
            attack: time_coefficient(float(slots, ATTACK_MS)?, sr),
            release: time_coefficient(float(slots, RELEASE_MS)?, sr),
            use_sidechain: flag(slots, SIDECHAIN)?,
        })
    }
}

/// `round(hold_ms * 0.001 * sr)`: Python's `round`, ties to even, as an integer.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a hold of at most 500 ms is far inside i64; `as` saturates and maps NaN to 0"
)]
fn hold_samples(hold_ms: f64, sample_rate: f64) -> i64 {
    (hold_ms * 0.001 * sample_rate).round_ties_even() as i64
}

/// The gate (the prototype's `Gate`).
pub(crate) struct Gate {
    spec: ProcessSpec,
    params: ParamSlots,
    mode: Mode,
    threshold: f64,
    hysteresis: f64,
    range: f64,
    ratio: f64,
    hold: i64,
    attack: f64,
    release: f64,
    use_sidechain: bool,
    /// The open/hold state machine of `gate_hold`.
    machine: GateState,
    /// The gain-reduction smoother's state (`_env`).
    env: f64,
    /// The largest gain reduction of the last block, in dB (`_max_gr`).
    max_gr: f64,
    /// Per-sample key level in dBFS, one block long.
    level_db: Vec<f64>,
    /// Per-sample target reduction (and first the gate's open flags), one block long.
    target: Vec<f64>,
    /// Per-sample smoothed reduction, one block long.
    gr: Vec<f64>,
}

impl Gate {
    /// Setup: an unprepared processor.
    pub(crate) fn new() -> Self {
        Self {
            spec: DEFAULT_SPEC,
            params: ParamSlots::EMPTY,
            mode: Mode::Gate,
            threshold: 0.0,
            hysteresis: 0.0,
            // No reduction until parameters arrive.
            range: 0.0,
            ratio: 1.0,
            hold: 0,
            attack: 0.0,
            release: 0.0,
            use_sidechain: false,
            machine: GateState::default(),
            env: 0.0,
            max_gr: 0.0,
            level_db: Vec::new(),
            target: Vec::new(),
            gr: Vec::new(),
        }
    }

    /// The prototype's `on_params_changed`.
    fn apply_params(&mut self) {
        let Some(p) = Settings::from_slots(&self.params, self.spec.sample_rate) else {
            return;
        };
        self.mode = p.mode;
        self.threshold = p.threshold;
        self.hysteresis = p.hysteresis;
        self.range = p.range;
        self.ratio = p.ratio;
        self.hold = p.hold;
        self.attack = p.attack;
        self.release = p.release;
        self.use_sidechain = p.use_sidechain;
    }
}

impl Processor for Gate {
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    fn prepare(&mut self, spec: ProcessSpec) {
        self.spec = spec;
        self.level_db = vec![0.0; spec.block_size];
        self.target = vec![0.0; spec.block_size];
        self.gr = vec![0.0; spec.block_size];
        self.reset();
        self.apply_params();
    }

    fn set_params(&mut self, params: &ParamSlots) {
        if store_params(&mut self.params, params) {
            self.apply_params();
        }
    }

    fn process(&mut self, buffer: &mut [[f64; 2]], ctx: &BlockContext<'_>) {
        let n = buffer.len().min(self.level_db.len());
        let (Some(buffer), Some(level_db), Some(target), Some(gr)) = (
            buffer.get_mut(..n),
            self.level_db.get_mut(..n),
            self.target.get_mut(..n),
            self.gr.get_mut(..n),
        ) else {
            return;
        };
        let key: &[[f64; 2]] = match ctx.sidechain {
            Some(sidechain) if self.use_sidechain => sidechain,
            _ => buffer,
        };
        key_level_db(key, level_db);
        match self.mode {
            Mode::Gate => {
                let close = self.threshold - self.hysteresis;
                gate_hold(
                    level_db,
                    target,
                    self.threshold,
                    close,
                    self.hold,
                    &mut self.machine,
                );
                // 0 dB when open, −range when closed (the range is negative).
                for t in target.iter_mut() {
                    *t = (1.0 - *t) * (-self.range);
                }
            }
            Mode::Expander => {
                for (t, &level) in target.iter_mut().zip(level_db.iter()) {
                    let below = maximum(self.threshold - level, 0.0);
                    *t = minimum(below * (self.ratio - 1.0), -self.range);
                }
            }
        }
        // A rising reduction is the gate closing (release), a falling one opening (attack): the
        // smoother's coefficients are swapped on purpose.
        ar_smooth(target, gr, self.release, self.attack, &mut self.env);
        self.max_gr = max_initial(gr, 0.0);
        for (frame, &g) in buffer.iter_mut().zip(gr.iter()) {
            *frame = scale(*frame, 10.0_f64.powf(-g / 20.0));
        }
    }

    fn reset(&mut self) {
        self.machine = GateState::default();
        self.env = 0.0;
    }

    fn report(&self) -> Report {
        Report::new(&[("gain_reduction_db", self.max_gr)])
    }
}

/// `20·log10(np.maximum(np.max(np.abs(key), axis=1), 1e-10))` into `level_db`. Frames the key
/// does not have are at the floor.
fn key_level_db(key: &[[f64; 2]], level_db: &mut [f64]) {
    let floor = 20.0 * LEVEL_FLOOR.log10();
    let mut frames = key.iter();
    for level in level_db.iter_mut() {
        *level = frames.next().map_or(floor, |&[l, r]| {
            20.0 * maximum(maximum(l.abs(), r.abs()), LEVEL_FLOOR).log10()
        });
    }
}

#[cfg(test)]
mod tests {
    use sonara_core::model::device::{ParamValue, Params};
    use sonara_core::plugins::builtin_registry;

    use super::{
        ATTACK_MS, Gate, HOLD_MS, HYSTERESIS_DB, MODE, PLUGIN_ID, RANGE_DB, RATIO, RELEASE_MS,
        SIDECHAIN, THRESHOLD_DB, hold_samples,
    };
    use crate::processor::{BlockContext, ProcessSpec, Processor};
    use crate::processors::param_slots;

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

    fn prepared(values: &[(&str, ParamValue)]) -> Gate {
        let registry = builtin_registry();
        let values: Params = values
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        let mut gate = Gate::new();
        gate.prepare(ProcessSpec {
            sample_rate: 48_000,
            block_size: 64,
            channels: 2,
        });
        gate.set_params(&param_slots(registry.get(PLUGIN_ID).unwrap(), &values).unwrap());
        gate
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
            (THRESHOLD_DB, "threshold_db"),
            (RANGE_DB, "range_db"),
            (RATIO, "ratio"),
            (HYSTERESIS_DB, "hysteresis_db"),
            (ATTACK_MS, "attack_ms"),
            (HOLD_MS, "hold_ms"),
            (RELEASE_MS, "release_ms"),
            (SIDECHAIN, "sidechain"),
        ];
        assert_eq!(ids.len(), slots.len());
        for (index, id) in slots {
            assert_eq!(ids.get(index), Some(&id));
        }
    }

    #[test]
    fn hold_rounds_half_to_even() {
        // 0.5 ms at 1 kHz is half a sample; 1.5 ms is one and a half.
        assert_eq!(hold_samples(0.5, 1000.0), 0);
        assert_eq!(hold_samples(1.5, 1000.0), 2);
        assert_eq!(hold_samples(20.0, 48_000.0), 960);
    }

    #[test]
    fn a_closed_gate_attenuates_and_an_open_one_passes() {
        // Instant attack and release make the reduction follow the gate exactly.
        let fast = [
            ("attack_ms", ParamValue::Float(0.05)),
            ("release_ms", ParamValue::Float(5.0)),
        ];
        let mut gate = prepared(&fast);
        let mut quiet = [[1e-4, -1e-4]; 64];
        gate.process(&mut quiet, &ctx(None));
        assert!(
            quiet.iter().all(|f| f[0].abs() < 1e-4),
            "closed: attenuated"
        );
        assert!(gate.max_gr > 0.0);
        let mut gate = prepared(&fast);
        let mut loud = [[0.5, -0.5]; 64];
        gate.process(&mut loud, &ctx(None));
        assert_eq!(loud, [[0.5, -0.5]; 64], "open from the first sample");
        assert_eq!(
            gate.report().entries(),
            [("gain_reduction_db", 0.0)].as_slice()
        );
    }

    #[test]
    fn the_sidechain_keys_the_gate_only_when_enabled() {
        let loud_key = [[0.5, 0.5]; 64];
        for (enabled, opens) in [(true, true), (false, false)] {
            let mut gate = prepared(&[("sidechain", ParamValue::Bool(enabled))]);
            let mut quiet = [[1e-4, 1e-4]; 64];
            gate.process(&mut quiet, &ctx(Some(&loud_key)));
            assert_eq!(quiet == [[1e-4, 1e-4]; 64], opens, "sidechain {enabled}");
        }
    }
}
