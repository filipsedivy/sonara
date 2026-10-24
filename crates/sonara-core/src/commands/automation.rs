//! Automation lane commands (`sonara.application.commands.automation_cmds`).

use serde_json::json;

use super::{ChangeSet, Command, CommandContext, Flag, ObjectKind, Outcome};
use crate::error::{DomainError, ErrorCode, Result};
use crate::model::automation::{
    AutomationLane, AutomationLaneData, AutomationPoint, AutomationTarget,
};
use crate::model::project::Project;
use crate::model::track::{MAX_SEND_DB, MAX_VOLUME_DB, MIN_SEND_DB, MIN_VOLUME_DB, Track};
use crate::plugins::ParamKind;
use crate::pyfmt::{float_g, str_repr};

/// Most points in one lane.
pub const MAX_POINTS_PER_LANE: usize = 20_000;

/// The track holding the lane, and the lane.
///
/// # Errors
///
/// [`ErrorCode::NotFound`]: `automation lane '…' not found`.
pub fn find_lane<'a>(
    project: &'a Project,
    lane_id: &str,
) -> Result<(&'a Track, &'a AutomationLane)> {
    project
        .all_tracks()
        .find_map(|t| {
            t.automation
                .iter()
                .find(|l| l.id == lane_id)
                .map(|l| (t, l))
        })
        .ok_or_else(|| DomainError::not_found("automation lane", lane_id))
}

/// The allowed value range of a target in its native units.
fn value_range(
    ctx: CommandContext<'_>,
    track: &Track,
    target: &AutomationTarget,
) -> Result<(f64, f64)> {
    let Some(device_id) = &target.device_id else {
        return match target.param.as_str() {
            "volume_db" => Ok((MIN_VOLUME_DB, MAX_VOLUME_DB)),
            "pan" => Ok((-1.0, 1.0)),
            _ => {
                // The target was validated: a mixer parameter that is not volume or pan is a
                // send level.
                if let Some(send_id) = target.send_id() {
                    track.send(send_id)?;
                }
                Ok((MIN_SEND_DB, MAX_SEND_DB))
            }
        };
    };
    let device = track.find_device(device_id).ok_or_else(|| {
        DomainError::new(
            ErrorCode::NotFound,
            format!(
                "device {} is not on track {}",
                str_repr(device_id),
                str_repr(&track.name)
            ),
        )
    })?;
    let spec = ctx.plugins.get(&device.plugin_id)?.param(&target.param)?;
    if !spec.automatable {
        return Err(DomainError::new(
            ErrorCode::Unsupported,
            format!("parameter {} is not automatable", str_repr(&spec.id)),
        ));
    }
    Ok(match spec.kind {
        ParamKind::Bool => (0.0, 1.0),
        #[expect(clippy::cast_precision_loss, reason = "choice counts are tiny")]
        ParamKind::Enum => (0.0, spec.choices.len().saturating_sub(1) as f64),
        ParamKind::Float | ParamKind::Int => (
            spec.minimum.unwrap_or(f64::NEG_INFINITY),
            spec.maximum.unwrap_or(f64::INFINITY),
        ),
    })
}

fn check_points(points: &[AutomationPoint], low: f64, high: f64) -> Result<()> {
    if points.len() > MAX_POINTS_PER_LANE {
        return Err(DomainError::new(
            ErrorCode::LimitExceeded,
            format!("a lane can hold at most {MAX_POINTS_PER_LANE} points"),
        ));
    }
    for pt in points {
        if !(low - 1e-9 <= pt.value && pt.value <= high + 1e-9) {
            return Err(DomainError::invalid(format!(
                "automation value {} at beat {} is outside {}..{}",
                float_g(pt.value),
                float_g(pt.beat),
                float_g(low),
                float_g(high)
            )));
        }
    }
    Ok(())
}

