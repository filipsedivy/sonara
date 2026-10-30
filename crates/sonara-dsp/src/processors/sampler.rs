//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! `sonara.sampler`: a polyphonic sample player for the instruments of an SFZ patch
//! ([`SampleBank`]). It is Sonara's own engine (ADR 0010): there is no prototype to match, so
//! the golden data does not cover it; `tests/sampler.rs` pins its behaviour.
//!
//! - **Layers and articulations.** A key press starts every region whose key range, velocity
//!   range, keyswitch, random range and round-robin position match. A press of one of the
//!   patch's keyswitch keys makes it the active switch; the patch's default switch is active
//!   until then, also when the patch has no keyswitch keys.
//! - **Velocity.** A region's gain follows the note's velocity as its `amp_veltrack` says: the
//!   curve `(velocity / 127)²` at the default 100 %, constant at 0 %, inverted below 0. Its
//!   velocity crossfade (`xfin_*vel`, `xfout_*vel`, `xf_velcurve`) scales it too, so dynamic
//!   layers that overlap in velocity blend instead of both sounding at full gain
//!   ([`Region::velocity_gain`]). The gain is set when the voice starts and costs nothing per
//!   sample. A region that a note would start at zero gain, such as a layer outside its
//!   crossfade, starts no voice.
//! - **Round robin.** `seq_length`/`seq_position` follow one counter that advances on each
//!   key press that matched a round-robin region. `lorand`/`hirand` use a fixed-seed generator,
//!   so a render is deterministic (TST-04).
//! - **Release.** Key release fades the voice out over the region's release time, and starts
//!   the `trigger=release` regions that match the released key and its original velocity.
//! - **Sustain pedal.** A key released while the pedal (CC 64) is down keeps sounding until the
//!   pedal lifts. Its `trigger=release` regions start then, chosen as at the release: by the
//!   velocity the key was pressed with and the keyswitch active when it was released. A key
//!   released twice under one pedal starts them once, for the later release.
//! - **Exclusive groups.** `group`/`off_by` silence voices quickly.
//! - **Playback.** Linear interpolation, constant memory: 64 preallocated voices, the oldest
//!   one is stolen when all sound (a releasing one first).
//! - **Controls.** Sustain pedal (CC 64), volume (CC 7), expression (CC 11), pitch bend
//!   (±2 semitones) and all notes off.
//!
//! Not supported, and not offered as parameters: CC-driven layers and crossfades, filters,
//! per-region pan, sample streaming. A patch that uses them plays without them (the loader
//! reports the opcodes it ignored). A region that no key starts ([`Region::on_key`] is false: a
//! key of -1, or a region a controller starts) is never started by a key press or release.

use std::sync::Arc;

use super::DEFAULT_SPEC;
use super::params::float;
use crate::bank::{LoopMode, Region, SampleBank, Trigger};
use crate::filters::db_to_amp;
use crate::processor::{
    BlockContext, DeviceState, EventKind, InstrumentEvent, ParamSlots, ProcessSpec, Processor,
    Report,
};

/// The plugin this processor implements.
pub(crate) const PLUGIN_ID: &str = "sonara.sampler";

// Slot indices, in the descriptor's parameter order.
const VOLUME_DB: usize = 0;
const PAN: usize = 1;
const RELEASE_SCALE: usize = 2;

/// Voices that can sound at once.
const MAX_VOICES: usize = 64;
/// Shortest fade-in, in seconds: prevents clicks on regions without an attack.
const MIN_ATTACK: f64 = 0.0005;
/// Shortest fade-out, in seconds: prevents clicks on regions without a release.
const MIN_RELEASE: f64 = 0.005;
/// The fade-out of a voice that an exclusive group or a steal silences, in seconds.
const CHOKE_RELEASE: f64 = 0.01;
/// Pitch bend range in semitones.
const BEND_RANGE: f64 = 2.0;
/// The seed of the generator behind `lorand`/`hirand`.
const RNG_SEED: u32 = 0x2545_F491;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Settings {
    gain: f64,
    pan: f64,
    release_scale: f64,
}

