//! Real-time: runs on the audio render thread (see docs/audio-engine.md); `prepare` is setup.
//!
//! `sonara.delay`: a stereo feedback delay with tempo sync, ping-pong and a band-limited
//! feedback path, as the prototype's `dsp/time_frequency/delay.py` computes it.
//!
//! The recursion `line[n] = x[n] + fb · wet[n − D]` runs in chunks no longer than the delay `D`,
//! so a chunk only reads frames that earlier chunks or blocks wrote. A delay change crossfades
//! the old and the new read positions over the whole block with `linspace(0, 1, n)`, in chunks
//! of the shorter of the two delays. The echo passes the feedback filter (an RBJ high-pass when
//! `lowcut_hz > 21`, an RBJ low-pass when `highcut_hz < 19999`, Q 0.7071) before it is both the
//! wet signal and the feedback.
//!
//! Memory: `prepare` sizes the line for the longest delay, `int(4 · sr) + 2 · block` frames,
//! like the prototype's `reset`; nothing is allocated afterwards.
//!
//! Kept quirk (section 5): `tail_seconds` uses the delay of the last
//! processed block, so it is 0 before the first block and the offline auto tail ignores the
//! delay's echoes.

use super::params::{choice, flag, float};
use super::pyops::{py_max, py_min};
use super::{DEFAULT_SPEC, ramp_length, store_params};
use crate::filters::{BiquadType, Section, Smoothed, SmoothedValue, SosFilter, biquad};
use crate::numpy::math::linspace;
use crate::processor::{BlockContext, ParamSlots, ProcessSpec, Processor};

/// The plugin this processor implements.
pub(crate) const PLUGIN_ID: &str = "sonara.delay";

// Slot indices, in the descriptor's parameter order.
const TIME_MS: usize = 0;
const SYNC: usize = 1;
const NOTE: usize = 2;
const FEEDBACK: usize = 3;
const MIX: usize = 4;
const PING_PONG: usize = 5;
const LOWCUT_HZ: usize = 6;
const HIGHCUT_HZ: usize = 7;

/// The longest delay, in seconds (`MAX_DELAY_SECONDS`).
const MAX_DELAY_SECONDS: f64 = 4.0;
/// The shortest delay, in seconds.
const MIN_DELAY_SECONDS: f64 = 0.0005;
/// The feedback high-pass is on above this frequency, in Hz.
const LOWCUT_ON_ABOVE: f64 = 21.0;
/// The feedback low-pass is on below this frequency, in Hz.
const HIGHCUT_ON_BELOW: f64 = 19_999.0;
/// The feedback filters' Q, the prototype's literal.
#[expect(
    clippy::approx_constant,
    reason = "RT-11: the prototype's 0.7071, not 1/√2 (0.70710678…)"
)]
const FILTER_Q: f64 = 0.7071;
/// The feedback filter's largest design: a high-pass and a low-pass section.
const FILTER_SECTIONS: usize = 2;
/// The feedback before the first parameters (`_feedback`).
const INITIAL_FEEDBACK: f64 = 0.3;
/// The longest tail, in seconds.
const MAX_TAIL_SECONDS: f64 = 30.0;

/// `NOTE_VALUES`: the length of each `note` choice in beats, in descriptor order (`1/32`,
/// `1/16T`, `1/16`, `1/16D`, `1/8T`, `1/8`, `1/8D`, `1/4T`, `1/4`, `1/4D`, `1/2`, `1/2D`,
/// `1/1`). The thirds are Python's correctly rounded `1 / 6`, `1 / 3` and `2 / 3`.
const NOTE_BEATS: [f64; 13] = [
    0.125,
    1.0 / 6.0,
    0.25,
    0.375,
    1.0 / 3.0,
    0.5,
    0.75,
    2.0 / 3.0,
    1.0,
    1.5,
    2.0,
    3.0,
    4.0,
];

/// The decoded parameter set.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Settings {
    time_ms: f64,
    sync: bool,
    /// The note value in beats.
    note_beats: f64,
    feedback: f64,
    mix: f64,
    ping_pong: bool,
    lowcut_hz: f64,
    highcut_hz: f64,
}

