//! Humanization: the small deviations of real players from the written notes, and the attack
//! compensation that makes a slow onset sound on the beat.
//!
//! **Deviations by family.** Players of each family drift from the written notes by their own
//! amounts ([`profile`]): a timing offset drawn from a normal distribution with the family's
//! standard deviation and bounded at twice it ([`TIMING_BOUND`]), a velocity step and a share of
//! the note's length, both drawn evenly from the family's range. The notes struck together in
//! one part (a chord, a divisi) share one timing offset and each moves a little from it
//! ([`Profile::spread_ms`]); on a keyboard or a harp the chord keeps its voicing, so a lower note
//! never starts after a higher one. A timing offset moves the whole note; the length share moves
//! its end.
//!
//! | Family | Timing σ | Velocity ± | Length ± | Chord spread ± |
//! |---|---|---|---|---|
//! | String section, choir | 8 ms | 4 | 5 % | 2 ms |
//! | Solo string, solo voice, woodwinds, other | 6 ms | 5 | 4 % | 2 ms |
//! | Brass | 8 ms | 5 | 4 % | 2 ms |
//! | Percussion | 3 ms | 6 | 0 | 0 |
//! | Keyboards, plucked strings | 4 ms | 6 | 3 % | 2 ms, in voicing order |
//!
//! The values are chosen to stay below audible flamming and are starting values, to be confirmed
//! by listening. The draws come from the part's own stream ([`Seed::stream`]): per onset group in
//! time order, its timing offset, then the spreads of a chord's notes, then per note its velocity
//! step and its length share. A draw whose range is zero is skipped. So the same line, score and
//! seed give the same deviations, and another part never changes them (TST-04).
//!
//! **Attack compensation.** A sound whose onset is slow sounds late: its note-on comes before the
//! audible start by the sound's attack. So a note-on moves earlier by the attack of the
//! articulation that sounds for it, the one its passage switches the sampler to
//! ([`SoundCaps::attack_ms`]); its note-off stays where it was written, so the audible note keeps
//! its written length. A note the overlap legato crossfades into, because another note at another
//! pitch is still held when it starts in a legato passage, fades in over the crossfade past its
//! sample's attack, so it moves by half the crossfade ([`LegatoKind::Overlap`]). An articulation
//! whose onset is not known moves nothing ([`Compensation::Unknown`]): the note is reported, never
//! guessed.
//!
//! **What the moves may not do** (TST-07):
//!
//! - a note never starts before its clip's beat 0; a note-on that would is held there;
//! - the note-ons keep their written order: every note of an onset group starts after every note
//!   of the group before it, at least half their written distance after the latest of them, so a
//!   fast run never collapses and a switch of articulation always fits between two passages;
//! - a note that ended before a later onset in writing still ends by that onset's earliest
//!   note-on, so no overlap is created and no repeated pitch is cut by its own note-off;
//! - a note that overlapped a later note at another pitch keeps overlapping it by the written
//!   overlap or 10 ms, whichever is less, so a legato join survives;
//! - a velocity stays within 1-127 and, where the written velocity sounds, the note-on stays in the
//!   velocity layer its dynamics chose ([`SoundCaps::layer`]).
//!
//! The note-ons the clip start or the order hold back are reported as compensated less than
//! wanted ([`Compensation::Advanced`]). Everything is a pure function of the line, the score and
//! the seed. Nothing here runs on a real-time thread (RT-01).

use std::ops::Range;

use sonara_core::midi::MidiNote;

use super::articulate::{Analysis, BEAT_EPSILON, Clock, written_onsets};
use super::check::note_order;
use super::dynamics::{NoteLevel, velocity_of};
use super::{LegatoKind, NoteArticulation, OnsetSpan, PartLine, ScoreContext, Seed, SoundCaps};
use crate::orchestration::{Articulation, Family, Forms, Instrument};

/// A timing offset is bounded at this many standard deviations of its family on either side.
pub const TIMING_BOUND: f64 = 2.0;

/// A note-on stays at least this share of the written distance after the latest note-on of the
/// onset group before it.
const ORDER_SHARE: f64 = 0.5;

/// A written overlap of two notes at different pitches is kept at least this long, in seconds,
/// or as long as it was written when that is shorter.
const KEEP_OVERLAP_S: f64 = 0.010;

/// Two times in seconds closer than this count as one.
const TIME_EPSILON: f64 = 1e-12;

