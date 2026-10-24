//! Real-time: runs on the audio render thread (see docs/audio-engine.md); the sample synthesis
//! writes into buffers sized by `prepare` (see "When samples are synthesized").
//!
//! `sonara.drums`: the synthesized drum machine of the prototype's `instruments/drums.py` on the
//! General MIDI percussion map (section 1). Every sound is
//! synthesized once per kit into a mono sample (seeded noise, Butterworth filters, sweeps and
//! square-wave "metal"), peak-normalized, and played back sample-accurately. Velocity scales
//! the level, each sound has a fixed pan scaled by the stereo width, the closed and pedal
//! hi-hats choke the open hi-hat, and unmapped notes are ignored and counted in `report`.
//!
//! **Synthesis** follows `synthesize(name, kit, sr)`: `default_rng(zlib.crc32(name))` per
//! sound; `_env = exp(-6.9 t / max(d, 1e-3))`; `_sweep = sin(2π cumsum(f1 + (f0 - f1)
//! exp(-t/τ)) / sr)` with NumPy's sequential `cumsum`; `_metal`, the mean of six
//! `signal.square(2π f t)` (`fmod(arg, 2π) < π`); the band-pass and high-pass designs as exact
//! tables ([`crate::tables`]), filtered from zero state; the drive `tanh(x k) / tanh(k)`; the
//! peak taken before the 64-sample fade-out, then `x / peak`. Every array expression keeps
//! NumPy's element order and association, so a recipe can stream sample by sample.
//!
//! **When samples are synthesized.** The prototype synthesizes lazily, at the first note-on of
//! a sound, and caches the sample per (sound, kit, rate) in the instance, a cache its `reset`
//! keeps. Every sound draws its noise from its own generator (`default_rng(crc32(name))`) and
//! filters from zero state, so a sample does not depend on which sounds were synthesized
//! before it. Here the samples are synthesized eagerly, for the **playable sounds**: those of
//! the device state's playable notes, every sound by default (live input can play any pad, and
//! the render thread must not synthesize on a note-on, RT-02). An offline render passes only
//! the notes its schedule plays, so only their sounds are synthesized; the other samples stay
//! empty, and a note on one of them would play silence, but offline only the schedule plays.
//!
//! - `prepare` (setup) allocates one buffer per sound, as long as the longest sample that sound
//!   can have within the descriptor's ranges at the prepared rate, and synthesizes the playable
//!   sounds for the stored kit. At an unchanged rate it keeps the buffers and their samples,
//!   like the prototype's cache, so the render graph's reset before an offline render (a
//!   second `prepare`) synthesizes nothing.
//! - `set_state` (setup) adopts the playable notes and synthesizes the sounds that became
//!   playable for the stored kit.
//! - `set_params` synthesizes the playable sounds again whenever one of the twelve kit
//!   parameters changes (the levels, the width and the output gain do not). It writes into the
//!   buffers of `prepare`, so it allocates nothing, but it is the one expensive call of this
//!   processor: for all 29 sounds about 26 ms for the default kit at 48 kHz, 46 ms for the
//!   longest preset and 100 ms at 192 kHz (release build, development container on arm64). The
//!   render graph must therefore apply a device's first parameter set while it builds
//!   the node, off the real-time thread; then only automation of a kit parameter puts a
//!   synthesis on the render thread (the prototype pays the same cost there, per sound at its
//!   next note-on).
//! - A kit whose decay parameters exceed the descriptor's maxima would need longer buffers; such
//!   a set does not decode and is ignored, like any other undecodable set.
//! - **Deviation:** a hit that is sounding when the kit changes continues with the new kit's
//!   sample from its position, where the prototype finishes it with the old sample (each of the
//!   prototype's hits holds its own array, so under kit automation the number of live samples
//!   is unbounded). No golden render changes the kit while it plays.
//! - The memory is the sum of the longest samples: 42.02 s of `f64` per instance, about 16 MB at
//!   48 kHz and 65 MB at 192 kHz.
//! - At a sample rate outside the eight supported rates there are no filter tables, so every
//!   sample is empty and the kit is silent (the project model accepts only those rates,
//!   `sonara_core::model::project::SUPPORTED_SAMPLE_RATES`).
//!
//! **Playback** follows `process`: a hit's gain is `level · (vel/127)^1.3`, panned by
//! `θ = (pan · width + 1) π/4` into `(level cos θ) √2` and `(level sin θ) √2`; hits are summed
//! in list order and the block is scaled by the output gain. A choke fades the open hi-hat
//! over 10 ms and all notes off fades every hit over 5 ms, in both cases from the hit's first
//! sample in the current block, not from the event (a kept quirk,
//! section 5).
//!
//! **Capacity.** The prototype's hit list is unbounded. Here it holds [`MAX_HITS`] hits, which
//! `new` allocates; a note-on that finds it full first drops the oldest hit (the first in list
//! order), which cuts that hit off. Events are read from the slice, so nothing else is bounded.
//!
//! **Exactness.** The filters are FMA-exact and the noise is the bit-exact `default_rng`
//! stream; `sin`, `exp` and `pow` are libm. NumPy's own SIMD `tanh` differs from
//! libm by at most one ulp in about a fifth of its inputs, which the drive carries into
//! the samples.

use std::f64::consts::{PI, SQRT_2, TAU};

use super::params::float;
use super::pyops::{clip, maximum, py_max, py_min};
use super::{DEFAULT_SPEC, scale, store_params};
use crate::filters::{Section, SosFilter, db_to_amp};
use crate::numpy::math::linspace;
use crate::numpy::random::{Generator, default_rng};
use crate::processor::{
    BlockContext, DeviceState, EventKind, InstrumentEvent, NoteSet, ParamSlots, ProcessSpec,
    Processor, Report,
};
use crate::tables::{
    BandpassDesign, HighpassDesign, SupportedRate, butter_bandpass, butter_highpass,
};

/// The plugin this processor implements.
pub(crate) const PLUGIN_ID: &str = "sonara.drums";

// Slot indices, in the descriptor's parameter order.
const KICK_TUNE: usize = 0;
const KICK_DECAY: usize = 1;
const KICK_PUNCH: usize = 2;
const SNARE_TUNE: usize = 3;
const SNARE_TONE: usize = 4;
const SNARE_DECAY: usize = 5;
const HAT_TUNE: usize = 6;
const HAT_DECAY: usize = 7;
const CLAP_DECAY: usize = 8;
const TOM_TUNE: usize = 9;
const CYMBAL_DECAY: usize = 10;
const DRIVE: usize = 11;
const KICK_LEVEL_DB: usize = 12;
const SNARE_LEVEL_DB: usize = 13;
const HAT_LEVEL_DB: usize = 14;
const CYMBAL_LEVEL_DB: usize = 15;
const PERC_LEVEL_DB: usize = 16;
const STEREO_WIDTH: usize = 17;
const GAIN_DB: usize = 18;

/// The hits the list holds (see the module docs).
pub(crate) const MAX_HITS: usize = 256;
/// The fade-out at the end of every sample, in samples.
const FADE: usize = 64;
/// The sections of the largest drum filter design (order 4 high-pass, order 2 band-pass).
const MAX_SECTIONS: usize = 2;
/// The choke fade of the open hi-hat, in seconds.
const CHOKE_SECONDS: f64 = 0.01;
/// The fade of all notes off, in seconds.
const ALL_NOTES_OFF_SECONDS: f64 = 0.005;
/// The GM note of the open hi-hat, which notes 42 and 44 choke (`_CHOKES`).
const OPEN_HAT_NOTE: u8 = 46;
/// The frequency ratios of `_metal` (`_HAT_RATIOS`).
const HAT_RATIOS: [f64; 6] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0];

/// The number of sounds.
const SOUND_COUNT: usize = 29;

