//! Clip and note commands (`sonara.application.commands.clip_cmds`).
//!
//! All edits are non-destructive: audio media is never modified. Trimming changes the clip's
//! window onto the source (`source_offset`); MIDI trimming changes `content_offset`.
//! Overlapping clips on one track are mixed; fades over an overlap form a crossfade.

use std::collections::BTreeSet;
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{Value, json};

use super::{ChangeSet, Command, CommandContext, Flag, ObjectKind, Outcome};
use crate::EPSILON;
use crate::error::{DomainError, ErrorCode, Result};
use crate::midi::{ControlKind, MidiControlEvent, MidiNote};
use crate::model::JsonMap;
use crate::model::clip::{AudioClip, AudioClipData, Clip, Fade, MidiClip, MidiClipData};
use crate::model::project::Project;
use crate::model::py_mod;
use crate::model::track::Track;
use crate::pyfmt::{str_list_repr, str_repr};
use crate::time::python_round;

/// Most clips on one track.
pub const MAX_CLIPS_PER_TRACK: usize = 10_000;
/// Most notes in one clip.
pub const MAX_NOTES_PER_CLIP: usize = 100_000;

/// The length in beats of `seconds` of audio starting at `start`.
#[must_use]
pub fn audio_length_beats(project: &Project, start: f64, seconds: f64) -> f64 {
    let t0 = project.tempo_map.beat_to_seconds(start);
    project.tempo_map.seconds_to_beat(t0 + seconds) - start
}

/// Source frames at `sample_rate` between two timeline beats.
#[must_use]
pub fn source_frames_between(project: &Project, sample_rate: u32, beat_a: f64, beat_b: f64) -> i64 {
    let seconds =
        project.tempo_map.beat_to_seconds(beat_b) - project.tempo_map.beat_to_seconds(beat_a);
    python_round(seconds * f64::from(sample_rate))
}

/// Seconds of audio left after `offset` frames.
#[expect(
    clippy::cast_precision_loss,
    reason = "Python's int / int division rounds the same way"
)]
fn remaining_seconds(frames: u64, offset: i64, sample_rate: u32) -> Result<f64> {
    if sample_rate == 0 {
        return Err(DomainError::invalid("the media has no sample rate"));
    }
    Ok((frames as f64 - offset as f64) / f64::from(sample_rate))
}

fn check_track_accepts(track: &Track, clip: &Clip) -> Result<()> {
    let (holds, what) = match clip {
        Clip::Audio(_) => (track.kind.holds_audio_clips(), "audio"),
        Clip::Midi(_) => (track.kind.holds_midi_clips(), "MIDI"),
    };
    if !holds {
        return Err(DomainError::invalid(format!(
            "track {} ({}) cannot hold {what} clips",
            str_repr(&track.name),
            track.kind
        )));
    }
    if track.clips.len() >= MAX_CLIPS_PER_TRACK && track.find_clip(clip.id()).is_none() {
        return Err(DomainError::new(
            ErrorCode::LimitExceeded,
            format!("a track can hold at most {MAX_CLIPS_PER_TRACK} clips"),
        ));
    }
    Ok(())
}

fn clip_json(project: &Project, clip: &Clip) -> Value {
    json!({
        "clip_id": clip.id(),
        "start_beat": clip.start(),
        "length_beats": clip.length(),
        "start_seconds": project.beat_to_seconds(clip.start()),
        "end_seconds": project.beat_to_seconds(clip.end()),
    })
}

/// `base` with the entries of `extra` appended (Python's `dict | dict`).
fn merged(mut base: Value, extra: Value) -> Value {
    if let (Value::Object(base), Value::Object(extra)) = (&mut base, extra) {
        base.extend(extra);
    }
    base
}

/// Adds an audio clip playing an asset.
#[derive(Debug, Clone)]
pub struct AddAudioClip {
    /// The audio track.
    pub track_id: String,
    /// The audio asset.
    pub asset_id: String,
    /// Timeline start in beats.
    pub start: f64,
    /// Length in beats; the rest of the media when `None`.
    pub length: Option<f64>,
    /// First source frame.
    pub source_offset: i64,
    /// Display name; the asset name by default.
    pub name: Option<String>,
    /// Clip gain in dB.
    pub gain_db: f64,
    /// The new clip's ID.
    pub clip_id: String,
}

