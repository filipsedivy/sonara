# Sound source (layer b)

_Status: proposed (2026-10-30). Part of the orchestra design; decisions in [../../orchestra-design.md](../../orchestra-design.md)._

## Summary

This layer turns "a horn section, staccato, mezzo-forte" into samples that sound. It refines spine
decisions S4-S8 and adds no agent tool, schema field, IPC message or CLI command (CTR-01).

- **Library profiles.** `sonara-library.json` version 2 says, per preset: the orchestration id;
  solo or section; how each articulation is reached (`patch`, `keyswitch`, `velocity`, `cc`); what
  drives dynamics; the legato kind; in situ or not; a verdict; the licence. Sonara ships profiles
  for **VSCO 2 CE** and **VCSL v1.2.2-RC** (own 0BSD data, embedded in `sonara-app`). A user's
  manifest overrides them field by field. Version 1 manifests keep working.
- **Preset ids.** The `instrument` choices become catalog preset ids, plus profile-defined
  multi-patch sets. An app-injected resolver in `MediaProvider` turns an id into patch files and a
  compiled articulation table. Old absolute paths are mapped to ids before validation. This amends
  ADR 0010 decision 4 (owner, A5).
- **Three new sampler parameters.** `instrument_type` (identity of an unprofiled preset);
  `articulation`, an automatable enum over one fixed, append-only list (`auto`, `ordinario`, the
  vocabulary); `dynamics`, an automatable float from 0 to 1. A change becomes a forced key switch, a
  velocity zone, another preloaded patch member or a controller value by a table lookup, with no
  allocation. Lanes are evaluated at every block's own position, so a seek or a loop wrap
  re-applies the right articulation. This fixes C11 on the authoring path.
- **Sampler engine work, in spine order**, each with its TST-06 fixture from
  [../sfz-conformance.md](../sfz-conformance.md) section D: (1) conformance Q1/N4, Q11, Q2-Q4, Q7,
  N1, N3, N2, C8; (2) expression Q6, Q8, CC gates and crossfades, `dynamics`, Q9; (3) legato: Q5
  triggers and an overlap-crossfade legato on sustain samples; (4) multi-patch sets with one loader,
  so the 3 GiB cap is per device.
- **Degradation chain.** A pure `sonara-app` service implements the seven steps of
  [../sound-mapping.md](../sound-mapping.md) section 4, with a message per substitution, never on
  the audio thread.
- **Outer options.** A General MIDI engine (`rustysynth`) gets a QG-13 and RT review plan with
  decision criteria (owner, O6). The same articulation table drives a hosted VST3, AU or CLAP
  instrument once M5 exists.

## Today (verified)

Code was read in the `sonara-orchestra` checkout; its `crates/` and `apps/` equal `main` at
`a212544` (Release 0.1.11). Paths below are repository paths.

| # | Fact | Where |
|---|---|---|
| T1 | The sampler is a real-time processor in `sonara-dsp`; the crate root denies the panic and disallowed-method lints. 64 voices live in a fixed array; per-key state is `[u8; 128]`/`[bool; 128]`. | `crates/sonara-dsp/src/lib.rs:6-17`; `crates/sonara-dsp/src/processors/sampler.rs:44,73-94,117-134` |
| T2 | A note-on sets `last_switch` when the key lies in the bank's one switch range, then starts every attack region that matches key, velocity, `sw_last == last_switch`, random range and round robin. | `sampler.rs:228-262,484-488` |
| T3 | `rest()` restores `sw_default`; it runs from `prepare`, `set_state` and `reset` only. `AllNotesOff` releases voices and clears pedal and keys but keeps `last_switch`. | `sampler.rs:158-173,192-200,537-552,579-581` |
| T4 | Gain is `db_to_amp(volume) × (v/127)²`; every choke uses a fixed 10 ms release; a key released under the pedal never starts its release regions (N1). Only CC 7, 11 and 64 act; channel pressure is ignored. | `sampler.rs:50,191,204-226,287-295,315-325,344-345` |
| T5 | `Trigger` has only `Attack` and `Release`; `Region` holds key, velocity, round-robin, `sw_last`, group, loop, offset and envelope fields; a bank has one `Keyswitches` range. | `crates/sonara-dsp/src/bank.rs:10-18,53-102,134-159` |
| T6 | `DeviceState` carries one `sample_bank: Option<Arc<SampleBank>>`; enum values reach a processor as `ParamSlot::Choice(u16)`; at most 48 parameters. The `Processor` trait is the fixed one of TRT (`prepare`, `set_params`, `set_state`, `process`, …). | `crates/sonara-dsp/src/processor.rs:51,124-135,243-255,325` |
| T7 | `sampler_descriptor(patches)` builds parameters `volume`, `pan`, `release_scale` and the enum `instrument` whose choices are the scanned `.sfz` paths. `ep()` forces `automatable: false`. The app registers it only when a scan finds patches; `builtin_registry()` stays at 12. | `crates/sonara-core/src/plugins/builtin.rs:84-91,755-798`; `crates/sonara-app/src/samples.rs:45-54,87-94` |
| T8 | An enum value is coerced from a choice string or an index and stored as the string; the automation command refuses a parameter that is not automatable. | `crates/sonara-core/src/plugins/descriptor.rs:216-230`; `crates/sonara-core/src/commands/automation.rs:69-74` |
| T9 | Device automation is applied once per block, at the block's start beat, before `process`: an enum becomes `Choice(round(clamp(v, 0, n−1)))`. Blocks are 512 frames. A point inside a block therefore takes effect at the next block start (hold curve). | `crates/sonara-engine/src/automation.rs:12-15,164-178`; `crates/sonara-engine/src/graph/process.rs:240-262,296-308`; `docs/audio-engine.md:187` |
| T10 | The graph drops an `instrument` value that is not a current choice, with the warning "… is not installed; it plays silence", loads the bank through `MediaProvider::load_sample_bank(path)` and warns about ignored opcodes. A live rebuild never reuses a sampler, so sounding notes restart. | `crates/sonara-engine/src/graph/mod.rs:763-786,1063-1140` |
| T11 | `MediaProvider` already takes an app-supplied resolver closure for assets and caches banks by path. | `crates/sonara-media/src/provider.rs:22,38-44,89-100,262-270`; `crates/sonara-app/src/media.rs:798` |
| T12 | The SFZ loader dedups samples by file within one `load_sfz` call and caps that call at `MAX_BANK_BYTES` = 3 GiB. | `crates/sonara-media/src/sfz.rs:30,328,414-462` |
| T13 | The chase after a seek or loop wrap sends an all-notes-off and re-sends notes that span the position; controller events are not chased, and a seek while playing does not reset the sampler (C11). | `crates/sonara-engine/src/schedule.rs:6-13,62-70,205-209`; `crates/sonara-engine/src/player/control.rs:795-796` |
| T14 | The catalog preset has `id`, `display_name`, `family`, `library`, `patch_path`, `articulations` (manifest only), `license` and `availability`. The id is `<library>/<stem>`. Without a manifest, the library id is a slug of the folder name (C4, C5). | `crates/sonara-app/src/instruments/model.rs:49-65,90-95,134-155` |
| T15 | The manifest reader accepts only version 1. A rejected manifest is reported and the library is scanned without it. `verify` decodes every sample. | `crates/sonara-app/src/instruments/manifest.rs:33,54-71,110-112`; `discovery.rs:99-120,138-147`; `verify.rs:33-38` |
| T16 | The scan lists every `.sfz` below a root, so the 190 Sonatina include fragments are both presets and `instrument` choices (C8). | `crates/sonara-app/src/samples.rs:56-82`; [../sfz-conformance.md](../sfz-conformance.md) C8 |
| T17 | Allowed crate edges: `sonara-core` has no workspace dependency; `sonara-dsp` → core; `sonara-media` → core, dsp; `sonara-engine` → core, dsp, media, midi, rt; `sonara-app` → engine, media, midi and others. | `crates/sonara-testkit/tests/architecture.rs:15-64` |
| T18 | ADR 0010: the sampler is not a built-in (decision 4, choices are absolute paths); soft failure (5); sfizz, FluidSynth, `rustysynth` and plugin hosting deferred (6). | `docs/decisions/0010-sample-based-orchestral-instruments.md:31-47` |
| T19 | `docs/sample-libraries.md:50-51` says that switch keys make no sound, but a switch key also starts any region mapped to it (C17). | `sampler.rs:233-258` |
| T20 | A stored enum value that is not a choice fails `param_slots`, and with it the graph build (`EngineError::Params`). Only `instrument` gets the stale-value treatment of T10. | `crates/sonara-dsp/src/processors/params.rs:110-141`; `graph/mod.rs:1074-1089` |
| T21 | `EventKind` is a `Copy` enum of six unit variants; `InstrumentEvent` carries `pitch`, `velocity`, `value` and `number`. The synth and the sampler match it exhaustively, the Drum Kit with a wildcard. The schedule sizes its event buffer at build for `1 + 2·notes + controls`. | `crates/sonara-dsp/src/processor.rs:66-100`; `processors/synth.rs:630-642`; `sampler.rs:185-201`; `processors/drums.rs:1225-1236`; `schedule.rs:17-19` |
| T22 | The catalog documents that articulations and the licence come only from a manifest, and a contract test checks that a preset without one claims neither, with an inferred family. | `docs/instrument-catalog.md:13-17`; `crates/sonara-agent/tests/contracts.rs:223-241` |
| T23 | The manifest is deserialized in one `serde` pass: a type error in any entry fails the whole manifest; only the path and family checks run per entry. The scan is matched to manifest entries by lower-cased path. | `manifest.rs:54-71,108-133,143-153`; `discovery.rs:203-215` |

Everything else this layer relies on about SFZ behaviour is in [../sfz-conformance.md](../sfz-conformance.md) (Q1-Q11, C1-C24, N1-N7), which is authoritative.

## Proposal

### 1. The path from a request to sound

```text
request: instrument id, solo/section, articulations, note span     (layer c, planner, GUI)
  -> degradation chain (sonara-app, section 7)            -> SoundPlan + messages
  -> project edits through existing tools: create_track, set_instrument params
     {instrument: <preset id>, articulation, instrument_type}, create_automation <dev>:articulation|dynamics
  -> graph build (sonara-engine, off the RT thread)       -> resolver closure (app) -> SamplerSetup
  -> DeviceState.sampler (Arc, set_state)                  -> sampler on the render thread
```

### 2. Library profiles: `sonara-library.json` version 2

#### 2.1 Example (a VSCO 2 CE excerpt; patch facts from [../sound-mapping.md](../sound-mapping.md) 2.1)

