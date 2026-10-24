//! Commands, change sets, undo and the session follow the prototype
//! (`tests/unit/test_commands.py`).
//!
//! Every scenario was replayed in the prototype with `deterministic_ids()`: the labels, change
//! sets, results, error codes, messages and details asserted here are the prototype's. The
//! fixture mints IDs in the order the prototype's dataclass default factories do, so the IDs
//! match too (`CreateTrack` always takes a track and a device ID, `SetAutomation` always a lane
//! ID, and so on).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use sonara_core::commands::automation::{
    DeleteAutomationLane, SetAutomation, SetLaneEnabled, find_lane,
};
use sonara_core::commands::clip::{
    AddAudioClip, AddNotes, CreateMidiClip, DeleteClips, DeleteNotes, DuplicateClip, EditNotes,
    MoveClip, MoveClips, NoteEdit, ReplaceNotes, SetClipProperties, SetControlEvents, SplitClip,
    TrimClip,
};
use sonara_core::commands::device::{
    AddDevice, ApplyPreset, MoveDevice, RemoveDevice, SetDeviceEnabled, SetDeviceParams,
    SetDeviceState, SetInstrument, SetSidechain, params_json,
};
use sonara_core::commands::project::{
    AddAsset, AddMarker, AddSection, RemoveAsset, RemoveMarker, RemoveSection, RemoveTempoPoint,
    RemoveTimeSignature, RenameProject, ReplaceAsset, SetEditorState, SetMetadata,
    SetProjectLength, SetSampleRate, SetTempo, SetTimeSignature, SetTransportSettings,
    UpdateMarker, UpdateSection,
};
use sonara_core::commands::track::{
    AddSend, CreateTrack, DeleteTrack, MoveTrack, RemoveSend, RenameTrack, SetTrackColor,
    SetTrackInput, SetTrackMixer, SetTrackOutput, SetTrackParent, UpdateSend,
};
use sonara_core::commands::{ChangeSet, Command, Flag, ObjectKind, ObjectRef, Transaction};
use sonara_core::error::ErrorCode;
use sonara_core::ids::{self, IdGenerator, MASTER_ID, SequentialIds};
use sonara_core::midi::{ControlKind, MidiControlEvent, MidiNote};
use sonara_core::model::JsonMap;
use sonara_core::model::automation::{AutomationPoint, CurveKind};
use sonara_core::model::clip::{Fade, FadeCurve, MidiClip};
use sonara_core::model::device::ParamValue;
use sonara_core::model::media::{MediaAsset, MediaAssetData, MediaKind, MediaLocation};
use sonara_core::model::project::Project;
use sonara_core::model::track::{MonitorMode, TrackKind};
use sonara_core::plugins::builtin_registry;
use sonara_core::session::{CommandResult, EventKind, ProjectLocation, ProjectSession};

const INVALID: ErrorCode = ErrorCode::InvalidArgument;
const NOT_FOUND: ErrorCode = ErrorCode::NotFound;

// -- the fixture --------------------------------------------------------------------------------

/// A settable clock shared with the session (the prototype patches `session.now`).
#[derive(Clone, Default)]
struct Clock(Arc<AtomicU64>);

impl Clock {
    fn get(&self) -> f64 {
        f64::from_bits(self.0.load(Ordering::Relaxed))
    }

    fn set(&self, seconds: f64) {
        self.0.store(seconds.to_bits(), Ordering::Relaxed);
    }

    fn advance(&self, seconds: f64) {
        self.set(self.get() + seconds);
    }
}

fn location() -> ProjectLocation {
    ProjectLocation {
        path: None,
        workspace: PathBuf::from("/tmp/ws"),
        packaged: false,
    }
}

/// The prototype's `session` fixture: an open 120 BPM, 48 kHz project, created with
/// `deterministic_ids()` (`prj_000000000001`), so later IDs count on from 2.
struct Fx {
    session: ProjectSession,
    ids: SequentialIds,
    clock: Clock,
}

impl Fx {
    fn new() -> Self {
        Self::with("Test", 500, false)
    }

    fn with(name: &str, undo_limit: usize, saved: bool) -> Self {
        let clock = Clock::default();
        let shared = clock.clone();
        let mut session = ProjectSession::with_options(
            Arc::new(builtin_registry()),
            undo_limit,
            Box::new(move || shared.get()),
        );
        let mut ids = SequentialIds::new(0);
        let project = Project::create(&mut ids, name, 48_000, 120.0).expect("valid project");
        assert_eq!(project.id, "prj_000000000001");
        session.load(project, location(), saved);
        Self {
            session,
            ids,
            clock,
        }
    }

    fn id(&mut self, prefix: &str) -> String {
        self.ids.new_id(prefix)
    }

    fn project(&self) -> &Project {
        self.session.project().expect("a project is open")
    }

    fn exec(&mut self, cmd: &dyn Command) -> CommandResult {
        self.session
            .execute(cmd, false, "user")
            .unwrap_or_else(|err| panic!("{} failed: {err}", cmd.name()))
    }

    /// Executes `cmd` and checks its label, change set and result against the prototype's.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the expected values are json! literals"
    )]
    fn run(
        &mut self,
        cmd: &dyn Command,
        operation: &str,
        changes: Value,
        result: Value,
    ) -> JsonMap {
        let res = self.exec(cmd);
        assert!(res.committed && !res.dry_run, "{operation} committed");
        check(&res, operation, &changes, &result);
        res.result
    }

    /// The prototype's `expect`: the command fails with this code and message (and no
    /// details), and the session is untouched.
    fn fails(&mut self, cmd: &dyn Command, code: ErrorCode, message: &str) {
        self.fails_with(cmd, code, message, json!({}));
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "the expected details are json! literals"
    )]
    fn fails_with(&mut self, cmd: &dyn Command, code: ErrorCode, message: &str, details: Value) {
        let before = self.project().clone();
        let version = self.session.version();
        let err = self.session.execute(cmd, false, "user").expect_err(message);
        assert_eq!((err.code, err.message.as_str()), (code, message));
        assert_eq!(Value::Object(err.details), details, "{message}");
        assert!(self.project().ptr_eq(&before), "{message}: project changed");
        assert_eq!(
            self.session.version(),
            version,
            "{message}: version changed"
        );
    }

    // -- commands with the IDs the prototype's default factories mint ------------------------

    fn create_track(&mut self, name: &str, kind: TrackKind) -> CreateTrack {
        CreateTrack {
            name: name.to_owned(),
            kind,
            index: None,
            color: None,
            output_id: MASTER_ID.to_owned(),
            instrument_plugin: None,
            instrument_preset: None,
            parent_id: None,
            track_id: self.id(ids::TRACK),
            instrument_id: self.id(ids::DEVICE),
        }
    }

    /// Runs a `CreateTrack` and checks its change set and result.
    fn add_track(&mut self, cmd: &CreateTrack) -> String {
        let res = self.exec(cmd);
        let mut created = vec![format!("track:{}", cmd.track_id)];
        let mut result = json!({"track_id": cmd.track_id});
        if cmd.instrument_plugin.is_some() {
            created.push(format!("device:{}", cmd.instrument_id));
            result["instrument_device_id"] = cmd.instrument_id.clone().into();
        }
        let changes = json!({"created": created, "flags": ["routing", "tracks"]});
        check(&res, "Create Track", &changes, &result);
        cmd.track_id.clone()
    }

    /// The prototype's `track` helper.
    fn track(&mut self, kind: TrackKind, name: &str) -> String {
        let cmd = self.create_track(name, kind);
        self.add_track(&cmd)
    }

    /// `track(session, TrackKind.INSTRUMENT, instrument_plugin="sonara.synth.poly")`.
    fn synth_track(&mut self) -> String {
        let cmd = CreateTrack {
            instrument_plugin: Some("sonara.synth.poly".into()),
            ..self.create_track("T", TrackKind::Instrument)
        };
        self.add_track(&cmd)
    }

    fn add_asset(&mut self, asset: MediaAsset) {
        let id = asset.id.clone();
        self.run(
            &AddAsset { asset },
            "Add Media",
            json!({"created": [format!("asset:{id}")], "flags": ["media"]}),
            json!({"asset_id": id}),
        );
    }

    fn audio_clip(&mut self, track_id: &str, asset_id: &str, start: f64) -> AddAudioClip {
        AddAudioClip {
            track_id: track_id.to_owned(),
            asset_id: asset_id.to_owned(),
            start,
            length: None,
            source_offset: 0,
            name: None,
            gain_db: 0.0,
            clip_id: self.id(ids::CLIP),
        }
    }

    fn midi_clip(&mut self, track_id: &str, start: f64, length: f64) -> CreateMidiClip {
        CreateMidiClip {
            track_id: track_id.to_owned(),
            start,
            length,
            name: "MIDI".to_owned(),
            notes: Vec::new(),
            loop_length: None,
            color: None,
            clip_id: self.id(ids::CLIP),
        }
    }

    /// The prototype's `audio_setup` fixture: an audio track with a 4-second clip of a 4-second
    /// 48 kHz asset at beat 0.
    fn audio_setup(&mut self) -> (String, String) {
        self.add_asset(asset(4, 48_000, "med_a"));
        let tid = self.track(TrackKind::Audio, "T");
        let cmd = self.audio_clip(&tid, "med_a", 0.0);
        let cid = cmd.clip_id.clone();
        self.run(
            &cmd,
            "Add Audio Clip",
            json!({"created": [format!("clip:{cid}")], "updated": [format!("track:{tid}")], "flags": ["clips"]}),
            json!({"clip_id": cid, "start_beat": 0.0, "length_beats": 8.0, "start_seconds": 0.0, "end_seconds": 4.0}),
        );
        (tid, cid)
    }

    /// The prototype's `_midi_setup`: a synth track with an 8-beat clip at beat 4 holding the
    /// notes `n_0`..`n_7`.
    fn midi_setup(&mut self, loop_length: Option<f64>) -> (String, String) {
        let tid = self.synth_track();
        let notes = (0_i64..8)
            .map(|i| {
                note(
                    &format!("n_{i}"),
                    60 + i,
                    f64::from(u8::try_from(i).expect("small")),
                    0.5,
                )
            })
            .collect();
        let cmd = CreateMidiClip {
            notes,
            loop_length,
            ..self.midi_clip(&tid, 4.0, 8.0)
        };
        let cid = cmd.clip_id.clone();
        self.run(
            &cmd,
            "Create MIDI Clip",
            json!({"created": [format!("clip:{cid}")], "updated": [format!("track:{tid}")], "flags": ["clips", "notes"]}),
            json!({"clip_id": cid, "start_beat": 4.0, "length_beats": 8.0, "start_seconds": 2.0, "end_seconds": 6.0,
                   "note_ids": ["n_0", "n_1", "n_2", "n_3", "n_4", "n_5", "n_6", "n_7"]}),
        );
        (tid, cid)
    }

    fn split(&mut self, clip_id: &str, at: f64) -> SplitClip {
        SplitClip {
            clip_id: clip_id.to_owned(),
            at,
            new_clip_id: self.id(ids::CLIP),
        }
    }

    fn duplicate(&mut self, clip_id: &str) -> DuplicateClip {
        DuplicateClip {
            clip_id: clip_id.to_owned(),
            start: None,
            track_id: None,
            new_clip_id: self.id(ids::CLIP),
        }
    }

    fn add_send(&mut self, track_id: &str, target_id: &str, level_db: f64) -> AddSend {
        AddSend {
            track_id: track_id.to_owned(),
            target_id: target_id.to_owned(),
            level_db,
            pre_fader: false,
            send_id: self.id(ids::SEND),
        }
    }

    fn add_marker(&mut self, beat: f64, name: &str) -> AddMarker {
        AddMarker {
            beat,
            name: name.to_owned(),
            color: None,
            marker_id: self.id(ids::MARKER),
        }
    }

    fn add_section(&mut self, name: &str, start: f64, length: f64) -> AddSection {
        AddSection {
            name: name.to_owned(),
            start,
            length,
            color: None,
            section_id: self.id(ids::SECTION),
        }
    }

    fn add_device(&mut self, track_id: &str, plugin_id: &str) -> AddDevice {
        AddDevice {
            track_id: track_id.to_owned(),
            plugin_id: plugin_id.to_owned(),
            index: None,
            params: JsonMap::new(),
            preset: None,
            sidechain_id: None,
            name: None,
            device_id: self.id(ids::DEVICE),
        }
    }

    fn set_instrument(&mut self, track_id: &str, plugin_id: &str) -> SetInstrument {
        SetInstrument {
            track_id: track_id.to_owned(),
            plugin_id: plugin_id.to_owned(),
            preset: None,
            params: JsonMap::new(),
            device_id: self.id(ids::DEVICE),
        }
    }

    fn automation(
        &mut self,
        track_id: &str,
        device_id: Option<&str>,
        param: &str,
        points: &[(f64, f64)],
    ) -> SetAutomation {
        SetAutomation {
            track_id: track_id.to_owned(),
            device_id: device_id.map(str::to_owned),
            param: param.to_owned(),
            points: linear(points),
            start: None,
            end: None,
            lane_id: self.id(ids::LANE),
        }
    }
}

/// Expands the change shorthand (`"kind:id"` references; missing lists are empty) into the
/// agent-facing form of [`ChangeSet::to_json`].
fn changes_json(short: &Value) -> Value {
    let refs = |key: &str| -> Vec<Value> {
        short
            .get(key)
            .and_then(Value::as_array)
            .map_or_else(Vec::new, |items| {
                items
                    .iter()
                    .map(|r| {
                        let (kind, id) =
                            r.as_str().and_then(|r| r.split_once(':')).expect("kind:id");
                        json!({"kind": kind, "id": id})
                    })
                    .collect()
            })
    };
    json!({
        "created": refs("created"),
        "updated": refs("updated"),
        "deleted": refs("deleted"),
        "flags": short.get("flags").cloned().unwrap_or_else(|| json!([])),
    })
}

fn check(res: &CommandResult, operation: &str, changes: &Value, result: &Value) {
    assert_eq!(res.label, operation);
    assert_eq!(
        res.changes.to_json(),
        changes_json(changes),
        "{operation}: changes"
    );
    assert_eq!(
        &Value::Object(res.result.clone()),
        result,
        "{operation}: result"
    );
}

/// `json!` as a map, for command fields.
fn map(value: Value) -> JsonMap {
    match value {
        Value::Object(map) => map,
        other => panic!("not a JSON object: {other}"),
    }
}

/// `base` with the entries of `changes` replaced (the parameter literals below).
fn with(mut base: Value, changes: Value) -> Value {
    if let (Some(base), Value::Object(changes)) = (base.as_object_mut(), changes) {
        for (key, value) in changes {
            *base.get_mut(&key).expect("a known parameter") = value;
        }
    }
    base
}

/// The prototype's `asset` helper: `seconds` of stereo audio at `sample_rate`.
fn asset(seconds: u32, sample_rate: u32, asset_id: &str) -> MediaAsset {
    MediaAsset::new(MediaAssetData {
        id: asset_id.to_owned(),
        kind: MediaKind::Audio,
        name: "a.wav".to_owned(),
        path: "media/audio/a.wav".to_owned(),
        location: MediaLocation::Embedded,
        size_bytes: 100,
        sha256: None,
        sample_rate,
        channels: 2,
        frames: u64::from(seconds * sample_rate),
        format: "WAV".to_owned(),
        subtype: "PCM_24".to_owned(),
        original_path: None,
        relative_hint: None,
        metadata: JsonMap::new(),
        extra: JsonMap::new(),
    })
    .expect("valid asset")
}

fn note(id: &str, pitch: i64, start: f64, length: f64) -> MidiNote {
    MidiNote::new(id, pitch, start, length, 100, 0).expect("valid note")
}

fn linear(points: &[(f64, f64)]) -> Vec<AutomationPoint> {
    points
        .iter()
        .map(|&(beat, value)| AutomationPoint::new(beat, value, CurveKind::Linear).expect("valid"))
        .collect()
}

fn fade(seconds: f64) -> Fade {
    Fade::new(seconds, FadeCurve::Linear).expect("valid fade")
}

fn not_found(kind: &str, id: &str) -> Value {
    json!({"kind": kind, "id": id})
}

fn routing_issue(object_id: &str) -> Value {
    json!({"object_id": object_id, "issue_count": 1})
}

/// The prototype's `_heard`: `(timeline beat rounded to 9 digits, pitch)`, sorted.
fn heard(project: &Project, clip_ids: &[&str]) -> Vec<(f64, u8)> {
    let mut out: Vec<(f64, u8)> = clip_ids
        .iter()
        .flat_map(|cid| {
            let (_, clip) = project.clip(cid).expect("clip exists");
            let clip = clip.as_midi().expect("a MIDI clip");
            clip.playback_notes()
                .into_iter()
                .map(|(t, n)| ((t * 1e9).round() / 1e9, n.pitch))
                .collect::<Vec<_>>()
        })
        .collect();
    out.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    out
}

fn midi<'a>(project: &'a Project, clip_id: &str) -> &'a MidiClip {
    project
        .clip(clip_id)
        .expect("clip exists")
        .1
        .as_midi()
        .expect("a MIDI clip")
}

fn eq_defaults() -> Value {
    json!({"hp_on": false, "hp_freq": 80.0, "hp_slope": "12", "low_on": true, "low_type": "shelf", "low_freq": 100.0, "low_gain": 0.0, "low_q": 0.71, "mid1_on": true, "mid1_freq": 500.0, "mid1_gain": 0.0, "mid1_q": 1.0, "mid2_on": true, "mid2_freq": 2500.0, "mid2_gain": 0.0, "mid2_q": 1.0, "high_on": true, "high_type": "shelf", "high_freq": 8000.0, "high_gain": 0.0, "high_q": 0.71, "lp_on": false, "lp_freq": 18000.0, "lp_slope": "12", "output_db": 0.0})
}

fn compressor_defaults() -> Value {
    json!({"threshold_db": -18.0, "ratio": 4.0, "attack_ms": 10.0, "release_ms": 120.0, "knee_db": 6.0, "makeup_db": 0.0, "auto_makeup": false, "mix": 1.0, "detector": "peak", "sidechain": false, "sidechain_hpf_hz": 20.0, "sidechain_lpf_hz": 20000.0})
}

