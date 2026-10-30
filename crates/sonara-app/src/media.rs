//! Importing audio and MIDI into the open project, and the media reader factory bound to the open
//! workspace (`sonara.application.services.media_service`, and `media_provider` of
//! `sonara.application.context`).
//!
//! **Imports.** An import brings a file into the project and records it with commands, as one
//! undo step: audio (`Import Audio`: the asset, a new audio track unless one is given, and a
//! clip), impulse responses (`Load Impulse Response`: the asset and the convolver's state) and
//! Standard MIDI Files (`Import MIDI`: the tempo map, meter and markers, and one instrument track
//! with one clip per MIDI track and channel). The commands run as the prototype's service runs
//! them, with the session's default source `user`.
//!
//! **IDs** are drawn in the prototype's order, which the golden transcripts record: an import's
//! asset ID before its file is inspected (a file that cannot be imported still uses one), a new
//! track's ID and then its instrument's ID, then the clip's, and a decode job's operation ID
//! last. Reading a MIDI file draws its note IDs, also for a dry run.
//!
//! **Decoding.** Media the player cannot read directly (compressed, or at another rate than the
//! project) is decoded into the workspace's cache by a background `decode` job
//! ([`crate::jobs::JobManager`]); the import reports the job's operation ID.
//!
//! **The reader factory.** [`MediaService::provider`] is the [`MediaProvider`] of the open
//! workspace: media resolves against it, and the decode cache lives in its `cache/decoded`. When
//! the workspace changes (a project is opened, closed, or saved somewhere else), the provider is
//! rebuilt. The service subscribes to the session for `loaded`, `closed` and `saved` events, as
//! the prototype does; the session delivers them through a channel, so they are handled when the
//! provider is next asked for (it closes the old provider's readers, then binds a new one).
//!
//! **Locks.** The session lock is held only to copy out a snapshot or to run one command, never
//! across file I/O, and never while the provider binding is locked.
//!
//! **The `FFmpeg` fallback**: a file the media crate cannot
//! read is inspected with `ffprobe` and transcoded losslessly to FLAC by an external `ffmpeg`
//! process, when `ffmpeg` is on `PATH` ([`sonara_media::Ffmpeg`]). The FLAC file becomes the
//! asset, the original is copied to `media/{audio|impulses}/originals/`, and the asset's
//! metadata names both, as in the prototype; the FLAC file then decodes into the cache like any
//! other. `FFmpeg` is looked up at every import ([`FfmpegLocator`], [`MediaService::ffmpeg`]).

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use sonara_core::commands::Command;
use sonara_core::commands::Transaction;
use sonara_core::commands::clip::{AddAudioClip, CreateMidiClip};
use sonara_core::commands::device::SetDeviceState;
use sonara_core::commands::project::{AddAsset, AddMarker, SetTempo, SetTimeSignature};
use sonara_core::commands::track::CreateTrack;
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::ids::{self, IdGenerator as _, MASTER_ID};
use sonara_core::model::JsonMap;
use sonara_core::model::media::{MediaAsset, MediaAssetData, MediaKind, MediaLocation};
use sonara_core::model::project::Project;
use sonara_core::model::track::TrackKind;
use sonara_core::pyfmt::str_repr;
use sonara_core::session::{EventKind, ProjectLocation, SessionEvent};
use sonara_media::{
    AudioFileInfo, MediaError, MediaProvider, check_import_size, import_path, import_warnings,
    inspect_audio_with, path_stem, path_suffix, sound_info, transcode_paths, transcode_warning,
};
use sonara_midi::smf::{ImportedMidi, PPQ, clip_from_imported, export_smf, import_smf};
use sonara_project::atomic::{atomic_copy, sha256_file};
use sonara_project::store::asset_path;

use crate::error::{AppError, AppResult, IoContext as _};
use crate::events::ErrorNotice;
use crate::ids::IdSource;
use crate::instruments::InstalledInstruments;
use crate::jobs::{Job, JobManager};
use crate::paths::{expand_user, resolve};
use crate::projects::{MediaProbe, ProbedAudio};
use crate::session::SharedSession;
use crate::sync::lock;

pub use sonara_media::Ffmpeg;

/// Finds `FFmpeg` for the import fallback; `None` when it is not installed.
pub type FfmpegLocator = Arc<dyn Fn() -> Option<Ffmpeg> + Send + Sync>;

/// `ffmpeg_path()`: `ffmpeg` (and `ffprobe`) on the process's `PATH`.
#[must_use]
pub fn system_ffmpeg() -> Option<Ffmpeg> {
    Ffmpeg::discover()
}

/// Describes the audio file at `path`, with `~` expanded (`inspect_audio`): what an import sees,
/// and what `sonara info` prints for a file that is not a project. A file the media crate cannot
/// read goes to `ffprobe` when `ffmpeg` is on `PATH`.
///
/// # Errors
///
/// The errors of [`inspect_audio_with`]: `NOT_FOUND`, `UNSUPPORTED`, and the `FFmpeg`
/// fallback's.
pub fn inspect_file(path: &Path) -> Result<AudioFileInfo, DomainError> {
    inspect_audio_with(&expand_user(path), system_ffmpeg().as_ref())
}

/// The job kind of an interface's audio import ([`MediaService::import_audio_job`]).
pub const IMPORT_AUDIO_JOB: &str = "import_audio";

/// The audio file suffixes an interface imports: the timeline's and the window's drops (TL-09,
/// MW-24), the Import Audio panels and the browser's Files tab (BR-04,
/// [`crate::browser::list_directory`]) take the same ones, the prototype canvas's
/// `AUDIO_EXTENSIONS`, lower case (the
/// prototype's browser, file panel and window drop lacked `.rf64` and `.aac`).
pub const AUDIO_IMPORT_SUFFIXES: [&str; 14] = [
    ".wav", ".wave", ".aif", ".aiff", ".flac", ".mp3", ".ogg", ".oga", ".opus", ".w64", ".rf64",
    ".caf", ".m4a", ".aac",
];

/// Whether `path`'s suffix (Python's `Path.suffix`, any case) is one of
/// [`AUDIO_IMPORT_SUFFIXES`].
#[must_use]
pub fn is_audio_import(path: &Path) -> bool {
    let suffix = path_suffix(&path.to_string_lossy()).to_lowercase();
    AUDIO_IMPORT_SUFFIXES.contains(&suffix.as_str())
}

/// What a finished import job reports in the status bar, one message per file as the
/// prototype's timeline drop did: "Imported name" and each import warning, or "Import of name
/// failed: message" as an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReport {
    /// The messages, in the order of the files.
    pub messages: Vec<ImportMessage>,
}

/// One status-bar line of an [`ImportReport`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportMessage {
    /// The text.
    pub text: String,
    /// Whether it reports a failure.
    pub error: bool,
}

