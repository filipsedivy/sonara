//! Commands: immutable descriptions of project edits (`sonara.application.commands`, ADR 0002).
//!
//! [`Command::apply`] is a pure function from one project snapshot to the next. It validates
//! everything and returns a [`DomainError`](crate::error::DomainError) (leaving the session untouched) when the edit is not
//! allowed. Commands receive their new object IDs when they are built, so applying one is
//! deterministic and replayable.

pub mod automation;
pub mod clip;
pub mod device;
pub mod project;
pub mod track;

use std::collections::BTreeSet;
use std::fmt;

use indexmap::IndexSet;
use serde_json::{Value, json};

use crate::error::Result;
use crate::model::JsonMap;
use crate::model::project::Project;
use crate::plugins::PluginRegistry;
use crate::routing;

/// The kind of object a change refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ObjectKind {
    /// The project itself.
    Project,
    /// A track (or the master).
    Track,
    /// A clip.
    Clip,
    /// A MIDI note.
    Note,
    /// An instrument or effect.
    Device,
    /// A send.
    Send,
    /// An automation lane.
    AutomationLane,
    /// A marker.
    Marker,
    /// A section.
    Section,
    /// A media asset.
    Asset,
}

impl ObjectKind {
    /// The kind as reported to observers and agents.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Track => "track",
            Self::Clip => "clip",
            Self::Note => "note",
            Self::Device => "device",
            Self::Send => "send",
            Self::AutomationLane => "automation_lane",
            Self::Marker => "marker",
            Self::Section => "section",
            Self::Asset => "asset",
        }
    }
}

/// A reference to a changed object.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectRef {
    /// What kind of object.
    pub kind: ObjectKind,
    /// Its ID (`project` for the project).
    pub id: String,
}

impl ObjectRef {
    /// A reference to `id` of `kind`.
    #[must_use]
    pub fn new(kind: ObjectKind, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
        }
    }

    /// The reference to the project itself.
    #[must_use]
    pub fn project() -> Self {
        Self::new(ObjectKind::Project, "project")
    }

    /// `{"kind": …, "id": …}`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({"kind": self.kind.as_str(), "id": self.id})
    }
}

/// Coarse change categories that observers use to decide what to refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Flag {
    /// Track list, order or track properties.
    Tracks,
    /// Clips.
    Clips,
    /// Notes and controller events.
    Notes,
    /// Mixer settings.
    Mixer,
    /// Instruments and effects.
    Devices,
    /// Outputs, sends and sidechains.
    Routing,
    /// Automation lanes.
    Automation,
    /// Tempo and time signatures.
    Tempo,
    /// Media assets.
    Media,
    /// Markers and sections.
    Markers,
    /// Transport settings.
    Transport,
    /// Project settings.
    Settings,
    /// Project metadata.
    Metadata,
    /// Editor state.
    Editor,
    /// The whole project was replaced.
    Project,
}

impl Flag {
    /// The flag as reported to observers and agents.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tracks => "tracks",
            Self::Clips => "clips",
            Self::Notes => "notes",
            Self::Mixer => "mixer",
            Self::Devices => "devices",
            Self::Routing => "routing",
            Self::Automation => "automation",
            Self::Tempo => "tempo",
            Self::Media => "media",
            Self::Markers => "markers",
            Self::Transport => "transport",
            Self::Settings => "settings",
            Self::Metadata => "metadata",
            Self::Editor => "editor",
            Self::Project => "project",
        }
    }
}

/// What a command created, updated and deleted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangeSet {
    /// New objects, in order.
    pub created: Vec<ObjectRef>,
    /// Changed objects, in order.
    pub updated: Vec<ObjectRef>,
    /// Removed objects, in order.
    pub deleted: Vec<ObjectRef>,
    /// Change categories.
    pub flags: BTreeSet<Flag>,
}

