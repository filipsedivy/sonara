//! Real-time: the per-block event lookup runs on the audio render thread (see
//! docs/audio-engine.md); [`NoteSchedule::build`] is setup.
//!
//! The MIDI schedule of an instrument track (`NoteSchedule` in `audio/engine/graph.py`):
//!
//! - **Build:** for every unmuted clip, the clip's `playback_notes()` with
//!   `on = beat_to_sample(t)` and `off = beat_to_sample(min(t + length, clip_end))`, at least
//!   `on + 1`; controller events only for clips that do not loop and only inside the clip span.
//!   Notes are sorted stably by `on`, the note-off order stably by `off`, the controller events
//!   stably by frame.
//! - **Per block** (`[frame, frame + n)`): on a discontinuity (the chase), an all-notes-off and
//!   a note-on for every note with `on < frame < off`, all at offset 0; then the note-offs, the
//!   note-ons and the controller events of the block, stably sorted by offset. The four groups
//!   are each sorted already, so a merge that prefers the earlier group on equal offsets gives
//!   exactly the prototype's stable sort, without the scratch memory a stable sort allocates.
//! - **A sampler's chase** ([`NoteSchedule::chase_sampler`]) restores more than the notes,
//!   at offset 0 between the all-notes-off and the chased notes: a keyswitch event
//!   ([`EventKind::KeySwitch`]) for the last keyswitch note of the patch that started before
//!   `frame` (a note that the preset's key offset moves onto a keyswitch key), then the last
//!   event before `frame` of each of the controllers 1 (modulation), 11 (expression) and 64
//!   (sustain), on its own channel and in timeline order. So a seek or a loop wrap plays the
//!   articulation, the swell and the pedal that playing up to the new position leaves. The
//!   sampler makes its default switch active, and applies its articulation parameter again, at
//!   the all-notes-off, so a position before every keyswitch note gets the default or the
//!   parameter's articulation. A controller with no event before `frame` keeps its value. Each
//!   controller is restored once, by its last event on whichever channel: a value an earlier
//!   event left on another channel is not restored, which only the sampler's per-channel gates
//!   and crossfades can tell (the pedal, volume and expression it applies act on every
//!   channel). Every other instrument (the Poly Synth and the Drum Kit among them) gets the
//!   plain chase, so its events and its golden renders stay the same (GLD-01).
//!
//! The event buffer is sized at build for the worst case, every event in one block
//! (`1 + 2 · notes + controls`: a chased note cannot also start in the block), and a sampler's
//! chase adds a slot for the keyswitch and one for each restored controller that the schedule
//! has, at most 4, so no event is ever dropped.
//!
//! **Held notes** ([`NoteSchedule::held_at`]): a live graph's rebuild compares the
//! notes the old and the edited schedule hold at the carried frame, and releases those that
//! only the old one holds ([`NoteBits`]).
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

use sonara_core::midi::ControlKind;
use sonara_core::model::clip::MidiClip;
use sonara_core::time::TempoMap;
use sonara_dsp::processor::{EventKind, InstrumentEvent, NoteSet};

/// How many controllers a sampler's chase restores ([`chased_bit`]).
const CHASED_CONTROLLERS: usize = 3;

/// A note of the schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Note {
    on: i64,
    off: i64,
    pitch: u8,
    velocity: u8,
    channel: u8,
}

/// A controller event of the schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Control {
    frame: i64,
    kind: EventKind,
    number: u8,
    value: i32,
    channel: u8,
}

/// The filler of unused event slots.
const NO_EVENT: InstrumentEvent = InstrumentEvent {
    offset: 0,
    kind: EventKind::AllNotesOff,
    pitch: 0,
    velocity: 0,
    channel: 0,
    value: 0,
    number: 0,
};

/// The notes and controller events of an instrument track in timeline frames.
#[derive(Debug, Clone)]
pub(crate) struct NoteSchedule {
    /// Notes, stably sorted by `on`.
    notes: Vec<Note>,
    /// `off` of the notes in `off_order` (ascending).
    off_sorted: Vec<i64>,
    /// Indices into `notes`, stably sorted by `off`.
    off_order: Vec<usize>,
    /// Controller events, stably sorted by frame.
    controls: Vec<Control>,
    /// What the chase restores besides the notes, for a sampler only.
    sampler: Option<SamplerChase>,
    /// The events of the last block: the first `len` slots.
    events: Vec<InstrumentEvent>,
    len: usize,
}

