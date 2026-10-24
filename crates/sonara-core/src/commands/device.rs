//! Instrument and effect commands (`sonara.application.commands.device_cmds`).

use serde_json::{Value, json};

use super::track::{DeviceOptions, build_device};
use super::{ChangeSet, Command, CommandContext, Flag, ObjectKind, Outcome};
use crate::error::{DomainError, ErrorCode, Result};
use crate::model::JsonMap;
use crate::model::device::Params;
use crate::model::project::Project;
use crate::model::track::{TrackKind, clamp_index};
use crate::plugins::PluginKind;

/// Most insert effects on one track.
pub const MAX_INSERTS: usize = 64;

/// Parameter values as a JSON object, in order.
#[must_use]
pub fn params_json(params: &Params) -> Value {
    Value::Object(
        params
            .iter()
            .map(|(k, v)| (k.clone(), Value::from(v)))
            .collect(),
    )
}

/// Inserts an effect into a track's chain (`index` `None`: the end).
#[derive(Debug, Clone)]
pub struct AddDevice {
    /// The track.
    pub track_id: String,
    /// The effect plugin.
    pub plugin_id: String,
    /// Position in the insert chain (clamped), or the end.
    pub index: Option<i64>,
    /// Parameter values over the defaults and the preset.
    pub params: JsonMap,
    /// A preset name.
    pub preset: Option<String>,
    /// The track feeding the sidechain input.
    pub sidechain_id: Option<String>,
    /// Display name; the plugin name by default.
    pub name: Option<String>,
    /// The new device's ID.
    pub device_id: String,
}

impl Command for AddDevice {
    fn name(&self) -> &'static str {
        "AddDevice"
    }

    fn label(&self) -> &'static str {
        "Add Effect"
    }

    fn affects_routing(&self) -> bool {
        true
    }

    fn apply(&self, project: &Project, ctx: &CommandContext<'_>) -> Result<Outcome> {
        let track = project.track(&self.track_id)?;
        if track.kind == TrackKind::Folder {
            return Err(DomainError::invalid("folder tracks cannot hold effects"));
        }
        if track.inserts.len() >= MAX_INSERTS {
            return Err(DomainError::new(
                ErrorCode::LimitExceeded,
                format!("a track can hold at most {MAX_INSERTS} effects"),
            ));
        }
        if let Some(source) = &self.sidechain_id {
            project.track(source)?;
        }
        let device = build_device(
            ctx,
            &self.plugin_id,
            &self.device_id,
            PluginKind::Effect,
            DeviceOptions {
                preset: self.preset.as_deref(),
                params: Some(&self.params),
                name: self.name.as_deref(),
                sidechain_id: self.sidechain_id.as_deref(),
            },
        )?;
        let new_track = track.with_insert(device.clone(), self.index)?;
        let index = new_track
            .inserts
            .iter()
            .position(|d| d.id == device.id)
            .unwrap_or_else(|| clamp_index(self.index, track.inserts.len()));
        let p = project.with_track(new_track)?;
        Ok(Outcome::with_result(
            p,
            ChangeSet::of(
                [(ObjectKind::Device, device.id.as_str())],
                [(ObjectKind::Track, track.id.as_str())],
                [],
                [Flag::Devices],
            ),
            json!({"device_id": device.id, "index": index, "params": params_json(&device.params)}),
        ))
    }
}

/// Sets (or replaces) the instrument of an instrument track.
#[derive(Debug, Clone)]
pub struct SetInstrument {
    /// The instrument track.
    pub track_id: String,
    /// The instrument plugin.
    pub plugin_id: String,
    /// A preset name.
    pub preset: Option<String>,
    /// Parameter values over the defaults and the preset.
    pub params: JsonMap,
    /// The new device's ID.
    pub device_id: String,
}

