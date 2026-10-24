//! Offline rendering, previews, analysis and export of the open project
//! (`sonara.application.services.render_service`).
//!
//! [`RenderService`] renders the session's project with [`OfflineRenderer`] over the media of
//! the open workspace ([`MediaService::provider`]) and runs the long operations as background
//! jobs ([`JobManager`]):
//!
//! - **Preview** ([`RenderService::preview`], job `render_preview`): the render is analyzed in
//!   its `float64` blocks and written as a 32-bit float WAV (`.<name>.partial`, renamed when
//!   complete, removed on failure); optionally each audio, instrument and bus track is rendered
//!   alone and measured ([`RenderService::track_levels`]). A sidecar `<stem>.analysis.json`
//!   (Python's `json.dumps(…, indent=1)`, [`dumps`]) keeps the result with the short-term
//!   loudness series. Without a path, the file is
//!   `<workspace>/cache/previews/preview-<local time %Y%m%d-%H%M%S>-<6 characters of a new
//!   r_ ID>.wav`; the time zone is [`AppOptions::local_zone`](crate::context::AppOptions).
//! - **Analysis** ([`RenderService::analyze_file`], [`RenderService::analyze_asset`],
//!   [`analyze_file`]): the analyzer over a decoded file in 65536-frame `float64` blocks.
//! - **Export** ([`RenderService::export`], job `export_audio`): the render streamed into the
//!   two-pass [`Exporter`], with the project's metadata as defaults, the render's first frame as
//!   the BWF time reference, and the render's warnings before the export's.
//! - **Stems** ([`RenderService::export_stems`], job `export_stems`): one export per track,
//!   rendered alone through its output path, with or without the master.
//!
//! Progress and messages follow the prototype, whose last values the golden transcripts record:
//! a preview renders up to 0.9 (`rendering`), then measures each track (`measuring <name>`); an
//! export renders up to 0.6 (`rendering`) and encodes to 1 (`encoding`); stems report
//! `stem <name>` per track.
//!
//! The wall clock of the application ([`WallClock`]) names previews and stamps the files' time
//! chunks (`PEAK`, `bext`), so golden runs are reproducible. Nothing here runs on a real-time
//! thread.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use serde_json::{Value, json};
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::ids::IdGenerator as _;
use sonara_core::model::JsonMap;
use sonara_core::model::project::Project;
use sonara_core::model::track::TrackKind;
use sonara_core::plugins::PluginRegistry;
use sonara_core::pyfmt::py_round;
use sonara_dsp::analysis::{AnalysisReport, Analyzer, Issue};
use sonara_engine::{EngineError, OfflineRenderer, RenderStream};
use sonara_media::export::{
    ExportError, ExportOptions, Exporter, PcmContainer, PcmFileWriter, SampleFormat,
    available_containers, validate_output,
};
use sonara_media::safe_stem;
use sonara_project::atomic::atomic_write_bytes;

pub use sonara_engine::{RenderRequest, RenderStats, RenderedAudio, Tail};
pub use sonara_media::export::{
    Container, Dither, ExportResult, Normalize, NormalizeSpec, OutputResult, OutputSpec,
};

use crate::clock::{LocalZone, WallClock, local_stamp};
use crate::error::{AppError, AppResult, IoContext as _};
use crate::ids::IdSource;
use crate::jobs::{Job, JobContext, JobManager};
use crate::media::MediaService;
use crate::paths::with_suffix;
use crate::pyjson::dumps;
use crate::session::SharedSession;

/// The longest preview, in seconds (`MAX_PREVIEW_SECONDS`).
pub const MAX_PREVIEW_SECONDS: f64 = 15.0 * 60.0;
/// The share of a preview job's progress that the render takes; track levels take the rest.
const PREVIEW_RENDER_SHARE: f64 = 0.9;
/// The share of an export job's progress that the render takes; encoding takes the rest.
const EXPORT_RENDER_SHARE: f64 = 0.6;
/// The longest stem name derived from the project and track names, in characters.
const STEM_NAME_CHARS: usize = 120;
/// The software tag written into exported files.
const SOFTWARE: &str = "Sonara";

/// The export formats this build writes, sorted by name (`get_capabilities.export_formats`):
/// every container, MP3 only when libmp3lame can be loaded.
#[must_use]
pub fn export_formats() -> Vec<&'static str> {
    let mut formats: Vec<&'static str> = available_containers()
        .into_iter()
        .map(Container::value)
        .collect();
    formats.sort_unstable();
    formats
}

/// The analysis of the audio file at `path` as `sonara analyze` prints it:
/// `{"analysis": report.to_json(include_series), "issues": [...]}`. The file is decoded and fed
/// to the analyzer in 65536-frame `float64` blocks, as the render service's `analyze_file`
/// feeds it ([`sonara_media::export::analyze_file`]).
///
/// # Errors
///
/// The errors of [`sonara_media::export::analyze_file`]: a file that cannot be opened or
/// decoded, or a rate or channel count the analyzer rejects.
pub fn analyze_file(path: &Path, include_series: bool) -> Result<Value, ExportError> {
    let report = sonara_media::export::analyze_file(path)?;
    Ok(Value::Object(analysis_entries(&report, include_series)))
}

