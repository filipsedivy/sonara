//! Effect tools (the prototype's "devices" section): inserting and removing effects, and
//! setting the parameters, preset and bypass of an effect or instrument.

use indexmap::IndexMap;
use serde_json::Value;
use sonara_core::commands::Command;
use sonara_core::commands::device::{
    AddDevice, ApplyPreset, RemoveDevice, SetDeviceEnabled, SetDeviceParams, params_json,
};
use sonara_core::error::DomainError;
use sonara_core::ids::{self, IdGenerator};
use sonara_core::model::JsonMap;

use super::tracks::{commit_steps, list_index};
use crate::context::AgentContext;
use crate::contract::args::{AddEffect, ParamValue, RemoveEffect, SetEffectParameter};
use crate::registry::ToolResult;

/// A parameter value as the commands take it: the JSON value of the kind the argument union
/// chose (`dict(a.params)`).
fn param_value(value: &ParamValue) -> Value {
    match value {
        ParamValue::Bool(value) => Value::Bool(*value),
        ParamValue::Int(value) => Value::from(*value),
        ParamValue::Float(value) => Value::from(*value),
        ParamValue::Str(value) => Value::from(value.as_str()),
    }
}

/// Parameter values by ID, in argument order.
pub(super) fn param_values(params: &IndexMap<String, ParamValue>) -> JsonMap {
    params
        .iter()
        .map(|(id, value)| (id.clone(), param_value(value)))
        .collect()
}

/// `add_effect`: inserts an effect on a track, bus or the master, from defaults, a preset and
/// parameter values, optionally keyed by a sidechain source.
pub(super) fn add_effect(ctx: &AgentContext, args: AddEffect) -> ToolResult {
    let AddEffect {
        dry_run,
        track_id,
        plugin_id,
        preset,
        params,
        index,
        sidechain_source,
    } = args;
    let device_id = ctx.app().ids().new_id(ids::DEVICE);
    let command = AddDevice {
        track_id,
        plugin_id,
        index: list_index(index),
        params: param_values(&params),
        preset,
        // `a.sidechain_source or None`: an empty source means none.
        sidechain_id: sidechain_source.filter(|source| !source.is_empty()),
        name: None,
        device_id,
    };
    Ok(ctx.commit(&command, dry_run)?)
}

/// `set_effect_parameter`: loads a preset, sets parameters and bypasses, as one undo step
/// (`Set Parameter`). The result reports the device's resolved parameters afterwards
/// (`params`; unchanged by a dry run).
pub(super) fn set_effect_parameter(ctx: &AgentContext, args: SetEffectParameter) -> ToolResult {
    let SetEffectParameter {
        dry_run,
        device_id,
        param,
        value,
        params,
        preset,
        enabled,
    } = args;
    let mut commands: Vec<Box<dyn Command>> = Vec::new();
    if let Some(preset) = preset {
        commands.push(Box::new(ApplyPreset {
            device_id: device_id.clone(),
            preset,
        }));
    }
    let mut values = param_values(&params);
    if let Some(param) = param {
        let Some(value) = value else {
            return Err(DomainError::invalid("value is required with param").into());
        };
        values.insert(param, param_value(&value));
    }
    // A sampler instrument given by its patch path, as before preset IDs, stores its ID.
    ctx.app().normalize_device_params(&device_id, &mut values);
    if !values.is_empty() {
        commands.push(Box::new(SetDeviceParams {
            device_id: device_id.clone(),
            values,
        }));
    }
    if let Some(enabled) = enabled {
        commands.push(Box::new(SetDeviceEnabled {
            device_id: device_id.clone(),
            enabled,
        }));
    }
    let mut out = commit_steps(ctx, commands, "Set Parameter", dry_run)?;
    let resolved = {
        let session = ctx.app().session().lock();
        let (_, device) = session.require_project()?.device(&device_id)?;
        let descriptor = ctx.app().plugins().get(&device.plugin_id)?;
        params_json(&descriptor.resolve(&device.params))
    };
    out["params"] = resolved;
    Ok(out)
}

/// `remove_effect`: removes an effect from its chain.
pub(super) fn remove_effect(ctx: &AgentContext, args: RemoveEffect) -> ToolResult {
    let command = RemoveDevice {
        device_id: args.device_id,
    };
    Ok(ctx.commit(&command, args.dry_run)?)
}
