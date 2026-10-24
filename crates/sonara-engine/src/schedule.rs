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
//!
//! The event buffer is sized at build for the worst case, every event in one block
//! (`1 + 2 · notes + controls`: a chased note cannot also start in the block), so no event is
//! ever dropped.
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
    /// The events of the last block: the first `len` slots.
    events: Vec<InstrumentEvent>,
    len: usize,
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
        let capacity = notes
            .len()
            .saturating_mul(2)
            .saturating_add(controls.len())
            .saturating_add(1);
        Self {
            off_sorted: by_off.iter().map(|&(off, _)| off).collect(),
            off_order: by_off.iter().map(|&(_, index)| index).collect(),
            notes,
            controls,
            events: vec![NO_EVENT; capacity],
            len: 0,
        }
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
    /// by an all-notes-off and the notes sounding at `frame` (`NoteSchedule.events`).
    pub(crate) fn events(&mut self, frame: i64, n: usize, chase: bool) -> &[InstrumentEvent] {
        let end = frame.saturating_add(i64::try_from(n).unwrap_or(i64::MAX));
        let Self {
            notes,
            off_sorted,
            off_order,
            controls,
            events,
            len,
        } = self;
        let mut out = EventWriter {
            slots: events.as_mut_slice(),
            len: 0,
        };
        if chase {
            out.push(NO_EVENT);
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
    use sonara_core::time::TempoMap;
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
}
