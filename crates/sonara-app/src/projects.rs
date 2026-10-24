//! The project lifecycle (`sonara.application.services.project_service`): new, open, save,
//! save as, save a copy, autosave, close, recovery and media integrity. The file side builds on
//! the `sonara-project` store.
//!
//! **Paths.** User paths are expanded (`~`) and resolved like Python's `Path.resolve()` before
//! they reach the store, so tool results spell them as the prototype does (on macOS,
//! `/var/…` becomes `/private/var/…`).
//!
//! **Locks.** The session lock is held only to read a snapshot or to make one session call,
//! never across file I/O and never while the settings lock is held.
//!
//! **Deviation.** Before another project replaces the open one, the prototype removes the open
//! project's clean, never-saved scratch workspace. Here that removal happens after the new
//! project is loaded, and never removes the workspace being loaded (the prototype would delete
//! the media of `recover_session` on the open project's own clean scratch workspace).

use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use sonara_core::commands::CommandContext;
use sonara_core::commands::project::ReplaceAsset;
use sonara_core::error::{DomainError, ErrorCode, Result};
use sonara_core::ids::IdGenerator;
use sonara_core::model::media::{MediaAsset, MediaLocation};
use sonara_core::model::project::{Project, ProjectMetadata};
use sonara_core::plugins::PluginRegistry;
use sonara_core::session::{ProjectLocation, ProjectSession};
use sonara_core::time::{TimeSignatureMap, TimeSignaturePoint};
use sonara_project::atomic::{atomic_copy, link_or_copy, sha256_file};
use sonara_project::store::{
    IntegrityReport, Layout, LoadedProject, ProjectStore, SaveResult, asset_path, detect_layout,
};
use walkdir::WalkDir;

use crate::clock::{WallClock, iso_seconds, system_clock};
use crate::error::{AppResult, IoContext as _};
use crate::ids::IdSource;
use crate::paths::{expand_user, resolve};
use crate::session::SharedSession;
use crate::settings::{Settings, save_settings};
use crate::sync::lock;
use crate::templates::apply_template;

/// The extension of project paths.
pub const SUFFIX: &str = ".sonara";

/// What a new project starts with.
#[derive(Debug, Clone, PartialEq)]
pub struct NewProject {
    /// Project name, also the metadata title.
    pub name: String,
    /// Sample rate in Hz.
    pub sample_rate: i64,
    /// Initial tempo in BPM.
    pub bpm: f64,
    /// Initial time signature (numerator, denominator).
    pub time_signature: (u32, u32),
    /// Template ID (`empty`, `electronic`, `band`, `orchestral`).
    pub template: String,
    /// Explicit song length in beats.
    pub length_beats: Option<f64>,
}

impl Default for NewProject {
    fn default() -> Self {
        Self {
            name: "Untitled".to_owned(),
            sample_rate: 48_000,
            bpm: 120.0,
            time_signature: (4, 4),
            template: "empty".to_owned(),
            length_beats: None,
        }
    }
}

/// The New Project dialog's time signatures (the field is editable).
pub const NEW_PROJECT_SIGNATURES: [&str; 6] = ["4/4", "3/4", "6/8", "5/4", "7/8", "12/8"];
/// The New Project dialog's sample rates in Hz; 48 kHz is chosen first.
pub const NEW_PROJECT_RATES: [i64; 4] = [44_100, 48_000, 88_200, 96_000];
/// The New Project dialog's tempo range in BPM (two decimals).
pub const NEW_PROJECT_BPM: (f64, f64) = (10.0, 999.0);

/// A field the New Project dialog got wrong: the dialog shows the message and stays open.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FieldError {
    /// The field: `name`, `template`, `bpm`, `time_signature` or `sample_rate`.
    pub field: &'static str,
    /// The error code (`INVALID_ARGUMENT`, `NOT_FOUND`).
    pub code: &'static str,
    /// What is wrong, worded for the dialog.
    pub message: String,
}

impl FieldError {
    fn new(field: &'static str, error: &DomainError) -> Self {
        Self {
            field,
            code: error.code.as_str(),
            message: error.message.clone(),
        }
    }
}

/// The New Project dialog's fields as the page sends them: the time signature as typed.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewProjectForm {
    /// The name; blank is "Untitled".
    pub name: String,
    /// The template ID.
    pub template: String,
    /// The tempo in BPM.
    pub bpm: f64,
    /// The time signature, `n/d`.
    pub time_signature: String,
    /// The sample rate in Hz.
    pub sample_rate: i64,
}

