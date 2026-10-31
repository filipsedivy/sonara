//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! `sonara.synth.poly`: the polyphonic subtractive synthesizer of the prototype's
//! `instruments/synth.py` (section 1). Per voice it runs two
//! band-limited oscillators with unison detune and spread ([`osc_bank`]), seeded noise, a
//! resonant state-variable filter whose cutoff follows an envelope, key tracking and the LFO
//! ([`svf`]), and an amplitude envelope.
//!
//! - **Events** are applied sample-accurately: the block is split at the event offsets
//!   (`end = min(max(next, pos + 1), n)`), and the events at or before a segment's start are
//!   applied before it is rendered. `ctx.events` is used in the order given, which
//!   [`BlockContext`] guarantees is sorted by offset; the prototype's stable `sorted` is then a
//!   no-op.
//! - **Voicing:** polyphonic with retrigger and voice stealing (the victim is the first voice
//!   with the smallest `(not releasing, order)` among those not fast-releasing), or mono and
//!   legato, which reuse the last active voice and glide from its pitch. The sustain pedal
//!   (CC 64), the mod wheel (CC 1), CC 120 and 123, all notes off and pitch bend follow
//!   `_apply`.
//! - **Per segment:** the LFO starts from the stored phase, which then advances by
//!   `rate · m / sr` modulo 1, even without voices; the noise generator is reseeded with
//!   `default_rng((seed · 1 000 003 + age) & 0xFFFFFFFF)`. Both depend on the segmentation,
//!   as in the prototype (a kept quirk, section 5).
//! - **Envelopes** are analytic in the voice age: a linear attack, an exponential decay toward
//!   the sustain level and an exponential release (−60 dB at the release time). A voice ends
//!   once its release has lasted longer than the release time, checked at the end of each
//!   segment.
//! - `reset` keeps the note counter and the mod wheel, as the prototype's does.
//!
//! **Exactness.** The oscillators and the filter are the FMA-exact kernels; the noise is
//! the bit-exact `default_rng` stream; everything else is plain arithmetic in the
//! prototype's order. `np.power(2.0, x)` is libm's `pow` (bitwise, verified on 400 000
//! values with the prototype's NumPy 2.5.4), so the cutoff uses `powf`, not `exp2`. What
//! remains is libm.
//!
//! **Capacity.** The prototype's voice list is unbounded: a fast-released voice (a retrigger, a
//! stolen voice, a mono hand-over or CC 120) keeps sounding for 5 ms after the steal, so many
//! note-ons within 5 ms add voices beyond the polyphony. The pool here holds [`MAX_VOICES`]
//! voices, which `new` allocates. At most 32 voices (the polyphony's maximum) are not
//! fast-releasing at any time, so a full pool always holds fast-releasing voices. A note-on
//! that finds the pool full first drops the oldest fast-releasing voice (the first in list
//! order; the oldest voice when there is none), which cuts that voice's last milliseconds of
//! fade. No other limit exists: events are read from the slice, and the per-segment buffers are
//! sized for the prepared block.

use std::f64::consts::TAU;

use super::params::{choice, float, int};
use super::pyops::{clip, maximum, py_max, py_min, py_mod};
use super::{DEFAULT_SPEC, scale, store_params};
use crate::filters::db_to_amp;
use crate::kernels::{Oscillator, SvfMode, Wave, clamp_cutoff, clamp_q, osc_bank, svf};
use crate::numpy::math::linspace;
use crate::numpy::random::default_rng;
use crate::processor::{
    BlockContext, EventKind, InstrumentEvent, ParamSlots, ProcessSpec, Processor, Report,
};

/// The plugin this processor implements.
pub(crate) const PLUGIN_ID: &str = "sonara.synth.poly";

// Slot indices, in the descriptor's parameter order.
const VOICE_MODE: usize = 0;
const POLYPHONY: usize = 1;
const GLIDE: usize = 2;
const OSC1_WAVE: usize = 3;
const OSC1_LEVEL: usize = 4;
const OSC1_OCTAVE: usize = 5;
const OSC1_DETUNE: usize = 6;
const OSC2_WAVE: usize = 7;
const OSC2_LEVEL: usize = 8;
const OSC2_OCTAVE: usize = 9;
const OSC2_SEMITONE: usize = 10;
const OSC2_DETUNE: usize = 11;
const PULSE_WIDTH: usize = 12;
const NOISE_LEVEL: usize = 13;
const UNISON: usize = 14;
const UNISON_DETUNE: usize = 15;
const UNISON_SPREAD: usize = 16;
const FILTER_TYPE: usize = 17;
const FILTER_CUTOFF: usize = 18;
const FILTER_RESONANCE: usize = 19;
const FILTER_ENV_AMOUNT: usize = 20;
const FILTER_KEYTRACK: usize = 21;
const FILTER_ATTACK: usize = 22;
const FILTER_DECAY: usize = 23;
const FILTER_SUSTAIN: usize = 24;
const FILTER_RELEASE: usize = 25;
const AMP_ATTACK: usize = 26;
const AMP_DECAY: usize = 27;
const AMP_SUSTAIN: usize = 28;
const AMP_RELEASE: usize = 29;
const LFO_RATE: usize = 30;
const LFO_PITCH_CENTS: usize = 31;
const LFO_CUTOFF_OCT: usize = 32;
const VELOCITY_SENSITIVITY: usize = 33;
const VELOCITY_TO_FILTER: usize = 34;
const BEND_RANGE: usize = 35;
const GAIN_DB: usize = 36;

/// `_LN_DECAY`: the decay stage and the glide reach 1 % of their distance at their time. The
/// prototype's literal, not `ln(100)`.
const LN_DECAY: f64 = 4.605;
/// `_LN_RELEASE`: the release reaches −60 dB at the release time. The prototype's literal, not
/// `ln(1000)`.
const LN_RELEASE: f64 = 6.908;
/// The release time of a fast release (retrigger, steal, mono hand-over, CC 120), in seconds.
const FAST_RELEASE: f64 = 0.005;
/// The shortest attack, decay, release and glide time the envelopes use, in seconds.
const MIN_STAGE: f64 = 1e-4;
/// The noise generator's seed multiplier (`seed * 1_000_003 + age`).
const NOISE_SEED_STRIDE: u64 = 1_000_003;
/// The most unison voices per oscillator (the `unison` parameter's maximum).
const MAX_UNISON: usize = 7;
/// The most oscillators per voice: two oscillators times the unison.
const MAX_BANK: usize = 2 * MAX_UNISON;
/// The voices the pool holds (see the module docs).
pub(crate) const MAX_VOICES: usize = 128;

