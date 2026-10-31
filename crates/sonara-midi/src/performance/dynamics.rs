//! Dynamics: how loud each note of a line plays, the velocity it is written at, and the points of
//! the sampler's `dynamics` lane that shape the level between and during the notes.
//!
//! **One level table.** [`LEVELS`] ties the six notated markings to velocities: the band of
//! velocities that reads as a marking, the level velocity a note at the marking plays, and the
//! `dynamics` value that stands for the marking when the parameter drives a controller crossfade.
//! The shaping here, the velocities a performance writes and the rule checks' reading of a
//! velocity as a dynamic ([`velocity_band`](super::check::velocity_band)) all read this one table
//! ([`marking`]), so they cannot disagree (DES-01). The numbers spread the markings evenly over
//! the velocities, starting values to be calibrated against the loudness of real libraries by
//! listening.
//!
//! **Levels.** A [`Level`] counts markings from pp (0) to ff (5), so a shape can rise a step, or
//! fall half a step, between two markings: a fraction lies between two rows of the table, and its
//! velocity and controller value are interpolated between theirs.
//!
//! **Phrases and shapes.** A line falls into phrases: a phrase ends at a rest of at least one beat
//! or 0.6 s, at the start of a section ([`ScoreContext::with_sections`]), after a note at least
//! twice the line's median length that a rest follows, and after eight bars at most. A part plays
//! at the marking the request gives its melody, a part that accompanies (harmony, bass, pedal) one
//! marking below ([`part_marking`]), so the melody stands out by the step the balance rules ask
//! for. On that level L:
//!
//! - a melody or counter-melody phrase of several onsets is an **arc**: it starts at L, peaks one
//!   step higher at its highest note (the later one on ties, or 60 % into the phrase when every
//!   note has one pitch) and ends half a step below L, on smooth curves;
//! - any other phrase holds L;
//! - a note that asks for `crescendo_swell` which the sound has not recorded **swells**: from the
//!   level at its onset one step up at 60 % of its length and back, and so does every held note of
//!   at least two beats in a part that accompanies when the request asks for swells;
//! - a percussion roll grows to the request's target when it asks for one, **back-loaded**: a
//!   third of the rise at 70 % of the roll, then smoothly to the target at its end. The level then
//!   returns to the phrase's for the notes after it. A roll during which another note starts
//!   keeps its phrase's level.
//!
//! **Written velocities and the lane**, by how the sound follows `dynamics`
//! ([`DynamicsKind`]):
//!
//! - **Velocity.** A note-on plays its velocity times `dynamics`, which picks the velocity layer and
//!   the gain, and a held note follows later changes of `dynamics` as the square of their ratio.
//!   So every note is written at the level velocity of the loudest level its phrase reaches, and
//!   the lane holds the level velocity of each moment divided by it (at most 1): each note-on lands
//!   on its level's velocity and so in its level's layer, and the lane moves smoothly between the
//!   notes. Phrases joined by a note that sounds across their boundary share one written
//!   velocity, and so does every phrase of a percussion, keyboard or plucked part, whose notes
//!   ring after they end, so that no change of the lane at a phrase's start jumps a sounding note.
//! - **Fixed.** The sound plays every note-on at its own fixed velocity, so the lane alone carries
//!   the level, as a ratio to that velocity; a level above it plays at it, and the notes are
//!   written at their levels' velocities.
//! - **Controller.** The lane holds each level's controller value from the table, and every note
//!   is written at its level's velocity.
//!
//! A held note's gain can at most double its level (+12 dB), so a roll's crescendo stops where the
//! level of a note sounding at its start has doubled, and the result says so ([`Remark`]).
//! A velocity accent the articulation did not turn into `marcato` (a note at least 20 above its
//! phrase's median velocity), and a rewrite that asks a note to sound a step louder or softer,
//! moves that note's written velocity a step from its level.
//!
//! **When the lane is read.** The engine reads a device parameter once per block, at the block's
//! first frame, so a value that must hold at a note-on is held from two blocks (at the slowest
//! sample rate a project supports) before it, or from halfway after the previous note-on when that
//! is nearer: whichever block holds the note-on reads exactly its level. The phrases and their
//! shapes are read from the notes as written, but the holds follow the note-ons as played: when
//! humanization moves the notes of an onset ([`perform`](super::perform)), the value holds from
//! before the earliest of them to the latest. On a sound without a `dynamics` parameter the same
//! shape can drive the track's `volume_db` lane ([`Dynamics::volume_lane`]).
//!
//! **Controller events.** The same shape also comes as the events of one MIDI controller in the
//! part's clip ([`ControllerLane`]), which move with the clip where the lane stays with the track.
//! They take the lane's place, with `dynamics` left at its default 1: written together, the two
//! would shape the level twice. The controller follows the sound ([`DynamicsKind::controller`]):
//!
//! - **Velocity and fixed.** Expression (CC 11), which the sampler applies as a linear gain of
//!   `value / 127` to everything the track plays. The lane's value `d` gives a note the gain `d²`
//!   (a note-on plays its velocity times `d`, and the sampler's level follows the square of the
//!   velocity; a held note follows `(d / d₀)²`), so the event's value is `127 · d²`, and every
//!   note plays at the level the lane would give it. Expression does not scale the velocity, so
//!   a note plays the layer of its written velocity, its phrase's loudest level in velocity mode,
//!   where the lane would pick the layer of its own level.
//! - **Controller.** Modulation (CC 1), the controller libraries crossfade their dynamic layers
//!   on: the value is the controller value `dynamics` writes, `127 · d`. Both values are rounded
//!   half away from zero, as the sampler rounds the parameter's controller value. A preset that
//!   crossfades on another controller takes the same values there
//!   ([`ControllerLane::on_controller`]).
//!
//! The first event holds from before the first note-on, as a later onset's value does, so the
//! controller is set before the note that needs it. Along a curve the events follow the lane at
//! most once per engine block (at the slowest sample rate, the longest block), stopping a block
//! short of the curve's next point, and an event that repeats the value before it is left out.
//! The last value holds after the part's last note.
//!
//! Everything is a pure function of the line and the score: no random choice is made, so the same
//! input gives the same result (TST-04). Nothing here runs on a real-time thread (RT-01).

use std::f64::consts::PI;
use std::ops::Range;

use sonara_core::EPSILON;
use sonara_core::midi::{ControlKind, MidiControlEvent, MidiNote};
use sonara_core::model::automation::{AutomationPoint, CurveKind};

use super::articulate::{
    ACCENT_VELOCITY, BEAT_EPSILON, Class, Clock, NoteArticulation, PHRASE_REST_BEATS,
    SWITCH_LEAD_BLOCKS, block_seconds, onset_groups,
};
use super::{DynamicsKind, OnsetSpan, PartLine, Role, ScoreContext};
use crate::num::py_round;
use crate::orchestration::fallback::Emulation;
use crate::orchestration::rules::Dynamic;
use crate::orchestration::{Articulation, Family};

