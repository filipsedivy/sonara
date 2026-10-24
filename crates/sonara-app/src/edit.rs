//! The edit boundary of an interface (TYP-06): one validated [`Edit`] per
//! gesture of the prototype's panels, mapped to `sonara-core` commands and transactions, run
//! with the source `"gui"`; and `undo`, `redo` and `seal_undo`.
//!
//! The prototype's panels built commands themselves and ran them through
//! `SessionBridge.execute`. Here the page sends an [`Edit`] (a tagged JSON object, `{"op":
//! "set_track_mixer", "track_id": …, "volume_db": …}`); [`Edit::from_json`] validates its shape,
//! and [`apply`] draws the new IDs from the application's [`IdSource`](crate::ids::IdSource) and
//! runs the command. The composite gestures of the timeline, the piano roll and the device cards
//! keep their one undo step and label: "Split Clips", "Duplicate Clips", "Set Sidechain" and
//! "Quantize Notes". Quantize runs `sonara-midi`'s `quantize` here, so the page reimplements
//! nothing (DES-01).
//!
//! **Errors** keep their codes for the status bar (TST-03): a command's [`DomainError`] is
//! reported with its code and message ([`ErrorNotice`]), and an edit whose JSON does not match
//! is `INVALID_ARGUMENT`.
//!
//! **Undo steps.** Every edit is one step. Faders, knobs, sends and piano-roll drags send their
//! edit on every move; the commands' merge keys merge the moves of one gesture (within the
//! session's merge window), and the page calls [`seal_undo`] when the gesture ends, so the next
//! gesture starts a new step (the prototype's `bridge.seal_undo`).

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sonara_core::EPSILON;
use sonara_core::commands::Command;
use sonara_core::commands::Transaction;
use sonara_core::commands::clip::{
    AddNotes, CreateMidiClip, DeleteClips, DeleteNotes, DuplicateClip, EditNotes, MoveClip,
    MoveClips, NoteEdit as CoreNoteEdit, ReplaceNotes, SetClipProperties, SplitClip, TrimClip,
};
use sonara_core::commands::device::{
    AddDevice, ApplyPreset, MoveDevice, RemoveDevice, SetDeviceEnabled, SetDeviceParams,
    SetInstrument, SetSidechain,
};
use sonara_core::commands::project::{
    RenameProject, SetMetadata, SetProjectLength, SetTempo, SetTimeSignature, SetTransportSettings,
};
use sonara_core::commands::track::{
    AddSend, CreateTrack, DeleteTrack, MoveTrack, RemoveSend, RenameTrack, SetTrackColor,
    SetTrackInput, SetTrackMixer, SetTrackOutput, UpdateSend,
};
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::ids::{self, IdGenerator};
use sonara_core::midi::MidiNote;
use sonara_core::model::JsonMap;
use sonara_core::model::clip::{Fade, FadeCurve};
use sonara_core::model::project::Project;
use sonara_core::model::track::{MonitorMode, TrackKind};
use sonara_core::session::CommandResult;
use sonara_midi::transforms::{QuantizeOptions, quantize};

use crate::context::AppContext;
use crate::events::{ChangeNotice, ErrorNotice};

/// The source of every edit, undo and redo made through this boundary.
pub const SOURCE: &str = "gui";

/// The label of the timeline's split.
pub const SPLIT_CLIPS: &str = "Split Clips";
/// The label of the timeline's block duplicate.
pub const DUPLICATE_CLIPS: &str = "Duplicate Clips";
/// The label of a device card's sidechain choice.
pub const SET_SIDECHAIN: &str = "Set Sidechain";
/// The label of the piano roll's quantize.
pub const QUANTIZE_NOTES: &str = "Quantize Notes";

/// A new track's kind (the master is never created).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NewTrackKind {
    /// Plays audio clips.
    Audio,
    /// Plays MIDI clips through an instrument.
    Instrument,
    /// A mix bus.
    Bus,
    /// A folder of tracks.
    Folder,
}