fn drum_defaults() -> Value {
    json!({"kick_tune": 52.0, "kick_decay": 0.45, "kick_punch": 0.5, "snare_tune": 1.0, "snare_tone": 0.6, "snare_decay": 0.18, "hat_tune": 1.0, "hat_decay": 1.0, "clap_decay": 0.25, "tom_tune": 1.0, "cymbal_decay": 1.6, "drive": 0.5, "kick_level_db": 0.0, "snare_level_db": -2.0, "hat_level_db": -9.0, "cymbal_level_db": -10.0, "perc_level_db": -6.0, "stereo_width": 1.0, "gain_db": -9.0})
}

// -- change sets ------------------------------------------------------------------------------

#[test]
fn changeset_merge_and_invert() {
    let a = ChangeSet::of(
        [(ObjectKind::Track, "t1")],
        [(ObjectKind::Clip, "c1")],
        [],
        [Flag::Tracks],
    );
    let b = ChangeSet::of(
        [],
        [(ObjectKind::Clip, "c1"), (ObjectKind::Clip, "c2")],
        [(ObjectKind::Track, "t1")],
        [Flag::Clips],
    );
    let m = a.merge(&b);
    // Created, then deleted: vanished.
    assert!(m.created.is_empty() && m.deleted.is_empty());
    assert_eq!(
        m.updated,
        [
            ObjectRef::new(ObjectKind::Clip, "c1"),
            ObjectRef::new(ObjectKind::Clip, "c2")
        ]
    );
    assert_eq!(m.flags, [Flag::Tracks, Flag::Clips].into_iter().collect());
    let inv = a.inverted();
    assert!(inv.deleted == a.created && inv.created == a.deleted);
    assert_eq!(a.ids(Some(ObjectKind::Track)), ["t1"]);
    assert!(ChangeSet::default().is_empty() && !a.is_empty());
    assert_eq!(
        a.to_json()["created"],
        json!([{"kind": "track", "id": "t1"}])
    );
    assert_eq!(
        m.to_json(),
        json!({"created": [], "updated": [{"kind": "clip", "id": "c1"}, {"kind": "clip", "id": "c2"}], "deleted": [], "flags": ["clips", "tracks"]})
    );
    assert_eq!(
        inv.to_json(),
        json!({"created": [], "updated": [{"kind": "clip", "id": "c1"}], "deleted": [{"kind": "track", "id": "t1"}], "flags": ["tracks"]})
    );
}

// -- session ----------------------------------------------------------------------------------

/// The prototype unsubscribes with a callback; in Rust dropping the receiver unsubscribes, so
/// the events received until then are checked.
#[test]
fn execute_undo_redo_and_versions() {
    let mut fx = Fx::new();
    let events = fx.session.subscribe();
    let v0 = fx.session.version();
    assert_eq!(v0, 1);
    let cmd = CreateTrack {
        instrument_plugin: Some("sonara.synth.poly".into()),
        ..fx.create_track("Bass", TrackKind::Instrument)
    };
    let res = fx.exec(&cmd);
    assert!(res.committed && !res.dry_run && res.version == v0 + 1);
    let tid = res.result["track_id"].as_str().expect("an ID").to_owned();
    assert_eq!(res.changes.ids(Some(ObjectKind::Track)), [tid.as_str()]);
    assert_eq!(
        res.to_json(),
        json!({"committed": true, "dry_run": false, "project_version": 2, "operation": "Create Track", "changes": {"created": [{"kind": "track", "id": "trk_000000000002"}, {"kind": "device", "id": "dev_000000000003"}], "updated": [], "deleted": [], "flags": ["routing", "tracks"]}, "result": {"track_id": "trk_000000000002", "instrument_device_id": "dev_000000000003"}})
    );
    assert_eq!(fx.session.history().undo_label(), Some("Create Track"));
    let undo = fx.session.undo("user").expect("one step");
    assert!(undo.committed && fx.project().find_track(&tid).is_none());
    assert_eq!(undo.changes.deleted[0].id, tid);
    assert_eq!(
        undo.to_json(),
        json!({"committed": true, "dry_run": false, "project_version": 3, "operation": "Undo Create Track", "changes": {"created": [], "updated": [], "deleted": [{"kind": "track", "id": "trk_000000000002"}, {"kind": "device", "id": "dev_000000000003"}], "flags": ["routing", "tracks"]}})
    );
    let redo = fx.session.redo("user").expect("one step");
    assert!(fx.project().find_track(&tid).is_some());
    assert_eq!(redo.version, v0 + 3);
    check(
        &redo,
        "Redo Create Track",
        &json!({"created": ["track:trk_000000000002", "device:dev_000000000003"], "flags": ["routing", "tracks"]}),
        &json!({}),
    );
    let seen: Vec<_> = events.try_iter().collect();
    let kinds: Vec<EventKind> = seen.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        [EventKind::Command, EventKind::Undo, EventKind::Redo]
    );
    let versions: Vec<u64> = seen.iter().map(|e| e.version).collect();
    assert_eq!(versions, [2, 3, 4]);
    for e in &seen {
        assert_eq!(
            (e.label.as_str(), e.source.as_str()),
            ("Create Track", "user")
        );
    }
    assert_eq!(seen[1].changes, res.changes.inverted());
    assert_eq!(seen[2].changes, res.changes);
    drop(events);
    fx.run(
        &RenameTrack {
            track_id: tid.clone(),
            name: "B".into(),
        },
        "Rename Track",
        json!({"updated": ["track:trk_000000000002"], "flags": ["tracks"]}),
        json!({}),
    );
    let first = fx.session.undo("user").expect("rename");
    assert_eq!(first.label, "Undo Rename Track");
    let second = fx.session.undo("user").expect("create");
    assert_eq!(second.label, "Undo Create Track");
    let err = fx.session.undo("user").expect_err("nothing left");
    assert_eq!(
        (err.code, err.message.as_str()),
        (ErrorCode::NothingToUndo, "nothing to undo")
    );
    assert_eq!(
        fx.session.redo("user").expect("create").label,
        "Redo Create Track"
    );
    assert_eq!(
        fx.session.redo("user").expect("rename").label,
        "Redo Rename Track"
    );
    let err = fx.session.redo("user").expect_err("nothing left");
    assert_eq!(
        (err.code, err.message.as_str()),
        (ErrorCode::NothingToRedo, "nothing to redo")
    );
    assert_eq!(fx.session.version(), 9);
}

#[test]
fn new_command_clears_redo() {
    let mut fx = Fx::new();
    fx.track(TrackKind::Audio, "A");
    fx.session.undo("user").expect("one step");
    assert!(fx.session.history().can_redo());
    assert_eq!(fx.track(TrackKind::Audio, "B"), "trk_000000000004");
    assert!(!fx.session.history().can_redo());
    assert_eq!(
        fx.session.history().labels(),
        (vec!["Create Track"], vec![])
    );
}

#[test]
fn dry_run_does_not_commit() {
    let mut fx = Fx::new();
    let (before, version) = (fx.project().clone(), fx.session.version());
    let cmd = fx.create_track("X", TrackKind::Audio);
    let res = fx.session.execute(&cmd, true, "user").expect("valid");
    assert!(res.dry_run && !res.committed && !res.changes.created.is_empty());
    assert_eq!(
        res.to_json(),
        json!({"committed": false, "dry_run": true, "project_version": 1, "operation": "Create Track", "changes": {"created": [{"kind": "track", "id": "trk_000000000002"}], "updated": [], "deleted": [], "flags": ["routing", "tracks"]}, "result": {"track_id": "trk_000000000002"}})
    );
    assert!(fx.project().ptr_eq(&before) && fx.session.version() == version);
    assert!(!fx.session.history().can_undo());
    let cmd = fx.create_track("Y", TrackKind::Audio);
    let simulated = fx.session.simulate(&cmd).expect("valid");
    let tracks: Vec<&str> = simulated.tracks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(tracks, ["trk_000000000004"]);
    assert!(fx.project().ptr_eq(&before));
}

#[test]
fn dirty_tracking() {
    let mut fx = Fx::with("P", 500, true);
    assert!(!fx.session.is_dirty());
    fx.exec(&RenameProject { name: "Q".into() });
    assert!(fx.session.is_dirty());
    fx.session.undo("user").expect("one step");
    // Back to the saved snapshot.
    assert!(!fx.session.is_dirty());
    fx.session.redo("user").expect("one step");
    let (project, location) = (
        fx.project().clone(),
        fx.session.require_location().expect("open").clone(),
    );
    fx.session.mark_saved(project, location);
    assert!(!fx.session.is_dirty());
    fx.session.close();
    assert!(fx.session.project().is_none() && !fx.session.is_dirty());
    let no_project = (
        ErrorCode::NoProject,
        "no project is open; create or open one first",
    );
    let err = fx.session.require_project().expect_err("closed");
    assert_eq!((err.code, err.message.as_str()), no_project);
    // The other entry points without a project, from the prototype.
    let err = fx.session.require_location().expect_err("closed");
    assert_eq!(
        (err.code, err.message.as_str()),
        (ErrorCode::NoProject, "no project is open")
    );
    let err = fx.session.undo("user").expect_err("closed");
    assert_eq!((err.code, err.message.as_str()), no_project);
    let err = fx.session.redo("user").expect_err("closed");
    assert_eq!((err.code, err.message.as_str()), no_project);
    let rename = RenameProject { name: "x".into() };
    let err = fx
        .session
        .execute(&rename, false, "user")
        .expect_err("closed");
    assert_eq!((err.code, err.message.as_str()), no_project);
    let err = fx.session.simulate(&rename).expect_err("closed");
    assert_eq!((err.code, err.message.as_str()), no_project);
}

/// A Rust listener cannot raise; it fails when its receiver is gone. The session drops such a
/// listener and still delivers the event to the others.
#[test]
fn listener_errors_are_isolated() {
    let mut fx = Fx::new();
    let gone = fx.session.subscribe();
    drop(gone);
    let seen = fx.session.subscribe();
    fx.exec(&RenameProject { name: "Z".into() });
    let kinds: Vec<EventKind> = seen.try_iter().map(|e| e.kind).collect();
    assert_eq!(kinds, [EventKind::Command]);
}

#[test]
fn merge_key_merges_within_window() {
    let mut fx = Fx::new();
    let tid = fx.track(TrackKind::Audio, "T");
    let mixer = |volume_db: Option<f64>, pan: Option<f64>, mute: Option<bool>| SetTrackMixer {
        track_id: tid.clone(),
        volume_db,
        pan,
        mute,
        ..SetTrackMixer::default()
    };
    let state = |volume_db: f64, pan: f64, mute: bool| json!({"volume_db": volume_db, "pan": pan, "mute": mute, "solo": false, "solo_safe": false});
    let updated = json!({"updated": [format!("track:{tid}")], "flags": ["mixer"]});
    fx.clock.set(100.0);
    for db in [-1.0, -2.0, -3.0] {
        fx.run(
            &mixer(Some(db), None, None),
            "Track Volume",
            updated.clone(),
            state(db, 0.0, false),
        );
        fx.clock.advance(0.5);
    }
    assert_eq!(
        fx.session.history().labels().0,
        ["Create Track", "Track Volume"]
    );
    fx.clock.advance(10.0);
    fx.run(
        &mixer(Some(-4.0), None, None),
        "Track Volume",
        updated.clone(),
        state(-4.0, 0.0, false),
    );
    assert_eq!(fx.session.history().labels().0.len(), 3);
    let volume = |fx: &Fx| fx.project().track(&tid).expect("exists").mixer.volume_db;
    let undo = fx.session.undo("user").expect("one step");
    check(&undo, "Undo Track Volume", &updated, &json!({}));
    assert!((volume(&fx) + 3.0).abs() < f64::EPSILON);
    fx.session.undo("user").expect("merged step");
    assert!(volume(&fx).abs() < f64::EPSILON);
    // Toggles never merge; seal() stops merging.
    fx.run(
        &mixer(None, None, Some(true)),
        "Mute",
        updated.clone(),
        state(0.0, 0.0, true),
    );
    fx.run(
        &mixer(None, None, Some(false)),
        "Mute",
        updated.clone(),
        state(0.0, 0.0, false),
    );
    fx.run(
        &mixer(None, Some(0.5), None),
        "Track Pan",
        updated.clone(),
        state(0.0, 0.5, false),
    );
    fx.session.seal_undo();
    fx.run(
        &mixer(None, Some(0.6), None),
        "Track Pan",
        updated,
        state(0.0, 0.6, false),
    );
    let labels = fx.session.history().labels().0;
    assert_eq!(
        labels[labels.len() - 4..],
        ["Mute", "Mute", "Track Pan", "Track Pan"]
    );
    assert_eq!(
        labels,
        ["Create Track", "Mute", "Mute", "Track Pan", "Track Pan"]
    );
}

#[test]
fn undo_limit() {
    let mut fx = Fx::with("P", 3, false);
    for i in 0..5 {
        fx.run(
            &RenameProject {
                name: format!("N{i}"),
            },
            "Rename Project",
            json!({"updated": ["project:project"], "flags": ["metadata"]}),
            json!({}),
        );
    }
    assert_eq!(fx.session.history().labels().0.len(), 3);
    assert_eq!(fx.session.history().reachable_projects().len(), 6);
}

#[test]
fn transaction_is_atomic() {
    let mut fx = Fx::new();
    let cmd = Transaction {
        commands: vec![
            Box::new(fx.create_track("A", TrackKind::Audio)),
            Box::new(RenameTrack {
                track_id: "trk_missing".into(),
                name: "x".into(),
            }),
        ],
        name: "Batch".into(),
    };
    fx.fails_with(
        &cmd,
        NOT_FOUND,
        "track 'trk_missing' not found",
        not_found("track", "trk_missing"),
    );
    let c = fx.create_track("A", TrackKind::Audio);
    let tid = c.track_id.clone();
    let ok = Transaction {
        commands: vec![
            Box::new(c),
            Box::new(RenameTrack {
                track_id: tid.clone(),
                name: "Renamed".into(),
            }),
        ],
        name: "Batch".into(),
    };
    let res = fx.run(
        &ok,
        "Batch",
        json!({"created": ["track:trk_000000000004"], "flags": ["routing", "tracks"]}),
        json!({"steps": [{"track_id": "trk_000000000004"}, {}]}),
    );
    assert_eq!(res["steps"].as_array().map(Vec::len), Some(2));
    assert_eq!(fx.project().track(&tid).expect("exists").name, "Renamed");
    let undo = fx.session.undo("user").expect("one step");
    check(
        &undo,
        "Undo Batch",
        &json!({"deleted": ["track:trk_000000000004"], "flags": ["routing", "tracks"]}),
        &json!({}),
    );
    assert_eq!(fx.project().tracks.len(), 0);
}

#[test]
fn command_context_is_registry() {
    let registry = Arc::new(builtin_registry());
    let session = ProjectSession::new(Arc::clone(&registry));
    assert!(Arc::ptr_eq(session.plugins(), &registry));
}

// -- project commands -------------------------------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "mirrors test_commands.py::test_project_commands"
)]
fn project_commands() {
    let mut fx = Fx::new();
    let project = || json!({"updated": ["project:project"], "flags": ["metadata"]});
    let tempo = || json!({"updated": ["project:project"], "flags": ["tempo"]});
    let settings = || json!({"updated": ["project:project"], "flags": ["settings"]});
    let set_tempo = |bpm: f64, beat: f64, clear_changes: bool| SetTempo {
        bpm,
        beat,
        ramp: false,
        clear_changes,
    };
    fx.run(
        &RenameProject {
            name: "  New  ".into(),
        },
        "Rename Project",
        project(),
        json!({}),
    );
    assert_eq!(fx.project().name, "New");
    fx.run(
        &set_tempo(140.0, 0.0, false),
        "Set Tempo",
        tempo(),
        json!({"tempo_points": 1}),
    );
    fx.run(
        &set_tempo(100.0, 16.0, false),
        "Set Tempo",
        tempo(),
        json!({"tempo_points": 2}),
    );
    let bpms: Vec<f64> = fx
        .project()
        .tempo_map
        .points()
        .iter()
        .map(|p| p.bpm)
        .collect();
    assert_eq!(bpms, [140.0, 100.0]);
    fx.run(
        &RemoveTempoPoint { beat: 16.0 },
        "Remove Tempo Change",
        tempo(),
        json!({}),
    );
    fx.fails(
        &RemoveTempoPoint { beat: 16.0 },
        NOT_FOUND,
        "no tempo point at beat 16.0",
    );
    fx.run(
        &set_tempo(90.0, 8.0, false),
        "Set Tempo",
        tempo(),
        json!({"tempo_points": 2}),
    );
    fx.run(
        &set_tempo(80.0, 0.0, true),
        "Set Tempo",
        tempo(),
        json!({"tempo_points": 1}),
    );
    assert_eq!(fx.project().tempo_map.points().len(), 1);
    fx.fails(
        &set_tempo(5.0, 0.0, false),
        INVALID,
        "tempo must be within 10.0-999.0 BPM",
    );
    fx.run(
        &SetTimeSignature {
            numerator: 3,
            denominator: 4,
            bar: 4,
        },
        "Set Time Signature",
        tempo(),
        json!({}),
    );
    fx.run(
        &RemoveTimeSignature { bar: 4 },
        "Remove Time Signature Change",
        tempo(),
        json!({}),
    );
    fx.fails(
        &SetTimeSignature {
            numerator: 4,
            denominator: 5,
            bar: 0,
        },
        INVALID,
        "time signature denominator must be one of (1, 2, 4, 8, 16, 32)",
    );
    fx.run(
        &SetProjectLength {
            length_beats: Some(64.0),
        },
        "Set Project Length",
        settings(),
        json!({}),
    );
    assert!((fx.project().end_beat() - 64.0).abs() < f64::EPSILON);
    fx.run(
        &SetProjectLength { length_beats: None },
        "Set Project Length",
        settings(),
        json!({}),
    );
    fx.fails(
        &SetSampleRate {
            sample_rate: 12_345,
        },
        INVALID,
        "sample rate must be one of (22050, 32000, 44100, 48000, 88200, 96000, 176400, 192000)",
    );
    fx.run(
        &SetSampleRate {
            sample_rate: 44_100,
        },
        "Set Sample Rate",
        settings(),
        json!({}),
    );
    let metadata = |pairs: &[(&str, &str)]| SetMetadata {
        values: pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    };
    fx.run(
        &metadata(&[("artist", "Me"), ("title", "Song")]),
        "Edit Project Info",
        project(),
        json!({}),
    );
    assert_eq!(fx.project().metadata.artist, "Me");
    fx.fails(
        &metadata(&[("bogus", "x")]),
        INVALID,
        "unknown metadata fields ['bogus']",
    );
    let transport = |values: Value| SetTransportSettings {
        values: map(values),
    };
    fx.run(
        &transport(json!({"loop_enabled": true, "loop_start": 4.0, "loop_end": 8.0})),
        "Transport Settings",
        json!({"updated": ["project:project"], "flags": ["transport"]}),
        json!({}),
    );
    assert!(fx.project().transport.loop_enabled);
    fx.fails(
        &transport(json!({"loop_start": 9.0})),
        INVALID,
        "loop start must be >= 0 and before loop end",
    );
    fx.fails(
        &transport(json!({"nope": 1})),
        INVALID,
        "unknown transport settings ['nope']",
    );
    let editor = |values: Value| SetEditorState {
        values: map(values),
    };
    let flags = json!({"flags": ["editor"]});
    fx.run(
        &editor(json!({"zoom": 2.0, "snap": "1/16"})),
        "Editor Settings",
        flags.clone(),
        json!({}),
    );
    fx.run(
        &editor(json!({"zoom": null})),
        "Editor Settings",
        flags,
        json!({}),
    );
    assert_eq!(
        Value::Object(fx.project().editor.clone()),
        json!({"snap": "1/16"})
    );
    // Editor state bypasses the undo history.
    assert_eq!(
        fx.session.history().labels().0,
        [
            "Rename Project",
            "Set Tempo",
            "Set Tempo",
            "Remove Tempo Change",
            "Set Tempo",
            "Set Tempo",
            "Set Time Signature",
            "Remove Time Signature Change",
            "Set Project Length",
            "Set Project Length",
            "Set Sample Rate",
            "Edit Project Info",
            "Transport Settings",
        ]
    );
}

