//! Real-time: the bank is read on the audio render thread (see docs/audio-engine.md); building
//! one is setup and runs off it.
//!
//! The data model of the sample-based instrument `sonara.sampler`: decoded samples and the
//! regions that map notes, velocities and sequence positions onto them. It is the engine-neutral
//! result of reading an SFZ patch (`sonara-media` does the reading, off the real-time thread)
//! and the input of the sampler processor. A bank is immutable once built and shared through
//! an [`Arc`](std::sync::Arc), so the render thread never frees or grows it (RT-02, RT-09).

/// What starts a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Trigger {
    /// A key press (SFZ `trigger=attack`, the default).
    #[default]
    Attack,
    /// A key release (SFZ `trigger=release`): the sample of the released note's tail.
    Release,
}

/// How a region plays after it started (SFZ `loop_mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoopMode {
    /// Plays to its end, or until the key is released.
    #[default]
    NoLoop,
    /// Plays to its end whatever the key does.
    OneShot,
    /// Loops between the loop points while it sounds.
    Continuous,
    /// Loops between the loop points while the key is held.
    Sustain,
}

/// The shape of a velocity crossfade (SFZ `xf_velcurve`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrossfadeCurve {
    /// Equal power: the gain is the square root of the position in the fade, so two layers that
    /// fade across each other keep their summed power (SFZ `power`, the default).
    #[default]
    Power,
    /// Linear gain: the gain is the position in the fade, so two phase-aligned layers keep their
    /// summed amplitude (SFZ `gain`).
    Gain,
}

impl CrossfadeCurve {
    /// The gain at `position` in a fade, from 0 (silent) to 1 (full).
    fn gain(self, position: f64) -> f64 {
        match self {
            Self::Power => position.sqrt(),
            Self::Gain => position,
        }
    }
}

/// How a region fades in and out with the velocity of the note that starts it (SFZ
/// `xfin_lovel`/`xfin_hivel`, `xfout_lovel`/`xfout_hivel` and `xf_velcurve`), as the SFZ format
/// describes it; the gains match the reference player sfizz's tests.
///
/// The fade-in is silent at velocities at or below `in_lo` and full at or above `in_hi`; the
/// fade-out is full at or below `out_lo` and silent at or above `out_hi`. In between, the gain
/// follows `curve` over the position in the fade, `(velocity - lo) / (hi - lo)`. Where the two
/// bounds of a fade meet or cross, the low bound decides: such a fade-in is silent below `in_lo`
/// and full from it, and such a fade-out is full up to `out_lo` and silent above it. So
/// `xfin_lovel=70` alone starts the region at velocity 70, and `xfout_hivel=90` alone, whose
/// `xfout_lovel` stays 127, fades nothing, as in sfizz.
///
/// Two edges keep the SFZ format's bounds where sfizz differs: a fade whose high bound is 127
/// reaches its end at 127 (sfizz ends it one velocity early), and a fade-out of no width is silent
/// above its velocity (sfizz fades nothing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VelocityCrossfade {
    /// The velocity at and below which the fade-in is silent (SFZ `xfin_lovel`, default 0).
    pub in_lo: u8,
    /// The velocity from which the fade-in is full (SFZ `xfin_hivel`, default 0).
    pub in_hi: u8,
    /// The velocity up to which the fade-out is full (SFZ `xfout_lovel`, default 127).
    pub out_lo: u8,
    /// The velocity from which the fade-out is silent (SFZ `xfout_hivel`, default 127).
    pub out_hi: u8,
    /// The shape of both fades (SFZ `xf_velcurve`).
    pub curve: CrossfadeCurve,
}

impl VelocityCrossfade {
    /// No crossfade, the SFZ defaults: full at every velocity.
    pub const NONE: Self = Self {
        in_lo: 0,
        in_hi: 0,
        out_lo: 127,
        out_hi: 127,
        curve: CrossfadeCurve::Power,
    };

    /// The gain of the crossfade at `velocity`, from 0 (silent) to 1 (full): the fade-in's gain
    /// times the fade-out's.
    #[must_use]
    pub fn gain(self, velocity: u8) -> f64 {
        self.fade_in(velocity) * self.fade_out(velocity)
    }

    fn fade_in(self, velocity: u8) -> f64 {
        let (lo, hi) = (self.in_lo, self.in_hi);
        if velocity < lo {
            0.0
        } else if velocity >= hi {
            1.0
        } else {
            // lo <= velocity < hi: the fade spans at least one step.
            self.curve.gain(position(velocity, lo, hi))
        }
    }