/// `{"analysis": report.to_json(include_series), "issues": report.issues()}`.
fn analysis_entries(report: &AnalysisReport, include_series: bool) -> JsonMap {
    let mut out = JsonMap::new();
    out.insert("analysis".to_owned(), report.to_json(include_series));
    out.insert("issues".to_owned(), issues_json(report));
    out
}

fn issues_json(report: &AnalysisReport) -> Value {
    Value::Array(report.issues().iter().map(Issue::to_json).collect())
}

/// The timeline frames `[start, end)` that `request` renders from `project` (the prototype's
/// `renderer().range_frames`, for the dry run of an export).
///
/// # Errors
///
/// `INVALID_ARGUMENT` for an empty range, `LIMIT_EXCEEDED` for a range over
/// `request.max_seconds` ([`sonara_engine::range_frames`]).
pub fn range_frames(project: &Project, request: &RenderRequest) -> AppResult<(i64, i64)> {
    Ok(sonara_engine::range_frames(project, request)?)
}

/// `_num`: a level rounded to 2 decimals, `None` when it is not finite (silence).
fn level(value: f64) -> Option<f64> {
    value.is_finite().then(|| py_round(value, 2))
}

fn level_json(value: f64) -> Value {
    level(value).map_or(Value::Null, Value::from)
}

/// The levels of one track rendered alone ([`RenderService::track_levels`]).
#[derive(Debug, Clone, PartialEq)]
pub struct TrackLevels {
    /// The track's name.
    pub name: String,
    /// Integrated loudness in LUFS, 2 decimals; `None` for silence.
    pub integrated_lufs: Option<f64>,
    /// Sample peak in dBFS, 2 decimals; `None` for silence.
    pub peak_dbfs: Option<f64>,
    /// The loudest short-term loudness in LUFS, 2 decimals; `None` for silence.
    pub short_term_max_lufs: Option<f64>,
}

impl TrackLevels {
    fn measured(name: &str, report: &AnalysisReport) -> Self {
        Self {
            name: name.to_owned(),
            integrated_lufs: level(report.integrated_lufs),
            peak_dbfs: level(report.sample_peak_dbfs),
            short_term_max_lufs: level(report.short_term_max_lufs),
        }
    }

    /// The prototype's dict, in its key order.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let opt = |v: Option<f64>| v.map_or(Value::Null, Value::from);
        let mut out = JsonMap::new();
        out.insert("name".to_owned(), Value::from(self.name.clone()));
        out.insert("integrated_lufs".to_owned(), opt(self.integrated_lufs));
        out.insert("peak_dbfs".to_owned(), opt(self.peak_dbfs));
        out.insert(
            "short_term_max_lufs".to_owned(),
            opt(self.short_term_max_lufs),
        );
        Value::Object(out)
    }
}

/// A finished preview (`PreviewResult`).
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewResult {
    /// The WAV file.
    pub path: PathBuf,
    /// Rendered seconds: the range and the tail.
    pub seconds: f64,
    /// The project's sample rate.
    pub sample_rate: u32,
    /// The first beat requested.
    pub start_beat: f64,
    /// The end beat: the request's, else the project end.
    pub end_beat: f64,
    /// Wall-clock seconds the render took (without track levels).
    pub render_seconds: f64,
    /// Rendered seconds per wall-clock second; 0 when no time passed.
    pub realtime_factor: f64,
    /// The analysis of the render (before the float32 storage).
    pub analysis: AnalysisReport,
    /// Frame counts, latency, warnings and missing media.
    pub stats: RenderStats,
    /// Per track ID, the levels of the track rendered alone (empty unless requested).
    pub track_levels: Vec<(String, TrackLevels)>,
}

impl PreviewResult {
    /// The prototype's `to_json`, in its key order: seconds and the tail rounded to 3 decimals,
    /// the realtime factor to 2; `track_levels` only when measured.
    #[must_use]
    pub fn to_json(&self) -> JsonMap {
        let mut m = JsonMap::new();
        m.insert(
            "path".to_owned(),
            Value::from(self.path.display().to_string()),
        );
        m.insert("seconds".to_owned(), Value::from(py_round(self.seconds, 3)));
        m.insert("sample_rate".to_owned(), Value::from(self.sample_rate));
        m.insert(
            "range_beats".to_owned(),
            json!([self.start_beat, self.end_beat]),
        );
        m.insert(
            "render_seconds".to_owned(),
            Value::from(py_round(self.render_seconds, 3)),
        );
        m.insert(
            "realtime_factor".to_owned(),
            Value::from(py_round(self.realtime_factor, 2)),
        );
        m.insert(
            "latency_samples".to_owned(),
            Value::from(self.stats.latency),
        );
        m.insert(
            "tail_seconds".to_owned(),
            Value::from(py_round(
                frames_to_f64(self.stats.tail_frames) / f64::from(self.sample_rate),
                3,
            )),
        );
        m.insert(
            "warnings".to_owned(),
            Value::from(self.stats.warnings.clone()),
        );
        m.insert(
            "missing_media".to_owned(),
            Value::from(self.stats.missing_media.clone()),
        );
        m.extend(analysis_entries(&self.analysis, false));
        if !self.track_levels.is_empty() {
            let levels: JsonMap = self
                .track_levels
                .iter()
                .map(|(id, levels)| (id.clone(), levels.to_json()))
                .collect();
            m.insert("track_levels".to_owned(), Value::Object(levels));
        }
        m
    }
}

