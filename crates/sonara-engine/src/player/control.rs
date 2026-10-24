//! The player's control side (`RealtimePlayer`'s public methods in the prototype's
//! `player.py`): not real-time code.
//!
//! [`Player`]'s calls run on the caller's thread under the player's lock, which no real-time
//! thread ever takes. They flush the stream and arm nothing themselves: the generation's flush
//! is an atomic store, and everything else reaches the render thread as a message in one
//! ordered hand-off. A message that does not fit waits in an outbox that the next call or the
//! `sonara-player` thread sends; no call waits for the render thread (RT-06).
//!
//! **Recording**: [`Player::record`] creates the take files, starts like a play from
//! the count-in with the clicks forced on, arms the capture and starts the take writer thread
//! (`super::takes`). [`Player::stop`] disarms the capture, lets the writer drain the ring a last
//! time and finish the files, and returns the [`Recorded`] takes with the alignment inputs of
//! `_finish_recording`; the transport service turns them into assets, clips and one "Record"
//! transaction. Stopping waits for the writer thread, never for a real-time thread.

use std::collections::{BTreeSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::model::project::Project;
use sonara_core::model::track::TrackKind;
use sonara_core::plugins::PluginRegistry;
use sonara_dsp::numpy::math::py_round;
use sonara_dsp::processor::Report;
use sonara_media::MediaProvider;
use sonara_rt::{
    CaptureReader, Generation, MidiEvent, MidiReceiver, StatsSnapshot, StreamControl, handoff,
    queue,
};

use super::clicks::ClickTables;
use super::frames::{BLOCK, LoopRange, frame_at};
use super::message::{
    Cue, LiveTargets, MediaFeed, Message, MeterEntry, MidiChange, RecordedMidi, ReportEntry,
    StreamPart,
};
use super::prefetch::{FeedStart, PREFETCH_BLOCKS, Prefetch};
use super::render::RenderLoop;
use super::shared::{RenderCounters, Shared};
use super::takes::{OpenTakes, RecordedTake, TakeTarget, TakeWriter, WriterStats};
use super::threads::{PrefetchWorker, RenderThreadStart, Threads};
use super::{AudioDevice, PlayerSettings, ProjectSnapshot, ProjectSource, StreamRequest};
use crate::graph::{DeviceIndex, GraphId, GraphLayout, NodeIndex};
use crate::update::{SetupTicket, Update};
use crate::{DEFAULT_BLOCK, EngineError, GraphBuilder, NodeMeter};

/// Messages in flight to the render thread.
const MESSAGES: usize = 64;
/// Setup requests in flight to the builder thread.
const TICKETS: usize = 64;
/// Meter and report entries in flight to the control thread.
const METER_ENTRIES: usize = 8192;
/// Blocks of meters the history keeps (the prototype's `deque(maxlen=512)`).
const METER_HISTORY: usize = 512;
/// Layouts kept for mapping meters and reports of recent graphs to IDs.
const LAYOUTS: usize = 8;
/// Recorded MIDI messages in flight to the control thread (the maintenance collects them every
/// few milliseconds).
const RECORDED_MIDI: usize = 4096;
/// Seconds of input the capture ring holds (the prototype allocated 600 s). The take
/// writer drains it every few milliseconds; the size covers its longest measured stall with a
/// wide margin (see the spec's step 8 status).
pub const CAPTURE_SECONDS: usize = 4;

/// The transport state (`PlayerState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    /// Stopped.
    Stopped,
    /// Playing.
    Playing,
    /// Paused.
    Paused,
    /// Recording.
    Recording,
}

impl PlayerState {
    /// The prototype's name: `"stopped"`, `"playing"`, `"paused"` or `"recording"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Playing => "playing",
            Self::Paused => "paused",
            Self::Recording => "recording",
        }
    }

    fn is_rolling(self) -> bool {
        matches!(self, Self::Playing | Self::Recording)
    }
}

/// A transport position as the prototype's position JSON gives it.
#[derive(Debug, Clone, PartialEq)]
pub enum Position {
    /// With a project: `position_beat` and `position_seconds` rounded to 4 decimals, and
    /// `bar_beat` as `"<bar>:<beat>"`.
    Musical {
        /// The frame (not in the JSON).
        frame: i64,
        /// `round(beat, 4)`.
        beat: f64,
        /// `round(frame / sr, 4)`.
        seconds: f64,
        /// The 1-based bar.
        bar: i64,
        /// The 1-based beat in the bar.
        beat_in_bar: i64,
    },
    /// Without a project: `position_frame`.
    Frame(i64),
}

impl Position {
    /// The position of `frame` (negative frames count as 0), with `project`'s tempo and
    /// time signatures.
    #[must_use]
    pub fn of(frame: i64, project: Option<&Project>) -> Self {
        let frame = frame.max(0);
        let Some(project) = project else {
            return Self::Frame(frame);
        };
        let rate = project.settings.sample_rate;
        let seconds = frame_f64(frame) / f64::from(rate);
        let beat = project.tempo_map.sample_to_beat(frame_f64(frame), rate);
        let at = project.time_signatures.position(beat);
        Self::Musical {
            frame,
            beat: py_round(beat, 4),
            seconds: py_round(seconds, 4),
            bar: at.bar,
            beat_in_bar: at.beat,
        }
    }

    /// The frame.
    #[must_use]
    pub fn frame(&self) -> i64 {
        match self {
            Self::Musical { frame, .. } | Self::Frame(frame) => *frame,
        }
    }
}

/// `status()`: every key of the prototype's status JSON, rounded as it rounds them.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerStatus {
    /// `state`.
    pub state: PlayerState,
    /// The position keys.
    pub position: Position,
    /// `device_sample_rate`: 0 without a running device.
    pub device_sample_rate: u32,
    /// `buffer_frames`: the device period.
    pub buffer_frames: u32,
    /// `queued_ms`: `round(1000 · queued / sr, 1)`.
    pub queued_ms: f64,
    /// `lookahead_ms` from the settings.
    pub lookahead_ms: f64,
    /// `output_latency_estimate_ms`: `round(1000 · latency_frames / sr, 2)`; an estimate,
    /// never a position (RT-07).
    pub output_latency_estimate_ms: f64,
    /// `underruns`.
    pub underruns: u64,
    /// `callback_load`: the last callback's load, `round(…, 4)`.
    pub callback_load: f64,
    /// `render_load`: render time over the played time of the rendered blocks since the
    /// player was created, `round(…, 3)`; 0 before the first block.
    pub render_load: f64,
    /// `loop`: the loop of the playing sequence in frames.
    pub looped: Option<(i64, i64)>,
    /// `device_errors`.
    pub device_errors: u64,
    /// `last_error`: a failure of the player (a graph that could not be rebuilt, a clip that
    /// could not be read).
    pub last_error: Option<String>,
    /// `device_status`: `"device stopped"` when the device stopped by itself.
    pub device_status: Option<&'static str>,
}

/// What `stop()` answers.
#[derive(Debug, Clone, PartialEq)]
pub struct Stopped {
    /// `was_playing`: playing, paused or recording before the stop.
    pub was_playing: bool,
    /// The position keys; `None` when the transport was stopped already.
    pub position: Option<Position>,
    /// The recording the stop finished, if one was running.
    pub recording: Option<Recorded>,
}

/// What [`Player::record`] records: the prototype's `record(start_beat, track_ids)` once the
/// transport service has chosen the tracks and named the take files.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordPlan {
    /// Where the takes start; the resting position when `None`. With count-in bars, playback
    /// starts that many bars (of the time signature there) earlier.
    pub start_beat: Option<f64>,
    /// The audio takes, one per armed audio track.
    pub takes: Vec<TakeTarget>,
    /// The instrument tracks that get the MIDI take, in track order.
    pub midi_tracks: Vec<String>,
}

