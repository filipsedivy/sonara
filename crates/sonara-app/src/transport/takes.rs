//! Finishing a recording (`_finish_recording` and `_midi_takes` of the prototype's player): the
//! takes become assets and clips in one "Record" transaction, one undo step,
//! and the result is the prototype's JSON.
//!
//! - **Audio takes** align with the capture start the device callback stamped on them:
//!   frame `i` of a take was heard at `capture_start + i` and arrived after the output and
//!   input latency, so the take aligns at `capture_start − latency`, and the frames before the
//!   record position (the count-in) are trimmed with `source_offset`. A take no longer than the
//!   trim is deleted.
//! - **MIDI takes** keep the notes between the record position and the stop, the same notes on
//!   every armed instrument track, and their controllers through `SetControlEvents` after each
//!   `CreateMidiClip`: the prototype counted controllers in `"controllers"` but dropped
//!   them.
//!
//! The IDs are drawn in the prototype's order: the asset IDs when the recording starts, then the
//! MIDI notes and clips, then the audio clips.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sonara_core::commands::Command;
use sonara_core::commands::clip::{AddAudioClip, CreateMidiClip, SetControlEvents};
use sonara_core::commands::project::AddAsset;
use sonara_core::error::DomainError;
use sonara_core::ids::{self, IdGenerator};
use sonara_core::midi::{ControlKind, MidiControlEvent, MidiNote};
use sonara_core::model::JsonMap;
use sonara_core::model::media::{MediaAsset, MediaAssetData, MediaKind, MediaLocation};
use sonara_core::model::project::Project;
use sonara_dsp::numpy::math::py_round;
use sonara_engine::player::Recorded;
use sonara_media::export::SampleFormat;
use sonara_media::inspect_audio;
use sonara_project::atomic::sha256_file;
use sonara_rt::{MidiEvent, MidiKind};

use crate::error::{AppResult, IoContext};

/// An audio take of the running recording as the service named it.
#[derive(Debug, Clone)]
pub(crate) struct PendingTake {
    pub(crate) track_id: String,
    pub(crate) track_name: String,
    pub(crate) asset_id: String,
    /// The file's path inside the workspace, with `/` (the asset's `path`).
    pub(crate) rel: String,
}

/// `media/audio/{asset_id}-{safe_stem(track name)}-take.wav`.
pub(crate) fn take_path(asset_id: &str, track_name: &str) -> String {
    format!(
        "media/audio/{asset_id}-{}-take.wav",
        sonara_media::safe_stem(track_name, 48)
    )
}

/// The encoding of `record_subtype`: the WAV subtypes the take writer writes.
///
/// # Errors
///
/// `INVALID_ARGUMENT` for another subtype. The prototype passed it to libsndfile, which also
/// wrote `PCM_U8`, `ULAW`, `ALAW` and the ADPCM variants; no recorded project uses them.
pub(crate) fn sample_format(subtype: &str) -> Result<SampleFormat, DomainError> {
    match subtype {
        "PCM_16" => Ok(SampleFormat::Pcm16),
        "PCM_24" => Ok(SampleFormat::Pcm24),
        "PCM_32" => Ok(SampleFormat::Pcm32),
        "FLOAT" => Ok(SampleFormat::Float),
        "DOUBLE" => Ok(SampleFormat::Double),
        other => Err(DomainError::invalid(format!(
            "cannot record takes with the subtype {other}"
        ))),
    }
}

/// Commands of one transaction.
type Commands = Vec<Box<dyn Command>>;

/// The commands and the result of a finished recording (`_finish_recording` from the
/// alignment on).
pub(crate) struct Finished {
    pub(crate) commands: Commands,
    pub(crate) result: Value,
}