/// How [`RenderService::export`] treats its outputs besides their formats.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExportSettings {
    /// Normalization.
    pub normalize: NormalizeSpec,
    /// Metadata over the project's defaults, in order; a repeated key replaces the value.
    pub metadata: Vec<(String, String)>,
    /// Replace existing files.
    pub overwrite: bool,
}

/// What the render service shares with its jobs: everything but the job manager, so a queued
/// job does not hold the manager that holds it.
struct Core {
    session: SharedSession,
    plugins: Arc<PluginRegistry>,
    media: Arc<MediaService>,
    ids: IdSource,
    clock: WallClock,
    zone: LocalZone,
}

/// Renders, previews, analyzes and exports the session's project.
pub struct RenderService {
    core: Arc<Core>,
    jobs: JobManager,
}

impl fmt::Debug for RenderService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderService")
            .field("zone", &self.core.zone)
            .finish_non_exhaustive()
    }
}

impl RenderService {
    /// A service rendering `session`'s project with `plugins` and the media of `media`,
    /// running jobs on `jobs`, naming previews with `ids` and the time of `clock` in `zone`.
    #[must_use]
    pub fn new(
        session: SharedSession,
        plugins: Arc<PluginRegistry>,
        media: Arc<MediaService>,
        jobs: JobManager,
        ids: IdSource,
        clock: WallClock,
        zone: LocalZone,
    ) -> Self {
        Self {
            core: Arc::new(Core {
                session,
                plugins,
                media,
                ids,
                clock,
                zone,
            }),
            jobs,
        }
    }

    // -- preview ----------------------------------------------------------------------------

    /// Renders `request` (at most [`MAX_PREVIEW_SECONDS`]) to a 32-bit float WAV at `path`
    /// (default: a new file in the workspace's `cache/previews`), analyzes it, measures each
    /// track alone with `per_track_levels`, and writes the sidecar `<stem>.analysis.json`.
    /// `job` reports progress and is polled for cancellation.
    ///
    /// # Errors
    ///
    /// - [`ErrorCode::NoProject`];
    /// - the render's errors: an empty range (`INVALID_ARGUMENT`), a range over the limit
    ///   (`LIMIT_EXCEEDED`), an unknown target (`NOT_FOUND`), a routing cycle, a cancelled job
    ///   (`CANCELLED`), unreadable media;
    /// - file errors of the WAV and the sidecar. The partial WAV is removed on any failure.
    pub fn preview(
        &self,
        request: &RenderRequest,
        path: Option<&Path>,
        per_track_levels: bool,
        job: Option<&JobContext<'_>>,
    ) -> AppResult<PreviewResult> {
        self.core.preview(request, path, per_track_levels, job)
    }

    /// Queues [`RenderService::preview`] as a `render_preview` job whose result is the preview's
    /// JSON.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`], and the errors of [`JobManager::submit`].
    pub fn preview_job(
        &self,
        request: RenderRequest,
        path: Option<PathBuf>,
        per_track_levels: bool,
    ) -> AppResult<Job> {
        self.core.require_project()?;
        let core = Arc::clone(&self.core);
        self.jobs
            .submit("render_preview", "Render preview", move |ctx| {
                Ok(core
                    .preview(&request, path.as_deref(), per_track_levels, Some(ctx))?
                    .to_json())
            })
    }

    /// Renders each audio, instrument and bus track of `project` alone, through its output path
    /// without the master's processing and without a tail, and measures it, so agents can judge
    /// the balance without listening. `job` reports `measuring <name>` and is polled for
    /// cancellation between tracks.
    ///
    /// # Errors
    ///
    /// The render's errors (see [`RenderService::preview`]) and `CANCELLED`.
    pub fn track_levels(
        &self,
        project: &Project,
        request: &RenderRequest,
        job: Option<&JobContext<'_>>,
    ) -> AppResult<Vec<(String, TrackLevels)>> {
        self.core.track_levels(project, request, job)
    }

    // -- analysis ---------------------------------------------------------------------------