#[test]
fn markers_and_sections() {
    let mut fx = Fx::new();
    let cmd = fx.add_marker(8.0, "Drop");
    let mid = "mrk_000000000002";
    assert_eq!(cmd.marker_id, mid);
    let marker = json!({"updated": [format!("marker:{mid}")], "flags": ["markers"]});
    fx.run(
        &cmd,
        "Add Marker",
        json!({"created": [format!("marker:{mid}")], "flags": ["markers"]}),
        json!({"marker_id": mid}),
    );
    let update = UpdateMarker {
        marker_id: mid.into(),
        beat: Some(12.0),
        name: Some("Drop 2".into()),
        color: None,
    };
    fx.run(&update, "Edit Marker", marker, json!({}));
    let m = fx.project().find_marker(mid).expect("exists");
    assert_eq!((m.beat, m.name.as_str()), (12.0, "Drop 2"));
    let remove = RemoveMarker {
        marker_id: mid.into(),
    };
    fx.run(
        &remove,
        "Remove Marker",
        json!({"deleted": [format!("marker:{mid}")], "flags": ["markers"]}),
        json!({}),
    );
    let message = format!("marker '{mid}' not found");
    fx.fails_with(&remove, NOT_FOUND, &message, not_found("marker", mid));
    let rename = UpdateMarker {
        marker_id: mid.into(),
        beat: None,
        name: Some("x".into()),
        color: None,
    };
    fx.fails_with(&rename, NOT_FOUND, &message, not_found("marker", mid));
    let cmd = fx.add_section("Intro", 0.0, 16.0);
    let sid = "sec_000000000003";
    assert_eq!(cmd.section_id, sid);
    fx.run(
        &cmd,
        "Add Section",
        json!({"created": [format!("section:{sid}")], "flags": ["markers"]}),
        json!({"section_id": sid}),
    );
    let update = UpdateSection {
        section_id: sid.into(),
        name: None,
        start: None,
        length: Some(32.0),
        color: None,
    };
    fx.run(
        &update,
        "Edit Section",
        json!({"updated": [format!("section:{sid}")], "flags": ["markers"]}),
        json!({}),
    );
    assert!((fx.project().find_section(sid).expect("exists").length - 32.0).abs() < f64::EPSILON);
    let blank = fx.add_section("", 0.0, 4.0);
    fx.fails(&blank, INVALID, "section name must not be empty");
    let remove = RemoveSection {
        section_id: sid.into(),
    };
    fx.run(
        &remove,
        "Remove Section",
        json!({"deleted": [format!("section:{sid}")], "flags": ["markers"]}),
        json!({}),
    );
    fx.fails_with(
        &remove,
        NOT_FOUND,
        &format!("section '{sid}' not found"),
        not_found("section", sid),
    );
}

#[test]
fn asset_commands() {
    let mut fx = Fx::new();
    fx.add_asset(asset(4, 48_000, "med_a"));
    fx.fails(
        &AddAsset {
            asset: asset(4, 48_000, "med_a"),
        },
        ErrorCode::Conflict,
        "asset 'med_a' already exists",
    );
    let renamed = asset(4, 48_000, "med_a")
        .edit(|d| d.name = "b.wav".into())
        .expect("valid");
    fx.run(
        &ReplaceAsset { asset: renamed },
        "Relink Media",
        json!({"updated": ["asset:med_a"], "flags": ["media"]}),
        json!({}),
    );
    assert_eq!(fx.project().asset("med_a").expect("exists").name, "b.wav");
    fx.fails_with(
        &ReplaceAsset {
            asset: asset(4, 48_000, "med_zz"),
        },
        NOT_FOUND,
        "asset 'med_zz' not found",
        not_found("asset", "med_zz"),
    );
    let tid = fx.track(TrackKind::Audio, "T");
    let cmd = fx.audio_clip(&tid, "med_a", 0.0);
    let cid = cmd.clip_id.clone();
    fx.run(
        &cmd,
        "Add Audio Clip",
        json!({"created": ["clip:clp_000000000004"], "updated": ["track:trk_000000000002"], "flags": ["clips"]}),
        json!({"clip_id": "clp_000000000004", "start_beat": 0.0, "length_beats": 8.0, "start_seconds": 0.0, "end_seconds": 4.0}),
    );
    let remove = RemoveAsset {
        asset_id: "med_a".into(),
    };
    fx.fails(
        &remove,
        ErrorCode::Conflict,
        "asset 'med_a' is still used by clips or devices",
    );
    fx.run(
        &DeleteClips {
            clip_ids: vec![cid],
        },
        "Delete Clips",
        json!({"deleted": ["clip:clp_000000000004"], "flags": ["clips"]}),
        json!({}),
    );
    fx.run(
        &remove,
        "Remove Media",
        json!({"deleted": ["asset:med_a"], "flags": ["media"]}),
        json!({}),
    );
    assert_eq!(fx.project().assets.len(), 0);
}

// -- tracks -----------------------------------------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "mirrors test_commands.py::test_create_track_variants"
)]
fn create_track_variants() {
    let mut fx = Fx::new();
    let registry = builtin_registry();
    let synth = registry.get("sonara.synth.poly").expect("built in");
    let cmd = CreateTrack {
        instrument_plugin: Some("sonara.synth.poly".into()),
        instrument_preset: Some("Analog Bass".into()),
        ..fx.create_track("Bass", TrackKind::Instrument)
    };
    let tid = fx.add_track(&cmd);
    let t = fx.project().track(&tid).expect("exists");
    let instrument = t.instrument.as_ref().expect("an instrument");
    assert_eq!(instrument.id, cmd.instrument_id);
    let preset = synth.preset("Analog Bass").expect("a factory preset");
    assert_eq!(instrument.preset.as_deref(), Some("Analog Bass"));
    assert_eq!(instrument.params["voice_mode"], preset.params["voice_mode"]);
    assert_eq!(instrument.params.len(), synth.params.len());
    assert_eq!(
        params_json(&instrument.params),
        json!({"voice_mode": "mono", "polyphony": 16, "glide": 0.03, "osc1_wave": "saw", "osc1_level": 0.8, "osc1_octave": 0, "osc1_detune": 0.0, "osc2_wave": "square", "osc2_level": 0.6, "osc2_octave": -1, "osc2_semitone": 0, "osc2_detune": 0.0, "pulse_width": 0.5, "noise_level": 0.0, "unison": 1, "unison_detune": 15.0, "unison_spread": 0.7, "filter_type": "lowpass", "filter_cutoff": 420.0, "filter_resonance": 0.3, "filter_env_amount": 2.8, "filter_keytrack": 0.3, "filter_attack": 0.005, "filter_decay": 0.22, "filter_sustain": 0.15, "filter_release": 0.3, "amp_attack": 0.002, "amp_decay": 0.3, "amp_sustain": 0.85, "amp_release": 0.08, "lfo_rate": 5.0, "lfo_pitch_cents": 0.0, "lfo_cutoff_oct": 0.0, "velocity_sensitivity": 0.7, "velocity_to_filter": 0.3, "bend_range": 2, "gain_db": -6.0})
    );
    let master = fx.create_track("M", TrackKind::Master);
    fx.fails(&master, INVALID, "a project has exactly one master track");
    let audio_with_synth = CreateTrack {
        instrument_plugin: Some("sonara.synth.poly".into()),
        ..fx.create_track("A", TrackKind::Audio)
    };
    fx.fails(
        &audio_with_synth,
        INVALID,
        "only instrument tracks take an instrument",
    );
    let eq_instrument = CreateTrack {
        instrument_plugin: Some("sonara.eq.parametric".into()),
        ..fx.create_track("I", TrackKind::Instrument)
    };
    fx.fails(
        &eq_instrument,
        INVALID,
        "Parametric EQ is an effect, not an instrument",
    );
    let unknown = CreateTrack {
        instrument_plugin: Some("nope".into()),
        ..fx.create_track("I", TrackKind::Instrument)
    };
    fx.fails_with(
        &unknown,
        NOT_FOUND,
        "unknown plugin 'nope'",
        json!({"available": ["sonara.convolver", "sonara.delay", "sonara.drums", "sonara.dynamics.compressor", "sonara.dynamics.gate", "sonara.dynamics.limiter", "sonara.eq.parametric", "sonara.filter", "sonara.reverb", "sonara.saturation", "sonara.synth.poly", "sonara.utility"]}),
    );
    let missing_output = CreateTrack {
        output_id: "trk_missing".into(),
        ..fx.create_track("A", TrackKind::Audio)
    };
    fx.fails_with(
        &missing_output,
        ErrorCode::InvalidRouting,
        "output target 'trk_missing' of 'A' does not exist",
        routing_issue("trk_000000000012"),
    );
    let first = CreateTrack {
        index: Some(0),
        ..fx.create_track("First", TrackKind::Audio)
    };
    let first = fx.add_track(&first);
    assert_eq!(first, "trk_000000000014");
    assert_eq!(fx.project().tracks[0].id, first);
    let updated = json!({"updated": [format!("track:{first}")], "flags": ["tracks"]});
    fx.run(
        &RenameTrack {
            track_id: first.clone(),
            name: " Lead ".into(),
        },
        "Rename Track",
        updated.clone(),
        json!({}),
    );
    fx.run(
        &SetTrackColor {
            track_id: first.clone(),
            color: Some("#ff0000".into()),
        },
        "Track Color",
        updated.clone(),
        json!({}),
    );
    let t = fx.project().track(&first).expect("exists");
    assert_eq!(
        (t.name.as_str(), t.color.as_deref()),
        ("Lead", Some("#ff0000"))
    );
    fx.fails(
        &SetTrackColor {
            track_id: first.clone(),
            color: Some("red".into()),
        },
        INVALID,
        "color must be '#rrggbb', got 'red'",
    );
    let moved = fx.run(
        &MoveTrack {
            track_id: first,
            index: 99,
        },
        "Move Track",
        updated,
        json!({"index": 1}),
    );
    assert_eq!(moved["index"], json!(fx.project().tracks.len() - 1));
}

#[test]
fn mixer_and_input() {
    let mut fx = Fx::new();
    let tid = fx.track(TrackKind::Audio, "T");
    let updated = json!({"updated": [format!("track:{tid}")], "flags": ["mixer"]});
    let out = fx.run(
        &SetTrackMixer {
            track_id: tid.clone(),
            volume_db: Some(-6.0),
            pan: Some(0.25),
            ..SetTrackMixer::default()
        },
        "Track Volume",
        updated.clone(),
        json!({"volume_db": -6.0, "pan": 0.25, "mute": false, "solo": false, "solo_safe": false}),
    );
    assert!(out["volume_db"] == -6.0 && out["pan"] == 0.25);
    fx.run(
        &SetTrackMixer {
            track_id: tid.clone(),
            mute: Some(true),
            ..SetTrackMixer::default()
        },
        "Mute",
        updated,
        json!({"volume_db": -6.0, "pan": 0.25, "mute": true, "solo": false, "solo_safe": false}),
    );
    let m = fx.project().track(&tid).expect("exists").mixer;
    assert_eq!((m.volume_db, m.pan, m.mute), (-6.0, 0.25, true));
    fx.fails(
        &SetTrackMixer {
            track_id: tid.clone(),
            volume_db: Some(20.0),
            ..SetTrackMixer::default()
        },
        INVALID,
        "volume must be within -144..+12 dB",
    );
    fx.fails(
        &SetTrackMixer {
            track_id: MASTER_ID.into(),
            solo: Some(true),
            ..SetTrackMixer::default()
        },
        INVALID,
        "the master track cannot be soloed",
    );
    fx.run(
        &SetTrackMixer {
            track_id: MASTER_ID.into(),
            volume_db: Some(-1.0),
            ..SetTrackMixer::default()
        },
        "Track Volume",
        json!({"updated": ["track:master"], "flags": ["mixer"]}),
        json!({"volume_db": -1.0, "pan": 0.0, "mute": false, "solo": false, "solo_safe": false}),
    );
    fx.run(
        &SetTrackInput {
            track_id: tid.clone(),
            armed: Some(true),
            monitor: Some(MonitorMode::On),
            channels: Some(vec![0, 1]),
        },
        "Track Input",
        json!({"updated": [format!("track:{tid}")], "flags": ["tracks"]}),
        json!({}),
    );
    let input = &fx.project().track(&tid).expect("exists").input;
    assert!(input.armed && input.monitor == MonitorMode::On && input.channels == [0, 1]);
    let bus = fx.track(TrackKind::Bus, "T");
    fx.fails(
        &SetTrackInput {
            track_id: bus,
            armed: Some(true),
            ..SetTrackInput::default()
        },
        INVALID,
        "only audio and instrument tracks can record",
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "mirrors test_commands.py::test_routing_commands"
)]
fn routing_commands() {
    let mut fx = Fx::new();
    let a = fx.track(TrackKind::Audio, "A");
    let bus1 = fx.track(TrackKind::Bus, "Bus1");
    let bus2 = fx.track(TrackKind::Bus, "Bus2");
    let output = |track_id: &str, output_id: &str| SetTrackOutput {
        track_id: track_id.to_owned(),
        output_id: output_id.to_owned(),
    };
    let routed = |id: &str| json!({"updated": [format!("track:{id}")], "flags": ["routing"]});
    fx.run(&output(&a, &bus1), "Route Output", routed(&a), json!({}));
    fx.run(
        &output(&bus1, &bus2),
        "Route Output",
        routed(&bus1),
        json!({}),
    );
    let cycle = ErrorCode::RoutingCycle;
    let invalid_routing = ErrorCode::InvalidRouting;
    fx.fails_with(
        &output(&bus2, &bus1),
        cycle,
        "routing feedback loop: Bus1 → Bus2 → Bus1",
        routing_issue(&bus1),
    );
    fx.fails_with(
        &output(&bus1, &bus1),
        cycle,
        "'Bus1' cannot route to itself",
        routing_issue(&bus1),
    );
    fx.fails(
        &output(MASTER_ID, &bus1),
        invalid_routing,
        "the master output is the audio device",
    );
    fx.fails_with(
        &output(&bus2, &a),
        invalid_routing,
        "'Bus2' can only output to a bus or the master",
        routing_issue(&bus2),
    );
    fx.fails_with(
        &output(&a, "trk_missing"),
        NOT_FOUND,
        "track 'trk_missing' not found",
        not_found("track", "trk_missing"),
    );
    let cmd = fx.add_send(&a, &bus2, -6.0);
    let send_id = cmd.send_id.clone();
    assert_eq!(send_id, "snd_000000000008");
    fx.run(
        &cmd,
        "Add Send",
        json!({"created": [format!("send:{send_id}")], "updated": [format!("track:{a}")], "flags": ["routing"]}),
        json!({"send_id": send_id}),
    );
    let again = fx.add_send(&a, &bus2, 0.0);
    fx.fails(&again, ErrorCode::Conflict, "'A' already sends to 'Bus2'");
    let to_audio = fx.add_send(&bus1, &a, 0.0);
    fx.fails(&to_audio, invalid_routing, "sends must target a bus track");
    let feedback = fx.add_send(&bus2, &bus1, 0.0);
    fx.fails_with(
        &feedback,
        cycle,
        "routing feedback loop: Bus1 → Bus2 → Bus1",
        routing_issue(&bus1),
    );
    let update = UpdateSend {
        track_id: a.clone(),
        send_id: send_id.clone(),
        level_db: Some(-3.0),
        pre_fader: Some(true),
        enabled: None,
    };
    fx.run(
        &update,
        "Send Level",
        json!({"updated": [format!("send:{send_id}"), format!("track:{a}")], "flags": ["mixer"]}),
        json!({}),
    );
    let s = fx
        .project()
        .track(&a)
        .and_then(|t| t.send(&send_id))
        .expect("exists");
    assert_eq!((s.level_db, s.pre_fader, s.enabled), (-3.0, true, true));
    let lane = fx.automation(
        &a,
        None,
        &format!("send.{send_id}.level_db"),
        &[(0.0, -10.0)],
    );
    assert_eq!(lane.lane_id, "aut_000000000012");
    fx.run(
        &lane,
        "Edit Automation",
        json!({"created": ["automation_lane:aut_000000000012"], "updated": [format!("track:{a}")], "flags": ["automation"]}),
        json!({"lane_id": "aut_000000000012", "point_count": 1, "value_range": [-144.0, 12.0]}),
    );
    let remove = RemoveSend {
        track_id: a.clone(),
        send_id: send_id.clone(),
    };
    fx.run(
        &remove,
        "Remove Send",
        json!({"updated": [format!("track:{a}")], "deleted": [format!("send:{send_id}")], "flags": ["routing"]}),
        json!({}),
    );
    let t = fx.project().track(&a).expect("exists");
    assert!(t.sends.is_empty() && t.automation.is_empty());
    fx.fails_with(
        &remove,
        NOT_FOUND,
        &format!("send '{send_id}' not found"),
        not_found("send", &send_id),
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "mirrors test_commands.py::test_delete_track_repairs_references"
)]
fn delete_track_repairs_references() {
    let mut fx = Fx::new();
    let bus = fx.track(TrackKind::Bus, "Bus");
    let folder = fx.track(TrackKind::Folder, "Folder");
    let cmd = CreateTrack {
        output_id: bus.clone(),
        parent_id: Some(folder.clone()),
        ..fx.create_track("A", TrackKind::Audio)
    };
    let a = fx.add_track(&cmd);
    let b = fx.track(TrackKind::Audio, "B");
    let cmd = fx.add_send(&b, &bus, 0.0);
    let send = cmd.send_id.clone();
    fx.run(
        &cmd,
        "Add Send",
        json!({"created": [format!("send:{send}")], "updated": [format!("track:{b}")], "flags": ["routing"]}),
        json!({"send_id": send}),
    );
    let lane = fx.automation(&b, None, &format!("send.{send}.level_db"), &[(0.0, -6.0)]);
    fx.run(
        &lane,
        "Edit Automation",
        json!({"created": ["automation_lane:aut_000000000011"], "updated": [format!("track:{b}")], "flags": ["automation"]}),
        json!({"lane_id": "aut_000000000011", "point_count": 1, "value_range": [-144.0, 12.0]}),
    );
    let kick = fx.track(TrackKind::Audio, "Kick");
    let cmd = AddDevice {
        sidechain_id: Some(kick.clone()),
        ..fx.add_device(&b, "sonara.dynamics.compressor")
    };
    let comp = cmd.device_id.clone();
    fx.run(
        &cmd,
        "Add Effect",
        json!({"created": [format!("device:{comp}")], "updated": [format!("track:{b}")], "flags": ["devices"]}),
        json!({"device_id": comp, "index": 0, "params": compressor_defaults()}),
    );
    let res = fx.run(
        &DeleteTrack { track_id: bus.clone() },
        "Delete Track",
        json!({"updated": [format!("track:{a}"), format!("track:{b}")], "deleted": [format!("track:{bus}"), format!("send:{send}")], "flags": ["clips", "routing", "tracks"]}),
        json!({"deleted_clips": 0, "rerouted_tracks": [a, b]}),
    );
    let p = fx.project();
    assert_eq!(
        p.track(&a).expect("exists").output_id.as_deref(),
        Some(MASTER_ID)
    );
    let tb = p.track(&b).expect("exists");
    assert!(tb.sends.is_empty() && tb.automation.is_empty());
    assert_eq!(res["rerouted_tracks"], json!([a, b]));
    fx.run(
        &DeleteTrack { track_id: kick.clone() },
        "Delete Track",
        json!({"updated": [format!("track:{b}")], "deleted": [format!("track:{kick}")], "flags": ["clips", "routing", "tracks"]}),
        json!({"deleted_clips": 0, "rerouted_tracks": [b]}),
    );
    let sidechain = fx
        .project()
        .track(&b)
        .ok()
        .and_then(|t| t.find_device(&comp))
        .map(|d| d.sidechain_id.clone());
    assert_eq!(sidechain, Some(None));
    fx.run(
        &DeleteTrack { track_id: folder.clone() },
        "Delete Track",
        json!({"updated": [format!("track:{a}")], "deleted": [format!("track:{folder}")], "flags": ["clips", "routing", "tracks"]}),
        json!({"deleted_clips": 0, "rerouted_tracks": [a]}),
    );
    assert_eq!(fx.project().track(&a).expect("exists").parent_id, None);
    fx.fails(
        &DeleteTrack {
            track_id: MASTER_ID.into(),
        },
        INVALID,
        "the master track cannot be deleted",
    );
    fx.fails_with(
        &DeleteTrack {
            track_id: bus.clone(),
        },
        NOT_FOUND,
        &format!("track '{bus}' not found"),
        not_found("track", &bus),
    );
    let undo = fx.session.undo("user").expect("folder");
    check(
        &undo,
        "Undo Delete Track",
        &json!({"created": [format!("track:{folder}")], "updated": [format!("track:{a}")], "flags": ["clips", "routing", "tracks"]}),
        &json!({}),
    );
    fx.session.undo("user").expect("kick");
    let undo = fx.session.undo("user").expect("bus");
    check(
        &undo,
        "Undo Delete Track",
        &json!({"created": [format!("track:{bus}"), format!("send:{send}")], "updated": [format!("track:{a}"), format!("track:{b}")], "flags": ["clips", "routing", "tracks"]}),
        &json!({}),
    );
    assert_eq!(
        fx.project().track(&a).expect("exists").output_id.as_deref(),
        Some(bus.as_str())
    );
}