    fn fade_out(self, velocity: u8) -> f64 {
        let (lo, hi) = (self.out_lo, self.out_hi);
        if velocity <= lo {
            1.0
        } else if velocity >= hi {
            0.0
        } else {
            // lo < velocity < hi: the fade spans at least two steps.
            self.curve.gain(1.0 - position(velocity, lo, hi))
        }
    }
}

impl Default for VelocityCrossfade {
    fn default() -> Self {
        Self::NONE
    }
}

/// Where `velocity` lies between `lo` (0) and `hi` (1); the callers keep `lo <= velocity < hi`,
/// so the divisor is positive.
fn position(velocity: u8, lo: u8, hi: u8) -> f64 {
    (f64::from(velocity) - f64::from(lo)) / (f64::from(hi) - f64::from(lo))
}

/// One decoded sample: interleaved `f32` frames at the file's own rate.
///
/// A sample without frames plays nothing: a region that names one starts no voice, but its note
/// still switches off the exclusive group it belongs to (SFZ `sample=*silence`).
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// Sample rate of the file in Hz.
    pub rate: u32,
    /// Channels per frame: 1 or 2.
    pub channels: usize,
    /// Interleaved samples.
    pub data: Vec<f32>,
}

impl Sample {
    /// Frames in the sample.
    #[must_use]
    pub fn frames(&self) -> usize {
        self.data.len().checked_div(self.channels).unwrap_or(0)
    }
}

/// One SFZ region: the conditions that select it and how it plays.
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    /// Index into [`SampleBank::samples`].
    pub sample: usize,
    /// Lowest MIDI key that plays the region.
    pub key_lo: u8,
    /// Highest MIDI key that plays the region.
    pub key_hi: u8,
    /// Whether a key starts the region: a press for an attack region, a release for a
    /// release region. `false` when the patch writes a key of -1 ("no key"), or when a
    /// controller starts the region instead (SFZ `on_loccN`/`on_hiccN`). A flag rather than an
    /// empty key range, so the range keeps its meaning (TYP-03).
    pub on_key: bool,
    /// Lowest velocity.
    pub vel_lo: u8,
    /// Highest velocity.
    pub vel_hi: u8,
    /// The key at which the sample plays at its original pitch.
    pub pitch_keycenter: u8,
    /// Fine tuning in cents.
    pub tune_cents: f64,
    /// Region volume in dB.
    pub volume_db: f64,
    /// How much the gain follows the note's velocity, in percent (SFZ `amp_veltrack`, -100 to
    /// 100, default 100); see [`Region::velocity_gain`].
    pub amp_veltrack: f64,
    /// How the region fades in and out with the note's velocity.
    pub velocity_crossfade: VelocityCrossfade,
    /// What starts the region.
    pub trigger: Trigger,
    /// Round-robin length (SFZ `seq_length`); 1 means no round robin.
    pub seq_length: u32,
    /// Position in the round robin, from 1 (SFZ `seq_position`).
    pub seq_position: u32,
    /// Random range start, inclusive (SFZ `lorand`).
    pub rand_lo: f64,
    /// Random range end, exclusive (SFZ `hirand`).
    pub rand_hi: f64,
    /// Keyswitch that must be the last one pressed (SFZ `sw_last`).
    pub sw_last: Option<u8>,
    /// Exclusive group this region belongs to; 0 for none (SFZ `group`).
    pub group: u32,
    /// Group whose start silences this region's voices; 0 for none (SFZ `off_by`).
    pub off_by: u32,
    /// Looping behaviour.
    pub loop_mode: LoopMode,
    /// First looped frame.
    pub loop_start: usize,
    /// Frame after the last looped one.
    pub loop_end: usize,
    /// First frame played (SFZ `offset`).
    pub offset: usize,
    /// Frame after the last one played (SFZ `end`).
    pub end: usize,
    /// Attack time in seconds.
    pub attack: f64,
    /// Release time in seconds.
    pub release: f64,
}

impl Default for Region {
    fn default() -> Self {
        Self {
            sample: 0,
            key_lo: 0,
            key_hi: 127,
            on_key: true,
            vel_lo: 0,
            vel_hi: 127,
            pitch_keycenter: 60,
            tune_cents: 0.0,
            volume_db: 0.0,
            amp_veltrack: 100.0,
            velocity_crossfade: VelocityCrossfade::NONE,
            trigger: Trigger::Attack,
            seq_length: 1,
            seq_position: 1,
            rand_lo: 0.0,
            rand_hi: 1.0,
            sw_last: None,
            group: 0,
            off_by: 0,
            loop_mode: LoopMode::NoLoop,
            loop_start: 0,
            loop_end: 0,
            offset: 0,
            end: 0,
            attack: 0.0,
            release: 0.0,
        }
    }
}