    /// The analysis of the audio file at `path`: `{"path", "analysis", "issues"}`.
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` (`<path> does not exist`) unless `path` is a file, and the errors of
    /// decoding it ([`analyze_file`]).
    pub fn analyze_file(&self, path: &Path) -> AppResult<JsonMap> {
        Core::analyze_file(path)
    }

    /// The analysis of the open project's media asset `asset_id`, from its file:
    /// `{"path", "analysis", "issues", "asset_id"}`.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`], `NOT_FOUND` for an unknown asset or a missing file, and the
    /// errors of decoding it.
    pub fn analyze_asset(&self, asset_id: &str) -> AppResult<JsonMap> {
        let (project, _) = self.core.snapshot()?;
        let path = self
            .core
            .media
            .provider()
            .path_for(project.asset(asset_id)?);
        let mut out = Core::analyze_file(&path)?;
        out.insert("asset_id".to_owned(), Value::from(asset_id));
        Ok(out)
    }

    // -- export -----------------------------------------------------------------------------

    /// Renders `request` and exports it to every output (`export`). The project's metadata are
    /// the defaults (`title` falls back to the project name; `software` is `Sonara`); the
    /// render's warnings come before the export's. `job` reports progress and is polled for
    /// cancellation.
    ///
    /// # Errors
    ///
    /// - [`ErrorCode::NoProject`];
    /// - the errors of [`validate_output`] (`UNSUPPORTED`, `INVALID_ARGUMENT`);
    /// - the render's errors (see [`RenderService::preview`]);
    /// - the exporter's: no outputs, an existing output without `overwrite` (`CONFLICT`),
    ///   cancellation, and the file, codec and verification errors of writing.
    pub fn export(
        &self,
        outputs: &[OutputSpec],
        request: &RenderRequest,
        settings: &ExportSettings,
        job: Option<&JobContext<'_>>,
    ) -> AppResult<ExportResult> {
        self.core.export(outputs, request, settings, job)
    }

    /// Renders `request` from the open project into memory (`render_array`): the frames, rate
    /// and statistics as the offline renderer returns them, with an auto tail trimmed. The
    /// render benchmark (`crates/sonara-agent/examples/bench.rs`) times it.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`] and the render's errors (see [`RenderService::preview`]).
    pub fn render_array(&self, request: &RenderRequest) -> AppResult<RenderedAudio> {
        let (project, _) = self.core.snapshot()?;
        let provider = self.core.media.provider();
        Ok(OfflineRenderer::new(&self.core.plugins, &provider).render(&project, request, None)?)
    }

    /// Queues [`RenderService::export`] as an `export_audio` job whose result is the export's
    /// JSON.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`], the errors of [`validate_output`], and the errors of
    /// [`JobManager::submit`].
    pub fn export_job(
        &self,
        outputs: Vec<OutputSpec>,
        request: RenderRequest,
        settings: ExportSettings,
    ) -> AppResult<Job> {
        self.core.require_project()?;
        for spec in &outputs {
            validate_output(spec)?;
        }
        let core = Arc::clone(&self.core);
        self.jobs
            .submit("export_audio", "Export audio", move |ctx| {
                let result = core.export(&outputs, &request, &settings, Some(ctx))?;
                Ok(into_map(result.to_json()))
            })
    }

    /// Exports each track of `track_ids` (default: every audio, instrument and bus track) as
    /// its own file in `directory`, rendered alone through its output path, through the master
    /// only with `through_master`. Files are named `<project> - <nn> <track>` (made safe) with
    /// the container's extension. `job` reports `stem <name>` and is polled between stems.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`]; `INVALID_ARGUMENT` (`no tracks to export as stems`);
    /// `NOT_FOUND` for an unknown track; `CANCELLED`; and the errors of
    /// [`RenderService::export`].
    pub fn export_stems(
        &self,
        directory: &Path,
        track_ids: Option<&[String]>,
        template: &OutputSpec,
        request: &RenderRequest,
        stems: StemOptions,
        job: Option<&JobContext<'_>>,
    ) -> AppResult<JsonMap> {
        self.core
            .export_stems(directory, track_ids, template, request, stems, job)
    }

    /// Queues [`RenderService::export_stems`] as an `export_stems` job.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`], the errors of [`validate_output`] for `template`, and the
    /// errors of [`JobManager::submit`].
    pub fn export_stems_job(
        &self,
        directory: PathBuf,
        track_ids: Option<Vec<String>>,
        template: OutputSpec,
        request: RenderRequest,
        stems: StemOptions,
    ) -> AppResult<Job> {
        self.core.require_project()?;
        validate_output(&template)?;
        let core = Arc::clone(&self.core);
        self.jobs
            .submit("export_stems", "Export stems", move |ctx| {
                core.export_stems(
                    &directory,
                    track_ids.as_deref(),
                    &template,
                    &request,
                    stems,
                    Some(ctx),
                )
            })
    }
}

/// The files of a stems export: `<project> - <nn> <track>` made safe, a `_` added until the
/// name is new, with the container's extension.
struct StemNamer<'a> {
    directory: &'a Path,
    extension: &'static str,
    names: HashSet<String>,
}

impl<'a> StemNamer<'a> {
    fn new(directory: &'a Path, container: Container) -> Self {
        Self {
            directory,
            extension: container.extension(),
            names: HashSet::new(),
        }
    }

    /// The file of the `index`th stem (0-based), the track `track`.
    fn path(&mut self, project: &str, index: usize, track: &str) -> PathBuf {
        let mut base = safe_stem(
            &format!("{project} - {:02} {track}", index + 1),
            STEM_NAME_CHARS,
        );
        while self.names.contains(&base) {
            base.push('_');
        }
        self.names.insert(base.clone());
        self.directory.join(format!("{base}{}", self.extension))
    }
}

/// The files [`RenderService::export_stems`] writes into `directory` for `track_ids` (default:
/// every audio, instrument and bus track) of `project` with `container`, in order: the Export
/// dialog asks before it overwrites any of them.
///
/// # Errors
///
/// `NOT_FOUND` for an unknown track.
pub fn stem_paths(
    project: &Project,
    directory: &Path,
    track_ids: Option<&[String]>,
    container: Container,
) -> AppResult<Vec<PathBuf>> {
    let track_ids: Vec<String> = match track_ids {
        Some(ids) => ids.to_vec(),
        None => sounding(project).map(|track| track.id.clone()).collect(),
    };
    let mut namer = StemNamer::new(directory, container);
    track_ids
        .iter()
        .enumerate()
        .map(|(i, id)| Ok(namer.path(&project.name, i, &project.track(id)?.name)))
        .collect()
}