impl Default for NewProjectForm {
    /// The dialog's fields as it opens: "Untitled", the empty template, 120 BPM, 4/4, 48 kHz.
    fn default() -> Self {
        Self {
            name: "Untitled".to_owned(),
            template: "empty".to_owned(),
            bpm: 120.0,
            time_signature: "4/4".to_owned(),
            sample_rate: 48_000,
        }
    }
}

impl NewProjectForm {
    /// What the dialog's `values()` gives (the name stripped, blank as "Untitled"; the tempo
    /// clamped and held with two decimals, as its spin box holds it), checked field by field.
    ///
    /// The prototype raised a `ValueError` for a time signature it
    /// could not parse (`x/4`, `4`) and let the domain's rejection escape, so nothing happened
    /// and nothing said why. Here each field's error comes back, and the project is built in
    /// memory before anything else happens ([`ProjectService::check_new_project`]).
    ///
    /// # Errors
    ///
    /// The first field that is wrong: an unknown template (`NOT_FOUND`), a tempo that is not
    /// a number, a time signature that is not `n/d` or that the project rejects, a sample rate
    /// the dialog does not offer (`INVALID_ARGUMENT`).
    pub fn to_new_project(&self) -> std::result::Result<NewProject, FieldError> {
        let name = self.name.trim();
        let name = if name.is_empty() { "Untitled" } else { name };
        crate::templates::template(&self.template).map_err(|e| FieldError::new("template", &e))?;
        if !self.bpm.is_finite() {
            return Err(FieldError::new(
                "bpm",
                &DomainError::invalid("the tempo must be a number"),
            ));
        }
        let (low, high) = NEW_PROJECT_BPM;
        let bpm = crate::settings_dialog::qt_round(self.bpm.clamp(low, high), 2);
        let time_signature = parse_time_signature(&self.time_signature)
            .map_err(|e| FieldError::new("time_signature", &e))?;
        if !NEW_PROJECT_RATES.contains(&self.sample_rate) {
            return Err(FieldError::new(
                "sample_rate",
                &DomainError::invalid(format!(
                    "{} Hz is not a sample rate the dialog offers",
                    self.sample_rate
                )),
            ));
        }
        Ok(NewProject {
            name: name.to_owned(),
            sample_rate: self.sample_rate,
            bpm,
            time_signature,
            template: self.template.clone(),
            length_beats: None,
        })
    }
}

/// `n/d` with whitespace around either number, as Python's `int` takes them, checked as the
/// project's first time signature.
///
/// # Errors
///
/// `INVALID_ARGUMENT`: `invalid time signature '…'` for a text that is not two integers around
/// one `/`, or the signature's own error (a denominator that is not a power of two, …).
fn parse_time_signature(text: &str) -> Result<(u32, u32)> {
    let invalid = || DomainError::invalid(format!("invalid time signature '{text}'"));
    let mut parts = text.split('/');
    let (Some(numerator), Some(denominator), None) = (parts.next(), parts.next(), parts.next())
    else {
        return Err(invalid());
    };
    let numerator: i64 = numerator.trim().parse().map_err(|_| invalid())?;
    let denominator: i64 = denominator.trim().parse().map_err(|_| invalid())?;
    let (Ok(numerator), Ok(denominator)) = (u32::try_from(numerator), u32::try_from(denominator))
    else {
        return Err(DomainError::invalid(format!(
            "time signature {numerator}/{denominator} is out of range"
        )));
    };
    TimeSignaturePoint::new(0, numerator, denominator)?;
    Ok((numerator, denominator))
}

/// A template of the New Project dialog: its ID, name and description (the tooltip).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TemplateChoice {
    /// The ID the form sends.
    pub id: &'static str,
    /// The name shown.
    pub name: &'static str,
    /// The tooltip.
    pub description: &'static str,
}

/// What the New Project dialog shows.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct NewProjectDialogView {
    /// The fields as it opens.
    pub form: NewProjectForm,
    /// The templates, in the prototype's order.
    pub templates: Vec<TemplateChoice>,
    /// The time signatures offered (the field takes others).
    pub signatures: [&'static str; 6],
    /// The sample rates: "44.1 kHz" and so on, with their values.
    pub rates: Vec<(String, i64)>,
    /// The tempo's range.
    pub bpm: (f64, f64),
}

