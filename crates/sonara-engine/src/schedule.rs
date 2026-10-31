//! Real-time: the per-block event lookup runs on the audio render thread (see
//! docs/audio-engine.md); [`NoteSchedule::build`] is setup.
//!
//! The MIDI schedule of an instrument track (`NoteSchedule` in `audio/engine/graph.py`):
//!
//! - **Build:** for every unmuted clip, the clip's `playback_notes()` with
//!   `on = beat_to_sample(t)` and `off = beat_to_sample(min(t + length, clip_end))`, at least
//!   `on + 1`; and its controller events inside the clip span. A clip that does not loop plays
//!   each event at `start + (beat - content_offset)`. A looping clip plays the events of its
//!   content `[0, loop_length)` on every pass of the loop, on the passes and the beats its notes
//!   use ([`looped_controls`]), so an event lands on the frame of a note that starts on its
//!   content beat, a curve repeats with the loop, and the value one pass leaves holds into the
//!   next until that pass's first event of the controller. As with a clip that does not loop,
//!   what the content sets before the clip start (a `content_offset` into the loop) is not
//!   played. The prototype dropped the controller events of looping clips; no golden project
//!   has a clip with controller events, so the golden renders stay the same (GLD-01).
//!   Notes are sorted stably by `on`, the note-off order stably by `off`, the controller events
//!   stably by frame, every pass's events placed at build: the per-block lookup stays a binary
//!   search into preallocated slices (RT-02, RT-05).
//! - **Per block** (`[frame, frame + n)`): on a discontinuity (the chase), an all-notes-off and
//!   a note-on for every note with `on < frame < off`, all at offset 0; then the note-offs, the
//!   note-ons and the controller events of the block, stably sorted by offset. The four groups
//!   are each sorted already, so a merge that prefers the earlier group on equal offsets gives
//!   exactly the prototype's stable sort, without the scratch memory a stable sort allocates.
//! - **A sampler's chase** ([`NoteSchedule::chase_sampler`]) restores more than the notes,
//!   at offset 0 between the all-notes-off and the chased notes: a keyswitch event
//!   ([`EventKind::KeySwitch`]) for the last note that started before `frame` on each
//!   keyswitch key (a note that the preset's key offset moves onto a keyswitch key of the
//!   patch, or of a patch of a set), in timeline order, so every patch of a set gets its own
//!   last keyswitch; then the last event before `frame` of each of the controllers 1
//!   (modulation), 11 (expression) and 64 (sustain), on its own channel and in timeline order.
//!   So a seek or a loop wrap plays the articulation, the swell and the pedal that playing up
//!   to the new position leaves. A keyswitch note that an automated change of the sampler's
//!   articulation, mute or vibrato overrode, because the change applied the articulation's
//!   target again after it (at or before `frame`), is sent with `value` 1, so the target keeps
//!   winning over it, as in straight play. A
//!   looping clip's events count on every pass they are placed on, so a position early in a
//!   later pass, before the pass's own event, gets the value the previous pass left. The
//!   sampler makes its default switch active, and applies its articulation parameter again, at
//!   the all-notes-off, so a position before every keyswitch note gets the default or the
//!   parameter's articulation. A controller with no event before `frame` keeps its value, except
//!   the one a sampler's preset drives with its `dynamics` parameter, which the sampler sets to
//!   the parameter's value again at the all-notes-off. Each controller is restored once, by its
//!   last event on whichever channel: a value an earlier event left on another channel is not
//!   restored, which only the sampler's per-channel gates and crossfades can tell (the pedal,
//!   volume and expression it applies act on every channel). Every other instrument (the Poly
//!   Synth and the Drum Kit among them) gets the plain chase, so its events and its golden
//!   renders stay the same (GLD-01).
//! - **A note's own articulation** travels with its note-on
//!   ([`InstrumentEvent::articulation`]), the chased one included, so a note that a seek or a
//!   loop wrap re-sends plays its own articulation again. The build resolves each note's stored
//!   articulation once, off the real-time thread, into the index of a choice of the sampler's
//!   `articulation` parameter, with the resolver the graph gives it: a sampler's resolver looks
//!   the id up among its choices, every other instrument's names none, so its events stay the
//!   same (GLD-01).
//!
//! - **A sampler's automated enum parameters** ([`NoteSchedule::send_choices`]): each change
//!   of one (its articulation, mute or vibrato), computed at build ([`crate::automation::choice_changes`]), is
//!   a choice event ([`EventKind::ParamChoice`]) at its frame. On equal offsets it comes after
//!   the note-offs and before the note-ons, so a note that ends on the change's frame releases
//!   in the choice it played, and a note that starts there plays the new one; a chase block's
//!   own group comes first, so a change on the chase frame follows the chased notes. The
//!   block's parameters still carry the choice at the block's first frame, so a change on that
//!   frame can arrive twice, as a parameter and as an event, and the sampler applies it once.
//!   Every other instrument gets no choice events, so its events stay the same (GLD-01).
//!
//! The event buffer is sized at build for the worst case, every event in one block
//! (`1 + 2 · notes + controls`: a chased note cannot also start in the block), and a sampler's
//! chase adds a slot for each keyswitch key of its keyswitch notes and one for each restored
//! controller that the schedule has, at most 3, and one for each choice change, so no event is
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

