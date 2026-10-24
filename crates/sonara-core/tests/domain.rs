//! The project model follows the prototype (`tests/unit/test_domain.py`): validation rules
//! and their messages, normalization, lookups and copy-on-write edits. Expected messages were
//! taken from the prototype.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::json;
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::ids::{IdGenerator, RandomIds, SequentialIds, id_prefix, is_valid_id};
use sonara_core::midi::{ControlKind, MidiControlEvent, MidiNote, note_name, parse_note_name};
use sonara_core::model::automation::{
    AutomationLane, AutomationLaneData, AutomationPoint, AutomationTarget, CurveKind,
};
use sonara_core::model::clip::{AudioClip, Clip, Fade, FadeCurve, MidiClip, MidiClipData};
use sonara_core::model::device::{Device, ParamValue, Params};
use sonara_core::model::markers::{Marker, Section};
use sonara_core::model::media::{MediaAsset, MediaAssetData, MediaKind, MediaLocation};
use sonara_core::model::project::{Project, ProjectSettings, TransportSettings};
use sonara_core::model::track::{
    InputSettings, MixerSettings, MonitorMode, Send, Track, TrackData, TrackKind, db_to_gain,
    gain_to_db,
};
use sonara_core::model::{JsonMap, validate_color};

const INVALID: ErrorCode = ErrorCode::InvalidArgument;

fn assert_error<T: std::fmt::Debug>(
    result: Result<T, DomainError>,
    code: ErrorCode,
    message: &str,
) {
    let err = result.expect_err(message);
    assert_eq!((err.code, err.message.as_str()), (code, message));
}

fn note(id: &str, pitch: i64, start: f64, length: f64) -> MidiNote {
    MidiNote::new(id, pitch, start, length, 100, 0).expect("valid note")
}

fn control(beat: f64, kind: ControlKind, value: i64, number: i64) -> MidiControlEvent {
    MidiControlEvent::new(beat, kind, value, number, 0).expect("valid event")
}

fn midi_data(id: &str, start: f64, length: f64, notes: Vec<MidiNote>) -> MidiClipData {
    MidiClipData {
        id: id.to_owned(),
        name: "m".to_owned(),
        start,
        length,
        notes: Arc::new(notes),
        controls: Arc::new(Vec::new()),
        content_offset: 0.0,
        loop_length: None,
        muted: false,
        color: None,
        extra: JsonMap::new(),
    }
}

fn track(ids: &mut SequentialIds, name: &str, kind: TrackKind) -> Track {
    Track::create(ids, name, kind, |_| {}).expect("valid track")
}

fn device(ids: &mut SequentialIds, plugin_id: &str, name: &str) -> Device {
    Device::create(ids, plugin_id, name, Params::new(), None).expect("valid device")
}

fn asset_data(id: &str, path: &str, frames: u64) -> MediaAssetData {
    MediaAssetData {
        id: id.to_owned(),
        kind: MediaKind::Audio,
        name: "a.wav".to_owned(),
        path: path.to_owned(),
        location: MediaLocation::Embedded,
        size_bytes: 10,
        sha256: None,
        sample_rate: 48_000,
        channels: 2,
        frames,
        format: String::new(),
        subtype: String::new(),
        original_path: None,
        relative_hint: None,
        metadata: JsonMap::new(),
        extra: JsonMap::new(),
    }
}

/// `(timeline beat rounded to 6 digits, note ID)` pairs, like the prototype's assertions.
fn heard(clip: &MidiClip) -> Vec<(f64, String)> {
    clip.playback_notes()
        .into_iter()
        .map(|(t, n)| ((t * 1e6).round() / 1e6, n.id.clone()))
        .collect()
}

fn heard_ids(pairs: &[(f64, &str)]) -> Vec<(f64, String)> {
    pairs.iter().map(|(t, id)| (*t, (*id).to_owned())).collect()
}

// -- colors and IDs ---------------------------------------------------------------------------

#[test]
fn validate_colors() {
    validate_color(None).expect("automatic color");
    validate_color(Some("#a1B2c3")).expect("valid color");
    for bad in ["red", "#12345", "#1234567", "123456#", "#gggggg"] {
        assert_error(
            validate_color(Some(bad)),
            INVALID,
            &format!("color must be '#rrggbb', got '{bad}'"),
        );
    }
}