#[test]
fn track_parent() {
    let mut fx = Fx::new();
    let folder = fx.track(TrackKind::Folder, "F");
    let a = fx.track(TrackKind::Audio, "T");
    let updated = json!({"updated": [format!("track:{a}")], "flags": ["tracks"]});
    let parent = |parent_id: Option<&str>| SetTrackParent {
        track_id: a.clone(),
        parent_id: parent_id.map(str::to_owned),
    };
    fx.run(
        &parent(Some(&folder)),
        "Move Into Folder",
        updated.clone(),
        json!({}),
    );
    assert_eq!(
        fx.project().track(&a).expect("exists").parent_id.as_deref(),
        Some(folder.as_str())
    );
    let other = fx.track(TrackKind::Audio, "X");
    fx.fails_with(
        &parent(Some(&other)),
        ErrorCode::InvalidRouting,
        "parent of 'T' must be a folder track",
        routing_issue(&a),
    );
    fx.run(&parent(None), "Move Into Folder", updated, json!({}));
}

// -- clips ------------------------------------------------------------------------------------

#[test]
fn add_audio_clip_lengths() {
    let mut fx = Fx::new();
    fx.add_asset(asset(4, 48_000, "med_a"));
    let tid = fx.track(TrackKind::Audio, "T");
    let added = |cid: &str| json!({"created": [format!("clip:{cid}")], "updated": [format!("track:{tid}")], "flags": ["clips"]});
    // 4 s at 120 BPM = 8 beats.
    let cmd = fx.audio_clip(&tid, "med_a", 2.0);
    let cid = cmd.clip_id.clone();
    let res = fx.run(
        &cmd,
        "Add Audio Clip",
        added(&cid),
        json!({"clip_id": cid, "start_beat": 2.0, "length_beats": 8.0, "start_seconds": 1.0, "end_seconds": 5.0}),
    );
    assert!(res["length_beats"] == 8.0 && res["start_seconds"] == 1.0);
    let cmd = AddAudioClip {
        length: Some(2.0),
        source_offset: 96_000,
        ..fx.audio_clip(&tid, "med_a", 0.0)
    };
    let cid = cmd.clip_id.clone();
    fx.run(
        &cmd,
        "Add Audio Clip",
        added(&cid),
        json!({"clip_id": cid, "start_beat": 0.0, "length_beats": 2.0, "start_seconds": 0.0, "end_seconds": 1.0}),
    );
    let too_long = AddAudioClip {
        length: Some(9.0),
        ..fx.audio_clip(&tid, "med_a", 0.0)
    };
    fx.fails_with(
        &too_long,
        INVALID,
        "clip is longer than the remaining media (8.000 beats)",
        json!({"max_length_beats": 8.0}),
    );
    let past_end = AddAudioClip {
        source_offset: 192_000,
        ..fx.audio_clip(&tid, "med_a", 0.0)
    };
    fx.fails(
        &past_end,
        INVALID,
        "source offset is beyond the end of the media",
    );
    let inst = fx.track(TrackKind::Instrument, "T");
    let on_instrument = fx.audio_clip(&inst, "med_a", 0.0);
    fx.fails(
        &on_instrument,
        INVALID,
        "track 'T' (instrument) cannot hold audio clips",
    );
    let unknown = fx.audio_clip(&tid, "med_zz", 0.0);
    fx.fails_with(
        &unknown,
        NOT_FOUND,
        "asset 'med_zz' not found",
        not_found("asset", "med_zz"),
    );
}

fn trim(clip_id: &str, start: Option<f64>, end: Option<f64>) -> TrimClip {
    TrimClip {
        clip_id: clip_id.to_owned(),
        start,
        end,
    }
}

#[test]
fn audio_trim_moves_source_offset() {
    let mut fx = Fx::new();
    let (_, cid) = fx.audio_setup();
    let trimmed = json!({"updated": [format!("clip:{cid}")], "flags": ["clips"]});
    // 1 beat = 0.5 s = 24000 frames.
    fx.run(
        &trim(&cid, Some(1.0), None),
        "Trim Clip",
        trimmed.clone(),
        json!({"clip_id": cid, "start_beat": 1.0, "length_beats": 7.0, "start_seconds": 0.5, "end_seconds": 4.0}),
    );
    let clip = fx
        .project()
        .clip(&cid)
        .expect("exists")
        .1
        .as_audio()
        .expect("audio")
        .clone();
    assert_eq!(
        (clip.start, clip.length, clip.source_offset),
        (1.0, 7.0, 24_000)
    );
    fx.run(
        &trim(&cid, Some(0.5), Some(6.0)),
        "Trim Clip",
        trimmed,
        json!({"clip_id": cid, "start_beat": 0.5, "length_beats": 5.5, "start_seconds": 0.25, "end_seconds": 3.0}),
    );
    let clip = fx
        .project()
        .clip(&cid)
        .expect("exists")
        .1
        .as_audio()
        .expect("audio")
        .clone();
    assert_eq!(
        (clip.start, clip.start + clip.length, clip.source_offset),
        (0.5, 6.0, 12_000)
    );
    // Before the media start, past the media end, and an empty clip.
    fx.fails(
        &trim(&cid, Some(-0.5), None),
        INVALID,
        "cannot extend a clip before the start of its media",
    );
    fx.fails(
        &trim(&cid, None, Some(8.6)),
        INVALID,
        "cannot extend a clip past the end of its media",
    );
    fx.fails(
        &trim(&cid, Some(6.0), None),
        INVALID,
        "clip end must be after its start",
    );
}

#[test]
fn audio_trim_respects_asset_rate() {
    let mut fx = Fx::new();
    fx.add_asset(asset(4, 44_100, "med_a"));
    let tid = fx.track(TrackKind::Audio, "T");
    let cmd = fx.audio_clip(&tid, "med_a", 0.0);
    let cid = cmd.clip_id.clone();
    fx.run(
        &cmd,
        "Add Audio Clip",
        json!({"created": [format!("clip:{cid}")], "updated": [format!("track:{tid}")], "flags": ["clips"]}),
        json!({"clip_id": cid, "start_beat": 0.0, "length_beats": 8.0, "start_seconds": 0.0, "end_seconds": 4.0}),
    );
    // 1 s at the source's 44.1 kHz.
    fx.run(
        &trim(&cid, Some(2.0), None),
        "Trim Clip",
        json!({"updated": [format!("clip:{cid}")], "flags": ["clips"]}),
        json!({"clip_id": cid, "start_beat": 2.0, "length_beats": 6.0, "start_seconds": 1.0, "end_seconds": 4.0}),
    );
    let offset = fx
        .project()
        .clip(&cid)
        .expect("exists")
        .1
        .as_audio()
        .map(|c| c.source_offset);
    assert_eq!(offset, Some(44_100));
}

#[test]
fn audio_split() {
    let mut fx = Fx::new();
    let (_, cid) = fx.audio_setup();
    let fades = SetClipProperties {
        clip_id: cid.clone(),
        fade_in: Some(fade(0.1)),
        fade_out: Some(fade(0.2)),
        ..SetClipProperties::default()
    };
    fx.run(
        &fades,
        "Clip Properties",
        json!({"updated": [format!("clip:{cid}")], "flags": ["clips"]}),
        json!({}),
    );
    let split = SplitClip {
        clip_id: cid.clone(),
        at: 3.0,
        new_clip_id: "clp_right".into(),
    };
    let res = fx.run(
        &split,
        "Split Clip",
        json!({"created": ["clip:clp_right"], "updated": [format!("clip:{cid}")], "flags": ["clips"]}),
        json!({"left_clip_id": cid, "right_clip_id": "clp_right"}),
    );
    let p = fx.project();
    let left = p.clip(&cid).expect("exists").1.as_audio().expect("audio");
    let right = p
        .clip("clp_right")
        .expect("exists")
        .1
        .as_audio()
        .expect("audio");
    assert_eq!((left.start, left.length, left.source_offset), (0.0, 3.0, 0));
    assert_eq!(
        (right.start, right.length, right.source_offset),
        (3.0, 5.0, 72_000)
    );
    assert_eq!((left.fade_in.seconds, left.fade_out.seconds), (0.1, 0.0));
    assert_eq!((right.fade_in.seconds, right.fade_out.seconds), (0.0, 0.2));
    assert_eq!(
        Value::Object(res),
        json!({"left_clip_id": cid, "right_clip_id": "clp_right"})
    );
    let again = fx.split(&cid, 3.0);
    fx.fails(&again, INVALID, "split position must be inside the clip");
    let at_start = fx.split(&cid, 0.0);
    fx.fails(&at_start, INVALID, "split position must be inside the clip");
}

#[test]
fn midi_split_preserves_playback() {
    let mut fx = Fx::new();
    let (_, cid) = fx.midi_setup(None);
    let original = heard(fx.project(), &[cid.as_str()]);
    assert_eq!(
        original,
        [
            (4.0, 60),
            (5.0, 61),
            (6.0, 62),
            (7.0, 63),
            (8.0, 64),
            (9.0, 65),
            (10.0, 66),
            (11.0, 67)
        ]
    );
    let split = SplitClip {
        clip_id: cid.clone(),
        at: 7.0,
        new_clip_id: "clp_r".into(),
    };
    fx.run(
        &split,
        "Split Clip",
        json!({"created": ["clip:clp_r"], "updated": [format!("clip:{cid}")], "flags": ["clips"]}),
        json!({"left_clip_id": cid, "right_clip_id": "clp_r"}),
    );
    assert!((midi(fx.project(), "clp_r").content_offset - 3.0).abs() < f64::EPSILON);
    assert_eq!(heard(fx.project(), &[cid.as_str(), "clp_r"]), original);
}

#[test]
fn midi_trim_and_loop() {
    let mut fx = Fx::new();
    let (_, cid) = fx.midi_setup(None);
    fx.run(
        &trim(&cid, Some(6.0), None),
        "Trim Clip",
        json!({"updated": [format!("clip:{cid}")], "flags": ["clips"]}),
        json!({"clip_id": cid, "start_beat": 6.0, "length_beats": 6.0, "start_seconds": 3.0, "end_seconds": 6.0}),
    );
    let c = midi(fx.project(), &cid);
    assert_eq!((c.start, c.length, c.content_offset), (6.0, 6.0, 2.0));
    fx.fails(
        &trim(&cid, Some(3.0), None),
        INVALID,
        "cannot extend a MIDI clip before its content start",
    );
    let (_, looped) = fx.midi_setup(Some(2.0));
    // Extending before the content start wraps within the loop.
    fx.run(
        &trim(&looped, Some(3.0), None),
        "Trim Clip",
        json!({"updated": [format!("clip:{looped}")], "flags": ["clips"]}),
        json!({"clip_id": looped, "start_beat": 3.0, "length_beats": 9.0, "start_seconds": 1.5, "end_seconds": 6.0}),
    );
    let c = midi(fx.project(), &looped);
    assert!((c.start - 3.0).abs() < f64::EPSILON && (c.content_offset - 1.0).abs() < 1e-12);
    let heard = heard(fx.project(), &[looped.as_str()]);
    assert!(heard[0] == (3.0, 61) && heard[1] == (4.0, 60));
    assert_eq!(
        heard,
        [
            (3.0, 61),
            (4.0, 60),
            (5.0, 61),
            (6.0, 60),
            (7.0, 61),
            (8.0, 60),
            (9.0, 61),
            (10.0, 60),
            (11.0, 61)
        ]
    );
}

#[test]
fn move_and_duplicate() {
    let mut fx = Fx::new();
    let (tid, cid) = fx.audio_setup();
    let other = fx.track(TrackKind::Audio, "Other");
    let cmd = MoveClip {
        clip_id: cid.clone(),
        start: Some(4.0),
        track_id: Some(other.clone()),
    };
    fx.run(
        &cmd,
        "Move Clip",
        json!({"updated": [format!("clip:{cid}"), format!("track:{tid}"), format!("track:{other}")], "flags": ["clips"]}),
        json!({"clip_id": cid, "start_beat": 4.0, "length_beats": 8.0, "start_seconds": 2.0, "end_seconds": 6.0}),
    );
    let p = fx.project();
    assert_eq!(p.track(&tid).expect("exists").clips.len(), 0);
    assert!(
        (p.track(&other)
            .and_then(|t| t.clip(&cid))
            .expect("moved")
            .start()
            - 4.0)
            .abs()
            < f64::EPSILON
    );
    let inst = fx.track(TrackKind::Instrument, "T");
    let message = "track 'T' (instrument) cannot hold audio clips";
    fx.fails(
        &MoveClip {
            clip_id: cid.clone(),
            start: None,
            track_id: Some(inst.clone()),
        },
        INVALID,
        message,
    );
    let dup = DuplicateClip {
        new_clip_id: "clp_dup".into(),
        ..fx.duplicate(&cid)
    };
    let res = fx.run(
        &dup,
        "Duplicate Clip",
        json!({"created": ["clip:clp_dup"], "updated": [format!("track:{other}")], "flags": ["clips"]}),
        json!({"clip_id": "clp_dup", "start_beat": 12.0, "length_beats": 8.0, "start_seconds": 6.0, "end_seconds": 10.0}),
    );
    assert_eq!(res["start_beat"], 12.0);
    let to_instrument = DuplicateClip {
        track_id: Some(inst),
        ..fx.duplicate(&cid)
    };
    fx.fails(&to_instrument, INVALID, message);
    fx.run(
        &MoveClips {
            clip_ids: vec![cid.clone(), "clp_dup".into()],
            delta_beats: -4.0,
        },
        "Move Clips",
        json!({"updated": [format!("clip:{cid}"), "clip:clp_dup"], "flags": ["clips"]}),
        json!({}),
    );
    assert!((fx.project().clip("clp_dup").expect("exists").1.start() - 8.0).abs() < f64::EPSILON);
    fx.fails(
        &MoveClips {
            clip_ids: vec![cid.clone()],
            delta_beats: -1.0,
        },
        INVALID,
        "clips cannot move before the project start",
    );
    fx.run(
        &DeleteClips { clip_ids: vec![cid.clone(), "clp_dup".into(), cid.clone()] },
        "Delete Clips",
        json!({"deleted": [format!("clip:{cid}"), "clip:clp_dup", format!("clip:{cid}")], "flags": ["clips"]}),
        json!({}),
    );
    assert_eq!(fx.project().track(&other).expect("exists").clips.len(), 0);
    fx.fails_with(
        &DeleteClips {
            clip_ids: vec![cid.clone()],
        },
        NOT_FOUND,
        &format!("clip '{cid}' not found"),
        not_found("clip", &cid),
    );
}

