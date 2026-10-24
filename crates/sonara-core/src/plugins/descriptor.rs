//! Plugin descriptors: identity, capabilities, parameter schemas and presets
//! (`sonara.plugins.descriptor`).
//!
//! Descriptors are pure metadata. The application layer validates parameter values against
//! them, the GUI generates editors from them and the agent API exposes them as JSON. The
//! prototype's descriptors also carry a processor factory; here the processors live in
//! `sonara-dsp` and are looked up by plugin ID.

use indexmap::IndexMap;
use serde_json::Value;

use crate::error::{DomainError, ErrorCode, Result};
use crate::model::JsonMap;
use crate::model::device::{ParamValue, Params};
use crate::pyfmt::{float_g, float_repr, str_list_repr, str_repr};

/// Slack of the range check in [`ParamSpec::coerce`], as in the prototype.
const RANGE_SLACK: f64 = 1e-9;

/// Whether a plugin processes audio or generates it from notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginKind {
    /// An audio effect.
    Effect,
    /// An instrument played by MIDI notes.
    Instrument,
}

impl PluginKind {
    /// The kind as it appears in JSON, for example `"effect"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Effect => "effect",
            Self::Instrument => "instrument",
        }
    }
}

/// Where a plugin comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginFormat {
    /// A plugin built into Sonara.
    Builtin,
    /// A VST3 plugin.
    Vst3,
    /// An Audio Unit.
    Au,
    /// A CLAP plugin.
    Clap,
}

impl PluginFormat {
    /// The format as it appears in JSON, for example `"builtin"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Vst3 => "vst3",
            Self::Au => "au",
            Self::Clap => "clap",
        }
    }
}

/// The value type of a parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamKind {
    /// A float within a range.
    Float,
    /// An integer within a range.
    Int,
    /// A switch.
    Bool,
    /// One of a fixed list of strings.
    Enum,
}

impl ParamKind {
    /// The kind as it appears in JSON, for example `"float"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::Int => "int",
            Self::Bool => "bool",
            Self::Enum => "enum",
        }
    }
}

/// How a numeric parameter maps to a 0..1 control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamScale {
    /// Evenly spaced.
    Linear,
    /// Evenly spaced in the logarithm (frequencies, times); needs a positive minimum.
    Log,
}

impl ParamScale {
    /// The scale as it appears in JSON, for example `"log"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Log => "log",
        }
    }
}

/// The schema of one plugin parameter.
///
/// A valid spec ([`ParamSpec::validate`]) has both bounds and `minimum < maximum` for
/// [`ParamKind::Float`] and [`ParamKind::Int`], a non-empty `choices` list containing the default
/// for [`ParamKind::Enum`], and a positive minimum for [`ParamScale::Log`].
#[derive(Debug, Clone, PartialEq)]
pub struct ParamSpec {
    /// Stable parameter ID, the key in a device's parameter map.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Value type.
    pub kind: ParamKind,
    /// Default value, in the canonical type of `kind`.
    pub default: ParamValue,
    /// Lower bound of a numeric parameter.
    pub minimum: Option<f64>,
    /// Upper bound of a numeric parameter.
    pub maximum: Option<f64>,
    /// Unit shown next to the value; empty for none.
    pub unit: String,
    /// The allowed values of an enum parameter.
    pub choices: Vec<String>,
    /// Mapping to a 0..1 control.
    pub scale: ParamScale,
    /// Whether automation may drive the parameter.
    pub automatable: bool,
    /// Help text; empty for none.
    pub description: String,
}

impl ParamSpec {
    /// Checks the invariants that the prototype asserts when it builds a spec.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::Internal`] (`"invalid parameter spec '<id>'"`) when a numeric parameter lacks
    /// a bound or has `minimum >= maximum`, an enum's default is not one of its choices, or a
    /// log-scaled parameter has no positive minimum. A broken spec is a programming error.
    pub fn validate(&self) -> Result<()> {
        let numeric_ok = match (self.kind, self.minimum, self.maximum) {
            (ParamKind::Float | ParamKind::Int, Some(minimum), Some(maximum)) => minimum < maximum,
            (ParamKind::Float | ParamKind::Int, _, _) => false,
            _ => true,
        };
        let enum_ok = self.kind != ParamKind::Enum
            || matches!(&self.default, ParamValue::Str(d) if self.choices.contains(d));
        let scale_ok =
            self.scale != ParamScale::Log || self.minimum.is_some_and(|minimum| minimum > 0.0);
        if numeric_ok && enum_ok && scale_ok {
            Ok(())
        } else {
            Err(DomainError::new(
                ErrorCode::Internal,
                format!("invalid parameter spec {}", str_repr(&self.id)),
            ))
        }
    }

    /// Validates a value from JSON (tool arguments, project files) and returns it in the
    /// canonical type of the parameter, like the prototype's `coerce`.
    ///
    /// - [`ParamKind::Bool`] takes a boolean, or a number equal to 0 or 1.
    /// - [`ParamKind::Enum`] takes one of the choices, or an integer index into them.
    /// - [`ParamKind::Float`] and [`ParamKind::Int`] take a finite number within the range (with
    ///   a slack of 1e-9), clamp it to the range and return a float, or an integer rounded half
    ///   to even.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::InvalidArgument`]: `"parameter '<id>' must be a boolean"`,
    /// `"… must be one of ['a', 'b']"` (detail `choices`), `"… must be a number"`,
    /// `"… must be finite"` or `"… must be within <min>..<max> <unit>"` (details `minimum`,
    /// `maximum` and `unit`).
    pub fn coerce(&self, value: &Value) -> Result<ParamValue> {
        self.coerce_input(Input::from_json(value))
    }

