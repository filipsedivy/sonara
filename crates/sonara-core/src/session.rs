//! The single authoritative owner of the open project (`sonara.application.session`,
//! ADR 0002).
//!
//! The GUI, the CLI, IPC clients and the MCP server edit the project only through
//! [`ProjectSession::execute`], [`ProjectSession::undo`] and [`ProjectSession::redo`]. Readers
//! take [`ProjectSession::project`], an immutable snapshot that is cheap to clone. The session
//! itself is not synchronized; the application wraps it in a lock.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use serde_json::{Value, json};

use crate::commands::{ChangeSet, Command, CommandContext, Flag, run};
use crate::error::{DomainError, ErrorCode, Result};
use crate::model::JsonMap;
use crate::model::project::Project;
use crate::plugins::PluginRegistry;
use crate::undo::{DEFAULT_LIMIT, DEFAULT_MERGE_WINDOW, UndoEntry, UndoHistory};

/// What happened in the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// A command committed.
    Command,
    /// A step was undone.
    Undo,
    /// A step was redone.
    Redo,
    /// A project was opened or created.
    Loaded,
    /// The project was closed.
    Closed,
    /// The project was saved.
    Saved,
}

impl EventKind {
    /// The kind as reported to observers.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Loaded => "loaded",
            Self::Closed => "closed",
            Self::Saved => "saved",
        }
    }
}

/// A change notification for observers (GUI, engine, agents).
#[derive(Debug, Clone, PartialEq)]
pub struct SessionEvent {
    /// What happened.
    pub kind: EventKind,
    /// The project version after the event.
    pub version: u64,
    /// What changed.
    pub changes: ChangeSet,
    /// The command label, if any.
    pub label: String,
    /// Who caused the event.
    pub source: String,
}

/// The result of executing, undoing or redoing.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandResult {
    /// Whether the project changed.
    pub committed: bool,
    /// Whether this was only a simulation.
    pub dry_run: bool,
    /// The project version afterwards.
    pub version: u64,
    /// The operation label.
    pub label: String,
    /// What changed (or would change).
    pub changes: ChangeSet,
    /// Command-specific result data.
    pub result: JsonMap,
}

impl CommandResult {
    /// The agent-facing form; `result` only when it has data.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut out = json!({
            "committed": self.committed,
            "dry_run": self.dry_run,
            "project_version": self.version,
            "operation": self.label,
            "changes": self.changes.to_json(),
        });
        if !self.result.is_empty() {
            out["result"] = Value::Object(self.result.clone());
        }
        out
    }
}

/// Where the open project lives.
///
/// `path` is the user-visible `.sonara` path (`None` until the first save). `workspace` is the
/// directory holding media while the project is open: the bundle itself, an extraction
/// directory for archives, or a scratch directory for unsaved projects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLocation {
    /// The `.sonara` path.
    pub path: Option<PathBuf>,
    /// The media workspace.
    pub workspace: PathBuf,
    /// Whether the project is a ZIP archive.
    pub packaged: bool,
}

/// A source of timestamps in seconds for undo merging.
pub type Clock = Box<dyn Fn() -> f64 + Send + Sync>;

/// Seconds since the first call, from a monotonic clock.
fn monotonic_seconds() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// The open project, its undo history and its observers.
pub struct ProjectSession {
    plugins: Arc<PluginRegistry>,
    project: Option<Project>,
    location: Option<ProjectLocation>,
    history: UndoHistory,
    version: u64,
    saved_project: Option<Project>,
    listeners: Vec<Sender<SessionEvent>>,
    clock: Clock,
}

impl std::fmt::Debug for ProjectSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectSession")
            .field("project", &self.project.as_ref().map(|p| p.id.clone()))
            .field("location", &self.location)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