/// One row of the level table: a notated marking and the velocities that stand for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelRow {
    /// The marking.
    pub marking: Dynamic,
    /// The softest velocity that reads as the marking.
    pub lowest: u8,
    /// The loudest velocity that reads as the marking.
    pub highest: u8,
    /// The velocity a note at the marking plays: its level velocity.
    pub velocity: u8,
    /// The value of the sampler's `dynamics` parameter that stands for the marking when the
    /// parameter drives a controller crossfade, an absolute level from 0 to 1.
    pub cc: f64,
}

/// The level table, from pp to ff, row `i` the row of `Dynamic::ALL[i]`. The bands cover the
/// velocities 1 to 127 without a gap, and the level velocities and controller values rise with the
/// marking; a compile-time check proves both.
///
/// | Marking | Velocities | Level velocity | `dynamics` (controller) |
/// |---|---|---|---|
/// | pp | 1-39 | 32 | 0.15 |
/// | p | 40-54 | 47 | 0.30 |
/// | mp | 55-69 | 62 | 0.45 |
/// | mf | 70-84 | 76 | 0.60 |
/// | f | 85-104 | 95 | 0.75 |
/// | ff | 105-127 | 115 | 0.90 |
///
/// The values spread the markings evenly; they are starting values to be calibrated against the
/// loudness of real libraries by listening.
pub const LEVELS: [LevelRow; 6] = [
    level_row(Dynamic::Pianissimo, 1, 39, 32, 0.15),
    level_row(Dynamic::Piano, 40, 54, 47, 0.30),
    level_row(Dynamic::MezzoPiano, 55, 69, 62, 0.45),
    level_row(Dynamic::MezzoForte, 70, 84, 76, 0.60),
    level_row(Dynamic::Forte, 85, 104, 95, 0.75),
    level_row(Dynamic::Fortissimo, 105, 127, 115, 0.90),
];

const fn level_row(marking: Dynamic, lowest: u8, highest: u8, velocity: u8, cc: f64) -> LevelRow {
    LevelRow {
        marking,
        lowest,
        highest,
        velocity,
        cc,
    }
}

/// Whether row `i` of [`LEVELS`] is the row of `Dynamic::ALL[i]`, the bands run from 1 to 127
/// without a gap, each level velocity lies in its band, and the level velocities and controller
/// values rise strictly within 0 to 1.
const fn levels_are_coherent() -> bool {
    if LEVELS.len() != Dynamic::ALL.len() {
        return false;
    }
    let mut i = 0;
    while i < LEVELS.len() {
        let row = LEVELS[i];
        if row.marking as usize != i || Dynamic::ALL[i] as usize != i {
            return false;
        }
        if row.velocity < row.lowest || row.velocity > row.highest {
            return false;
        }
        if !(row.cc > 0.0 && row.cc <= 1.0) {
            return false;
        }
        let follows = if i == 0 {
            row.lowest == 1
        } else {
            let previous = LEVELS[i - 1];
            row.lowest as u16 == previous.highest as u16 + 1
                && row.velocity > previous.velocity
                && row.cc > previous.cc
        };
        if !follows {
            return false;
        }
        i += 1;
    }
    LEVELS[LEVELS.len() - 1].highest == 127
}

const _: () = assert!(
    levels_are_coherent(),
    "the level table follows the markings and covers the velocities"
);

/// The level velocity of `marking`: the velocity a note at the marking plays.
#[must_use]
pub const fn level_velocity(marking: Dynamic) -> u8 {
    // Row `i` is the row of the marking `i`, which a compile-time check proves.
    LEVELS[marking as usize].velocity
}

/// The value of the sampler's `dynamics` parameter that stands for `marking` when the parameter
/// drives a controller crossfade.
#[must_use]
pub const fn cc_value(marking: Dynamic) -> f64 {
    // Row `i` is the row of the marking `i`, which a compile-time check proves.
    LEVELS[marking as usize].cc
}

/// The marking a velocity reads as: the marking whose band in [`LEVELS`] holds it. A velocity of
/// 0 is no note and reads as pp.
#[must_use]
pub const fn marking(velocity: u8) -> Dynamic {
    let mut i = 0;
    while i < LEVELS.len() {
        if velocity <= LEVELS[i].highest {
            return LEVELS[i].marking;
        }
        i += 1;
    }
    Dynamic::Fortissimo
}

/// The marking a part with `role` plays at when the request gives its melody `melody`: one
/// marking below for a part that accompanies (harmony, bass or pedal), so that the melody stands
/// out by one marked step, and `melody` for every other part. pp stays pp.
#[must_use]
pub const fn part_marking(melody: Dynamic, role: Role) -> Dynamic {
    if !accompanies(role) {
        return melody;
    }
    match melody {
        Dynamic::Pianissimo | Dynamic::Piano => Dynamic::Pianissimo,
        Dynamic::MezzoPiano => Dynamic::Piano,
        Dynamic::MezzoForte => Dynamic::MezzoPiano,
        Dynamic::Forte => Dynamic::MezzoForte,
        Dynamic::Fortissimo => Dynamic::Forte,
    }
}

/// Whether a part with `role` accompanies the melody: harmony, bass or pedal.
const fn accompanies(role: Role) -> bool {
    matches!(role, Role::Harmony | Role::Bass | Role::Pedal)
}

/// Whether a part with `role` carries a tune whose phrases arc: the melody or a counter-melody.
const fn sings(role: Role) -> bool {
    matches!(role, Role::Melody | Role::CounterMelody)
}

/// The step of the loudest marking, ff.
#[expect(
    clippy::cast_precision_loss,
    reason = "the table has six rows, exactly representable"
)]
const LOUDEST_STEP: f64 = (LEVELS.len() - 1) as f64;

/// A dynamic level, in marked steps from pp (0) to ff (5). A fraction lies between two markings:
/// 3.5 is half a step above mf. Levels below pp and above ff read as pp and ff.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Level(f64);

impl Level {
    /// The level of `marking`.
    #[must_use]
    pub const fn of(marking: Dynamic) -> Self {
        Self(marking as u8 as f64)
    }

    /// The level `steps` marked steps above pp, kept within pp and ff; a value that is not a
    /// number reads as pp.
    #[must_use]
    pub fn from_steps(steps: f64) -> Self {
        if steps.is_nan() {
            Self(0.0)
        } else {
            Self(steps.clamp(0.0, LOUDEST_STEP))
        }
    }

    /// The marked steps above pp, from 0 to 5.
    #[must_use]
    pub const fn steps(self) -> f64 {
        self.0
    }

    /// The level velocity: between two markings, interpolated between their level velocities and
    /// rounded as Python rounds (ties to even), so half a step below mf (76) plays 69.
    #[must_use]
    pub fn velocity(self) -> u8 {
        velocity_of(self.interpolate(|row| f64::from(row.velocity)))
    }