/// The prototype also checks that leaving `deterministic_ids()` restores random IDs, which is
/// Python context-manager mechanics; Rust passes the generator explicitly.
#[test]
fn ids_random_and_deterministic() {
    let mut random = RandomIds;
    let (a, b) = (random.new_id("trk"), random.new_id("trk"));
    assert!(a != b && a.starts_with("trk_") && a.len() == 16, "{a} {b}");
    assert!(is_valid_id(&a));
    assert_eq!(id_prefix(&a), "trk");
    let mut ids = SequentialIds::new(0);
    assert_eq!(
        [ids.new_id("trk"), ids.new_id("clp")],
        ["trk_000000000001", "clp_000000000002"]
    );
    assert_eq!(SequentialIds::new(10).new_id("n"), "n_000000000011");
}

#[test]
fn valid_ids() {
    for value in ["master", "trk_abc", "op_a-b"] {
        assert!(is_valid_id(value), "{value}");
    }
}

#[test]
fn invalid_ids() {
    let long = format!("trk_{}", "a".repeat(65));
    for value in [
        "",
        "trk",
        "_abc",
        "trk_",
        "tr k_abc",
        "trk_a b",
        long.as_str(),
    ] {
        assert!(!is_valid_id(value), "{value}");
    }
}

// -- MIDI -------------------------------------------------------------------------------------

#[test]
fn note_validation() {
    let mut ids = SequentialIds::new(0);
    let n = MidiNote::create(&mut ids, 60, 1.0, 0.5, 100, 0).expect("valid");
    assert_eq!(n.id, "n_000000000001");
    assert!((n.end() - 1.5).abs() < f64::EPSILON);
    assert_eq!(n.moved(1.0, 2).expect("valid").pitch, 62);
    assert!(n.moved(-5.0, 0).expect("valid").start.abs() < f64::EPSILON);
    // (pitch, start, length, velocity, channel) changes of a valid note, and the message.
    let cases = [
        ((128, 0.0, 1.0, 100, 0), "note pitch must be 0-127, got 128"),
        ((-1, 0.0, 1.0, 100, 0), "note pitch must be 0-127, got -1"),
        ((60, 0.0, 1.0, 0, 0), "note velocity must be 1-127, got 0"),
        (
            (60, 0.0, 1.0, 128, 0),
            "note velocity must be 1-127, got 128",
        ),
        ((60, 0.0, 1.0, 100, 16), "MIDI channel must be 0-15"),
        ((60, -1.0, 1.0, 100, 0), "note start must be >= 0"),
        ((60, 0.0, 0.0, 100, 0), "note length must be > 0"),
        (
            (60, 0.0, f64::NAN, 100, 0),
            "note length must be a finite number",
        ),
    ];
    for ((pitch, start, length, velocity, channel), message) in cases {
        assert_error(
            MidiNote::new("n_1", pitch, start, length, velocity, channel),
            INVALID,
            message,
        );
    }
}

#[test]
fn control_event_validation() {
    MidiControlEvent::new(0.0, ControlKind::PitchBend, -8192, 0, 0).expect("lowest bend");
    MidiControlEvent::new(0.0, ControlKind::PitchBend, 8191, 0, 0).expect("highest bend");
    MidiControlEvent::new(0.0, ControlKind::Cc, 127, 127, 0).expect("highest CC");
    assert_error(
        MidiControlEvent::new(0.0, ControlKind::PitchBend, 8192, 0, 0),
        INVALID,
        "pitch bend must be -8192..8191",
    );
    assert_error(
        MidiControlEvent::new(0.0, ControlKind::Cc, 128, 1, 0),
        INVALID,
        "controller value must be 0-127",
    );
    assert_error(
        MidiControlEvent::new(0.0, ControlKind::Cc, 1, 128, 0),
        INVALID,
        "CC number must be 0-127",
    );
    assert_error(
        MidiControlEvent::new(-1.0, ControlKind::Cc, 1, 0, 0),
        INVALID,
        "controller event beat must be >= 0",
    );
}

#[test]
fn parse_note_names() {
    for (name, pitch) in [
        ("C4", 60),
        ("c4", 60),
        ("A4", 69),
        ("C-1", 0),
        ("G9", 127),
        ("F#3", 54),
        ("Gb3", 54),
        ("bb2", 46),
        ("Cb4", 59),
        ("B#3", 60),
        ("E#4", 65),
        (" D5 ", 74),
    ] {
        assert_eq!(parse_note_name(name).expect(name), pitch, "{name}");
    }
}

