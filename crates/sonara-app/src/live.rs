//! The live frame of an interface: the transport, the engine status, the
//! track meters and the devices' gain reduction in one packed little-endian binary frame.
//!
//! The prototype polled `transport.status()`, `meters()` and `device_reports()` from a 33 ms
//! GUI timer. Here a [`LiveFeed`] thread builds a frame and hands its bytes to a
//! sink (the desktop's Tauri channel): every display frame (60 Hz) while playing or recording,
//! and about 4 Hz otherwise ([`LiveFeed::IDLE_INTERVAL`]), plus at once when the state changes.
//! It looks at the transport's state 60 times a second either way, so a play starts the fast
//! frames within one display frame.
//!
//! **Real-time rules.** The feed is an ordinary thread. It reads the player's state through the
//! transport service's typed snapshot
//! ([`crate::transport::TransportService::live_snapshot`]), which takes the same locks as the
//! agent's JSON calls and never touches the render or device threads (RT-01, RT-06, RT-12). It
//! never creates the transport: without one, the frame says so, as the prototype's
//! `transport_status` answered "available: None" without creating it.
//!
//! **Position and latency** stay separate fields (RT-07): the position is the transport's
//! software position, and the output latency is the backend's estimate.
//!
//! **Layout** (format [`FRAME_VERSION`]; all little-endian):
//!
//! | Offset | Type | Field |
//! |---|---|---|
//! | 0 | u16 | format version, [`FRAME_VERSION`] |
//! | 2 | u16 | flags: 1 transport, 2 available, 4 running, 8 musical position, 16 device stopped |
//! | 4 | u8 | state: 0 stopped, 1 playing, 2 paused, 3 recording |
//! | 5 | u8, u16 | 0 |
//! | 8 | u32 | sequence number (wraps) |
//! | 12 | u32 | meter layout ([`crate::view::meter_layout`]; 0 without a project) |
//! | 16 | u64 | session version of the project the meters were laid out for |
//! | 24 | i64 | position in frames |
//! | 32 | f64 | position in beats (musical position only) |
//! | 40 | f64 | position in seconds (musical position only) |
//! | 48 | u32 | device sample rate (0 without a running device) |
//! | 52 | u32 | device buffer in frames |
//! | 56 | f64 | render load, 0–1 |
//! | 64 | f64 | the last callback's load, 0–1 |
//! | 72 | f64 | output latency estimate in ms |
//! | 80 | f64 | lookahead in ms |
//! | 88 | f64 | queued audio in ms |
//! | 96 | u64 | underruns |
//! | 104 | u64 | device errors |
//! | 112 | u32 | meter count: the tracks in order, then the master |
//! | 116 | u32 | device count: each track's instrument and effects in order, then the master's |
//! | 120 | 16 × meters | per meter f32 peak left, peak right, RMS left, RMS right (linear) |
//! | … | 4 × devices | per device f32 gain reduction in dB; NaN for a device that reports none |
//!
//! The meters and devices follow the project's order only when the frame's meter layout equals
//! the layout of the page's project view.

use std::io;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{RecvTimeoutError, Sender};
use sonara_core::model::project::Project;
use sonara_core::model::track::Track;

use crate::context::AppContext;
use crate::transport::{PlayerState, Position, TransportSnapshot};
use crate::view::meter_layout;

/// The format version of a frame.
pub const FRAME_VERSION: u16 = 1;
/// The bytes before the meters.
pub const HEADER_BYTES: usize = 120;
/// The bytes of one meter.
pub const METER_BYTES: usize = 16;
/// The bytes of one device's gain reduction.
pub const DEVICE_BYTES: usize = 4;

const FLAG_TRANSPORT: u16 = 1;
const FLAG_AVAILABLE: u16 = 2;
const FLAG_RUNNING: u16 = 4;
const FLAG_MUSICAL: u16 = 8;
const FLAG_DEVICE_STOPPED: u16 = 16;