    /// The `dynamics` value in controller mode, interpolated between the markings' values.
    #[must_use]
    pub fn cc(self) -> f64 {
        self.interpolate(|row| row.cc)
    }

    /// The marking the level's velocity reads as ([`marking`]).
    #[must_use]
    pub fn marking(self) -> Dynamic {
        marking(self.velocity())
    }

    /// The level `steps` marked steps louder (softer when negative).
    fn up(self, steps: f64) -> Self {
        Self::from_steps(self.0 + steps)
    }

    /// The level whose interpolated level velocity is `velocity`, kept within pp and ff.
    fn from_velocity(velocity: f64) -> Self {
        let mut previous: Option<(f64, f64)> = None;
        for (step, row) in (0_u8..).zip(LEVELS.iter()) {
            let here = (f64::from(step), f64::from(row.velocity));
            match previous {
                None if velocity <= here.1 => return Self(here.0),
                Some((low_step, low_velocity)) if velocity <= here.1 => {
                    let share = (velocity - low_velocity) / (here.1 - low_velocity);
                    return Self::from_steps(low_step + share);
                }
                _ => previous = Some(here),
            }
        }
        Self(LOUDEST_STEP)
    }

    /// A column of the table at this level, interpolated between the two rows around it.
    fn interpolate(self, column: impl Fn(&LevelRow) -> f64) -> f64 {
        let below = self.0.floor();
        let share = self.0 - below;
        let at = |step: f64| -> Option<f64> {
            let index = num_steps(step)?;
            LEVELS.get(index).map(&column)
        };
        match (at(below), at(below + 1.0)) {
            (Some(low), Some(high)) => low + (high - low) * share,
            (Some(low), None) => low,
            _ => LEVELS.first().map_or(0.0, &column),
        }
    }
}

/// A whole step from 0 to 5 as an index of [`LEVELS`].
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the step is a whole number from 0 to 5, checked first"
)]
fn num_steps(step: f64) -> Option<usize> {
    ((0.0..=LOUDEST_STEP).contains(&step) && step.fract() == 0.0).then_some(step as usize)
}

/// What a request asks of a part's dynamics.
///
/// [`DynamicsRequest::default`] is a melody at mf, no swells and rolls that hold their level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DynamicsRequest {
    /// The marking the request gives its melody. The part plays at it, or one marking below when
    /// it accompanies ([`part_marking`]).
    pub marking: Dynamic,
    /// Whether the request asks the accompaniment to swell: every held note of at least two beats
    /// of a part that accompanies swells a step and back.
    pub swells: bool,
    /// The marking the part's percussion rolls grow to; `None` keeps them at the part's level.
    pub roll_to: Option<Dynamic>,
}

impl Default for DynamicsRequest {
    fn default() -> Self {
        Self {
            marking: Dynamic::MezzoForte,
            swells: false,
            roll_to: None,
        }
    }
}

/// One phrase of a line.
#[derive(Clone, Debug, PartialEq)]
pub struct Phrase {
    /// The content beat of its first note-on as written.
    pub start: f64,
    /// The content beat where its last note ends as written.
    pub end: f64,
    /// The content beat of an arc's peak; `None` for a phrase that holds the part's level.
    pub peak: Option<f64>,
    /// In velocity mode, the velocity its notes are written at before accents: the level velocity
    /// of the loudest level the lane reaches during it and the phrases joined to it; `None` in the
    /// other modes.
    pub written: Option<u8>,
}

/// The level of one note.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteLevel {
    /// The id of the note.
    pub note: String,
    /// The level its note-on plays, its accent included.
    pub level: Level,
    /// The velocity its note-on plays at: the written velocity times the lane in velocity mode,
    /// the fixed velocity times the lane in fixed mode, the written velocity in controller mode.
    pub velocity: u8,
}

/// Something the dynamics could not do as asked.
#[derive(Clone, Debug, PartialEq)]
pub enum Remark {
    /// A roll's crescendo stops below the marking asked for, where the level of a note sounding at
    /// its start has doubled: a held note's gain can rise by +12 dB at most.
    BoundedRoll {
        /// The id of the roll.
        note: String,
        /// The marking asked for.
        asked: Dynamic,
        /// The level the roll reaches.
        reached: Level,
    },
    /// The sound plays every note-on at a fixed velocity below the loudest level asked for, so the
    /// louder levels play at the fixed velocity.
    FixedCeiling {
        /// The sound's fixed velocity.
        velocity: u8,
        /// The level velocity of the loudest level asked for.
        wanted: u8,
    },
}

/// The dynamics of a line.
#[derive(Clone, Debug, PartialEq)]
pub struct Dynamics {
    /// How the sound follows `dynamics`.
    pub kind: DynamicsKind,
    /// The marking the part plays at.
    pub marking: Dynamic,
    /// The phrases, in time order.
    pub phrases: Vec<Phrase>,
    /// The level of each note, in the order of the performance's notes.
    pub notes: Vec<NoteLevel>,
    /// The points of the sampler's `dynamics` lane, at timeline beats in time order; empty for a
    /// line without notes.
    pub lane: Vec<AutomationPoint>,
    /// The same shape as the events of one MIDI controller in the part's clip, to write in place
    /// of the lane.
    pub controller: ControllerLane,
    /// What could not be done as asked.
    pub remarks: Vec<Remark>,
}

impl Dynamics {
    /// The lane as points of the track's `volume_db` lane, for a sound without a `dynamics`
    /// parameter: each value `d` becomes `fader_db + 40 · log10(d)`, the sampler's square law of a
    /// held note's gain (for another instrument, an estimate). `None` unless the sound follows
    /// `dynamics` by velocity, where the lane is a ratio.
    #[must_use]
    pub fn volume_lane(&self, fader_db: f64) -> Option<Vec<AutomationPoint>> {
        (self.kind == DynamicsKind::Velocity).then(|| {
            self.lane
                .iter()
                .map(|point| AutomationPoint {
                    value: fader_db + 40.0 * point.value.log10(),
                    ..*point
                })
                .collect()
        })
    }
}

/// The MIDI controller number of expression, which the sampler applies as a linear gain.
pub const EXPRESSION: u8 = 11;

/// The MIDI controller number of modulation, the wheel libraries crossfade their dynamic layers
/// on.
pub const MODULATION: u8 = 1;

/// A part's dynamics as the events of one MIDI controller in its clip (see the module
/// documentation): control changes in the shape of the clip's controller events, ready for the
/// clip's `controls`, to write in place of the `dynamics` lane.
#[derive(Clone, Debug, PartialEq)]
pub struct ControllerLane {
    /// The controller number, 0-127: [`DynamicsKind::controller`] of the sound.
    pub number: u8,
    /// The MIDI channel of the events, 0-15: the channel of the part's first note, 0 for a part
    /// without notes.
    pub channel: u8,
    /// The events, at content beats of the part's clip (the beats of its notes) in strictly
    /// ascending order, each a control change of [`ControllerLane::number`] on
    /// [`ControllerLane::channel`] with a value of 0-127 that holds until the next event, and
    /// no two following events with one value. Empty for a part without notes.
    pub events: Vec<MidiControlEvent>,
}

