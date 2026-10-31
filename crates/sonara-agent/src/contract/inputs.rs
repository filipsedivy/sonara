//! The input models of the agent tools: a port of the prototype's
//! `sonara.agent.schemas.models` as data, and the models of the tools that a later API version
//! adds (`super::versions`).
//!
//! Fields are listed in pydantic's order. Fields inherited from the prototype's base classes
//! (`Mutating`, `Confirmable`, `RenderRange`) are shared constants, listed where pydantic puts
//! them: base-class fields first, in reverse method resolution order. That is why
//! `ExportAudio(Confirmable, RenderRange)` starts with the render range, then `dry_run` and
//! `confirmation_token`.
//!
//! Conventions (`docs/agent-api.md`): positions are beats from the song start (a number), a
//! `"bar:beat[:tick]"` string or seconds (`"12.5s"`); lengths are beats, `"N bars"`/`"Nb"` or
//! seconds; positions inside a MIDI clip are beats from the clip start.

use serde_json::Value;

use super::model::{
    Constant, Factory, Field, Kind, LengthBounds, Model, Number, NumberBounds, Pattern, StrRules,
};

/// `^\d{1,2}/(1|2|4|8|16|32)$`: a time signature such as `4/4`.
static TIME_SIGNATURE: Pattern = Pattern::new(r"^\d{1,2}/(1|2|4|8|16|32)$");
/// `^#[0-9a-fA-F]{6}$`: a `#rrggbb` colour.
static HEX_COLOR: Pattern = Pattern::new(r"^#[0-9a-fA-F]{6}$");

/// Every pattern of the table, for the test that compiles them.
#[cfg(test)]
pub(crate) static PATTERNS: [&Pattern; 2] = [&TIME_SIGNATURE, &HEX_COLOR];

/// `Position`: a time position as beats, `'bar:beat[:tick]'` or seconds.
pub const POSITION: Kind = Kind::Described(
    &Kind::Union(&[Kind::FLOAT, Kind::STR]),
    "Beats (number, 0 = song start), 'bar:beat[:tick]' (1-based) or seconds like '12.5s'",
);

/// `Length`: a length as beats, bars or seconds.
pub const LENGTH: Kind = Kind::Described(
    &Kind::Union(&[Kind::FLOAT, Kind::STR]),
    "Beats (number), '4 bars' / '4b', or seconds like '30s'",
);

/// `CreateAutomation._target`: the target must not be empty (after whitespace stripping).
fn target_not_empty(value: &Value) -> Option<&'static str> {
    (value.as_str() == Some("")).then_some("target must not be empty")
}

// -- Mutating and Confirmable ---------------------------------------------------------------

/// `Mutating.dry_run`.
const DRY_RUN: Field = Field::new("dry_run", Kind::Bool)
    .default_value(Constant::Bool(false))
    .description("Validate and describe the change without committing it");

/// `Confirmable.confirmation_token`.
const CONFIRMATION_TOKEN: Field = Field::new("confirmation_token", Kind::Nullable(&Kind::STR))
    .default_value(Constant::Null)
    .description("Token from a previous CONFIRMATION_REQUIRED response, after the user agreed");

// -- project ------------------------------------------------------------------------------

