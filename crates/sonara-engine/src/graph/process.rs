//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! The processing path of the render graph (`RenderGraph.process` and `_fader` in
//! `audio/engine/graph.py`). It works only on
//! memory the graph preallocated: node indices instead of track IDs, the per-node signal
//! buffers, and one block of scratch. The clip audio of the block was read before by
//! [`RenderGraph::read_media`].
//!
//! The arithmetic keeps the prototype's order and its scalar-versus-array paths:
//!
//! - **Fader:** `gain = vol · mute`; the balance law `θ = (pan + 1)π/4`,
//!   `L = min(1, √2 cos θ)`, `R = min(1, √2 sin θ)`. With a steady gain and pan the block is
//!   `buf · [g·L, g·R]`, or unchanged when all three are 1; otherwise `(buf · g) · L` per sample.
//!   A volume lane is `10^(v/20)` per sample and sets the smoother to its last value; a pan
//!   lane feeds the balance law directly.
//! - **Sends:** the smoother advances every block; a lane replaces its value with
//!   `10^(v/20) · (target > 0)`; a pre-fader send multiplies by the mute value after this
//!   block's fader step. The contribution is delayed by the edge's compensation, then added.
//! - **Automation clock:** device parameters at the source frame's beat once per block; the
//!   mixer at `frame − output_latency` per sample. A sampler's automated enum parameters (its
//!   articulation) also change at their own frames inside the block: its schedule sends each
//!   change as a choice event among the instrument's events.
//!
//! A live graph also merges each instrument's live input into its events and meters every node
//! ([`super::live`]); an offline graph has neither, so its path is unchanged.
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

use std::f64::consts::{PI, SQRT_2};

use sonara_dsp::filters::Smoothed;
use sonara_dsp::processor::{BlockContext, ParamSlots};

use super::live::NodeMeter;
use super::{DeviceSlot, Node, RenderGraph, Scratch};
use crate::automation::TempoCurve;

/// A steady value or one per sample in a scratch slice (`SmoothedValue.next_block`'s float
/// versus array).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Value {
    Scalar(f64),
    Array,
}

impl From<Smoothed> for Value {
    fn from(smoothed: Smoothed) -> Self {
        match smoothed {
            Smoothed::Steady(v) => Self::Scalar(v),
            Smoothed::Ramp => Self::Array,
        }
    }
}

/// Whether a block plays the clip audio read for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipAudio {
    /// The clip audio read for the frame, or silence counted as missing media.
    Media,
    /// No clip audio, deliberately: nothing is counted (the idle step).
    Omit,
}

/// What every node of one block shares.
#[derive(Debug, Clone, Copy)]
struct Block<'a> {
    frame: i64,
    sample_rate: u32,
    beat: f64,
    bpm: f64,
    playing: bool,
    chase: bool,
    /// Whether the clip buffers hold this block's audio.
    media_ready: bool,
    tempo: &'a TempoCurve,
}

/// The signal buffers around the node being processed.
struct Buffers<'a> {
    /// The nodes before it: their pre-fader signals (sidechain sources).
    before: &'a [Vec<[f64; 2]>],
    /// Its own input, processed in place into its pre-fader signal.
    current: &'a mut [[f64; 2]],
    /// The nodes after it: the targets of its output and sends.
    after: &'a mut [Vec<[f64; 2]>],
    /// Its index.
    index: usize,
}

impl Buffers<'_> {
    /// The input buffer of node `target`, which processing order puts after this one.
    fn target(&mut self, target: Option<usize>) -> Option<&mut Vec<[f64; 2]>> {
        let offset = target?.checked_sub(self.index.checked_add(1)?)?;
        self.after.get_mut(offset)
    }
}

impl Node {
    /// The latency of the node's output: its input's plus its inserts' and instrument's.
    pub(super) fn output_latency(&self) -> usize {
        self.input_latency.saturating_add(self.insert_latency)
    }

    /// Whether the mixer evaluates automation per sample.
    fn has_mixer_lanes(&self) -> bool {
        self.volume_lane.is_some()
            || self.pan_lane.is_some()
            || self.sends.iter().any(|s| s.lane.is_some())
    }
}

impl RenderGraph {
    /// The delay of the master output behind the sources, in frames: the master's output
    /// latency.
    #[must_use]
    pub fn latency(&self) -> usize {
        self.latency
    }