/// Turns `recorded` into the "Record" transaction's commands and the result JSON. `pending`
/// names the audio takes.
///
/// # Errors
///
/// The errors of inspecting and hashing a take file, deleting a take shorter than the trim,
/// and of building the notes, controllers and clips.
pub(crate) fn finish(
    project: &Project,
    recorded: &Recorded,
    pending: &[PendingTake],
    ids: &mut dyn IdGenerator,
) -> AppResult<Finished> {
    let rate = project.settings.sample_rate;
    let (mut commands, midi) = midi_takes(project, recorded, ids)?;
    let latency = i64::from(recorded.latency_frames) + i64::from(recorded.input_latency_frames);
    let capture_start = recorded.capture_start.unwrap_or(recorded.record_from);
    let aligned = capture_start - latency;
    let trim = (recorded.record_from - aligned).max(0);
    let mut audio_commands: Commands = Vec::new();
    let mut takes = Vec::new();
    for take in &recorded.takes {
        let Some(meta) = pending.iter().find(|p| p.track_id == take.track_id) else {
            continue;
        };
        if let Some(error) = &take.error {
            // The prototype's render thread died on a failed write and never finished the
            // recording; here the other takes are kept and the broken file is dropped.
            log::warn!(
                "the take of track {} was not recorded: {error}",
                meta.track_id
            );
            remove(&take.path)?;
            continue;
        }
        let frames = i64::try_from(take.frames).unwrap_or(i64::MAX);
        if frames <= trim {
            remove(&take.path)?;
            continue;
        }
        let name = format!("{} take", meta.track_name);
        let asset = asset_from_file(&take.path, &meta.rel, &meta.asset_id, &name)?;
        let start = project
            .tempo_map
            .sample_to_beat(frame_f64(aligned + trim), rate)
            .max(0.0);
        let clip = AddAudioClip {
            track_id: take.track_id.clone(),
            asset_id: asset.id.clone(),
            start,
            length: None,
            source_offset: trim,
            name: Some(name),
            gain_db: 0.0,
            clip_id: ids.new_id(ids::CLIP),
        };
        takes.push(json!({
            "track_id": take.track_id,
            "asset_id": asset.id,
            "clip_id": clip.clip_id,
            "seconds": py_round(frame_f64(frames - trim) / f64::from(rate), 3),
        }));
        audio_commands.push(Box::new(AddAsset { asset }));
        audio_commands.push(Box::new(clip));
    }
    audio_commands.append(&mut commands);
    takes.extend(midi);
    Ok(Finished {
        commands: audio_commands,
        result: json!({
            "takes": takes,
            "latency_compensation_frames": latency,
            "capture_overruns": recorded.capture_overruns,
        }),
    })
}

/// `asset_from_file`: a take file inside the workspace as an embedded audio asset.
fn asset_from_file(path: &Path, rel: &str, asset_id: &str, name: &str) -> AppResult<MediaAsset> {
    let info = inspect_audio(path)?;
    let size_bytes = std::fs::metadata(path).io("read", path)?.len();
    let sha256 = sha256_file(path).io("read", path)?;
    Ok(MediaAsset::new(MediaAssetData {
        id: asset_id.to_owned(),
        kind: MediaKind::Audio,
        name: name.to_owned(),
        path: rel.to_owned(),
        location: MediaLocation::Embedded,
        size_bytes,
        sha256: Some(sha256),
        sample_rate: info.sample_rate,
        channels: info.channels,
        frames: info.frames,
        format: info.format,
        subtype: info.subtype,
        original_path: None,
        relative_hint: None,
        metadata: JsonMap::new(),
        extra: JsonMap::new(),
    })?)
}

fn remove(path: &Path) -> AppResult<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error).io("delete", path),
        _ => Ok(()),
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "frame counts stay far below 2^53, where the conversion is exact"
)]
fn frame_f64(frame: i64) -> f64 {
    frame as f64
}

/// The notes still sounding, in the order a Python dict keeps them: a key keeps its place when
/// a note-on overwrites it, and goes to the end when it is inserted again after its note-off.
#[derive(Default)]
struct OpenNotes(Vec<((u8, u8), (f64, u8))>);

impl OpenNotes {
    fn insert(&mut self, key: (u8, u8), value: (f64, u8)) {
        match self.0.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = value,
            None => self.0.push((key, value)),
        }
    }

    fn pop(&mut self, key: (u8, u8)) -> Option<(f64, u8)> {
        let index = self.0.iter().position(|(k, _)| *k == key)?;
        Some(self.0.remove(index).1)
    }
}