/// The `voice_mode` choices, in descriptor order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VoiceMode {
    Poly,
    Mono,
    Legato,
}

fn voice_mode(index: u16) -> Option<VoiceMode> {
    match index {
        0 => Some(VoiceMode::Poly),
        1 => Some(VoiceMode::Mono),
        2 => Some(VoiceMode::Legato),
        _ => None,
    }
}

/// The wave choices (`saw`, `square`, `pulse`, `triangle`, `sine`), which are the kernel's wave
/// codes (`WAVE_CODES`).
fn wave(index: u16) -> Option<Wave> {
    match index {
        0 => Some(Wave::Saw),
        1 => Some(Wave::Square),
        2 => Some(Wave::Pulse),
        3 => Some(Wave::Triangle),
        4 => Some(Wave::Sine),
        _ => None,
    }
}

/// The `filter_type` choices (`_FILTER_MODES`).
fn filter_mode(index: u16) -> Option<SvfMode> {
    match index {
        0 => Some(SvfMode::Lowpass),
        1 => Some(SvfMode::Highpass),
        2 => Some(SvfMode::Bandpass),
        _ => None,
    }
}

/// `q_from_resonance`: a 0..1 resonance to a Q of 0.5 to 25, exponentially.
fn q_from_resonance(resonance: f64) -> f64 {
    let r = py_min(py_max(resonance, 0.0), 1.0);
    0.5 * (r * 50.0_f64.ln()).exp()
}

/// A sample count or a voice age as a float; exact below 2^53 samples (about 5 900 years at
/// 48 kHz).
#[expect(
    clippy::cast_precision_loss,
    reason = "sample counts stay far below 2^53, which converts exactly"
)]
fn count_f64(count: u64) -> f64 {
    count as f64
}

/// A frame index within a block as a float (exact).
#[expect(
    clippy::cast_precision_loss,
    reason = "block indices stay far below 2^53, which converts exactly"
)]
fn index_f64(index: usize) -> f64 {
    index as f64
}

/// A block length as a sample count.
fn count_u64(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}

/// An analytic ADSR envelope (`Envelope`), in seconds and levels as the parameters give them.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Envelope {
    attack: f64,
    decay: f64,
    sustain: f64,
    release: f64,
}

/// The release stage of a voice: when it started, the level it starts from and its effective
/// time.
#[derive(Clone, Copy, Debug)]
struct ReleaseStage {
    at: f64,
    start: f64,
    time: f64,
}

impl Envelope {
    /// `level_at`: the level `t` seconds after the note-on, ignoring the release.
    fn level_at(self, t: f64) -> f64 {
        let a = py_max(self.attack, MIN_STAGE);
        if t < a {
            clip(t / a, 0.0, 1.0)
        } else {
            let d = py_max(self.decay, MIN_STAGE);
            self.sustain + (1.0 - self.sustain) * (-LN_DECAY * maximum(t - a, 0.0) / d).exp()
        }
    }

    /// The release stage for a release `at` seconds after the note-on.
    fn release_from(self, at: f64) -> ReleaseStage {
        ReleaseStage {
            at,
            start: self.level_at(at),
            time: py_max(self.release, MIN_STAGE),
        }
    }

    /// `render`: the level `t` seconds after the note-on, with the release stage if any.
    fn render(self, t: f64, release: Option<ReleaseStage>) -> f64 {
        match release {
            Some(r) if t >= r.at || t.is_nan() => {
                r.start * (-LN_RELEASE * maximum(t - r.at, 0.0) / r.time).exp()
            }
            _ => self.level_at(t),
        }
    }
}

/// The decoded parameter set.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Settings {
    voice_mode: VoiceMode,
    polyphony: i64,
    glide: f64,
    pulse_width: f64,
    noise_level: f64,
    unison: usize,
    filter_mode: SvfMode,
    filter_cutoff: f64,
    q: f64,
    env_amount: f64,
    keytrack: f64,
    filter_env: Envelope,
    amp_env: Envelope,
    lfo_rate: f64,
    lfo_pitch_cents: f64,
    lfo_cutoff_oct: f64,
    velocity_sensitivity: f64,
    velocity_to_filter: f64,
    bend_range: f64,
    gain: f64,
    /// The oscillator bank in the prototype's layout (`osc · unison + u`), with zero phases;
    /// the first `2 · unison` entries are used.
    bank: [Oscillator; MAX_BANK],
}

/// An oscillator setting: wave, level and pitch offset in semitones.
type OscSetting = (Wave, f64, f64);

/// A small integer parameter as a float, or `None` beyond `i32`.
fn small_int(slots: &ParamSlots, index: usize) -> Option<f64> {
    i32::try_from(int(slots, index)?).ok().map(f64::from)
}