/// What a request asks of the humanization of a part.
///
/// [`HumanizeRequest::default`] humanizes: the family's deviations and attack compensation, with
/// a string part played by a section.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HumanizeRequest {
    /// Whether the notes get their family's random deviations of timing, velocity and length.
    pub deviations: bool,
    /// Whether each note-on moves earlier by the attack of the sound that plays it.
    pub attack_compensation: bool,
    /// Whether one player plays the part, not a section: it selects the soloist's deviations for
    /// an instrument that plays both ways (the violin, the viola, the cello and the double bass).
    /// An instrument recorded only as a soloist or only as a section ignores it.
    pub solo: bool,
}

impl HumanizeRequest {
    /// No humanization at all: the notes play exactly as written.
    pub const EXACT: Self = Self {
        deviations: false,
        attack_compensation: false,
        solo: false,
    };
}

impl Default for HumanizeRequest {
    fn default() -> Self {
        Self {
            deviations: true,
            attack_compensation: true,
            solo: false,
        }
    }
}

/// How far one family's players move from the written notes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    /// The standard deviation of a timing offset, in milliseconds; an offset is bounded at
    /// [`TIMING_BOUND`] times it.
    pub timing_ms: f64,
    /// The most velocity steps a note moves either way.
    pub velocity: u8,
    /// The largest share of its written length by which a note's end moves either way.
    pub length: f64,
    /// The most milliseconds each note of a chord moves either way from the chord's shared
    /// offset.
    pub spread_ms: f64,
    /// Whether a chord keeps its voicing order: a lower note never starts after a higher one.
    pub voiced_chords: bool,
}

/// The deviations of a string section and of a choir section.
pub const SECTION_PROFILE: Profile = Profile {
    timing_ms: 8.0,
    velocity: 4,
    length: 0.05,
    spread_ms: 2.0,
    voiced_chords: false,
};

/// The deviations of a solo string player, a solo singer, a woodwind player and any other
/// instrument.
pub const SOLO_PROFILE: Profile = Profile {
    timing_ms: 6.0,
    velocity: 5,
    length: 0.04,
    spread_ms: 2.0,
    voiced_chords: false,
};

/// The deviations of a brass player.
pub const BRASS_PROFILE: Profile = Profile {
    timing_ms: 8.0,
    velocity: 5,
    length: 0.04,
    spread_ms: 2.0,
    voiced_chords: false,
};

/// The deviations of a percussionist: notes struck together share one offset exactly, and a
/// note's length, which only damps a ringing stroke, stays.
pub const PERCUSSION_PROFILE: Profile = Profile {
    timing_ms: 3.0,
    velocity: 6,
    length: 0.0,
    spread_ms: 0.0,
    voiced_chords: false,
};

/// The deviations of a keyboard player and of a harpist or other player of plucked strings.
pub const KEYBOARD_PROFILE: Profile = Profile {
    timing_ms: 4.0,
    velocity: 6,
    length: 0.03,
    spread_ms: 2.0,
    voiced_chords: true,
};

/// The deviations of the players of `instrument` under `request`, by its family: strings and
/// voices by whether a section plays them ([`HumanizeRequest::solo`] decides for an instrument
/// that plays both ways), woodwinds and other instruments as soloists, brass, percussion, and
/// keyboards with plucked strings.
#[must_use]
pub fn profile(instrument: &Instrument, request: HumanizeRequest) -> Profile {
    match instrument.family() {
        Family::Strings | Family::Choir => {
            let section = match instrument.ensemble().map(|profile| profile.forms) {
                Some(Forms::Section) => true,
                Some(Forms::Solo) => false,
                Some(Forms::Both) | None => !request.solo,
            };
            if section {
                SECTION_PROFILE
            } else {
                SOLO_PROFILE
            }
        }
        Family::Woodwinds | Family::Other => SOLO_PROFILE,
        Family::Brass => BRASS_PROFILE,
        Family::Percussion => PERCUSSION_PROFILE,
        Family::Keyboards | Family::Plucked => KEYBOARD_PROFILE,
    }
}

/// How far a note-on moved earlier for the attack of its sound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Compensation {
    /// The request asks for no attack compensation.
    Off,
    /// The onset of the articulation that sounds for the note is not known, so its note-on
    /// stays.
    Unknown,
    /// The note-on moved `advanced_ms` earlier of the `wanted_ms` the sound's attack asks for;
    /// less when the clip's start or the note-ons before it leave no room.
    Advanced {
        /// How far the attack asks the note-on to move, in milliseconds.
        wanted_ms: f64,
        /// How far it moved, in milliseconds: from 0 to `wanted_ms`.
        advanced_ms: f64,
    },
}

