//! Project-level commands: tempo, meter, length, metadata, transport, markers, sections and
//! media assets (`sonara.application.commands.project_cmds`).

use indexmap::IndexMap;
use serde_json::{Value, json};

use super::{ChangeSet, Command, CommandContext, Flag, ObjectKind, Outcome};
use crate::error::{DomainError, ErrorCode, Result};
use crate::model::JsonMap;
use crate::model::markers::{Marker, Section};
use crate::model::media::MediaAsset;
use crate::model::project::{Project, ProjectMetadata, ProjectSettings, TransportSettings};
use crate::pyfmt::{float_repr, str_list_repr, str_repr};
use crate::time::{TempoMap, TempoPoint};

const PROJECT: (ObjectKind, &str) = (ObjectKind::Project, "project");

/// A change of the project itself with `flag`.
fn project_change(flag: Flag) -> ChangeSet {
    ChangeSet::of([], [PROJECT], [], [flag])
}

/// Python's `int` from a validated `u32`-range request, mapping out-of-range values to 0 so
/// that validation reports them with the prototype's message.
fn to_u32(value: i64) -> u32 {
    u32::try_from(value).unwrap_or(0)
}

/// Renames the project (surrounding whitespace removed).
#[derive(Debug, Clone)]
pub struct RenameProject {
    /// The new name.
    pub name: String,
}

impl Command for RenameProject {
    fn name(&self) -> &'static str {
        "RenameProject"
    }

    fn label(&self) -> &'static str {
        "Rename Project"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let p = project.edit(|d| self.name.trim().clone_into(&mut d.name))?;
        Ok(Outcome::new(p, project_change(Flag::Metadata)))
    }
}

/// Sets the tempo at `beat` (0 = the song tempo). `ramp` glides linearly to the next tempo
/// point; `clear_changes` replaces the whole map with one constant tempo.
#[derive(Debug, Clone)]
pub struct SetTempo {
    /// Tempo in BPM.
    pub bpm: f64,
    /// Position in beats.
    pub beat: f64,
    /// Glide to the next point.
    pub ramp: bool,
    /// Replace every tempo change.
    pub clear_changes: bool,
}

impl Command for SetTempo {
    fn name(&self) -> &'static str {
        "SetTempo"
    }

    fn label(&self) -> &'static str {
        "Set Tempo"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let tempo = if self.clear_changes {
            TempoMap::new(vec![TempoPoint::new(0.0, self.bpm, false)?])?
        } else {
            project
                .tempo_map
                .with_point(self.beat, self.bpm, self.ramp)?
        };
        let points = tempo.points().len();
        let p = project.edit(|d| d.tempo_map = tempo)?;
        Ok(Outcome::with_result(
            p,
            project_change(Flag::Tempo),
            json!({"tempo_points": points}),
        ))
    }

    fn merge_key(&self) -> Option<String> {
        Some(format!("tempo:{}", float_repr(self.beat)))
    }
}

/// Removes the tempo change at `beat`.
#[derive(Debug, Clone)]
pub struct RemoveTempoPoint {
    /// Position in beats.
    pub beat: f64,
}

impl Command for RemoveTempoPoint {
    fn name(&self) -> &'static str {
        "RemoveTempoPoint"
    }

    fn label(&self) -> &'static str {
        "Remove Tempo Change"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let tempo = project.tempo_map.without_point(self.beat)?;
        let p = project.edit(|d| d.tempo_map = tempo)?;
        Ok(Outcome::new(p, project_change(Flag::Tempo)))
    }
}

/// Sets the time signature from 0-based `bar` on.
#[derive(Debug, Clone)]
pub struct SetTimeSignature {
    /// Beats per bar.
    pub numerator: i64,
    /// Beat note value.
    pub denominator: i64,
    /// 0-based bar.
    pub bar: i64,
}

impl Command for SetTimeSignature {
    fn name(&self) -> &'static str {
        "SetTimeSignature"
    }

    fn label(&self) -> &'static str {
        "Set Time Signature"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let signatures = project.time_signatures.with_point(
            self.bar,
            to_u32(self.numerator),
            to_u32(self.denominator),
        )?;
        let p = project.edit(|d| d.time_signatures = signatures)?;
        Ok(Outcome::new(p, project_change(Flag::Tempo)))
    }
}

/// Removes the time signature change at 0-based `bar`.
#[derive(Debug, Clone)]
pub struct RemoveTimeSignature {
    /// 0-based bar.
    pub bar: i64,
}

impl Command for RemoveTimeSignature {
    fn name(&self) -> &'static str {
        "RemoveTimeSignature"
    }

    fn label(&self) -> &'static str {
        "Remove Time Signature Change"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let signatures = project.time_signatures.without_point(self.bar)?;
        let p = project.edit(|d| d.time_signatures = signatures)?;
        Ok(Outcome::new(p, project_change(Flag::Tempo)))
    }
}

