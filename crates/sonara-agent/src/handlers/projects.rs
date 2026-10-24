//! Project tools (the prototype's "project" section): creating, opening and saving the project,
//! its tempo, meter, length, sections, markers, metadata and loop, and the integrity check.

use std::path::Path;

use serde_json::{Value, json};
use sonara_app::projects::{NewProject, normalize_project_path};
use sonara_core::commands::project::{
    AddMarker, AddSection, SetMetadata, SetProjectLength, SetTempo, SetTimeSignature,
    SetTransportSettings,
};
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::ids::{self, IdGenerator as _};
use sonara_core::model::JsonMap;
use sonara_core::model::project::Project;
use sonara_core::time::{TimeSignatureMap, TimeSignaturePoint};

use super::positions::Positions;
use crate::context::{AgentContext, model_dump};
use crate::contract::args::{
    self, AddMarker as AddMarkerArgs, AddSection as AddSectionArgs, CreateProject, Empty,
    OpenProject, SaveProject, SetLoop, SetMetadata as SetMetadataArgs,
    SetProjectLength as SetProjectLengthArgs, SetTempo as SetTempoArgs,
    SetTimeSignature as SetTimeSignatureArgs,
};
use crate::conversion;
use crate::permissions::Access;
use crate::registry::ToolResult;
use crate::state::{StateOptions, project_state};

/// The sample rate of the project that only gives `create_project`'s `length` a meter and tempo
/// (the prototype's `Project.create` default).
const SHAPE_SAMPLE_RATE: i64 = 48_000;

/// A path as tool results spell it (Python's `str(path)`).
fn shown(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Python's `round(x, 3)`: the correctly rounded decimal string, parsed back.
fn round3(value: f64) -> f64 {
    format!("{value:.3}").parse().unwrap_or(value)
}

/// The template ID of `create_project`'s `template`.
fn template_id(template: args::Template) -> &'static str {
    match template {
        args::Template::Empty => "empty",
        args::Template::Electronic => "electronic",
        args::Template::Band => "band",
        args::Template::Orchestral => "orchestral",
    }
}

/// `int(x) for x in time_signature.split("/")` of a value the contract's pattern admitted.
///
/// The pattern's `\d` admits every Unicode decimal digit, which Python's `int` reads; this
/// reads ASCII digits only and rejects the rest as an invalid time signature (the same
/// deviation as [`conversion`]'s numbers).
fn time_signature(text: &str) -> Result<(u32, u32), DomainError> {
    let invalid = || {
        DomainError::invalid(format!(
            "invalid time signature {}",
            sonara_core::pyfmt::str_repr(text)
        ))
    };
    let (numerator, denominator) = text.split_once('/').ok_or_else(invalid)?;
    let number = |part: &str| part.parse::<u32>().map_err(|_| invalid());
    Ok((number(numerator)?, number(denominator)?))
}

/// The project whose tempo and meter `create_project` measures its `length` with, before the
/// new project exists: named and timed like it, at the default rate. Drawing its ID is part of
/// the prototype's ID order.
fn shape_project(
    ctx: &AgentContext,
    name: &str,
    bpm: f64,
    (numerator, denominator): (u32, u32),
) -> Result<Project, DomainError> {
    let mut ids = ctx.app().ids();
    let project = Project::create(&mut ids, name, SHAPE_SAMPLE_RATE, bpm)?;
    let signatures =
        TimeSignatureMap::new(vec![TimeSignaturePoint::new(0, numerator, denominator)?])?;
    project.edit(|data| data.time_signatures = signatures)
}

/// `create_project`: a new project replacing the open one (discarding unsaved changes needs
/// confirmation), optionally saved at once; returns the project's state view.
pub(super) fn create_project(ctx: &AgentContext, args: CreateProject) -> ToolResult {
    let meter = time_signature(&args.time_signature)?;
    if args.dry_run {
        let dirty = ctx.app().session().lock().is_dirty();
        return Ok(json!({
            "dry_run": true,
            "committed": false,
            "would_discard_unsaved": dirty,
        }));
    }
    ctx.require_clean_or_confirm("create_project", &args)?;
    let length = match &args.length {
        Some(length) => {
            let shape = shape_project(ctx, &args.name, args.bpm, meter)?;
            Some(conversion::length_beats(length, 0.0, &shape)?)
        }
        None => None,
    };
    let path = args.path.filter(|path| !path.is_empty());
    let project = ctx.app().projects().new_project(&NewProject {
        name: args.name,
        sample_rate: i64::from(args.sample_rate),
        bpm: args.bpm,
        time_signature: meter,
        template: template_id(args.template).to_owned(),
        length_beats: length,
    })?;
    let mut out = json!({"committed": true, "project_id": project.id});
    // `if a.path:`: an empty path saves nothing.
    if let Some(path) = path {
        let target = ctx.path(&path, Access::Write)?;
        let saved = ctx.app().projects().save(Some(&target), None)?;
        ctx.app().session().lock().clear_history();
        out["path"] = Value::from(shown(&saved.path));
    }
    let (project, version) = {
        let session = ctx.app().session().lock();
        (session.require_project()?.clone(), session.version())
    };
    out["state"] = project_state(&project, ctx.app().plugins(), StateOptions::default());
    out["project_version"] = Value::from(version);
    Ok(out)
}