impl From<NewTrackKind> for TrackKind {
    fn from(kind: NewTrackKind) -> Self {
        match kind {
            NewTrackKind::Audio => Self::Audio,
            NewTrackKind::Instrument => Self::Instrument,
            NewTrackKind::Bus => Self::Bus,
            NewTrackKind::Folder => Self::Folder,
        }
    }
}

/// Input monitoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Monitor {
    /// Never.
    Off,
    /// While armed.
    Auto,
    /// Always.
    On,
}

impl From<Monitor> for MonitorMode {
    fn from(mode: Monitor) -> Self {
        match mode {
            Monitor::Off => Self::Off,
            Monitor::Auto => Self::Auto,
            Monitor::On => Self::On,
        }
    }
}

/// A fade's shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Curve {
    /// Linear gain.
    Linear,
    /// Constant power.
    EqualPower,
    /// Exponential.
    Exponential,
    /// S-shaped.
    SCurve,
}

/// A fade.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FadeEdit {
    /// Its length in seconds.
    pub seconds: f64,
    /// Its shape.
    pub curve: Curve,
}

impl FadeEdit {
    fn fade(self) -> Result<Fade, DomainError> {
        let curve = match self.curve {
            Curve::Linear => FadeCurve::Linear,
            Curve::EqualPower => FadeCurve::EqualPower,
            Curve::Exponential => FadeCurve::Exponential,
            Curve::SCurve => FadeCurve::SCurve,
        };
        Fade::new(self.seconds, curve)
    }
}

/// A new note: its ID is drawn when the edit runs.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewNote {
    /// The pitch, 0–127.
    pub pitch: i64,
    /// The start in content beats.
    pub start: f64,
    /// The length in beats.
    pub length: f64,
    /// The velocity, 1–127.
    pub velocity: i64,
    /// The MIDI channel, 0–15 (0 by default).
    #[serde(default)]
    pub channel: i64,
}

/// A change of one note; a missing field keeps the value.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoteChange {
    /// The note.
    pub note_id: String,
    /// The new pitch.
    pub pitch: Option<i64>,
    /// The new start in content beats.
    pub start: Option<f64>,
    /// The new length.
    pub length: Option<f64>,
    /// The new velocity.
    pub velocity: Option<i64>,
    /// The new channel.
    pub channel: Option<i64>,
}

/// Song metadata the inspector edits; a missing field is unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataEdit {
    /// Title.
    pub title: Option<String>,
    /// Artist.
    pub artist: Option<String>,
    /// Album.
    pub album: Option<String>,
    /// Genre.
    pub genre: Option<String>,
}