impl ControllerLane {
    /// The same events on controller `number`, for a sound whose dynamic layers crossfade on a
    /// controller other than modulation: the values are the ones its `dynamics` parameter would
    /// write into that controller. `None` for a number above 127, which MIDI has no controller
    /// for.
    #[must_use]
    pub fn on_controller(&self, number: u8) -> Option<Self> {
        (number <= 127).then(|| Self {
            number,
            channel: self.channel,
            events: self
                .events
                .iter()
                .map(|event| MidiControlEvent {
                    number: i64::from(number),
                    ..event.clone()
                })
                .collect(),
        })
    }
}

/// The value of an automation lane at `beat`, as the engine evaluates it: before the first point
/// the first value, from the last point on the last value, and between two points the curve of
/// the segment that starts at the earlier one (`hold` keeps its value, `linear` runs straight,
/// `smooth` eases as a raised cosine). Of points that share a beat, the last one counts from that
/// beat on. A lane without points is 0.
#[must_use]
pub fn value_at(points: &[AutomationPoint], beat: f64) -> f64 {
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return 0.0;
    };
    if beat >= last.beat {
        return last.value;
    }
    if beat < first.beat {
        return first.value;
    }
    let index = points
        .partition_point(|point| point.beat <= beat)
        .saturating_sub(1);
    let (Some(from), Some(to)) = (points.get(index), points.get(index + 1)) else {
        return last.value;
    };
    let span = to.beat - from.beat;
    let t = if span > EPSILON {
        ((beat - from.beat) / span).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let shaped = match from.curve {
        CurveKind::Hold => 0.0,
        CurveKind::Smooth => 0.5 - 0.5 * (PI * t).cos(),
        CurveKind::Linear => t,
    };
    from.value + (to.value - from.value) * shaped
}

/// The share of a phrase at which a flat contour peaks, and of a note at which a swell peaks.
const PEAK_SHARE: f64 = 0.6;

/// The share of a roll at which its crescendo has made a third of its rise.
const ROLL_KNEE_SHARE: f64 = 0.7;

/// The share of the rise a roll's crescendo has made at [`ROLL_KNEE_SHARE`].
const ROLL_KNEE_RISE: f64 = 1.0 / 3.0;

/// A rest of at least this many seconds ends a phrase.
const PHRASE_REST_S: f64 = 0.6;

/// A note at least this many times the line's median length, followed by a rest, ends its phrase.
const LONG_NOTE_FACTOR: f64 = 2.0;

/// A phrase spans this many bars at most.
const MAX_PHRASE_BARS: f64 = 8.0;

/// A held note of an accompanying part at least this many beats long swells when the request asks
/// for swells.
const SWELL_BEATS: f64 = 2.0;

/// The most a held note's level may rise over its note-on's, as a ratio of `dynamics` values: the
/// sampler caps the gain of a held note at the square of it, +12 dB.
const HELD_RISE: f64 = 2.0;

/// Values closer than this count as one lane value.
const VALUE_EPSILON: f64 = 1e-12;

/// The dynamics of a line, and its notes written at their velocities.
pub(super) struct Shaped {
    pub(super) notes: Vec<MidiNote>,
    pub(super) dynamics: Dynamics,
    /// For each note, the factor its written velocity sounds at: its note-on plays
    /// `velocity_of(written · factor)`, which picks its layer. The lane at the note-on in velocity
    /// mode, 1 in controller mode, and `None` in fixed mode, where the written velocity does not
    /// sound.
    pub(super) scales: Vec<Option<f64>>,
}

/// Shapes the dynamics of `notes`, the articulated notes of `line` in the project's note order,
/// with `articulations` their articulations in the same order. `onsets` holds the note-ons each
/// onset group of `notes` is played at: the lane holds each onset's value from before the first
/// to the last of them ([`OnsetSpan`]).
pub(super) fn shape(
    line: &PartLine,
    score: &ScoreContext,
    notes: Vec<MidiNote>,
    articulations: &[NoteArticulation],
    onsets: &[OnsetSpan],
) -> Shaped {
    let kind = line.caps.dynamics;
    let part_level = part_marking(line.dynamics.marking, line.role);
    let clock = Clock::new(score);
    let groups = onset_groups(notes.iter().map(|note| note.start));
    let mut spans = phrase_spans(&notes, &groups, &clock, score);
    let level = Level::of(part_level);
    let mut points = base_points(&notes, &groups, &mut spans, level, sings(line.role));
    let remarks_bound = add_bumps(line, &notes, articulations, &groups, &spans, &mut points);
    let shape_points: Vec<AutomationPoint> = points.iter().map(|point| point.point).collect();
    let onset_levels: Vec<Level> = groups
        .iter()
        .map(|group| {
            let start = notes.get(group.start).map_or(0.0, |note| note.start);
            Level::from_steps(value_at(&shape_points, start))
        })
        .collect();
    let rings = Class::of(line.instrument.family()) == Class::Struck;
    let written = written_velocities(&spans, &points, &onset_levels, rings, kind);
    let placer = Lane {
        clock: &clock,
        score,
        kind,
        written: &written,
    };
    let mut lane_points = placer.points(
        &notes,
        &groups,
        onsets,
        &spans,
        &points,
        &shape_points,
        &onset_levels,
    );
    // The first note-on as played, as the lane places its onsets.
    let first_lead = notes.first().map(|note| {
        let first = onsets.first().map_or(note.start, |onset| onset.first);
        placer.lead(first, None)
    });
    let channel = notes.first().map_or(0, |note| note.channel);
    let controller = controller_lane(&lane_points, first_lead, kind, channel, &clock);
    let origin = score.origin();
    for point in &mut lane_points {
        point.beat += origin;
    }
    let mut remarks = remarks_bound;
    if let DynamicsKind::Fixed(velocity) = kind {
        let wanted = points
            .iter()
            .map(|point| Level::from_steps(point.point.value).velocity())
            .max()
            .unwrap_or(0);
        if wanted > velocity {
            remarks.push(Remark::FixedCeiling { velocity, wanted });
        }
    }
    let (notes, levels, scales) = write_notes(
        notes,
        articulations,
        &groups,
        &spans,
        &onset_levels,
        &written,
        kind,
    );
    let phrases = spans
        .iter()
        .zip(&written)
        .map(|(span, &written)| Phrase {
            start: span.start,
            end: span.end,
            peak: span.peak,
            written,
        })
        .collect();
    Shaped {
        notes,
        dynamics: Dynamics {
            kind,
            marking: part_level,
            phrases,
            notes: levels,
            lane: lane_points,
            controller,
            remarks,
        },
        scales,
    }
}