/// What a sampler's chase restores besides the notes (see the module docs).
#[derive(Debug, Clone)]
struct SamplerChase {
    /// The notes on a keyswitch key of the patch, stably sorted by `on`.
    switches: Vec<Note>,
    /// The events of the controllers the chase restores, stably sorted by frame.
    controls: Vec<Control>,
    /// The controllers that `controls` has, one bit each ([`chased_bit`]).
    present: u8,
}

impl SamplerChase {
    /// The events the buffer needs room for: one keyswitch and one event per controller.
    fn slots(&self) -> usize {
        usize::from(!self.switches.is_empty())
            .saturating_add(usize::try_from(self.present.count_ones()).unwrap_or(0))
    }

    /// Writes the keyswitch and the controller values that playing up to `frame` leaves.
    fn restore(&self, frame: i64, out: &mut EventWriter<'_>) {
        let pressed = self.switches.partition_point(|note| note.on < frame);
        if let Some(note) = pressed
            .checked_sub(1)
            .and_then(|last| self.switches.get(last))
        {
            out.push(InstrumentEvent {
                kind: EventKind::KeySwitch,
                ..note_on(note, 0)
            });
        }
        let before = self
            .controls
            .get(
                ..self
                    .controls
                    .partition_point(|control| control.frame < frame),
            )
            .unwrap_or_default();
        // The last event of each controller, latest first: found walking back from `frame`.
        let mut latest = [None; CHASED_CONTROLLERS];
        let mut seen = 0_u8;
        for control in before.iter().rev() {
            let bit = chased_bit(control.number);
            if bit == 0 || seen & bit != 0 {
                continue;
            }
            seen |= bit;
            if let Some(slot) = latest.iter_mut().find(|slot| slot.is_none()) {
                *slot = Some(control);
            }
            if seen == self.present {
                break;
            }
        }
        for control in latest.iter().rev().flatten() {
            out.push(control_event(control, 0));
        }
    }
}

/// The bit of controller `number` among those a sampler's chase restores: modulation (1),
/// expression (11) and the sustain pedal (64); 0 for every other controller.
fn chased_bit(number: u8) -> u8 {
    match number {
        1 => 0b001,
        11 => 0b010,
        64 => 0b100,
        _ => 0,
    }
}