#[test]
fn parse_note_name_rejects() {
    for (name, message) in [
        ("", "invalid note name ''"),
        ("H4", "invalid note name 'H4'"),
        ("C", "invalid note name 'C'"),
        ("Cx4", "invalid note name 'Cx4'"),
        ("G#9", "note 'G#9' is outside the MIDI range"),
        ("C-2", "note 'C-2' is outside the MIDI range"),
        ("C4.5", "invalid note name 'C4.5'"),
    ] {
        assert_error(parse_note_name(name), INVALID, message);
    }
}

#[test]
fn note_name_round_trip() {
    assert_eq!(note_name(60), "C4");
    assert_eq!(note_name(0), "C-1");
    for pitch in 0..=127 {
        assert_eq!(parse_note_name(&note_name(pitch)).expect("valid"), pitch);
    }
}

// -- clips ------------------------------------------------------------------------------------

#[test]
fn audio_clip_validation() {
    let mut ids = SequentialIds::new(0);
    let c = AudioClip::create(&mut ids, "a", "med_1", 0.0, 4.0, 0).expect("valid");
    assert!((c.start + c.length - 4.0).abs() < f64::EPSILON);
    let moved = Clip::Audio(c.clone()).moved_to(2.0).expect("valid");
    assert!((moved.start() - 2.0).abs() < f64::EPSILON);
    assert_error(
        c.edit(|d| d.length = 0.0),
        INVALID,
        "clip length must be > 0",
    );
    assert_error(
        c.edit(|d| d.start = -1.0),
        INVALID,
        "clip start must be >= 0",
    );
    assert_error(
        c.edit(|d| d.source_offset = -1),
        INVALID,
        "source offset must be >= 0",
    );
    assert_error(
        c.edit(|d| d.gain_db = 30.0),
        INVALID,
        "clip gain must be within -96..+24 dB",
    );
    assert_error(
        c.edit(|d| d.color = Some("blue".into())),
        INVALID,
        "color must be '#rrggbb', got 'blue'",
    );
    assert_error(
        Fade::new(-1.0, FadeCurve::Linear),
        INVALID,
        "fade length must be >= 0",
    );
}

#[test]
fn midi_clip_sorts_notes_and_controls() {
    let notes = vec![
        note("n_b", 64, 2.0, 1.0),
        note("n_a", 60, 0.0, 1.0),
        note("n_c", 62, 0.0, 1.0),
    ];
    let mut data = midi_data("clp_1", 0.0, 4.0, notes);
    data.controls = Arc::new(vec![
        control(3.0, ControlKind::Cc, 1, 1),
        control(1.0, ControlKind::Cc, 2, 1),
    ]);
    let clip = MidiClip::new(data).expect("valid");
    let ids: Vec<&str> = clip.notes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["n_a", "n_c", "n_b"]);
    let beats: Vec<f64> = clip.controls.iter().map(|e| e.beat).collect();
    assert_eq!(beats, [1.0, 3.0]);
    assert!(clip.note("n_b").is_some() && clip.note("zz").is_none());
    assert!((clip.content_end() - 3.0).abs() < f64::EPSILON);
    assert_error(
        clip.edit(|d| d.loop_length = Some(0.0)),
        INVALID,
        "loop length must be > 0",
    );
    assert_error(
        clip.edit(|d| d.content_offset = -1.0),
        INVALID,
        "content offset must be >= 0",
    );
}

/// The prototype's `_clip`: notes `n_<i>` with pitch `60 + i`.
fn clip(
    start: f64,
    length: f64,
    notes: &[(f64, f64)],
    change: impl FnOnce(&mut MidiClipData),
) -> MidiClip {
    let notes = (0_i64..)
        .zip(notes)
        .map(|(i, (s, len))| note(&format!("n_{i}"), 60 + i, *s, *len))
        .collect();
    let mut data = midi_data("clp_1", start, length, notes);
    change(&mut data);
    MidiClip::new(data).expect("valid clip")
}