/// A finished recording: the takes and everything `_finish_recording` aligns them with.
#[derive(Debug, Clone, PartialEq)]
pub struct Recorded {
    /// The record position in frames (`record_from_frame`).
    pub record_from: i64,
    /// `max(record_from + 1, stop position − output latency)`: where the MIDI take ends.
    pub stop_frame: i64,
    /// The timeline frame of the audio take's first frame; `None` when nothing was
    /// captured.
    pub capture_start: Option<i64>,
    /// The output latency estimate in frames (`latency_frames`).
    pub latency_frames: u32,
    /// The input latency estimate in frames (`input_latency_frames`).
    pub input_latency_frames: u32,
    /// Capture overruns of the stream (`stats.overruns`).
    pub capture_overruns: u64,
    /// The audio takes, their files finished.
    pub takes: Vec<RecordedTake>,
    /// The instrument tracks of the MIDI take.
    pub midi_tracks: Vec<String>,
    /// The live MIDI messages recorded, with the timeline frame each was heard at.
    pub midi_events: Vec<(i64, MidiEvent)>,
    /// How the take writer kept up.
    pub writer: WriterStats,
}

/// A recording while it runs.
struct ActiveRecording {
    record_from: i64,
    midi_tracks: Vec<String>,
    midi_events: Vec<(i64, MidiEvent)>,
    /// The take writer; `None` without audio takes.
    writer: Option<TakeWriter>,
    /// The take files of a recording whose writer is not running.
    files: Vec<PathBuf>,
    /// The stream the takes capture from.
    stream: u64,
}

/// What `pause()` answers.
#[derive(Debug, Clone, PartialEq)]
pub struct Paused {
    /// `paused`: false unless the transport was playing.
    pub paused: bool,
    /// The state when it did not pause (`state`).
    pub state: PlayerState,
    /// The position keys when it paused.
    pub position: Option<Position>,
}

/// One track's post-fader meter (`meters_at`).
#[derive(Debug, Clone, PartialEq)]
pub struct TrackMeter {
    /// The track (or `master`).
    pub track_id: String,
    /// `peak`: left and right.
    pub peak: [f64; 2],
    /// `rms`: left and right.
    pub rms: [f64; 2],
}

/// One device's live readouts (`device_reports`).
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceReport {
    /// The device.
    pub device_id: String,
    /// The readouts by name.
    pub values: Vec<(&'static str, f64)>,
}

/// The running stream as the control side keeps it.
struct StreamInfo {
    control: StreamControl,
    sample_rate: u32,
    inputs: usize,
    target_blocks: u64,
    /// The stream's capture reader; the take writer holds it while a recording runs.
    capture: Option<CaptureReader>,
    /// Identifies the stream among the player's streams.
    serial: u64,
}

/// The last graph sent to the render thread.
struct LiveGraph {
    id: GraphId,
    version: u64,
    project: Project,
    media: Arc<MediaProvider>,
}

/// A graph message on its way to the render thread and back.
#[derive(Debug, Clone, Copy)]
enum Flight {
    /// A graph built from the project at `version`.
    Graph { id: GraphId, version: u64 },
    /// A recall of the playing graph.
    Recall,
}

/// A play that waits for the recalled graph.
struct PendingStart {
    snapshot: ProjectSnapshot,
    cue: Cue,
}

/// One block of the meter history.
struct MeterBlock {
    block: u64,
    tag: i64,
    pass: u32,
    graph: GraphId,
    nodes: Vec<(NodeIndex, NodeMeter)>,
}

/// The device reports of the most recently rendered block.
struct LatestReports {
    block: u64,
    graph: GraphId,
    reports: Vec<(DeviceIndex, Report)>,
}

/// The player's control state.
pub(crate) struct Core {
    registry: Arc<PluginRegistry>,
    device: Box<dyn AudioDevice>,
    settings: PlayerSettings,
    builder: GraphBuilder,
    sender: handoff::Sender<Message>,
    outbox: VecDeque<Message>,
    tickets: queue::Receiver<SetupTicket>,
    meter_entries: queue::Receiver<MeterEntry>,
    report_entries: queue::Receiver<ReportEntry>,
    recorded_midi: queue::Receiver<RecordedMidi>,
    prefetch: Arc<Mutex<Prefetch>>,
    shared: Arc<Shared>,
    stream: Option<StreamInfo>,
    /// Streams opened so far.
    streams: u64,
    recording: Option<ActiveRecording>,
    state: PlayerState,
    generation: Generation,
    /// The resting position when stopped or paused (`_position_frame`).
    position: i64,
    /// The playing sequence.
    cue: Option<Cue>,
    graph: Option<LiveGraph>,
    /// The graph messages posted and not yet back, in order.
    in_flight: VecDeque<Flight>,
    /// A play that waits for its graph to come back from the render thread.
    starting: Option<PendingStart>,
    /// The project version of the graph the render thread plays.
    playing_version: Option<u64>,
    /// A graph came back rejected: the next one is built fresh.
    needs_fresh: bool,
    /// A version whose graph could not be built; it is not tried again.
    failed_version: Option<u64>,
    focus: Option<String>,
    /// The ports of the MIDI inputs on the render thread: live input plays on a stopped
    /// transport while there are any.
    midi_ports: BTreeSet<u8>,
    meters: VecDeque<MeterBlock>,
    reports: Option<LatestReports>,
    layouts: VecDeque<GraphLayout>,
    last_error: Option<String>,
}

/// What [`Player::new`] and [`Player::manual`] need.
pub struct PlayerSetup {
    /// The plugins the graphs are built with.
    pub registry: Arc<PluginRegistry>,
    /// The device layer.
    pub device: Box<dyn AudioDevice>,
    /// Where the `sonara-player` thread looks for edits.
    pub source: Arc<dyn ProjectSource>,
    /// The audio settings.
    pub settings: PlayerSettings,
}

/// The real-time player's control handle (see the [module docs](super)). Dropping it stops
/// its threads, drops the render thread's state off the render thread (RT-09) and stops the
/// device.
pub struct Player {
    core: Arc<Mutex<Core>>,
    prefetch: Arc<Mutex<Prefetch>>,
    shared: Arc<Shared>,
    source: Arc<dyn ProjectSource>,
    threads: Option<Threads>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic under the lock leaves the state of a finished call: take it over.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "frame counts stay far below 2^53, where the conversion is exact"
)]
fn frame_f64(frame: i64) -> f64 {
    frame as f64
}

#[expect(
    clippy::cast_precision_loss,
    reason = "counts stay far below 2^53, where the conversion is exact"
)]
fn count_f64(count: u64) -> f64 {
    count as f64
}

/// `max(2, ceil(lookahead_ms · 0.001 · sr / 512))`.
fn target_blocks(lookahead_ms: f64, sample_rate: u32) -> u64 {
    let blocks = (lookahead_ms * 0.001 * f64::from(sample_rate) / 512.0).ceil();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a small positive block count; negative lookaheads give the minimum"
    )]
    let blocks = blocks.max(0.0) as u64;
    blocks.max(2)
}

/// `max(4, ceil(max(2048, int(lookahead_ms · 0.001 · sr · 2)) / 512))`.
fn ring_blocks(lookahead_ms: f64, sample_rate: u32) -> usize {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Python's int() of a non-negative frame count"
    )]
    let frames = (lookahead_ms * 0.001 * f64::from(sample_rate) * 2.0).max(0.0) as usize;
    frames.max(4 * BLOCK).div_ceil(BLOCK).max(4)
}

