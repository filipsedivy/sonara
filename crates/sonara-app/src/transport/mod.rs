//! The transport service (the prototype's `TransportService` in
//! `audio/transport/service.py`, with the parts of its `RealtimePlayer` that need the session):
//! real-time playback, recording and live MIDI input on an audio backend. Not real-time code.
//!
//! **Backends** (`create_backend`): `null` has none; `native-null` is miniaudio's null context
//! (a simulated real-time device, the golden `transport` area's backend); `auto`, `native` and
//! any other name the system's default context. A `native` or `native-null` context that cannot
//! be created makes every real-time call fail with `ENGINE_UNAVAILABLE` and the context's
//! message; for `auto` the reason is "native audio engine not available", and for `null`
//! "audio backend disabled (null backend)". Offline rendering and export never depend on it.
//!
//! **Lazy creation.** The context opens when the service is created (the application creates
//! the service on first use); the player and its threads start with the first call that needs
//! them, and the device opens on the first play, recording or live MIDI input.
//!
//! **Threads and locks.** The service's lock guards the player, the MIDI hub and the running
//! recording's take names; it is taken before the session's lock, never after it. The player
//! polls the session for edits (its project source) without the service's lock. A `sonara-midi-scan` thread rescans the hub's ports every 2 s while it wants any (the
//! prototype rescanned on its render thread) and hands new inputs to the player and closed ones
//! back. [`TransportService::shutdown`] finishes a running recording, then drops the player
//! and the scan thread outside every lock, because dropping them joins threads that may wait
//! for those locks.
//!
//! The answers are the prototype's JSON ([`json`](mod@json)).

mod device;
pub mod json;
mod takes;

use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{RecvTimeoutError, Sender};
use serde_json::{Map, Value, json};
use sonara_core::commands::Transaction;
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::model::media::MediaAsset;
use sonara_core::model::project::Project;
use sonara_core::model::track::{Track, TrackKind};
use sonara_core::plugins::PluginRegistry;
use sonara_dsp::processor::InstrumentEvent;
pub use sonara_engine::player::{DeviceReport, PlayerState, PlayerStatus, Position, TrackMeter};
use sonara_engine::player::{
    Player, PlayerSettings, PlayerSetup, ProjectSnapshot, RecordPlan, TakeTarget,
};
use sonara_io::{DeviceInfo, Engine};

pub use self::device::device_json;
use self::device::{EngineDevice, SessionSource, create_backend, render_thread_start, snapshot};
use self::json::{meters_json, paused_json, position_json, reports_json, status_json};
use self::takes::{PendingTake, finish, sample_format, take_path, workspace_path};
use crate::error::{AppResult, IoContext};
use crate::ids::IdSource;
use crate::media::MediaService;
use crate::midi::{MidiDevices, MidiHub, devices_json, idle_status_json, status_json as midi_json};
use crate::session::SharedSession;
use crate::settings::Settings;
use crate::sync::lock;

/// How often the scan thread looks whether the hub's rescan is due.
const SCAN_TICK: Duration = Duration::from_millis(100);

/// Why `sonara devices` could not list the native backend's devices.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DevicesError {
    /// The system's audio context cannot be created: the prototype's `ENGINE_UNAVAILABLE`
    /// reason, which `sonara devices` prints as `{"available": false, "reason"}`.
    #[error("{0}")]
    Unavailable(String),
    /// The context exists but miniaudio cannot list its devices.
    #[error("{0}")]
    Enumeration(String),
}