impl NoteSchedule {
    /// The schedule of `clips` (a track's MIDI clips in track order) at `sample_rate`.
    ///
    /// Setup: allocates, so it runs off the real-time thread, when the graph is built.
    #[expect(
        clippy::disallowed_methods,
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    pub(crate) fn build<'a>(
        clips: impl Iterator<Item = &'a MidiClip>,
        tempo: &TempoMap,
        sample_rate: u32,
    ) -> Self {
        let mut notes = Vec::new();
        let mut controls = Vec::new();
        for clip in clips.filter(|clip| !clip.muted) {
            let clip_end = clip.start + clip.length;
            for (t, note) in clip.playback_notes() {
                let on = tempo.beat_to_sample(t, sample_rate);
                // Python's `min(t + length, clip_end)` keeps the first of equal values.
                let end = t + note.length;
                let end = if clip_end < end { clip_end } else { end };
                let off = tempo.beat_to_sample(end, sample_rate);
                notes.push(Note {
                    on,
                    off: off.max(on.saturating_add(1)),
                    pitch: note.pitch,
                    velocity: note.velocity,
                    channel: note.channel,
                });
            }
            if clip.loop_length.is_some() {
                continue;
            }
            for event in clip.controls.iter() {
                let beat = clip.start + (event.beat - clip.content_offset);
                if !(clip.start <= beat && beat < clip_end) {
                    continue;
                }
                controls.push(Control {
                    frame: tempo.beat_to_sample(beat, sample_rate),
                    kind: match event.kind {
                        ControlKind::Cc => EventKind::Cc,
                        ControlKind::PitchBend => EventKind::PitchBend,
                        ControlKind::ChannelPressure => EventKind::ChannelPressure,
                    },
                    // Validated ranges: CC numbers 0–127 (a pitch bend's number is unused),
                    // values within ±8192, channels 0–15.
                    number: u8::try_from(event.number).unwrap_or(0),
                    value: i32::try_from(event.value).unwrap_or(0),
                    channel: u8::try_from(event.channel).unwrap_or(0),
                });
            }
        }
        // `sort_by_key` is stable, like NumPy's `argsort(kind="stable")` and Python's sort.
        notes.sort_by_key(|note| note.on);
        controls.sort_by_key(|control| control.frame);
        let mut by_off: Vec<(i64, usize)> = notes
            .iter()
            .enumerate()
            .map(|(index, note)| (note.off, index))
            .collect();
        by_off.sort_by_key(|&(off, _)| off);
        let capacity = plain_capacity(&notes, &controls);
        Self {
            off_sorted: by_off.iter().map(|&(off, _)| off).collect(),
            off_order: by_off.iter().map(|&(_, index)| index).collect(),
            notes,
            controls,
            sampler: None,
            events: vec![NO_EVENT; capacity],
            len: 0,
        }
    }

    /// Makes the chase restore what a sampler whose keyswitch notes are `switch_keys` (the clip
    /// pitches that select an articulation of its patch) needs besides the notes: its last
    /// keyswitch and the last values of the controllers 1, 11 and 64 (see the module docs). The
    /// graph calls it for sampler instruments only.
    ///
    /// Setup: allocates (the event buffer grows by the slots the chase adds), so it runs off
    /// the real-time thread, when the graph is built, before the first block.
    pub(crate) fn chase_sampler(&mut self, switch_keys: NoteSet) {
        let switches: Vec<Note> = self
            .notes
            .iter()
            .filter(|note| switch_keys.contains(note.pitch))
            .copied()
            .collect();
        let controls: Vec<Control> = self
            .controls
            .iter()
            .filter(|control| control.kind == EventKind::Cc && chased_bit(control.number) != 0)
            .copied()
            .collect();
        let present = controls
            .iter()
            .fold(0, |bits, control| bits | chased_bit(control.number));
        let chase = SamplerChase {
            switches,
            controls,
            present,
        };
        let capacity = plain_capacity(&self.notes, &self.controls).saturating_add(chase.slots());
        self.events.resize(capacity, NO_EVENT);
        self.sampler = Some(chase);
    }

    /// The size of the event buffer: the most events one block can have.
    pub(crate) fn capacity(&self) -> usize {
        self.events.len()
    }

    /// The notes held at `frame` by blocks that played the schedule up to it: started before it
    /// (`on < frame`) and not yet released (`off >= frame`, so a note-off at `frame` itself is
    /// still to come), by channel and pitch.
    pub(crate) fn held_at(&self, frame: i64) -> NoteBits {
        let mut held = NoteBits::EMPTY;
        for note in self.notes.iter().filter(|n| n.on < frame && n.off >= frame) {
            held.insert(note.channel, note.pitch);
        }
        held
    }

    /// The pitches of the schedule's notes: every note-on [`NoteSchedule::events`] can send,
    /// chased or not.
    pub(crate) fn pitches(&self) -> NoteSet {
        self.notes.iter().map(|note| note.pitch).collect()
    }

    /// The events of the block `[frame, frame + n)`, sorted by offset; with `chase`, preceded
    /// by an all-notes-off and the notes sounding at `frame` (`NoteSchedule.events`), and for a
    /// sampler by the keyswitch and controllers between them (see the module docs).
    pub(crate) fn events(&mut self, frame: i64, n: usize, chase: bool) -> &[InstrumentEvent] {
        let end = frame.saturating_add(i64::try_from(n).unwrap_or(i64::MAX));
        let Self {
            notes,
            off_sorted,
            off_order,
            controls,
            sampler,
            events,
            len,
        } = self;
        let mut out = EventWriter {
            slots: events.as_mut_slice(),
            len: 0,
        };
        if chase {
            out.push(NO_EVENT);
            if let Some(sampler) = sampler {
                sampler.restore(frame, &mut out);
            }
            for note in notes.iter().filter(|n| n.on < frame && n.off > frame) {
                out.push(note_on(note, 0));
            }
        }
        let offs = window(off_sorted, frame, end, |&off| off);
        let ons = window(notes, frame, end, |note| note.on);
        let ctls = window(controls, frame, end, |control| control.frame);
        let mut offs = off_order
            .get(offs)
            .unwrap_or_default()
            .iter()
            .filter_map(|&index| notes.get(index))
            .map(|note| note_off(note, offset(note.off, frame)))
            .peekable();
        let mut ons = notes
            .get(ons)
            .unwrap_or_default()
            .iter()
            .map(|note| note_on(note, offset(note.on, frame)))
            .peekable();
        let mut ctls = controls
            .get(ctls)
            .unwrap_or_default()
            .iter()
            .map(|control| control_event(control, offset(control.frame, frame)))
            .peekable();
        // The merge: the smallest offset first; on equal offsets note-offs, then note-ons,
        // then controller events (their order in the prototype's list before its stable sort).
        loop {
            let off = offs.peek().map(|e| e.offset);
            let on = ons.peek().map(|e| e.offset);
            let ctl = ctls.peek().map(|e| e.offset);
            let take_off =
                off.is_some_and(|o| on.is_none_or(|x| o <= x) && ctl.is_none_or(|x| o <= x));
            let take_on = !take_off && on.is_some_and(|o| ctl.is_none_or(|x| o <= x));
            let next = if take_off {
                offs.next()
            } else if take_on {
                ons.next()
            } else {
                ctls.next()
            };
            match next {
                Some(event) => out.push(event),
                None => break,
            }
        }
        *len = out.len;
        events.get(..*len).unwrap_or_default()
    }
}