#[test]
fn clip_properties() {
    let mut fx = Fx::new();
    let (_, cid) = fx.audio_setup();
    let props = |clip_id: &str| SetClipProperties {
        clip_id: clip_id.to_owned(),
        ..SetClipProperties::default()
    };
    let cmd = SetClipProperties {
        name: Some("Vox".into()),
        gain_db: Some(-3.0),
        muted: Some(true),
        color: Some("#00ff00".into()),
        fade_in: Some(Fade::new(0.5, FadeCurve::EqualPower).expect("valid")),
        ..props(&cid)
    };
    fx.run(
        &cmd,
        "Clip Properties",
        json!({"updated": [format!("clip:{cid}")], "flags": ["clips"]}),
        json!({}),
    );
    let c = fx
        .project()
        .clip(&cid)
        .expect("exists")
        .1
        .as_audio()
        .expect("audio")
        .clone();
    assert_eq!(
        (
            c.name.as_str(),
            c.gain_db,
            c.muted,
            c.color.as_deref(),
            c.fade_in.curve
        ),
        ("Vox", -3.0, true, Some("#00ff00"), FadeCurve::EqualPower)
    );
    let long_fades = SetClipProperties {
        fade_in: Some(fade(3.0)),
        fade_out: Some(fade(2.0)),
        ..props(&cid)
    };
    fx.fails(&long_fades, INVALID, "fades are longer than the clip");
    let audio_loop = SetClipProperties {
        loop_length: Some(Some(2.0)),
        ..props(&cid)
    };
    fx.fails(
        &audio_loop,
        ErrorCode::Unsupported,
        "audio clip looping is not supported yet",
    );
    let (_, mid) = fx.midi_setup(None);
    let midi_gain = SetClipProperties {
        gain_db: Some(-1.0),
        ..props(&mid)
    };
    fx.fails(
        &midi_gain,
        INVALID,
        "gain and fades apply to audio clips only",
    );
    let updated = json!({"updated": [format!("clip:{mid}")], "flags": ["clips"]});
    fx.run(
        &SetClipProperties {
            loop_length: Some(Some(2.0)),
            ..props(&mid)
        },
        "Clip Properties",
        updated.clone(),
        json!({}),
    );
    assert_eq!(midi(fx.project(), &mid).loop_length, Some(2.0));
    fx.run(
        &SetClipProperties {
            loop_length: Some(None),
            ..props(&mid)
        },
        "Clip Properties",
        updated,
        json!({}),
    );
    assert_eq!(midi(fx.project(), &mid).loop_length, None);
    let gain = SetClipProperties {
        gain_db: Some(-1.0),
        ..props(&cid)
    };
    assert_eq!(gain.merge_key(), Some(format!("clipgain:{cid}")));
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "mirrors test_commands.py::test_note_commands"
)]
fn note_commands() {
    let mut fx = Fx::new();
    let (_, cid) = fx.midi_setup(None);
    let res = fx.run(
        &AddNotes {
            clip_id: cid.clone(),
            notes: vec![note("n_x", 72, 0.25, 1.0)],
        },
        "Add Notes",
        json!({"created": ["note:n_x"], "updated": [format!("clip:{cid}")], "flags": ["notes"]}),
        json!({"note_ids": ["n_x"], "note_count": 9}),
    );
    assert!(res["note_ids"] == json!(["n_x"]) && res["note_count"] == 9);
    fx.fails(
        &AddNotes {
            clip_id: cid.clone(),
            notes: vec![note("n_x", 73, 0.0, 1.0)],
        },
        ErrorCode::Conflict,
        "note ids already exist: ['n_x']",
    );
    let edit = |edits: Vec<NoteEdit>| EditNotes {
        clip_id: cid.clone(),
        edits,
    };
    fx.run(
        &edit(vec![
            NoteEdit {
                note_id: "n_x".into(),
                pitch: Some(74),
                velocity: Some(50),
                ..NoteEdit::default()
            },
            NoteEdit {
                note_id: "n_0".into(),
                start: Some(0.125),
                ..NoteEdit::default()
            },
        ]),
        "Edit Notes",
        json!({"updated": [format!("clip:{cid}"), "note:n_x", "note:n_0"], "flags": ["notes"]}),
        json!({}),
    );
    let c = midi(fx.project(), &cid);
    let n = c.note("n_x").expect("exists");
    assert_eq!((n.pitch, n.velocity, n.start), (74, 50, 0.25));
    assert!((c.note("n_0").expect("exists").start - 0.125).abs() < f64::EPSILON);
    fx.fails(
        &edit(vec![NoteEdit {
            note_id: "n_missing".into(),
            pitch: Some(1),
            ..NoteEdit::default()
        }]),
        NOT_FOUND,
        &format!("note 'n_missing' not found in clip '{cid}'"),
    );
    fx.fails(
        &edit(vec![NoteEdit {
            note_id: "n_x".into(),
            pitch: Some(200),
            ..NoteEdit::default()
        }]),
        INVALID,
        "note pitch must be 0-127, got 200",
    );
    let delete = |ids: &[&str]| DeleteNotes {
        clip_id: cid.clone(),
        note_ids: ids.iter().map(|id| (*id).to_owned()).collect(),
    };
    fx.run(
        &delete(&["n_x", "n_0"]),
        "Delete Notes",
        json!({"updated": [format!("clip:{cid}")], "deleted": ["note:n_x", "note:n_0"], "flags": ["notes"]}),
        json!({}),
    );
    fx.fails(&delete(&["n_x"]), NOT_FOUND, "notes not found: ['n_x']");
    fx.run(
        &ReplaceNotes {
            clip_id: cid.clone(),
            notes: vec![note("n_new", 40, 0.0, 4.0)],
            name: "Edit Notes".into(),
        },
        "Edit Notes",
        json!({"updated": [format!("clip:{cid}")], "flags": ["notes"]}),
        json!({"note_count": 1}),
    );
    let ids: Vec<&str> = midi(fx.project(), &cid)
        .notes
        .iter()
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(ids, ["n_new"]);
    let audio_track = fx.track(TrackKind::Audio, "T");
    fx.add_asset(asset(4, 48_000, "med_a"));
    let cmd = fx.audio_clip(&audio_track, "med_a", 0.0);
    let acid = cmd.clip_id.clone();
    fx.run(
        &cmd,
        "Add Audio Clip",
        json!({"created": [format!("clip:{acid}")], "updated": [format!("track:{audio_track}")], "flags": ["clips"]}),
        json!({"clip_id": acid, "start_beat": 0.0, "length_beats": 8.0, "start_seconds": 0.0, "end_seconds": 4.0}),
    );
    fx.fails(
        &AddNotes {
            clip_id: acid.clone(),
            notes: vec![note("n_1", 60, 0.0, 1.0)],
        },
        INVALID,
        &format!("clip '{acid}' is not a MIDI clip"),
    );
}

fn controls(
    clip_id: &str,
    events: Vec<MidiControlEvent>,
    start: f64,
    end: f64,
) -> SetControlEvents {
    SetControlEvents {
        clip_id: clip_id.to_owned(),
        events,
        start,
        end,
    }
}

#[test]
fn control_events_replace_range() {
    let mut fx = Fx::new();
    let (_, cid) = fx.midi_setup(None);
    let event = |beat: f64, kind: ControlKind, value: i64, number: i64| {
        MidiControlEvent::new(beat, kind, value, number, 0).expect("valid event")
    };
    let updated = json!({"updated": [format!("clip:{cid}")], "flags": ["notes"]});
    let cc: Vec<MidiControlEvent> = (0..4_i64)
        .map(|b| {
            event(
                f64::from(u8::try_from(b).expect("small")),
                ControlKind::Cc,
                10 * b,
                1,
            )
        })
        .collect();
    fx.run(
        &controls(&cid, cc, 0.0, f64::INFINITY),
        "Edit Controllers",
        updated.clone(),
        json!({}),
    );
    let replacement = vec![event(1.5, ControlKind::Cc, 99, 1)];
    fx.run(
        &controls(&cid, replacement, 1.0, 3.0),
        "Edit Controllers",
        updated.clone(),
        json!({}),
    );
    let events: Vec<(f64, i64)> = midi(fx.project(), &cid)
        .controls
        .iter()
        .map(|e| (e.beat, e.value))
        .collect();
    assert_eq!(events, [(0.0, 0), (1.5, 99), (3.0, 30)]);
    let bend = vec![event(0.0, ControlKind::PitchBend, 100, 0)];
    fx.run(
        &controls(&cid, bend, 0.0, f64::INFINITY),
        "Edit Controllers",
        updated,
        json!({}),
    );
    // Other controller kinds are untouched.
    let events: Vec<(f64, ControlKind, i64, i64)> = midi(fx.project(), &cid)
        .controls
        .iter()
        .map(|e| (e.beat, e.kind, e.value, e.number))
        .collect();
    assert_eq!(
        events,
        [
            (0.0, ControlKind::Cc, 0, 1),
            (0.0, ControlKind::PitchBend, 100, 0),
            (1.5, ControlKind::Cc, 99, 1),
            (3.0, ControlKind::Cc, 30, 1),
        ]
    );
}

// -- devices ----------------------------------------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "mirrors test_commands.py::test_add_and_edit_devices"
)]
fn add_and_edit_devices() {
    let mut fx = Fx::new();
    let tid = fx.track(TrackKind::Audio, "T");
    let added = |id: &str, track: &str| json!({"created": [format!("device:{id}")], "updated": [format!("track:{track}")], "flags": ["devices"]});
    let cmd = AddDevice {
        params: map(json!({"low_gain": 3})),
        ..fx.add_device(&tid, "sonara.eq.parametric")
    };
    let eq = cmd.device_id.clone();
    fx.run(
        &cmd,
        "Add Effect",
        added(&eq, &tid),
        json!({"device_id": eq, "index": 0, "params": with(eq_defaults(), json!({"low_gain": 3.0}))}),
    );
    let cmd = AddDevice {
        index: Some(0),
        preset: Some("Vocal Leveler".into()),
        ..fx.add_device(&tid, "sonara.dynamics.compressor")
    };
    let comp = cmd.device_id.clone();
    let res = fx.run(
        &cmd,
        "Add Effect",
        added(&comp, &tid),
        json!({"device_id": comp, "index": 0, "params": {"threshold_db": -20.0, "ratio": 3.5, "attack_ms": 5.0, "release_ms": 100.0, "knee_db": 6.0, "makeup_db": 0.0, "auto_makeup": false, "mix": 1.0, "detector": "peak", "sidechain": false, "sidechain_hpf_hz": 20.0, "sidechain_lpf_hz": 20000.0}}),
    );
    assert_eq!(res["index"], 0);
    let t = fx.project().track(&tid).expect("exists");
    let order: Vec<&str> = t.inserts.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(order, [comp.as_str(), eq.as_str()]);
    // The integer 3 is stored as a float.
    assert!(
        matches!(t.inserts[1].params["low_gain"], ParamValue::Float(v) if (v - 3.0).abs() < f64::EPSILON)
    );
    assert_eq!(t.inserts[0].preset.as_deref(), Some("Vocal Leveler"));
    let eq_params = json!([
        "hp_on",
        "hp_freq",
        "hp_slope",
        "low_on",
        "low_type",
        "low_freq",
        "low_gain",
        "low_q",
        "mid1_on",
        "mid1_freq",
        "mid1_gain",
        "mid1_q",
        "mid2_on",
        "mid2_freq",
        "mid2_gain",
        "mid2_q",
        "high_on",
        "high_type",
        "high_freq",
        "high_gain",
        "high_q",
        "lp_on",
        "lp_freq",
        "lp_slope",
        "output_db"
    ]);
    let cmd = AddDevice {
        params: map(json!({"low_gain": 99})),
        ..fx.add_device(&tid, "sonara.eq.parametric")
    };
    fx.fails_with(
        &cmd,
        INVALID,
        "parameter 'low_gain' must be within -24..24 dB",
        json!({"minimum": -24.0, "maximum": 24.0, "unit": "dB"}),
    );
    let cmd = AddDevice {
        params: map(json!({"nope": 1})),
        ..fx.add_device(&tid, "sonara.eq.parametric")
    };
    fx.fails_with(
        &cmd,
        INVALID,
        "Parametric EQ has no parameter 'nope'",
        json!({"available": eq_params}),
    );
    let cmd = fx.add_device(&tid, "sonara.synth.poly");
    fx.fails(
        &cmd,
        INVALID,
        "Sonara Poly Synth is an instrument, not an effect",
    );
    let cmd = AddDevice {
        preset: Some("No Such".into()),
        ..fx.add_device(&tid, "sonara.eq.parametric")
    };
    fx.fails_with(
        &cmd,
        NOT_FOUND,
        "Parametric EQ has no preset 'No Such'",
        json!({"available": ["Low Cut", "Vocal Presence", "Bass Tight", "Air", "Telephone"]}),
    );
    let cmd = fx.add_device(&tid, "sonara.nope");
    fx.fails_with(
        &cmd,
        NOT_FOUND,
        "unknown plugin 'sonara.nope'",
        json!({"available": ["sonara.convolver", "sonara.delay", "sonara.drums", "sonara.dynamics.compressor", "sonara.dynamics.gate", "sonara.dynamics.limiter", "sonara.eq.parametric", "sonara.filter", "sonara.reverb", "sonara.saturation", "sonara.synth.poly", "sonara.utility"]}),
    );
    let folder = fx.track(TrackKind::Folder, "T");
    assert_eq!(folder, "trk_000000000011");
    let cmd = fx.add_device(&folder, "sonara.utility");
    fx.fails(&cmd, INVALID, "folder tracks cannot hold effects");
    let params = |values: Value| SetDeviceParams {
        device_id: eq.clone(),
        values: map(values),
    };
    let updated = json!({"updated": [format!("device:{eq}")], "flags": ["devices"]});
    let out = fx.run(
        &params(json!({"low_gain": -2, "hp_on": 1})),
        "Set Parameter",
        updated.clone(),
        json!({"params": {"low_gain": -2.0, "hp_on": true}}),
    );
    assert_eq!(out["params"], json!({"low_gain": -2.0, "hp_on": true}));
    fx.fails_with(
        &params(json!({"hp_slope": "13"})),
        INVALID,
        "parameter 'hp_slope' must be one of ['12', '24', '36', '48']",
        json!({"choices": ["12", "24", "36", "48"]}),
    );
    // An enum by index.
    fx.run(
        &params(json!({"hp_slope": 1})),
        "Set Parameter",
        updated.clone(),
        json!({"params": {"hp_slope": "24"}}),
    );
    let d = fx.project().device(&eq).expect("exists").1;
    assert!(d.params["hp_slope"] == ParamValue::Str("24".into()) && d.preset.is_none());
    fx.run(
        &SetDeviceEnabled {
            device_id: eq.clone(),
            enabled: false,
        },
        "Bypass Effect",
        updated.clone(),
        json!({}),
    );
    assert!(!fx.project().device(&eq).expect("exists").1.enabled);
    fx.run(
        &ApplyPreset { device_id: eq.clone(), preset: "Low Cut".into() },
        "Load Preset",
        updated.clone(),
        json!({"params": {"hp_on": true, "hp_freq": 100.0, "hp_slope": "24", "low_on": true, "low_type": "shelf", "low_freq": 100.0, "low_gain": 0.0, "low_q": 0.71, "mid1_on": true, "mid1_freq": 500.0, "mid1_gain": 0.0, "mid1_q": 1.0, "mid2_on": true, "mid2_freq": 2500.0, "mid2_gain": 0.0, "mid2_q": 1.0, "high_on": true, "high_type": "shelf", "high_freq": 8000.0, "high_gain": 0.0, "high_q": 0.71, "lp_on": false, "lp_freq": 18000.0, "lp_slope": "12", "output_db": 0.0}}),
    );
    let d = fx.project().device(&eq).expect("exists").1;
    assert_eq!(d.preset.as_deref(), Some("Low Cut"));
    assert!(
        d.params["low_gain"] == ParamValue::Float(0.0)
            && d.params["hp_on"] == ParamValue::Bool(true)
    );
    fx.run(
        &MoveDevice {
            device_id: eq.clone(),
            index: 0,
        },
        "Reorder Effects",
        json!({"updated": [format!("device:{eq}"), format!("track:{tid}")], "flags": ["devices"]}),
        json!({}),
    );
    assert_eq!(fx.project().track(&tid).expect("exists").inserts[0].id, eq);
    fx.run(
        &SetDeviceState {
            device_id: eq.clone(),
            values: map(json!({"note": "hi", "drop": null})),
        },
        "Device Settings",
        updated,
        json!({}),
    );
    assert_eq!(
        Value::Object(fx.project().device(&eq).expect("exists").1.state.clone()),
        json!({"note": "hi"})
    );
    let remove = RemoveDevice {
        device_id: eq.clone(),
    };
    fx.run(
        &remove,
        "Remove Effect",
        json!({"updated": [format!("track:{tid}")], "deleted": [format!("device:{eq}")], "flags": ["devices"]}),
        json!({}),
    );
    fx.fails_with(
        &remove,
        NOT_FOUND,
        &format!("device '{eq}' not found"),
        not_found("device", &eq),
    );
    let cmd = fx.add_device(MASTER_ID, "sonara.dynamics.limiter");
    let limiter = cmd.device_id.clone();
    assert_eq!(limiter, "dev_000000000014");
    fx.run(
        &cmd,
        "Add Effect",
        added(&limiter, MASTER_ID),
        json!({"device_id": limiter, "index": 0, "params": {"ceiling_db": -1.0, "input_gain_db": 0.0, "release_ms": 60.0, "lookahead_ms": 2.0, "true_peak": true}}),
    );
    assert_eq!(fx.project().master.inserts.len(), 1);
    let ab = SetDeviceParams {
        device_id: eq.clone(),
        values: map(json!({"a": 1, "b": 2})),
    };
    let ba = SetDeviceParams {
        device_id: eq.clone(),
        values: map(json!({"b": 3, "a": 4})),
    };
    assert_eq!(ab.merge_key(), ba.merge_key());
    assert_eq!(ab.merge_key(), Some(format!("params:{eq}:a,b")));
}

