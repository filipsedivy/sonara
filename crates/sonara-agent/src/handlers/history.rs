//! History tools: undo, redo and the edit history, shared with the GUI.

use serde_json::json;

use crate::context::AgentContext;
use crate::contract::args::Empty;
use crate::registry::ToolResult;

/// The undo labels `get_history` reports (the newest).
const UNDO_LABELS: usize = 50;
/// The redo labels `get_history` reports (the next).
const REDO_LABELS: usize = 50;
/// The audit entries `get_history` reports (the newest).
const RECENT_ACTIONS: usize = 20;

/// `undo`: undoes the last edit.
pub(super) fn undo(ctx: &AgentContext, _args: Empty) -> ToolResult {
    let result = ctx.app().session().lock().undo(ctx.source())?;
    Ok(result.to_json())
}

/// `redo`: redoes the last undone edit.
pub(super) fn redo(ctx: &AgentContext, _args: Empty) -> ToolResult {
    let result = ctx.app().session().lock().redo(ctx.source())?;
    Ok(result.to_json())
}

/// `get_history`: the undo and redo labels (newest undo last, next redo first) and the recent
/// agent actions of the audit log.
pub(super) fn get_history(ctx: &AgentContext, _args: Empty) -> ToolResult {
    let (undo, redo) = {
        let session = ctx.app().session().lock();
        let (undo, redo) = session.history().labels();
        let skip = undo.len().saturating_sub(UNDO_LABELS);
        (
            undo.into_iter()
                .skip(skip)
                .map(str::to_owned)
                .collect::<Vec<_>>(),
            redo.into_iter()
                .take(REDO_LABELS)
                .map(str::to_owned)
                .collect::<Vec<_>>(),
        )
    };
    Ok(json!({
        "undo": undo,
        "redo": redo,
        "recent_agent_actions": ctx.audit().tail(RECENT_ACTIONS)?,
    }))
}