impl Settings {
    fn from_slots(slots: &ParamSlots) -> Option<Self> {
        Some(Self {
            time_ms: float(slots, TIME_MS)?,
            sync: flag(slots, SYNC)?,
            note_beats: *NOTE_BEATS.get(usize::from(choice(slots, NOTE)?))?,
            feedback: float(slots, FEEDBACK)?,
            mix: float(slots, MIX)?,
            ping_pong: flag(slots, PING_PONG)?,
            lowcut_hz: float(slots, LOWCUT_HZ)?,
            highcut_hz: float(slots, HIGHCUT_HZ)?,
        })
    }

    /// `_target_delay`: the delay in frames for a block at `bpm`, `max(1, round(seconds · sr))`
    /// with the seconds clamped to `[0.0005, 4]`.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Python's round() of a positive length of at most 4 · sr frames"
    )]
    fn delay_frames(&self, bpm: f64, sample_rate: f64) -> usize {
        let seconds = if self.sync {
            self.note_beats * 60.0 / bpm
        } else {
            self.time_ms * 0.001
        };
        let seconds = py_min(py_max(seconds, MIN_DELAY_SECONDS), MAX_DELAY_SECONDS);
        ((seconds * sample_rate).round_ties_even() as usize).max(1)
    }
}

/// The delay (the prototype's `Delay`).
pub(crate) struct Delay {
    spec: ProcessSpec,
    params: ParamSlots,
    settings: Option<Settings>,
    /// The delay line, `int(4 · sr) + 2 · block` frames (`_line`).
    line: Vec<[f64; 2]>,
    /// Frame of the line the next block starts writing (`_write`).
    write: usize,
    /// The delay of the last block in frames; 0 before the first block, which therefore does
    /// not crossfade (`_delay_samples`).
    current_frames: usize,
    /// The feedback filter (`_fb_filter`), an identity while both filters are off.
    feedback_filter: SosFilter,
    mix: SmoothedValue,
    feedback: f64,
    ping_pong: bool,
    /// The block's echo, filtered (`wet`).
    wet: Vec<[f64; 2]>,
    /// The crossfade `linspace(0, 1, n)` of a block whose delay changed.
    fade: Vec<f64>,
    mix_ramp: Vec<f64>,
}

impl Delay {
    /// Setup: an unprepared processor.
    pub(crate) fn new() -> Self {
        Self {
            spec: DEFAULT_SPEC,
            params: ParamSlots::EMPTY,
            settings: None,
            line: Vec::new(),
            write: 0,
            current_frames: 0,
            feedback_filter: SosFilter::new(2, FILTER_SECTIONS),
            mix: SmoothedValue::new(),
            feedback: INITIAL_FEEDBACK,
            ping_pong: false,
            wet: Vec::new(),
            fade: Vec::new(),
            mix_ramp: Vec::new(),
        }
    }

    /// The prototype's `on_params_changed`.
    fn apply_params(&mut self) {
        let Some(p) = Settings::from_slots(&self.params) else {
            return;
        };
        let sr = f64::from(self.spec.sample_rate);
        self.settings = Some(p);
        self.feedback = p.feedback;
        self.ping_pong = p.ping_pong;
        self.mix.set_ramp(ramp_length(self.spec.block_size));
        self.mix.set_target(p.mix);
        let mut design: [Section; FILTER_SECTIONS] = [[0.0; 6]; FILTER_SECTIONS];
        let mut count = 0;
        let mut add = |section: Section| {
            if let Some(slot) = design.get_mut(count) {
                *slot = section;
                count += 1;
            }
        };
        if p.lowcut_hz > LOWCUT_ON_ABOVE {
            add(biquad(BiquadType::Highpass, p.lowcut_hz, sr, FILTER_Q, 0.0));
        }
        if p.highcut_hz < HIGHCUT_ON_BELOW {
            add(biquad(BiquadType::Lowpass, p.highcut_hz, sr, FILTER_Q, 0.0));
        }
        // ERR-06: cannot fail, the filter holds FILTER_SECTIONS, the largest design; an empty
        // design switches to identity (`set_sos(None)`).
        let _ = self
            .feedback_filter
            .set_sections(design.get(..count).unwrap_or_default());
    }