#[test]
fn playback_notes_plain_and_trimmed() {
    let plain = clip(
        8.0,
        4.0,
        &[(0.0, 1.0), (1.5, 0.5), (3.9, 0.5), (4.0, 1.0)],
        |_| {},
    );
    assert_eq!(
        heard(&plain),
        heard_ids(&[(8.0, "n_0"), (9.5, "n_1"), (11.9, "n_2")])
    );
    let trimmed = plain
        .edit(|d| {
            d.content_offset = 1.0;
            d.length = 3.0;
        })
        .expect("valid");
    // Notes before the content offset are dropped; the rest shift left.
    assert_eq!(heard(&trimmed), heard_ids(&[(8.5, "n_1"), (10.9, "n_2")]));
}

#[test]
fn playback_notes_loop() {
    let looped = clip(4.0, 8.0, &[(0.0, 0.5), (1.0, 0.5), (2.5, 0.5)], |d| {
        d.loop_length = Some(2.0);
    });
    // The note at 2.5 is outside the loop content and never plays.
    assert_eq!(
        heard(&looped),
        heard_ids(&[
            (4.0, "n_0"),
            (5.0, "n_1"),
            (6.0, "n_0"),
            (7.0, "n_1"),
            (8.0, "n_0"),
            (9.0, "n_1"),
            (10.0, "n_0"),
            (11.0, "n_1"),
        ])
    );
}

#[test]
fn playback_notes_loop_with_offset() {
    let looped = clip(0.0, 3.0, &[(0.0, 0.5), (1.0, 0.5)], |d| {
        d.loop_length = Some(2.0);
        d.content_offset = 1.0;
    });
    // Starts mid-loop at content beat 1, so: n_1 at 0, n_0 at 1, n_1 at 2.
    assert_eq!(
        heard(&looped),
        heard_ids(&[(0.0, "n_1"), (1.0, "n_0"), (2.0, "n_1")])
    );
}

// -- automation -------------------------------------------------------------------------------

#[test]
fn automation_target_validation() {
    AutomationTarget::new(None, "volume_db").expect("volume");
    AutomationTarget::new(None, "pan").expect("pan");
    let t = AutomationTarget::new(None, "send.snd_1.level_db").expect("send level");
    assert_eq!(t.send_id(), Some("snd_1"));
    let device = AutomationTarget::new(Some("dev_1".into()), "anything").expect("device");
    assert_eq!(device.send_id(), None);
    assert_eq!(
        AutomationTarget::new(Some("dev_1".into()), "cutoff_hz")
            .expect("device")
            .to_string(),
        "dev_1.cutoff_hz"
    );
    for (bad, message) in [
        ("width", "unknown mixer automation parameter 'width'"),
        (
            "send.snd_1",
            "unknown mixer automation parameter 'send.snd_1'",
        ),
        (
            "send..level_db.x",
            "unknown mixer automation parameter 'send..level_db.x'",
        ),
        ("", "automation target parameter must not be empty"),
    ] {
        assert_error(AutomationTarget::new(None, bad), INVALID, message);
    }
}

fn lane(points: &[(f64, f64, CurveKind)]) -> AutomationLane {
    AutomationLane::new(AutomationLaneData {
        id: "aut_1".to_owned(),
        target: AutomationTarget::new(None, "volume_db").expect("volume"),
        points: points
            .iter()
            .map(|&(b, v, c)| AutomationPoint::new(b, v, c).expect("valid point"))
            .collect(),
        enabled: true,
    })
    .expect("valid lane")
}

/// The prototype also evaluates the lanes (`value_at`). Lane evaluation is not part of
/// `sonara-core`: it belongs to the render path. The model's own guarantees are ported:
/// points are sorted by beat, a vertical jump keeps its order, and points are validated.
#[test]
fn lane_single_point_unsorted_and_jumps() {
    let unsorted = lane(&[(4.0, 1.0, CurveKind::Linear), (0.0, 0.0, CurveKind::Linear)]);
    let beats: Vec<f64> = unsorted.points.iter().map(|p| p.beat).collect();
    assert_eq!(beats, [0.0, 4.0]);
    let jump = lane(&[
        (0.0, 0.0, CurveKind::Linear),
        (2.0, 0.0, CurveKind::Linear),
        (2.0, 10.0, CurveKind::Linear),
        (4.0, 10.0, CurveKind::Linear),
    ]);
    let values: Vec<(f64, f64)> = jump.points.iter().map(|p| (p.beat, p.value)).collect();
    assert_eq!(values, [(0.0, 0.0), (2.0, 0.0), (2.0, 10.0), (4.0, 10.0)]);
    assert_error(
        AutomationPoint::new(-1.0, 0.0, CurveKind::Linear),
        INVALID,
        "automation point beat must be >= 0",
    );
}