/// `sonara devices` (`create_backend("native").devices()`): the system context's backend as
/// `native/<name>` and its devices, playback devices first, in the prototype's JSON shape.
///
/// # Errors
///
/// [`DevicesError::Unavailable`] when the context cannot be created,
/// [`DevicesError::Enumeration`] when the devices cannot be listed.
pub fn native_devices() -> Result<Value, DevicesError> {
    let mut engine = create_backend("native")
        .map_err(DevicesError::Unavailable)?
        .ok_or_else(|| DevicesError::Unavailable("native audio engine not available".to_owned()))?;
    let devices = engine
        .devices()
        .map_err(|error| DevicesError::Enumeration(error.to_string()))?;
    let devices: Vec<Value> = devices.iter().map(device_json).collect();
    Ok(json!({
        "backend": format!("native/{}", engine.backend_name()),
        "devices": devices,
    }))
}

/// The transport's state at one moment, typed ([`TransportService::live_snapshot`]).
#[derive(Debug, Clone, PartialEq)]
pub struct TransportSnapshot {
    /// Whether an audio backend is usable.
    pub available: bool,
    /// The player's status; `None` before a player exists.
    pub status: Option<PlayerStatus>,
    /// Each track's post-fader meter of the block being heard; empty before a player
    /// exists.
    pub meters: Vec<TrackMeter>,
    /// The processors' live readouts; empty before a player exists.
    pub reports: Vec<DeviceReport>,
}

/// The application's services the transport works with.
pub(crate) struct TransportParts {
    pub(crate) session: SharedSession,
    pub(crate) plugins: Arc<PluginRegistry>,
    pub(crate) media: Arc<MediaService>,
    pub(crate) settings: Arc<Mutex<Settings>>,
    pub(crate) midi_devices: Arc<dyn MidiDevices>,
    pub(crate) ids: IdSource,
}

/// What the service's lock guards.
#[derive(Default)]
struct Inner {
    player: Option<Player>,
    /// The live lookahead the player was created with (`midi_status`).
    live_lookahead_ms: f64,
    hub: Option<MidiHub>,
    scanner: Option<Scanner>,
    /// The audio takes of the running recording.
    takes: Vec<PendingTake>,
}

/// The state shared with the scan thread.
struct Shared {
    parts: TransportParts,
    inner: Mutex<Inner>,
}

/// The transport: the audio backend, the player and the MIDI hub (see the module docs).
pub struct TransportService {
    shared: Arc<Shared>,
    /// The requested backend name.
    backend: String,
    /// The engine; `None` without a usable backend.
    engine: Option<Arc<Mutex<Engine>>>,
    /// `native/<miniaudio's backend name>`.
    engine_name: String,
    /// Why there is no backend.
    reason: Option<String>,
}

impl fmt::Debug for TransportService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransportService")
            .field("backend", &self.backend)
            .field("engine", &self.engine_name)
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

impl TransportService {
    /// The service on the audio backend `backend` (`auto`, `native`, `native-null` or `null`).
    /// Creates the backend's context; a backend that cannot be created leaves the service
    /// without one, with the reason its calls report.
    #[must_use]
    pub(crate) fn new(parts: TransportParts, backend: &str) -> Self {
        let (engine, reason) = match create_backend(backend) {
            Ok(Some(engine)) => (Some(engine), None),
            Ok(None) if backend == "null" => (
                None,
                Some("audio backend disabled (null backend)".to_owned()),
            ),
            Ok(None) => (None, Some("native audio engine not available".to_owned())),
            Err(reason) => (None, Some(reason)),
        };
        let engine_name = engine
            .as_ref()
            .map(|engine| format!("native/{}", engine.backend_name()))
            .unwrap_or_default();
        Self {
            shared: Arc::new(Shared {
                parts,
                inner: Mutex::new(Inner::default()),
            }),
            backend: backend.to_owned(),
            engine: engine.map(|engine| Arc::new(Mutex::new(engine))),
            engine_name,
            reason,
        }
    }

    // -- status -------------------------------------------------------------------------

    /// Whether a backend is usable.
    #[must_use]
    pub fn available(&self) -> bool {
        self.engine.is_some()
    }