impl Settings {
    fn from_slots(slots: &ParamSlots) -> Option<Self> {
        let unison = usize::try_from(int(slots, UNISON)?)
            .ok()
            .filter(|u| (1..=MAX_UNISON).contains(u))?;
        // `int(octave) * 12.0 + detune / 100.0`; the second adds the semitone after the octave.
        let osc1: OscSetting = (
            wave(choice(slots, OSC1_WAVE)?)?,
            float(slots, OSC1_LEVEL)?,
            small_int(slots, OSC1_OCTAVE)? * 12.0 + float(slots, OSC1_DETUNE)? / 100.0,
        );
        let osc2: OscSetting = (
            wave(choice(slots, OSC2_WAVE)?)?,
            float(slots, OSC2_LEVEL)?,
            small_int(slots, OSC2_OCTAVE)? * 12.0
                + small_int(slots, OSC2_SEMITONE)?
                + float(slots, OSC2_DETUNE)? / 100.0,
        );
        let envelope = |a, d, s, r| {
            Some(Envelope {
                attack: float(slots, a)?,
                decay: float(slots, d)?,
                sustain: float(slots, s)?,
                release: float(slots, r)?,
            })
        };
        Some(Self {
            voice_mode: voice_mode(choice(slots, VOICE_MODE)?)?,
            polyphony: int(slots, POLYPHONY)?,
            glide: float(slots, GLIDE)?,
            pulse_width: float(slots, PULSE_WIDTH)?,
            noise_level: float(slots, NOISE_LEVEL)?,
            unison,
            filter_mode: filter_mode(choice(slots, FILTER_TYPE)?)?,
            filter_cutoff: float(slots, FILTER_CUTOFF)?,
            q: q_from_resonance(float(slots, FILTER_RESONANCE)?),
            env_amount: float(slots, FILTER_ENV_AMOUNT)?,
            keytrack: float(slots, FILTER_KEYTRACK)?,
            filter_env: envelope(FILTER_ATTACK, FILTER_DECAY, FILTER_SUSTAIN, FILTER_RELEASE)?,
            amp_env: envelope(AMP_ATTACK, AMP_DECAY, AMP_SUSTAIN, AMP_RELEASE)?,
            lfo_rate: float(slots, LFO_RATE)?,
            lfo_pitch_cents: float(slots, LFO_PITCH_CENTS)?,
            lfo_cutoff_oct: float(slots, LFO_CUTOFF_OCT)?,
            velocity_sensitivity: float(slots, VELOCITY_SENSITIVITY)?,
            velocity_to_filter: float(slots, VELOCITY_TO_FILTER)?,
            bend_range: small_int(slots, BEND_RANGE)?,
            gain: db_to_amp(float(slots, GAIN_DB)?),
            bank: bank(
                [osc1, osc2],
                unison,
                float(slots, UNISON_DETUNE)?,
                float(slots, UNISON_SPREAD)?,
            ),
        })
    }
}

/// The oscillator bank of `_render`: unison offsets `linspace(−d, d, U)` and pans
/// `linspace(−s, s, U)` (zero for one voice), constant-power pan gains
/// `sqrt(0.5 · (1 ∓ pan))` and the levels scaled by `1 / sqrt(U)`.
///
/// `chunks_exact_mut(unison)` cannot panic: `Settings::from_slots` admits 1 to 7 voices only.
#[expect(
    clippy::manual_midpoint,
    reason = "the prototype's `0.5 * (1.0 + pan)`, kept literal next to `0.5 * (1.0 - pan)`"
)]
fn bank(
    oscillators: [OscSetting; 2],
    unison: usize,
    detune_cents: f64,
    spread: f64,
) -> [Oscillator; MAX_BANK] {
    let mut offsets = [0.0; MAX_UNISON];
    let mut pans = [0.0; MAX_UNISON];
    if unison > 1 {
        let detune = detune_cents / 100.0;
        if let Some(offsets) = offsets.get_mut(..unison) {
            linspace(-detune, detune, offsets);
        }
        if let Some(pans) = pans.get_mut(..unison) {
            linspace(-spread, spread, pans);
        }
    }
    let norm = 1.0 / count_f64(count_u64(unison)).sqrt();
    let mut bank = [Oscillator {
        wave: Wave::Sine,
        level: 0.0,
        pitch_offset: 0.0,
        pan_l: 0.0,
        pan_r: 0.0,
        phase: 0.0,
    }; MAX_BANK];
    for (group, (wave, level, offset)) in bank.chunks_exact_mut(unison).zip(oscillators) {
        for ((osc, &detune), &pan) in group.iter_mut().zip(&offsets).zip(&pans) {
            *osc = Oscillator {
                wave,
                level: level * norm,
                pitch_offset: offset + detune,
                pan_l: (0.5 * (1.0 - pan)).sqrt(),
                pan_r: (0.5 * (1.0 + pan)).sqrt(),
                phase: 0.0,
            };
        }
    }
    bank
}

/// One voice (`Voice`).
#[derive(Clone, Copy, Debug)]
struct Voice {
    pitch: u8,
    velocity: u8,
    channel: u8,
    /// Samples since the note-on.
    age: u64,
    /// The age at the release, once released.
    release_age: Option<u64>,
    /// A note-off arrived while the sustain pedal was down.
    sustained: bool,
    fast_release: bool,
    /// The oscillator phases; the first `phase_count` are used.
    phases: [f64; MAX_BANK],
    phase_count: usize,
    /// The filter's integrator states per channel.
    filter_state: [[f64; 2]; 2],
    /// The starting pitch of a running glide, in semitones.
    glide_from: Option<f64>,
    glide_age: u64,
    seed: u64,
    order: u64,
}

impl Voice {
    /// An unused pool slot.
    const EMPTY: Self = Self {
        pitch: 0,
        velocity: 0,
        channel: 0,
        age: 0,
        release_age: None,
        sustained: false,
        fast_release: false,
        phases: [0.0; MAX_BANK],
        phase_count: 0,
        filter_state: [[0.0; 2]; 2],
        glide_from: None,
        glide_age: 0,
        seed: 0,
        order: 0,
    };

    /// A new voice for `event`, numbered `order`, with `phase_count` starting phases
    /// `default_rng(order).random(phase_count)` (`_initial_phases`).
    fn new(event: &InstrumentEvent, order: u64, phase_count: usize) -> Self {
        let mut phases = [0.0; MAX_BANK];
        let count = phase_count.min(MAX_BANK);
        if let Some(phases) = phases.get_mut(..count) {
            default_rng(order).random_fill(phases);
        }
        Self {
            pitch: event.pitch,
            velocity: event.velocity,
            channel: event.channel,
            phases,
            phase_count: count,
            seed: order,
            order,
            ..Self::EMPTY
        }
    }

    fn releasing(&self) -> bool {
        self.release_age.is_some()
    }

    /// `_release`: the release starts now unless it already has; `fast` marks a fast release.
    fn release(&mut self, fast: bool) {
        if self.release_age.is_none() {
            self.release_age = Some(self.age);
        }
        if fast {
            self.fast_release = true;
        }
    }