/// A set of (MIDI channel, pitch) pairs, without allocation: the notes an instrument holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NoteBits([u128; 16]);

impl NoteBits {
    /// No note.
    pub(crate) const EMPTY: Self = Self([0; 16]);

    /// Adds a note; channels above 15 and pitches above 127 are not MIDI notes and are ignored.
    pub(crate) fn insert(&mut self, channel: u8, pitch: u8) {
        if let (Some(bits), Some(bit)) = (
            self.0.get_mut(usize::from(channel)),
            1_u128.checked_shl(u32::from(pitch)),
        ) {
            *bits |= bit;
        }
    }

    /// The notes of `self` that `other` does not hold.
    pub(crate) fn without(self, other: Self) -> Self {
        let mut out = self;
        for (bits, &theirs) in out.0.iter_mut().zip(&other.0) {
            *bits &= !theirs;
        }
        out
    }

    /// The notes of both.
    pub(crate) fn union(self, other: Self) -> Self {
        let mut out = self;
        for (bits, &theirs) in out.0.iter_mut().zip(&other.0) {
            *bits |= theirs;
        }
        out
    }

    /// Whether it holds no note.
    pub(crate) fn is_empty(&self) -> bool {
        self.0.iter().all(|&bits| bits == 0)
    }

    /// Note-offs at offset 0 for every note, by channel, then pitch, ascending.
    pub(crate) fn note_offs(&self) -> impl Iterator<Item = InstrumentEvent> + '_ {
        (0_u8..16).zip(self.0.iter()).flat_map(|(channel, &bits)| {
            (0_u8..128)
                .filter(move |&pitch| bits & (1_u128 << pitch) != 0)
                .map(move |pitch| InstrumentEvent {
                    offset: 0,
                    kind: EventKind::NoteOff,
                    pitch,
                    velocity: 0,
                    channel,
                    value: 0,
                    number: 0,
                })
        })
    }
}

/// The size of a plain schedule's event buffer: every event in one block, `1 + 2 · notes +
/// controls` (see the module docs).
fn plain_capacity(notes: &[Note], controls: &[Control]) -> usize {
    notes
        .len()
        .saturating_mul(2)
        .saturating_add(controls.len())
        .saturating_add(1)
}

/// Writes events into the preallocated slots.
struct EventWriter<'a> {
    slots: &'a mut [InstrumentEvent],
    len: usize,
}

impl EventWriter<'_> {
    /// Appends `event`; the slots hold the worst case (see the module docs), so nothing is
    /// dropped.
    fn push(&mut self, event: InstrumentEvent) {
        if let Some(slot) = self.slots.get_mut(self.len) {
            *slot = event;
            self.len = self.len.saturating_add(1);
        }
    }
}

/// The index range of the items whose key lies in `[frame, end)` (`searchsorted` with
/// `side="left"` on both bounds) of a slice sorted by `key`.
fn window<T>(items: &[T], frame: i64, end: i64, key: impl Fn(&T) -> i64) -> std::ops::Range<usize> {
    let lo = items.partition_point(|item| key(item) < frame);
    let hi = items.partition_point(|item| key(item) < end).max(lo);
    lo..hi
}

/// The offset of timeline frame `at` within the block starting at `frame` (`at >= frame`).
fn offset(at: i64, frame: i64) -> usize {
    usize::try_from(at.saturating_sub(frame)).unwrap_or(0)
}

fn note_on(note: &Note, offset: usize) -> InstrumentEvent {
    InstrumentEvent {
        offset,
        kind: EventKind::NoteOn,
        pitch: note.pitch,
        velocity: note.velocity,
        channel: note.channel,
        value: 0,
        number: 0,
    }
}

fn note_off(note: &Note, offset: usize) -> InstrumentEvent {
    InstrumentEvent {
        offset,
        kind: EventKind::NoteOff,
        pitch: note.pitch,
        velocity: 0,
        channel: note.channel,
        value: 0,
        number: 0,
    }
}