impl Player {
    /// A player with its three threads running (see the [module docs](super)).
    ///
    /// # Errors
    ///
    /// `INTERNAL` when the system cannot start a thread.
    pub fn new(setup: PlayerSetup) -> Result<Self, EngineError> {
        Self::with_render_start(setup, None)
    }

    /// As [`Player::new`], and the `sonara-render` thread runs `render_start`, if any, once on
    /// itself before its loop: setup such as raising its scheduling class. The hook runs
    /// outside the real-time loop ([`RenderThreadStart`]).
    ///
    /// # Errors
    ///
    /// `INTERNAL` when the system cannot start a thread.
    pub fn with_render_start(
        setup: PlayerSetup,
        render_start: Option<RenderThreadStart>,
    ) -> Result<Self, EngineError> {
        let (mut player, render) = Self::parts(setup);
        let threads = Threads::spawn(
            render,
            Arc::clone(&player.core),
            Arc::clone(&player.prefetch),
            Arc::clone(&player.source),
            render_start,
        )
        .map_err(|error| {
            DomainError::new(
                ErrorCode::Internal,
                format!("cannot start the player's threads: {error}"),
            )
        })?;
        player.threads = Some(threads);
        Ok(player)
    }

    /// A player without threads, and the parts its threads would run: a test drives the render
    /// thread ([`RenderLoop::step`]), the prefetch thread ([`PrefetchWorker::fill`]) and the
    /// maintenance ([`Player::service`]) by hand, deterministically. There is no render thread,
    /// so there is no [`RenderThreadStart`] hook either.
    #[must_use]
    pub fn manual(setup: PlayerSetup) -> (Self, RenderLoop, PrefetchWorker) {
        let (player, render) = Self::parts(setup);
        let prefetch = PrefetchWorker::new(Arc::clone(&player.prefetch));
        (player, render, prefetch)
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    fn parts(setup: PlayerSetup) -> (Self, RenderLoop) {
        let (sender, receiver) = handoff::channel(MESSAGES);
        let (ticket_in, tickets) = queue::channel(TICKETS);
        let (meter_in, meter_entries) = queue::channel(METER_ENTRIES);
        let (report_in, report_entries) = queue::channel(METER_ENTRIES);
        let (recorded_in, recorded_midi) = queue::channel(RECORDED_MIDI);
        let shared = Arc::new(Shared::default());
        let prefetch = Arc::new(Mutex::new(Prefetch::default()));
        let render = RenderLoop::new(
            receiver,
            ticket_in,
            meter_in,
            report_in,
            recorded_in,
            Arc::clone(&shared),
        );
        let core = Core {
            registry: setup.registry,
            device: setup.device,
            settings: setup.settings,
            builder: GraphBuilder::new(DEFAULT_BLOCK),
            sender,
            outbox: VecDeque::new(),
            tickets,
            meter_entries,
            report_entries,
            recorded_midi,
            prefetch: Arc::clone(&prefetch),
            shared: Arc::clone(&shared),
            stream: None,
            streams: 0,
            recording: None,
            state: PlayerState::Stopped,
            generation: Generation::new(1),
            position: 0,
            cue: None,
            graph: None,
            in_flight: VecDeque::new(),
            starting: None,
            playing_version: None,
            needs_fresh: false,
            failed_version: None,
            focus: None,
            midi_ports: BTreeSet::new(),
            meters: VecDeque::new(),
            reports: None,
            layouts: VecDeque::new(),
            last_error: None,
        };
        let player = Self {
            core: Arc::new(Mutex::new(core)),
            prefetch,
            shared,
            source: setup.source,
            threads: None,
        };
        (player, render)
    }

    /// `play(start_beat, end_beat, loop)`: plays from `start_beat` (the resting position when
    /// `None`) to `end_beat`, or loops `[start_beat or the project's loop start, end_beat or its
    /// loop end)`. Opens the stream when needed, flushes, resets the player's graph (its first
    /// play builds one) and starts the render thread on the new generation. Answers the status
    /// with the new position.
    ///
    /// # Errors
    ///
    /// `DEVICE_ERROR` ("cannot open the audio device: …") when the stream cannot be opened;
    /// `INVALID_ARGUMENT` ("loop range is too short") for a loop of 512 frames or less, after
    /// the stream was opened, as in the prototype; the errors of building the graph.
    pub fn play(
        &self,
        snapshot: &ProjectSnapshot,
        start_beat: Option<f64>,
        end_beat: Option<f64>,
        looped: bool,
    ) -> Result<PlayerStatus, EngineError> {
        let mut core = lock(&self.core);
        let project = &snapshot.project;
        let inputs = core.stream.as_ref().map_or(0, |s| s.inputs);
        core.ensure_stream(project, inputs)?;
        let mut start = start_beat.map_or(core.position, |beat| project.beat_to_frame(beat));
        let mut range = None;
        if looped {
            let transport = &project.transport;
            let loop_start = if start_beat.is_some() {
                start
            } else {
                project.beat_to_frame(transport.loop_start)
            };
            let loop_end = project.beat_to_frame(end_beat.unwrap_or(transport.loop_end));
            let Some(found) = LoopRange::new(loop_start, loop_end) else {
                return Err(DomainError::invalid("loop range is too short").into());
            };
            if !(found.start() <= start && start < found.end()) {
                start = found.start();
            }
            range = Some(found);
        }
        let end = if looped {
            None
        } else {
            end_beat.map(|beat| project.beat_to_frame(beat))
        };
        // A recording keeps capturing through a play (the prototype's `_recording` survives
        // it), MIDI included.
        let record_midi = core.records_midi();
        core.start(snapshot, start, end, range, false, record_midi)?;
        core.state = PlayerState::Playing;
        core.flush_outbox();
        let mut status = core.status(Some(project));
        status.position = Position::of(core.position, Some(project));
        Ok(status)
    }

    /// `stop()`: stops playback and finishes a running recording (`_finish_recording`'s
    /// capture part): the capture is disarmed, the take writer drains the ring a last time and
    /// finishes the files, and the takes come back in [`Stopped::recording`]. The position
    /// becomes the device's software position.
    ///
    /// A recording that a play took over (the prototype's `_recording` survives a play) is
    /// finished by the next stop too, even when the play has stopped by itself; the prototype
    /// never finished it and left its files open.
    #[must_use = "the answer carries the position and the takes"]
    pub fn stop(&self, project: Option<&Project>) -> Stopped {
        let mut core = lock(&self.core);
        core.reconcile();
        if core.state == PlayerState::Stopped {
            let recording = core
                .recording
                .take()
                .map(|active| core.end_recording(active));
            return Stopped {
                was_playing: false,
                position: None,
                recording,
            };
        }
        let position = core.current_frame();
        core.state = PlayerState::Stopped;
        core.halt(position);
        core.flush_outbox();
        let recording = core
            .recording
            .take()
            .map(|active| core.end_recording(active));
        Stopped {
            was_playing: true,
            position: Some(Position::of(core.position, project)),
            recording,
        }
    }

    /// Opens the stream at `project`'s rate with `inputs` input channels, or with the current
    /// stream's inputs when `None` (`_ensure_stream`), unless one runs at that rate with at least
    /// as many. Returns the stream's input channels.
    ///
    /// # Errors
    ///
    /// `DEVICE_ERROR` ("cannot open the audio device: …") when the stream cannot be opened.
    pub fn open_stream(
        &self,
        project: &Project,
        inputs: Option<usize>,
    ) -> Result<usize, EngineError> {
        let mut core = lock(&self.core);
        let inputs = inputs.unwrap_or_else(|| core.stream.as_ref().map_or(0, |s| s.inputs));
        let opened = core.ensure_stream(project, inputs);
        core.flush_outbox();
        opened?;
        Ok(core.stream.as_ref().map_or(0, |s| s.inputs))
    }

    /// `record(start_beat, track_ids)` after the service chose the tracks: records the takes of
    /// `plan` from its start beat (the resting position when `None`). With count-in bars,
    /// playback starts that many bars earlier, in the time signature at the record position.
    /// The take files are created, the transport starts like a play with the metronome forced
    /// on (the count-in clicks) and recording MIDI input when there are MIDI takes, and the
    /// capture is armed for the audio takes, which the take writer thread writes. Answers the
    /// position the transport starts at (as `play`).
    ///
    /// The stream must have been opened with enough input channels ([`Player::open_stream`]);
    /// it is opened here with the takes' channels otherwise. A recording still running is
    /// finished first, its takes unused (the transport service stops it before).
    ///
    /// # Errors
    ///
    /// `DEVICE_ERROR` when the stream cannot be opened; `INTERNAL` when a take file cannot be
    /// created (the files created before it are removed) or the writer thread cannot start;
    /// the errors of building the graph (the take files are removed).
    pub fn record(
        &self,
        snapshot: &ProjectSnapshot,
        plan: RecordPlan,
    ) -> Result<Position, EngineError> {
        let mut core = lock(&self.core);
        core.reconcile();
        if let Some(active) = core.recording.take() {
            drop(core.end_recording(active));
        }
        let project = &snapshot.project;
        let needed = plan
            .takes
            .iter()
            .flat_map(|take| take.channels.iter().map(|c| c.saturating_add(1)))
            .max()
            .unwrap_or(0);
        let inputs = core.stream.as_ref().map_or(0, |s| s.inputs).max(needed);
        let opened = core.ensure_stream(project, inputs);
        core.flush_outbox();
        opened?;
        let rate = project.settings.sample_rate;
        let record_from = plan
            .start_beat
            .map_or(core.position, |beat| project.beat_to_frame(beat));
        let preroll = count_in_start(project, record_from);
        let files: Vec<PathBuf> = plan.takes.iter().map(|take| take.path.clone()).collect();
        let takes = if plan.takes.is_empty() {
            None
        } else {
            Some(OpenTakes::create(&plan.takes, rate).map_err(|error| {
                DomainError::new(
                    ErrorCode::Internal,
                    format!("cannot create the take file {error}"),
                )
            })?)
        };
        let record_midi = !plan.midi_tracks.is_empty();
        if let Err(error) = core.start(snapshot, preroll, None, None, true, record_midi) {
            if let Some(takes) = takes {
                remove_takes(takes);
            }
            core.flush_outbox();
            return Err(error);
        }
        let writer = match takes {
            Some(takes) => match core.capture_into(takes) {
                Ok(writer) => Some(writer),
                Err(error) => {
                    core.state = PlayerState::Stopped;
                    let position = core.position;
                    core.halt(position);
                    core.flush_outbox();
                    return Err(error);
                }
            },
            None => None,
        };
        core.recording = Some(ActiveRecording {
            record_from,
            midi_tracks: plan.midi_tracks,
            midi_events: Vec::new(),
            writer,
            files,
            stream: core.stream.as_ref().map_or(0, |s| s.serial),
        });
        core.state = PlayerState::Recording;
        core.flush_outbox();
        Ok(Position::of(core.position, Some(project)))
    }

    /// Whether a recording runs: from [`Player::record`] until the stop that finishes it.
    #[must_use]
    pub fn is_recording(&self) -> bool {
        lock(&self.core).recording.is_some()
    }

    /// `pause()`: pauses playback at the device's software position; only while playing.
    #[must_use = "the answer says whether the transport paused"]
    pub fn pause(&self, project: Option<&Project>) -> Paused {
        let mut core = lock(&self.core);
        core.reconcile();
        if core.state != PlayerState::Playing {
            return Paused {
                paused: false,
                state: core.state,
                position: None,
            };
        }
        let position = core.current_frame();
        core.state = PlayerState::Paused;
        core.halt(position);
        core.flush_outbox();
        Paused {
            paused: true,
            state: PlayerState::Paused,
            position: Some(Position::of(core.position, project)),
        }
    }

    /// `seek(beat)`: moves the resting position; while playing, the render thread continues
    /// from it in a new generation, without a reset (the next block chases). Answers the new
    /// position.
    #[must_use = "the answer carries the new position"]
    pub fn seek(&self, project: &Project, beat: f64) -> Position {
        let mut core = lock(&self.core);
        core.reconcile();
        let frame = project.beat_to_frame(beat).max(0);
        core.position = frame;
        if core.state.is_rolling()
            && let Some(cue) = core.cue
        {
            let generation = core.generation.next();
            core.generation = generation;
            core.flush_stream();
            let cue = Cue {
                generation,
                start: frame,
                ..cue
            };
            core.cue = Some(cue);
            if let Some(pending) = core.starting.as_mut() {
                // The play has not started yet: it starts here.
                pending.cue = cue;
            } else {
                let feed = core.feed(FeedStart {
                    generation,
                    index: 0,
                    frame,
                    looped: cue.looped,
                    end: cue.end,
                });
                core.post(Message::Seek {
                    generation,
                    frame,
                    feed,
                });
            }
        } else {
            core.post(Message::Rest(frame));
        }
        core.flush_outbox();
        Position::of(frame, Some(project))
    }

    /// `status()`.
    #[must_use]
    pub fn status(&self, project: Option<&Project>) -> PlayerStatus {
        let mut core = lock(&self.core);
        core.reconcile();
        core.status(project)
    }

    /// The transport state.
    #[must_use]
    pub fn state(&self) -> PlayerState {
        let mut core = lock(&self.core);
        core.reconcile();
        core.state
    }

    /// `current_frame()`: the resting position when stopped or paused, otherwise the device's
    /// software position (RT-07: not the hardware position).
    #[must_use]
    pub fn current_frame(&self) -> i64 {
        let mut core = lock(&self.core);
        core.reconcile();
        core.current_frame()
    }

    /// `position_beats()`: [`Player::current_frame`] in beats; 0 without a project.
    #[must_use]
    pub fn position_beats(&self, project: Option<&Project>) -> f64 {
        let frame = self.current_frame().max(0);
        project.map_or(0.0, |p| {
            p.tempo_map
                .sample_to_beat(frame_f64(frame), p.settings.sample_rate)
        })
    }

    /// Adds MIDI inputs (the hub's rings, `sonara_app::midi::MidiHub::take_inputs`) by their
    /// port index: the render thread drains them every step (`_poll_midi`). With a project, the
    /// stream is opened and a graph built if none plays, so live input sounds on a stopped
    /// transport (the prototype's idle step; [`Player::prepare_live`]). At most
    /// [`MAX_MIDI_INPUTS`](super::MAX_MIDI_INPUTS) inputs are read; an input beyond them, or
    /// for a port that is open already, comes back and is dropped here.
    ///
    /// # Errors
    ///
    /// The errors of opening the stream and of building the graph. The inputs are added
    /// anyway (they are counted and recorded), and a later [`Player::prepare_live`] can retry.
    pub fn add_midi_inputs(
        &self,
        inputs: Vec<(u8, MidiReceiver)>,
        snapshot: Option<&ProjectSnapshot>,
    ) -> Result<(), EngineError> {
        let mut core = lock(&self.core);
        let prepared = match snapshot {
            Some(snapshot) if !inputs.is_empty() => core.prepare_live(snapshot),
            _ => Ok(()),
        };
        let mut inputs = inputs.into_iter().peekable();
        while inputs.peek().is_some() {
            let mut change = Box::new(MidiChange::new());
            for (slot, (port, input)) in change.add.iter_mut().zip(inputs.by_ref()) {
                if core.midi_ports.insert(port) {
                    *slot = Some((port, input));
                }
            }
            core.post(Message::Midi(change));
        }
        core.flush_outbox();
        prepared
    }

    /// Opens the stream and builds a graph when MIDI inputs are open and none plays, so live
    /// input sounds on a stopped transport (the prototype's idle step opened the stream
    /// itself). Does nothing without inputs.
    ///
    /// # Errors
    ///
    /// The errors of opening the stream and of building the graph.
    pub fn prepare_live(&self, snapshot: &ProjectSnapshot) -> Result<(), EngineError> {
        let mut core = lock(&self.core);
        if core.midi_ports.is_empty() {
            return Ok(());
        }
        let prepared = core.prepare_live(snapshot);
        core.flush_outbox();
        prepared
    }

    /// Removes the MIDI inputs of `ports` (closed by the hub): their rings come back from the
    /// render thread and are dropped here (RT-09). Without inputs, live input stops.
    pub fn remove_midi_inputs(&self, ports: &[u8]) {
        let mut core = lock(&self.core);
        let mut change = Box::new(MidiChange::new());
        for &port in ports {
            core.midi_ports.remove(&port);
            change.remove_port(port);
        }
        core.post(Message::Midi(change));
        core.flush_outbox();
    }

    /// The ports of the MIDI inputs added and not removed.
    #[must_use]
    pub fn midi_ports(&self) -> Vec<u8> {
        lock(&self.core).midi_ports.iter().copied().collect()
    }

    /// `set_midi_focus`: the instrument track that takes live input besides the armed ones.
    pub fn set_midi_focus(&self, track_id: Option<String>) {
        let mut core = lock(&self.core);
        core.focus = track_id;
        if let Some(targets) = core.current_targets() {
            core.post(Message::Targets(targets));
        }
        core.flush_outbox();
    }

    /// `_live_targets`: the armed instrument tracks and the focused one, in track order.
    #[must_use]
    pub fn midi_targets(&self, project: &Project) -> Vec<String> {
        let core = lock(&self.core);
        target_tracks(project, core.focus.as_deref())
            .map(str::to_owned)
            .collect()
    }

    /// `messages_received`: MIDI messages received while there were targets.
    #[must_use]
    pub fn messages_received(&self) -> u64 {
        self.shared.messages_received()
    }

    /// `meters_at(frame)`: each track's post-fader meter of the block heard at `frame`: the
    /// newest block with `tag ≤ frame` among those of the current pass, since the last loop
    /// wrap, seek or start; none before.
    #[must_use]
    pub fn meters_at(&self, frame: i64) -> Vec<TrackMeter> {
        let mut core = lock(&self.core);
        core.collect();
        core.meters_at(frame)
    }

    /// `meters()`: [`Player::meters_at`] the current frame.
    #[must_use]
    pub fn meters(&self) -> Vec<TrackMeter> {
        let frame = self.current_frame();
        self.meters_at(frame)
    }

    /// `device_reports()`: the processors' readouts after the most recently rendered block.
    #[must_use]
    pub fn device_reports(&self) -> Vec<DeviceReport> {
        let mut core = lock(&self.core);
        core.collect();
        core.device_reports()
    }

    /// `output_peaks()`: the device's output peaks since the previous call that read them
    /// (the stream's `stats()`, which resets them); empty without a stream.
    #[must_use]
    pub fn output_peaks(&self) -> Vec<f32> {
        let core = lock(&self.core);
        core.stream
            .as_ref()
            .map(|s| s.control.stats().peak.as_slice().to_vec())
            .unwrap_or_default()
    }

    /// The project version of the graph the render thread plays; `None` before the first.
    #[must_use]
    pub fn graph_version(&self) -> Option<u64> {
        let mut core = lock(&self.core);
        core.take_returned();
        core.playing_version
    }

    /// The render thread's counters (RT-08).
    #[must_use]
    pub fn counters(&self) -> RenderCounters {
        self.shared.counters()
    }

    /// Clip reads that failed on the prefetch thread.
    #[must_use]
    pub fn media_errors(&self) -> u64 {
        lock(&self.prefetch).errors().0
    }

    /// The maintenance the `sonara-player` thread runs every few milliseconds: takes back and
    /// drops what the render thread returned (RT-09), answers setup requests, rebuilds the graph
    /// when the project changed, notices the end of a play range, collects meters and reports,
    /// and sends waiting messages. A manual player's test calls it.
    pub fn service(&self) {
        service(&self.core, &*self.source);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        // Joins the threads; the render thread's state is dropped here (RT-09).
        drop(self.threads.take());
        lock(&self.core).device.stop();
    }
}

/// One round of maintenance (see [`Player::service`]).
pub(crate) fn service(core: &Mutex<Core>, source: &dyn ProjectSource) {
    // Asked without the player's lock (see the module docs of `player`).
    let snapshot = source.snapshot();
    let mut core = lock(core);
    core.take_returned();
    core.answer_tickets();
    if let Some(snapshot) = snapshot {
        core.follow(&snapshot);
    }
    core.reconcile();
    core.collect();
    core.flush_outbox();
}

/// The first frame a recording from `record_from` plays: `count_in_bars` bars earlier, in the
/// time signature at the record position, not before 0 (`record`'s pre-roll).
fn count_in_start(project: &Project, record_from: i64) -> i64 {
    let count_in = project.transport.count_in_bars;
    if count_in == 0 {
        return record_from;
    }
    let rate = project.settings.sample_rate;
    let beat = project
        .tempo_map
        .sample_to_beat(frame_f64(record_from), rate);
    let bar = project.time_signatures.at_beat(beat).beats_per_bar();
    project.beat_to_frame((beat - f64::from(count_in) * bar).max(0.0))
}

/// Finishes and removes take files that will not be recorded into.
fn remove_takes(takes: OpenTakes) {
    for take in takes.finish() {
        // Best effort: the recording is being rejected with its own error.
        drop(std::fs::remove_file(&take.path));
    }
}

/// The instrument tracks of `project` that take live input: armed, or the focused one.
fn target_tracks<'a>(
    project: &'a Project,
    focus: Option<&'a str>,
) -> impl Iterator<Item = &'a str> {
    project
        .tracks
        .iter()
        .filter(move |t| {
            t.kind == TrackKind::Instrument && (t.input.armed || focus == Some(t.id.as_str()))
        })
        .map(|t| t.id.as_str())
}

