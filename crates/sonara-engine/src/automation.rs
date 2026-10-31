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
//! - **A sampler's enum parameters** also change at their own frames inside a block
//!   ([`choice_changes`], computed at build): a point takes effect at the frame a note on the
//!   point's beat starts on (`beat_to_sample`, as the note schedule places notes), and a
//!   linear or smooth segment at the first frame whose beat gives the next choice. The
//!   schedule sends each change as a choice event at its frame. Before the first point's
//!   frame the first value holds, from the last point's frame on the last one, as above.
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
        self.segment_value(index, beat).unwrap_or(last_value)
    }

    /// The value at `beat` of the segment from point `index` to the next: `t` is the position
    /// in the segment, clipped to `[0, 1]` (1 for a segment shorter than `EPSILON` beats), and
    /// the curve of point `index` shapes it. `None` for the last point, which starts no
    /// segment. The one formula of both [`Lane::value_at`] and the frames of
    /// [`choice_changes`], so the two agree on every beat.
    fn segment_value(&self, index: usize, beat: f64) -> Option<f64> {
        let next = index.checked_add(1)?;
        let (Some(&x0), Some(&x1), Some(&y0), Some(&y1), Some(&curve)) = (
            self.beats.get(index),
            self.beats.get(next),
            self.values.get(index),
            self.values.get(next),
            self.curves.get(index),
        ) else {
            return None;
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
        Some(y0 + (y1 - y0) * shaped)
    }

    /// The value at timeline `frame`, whose beat is `beat`, as an enum lane's choices take
    /// effect ([`choice_changes`]): `frames` holds each point's frame, in point order. The
    /// segment is that of the last point whose frame is at or before `frame`, evaluated at
    /// `beat`; before the first point's frame the first value holds, from the last point's
    /// frame on the last value.
    fn value_at_frame(&self, frames: &[i64], frame: i64, beat: f64) -> f64 {
        let (Some(&first_value), Some(&last_value)) = (self.values.first(), self.values.last())
        else {
            return 0.0;
        };
        match frames.partition_point(|&at| at <= frame).checked_sub(1) {
            None => first_value,
            Some(index) => self.segment_value(index, beat).unwrap_or(last_value),
        }
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
            SlotKind::Choice { last } => ParamSlot::Choice(choice_of(v, last)),
        };
        params.set(self.slot, slot);
    }

    /// The frames at which an enum lane's choice changes, with the choice from each on, in
    /// frame order and each frame once; empty for a lane of another kind (see
    /// [`choice_changes`]).
    ///
    /// Setup: allocates, so it runs off the real-time thread, when the graph is built.
    #[expect(
        clippy::disallowed_methods,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    fn choice_changes(&self, tempo: &TempoMap, sample_rate: u32) -> Vec<(i64, u16)> {
        let SlotKind::Choice { last } = self.kind else {
            return Vec::new();
        };
        let lane = &self.lane;
        let frames: Vec<i64> = lane
            .beats
            .iter()
            .map(|&beat| tempo.beat_to_sample(beat, sample_rate))
            .collect();
        let choice_at = |frame: i64| {
            let beat = frame_beat(tempo, frame, sample_rate);
            choice_of(lane.value_at_frame(&frames, frame, beat), last)
        };
        let mut changes = Vec::new();
        let mut current = choice_of(lane.values.first().copied().unwrap_or(0.0), last);
        for (index, &start) in frames.iter().enumerate() {
            // The frames of point `index`: up to the next point's frame, none when a later
            // point shares its frame (that point's value is the one that plays there).
            let end = frames.get(index.saturating_add(1)).copied();
            if end == Some(start) {
                continue;
            }
            let mut frame = start;
            let choice = choice_at(frame);
            if choice != current {
                changes.push((frame, choice));
                current = choice;
            }
            // Within a linear or smooth segment the choice moves on monotonically (a hold
            // segment keeps it): each next change is the first frame whose choice differs,
            // found by bisection, so a long ramp costs a few evaluations per change.
            let Some(last_frame) = end.map(|end| end.saturating_sub(1)) else {
                continue;
            };
            while frame < last_frame && choice_at(last_frame) != current {
                // Invariant: the choice at `hi` differs from `current`, the one at `lo - 1`
                // does not.
                let (mut lo, mut hi) = (frame.saturating_add(1), last_frame);
                while lo < hi {
                    let mid = lo.saturating_add(hi.saturating_sub(lo) / 2);
                    if choice_at(mid) == current {
                        lo = mid.saturating_add(1);
                    } else {
                        hi = mid;
                    }
                }
                frame = lo;
                current = choice_at(frame);
                changes.push((frame, current));
            }
        }
        changes
    }
}