impl Command for SetInstrument {
    fn name(&self) -> &'static str {
        "SetInstrument"
    }

    fn label(&self) -> &'static str {
        "Set Instrument"
    }

    fn apply(&self, project: &Project, ctx: &CommandContext<'_>) -> Result<Outcome> {
        let track = project.track(&self.track_id)?;
        if track.kind != TrackKind::Instrument {
            return Err(DomainError::invalid(
                "only instrument tracks hold an instrument",
            ));
        }
        let device = build_device(
            ctx,
            &self.plugin_id,
            &self.device_id,
            PluginKind::Instrument,
            DeviceOptions {
                preset: self.preset.as_deref(),
                params: Some(&self.params),
                ..DeviceOptions::default()
            },
        )?;
        let old_id = track.instrument.as_ref().map(|d| d.id.clone());
        let p = project.with_track(track.edit(|d| {
            if let Some(old) = &old_id {
                d.automation
                    .retain(|a| a.target.device_id.as_deref() != Some(old.as_str()));
            }
            d.instrument = Some(device.clone());
        })?)?;
        Ok(Outcome::with_result(
            p,
            ChangeSet::of(
                [(ObjectKind::Device, device.id.as_str())],
                [(ObjectKind::Track, track.id.as_str())],
                old_id.iter().map(|id| (ObjectKind::Device, id.as_str())),
                [Flag::Devices],
            ),
            json!({"device_id": device.id, "params": params_json(&device.params)}),
        ))
    }
}

/// Removes an instrument or effect and its automation.
#[derive(Debug, Clone)]
pub struct RemoveDevice {
    /// The device.
    pub device_id: String,
}

impl Command for RemoveDevice {
    fn name(&self) -> &'static str {
        "RemoveDevice"
    }

    fn label(&self) -> &'static str {
        "Remove Effect"
    }

    fn affects_routing(&self) -> bool {
        true
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, _) = project.device(&self.device_id)?;
        let p = project.with_track(track.without_device(&self.device_id)?)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [(ObjectKind::Track, track.id.as_str())],
                [(ObjectKind::Device, self.device_id.as_str())],
                [Flag::Devices],
            ),
        ))
    }
}

/// Moves an effect within its track's insert chain.
#[derive(Debug, Clone)]
pub struct MoveDevice {
    /// The effect.
    pub device_id: String,
    /// The new position (clamped).
    pub index: i64,
}

impl Command for MoveDevice {
    fn name(&self) -> &'static str {
        "MoveDevice"
    }

    fn label(&self) -> &'static str {
        "Reorder Effects"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, device) = project.device(&self.device_id)?;
        if track.instrument.as_ref().is_some_and(|i| i.id == device.id) {
            return Err(DomainError::invalid(
                "the instrument is always first in the chain",
            ));
        }
        let p = project.with_track(track.edit(|d| {
            d.inserts.retain(|i| i.id != device.id);
            let at = clamp_index(Some(self.index), d.inserts.len());
            d.inserts.insert(at, device.clone());
        })?)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [
                    (ObjectKind::Device, device.id.as_str()),
                    (ObjectKind::Track, track.id.as_str()),
                ],
                [],
                [Flag::Devices],
            ),
        ))
    }
}

/// Sets device parameters (validated against the plugin).
#[derive(Debug, Clone)]
pub struct SetDeviceParams {
    /// The device.
    pub device_id: String,
    /// New values by parameter ID.
    pub values: JsonMap,
}

impl Command for SetDeviceParams {
    fn name(&self) -> &'static str {
        "SetDeviceParams"
    }

    fn label(&self) -> &'static str {
        "Set Parameter"
    }

    fn apply(&self, project: &Project, ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, device) = project.device(&self.device_id)?;
        let descriptor = ctx.plugins.get(&device.plugin_id)?;
        let values = descriptor.validate_params(&self.values)?;
        let new = device.with_params(values.clone())?;
        let p = project.with_track(track.with_device(new)?)?;
        Ok(Outcome::with_result(
            p,
            ChangeSet::of(
                [],
                [(ObjectKind::Device, device.id.as_str())],
                [],
                [Flag::Devices],
            ),
            json!({"params": params_json(&values)}),
        ))
    }

    fn merge_key(&self) -> Option<String> {
        let mut keys: Vec<&str> = self.values.keys().map(String::as_str).collect();
        keys.sort_unstable();
        Some(format!("params:{}:{}", self.device_id, keys.join(",")))
    }
}