/// The transport state in a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LiveState {
    /// Stopped (also before a player exists).
    #[default]
    Stopped,
    /// Playing.
    Playing,
    /// Paused.
    Paused,
    /// Recording.
    Recording,
}

impl LiveState {
    fn code(self) -> u8 {
        match self {
            Self::Stopped => 0,
            Self::Playing => 1,
            Self::Paused => 2,
            Self::Recording => 3,
        }
    }

    fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Stopped),
            1 => Some(Self::Playing),
            2 => Some(Self::Paused),
            3 => Some(Self::Recording),
            _ => None,
        }
    }

    /// Whether the transport rolls: playing or recording.
    #[must_use]
    pub fn is_rolling(self) -> bool {
        matches!(self, Self::Playing | Self::Recording)
    }
}

impl From<PlayerState> for LiveState {
    fn from(state: PlayerState) -> Self {
        match state {
            PlayerState::Stopped => Self::Stopped,
            PlayerState::Playing => Self::Playing,
            PlayerState::Paused => Self::Paused,
            PlayerState::Recording => Self::Recording,
        }
    }
}

/// How far the audio engine is (the frame's flags 1, 2 and 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EngineState {
    /// The transport was not created: nothing has opened the audio context yet.
    #[default]
    Absent,
    /// The transport exists, but no audio backend is usable ("Audio engine unavailable").
    Unavailable,
    /// A backend is usable; no player runs yet.
    Ready,
    /// A player runs: the state, position and engine fields are its status.
    Running,
}

impl EngineState {
    fn flags(self) -> u16 {
        match self {
            Self::Absent => 0,
            Self::Unavailable => FLAG_TRANSPORT,
            Self::Ready => FLAG_TRANSPORT | FLAG_AVAILABLE,
            Self::Running => FLAG_TRANSPORT | FLAG_AVAILABLE | FLAG_RUNNING,
        }
    }

    fn from_flags(flags: u16) -> Self {
        if flags & FLAG_RUNNING != 0 {
            Self::Running
        } else if flags & FLAG_AVAILABLE != 0 {
            Self::Ready
        } else if flags & FLAG_TRANSPORT != 0 {
            Self::Unavailable
        } else {
            Self::Absent
        }
    }
}

/// One meter: post-fader peak and RMS, left and right, linear.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MeterLevels {
    /// Peak, left and right.
    pub peak: [f32; 2],
    /// RMS, left and right.
    pub rms: [f32; 2],
}

/// The software position (RT-07: never the hardware position).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LivePosition {
    /// The timeline frame.
    pub frame: i64,
    /// The beat and the seconds, rounded as the status JSON rounds them; `None` without a
    /// project.
    pub musical: Option<(f64, f64)>,
}

/// One live frame, decoded (see the module docs for its bytes).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LiveFrame {
    /// The frame's sequence number; it wraps.
    pub sequence: u32,
    /// How far the audio engine is; the position and the engine fields are 0 unless it runs.
    pub engine: EngineState,
    /// The transport state.
    pub state: LiveState,
    /// The position.
    pub position: LivePosition,
    /// Whether the device stopped by itself (`device_status`).
    pub device_stopped: bool,
    /// The device's sample rate; 0 without a running device.
    pub sample_rate: u32,
    /// The device period in frames.
    pub buffer_frames: u32,
    /// Render time over played time, 0–1.
    pub render_load: f64,
    /// The last callback's load, 0–1.
    pub callback_load: f64,
    /// The output latency estimate in ms (RT-07: an estimate, never a position).
    pub output_latency_ms: f64,
    /// The lookahead in ms.
    pub lookahead_ms: f64,
    /// The audio queued ahead of the device in ms.
    pub queued_ms: f64,
    /// Underruns since the stream started.
    pub underruns: u64,
    /// Device errors.
    pub device_errors: u64,
    /// The session version of the project the meters were laid out for.
    pub version: u64,
    /// The meter layout ([`meter_layout`]), 0 without a project.
    pub meter_layout: u32,
    /// The meters: the tracks in order, then the master.
    pub meters: Vec<MeterLevels>,
    /// The gain reduction in dB of each track's devices in order, then the master's; NaN for a
    /// device that reports none.
    pub gain_reduction: Vec<f32>,
}

