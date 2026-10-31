//! Track, mixer, routing and send commands (`sonara.application.commands.track_cmds`).

use serde_json::json;

use super::{ChangeSet, Command, CommandContext, Flag, ObjectKind, Outcome};
use crate::error::{DomainError, ErrorCode, Result};
use crate::ids;
use crate::model::JsonMap;
use crate::model::device::{Device, DeviceData};
use crate::model::part::Part;
use crate::model::project::Project;
use crate::model::track::{InputSettings, MixerSettings, MonitorMode, Send, TrackData, TrackKind};
use crate::plugins::PluginKind;
use crate::pyfmt::str_repr;

/// What [`build_device`] needs besides the plugin.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeviceOptions<'a> {
    /// A preset name.
    pub preset: Option<&'a str>,
    /// Parameter values over the defaults and the preset; any value clears the preset name.
    pub params: Option<&'a JsonMap>,
    /// Display name; the plugin name when `None` or empty.
    pub name: Option<&'a str>,
    /// The track feeding the sidechain input.
    pub sidechain_id: Option<&'a str>,
}

/// A new device of `plugin_id`: defaults, then the preset, then explicit parameters.
///
/// # Errors
///
/// [`ErrorCode::NotFound`] for an unknown plugin or preset, [`ErrorCode::InvalidArgument`] for
/// a plugin of the wrong kind or an invalid parameter, [`ErrorCode::Unsupported`] for a
/// sidechain on a plugin without one.
pub fn build_device(
    ctx: &CommandContext<'_>,
    plugin_id: &str,
    device_id: &str,
    expected_kind: PluginKind,
    options: DeviceOptions<'_>,
) -> Result<Device> {
    let descriptor = ctx.plugins.get(plugin_id)?;
    if descriptor.kind != expected_kind {
        return Err(DomainError::invalid(format!(
            "{} is an {}, not an {}",
            descriptor.name,
            descriptor.kind.as_str(),
            expected_kind.as_str()
        )));
    }
    let mut values = descriptor.defaults();
    let mut preset_name = None;
    if let Some(preset) = options.preset.filter(|p| !p.is_empty()) {
        let preset = descriptor.preset(preset)?;
        values.extend(preset.params.iter().map(|(k, v)| (k.clone(), v.clone())));
        preset_name = Some(preset.name.clone());
    }
    if let Some(params) = options.params.filter(|p| !p.is_empty()) {
        values.extend(descriptor.validate_params(params)?);
        preset_name = None;
    }
    if options.sidechain_id.is_some() && !descriptor.sidechain {
        return Err(DomainError::new(
            ErrorCode::Unsupported,
            format!("{} has no sidechain input", descriptor.name),
        ));
    }
    Device::new(DeviceData {
        id: device_id.to_owned(),
        plugin_id: plugin_id.to_owned(),
        name: options
            .name
            .filter(|n| !n.is_empty())
            .unwrap_or(&descriptor.name)
            .to_owned(),
        params: values,
        enabled: true,
        sidechain_id: options.sidechain_id.map(str::to_owned),
        preset: preset_name,
        state: JsonMap::new(),
        extra: JsonMap::new(),
    })
}

/// Creates a track, optionally with an instrument and the part it plays.
#[derive(Debug, Clone)]
pub struct CreateTrack {
    /// Display name ("Track" when blank).
    pub name: String,
    /// Kind (not master).
    pub kind: TrackKind,
    /// Position in the track list (clamped), or the end.
    pub index: Option<i64>,
    /// `#rrggbb`, or `None` for automatic.
    pub color: Option<String>,
    /// The bus or master the track feeds.
    pub output_id: String,
    /// The instrument plugin of an instrument track.
    pub instrument_plugin: Option<String>,
    /// The instrument preset.
    pub instrument_preset: Option<String>,
    /// The folder track the new track belongs to.
    pub parent_id: Option<String>,
    /// The part an instrument track plays (project schema version 2); `None` states none.
    pub part: Option<Part>,
    /// The new track's ID.
    pub track_id: String,
    /// The new instrument's ID (used only with an instrument).
    pub instrument_id: String,
}

