# Instrument catalog

`sonara_app::instruments` lists the instrument presets in the sample libraries you installed
([sample-libraries.md](sample-libraries.md)), with what is known about each one and whether it
can be played on this machine. It is a read model: it starts no audio engine, changes no project
and adds no agent tool (CTR-01). Sonara ships no samples and no library.

## What a preset holds

| Field | Meaning |
|---|---|
| `id` | `<library id>/<patch path below the library, without .sfz>`, for example `vsco2ce/Strings/Violin Solo`. It holds no absolute path, so it survives moving the library or opening the project on another machine. |
| `display_name` | The manifest's name, else the file name. |
| `family` | `strings`, `woodwinds`, `brass`, `percussion`, `keyboards`, `plucked`, `choir` or `other`, with its source: **declared** (manifest), **inferred** (words in the path, such as `Violin`, `Horn`, `Timpani`) or **unknown**. An inferred family is a guess; treat it as one. The family is the orchestration metadata's ([orchestration-metadata.md](orchestration-metadata.md)), so a preset and an instrument type share one family type; a manifest must spell the slug exactly, in lower case. |
| `articulations` | Only what a manifest declares. Empty means **not verified**, never "none". Keyswitch labels in a patch are not read. |
| `license` | The licence the library's manifest states. `None` means **unknown**: read the library's own terms. |
| `availability` | `available` (a regular, non-empty, readable file inside the library), `missing` (listed by the manifest, not on disk) or `invalid` with a reason. |
| `patch_path` | The `.sfz` file on this machine. |

SFZ patches have no bank or program number, so presets carry none.

## How libraries are found

`InstrumentCatalog::from_settings` reads the `sample_libraries` directories; `discover` takes
them directly. Each directory is scanned for `.sfz` files with the same rules as the sampler
(eight levels deep, hidden folders and symbolic links skipped, at most 5,000 patches), and its
optional manifest is merged in. A patch the manifest does not list is still found and gets an
inferred family and no articulations. A manifest entry whose file is gone becomes `missing`.

Nothing fails the discovery. Missing or non-directory roots, an unreadable or invalid manifest,
a rejected manifest entry, a duplicate library ID and a truncated scan are returned in
`issues()`; the rest of the catalog is still built. Symbolic links are never followed, and a
manifest path that leaves the library (`..`, absolute, through a link) is rejected or marked
`invalid`.

`InstrumentCatalog::verify` goes deeper on request: it loads one patch through a `PatchLoader`,
which for `MediaPatchLoader` means parsing the SFZ and decoding every sample it names (the load
the sampler does, so it can be slow and use memory). A patch that fails becomes `invalid` with the
loader's message. The check is stricter than playback: a patch with a missing sample is `invalid`,
although the sampler plays its other regions. Tests use a fake loader.

## The manifest, `sonara-library.json`

Optional, in the library's root directory, at most 1 MiB:

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

Only `version` (1) is required. `id` is a slug (`a-z`, `0-9`, `-`, `_`, at most 64); without it the
directory name is slugged. `path` uses `/` and is relative to the library. Write only what you
verified: the catalog trusts a manifest.

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

No agent tool, CLI command or GUI uses the catalog yet, so the sampler's `instrument` choices are
still the raw patch paths. A General MIDI or SF2 source would add bank and program. Sound quality
of any library has not been judged.
