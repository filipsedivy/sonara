//! Snapshot-based undo and redo (`sonara.application.undo.history`, ADR 0002).

use crate::commands::ChangeSet;
use crate::model::project::Project;

/// Default number of undo steps kept.
pub const DEFAULT_LIMIT: usize = 500;
/// Default window in seconds within which commands with the same merge key merge.
pub const DEFAULT_MERGE_WINDOW: f64 = 1.5;

/// One undo step: the snapshots before and after it.
#[derive(Debug, Clone)]
pub struct UndoEntry {
    /// The label shown in the undo menu.
    pub label: String,
    /// The project before the step.
    pub before: Project,
    /// The project after the step.
    pub after: Project,
    /// What the step changed.
    pub changes: ChangeSet,
    /// Consecutive steps with an equal key merge.
    pub merge_key: Option<String>,
    /// When the step happened, in seconds of the session clock.
    pub timestamp: f64,
    /// Who made the step (`user`, `agent`, …).
    pub source: String,
}

/// The undo and redo stacks.
#[derive(Debug, Clone)]
pub struct UndoHistory {
    undo: Vec<UndoEntry>,
    redo: Vec<UndoEntry>,
    /// Most undo steps kept; older ones are dropped.
    pub limit: usize,
    /// Merge window in seconds.
    pub merge_window: f64,
}

impl Default for UndoHistory {
    fn default() -> Self {
        Self::new(DEFAULT_LIMIT, DEFAULT_MERGE_WINDOW)
    }
}

impl UndoHistory {
    /// An empty history.
    #[must_use]
    pub fn new(limit: usize, merge_window: f64) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit,
            merge_window,
        }
    }

    /// Records a step. It clears the redo stack and merges into the top entry when both have
    /// the same merge key and lie within the merge window.
    pub fn push(&mut self, entry: UndoEntry) {
        self.redo.clear();
        if let Some(top) = self.undo.last_mut()
            && entry.merge_key.is_some()
            && top.merge_key == entry.merge_key
            && entry.timestamp - top.timestamp <= self.merge_window
        {
            top.after = entry.after;
            top.changes = top.changes.merge(&entry.changes);
            top.timestamp = entry.timestamp;
            return;
        }
        self.undo.push(entry);
        if self.undo.len() > self.limit {
            let excess = self.undo.len() - self.limit;
            self.undo.drain(..excess);
        }
    }

    /// Moves the top undo entry to the redo stack and returns it.
    pub fn pop_undo(&mut self) -> Option<&UndoEntry> {
        let entry = self.undo.pop()?;
        self.redo.push(entry);
        self.redo.last()
    }

    /// Moves the top redo entry to the undo stack and returns it.
    pub fn pop_redo(&mut self) -> Option<&UndoEntry> {
        let entry = self.redo.pop()?;
        self.undo.push(entry);
        self.undo.last()
    }

    /// Stops the top entry from merging with the next step (for example on mouse release).
    pub fn seal(&mut self) {
        if let Some(top) = self.undo.last_mut() {
            top.merge_key = None;
        }
    }

    /// Forgets every step.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// Whether a step can be undone.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether a step can be redone.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The label of the next undo.
    #[must_use]
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|e| e.label.as_str())
    }

    /// The label of the next redo.
    #[must_use]
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|e| e.label.as_str())
    }

    /// Undo labels (oldest first) and redo labels (next first).
    #[must_use]
    pub fn labels(&self) -> (Vec<&str>, Vec<&str>) {
        (
            self.undo.iter().map(|e| e.label.as_str()).collect(),
            self.redo.iter().rev().map(|e| e.label.as_str()).collect(),
        )
    }

    /// Every snapshot still reachable through undo or redo (for safe media cleanup).
    #[must_use]
    pub fn reachable_projects(&self) -> Vec<&Project> {
        self.undo
            .iter()
            .chain(&self.redo)
            .flat_map(|e| [&e.before, &e.after])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{UndoEntry, UndoHistory};
    use crate::commands::ChangeSet;
    use crate::ids::SequentialIds;
    use crate::model::project::Project;

    fn entry(project: &Project, label: &str, key: Option<&str>, timestamp: f64) -> UndoEntry {
        UndoEntry {
            label: label.to_owned(),
            before: project.clone(),
            after: project.clone(),
            changes: ChangeSet::default(),
            merge_key: key.map(str::to_owned),
            timestamp,
            source: "user".to_owned(),
        }
    }

    #[test]
    fn steps_merge_within_the_window_and_respect_the_limit() {
        let project = Project::create(&mut SequentialIds::new(0), "Song", 48_000, 120.0)
            .expect("valid project");
        let mut history = UndoHistory::new(2, 1.5);
        history.push(entry(&project, "Volume", Some("mixer"), 0.0));
        history.push(entry(&project, "Volume", Some("mixer"), 1.0));
        assert_eq!(history.labels().0, ["Volume"]);
        history.push(entry(&project, "Volume", Some("mixer"), 3.0));
        history.push(entry(&project, "Pan", None, 3.0));
        assert_eq!(history.labels().0, ["Volume", "Pan"]);
        assert_eq!(
            history.pop_undo().map(|e| e.label.clone()),
            Some("Pan".into())
        );
        assert_eq!(history.redo_label(), Some("Pan"));
        history.seal();
        history.push(entry(&project, "Volume", Some("mixer"), 3.1));
        assert_eq!(history.labels().0, ["Volume", "Volume"]);
        assert!(!history.can_redo());
    }
}
