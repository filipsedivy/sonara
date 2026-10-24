//! The transport's JSON (CTR-01): the prototype's position, status, stop and pause answers, the
//! meters and the device reports, with its keys in its order. The values come rounded from the
//! player (`py_round`), so this only arranges them.

use serde_json::{Map, Value, json};
use sonara_engine::player::{DeviceReport, Paused, PlayerStatus, Position, TrackMeter};

/// `_position_json`: `position_beat`, `position_seconds` and `bar_beat`, or `position_frame`
/// without a project.
#[must_use]
pub fn position_json(position: &Position) -> Map<String, Value> {
    let mut out = Map::new();
    match position {
        Position::Musical {
            beat,
            seconds,
            bar,
            beat_in_bar,
            ..
        } => {
            out.insert("position_beat".to_owned(), json!(beat));
            out.insert("position_seconds".to_owned(), json!(seconds));
            out.insert("bar_beat".to_owned(), json!(format!("{bar}:{beat_in_bar}")));
        }
        Position::Frame(frame) => {
            out.insert("position_frame".to_owned(), json!(frame));
        }
    }
    out
}

/// The player's `status()`: the state, the position, the device format and the counters,
/// with `last_error` and `device_status` when set.
#[must_use]
pub fn status_json(status: &PlayerStatus) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("state".to_owned(), json!(status.state.as_str()));
    out.extend(position_json(&status.position));
    out.insert(
        "device_sample_rate".to_owned(),
        json!(status.device_sample_rate),
    );
    out.insert("buffer_frames".to_owned(), json!(status.buffer_frames));
    out.insert("queued_ms".to_owned(), json!(status.queued_ms));
    out.insert("lookahead_ms".to_owned(), json!(status.lookahead_ms));
    out.insert(
        "output_latency_estimate_ms".to_owned(),
        json!(status.output_latency_estimate_ms),
    );
    out.insert("underruns".to_owned(), json!(status.underruns));
    out.insert("callback_load".to_owned(), json!(status.callback_load));
    out.insert("render_load".to_owned(), json!(status.render_load));
    out.insert(
        "loop".to_owned(),
        json!(status.looped.map(|(start, end)| [start, end])),
    );
    out.insert("device_errors".to_owned(), json!(status.device_errors));
    if let Some(error) = &status.last_error {
        out.insert("last_error".to_owned(), json!(error));
    }
    if let Some(device) = status.device_status {
        out.insert("device_status".to_owned(), json!(device));
    }
    out
}

/// `pause()`: `{"paused": true, …position}`, or `{"paused": false, "state": …}`.
#[must_use]
pub fn paused_json(paused: &Paused) -> Value {
    let mut out = Map::new();
    out.insert("paused".to_owned(), json!(paused.paused));
    match &paused.position {
        Some(position) if paused.paused => out.extend(position_json(position)),
        _ => {
            out.insert("state".to_owned(), json!(paused.state.as_str()));
        }
    }
    Value::Object(out)
}

/// `meters()`: each track's post-fader `peak` and `rms` (linear, left and right) by track ID.
#[must_use]
pub fn meters_json(meters: &[TrackMeter]) -> Value {
    let out: Map<String, Value> = meters
        .iter()
        .map(|meter| {
            (
                meter.track_id.clone(),
                json!({"peak": meter.peak, "rms": meter.rms}),
            )
        })
        .collect();
    Value::Object(out)
}

/// `device_reports()`: each device's readouts by name, by device ID.
#[must_use]
pub fn reports_json(reports: &[DeviceReport]) -> Value {
    let out: Map<String, Value> = reports
        .iter()
        .map(|report| {
            let values: Map<String, Value> = report
                .values
                .iter()
                .map(|(name, value)| ((*name).to_owned(), json!(value)))
                .collect();
            (report.device_id.clone(), Value::Object(values))
        })
        .collect();
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sonara_engine::player::{Paused, PlayerState, PlayerStatus, Position};

    use super::{paused_json, position_json, status_json};

    fn musical() -> Position {
        Position::Musical {
            frame: 60_000,
            beat: 2.5,
            seconds: 1.25,
            bar: 1,
            beat_in_bar: 3,
        }
    }

    /// The prototype's keys, in its order (`transport/controls`).
    #[test]
    fn status_has_the_prototype_keys_in_order() {
        let status = PlayerStatus {
            state: PlayerState::Stopped,
            position: musical(),
            device_sample_rate: 48_000,
            buffer_frames: 256,
            queued_ms: 0.0,
            lookahead_ms: 80.0,
            output_latency_estimate_ms: 16.0,
            underruns: 0,
            callback_load: 0.0,
            render_load: 0.0,
            looped: None,
            device_errors: 0,
            last_error: None,
            device_status: None,
        };
        let json = status_json(&status);
        let keys: Vec<&str> = json.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "state",
                "position_beat",
                "position_seconds",
                "bar_beat",
                "device_sample_rate",
                "buffer_frames",
                "queued_ms",
                "lookahead_ms",
                "output_latency_estimate_ms",
                "underruns",
                "callback_load",
                "render_load",
                "loop",
                "device_errors",
            ]
        );
        assert_eq!(
            serde_json::to_string(&json["lookahead_ms"]).unwrap(),
            "80.0"
        );
        assert_eq!(json["bar_beat"], "1:3");
        assert_eq!(json["loop"], json!(null));
        let looped = PlayerStatus {
            looped: Some((95_960, 143_900)),
            last_error: Some("MediaError: gone".to_owned()),
            device_status: Some("device stopped"),
            ..status
        };
        let json = status_json(&looped);
        assert_eq!(json["loop"], json!([95_960, 143_900]));
        assert_eq!(json["last_error"], "MediaError: gone");
        assert_eq!(json["device_status"], "device stopped");
    }

    #[test]
    fn positions_and_pauses_answer_like_the_prototype() {
        assert_eq!(
            serde_json::Value::Object(position_json(&musical())),
            json!({"position_beat": 2.5, "position_seconds": 1.25, "bar_beat": "1:3"})
        );
        assert_eq!(
            serde_json::Value::Object(position_json(&Position::Frame(7))),
            json!({"position_frame": 7})
        );
        let not = Paused {
            paused: false,
            state: PlayerState::Stopped,
            position: None,
        };
        assert_eq!(
            paused_json(&not),
            json!({"paused": false, "state": "stopped"})
        );
        let paused = Paused {
            paused: true,
            state: PlayerState::Paused,
            position: Some(musical()),
        };
        assert_eq!(
            paused_json(&paused),
            json!({"paused": true, "position_beat": 2.5, "position_seconds": 1.25, "bar_beat": "1:3"})
        );
    }
}