impl ImportReport {
    /// The report of an import job's result (`{"imported": […], "failed": […]}`); files are
    /// reported in the order they were imported, the failures after them.
    #[must_use]
    pub fn of(result: &Value) -> Self {
        let mut messages = Vec::new();
        let text = |value: &Value, key: &str| value[key].as_str().unwrap_or_default().to_owned();
        for file in result["imported"].as_array().into_iter().flatten() {
            messages.push(ImportMessage {
                text: format!("Imported {}", text(file, "file")),
                error: false,
            });
            for warning in file["warnings"].as_array().into_iter().flatten() {
                if let Some(warning) = warning.as_str() {
                    messages.push(ImportMessage {
                        text: warning.to_owned(),
                        error: false,
                    });
                }
            }
        }
        for file in result["failed"].as_array().into_iter().flatten() {
            messages.push(ImportMessage {
                text: format!(
                    "Import of {} failed: {}",
                    text(file, "file"),
                    text(file, "message")
                ),
                error: true,
            });
        }
        Self { messages }
    }
}

/// The plugin an impulse response loads into.
const CONVOLVER: &str = "sonara.convolver";
/// The longest impulse response the convolver uses, in seconds.
const MAX_IMPULSE_SECONDS: f64 = 30.0;
/// The instrument and preset of an imported MIDI track on the drum channel.
const DRUM_INSTRUMENT: (&str, &str) = ("sonara.drums", "Punchy 909");
/// The instrument and preset of every other imported MIDI track.
const KEYS_INSTRUMENT: (&str, &str) = ("sonara.synth.poly", "Electric Keys");
/// The longest track name an audio import derives from the file name, in characters.
const AUDIO_TRACK_NAME_CHARS: usize = 60;
/// The longest track name a MIDI import takes from the file, in characters.
const MIDI_TRACK_NAME_CHARS: usize = 120;

/// How an audio file comes into the project (`audio_import.ImportMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportMode {
    /// Copied into the project's `media/` (embedded).
    #[default]
    Copy,
    /// Referenced where it is (linked).
    Link,
}

/// What [`MediaService::import_audio`] does besides bringing the file in.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AudioImport {
    /// The audio track that receives the clip; `None` creates one.
    pub track_id: Option<String>,
    /// Where the clip starts, in beats.
    pub start_beat: f64,
    /// Copy or link the file.
    pub mode: ImportMode,
    /// The name of a new track (default: the file's stem).
    pub track_name: Option<String>,
    /// Whether a clip (and, without `track_id`, a track) is created.
    pub create_clip: bool,
    /// Only inspect the file and report what would happen.
    pub dry_run: bool,
}

/// Reads the facts of an audio file for relinking ([`MediaProbe`]) with `sonara-media`'s
/// inspection (the prototype's `soundfile.info`).
#[derive(Debug, Clone, Copy, Default)]
pub struct SoundFileProbe;

impl MediaProbe for SoundFileProbe {
    fn probe(&self, path: &Path) -> AppResult<ProbedAudio> {
        let info = sound_info(path).map_err(media_error)?;
        Ok(ProbedAudio {
            frames: info.frames,
            sample_rate: info.sample_rate,
            channels: info.channels,
        })
    }
}

/// A media error as the prototype's `SonaraError` (code, message and details).
#[expect(
    clippy::needless_pass_by_value,
    reason = "used as `map_err(media_error)`, which hands over the error"
)]
fn media_error(error: MediaError) -> AppError {
    AppError::Domain(error.to_domain_error())
}

/// The provider bound to one workspace (`None`: no project open).
struct Binding {
    workspace: Option<PathBuf>,
    provider: Arc<MediaProvider>,
}

/// Imports media into the session's project and keeps the reader factory of its workspace.
pub struct MediaService {
    session: SharedSession,
    jobs: JobManager,
    ids: IdSource,
    ffmpeg: FfmpegLocator,
    instruments: Arc<InstalledInstruments>,
    binding: Mutex<Option<Binding>>,
    events: Mutex<Receiver<SessionEvent>>,
}

impl fmt::Debug for MediaService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let workspace = lock(&self.binding)
            .as_ref()
            .map(|binding| binding.workspace.clone());
        f.debug_struct("MediaService")
            .field("bound_workspace", &workspace)
            .finish_non_exhaustive()
    }
}

impl MediaService {
    /// A service importing into `session`'s project, decoding media with `jobs` and drawing
    /// IDs from `ids`, with `FFmpeg` from `PATH` ([`system_ffmpeg`]). It subscribes to the
    /// session's events.
    #[must_use]
    pub fn new(session: SharedSession, jobs: JobManager, ids: IdSource) -> Self {
        let events = session.lock().subscribe();
        Self {
            session,
            jobs,
            ids,
            ffmpeg: Arc::new(system_ffmpeg),
            instruments: Arc::default(),
            binding: Mutex::new(None),
            events: Mutex::new(events),
        }
    }

    /// The same service, finding `FFmpeg` with `locator`.
    #[must_use]
    pub fn with_ffmpeg(mut self, locator: FfmpegLocator) -> Self {
        self.ffmpeg = locator;
        self
    }

    /// The same service, whose providers play the sampler instruments of `instruments`
    /// (without it, no sampler instrument resolves).
    #[must_use]
    pub fn with_instruments(mut self, instruments: Arc<InstalledInstruments>) -> Self {
        self.instruments = instruments;
        self
    }

    /// The `FFmpeg` tools the import fallback would use now; `None` when `FFmpeg` is not installed
    /// (`ffmpeg_path()`, which `get_capabilities` reports as `ffmpeg_import`).
    #[must_use]
    pub fn ffmpeg(&self) -> Option<Ffmpeg> {
        (self.ffmpeg)()
    }

    // -- the reader factory ---------------------------------------------------------------

    /// The reader factory bound to the open project's workspace (`media_provider`): assets
    /// resolve against the workspace, and the decode cache is its `cache/decoded`. Without a
    /// project, asset paths are taken as they are and there is no cache. A sampler's
    /// `instrument` resolves through the service's installed instruments
    /// ([`InstalledInstruments::resolve`]). The provider is rebuilt when the workspace changed
    /// since the last call; the old one's readers are closed.
    pub fn provider(&self) -> Arc<MediaProvider> {
        self.handle_events();
        let workspace = self.workspace();
        let mut binding = lock(&self.binding);
        if let Some(bound) = binding.as_ref()
            && bound.workspace == workspace
        {
            return Arc::clone(&bound.provider);
        }
        if let Some(old) = binding.take() {
            old.provider.close();
        }
        let provider = Arc::new(bind(workspace.clone(), &self.instruments));
        *binding = Some(Binding {
            workspace,
            provider: Arc::clone(&provider),
        });
        provider
    }

    /// Closes the provider's readers and forgets it (application shutdown).
    pub fn close(&self) {
        if let Some(old) = lock(&self.binding).take() {
            old.provider.close();
        }
    }

