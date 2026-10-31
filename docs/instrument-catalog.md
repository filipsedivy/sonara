# Instrument catalog

`sonara_app::instruments` lists the instrument presets in the sample libraries you installed
([sample-libraries.md](sample-libraries.md)), with what is known about each one and whether it
can be played on this machine. It is a read model: it starts no audio engine, changes no project
and adds no agent tool (CTR-01). Sonara ships no samples and no library.

## What a preset holds

Every claim about a preset says who makes it, its **source**: **declared** by the library's own
manifest, which the catalog trusts; **profile**, from the profile Sonara ships for the library
(below), Sonara's reading of the library's files and not the library's word; or **inferred**, a
guess from the words of the patch's path. A claim that nobody makes is `null`.

| Field | Meaning |
|---|---|
| `id` | `<library id>/<patch path below the library, without .sfz>`, for example `vsco2ce/Strings/Violin Solo`, or `<library id>/@<set>` for a multi-patch set, for example `vsco2ce/@horn`. It holds no absolute path, so it survives moving the library or opening the project on another machine. The sampler's `instrument` parameter offers the IDs of the available patches, and projects store them ([sample-libraries.md](sample-libraries.md)). |
| `kind` | `patch`, one patch file, or `set`, a multi-patch set (below) with its `members`: for each patch, its `path` below the library, its `patch_path`, its own `availability` and, once verified, its `facts`. |
| `display_name` | The manifest's name, else the name in the profile Sonara ships for the library (below), else the file name. |
| `family` | `strings`, `woodwinds`, `brass`, `percussion`, `keyboards`, `plucked`, `choir` or `other`, with its source: **declared**, **profile**, **inferred** (words in the path, below) or **unknown**. A manifest or profile states the family, or the instrument the patch plays, whose family it is; where one states both, the family it writes wins, and the manifest's word wins over the profile's. An inferred family is a guess; treat it as one. The family is the orchestration metadata's ([orchestration-metadata.md](orchestration-metadata.md)), so a preset and an instrument type share one family type; a manifest must spell the slug exactly, in lower case. |
| `instrument` | The instrument type the patch plays, an ID of the orchestration metadata, with its source: `{"instrument": "violin", "source": "profile"}`. Declared or stated by a profile; else inferred from the path (below), and then kept only when its family is the preset's family. `null` when nothing names one type, or when the manifest or profile says the patch plays several (`instrument: null`, or a key map). |
| `articulations` | Only what the library's manifest declares: the names of a version 1 manifest, the articulation IDs of a version 2 one. Empty means **not verified**, never "none". Keyswitch labels in a patch are not read. |
| `articulation_map` | The articulations and how each is reached, as the manifest or the profile maps them, with the source: `{"value": [...], "source": "declared"}`. The list is written as in the manifest: names for version 1, objects for version 2. `null` means not verified; articulations are never guessed. |
| `ensemble`, `keymap`, `key_offset`, `dynamics`, `legato`, `in_situ` | The manifest's or the profile's value of the version 2 field of the same name (below), each as `{"value": …, "source": …}`, written as in the manifest. |
| `license` | The licence the library's manifest states for this recording (a version 2 entry's own `license`), else for the library. `None` means **unknown**: read the library's own terms. |
| `license_claim` | The licence of this recording, else of its library, as the manifest declares it or the profile states it, with the source. |
| `verdict` | How well the preset plays as the manifest or the profile judges it, with the source: `{"value": {"state": "partly", "lost": [...]}, "source": "profile"}`. |
| `facts` | What the structural check found in the patch (below), once `verify` has run; `null` before, and always for a set, whose members hold their own. |
| `availability` | `available` (a regular, non-empty, readable file inside the library), `missing` (listed by the manifest or by Sonara's profile of the library, not on disk) or `invalid` with a reason. A set is `available` when every member is, `missing` when a member is missing, else `invalid` when a member is, with the member's path before the reason, and `invalid` when it names no patch. |
| `patch_path` | The `.sfz` file on this machine. For a set, the patch of its `default` articulation, else of its first one. |

SFZ patches have no bank or program number, so presets carry none.

An inferred family comes from the English instrument names and aliases of the orchestration
metadata, read with its `resolve` ([orchestration-metadata.md](orchestration-metadata.md)), so
`Violin`, `French Horn`, `Vibes` and `Celeste` name their families. A
run-together name is split at its capitals and digits first (`TubularBells`, `FHornSus`,
`Horn2`). The file name decides when its words agree on one family, else the nearest folder whose
words do. A word of two families names none: `Bass` is the double bass or the bass voice, so
`Strings/Bass.sfz` is strings and a bare `Bass.sfz` is unknown. A word the metadata does not
list, such as `Drums` or `Chorus`, names nothing.

An inferred instrument is read from the same words, the file name first, then the folders. A
segment decides when a word names exactly one instrument type and every other word it names
something with agrees (the same type, a category that holds it, or its family): `Solo Violin`,
`FHornSus` and `Timpani 1 - Hit` name the violin, the horn in F and the timpani. A category alone
names no type, even one with a usual member (`Trumpet`, `Clarinet`, `Piano`), nor does a family
(`Brass/Patch 1.sfz`), a word of several types (`Bass`) or words that disagree (`Cello Violin`).

## How libraries are found

`InstrumentCatalog::from_settings` reads the `sample_libraries` directories; `discover` takes
them directly. Each directory is scanned for `.sfz` files with the same rules as the sampler
(eight levels deep, hidden folders, symbolic links and folders named `includes` skipped, at most
5,000 patches), and its optional manifest, over the profile Sonara ships for the library if there
is one, is merged in. A patch the manifest does not list is still found and gets an inferred
family, an inferred instrument when its path names one, and no articulations. A manifest or profile entry whose file is gone becomes `missing`;
one that names a file in an `includes` folder is listed like any other entry.

Nothing fails the discovery. Missing or non-directory roots, an unreadable or invalid manifest,
a rejected manifest entry, a library that two of Sonara's profiles match, a duplicate library ID
and a truncated scan are returned in `issues()`; the rest of the catalog is still built. Symbolic
links are never followed, and a manifest path that leaves the library (`..`, absolute, through a
link) is rejected or marked `invalid`.

`InstrumentCatalog::verify` goes deeper on request: it loads one patch through a `PatchLoader`,
which for `MediaPatchLoader` means parsing the SFZ and decoding every sample it names (the load
the sampler does, so it can be slow and use memory). A patch that fails becomes `invalid` with the
loader's message; so does a file with no region, which discovery lists as `available` because it
does not parse the patch. The check is stricter than playback: a patch with a missing sample is
`invalid`, although the sampler plays its other regions. Tests use a fake loader. Verifying a set
loads each of its members in turn; a member that fails makes the set `invalid`, with the member's
path before the loader's message.

The same check records the preset's `facts`: `MediaPatchLoader` reads the patch and the headers
of its samples without decoding them, and the catalog keeps the number of regions and of regions
that can play, the keys that sound with their velocity layers (`sampled`), the switch keys and
the default switch, the opcodes the sampler ignores, the missing and unreadable sample files with
a note for each, the memory the decoded samples would take, and whether the patch is expected to
load. A loader that does not inspect patches records none.

**Facts beat claims.** `InstrumentPreset::effective_verdict()` is the claimed `verdict` lowered by
the facts, each finding added to what is lost: a patch with no region, none that can play, an
unreadable sample or samples over the sampler's 3 GiB cap is `not`; a sample left out, or a
`keyswitch` articulation whose key the patch does not switch on, makes it at best `partly`. The
opcodes the sampler ignores lower nothing by themselves; what they cost is the claim's to say.
Without a claim and without a finding, the verdict is unknown (`None`). The claim itself is kept
as it was made. A set takes the findings of every member, each led by the member's path, and
checks a `keyswitch` articulation against the member that plays it; a member that does not load
makes the whole set `not`.

## The manifest, `sonara-library.json`

Optional, in the library's root directory, at most 1 MiB. Version 1:

```json
{
  "version": 1,
  "id": "vsco2ce",
  "name": "VSCO 2 Community Edition",
  "license": "CC0 1.0",
  "instruments": [
    { "path": "Strings/Violin Solo.sfz", "name": "Solo Violin", "family": "strings",
      "articulations": ["sustain", "pizzicato"] }
  ]
}
```

Only `version` (1 or 2) is required. `id` is a slug (`a-z`, `0-9`, `-`, `_`, at most 64); without
it the library takes the ID of a library Sonara recognizes (below), or else its directory name,
slugged. `path` uses `/` and is relative to the library. Write only what you verified: the
catalog trusts a manifest.

### Version 2

Version 2 keeps every field of version 1 and says, per patch, what instrument it plays and how
Sonara can play it:

```json
{
  "version": 2,
  "id": "my-orchestra",
  "name": "My Orchestra",
  "license": "CC0 1.0",
  "attribution": null,
  "source": { "url": "https://example.org/my-orchestra", "revision": "v1.0" },
  "profile": { "checked": "2026-10-30", "method": "code_reading" },
  "instruments": [
    { "path": "Strings/Violins.sfz", "name": "Violins", "family": "strings",
      "instrument": "violin", "ensemble": { "kind": "section", "players": 16 },
      "articulations": [
        { "id": "sustain", "via": "keyswitch", "key": 24, "label": "Sustain", "default": true },
        { "id": "pizzicato", "via": "keyswitch", "key": 25 },
        { "id": "staccato", "via": "velocity", "lo": 1, "hi": 63 }
      ],
      "dynamics": { "via": "cc", "cc": 1 },
      "legato": { "kind": "overlap", "crossfade_ms": 90, "start_offset_ms": 60, "gap_ms": 0 },
      "in_situ": null,
      "verdict": { "state": "as_is", "lost": [] } },
    { "set": "trumpet", "name": "Trumpet", "family": "brass", "instrument": "trumpet_bb",
      "ensemble": { "kind": "solo", "players": 1 },
      "articulations": [
        { "id": "sustain", "via": "patch", "path": "Brass/Trumpet Sustain.sfz", "default": true },
        { "id": "sustain", "via": "patch", "path": "Brass/Trumpet Muted.sfz", "mute": "con_sordino" },
        { "id": "staccato", "via": "patch", "path": "Brass/Trumpet Staccato.sfz" }
      ] },
    { "path": "Percussion/Kit.sfz", "family": "percussion", "instrument": null,
      "keymap": [
        { "keys": [36], "instrument": "bass_drum" },
        { "keys": [38], "instrument": "snare_drum" },
        { "keys": [39], "instrument": "snare_drum", "articulation": "roll", "name": "Snare roll" }
      ],
      "dynamics": { "via": "velocity" } }
  ]
}
```

| Field | Of | Value |
|---|---|---|
| `id`, `name`, `license` | library | As in version 1. |
| `attribution` | library | Credit text that the licence requires. |
| `source` | library | `{url, revision}`: where the library comes from and the revision the claims describe. `url` is required. |
| `profile` | library | `{checked, method}`: the day the claims were checked (`YYYY-MM-DD`) and how, `code_reading` or `render`. |
| `detect` | library | Paths that recognize a copy of the library; only Sonara's own profiles use them (below). |
| `path` | entry | One patch, as in version 1. It must not start with `@`. An entry has exactly one of `path` and `set`. |
| `set` | entry | A multi-patch set of one instrument, named by a slug (`a-z`, `0-9`, `-`, at most 64): each of its articulations names its patch with `path`. |
| `name`, `family` | entry | As in version 1. |
| `instrument` | entry | The instrument type the patch plays, an ID of the orchestration metadata ([orchestration-metadata.md](orchestration-metadata.md)) such as `violin` or `horn_f`, or `null` for a patch of several instruments, which uses `keymap`. |
| `ensemble` | entry | `{kind, players}`: `solo`, `section` or `section_emulated`, and the number of players (1 to 64) or `null` when not stated. A solo has one player. |
| `articulations` | entry | The articulations and how each is reached (below). |
| `keymap` | entry | For percussion patches with one sound per key: rows of `{keys, instrument, articulation, name}`; `keys` are 0 to 127, at least one per row, and no key is in two rows. `articulation` and `name` (the pad's name) are optional. |
| `key_offset` | entry | Semitones, −48 to 48, added to incoming notes, for a patch keyed away from sounding pitch. |
| `dynamics` | entry | What drives dynamics: `{via: velocity}`, `{via: cc, cc}` (a controller, 0 to 127) or `{via: fixed, velocity}` (every note at that velocity, 1 to 127). |
| `legato` | entry | `{kind: none}`, `{kind: trigger}` (the patch's own legato regions) or `{kind: overlap, crossfade_ms, start_offset_ms, gap_ms}` (10 to 500, 0 to 500 and 0 to 100 ms, all required). |
| `in_situ` | entry | `true` when the library was recorded in seating position, `false` when not, `null` when unknown. |
| `verdict` | entry | `{state, lost}`: `as_is`, `partly` or `not`, and what is lost, in words. |
| `license` | entry | The licence of this recording when it differs from the library's. |

Each articulation is `{id, via, …}`:

- `id` is an articulation ID of the orchestration metadata, such as `sustain` or `pizzicato`
  (spelled exactly), or `ordinario` for a patch whose ordinary sound no better ID describes.
- `via` says how it is reached, with only the fields that go with it: `patch` (the whole patch),
  `keyswitch` with `key` (0 to 127), `velocity` with `lo` and `hi` (1 ≤ `lo` ≤ `hi` ≤ 127) or
  `cc` with `cc` and `value` (0 to 127).
- `path` names the patch that plays it in a `set`, where it is required; an entry of one patch
  never has it.
- Optional: `label` (the library's own name for it), `default: true` for the patch's ordinary
  sound (at most one per entry), `mute` and `vibrato` for the state a recording carries (`mute`
  is one of the mute articulation IDs, `con_sordino`, `mute_cup`, `mute_harmon`, `mute_plunger`,
  `mute_bucket`, `stopped`, `palm_mute` or `xylophonic`; `vibrato` is `vibrato_on` or
  `non_vibrato`), and `attack_ms` (0 to 2000), the onset of its samples.
- An ID appears once per mute and vibrato: `sustain` and `sustain` with `mute: con_sordino` are
  two articulations.

**Validation.** A version 2 manifest is checked entry by entry: an entry with an unknown
instrument or articulation ID, a key outside 0 to 127, two defaults, a path that leaves the
library, a value of the wrong type or any other value above that does not hold is rejected on its
own and reported in `issues()` as `manifest_entry_rejected`, named by its `path`, by `@<set>`, or
by `#<n>` (its position, counted from 1) when it names neither; the rest of the manifest is used.
A library field that does not hold (`id`, `source`, `profile`, `detect`) makes the whole manifest
invalid, as does a version other than 1 or 2: the library is then scanned without it. A version 1
manifest is read as before, in one pass, so there a value of the wrong type in any entry makes the
whole manifest invalid. Sonara builds that read only version 1 report a version 2 manifest as
invalid and scan the library without it.

**What the catalog shows.** From a version 2 entry the catalog shows every field of the preset:
the name, the family, the articulation IDs and the licence as before, and each of the others as
a claim with its source (above). A `set` entry becomes a preset of its own, `<library id>/@<set>`
with the kind `set`, whose members are the patches its articulations name, each once, spelled as
on disk; each of those patches is still listed as the preset it is, with its own entry's claims.
A set without a stated instrument or family has them inferred from its slug, as a patch has them
from its path. A set whose ID a patch already has (a file `@<set>.sfz` in the library root) is not
listed. The sampler plays one patch per device, so the sampler's `instrument` parameter offers a
set's patches, not the set.

**Over a shipped profile.** Sonara's profiles (below) are written in the version 2 shape. When one
applies to a library, the library's manifest lies over it, field by field: a field the manifest
writes replaces the profile's, a field written as `null` clears it, and a field it leaves out
keeps the profile's. `articulations` and `keymap` are replaced whole, never merged item by item.
An entry only in the profile stays, an entry only in the manifest is added, and entries match by
`path` ignoring case, or by `set`. A version 1 manifest replaces only the fields it sets: it
cannot clear one. What a profile claims is Sonara's word, not the library's, so the catalog never
shows it as declared: each claim the profile makes is shown with the source `profile`, a field the
manifest writes with the source `declared`, and `articulations` and `license` keep only what the
library's own manifest declares.

## Libraries Sonara recognizes

Sonara carries a profile of two free libraries: their identity, so that their preset IDs are the
same on every machine and survive renaming the folder, with or without a manifest, and what each
of their patches plays:

| Library | ID | Recognized by these files below its root |
|---|---|---|
| VSCO 2 Community Edition (branch `SFZ`) | `vsco2ce` | `ViolinEns-KS.sfz`, `Tuba-KS.sfz`, `GM-StylePerc.sfz` |
| Versilian Community Sample Library (tag `v1.2.2-RC`) | `vcsl` | `Aerophones/Reed Aerophones/Tenor Saxophone - Keyswitch.sfz`, `Idiophones/Struck Idiophones/Triangles.sfz`, `Membranophones/Struck Membranophones/Timpani 1 - Keyswitch.sfz` |

A library whose manifest declares no `id`, or that has no manifest, and that holds all of one
row's files as regular files takes that row's ID and name. A library whose manifest declares one
of these IDs is that library too. A manifest `id` always wins. A folder that holds the files of
both libraries takes neither ID and is reported in `issues()`; two copies of one library both take
its ID, so the second is skipped as a duplicate.

### What the profiles say

Each profile is a version 2 manifest that Sonara carries in `crates/sonara-app/profiles/`:
Sonara's own data, file names and facts read from the library's patch files, with no samples and
no SFZ text. It lists every patch of its revision, 75 for VSCO 2 CE and 183 for VCSL, so a patch
your copy lacks shows as `missing`. The catalog shows each patch's name from the profile: VSCO 2
CE's file names are short codes, so its profile spells them out (`ViolinEnsSusVib` is "Violin
Section, sustain vibrato"), while VCSL's patches keep their file names. The catalog shows the
rest of the profile as claims with the source `profile`, never as the library's word; the fields
`articulations` and `license` still come only from a manifest of the library's own. The profiles
say:

- **What a patch plays:** the instrument type and solo or section. VSCO 2 CE's `Ens` strings are
  sections; everything else in both libraries is taken as solo, as the single-instrument names
  suggest (neither library says). A patch of an instrument the orchestration
  metadata does not know (recorders, harmonicas, congas, the TX81Z synthesizer and the like)
  claims only a family.
- **Articulations:** each switch of a key-switch patch that the articulation vocabulary names,
  with the library's own label, and the articulation of each single-articulation patch. The one
  marked `default` is the switch the patch itself starts on (`sw_default`) when that is an
  ordinary sound. The VCSL vibraphone starts on the bow, so its soft mallets are marked instead,
  and the Flemish harpsichord and the Renaissance organ start on a 4' stop, so their 8' stop is.
  Switches between registrations, stops, mallets or drums, which the vocabulary does not name,
  are left out, apart from the ordinary one, which is mapped as `ordinario`.
- **Vibrato and mutes:** a recording is marked `vibrato_on` only beside a plain sustain of the same
  instrument (VSCO 2 CE's `TrumpetSusVib` beside `TrumpetSus`) and `non_vibrato` only beside one
  with vibrato (`FluteSusNV` beside `FluteSusVib`), so each instrument keeps one plain `sustain`.
  A muted recording is `sustain` with its mute: `con_sordino` for the horn's mute and the
  trumpet's straight mute, `mute_harmon` for the harmon mute.
- **Sets:** an instrument's single-articulation patches form a set, for example VSCO 2 CE's
  `@horn` (`FHornSus`, `FHornStac`, `FHornMute`) and VCSL's `@timpani-1` (`Timpani 1 - Hit` and
  `- Roll`): 15 sets in VSCO 2 CE and 4 in VCSL, listed beside the patches as `vsco2ce/@horn` and
  `vcsl/@timpani-1`. A second, quieter sustain recording (VSCO 2 CE's `-Quiet` patches) stays out
  of its set.
- **Key offset:** VSCO 2 CE's piccolo, glockenspiel and xylophone and VCSL's glockenspiel and
  xylophones are keyed an octave below the pitch they sound (measured from one sample of each), so
  their entries carry `key_offset: -12`. The marimbas, vibraphones, harps and tubular bells sound
  at their keys.
- **Percussion:** a patch with one sound per key has a key map of the keys whose sound the
  orchestration metadata names; keys of other instruments (VSCO 2 CE's congas, cowbell, claves
  and ratchet, for example) and sounds without an articulation ID are left out. VSCO 2 CE's
  `GM-StylePerc` is not a General MIDI kit: keys 42 and 46 are a gong, 47, 48 and 50 cymbal swells.
- **Dynamics:** the velocity, for every patch.
- **Verdicts,** checked on 2026-10-30 by reading every patch file, with velocity crossfades and
  `amp_veltrack` counted as played. A `partly` verdict names what is lost: envelope decay
  (`ampeg_decay` with `ampeg_sustain`) and `rt_decay`, which the sampler does not play; the
  sustain-pedal gates (`locc64`, `hicc64`) of VCSL's Steinway B, whose pedal-up and pedal-down
  samples sound together, and the pedal noises of the Knight upright; and switch keys inside the
  instrument's playable range: VSCO 2 CE's `CelloEns-KS` (84 to 87) and VCSL's
  `Timpani 1 - Keyswitch` (36 and 37), `Renaissance Organ - Keyswitch` (93 to 96) and
  `Xylophone - Keyswitch` (48 to 51, which the sounding C4 to D#4 reach through the offset). A
  note on such a key changes the articulation and does not sound; use the separate patches for a
  part that goes there. Every VSCO 2 CE patch sets `ampeg_dynamic`, which changes nothing audible.
- **Not claimed:** legato times (they wait for a listening check), whether the library was
  recorded in seating position, and licences per patch: both libraries are CC0 as a whole.

A manifest in the library's folder lies over its profile field by field (above), so you can
rename a preset, correct a claim or clear one with `null`.

## Licence and installation notes

- Install a library by adding its directory to `sample_libraries` and restarting
  ([sample-libraries.md](sample-libraries.md)).
- Each library is under its own licence, which binds you, not Sonara. Checked sources and their
  licences are in [orchestral-audio-engine-research.md](orchestral-audio-engine-research.md),
  section 5: VSCO 2 CE and VCSL are CC0, Sonatina is CC Sampling Plus 1.0 (not bundlable), Virtual
  Playing Orchestra has share-alike parts. Put the licence in the manifest so it travels with the
  catalog.
- Sonara bundles nothing (ADR 0010). Adding any library to the repository or the app bundle needs
  a per-library licence decision first (LIC-01).

## Not done

No agent tool, CLI command or GUI lists the catalog itself yet. The sampler's `instrument` choices
are its available presets' IDs; the device editor shows them as the presets' names grouped by
library (`InstalledInstruments::by_library`), and a preset whose name another preset of the same
library shares also shows its patch path below the library. The catalog lists no set as a
preset, and records a patch's facts only when `verify` runs on it, one preset at a time. A General
MIDI or SF2 source would add bank and program. Sound quality of any library has not been judged.