impl Compensation {
    /// Whether the note sounds later than its attack asks: its onset is not known, or its
    /// note-on moved less than wanted. Compensation that was not asked for never falls short.
    #[must_use]
    pub fn falls_short(self) -> bool {
        match self {
            Self::Off => false,
            Self::Unknown => true,
            Self::Advanced {
                wanted_ms,
                advanced_ms,
            } => advanced_ms < wanted_ms - 1e-6,
        }
    }
}

/// How one note was humanized.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteTiming {
    /// The id of the note.
    pub note: String,
    /// How far its note-on moved earlier for its sound's attack.
    pub compensation: Compensation,
    /// The timing offset drawn for it, in milliseconds, later when positive: its onset group's
    /// bounded offset plus its own spread in a chord. 0 without deviations. The note moves by it
    /// unless the clip's start or the order of the note-ons holds it.
    pub offset_ms: f64,
    /// The velocity steps drawn for it. Its written velocity moves by them unless 1-127 or its
    /// velocity layer stops it.
    pub velocity: i16,
    /// The share of its written length drawn for it: its end moves by that share of its length,
    /// unless a later note's start or a kept overlap holds it.
    pub length: f64,
}

/// How the notes of a performance were humanized.
#[derive(Clone, Debug, PartialEq)]
pub struct Humanized {
    /// What the request asked.
    pub request: HumanizeRequest,
    /// How each note was humanized, in the order of the performance's notes.
    pub notes: Vec<NoteTiming>,
}

impl Humanized {
    /// The notes that sound later than their sound's attack asks ([`Compensation::falls_short`]),
    /// in the order of the performance's notes.
    pub fn uncompensated(&self) -> impl Iterator<Item = &NoteTiming> {
        self.notes
            .iter()
            .filter(|timing| timing.compensation.falls_short())
    }
}

/// The deviations drawn for one note.
#[derive(Clone, Copy, Debug)]
struct Drawn {
    /// The timing offset, in seconds.
    offset_s: f64,
    velocity: i16,
    length: f64,
}

impl Drawn {
    const NONE: Self = Self {
        offset_s: 0.0,
        velocity: 0,
        length: 0.0,
    };
}

/// The notes of a line as played, before their velocities move.
pub(super) struct Timed {
    /// The note-ons of each onset group of the analysed notes.
    pub(super) onsets: Vec<OnsetSpan>,
    /// The start and the length of each analysed note, in content beats.
    pub(super) spans: Vec<(f64, f64)>,
    /// How each analysed note is humanized, in their order.
    pub(super) notes: Vec<NoteTiming>,
}

/// Times the analysed notes of `line` (see the module documentation): draws their deviations
/// from `seed`'s stream of the part, moves their note-ons by the deviations and the attacks, and
/// keeps the moves inside their limits.
pub(super) fn time(
    line: &PartLine,
    score: &ScoreContext,
    seed: Seed,
    analysis: &Analysis,
) -> Timed {
    let request = line.humanize;
    let notes = &analysis.notes;
    let groups = &analysis.groups;
    if !request.deviations && !request.attack_compensation {
        return Timed::exact(notes, groups);
    }
    let family = profile(line.instrument, request);
    let drawn = draw(line, seed, family, groups, notes.len());
    let clock = Clock::new(score);
    let (on_s, compensations) = note_ons(line, analysis, &clock, &drawn);
    let starts: Vec<f64> = notes
        .iter()
        .zip(&on_s)
        .map(|(note, &on)| beat_at(&clock, on, note.start).max(0.0))
        .collect();
    let onsets: Vec<OnsetSpan> = groups
        .iter()
        .map(|group| {
            let members = starts.get(group.clone()).unwrap_or_default();
            OnsetSpan {
                first: members.iter().copied().fold(f64::INFINITY, f64::min),
                last: members.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            }
        })
        .collect();
    let ends = ends(
        &Line {
            notes,
            groups,
            clock: &clock,
            on_s: &on_s,
            starts: &starts,
            onsets: &onsets,
        },
        &drawn,
        if request.deviations {
            family.length
        } else {
            0.0
        },
    );
    let spans = notes
        .iter()
        .zip(starts.iter().zip(&ends))
        .map(|(note, (&start, &end))| {
            if same(start, note.start) && same(end, note.end()) {
                (note.start, note.length)
            } else {
                // The end lies after the start: a later onset that caps it starts at least half
                // a written distance after this note's note-on. The floor only guards rounding.
                (start, (end - start).max(BEAT_EPSILON))
            }
        })
        .collect();
    let timings = notes
        .iter()
        .zip(drawn.iter().zip(compensations))
        .map(|(note, (drawn, compensation))| NoteTiming {
            note: note.id.clone(),
            compensation,
            offset_ms: 1000.0 * drawn.offset_s,
            velocity: drawn.velocity,
            length: drawn.length,
        })
        .collect();
    Timed {
        onsets,
        spans,
        notes: timings,
    }
}

