//! The application composition root (`sonara.application.context`), shared by the GUI, the
//! CLI, the IPC server and the MCP server.
//!
//! There is exactly one project session per [`AppContext`]; every interface edits the project
//! through it, so GUI and agent edits share state and one undo history.
//!
//! The media service ([`AppContext::media`]) imports media and keeps the reader factory bound to
//! the open workspace (`media_provider`), which it rebinds on the session's `loaded`, `closed`
//! and `saved` events. It also provides the [`MediaProbe`] that [`ProjectService::relink`]
//! needs ([`SoundFileProbe`]; [`AppOptions::media_probe`] replaces it), and finds `FFmpeg` for
//! the import fallback ([`AppOptions::ffmpeg`]).
//!
//! The render service ([`AppContext::render`]) renders previews, analyzes and exports; it submits
//! its work to [`AppContext::jobs`] and reads media through [`MediaService::provider`].
//!
//! The MIDI devices ([`AppContext::midi_devices`]) list the MIDI ports and open inputs for the
//! MIDI hub ([`MidiHub`](crate::midi::MidiHub)); [`AppOptions::midi_devices`] replaces them.
//!
//! The transport ([`AppContext::transport`]) is created on first use, because it owns the audio
//! device; its backend is [`AppContext::audio_backend`], and [`AppContext::close`] shuts it
//! down. It owns the MIDI hub, as the prototype's player does.

use std::fmt;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::model::JsonMap;
use sonara_core::model::track::TrackKind;
use sonara_core::plugins::PluginRegistry;
use sonara_core::session::ProjectSession;
use sonara_dsp::processor::{EventKind, InstrumentEvent};
use sonara_project::store::ProjectStore;

use crate::clock::{LocalZone, WallClock, system_clock};
use crate::error::{AppError, AppResult};
use crate::ids::IdSource;
use crate::instruments::InstalledInstruments;
use crate::jobs::{DEFAULT_MAX_PENDING, DEFAULT_WORKERS, JobManager};
use crate::media::{FfmpegLocator, MediaService, SoundFileProbe, system_ffmpeg};
use crate::midi::{MidiDevices, SystemMidiDevices};
use crate::paths::workspaces_dir;
use crate::peaks::PeakCache;
use crate::projects::{MediaProbe, ProjectService};
use crate::render::RenderService;
use crate::samples::registry_for;
use crate::session::SharedSession;
use crate::settings::{Settings, load_settings, settings_file};
use crate::sync::lock;
use crate::transport::{TransportParts, TransportService};

/// A source of seconds for undo merging (see `ProjectSession::with_options`).
pub type UndoClock = Arc<dyn Fn() -> f64 + Send + Sync>;

/// How [`AppContext::create`] builds the context (the prototype's `AppOptions`, plus the
/// sources that the prototype replaces globally in golden runs).
#[derive(Clone)]
pub struct AppOptions {
    /// The root of scratch and extraction workspaces (default: [`workspaces_dir`]).
    pub workspaces: Option<PathBuf>,
    /// The settings to use instead of loading them.
    pub settings: Option<Settings>,
    /// Whether changes to the recent projects are saved to the settings file.
    pub persist_settings: bool,
    /// Background worker threads.
    pub job_workers: NonZeroUsize,
    /// Overrides `settings.audio.backend`.
    pub audio_backend: Option<String>,
    /// The settings file to load and save (default: `settings_path()`). Rust addition: the
    /// prototype's tests redirect it through `SONARA_HOME`.
    pub settings_path: Option<PathBuf>,
    /// The ID source of every command and operation (default: random IDs). Rust addition:
    /// golden runs use sequential IDs.
    pub ids: Option<IdSource>,
    /// The wall clock of time stamps in projects, backups, workspaces, preview names and
    /// exported files (default: the system clock). Rust addition: golden runs freeze it at
    /// 2026-01-01T12:00:00Z.
    pub wall_clock: Option<WallClock>,
    /// The time zone of local time stamps, the names of previews (default: the system's).
    /// Rust addition: golden runs use UTC, as the recorder's frozen `time.strftime` does.
    pub local_zone: LocalZone,
    /// The clock of undo merging (default: monotonic). Rust addition: golden runs pin it.
    pub undo_clock: Option<UndoClock>,
    /// Audio file probing for relinking media (default: [`SoundFileProbe`]).
    pub media_probe: Option<Arc<dyn MediaProbe>>,
    /// The MIDI ports (default: [`SystemMidiDevices`], the system's through midir). Rust
    /// addition: the golden replay pins the recording host's ports, as the prototype's recorder
    /// patched `input_ports` and `output_ports`.
    pub midi_devices: Option<Arc<dyn MidiDevices>>,
    /// How imports find `FFmpeg` (default: [`system_ffmpeg`], `ffmpeg` on `PATH` at every
    /// import, the prototype's `ffmpeg_path`). Rust addition: golden runs pin it to absent, as
    /// the prototype's recorder patched `ffmpeg_path`.
    pub ffmpeg: Option<FfmpegLocator>,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            workspaces: None,
            settings: None,
            persist_settings: true,
            job_workers: DEFAULT_WORKERS,
            audio_backend: None,
            settings_path: None,
            ids: None,
            wall_clock: None,
            local_zone: LocalZone::System,
            undo_clock: None,
            media_probe: None,
            midi_devices: None,
            ffmpeg: None,
        }
    }
}