impl Command for AddAudioClip {
    fn name(&self) -> &'static str {
        "AddAudioClip"
    }

    fn label(&self) -> &'static str {
        "Add Audio Clip"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let track = project.track(&self.track_id)?;
        let asset = project.asset(&self.asset_id)?;
        if u64::try_from(self.source_offset).is_ok_and(|offset| offset >= asset.frames) {
            return Err(DomainError::invalid(
                "source offset is beyond the end of the media",
            ));
        }
        let remaining = remaining_seconds(asset.frames, self.source_offset, asset.sample_rate)?;
        let max_length = audio_length_beats(project, self.start, remaining);
        let length = self.length.unwrap_or(max_length);
        if length > max_length + 1e-6 {
            return Err(DomainError::invalid(format!(
                "clip is longer than the remaining media ({max_length:.3} beats)"
            ))
            .with_detail("max_length_beats", max_length));
        }
        let clip = Clip::Audio(AudioClip::new(AudioClipData {
            id: self.clip_id.clone(),
            name: self
                .name
                .clone()
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| asset.name.clone()),
            asset_id: asset.id.clone(),
            start: self.start,
            length,
            source_offset: self.source_offset,
            gain_db: self.gain_db,
            fade_in: Fade::NONE,
            fade_out: Fade::NONE,
            muted: false,
            color: None,
            extra: JsonMap::new(),
        })?);
        check_track_accepts(track, &clip)?;
        let p = project.with_track(track.with_clip(clip.clone())?)?;
        let result = clip_json(&p, &clip);
        Ok(Outcome::with_result(
            p,
            ChangeSet::of(
                [(ObjectKind::Clip, self.clip_id.as_str())],
                [(ObjectKind::Track, track.id.as_str())],
                [],
                [Flag::Clips],
            ),
            result,
        ))
    }
}

/// Creates a MIDI clip, optionally with notes.
#[derive(Debug, Clone)]
pub struct CreateMidiClip {
    /// The instrument track.
    pub track_id: String,
    /// Timeline start in beats.
    pub start: f64,
    /// Length in beats.
    pub length: f64,
    /// Display name.
    pub name: String,
    /// Initial notes.
    pub notes: Vec<MidiNote>,
    /// Length of the repeated content.
    pub loop_length: Option<f64>,
    /// `#rrggbb`, or `None` for automatic.
    pub color: Option<String>,
    /// The new clip's ID.
    pub clip_id: String,
}

impl Command for CreateMidiClip {
    fn name(&self) -> &'static str {
        "CreateMidiClip"
    }

    fn label(&self) -> &'static str {
        "Create MIDI Clip"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let track = project.track(&self.track_id)?;
        check_note_count(self.notes.len())?;
        let clip = MidiClip::new(MidiClipData {
            id: self.clip_id.clone(),
            name: self.name.clone(),
            start: self.start,
            length: self.length,
            notes: Arc::new(self.notes.clone()),
            controls: Arc::new(Vec::new()),
            content_offset: 0.0,
            loop_length: self.loop_length,
            muted: false,
            color: self.color.clone(),
            extra: JsonMap::new(),
        })?;
        let note_ids: Vec<&str> = clip.notes.iter().map(|n| n.id.as_str()).collect();
        let note_ids = json!({"note_ids": note_ids});
        let clip = Clip::Midi(clip);
        check_track_accepts(track, &clip)?;
        let p = project.with_track(track.with_clip(clip.clone())?)?;
        let result = merged(clip_json(&p, &clip), note_ids);
        Ok(Outcome::with_result(
            p,
            ChangeSet::of(
                [(ObjectKind::Clip, self.clip_id.as_str())],
                [(ObjectKind::Track, track.id.as_str())],
                [],
                [Flag::Clips, Flag::Notes],
            ),
            result,
        ))
    }
}

/// Moves a clip in time and/or to another track.
#[derive(Debug, Clone)]
pub struct MoveClip {
    /// The clip.
    pub clip_id: String,
    /// New start; unchanged when `None`.
    pub start: Option<f64>,
    /// Destination track; the same track when `None`.
    pub track_id: Option<String>,
}