impl Timed {
    /// The notes exactly as written.
    fn exact(notes: &[MidiNote], groups: &[Range<usize>]) -> Self {
        Self {
            onsets: written_onsets(notes, groups),
            spans: notes.iter().map(|note| (note.start, note.length)).collect(),
            notes: notes
                .iter()
                .map(|note| NoteTiming {
                    note: note.id.clone(),
                    compensation: Compensation::Off,
                    offset_ms: 0.0,
                    velocity: 0,
                    length: 0.0,
                })
                .collect(),
        }
    }
}

/// The note-on of each analysed note in seconds, onset group by onset group, with the
/// compensation it keeps: its written start, earlier by its attack and moved by its drawn
/// offset, but never before the clip's beat 0 nor closer to the latest note-on of the group
/// before than half their written distance.
fn note_ons(
    line: &PartLine,
    analysis: &Analysis,
    clock: &Clock<'_>,
    drawn: &[Drawn],
) -> (Vec<f64>, Vec<Compensation>) {
    let compensate = line.humanize.attack_compensation;
    let notes = &analysis.notes;
    let zero_s = clock.seconds(0.0);
    let mut on_s = Vec::with_capacity(notes.len());
    let mut compensations = Vec::with_capacity(notes.len());
    let mut previous: Option<(f64, f64)> = None;
    for group in &analysis.groups {
        let onset_s = clock.seconds(notes.get(group.start).map_or(0.0, |note| note.start));
        let floor = previous.map_or(zero_s, |(previous_s, latest)| {
            (latest + ORDER_SHARE * (onset_s - previous_s)).max(zero_s)
        });
        let mut latest = f64::NEG_INFINITY;
        for index in group.clone() {
            let written_s = clock.seconds(notes.get(index).map_or(0.0, |note| note.start));
            let offset = drawn.get(index).map_or(0.0, |drawn| drawn.offset_s);
            let wanted_ms = if compensate {
                attack_ms(line, analysis, index)
            } else {
                None
            };
            let advance = wanted_ms.map_or(0.0, |ms| ms / 1000.0);
            let desired = written_s - advance + offset;
            let on = desired.max(floor);
            on_s.push(on);
            compensations.push(match (compensate, wanted_ms) {
                (false, _) => Compensation::Off,
                (true, None) => Compensation::Unknown,
                (true, Some(wanted_ms)) => {
                    // What the clip's start or the note-ons before held back comes off the
                    // advance first; the drawn offset moves the note either way.
                    let held = on - desired;
                    let advanced_ms = if held > 0.0 {
                        1000.0 * (advance - held).clamp(0.0, advance)
                    } else {
                        wanted_ms
                    };
                    Compensation::Advanced {
                        wanted_ms,
                        advanced_ms,
                    }
                }
            });
            latest = latest.max(on);
        }
        previous = Some((onset_s, latest));
    }
    (on_s, compensations)
}

/// The deviations of every note of `line`, drawn from the part's stream of `seed` in the order of
/// the module documentation; none when the request asks for none.
fn draw(
    line: &PartLine,
    seed: Seed,
    family: Profile,
    groups: &[Range<usize>],
    count: usize,
) -> Vec<Drawn> {
    let mut drawn = vec![Drawn::NONE; count];
    if !line.humanize.deviations {
        return drawn;
    }
    let mut stream = seed.stream(&line.part);
    let sigma = family.timing_ms / 1000.0;
    let bound = TIMING_BOUND * sigma;
    let spread = family.spread_ms / 1000.0;
    let velocity = i64::from(family.velocity);
    for group in groups {
        let shared = if sigma > 0.0 {
            stream.gauss(0.0, sigma).clamp(-bound, bound)
        } else {
            0.0
        };
        let chord = group.len() > 1 && spread > 0.0;
        let mut spreads: Vec<f64> = group
            .clone()
            .map(|_| {
                if chord {
                    stream.uniform(-spread, spread)
                } else {
                    0.0
                }
            })
            .collect();
        if family.voiced_chords {
            // The notes of a group lie in note order, the lowest pitch first.
            spreads.sort_by(f64::total_cmp);
        }
        for (slot, spread) in drawn
            .get_mut(group.clone())
            .into_iter()
            .flatten()
            .zip(spreads)
        {
            let step = if velocity > 0 {
                stream.randint(-velocity, velocity)
            } else {
                0
            };
            let length = if family.length > 0.0 {
                stream.uniform(-family.length, family.length)
            } else {
                0.0
            };
            *slot = Drawn {
                offset_s: shared + spread,
                velocity: i16::try_from(step)
                    .expect("a step of at most 255 either way fits an i16"),
                length,
            };
        }
    }
    drawn
}

