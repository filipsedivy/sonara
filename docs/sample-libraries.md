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
(`sw_*`), release samples (`trigger=release` and `release_key`), legato triggers
(`trigger=first` and `legato`), exclusive groups (`group`/`off_by`, faded out as
`off_mode`/`off_time` say), loops, attack and release times, tuning and volume. A region's level
follows the velocity as its `amp_veltrack` says: at the default 100 % the gain is
`(velocity / 127)²`, at 0 % (organs, harpsichords) every velocity plays at full level, and below 0
the response is inverted, as the SFZ format and sfizz define it. Velocity crossfades (`xfin_lovel`/`xfin_hivel`, `xfout_lovel`/`xfout_hivel`, with
`xf_velcurve` `power`, the default, or `gain`) fade a layer in and out over a range of velocities,
so dynamic layers written that way blend instead of all sounding at full level on every note; a
layer a note's velocity leaves silent starts no voice. `amplitude` scales a region's level in
percent. MIDI: notes, sustain pedal (CC 64), volume (CC 7), expression (CC 11), the controllers a
patch uses (below), pitch bend (±2 semitones) and all notes off. Parameters: volume, pan, release
scale, instrument, and instrument type, articulation and dynamics (below).

Release samples start when the key is released; for a key released while the sustain pedal is
down, they start when the pedal lifts, at the velocity the key was pressed with and with the
keyswitch that was active at the release. All notes off drops the releases still waiting for the
pedal. A `trigger=release_key` region ignores the pedal and starts at the key release in any case
(Sonatina's timpani end their rolls this way). A release sample plays on: releasing the same key
again later does not fade it.

A `trigger=first` region plays only when its key is pressed while no other key is down, and a
`trigger=legato` region only while another key is down, as the SFZ format and sfizz define them.
Sonatina's legato patches are written this way: the first note of a phrase plays its attack, and
each note played while the previous key is still held plays the legato group, which overlaps the
old note for the patch's `off_time`. Held keyswitch keys do not count as notes (sfizz counts
them), keys the sustain pedal holds after their release are not down, and all notes off (a seek or
a loop wrap) starts the next note as a new phrase.

An exclusive group switches off the notes of the regions that name it in `off_by`, and each fades
out as its region's `off_mode` says: `fast` (the default) in 10 ms, `normal` over the region's own
release (scaled by the release parameter, as a key release is), and `time` over `off_time` seconds
(6 ms when the patch gives none), which the release parameter does not scale. A region that writes
`off_time` without `off_mode` fades over it too, as ARIA and sfizz read it. So Sonatina's legato
patches (`off_mode=time off_time=1`) overlap each note with the next for a second instead of
cutting it off. A switch-off never makes a fade already under way longer, and the sustain pedal
does not hold a note its group has switched off. `off_curve` and `off_shape` are not supported.

The sampler keeps the last value of every MIDI controller on each of the 16 channels. They start
at 0, volume (CC 7) and expression (CC 11) at 127, or where the patch sets them with `set_ccN`,
and a reset of the sampler brings them back there; all notes off lifts the sustain pedal, so CC 64
reads 0 afterwards. `set_ccN` sets only what the patch's own controller opcodes read: the volume and
expression the sampler applies start full, and the pedal up, whatever the patch sets. A patch uses
the controllers in four ways, each on the channel of its note:

- **Gates** (`loccN`/`hiccN`): a region plays only while controller N lies in its range when the
  region starts, at the key press, at the key release for a release sample (at the pedal's lift
  for a key released under the pedal) or at the controller event. A note that already sounds is
  not cut off when the controller moves. Piano layers for pedal up and pedal down are written
  this way.
- **Triggers** (`on_loccN`/`on_hiccN`): a controller event in the range starts the region instead
  of a key, at the sample's own pitch and full level, chosen by `lorand`/`hirand` like any region
  but outside the round robin. Every such event starts it again. A key release does not end it;
  it plays to its end or until all notes off. Salamander's pedal noises play this way when the
  pedal goes down and when it comes up.
