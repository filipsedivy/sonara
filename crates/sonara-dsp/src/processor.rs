//! Real-time: the processor contract shared by the built-in effects and instruments; `process`
//! runs on the render thread (see docs/audio-engine.md).
//!
//! It mirrors the prototype's `sonara/dsp/base.py` (TRT, `design.md`):
//!
//! - **`prepare`** (setup) is called before the first block and whenever the sample rate or the
//!   block size changes. Like the prototype it resets the state and then applies the current
//!   parameters (`reset()` followed by `on_params_changed()`), which may be the empty set.
//! - **`set_params`** receives the full, validated parameter set in descriptor order, **every
//!   block**: the render graph samples device automation once per block. Like the prototype's
//!   dict comparison, it recomputes coefficients only when the new set differs from the stored
//!   one, so block-clocked behaviour (the EQ's glide, the filter's LFO) keeps the prototype's
//!   timing. The slots have a fixed capacity, so the call allocates nothing.
//! - **`set_state`** (setup) receives non-parameter state: a convolver's impulse response, and
//!   the notes an instrument can receive (every note, unless an offline render passes only
//!   the notes its schedule plays).
//! - **`process`** works in place on one block of stereo frames.
//! - **`latency`** and **`tail_seconds`** are read at the points the renderer reads them; they
//!   may change with the parameters (the limiter's look-ahead, the delay's time).
//! - **`report`** returns live measurements (gain reduction and the like). They are part of the
//!   golden data (`plugins/<id>/<config>/config.json`), so they are compared too.
//!
//! A processor reproduces the prototype's Python wrapper (`dsp/*.py`), including its clamps,
//! smoothers and block-size-dependent state; the kernels in [`crate::kernels`] stay raw.
//!
//! **Deferred setup**. Two processors do setup work when a parameter
//! changes: the reverb synthesizes its impulse response when a parameter that shapes it changes,
//! and the convolver reloads its response when "Normalize IR" changes. Offline, and in the
//! device setup of every graph, that work runs in the call, as in the prototype. A live graph
//! calls [`Processor::defer_setup`] once a processor is set up; from then on `set_params` never
//! does setup work:
//!
//! 1. the processor keeps playing its current setup and names the setup it waits for in
//!    [`Processor::setup_request`], a small `Copy` value that crosses a real-time queue;
//! 2. the builder thread runs [`SetupRequest::prepare`], which does the work and allocates;
//! 3. the render thread moves the result in with [`Processor::install_setup`], without
//!    allocating or freeing, and hands what it displaced back for dropping (RT-09).
//!
//! Until the prepared setup arrives the old impulse response plays on: a timing deviation of
//! live playback only. The convolver never waits for setup: its response changes only with the
//! device state or with "Normalize IR", which cannot be automated, so a live graph replaces a
//! convolver whose response changes ([`crate::processors::reusable`]).

use std::fmt;
use std::sync::Arc;

use crate::bank::SamplerSetup;
use crate::processors::setup::{Prepared, Request};

/// Most parameters a processor has (the poly synth has 37).
pub const MAX_PARAMS: usize = 48;
/// Most entries a [`Report`] holds.
pub const MAX_REPORT: usize = 8;

/// The processing format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessSpec {
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Frames per `process` call.
    pub block_size: usize,
    /// Channels per frame (the render graph uses 2).
    pub channels: usize,
}

/// The kind of an [`InstrumentEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// A key press.
    NoteOn,
    /// A key release.
    NoteOff,
    /// A control change.
    Cc,
    /// Pitch bend.
    PitchBend,
    /// Channel pressure.
    ChannelPressure,
    /// Every sounding note stops.
    AllNotesOff,
    /// The keyswitch key `pitch` selects its articulation without sounding: no region starts
    /// and the key does not count as held. The engine's chase sends it to a sampler for the
    /// last keyswitch note before the new position; an instrument without keyswitches ignores
    /// it.
    KeySwitch,
}