    /// [`ParamSpec::coerce`] for an already typed value, with the same rules.
    ///
    /// # Errors
    ///
    /// The errors of [`ParamSpec::coerce`].
    pub fn coerce_param(&self, value: &ParamValue) -> Result<ParamValue> {
        self.coerce_input(Input::from_param(value))
    }

    #[expect(
        clippy::float_cmp,
        reason = "Python's `value in (0, 1)` compares exactly"
    )]
    fn coerce_input(&self, value: Input<'_>) -> Result<ParamValue> {
        let name = format!("parameter {}", str_repr(&self.id));
        match self.kind {
            ParamKind::Bool => match value {
                Input::Bool(flag) => Ok(ParamValue::Bool(flag)),
                Input::Int(number) if number == 0 || number == 1 => {
                    Ok(ParamValue::Bool(number == 1))
                }
                Input::Float(number) if number == 0.0 || number == 1.0 => {
                    Ok(ParamValue::Bool(number == 1.0))
                }
                _ => Err(DomainError::invalid(format!("{name} must be a boolean"))),
            },
            ParamKind::Enum => {
                let choice = match value {
                    Input::Str(text) => self.choices.iter().find(|c| *c == text),
                    Input::Int(index) => usize::try_from(index)
                        .ok()
                        .and_then(|index| self.choices.get(index)),
                    _ => None,
                };
                choice.map(|c| ParamValue::Str(c.clone())).ok_or_else(|| {
                    DomainError::invalid(format!(
                        "{name} must be one of {}",
                        str_list_repr(&self.choices)
                    ))
                    .with_detail("choices", self.choices.clone())
                })
            }
            ParamKind::Float | ParamKind::Int => {
                let number = match value {
                    #[expect(
                        clippy::cast_precision_loss,
                        reason = "Python's float(int) rounds the same way"
                    )]
                    Input::Int(number) => number as f64,
                    Input::Float(number) => number,
                    _ => return Err(DomainError::invalid(format!("{name} must be a number"))),
                };
                if !number.is_finite() {
                    return Err(DomainError::invalid(format!("{name} must be finite")));
                }
                // A valid spec has both bounds (`validate`); without one the range is open.
                let minimum = self.minimum.unwrap_or(f64::NEG_INFINITY);
                let maximum = self.maximum.unwrap_or(f64::INFINITY);
                if !(minimum - RANGE_SLACK <= number && number <= maximum + RANGE_SLACK) {
                    let message = format!(
                        "{name} must be within {}..{} {}",
                        float_g(minimum),
                        float_g(maximum),
                        self.unit
                    );
                    return Err(DomainError::invalid(message.trim_end())
                        .with_detail("minimum", minimum)
                        .with_detail("maximum", maximum)
                        .with_detail("unit", self.unit.as_str()));
                }
                let number = py_min(py_max(number, minimum), maximum);
                Ok(if self.kind == ParamKind::Int {
                    ParamValue::Int(round_to_int(number))
                } else {
                    ParamValue::Float(number)
                })
            }
        }
    }

    /// Maps a value to 0..1 for a GUI control.
    ///
    /// Where the prototype raises, this returns 0.0: for a value of the wrong type (an enum value
    /// that is not a choice, a non-numeric string), a numeric spec without bounds, and a
    /// non-positive value on a log scale.
    #[must_use]
    pub fn to_normalized(&self, value: &ParamValue) -> f64 {
        match self.kind {
            ParamKind::Bool => {
                if py_truthy(value) {
                    1.0
                } else {
                    0.0
                }
            }
            ParamKind::Enum => {
                let text = py_str(value);
                let Some(index) = self.choices.iter().position(|c| *c == text) else {
                    return 0.0;
                };
                usize_to_f64(index) / usize_to_f64(self.choices.len().saturating_sub(1).max(1))
            }
            ParamKind::Float | ParamKind::Int => {
                let (Some(minimum), Some(maximum), Some(v)) =
                    (self.minimum, self.maximum, py_float(value))
                else {
                    return 0.0;
                };
                if self.scale == ParamScale::Log {
                    let ratio = v / minimum;
                    if ratio <= 0.0 {
                        return 0.0;
                    }
                    ratio.ln() / (maximum / minimum).ln()
                } else {
                    (v - minimum) / (maximum - minimum)
                }
            }
        }
    }

    /// Maps a 0..1 control position to a value; `x` is clamped to 0..1 first.
    ///
    /// Booleans switch at 0.5, enums pick the nearest choice and integers round half to even.
    /// Where the prototype raises (an enum without choices, a numeric spec without bounds), this
    /// returns the default.
    #[must_use]
    pub fn from_normalized(&self, x: f64) -> ParamValue {
        let x = py_min(py_max(x, 0.0), 1.0);
        match self.kind {
            ParamKind::Bool => ParamValue::Bool(x >= 0.5),
            ParamKind::Enum => {
                let last = usize_to_f64(self.choices.len().saturating_sub(1));
                // `x` is within 0..1, so the index is within 0..=last.
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "the rounded position is a non-negative whole number <= last"
                )]
                let index = (x * last).round_ties_even() as usize;
                self.choices
                    .get(index)
                    .map_or_else(|| self.default.clone(), |c| ParamValue::Str(c.clone()))
            }
            ParamKind::Float | ParamKind::Int => {
                let (Some(minimum), Some(maximum)) = (self.minimum, self.maximum) else {
                    return self.default.clone();
                };
                let v = if self.scale == ParamScale::Log {
                    minimum * (maximum / minimum).powf(x)
                } else {
                    minimum + x * (maximum - minimum)
                };
                if self.kind == ParamKind::Int {
                    ParamValue::Int(round_to_int(v))
                } else {
                    ParamValue::Float(v)
                }
            }
        }
    }

    /// The spec as the agent API shows it. Keys in this order: `id`, `name`, `type`, `default`,
    /// `automatable`, then `min` and `max` for numeric parameters, `unit`, `choices`, `scale`
    /// (only `"log"`) and `description` when present.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut data = JsonMap::new();
        data.insert("id".to_owned(), Value::from(self.id.as_str()));
        data.insert("name".to_owned(), Value::from(self.name.as_str()));
        data.insert("type".to_owned(), Value::from(self.kind.as_str()));
        data.insert("default".to_owned(), Value::from(&self.default));
        data.insert("automatable".to_owned(), Value::from(self.automatable));
        if let Some(minimum) = self.minimum {
            data.insert("min".to_owned(), Value::from(minimum));
            data.insert(
                "max".to_owned(),
                self.maximum.map_or(Value::Null, Value::from),
            );
        }
        if !self.unit.is_empty() {
            data.insert("unit".to_owned(), Value::from(self.unit.as_str()));
        }
        if !self.choices.is_empty() {
            data.insert("choices".to_owned(), Value::from(self.choices.clone()));
        }
        if self.scale == ParamScale::Log {
            data.insert("scale".to_owned(), Value::from("log"));
        }
        if !self.description.is_empty() {
            data.insert(
                "description".to_owned(),
                Value::from(self.description.as_str()),
            );
        }
        Value::Object(data)
    }
}