#[test]
fn instrument_commands() {
    let mut fx = Fx::new();
    let tid = fx.synth_track();
    let old = fx
        .project()
        .track(&tid)
        .ok()
        .and_then(|t| t.instrument.as_ref())
        .map(|d| d.id.clone())
        .expect("synth");
    let lane = fx.automation(&tid, Some(&old), "filter_cutoff", &[(0.0, 500.0)]);
    fx.run(
        &lane,
        "Edit Automation",
        json!({"created": ["automation_lane:aut_000000000004"], "updated": [format!("track:{tid}")], "flags": ["automation"]}),
        json!({"lane_id": "aut_000000000004", "point_count": 1, "value_range": [20.0, 20000.0]}),
    );
    fx.fails(
        &MoveDevice {
            device_id: old.clone(),
            index: 0,
        },
        INVALID,
        "the instrument is always first in the chain",
    );
    let cmd = SetInstrument {
        preset: Some("Punchy 909".into()),
        ..fx.set_instrument(&tid, "sonara.drums")
    };
    let drums = cmd.device_id.clone();
    let res = fx.run(
        &cmd,
        "Set Instrument",
        json!({"created": [format!("device:{drums}")], "updated": [format!("track:{tid}")], "deleted": [format!("device:{old}")], "flags": ["devices"]}),
        json!({"device_id": drums, "params": {"kick_tune": 55.0, "kick_decay": 0.35, "kick_punch": 0.8, "snare_tune": 1.1, "snare_tone": 0.55, "snare_decay": 0.2, "hat_tune": 1.15, "hat_decay": 1.0, "clap_decay": 0.25, "tom_tune": 1.0, "cymbal_decay": 1.6, "drive": 0.25, "kick_level_db": 0.0, "snare_level_db": -2.0, "hat_level_db": -9.0, "cymbal_level_db": -10.0, "perc_level_db": -6.0, "stereo_width": 1.0, "gain_db": -9.0}}),
    );
    let t = fx.project().track(&tid).expect("exists");
    let instrument = t.instrument.as_ref().expect("drums");
    assert_eq!(res["device_id"], json!(instrument.id));
    assert_eq!(instrument.plugin_id, "sonara.drums");
    // Automation of the replaced instrument is dropped.
    assert_eq!(t.automation.len(), 0);
    let audio = fx.track(TrackKind::Audio, "T");
    let cmd = fx.set_instrument(&audio, "sonara.synth.poly");
    fx.fails(&cmd, INVALID, "only instrument tracks hold an instrument");
    let cmd = fx.set_instrument(&tid, "sonara.reverb");
    fx.fails(&cmd, INVALID, "Reverb is an effect, not an instrument");
}

#[test]
fn sidechain_rules() {
    let mut fx = Fx::new();
    let kick = fx.track(TrackKind::Audio, "Kick");
    let bass = fx.track(TrackKind::Audio, "Bass");
    let cmd = AddDevice {
        sidechain_id: Some(kick.clone()),
        ..fx.add_device(&bass, "sonara.dynamics.compressor")
    };
    let comp = cmd.device_id.clone();
    fx.run(
        &cmd,
        "Add Effect",
        json!({"created": [format!("device:{comp}")], "updated": [format!("track:{bass}")], "flags": ["devices"]}),
        json!({"device_id": comp, "index": 0, "params": compressor_defaults()}),
    );
    assert_eq!(
        fx.project()
            .device(&comp)
            .expect("exists")
            .1
            .sidechain_id
            .as_deref(),
        Some(kick.as_str())
    );
    let cmd = AddDevice {
        sidechain_id: Some(kick.clone()),
        ..fx.add_device(&bass, "sonara.reverb")
    };
    fx.fails(
        &cmd,
        ErrorCode::Unsupported,
        "Reverb has no sidechain input",
    );
    let cmd = AddDevice {
        sidechain_id: Some("trk_x".into()),
        ..fx.add_device(&bass, "sonara.dynamics.compressor")
    };
    fx.fails_with(
        &cmd,
        NOT_FOUND,
        "track 'trk_x' not found",
        not_found("track", "trk_x"),
    );
    let cmd = fx.add_device(&bass, "sonara.eq.parametric");
    let eq = cmd.device_id.clone();
    fx.run(
        &cmd,
        "Add Effect",
        json!({"created": [format!("device:{eq}")], "updated": [format!("track:{bass}")], "flags": ["devices"]}),
        json!({"device_id": eq, "index": 1, "params": eq_defaults()}),
    );
    let sidechain = |device_id: &str, source: Option<&str>| SetSidechain {
        device_id: device_id.to_owned(),
        source_track_id: source.map(str::to_owned),
    };
    fx.fails(
        &sidechain(&eq, Some(&kick)),
        ErrorCode::Unsupported,
        "Parametric EQ has no sidechain input",
    );
    // A bus keying a compressor on a track that feeds the bus is a feedback loop.
    let bus = fx.track(TrackKind::Bus, "Bus");
    fx.run(
        &SetTrackOutput {
            track_id: bass.clone(),
            output_id: bus.clone(),
        },
        "Route Output",
        json!({"updated": [format!("track:{bass}")], "flags": ["routing"]}),
        json!({}),
    );
    fx.fails_with(
        &sidechain(&comp, Some(&bus)),
        ErrorCode::RoutingCycle,
        "routing feedback loop: Bass → Bus → Bass",
        routing_issue(&bass),
    );
    fx.fails_with(
        &sidechain(&comp, Some(MASTER_ID)),
        ErrorCode::InvalidRouting,
        "'Master' cannot be a sidechain source",
        routing_issue(&comp),
    );
    fx.run(
        &sidechain(&comp, None),
        "Set Sidechain",
        json!({"updated": [format!("device:{comp}")], "flags": ["devices", "routing"]}),
        json!({}),
    );
    assert_eq!(
        fx.project().device(&comp).expect("exists").1.sidechain_id,
        None
    );
}

// -- automation -------------------------------------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "mirrors test_commands.py::test_automation_commands"
)]
fn automation_commands() {
    let mut fx = Fx::new();
    let tid = fx.track(TrackKind::Audio, "T");
    let cmd = fx.add_device(&tid, "sonara.eq.parametric");
    let eq = cmd.device_id.clone();
    fx.run(
        &cmd,
        "Add Effect",
        json!({"created": [format!("device:{eq}")], "updated": [format!("track:{tid}")], "flags": ["devices"]}),
        json!({"device_id": eq, "index": 0, "params": eq_defaults()}),
    );
    let created = |lane: &str| json!({"created": [format!("automation_lane:{lane}")], "updated": [format!("track:{tid}")], "flags": ["automation"]});
    let points: Vec<(f64, f64)> = (0..5_u8).map(|b| (f64::from(b), -f64::from(b))).collect();
    let cmd = fx.automation(&tid, None, "volume_db", &points);
    let lane_id = cmd.lane_id.clone();
    let res = fx.run(
        &cmd,
        "Edit Automation",
        created(&lane_id),
        json!({"lane_id": lane_id, "point_count": 5, "value_range": [-144.0, 12.0]}),
    );
    assert!(res["point_count"] == 5 && res["value_range"] == json!([-144.0, 12.0]));
    let cmd = SetAutomation {
        start: Some(1.0),
        end: Some(3.0),
        ..fx.automation(&tid, None, "volume_db", &[(2.5, -20.0)])
    };
    let res = fx.run(
        &cmd,
        "Edit Automation",
        json!({"updated": [format!("automation_lane:{lane_id}")], "flags": ["automation"]}),
        json!({"lane_id": lane_id, "point_count": 3, "value_range": [-144.0, 12.0]}),
    );
    assert_eq!(res["lane_id"], json!(lane_id));
    let (_, lane) = find_lane(fx.project(), &lane_id).expect("exists");
    let beats: Vec<f64> = lane.points.iter().map(|p| p.beat).collect();
    assert_eq!(beats, [0.0, 2.5, 4.0]);
    let cmd = fx.automation(&tid, None, "pan", &[(0.0, 2.0)]);
    fx.fails(
        &cmd,
        INVALID,
        "automation value 2 at beat 0 is outside -1..1",
    );
    let cmd = fx.automation(&tid, Some(&eq), "low_freq", &[(0.0, 200.0)]);
    let freq_lane = cmd.lane_id.clone();
    assert_eq!(freq_lane, "aut_000000000008");
    fx.run(
        &cmd,
        "Edit Automation",
        created(&freq_lane),
        json!({"lane_id": freq_lane, "point_count": 1, "value_range": [20.0, 1000.0]}),
    );
    let cmd = fx.automation(&tid, Some(&eq), "low_freq", &[(0.0, 5000.0)]);
    fx.fails(
        &cmd,
        INVALID,
        "automation value 5000 at beat 0 is outside 20..1000",
    );
    let cmd = fx.automation(&tid, Some(&eq), "hp_slope", &[(0.0, 1.0)]);
    fx.fails(
        &cmd,
        ErrorCode::Unsupported,
        "parameter 'hp_slope' is not automatable",
    );
    let cmd = fx.automation(&tid, Some(&eq), "nope", &[(0.0, 1.0)]);
    fx.fails_with(
        &cmd,
        INVALID,
        "Parametric EQ has no parameter 'nope'",
        json!({"available": ["hp_on", "hp_freq", "hp_slope", "low_on", "low_type", "low_freq", "low_gain", "low_q", "mid1_on", "mid1_freq", "mid1_gain", "mid1_q", "mid2_on", "mid2_freq", "mid2_gain", "mid2_q", "high_on", "high_type", "high_freq", "high_gain", "high_q", "lp_on", "lp_freq", "lp_slope", "output_db"]}),
    );
    let cmd = fx.automation(&tid, Some("dev_missing"), "x", &[(0.0, 1.0)]);
    fx.fails(&cmd, NOT_FOUND, "device 'dev_missing' is not on track 'T'");
    let cmd = fx.automation(&tid, None, "send.snd_x.level_db", &[(0.0, 0.0)]);
    fx.fails_with(
        &cmd,
        NOT_FOUND,
        "send 'snd_x' not found",
        not_found("send", "snd_x"),
    );
    // Booleans automate as 0/1.
    let cmd = fx.automation(&tid, Some(&eq), "hp_on", &[(0.0, 1.0)]);
    let bool_lane = cmd.lane_id.clone();
    assert_eq!(bool_lane, "aut_000000000014");
    fx.run(
        &cmd,
        "Edit Automation",
        created(&bool_lane),
        json!({"lane_id": bool_lane, "point_count": 1, "value_range": [0.0, 1.0]}),
    );
    let lane_changed =
        json!({"updated": [format!("automation_lane:{lane_id}")], "flags": ["automation"]});
    fx.run(
        &SetLaneEnabled {
            lane_id: lane_id.clone(),
            enabled: false,
        },
        "Toggle Automation",
        lane_changed,
        json!({}),
    );
    assert!(!find_lane(fx.project(), &lane_id).expect("exists").1.enabled);
    let delete = DeleteAutomationLane {
        lane_id: lane_id.clone(),
    };
    fx.run(
        &delete,
        "Delete Automation",
        json!({"deleted": [format!("automation_lane:{lane_id}")], "flags": ["automation"]}),
        json!({}),
    );
    fx.fails_with(
        &delete,
        NOT_FOUND,
        &format!("automation lane '{lane_id}' not found"),
        not_found("automation lane", &lane_id),
    );
}