/// One edit of an interface: a gesture of the panels, by its GUI feature ID.
///
/// A missing optional field leaves its value as it is. For [`Edit::SetClipProperties`]'s
/// `loop_length`, a missing field keeps the loop, `null` ends it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    // -- tracks -------------------------------------------------------------------------
    /// A new track at the end, feeding the master. An instrument track
    /// takes the instrument `instrument` with `preset`.
    CreateTrack {
        /// The name ("Track" when blank).
        name: String,
        /// The kind.
        kind: NewTrackKind,
        /// The instrument plugin of an instrument track.
        instrument: Option<String>,
        /// The instrument's preset.
        preset: Option<String>,
    },
    /// Deletes a track (never the master).
    DeleteTrack {
        /// The track.
        track_id: String,
    },
    /// TH-04, IN-09.
    RenameTrack {
        /// The track.
        track_id: String,
        /// The new name.
        name: String,
    },
    /// A colour, or `null` for the automatic colour.
    SetTrackColor {
        /// The track.
        track_id: String,
        /// `#rrggbb`, or `None`.
        color: Option<String>,
    },
    /// Moves a track to `index` in the track list.
    MoveTrack {
        /// The track.
        track_id: String,
        /// The new position (clamped to the list).
        index: i64,
    },
    /// Fader, pan, mute and solo.
    SetTrackMixer {
        /// The track or `master`.
        track_id: String,
        /// The fader level in dB.
        volume_db: Option<f64>,
        /// The pan, −1 to 1.
        pan: Option<f64>,
        /// Muted.
        mute: Option<bool>,
        /// Soloed.
        solo: Option<bool>,
    },
    /// Arm, monitoring and input channels.
    SetTrackInput {
        /// The track.
        track_id: String,
        /// Armed for recording.
        armed: Option<bool>,
        /// Monitoring.
        monitor: Option<Monitor>,
        /// The input channels, 0-based.
        channels: Option<Vec<i64>>,
    },
    /// MX-05, IN-10.
    SetTrackOutput {
        /// The track.
        track_id: String,
        /// The bus or `master`.
        output_id: String,
    },
    /// A send to a bus.
    AddSend {
        /// The sending track.
        track_id: String,
        /// The bus.
        target_id: String,
        /// The level in dB (the mixer adds sends at −12 dB).
        level_db: f64,
        /// Whether it taps before the fader.
        #[serde(default)]
        pre_fader: bool,
    },
    /// A send's level or tap.
    UpdateSend {
        /// The sending track.
        track_id: String,
        /// The send.
        send_id: String,
        /// The level in dB.
        level_db: Option<f64>,
        /// Whether it taps before the fader.
        pre_fader: Option<bool>,
    },
    /// MX-11.
    RemoveSend {
        /// The sending track.
        track_id: String,
        /// The send.
        send_id: String,
    },

    // -- devices ------------------------------------------------------------------------
    /// An effect at the end of the chain, or at `index`.
    AddDevice {
        /// The track.
        track_id: String,
        /// The effect plugin.
        plugin_id: String,
        /// A preset.
        preset: Option<String>,
        /// The position in the chain.
        index: Option<i64>,
    },
    /// MX-07, DV-06.
    RemoveDevice {
        /// The device.
        device_id: String,
    },
    /// Moves an effect to `index` in its chain.
    MoveDevice {
        /// The device.
        device_id: String,
        /// The new position.
        index: i64,
    },
    /// MX-07, DV-03.
    SetDeviceEnabled {
        /// The device.
        device_id: String,
        /// On, or bypassed.
        enabled: bool,
    },
    /// Parameter values, checked against the plugin's descriptor.
    SetDeviceParams {
        /// The device.
        device_id: String,
        /// The values by parameter ID.
        values: JsonMap,
    },
    /// Every parameter back to the descriptor's default.
    ResetDevice {
        /// The device.
        device_id: String,
    },
    /// DV-04.
    ApplyPreset {
        /// The device.
        device_id: String,
        /// The preset's name.
        preset: String,
    },
    /// Replaces the instrument of an instrument track.
    SetInstrument {
        /// The instrument track.
        track_id: String,
        /// The instrument plugin.
        plugin_id: String,
    },
    /// The sidechain source, and the plugin's `sidechain` switch on when a source is
    /// set and the plugin has one, as one step "Set Sidechain".
    SetSidechain {
        /// The device.
        device_id: String,
        /// The source track, or `None`.
        source_track_id: Option<String>,
    },

    // -- clips --------------------------------------------------------------------------
    /// An empty MIDI clip.
    CreateMidiClip {
        /// The instrument track.
        track_id: String,
        /// The start in beats.
        start: f64,
        /// The length in beats.
        length: f64,
        /// The name.
        name: String,
    },
    /// One clip to a new start and, optionally, another compatible track.
    MoveClip {
        /// The clip.
        clip_id: String,
        /// The new start in beats.
        start: Option<f64>,
        /// The new track.
        track_id: Option<String>,
    },
    /// Several clips by the same delta.
    MoveClips {
        /// The clips.
        clip_ids: Vec<String>,
        /// The shift in beats.
        delta_beats: f64,
    },
    /// A new start (content stays in place) or end.
    TrimClip {
        /// The clip.
        clip_id: String,
        /// The new start in beats.
        start: Option<f64>,
        /// The new end in beats.
        end: Option<f64>,
    },
    /// Splits each of `clip_ids` that contains `at` (by more than the beat
    /// tolerance), as one step "Split Clips". Clips that do not contain it are left alone;
    /// when none does, nothing changes (`committed` is false).
    SplitClips {
        /// The selected clips.
        clip_ids: Vec<String>,
        /// The split position in beats.
        at: f64,
    },
    /// Duplicates the clips as a block right after their span, as one step
    /// "Duplicate Clips"; the copies are the outcome's created clips, in order.
    DuplicateClips {
        /// The selected clips.
        clip_ids: Vec<String>,
    },
    /// MW-19, TL-07, TL-08.
    DeleteClips {
        /// The clips.
        clip_ids: Vec<String>,
    },
    /// TL-08, IN-01, IN-04–IN-06, IN-08.
    SetClipProperties {
        /// The clip.
        clip_id: String,
        /// The name.
        name: Option<String>,
        /// Muted.
        muted: Option<bool>,
        /// The gain of an audio clip in dB.
        gain_db: Option<f64>,
        /// The fade-in of an audio clip.
        fade_in: Option<FadeEdit>,
        /// The fade-out of an audio clip.
        fade_out: Option<FadeEdit>,
        /// The loop of a MIDI clip; `null` ends it.
        #[serde(default, deserialize_with = "LoopChange::deserialize")]
        loop_length: LoopChange,
    },

    // -- notes --------------------------------------------------------------------------
    /// New notes; they are the outcome's created notes, in order.
    AddNotes {
        /// The MIDI clip.
        clip_id: String,
        /// The notes.
        notes: Vec<NewNote>,
    },
    /// PR-03, PR-04, PR-06, PR-09, PR-10.
    EditNotes {
        /// The MIDI clip.
        clip_id: String,
        /// The changes.
        edits: Vec<NoteChange>,
    },
    /// PR-05, PR-06.
    DeleteNotes {
        /// The MIDI clip.
        clip_id: String,
        /// The notes.
        note_ids: Vec<String>,
    },
    /// Quantizes `note_ids` (every note when empty) toward a grid of `grid` beats that
    /// starts at content beat 0 (as the agent's `edit_midi_notes`), at `strength`, as one
    /// step "Quantize Notes".
    QuantizeNotes {
        /// The MIDI clip.
        clip_id: String,
        /// The notes; empty for all.
        #[serde(default)]
        note_ids: Vec<String>,
        /// The grid in beats.
        grid: f64,
        /// How far starts move toward the grid, 0 to 1.
        strength: f64,
    },

    // -- project ------------------------------------------------------------------------
    /// Loop, metronome and count-in.
    SetTransport {
        /// Whether playback loops.
        loop_enabled: Option<bool>,
        /// The loop start in beats.
        loop_start: Option<f64>,
        /// The loop end in beats.
        loop_end: Option<f64>,
        /// Whether the metronome clicks.
        metronome: Option<bool>,
        /// The count-in in bars, 0–4.
        count_in_bars: Option<i64>,
    },
    /// The base tempo.
    SetTempo {
        /// Beats per minute.
        bpm: f64,
    },
    /// The time signature from bar 1 on.
    SetTimeSignature {
        /// Beats per bar.
        numerator: i64,
        /// The beat's note value.
        denominator: i64,
    },
    /// IN-13.
    RenameProject {
        /// The new name.
        name: String,
    },
    /// The song end in beats, `null` for the end of the content.
    SetProjectLength {
        /// The song end.
        length_beats: Option<f64>,
    },
    /// IN-15.
    SetMetadata {
        /// The fields to change.
        values: MetadataEdit,
    },
}