/// A named set of parameter values.
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    /// Display name, matched case-insensitively by [`PluginDescriptor::preset`].
    pub name: String,
    /// The values the preset sets, in the prototype's order; other parameters keep their
    /// defaults.
    pub params: Params,
    /// Help text; empty for none.
    pub description: String,
}

/// A plugin's identity, capabilities, parameter schema and presets.
///
/// A valid descriptor ([`PluginDescriptor::validate`]) has distinct parameter IDs, valid
/// parameter specs and presets whose values pass [`PluginDescriptor::validate_param_values`].
#[derive(Debug, Clone, PartialEq)]
pub struct PluginDescriptor {
    /// Stable plugin ID, for example `"sonara.eq.parametric"`.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Effect or instrument.
    pub kind: PluginKind,
    /// Browser category, for example `"dynamics"`.
    pub category: String,
    /// Parameter schemas, in display order.
    pub params: Vec<ParamSpec>,
    /// Help text.
    pub description: String,
    /// Vendor name.
    pub vendor: String,
    /// Version of the plugin's parameter set.
    pub version: u32,
    /// Where the plugin comes from.
    pub format: PluginFormat,
    /// Whether the plugin takes a sidechain input.
    pub sidechain: bool,
    /// Factory presets.
    pub presets: Vec<Preset>,
    /// Search tags.
    pub tags: Vec<String>,
}

impl PluginDescriptor {
    /// Checks the invariants that the prototype checks when it builds a descriptor.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::Internal`] for duplicate parameter IDs
    /// (`"duplicate parameter ids in <id>"`) or an invalid spec ([`ParamSpec::validate`]), and
    /// the error of [`PluginDescriptor::validate_param_values`] for an invalid preset.
    pub fn validate(&self) -> Result<()> {
        for (index, spec) in self.params.iter().enumerate() {
            if self.params.iter().take(index).any(|p| p.id == spec.id) {
                return Err(DomainError::new(
                    ErrorCode::Internal,
                    format!("duplicate parameter ids in {}", self.id),
                ));
            }
        }
        for spec in &self.params {
            spec.validate()?;
        }
        for preset in &self.presets {
            self.validate_param_values(&preset.params)?;
        }
        Ok(())
    }

    /// The spec of parameter `param_id`.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::InvalidArgument`]: `"<name> has no parameter '<param_id>'"`, with the
    /// parameter IDs as detail `available`.
    pub fn param(&self, param_id: &str) -> Result<&ParamSpec> {
        self.params
            .iter()
            .find(|p| p.id == param_id)
            .ok_or_else(|| {
                let available: Vec<&str> = self.params.iter().map(|p| p.id.as_str()).collect();
                DomainError::invalid(format!(
                    "{} has no parameter {}",
                    self.name,
                    str_repr(param_id)
                ))
                .with_detail("available", available)
            })
    }

    /// Whether the plugin has parameter `param_id`.
    #[must_use]
    pub fn has_param(&self, param_id: &str) -> bool {
        self.params.iter().any(|p| p.id == param_id)
    }

    /// Every parameter's default value, in parameter order.
    #[must_use]
    pub fn defaults(&self) -> Params {
        self.params
            .iter()
            .map(|p| (p.id.clone(), p.default.clone()))
            .collect()
    }