/// A phrase while it is shaped.
struct Span {
    /// Its onset groups.
    groups: Range<usize>,
    /// The content beat of its first note-on.
    start: f64,
    /// The content beat where its last note ends.
    end: f64,
    /// The content beat of an arc's peak.
    peak: Option<f64>,
}

/// The phrases of a line (see the module documentation).
fn phrase_spans(
    notes: &[MidiNote],
    groups: &[Range<usize>],
    clock: &Clock<'_>,
    score: &ScoreContext,
) -> Vec<Span> {
    let long = LONG_NOTE_FACTOR * median_length(notes);
    let origin = score.origin();
    let sections: Vec<f64> = score
        .sections()
        .iter()
        .map(|&start| start - origin)
        .collect();
    let bar = |beat: f64| score.signatures().beat_to_bar(origin + beat);
    let mut spans: Vec<Span> = Vec::new();
    let mut first_bar = 0.0;
    // Whether a note that ends where the phrase's notes end so far lasts at least `long`.
    let mut ends_long = false;
    let mut previous_start = f64::NEG_INFINITY;
    for (index, group) in groups.iter().enumerate() {
        let members = notes.get(group.clone()).unwrap_or_default();
        let Some(start) = members.first().map(|note| note.start) else {
            continue;
        };
        let ends = match spans.last() {
            None => true,
            Some(phrase) => {
                let gap = start - phrase.end;
                let rests = gap > BEAT_EPSILON
                    && (gap >= PHRASE_REST_BEATS - BEAT_EPSILON
                        || clock.span(phrase.end, start) >= PHRASE_REST_S - BEAT_EPSILON);
                let section = sections.iter().any(|&section| {
                    section > previous_start + BEAT_EPSILON && section <= start + BEAT_EPSILON
                });
                let after_long = gap > BEAT_EPSILON && ends_long;
                let full = bar(start) - first_bar >= MAX_PHRASE_BARS - BEAT_EPSILON;
                rests || section || after_long || full
            }
        };
        if ends {
            spans.push(Span {
                groups: index..index + 1,
                start,
                end: start,
                peak: None,
            });
            first_bar = bar(start);
            ends_long = false;
        }
        if let Some(phrase) = spans.last_mut() {
            phrase.groups.end = index + 1;
            for note in members {
                let end = note.end();
                if end > phrase.end + BEAT_EPSILON {
                    phrase.end = end;
                    ends_long = note.length >= long - BEAT_EPSILON;
                } else if (end - phrase.end).abs() <= BEAT_EPSILON {
                    ends_long |= note.length >= long - BEAT_EPSILON;
                }
            }
        }
        previous_start = start;
    }
    spans
}

/// The median length of the notes, in beats; 0 for no notes.
fn median_length(notes: &[MidiNote]) -> f64 {
    let mut lengths: Vec<f64> = notes.iter().map(|note| note.length).collect();
    lengths.sort_by(f64::total_cmp);
    let at = |index: usize| lengths.get(index).copied().unwrap_or(0.0);
    match lengths.len() {
        0 => 0.0,
        count => f64::midpoint(at((count - 1) / 2), at(count / 2)),
    }
}

/// A point of the level shape: a content beat and a level in marked steps (the point's value),
/// with the phrase it belongs to.
#[derive(Clone, Copy, Debug)]
struct ShapePoint {
    point: AutomationPoint,
    phrase: usize,
}

impl ShapePoint {
    const fn new(beat: f64, level: Level, curve: CurveKind, phrase: usize) -> Self {
        Self {
            point: AutomationPoint {
                beat,
                value: level.steps(),
                curve,
            },
            phrase,
        }
    }
}

/// The phrases' own shapes: an arc for a phrase of several onsets of a part that sings, the part's
/// level for every other. Records each arc's peak in its span.
fn base_points(
    notes: &[MidiNote],
    groups: &[Range<usize>],
    spans: &mut [Span],
    level: Level,
    arcs: bool,
) -> Vec<ShapePoint> {
    let mut points = Vec::new();
    let starts: Vec<f64> = spans.iter().map(|span| span.start).collect();
    for (index, span) in spans.iter_mut().enumerate() {
        let start = span.start;
        if !arcs || span.groups.len() < 2 {
            points.push(ShapePoint::new(start, level, CurveKind::Hold, index));
            continue;
        }
        // The arc ends where the phrase's notes end, or where the next phrase starts.
        let end = starts
            .get(index + 1)
            .map_or(span.end, |&next| span.end.min(next));
        let members = notes
            .get(span_notes(groups, &span.groups))
            .unwrap_or_default();
        let lowest = members.iter().map(|note| note.pitch).min();
        let highest = members.iter().map(|note| note.pitch).max();
        let peak = if lowest == highest {
            start + PEAK_SHARE * (end - start)
        } else {
            // The highest note's onset, the later one on ties: notes are in note order.
            members
                .iter()
                .filter(|note| Some(note.pitch) == highest)
                .map(|note| note.start)
                .fold(start, f64::max)
        };
        if peak > start + BEAT_EPSILON {
            points.push(ShapePoint::new(start, level, CurveKind::Smooth, index));
        }
        points.push(ShapePoint::new(
            peak,
            level.up(1.0),
            CurveKind::Smooth,
            index,
        ));
        if end > peak + BEAT_EPSILON {
            points.push(ShapePoint::new(end, level.up(-0.5), CurveKind::Hold, index));
        }
        span.peak = Some(peak);
    }
    points
}

/// The note indices of the onset groups `range`.
fn span_notes(groups: &[Range<usize>], range: &Range<usize>) -> Range<usize> {
    let first = groups.get(range.start).map_or(0, |group| group.start);
    let last = range
        .end
        .checked_sub(1)
        .and_then(|group| groups.get(group))
        .map_or(first, |group| group.end);
    first..last
}

/// The phrase that holds onset group `group`.
fn phrase_of(spans: &[Span], group: usize) -> usize {
    spans
        .iter()
        .position(|span| span.groups.contains(&group))
        .unwrap_or(0)
}

/// What an onset group asks of the level beyond its phrase's shape.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Bump {
    /// A step up at 60 % of the note and back.
    Swell,
    /// A crescendo to a marking, back-loaded.
    Roll(Dynamic),
}