impl ProjectSession {
    /// A session without a project, with the default undo limit and a monotonic clock.
    #[must_use]
    pub fn new(plugins: Arc<PluginRegistry>) -> Self {
        Self::with_options(plugins, DEFAULT_LIMIT, Box::new(monotonic_seconds))
    }

    /// A session with an undo limit and a clock (a fixed clock makes merging deterministic).
    #[must_use]
    pub fn with_options(plugins: Arc<PluginRegistry>, undo_limit: usize, clock: Clock) -> Self {
        Self {
            plugins,
            project: None,
            location: None,
            history: UndoHistory::new(undo_limit, DEFAULT_MERGE_WINDOW),
            version: 0,
            saved_project: None,
            listeners: Vec::new(),
            clock,
        }
    }

    // -- state ----------------------------------------------------------------------------

    /// The plugin registry.
    #[must_use]
    pub fn plugins(&self) -> &Arc<PluginRegistry> {
        &self.plugins
    }

    /// The open project.
    #[must_use]
    pub fn project(&self) -> Option<&Project> {
        self.project.as_ref()
    }

    /// The open project.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`] when no project is open.
    pub fn require_project(&self) -> Result<&Project> {
        self.project.as_ref().ok_or_else(|| {
            DomainError::new(
                ErrorCode::NoProject,
                "no project is open; create or open one first",
            )
        })
    }

    /// Where the open project lives.
    #[must_use]
    pub fn location(&self) -> Option<&ProjectLocation> {
        self.location.as_ref()
    }

    /// Where the open project lives.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`] when no project is open.
    pub fn require_location(&self) -> Result<&ProjectLocation> {
        self.location
            .as_ref()
            .ok_or_else(|| DomainError::new(ErrorCode::NoProject, "no project is open"))
    }

    /// Increases with every change.
    #[must_use]
    pub fn version(&self) -> u64 {
        self.version
    }

    /// The undo history.
    #[must_use]
    pub fn history(&self) -> &UndoHistory {
        &self.history
    }