impl Command for MoveClip {
    fn name(&self) -> &'static str {
        "MoveClip"
    }

    fn label(&self) -> &'static str {
        "Move Clip"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (src, clip) = project.clip(&self.clip_id)?;
        let moved = clip.moved_to(self.start.unwrap_or_else(|| clip.start()))?;
        let dst_id = self.track_id.clone().unwrap_or_else(|| src.id.clone());
        let mut updated = vec![
            (ObjectKind::Clip, clip.id().to_owned()),
            (ObjectKind::Track, src.id.clone()),
        ];
        let p = if dst_id == src.id {
            project.with_track(src.with_clip(moved.clone())?)?
        } else {
            let dst = project.track(&dst_id)?;
            check_track_accepts(dst, &moved)?;
            let p = project.with_track(src.without_clip(clip.id())?)?;
            let dst = p.track(&dst_id)?.with_clip(moved.clone())?;
            updated.push((ObjectKind::Track, dst_id.clone()));
            p.with_track(dst)?
        };
        let result = clip_json(&p, &moved);
        Ok(Outcome::with_result(
            p,
            ChangeSet::of(
                [],
                updated.iter().map(|(k, id)| (*k, id.as_str())),
                [],
                [Flag::Clips],
            ),
            result,
        ))
    }

    fn merge_key(&self) -> Option<String> {
        Some(format!("move:{}", self.clip_id))
    }
}

/// Moves several clips by the same beat delta (multi-selection drag).
#[derive(Debug, Clone)]
pub struct MoveClips {
    /// The clips.
    pub clip_ids: Vec<String>,
    /// The shift in beats.
    pub delta_beats: f64,
}

impl Command for MoveClips {
    fn name(&self) -> &'static str {
        "MoveClips"
    }

    fn label(&self) -> &'static str {
        "Move Clips"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let mut p = project.clone();
        for cid in &self.clip_ids {
            let (track, clip) = p.clip(cid)?;
            let new_start = clip.start() + self.delta_beats;
            if new_start < -EPSILON {
                return Err(DomainError::invalid(
                    "clips cannot move before the project start",
                ));
            }
            let track = track.with_clip(clip.moved_to(new_start.max(0.0))?)?;
            p = p.with_track(track)?;
        }
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                self.clip_ids.iter().map(|c| (ObjectKind::Clip, c.as_str())),
                [],
                [Flag::Clips],
            ),
        ))
    }
}

/// Moves a clip's start and/or end edge without moving its content in time.
#[derive(Debug, Clone)]
pub struct TrimClip {
    /// The clip.
    pub clip_id: String,
    /// New start edge.
    pub start: Option<f64>,
    /// New end edge.
    pub end: Option<f64>,
}

impl Command for TrimClip {
    fn name(&self) -> &'static str {
        "TrimClip"
    }

    fn label(&self) -> &'static str {
        "Trim Clip"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, clip) = project.clip(&self.clip_id)?;
        let new_start = self.start.unwrap_or_else(|| clip.start());
        let new_end = self.end.unwrap_or_else(|| clip.end());
        if new_end <= new_start + EPSILON {
            return Err(DomainError::invalid("clip end must be after its start"));
        }
        let new = match clip {
            Clip::Audio(c) => {
                let asset = project.asset(&c.asset_id)?;
                let shift = source_frames_between(project, asset.sample_rate, c.start, new_start);
                let offset = c.source_offset + shift;
                if offset < 0 {
                    return Err(DomainError::invalid(
                        "cannot extend a clip before the start of its media",
                    ));
                }
                let remaining = remaining_seconds(asset.frames, offset, asset.sample_rate)?;
                let max_end = new_start + audio_length_beats(project, new_start, remaining);
                if new_end > max_end + 1e-6 {
                    return Err(DomainError::invalid(
                        "cannot extend a clip past the end of its media",
                    ));
                }
                Clip::Audio(c.edit(|d| {
                    d.start = new_start;
                    d.length = new_end - new_start;
                    d.source_offset = offset;
                })?)
            }
            Clip::Midi(c) => {
                let mut offset_beats = c.content_offset + (new_start - c.start);
                if offset_beats < -EPSILON {
                    let Some(loop_length) = c.loop_length else {
                        return Err(DomainError::invalid(
                            "cannot extend a MIDI clip before its content start",
                        ));
                    };
                    offset_beats = py_mod(offset_beats, loop_length);
                }
                Clip::Midi(c.edit(|d| {
                    d.start = new_start;
                    d.length = new_end - new_start;
                    d.content_offset = offset_beats.max(0.0);
                })?)
            }
        };
        let p = project.with_track(track.with_clip(new.clone())?)?;
        let result = clip_json(&p, &new);
        Ok(Outcome::with_result(
            p,
            ChangeSet::of([], [(ObjectKind::Clip, clip.id())], [], [Flag::Clips]),
            result,
        ))
    }

    fn merge_key(&self) -> Option<String> {
        Some(format!("trim:{}", self.clip_id))
    }
}