/// `open_project`: opens a `.sonara` bundle or archive (discarding unsaved changes needs
/// confirmation); reports recovery, missing media and autosaves, and the state view.
#[expect(
    clippy::needless_pass_by_value,
    reason = "every handler takes its arguments by value (ToolRegistry::register); a \
              confirmation is bound to all of them"
)]
pub(super) fn open_project(ctx: &AgentContext, args: OpenProject) -> ToolResult {
    let path = ctx.path(&args.path, Access::Read)?;
    if args.dry_run {
        return Ok(json!({
            "dry_run": true,
            "committed": false,
            "path": shown(&path),
            "exists": path.exists(),
        }));
    }
    ctx.require_clean_or_confirm("open_project", &args)?;
    let loaded = ctx
        .app()
        .projects()
        .open_project(&path, args.recover_autosave)?;
    let state = project_state(
        &loaded.project,
        ctx.app().plugins(),
        StateOptions::default(),
    );
    let version = ctx.app().session().lock().version();
    Ok(json!({
        "committed": true,
        "path": shown(&loaded.path),
        "layout": loaded.layout.as_str(),
        "warnings": loaded.warnings,
        "recovered_from": loaded.recovered_from,
        "autosave_available": loaded.autosave_available,
        "missing_assets": loaded.missing_assets,
        "migrated_from_schema": loaded.migrated_from,
        "state": state,
        "project_version": version,
    }))
}

/// `save_project`: saves in place, to a new path (Save As) or as a copy. Overwriting another
/// existing file or directory needs confirmation.
#[expect(
    clippy::needless_pass_by_value,
    reason = "every handler takes its arguments by value (ToolRegistry::register); a \
              confirmation is bound to all of them"
)]
pub(super) fn save_project(ctx: &AgentContext, args: SaveProject) -> ToolResult {
    let location = {
        let session = ctx.app().session().lock();
        session.require_project()?;
        session.require_location()?.clone()
    };
    let mut target = None;
    let mut needs = false;
    if let Some(path) = args.path.as_deref().filter(|path| !path.is_empty()) {
        let normalized = normalize_project_path(&ctx.path(path, Access::Write)?)?;
        let overwrite = normalized.exists()
            && location
                .path
                .as_ref()
                .is_none_or(|open| *open != normalized);
        needs = ctx.confirm(
            "save_project",
            &args,
            overwrite,
            &format!(
                "{} already exists and will be overwritten",
                shown(&normalized)
            ),
        )?;
        target = Some(normalized);
    } else if location.path.is_none() {
        return Err(DomainError::invalid("the project has never been saved; pass a path").into());
    }
    if args.dry_run {
        let shown_target = target.as_ref().or(location.path.as_ref()).map(|p| shown(p));
        return Ok(json!({
            "dry_run": true,
            "committed": false,
            "target": shown_target,
            "requires_confirmation": needs,
        }));
    }
    let projects = ctx.app().projects();
    let saved = if args.as_copy {
        let Some(target) = &target else {
            return Err(DomainError::invalid("copy requires a path").into());
        };
        projects.save_copy(target, args.packaged)?
    } else {
        // `a.packaged if a.path or a.packaged else None`
        let packaged = (target.is_some() || args.packaged).then_some(args.packaged);
        projects.save(target.as_deref(), packaged)?
    };
    let dirty = ctx.app().session().lock().is_dirty();
    Ok(json!({
        "committed": true,
        "path": shown(&saved.path),
        "layout": saved.layout.as_str(),
        "assets": saved.assets,
        "project_sha256": saved.project_sha256,
        "backup": saved.backup.as_deref().map(shown),
        "dirty": dirty,
    }))
}

/// `set_tempo`: the song tempo, or a tempo change (optionally a ramp) at a position.
pub(super) fn set_tempo(ctx: &AgentContext, args: SetTempoArgs) -> ToolResult {
    let SetTempoArgs {
        dry_run,
        bpm,
        at,
        ramp,
        clear_changes,
    } = args;
    let beat = Positions::new(ctx).beats(&at)?;
    let command = SetTempo {
        bpm,
        beat,
        ramp,
        clear_changes,
    };
    Ok(ctx.commit(&command, dry_run)?)
}