/// Sets (or, with `None`, clears) the explicit song length.
#[derive(Debug, Clone)]
pub struct SetProjectLength {
    /// Song end in beats.
    pub length_beats: Option<f64>,
}

impl Command for SetProjectLength {
    fn name(&self) -> &'static str {
        "SetProjectLength"
    }

    fn label(&self) -> &'static str {
        "Set Project Length"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let s = &project.settings;
        let settings = ProjectSettings::new(
            i64::from(s.sample_rate),
            self.length_beats,
            s.record_subtype.clone(),
        )?;
        let p = project.edit(|d| d.settings = settings)?;
        Ok(Outcome::new(p, project_change(Flag::Settings)))
    }
}

/// Changes the project sample rate.
#[derive(Debug, Clone)]
pub struct SetSampleRate {
    /// One of the supported sample rates.
    pub sample_rate: i64,
}

impl Command for SetSampleRate {
    fn name(&self) -> &'static str {
        "SetSampleRate"
    }

    fn label(&self) -> &'static str {
        "Set Sample Rate"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let s = &project.settings;
        let settings =
            ProjectSettings::new(self.sample_rate, s.length_beats, s.record_subtype.clone())?;
        let p = project.edit(|d| d.settings = settings)?;
        Ok(Outcome::new(p, project_change(Flag::Settings)))
    }
}

/// Sets song metadata fields (title, artist, …).
#[derive(Debug, Clone, Default)]
pub struct SetMetadata {
    /// New values by field name.
    pub values: IndexMap<String, String>,
}

impl Command for SetMetadata {
    fn name(&self) -> &'static str {
        "SetMetadata"
    }

    fn label(&self) -> &'static str {
        "Edit Project Info"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let mut unknown: Vec<&str> = self
            .values
            .keys()
            .map(String::as_str)
            .filter(|k| !ProjectMetadata::FIELDS.contains(k))
            .collect();
        if !unknown.is_empty() {
            unknown.sort_unstable();
            return Err(DomainError::invalid(format!(
                "unknown metadata fields {}",
                str_list_repr(&unknown)
            )));
        }
        let p = project.edit(|d| {
            for (field, value) in &self.values {
                if let Some(slot) = d.metadata.get_mut(field) {
                    slot.clone_from(value);
                }
            }
        })?;
        Ok(Outcome::new(p, project_change(Flag::Metadata)))
    }
}

/// Changes loop, punch and metronome settings.
#[derive(Debug, Clone, Default)]
pub struct SetTransportSettings {
    /// New values by field name.
    pub values: JsonMap,
}

const TRANSPORT_FIELDS: [&str; 8] = [
    "loop_enabled",
    "loop_start",
    "loop_end",
    "punch_enabled",
    "punch_in",
    "punch_out",
    "metronome",
    "count_in_bars",
];

fn transport_bool(name: &str, value: &Value) -> Result<bool> {
    value.as_bool().ok_or_else(|| {
        DomainError::invalid(format!(
            "transport setting {} must be a boolean",
            str_repr(name)
        ))
    })
}

fn transport_number(name: &str, value: &Value) -> Result<f64> {
    value
        .as_f64()
        .filter(|_| !value.is_boolean())
        .ok_or_else(|| {
            DomainError::invalid(format!(
                "transport setting {} must be a number",
                str_repr(name)
            ))
        })
}

impl Command for SetTransportSettings {
    fn name(&self) -> &'static str {
        "SetTransportSettings"
    }

    fn label(&self) -> &'static str {
        "Transport Settings"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let mut unknown: Vec<&str> = self
            .values
            .keys()
            .map(String::as_str)
            .filter(|k| !TRANSPORT_FIELDS.contains(k))
            .collect();
        if !unknown.is_empty() {
            unknown.sort_unstable();
            return Err(DomainError::invalid(format!(
                "unknown transport settings {}",
                str_list_repr(&unknown)
            )));
        }
        let mut transport = project.transport;
        for (name, value) in &self.values {
            match name.as_str() {
                "loop_enabled" => transport.loop_enabled = transport_bool(name, value)?,
                "punch_enabled" => transport.punch_enabled = transport_bool(name, value)?,
                "metronome" => transport.metronome = transport_bool(name, value)?,
                "loop_start" => transport.loop_start = transport_number(name, value)?,
                "loop_end" => transport.loop_end = transport_number(name, value)?,
                "punch_in" => transport.punch_in = transport_number(name, value)?,
                "punch_out" => transport.punch_out = transport_number(name, value)?,
                _ => {
                    // An invalid count-in fails `validate` after the loop and punch checks, as
                    // in the prototype.
                    transport.count_in_bars = value
                        .as_i64()
                        .and_then(|bars| TransportSettings::count_in(bars).ok())
                        .unwrap_or(u32::MAX);
                }
            }
        }
        transport.validate()?;
        let p = project.edit(|d| d.transport = transport)?;
        Ok(Outcome::new(p, project_change(Flag::Transport)))
    }

    fn merge_key(&self) -> Option<String> {
        let mut keys: Vec<&str> = self.values.keys().map(String::as_str).collect();
        keys.sort_unstable();
        Some(format!("transport:{}", keys.join(",")))
    }
}