/// `_midi_takes`: the recorded messages between the record position and the stop as notes and
/// controllers, one clip per armed instrument track (the same notes on each), with the
/// controllers stored through `SetControlEvents`. Returns the commands and the takes'
/// JSON.
fn midi_takes(
    project: &Project,
    recorded: &Recorded,
    ids: &mut dyn IdGenerator,
) -> AppResult<(Commands, Vec<Value>)> {
    if recorded.midi_tracks.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let rate = project.settings.sample_rate;
    let tempo = &project.tempo_map;
    let start_beat = tempo.sample_to_beat(frame_f64(recorded.record_from), rate);
    let end_beat =
        (start_beat + 0.25).max(tempo.sample_to_beat(frame_f64(recorded.stop_frame), rate));
    let length = end_beat - start_beat;
    let mut events: Vec<&(i64, MidiEvent)> = recorded.midi_events.iter().collect();
    events.sort_by_key(|(frame, _)| *frame);
    let mut notes = Vec::new();
    let mut controls = Vec::new();
    let mut open = OpenNotes::default();
    for (frame, message) in events {
        let beat = tempo.sample_to_beat(frame_f64(*frame), rate) - start_beat;
        if beat < 0.0 || beat >= length {
            continue;
        }
        let channel = message.channel();
        let key = (channel, message.note());
        match message.kind() {
            MidiKind::NoteOn => open.insert(key, (beat, message.velocity())),
            MidiKind::NoteOff => {
                if let Some((on, velocity)) = open.pop(key) {
                    notes.push(note(ids, key, on, beat - on, velocity)?);
                }
            }
            MidiKind::Cc => controls.push(MidiControlEvent::new(
                beat,
                ControlKind::Cc,
                i64::from(message.value()),
                i64::from(message.note()),
                i64::from(channel),
            )?),
            MidiKind::PitchBend => controls.push(MidiControlEvent::new(
                beat,
                ControlKind::PitchBend,
                i64::from(message.value()),
                0,
                i64::from(channel),
            )?),
            MidiKind::ChannelPressure => controls.push(MidiControlEvent::new(
                beat,
                ControlKind::ChannelPressure,
                i64::from(message.value()),
                0,
                i64::from(channel),
            )?),
            MidiKind::PolyPressure | MidiKind::Program | MidiKind::Other => {}
        }
    }
    for (key, (on, velocity)) in open.0 {
        notes.push(note(ids, key, on, length - on, velocity)?);
    }
    let mut commands: Commands = Vec::new();
    let mut takes = Vec::new();
    if notes.is_empty() && controls.is_empty() {
        return Ok((commands, takes));
    }
    for track_id in &recorded.midi_tracks {
        let Some(track) = project.find_track(track_id) else {
            continue;
        };
        let clip = CreateMidiClip {
            track_id: track_id.clone(),
            start: start_beat,
            length,
            name: format!("{} take", track.name),
            notes: notes.clone(),
            loop_length: None,
            color: None,
            clip_id: ids.new_id(ids::CLIP),
        };
        takes.push(json!({
            "track_id": track_id,
            "clip_id": clip.clip_id,
            "notes": notes.len(),
            "controllers": controls.len(),
            "beats": py_round(length, 3),
        }));
        let clip_id = clip.clip_id.clone();
        commands.push(Box::new(clip));
        if !controls.is_empty() {
            // The take keeps its controllers, in the same undo step.
            commands.push(Box::new(SetControlEvents {
                clip_id,
                events: controls.clone(),
                start: 0.0,
                end: length,
            }));
        }
    }
    Ok((commands, takes))
}

/// `MidiNote.create(pitch, on, max(length, 1e-3), max(1, velocity), channel)`.
fn note(
    ids: &mut dyn IdGenerator,
    (channel, pitch): (u8, u8),
    start: f64,
    length: f64,
    velocity: u8,
) -> Result<MidiNote, DomainError> {
    MidiNote::create(
        ids,
        i64::from(pitch),
        start,
        length.max(1e-3),
        i64::from(velocity.max(1)),
        i64::from(channel),
    )
}

/// Where a take file lives: the workspace joined with its `/`-separated path.
pub(crate) fn workspace_path(workspace: &Path, rel: &str) -> PathBuf {
    let mut path = workspace.to_path_buf();
    path.extend(rel.split('/'));
    path
}