/// How stems are exported besides their format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StemOptions {
    /// Each stem goes through the master's processing.
    pub through_master: bool,
    /// Replace existing files.
    pub overwrite: bool,
}

/// The tracks that make sound of their own: audio, instrument and bus tracks.
fn sounding(project: &Project) -> impl Iterator<Item = &sonara_core::model::track::Track> {
    project.tracks.iter().filter(|track| {
        matches!(
            track.kind,
            TrackKind::Audio | TrackKind::Instrument | TrackKind::Bus
        )
    })
}

impl Core {
    fn require_project(&self) -> AppResult<()> {
        self.session.lock().require_project()?;
        Ok(())
    }

    /// The open project and its workspace (`_snapshot`).
    fn snapshot(&self) -> AppResult<(Project, PathBuf)> {
        let session = self.session.lock();
        let project = session.require_project()?.clone();
        let workspace = session.require_location()?.workspace.clone();
        Ok((project, workspace))
    }

    fn analyze_file(path: &Path) -> AppResult<JsonMap> {
        if !path.is_file() {
            return Err(DomainError::new(
                ErrorCode::NotFound,
                format!("{} does not exist", path.display()),
            )
            .into());
        }
        let report = sonara_media::export::analyze_file(path)?;
        let mut out = JsonMap::new();
        out.insert("path".to_owned(), Value::from(path.display().to_string()));
        out.extend(analysis_entries(&report, false));
        Ok(out)
    }

    /// The default preview file: `preview-<local time>-<6 characters of a new r_ ID>.wav`.
    fn preview_path(&self, workspace: &Path) -> PathBuf {
        let stamp = local_stamp((self.clock)(), self.zone);
        let id = self.ids.clone().new_id("r");
        // `new_id('r')[2:8]`: the characters after `r_`.
        let tag: String = id.chars().skip(2).take(6).collect();
        workspace
            .join("cache")
            .join("previews")
            .join(format!("preview-{stamp}-{tag}.wav"))
    }