/// Adds the swells and the roll crescendos to `points` (see the module documentation), and
/// returns the remarks about rolls that stop short of their marking.
fn add_bumps(
    line: &PartLine,
    notes: &[MidiNote],
    articulations: &[NoteArticulation],
    groups: &[Range<usize>],
    spans: &[Span],
    points: &mut Vec<ShapePoint>,
) -> Vec<Remark> {
    let bounded = matches!(
        line.caps.dynamics,
        DynamicsKind::Velocity | DynamicsKind::Fixed(_)
    );
    let mut remarks = Vec::new();
    let mut busy_until = f64::NEG_INFINITY;
    for (group_index, group) in groups.iter().enumerate() {
        let members = notes.get(group.clone()).unwrap_or_default();
        let asked = articulations.get(group.clone()).unwrap_or_default();
        let Some(start) = members.first().map(|note| note.start) else {
            continue;
        };
        let Some((bump, bumped)) = bump_of(line, members, asked) else {
            continue;
        };
        let end = bumped.iter().map(|note| note.end()).fold(start, f64::max);
        if start < busy_until - BEAT_EPSILON {
            continue;
        }
        // A roll grows only while it sounds alone: a note that started during its crescendo
        // could not follow the level back afterwards without rising more than a held note may.
        if matches!(bump, Bump::Roll(_))
            && notes
                .iter()
                .any(|note| note.start > start + BEAT_EPSILON && note.start < end - BEAT_EPSILON)
        {
            continue;
        }
        busy_until = end;
        let phrase = phrase_of(spans, group_index);
        let base: Vec<AutomationPoint> = points.iter().map(|point| point.point).collect();
        let from = Level::from_steps(value_at(&base, start));
        points.retain(|point| {
            point.phrase != phrase
                || point.point.beat < start - BEAT_EPSILON
                || point.point.beat > end + BEAT_EPSILON
        });
        let length = end - start;
        points.push(ShapePoint::new(start, from, CurveKind::Smooth, phrase));
        match bump {
            Bump::Swell => {
                points.push(ShapePoint::new(
                    start + PEAK_SHARE * length,
                    from.up(1.0),
                    CurveKind::Smooth,
                    phrase,
                ));
                let back = Level::from_steps(value_at(&base, end));
                points.push(ShapePoint::new(end, back, curve_at(&base, end), phrase));
            }
            Bump::Roll(target) => {
                let asked = Level::of(target);
                let ceiling = roll_ceiling(notes, &base, start, from);
                let reached = match ceiling {
                    Some(ceiling) if bounded && asked > ceiling => {
                        if let Some(roll) = bumped.first() {
                            remarks.push(Remark::BoundedRoll {
                                note: roll.id.clone(),
                                asked: target,
                                reached: ceiling,
                            });
                        }
                        ceiling
                    }
                    _ => asked,
                };
                let knee = from.up((reached.steps() - from.steps()) * ROLL_KNEE_RISE);
                points.push(ShapePoint::new(
                    start + ROLL_KNEE_SHARE * length,
                    knee,
                    CurveKind::Smooth,
                    phrase,
                ));
                points.push(ShapePoint::new(end, reached, CurveKind::Hold, phrase));
                // The next note of the phrase, if one follows the roll, returns to the phrase's
                // level.
                let next = (group_index + 1..groups.len()).find_map(|later| {
                    let beat = groups
                        .get(later)
                        .and_then(|members| notes.get(members.start))
                        .map(|note| note.start)?;
                    (beat >= end - BEAT_EPSILON).then_some((later, beat))
                });
                if let Some((later, beat)) = next
                    && phrase_of(spans, later) == phrase
                {
                    points.push(ShapePoint::new(
                        beat,
                        Level::from_steps(value_at(&base, beat)),
                        curve_at(&base, beat),
                        phrase,
                    ));
                }
            }
        }
        points.sort_by(|a, b| {
            a.point
                .beat
                .total_cmp(&b.point.beat)
                .then(a.phrase.cmp(&b.phrase))
        });
    }
    remarks
}

/// What the onset group of `members`, whose articulations are `asked`, asks beyond its phrase's
/// shape, with the notes that ask it: a roll crescendo for the rolls of a percussion part whose
/// request gives rolls a target; else nothing when the sound plays a recorded swell; else a swell
/// for the notes that ask for one, or for the held notes of an accompanying part whose request
/// asks for swells.
fn bump_of<'n>(
    line: &PartLine,
    members: &'n [MidiNote],
    asked: &[NoteArticulation],
) -> Option<(Bump, Vec<&'n MidiNote>)> {
    let notes_where = |wanted: &dyn Fn(&MidiNote, &NoteArticulation) -> bool| -> Vec<&'n MidiNote> {
        members
            .iter()
            .zip(asked)
            .filter(|(note, articulation)| wanted(note, articulation))
            .map(|(note, _)| note)
            .collect()
    };
    let request = line.dynamics;
    if let Some(target) = request.roll_to
        && line.instrument.family() == Family::Percussion
    {
        let rolls = notes_where(&|_, articulation| {
            matches!(
                articulation.asked,
                Some(Articulation::Roll | Articulation::Tremolo)
            )
        });
        if !rolls.is_empty() {
            return Some((Bump::Roll(target), rolls));
        }
    }
    let recorded =
        |articulation: &NoteArticulation| articulation.plays == Some(Articulation::CrescendoSwell);
    if asked.iter().any(recorded) {
        return None;
    }
    let swells =
        notes_where(&|_, articulation| articulation.asked == Some(Articulation::CrescendoSwell));
    if !swells.is_empty() {
        return Some((Bump::Swell, swells));
    }
    if request.swells && accompanies(line.role) {
        let held = notes_where(&|note, _| note.length >= SWELL_BEATS - BEAT_EPSILON);
        if !held.is_empty() {
            return Some((Bump::Swell, held));
        }
    }
    None
}

/// The loudest level a roll starting at content beat `start` at level `from` may grow to, when
/// the sound's level follows `dynamics` as a held note's gain: twice the level velocity of the
/// softest note sounding at its start, the roll included, at its own note-on (`shape` gives the
/// levels so far). `None` when that lies beyond ff.
fn roll_ceiling(
    notes: &[MidiNote],
    shape: &[AutomationPoint],
    start: f64,
    from: Level,
) -> Option<Level> {
    let softest = notes
        .iter()
        .filter(|note| note.start <= start + BEAT_EPSILON && note.end() > start + BEAT_EPSILON)
        .map(|note| Level::from_steps(value_at(shape, note.start)).velocity())
        .min()
        .unwrap_or_else(|| from.velocity());
    let ceiling = HELD_RISE * f64::from(softest);
    (ceiling < f64::from(level_velocity(Dynamic::Fortissimo)))
        .then(|| Level::from_velocity(ceiling))
}

/// The curve of the segment of `points` that holds `beat`: the curve of the last point at or
/// before it, or `hold` before the first point.
fn curve_at(points: &[AutomationPoint], beat: f64) -> CurveKind {
    points
        .partition_point(|point| point.beat <= beat + BEAT_EPSILON)
        .checked_sub(1)
        .and_then(|index| points.get(index))
        .map_or(CurveKind::Hold, |point| point.curve)
}

