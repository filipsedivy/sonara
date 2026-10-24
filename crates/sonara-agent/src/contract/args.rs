//! Typed argument structs: one per input model, holding the normalized output of
//! [`super::validation`]. Handlers take these instead of
//! JSON.
//!
//! The structs mirror the table field for field; a test deserializes every normalized fixture
//! into its struct and serializes it back to the same JSON, so the table and the structs cannot
//! drift. Integer fields use the narrowest type that holds every value the table accepts;
//! unbounded integers are `i64` (`u64` when only a non-negative minimum is set).

use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sonara_core::error::{DomainError, ErrorCode};

use super::inputs;
use super::model::Model;
use super::tools::ToolSpec;
use super::validation::validate;

/// A typed argument struct and the input model it holds the validated values of.
pub trait ToolArgs: DeserializeOwned {
    /// The input model.
    const MODEL: &'static Model;
}

/// Validates `arguments` for `tool` and returns them as the tool's typed struct.
///
/// # Errors
///
/// - [`ErrorCode::InvalidArgument`] with the prototype's validation envelope when the arguments
///   do not validate ([`validate`]);
/// - [`ErrorCode::Internal`] when `T` is not the struct of the tool's input model or the
///   normalized values do not deserialize into it, both programming errors.
pub fn parse<T: ToolArgs>(tool: &ToolSpec, arguments: &Value) -> Result<T, DomainError> {
    if !std::ptr::eq(tool.input, T::MODEL) {
        return Err(DomainError::new(
            ErrorCode::Internal,
            format!(
                "{} takes {} arguments, not {}",
                tool.name,
                tool.input.name,
                T::MODEL.name
            ),
        ));
    }
    let values = validate(tool, arguments)?;
    serde_json::from_value(Value::Object(values)).map_err(|error| {
        DomainError::new(
            ErrorCode::Internal,
            format!(
                "validated arguments of {} do not fit {}: {error}",
                tool.name,
                T::MODEL.name
            ),
        )
    })
}

// -- shared value types ----------------------------------------------------------------------

/// `Position`: beats from the song start, or a `'bar:beat[:tick]'` or seconds (`'12.5s'`)
/// string, resolved by the handler against the project.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Position {
    /// Beats from the song start.
    Beats(f64),
    /// ``'bar:beat[:tick]'`` or seconds.
    Text(String),
}

/// `Length`: beats, or a bars (`'4 bars'`, `'4b'`) or seconds (`'30s'`) string.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Length {
    /// Beats.
    Beats(f64),
    /// Bars or seconds.
    Text(String),
}

/// A note pitch (`int | str`): a MIDI number or a note name such as `'C4'`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Pitch {
    /// A MIDI note number (range-checked by the handler).
    Number(i64),
    /// A note name such as `'F#3'`.
    Name(String),
}

/// A device parameter value (`float | int | bool | str`), keeping the kind the union chose.
/// Integers are tried before floats so that `5` and `5.0` stay distinct.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ParamValue {
    /// A boolean.
    Bool(bool),
    /// An integer.
    Int(i64),
    /// A float.
    Float(f64),
    /// A string (an enumerated choice).
    Str(String),
}

/// `GenerateMidi.beats_per_chord`: one duration for every chord or one per chord.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BeatsPerChord {
    /// The same duration for every chord.
    Uniform(f64),
    /// One duration per chord.
    PerChord(Vec<f64>),
}

/// `RenderRange.tail`: `'auto'`, `'none'` or seconds.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Tail {
    /// `'auto'` or `'none'`.
    Mode(TailMode),
    /// A tail length in seconds.
    Seconds(f64),
}

/// The keyword forms of [`Tail`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TailMode {
    /// Render until the effects ring out.
    Auto,
    /// No tail.
    #[serde(rename = "none")]
    Off,
}

/// `CreateProject.template`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Template {
    /// No tracks.
    Empty,
    /// The electronic template.
    Electronic,
    /// The band template.
    Band,
    /// The orchestral template.
    Orchestral,
}

/// `GetProjectState.detail`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Detail {
    /// The summary.
    Summary,
    /// The summary plus devices, parameters and automation.
    Full,
}

/// `CreateTrack.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    /// An audio track.
    Audio,
    /// An instrument (MIDI) track.
    Instrument,
    /// A bus.
    Bus,
    /// A folder.
    Folder,
}

/// `GenerateMidi.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratorKind {
    /// Voice-led chords.
    Chords,
    /// An arpeggio.
    Arpeggio,
    /// A bass line.
    Bass,
    /// A drum pattern.
    Drums,
    /// A melody.
    Melody,
}