impl Core {
    /// Opens the stream at `project`'s rate when none runs at that rate with at least `inputs`
    /// input channels (`_ensure_stream`). The click tables are computed here.
    fn ensure_stream(&mut self, project: &Project, inputs: usize) -> Result<(), EngineError> {
        let rate = project.settings.sample_rate;
        if self
            .stream
            .as_ref()
            .is_some_and(|s| s.sample_rate == rate && s.inputs >= inputs)
        {
            return Ok(());
        }
        if self.stream.take().is_some() {
            self.device.stop();
        }
        let request = StreamRequest {
            sample_rate: rate,
            buffer_frames: self.settings.buffer_frames,
            output_channels: 2,
            input_channels: inputs,
            ring_blocks: ring_blocks(self.settings.lookahead_ms, rate),
            // Step 8 sizes the capture ring from a measurement.
            capture_frames: if inputs > 0 {
                usize::try_from(rate)
                    .unwrap_or(usize::MAX)
                    .saturating_mul(CAPTURE_SECONDS)
            } else {
                0
            },
            output_device: self.settings.output_device.clone(),
            input_device: self.settings.input_device.clone(),
        };
        let stream = self.device.start(&request).map_err(|error| {
            DomainError::new(
                ErrorCode::DeviceError,
                format!("cannot open the audio device: {error}"),
            )
        })?;
        let target = target_blocks(self.settings.lookahead_ms, rate);
        let part = StreamPart {
            producer: stream.producer,
            control: stream.control.clone(),
            sample_rate: stream.sample_rate,
            clicks: ClickTables::new(rate),
            target_blocks: target,
            live_target_blocks: target_blocks(self.settings.live_lookahead_ms, rate),
        };
        self.post(Message::Stream(Some(Box::new(part))));
        self.streams += 1;
        self.stream = Some(StreamInfo {
            control: stream.control,
            sample_rate: stream.sample_rate,
            inputs,
            target_blocks: target,
            capture: Some(stream.capture),
            serial: self.streams,
        });
        Ok(())
    }