/// The New Project dialog as it opens.
#[must_use]
pub fn new_project_dialog() -> NewProjectDialogView {
    NewProjectDialogView {
        form: NewProjectForm::default(),
        templates: crate::templates::TEMPLATES
            .iter()
            .map(|t| TemplateChoice {
                id: t.id,
                name: t.name,
                description: t.description,
            })
            .collect(),
        signatures: NEW_PROJECT_SIGNATURES,
        rates: NEW_PROJECT_RATES
            .iter()
            .map(|&rate| (format!("{} kHz", rate_khz(rate)), rate))
            .collect(),
        bpm: NEW_PROJECT_BPM,
    }
}

/// `f"{rate / 1000:g}"`: 44.1, 48, 88.2, 96.
fn rate_khz(rate: i64) -> String {
    let thousands = rate / 1000;
    let rest = rate % 1000;
    if rest == 0 {
        thousands.to_string()
    } else {
        format!("{thousands}.{}", format!("{rest:03}").trim_end_matches('0'))
    }
}

/// A new project: name, tempo, meter, length and metadata, then the template's tracks.
/// `created` is the creation timestamp (`%Y-%m-%dT%H:%M:%SZ`, UTC).
///
/// # Errors
///
/// A validation error for an invalid name, rate, tempo, meter or length, or an unknown
/// template.
pub fn build_new_project(
    options: &NewProject,
    created: &str,
    ctx: &CommandContext<'_>,
    ids: &mut dyn IdGenerator,
) -> Result<Project> {
    let project = Project::create(ids, options.name.clone(), options.sample_rate, options.bpm)?;
    let (numerator, denominator) = options.time_signature;
    let signatures =
        TimeSignatureMap::new(vec![TimeSignaturePoint::new(0, numerator, denominator)?])?;
    let settings = sonara_core::model::project::ProjectSettings::new(
        i64::from(project.settings.sample_rate),
        options.length_beats,
        project.settings.record_subtype.clone(),
    )?;
    let project = project.edit(|d| {
        d.time_signatures = signatures;
        d.metadata = ProjectMetadata {
            title: options.name.clone(),
            created: created.to_owned(),
            ..ProjectMetadata::default()
        };
        d.settings = settings;
    })?;
    apply_template(&project, &options.template, ctx, ids)
}

/// `path` with `~` expanded and the `.sonara` extension added unless it has it (in any case),
/// resolved like Python's `Path.resolve()`.
///
/// # Errors
///
/// [`ErrorCode::Internal`]: `<path> has an empty name`, for a path without a last component
/// (`/`), where the prototype raises a `ValueError`.
pub fn normalize_project_path(path: &Path) -> AppResult<PathBuf> {
    let path = expand_user(path);
    // Python's `Path()` drops `.` segments; `..` is a name like any other.
    let mut components: Vec<Component<'_>> = path
        .components()
        .filter(|c| !matches!(c, Component::CurDir))
        .collect();
    let name = match components.last() {
        Some(Component::Normal(name)) => name.to_os_string(),
        Some(Component::ParentDir) => OsString::from(".."),
        _ => {
            return Err(DomainError::new(
                ErrorCode::Internal,
                format!("{} has an empty name", path.display()),
            )
            .into());
        }
    };
    if py_suffix(&name.to_string_lossy()).to_lowercase() == SUFFIX {
        return Ok(resolve(&path));
    }
    components.pop();
    let mut named: PathBuf = components.iter().collect();
    let mut file_name = name;
    file_name.push(SUFFIX);
    named.push(file_name);
    Ok(resolve(&named))
}

/// Python's `PurePath.suffix` of a file name: from the last dot, unless the dot is first or
/// last (`.sonara` and `a.` have none).
fn py_suffix(name: &str) -> &str {
    match name.rfind('.') {
        Some(dot) if dot > 0 && dot + 1 < name.len() => &name[dot..],
        _ => "",
    }
}

/// The audio facts relinking compares and records (the prototype's `soundfile.info`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbedAudio {
    /// Frames per channel.
    pub frames: u64,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Channel count.
    pub channels: u32,
}

/// Reads the facts of an audio file for [`ProjectService::relink`].
///
/// The application context gives its service `sonara-media`'s inspection
/// ([`crate::media::SoundFileProbe`]); a service built without a probe fails to relink with
/// [`ErrorCode::Unsupported`] instead of recording unchecked facts.
pub trait MediaProbe: Send + Sync {
    /// The facts of the audio file at `path`.
    ///
    /// # Errors
    ///
    /// An error when the file cannot be read as audio.
    fn probe(&self, path: &Path) -> AppResult<ProbedAudio>;
}