    /// `_current_pitch`: the pitch in semitones, including a running glide.
    fn current_pitch(&self, glide: f64, sr: f64) -> f64 {
        let pitch = f64::from(self.pitch);
        let Some(from) = self.glide_from else {
            return pitch;
        };
        let glide_s = py_max(glide, MIN_STAGE);
        let t = count_f64(self.glide_age) / sr;
        let k = (-LN_DECAY * t / glide_s).exp();
        pitch + (from - pitch) * k
    }

    /// `np.resize(phases, count)` after a unison change: the old phases repeated cyclically,
    /// or zeros when there were none.
    fn resize_phases(&mut self, count: usize) {
        let count = count.min(MAX_BANK);
        if self.phase_count == count {
            return;
        }
        let old = self.phases;
        let old = old.get(..self.phase_count).unwrap_or_default();
        let new = self.phases.get_mut(..count).unwrap_or_default();
        if old.is_empty() {
            new.fill(0.0);
        } else {
            for (slot, &phase) in new.iter_mut().zip(old.iter().cycle()) {
                *slot = phase;
            }
        }
        self.phase_count = count;
    }

    /// Whether the release has lasted longer than the release time (`finished`).
    fn finished(&self, sr: f64, amp_release: f64) -> bool {
        let Some(released) = self.release_age else {
            return false;
        };
        let limit = if self.fast_release {
            FAST_RELEASE
        } else {
            amp_release
        };
        count_f64(self.age.saturating_sub(released)) / sr > limit
    }
}

/// The LFO of one segment: the vibrato in semitones and the cutoff offset in octaves.
struct Lfo {
    vibrato: Vec<f64>,
    cutoff: Vec<f64>,
}

/// The per-voice buffers of one segment.
struct VoiceBuffers {
    pitch: Vec<f64>,
    left: Vec<f64>,
    right: Vec<f64>,
    cutoff: Vec<f64>,
    dry: Vec<[f64; 2]>,
    wet: Vec<[f64; 2]>,
}

impl VoiceBuffers {
    /// Setup: buffers of `frames` frames.
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    fn new(frames: usize) -> Self {
        Self {
            pitch: vec![0.0; frames],
            left: vec![0.0; frames],
            right: vec![0.0; frames],
            cutoff: vec![0.0; frames],
            dry: vec![[0.0; 2]; frames],
            wet: vec![[0.0; 2]; frames],
        }
    }
}

/// What every voice of one segment shares.
struct Segment<'a> {
    sr: f64,
    bend: f64,
    vibrato: &'a [f64],
    cutoff_lfo: &'a [f64],
}

/// The poly synth (the prototype's `PolySynth`).
pub(crate) struct PolySynth {
    spec: ProcessSpec,
    params: ParamSlots,
    settings: Option<Settings>,
    /// The voice pool, [`MAX_VOICES`] long; the first `voice_count` are the voice list.
    voices: Vec<Voice>,
    voice_count: usize,
    /// Pitch bend in semitones.
    bend: f64,
    sustain_pedal: bool,
    mod_wheel: f64,
    /// The LFO phase at the next segment's start, in cycles.
    lfo_phase: f64,
    /// The note-on counter (`_order`): voice order, noise seed and starting phases.
    order: u64,
    /// The pitch a new mono voice glides from (`_last_pitch`).
    last_pitch: Option<f64>,
    lfo: Lfo,
    buffers: VoiceBuffers,
}

impl PolySynth {
    /// Setup: an unprepared synth with its voice pool.
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    pub(crate) fn new() -> Self {
        Self {
            spec: DEFAULT_SPEC,
            params: ParamSlots::EMPTY,
            settings: None,
            voices: vec![Voice::EMPTY; MAX_VOICES],
            voice_count: 0,
            bend: 0.0,
            sustain_pedal: false,
            mod_wheel: 0.0,
            lfo_phase: 0.0,
            order: 0,
            last_pitch: None,
            lfo: Lfo {
                vibrato: Vec::new(),
                cutoff: Vec::new(),
            },
            buffers: VoiceBuffers::new(0),
        }
    }

    /// The prototype's `on_params_changed`; a set that does not decode changes nothing.
    fn apply_params(&mut self) {
        if let Some(settings) = Settings::from_slots(&self.params) {
            self.settings = Some(settings);
        }
    }

    /// The voice list.
    fn active(&mut self) -> &mut [Voice] {
        self.voices.get_mut(..self.voice_count).unwrap_or_default()
    }

    /// Appends `voice` to the list; a full pool first drops its oldest fast-releasing voice
    /// (see the module docs).
    fn push(&mut self, voice: Voice) {
        if self.voice_count >= self.voices.len() {
            let active = self.active();
            let index = active.iter().position(|v| v.fast_release).unwrap_or(0);
            if let Some(tail) = active.get_mut(index..)
                && !tail.is_empty()
            {
                tail.rotate_left(1);
                self.voice_count -= 1;
            }
        }
        if let Some(slot) = self.voices.get_mut(self.voice_count) {
            *slot = voice;
            self.voice_count += 1;
        }
    }

    /// `_apply`.
    fn apply(&mut self, event: &InstrumentEvent, s: &Settings) {
        match event.kind {
            EventKind::NoteOn if event.velocity == 0 => self.note_off(event),
            EventKind::NoteOn => self.note_on(event, s),
            EventKind::NoteOff => self.note_off(event),
            EventKind::PitchBend => self.bend = f64::from(event.value) / 8192.0 * s.bend_range,
            EventKind::Cc => self.control_change(event),
            EventKind::AllNotesOff => {
                for v in self.active() {
                    v.release(true);
                }
                self.sustain_pedal = false;
            }
            // The synth has no keyswitches and no enum parameter it reads mid-block; the engine
            // sends keyswitch and choice events to samplers only, so the synth's events and
            // output stay the golden ones (GLD-01).
            EventKind::ChannelPressure | EventKind::KeySwitch | EventKind::ParamChoice => {}
        }
    }