/// A synthesized drum sound (a value of the prototype's `GM_MAP`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sound {
    Kick,
    Rim,
    Snare,
    Clap,
    TomFloorLow,
    HatClosed,
    TomFloorHigh,
    HatPedal,
    TomLow,
    HatOpen,
    TomMidLow,
    TomMidHigh,
    Crash,
    TomHigh,
    Ride,
    China,
    RideBell,
    Tambourine,
    Splash,
    Cowbell,
    BongoHigh,
    BongoLow,
    CongaMute,
    CongaHigh,
    CongaLow,
    Shaker,
    Claves,
    BlockHigh,
    BlockLow,
}

/// A set of sounds, one bit per [`Sound`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sounds(u32);

impl Sounds {
    /// Every sound.
    const ALL: Self = Self((1 << SOUND_COUNT) - 1);
    /// No sound.
    const NONE: Self = Self(0);

    /// The sounds that the MIDI notes in `notes` play.
    fn of_notes(notes: NoteSet) -> Self {
        let mut bits = 0;
        for pitch in 0..=127 {
            if notes.contains(pitch)
                && let Some(sound) = Sound::from_note(pitch)
            {
                bits |= sound.bit();
            }
        }
        Self(bits)
    }

    fn contains(self, sound: Sound) -> bool {
        self.0 & sound.bit() != 0
    }

    /// Whether every sound of `other` is in the set.
    fn covers(self, other: Self) -> bool {
        other.0 & !self.0 == 0
    }
}

/// The level group of a sound (`_GROUPS`, `perc` by default).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Group {
    Kick,
    Snare,
    Hat,
    Cymbal,
    Perc,
}

impl Sound {
    /// Every sound, in the order of its first note in `GM_MAP`; a sound's index here is its
    /// sample buffer.
    const ALL: [Self; SOUND_COUNT] = [
        Self::Kick,
        Self::Rim,
        Self::Snare,
        Self::Clap,
        Self::TomFloorLow,
        Self::HatClosed,
        Self::TomFloorHigh,
        Self::HatPedal,
        Self::TomLow,
        Self::HatOpen,
        Self::TomMidLow,
        Self::TomMidHigh,
        Self::Crash,
        Self::TomHigh,
        Self::Ride,
        Self::China,
        Self::RideBell,
        Self::Tambourine,
        Self::Splash,
        Self::Cowbell,
        Self::BongoHigh,
        Self::BongoLow,
        Self::CongaMute,
        Self::CongaHigh,
        Self::CongaLow,
        Self::Shaker,
        Self::Claves,
        Self::BlockHigh,
        Self::BlockLow,
    ];

    /// The sound a GM note plays (`GM_MAP.get`).
    fn from_note(note: u8) -> Option<Self> {
        Some(match note {
            35 | 36 => Self::Kick,
            37 => Self::Rim,
            38 | 40 => Self::Snare,
            39 => Self::Clap,
            41 => Self::TomFloorLow,
            42 => Self::HatClosed,
            43 => Self::TomFloorHigh,
            44 => Self::HatPedal,
            45 => Self::TomLow,
            46 => Self::HatOpen,
            47 => Self::TomMidLow,
            48 => Self::TomMidHigh,
            49 | 57 => Self::Crash,
            50 => Self::TomHigh,
            51 | 59 => Self::Ride,
            52 => Self::China,
            53 => Self::RideBell,
            54 => Self::Tambourine,
            55 => Self::Splash,
            56 => Self::Cowbell,
            60 => Self::BongoHigh,
            61 => Self::BongoLow,
            62 => Self::CongaMute,
            63 => Self::CongaHigh,
            64 => Self::CongaLow,
            69 | 70 => Self::Shaker,
            75 => Self::Claves,
            76 => Self::BlockHigh,
            77 => Self::BlockLow,
            _ => return None,
        })
    }

    /// The prototype's name, which seeds the sound's noise (`zlib.crc32(name)`).
    fn name(self) -> &'static str {
        match self {
            Self::Kick => "kick",
            Self::Rim => "rim",
            Self::Snare => "snare",
            Self::Clap => "clap",
            Self::TomFloorLow => "tom_floor_low",
            Self::HatClosed => "hat_closed",
            Self::TomFloorHigh => "tom_floor_high",
            Self::HatPedal => "hat_pedal",
            Self::TomLow => "tom_low",
            Self::HatOpen => "hat_open",
            Self::TomMidLow => "tom_mid_low",
            Self::TomMidHigh => "tom_mid_high",
            Self::Crash => "crash",
            Self::TomHigh => "tom_high",
            Self::Ride => "ride",
            Self::China => "china",
            Self::RideBell => "ride_bell",
            Self::Tambourine => "tambourine",
            Self::Splash => "splash",
            Self::Cowbell => "cowbell",
            Self::BongoHigh => "bongo_high",
            Self::BongoLow => "bongo_low",
            Self::CongaMute => "conga_mute",
            Self::CongaHigh => "conga_high",
            Self::CongaLow => "conga_low",
            Self::Shaker => "shaker",
            Self::Claves => "claves",
            Self::BlockHigh => "block_high",
            Self::BlockLow => "block_low",
        }
    }

    /// The index of the sound's sample buffer.
    fn index(self) -> usize {
        self as usize
    }

    /// The sound's bit in a [`Sounds`] mask.
    fn bit(self) -> u32 {
        // At most 1 << 28: `Sound` has 29 variants.
        1 << (self as u32)
    }

    /// The pan position before the stereo width (`_PANS.get(name, 0.0)`).
    fn pan(self) -> f64 {
        match self {
            Self::HatClosed | Self::HatPedal | Self::HatOpen | Self::CongaMute => 0.25,
            Self::Ride | Self::RideBell => -0.3,
            Self::Crash => -0.4,
            Self::China => 0.45,
            Self::Splash => 0.4,
            Self::TomHigh | Self::CongaHigh => 0.3,
            Self::TomMidHigh => 0.15,
            Self::TomLow => -0.15,
            Self::TomFloorHigh | Self::Claves => -0.25,
            Self::TomFloorLow | Self::Shaker => -0.35,
            Self::Tambourine | Self::BongoHigh => 0.35,
            Self::Cowbell => 0.2,
            Self::CongaLow => -0.2,
            Self::BongoLow => 0.1,
            Self::Kick
            | Self::Rim
            | Self::Snare
            | Self::Clap
            | Self::TomMidLow
            | Self::BlockHigh
            | Self::BlockLow => 0.0,
        }
    }

    /// The level group (`_GROUPS.get(name, "perc")`).
    fn group(self) -> Group {
        match self {
            Self::Kick => Group::Kick,
            Self::Snare | Self::Rim | Self::Clap => Group::Snare,
            Self::HatClosed | Self::HatPedal | Self::HatOpen => Group::Hat,
            Self::Crash | Self::Ride | Self::RideBell | Self::China | Self::Splash => Group::Cymbal,
            _ => Group::Perc,
        }
    }

    /// The sample length in seconds (the argument of `length`).
    fn duration(self, kit: &Kit) -> f64 {
        match self {
            Self::Kick => kit.kick_decay * 1.6 + 0.05,
            Self::Snare => py_max(0.25, kit.snare_decay * 1.5),
            Self::Rim => 0.06,
            Self::Clap => kit.clap_decay * 1.5 + 0.05,
            Self::HatClosed | Self::HatPedal | Self::HatOpen => self.hat_decay(kit) * 1.4 + 0.02,
            Self::Crash | Self::China | Self::Splash => self.cymbal_decay(kit) * 1.3,
            Self::Ride | Self::RideBell => kit.cymbal_decay * 1.2,
            Self::TomFloorLow
            | Self::TomFloorHigh
            | Self::TomLow
            | Self::TomMidLow
            | Self::TomMidHigh
            | Self::TomHigh => 0.6,
            Self::Tambourine => 0.25,
            Self::Cowbell => 0.35,
            Self::Shaker => 0.12,
            Self::Claves | Self::BlockHigh | Self::BlockLow => 0.08,
            Self::BongoHigh
            | Self::BongoLow
            | Self::CongaMute
            | Self::CongaHigh
            | Self::CongaLow => 0.4,
        }
    }

    /// A hi-hat's decay: `{closed: 0.045, pedal: 0.08, open: 0.45}[name] * hat_decay`.
    fn hat_decay(self, kit: &Kit) -> f64 {
        let base = match self {
            Self::HatPedal => 0.08,
            Self::HatOpen => 0.45,
            _ => 0.045,
        };
        base * kit.hat_decay
    }

    /// A crash, china or splash decay: `cymbal_decay * {crash: 1.0, china: 0.8, splash: 0.45}`.
    fn cymbal_decay(self, kit: &Kit) -> f64 {
        let factor = match self {
            Self::China => 0.8,
            Self::Splash => 0.45,
            _ => 1.0,
        };
        kit.cymbal_decay * factor
    }
}

