//! Media tools: importing audio, MIDI files and impulse responses, exporting MIDI files, and
//! relinking media (the prototype's "clips", "devices", "export" and integrity sections).

use std::path::PathBuf;

use serde_json::{Value, json};
use sonara_app::media::{AudioImport, ImportMode};
use sonara_core::error::{DomainError, ErrorCode};

use super::positions::Positions;
use crate::context::AgentContext;
use crate::contract::args::{
    self, ExportMidi, ImportAudio, ImportMidi, LoadImpulseResponse, Relink,
};
use crate::permissions::Access;
use crate::registry::ToolResult;

/// `result | {"committed": committed}`: the flag set in place, or appended.
fn committed(mut result: Value, committed: bool) -> Value {
    result["committed"] = Value::Bool(committed);
    result
}

/// `import_audio`: an audio file onto a new or existing audio track; `copy` embeds it in the
/// project, `link` references it where it is.
pub(super) fn import_audio(ctx: &AgentContext, args: ImportAudio) -> ToolResult {
    let path = ctx.path(&args.path, Access::Read)?;
    let start_beat = Positions::new(ctx).beats(&args.start)?;
    let mode = match args.mode {
        args::ImportMode::Copy => ImportMode::Copy,
        args::ImportMode::Link => ImportMode::Link,
    };
    let result = ctx.app().media().import_audio(
        &path,
        &AudioImport {
            track_id: args.track_id,
            start_beat,
            mode,
            track_name: args.track_name,
            create_clip: true,
            dry_run: args.dry_run,
        },
    )?;
    Ok(committed(result, !args.dry_run))
}

/// `import_midi`: a Standard MIDI File, one instrument track per MIDI track and channel (the
/// drum channel plays the drum kit), optionally with the file's tempo map.
pub(super) fn import_midi(ctx: &AgentContext, args: ImportMidi) -> ToolResult {
    let ImportMidi {
        dry_run,
        path,
        apply_tempo,
        start,
    } = args;
    let path = ctx.path(&path, Access::Read)?;
    let media = ctx.app().media();
    if dry_run {
        let imported = media.read_midi(&path)?;
        let tracks: Vec<Value> = imported
            .tracks
            .iter()
            .map(|track| {
                json!({
                    "name": track.name,
                    "notes": track.notes.len(),
                    "drums": track.is_drums(),
                })
            })
            .collect();
        return Ok(json!({"dry_run": true, "committed": false, "tracks": tracks}));
    }
    let start_beat = Positions::new(ctx).beats(&start)?;
    let result = media.import_midi(&path, apply_tempo, start_beat)?;
    Ok(committed(result, true))
}

/// `load_impulse_response`: an impulse-response file into a Convolution Reverb device.
pub(super) fn load_impulse_response(ctx: &AgentContext, args: LoadImpulseResponse) -> ToolResult {
    let LoadImpulseResponse {
        dry_run,
        device_id,
        path,
    } = args;
    let path = ctx.path(&path, Access::Read)?;
    let media = ctx.app().media();
    if dry_run {
        let file = media.inspect(&path)?.to_json();
        return Ok(json!({"dry_run": true, "committed": false, "file": file}));
    }
    let result = media.import_impulse_response(&path, &device_id)?;
    Ok(committed(result, true))
}

/// `export_midi`: instrument tracks as a Standard MIDI File (Type 1, 960 PPQ) with the tempo
/// map, time signatures and markers. Overwriting needs `overwrite` and confirmation.
#[expect(
    clippy::needless_pass_by_value,
    reason = "every handler takes its arguments by value (ToolRegistry::register); a \
              confirmation is bound to all of them"
)]
pub(super) fn export_midi(ctx: &AgentContext, args: ExportMidi) -> ToolResult {
    let mut path = ctx.path(&args.path, Access::Write)?;
    if !has_midi_suffix(&path) {
        path = with_suffix(&path, ".mid");
    }
    let shown = path.to_string_lossy().into_owned();
    if path.exists() && !args.overwrite {
        return Err(DomainError::new(
            ErrorCode::Conflict,
            format!("{shown} exists; set overwrite=true"),
        )
        .with_detail("path", shown)
        .into());
    }
    let needs = ctx.confirm(
        "export_midi",
        &args,
        path.exists(),
        &format!("{shown} will be overwritten"),
    )?;
    if args.dry_run {
        return Ok(json!({
            "dry_run": true,
            "committed": false,
            "path": shown,
            "requires_confirmation": needs,
        }));
    }
    let result = ctx
        .app()
        .media()
        .export_midi(&path, args.track_ids.as_deref(), args.overwrite)?;
    Ok(committed(result, true))
}

/// Python's `path.suffix.lower() in (".mid", ".midi")`.
fn has_midi_suffix(path: &std::path::Path) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let suffix = py_suffix(&name).to_lowercase();
    suffix == ".mid" || suffix == ".midi"
}

/// Python's `PurePath.suffix` of a file name: from the last dot, unless the dot is first or last.
fn py_suffix(name: &str) -> &str {
    match name.rfind('.') {
        Some(dot) if dot > 0 && dot + 1 < name.len() => &name[dot..],
        _ => "",
    }
}

/// Python's `path.with_suffix(suffix)`: the last suffix replaced, or `suffix` appended.
fn with_suffix(path: &std::path::Path, suffix: &str) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = &name[..name.len() - py_suffix(&name).len()];
    path.with_file_name(format!("{stem}{suffix}"))
}

/// `relink_media`: points a missing or moved media asset at a file, optionally copying it into
/// the project.
pub(super) fn relink_media(ctx: &AgentContext, args: Relink) -> ToolResult {
    let Relink {
        dry_run,
        asset_id,
        path,
        embed,
    } = args;
    let path = ctx.path(&path, Access::Read)?;
    if dry_run {
        let file = ctx.app().media().inspect(&path)?.to_json();
        return Ok(json!({"dry_run": true, "committed": false, "file": file}));
    }
    let relinked = ctx.app().projects().relink(&asset_id, &path, embed)?;
    Ok(relinked.to_json())
}