    /// The session events since the last call: after a load, close or save that moved the
    /// workspace, the provider is closed and forgotten (`_on_session_event`).
    fn handle_events(&self) {
        let moved = {
            let events = lock(&self.events);
            // Every pending event is taken, so `filter` and `count` rather than `any`.
            events
                .try_iter()
                .filter(|event| {
                    matches!(
                        event.kind,
                        EventKind::Loaded | EventKind::Closed | EventKind::Saved
                    )
                })
                .count()
                > 0
        };
        if !moved {
            return;
        }
        let workspace = self.workspace();
        let mut binding = lock(&self.binding);
        if binding
            .as_ref()
            .is_some_and(|bound| bound.workspace != workspace)
            && let Some(old) = binding.take()
        {
            old.provider.close();
        }
    }

    /// The open project's workspace.
    fn workspace(&self) -> Option<PathBuf> {
        self.session
            .lock()
            .location()
            .map(|location| location.workspace.clone())
    }

    /// The open project and where it lives.
    fn snapshot(&self) -> AppResult<(Project, ProjectLocation)> {
        let session = self.session.lock();
        let project = session.require_project()?.clone();
        let location = session.require_location()?.clone();
        Ok((project, location))
    }

    /// Runs `commands` as one undo step labelled `name`, with the session's default source.
    fn execute(
        &self,
        commands: Vec<Box<dyn Command>>,
        name: &str,
    ) -> AppResult<sonara_core::session::CommandResult> {
        let transaction = Transaction {
            commands,
            name: name.to_owned(),
        };
        Ok(self.session.lock().execute(&transaction, false, "user")?)
    }

    // -- audio ----------------------------------------------------------------------------

    /// Describes an audio file as an import would see it (`inspect`), with `~` expanded and
    /// the `FFmpeg` fallback when `FFmpeg` is installed.
    ///
    /// # Errors
    ///
    /// The errors of [`inspect_audio_with`]: `NOT_FOUND`, `UNSUPPORTED`, and the `FFmpeg`
    /// fallback's.
    pub fn inspect(&self, path: &Path) -> Result<AudioFileInfo, DomainError> {
        inspect_audio_with(&expand_user(path), self.ffmpeg().as_ref())
    }

    /// Imports the audio file at `path` (`import_audio`): the asset and, by default, a clip at
    /// `options.start_beat` on `options.track_id` or on a new audio track, as one undo step.
    /// Media the player cannot read directly gets a background decode job.
    ///
    /// # Errors
    ///
    /// - [`ErrorCode::NoProject`];
    /// - [`ErrorCode::NotFound`] for an unknown track;
    /// - [`ErrorCode::InvalidArgument`]: `track '<name>' is not an audio track`;
    /// - the errors of inspecting, copying and transcoding the file ([`inspect_audio_with`],
    ///   [`check_import_size`], [`Ffmpeg::transcode_to_flac`], I/O errors);
    /// - the commands' errors, and those of submitting the decode job.
    pub fn import_audio(&self, path: &Path, options: &AudioImport) -> AppResult<Value> {
        let (project, location) = self.snapshot()?;
        if let Some(track_id) = &options.track_id {
            let track = project.track(track_id)?;
            if track.kind != TrackKind::Audio {
                return Err(DomainError::invalid(format!(
                    "track {} is not an audio track",
                    str_repr(&track.name)
                ))
                .into());
            }
        }
        if options.dry_run {
            let info = self.inspect(path)?;
            return Ok(json!({
                "dry_run": true,
                "file": info.to_json(),
                "would_create_track": options.track_id.is_none(),
            }));
        }
        let mut ids = self.ids.clone();
        let asset_id = MediaAsset::new_id(&mut ids);
        let (asset, info, warnings) = bring_in(
            path,
            &location.workspace,
            asset_id,
            options.mode,
            MediaKind::Audio,
            self.ffmpeg().as_ref(),
        )?;
        let stem = path_stem(&asset.name).to_owned();
        let mut commands: Vec<Box<dyn Command>> = vec![Box::new(AddAsset {
            asset: asset.clone(),
        })];
        let mut track_id = options.track_id.clone();
        let mut created_track = false;
        if options.create_clip && track_id.is_none() {
            let derived: String = stem.chars().take(AUDIO_TRACK_NAME_CHARS).collect();
            let name = options
                .track_name
                .clone()
                .filter(|name| !name.is_empty())
                .or_else(|| Some(derived).filter(|name| !name.is_empty()))
                .unwrap_or_else(|| "Audio".to_owned());
            let track = new_track(&mut ids, name, TrackKind::Audio, None);
            track_id = Some(track.track_id.clone());
            created_track = true;
            commands.push(Box::new(track));
        }
        let mut clip_id = None;
        if options.create_clip
            && let Some(track_id) = &track_id
        {
            let clip = AddAudioClip {
                track_id: track_id.clone(),
                asset_id: asset.id.clone(),
                start: options.start_beat,
                length: None,
                source_offset: 0,
                name: Some(stem),
                gain_db: 0.0,
                clip_id: ids.new_id(ids::CLIP),
            };
            clip_id = Some(clip.clip_id.clone());
            commands.push(Box::new(clip));
        }
        let result = self.execute(commands, "Import Audio")?;
        let decode = self.decode_job(&asset, project.settings.sample_rate)?;
        Ok(json!({
            "asset_id": asset.id,
            "track_id": track_id,
            "clip_id": clip_id,
            "created_track": created_track,
            "file": info.to_json(),
            "warnings": warnings,
            "decode_operation_id": decode,
            "changes": result.changes.to_json(),
            "project_version": result.version,
        }))
    }

    /// Imports the audio files `paths` one after another on a job worker: each
    /// as by [`import_audio`](Self::import_audio), copied into the project, with a clip at
    /// `start_beat` on the audio track `track_id`, or on a new track per file without one. The
    /// prototype imported a timeline drop on a thread of its own and the other imports on the
    /// GUI thread; here every import is a job with progress, so the window never waits.
    ///
    /// The job's progress message names the file being imported. A file that fails does not
    /// stop the others; the result lists both (`{"imported": [{"file", "asset_id",
    /// "track_id", "clip_id", "warnings"}], "failed": [{"file", "code", "message"}]}`), which
    /// [`ImportReport`] words for the status bar.
    ///
    /// # Errors
    ///
    /// The errors of submitting the job ([`JobManager::submit`]).
    pub fn import_audio_job(
        self: &Arc<Self>,
        paths: Vec<PathBuf>,
        track_id: Option<String>,
        start_beat: f64,
    ) -> AppResult<Job> {
        let label = match paths.as_slice() {
            [one] => format!("Import {}", file_name(one)),
            many => format!("Import {} files", many.len()),
        };
        let media = Arc::clone(self);
        self.jobs.submit(IMPORT_AUDIO_JOB, &label, move |ctx| {
            let mut imported = Vec::new();
            let mut failed = Vec::new();
            // Exact for any realistic number of files.
            #[expect(clippy::cast_precision_loss, reason = "a file count")]
            let total = paths.len().max(1) as f64;
            for (done, path) in paths.iter().enumerate() {
                ctx.check()?;
                let name = file_name(path);
                #[expect(clippy::cast_precision_loss, reason = "a file count")]
                ctx.progress(done as f64 / total, Some(&name));
                let options = AudioImport {
                    track_id: track_id.clone(),
                    start_beat,
                    mode: ImportMode::Copy,
                    create_clip: true,
                    ..AudioImport::default()
                };
                match media.import_audio(path, &options) {
                    Ok(result) => imported.push(json!({
                        "file": name,
                        "asset_id": result["asset_id"],
                        "track_id": result["track_id"],
                        "clip_id": result["clip_id"],
                        "warnings": result["warnings"],
                    })),
                    Err(error) => {
                        let notice = ErrorNotice::from(&error);
                        log::info!("import of {} failed: {}", path.display(), notice.message);
                        failed.push(json!({
                            "file": name,
                            "code": notice.code,
                            "message": notice.message,
                        }));
                    }
                }
            }
            let mut result = JsonMap::new();
            result.insert("imported".to_owned(), Value::Array(imported));
            result.insert("failed".to_owned(), Value::Array(failed));
            Ok(result)
        })
    }