use sonara_core::EPSILON;
use sonara_core::midi::{ControlKind, MidiControlEvent};
use sonara_core::model::clip::MidiClip;
use sonara_core::model::py_mod;
use sonara_core::time::TempoMap;
use sonara_dsp::processor::{EventKind, InstrumentEvent, NoteSet};

use crate::automation::ChoiceChange;

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
    /// The choice of the sampler's `articulation` parameter the note plays with, resolved at
    /// build; `None` for a note that plays the parameter's choice.
    articulation: Option<u16>,
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

impl Control {
    /// The schedule's event of the clip's controller event `event` at timeline frame `frame`.
    fn new(event: &MidiControlEvent, frame: i64) -> Self {
        Self {
            frame,
            kind: match event.kind {
                ControlKind::Cc => EventKind::Cc,
                ControlKind::PitchBend => EventKind::PitchBend,
                ControlKind::ChannelPressure => EventKind::ChannelPressure,
            },
            // Validated ranges: CC numbers 0–127 (a pitch bend's number is unused), values
            // within ±8192, channels 0–15.
            number: u8::try_from(event.number).unwrap_or(0),
            value: i32::try_from(event.value).unwrap_or(0),
            channel: u8::try_from(event.channel).unwrap_or(0),
        }
    }
}

/// The most loop passes after the first that a looping clip plays: the bound of
/// `MidiClipData::playback_notes`, so a clip's controller events cover the passes its notes do.
const MAX_LOOP_PASSES: u32 = 100_000;

/// The controller events of the looping clip `clip`, whose loop is `loop_length` beats long, at
/// their timeline beats, pass by pass: every pass plays the events of the content `[0,
/// loop_length)` in their stored order.
///
/// The passes, the content and the span are those of the clip's notes, with the same arithmetic
/// as `MidiClipData::playback_notes`, so an event lands on the very beat (and frame) of a note
/// that starts on its content beat: the first pass begins `content_offset` (modulo the loop)
/// before the clip start, and an event of a pass is played when it lies within the clip span
/// (`EPSILON` tolerant at both ends, as a note).
fn looped_controls(
    clip: &MidiClip,
    loop_length: f64,
) -> impl Iterator<Item = (f64, &MidiControlEvent)> {
    let clip_end = clip.start + clip.length;
    let iteration_start = clip.start - py_mod(clip.content_offset, loop_length);
    (0..=MAX_LOOP_PASSES)
        .map(move |k| iteration_start + f64::from(k) * loop_length)
        .take_while(move |&base| base < clip_end - EPSILON)
        .flat_map(move |base| {
            clip.controls
                .iter()
                .filter(move |event| event.beat < loop_length - EPSILON)
                .map(move |event| (base + event.beat, event))
                .filter(move |&(t, _)| t + EPSILON >= clip.start && t < clip_end - EPSILON)
        })
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
    articulation: None,
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
    /// The changes of a sampler's automated enum parameters, sorted by frame; empty for every
    /// other instrument.
    choices: Vec<ChoiceChange>,
    /// The frames of the changes in `choices` that make the sampler apply its articulation's
    /// target again, sorted; what its chase compares the clip's keyswitch notes with.
    applied: Vec<i64>,
    /// The events of the last block: the first `len` slots.
    events: Vec<InstrumentEvent>,
    len: usize,
}

/// What a sampler's chase restores besides the notes (see the module docs).
#[derive(Debug, Clone)]
struct SamplerChase {
    /// The notes on a keyswitch key of the patch, stably sorted by `on`.
    switches: Vec<Note>,
    /// The keyswitch keys that `switches` has: the most keyswitch events one chase sends.
    switch_keys: usize,
    /// The events of the controllers the chase restores, stably sorted by frame.
    controls: Vec<Control>,
    /// The controllers that `controls` has, one bit each ([`chased_bit`]).
    present: u8,
}

impl SamplerChase {
    /// The events the buffer needs room for: one keyswitch per keyswitch key and one event per
    /// controller.
    fn slots(&self) -> usize {
        self.switch_keys
            .saturating_add(usize::try_from(self.present.count_ones()).unwrap_or(0))
    }