    fn preview(
        &self,
        request: &RenderRequest,
        path: Option<&Path>,
        per_track_levels: bool,
        job: Option<&JobContext<'_>>,
    ) -> AppResult<PreviewResult> {
        let (project, workspace) = self.snapshot()?;
        let path = match path {
            Some(path) => path.to_path_buf(),
            None => self.preview_path(&workspace),
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).io("create", parent)?;
        }
        let mut request = request.clone();
        request.max_seconds = request.max_seconds.min(MAX_PREVIEW_SECONDS);
        let sample_rate = project.settings.sample_rate;
        let started = Instant::now();
        let partial = partial_path(&path);
        let provider = self.media.provider();
        let renderer = OfflineRenderer::new(&self.plugins, &provider);
        // The range is checked before the partial file exists.
        range_frames(&project, &request)?;
        let (stats, report) =
            match write_preview(&renderer, &project, &request, &partial, &self.clock, job) {
                Ok(done) => done,
                Err(err) => {
                    // ERR-06: the render's error is the one reported; a partial file that
                    // cannot be removed changes nothing about it (`unlink(missing_ok=True)`).
                    let _ = fs::remove_file(&partial);
                    return Err(err);
                }
            };
        if let Err(err) = fs::rename(&partial, &path) {
            // ERR-06: as above, the rename's error is the one reported.
            let _ = fs::remove_file(&partial);
            return Err(AppError::Io {
                operation: "rename",
                path: partial,
                source: err,
            });
        }
        let render_seconds = started.elapsed().as_secs_f64();
        let track_levels = if per_track_levels {
            self.track_levels(&project, &request, job)?
        } else {
            Vec::new()
        };
        let seconds = frames_to_f64(stats.frames) / f64::from(sample_rate);
        let result = PreviewResult {
            path: path.clone(),
            seconds,
            sample_rate,
            start_beat: request.start_beat,
            end_beat: request.end_beat.unwrap_or_else(|| project.end_beat()),
            render_seconds,
            realtime_factor: if render_seconds > 0.0 {
                seconds / render_seconds
            } else {
                0.0
            },
            analysis: report,
            stats,
            track_levels,
        };
        // The sidecar keeps the short-term loudness series (1 value/s) for later queries:
        // `result.to_json() | {"analysis": report.to_json(include_series=True)}`.
        let mut stored = result.to_json();
        stored.insert("analysis".to_owned(), result.analysis.to_json(true));
        let sidecar = sidecar_path(&path);
        atomic_write_bytes(&sidecar, dumps(&Value::Object(stored), 1).as_bytes())
            .io("write", &sidecar)?;
        Ok(result)
    }

    fn track_levels(
        &self,
        project: &Project,
        request: &RenderRequest,
        job: Option<&JobContext<'_>>,
    ) -> AppResult<Vec<(String, TrackLevels)>> {
        let tracks: Vec<_> = sounding(project).collect();
        let provider = self.media.provider();
        let renderer = OfflineRenderer::new(&self.plugins, &provider);
        let mut out = Vec::with_capacity(tracks.len());
        for (i, track) in tracks.iter().enumerate() {
            if let Some(job) = job {
                job.check()?;
                job.progress(
                    PREVIEW_RENDER_SHARE
                        + (1.0 - PREVIEW_RENDER_SHARE) * frames_to_f64(i)
                            / frames_to_f64(tracks.len().max(1)),
                    Some(&format!("measuring {}", track.name)),
                );
            }
            let alone = RenderRequest {
                target: Some(track.id.clone()),
                bypass_master: true,
                tail: Tail::None,
                ..request.clone()
            };
            let mut analyzer = analyzer(project.settings.sample_rate)?;
            let mut stream = renderer.stream(project, &alone)?;
            let mut interleaved = Vec::new();
            while let Some(block) = stream.next_block(None)? {
                interleave(block, &mut interleaved);
                analyzer.feed(&interleaved);
            }
            out.push((
                track.id.clone(),
                TrackLevels::measured(&track.name, &analyzer.finish()),
            ));
        }
        Ok(out)
    }

    fn export(
        &self,
        outputs: &[OutputSpec],
        request: &RenderRequest,
        settings: &ExportSettings,
        job: Option<&JobContext<'_>>,
    ) -> AppResult<ExportResult> {
        let (project, workspace) = self.snapshot()?;
        for spec in outputs {
            validate_output(spec)?;
        }
        let metadata = export_metadata(&project, &settings.metadata);
        let provider = self.media.provider();
        let renderer = OfflineRenderer::new(&self.plugins, &provider);
        let (start, _) = range_frames(&project, request)?;
        let cancelled = || job.is_some_and(JobContext::is_cancelled);
        let render_progress = |fraction: f64| {
            if let Some(job) = job {
                job.progress(fraction * EXPORT_RENDER_SHARE, Some("rendering"));
            }
        };
        let failure = RefCell::new(None);
        let failed = Cell::new(false);
        let mut blocks = Blocks {
            renderer: &renderer,
            project: &project,
            request,
            stream: None,
            cancelled: &cancelled,
            progress: &render_progress,
            failure: &failure,
            failed: &failed,
            done: false,
        };
        let stop = || failed.get() || cancelled();
        // The exporter's callback is `'static`: it reports through an owned handle.
        let reporter = job.map(JobContext::progress_handle);
        let progress = move |fraction: f64, message: &str| {
            if let Some(reporter) = &reporter {
                reporter.progress(fraction, Some(message));
            }
        };
        let exporter = Exporter::new(workspace.join("cache").join("export"))
            .with_clock(Arc::clone(&self.clock));
        let outcome = exporter.export(
            &mut blocks,
            project.settings.sample_rate,
            outputs,
            &ExportOptions {
                normalize: settings.normalize,
                metadata: &metadata,
                overwrite: settings.overwrite,
                time_reference: u64::try_from(start).unwrap_or(0),
                cancelled: Some(&stop),
                progress: Some(&progress),
                ogg_serial: None,
            },
        );
        let stats = blocks.stream.take().map(RenderStream::into_stats);
        drop(blocks);
        // A render failure stops the exporter (as cancelled); the render's error is the cause.
        if let Some(err) = failure.into_inner() {
            return Err(err.into());
        }
        let mut result = outcome?;
        if let Some(stats) = stats {
            result.warnings.splice(0..0, stats.warnings);
        }
        Ok(result)
    }

    fn export_stems(
        &self,
        directory: &Path,
        track_ids: Option<&[String]>,
        template: &OutputSpec,
        request: &RenderRequest,
        options: StemOptions,
        job: Option<&JobContext<'_>>,
    ) -> AppResult<JsonMap> {
        let (project, _) = self.snapshot()?;
        let track_ids: Vec<String> = match track_ids {
            Some(ids) => ids.to_vec(),
            None => sounding(&project).map(|track| track.id.clone()).collect(),
        };
        if track_ids.is_empty() {
            return Err(DomainError::invalid("no tracks to export as stems").into());
        }
        let mut namer = StemNamer::new(directory, template.container);
        let mut stems = Vec::with_capacity(track_ids.len());
        for (i, track_id) in track_ids.iter().enumerate() {
            let track = project.track(track_id)?;
            let spec = OutputSpec {
                path: namer.path(&project.name, i, &track.name),
                ..template.clone()
            };
            let alone = RenderRequest {
                target: Some(track_id.clone()),
                bypass_master: !options.through_master,
                ..request.clone()
            };
            if let Some(job) = job {
                job.check()?;
                job.progress(
                    frames_to_f64(i) / frames_to_f64(track_ids.len()),
                    Some(&format!("stem {}", track.name)),
                );
            }
            let settings = ExportSettings {
                overwrite: options.overwrite,
                ..ExportSettings::default()
            };
            let exported = self.export(&[spec], &alone, &settings, None)?;
            let mut stem = JsonMap::new();
            stem.insert("track_id".to_owned(), Value::from(track_id.clone()));
            stem.insert("name".to_owned(), Value::from(track.name.clone()));
            if let Some(output) = exported.outputs.first() {
                stem.extend(into_map(output.to_json()));
            }
            stem.insert(
                "peak_dbtp".to_owned(),
                level_json(exported.source_analysis.true_peak_dbtp),
            );
            stem.insert(
                "integrated_lufs".to_owned(),
                level_json(exported.source_analysis.integrated_lufs),
            );
            stems.push(Value::Object(stem));
        }
        let mut out = JsonMap::new();
        out.insert(
            "directory".to_owned(),
            Value::from(directory.display().to_string()),
        );
        out.insert("stems".to_owned(), Value::Array(stems));
        out.insert(
            "through_master".to_owned(),
            Value::from(options.through_master),
        );
        Ok(out)
    }
}