// -- markers and sections -------------------------------------------------------------------

/// Adds a marker.
#[derive(Debug, Clone)]
pub struct AddMarker {
    /// Position in beats.
    pub beat: f64,
    /// Display name.
    pub name: String,
    /// `#rrggbb`, or `None` for automatic.
    pub color: Option<String>,
    /// The new marker's ID.
    pub marker_id: String,
}

impl Command for AddMarker {
    fn name(&self) -> &'static str {
        "AddMarker"
    }

    fn label(&self) -> &'static str {
        "Add Marker"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let marker = Marker::new(
            self.marker_id.clone(),
            self.beat,
            self.name.clone(),
            self.color.clone(),
        )?;
        let p = project.edit(|d| d.markers.push(marker))?;
        Ok(Outcome::with_result(
            p,
            ChangeSet::of(
                [(ObjectKind::Marker, self.marker_id.as_str())],
                [],
                [],
                [Flag::Markers],
            ),
            json!({"marker_id": self.marker_id}),
        ))
    }
}

/// Changes a marker; `None` keeps a value.
#[derive(Debug, Clone)]
pub struct UpdateMarker {
    /// The marker.
    pub marker_id: String,
    /// New position.
    pub beat: Option<f64>,
    /// New name.
    pub name: Option<String>,
    /// New color.
    pub color: Option<String>,
}

impl Command for UpdateMarker {
    fn name(&self) -> &'static str {
        "UpdateMarker"
    }

    fn label(&self) -> &'static str {
        "Edit Marker"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let m = project
            .find_marker(&self.marker_id)
            .ok_or_else(|| DomainError::not_found("marker", &self.marker_id))?;
        let new = Marker::new(
            m.id.clone(),
            self.beat.unwrap_or(m.beat),
            self.name.clone().unwrap_or_else(|| m.name.clone()),
            self.color.clone().or_else(|| m.color.clone()),
        )?;
        let p = project.edit(|d| {
            for slot in &mut d.markers {
                if slot.id == new.id {
                    *slot = new.clone();
                }
            }
        })?;
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [(ObjectKind::Marker, m.id.as_str())],
                [],
                [Flag::Markers],
            ),
        ))
    }
}

/// Removes a marker.
#[derive(Debug, Clone)]
pub struct RemoveMarker {
    /// The marker.
    pub marker_id: String,
}

impl Command for RemoveMarker {
    fn name(&self) -> &'static str {
        "RemoveMarker"
    }

    fn label(&self) -> &'static str {
        "Remove Marker"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        if project.find_marker(&self.marker_id).is_none() {
            return Err(DomainError::not_found("marker", &self.marker_id));
        }
        let p = project.edit(|d| d.markers.retain(|m| m.id != self.marker_id))?;
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [],
                [(ObjectKind::Marker, self.marker_id.as_str())],
                [Flag::Markers],
            ),
        ))
    }
}

/// Adds an arrangement section.
#[derive(Debug, Clone)]
pub struct AddSection {
    /// Display name.
    pub name: String,
    /// Start in beats.
    pub start: f64,
    /// Length in beats.
    pub length: f64,
    /// `#rrggbb`, or `None` for automatic.
    pub color: Option<String>,
    /// The new section's ID.
    pub section_id: String,
}

impl Command for AddSection {
    fn name(&self) -> &'static str {
        "AddSection"
    }

    fn label(&self) -> &'static str {
        "Add Section"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let section = Section::new(
            self.section_id.clone(),
            self.name.clone(),
            self.start,
            self.length,
            self.color.clone(),
        )?;
        let p = project.edit(|d| d.sections.push(section))?;
        Ok(Outcome::with_result(
            p,
            ChangeSet::of(
                [(ObjectKind::Section, self.section_id.as_str())],
                [],
                [],
                [Flag::Markers],
            ),
            json!({"section_id": self.section_id}),
        ))
    }
}

/// Changes a section; `None` keeps a value.
#[derive(Debug, Clone)]
pub struct UpdateSection {
    /// The section.
    pub section_id: String,
    /// New name.
    pub name: Option<String>,
    /// New start.
    pub start: Option<f64>,
    /// New length.
    pub length: Option<f64>,
    /// New color.
    pub color: Option<String>,
}