/// `set_time_signature`: the time signature from a 1-based bar on.
#[expect(
    clippy::needless_pass_by_value,
    reason = "every handler takes its arguments by value (ToolRegistry::register)"
)]
pub(super) fn set_time_signature(ctx: &AgentContext, args: SetTimeSignatureArgs) -> ToolResult {
    let command = SetTimeSignature {
        numerator: i64::from(args.numerator),
        denominator: i64::from(args.denominator),
        // The contract admits bars from 1.
        bar: i64::try_from(args.bar)
            .unwrap_or(i64::MAX)
            .saturating_sub(1),
    };
    Ok(ctx.commit(&command, args.dry_run)?)
}

/// `set_project_length`: the song length (render and export end); `null` is the end of the last
/// clip. A set length is also reported in seconds.
pub(super) fn set_project_length(ctx: &AgentContext, args: SetProjectLengthArgs) -> ToolResult {
    let SetProjectLengthArgs { dry_run, length } = args;
    let length = match length {
        Some(length) => Some(Positions::new(ctx).length_beats(&length, 0.0)?),
        None => None,
    };
    let mut out = ctx.commit(
        &SetProjectLength {
            length_beats: length,
        },
        dry_run,
    )?;
    if let Some(length) = length {
        let seconds = ctx.project()?.beat_to_seconds(length);
        out["length_seconds"] = Value::from(round3(seconds));
    }
    Ok(out)
}

/// `add_section`: a named song section spanning a range.
pub(super) fn add_section(ctx: &AgentContext, args: AddSectionArgs) -> ToolResult {
    let positions = Positions::new(ctx);
    let start = positions.beats(&args.start)?;
    let length = positions.length_beats(&args.length, start)?;
    let command = AddSection {
        name: args.name,
        start,
        length,
        color: args.color,
        section_id: ctx.app().ids().new_id(ids::SECTION),
    };
    Ok(ctx.commit(&command, args.dry_run)?)
}

/// `add_marker`: a named timeline marker.
pub(super) fn add_marker(ctx: &AgentContext, args: AddMarkerArgs) -> ToolResult {
    let beat = Positions::new(ctx).beats(&args.at)?;
    let command = AddMarker {
        beat,
        name: args.name,
        color: None,
        marker_id: ctx.app().ids().new_id(ids::MARKER),
    };
    Ok(ctx.commit(&command, args.dry_run)?)
}

/// `set_metadata`: title, artist, album, genre, composer and comment, the fields given.
#[expect(
    clippy::needless_pass_by_value,
    reason = "every handler takes its arguments by value (ToolRegistry::register)"
)]
pub(super) fn set_metadata(ctx: &AgentContext, args: SetMetadataArgs) -> ToolResult {
    // `{k: v for k, v in model_dump(a).items() if k != "dry_run"}`: the fields that are set.
    let values = model_dump(&args)?
        .into_iter()
        .filter(|(key, _)| key != "dry_run")
        .map(|(key, value)| match value {
            Value::String(text) => Ok((key, text)),
            other => Err(DomainError::new(
                ErrorCode::Internal,
                format!("metadata field {key} is not text: {other}"),
            )),
        })
        .collect::<Result<_, _>>()?;
    Ok(ctx.commit(&SetMetadata { values }, args.dry_run)?)
}

/// `set_loop`: enables or disables the transport loop and positions it; an end not given keeps
/// the current one.
pub(super) fn set_loop(ctx: &AgentContext, args: SetLoop) -> ToolResult {
    let SetLoop {
        dry_run,
        enabled,
        start,
        end,
    } = args;
    let positions = Positions::new(ctx);
    let transport = positions.project()?.transport;
    let start = match start {
        Some(start) => positions.beats(&start)?,
        None => transport.loop_start,
    };
    let end = match end {
        Some(end) => positions.beats(&end)?,
        None => transport.loop_end,
    };
    let mut values = JsonMap::new();
    values.insert("loop_enabled".to_owned(), Value::Bool(enabled));
    values.insert("loop_start".to_owned(), Value::from(start));
    values.insert("loop_end".to_owned(), Value::from(end));
    Ok(ctx.commit(&SetTransportSettings { values }, dry_run)?)
}

/// `verify_project`: media presence, sizes and hashes, and the routing graph; `intact` is true
/// when nothing is missing, mismatched or misrouted.
pub(super) fn verify_project(ctx: &AgentContext, _args: Empty) -> ToolResult {
    let mut report = ctx.app().projects().verify(true)?.to_json();
    if let Value::Object(map) = &mut report {
        // `report["intact"] = report.pop("ok")`: the others keep their order, `intact` is last.
        let intact = map.shift_remove("ok").unwrap_or(Value::Null);
        map.insert("intact".to_owned(), intact);
    }
    Ok(report)
}