impl Command for CreateTrack {
    fn name(&self) -> &'static str {
        "CreateTrack"
    }

    fn label(&self) -> &'static str {
        "Create Track"
    }

    fn affects_routing(&self) -> bool {
        true
    }

    fn apply(&self, project: &Project, ctx: &CommandContext<'_>) -> Result<Outcome> {
        if self.kind == TrackKind::Master {
            return Err(DomainError::invalid(
                "a project has exactly one master track",
            ));
        }
        let instrument = match &self.instrument_plugin {
            None => None,
            Some(plugin) => {
                if self.kind != TrackKind::Instrument {
                    return Err(DomainError::invalid(
                        "only instrument tracks take an instrument",
                    ));
                }
                let options = DeviceOptions {
                    preset: self.instrument_preset.as_deref(),
                    ..DeviceOptions::default()
                };
                Some(build_device(
                    ctx,
                    plugin,
                    &self.instrument_id,
                    PluginKind::Instrument,
                    options,
                )?)
            }
        };
        let name = self.name.trim();
        let mut data = TrackData::new(
            self.track_id.clone(),
            if name.is_empty() { "Track" } else { name },
            self.kind,
        );
        data.instrument.clone_from(&instrument);
        data.output_id = Some(self.output_id.clone());
        data.color.clone_from(&self.color);
        data.parent_id.clone_from(&self.parent_id);
        data.part.clone_from(&self.part);
        let track = crate::model::track::Track::new(data)?;
        let p = project.adding_track(track, self.index)?;
        let mut created = vec![(ObjectKind::Track, self.track_id.as_str())];
        let mut result = json!({"track_id": self.track_id});
        if let Some(device) = &instrument {
            created.push((ObjectKind::Device, device.id.as_str()));
            result["instrument_device_id"] = device.id.as_str().into();
        }
        Ok(Outcome::with_result(
            p,
            ChangeSet::of(created, [], [], [Flag::Tracks, Flag::Routing]),
            result,
        ))
    }
}

/// Renames a track (surrounding whitespace removed).
#[derive(Debug, Clone)]
pub struct RenameTrack {
    /// The track.
    pub track_id: String,
    /// The new name.
    pub name: String,
}

impl Command for RenameTrack {
    fn name(&self) -> &'static str {
        "RenameTrack"
    }

    fn label(&self) -> &'static str {
        "Rename Track"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let t = project.track(&self.track_id)?;
        let p = project.with_track(t.edit(|d| self.name.trim().clone_into(&mut d.name))?)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of([], [(ObjectKind::Track, t.id.as_str())], [], [Flag::Tracks]),
        ))
    }
}

/// Sets or clears a track's color.
#[derive(Debug, Clone)]
pub struct SetTrackColor {
    /// The track.
    pub track_id: String,
    /// `#rrggbb`, or `None` for automatic.
    pub color: Option<String>,
}

impl Command for SetTrackColor {
    fn name(&self) -> &'static str {
        "SetTrackColor"
    }

    fn label(&self) -> &'static str {
        "Track Color"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let t = project.track(&self.track_id)?;
        let p = project.with_track(t.edit(|d| d.color.clone_from(&self.color))?)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of([], [(ObjectKind::Track, t.id.as_str())], [], [Flag::Tracks]),
        ))
    }
}

/// Deletes a track and repairs every reference to it: outputs to a deleted bus go to the
/// master, sends to it are removed, sidechains from it are cleared and folder children move up.
#[derive(Debug, Clone)]
pub struct DeleteTrack {
    /// The track.
    pub track_id: String,
}

