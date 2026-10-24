//! The transport bar's calls into the transport: play, stop, record and
//! seek, the engine label's status ("Live data"), and the start
//! of live MIDI input, as the prototype's `SessionBridge.transport` and
//! `MainWindow._start_midi` ran them.
//!
//! The page keeps the transport bar's own state (the last start, the Record button) and decides,
//! from the live frame, whether the transport exists. These calls run the service and report
//! its errors with their codes for the status bar ([`ErrorNotice`]), as the bridge's
//! `errorOccurred` did.
//!
//! **Which calls create the transport.** Creating it opens the audio backend, so only the calls
//! the prototype made through `app.transport` create it: [`play`], [`record`], [`seek`] and,
//! with MIDI inputs present, [`start_live_midi`]. [`stop`] and [`engine_status`] never do, as
//! the prototype's `transport_status` and the agent's `stop_playback` do not.

use serde::Serialize;
use serde_json::Value;

use crate::context::AppContext;
use crate::events::{ErrorNotice, StatusMessage};

/// What a stop did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stopped {
    /// Whether the transport was playing or recording.
    pub was_playing: bool,
    /// The takes a recording that ended added, when one ended.
    pub takes: Option<usize>,
    /// "Recorded N take(s)" when a recording ended.
    pub message: Option<StatusMessage>,
    /// The service's answer (`stop()`); `{"stopped": true, "was_playing": false}` without a
    /// transport.
    pub status: Value,
}

/// Plays from `start_beat`, looping the loop range when `looped`; the player's status.
///
/// # Errors
///
/// `NO_PROJECT`, `ENGINE_UNAVAILABLE`, and the player's errors (`DEVICE_ERROR`,
/// `INVALID_ARGUMENT` "loop range is too short", the graph's).
pub fn play(app: &AppContext, start_beat: f64, looped: bool) -> Result<Value, ErrorNotice> {
    app.transport()
        .play(Some(start_beat), None, looped)
        .map_err(|error| ErrorNotice::from(&error))
}

/// Stops playing or recording; a recording that ended is reported as "Recorded N
/// take(s)". Without a transport nothing plays, and none is created.
///
/// # Errors
///
/// The errors of finishing a recording; the transport has stopped either way.
pub fn stop(app: &AppContext) -> Result<Stopped, ErrorNotice> {
    if !app.has_transport() {
        return Ok(Stopped {
            was_playing: false,
            takes: None,
            message: None,
            status: serde_json::json!({"stopped": true, "was_playing": false}),
        });
    }
    let status = app
        .transport()
        .stop()
        .map_err(|error| ErrorNotice::from(&error))?;
    let takes = status.get("recording").map(|recording| {
        recording
            .get("takes")
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    });
    Ok(Stopped {
        was_playing: status
            .get("was_playing")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        takes,
        message: takes.map(|count| StatusMessage::info(format!("Recorded {count} take(s)"))),
        status,
    })
}

/// Records the armed tracks from `start_beat`.
///
/// # Errors
///
/// `ENGINE_UNAVAILABLE`, `NO_PROJECT`, `INVALID_ARGUMENT` "no armed audio or instrument tracks
/// to record", `DEVICE_ERROR`, and the errors of creating the take files and of starting.
pub fn record(app: &AppContext, start_beat: f64) -> Result<Value, ErrorNotice> {
    app.transport()
        .record(Some(start_beat), None)
        .map_err(|error| ErrorNotice::from(&error))
}

/// Moves the transport to `beat`; the new position.
///
/// # Errors
///
/// `ENGINE_UNAVAILABLE`, then `NO_PROJECT`.
pub fn seek(app: &AppContext, beat: f64) -> Result<Value, ErrorNotice> {
    app.transport()
        .seek(beat)
        .map_err(|error| ErrorNotice::from(&error))
}

/// The engine label's status (`transport.status()`): `{"available": false, "backend",
/// "reason"}` without a usable backend, else the backend and the player's status. `None` before
/// the transport exists, which this never creates.
#[must_use]
pub fn engine_status(app: &AppContext) -> Option<Value> {
    app.has_transport().then(|| app.transport().status())
}

/// At startup, opens the MIDI inputs for live playing when the audio backend is not
/// `null` (neither the launch option nor the settings) and input ports exist. Returns whether
/// they were opened; no ports, or a MIDI system that cannot be used, opens nothing.
///
/// # Errors
///
/// The errors of opening them: `ENGINE_UNAVAILABLE`, and those of starting the player.
pub fn start_live_midi(app: &AppContext) -> Result<bool, ErrorNotice> {
    let disabled = app.options().audio_backend.as_deref() == Some("null")
        || app.settings().audio.backend == "null";
    if disabled {
        return Ok(false);
    }
    match app.midi_devices().input_ports() {
        Ok(ports) if !ports.is_empty() => {}
        Ok(_) => return Ok(false),
        Err(error) => {
            log::info!("MIDI input not started: {error}");
            return Ok(false);
        }
    }
    app.transport()
        .enable_midi(None)
        .map(|_| true)
        .map_err(|error| ErrorNotice::from(&error))
}