/// `ImportAudio.mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportMode {
    /// Copy the file into the project.
    Copy,
    /// Link the file in place.
    Link,
}

/// `EditClip.fade_curve`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FadeCurve {
    /// Linear.
    Linear,
    /// Equal power.
    EqualPower,
    /// Exponential.
    Exponential,
    /// S-curve.
    SCurve,
}

/// `ListPlugins.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    /// Effects.
    Effect,
    /// Instruments.
    Instrument,
}

/// `AutomationPointInput.curve`: the shape of the segment after a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CurveShape {
    /// A straight line.
    Linear,
    /// A step.
    Hold,
    /// A smooth curve.
    Smooth,
}

/// `FormatSpec.format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioFormat {
    /// WAV.
    Wav,
    /// Broadcast WAV.
    Bwf,
    /// RF64.
    Rf64,
    /// Sony Wave64.
    W64,
    /// AIFF.
    Aiff,
    /// Core Audio Format.
    Caf,
    /// FLAC.
    Flac,
    /// MP3.
    Mp3,
    /// Ogg Vorbis.
    Ogg,
    /// Ogg Opus.
    Opus,
}

/// `FormatSpec.dither`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dither {
    /// Triangular dither.
    Tpdf,
    /// No dither.
    #[serde(rename = "none")]
    Off,
}

/// `ExportAudio.normalize`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Normalize {
    /// No normalization.
    #[serde(rename = "none")]
    Off,
    /// To a sample peak.
    Peak,
    /// To a true peak.
    TruePeak,
    /// To an integrated loudness.
    Loudness,
}

// -- input models ----------------------------------------------------------------------------

/// `CreateProject`: the arguments of `create_project`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateProject {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Token from a previous `CONFIRMATION_REQUIRED` response, after the user agreed.
    pub confirmation_token: Option<String>,
    /// Project name.
    pub name: String,
    /// Initial tempo.
    pub bpm: f64,
    /// Initial time signature, for example `4/4`.
    pub time_signature: String,
    /// Sample rate in Hz: 44100, 48000, 88200 or 96000.
    pub sample_rate: u32,
    /// Starting point; 'empty' has no tracks.
    pub template: Template,
    /// Optional song length (sets the project end).
    pub length: Option<Length>,
    /// Save immediately to this .sonara path.
    pub path: Option<String>,
}

impl ToolArgs for CreateProject {
    const MODEL: &'static Model = &inputs::CREATE_PROJECT;
}

/// `OpenProject`: the arguments of `open_project`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OpenProject {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Token from a previous `CONFIRMATION_REQUIRED` response, after the user agreed.
    pub confirmation_token: Option<String>,
    /// Path to a .sonara bundle directory or archive.
    pub path: String,
    /// Restore unsaved changes from the autosave if present.
    pub recover_autosave: bool,
}

impl ToolArgs for OpenProject {
    const MODEL: &'static Model = &inputs::OPEN_PROJECT;
}

/// `SaveProject`: the arguments of `save_project`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SaveProject {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Token from a previous `CONFIRMATION_REQUIRED` response, after the user agreed.
    pub confirmation_token: Option<String>,
    /// Target path (Save As); omit to save in place.
    pub path: Option<String>,
    /// Write a single-file ZIP archive instead of a bundle directory.
    pub packaged: bool,
    /// Save a copy without switching the open project to it.
    pub as_copy: bool,
}

impl ToolArgs for SaveProject {
    const MODEL: &'static Model = &inputs::SAVE_PROJECT;
}

/// `GetProjectState`: the arguments of `get_project_state`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GetProjectState {
    /// 'full' adds devices, parameters and automation.
    pub detail: Detail,
    /// Include every MIDI note (can be large).
    pub include_notes: bool,
    /// Limit the report to these tracks.
    pub track_ids: Option<Vec<String>>,
}

impl ToolArgs for GetProjectState {
    const MODEL: &'static Model = &inputs::GET_PROJECT_STATE;
}

/// `Empty`: the arguments of `list_tracks`, `list_midi_devices`, `stop_playback`, `list_operations`, `undo`, `redo`, `get_history`, `verify_project`, `get_capabilities`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

impl ToolArgs for Empty {
    const MODEL: &'static Model = &inputs::EMPTY;
}

/// `SetTempo`: the arguments of `set_tempo`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetTempo {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Tempo in BPM.
    pub bpm: f64,
    /// Where the tempo starts (0 = song tempo).
    pub at: Position,
    /// Glide linearly from this point to the next tempo point.
    pub ramp: bool,
    /// Replace all tempo changes with one constant tempo.
    pub clear_changes: bool,
}

impl ToolArgs for SetTempo {
    const MODEL: &'static Model = &inputs::SET_TEMPO;
}