// -- error paths and cases test_commands.py does not cover (checked against the prototype) ---

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one scenario per command family, as in the prototype tests"
)]
fn project_command_error_paths() {
    let mut fx = Fx::new();
    fx.fails(
        &RenameProject { name: "  ".into() },
        INVALID,
        "project name must not be empty",
    );
    fx.fails(
        &RemoveTempoPoint { beat: 0.0 },
        INVALID,
        "the tempo point at beat 0 cannot be removed",
    );
    fx.fails(
        &SetTempo {
            bpm: 120.0,
            beat: -1.0,
            ramp: false,
            clear_changes: false,
        },
        INVALID,
        "tempo point beat must be >= 0",
    );
    fx.fails(
        &RemoveTimeSignature { bar: 0 },
        INVALID,
        "the time signature at bar 0 cannot be removed",
    );
    fx.fails(
        &RemoveTimeSignature { bar: 3 },
        NOT_FOUND,
        "no time signature at bar 4",
    );
    fx.fails(
        &SetTimeSignature {
            numerator: -3,
            denominator: 4,
            bar: 0,
        },
        INVALID,
        "time signature numerator must be within 1-64",
    );
    fx.fails(
        &SetTimeSignature {
            numerator: 3,
            denominator: 4,
            bar: -1,
        },
        INVALID,
        "time signature bar must be >= 0",
    );
    fx.fails(
        &SetProjectLength {
            length_beats: Some(0.0),
        },
        INVALID,
        "project length must be > 0",
    );
    fx.fails(
        &SetSampleRate { sample_rate: -1 },
        INVALID,
        "sample rate must be one of (22050, 32000, 44100, 48000, 88200, 96000, 176400, 192000)",
    );
    let transport = |values: Value| SetTransportSettings {
        values: map(values),
    };
    fx.fails(
        &transport(json!({"count_in_bars": 5})),
        INVALID,
        "count-in must be 0-4 bars",
    );
    fx.fails(
        &transport(json!({"punch_in": 20.0})),
        INVALID,
        "punch-in must be >= 0 and before punch-out",
    );
    let metadata = SetMetadata {
        values: [("zeta", "1"), ("alpha", "2"), ("title", "x")]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect(),
    };
    fx.fails(
        &metadata,
        INVALID,
        "unknown metadata fields ['alpha', 'zeta']",
    );
    let cmd = fx.add_marker(-1.0, "x");
    fx.fails(&cmd, INVALID, "marker beat must be >= 0");
    let cmd = AddMarker {
        color: Some("red".into()),
        ..fx.add_marker(1.0, "x")
    };
    fx.fails(&cmd, INVALID, "color must be '#rrggbb', got 'red'");
    let cmd = AddMarker {
        color: Some("#00ff00".into()),
        ..fx.add_marker(1.0, "M")
    };
    let mid = cmd.marker_id.clone();
    assert_eq!(mid, "mrk_000000000004");
    let marker = json!({"updated": [format!("marker:{mid}")], "flags": ["markers"]});
    fx.run(
        &cmd,
        "Add Marker",
        json!({"created": [format!("marker:{mid}")], "flags": ["markers"]}),
        json!({"marker_id": mid}),
    );
    let update = |beat: Option<f64>, color: Option<&str>| UpdateMarker {
        marker_id: mid.clone(),
        beat,
        name: None,
        color: color.map(str::to_owned),
    };
    fx.fails(
        &update(Some(-1.0), None),
        INVALID,
        "marker beat must be >= 0",
    );
    fx.run(
        &update(None, Some("#0000ff")),
        "Edit Marker",
        marker,
        json!({}),
    );
    let m = fx.project().find_marker(&mid).expect("exists");
    assert_eq!(
        (m.beat, m.name.as_str(), m.color.as_deref()),
        (1.0, "M", Some("#0000ff"))
    );
    let section = |section_id: &str, name: Option<&str>, length: Option<f64>| UpdateSection {
        section_id: section_id.to_owned(),
        name: name.map(str::to_owned),
        start: None,
        length,
        color: None,
    };
    fx.fails_with(
        &section("sec_x", Some("x"), None),
        NOT_FOUND,
        "section 'sec_x' not found",
        not_found("section", "sec_x"),
    );
    let cmd = fx.add_section("S", 0.0, 0.0);
    fx.fails(&cmd, INVALID, "section length must be > 0");
    let cmd = fx.add_section("S", 0.0, 4.0);
    let sid = cmd.section_id.clone();
    assert_eq!(sid, "sec_000000000006");
    fx.run(
        &cmd,
        "Add Section",
        json!({"created": [format!("section:{sid}")], "flags": ["markers"]}),
        json!({"section_id": sid}),
    );
    fx.fails(
        &section(&sid, None, Some(0.0)),
        INVALID,
        "section length must be > 0",
    );
    fx.fails(
        &section(&sid, Some(" "), None),
        INVALID,
        "section name must not be empty",
    );
    fx.fails_with(
        &RemoveAsset {
            asset_id: "med_x".into(),
        },
        NOT_FOUND,
        "asset 'med_x' not found",
        not_found("asset", "med_x"),
    );
    // Editor state has no error path of its own; an empty change still commits.
    fx.run(
        &SetEditorState {
            values: JsonMap::new(),
        },
        "Editor Settings",
        json!({"flags": ["editor"]}),
        json!({}),
    );
    fx.run(
        &transport(json!({"count_in_bars": 4, "metronome": true, "punch_enabled": true, "punch_in": 1.0, "punch_out": 2.0})),
        "Transport Settings",
        json!({"updated": ["project:project"], "flags": ["transport"]}),
        json!({}),
    );
    let t = fx.project().transport;
    assert_eq!(
        (
            t.loop_enabled,
            t.loop_start,
            t.loop_end,
            t.punch_enabled,
            t.punch_in,
            t.punch_out,
            t.metronome,
            t.count_in_bars
        ),
        (false, 0.0, 16.0, true, 1.0, 2.0, true, 4)
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one scenario per command family, as in the prototype tests"
)]
fn track_command_error_paths() {
    let mut fx = Fx::new();
    let tid = fx.track(TrackKind::Audio, "T");
    let missing = || not_found("track", "trk_x");
    let no_track = "track 'trk_x' not found";
    fx.fails_with(
        &RenameTrack {
            track_id: "trk_x".into(),
            name: "x".into(),
        },
        NOT_FOUND,
        no_track,
        missing(),
    );
    fx.fails(
        &RenameTrack {
            track_id: tid.clone(),
            name: "   ".into(),
        },
        INVALID,
        "track name must not be empty",
    );
    fx.fails(
        &RenameTrack {
            track_id: tid.clone(),
            name: "x".repeat(201),
        },
        INVALID,
        "track name is too long",
    );
    fx.fails_with(
        &SetTrackColor {
            track_id: "trk_x".into(),
            color: None,
        },
        NOT_FOUND,
        no_track,
        missing(),
    );
    fx.fails_with(
        &MoveTrack {
            track_id: "trk_x".into(),
            index: 0,
        },
        NOT_FOUND,
        no_track,
        missing(),
    );
    fx.fails(
        &MoveTrack {
            track_id: MASTER_ID.into(),
            index: 0,
        },
        INVALID,
        "the master track cannot be deleted",
    );
    let mixer = |track_id: &str, pan: f64| SetTrackMixer {
        track_id: track_id.to_owned(),
        pan: Some(pan),
        ..SetTrackMixer::default()
    };
    fx.fails_with(&mixer("trk_x", 0.0), NOT_FOUND, no_track, missing());
    fx.fails(
        &mixer(&tid, -1.5),
        INVALID,
        "pan must be within -1 (left) .. +1 (right)",
    );
    let output = SetTrackOutput {
        track_id: "trk_x".into(),
        output_id: MASTER_ID.into(),
    };
    fx.fails_with(&output, NOT_FOUND, no_track, missing());
    let bus = fx.track(TrackKind::Bus, "Bus");
    let cmd = fx.add_send("trk_x", &bus, 0.0);
    fx.fails_with(&cmd, NOT_FOUND, no_track, missing());
    let cmd = fx.add_send(&tid, "trk_x", 0.0);
    fx.fails_with(&cmd, NOT_FOUND, no_track, missing());
    let cmd = fx.add_send(&tid, &bus, 13.0);
    fx.fails(&cmd, INVALID, "send level must be within -144..+12 dB");
    let cmd = fx.add_send(&tid, MASTER_ID, 0.0);
    fx.fails(
        &cmd,
        ErrorCode::InvalidRouting,
        "sends must target a bus track",
    );
    let cmd = AddSend {
        pre_fader: true,
        ..fx.add_send(&tid, &bus, -3.0)
    };
    let send_id = cmd.send_id.clone();
    assert_eq!(send_id, "snd_000000000010");
    fx.run(
        &cmd,
        "Add Send",
        json!({"created": [format!("send:{send_id}")], "updated": [format!("track:{tid}")], "flags": ["routing"]}),
        json!({"send_id": send_id}),
    );
    let update_send = |send_id: &str, level_db: Option<f64>, enabled: Option<bool>| UpdateSend {
        track_id: tid.clone(),
        send_id: send_id.to_owned(),
        level_db,
        pre_fader: None,
        enabled,
    };
    fx.fails_with(
        &update_send("snd_x", Some(0.0), None),
        NOT_FOUND,
        "send 'snd_x' not found",
        not_found("send", "snd_x"),
    );
    fx.fails(
        &update_send(&send_id, Some(13.0), None),
        INVALID,
        "send level must be within -144..+12 dB",
    );
    fx.run(
        &update_send(&send_id, None, Some(false)),
        "Send Level",
        json!({"updated": [format!("send:{send_id}"), format!("track:{tid}")], "flags": ["mixer"]}),
        json!({}),
    );
    let s = fx
        .project()
        .track(&tid)
        .and_then(|t| t.send(&send_id))
        .expect("exists");
    assert_eq!((s.level_db, s.pre_fader, s.enabled), (-3.0, true, false));
    assert_eq!(
        update_send(&send_id, Some(1.0), None).merge_key(),
        Some(format!("send:{send_id}"))
    );
    assert_eq!(
        update_send(&send_id, Some(1.0), Some(true)).merge_key(),
        None
    );
    fx.fails_with(
        &RemoveSend {
            track_id: "trk_x".into(),
            send_id: send_id.clone(),
        },
        NOT_FOUND,
        no_track,
        missing(),
    );
    fx.fails_with(
        &SetTrackParent {
            track_id: tid.clone(),
            parent_id: Some("trk_x".into()),
        },
        ErrorCode::InvalidRouting,
        "parent of 'T' must be a folder track",
        routing_issue(&tid),
    );
    fx.fails_with(
        &SetTrackParent {
            track_id: "trk_x".into(),
            parent_id: None,
        },
        NOT_FOUND,
        no_track,
        missing(),
    );
    let input = |track_id: &str, channels: Option<Vec<i64>>| SetTrackInput {
        track_id: track_id.to_owned(),
        armed: Some(true),
        monitor: None,
        channels,
    };
    fx.fails(
        &input(&tid, Some(vec![])),
        INVALID,
        "track input uses 1 or 2 device channels",
    );
    fx.fails(
        &input(&tid, Some(vec![-1])),
        INVALID,
        "input channels must be >= 0",
    );
    fx.fails_with(&input("trk_x", None), NOT_FOUND, no_track, missing());
    fx.fails_with(
        &DeleteTrack {
            track_id: "trk_x".into(),
        },
        NOT_FOUND,
        no_track,
        missing(),
    );
    // A missing or non-folder parent is a routing issue of the new track.
    let cmd = CreateTrack {
        parent_id: Some("trk_x".into()),
        ..fx.create_track("P", TrackKind::Audio)
    };
    fx.fails_with(
        &cmd,
        ErrorCode::InvalidRouting,
        "parent of 'P' must be a folder track",
        routing_issue("trk_000000000011"),
    );
    let cmd = CreateTrack {
        parent_id: Some(tid.clone()),
        ..fx.create_track("P", TrackKind::Audio)
    };
    fx.fails_with(
        &cmd,
        ErrorCode::InvalidRouting,
        "parent of 'P' must be a folder track",
        routing_issue("trk_000000000013"),
    );
    let cmd = CreateTrack {
        instrument_plugin: Some("sonara.synth.poly".into()),
        instrument_preset: Some("Nope".into()),
        ..fx.create_track("I", TrackKind::Instrument)
    };
    fx.fails_with(
        &cmd,
        NOT_FOUND,
        "Sonara Poly Synth has no preset 'Nope'",
        json!({"available": ["Init", "Analog Bass", "Sub Bass", "Acid Bass", "Warm Pad", "Strings", "Brass", "Pluck", "Supersaw Lead", "Square Lead", "Electric Keys", "Glass Bells", "Choir Pad"]}),
    );
    // A blank name becomes "Track"; a negative index inserts at the top.
    let cmd = CreateTrack {
        color: Some("#123456".into()),
        index: Some(-5),
        ..fx.create_track("  ", TrackKind::Bus)
    };
    assert_eq!(fx.add_track(&cmd), "trk_000000000017");
    let tracks: Vec<(&str, &str, Option<&str>)> = fx
        .project()
        .tracks
        .iter()
        .map(|t| (t.id.as_str(), t.name.as_str(), t.color.as_deref()))
        .collect();
    assert_eq!(
        tracks,
        [
            ("trk_000000000017", "Track", Some("#123456")),
            ("trk_000000000002", "T", None),
            ("trk_000000000004", "Bus", None),
        ]
    );
    let cmd = CreateTrack {
        color: Some("blue".into()),
        ..fx.create_track("C", TrackKind::Audio)
    };
    fx.fails(&cmd, INVALID, "color must be '#rrggbb', got 'blue'");
}

#[test]
fn delete_track_with_content() {
    let mut fx = Fx::new();
    let tid = fx.synth_track();
    let cmd = fx.add_device(&tid, "sonara.eq.parametric");
    let eq = cmd.device_id.clone();
    fx.run(
        &cmd,
        "Add Effect",
        json!({"created": [format!("device:{eq}")], "updated": [format!("track:{tid}")], "flags": ["devices"]}),
        json!({"device_id": eq, "index": 0, "params": eq_defaults()}),
    );
    for start in [0.0, 4.0] {
        let cmd = fx.midi_clip(&tid, start, 4.0);
        let cid = cmd.clip_id.clone();
        fx.run(
            &cmd,
            "Create MIDI Clip",
            json!({"created": [format!("clip:{cid}")], "updated": [format!("track:{tid}")], "flags": ["clips", "notes"]}),
            json!({"clip_id": cid, "start_beat": start, "length_beats": 4.0, "start_seconds": start / 2.0, "end_seconds": start / 2.0 + 2.0, "note_ids": []}),
        );
    }
    // The track, its clips, then its instrument and inserts.
    fx.run(
        &DeleteTrack { track_id: tid.clone() },
        "Delete Track",
        json!({"deleted": [format!("track:{tid}"), "clip:clp_000000000005", "clip:clp_000000000006", "device:dev_000000000003", format!("device:{eq}")], "flags": ["clips", "routing", "tracks"]}),
        json!({"deleted_clips": 2, "rerouted_tracks": []}),
    );
    // An empty transaction changes nothing and does not commit.
    let empty = Transaction {
        commands: Vec::new(),
        name: "Nothing".into(),
    };
    let res = fx.exec(&empty);
    assert!(!res.committed && !res.dry_run && res.version == 6);
    check(&res, "Nothing", &json!({}), &json!({"steps": []}));
    assert_eq!(
        fx.session.history().labels().0,
        [
            "Create Track",
            "Add Effect",
            "Create MIDI Clip",
            "Create MIDI Clip",
            "Delete Track"
        ]
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one scenario per command family, as in the prototype tests"
)]
fn clip_command_error_paths() {
    let mut fx = Fx::new();
    fx.add_asset(asset(4, 48_000, "med_a"));
    let aid = fx.track(TrackKind::Audio, "T");
    let cmd = AddAudioClip {
        length: Some(4.0),
        ..fx.audio_clip(&aid, "med_a", 0.0)
    };
    let cid = cmd.clip_id.clone();
    fx.run(
        &cmd,
        "Add Audio Clip",
        json!({"created": [format!("clip:{cid}")], "updated": [format!("track:{aid}")], "flags": ["clips"]}),
        json!({"clip_id": cid, "start_beat": 0.0, "length_beats": 4.0, "start_seconds": 0.0, "end_seconds": 2.0}),
    );
    let no_clip = || not_found("clip", "clp_x");
    let no_track = || not_found("track", "trk_x");
    let cmd = fx.audio_clip("trk_x", "med_a", 0.0);
    fx.fails_with(&cmd, NOT_FOUND, "track 'trk_x' not found", no_track());
    let cmd = AddAudioClip {
        source_offset: -1,
        ..fx.audio_clip(&aid, "med_a", 0.0)
    };
    fx.fails(&cmd, INVALID, "source offset must be >= 0");
    let cmd = AddAudioClip {
        gain_db: 30.0,
        ..fx.audio_clip(&aid, "med_a", 0.0)
    };
    fx.fails(&cmd, INVALID, "clip gain must be within -96..+24 dB");
    let cmd = fx.midi_clip(&aid, 0.0, 4.0);
    fx.fails(&cmd, INVALID, "track 'T' (audio) cannot hold MIDI clips");
    let iid = fx.track(TrackKind::Instrument, "Keys");
    let cmd = fx.midi_clip(&iid, 0.0, 0.0);
    fx.fails(&cmd, INVALID, "clip length must be > 0");
    let cmd = CreateMidiClip {
        loop_length: Some(0.0),
        ..fx.midi_clip(&iid, 0.0, 4.0)
    };
    fx.fails(&cmd, INVALID, "loop length must be > 0");
    let cmd = CreateMidiClip {
        color: Some("red".into()),
        ..fx.midi_clip(&iid, 0.0, 4.0)
    };
    fx.fails(&cmd, INVALID, "color must be '#rrggbb', got 'red'");
    let cmd = fx.midi_clip("trk_x", 0.0, 4.0);
    fx.fails_with(&cmd, NOT_FOUND, "track 'trk_x' not found", no_track());
    let cmd = CreateMidiClip {
        name: "Keys".into(),
        notes: vec![note("n_b", 64, 1.0, 1.0), note("n_a", 60, 0.0, 1.0)],
        color: Some("#abcdef".into()),
        loop_length: Some(2.0),
        ..fx.midi_clip(&iid, 0.0, 4.0)
    };
    let mid = cmd.clip_id.clone();
    assert_eq!(mid, "clp_000000000015");
    // The note IDs come back sorted by start.
    fx.run(
        &cmd,
        "Create MIDI Clip",
        json!({"created": [format!("clip:{mid}")], "updated": [format!("track:{iid}")], "flags": ["clips", "notes"]}),
        json!({"clip_id": mid, "start_beat": 0.0, "length_beats": 4.0, "start_seconds": 0.0, "end_seconds": 2.0, "note_ids": ["n_a", "n_b"]}),
    );
    let move_clip = |clip_id: &str, start: Option<f64>, track_id: Option<&str>| MoveClip {
        clip_id: clip_id.to_owned(),
        start,
        track_id: track_id.map(str::to_owned),
    };
    fx.fails_with(
        &move_clip("clp_x", Some(1.0), None),
        NOT_FOUND,
        "clip 'clp_x' not found",
        no_clip(),
    );
    fx.fails(
        &move_clip(&cid, Some(-1.0), None),
        INVALID,
        "clip start must be >= 0",
    );
    fx.fails_with(
        &move_clip(&cid, None, Some("trk_x")),
        NOT_FOUND,
        "track 'trk_x' not found",
        no_track(),
    );
    fx.run(
        &move_clip(&cid, Some(2.0), None),
        "Move Clip",
        json!({"updated": [format!("clip:{cid}"), format!("track:{aid}")], "flags": ["clips"]}),
        json!({"clip_id": cid, "start_beat": 2.0, "length_beats": 4.0, "start_seconds": 1.0, "end_seconds": 3.0}),
    );
    assert_eq!(
        move_clip(&cid, None, None).merge_key(),
        Some(format!("move:{cid}"))
    );
    assert_eq!(
        trim(&cid, None, None).merge_key(),
        Some(format!("trim:{cid}"))
    );
    let no_such = "clip 'clp_x' not found";
    fx.fails_with(
        &MoveClips {
            clip_ids: vec!["clp_x".into()],
            delta_beats: 1.0,
        },
        NOT_FOUND,
        no_such,
        no_clip(),
    );
    fx.fails_with(
        &trim("clp_x", Some(1.0), None),
        NOT_FOUND,
        no_such,
        no_clip(),
    );
    let cmd = fx.split("clp_x", 1.0);
    fx.fails_with(&cmd, NOT_FOUND, no_such, no_clip());
    let cmd = fx.duplicate("clp_x");
    fx.fails_with(&cmd, NOT_FOUND, no_such, no_clip());
    let cmd = DuplicateClip {
        track_id: Some("trk_x".into()),
        ..fx.duplicate(&cid)
    };
    fx.fails_with(&cmd, NOT_FOUND, "track 'trk_x' not found", no_track());
    let cmd = DuplicateClip {
        start: Some(8.0),
        ..fx.duplicate(&mid)
    };
    let copy = cmd.new_clip_id.clone();
    assert_eq!(copy, "clp_000000000019");
    fx.run(
        &cmd,
        "Duplicate Clip",
        json!({"created": [format!("clip:{copy}")], "updated": [format!("track:{iid}")], "flags": ["clips"]}),
        json!({"clip_id": copy, "start_beat": 8.0, "length_beats": 4.0, "start_seconds": 4.0, "end_seconds": 6.0}),
    );
    let props = |clip_id: &str| SetClipProperties {
        clip_id: clip_id.to_owned(),
        ..SetClipProperties::default()
    };
    fx.fails_with(
        &SetClipProperties {
            name: Some("x".into()),
            ..props("clp_x")
        },
        NOT_FOUND,
        no_such,
        no_clip(),
    );
    fx.fails(
        &SetClipProperties {
            color: Some("red".into()),
            ..props(&cid)
        },
        INVALID,
        "color must be '#rrggbb', got 'red'",
    );
    fx.fails(
        &SetClipProperties {
            gain_db: Some(25.0),
            ..props(&cid)
        },
        INVALID,
        "clip gain must be within -96..+24 dB",
    );
    let cmd = SetClipProperties {
        name: Some("Lead".into()),
        muted: Some(true),
        color: Some("#000000".into()),
        ..props(&mid)
    };
    fx.run(
        &cmd,
        "Clip Properties",
        json!({"updated": [format!("clip:{mid}")], "flags": ["clips"]}),
        json!({}),
    );
    let c = midi(fx.project(), &mid);
    assert_eq!(
        (c.name.as_str(), c.muted, c.color.as_deref()),
        ("Lead", true, Some("#000000"))
    );
    let gain_and_fade = SetClipProperties {
        gain_db: Some(1.0),
        fade_in: Some(fade(0.1)),
        ..props(&cid)
    };
    assert_eq!(gain_and_fade.merge_key(), None);
    let not_midi = format!("clip '{cid}' is not a MIDI clip");
    fx.fails(
        &ReplaceNotes {
            clip_id: cid.clone(),
            notes: Vec::new(),
            name: "Edit Notes".into(),
        },
        INVALID,
        &not_midi,
    );
    fx.fails(
        &controls(&cid, Vec::new(), 0.0, f64::INFINITY),
        INVALID,
        &not_midi,
    );
    fx.fails(
        &EditNotes {
            clip_id: cid.clone(),
            edits: Vec::new(),
        },
        INVALID,
        &not_midi,
    );
    fx.fails(
        &DeleteNotes {
            clip_id: cid.clone(),
            note_ids: Vec::new(),
        },
        INVALID,
        &not_midi,
    );
    fx.fails_with(
        &AddNotes {
            clip_id: "clp_x".into(),
            notes: Vec::new(),
        },
        NOT_FOUND,
        no_such,
        no_clip(),
    );
    // At most five IDs are listed; missing ones are sorted.
    let many: Vec<MidiNote> = "abcdefg"
        .chars()
        .map(|c| note(&format!("n_{c}"), 60, 0.0, 1.0))
        .collect();
    fx.fails(
        &AddNotes {
            clip_id: mid.clone(),
            notes: many,
        },
        ErrorCode::Conflict,
        "note ids already exist: ['n_a', 'n_b']",
    );
    let doomed = ["n_z", "n_y", "n_a", "n_x", "n_w", "n_v", "n_u"]
        .map(str::to_owned)
        .to_vec();
    fx.fails(
        &DeleteNotes {
            clip_id: mid.clone(),
            note_ids: doomed,
        },
        NOT_FOUND,
        "notes not found: ['n_u', 'n_v', 'n_w', 'n_x', 'n_y']",
    );
    let edits = |edits: Vec<NoteEdit>| EditNotes {
        clip_id: mid.clone(),
        edits,
    };
    let ids = |ids: &[&str]| -> Vec<NoteEdit> {
        ids.iter()
            .map(|id| NoteEdit {
                note_id: (*id).to_owned(),
                ..NoteEdit::default()
            })
            .collect()
    };
    assert_eq!(
        edits(ids(&["n_b", "n_a"])).merge_key(),
        Some(format!("notes:{mid}:n_a,n_b"))
    );
    let edit = NoteEdit {
        note_id: "n_b".into(),
        start: Some(0.0),
        length: Some(2.0),
        channel: Some(3),
        ..NoteEdit::default()
    };
    fx.run(
        &edits(vec![edit]),
        "Edit Notes",
        json!({"updated": [format!("clip:{mid}"), "note:n_b"], "flags": ["notes"]}),
        json!({}),
    );
    let notes: Vec<(&str, u8, f64, f64, u8, u8)> = midi(fx.project(), &mid)
        .notes
        .iter()
        .map(|n| {
            (
                n.id.as_str(),
                n.pitch,
                n.start,
                n.length,
                n.velocity,
                n.channel,
            )
        })
        .collect();
    assert_eq!(
        notes,
        [("n_a", 60, 0.0, 1.0, 100, 0), ("n_b", 64, 0.0, 2.0, 100, 3)]
    );
    fx.run(
        &ReplaceNotes {
            clip_id: mid.clone(),
            notes: Vec::new(),
            name: "Clear".into(),
        },
        "Clear",
        json!({"updated": [format!("clip:{mid}")], "flags": ["notes"]}),
        json!({"note_count": 0}),
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one scenario per command family, as in the prototype tests"
)]
fn device_command_error_paths() {
    let mut fx = Fx::new();
    let tid = fx.track(TrackKind::Audio, "T");
    let cmd = AddDevice {
        name: Some("My EQ".into()),
        ..fx.add_device(&tid, "sonara.eq.parametric")
    };
    let eq = cmd.device_id.clone();
    fx.run(
        &cmd,
        "Add Effect",
        json!({"created": [format!("device:{eq}")], "updated": [format!("track:{tid}")], "flags": ["devices"]}),
        json!({"device_id": eq, "index": 0, "params": eq_defaults()}),
    );
    assert_eq!(fx.project().device(&eq).expect("exists").1.name, "My EQ");
    let no_device = || not_found("device", "dev_x");
    let missing = "device 'dev_x' not found";
    let cmd = fx.add_device("trk_x", "sonara.eq.parametric");
    fx.fails_with(
        &cmd,
        NOT_FOUND,
        "track 'trk_x' not found",
        not_found("track", "trk_x"),
    );
    fx.fails_with(
        &SetDeviceEnabled {
            device_id: "dev_x".into(),
            enabled: false,
        },
        NOT_FOUND,
        missing,
        no_device(),
    );
    fx.fails_with(
        &SetDeviceState {
            device_id: "dev_x".into(),
            values: JsonMap::new(),
        },
        NOT_FOUND,
        missing,
        no_device(),
    );
    fx.fails_with(
        &ApplyPreset {
            device_id: "dev_x".into(),
            preset: "Low Cut".into(),
        },
        NOT_FOUND,
        missing,
        no_device(),
    );
    fx.fails_with(
        &ApplyPreset {
            device_id: eq.clone(),
            preset: "Nope".into(),
        },
        NOT_FOUND,
        "Parametric EQ has no preset 'Nope'",
        json!({"available": ["Low Cut", "Vocal Presence", "Bass Tight", "Air", "Telephone"]}),
    );
    fx.fails_with(
        &SetDeviceParams {
            device_id: "dev_x".into(),
            values: JsonMap::new(),
        },
        NOT_FOUND,
        missing,
        no_device(),
    );
    fx.fails_with(
        &SetDeviceParams { device_id: eq.clone(), values: map(json!({"nope": 1})) },
        INVALID,
        "Parametric EQ has no parameter 'nope'",
        json!({"available": ["hp_on", "hp_freq", "hp_slope", "low_on", "low_type", "low_freq", "low_gain", "low_q", "mid1_on", "mid1_freq", "mid1_gain", "mid1_q", "mid2_on", "mid2_freq", "mid2_gain", "mid2_q", "high_on", "high_type", "high_freq", "high_gain", "high_q", "lp_on", "lp_freq", "lp_slope", "output_db"]}),
    );
    fx.fails_with(
        &MoveDevice {
            device_id: "dev_x".into(),
            index: 0,
        },
        NOT_FOUND,
        missing,
        no_device(),
    );
    fx.fails_with(
        &RemoveDevice {
            device_id: "dev_x".into(),
        },
        NOT_FOUND,
        missing,
        no_device(),
    );
    fx.fails_with(
        &SetSidechain {
            device_id: "dev_x".into(),
            source_track_id: None,
        },
        NOT_FOUND,
        missing,
        no_device(),
    );
    let cmd = AddDevice {
        index: Some(0),
        ..fx.add_device(&tid, "sonara.dynamics.compressor")
    };
    let comp = cmd.device_id.clone();
    fx.run(
        &cmd,
        "Add Effect",
        json!({"created": [format!("device:{comp}")], "updated": [format!("track:{tid}")], "flags": ["devices"]}),
        json!({"device_id": comp, "index": 0, "params": compressor_defaults()}),
    );
    fx.fails_with(
        &SetSidechain {
            device_id: comp.clone(),
            source_track_id: Some("trk_x".into()),
        },
        NOT_FOUND,
        "track 'trk_x' not found",
        not_found("track", "trk_x"),
    );
    // A large index moves to the end of the chain.
    fx.run(
        &MoveDevice { device_id: comp.clone(), index: 99 },
        "Reorder Effects",
        json!({"updated": [format!("device:{comp}"), format!("track:{tid}")], "flags": ["devices"]}),
        json!({}),
    );
    let order: Vec<&str> = fx
        .project()
        .track(&tid)
        .expect("exists")
        .inserts
        .iter()
        .map(|d| d.id.as_str())
        .collect();
    assert_eq!(order, [eq.as_str(), comp.as_str()]);
    let cmd = fx.set_instrument("trk_x", "sonara.drums");
    fx.fails_with(
        &cmd,
        NOT_FOUND,
        "track 'trk_x' not found",
        not_found("track", "trk_x"),
    );
    let inst = fx.track(TrackKind::Instrument, "Drums");
    let cmd = SetInstrument {
        preset: Some("Nope".into()),
        ..fx.set_instrument(&inst, "sonara.drums")
    };
    fx.fails_with(
        &cmd,
        NOT_FOUND,
        "Sonara Drum Kit has no preset 'Nope'",
        json!({"available": ["Electronic 808", "Punchy 909", "Tight House", "Cinematic Toms", "Lo-Fi"]}),
    );
    let cmd = SetInstrument {
        params: map(json!({"kick_tune": 1e9})),
        ..fx.set_instrument(&inst, "sonara.drums")
    };
    fx.fails_with(
        &cmd,
        INVALID,
        "parameter 'kick_tune' must be within 30..120 Hz",
        json!({"minimum": 30.0, "maximum": 120.0, "unit": "Hz"}),
    );
    // An instrument track without an instrument: nothing is deleted.
    let cmd = SetInstrument {
        params: map(json!({"drive": 0.5})),
        ..fx.set_instrument(&inst, "sonara.drums")
    };
    let drums = cmd.device_id.clone();
    fx.run(
        &cmd,
        "Set Instrument",
        json!({"created": [format!("device:{drums}")], "updated": [format!("track:{inst}")], "flags": ["devices"]}),
        json!({"device_id": drums, "params": drum_defaults()}),
    );
    fx.run(
        &SetDeviceState {
            device_id: eq.clone(),
            values: map(json!({"ir": {"a": [1, 2]}, "n": 3})),
        },
        "Device Settings",
        json!({"updated": [format!("device:{eq}")], "flags": ["devices"]}),
        json!({}),
    );
    let state = Value::Object(fx.project().device(&eq).expect("exists").1.state.clone());
    assert_eq!(state, json!({"ir": {"a": [1, 2]}, "n": 3}));
}