    /// Writes the keyswitches and the controller values that playing up to `frame` leaves.
    /// `applied` holds the frames, sorted, at which the sampler applies its articulation's
    /// target again because an automated choice changes it ([`NoteSchedule::send_choices`]).
    ///
    /// The keyswitches are the last note before `frame` on each keyswitch key, in timeline
    /// order: each patch of a multi-patch set keeps the last of its own, so replaying them in
    /// order leaves every patch where playing up to `frame` left it. A keyswitch note that lies
    /// before the last such frame (at or before `frame`) is sent with `value` 1: the target
    /// applied after it wins over it, as in straight play, and the note only sets the patch's
    /// switch for when the target no longer forces one.
    fn restore(&self, frame: i64, applied: &[i64], out: &mut EventWriter<'_>) {
        let pressed = self.switches.partition_point(|note| note.on < frame);
        let last_applied = applied
            .get(..applied.partition_point(|&at| at <= frame))
            .and_then(<[i64]>::last)
            .copied();
        // The last note of each keyswitch key, found walking back from `frame` (latest first)
        // into a fixed array of their indices, then sent in timeline order (RT-02).
        let mut latest = [0_usize; 128];
        let mut found = 0_usize;
        let mut seen = NoteSet::EMPTY;
        let before = self.switches.get(..pressed).unwrap_or_default();
        for (index, note) in before.iter().enumerate().rev() {
            if found >= self.switch_keys {
                break;
            }
            if seen.contains(note.pitch) {
                continue;
            }
            seen.insert(note.pitch);
            if let Some(slot) = latest.get_mut(found) {
                *slot = index;
                found = found.saturating_add(1);
            }
        }
        for note in latest
            .get(..found)
            .unwrap_or_default()
            .iter()
            .rev()
            .filter_map(|&index| before.get(index))
        {
            let under_target = last_applied.is_some_and(|at| note.on < at);
            out.push(InstrumentEvent {
                kind: EventKind::KeySwitch,
                value: i32::from(under_target),
                articulation: None,
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
    /// The schedule of `clips` (a track's MIDI clips in track order) at `sample_rate`, whose
    /// note-ons carry the articulation `articulation` resolves each note's stored id to: the
    /// index of a choice of the sampler's `articulation` parameter, or `None`, so the note plays
    /// the parameter's choice. A note without a stored articulation carries none.
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
        mut articulation: impl FnMut(&str) -> Option<u16>,
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
                    articulation: note.articulation.as_deref().and_then(&mut articulation),
                });
            }
            if let Some(loop_length) = clip.loop_length {
                controls.extend(looped_controls(clip, loop_length).map(|(beat, event)| {
                    Control::new(event, tempo.beat_to_sample(beat, sample_rate))
                }));
                continue;
            }
            for event in clip.controls.iter() {
                let beat = clip.start + (event.beat - clip.content_offset);
                if !(clip.start <= beat && beat < clip_end) {
                    continue;
                }
                controls.push(Control::new(event, tempo.beat_to_sample(beat, sample_rate)));
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
            choices: Vec::new(),
            applied: Vec::new(),
            events: vec![NO_EVENT; capacity],
            len: 0,
        }
    }