impl fmt::Debug for AppOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppOptions")
            .field("workspaces", &self.workspaces)
            .field("settings", &self.settings)
            .field("persist_settings", &self.persist_settings)
            .field("job_workers", &self.job_workers)
            .field("audio_backend", &self.audio_backend)
            .field("settings_path", &self.settings_path)
            .field("ids", &self.ids)
            .field("wall_clock", &self.wall_clock.is_some())
            .field("local_zone", &self.local_zone)
            .field("undo_clock", &self.undo_clock.is_some())
            .field("media_probe", &self.media_probe.is_some())
            .field("midi_devices", &self.midi_devices.is_some())
            .field("ffmpeg", &self.ffmpeg.is_some())
            .finish()
    }
}

/// The application: plugins, the one session, the store, settings, jobs, and the project,
/// media and render services.
pub struct AppContext {
    options: AppOptions,
    plugins: Arc<PluginRegistry>,
    /// The instruments the sampler offers and plays, read once at startup.
    instruments: Arc<InstalledInstruments>,
    session: SharedSession,
    store: ProjectStore,
    settings: Arc<Mutex<Settings>>,
    jobs: JobManager,
    projects: ProjectService,
    media: Arc<MediaService>,
    render: RenderService,
    midi_devices: Arc<dyn MidiDevices>,
    ids: IdSource,
    /// The waveform pyramids of an interface; its workers start with the first request.
    peaks: PeakCache,
    /// The transport, once created.
    transport: Mutex<Option<Arc<TransportService>>>,
}

impl fmt::Debug for AppContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppContext")
            .field("options", &self.options)
            .field("session", &self.session)
            .field("store", &self.store)
            .field("jobs", &self.jobs)
            .finish_non_exhaustive()
    }
}