    /// Flushes the stream to the current generation: stale blocks fade out, and `playing`
    /// stays disarmed until the generation's first block is written.
    fn flush_stream(&self) {
        if let Some(stream) = &self.stream {
            stream.control.flush(self.generation);
        }
    }

    /// `_start_locked`: a new generation from `start`, flushed, with the player's graph reset.
    ///
    /// The prototype resets the graph it has (`graph.reset()`), so state that survives a
    /// processor's reset survives a play: the synth's note counter, which seeds the phases of
    /// its voices, keeps counting (the golden `play-position` resume shows it). A reset may
    /// allocate, so the render thread hands the graph back ([`Message::Recall`]) and the start
    /// completes when it arrives ([`Core::complete_start`]); this call does not wait for it
    /// (RT-06). Without a graph, as on the first play, a fresh one starts at once.
    fn start(
        &mut self,
        snapshot: &ProjectSnapshot,
        start: i64,
        end: Option<i64>,
        looped: Option<LoopRange>,
        force_clicks: bool,
        record_midi: bool,
    ) -> Result<(), EngineError> {
        self.generation = self.generation.next();
        self.flush_stream();
        self.shared
            .set_metronome(snapshot.project.transport.metronome);
        let src = start.max(0);
        let cue = Cue {
            generation: self.generation,
            start: src,
            end,
            looped,
            force_clicks,
            record_midi,
        };
        self.position = src;
        self.cue = Some(cue);
        self.meters.clear();
        if self.graph.is_some() {
            if self.starting.is_none() {
                self.in_flight.push_back(Flight::Recall);
                self.post(Message::Recall(Update::Graph(None)));
            }
            self.starting = Some(PendingStart {
                snapshot: snapshot.clone(),
                cue,
            });
            return Ok(());
        }
        match self.build(snapshot, true) {
            Ok((graph, _)) => {
                self.launch(graph, snapshot, cue);
                Ok(())
            }
            Err(error) => {
                self.fail_start();
                Err(error)
            }
        }
    }