    /// The backend's engine (`require_backend`).
    ///
    /// # Errors
    ///
    /// `ENGINE_UNAVAILABLE` with the reason there is none.
    fn require_backend(&self) -> Result<&Arc<Mutex<Engine>>, DomainError> {
        self.engine.as_ref().ok_or_else(|| {
            DomainError::new(
                ErrorCode::EngineUnavailable,
                self.reason.as_deref().unwrap_or("no audio backend"),
            )
        })
    }

    /// The backend's devices: playback devices first, then capture devices.
    ///
    /// # Errors
    ///
    /// `ENGINE_UNAVAILABLE` without a backend; `DEVICE_ERROR` when the devices cannot be listed.
    pub fn devices(&self) -> AppResult<Vec<DeviceInfo>> {
        let engine = self.require_backend()?;
        let devices = lock(engine)
            .devices()
            .map_err(|error| DomainError::new(ErrorCode::DeviceError, error.to_string()))?;
        Ok(devices)
    }

    /// `status()`: without a backend `{"available": false, "backend", "reason"}`; otherwise
    /// `{"available": true, "backend": "native/<name>"}` with the player's status, or
    /// `"state": "stopped"` before a player exists.
    #[must_use]
    pub fn status(&self) -> Value {
        if self.engine.is_none() {
            return json!({
                "available": false,
                "backend": self.backend,
                "reason": self.reason,
            });
        }
        let mut out = Map::new();
        out.insert("available".to_owned(), json!(true));
        out.insert("backend".to_owned(), json!(self.engine_name));
        let inner = self.inner();
        match &inner.player {
            Some(player) => {
                let project = self.shared.parts.session.project();
                out.extend(status_json(&player.status(project.as_ref())));
            }
            None => {
                out.insert("state".to_owned(), json!("stopped"));
            }
        }
        Value::Object(out)
    }

    // -- control ------------------------------------------------------------------------

    /// `play(start_beat, end_beat, loop)`: the player's status with the new position.
    ///
    /// # Errors
    ///
    /// `NO_PROJECT`, then `ENGINE_UNAVAILABLE`, then the player's errors (`DEVICE_ERROR`,
    /// `INVALID_ARGUMENT` "loop range is too short", the graph's).
    pub fn play(
        &self,
        start_beat: Option<f64>,
        end_beat: Option<f64>,
        looped: bool,
    ) -> AppResult<Value> {
        self.shared.parts.session.lock().require_project()?;
        let mut inner = self.inner();
        let player = self.player(&mut inner)?;
        let snapshot = self.snapshot()?;
        let status = player.play(&snapshot, start_beat, end_beat, looped)?;
        Ok(Value::Object(status_json(&status)))
    }

    /// `stop()`: `{"stopped": true, "was_playing": …}` with the position, and the takes in
    /// `"recording"` when it finished a recording. Without a player nothing was playing.
    ///
    /// # Errors
    ///
    /// The errors of finishing a recording: inspecting its files, and the "Record"
    /// transaction's. The transport has stopped either way.
    pub fn stop(&self) -> AppResult<Value> {
        let mut inner = self.inner();
        self.stop_locked(&mut inner)
    }

    fn stop_locked(&self, inner: &mut Inner) -> AppResult<Value> {
        let Some(player) = inner.player.as_ref() else {
            return Ok(json!({"stopped": true, "was_playing": false}));
        };
        let project = self.shared.parts.session.project();
        let stopped = player.stop(project.as_ref());
        let recording = match &stopped.recording {
            Some(recorded) => {
                Some(self.finish_recording(recorded, &std::mem::take(&mut inner.takes))?)
            }
            None => None,
        };
        let mut out = Map::new();
        out.insert("stopped".to_owned(), json!(true));
        out.insert("was_playing".to_owned(), json!(stopped.was_playing));
        if let Some(position) = &stopped.position {
            out.extend(position_json(position));
        }
        if let Some(recording) = recording {
            out.insert("recording".to_owned(), recording);
        }
        Ok(Value::Object(out))
    }