/// `(kind, id)` pairs as object references.
fn refs<'a>(items: impl IntoIterator<Item = (ObjectKind, &'a str)>) -> Vec<ObjectRef> {
    items
        .into_iter()
        .map(|(kind, id)| ObjectRef::new(kind, id))
        .collect()
}

impl ChangeSet {
    /// A change set from `(kind, id)` pairs (the prototype's `ChangeSet.of`).
    #[must_use]
    pub fn of<'a>(
        created: impl IntoIterator<Item = (ObjectKind, &'a str)>,
        updated: impl IntoIterator<Item = (ObjectKind, &'a str)>,
        deleted: impl IntoIterator<Item = (ObjectKind, &'a str)>,
        flags: impl IntoIterator<Item = Flag>,
    ) -> Self {
        Self {
            created: refs(created),
            updated: refs(updated),
            deleted: refs(deleted),
            flags: flags.into_iter().collect(),
        }
    }

    /// Only flags.
    #[must_use]
    pub fn flags(flags: impl IntoIterator<Item = Flag>) -> Self {
        Self {
            flags: flags.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Both changes as one. Objects created and then deleted vanish; updates of created or
    /// deleted objects are dropped; order is kept and duplicates are removed.
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        let created: IndexSet<&ObjectRef> = self.created.iter().chain(&other.created).collect();
        let deleted: IndexSet<&ObjectRef> = self.deleted.iter().chain(&other.deleted).collect();
        let updated: IndexSet<&ObjectRef> = self
            .updated
            .iter()
            .chain(&other.updated)
            .filter(|r| !created.contains(r) && !deleted.contains(r))
            .collect();
        Self {
            created: created
                .iter()
                .filter(|r| !deleted.contains(*r))
                .map(|r| (*r).clone())
                .collect(),
            updated: updated.into_iter().cloned().collect(),
            deleted: deleted
                .iter()
                .filter(|r| !created.contains(*r))
                .map(|r| (*r).clone())
                .collect(),
            flags: self.flags.union(&other.flags).copied().collect(),
        }
    }

    /// The change set of undoing this change.
    #[must_use]
    pub fn inverted(&self) -> Self {
        Self {
            created: self.deleted.clone(),
            updated: self.updated.clone(),
            deleted: self.created.clone(),
            flags: self.flags.clone(),
        }
    }

    /// Whether nothing changed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.created.is_empty()
            && self.updated.is_empty()
            && self.deleted.is_empty()
            && self.flags.is_empty()
    }

    /// IDs of all referenced objects, optionally only of one kind.
    #[must_use]
    pub fn ids(&self, kind: Option<ObjectKind>) -> Vec<&str> {
        self.created
            .iter()
            .chain(&self.updated)
            .chain(&self.deleted)
            .filter(|r| kind.is_none_or(|k| r.kind == k))
            .map(|r| r.id.as_str())
            .collect()
    }

    /// The agent-facing form, flags sorted by name.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut flags: Vec<&str> = self.flags.iter().map(|f| f.as_str()).collect();
        flags.sort_unstable();
        json!({
            "created": self.created.iter().map(ObjectRef::to_json).collect::<Vec<_>>(),
            "updated": self.updated.iter().map(ObjectRef::to_json).collect::<Vec<_>>(),
            "deleted": self.deleted.iter().map(ObjectRef::to_json).collect::<Vec<_>>(),
            "flags": flags,
        })
    }
}

/// What commands may consult besides the project.
#[derive(Debug, Clone, Copy)]
pub struct CommandContext<'a> {
    /// The available plugins.
    pub plugins: &'a PluginRegistry,
}

/// The result of applying a command.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// The new snapshot (the input snapshot itself when nothing changed).
    pub project: Project,
    /// What changed.
    pub changes: ChangeSet,
    /// Command-specific result data for the caller.
    pub result: JsonMap,
}

impl Outcome {
    /// An outcome without result data.
    #[must_use]
    pub fn new(project: Project, changes: ChangeSet) -> Self {
        Self {
            project,
            changes,
            result: JsonMap::new(),
        }
    }