    /// The echo and the feedback of frames `pos..pos + c` of `buffer`: one chunk of the
    /// prototype's loop. Returns `false` when a buffer is shorter than the chunk (which the
    /// sizing in `prepare` rules out).
    fn chunk(
        &mut self,
        buffer: &[[f64; 2]],
        pos: usize,
        c: usize,
        target: usize,
        old: Option<usize>,
    ) -> bool {
        let size = self.line.len();
        let end = pos.saturating_add(c);
        let (Some(x), Some(echo)) = (buffer.get(pos..end), self.wet.get_mut(pos..end)) else {
            return false;
        };
        let w = self.write.saturating_add(pos);
        let fade = self.fade.get(pos..end).unwrap_or_default();
        for (k, e) in echo.iter_mut().enumerate() {
            let frame = w.saturating_add(k);
            let new = read(&self.line, frame, target);
            *e = match (old, fade.get(k)) {
                // `echo * f + read(old) * (1 − f)`.
                (Some(old), Some(&f)) => {
                    let [ol, or] = read(&self.line, frame, old);
                    let keep = 1.0 - f;
                    [new[0] * f + ol * keep, new[1] * f + or * keep]
                }
                _ => new,
            };
        }
        self.feedback_filter
            .process_in_place(echo.as_flattened_mut());
        let fb = self.feedback;
        for (k, (&[el, er], &[l, r])) in echo.iter().zip(x).enumerate() {
            let feed = if self.ping_pong {
                // `mean(axis=1)` of two channels is `(l + r) / 2`.
                #[expect(
                    clippy::manual_midpoint,
                    reason = "RT-11: NumPy's two-channel mean, written as it computes it"
                )]
                let mono = (l + r) / 2.0;
                [mono + fb * er, fb * el]
            } else {
                [l + fb * el, r + fb * er]
            };
            let index = w.saturating_add(k).checked_rem(size).unwrap_or(0);
            if let Some(slot) = self.line.get_mut(index) {
                *slot = feed;
            }
        }
        true
    }
}

/// `buffer * (1 − m) + wet * m`, with the steady mix or the ramp of `mix`.
fn blend(buffer: &mut [[f64; 2]], wet: &[[f64; 2]], mix: Smoothed, ramp: &[f64]) {
    let mixed = |dry: [f64; 2], wet: [f64; 2], m: f64| {
        let keep = 1.0 - m;
        [dry[0] * keep + wet[0] * m, dry[1] * keep + wet[1] * m]
    };
    match mix {
        Smoothed::Steady(m) => {
            for (frame, &echo) in buffer.iter_mut().zip(wet) {
                *frame = mixed(*frame, echo, m);
            }
        }
        Smoothed::Ramp => {
            for ((frame, &echo), &m) in buffer.iter_mut().zip(wet).zip(ramp) {
                *frame = mixed(*frame, echo, m);
            }
        }
    }
}

/// `_read`: the frame `delay` frames before `frame`, on the ring `line`
/// (`(frame − delay) % size`, Python's non-negative modulo). Silence for an empty line.
fn read(line: &[[f64; 2]], frame: usize, delay: usize) -> [f64; 2] {
    let size = line.len();
    // `frame < size + block` and `delay < size`, so the sum neither overflows nor underflows.
    let index = frame
        .wrapping_add(size)
        .wrapping_sub(delay)
        .checked_rem(size)
        .unwrap_or(0);
    line.get(index).copied().unwrap_or([0.0; 2])
}