    /// `pause()`: `{"paused": true, …position}` while playing; otherwise `{"paused": false,
    /// "state": …}`, or `{"paused": false}` before a player exists.
    #[must_use]
    pub fn pause(&self) -> Value {
        let inner = self.inner();
        let Some(player) = &inner.player else {
            return json!({"paused": false});
        };
        let project = self.shared.parts.session.project();
        paused_json(&player.pause(project.as_ref()))
    }

    /// `seek(beat)`: the new position.
    ///
    /// # Errors
    ///
    /// `ENGINE_UNAVAILABLE`, then `NO_PROJECT`.
    pub fn seek(&self, beat: f64) -> AppResult<Value> {
        let mut inner = self.inner();
        let player = self.player(&mut inner)?;
        let project = self.shared.parts.session.lock().require_project()?.clone();
        Ok(Value::Object(position_json(&player.seek(&project, beat))))
    }

    /// Plays or releases one GUI preview note of `track_id`'s instrument, for the piano roll
    /// keyboard's click. Opens the device and builds a graph on a stopped transport too, the
    /// same as live MIDI input.
    ///
    /// # Errors
    ///
    /// `NO_PROJECT`, `ENGINE_UNAVAILABLE`, and the player's errors (`DEVICE_ERROR`).
    pub fn audition_note(&self, track_id: &str, event: InstrumentEvent) -> AppResult<()> {
        self.shared.parts.session.lock().require_project()?;
        let mut inner = self.inner();
        let player = self.player(&mut inner)?;
        let snapshot = self.snapshot()?;
        player.audition(&snapshot, track_id, event)?;
        Ok(())
    }

    /// `position_beats()`: the transport's position in beats; 0 before a player exists.
    #[must_use]
    pub fn position_beats(&self) -> f64 {
        let inner = self.inner();
        inner.player.as_ref().map_or(0.0, |player| {
            let project = self.shared.parts.session.project();
            player.position_beats(project.as_ref())
        })
    }

