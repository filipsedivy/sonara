//! The time base reproduces the prototype's conversions (golden `timebase/`).

use serde_json::Value;
use sonara_core::time::{
    SnapMode, TempoMap, TempoPoint, TimeSignatureMap, TimeSignaturePoint, parse_position, snap,
};
use sonara_testkit::Golden;

/// Seconds and beats agree within 1e-9; libm differs across platforms in the last
/// bits.
const TOLERANCE: f64 = 1e-9;

fn f(v: &Value) -> f64 {
    v.as_f64().expect("number")
}

fn rows(v: &Value) -> &Vec<Value> {
    v.as_array().expect("array")
}

fn close(expected: f64, actual: f64, what: &str) {
    assert!(
        (expected - actual).abs() <= TOLERANCE * expected.abs().max(1.0),
        "{what}: expected {expected}, got {actual}"
    );
}

fn tempo_map(spec: &Value) -> TempoMap {
    let points = rows(&spec["points"])
        .iter()
        .map(|p| {
            TempoPoint::new(
                f(&p["beat"]),
                f(&p["bpm"]),
                p["ramp"].as_bool().expect("valid golden data"),
            )
            .expect("valid golden data")
        })
        .collect();
    TempoMap::new(points).expect("valid golden data")
}

fn meter_map(spec: &Value) -> TimeSignatureMap {
    let points = rows(&spec["points"])
        .iter()
        .map(|p| {
            TimeSignaturePoint::new(
                p["bar"].as_i64().expect("valid golden data"),
                u32::try_from(p["numerator"].as_u64().expect("valid golden data"))
                    .expect("valid golden data"),
                u32::try_from(p["denominator"].as_u64().expect("valid golden data"))
                    .expect("valid golden data"),
            )
            .expect("valid golden data")
        })
        .collect();
    TimeSignatureMap::new(points).expect("valid golden data")
}

#[test]
fn tempo_maps_match_the_prototype() {
    let golden: Value = Golden::open()
        .expect("valid golden data")
        .json("timebase/timebase.json")
        .expect("valid golden data");
    for (name, spec) in golden["tempo_maps"].as_object().expect("valid golden data") {
        let map = tempo_map(spec);
        for row in rows(&spec["bpm_at"]) {
            close(
                f(&row[1]),
                map.bpm_at(f(&row[0])),
                &format!("{name} bpm_at {}", row[0]),
            );
        }
        for row in rows(&spec["beat_to_seconds"]) {
            close(
                f(&row[1]),
                map.beat_to_seconds(f(&row[0])),
                &format!("{name} beat_to_seconds {}", row[0]),
            );
        }
        for row in rows(&spec["seconds_to_beat"]) {
            close(
                f(&row[1]),
                map.seconds_to_beat(f(&row[0])),
                &format!("{name} seconds_to_beat {}", row[0]),
            );
        }
        for (rate, table) in spec["beat_to_sample"]
            .as_object()
            .expect("valid golden data")
        {
            let rate: u32 = rate.parse().expect("valid golden data");
            for row in rows(table) {
                assert_eq!(
                    row[1].as_i64().expect("valid golden data"),
                    map.beat_to_sample(f(&row[0]), rate),
                    "{name} beat_to_sample {} @ {rate}",
                    row[0]
                );
            }
        }
        for (rate, table) in spec["sample_to_beat"]
            .as_object()
            .expect("valid golden data")
        {
            let rate: u32 = rate.parse().expect("valid golden data");
            for row in rows(table) {
                close(
                    f(&row[1]),
                    map.sample_to_beat(f(&row[0]), rate),
                    &format!("{name} sample_to_beat {}", row[0]),
                );
            }
        }
    }
}

#[test]
fn meter_maps_match_the_prototype() {
    let golden: Value = Golden::open()
        .expect("valid golden data")
        .json("timebase/timebase.json")
        .expect("valid golden data");
    for (name, spec) in golden["meter_maps"].as_object().expect("valid golden data") {
        let map = meter_map(spec);
        for row in rows(&spec["bar_to_beat"]) {
            close(
                f(&row[1]),
                map.bar_to_beat(f(&row[0])),
                &format!("{name} bar_to_beat {}", row[0]),
            );
        }
        for row in rows(&spec["beat_to_bar"]) {
            close(
                f(&row[1]),
                map.beat_to_bar(f(&row[0])),
                &format!("{name} beat_to_bar {}", row[0]),
            );
        }
        for row in rows(&spec["position"]) {
            assert_eq!(
                row[1].as_str().expect("valid golden data"),
                map.position(f(&row[0])).to_string(),
                "{name} position {}",
                row[0]
            );
        }
        for row in rows(&spec["bar_length"]) {
            close(
                f(&row[1]),
                map.bar_length(row[0].as_i64().expect("valid golden data")),
                &format!("{name} bar_length {}", row[0]),
            );
        }
        for row in rows(&spec["parse_position"]) {
            let text = row[0].as_str().expect("valid golden data");
            let outcome = &row[1];
            match parse_position(text, &map) {
                Ok(beat) => close(f(&outcome["ok"]), beat, &format!("{name} parse {text:?}")),
                Err(err) => {
                    assert_eq!(
                        outcome["error"].as_str(),
                        Some(err.code.as_str()),
                        "{name} parse {text:?}"
                    );
                    assert_eq!(
                        outcome["message"].as_str(),
                        Some(err.message.as_str()),
                        "{name} parse {text:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn snapping_matches_the_prototype() {
    let golden: Value = Golden::open()
        .expect("valid golden data")
        .json("timebase/timebase.json")
        .expect("valid golden data");
    for row in rows(&golden["snap"]) {
        let mode = match row[2].as_str().expect("valid golden data") {
            "floor" => SnapMode::Floor,
            "ceil" => SnapMode::Ceil,
            _ => SnapMode::Nearest,
        };
        close(
            f(&row[3]),
            snap(f(&row[0]), f(&row[1]), mode),
            &format!("snap {row:?}"),
        );
    }
}