    /// Whether a running recording records MIDI input.
    fn records_midi(&self) -> bool {
        self.recording
            .as_ref()
            .is_some_and(|recording| !recording.midi_tracks.is_empty())
    }

    /// The stream and a graph for live input on a stopped transport.
    fn prepare_live(&mut self, snapshot: &ProjectSnapshot) -> Result<(), EngineError> {
        let channels = self.stream.as_ref().map_or(0, |s| s.inputs);
        self.ensure_stream(&snapshot.project, channels)?;
        if self.graph.is_none() {
            self.install_graph(snapshot, true)?;
        }
        Ok(())
    }

    /// Arms the capture for a new take and starts the writer thread on the stream's capture
    /// reader.
    fn capture_into(&mut self, takes: OpenTakes) -> Result<TakeWriter, EngineError> {
        let internal = |message: String| DomainError::new(ErrorCode::Internal, message);
        let Some(stream) = self.stream.as_mut() else {
            remove_takes(takes);
            return Err(internal("no stream to record from".to_owned()).into());
        };
        let Some(reader) = stream.capture.take() else {
            remove_takes(takes);
            return Err(internal("the stream's input is already being recorded".to_owned()).into());
        };
        let take = stream.control.arm_capture();
        TakeWriter::spawn(reader, take, takes).map_err(|error| {
            stream.control.disarm_capture();
            internal(format!("cannot start the take writer: {error}")).into()
        })
    }

    /// Ends `active` (`_finish_recording` up to the alignment): disarms the capture, finishes
    /// the take files after a last drain, collects the recorded MIDI, and reads the latency
    /// estimates and overruns of the stream.
    fn end_recording(&mut self, mut active: ActiveRecording) -> Recorded {
        let stream = self.stream.as_mut().filter(|s| s.serial == active.stream);
        if let Some(stream) = &stream {
            stream.control.disarm_capture();
        }
        let mut takes = Vec::new();
        let mut capture_start = None;
        let mut writer = WriterStats::default();
        if let Some(running) = active.writer.take() {
            match running.finish() {
                Some(result) => {
                    takes = result.takes;
                    capture_start = result.capture_start;
                    writer = result.stats;
                    if let Some(stream) = stream {
                        stream.capture = Some(result.reader);
                    }
                }
                None => {
                    // The writer panicked: its files are incomplete and not offered as takes.
                    for path in &active.files {
                        drop(std::fs::remove_file(path));
                    }
                }
            }
        }
        while let Some(entry) = self.recorded_midi.pop() {
            active.midi_events.push((entry.frame, entry.event));
        }
        let (latency, input_latency, overruns) = self.stream.as_ref().map_or((0, 0, 0), |s| {
            let stats = s.control.stats();
            (
                stats.latency_frames.get(),
                stats.input_latency_frames.get(),
                stats.overruns,
            )
        });
        let stop_frame = active
            .record_from
            .saturating_add(1)
            .max(self.position.saturating_sub(i64::from(latency)));
        Recorded {
            record_from: active.record_from,
            stop_frame,
            capture_start,
            latency_frames: latency,
            input_latency_frames: input_latency,
            capture_overruns: overruns,
            takes,
            midi_tracks: active.midi_tracks,
            midi_events: active.midi_events,
            writer,
        }
    }

    /// A start that could not get a graph: the transport stops.
    fn fail_start(&mut self) {
        self.state = PlayerState::Stopped;
        lock(&self.prefetch).end();
        self.post(Message::Halt {
            generation: self.generation,
            rest: self.position,
            feed: None,
        });
    }

    /// Sends `graph` with the start of `cue`, its media feed and its live-input targets.
    fn launch(&mut self, graph: Box<crate::RenderGraph>, snapshot: &ProjectSnapshot, cue: Cue) {
        let feed = self.feed(FeedStart {
            generation: cue.generation,
            index: 0,
            frame: cue.start,
            looped: cue.looped,
            end: cue.end,
        });
        let targets = self.targets_for(graph.id(), snapshot);
        self.in_flight.push_back(Flight::Graph {
            id: graph.id(),
            version: self.graph.as_ref().map_or(snapshot.version, |g| g.version),
        });
        self.post(Message::Start {
            graph: Update::Graph(Some(graph)),
            feed,
            targets,
            cue,
        });
    }

    /// The recalled graph is back: resets it (or a rebuild that took over its processors, when
    /// the project changed) and starts the waiting play with it. Without a waiting play (a stop
    /// came first) the graph goes back unchanged, so its processors keep their state.
    fn complete_start(&mut self, recalled: Option<Box<crate::RenderGraph>>) {
        let Some(pending) = self.starting.take() else {
            if let Some(graph) = recalled {
                self.return_graph(graph);
            }
            return;
        };
        let current = self.graph.as_ref().is_some_and(|g| {
            g.version == pending.snapshot.version && g.project.id == pending.snapshot.project.id
        });
        let graph = match recalled {
            Some(mut graph) if current => {
                graph.reset();
                Ok(graph)
            }
            Some(mut old) => match self.build(&pending.snapshot, false) {
                Ok((mut graph, _)) => {
                    if graph.adopt(&mut old).is_ok() {
                        graph.reset();
                        Ok(graph)
                    } else {
                        // The builder planned against another graph: its stand-ins cannot
                        // play, so the start builds a fresh graph.
                        self.build(&pending.snapshot, true).map(|(graph, _)| graph)
                    }
                }
                Err(error) => Err(error),
            },
            None => self.build(&pending.snapshot, true).map(|(graph, _)| graph),
        };
        match graph {
            Ok(graph) => self.launch(graph, &pending.snapshot, pending.cue),
            Err(error) => {
                self.last_error = Some(error.to_string());
                self.fail_start();
            }
        }
    }

