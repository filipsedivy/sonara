# The `.sonara` project format

- **Container format version:** 1 (`manifest.format_version`)
- **Document schema version:** 1 (`project.schema_version`)

Machine-readable schemas ship with the package:

- `crates/sonara-project/schemas/project-v1.schema.json`
- `crates/sonara-project/schemas/manifest-v1.schema.json`

Both are JSON Schema draft 2020-12.

## Two layouts, one structure

A `.sonara` path is either:

- a **bundle**, a directory and the working form. Saves rewrite only `project.json` and
  `manifest.json`. Media files are never rewritten.
- an **archive**, a ZIP file and the packaged form for transport. Opening extracts it into a
  managed workspace. Saving writes a new archive atomically. JSON members are deflated;
  media members are stored uncompressed.

Sonara detects the layout automatically. Both contain:

```text
manifest.json        container metadata, integrity, required features
project.json         the project document
media/audio/         imported audio (original bytes) and recordings
media/audio/originals/  original files kept when FFmpeg transcoded an import
media/impulses/      impulse responses
media/midi/          imported MIDI files (provenance)
presets/             device presets saved with the project
thumbnails/          optional preview images
analysis/            regenerable analysis results
```

Bundles also contain members that are never packaged:

- `backups/`: previous versions of `project.json`, rotated, 10 by default.
- `.autosave/`: crash-recovery copy plus `info.json`.
- `cache/`: decoded media, waveform peaks, previews and export intermediates. All of it is
  regenerable.

## `manifest.json`

```json
{
  "format": "sonara.manifest",
  "format_version": 1,
  "sonara_version": "0.1.0",
  "project_id": "prj_k3v9...",
  "saved": "2026-10-20T13:56:57Z",
  "required_features": ["core", "midi-clips", "sends", "sidechain"],
  "project_file": {"path": "project.json", "sha256": "…", "size": 46898},
  "assets": [{"id": "med_…", "path": "media/audio/med_…-kick.wav", "location": "embedded",
              "size": 211234, "sha256": "…", "kind": "audio"}],
  "compatibility": {"min_reader_version": 1}
}
```

`required_features` lists capabilities the project relies on. A reader that does not know a
listed feature refuses to open the project and returns `UNSUPPORTED` with the feature names.
This lets older Sonara versions reject projects they cannot represent instead of silently
dropping data.

Known features: `core`, `audio-clips`, `midi-clips`, `automation`, `sends`, `sidechain`,
`tempo-ramps`, `linked-media`, `sections`, `folders`.

## `project.json`

UTF-8 JSON with stable key order and float values written with `repr` precision.
Serialization is deterministic: the same project always produces the same bytes. Top-level
fields:

| Field | Meaning |
|---|---|
| `format`, `schema_version` | `"sonara.project"`, `1` |
| `id`, `name` | Stable project ID and display name |
| `settings` | `sample_rate`, `length_beats` (`null` means the end of the content), `record_subtype` |
| `tempo_map` | `[{beat, bpm, ramp}]`; the first point is at beat 0. A `ramp` point glides linearly to the next point's tempo. |
| `time_signatures` | `[{bar, numerator, denominator}]`, with 0-based bars |
| `tracks` | Track objects in display order |
| `master` | The master track (`id: "master"`) |
| `assets` | Media records |
| `markers`, `sections` | Timeline markers and arrangement sections |
| `transport` | Loop and punch ranges, metronome, count-in |
| `metadata` | Title, artist, album, genre, composer, comment, created, modified |
| `editor` | GUI view state (zoom, snap); not undoable |

**Units.** Positions and lengths are quarter-note **beats** (float64, see
[ADR 0003](decisions/0003-musical-time-base.md)). Note positions are relative to their
clip's content start. Audio `source_offset` is in frames at the source's own sample rate.
Fades are in seconds. Levels are in dB, and pan runs from −1 to +1.

**Tracks.** Each track has `id`, `name`, `kind` (audio, instrument, bus, folder or master),
`color`, `output_id`, `parent_id`, `mixer` (`volume_db`, `pan`, `mute`, `solo`,
`solo_safe`), `input` (`channels`, `armed`, `monitor`), `instrument` (a device or `null`),
`inserts`, `sends` (`id`, `target_id`, `level_db`, `pre_fader`, `enabled`), `automation`
and `clips`.