/// Splits a clip in two at a timeline beat.
#[derive(Debug, Clone)]
pub struct SplitClip {
    /// The clip.
    pub clip_id: String,
    /// The split position in beats.
    pub at: f64,
    /// The ID of the right half.
    pub new_clip_id: String,
}

impl Command for SplitClip {
    fn name(&self) -> &'static str {
        "SplitClip"
    }

    fn label(&self) -> &'static str {
        "Split Clip"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, clip) = project.clip(&self.clip_id)?;
        if !(clip.start() + EPSILON < self.at && self.at < clip.end() - EPSILON) {
            return Err(DomainError::invalid(
                "split position must be inside the clip",
            ));
        }
        let left_len = self.at - clip.start();
        let right_len = clip.end() - self.at;
        let (left, right) = match clip {
            Clip::Audio(c) => {
                let asset = project.asset(&c.asset_id)?;
                let shift = source_frames_between(project, asset.sample_rate, c.start, self.at);
                let left = c.edit(|d| {
                    d.length = left_len;
                    d.fade_out = Fade::NONE;
                })?;
                let right = c.edit(|d| {
                    d.id.clone_from(&self.new_clip_id);
                    d.start = self.at;
                    d.length = right_len;
                    d.source_offset = c.source_offset + shift;
                    d.fade_in = Fade::NONE;
                })?;
                (Clip::Audio(left), Clip::Audio(right))
            }
            Clip::Midi(c) => {
                let left = c.edit(|d| d.length = left_len)?;
                let right = c.edit(|d| {
                    d.id.clone_from(&self.new_clip_id);
                    d.start = self.at;
                    d.length = right_len;
                    d.content_offset = c.content_offset + left_len;
                })?;
                (Clip::Midi(left), Clip::Midi(right))
            }
        };
        let p = project.with_track(track.with_clip(left.clone())?.with_clip(right.clone())?)?;
        Ok(Outcome::with_result(
            p,
            ChangeSet::of(
                [(ObjectKind::Clip, right.id())],
                [(ObjectKind::Clip, left.id())],
                [],
                [Flag::Clips],
            ),
            json!({"left_clip_id": left.id(), "right_clip_id": right.id()}),
        ))
    }
}

/// Duplicates a clip, by default right after itself on the same track.
#[derive(Debug, Clone)]
pub struct DuplicateClip {
    /// The clip.
    pub clip_id: String,
    /// Start of the copy; the end of the original when `None`.
    pub start: Option<f64>,
    /// Destination track; the same track when `None`.
    pub track_id: Option<String>,
    /// The copy's ID.
    pub new_clip_id: String,
}

impl Command for DuplicateClip {
    fn name(&self) -> &'static str {
        "DuplicateClip"
    }

    fn label(&self) -> &'static str {
        "Duplicate Clip"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, clip) = project.clip(&self.clip_id)?;
        let start = self.start.unwrap_or_else(|| clip.end());
        let copy = match clip {
            Clip::Audio(c) => Clip::Audio(c.edit(|d| {
                d.id.clone_from(&self.new_clip_id);
                d.start = start;
            })?),
            Clip::Midi(c) => Clip::Midi(c.edit(|d| {
                d.id.clone_from(&self.new_clip_id);
                d.start = start;
            })?),
        };
        let dst = match &self.track_id {
            None => track,
            Some(id) => project.track(id)?,
        };
        check_track_accepts(dst, &copy)?;
        let p = project.with_track(dst.with_clip(copy.clone())?)?;
        let result = clip_json(&p, &copy);
        Ok(Outcome::with_result(
            p,
            ChangeSet::of(
                [(ObjectKind::Clip, copy.id())],
                [(ObjectKind::Track, dst.id.as_str())],
                [],
                [Flag::Clips],
            ),
            result,
        ))
    }
}

/// Deletes clips.
#[derive(Debug, Clone)]
pub struct DeleteClips {
    /// The clips.
    pub clip_ids: Vec<String>,
}