/// Why bytes are not a live frame.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    /// Fewer bytes than the header.
    #[error("a live frame needs {HEADER_BYTES} bytes, got {0}")]
    TooShort(usize),
    /// Another format version.
    #[error("unknown live frame version {0}")]
    Version(u16),
    /// An unknown transport state.
    #[error("unknown transport state {0}")]
    State(u8),
    /// The length does not match the counts.
    #[error("a live frame of {meters} meters and {devices} devices is not {length} bytes")]
    Length {
        /// The meter count.
        meters: u32,
        /// The device count.
        devices: u32,
        /// The byte count.
        length: usize,
    },
}

/// A value as f32, the frame's level precision.
#[expect(
    clippy::cast_possible_truncation,
    reason = "meter levels and gain reduction are drawn, f32 is their precision"
)]
fn level(value: f64) -> f32 {
    value as f32
}

impl LiveFrame {
    /// The frame of `transport` (`None` before the transport exists) for `project` at session
    /// `version`, numbered `sequence`.
    #[must_use]
    pub fn of(
        transport: Option<&TransportSnapshot>,
        project: Option<&Project>,
        version: u64,
        sequence: u32,
    ) -> Self {
        let mut frame = Self {
            sequence,
            version,
            meter_layout: project.map_or(0, meter_layout),
            ..Self::default()
        };
        if let Some(project) = project {
            let meters = transport.map(|t| t.meters.as_slice()).unwrap_or_default();
            frame.meters = project
                .all_tracks()
                .map(|track| {
                    meters
                        .iter()
                        .find(|meter| meter.track_id == track.id)
                        .map(|meter| MeterLevels {
                            peak: meter.peak.map(level),
                            rms: meter.rms.map(level),
                        })
                        .unwrap_or_default()
                })
                .collect();
            let reports = transport.map(|t| t.reports.as_slice()).unwrap_or_default();
            frame.gain_reduction = project
                .all_tracks()
                .flat_map(Track::devices)
                .map(|device| {
                    reports
                        .iter()
                        .find(|report| report.device_id == device.id)
                        .and_then(|report| {
                            report
                                .values
                                .iter()
                                .find(|(name, _)| *name == "gain_reduction_db")
                        })
                        .map_or(f32::NAN, |(_, value)| level(*value))
                })
                .collect();
        }
        let Some(transport) = transport else {
            return frame;
        };
        frame.engine = if transport.available {
            EngineState::Ready
        } else {
            EngineState::Unavailable
        };
        let Some(status) = &transport.status else {
            return frame;
        };
        frame.engine = EngineState::Running;
        frame.state = status.state.into();
        frame.position = match status.position {
            Position::Musical {
                frame,
                beat,
                seconds,
                ..
            } => LivePosition {
                frame,
                musical: Some((beat, seconds)),
            },
            Position::Frame(frame) => LivePosition {
                frame,
                musical: None,
            },
        };
        frame.device_stopped = status.device_status.is_some();
        frame.sample_rate = status.device_sample_rate;
        frame.buffer_frames = status.buffer_frames;
        frame.render_load = status.render_load;
        frame.callback_load = status.callback_load;
        frame.output_latency_ms = status.output_latency_estimate_ms;
        frame.lookahead_ms = status.lookahead_ms;
        frame.queued_ms = status.queued_ms;
        frame.underruns = status.underruns;
        frame.device_errors = status.device_errors;
        frame
    }

    /// The frame of `app` now, numbered `sequence`. It does not create the transport.
    #[must_use]
    pub fn build(app: &AppContext, sequence: u32) -> Self {
        let (project, version) = {
            let session = app.session().lock();
            (session.project().cloned(), session.version())
        };
        let transport = app.has_transport().then(|| app.transport().live_snapshot());
        Self::of(transport.as_ref(), project.as_ref(), version, sequence)
    }