fn control_event(control: &Control, offset: usize) -> InstrumentEvent {
    InstrumentEvent {
        offset,
        kind: control.kind,
        pitch: 0,
        velocity: 0,
        channel: control.channel,
        value: control.value,
        number: control.number,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sonara_core::midi::{ControlKind, MidiControlEvent, MidiNote};
    use sonara_core::model::JsonMap;
    use sonara_core::model::clip::{MidiClip, MidiClipData};
    use sonara_core::time::{TempoMap, TempoPoint};
    use sonara_dsp::processor::{EventKind, InstrumentEvent, NoteSet};

    use super::NoteSchedule;

    fn clip(
        start: f64,
        length: f64,
        notes: &[(u8, f64, f64)],
        loop_length: Option<f64>,
    ) -> MidiClip {
        let notes = ["n_0", "n_1", "n_2"]
            .iter()
            .zip(notes)
            .map(|(id, &(pitch, s, l))| MidiNote::new(*id, i64::from(pitch), s, l, 100, 0).unwrap())
            .collect();
        let controls = [MidiControlEvent::new(0.5, ControlKind::Cc, 64, 1, 0).unwrap()].to_vec();
        MidiClip::new(MidiClipData {
            id: "clp_1".to_owned(),
            name: "Clip".to_owned(),
            start,
            length,
            notes: Arc::new(notes),
            controls: Arc::new(controls),
            content_offset: 0.0,
            loop_length,
            muted: false,
            color: None,
            extra: JsonMap::new(),
        })
        .unwrap()
    }

    fn kinds(events: &[InstrumentEvent]) -> Vec<(usize, EventKind, u8)> {
        events.iter().map(|e| (e.offset, e.kind, e.pitch)).collect()
    }

    /// At 120 BPM and 8 frames per second a beat is 4 frames. The expected events are the
    /// prototype's `NoteSchedule.events` for the same clips (host virtualenv, 2026-10-21).
    #[test]
    fn blocks_hold_offs_then_ons_then_controls_by_offset() {
        let tempo = TempoMap::constant(120.0).unwrap();
        // C4 0–1 beat (frames 0–4), E4 1–2 (4–8), G4 from 0.75 crossing the clip end at 2
        // beats, so its off is cut to frame 8.
        let c = clip(
            0.0,
            2.0,
            &[(60, 0.0, 1.0), (64, 1.0, 1.0), (67, 0.75, 4.0)],
            None,
        );
        let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 8);
        assert_eq!(
            kinds(schedule.events(0, 4, true)),
            [
                (0, EventKind::AllNotesOff, 0),
                (0, EventKind::NoteOn, 60),
                (2, EventKind::Cc, 0),
                (3, EventKind::NoteOn, 67),
            ]
        );
        // The off of C4 and the on of E4 share offset 0: the off comes first.
        assert_eq!(
            kinds(schedule.events(4, 4, false)),
            [(0, EventKind::NoteOff, 60), (0, EventKind::NoteOn, 64)]
        );
        // A chase at frame 5 retriggers the notes sounding there, in note-on order (G4 started
        // first); the offs at equal frames keep the same order (stable sort by off).
        assert_eq!(
            kinds(schedule.events(5, 4, true)),
            [
                (0, EventKind::AllNotesOff, 0),
                (0, EventKind::NoteOn, 67),
                (0, EventKind::NoteOn, 64),
                (3, EventKind::NoteOff, 67),
                (3, EventKind::NoteOff, 64),
            ]
        );
    }

    /// The schedule follows the tempo map, not a fixed tempo (SON-P03): at 8 frames per second,
    /// 120 BPM up to beat 4 and 60 BPM after it, a note from beat 6 to 7 starts at second
    /// 2 + 2 = 4 (frame 32) and ends at second 5 (frame 40); the oracle is the arithmetic of the
    /// map, not the code under test. A chase inside the note retriggers it at offset 0.
    #[test]
    fn notes_follow_a_tempo_change() {
        let tempo = TempoMap::new(
            [
                TempoPoint::new(0.0, 120.0, false).unwrap(),
                TempoPoint::new(4.0, 60.0, false).unwrap(),
            ]
            .to_vec(),
        )
        .unwrap();
        let c = clip(0.0, 16.0, &[(60, 6.0, 1.0)], None);
        let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 8);
        let notes = |events: &[InstrumentEvent]| {
            kinds(events)
                .into_iter()
                .filter(|&(_, kind, _)| matches!(kind, EventKind::NoteOn | EventKind::NoteOff))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            notes(schedule.events(32, 16, false)),
            [(0, EventKind::NoteOn, 60), (8, EventKind::NoteOff, 60)]
        );
        assert_eq!(
            notes(schedule.events(36, 8, true)),
            [(0, EventKind::NoteOn, 60), (4, EventKind::NoteOff, 60)]
        );
    }

    #[test]
    fn looped_clips_repeat_notes_and_drop_controls() {
        let tempo = TempoMap::constant(120.0).unwrap();
        let c = clip(0.0, 4.0, &[(60, 0.0, 0.5)], Some(2.0));
        let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 8);
        let all = kinds(schedule.events(0, 16, false));
        assert_eq!(
            all,
            [
                (0, EventKind::NoteOn, 60),
                (2, EventKind::NoteOff, 60),
                (8, EventKind::NoteOn, 60),
                (10, EventKind::NoteOff, 60),
            ]
        );
    }

    #[test]
    fn the_pitches_are_those_of_every_note_on() {
        let tempo = TempoMap::constant(120.0).unwrap();
        let a = clip(0.0, 4.0, &[(36, 0.0, 0.5), (42, 1.0, 0.5)], Some(2.0));
        let b = clip(4.0, 2.0, &[(38, 0.5, 1.0), (36, 1.0, 1.0)], None);
        let mut schedule = NoteSchedule::build([&a, &b].into_iter(), &tempo, 8);
        let pitches = schedule.pitches();
        let members: Vec<u8> = (0..=u8::MAX).filter(|&p| pitches.contains(p)).collect();
        assert_eq!(members, [36, 38, 42]);
        // The oracle: the note-ons the schedule sends, in one block and chased mid-note (38
        // sounds from frame 18 to 22).
        let mut sent = Vec::new();
        for (frame, n, chase) in [(0, 1_000, false), (19, 1, true)] {
            let events = schedule.events(frame, n, chase);
            sent.extend(
                events
                    .iter()
                    .filter(|e| e.kind == EventKind::NoteOn)
                    .map(|e| e.pitch),
            );
        }
        assert!(sent.ends_with(&[38]), "the chase sends 38");
        sent.sort_unstable();
        sent.dedup();
        assert_eq!(sent, members);
        let empty = NoteSchedule::build(std::iter::empty(), &tempo, 8);
        assert_eq!(empty.pitches(), NoteSet::EMPTY);
    }

    /// A clip of 8 beats from beat 0 with `notes` (pitch, start, length) at velocity 100 on
    /// channel 0 and `controls` (beat, kind, value, number, channel).
    fn clip_with(
        notes: &[(u8, f64, f64)],
        controls: &[(f64, ControlKind, i64, i64, i64)],
    ) -> MidiClip {
        let ids = ["n_0", "n_1", "n_2", "n_3", "n_4", "n_5", "n_6", "n_7"];
        assert!(notes.len() <= ids.len(), "an ID for every note");
        let notes = ids
            .iter()
            .zip(notes)
            .map(|(id, &(pitch, s, l))| MidiNote::new(*id, i64::from(pitch), s, l, 100, 0).unwrap())
            .collect();
        let controls = controls
            .iter()
            .map(|&(beat, kind, value, number, channel)| {
                MidiControlEvent::new(beat, kind, value, number, channel).unwrap()
            })
            .collect();
        MidiClip::new(MidiClipData {
            id: "clp_1".to_owned(),
            name: "Clip".to_owned(),
            start: 0.0,
            length: 8.0,
            notes: Arc::new(notes),
            controls: Arc::new(controls),
            content_offset: 0.0,
            loop_length: None,
            muted: false,
            color: None,
            extra: JsonMap::new(),
        })
        .unwrap()
    }

    /// Offset, kind, pitch, controller number, value and channel of each event.
    fn details(events: &[InstrumentEvent]) -> Vec<(usize, EventKind, u8, u8, i32, u8)> {
        events
            .iter()
            .map(|e| (e.offset, e.kind, e.pitch, e.number, e.value, e.channel))
            .collect()
    }

    /// The keyswitch keys of the patches below: C1 and C#1.
    fn switch_keys() -> NoteSet {
        [24, 25].into_iter().collect()
    }

    /// The Poly Synth and the Drum Kit play a plain schedule. Its chase re-sends only the
    /// notes sounding at the new position, never a keyswitch note that ended before it (C1 at
    /// beat 0) or a controller, and its buffer keeps the prototype's size, so their events are
    /// the prototype's and their renders stay byte for byte the golden ones
    /// (`tests/golden_render.rs`, GLD-01).
    #[test]
    fn a_plain_chase_sends_no_switch_and_no_controller() {
        let tempo = TempoMap::constant(120.0).unwrap();
        // The clip also holds CC 1 at beat 0.5, before the position.
        let c = clip(0.0, 4.0, &[(24, 0.0, 0.25), (60, 2.0, 1.0)], None);
        let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 8);
        assert_eq!(schedule.capacity(), 1 + 2 * 2 + 1);
        // Beat 2.5 is frame 10, inside the note C4 (frames 8 to 12).
        assert_eq!(
            kinds(schedule.events(10, 1, true)),
            [(0, EventKind::AllNotesOff, 0), (0, EventKind::NoteOn, 60)]
        );
    }

    /// The schedule of the chase tests: keyswitch notes C1 at beat 0 and C#1 at beat 0.5, C4
    /// from beat 2 to 3, C1 again at beat 3; CC 1, 11 and 64 before beat 2.5 (on two channels)
    /// and after it, with CC 7 and a pitch bend that the chase does not restore.
    fn sampler_schedule() -> NoteSchedule {
        let tempo = TempoMap::constant(120.0).unwrap();
        let c = clip_with(
            &[
                (24, 0.0, 0.25),
                (25, 0.5, 0.25),
                (60, 2.0, 1.0),
                (24, 3.0, 0.25),
            ],
            &[
                (0.25, ControlKind::Cc, 10, 1, 0),
                (0.25, ControlKind::Cc, 127, 64, 0),
                (0.5, ControlKind::Cc, 90, 11, 0),
                (0.75, ControlKind::Cc, 20, 1, 0),
                (0.75, ControlKind::Cc, 30, 7, 0),
                (0.75, ControlKind::PitchBend, 4096, 0, 0),
                (1.0, ControlKind::Cc, 100, 64, 1),
                (3.0, ControlKind::Cc, 40, 11, 0),
            ],
        );
        let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 8);
        schedule.chase_sampler(switch_keys());
        schedule
    }

    #[test]
    fn a_sampler_chase_restores_the_last_switch_and_controllers() {
        let mut schedule = sampler_schedule();
        // The plain worst case (1 + 2 · 4 notes + 8 controls) plus the keyswitch and the three
        // controllers.
        assert_eq!(schedule.capacity(), 17 + 4);
        // At frame 10 (beat 2.5): the last switch C#1 (frame 2), then the last CC 11 (frame
        // 2), CC 1 (frame 3) and CC 64 (frame 4, channel 1) in timeline order, then C4.
        assert_eq!(
            details(schedule.events(10, 1, true)),
            [
                (0, EventKind::AllNotesOff, 0, 0, 0, 0),
                (0, EventKind::KeySwitch, 25, 0, 0, 0),
                (0, EventKind::Cc, 0, 11, 90, 0),
                (0, EventKind::Cc, 0, 1, 20, 0),
                (0, EventKind::Cc, 0, 64, 100, 1),
                (0, EventKind::NoteOn, 60, 0, 0, 0),
            ]
        );
        // A block that continues the last one does not chase.
        assert_eq!(details(schedule.events(11, 1, false)), []);
        // After beat 3 the last switch is C1 again and the last CC 11 is 40.
        assert_eq!(
            details(schedule.events(14, 1, true)),
            [
                (0, EventKind::AllNotesOff, 0, 0, 0, 0),
                (0, EventKind::KeySwitch, 24, 0, 0, 0),
                (0, EventKind::Cc, 0, 1, 20, 0),
                (0, EventKind::Cc, 0, 64, 100, 1),
                (0, EventKind::Cc, 0, 11, 40, 0),
            ]
        );
    }

    #[test]
    fn a_sampler_chase_restores_only_what_lies_before_the_position() {
        let mut schedule = sampler_schedule();
        // At frame 0 nothing lies before it: the switch note at frame 0 plays in the block.
        assert_eq!(
            kinds(schedule.events(0, 1, true)),
            [(0, EventKind::AllNotesOff, 0), (0, EventKind::NoteOn, 24)]
        );
        // At frame 1 the switch C1 lies before it; the events at frame 1 itself (the end of
        // C1, CC 1 and CC 64) play in the block, after the chase.
        assert_eq!(
            details(schedule.events(1, 1, true)),
            [
                (0, EventKind::AllNotesOff, 0, 0, 0, 0),
                (0, EventKind::KeySwitch, 24, 0, 0, 0),
                (0, EventKind::NoteOff, 24, 0, 0, 0),
                (0, EventKind::Cc, 0, 1, 10, 0),
                (0, EventKind::Cc, 0, 64, 127, 0),
            ]
        );
        // A sampler without keyswitch keys gets the controllers only.
        let tempo = TempoMap::constant(120.0).unwrap();
        let c = clip(0.0, 4.0, &[(24, 0.0, 0.25), (60, 2.0, 1.0)], None);
        let mut plain = NoteSchedule::build([&c].into_iter(), &tempo, 8);
        plain.chase_sampler(NoteSet::EMPTY);
        assert_eq!(plain.capacity(), 1 + 2 * 2 + 1 + 1);
        assert_eq!(
            details(plain.events(10, 1, true)),
            [
                (0, EventKind::AllNotesOff, 0, 0, 0, 0),
                (0, EventKind::Cc, 0, 1, 64, 0),
                (0, EventKind::NoteOn, 60, 0, 0, 0),
            ]
        );
    }

    #[test]
    fn a_held_switch_note_is_chased_as_a_note_too() {
        // As before, a note sounding at the position is re-sent; a held keyswitch note also
        // comes first as the keyswitch.
        let tempo = TempoMap::constant(120.0).unwrap();
        let c = clip_with(&[(25, 0.0, 4.0), (60, 1.0, 2.0)], &[]);
        let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 8);
        schedule.chase_sampler(switch_keys());
        assert_eq!(schedule.capacity(), 1 + 2 * 2 + 1);
        assert_eq!(
            kinds(schedule.events(8, 1, true)),
            [
                (0, EventKind::AllNotesOff, 0),
                (0, EventKind::KeySwitch, 25),
                (0, EventKind::NoteOn, 25),
                (0, EventKind::NoteOn, 60),
            ]
        );
    }

    /// TST-07: a chase never drops an event. The oracle counts the events of each block from
    /// the clip's own frames (a beat is 4 frames), not from the schedule.
    #[test]
    fn chase_events_fit_the_buffer() {
        // (pitch, on, off) in frames; C1 and C#1 are keyswitch keys.
        let notes: [(u8, i64, i64); 6] = [
            (24, 0, 1),
            (60, 1, 16),
            (62, 2, 16),
            (25, 4, 5),
            (64, 4, 20),
            (24, 12, 13),
        ];
        // (frame, controller): the chased controllers 1, 11 and 64 and the unchased 7.
        let controls: [(i64, i64); 6] = [(1, 1), (2, 11), (3, 64), (6, 1), (8, 7), (12, 11)];
        let tempo = TempoMap::constant(120.0).unwrap();
        let beats = |frames: i64| f64::from(i32::try_from(frames).unwrap()) / 4.0;
        let c = clip_with(
            &notes.map(|(pitch, on, off)| (pitch, beats(on), beats(off - on))),
            &controls.map(|(frame, number)| (beats(frame), ControlKind::Cc, 64, number, 0)),
        );
        let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 8);
        schedule.chase_sampler(switch_keys());
        let capacity = schedule.capacity();
        assert_eq!(capacity, 1 + 2 * 6 + 6 + 4);
        let mut largest = 0;
        for frame in 0..24_i64 {
            for n in [1_i64, 3, 1_000] {
                let end = frame + n;
                let switch = notes
                    .iter()
                    .any(|&(pitch, on, _)| matches!(pitch, 24 | 25) && on < frame);
                let chased_controllers = [1, 11, 64]
                    .iter()
                    .filter(|&&number| controls.iter().any(|&(at, c)| c == number && at < frame))
                    .count();
                let held = notes
                    .iter()
                    .filter(|&&(_, on, off)| on < frame && frame < off);
                let offs = notes
                    .iter()
                    .filter(|&&(_, _, off)| (frame..end).contains(&off));
                let ons = notes
                    .iter()
                    .filter(|&&(_, on, _)| (frame..end).contains(&on));
                let ctls = controls
                    .iter()
                    .filter(|&&(at, _)| (frame..end).contains(&at));
                let expected = 1
                    + usize::from(switch)
                    + chased_controllers
                    + held.count()
                    + offs.count()
                    + ons.count()
                    + ctls.count();
                let n = usize::try_from(n).unwrap();
                let got = schedule.events(frame, n, true).len();
                assert_eq!(got, expected, "frame {frame}, {n} frames");
                largest = largest.max(got);
            }
        }
        assert!(largest <= capacity, "{largest} > {capacity}");
    }
}
