# Sample libraries and the sampler

`sonara.sampler` plays SFZ instruments from sample libraries that you install yourself. Sonara
ships no samples (ADR 0010).

## Install a library

1. Download a library whose licence suits your use. Checked sources and their licences are in
   [orchestral-audio-engine-research.md](orchestral-audio-engine-research.md), section 5.
   For a start: **VSCO 2 Community Edition** (CC0), branch `SFZ` of
   <https://github.com/sgossner/VSCO-2-CE>. Clone or download that branch so the `.sfz` files
   and their `Samples` folders stay together. **VCSL** (CC0) adds keyboards, saxophones and
   percussion; its `.sfz` files exist only in tag `v1.2.2-RC` of
   <https://github.com/sgossner/VCSL>, not on its default branch, so check out or download that
   tag.
2. Add the library's directory to the settings file (`settings.json` in Sonara's config
   directory; unknown keys are kept):

   ```json
   { "sample_libraries": ["/Users/you/Music/VSCO-2-CE"] }
   ```

3. Restart Sonara. The directories are scanned for `.sfz` files at startup (eight levels deep,
   hidden folders, symbolic links and folders named `includes` skipped, at most 5,000 patches).
   An `includes` folder holds the fragments that a library's patches include (Sonatina has 190
   of them), which cannot be played on their own, so they are not offered as instruments. Nor
   is a patch file that is empty, unreadable, or listed by the library's manifest or by Sonara's
   profile of the library but missing.

To replace or add a library, edit the list and restart.

## Preset IDs

The sampler's instruments are named by **preset ID**, `<library id>/<patch path below the
library, without .sfz>`, for example `vsco2ce/ViolinEns-KS`. A project stores the ID, not a file
path, so it plays on any machine where the library is installed, wherever it lies. On a machine
without the instrument the track plays silence and the render warns
`instrument … is not installed`.

The library ID is the `id` in the library's manifest
([instrument-catalog.md](instrument-catalog.md)). Without one, Sonara recognizes **VSCO 2 CE**
and **VCSL** by a few of their files and uses `vsco2ce` and `vcsl`. Any other library without a
manifest takes its folder name, lower-cased with other characters turned into `-`; renaming that
folder changes its IDs, so give the library a manifest with an `id` before you use it in
projects.

A project saved before preset IDs stores the patch's absolute path. It still plays when that
path lies below a configured library: Sonara reads it as the preset of that patch and leaves the
project unchanged until you choose the instrument again, which stores the ID. A tool call or
script that passes such a path to `set_instrument` or as the `instrument` parameter stores the
preset ID too.

## VSCO 2 CE and VCSL

Sonara carries a profile of each of these two libraries
([instrument-catalog.md](instrument-catalog.md), "What the profiles say"). It lists every patch of
the revision named above, with what the patch plays, and gives VSCO 2 CE's patches readable
names: the Instrument combo shows `ViolinEns-KS` as "Violin Section, key switches" and `FHornMute`
as "Horn, muted sustain". VCSL's patches keep their file names. A patch the profile lists that
your copy lacks is not offered. A manifest in the library's folder overrides the profile.

Some key-switch patches use switch keys that the instrument can play: VSCO 2 CE's
`CelloEns-KS` (C6 to D#6) and VCSL's `Timpani 1 - Keyswitch` (C2 and C#2),
`Renaissance Organ - Keyswitch` and `Xylophone - Keyswitch`. A note there changes the
articulation instead of sounding. The libraries' single-articulation patches switch nothing.

## Try an instrument

- **Agent or CLI:** `list_plugins` with `plugin_id: "sonara.sampler"` shows the installed presets'
  IDs as the choices of `instrument`. Then `create_track` (`kind: instrument`,
  `instrument: "sonara.sampler"`) and `set_instrument` with
  `params: {"instrument": "<preset id>"}`, add a MIDI clip, and `render`.
- **GUI:** Sonara Sampler is among the instruments of Replace Instrument and the Browser. Its
  device card's Instrument combo lists the presets by name, grouped under each library's name
  (the manifest's name, VSCO 2 CE's and VCSL's own, else the library ID); when two presets of a
  library have the same name, each also shows its patch path below the library. Choosing one
  stores its preset ID. A project that stores an older patch path shows the preset it plays; an
  instrument that is not installed on this machine shows as "Not installed: <name>" until you
  choose another. **This was not exercised in a running app** (the page was tested against
  mocked commands); if the sampler does not show up, use the agent route above.

If `sonara.sampler` is not listed, no configured directory holds a `.sfz` file.

## What the sampler plays

Velocity layers, round robin (`seq_length`/`seq_position`, `lorand`/`hirand`), keyswitches
(`sw_*`), release samples (`trigger=release`), exclusive groups (`group`/`off_by`), loops,
attack and release times, tuning and volume. MIDI: notes, sustain pedal (CC 64), volume (CC 7),
expression (CC 11), pitch bend (±2 semitones) and all notes off. Parameters: volume, pan,
release scale, instrument.

Release samples start when the key is released; for a key released while the sustain pedal is
down, they start when the pedal lifts, at the velocity the key was pressed with and with the
keyswitch that was active at the release. All notes off drops the releases still waiting for the
pedal.

Not supported: CC-driven layers and crossfades (for example Sonatina's dynamics on CC 1 and
vibrato on CC 21), filters, per-region pan, streaming. The loader reports unsupported opcodes in
the load warning when a patch is rendered, together with the values the sampler does not play
(`trigger=first`, `legato` or `release_key`, which play on every key press for now, and unknown
`loop_mode` values, which play as `no_loop`) and the headers whose opcodes it skips (`<curve>`,
`<effect>`, `<midi>`, `<sample>`). A keyswitch key selects an articulation; it makes no sound
unless a region of the patch also covers that key, and then it does both. Regions that the patch
starts from a controller instead of a key
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