    /// Submits the decode job of `asset` at `sample_rate` when the player cannot read it
    /// directly and its cache entry is missing; returns the job's operation ID.
    fn decode_job(&self, asset: &MediaAsset, sample_rate: u32) -> AppResult<Option<String>> {
        let provider = self.provider();
        if !MediaProvider::needs_cache(asset, sample_rate)
            || provider.cache_ready(asset, sample_rate)
        {
            return Ok(None);
        }
        let label = format!("Decode {}", asset.name);
        let asset = asset.clone();
        let job = self.jobs.submit("decode", &label, move |ctx| {
            let cancelled = || ctx.is_cancelled();
            provider
                .prepare(&asset, sample_rate, Some(&cancelled))
                .map_err(media_error)?;
            let mut result = JsonMap::new();
            result.insert("asset_id".to_owned(), Value::from(asset.id.as_str()));
            result.insert("decoded".to_owned(), Value::Bool(true));
            Ok(result)
        })?;
        Ok(Some(job.id().to_owned()))
    }

    /// Imports the impulse response at `path` into the convolution reverb `device_id`
    /// (`import_impulse_response`): the file is copied into `media/impulses/` and becomes the
    /// device's `ir_asset_id`, as one undo step.
    ///
    /// # Errors
    ///
    /// - [`ErrorCode::NoProject`];
    /// - [`ErrorCode::NotFound`] for an unknown device;
    /// - [`ErrorCode::InvalidArgument`]: `impulse responses load into the Convolution Reverb`;
    /// - the errors of inspecting and copying the file, and the commands' errors.
    pub fn import_impulse_response(&self, path: &Path, device_id: &str) -> AppResult<Value> {
        let (project, location) = self.snapshot()?;
        let (_, device) = project.device(device_id)?;
        if device.plugin_id != CONVOLVER {
            return Err(
                DomainError::invalid("impulse responses load into the Convolution Reverb").into(),
            );
        }
        let mut ids = self.ids.clone();
        let asset_id = MediaAsset::new_id(&mut ids);
        let (asset, info, mut warnings) = bring_in(
            path,
            &location.workspace,
            asset_id,
            ImportMode::Copy,
            MediaKind::Impulse,
            self.ffmpeg().as_ref(),
        )?;
        if info.duration_seconds() > MAX_IMPULSE_SECONDS {
            warnings
                .push("impulse response longer than 30 s; only the first 30 s are used".to_owned());
        }
        let mut state = JsonMap::new();
        state.insert("ir_asset_id".to_owned(), Value::from(asset.id.as_str()));
        let commands: Vec<Box<dyn Command>> = vec![
            Box::new(AddAsset {
                asset: asset.clone(),
            }),
            Box::new(SetDeviceState {
                device_id: device_id.to_owned(),
                values: state,
            }),
        ];
        let result = self.execute(commands, "Load Impulse Response")?;
        Ok(json!({
            "asset_id": asset.id,
            "device_id": device_id,
            "file": info.to_json(),
            "warnings": warnings,
            "project_version": result.version,
        }))
    }

    // -- MIDI files -----------------------------------------------------------------------

    /// Reads the Standard MIDI File at `path` without changing the project (the prototype's
    /// handler calls `import_smf` for a dry run). Its note IDs are drawn from the application's
    /// ID source, as the prototype draws them.
    ///
    /// # Errors
    ///
    /// The errors of [`import_smf`]: `NOT_FOUND`, `LIMIT_EXCEEDED`, `FORMAT_ERROR`,
    /// `UNSUPPORTED`, `INVALID_ARGUMENT`.
    pub fn read_midi(&self, path: &Path) -> AppResult<ImportedMidi> {
        let mut ids = self.ids.clone();
        Ok(import_smf(&mut ids, path)?)
    }

    /// Imports the Standard MIDI File at `path` (`import_midi`): with `apply_tempo`, the file's
    /// tempo map (without ramps), time signatures and markers (from `start_beat`); then one
    /// instrument track with one clip at `start_beat` per MIDI track and channel that has notes
    /// (the drum channel plays the drum kit), as one undo step.
    ///
    /// # Errors
    ///
    /// - the errors of [`import_smf`];
    /// - [`ErrorCode::InvalidArgument`]: `<file name> contains no notes`;
    /// - [`ErrorCode::NoProject`] and the commands' errors.
    pub fn import_midi(&self, path: &Path, apply_tempo: bool, start_beat: f64) -> AppResult<Value> {
        let mut ids = self.ids.clone();
        let imported = import_smf(&mut ids, &expand_user(path))?;
        let mut commands: Vec<Box<dyn Command>> = Vec::new();
        // `tempo_map.points[0].bpm`: a tempo map starts with a point at beat 0.
        let first_tempo = imported.tempo_map.bpm_at(0.0);
        if apply_tempo {
            commands.push(Box::new(SetTempo {
                bpm: first_tempo,
                beat: 0.0,
                ramp: false,
                clear_changes: true,
            }));
            for point in imported.tempo_map.points().iter().skip(1) {
                commands.push(Box::new(SetTempo {
                    bpm: point.bpm,
                    beat: point.beat,
                    ramp: false,
                    clear_changes: false,
                }));
            }
            for point in imported.time_signatures.points() {
                commands.push(Box::new(SetTimeSignature {
                    numerator: i64::from(point.numerator),
                    denominator: i64::from(point.denominator),
                    bar: point.bar,
                }));
            }
            for (beat, name) in &imported.markers {
                commands.push(Box::new(AddMarker {
                    beat: start_beat + beat,
                    name: name.clone(),
                    color: None,
                    marker_id: ids.new_id(ids::MARKER),
                }));
            }
        }
        let mut created = Vec::new();
        for track in imported.tracks.iter().filter(|t| !t.notes.is_empty()) {
            let (plugin, preset) = if track.is_drums() {
                DRUM_INSTRUMENT
            } else {
                KEYS_INSTRUMENT
            };
            let name: String = track.name.chars().take(MIDI_TRACK_NAME_CHARS).collect();
            let name = if name.is_empty() {
                "MIDI".to_owned()
            } else {
                name
            };
            let new = new_track(
                &mut ids,
                name,
                TrackKind::Instrument,
                Some((plugin, preset)),
            );
            let clip = clip_from_imported(&mut ids, track, start_beat, None, None)?;
            created.push(json!({
                "track_id": new.track_id,
                "clip_id": clip.id,
                "name": new.name,
                "notes": clip.notes.len(),
                "drums": track.is_drums(),
            }));
            let clip_command = CreateMidiClip {
                track_id: new.track_id.clone(),
                start: clip.start,
                length: clip.length,
                name: clip.name.clone(),
                notes: clip.notes.to_vec(),
                loop_length: None,
                color: None,
                clip_id: clip.id.clone(),
            };
            commands.push(Box::new(new));
            commands.push(Box::new(clip_command));
        }
        if created.is_empty() {
            return Err(
                DomainError::invalid(format!("{} contains no notes", file_name(path))).into(),
            );
        }
        let result = self.execute(commands, "Import MIDI")?;
        Ok(json!({
            "tracks": created,
            "warnings": imported.warnings,
            "project_version": result.version,
            "tempo_bpm": first_tempo,
        }))
    }