/// A MIDI-like event at frame `offset` within the current block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstrumentEvent {
    /// Frame within the block.
    pub offset: usize,
    /// What happened.
    pub kind: EventKind,
    /// MIDI pitch (notes), or the key of a [`EventKind::KeySwitch`].
    pub pitch: u8,
    /// Velocity (notes).
    pub velocity: u8,
    /// MIDI channel.
    pub channel: u8,
    /// Controller value, bend or pressure.
    pub value: i32,
    /// Controller number.
    pub number: u8,
    /// The articulation a note-on plays with: the index of a choice of the sampler's
    /// `articulation` parameter, which takes the place of the parameter's choice for this note
    /// only. `None` for a note that plays the parameter's choice and for every other event. A
    /// sampler's schedule sets it from the note's stored articulation; every other instrument
    /// ignores it.
    pub articulation: Option<u16>,
}

/// What a block is processed with.
#[derive(Debug, Clone, Copy)]
pub struct BlockContext<'a> {
    /// Timeline frame of the block's first sample (after the latency pre-roll).
    pub frame: i64,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Beat position at the block start.
    pub beat: f64,
    /// Tempo at the block start.
    pub bpm: f64,
    /// Whether the transport plays.
    pub playing: bool,
    /// The sidechain input, for processors that declare one.
    pub sidechain: Option<&'a [[f64; 2]]>,
    /// Instrument events of this block, sorted by offset.
    pub events: &'a [InstrumentEvent],
}

/// One parameter value in its canonical type (`ParamSpec.coerce`): an enum choice is its index
/// into the descriptor's choices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamSlot {
    /// A float parameter.
    Float(f64),
    /// An integer parameter.
    Int(i64),
    /// A switch.
    Bool(bool),
    /// An enum parameter: the index of the choice.
    Choice(u16),
}

/// A processor's full parameter set in descriptor order, with a fixed capacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSlots {
    slots: [ParamSlot; MAX_PARAMS],
    len: usize,
}

impl ParamSlots {
    /// The empty set (before the first `set_params`).
    pub const EMPTY: Self = Self {
        slots: [ParamSlot::Bool(false); MAX_PARAMS],
        len: 0,
    };

    /// A set from slots in descriptor order; `None` when there are more than [`MAX_PARAMS`].
    #[must_use]
    pub fn new(values: &[ParamSlot]) -> Option<Self> {
        let mut out = Self::EMPTY;
        let target = out.slots.get_mut(..values.len())?;
        target.copy_from_slice(values);
        out.len = values.len();
        Some(out)
    }

    /// The slots.
    #[must_use]
    pub fn as_slice(&self) -> &[ParamSlot] {
        self.slots.get(..self.len).unwrap_or(&[])
    }

    /// Whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Replaces the slot at `index` (automation writes into the set in place).
    pub fn set(&mut self, index: usize, value: ParamSlot) {
        if let Some(slot) = self
            .slots
            .get_mut(..self.len)
            .and_then(|s| s.get_mut(index))
        {
            *slot = value;
        }
    }

    /// The slot at `index`.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<ParamSlot> {
        self.as_slice().get(index).copied()
    }
}

/// An impulse response as the media layer supplies it: interleaved frames at the project rate.
#[derive(Debug, Clone, PartialEq)]
pub struct ImpulseResponse {
    /// Channels per frame (1 or 2).
    pub channels: usize,
    /// Interleaved samples.
    pub samples: Vec<f64>,
}

/// A set of MIDI notes (0–127), without allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NoteSet(u128);

impl NoteSet {
    /// Every MIDI note.
    pub const ALL: Self = Self(u128::MAX);
    /// No note.
    pub const EMPTY: Self = Self(0);

    /// Adds `pitch`; a value above 127 is not a MIDI note and is ignored.
    pub fn insert(&mut self, pitch: u8) {
        if let Some(bit) = 1_u128.checked_shl(u32::from(pitch)) {
            self.0 |= bit;
        }
    }