```json
{
  "version": 2,
  "id": "vsco2ce",
  "name": "VSCO 2 Community Edition",
  "license": "CC0 1.0",
  "attribution": null,
  "source": { "url": "https://github.com/sgossner/VSCO-2-CE", "revision": "branch SFZ" },
  "detect": ["ViolinEns-KS.sfz", "Tuba-KS.sfz", "GM-StylePerc.sfz"],
  "profile": { "checked": "2026-10-30", "method": "code_reading" },
  "instruments": [
    {
      "path": "ViolinEns-KS.sfz",
      "name": "Violin Section (key switches)",
      "family": "strings",
      "instrument": "violin",
      "ensemble": { "kind": "section", "players": null },
      "articulations": [
        { "id": "sustain",   "via": "keyswitch", "key": 36, "label": "Sustain Vibrato", "default": true },
        { "id": "tremolo",   "via": "keyswitch", "key": 37 },
        { "id": "spiccato",  "via": "keyswitch", "key": 38 },
        { "id": "pizzicato", "via": "keyswitch", "key": 39 }
      ],
      "dynamics": { "via": "velocity" },
      "legato": { "kind": "overlap", "crossfade_ms": 90, "start_offset_ms": 60, "gap_ms": 0 },
      "in_situ": null,
      "verdict": { "state": "partly", "lost": ["sw_default lost (Q1); the profile default applies", "ampeg_dynamic"] }
    },
    { "path": "ViolinEnsSusVib.sfz", "instrument": "violin", "ensemble": { "kind": "section" },
      "articulations": [{ "id": "sustain", "via": "patch", "default": true }],
      "dynamics": { "via": "velocity" }, "verdict": { "state": "as_is", "lost": ["ampeg_dynamic"] } },
    { "set": "horn-solo", "name": "Horn in F (sustain, staccato, muted)", "family": "brass",
      "instrument": "horn_f", "ensemble": { "kind": "solo", "players": 1 },
      "articulations": [
        { "id": "sustain",     "via": "patch", "path": "FHornSus.sfz", "default": true },
        { "id": "staccato",    "via": "patch", "path": "FHornStac.sfz" },
        { "id": "con_sordino", "via": "patch", "path": "FHornMute.sfz" }
      ],
      "dynamics": { "via": "velocity" },
      "legato": { "kind": "overlap", "crossfade_ms": 70, "start_offset_ms": 50, "gap_ms": 0 },
      "in_situ": null, "verdict": { "state": "as_is", "lost": [] } },
    { "set": "timpani", "instrument": "timpani", "ensemble": { "kind": "solo", "players": 1 },
      "articulations": [
        { "id": "ordinario", "via": "patch", "path": "Timpani.sfz", "default": true },
        { "id": "roll",      "via": "patch", "path": "TimpaniRolls.sfz" }
      ],
      "dynamics": { "via": "velocity" }, "legato": { "kind": "none" },
      "verdict": { "state": "as_is", "lost": [] } },
    { "path": "GM-StylePerc.sfz", "family": "percussion",
      "keymap": [
        { "keys": [36], "instrument": "bass_drum" },  { "keys": [38], "instrument": "snare_drum" },
        { "keys": [39], "instrument": "snare_drum", "articulation": "roll" },
        { "keys": [40], "instrument": "snare_drum", "articulation": "snares_off" },
        { "keys": [49], "instrument": "clash_cymbals" }, { "keys": [54], "instrument": "tambourine" },
        { "keys": [78, 79, 80, 81], "instrument": "triangle" }
      ],
      "verdict": { "state": "as_is", "lost": ["ampeg_dynamic"] } }
  ]
}
```

A VCSL entry shows a set that avoids switch keys inside the playable range. `Timpani 1 - Keyswitch`
switches on keys 36 and 37, which the timpani can play, so the profile builds the timpani from the
separate patches:
`{ "set": "timpani-1", "articulations": [{ "id": "ordinario", "via": "patch", "path":
"Membranophones/Struck Membranophones/Timpani 1 - Hit.sfz", "default": true }, { "id": "roll",
"via": "patch", "path": "Membranophones/Struck Membranophones/Timpani 1 - Roll.sfz" }] }`.
See [../sound-mapping.md](../sound-mapping.md) 2.3.

#### 2.2 Fields