    /// Renders one block whose sources start at timeline `frame` and returns the master output,
    /// which lags the sources by [`RenderGraph::latency`] frames.
    ///
    /// A block that does not continue the previous one (the first, after a seek or a loop)
    /// chases: instruments get an all-notes-off and the notes sounding at `frame`. With
    /// `playing` false, no scheduled events are sent; a live graph still delivers its live
    /// input, as the prototype's idle step does.
    ///
    /// Real-time: allocates, locks and blocks on nothing. Call [`RenderGraph::read_media`] for
    /// the same `frame` first, or the block plays without its clip audio (counted in
    /// [`RenderGraph::blocks_without_media`]).
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn process(&mut self, frame: i64, playing: bool) -> &[[f64; 2]] {
        self.render_block(frame, playing, ClipAudio::Media)
    }

    /// Renders one block of a stopped transport at `frame` for live input (the player's idle
    /// step): no scheduled events and no clip audio, so a stopped playhead inside a clip does
    /// not repeat it. Live input, held voices and effect tails sound as in
    /// [`RenderGraph::process`] with `playing` false. Nothing is counted as missing media.
    ///
    /// Real-time: allocates, locks and blocks on nothing.
    #[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
    pub fn process_idle(&mut self, frame: i64) -> &[[f64; 2]] {
        self.render_block(frame, false, ClipAudio::Omit)
    }

    fn render_block(&mut self, frame: i64, playing: bool, clips: ClipAudio) -> &[[f64; 2]] {
        let Self {
            sample_rate,
            block_size,
            tempo,
            nodes,
            signal,
            scratch,
            output,
            next_frame,
            media_frame,
            blocks_without_media,
            last_playing,
            meters,
            ..
        } = self;
        let block = Block {
            frame,
            sample_rate: *sample_rate,
            beat: tempo.sample_to_beat(frame, *sample_rate),
            bpm: 0.0,
            playing,
            chase: *next_frame != Some(frame),
            media_ready: clips == ClipAudio::Media && *media_frame == Some(frame),
            tempo,
        };
        let block = Block {
            bpm: tempo.bpm_at(block.beat),
            ..block
        };
        *next_frame = Some(frame.saturating_add(i64::try_from(*block_size).unwrap_or(i64::MAX)));
        *last_playing = playing;
        if clips == ClipAudio::Media
            && !block.media_ready
            && nodes.iter().any(|node| node.audio.is_some())
        {
            *blocks_without_media = blocks_without_media.wrapping_add(1);
        }
        for buffer in signal.iter_mut() {
            buffer.fill([0.0; 2]);
        }
        output.fill([0.0; 2]);
        for (index, node) in nodes.iter_mut().enumerate() {
            let Some((before, rest)) = signal.split_at_mut_checked(index) else {
                break;
            };
            let Some((current, after)) = rest.split_first_mut() else {
                break;
            };
            let mut buffers = Buffers {
                before,
                current: current.as_mut_slice(),
                after,
                index,
            };
            process_node(node, &mut buffers, scratch, &block);
            if let Some(meter) = meters.get_mut(index) {
                *meter = NodeMeter::measure(&scratch.post);
            }
            if node.is_master {
                for (y, x) in output.iter_mut().zip(&scratch.post) {
                    *y = *x;
                }
                break;
            }
            send_output(node, &mut buffers, scratch);
        }
        output
    }
}