impl Command for DeleteClips {
    fn name(&self) -> &'static str {
        "DeleteClips"
    }

    fn label(&self) -> &'static str {
        "Delete Clips"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let mut p = project.clone();
        let unique: indexmap::IndexSet<&str> = self.clip_ids.iter().map(String::as_str).collect();
        for cid in unique {
            let (track, _) = p.clip(cid)?;
            let track = track.without_clip(cid)?;
            p = p.with_track(track)?;
        }
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [],
                self.clip_ids.iter().map(|c| (ObjectKind::Clip, c.as_str())),
                [Flag::Clips],
            ),
        ))
    }
}

/// Changes clip properties; `None` keeps a value. Gain and fades apply to audio clips,
/// `loop_length` to MIDI clips (`Some(None)` turns looping off).
#[derive(Debug, Clone, Default)]
pub struct SetClipProperties {
    /// The clip.
    pub clip_id: String,
    /// Display name.
    pub name: Option<String>,
    /// Clip gain in dB (audio).
    pub gain_db: Option<f64>,
    /// Fade in (audio).
    pub fade_in: Option<Fade>,
    /// Fade out (audio).
    pub fade_out: Option<Fade>,
    /// Muted.
    pub muted: Option<bool>,
    /// `#rrggbb`.
    pub color: Option<String>,
    /// Loop length (MIDI): `Some(None)` turns looping off.
    pub loop_length: Option<Option<f64>>,
}

impl Command for SetClipProperties {
    fn name(&self) -> &'static str {
        "SetClipProperties"
    }

    fn label(&self) -> &'static str {
        "Clip Properties"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, clip) = project.clip(&self.clip_id)?;
        let new = match clip {
            Clip::Audio(c) => {
                let c = c.edit(|d| {
                    if let Some(name) = &self.name {
                        d.name.clone_from(name);
                    }
                    d.muted = self.muted.unwrap_or(d.muted);
                    if self.color.is_some() {
                        d.color.clone_from(&self.color);
                    }
                })?;
                if self.loop_length.is_some() {
                    return Err(DomainError::new(
                        ErrorCode::Unsupported,
                        "audio clip looping is not supported yet",
                    ));
                }
                let c = c.edit(|d| {
                    d.gain_db = self.gain_db.unwrap_or(d.gain_db);
                    d.fade_in = self.fade_in.unwrap_or(d.fade_in);
                    d.fade_out = self.fade_out.unwrap_or(d.fade_out);
                })?;
                let fades = c.fade_in.seconds + c.fade_out.seconds;
                let duration =
                    project.beat_to_seconds(c.start + c.length) - project.beat_to_seconds(c.start);
                if fades > duration + 1e-6 {
                    return Err(DomainError::invalid("fades are longer than the clip"));
                }
                Clip::Audio(c)
            }
            Clip::Midi(c) => {
                let c = c.edit(|d| {
                    if let Some(name) = &self.name {
                        d.name.clone_from(name);
                    }
                    d.muted = self.muted.unwrap_or(d.muted);
                    if self.color.is_some() {
                        d.color.clone_from(&self.color);
                    }
                })?;
                if self.gain_db.is_some() || self.fade_in.is_some() || self.fade_out.is_some() {
                    return Err(DomainError::invalid(
                        "gain and fades apply to audio clips only",
                    ));
                }
                let c = match self.loop_length {
                    Some(loop_length) => c.edit(|d| d.loop_length = loop_length)?,
                    None => c,
                };
                Clip::Midi(c)
            }
        };
        let p = project.with_track(track.with_clip(new)?)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of([], [(ObjectKind::Clip, clip.id())], [], [Flag::Clips]),
        ))
    }

    fn merge_key(&self) -> Option<String> {
        (self.gain_db.is_some() && self.fade_in.is_none() && self.fade_out.is_none())
            .then(|| format!("clipgain:{}", self.clip_id))
    }
}

// -- MIDI note editing ----------------------------------------------------------------------

fn midi_clip<'a>(project: &'a Project, clip_id: &str) -> Result<(&'a Track, &'a MidiClip)> {
    let (track, clip) = project.clip(clip_id)?;
    match clip {
        Clip::Midi(c) => Ok((track, c)),
        Clip::Audio(_) => Err(DomainError::invalid(format!(
            "clip {} is not a MIDI clip",
            str_repr(clip_id)
        ))),
    }
}