impl Settings {
    fn from_slots(slots: &ParamSlots) -> Option<Self> {
        Some(Self {
            gain: db_to_amp(float(slots, VOLUME_DB)?),
            pan: float(slots, PAN)?.clamp(-1.0, 1.0),
            release_scale: float(slots, RELEASE_SCALE)?.clamp(0.1, 4.0),
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct Voice {
    active: bool,
    region: usize,
    sample: usize,
    key: u8,
    channel: u8,
    /// Position in source frames.
    pos: f64,
    /// Source frames per output frame, before pitch bend.
    step: f64,
    /// Region gain times the gain the note's velocity gives the region.
    gain: f64,
    level: f64,
    attack_step: f64,
    release_step: f64,
    releasing: bool,
    /// Key released while the pedal was down: releases when the pedal comes up.
    held_by_pedal: bool,
    off_by: u32,
    age: u64,
}

impl Voice {
    const IDLE: Self = Self {
        active: false,
        region: 0,
        sample: 0,
        key: 0,
        channel: 0,
        pos: 0.0,
        step: 0.0,
        gain: 0.0,
        level: 0.0,
        attack_step: 0.0,
        release_step: 0.0,
        releasing: false,
        held_by_pedal: false,
        off_by: 0,
        age: 0,
    };
}

/// What selects the `trigger=release` regions of a key release.
#[derive(Debug, Clone, Copy)]
struct KeyRelease {
    /// The velocity the key was pressed with.
    velocity: u8,
    /// The last keyswitch when the key was released.
    switch: Option<u8>,
}

/// The sample player.
pub(crate) struct Sampler {
    spec: ProcessSpec,
    params: ParamSlots,
    settings: Option<Settings>,
    bank: Option<Arc<SampleBank>>,
    voices: [Voice; MAX_VOICES],
    /// The velocity each key was pressed with, for release-trigger regions.
    key_velocity: [u8; 128],
    key_down: [bool; 128],
    /// The keys released while the sustain pedal was down: their release regions start when
    /// the pedal lifts.
    released_under_pedal: [Option<KeyRelease>; 128],
    last_switch: Option<u8>,
    round_robin: u32,
    rng: u32,
    bend: f64,
    volume_cc: f64,
    expression_cc: f64,
    pedal: bool,
    clock: u64,
}

impl Sampler {
    /// Setup: an unprepared processor without an instrument (it plays silence).
    pub(crate) fn new() -> Self {
        Self {
            spec: DEFAULT_SPEC,
            params: ParamSlots::EMPTY,
            settings: None,
            bank: None,
            voices: [Voice::IDLE; MAX_VOICES],
            key_velocity: [0; 128],
            key_down: [false; 128],
            released_under_pedal: [None; 128],
            last_switch: None,
            round_robin: 0,
            rng: RNG_SEED,
            bend: 0.0,
            volume_cc: 1.0,
            expression_cc: 1.0,
            pedal: false,
            clock: 0,
        }
    }

    fn rest(&mut self) {
        self.voices = [Voice::IDLE; MAX_VOICES];
        self.key_velocity = [0; 128];
        self.key_down = [false; 128];
        self.released_under_pedal = [None; 128];
        self.last_switch = self.bank.as_ref().and_then(|bank| bank.keyswitches.default);
        self.round_robin = 0;
        self.rng = RNG_SEED;
        self.bend = 0.0;
        self.volume_cc = 1.0;
        self.expression_cc = 1.0;
        self.pedal = false;
    }

    /// A uniform number in `[0, 1)` (xorshift32).
    fn random(&mut self) -> f64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        f64::from(x) / (f64::from(u32::MAX) + 1.0)
    }

    fn apply(&mut self, event: &InstrumentEvent) {
        match event.kind {
            EventKind::NoteOn if event.velocity > 0 => self.note_on(event),
            EventKind::NoteOn | EventKind::NoteOff => self.note_off(event),
            EventKind::PitchBend => self.bend = f64::from(event.value) / 8192.0 * BEND_RANGE,
            EventKind::Cc => self.control_change(event),
            EventKind::ChannelPressure => {}
            EventKind::AllNotesOff => {
                self.pedal = false;
                self.key_down = [false; 128];
                // Everything stops: the releases under the pedal start no tail.
                self.released_under_pedal = [None; 128];
                for voice in &mut self.voices {
                    if voice.active {
                        start_release(voice, self.spec.sample_rate, CHOKE_RELEASE);
                    }
                }
            }
        }
    }

    fn control_change(&mut self, event: &InstrumentEvent) {
        let value = f64::from(event.value.clamp(0, 127)) / 127.0;
        match event.number {
            7 => self.volume_cc = value,
            11 => self.expression_cc = value,
            64 => {
                let down = event.value >= 64;
                if self.pedal && !down {
                    let rate = self.spec.sample_rate;
                    let scale = self.settings.map_or(1.0, |s| s.release_scale);
                    for voice in &mut self.voices {
                        if voice.active && voice.held_by_pedal {
                            voice.held_by_pedal = false;
                            let release = release_time(self.bank.as_deref(), voice.region, scale);
                            start_release(voice, rate, release);
                        }
                    }
                    self.start_pedal_releases(event.channel);
                }
                self.pedal = down;
            }
            _ => {}
        }
    }

    fn note_on(&mut self, event: &InstrumentEvent) {
        let Some(bank) = self.bank.clone() else {
            return;
        };
        let key = event.pitch;
        if bank.keyswitches.contains(key) {
            self.last_switch = Some(key);
        }
        if let Some(down) = self.key_down.get_mut(usize::from(key)) {
            *down = true;
        }
        if let Some(vel) = self.key_velocity.get_mut(usize::from(key)) {
            *vel = event.velocity;
        }
        let roll = self.random();
        let counter = self.round_robin;
        let mut advanced = false;
        for (index, region) in bank.regions.iter().enumerate() {
            if region.trigger != Trigger::Attack
                || !matches_note(region, key, event.velocity, self.last_switch)
                || !(region.rand_lo..region.rand_hi).contains(&roll)
                || !in_sequence(region, counter)
            {
                continue;
            }
            advanced |= region.seq_length > 1;
            self.choke(region.group);
            self.start_voice(&bank, index, region, event);
        }
        if advanced {
            self.round_robin = self.round_robin.wrapping_add(1);
        }
    }

    fn note_off(&mut self, event: &InstrumentEvent) {
        let key = event.pitch;
        if let Some(down) = self.key_down.get_mut(usize::from(key)) {
            *down = false;
        }
        let rate = self.spec.sample_rate;
        let pedal = self.pedal;
        let scale = self.settings.map_or(1.0, |s| s.release_scale);
        let bank = self.bank.clone();
        for voice in &mut self.voices {
            if !voice.active
                || voice.releasing
                || voice.key != key
                || voice.channel != event.channel
            {
                continue;
            }
            let Some(region) = bank.as_ref().and_then(|b| b.regions.get(voice.region)) else {
                continue;
            };
            if region.loop_mode == LoopMode::OneShot || region.trigger == Trigger::Release {
                continue;
            }
            if pedal {
                voice.held_by_pedal = true;
            } else {
                start_release(voice, rate, region.release.max(MIN_RELEASE) * scale);
            }
        }
        let release = KeyRelease {
            velocity: self
                .key_velocity
                .get(usize::from(key))
                .copied()
                .unwrap_or(event.velocity),
            switch: self.last_switch,
        };
        if pedal {
            // SFZ `trigger=release` plays "on note-off or sustain pedal off": the regions wait
            // for the pedal. The release is captured now, so a re-press of the key before the
            // pedal lifts does not change it; a later release of the key replaces it.
            if let Some(slot) = self.released_under_pedal.get_mut(usize::from(key)) {
                *slot = Some(release);
            }
            return;
        }
        if let Some(bank) = bank {
            self.start_release_regions(&bank, key, event.channel, release);
        }
    }

    /// Starts the release regions of the keys released while the pedal was down, in key order,
    /// and forgets those releases.
    fn start_pedal_releases(&mut self, channel: u8) {
        let pending = std::mem::replace(&mut self.released_under_pedal, [None; 128]);
        let Some(bank) = self.bank.clone() else {
            return;
        };
        for (key, release) in (0..=127_u8).zip(pending) {
            if let Some(release) = release {
                self.start_release_regions(&bank, key, channel, release);
            }
        }
    }

    /// Starts the `trigger=release` regions that `release` of `key` selects.
    fn start_release_regions(
        &mut self,
        bank: &SampleBank,
        key: u8,
        channel: u8,
        release: KeyRelease,
    ) {
        let event = InstrumentEvent {
            offset: 0,
            kind: EventKind::NoteOff,
            pitch: key,
            velocity: release.velocity,
            channel,
            value: 0,
            number: 0,
        };
        for (index, region) in bank.regions.iter().enumerate() {
            if region.trigger == Trigger::Release
                && matches_note(region, key, release.velocity, release.switch)
            {
                self.start_voice(bank, index, region, &event);
            }
        }
    }

    /// Silences the voices that exclusive group `group` switches off.
    fn choke(&mut self, group: u32) {
        if group == 0 {
            return;
        }
        let rate = self.spec.sample_rate;
        for voice in &mut self.voices {
            if voice.active && voice.off_by == group {
                start_release(voice, rate, CHOKE_RELEASE);
            }
        }
    }

    fn start_voice(
        &mut self,
        bank: &SampleBank,
        index: usize,
        region: &Region,
        event: &InstrumentEvent,
    ) {
        let Some(sample) = bank.samples.get(region.sample) else {
            return;
        };
        if sample.frames() == 0 || self.spec.sample_rate == 0 {
            return;
        }
        let rate = f64::from(self.spec.sample_rate);
        let cents = (f64::from(event.pitch) - f64::from(region.pitch_keycenter)) * 100.0
            + region.tune_cents;
        let step = f64::from(sample.rate) / rate * (cents / 1200.0).exp2();
        let gain = db_to_amp(region.volume_db) * region.velocity_gain(event.velocity);
        if gain.is_nan() || gain <= 0.0 {
            // A voice that would stay silent is not started. A key press has already switched
            // off the region's exclusive group and counted its round robin (`note_on`).
            return;
        }
        let attack = region.attack.max(MIN_ATTACK);
        self.clock = self.clock.wrapping_add(1);
        let voice = Voice {
            active: true,
            region: index,
            sample: region.sample,
            key: event.pitch,
            channel: event.channel,
            pos: frames_f64(region.offset),
            step,
            gain,
            level: 0.0,
            attack_step: 1.0 / (attack * rate),
            release_step: 0.0,
            releasing: false,
            held_by_pedal: false,
            off_by: region.off_by,
            age: self.clock,
        };
        let slot = self.free_voice();
        if let Some(slot) = self.voices.get_mut(slot) {
            *slot = voice;
        }
    }

    /// The voice to start on: an idle one, else the oldest releasing one, else the oldest.
    fn free_voice(&self) -> usize {
        let mut best = (0, (true, u64::MAX));
        for (i, voice) in self.voices.iter().enumerate() {
            if !voice.active {
                return i;
            }
            let rank = (!voice.releasing, voice.age);
            if rank < best.1 {
                best = (i, rank);
            }
        }
        best.0
    }

    fn render(&mut self, out: &mut [[f64; 2]], settings: Settings) {
        let Some(bank) = self.bank.as_deref() else {
            return;
        };
        let bend = (self.bend / 12.0).exp2();
        let master = settings.gain * self.volume_cc * self.expression_cc;
        // Equal-power balance, unity at the centre.
        let angle = (settings.pan + 1.0) * std::f64::consts::FRAC_PI_4;
        let (pan_l, pan_r) = (
            angle.cos() * std::f64::consts::SQRT_2,
            angle.sin() * std::f64::consts::SQRT_2,
        );
        for voice in &mut self.voices {
            if !voice.active {
                continue;
            }
            let (Some(region), Some(sample)) = (
                bank.regions.get(voice.region),
                bank.samples.get(voice.sample),
            ) else {
                voice.active = false;
                continue;
            };
            let frames = sample.frames();
            let end = if region.end == 0 {
                frames
            } else {
                region.end.min(frames)
            };
            let looping = matches!(region.loop_mode, LoopMode::Continuous | LoopMode::Sustain)
                && region.loop_end > region.loop_start
                && region.loop_end <= frames;
            let step = voice.step * bend;
            for frame in out.iter_mut() {
                if voice.releasing {
                    voice.level -= voice.release_step;
                    if voice.level <= 0.0 {
                        voice.active = false;
                        break;
                    }
                } else if voice.level < 1.0 {
                    voice.level = (voice.level + voice.attack_step).min(1.0);
                }
                if looping && voice.pos >= frames_f64(region.loop_end) {
                    voice.pos -= frames_f64(region.loop_end.saturating_sub(region.loop_start));
                }
                if voice.pos >= frames_f64(end) {
                    voice.active = false;
                    break;
                }
                let [l, r] = read_frame(
                    sample.channels,
                    &sample.data,
                    voice.pos,
                    if looping {
                        Some((region.loop_start, region.loop_end))
                    } else {
                        None
                    },
                    end,
                );
                let amp = voice.gain * voice.level * master;
                frame[0] += l * amp * pan_l;
                frame[1] += r * amp * pan_r;
                voice.pos += step;
            }
        }
    }
}

/// The release time of region `region` in seconds, scaled.
fn release_time(bank: Option<&SampleBank>, region: usize, scale: f64) -> f64 {
    let release = bank
        .and_then(|bank| bank.regions.get(region))
        .map_or(0.0, |r| r.release);
    release.max(MIN_RELEASE) * scale
}

/// A frame count or position as a float; sample lengths stay far below 2^52.
#[expect(
    clippy::cast_precision_loss,
    reason = "frame counts of a sample are far below 2^52"
)]
fn frames_f64(frames: usize) -> f64 {
    frames as f64
}

/// The whole frame below a non-negative position.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the position is non-negative and below the sample length"
)]
fn floor_index(pos: f64) -> usize {
    pos as usize
}