impl AppContext {
    /// The context `options` describe: settings given or loaded, the built-in plugins, a
    /// session with the settings' undo limit, a store keeping the settings' backups, and the
    /// job workers.
    ///
    /// # Errors
    ///
    /// - [`AppError::NoHome`] when a default directory is needed (workspaces, the settings
    ///   file to load or to persist to) and the home directory is unknown;
    /// - [`AppError::Workers`] when a worker thread cannot be started.
    pub fn create(options: AppOptions) -> AppResult<Self> {
        let settings = match &options.settings {
            Some(settings) => settings.clone(),
            None => load_settings(&settings_file(options.settings_path.as_deref())?),
        };
        let persist_to = if options.persist_settings {
            Some(settings_file(options.settings_path.as_deref())?)
        } else {
            None
        };
        let workspaces = match &options.workspaces {
            Some(dir) => dir.clone(),
            None => workspaces_dir()?,
        };
        let ids = options.ids.clone().unwrap_or_default();
        let wall_clock = options.wall_clock.clone().unwrap_or_else(system_clock);
        // One snapshot of the sample libraries gives the sampler's choices and the resolver of
        // the media provider, so they always agree.
        let instruments = Arc::new(InstalledInstruments::from_settings(&settings));
        let plugins = Arc::new(registry_for(&instruments.ids()));
        // Python's limits behave as 0 when negative.
        let undo_limit = usize::try_from(settings.undo_limit).unwrap_or(0);
        let undo_clock: sonara_core::session::Clock = if let Some(clock) = &options.undo_clock {
            let clock = Arc::clone(clock);
            Box::new(move || clock())
        } else {
            // Monotonic seconds; merging compares differences only.
            let start = Instant::now();
            Box::new(move || start.elapsed().as_secs_f64())
        };
        let session = SharedSession::new(ProjectSession::with_options(
            Arc::clone(&plugins),
            undo_limit,
            undo_clock,
        ));
        let store_clock = Arc::clone(&wall_clock);
        let store = ProjectStore::new(workspaces)
            .with_backups(usize::try_from(settings.backup_retention).unwrap_or(0))
            .with_clock(move || store_clock());
        let jobs = JobManager::new(options.job_workers, DEFAULT_MAX_PENDING, ids.clone())
            .map_err(AppError::Workers)?;
        let settings = Arc::new(Mutex::new(settings));
        let mut projects = ProjectService::new(
            session.clone(),
            store.clone(),
            Arc::clone(&settings),
            ids.clone(),
        )
        .with_clock(Arc::clone(&wall_clock));
        if let Some(path) = persist_to {
            projects = projects.persisting_settings(path);
        }
        let probe: Arc<dyn MediaProbe> = match &options.media_probe {
            Some(probe) => Arc::clone(probe),
            None => Arc::new(SoundFileProbe),
        };
        projects = projects.with_media_probe(probe);
        let ffmpeg: FfmpegLocator = match &options.ffmpeg {
            Some(locator) => Arc::clone(locator),
            None => Arc::new(system_ffmpeg),
        };
        let media = Arc::new(
            MediaService::new(session.clone(), jobs.clone(), ids.clone())
                .with_ffmpeg(ffmpeg)
                .with_instruments(Arc::clone(&instruments)),
        );
        let render = RenderService::new(
            session.clone(),
            Arc::clone(&plugins),
            Arc::clone(&media),
            jobs.clone(),
            ids.clone(),
            wall_clock,
            options.local_zone,
        );
        // Lazy: nothing touches the MIDI system until a port is listed or opened.
        let midi_devices: Arc<dyn MidiDevices> = match &options.midi_devices {
            Some(devices) => Arc::clone(devices),
            None => Arc::new(SystemMidiDevices),
        };
        let peaks = PeakCache::new(Arc::clone(&media), session.clone());
        Ok(Self {
            options,
            plugins,
            instruments,
            session,
            store,
            settings,
            jobs,
            projects,
            media,
            render,
            midi_devices,
            ids,
            peaks,
            transport: Mutex::new(None),
        })
    }

    /// The options the context was created with.
    #[must_use]
    pub fn options(&self) -> &AppOptions {
        &self.options
    }

    /// The plugin registry.
    #[must_use]
    pub fn plugins(&self) -> &Arc<PluginRegistry> {
        &self.plugins
    }

    /// The instruments of the configured sample libraries that the sampler offers and plays,
    /// as they were when the context was created.
    #[must_use]
    pub fn instruments(&self) -> &Arc<InstalledInstruments> {
        &self.instruments
    }

    /// Rewrites a patch path given as the `instrument` of a sampler to its preset ID
    /// ([`InstalledInstruments::normalize_params`]) when `device_id` is a sampler of the open
    /// project. Without a project or such a device nothing changes; the edit that follows
    /// reports that.
    pub fn normalize_device_params(&self, device_id: &str, values: &mut JsonMap) {
        if !values.contains_key("instrument") {
            return;
        }
        let plugin_id = {
            let session = self.session.lock();
            let Ok(project) = session.require_project() else {
                return;
            };
            let Ok((_, device)) = project.device(device_id) else {
                return;
            };
            device.plugin_id.clone()
        };
        self.instruments.normalize_params(&plugin_id, values);
    }

    /// The one project session.
    #[must_use]
    pub fn session(&self) -> &SharedSession {
        &self.session
    }

    /// The project store.
    #[must_use]
    pub fn store(&self) -> &ProjectStore {
        &self.store
    }