    /// Whether `pitch` is in the set (never for a value above 127).
    #[must_use]
    pub fn contains(self, pitch: u8) -> bool {
        1_u128
            .checked_shl(u32::from(pitch))
            .is_some_and(|bit| self.0 & bit != 0)
    }
}

/// Every note: what an instrument must be ready for when live input can play any.
impl Default for NoteSet {
    fn default() -> Self {
        Self::ALL
    }
}

impl FromIterator<u8> for NoteSet {
    fn from_iter<I: IntoIterator<Item = u8>>(pitches: I) -> Self {
        let mut set = Self::EMPTY;
        for pitch in pitches {
            set.insert(pitch);
        }
        set
    }
}

/// Non-parameter device state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeviceState {
    /// A convolver's impulse response.
    pub impulse_response: Option<Arc<ImpulseResponse>>,
    /// The notes an instrument can receive. [`NoteSet::ALL`] by default, because live input
    /// can play any note and the render thread must not do setup work (RT-02). An offline
    /// render, which has no live input, passes only the notes its schedule plays; an
    /// instrument may then skip the setup work for the others (the drum kit synthesizes only
    /// the sounds of these notes), so a note outside the set may play silence.
    pub playable_notes: NoteSet,
    /// A sampler's instrument: the decoded samples and regions of its SFZ patch and the
    /// mapping of the preset it plays ([`SamplerSetup`]), built off the real-time thread and
    /// shared read-only.
    pub sampler: Option<Arc<SamplerSetup>>,
}