/// The outcome of [`ProjectService::relink`].
#[derive(Debug, Clone, PartialEq)]
pub struct Relinked {
    /// The asset.
    pub asset_id: String,
    /// The asset's new path (bundle-relative when embedded).
    pub path: String,
    /// Where the asset's file lives now.
    pub location: MediaLocation,
    /// What differs from the original file.
    pub warnings: Vec<String>,
    /// Whether the asset record changed.
    pub committed: bool,
}

impl Relinked {
    /// The prototype's result: `asset_id`, `path`, `location`, `warnings`, `committed`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "asset_id": self.asset_id,
            "path": self.path,
            "location": self.location.as_str(),
            "warnings": self.warnings,
            "committed": self.committed,
        })
    }
}

/// An unsaved project that a scratch workspace's autosave can restore (the prototype's
/// Recovery dialog).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RecoverableSession {
    /// The scratch workspace (`<workspaces>/new-…`).
    pub workspace: PathBuf,
    /// The project's name, from the autosave's `info.json`.
    pub name: Option<String>,
    /// When the autosave was written (`saved_at` of `info.json`).
    pub saved_at: Option<String>,
}

/// Opening, saving and closing the session's project.
pub struct ProjectService {
    session: SharedSession,
    plugins: Arc<PluginRegistry>,
    store: ProjectStore,
    settings: Arc<Mutex<Settings>>,
    /// Where recent projects are persisted; `None` keeps them in memory.
    settings_file: Option<PathBuf>,
    ids: IdSource,
    clock: WallClock,
    media_probe: Option<Arc<dyn MediaProbe>>,
    last_load: Mutex<Option<LoadedProject>>,
}

impl std::fmt::Debug for ProjectService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectService")
            .field("store", &self.store)
            .field("settings_file", &self.settings_file)
            .field("media_probe", &self.media_probe.is_some())
            .finish_non_exhaustive()
    }
}

impl ProjectService {
    /// A service for `session`, storing projects with `store`, remembering recent projects in
    /// `settings` (in memory until [`Self::persisting_settings`]), drawing IDs from `ids` and
    /// reading the system clock.
    #[must_use]
    pub fn new(
        session: SharedSession,
        store: ProjectStore,
        settings: Arc<Mutex<Settings>>,
        ids: IdSource,
    ) -> Self {
        let plugins = Arc::clone(session.lock().plugins());
        Self {
            session,
            plugins,
            store,
            settings,
            settings_file: None,
            ids,
            clock: system_clock(),
            media_probe: None,
            last_load: Mutex::new(None),
        }
    }

    /// The service saving the settings to `path` whenever the recent projects change.
    #[must_use]
    pub fn persisting_settings(mut self, path: PathBuf) -> Self {
        self.settings_file = Some(path);
        self
    }

    /// The settings file the service saves to; `None` while the settings stay in memory
    /// ([`Self::persisting_settings`]). The Settings dialog saves to the same file
    /// ([`crate::settings_dialog::apply_and_save`]).
    #[must_use]
    pub fn settings_file(&self) -> Option<&Path> {
        self.settings_file.as_deref()
    }

    /// The service stamping new projects with `clock`'s time.
    #[must_use]
    pub fn with_clock(mut self, clock: WallClock) -> Self {
        self.clock = clock;
        self
    }

    /// The service relinking media with `probe`.
    #[must_use]
    pub fn with_media_probe(mut self, probe: Arc<dyn MediaProbe>) -> Self {
        self.media_probe = Some(probe);
        self
    }

    /// The store.
    #[must_use]
    pub fn store(&self) -> &ProjectStore {
        &self.store
    }

    /// What the last [`Self::open_project`] read.
    #[must_use]
    pub fn last_load(&self) -> Option<LoadedProject> {
        lock(&self.last_load).clone()
    }

    // -- create / open --------------------------------------------------------------------

    /// Creates a project in a fresh scratch workspace and makes it the open project, clean
    /// until the first edit.
    ///
    /// # Errors
    ///
    /// The errors of [`build_new_project`], or [`crate::error::AppError::Store`] when the
    /// scratch workspace cannot be created.
    pub fn new_project(&self, options: &NewProject) -> AppResult<Project> {
        let created = iso_seconds((self.clock)());
        let ctx = CommandContext {
            plugins: &self.plugins,
        };
        let mut ids = self.ids.clone();
        let project = build_new_project(options, &created, &ctx, &mut ids)?;
        let workspace = self.store.scratch_workspace(&project.id)?;
        let location = ProjectLocation {
            path: None,
            workspace,
            packaged: false,
        };
        // A pristine new project has nothing to lose: it is clean until the first edit.
        self.replace_open_project(project.clone(), location, true);
        Ok(project)
    }