// -- tracks, mixer, devices -------------------------------------------------------------------

#[test]
fn mixer_and_send_ranges() {
    MixerSettings::new(12.0, -1.0, false, false, false).expect("limits are valid");
    for (volume_db, pan, message) in [
        (13.0, 0.0, "volume must be within -144..+12 dB"),
        (-200.0, 0.0, "volume must be within -144..+12 dB"),
        (0.0, 1.5, "pan must be within -1 (left) .. +1 (right)"),
        (0.0, f64::NAN, "pan must be a finite number"),
    ] {
        assert_error(
            MixerSettings::new(volume_db, pan, false, false, false),
            INVALID,
            message,
        );
    }
    assert_error(
        Send::new("snd_1", "trk_1", 13.0, false, true),
        INVALID,
        "send level must be within -144..+12 dB",
    );
    assert_eq!(
        [
            db_to_gain(-200.0),
            db_to_gain(0.0),
            gain_to_db(1.0),
            gain_to_db(0.0)
        ],
        [0.0, 1.0, 0.0, f64::NEG_INFINITY]
    );
    for channels in [&[][..], &[0, 1, 2][..]] {
        assert_error(
            InputSettings::new(channels, false, MonitorMode::Auto),
            INVALID,
            "track input uses 1 or 2 device channels",
        );
    }
}

#[test]
fn track_invariants() {
    let mut ids = SequentialIds::new(0);
    let audio = Clip::Audio(AudioClip::create(&mut ids, "a", "med_1", 0.0, 4.0, 0).expect("ok"));
    let midi = Clip::Midi(MidiClip::create(&mut ids, "m", 0.0, 4.0, Vec::new()).expect("ok"));
    let with_clip = |ids: &mut SequentialIds, name: &str, kind: TrackKind, clip: &Clip| {
        Track::create(ids, name, kind, |d| d.clips = vec![clip.clone()])
    };
    with_clip(&mut ids, "A", TrackKind::Audio, &audio).expect("audio on audio");
    with_clip(&mut ids, "I", TrackKind::Instrument, &midi).expect("MIDI on instrument");
    assert_error(
        with_clip(&mut ids, "I", TrackKind::Instrument, &audio),
        INVALID,
        "instrument track cannot hold audio clips",
    );
    assert_error(
        with_clip(&mut ids, "A", TrackKind::Audio, &midi),
        INVALID,
        "audio track cannot hold MIDI clips",
    );
    assert_error(
        with_clip(&mut ids, "B", TrackKind::Bus, &midi),
        INVALID,
        "bus track cannot hold MIDI clips",
    );
    let synth = device(&mut ids, "sonara.synth.poly", "Synth");
    assert_error(
        Track::create(&mut ids, "A", TrackKind::Audio, |d| {
            d.instrument = Some(synth);
        }),
        INVALID,
        "only instrument tracks have an instrument",
    );
    assert_error(
        Track::create(&mut ids, "  ", TrackKind::Audio, |_| {}),
        INVALID,
        "track name must not be empty",
    );
    // The master must not have an output.
    assert_error(
        Track::new(TrackData::new("master", "M", TrackKind::Master)),
        INVALID,
        "the master track has no output target",
    );
    let utility = device(&mut ids, "sonara.utility", "U");
    assert_error(
        Track::create(&mut ids, "F", TrackKind::Folder, |d| {
            d.inserts = vec![utility];
        }),
        INVALID,
        "folder tracks do not process audio",
    );
}