    /// The control changes of `_apply`: sustain pedal, mod wheel, CC 120 (all sound off, fast)
    /// and CC 123 (all notes off, normal release).
    fn control_change(&mut self, event: &InstrumentEvent) {
        match event.number {
            64 => {
                let down = event.value >= 64;
                if self.sustain_pedal && !down {
                    for v in self.active() {
                        if v.sustained {
                            v.sustained = false;
                            v.release(false);
                        }
                    }
                }
                self.sustain_pedal = down;
            }
            1 => self.mod_wheel = f64::from(event.value) / 127.0,
            120 | 123 => {
                let fast = event.number == 120;
                for v in self.active() {
                    v.release(fast);
                }
            }
            _ => {}
        }
    }

    /// `_note_off`: releases the sounding voices of the key, or marks them sustained while
    /// the pedal is down.
    fn note_off(&mut self, event: &InstrumentEvent) {
        let pedal = self.sustain_pedal;
        for v in self.active() {
            if v.pitch == event.pitch && v.channel == event.channel && !v.releasing() {
                if pedal {
                    v.sustained = true;
                } else {
                    v.release(false);
                }
            }
        }
    }

    /// `_note_on`.
    fn note_on(&mut self, event: &InstrumentEvent, s: &Settings) {
        self.order = self.order.wrapping_add(1);
        let phase_count = 2 * s.unison;
        match s.voice_mode {
            VoiceMode::Poly => self.poly_note_on(event, s, phase_count),
            mode => self.mono_note_on(event, s, phase_count, mode == VoiceMode::Mono),
        }
    }

    /// Mono and legato: the last active voice takes the new note and glides from its current
    /// pitch; `restart` (mono) restarts its envelopes. Without an active voice a new one starts,
    /// gliding from the last pitch.
    fn mono_note_on(
        &mut self,
        event: &InstrumentEvent,
        s: &Settings,
        phase_count: usize,
        restart: bool,
    ) {
        let sr = f64::from(self.spec.sample_rate);
        let order = self.order;
        let voices = self.active();
        if let Some(index) = voices.iter().rposition(|v| !v.releasing()) {
            let start = voices.get(index).map(|v| v.current_pitch(s.glide, sr));
            for (other, v) in voices.iter_mut().enumerate() {
                if other != index {
                    v.release(true);
                }
            }
            if let Some(v) = voices.get_mut(index) {
                v.glide_from = if s.glide > 0.0 { start } else { None };
                v.glide_age = 0;
                v.pitch = event.pitch;
                v.velocity = event.velocity;
                v.channel = event.channel;
                v.release_age = None;
                v.sustained = false;
                if restart {
                    v.age = 0;
                }
                v.order = order;
            }
            return;
        }
        for v in voices.iter_mut() {
            v.release(true);
        }
        let mut voice = Voice::new(event, order, phase_count);
        if s.glide > 0.0 {
            voice.glide_from = self.last_pitch;
        }
        self.push(voice);
    }

    /// Poly: a retriggered key fast-releases its sounding voice, and a full polyphony steals the
    /// first voice with the smallest `(not releasing, order)` among those not fast-releasing.
    fn poly_note_on(&mut self, event: &InstrumentEvent, s: &Settings, phase_count: usize) {
        let voices = self.active();
        for v in voices.iter_mut() {
            if v.pitch == event.pitch && v.channel == event.channel && !v.releasing() {
                v.release(true);
            }
        }
        let sounding = voices.iter().filter(|v| !v.fast_release).count();
        if i64::try_from(sounding).unwrap_or(i64::MAX) >= s.polyphony {
            // `min` keeps the first of equal keys, as `min_by_key` does.
            let victim = voices
                .iter_mut()
                .filter(|v| !v.fast_release)
                .min_by_key(|v| (!v.releasing(), v.order));
            if let Some(v) = victim {
                v.release(true);
            }
        }
        self.push(Voice::new(event, self.order, phase_count));
    }

    /// `_render` of one segment into `out`, which the voices accumulate into.
    fn render(&mut self, out: &mut [[f64; 2]], s: &Settings) {
        let m = out.len();
        let sr = f64::from(self.spec.sample_rate);
        let lfo_start = self.lfo_phase;
        self.lfo_phase = py_mod(lfo_start + s.lfo_rate * index_f64(m) / sr, 1.0);
        if self.voice_count == 0 {
            return;
        }
        let (Some(vibrato), Some(cutoff_lfo)) =
            (self.lfo.vibrato.get_mut(..m), self.lfo.cutoff.get_mut(..m))
        else {
            return;
        };
        let depth = (s.lfo_pitch_cents + 50.0 * self.mod_wheel) / 100.0;
        for (k, (vib, cut)) in vibrato.iter_mut().zip(cutoff_lfo.iter_mut()).enumerate() {
            let lfo = (TAU * (lfo_start + s.lfo_rate * index_f64(k) / sr)).sin();
            *vib = depth * lfo;
            *cut = s.lfo_cutoff_oct * lfo;
        }
        let segment = Segment {
            sr,
            bend: self.bend,
            vibrato,
            cutoff_lfo,
        };
        let voices = self.voices.get_mut(..self.voice_count).unwrap_or_default();
        let mut kept = 0;
        for index in 0..voices.len() {
            let Some(mut voice) = voices.get(index).copied() else {
                break;
            };
            render_voice(&mut voice, s, &segment, &mut self.buffers, out);
            if voice.finished(sr, s.amp_env.release) {
                self.last_pitch = Some(f64::from(voice.pitch));
            } else {
                if let Some(slot) = voices.get_mut(kept) {
                    *slot = voice;
                }
                kept += 1;
            }
        }
        self.voice_count = kept;
        if let Some(last) = kept.checked_sub(1).and_then(|i| self.voices.get(i)) {
            self.last_pitch = Some(f64::from(last.pitch));
        }
    }
}