    /// Builds the project `options` describe in memory, without opening it, without drawing IDs
    /// from the application's source and without touching the session or the disk: the New
    /// Project dialog checks the choices before the unsaved-changes prompt.
    ///
    /// # Errors
    ///
    /// The errors of [`build_new_project`].
    pub fn check_new_project(&self, options: &NewProject) -> AppResult<()> {
        let ctx = CommandContext {
            plugins: &self.plugins,
        };
        let mut ids = IdSource::random();
        build_new_project(options, &iso_seconds((self.clock)()), &ctx, &mut ids)?;
        Ok(())
    }

    /// Opens the project at `path` (bundle or archive). With `recover_autosave`, a newer
    /// autosave replaces the saved project and the project is dirty; a project recovered from
    /// a backup is dirty too.
    ///
    /// # Errors
    ///
    /// The errors of [`ProjectStore::open`].
    pub fn open_project(&self, path: &Path, recover_autosave: bool) -> AppResult<LoadedProject> {
        let path = resolve(&expand_user(path));
        let loaded = self.store.open(&path, recover_autosave)?;
        let location = ProjectLocation {
            path: Some(loaded.path.clone()),
            workspace: loaded.workspace.clone(),
            packaged: loaded.layout == Layout::Archive,
        };
        let clean = loaded
            .recovered_from
            .as_deref()
            .is_none_or(|from| from.is_empty() || from.starts_with("project.json"));
        self.replace_open_project(loaded.project.clone(), location, clean && !recover_autosave);
        *lock(&self.last_load) = Some(loaded.clone());
        self.remember(&loaded.path);
        Ok(loaded)
    }

    /// Restores an unsaved project from its scratch workspace's autosave (dirty).
    ///
    /// # Errors
    ///
    /// The errors of [`ProjectStore::read_autosave`].
    pub fn recover_session(&self, workspace: &Path) -> AppResult<Project> {
        let project = self.store.read_autosave(workspace)?;
        let location = ProjectLocation {
            path: None,
            workspace: workspace.to_path_buf(),
            packaged: false,
        };
        self.replace_open_project(project.clone(), location, false);
        Ok(project)
    }