#[test]
fn track_edits() {
    let mut ids = SequentialIds::new(0);
    let a = AudioClip::create(&mut ids, "a", "med_1", 4.0, 4.0, 0).expect("valid");
    let b = AudioClip::create(&mut ids, "b", "med_1", 0.0, 4.0, 0).expect("valid");
    let t = Track::create(&mut ids, "A", TrackKind::Audio, |d| {
        d.clips = vec![a.clone().into(), b.clone().into()];
    })
    .expect("valid");
    let order: Vec<&str> = t.clips.iter().map(Clip::id).collect();
    assert_eq!(order, [b.id.as_str(), a.id.as_str()]);
    assert!((t.content_end() - 8.0).abs() < f64::EPSILON);
    assert_eq!(t.clip(&a.id).expect("found"), &Clip::Audio(a.clone()));
    let missing = DomainError::not_found("clip", "clp_missing");
    assert_eq!(t.clip("clp_missing").expect_err("missing"), missing);
    assert_eq!(
        missing.details,
        *json!({"kind": "clip", "id": "clp_missing"})
            .as_object()
            .expect("object")
    );
    let moved = a.edit(|d| d.start = 10.0).expect("valid");
    let t2 = t.with_clip(moved.into()).expect("valid");
    assert!((t2.clip(&a.id).expect("found").start() - 10.0).abs() < f64::EPSILON);
    assert_eq!(t2.clips.len(), 2);
    assert_eq!(t2.without_clip(&a.id).expect("valid").clips.len(), 1);
    assert_error(
        t.without_clip("clp_x"),
        ErrorCode::NotFound,
        "clip 'clp_x' not found",
    );
    let eq = device(&mut ids, "sonara.eq.parametric", "EQ");
    let comp = device(&mut ids, "sonara.dynamics.compressor", "Comp");
    let t3 = t
        .with_insert(eq.clone(), None)
        .and_then(|t| t.with_insert(comp.clone(), Some(0)))
        .expect("valid");
    let inserts: Vec<&str> = t3.inserts.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(inserts, [comp.id.as_str(), eq.id.as_str()]);
    let t4 = t3
        .with_device(eq.with_param("output_db", 3.0.into()).expect("valid"))
        .expect("valid");
    let output = &t4.find_device(&eq.id).expect("found").params["output_db"];
    assert_eq!(output, &ParamValue::Float(3.0));
    let other = device(&mut ids, "x.y", "X");
    assert_eq!(other.id, "dev_000000000006");
    assert_error(
        t4.with_device(other),
        ErrorCode::NotFound,
        "device 'dev_000000000006' not found",
    );
    let target = AutomationTarget::new(Some(eq.id.clone()), "output_db").expect("valid");
    let point = AutomationPoint::new(0.0, 1.0, CurveKind::Linear).expect("valid");
    let lane = AutomationLane::create(&mut ids, target, vec![point]).expect("valid");
    let t5 = t4.with_lane(lane.clone()).expect("valid");
    assert_eq!(t5.lane_for(Some(&eq.id), "output_db"), Some(&lane));
    let t6 = t5.without_device(&eq.id).expect("valid");
    assert!(t6.find_device(&eq.id).is_none() && t6.automation.is_empty());
}

#[test]
fn instrument_track_device_order() {
    let mut ids = SequentialIds::new(0);
    let synth = device(&mut ids, "sonara.synth.poly", "Synth");
    let eq = device(&mut ids, "sonara.eq.parametric", "EQ");
    let t = Track::create(&mut ids, "I", TrackKind::Instrument, |d| {
        d.instrument = Some(synth.clone());
        d.inserts = vec![eq.clone()];
    })
    .expect("valid");
    let order: Vec<&str> = t.devices().map(|d| d.id.as_str()).collect();
    assert_eq!(order, [synth.id.as_str(), eq.id.as_str()]);
    assert!(
        t.without_device(&synth.id)
            .expect("valid")
            .instrument
            .is_none()
    );
}

#[test]
fn device_validation() {
    let mut ids = SequentialIds::new(0);
    let base = device(&mut ids, "p", "x");
    assert_error(
        base.edit(|d| d.plugin_id = String::new()),
        INVALID,
        "device plugin_id must not be empty",
    );
    assert_error(
        base.edit(|d| {
            d.params.insert("a".into(), f64::INFINITY.into());
        }),
        INVALID,
        "parameter 'a' must be finite",
    );
    let mut params = Params::new();
    params.insert("a".into(), 1.0.into());
    let d = Device::create(&mut ids, "p", "x", params, None).expect("valid");
    let d2 = d
        .edit(|data| data.preset = Some("Init".into()))
        .and_then(|d| d.with_params([("b".to_owned(), true.into())]))
        .expect("valid");
    assert_eq!(d2.preset, None);
    let expected: Params = [
        ("a".to_owned(), ParamValue::Float(1.0)),
        ("b".to_owned(), ParamValue::Bool(true)),
    ]
    .into_iter()
    .collect();
    assert_eq!(d2.params, expected);
}

// -- media, markers, project ------------------------------------------------------------------