/// Live measurements: up to [`MAX_REPORT`] named values, in the prototype's key order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Report {
    entries: [(&'static str, f64); MAX_REPORT],
    len: usize,
}

impl Report {
    /// No measurements.
    pub const EMPTY: Self = Self {
        entries: [("", 0.0); MAX_REPORT],
        len: 0,
    };

    /// A report from `(name, value)` pairs; entries beyond [`MAX_REPORT`] are dropped.
    #[must_use]
    pub fn new(entries: &[(&'static str, f64)]) -> Self {
        let mut out = Self::EMPTY;
        for (slot, entry) in out.entries.iter_mut().zip(entries) {
            *slot = *entry;
            out.len += 1;
        }
        out
    }

    /// The entries.
    #[must_use]
    pub fn entries(&self) -> &[(&'static str, f64)] {
        self.entries.get(..self.len).unwrap_or(&[])
    }
}

/// Setup that a processor in deferred mode waits for (see the module docs): a small `Copy`
/// value, so the render thread can post it to the builder thread through a real-time queue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SetupRequest(pub(crate) Request);

impl SetupRequest {
    /// Does the setup work the request names: for the reverb, synthesizes the impulse response
    /// and partitions it for the convolver.
    ///
    /// Setup: allocates, so it runs on the builder thread, never on the real-time thread.
    pub fn prepare(&self) -> PreparedSetup {
        PreparedSetup(self.0.prepare())
    }
}

/// Setup prepared off the real-time thread for [`Processor::install_setup`]; after an install,
/// what the install displaced. Either way it owns heap memory, so drop it off the real-time
/// thread (RT-09).
#[must_use = "dropping frees memory: drop it off the real-time thread (RT-09)"]
pub struct PreparedSetup(pub(crate) Prepared);

impl PreparedSetup {
    /// Whether it holds setup that no processor has installed yet.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.0.is_pending()
    }
}

impl fmt::Debug for PreparedSetup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PreparedSetup").field(&self.0).finish()
    }
}

/// An audio processor: a built-in effect or instrument.
pub trait Processor: Send {
    /// Setup: adopts the format, resets the state and applies the current parameters. Runs off
    /// the real-time thread and may allocate.
    fn prepare(&mut self, spec: ProcessSpec);

    /// The full parameter set in descriptor order, every block; recomputes only on a change.
    fn set_params(&mut self, params: &ParamSlots);

    /// Setup: non-parameter state, such as an impulse response or the notes an instrument can
    /// receive ([`DeviceState::playable_notes`]).
    fn set_state(&mut self, state: &DeviceState) {
        let _ = state;
    }

    /// Processes one block of stereo frames in place.
    fn process(&mut self, buffer: &mut [[f64; 2]], ctx: &BlockContext<'_>);

    /// Clears internal state: delay lines, envelopes, voices.
    fn reset(&mut self) {}

    /// Processing delay in frames, compensated by the render graph.
    fn latency(&self) -> usize {
        0
    }

    /// How long the processor keeps producing output after silent input, in seconds.
    fn tail_seconds(&self) -> f64 {
        0.0
    }

    /// Live measurements for the GUI and agents.
    fn report(&self) -> Report {
        Report::EMPTY
    }

    /// Setup: switches the processor to deferred setup for a live graph (see the module docs).
    /// Afterwards [`Processor::set_params`] never does setup work: a change that needs some
    /// keeps the current setup and shows up in [`Processor::setup_request`].
    /// [`Processor::prepare`] and [`Processor::set_state`] stay setup and still do their work
    /// in the call. Processors without parameter-driven setup ignore it.
    fn defer_setup(&mut self) {}

    /// The setup the current parameters wait for, if any; `None` when the installed setup
    /// matches them, and always outside deferred mode.
    ///
    /// Real-time: compares a few values.
    fn setup_request(&self) -> Option<SetupRequest> {
        None
    }

    /// Moves prepared setup in, without allocating or freeing, and leaves what it displaced in
    /// `setup`, to be dropped off the real-time thread. Returns whether it installed: false,
    /// with `setup` unchanged, for setup of another processor kind or setup that is already
    /// installed.
    ///
    /// Real-time.
    fn install_setup(&mut self, setup: &mut PreparedSetup) -> bool {
        let _ = setup;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{DeviceState, MAX_PARAMS, NoteSet, ParamSlot, ParamSlots, Report};

    #[test]
    fn note_sets_hold_midi_notes_only() {
        let set: NoteSet = [0, 36, 42, 127, 128, 255].into_iter().collect();
        let members: Vec<u8> = (0..=u8::MAX).filter(|&p| set.contains(p)).collect();
        assert_eq!(members, [0, 36, 42, 127], "values above 127 are not notes");
        assert!((0..=127).all(|p| NoteSet::ALL.contains(p)));
        assert!(!NoteSet::ALL.contains(128));
        assert!((0..=u8::MAX).all(|p| !NoteSet::EMPTY.contains(p)));
        assert_eq!(
            DeviceState::default().playable_notes,
            NoteSet::ALL,
            "every note by default"
        );
    }

    #[test]
    fn param_slots_keep_order_and_capacity() {
        let slots = ParamSlots::new(&[ParamSlot::Float(1.0), ParamSlot::Choice(2)]);
        let mut slots = slots.unwrap_or(ParamSlots::EMPTY);
        assert_eq!(slots.as_slice().len(), 2);
        slots.set(0, ParamSlot::Float(0.5));
        assert_eq!(slots.get(0), Some(ParamSlot::Float(0.5)));
        assert_eq!(slots.get(2), None);
        assert!(ParamSlots::new(&[ParamSlot::Bool(true); MAX_PARAMS + 1]).is_none());
        assert_ne!(slots, ParamSlots::EMPTY);
    }

    #[test]
    fn reports_keep_their_order() {
        let report = Report::new(&[("gain_reduction_db", -3.0), ("input_db", -12.0)]);
        let names: Vec<&str> = report.entries().iter().map(|(name, _)| *name).collect();
        assert_eq!(names, ["gain_reduction_db", "input_db"]);
    }
}