    /// Exports the open project's instrument tracks (or `track_ids`) as a Type 1 Standard MIDI
    /// File at `path` (the prototype's `RenderService.export_midi`; here, because MIDI export
    /// needs no rendering).
    ///
    /// # Errors
    ///
    /// - [`ErrorCode::NoProject`];
    /// - [`ErrorCode::Conflict`]: `<path> already exists` (detail `path`), unless `overwrite`;
    /// - the errors of [`export_smf`].
    pub fn export_midi(
        &self,
        path: &Path,
        track_ids: Option<&[String]>,
        overwrite: bool,
    ) -> AppResult<Value> {
        let (project, _) = self.snapshot()?;
        if path.exists() && !overwrite {
            let shown = path.display().to_string();
            return Err(
                DomainError::new(ErrorCode::Conflict, format!("{shown} already exists"))
                    .with_detail("path", shown)
                    .into(),
            );
        }
        let track_ids: Option<Vec<&str>> =
            track_ids.map(|ids| ids.iter().map(String::as_str).collect());
        let exported = export_smf(&project, path, track_ids.as_deref())?;
        Ok(json!({
            "path": exported.path,
            "tracks": exported.tracks,
            "notes": exported.notes,
            "max_rounding_error_beats": exported.max_rounding_error_beats,
            "ppq": PPQ,
        }))
    }
}

/// The reader factory of `workspace` (`None`: asset paths as they are, no cache), playing the
/// sampler instruments of `instruments`.
fn bind(workspace: Option<PathBuf>, instruments: &Arc<InstalledInstruments>) -> MediaProvider {
    let cache = workspace
        .as_ref()
        .map(|dir| dir.join("cache").join("decoded"));
    let instruments = Arc::clone(instruments);
    MediaProvider::new(
        move |asset| match &workspace {
            Some(dir) => asset_path(asset, dir),
            None => PathBuf::from(&asset.path),
        },
        cache,
    )
    .with_instruments(move |value| instruments.resolve(value))
}

/// A track command with its IDs drawn as the prototype draws them: the track's, then its
/// instrument's (also for tracks without an instrument).
fn new_track(
    ids: &mut IdSource,
    name: String,
    kind: TrackKind,
    instrument: Option<(&str, &str)>,
) -> CreateTrack {
    let track_id = ids.new_id(ids::TRACK);
    let instrument_id = ids.new_id(ids::DEVICE);
    CreateTrack {
        name,
        kind,
        index: None,
        color: None,
        output_id: MASTER_ID.to_owned(),
        instrument_plugin: instrument.map(|(plugin, _)| plugin.to_owned()),
        instrument_preset: instrument.map(|(_, preset)| preset.to_owned()),
        parent_id: None,
        track_id,
        instrument_id,
    }
}

/// Python's `Path.name`.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Brings the audio file `source` into the project at `workspace` as asset `asset_id`
/// (`audio_import.import_audio`): inspected (with the `FFmpeg` fallback when `ffmpeg` is
/// given), size-checked, then linked where it is or copied to
/// `media/{audio|impulses}/<asset_id>-<stem><suffix>`, and hashed. A file only `FFmpeg` reads
/// is transcoded instead ([`bring_in_transcoded`]), whatever the mode. Returns the asset, the
/// inspection and the import warnings.
fn bring_in(
    source: &Path,
    workspace: &Path,
    asset_id: String,
    mode: ImportMode,
    kind: MediaKind,
    ffmpeg: Option<&Ffmpeg>,
) -> AppResult<(MediaAsset, AudioFileInfo, Vec<String>)> {
    let source = resolve(&expand_user(source));
    let info = inspect_audio_with(&source, ffmpeg)?;
    let name = file_name(&source);
    check_import_size(&name, info.size_bytes)?;
    let warnings = import_warnings(&name, &info);
    let metadata: JsonMap = info
        .metadata
        .iter()
        .map(|(key, value)| (key.clone(), Value::from(value.as_str())))
        .collect();
    let original = source.to_string_lossy().into_owned();
    if let Some(ffmpeg) = ffmpeg.filter(|_| info.decoder == "ffmpeg") {
        let transcoded = Transcoded {
            source: &source,
            workspace,
            asset_id,
            kind,
            name,
            original,
        };
        return bring_in_transcoded(transcoded, ffmpeg, info, metadata, warnings);
    }
    let (path, location, size_bytes, sha256) = match mode {
        ImportMode::Link => {
            let digest = sha256_file(&source).io("read", &source)?;
            (
                original.clone(),
                MediaLocation::Linked,
                info.size_bytes,
                digest,
            )
        }
        ImportMode::Copy => {
            let rel = import_path(&asset_id, kind, &original);
            let dest = in_workspace(workspace, &rel);
            atomic_copy(&source, &dest).io("copy", &source)?;
            let digest = sha256_file(&dest).io("read", &dest)?;
            let size = std::fs::metadata(&dest).io("read", &dest)?.len();
            (rel, MediaLocation::Embedded, size, digest)
        }
    };
    let asset = MediaAsset::new(MediaAssetData {
        id: asset_id,
        kind,
        name,
        path,
        location,
        size_bytes,
        sha256: Some(sha256),
        sample_rate: info.sample_rate,
        channels: info.channels,
        frames: info.frames,
        format: info.format.clone(),
        subtype: info.subtype.clone(),
        original_path: Some(original),
        relative_hint: None,
        metadata,
        extra: JsonMap::new(),
    })?;
    Ok((asset, info, warnings))
}

/// `rel` (a `/`-separated project path) inside `workspace`.
fn in_workspace(workspace: &Path, rel: &str) -> PathBuf {
    let mut path = workspace.to_path_buf();
    path.extend(rel.split('/'));
    path
}