    /// Validates JSON values ([`ParamSpec::coerce`]) and returns them in canonical types, in
    /// the input order.
    ///
    /// # Errors
    ///
    /// The first error of [`PluginDescriptor::param`] or [`ParamSpec::coerce`], in input order.
    pub fn validate_params(&self, values: &JsonMap) -> Result<Params> {
        values
            .iter()
            .map(|(key, value)| Ok((key.clone(), self.param(key)?.coerce(value)?)))
            .collect()
    }

    /// [`PluginDescriptor::validate_params`] for already typed values
    /// ([`ParamSpec::coerce_param`]).
    ///
    /// # Errors
    ///
    /// The first error of [`PluginDescriptor::param`] or [`ParamSpec::coerce_param`], in input
    /// order.
    pub fn validate_param_values(&self, values: &Params) -> Result<Params> {
        values
            .iter()
            .map(|(key, value)| Ok((key.clone(), self.param(key)?.coerce_param(value)?)))
            .collect()
    }

    /// The full parameter set: the defaults, in parameter order, overlaid with the stored
    /// `values`. Keys that are not parameters are dropped; values are not validated.
    #[must_use]
    pub fn resolve(&self, values: &Params) -> Params {
        let mut out = self.defaults();
        for (key, value) in values {
            if let Some(slot) = out.get_mut(key) {
                *slot = value.clone();
            }
        }
        out
    }

    /// The preset called `name`, compared case-insensitively.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NotFound`]: `"<plugin name> has no preset '<name>'"`, with the preset names
    /// as detail `available`.
    pub fn preset(&self, name: &str) -> Result<&Preset> {
        let wanted = name.to_lowercase();
        self.presets
            .iter()
            .find(|p| p.name.to_lowercase() == wanted)
            .ok_or_else(|| {
                let available: Vec<&str> = self.presets.iter().map(|p| p.name.as_str()).collect();
                DomainError::new(
                    ErrorCode::NotFound,
                    format!("{} has no preset {}", self.name, str_repr(name)),
                )
                .with_detail("available", available)
            })
    }

    /// The descriptor as the agent API shows it. Keys in this order: `plugin_id`, `name`,
    /// `kind`, `category`, `format`, `vendor`, `description`, `sidechain`, `presets` (the
    /// names), then `parameters` ([`ParamSpec::to_json`]) when `include_params` is set.
    #[must_use]
    pub fn to_json(&self, include_params: bool) -> Value {
        let mut data = JsonMap::new();
        data.insert("plugin_id".to_owned(), Value::from(self.id.as_str()));
        data.insert("name".to_owned(), Value::from(self.name.as_str()));
        data.insert("kind".to_owned(), Value::from(self.kind.as_str()));
        data.insert("category".to_owned(), Value::from(self.category.as_str()));
        data.insert("format".to_owned(), Value::from(self.format.as_str()));
        data.insert("vendor".to_owned(), Value::from(self.vendor.as_str()));
        data.insert(
            "description".to_owned(),
            Value::from(self.description.as_str()),
        );
        data.insert("sidechain".to_owned(), Value::from(self.sidechain));
        let presets: Vec<&str> = self.presets.iter().map(|p| p.name.as_str()).collect();
        data.insert("presets".to_owned(), Value::from(presets));
        if include_params {
            let params: Vec<Value> = self.params.iter().map(ParamSpec::to_json).collect();
            data.insert("parameters".to_owned(), Value::Array(params));
        }
        Value::Object(data)
    }
}

/// Index of the available plugins, in registration order. Populated at bootstrap and read-only
/// afterwards.
#[derive(Debug, Clone, Default)]
pub struct PluginRegistry {
    plugins: IndexMap<String, PluginDescriptor>,
}

impl PluginRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `descriptor`.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::Internal`]: `"plugin '<id>' registered twice"` when the ID is taken. The
    /// prototype raises a `ValueError`, which its agent boundary reports as `INTERNAL`.
    pub fn register(&mut self, descriptor: PluginDescriptor) -> Result<()> {
        if self.plugins.contains_key(&descriptor.id) {
            return Err(DomainError::new(
                ErrorCode::Internal,
                format!("plugin {} registered twice", str_repr(&descriptor.id)),
            ));
        }
        self.plugins.insert(descriptor.id.clone(), descriptor);
        Ok(())
    }

    /// The plugin `plugin_id`.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NotFound`]: `"unknown plugin '<plugin_id>'"`, with the sorted plugin IDs as
    /// detail `available`.
    pub fn get(&self, plugin_id: &str) -> Result<&PluginDescriptor> {
        self.plugins.get(plugin_id).ok_or_else(|| {
            let mut available: Vec<&str> = self.plugins.keys().map(String::as_str).collect();
            available.sort_unstable();
            DomainError::new(
                ErrorCode::NotFound,
                format!("unknown plugin {}", str_repr(plugin_id)),
            )
            .with_detail("available", available)
        })
    }

    /// The plugin `plugin_id`, if registered.
    #[must_use]
    pub fn find(&self, plugin_id: &str) -> Option<&PluginDescriptor> {
        self.plugins.get(plugin_id)
    }

    /// The plugins of `kind` (all plugins for `None`), in registration order.
    #[must_use]
    pub fn all(&self, kind: Option<PluginKind>) -> Vec<&PluginDescriptor> {
        self.plugins
            .values()
            .filter(|d| kind.is_none_or(|kind| d.kind == kind))
            .collect()
    }

    /// Whether `plugin_id` is registered.
    #[must_use]
    pub fn contains(&self, plugin_id: &str) -> bool {
        self.plugins.contains_key(plugin_id)
    }
}