/// `SetTimeSignature`: the arguments of `set_time_signature`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetTimeSignature {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Beats per bar.
    pub numerator: u8,
    /// Beat unit: 1, 2, 4, 8, 16 or 32.
    pub denominator: u8,
    /// 1-based bar where the signature starts.
    pub bar: u64,
}

impl ToolArgs for SetTimeSignature {
    const MODEL: &'static Model = &inputs::SET_TIME_SIGNATURE;
}

/// `SetProjectLength`: the arguments of `set_project_length`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetProjectLength {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Song length; null = end of the last clip.
    pub length: Option<Length>,
}

impl ToolArgs for SetProjectLength {
    const MODEL: &'static Model = &inputs::SET_PROJECT_LENGTH;
}

/// `AddSection`: the arguments of `add_section`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AddSection {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Section name.
    pub name: String,
    /// Beats (number, 0 = song start), `'bar:beat[:tick]'` (1-based) or seconds like '12.5s'.
    pub start: Position,
    /// Beats (number), '4 bars' / '4b', or seconds like '30s'.
    pub length: Length,
    /// Colour as `#rrggbb`.
    pub color: Option<String>,
}

impl ToolArgs for AddSection {
    const MODEL: &'static Model = &inputs::ADD_SECTION;
}

/// `AddMarker`: the arguments of `add_marker`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AddMarker {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Marker name.
    pub name: String,
    /// Beats (number, 0 = song start), `'bar:beat[:tick]'` (1-based) or seconds like '12.5s'.
    pub at: Position,
}

impl ToolArgs for AddMarker {
    const MODEL: &'static Model = &inputs::ADD_MARKER;
}

/// `SetMetadata`: the arguments of `set_metadata`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetMetadata {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Title.
    pub title: Option<String>,
    /// Artist.
    pub artist: Option<String>,
    /// Album.
    pub album: Option<String>,
    /// Genre.
    pub genre: Option<String>,
    /// Composer.
    pub composer: Option<String>,
    /// Comment.
    pub comment: Option<String>,
}

impl ToolArgs for SetMetadata {
    const MODEL: &'static Model = &inputs::SET_METADATA;
}

/// `SetLoop`: the arguments of `set_loop`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetLoop {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Enable or disable the loop.
    pub enabled: bool,
    /// Loop start.
    pub start: Option<Position>,
    /// Loop end.
    pub end: Option<Position>,
}

impl ToolArgs for SetLoop {
    const MODEL: &'static Model = &inputs::SET_LOOP;
}

/// `CreateTrack`: the arguments of `create_track`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateTrack {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Track name.
    pub name: String,
    /// Track kind.
    pub kind: TrackKind,
    /// Instrument `plugin_id` (instrument tracks), e.g. `'sonara.synth.poly'`.
    pub instrument: Option<String>,
    /// Instrument preset name, e.g. 'Analog Bass'.
    pub preset: Option<String>,
    /// Colour as `#rrggbb`.
    pub color: Option<String>,
    /// Position in the track list (default: end).
    pub index: Option<u64>,
    /// Output: `'master'` or a bus `track_id`.
    pub output: String,
    /// Folder track to place the track in.
    pub parent_id: Option<String>,
}

impl ToolArgs for CreateTrack {
    const MODEL: &'static Model = &inputs::CREATE_TRACK;
}

/// `RenameTrack`: the arguments of `rename_track`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RenameTrack {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The track to rename.
    pub track_id: String,
    /// New track name.
    pub name: String,
}

impl ToolArgs for RenameTrack {
    const MODEL: &'static Model = &inputs::RENAME_TRACK;
}

/// `DeleteTrack`: the arguments of `delete_track`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteTrack {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Token from a previous `CONFIRMATION_REQUIRED` response, after the user agreed.
    pub confirmation_token: Option<String>,
    /// The track to delete.
    pub track_id: String,
}

impl ToolArgs for DeleteTrack {
    const MODEL: &'static Model = &inputs::DELETE_TRACK;
}

/// `SetTrackVolume`: the arguments of `set_track_volume`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetTrackVolume {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Track ID or 'master'.
    pub track_id: String,
    /// Fader level in dB (0 = unity).
    pub volume_db: f64,
}

impl ToolArgs for SetTrackVolume {
    const MODEL: &'static Model = &inputs::SET_TRACK_VOLUME;
}

/// `SetTrackPan`: the arguments of `set_track_pan`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetTrackPan {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The track.
    pub track_id: String,
    /// -1 = hard left, 0 = centre, +1 = hard right.
    pub pan: f64,
}

impl ToolArgs for SetTrackPan {
    const MODEL: &'static Model = &inputs::SET_TRACK_PAN;
}

