//! Real-time: the slot reads run on the audio render thread (see docs/audio-engine.md);
//! [`param_slots`] is setup.
//!
//! Parameter sets: the conversion of a device's stored values into the [`ParamSlots`] that
//! [`Processor::set_params`](crate::processor::Processor::set_params) takes, and the typed slot
//! reads the processors decode them with.

use std::fmt;

use sonara_core::error::DomainError;
use sonara_core::model::device::{ParamValue, Params};
use sonara_core::plugins::{ParamSpec, PluginDescriptor};

use crate::processor::{MAX_PARAMS, ParamSlot, ParamSlots};

/// Why [`param_slots`] could not convert a parameter set.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamSlotsError {
    /// A value does not fit its parameter's schema ([`ParamSpec::coerce_param`]).
    InvalidValue {
        /// The plugin whose parameters were converted.
        plugin: String,
        /// The schema violation.
        source: Box<DomainError>,
    },
    /// The descriptor has more parameters than a [`ParamSlots`] holds ([`MAX_PARAMS`]).
    TooManyParams {
        /// The plugin whose parameters were converted.
        plugin: String,
        /// Its number of parameters.
        count: usize,
    },
    /// An enum parameter has more choices than a slot can index (65 536).
    TooManyChoices {
        /// The plugin whose parameters were converted.
        plugin: String,
        /// The enum parameter.
        param: String,
    },
}

impl fmt::Display for ParamSlotsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue { plugin, source } => {
                write!(f, "cannot convert the parameters of {plugin}: {source}")
            }
            Self::TooManyParams { plugin, count } => write!(
                f,
                "{plugin} has {count} parameters, more than the {MAX_PARAMS} a processor takes"
            ),
            Self::TooManyChoices { plugin, param } => write!(
                f,
                "parameter {param} of {plugin} has more choices than a slot can index"
            ),
        }
    }
}

impl std::error::Error for ParamSlotsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidValue { source, .. } => Some(source.as_ref()),
            Self::TooManyParams { .. } | Self::TooManyChoices { .. } => None,
        }
    }
}

/// The full parameter set of a device as its processor takes it: the descriptor's defaults
/// overlaid with `values` ([`PluginDescriptor::resolve`]), in descriptor order, each value in
/// its canonical type ([`ParamSpec::coerce_param`]) and each enum choice as its index into the
/// descriptor's choices.
///
/// `values` are the device's stored values, normally already validated
/// ([`PluginDescriptor::validate_param_values`]); keys that are not parameters are dropped, as
/// `resolve` drops them. Validating again is idempotent, so a value that passed validation keeps
/// its meaning.
///
/// Setup: allocates, so it runs off the real-time thread, whenever a device's values change.
///
/// # Errors
///
/// - [`ParamSlotsError::InvalidValue`] when a value does not fit its parameter, with the
///   schema error as the source;
/// - [`ParamSlotsError::TooManyParams`] when the descriptor has more than [`MAX_PARAMS`]
///   parameters;
/// - [`ParamSlotsError::TooManyChoices`] when an enum has more choices than a slot indexes.
///
/// # Examples
///
/// ```
/// use sonara_core::model::device::{ParamValue, Params};
/// use sonara_core::plugins::builtin_registry;
/// use sonara_dsp::processor::ParamSlot;
/// use sonara_dsp::processors::param_slots;
///
/// let registry = builtin_registry();
/// let gate = registry.get("sonara.dynamics.gate").unwrap();
/// let values = Params::from([("mode".to_owned(), ParamValue::from("expander"))]);
/// let slots = param_slots(gate, &values).unwrap();
/// assert_eq!(slots.as_slice().len(), gate.params.len());
/// // The enum is its choice index; every other parameter keeps its default.
/// assert_eq!(slots.get(0), Some(ParamSlot::Choice(1)));
/// assert_eq!(slots.get(1), Some(ParamSlot::Float(-40.0)));
/// ```
#[expect(
    clippy::disallowed_methods,
    reason = "setup: runs off the real-time thread whenever a device's values change"
)]
pub fn param_slots(
    descriptor: &PluginDescriptor,
    values: &Params,
) -> Result<ParamSlots, ParamSlotsError> {
    let count = descriptor.params.len();
    if count > MAX_PARAMS {
        return Err(ParamSlotsError::TooManyParams {
            plugin: descriptor.id.clone(),
            count,
        });
    }
    let resolved = descriptor.resolve(values);
    let mut slots = [ParamSlot::Bool(false); MAX_PARAMS];
    for (slot, spec) in slots.iter_mut().zip(&descriptor.params) {
        // `resolve` starts from the defaults, so every parameter has a value.
        let value = resolved.get(&spec.id).unwrap_or(&spec.default);
        let canonical =
            spec.coerce_param(value)
                .map_err(|source| ParamSlotsError::InvalidValue {
                    plugin: descriptor.id.clone(),
                    source: Box::new(source),
                })?;
        *slot = to_slot(spec, &canonical).ok_or_else(|| ParamSlotsError::TooManyChoices {
            plugin: descriptor.id.clone(),
            param: spec.id.clone(),
        })?;
    }
    let slots = slots.get(..count).unwrap_or_default();
    ParamSlots::new(slots).ok_or_else(|| ParamSlotsError::TooManyParams {
        plugin: descriptor.id.clone(),
        count,
    })
}