#[test]
fn media_asset_validation() {
    let ok = MediaAsset::new(asset_data("med_1", "media/audio/a.wav", 96_000)).expect("valid");
    assert!((ok.duration_seconds() - 2.0).abs() < f64::EPSILON);
    for path in ["/abs/a.wav", "media/../x.wav", "other/a.wav"] {
        assert_error(
            ok.edit(|d| d.path = path.to_owned()),
            INVALID,
            &format!("embedded asset path must be relative and inside media/: '{path}'"),
        );
    }
    let mut linked = asset_data("med_2", "/abs/a.wav", 1);
    linked.location = MediaLocation::Linked;
    MediaAsset::new(linked).expect("linked assets may be absolute");
    assert_error(
        ok.edit(|d| d.sample_rate = 0),
        INVALID,
        "audio asset needs a sample rate",
    );
    let mut midi = asset_data("med_3", "media/midi/a.mid", 0);
    midi.kind = MediaKind::Midi;
    midi.name = "a.mid".to_owned();
    midi.sample_rate = 0;
    midi.channels = 0;
    MediaAsset::new(midi).expect("MIDI assets have no sample rate");
}

#[test]
fn markers_and_sections() {
    let mut ids = SequentialIds::new(0);
    Marker::create(&mut ids, 0.0, "start").expect("valid");
    assert_error(
        Marker::create(&mut ids, -1.0, "x"),
        INVALID,
        "marker beat must be >= 0",
    );
    let s = Section::create(&mut ids, "Intro", 0.0, 16.0).expect("valid");
    assert!((s.end() - 16.0).abs() < f64::EPSILON);
    assert_error(
        Section::create(&mut ids, "", 0.0, 4.0),
        INVALID,
        "section name must not be empty",
    );
    assert_error(
        Section::create(&mut ids, "x", 0.0, 0.0),
        INVALID,
        "section length must be > 0",
    );
}

#[test]
fn project_settings_and_transport() {
    assert_error(
        ProjectSettings::new(12_345, None, "PCM_24"),
        INVALID,
        "sample rate must be one of (22050, 32000, 44100, 48000, 88200, 96000, 176400, 192000)",
    );
    assert_error(
        ProjectSettings::new(48_000, Some(0.0), "PCM_24"),
        INVALID,
        "project length must be > 0",
    );
    let empty_loop = TransportSettings {
        loop_start: 4.0,
        loop_end: 4.0,
        ..TransportSettings::default()
    };
    assert_error(
        empty_loop.validate(),
        INVALID,
        "loop start must be >= 0 and before loop end",
    );
    let long_count_in = TransportSettings {
        count_in_bars: 5,
        ..TransportSettings::default()
    };
    assert_error(
        long_count_in.validate(),
        INVALID,
        "count-in must be 0-4 bars",
    );
    assert_error(
        TransportSettings::count_in(5),
        INVALID,
        "count-in must be 0-4 bars",
    );
}

#[test]
fn project_lookup_and_edits() {
    let mut ids = SequentialIds::new(0);
    let p = Project::create(&mut ids, "P", 48_000, 90.0).expect("valid");
    assert!((p.tempo_map.points()[0].bpm - 90.0).abs() < f64::EPSILON);
    let a = track(&mut ids, "A", TrackKind::Audio);
    let b = track(&mut ids, "B", TrackKind::Bus);
    let p = p
        .adding_track(a.clone(), None)
        .and_then(|p| p.adding_track(b.clone(), Some(0)))
        .expect("valid");
    let order: Vec<&str> = p.tracks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(order, [b.id.as_str(), a.id.as_str()]);
    assert!(p.track("master").expect("master").ptr_eq(&p.master));
    assert!(p.find_track("trk_nope").is_none());
    assert_error(
        p.track("trk_nope"),
        ErrorCode::NotFound,
        "track 'trk_nope' not found",
    );
    assert_error(
        p.adding_track(a.clone(), None),
        ErrorCode::Conflict,
        "track 'trk_000000000002' already exists",
    );
    assert_error(
        p.removing_track("master"),
        INVALID,
        "the master track cannot be deleted",
    );
    let buses: Vec<&str> = p.buses().map(|t| t.id.as_str()).collect();
    assert_eq!(buses, [b.id.as_str()]);
    let main = p.master.edit(|d| d.name = "Main".into()).expect("valid");
    assert_eq!(p.with_track(main).expect("valid").master.name, "Main");
    assert_eq!(p.removing_track(&a.id).expect("valid").tracks.len(), 1);
    assert_error(
        p.edit(|d| d.tracks = vec![a.clone(), a.clone()]),
        ErrorCode::Conflict,
        "duplicate track id 'trk_000000000002'",
    );
    assert_error(
        p.edit(|d| d.name = " ".into()),
        INVALID,
        "project name must not be empty",
    );
    let mut second_master = TrackData::new("master", "M2", TrackKind::Master);
    second_master.output_id = None;
    let second_master = Track::new(second_master).expect("a valid master");
    assert_error(
        p.edit(|d| d.tracks = vec![second_master]),
        INVALID,
        "only one master track is allowed",
    );
}

