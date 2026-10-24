//! Positions and lengths of tool arguments, resolved against the open project as the
//! prototype's `ctx.beats` and `ctx.length_beats` resolve them.

use std::cell::OnceCell;

use sonara_core::error::DomainError;
use sonara_core::model::project::Project;

use crate::context::AgentContext;
use crate::contract::args::{Length, Position};
use crate::conversion;

/// The open project, read on first use. The prototype reads `ctx.project` afresh for every
/// conversion; nothing changes the project while a handler converts its arguments, so one
/// snapshot gives the same values. Reading it lazily keeps the prototype's error order: a
/// numeric position converts without an open project, a length does not.
pub(super) struct Positions<'a> {
    ctx: &'a AgentContext,
    project: OnceCell<Project>,
}

impl<'a> Positions<'a> {
    /// Nothing is read yet.
    pub(super) fn new(ctx: &'a AgentContext) -> Self {
        Self {
            ctx,
            project: OnceCell::new(),
        }
    }

    /// The open project (`ctx.project`).
    ///
    /// # Errors
    ///
    /// [`sonara_core::error::ErrorCode::NoProject`].
    pub(super) fn project(&self) -> Result<&Project, DomainError> {
        if let Some(project) = self.project.get() {
            return Ok(project);
        }
        let project = self.ctx.project()?;
        Ok(self.project.get_or_init(|| project))
    }

    /// `ctx.beats(value)` ([`conversion::beats`]).
    ///
    /// # Errors
    ///
    /// The errors of [`conversion::beats`].
    pub(super) fn beats(&self, value: &Position) -> Result<f64, DomainError> {
        conversion::beats(value, || self.project())
    }

    /// `ctx.length_beats(value, start)` ([`conversion::length_beats`]); the prototype reads the
    /// project before it looks at the value.
    ///
    /// # Errors
    ///
    /// [`sonara_core::error::ErrorCode::NoProject`], or the errors of
    /// [`conversion::length_beats`].
    pub(super) fn length_beats(&self, value: &Length, start: f64) -> Result<f64, DomainError> {
        conversion::length_beats(value, start, self.project()?)
    }
}
