//! Real-time: the bank is read on the audio render thread (see docs/audio-engine.md); building
//! one is setup and runs off it.
//!
//! The data model of the sample-based instrument `sonara.sampler`: decoded samples and the
//! regions that map notes, velocities and sequence positions onto them. It is the engine-neutral
//! result of reading an SFZ patch (`sonara-media` does the reading, off the real-time thread)
//! and the input of the sampler processor. A bank is immutable once built and shared through
//! an [`Arc`], so the render thread never frees or grows it (RT-02, RT-09).
//!
//! A sampler device receives its bank inside a [`SamplerSetup`], beside the [`SamplerMapping`]
//! of the preset it plays: what each choice of its `articulation` parameter selects in the
//! patch, what its `dynamics` parameter drives, how the preset plays legato and how far incoming
//! notes are moved. The application compiles the mapping from what the library's manifest or
//! Sonara's profile says about the preset, off the real-time thread; the render thread only
//! looks a choice up ([`SamplerMapping::target`]).

use std::sync::Arc;

use crate::filters::db_to_amp;

/// What starts a region (SFZ `trigger`), as the SFZ format and the reference player sfizz read
/// it. "Another key down" counts the keys held down that are no keyswitch keys (see
/// [`Trigger::starts_on_press`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Trigger {
    /// A key press (SFZ `trigger=attack`, the default).
    #[default]
    Attack,
    /// A key release (SFZ `trigger=release`): the sample of the released note's tail. Under
    /// the sustain pedal it waits for the pedal to lift ("on note-off or sustain pedal off").
    Release,
    /// A key press while no other key is down (SFZ `trigger=first`): the first note of a
    /// phrase, which a legato patch plays with its attack.
    First,
    /// A key press while another key is down (SFZ `trigger=legato`): a note that joins the one
    /// before it, which a legato patch plays as a transition.
    Legato,
    /// A key release, whatever the sustain pedal does (SFZ `trigger=release_key`, "ignores
    /// sustain pedal"): Sonatina's timpani roll endings.
    ReleaseKey,
}

impl Trigger {
    /// Whether a key press starts a region of this trigger, given whether another key was down
    /// when it was pressed: an attack region always, a `first` region only when no other key
    /// was down, a `legato` region only when one was, a release region never.
    #[must_use]
    pub fn starts_on_press(self, another_key_down: bool) -> bool {
        match self {
            Self::Attack => true,
            Self::First => !another_key_down,
            Self::Legato => another_key_down,
            Self::Release | Self::ReleaseKey => false,
        }
    }

    /// Whether a key release, rather than a press, starts a region of this trigger (`release`
    /// and `release_key`): its voice is the released note's tail, which plays on and which a
    /// later release of the key does not fade.
    #[must_use]
    pub fn is_release(self) -> bool {
        matches!(self, Self::Release | Self::ReleaseKey)
    }
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

/// How the voices of a region fade out when the exclusive group named in its `off_by` switches
/// them off (SFZ `off_mode`, and `off_time` for [`OffMode::Time`]).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum OffMode {
    /// A quick fade that ignores the region's release (SFZ `fast`, the default): the sampler's
    /// fixed 10 ms choke.
    #[default]
    Fast,
    /// The region's own release, as when its key is released (SFZ `normal`): a note that an
    /// exclusive group switches off fades over its release time while the note that switched it
    /// off starts, so the two overlap.
    Normal,
    /// A fade over the given time in seconds, independent of the region's release and of the
    /// sampler's release scale (SFZ `time` with `off_time`, an ARIA extension): Sonatina's legato
    /// patches overlap their notes by 1 s this way. The sampler fades over at least its shortest
    /// fade-out, so a time of 0 does not click.
    Time(f64),
}

impl OffMode {
    /// The `off_time` of [`OffMode::Time`] when the patch writes none, in seconds: 6 ms, the
    /// default the SFZ format gives (ARIA).
    pub const DEFAULT_TIME: f64 = 0.006;
}

/// The shape of a crossfade (SFZ `xf_velcurve` for velocity crossfades, `xf_cccurve` for
/// controller crossfades).
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