    /// Sends a recalled graph back to the render thread unchanged (live input on a stopped
    /// transport plays it).
    fn return_graph(&mut self, graph: Box<crate::RenderGraph>) {
        let Some(info) = &self.graph else {
            return;
        };
        let snapshot = ProjectSnapshot {
            project: info.project.clone(),
            version: info.version,
            media: Arc::clone(&info.media),
        };
        let targets = self.targets_for(graph.id(), &snapshot);
        self.in_flight.push_back(Flight::Graph {
            id: graph.id(),
            version: info.version,
        });
        self.post(Message::Graph {
            update: Update::Graph(Some(graph)),
            feed: None,
            targets,
        });
    }

    /// Halts the playing sequence at `position` (stop, pause): a new generation, flushed.
    fn halt(&mut self, position: i64) {
        // A play that waits for its graph does not start.
        self.starting = None;
        self.generation = self.generation.next();
        self.flush_stream();
        self.position = position.max(0);
        lock(&self.prefetch).end();
        self.post(Message::Halt {
            generation: self.generation,
            rest: self.position,
            feed: None,
        });
    }

    /// Builds a graph of `snapshot` (fresh, or a rebuild that takes over the playing graph's
    /// state), hands new clip sources to the prefetch thread and records it as the live graph.
    /// Returns the graph and whether its clips differ from the last graph's.
    fn build(
        &mut self,
        snapshot: &ProjectSnapshot,
        fresh: bool,
    ) -> Result<(Box<crate::RenderGraph>, bool), EngineError> {
        let project = &snapshot.project;
        let media = &*snapshot.media;
        let mut graph = if fresh {
            self.builder.fresh(project, &self.registry, media)?
        } else {
            self.builder.rebuild(project, &self.registry, media)?
        };
        let changed = match self.builder.key_media(&mut graph) {
            Some((key, sources)) => {
                lock(&self.prefetch).set_sources(key, sources);
                true
            }
            None => false,
        };
        if let Some(layout) = self.builder.layout() {
            if self.layouts.len() >= LAYOUTS {
                self.layouts.pop_front();
            }
            self.layouts.push_back(layout.clone());
        }
        self.graph = Some(LiveGraph {
            id: graph.id(),
            version: snapshot.version,
            project: project.clone(),
            media: Arc::clone(&snapshot.media),
        });
        self.needs_fresh = false;
        self.failed_version = None;
        Ok((graph, changed))
    }

    /// Builds and sends a graph without starting a sequence: for live input on a stopped
    /// transport (`fresh`), or a rebuild during playback.
    fn install_graph(
        &mut self,
        snapshot: &ProjectSnapshot,
        fresh: bool,
    ) -> Result<(), EngineError> {
        let (graph, media_changed) = self.build(snapshot, fresh)?;
        let feed = if media_changed && self.state.is_rolling() {
            self.continue_feed()
        } else {
            None
        };
        let targets = self.targets_for(graph.id(), snapshot);
        self.in_flight.push_back(Flight::Graph {
            id: graph.id(),
            version: snapshot.version,
        });
        self.post(Message::Graph {
            update: Update::Graph(Some(graph)),
            feed,
            targets,
        });
        Ok(())
    }

    /// A feed for the playing sequence from the render thread's position: after a rebuild that
    /// plays other clips.
    fn continue_feed(&mut self) -> Option<MediaFeed> {
        let cue = self.cue?;
        let (generation, index) = self.shared.progress();
        let index = if generation == cue.generation {
            u64::from(index)
        } else {
            0
        };
        self.feed(FeedStart {
            generation: cue.generation,
            index,
            frame: frame_at(cue.start, cue.looped, index),
            looped: cue.looped,
            end: cue.end,
        })
    }

    /// Opens a feed from `start` with the blocks the render thread needs first already read.
    fn feed(&mut self, start: FeedStart) -> Option<MediaFeed> {
        let target = self.stream.as_ref().map_or(2, |s| s.target_blocks);
        let primed = usize::try_from(target)
            .unwrap_or(PREFETCH_BLOCKS)
            .saturating_add(2)
            .min(PREFETCH_BLOCKS);
        lock(&self.prefetch).begin(start, primed)
    }

    /// The live-input targets of graph `id`, built from `snapshot` (its latest layout).
    fn targets_for(&self, id: GraphId, snapshot: &ProjectSnapshot) -> LiveTargets {
        let Some(layout) = self.layouts.iter().rev().find(|l| l.id() == id) else {
            return LiveTargets::default();
        };
        LiveTargets {
            graph: Some(id),
            nodes: target_tracks(&snapshot.project, self.focus.as_deref())
                .filter_map(|track| layout.node_index(track))
                .collect(),
        }
    }

    /// The targets of the last graph sent, after a focus change.
    fn current_targets(&self) -> Option<LiveTargets> {
        let graph = self.graph.as_ref()?;
        let snapshot = ProjectSnapshot {
            project: graph.project.clone(),
            version: graph.version,
            media: Arc::clone(&graph.media),
        };
        Some(self.targets_for(graph.id, &snapshot))
    }

    /// Rebuilds the graph when `snapshot` is a newer project than the playing graph's and the
    /// graph renders (`graph_for`), and follows the project's metronome switch.
    fn follow(&mut self, snapshot: &ProjectSnapshot) {
        self.shared
            .set_metronome(snapshot.project.transport.metronome);
        let Some(graph) = &self.graph else {
            return;
        };
        if self.starting.is_some() {
            // The graph is on its way back to be reset; the start rebuilds it if needed.
            return;
        }
        if !self.state.is_rolling() && self.midi_ports.is_empty() {
            // Nothing renders: the next start rebuilds it, as the prototype's `graph_for`
            // rebuilt only when it rendered.
            return;
        }
        let changed = graph.version != snapshot.version || graph.project.id != snapshot.project.id;
        if !(changed || self.needs_fresh) || self.failed_version == Some(snapshot.version) {
            return;
        }
        let fresh = self.needs_fresh || graph.project.id != snapshot.project.id;
        if let Err(error) = self.install_graph(snapshot, fresh) {
            self.last_error = Some(error.to_string());
            self.failed_version = Some(snapshot.version);
        }
    }

    /// Takes back what the render thread returned and drops it here (RT-09); notes which
    /// graph plays and whether one was rejected.
    fn take_returned(&mut self) {
        while let Some(mut message) = self.sender.take_returned() {
            let recalled = match &mut message {
                Message::Recall(Update::Graph(graph)) => Some(graph.take()),
                _ => None,
            };
            let update = match &message {
                Message::Start { graph, .. } => Some(graph),
                Message::Graph { update, .. } | Message::Recall(update) => Some(update),
                _ => None,
            };
            // Every graph message has its flight; a recall's needs no bookkeeping.
            if let Some(update) = update
                && let Some(Flight::Graph { id, version }) = self.in_flight.pop_front()
            {
                let rejected = matches!(update, Update::Graph(Some(back)) if back.id() == id);
                if rejected {
                    self.needs_fresh = true;
                } else {
                    self.playing_version = Some(version);
                }
            }
            drop(message);
            if let Some(graph) = recalled {
                self.complete_start(graph);
            }
        }
    }

