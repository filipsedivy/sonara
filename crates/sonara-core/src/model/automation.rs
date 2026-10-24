//! Parameter automation lanes (`sonara.domain.automation`).
//!
//! A lane targets a device parameter (`device_id` set) or a mixer parameter of the owning track
//! (`device_id` is `None`): `volume_db`, `pan` or `send.<send_id>.level_db`. Point values are in
//! the parameter's native units. The curve of a point shapes the segment that starts at it.

use std::fmt;

use super::{Valid, Validate, cmp_f64};
use crate::error::{Result, require, require_finite};
use crate::ids::{self, IdGenerator};
use crate::pyfmt::str_repr;

/// Mixer parameters a lane without a device can target, besides send levels.
pub const MIXER_PARAMS: [&str; 2] = ["volume_db", "pan"];

/// The shape of the segment that starts at a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CurveKind {
    /// Straight line to the next point.
    #[default]
    Linear,
    /// The value holds until the next point.
    Hold,
    /// Raised-cosine ease to the next point.
    Smooth,
}

impl CurveKind {
    /// The kind as stored in project files.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Hold => "hold",
            Self::Smooth => "smooth",
        }
    }

    /// Parses the stored form.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "linear" => Some(Self::Linear),
            "hold" => Some(Self::Hold),
            "smooth" => Some(Self::Smooth),
            _ => None,
        }
    }
}

/// An automation point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutomationPoint {
    /// Position in beats (≥ 0).
    pub beat: f64,
    /// Value in the parameter's native units.
    pub value: f64,
    /// Shape of the segment that starts here.
    pub curve: CurveKind,
}

impl AutomationPoint {
    /// A validated point.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::InvalidArgument`] for a non-finite or negative beat or a
    /// non-finite value.
    pub fn new(beat: f64, value: f64, curve: CurveKind) -> Result<Self> {
        require_finite(beat, "automation point beat")?;
        require_finite(value, "automation point value")?;
        require(beat >= 0.0, || {
            "automation point beat must be >= 0".to_owned()
        })?;
        Ok(Self { beat, value, curve })
    }
}

/// What a lane automates.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AutomationTarget {
    /// The device, or `None` for a mixer parameter of the owning track.
    pub device_id: Option<String>,
    /// Parameter name.
    pub param: String,
}

impl AutomationTarget {
    /// A validated target.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::InvalidArgument`] for an empty parameter, or an unknown mixer
    /// parameter when `device_id` is `None`.
    pub fn new(device_id: Option<String>, param: impl Into<String>) -> Result<Self> {
        let param = param.into();
        require(!param.is_empty(), || {
            "automation target parameter must not be empty".to_owned()
        })?;
        if device_id.is_none() {
            require(
                MIXER_PARAMS.contains(&param.as_str()) || send_param_id(&param).is_some(),
                || format!("unknown mixer automation parameter {}", str_repr(&param)),
            )?;
        }
        Ok(Self { device_id, param })
    }

    /// The send ID of a `send.<id>.level_db` target.
    #[must_use]
    pub fn send_id(&self) -> Option<&str> {
        send_param_id(&self.param)
    }
}

impl fmt::Display for AutomationTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let owner = self
            .device_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .unwrap_or("mixer");
        write!(f, "{owner}.{}", self.param)
    }
}

/// `Some(id)` for `send.<id>.level_db`.
fn send_param_id(param: &str) -> Option<&str> {
    let mut parts = param.split('.');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("send"), Some(id), Some("level_db"), None) => Some(id),
        _ => None,
    }
}

/// The data of an [`AutomationLane`].
#[derive(Debug, Clone, PartialEq)]
pub struct AutomationLaneData {
    /// Stable ID (`aut_…`).
    pub id: String,
    /// What the lane automates.
    pub target: AutomationTarget,
    /// Points, sorted by beat on validation (two points may share a beat: a vertical jump).
    pub points: Vec<AutomationPoint>,
    /// Whether playback follows the lane.
    pub enabled: bool,
}

impl Validate for AutomationLaneData {
    fn validate(&mut self) -> Result<()> {
        self.points.sort_by(|a, b| cmp_f64(a.beat, b.beat));
        Ok(())
    }
}

/// An automation lane.
pub type AutomationLane = Valid<AutomationLaneData>;

impl AutomationLane {
    /// A new enabled lane with a fresh ID.
    ///
    /// # Errors
    ///
    /// None in practice; lanes only normalize their point order.
    pub fn create(
        ids: &mut dyn IdGenerator,
        target: AutomationTarget,
        points: Vec<AutomationPoint>,
    ) -> Result<Self> {
        Self::new(AutomationLaneData {
            id: ids.new_id(ids::LANE),
            target,
            points,
            enabled: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{AutomationLaneData, AutomationPoint, AutomationTarget, CurveKind, Valid};

    #[test]
    fn mixer_targets_follow_the_prototype() {
        assert!(AutomationTarget::new(None, "volume_db").is_ok());
        let send = AutomationTarget::new(None, "send.snd_1.level_db").expect("send level");
        assert_eq!(send.send_id(), Some("snd_1"));
        assert_eq!(send.to_string(), "mixer.send.snd_1.level_db");
        let err = AutomationTarget::new(None, "cutoff").expect_err("unknown mixer param");
        assert_eq!(err.message, "unknown mixer automation parameter 'cutoff'");
        let device = AutomationTarget::new(Some("dev_1".into()), "cutoff").expect("device");
        assert_eq!(device.to_string(), "dev_1.cutoff");
        assert_eq!(device.send_id(), None);
    }

    #[test]
    fn lanes_sort_points_stably() {
        let point = |beat, value| AutomationPoint::new(beat, value, CurveKind::Linear);
        let lane = Valid::new(AutomationLaneData {
            id: "aut_1".into(),
            target: AutomationTarget::new(None, "pan").expect("pan"),
            points: vec![
                point(4.0, 1.0).expect("valid"),
                point(0.0, 0.0).expect("valid"),
                point(4.0, 2.0).expect("valid"),
            ],
            enabled: true,
        })
        .expect("valid lane");
        let values: Vec<f64> = lane.points.iter().map(|p| p.value).collect();
        assert_eq!(values, [0.0, 1.0, 2.0]);
    }
}