/// One voice of `_render`: pitch track, oscillators, noise, filter and amplitude, accumulated
/// into `out`; the voice's age advances by the segment length.
fn render_voice(
    v: &mut Voice,
    s: &Settings,
    segment: &Segment<'_>,
    buffers: &mut VoiceBuffers,
    out: &mut [[f64; 2]],
) {
    let m = out.len();
    let sr = segment.sr;
    let (Some(pitch), Some(left), Some(right), Some(cutoff), Some(dry), Some(wet)) = (
        buffers.pitch.get_mut(..m),
        buffers.left.get_mut(..m),
        buffers.right.get_mut(..m),
        buffers.cutoff.get_mut(..m),
        buffers.dry.get_mut(..m),
        buffers.wet.get_mut(..m),
    ) else {
        return;
    };
    let phase_count = 2 * s.unison;
    v.resize_phases(phase_count);
    let released_at = v.release_age.map(|age| count_f64(age) / sr);
    let amp_env = if v.fast_release {
        Envelope {
            release: FAST_RELEASE,
            ..s.amp_env
        }
    } else {
        s.amp_env
    };
    let amp_release = released_at.map(|at| amp_env.release_from(at));
    let filter_release = released_at.map(|at| s.filter_env.release_from(at));

    pitch_track(v, s.glide, sr, pitch);
    for (p, &vibrato) in pitch.iter_mut().zip(segment.vibrato) {
        *p = *p + segment.bend + vibrato;
    }
    left.fill(0.0);
    right.fill(0.0);
    let mut oscillators = s.bank;
    let bank = oscillators.get_mut(..phase_count).unwrap_or_default();
    for (osc, &phase) in bank.iter_mut().zip(&v.phases) {
        osc.phase = phase;
    }
    osc_bank(pitch, bank, s.pulse_width, sr, left, right);
    for (phase, osc) in v.phases.iter_mut().zip(bank.iter()) {
        *phase = osc.phase;
    }
    if s.noise_level > 0.0 {
        // Python's unbounded `seed * 1_000_003 + age`: the low 32 bits of the wrapped u64
        // arithmetic are the same.
        let seed = v.seed.wrapping_mul(NOISE_SEED_STRIDE).wrapping_add(v.age) & 0xFFFF_FFFF;
        let mut rng = default_rng(seed);
        let level = s.noise_level * 0.5;
        let half = 0.5_f64.sqrt();
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let noise = rng.standard_normal() * level;
            *l += noise * half;
            *r += noise * half;
        }
    }

    let age = count_f64(v.age);
    let vel = f64::from(v.velocity) / 127.0;
    let vel_filter = 1.0 - s.velocity_to_filter + s.velocity_to_filter * vel;
    let key_octaves = s.keytrack * f64::from(i16::from(v.pitch) - 60) / 12.0;
    let frames = cutoff
        .iter_mut()
        .zip(dry.iter_mut())
        .zip(left.iter().zip(right.iter()));
    for (k, (((c, d), (&l, &r)), &lfo)) in frames.zip(segment.cutoff_lfo).enumerate() {
        let t = (age + index_f64(k)) / sr;
        let fenv = s.filter_env.render(t, filter_release);
        let octaves = s.env_amount * fenv * vel_filter + key_octaves + lfo;
        *c = clamp_cutoff(s.filter_cutoff * 2.0_f64.powf(octaves), sr);
        *d = [l, r];
    }
    svf(
        dry.as_flattened(),
        wet.as_flattened_mut(),
        cutoff,
        clamp_q(s.q),
        s.filter_mode,
        sr,
        &mut v.filter_state,
    );
    let vel_gain = 1.0 - s.velocity_sensitivity + s.velocity_sensitivity * vel * vel;
    for (k, ([ol, or], &[wl, wr])) in out.iter_mut().zip(wet.iter()).enumerate() {
        let t = (age + index_f64(k)) / sr;
        let gain = amp_env.render(t, amp_release) * vel_gain;
        *ol += wl * gain;
        *or += wr * gain;
    }
    v.age = v.age.saturating_add(count_u64(m));
}

/// The voice's pitch in semitones per sample, including a glide, which ends once it has run
/// for twice the glide time (checked at the end of the segment).
fn pitch_track(v: &mut Voice, glide: f64, sr: f64, pitch: &mut [f64]) {
    let base = f64::from(v.pitch);
    let Some(from) = v.glide_from else {
        pitch.fill(base);
        return;
    };
    let glide_s = py_max(glide, MIN_STAGE);
    let glide_age = count_f64(v.glide_age);
    for (k, p) in pitch.iter_mut().enumerate() {
        let t = (glide_age + index_f64(k)) / sr;
        *p = base + (from - base) * (-LN_DECAY * t / glide_s).exp();
    }
    v.glide_age = v.glide_age.saturating_add(count_u64(pitch.len()));
    if count_f64(v.glide_age) / sr > glide_s * 2.0 {
        v.glide_from = None;
    }
}

impl Processor for PolySynth {
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    fn prepare(&mut self, spec: ProcessSpec) {
        self.spec = spec;
        self.lfo = Lfo {
            vibrato: vec![0.0; spec.block_size],
            cutoff: vec![0.0; spec.block_size],
        };
        self.buffers = VoiceBuffers::new(spec.block_size);
        self.reset();
        self.apply_params();
    }

    fn set_params(&mut self, params: &ParamSlots) {
        if store_params(&mut self.params, params) {
            self.apply_params();
        }
    }

    fn process(&mut self, buffer: &mut [[f64; 2]], ctx: &BlockContext<'_>) {
        let n = buffer.len().min(self.lfo.vibrato.len());
        let Some(out) = buffer.get_mut(..n) else {
            return;
        };
        // An instrument ignores its input (`out = np.zeros((n, 2))`).
        out.fill([0.0; 2]);
        let Some(s) = self.settings else {
            return;
        };
        let events = ctx.events;
        let mut next = 0;
        let mut pos = 0;
        while pos < n {
            while let Some(event) = events.get(next)
                && event.offset <= pos
            {
                self.apply(event, &s);
                next += 1;
            }
            let until = events.get(next).map_or(n, |event| event.offset);
            let end = until.max(pos + 1).min(n);
            if let Some(segment) = out.get_mut(pos..end) {
                self.render(segment, &s);
            }
            pos = end;
        }
        for event in events.get(next..).unwrap_or_default() {
            self.apply(event, &s);
        }
        for frame in out.iter_mut() {
            *frame = scale(*frame, s.gain);
        }
    }