    /// The never-saved projects that a scratch workspace's autosave can restore, sorted by
    /// workspace: the sessions of [`ProjectStore::recoverable_sessions`] whose workspace is
    /// named `new-…`, as the prototype's `offer_recovery` selects them. Extracted
    /// archives are left out: opening their project offers its autosave.
    ///
    /// # Errors
    ///
    /// [`crate::error::AppError::Store`] when the workspaces folder cannot be listed.
    pub fn recoverable_scratch_sessions(&self) -> AppResult<Vec<RecoverableSession>> {
        let text = |map: &serde_json::Map<String, Value>, key: &str| {
            map.get(key).and_then(Value::as_str).map(str::to_owned)
        };
        Ok(self
            .store
            .recoverable_sessions()?
            .into_iter()
            .filter_map(|session| {
                let workspace = PathBuf::from(text(&session, "workspace")?);
                let scratch = workspace
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("new-"));
                scratch.then(|| RecoverableSession {
                    name: text(&session, "name"),
                    saved_at: text(&session, "saved_at"),
                    workspace,
                })
            })
            .collect())
    }

    /// Drops the open project's autosave when it has unsaved changes, because the user
    /// discarded them on quitting (the prototype's `closeEvent`), so that the next start does
    /// not offer them for recovery. Returns whether there was a dirty project to do it for.
    pub fn discard_autosave(&self) -> bool {
        let workspace = {
            let session = self.session.lock();
            session
                .location()
                .filter(|_| session.is_dirty())
                .map(|location| location.workspace.clone())
        };
        if let Some(workspace) = &workspace {
            self.store.discard_autosave(workspace);
        }
        workspace.is_some()
    }

    /// Loads `project`, then removes the replaced project's disposable scratch workspace.
    fn replace_open_project(&self, project: Project, location: ProjectLocation, saved: bool) {
        let scratch = {
            let mut session = self.session.lock();
            let scratch = self
                .disposable_scratch(&session)
                .filter(|dir| *dir != location.workspace);
            session.load(project, location, saved);
            scratch
        };
        remove_scratch(scratch.as_deref());
    }

    // -- save -----------------------------------------------------------------------------

    /// Saves the open project in place, or to `path` (Save As, `.sonara` added). Without
    /// `packaged`, an existing target keeps its layout, and a new target is an archive only
    /// when the project is an archive saved in place. A bundle receives every media file of
    /// the old workspace, also those only reachable through undo; a scratch workspace left
    /// behind is removed.
    ///
    /// # Errors
    ///
    /// - [`ErrorCode::NoProject`];
    /// - [`ErrorCode::InvalidArgument`]: `the project has never been saved; a path is
    ///   required`;
    /// - the errors of [`normalize_project_path`] and [`ProjectStore::save`];
    /// - [`crate::error::AppError::Io`] when carrying media fails.
    pub fn save(&self, path: Option<&Path>, packaged: Option<bool>) -> AppResult<SaveResult> {
        let (project, location) = self.snapshot()?;
        let target = match path {
            Some(path) => normalize_project_path(path)?,
            None => location.path.clone().ok_or_else(|| {
                DomainError::invalid("the project has never been saved; a path is required")
            })?,
        };
        let layout = match packaged {
            Some(true) => Layout::Archive,
            Some(false) => Layout::Bundle,
            None => {
                let existing = if target.exists() {
                    detect_layout(&target)
                } else {
                    None
                };
                existing.unwrap_or(
                    if location.packaged && location.path.as_ref() == Some(&target) {
                        Layout::Archive
                    } else {
                        Layout::Bundle
                    },
                )
            }
        };
        let old_workspace = location.workspace;
        if layout == Layout::Bundle && resolve(&old_workspace) != resolve(&target) {
            carry_media(&old_workspace, &target)?;
        }
        let result = self
            .store
            .save(&project, &old_workspace, &target, Some(layout))?;
        let saved_at = ProjectLocation {
            path: Some(target.clone()),
            workspace: result.workspace.clone(),
            packaged: layout == Layout::Archive,
        };
        self.session.lock().mark_saved(project, saved_at);
        self.remember(&target);
        if old_workspace != result.workspace && self.is_scratch(&old_workspace) {
            remove_scratch(Some(&old_workspace));
        }
        Ok(result)
    }

    /// Writes a copy to `path` (`.sonara` added) without changing where the open project
    /// lives.
    ///
    /// # Errors
    ///
    /// - [`ErrorCode::NoProject`];
    /// - [`ErrorCode::Conflict`]: `the copy target is the open project itself`;
    /// - the errors of [`normalize_project_path`] and [`ProjectStore::save`];
    /// - [`crate::error::AppError::Io`] when carrying media fails.
    pub fn save_copy(&self, path: &Path, packaged: bool) -> AppResult<SaveResult> {
        let (project, location) = self.snapshot()?;
        let target = normalize_project_path(path)?;
        let layout = if packaged {
            Layout::Archive
        } else {
            Layout::Bundle
        };
        if layout == Layout::Bundle {
            if resolve(&target) == resolve(&location.workspace) {
                return Err(DomainError::new(
                    ErrorCode::Conflict,
                    "the copy target is the open project itself",
                )
                .into());
            }
            carry_media(&location.workspace, &target)?;
        }
        Ok(self
            .store
            .save(&project, &location.workspace, &target, Some(layout))?)
    }

    /// Writes the autosave of a dirty project into its workspace; `None` when no project is
    /// open or it has no unsaved changes.
    ///
    /// # Errors
    ///
    /// The errors of [`ProjectStore::write_autosave`].
    pub fn autosave(&self) -> AppResult<Option<PathBuf>> {
        let snapshot = {
            let session = self.session.lock();
            match (session.project(), session.location()) {
                (Some(project), Some(location)) if session.is_dirty() => {
                    Some((project.clone(), location.workspace.clone()))
                }
                _ => None,
            }
        };
        let Some((project, workspace)) = snapshot else {
            return Ok(None);
        };
        Ok(Some(self.store.write_autosave(&project, &workspace)?))
    }

    // -- close ----------------------------------------------------------------------------

    /// Closes the project; nothing happens without one. A never-saved, clean project's scratch
    /// workspace is removed; a dirty one's stays for crash recovery.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::ConfirmationRequired`]: `the project has unsaved changes`, unless
    /// `discard_changes`.
    pub fn close(&self, discard_changes: bool) -> AppResult<()> {
        let scratch = {
            let mut session = self.session.lock();
            if session.project().is_none() {
                return Ok(());
            }
            if session.is_dirty() && !discard_changes {
                return Err(DomainError::new(
                    ErrorCode::ConfirmationRequired,
                    "the project has unsaved changes",
                )
                .into());
            }
            let scratch = self.disposable_scratch(&session);
            session.close();
            scratch
        };
        remove_scratch(scratch.as_deref());
        Ok(())
    }

    /// Whether `workspace` is one of the store's scratch workspaces (`new-…`).
    fn is_scratch(&self, workspace: &Path) -> bool {
        let named_new = workspace
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("new-"));
        named_new
            && resolve(workspace).parent() == Some(resolve(self.store.workspaces_root()).as_path())
    }

    /// The open project's scratch workspace, when nothing is lost by removing it: never saved,
    /// no unsaved changes.
    fn disposable_scratch(&self, session: &ProjectSession) -> Option<PathBuf> {
        let location = session.location()?;
        (location.path.is_none() && self.is_scratch(&location.workspace) && !session.is_dirty())
            .then(|| location.workspace.clone())
    }

    /// The open project and its location.
    fn snapshot(&self) -> AppResult<(Project, ProjectLocation)> {
        let session = self.session.lock();
        let project = session.require_project()?.clone();
        let location = session.require_location()?.clone();
        Ok((project, location))
    }

    // -- media integrity ------------------------------------------------------------------

    /// Checks the project's media (existence, size and, with `check_hashes`, content) and its
    /// routing.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`], or the errors of [`ProjectStore::verify`].
    pub fn verify(&self, check_hashes: bool) -> AppResult<IntegrityReport> {
        let (project, location) = self.snapshot()?;
        Ok(self
            .store
            .verify(&project, &location.workspace, check_hashes)?)
    }

    /// IDs of the assets whose files are missing.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`].
    pub fn missing_assets(&self) -> AppResult<Vec<String>> {
        let (project, location) = self.snapshot()?;
        Ok(project
            .assets
            .iter()
            .filter(|a| !asset_path(a, &location.workspace).exists())
            .map(|a| a.id.clone())
            .collect())
    }

    /// Embedded media files that neither the project nor any snapshot reachable through undo
    /// or redo refers to.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`].
    pub fn unused_media(&self) -> AppResult<Vec<PathBuf>> {
        let (project, workspace, keep) = {
            let session = self.session.lock();
            let project = session.require_project()?.clone();
            let workspace = session.require_location()?.workspace.clone();
            let keep: Vec<String> = session
                .history()
                .reachable_projects()
                .into_iter()
                .flat_map(|p| p.assets.iter().map(|a| a.path.clone()))
                .collect();
            (project, workspace, keep)
        };
        Ok(self.store.unused_media(&project, &workspace, keep))
    }

    /// Deletes [`Self::unused_media`]; returns the deleted paths.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`], or [`crate::error::AppError::Io`] when a file cannot be
    /// deleted (one that is already gone is fine).
    pub fn delete_unused_media(&self) -> AppResult<Vec<String>> {
        let mut removed = Vec::new();
        for file in self.unused_media()? {
            match fs::remove_file(&file) {
                Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
                    return Err(err).io("delete", &file);
                }
                _ => removed.push(file.to_string_lossy().into_owned()),
            }
        }
        Ok(removed)
    }

    /// Points the asset at `new_path`, checked against the original by hash, length and
    /// sample rate (differences become warnings). With `embed`, the file is copied into the
    /// project's `media/audio`. The change is an undoable command.
    ///
    /// # Errors
    ///
    /// - [`ErrorCode::NoProject`];
    /// - [`ErrorCode::NotFound`]: `asset '<id>' not found`, `<path> does not exist`;
    /// - [`ErrorCode::Unsupported`] while no [`MediaProbe`] is configured;
    /// - the probe's error, an I/O error reading or copying the file, or the command's error.
    pub fn relink(&self, asset_id: &str, new_path: &Path, embed: bool) -> AppResult<Relinked> {
        let (project, location) = self.snapshot()?;
        let asset = project.asset(asset_id)?.clone();
        let new_path = resolve(&expand_user(new_path));
        if !new_path.is_file() {
            return Err(DomainError::new(
                ErrorCode::NotFound,
                format!("{} does not exist", new_path.display()),
            )
            .into());
        }
        let digest = sha256_file(&new_path).io("read", &new_path)?;
        let mut warnings = Vec::new();
        if asset
            .sha256
            .as_deref()
            .is_some_and(|hash| !hash.is_empty() && hash != digest)
        {
            warnings
                .push("the new file's content differs from the original (hash mismatch)".into());
        }
        let probe = self.media_probe.as_ref().ok_or_else(|| {
            DomainError::new(
                ErrorCode::Unsupported,
                "relinking media needs audio file probing, which this build does not have yet",
            )
        })?;
        let info = probe.probe(&new_path)?;
        if info.frames != asset.frames || info.sample_rate != asset.sample_rate {
            warnings.push(
                "the new file differs in length or sample rate; clips may shift or be cut".into(),
            );
        }
        let updated = if embed {
            let rel = if asset.location == MediaLocation::Embedded {
                asset.path.clone()
            } else {
                let name = new_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                format!("media/audio/{}-{name}", asset.id)
            };
            let mut dest = location.workspace.clone();
            dest.extend(rel.split('/').filter(|p| !p.is_empty() && *p != "."));
            atomic_copy(&new_path, &dest).io("copy", &new_path)?;
            let size = fs::metadata(&dest).io("read", &dest)?.len();
            relinked(&asset, rel, MediaLocation::Embedded, &digest, size, info)?
        } else {
            let size = fs::metadata(&new_path).io("read", &new_path)?.len();
            let path = new_path.to_string_lossy().into_owned();
            relinked(&asset, path, MediaLocation::Linked, &digest, size, info)?
        };
        let result = self.session.lock().execute(
            &ReplaceAsset {
                asset: updated.clone(),
            },
            false,
            "user",
        )?;
        Ok(Relinked {
            asset_id: asset_id.to_owned(),
            path: updated.path.clone(),
            location: updated.location,
            warnings,
            committed: result.committed,
        })
    }

    /// Copies every linked asset whose file exists into the project; returns their IDs.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`], or the errors of [`Self::relink`].
    pub fn embed_linked_media(&self) -> AppResult<Vec<String>> {
        let (project, _) = self.snapshot()?;
        let mut done = Vec::new();
        for asset in &project.assets {
            if asset.location == MediaLocation::Linked && Path::new(&asset.path).exists() {
                self.relink(&asset.id, Path::new(&asset.path), true)?;
                done.push(asset.id.clone());
            }
        }
        Ok(done)
    }

    // -- recent projects ------------------------------------------------------------------

    /// Puts `path` first in the recent projects and saves the settings when they persist.
    fn remember(&self, path: &Path) {
        let mut settings = lock(&self.settings);
        settings.add_recent(path);
        if let Some(file) = &self.settings_file
            && let Err(err) = save_settings(&settings, file)
        {
            // ERR-06: as the prototype, a read-only config directory must not fail the open or
            // save that succeeded; the warning reports it.
            log::warn!("could not save settings: {err}");
        }
    }

    /// The recent projects that still exist, newest first.
    #[must_use]
    pub fn recent_projects(&self) -> Vec<String> {
        lock(&self.settings)
            .recent_projects
            .iter()
            .filter(|p| Path::new(p).exists())
            .cloned()
            .collect()
    }
}