/// `SetTrackState`: the arguments of `set_track_state`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetTrackState {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The track.
    pub track_id: String,
    /// Mute or unmute.
    pub mute: Option<bool>,
    /// Solo or unsolo.
    pub solo: Option<bool>,
    /// Exempt the track from solo muting.
    pub solo_safe: Option<bool>,
    /// Colour as `#rrggbb`.
    pub color: Option<String>,
    /// Arm for recording.
    pub armed: Option<bool>,
}

impl ToolArgs for SetTrackState {
    const MODEL: &'static Model = &inputs::SET_TRACK_STATE;
}

/// `SetInstrument`: the arguments of `set_instrument`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetInstrument {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The instrument track.
    pub track_id: String,
    /// Instrument `plugin_id`.
    pub plugin_id: String,
    /// Preset to load.
    pub preset: Option<String>,
    /// Initial parameter values by parameter ID.
    pub params: IndexMap<String, ParamValue>,
}

impl ToolArgs for SetInstrument {
    const MODEL: &'static Model = &inputs::SET_INSTRUMENT;
}

/// `SendSpec`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SendSpec {
    /// Bus `track_id`.
    pub target: String,
    /// Send level in dB.
    pub level_db: f64,
    /// Tap the send before the fader.
    pub pre_fader: bool,
}

/// `ConfigureRouting`: the arguments of `configure_routing`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigureRouting {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The track to route.
    pub track_id: String,
    /// New output: `'master'` or a bus `track_id`.
    pub output: Option<String>,
    /// Sends to add.
    pub add_sends: Vec<SendSpec>,
    /// `send_id` -> new level in dB.
    pub update_sends: IndexMap<String, f64>,
    /// send IDs to remove.
    pub remove_sends: Vec<String>,
    /// Device on this track whose sidechain to set.
    pub sidechain_device_id: Option<String>,
    /// Track ID feeding the sidechain ('' clears it).
    pub sidechain_source: Option<String>,
}

impl ToolArgs for ConfigureRouting {
    const MODEL: &'static Model = &inputs::CONFIGURE_ROUTING;
}

/// `NoteInput`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NoteInput {
    /// MIDI number 0-127 or a name like 'C4' (C4 = 60), 'F#3', 'Bb2'.
    pub pitch: Pitch,
    /// Beats from the clip start.
    pub start: f64,
    /// Duration in beats.
    pub length: f64,
    /// Velocity, 1 to 127.
    pub velocity: u8,
    /// MIDI channel, 0 to 15.
    pub channel: u8,
}

/// `CreateMidiClip`: the arguments of `create_midi_clip`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateMidiClip {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The instrument track.
    pub track_id: String,
    /// Beats (number, 0 = song start), `'bar:beat[:tick]'` (1-based) or seconds like '12.5s'.
    pub start: Position,
    /// Beats (number), '4 bars' / '4b', or seconds like '30s'.
    pub length: Length,
    /// Clip name.
    pub name: String,
    /// Notes of the clip.
    pub notes: Vec<NoteInput>,
    /// Repeat the first N beats of content across the clip.
    pub loop_length: Option<Length>,
    /// Colour as `#rrggbb`.
    pub color: Option<String>,
}

impl ToolArgs for CreateMidiClip {
    const MODEL: &'static Model = &inputs::CREATE_MIDI_CLIP;
}

/// `AddMidiNotes`: the arguments of `add_midi_notes`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AddMidiNotes {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The MIDI clip.
    pub clip_id: String,
    /// Notes to add.
    pub notes: Vec<NoteInput>,
}

impl ToolArgs for AddMidiNotes {
    const MODEL: &'static Model = &inputs::ADD_MIDI_NOTES;
}

/// `NoteEditInput`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NoteEditInput {
    /// The note to edit.
    pub note_id: String,
    /// New pitch: a MIDI number or a note name.
    pub pitch: Option<Pitch>,
    /// New start in beats from the clip start.
    pub start: Option<f64>,
    /// New length in beats.
    pub length: Option<f64>,
    /// New velocity.
    pub velocity: Option<u8>,
}

/// `QuantizeSpec`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuantizeSpec {
    /// Grid in beats (0.25 = sixteenths).
    pub grid: f64,
    /// Quantize strength, 0 to 1.
    pub strength: f64,
    /// Swing amount, 0 to 1.
    pub swing: f64,
    /// Also quantize note ends.
    pub ends: bool,
}

/// `HumanizeSpec`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HumanizeSpec {
    /// Timing deviation (std. dev., beats).
    pub timing: f64,
    /// Velocity deviation.
    pub velocity: u8,
    /// Random seed.
    pub seed: i64,
}

