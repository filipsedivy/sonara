//! Real-time tools (the prototype's "playback" section): playback, recording and live MIDI input
//! through the audio and MIDI devices, on the application's transport service
//! ([`AppContext::transport`](sonara_app::context::AppContext::transport)).
//!
//! The service answers with the prototype's JSON and errors: without a usable audio backend
//! (the `null` backend, or a native one that cannot start) every call that needs the engine
//! fails with `ENGINE_UNAVAILABLE`, and stopping reports that nothing was playing.

use serde_json::{Value, json};
use sonara_app::context::AppContext;

use super::positions::Positions;
use crate::context::AgentContext;
use crate::contract::args::{Empty, PlayPreview, SetMidiInput, StartRecording};
use crate::registry::{ToolError, ToolResult};

/// `play_preview`: real-time playback from a position (the playhead when absent), optionally a
/// looped range; answers the player's status.
pub(super) fn play_preview(ctx: &AgentContext, args: PlayPreview) -> ToolResult {
    let PlayPreview { start, end, r#loop } = args;
    let positions = Positions::new(ctx);
    let start = start.map(|p| positions.beats(&p)).transpose()?;
    let end = end.map(|p| positions.beats(&p)).transpose()?;
    Ok(ctx.app().transport().play(start, end, r#loop)?)
}

/// `start_recording`: records armed tracks (audio from the input device, MIDI from the open
/// MIDI inputs) from a position until `stop_playback`, which returns the takes.
pub(super) fn start_recording(ctx: &AgentContext, args: StartRecording) -> ToolResult {
    let StartRecording { start, track_ids } = args;
    let start = start.map(|p| Positions::new(ctx).beats(&p)).transpose()?;
    Ok(ctx.app().transport().record(start, track_ids.as_deref())?)
}

/// `list_midi_devices`: the MIDI ports and the live-input status
/// (`{"devices": midi_devices(), "status": midi_status()}`).
///
/// A port listing that fails (for example without an ALSA sequencer) is `INTERNAL`, as the
/// prototype's RtMidi error was.
pub(super) fn list_midi_devices(ctx: &AgentContext, _args: Empty) -> ToolResult {
    let transport = ctx.app().transport();
    let devices = transport
        .midi_devices()
        .map_err(|err| ToolError::Internal(err.to_string()))?;
    Ok(json!({"devices": devices, "status": transport.midi_status()}))
}

/// `set_midi_input`: opens MIDI inputs for live playing and recording into armed instrument
/// tracks (needs the engine), or closes them.
pub(super) fn set_midi_input(ctx: &AgentContext, args: SetMidiInput) -> ToolResult {
    let transport = ctx.app().transport();
    if !args.enabled {
        return Ok(transport.disable_midi());
    }
    Ok(transport.enable_midi(args.ports)?)
}

/// `stop_playback`: stops playback or recording; without a transport nothing was playing, and
/// none is created.
pub(super) fn stop_playback(ctx: &AgentContext, _args: Empty) -> ToolResult {
    if !ctx.app().has_transport() {
        return Ok(json!({"stopped": true, "was_playing": false}));
    }
    Ok(ctx.app().transport().stop()?)
}

/// The transport part of `get_capabilities` (`TransportService.status()`).
pub(super) fn engine_status(app: &AppContext) -> Value {
    app.transport().status()
}
