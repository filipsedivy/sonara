//! Project tools (the prototype's "project" section): the state views.

use serde_json::{Value, json};
use sonara_core::ids::MASTER_ID;

use crate::context::AgentContext;
use crate::contract::args::{Detail, Empty, GetProjectState};
use crate::registry::ToolResult;
use crate::state::{StateOptions, project_state};

/// `get_project_state`: the project state view, with the session's version, dirty flag, path
/// and the next undo and redo labels.
pub(super) fn get_project_state(ctx: &AgentContext, args: GetProjectState) -> ToolResult {
    let GetProjectState {
        detail,
        include_notes,
        track_ids,
    } = args;
    // One short read of the session; the view is built after the lock is released.
    let (project, version, dirty, path, undo, redo) = {
        let session = ctx.app().session().lock();
        let project = session.require_project()?.clone();
        let location = session.require_location()?;
        (
            project,
            session.version(),
            session.is_dirty(),
            location
                .path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            session.history().undo_label().map(str::to_owned),
            session.history().redo_label().map(str::to_owned),
        )
    };
    let state = project_state(
        &project,
        ctx.app().plugins(),
        StateOptions {
            full: detail == Detail::Full,
            include_notes,
            track_ids: track_ids.as_deref(),
        },
    );
    Ok(json!({
        "state": state,
        "project_version": version,
        "dirty": dirty,
        "path": path,
        "undo": undo,
        "redo": redo,
    }))
}

/// `list_tracks`: the tracks in display order with kind, mixer state, instrument and clip
/// count, and the master.
pub(super) fn list_tracks(ctx: &AgentContext, _args: Empty) -> ToolResult {
    let project = ctx.project()?;
    let tracks: Vec<Value> = project
        .tracks
        .iter()
        .map(|track| {
            json!({
                "track_id": track.id,
                "name": track.name,
                "kind": track.kind.as_str(),
                "volume_db": track.mixer.volume_db,
                "pan": track.mixer.pan,
                "mute": track.mixer.mute,
                "solo": track.mixer.solo,
                "output": track.output_id,
                "clips": track.clips.len(),
                "instrument": track.instrument.as_ref().map(|device| device.plugin_id.as_str()),
                "effects": track.inserts.iter().map(|device| device.plugin_id.as_str()).collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(json!({
        "tracks": tracks,
        "master": {
            "track_id": MASTER_ID,
            "volume_db": project.master.mixer.volume_db,
            "effects": project.master.inserts.iter().map(|device| device.plugin_id.as_str()).collect::<Vec<_>>(),
        },
    }))
}