/// One node up to its fader and sends: sources, instrument, inserts, the finite-value guard,
/// the fader into `scratch.post` and the sends into their targets.
fn process_node(
    node: &mut Node,
    buffers: &mut Buffers<'_>,
    scratch: &mut Scratch,
    block: &Block<'_>,
) {
    if block.media_ready
        && let Some(audio) = &node.audio
    {
        for (y, x) in buffers.current.iter_mut().zip(&audio.buffer) {
            *y = [y[0] + x[0], y[1] + x[1]];
        }
    }
    if let (Some(slot), Some(schedule)) = (&mut node.instrument, &mut node.schedule) {
        let scheduled = if block.playing {
            schedule.events(block.frame, buffers.current.len(), block.chase)
        } else {
            &[]
        };
        let events = match node.live.as_mut() {
            Some(live) if !live.is_idle() => live.merge(scheduled, &mut scratch.events),
            _ => scheduled,
        };
        let ctx = BlockContext {
            frame: block.frame,
            sample_rate: block.sample_rate,
            beat: block.beat,
            bpm: block.bpm,
            playing: block.playing,
            sidechain: None,
            events,
        };
        scratch.instrument.fill([0.0; 2]);
        set_params(slot, block.beat);
        slot.processor.process(&mut scratch.instrument, &ctx);
        for (y, x) in buffers.current.iter_mut().zip(&scratch.instrument) {
            *y = [y[0] + x[0], y[1] + x[1]];
        }
    }
    for slot in &mut node.inserts {
        let sidechain = slot
            .sidechain
            .and_then(|source| buffers.before.get(source))
            .map(Vec::as_slice);
        let ctx = BlockContext {
            frame: block.frame,
            sample_rate: block.sample_rate,
            beat: block.beat,
            bpm: block.bpm,
            playing: block.playing,
            sidechain,
            events: &[],
        };
        set_params(slot, block.beat);
        slot.processor.process(buffers.current, &ctx);
    }
    ensure_finite(buffers.current);
    if node.has_mixer_lanes() {
        let first = block
            .frame
            .saturating_sub(i64::try_from(node.output_latency()).unwrap_or(i64::MAX));
        block
            .tempo
            .frame_beats(first, block.sample_rate, &mut scratch.beats);
    }
    fader(node, buffers.current, scratch);
    sends(node, buffers, scratch);
}

/// `set_params(params_at(beat))`: the device's values with its lanes applied.
fn set_params(slot: &mut DeviceSlot, beat: f64) {
    if slot.lanes.is_empty() {
        slot.processor.set_params(&slot.params);
        return;
    }
    let mut params: ParamSlots = slot.params;
    for lane in &slot.lanes {
        lane.apply(beat, &mut params);
    }
    slot.processor.set_params(&params);
}

/// Replaces NaN and infinities with silence (`ensure_finite`).
fn ensure_finite(buffer: &mut [[f64; 2]]) {
    for sample in buffer.iter_mut().flatten() {
        if !sample.is_finite() {
            *sample = 0.0;
        }
    }
}

/// The balance law: unity at the centre, a constant-power taper of the attenuated side.
fn balance(pan: f64) -> (f64, f64) {
    let theta = (pan + 1.0) * (PI / 4.0);
    (
        (SQRT_2 * theta.cos()).min(1.0),
        (SQRT_2 * theta.sin()).min(1.0),
    )
}

/// Fader, mute and pan of `buf` into `scratch.post` (`_fader`).
#[expect(
    clippy::float_cmp,
    reason = "the prototype passes the block through exactly when gain and pan are unity"
)]
fn fader(node: &mut Node, buf: &[[f64; 2]], scratch: &mut Scratch) {
    let Scratch {
        post,
        beats,
        volume,
        mute,
        pan,
        left,
        right,
        ..
    } = scratch;
    let vol = if let Some(lane) = &node.volume_lane {
        lane.values_at(beats, volume);
        for v in volume.iter_mut() {
            *v = 10.0_f64.powf(*v / 20.0);
        }
        if let Some(&last) = volume.last() {
            node.volume.set_immediate(last);
        }
        Value::Array
    } else {
        node.volume.next_block(volume).into()
    };
    let mute_value: Value = node.mute.next_block(mute).into();
    let pan_value = if let Some(lane) = &node.pan_lane {
        lane.values_at(beats, pan);
        Value::Array
    } else {
        node.pan.next_block(pan).into()
    };
    // `gain = vol * mute`; an array gain lands in `volume`.
    let gain = match (vol, mute_value) {
        (Value::Scalar(v), Value::Scalar(m)) => Value::Scalar(v * m),
        (Value::Scalar(v), Value::Array) => {
            for (g, m) in volume.iter_mut().zip(mute.iter()) {
                *g = v * m;
            }
            Value::Array
        }
        (Value::Array, Value::Scalar(m)) => {
            for g in volume.iter_mut() {
                *g *= m;
            }
            Value::Array
        }
        (Value::Array, Value::Array) => {
            for (g, m) in volume.iter_mut().zip(mute.iter()) {
                *g *= m;
            }
            Value::Array
        }
    };
    if let (Value::Scalar(g), Value::Scalar(p)) = (gain, pan_value) {
        let (l, r) = balance(p);
        if g == 1.0 && l == 1.0 && r == 1.0 {
            for (y, x) in post.iter_mut().zip(buf) {
                *y = *x;
            }
        } else {
            let (gl, gr) = (g * l, g * r);
            for (y, x) in post.iter_mut().zip(buf) {
                *y = [x[0] * gl, x[1] * gr];
            }
        }
        return;
    }
    let steady_pan = match pan_value {
        Value::Scalar(p) => Some(balance(p)),
        Value::Array => {
            for ((l, r), p) in left.iter_mut().zip(right.iter_mut()).zip(pan.iter()) {
                (*l, *r) = balance(*p);
            }
            None
        }
    };
    let steady_gain = match gain {
        Value::Scalar(g) => Some(g),
        Value::Array => None,
    };
    let frames = post
        .iter_mut()
        .zip(buf)
        .zip(volume.iter())
        .zip(left.iter().zip(right.iter()));
    for (((y, x), &g_array), (&l_array, &r_array)) in frames {
        let g = steady_gain.unwrap_or(g_array);
        let (l, r) = steady_pan.unwrap_or((l_array, r_array));
        *y = [(x[0] * g) * l, (x[1] * g) * r];
    }
}