/// An automated change of an enum parameter inside the timeline: from `frame` on, the
/// parameter at index `slot` in descriptor order takes the choice with index `choice`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChoiceChange {
    /// The timeline frame the change takes effect at.
    pub(crate) frame: i64,
    /// The parameter's index in descriptor order.
    pub(crate) slot: u8,
    /// The index of the choice.
    pub(crate) choice: u16,
}

/// The changes of the enum parameters that `lanes`, a device's lanes in lane order, automate,
/// frame by frame and in frame order, for a sampler, which takes them at their frames inside a
/// block (the module docs). For each parameter only its last lane counts, as in the block's
/// parameters, where the last lane is applied last.
///
/// The choice in force at a frame, the last change at or before it or the first point's
/// choice before every change, is the one the block's parameters give at the frame's beat,
/// except on a point's own frame when the point's beat lies up to half a frame after the
/// frame's (`beat_to_sample` rounds to the nearest frame): there the change is a little
/// earlier, so a note that starts on the point's beat plays it.
///
/// Setup: allocates, so it runs off the real-time thread, when the graph is built.
pub(crate) fn choice_changes(
    lanes: &[DeviceLane],
    tempo: &TempoMap,
    sample_rate: u32,
) -> Vec<ChoiceChange> {
    let mut changes: Vec<ChoiceChange> = lanes
        .iter()
        .enumerate()
        .filter(|&(position, lane)| {
            let later = lanes.get(position.saturating_add(1)..).unwrap_or_default();
            !later.iter().any(|other| other.slot == lane.slot)
        })
        .filter_map(|(_, lane)| Some((u8::try_from(lane.slot).ok()?, lane)))
        .flat_map(|(slot, lane)| {
            lane.choice_changes(tempo, sample_rate)
                .into_iter()
                .map(move |(frame, choice)| ChoiceChange {
                    frame,
                    slot,
                    choice,
                })
        })
        .collect();
    // Stable: the changes of one frame keep their lanes' order.
    changes.sort_by_key(|change| change.frame);
    changes
}

/// The enum choice `round(clamp(v, 0, last))` with Python's rounding and clamp (Python's
/// `min(max(v, 0), last)`: each keeps its first argument unless the second is strictly
/// better).
fn choice_of(v: f64, last: f64) -> u16 {
    let low = if 0.0 > v { 0.0 } else { v };
    let clamped = if last < low { last } else { low };
    u16::try_from(python_round(clamped)).unwrap_or(0)
}