    /// The settings, locked until the guard drops. Never hold the guard while calling the
    /// project service.
    pub fn settings(&self) -> MutexGuard<'_, Settings> {
        lock(&self.settings)
    }

    /// The background jobs.
    #[must_use]
    pub fn jobs(&self) -> &JobManager {
        &self.jobs
    }

    /// The project lifecycle service.
    #[must_use]
    pub fn projects(&self) -> &ProjectService {
        &self.projects
    }

    /// The media service: imports and the reader factory of the open workspace. It is shared,
    /// so that a job can hold it ([`MediaService::import_audio_job`]).
    #[must_use]
    pub fn media(&self) -> &Arc<MediaService> {
        &self.media
    }

    /// The render service: previews, track levels, analysis, exports and stems.
    #[must_use]
    pub fn render(&self) -> &RenderService {
        &self.render
    }

    /// The MIDI devices: the port listings and the inputs of a MIDI hub.
    #[must_use]
    pub fn midi_devices(&self) -> &Arc<dyn MidiDevices> {
        &self.midi_devices
    }

    /// The ID source every interface builds its commands with, shared with the jobs.
    #[must_use]
    pub fn ids(&self) -> IdSource {
        self.ids.clone()
    }

    /// The waveform peak pyramids.
    #[must_use]
    pub fn peaks(&self) -> &PeakCache {
        &self.peaks
    }

    /// The audio backend the transport will use: the option, else the settings'.
    #[must_use]
    pub fn audio_backend(&self) -> String {
        self.options
            .audio_backend
            .clone()
            .unwrap_or_else(|| self.settings().audio.backend.clone())
    }

    /// The transport (`AppContext.transport`), created on first use with
    /// [`audio_backend`](Self::audio_backend): creating it opens the backend's context.
    #[must_use]
    pub fn transport(&self) -> Arc<TransportService> {
        let mut slot = lock(&self.transport);
        if let Some(transport) = slot.as_ref() {
            return Arc::clone(transport);
        }
        let parts = TransportParts {
            session: self.session.clone(),
            plugins: Arc::clone(&self.plugins),
            media: Arc::clone(&self.media),
            settings: Arc::clone(&self.settings),
            midi_devices: Arc::clone(&self.midi_devices),
            ids: self.ids.clone(),
        };
        let transport = Arc::new(TransportService::new(parts, &self.audio_backend()));
        *slot = Some(Arc::clone(&transport));
        transport
    }

    /// Whether the transport was created (`has_transport`): `stop_playback` does not create it.
    #[must_use]
    pub fn has_transport(&self) -> bool {
        lock(&self.transport).is_some()
    }

    /// The prototype main window's `_update_midi_focus`: the instrument track
    /// `track_id` takes live MIDI input besides the armed tracks; any other track, an unknown
    /// one or `None` clears the focus. Without a transport it does nothing, and it never creates
    /// one.
    pub fn focus_midi(&self, track_id: Option<&str>) {
        if !self.has_transport() {
            return;
        }
        let focus = track_id.and_then(|id| {
            let project = self.session.project()?;
            let track = project.find_track(id)?;
            (track.kind == TrackKind::Instrument).then(|| track.id.clone())
        });
        self.transport().set_midi_focus(focus);
    }

    /// Plays (`on: true`) or releases one preview note of `track_id`'s instrument, for a key
    /// clicked in the piano roll's keyboard: the user hears which pitch a row is. Opens the
    /// device and builds a graph on first use, like [`AppContext::transport`].
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` for a pitch past 127 (`on`) or a velocity outside 1–127 (`on`);
    /// `NOT_FOUND` for an unknown track, or one that is not an instrument track; the errors of
    /// [`focus_midi`](Self::focus_midi)'s transport, `NO_PROJECT` and `ENGINE_UNAVAILABLE`
    /// included.
    pub fn audition_note(
        &self,
        track_id: &str,
        pitch: u8,
        velocity: u8,
        on: bool,
    ) -> AppResult<()> {
        if pitch > 127 || (on && !(1..=127).contains(&velocity)) {
            return Err(DomainError::new(
                ErrorCode::InvalidArgument,
                "audition_note needs a pitch and velocity of 0-127",
            )
            .into());
        }
        let project = self
            .session
            .project()
            .ok_or_else(|| DomainError::new(ErrorCode::NoProject, "no project is open"))?;
        let track = project
            .find_track(track_id)
            .ok_or_else(|| DomainError::new(ErrorCode::NotFound, format!("no track {track_id}")))?;
        if track.kind != TrackKind::Instrument {
            return Err(DomainError::new(
                ErrorCode::InvalidArgument,
                format!("track {track_id} is not an instrument track"),
            )
            .into());
        }
        let event = InstrumentEvent {
            offset: 0,
            kind: if on {
                EventKind::NoteOn
            } else {
                EventKind::NoteOff
            },
            pitch,
            velocity: if on { velocity } else { 0 },
            channel: 0,
            value: 0,
            number: 0,
            articulation: None,
        };
        self.transport().audition_note(track_id, event)
    }

    /// Shuts the transport down (a running recording is finished first), stops the background
    /// jobs without waiting for them (queued jobs end cancelled), stops the peak workers and
    /// closes the media readers.
    pub fn close(&self) {
        let transport = lock(&self.transport).take();
        if let Some(transport) = transport {
            transport.shutdown();
        }
        self.jobs.shutdown(false);
        self.peaks.close();
        self.media.close();
    }
}