impl Processor for Delay {
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Python's int() of 4 · sr, a positive frame count"
    )]
    fn prepare(&mut self, spec: ProcessSpec) {
        self.spec = spec;
        let line_frames = (MAX_DELAY_SECONDS * f64::from(spec.sample_rate)) as usize;
        self.line = vec![[0.0; 2]; line_frames.saturating_add(spec.block_size.saturating_mul(2))];
        self.wet = vec![[0.0; 2]; spec.block_size];
        self.fade = vec![0.0; spec.block_size];
        self.mix_ramp = vec![0.0; spec.block_size];
        self.reset();
        self.apply_params();
    }

    fn set_params(&mut self, params: &ParamSlots) {
        if store_params(&mut self.params, params) {
            self.apply_params();
        }
    }

    fn process(&mut self, buffer: &mut [[f64; 2]], ctx: &BlockContext<'_>) {
        // The prototype cannot process before its first parameters; the block passes.
        let Some(p) = self.settings else {
            return;
        };
        let n = buffer.len().min(self.wet.len());
        let size = self.line.len();
        let Some(buffer) = buffer.get_mut(..n) else {
            return;
        };
        if n == 0 || size == 0 {
            return;
        }
        // The line is longer than the longest delay, so the clamp never binds.
        let target = p
            .delay_frames(ctx.bpm, f64::from(self.spec.sample_rate))
            .min(size - 1);
        let old = if self.current_frames == 0 {
            target
        } else {
            self.current_frames
        };
        self.current_frames = target;
        let (step, fade_from) = if target == old {
            (target, None)
        } else {
            if let Some(fade) = self.fade.get_mut(..n) {
                linspace(0.0, 1.0, fade);
            }
            (target.min(old), Some(old))
        };
        let mut pos = 0;
        while pos < n {
            let c = step.min(n - pos);
            if !self.chunk(buffer, pos, c, target, fade_from) {
                return;
            }
            pos += c;
        }
        self.write = self.write.saturating_add(n).checked_rem(size).unwrap_or(0);
        let (Some(wet), Some(ramp)) = (self.wet.get(..n), self.mix_ramp.get_mut(..n)) else {
            return;
        };
        let mix = self.mix.next_block(ramp);
        blend(buffer, wet, mix, ramp);
    }

    /// The prototype's `reset`: an empty line, no delay yet, and a new (identity) feedback
    /// filter. The line keeps its size; `prepare` sizes it.
    fn reset(&mut self) {
        self.line.fill([0.0; 2]);
        self.write = 0;
        self.current_frames = 0;
        self.feedback_filter.set_identity();
        self.feedback_filter.reset();
    }

    /// `(repeats + 1) · delay / sr`, at most 30 s, with `repeats = 60 / max(0.001, −20 ·
    /// log10(max(fb, 1e-6)))` for a feedback clamped to `[0, 0.98]` (one repeat without
    /// feedback); 0 before the first parameters.
    #[expect(
        clippy::cast_precision_loss,
        reason = "a delay of at most 4 · sr frames is exact in f64"
    )]
    fn tail_seconds(&self) -> f64 {
        if self.params.is_empty() {
            return 0.0;
        }
        let fb = py_min(py_max(self.feedback, 0.0), 0.98);
        let repeats = if fb > 0.0 {
            60.0 / py_max(1e-3, -20.0 * py_max(fb, 1e-6).log10())
        } else {
            1.0
        };
        let tail = (repeats + 1.0) * self.current_frames as f64 / f64::from(self.spec.sample_rate);
        py_min(MAX_TAIL_SECONDS, tail)
    }
}

#[cfg(test)]
mod tests {
    use sonara_core::model::device::{ParamValue, Params};
    use sonara_core::plugins::builtin_registry;

    use super::{
        Delay, FEEDBACK, HIGHCUT_HZ, LOWCUT_HZ, MIX, NOTE, NOTE_BEATS, PING_PONG, PLUGIN_ID, SYNC,
        TIME_MS,
    };
    use crate::processor::{BlockContext, ProcessSpec, Processor};
    use crate::processors::param_slots;

    const BLOCK: usize = 64;

    fn prepared(values: &[(&str, ParamValue)]) -> Delay {
        let registry = builtin_registry();
        let values: Params = values
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        let mut delay = Delay::new();
        delay.prepare(ProcessSpec {
            sample_rate: 48_000,
            block_size: BLOCK,
            channels: 2,
        });
        delay.set_params(&param_slots(registry.get(PLUGIN_ID).unwrap(), &values).unwrap());
        delay
    }