/// How a region fades in and out over the values 0 to 127 of the note's velocity (SFZ
/// `xfin_lovel`/`xfin_hivel`, `xfout_lovel`/`xfout_hivel` and `xf_velcurve`) or of a controller
/// (SFZ `xfin_loccN`/`xfin_hiccN`, `xfout_loccN`/`xfout_hiccN` and `xf_cccurve`), as the SFZ
/// format describes it; the velocity gains match the reference player sfizz's tests.
///
/// The fade-in is silent at values at or below `in_lo` and full at or above `in_hi`; the
/// fade-out is full at or below `out_lo` and silent at or above `out_hi`. In between, the gain
/// follows `curve` over the position in the fade, `(value - lo) / (hi - lo)`. Where the two
/// bounds of a fade meet or cross, the low bound decides: such a fade-in is silent below `in_lo`
/// and full from it, and such a fade-out is full up to `out_lo` and silent above it. So
/// `xfin_lovel=70` alone starts the region at velocity 70, and `xfout_hivel=90` alone, whose
/// `xfout_lovel` stays 127, fades nothing, as in sfizz.
///
/// Two edges keep the SFZ format's bounds where sfizz differs for velocities: a fade whose high
/// bound is 127 reaches its end at 127 (sfizz ends it one velocity early), and a fade-out of no
/// width is silent above its value (sfizz fades nothing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crossfade {
    /// The value at and below which the fade-in is silent (SFZ `xfin_lovel`, `xfin_loccN`;
    /// default 0).
    pub in_lo: u8,
    /// The value from which the fade-in is full (SFZ `xfin_hivel`, `xfin_hiccN`; default 0).
    pub in_hi: u8,
    /// The value up to which the fade-out is full (SFZ `xfout_lovel`, `xfout_loccN`; default
    /// 127).
    pub out_lo: u8,
    /// The value from which the fade-out is silent (SFZ `xfout_hivel`, `xfout_hiccN`; default
    /// 127).
    pub out_hi: u8,
    /// The shape of both fades (SFZ `xf_velcurve`, `xf_cccurve`).
    pub curve: CrossfadeCurve,
}

impl Crossfade {
    /// No crossfade, the SFZ defaults: full at every value.
    pub const NONE: Self = Self {
        in_lo: 0,
        in_hi: 0,
        out_lo: 127,
        out_hi: 127,
        curve: CrossfadeCurve::Power,
    };

    /// The gain of the crossfade at `value`, from 0 (silent) to 1 (full): the fade-in's gain
    /// times the fade-out's.
    #[must_use]
    pub fn gain(self, value: u8) -> f64 {
        self.fade_in(value) * self.fade_out(value)
    }

    fn fade_in(self, value: u8) -> f64 {
        let (lo, hi) = (self.in_lo, self.in_hi);
        if value < lo {
            0.0
        } else if value >= hi {
            1.0
        } else {
            // lo <= value < hi: the fade spans at least one step.
            self.curve.gain(position(value, lo, hi))
        }
    }

    fn fade_out(self, value: u8) -> f64 {
        let (lo, hi) = (self.out_lo, self.out_hi);
        if value <= lo {
            1.0
        } else if value >= hi {
            0.0
        } else {
            // lo < value < hi: the fade spans at least two steps.
            self.curve.gain(1.0 - position(value, lo, hi))
        }
    }
}

impl Default for Crossfade {
    fn default() -> Self {
        Self::NONE
    }
}

/// Where `value` lies between `lo` (0) and `hi` (1); the callers keep `lo <= value < hi`, so the
/// divisor is positive.
fn position(value: u8, lo: u8, hi: u8) -> f64 {
    (f64::from(value) - f64::from(lo)) / (f64::from(hi) - f64::from(lo))
}

/// The last value of each of the 128 MIDI controllers, 0 to 127: what a region's controller
/// gates, triggers, crossfades and gains read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Controllers([u8; 128]);

impl Controllers {
    /// Where the controllers start when a patch sets none (SFZ `set_ccN`): at 0, as the SFZ
    /// format starts them, except volume (7) and expression (11), which start at 127 because
    /// the sampler plays them full until a controller event arrives.
    pub const INITIAL: Self = {
        let mut values = [0; 128];
        values[7] = 127;
        values[11] = 127;
        Self(values)
    };

    /// The value of controller `number`; 0 for a number above 127, which names no MIDI
    /// controller.
    #[must_use]
    pub fn value(&self, number: u8) -> u8 {
        self.0.get(usize::from(number)).copied().unwrap_or(0)
    }

    /// Sets controller `number` to `value`, read as 127 when above. A number above 127 names no
    /// MIDI controller and is ignored.
    pub fn set(&mut self, number: u8, value: u8) {
        if let Some(slot) = self.0.get_mut(usize::from(number)) {
            *slot = value.min(127);
        }
    }
}