/// `EditMidiNotes`: the arguments of `edit_midi_notes`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EditMidiNotes {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The MIDI clip.
    pub clip_id: String,
    /// Per-note edits.
    pub edits: Vec<NoteEditInput>,
    /// Notes to delete.
    pub delete_note_ids: Vec<String>,
    /// Semitones applied to the selected notes.
    pub transpose: Option<i8>,
    /// Quantize the selected notes.
    pub quantize: Option<QuantizeSpec>,
    /// Humanize the selected notes.
    pub humanize: Option<HumanizeSpec>,
    /// Factor applied to the selected notes' velocities.
    pub velocity_scale: Option<f64>,
    /// Restrict transpose/quantize/humanize/velocity to these notes.
    pub note_ids: Option<Vec<String>>,
    /// Replace the clip's notes entirely.
    pub replace_all: Option<Vec<NoteInput>>,
}

impl ToolArgs for EditMidiNotes {
    const MODEL: &'static Model = &inputs::EDIT_MIDI_NOTES;
}

/// `GenerateMidi`: the arguments of `generate_midi`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerateMidi {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Instrument track receiving a new clip.
    pub track_id: String,
    /// What to generate.
    pub kind: GeneratorKind,
    /// Beats (number, 0 = song start), `'bar:beat[:tick]'` (1-based) or seconds like '12.5s'.
    pub start: Position,
    /// Clip length; default = the progression length (or bars*4).
    pub length: Option<Length>,
    /// Key like 'A minor', 'F# major', 'D dorian' (Roman numerals need it).
    pub key: Option<String>,
    /// Roman numerals ('i','VI','III','VII', 'V7') or chord symbols ('Am','F','C','G7').
    pub progression: Option<Vec<String>>,
    /// Chord duration(s) in beats.
    pub beats_per_chord: BeatsPerChord,
    /// The pattern style. Chords: `sustain|stabs|offbeat|pulse8|half`; arpeggio:
    /// `up|down|updown|downup|random|pinky`; bass:
    /// `root|pulse|octaves|offbeat|fifths|walking|syncopated`; drums:
    /// `four_on_the_floor|house|techno|breakbeat|hiphop|trap|rock|halftime|dnb|ride_jazz|cinematic`.
    pub style: Option<String>,
    /// Drums: number of bars.
    pub bars: u16,
    /// Register: chord/arp centre octave (C4 = 60) or bass octave.
    pub octave: u8,
    /// Arpeggio/bass/melody note rate in beats.
    pub rate: f64,
    /// Base velocity.
    pub velocity: u8,
    /// Drums: ghost notes and kick variation.
    pub variation: f64,
    /// Drums: fill in the last half bar.
    pub fill: bool,
    /// Swing amount, 0 to 1.
    pub swing: f64,
    /// Small, reproducible timing/velocity deviations.
    pub humanize: bool,
    /// Random seed; same inputs + seed = same notes.
    pub seed: i64,
    /// Clip name.
    pub name: Option<String>,
    /// Append into this existing clip instead of creating one.
    pub clip_id: Option<String>,
}

impl ToolArgs for GenerateMidi {
    const MODEL: &'static Model = &inputs::GENERATE_MIDI;
}

/// `ImportAudio`: the arguments of `import_audio`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportAudio {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The audio file.
    pub path: String,
    /// Existing audio track; omit to create a new track.
    pub track_id: Option<String>,
    /// Beats (number, 0 = song start), `'bar:beat[:tick]'` (1-based) or seconds like '12.5s'.
    pub start: Position,
    /// copy into the project (safe) or link in place.
    pub mode: ImportMode,
    /// Name of the new track.
    pub track_name: Option<String>,
}

impl ToolArgs for ImportAudio {
    const MODEL: &'static Model = &inputs::IMPORT_AUDIO;
}

/// `ImportMidi`: the arguments of `import_midi`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportMidi {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The Standard MIDI File.
    pub path: String,
    /// Use the file's tempo map and time signatures.
    pub apply_tempo: bool,
    /// Beats (number, 0 = song start), `'bar:beat[:tick]'` (1-based) or seconds like '12.5s'.
    pub start: Position,
}

impl ToolArgs for ImportMidi {
    const MODEL: &'static Model = &inputs::IMPORT_MIDI;
}

/// `MoveClip`: the arguments of `move_clip`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MoveClip {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The clip to move.
    pub clip_id: String,
    /// New start position.
    pub start: Option<Position>,
    /// Move to another compatible track.
    pub track_id: Option<String>,
}

impl ToolArgs for MoveClip {
    const MODEL: &'static Model = &inputs::MOVE_CLIP;
}

