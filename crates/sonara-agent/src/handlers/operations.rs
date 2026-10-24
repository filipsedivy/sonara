//! Operation tools: the status, cancellation and list of background operations (render, export,
//! decode jobs).

use std::time::Duration;

use serde_json::{Value, json};

use crate::context::AgentContext;
use crate::contract::args::{Empty, OperationId};
use crate::registry::ToolResult;

/// The operations `list_operations` reports (the newest).
const LISTED_OPERATIONS: usize = 30;

/// `get_operation_status`: the status, progress, result or error of an operation, after
/// waiting up to `wait_seconds` for it to finish.
pub(super) fn get_operation_status(ctx: &AgentContext, args: OperationId) -> ToolResult {
    let OperationId {
        operation_id,
        wait_seconds,
    } = args;
    let job = ctx.app().jobs().get(&operation_id)?;
    // `if a.wait_seconds:` – zero does not wait. A wait too long for a `Duration` has no bound
    // (`Event.wait(timeout)`).
    if wait_seconds != 0.0 {
        let timeout = Duration::try_from_secs_f64(wait_seconds).ok();
        // The status reported below says whether the operation finished in time.
        let _finished = job.wait(timeout);
    }
    Ok(job.to_json())
}

/// `cancel_operation`: requests cancellation of a running operation; a finished one is reported
/// unchanged.
pub(super) fn cancel_operation(ctx: &AgentContext, args: OperationId) -> ToolResult {
    let OperationId { operation_id, .. } = args;
    Ok(ctx.app().jobs().cancel(&operation_id)?.to_json())
}

/// `list_operations`: the recent operations, oldest first, without their results.
#[expect(
    clippy::unnecessary_wraps,
    reason = "every handler returns a ToolResult (ToolRegistry::register)"
)]
pub(super) fn list_operations(ctx: &AgentContext, _args: Empty) -> ToolResult {
    let operations: Vec<Value> = ctx
        .app()
        .jobs()
        .list(LISTED_OPERATIONS)
        .iter()
        .map(|job| {
            // `j.to_json() | {"result": None}`: a result is blanked in place, else appended.
            let mut operation = job.to_json();
            operation["result"] = Value::Null;
            operation
        })
        .collect();
    Ok(json!({"operations": operations}))
}