impl Default for Controllers {
    fn default() -> Self {
        Self::INITIAL
    }
}

/// A range of values of one MIDI controller: a region's gate (SFZ `loccN`/`hiccN`) or trigger
/// (SFZ `on_loccN`/`on_hiccN`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CcRange {
    /// The controller number, 0 to 127.
    pub controller: u8,
    /// The lowest value in the range.
    pub lo: u8,
    /// The highest value in the range; a range whose `hi` is below its `lo` holds no value.
    pub hi: u8,
}

impl CcRange {
    /// Whether `value` lies in the range, both ends included.
    #[must_use]
    pub fn contains(self, value: u8) -> bool {
        (self.lo..=self.hi).contains(&value)
    }
}

/// How a region fades in and out with one controller (SFZ `xfin_loccN`, `xfin_hiccN`,
/// `xfout_loccN`, `xfout_hiccN` and `xf_cccurve`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CcCrossfade {
    /// The controller number, 0 to 127.
    pub controller: u8,
    /// The fades over the controller's values.
    pub fade: Crossfade,
}

/// How far one controller moves a region's level at its full value 127, in proportion to the
/// value: decibels for [`Region::cc_volume`], a percentage for [`Region::cc_amplitude`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CcDepth {
    /// The controller number, 0 to 127.
    pub controller: u8,
    /// The change at controller value 127.
    pub depth: f64,
}

impl CcDepth {
    /// The change at the controller's value in `controllers`: `depth * value / 127`.
    fn at(self, controllers: &Controllers) -> f64 {
        self.depth * f64::from(controllers.value(self.controller)) / 127.0
    }
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
    /// controller starts the region instead (SFZ `on_loccN`/`on_hiccN`, see
    /// [`Region::cc_triggers`]). A flag rather than an empty key range, so the range keeps its
    /// meaning (TYP-03).
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
    /// Region amplitude in percent of full, 0 to 100 (SFZ `amplitude`, default 100).
    pub amplitude: f64,
    /// How much the gain follows the note's velocity, in percent (SFZ `amp_veltrack`, -100 to
    /// 100, default 100); see [`Region::velocity_gain`].
    pub amp_veltrack: f64,
    /// How the region fades in and out with the note's velocity.
    pub velocity_crossfade: Crossfade,
    /// The controller ranges a region needs to start (SFZ `loccN`/`hiccN`): each controller's
    /// last value must lie in its range when the key press, key release or controller event
    /// would start the region. Read only then: a voice that sounds keeps sounding when the
    /// controller leaves the range. Empty for none, sorted by controller.
    pub cc_gates: Box<[CcRange]>,
    /// The controller ranges that start the region (SFZ `on_loccN`/`on_hiccN`): a controller
    /// event whose value lies in one of them starts it, at its `pitch_keycenter` and without a
    /// velocity gain. Empty for a region no controller starts, sorted by controller.
    pub cc_triggers: Box<[CcRange]>,
    /// How the region fades in and out with controllers, while it sounds. Empty for none,
    /// sorted by controller.
    pub cc_crossfades: Box<[CcCrossfade]>,
    /// The volume change in dB each controller gives the region at value 127, in proportion to
    /// its value, while it sounds (SFZ `volume_onccN`, also written `gain_ccN` and
    /// `gain_onccN`). Empty for none, sorted by controller.
    pub cc_volume: Box<[CcDepth]>,
    /// The percentage of its amplitude each controller gives the region at value 127, in
    /// proportion to its value, while it sounds (SFZ `amplitude_onccN`, also written
    /// `amplitude_ccN`): 100 plays it full at 127 and silences it at 0. Empty for none, sorted by
    /// controller.
    pub cc_amplitude: Box<[CcDepth]>,
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
    /// How the region's voices fade out when the group in `off_by` silences them.
    pub off_mode: OffMode,
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
            amplitude: 100.0,
            amp_veltrack: 100.0,
            velocity_crossfade: Crossfade::NONE,
            cc_gates: Box::default(),
            cc_triggers: Box::default(),
            cc_crossfades: Box::default(),
            cc_volume: Box::default(),
            cc_amplitude: Box::default(),
            trigger: Trigger::Attack,
            seq_length: 1,
            seq_position: 1,
            rand_lo: 0.0,
            rand_hi: 1.0,
            sw_last: None,
            group: 0,
            off_by: 0,
            off_mode: OffMode::Fast,
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
    /// ([`Crossfade::gain`]).
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