/// The synthesis parameters (`_KitParams`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Kit {
    kick_tune: f64,
    kick_decay: f64,
    kick_punch: f64,
    snare_tune: f64,
    snare_tone: f64,
    snare_decay: f64,
    hat_tune: f64,
    hat_decay: f64,
    clap_decay: f64,
    tom_tune: f64,
    cymbal_decay: f64,
    drive: f64,
}

impl Kit {
    /// The longest samples: the descriptor's maxima of the parameters a sample length depends
    /// on (the other fields do not change a length).
    const LONGEST: Self = Self {
        kick_tune: 0.0,
        kick_decay: 2.0,
        kick_punch: 0.0,
        snare_tune: 0.0,
        snare_tone: 0.0,
        snare_decay: 1.0,
        hat_tune: 0.0,
        hat_decay: 3.0,
        clap_decay: 1.0,
        tom_tune: 0.0,
        cymbal_decay: 5.0,
        drive: 0.0,
    };

    /// Whether every sample fits the buffers sized for [`Kit::LONGEST`]: a length grows
    /// monotonically with its decay parameter. NaN does not fit.
    fn fits(&self) -> bool {
        let longest = &Self::LONGEST;
        self.kick_decay <= longest.kick_decay
            && self.snare_decay <= longest.snare_decay
            && self.hat_decay <= longest.hat_decay
            && self.clap_decay <= longest.clap_decay
            && self.cymbal_decay <= longest.cymbal_decay
    }
}

/// The decoded parameter set.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Settings {
    kit: Kit,
    kick_level: f64,
    snare_level: f64,
    hat_level: f64,
    cymbal_level: f64,
    perc_level: f64,
    width: f64,
    gain: f64,
}

impl Settings {
    fn from_slots(slots: &ParamSlots) -> Option<Self> {
        let kit = Kit {
            kick_tune: float(slots, KICK_TUNE)?,
            kick_decay: float(slots, KICK_DECAY)?,
            kick_punch: float(slots, KICK_PUNCH)?,
            snare_tune: float(slots, SNARE_TUNE)?,
            snare_tone: float(slots, SNARE_TONE)?,
            snare_decay: float(slots, SNARE_DECAY)?,
            hat_tune: float(slots, HAT_TUNE)?,
            hat_decay: float(slots, HAT_DECAY)?,
            clap_decay: float(slots, CLAP_DECAY)?,
            tom_tune: float(slots, TOM_TUNE)?,
            cymbal_decay: float(slots, CYMBAL_DECAY)?,
            drive: float(slots, DRIVE)?,
        };
        if !kit.fits() {
            return None;
        }
        Some(Self {
            kit,
            kick_level: db_to_amp(float(slots, KICK_LEVEL_DB)?),
            snare_level: db_to_amp(float(slots, SNARE_LEVEL_DB)?),
            hat_level: db_to_amp(float(slots, HAT_LEVEL_DB)?),
            cymbal_level: db_to_amp(float(slots, CYMBAL_LEVEL_DB)?),
            perc_level: db_to_amp(float(slots, PERC_LEVEL_DB)?),
            width: float(slots, STEREO_WIDTH)?,
            gain: db_to_amp(float(slots, GAIN_DB)?),
        })
    }

    fn level(&self, group: Group) -> f64 {
        match group {
            Group::Kick => self.kick_level,
            Group::Snare => self.snare_level,
            Group::Hat => self.hat_level,
            Group::Cymbal => self.cymbal_level,
            Group::Perc => self.perc_level,
        }
    }
}

/// A frame index or count as a float (exact).
#[expect(
    clippy::cast_precision_loss,
    reason = "sample indices stay far below 2^53, which converts exactly"
)]
fn index_f64(index: usize) -> f64 {
    index as f64
}

/// `int(seconds * sr)`: Python's truncation; a negative or NaN length is an empty sample.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Python's int() truncates toward zero; `as` saturates the rest to 0"
)]
fn frames(seconds: f64, sr: f64) -> usize {
    (seconds * sr) as usize
}

/// `_env`: `exp(-6.9 t / max(decay, 1e-3))`.
fn env(t: f64, decay: f64) -> f64 {
    (-6.9 * t / py_max(decay, 1e-3)).exp()
}

/// `signal.square(arg)` with a duty of 0.5: 1 on `[0, π)` of each period, else −1. The
/// argument is never negative, so NumPy's `%` is `fmod`.
fn square(arg: f64) -> f64 {
    if arg % TAU < PI { 1.0 } else { -1.0 }
}

/// A NumPy boolean mask as a factor: `True` is 1, `False` is 0.
fn mask(condition: bool) -> f64 {
    if condition { 1.0 } else { 0.0 }
}

/// `_sweep`, one sample at a time: the running `cumsum` of the gliding frequency.
struct Sweep {
    f0: f64,
    f1: f64,
    tau: f64,
    sr: f64,
    sum: Option<f64>,
}

impl Sweep {
    fn new(f0: f64, f1: f64, tau: f64, sr: f64) -> Self {
        Self {
            f0,
            f1,
            tau,
            sr,
            sum: None,
        }
    }

    /// `sin(2π cumsum(freq) / sr)` at `t`, the next sample.
    fn next(&mut self, t: f64) -> f64 {
        let freq = self.f1 + (self.f0 - self.f1) * (-t / self.tau).exp();
        let sum = self.sum.map_or(freq, |s| s + freq);
        self.sum = Some(sum);
        (TAU * sum / self.sr).sin()
    }
}

/// `_metal`: six square waves at `min(r · tune · 2, 0.45 sr)`, summed in ratio order and
/// divided by six.
struct Metal {
    /// `2π f` per ratio, formed before the product with `t` as NumPy does.
    omega: [f64; 6],
}

impl Metal {
    fn new(tune: f64, sr: f64) -> Self {
        let mut omega = [0.0; 6];
        for (w, &ratio) in omega.iter_mut().zip(&HAT_RATIOS) {
            *w = TAU * py_min(ratio * tune * 2.0, 0.45 * sr);
        }
        Self { omega }
    }

    fn at(&self, t: f64) -> f64 {
        let mut out = 0.0;
        for &w in &self.omega {
            out += square(w * t);
        }
        out / 6.0
    }
}

/// What a recipe works with: the rate, the sound's noise generator and the shared filter.
struct Synthesis<'a> {
    rate: SupportedRate,
    sr: f64,
    rng: Generator,
    filter: &'a mut SosFilter,
}

