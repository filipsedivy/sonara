//! The agent tools: name, title, safety class and input model of each. The 57 tools of the
//! recorded contract come first, in the order of the prototype's `@tool(name, title, Model,
//! Safety)` decorators in `sonara.agent.tools.handlers`; the tools a later API version adds
//! (`super::versions`) follow them. Tool listings keep this order.

use super::inputs;
use super::model::Model;

/// How much a tool may change (ADR 0005, `docs/agent-api.md`, "Safety").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Safety {
    /// Reads state only.
    Read,
    /// Changes the project (undoable).
    Write,
    /// May discard work or overwrite files; needs confirmation.
    Destructive,
}

impl Safety {
    /// The name used in the contract: `"read"`, `"write"` or `"destructive"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Destructive => "destructive",
        }
    }
}

/// One tool of the registry: the prototype's `ToolSpec` without its handler.
#[derive(Debug)]
pub struct ToolSpec {
    /// The tool name, for example `create_project`.
    pub name: &'static str,
    /// The human-readable title.
    pub title: &'static str,
    /// The input model its arguments are validated against.
    pub input: &'static Model,
    /// The safety class.
    pub safety: Safety,
    /// The registry's `idempotent` flag. The MCP listing does not publish it: the prototype's
    /// server always reports `idempotentHint: false`.
    pub idempotent: bool,
    /// The handler's docstring, as written.
    docstring: &'static str,
}