    /// Whether `controllers` open the region's gates ([`Region::cc_gates`]): every gated
    /// controller lies in its range. A region without gates is always open.
    #[must_use]
    pub fn gates_open(&self, controllers: &Controllers) -> bool {
        self.cc_gates
            .iter()
            .all(|gate| gate.contains(controllers.value(gate.controller)))
    }

    /// Whether an event that sets controller `number` to `value` starts the region
    /// ([`Region::cc_triggers`]), before its gates are checked.
    #[must_use]
    pub fn starts_on_controller(&self, number: u8, value: u8) -> bool {
        self.cc_triggers
            .iter()
            .any(|trigger| trigger.controller == number && trigger.contains(value))
    }

    /// The gain `controllers` give the sounding region, from 0 up: its controller crossfades
    /// ([`Crossfade::gain`]), times the amplitude of [`Region::cc_amplitude`] (each controller's
    /// `depth / 100 * value / 127`, multiplied together), times the gain of the volume change of
    /// [`Region::cc_volume`] (the sum of each controller's `depth * value / 127` dB). Exactly 1
    /// for a region no controller changes.
    #[must_use]
    pub fn controller_gain(&self, controllers: &Controllers) -> f64 {
        let crossfades = self.cc_crossfades.iter().fold(1.0, |gain, crossfade| {
            gain * crossfade.fade.gain(controllers.value(crossfade.controller))
        });
        let amplitude = self
            .cc_amplitude
            .iter()
            .fold(1.0, |gain, depth| gain * depth.at(controllers) / 100.0);
        if self.cc_volume.is_empty() {
            return crossfades * amplitude;
        }
        let db = self
            .cc_volume
            .iter()
            .fold(0.0, |db, depth| db + depth.at(controllers));
        crossfades * amplitude * db_to_amp(db)
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
    /// Where the controllers start on every MIDI channel when the instrument is loaded or
    /// reset: [`Controllers::INITIAL`] with the values the patch sets (SFZ `set_ccN`).
    pub controllers: Controllers,
    /// How long the instrument can keep sounding after the last key release, in seconds: the
    /// longest release, the longest timed fade of a region that an exclusive group can switch
    /// off ([`OffMode::Time`]: a press just before the last release may have started it), and
    /// the longest sample a key release (`trigger=release` and `release_key`) or a controller
    /// (such as a pedal lifted after the last note) starts.
    pub tail_seconds: f64,
    /// What the patch writes that the sampler does not play, sorted, without duplicates: the
    /// names of unsupported SFZ opcodes (a numbered family as one entry, `pan_oncc*`), the
    /// values of supported opcodes it plays as if they were absent (`loop_mode=loop_forever`),
    /// and the headers whose opcodes are skipped (`<curve>`).
    pub ignored_opcodes: Vec<String>,
    /// The regions the loader left out and why, one note per sample they name, in the order the
    /// patch first names it: a sample file that does not exist, or a generator the sampler does
    /// not play (SFZ `*sine`). Each note counts the regions it left out. Empty when every region
    /// was loaded.
    pub load_notes: Vec<String>,
}

/// How a choice of the sampler's `articulation` parameter is reached in the patch, as the
/// library's manifest or Sonara's profile maps the articulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selector {
    /// The keyswitch with this key, 0 to 127, is the active switch for new notes.
    Keyswitch(u8),
    /// Notes play in the velocity zone `lo` to `hi`, with 1 ≤ `lo` ≤ `hi` ≤ 127.
    Velocity {
        /// The lowest velocity of the zone.
        lo: u8,
        /// The highest velocity of the zone.
        hi: u8,
    },
    /// The controller `number` is set to `value`, both 0 to 127.
    Controller {
        /// The controller number.
        number: u8,
        /// The value that selects the articulation.
        value: u8,
    },
}

/// What one choice of the sampler's `articulation` parameter plays in a preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArticulationTarget {
    /// How the articulation is reached in the patch; `None` when the patch as it is plays it,
    /// with nothing to select (a patch that is the articulation).
    pub selector: Option<Selector>,
    /// Whether successive notes are joined by the sampler's overlap legato
    /// ([`SamplerMapping::legato`]).
    pub legato: bool,
}

/// What the sampler's `dynamics` parameter drives in a preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DynamicsDrive {
    /// The note velocity picks the layer and the gain (the default).
    #[default]
    Velocity,
    /// The controller with this number, 0 to 127, crossfades the layers.
    Controller(u8),
    /// Every note plays at this velocity, 1 to 127, whatever its own.
    Fixed(u8),
}