impl Command for DeleteTrack {
    fn name(&self) -> &'static str {
        "DeleteTrack"
    }

    fn label(&self) -> &'static str {
        "Delete Track"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let victim = project.track(&self.track_id)?;
        let victim_id = victim.id.as_str();
        let mut p = project.removing_track(&self.track_id)?;
        let mut updated: Vec<String> = Vec::new();
        let mut deleted: Vec<(ObjectKind, String)> = vec![(ObjectKind::Track, victim.id.clone())];
        deleted.extend(
            victim
                .clips
                .iter()
                .map(|c| (ObjectKind::Clip, c.id().to_owned())),
        );
        deleted.extend(victim.devices().map(|d| (ObjectKind::Device, d.id.clone())));
        let tracks: Vec<_> = p.all_tracks().cloned().collect();
        for t in tracks {
            let mut data = t.to_data();
            let mut changed = false;
            if data.output_id.as_deref() == Some(victim_id) {
                data.output_id = Some(ids::MASTER_ID.to_owned());
                changed = true;
            }
            let removed: Vec<String> = data
                .sends
                .iter()
                .filter(|s| s.target_id == victim_id)
                .map(|s| s.id.clone())
                .collect();
            if !removed.is_empty() {
                deleted.extend(removed.iter().map(|id| (ObjectKind::Send, id.clone())));
                data.sends.retain(|s| s.target_id != victim_id);
                data.automation.retain(|a| {
                    a.target
                        .send_id()
                        .is_none_or(|id| !removed.iter().any(|r| r == id))
                });
                changed = true;
            }
            if data.parent_id.as_deref() == Some(victim_id) {
                data.parent_id.clone_from(&victim.parent_id);
                changed = true;
            }
            if let Some(instrument) = &data.instrument
                && instrument.sidechain_id.as_deref() == Some(victim_id)
            {
                data.instrument = Some(instrument.edit(|d| d.sidechain_id = None)?);
                changed = true;
            }
            if data
                .inserts
                .iter()
                .any(|d| d.sidechain_id.as_deref() == Some(victim_id))
            {
                for slot in &mut data.inserts {
                    if slot.sidechain_id.as_deref() == Some(victim_id) {
                        *slot = slot.edit(|d| d.sidechain_id = None)?;
                    }
                }
                changed = true;
            }
            if changed {
                p = p.with_track(crate::model::track::Track::new(data)?)?;
                updated.push(t.id.clone());
            }
        }
        let changes = ChangeSet::of(
            [],
            updated.iter().map(|id| (ObjectKind::Track, id.as_str())),
            deleted.iter().map(|(kind, id)| (*kind, id.as_str())),
            [Flag::Tracks, Flag::Routing, Flag::Clips],
        );
        Ok(Outcome::with_result(
            p,
            changes,
            json!({"deleted_clips": victim.clips.len(), "rerouted_tracks": updated}),
        ))
    }
}

/// Moves a track to `index` (clamped).
#[derive(Debug, Clone)]
pub struct MoveTrack {
    /// The track.
    pub track_id: String,
    /// The new position.
    pub index: i64,
}

impl Command for MoveTrack {
    fn name(&self) -> &'static str {
        "MoveTrack"
    }

    fn label(&self) -> &'static str {
        "Move Track"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let t = project.track(&self.track_id)?;
        let p = project
            .removing_track(&t.id)?
            .adding_track(t.clone(), Some(self.index))?;
        let index = p.track_index(&t.id)?;
        Ok(Outcome::with_result(
            p,
            ChangeSet::of([], [(ObjectKind::Track, t.id.as_str())], [], [Flag::Tracks]),
            json!({"index": index}),
        ))
    }
}

/// Changes mixer settings; `None` keeps a value.
#[derive(Debug, Clone, Default)]
pub struct SetTrackMixer {
    /// The track.
    pub track_id: String,
    /// Fader level in dB.
    pub volume_db: Option<f64>,
    /// Pan, −1..+1.
    pub pan: Option<f64>,
    /// Mute.
    pub mute: Option<bool>,
    /// Solo.
    pub solo: Option<bool>,
    /// Solo-safe.
    pub solo_safe: Option<bool>,
}