impl Region {
    /// The gain a note of `velocity` (1 to 127) gives the region, from 0 to 1, before its
    /// volume: its velocity tracking times its velocity crossfade
    /// ([`VelocityCrossfade::gain`]).
    ///
    /// Velocity tracking (`amp_veltrack`) follows the SFZ format and the reference player
    /// sfizz: with `v = velocity / 127` and `t = amp_veltrack / 100`, the gain is
    /// `1 - t * (1 - v²)` for `t >= 0`, so 100 % is the curve `v²` and 0 % ignores the velocity,
    /// and `|t| * (1 - v²)` for `t < 0`, so -100 % makes velocity 127 silent and the softest notes
    /// loudest. A value outside -100 to 100 counts as the nearest end, and one that is not a
    /// number as 100.
    #[must_use]
    pub fn velocity_gain(&self, velocity: u8) -> f64 {
        self.tracking_gain(velocity) * self.velocity_crossfade.gain(velocity)
    }

    /// The velocity tracking part of [`Region::velocity_gain`].
    fn tracking_gain(&self, velocity: u8) -> f64 {
        let v = f64::from(velocity.min(127)) / 127.0;
        let curve = v * v;
        let track = if self.amp_veltrack.is_nan() {
            1.0
        } else {
            (self.amp_veltrack / 100.0).clamp(-1.0, 1.0)
        };
        if track >= 0.0 {
            // `(1 - t) + t * v²` is `1 - t * (1 - v²)`; at 100 % it is exactly `v²`.
            (1.0 - track) + track * curve
        } else {
            -track * (1.0 - curve)
        }
    }
}

/// The keyswitches of a patch: the keys that select an articulation (SFZ `sw_lokey`/`sw_hikey`)
/// and the switch that is active before any of them is pressed (SFZ `sw_default`).
///
/// The keys are a set, not one range: when the groups of a patch declare disjoint ranges, only
/// the keys inside them switch, and the keys between them stay playable notes. A patch without
/// keyswitches has no keys and no default ([`Keyswitches::NONE`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keyswitches {
    /// Whether each MIDI key is a keyswitch.
    keys: [bool; 128],
    /// The keyswitch that is active before any key is pressed (SFZ `sw_default`), with or
    /// without keyswitch keys.
    pub default: Option<u8>,
}

impl Keyswitches {
    /// No keyswitch keys and no default switch.
    pub const NONE: Self = Self {
        keys: [false; 128],
        default: None,
    };

    /// Setup: makes the keys `lo..=hi` keyswitches. An inverted range adds no key; values above
    /// 127 are no MIDI keys and are left out.
    pub fn add_range(&mut self, lo: u8, hi: u8) {
        for key in lo..=hi {
            if let Some(slot) = self.keys.get_mut(usize::from(key)) {
                *slot = true;
            }
        }
    }

    /// Whether a press of `key` selects an articulation.
    #[must_use]
    pub fn contains(&self, key: u8) -> bool {
        self.keys.get(usize::from(key)).copied().unwrap_or(false)
    }

    /// The keyswitch keys, ascending.
    pub fn keys(&self) -> impl Iterator<Item = u8> + '_ {
        (0..=127_u8).filter(|key| self.contains(*key))
    }
}

impl Default for Keyswitches {
    fn default() -> Self {
        Self::NONE
    }
}

/// A loaded instrument: its samples and regions. Immutable and shared.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SampleBank {
    /// The decoded samples.
    pub samples: Vec<Sample>,
    /// The regions, in patch order.
    pub regions: Vec<Region>,
    /// The patch's keyswitches; [`Keyswitches::NONE`] when it has none.
    pub keyswitches: Keyswitches,
    /// How long the instrument can keep sounding after the last key release, in seconds: the
    /// longest release and the longest release-trigger sample.
    pub tail_seconds: f64,
    /// What the patch writes that the sampler does not play, sorted, without duplicates: the
    /// names of unsupported SFZ opcodes (a numbered family as one entry, `locc*`), the values of
    /// supported opcodes it plays as if they were absent (`trigger=legato`), and the headers
    /// whose opcodes are skipped (`<curve>`).
    pub ignored_opcodes: Vec<String>,
    /// The regions the loader left out and why, one note per sample they name, in the order the
    /// patch first names it: a sample file that does not exist, or a generator the sampler does
    /// not play (SFZ `*sine`). Each note counts the regions it left out. Empty when every region
    /// was loaded.
    pub load_notes: Vec<String>,
}