impl Synthesis<'_> {
    /// `np.arange(n)[i] / sr`.
    fn t(&self, i: usize) -> f64 {
        index_f64(i) / self.sr
    }

    /// `rng.standard_normal(len(x))` into `x`.
    fn noise(&mut self, x: &mut [f64]) {
        self.rng.standard_normal_fill(x);
    }

    /// `sosfilt(sections, x)` from zero state, in place. Every drum design fits the filter
    /// (unit test); one that did not would leave a silent sample.
    fn sosfilt(&mut self, sections: &[Section], x: &mut [f64]) {
        if self.filter.set_sections(sections).is_err() {
            x.fill(0.0);
            return;
        }
        self.filter.reset();
        self.filter.process_in_place(x);
    }

    fn highpass(&mut self, design: HighpassDesign, x: &mut [f64]) {
        self.sosfilt(butter_highpass(design, self.rate), x);
    }

    fn bandpass(&mut self, design: BandpassDesign, x: &mut [f64]) {
        self.sosfilt(butter_bandpass(design, self.rate), x);
    }

    fn kick(&mut self, kit: &Kit, x: &mut [f64]) {
        self.noise(x);
        self.highpass(HighpassDesign::KickClick, x);
        let f1 = kit.kick_tune;
        let tau = 0.012 + 0.03 * (1.0 - kit.kick_punch);
        let mut body = Sweep::new(f1 * (3.5 + 6.0 * kit.kick_punch), f1, tau, self.sr);
        let click = 0.25 + 0.6 * kit.kick_punch;
        for (i, v) in x.iter_mut().enumerate() {
            let t = self.t(i);
            let body = body.next(t) * env(t, kit.kick_decay);
            *v = body + *v * env(t, 0.004) * click;
        }
    }

    fn snare(&mut self, kit: &Kit, x: &mut [f64]) {
        self.noise(x);
        self.bandpass(BandpassDesign::Snare, x);
        let (w1, w2) = (TAU * 185.0 * kit.snare_tune, TAU * 330.0 * kit.snare_tune);
        let tone = kit.snare_tone;
        for (i, v) in x.iter_mut().enumerate() {
            let t = self.t(i);
            let body = ((w1 * t).sin() + 0.6 * (w2 * t).sin()) * env(t, 0.09);
            let noise = *v * env(t, kit.snare_decay);
            *v = (1.0 - tone) * body * 0.9 + tone * noise * 1.6 + 0.15 * body;
        }
    }

    fn rim(&mut self, x: &mut [f64]) {
        self.noise(x);
        self.bandpass(BandpassDesign::Rim, x);
        let w = TAU * 1700.0;
        for (i, v) in x.iter_mut().enumerate() {
            let t = self.t(i);
            *v = ((w * t).sin() * 0.7 + *v) * env(t, 0.02);
        }
    }

    fn clap(&mut self, kit: &Kit, x: &mut [f64]) {
        self.noise(x);
        self.bandpass(BandpassDesign::Clap, x);
        for (i, v) in x.iter_mut().enumerate() {
            let t = self.t(i);
            let mut envelope = 0.0;
            for offset in [0.0, 0.011, 0.022, 0.031] {
                envelope += mask(t >= offset) * env(maximum(t - offset, 0.0), 0.008);
            }
            envelope += mask(t >= 0.031) * 0.7 * env(maximum(t - 0.031, 0.0), kit.clap_decay);
            *v *= envelope;
        }
    }

    fn hat(&mut self, sound: Sound, kit: &Kit, x: &mut [f64]) {
        let decay = sound.hat_decay(kit);
        let metal = Metal::new(kit.hat_tune, self.sr);
        for (i, v) in x.iter_mut().enumerate() {
            *v = metal.at(self.t(i)) + 0.5 * self.rng.standard_normal();
        }
        let design = if sound == Sound::HatOpen {
            HighpassDesign::OpenHat
        } else {
            HighpassDesign::ClosedHat
        };
        self.highpass(design, x);
        for (i, v) in x.iter_mut().enumerate() {
            *v *= env(self.t(i), decay);
        }
    }

    fn cymbal(&mut self, sound: Sound, kit: &Kit, x: &mut [f64]) {
        let decay = sound.cymbal_decay(kit);
        let tune = kit.hat_tune * if sound == Sound::China { 0.8 } else { 1.0 };
        let metal = Metal::new(tune, self.sr);
        for (i, v) in x.iter_mut().enumerate() {
            *v = metal.at(self.t(i)) * 0.6 + self.rng.standard_normal();
        }
        self.highpass(HighpassDesign::Cymbal, x);
        for (i, v) in x.iter_mut().enumerate() {
            let t = self.t(i);
            *v = *v * env(t, decay) * (1.0 - (-t / 0.002).exp());
        }
    }

    fn ride(&mut self, sound: Sound, kit: &Kit, x: &mut [f64]) {
        let decay = kit.cymbal_decay * 1.2;
        let metal = Metal::new(kit.hat_tune * 0.7, self.sr);
        for (i, v) in x.iter_mut().enumerate() {
            *v = metal.at(self.t(i)) * 0.5 + 0.4 * self.rng.standard_normal();
        }
        self.highpass(HighpassDesign::Ride, x);
        let (ping, ping_gain, env_factor) = if sound == Sound::Ride {
            (3200.0, 0.25, 1.0)
        } else {
            (2300.0, 0.9, 0.6)
        };
        let w = TAU * ping;
        let decay = decay * env_factor;
        for (i, v) in x.iter_mut().enumerate() {
            let t = self.t(i);
            *v = (*v + (w * t).sin() * ping_gain) * env(t, decay);
        }
    }

    fn tom(&mut self, sound: Sound, kit: &Kit, x: &mut [f64]) {
        let base = match sound {
            Sound::TomFloorLow => 72.0,
            Sound::TomFloorHigh => 86.0,
            Sound::TomLow => 100.0,
            Sound::TomMidLow => 118.0,
            Sound::TomMidHigh => 140.0,
            _ => 165.0,
        } * kit.tom_tune;
        let mut sweep = Sweep::new(base * 1.8, base, 0.03, self.sr);
        for (i, v) in x.iter_mut().enumerate() {
            let t = self.t(i);
            *v = sweep.next(t) * env(t, 0.38) + 0.05 * self.rng.standard_normal() * env(t, 0.03);
        }
    }

    fn tambourine(&mut self, x: &mut [f64]) {
        self.noise(x);
        self.highpass(HighpassDesign::Tambourine, x);
        for (i, v) in x.iter_mut().enumerate() {
            let t = self.t(i);
            let later = 0.5 * mask(t > 0.03) * env(maximum(t - 0.03, 0.0), 0.1);
            *v *= env(t, 0.12) + later;
        }
    }

    fn cowbell(&mut self, x: &mut [f64]) {
        let (w1, w2) = (TAU * 540.0, TAU * 800.0);
        for (i, v) in x.iter_mut().enumerate() {
            let t = self.t(i);
            *v = square(w1 * t) + square(w2 * t);
        }
        self.bandpass(BandpassDesign::Cowbell, x);
        for (i, v) in x.iter_mut().enumerate() {
            *v *= env(self.t(i), 0.25);
        }
    }

    fn shaker(&mut self, x: &mut [f64]) {
        self.noise(x);
        self.highpass(HighpassDesign::Shaker, x);
        for (i, v) in x.iter_mut().enumerate() {
            // `np.sin(...) ** 2` is a square.
            let s = (PI * clip(self.t(i) / 0.09, 0.0, 1.0)).sin();
            *v *= s * s;
        }
    }

    /// Claves and wood blocks: a decaying sine.
    fn tone(&mut self, frequency: f64, decay: f64, x: &mut [f64]) {
        let w = TAU * frequency;
        for (i, v) in x.iter_mut().enumerate() {
            let t = self.t(i);
            *v = (w * t).sin() * env(t, decay);
        }
    }

    /// Congas and bongos: a short downward sweep.
    fn hand_drum(&mut self, sound: Sound, x: &mut [f64]) {
        let base = match sound {
            Sound::CongaLow => 190.0,
            Sound::CongaHigh => 280.0,
            Sound::CongaMute => 300.0,
            Sound::BongoLow => 330.0,
            _ => 420.0,
        };
        let decay = if sound == Sound::CongaMute {
            0.06
        } else {
            0.22
        };
        let mut sweep = Sweep::new(base * 1.3, base, 0.01, self.sr);
        for (i, v) in x.iter_mut().enumerate() {
            let t = self.t(i);
            *v = sweep.next(t) * env(t, decay);
        }
    }
}