    /// The frame's bytes (see the module docs).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            HEADER_BYTES
                + self.meters.len() * METER_BYTES
                + self.gain_reduction.len() * DEVICE_BYTES,
        );
        let mut flags = self.engine.flags();
        if self.position.musical.is_some() {
            flags |= FLAG_MUSICAL;
        }
        if self.device_stopped {
            flags |= FLAG_DEVICE_STOPPED;
        }
        let (beat, seconds) = self.position.musical.unwrap_or_default();
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        out.extend_from_slice(&FRAME_VERSION.to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());
        out.push(self.state.code());
        out.push(0);
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&self.sequence.to_le_bytes());
        out.extend_from_slice(&self.meter_layout.to_le_bytes());
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&self.position.frame.to_le_bytes());
        out.extend_from_slice(&beat.to_le_bytes());
        out.extend_from_slice(&seconds.to_le_bytes());
        out.extend_from_slice(&self.sample_rate.to_le_bytes());
        out.extend_from_slice(&self.buffer_frames.to_le_bytes());
        for value in [
            self.render_load,
            self.callback_load,
            self.output_latency_ms,
            self.lookahead_ms,
            self.queued_ms,
        ] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&self.underruns.to_le_bytes());
        out.extend_from_slice(&self.device_errors.to_le_bytes());
        out.extend_from_slice(&count(self.meters.len()).to_le_bytes());
        out.extend_from_slice(&count(self.gain_reduction.len()).to_le_bytes());
        for meter in &self.meters {
            for value in meter.peak.iter().chain(&meter.rms) {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        for value in &self.gain_reduction {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out
    }

    /// The frame of `bytes` ([`Self::encode`]).
    ///
    /// # Errors
    ///
    /// A [`FrameError`] for too few bytes, another format version, an unknown state or a length
    /// that does not match the counts.
    pub fn decode(bytes: &[u8]) -> Result<Self, FrameError> {
        let mut reader = Bytes(bytes);
        let header = reader
            .take(HEADER_BYTES)
            .ok_or(FrameError::TooShort(bytes.len()))?;
        let mut h = Bytes(header);
        let version = h.u16();
        if version != FRAME_VERSION {
            return Err(FrameError::Version(version));
        }
        let flags = h.u16();
        let code = h.u8();
        let state = LiveState::from_code(code).ok_or(FrameError::State(code))?;
        h.skip(3);
        let sequence = h.u32();
        let layout = h.u32();
        let session_version = h.u64();
        let position_frame = h.i64();
        let beat = h.f64();
        let seconds = h.f64();
        let sample_rate = h.u32();
        let buffer_frames = h.u32();
        let render_load = h.f64();
        let callback_load = h.f64();
        let output_latency_ms = h.f64();
        let lookahead_ms = h.f64();
        let queued_ms = h.f64();
        let underruns = h.u64();
        let device_errors = h.u64();
        let meters = h.u32();
        let devices = h.u32();
        let expected = usize::try_from(meters)
            .ok()
            .and_then(|m| m.checked_mul(METER_BYTES))
            .zip(
                usize::try_from(devices)
                    .ok()
                    .and_then(|d| d.checked_mul(DEVICE_BYTES)),
            )
            .and_then(|(m, d)| m.checked_add(d));
        if expected != Some(reader.0.len()) {
            return Err(FrameError::Length {
                meters,
                devices,
                length: bytes.len(),
            });
        }
        let mut levels = Vec::with_capacity(usize::try_from(meters).unwrap_or(0));
        for _ in 0..meters {
            levels.push(MeterLevels {
                peak: [reader.f32(), reader.f32()],
                rms: [reader.f32(), reader.f32()],
            });
        }
        let gain_reduction = (0..devices).map(|_| reader.f32()).collect();
        Ok(Self {
            sequence,
            engine: EngineState::from_flags(flags),
            state,
            position: LivePosition {
                frame: position_frame,
                musical: (flags & FLAG_MUSICAL != 0).then_some((beat, seconds)),
            },
            device_stopped: flags & FLAG_DEVICE_STOPPED != 0,
            sample_rate,
            buffer_frames,
            render_load,
            callback_load,
            output_latency_ms,
            lookahead_ms,
            queued_ms,
            underruns,
            device_errors,
            version: session_version,
            meter_layout: layout,
            meters: levels,
            gain_reduction,
        })
    }
}

/// A little-endian reader over bytes whose length was checked: a short read gives zeros.
struct Bytes<'a>(&'a [u8]);

