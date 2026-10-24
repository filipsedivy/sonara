//! Real-time: automation and tempo lookups run on the audio render thread (see
//! docs/audio-engine.md); building a [`Lane`], a [`DeviceLane`] or a [`TempoCurve`] is setup.
//!
//! The prototype's automation evaluation (`AutomationLane.values_at` in
//! `sonara.domain.automation`) and the tempo conversions of the render graph
//! (item 3):
//!
//! - **Lanes:** the point index is NumPy's `searchsorted(side="right") − 1`, clamped to the
//!   points; a segment shorter than `EPSILON` beats counts as already reached (`t = 1`); `t` is
//!   clipped to `[0, 1]`; hold gives `0`, smooth `0.5 − 0.5 cos(πt)`, linear `t`; before the
//!   first point the first value holds and from the last point on the last value.
//! - **Device parameters** are sampled once per block at the source frame's beat
//!   ([`DeviceLane::apply`]): a switch is `v >= 0.5`, an enum the choice
//!   `round(clamp(v, 0, choices − 1))` and an integer `round(v)`, both with Python's rounding;
//!   a float passes unclamped.
//! - **Mixer automation** is evaluated per sample through the prototype's vectorized
//!   `TempoMap.seconds_to_beats` ([`TempoCurve::frame_beats`]). With several tempo points it
//!   computes exactly what the scalar `seconds_to_beat` does; with one point it is
//!   `s * (bpm / 60)`, where the scalar form is `s * bpm / 60`.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

use std::f64::consts::PI;

use sonara_core::EPSILON;
use sonara_core::model::automation::{AutomationLane, CurveKind};
use sonara_core::plugins::{ParamKind, ParamSpec};
use sonara_core::time::{TempoMap, python_round};
use sonara_dsp::processor::{ParamSlot, ParamSlots};

/// An automation lane with at least one point, as the arrays `values_at` works on.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Lane {
    /// Point positions in beats, ascending (two points may share a beat).
    beats: Vec<f64>,
    /// Point values.
    values: Vec<f64>,
    /// The curve of the segment that starts at each point.
    curves: Vec<CurveKind>,
}

impl Lane {
    /// The points of `lane`, or `None` for a lane without points (the prototype skips those).
    /// Setup.
    pub(crate) fn new(lane: &AutomationLane) -> Option<Self> {
        if lane.points.is_empty() {
            return None;
        }
        Some(Self {
            beats: lane.points.iter().map(|p| p.beat).collect(),
            values: lane.points.iter().map(|p| p.value).collect(),
            curves: lane.points.iter().map(|p| p.curve).collect(),
        })
    }

    /// The value at `beat` (`values_at` for one element).
    pub(crate) fn value_at(&self, beat: f64) -> f64 {
        let (Some(&first_beat), Some(&last_beat), Some(&first_value), Some(&last_value)) = (
            self.beats.first(),
            self.beats.last(),
            self.values.first(),
            self.values.last(),
        ) else {
            return 0.0;
        };
        if self.beats.len() == 1 {
            return first_value;
        }
        // `where(beats >= xs[-1], ys[-1], where(beats < xs[0], ys[0], out))`.
        if beat >= last_beat {
            return last_value;
        }
        if beat < first_beat {
            return first_value;
        }
        let last = self.beats.len().saturating_sub(1);
        let index = self
            .beats
            .partition_point(|&x| x <= beat)
            .saturating_sub(1)
            .min(last);
        let next = index.saturating_add(1).min(last);
        let (Some(&x0), Some(&x1), Some(&y0), Some(&y1), Some(&curve)) = (
            self.beats.get(index),
            self.beats.get(next),
            self.values.get(index),
            self.values.get(next),
            self.curves.get(index),
        ) else {
            return last_value;
        };
        let span = x1 - x0;
        let t = if span > EPSILON {
            (beat - x0) / span
        } else {
            1.0
        };
        let t = t.clamp(0.0, 1.0);
        let shaped = match curve {
            CurveKind::Hold => 0.0,
            CurveKind::Smooth => 0.5 - 0.5 * (PI * t).cos(),
            CurveKind::Linear => t,
        };
        y0 + (y1 - y0) * shaped
    }

    /// The values at `beats`, written into `out` (`values_at`).
    pub(crate) fn values_at(&self, beats: &[f64], out: &mut [f64]) {
        for (y, &beat) in out.iter_mut().zip(beats) {
            *y = self.value_at(beat);
        }
    }
}