    /// Answers the processors' setup requests: the work runs here, the result goes to
    /// the render thread.
    fn answer_tickets(&mut self) {
        while let Some(ticket) = self.tickets.pop() {
            self.post(Message::Setup(Update::Setup(ticket.prepare())));
        }
    }

    /// Applies the end of a play range that the render thread reported: the transport stops
    /// at the range's end frame.
    fn reconcile(&mut self) {
        if self.state == PlayerState::Playing
            && let Some(cue) = self.cue
            && self.shared.ended() == Some(cue.generation)
        {
            self.state = PlayerState::Stopped;
            self.position = cue.end.unwrap_or(self.position);
        }
    }

    fn current_frame(&self) -> i64 {
        if !self.state.is_rolling() {
            return self.position;
        }
        self.stream
            .as_ref()
            .and_then(|s| s.control.position())
            .map_or(self.position, sonara_rt::TimelineFrame::get)
    }

    fn status(&self, project: Option<&Project>) -> PlayerStatus {
        let stats = self.stream.as_ref().map(|s| s.control.stats());
        let stats = stats.unwrap_or(StatsSnapshot {
            running: false,
            playing: false,
            sample_rate: 0,
            buffer_frames: 0,
            callback_frames: 0,
            callbacks: 0,
            underruns: 0,
            overruns: 0,
            queued_frames: 0,
            timeline_frame: None,
            latency_frames: sonara_rt::LatencyFrames::new(0),
            input_latency_frames: sonara_rt::LatencyFrames::new(0),
            peak: sonara_rt::Peaks::default(),
            input_peak: sonara_rt::Peaks::default(),
            callback_load: 0.0,
            device_errors: 0,
            stopped_by_device: false,
            rerouted: 0,
        });
        let rate = if stats.sample_rate > 0 {
            stats.sample_rate
        } else {
            self.stream.as_ref().map_or(48_000, |s| s.sample_rate)
        };
        let rate = f64::from(rate.max(1));
        let counters = self.shared.counters();
        let render_load = if counters.blocks_rendered > 0 {
            let played = count_f64(counters.blocks_rendered.saturating_mul(512)) / rate;
            py_round(count_f64(counters.render_nanos) / 1e9 / played.max(1e-9), 3)
        } else {
            0.0
        };
        let (media_errors, media_error) = {
            let prefetch = lock(&self.prefetch);
            let (count, last) = prefetch.errors();
            (count, last.map(str::to_owned))
        };
        let last_error = self.last_error.clone().or_else(|| {
            media_error
                .filter(|_| media_errors > 0)
                .map(|e| format!("MediaError: {e}"))
        });
        PlayerStatus {
            state: self.state,
            position: Position::of(self.current_frame(), project),
            device_sample_rate: stats.sample_rate,
            buffer_frames: stats.buffer_frames,
            queued_ms: py_round(1000.0 * count_f64(stats.queued_frames) / rate, 1),
            lookahead_ms: self.settings.lookahead_ms,
            output_latency_estimate_ms: py_round(
                1000.0 * f64::from(stats.latency_frames.get()) / rate,
                2,
            ),
            underruns: stats.underruns,
            callback_load: py_round(f64::from(stats.callback_load), 4),
            render_load,
            looped: self
                .cue
                .and_then(|cue| cue.looped)
                .map(|range| (range.start(), range.end())),
            device_errors: stats.device_errors,
            last_error,
            device_status: stats.stopped_by_device.then_some("device stopped"),
        }
    }

    /// Moves the render thread's meters and reports into the history, and recorded MIDI into
    /// the running recording.
    fn collect(&mut self) {
        while let Some(entry) = self.recorded_midi.pop() {
            if let Some(recording) = self.recording.as_mut() {
                recording.midi_events.push((entry.frame, entry.event));
            }
        }
        while let Some(entry) = self.meter_entries.pop() {
            let same = self
                .meters
                .back()
                .is_some_and(|b| b.block == entry.block && b.graph == entry.graph);
            if !same {
                if self.meters.len() >= METER_HISTORY {
                    self.meters.pop_front();
                }
                self.meters.push_back(MeterBlock {
                    block: entry.block,
                    tag: entry.tag,
                    pass: entry.pass,
                    graph: entry.graph,
                    nodes: Vec::new(),
                });
            }
            if let Some(block) = self.meters.back_mut() {
                block.nodes.push((entry.node, entry.meter));
            }
        }
        while let Some(entry) = self.report_entries.pop() {
            match &mut self.reports {
                Some(latest) if latest.block == entry.block && latest.graph == entry.graph => {
                    latest.reports.push((entry.device, entry.report));
                }
                _ => {
                    self.reports = Some(LatestReports {
                        block: entry.block,
                        graph: entry.graph,
                        reports: vec![(entry.device, entry.report)],
                    });
                }
            }
        }
    }

    fn layout(&self, id: GraphId) -> Option<&GraphLayout> {
        self.layouts.iter().rev().find(|layout| layout.id() == id)
    }

    fn meters_at(&self, frame: i64) -> Vec<TrackMeter> {
        let Some(pass) = self.meters.back().map(|b| b.pass) else {
            return Vec::new();
        };
        let Some(block) = self
            .meters
            .iter()
            .rev()
            .take_while(|b| b.pass == pass)
            .find(|b| b.tag <= frame)
        else {
            return Vec::new();
        };
        let Some(layout) = self.layout(block.graph) else {
            return Vec::new();
        };
        block
            .nodes
            .iter()
            .filter_map(|(node, meter)| {
                Some(TrackMeter {
                    track_id: layout.track_id(*node)?.to_owned(),
                    peak: meter.peak,
                    rms: meter.rms,
                })
            })
            .collect()
    }

    fn device_reports(&self) -> Vec<DeviceReport> {
        let Some(latest) = &self.reports else {
            return Vec::new();
        };
        let Some(layout) = self.layout(latest.graph) else {
            return Vec::new();
        };
        latest
            .reports
            .iter()
            .filter_map(|(device, report)| {
                Some(DeviceReport {
                    device_id: layout.device_id(*device)?.to_owned(),
                    values: report.entries().to_vec(),
                })
            })
            .collect()
    }

    /// Queues `message` for the render thread, behind any message still waiting.
    fn post(&mut self, message: Message) {
        self.outbox.push_back(message);
    }

    /// Sends the waiting messages that fit; the rest wait for the next call.
    fn flush_outbox(&mut self) {
        self.take_returned();
        while let Some(message) = self.outbox.pop_front() {
            if let Err(handoff_full) = self.sender.send(message) {
                self.outbox.push_front(handoff_full.0);
                break;
            }
        }
    }
}

impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let core = lock(&self.core);
        f.debug_struct("Player")
            .field("state", &core.state)
            .field("generation", &core.generation)
            .field("position", &core.position)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{ring_blocks, target_blocks};

    /// The prototype's sizes: 80 ms at 48 kHz is a ring of 7680 frames (15 blocks) and a target
    /// of 8 blocks; 25 ms is 3 blocks; the test lookahead of 250 ms is 47 blocks.
    #[test]
    fn lookahead_sizes_follow_the_prototype() {
        assert_eq!(target_blocks(80.0, 48_000), 8);
        assert_eq!(target_blocks(25.0, 48_000), 3);
        assert_eq!(target_blocks(1.0, 48_000), 2);
        assert_eq!(ring_blocks(80.0, 48_000), 15);
        assert_eq!(ring_blocks(250.0, 48_000), 47);
        assert_eq!(ring_blocks(10.0, 48_000), 4);
    }
}