/// `synthesize(name, kit, sr)` into `x`, which is exactly the sample's length.
fn synthesize(
    sound: Sound,
    kit: &Kit,
    rate: SupportedRate,
    seed: u64,
    filter: &mut SosFilter,
    x: &mut [f64],
) {
    let mut synthesis = Synthesis {
        rate,
        sr: f64::from(rate.hz()),
        rng: default_rng(seed),
        filter,
    };
    match sound {
        Sound::Kick => synthesis.kick(kit, x),
        Sound::Snare => synthesis.snare(kit, x),
        Sound::Rim => synthesis.rim(x),
        Sound::Clap => synthesis.clap(kit, x),
        Sound::HatClosed | Sound::HatPedal | Sound::HatOpen => synthesis.hat(sound, kit, x),
        Sound::Crash | Sound::China | Sound::Splash => synthesis.cymbal(sound, kit, x),
        Sound::Ride | Sound::RideBell => synthesis.ride(sound, kit, x),
        Sound::TomFloorLow
        | Sound::TomFloorHigh
        | Sound::TomLow
        | Sound::TomMidLow
        | Sound::TomMidHigh
        | Sound::TomHigh => synthesis.tom(sound, kit, x),
        Sound::Tambourine => synthesis.tambourine(x),
        Sound::Cowbell => synthesis.cowbell(x),
        Sound::Shaker => synthesis.shaker(x),
        Sound::Claves => synthesis.tone(2500.0, 0.04, x),
        Sound::BlockHigh => synthesis.tone(1800.0, 0.05, x),
        Sound::BlockLow => synthesis.tone(1200.0, 0.05, x),
        Sound::BongoHigh
        | Sound::BongoLow
        | Sound::CongaMute
        | Sound::CongaHigh
        | Sound::CongaLow => {
            synthesis.hand_drum(sound, x);
        }
    }
    finish(x, kit.drive);
}

/// The end of `synthesize`: the drive, the peak, the 64-sample fade-out, then `x / peak`.
fn finish(x: &mut [f64], drive: f64) {
    if drive > 0.0 {
        let k = 1.0 + 6.0 * drive;
        let norm = k.tanh();
        for v in x.iter_mut() {
            *v = (*v * k).tanh() / norm;
        }
    }
    // `float(np.max(np.abs(x))) or 1.0`; NumPy's max propagates NaN.
    let peak = x.iter().fold(0.0, |peak, v| maximum(peak, v.abs()));
    let peak = if peak == 0.0 { 1.0 } else { peak };
    let fade = x.len().min(FADE);
    let mut ramp = [0.0; FADE];
    if let Some(ramp) = ramp.get_mut(..fade) {
        linspace(1.0, 0.0, ramp);
    }
    if let Some(tail) = x.get_mut(x.len() - fade..) {
        for (v, &r) in tail.iter_mut().zip(&ramp) {
            *v *= r;
        }
    }
    for v in x.iter_mut() {
        *v /= peak;
    }
}

/// The samples of one kit at the prepared rate.
struct SampleBank {
    /// The rate the buffers are sized for, in Hz (0 before the first `prepare`).
    sample_rate: u32,
    rate: Option<SupportedRate>,
    /// One buffer per sound ([`Sound::ALL`] order), as long as the sound's longest sample.
    buffers: Vec<Vec<f64>>,
    /// The length of each sound's current sample; 0 for a sound not synthesized.
    lengths: [usize; SOUND_COUNT],
    /// The kit the samples were synthesized for.
    kit: Option<Kit>,
    /// The sounds synthesized for `kit`.
    synthesized: Sounds,
    filter: SosFilter,
    /// How many samples were synthesized (unit tests count the syntheses).
    #[cfg(test)]
    syntheses: usize,
}

impl SampleBank {
    /// Setup: buffers for the longest samples at `sample_rate` (none at an unsupported rate),
    /// all empty until [`synthesize`](Self::synthesize).
    #[expect(
        clippy::disallowed_methods,
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    fn new(sample_rate: u32) -> Self {
        let rate = SupportedRate::from_hz(sample_rate);
        let sr = f64::from(sample_rate);
        let mut buffers = Vec::with_capacity(SOUND_COUNT);
        for sound in Sound::ALL {
            let capacity = rate.map_or(0, |_| frames(sound.duration(&Kit::LONGEST), sr));
            buffers.push(vec![0.0; capacity]);
        }
        Self {
            sample_rate,
            rate,
            buffers,
            lengths: [0; SOUND_COUNT],
            kit: None,
            synthesized: Sounds::NONE,
            filter: SosFilter::new(1, MAX_SECTIONS),
            #[cfg(test)]
            syntheses: 0,
        }
    }

    /// Whether `sounds` of `kit` still need synthesizing: the samples are of another kit, or
    /// one of the sounds was not synthesized.
    fn lacks(&self, kit: &Kit, sounds: Sounds) -> bool {
        self.kit != Some(*kit) || !self.synthesized.covers(sounds)
    }

    /// Synthesizes `sounds` for `kit` into the prepared buffers and empties the other samples;
    /// allocation-free. A sample longer than its buffer (impossible for a kit that
    /// [`fits`](Kit::fits)) stays empty.
    fn synthesize(&mut self, kit: &Kit, seeds: &[u64; SOUND_COUNT], sounds: Sounds) {
        self.kit = Some(*kit);
        self.synthesized = sounds;
        let Some(rate) = self.rate else {
            self.lengths = [0; SOUND_COUNT];
            return;
        };
        let sr = f64::from(rate.hz());
        let all = Sound::ALL.iter().zip(seeds);
        let slots = self.buffers.iter_mut().zip(self.lengths.iter_mut());
        for ((&sound, &seed), (buffer, length)) in all.zip(slots) {
            let n = frames(sound.duration(kit), sr);
            let Some(x) = buffer.get_mut(..n).filter(|_| sounds.contains(sound)) else {
                *length = 0;
                continue;
            };
            synthesize(sound, kit, rate, seed, &mut self.filter, x);
            #[cfg(test)]
            {
                self.syntheses += 1;
            }
            *length = n;
        }
    }

    /// The current sample of `sound`.
    fn sample(&self, sound: Sound) -> &[f64] {
        let index = sound.index();
        let length = self.lengths.get(index).copied().unwrap_or(0);
        self.buffers
            .get(index)
            .and_then(|buffer| buffer.get(..length))
            .unwrap_or_default()
    }
}

/// One sounding hit (`_Hit`).
#[derive(Clone, Copy, Debug)]
struct Hit {
    note: u8,
    sound: Sound,
    /// Samples already played; negative while the hit starts later in the block.
    pos: i64,
    gain_l: f64,
    gain_r: f64,
    /// The fade level, which falls by `fade_step` per sample once the hit fades.
    level: f64,
    fade_step: f64,
}

impl Hit {
    /// An unused list slot.
    const EMPTY: Self = Self {
        note: 0,
        sound: Sound::Kick,
        pos: 0,
        gain_l: 0.0,
        gain_r: 0.0,
        level: 1.0,
        fade_step: 0.0,
    };
}