/// `EditClip`: the arguments of `edit_clip`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EditClip {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The clip to edit.
    pub clip_id: String,
    /// New clip name.
    pub name: Option<String>,
    /// Mute or unmute the clip.
    pub muted: Option<bool>,
    /// Audio clips only.
    pub gain_db: Option<f64>,
    /// Fade-in length in seconds.
    pub fade_in_seconds: Option<f64>,
    /// Fade-out length in seconds.
    pub fade_out_seconds: Option<f64>,
    /// Fade curve shape.
    pub fade_curve: Option<FadeCurve>,
    /// New clip start (content stays in place).
    pub trim_start: Option<Position>,
    /// New clip end.
    pub trim_end: Option<Position>,
    /// MIDI clips: repeat the first N beats.
    pub loop_length: Option<Length>,
}

impl ToolArgs for EditClip {
    const MODEL: &'static Model = &inputs::EDIT_CLIP;
}

/// `SplitClip`: the arguments of `split_clip`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SplitClip {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The clip to split.
    pub clip_id: String,
    /// Beats (number, 0 = song start), `'bar:beat[:tick]'` (1-based) or seconds like '12.5s'.
    pub at: Position,
}

impl ToolArgs for SplitClip {
    const MODEL: &'static Model = &inputs::SPLIT_CLIP;
}

/// `DuplicateClip`: the arguments of `duplicate_clip`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DuplicateClip {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The clip to duplicate.
    pub clip_id: String,
    /// Number of copies placed back to back.
    pub times: u16,
    /// Start of the first copy (default: right after the clip).
    pub start: Option<Position>,
    /// Track for the copies (default: the clip's track).
    pub track_id: Option<String>,
}

impl ToolArgs for DuplicateClip {
    const MODEL: &'static Model = &inputs::DUPLICATE_CLIP;
}

/// `DeleteClips`: the arguments of `delete_clips`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteClips {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The clips to delete.
    pub clip_ids: Vec<String>,
}

impl ToolArgs for DeleteClips {
    const MODEL: &'static Model = &inputs::DELETE_CLIPS;
}

/// `ListPlugins`: the arguments of `list_plugins`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ListPlugins {
    /// Only effects or only instruments.
    pub kind: Option<PluginKind>,
    /// Return full parameter schema for one plugin.
    pub plugin_id: Option<String>,
}

impl ToolArgs for ListPlugins {
    const MODEL: &'static Model = &inputs::LIST_PLUGINS;
}

/// `AddEffect`: the arguments of `add_effect`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AddEffect {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Track, bus or 'master'.
    pub track_id: String,
    /// Effect `plugin_id`, e.g. `'sonara.dynamics.compressor'`.
    pub plugin_id: String,
    /// Preset to load.
    pub preset: Option<String>,
    /// Initial parameter values by parameter ID.
    pub params: IndexMap<String, ParamValue>,
    /// Insert position (default: end of chain).
    pub index: Option<u64>,
    /// Track ID keying the sidechain (compressor/gate).
    pub sidechain_source: Option<String>,
}

impl ToolArgs for AddEffect {
    const MODEL: &'static Model = &inputs::ADD_EFFECT;
}

/// `SetEffectParameter`: the arguments of `set_effect_parameter`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetEffectParameter {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Effect or instrument device ID.
    pub device_id: String,
    /// Parameter ID (use with value).
    pub param: Option<String>,
    /// Value for `param`.
    pub value: Option<ParamValue>,
    /// Several parameters at once.
    pub params: IndexMap<String, ParamValue>,
    /// Load a preset first.
    pub preset: Option<String>,
    /// false = bypass.
    pub enabled: Option<bool>,
}

impl ToolArgs for SetEffectParameter {
    const MODEL: &'static Model = &inputs::SET_EFFECT_PARAMETER;
}

/// `RemoveEffect`: the arguments of `remove_effect`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoveEffect {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The effect device.
    pub device_id: String,
}

impl ToolArgs for RemoveEffect {
    const MODEL: &'static Model = &inputs::REMOVE_EFFECT;
}

/// `LoadImpulseResponse`: the arguments of `load_impulse_response`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LoadImpulseResponse {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The convolution reverb device.
    pub device_id: String,
    /// The impulse-response audio file.
    pub path: String,
}

impl ToolArgs for LoadImpulseResponse {
    const MODEL: &'static Model = &inputs::LOAD_IMPULSE_RESPONSE;
}

/// `AutomationPointInput`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationPointInput {
    /// Position (same forms as other positions).
    pub at: Position,
    /// Value in the parameter's units (dB, Hz, 0..1, ...).
    pub value: f64,
    /// Shape of the segment after this point.
    pub curve: CurveShape,
}