/// Whether `region` plays for a press (or, for a release region, a release) of `key` at
/// `velocity` with `switch` as the last keyswitch. A region that no key starts never does.
fn matches_note(region: &Region, key: u8, velocity: u8, switch: Option<u8>) -> bool {
    region.on_key
        && (region.key_lo..=region.key_hi).contains(&key)
        && (region.vel_lo..=region.vel_hi).contains(&velocity)
        && region.sw_last.is_none_or(|sw| Some(sw) == switch)
}

/// Whether `region` is the round-robin position that `counter` selects.
fn in_sequence(region: &Region, counter: u32) -> bool {
    region.seq_length <= 1
        || counter.checked_rem(region.seq_length).map(|n| n + 1) == Some(region.seq_position)
}

/// Starts a linear fade-out of `seconds` from the voice's current level.
fn start_release(voice: &mut Voice, sample_rate: u32, seconds: f64) {
    if voice.releasing {
        // A choke shortens a release in progress, never lengthens it.
        let quick = voice.level / (seconds * f64::from(sample_rate)).max(1.0);
        voice.release_step = voice.release_step.max(quick);
        return;
    }
    voice.releasing = true;
    voice.release_step =
        voice.level.max(f64::EPSILON) / (seconds * f64::from(sample_rate)).max(1.0);
}