/// The node's sends into their targets.
fn sends(node: &mut Node, buffers: &mut Buffers<'_>, scratch: &mut Scratch) {
    let mute = node.mute.current();
    for send in &mut node.sends {
        let mut gain: Value = send.gain.next_block(&mut scratch.send).into();
        if let Some(lane) = &send.lane {
            lane.values_at(&scratch.beats, &mut scratch.send);
            let on = if send.gain.target() > 0.0 { 1.0 } else { 0.0 };
            for g in &mut scratch.send {
                *g = 10.0_f64.powf(*g / 20.0) * on;
            }
            gain = Value::Array;
        }
        if send.pre_fader {
            gain = match gain {
                Value::Scalar(g) => Value::Scalar(g * mute),
                Value::Array => {
                    for g in &mut scratch.send {
                        *g *= mute;
                    }
                    Value::Array
                }
            };
        }
        let source: &[[f64; 2]] = if send.pre_fader {
            buffers.current
        } else {
            &scratch.post
        };
        match gain {
            Value::Scalar(g) => {
                for (y, x) in scratch.contribution.iter_mut().zip(source) {
                    *y = [x[0] * g, x[1] * g];
                }
            }
            Value::Array => {
                let frames = scratch
                    .contribution
                    .iter_mut()
                    .zip(source)
                    .zip(&scratch.send);
                for ((y, x), &g) in frames {
                    *y = [x[0] * g, x[1] * g];
                }
            }
        }
        if let Some(line) = &mut send.compensation {
            line.process_in_place(scratch.contribution.as_flattened_mut());
        }
        if send.carries
            && let Some(target) = buffers.target(send.target)
        {
            add(target, &scratch.contribution);
        }
    }
}

/// The node's post-fader signal through its compensation delay into its output target.
fn send_output(node: &mut Node, buffers: &mut Buffers<'_>, scratch: &mut Scratch) {
    if let Some(line) = &mut node.output_compensation {
        line.process_in_place(scratch.post.as_flattened_mut());
    }
    if node.output_carries
        && let Some(target) = buffers.target(node.output)
    {
        add(target, &scratch.post);
    }
}

/// `target += x`.
fn add(target: &mut [[f64; 2]], x: &[[f64; 2]]) {
    for (y, v) in target.iter_mut().zip(x) {
        *y = [y[0] + v[0], y[1] + v[1]];
    }
}

#[cfg(test)]
mod tests {
    use super::balance;

    /// The prototype's `balance_gains` (host virtualenv, 2026-10-21): exactly unity at the
    /// centre; at pan 0.1 the left side tapers.
    #[test]
    fn the_balance_law_matches_the_prototype() {
        assert_eq!(balance(0.0), (1.0, 1.0));
        let (l, r) = balance(0.1);
        assert_eq!(l.to_bits(), 0x3fed_6402_87df_2ee5, "{l}");
        assert_eq!(r, 1.0);
    }
}