fn check_note_count(count: usize) -> Result<()> {
    if count > MAX_NOTES_PER_CLIP {
        return Err(DomainError::new(
            ErrorCode::LimitExceeded,
            format!("a clip can hold at most {MAX_NOTES_PER_CLIP} notes"),
        ));
    }
    Ok(())
}

fn with_notes(
    project: &Project,
    track: &Track,
    clip: &MidiClip,
    notes: Vec<MidiNote>,
) -> Result<Project> {
    check_note_count(notes.len())?;
    let clip = clip.edit(|d| d.notes = Arc::new(notes))?;
    project.with_track(track.with_clip(Clip::Midi(clip))?)
}

/// Adds notes to a MIDI clip.
#[derive(Debug, Clone)]
pub struct AddNotes {
    /// The MIDI clip.
    pub clip_id: String,
    /// The new notes (with new IDs).
    pub notes: Vec<MidiNote>,
}

impl Command for AddNotes {
    fn name(&self) -> &'static str {
        "AddNotes"
    }

    fn label(&self) -> &'static str {
        "Add Notes"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, clip) = midi_clip(project, &self.clip_id)?;
        let existing: BTreeSet<&str> = clip.notes.iter().map(|n| n.id.as_str()).collect();
        let dupes: Vec<&str> = self
            .notes
            .iter()
            .map(|n| n.id.as_str())
            .filter(|id| existing.contains(id))
            .collect();
        if !dupes.is_empty() {
            return Err(DomainError::new(
                ErrorCode::Conflict,
                format!(
                    "note ids already exist: {}",
                    str_list_repr(&dupes[..dupes.len().min(5)])
                ),
            ));
        }
        let notes: Vec<MidiNote> = clip.notes.iter().chain(&self.notes).cloned().collect();
        let p = with_notes(project, track, clip, notes)?;
        let note_ids: Vec<&str> = self.notes.iter().map(|n| n.id.as_str()).collect();
        Ok(Outcome::with_result(
            p,
            ChangeSet::of(
                note_ids.iter().map(|id| (ObjectKind::Note, *id)),
                [(ObjectKind::Clip, clip.id.as_str())],
                [],
                [Flag::Notes],
            ),
            json!({"note_ids": note_ids, "note_count": clip.notes.len() + self.notes.len()}),
        ))
    }
}

/// One note change of [`EditNotes`]; `None` keeps a value.
#[derive(Debug, Clone, Default)]
pub struct NoteEdit {
    /// The note.
    pub note_id: String,
    /// New pitch.
    pub pitch: Option<i64>,
    /// New start.
    pub start: Option<f64>,
    /// New length.
    pub length: Option<f64>,
    /// New velocity.
    pub velocity: Option<i64>,
    /// New channel.
    pub channel: Option<i64>,
}

/// Changes notes of a MIDI clip.
#[derive(Debug, Clone)]
pub struct EditNotes {
    /// The MIDI clip.
    pub clip_id: String,
    /// The changes.
    pub edits: Vec<NoteEdit>,
}

impl Command for EditNotes {
    fn name(&self) -> &'static str {
        "EditNotes"
    }

    fn label(&self) -> &'static str {
        "Edit Notes"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, clip) = midi_clip(project, &self.clip_id)?;
        let mut by_id: IndexMap<&str, MidiNote> = clip
            .notes
            .iter()
            .map(|n| (n.id.as_str(), n.clone()))
            .collect();
        for e in &self.edits {
            let Some(n) = by_id.get_mut(e.note_id.as_str()) else {
                return Err(DomainError::new(
                    ErrorCode::NotFound,
                    format!(
                        "note {} not found in clip {}",
                        str_repr(&e.note_id),
                        str_repr(&clip.id)
                    ),
                ));
            };
            *n = MidiNote::new(
                n.id.clone(),
                e.pitch.unwrap_or_else(|| i64::from(n.pitch)),
                e.start.unwrap_or(n.start),
                e.length.unwrap_or(n.length),
                e.velocity.unwrap_or_else(|| i64::from(n.velocity)),
                e.channel.unwrap_or_else(|| i64::from(n.channel)),
            )?;
        }
        let p = with_notes(project, track, clip, by_id.into_values().collect())?;
        let updated = std::iter::once((ObjectKind::Clip, clip.id.as_str())).chain(
            self.edits
                .iter()
                .map(|e| (ObjectKind::Note, e.note_id.as_str())),
        );
        Ok(Outcome::new(
            p,
            ChangeSet::of([], updated, [], [Flag::Notes]),
        ))
    }

    fn merge_key(&self) -> Option<String> {
        let mut ids: Vec<&str> = self.edits.iter().map(|e| e.note_id.as_str()).collect();
        ids.sort_unstable();
        Some(format!("notes:{}:{}", self.clip_id, ids.join(",")))
    }
}

