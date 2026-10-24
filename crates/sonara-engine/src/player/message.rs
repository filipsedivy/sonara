//! Real-time: the render thread applies these messages (see docs/audio-engine.md);
//! `MidiChange::new` and `MidiChange::remove_port` are setup on the control thread.
//!
//! What the control thread hands to the render thread, in one ordered `sonara_rt::handoff`
//! channel (RT-04, RT-09), and the fixed-size entries that come back through
//! `sonara_rt::queue`s.
//!
//! One channel carries everything the render thread must apply in order: a new start with its
//! fresh graph, a seek, a halt, a rebuilt graph, prepared setup, the stream, the MIDI inputs and
//! the live-input targets. Two rings could not keep their order across each other (a start could
//! arrive before its graph). The render thread changes each message in place, swapping the new
//! value with the one it replaces, and the message goes back to the control thread, which drops
//! the old graph, stream, feed, receivers or targets there (RT-09).
//!
//! These are plain data; the render side applies them in `super::render`.
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

use sonara_dsp::processor::{InstrumentEvent, Report};
use sonara_rt::{BlockProducer, Generation, MidiEvent, MidiReceiver, StreamControl, handoff};

use super::clicks::ClickTables;
use super::frames::LoopRange;
use crate::NodeMeter;
use crate::graph::{GraphId, MediaKey, NodeIndex};
use crate::update::Update;

/// A message to the render thread. After the render thread applied it, it holds what the
/// render thread replaced, to be dropped on the control thread.
pub(crate) enum Message {
    /// A rebuilt graph (or a fresh one after a rejection) with the targets of its live input,
    /// and a new media feed when it plays other clips than the graph it replaces. Only a
    /// swapped-in graph takes the feed and the targets.
    Graph {
        update: Update,
        feed: Option<MediaFeed>,
        targets: LiveTargets,
    },
    /// Prepared setup for a processor (`Update::Setup`).
    Setup(Update),
    /// Takes the graph away (`Update::Graph(None)`), so that a play resets it on the control
    /// thread: the prototype resets the graph it has instead of building a new one, and state
    /// that survives a processor's reset (the synth's note counter, which seeds its voices)
    /// must survive a play too.
    Recall(Update),
    /// A play: a fresh graph, its media feed and live-input targets, and the sequence to play.
    Start {
        graph: Update,
        feed: Option<MediaFeed>,
        targets: LiveTargets,
        cue: Cue,
    },
    /// A seek while playing: a new generation from `frame`, keeping the end and the loop.
    Seek {
        generation: Generation,
        frame: i64,
        feed: Option<MediaFeed>,
    },
    /// Pause or stop: no more blocks of the playing sequence. Idle blocks (live input) take
    /// `generation` and the resting position `rest`. The feed is taken away.
    Halt {
        generation: Generation,
        rest: i64,
        feed: Option<MediaFeed>,
    },
    /// A new resting position while halted (a seek while stopped or paused).
    Rest(i64),
    /// The stream to write to (`None` takes it away).
    Stream(Option<Box<StreamPart>>),
    /// MIDI inputs to add and ports to remove; the removed inputs come back in it.
    Midi(Box<MidiChange>),
    /// New live-input targets for the current graph (a focus change).
    Targets(LiveTargets),
    /// A GUI preview note (the piano roll keyboard's click), injected straight into `node` of
    /// `graph`: no live-input routing, no recording. A stale `graph` (rebuilt since the GUI
    /// posted it) is dropped by the render thread.
    Audition {
        graph: GraphId,
        node: NodeIndex,
        event: InstrumentEvent,
    },
}

/// The sequence a start plays.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Cue {
    pub(crate) generation: Generation,
    /// The first source frame.
    pub(crate) start: i64,
    /// The source frame where a play range ends (no loop).
    pub(crate) end: Option<i64>,
    pub(crate) looped: Option<LoopRange>,
    /// Whether the metronome clicks whatever the project says (a recording's count-in).
    pub(crate) force_clicks: bool,
    /// Whether live MIDI input is recorded with its frames (a recording with MIDI takes).
    pub(crate) record_midi: bool,
}