/// What [`bring_in_transcoded`] imports.
struct Transcoded<'a> {
    source: &'a Path,
    workspace: &'a Path,
    asset_id: String,
    kind: MediaKind,
    name: String,
    original: String,
}

/// The `FFmpeg` branch of `import_audio`: the original is copied to
/// `media/{folder}/originals/`, `ffmpeg` transcodes it to FLAC next to it, and the FLAC file is
/// the embedded asset, described as libsndfile reads it, with the metadata `transcoded_by`,
/// `original_file` and `original_format` after the file's tags, and a warning that says so.
fn bring_in_transcoded(
    import: Transcoded<'_>,
    ffmpeg: &Ffmpeg,
    info: AudioFileInfo,
    mut metadata: JsonMap,
    mut warnings: Vec<String>,
) -> AppResult<(MediaAsset, AudioFileInfo, Vec<String>)> {
    let Transcoded {
        source,
        workspace,
        asset_id,
        kind,
        name,
        original,
    } = import;
    let (dest_rel, original_rel) = transcode_paths(&asset_id, kind, &original);
    atomic_copy(source, &in_workspace(workspace, &original_rel)).io("copy", source)?;
    let dest = in_workspace(workspace, &dest_rel);
    ffmpeg.transcode_to_flac(source, &dest)?;
    metadata.insert("transcoded_by".to_owned(), Value::from("ffmpeg"));
    metadata.insert("original_file".to_owned(), Value::from(original_rel));
    metadata.insert(
        "original_format".to_owned(),
        Value::from(info.format.as_str()),
    );
    let decoded = sound_info(&dest).map_err(media_error)?;
    let size_bytes = std::fs::metadata(&dest).io("read", &dest)?.len();
    let sha256 = sha256_file(&dest).io("read", &dest)?;
    let asset = MediaAsset::new(MediaAssetData {
        id: asset_id,
        kind,
        name: name.clone(),
        path: dest_rel,
        location: MediaLocation::Embedded,
        size_bytes,
        sha256: Some(sha256),
        sample_rate: decoded.sample_rate,
        channels: decoded.channels,
        frames: decoded.frames,
        format: decoded.format.name().to_owned(),
        subtype: decoded.subtype.name().to_owned(),
        original_path: Some(original),
        relative_hint: None,
        metadata,
        extra: JsonMap::new(),
    })?;
    warnings.push(transcode_warning(&name));
    Ok((asset, info, warnings))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;

    use sonara_core::error::ErrorCode;
    use sonara_core::ids::SequentialIds;
    use sonara_media::CacheInfo;

    use super::{AudioImport, ImportMode, SoundFileProbe};
    use crate::context::{AppContext, AppOptions};
    use crate::ids::IdSource;
    use crate::jobs::JobStatus;
    use crate::projects::{MediaProbe, NewProject};
    use crate::settings::Settings;

    fn app(root: &Path) -> AppContext {
        AppContext::create(AppOptions {
            workspaces: Some(root.join("workspaces")),
            settings: Some(Settings::default()),
            persist_settings: false,
            ids: Some(IdSource::new(Box::new(SequentialIds::new(0)))),
            ..AppOptions::default()
        })
        .expect("the application")
    }

    #[test]
    fn the_provider_follows_the_open_workspace() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let app = app(dir.path());
        let media = app.media();
        assert_eq!(media.provider().cache_dir(), None);
        app.projects()
            .new_project(&NewProject::default())
            .expect("a project");
        let scratch = app
            .session()
            .lock()
            .location()
            .map(|location| location.workspace.clone())
            .expect("a workspace");
        let bound = media.provider();
        assert_eq!(
            bound.cache_dir(),
            Some(scratch.join("cache").join("decoded").as_path())
        );
        assert!(
            Arc::ptr_eq(&bound, &media.provider()),
            "kept while unchanged"
        );
        let saved = app
            .projects()
            .save(Some(&dir.path().join("Song")), None)
            .expect("saved");
        let rebound = media.provider();
        assert!(!Arc::ptr_eq(&bound, &rebound), "rebound after Save As");
        assert_eq!(
            rebound.cache_dir(),
            Some(saved.workspace.join("cache").join("decoded").as_path())
        );
        app.projects().save(None, None).expect("saved in place");
        assert!(
            Arc::ptr_eq(&rebound, &media.provider()),
            "kept after saving in place"
        );
        app.projects().close(false).expect("closed");
        assert_eq!(media.provider().cache_dir(), None);
    }

    #[test]
    fn importing_media_at_another_rate_decodes_it_into_the_cache() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let app = app(dir.path());
        app.projects()
            .new_project(&NewProject::default())
            .expect("a project");
        let wav = dir.path().join("in").join("tone.wav");
        fs::create_dir_all(wav.parent().expect("a folder")).expect("the folder");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&wav, spec).expect("a WAV writer");
        for frame in 0..22_050_i32 {
            let sample = i16::try_from(frame % 2000 - 1000).expect("in range");
            writer.write_sample(sample).expect("a sample");
        }
        writer.finalize().expect("the WAV is written");
        let options = AudioImport {
            mode: ImportMode::Link,
            create_clip: true,
            ..AudioImport::default()
        };
        let result = app.media().import_audio(&wav, &options).expect("imported");
        // Sequential IDs: the project, then asset, track, instrument, clip and the job.
        assert_eq!(result["asset_id"], "med_000000000002");
        assert_eq!(result["track_id"], "trk_000000000003");
        assert_eq!(result["clip_id"], "clp_000000000005");
        let operation = result["decode_operation_id"]
            .as_str()
            .expect("a decode job");
        assert_eq!(operation, "op_000000000006");
        let job = app.jobs().wait(operation, None).expect("the job");
        assert_eq!(job.status(), JobStatus::Succeeded, "{}", job.to_json());
        let project = app.session().project().expect("open");
        let asset = project.asset("med_000000000002").expect("the asset");
        let workspace = app
            .session()
            .lock()
            .location()
            .map(|location| location.workspace.clone())
            .expect("a workspace");
        let cache = CacheInfo::new(&workspace.join("cache").join("decoded"), &asset.id, 48_000);
        assert!(cache.is_ready(asset.sha256.as_deref()));
        let (channels, frames) = cache.read_layout().expect("the cache layout");
        assert_eq!((channels, frames), (1, 24_000));
        // The player now reads the asset from its cache.
        assert!(app.media().provider().cache_ready(asset, 48_000));
    }

    #[test]
    fn the_probe_reads_audio_facts_and_refuses_other_files() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let wav = dir.path().join("tone.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&wav, spec).expect("a WAV writer");
        for _ in 0..(441 * 2) {
            writer.write_sample(0_i16).expect("a sample");
        }
        writer.finalize().expect("the WAV is written");
        let facts = SoundFileProbe.probe(&wav).expect("audio facts");
        assert_eq!(
            (facts.frames, facts.sample_rate, facts.channels),
            (441, 44_100, 2)
        );
        let text = dir.path().join("notes.wav");
        fs::write(&text, b"not audio").expect("the file is written");
        let err = SoundFileProbe.probe(&text).expect_err("not audio");
        assert_eq!(err.code(), ErrorCode::Unsupported);
    }
}