impl ToolSpec {
    /// The description, built as the prototype's decorator builds it: the handler's docstring
    /// (or the title when there is none), stripped, with every run of whitespace replaced by
    /// one space (`" ".join(doc.split())`).
    #[must_use]
    pub fn description(&self) -> String {
        let doc = if self.docstring.trim().is_empty() {
            self.title
        } else {
            self.docstring
        };
        doc.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

/// The tool called `name`.
#[must_use]
pub fn find(name: &str) -> Option<&'static ToolSpec> {
    TOOLS.iter().find(|tool| tool.name == name)
}

/// Every tool: the prototype's, in its registration order, then the added ones.
pub static TOOLS: [ToolSpec; 58] = [
    ToolSpec {
        name: "create_project",
        title: "Create project",
        input: &inputs::CREATE_PROJECT,
        safety: Safety::Destructive,
        idempotent: false,
        docstring: "Create a new project (replacing the open one) with tempo, time signature, sample rate and
            an optional template ('electronic', 'band', 'orchestral'). Discarding unsaved changes requires
            confirmation. Returns the project state summary.",
    },
    ToolSpec {
        name: "open_project",
        title: "Open project",
        input: &inputs::OPEN_PROJECT,
        safety: Safety::Destructive,
        idempotent: false,
        docstring: "Open a .sonara project (bundle directory or archive). Reports recovered saves, missing
            media and available autosaves.",
    },
    ToolSpec {
        name: "save_project",
        title: "Save project",
        input: &inputs::SAVE_PROJECT,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Save the project in place, to a new path (Save As), or as a copy. Overwriting a
            different existing file or directory requires confirmation.",
    },
    ToolSpec {
        name: "get_project_state",
        title: "Get project state",
        input: &inputs::GET_PROJECT_STATE,
        safety: Safety::Read,
        idempotent: true,
        docstring: "The actual project: tempo, meter, sections, tracks (mixer, instrument, effects, sends,
            clips), master and media. Positions are given as beats, seconds and bar:beat.",
    },
    ToolSpec {
        name: "list_tracks",
        title: "List tracks",
        input: &inputs::EMPTY,
        safety: Safety::Read,
        idempotent: true,
        docstring: "Tracks in display order with kind, mixer state, instrument and clip counts.",
    },
    ToolSpec {
        name: "set_tempo",
        title: "Set tempo",
        input: &inputs::SET_TEMPO,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Set the song tempo, or add a tempo change (optionally a ramp) at a position.",
    },
    ToolSpec {
        name: "set_time_signature",
        title: "Set time signature",
        input: &inputs::SET_TIME_SIGNATURE,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Set the time signature from a 1-based bar onward.",
    },
    ToolSpec {
        name: "set_project_length",
        title: "Set project length",
        input: &inputs::SET_PROJECT_LENGTH,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Set the song length (render/export end). null = end of the last clip.",
    },
    ToolSpec {
        name: "add_section",
        title: "Add arrangement section",
        input: &inputs::ADD_SECTION,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Add a named song section (intro, verse, drop...) spanning a range.",
    },
    ToolSpec {
        name: "add_marker",
        title: "Add marker",
        input: &inputs::ADD_MARKER,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Add a named timeline marker.",
    },
    ToolSpec {
        name: "set_metadata",
        title: "Set project metadata",
        input: &inputs::SET_METADATA,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Title, artist, album, genre, composer, comment (written into exports).",
    },
    ToolSpec {
        name: "set_loop",
        title: "Set loop region",
        input: &inputs::SET_LOOP,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Enable/disable and position the transport loop region.",
    },
    ToolSpec {
        name: "create_track",
        title: "Create track",
        input: &inputs::CREATE_TRACK,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Create an audio, instrument, bus or folder track. Instrument tracks take an instrument
            plugin_id and preset (see list_plugins); default instrument is the Poly Synth.",
    },
    ToolSpec {
        name: "rename_track",
        title: "Rename track",
        input: &inputs::RENAME_TRACK,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Rename a track (its ID stays the same).",
    },
    ToolSpec {
        name: "delete_track",
        title: "Delete track",
        input: &inputs::DELETE_TRACK,
        safety: Safety::Destructive,
        idempotent: false,
        docstring: "Delete a track. References are repaired (outputs to a deleted bus go to the master,
            sends are removed). Tracks with clips or devices require confirmation. Undoable.",
    },
    ToolSpec {
        name: "set_track_volume",
        title: "Set track volume",
        input: &inputs::SET_TRACK_VOLUME,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Set a track's (or the master's) fader level in dB.",
    },
    ToolSpec {
        name: "set_track_pan",
        title: "Set track pan",
        input: &inputs::SET_TRACK_PAN,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Set a track's stereo pan/balance (-1 left .. +1 right).",
    },
    ToolSpec {
        name: "set_track_state",
        title: "Set mute/solo/arm/color",
        input: &inputs::SET_TRACK_STATE,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Mute, solo, solo-safe, record-arm or color a track.",
    },
    ToolSpec {
        name: "set_instrument",
        title: "Set instrument",
        input: &inputs::SET_INSTRUMENT,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Replace an instrument track's instrument (plugin_id, preset, parameters).",
    },
    ToolSpec {
        name: "configure_routing",
        title: "Configure routing",
        input: &inputs::CONFIGURE_ROUTING,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Change a track's output (master or a bus), add/update/remove aux sends, and set a
            device's sidechain source. Feedback loops are rejected (ROUTING_CYCLE).",
    },
    ToolSpec {
        name: "create_midi_clip",
        title: "Create MIDI clip",
        input: &inputs::CREATE_MIDI_CLIP,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Create a MIDI clip on an instrument track, optionally with notes (note starts are beats
            relative to the clip start).",
    },
    ToolSpec {
        name: "add_midi_notes",
        title: "Add MIDI notes",
        input: &inputs::ADD_MIDI_NOTES,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Add notes to a MIDI clip. Returns the new note IDs (use them with edit_midi_notes).",
    },
    ToolSpec {
        name: "edit_midi_notes",
        title: "Edit MIDI notes",
        input: &inputs::EDIT_MIDI_NOTES,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Edit, delete, transpose, quantize, humanize or rescale notes in a MIDI clip, or replace
            them all. All changes are applied as one undoable step.",
    },
    ToolSpec {
        name: "generate_midi",
        title: "Generate MIDI",
        input: &inputs::GENERATE_MIDI,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Generate musical material into a new (or existing) MIDI clip: voice-led chords,
            arpeggios, basslines, drum grooves (GM map) or a melody, from a key and a progression of
            Roman numerals or chord symbols. Deterministic for a given seed. The notes are ordinary,
            editable MIDI.",
    },
    ToolSpec {
        name: "import_audio",
        title: "Import audio",
        input: &inputs::IMPORT_AUDIO,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Import an audio file (WAV, AIFF, FLAC, MP3, Ogg Vorbis/Opus; more with FFmpeg) onto a
            new or existing audio track. The original is preserved; 'copy' embeds it in the project.",
    },
    ToolSpec {
        name: "import_midi",
        title: "Import MIDI file",
        input: &inputs::IMPORT_MIDI,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Import a Standard MIDI File: each MIDI track/channel becomes an instrument track
            (channel 10 uses the drum kit). Optionally adopts the file's tempo map.",
    },
    ToolSpec {
        name: "move_clip",
        title: "Move clip",
        input: &inputs::MOVE_CLIP,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Move a clip in time and/or to another compatible track.",
    },
    ToolSpec {
        name: "edit_clip",
        title: "Edit clip",
        input: &inputs::EDIT_CLIP,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Rename, mute, set gain and fades (audio), trim edges (non-destructive) or set a loop
            length (MIDI).",
    },
    ToolSpec {
        name: "split_clip",
        title: "Split clip",
        input: &inputs::SPLIT_CLIP,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Split a clip at a position into two clips.",
    },
    ToolSpec {
        name: "duplicate_clip",
        title: "Duplicate clip",
        input: &inputs::DUPLICATE_CLIP,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Duplicate a clip one or more times, back to back (e.g. repeat a 4-bar loop).",
    },
    ToolSpec {
        name: "delete_clips",
        title: "Delete clips",
        input: &inputs::DELETE_CLIPS,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Delete clips (undoable; media stays in the project).",
    },
    ToolSpec {
        name: "list_plugins",
        title: "List instruments and effects",
        input: &inputs::LIST_PLUGINS,
        safety: Safety::Read,
        idempotent: true,
        docstring: "Available instruments and effects with presets; pass plugin_id for the full parameter
            schema (ranges, units, defaults).",
    },
    ToolSpec {
        name: "add_effect",
        title: "Add effect",
        input: &inputs::ADD_EFFECT,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Insert an effect on a track, bus or the master, optionally from a preset and with a
            sidechain source (e.g. a compressor on the bass keyed by the kick track).",
    },
    ToolSpec {
        name: "set_effect_parameter",
        title: "Set effect parameters",
        input: &inputs::SET_EFFECT_PARAMETER,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Set one or more parameters of an effect or instrument, load a preset, or bypass it.
            Values are validated against the plugin's parameter schema.",
    },
    ToolSpec {
        name: "remove_effect",
        title: "Remove effect",
        input: &inputs::REMOVE_EFFECT,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Remove an effect from its chain (undoable).",
    },
    ToolSpec {
        name: "load_impulse_response",
        title: "Load impulse response",
        input: &inputs::LOAD_IMPULSE_RESPONSE,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Load an impulse-response audio file into a Convolution Reverb device.",
    },
    ToolSpec {
        name: "create_automation",
        title: "Create automation",
        input: &inputs::CREATE_AUTOMATION,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Write automation for volume, pan, a send level or any automatable device parameter.
            Points are (position, value, curve); values use the parameter's units.",
    },
    ToolSpec {
        name: "render_preview",
        title: "Render preview",
        input: &inputs::RENDER_PREVIEW,
        safety: Safety::Read,
        idempotent: false,
        docstring: "Render the project (or a range / one track) offline to a WAV file and analyze it:
            loudness (LUFS), true peak, clipping, stereo correlation, spectral balance and detected
            issues. Use this to judge the actual sound before revising.",
    },
    ToolSpec {
        name: "play_preview",
        title: "Play through audio device",
        input: &inputs::PLAY_PREVIEW,
        safety: Safety::Read,
        idempotent: false,
        docstring: "Start real-time playback on the audio output device from a position (optionally a
            looped range). Fails with ENGINE_UNAVAILABLE when no audio device/engine is available.",
    },
    ToolSpec {
        name: "start_recording",
        title: "Start recording",
        input: &inputs::START_RECORDING,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Record armed tracks (audio from the input device, MIDI from open MIDI inputs) from a
            position until stop_playback, which returns the takes. Requires the audio engine.",
    },
    ToolSpec {
        name: "list_midi_devices",
        title: "List MIDI devices",
        input: &inputs::EMPTY,
        safety: Safety::Read,
        idempotent: true,
        docstring: "Available MIDI input/output ports and the live-input status (open ports, target
            tracks: armed instrument tracks plus the GUI-focused one).",
    },
    ToolSpec {
        name: "set_midi_input",
        title: "Set MIDI input",
        input: &inputs::SET_MIDI_INPUT,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Open or close MIDI inputs for live playing and recording into armed instrument tracks.",
    },
    ToolSpec {
        name: "stop_playback",
        title: "Stop playback",
        input: &inputs::EMPTY,
        safety: Safety::Read,
        idempotent: true,
        docstring: "Stop real-time playback (or recording).",
    },
    ToolSpec {
        name: "export_audio",
        title: "Export audio",
        input: &inputs::EXPORT_AUDIO,
        safety: Safety::Destructive,
        idempotent: false,
        docstring: "Render and export the mix (or a range / one track) to one or more formats at once:
            WAV/BWF/AIFF/FLAC (16/24/32-bit, float), MP3, Ogg Vorbis, Opus. Supports dither,
            resampling, peak or loudness normalization (never exceeding the true-peak ceiling) and
            metadata. Overwriting existing files requires overwrite=true and confirmation.",
    },
    ToolSpec {
        name: "export_stems",
        title: "Export stems",
        input: &inputs::EXPORT_STEMS,
        safety: Safety::Destructive,
        idempotent: false,
        docstring: "Export each track/bus as its own file (isolated through its output path; master
            processing optional).",
    },
    ToolSpec {
        name: "export_midi",
        title: "Export MIDI file",
        input: &inputs::EXPORT_MIDI,
        safety: Safety::Destructive,
        idempotent: false,
        docstring: "Export instrument tracks as a Standard MIDI File (Type 1, 960 PPQ) with tempo map,
            time signatures and markers.",
    },
    ToolSpec {
        name: "analyze_audio",
        title: "Analyze audio",
        input: &inputs::ANALYZE_AUDIO,
        safety: Safety::Read,
        idempotent: false,
        docstring: "Measure an audio file, a project media asset, or a fresh render of the project:
            LUFS (integrated/short-term/momentary), LRA, sample and true peak, RMS, crest factor,
            clipping, silence, stereo correlation/width, octave-band balance, plus issues found.",
    },
    ToolSpec {
        name: "get_audio_analysis",
        title: "Get audio analysis",
        input: &inputs::GET_AUDIO_ANALYSIS,
        safety: Safety::Read,
        idempotent: true,
        docstring: "Return the analysis of a finished render_preview/export operation, or of a preview
            file rendered earlier.",
    },
    ToolSpec {
        name: "get_operation_status",
        title: "Get operation status",
        input: &inputs::OPERATION_ID,
        safety: Safety::Read,
        idempotent: true,
        docstring: "Status, progress, result or error of a background operation (render, export, decode).",
    },
    ToolSpec {
        name: "cancel_operation",
        title: "Cancel operation",
        input: &inputs::OPERATION_ID,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Request cancellation of a running background operation.",
    },
    ToolSpec {
        name: "list_operations",
        title: "List operations",
        input: &inputs::EMPTY,
        safety: Safety::Read,
        idempotent: true,
        docstring: "Recent background operations and their status.",
    },
    ToolSpec {
        name: "undo",
        title: "Undo",
        input: &inputs::EMPTY,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Undo the last edit (shared history with the GUI).",
    },
    ToolSpec {
        name: "redo",
        title: "Redo",
        input: &inputs::EMPTY,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Redo the last undone edit.",
    },
    ToolSpec {
        name: "get_history",
        title: "Get edit history",
        input: &inputs::EMPTY,
        safety: Safety::Read,
        idempotent: true,
        docstring: "Undo/redo stacks (labels, newest last) and recent agent actions.",
    },
    ToolSpec {
        name: "verify_project",
        title: "Verify project integrity",
        input: &inputs::EMPTY,
        safety: Safety::Read,
        idempotent: true,
        docstring: "Check media presence, sizes and hashes, and the routing graph. ``intact`` is true when
            nothing is missing, mismatched or misrouted.",
    },
    ToolSpec {
        name: "relink_media",
        title: "Relink media",
        input: &inputs::RELINK,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Point a missing or moved media asset at a file (optionally copying it into the project).",
    },
    ToolSpec {
        name: "get_capabilities",
        title: "Get capabilities",
        input: &inputs::EMPTY,
        safety: Safety::Read,
        idempotent: true,
        docstring: "What this Sonara build can do: API version, export formats, audio engine status,
            allowed file locations and limits.",
    },
    // Added in agent API 1.1.
    ToolSpec {
        name: "write_midi_controllers",
        title: "Write MIDI controllers",
        input: &inputs::WRITE_MIDI_CONTROLLERS,
        safety: Safety::Write,
        idempotent: false,
        docstring: "Write the events of one MIDI controller (CC) into a MIDI clip: for example
            expression (CC 11) or modulation (CC 1) curves that shape a part's dynamics, or the
            sustain pedal (CC 64). The new events replace every event of that controller in the
            clip, as one undoable step; other controllers and the notes stay. Event beats are
            relative to the clip start, and a value holds until the controller's next event.",
    },
];