| Field | Level | Type and values | Meaning |
|---|---|---|---|
| `version` | library | `1` or `2` | v1 entries keep `articulations` as a list of strings; v2 uses objects. |
| `id`, `name`, `license` | library | as in v1 | Unchanged meaning (T14, T15). `license` is free text; `null` means unknown. |
| `attribution` | library | string or `null` | Credit text that a licence requires (CC BY libraries), shown with the preset. |
| `source` | library | `{url, revision}` | Where the library comes from, and the revision the profile describes. |
| `detect` | library, shipped profiles | list of relative paths | All of them must exist as regular files for a shipped profile to match a manifest-less folder (2.4). |
| `profile` | library | `{checked: date, method}` | When and how the claims were checked (`code_reading`, `render`); it dates the verdicts. |
| `path` | entry | relative `.sfz` path | One patch, as in v1. Exactly one of `path` and `set`. |
| `set` | entry | slug `[a-z0-9-]` | A multi-patch set of one instrument. Its preset id is `<library>/@<set>` (section 4). |
| `instrument` | entry | orchestration id (S1) or `null` | What the preset plays. `null` for a patch with several instruments, which uses `keymap`. |
| `ensemble` | entry | `{kind: solo, section or section_emulated; players: 1-64 or null}` | Solo or section, with the player count when it is stated. |
| `articulations[]` | entry | `{id, via, key, lo, hi, cc, value, path, label, default}` | Articulation `id` from the vocabulary (S2), or `ordinario` for a patch whose ordinary sound has no better id (it maps to the `ordinario` choice of 5.1). `via` is one of: `patch` (in a `path` entry, the whole patch, without `path`; in a `set`, the member named by `path`); `keyswitch` (`key` 0-127); `velocity` (`lo`-`hi` zone); `cc` (`cc` number and `value`). `default: true` marks the ordinary sound. Optional `mute` and `vibrato` name the passage state a recording carries (layer a's `Mute`, `Vibrato`), so a recorded combination such as `TrumpetStraightMuteSus` is `{id: sustain, mute: generic}` (5.7, spine S2). Optional `attack_ms` overrides the onset that `verify` measures (SS-35) for layer (c)'s attack compensation. |
| `keymap[]` | entry | `{keys, instrument, articulation?, name?}` | Percussion patches with one sound per key. `name` is the pad and key label layer (d) shows (its UI-17, UI-26); without it the label is built from `instrument` and `articulation`. Read by the chain, layer (c) and layer (d), not by the sampler. |
| `key_offset` | entry | semitones, −48 to 48 | Added to incoming notes, for patches keyed away from sounding pitch: mallets mapped an octave or two low, or a piccolo keyed at written pitch (A6). |
| `dynamics` | entry | `{via: velocity}`, `{via: cc, cc: n}` or `{via: fixed, velocity: n}` | What the `dynamics` parameter drives (5.5). `fixed` is for organs whose regions should ignore velocity until Q8 is fixed. |
| `legato` | entry | `{kind: none, overlap or trigger; crossfade_ms; start_offset_ms; gap_ms}` | `overlap` uses the engine's overlap legato (6.3). `trigger` means the patch's own `trigger=first`/`legato` regions do the job (Q5). |
| `in_situ` | entry | `true`, `false` or `null` | Recorded in seating position, so the template keeps pan 0 ([../ensembles.md](../ensembles.md) 2.4). Unverified for VSCO and VCSL, so they ship `null`. |
| `verdict` | entry | `{state: as_is, partly or not; lost: [..]}` | The profile's claim. Section 3.3 combines it with the structural check. |
| `license` | entry | string | Overrides the library licence for single recordings (the VCSL pipe organ note). |

Not in v2, on purpose (DES-03): score `part` (part names live in track names, S3), tuning
(harpsichords at A = 415), and the switch range and sampled keys. The last two are derived from the
file (3.2), and a profile cannot contradict the file.

#### 2.3 Validation (TYP-06, ERR-03)

The reader in `sonara_app::instruments::manifest` accepts versions 1 and 2. Any other version
rejects the manifest as today (`manifest.rs:110-112`). Each entry is validated on its own. Today one
`serde` pass would reject the whole manifest for a type error in a single entry (T23), so v2 reads
`instruments` as a list of `serde_json::Value` and deserializes each entry separately. A failure
rejects only that entry and becomes `LibraryIssue::ManifestEntryRejected` with a reason
(`model.rs:194-202`); its `entry` field holds the `path`, or `@<set>` for a set. The rules:

- `instrument` and every `keymap[].instrument` resolve through `sonara_midi::orchestration::instrument`
  (`crates/sonara-midi/src/orchestration/mod.rs:23`).
- Every articulation id is in the vocabulary or is `ordinario`. At most one `default`. No id
  appears twice. A `mute` or `vibrato` qualifier resolves through layer (a)'s `Mute` or `Vibrato`;
  an unknown value rejects the entry.
- Keys are in 0-127. For a velocity zone, 1 ≤ `lo` ≤ `hi` ≤ 127. `cc` and `value` are in 0-127.
- `via: patch` carries a `path` only in a `set`, and that path must lie inside the library under
  the v1 path rules (no `..`, no absolute path, `.sfz`); in a `path` entry it names the entry's own
  patch.
- `set` slugs are unique, and a `path` entry must not start with `@`.
- The legato times are in range: crossfade 10-500 ms, start offset 0-500 ms, gap 0-100 ms; an
  `attack_ms` override is 0-2000 ms.
- The 1 MiB size cap (`manifest.rs:35`) stays.

**Facts beat claims.** A `keyswitch` key outside the switch range that the loader reads (3.2) keeps
the entry but downgrades its verdict with a note. It is not trusted silently.

#### 2.4 Merge rules

Sources, from lowest to highest precedence. Matching is per entry: by `path` (case-insensitively,
as discovery matches v1 entries to the scan at `discovery.rs:203-215`) or by `set`.

1. **Scan.** Every `.sfz` except `includes/` fragments (3.1). Family is inferred. No instrument
   claim, except an *inferred* one from S1's aliases, which is always shown as inferred.
2. **Structural check** (`sfz::inspect`, 3.2). Facts from the file. They are never overridden.
3. **Shipped profile.** It matches a library whose manifest `id` equals the profile `id`, or a
   manifest-less folder that holds every `detect` path. If two profiles match, neither applies, and
   a `LibraryIssue` says so.
4. **User manifest** in the library root, v1 or v2. A field it sets replaces the shipped field, and
   an explicit `null` clears it. `articulations` and `keymap` replace the shipped lists whole, so
   the user stays in control. Entries only in the shipped profile remain; entries only in the user
   manifest are added.

**Library id.** The user manifest `id` wins over a detected profile `id`, which wins over the
folder slug. A manifest-less VSCO checkout in `VSCO-2-CE` therefore moves from `vsco-2-ce/…` to
`vsco2ce/…`. Ids then survive a folder rename, which closes C4 for the shipped libraries. No stored
data uses catalog ids yet (C6), so the change is free only if it lands before any project stores an
id. The identity half of the shipped profiles (`id` and `detect`) therefore ships with SS-10 in O2;
the mapping half follows in O3 (SS-12).

**An older build** that reads a v2 manifest reports `ManifestInvalid` and scans the library without
it (`discovery.rs:138-147`). That is a degraded catalog, not a failure.

#### 2.5 Shipped profiles

- **What ships, and where.** `crates/sonara-app/profiles/vsco2ce.json` (the 75 patches of branch
  `SFZ`, plus sets for instruments with separate articulation patches) and
  `crates/sonara-app/profiles/vcsl.json` (the 183 patches of tag `v1.2.2-RC`;
  [../sound-mapping.md](../sound-mapping.md) 2.1, 2.3). They are embedded with `include_str!` in a
  new module `sonara_app::instruments::profiles`, so no file is read at run time, and tests check
  them.
- **Licence.** Sonara's own data under the repository licence (0BSD; LIC-01): file names and facts,
  no samples or SFZ text (ADR 0010 decision 3). Both libraries are CC0.
- **Not shipped.** Sonatina (CC Sampling Plus 1.0, Philharmonia-derived parts; A17, spine S7), and
  CC BY libraries such as Salamander and MTG until the owner decides. Users write their own;
  `docs/instrument-catalog.md` gets an authoring section.
- **Verdicts follow the conformance record:** `ViolinEns-KS`, `Flute-KS` and `Clarinet-KS` are
  `partly` until SS-01; `CelloEns-KS` is flagged because its switch keys 84-87 lie inside the
  cello's playable range.

### 3. From catalog preset to instrument model

#### 3.1 Discovery changes (C8)

- `samples::walk` (`samples.rs:56-82`) skips directories named `includes`, ignoring case. The scan
  feeds both the catalog and the `instrument` choices, so both lose Sonatina's 190 fragments at no
  cost.
- A manifest entry that names a path under `includes/` explicitly is still listed.
- A patch with no `<region>` after preprocessing is marked `Invalid` at verify or inspect time,
  never during discovery. That check needs a file read; discovery reads directory entries only.

#### 3.2 Structural check without decoding: `sonara_media::sfz::inspect`

`inspect(path) -> PatchReport` runs the preprocessor and parser, and opens each sample only for its
header (frames and channels, as `Loader::sample` does before decoding, `sfz.rs:414-431`). It does not
decode audio.

It returns the region count, the sampled keys, the velocity-layer boundaries per key, the switch keys
and effective default, the unsupported opcodes (including remapped `trigger`/`loop_mode` values and
skipped headers, N2), the missing sample files (N3) and the decoded bytes computed from the headers
(frames × channels × 4, the loader's own count at `sfz.rs:432-434`). It runs in the background after
discovery, on a worker of `sonara_app::jobs` (`crates/sonara-app/src/jobs.rs`), cheap enough for
whole libraries, unlike `verify` (T15). It feeds the verdicts and the chain's availability rule
(section 7), as C24 asks: verdicts from structural checks, not from the ignored-opcode list alone.
The sampled keys and layer boundaries are the `layers` and `sampled` facts layer (c) reads through
`SoundCaps` (its PF-9). `attack_ms` (onset per articulation) and `max_hold_s` (longest unlooped
sample) need decoded audio, so the deep `verify` (T15) adds them to `PatchFacts` when it runs (SS-35);
until then they are unknown, and layer (c) reports a note as uncompensated rather than guess.

#### 3.3 The catalog's view of a preset

`InstrumentPreset` (`model.rs:134-155`) gains these fields. All are serialized like the existing
ones.

- `instrument: Option<InstrumentClaim { id, source: Declared | Profile | Inferred }>`.
- `ensemble`, `articulations: Vec<ArticulationEntry>` (v1 strings become `{id, via: unknown}`),
  `dynamics`, `legato`, `key_offset`, `in_situ`, `keymap`.
- `kind: Patch | Set { members }`.
- `profile: Shipped | User | None`, `verdict` and the facts of `inspect`. `PatchReport` is a
  `sonara-media` type and that crate has no `serde` dependency, so the app copies it into its own
  serialized `PatchFacts` (no new dependency edge).

**Provenance stays visible (T22).** Today a preset without a manifest claims no articulations, no
licence and only an inferred family, and `contract_instrument_catalog` pins that. A shipped profile
is not the library's manifest, so its claims carry the source `Profile`: the family becomes
`FamilyClaim::Profile`, beside `Declared`, `Inferred` and `Unknown`
(`crates/sonara-app/src/instruments/family.rs:31-38`), and the licence and articulations are
shown as Sonara's profile, not the library's word. The contract test keeps passing (its folder
matches no `detect` list); a new test pins the `Profile` source, and `docs/instrument-catalog.md:13-17`
changes with it (DOC-01).

A track's identity comes from, in order, with the provenance word all four layers show: phase 2's
`part.instrument` (*part*, layer a 14); the device's `instrument_type` when it is not `auto` (*named*,
the user's explicit choice on this track); the preset's claim from its library manifest (*declared*);
from a shipped profile (*profile*); an alias match on the patch path or, for Poly Synth and Drum Kit
tracks, the track name (*inferred*, always labelled); else *none*.

### 4. Preset ids in the `instrument` parameter (amends ADR 0010 decision 4; owner, A5)

- **Choices.** The `instrument` choices are the catalog's available preset ids, including
  `<library>/@<set>` for sets, sorted. `sampler_descriptor` receives them from the app (T7). It is
  never a golden built-in (GLD-01).
- **Resolver.** `MediaProvider` gains an optional second closure, the pattern of its asset
  `Resolver` (T11): `MediaProvider::with_instruments(resolve)`, `resolve: Fn(&str) ->
  Option<InstrumentSource>`. The argument is a stored `instrument` value, either a preset id or a
  legacy `.sfz` path; the closure tries both. `InstrumentSource` (a `sonara-media` type, built by the
  app from catalog and profiles) holds the canonical preset id, the patch paths, `targets`,
  `dynamics`, `legato` and `key_offset`. `load_instrument` loads all patches with one `Loader`
  (6.4), caches by preset id and profile revision, and returns `Arc<SamplerSetup>`;
  `sampler_state` (`graph/mod.rs:1111-1140`) calls it instead of `load_sample_bank`. The engine
  still knows nothing of profiles or the app
  (ARC-11, RT-01): the closure is dependency inversion (DES-06, ARC-08), not a new trait (TRT-04).
- **Backward compatibility, before coercion.** `sampler_params` (`graph/mod.rs:1063-1092`) first
  passes the stored string through the resolver: a `.sfz` path under a configured library maps to
  its preset id; anything else gets today's "not installed" warning and silence (ADR 0010 decision
  5). On the edit path, `sonara-app` normalizes a path given to `set_instrument` the same way before
  the descriptor coerces it (T8), so agents and scripts that pass paths keep working. The project is
  not rewritten on load; the next explicit edit stores the id. The schema does not change, only how
  the value is read (CTR-01: that change is the ADR amendment).
- **Rejected:** a `GraphOptions` field (`graph/mod.rs:86-96`). Every caller of `RenderGraph::new`
  would have to pass it, and the media cache, which already sits in `MediaProvider`, could not
  share it.

### 5. New sampler parameters

#### 5.1 Descriptor

| id | Kind | Values | Default | Automatable | Read by |
|---|---|---|---|---|---|
| `volume`, `pan`, `release_scale` | float | unchanged | unchanged | yes | sampler (today) |
| `instrument` | enum | preset ids (section 4) | first | no | graph, through the resolver |
| `instrument_type` | enum | `auto` + the 85 ids in layer (a)'s score order (`InstrumentType::ALL`) | `auto` | no | `sonara-app` only: identity, chain, checks. The processor ignores its slot. |
| `articulation` | enum | `auto`, `ordinario`, then the vocabulary ids in layer (a)'s append-only `Articulation::ALL` order | `auto` | **yes** | sampler |
| `dynamics` | float | 0.0-1.0 | 1.0 | yes | sampler |

- **Layout.** New parameters go at the end, so slots 0-2 keep their indices (`sampler.rs:39-41`);
  seven parameters stay far below `MAX_PARAMS` (T6). `sampler_descriptor` takes a
  `SamplerChoices { presets, instrument_types, articulations }` built by `sonara-app` from the
  catalog and `sonara-midi`, so `sonara-core` gains no dependency (T17). `articulation` uses a new
  `ep(..).automatable()` beside `not_automatable` (`builtin.rs:110-116`), because `ep` hard-codes
  `false` (T7) and the automation command checks the flag (T8).
- **One list for every device** (see "Deviation from the spine"). A descriptor belongs to the
  plugin, not to a device, so the choices cannot change per preset. Automation points store choice
  indices (T9) while project values store strings (T8). The `articulation` list is therefore frozen
  and append-only, and a snapshot test pins it (TST-05). `instrument_type` is not automatable, so
  only its strings matter: it follows score order, and an id is never renamed or removed.
- **Unknown stored values are soft (T20).** A project saved by a newer build can hold an
  `articulation` or `instrument_type` string this build does not know. `sampler_params` drops it to
  `auto` with a warning, as it already drops a stale `instrument` (`graph/mod.rs:1074-1085`);
  otherwise `param_slots` would fail the whole build (ADR 0010 decision 5).
- **What the choices mean.**
  - `auto` (the default) keeps today's behaviour: the patch's own default and the key-switch notes
    in clips decide. Imported MIDI and existing projects sound the same, apart from the intended
    conformance fixes of 6.1 (Risks).
  - `ordinario` forces the profile's `default` articulation; on a preset without a profile it
    behaves as `auto`.
  - Every other id forces its mapped target, or the nearest one that the chain chose when the
    profile was compiled (5.2).

#### 5.2 The compiled setup

```rust
// Sketch (not implemented): sonara_dsp::bank, built off the real-time thread, read-only on it.
pub struct SamplerSetup {
    pub bank: Arc<SampleBank>,       // all members' regions; `Region::member` tells them apart (6.4)
    pub targets: Box<[Target]>,      // one per `articulation` choice; len == choices
    pub default_target: u16,         // what `ordinario` uses; seeds `last_switch` when Q1 lost it
    pub dynamics: DynamicsMode,      // Velocity | Cc(u8) | Fixed(u8)
    pub legato: Option<LegatoSpec>,  // crossfade, start offset, gap in seconds (frames at prepare)
    pub key_offset: i8,
}
#[derive(Clone, Copy)]
pub struct Target { pub member: u8, pub switch: Option<u8>, pub velocity: Option<(u8, u8)>,
                    pub cc: Option<(u8, u8)>, pub legato: bool }
```

- **Where it lives.** `DeviceState::sample_bank` (T6) is replaced by `sampler:
  Option<Arc<SamplerSetup>>`, an internal API change and no contract. The `Processor` trait is
  untouched (TRT).
- **Every choice is filled off the thread** by step 2 of the chain (the substitutions of
  [../sound-mapping.md](../sound-mapping.md) 4.2 and the refinement fallbacks of
  [../articulation-vocabulary.md](../articulation-vocabulary.md) 3.5), so the render thread never
  searches. `auto` gets no target.
- **Which target is legato.** Only the `legato` choice. With profile `legato.kind = overlap` it
  compiles to the `default` long target with `legato: true` (6.3); with `trigger` to the patch's own
  target, whose `trigger=legato` regions do the work after SS-23; with `none` it falls back through
  step 2. `sustain` and the other long choices stay polyphonic, so a string chord is never collapsed
  to one note by accident.

#### 5.3 A parameter change on the render thread (RT-02..RT-05)

`set_params` is called every block. `store_params` reports a change (`processors/mod.rs:157-164`),
and only then does `apply_params` run:

```rust
// Sketch (not implemented)
let choice = choice_slot(&self.params, ARTICULATION).unwrap_or(0);
if choice != self.articulation {
    self.articulation = choice;
    self.target = self.setup.as_deref().and_then(|s| s.targets.get(usize::from(choice)).copied());
    if let Some(Target { cc: Some((n, v)), .. }) = self.target { self.set_cc(n, v); }
}
self.dynamics.set_target(dynamics_slot(&self.params).clamp(0.0, 1.0)); // SmoothedValue
```

What each target does to the next note-on:

- **Key switch.** Region matching uses `effective_switch = target.switch.or(last_switch)` in place
  of `last_switch`. Clip key-switch notes still update `last_switch`, so going back to `auto`
  restores them. A forced switch works even when Q1 lost the file's range, because `matches_note`
  compares `sw_last` with the key directly (`sampler.rs:484-488`). It cannot un-stack the regions
  that Q11 stripped of their own `sw_last`; those need SS-02.
- **Velocity zone.** The note velocity is remapped into the zone with saturating integer arithmetic
  (`lo + (v − 1)·(hi − lo)/126`), so some dynamic range remains.
- **Patch member.** New notes match only regions with `member == target.member`. Sounding voices
  ring on, because each voice refers to its region (T1), the way an SFZ key switch affects only new
  notes.
- **Controller.** The value goes into the controller table (6.2) that gates and crossfades read.
- **Key offset.** `pitch.checked_add_signed(key_offset)`. An event that leaves 0-127 is dropped.

All of it is a lookup into the `Arc` that `set_state` installed: no allocation, lock, panic or
indexing (`get()`; RT-05); the setup is dropped with the processor off the thread (RT-09). `rest()`
re-applies the current target from `self.params`, so `prepare` and `set_state` cannot lose a forced
articulation. In `auto` mode it seeds `last_switch` with `default_target`'s switch only when the
patch has a switch range but no `sw_default` at all. SS-01 (O2) already recovers VSCO's lost
`sw_default` before this lands in O3, so the seed is a fallback for files that never declare one.
`report()` adds
`articulation` and `dynamics` beside `voices` for the GUI (RT-12).

#### 5.4 `dynamics` (no sentinel, TYP-03)

The default 1.0 means "no change" in `velocity` and `fixed` mode: a render is then identical to
today's. In `cc` mode the default writes 127 (below), so a patch with CC crossfades plays its loudest
layer; that differs from today because SS-20 implements the crossfades (today every crossfaded layer
sounds, Q6/Q7), not because of `dynamics`. Only presets whose profile declares `cc` are affected.

- **`velocity`** (default mode).
  - At note-on, the effective velocity is `max(1, round(v · d))`. It picks the layer and the gain.
  - The voice remembers `d₀ = max(d, 1/127)`.
  - Held notes follow `(d/d₀)²`, capped at 4 (+12 dB) and ramped across the block with the existing
    `sonara_dsp::filters::SmoothedValue` (`crates/sonara-dsp/src/filters/smoothed.rs:33`; DES-01). A
    swell on a held chord therefore changes level; the timbre changes from the next note on.
- **`cc n`.** A change of `dynamics` writes `round(d · 127)` into controller `n`, which drives the
  CC crossfades and gates of 6.2.
  - A clip CC `n` writes the same slot. The last writer wins.
  - At `rest()` the slot starts from the parameter, so the default selects the loudest layer. This
    is documented, and a lane or a lower value chooses another.
- **`fixed v`.** Every note-on uses velocity `v` in place of the note's own, for organ regions before
  Q8; `dynamics` then acts as in `velocity` mode on `v` (`max(1, round(v · d))` at note-on, `(d/d₀)²`
  on held notes), so 1.0 is again today's render.

#### 5.5 Seek, loop wrap and play from stop (C11)

- **The parameter path is correct by construction.** Lanes are evaluated at each block's own beat
  (T9). After a seek or a loop wrap, the first block calls `set_params` with the values of the new
  position; a change is applied at once, an unchanged value was already right. The chase's
  all-notes-off (T13) leaves the target alone, and on play from stop `rest()` re-applies it (5.3).
  Invariant (TST-07): after any chase, the active switch equals the parameter's target when it is
  not `auto`.
- **Timing.** A point inside a block takes effect at the next block start, up to 511 frames late
  (11.6 ms at 44.1 kHz; T9). The lead rule that places points is layer (c)'s, stated once in
  [performance.md](performance.md) section 2: two blocks before the passage's first note-on, never
  before the previous passage's last note-on, else the note is reported as possibly late. Optional
  SS-33 would emit enum lane points inside a block as offset events, for sampler devices only; no
  built-in has an automatable enum, so no golden output can move (GLD-01).
- **The clip key-switch path (`auto`)** keeps C11 until SS-18, a sampler-only chase. The graph
  passes the bank's switch keys to the schedule at build. On a chase, the schedule emits at offset
  0, after the all-notes-off and before the chased notes, a new unit variant `EventKind::KeySwitch`
  (the key in `pitch`, T21) for the last switch note that started before the position, and the last
  values of CC 1, 11 and 64. The buffer sized at build (T21) grows by these four events (RT-02); the
  live-merge scratch is sized from `schedule.capacity()` (`graph/mod.rs:1190-1203`), so it follows.
  Only sampler tracks get them, so Poly Synth and Drum Kit schedules and golden renders are unchanged
  (GLD-01); the synth's exhaustive `match` gains an empty arm, a source change with no output change.
  `KeySwitch` sets `last_switch` without starting regions, which also avoids C17's double use of the
  switch key.

#### 5.6 Live rebuild

A static change of `articulation` or `dynamics` edits the project, so the graph is rebuilt, and
today every sampler is recreated and its notes restart (T10). SS-17 lets `reuse` keep a sampler
whose `SamplerSetup` is the same `Arc`: the device layout stores that `Arc` beside `ir_source`, the
`SAMPLER_ID` exclusion (`graph/mod.rs:776-780`) goes, and the check compares pointers, as it already
does for the convolver's impulse response (`graph/mod.rs:771-775`). The resolver's cache returns the
same `Arc` for an unchanged preset. The new parameters then arrive through `set_params` without a
restart. A different `instrument` still rebuilds the sampler. This changes a consequence recorded in
ADR 0010 (lines 51-52), so the amendment records it.

#### 5.7 Passage states: mute and vibrato (spine S2; layer d's request)

- **Phase 1** reaches a state through the one `articulation` parameter: `con_sordino`, the mute-kind
  ids, `vibrato_on` and `non_vibrato` are choices like any other id, mapped to the recorded patch
  (`FHornMute`, `TrumpetStraightMuteSus`).
- **Combinations.** A recorded pair is an articulation entry with a `mute` or `vibrato` qualifier
  (2.2). One parameter cannot ask for "staccato, muted", so the chain drops the state and says so
  (spine S2).
- **Layer (d)** binds its Mute and Vibrato controls to these `articulation` choices in phase 1 (its
  section 6; choosing one replaces the primary id). **It asks for** two more automatable enums, `mute` (`auto` plus layer a's 9 `Mute` values)
  and `vibrato` (`auto` plus 3 `Vibrato` values). They are buildable exactly like `articulation`:
  frozen lists appended after `dynamics` (slots 7 and 8, far below 48), a target table indexed by
  (articulation, mute, vibrato) compiled off the thread (98 × 10 × 4 `Copy` entries, about 40 KB per
  device), and a missing pair falls back by dropping the state. SS-34 is optional and waits for the
  owner's A16 (flat list or axes), which decides whether the axes exist at all.

### 6. Sampler engine work, in the order of spine S6

Every item touches real-time code. It keeps the lint block (T1), preallocates at `prepare`/`set_state`
(RT-02), uses `get()` and checked or saturating arithmetic (RT-05), and gets an `assert_no_alloc`
test beside the existing sampler tests (`crates/sonara-dsp/tests/sampler.rs:10,87`). QG-13: no
maintained, permissive Rust crate plays SFZ. ADR 0010 decision 1 names the crates considered:
`sofiza` (unmaintained), `oxisynth`, `xsynth-core` and `nidhi` (LGPL/GPL, banned by LIC-01) and
`rustysynth` (SF2 only, no articulations). sfizz needs an RT-10 change, so the work stays in our own
code. Smoothing reuses `SmoothedValue`; the manifest reuses `serde`/`serde_json`.

#### 6.1 Conformance (S6.1, O2)

| Id | Change | Data and real-time design | TST-06 fixture ([../sfz-conformance.md](../sfz-conformance.md) D) |
|---|---|---|---|
| Q1, N4 | The switch keys are the union of `sw_lokey`/`sw_hikey` over every region's merged opcodes, region level included; `sw_default` is the last one parsed in any scope (as sfizz does). | Parser (`sfz.rs:269-280`); `Keyswitches` (`bank.rs:134-143`) holds a `[bool; 128]` key set instead of one `lo..hi` range, so disjoint ranges do not capture playable keys between them. | `sw_default_in_the_first_group_survives_later_groups`; `region_level_switch_range_is_read` |
| Q11 | `<control>` resets only the control scope, never `<global>`, `<master>` or `<group>` (ARIA, sfizz). | `sfz.rs:218-221`. | `control_header_keeps_the_enclosing_global_switch` |
| Q2 | `#include` and `#define` are directives wherever a token starts, not only at the start of a line. | Token-level preprocessor in `read_preprocessed`. | `an_include_after_a_header_on_the_same_line_is_read` |
| Q3, Q4 | `$defines` are expanded in include paths; nested includes resolve against the main file's directory. | The main directory is passed through the recursion. | `defines_expand_in_include_paths`; `nested_includes_resolve_against_the_main_file` |
| Q7 | `lokey`/`hikey`/`key` = −1 means "no key". `on_locc` regions do not play on keys until CC triggers exist (6.2). | `Region::on_key: bool` instead of a magic range (TYP-03). | `minus_one_keys_disable_key_triggering` |
| N1 | Release regions start when the pedal lifts. | `released_under_pedal: [Option<u8>; 128]`, the velocity captured in `note_off`, so a re-press before the pedal lifts does not change it (as it changes `key_velocity`), drained by CC 64 up. | `release_samples_play_when_the_pedal_lifts` |
| N3 | `*silence` is a silent region; other `*` generators and missing files skip that region with a note, and the patch fails only with no region left. | `SampleBank::load_notes: Vec<String>`; a zero-frame sample is already silent (`sampler.rs:337`). | `a_generator_sample_does_not_fail_the_patch` |
| N2 | Unsupported `trigger`/`loop_mode` values and skipped headers (`<curve>`, `<effect>`, `<midi>`, `<sample>`) are reported. | `ignored_opcodes` entries such as `trigger=first`. | `unsupported_values_and_headers_are_reported`; `unsupported_trigger_values_are_reported` |
| C8 | Section 3.1. | `samples::walk`. | `include_fragments_are_not_patches` |
| N5, N6, N7 | Latent; taken when the code is touched (SS-29). | Split a token at `>`; read WAV `smpl` loops; keep multi-word defines. | `a_header_glued_to_its_first_opcode_is_parsed`; `embedded_loop_points_are_the_default` |

#### 6.2 Expression (S6.2, O3)

| Id | Change | Data and real-time design | Fixture |
|---|---|---|---|
| Q6 | `xfin_/xfout_lovel/hivel`, `xf_velcurve` (power default, or gain). | `Region::xf_vel: [u8; 4]`; the gain is multiplied into `voice.gain` at note-on (`sampler.rs:344-345`), so it costs nothing per sample; a zero-gain layer starts no voice. | `velocity_crossfade_scales_each_layer` |
| Q8 | `amp_veltrack`: `gain = 1 − t + t·(v/127)²`, `t` = percent/100; 100 equals today's curve. | `Region::amp_veltrack: f32`. | `amp_veltrack_zero_ignores_velocity` |
| Q7 (CC) | `locc`/`hicc` gates at note-on; `on_locc`/`on_hicc` regions start from `control_change`. | Controller table `cc: [u8; 128]`, reset in `rest()`; `Region::cc_gates: Box<[CcRange]>`, built off the thread and only read on it. | `cc_gate_selects_the_layer`; `on_locc_region_plays_on_pedal_down` |
| CC crossfade | `xfin/xfout_locc/hiccN` (one controller per region; more are reported) and `gain_ccN`. | Per voice, Copy fields `cc_gain` and `cc_step`: the target from the table at block start, ramped linearly over the block. Equal-power curve by default. | `cc_crossfade_follows_the_controller`; `cc_crossfade_is_block_continuous` |
| dynamics | Section 5.4. | `SmoothedValue` for `d`; `d₀` per voice. | `dynamics_one_is_identical_to_today`; `dynamics_scales_held_notes` |
| Q9 | `off_mode=normal` (choke over the region's release) and `off_mode=time` with `off_time`; `fast` stays 10 ms. | `Region::off: OffMode { Fast, Normal, Time(f32) }`. | `off_mode_normal_chokes_over_the_region_release` |

#### 6.3 Legato (S6.3, O3)

- **SFZ triggers (Q5).** `Trigger` gains `First`, `Legato` and `ReleaseKey`. With `held` = the keys
  down outside the switch range, `first` matches when `held == 0` before this key and `legato` when
  `held > 0`; `release_key` starts at note-off and ignores the pedal. With Q9's `off_mode=time`,
  Sonatina's legato patches get their written 1 s overlap ([../sfz-conformance.md](../sfz-conformance.md)
  Q5, Q9). Fixtures: `legato_region_plays_only_while_another_key_is_held`,
  `release_key_plays_on_note_off_even_with_the_pedal_down`.
- **Overlap-crossfade legato on sustain samples.** It is active when the current target has
  `legato: true` and the profile says `kind: overlap`. It is monophonic per track.

  On a note-on of key `k` with another key held (or a note-off at most `gap_ms` earlier) and `k`
  different from the sounding legato key:
  1. Every active, non-releasing voice of the sounding legato key starts a release over
     `crossfade_ms`, with `equal_power = true`. The `CHOKE_RELEASE` choke is skipped for this
     transition. Voices still fading from an earlier transition are shortened to `CHOKE_RELEASE`
     (`start_release` only ever shortens, `sampler.rs:497-503`), so fast runs cannot pile up voices.
     The release regions of a key left this way do not start at its note-off.
  2. The regions for `k` start as usual, but at `offset + start_offset` frames (clamped to the loop
     or end), with attack = `crossfade_ms` and `equal_power = true`. The new note skips the
     sample's attack.
  3. The legato key becomes `k`.

  Otherwise: a note-on with no key held is a normal attack; a repeated pitch retriggers; note-off
  releases only that key's voices (`sampler.rs:264-292`); `AllNotesOff` clears the legato key, so a
  chase starts with an attack.

  The crossfade reuses the voice's `level` ramp (`attack_step`, `release_step`). An
  `equal_power: bool` field in `Voice` maps `level` through `sin(level·π/2)`, so a fade-out and a
  fade-in sum to unit power mid-way. With the shortening of step 1, at most two voices per layer
  sound at full crossfade length (plus 10 ms tails), well inside 64.

  Parameters per profile: `crossfade_ms` 90 (10-500), `start_offset_ms` 60 (0-500) and `gap_ms` 0
  (0-100). These are Estimates; listening test B2 settles them. Fixtures:
  `legato_overlap_crossfades_over_the_profile_time`, `legato_same_pitch_retriggers`,
  `chord_in_legato_mode_is_monophonic`.
- **What this is not.** These are not recorded transitions. Messages say "legato emulated by
  crossfade" (spine headline). Recorded legato comes with M5 (section 9).

#### 6.4 Multi-patch sets and memory (S6.4, O3)

- **One loader per device.** A set's member patches are loaded by one `Loader` (T12), into one
  `SampleBank` whose regions carry `member: u8`, with per-member `Keyswitches`. The loader's
  per-file index (T12) decodes a sample that two members name only once. The 3 GiB cap then counts
  the device as a whole (spine S6.4).
- **Over the cap,** loading stops at the member that would cross 3 GiB (today the loader fails the
  whole call, `sfz.rs:435-442`); its articulations fall back through step 2 with a load note (soft
  failure, ADR 0010 decision 5). `SampleBank` gains a `bytes` field with the total. The cache key is preset id plus profile revision, so a single patch and a set sharing
  files decode them twice; accepted, and `inspect`'s estimate shows it before loading.
- **On the render thread,** the target's `member` (5.3) filters `note_on`; release regions use the
  member stored at note-on (`key_member: [u8; 128]`); `auto` on a set means `default_target`.

### 7. The degradation chain as an application service

`sonara_app::instruments::chain` is a pure function over a catalog snapshot. It does no I/O and
starts no engine (TST-08). It runs on the caller's thread: the planner's `ToolRunner`, a GUI
command, or the compile step of 5.2 inside the resolver on the graph builder thread. It never runs
on the render thread.

```rust
// Sketch (not implemented)
pub struct SoundRequest { pub instrument: InstrumentType, pub ensemble: EnsembleWish,  // layer (a) types
                          pub articulations: Vec<Articulation>, pub span: Option<PitchSpan> }
pub enum SoundSource { Sampler { preset: InstrumentId }, GeneralMidi { program: u8, percussion: bool },
                       Builtin { plugin_id: &'static str, preset: &'static str }, DrumNote { note: u8 }, Silence }
pub struct SoundPlan { pub source: SoundSource, pub articulations: Vec<ArticulationChoice>,
                       pub substitutions: Vec<Substitution>, pub confirm: Option<Question> }
```

**Available** means three things: the catalog says available, `inspect` found regions and no missing
samples, and the merged verdict is not `not`. The fallback tables (family neighbours, the built-in
per id, articulation substitutions) are data read from one place (DES-01): family neighbours and
articulation substitutions are layer (a)'s `orchestration::fallback` (IM-20); the step-1 presets and
the step-6 built-in per id name libraries and plugins, which the metadata must not know, so they are
a table in this module.

| Step | Tried when the earlier steps failed | Result | Message (template) | Confirmation |
|---|---|---|---|---|
| 1 Exact | A preset maps the id, the ensemble and the articulation | sampler preset | none for `as_is`; for `partly`: "{name} ({preset}) plays partly: {losses}." | no |
| 2 Nearest articulation | The articulation is not mapped | another target, or `Emulate(kind)` for layer (c) | "{articulation} is not available for {instrument}; playing {substitute}." | no |
| 3 Solo/section swap | No preset of that ensemble | the other ensemble, then step 2 again | "No {ensemble} {instrument} installed; a {other} plays the part." | no |
| 4 Family neighbour | The id has no preset | a neighbour whose playable range holds the span | "No {instrument} installed; {neighbour} ({preset}) plays the part." | yes, if notes fall outside the range or the role changes |
| 5 General MIDI | Only if a GM engine is registered (section 8) | program or percussion note | "No sampled {instrument}; General MIDI program {n} {name}." | no |
| 6 Built-in | — | Poly Synth preset or Drum Kit note ([../sound-mapping.md](../sound-mapping.md) 4.3) | "No sampled or GM sound for {instrument}; built-in {preset}, a synthetic approximation." | only for the role-changing rows (whip as clap) |
| 7 Silence | — | silence | "No sound source for {instrument}; the track is silent." | no |

Messages return in the `SoundPlan`. The planner turns them into plan warnings and its existing
clarification and confirmation gate (`docs/orchestration.md:22`, "Flow and guarantees"); the GUI
shows them on the track (layer d); render warnings stay as they are (T10). One user preference swaps
steps 2 and 3. Nothing transposes by octaves silently.

### 8. General MIDI fallback (owner, O6; A10, A11)

- **Placement, if accepted.** The processor goes in `sonara-dsp::processors::gm`, because `create()`
  dispatches there (`processors/mod.rs:113-131`); `rustysynth` then sits on the real-time path,
  which the change must call out (QG-10). SoundFont loading goes in `sonara-media::soundfont`, off
  the thread, into a `DeviceState` field. `gm_descriptor(soundfonts)` sits beside
  `sampler_descriptor`, registered by the app only when a user-installed SoundFont is configured,
  never in `builtin_registry()` (GLD-01). Parameters: `soundfont`, `program` (128 GM names, frozen
  order), `percussion`, `volume`, `pan`. No new workspace crate (MOD-09).
- **QG-13 and RT review**, a spike with no product code: (1) the licence is MIT, which `deny.toml`
  allows (`deny.toml:29-39`), and advisories are clean; (2) read the crate's `unsafe` surface;
  (3) MSRV against the pinned toolchain, and release recency; (4) `assert_no_alloc` around
  `note_on`, `note_off` and `render` after warm-up, plus an RTSan run through a `sonara-engine` test
  (RTSan lives in engine, rt and io); (5) determinism (TST-04); (6) SF2 only: a MIT SoundFont
  (MuseScore_General, FluidR3_GM) loads.
- **Decision criteria.** (a) After SS-01..SS-12, recount the ids with no playing free SFZ (36 of 85
  today, [../coverage.md](../coverage.md)); proceed only if the owner judges the remainder worth a
  second engine. (b) Review step 4 passes; a crate that allocates on the render path is rejected
  (RT-02). (c) A listening check of one SoundFont for the gap ids. (d) The cost ADR 0010 decision 6
  names: a second engine and data model. Rejected outright: the Discord SFZ GM bank (stubs, N3,
  `loprog`).

### 9. Plugin hosting (M5) as the upper bound

A hosted instrument cannot read Sonara's `articulation` parameter, so the same table goes to a
different sink.

- **What the profile describes.** A profile kind for plugins is keyed by format, plugin id and
  preset. Its `via` set widens to `keyswitch` (sticky, or momentary with a held note), `cc` (UACC on
  CC 32), `program`, `channel` and `velocity`. `dynamics` becomes CC 1, 11 or 2 events.
- **Where the translation runs.** `sonara-engine::articulation`, a real-time module, runs in the
  event assembly before `processor.process` (`graph/process.rs:240-262`). It reads the compiled
  `targets` (5.2) and writes switch and controller events into the preallocated event buffer when
  the parameter changes. It sends them again after every chase, which is the C11 lesson, and thins
  dynamics to one event per block.
- **Identity and scope.** A hosted device has no `instrument_type`, so it needs S3 phase 2 (a `part`
  object). Hosting itself (sandboxing, AU, VST3) stays M5's decision
  ([../../plugin-hosting.md](../../plugin-hosting.md); ADR 0010 decision 6). Recorded legato and the
  top of the quality range arrive this way; the overlap legato of 6.3 stays the free fallback.

## Contracts and rules

| Rule | How the proposal holds it |
|---|---|
| CTR-01 | No tool, tool schema, `.sonara` schema, IPC or CLI change. New parameters are data of a non-golden descriptor (no golden file mentions `sonara.sampler`). The `instrument` value semantics, including `set_instrument` accepting a path and storing an id, change only through the ADR 0010 amendment (owner, A5). `sonara-library.json` is not on the CTR-01 list, and v1 stays readable. The catalog's documented provenance (T22) is kept by the `Profile` source (3.3). |
| GLD-01 | `builtin_registry()` stays at 12. Poly Synth, Drum Kit and the `orchestral` template are untouched. Engine changes (SS-18, SS-33) apply to sampler devices only, and `golden/plugins/catalog.json` has no automatable enum, so golden renders and the catalog cannot change. The new `EventKind` variant adds an empty arm to the synth's `match` (T21) and is never emitted for it. |
| LIC-01 | Profiles are 0BSD data with no samples. Only CC0 libraries are profiled. Attribution fields exist for CC BY libraries. `rustysynth` (MIT) only after review. SoundFonts are user-installed. |
| HW-01 | Sampler load numbers are measured locally and recorded in `docs/audio-engine.md`, never as CI gates. |
| DOC-01 | Each user-visible item updates `CHANGELOG.md` and `docs/implementation-status.md`. Also `docs/sample-libraries.md` (preset ids, parameters, fixing lines 50-51), `docs/instrument-catalog.md` (v2, authoring, the `Profile` source of lines 13-17), the ADR 0010 amendment (decision 4, and the consequence at lines 51-52 that SS-17 changes), and `docs/dependencies.md` only if a crate is added. |
| QG-03, QG-05 | No `unwrap` in production. New public types document their errors (`LibraryIssue`, `MediaError`). |
| QG-10, QG-13 | No new dependency in O2/O3. SFZ, legato and crossfades are our own code: the crates ADR 0010 considered (section 6) are unmaintained, banned or SF2-only, and the work must meet RT-02/RT-05. `SmoothedValue` and `serde` are reused. `rustysynth` gets the review of section 8. |
| QG-11, PERF-10 | A criterion bench of the sampler (64 voices; CC crossfade and legato on and off) goes with SS-27. Ascension and Night Drive use no sampler, so their baselines cannot move. |
| RT-01 | Profiles, catalog, chain and compile live in `sonara-app`. RT crates get types and closures only. |
| RT-02..RT-05 | Fixed arrays (`cc`, `released_under_pedal`, `key_member`, the switch key set); `Copy` voice fields; setup through `set_state`; the schedule's event buffer sized at build for the chase's extra events; `get()` and checked arithmetic; `assert_no_alloc` per path; RTSan through engine tests. |
| RT-06 | Articulation and dynamics changes are project edits or automation. Nothing waits on the render thread. |
| RT-09 | `SamplerSetup` is an `Arc` installed by `set_state` and dropped with the replaced graph off the thread. |
| RT-10 | No `unsafe` anywhere in this layer. |
| RT-11, TST-04 | No fast-math. The fixed-seed RNG (`sampler.rs:54`) is unchanged. In `velocity` mode the defaults reproduce today's output; the intended conformance changes are listed under Risks. |
| RT-12 | Unusual events (a target member not loaded, a dropped key offset) become report values and load notes, never handling on the thread. |
| ARC-11, MOD-09 | Only existing edges are used (T17): app → midi/media/engine, engine → media/dsp, core receives strings. No new crate. |
| TRT | The `Processor` trait is unchanged. The resolver is a closure, as `Resolver` is, not a single-implementation trait (TRT-04). |
| TYP-01, TYP-03, TYP-06, TYP-07 | Domain types: the existing `InstrumentId` (`model.rs:85-87`), layer (a)'s `InstrumentType` and `Articulation`, a new `ArticulationChoice(u16)`. No magic key values (`on_key`, `Option<u8>` release velocities). Validation sits at the manifest boundary. |
| ERR-03, ERR-04, ERR-06 | Profile problems are typed `LibraryIssue`s that keep their source. Missing samples and unknown stored choices are reported, never swallowed. |

## Alternatives considered

| Alternative | Why not (now) |
|---|---|
| `articulation` choices per preset (the spine's letter) | A descriptor is per plugin (T7), and lanes store indices (T9). It cannot be built. |
| Key-switch notes written by the planner (A3 a) | They clutter the user's MIDI and keep C11. They remain the import path (`auto`). |
| A CC-writing tool and a CC lane (A3 b, A4 a) | CTR-01, versioned. That is phase 2 (spine S4), and it can reuse 6.2 unchanged. |
| A per-library sidecar or a Sonara-side store instead of manifest v2 (A6 b, c) | The manifest travels with the library, and the v1 reader and merge exist. Shipped profiles are the Sonara-side part. |
| Shipped profiles read from `resources/` at run time | This needs bundle plumbing and run-time I/O. Embedding lets tests pin them. |
| sfizz through FFI | It needs an RT-10 change and is an owner decision (ADR 0010 decision 6). |
| Matching libraries by hashing files | Slow and brittle across revisions. `detect` paths plus `revision` are enough. |
| One sampler track per articulation (sound-mapping 3.3 step 1) | It costs tracks and splits one part over several tracks. Sets do the same in one device (spine S6.4). A doubled instrument (flute and piccolo) is another instrument and keeps its own track (A2). |
| A linear legato crossfade only | It dips about 3 dB mid-way on uncorrelated samples. The equal-power flag costs one `sin` on fading voices only. |
| Treating `dynamics` as replacing velocity | It would change every existing render. With a default of 1.0, nothing changes. |
| Overlap legato on every long-note target | It would collapse string chords to one note. Only the `legato` choice is monophonic (5.2). |

## Implementation plan

| Work item | Milestone | Crate / module | Size | Depends on |
|---|---|---|---|---|
| SS-01 Q1 and N4: switch key set (union), last `sw_default` | O2 | `sonara-media::sfz`, `sonara-dsp::{bank, processors::sampler}` | S | — |
| SS-02 Q11: `<control>` resets the control scope only | O2 | `sonara-media::sfz` | S | — |
| SS-03 Q2: directives anywhere in a line | O2 | `sonara-media::sfz` | M | — |
| SS-04 Q3 defines in include paths; Q4 main-file directory | O2 | `sonara-media::sfz` | S | SS-03 |
| SS-05 Q7: `−1` keys; CC-triggered regions not key-triggered | O2 | `sonara-media::sfz`, `sonara-dsp::bank` | S | — |
| SS-06 N3 generators and missing samples per region; N2 reporting | O2 | `sonara-media::sfz`, `sonara-dsp::bank` | M | — |
| SS-07 N1: release samples on pedal up | O2 | `sonara-dsp::processors::sampler` | S | — |
| SS-08 C8: skip `includes/`; region-less patches invalid | O2 | `sonara-app::samples`, `::instruments::discovery` | S | — |
| SS-09 `sfz::inspect` structural report | O2 | `sonara-media::sfz` | M | SS-01..SS-06 |
| SS-10 Preset-id choices, instrument resolver, path compatibility; shipped-profile identity (`id`, `detect`) so library ids are final before any project stores one (2.4) | O2 | `sonara-core::plugins::builtin`, `sonara-media::provider`, `sonara-engine::graph`, `sonara-app::{samples, instruments::profiles}` | M | SS-08; owner (A5) |
| SS-11 Manifest v2 reader (per-entry parsing), validation, merge | O3 | `sonara-app::instruments::{manifest, profiles}` | M | SS-10; IM-1, IM-6, IM-15 |
| SS-12 Shipped profiles VSCO 2 CE and VCSL v1.2.2-RC | O3 | `crates/sonara-app/profiles/`, `::instruments::profiles` | L | SS-11 |
| SS-13 Catalog mapping: claims with the `Profile` source, ensemble, articulations, verdict, `PatchFacts` | O3 | `sonara-app::instruments::{model, family, discovery}` | M | SS-09, SS-11; IM-18 (`Inferred` claims) |
| SS-14 Parameters `instrument_type`, `articulation`, `dynamics`; `SamplerChoices` (absorbs layer a's IM-17); unknown stored choices fall back to `auto` | O3 | `sonara-core::plugins::builtin`, `sonara-app::samples`, `sonara-engine::graph` | S | IM-1, IM-6 (`InstrumentType::ALL`, `Articulation::ALL`) |
| SS-15 `SamplerSetup` in `DeviceState`; compile with step-2 fallbacks | O3 | `sonara-dsp::{bank, processor}`, `sonara-app::instruments::compile`, `sonara-engine::graph` | M | SS-10, SS-13, SS-14, SS-26 |
| SS-16 Sampler applies targets, key offset; `rest()` re-applies | O3 | `sonara-dsp::processors::sampler` | M | SS-15 |
| SS-17 Live rebuild reuses an unchanged sampler | O3 | `sonara-engine::graph` | S | SS-15 |
| SS-18 Sampler-only chase: `KeySwitch` event, CC 1/11/64, event buffer +4 | O3 | `sonara-engine::{schedule, graph}`, `sonara-dsp::{processor, processors::{sampler, synth}}` | M | SS-01 |
| SS-19 Q6 velocity crossfades; Q8 `amp_veltrack` | O3 | `sonara-media::sfz`, `sonara-dsp::{bank, sampler}` | S | — |
| SS-20 Controller table, gates, CC crossfades, `gain_cc` | O3 | same | M | SS-05 |
| SS-21 `dynamics` drive (velocity, cc, fixed) | O3 | `sonara-dsp::processors::sampler` | M | SS-14, SS-15, SS-20 |
| SS-22 Q9 `off_mode` normal and time | O3 | `sfz`, `bank`, `sampler` | S | — |
| SS-23 Q5 `first`, `legato`, `release_key` | O3 | `sfz`, `bank`, `sampler` | S | SS-22 |
| SS-24 Overlap-crossfade legato | O3 | `sonara-dsp::processors::sampler` | M | SS-16, SS-23 |
| SS-25 Multi-patch sets, one loader per device, memory cap | O3 | `sonara-app::instruments`, `sonara-media::{sfz, provider}`, `sonara-dsp::{bank, sampler}` | L | SS-15, SS-16 |
| SS-26 Degradation chain service; the step-1 and step-6 tables | O3 | `sonara-app::instruments::chain` | M | SS-13; IM-20 |
| SS-27 Sampler bench, `assert_no_alloc` and RTSan coverage | O3 | `sonara-dsp` benches and tests, `sonara-engine` tests | S | SS-16..SS-25 |
| SS-28 Docs: ADR 0010 amendment, sample-libraries, instrument-catalog, CHANGELOG, status | O2, O3 | `docs/` | S | each item |
| SS-29 N5, N6, N7 (latent) | O3 | `sonara-media::sfz` | S | — |
| SS-30 GM review spike and decision record | O6 | none (spike) | S | SS-01..SS-12 |
| SS-31 `sonara.gm` engine, if accepted | O6 | `sonara-dsp::processors::gm`, `sonara-media::soundfont`, `sonara-core::plugins::builtin`, `sonara-app::samples` | L | SS-30; owner (A10) |
| SS-32 Articulation translator for hosted instruments | O6 (M5) | `sonara-engine::articulation` | M | M5 host |
| SS-33 Event-accurate enum automation for samplers (optional) | O3 | `sonara-engine::graph::process`, `sonara-dsp::processor` | M | SS-16 |
| SS-34 `mute` and `vibrato` parameters (optional, 5.7) | O3 | `sonara-core::plugins::builtin`, `sonara-app::instruments::compile`, `sonara-dsp::processors::sampler` | M | SS-15, SS-16; owner (A16) |
| SS-35 Decoded facts at `verify`: onset per articulation (`attack_ms`) and longest unlooped sample (`max_hold_s`) into `PatchFacts`; the profile's `attack_ms` override (absorbs layer c's PF-8) | O3 | `sonara-media::sfz` (`PatchReport`), `sonara-app::instruments::{verify, model}` | M | SS-09, SS-13 |

## Tests

| TST id | What | Tests |
|---|---|---|
| TST-06 | Each conformance row gets the fixture named in 6.1 and 6.2: hand-written in the real library's header layout, copying no library text (LIC-01). | Pin-current tests proposed in [../sfz-conformance.md](../sfz-conformance.md) D, not in the tree today, are added first: `a_seek_while_playing_keeps_the_last_switch` and `all_notes_off_keeps_the_switch` pin today's `auto` behaviour and flip with SS-18; `renaming_a_library_without_manifest_changes_its_ids` stays for undetected libraries, beside the new `a_detected_library_keeps_its_profile_id`. The existing loader assertion that `locc*` is ignored (`crates/sonara-media/tests/sfz.rs:115`) changes with SS-20. |
| TST-01 | Units | The articulation table (switch, velocity zone, member, controller; `member_target_switches_patch_for_new_notes_only`); Q6 and Q8 gains; the dynamics ratio; the legato state machine; the equal-power mapping; the chain step by step over fake catalogs. |
| TST-10 | Oracles | The SFZ formulas (xfin, `amp_veltrack`); `verify_measures_onset_and_longest_unlooped_sample` on generated samples of known onset and length (SS-35). `dynamics_one_is_identical_to_today`: today's sampler output is the oracle for the new defaults in `velocity` mode. Equal-power sums to unit power at the midpoint. |
| TST-09 | Properties (proptest) | Articulation and dynamics changes at block boundaries render the same in any block split (state continuity); the manifest reader never panics on arbitrary JSON and rejects entries one by one; compiled `targets` cover every choice index. |
| TST-07 | Invariants | After any chase the active switch equals the target when it is not `auto` (5.5); `targets.len()` equals the choice count; a legato run of notes faster than `crossfade_ms` never holds more than two voices per layer beyond 10 ms tails (`legato_run_keeps_two_voices_per_layer`); a chase never drops an event (`chase_events_fit_the_buffer`); set memory stays within 3 GiB per device; `auto` never forces a switch. |
| TST-03 | Failure paths, each a `LibraryIssue` or a warning | An unknown orchestration or articulation id, a key above 127, two defaults, a `patch` member outside the library; one malformed entry (the others are kept); a v3 manifest; a missing set member; a preset id not installed (silence plus the existing warning); an unknown stored `articulation` or `instrument_type` (`auto` plus a warning, not a failed build); an old absolute path (resolved); a member over the memory cap. |
| TST-05 | Public API | A snapshot of the `articulation` choice order (append-only) and a check that every released `instrument_type` id still resolves; the sampler and GM descriptors stay out of `builtin_registry()` (`creates_every_ported_plugin_and_nothing_else`, `processors/mod.rs:182-196`); v1 manifests parse unchanged; `contract_instrument_catalog` passes unchanged, and `a_detected_library_claims_with_the_profile_source` pins 3.3. |
| TST-02 | Integration | `crates/sonara-engine/tests/sampler.rs` (exists, generated fixtures) renders a VSCO-layout fixture set with `articulation` and `dynamics` lanes across a seek and a loop wrap (`a_seek_applies_the_automated_articulation`), and a clip key switch across a seek (`a_chase_restores_the_clip_key_switch`, SS-18). |
| TST-08 | Isolation | Catalog, profiles and chain on temp directories and fake loaders; shipped profiles tested without the libraries (they parse; every id and vocabulary entry exists); a local-only `#[ignore]` test against a real checkout named by an environment variable, never a CI gate (HW-01 spirit). |
| RT | No allocation, no blocking | `assert_no_alloc` around `set_params`/`process` for each new path; RTSan through the `sonara-engine` sampler test. GLD-01: the golden suites run unchanged. |

## Risks

| Risk | Mitigation |
|---|---|
| The overlap legato sounds poor | Per-profile times; listening test B2; messages call it emulated; M5 for recorded legato. |
| Switches land up to 511 frames late | The planner's lead rule (5.5); the GUI lane shows block granularity; SS-33 if it matters. |
| An unordered or renamed vocabulary breaks automation | The frozen, append-only order and its snapshot test; owner choices A12, A13 and A16 settled before the first release. |
| RAM: sets multiply decoded memory, and there is no streaming | Per-device cap; `inspect` estimates bytes before loading; documented in `docs/sample-libraries.md` "Memory". |
| Profiles drift from library revisions | `detect` and `revision`; missing paths show as `missing`; verdicts are dated. |
| Q1 and Q11 fixes change how existing projects with VSCO or Sonatina key-switch patches sound; SS-19/SS-20 change patches with velocity or CC crossfades (crossfaded layers instead of every layer at full gain) | Intended (silence, stacking and doubled layers end); CHANGELOG entry (DOC-01). |
| Per-voice CC ramps and `sin` cost render time | The bench in SS-27 (QG-11); equal-power only on fading voices. |
| A parser change opens new failure modes on untrusted files | Include depth and size caps stay; parser properties (TST-09); soft failure. |

## Owner decisions

1. **Preset ids in `instrument`**, amending decision 4 of
   [ADR 0010](../../decisions/0010-sample-based-orchestral-instruments.md)
   ([A5](../open-questions.md#a5-the-samplers-instrument-parameter-absolute-paths-or-preset-ids)).
   Recommended.
2. **Mapping data as `sonara-library.json` v2**, with shipped VSCO 2 CE and VCSL profiles embedded in
   `sonara-app`
   ([A6](../open-questions.md#a6-library-mapping-data-where-it-lives-what-it-holds-whether-sonara-ships-it)).
   Recommended.
3. **No shipped Sonatina profile**; documented as a user install
   ([A17](../open-questions.md#a17-licences-recommending-and-supporting-sonatina--iowa--or-philharmonia-derived-libraries),
   spine S7). Recommended.
4. **Articulations through an automatable parameter, not clip key-switch notes**
   ([A3](../open-questions.md#a3-how-articulations-reach-a-library) option c, chosen by spine S4
   although A3 recommends option a); sets instead of one track per articulation (spine S6.4), while a
   doubled instrument keeps its own track
   ([A2](../open-questions.md#a2-a-doubled-instrument-its-own-track-or-a-switch)).
5. **`articulation` default `auto`, and what `ordinario` means**
   ([A14](../open-questions.md#a14-articulation-vocabulary-the-ordinary-note-and-ids-used-in-two-meanings);
   spine S2 owner point 2). Recommended: `auto` default, `ordinario` = the profile default.
6. **Which vocabulary ids enter the frozen enum** before the first release
   ([A12](../open-questions.md#a12-articulation-vocabulary-the-merges-and-col_legno) `col_legno`,
   [A13](../open-questions.md#a13-articulation-vocabulary-the-34-invented-ids-and-the-candidate-merges)
   invented ids,
   [A16](../open-questions.md#a16-articulation-vocabulary-flat-list-or-separate-axes-mutes-beaters-vowels-shapes)
   flat list or axes). A16 also decides SS-34, the `mute` and `vibrato` parameters (5.7).
7. **`dynamics` as an automatable sampler parameter** with the semantics of 5.4
   ([A4](../open-questions.md#a4-dynamics-an-automatable-sampler-parameter-or-a-cc-writing-tool)
   option b).
8. **Overlap-crossfade legato first**, recorded legato through M5
   ([A9](../open-questions.md#a9-sampler-features-legato-crossfades-velocity-and-pitch-bend);
   defaults from [B2](../open-questions.md#b-human-checks)).
9. **Follow ARIA and sfizz where the SFZ spec is silent**: Q1, Q2, Q11
   ([A8](../open-questions.md#a8-fix-the-sfz-parser-and-sampler-divergences-or-work-around-them)).
   Recommended.
10. **General MIDI engine** by the criteria of section 8, and the GM number policy
    ([A10](../open-questions.md#a10-a-general-midi-fallback-engine),
    [A11](../open-questions.md#a11-gm-numbers-for-ids-without-a-gm1-program-and-the-drum-kit-map)).
    Decide in O6.
11. **Plugin hosting (M5)** and sfizz stay separate decisions (ADR 0010 decision 6;
    [../../plugin-hosting.md](../../plugin-hosting.md)).

## Deviation from the spine

1. **S4, the `articulation` choices (now folded into S4).** The spine's earlier wording said the choices are the preset's mapped ids plus
   `ordinario`. This design uses one fixed list for every device: `auto`, `ordinario`, then the
   vocabulary in a frozen order; the preset's mapped subset plays exactly, every other choice
   resolves through step 2. Reason: a `ParamSpec` belongs to the plugin descriptor, not to a device
   (T7); lanes store choice indices (T9) and projects store strings (T8), so per-preset lists cannot
   be built and would reinterpret lanes. Rules: CTR-01 (no schema change), TST-05 (a stable public
   list), PRI-04.
2. **S4, the added `auto` default.** Not in the spine; it keeps today's sound for existing projects
   and imported key-switch MIDI, apart from the intended conformance fixes of 6.1 (CTR-01, no silent
   behaviour change). The owner decides it with A14.
3. **S4, the timing statement (now folded into S4).** The spine's earlier wording said a switch "lands at the start of the block that holds
   its beat". The code evaluates lanes at the block's start beat (T9), so a point inside a block
   takes effect at the next block, up to 511 frames later. Layer (c) owns the lead rule (its section
   2); this layer adds the optional SS-33. This corrects a fact; the decision stands.
4. **S7, additions only.** Added fields: `set` (needed by S6.4), `keymap` (with `name`, layer d's
   pad and key labels), `attack_ms` (layer c's override), `key_offset`, `detect`, `source` and `attribution`, the `mute`/`vibrato` qualifiers
   (S2's recorded combinations), plus `dynamics.via = fixed`. They refine S7 and contradict nothing.
5. **S4, more parameters (optional).** S4 names two phase-1 carriers. SS-34 would add `mute` and
   `vibrato` (5.7) at layer (d)'s request, built the same way and with no contract change (the
   sampler is not golden; CTR-01, GLD-01). It waits for A16, so the spine's two-parameter phase 1
   stands until the owner decides.

## Review

Adversarial review against the checkout (`crates/` and `apps/` equal `a212544`), the conformance
record and the spine. Changes, one per line:

1. Retitled to "Sound source (layer b)", matching the sibling layers.
2. Added T20-T23 (stale enum values fail the build; `EventKind`, its matches and the event buffer; the catalog's provenance contract; the one-pass manifest parse), all read in the code.
3. 2.2: documented the example's `profile` field; added keymap `name` (layer d's `key_names`) and the `mute`/`vibrato` qualifiers (S2 combinations).
4. 2.3: per-entry validation was infeasible with today's single `serde` pass (T23); v2 now parses entries one by one.
5. 2.4: wrong citation fixed; `manifest.rs:118-121` is the duplicate check, scan matching is `discovery.rs:203-215`.
6. 2.4 and plan: the library-id change had to precede stored ids, but detection sat in O3 after SS-10 (O2); profile identity moved into SS-10.
7. 3.2: "only `stat`s samples" cannot give decoded bytes; `inspect` reads sample headers (`sfz.rs:414-434`), returns layer boundaries for layer (c)'s PF-8, and runs on `sonara_app::jobs`.
8. 3.3: `PatchReport` cannot be serialized as is (`sonara-media` has no `serde`); the app copies it into `PatchFacts`.
9. 3.3: shipped claims would have broken the catalog's documented provenance (T22); added the `Shipped`/`Profile` source, the contract-test note and the doc update.
10. 4: the resolver now states that it takes a preset id or a legacy path.
11. 5.1: an unknown stored `articulation` or `instrument_type` would fail the graph build (T20); it now falls back to `auto` with a warning.
12. 5.1: `instrument_type` follows layer (a)'s score order and is not frozen; `articulation` follows the append-only `Articulation::ALL`; TST-05 adjusted.
13. 5.1: "existing projects sound the same" qualified; `ordinario` without a profile defined as `auto`.
14. 5.2: stated which target is legato (only the `legato` choice), so overlap legato is reachable and chords stay polyphonic.
15. 5.3: temporal error fixed; SS-01 (O2) recovers VSCO's `sw_default` before SS-16 (O3), so the seed is a fallback.
16. 5.4: "identical to today in every mode" was false for `cc` mode; narrowed, and the change attributed to SS-20.
17. 5.5: `EventKind::KeySwitch { key }` became a unit variant with the key in `pitch`; event order, the +4 buffer slots (RT-02) and the synth's match arm (GLD-01) stated.
18. 5.6: SS-17 now names the layout change, the removed `SAMPLER_ID` exclusion and the ADR 0010 consequence it changes.
19. 5.7 (new): answers layer (d)'s `mute`/`vibrato` request and S2's combinations; optional SS-34, owner A16, deviation 5.
20. 6: QG-13 now names the crates considered (ADR 0010 decision 1).
21. 6.1 Q1: a union of disjoint switch ranges does not fit one `lo..hi`; switch keys become a key set.
22. 6.1 N1: a re-press before pedal-up overwrote the stored velocity; it is now captured at note-off.
23. 6.3: "two voices per layer" failed for runs faster than the crossfade; earlier fading voices are now shortened, and a named test pins it.
24. 6.4: the dedup example was not a set; reworded, and today's whole-call failure over the cap cited.
25. 7: sketch types aligned with layer (a) (`InstrumentType`, `Articulation`).
26. Contracts, plan, tests, risks and owner decisions updated for the above; SS-01, SS-10, SS-11, SS-13, SS-14, SS-18 and SS-21 corrected, SS-34 added.
27. Tests: the pin-current tests of [../sfz-conformance.md](../sfz-conformance.md) D were described as existing; none is in the tree, so they are now "added first".
28. Owner decisions: A ids linked; A2 and A3 added (spine S4 chose A3 option c against A3's own recommendation).
29. 2.3: the new `mute`/`vibrato` qualifiers are validated against layer (a)'s enums (TYP-06).
30. Deviation 2: qualified like 5.1 (conformance fixes change some existing projects).
31. 5.5: confirmed the live-merge scratch is sized from the schedule (`graph/mod.rs:1190-1203`), so the +4 needs no second change.

**Verified, no change.** T1-T19 and every other file:line in the proposal hold at `a212544`. No
golden file names `sonara.sampler`; `golden/plugins/catalog.json` has no automatable enum. The
proposal changes no tool, schema, IPC or CLI (CTR-01), uses only allowed crate edges (ARC-11, RT-01),
adds no `unsafe` (RT-10) and no crate (MOD-09), and ships no samples (LIC-01, ADR 0010). The
inventory facts of 2.1 and 2.5 match [../sound-mapping.md](../sound-mapping.md) 2.1 and 2.3.
Deviations 1-4 were correctly declared.

**Could not fix.**

- Length: about 890 lines, above the 750 aim, because of the review and the added facts; no
  load-bearing content was cut.
- Owner-only: A5 (the amendment), A14, A16 (vocabulary shape and SS-34), A10/A11 (GM), A17.
- Render-only: the legato defaults (B2) and the audibility of the `ampeg_dynamic` loss.
- Whether project loading outside the graph validates sampler enum values was not checked (C3
  left it open); only the graph path is made soft.

**Cross-layer review of 2026-10-30** against layers a, c and d (one line per change):

32. 2.2, 3.2, SS-35 (new): layer c's `attack_ms` and `max_hold_s` had no owner and no profile field; `verify` now measures them into `PatchFacts` (SS-35, absorbing layer c's PF-8), and entries gain an optional `attack_ms` override (2.3 range 0-2000 ms).
33. 3.3: the claim source was called both `Shipped` and `Profile`; it is `Profile` everywhere (text, SS-13, CTR-01 and DOC-01 rows, test name); `profile: Shipped | User | None` still names where a preset's record came from.
34. 3.3: track identity precedence and its words (*part*, *named*, *declared*, *profile*, *inferred*, *none*) now match layers a 14, c 6 and d principle 4.
35. 5.4: `dynamics` in `fixed` mode was unspecified (layer c's `SoundCaps` lacked it); it now acts on the fixed velocity as in `velocity` mode.
36. 5.5 and Deviation 3: the lead rule conflicted with layer c ("512 frames … or 1/32 beat early" against "two blocks, after the previous passage's last note-on"); layer c owns it, this layer states the engine fact (11.6 ms, not "about 12 ms").
37. 7 and SS-26: the chain read fallback tables "in `sonara_midi::orchestration`" that layer a did not define and could not hold (plugin names); neighbours and substitutions are layer a's IM-20, step-1 and step-6 tables stay in `chain`.
38. 2.2, 5.7 and Deviation 4: layer d's `key_names` request is answered by `keymap[].name`, and its Mute and Vibrato controls bind to `articulation` choices in phase 1.
39. Alternatives and owner decision 4: A2 is about a doubled instrument, not articulation tracks; the citation now says so and sets cite spine S6.4.
40. Plan: SS-11, SS-13, SS-14 and SS-26 depend on named layer-a items (IM-1, IM-6, IM-15, IM-18, IM-20); SS-14 absorbs IM-17; layer c's work items are cited as `PF-n`.
41. "Could not fix": the `SamplerChoices` and `attack_ms`/`max_hold_s` items are resolved above and removed.