    fn ctx(bpm: f64) -> BlockContext<'static> {
        BlockContext {
            frame: 0,
            sample_rate: 48_000,
            beat: 0.0,
            bpm,
            playing: true,
            sidechain: None,
            events: &[],
        }
    }

    #[test]
    fn slots_follow_the_descriptor_order() {
        let registry = builtin_registry();
        let descriptor = registry.get(PLUGIN_ID).unwrap();
        let ids: Vec<&str> = descriptor.params.iter().map(|p| p.id.as_str()).collect();
        let slots = [
            (TIME_MS, "time_ms"),
            (SYNC, "sync"),
            (NOTE, "note"),
            (FEEDBACK, "feedback"),
            (MIX, "mix"),
            (PING_PONG, "ping_pong"),
            (LOWCUT_HZ, "lowcut_hz"),
            (HIGHCUT_HZ, "highcut_hz"),
        ];
        assert_eq!(ids.len(), slots.len());
        for (index, id) in slots {
            assert_eq!(ids.get(index), Some(&id));
        }
        let note = descriptor.params.get(NOTE).unwrap();
        assert_eq!(note.choices.len(), NOTE_BEATS.len());
        assert_eq!(note.choices.get(6).map(String::as_str), Some("1/8D"));
    }

    #[test]
    fn an_impulse_echoes_after_the_delay_with_the_feedback() {
        // 1 ms unsynced is 48 frames; full wet, flat feedback path.
        let mut delay = prepared(&[
            ("sync", ParamValue::Bool(false)),
            ("time_ms", ParamValue::Float(1.0)),
            ("mix", ParamValue::Float(1.0)),
            ("feedback", ParamValue::Float(0.5)),
            ("lowcut_hz", ParamValue::Float(20.0)),
            ("highcut_hz", ParamValue::Float(20_000.0)),
        ]);
        let mut block = [[0.0; 2]; BLOCK];
        block[0] = [1.0, -1.0];
        delay.process(&mut block, &ctx(120.0));
        assert_eq!(block[0], [0.0, 0.0], "fully wet: the dry impulse is gone");
        assert_eq!(block[48], [1.0, -1.0]);
        let mut next = [[0.0; 2]; BLOCK];
        delay.process(&mut next, &ctx(120.0));
        // The second repeat at frame 96 is the first one times the feedback.
        assert_eq!(next[96 - BLOCK], [0.5, -0.5]);
        assert_eq!(delay.current_frames, 48);
    }

    #[test]
    fn ping_pong_feeds_the_mono_sum_left_and_crosses_the_echoes() {
        let mut delay = prepared(&[
            ("note", ParamValue::from("1/32")),
            ("mix", ParamValue::Float(1.0)),
            ("feedback", ParamValue::Float(0.5)),
            ("ping_pong", ParamValue::Bool(true)),
            ("lowcut_hz", ParamValue::Float(20.0)),
            ("highcut_hz", ParamValue::Float(20_000.0)),
        ]);
        let mut block = [[0.0; 2]; BLOCK];
        block[0] = [1.0, 0.0];
        // A 1/32 note at 10^6 BPM is clamped to the shortest delay, 0.5 ms or 24 frames.
        delay.process(&mut block, &ctx(1e6));
        assert_eq!(delay.current_frames, 24);
        // The left echo of the mono sum, then the crossed right one.
        assert_eq!(block[24], [0.5, 0.0]);
        assert_eq!(block[48], [0.0, 0.25]);
    }

    #[test]
    fn the_tail_follows_the_last_block_and_sync_uses_the_tempo() {
        let mut delay = prepared(&[
            ("note", ParamValue::from("1/4")),
            ("feedback", ParamValue::Float(0.0)),
        ]);
        assert_eq!(delay.tail_seconds(), 0.0, "no block yet: the kept quirk");
        let mut block = [[0.0; 2]; BLOCK];
        delay.process(&mut block, &ctx(120.0));
        // A quarter at 120 BPM is 0.5 s; one repeat without feedback.
        assert_eq!(delay.current_frames, 24_000);
        assert_eq!(delay.tail_seconds(), 1.0);
        assert_eq!(Delay::new().tail_seconds(), 0.0);
    }

    #[test]
    fn a_tempo_change_crossfades_without_panicking_and_reset_clears() {
        let mut delay = prepared(&[("mix", ParamValue::Float(0.5))]);
        let mut block = [[0.25, -0.25]; BLOCK];
        for bpm in [120.0, 90.0, 240.0, 1e9] {
            delay.process(&mut block, &ctx(bpm));
            assert!(block.iter().flatten().all(|v| v.is_finite()), "{bpm}");
        }
        delay.reset();
        assert_eq!(delay.current_frames, 0);
        assert!(delay.line.iter().flatten().all(|&v| v == 0.0));
        let mut silence = [[0.0; 2]; BLOCK];
        delay.process(&mut silence, &ctx(120.0));
        assert_eq!(silence, [[0.0; 2]; BLOCK]);
    }
}