impl<'a> Bytes<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let (head, rest) = self.0.split_at_checked(n)?;
        self.0 = rest;
        Some(head)
    }

    fn array<const N: usize>(&mut self) -> [u8; N] {
        self.take(N)
            .and_then(|bytes| bytes.try_into().ok())
            .unwrap_or([0; N])
    }

    fn skip(&mut self, n: usize) {
        let _ = self.take(n);
    }

    fn u8(&mut self) -> u8 {
        u8::from_le_bytes(self.array())
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.array())
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.array())
    }

    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.array())
    }

    fn i64(&mut self) -> i64 {
        i64::from_le_bytes(self.array())
    }

    fn f32(&mut self) -> f32 {
        f32::from_le_bytes(self.array())
    }

    fn f64(&mut self) -> f64 {
        f64::from_le_bytes(self.array())
    }
}

/// The thread that sends live frames (see the module docs). Dropping it stops the thread.
#[derive(Debug)]
pub struct LiveFeed {
    stop: Option<Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl LiveFeed {
    /// The frame interval while playing or recording: one display frame at 60 Hz.
    pub const ROLLING_INTERVAL: Duration = Duration::from_micros(16_667);
    /// The frame interval otherwise (about 4 Hz; the prototype emitted every 8th 33 ms tick).
    pub const IDLE_INTERVAL: Duration = Duration::from_millis(250);

    /// Starts sending `app`'s frames to `sink` on a thread named `sonara-live`.
    ///
    /// # Errors
    ///
    /// The I/O error of starting the thread.
    pub fn start<F>(app: Arc<AppContext>, mut sink: F) -> io::Result<Self>
    where
        F: FnMut(&[u8]) + Send + 'static,
    {
        let (stop, stopped) = crossbeam_channel::bounded::<()>(0);
        let thread = thread::Builder::new()
            .name("sonara-live".to_owned())
            .spawn(move || {
                let mut sequence: u32 = 0;
                let mut sent: Option<(Instant, Option<PlayerState>)> = None;
                let mut tick = Instant::now();
                loop {
                    let state = if app.has_transport() {
                        app.transport().state()
                    } else {
                        None
                    };
                    let rolling = state.is_some_and(|s| LiveState::from(s).is_rolling());
                    let due = match sent {
                        None => true,
                        Some((at, last)) => {
                            rolling || last != state || at.elapsed() >= Self::IDLE_INTERVAL
                        }
                    };
                    if due {
                        sink(&LiveFrame::build(&app, sequence).encode());
                        sequence = sequence.wrapping_add(1);
                        sent = Some((Instant::now(), state));
                    }
                    // A deadline per tick keeps the rate at 60 Hz whatever a frame costs; a
                    // stall skips the missed ticks instead of bursting.
                    tick += Self::ROLLING_INTERVAL;
                    let now = Instant::now();
                    if tick < now {
                        tick = now;
                    }
                    match stopped.recv_deadline(tick) {
                        Err(RecvTimeoutError::Timeout) => {}
                        Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
            })?;
        Ok(Self {
            stop: Some(stop),
            thread: Some(thread),
        })
    }

    /// Stops the thread and waits for it.
    pub fn stop(mut self) {
        self.halt();
    }

    fn halt(&mut self) {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            // ERR-06: a sink that panicked has ended the thread and reported it there.
            let _ = thread.join();
        }
    }
}

impl Drop for LiveFeed {
    fn drop(&mut self) {
        self.halt();
    }
}