impl Command for UpdateSection {
    fn name(&self) -> &'static str {
        "UpdateSection"
    }

    fn label(&self) -> &'static str {
        "Edit Section"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let s = project
            .find_section(&self.section_id)
            .ok_or_else(|| DomainError::not_found("section", &self.section_id))?;
        let new = Section::new(
            s.id.clone(),
            self.name.clone().unwrap_or_else(|| s.name.clone()),
            self.start.unwrap_or(s.start),
            self.length.unwrap_or(s.length),
            self.color.clone().or_else(|| s.color.clone()),
        )?;
        let p = project.edit(|d| {
            for slot in &mut d.sections {
                if slot.id == new.id {
                    *slot = new.clone();
                }
            }
        })?;
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [(ObjectKind::Section, s.id.as_str())],
                [],
                [Flag::Markers],
            ),
        ))
    }
}

/// Removes a section.
#[derive(Debug, Clone)]
pub struct RemoveSection {
    /// The section.
    pub section_id: String,
}

impl Command for RemoveSection {
    fn name(&self) -> &'static str {
        "RemoveSection"
    }

    fn label(&self) -> &'static str {
        "Remove Section"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        if project.find_section(&self.section_id).is_none() {
            return Err(DomainError::not_found("section", &self.section_id));
        }
        let p = project.edit(|d| d.sections.retain(|s| s.id != self.section_id))?;
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [],
                [(ObjectKind::Section, self.section_id.as_str())],
                [Flag::Markers],
            ),
        ))
    }
}

// -- media assets ---------------------------------------------------------------------------

/// Adds a media asset record.
#[derive(Debug, Clone)]
pub struct AddAsset {
    /// The new asset.
    pub asset: MediaAsset,
}

impl Command for AddAsset {
    fn name(&self) -> &'static str {
        "AddAsset"
    }

    fn label(&self) -> &'static str {
        "Add Media"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let id = self.asset.id.as_str();
        if project.find_asset(id).is_some() {
            return Err(DomainError::new(
                ErrorCode::Conflict,
                format!("asset {} already exists", str_repr(id)),
            ));
        }
        let p = project.with_asset(self.asset.clone())?;
        Ok(Outcome::with_result(
            p,
            ChangeSet::of([(ObjectKind::Asset, id)], [], [], [Flag::Media]),
            json!({"asset_id": id}),
        ))
    }
}

/// Updates an asset record in place (relinking, re-hashing).
#[derive(Debug, Clone)]
pub struct ReplaceAsset {
    /// The new record, with the ID of an existing asset.
    pub asset: MediaAsset,
}

impl Command for ReplaceAsset {
    fn name(&self) -> &'static str {
        "ReplaceAsset"
    }

    fn label(&self) -> &'static str {
        "Relink Media"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let id = self.asset.id.as_str();
        project.asset(id)?;
        let p = project.with_asset(self.asset.clone())?;
        Ok(Outcome::new(
            p,
            ChangeSet::of([], [(ObjectKind::Asset, id)], [], [Flag::Media]),
        ))
    }
}

/// Removes an unused media asset record.
#[derive(Debug, Clone)]
pub struct RemoveAsset {
    /// The asset.
    pub asset_id: String,
}

impl Command for RemoveAsset {
    fn name(&self) -> &'static str {
        "RemoveAsset"
    }

    fn label(&self) -> &'static str {
        "Remove Media"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let id = self.asset_id.as_str();
        project.asset(id)?;
        if project.referenced_asset_ids().contains(id) {
            return Err(DomainError::new(
                ErrorCode::Conflict,
                format!("asset {} is still used by clips or devices", str_repr(id)),
            ));
        }
        let p = project.edit(|d| d.assets.retain(|a| a.id != id))?;
        Ok(Outcome::new(
            p,
            ChangeSet::of([], [], [(ObjectKind::Asset, id)], [Flag::Media]),
        ))
    }
}

/// Persisted editor settings (zoom, snap, track heights). A command so that every project
/// change flows through the session, but neither undoable nor "dirtying". `null` removes a key.
#[derive(Debug, Clone, Default)]
pub struct SetEditorState {
    /// New values by key.
    pub values: JsonMap,
}

impl Command for SetEditorState {
    fn name(&self) -> &'static str {
        "SetEditorState"
    }

    fn label(&self) -> &'static str {
        "Editor Settings"
    }

    fn undoable(&self) -> bool {
        false
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let p = project.edit(|d| {
            for (key, value) in &self.values {
                if value.is_null() {
                    d.editor.shift_remove(key);
                } else {
                    d.editor.insert(key.clone(), value.clone());
                }
            }
        })?;
        Ok(Outcome::new(p, ChangeSet::flags([Flag::Editor])))
    }

    fn merge_key(&self) -> Option<String> {
        Some("editor".to_owned())
    }
}