/// `CreateAutomation`: the arguments of `create_automation`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateAutomation {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The track that owns the automated target.
    pub track_id: String,
    /// `'volume'`, `'pan'`, `'send:<send_id>'` or `'<device_id>:<param>'` (e.g. `'dev_x:cutoff_hz'`).
    pub target: String,
    /// The automation points.
    pub points: Vec<AutomationPointInput>,
    /// Only replace existing points between the first and last new point.
    pub replace_range: bool,
}

impl ToolArgs for CreateAutomation {
    const MODEL: &'static Model = &inputs::CREATE_AUTOMATION;
}

/// `RenderRange`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RenderRange {
    /// Beats (number, 0 = song start), `'bar:beat[:tick]'` (1-based) or seconds like '12.5s'.
    pub start: Position,
    /// Default: project end.
    pub end: Option<Position>,
    /// Effect tail after the end: auto, none or seconds.
    pub tail: Tail,
    /// Render one track/bus alone (through its output path).
    pub track_id: Option<String>,
    /// Warm up reverbs/delays before 'start'.
    pub preroll_seconds: f64,
}

/// `RenderPreview`: the arguments of `render_preview`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RenderPreview {
    /// Beats (number, 0 = song start), `'bar:beat[:tick]'` (1-based) or seconds like '12.5s'.
    pub start: Position,
    /// Default: project end.
    pub end: Option<Position>,
    /// Effect tail after the end: auto, none or seconds.
    pub tail: Tail,
    /// Render one track/bus alone (through its output path).
    pub track_id: Option<String>,
    /// Warm up reverbs/delays before 'start'.
    pub preroll_seconds: f64,
    /// WAV output path (default: project cache/previews).
    pub path: Option<String>,
    /// Also measure each track's loudness in isolation.
    pub per_track_levels: bool,
    /// Wait for completion (up to timeout) and return the analysis.
    pub wait: bool,
    /// How long `wait` waits, in seconds.
    pub timeout_seconds: f64,
}

impl ToolArgs for RenderPreview {
    const MODEL: &'static Model = &inputs::RENDER_PREVIEW;
}

/// `PlayPreview`: the arguments of `play_preview`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlayPreview {
    /// Start position (default: current playhead).
    pub start: Option<Position>,
    /// Stop (or loop back) here.
    pub end: Option<Position>,
    /// Loop between `start` and `end`.
    pub r#loop: bool,
}

impl ToolArgs for PlayPreview {
    const MODEL: &'static Model = &inputs::PLAY_PREVIEW;
}

/// `FormatSpec`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FormatSpec {
    /// Container and codec.
    pub format: AudioFormat,
    /// PCM bit depth (ignored for lossy formats).
    pub bit_depth: u8,
    /// 32-bit float (WAV/AIFF/CAF/W64/RF64).
    pub floating_point: bool,
    /// Resample on export (default: project rate).
    pub sample_rate: Option<i64>,
    /// Channel count: 1 or 2.
    pub channels: u8,
    /// Lossy target bitrate (MP3 default 320 CBR).
    pub bitrate_kbps: Option<u16>,
    /// Use a variable bitrate (lossy formats).
    pub variable_bitrate: bool,
    /// Dither applied when reducing the bit depth.
    pub dither: Dither,
    /// Explicit file path; default derives from the main path/extension.
    pub path: Option<String>,
}

/// `ExportAudio`: the arguments of `export_audio`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExportAudio {
    /// Beats (number, 0 = song start), `'bar:beat[:tick]'` (1-based) or seconds like '12.5s'.
    pub start: Position,
    /// Default: project end.
    pub end: Option<Position>,
    /// Effect tail after the end: auto, none or seconds.
    pub tail: Tail,
    /// Render one track/bus alone (through its output path).
    pub track_id: Option<String>,
    /// Warm up reverbs/delays before 'start'.
    pub preroll_seconds: f64,
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Token from a previous `CONFIRMATION_REQUIRED` response, after the user agreed.
    pub confirmation_token: Option<String>,
    /// Output file path (the extension may be replaced per format).
    pub path: String,
    /// Output formats (one file each).
    pub formats: Vec<FormatSpec>,
    /// Normalization mode.
    pub normalize: Normalize,
    /// Peak dBFS / dBTP or integrated LUFS target.
    pub target_db: f64,
    /// True-peak ceiling in dBTP.
    pub true_peak_ceiling_db: f64,
    /// Metadata tags written into the files.
    pub metadata: IndexMap<String, String>,
    /// Overwrite existing files (needs confirmation).
    pub overwrite: bool,
    /// Wait for completion and return the result.
    pub wait: bool,
    /// How long `wait` waits, in seconds.
    pub timeout_seconds: f64,
}