#[test]
fn project_content_end_and_assets() {
    let mut ids = SequentialIds::new(0);
    let p = Project::create(&mut ids, "P", 48_000, 120.0).expect("valid");
    let mut data = asset_data("med_1", "media/audio/a.wav", 48_000);
    data.size_bytes = 1;
    data.channels = 1;
    let asset = MediaAsset::new(data).expect("valid");
    let clip = AudioClip::create(&mut ids, "a", &asset.id, 4.0, 8.0, 0).expect("valid");
    let t = Track::create(&mut ids, "A", TrackKind::Audio, |d| {
        d.clips = vec![clip.clone().into()];
    })
    .expect("valid");
    let conv = device(&mut ids, "sonara.convolver", "IR")
        .edit(|d| {
            d.state.insert("ir_asset_id".into(), "med_2".into());
        })
        .expect("valid");
    let master = p
        .master
        .edit(|d| d.inserts = vec![conv.clone()])
        .expect("valid");
    let p = p
        .edit(|d| {
            d.tracks = vec![t.clone()];
            d.master = master;
        })
        .and_then(|p| p.with_asset(asset.clone()))
        .expect("valid");
    assert!((p.content_end() - 12.0).abs() < f64::EPSILON);
    assert!((p.end_beat() - 12.0).abs() < f64::EPSILON);
    let long = p
        .edit(|d| d.settings = ProjectSettings::new(48_000, Some(64.0), "PCM_24").expect("ok"))
        .expect("valid");
    assert!((long.end_beat() - 64.0).abs() < f64::EPSILON);
    assert_eq!(
        p.referenced_asset_ids(),
        BTreeSet::from(["med_1".to_owned(), "med_2".to_owned()])
    );
    assert_eq!(p.asset("med_1").expect("found"), &asset);
    assert_error(
        p.asset("med_x"),
        ErrorCode::NotFound,
        "asset 'med_x' not found",
    );
    let (found_track, found_clip) = p.find_clip(&clip.id).expect("found");
    assert_eq!((found_track, found_clip), (&t, &Clip::Audio(clip.clone())));
    assert_eq!(p.device(&conv.id).expect("found").0.id, "master");
    assert_error(
        p.clip("clp_x"),
        ErrorCode::NotFound,
        "clip 'clp_x' not found",
    );
    assert_error(
        p.device("dev_x"),
        ErrorCode::NotFound,
        "device 'dev_x' not found",
    );
    assert_eq!(p.beat_to_frame(1.0), 24_000);
    let section = Section::create(&mut ids, "S", 0.0, 32.0).expect("valid");
    let with_section = p.edit(|d| d.sections = vec![section]).expect("valid");
    assert!((with_section.content_end() - 32.0).abs() < f64::EPSILON);
}

#[test]
fn project_sorts_markers_and_sections() {
    let mut ids = SequentialIds::new(0);
    let m1 = Marker::new("mrk_b", 8.0, "b", None).expect("valid");
    let m2 = Marker::new("mrk_a", 2.0, "a", None).expect("valid");
    let s1 = Section::new("sec_b", "B", 8.0, 4.0, None).expect("valid");
    let s2 = Section::new("sec_a", "A", 0.0, 8.0, None).expect("valid");
    let p = Project::create(&mut ids, "P", 48_000, 120.0)
        .and_then(|p| {
            p.edit(|d| {
                d.markers = vec![m1, m2.clone()];
                d.sections = vec![s1, s2];
            })
        })
        .expect("valid");
    let markers: Vec<&str> = p.markers.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(markers, ["mrk_a", "mrk_b"]);
    let sections: Vec<&str> = p.sections.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(sections, ["sec_a", "sec_b"]);
    assert_eq!(p.find_marker("mrk_a"), Some(&m2));
    assert!(p.find_section("sec_x").is_none());
}
