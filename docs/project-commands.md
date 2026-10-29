# Project commands and history (SON-P09)

Every change to a project is a `Command` run by `ProjectSession::execute` (`sonara-core`). The UI,
the agent tools and the CLI have no other way to write the project (CTR-01 keeps the agent tools'
schemas; the commands behind them are listed here).

## Guarantees

Pinned by `crates/sonara-core/tests/commands.rs` and `tests/history.rs`; the long form is in
[architecture.md](architecture.md#command-and-history-guarantees).

- **Atomic.** A command that fails leaves the project, its version and both undo stacks as they
  were.
- **No-ops** record no history step.
- **One step per command or transaction;** `undo` and `redo` restore the exact snapshots, and
  note IDs survive delete and undo.
- **Auditable.** A committed command returns the `ChangeSet` it made (created, updated and
  deleted objects and flags), which the agent tools report.

## The commands

`crates/sonara-core/tests/command_catalog.rs` fails when a command is added or removed without
this list changing.

| Module | Command | What it does |
|---|---|---|
| `automation` | `SetAutomation` | Creates the lane for `(device_id, param)` on a track or replaces its points; with `start`/`end`, only the points inside `[start, end]` are replaced |
| `automation` | `SetLaneEnabled` | Enables or disables a lane |
| `automation` | `DeleteAutomationLane` | Deletes a lane |
| `clip` | `AddAudioClip` | Adds an audio clip playing an asset |
| `clip` | `CreateMidiClip` | Creates a MIDI clip, optionally with notes |
| `clip` | `MoveClip` | Moves a clip in time and/or to another track |
| `clip` | `MoveClips` | Moves several clips by the same beat delta (multi-selection drag) |
| `clip` | `TrimClip` | Moves a clip's start and/or end edge without moving its content in time |
| `clip` | `SplitClip` | Splits a clip in two at a timeline beat |
| `clip` | `DuplicateClip` | Duplicates a clip, by default right after itself on the same track |
| `clip` | `DeleteClips` | Deletes clips |
| `clip` | `SetClipProperties` | Changes clip properties; `None` keeps a value |
| `clip` | `AddNotes` | Adds notes to a MIDI clip |
| `clip` | `EditNotes` | Changes notes of a MIDI clip |
| `clip` | `DeleteNotes` | Deletes notes from a MIDI clip |
| `clip` | `ReplaceNotes` | Replaces a clip's entire note content (quantize, transpose, generators) |
| `clip` | `SetControlEvents` | Replaces the controller events of the given kinds and numbers in `[start, end)` |
| `device` | `AddDevice` | Inserts an effect into a track's chain (`index` `None`: the end) |
| `device` | `SetInstrument` | Sets (or replaces) the instrument of an instrument track |
| `device` | `RemoveDevice` | Removes an instrument or effect and its automation |
| `device` | `MoveDevice` | Moves an effect within its track's insert chain |
| `device` | `SetDeviceParams` | Sets device parameters (validated against the plugin) |
| `device` | `SetDeviceEnabled` | Enables or bypasses a device |
| `device` | `ApplyPreset` | Loads a preset: defaults overlaid with the preset's values |
| `device` | `SetSidechain` | Connects (or, with `None`, disconnects) a device's sidechain input |
| `device` | `SetDeviceState` | Sets non-parameter device state (for example a convolver's impulse response); `null` removes a key |
| `mod` | `Transaction` | Several commands applied atomically as one undo step |
| `project` | `RenameProject` | Renames the project (surrounding whitespace removed) |
| `project` | `SetTempo` | Sets the tempo at `beat` (0 = the song tempo) |
| `project` | `RemoveTempoPoint` | Removes the tempo change at `beat` |
| `project` | `SetTimeSignature` | Sets the time signature from 0-based `bar` on |
| `project` | `RemoveTimeSignature` | Removes the time signature change at 0-based `bar` |
| `project` | `SetProjectLength` | Sets (or, with `None`, clears) the explicit song length |
| `project` | `SetSampleRate` | Changes the project sample rate |
| `project` | `SetMetadata` | Sets song metadata fields (title, artist, …) |
| `project` | `SetTransportSettings` | Changes loop, punch and metronome settings |
| `project` | `AddMarker` | Adds a marker |
| `project` | `UpdateMarker` | Changes a marker; `None` keeps a value |
| `project` | `RemoveMarker` | Removes a marker |
| `project` | `AddSection` | Adds an arrangement section |
| `project` | `UpdateSection` | Changes a section; `None` keeps a value |
| `project` | `RemoveSection` | Removes a section |
| `project` | `AddAsset` | Adds a media asset record |
| `project` | `ReplaceAsset` | Updates an asset record in place (relinking, re-hashing) |
| `project` | `RemoveAsset` | Removes an unused media asset record |
| `project` | `SetEditorState` | Persisted editor settings (zoom, snap, track heights) |
| `track` | `CreateTrack` | Creates a track, optionally with an instrument |
| `track` | `RenameTrack` | Renames a track (surrounding whitespace removed) |
| `track` | `SetTrackColor` | Sets or clears a track's color |
| `track` | `DeleteTrack` | Deletes a track and repairs every reference to it: outputs to a deleted bus go to the master, sends to it are removed, sidechains from it are cleared and folder children move up |
| `track` | `MoveTrack` | Moves a track to `index` (clamped) |
| `track` | `SetTrackMixer` | Changes mixer settings; `None` keeps a value |
| `track` | `SetTrackOutput` | Routes a track's output to a bus or the master |
| `track` | `AddSend` | Adds a send from a track to a bus |
| `track` | `UpdateSend` | Changes a send; `None` keeps a value |
| `track` | `RemoveSend` | Removes a send and its automation |
| `track` | `SetTrackParent` | Moves a track into a folder (or, with `None`, to the top level) |
| `track` | `SetTrackInput` | Changes a track's recording input; `None` keeps a value |