/// The drum kit (the prototype's `DrumKit`).
pub(crate) struct DrumKit {
    spec: ProcessSpec,
    /// The prepared block size; 0 before `prepare`.
    block_size: usize,
    params: ParamSlots,
    settings: Option<Settings>,
    /// `zlib.crc32(name)` of every sound, the seeds of their noise.
    seeds: [u64; SOUND_COUNT],
    /// The sounds of the playable notes (`DeviceState::playable_notes`), the ones synthesized.
    playable: Sounds,
    samples: SampleBank,
    /// The hit list, [`MAX_HITS`] long; the first `hit_count` are sounding.
    hits: Vec<Hit>,
    hit_count: usize,
    /// Unmapped notes since construction (`_ignored`; `reset` keeps it).
    ignored: u64,
}

impl DrumKit {
    /// Setup: an unprepared kit with its hit list; the samples are allocated by `prepare`.
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    pub(crate) fn new() -> Self {
        let mut seeds = [0; SOUND_COUNT];
        for (seed, sound) in seeds.iter_mut().zip(Sound::ALL) {
            *seed = u64::from(crc32fast::hash(sound.name().as_bytes()));
        }
        Self {
            spec: DEFAULT_SPEC,
            block_size: 0,
            params: ParamSlots::EMPTY,
            settings: None,
            seeds,
            playable: Sounds::ALL,
            samples: SampleBank::new(0),
            hits: vec![Hit::EMPTY; MAX_HITS],
            hit_count: 0,
            ignored: 0,
        }
    }

    /// The prototype's `on_params_changed`, with the playable samples synthesized eagerly when
    /// the kit changed (see the module docs); a set that does not decode changes nothing.
    fn apply_params(&mut self) {
        let Some(settings) = Settings::from_slots(&self.params) else {
            return;
        };
        if self.samples.lacks(&settings.kit, self.playable) {
            self.samples
                .synthesize(&settings.kit, &self.seeds, self.playable);
        }
        self.settings = Some(settings);
    }

    /// The hit list.
    fn active(&mut self) -> &mut [Hit] {
        self.hits.get_mut(..self.hit_count).unwrap_or_default()
    }

    /// Appends `hit`; a full list first drops its oldest hit.
    fn push(&mut self, hit: Hit) {
        if self.hit_count >= self.hits.len()
            && let Some(active) = self.hits.get_mut(..self.hit_count)
            && !active.is_empty()
        {
            active.rotate_left(1);
            self.hit_count -= 1;
        }
        if let Some(slot) = self.hits.get_mut(self.hit_count) {
            *slot = hit;
            self.hit_count += 1;
        }
    }

    /// `1 / max(1, int(seconds * sr))`: a fade's step per sample.
    fn fade_step(&self, seconds: f64) -> f64 {
        1.0 / index_f64(frames(seconds, f64::from(self.spec.sample_rate)).max(1))
    }

    /// A note-on with a velocity: a new hit, after the hi-hat choke.
    fn note_on(&mut self, event: &InstrumentEvent, s: &Settings) {
        let Some(sound) = Sound::from_note(event.pitch) else {
            self.ignored = self.ignored.saturating_add(1);
            return;
        };
        if matches!(event.pitch, 42 | 44) {
            let step = self.fade_step(CHOKE_SECONDS);
            for hit in self.active() {
                if hit.note == OPEN_HAT_NOTE && hit.fade_step == 0.0 {
                    hit.fade_step = step;
                }
            }
        }
        let level = s.level(sound.group()) * (f64::from(event.velocity) / 127.0).powf(1.3);
        let pan = sound.pan() * s.width;
        let theta = (pan + 1.0) * PI / 4.0;
        self.push(Hit {
            note: event.pitch,
            sound,
            pos: i64::try_from(event.offset).map_or(i64::MIN, |offset| -offset),
            gain_l: level * theta.cos() * SQRT_2,
            gain_r: level * theta.sin() * SQRT_2,
            ..Hit::EMPTY
        });
    }

    /// Adds every hit's part of this block to `out`, in list order, and drops the hits that
    /// ended.
    fn render(&mut self, out: &mut [[f64; 2]]) {
        let n = i64::try_from(out.len()).unwrap_or(i64::MAX);
        let mut kept = 0;
        for index in 0..self.hit_count {
            let Some(mut hit) = self.hits.get(index).copied() else {
                break;
            };
            let sample = self.samples.sample(hit.sound);
            let size = i64::try_from(sample.len()).unwrap_or(i64::MAX);
            let start = hit.pos.saturating_neg().max(0);
            let source = hit.pos.max(0);
            let count = (n - start).min(size - source);
            if count > 0 {
                // All three are non-negative, `start + count <= n` and `source + count <= size`.
                let [start, source, count] =
                    [start, source, count].map(|v| usize::try_from(v).unwrap_or(0));
                if let (Some(dst), Some(seg)) = (
                    out.get_mut(start..start + count),
                    sample.get(source..source + count),
                ) {
                    play(&mut hit, dst, seg);
                }
                if hit.fade_step > 0.0 && hit.level <= 0.0 {
                    hit.pos = size;
                }
            }
            if hit.pos < size {
                hit.pos = hit.pos.saturating_add(n);
            }
            if hit.pos < size {
                if let Some(slot) = self.hits.get_mut(kept) {
                    *slot = hit;
                }
                kept += 1;
            }
        }
        self.hit_count = kept;
    }
}

/// One hit's segment `seg` into `dst`, faded when the hit fades: `clip(level - step · k, 0, 1)`
/// for `k = 1..=count`, after which the level is the last ramp value.
fn play(hit: &mut Hit, dst: &mut [[f64; 2]], seg: &[f64]) {
    if hit.fade_step > 0.0 {
        let level = hit.level;
        let mut last = level;
        for (k, ([l, r], &x)) in dst.iter_mut().zip(seg).enumerate() {
            let ramp = clip(level - hit.fade_step * index_f64(k + 1), 0.0, 1.0);
            let v = x * ramp;
            *l += v * hit.gain_l;
            *r += v * hit.gain_r;
            last = ramp;
        }
        hit.level = last;
    } else {
        for ([l, r], &x) in dst.iter_mut().zip(seg) {
            *l += x * hit.gain_l;
            *r += x * hit.gain_r;
        }
    }
}

impl Processor for DrumKit {
    fn prepare(&mut self, spec: ProcessSpec) {
        self.spec = spec;
        self.block_size = spec.block_size;
        // The samples depend on the rate only, so they survive a prepare at the same rate,
        // like the prototype's cache survives its reset.
        if self.samples.sample_rate != spec.sample_rate {
            self.samples = SampleBank::new(spec.sample_rate);
        }
        self.reset();
        self.apply_params();
    }

    fn set_params(&mut self, params: &ParamSlots) {
        if store_params(&mut self.params, params) {
            self.apply_params();
        }
    }

    /// Setup: adopts the playable notes and synthesizes the sounds that became playable (see
    /// the module docs).
    fn set_state(&mut self, state: &DeviceState) {
        self.playable = Sounds::of_notes(state.playable_notes);
        self.apply_params();
    }

    fn process(&mut self, buffer: &mut [[f64; 2]], ctx: &BlockContext<'_>) {
        let n = buffer.len().min(self.block_size);
        let Some(out) = buffer.get_mut(..n) else {
            return;
        };
        // An instrument ignores its input (`out = np.zeros((n, 2))`).
        out.fill([0.0; 2]);
        let Some(s) = self.settings else {
            return;
        };
        for event in ctx.events {
            match event.kind {
                EventKind::NoteOn if event.velocity > 0 => self.note_on(event, &s),
                EventKind::AllNotesOff => {
                    let step = self.fade_step(ALL_NOTES_OFF_SECONDS);
                    for hit in self.active() {
                        if hit.fade_step == 0.0 {
                            hit.fade_step = step;
                        }
                    }
                }
                _ => {}
            }
        }
        self.render(out);
        for frame in out.iter_mut() {
            *frame = scale(*frame, s.gain);
        }
    }

    fn reset(&mut self) {
        self.hit_count = 0;
    }