/// The beat at timeline `frame` (`TempoMap.sample_to_beat`, the scalar conversion): the beat
/// of a block start ([`TempoCurve::sample_to_beat`]) and of the frames of
/// [`choice_changes`].
fn frame_beat(tempo: &TempoMap, frame: i64, sample_rate: u32) -> f64 {
    tempo.sample_to_beat(frame_to_f64(frame), sample_rate)
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
        frame_beat(&self.map, frame, sample_rate)
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
    use sonara_core::plugins::{
        PluginDescriptor, SamplerChoices, builtin_registry, sampler_descriptor,
    };
    use sonara_core::time::{TempoMap, TempoPoint};
    use sonara_dsp::processor::{MAX_PARAMS, ParamSlot, ParamSlots};

    use super::{ChoiceChange, DeviceLane, Lane, TempoCurve, choice_changes, frame_beat};

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

    /// The sampler's descriptor with `articulations` articulation IDs, so its `articulation`
    /// has `articulations + 2` choices (`auto`, `ordinario`, then the IDs), and two instrument
    /// types (`auto`, `violin`).
    fn sampler(articulations: usize) -> PluginDescriptor {
        sampler_descriptor(&SamplerChoices {
            presets: ["lib/patch".to_owned()].to_vec(),
            instrument_types: ["violin".to_owned()].to_vec(),
            articulations: (0..articulations)
                .map(|i| "a".to_owned() + &i.to_string())
                .collect(),
            ..SamplerChoices::default()
        })
        .unwrap()
    }

    /// The index of `param` in `descriptor`'s order.
    fn slot_of(descriptor: &PluginDescriptor, param: &str) -> usize {
        descriptor
            .params
            .iter()
            .position(|p| p.id == param)
            .unwrap()
    }

    /// A lane of `descriptor`'s parameter `param` with `points`.
    fn device_lane(
        descriptor: &PluginDescriptor,
        param: &str,
        points: &[(f64, f64, CurveKind)],
    ) -> DeviceLane {
        DeviceLane::new(
            lane(points),
            slot_of(descriptor, param),
            descriptor.param(param).unwrap(),
        )
    }

    /// What the block's parameters give the lane's parameter at `beat` ([`DeviceLane::apply`]):
    /// the oracle of the choice in force, apart from a point's own frame.
    fn block_choice(lane: &DeviceLane, descriptor: &PluginDescriptor, beat: f64) -> u16 {
        let filler = [ParamSlot::Choice(0); MAX_PARAMS];
        let mut params = ParamSlots::new(filler.get(..descriptor.params.len()).unwrap()).unwrap();
        lane.apply(beat, &mut params);
        params
            .get(lane.slot)
            .and_then(|slot| match slot {
                ParamSlot::Choice(choice) => Some(choice),
                _ => None,
            })
            .expect("an enum slot")
    }

    /// A point takes effect at the frame a note on its beat starts on. At 128 bpm and 44.1 kHz
    /// a beat is 20 671.875 frames, so beat 5 lies at frame 103 359.375: the note and the
    /// change land on frame 103 359, whose own beat is still before 5, so the block's
    /// parameters see the point only from the next frame. Points that keep the choice change
    /// nothing, and of two points on one beat the later one plays.
    #[test]
    fn changes_land_where_notes_on_their_beats_start() {
        let descriptor = sampler(4);
        let tempo = TempoMap::constant(128.0).unwrap();
        let rate = 44_100;
        let articulation = device_lane(
            &descriptor,
            "articulation",
            &[
                (0.0, 2.0, CurveKind::Hold),
                (1.0, 2.0, CurveKind::Hold),
                (2.0, 3.0, CurveKind::Hold),
                (5.0, 1.0, CurveKind::Hold),
                (5.0, 4.0, CurveKind::Hold),
                (6.0, 4.4, CurveKind::Hold),
            ],
        );
        // Where the note schedule puts a note on each beat.
        assert_eq!(tempo.beat_to_sample(2.0, rate), 41_344);
        assert_eq!(tempo.beat_to_sample(5.0, rate), 103_359);
        assert_eq!(
            articulation.choice_changes(&tempo, rate),
            [(41_344, 3), (103_359, 4)]
        );
        let beat = |frame| frame_beat(&tempo, frame, rate);
        assert!(beat(103_359) < 5.0);
        assert_eq!(block_choice(&articulation, &descriptor, beat(103_359)), 3);
        assert_eq!(block_choice(&articulation, &descriptor, beat(103_360)), 4);
    }

    /// TST-10: the choice in force at every frame, from the change list, against the block's
    /// parameters at that frame's beat, over a rising linear ramp across all 98 choices, a
    /// falling smooth ramp, a jump, a flat ramp, two points on one beat and a falling linear
    /// ramp, at a steady tempo and through a tempo ramp. Only a point's own frame may differ,
    /// where the point's beat lies after the frame's (the change comes up to half a frame
    /// early, as a note on that beat does).
    #[test]
    fn ramps_change_at_the_first_frame_of_each_choice() {
        let descriptor = sampler(96);
        let rate = 8_000;
        let points = [
            (0.5, 0.0, CurveKind::Linear),
            (2.5, 97.0, CurveKind::Smooth),
            (4.0, 10.0, CurveKind::Hold),
            (4.25, 50.0, CurveKind::Linear),
            (5.0, 50.0, CurveKind::Linear),
            (5.0, 60.3, CurveKind::Linear),
            (6.0, 3.0, CurveKind::Hold),
        ];
        let articulation = device_lane(&descriptor, "articulation", &points);
        let tempos = [
            TempoMap::constant(120.0).unwrap(),
            TempoMap::new(
                [
                    TempoPoint::new(0.0, 100.0, true).unwrap(),
                    TempoPoint::new(4.0, 160.0, false).unwrap(),
                ]
                .to_vec(),
            )
            .unwrap(),
        ];
        for tempo in &tempos {
            let changes = articulation.choice_changes(tempo, rate);
            let point_frames: Vec<i64> = points
                .iter()
                .map(|&(beat, _, _)| tempo.beat_to_sample(beat, rate))
                .collect();
            // Every change is a change, on a frame of its own, in frame order.
            let mut previous = (i64::MIN, 0);
            for &(frame, choice) in &changes {
                assert!(frame > previous.0 && choice != previous.1, "{changes:?}");
                previous = (frame, choice);
            }
            // The rising ramp passes each choice once, in order.
            let ramp_end = tempo.beat_to_sample(2.5, rate);
            let rising: Vec<u16> = changes
                .iter()
                .filter(|&&(frame, _)| frame < ramp_end)
                .map(|&(_, choice)| choice)
                .collect();
            assert_eq!(rising, (1..=97).collect::<Vec<u16>>());
            let end = tempo.beat_to_sample(6.5, rate);
            let mut in_force = 0;
            let mut next = changes.iter().peekable();
            let mut early = 0;
            for frame in -10..=end {
                while let Some(&&(at, choice)) = next.peek()
                    && at <= frame
                {
                    in_force = choice;
                    next.next();
                }
                let beat = frame_beat(tempo, frame, rate);
                let expected = block_choice(&articulation, &descriptor, beat);
                let ahead = points
                    .iter()
                    .zip(&point_frames)
                    .any(|(&(point, _, _), &at)| at == frame && beat < point);
                if ahead && in_force != expected {
                    early += 1;
                    continue;
                }
                assert_eq!(in_force, expected, "frame {frame}, beat {beat}");
            }
            assert!(early <= points.len(), "{early} early frames");
        }
    }

    #[test]
    fn only_the_last_lane_of_an_enum_parameter_changes_its_choice() {
        // At 120 bpm and 48 kHz a beat is 24 000 frames. The first `articulation` lane is
        // replaced by the second, as in the block's parameters; the volume is no enum; the
        // `instrument_type` lane changes its own parameter, in frame order with the other.
        let descriptor = sampler(4);
        let tempo = TempoMap::constant(120.0).unwrap();
        let rate = 48_000;
        let lanes = [
            device_lane(
                &descriptor,
                "articulation",
                &[(0.0, 2.0, CurveKind::Hold), (1.0, 3.0, CurveKind::Hold)],
            ),
            device_lane(
                &descriptor,
                "instrument_type",
                &[(0.0, 0.0, CurveKind::Hold), (1.0, 1.0, CurveKind::Hold)],
            ),
            device_lane(
                &descriptor,
                "volume",
                &[(0.0, -6.0, CurveKind::Linear), (1.0, 0.0, CurveKind::Hold)],
            ),
            device_lane(
                &descriptor,
                "articulation",
                &[(0.0, 2.0, CurveKind::Hold), (2.0, 4.0, CurveKind::Hold)],
            ),
        ];
        let slot = |param| u8::try_from(slot_of(&descriptor, param)).unwrap();
        assert_eq!(
            choice_changes(&lanes, &tempo, rate),
            [
                ChoiceChange {
                    frame: 24_000,
                    slot: slot("instrument_type"),
                    choice: 1
                },
                ChoiceChange {
                    frame: 48_000,
                    slot: slot("articulation"),
                    choice: 4
                },
            ]
        );
        // A lane whose points all round to the choice it starts with changes nothing.
        let steady = device_lane(
            &descriptor,
            "articulation",
            &[
                (0.0, 2.2, CurveKind::Linear),
                (1.0, 1.8, CurveKind::Smooth),
                (3.0, 2.4, CurveKind::Hold),
            ],
        );
        assert_eq!(choice_changes(&[steady], &tempo, rate), []);
    }
}