impl Command for SetTrackMixer {
    fn name(&self) -> &'static str {
        "SetTrackMixer"
    }

    fn label(&self) -> &'static str {
        "Mixer"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let t = project.track(&self.track_id)?;
        let m = t.mixer;
        let mixer = MixerSettings::new(
            self.volume_db.unwrap_or(m.volume_db),
            self.pan.unwrap_or(m.pan),
            self.mute.unwrap_or(m.mute),
            self.solo.unwrap_or(m.solo),
            self.solo_safe.unwrap_or(m.solo_safe),
        )?;
        if t.kind == TrackKind::Master && mixer.solo {
            return Err(DomainError::invalid("the master track cannot be soloed"));
        }
        let p = project.with_track(t.edit(|d| d.mixer = mixer)?)?;
        Ok(Outcome::with_result(
            p,
            ChangeSet::of([], [(ObjectKind::Track, t.id.as_str())], [], [Flag::Mixer]),
            json!({
                "volume_db": mixer.volume_db,
                "pan": mixer.pan,
                "mute": mixer.mute,
                "solo": mixer.solo,
                "solo_safe": mixer.solo_safe,
            }),
        ))
    }

    fn describe(&self) -> String {
        if self.volume_db.is_some() {
            "Track Volume"
        } else if self.pan.is_some() {
            "Track Pan"
        } else if self.mute.is_some() {
            "Mute"
        } else if self.solo.is_some() {
            "Solo"
        } else {
            self.label()
        }
        .to_owned()
    }

    fn merge_key(&self) -> Option<String> {
        let changed: Vec<&str> = [("volume_db", self.volume_db), ("pan", self.pan)]
            .into_iter()
            .filter_map(|(name, value)| value.map(|_| name))
            .collect();
        let toggles = self.mute.is_some() || self.solo.is_some() || self.solo_safe.is_some();
        (!changed.is_empty() && !toggles)
            .then(|| format!("mixer:{}:{}", self.track_id, changed.join(",")))
    }
}

/// Routes a track's output to a bus or the master.
#[derive(Debug, Clone)]
pub struct SetTrackOutput {
    /// The track.
    pub track_id: String,
    /// The bus or master.
    pub output_id: String,
}

impl Command for SetTrackOutput {
    fn name(&self) -> &'static str {
        "SetTrackOutput"
    }

    fn label(&self) -> &'static str {
        "Route Output"
    }

    fn affects_routing(&self) -> bool {
        true
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let t = project.track(&self.track_id)?;
        if t.kind == TrackKind::Master {
            return Err(DomainError::new(
                ErrorCode::InvalidRouting,
                "the master output is the audio device",
            ));
        }
        project.track(&self.output_id)?;
        let p = project.with_track(t.edit(|d| d.output_id = Some(self.output_id.clone()))?)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [(ObjectKind::Track, t.id.as_str())],
                [],
                [Flag::Routing],
            ),
        ))
    }
}

/// Adds a send from a track to a bus.
#[derive(Debug, Clone)]
pub struct AddSend {
    /// The sending track.
    pub track_id: String,
    /// The receiving bus.
    pub target_id: String,
    /// Level in dB.
    pub level_db: f64,
    /// Tap before the fader.
    pub pre_fader: bool,
    /// The new send's ID.
    pub send_id: String,
}

impl Command for AddSend {
    fn name(&self) -> &'static str {
        "AddSend"
    }

    fn label(&self) -> &'static str {
        "Add Send"
    }

    fn affects_routing(&self) -> bool {
        true
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let t = project.track(&self.track_id)?;
        let target = project.track(&self.target_id)?;
        if target.kind != TrackKind::Bus {
            return Err(DomainError::new(
                ErrorCode::InvalidRouting,
                "sends must target a bus track",
            ));
        }
        if t.sends.iter().any(|s| s.target_id == self.target_id) {
            return Err(DomainError::new(
                ErrorCode::Conflict,
                format!(
                    "{} already sends to {}",
                    str_repr(&t.name),
                    str_repr(&target.name)
                ),
            ));
        }
        let send = Send::new(
            self.send_id.clone(),
            self.target_id.clone(),
            self.level_db,
            self.pre_fader,
            true,
        )?;
        let p = project.with_track(t.edit(|d| d.sends.push(send))?)?;
        Ok(Outcome::with_result(
            p,
            ChangeSet::of(
                [(ObjectKind::Send, self.send_id.as_str())],
                [(ObjectKind::Track, t.id.as_str())],
                [],
                [Flag::Routing],
            ),
            json!({"send_id": self.send_id}),
        ))
    }
}

/// Changes a send; `None` keeps a value.
#[derive(Debug, Clone)]
pub struct UpdateSend {
    /// The sending track.
    pub track_id: String,
    /// The send.
    pub send_id: String,
    /// Level in dB.
    pub level_db: Option<f64>,
    /// Tap before the fader.
    pub pre_fader: Option<bool>,
    /// Active.
    pub enabled: Option<bool>,
}