impl ToolArgs for ExportAudio {
    const MODEL: &'static Model = &inputs::EXPORT_AUDIO;
}

/// `ExportStems`: the arguments of `export_stems`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "mirrors the prototype's ExportStems input field for field (CTR-01)"
)]
pub struct ExportStems {
    /// Beats (number, 0 = song start), `'bar:beat[:tick]'` (1-based) or seconds like '12.5s'.
    pub start: Position,
    /// Default: project end.
    pub end: Option<Position>,
    /// Effect tail after the end: auto, none or seconds.
    pub tail: Tail,
    /// Render one track/bus alone (through its output path).
    pub track_id: Option<String>,
    /// Warm up reverbs/delays before 'start'.
    pub preroll_seconds: f64,
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Token from a previous `CONFIRMATION_REQUIRED` response, after the user agreed.
    pub confirmation_token: Option<String>,
    /// Output directory.
    pub directory: String,
    /// Default: all audio, instrument and bus tracks.
    pub track_ids: Option<Vec<String>>,
    /// Output format of every stem.
    pub format: FormatSpec,
    /// Include master processing in each stem.
    pub through_master: bool,
    /// Overwrite existing files (needs confirmation).
    pub overwrite: bool,
    /// Wait for completion and return the result.
    pub wait: bool,
    /// How long `wait` waits, in seconds.
    pub timeout_seconds: f64,
}

impl ToolArgs for ExportStems {
    const MODEL: &'static Model = &inputs::EXPORT_STEMS;
}

/// `ExportMidi`: the arguments of `export_midi`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExportMidi {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// Token from a previous `CONFIRMATION_REQUIRED` response, after the user agreed.
    pub confirmation_token: Option<String>,
    /// Output file path.
    pub path: String,
    /// Tracks to export (default: all instrument tracks).
    pub track_ids: Option<Vec<String>>,
    /// Overwrite an existing file (needs confirmation).
    pub overwrite: bool,
}

impl ToolArgs for ExportMidi {
    const MODEL: &'static Model = &inputs::EXPORT_MIDI;
}

/// `AnalyzeAudio`: the arguments of `analyze_audio`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalyzeAudio {
    /// Audio file to analyze.
    pub path: Option<String>,
    /// Project media asset to analyze.
    pub asset_id: Option<String>,
    /// Render this part of the project and analyze it.
    pub render: Option<RenderRange>,
    /// Also measure each track's loudness in isolation.
    pub per_track_levels: bool,
}

impl ToolArgs for AnalyzeAudio {
    const MODEL: &'static Model = &inputs::ANALYZE_AUDIO;
}

/// `GetAudioAnalysis`: the arguments of `get_audio_analysis`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GetAudioAnalysis {
    /// Render/export operation whose analysis to return.
    pub operation_id: Option<String>,
    /// A preview WAV rendered earlier (reads its .analysis.json).
    pub path: Option<String>,
    /// Include the short-term loudness series (1 value/s).
    pub include_series: bool,
}

impl ToolArgs for GetAudioAnalysis {
    const MODEL: &'static Model = &inputs::GET_AUDIO_ANALYSIS;
}

/// `SetMidiInput`: the arguments of `set_midi_input`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetMidiInput {
    /// Open (true) or close (false) MIDI inputs.
    pub enabled: bool,
    /// Input port names; null = all available.
    pub ports: Option<Vec<String>>,
}

impl ToolArgs for SetMidiInput {
    const MODEL: &'static Model = &inputs::SET_MIDI_INPUT;
}

/// `StartRecording`: the arguments of `start_recording`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StartRecording {
    /// Where recording starts (default: current position).
    pub start: Option<Position>,
    /// Tracks to record (default: all armed tracks).
    pub track_ids: Option<Vec<String>>,
}

impl ToolArgs for StartRecording {
    const MODEL: &'static Model = &inputs::START_RECORDING;
}

/// `OperationId`: the arguments of `get_operation_status`, `cancel_operation`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OperationId {
    /// The background operation.
    pub operation_id: String,
    /// Block up to this long for completion.
    pub wait_seconds: f64,
}

impl ToolArgs for OperationId {
    const MODEL: &'static Model = &inputs::OPERATION_ID;
}

/// `Relink`: the arguments of `relink_media`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Relink {
    /// Validate and describe the change without committing it.
    pub dry_run: bool,
    /// The media asset.
    pub asset_id: String,
    /// The file to point the asset at.
    pub path: String,
    /// Copy the file into the project instead of linking.
    pub embed: bool,
}

impl ToolArgs for Relink {
    const MODEL: &'static Model = &inputs::RELINK;
}