/// The interpolated stereo frame at source position `pos`.
fn read_frame(
    channels: usize,
    data: &[f32],
    pos: f64,
    looping: Option<(usize, usize)>,
    end: usize,
) -> [f64; 2] {
    let first = floor_index(pos);
    let frac = pos - frames_f64(first);
    let mut next = first + 1;
    match looping {
        Some((start, stop)) if next >= stop => next = start + (next - stop),
        _ if next >= end => next = first,
        _ => {}
    }
    let at = |frame: usize, channel: usize| -> f64 {
        data.get(frame * channels + channel)
            .map_or(0.0, |v| f64::from(*v))
    };
    let left = at(first, 0) * (1.0 - frac) + at(next, 0) * frac;
    if channels < 2 {
        return [left, left];
    }
    [left, at(first, 1) * (1.0 - frac) + at(next, 1) * frac]
}

impl Processor for Sampler {
    fn prepare(&mut self, spec: ProcessSpec) {
        self.spec = spec;
        self.rest();
        self.apply_params();
    }

    fn set_params(&mut self, params: &ParamSlots) {
        if super::store_params(&mut self.params, params) {
            self.apply_params();
        }
    }

    fn set_state(&mut self, state: &DeviceState) {
        self.bank.clone_from(&state.sample_bank);
        self.rest();
    }

