//! Track tools (the prototype's "tracks" section): creating, renaming and deleting tracks, the
//! mixer, track state, the instrument and routing.

use serde_json::{Value, json};
use sonara_core::commands::Command;
use sonara_core::commands::Transaction;
use sonara_core::commands::device::{SetInstrument as SetInstrumentCommand, SetSidechain};
use sonara_core::commands::track::{
    AddSend, CreateTrack as CreateTrackCommand, DeleteTrack as DeleteTrackCommand, RemoveSend,
    RenameTrack as RenameTrackCommand, SetTrackColor, SetTrackInput, SetTrackMixer, SetTrackOutput,
    UpdateSend,
};
use sonara_core::error::DomainError;
use sonara_core::ids::{self, IdGenerator};
use sonara_core::model::track::TrackKind;
use sonara_core::pyfmt::str_repr;

use super::effects::param_values;
use crate::context::AgentContext;
use crate::contract::args::{
    self, ConfigureRouting, CreateTrack, DeleteTrack, RenameTrack, SetInstrument, SetTrackPan,
    SetTrackState, SetTrackVolume,
};
use crate::conversion;
use crate::registry::ToolResult;

/// The instrument of a new instrument track that names none.
const DEFAULT_INSTRUMENT: &str = "sonara.synth.poly";

/// A list position of the arguments (`ge=0`) as the commands take it; positions are clamped to
/// the list, so the largest values all mean the end.
pub(super) fn list_index(index: Option<u64>) -> Option<i64> {
    index.map(|index| i64::try_from(index).unwrap_or(i64::MAX))
}

fn track_kind(kind: args::TrackKind) -> TrackKind {
    match kind {
        args::TrackKind::Audio => TrackKind::Audio,
        args::TrackKind::Instrument => TrackKind::Instrument,
        args::TrackKind::Bus => TrackKind::Bus,
        args::TrackKind::Folder => TrackKind::Folder,
    }
}

/// `create_track`: an audio, instrument, bus or folder track; instrument tracks get the poly
/// synth unless they name an instrument, and may state the part they play.
///
/// Both new IDs are drawn for every kind, the instrument's too, as the prototype's command
/// draws them. A part is checked first, so a part that names no instrument type draws none.
pub(super) fn create_track(ctx: &AgentContext, args: CreateTrack) -> ToolResult {
    let CreateTrack {
        dry_run,
        name,
        kind,
        instrument,
        preset,
        color,
        index,
        output,
        parent_id,
        part,
    } = args;
    let part = part.as_ref().map(conversion::part).transpose()?;
    let kind = track_kind(kind);
    let instrument = match instrument {
        None if kind == TrackKind::Instrument => Some(DEFAULT_INSTRUMENT.to_owned()),
        other => other,
    };
    let mut ids = ctx.app().ids();
    let track_id = ids.new_id(ids::TRACK);
    let instrument_id = ids.new_id(ids::DEVICE);
    let command = CreateTrackCommand {
        name,
        kind,
        index: list_index(index),
        color,
        output_id: output,
        instrument_plugin: instrument,
        instrument_preset: preset,
        parent_id,
        part,
        track_id,
        instrument_id,
    };
    Ok(ctx.commit(&command, dry_run)?)
}

/// `rename_track`: a new name; the ID stays.
pub(super) fn rename_track(ctx: &AgentContext, args: RenameTrack) -> ToolResult {
    let RenameTrack {
        dry_run,
        track_id,
        name,
    } = args;
    Ok(ctx.commit(&RenameTrackCommand { track_id, name }, dry_run)?)
}

/// `delete_track`: deletes a track and repairs the references to it. A track with clips or
/// effects needs confirmation.
pub(super) fn delete_track(ctx: &AgentContext, args: DeleteTrack) -> ToolResult {
    let (needed, description) = {
        let project = ctx.project()?;
        let track = project.track(&args.track_id)?;
        (
            !track.clips.is_empty() || !track.inserts.is_empty(),
            format!(
                "track {} has {} clip(s) and {} effect(s)",
                str_repr(&track.name),
                track.clips.len(),
                track.inserts.len()
            ),
        )
    };
    ctx.confirm("delete_track", &args, needed, &description)?;
    let command = DeleteTrackCommand {
        track_id: args.track_id,
    };
    Ok(ctx.commit(&command, args.dry_run)?)
}

/// `set_track_volume`: a track's (or the master's) fader level.
pub(super) fn set_track_volume(ctx: &AgentContext, args: SetTrackVolume) -> ToolResult {
    let command = SetTrackMixer {
        track_id: args.track_id,
        volume_db: Some(args.volume_db),
        ..SetTrackMixer::default()
    };
    Ok(ctx.commit(&command, args.dry_run)?)
}