/// Deletes notes from a MIDI clip.
#[derive(Debug, Clone)]
pub struct DeleteNotes {
    /// The MIDI clip.
    pub clip_id: String,
    /// The notes.
    pub note_ids: Vec<String>,
}

impl Command for DeleteNotes {
    fn name(&self) -> &'static str {
        "DeleteNotes"
    }

    fn label(&self) -> &'static str {
        "Delete Notes"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, clip) = midi_clip(project, &self.clip_id)?;
        let doomed: BTreeSet<&str> = self.note_ids.iter().map(String::as_str).collect();
        let existing: BTreeSet<&str> = clip.notes.iter().map(|n| n.id.as_str()).collect();
        let missing: Vec<&str> = doomed.difference(&existing).copied().collect();
        if !missing.is_empty() {
            return Err(DomainError::new(
                ErrorCode::NotFound,
                format!(
                    "notes not found: {}",
                    str_list_repr(&missing[..missing.len().min(5)])
                ),
            ));
        }
        let notes: Vec<MidiNote> = clip
            .notes
            .iter()
            .filter(|n| !doomed.contains(n.id.as_str()))
            .cloned()
            .collect();
        let p = with_notes(project, track, clip, notes)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [(ObjectKind::Clip, clip.id.as_str())],
                self.note_ids
                    .iter()
                    .map(|id| (ObjectKind::Note, id.as_str())),
                [Flag::Notes],
            ),
        ))
    }
}

/// Replaces a clip's entire note content (quantize, transpose, generators).
#[derive(Debug, Clone)]
pub struct ReplaceNotes {
    /// The MIDI clip.
    pub clip_id: String,
    /// The new notes.
    pub notes: Vec<MidiNote>,
    /// The undo label.
    pub name: String,
}

impl Command for ReplaceNotes {
    fn name(&self) -> &'static str {
        "ReplaceNotes"
    }

    fn label(&self) -> &'static str {
        "Edit"
    }

    fn describe(&self) -> String {
        self.name.clone()
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, clip) = midi_clip(project, &self.clip_id)?;
        let p = with_notes(project, track, clip, self.notes.clone())?;
        Ok(Outcome::with_result(
            p,
            ChangeSet::of(
                [],
                [(ObjectKind::Clip, clip.id.as_str())],
                [],
                [Flag::Notes],
            ),
            json!({"note_count": self.notes.len()}),
        ))
    }
}

/// Replaces the controller events of the given kinds and numbers in `[start, end)`.
#[derive(Debug, Clone)]
pub struct SetControlEvents {
    /// The MIDI clip.
    pub clip_id: String,
    /// The new events.
    pub events: Vec<MidiControlEvent>,
    /// Start of the replaced range.
    pub start: f64,
    /// End of the replaced range.
    pub end: f64,
}

impl Command for SetControlEvents {
    fn name(&self) -> &'static str {
        "SetControlEvents"
    }

    fn label(&self) -> &'static str {
        "Edit Controllers"
    }

    fn apply(&self, project: &Project, _ctx: &CommandContext<'_>) -> Result<Outcome> {
        let (track, clip) = midi_clip(project, &self.clip_id)?;
        let kinds: BTreeSet<(ControlKind, i64)> =
            self.events.iter().map(|e| (e.kind, e.number)).collect();
        let controls: Vec<MidiControlEvent> = clip
            .controls
            .iter()
            .filter(|e| {
                !(kinds.contains(&(e.kind, e.number)) && self.start <= e.beat && e.beat < self.end)
            })
            .chain(&self.events)
            .cloned()
            .collect();
        let clip_new = clip.edit(|d| d.controls = Arc::new(controls))?;
        let p = project.with_track(track.with_clip(Clip::Midi(clip_new))?)?;
        Ok(Outcome::new(
            p,
            ChangeSet::of(
                [],
                [(ObjectKind::Clip, clip.id.as_str())],
                [],
                [Flag::Notes],
            ),
        ))
    }
}