/// In velocity mode, the velocity each phrase's notes are written at: the level velocity of the
/// loudest level the shape reaches in the phrase and in every phrase joined to it, by a note that
/// sounds across their boundary or, for a part whose notes ring, by the whole line. `None` for
/// every phrase in the other modes.
fn written_velocities(
    spans: &[Span],
    points: &[ShapePoint],
    onset_levels: &[Level],
    rings: bool,
    kind: DynamicsKind,
) -> Vec<Option<u8>> {
    if kind != DynamicsKind::Velocity {
        return vec![None; spans.len()];
    }
    // The loudest level velocity of each phrase.
    let loudest: Vec<u8> = spans
        .iter()
        .enumerate()
        .map(|(index, span)| {
            let shaped = points
                .iter()
                .filter(|point| point.phrase == index)
                .map(|point| Level::from_steps(point.point.value).velocity());
            let onsets = onset_levels
                .get(span.groups.clone())
                .unwrap_or_default()
                .iter()
                .map(|level| level.velocity());
            shaped.chain(onsets).max().unwrap_or(1)
        })
        .collect();
    // Runs of joined phrases.
    let mut runs: Vec<Range<usize>> = Vec::new();
    let mut latest_end = f64::NEG_INFINITY;
    for (index, span) in spans.iter().enumerate() {
        match runs.last_mut() {
            Some(run) if rings || latest_end > span.start + BEAT_EPSILON => run.end = index + 1,
            _ => runs.push(index..index + 1),
        }
        latest_end = latest_end.max(span.end);
    }
    let mut written = vec![None; spans.len()];
    for run in runs {
        let velocity = loudest
            .get(run.clone())
            .unwrap_or_default()
            .iter()
            .copied()
            .max()
            .unwrap_or(1);
        for slot in written.get_mut(run).into_iter().flatten() {
            *slot = Some(velocity);
        }
    }
    written
}

/// Places the points of the `dynamics` lane.
struct Lane<'a> {
    clock: &'a Clock<'a>,
    score: &'a ScoreContext,
    kind: DynamicsKind,
    /// The written velocity of each phrase in velocity mode.
    written: &'a [Option<u8>],
}

impl Lane<'_> {
    /// The lane's value for `level` in phrase `phrase`: in velocity mode its level velocity over
    /// the phrase's written velocity, in fixed mode over the fixed velocity (at most 1), and in
    /// controller mode its controller value.
    fn value(&self, level: Level, phrase: usize) -> f64 {
        let velocity = f64::from(level.velocity());
        match self.kind {
            DynamicsKind::Velocity => {
                let written = self
                    .written
                    .get(phrase)
                    .copied()
                    .flatten()
                    .map_or(velocity, f64::from);
                (velocity / written).min(1.0)
            }
            DynamicsKind::Fixed(fixed) => (velocity / f64::from(fixed.max(1))).min(1.0),
            DynamicsKind::Cc => level.cc(),
        }
    }

    /// The content beat from which the value at the note-on `start` holds: two blocks before it,
    /// or halfway after the previous note-on `previous` when that is later, and never before
    /// beat 0.
    fn lead(&self, start: f64, previous: Option<f64>) -> f64 {
        let lead_s = f64::from(SWITCH_LEAD_BLOCKS) * block_seconds();
        let start_s = self.clock.seconds(start);
        let mut at_s = start_s - lead_s;
        if let Some(previous) = previous {
            at_s = at_s.max(f64::midpoint(self.clock.seconds(previous), start_s));
        }
        let beat = self.score.tempo().seconds_to_beat(at_s) - self.score.origin();
        beat.clamp(0.0, start)
    }

    /// The lane's points at content beats: at each onset its level, held from the lead of its
    /// first note-on to its last note-on (`onsets`, one per onset group, as played), where the
    /// shape's curve from the onset as written continues, and between the onsets the shape's own
    /// points.
    #[expect(
        clippy::too_many_arguments,
        reason = "the lane reads the notes, their onsets as written and as played, the phrases \
                  and the shape"
    )]
    fn points(
        &self,
        notes: &[MidiNote],
        groups: &[Range<usize>],
        onsets: &[OnsetSpan],
        spans: &[Span],
        points: &[ShapePoint],
        shape: &[AutomationPoint],
        onset_levels: &[Level],
    ) -> Vec<AutomationPoint> {
        let mut lane: Vec<AutomationPoint> = Vec::with_capacity(2 * groups.len() + points.len());
        let mut cursor = 0;
        let mut previous: Option<f64> = None;
        for (group_index, group) in groups.iter().enumerate() {
            let Some(start) = notes.get(group.start).map(|note| note.start) else {
                continue;
            };
            let phrase = phrase_of(spans, group_index);
            let onset = onsets.get(group_index).copied().unwrap_or(OnsetSpan {
                first: start,
                last: start,
            });
            let lead = self.lead(onset.first, previous);
            // The next onset's lead lies after this onset's last note-on, so the hold never runs
            // into it; as written, the last note-on is the onset's start.
            let held_until = onset.last;
            let level = onset_levels.get(group_index).copied().unwrap_or(Level(0.0));
            // The shape's points before the onset end where the onset's value starts to hold, and
            // so do those at the onset that close what came before it: the end of an earlier
            // phrase, or a crescendo that the onset's own level then leaves.
            while let Some(point) = points.get(cursor).filter(|point| {
                point.point.beat < start - BEAT_EPSILON
                    || (point.point.beat <= start + BEAT_EPSILON
                        && (point.phrase != phrase
                            || (point.point.value - level.steps()).abs() > VALUE_EPSILON))
            }) {
                let value = self.value(Level::from_steps(point.point.value), point.phrase);
                push(
                    &mut lane,
                    point.point.beat.min(lead),
                    value,
                    point.point.curve,
                );
                cursor += 1;
            }
            // The onset's own points: its value stands for them.
            while points
                .get(cursor)
                .is_some_and(|point| point.point.beat <= start + BEAT_EPSILON)
            {
                cursor += 1;
            }
            let value = self.value(level, phrase);
            // Before its first point a lane holds the first value, so the first onset needs no
            // lead.
            if !lane.is_empty() && lead < held_until - BEAT_EPSILON {
                push(&mut lane, lead, value, CurveKind::Hold);
            }
            push(&mut lane, held_until, value, curve_at(shape, start));
            previous = Some(onset.last);
        }
        for point in points.get(cursor..).unwrap_or_default() {
            let value = self.value(Level::from_steps(point.point.value), point.phrase);
            push(&mut lane, point.point.beat, value, point.point.curve);
        }
        lane
    }
}

/// Appends a point at content beat `beat` (at least the last point's beat) unless it repeats a
/// held value: a `hold` point with the value of a `hold` point before it changes nothing.
fn push(lane: &mut Vec<AutomationPoint>, beat: f64, value: f64, curve: CurveKind) {
    let beat = lane.last().map_or(beat, |last| beat.max(last.beat));
    if let Some(last) = lane.last()
        && last.curve == CurveKind::Hold
        && curve == CurveKind::Hold
        && (last.value - value).abs() <= VALUE_EPSILON
    {
        return;
    }
    lane.push(AutomationPoint { beat, value, curve });
}