**Clips.**

- `"type": "audio"`: `asset_id`, `start`, `length`, `source_offset`, `gain_db`, `fade_in`,
  `fade_out` (`{seconds, curve}`), `muted`, `color`.
- `"type": "midi"`: `start`, `length`, `content_offset`, `loop_length`, `notes`
  (`{id, pitch, start, length, velocity, channel}`), and `controls`
  (`{beat, kind: cc|pitch_bend|channel_pressure, number, value, channel}`).

**Devices.** `id`, `plugin_id`, `name`, `enabled`, `params` (values only; ranges and defaults
come from the plugin's descriptor), `sidechain_id`, `preset`, and `state` (non-parameter
state, such as an impulse response's `ir_asset_id`).

**Automation lanes.** `id`, `target` (`device_id`, which is `null` for mixer parameters, and
`param`), `enabled`, and `points` (`{beat, value, curve: linear|hold|smooth}`). Mixer
parameters are `volume_db`, `pan` and `send.<send_id>.level_db`.

**Assets.** `id`, `kind` (audio, impulse or midi), `name`, `path`, `location` (embedded or
linked), `size_bytes`, `sha256`, `sample_rate`, `channels`, `frames`, `format`, `subtype`,
`original_path`, `relative_hint` and `metadata`.

- Embedded paths are relative POSIX paths under `media/`.
- Linked paths are absolute.

**Forward compatibility.** Unknown keys on project, track, clip, device and asset objects
are preserved on round-trip through each object's `extra` map. Unknown keys inside small
value objects (`mixer`, `settings` and similar) are not preserved.

## Saving

1. Serialize the project and check that every embedded media file exists in the workspace.
2. **Bundle.** Hard-link or copy media into the target bundle when saving to a new location,
   including media reachable only through undo history. Copy the previous `project.json`
   into `backups/`. Write `project.json` (temp file, fsync, rename), then `manifest.json` the
   same way, then fsync the directory. Remove `.autosave/`.
3. **Archive.** Write a temporary ZIP next to the target, fsync it and rename it over the
   target.

## Opening and recovery

1. Validate `manifest.json`. An unreadable manifest produces a warning, and Sonara continues
   without it.
2. Check `format_version` and `required_features`.
3. Read `project.json` and compare its SHA-256 with the manifest.
   - **Hash mismatch, but the document is valid.** The previous save was interrupted between
     the two renames. The newer `project.json` is used and the manifest is rewritten on the
     next save.
   - **`project.json` damaged or missing.** The newest valid file in `backups/` is used, and
     the result reports `recovered_from`.
4. Run migrations, then validate against the JSON Schema, then deserialize.
5. Report missing media (rendered as silence until relinked) and offer a newer autosave.
   Unsaved projects live in scratch workspaces whose autosaves are offered for recovery at
   the next start.

## Archive safety

Extraction rejects all of the following:

- absolute paths, `..` components, backslashes and drive letters
- symlink members
- top-level members other than the documented ones
- more than 100,000 members, or more than 64 GB in total
- members whose decompressed size exceeds their declared size
- suspicious compression ratios (above 200:1 on members over 64 MB)
- archives larger than the available disk space

## Versioning and migrations

`project.schema_version` versions the document. Migrations live in
`crates/sonara-project/src/migrations.rs` and are pure functions that upgrade a document by one
version. They run before validation. A document from a newer schema is rejected with
`UNSUPPORTED`, and is never treated as damaged. Container changes bump `format_version`.

Schema version 1 is the only version so far, so the migration table is empty. A missing or
non-integer `schema_version` is a `FORMAT_ERROR`.

## Fixtures and contract tests

`crates/sonara-project/tests/fixtures/` holds two canonical documents that other crates can
load: `rich.project.json` (every feature, recorded from the prototype) and `song.project.json`
(a three-track song with a tempo change, a meter change, two sections and overlapping notes).
`crates/sonara-project/tests/song_project_contract.rs` checks the song fixture against the
schema, the model and the routing rules, its byte-exact round trip, an empty project, and the
rejection of out-of-range or inconsistent values.