/// The attack in milliseconds that note `index` of the analysis asks its note-on to move by:
/// half the crossfade for a note the overlap legato crossfades into, else the attack the sound
/// records for the articulation of the note's passage; `None` when that is not known.
fn attack_ms(line: &PartLine, analysis: &Analysis, index: usize) -> Option<f64> {
    let sounding = analysis
        .articulations
        .get(index)
        .and_then(|articulation: &NoteArticulation| analysis.drafts.get(articulation.passage))
        .and_then(|draft| draft.articulation);
    if let LegatoKind::Overlap { crossfade_ms } = line.caps.legato
        && sounding == Some(Articulation::Legato)
        && is_held_into(&analysis.notes, index)
    {
        return Some(crossfade_ms / 2.0);
    }
    line.caps
        .attack_ms
        .iter()
        .find(|(articulation, _)| *articulation == sounding)
        .map(|&(_, ms)| ms)
}

/// Whether a note of `notes` (in note order) that started before note `index`, at another pitch,
/// still sounds when it starts: the sampler's overlap legato then crossfades into it.
fn is_held_into(notes: &[MidiNote], index: usize) -> bool {
    let Some(note) = notes.get(index) else {
        return false;
    };
    notes
        .get(..index)
        .unwrap_or_default()
        .iter()
        .any(|earlier| {
            earlier.start < note.start - BEAT_EPSILON
                && earlier.end() > note.start + BEAT_EPSILON
                && earlier.pitch != note.pitch
        })
}

/// A line's notes with their note-ons as played.
struct Line<'a> {
    notes: &'a [MidiNote],
    groups: &'a [Range<usize>],
    clock: &'a Clock<'a>,
    /// Each note's note-on, in seconds.
    on_s: &'a [f64],
    /// Each note's start, in content beats.
    starts: &'a [f64],
    /// Each onset group's note-ons, in content beats.
    onsets: &'a [OnsetSpan],
}

/// The end of each note as played, in content beats: moved with its note-on's offset and by its
/// length share, never shorter than `1 - length` of its written length after its note-on, kept
/// past the note-on of a later note it overlapped in writing, and ended by the earliest note-on
/// of the first later onset it did not overlap.
fn ends(line: &Line<'_>, drawn: &[Drawn], length: f64) -> Vec<f64> {
    let clock = line.clock;
    let group_starts: Vec<f64> = line
        .groups
        .iter()
        .map(|group| line.notes.get(group.start).map_or(0.0, |note| note.start))
        .collect();
    let mut ends = Vec::with_capacity(line.notes.len());
    for (group_index, group) in line.groups.iter().enumerate() {
        for index in group.clone() {
            let (Some(note), Some(&on)) = (line.notes.get(index), line.on_s.get(index)) else {
                continue;
            };
            let drawn = drawn.get(index).copied().unwrap_or(Drawn::NONE);
            let start_s = clock.seconds(note.start);
            let written_end_s = clock.seconds(note.end());
            let length_s = written_end_s - start_s;
            let mut end = written_end_s + drawn.offset_s + drawn.length * length_s;
            let least = on + (1.0 - length) * length_s;
            if length > 0.0 && least > end + TIME_EPSILON {
                end = least;
            }
            // Keep each written overlap with a later note at another pitch.
            for later_index in group.end..line.notes.len() {
                let Some(later) = line.notes.get(later_index) else {
                    break;
                };
                if later.start >= note.end() - BEAT_EPSILON {
                    break;
                }
                let Some(&later_on) = line.on_s.get(later_index) else {
                    break;
                };
                if later.pitch != note.pitch {
                    let overlap = written_end_s - clock.seconds(later.start);
                    let needed = later_on + overlap.min(KEEP_OVERLAP_S);
                    if needed > end + TIME_EPSILON {
                        end = needed;
                    }
                }
            }
            let mut end_beat = beat_at(clock, end, note.end());
            // End by the first later onset the note did not overlap in writing.
            let after = group_index + 1;
            let first_clear = after
                + group_starts
                    .get(after..)
                    .unwrap_or_default()
                    .partition_point(|&start| start < note.end() - BEAT_EPSILON);
            if let Some(onset) = line.onsets.get(first_clear) {
                end_beat = end_beat.min(onset.first);
            }
            // A note-on that was held back past its written end (a very short note in a crowded
            // run) still lasts: it ends by the next onset at the latest.
            let start = line.starts.get(index).copied().unwrap_or(note.start);
            if end_beat <= start {
                end_beat = line
                    .onsets
                    .get(first_clear)
                    .map_or(start + note.length, |onset| onset.first);
            }
            ends.push(end_beat);
        }
    }
    ends
}