/// Enables or bypasses a device.
#[derive(Debug, Clone)]
pub struct SetDeviceEnabled {
    /// The device.
    pub device_id: String,
    /// Whether it processes.
    pub enabled: bool,
}

impl Command for SetDeviceEnabled {
    fn name(&self) -> &'static str {
        "SetDeviceEnabled"
    }

    fn label(&self) -> &'static str {
        "Bypass Effect"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, device) = project.device(&self.device_id)?;
        let p =
            project.with_track(track.with_device(device.edit(|d| d.enabled = self.enabled)?)?)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [(ObjectKind::Device, device.id.as_str())],
                [],
                [Flag::Devices],
            ),
        ))
    }
}

/// Loads a preset: defaults overlaid with the preset's values.
#[derive(Debug, Clone)]
pub struct ApplyPreset {
    /// The device.
    pub device_id: String,
    /// The preset name (case-insensitive).
    pub preset: String,
}

impl Command for ApplyPreset {
    fn name(&self) -> &'static str {
        "ApplyPreset"
    }

    fn label(&self) -> &'static str {
        "Load Preset"
    }

    fn apply(&self, project: &Project, ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, device) = project.device(&self.device_id)?;
        let descriptor = ctx.plugins.get(&device.plugin_id)?;
        let preset = descriptor.preset(&self.preset)?;
        let mut values = descriptor.defaults();
        values.extend(preset.params.iter().map(|(k, v)| (k.clone(), v.clone())));
        let new = device.edit(|d| {
            d.params.clone_from(&values);
            d.preset = Some(preset.name.clone());
        })?;
        let p = project.with_track(track.with_device(new)?)?;
        Ok(Outcome::with_result(
            p,
            ChangeSet::of(
                [],
                [(ObjectKind::Device, device.id.as_str())],
                [],
                [Flag::Devices],
            ),
            json!({"params": params_json(&values)}),
        ))
    }
}

/// Connects (or, with `None`, disconnects) a device's sidechain input.
#[derive(Debug, Clone)]
pub struct SetSidechain {
    /// The device.
    pub device_id: String,
    /// The track feeding the sidechain.
    pub source_track_id: Option<String>,
}

impl Command for SetSidechain {
    fn name(&self) -> &'static str {
        "SetSidechain"
    }

    fn label(&self) -> &'static str {
        "Set Sidechain"
    }

    fn affects_routing(&self) -> bool {
        true
    }

    fn apply(&self, project: &Project, ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, device) = project.device(&self.device_id)?;
        if let Some(source) = &self.source_track_id {
            if !ctx.plugins.get(&device.plugin_id)?.sidechain {
                return Err(DomainError::new(
                    ErrorCode::Unsupported,
                    format!("{} has no sidechain input", device.name),
                ));
            }
            project.track(source)?;
        }
        let new = device.edit(|d| d.sidechain_id.clone_from(&self.source_track_id))?;
        let p = project.with_track(track.with_device(new)?)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [(ObjectKind::Device, device.id.as_str())],
                [],
                [Flag::Devices, Flag::Routing],
            ),
        ))
    }
}

/// Sets non-parameter device state (for example a convolver's impulse response); `null`
/// removes a key.
#[derive(Debug, Clone)]
pub struct SetDeviceState {
    /// The device.
    pub device_id: String,
    /// New values by key.
    pub values: JsonMap,
}

impl Command for SetDeviceState {
    fn name(&self) -> &'static str {
        "SetDeviceState"
    }

    fn label(&self) -> &'static str {
        "Device Settings"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, device) = project.device(&self.device_id)?;
        let new = device.edit(|d| {
            for (key, value) in &self.values {
                if value.is_null() {
                    d.state.shift_remove(key);
                } else {
                    d.state.insert(key.clone(), value.clone());
                }
            }
        })?;
        let p = project.with_track(track.with_device(new)?)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [(ObjectKind::Device, device.id.as_str())],
                [],
                [Flag::Devices],
            ),
        ))
    }
}