    fn reset(&mut self) {
        self.voice_count = 0;
        self.bend = 0.0;
        self.sustain_pedal = false;
        self.lfo_phase = 0.0;
        self.last_pitch = None;
    }

    fn tail_seconds(&self) -> f64 {
        self.settings.map_or(0.5, |s| s.amp_env.release + 0.05)
    }

    fn report(&self) -> Report {
        Report::new(&[("active_voices", count_f64(count_u64(self.voice_count)))])
    }
}

#[cfg(test)]
mod tests {
    use sonara_core::model::device::{ParamValue, Params};
    use sonara_core::plugins::builtin_registry;

    use super::*;
    use crate::processors::param_slots;

    const SR: u32 = 48_000;
    const BLOCK: usize = 64;

    fn slots(values: &[(&str, ParamValue)]) -> ParamSlots {
        let registry = builtin_registry();
        let values: Params = values
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        param_slots(registry.get(PLUGIN_ID).unwrap(), &values).unwrap()
    }

    fn prepared(values: &[(&str, ParamValue)]) -> PolySynth {
        let mut synth = PolySynth::new();
        synth.prepare(ProcessSpec {
            sample_rate: SR,
            block_size: BLOCK,
            channels: 2,
        });
        synth.set_params(&slots(values));
        synth
    }

    fn event(offset: usize, kind: EventKind, pitch: u8, velocity: u8) -> InstrumentEvent {
        InstrumentEvent {
            offset,
            kind,
            pitch,
            velocity,
            channel: 0,
            value: 0,
            number: 0,
            articulation: None,
        }
    }

    fn cc(offset: usize, number: u8, value: i32) -> InstrumentEvent {
        InstrumentEvent {
            value,
            number,
            ..event(offset, EventKind::Cc, 0, 0)
        }
    }

    fn block(synth: &mut PolySynth, events: &[InstrumentEvent]) -> [[f64; 2]; BLOCK] {
        let mut out = [[1.0; 2]; BLOCK];
        let ctx = BlockContext {
            frame: 0,
            sample_rate: SR,
            beat: 0.0,
            bpm: 120.0,
            playing: true,
            sidechain: None,
            events,
        };
        synth.process(&mut out, &ctx);
        out
    }