/// A value as the prototype's `coerce` sees it: the JSON or typed input reduced to the cases its
/// `isinstance` checks tell apart. Python bools are ints, so `Bool` is checked first there; here
/// it is a separate case.
#[derive(Clone, Copy)]
enum Input<'a> {
    Bool(bool),
    Int(i128),
    Float(f64),
    Str(&'a str),
    /// `null`, arrays and objects.
    Other,
}

impl<'a> Input<'a> {
    fn from_json(value: &'a Value) -> Self {
        match value {
            Value::Bool(flag) => Self::Bool(*flag),
            Value::Number(number) => {
                if let Some(int) = number.as_i64() {
                    Self::Int(i128::from(int))
                } else if let Some(int) = number.as_u64() {
                    Self::Int(i128::from(int))
                } else {
                    // Without `arbitrary_precision` every other number is an f64.
                    Self::Float(number.as_f64().unwrap_or(f64::NAN))
                }
            }
            Value::String(text) => Self::Str(text),
            Value::Null | Value::Array(_) | Value::Object(_) => Self::Other,
        }
    }

    fn from_param(value: &'a ParamValue) -> Self {
        match value {
            ParamValue::Bool(flag) => Self::Bool(*flag),
            ParamValue::Int(int) => Self::Int(i128::from(*int)),
            ParamValue::Float(number) => Self::Float(*number),
            ParamValue::Str(text) => Self::Str(text),
        }
    }
}

/// Python's `max(a, b)`: `a` unless `b > a` (so a NaN `a` and the sign of an equal zero stay).
fn py_max(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// Python's `min(a, b)`: `a` unless `b < a`.
fn py_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// Python's `round(x)` for a float: half to even, saturating where Python raises (NaN,
/// infinities) or would return an integer beyond `i64`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "callers pass values clamped to the parameter range; `as` saturates otherwise"
)]
fn round_to_int(x: f64) -> i64 {
    x.round_ties_even() as i64
}

#[expect(
    clippy::cast_precision_loss,
    reason = "choice counts are far below 2^52"
)]
fn usize_to_f64(n: usize) -> f64 {
    n as f64
}

/// Python's truthiness of a parameter value.
fn py_truthy(value: &ParamValue) -> bool {
    match value {
        ParamValue::Bool(flag) => *flag,
        ParamValue::Int(int) => *int != 0,
        ParamValue::Float(number) => *number != 0.0,
        ParamValue::Str(text) => !text.is_empty(),
    }
}

/// Python's `str(value)` of a parameter value.
fn py_str(value: &ParamValue) -> String {
    match value {
        ParamValue::Bool(true) => "True".to_owned(),
        ParamValue::Bool(false) => "False".to_owned(),
        ParamValue::Int(int) => int.to_string(),
        ParamValue::Float(number) => float_repr(*number),
        ParamValue::Str(text) => text.clone(),
    }
}