/// The render pass of a preview: the stream analyzed in its `float64` blocks and written as
/// 32-bit floats to `partial`. Returns the render's statistics and analysis.
fn write_preview(
    renderer: &OfflineRenderer<'_>,
    project: &Project,
    request: &RenderRequest,
    partial: &Path,
    clock: &WallClock,
    job: Option<&JobContext<'_>>,
) -> AppResult<(RenderStats, AnalysisReport)> {
    let sample_rate = project.settings.sample_rate;
    let mut analyzer = analyzer(sample_rate)?;
    let mut stream = renderer.stream(project, request)?;
    let mut writer = PcmFileWriter::create(
        partial,
        PcmContainer::Wav,
        SampleFormat::Float,
        2,
        sample_rate,
        &[],
        Arc::clone(clock),
    )?;
    let cancelled = || job.is_some_and(JobContext::is_cancelled);
    let mut interleaved = Vec::new();
    let mut narrow = Vec::new();
    while let Some(block) = stream.next_block(Some(&cancelled))? {
        interleave(block, &mut interleaved);
        analyzer.feed(&interleaved);
        narrow.clear();
        narrow.extend(interleaved.iter().copied().map(to_f32));
        writer.write_f32(&narrow)?;
        if let Some(job) = job {
            job.progress(stream.progress() * PREVIEW_RENDER_SHARE, Some("rendering"));
        }
    }
    writer.finish()?;
    Ok((stream.into_stats(), analyzer.finish()))
}

/// The render as the exporter's block source: the stream is built on the first block, so the
/// exporter checks its outputs first, as the prototype's lazy `render_iter` let it. A render
/// error ends the stream: it is kept in `failure`, and one empty block follows so that the
/// exporter polls its cancellation, which `failed` then reports.
struct Blocks<'a> {
    renderer: &'a OfflineRenderer<'a>,
    project: &'a Project,
    request: &'a RenderRequest,
    stream: Option<RenderStream>,
    cancelled: &'a dyn Fn() -> bool,
    progress: &'a dyn Fn(f64),
    failure: &'a RefCell<Option<EngineError>>,
    failed: &'a Cell<bool>,
    done: bool,
}

impl Blocks<'_> {
    fn next_block(&mut self) -> Result<Option<Vec<f64>>, EngineError> {
        if self.stream.is_none() {
            self.stream = Some(self.renderer.stream(self.project, self.request)?);
        }
        let Some(stream) = self.stream.as_mut() else {
            return Ok(None);
        };
        let Some(block) = stream.next_block(Some(self.cancelled))? else {
            return Ok(None);
        };
        let interleaved: Vec<f64> = block.iter().flatten().copied().collect();
        (self.progress)(stream.progress());
        Ok(Some(interleaved))
    }
}

impl Iterator for Blocks<'_> {
    type Item = Vec<f64>;

    fn next(&mut self) -> Option<Vec<f64>> {
        if self.done {
            return None;
        }
        match self.next_block() {
            Ok(Some(block)) => Some(block),
            Ok(None) => {
                self.done = true;
                None
            }
            Err(err) => {
                self.done = true;
                *self.failure.borrow_mut() = Some(err);
                self.failed.set(true);
                Some(Vec::new())
            }
        }
    }
}

/// The metadata of an export: the project's (`title` falls back to the project name,
/// `software` is `Sonara`), then `extra` in order.
fn export_metadata(project: &Project, extra: &[(String, String)]) -> Vec<(String, String)> {
    let m = &project.metadata;
    let title = if m.title.is_empty() {
        project.name.clone()
    } else {
        m.title.clone()
    };
    let mut out = vec![
        ("title".to_owned(), title),
        ("artist".to_owned(), m.artist.clone()),
        ("album".to_owned(), m.album.clone()),
        ("genre".to_owned(), m.genre.clone()),
        ("comment".to_owned(), m.comment.clone()),
        ("software".to_owned(), SOFTWARE.to_owned()),
    ];
    out.extend(extra.iter().cloned());
    out
}

fn analyzer(sample_rate: u32) -> AppResult<Analyzer> {
    Analyzer::new(sample_rate, 2).map_err(|err| DomainError::invalid(err.to_string()).into())
}

/// Stereo frames as interleaved samples, into `out`.
fn interleave(block: &[[f64; 2]], out: &mut Vec<f64>) {
    out.clear();
    out.extend(block.iter().flatten().copied());
}

/// `block.astype(np.float32)`: round to the nearest `f32`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the float32 storage of the preview, rounded to nearest as NumPy's astype"
)]
fn to_f32(value: f64) -> f32 {
    value as f32
}

#[expect(
    clippy::cast_precision_loss,
    reason = "frame and track counts stay far below 2^52"
)]
fn frames_to_f64(frames: usize) -> f64 {
    frames as f64
}

/// `.<name>.partial` next to `path`.
fn partial_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!(".{name}.partial"))
}