    /// `record(start_beat, track_ids)`: records the tracks `track_ids` (all armed tracks when
    /// `None` or empty) from `start_beat` (the resting position when `None`) until
    /// [`stop`](Self::stop): audio tracks from the input device, instrument tracks from the
    /// MIDI inputs. Answers `{"recording": true, "tracks": [audio…, instrument…], …position}`.
    ///
    /// A recording that is still running is finished first, its takes added as by a stop.
    ///
    /// # Errors
    ///
    /// In the prototype's order: `ENGINE_UNAVAILABLE`; `NO_PROJECT`; `INVALID_ARGUMENT` "no
    /// armed audio or instrument tracks to record"; `DEVICE_ERROR` when the device cannot be
    /// opened or has too few input channels; `INVALID_ARGUMENT` for a `record_subtype` the
    /// take writer does not write; the errors of creating the take files and of starting.
    pub fn record(
        &self,
        start_beat: Option<f64>,
        track_ids: Option<&[String]>,
    ) -> AppResult<Value> {
        let mut inner = self.inner();
        self.player(&mut inner)?;
        if inner.player.as_ref().is_some_and(Player::is_recording) {
            self.stop_locked(&mut inner)?;
        }
        let (project, version, workspace) = {
            let session = self.shared.parts.session.lock();
            (
                session.require_project()?.clone(),
                session.version(),
                session.require_location()?.workspace.clone(),
            )
        };
        let wanted = track_ids.filter(|ids| !ids.is_empty());
        let chosen: Vec<&Track> = project
            .tracks
            .iter()
            .filter(|t| wanted.map_or(t.input.armed, |ids| ids.contains(&t.id)))
            .collect();
        let audio: Vec<&Track> = chosen
            .iter()
            .copied()
            .filter(|t| t.kind == TrackKind::Audio)
            .collect();
        let midi: Vec<String> = chosen
            .iter()
            .filter(|t| t.kind == TrackKind::Instrument)
            .map(|t| t.id.clone())
            .collect();
        if audio.is_empty() && midi.is_empty() {
            return Err(
                DomainError::invalid("no armed audio or instrument tracks to record").into(),
            );
        }
        let Some(player) = inner.player.as_ref() else {
            return Err(DomainError::new(ErrorCode::Internal, "no player").into());
        };
        if audio.is_empty() {
            player.open_stream(&project, None)?;
        } else {
            let inputs = audio
                .iter()
                .filter_map(|t| t.input.channels.iter().max())
                .map(|&channel| {
                    usize::try_from(channel)
                        .unwrap_or(usize::MAX)
                        .saturating_add(1)
                })
                .max()
                .unwrap_or(0);
            let configured =
                usize::try_from(self.shared.parts.settings().audio.input_channels).unwrap_or(0);
            let opened = player.open_stream(&project, Some(inputs.max(configured)))?;
            if opened < inputs {
                return Err(DomainError::new(
                    ErrorCode::DeviceError,
                    "the audio device has too few input channels",
                )
                .into());
            }
        }
        if !midi.is_empty() && inner.hub.is_none() {
            // The prototype's player opened every port here, not the configured ones.
            self.open_hub(&mut inner, None);
        }
        let (targets, pending) = self.name_takes(&project, &workspace, &audio)?;
        let tracks: Vec<String> = audio
            .iter()
            .map(|t| t.id.clone())
            .chain(midi.iter().cloned())
            .collect();
        let snapshot = ProjectSnapshot {
            project: project.clone(),
            version,
            media: self.shared.parts.media.provider(),
        };
        let Some(player) = inner.player.as_ref() else {
            return Err(DomainError::new(ErrorCode::Internal, "no player").into());
        };
        let position = player.record(
            &snapshot,
            RecordPlan {
                start_beat,
                takes: targets,
                midi_tracks: midi,
            },
        )?;
        inner.takes = pending;
        let mut out = Map::new();
        out.insert("recording".to_owned(), json!(true));
        out.insert("tracks".to_owned(), json!(tracks));
        out.extend(position_json(&position));
        Ok(Value::Object(out))
    }