/// The `FFmpeg` fallback of the import. Oracle:
/// `sonara-media`'s `tests/fixtures/formats/oracle.json` (`ffmpeg.import`), the prototype's
/// `import_audio` with stand-in `ffprobe` and `ffmpeg` scripts on `PATH`, recorded by
/// `scripts/golden/media_format_oracles.py` (GLD-01). The same scripts run here.
#[cfg(all(test, unix))]
mod ffmpeg_tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use serde_json::{Value, json};
    use sonara_core::ids::SequentialIds;
    use sonara_core::model::media::MediaKind;
    use sonara_media::{CacheInfo, Ffmpeg};
    use sonara_project::serializer::asset_to_value;

    use super::{AudioImport, ImportMode, bring_in, system_ffmpeg};
    use crate::context::{AppContext, AppOptions};
    use crate::ids::IdSource;
    use crate::jobs::JobStatus;
    use crate::projects::NewProject;
    use crate::settings::Settings;

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../sonara-media/tests/fixtures/formats")
    }

    fn oracle() -> Value {
        let text = fs::read_to_string(fixtures().join("oracle.json")).expect("the oracle");
        serde_json::from_str(&text).expect("JSON")
    }

    fn executable(path: &Path, text: &str) {
        fs::write(path, text).expect("a script");
        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("executable");
    }

    /// The recorder's stand-ins: `ffprobe` prints `probe`; `ffmpeg` copies `flac` to its last
    /// argument, or prints `err` and exits with `rc`. Both record their arguments.
    fn tools(bin: &Path, probe: &str, flac: &Path, rc: i64, err: &str) {
        fs::create_dir_all(bin).expect("bin");
        let d = bin.display();
        fs::write(bin.join("ffprobe.out"), probe).expect("output");
        fs::write(bin.join("ffprobe.err"), "").expect("error output");
        executable(
            &bin.join("ffprobe"),
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{d}/ffprobe.args'\n/bin/cat '{d}/ffprobe.out'\n\
                 /bin/cat '{d}/ffprobe.err' >&2\nexit 0\n"
            ),
        );
        let body = if rc == 0 {
            format!(
                "for last; do :; done\n/bin/cp '{}' \"$last\"\n",
                flac.display()
            )
        } else {
            fs::write(bin.join("ffmpeg.err"), err).expect("error output");
            format!("/bin/cat '{d}/ffmpeg.err' >&2\nexit {rc}\n")
        };
        executable(
            &bin.join("ffmpeg"),
            &format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{d}/ffmpeg.args'\n{body}"),
        );
    }

    fn sanitized(text: &str, root: &Path) -> String {
        text.replace(&root.display().to_string(), "<ROOT>")
    }

    fn files_under(dir: &Path) -> Vec<String> {
        fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
            for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, base, out);
                } else if let Ok(rel) = path.strip_prefix(base) {
                    out.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        let mut out = Vec::new();
        walk(dir, dir, &mut out);
        out.sort();
        out
    }

    #[test]
    fn imports_match_the_prototypes_ffmpeg_branch() {
        let oracle = oracle();
        let probe = oracle["ffmpeg"]["inspect"]
            .as_array()
            .expect("scenarios")
            .iter()
            .find(|s| s["name"] == "m4a")
            .and_then(|s| s["probe"].as_str())
            .expect("the M4A probe")
            .to_owned();
        let not_audio = oracle["ffmpeg"]["not_audio"]
            .as_str()
            .expect("the stand-in");
        let scenarios = oracle["ffmpeg"]["import"]
            .as_array()
            .expect("scenarios")
            .clone();
        assert_eq!(scenarios.len(), 4);
        for scenario in scenarios {
            import_scenario(&scenario, &probe, not_audio);
        }
    }

    /// One recorded import: the asset, inspection, warnings and original (or the error), the
    /// command line, and the files left in the project.
    fn import_scenario(scenario: &Value, probe: &str, not_audio: &str) {
        let name = scenario["name"].as_str().expect("a name");
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = fs::canonicalize(dir.path()).expect("resolves");
        let bin = root.join(name).join("bin");
        tools(
            &bin,
            probe,
            &fixtures().join("inputs-ffmpeg/transcoded.flac"),
            scenario["ffmpeg_rc"].as_i64().expect("rc"),
            scenario["ffmpeg_stderr"].as_str().expect("stderr"),
        );
        let source = root
            .join(name)
            .join("in")
            .join(scenario["file"].as_str().expect("file"));
        fs::create_dir_all(source.parent().expect("a folder")).expect("in");
        fs::write(&source, not_audio).expect("the stand-in");
        let workspace = root.join(name).join("ws");
        let mode = match scenario["mode"].as_str() {
            Some("link") => ImportMode::Link,
            _ => ImportMode::Copy,
        };
        let kind = match scenario["kind"].as_str() {
            Some("impulse") => MediaKind::Impulse,
            _ => MediaKind::Audio,
        };
        let ffmpeg = Ffmpeg::discover_in(Some(bin.as_os_str()));
        let result = bring_in(
            &source,
            &workspace,
            scenario["asset_id"].as_str().expect("an ID").to_owned(),
            mode,
            kind,
            ffmpeg.as_ref(),
        );
        match result {
            Ok((asset, info, warnings)) => {
                let mut ours = asset_to_value(&asset);
                ours["original_path"] = Value::String(sanitized(
                    asset.original_path.as_deref().unwrap_or_default(),
                    &root,
                ));
                assert_eq!(ours, scenario["asset"], "{name}: asset");
                let order: Vec<&String> = asset.metadata.keys().collect();
                assert_eq!(
                    order,
                    [
                        "major_brand",
                        "title",
                        "artist",
                        "encoder",
                        "transcoded_by",
                        "original_file",
                        "original_format"
                    ],
                    "{name}: metadata order"
                );
                let mut info = info.to_json();
                info["path"] =
                    Value::String(sanitized(info["path"].as_str().unwrap_or_default(), &root));
                assert_eq!(info, scenario["info"], "{name}: inspection");
                assert_eq!(json!(warnings), scenario["warnings"], "{name}: warnings");
                // The original is kept byte for byte.
                let original =
                    workspace.join(asset.metadata["original_file"].as_str().expect("a path"));
                assert_eq!(
                    fs::read(original).expect("the original"),
                    not_audio.as_bytes()
                );
            }
            Err(error) => {
                let notice = crate::events::ErrorNotice::from(&error);
                assert_eq!(json!(notice.code), scenario["error"]["code"], "{name}");
                assert_eq!(
                    json!(notice.message),
                    scenario["error"]["message"],
                    "{name}"
                );
            }
        }
        let args: Vec<String> = fs::read_to_string(bin.join("ffmpeg.args"))
            .expect("ffmpeg ran")
            .lines()
            .map(|line| sanitized(line, &root))
            .collect();
        assert_eq!(
            json!(args),
            scenario["ffmpeg_args"],
            "{name}: ffmpeg's arguments"
        );
        assert_eq!(
            json!(files_under(&workspace)),
            scenario["workspace_files"],
            "{name}: files"
        );
    }

    fn app(root: &Path, locator: super::FfmpegLocator) -> AppContext {
        AppContext::create(AppOptions {
            workspaces: Some(root.join("workspaces")),
            settings: Some(Settings::default()),
            persist_settings: false,
            ids: Some(IdSource::new(Box::new(SequentialIds::new(0)))),
            ffmpeg: Some(locator),
            ..AppOptions::default()
        })
        .expect("the application")
    }

    #[test]
    fn the_service_imports_through_ffmpeg_and_decodes_the_flac_into_the_cache() {
        let oracle = oracle();
        let probe = oracle["ffmpeg"]["inspect"][0]["probe"]
            .as_str()
            .expect("a probe")
            .to_owned();
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = fs::canonicalize(dir.path()).expect("resolves");
        let bin = root.join("bin");
        tools(
            &bin,
            &probe,
            &fixtures().join("inputs-ffmpeg/transcoded.flac"),
            0,
            "",
        );
        let found = Ffmpeg::discover_in(Some(bin.as_os_str())).expect("the stand-ins");
        let app = app(&root, Arc::new(move || Some(found.clone())));
        assert!(app.media().ffmpeg().is_some());
        app.projects()
            .new_project(&NewProject::default())
            .expect("a project");
        let source = root.join("Take.m4a");
        fs::write(&source, "not audio").expect("a source");
        let dry = app
            .media()
            .import_audio(
                &source,
                &AudioImport {
                    dry_run: true,
                    ..AudioImport::default()
                },
            )
            .expect("a dry run");
        assert_eq!(dry["file"]["decoder"], "ffmpeg");
        let result = app
            .media()
            .import_audio(
                &source,
                &AudioImport {
                    create_clip: true,
                    ..AudioImport::default()
                },
            )
            .expect("imported");
        assert_eq!(result["file"]["format"], "AAC");
        assert_eq!(result["warnings"].as_array().map(Vec::len), Some(2));
        let asset_id = result["asset_id"].as_str().expect("an asset").to_owned();
        let project = app.session().project().expect("open");
        let asset = project.asset(&asset_id).expect("the asset");
        assert_eq!(
            (asset.format.as_str(), asset.subtype.as_str()),
            ("FLAC", "PCM_24")
        );
        // The FLAC file plays from the decode cache, at the project's rate.
        let operation = result["decode_operation_id"]
            .as_str()
            .expect("a decode job");
        let job = app.jobs().wait(operation, None).expect("the job");
        assert_eq!(job.status(), JobStatus::Succeeded, "{}", job.to_json());
        let workspace = app
            .session()
            .lock()
            .location()
            .map(|location| location.workspace.clone())
            .expect("a workspace");
        let cache = CacheInfo::new(&workspace.join("cache").join("decoded"), &asset.id, 48_000);
        assert!(cache.is_ready(asset.sha256.as_deref()));
    }

    #[test]
    fn without_ffmpeg_an_unreadable_file_is_unsupported() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = fs::canonicalize(dir.path()).expect("resolves");
        let app = app(&root, Arc::new(|| None));
        assert!(app.media().ffmpeg().is_none());
        let source = root.join("x.m4a");
        fs::write(&source, "not audio").expect("a source");
        let err = app.media().inspect(&source).expect_err("unsupported");
        assert_eq!(err.code.as_str(), "UNSUPPORTED");
        assert!(
            err.message.contains("FFmpeg is not installed"),
            "{}",
            err.message
        );
    }

    /// The prototype's `test_ffmpeg_fallback_transcodes_losslessly_and_keeps_original`, with
    /// the real `ffmpeg`: skipped (with a note) when it is not installed, as the prototype's
    /// test was (`skipif`).
    #[test]
    fn real_ffmpeg_transcodes_losslessly_and_keeps_the_original() {
        let Some(ffmpeg) = system_ffmpeg() else {
            println!("skipped: ffmpeg is not installed");
            return;
        };
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = fs::canonicalize(dir.path()).expect("resolves");
        let wav = root.join("s.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&wav, spec).expect("a WAV writer");
        let signal: Vec<f64> = (0..48_000)
            .map(|n| {
                let t = f64::from(n) / 48_000.0;
                0.4 * (2.0 * std::f64::consts::PI * 220.0 * t).sin()
                    + 0.2 * (2.0 * std::f64::consts::PI * 1_330.0 * t).sin()
            })
            .collect();
        for value in &signal {
            #[expect(clippy::cast_possible_truncation, reason = "a 16-bit test signal")]
            let sample = (value * 32_767.0).round() as i16;
            writer.write_sample(sample).expect("left");
            writer.write_sample(sample).expect("right");
        }
        writer.finalize().expect("the WAV");
        let m4a = root.join("s.m4a");
        let status = std::process::Command::new(ffmpeg.ffmpeg())
            .args(["-nostdin", "-v", "error", "-y", "-i"])
            .arg(&wav)
            .args(["-c:a", "aac", "-b:a", "256k"])
            .arg(&m4a)
            .status()
            .expect("ffmpeg runs");
        assert!(status.success());
        let workspace = root.join("ws");
        let (asset, info, warnings) = bring_in(
            &m4a,
            &workspace,
            "med_test0005".to_owned(),
            ImportMode::Copy,
            MediaKind::Audio,
            Some(&ffmpeg),
        )
        .expect("imported");
        assert_eq!(info.decoder, "ffmpeg");
        assert_eq!(asset.format, "FLAC");
        let original = workspace.join(asset.metadata["original_file"].as_str().expect("a path"));
        assert_eq!(
            fs::read(original).expect("the original"),
            fs::read(&m4a).expect("the M4A")
        );
        assert!(
            warnings.iter().any(|w| w.contains("FFmpeg")),
            "{warnings:?}"
        );
        let mut reader = sonara_media::MediaReader::Direct(
            sonara_media::DirectReader::open(&workspace.join(&asset.path)).expect("the FLAC"),
        );
        let frames = usize::try_from(reader.frames()).expect("frames");
        let decoded: Vec<f64> = reader
            .read(0, frames)
            .expect("decodes")
            .as_chunks::<2>()
            .0
            .iter()
            .map(|frame| frame[0])
            .collect();
        assert!(aligned_correlation(&signal, &decoded) > 0.99);
    }

    /// The largest normalized correlation of `b` delayed by up to 4096 frames against `a`.
    fn aligned_correlation(a: &[f64], b: &[f64]) -> f64 {
        let mut best = f64::MIN;
        for lag in 0..4096.min(b.len()) {
            let pairs = a.iter().zip(b.iter().skip(lag));
            let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
            for (x, y) in pairs {
                ab += x * y;
                aa += x * x;
                bb += y * y;
            }
            if aa > 0.0 && bb > 0.0 {
                best = best.max(ab / (aa * bb).sqrt());
            }
        }
        best
    }
}