/// `set_track_pan`: a track's pan.
pub(super) fn set_track_pan(ctx: &AgentContext, args: SetTrackPan) -> ToolResult {
    let command = SetTrackMixer {
        track_id: args.track_id,
        pan: Some(args.pan),
        ..SetTrackMixer::default()
    };
    Ok(ctx.commit(&command, args.dry_run)?)
}

/// `set_track_state`: mute, solo, solo-safe, record arm and colour, as one undo step
/// (`Track State`).
pub(super) fn set_track_state(ctx: &AgentContext, args: SetTrackState) -> ToolResult {
    let SetTrackState {
        dry_run,
        track_id,
        mute,
        solo,
        solo_safe,
        color,
        armed,
    } = args;
    let mut commands: Vec<Box<dyn Command>> = Vec::new();
    if mute.is_some() || solo.is_some() || solo_safe.is_some() {
        commands.push(Box::new(SetTrackMixer {
            track_id: track_id.clone(),
            mute,
            solo,
            solo_safe,
            ..SetTrackMixer::default()
        }));
    }
    if let Some(color) = color {
        commands.push(Box::new(SetTrackColor {
            track_id: track_id.clone(),
            color: Some(color),
        }));
    }
    if let Some(armed) = armed {
        commands.push(Box::new(SetTrackInput {
            track_id,
            armed: Some(armed),
            ..SetTrackInput::default()
        }));
    }
    commit_steps(ctx, commands, "Track State", dry_run)
}

/// `set_instrument`: replaces an instrument track's instrument.
pub(super) fn set_instrument(ctx: &AgentContext, args: SetInstrument) -> ToolResult {
    let SetInstrument {
        dry_run,
        track_id,
        plugin_id,
        preset,
        params,
    } = args;
    let device_id = ctx.app().ids().new_id(ids::DEVICE);
    let mut params = param_values(&params);
    // A sampler instrument given by its patch path, as before preset IDs, stores its ID.
    ctx.app()
        .instruments()
        .normalize_params(&plugin_id, &mut params);
    let command = SetInstrumentCommand {
        track_id,
        plugin_id,
        preset,
        params,
        device_id,
    };
    Ok(ctx.commit(&command, dry_run)?)
}

/// `configure_routing`: the output, sends to add, update and remove, and a device's sidechain
/// source, as one undo step (`Routing`). The result lists the new send IDs (`send_ids`).
pub(super) fn configure_routing(ctx: &AgentContext, args: ConfigureRouting) -> ToolResult {
    let ConfigureRouting {
        dry_run,
        track_id,
        output,
        add_sends,
        update_sends,
        remove_sends,
        sidechain_device_id,
        sidechain_source,
    } = args;
    let mut commands: Vec<Box<dyn Command>> = Vec::new();
    if let Some(output_id) = output {
        commands.push(Box::new(SetTrackOutput {
            track_id: track_id.clone(),
            output_id,
        }));
    }
    let mut ids = ctx.app().ids();
    let mut send_ids = Vec::with_capacity(add_sends.len());
    for send in add_sends {
        let send_id = ids.new_id(ids::SEND);
        send_ids.push(send_id.clone());
        commands.push(Box::new(AddSend {
            track_id: track_id.clone(),
            target_id: send.target,
            level_db: send.level_db,
            pre_fader: send.pre_fader,
            send_id,
        }));
    }
    for (send_id, level_db) in update_sends {
        commands.push(Box::new(UpdateSend {
            track_id: track_id.clone(),
            send_id,
            level_db: Some(level_db),
            pre_fader: None,
            enabled: None,
        }));
    }
    for send_id in remove_sends {
        commands.push(Box::new(RemoveSend {
            track_id: track_id.clone(),
            send_id,
        }));
    }
    if let Some(device_id) = sidechain_device_id {
        commands.push(Box::new(SetSidechain {
            device_id,
            // `a.sidechain_source or None`: an empty source clears the sidechain.
            source_track_id: sidechain_source.filter(|source| !source.is_empty()),
        }));
    }
    let mut out = commit_steps(ctx, commands, "Routing", dry_run)?;
    out["send_ids"] = json!(send_ids);
    Ok(out)
}

/// Commits `commands` as one undo step labelled `name` (the prototype's `Transaction`).
///
/// # Errors
///
/// [`sonara_core::error::ErrorCode::InvalidArgument`] `nothing to change` without commands,
/// or the commit's error.
pub(super) fn commit_steps(
    ctx: &AgentContext,
    commands: Vec<Box<dyn Command>>,
    name: &str,
    dry_run: bool,
) -> Result<Value, crate::registry::ToolError> {
    if commands.is_empty() {
        return Err(DomainError::invalid("nothing to change").into());
    }
    let transaction = Transaction {
        commands,
        name: name.to_owned(),
    };
    Ok(ctx.commit(&transaction, dry_run)?)
}
