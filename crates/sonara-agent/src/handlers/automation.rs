//! Automation (`create_automation` of the prototype's "devices" section): lanes for volume,
//! pan, send levels and automatable device parameters.

use sonara_core::commands::automation::SetAutomation;
use sonara_core::error::DomainError;
use sonara_core::ids::{self, IdGenerator};
use sonara_core::model::automation::{AutomationPoint, CurveKind, MIXER_PARAMS};
use sonara_core::pyfmt::str_repr;

use super::positions::Positions;
use crate::context::AgentContext;
use crate::contract::args::{CreateAutomation, CurveShape};
use crate::pystr::py_strip;
use crate::registry::ToolResult;

/// The prototype's `_automation_target`: the device (`None` for the mixer) and the parameter a
/// target names: `volume`, `pan`, `send:<send_id>` or `<device_id>:<param>`.
///
/// # Errors
///
/// [`sonara_core::error::ErrorCode::InvalidArgument`]: `unknown automation target '<target>'`
/// with a `hint`.
fn automation_target(target: &str) -> Result<(Option<String>, String), DomainError> {
    let stripped = py_strip(target);
    if matches!(stripped, "volume" | "volume_db") {
        return Ok((None, "volume_db".to_owned()));
    }
    if stripped == "pan" {
        return Ok((None, "pan".to_owned()));
    }
    if let Some(send_id) = stripped.strip_prefix("send:") {
        return Ok((None, format!("send.{send_id}.level_db")));
    }
    if let Some((device_id, param)) = stripped.split_once(':') {
        return Ok((Some(device_id.to_owned()), param.to_owned()));
    }
    if MIXER_PARAMS.contains(&stripped) {
        return Ok((None, stripped.to_owned()));
    }
    Err(
        DomainError::invalid(format!("unknown automation target {}", str_repr(target)))
            .with_detail(
                "hint",
                "use 'volume', 'pan', 'send:<send_id>' or '<device_id>:<param>'",
            ),
    )
}

fn curve_kind(curve: CurveShape) -> CurveKind {
    match curve {
        CurveShape::Linear => CurveKind::Linear,
        CurveShape::Hold => CurveKind::Hold,
        CurveShape::Smooth => CurveKind::Smooth,
    }
}

/// `create_automation`: writes the points of a lane, creating it when the track has none for
/// the target. With `replace_range`, only the existing points between the first and the last
/// new point are replaced.
///
/// A new lane ID is drawn on every call, also when the lane exists, as the prototype's command
/// draws it.
pub(super) fn create_automation(ctx: &AgentContext, args: CreateAutomation) -> ToolResult {
    let CreateAutomation {
        dry_run,
        track_id,
        target,
        points,
        replace_range,
    } = args;
    let (device_id, param) = automation_target(&target)?;
    let positions = Positions::new(ctx);
    let points = points
        .iter()
        .map(|point| {
            AutomationPoint::new(
                positions.beats(&point.at)?,
                point.value,
                curve_kind(point.curve),
            )
        })
        .collect::<Result<Vec<_>, DomainError>>()?;
    let (start, end) = if replace_range {
        let beats = points.iter().map(|point| point.beat);
        (
            Some(beats.clone().fold(f64::INFINITY, f64::min)),
            Some(beats.fold(f64::NEG_INFINITY, f64::max)),
        )
    } else {
        (None, None)
    };
    let command = SetAutomation {
        track_id,
        device_id,
        param,
        points,
        start,
        end,
        lane_id: ctx.app().ids().new_id(ids::LANE),
    };
    Ok(ctx.commit(&command, dry_run)?)
}