    /// The voice list, which must hold exactly `N` voices.
    fn voices<const N: usize>(synth: &mut PolySynth) -> [Voice; N] {
        <[Voice; N]>::try_from(&*synth.active()).unwrap()
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
            (VOICE_MODE, "voice_mode"),
            (POLYPHONY, "polyphony"),
            (GLIDE, "glide"),
            (OSC1_WAVE, "osc1_wave"),
            (OSC1_LEVEL, "osc1_level"),
            (OSC1_OCTAVE, "osc1_octave"),
            (OSC1_DETUNE, "osc1_detune"),
            (OSC2_WAVE, "osc2_wave"),
            (OSC2_LEVEL, "osc2_level"),
            (OSC2_OCTAVE, "osc2_octave"),
            (OSC2_SEMITONE, "osc2_semitone"),
            (OSC2_DETUNE, "osc2_detune"),
            (PULSE_WIDTH, "pulse_width"),
            (NOISE_LEVEL, "noise_level"),
            (UNISON, "unison"),
            (UNISON_DETUNE, "unison_detune"),
            (UNISON_SPREAD, "unison_spread"),
            (FILTER_TYPE, "filter_type"),
            (FILTER_CUTOFF, "filter_cutoff"),
            (FILTER_RESONANCE, "filter_resonance"),
            (FILTER_ENV_AMOUNT, "filter_env_amount"),
            (FILTER_KEYTRACK, "filter_keytrack"),
            (FILTER_ATTACK, "filter_attack"),
            (FILTER_DECAY, "filter_decay"),
            (FILTER_SUSTAIN, "filter_sustain"),
            (FILTER_RELEASE, "filter_release"),
            (AMP_ATTACK, "amp_attack"),
            (AMP_DECAY, "amp_decay"),
            (AMP_SUSTAIN, "amp_sustain"),
            (AMP_RELEASE, "amp_release"),
            (LFO_RATE, "lfo_rate"),
            (LFO_PITCH_CENTS, "lfo_pitch_cents"),
            (LFO_CUTOFF_OCT, "lfo_cutoff_oct"),
            (VELOCITY_SENSITIVITY, "velocity_sensitivity"),
            (VELOCITY_TO_FILTER, "velocity_to_filter"),
            (BEND_RANGE, "bend_range"),
            (GAIN_DB, "gain_db"),
        ];
        assert_eq!(ids.len(), slots.len());
        for (index, id) in slots {
            assert_eq!(ids.get(index), Some(&id));
        }
        let unison = registry.get(PLUGIN_ID).unwrap().params.get(UNISON).unwrap();
        assert_eq!(unison.maximum, Some(7.0), "MAX_UNISON");
    }

    #[test]
    fn envelopes_are_analytic() {
        let env = Envelope {
            attack: 0.01,
            decay: 0.1,
            sustain: 0.5,
            release: 0.2,
        };
        assert_eq!(env.level_at(0.0), 0.0);
        assert!((env.level_at(0.005) - 0.5).abs() < 1e-12);
        assert_eq!(env.level_at(0.01), 1.0);
        // 1 % of the decay distance remains after the decay time.
        assert!((env.level_at(0.11) - 0.505).abs() < 1e-5);
        let release = Some(env.release_from(0.01));
        assert_eq!(
            env.render(0.005, release),
            env.level_at(0.005),
            "before the release"
        );
        // −60 dB at the release time.
        assert!((env.render(0.21, release) - 1e-3).abs() < 1e-6);
    }

    #[test]
    fn without_parameters_the_synth_is_silent() {
        let mut synth = PolySynth::new();
        synth.prepare(ProcessSpec {
            sample_rate: SR,
            block_size: BLOCK,
            channels: 2,
        });
        let out = block(&mut synth, &[event(0, EventKind::NoteOn, 60, 100)]);
        assert_eq!(out, [[0.0; 2]; BLOCK]);
        assert_eq!(synth.tail_seconds(), 0.5);
    }

    #[test]
    fn keyswitch_and_choice_events_change_nothing() {
        // Beside a note at the same offset the segments stay the same, so the oracle is the
        // note alone, bit for bit, also in the blocks after it. The choice event names the
        // synth's first parameter, the enum `voice_mode`, and another of its choices (legato).
        let on = event(0, EventKind::NoteOn, 60, 100);
        let switch = event(0, EventKind::KeySwitch, 24, 0);
        let choice = InstrumentEvent {
            value: 2,
            number: 0,
            ..event(0, EventKind::ParamChoice, 0, 0)
        };
        let mut plain = prepared(&[]);
        let mut switched = prepared(&[]);
        let first = block(&mut plain, &[on]);
        assert!(first.iter().flatten().any(|v| *v != 0.0), "the note sounds");
        assert_eq!(block(&mut switched, &[on, switch, choice]), first);
        for _ in 0..2 {
            assert_eq!(block(&mut switched, &[]), block(&mut plain, &[]));
        }
    }

    #[test]
    fn retrigger_and_stealing() {
        let mut synth = prepared(&[("polyphony", ParamValue::Int(2))]);
        let on = |offset, pitch| event(offset, EventKind::NoteOn, pitch, 100);
        block(&mut synth, &[on(0, 60), on(0, 60)]);
        // The retriggered voice fades out next to its successor.
        let [old, new] = voices::<2>(&mut synth);
        assert!(old.fast_release && !new.releasing());
        block(&mut synth, &[on(0, 62), on(0, 64)]);
        // 62 still fits the polyphony; 64 steals the oldest sounding voice (the second 60). The
        // fast releases have not ended yet (128 samples are below 5 ms).
        let [a, b, c, d] = voices::<4>(&mut synth);
        assert_eq!([a.pitch, b.pitch, c.pitch, d.pitch], [60, 60, 62, 64]);
        assert!(a.fast_release && b.fast_release);
        assert!(!c.releasing() && !d.releasing());
        assert_eq!(synth.report().entries(), [("active_voices", 4.0)]);
    }

    #[test]
    fn the_sustain_pedal_holds_released_notes() {
        let mut synth = prepared(&[]);
        block(
            &mut synth,
            &[
                cc(0, 64, 127),
                event(1, EventKind::NoteOn, 60, 100),
                event(2, EventKind::NoteOff, 60, 0),
            ],
        );
        let [v] = voices::<1>(&mut synth);
        assert!(v.sustained && !v.releasing());
        block(&mut synth, &[cc(0, 64, 0)]);
        let [v] = voices::<1>(&mut synth);
        assert!(!v.sustained && v.releasing() && !v.fast_release);
    }

    #[test]
    fn control_changes_and_bend() {
        let mut synth = prepared(&[("bend_range", ParamValue::Int(12))]);
        let bend = InstrumentEvent {
            value: -4096,
            ..event(0, EventKind::PitchBend, 0, 0)
        };
        block(&mut synth, &[bend, cc(0, 1, 127)]);
        assert_eq!(synth.bend, -6.0);
        assert_eq!(synth.mod_wheel, 1.0);
        block(
            &mut synth,
            &[event(0, EventKind::NoteOn, 60, 100), cc(5, 123, 0)],
        );
        let [v] = voices::<1>(&mut synth);
        assert!(v.releasing() && !v.fast_release, "CC 123 releases normally");
        block(&mut synth, &[cc(0, 120, 0)]);
        let [v] = voices::<1>(&mut synth);
        assert!(v.fast_release, "CC 120 cuts");
        // reset keeps the mod wheel and the note counter.
        synth.reset();
        assert_eq!(synth.mod_wheel, 1.0);
        assert_eq!(synth.order, 1);
        assert_eq!(synth.bend, 0.0);
    }

    #[test]
    fn mono_reuses_the_voice_and_glides() {
        let mut synth = prepared(&[
            ("voice_mode", ParamValue::from("legato")),
            ("glide", ParamValue::Float(0.05)),
        ]);
        block(&mut synth, &[event(0, EventKind::NoteOn, 48, 100)]);
        block(&mut synth, &[event(10, EventKind::NoteOn, 60, 90)]);
        let [v] = voices::<1>(&mut synth);
        assert_eq!((v.pitch, v.velocity, v.order), (60, 90, 2));
        assert_eq!(v.glide_from, Some(48.0));
        let block = count_u64(BLOCK);
        assert_eq!(v.age, 2 * block, "legato keeps the envelope running");
        assert_eq!(v.glide_age, block - 10);
    }

    #[test]
    fn a_unison_change_repeats_the_phases() {
        let mut voice = Voice::new(&event(0, EventKind::NoteOn, 60, 100), 3, 4);
        let [o0, o1, o2, o3, ..] = voice.phases;
        voice.resize_phases(6);
        let [p0, p1, p2, p3, p4, p5, ..] = voice.phases;
        assert_eq!([p0, p1, p2, p3, p4, p5], [o0, o1, o2, o3, o0, o1]);
        voice.resize_phases(2);
        let [p0, p1, ..] = voice.phases;
        assert_eq!([p0, p1], [o0, o1]);
        assert_eq!(voice.phase_count, 2);
    }

    #[test]
    fn a_full_pool_drops_the_oldest_fast_release() {
        let mut synth = prepared(&[("polyphony", ParamValue::Int(1))]);
        let ons: Vec<InstrumentEvent> = (0..=MAX_VOICES)
            .map(|i| event(0, EventKind::NoteOn, u8::try_from(i % 100).unwrap(), 100))
            .collect();
        let mut out = [[0.0; 2]; BLOCK];
        let ctx = BlockContext {
            frame: 0,
            sample_rate: SR,
            beat: 0.0,
            bpm: 120.0,
            playing: true,
            sidechain: None,
            events: &ons,
        };
        synth.process(&mut out, &ctx);
        let voices = synth.active();
        assert_eq!(voices.len(), MAX_VOICES);
        let (first, last) = (voices.first().unwrap(), voices.last().unwrap());
        assert_eq!(first.order, 2, "the first voice was dropped");
        assert!(first.fast_release && !last.releasing());
    }
}