/// `CreateProject`: the arguments of `create_project`.
pub static CREATE_PROJECT: Model = Model::tool_input(
    "CreateProject",
    &[
        DRY_RUN,
        CONFIRMATION_TOKEN,
        Field::new(
            "name",
            Kind::Str(StrRules::NONE.min_length(1).max_length(200)),
        )
        .default_value(Constant::Str("Untitled")),
        Field::new(
            "bpm",
            Kind::Float(NumberBounds::NONE.ge(Number::Int(10)).le(Number::Int(999))),
        )
        .default_value(Constant::Float(120.0))
        .description("Initial tempo"),
        Field::new(
            "time_signature",
            Kind::Str(StrRules::NONE.pattern(&TIME_SIGNATURE)),
        )
        .default_value(Constant::Str("4/4")),
        Field::new(
            "sample_rate",
            Kind::IntLiteral(&[44100, 48000, 88200, 96000]),
        )
        .default_value(Constant::Int(48000)),
        Field::new(
            "template",
            Kind::StrLiteral(&["empty", "electronic", "band", "orchestral"]),
        )
        .default_value(Constant::Str("empty"))
        .description("Starting point; 'empty' has no tracks"),
        Field::new("length", Kind::Nullable(&LENGTH))
            .default_value(Constant::Null)
            .description("Optional song length (sets the project end)"),
        Field::new("path", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Save immediately to this .sonara path"),
    ],
);

/// `OpenProject`: the arguments of `open_project`.
pub static OPEN_PROJECT: Model = Model::tool_input(
    "OpenProject",
    &[
        DRY_RUN,
        CONFIRMATION_TOKEN,
        Field::new("path", Kind::STR).description("Path to a .sonara bundle directory or archive"),
        Field::new("recover_autosave", Kind::Bool)
            .default_value(Constant::Bool(false))
            .description("Restore unsaved changes from the autosave if present"),
    ],
);

/// `SaveProject`: the arguments of `save_project`.
pub static SAVE_PROJECT: Model = Model::tool_input(
    "SaveProject",
    &[
        DRY_RUN,
        CONFIRMATION_TOKEN,
        Field::new("path", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Target path (Save As); omit to save in place"),
        Field::new("packaged", Kind::Bool)
            .default_value(Constant::Bool(false))
            .description("Write a single-file ZIP archive instead of a bundle directory"),
        Field::new("as_copy", Kind::Bool)
            .default_value(Constant::Bool(false))
            .description("Save a copy without switching the open project to it"),
    ],
);

/// `GetProjectState`: the arguments of `get_project_state`.
pub static GET_PROJECT_STATE: Model = Model::tool_input(
    "GetProjectState",
    &[
        Field::new("detail", Kind::StrLiteral(&["summary", "full"]))
            .default_value(Constant::Str("summary"))
            .description("'full' adds devices, parameters and automation"),
        Field::new("include_notes", Kind::Bool)
            .default_value(Constant::Bool(false))
            .description("Include every MIDI note (can be large)"),
        Field::new(
            "track_ids",
            Kind::Nullable(&Kind::List(&Kind::STR, LengthBounds::NONE)),
        )
        .default_value(Constant::Null)
        .description("Limit the report to these tracks"),
    ],
);

/// `Empty`: the arguments of `list_tracks`, `list_midi_devices`, `stop_playback`,
/// `list_operations`, `undo`, `redo`, `get_history`, `verify_project` and `get_capabilities`.
pub static EMPTY: Model = Model::tool_input("Empty", &[]);

/// `SetTempo`: the arguments of `set_tempo`.
pub static SET_TEMPO: Model = Model::tool_input(
    "SetTempo",
    &[
        DRY_RUN,
        Field::new(
            "bpm",
            Kind::Float(NumberBounds::NONE.ge(Number::Int(10)).le(Number::Int(999))),
        ),
        Field::new("at", POSITION)
            .default_value(Constant::Float(0.0))
            .description("Where the tempo starts (0 = song tempo)"),
        Field::new("ramp", Kind::Bool)
            .default_value(Constant::Bool(false))
            .description("Glide linearly from this point to the next tempo point"),
        Field::new("clear_changes", Kind::Bool)
            .default_value(Constant::Bool(false))
            .description("Replace all tempo changes with one constant tempo"),
    ],
);

/// `SetTimeSignature`: the arguments of `set_time_signature`.
pub static SET_TIME_SIGNATURE: Model = Model::tool_input(
    "SetTimeSignature",
    &[
        DRY_RUN,
        Field::new(
            "numerator",
            Kind::Int(NumberBounds::NONE.ge(Number::Int(1)).le(Number::Int(64))),
        ),
        Field::new("denominator", Kind::IntLiteral(&[1, 2, 4, 8, 16, 32])),
        Field::new("bar", Kind::Int(NumberBounds::NONE.ge(Number::Int(1))))
            .default_value(Constant::Int(1))
            .description("1-based bar where the signature starts"),
    ],
);

/// `SetProjectLength`: the arguments of `set_project_length`.
pub static SET_PROJECT_LENGTH: Model = Model::tool_input(
    "SetProjectLength",
    &[
        DRY_RUN,
        Field::new("length", Kind::Nullable(&LENGTH))
            .description("Song length; null = end of the last clip"),
    ],
);

/// `AddSection`: the arguments of `add_section`.
pub static ADD_SECTION: Model = Model::tool_input(
    "AddSection",
    &[
        DRY_RUN,
        Field::new(
            "name",
            Kind::Str(StrRules::NONE.min_length(1).max_length(80)),
        ),
        Field::new("start", POSITION),
        Field::new("length", LENGTH),
        Field::new(
            "color",
            Kind::Nullable(&Kind::Str(StrRules::NONE.pattern(&HEX_COLOR))),
        )
        .default_value(Constant::Null),
    ],
);

/// `AddMarker`: the arguments of `add_marker`.
pub static ADD_MARKER: Model = Model::tool_input(
    "AddMarker",
    &[
        DRY_RUN,
        Field::new(
            "name",
            Kind::Str(StrRules::NONE.min_length(1).max_length(80)),
        ),
        Field::new("at", POSITION),
    ],
);

/// `SetMetadata`: the arguments of `set_metadata`.
pub static SET_METADATA: Model = Model::tool_input(
    "SetMetadata",
    &[
        DRY_RUN,
        Field::new("title", Kind::Nullable(&Kind::STR)).default_value(Constant::Null),
        Field::new("artist", Kind::Nullable(&Kind::STR)).default_value(Constant::Null),
        Field::new("album", Kind::Nullable(&Kind::STR)).default_value(Constant::Null),
        Field::new("genre", Kind::Nullable(&Kind::STR)).default_value(Constant::Null),
        Field::new("composer", Kind::Nullable(&Kind::STR)).default_value(Constant::Null),
        Field::new("comment", Kind::Nullable(&Kind::STR)).default_value(Constant::Null),
    ],
);

/// `SetLoop`: the arguments of `set_loop`.
pub static SET_LOOP: Model = Model::tool_input(
    "SetLoop",
    &[
        DRY_RUN,
        Field::new("enabled", Kind::Bool).default_value(Constant::Bool(true)),
        Field::new("start", Kind::Nullable(&POSITION)).default_value(Constant::Null),
        Field::new("end", Kind::Nullable(&POSITION)).default_value(Constant::Null),
    ],
);

// -- tracks -------------------------------------------------------------------------------

/// `NotationSpec`: how a part of `create_track` is notated where it differs from its
/// instrument. The bounds and clefs are those of a track part in the project format.
pub static NOTATION_SPEC: Model = Model::plain(
    "NotationSpec",
    &[
        Field::new(
            "transposition",
            Kind::Nullable(&Kind::Int(
                NumberBounds::NONE.ge(Number::Int(-36)).le(Number::Int(36)),
            )),
        )
        .default_value(Constant::Null)
        .description("Sounding minus written pitch, in semitones"),
        Field::new(
            "clef",
            Kind::Nullable(&Kind::StrLiteral(&[
                "treble",
                "treble_8vb",
                "bass",
                "alto",
                "tenor",
                "percussion",
                "single_line",
            ])),
        )
        .default_value(Constant::Null),
    ],
);

/// `PartSpec`: the part an instrument track of `create_track` plays. The fields and their
/// bounds are those of a track part in the project format.
pub static PART_SPEC: Model = Model::plain(
    "PartSpec",
    &[
        Field::new("instrument", Kind::STR)
            .description("Instrument type id, e.g. 'violin', 'horn_f', 'timpani'"),
        Field::new(
            "name",
            Kind::Nullable(&Kind::Str(StrRules::NONE.min_length(1))),
        )
        .default_value(Constant::Null)
        .description("Printed part name, e.g. 'Violins I', 'Horn 3'"),
        Field::new(
            "players",
            Kind::Nullable(&Kind::Int(
                NumberBounds::NONE.ge(Number::Int(1)).le(Number::Int(128)),
            )),
        )
        .default_value(Constant::Null)
        .description("How many players play the part"),
        Field::new("solo", Kind::Nullable(&Kind::Bool))
            .default_value(Constant::Null)
            .description("One player (true) or a section (false)"),
        Field::new(
            "player",
            Kind::Nullable(&Kind::Str(StrRules::NONE.min_length(1))),
        )
        .default_value(Constant::Null)
        .description("The player who doubles this instrument, e.g. 'Flute 2' for a piccolo part"),
        Field::new("notation", Kind::Nullable(&Kind::Model(&NOTATION_SPEC)))
            .default_value(Constant::Null)
            .description("Notation where it differs from the instrument's"),
    ],
);

/// `CreateTrack`: the arguments of `create_track`.
pub static CREATE_TRACK: Model = Model::tool_input(
    "CreateTrack",
    &[
        DRY_RUN,
        Field::new(
            "name",
            Kind::Str(StrRules::NONE.min_length(1).max_length(120)),
        ),
        Field::new(
            "kind",
            Kind::StrLiteral(&["audio", "instrument", "bus", "folder"]),
        )
        .default_value(Constant::Str("instrument")),
        Field::new("instrument", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Instrument plugin_id (instrument tracks), e.g. 'sonara.synth.poly'"),
        Field::new("preset", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Instrument preset name, e.g. 'Analog Bass'"),
        Field::new(
            "color",
            Kind::Nullable(&Kind::Str(StrRules::NONE.pattern(&HEX_COLOR))),
        )
        .default_value(Constant::Null),
        Field::new(
            "index",
            Kind::Nullable(&Kind::Int(NumberBounds::NONE.ge(Number::Int(0)))),
        )
        .default_value(Constant::Null)
        .description("Position in the track list (default: end)"),
        Field::new("output", Kind::STR)
            .default_value(Constant::Str("master"))
            .description("Output: 'master' or a bus track_id"),
        Field::new("parent_id", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Folder track to place the track in"),
        Field::new("part", Kind::Nullable(&Kind::Model(&PART_SPEC)))
            .default_value(Constant::Null)
            .description(
                "The part an instrument track plays: its instrument type, printed name, \
                 players and notation",
            ),
    ],
);

/// `RenameTrack`: the arguments of `rename_track`.
pub static RENAME_TRACK: Model = Model::tool_input(
    "RenameTrack",
    &[
        DRY_RUN,
        Field::new("track_id", Kind::STR),
        Field::new(
            "name",
            Kind::Str(StrRules::NONE.min_length(1).max_length(120)),
        ),
    ],
);

/// `DeleteTrack`: the arguments of `delete_track`.
pub static DELETE_TRACK: Model = Model::tool_input(
    "DeleteTrack",
    &[
        DRY_RUN,
        CONFIRMATION_TOKEN,
        Field::new("track_id", Kind::STR),
    ],
);

/// `SetTrackVolume`: the arguments of `set_track_volume`.
pub static SET_TRACK_VOLUME: Model = Model::tool_input(
    "SetTrackVolume",
    &[
        DRY_RUN,
        Field::new("track_id", Kind::STR).description("Track ID or 'master'"),
        Field::new(
            "volume_db",
            Kind::Float(NumberBounds::NONE.ge(Number::Int(-144)).le(Number::Int(12))),
        )
        .description("Fader level in dB (0 = unity)"),
    ],
);

/// `SetTrackPan`: the arguments of `set_track_pan`.
pub static SET_TRACK_PAN: Model = Model::tool_input(
    "SetTrackPan",
    &[
        DRY_RUN,
        Field::new("track_id", Kind::STR),
        Field::new(
            "pan",
            Kind::Float(NumberBounds::NONE.ge(Number::Int(-1)).le(Number::Int(1))),
        )
        .description("-1 = hard left, 0 = centre, +1 = hard right"),
    ],
);

/// `SetTrackState`: the arguments of `set_track_state`.
pub static SET_TRACK_STATE: Model = Model::tool_input(
    "SetTrackState",
    &[
        DRY_RUN,
        Field::new("track_id", Kind::STR),
        Field::new("mute", Kind::Nullable(&Kind::Bool)).default_value(Constant::Null),
        Field::new("solo", Kind::Nullable(&Kind::Bool)).default_value(Constant::Null),
        Field::new("solo_safe", Kind::Nullable(&Kind::Bool)).default_value(Constant::Null),
        Field::new(
            "color",
            Kind::Nullable(&Kind::Str(StrRules::NONE.pattern(&HEX_COLOR))),
        )
        .default_value(Constant::Null),
        Field::new("armed", Kind::Nullable(&Kind::Bool))
            .default_value(Constant::Null)
            .description("Arm for recording"),
    ],
);

/// `SetInstrument`: the arguments of `set_instrument`.
pub static SET_INSTRUMENT: Model = Model::tool_input(
    "SetInstrument",
    &[
        DRY_RUN,
        Field::new("track_id", Kind::STR),
        Field::new("plugin_id", Kind::STR).description("Instrument plugin_id"),
        Field::new("preset", Kind::Nullable(&Kind::STR)).default_value(Constant::Null),
        Field::new(
            "params",
            Kind::Dict(&Kind::Union(&[
                Kind::FLOAT,
                Kind::INT,
                Kind::Bool,
                Kind::STR,
            ])),
        )
        .default_factory(Factory::EmptyDict),
    ],
);

/// `SendSpec`: an item of `configure_routing.add_sends`.
pub static SEND_SPEC: Model = Model::plain(
    "SendSpec",
    &[
        Field::new("target", Kind::STR).description("Bus track_id"),
        Field::new(
            "level_db",
            Kind::Float(NumberBounds::NONE.ge(Number::Int(-144)).le(Number::Int(12))),
        )
        .default_value(Constant::Float(0.0)),
        Field::new("pre_fader", Kind::Bool).default_value(Constant::Bool(false)),
    ],
);

/// `ConfigureRouting`: the arguments of `configure_routing`.
pub static CONFIGURE_ROUTING: Model = Model::tool_input(
    "ConfigureRouting",
    &[
        DRY_RUN,
        Field::new("track_id", Kind::STR),
        Field::new("output", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("New output: 'master' or a bus track_id"),
        Field::new(
            "add_sends",
            Kind::List(&Kind::Model(&SEND_SPEC), LengthBounds::NONE),
        )
        .default_factory(Factory::EmptyList),
        Field::new("update_sends", Kind::Dict(&Kind::FLOAT))
            .default_factory(Factory::EmptyDict)
            .description("send_id -> new level in dB"),
        Field::new("remove_sends", Kind::List(&Kind::STR, LengthBounds::NONE))
            .default_factory(Factory::EmptyList)
            .description("send IDs to remove"),
        Field::new("sidechain_device_id", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Device on this track whose sidechain to set"),
        Field::new("sidechain_source", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Track ID feeding the sidechain ('' clears it)"),
    ],
);

// -- clips and notes ----------------------------------------------------------------------

/// `NoteInput`: a note of `create_midi_clip`, `add_midi_notes` and `edit_midi_notes.replace_all`.
pub static NOTE_INPUT: Model = Model::plain(
    "NoteInput",
    &[
        Field::new("pitch", Kind::Union(&[Kind::INT, Kind::STR]))
            .description("MIDI number 0-127 or a name like 'C4' (C4 = 60), 'F#3', 'Bb2'"),
        Field::new("start", Kind::Float(NumberBounds::NONE.ge(Number::Int(0))))
            .description("Beats from the clip start"),
        Field::new(
            "length",
            Kind::Float(NumberBounds::NONE.gt(Number::Int(0)).le(Number::Int(4096))),
        )
        .description("Duration in beats"),
        Field::new(
            "velocity",
            Kind::Int(NumberBounds::NONE.ge(Number::Int(1)).le(Number::Int(127))),
        )
        .default_value(Constant::Int(100)),
        Field::new(
            "channel",
            Kind::Int(NumberBounds::NONE.ge(Number::Int(0)).le(Number::Int(15))),
        )
        .default_value(Constant::Int(0)),
        Field::new("articulation", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description(
                "How the note is played: an articulation id such as 'staccato', 'pizzicato', \
                 'legato' or 'tremolo'; null = the instrument's ordinary sound. Mutes and \
                 vibrato hold over a passage and are not note articulations",
            ),
    ],
);

/// `CreateMidiClip`: the arguments of `create_midi_clip`.
pub static CREATE_MIDI_CLIP: Model = Model::tool_input(
    "CreateMidiClip",
    &[
        DRY_RUN,
        Field::new("track_id", Kind::STR),
        Field::new("start", POSITION),
        Field::new("length", LENGTH),
        Field::new("name", Kind::Str(StrRules::NONE.max_length(120)))
            .default_value(Constant::Str("MIDI")),
        Field::new(
            "notes",
            Kind::List(
                &Kind::Model(&NOTE_INPUT),
                LengthBounds::NONE.max_length(20000),
            ),
        )
        .default_factory(Factory::EmptyList),
        Field::new("loop_length", Kind::Nullable(&LENGTH))
            .default_value(Constant::Null)
            .description("Repeat the first N beats of content across the clip"),
        Field::new(
            "color",
            Kind::Nullable(&Kind::Str(StrRules::NONE.pattern(&HEX_COLOR))),
        )
        .default_value(Constant::Null),
    ],
);

/// `AddMidiNotes`: the arguments of `add_midi_notes`.
pub static ADD_MIDI_NOTES: Model = Model::tool_input(
    "AddMidiNotes",
    &[
        DRY_RUN,
        Field::new("clip_id", Kind::STR),
        Field::new(
            "notes",
            Kind::List(
                &Kind::Model(&NOTE_INPUT),
                LengthBounds::NONE.min_length(1).max_length(20000),
            ),
        ),
    ],
);

/// `NoteEditInput`: an item of `edit_midi_notes.edits`.
pub static NOTE_EDIT_INPUT: Model = Model::plain(
    "NoteEditInput",
    &[
        Field::new("note_id", Kind::STR),
        Field::new(
            "pitch",
            Kind::Nullable(&Kind::Union(&[Kind::INT, Kind::STR])),
        )
        .default_value(Constant::Null),
        Field::new(
            "start",
            Kind::Nullable(&Kind::Float(NumberBounds::NONE.ge(Number::Int(0)))),
        )
        .default_value(Constant::Null),
        Field::new(
            "length",
            Kind::Nullable(&Kind::Float(NumberBounds::NONE.gt(Number::Int(0)))),
        )
        .default_value(Constant::Null),
        Field::new(
            "velocity",
            Kind::Nullable(&Kind::Int(
                NumberBounds::NONE.ge(Number::Int(1)).le(Number::Int(127)),
            )),
        )
        .default_value(Constant::Null),
    ],
);

/// `QuantizeSpec`: `edit_midi_notes.quantize`.
pub static QUANTIZE_SPEC: Model = Model::plain(
    "QuantizeSpec",
    &[
        Field::new("grid", Kind::Float(NumberBounds::NONE.gt(Number::Int(0))))
            .description("Grid in beats (0.25 = sixteenths)"),
        Field::new(
            "strength",
            Kind::Float(NumberBounds::NONE.ge(Number::Int(0)).le(Number::Int(1))),
        )
        .default_value(Constant::Float(1.0)),
        Field::new(
            "swing",
            Kind::Float(NumberBounds::NONE.ge(Number::Int(0)).le(Number::Int(1))),
        )
        .default_value(Constant::Float(0.0)),
        Field::new("ends", Kind::Bool).default_value(Constant::Bool(false)),
    ],
);

/// `HumanizeSpec`: `edit_midi_notes.humanize`.
pub static HUMANIZE_SPEC: Model = Model::plain(
    "HumanizeSpec",
    &[
        Field::new(
            "timing",
            Kind::Float(
                NumberBounds::NONE
                    .ge(Number::Int(0))
                    .le(Number::Float(0.25)),
            ),
        )
        .default_value(Constant::Float(0.01))
        .description("Timing deviation (std. dev., beats)"),
        Field::new(
            "velocity",
            Kind::Int(NumberBounds::NONE.ge(Number::Int(0)).le(Number::Int(64))),
        )
        .default_value(Constant::Int(8)),
        Field::new("seed", Kind::INT).default_value(Constant::Int(0)),
    ],
);

/// `EditMidiNotes`: the arguments of `edit_midi_notes`.
pub static EDIT_MIDI_NOTES: Model = Model::tool_input(
    "EditMidiNotes",
    &[
        DRY_RUN,
        Field::new("clip_id", Kind::STR),
        Field::new(
            "edits",
            Kind::List(
                &Kind::Model(&NOTE_EDIT_INPUT),
                LengthBounds::NONE.max_length(20000),
            ),
        )
        .default_factory(Factory::EmptyList),
        Field::new(
            "delete_note_ids",
            Kind::List(&Kind::STR, LengthBounds::NONE),
        )
        .default_factory(Factory::EmptyList),
        Field::new(
            "transpose",
            Kind::Nullable(&Kind::Int(
                NumberBounds::NONE.ge(Number::Int(-48)).le(Number::Int(48)),
            )),
        )
        .default_value(Constant::Null)
        .description("Semitones applied to the selected notes"),
        Field::new("quantize", Kind::Nullable(&Kind::Model(&QUANTIZE_SPEC)))
            .default_value(Constant::Null),
        Field::new("humanize", Kind::Nullable(&Kind::Model(&HUMANIZE_SPEC)))
            .default_value(Constant::Null),
        Field::new(
            "velocity_scale",
            Kind::Nullable(&Kind::Float(
                NumberBounds::NONE.gt(Number::Int(0)).le(Number::Int(4)),
            )),
        )
        .default_value(Constant::Null),
        Field::new(
            "note_ids",
            Kind::Nullable(&Kind::List(&Kind::STR, LengthBounds::NONE)),
        )
        .default_value(Constant::Null)
        .description("Restrict transpose/quantize/humanize/velocity to these notes"),
        Field::new(
            "replace_all",
            Kind::Nullable(&Kind::List(
                &Kind::Model(&NOTE_INPUT),
                LengthBounds::NONE.max_length(20000),
            )),
        )
        .default_value(Constant::Null)
        .description("Replace the clip's notes entirely"),
    ],
);

/// `ControllerEventInput`: an event of `write_midi_controllers` (added in agent API 1.1).
pub static CONTROLLER_EVENT_INPUT: Model = Model::plain(
    "ControllerEventInput",
    &[
        Field::new("beat", Kind::Float(NumberBounds::NONE.ge(Number::Int(0))))
            .description("Beats from the clip start"),
        Field::new(
            "value",
            Kind::Int(NumberBounds::NONE.ge(Number::Int(0)).le(Number::Int(127))),
        )
        .description("Controller value 0-127; it holds until the controller's next event"),
    ],
);

/// `WriteMidiControllers`: the arguments of `write_midi_controllers` (added in agent API 1.1).
pub static WRITE_MIDI_CONTROLLERS: Model = Model::tool_input(
    "WriteMidiControllers",
    &[
        DRY_RUN,
        Field::new("clip_id", Kind::STR),
        Field::new(
            "controller",
            Kind::Int(NumberBounds::NONE.ge(Number::Int(0)).le(Number::Int(127))),
        )
        .description(
            "MIDI CC number, for example 1 = modulation, 7 = volume, 11 = expression, \
             64 = sustain pedal (64 or more is down)",
        ),
        Field::new(
            "events",
            Kind::List(
                &Kind::Model(&CONTROLLER_EVENT_INPUT),
                LengthBounds::NONE.min_length(1).max_length(20000),
            ),
        )
        .description("The controller's new events; they replace all of its events in the clip"),
        Field::new(
            "channel",
            Kind::Int(NumberBounds::NONE.ge(Number::Int(0)).le(Number::Int(15))),
        )
        .default_value(Constant::Int(0))
        .description(
            "MIDI channel of the new events, as a note's channel; the controller's events are \
             replaced on every channel",
        ),
    ],
);

/// `GenerateMidi`: the arguments of `generate_midi`.
pub static GENERATE_MIDI: Model = Model::tool_input(
    "GenerateMidi",
    &[
    DRY_RUN,
    Field::new("track_id", Kind::STR).description("Instrument track receiving a new clip"),
    Field::new("kind", Kind::StrLiteral(&["chords", "arpeggio", "bass", "drums", "melody"])),
    Field::new("start", POSITION).default_value(Constant::Float(0.0)),
    Field::new("length", Kind::Nullable(&LENGTH)).default_value(Constant::Null).description("Clip length; default = the progression length (or bars*4)"),
    Field::new("key", Kind::Nullable(&Kind::STR)).default_value(Constant::Null).description("Key like 'A minor', 'F# major', 'D dorian' (Roman numerals need it)"),
    Field::new("progression", Kind::Nullable(&Kind::List(&Kind::STR, LengthBounds::NONE))).default_value(Constant::Null).description("Roman numerals ('i','VI','III','VII', 'V7') or chord symbols ('Am','F','C','G7')"),
    Field::new("beats_per_chord", Kind::Union(&[Kind::FLOAT, Kind::List(&Kind::FLOAT, LengthBounds::NONE)])).default_value(Constant::Float(4.0)).description("Chord duration(s) in beats"),
    Field::new("style", Kind::Nullable(&Kind::STR)).default_value(Constant::Null).description("chords: sustain|stabs|offbeat|pulse8|half; arpeggio: up|down|updown|downup|random|pinky; bass: root|pulse|octaves|offbeat|fifths|walking|syncopated; drums: four_on_the_floor|house|techno|breakbeat|hiphop|trap|rock|halftime|dnb|ride_jazz|cinematic"),
    Field::new("bars", Kind::Int(NumberBounds::NONE.ge(Number::Int(1)).le(Number::Int(512)))).default_value(Constant::Int(4)).description("Drums: number of bars"),
    Field::new("octave", Kind::Int(NumberBounds::NONE.ge(Number::Int(0)).le(Number::Int(8)))).default_value(Constant::Int(4)).description("Register: chord/arp centre octave (C4 = 60) or bass octave"),
    Field::new("rate", Kind::Float(NumberBounds::NONE.gt(Number::Int(0)).le(Number::Int(8)))).default_value(Constant::Float(0.5)).description("Arpeggio/bass/melody note rate in beats"),
    Field::new("velocity", Kind::Int(NumberBounds::NONE.ge(Number::Int(1)).le(Number::Int(127)))).default_value(Constant::Int(95)),
    Field::new("variation", Kind::Float(NumberBounds::NONE.ge(Number::Int(0)).le(Number::Int(1)))).default_value(Constant::Float(0.2)).description("Drums: ghost notes and kick variation"),
    Field::new("fill", Kind::Bool).default_value(Constant::Bool(true)).description("Drums: fill in the last half bar"),
    Field::new("swing", Kind::Float(NumberBounds::NONE.ge(Number::Int(0)).le(Number::Int(1)))).default_value(Constant::Float(0.0)),
    Field::new("humanize", Kind::Bool).default_value(Constant::Bool(true)).description("Small, reproducible timing/velocity deviations"),
    Field::new("seed", Kind::INT).default_value(Constant::Int(0)).description("Random seed; same inputs + seed = same notes"),
    Field::new("name", Kind::Nullable(&Kind::STR)).default_value(Constant::Null),
    Field::new("clip_id", Kind::Nullable(&Kind::STR)).default_value(Constant::Null).description("Append into this existing clip instead of creating one"),
    ],
);

/// `ImportAudio`: the arguments of `import_audio`.
pub static IMPORT_AUDIO: Model = Model::tool_input(
    "ImportAudio",
    &[
        DRY_RUN,
        Field::new("path", Kind::STR),
        Field::new("track_id", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Existing audio track; omit to create a new track"),
        Field::new("start", POSITION).default_value(Constant::Float(0.0)),
        Field::new("mode", Kind::StrLiteral(&["copy", "link"]))
            .default_value(Constant::Str("copy"))
            .description("copy into the project (safe) or link in place"),
        Field::new("track_name", Kind::Nullable(&Kind::STR)).default_value(Constant::Null),
    ],
);

/// `ImportMidi`: the arguments of `import_midi`.
pub static IMPORT_MIDI: Model = Model::tool_input(
    "ImportMidi",
    &[
        DRY_RUN,
        Field::new("path", Kind::STR),
        Field::new("apply_tempo", Kind::Bool)
            .default_value(Constant::Bool(true))
            .description("Use the file's tempo map and time signatures"),
        Field::new("start", POSITION).default_value(Constant::Float(0.0)),
    ],
);

/// `MoveClip`: the arguments of `move_clip`.
pub static MOVE_CLIP: Model = Model::tool_input(
    "MoveClip",
    &[
        DRY_RUN,
        Field::new("clip_id", Kind::STR),
        Field::new("start", Kind::Nullable(&POSITION)).default_value(Constant::Null),
        Field::new("track_id", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Move to another compatible track"),
    ],
);

/// `EditClip`: the arguments of `edit_clip`.
pub static EDIT_CLIP: Model = Model::tool_input(
    "EditClip",
    &[
        DRY_RUN,
        Field::new("clip_id", Kind::STR),
        Field::new("name", Kind::Nullable(&Kind::STR)).default_value(Constant::Null),
        Field::new("muted", Kind::Nullable(&Kind::Bool)).default_value(Constant::Null),
        Field::new(
            "gain_db",
            Kind::Nullable(&Kind::Float(
                NumberBounds::NONE.ge(Number::Int(-96)).le(Number::Int(24)),
            )),
        )
        .default_value(Constant::Null)
        .description("Audio clips only"),
        Field::new(
            "fade_in_seconds",
            Kind::Nullable(&Kind::Float(NumberBounds::NONE.ge(Number::Int(0)))),
        )
        .default_value(Constant::Null),
        Field::new(
            "fade_out_seconds",
            Kind::Nullable(&Kind::Float(NumberBounds::NONE.ge(Number::Int(0)))),
        )
        .default_value(Constant::Null),
        Field::new(
            "fade_curve",
            Kind::Nullable(&Kind::StrLiteral(&[
                "linear",
                "equal_power",
                "exponential",
                "s_curve",
            ])),
        )
        .default_value(Constant::Null),
        Field::new("trim_start", Kind::Nullable(&POSITION))
            .default_value(Constant::Null)
            .description("New clip start (content stays in place)"),
        Field::new("trim_end", Kind::Nullable(&POSITION))
            .default_value(Constant::Null)
            .description("New clip end"),
        Field::new("loop_length", Kind::Nullable(&LENGTH))
            .default_value(Constant::Null)
            .description("MIDI clips: repeat the first N beats"),
    ],
);

/// `SplitClip`: the arguments of `split_clip`.
pub static SPLIT_CLIP: Model = Model::tool_input(
    "SplitClip",
    &[
        DRY_RUN,
        Field::new("clip_id", Kind::STR),
        Field::new("at", POSITION),
    ],
);

/// `DuplicateClip`: the arguments of `duplicate_clip`.
pub static DUPLICATE_CLIP: Model = Model::tool_input(
    "DuplicateClip",
    &[
        DRY_RUN,
        Field::new("clip_id", Kind::STR),
        Field::new(
            "times",
            Kind::Int(NumberBounds::NONE.ge(Number::Int(1)).le(Number::Int(256))),
        )
        .default_value(Constant::Int(1))
        .description("Number of copies placed back to back"),
        Field::new("start", Kind::Nullable(&POSITION))
            .default_value(Constant::Null)
            .description("Start of the first copy (default: right after the clip)"),
        Field::new("track_id", Kind::Nullable(&Kind::STR)).default_value(Constant::Null),
    ],
);

/// `DeleteClips`: the arguments of `delete_clips`.
pub static DELETE_CLIPS: Model = Model::tool_input(
    "DeleteClips",
    &[
        DRY_RUN,
        Field::new(
            "clip_ids",
            Kind::List(&Kind::STR, LengthBounds::NONE.min_length(1)),
        ),
    ],
);

// -- devices and automation ---------------------------------------------------------------

/// `ListPlugins`: the arguments of `list_plugins`.
pub static LIST_PLUGINS: Model = Model::tool_input(
    "ListPlugins",
    &[
        Field::new(
            "kind",
            Kind::Nullable(&Kind::StrLiteral(&["effect", "instrument"])),
        )
        .default_value(Constant::Null),
        Field::new("plugin_id", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Return full parameter schema for one plugin"),
    ],
);

/// `AddEffect`: the arguments of `add_effect`.
pub static ADD_EFFECT: Model = Model::tool_input(
    "AddEffect",
    &[
        DRY_RUN,
        Field::new("track_id", Kind::STR).description("Track, bus or 'master'"),
        Field::new("plugin_id", Kind::STR)
            .description("Effect plugin_id, e.g. 'sonara.dynamics.compressor'"),
        Field::new("preset", Kind::Nullable(&Kind::STR)).default_value(Constant::Null),
        Field::new(
            "params",
            Kind::Dict(&Kind::Union(&[
                Kind::FLOAT,
                Kind::INT,
                Kind::Bool,
                Kind::STR,
            ])),
        )
        .default_factory(Factory::EmptyDict),
        Field::new(
            "index",
            Kind::Nullable(&Kind::Int(NumberBounds::NONE.ge(Number::Int(0)))),
        )
        .default_value(Constant::Null)
        .description("Insert position (default: end of chain)"),
        Field::new("sidechain_source", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Track ID keying the sidechain (compressor/gate)"),
    ],
);

/// `SetEffectParameter`: the arguments of `set_effect_parameter`.
pub static SET_EFFECT_PARAMETER: Model = Model::tool_input(
    "SetEffectParameter",
    &[
        DRY_RUN,
        Field::new("device_id", Kind::STR).description("Effect or instrument device ID"),
        Field::new("param", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Parameter ID (use with value)"),
        Field::new(
            "value",
            Kind::Nullable(&Kind::Union(&[
                Kind::FLOAT,
                Kind::INT,
                Kind::Bool,
                Kind::STR,
            ])),
        )
        .default_value(Constant::Null),
        Field::new(
            "params",
            Kind::Dict(&Kind::Union(&[
                Kind::FLOAT,
                Kind::INT,
                Kind::Bool,
                Kind::STR,
            ])),
        )
        .default_factory(Factory::EmptyDict)
        .description("Several parameters at once"),
        Field::new("preset", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Load a preset first"),
        Field::new("enabled", Kind::Nullable(&Kind::Bool))
            .default_value(Constant::Null)
            .description("false = bypass"),
    ],
);

/// `RemoveEffect`: the arguments of `remove_effect`.
pub static REMOVE_EFFECT: Model = Model::tool_input(
    "RemoveEffect",
    &[DRY_RUN, Field::new("device_id", Kind::STR)],
);

/// `LoadImpulseResponse`: the arguments of `load_impulse_response`.
pub static LOAD_IMPULSE_RESPONSE: Model = Model::tool_input(
    "LoadImpulseResponse",
    &[
        DRY_RUN,
        Field::new("device_id", Kind::STR),
        Field::new("path", Kind::STR),
    ],
);

/// `AutomationPointInput`: a point of `create_automation.points`.
pub static AUTOMATION_POINT_INPUT: Model = Model::plain(
    "AutomationPointInput",
    &[
        Field::new("at", Kind::Union(&[Kind::FLOAT, Kind::STR]))
            .description("Position (same forms as other positions)"),
        Field::new("value", Kind::FLOAT)
            .description("Value in the parameter's units (dB, Hz, 0..1, ...)"),
        Field::new("curve", Kind::StrLiteral(&["linear", "hold", "smooth"]))
            .default_value(Constant::Str("linear"))
            .description("Shape of the segment after this point"),
    ],
);

/// `CreateAutomation`: the arguments of `create_automation`.
pub static CREATE_AUTOMATION: Model = Model::tool_input(
    "CreateAutomation",
    &[
    DRY_RUN,
    Field::new("track_id", Kind::STR),
    Field::new("target", Kind::STR).description("'volume', 'pan', 'send:<send_id>' or '<device_id>:<param>' (e.g. 'dev_x:cutoff_hz')").after(target_not_empty),
    Field::new("points", Kind::List(&Kind::Model(&AUTOMATION_POINT_INPUT), LengthBounds::NONE.min_length(1).max_length(10000))),
    Field::new("replace_range", Kind::Bool).default_value(Constant::Bool(false)).description("Only replace existing points between the first and last new point"),
    ],
);

// -- render, playback, export, analysis ---------------------------------------------------

/// `RenderRange.start`.
const RENDER_START: Field = Field::new("start", POSITION).default_value(Constant::Float(0.0));

/// `RenderRange.end`.
const RENDER_END: Field = Field::new("end", Kind::Nullable(&POSITION))
    .default_value(Constant::Null)
    .description("Default: project end");

/// `RenderRange.tail`.
const RENDER_TAIL: Field = Field::new(
    "tail",
    Kind::Union(&[Kind::StrLiteral(&["auto", "none"]), Kind::FLOAT]),
)
.default_value(Constant::Str("auto"))
.description("Effect tail after the end: auto, none or seconds");

/// `RenderRange.track_id`.
const RENDER_TRACK_ID: Field = Field::new("track_id", Kind::Nullable(&Kind::STR))
    .default_value(Constant::Null)
    .description("Render one track/bus alone (through its output path)");

/// `RenderRange.preroll_seconds`.
const RENDER_PREROLL_SECONDS: Field = Field::new(
    "preroll_seconds",
    Kind::Float(NumberBounds::NONE.ge(Number::Int(0)).le(Number::Int(30))),
)
.default_value(Constant::Float(0.0))
.description("Warm up reverbs/delays before 'start'");

/// `RenderRange`: the render range nested in `analyze_audio.render`; its fields are shared
/// by the render and export inputs.
pub static RENDER_RANGE: Model = Model::tool_input(
    "RenderRange",
    &[
        RENDER_START,
        RENDER_END,
        RENDER_TAIL,
        RENDER_TRACK_ID,
        RENDER_PREROLL_SECONDS,
    ],
);

/// `RenderPreview`: the arguments of `render_preview`.
pub static RENDER_PREVIEW: Model = Model::tool_input(
    "RenderPreview",
    &[
        RENDER_START,
        RENDER_END,
        RENDER_TAIL,
        RENDER_TRACK_ID,
        RENDER_PREROLL_SECONDS,
        Field::new("path", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("WAV output path (default: project cache/previews)"),
        Field::new("per_track_levels", Kind::Bool)
            .default_value(Constant::Bool(false))
            .description("Also measure each track's loudness in isolation"),
        Field::new("wait", Kind::Bool)
            .default_value(Constant::Bool(true))
            .description("Wait for completion (up to timeout) and return the analysis"),
        Field::new(
            "timeout_seconds",
            Kind::Float(NumberBounds::NONE.gt(Number::Int(0)).le(Number::Int(3600))),
        )
        .default_value(Constant::Float(300.0)),
    ],
);

/// `PlayPreview`: the arguments of `play_preview`.
pub static PLAY_PREVIEW: Model = Model::tool_input(
    "PlayPreview",
    &[
        Field::new("start", Kind::Nullable(&POSITION))
            .default_value(Constant::Null)
            .description("Start position (default: current playhead)"),
        Field::new("end", Kind::Nullable(&POSITION))
            .default_value(Constant::Null)
            .description("Stop (or loop back) here"),
        Field::new("loop", Kind::Bool).default_value(Constant::Bool(false)),
    ],
);

/// `FormatSpec`: an output format of `export_audio` and `export_stems`.
pub static FORMAT_SPEC: Model = Model::plain(
    "FormatSpec",
    &[
        Field::new(
            "format",
            Kind::StrLiteral(&[
                "wav", "bwf", "rf64", "w64", "aiff", "caf", "flac", "mp3", "ogg", "opus",
            ]),
        )
        .default_value(Constant::Str("wav")),
        Field::new("bit_depth", Kind::IntLiteral(&[16, 24, 32]))
            .default_value(Constant::Int(24))
            .description("PCM bit depth (ignored for lossy formats)"),
        Field::new("floating_point", Kind::Bool)
            .default_value(Constant::Bool(false))
            .description("32-bit float (WAV/AIFF/CAF/W64/RF64)"),
        Field::new("sample_rate", Kind::Nullable(&Kind::INT))
            .default_value(Constant::Null)
            .description("Resample on export (default: project rate)"),
        Field::new("channels", Kind::IntLiteral(&[1, 2])).default_value(Constant::Int(2)),
        Field::new(
            "bitrate_kbps",
            Kind::Nullable(&Kind::Int(
                NumberBounds::NONE.ge(Number::Int(8)).le(Number::Int(512)),
            )),
        )
        .default_value(Constant::Null)
        .description("Lossy target bitrate (MP3 default 320 CBR)"),
        Field::new("variable_bitrate", Kind::Bool).default_value(Constant::Bool(false)),
        Field::new("dither", Kind::StrLiteral(&["tpdf", "none"]))
            .default_value(Constant::Str("tpdf")),
        Field::new("path", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Explicit file path; default derives from the main path/extension"),
    ],
);

/// `ExportAudio`: the arguments of `export_audio`.
pub static EXPORT_AUDIO: Model = Model::tool_input(
    "ExportAudio",
    &[
        RENDER_START,
        RENDER_END,
        RENDER_TAIL,
        RENDER_TRACK_ID,
        RENDER_PREROLL_SECONDS,
        DRY_RUN,
        CONFIRMATION_TOKEN,
        Field::new("path", Kind::STR)
            .description("Output file path (the extension may be replaced per format)"),
        Field::new(
            "formats",
            Kind::List(
                &Kind::Model(&FORMAT_SPEC),
                LengthBounds::NONE.min_length(1).max_length(8),
            ),
        )
        .default_factory(Factory::ModelList(&FORMAT_SPEC)),
        Field::new(
            "normalize",
            Kind::StrLiteral(&["none", "peak", "true_peak", "loudness"]),
        )
        .default_value(Constant::Str("none")),
        Field::new(
            "target_db",
            Kind::Float(NumberBounds::NONE.ge(Number::Int(-60)).le(Number::Int(0))),
        )
        .default_value(Constant::Float(-14.0))
        .description("Peak dBFS / dBTP or integrated LUFS target"),
        Field::new(
            "true_peak_ceiling_db",
            Kind::Float(NumberBounds::NONE.ge(Number::Int(-20)).le(Number::Int(0))),
        )
        .default_value(Constant::Float(-1.0)),
        Field::new("metadata", Kind::Dict(&Kind::STR)).default_factory(Factory::EmptyDict),
        Field::new("overwrite", Kind::Bool).default_value(Constant::Bool(false)),
        Field::new("wait", Kind::Bool).default_value(Constant::Bool(true)),
        Field::new(
            "timeout_seconds",
            Kind::Float(NumberBounds::NONE.gt(Number::Int(0)).le(Number::Int(7200))),
        )
        .default_value(Constant::Float(600.0)),
    ],
);

/// `ExportStems`: the arguments of `export_stems`.
pub static EXPORT_STEMS: Model = Model::tool_input(
    "ExportStems",
    &[
        RENDER_START,
        RENDER_END,
        RENDER_TAIL,
        RENDER_TRACK_ID,
        RENDER_PREROLL_SECONDS,
        DRY_RUN,
        CONFIRMATION_TOKEN,
        Field::new("directory", Kind::STR),
        Field::new(
            "track_ids",
            Kind::Nullable(&Kind::List(&Kind::STR, LengthBounds::NONE)),
        )
        .default_value(Constant::Null)
        .description("Default: all audio, instrument and bus tracks"),
        Field::new("format", Kind::Model(&FORMAT_SPEC))
            .default_factory(Factory::Model(&FORMAT_SPEC)),
        Field::new("through_master", Kind::Bool)
            .default_value(Constant::Bool(false))
            .description("Include master processing in each stem"),
        Field::new("overwrite", Kind::Bool).default_value(Constant::Bool(false)),
        Field::new("wait", Kind::Bool).default_value(Constant::Bool(true)),
        Field::new(
            "timeout_seconds",
            Kind::Float(NumberBounds::NONE.gt(Number::Int(0)).le(Number::Int(7200))),
        )
        .default_value(Constant::Float(1200.0)),
    ],
);

/// `ExportMidi`: the arguments of `export_midi`.
pub static EXPORT_MIDI: Model = Model::tool_input(
    "ExportMidi",
    &[
        DRY_RUN,
        CONFIRMATION_TOKEN,
        Field::new("path", Kind::STR),
        Field::new(
            "track_ids",
            Kind::Nullable(&Kind::List(&Kind::STR, LengthBounds::NONE)),
        )
        .default_value(Constant::Null),
        Field::new("overwrite", Kind::Bool).default_value(Constant::Bool(false)),
    ],
);

/// `AnalyzeAudio`: the arguments of `analyze_audio`.
pub static ANALYZE_AUDIO: Model = Model::tool_input(
    "AnalyzeAudio",
    &[
        Field::new("path", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Audio file to analyze"),
        Field::new("asset_id", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Project media asset to analyze"),
        Field::new("render", Kind::Nullable(&Kind::Model(&RENDER_RANGE)))
            .default_value(Constant::Null)
            .description("Render this part of the project and analyze it"),
        Field::new("per_track_levels", Kind::Bool).default_value(Constant::Bool(false)),
    ],
);

/// `GetAudioAnalysis`: the arguments of `get_audio_analysis`.
pub static GET_AUDIO_ANALYSIS: Model = Model::tool_input(
    "GetAudioAnalysis",
    &[
        Field::new("operation_id", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("Render/export operation whose analysis to return"),
        Field::new("path", Kind::Nullable(&Kind::STR))
            .default_value(Constant::Null)
            .description("A preview WAV rendered earlier (reads its .analysis.json)"),
        Field::new("include_series", Kind::Bool)
            .default_value(Constant::Bool(false))
            .description("Include the short-term loudness series (1 value/s)"),
    ],
);

/// `SetMidiInput`: the arguments of `set_midi_input`.
pub static SET_MIDI_INPUT: Model = Model::tool_input(
    "SetMidiInput",
    &[
        Field::new("enabled", Kind::Bool)
            .default_value(Constant::Bool(true))
            .description("Open (true) or close (false) MIDI inputs"),
        Field::new(
            "ports",
            Kind::Nullable(&Kind::List(&Kind::STR, LengthBounds::NONE)),
        )
        .default_value(Constant::Null)
        .description("Input port names; null = all available"),
    ],
);

/// `StartRecording`: the arguments of `start_recording`.
pub static START_RECORDING: Model = Model::tool_input(
    "StartRecording",
    &[
        Field::new("start", Kind::Nullable(&POSITION))
            .default_value(Constant::Null)
            .description("Where recording starts (default: current position)"),
        Field::new(
            "track_ids",
            Kind::Nullable(&Kind::List(&Kind::STR, LengthBounds::NONE)),
        )
        .default_value(Constant::Null)
        .description("Tracks to record (default: all armed tracks)"),
    ],
);

/// `OperationId`: the arguments of `get_operation_status`, `cancel_operation`.
pub static OPERATION_ID: Model = Model::tool_input(
    "OperationId",
    &[
        Field::new("operation_id", Kind::STR),
        Field::new(
            "wait_seconds",
            Kind::Float(NumberBounds::NONE.ge(Number::Int(0)).le(Number::Int(600))),
        )
        .default_value(Constant::Float(0.0))
        .description("Block up to this long for completion"),
    ],
);

/// `Relink`: the arguments of `relink_media`.
pub static RELINK: Model = Model::tool_input(
    "Relink",
    &[
        DRY_RUN,
        Field::new("asset_id", Kind::STR),
        Field::new("path", Kind::STR),
        Field::new("embed", Kind::Bool)
            .default_value(Constant::Bool(false))
            .description("Copy the file into the project instead of linking"),
    ],
);