/// How an automated value becomes a parameter slot (`DeviceSlot.params_at`).
#[derive(Debug, Clone, Copy, PartialEq)]
enum SlotKind {
    Float,
    Int,
    Bool,
    /// An enum with choices `0..=last`.
    Choice {
        last: f64,
    },
}

/// A lane that automates one device parameter.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DeviceLane {
    lane: Lane,
    /// The parameter's position in descriptor order (its slot in [`ParamSlots`]).
    slot: usize,
    kind: SlotKind,
}

impl DeviceLane {
    /// The lane for parameter `spec` at `slot` in descriptor order. Setup.
    pub(crate) fn new(lane: Lane, slot: usize, spec: &ParamSpec) -> Self {
        let kind = match spec.kind {
            ParamKind::Float => SlotKind::Float,
            ParamKind::Int => SlotKind::Int,
            ParamKind::Bool => SlotKind::Bool,
            ParamKind::Enum => {
                // `param_slots` accepts at most 65 536 choices, so the count fits a `u16`.
                let last = spec.choices.len().saturating_sub(1);
                SlotKind::Choice {
                    last: f64::from(u16::try_from(last).unwrap_or(u16::MAX)),
                }
            }
        };
        Self { lane, slot, kind }
    }

    /// Writes the lane's value at `beat` into `params`.
    pub(crate) fn apply(&self, beat: f64, params: &mut ParamSlots) {
        let v = self.lane.value_at(beat);
        let slot = match self.kind {
            SlotKind::Bool => ParamSlot::Bool(v >= 0.5),
            SlotKind::Int => ParamSlot::Int(python_round(v)),
            SlotKind::Float => ParamSlot::Float(v),
            SlotKind::Choice { last } => {
                // Python's `min(max(v, 0), last)`: each keeps its first argument unless the
                // second is strictly better.
                let low = if 0.0 > v { 0.0 } else { v };
                let clamped = if last < low { last } else { low };
                ParamSlot::Choice(u16::try_from(python_round(clamped)).unwrap_or(0))
            }
        };
        params.set(self.slot, slot);
    }
}

/// The project's tempo map with the conversions the processing path needs.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TempoCurve {
    map: TempoMap,
    /// `bpm / 60` of a single-point map: the vectorized conversion's own formula.
    constant_beats_per_second: Option<f64>,
}

impl TempoCurve {
    /// The curve of `map`. Setup: copies the map.
    pub(crate) fn new(map: &TempoMap) -> Self {
        let constant_beats_per_second = match map.points() {
            [only] => Some(only.bpm / 60.0),
            _ => None,
        };
        Self {
            map: map.clone(),
            constant_beats_per_second,
        }
    }

    /// The beat at timeline `frame` (`TempoMap.sample_to_beat`, the scalar conversion).
    pub(crate) fn sample_to_beat(&self, frame: i64, sample_rate: u32) -> f64 {
        self.map.sample_to_beat(frame_to_f64(frame), sample_rate)
    }

    /// The tempo at `beat`.
    pub(crate) fn bpm_at(&self, beat: f64) -> f64 {
        self.map.bpm_at(beat)
    }

    /// The beat at `seconds` by the prototype's vectorized `TempoMap.seconds_to_beats`.
    pub(crate) fn seconds_to_beats(&self, seconds: f64) -> f64 {
        match self.constant_beats_per_second {
            Some(beats_per_second) => seconds * beats_per_second,
            None => self.map.seconds_to_beat(seconds),
        }
    }

    /// The beat of every frame from `first_frame` on, written into `out`:
    /// `seconds_to_beats((first_frame + arange(n)) / sample_rate)`.
    pub(crate) fn frame_beats(&self, first_frame: i64, sample_rate: u32, out: &mut [f64]) {
        let rate = f64::from(sample_rate);
        let mut frame = first_frame;
        for beat in out.iter_mut() {
            *beat = self.seconds_to_beats(frame_to_f64(frame) / rate);
            frame = frame.wrapping_add(1);
        }
    }
}

/// A timeline frame as NumPy converts an `int64` for true division.
#[expect(
    clippy::cast_precision_loss,
    reason = "timeline frames stay far below 2^53, where the conversion is exact"
)]
fn frame_to_f64(frame: i64) -> f64 {
    frame as f64
}

#[cfg(test)]
mod tests {
    use sonara_core::model::Valid;
    use sonara_core::model::automation::{
        AutomationLaneData, AutomationPoint, AutomationTarget, CurveKind,
    };
    use sonara_core::plugins::builtin_registry;
    use sonara_core::time::{TempoMap, TempoPoint};
    use sonara_dsp::processor::{MAX_PARAMS, ParamSlot, ParamSlots};