    /// Whether the open project differs from the saved one.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        match (&self.project, &self.saved_project) {
            (None, _) => false,
            (Some(project), Some(saved)) => !project.ptr_eq(saved),
            (Some(_), None) => true,
        }
    }

    fn context(&self) -> CommandContext<'_> {
        CommandContext {
            plugins: &self.plugins,
        }
    }

    // -- editing --------------------------------------------------------------------------

    /// Applies `command`. With `dry_run` the project stays unchanged and the result reports
    /// what would change.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`], or the command's error (the project is left unchanged).
    pub fn execute(
        &mut self,
        command: &dyn Command,
        dry_run: bool,
        source: &str,
    ) -> Result<CommandResult> {
        let before = self.require_project()?.clone();
        let outcome = run(command, &before, &self.context())?;
        let label = command.describe();
        if dry_run || outcome.project.ptr_eq(&before) {
            return Ok(CommandResult {
                committed: false,
                dry_run,
                version: self.version,
                label,
                changes: outcome.changes,
                result: outcome.result,
            });
        }
        self.project = Some(outcome.project.clone());
        self.version += 1;
        if command.undoable() {
            self.history.push(UndoEntry {
                label: label.clone(),
                before: before.clone(),
                after: outcome.project.clone(),
                changes: outcome.changes.clone(),
                merge_key: command.merge_key(),
                timestamp: (self.clock)(),
                source: source.to_owned(),
                in_group: false,
            });
        } else if self
            .saved_project
            .as_ref()
            .is_some_and(|s| s.ptr_eq(&before))
        {
            // View-state changes neither enter the history nor make a clean project dirty.
            self.saved_project = Some(outcome.project.clone());
        }
        self.emit(&SessionEvent {
            kind: EventKind::Command,
            version: self.version,
            changes: outcome.changes.clone(),
            label: label.clone(),
            source: source.to_owned(),
        });
        Ok(CommandResult {
            committed: true,
            dry_run: false,
            version: self.version,
            label,
            changes: outcome.changes,
            result: outcome.result,
        })
    }

    /// The snapshot `command` would produce, without committing it.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`], or the command's error.
    pub fn simulate(&self, command: &dyn Command) -> Result<Project> {
        let project = self.require_project()?;
        Ok(run(command, project, &self.context())?.project)
    }

    /// Undoes the last step.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`], or [`ErrorCode::NothingToUndo`].
    pub fn undo(&mut self, source: &str) -> Result<CommandResult> {
        self.require_project()?;
        let entry = self
            .history
            .pop_undo()
            .ok_or_else(|| DomainError::new(ErrorCode::NothingToUndo, "nothing to undo"))?;
        let (project, changes, label) = (
            entry.before.clone(),
            entry.changes.inverted(),
            entry.label.clone(),
        );
        Ok(self.moved(EventKind::Undo, project, changes, label, source))
    }

    /// Redoes the last undone step.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoProject`], or [`ErrorCode::NothingToRedo`].
    pub fn redo(&mut self, source: &str) -> Result<CommandResult> {
        self.require_project()?;
        let entry = self
            .history
            .pop_redo()
            .ok_or_else(|| DomainError::new(ErrorCode::NothingToRedo, "nothing to redo"))?;
        let (project, changes, label) = (
            entry.after.clone(),
            entry.changes.clone(),
            entry.label.clone(),
        );
        Ok(self.moved(EventKind::Redo, project, changes, label, source))
    }

    fn moved(
        &mut self,
        kind: EventKind,
        project: Project,
        changes: ChangeSet,
        label: String,
        source: &str,
    ) -> CommandResult {
        self.project = Some(project);
        self.version += 1;
        let verb = if kind == EventKind::Undo {
            "Undo"
        } else {
            "Redo"
        };
        let result = CommandResult {
            committed: true,
            dry_run: false,
            version: self.version,
            label: format!("{verb} {label}"),
            changes: changes.clone(),
            result: JsonMap::new(),
        };
        self.emit(&SessionEvent {
            kind,
            version: self.version,
            changes,
            label,
            source: source.to_owned(),
        });
        result
    }

    /// Stops the last step from merging with the next one.
    pub fn seal_undo(&mut self) {
        self.history.seal();
    }

    /// Opens an undo group: until [`Self::end_undo_group`], the steps of the `agent` source stay
    /// separate (an `undo` still undoes one) and are collapsed into one step at the end. Used by a
    /// whole run of the AI composer. A project that is opened or closed meanwhile drops the group.
    pub fn begin_undo_group(&mut self) {
        self.history.begin_group();
    }

    /// Closes the undo group and collapses its steps into one named `label`. When steps were
    /// collapsed, observers get an event without changes, so that the Undo item and the panels
    /// word the new history. Returns whether there was anything to collapse.
    pub fn end_undo_group(&mut self, label: &str) -> bool {
        let collapsed = self.history.end_group(label);
        if collapsed {
            self.emit(&SessionEvent {
                kind: EventKind::Command,
                version: self.version,
                changes: ChangeSet::default(),
                label: String::new(),
                source: "user".to_owned(),
            });
        }
        collapsed
    }

    /// Forgets every undo and redo step (the prototype's `session.history.clear()`, used after
    /// `create_project` saves the new project at once).
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    // -- lifecycle (used by the project service) -----------------------------------------

    /// Makes `project` the open project; `saved` marks it clean.
    pub fn load(&mut self, project: Project, location: ProjectLocation, saved: bool) {
        self.saved_project = saved.then(|| project.clone());
        self.project = Some(project);
        self.location = Some(location);
        self.history.clear();
        self.version += 1;
        self.emit(&SessionEvent {
            kind: EventKind::Loaded,
            version: self.version,
            changes: ChangeSet::flags([Flag::Project]),
            label: String::new(),
            source: "user".to_owned(),
        });
    }

    /// Records that `project` was saved at `location`.
    pub fn mark_saved(&mut self, project: Project, location: ProjectLocation) {
        self.saved_project = Some(project);
        self.location = Some(location);
        self.emit(&SessionEvent {
            kind: EventKind::Saved,
            version: self.version,
            changes: ChangeSet::default(),
            label: String::new(),
            source: "user".to_owned(),
        });
    }

    /// Changes where the open project lives without other changes.
    pub fn replace_location(&mut self, location: ProjectLocation) {
        self.location = Some(location);
    }

    /// Closes the project.
    pub fn close(&mut self) {
        self.project = None;
        self.location = None;
        self.saved_project = None;
        self.history.clear();
        self.version += 1;
        self.emit(&SessionEvent {
            kind: EventKind::Closed,
            version: self.version,
            changes: ChangeSet::flags([Flag::Project]),
            label: String::new(),
            source: "user".to_owned(),
        });
    }

    // -- observers ------------------------------------------------------------------------

    /// A channel receiving every later event. Dropping the receiver unsubscribes.
    pub fn subscribe(&mut self) -> Receiver<SessionEvent> {
        let (sender, receiver) = channel();
        self.listeners.push(sender);
        receiver
    }

    fn emit(&mut self, event: &SessionEvent) {
        self.listeners
            .retain(|listener| listener.send(event.clone()).is_ok());
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::{EventKind, ProjectLocation, ProjectSession};
    use crate::commands::project::{SetEditorState, SetTempo};
    use crate::error::ErrorCode;
    use crate::ids::SequentialIds;
    use crate::model::project::Project;
    use crate::plugins::PluginRegistry;

    fn session() -> ProjectSession {
        let mut session =
            ProjectSession::with_options(Arc::new(PluginRegistry::new()), 500, Box::new(|| 0.0));
        let project = Project::create(&mut SequentialIds::new(0), "Song", 48_000, 120.0)
            .expect("valid project");
        let location = ProjectLocation {
            path: None,
            workspace: PathBuf::from("/tmp/ws"),
            packaged: false,
        };
        session.load(project, location, true);
        session
    }

    fn tempo(bpm: f64) -> SetTempo {
        SetTempo {
            bpm,
            beat: 0.0,
            ramp: false,
            clear_changes: false,
        }
    }

    #[test]
    fn commands_undo_and_redo() {
        let mut session = session();
        let events = session.subscribe();
        assert!(!session.is_dirty());
        let result = session.execute(&tempo(90.0), false, "user").expect("valid");
        assert!(result.committed);
        assert!(session.is_dirty());
        // The same merge key within the window merges into one undo step.
        session.execute(&tempo(95.0), false, "user").expect("valid");
        assert_eq!(session.history().labels().0, ["Set Tempo"]);
        let undo = session.undo("user").expect("one step");
        assert_eq!(undo.label, "Undo Set Tempo");
        assert!(!session.is_dirty());
        let err = session.undo("user").expect_err("nothing left");
        assert_eq!(err.code, ErrorCode::NothingToUndo);
        session.redo("user").expect("redo");
        let bpm = session.project().map(|p| p.tempo_map.bpm_at(0.0));
        assert_eq!(bpm, Some(95.0));
        let kinds: Vec<EventKind> = events.try_iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            [
                EventKind::Command,
                EventKind::Command,
                EventKind::Undo,
                EventKind::Redo
            ]
        );
    }

    #[test]
    fn dry_runs_and_view_state_do_not_dirty_the_project() {
        let mut session = session();
        let dry = session.execute(&tempo(90.0), true, "user").expect("valid");
        assert!(!dry.committed && dry.dry_run);
        let mut values = crate::model::JsonMap::new();
        values.insert("zoom".into(), 2.0.into());
        session
            .execute(&SetEditorState { values }, false, "user")
            .expect("valid");
        assert!(!session.is_dirty());
        assert!(!session.history().can_undo());
    }
}
