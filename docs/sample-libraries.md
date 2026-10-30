# Sample libraries and the sampler

`sonara.sampler` plays SFZ instruments from sample libraries that you install yourself. Sonara
ships no samples (ADR 0010).

## Install a library

1. Download a library whose licence suits your use. Checked sources and their licences are in
   [orchestral-audio-engine-research.md](orchestral-audio-engine-research.md), section 5.
   For a start: **VSCO 2 Community Edition** (CC0), branch `SFZ` of
   <https://github.com/sgossner/VSCO-2-CE>. Clone or download that branch so the `.sfz` files
   and their `Samples` folders stay together.
2. Add the library's directory to the settings file (`settings.json` in Sonara's config
   directory; unknown keys are kept):

   ```json
   { "sample_libraries": ["/Users/you/Music/VSCO-2-CE"] }
   ```

3. Restart Sonara. The directories are scanned for `.sfz` files at startup (eight levels deep,
   hidden folders and symbolic links skipped, at most 5,000 patches).

To replace or add a library, edit the list and restart. Projects store the patch's absolute path;
on a machine without it the track plays silence and the render warns
`instrument … is not installed`.

## Try an instrument

- **Agent or CLI:** `list_plugins` with `plugin_id: "sonara.sampler"` shows the installed patches
  as the choices of `instrument`. Then `create_track` (`kind: instrument`,
  `instrument: "sonara.sampler"`) and `set_instrument` with
  `params: {"instrument": "<path of the .sfz>"}`, add a MIDI clip, and `render`.
- **GUI:** the project view builds its plugin catalog from the same registry, so Sonara Sampler
  should appear among the instruments and its `instrument` choice in the device editor. **This
  was not exercised in the GUI** (no GUI session was run); if it does not show up, use the agent
  route above.

If `sonara.sampler` is not listed, no configured directory holds a `.sfz` file.

## What the sampler plays

Velocity layers, round robin (`seq_length`/`seq_position`, `lorand`/`hirand`), keyswitches
(`sw_*`), release samples (`trigger=release`), exclusive groups (`group`/`off_by`), loops,
attack and release times, tuning and volume. MIDI: notes, sustain pedal (CC 64), volume (CC 7),
expression (CC 11), pitch bend (±2 semitones) and all notes off. Parameters: volume, pan,
release scale, instrument.

Not supported: CC-driven layers and crossfades (for example Sonatina's dynamics on CC 1 and
vibrato on CC 21), filters, per-region pan, streaming. The loader reports unsupported opcodes in
the load warning when a patch is rendered, together with the values the sampler does not play
(`trigger=first`, `legato` or `release_key`, which play on every key press for now, and unknown
`loop_mode` values, which play as `no_loop`) and the headers whose opcodes it skips (`<curve>`,
`<effect>`, `<midi>`, `<sample>`). Keyswitch keys select an articulation and make no
sound themselves. Regions that the patch starts from a controller instead of a key
(`on_locc`/`on_hicc`, such as Salamander's pedal noises) or that have a key of -1 (`key`, `lokey`
or `hikey`) never play on a key; until the sampler plays controller triggers they stay silent.

A region whose sample file is missing, or that names a generator other than `*silence` (`*sine`,
`*saw` and the like), is left out: the rest of the patch plays, and the load warning names each
missing file or generator and how many regions it left out. `sample=*silence` is a region that
plays nothing but still cuts off its exclusive group. A patch fails to load only when no region
is left; a sample file that is there but cannot be decoded still fails it.

## Memory

A patch's samples are decoded whole into memory (4 bytes per sample, mono or stereo). Pick
single-articulation patches for large libraries; a patch above 3 GiB is refused.