- **Crossfades** (`xfin_loccN`/`xfin_hiccN`, `xfout_loccN`/`xfout_hiccN`, with `xf_cccurve`
  `power`, the default, or `gain`): a region fades in and out with controller N, also while it
  sounds, so raising CC 1 crossfades the dynamic layers of a held chord (Sonatina's choirs). A
  layer the controller fades out entirely still starts its voice, silently, so that the
  controller can bring it in.
- **Level** (`volume_onccN`, also written `gain_ccN`, in dB at value 127; `amplitude_onccN` in
  percent at value 127): the region's level follows the controller, also while it sounds.

Crossfades and level changes reach sounding notes over one audio block with a linear ramp, so a
jump of the controller does not click; a controller event takes effect on sounding notes from the
block after it. The high-resolution forms (`lohdccN`, `on_lohdccN`, `set_hdccN` and the like) and
controllers above 127 are not supported.

Not supported: key crossfades, velocity curves (`amp_velcurve_N`), filters, per-region pan,
streaming, and controller modulation of anything but the level (for example Sonatina's vibrato on
CC 21 and filter cutoff on CC 1).
The loader reports unsupported opcodes in the load warning when a patch is rendered, together with
the values the sampler does not play (unknown `trigger` values, which play as `attack`, and
unknown `loop_mode` and `off_mode` values, which play as if the opcode were absent: an unknown
`loop_mode` plays as `no_loop`, or loops a sample that carries its own loop, below) and the headers
whose opcodes it skips
(`<curve>`, `<effect>`, `<midi>`, `<sample>`). A keyswitch key selects an articulation; it makes no
sound unless a region of the patch also covers that key, and then it does both. Regions that the
patch starts from a controller instead of a key (`on_locc`/`on_hicc`, such as Salamander's pedal
noises) or that have a key of -1 (`key`, `lokey` or `hikey`) never play on a key; the controller
starts the former (above).

A region whose sample file is missing, or that names a generator other than `*silence` (`*sine`,
`*saw` and the like), is left out: the rest of the patch plays, and the load warning names each
missing file or generator and how many regions it left out. `sample=*silence` is a region that
plays nothing but still cuts off its exclusive group. A patch fails to load only when no region
is left; a sample file that is there but cannot be decoded still fails it.

A header may be glued to the opcode before or after it, as the SFZ format site's examples write
it (`<region>key=36 sample=kick.wav`): an opcode's value ends where a header starts, as in sfizz.
A `#define` value is the rest of its line, spaces included, so a define can name a folder with a
space in it or stand for several opcodes (`#define $LAYER lovel=64 hivel=127`); ARIA and sfizz
keep only its first word.
Comments are read as in sfizz: `//` to the end of the line, `/* … */` to its close, and a `/*`
inside a `//` comment opens nothing.

A WAV sample can carry its own loop (a `smpl` chunk, which sample editors write). As in the SFZ
format and sfizz, a region over such a sample loops between the file's points while it sounds,
unless it writes its own `loop_mode`, `loop_start` or `loop_end`, which win one by one;
`loop_mode=no_loop` plays the sample through. Release samples and regions a controller starts do
not loop by default, since nothing would end the loop. Only the first loop of a WAV file is read,
and it plays forward; loops stored in AIFF or FLAC files are not read.

## Instrument type, articulation and dynamics

- **`instrument_type`**: `auto` (the default) or an instrument type ID such as `horn_f` or
  `violin`, in score order. It names the instrument a preset plays, for a preset whose library
  does not say; it does not change the sound. It cannot be automated.
- **`articulation`**: `auto` (the default), `ordinario`, then the articulation IDs (`sustain`,
  `legato`, `staccato`, ...) in a fixed order. It can be automated. An automation lane stores
  the position of a choice, so the list never changes its order: new articulations are added at
  its end.
- **`dynamics`**: 0 to 1, default 1. It can be automated.

Every sampler device offers the same lists, whatever preset it plays. **The sampler stores these
values but does not play them yet**: a track sounds the same whatever they are set to.

A project saved by a newer version of Sonara can hold an instrument type or an articulation that
this version does not know. It opens unchanged, and the device plays as `auto`; the render warns
`articulation '…' of '…' is unknown to this version of Sonara; it plays as 'auto'`.

## Memory

A patch's samples are decoded whole into memory (4 bytes per sample, mono or stereo). Pick
single-articulation patches for large libraries; a patch above 3 GiB is refused.