    /// Makes the chase restore what a sampler whose keyswitch notes are `switch_keys` (the clip
    /// pitches that select an articulation of its patch, or of the patches of a set) needs
    /// besides the notes: the last note on each keyswitch key and the last values of the
    /// controllers 1, 11 and 64 (see the module docs). The graph calls it for sampler
    /// instruments only.
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
        let keys: NoteSet = switches.iter().map(|note| note.pitch).collect();
        let switch_keys = (0..=127_u8).filter(|&key| keys.contains(key)).count();
        self.sampler = Some(SamplerChase {
            switches,
            switch_keys,
            controls,
            present,
        });
        self.events.resize(self.worst_case(), NO_EVENT);
    }

    /// Makes the schedule send `changes`, the changes of a sampler's automated enum parameters
    /// sorted by frame ([`crate::automation::choice_changes`]), as choice events at their
    /// frames (see the module docs). `applied` holds the frames, sorted, of the changes that
    /// make the sampler apply its articulation's target again
    /// ([`crate::automation::target_applications`]), so a chase knows which of the clip's
    /// keyswitch notes came after the target and win over it. The graph calls it for sampler
    /// instruments only.
    ///
    /// Setup: allocates (the event buffer grows by a slot per change), so it runs off the
    /// real-time thread, when the graph is built, before the first block.
    pub(crate) fn send_choices(&mut self, changes: Vec<ChoiceChange>, applied: Vec<i64>) {
        self.choices = changes;
        self.applied = applied;
        self.events.resize(self.worst_case(), NO_EVENT);
    }

    /// The most events one block can have: every note, controller and choice change in it,
    /// and a sampler's chase (see the module docs).
    fn worst_case(&self) -> usize {
        plain_capacity(&self.notes, &self.controls)
            .saturating_add(self.sampler.as_ref().map_or(0, SamplerChase::slots))
            .saturating_add(self.choices.len())
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
    /// sampler by the keyswitch and controllers between them; for a sampler with automated
    /// enum parameters, with their choice events among them (see the module docs).
    pub(crate) fn events(&mut self, frame: i64, n: usize, chase: bool) -> &[InstrumentEvent] {
        let end = frame.saturating_add(i64::try_from(n).unwrap_or(i64::MAX));
        let Self {
            notes,
            off_sorted,
            off_order,
            controls,
            sampler,
            choices,
            applied,
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
                sampler.restore(frame, applied, &mut out);
            }
            for note in notes.iter().filter(|n| n.on < frame && n.off > frame) {
                out.push(note_on(note, 0));
            }
        }
        let offs = window(off_sorted, frame, end, |&off| off);
        let ons = window(notes, frame, end, |note| note.on);
        let ctls = window(controls, frame, end, |control| control.frame);
        let chosen = window(choices, frame, end, |change| change.frame);
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
        let mut chosen = choices
            .get(chosen)
            .unwrap_or_default()
            .iter()
            .map(|change| choice_event(change, offset(change.frame, frame)))
            .peekable();
        // The merge: the smallest offset first; on equal offsets note-offs, then note-ons,
        // then controller events (their order in the prototype's list before its stable sort),
        // with a sampler's choice events between the note-offs and the note-ons. Without
        // choice events (`choice` is `None`) it is the merge of the other three alone.
        loop {
            let off = offs.peek().map(|e| e.offset);
            let choice = chosen.peek().map(|e| e.offset);
            let on = ons.peek().map(|e| e.offset);
            let ctl = ctls.peek().map(|e| e.offset);
            let take_off = off.is_some_and(|o| {
                choice.is_none_or(|x| o <= x)
                    && on.is_none_or(|x| o <= x)
                    && ctl.is_none_or(|x| o <= x)
            });
            let take_choice = !take_off
                && choice.is_some_and(|o| on.is_none_or(|x| o <= x) && ctl.is_none_or(|x| o <= x));
            let take_on =
                !take_off && !take_choice && on.is_some_and(|o| ctl.is_none_or(|x| o <= x));
            let next = if take_off {
                offs.next()
            } else if take_choice {
                chosen.next()
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
                    articulation: None,
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
        articulation: note.articulation,
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
        articulation: None,
    }
}

/// The choice event of `change` at `offset` ([`EventKind::ParamChoice`]).
fn choice_event(change: &ChoiceChange, offset: usize) -> InstrumentEvent {
    InstrumentEvent {
        offset,
        kind: EventKind::ParamChoice,
        pitch: 0,
        velocity: 0,
        channel: 0,
        value: i32::from(change.choice),
        number: change.slot,
        articulation: None,
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
        articulation: None,
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
    use crate::automation::ChoiceChange;

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
        let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 8, |_| None);
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
        let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 8, |_| None);
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

    /// A looping clip plays its controller events on every pass, as its notes. The prototype
    /// dropped them (TST-06 pins the change); no golden project has a clip with controller
    /// events, so the golden renders are unaffected (GLD-01).
    #[test]
    fn looped_clips_repeat_notes_and_controls() {
        let tempo = TempoMap::constant(120.0).unwrap();
        // C4 at content beat 0 and CC 1 at content beat 0.5, looped every 2 beats over 4.
        let c = clip(0.0, 4.0, &[(60, 0.0, 0.5)], Some(2.0));
        let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 8, |_| None);
        let all = kinds(schedule.events(0, 16, false));
        assert_eq!(
            all,
            [
                (0, EventKind::NoteOn, 60),
                (2, EventKind::NoteOff, 60),
                (2, EventKind::Cc, 0),
                (8, EventKind::NoteOn, 60),
                (10, EventKind::NoteOff, 60),
                (10, EventKind::Cc, 0),
            ]
        );
        // The buffer has room for every placed event: 1 + 2 · 2 notes + 2 controller events.
        assert_eq!(schedule.capacity(), 1 + 2 * 2 + 2);
    }

    /// A looping clip of `length` beats from `start`, whose content from `content_offset`
    /// repeats every `loop_length` beats, with a note (C4, a sixteenth of a beat long) and a CC 1
    /// event at each content beat of `beats`.
    fn twin_clip(
        start: f64,
        length: f64,
        content_offset: f64,
        loop_length: f64,
        beats: &[f64],
    ) -> MidiClip {
        let ids = ["n_0", "n_1", "n_2", "n_3", "n_4", "n_5", "n_6", "n_7"];
        assert!(beats.len() <= ids.len(), "an ID for every note");
        let notes = ids
            .iter()
            .zip(beats)
            .map(|(id, &beat)| MidiNote::new(*id, 60, beat, 0.0625, 100, 0).unwrap())
            .collect();
        let controls = beats
            .iter()
            .zip(0..)
            .map(|(&beat, value)| {
                MidiControlEvent::new(beat, ControlKind::Cc, value, 1, 0).unwrap()
            })
            .collect();
        MidiClip::new(MidiClipData {
            id: "clp_1".to_owned(),
            name: "Clip".to_owned(),
            start,
            length,
            notes: Arc::new(notes),
            controls: Arc::new(controls),
            content_offset,
            loop_length: Some(loop_length),
            muted: false,
            color: None,
            extra: JsonMap::new(),
        })
        .unwrap()
    }

    /// TST-07: a looping clip's controller event lands on exactly the passes and the beats
    /// where a note that starts on its content beat plays. The oracle is the clip's own
    /// `playback_notes()`, compared bit for bit, on a loop length, an offset and a clip end
    /// that are no binary fractions, so a different order of the arithmetic would show.
    #[test]
    fn a_looped_controller_event_lands_on_the_beat_of_a_note_on_its_content_beat() {
        // Content beats 0.7 (the loop length) and 0.9 lie outside the loop: neither plays.
        let beats = [0.0, 0.1, 0.25, 0.3, 0.6, 0.699_999_9, 0.7, 0.9];
        let cases = [
            (1.1, 3.05, 0.3, 0.7),
            (0.0, 4.0, 0.0, 0.7),
            (2.3, 0.65, 1.55, 0.7),
            (0.4, 7.3, 0.0, 1.1),
            (0.0, 1e-3, 0.0, 0.7),
        ];
        for (start, length, content_offset, loop_length) in cases {
            let c = twin_clip(start, length, content_offset, loop_length, &beats);
            let notes: Vec<(u64, u64)> = c
                .playback_notes()
                .iter()
                .map(|&(t, note)| (t.to_bits(), note.start.to_bits()))
                .collect();
            let controls: Vec<(u64, u64)> = super::looped_controls(&c, loop_length)
                .map(|(t, event)| (t.to_bits(), event.beat.to_bits()))
                .collect();
            assert!(!notes.is_empty(), "the clip from {start} plays notes");
            assert_eq!(controls, notes, "the clip from beat {start}");
            // Through the schedule: each controller event on the frame of its note's note-on.
            let tempo = TempoMap::constant(97.0).unwrap();
            let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 48_000, |_| None);
            let events = schedule.events(0, 1_000_000, false);
            let at = |kind| -> Vec<usize> {
                events
                    .iter()
                    .filter(|e| e.kind == kind)
                    .map(|e| e.offset)
                    .collect()
            };
            assert_eq!(
                at(EventKind::Cc),
                at(EventKind::NoteOn),
                "from beat {start}"
            );
        }
    }

    /// A looping clip's controller events follow its content offset, its loop and its end, as
    /// its notes do. The oracle is the arithmetic of the passes (a beat is 4 frames): the clip
    /// from beat 1 to 5.5 starts half a beat into its 2-beat loop, so the passes begin at beats
    /// 0.5, 2.5 and 4.5. Content beat 0.25 plays at 2.75 and 4.75 (not at 0.75, before the
    /// clip start), content beat 1.5 at 2 and 4 (not at 6, past the clip end), and content
    /// beat 2.5 (past the loop) never.
    #[test]
    fn looped_controller_events_follow_the_offset_the_loop_and_the_clip_end() {
        let tempo = TempoMap::constant(120.0).unwrap();
        let controls = [(0.25, 10), (1.5, 20), (2.5, 30)]
            .map(|(beat, value)| {
                MidiControlEvent::new(beat, ControlKind::Cc, value, 11, 2).unwrap()
            })
            .to_vec();
        let c = MidiClip::new(MidiClipData {
            id: "clp_1".to_owned(),
            name: "Clip".to_owned(),
            start: 1.0,
            length: 4.5,
            notes: Arc::new(Vec::new()),
            controls: Arc::new(controls),
            content_offset: 0.5,
            loop_length: Some(2.0),
            muted: false,
            color: None,
            extra: JsonMap::new(),
        })
        .unwrap();
        let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 8, |_| None);
        assert_eq!(
            details(schedule.events(0, 100, false)),
            [
                (8, EventKind::Cc, 0, 11, 20, 2),
                (11, EventKind::Cc, 0, 11, 10, 2),
                (16, EventKind::Cc, 0, 11, 20, 2),
                (19, EventKind::Cc, 0, 11, 10, 2),
            ]
        );
        assert_eq!(schedule.capacity(), 1 + 4);
        // A block reads only its own window of the placed events.
        assert_eq!(
            details(schedule.events(11, 6, false)),
            [
                (0, EventKind::Cc, 0, 11, 10, 2),
                (5, EventKind::Cc, 0, 11, 20, 2)
            ]
        );
        // A muted looping clip places nothing.
        let muted = c.edit(|d| d.muted = true).unwrap();
        let mut silent = NoteSchedule::build([&muted].into_iter(), &tempo, 8, |_| None);
        assert_eq!(details(silent.events(0, 100, false)), []);
    }

    #[test]
    fn the_pitches_are_those_of_every_note_on() {
        let tempo = TempoMap::constant(120.0).unwrap();
        let a = clip(0.0, 4.0, &[(36, 0.0, 0.5), (42, 1.0, 0.5)], Some(2.0));
        let b = clip(4.0, 2.0, &[(38, 0.5, 1.0), (36, 1.0, 1.0)], None);
        let mut schedule = NoteSchedule::build([&a, &b].into_iter(), &tempo, 8, |_| None);
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
        let empty = NoteSchedule::build(std::iter::empty(), &tempo, 8, |_| None);
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
        let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 8, |_| None);
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
        let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 8, |_| None);
        schedule.chase_sampler(switch_keys());
        schedule
    }

    /// The chase replays the last note on each keyswitch key in timeline order, so the last
    /// switch is sent last and a set's patches each get their own (TST-06: the chase used to
    /// send the last switch alone, which lost the switch of every other patch of a set).
    #[test]
    fn a_sampler_chase_restores_the_last_switches_and_controllers() {
        let mut schedule = sampler_schedule();
        // The plain worst case (1 + 2 · 4 notes + 8 controls) plus a keyswitch for each of the
        // two keyswitch keys and the three controllers.
        assert_eq!(schedule.capacity(), 17 + 2 + 3);
        // At frame 10 (beat 2.5): the last C1 (frame 0) and the last C#1 (frame 2), then the
        // last CC 11 (frame 2), CC 1 (frame 3) and CC 64 (frame 4, channel 1) in timeline
        // order, then C4.
        assert_eq!(
            details(schedule.events(10, 1, true)),
            [
                (0, EventKind::AllNotesOff, 0, 0, 0, 0),
                (0, EventKind::KeySwitch, 24, 0, 0, 0),
                (0, EventKind::KeySwitch, 25, 0, 0, 0),
                (0, EventKind::Cc, 0, 11, 90, 0),
                (0, EventKind::Cc, 0, 1, 20, 0),
                (0, EventKind::Cc, 0, 64, 100, 1),
                (0, EventKind::NoteOn, 60, 0, 0, 0),
            ]
        );
        // A block that continues the last one does not chase.
        assert_eq!(details(schedule.events(11, 1, false)), []);
        // After beat 3 the last switch is C1 again, after C#1, and the last CC 11 is 40.
        assert_eq!(
            details(schedule.events(14, 1, true)),
            [
                (0, EventKind::AllNotesOff, 0, 0, 0, 0),
                (0, EventKind::KeySwitch, 25, 0, 0, 0),
                (0, EventKind::KeySwitch, 24, 0, 0, 0),
                (0, EventKind::Cc, 0, 1, 20, 0),
                (0, EventKind::Cc, 0, 64, 100, 1),
                (0, EventKind::Cc, 0, 11, 40, 0),
            ]
        );
    }

    /// A change of a sampler's parameter 5 (its `articulation`) to `choice` at `frame`.
    fn change(frame: i64, choice: u16) -> ChoiceChange {
        ChoiceChange {
            frame,
            slot: 5,
            choice,
        }
    }

    /// A sampler's choice events come, on equal offsets, after the note-offs and before the
    /// note-ons and controllers, and in a chase block after the chase's own events. Each
    /// change has a slot in the buffer, whichever setup call comes last; no change, none.
    #[test]
    fn choice_events_sit_between_note_offs_and_note_ons() {
        let mut schedule = sampler_schedule();
        let capacity = schedule.capacity();
        schedule.send_choices(Vec::new(), Vec::new());
        assert_eq!(schedule.capacity(), capacity);
        schedule.send_choices(
            [change(8, 3), change(12, 4), change(13, 2)].to_vec(),
            Vec::new(),
        );
        assert_eq!(schedule.capacity(), capacity + 3);
        schedule.chase_sampler(switch_keys());
        assert_eq!(schedule.capacity(), capacity + 3);
        // Frame 12: C4 ends, a change, C1 starts and CC 11 moves; frame 13: C1 ends, a change.
        assert_eq!(
            details(schedule.events(12, 4, false)),
            [
                (0, EventKind::NoteOff, 60, 0, 0, 0),
                (0, EventKind::ParamChoice, 0, 5, 4, 0),
                (0, EventKind::NoteOn, 24, 0, 0, 0),
                (0, EventKind::Cc, 0, 11, 40, 0),
                (1, EventKind::NoteOff, 24, 0, 0, 0),
                (1, EventKind::ParamChoice, 0, 5, 2, 0),
            ]
        );
        // A chase at frame 8, where C4 starts with a change: the chase's events first.
        assert_eq!(
            details(schedule.events(8, 1, true)),
            [
                (0, EventKind::AllNotesOff, 0, 0, 0, 0),
                (0, EventKind::KeySwitch, 24, 0, 0, 0),
                (0, EventKind::KeySwitch, 25, 0, 0, 0),
                (0, EventKind::Cc, 0, 11, 90, 0),
                (0, EventKind::Cc, 0, 1, 20, 0),
                (0, EventKind::Cc, 0, 64, 100, 1),
                (0, EventKind::ParamChoice, 0, 5, 3, 0),
                (0, EventKind::NoteOn, 60, 0, 0, 0),
            ]
        );
        // Outside the changes' frames, the events are those without them.
        let mut plain = sampler_schedule();
        for frame in [0, 3, 9, 14] {
            let with = details(schedule.events(frame, 1, true));
            assert_eq!(with, details(plain.events(frame, 1, true)), "frame {frame}");
        }
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
        let mut plain = NoteSchedule::build([&c].into_iter(), &tempo, 8, |_| None);
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

    /// A sampler's chase early in a later pass of a looping clip restores the value the
    /// previous pass left, as playing through the loop does. The clip loops every 2 beats (8
    /// frames) over 8 beats and sets CC 11 to 100 at content beat 0.5 and to 50 at content
    /// beat 1.5: the oracle is the frames of its passes, 2 and 6, 10 and 14, 18 and 22, 26 and
    /// 30.
    #[test]
    fn a_sampler_chase_in_a_later_pass_restores_what_the_previous_pass_left() {
        let tempo = TempoMap::constant(120.0).unwrap();
        let controls = [(0.5, 100), (1.5, 50)]
            .map(|(beat, value)| {
                MidiControlEvent::new(beat, ControlKind::Cc, value, 11, 0).unwrap()
            })
            .to_vec();
        let c = MidiClip::new(MidiClipData {
            id: "clp_1".to_owned(),
            name: "Clip".to_owned(),
            start: 0.0,
            length: 8.0,
            notes: Arc::new(Vec::new()),
            controls: Arc::new(controls),
            content_offset: 0.0,
            loop_length: Some(2.0),
            muted: false,
            color: None,
            extra: JsonMap::new(),
        })
        .unwrap();
        let mut plain = NoteSchedule::build([&c].into_iter(), &tempo, 8, |_| None);
        let mut schedule = plain.clone();
        schedule.chase_sampler(NoteSet::EMPTY);
        // Eight placed events and the restored CC 11.
        assert_eq!(schedule.capacity(), 1 + 8 + 1);
        let off = (0, EventKind::AllNotesOff, 0, 0, 0, 0);
        let expression = |value| (0, EventKind::Cc, 0, 11, value, 0);
        // (frame, the restored value): nothing before the first event; in the first pass after
        // its second event; at the wrap and in the second pass before its first event, the
        // first pass's last value; after it, the second pass's first; in the last pass, before
        // its first event, the third pass's last.
        let cases = [
            (1, None),
            (7, Some(50)),
            (8, Some(50)),
            (9, Some(50)),
            (11, Some(100)),
            (25, Some(50)),
        ];
        for (frame, restored) in cases {
            let mut expected = [off].to_vec();
            expected.extend(restored.map(expression));
            assert_eq!(
                details(schedule.events(frame, 1, true)),
                expected,
                "frame {frame}"
            );
            // A plain chase restores no controller.
            assert_eq!(
                details(plain.events(frame, 1, true)),
                [off],
                "frame {frame}"
            );
        }
        // From the wrap on, a block plays the second pass's own events after the restored one.
        assert_eq!(
            details(schedule.events(8, 8, true)),
            [
                off,
                expression(50),
                (2, EventKind::Cc, 0, 11, 100, 0),
                (6, EventKind::Cc, 0, 11, 50, 0),
            ]
        );
        // Every placed event fits the buffer with the chase.
        let all = schedule.events(1, 1_000, true).len();
        assert_eq!(all, 1 + 8);
        assert!(all <= schedule.capacity());
    }

    #[test]
    fn a_held_switch_note_is_chased_as_a_note_too() {
        // As before, a note sounding at the position is re-sent; a held keyswitch note also
        // comes first as the keyswitch.
        let tempo = TempoMap::constant(120.0).unwrap();
        let c = clip_with(&[(25, 0.0, 4.0), (60, 1.0, 2.0)], &[]);
        let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 8, |_| None);
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

    /// A clip of 4 beats from beat 0 with `notes` (pitch, start, length and the stored
    /// articulation) at velocity 100 on channel 0, and CC 1 at 64 from beat 0.5.
    fn marked_clip(notes: &[(u8, f64, f64, Option<&str>)]) -> MidiClip {
        let ids = ["n_0", "n_1", "n_2", "n_3"];
        assert!(notes.len() <= ids.len(), "an ID for every note");
        let notes = ids
            .iter()
            .zip(notes)
            .map(|(id, &(pitch, s, l, articulation))| {
                MidiNote::new(*id, i64::from(pitch), s, l, 100, 0)
                    .unwrap()
                    .with_articulation(articulation.map(str::to_owned))
                    .unwrap()
            })
            .collect();
        let controls = [MidiControlEvent::new(0.5, ControlKind::Cc, 64, 1, 0).unwrap()].to_vec();
        MidiClip::new(MidiClipData {
            id: "clp_1".to_owned(),
            name: "Clip".to_owned(),
            start: 0.0,
            length: 4.0,
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

    /// Offset, kind, pitch and articulation of each event.
    fn marks(events: &[InstrumentEvent]) -> Vec<(usize, EventKind, u8, Option<u16>)> {
        events
            .iter()
            .map(|e| (e.offset, e.kind, e.pitch, e.articulation))
            .collect()
    }

    /// The notes of the articulation tests: C1, a keyswitch key, at beat 0 marked `staccato`;
    /// C4 from beat 0 to 1 marked `staccato`, E4 from 1 to 2 without an articulation and G4
    /// from 2 to 3 marked with an id the resolver does not know.
    const MARKED: [(u8, f64, f64, Option<&str>); 4] = [
        (24, 0.0, 0.25, Some("staccato")),
        (60, 0.0, 1.0, Some("staccato")),
        (64, 1.0, 1.0, None),
        (67, 2.0, 1.0, Some("flutter")),
    ];

    #[test]
    fn a_note_on_carries_the_articulation_its_resolver_names() {
        let tempo = TempoMap::constant(120.0).unwrap();
        let c = marked_clip(&MARKED);
        // How often the resolver is asked for `staccato` and for another id.
        let mut asked = (0, 0);
        let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 8, |id| {
            if id == "staccato" {
                asked.0 += 1;
                Some(3)
            } else {
                asked.1 += 1;
                None
            }
        });
        // The resolver is asked once for each note with a stored articulation, at the build.
        assert_eq!(asked, (2, 1));
        // Only the note-ons carry one: the note-offs and the controller event carry none.
        let marked = [
            (0, EventKind::NoteOn, 24, Some(3)),
            (0, EventKind::NoteOn, 60, Some(3)),
            (1, EventKind::NoteOff, 24, None),
            (2, EventKind::Cc, 0, None),
            (4, EventKind::NoteOff, 60, None),
            (4, EventKind::NoteOn, 64, None),
            (8, EventKind::NoteOff, 64, None),
            (8, EventKind::NoteOn, 67, None),
            (12, EventKind::NoteOff, 67, None),
        ];
        assert_eq!(marks(schedule.events(0, 16, false)), marked);
        // A chase inside C4 re-sends it with its articulation.
        assert_eq!(
            marks(schedule.events(3, 1, true)),
            [
                (0, EventKind::AllNotesOff, 0, None),
                (0, EventKind::NoteOn, 60, Some(3)),
            ]
        );
        // A sampler's chased keyswitch selects the switch alone: it carries no articulation.
        schedule.chase_sampler(switch_keys());
        assert_eq!(
            marks(schedule.events(3, 1, true)),
            [
                (0, EventKind::AllNotesOff, 0, None),
                (0, EventKind::KeySwitch, 24, None),
                (0, EventKind::Cc, 0, None),
                (0, EventKind::NoteOn, 60, Some(3)),
            ]
        );
        // A resolver that names none (every instrument but a sampler) gives the same events
        // without an articulation, so their order and timing do not depend on it.
        let mut plain = NoteSchedule::build([&c].into_iter(), &tempo, 8, |_| None);
        assert_eq!(
            marks(plain.events(0, 16, false)),
            marked.map(|(offset, kind, pitch, _)| (offset, kind, pitch, None))
        );
        assert_eq!(
            plain.capacity(),
            schedule.capacity() - 2,
            "only the chase grows it"
        );
    }

    /// A chased keyswitch that an automated change of the articulation's target came after
    /// yields to the target (`value` 1); one after the last such change, or with none before
    /// the position, wins (`value` 0). A change on the position's own frame counts: the block's
    /// parameters may already have applied it (TST-06: the chase used to let the clip's last
    /// keyswitch win over a target applied after it, so a seek played another articulation than
    /// straight play).
    #[test]
    fn a_chased_keyswitch_yields_to_a_target_applied_after_it() {
        let switches = |schedule: &mut NoteSchedule, frame: i64| -> Vec<(u8, i32)> {
            schedule
                .events(frame, 1, true)
                .iter()
                .filter(|event| event.kind == EventKind::KeySwitch)
                .map(|event| (event.pitch, event.value))
                .collect()
        };
        // C1 at frame 0, C#1 at frame 2, C1 again at frame 12 ([`sampler_schedule`]); the
        // target applied again at frames 6 and 14.
        let mut schedule = sampler_schedule();
        schedule.send_choices([change(6, 3), change(14, 2)].to_vec(), [6, 14].to_vec());
        // Before frame 6 nothing was applied after the switches: they win.
        assert_eq!(switches(&mut schedule, 5), [(24, 0), (25, 0)]);
        // From frame 6 on the target came after both.
        assert_eq!(switches(&mut schedule, 6), [(24, 1), (25, 1)]);
        assert_eq!(switches(&mut schedule, 10), [(24, 1), (25, 1)]);
        // C1 at frame 12 came after the change at frame 6, C#1 before it.
        assert_eq!(switches(&mut schedule, 13), [(25, 1), (24, 0)]);
        // The change at frame 14, the position's own frame, came after both again.
        assert_eq!(switches(&mut schedule, 14), [(25, 1), (24, 1)]);
        // Without applied frames every chased keyswitch wins, as before.
        let mut plain = sampler_schedule();
        plain.send_choices([change(6, 3), change(14, 2)].to_vec(), Vec::new());
        assert_eq!(switches(&mut plain, 14), [(25, 0), (24, 0)]);
    }

    /// TST-07: a chase never drops an event, choice events included. The oracle counts the
    /// events of each block from the clip's own frames (a beat is 4 frames) and the frames of
    /// the changes, not from the schedule.
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
        // Changes of two parameters, two of them on one frame.
        let changes = [
            change(0, 3),
            change(4, 2),
            change(12, 4),
            ChoiceChange {
                frame: 16,
                slot: 4,
                choice: 1,
            },
            change(16, 3),
        ];
        let mut schedule = NoteSchedule::build([&c].into_iter(), &tempo, 8, |_| None);
        schedule.send_choices(changes.to_vec(), Vec::new());
        schedule.chase_sampler(switch_keys());
        let capacity = schedule.capacity();
        // The plain worst case, a keyswitch for each of the two keyswitch keys, the three
        // chased controllers and the five changes.
        assert_eq!(capacity, 1 + 2 * 6 + 6 + 2 + 3 + 5);
        let mut largest = 0;
        for frame in 0..24_i64 {
            for n in [1_i64, 3, 1_000] {
                let end = frame + n;
                let switches = [24, 25]
                    .iter()
                    .filter(|&&key| {
                        notes
                            .iter()
                            .any(|&(pitch, on, _)| pitch == key && on < frame)
                    })
                    .count();
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
                let chosen = changes
                    .iter()
                    .filter(|change| (frame..end).contains(&change.frame));
                let expected = 1
                    + switches
                    + chased_controllers
                    + held.count()
                    + offs.count()
                    + ons.count()
                    + ctls.count()
                    + chosen.count();
                let n = usize::try_from(n).unwrap();
                let got = schedule.events(frame, n, true).len();
                assert_eq!(got, expected, "frame {frame}, {n} frames");
                largest = largest.max(got);
            }
        }
        assert!(largest <= capacity, "{largest} > {capacity}");
    }
}