    /// The take files of the audio tracks `audio`, with the asset IDs drawn in track order
    /// (`record`'s loop): `media/audio/{asset_id}-{safe_stem(name)}-take.wav` in `workspace`,
    /// with the project's `record_subtype`.
    fn name_takes(
        &self,
        project: &Project,
        workspace: &Path,
        audio: &[&Track],
    ) -> AppResult<(Vec<TakeTarget>, Vec<PendingTake>)> {
        let mut ids = self.shared.parts.ids.clone();
        let mut pending = Vec::new();
        let mut targets = Vec::new();
        for track in audio {
            let asset_id = MediaAsset::new_id(&mut ids);
            let rel = take_path(&asset_id, &track.name);
            let path = workspace_path(workspace, &rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).io("create", parent)?;
            }
            let format = sample_format(&project.settings.record_subtype)?;
            targets.push(TakeTarget {
                track_id: track.id.clone(),
                path,
                channels: track
                    .input
                    .channels
                    .iter()
                    .map(|&c| usize::try_from(c).unwrap_or(usize::MAX))
                    .collect(),
                format,
            });
            pending.push(PendingTake {
                track_id: track.id.clone(),
                track_name: track.name.clone(),
                asset_id,
                rel,
            });
        }
        Ok((targets, pending))
    }

    /// Adds the takes of `recorded` to the project in one "Record" transaction and answers the
    /// prototype's recording JSON.
    fn finish_recording(
        &self,
        recorded: &sonara_engine::player::Recorded,
        pending: &[PendingTake],
    ) -> AppResult<Value> {
        let session = &self.shared.parts.session;
        let project = session.lock().require_project()?.clone();
        let mut ids = self.shared.parts.ids.clone();
        let finished = finish(&project, recorded, pending, &mut ids)?;
        if !finished.commands.is_empty() {
            let transaction = Transaction {
                commands: finished.commands,
                name: "Record".to_owned(),
            };
            session.lock().execute(&transaction, false, "user")?;
        }
        Ok(finished.result)
    }

    // -- MIDI ---------------------------------------------------------------------------

    /// `midi_devices()`: the MIDI API and the port names (`devices` of `list_midi_devices`).
    ///
    /// # Errors
    ///
    /// The port listing failed.
    pub fn midi_devices(&self) -> Result<Value, crate::midi::MidiError> {
        devices_json(self.shared.parts.midi_devices.as_ref())
    }

    /// `enable_midi(ports)`: opens the MIDI inputs `ports` (the configured ones when `None`,
    /// and all when none are configured) for live playing and recording; answers
    /// `{"enabled": true, "ports": [opened…]}`. An open hub is closed first.
    ///
    /// # Errors
    ///
    /// `ENGINE_UNAVAILABLE`, and the errors of starting the player.
    pub fn enable_midi(&self, ports: Option<Vec<String>>) -> AppResult<Value> {
        let mut inner = self.inner();
        self.player(&mut inner)?;
        let wanted = ports.or_else(|| self.shared.parts.settings().audio.midi_inputs.clone());
        let opened = self.open_hub(&mut inner, wanted);
        Ok(json!({"enabled": true, "ports": opened}))
    }

    /// `disable_midi()`: closes the MIDI inputs; answers `{"enabled": false}`.
    #[must_use]
    pub fn disable_midi(&self) -> Value {
        let mut inner = self.inner();
        close_hub(&mut inner);
        json!({"enabled": false})
    }

    /// `set_midi_focus`: the instrument track that takes live input besides the armed ones.
    pub fn set_midi_focus(&self, track_id: Option<String>) {
        if let Some(player) = &self.inner().player {
            player.set_midi_focus(track_id);
        }
    }

    /// `midi_status()`: `{"enabled", "ports", "targets", "messages_received",
    /// "live_lookahead_ms", "errors"}`, or `{"enabled": false, "ports": []}` before a player
    /// exists.
    #[must_use]
    pub fn midi_status(&self) -> Value {
        let inner = self.inner();
        let Some(player) = &inner.player else {
            return idle_status_json();
        };
        let targets = self
            .shared
            .parts
            .session
            .project()
            .map(|project| player.midi_targets(&project))
            .unwrap_or_default();
        midi_json(
            inner.hub.as_ref(),
            &targets,
            player.messages_received(),
            inner.live_lookahead_ms,
        )
    }

    /// Opens a new hub on `wanted`, hands its inputs to the player and starts the scan thread.
    fn open_hub(&self, inner: &mut Inner, wanted: Option<Vec<String>>) -> Vec<String> {
        close_hub(inner);
        let mut hub = MidiHub::new(Arc::clone(&self.shared.parts.midi_devices), wanted);
        let opened = hub.refresh();
        inner.hub = Some(hub);
        self.shared.sync_inputs(inner);
        if inner.scanner.is_none() {
            match Scanner::spawn(Arc::downgrade(&self.shared)) {
                Ok(scanner) => inner.scanner = Some(scanner),
                Err(error) => log::warn!("cannot start the MIDI port scan: {error}"),
            }
        }
        opened
    }

    // -- meters and reports ---------------------------------------------------------------

    /// `meters()`: each track's post-fader peak and RMS for the block being heard (the newest
    /// block of the current loop pass at the device's position); `{}` before a player
    /// exists.
    #[must_use]
    pub fn meters(&self) -> Value {
        self.inner()
            .player
            .as_ref()
            .map_or_else(|| json!({}), |player| meters_json(&player.meters()))
    }

    /// `device_reports()`: the processors' live readouts by device ID; `{}` before a player
    /// exists.
    #[must_use]
    pub fn device_reports(&self) -> Value {
        self.inner().player.as_ref().map_or_else(
            || json!({}),
            |player| reports_json(&player.device_reports()),
        )
    }

    /// The transport's state for an interface's live frame: the typed
    /// form of [`status`](Self::status), [`meters`](Self::meters) and
    /// [`device_reports`](Self::device_reports), read under one lock of the service. The JSON
    /// forms stay for the agent. Not real-time code: it locks the service and the player, as
    /// the JSON forms do.
    #[must_use]
    pub fn live_snapshot(&self) -> TransportSnapshot {
        let inner = self.inner();
        let Some(player) = &inner.player else {
            return TransportSnapshot {
                available: self.engine.is_some(),
                status: None,
                meters: Vec::new(),
                reports: Vec::new(),
            };
        };
        let project = self.shared.parts.session.project();
        TransportSnapshot {
            available: true,
            status: Some(player.status(project.as_ref())),
            meters: player.meters(),
            reports: player.device_reports(),
        }
    }

    /// The player's state, `None` before a player exists: a cheaper question than
    /// [`live_snapshot`](Self::live_snapshot) for deciding how often to ask it.
    #[must_use]
    pub fn state(&self) -> Option<PlayerState> {
        self.inner().player.as_ref().map(Player::state)
    }

    /// `output_peaks()`: the device's output peaks since the previous call; empty without a
    /// stream.
    #[must_use]
    pub fn output_peaks(&self) -> Vec<f32> {
        self.inner()
            .player
            .as_ref()
            .map(Player::output_peaks)
            .unwrap_or_default()
    }

    /// The session version of the graph the player plays; `None` before it plays one (the
    /// prototype's tests read `_graph_version`).
    #[must_use]
    pub fn graph_version(&self) -> Option<u64> {
        self.inner().player.as_ref().and_then(Player::graph_version)
    }

    // -- lifecycle -------------------------------------------------------------------------

    /// `shutdown()`: finishes a running recording (its takes are added), closes the MIDI
    /// inputs, stops the player's threads and the device, and releases the backend's context.
    /// The player and the scan thread are dropped outside every lock (see the module docs).
    pub fn shutdown(&self) {
        let (player, scanner) = {
            let mut inner = self.inner();
            if inner.player.as_ref().is_some_and(Player::is_recording)
                && let Err(error) = self.stop_locked(&mut inner)
            {
                log::error!("could not finish the recording on shutdown: {error}");
            }
            close_hub(&mut inner);
            (inner.player.take(), inner.scanner.take())
        };
        drop(scanner);
        drop(player);
        if let Some(engine) = &self.engine {
            lock(engine).shutdown();
        }
    }

    // -- helpers ---------------------------------------------------------------------------

    fn inner(&self) -> MutexGuard<'_, Inner> {
        lock(&self.shared.inner)
    }

    /// The player, started on first use (`_ensure_player`).
    fn player<'a>(&self, inner: &'a mut Inner) -> AppResult<&'a Player> {
        let engine = self.require_backend()?;
        if inner.player.is_none() {
            let audio = self.shared.parts.settings().audio.clone();
            let parts = &self.shared.parts;
            // On a real device the render thread joins the output device's audio
            // workgroup, or raises its QoS class.
            let render_start = render_thread_start(&lock(engine), &audio);
            let player = Player::with_render_start(
                PlayerSetup {
                    registry: Arc::clone(&parts.plugins),
                    device: Box::new(EngineDevice {
                        engine: Arc::clone(engine),
                    }),
                    source: Arc::new(SessionSource {
                        session: parts.session.clone(),
                        media: Arc::clone(&parts.media),
                    }),
                    settings: PlayerSettings {
                        lookahead_ms: audio.lookahead_ms,
                        live_lookahead_ms: audio.live_lookahead_ms,
                        buffer_frames: u32::try_from(audio.buffer_frames).unwrap_or(0),
                        output_device: audio.output_device,
                        input_device: audio.input_device,
                    },
                },
                render_start,
            )?;
            inner.live_lookahead_ms = audio.live_lookahead_ms;
            inner.player = Some(player);
        }
        inner
            .player
            .as_ref()
            .ok_or_else(|| DomainError::new(ErrorCode::Internal, "no player").into())
    }

    /// The open project as the player plays it.
    fn snapshot(&self) -> AppResult<ProjectSnapshot> {
        let parts = &self.shared.parts;
        Ok(snapshot(&parts.session, &parts.media)
            .ok_or_else(|| DomainError::new(ErrorCode::NoProject, "no project is open"))?)
    }
}