    use super::{DeviceLane, Lane, TempoCurve};

    fn lane(points: &[(f64, f64, CurveKind)]) -> Lane {
        let data = AutomationLaneData {
            id: "aut_1".to_owned(),
            target: AutomationTarget::new(None, "pan").unwrap(),
            points: points
                .iter()
                .map(|&(b, v, c)| AutomationPoint::new(b, v, c).unwrap())
                .collect(),
            enabled: true,
        };
        Lane::new(&Valid::new(data).unwrap()).unwrap()
    }

    /// Values from the prototype's `AutomationLane.values_at` (host virtualenv, 2026-10-21).
    #[test]
    fn lanes_follow_values_at() {
        let l = lane(&[
            (1.0, 0.0, CurveKind::Linear),
            (3.0, 1.0, CurveKind::Smooth),
            (5.0, -1.0, CurveKind::Hold),
            (5.0, 2.0, CurveKind::Linear),
            (6.0, 4.0, CurveKind::Linear),
        ]);
        let beats = [0.0, 1.0, 2.0, 3.5, 4.999, 5.0, 5.5, 6.0, 9.0];
        let mut out = [0.0; 9];
        l.values_at(&beats, &mut out);
        let expected: [f64; 9] = [
            0.0,
            0.0,
            0.5,
            // 1 + (−2)(0.5 − 0.5 cos(π/4)), which rounds to 1/√2.
            std::f64::consts::FRAC_1_SQRT_2,
            -0.999_998_766_299_703_4,
            2.0,
            3.0,
            4.0,
            4.0,
        ];
        for (got, want) in out.iter().zip(expected) {
            assert_eq!(got.to_bits(), want.to_bits(), "{got} vs {want}");
        }
        let single = lane(&[(2.0, 0.25, CurveKind::Hold)]);
        assert_eq!(single.value_at(0.0), 0.25);
        assert_eq!(single.value_at(10.0), 0.25);
    }

    #[test]
    fn device_lanes_convert_like_params_at() {
        let registry = builtin_registry();
        let gate = registry.get("sonara.dynamics.gate").unwrap();
        let mode = gate.param("mode").unwrap();
        let mode_slot = gate.params.iter().position(|p| p.id == "mode").unwrap();
        let choices = mode.choices.len();
        let filler = [ParamSlot::Bool(false); MAX_PARAMS];
        let mut params = ParamSlots::new(filler.get(..gate.params.len()).unwrap()).unwrap();
        // Below zero clamps to the first choice, beyond the last to the last, and 0.5 rounds
        // half to even.
        for (value, choice) in [
            (-3.0, 0),
            (0.5, 0),
            (1.5, 2.min(choices - 1)),
            (99.0, choices - 1),
        ] {
            let l = DeviceLane::new(lane(&[(0.0, value, CurveKind::Linear)]), mode_slot, mode);
            l.apply(0.0, &mut params);
            assert_eq!(
                params.get(mode_slot),
                Some(ParamSlot::Choice(u16::try_from(choice).unwrap())),
                "{value}"
            );
        }
    }

    #[test]
    fn a_single_tempo_uses_the_vectorized_formula() {
        let map = TempoMap::constant(124.0).unwrap();
        let curve = TempoCurve::new(&map);
        let seconds = 1234.0 / 48_000.0;
        assert_eq!(curve.seconds_to_beats(seconds), seconds * (124.0 / 60.0));
        let ramps = TempoMap::new(
            [
                TempoPoint::new(0.0, 72.0, false).unwrap(),
                TempoPoint::new(8.0, 72.0, true).unwrap(),
                TempoPoint::new(12.0, 56.0, false).unwrap(),
            ]
            .to_vec(),
        )
        .unwrap();
        let curve = TempoCurve::new(&ramps);
        let mut before = [0.0; 1];
        curve.frame_beats(-2, 48_000, &mut before);
        assert_eq!(before, [-2.0 / 48_000.0 * 72.0 / 60.0]);
        let mut beats = [0.0; 3];
        curve.frame_beats(400_000, 48_000, &mut beats);
        for (k, beat) in beats.iter().enumerate() {
            let s = f64::from(400_000 + u32::try_from(k).unwrap()) / 48_000.0;
            assert_eq!(beat.to_bits(), ramps.seconds_to_beat(s).to_bits());
        }
    }
}