/// How the sampler's overlap legato joins two successive notes of a preset whose library says
/// it plays legato that way: the old note fades out while the new one fades in, past the
/// attack of its sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegatoSpec {
    /// The crossfade between the two notes, in milliseconds.
    pub crossfade_ms: u16,
    /// Where the new note's sample starts, in milliseconds into it.
    pub start_offset_ms: u16,
    /// The longest silence between two notes that still joins them, in milliseconds.
    pub gap_ms: u16,
}

/// How the sampler plays one preset beyond its patch, compiled by the application off the
/// real-time thread from what the library's manifest or Sonara's profile says about the
/// preset, and read on the render thread without allocating (RT-02).
///
/// - **Articulation targets**, one per choice of the sampler's `articulation` parameter, in the
///   order of its choices: [`Self::AUTO`], [`Self::ORDINARIO`], then the articulation
///   vocabulary. A choice the preset does not map holds the target of the nearest articulation
///   it plays, chosen when the mapping was compiled, so the render thread never searches.
///   `auto` never has a target: the patch's own default and the keyswitch notes of clips decide.
/// - **Dynamics**, **legato** and the **key offset** the preset is played with.
///
/// The default is the mapping of a preset about which nothing is claimed: no target for any
/// choice, velocity dynamics, no overlap legato and no key offset, so the patch plays as it is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SamplerMapping {
    /// The target of each choice, by index; shorter than the choices when the last ones have
    /// none, and empty when no choice has one, so that equal mappings compare equal.
    targets: Box<[Option<ArticulationTarget>]>,
    /// What the `dynamics` parameter drives.
    pub dynamics: DynamicsDrive,
    /// The overlap legato of a preset that plays legato by overlapping its notes; `None` for a
    /// preset that does not, or whose own regions play legato.
    pub legato: Option<LegatoSpec>,
    /// Semitones added to incoming notes, for a patch keyed away from the pitch it sounds.
    pub key_offset: i8,
}

impl SamplerMapping {
    /// The choice `auto` of the `articulation` parameter, its first: no target.
    pub const AUTO: u16 = 0;
    /// The choice `ordinario` of the `articulation` parameter, its second: the preset's
    /// ordinary sound, its default articulation.
    pub const ORDINARIO: u16 = 1;

    /// Setup: a mapping with the target of each choice of the `articulation` parameter, by
    /// index, and the dynamics, legato and key offset of the preset. A target given for
    /// [`Self::AUTO`] is dropped, because `auto` has none.
    #[must_use]
    pub fn new(
        mut targets: Vec<Option<ArticulationTarget>>,
        dynamics: DynamicsDrive,
        legato: Option<LegatoSpec>,
        key_offset: i8,
    ) -> Self {
        if let Some(auto) = targets.get_mut(usize::from(Self::AUTO)) {
            *auto = None;
        }
        let used = targets
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |last| last.saturating_add(1));
        targets.truncate(used);
        Self {
            targets: targets.into_boxed_slice(),
            dynamics,
            legato,
            key_offset,
        }
    }

    /// Real-time: the target of the `articulation` choice with index `choice`; `None` for
    /// `auto`, for a choice the preset has no target for, and for an index beyond the choices.
    #[must_use]
    pub fn target(&self, choice: u16) -> Option<ArticulationTarget> {
        self.targets.get(usize::from(choice)).copied().flatten()
    }

    /// Real-time: the target of [`Self::ORDINARIO`], the preset's ordinary sound.
    #[must_use]
    pub fn default_target(&self) -> Option<ArticulationTarget> {
        self.target(Self::ORDINARIO)
    }
}

/// A sampler's instrument as the render graph hands it to the processor through
/// [`DeviceState::sampler`](crate::processor::DeviceState::sampler): the loaded patch and the
/// mapping of the preset it plays. Built off the real-time thread, shared read-only, and
/// dropped with the processor off it (RT-09).
#[derive(Debug, Clone, PartialEq)]
pub struct SamplerSetup {
    /// The decoded samples and regions of the patch.
    pub bank: Arc<SampleBank>,
    /// How the preset is played beyond its patch.
    pub mapping: Arc<SamplerMapping>,
}

impl SamplerSetup {
    /// Setup: `bank` played as it is, with the mapping of a preset about which nothing is
    /// claimed ([`SamplerMapping::default`]).
    #[must_use]
    pub fn new(bank: Arc<SampleBank>) -> Self {
        Self {
            bank,
            mapping: Arc::default(),
        }
    }
}