impl Drop for TransportService {
    /// Stops the scan thread and the player outside the service's lock (see the module docs);
    /// a running recording's files are finished but not added (use [`Self::shutdown`]).
    fn drop(&mut self) {
        let (player, scanner) = {
            let mut inner = self.inner();
            close_hub(&mut inner);
            (inner.player.take(), inner.scanner.take())
        };
        drop(scanner);
        drop(player);
    }
}

impl TransportParts {
    fn settings(&self) -> MutexGuard<'_, Settings> {
        lock(&self.settings)
    }
}

/// Closes the hub and takes its inputs back from the player.
fn close_hub(inner: &mut Inner) {
    if let Some(mut hub) = inner.hub.take() {
        hub.close();
        if let Some(player) = &inner.player {
            let mut closed = hub.take_closed();
            closed.extend(player.midi_ports());
            closed.sort_unstable();
            closed.dedup();
            player.remove_midi_inputs(&closed);
        }
    }
}

impl Shared {
    /// Hands the hub's new inputs to the player and takes its closed ones back.
    fn sync_inputs(&self, inner: &mut Inner) {
        let (Some(hub), Some(player)) = (inner.hub.as_mut(), inner.player.as_ref()) else {
            return;
        };
        let closed = hub.take_closed();
        if !closed.is_empty() {
            player.remove_midi_inputs(&closed);
        }
        let inputs: Vec<_> = hub
            .take_inputs()
            .into_iter()
            .map(|input| (input.port, input.events))
            .collect();
        let snapshot = snapshot(&self.parts.session, &self.parts.media);
        let prepared = if inputs.is_empty() {
            match &snapshot {
                Some(snapshot) => player.prepare_live(snapshot),
                None => Ok(()),
            }
        } else {
            player.add_midi_inputs(inputs, snapshot.as_ref())
        };
        if let Err(error) = prepared {
            log::warn!("live MIDI input cannot sound yet: {error}");
        }
    }