/// The events of the controller that carries `lane`, the points of the `dynamics` lane at content
/// beats, on `channel` (see the module documentation). The first value holds from `first_lead`,
/// the content beat from which the first note-on's value holds, when it lies before the lane's
/// first point.
fn controller_lane(
    lane: &[AutomationPoint],
    first_lead: Option<f64>,
    kind: DynamicsKind,
    channel: u8,
    clock: &Clock<'_>,
) -> ControllerLane {
    let value = |d: f64| controller_value(kind, d);
    let mut steps: Vec<(f64, u8)> = Vec::new();
    if let Some(first) = lane.first() {
        let from = first_lead.map_or(first.beat, |lead| lead.min(first.beat));
        step(&mut steps, from, value(first.value));
    }
    let block_s = block_seconds();
    for (index, point) in lane.iter().enumerate() {
        step(&mut steps, point.beat, value(point.value));
        let Some(next) = lane.get(index + 1) else {
            break;
        };
        if point.curve == CurveKind::Hold || next.beat - point.beat <= BEAT_EPSILON {
            continue;
        }
        // Along the curve one value a block, the last a block before the curve's next point.
        let from_s = clock.seconds(point.beat);
        let last_s = clock.seconds(next.beat) - block_s;
        let mut count: u32 = 1;
        loop {
            let at_s = from_s + f64::from(count) * block_s;
            if at_s > last_s {
                break;
            }
            let beat = clock.beat(at_s);
            step(&mut steps, beat, value(value_at(lane, beat)));
            let Some(more) = count.checked_add(1) else {
                break;
            };
            count = more;
        }
    }
    let number = kind.controller();
    let events = steps
        .into_iter()
        .map(|(beat, value)| MidiControlEvent {
            beat: beat.max(0.0),
            kind: ControlKind::Cc,
            value: i64::from(value),
            number: i64::from(number),
            channel: i64::from(channel),
        })
        .collect();
    ControllerLane {
        number,
        channel,
        events,
    }
}

/// The controller value that gives the level of the `dynamics` value `d` on a sound of `kind`:
/// `127 · d²` on expression, the sampler's gain for `d` in velocity and fixed mode, and `127 · d`,
/// the value `dynamics` writes, in controller mode; rounded half away from zero, as the sampler
/// rounds the parameter, and kept within 0-127.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is rounded and kept within 0-127 first"
)]
fn controller_value(kind: DynamicsKind, d: f64) -> u8 {
    let share = match kind {
        DynamicsKind::Velocity | DynamicsKind::Fixed(_) => d * d,
        DynamicsKind::Cc => d,
    };
    (share * 127.0).round().clamp(0.0, 127.0) as u8
}

/// Appends the controller value `value` from content beat `beat` on: it replaces an event at the
/// last event's beat, as the later of two lane points at one beat counts, and an event that
/// repeats the value in force is left out.
fn step(steps: &mut Vec<(f64, u8)>, beat: f64, value: u8) {
    if steps
        .last()
        .is_some_and(|&(last, _)| beat <= last + BEAT_EPSILON)
    {
        steps.pop();
    }
    if steps.last().is_none_or(|&(_, last)| last != value) {
        steps.push((beat, value));
    }
}

/// The notes written at their velocities, with the level of each and the factor its written
/// velocity sounds at ([`Shaped::scales`]).
fn write_notes(
    mut notes: Vec<MidiNote>,
    articulations: &[NoteArticulation],
    groups: &[Range<usize>],
    spans: &[Span],
    onset_levels: &[Level],
    written: &[Option<u8>],
    kind: DynamicsKind,
) -> (Vec<MidiNote>, Vec<NoteLevel>, Vec<Option<f64>>) {
    // The median velocity the notes of each phrase were given, before they are rewritten.
    let medians: Vec<f64> = spans
        .iter()
        .map(|span| {
            median_velocity(
                notes
                    .get(span_notes(groups, &span.groups))
                    .unwrap_or_default(),
            )
        })
        .collect();
    let mut levels = Vec::with_capacity(notes.len());
    let mut scales = Vec::with_capacity(notes.len());
    for (group_index, group) in groups.iter().enumerate() {
        let phrase = phrase_of(spans, group_index);
        let median = medians.get(phrase).copied().unwrap_or(0.0);
        let level = onset_levels.get(group_index).copied().unwrap_or(Level(0.0));
        let at_level = f64::from(level.velocity());
        for index in group.clone() {
            let Some(note) = notes.get_mut(index) else {
                continue;
            };
            let target = level.up(accent(note, articulations.get(index), median));
            let (velocity, plays, scale) = match kind {
                DynamicsKind::Velocity => {
                    let phrase_written = written.get(phrase).copied().flatten();
                    let full = f64::from(phrase_written.unwrap_or_else(|| level.velocity()));
                    let lane = at_level / full;
                    let velocity = if target == level {
                        velocity_of(full)
                    } else {
                        velocity_of(f64::from(target.velocity()) / lane)
                    };
                    (
                        velocity,
                        velocity_of(f64::from(velocity) * lane),
                        Some(lane),
                    )
                }
                DynamicsKind::Fixed(fixed) => {
                    let lane = (at_level / f64::from(fixed.max(1))).min(1.0);
                    (
                        target.velocity(),
                        velocity_of(f64::from(fixed) * lane),
                        None,
                    )
                }
                DynamicsKind::Cc => (target.velocity(), target.velocity(), Some(1.0)),
            };
            note.velocity = velocity;
            scales.push(scale);
            levels.push(NoteLevel {
                note: note.id.clone(),
                level: target,
                velocity: plays,
            });
        }
    }
    (notes, levels, scales)
}

/// A velocity rounded as Python rounds and kept within 1-127.
pub(super) fn velocity_of(value: f64) -> u8 {
    u8::try_from(py_round(value).clamp(1, 127)).expect("a velocity clamped to 1-127 fits a u8")
}

/// The median velocity of `notes`; 0 for no notes.
fn median_velocity(notes: &[MidiNote]) -> f64 {
    let mut velocities: Vec<u8> = notes.iter().map(|note| note.velocity).collect();
    velocities.sort_unstable();
    let at = |index: usize| velocities.get(index).copied().map_or(0.0, f64::from);
    match velocities.len() {
        0 => 0.0,
        count => f64::midpoint(at((count - 1) / 2), at(count / 2)),
    }
}

/// The marked steps a note plays above its level: one louder for a velocity accent that did not
/// become `marcato` (at least [`ACCENT_VELOCITY`] above its phrase's median `median`) or for a
/// rewrite that asks for an accent, one softer for a rewrite that asks for it, none otherwise.
fn accent(note: &MidiNote, articulation: Option<&NoteArticulation>, median: f64) -> f64 {
    let rewrite = articulation.and_then(|articulation| articulation.rewrite);
    let marcato =
        articulation.is_some_and(|articulation| articulation.plays == Some(Articulation::Marcato));
    let by_velocity = !marcato && f64::from(note.velocity) >= median + ACCENT_VELOCITY;
    let louder = by_velocity || rewrite == Some(Emulation::Accent);
    let softer = rewrite == Some(Emulation::Soften);
    f64::from(i8::from(louder) - i8::from(softer))
}