/// A MIDI clip's loop in [`Edit::SetClipProperties`]: a missing field keeps it, `null` ends
/// it, a number sets its length in beats.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LoopChange {
    /// The loop stays as it is.
    #[default]
    Keep,
    /// The content plays once.
    End,
    /// The content loops every so many beats.
    Set(f64),
}

impl LoopChange {
    fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Option::<f64>::deserialize(deserializer)? {
            None => Self::End,
            Some(beats) => Self::Set(beats),
        })
    }

    /// The command's form: `None` keeps the loop, `Some(None)` ends it.
    #[expect(
        clippy::option_option,
        reason = "SetClipProperties takes the loop as Option<Option<f64>>"
    )]
    fn command(self) -> Option<Option<f64>> {
        match self {
            Self::Keep => None,
            Self::End => Some(None),
            Self::Set(beats) => Some(Some(beats)),
        }
    }
}

impl Edit {
    /// The edit `value` describes.
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` with the reason when `value` is not an edit: an unknown `op`, a
    /// missing or unknown field, or a value of the wrong type.
    pub fn from_json(value: Value) -> Result<Self, ErrorNotice> {
        serde_json::from_value(value).map_err(|error| ErrorNotice {
            code: ErrorCode::InvalidArgument.as_str(),
            message: format!("invalid edit: {error}"),
        })
    }
}

/// What an edit, undo or redo did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EditOutcome {
    /// Whether the project changed.
    pub committed: bool,
    /// The session version afterwards.
    pub version: u64,
    /// The label: the command's (`Set Track Mixer`), or `Undo …` and `Redo …`.
    pub label: String,
    /// What changed.
    pub changes: ChangeNotice,
    /// The command's result data, for example a new track's `track_id`.
    pub result: Value,
}

impl EditOutcome {
    fn of(result: CommandResult, kind: sonara_core::session::EventKind) -> Self {
        Self {
            committed: result.committed,
            version: result.version,
            changes: ChangeNotice::of(kind, &result.changes),
            label: result.label,
            result: Value::Object(result.result),
        }
    }
}

/// Runs `edit` on `app`'s session with the source `"gui"`.
///
/// # Errors
///
/// The [`ErrorNotice`] of the command's [`DomainError`]: `NO_PROJECT` without a project,
/// `NOT_FOUND` for an unknown track, clip, note, device or send, `INVALID_ARGUMENT` for a value
/// the model refuses, `ROUTING_CYCLE` and `INVALID_ROUTING` for routing the graph refuses, and
/// the other codes of the commands. The project is unchanged after an error.
pub fn apply(app: &AppContext, edit: Edit) -> Result<EditOutcome, ErrorNotice> {
    let command = command(app, edit).map_err(ErrorNotice::from)?;
    let result = app
        .session()
        .lock()
        .execute(command.as_ref(), false, SOURCE)
        .map_err(ErrorNotice::from)?;
    Ok(EditOutcome::of(
        result,
        sonara_core::session::EventKind::Command,
    ))
}

/// Undoes the last step; the label reads "Undo …".
///
/// # Errors
///
/// `NO_PROJECT`, or `NOTHING_TO_UNDO`.
pub fn undo(app: &AppContext) -> Result<EditOutcome, ErrorNotice> {
    let result = app
        .session()
        .lock()
        .undo(SOURCE)
        .map_err(ErrorNotice::from)?;
    Ok(EditOutcome::of(
        result,
        sonara_core::session::EventKind::Undo,
    ))
}

/// Redoes the last undone step; the label reads "Redo …".
///
/// # Errors
///
/// `NO_PROJECT`, or `NOTHING_TO_REDO`.
pub fn redo(app: &AppContext) -> Result<EditOutcome, ErrorNotice> {
    let result = app
        .session()
        .lock()
        .redo(SOURCE)
        .map_err(ErrorNotice::from)?;
    Ok(EditOutcome::of(
        result,
        sonara_core::session::EventKind::Redo,
    ))
}

/// Ends the gesture: the next edit starts a new undo step, even with the same merge key.
pub fn seal_undo(app: &AppContext) {
    app.session().lock().seal_undo();
}

/// The open project, for edits that read it to build their commands.
fn project(app: &AppContext) -> Result<Project, DomainError> {
    Ok(app.session().lock().require_project()?.clone())
}

/// The command of `edit`, with its new IDs drawn.
#[expect(
    clippy::too_many_lines,
    reason = "one arm per edit; each maps fields to its command and nothing else"
)]
fn command(app: &AppContext, edit: Edit) -> Result<Box<dyn Command>, DomainError> {
    let mut ids = app.ids();
    Ok(match edit {
        Edit::CreateTrack {
            name,
            kind,
            instrument,
            preset,
        } => {
            // Both IDs are drawn for every kind, as the prototype's command draws them.
            let track_id = ids.new_id(ids::TRACK);
            let instrument_id = ids.new_id(ids::DEVICE);
            Box::new(CreateTrack {
                name,
                kind: kind.into(),
                index: None,
                color: None,
                output_id: ids::MASTER_ID.to_owned(),
                instrument_plugin: instrument,
                instrument_preset: preset,
                parent_id: None,
                track_id,
                instrument_id,
            })
        }
        Edit::DeleteTrack { track_id } => Box::new(DeleteTrack { track_id }),
        Edit::RenameTrack { track_id, name } => Box::new(RenameTrack { track_id, name }),
        Edit::SetTrackColor { track_id, color } => Box::new(SetTrackColor { track_id, color }),
        Edit::MoveTrack { track_id, index } => Box::new(MoveTrack { track_id, index }),
        Edit::SetTrackMixer {
            track_id,
            volume_db,
            pan,
            mute,
            solo,
        } => Box::new(SetTrackMixer {
            track_id,
            volume_db,
            pan,
            mute,
            solo,
            solo_safe: None,
        }),
        Edit::SetTrackInput {
            track_id,
            armed,
            monitor,
            channels,
        } => Box::new(SetTrackInput {
            track_id,
            armed,
            monitor: monitor.map(MonitorMode::from),
            channels,
        }),
        Edit::SetTrackOutput {
            track_id,
            output_id,
        } => Box::new(SetTrackOutput {
            track_id,
            output_id,
        }),
        Edit::AddSend {
            track_id,
            target_id,
            level_db,
            pre_fader,
        } => Box::new(AddSend {
            track_id,
            target_id,
            level_db,
            pre_fader,
            send_id: ids.new_id(ids::SEND),
        }),
        Edit::UpdateSend {
            track_id,
            send_id,
            level_db,
            pre_fader,
        } => Box::new(UpdateSend {
            track_id,
            send_id,
            level_db,
            pre_fader,
            enabled: None,
        }),
        Edit::RemoveSend { track_id, send_id } => Box::new(RemoveSend { track_id, send_id }),
        Edit::AddDevice {
            track_id,
            plugin_id,
            preset,
            index,
        } => Box::new(AddDevice {
            track_id,
            plugin_id,
            index,
            params: JsonMap::new(),
            preset,
            sidechain_id: None,
            name: None,
            device_id: ids.new_id(ids::DEVICE),
        }),
        Edit::RemoveDevice { device_id } => Box::new(RemoveDevice { device_id }),
        Edit::MoveDevice { device_id, index } => Box::new(MoveDevice { device_id, index }),
        Edit::SetDeviceEnabled { device_id, enabled } => {
            Box::new(SetDeviceEnabled { device_id, enabled })
        }
        Edit::SetDeviceParams { device_id, values } => {
            Box::new(SetDeviceParams { device_id, values })
        }
        Edit::ResetDevice { device_id } => {
            let project = project(app)?;
            let (_, device) = project.device(&device_id)?;
            let descriptor = app.plugins().get(&device.plugin_id)?;
            let values: JsonMap = descriptor
                .defaults()
                .iter()
                .map(|(name, value)| (name.clone(), Value::from(value)))
                .collect();
            Box::new(SetDeviceParams { device_id, values })
        }
        Edit::ApplyPreset { device_id, preset } => Box::new(ApplyPreset { device_id, preset }),
        Edit::SetInstrument {
            track_id,
            plugin_id,
        } => Box::new(SetInstrument {
            track_id,
            plugin_id,
            preset: None,
            params: JsonMap::new(),
            device_id: ids.new_id(ids::DEVICE),
        }),
        Edit::SetSidechain {
            device_id,
            source_track_id,
        } => {
            let source = source_track_id.filter(|id| !id.is_empty());
            let project = project(app)?;
            let (_, device) = project.device(&device_id)?;
            let switch = source.is_some()
                && app
                    .plugins()
                    .find(&device.plugin_id)
                    .is_some_and(|d| d.has_param("sidechain"));
            let mut commands: Vec<Box<dyn Command>> = vec![Box::new(SetSidechain {
                device_id: device_id.clone(),
                source_track_id: source,
            })];
            if switch {
                let mut values = JsonMap::new();
                values.insert("sidechain".to_owned(), Value::Bool(true));
                commands.push(Box::new(SetDeviceParams { device_id, values }));
            }
            Box::new(Transaction {
                commands,
                name: SET_SIDECHAIN.to_owned(),
            })
        }
        Edit::CreateMidiClip {
            track_id,
            start,
            length,
            name,
        } => Box::new(CreateMidiClip {
            track_id,
            start,
            length,
            name,
            notes: Vec::new(),
            loop_length: None,
            color: None,
            clip_id: ids.new_id(ids::CLIP),
        }),
        Edit::MoveClip {
            clip_id,
            start,
            track_id,
        } => Box::new(MoveClip {
            clip_id,
            start,
            track_id,
        }),
        Edit::MoveClips {
            clip_ids,
            delta_beats,
        } => Box::new(MoveClips {
            clip_ids,
            delta_beats,
        }),
        Edit::TrimClip {
            clip_id,
            start,
            end,
        } => Box::new(TrimClip {
            clip_id,
            start,
            end,
        }),
        Edit::SplitClips { clip_ids, at } => split_clips(&project(app)?, &clip_ids, at, &mut ids)?,
        Edit::DuplicateClips { clip_ids } => duplicate_clips(&project(app)?, &clip_ids, &mut ids)?,
        Edit::DeleteClips { clip_ids } => Box::new(DeleteClips { clip_ids }),
        Edit::SetClipProperties {
            clip_id,
            name,
            muted,
            gain_db,
            fade_in,
            fade_out,
            loop_length,
        } => Box::new(SetClipProperties {
            clip_id,
            name,
            gain_db,
            fade_in: fade_in.map(FadeEdit::fade).transpose()?,
            fade_out: fade_out.map(FadeEdit::fade).transpose()?,
            muted,
            color: None,
            loop_length: loop_length.command(),
        }),
        Edit::AddNotes { clip_id, notes } => {
            let notes = notes
                .iter()
                .map(|n| {
                    MidiNote::create(&mut ids, n.pitch, n.start, n.length, n.velocity, n.channel)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Box::new(AddNotes { clip_id, notes })
        }
        Edit::EditNotes { clip_id, edits } => Box::new(EditNotes {
            clip_id,
            edits: edits
                .into_iter()
                .map(|e| CoreNoteEdit {
                    note_id: e.note_id,
                    pitch: e.pitch,
                    start: e.start,
                    length: e.length,
                    velocity: e.velocity,
                    channel: e.channel,
                })
                .collect(),
        }),
        Edit::DeleteNotes { clip_id, note_ids } => Box::new(DeleteNotes { clip_id, note_ids }),
        Edit::QuantizeNotes {
            clip_id,
            note_ids,
            grid,
            strength,
        } => quantize_notes(&project(app)?, clip_id, &note_ids, grid, strength)?,
        Edit::SetTransport {
            loop_enabled,
            loop_start,
            loop_end,
            metronome,
            count_in_bars,
        } => {
            let mut values = JsonMap::new();
            let mut set = |key: &str, value: Option<Value>| {
                if let Some(value) = value {
                    values.insert(key.to_owned(), value);
                }
            };
            set("loop_start", loop_start.map(Value::from));
            set("loop_end", loop_end.map(Value::from));
            set("loop_enabled", loop_enabled.map(Value::from));
            set("metronome", metronome.map(Value::from));
            set("count_in_bars", count_in_bars.map(Value::from));
            Box::new(SetTransportSettings { values })
        }
        Edit::SetTempo { bpm } => Box::new(SetTempo {
            bpm,
            beat: 0.0,
            ramp: false,
            clear_changes: false,
        }),
        Edit::SetTimeSignature {
            numerator,
            denominator,
        } => Box::new(SetTimeSignature {
            numerator,
            denominator,
            bar: 0,
        }),
        Edit::RenameProject { name } => Box::new(RenameProject { name }),
        Edit::SetProjectLength { length_beats } => Box::new(SetProjectLength { length_beats }),
        Edit::SetMetadata { values } => {
            let MetadataEdit {
                title,
                artist,
                album,
                genre,
            } = values;
            let values: IndexMap<String, String> = [
                ("title", title),
                ("artist", artist),
                ("album", album),
                ("genre", genre),
            ]
            .into_iter()
            .filter_map(|(field, value)| Some((field.to_owned(), value?)))
            .collect();
            Box::new(SetMetadata { values })
        }
    })
}

/// "Split Clips": a split of each of `clip_ids` that contains `at` (the canvas's
/// `split_selected`); the clips are checked in the project here so that the ones the beat
/// misses are skipped instead of failing the step.
fn split_clips(
    project: &Project,
    clip_ids: &[String],
    at: f64,
    ids: &mut dyn IdGenerator,
) -> Result<Box<dyn Command>, DomainError> {
    let mut commands: Vec<Box<dyn Command>> = Vec::new();
    for clip_id in clip_ids {
        let (_, clip) = project.clip(clip_id)?;
        if clip.start() + EPSILON < at && at < clip.end() - EPSILON {
            commands.push(Box::new(SplitClip {
                clip_id: clip_id.clone(),
                at,
                new_clip_id: ids.new_id(ids::CLIP),
            }));
        }
    }
    Ok(Box::new(Transaction {
        commands,
        name: SPLIT_CLIPS.to_owned(),
    }))
}

/// "Duplicate Clips": a copy of each clip, shifted by the span of all of them, so the block
/// repeats right after itself (the canvas's `duplicate_selected`).
fn duplicate_clips(
    project: &Project,
    clip_ids: &[String],
    ids: &mut dyn IdGenerator,
) -> Result<Box<dyn Command>, DomainError> {
    let clips = clip_ids
        .iter()
        .map(|id| project.clip(id).map(|(_, clip)| clip))
        .collect::<Result<Vec<_>, _>>()?;
    let start = clips
        .iter()
        .map(|c| c.start())
        .fold(f64::INFINITY, f64::min);
    let end = clips
        .iter()
        .map(|c| c.end())
        .fold(f64::NEG_INFINITY, f64::max);
    let span = end - start;
    let commands: Vec<Box<dyn Command>> = clips
        .iter()
        .map(|clip| -> Box<dyn Command> {
            Box::new(DuplicateClip {
                clip_id: clip.id().to_owned(),
                start: Some(clip.start() + span),
                track_id: None,
                new_clip_id: ids.new_id(ids::CLIP),
            })
        })
        .collect();
    Ok(Box::new(Transaction {
        commands,
        name: DUPLICATE_CLIPS.to_owned(),
    }))
}

/// "Quantize Notes" (the piano roll's `quantize`): the other notes, then the quantized ones,
/// replace the clip's notes.
fn quantize_notes(
    project: &Project,
    clip_id: String,
    note_ids: &[String],
    grid: f64,
    strength: f64,
) -> Result<Box<dyn Command>, DomainError> {
    if !(0.0..=1.0).contains(&strength) {
        return Err(DomainError::invalid("quantize strength must be 0-1"));
    }
    let (_, clip) = project.clip(&clip_id)?;
    let Some(midi) = clip.as_midi() else {
        return Err(DomainError::invalid(format!(
            "clip {clip_id} is not a MIDI clip"
        )));
    };
    if let Some(missing) = note_ids.iter().find(|id| midi.note(id).is_none()) {
        return Err(DomainError::not_found("note", missing));
    }
    let chosen = |id: &str| note_ids.is_empty() || note_ids.iter().any(|n| n == id);
    let (targets, others): (Vec<MidiNote>, Vec<MidiNote>) =
        midi.notes.iter().cloned().partition(|n| chosen(&n.id));
    let quantized = quantize(
        &targets,
        grid,
        &QuantizeOptions {
            strength,
            ..QuantizeOptions::default()
        },
    )?;
    Ok(Box::new(ReplaceNotes {
        clip_id,
        notes: others.into_iter().chain(quantized).collect(),
        name: QUANTIZE_NOTES.to_owned(),
    }))
}
