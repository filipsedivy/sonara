# Agent-driven examples

Each `*.json` file is a reproducible agent workflow: an ordered list of Sonara tool calls,
exactly the calls an agent makes over MCP. Run them from the repository root, with the `sonara`
CLI from the app bundle or `target/release/sonara`:

```sh
sonara batch examples/electronic_124bpm.json --connect never
sonara batch examples/sidechain_preview.json --connect never --results /tmp/sidechain.json
```

- `--connect never` runs the steps in a fresh in-process session. With `--connect auto` (the
  default) the steps run inside a running Sonara (the GUI or `sonara serve`), so you can watch
  the project being built.
- `--results FILE` writes every step's full result (state, analysis, export reports) as JSON.
- All outputs go to `examples/output/` (git-ignored). Re-running overwrites them, because the
  overwriting steps carry `"confirm": true` (see below).

## Batch file format

```json
{"steps": [
  {"id": "bass", "tool": "create_track", "args": {"name": "Bass", "preset": "Analog Bass"}},
  {"tool": "generate_midi", "args": {"track_id": "{{bass.result.track_id}}", "kind": "bass", "key": "A minor",
                                     "progression": ["i", "VI", "III", "VII"]}},
  {"tool": "export_audio", "confirm": true, "args": {"path": "examples/output/x.wav", "overwrite": true}}
]}
```

- `{{id.path}}` inserts a value from an earlier step's result, for example
  `{{project.state.tracks[0].track_id}}`. It works both as a whole value and inside strings.
- `"confirm": true` means the author of the file pre-approves that step. When it returns
  `CONFIRMATION_REQUIRED` (for example, overwriting files), the runner repeats it once with
  the issued token. Steps without it stop at the confirmation, as an interactive agent must.
- `"allow_error": true` lets the batch continue past that step's failure. `--keep-going` does
  this for every step.

## The examples

| File | Request it answers | What it builds |
|------|--------------------|----------------|
| `electronic_124bpm.json` | "Create a four-minute electronic track at 124 BPM with an intro, a build-up, two drops, a breakdown, and an outro." | 120 bars (3:55) in A minor with six sections. Drums, bass, chords and lead are generated per section. The bass is sidechained to the drums. Reverb and delay buses, a filter sweep through the build-up, a master fade and a limiter. Previews drop 1 with per-track levels, then exports WAV, FLAC and MP3 at −14 LUFS, plus MIDI and the project. |
| `cinematic_orchestral.json` | "Create a cinematic orchestral arrangement with strings, brass, percussion, and evolving dynamics." | The `orchestral` template plus a horn line in D minor at 72 BPM. Volume-automation crescendos into the climax, velocity-scaled rising percussion, a choir entry, and a tempo ramp (ritardando) into the final chord. Exports at −18 LUFS. |
| `sidechain_preview.json` | "Add a sidechain compressor to the bass, triggered by the kick, and export a preview." | A kick and bass groove. Measures the bass alone, adds a compressor keyed by the kick, measures again, and exports an 8-bar preview as WAV and MP3. |
| `mix_analysis.json` | "Analyze the mix for clipping, excessive peaks, and loudness issues, then propose corrections." | A deliberately hot mix that is analyzed (issues: `OVER_FULL_SCALE`, `CLIPPING`, `TRUE_PEAK`, `VERY_LOUD`). It then applies the corrections an agent derives from the per-track levels (faders down, bass EQ, master limiter), re-analyzes, and exports at −14 LUFS. |

The runs are deterministic: the same files and seeds produce the same audio, and the golden
transcripts in `golden/` pin these results.

| Example | Result |
|---------|--------|
| electronic_124bpm | 234.6 s mix: −14.00 LUFS integrated, −2.3 dBTP. Drop-1 per-track levels: drums −18.7, bass −21.2, chords −23.7, lead −22.1 LUFS. |
| cinematic_orchestral | 121.8 s cue: −15.2 LUFS, loudness range 20.1 LU (evolving dynamics). Exported at −18.0 LUFS, −3.8 dBTP. Most of the render time goes to the 7 s Cathedral reverb (long convolution). |
| sidechain_preview | Bass alone −10.98 → −18.72 LUFS after ducking (crest factor 7.2 → 9.8 dB). Preview −13.6 LUFS, −3.2 dBTP. |
| mix_analysis | Hot mix +11.5 dBTP, −3.9 LUFS, 2928 clip events → corrected −14.1 LUFS, −0.97 dBTP, no clipping. |

Per-track levels of *buses* currently include the dry signal of their sources (a known
limitation of bus isolation), so compare buses with care.

A batch is a script, so it cannot react to the measurements. An agent does: it reads the
`render_preview` / `analyze_audio` results (loudness, peaks, per-track levels, `issues`) and
decides the next edit. The corrections in `mix_analysis.json` are the ones that analysis calls
for.

## The same workflows through Claude Code (MCP)

Register the server once. Relative paths resolve against the current directory, or against
`--base-dir DIR`.

```sh
claude mcp add sonara -- /Applications/Sonara.app/Contents/MacOS/sonara mcp
```

Then ask in plain language, for example: "Create a four-minute electronic track at 124 BPM
with an intro, a build-up, two drops, a breakdown and an outro, then render a preview and
tell me how loud it is." Claude calls the same tools as the batch files (`create_project`,
`add_section`, `create_track`, `generate_midi`, `add_effect`, `configure_routing`,
`create_automation`, `render_preview`, `export_audio`, `save_project`, ...). Each step's
`args` object is exactly the MCP tool input.

If the Sonara GUI (or `sonara serve`) is running, `sonara mcp` connects to it and the edits
appear live in the open project. Otherwise it hosts its own headless session. File access is
limited to the working directory, the Sonara projects folder and paths you allow
(`--allow PATH`, or `SONARA_AGENT_ALLOWED_PATHS`).