#[test]
fn automation_command_error_paths() {
    let mut fx = Fx::new();
    let tid = fx.track(TrackKind::Audio, "T");
    let cmd = fx.automation("trk_x", None, "volume_db", &[]);
    fx.fails_with(
        &cmd,
        NOT_FOUND,
        "track 'trk_x' not found",
        not_found("track", "trk_x"),
    );
    let cmd = fx.automation(&tid, None, "width", &[]);
    fx.fails(&cmd, INVALID, "unknown mixer automation parameter 'width'");
    fx.fails_with(
        &SetLaneEnabled {
            lane_id: "aut_x".into(),
            enabled: true,
        },
        NOT_FOUND,
        "automation lane 'aut_x' not found",
        not_found("automation lane", "aut_x"),
    );
    let cmd = fx.automation(&tid, None, "pan", &[(4.0, 0.5), (0.0, -0.5)]);
    let lane = cmd.lane_id.clone();
    assert_eq!(lane, "aut_000000000006");
    fx.run(
        &cmd,
        "Edit Automation",
        json!({"created": [format!("automation_lane:{lane}")], "updated": [format!("track:{tid}")], "flags": ["automation"]}),
        json!({"lane_id": lane, "point_count": 2, "value_range": [-1.0, 1.0]}),
    );
    let lane_updated =
        json!({"updated": [format!("automation_lane:{lane}")], "flags": ["automation"]});
    let points = |fx: &Fx| -> Vec<(f64, f64)> {
        fx.project().track(&tid).expect("exists").automation[0]
            .points
            .iter()
            .map(|p| (p.beat, p.value))
            .collect()
    };
    // Only `start` set: the points from `start` on are replaced.
    let cmd = SetAutomation {
        start: Some(6.0),
        ..fx.automation(&tid, None, "pan", &[(8.0, 1.0)])
    };
    fx.run(
        &cmd,
        "Edit Automation",
        lane_updated.clone(),
        json!({"lane_id": lane, "point_count": 3, "value_range": [-1.0, 1.0]}),
    );
    assert_eq!(points(&fx), [(0.0, -0.5), (4.0, 0.5), (8.0, 1.0)]);
    // Only `end` set: the points up to `end` are replaced.
    let cmd = SetAutomation {
        end: Some(4.0),
        ..fx.automation(&tid, None, "pan", &[(1.0, 0.0)])
    };
    fx.run(
        &cmd,
        "Edit Automation",
        lane_updated.clone(),
        json!({"lane_id": lane, "point_count": 2, "value_range": [-1.0, 1.0]}),
    );
    assert_eq!(points(&fx), [(1.0, 0.0), (8.0, 1.0)]);
    let cmd = fx.automation(&tid, None, "pan", &[]);
    fx.run(
        &cmd,
        "Edit Automation",
        lane_updated.clone(),
        json!({"lane_id": lane, "point_count": 0, "value_range": [-1.0, 1.0]}),
    );
    let cmd = fx.automation(MASTER_ID, None, "volume_db", &[(0.0, -3.0)]);
    let master_lane = cmd.lane_id.clone();
    fx.run(
        &cmd,
        "Edit Automation",
        json!({"created": [format!("automation_lane:{master_lane}")], "updated": ["track:master"], "flags": ["automation"]}),
        json!({"lane_id": master_lane, "point_count": 1, "value_range": [-144.0, 12.0]}),
    );
    fx.run(
        &SetLaneEnabled {
            lane_id: lane.clone(),
            enabled: false,
        },
        "Toggle Automation",
        lane_updated,
        json!({}),
    );
    fx.run(
        &DeleteAutomationLane {
            lane_id: lane.clone(),
        },
        "Delete Automation",
        json!({"deleted": [format!("automation_lane:{lane}")], "flags": ["automation"]}),
        json!({}),
    );
}

#[test]
fn no_project_open() {
    let mut session = ProjectSession::new(Arc::new(builtin_registry()));
    let no_project = (
        ErrorCode::NoProject,
        "no project is open; create or open one first",
    );
    let rename = RenameProject { name: "x".into() };
    let err = session
        .execute(&rename, false, "user")
        .expect_err("no project");
    assert_eq!((err.code, err.message.as_str()), no_project);
    let err = session.simulate(&rename).expect_err("no project");
    assert_eq!((err.code, err.message.as_str()), no_project);
    let err = session.redo("user").expect_err("no project");
    assert_eq!((err.code, err.message.as_str()), no_project);
    assert!(!session.is_dirty());
    assert_eq!(session.version(), 0);
}

/// A command, its merge key, its undo label and whether it is undoable.
type LabelCase = (Box<dyn Command>, Option<&'static str>, &'static str, bool);

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "a table of cases checked against the prototype"
)]
fn merge_keys_and_labels() {
    let tempo = |bpm: f64, beat: f64| SetTempo {
        bpm,
        beat,
        ramp: false,
        clear_changes: false,
    };
    let mixer = |volume_db: Option<f64>,
                 pan: Option<f64>,
                 mute: Option<bool>,
                 solo: Option<bool>,
                 solo_safe: Option<bool>| SetTrackMixer {
        track_id: "t".into(),
        volume_db,
        pan,
        mute,
        solo,
        solo_safe,
    };
    let props = SetClipProperties {
        clip_id: "c".into(),
        ..SetClipProperties::default()
    };
    let transport = SetTransportSettings {
        values: map(json!({"loop_start": 1.0, "loop_end": 2.0})),
    };
    let cases: Vec<LabelCase> = vec![
        (
            Box::new(tempo(120.0, 0.0)),
            Some("tempo:0.0"),
            "Set Tempo",
            true,
        ),
        (
            Box::new(tempo(90.0, 16.0)),
            Some("tempo:16.0"),
            "Set Tempo",
            true,
        ),
        (
            Box::new(tempo(90.0, 0.5)),
            Some("tempo:0.5"),
            "Set Tempo",
            true,
        ),
        (
            Box::new(transport),
            Some("transport:loop_end,loop_start"),
            "Transport Settings",
            true,
        ),
        (
            Box::new(SetTransportSettings::default()),
            Some("transport:"),
            "Transport Settings",
            true,
        ),
        (
            Box::new(SetEditorState {
                values: map(json!({"zoom": 1})),
            }),
            Some("editor"),
            "Editor Settings",
            false,
        ),
        (
            Box::new(mixer(Some(-1.0), Some(0.5), None, None, None)),
            Some("mixer:t:volume_db,pan"),
            "Track Volume",
            true,
        ),
        (
            Box::new(mixer(None, Some(0.5), None, None, None)),
            Some("mixer:t:pan"),
            "Track Pan",
            true,
        ),
        (
            Box::new(mixer(Some(-1.0), None, Some(true), None, None)),
            None,
            "Track Volume",
            true,
        ),
        (
            Box::new(mixer(None, None, None, Some(true), None)),
            None,
            "Solo",
            true,
        ),
        (
            Box::new(mixer(None, None, None, None, Some(true))),
            None,
            "Mixer",
            true,
        ),
        (
            Box::new(mixer(None, None, None, None, None)),
            None,
            "Mixer",
            true,
        ),
        (
            Box::new(UpdateSend {
                track_id: "t".into(),
                send_id: "s".into(),
                level_db: None,
                pre_fader: Some(true),
                enabled: None,
            }),
            None,
            "Send Level",
            true,
        ),
        (
            Box::new(SetClipProperties {
                name: Some("x".into()),
                ..props.clone()
            }),
            None,
            "Clip Properties",
            true,
        ),
        (
            Box::new(SetClipProperties {
                gain_db: Some(1.0),
                fade_out: Some(fade(0.1)),
                ..props
            }),
            None,
            "Clip Properties",
            true,
        ),
        (
            Box::new(EditNotes {
                clip_id: "c".into(),
                edits: Vec::new(),
            }),
            Some("notes:c:"),
            "Edit Notes",
            true,
        ),
        (
            Box::new(SetDeviceParams {
                device_id: "d".into(),
                values: JsonMap::new(),
            }),
            Some("params:d:"),
            "Set Parameter",
            true,
        ),
        (
            Box::new(RenameProject { name: "x".into() }),
            None,
            "Rename Project",
            true,
        ),
        (
            Box::new(RenameTrack {
                track_id: "t".into(),
                name: "x".into(),
            }),
            None,
            "Rename Track",
            true,
        ),
        (
            Box::new(SetMetadata::default()),
            None,
            "Edit Project Info",
            true,
        ),
    ];
    for (cmd, merge_key, label, undoable) in cases {
        assert_eq!(
            (
                cmd.merge_key().as_deref(),
                cmd.describe().as_str(),
                cmd.undoable()
            ),
            (merge_key, label, undoable),
            "{}",
            cmd.name()
        );
    }
}

#[test]
fn lifecycle_events() {
    let mut fx = Fx::new();
    let events = fx.session.subscribe();
    fx.run(
        &RenameProject { name: "A".into() },
        "Rename Project",
        json!({"updated": ["project:project"], "flags": ["metadata"]}),
        json!({}),
    );
    let project = fx.project().clone();
    fx.session.mark_saved(project, location());
    assert!(!fx.session.is_dirty());
    let packaged = ProjectLocation {
        path: Some(PathBuf::from("/tmp/x.sonara")),
        workspace: PathBuf::from("/tmp/x"),
        packaged: true,
    };
    fx.session.replace_location(packaged.clone());
    assert_eq!(fx.session.location(), Some(&packaged));
    fx.session.close();
    let next = Project::create(&mut fx.ids, "B", 48_000, 120.0).expect("valid project");
    fx.session.load(next, packaged, true);
    assert!(
        !fx.session.is_dirty() && fx.session.version() == 4 && !fx.session.history().can_undo()
    );
    let seen: Vec<(EventKind, u64, String, String, Value)> = events
        .try_iter()
        .map(|e| (e.kind, e.version, e.label, e.source, e.changes.to_json()))
        .collect();
    let user = || "user".to_owned();
    assert_eq!(
        seen,
        [
            (
                EventKind::Command,
                2,
                "Rename Project".to_owned(),
                user(),
                changes_json(&json!({"updated": ["project:project"], "flags": ["metadata"]}))
            ),
            (
                EventKind::Saved,
                2,
                String::new(),
                user(),
                changes_json(&json!({}))
            ),
            (
                EventKind::Closed,
                3,
                String::new(),
                user(),
                changes_json(&json!({"flags": ["project"]}))
            ),
            (
                EventKind::Loaded,
                4,
                String::new(),
                user(),
                changes_json(&json!({"flags": ["project"]}))
            ),
        ]
    );
}