/// Creates the lane for `(device_id, param)` on a track or replaces its points; with
/// `start`/`end`, only the points inside `[start, end]` are replaced.
#[derive(Debug, Clone)]
pub struct SetAutomation {
    /// The track.
    pub track_id: String,
    /// The device, or `None` for a mixer parameter.
    pub device_id: Option<String>,
    /// The parameter.
    pub param: String,
    /// The new points.
    pub points: Vec<AutomationPoint>,
    /// Start of the replaced range.
    pub start: Option<f64>,
    /// End of the replaced range.
    pub end: Option<f64>,
    /// The ID of a new lane.
    pub lane_id: String,
}

impl Command for SetAutomation {
    fn name(&self) -> &'static str {
        "SetAutomation"
    }

    fn label(&self) -> &'static str {
        "Edit Automation"
    }

    fn apply(&self, project: &Project, ctx: &CommandContext<'_>) -> Result<Outcome> {
        let track = project.track(&self.track_id)?;
        let target = AutomationTarget::new(self.device_id.clone(), self.param.clone())?;
        let (low, high) = value_range(*ctx, track, &target)?;
        let existing = track.lane_for(self.device_id.as_deref(), &self.param);
        let points: Vec<AutomationPoint> = match existing {
            Some(lane) if self.start.is_some() || self.end.is_some() => {
                let lo = self.start.unwrap_or(f64::NEG_INFINITY);
                let hi = self.end.unwrap_or(f64::INFINITY);
                lane.points
                    .iter()
                    .filter(|p| !(lo <= p.beat && p.beat <= hi))
                    .chain(&self.points)
                    .copied()
                    .collect()
            }
            _ => self.points.clone(),
        };
        check_points(&points, low, high)?;
        let (lane, changes) = match existing {
            None => {
                let lane = AutomationLane::new(AutomationLaneData {
                    id: self.lane_id.clone(),
                    target,
                    points,
                    enabled: true,
                })?;
                let changes = ChangeSet::of(
                    [(ObjectKind::AutomationLane, lane.id.as_str())],
                    [(ObjectKind::Track, track.id.as_str())],
                    [],
                    [Flag::Automation],
                );
                (lane, changes)
            }
            Some(existing) => {
                let lane = existing.edit(|d| d.points = points)?;
                let changes = ChangeSet::of(
                    [],
                    [(ObjectKind::AutomationLane, lane.id.as_str())],
                    [],
                    [Flag::Automation],
                );
                (lane, changes)
            }
        };
        let result = json!({
            "lane_id": lane.id,
            "point_count": lane.points.len(),
            "value_range": [low, high],
        });
        let p = project.with_track(track.with_lane(lane)?)?;
        Ok(Outcome::with_result(p, changes, result))
    }
}

/// Enables or disables a lane.
#[derive(Debug, Clone)]
pub struct SetLaneEnabled {
    /// The lane.
    pub lane_id: String,
    /// Whether playback follows it.
    pub enabled: bool,
}

impl Command for SetLaneEnabled {
    fn name(&self) -> &'static str {
        "SetLaneEnabled"
    }

    fn label(&self) -> &'static str {
        "Toggle Automation"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, lane) = find_lane(project, &self.lane_id)?;
        let p = project.with_track(track.with_lane(lane.edit(|d| d.enabled = self.enabled)?)?)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [(ObjectKind::AutomationLane, lane.id.as_str())],
                [],
                [Flag::Automation],
            ),
        ))
    }
}

/// Deletes a lane.
#[derive(Debug, Clone)]
pub struct DeleteAutomationLane {
    /// The lane.
    pub lane_id: String,
}

impl Command for DeleteAutomationLane {
    fn name(&self) -> &'static str {
        "DeleteAutomationLane"
    }

    fn label(&self) -> &'static str {
        "Delete Automation"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, lane) = find_lane(project, &self.lane_id)?;
        let p = project.with_track(track.edit(|d| d.automation.retain(|a| a.id != lane.id))?)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [],
                [(ObjectKind::AutomationLane, lane.id.as_str())],
                [Flag::Automation],
            ),
        ))
    }
}