    /// An outcome with result data; `result` must be a JSON object.
    #[must_use]
    pub fn with_result(project: Project, changes: ChangeSet, result: Value) -> Self {
        let result = match result {
            Value::Object(map) => map,
            _ => JsonMap::new(),
        };
        Self {
            project,
            changes,
            result,
        }
    }
}

/// A project edit.
pub trait Command: fmt::Debug + Send + Sync {
    /// The prototype's class name (golden data, audit log).
    fn name(&self) -> &'static str;

    /// Human-readable label for the undo menu and the audit log.
    fn label(&self) -> &'static str;

    /// Whether the command changes routing, so the routing graph is validated afterwards.
    fn affects_routing(&self) -> bool {
        false
    }

    /// View-state commands (zoom, snap) bypass the undo history and do not make the project
    /// dirty.
    fn undoable(&self) -> bool {
        true
    }

    /// The edit itself.
    ///
    /// # Errors
    ///
    /// The [`DomainError`](crate::error::DomainError) of the first rule the edit breaks.
    fn apply(&self, project: &Project, ctx: &CommandContext<'_>) -> Result<Outcome>;

    /// Consecutive commands with an equal key merge into one undo step.
    fn merge_key(&self) -> Option<String> {
        None
    }

    /// The label shown for this particular edit.
    fn describe(&self) -> String {
        self.label().to_owned()
    }
}

/// Applies `command` and validates the routing graph when routing may have changed.
///
/// # Errors
///
/// The command's error, or the first routing issue of the new snapshot.
pub fn run(command: &dyn Command, project: &Project, ctx: &CommandContext<'_>) -> Result<Outcome> {
    let outcome = command.apply(project, ctx)?;
    if command.affects_routing() || outcome.changes.flags.contains(&Flag::Routing) {
        routing::validate(&outcome.project)?;
    }
    Ok(outcome)
}

/// Several commands applied atomically as one undo step.
#[derive(Debug)]
pub struct Transaction {
    /// The steps, in order.
    pub commands: Vec<Box<dyn Command>>,
    /// The undo label.
    pub name: String,
}

impl Command for Transaction {
    fn name(&self) -> &'static str {
        "Transaction"
    }

    fn label(&self) -> &'static str {
        "Edit"
    }

    fn describe(&self) -> String {
        self.name.clone()
    }

    fn apply(&self, project: &Project, ctx: &CommandContext<'_>) -> Result<Outcome> {
        let mut project = project.clone();
        let mut changes = ChangeSet::default();
        let mut steps = Vec::with_capacity(self.commands.len());
        for command in &self.commands {
            let outcome = run(command.as_ref(), &project, ctx)?;
            project = outcome.project;
            changes = changes.merge(&outcome.changes);
            steps.push(Value::Object(outcome.result));
        }
        Ok(Outcome::with_result(
            project,
            changes,
            json!({"steps": steps}),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{ChangeSet, Flag, ObjectKind};

    #[test]
    fn merging_drops_objects_created_and_deleted_in_one_change() {
        let first = ChangeSet::of(
            [(ObjectKind::Clip, "clp_1")],
            [(ObjectKind::Track, "trk_1")],
            [],
            [Flag::Clips],
        );
        let second = ChangeSet::of(
            [],
            [(ObjectKind::Clip, "clp_1"), (ObjectKind::Track, "trk_1")],
            [(ObjectKind::Clip, "clp_1")],
            [Flag::Tracks],
        );
        let merged = first.merge(&second);
        assert_eq!(merged.created, []);
        assert_eq!(merged.deleted, []);
        assert_eq!(merged.ids(None), ["trk_1"]);
        assert_eq!(
            merged.to_json()["flags"],
            serde_json::json!(["clips", "tracks"])
        );
        assert_eq!(first.inverted().deleted, first.created);
    }
}