    /// One tick of the scan thread: rescans the hub's ports when due.
    fn scan(&self, now: Instant) {
        let mut inner = lock(&self.inner);
        let rescanned = inner.hub.as_mut().is_some_and(|hub| hub.rescan_if_due(now));
        if rescanned {
            self.sync_inputs(&mut inner);
        }
    }
}

/// The `sonara-midi-scan` thread. Dropping it stops and joins the thread; never drop it while
/// holding the service's lock, which the thread takes.
struct Scanner {
    stop: Option<Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl Scanner {
    fn spawn(shared: Weak<Shared>) -> std::io::Result<Self> {
        let (stop, stopped) = crossbeam_channel::bounded::<()>(0);
        let handle = thread::Builder::new()
            .name("sonara-midi-scan".to_owned())
            .spawn(move || {
                while let Err(RecvTimeoutError::Timeout) = stopped.recv_timeout(SCAN_TICK) {
                    let Some(shared) = shared.upgrade() else {
                        break;
                    };
                    shared.scan(Instant::now());
                }
            })?;
        Ok(Self {
            stop: Some(stop),
            handle: Some(handle),
        })
    }
}

impl Drop for Scanner {
    fn drop(&mut self) {
        // Dropping the sender wakes the thread at once.
        drop(self.stop.take());
        if let Some(handle) = self.handle.take() {
            drop(handle.join());
        }
    }
}
