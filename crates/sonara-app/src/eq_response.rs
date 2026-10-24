//! The parametric EQ's response curve for its device editor (the prototype's `EqCurve` and
//! `eq_sections` in `ui/devices/panel.py`; DV and DES-01).
//!
//! The prototype's GUI rebuilt the processor's biquad cascade in its own code. Here the curve
//! comes from `sonara-dsp`'s own design ([`parametric_eq_design`], the function the processor
//! runs) and `sosfreqz`'s magnitude ([`response_db`]), over the prototype's 240 log-spaced points
//! from 20 Hz to 20 kHz, plus the output gain. The page only draws it.

use serde::Serialize;
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::model::device::{ParamValue, Params};
use sonara_core::model::project::Project;
use sonara_core::plugins::PluginRegistry;

use crate::session::SharedSession;
use sonara_dsp::filters::response_db;
use sonara_dsp::processors::{param_slots, parametric_eq_design};

/// The parametric EQ's plugin ID.
pub const PLUGIN_ID: &str = "sonara.eq.parametric";
/// The curve's points (`EqCurve.POINTS`).
pub const POINTS: usize = 240;
/// The lowest frequency drawn (`F_MIN`).
pub const F_MIN: f64 = 20.0;
/// The highest frequency drawn (`F_MAX`).
pub const F_MAX: f64 = 20_000.0;
/// The bands with markers, in order.
const BANDS: [&str; 4] = ["low", "mid1", "mid2", "high"];

/// The curve of one EQ device.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EqResponse {
    /// The frequencies in Hz: [`POINTS`] log-spaced from [`F_MIN`] to [`F_MAX`].
    pub freqs: Vec<f64>,
    /// The response at each frequency in dB, the output gain included.
    pub db: Vec<f64>,
    /// The band markers: low, mid 1, mid 2, high.
    pub bands: Vec<EqBand>,
}

/// A band's marker.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EqBand {
    /// The band's frequency.
    pub freq: f64,
    /// The band's gain in dB.
    pub gain_db: f64,
    /// Whether the band is on (an off band has no marker).
    pub on: bool,
}

/// NumPy's `geomspace(start, stop, num)`: `10^(log10(start) + i · step)`, with the ends exact.
#[must_use]
pub fn geomspace(start: f64, stop: f64, num: usize) -> Vec<f64> {
    let (low, high) = (start.log10(), stop.log10());
    let last = num.saturating_sub(1);
    #[expect(
        clippy::cast_precision_loss,
        reason = "point counts are tiny; NumPy converts the same way"
    )]
    let increment = (high - low) / last.max(1) as f64;
    (0..num)
        .map(|i| match i {
            0 => start,
            i if i == last => stop,
            #[expect(clippy::cast_precision_loss, reason = "as above")]
            i => 10.0_f64.powf(i as f64 * increment + low),
        })
        .collect()
}

/// The curve of the EQ with the parameter values `params` (resolved or not: the descriptor's
/// defaults fill the gaps) at `sample_rate` (`EqCurve.set_params`).
///
/// # Errors
///
/// `INVALID_ARGUMENT` when the parameters do not convert or decode (a value of the wrong type,
/// an unknown choice).
pub fn response(
    plugins: &PluginRegistry,
    params: &Params,
    sample_rate: u32,
) -> Result<EqResponse, DomainError> {
    let descriptor = plugins.get(PLUGIN_ID)?;
    let slots =
        param_slots(descriptor, params).map_err(|error| DomainError::invalid(error.to_string()))?;
    let rate = f64::from(sample_rate);
    let design = parametric_eq_design(&slots, rate)
        .ok_or_else(|| DomainError::invalid("the EQ's parameters do not decode"))?;
    let freqs = geomspace(F_MIN, F_MAX, POINTS);
    let db = freqs
        .iter()
        .map(|&freq| response_db(design.sections(), freq, rate) + design.output_db)
        .collect();
    let resolved = descriptor.resolve(params);
    let number = |key: &str| {
        resolved
            .get(key)
            .and_then(ParamValue::as_f64)
            .unwrap_or(0.0)
    };
    let bands = BANDS
        .iter()
        .map(|band| EqBand {
            freq: number(&format!("{band}_freq")),
            gain_db: number(&format!("{band}_gain")),
            on: matches!(
                resolved.get(&format!("{band}_on")),
                Some(ParamValue::Bool(true))
            ),
        })
        .collect();
    Ok(EqResponse { freqs, db, bands })
}

/// The curve of the EQ device `device_id` of `project`, at the project rate.
///
/// # Errors
///
/// `NOT_FOUND` for an unknown device; `INVALID_ARGUMENT` for a device that is not a parametric
/// EQ, and the errors of [`response`].
pub fn device_response(
    project: &Project,
    plugins: &PluginRegistry,
    device_id: &str,
) -> Result<EqResponse, DomainError> {
    let (_, device) = project.device(device_id)?;
    if device.plugin_id != PLUGIN_ID {
        return Err(DomainError::new(
            ErrorCode::InvalidArgument,
            format!("device {device_id} is not a parametric EQ"),
        ));
    }
    response(plugins, &device.params, project.settings.sample_rate)
}

/// The curve of the EQ device `device_id` of `session`'s open project.
///
/// # Errors
///
/// `NO_PROJECT` without a project, and the errors of [`device_response`].
pub fn curve(
    session: &SharedSession,
    plugins: &PluginRegistry,
    device_id: &str,
) -> Result<EqResponse, DomainError> {
    let project = session.lock().require_project()?.clone();
    device_response(&project, plugins, device_id)
}