/// `asset` pointing at `path`, with the new file's facts.
fn relinked(
    asset: &MediaAsset,
    path: String,
    location: MediaLocation,
    digest: &str,
    size: u64,
    info: ProbedAudio,
) -> Result<MediaAsset> {
    asset.edit(|d| {
        d.path = path;
        d.location = location;
        d.sha256 = Some(digest.to_owned());
        d.size_bytes = size;
        d.frames = info.frames;
        d.sample_rate = info.sample_rate;
        d.channels = info.channels;
    })
}

/// Links or copies every file under `source/media` to the same place under `target`.
fn carry_media(source: &Path, target: &Path) -> AppResult<()> {
    let media = source.join("media");
    if !media.exists() {
        return Ok(());
    }
    // Unreadable directories are skipped, as Python's `rglob` skips them.
    for entry in WalkDir::new(&media)
        .min_depth(1)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        let file = entry.path();
        if file.is_file() {
            let rel = file.strip_prefix(source).unwrap_or(file);
            link_or_copy(file, &target.join(rel)).io("copy", file)?;
        }
    }
    Ok(())
}

/// Removes a scratch workspace.
fn remove_scratch(dir: Option<&Path>) {
    if let Some(dir) = dir {
        // ERR-06: `shutil.rmtree(..., ignore_errors=True)` in the prototype; a leftover
        // scratch workspace is harmless.
        let _ = fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod tests {
    use super::py_suffix;

    #[test]
    fn suffix_follows_pathlib() {
        assert_eq!(py_suffix("a.sonara"), ".sonara");
        assert_eq!(py_suffix(".sonara"), "");
        assert_eq!(py_suffix("d."), "");
        assert_eq!(py_suffix(".."), "");
        assert_eq!(py_suffix("e.sonara.bak"), ".bak");
        assert_eq!(py_suffix("plain"), "");
    }
}
