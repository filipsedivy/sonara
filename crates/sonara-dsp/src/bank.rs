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

/// One decoded sample: interleaved `f32` frames at the file's own rate.
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
    /// SFZ opcodes the patch uses that the sampler does not play, sorted, without duplicates.
    pub ignored_opcodes: Vec<String>,
}
