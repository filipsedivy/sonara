//! Device instances: an instrument or effect placed on a track (`sonara.domain.effects`).
//!
//! A device stores its identity, its plugin reference and its parameter values. The parameter
//! schema (ranges, units, defaults) belongs to the plugin descriptor, so the model stays
//! independent of DSP code.

use indexmap::IndexMap;

use super::{JsonMap, Valid, Validate};
use crate::error::{Result, require};
use crate::ids::{self, IdGenerator};
use crate::pyfmt::str_repr;

/// A parameter value as stored in the project (a JSON number, boolean or string).
#[derive(Debug, Clone)]
pub enum ParamValue {
    /// A switch.
    Bool(bool),
    /// An integer (choice indices, counts).
    Int(i64),
    /// A float in the parameter's native units.
    Float(f64),
    /// A string (file references, modes).
    Str(String),
}

impl ParamValue {
    /// The numeric value, as Python's `float()` gives it (`true` is 1.0); `None` for a string.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
            #[expect(
                clippy::cast_precision_loss,
                reason = "Python's float(int) rounds the same way"
            )]
            Self::Int(value) => Some(*value as f64),
            Self::Float(value) => Some(*value),
            Self::Str(_) => None,
        }
    }
}

/// Python equality: numbers compare by value across `bool`, `int` and `float` (`1 == 1.0`).
impl PartialEq for ParamValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Str(a), Self::Str(b)) => a == b,
            (Self::Str(_), _) | (_, Self::Str(_)) => false,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            _ => self.as_f64() == other.as_f64(),
        }
    }
}

impl From<&ParamValue> for serde_json::Value {
    fn from(value: &ParamValue) -> Self {
        match value {
            ParamValue::Bool(b) => Self::from(*b),
            ParamValue::Int(i) => Self::from(*i),
            ParamValue::Float(f) => Self::from(*f),
            ParamValue::Str(s) => Self::from(s.as_str()),
        }
    }
}

impl From<f64> for ParamValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<i64> for ParamValue {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<bool> for ParamValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<&str> for ParamValue {
    fn from(value: &str) -> Self {
        Self::Str(value.to_owned())
    }
}

impl From<String> for ParamValue {
    fn from(value: String) -> Self {
        Self::Str(value)
    }
}

/// Parameter values by name, in insertion order.
pub type Params = IndexMap<String, ParamValue>;

/// The data of a [`Device`].
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceData {
    /// Stable ID (`dev_…`).
    pub id: String,
    /// The plugin this device instantiates.
    pub plugin_id: String,
    /// Display name.
    pub name: String,
    /// Parameter values.
    pub params: Params,
    /// Whether the device processes (a disabled effect is bypassed).
    pub enabled: bool,
    /// The track feeding the sidechain input.
    pub sidechain_id: Option<String>,
    /// The preset the parameters came from, cleared when a parameter changes.
    pub preset: Option<String>,
    /// Plugin state beyond parameters (for example an impulse response asset).
    pub state: JsonMap,
    /// Unknown keys preserved from newer project files.
    pub extra: JsonMap,
}

impl Validate for DeviceData {
    fn validate(&mut self) -> Result<()> {
        require(!self.plugin_id.is_empty(), || {
            "device plugin_id must not be empty".to_owned()
        })?;
        for (key, value) in &self.params {
            require(!key.is_empty(), || {
                "parameter names must be non-empty strings".to_owned()
            })?;
            if let ParamValue::Float(number) = value {
                require(number.is_finite(), || {
                    format!("parameter {} must be finite", str_repr(key))
                })?;
            }
        }
        Ok(())
    }
}

/// An instrument or effect on a track.
pub type Device = Valid<DeviceData>;

impl Device {
    /// A new enabled device with a fresh ID.
    ///
    /// # Errors
    ///
    /// The validation errors of [`DeviceData`].
    pub fn create(
        ids: &mut dyn IdGenerator,
        plugin_id: impl Into<String>,
        name: impl Into<String>,
        params: Params,
        sidechain_id: Option<String>,
    ) -> Result<Self> {
        Self::new(DeviceData {
            id: ids.new_id(ids::DEVICE),
            plugin_id: plugin_id.into(),
            name: name.into(),
            params,
            enabled: true,
            sidechain_id,
            preset: None,
            state: JsonMap::new(),
            extra: JsonMap::new(),
        })
    }

    /// The device with one parameter set; the preset no longer applies.
    ///
    /// # Errors
    ///
    /// The validation errors of [`DeviceData`].
    pub fn with_param(&self, name: impl Into<String>, value: ParamValue) -> Result<Self> {
        self.edit(|d| {
            d.params.insert(name.into(), value);
            d.preset = None;
        })
    }

    /// The device with several parameters set; the preset no longer applies.
    ///
    /// # Errors
    ///
    /// The validation errors of [`DeviceData`].
    pub fn with_params(
        &self,
        values: impl IntoIterator<Item = (String, ParamValue)>,
    ) -> Result<Self> {
        self.edit(|d| {
            d.params.extend(values);
            d.preset = None;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Device, ParamValue, Params};
    use crate::ids::SequentialIds;

    #[test]
    fn params_compare_like_python_numbers() {
        assert_eq!(ParamValue::Int(1), ParamValue::Float(1.0));
        assert_eq!(ParamValue::Bool(true), ParamValue::Int(1));
        assert_ne!(ParamValue::Str("1".into()), ParamValue::Int(1));
    }

    #[test]
    fn setting_a_parameter_keeps_its_position_and_clears_the_preset() {
        let mut ids = SequentialIds::new(0);
        let mut params = Params::new();
        params.insert("cutoff".into(), 1000.0.into());
        params.insert("q".into(), 0.7.into());
        let device = Device::create(&mut ids, "sonara.filter", "Filter", params, None)
            .and_then(|d| d.edit(|data| data.preset = Some("Warm".into())))
            .expect("valid device");
        let changed = device.with_param("cutoff", 500.0.into()).expect("valid");
        assert_eq!(changed.params.keys().collect::<Vec<_>>(), ["cutoff", "q"]);
        assert_eq!(changed.preset, None);
        let err = device
            .with_param("q", f64::NAN.into())
            .expect_err("non-finite parameter");
        assert_eq!(err.message, "parameter 'q' must be finite");
    }
}