impl Command for UpdateSend {
    fn name(&self) -> &'static str {
        "UpdateSend"
    }

    fn label(&self) -> &'static str {
        "Send Level"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let t = project.track(&self.track_id)?;
        let s = t.send(&self.send_id)?;
        let new = Send::new(
            s.id.clone(),
            s.target_id.clone(),
            self.level_db.unwrap_or(s.level_db),
            self.pre_fader.unwrap_or(s.pre_fader),
            self.enabled.unwrap_or(s.enabled),
        )?;
        let p = project.with_track(t.edit(|d| {
            for slot in &mut d.sends {
                if slot.id == new.id {
                    *slot = new.clone();
                }
            }
        })?)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [
                    (ObjectKind::Send, s.id.as_str()),
                    (ObjectKind::Track, t.id.as_str()),
                ],
                [],
                [Flag::Mixer],
            ),
        ))
    }

    fn merge_key(&self) -> Option<String> {
        (self.level_db.is_some() && self.pre_fader.is_none() && self.enabled.is_none())
            .then(|| format!("send:{}", self.send_id))
    }
}

/// Removes a send and its automation.
#[derive(Debug, Clone)]
pub struct RemoveSend {
    /// The sending track.
    pub track_id: String,
    /// The send.
    pub send_id: String,
}

impl Command for RemoveSend {
    fn name(&self) -> &'static str {
        "RemoveSend"
    }

    fn label(&self) -> &'static str {
        "Remove Send"
    }

    fn affects_routing(&self) -> bool {
        true
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let t = project.track(&self.track_id)?;
        t.send(&self.send_id)?;
        let p = project.with_track(t.edit(|d| {
            d.sends.retain(|s| s.id != self.send_id);
            d.automation
                .retain(|a| a.target.send_id() != Some(self.send_id.as_str()));
        })?)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [(ObjectKind::Track, t.id.as_str())],
                [(ObjectKind::Send, self.send_id.as_str())],
                [Flag::Routing],
            ),
        ))
    }
}

/// Moves a track into a folder (or, with `None`, to the top level).
#[derive(Debug, Clone)]
pub struct SetTrackParent {
    /// The track.
    pub track_id: String,
    /// The folder track.
    pub parent_id: Option<String>,
}

impl Command for SetTrackParent {
    fn name(&self) -> &'static str {
        "SetTrackParent"
    }

    fn label(&self) -> &'static str {
        "Move Into Folder"
    }

    fn affects_routing(&self) -> bool {
        true
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let t = project.track(&self.track_id)?;
        let p = project.with_track(t.edit(|d| d.parent_id.clone_from(&self.parent_id))?)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of([], [(ObjectKind::Track, t.id.as_str())], [], [Flag::Tracks]),
        ))
    }
}

/// Changes a track's recording input; `None` keeps a value.
#[derive(Debug, Clone, Default)]
pub struct SetTrackInput {
    /// The track.
    pub track_id: String,
    /// Armed for recording.
    pub armed: Option<bool>,
    /// Input monitoring.
    pub monitor: Option<MonitorMode>,
    /// One or two device input channels.
    pub channels: Option<Vec<i64>>,
}

impl Command for SetTrackInput {
    fn name(&self) -> &'static str {
        "SetTrackInput"
    }

    fn label(&self) -> &'static str {
        "Track Input"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let t = project.track(&self.track_id)?;
        if !matches!(t.kind, TrackKind::Audio | TrackKind::Instrument) {
            return Err(DomainError::invalid(
                "only audio and instrument tracks can record",
            ));
        }
        let i = &t.input;
        let channels: Vec<i64> = self
            .channels
            .clone()
            .unwrap_or_else(|| i.channels.iter().map(|c| i64::from(*c)).collect());
        let input = InputSettings::new(
            &channels,
            self.armed.unwrap_or(i.armed),
            self.monitor.unwrap_or(i.monitor),
        )?;
        let p = project.with_track(t.edit(|d| d.input = input)?)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of([], [(ObjectKind::Track, t.id.as_str())], [], [Flag::Tracks]),
        ))
    }
}