/// Python's `float(value)` of a parameter value; `None` where Python raises.
fn py_float(value: &ParamValue) -> Option<f64> {
    match value {
        ParamValue::Str(text) => text.trim().parse().ok(),
        other => other.as_f64(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{
        ParamKind, ParamScale, ParamSpec, PluginDescriptor, PluginFormat, PluginKind,
        PluginRegistry, Preset,
    };
    use crate::error::ErrorCode;
    use crate::model::JsonMap;
    use crate::model::device::{ParamValue, Params};

    fn spec(id: &str, kind: ParamKind, default: ParamValue) -> ParamSpec {
        ParamSpec {
            id: id.to_owned(),
            name: id.to_uppercase(),
            kind,
            default,
            minimum: None,
            maximum: None,
            unit: String::new(),
            choices: Vec::new(),
            scale: ParamScale::Linear,
            automatable: true,
            description: String::new(),
        }
    }

    fn ranged(id: &str, kind: ParamKind, default: ParamValue, lo: f64, hi: f64) -> ParamSpec {
        ParamSpec {
            minimum: Some(lo),
            maximum: Some(hi),
            ..spec(id, kind, default)
        }
    }

    fn cutoff() -> ParamSpec {
        ParamSpec {
            unit: "Hz".to_owned(),
            scale: ParamScale::Log,
            ..ranged("cut", ParamKind::Float, 1000.0.into(), 20.0, 20000.0)
        }
    }

    fn mode() -> ParamSpec {
        ParamSpec {
            choices: vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
            automatable: false,
            ..spec("m", ParamKind::Enum, "a".into())
        }
    }

    /// Typed equality: `Int(1)` and `Float(1.0)` differ (unlike `ParamValue`'s Python equality).
    fn same(a: &ParamValue, b: &ParamValue) -> bool {
        format!("{a:?}") == format!("{b:?}")
    }

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    fn keys(value: &Value) -> Vec<&str> {
        value
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect()
    }

    #[test]
    fn float_param_coercion_and_normalization() {
        let s = cutoff();
        assert!(same(
            &s.coerce(&json!(440)).unwrap(),
            &ParamValue::Float(440.0)
        ));
        // Within the slack, then clamped.
        assert!(same(
            &s.coerce(&json!(20_000.000_000_000_1)).unwrap(),
            &ParamValue::Float(20000.0)
        ));
        for bad in [
            json!(19.0),
            json!(30000),
            json!("100"),
            json!(true),
            json!(null),
        ] {
            let err = s.coerce(&bad).unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidArgument, "{bad}");
        }
        let err = s.coerce_param(&ParamValue::Float(f64::NAN)).unwrap_err();
        assert_eq!(err.message, "parameter 'cut' must be finite");
        assert!(close(s.to_normalized(&20.0.into()), 0.0, 1e-12));
        assert!(close(s.to_normalized(&20000.0.into()), 1.0, 1e-12));
        assert!(close(s.to_normalized(&632.4555.into()), 0.5, 1e-6));
        let ParamValue::Float(mid) = s.from_normalized(0.5) else {
            panic!("float expected")
        };
        assert!(close(mid, 632.4555, 632.4555 * 1e-6));
        assert!(same(&s.from_normalized(2.0), &ParamValue::Float(20000.0)));
        let js = s.to_json();
        assert_eq!(js["min"], json!(20.0));
        assert_eq!(js["scale"], json!("log"));
        assert_eq!(js["unit"], json!("Hz"));
    }

    #[test]
    fn int_bool_enum_params() {
        let i = ranged("n", ParamKind::Int, 4.into(), 1.0, 32.0);
        assert!(same(&i.coerce(&json!(7.6)).unwrap(), &ParamValue::Int(8)));
        assert!(same(&i.coerce(&json!(7.0)).unwrap(), &ParamValue::Int(7)));
        // Python's round() rounds half to even.
        assert!(same(&i.coerce(&json!(2.5)).unwrap(), &ParamValue::Int(2)));
        assert!(same(&i.coerce(&json!(3.5)).unwrap(), &ParamValue::Int(4)));
        assert!(same(&i.from_normalized(1.0), &ParamValue::Int(32)));
        assert!(close(i.to_normalized(&1.into()), 0.0, 1e-12));

        let b = spec("on", ParamKind::Bool, false.into());
        assert!(same(
            &b.coerce(&json!(true)).unwrap(),
            &ParamValue::Bool(true)
        ));
        assert!(same(
            &b.coerce(&json!(0)).unwrap(),
            &ParamValue::Bool(false)
        ));
        assert!(same(
            &b.coerce(&json!(1.0)).unwrap(),
            &ParamValue::Bool(true)
        ));
        assert!(same(
            &b.coerce(&json!(-0.0)).unwrap(),
            &ParamValue::Bool(false)
        ));
        for bad in [json!(2), json!("true"), json!(0.5), json!(null)] {
            let err = b.coerce(&bad).unwrap_err();
            assert_eq!(err.message, "parameter 'on' must be a boolean", "{bad}");
        }
        assert!(same(&b.from_normalized(0.7), &ParamValue::Bool(true)));
        assert!(close(b.to_normalized(&false.into()), 0.0, 1e-12));

        let e = mode();
        assert!(same(
            &e.coerce(&json!("b")).unwrap(),
            &ParamValue::Str("b".into())
        ));
        assert!(same(
            &e.coerce(&json!(2)).unwrap(),
            &ParamValue::Str("c".into())
        ));
        for bad in [json!("d"), json!(3), json!(-1), json!(true), json!(1.0)] {
            let err = e.coerce(&bad).unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidArgument, "{bad}");
            assert_eq!(err.message, "parameter 'm' must be one of ['a', 'b', 'c']");
            assert_eq!(err.details["choices"], json!(["a", "b", "c"]));
        }
        assert!(close(e.to_normalized(&"c".into()), 1.0, 1e-12));
        assert!(same(&e.from_normalized(0.49), &ParamValue::Str("b".into())));
        assert_eq!(e.to_json()["choices"], json!(["a", "b", "c"]));
    }

    #[test]
    fn typed_coercion_follows_the_same_rules() {
        let f = ranged("g", ParamKind::Float, 0.0.into(), -12.0, 12.0);
        assert!(same(
            &f.coerce_param(&ParamValue::Int(3)).unwrap(),
            &ParamValue::Float(3.0)
        ));
        let err = f.coerce_param(&ParamValue::Bool(true)).unwrap_err();
        assert_eq!(err.message, "parameter 'g' must be a number");
        let err = f.coerce_param(&ParamValue::Str("3".into())).unwrap_err();
        assert_eq!(err.message, "parameter 'g' must be a number");
        let b = spec("on", ParamKind::Bool, false.into());
        assert!(same(
            &b.coerce_param(&ParamValue::Int(1)).unwrap(),
            &ParamValue::Bool(true)
        ));
        let e = mode();
        assert!(same(
            &e.coerce_param(&ParamValue::Int(1)).unwrap(),
            &ParamValue::Str("b".into())
        ));
        assert!(e.coerce_param(&ParamValue::Bool(true)).is_err());
        assert!(e.coerce_param(&ParamValue::Float(1.0)).is_err());
    }

    #[test]
    fn range_errors_match_the_prototype() {
        let err = cutoff().coerce(&json!(19.0)).unwrap_err();
        assert_eq!(err.message, "parameter 'cut' must be within 20..20000 Hz");
        assert_eq!(
            Value::Object(err.details),
            json!({"minimum": 20.0, "maximum": 20000.0, "unit": "Hz"})
        );
        // An empty unit leaves no trailing space (`.rstrip()`).
        let mix = ranged("mix", ParamKind::Float, 1.0.into(), 0.0, 1.0);
        let err = mix.coerce(&json!(1.5)).unwrap_err();
        assert_eq!(err.message, "parameter 'mix' must be within 0..1");
        let attack = ParamSpec {
            unit: "s".to_owned(),
            ..ranged("a", ParamKind::Float, 0.005.into(), 0.0005, 10.0)
        };
        let err = attack.coerce(&json!(-1)).unwrap_err();
        assert_eq!(err.message, "parameter 'a' must be within 0.0005..10 s");
        let wide = ranged("w", ParamKind::Float, 0.0.into(), 1e-5, 1_234_567.0);
        let err = wide.coerce(&json!(-3)).unwrap_err();
        assert_eq!(
            err.message,
            "parameter 'w' must be within 1e-05..1.23457e+06"
        );
    }

    #[test]
    fn param_spec_invariants() {
        let unbounded = spec("x", ParamKind::Float, 0.0.into());
        let reversed = ranged("x", ParamKind::Float, 0.0.into(), 1.0, 0.0);
        let bad_enum = ParamSpec {
            choices: vec!["a".to_owned()],
            ..spec("x", ParamKind::Enum, "z".into())
        };
        let bad_log = ParamSpec {
            scale: ParamScale::Log,
            ..ranged("x", ParamKind::Float, 1.0.into(), 0.0, 10.0)
        };
        for broken in [unbounded, reversed, bad_enum, bad_log] {
            let err = broken.validate().unwrap_err();
            assert_eq!(err.code, ErrorCode::Internal);
            assert_eq!(err.message, "invalid parameter spec 'x'");
        }
        assert!(cutoff().validate().is_ok());
        assert!(mode().validate().is_ok());
    }

    #[test]
    fn normalization_where_the_prototype_raises() {
        let e = mode();
        assert!(close(e.to_normalized(&"zz".into()), 0.0, 1e-12));
        let slopes = ParamSpec {
            choices: ["12", "24", "36", "48"].map(str::to_owned).to_vec(),
            ..spec("slope", ParamKind::Enum, "12".into())
        };
        // `str(24)` is a choice.
        assert!(close(slopes.to_normalized(&24.into()), 1.0 / 3.0, 1e-12));
        assert!(close(cutoff().to_normalized(&(-5.0).into()), 0.0, 1e-12));
        assert!(close(cutoff().to_normalized(&"20000".into()), 1.0, 1e-12));
        let ParamValue::Float(nan) = cutoff().from_normalized(f64::NAN) else {
            panic!("float expected")
        };
        assert!(nan.is_nan());
    }

    fn descriptor(presets: Vec<Preset>) -> PluginDescriptor {
        let gain = ParamSpec {
            unit: "dB".to_owned(),
            ..ranged("gain", ParamKind::Float, 0.0.into(), -12.0, 12.0)
        };
        let mode = ParamSpec {
            choices: vec!["soft".to_owned(), "hard".to_owned()],
            ..spec("mode", ParamKind::Enum, "soft".into())
        };
        PluginDescriptor {
            id: "test.fx".to_owned(),
            name: "Test FX".to_owned(),
            kind: PluginKind::Effect,
            category: "test".to_owned(),
            params: vec![gain, mode],
            description: String::new(),
            vendor: "Sonara".to_owned(),
            version: 1,
            format: PluginFormat::Builtin,
            sidechain: false,
            presets,
            tags: Vec::new(),
        }
    }

    fn preset(name: &str, params: Params) -> Preset {
        Preset {
            name: name.to_owned(),
            params,
            description: String::new(),
        }
    }

    fn json_map(value: &Value) -> JsonMap {
        value.as_object().expect("object").clone()
    }

    #[test]
    fn descriptor_helpers() {
        let loud = preset("Loud", Params::from([("gain".to_owned(), 6.0.into())]));
        let d = descriptor(vec![loud]);
        assert!(d.validate().is_ok());
        let defaults = d.defaults();
        assert_eq!(defaults.keys().collect::<Vec<_>>(), ["gain", "mode"]);
        assert!(same(&defaults["gain"], &ParamValue::Float(0.0)));
        assert!(same(&defaults["mode"], &ParamValue::Str("soft".into())));

        let valid = d.validate_params(&json_map(&json!({"gain": 3}))).unwrap();
        assert_eq!(valid.len(), 1);
        assert!(same(&valid["gain"], &ParamValue::Float(3.0)));
        // Input order is kept.
        let valid = d
            .validate_params(&json_map(&json!({"mode": 1, "gain": -2})))
            .unwrap();
        assert_eq!(valid.keys().collect::<Vec<_>>(), ["mode", "gain"]);
        assert!(same(&valid["mode"], &ParamValue::Str("hard".into())));

        let err = d
            .validate_params(&json_map(&json!({"volume": 1})))
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidArgument);
        assert_eq!(err.message, "Test FX has no parameter 'volume'");
        assert_eq!(err.details["available"], json!(["gain", "mode"]));

        let stored = Params::from([
            ("stale".to_owned(), 1.into()),
            ("gain".to_owned(), 2.0.into()),
        ]);
        let resolved = d.resolve(&stored);
        assert_eq!(resolved.keys().collect::<Vec<_>>(), ["gain", "mode"]);
        assert!(same(&resolved["gain"], &ParamValue::Float(2.0)));
        assert!(same(&resolved["mode"], &ParamValue::Str("soft".into())));

        assert_eq!(d.preset("loud").unwrap().name, "Loud");
        assert_eq!(d.preset("LOUD").unwrap().name, "Loud");
        let err = d.preset("quiet").unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        assert_eq!(err.message, "Test FX has no preset 'quiet'");
        assert_eq!(err.details["available"], json!(["Loud"]));

        let js = d.to_json(true);
        assert_eq!(js["plugin_id"], json!("test.fx"));
        assert_eq!(js["presets"], json!(["Loud"]));
        assert_eq!(js["parameters"].as_array().unwrap().len(), 2);
        assert!(d.to_json(false).get("parameters").is_none());
        assert!(d.has_param("gain"));
        assert!(!d.has_param("x"));
        assert_eq!(d.param("mode").unwrap().id, "mode");
    }

    #[test]
    fn json_key_order_matches_the_prototype() {
        let d = descriptor(Vec::new());
        assert_eq!(
            keys(&d.to_json(true)),
            [
                "plugin_id",
                "name",
                "kind",
                "category",
                "format",
                "vendor",
                "description",
                "sidechain",
                "presets",
                "parameters"
            ]
        );
        assert_eq!(
            keys(&d.params[0].to_json()),
            [
                "id",
                "name",
                "type",
                "default",
                "automatable",
                "min",
                "max",
                "unit"
            ]
        );
        let full = ParamSpec {
            description: "help".to_owned(),
            ..cutoff()
        };
        assert_eq!(
            keys(&full.to_json()),
            [
                "id",
                "name",
                "type",
                "default",
                "automatable",
                "min",
                "max",
                "unit",
                "scale",
                "description"
            ]
        );
        assert_eq!(
            mode().to_json(),
            json!({"id": "m", "name": "M", "type": "enum", "default": "a", "automatable": false,
                   "choices": ["a", "b", "c"]})
        );
        assert_eq!(
            keys(&mode().to_json()),
            ["id", "name", "type", "default", "automatable", "choices"]
        );
        // Int defaults stay integers, float defaults floats.
        let n = ranged("n", ParamKind::Int, 4.into(), 1.0, 32.0);
        assert!(n.to_json()["default"].is_i64());
        assert!(n.to_json()["min"].is_f64());
        assert!(cutoff().to_json()["default"].is_f64());
    }

    #[test]
    fn invalid_presets_and_duplicates_rejected() {
        let bad = preset("Bad", Params::from([("gain".to_owned(), 99.0.into())]));
        let err = descriptor(vec![bad]).validate().unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidArgument);
        assert_eq!(err.message, "parameter 'gain' must be within -12..12 dB");
        let mut dup = descriptor(Vec::new());
        dup.params = vec![
            spec("a", ParamKind::Bool, false.into()),
            spec("a", ParamKind::Bool, true.into()),
        ];
        let err = dup.validate().unwrap_err();
        assert_eq!(err.code, ErrorCode::Internal);
        assert_eq!(err.message, "duplicate parameter ids in test.fx");
    }

    #[test]
    fn registry() {
        let mut reg = PluginRegistry::new();
        let d = descriptor(Vec::new());
        reg.register(d.clone()).unwrap();
        let err = reg.register(d.clone()).unwrap_err();
        assert_eq!(err.code, ErrorCode::Internal);
        assert_eq!(err.message, "plugin 'test.fx' registered twice");
        assert_eq!(reg.get("test.fx").unwrap(), &d);
        assert!(reg.find("nope").is_none());
        assert!(reg.contains("test.fx"));
        let err = reg.get("nope").unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        assert_eq!(err.message, "unknown plugin 'nope'");
        assert_eq!(err.details["available"], json!(["test.fx"]));
        assert_eq!(
            reg.all(Some(PluginKind::Instrument)),
            Vec::<&PluginDescriptor>::new()
        );
        assert_eq!(reg.all(None), [&d]);

        let mut other = descriptor(Vec::new());
        other.id = "a.first".to_owned();
        other.kind = PluginKind::Instrument;
        reg.register(other).unwrap();
        let ids: Vec<&str> = reg.all(None).iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["test.fx", "a.first"]);
        let err = reg.get("zzz").unwrap_err();
        assert_eq!(err.details["available"], json!(["a.first", "test.fx"]));
        assert_eq!(reg.all(Some(PluginKind::Instrument)).len(), 1);
    }

    #[test]
    fn enum_strings_match_the_prototype() {
        assert_eq!(PluginKind::Effect.as_str(), "effect");
        assert_eq!(PluginKind::Instrument.as_str(), "instrument");
        assert_eq!(
            [
                PluginFormat::Builtin,
                PluginFormat::Vst3,
                PluginFormat::Au,
                PluginFormat::Clap
            ]
            .map(PluginFormat::as_str),
            ["builtin", "vst3", "au", "clap"]
        );
        assert_eq!(
            [
                ParamKind::Float,
                ParamKind::Int,
                ParamKind::Bool,
                ParamKind::Enum
            ]
            .map(ParamKind::as_str),
            ["float", "int", "bool", "enum"]
        );
        assert_eq!(ParamScale::Linear.as_str(), "linear");
        assert_eq!(ParamScale::Log.as_str(), "log");
    }
}