    fn process(&mut self, buffer: &mut [[f64; 2]], ctx: &BlockContext<'_>) {
        // An instrument ignores its input.
        buffer.fill([0.0; 2]);
        let Some(settings) = self.settings else {
            return;
        };
        let n = buffer.len();
        let events = ctx.events;
        let (mut next, mut pos) = (0, 0);
        while pos < n {
            while let Some(event) = events.get(next)
                && event.offset <= pos
            {
                self.apply(event);
                next += 1;
            }
            let until = events.get(next).map_or(n, |event| event.offset);
            let end = until.max(pos + 1).min(n);
            if let Some(segment) = buffer.get_mut(pos..end) {
                self.render(segment, settings);
            }
            pos = end;
        }
    }

    fn reset(&mut self) {
        self.rest();
    }

    fn tail_seconds(&self) -> f64 {
        self.bank.as_ref().map_or(0.0, |bank| bank.tail_seconds)
    }

    fn report(&self) -> Report {
        let voices = self.voices.iter().filter(|v| v.active).count();
        Report::new(&[(
            "voices",
            f64::from(u32::try_from(voices).unwrap_or(u32::MAX)),
        )])
    }
}

impl Sampler {
    fn apply_params(&mut self) {
        if let Some(settings) = Settings::from_slots(&self.params) {
            self.settings = Some(settings);
        }
    }
}