    fn tail_seconds(&self) -> f64 {
        self.settings.map_or(2.0, |s| s.kit.cymbal_decay * 1.6)
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "the counts stay far below 2^53, which converts exactly"
    )]
    fn report(&self) -> Report {
        Report::new(&[
            ("active_hits", index_f64(self.hit_count)),
            ("ignored_notes", self.ignored as f64),
        ])
    }
}

#[cfg(test)]
mod tests {
    use sonara_core::model::device::{ParamValue, Params};
    use sonara_core::plugins::builtin_registry;
    use sonara_testkit::Golden;

    use super::*;
    use crate::processors::param_slots;

    const SR: u32 = 48_000;
    const BLOCK: usize = 512;

    fn slots(values: &[(&str, ParamValue)]) -> ParamSlots {
        let registry = builtin_registry();
        let values: Params = values
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        param_slots(registry.get(PLUGIN_ID).unwrap(), &values).unwrap()
    }

    fn prepared(sample_rate: u32, values: &[(&str, ParamValue)]) -> DrumKit {
        let mut kit = DrumKit::new();
        kit.prepare(ProcessSpec {
            sample_rate,
            block_size: BLOCK,
            channels: 2,
        });
        kit.set_params(&slots(values));
        kit
    }

    fn note(offset: usize, pitch: u8, velocity: u8) -> InstrumentEvent {
        InstrumentEvent {
            offset,
            kind: EventKind::NoteOn,
            pitch,
            velocity,
            channel: 9,
            value: 0,
            number: 0,
        }
    }

    fn block(kit: &mut DrumKit, events: &[InstrumentEvent]) -> [[f64; 2]; BLOCK] {
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
        kit.process(&mut out, &ctx);
        out
    }

    #[test]
    fn slots_follow_the_descriptor_order() {
        let registry = builtin_registry();
        let params = &registry.get(PLUGIN_ID).unwrap().params;
        let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
        let slots = [
            (KICK_TUNE, "kick_tune"),
            (KICK_DECAY, "kick_decay"),
            (KICK_PUNCH, "kick_punch"),
            (SNARE_TUNE, "snare_tune"),
            (SNARE_TONE, "snare_tone"),
            (SNARE_DECAY, "snare_decay"),
            (HAT_TUNE, "hat_tune"),
            (HAT_DECAY, "hat_decay"),
            (CLAP_DECAY, "clap_decay"),
            (TOM_TUNE, "tom_tune"),
            (CYMBAL_DECAY, "cymbal_decay"),
            (DRIVE, "drive"),
            (KICK_LEVEL_DB, "kick_level_db"),
            (SNARE_LEVEL_DB, "snare_level_db"),
            (HAT_LEVEL_DB, "hat_level_db"),
            (CYMBAL_LEVEL_DB, "cymbal_level_db"),
            (PERC_LEVEL_DB, "perc_level_db"),
            (STEREO_WIDTH, "stereo_width"),
            (GAIN_DB, "gain_db"),
        ];
        assert_eq!(ids.len(), slots.len());
        for (index, id) in slots {
            assert_eq!(ids.get(index), Some(&id));
        }
        // The longest samples use the descriptor's maxima.
        let max = |index: usize| params.get(index).unwrap().maximum.unwrap();
        let longest = Kit::LONGEST;
        assert_eq!(longest.kick_decay, max(KICK_DECAY));
        assert_eq!(longest.snare_decay, max(SNARE_DECAY));
        assert_eq!(longest.hat_decay, max(HAT_DECAY));
        assert_eq!(longest.clap_decay, max(CLAP_DECAY));
        assert_eq!(longest.cymbal_decay, max(CYMBAL_DECAY));
    }

    #[test]
    fn the_note_map_is_the_gm_map() {
        let mut mapped = 0;
        for note in 0..=u8::MAX {
            let expected = sonara_midi::gm::GM_MAP
                .iter()
                .find(|(n, _)| *n == note)
                .map(|(_, name)| *name);
            assert_eq!(Sound::from_note(note).map(Sound::name), expected, "{note}");
            mapped += usize::from(expected.is_some());
        }
        assert_eq!(mapped, sonara_midi::gm::GM_MAP.len());
        for (index, sound) in Sound::ALL.iter().enumerate() {
            assert_eq!(sound.index(), index);
            assert!(
                sonara_midi::gm::GM_MAP
                    .iter()
                    .any(|(_, n)| *n == sound.name())
            );
        }
    }

    #[test]
    fn the_seeds_are_the_recorded_crc32_of_the_names() {
        let golden = Golden::open().unwrap();
        let kit = DrumKit::new();
        for (sound, seed) in Sound::ALL.iter().zip(kit.seeds) {
            let header: serde_json::Value = golden
                .json(&["numpy/drums/", sound.name(), ".json"].concat())
                .unwrap();
            let recorded = header.get("seed").and_then(serde_json::Value::as_u64);
            assert_eq!(Some(seed), recorded, "{}", sound.name());
        }
    }

    #[test]
    fn every_design_fits_the_filter() {
        for rate in SupportedRate::ALL {
            for design in [
                BandpassDesign::Snare,
                BandpassDesign::Rim,
                BandpassDesign::Clap,
                BandpassDesign::Cowbell,
            ] {
                assert!(butter_bandpass(design, rate).len() <= MAX_SECTIONS);
            }
            for design in [
                HighpassDesign::KickClick,
                HighpassDesign::ClosedHat,
                HighpassDesign::OpenHat,
                HighpassDesign::Cymbal,
                HighpassDesign::Ride,
                HighpassDesign::Tambourine,
                HighpassDesign::Shaker,
            ] {
                assert!(butter_highpass(design, rate).len() <= MAX_SECTIONS);
            }
        }
    }

    #[test]
    fn samples_are_normalized_and_end_at_zero() {
        let kit = prepared(SR, &[]);
        let longest = 42.02 * f64::from(SR);
        let capacity: usize = kit.samples.buffers.iter().map(Vec::len).sum();
        assert!((index_f64(capacity) - longest).abs() < 30.0, "{capacity}");
        for sound in Sound::ALL {
            let sample = kit.samples.sample(sound);
            let peak = sample.iter().fold(0.0_f64, |p, v| p.max(v.abs()));
            assert!(peak > 0.9 && peak <= 1.0, "{}: peak {peak}", sound.name());
            assert_eq!(sample.last(), Some(&0.0), "{}", sound.name());
        }
    }

    #[test]
    fn hits_are_panned_and_counted() {
        let mut kit = prepared(SR, &[("gain_db", ParamValue::Float(0.0))]);
        let out = block(&mut kit, &[note(10, 36, 127), note(20, 58, 100)]);
        let (before, kick) = out.split_at(10);
        assert!(
            before.iter().flatten().all(|v| *v == 0.0),
            "the kick starts at its offset"
        );
        // The kick is centred: both gains are cos(π/4)·√2.
        assert!(kick.iter().all(|[l, r]| (l - r).abs() < 1e-12));
        assert!(kick.iter().any(|[l, _]| l.abs() > 0.1));
        let report = kit.report();
        assert_eq!(
            report.entries(),
            [("active_hits", 1.0), ("ignored_notes", 1.0)]
        );
        kit.reset();
        assert_eq!(
            kit.report().entries().get(1),
            Some(&("ignored_notes", 1.0)),
            "reset keeps it"
        );
    }

    #[test]
    fn the_choke_fades_from_the_block_start() {
        let mut free = prepared(SR, &[]);
        block(&mut free, &[note(0, 46, 100)]);
        let unchoked = block(&mut free, &[]);
        let mut kit = prepared(SR, &[]);
        block(&mut kit, &[note(0, 46, 100)]);
        // The closed hat at offset 300 chokes the open hat from the block's first sample, with
        // a 10 ms (480-sample) fade.
        let choked = block(&mut kit, &[note(300, 42, 100)]);
        for (k, (c, u)) in choked.iter().zip(&unchoked).take(300).enumerate() {
            let ramp = (1.0 - index_f64(k + 1) / 480.0).max(0.0);
            assert!((c[0] - u[0] * ramp).abs() < 1e-12, "{k}");
        }
        assert_eq!(kit.hit_count, 1, "the open hat ended inside the block");
        assert_eq!(kit.hits.first().map(|h| h.note), Some(42));
    }