/// `path.with_suffix(".analysis.json")`.
fn sidecar_path(path: &Path) -> PathBuf {
    with_suffix(path, ".analysis.json")
}

/// The object of a JSON value built as one (`to_json`); anything else as an empty map.
fn into_map(value: Value) -> JsonMap {
    match value {
        Value::Object(map) => map,
        _ => JsonMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use serde_json::{Value, json};
    use sonara_core::ids::SequentialIds;
    use sonara_core::model::project::Project;
    use sonara_dsp::analysis::Analyzer;

    use super::{
        PreviewResult, RenderStats, TrackLevels, export_metadata, partial_path, sidecar_path,
    };

    fn report() -> sonara_dsp::analysis::AnalysisReport {
        let mut analyzer = Analyzer::new(48_000, 2).expect("an analyzer");
        let block: Vec<f64> = (0..96_000_u32)
            .map(|i| 0.25 * (f64::from(i / 2) * 0.05).sin())
            .collect();
        analyzer.feed(&block);
        analyzer.finish()
    }

    #[test]
    fn preview_json_has_the_prototype_order_and_rounding() {
        let silent = TrackLevels::measured("Pad", &Analyzer::new(48_000, 2).unwrap().finish());
        let result = PreviewResult {
            path: PathBuf::from("/w/p.wav"),
            // Python: round(33.8565, 3) == 33.856 (33.856499… in binary).
            seconds: 33.8565,
            sample_rate: 48_000,
            start_beat: 0.0,
            end_beat: 64.0,
            // CPython 3.13: round(2.0004, 3) == 2.0, round(12.345, 2) == 12.35 (12.34500…01 in
            // binary).
            render_seconds: 2.0004,
            realtime_factor: 12.345,
            analysis: report(),
            stats: RenderStats {
                frames: 1_625_088,
                latency: 102,
                tail_frames: 12_288,
                warnings: vec!["w".to_owned()],
                missing_media: Vec::new(),
            },
            track_levels: vec![("trk_1".to_owned(), silent)],
        };
        let json = result.to_json();
        let keys: Vec<&str> = json.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "path",
                "seconds",
                "sample_rate",
                "range_beats",
                "render_seconds",
                "realtime_factor",
                "latency_samples",
                "tail_seconds",
                "warnings",
                "missing_media",
                "analysis",
                "issues",
                "track_levels"
            ]
        );
        assert_eq!(json["seconds"], json!(33.856));
        assert_eq!(json["render_seconds"], json!(2.0));
        assert_eq!(json["realtime_factor"], json!(12.35));
        // 12288 / 48000 = 0.256.
        assert_eq!(json["tail_seconds"], json!(0.256));
        assert_eq!(json["range_beats"], json!([0.0, 64.0]));
        assert_eq!(json["latency_samples"], json!(102));
        assert!(json["analysis"].get("short_term_lufs").is_none());
        // Silence measures as `None` (`_num`), in the prototype's key order.
        let levels = &json["track_levels"]["trk_1"];
        let keys: Vec<&str> = levels
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "name",
                "integrated_lufs",
                "peak_dbfs",
                "short_term_max_lufs"
            ]
        );
        assert_eq!(levels["integrated_lufs"], Value::Null);
        // Without levels, the key is left out.
        let plain = PreviewResult {
            track_levels: Vec::new(),
            ..result
        };
        assert!(!plain.to_json().contains_key("track_levels"));
    }

    #[test]
    fn levels_round_to_two_decimals() {
        let levels = TrackLevels::measured("Tone", &report());
        let peak = levels.peak_dbfs.expect("a level");
        assert_eq!(
            peak,
            sonara_core::pyfmt::py_round(report().sample_peak_dbfs, 2)
        );
    }

    #[test]
    fn export_metadata_defaults_to_the_project() {
        let mut ids = SequentialIds::new(0);
        let project = Project::create(&mut ids, "Song", 48_000, 120.0).unwrap();
        let extra = [("artist".to_owned(), "Me".to_owned())];
        let pairs = |p: &Project| -> Vec<(String, String)> { export_metadata(p, &extra) };
        let meta = pairs(&project);
        assert_eq!(meta[0], ("title".to_owned(), "Song".to_owned()));
        assert_eq!(meta[5], ("software".to_owned(), "Sonara".to_owned()));
        // The extra pairs follow; the exporter applies them like a dict update.
        assert_eq!(meta[6], ("artist".to_owned(), "Me".to_owned()));
        let titled = project
            .edit(|data| "Real Title".clone_into(&mut data.metadata.title))
            .unwrap();
        assert_eq!(pairs(&titled)[0].1, "Real Title");
    }

    #[test]
    fn file_names_follow_the_prototype() {
        assert_eq!(
            partial_path(Path::new("/w/previews/full.wav")),
            Path::new("/w/previews/.full.wav.partial")
        );
        // `Path.with_suffix(".analysis.json")`.
        for (wav, sidecar) in [
            ("/w/full.wav", "/w/full.analysis.json"),
            ("/w/a.b.wav", "/w/a.b.analysis.json"),
            ("/w/plain", "/w/plain.analysis.json"),
            ("/w/trailing.", "/w/trailing..analysis.json"),
        ] {
            assert_eq!(sidecar_path(Path::new(wav)), Path::new(sidecar));
        }
    }
}