/// What the render thread needs of a running stream: the ring it writes to, the stream's
/// clicks and its lookahead targets, and for recorded MIDI the device position and latency.
pub(crate) struct StreamPart {
    pub(crate) producer: BlockProducer,
    /// Read for the software position and the latency estimate only: atomic loads.
    pub(crate) control: StreamControl,
    /// The stream's sample rate.
    pub(crate) sample_rate: u32,
    pub(crate) clicks: ClickTables,
    /// Blocks to keep queued: `max(2, ceil(lookahead_ms · 0.001 · sr / 512))`.
    pub(crate) target_blocks: u64,
    /// The same for the live lookahead, while live input is active.
    pub(crate) live_target_blocks: u64,
}

/// The most MIDI inputs the render thread reads at once.
pub const MAX_MIDI_INPUTS: usize = 16;

/// A MIDI input as the render thread holds it: its port index and its ring.
pub(crate) type MidiSlot = Option<(u8, MidiReceiver)>;

/// A change of the MIDI inputs. The render thread moves the inputs of `add` into free slots
/// and the inputs of the ports in `remove` into `removed` (index for index with its slots), so
/// nothing is allocated or dropped there; whatever it holds afterwards is dropped on the control
/// thread (RT-09).
pub(crate) struct MidiChange {
    pub(crate) add: [MidiSlot; MAX_MIDI_INPUTS],
    /// The ports to remove, one bit per port.
    pub(crate) remove: [u64; 4],
    pub(crate) removed: [MidiSlot; MAX_MIDI_INPUTS],
}

impl MidiChange {
    /// Setup: an empty change.
    pub(crate) fn new() -> Self {
        Self {
            add: std::array::from_fn(|_| None),
            remove: [0; 4],
            removed: std::array::from_fn(|_| None),
        }
    }

    /// Marks `port` for removal.
    pub(crate) fn remove_port(&mut self, port: u8) {
        if let Some(word) = self.remove.get_mut(usize::from(port / 64)) {
            *word |= 1 << (port % 64);
        }
    }

    /// Whether the port set `remove` (one bit per port) marks `port`.
    pub(crate) fn marks(remove: &[u64; 4], port: u8) -> bool {
        remove
            .get(usize::from(port / 64))
            .is_some_and(|word| word & (1 << (port % 64)) != 0)
    }
}

/// The tracks that take live input, as node indices of one graph (indices are valid for one
/// graph only).
#[derive(Debug, Default)]
pub(crate) struct LiveTargets {
    pub(crate) graph: Option<GraphId>,
    pub(crate) nodes: Vec<NodeIndex>,
}

/// Clip audio the prefetch thread read for one block of the sequence: one buffer per audio
/// node of the graphs with media key `key`, a block long.
#[derive(Debug)]
pub(crate) struct MediaBlock {
    pub(crate) key: MediaKey,
    pub(crate) generation: Generation,
    /// Its position in the generation's sequence.
    pub(crate) index: u64,
    /// The source frame.
    pub(crate) frame: i64,
    pub(crate) tracks: Vec<Vec<[f64; 2]>>,
}

/// The render thread's end of a media feed: the blocks of one generation's sequence, read
/// ahead for the graphs of one media key. Each consumed block goes back to be refilled.
pub(crate) struct MediaFeed {
    pub(crate) receiver: handoff::Receiver<MediaBlock>,
}

/// One node's meter after a played block, for the control thread's meter history.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MeterEntry {
    /// The block's number (blocks written since the player was created).
    pub(crate) block: u64,
    /// The block's timeline tag.
    pub(crate) tag: i64,
    /// Starts, seeks and loop wraps so far: the pass the block belongs to.
    pub(crate) pass: u32,
    pub(crate) graph: GraphId,
    pub(crate) node: NodeIndex,
    pub(crate) meter: NodeMeter,
}

/// A live MIDI message recorded for a MIDI take, with the timeline frame it was heard at
/// (`_poll_midi`'s `heard_now − round((now − t) · sr)`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct RecordedMidi {
    pub(crate) frame: i64,
    pub(crate) event: MidiEvent,
}

/// One device's report after a played block.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ReportEntry {
    pub(crate) block: u64,
    pub(crate) graph: GraphId,
    pub(crate) device: crate::DeviceIndex,
    pub(crate) report: Report,
}