    #[test]
    fn all_notes_off_fades_every_hit() {
        let mut kit = prepared(SR, &[]);
        block(&mut kit, &[note(0, 49, 100), note(0, 51, 100)]);
        let off = InstrumentEvent {
            kind: EventKind::AllNotesOff,
            ..note(0, 0, 0)
        };
        block(&mut kit, &[off]);
        // 5 ms is 240 samples, shorter than the block.
        assert_eq!(kit.hit_count, 0);
    }

    #[test]
    fn a_full_list_drops_the_oldest_hit() {
        let mut kit = prepared(SR, &[]);
        let hits: Vec<InstrumentEvent> = (0..=MAX_HITS).map(|i| note(i % BLOCK, 49, 100)).collect();
        block(&mut kit, &hits);
        assert_eq!(kit.hit_count, MAX_HITS);
        // The first hit (offset 0) was dropped; the oldest left started at offset 1.
        let oldest = kit.hits.first().map(|h| h.pos);
        assert_eq!(oldest, Some(i64::try_from(BLOCK).unwrap() - 1));
    }

    #[test]
    fn an_unsupported_rate_is_silent() {
        let mut kit = prepared(50_000, &[]);
        let out = block(&mut kit, &[note(0, 36, 127)]);
        assert!(out.iter().flatten().all(|v| *v == 0.0));
        assert_eq!(kit.hit_count, 0);
    }

    #[test]
    fn kit_changes_resynthesize_and_out_of_range_kits_are_ignored() {
        let mut kit = prepared(SR, &[]);
        let before = kit.samples.sample(Sound::Kick).len();
        kit.set_params(&slots(&[("kick_decay", ParamValue::Float(1.0))]));
        assert!(kit.samples.sample(Sound::Kick).len() > before);
        assert_eq!(kit.samples.kit.map(|k| k.kick_decay), Some(1.0));
        let mut too_long = slots(&[]);
        too_long.set(CYMBAL_DECAY, crate::processor::ParamSlot::Float(6.0));
        kit.set_params(&too_long);
        assert_eq!(kit.settings.map(|s| s.kit.cymbal_decay), Some(1.6));
        assert_eq!(kit.tail_seconds(), 1.6 * 1.6);
    }

    fn spec(sample_rate: u32) -> ProcessSpec {
        ProcessSpec {
            sample_rate,
            block_size: BLOCK,
            channels: 2,
        }
    }

    fn playable(notes: &[u8]) -> DeviceState {
        DeviceState {
            playable_notes: notes.iter().copied().collect(),
            ..DeviceState::default()
        }
    }

    /// A kit set up like the render graph does it: prepare, `set_state`, `set_params`.
    fn built(state: &DeviceState, values: &[(&str, ParamValue)]) -> DrumKit {
        let mut kit = DrumKit::new();
        kit.prepare(spec(SR));
        kit.set_state(state);
        kit.set_params(&slots(values));
        kit
    }

    /// A note that plays `sound`.
    fn note_of(sound: Sound) -> u8 {
        (0..=127)
            .find(|&p| Sound::from_note(p) == Some(sound))
            .unwrap()
    }

    #[test]
    fn notes_map_to_their_sounds() {
        assert_eq!(Sounds::of_notes(NoteSet::ALL), Sounds::ALL);
        assert_eq!(Sounds::of_notes(NoteSet::EMPTY), Sounds::NONE);
        let kicks = Sounds::of_notes([35, 36, 0, 127].into_iter().collect());
        let members: Vec<Sound> = Sound::ALL
            .into_iter()
            .filter(|&s| kicks.contains(s))
            .collect();
        assert_eq!(members, [Sound::Kick], "unmapped notes have no sound");
        assert!(Sounds::ALL.covers(kicks) && !kicks.covers(Sounds::ALL));
    }

    #[test]
    fn only_the_playable_sounds_are_synthesized() {
        let mut kit = built(&playable(&[36, 42, 44]), &[]);
        assert_eq!(kit.samples.syntheses, 3);
        for sound in Sound::ALL {
            let expected = matches!(sound, Sound::Kick | Sound::HatClosed | Sound::HatPedal);
            assert_eq!(
                !kit.samples.sample(sound).is_empty(),
                expected,
                "{}",
                sound.name()
            );
        }
        // The reset before an offline render (a second prepare at the same rate) keeps them.
        kit.prepare(spec(SR));
        assert_eq!(kit.samples.syntheses, 3);
        // A kit change synthesizes the playable sounds only.
        kit.set_params(&slots(&[("kick_decay", ParamValue::Float(1.0))]));
        assert_eq!(kit.samples.syntheses, 6);
        assert_eq!(kit.samples.sample(Sound::Snare), [0.0_f64; 0]);
        // A note outside the set plays silence and leaves no hit.
        let out = block(&mut kit, &[note(0, 38, 127)]);
        assert!(out.iter().flatten().all(|v| *v == 0.0));
        assert_eq!(kit.report().entries().first(), Some(&("active_hits", 0.0)));
        // Widening the set synthesizes the sounds that became playable.
        kit.set_state(&DeviceState::default());
        assert!(
            Sound::ALL
                .iter()
                .all(|&s| !kit.samples.sample(s).is_empty())
        );
        assert_eq!(kit.samples.syntheses, 6 + SOUND_COUNT);
    }

    #[test]
    fn a_sample_does_not_depend_on_the_other_sounds() {
        let defaults: [(&str, ParamValue); 0] = [];
        let tuned = [
            ("drive", ParamValue::Float(0.4)),
            ("hat_tune", ParamValue::Float(1.3)),
            ("cymbal_decay", ParamValue::Float(0.8)),
        ];
        for values in [&defaults[..], &tuned[..]] {
            let full = built(&DeviceState::default(), values);
            assert_eq!(full.samples.syntheses, SOUND_COUNT);
            for sound in Sound::ALL {
                let alone = built(&playable(&[note_of(sound)]), values);
                assert_eq!(alone.samples.syntheses, 1, "{}", sound.name());
                let bits = |kit: &DrumKit| -> Vec<u64> {
                    kit.samples
                        .sample(sound)
                        .iter()
                        .map(|v| v.to_bits())
                        .collect()
                };
                assert_ne!(bits(&alone), [0_u64; 0]);
                assert_eq!(bits(&alone), bits(&full), "{}", sound.name());
            }
        }
    }

    #[test]
    fn a_prepare_at_the_same_rate_keeps_the_samples() {
        let mut kit = prepared(SR, &[]);
        assert_eq!(kit.samples.syntheses, SOUND_COUNT);
        let kick: Vec<f64> = kit.samples.sample(Sound::Kick).to_vec();
        block(&mut kit, &[note(0, 36, 127)]);
        kit.prepare(spec(SR));
        assert_eq!(
            kit.samples.syntheses, SOUND_COUNT,
            "nothing synthesized again"
        );
        assert_eq!(kit.samples.sample(Sound::Kick), kick.as_slice());
        assert_eq!(kit.hit_count, 0, "the prepare reset the hits");
        // Another rate: a new bank, synthesized for it (the default kick decay is 0.45 s).
        kit.prepare(spec(44_100));
        assert_eq!(kit.samples.sample_rate, 44_100);
        assert_eq!(kit.samples.syntheses, SOUND_COUNT);
        assert_eq!(
            kit.samples.sample(Sound::Kick).len(),
            frames(0.45 * 1.6 + 0.05, 44_100.0)
        );
    }
}