/// The content beat at `seconds`, or exactly `written` when `seconds` is the time `written`
/// converts to, so that an unmoved note keeps its beats bit for bit.
fn beat_at(clock: &Clock<'_>, seconds: f64, written: f64) -> f64 {
    if same(seconds, clock.seconds(written)) {
        written
    } else {
        clock.beat(seconds)
    }
}

/// Whether two values are the same number, bit for bit.
fn same(a: f64, b: f64) -> bool {
    a.total_cmp(&b).is_eq()
}

/// Gives `notes`, the analysed notes with their written velocities, their timing from `timed`,
/// and moves each written velocity by its drawn step: within 1-127, and, where the written
/// velocity sounds (`scales`, [`Shaped::scales`](super::dynamics)), inside the velocity layer of
/// `caps` that its note-on played before the step. `levels` then holds the velocity each note-on
/// plays.
pub(super) fn apply(
    timed: &Timed,
    caps: &SoundCaps,
    scales: &[Option<f64>],
    notes: &mut [MidiNote],
    levels: &mut [NoteLevel],
) {
    for (index, note) in notes.iter_mut().enumerate() {
        if let Some(&(start, length)) = timed.spans.get(index) {
            note.start = start;
            note.length = length;
        }
        let step = timed.notes.get(index).map_or(0, |timing| timing.velocity);
        if step == 0 {
            continue;
        }
        let written = i16::from(note.velocity);
        let mut velocity = written.saturating_add(step).clamp(1, 127);
        let scale = scales.get(index).copied().flatten();
        if let Some(scale) = scale {
            let sounds = |velocity: i16| velocity_of(f64::from(velocity) * scale);
            if let Some(layer) = caps.layer(sounds(written)) {
                // The written velocity sounds inside the layer, so stepping back toward it ends
                // inside the layer.
                while velocity != written && !layer.contains(&sounds(velocity)) {
                    velocity += (written - velocity).signum();
                }
            }
        }
        note.velocity = u8::try_from(velocity).expect("a velocity clamped to 1-127 fits a u8");
        if let (Some(scale), Some(level)) = (scale, levels.get_mut(index)) {
            level.velocity = velocity_of(f64::from(note.velocity) * scale);
        }
    }
}

/// The notes in the project's note order (start, then pitch, then id), with their articulations,
/// levels and timings in the same order.
pub(super) fn in_note_order(
    notes: Vec<MidiNote>,
    articulations: Vec<NoteArticulation>,
    levels: Vec<NoteLevel>,
    timings: Vec<NoteTiming>,
) -> (
    Vec<MidiNote>,
    Vec<NoteArticulation>,
    Vec<NoteLevel>,
    Vec<NoteTiming>,
) {
    let mut rows: Vec<(MidiNote, NoteArticulation, NoteLevel, NoteTiming)> = notes
        .into_iter()
        .zip(articulations)
        .zip(levels.into_iter().zip(timings))
        .map(|((note, articulation), (level, timing))| (note, articulation, level, timing))
        .collect();
    rows.sort_by(|a, b| note_order(&a.0, &b.0));
    let mut notes = Vec::with_capacity(rows.len());
    let mut articulations = Vec::with_capacity(rows.len());
    let mut levels = Vec::with_capacity(rows.len());
    let mut timings = Vec::with_capacity(rows.len());
    for (note, articulation, level, timing) in rows {
        notes.push(note);
        articulations.push(articulation);
        levels.push(level);
        timings.push(timing);
    }
    (notes, articulations, levels, timings)
}