/// The slot of a canonical value (`coerce_param` returns the parameter kind's type: a float, an
/// integer, a boolean or one of the choices); `None` for a choice index beyond `u16`.
fn to_slot(spec: &ParamSpec, canonical: &ParamValue) -> Option<ParamSlot> {
    Some(match canonical {
        ParamValue::Float(value) => ParamSlot::Float(*value),
        ParamValue::Int(value) => ParamSlot::Int(*value),
        ParamValue::Bool(value) => ParamSlot::Bool(*value),
        ParamValue::Str(choice) => {
            let index = spec.choices.iter().position(|c| c == choice)?;
            ParamSlot::Choice(u16::try_from(index).ok()?)
        }
    })
}

/// The float in slot `index`, or `None` when the slot is missing or holds another type.
pub(crate) fn float(slots: &ParamSlots, index: usize) -> Option<f64> {
    match slots.get(index)? {
        ParamSlot::Float(value) => Some(value),
        _ => None,
    }
}

/// The integer in slot `index`, or `None` when the slot is missing or holds another type.
pub(crate) fn int(slots: &ParamSlots, index: usize) -> Option<i64> {
    match slots.get(index)? {
        ParamSlot::Int(value) => Some(value),
        _ => None,
    }
}

/// The switch in slot `index`, or `None` when the slot is missing or holds another type.
pub(crate) fn flag(slots: &ParamSlots, index: usize) -> Option<bool> {
    match slots.get(index)? {
        ParamSlot::Bool(value) => Some(value),
        _ => None,
    }
}

/// The choice index in slot `index`, or `None` when the slot is missing or holds another type.
pub(crate) fn choice(slots: &ParamSlots, index: usize) -> Option<u16> {
    match slots.get(index)? {
        ParamSlot::Choice(value) => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use sonara_core::model::device::{ParamValue, Params};
    use sonara_core::plugins::builtin_registry;

    use super::{ParamSlotsError, choice, flag, float, param_slots};
    use crate::processor::{MAX_PARAMS, ParamSlot};

    fn values(pairs: &[(&str, ParamValue)]) -> Params {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect()
    }

    #[test]
    fn defaults_in_descriptor_order_with_choices_as_indices() {
        let registry = builtin_registry();
        let eq = registry.get("sonara.eq.parametric").unwrap();
        let slots = param_slots(eq, &Params::new()).unwrap();
        assert_eq!(slots.as_slice().len(), eq.params.len());
        for (slot, spec) in slots.as_slice().iter().zip(&eq.params) {
            let expected = match &spec.default {
                ParamValue::Float(v) => ParamSlot::Float(*v),
                ParamValue::Int(v) => ParamSlot::Int(*v),
                ParamValue::Bool(v) => ParamSlot::Bool(*v),
                ParamValue::Str(c) => ParamSlot::Choice(
                    u16::try_from(spec.choices.iter().position(|x| x == c).unwrap()).unwrap(),
                ),
            };
            assert_eq!(*slot, expected, "{}", spec.id);
        }
    }

    #[test]
    fn values_overlay_the_defaults_and_are_canonical() {
        let registry = builtin_registry();
        let eq = registry.get("sonara.eq.parametric").unwrap();
        let slots = param_slots(
            eq,
            &values(&[
                ("hp_slope", ParamValue::from("48")),
                // An integer for a float parameter becomes a float, as `coerce` makes it.
                ("hp_freq", ParamValue::Int(100)),
                ("not_a_param", ParamValue::Bool(true)),
                ("hp_on", ParamValue::Bool(true)),
            ]),
        )
        .unwrap();
        assert_eq!(flag(&slots, 0), Some(true));
        assert_eq!(float(&slots, 1), Some(100.0));
        assert_eq!(choice(&slots, 2), Some(3));
        assert_eq!(slots.as_slice().len(), eq.params.len());
        // Typed reads reject other slot types and missing slots.
        assert_eq!(float(&slots, 0), None);
        assert_eq!(flag(&slots, 2), None);
        assert_eq!(choice(&slots, 1), None);
        assert_eq!(float(&slots, eq.params.len()), None);
    }

    #[test]
    fn invalid_values_are_errors_with_their_cause() {
        let registry = builtin_registry();
        let gate = registry.get("sonara.dynamics.gate").unwrap();
        for bad in [
            ("mode", ParamValue::from("compressor")),
            ("threshold_db", ParamValue::from("loud")),
            ("threshold_db", ParamValue::Float(10.0)),
            ("sidechain", ParamValue::Int(2)),
        ] {
            let err = param_slots(gate, &values(std::slice::from_ref(&bad))).unwrap_err();
            assert!(
                matches!(
                    &err,
                    ParamSlotsError::InvalidValue { plugin, source }
                        if plugin == "sonara.dynamics.gate" && source.message.contains(bad.0)
                ),
                "{bad:?}: {err}"
            );
            assert!(err.source().is_some(), "{bad:?}");
        }
    }

    #[test]
    fn descriptors_fit_the_slot_capacity() {
        let registry = builtin_registry();
        for descriptor in registry.all(None) {
            assert!(descriptor.params.len() <= MAX_PARAMS, "{}", descriptor.id);
            assert!(param_slots(descriptor, &Params::new()).is_ok());
        }
        let mut wide = registry.get("sonara.utility").unwrap().clone();
        let first = wide.params.first().unwrap().clone();
        wide.params = core::iter::repeat_n(first, MAX_PARAMS + 1).collect();
        assert_eq!(
            param_slots(&wide, &Params::new()),
            Err(ParamSlotsError::TooManyParams {
                plugin: "sonara.utility".to_owned(),
                count: MAX_PARAMS + 1
            })
        );
    }
}
