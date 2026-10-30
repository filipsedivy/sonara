# Instrument-aware user interface (layer d)

_Status: proposed (2026-10-30). Part of the orchestra design; decisions in [../../orchestra-design.md](../../orchestra-design.md)._

## Summary

Today the desktop GUI knows plugins, not instruments: the Browser lists plugin presets, a device
card is generated from parameter descriptors, the piano roll knows only one special case (GM drum
names on `sonara.drums` tracks), and nothing reads the orchestration metadata, the instrument
catalog or the quality findings. This layer turns spine decision S11 into an information
architecture and behaviour:

- **Library** (a new Browser tab): family → instrument → sources, each source with its
  availability (ready, partly, not installed, invalid, built-in approximation), its licence and,
  when missing, how to install it. Search matches the multilingual aliases of S1.
- **Track header and Inspector** show the instrument identity (instrument, part, solo or section,
  where the identity comes from), the articulation at the playhead and range warnings.
- **Family panels** on the sampler's device card: strings, keyboards, percussion, woodwinds and
  brass, voices, harp, organ. Every control is a *binding* to something that already exists or
  that S3/S4 adds (a sampler parameter, clip CC events, a track operation) or is read-only derived
  data. A control the source cannot honour is not drawn as a live control; it is listed with the
  reason.
- **Piano roll**: a lane selector (Velocity, Articulation, Dynamics, Pedal CC64, Expression CC11;
  each only where the track's device reads it), range shading from the model, a written/sounding
  toggle that changes labels only, labelled key-switch keys and percussion names.
- **Ensembles**: New Project offers ensembles beside the unchanged templates; "Add Ensemble…"
  applies one to an open project as one undo step, in score order with family folders.

Everything the page shows is computed in `sonara-app` from `sonara_midi::orchestration` (S1), the
catalog and the library profiles (S7); the desktop reaches them only through new GUI-only Tauri
commands and `sonara_app::edit::Edit` variants (ARC-11). No agent tool, schema, IPC or CLI
changes in phases O1-O5 (CTR-01). Every new canvas element is measured against ADR 0009's budgets.

## Today (verified)

Checked at `main` `a212544` (Release 0.1.11). Since then 0.1.12 (`558f898`, this branch's base) changed only `session.rs`, `undo.rs`, `edit.rs` and `composer.rs` (the undo groups); the `edit.rs`
line numbers below are those of 0.1.12.

**Browser.** The Devices tab is a tree "Instruments" → plugin → presets and "Effects" → category
(`crates/sonara-app/src/browser.rs:71-127`, rendered by
`apps/sonara-desktop/src/lib/browser/Browser.svelte:247-300`). Activating an instrument or preset
sends one `create_track` edit named after the preset (`browser/tree.ts:92-110`). Search is a
substring match on the item text (`tree.ts:38-59`). There is no preview playback
(`Browser.svelte:23-29`). The instrument catalog (`crates/sonara-app/src/instruments/`) has
availability states `Available | Missing | Invalid { reason }` (`instruments/model.rs:110-125`) and
presets with id, family claim, articulations and licence (`model.rs:134-155`), but no GUI uses it
([../sfz-conformance.md](../sfz-conformance.md) C6; `docs/implementation-status.md`, row "Instrument
catalog"). The preset id type `InstrumentId` (`model.rs:85-102`) has only a `pub(crate)`
constructor from a library id and a relative path; nothing parses one from text.

**Prototype-recorded GUI oracles (GLD-01).** `apps/sonara-desktop/tests/fixtures/dialogs-v1.json`
was written by the frozen prototype (`crates/sonara-app/tests/gui_dialogs.rs:1-13`). It pins New
Project's template list and form fields (`gui_dialogs.rs:264-305`), the bytes `settings.json` gets
in five Settings cases (`gui_dialogs.rs:107-138`), the Devices tree (`:887`), and the Inspector rows,
including the "Instrument" text "name · preset" (`tests/unit/inspector.test.ts:171`).
`tests/fixtures/device-editors-v1.json` pins the device cards of the 12 built-ins; the sampler is
not in it.

**Sample libraries in the GUI.** The Settings dialog has no library field
(`apps/sonara-desktop/src/lib/dialogs/Settings.svelte`, no `sample`/`librar` match); a library is
added by editing `settings.json` and restarting (`docs/sample-libraries.md:13-25`). The page treats
the plugin registry as fixed while the app runs (`devices/catalog.ts:1-4`). `open_link` opens only
the About credits and the author's address (`crates/sonara-app/src/about.rs:140-146`); any other
link is `PERMISSION_DENIED` (`src-tauri/src/dialogs.rs:434-440`). The app bundle carries no
`docs/` pages (`scripts/bundle-macos.sh` copies none), so the GUI cannot open a repository doc.

**Device editor.** Cards are generic: parameters grouped by id prefix (`devices/model.ts:85-104`),
a knob, toggle or combo per descriptor (`model.ts:154-171`, `devices/ParamControl.svelte:49-83`).
The one plugin-specific view is the parametric EQ's curve (`devices/DeviceCard.svelte:205-206`).
The sampler's parameters are `volume`, `pan`, `release_scale` and the enum `instrument` whose
choices are `.sfz` paths, not automatable (`crates/sonara-core/src/plugins/builtin.rs:755-798`,
`ep` at `builtin.rs:84-91`), so the card shows a combo of file paths.

**Track headers.** DOM rows beside the WebGL2 arrangement (`timeline/arrangement.ts:1`), one per
track, all rendered and moved by `translateY` (`timeline/Headers.svelte:301-321`, controls layer
`:365`); a detail line "MIDI · preset · dB · pan · n fx"
(`timeline/headers.ts:29-44`, pinned by `tests/unit/headers.test.ts:11-27`) only on rows of 44 px
or more (`headers.ts:17`; heights S 34, M 56, L 88, XL 132 in `state/timeline.ts:13`); M, S, R
buttons; a context menu with Rename…, Colour…, Automatic Colour, Show Devices, Add MIDI Clip at
Playhead (instrument tracks), Import Audio… (audio tracks), Delete Track
(`Headers.svelte:247-268`). For VoiceOver the rows are a list box
whose options read "name, dB, muted, solo, armed" (`Headers.svelte:34-38`, `a11y/labels.ts:46-54`).
Folders indent by depth; there is no collapse (`timeline/layout.ts:1-4`: one row per track).

**Inspector.** The Track group shows name, volume, pan, output, an "Instrument" text "name ·
preset", clips and effects (`inspector/Inspector.svelte:182-256`, `inspector/inspector.ts:73-78`).

**Piano roll.** Four Canvas2D surfaces: grid, keys, ruler, lane (`pianoroll/paint.ts:99-106`,
`158-174`), virtualised and batched by fill colour (`paint.ts:1-9`, `fillKey` at `paint.ts:63-65`,
whose `playable` bit means "inside the content the clip plays", `model.ts:234-243`, not a range).
Black-key rows are shaded with one `fillRect` each (`paint.ts:208-215`). Drum mode is on only when
the track's instrument is `sonara.drums` (`pianoroll/PianoRoll.svelte:64-67`); the names come from
`drum_map` (`src-tauri/src/commands.rs:670-677`, `crates/sonara-app/src/catalog.rs:158-179`) and widen
the keyboard to 132 px (`paint.ts:32-33`, `544-547`). The lane is the velocity lane only, 72 px
(`paint.ts:35`, `paint.ts:478-530`). Keys: Delete, ⌘D, ⌘A, Escape, arrows nudge, ⇧↑/↓ an octave
(`pianoroll/gestures.ts:354-370`). The notes have an ARIA list box of at most 256 options
(`pianoroll/noteList.ts:13-14`), named by pitch or GM drum name (`noteList.ts:40-43`).

**No automation or CC at the GUI boundary.** The page's `Edit` union has no automation or
controller edit (`src/lib/types.ts:431-514`), mirroring `sonara_app::edit::Edit`
(`crates/sonara-app/src/edit.rs:207-529`). `Edit::CreateTrack` has no parameter values, index or
parent (`edit.rs:211-220`), though core `CreateTrack` has `index` and `parent_id`
(`crates/sonara-core/src/commands/track.rs:93,103`); `Edit::SetInstrument` takes no preset or
parameters (`types.ts:475`). Edits that need several commands already build one `Transaction`
(`edit.rs:842`, `1015`, `1052`; `crates/sonara-core/src/commands/mod.rs:376-414`). `TrackView` has
no automation lanes and `MidiClipView` no controller events (`crates/sonara-app/src/view.rs:186-217`,
`339-358`; `types.ts:244-271`); `docs/implementation-status.md` (row "CC automation and pitch bend
in clips") says the piano roll does not edit CC lanes. The core has the commands:

- `SetAutomation` (`crates/sonara-core/src/commands/automation.rs:107-125`), curves `linear | hold
  | smooth` (`model/automation.rs:18-27`). It refuses a non-automatable parameter with
  `UNSUPPORTED` (`automation.rs:69-73`), more than 20,000 points with `LIMIT_EXCEEDED` (`:87-91`)
  and a value out of range with `INVALID_ARGUMENT` (`:93-101`); an enum's range is the choice
  indices of the plugin's descriptor (`:68`, `:78`).
- `SetControlEvents` (`commands/clip.rs:942-987`, used by recording in
  `crates/sonara-app/src/transport/takes.rs:316`) clears in `[start, end)` only the (kind, number)
  pairs present in its new events (`clip.rs:966-973`), so it cannot erase a range by sending none.
- `SetTrackParent` (`track.rs:653-683`).

Device parameters are sampled once per block, an enum by `round(clamp(v, 0, choices − 1))`
(`crates/sonara-engine/src/automation.rs:12-15`).

**Instruments and MIDI controllers.** The sampler reacts to CC 7, 11 and 64 only
(`crates/sonara-dsp/src/processors/sampler.rs:204-225`; C15); CC66 (sostenuto) and CC67 (una corda)
do nothing. Release samples are not started for a key released under the pedal
([../sfz-conformance.md](../sfz-conformance.md) N1). The Poly Synth reads CC64, CC1 (mod wheel),
120 and 123, not CC11 (`crates/sonara-dsp/src/processors/synth.rs:646-670`); the Drum Kit reads
note-ons and all-notes-off only (`processors/drums.rs:1224-1236`).

**New Project.** The dialog's template select lists `sonara_app::templates::TEMPLATES`
(`dialogs/NewProject.svelte:54-58`, `crates/sonara-app/src/projects.rs:237-257`,
`templates.rs:30-57`), pinned by the GLD-01 oracle above; the agent tool has its own literal list
(`crates/sonara-agent/src/contract/inputs.rs:89`). Templates are command lists applied to a fresh
project (`templates.rs:26`, `projects.rs:277-302`), not undoable edits.

**Theme, menus, shortcuts.** Panels follow the dark or light palette through CSS tokens; all
canvases stay dark in both themes (`src/lib/theme.ts:1-8`, `theme.svelte.ts:16-21`). The Track menu
has New Instrument/Drum/Audio Track, Bus, Folder, Delete (`crates/sonara-app/src/menus.rs:611-624`);
View has ⌘1-⌘3 (`menus.rs:641-653`); Space, Home, R, L, C are page-wide single keys
(`shell/shortcuts.ts:1-4`). A menu `Shortcut` has only ⌘ and ⇧ (`menus.rs:349-358`, accelerator
text at `385-397`). The menu model's page fixture `tests/fixtures/menus-v1.json` is written from
sonara-app with `SONARA_WRITE_FIXTURES=1` (`crates/sonara-app/tests/gui_menus.rs:653-663`); it is
not prototype-recorded golden data. The piano roll's own keys are page keys with a `Mods` of ⇧, ⌘
and ⌥ (`pianoroll/gestures.ts:59-66`).

**Commands and layering.** The Tauri handlers are listed in `src-tauri/src/main.rs:143-191`;
GUI-only commands already exist, for example `audition_note`, documented as "no agent or MCP tool"
(`src-tauri/src/commands.rs:381-392`). The desktop depends only on `sonara-app` and `sonara-agent`
(`apps/sonara-desktop/src-tauri/Cargo.toml:31-33`, `crates/sonara-testkit/tests/architecture.rs:59-60`);
`sonara-app` depends on `sonara-midi` (`crates/sonara-app/Cargo.toml:33`).

**Findings.** `sonara_app::quality::check_project` gives findings with a severity, a message, an
explanation and `track_id`/`clip_id`/`note_ids`; no GUI shows them (`crates/sonara-app/src/quality.rs:1-13`,
`155`).

**Performance harness.** `pnpm run perf:signoff` runs the timeline, meters, piano-roll and
playback parts (`apps/sonara-desktop/scripts/perf-signoff.mjs:1-15`, `57-77`); the suites live in
`src/lib/bench/` (`timeline.ts:1-17` measures the arrangement draw and the project view's cost;
`meters.ts:1-24` includes a knob drag on a device card; `pianoroll.ts:1-17` measures drag, resize
and lasso on 2,000 notes and the owner's 10,000-note paint in one frame). ADR 0009's targets are at
[0009-gui-toolkit.md](../../decisions/0009-gui-toolkit.md) lines 73-76; the desktop measured
timeline p95 3 ms, meters 60 fps p99 38 ms, 10,000 notes in 2 ms (lines 177-180).

## Proposal

### 1. Principles

1. **One source of knowledge (DES-01, ARC-11).** The page holds no instrument data. `sonara-app`
   builds every view from `sonara_midi::orchestration` (S1), the catalog and the profiles (S7) and
   the degradation chain (S8). The page renders and edits.
2. **Controls are bindings.** Each control binds to exactly one of: a sampler parameter (S3, S4),
   clip controller events, a track or project operation, or read-only derived data. Nothing is
   state that only the GUI holds, so the agent can reach the same result (the lesson of
   [../cards/apple.md](../cards/apple.md) section 7, "what not to copy"). The one asymmetry, CC lanes,
   is called out (owner decision U1).
3. **Honest availability.** A control is live only when the engine and the source honour it. The
   rest sit under a collapsed "Not available with this source (n)" with the reason, citing the
   conformance id ([../sfz-conformance.md](../sfz-conformance.md)) or the open question.
4. **Identity has a provenance (S3).** Shown with the words and in the precedence of layer (b) 3.3:
   *part* (O6), *named* (the user set `instrument_type`), *declared* (the library's manifest),
   *profile* (Sonara's shipped profile), *inferred* (an alias match on a patch path or on a Poly
   Synth or Drum Kit track name, never presented as fact) or *none*.
5. **Edits are undoable steps.** Each user action is one `Edit`; actions that need several core
   commands become one `Transaction`, as Set Sidechain, Split Clips and Duplicate Clips already do
   (`edit.rs:842`, `1015`, `1052`).
6. **Budgets first.** During playback the only new per-frame work is one comparison per header row
   (playhead against that row's next articulation switch) and a DOM write when the text changes;
   every new canvas element is batched like the existing painters and measured (section 12).
7. **Prototype oracles stay green (GLD-01).** The Devices tree, the New Project template list, the
   Settings bytes, the Inspector's existing rows and the 12 built-in device cards keep exactly what
   `dialogs-v1.json` and `device-editors-v1.json` record; new things are added beside them.

### 2. Information architecture

```text
┌ Browser ───────────────┬ Timeline: headers │ arrangement ──────────────┬ Inspector ─────────┐
│ [Library|Devices|Files]│ ▾ Strings (folder)                             │ Track · instrument │
│ search "housle"        │ │ Vn. Violins I   section·sustain  ⚠2  M S R   │ Instrument  Violin │
│ ▾ Strings              │ │ Vn. Violins II  section·pizz         M S R   │ Part   Violins I   │
│   ▾ Violin  Vn. 55-105 │ ▾ Brass (folder)                               │ Section (source)   │
│     ● VSCO 2 CE sect.  │ │ Hn. Horn 1      solo·legato          M S R   │ Range  G3-A7 …     │
│     ◐ VSCO 2 CE solo   │                                                │ Source VSCO 2 CE … │
│     ⊘ Sonatina 1st Vln │                                                │ Findings (2)       │
│ ── Violin (detail) ──  ├ Editors: [Mixer|Piano Roll|Devices] ───────────┴────────────────────┤
│ ranges, clefs, arts.   │ Devices: sampler card + family panel │ Piano roll: lane selector    │
└────────────────────────┴──────────────────────────────────────────────────────────────────────┘
```

- **Find** an instrument in the Library tab; **see and change** its identity in the header and the
  Inspector; **play** it through the family panel in the Devices tab and the lanes of the piano
  roll; **build** an orchestra with New Project or Add Ensemble….
- The Devices tab of the Browser stays exactly as it is (its tree is pinned by `dialogs-v1.json`);
  the Library tab is first and selected by default once a profile or library exists.

### 3. The Library browser

**Tree.** The S1 `Family` values (strings, woodwinds, brass, percussion, keyboards, plucked, choir,
other; `crates/sonara-midi/src/orchestration/instrument.rs:12-29`, one enum after layer a's section
13) in score order (layer a's `Family::score_rank()`): woodwinds, brass, percussion, plucked (harp
first) and keyboards, choir as "Voices", strings, other ([../ensembles.md](../ensembles.md) section 1.1); then instruments in
score order within the family, then sources. An instrument row shows the name, score abbreviation, the sounding
playable range and "solo / section". A source row shows the library and patch name, the ensemble
(solo, section, players if the profile knows them) and a state glyph *with text*:

| State | Shown when | Text and action |
|---|---|---|
| Ready | *available* as layer b 7 defines it for the chain (catalog `Available`, `inspect` found regions and no missing sample, verdict not `not`) and verdict `as_is` (S7) | "Ready". Enter creates a track. |
| Partly | *available* (layer b 7) and verdict `partly` | "Plays partly" and the losses of the S7 verdict (from its structural checks, not the ignored-opcode list alone; C24). |
| Not installed | a shipped profile (VSCO 2 CE, VCSL; S7) lists the preset but no configured library has it | "Not installed: VSCO 2 CE (CC0)" with "How to install…" (an in-app sheet) and "Sample Libraries…". |
| Invalid | catalog `Missing` (a manifest lists a file that is absent) or `Invalid { reason }`, or a deep verify failed | "File missing" or "Cannot load: reason". Include fragments are not listed (C8, after layer b's SS-08 in O2). |
| Built-in | the S8 step 6 fallback (Poly Synth or Drum Kit preset) | "Built-in approximation" in a separate group, always last. |
| General MIDI | only if A10 is accepted | "GM program n from <SoundFont>". |

**Search** matches the instrument's names and S1 aliases (Czech "housle" finds Violin), the
abbreviation and source names; a matching instrument opens with all its sources. At most 500 rows
render at once; a "Show all n results" row extends it (same idea as `MAX_NOTE_OPTIONS`). A filter
"Installed only" hides instruments without a ready or partly source.

**Detail pane** under the tree (the selected instrument): names (English, Italian,
abbreviation), sounding and written ranges with provenance, transposition, clefs, articulations
grouped by kind with which ones the selected source maps, ensemble profile, and for keyboards the
mechanics row of [../model.md](../model.md) (action, keys, pedals, resonance, noise, decay).

**Actions.** Double-click or Enter on a source: *Create Track* (one undo step: core `CreateTrack`
with `index` and `parent_id` plus `SetDeviceParams` for `instrument` = the preset id (S5), in one
`Transaction`; named by the next free part name, "Horn 3" after Horn 1-2
([../ensembles.md](../ensembles.md) 1.3), placed in score order inside the family folder when one
exists). Context menu: Create Track, Use on Selected Track (`Edit::SetSource`: `SetInstrument` when
the track has no sampler, then `SetDeviceParams`, one `Transaction`), Show in Finder, How to
Install…. Activating an instrument row uses its best source by the S8 chain and reports what it
chose in the status bar.

**Install guidance.** The bundle has no docs, so "How to install…" is an in-app sheet whose steps
come from the shipped profile (download page, branch or tag, folder to add; the same text as
`docs/sample-libraries.md`, DOC-01) with "Open Download Page". `open_link` accepts only known links
(`about.rs:143-146`, TYP-06), so a fixed table of library pages joins the allow-list: VSCO 2 CE and
VCSL now; Sonatina and others only if the owner accepts A17. "Sample Libraries…" is a sheet of its
own (folders added through the native picker, `src-tauri/src/prompt.rs:173-180`) whose commands
write only the `sample_libraries` key, so the Settings dialog and its byte oracle stay untouched
(GLD-01). Phase 1 says "takes effect after restart", because the registry is built at startup
(`devices/catalog.ts:1-4`; `crates/sonara-app/src/samples.rs:87-95` from `context.rs:193`). A live
rescan is a later item.

Sketch (not implemented):

```ts
/** sonara-app `browser::LibraryNode` (Tauri `library_tree`). */
interface LibraryInstrument {
  id: string; name: string; abbreviation: string; family: Family;
  playable: [number, number] | null; solo: boolean; section: boolean;
  sources: LibrarySource[];
}
interface LibrarySource {
  preset_id: string | null; plugin_id: string; label: string; library: string | null;
  licence: string | null; ensemble: "solo" | "section" | "section_emulated" | null; players: number | null;
  state: "ready" | "partly" | "not_installed" | "invalid" | "builtin";
  losses: string[]; install: { library: string; steps: string[]; link: string | null } | null;
}
```

### 4. Track header

- **Name** stays the part name (S3 phase 1: parts live in track names). A **chip** before it
  shows the score abbreviation ("Vn.", "Hn."), drawn as text; an *inferred* identity shows "≈Vn.".
- **Detail line** (rows ≥ 44 px) on sampler tracks with a *named*, *declared* or *profile* identity: "Violin ·
  section · VSCO 2 CE · sustain", replacing "MIDI · preset"; volume, pan and fx keep their place at
  the end. Every other track keeps today's `detailText` (pinned by `headers.test.ts`); an
  *inferred* identity adds only the "≈" chip.
- **Articulation** in the detail line is the `articulation` automation value at the playhead. The
  page precomputes each track's sorted switch beats and keeps the next one; per live frame it
  compares the playhead with it and moves on only when crossed (a binary search after a seek); the
  text is a `$derived` string, so equal values cause no DOM update. A track without an
  `articulation` lane shows the device's static parameter value.
- **Warnings**: a badge "⚠ n" for notes outside the playable range (error) or outside the
  comfortable range (warning), from the findings service (section 10); a tooltip names the clip
  and bar of the first one. Compact rows (34 px) show only the chip and the badge.
- **Context menu** additions after Show Devices (the existing items keep their order): Show in
  Library, Instrument… (opens the Inspector's Instrument group), Split into Divisi… (string sections), Arrange
  in Score Order, Group by Family, Add Ensemble….
- **Accessibility**: `trackOptionLabel` (`a11y/labels.ts:46-54`) becomes "Violins I, violin
  section, 0.0 dB, 2 notes out of range, solo" (name, identity, volume, warnings, state); the
  provenance word "inferred" is spoken when it applies; a track without an identity keeps today's
  label.

### 5. Inspector

The Track group keeps its rows exactly (the oracle pins them, including "Instrument: name ·
preset"). Below it, an instrument track with an identity gets a separate **Instrument** group
(DOM, theme tokens):

| Row | Binding | Phase 1 behaviour |
|---|---|---|
| Instrument | sampler `instrument_type` (S3; layer b's SS-14: a non-automatable enum of `auto` and layer a's 85 ids), offered filtered to the source's family + "Auto (from source)" | editable for unprofiled presets; for profiled ones "Auto" shows the profile's id and a note. Poly Synth/Drum Kit tracks show an inferred id read-only. |
| Part | track name | the existing rename. |
| Ensemble | profile `ensemble` | "Solo", or "Section" with the player count when the profile records one; read-only until O6 (`part.players`, A1). |
| Range | `sonara_midi::orchestration` | "Sounding G3-A7 (55-105), comfortable G3-A6"; transposing instruments add "written …". |
| Clefs, transposition | S1 | read-only. |
| Source | `instrument` (preset id, S5) | library, patch, licence, verdict and losses; "Show in Library". |
| Findings | findings service | list of range and playability findings; a click selects the clip and notes in the piano roll. |

### 6. Device editor: the family panel

On a `sonara.sampler` card, a family component renders above the generic groups, chosen from the
track's instrument context (the same mechanism as `plugin.id === EQ_PLUGIN` → `EqCurve`,
`DeviceCard.svelte:205-206`). The parameters the panel shows (`instrument`, `instrument_type`,
`articulation`, `dynamics` and any state parameters) are removed from the generic grid below it, so
no control appears twice. Without an identity the card stays generic and offers "Name this
instrument…" (sets `instrument_type`). The sampler is not in `device-editors-v1.json`, so its card
may change; on a `sonara.drums` card the percussion pads are added above the unchanged generic
groups, and no other built-in card changes (GLD-01).

```text
┌ Sonara Sampler  [On]  Violins I                          Preset ▾  ⋯ ┐
│ Violin · section · VSCO 2 CE / ViolinEns-KS        ◐ plays partly ⓘ │
│ Source [Violin section, VSCO 2 CE ▾]   [Solo | Section]              │
│ Articulation [sustain ▾]   (auto · ordinario · mapped ids, by kind)  │
│ Dynamics ◯ 0.80 (× vel., −3.9 dB)  Divisi [Split…]  Players: not rec.│
│ ▸ Not available with this source (2)                                 │
├ Main: Volume  Pan  Release                                           │
└──────────────────────────────────────────────────────────────────────┘
```

**Common header** for every family: the identity line with the verdict; a **Source** picker
listing the presets of the same instrument (solo and section groups, availability glyphs), which
sets `instrument`; **Articulation** (a combo of `auto`, the default, in which the patch and clip key
switches decide; `ordinario`, the profile's default; then the preset's mapped ids grouped by kind
from [../articulation-vocabulary.md](../articulation-vocabulary.md), each with its key switch "C1"
when the profile names one; `legato` is marked "monophonic", layer b 5.2); **Dynamics** (knob 0..1;
the pp-ff mark of layer c's level table only in the profile's `cc` mode, where the value is
absolute; in `velocity` and `fixed` mode it is a ratio, so the knob reads "× written velocity" and
the held-note change `40·log10(d)` dB, layer c section 3). Changing Articulation or Dynamics in the panel sets the parameter's static
value; the lanes (section 7) automate it. The panel says so when an automation lane exists ("set
by automation from bar 3").

**Parameters the panel binds to.** `articulation`, `dynamics` (S4) and `instrument_type` (S3).
Choices belong to the plugin descriptor, not to a device (`builtin.rs:767-798`; `SetAutomation`
checks against it, `automation.rs:68-83`), and enum automation stores indices; so the Articulation
combo filters one global, append-only choice list to the preset's mapped ids, and stored points
survive a source change. On a preset no profile maps, only `auto` acts, so the combo is listed
under "Not available: no profile maps its articulations". Mute and vibrato (S2 states) travel in
phase 1 as `articulation` choices (layer b 5.7): the Mute and Vibrato rows below are filtered views
of the same combo (ids whose layer-a `axis()` is `Mute` or `Vibrato`), and choosing one replaces
the primary id unless the profile recorded the pair. Separate `mute` and `vibrato` enums arrive only
with layer b's optional SS-34 (A16).

**Family views.** "Shown when" is the gating rule; anything not shown is listed with its reason.

| Family | Control | Bound to | Shown when |
|---|---|---|---|
| Strings | Articulation, Dynamics | `articulation`, `dynamics` | always (choices from the profile; unprofiled: Dynamics only, Articulation under "Not available") |
| | Solo / Section | `instrument`: the counterpart preset of the same instrument | a counterpart is installed; else disabled, "No solo violin installed" (S8 step 3 wording) |
| | Players | profile `ensemble.players` | read-only; editable only with O6's `part.players` (A1) |
| | Divisi | action `split_divisi`: new tracks "Violins I div. 1/2" with the same device parameters and copies of the track's automation lanes; the selected notes, or the upper and lower voice, move to clips at the same positions | string sections. One articulation state per track (S4), so divisi is always a track split, never a per-note split |
| | Vibrato | `articulation` choices `vibrato_on`, `non_vibrato` (SS-34's `vibrato` if A16 adds it) | the profile maps vibrato variants (VSCO flute and oboe vib/non-vib patches are the model) |
| | Mute (con sordino) | `articulation` choice `con_sordino` (SS-34's `mute` if A16 adds it) | the profile maps a muted patch; PLY-04 (wave 1 in layer c) warns when the lane switches the mute with too little rest ([../doubling-and-balance.md](../doubling-and-balance.md)) |
| Keyboards | Instrument type | `instrument` among the keyboard ids' installed sources; unprofiled: `instrument_type` | always; each type shows its one-line difference from the mechanics data (action, keys, pedals, resonance, decay) |
| | Sustain pedal | Pedal lane (CC64 in the clip; sampler handles it, `sampler.rs:209-224`) | the instrument's mechanics list CC64; a button opens the piano roll on the Pedal lane |
| | Sostenuto, una corda / soft | CC66, CC67 lanes | only when the engine handles the CC and the profile declares it; not in phase 1 (today the sampler ignores both, `sampler.rs:206-225`; layer b's SS-20 adds CC gates and crossfades, not pedal semantics) |
| | Resonance, key and pedal noise | a profile-declared level control | only when S7 declares one; none in phase 1 (S7 has no level field; after layer b's SS-07 (N1) and SS-05, SS-20 (Q7, `on_locc`) Salamander's release, resonance and pedal-noise regions play at the patch's own levels) |
| | Swell (Clavinet foot swell, organs) | Expression lane (CC11, a linear gain on the sampler, `sampler.rs:208,391`; C15) | mechanics list CC11 |
| Percussion | Pads | a grid of the preset's keys: GM name, orchestral name and articulation ("Snare drum: rim"), key number; click auditions through `audition_note` | `sonara.drums` (GM map) or a profile key map; for unprofiled presets pitched names only |
| | Timpani tuning | derived by layer c's `check::timpani_tunings` (PF-6): tunings per passage and the passages no tuning covers | timpani; PLY-11 findings join with layer c's wave 2 |
| Woodwinds, brass | Dynamics | `dynamics` (large knob, the main control) | always |
| | Breath and endurance | PLY-01, PLY-02 findings with "Show in piano roll" | wind and brass ids |
| | Mute (brass) | `articulation` choices of the mute kind (straight, cup, harmon … as mapped; SS-34's `mute` if A16 adds it) | profile maps mutes (VSCO `FHornMute`, `TrumpetStraightMuteSus`) |
| Voices | Vowel | `articulation` entries of kind vocal, when the profile maps vowel patches | profile maps them; the shape of a vowel value waits for A16 |
| | Text (lyrics) | none | never in phase 1: per-note text needs a project-format change (A16, CTR-01) |
| Harp | Pedal diagram | derived by layer c's `check::harp_pedals` (PF-6) per passage: D C B │ E F G A, flat/natural/sharp, and the bars no setting covers | harp; PLY-07 and PLY-08 findings join with layer c's wave 2. User-fixed settings need storage (O6) |
| Organ | Registration | `instrument`: each registration is a preset of the instrument as the profile lists it | profile lists more than one |
| | Swell | Expression lane (CC11) | always for organs |
| | Drawbars, Leslie, stops as controls | none | never in phase 1 (A22) |

The harp pedal settings and the timpani tunings per passage are derivations of the performance
layer (PF-6, O4); as pure functions over sounding pitches they live in sonara-midi (no I/O, not a
real-time crate; ARC-11, RT-01). This layer only renders what the context service returns; the
PLY-07, PLY-08 and PLY-11 findings over them arrive with layer c's wave 2.
Percussion key names come from the profile's `keymap[].name` (layer b 2.2, SS-12); an entry without
a name is labelled from its instrument and articulation, a preset without a keymap shows key numbers
and pitch names, and `sonara.drums` shows GM names.

### 7. Piano roll

**7.1 Lane selector.** The toolbar gets a segmented control: Velocity | Articulation | Dynamics |
Pedal | Expression. One lane canvas (the existing `lane` surface) draws whichever lane is chosen,
so the canvas count stays four. Articulation and Dynamics appear only on sampler tracks whose
descriptor has the parameter; Pedal on sampler and Poly Synth tracks (both read CC64); Expression
on sampler tracks only (the Poly Synth ignores CC11, the Drum Kit every CC; "Today"). A missing lane
is listed with its reason in the selector's tooltip. The choice is page state per track.

**7.2 Articulation lane (phase 1: track automation).** It shows and edits the `articulation`
automation lane of the track (`<device_id>:articulation`, curve `hold`), which `SetAutomation`
writes. Drawing: segments [switch, next switch) filled by the kind's colour, with the articulation
name; a hatched segment marks an id the current source does not map, with the S8 fallback text
("Spiccato not available; playing Staccato"). Editing: pick an articulation from the lane's menu
and drag across the lane to paint a segment; or select notes and choose Set Articulation (grid
context menu): the edit writes a switch at the first selected note's start and restores the
previous value after the last one ends, replacing points in that range (`SetAutomation` with
`start`/`end`). Points are written in timeline beats through `contentToTimeline`
(`pianoroll/model.ts:216`).
Two truths the lane makes visible: automation belongs to the track, not the clip, so a moved or
duplicated clip leaves its switches behind and a looped clip does not repeat them (a striped band
over loop repeats says so); and lanes are read at each block's start beat
(`crates/sonara-engine/src/automation.rs:12-15`), so a switch takes effect at the first block that
starts at or after its beat, up to 511 frames late (layer b 5.5); the performance layer's lead rule
(two blocks before the note, its section 2) is drawn as a marker before the note. A hatched
segment's fallback text is the substitute layer b compiled for that choice (5.2).

**7.3 Dynamics lane.** The `dynamics` lane (0..1, curves `smooth` or `linear`) as a breakpoint
curve; in the profile's `cc` mode its horizontal guides are labelled pp-ff (layer c's level table),
in `velocity` and `fixed` mode they show the ratio and its dB change (`40·log10(d)`). The
pencil thins to at most one point per 1/16 beat; points can be dragged and deleted.

**7.4 Pedal and Expression lanes.** Controller events of the clip (`controls`, CC64 and CC11)
edited through `SetControlEvents` (replace kind and number in [start, end)). Erasing a range needs
the command to name its target pair even with no new events, so core `SetControlEvents` gains an
explicit `(kind, number)` list (UI-3; an internal command: no tool, no schema; recording keeps its
behaviour). Pedal draws down/up blocks (down at 64 or more, as the sampler and the Poly Synth read
it, `sampler.rs:210`, `synth.rs:651`); Expression a 0-127 curve. Imported MIDI's CC64 and CC11
become visible and editable for the first time. Agents cannot write CC until O6 (S4 phase 2):
owner decision U1.

**7.5 Range shading.** From the context's sounding ranges: rows outside the playable range get a
dark red tint, rows outside the comfortable range an amber tint, on the grid and the keyboard,
drawn as at most four `fillRect`s per surface beside the black-key shading (`paint.ts:208-215`).
Notes outside the playable range get a red outline; outside comfortable, an amber one. Outlines are
strokes, so they go into two more batched `Path2D`s (red, amber), chosen per note by comparing its
pitch with the two ranges; the fills and `fillKey` (whose `playable` bit means something else) stay
as they are. Unpitched instruments get no shading. Extensions and gaps follow A19 when the model
grows them.

**7.6 Written and sounding.** Notes stay at sounding pitch in the data, as Logic keeps MIDI at
sounding pitch and transposes only the display ([../cards/apple.md](../cards/apple.md) sections
3.4 and 7). A toolbar toggle "Written" (transposing instruments only) relabels the keyboard (C
names on rows whose *written* pitch is a C), the hover readout and the ARIA note names ("written
C5"); rows, notes and edits are unchanged, so no conversion enters the edit path. A per-part
transposition override is A20.

**7.7 Key-switch keys and percussion names.** The keyboard's labels generalise from `drumLabels`
to a per-track `keyLabels` map from the context: key-switch keys ("KS pizz"), percussion names,
or GM names. Key-switch notes already in clips (imported MIDI stays playable, S4) are drawn
hatched and stay visible; a view option hides them. Key-switch rows inside the playing range (VSCO
`CelloEns-KS` 84-87, VCSL `Timpani 1 - Keyswitch` 36-37; [../coverage.md](../coverage.md) gap 3
in section 3 and section 4) get the label and a warning tint, because a note there switches
instead of sounding. After layer b's SS-18 (O3) a seek or loop wrap restores the last clip key
switch ([../sfz-conformance.md](../sfz-conformance.md) C11 fixed on the `auto` path); the tooltip
says which switch is in effect.

**7.8 Phase 2 (O6).** With a per-note `articulation` field, notes are tinted by articulation,
the Articulation lane shows per-note chips aligned with the notes, and a note inspector edits
them; the track `articulation` lane (or SS-34's `mute` and `vibrato` lanes) remains for passage
states.

### 8. Arrangement: score order and family folders

- **Arrange in Score Order** reorders the selected tracks (or all) by S1 family and score order,
  keeping a doubling track below its player's main track ([../ensembles.md](../ensembles.md) 1.1);
  tracks without an identity keep their relative order at the end. One `Transaction` of `MoveTrack`.
- **Group by Family** creates the missing folder tracks, one per `Family` in use, in the order and
  with the names of the Library (section 3), and parents the tracks to them (core `CreateTrack`
  with `parent_id`, `track.rs:103`, and `SetTrackParent`, `track.rs:653-683`), one `Transaction`.
- **Folder collapse** is page state (a disclosure on folder rows; collapsed children are not laid
  out), not project data. Fewer rows make the timeline cheaper, not dearer.

### 9. Ensembles: New Project and Add Ensemble…

- **New Project.** "Start with" becomes a choice: *Template* (the existing list, Orchestral
  unchanged, GLD-01) or *Ensemble* (classical, Beethoven, romantic, late-romantic, film, chamber,
  string orchestra, big band, choir and orchestra, small groups; S9). The form gains an optional
  `ensemble` field (`#[serde(default)]`, `None`), applied after the `empty` template by the S9 app
  service (layer c's PF-20, the same plan its planner turns into tool steps); the `templates` list and the other fields stay as the oracle records them (GLD-01). A
  preview lists the parts with their sources and the S8 substitutions ("Contrabassoon: no source;
  Bassoon 2 plays it", "Timpani: VCSL Timpani 1, ready").
- **Add Ensemble…** (Track menu, ⌥⌘N) opens a dialog: ensemble; a checklist of parts by family
  with players; options "Folders by family" (default on) and "Score order" (default on); the same
  preview. OK applies one `apply_ensemble` edit, one undo step; existing tracks are never changed,
  new folders merge into existing family folders of the same name. The S9 service also creates the
  family buses and sets pan only for sources not recorded in situ; the preview shows both, and the
  dialog does not expose them separately in phase 1.
- In phase 1 the ensemble service and the planner are reached from the GUI (New Project, Add Ensemble…, Orchestrate…);
  MCP agents and the AI Composer build ensembles from existing tools, guided by the orchestration reference (S9, S10).
  Ensemble and orchestrate tools are O6 (A7, CTR-01; layer c's PF-19, PF-22).
- **Orchestrate…** (Track menu) is layer c's text field for its planner (PF-15, O5): the request,
  then the planner's questions, substitutions and confirmation in the same sheet, applied inside one
  undo group. This layer gives it the sheet's place; PF-15 builds it.

### 10. Backend: Tauri commands and sonara-app services

All logic is in `sonara-app`; `src-tauri` adds thin commands (ARC-11). None is an agent tool,
schema, IPC message or CLI command (CTR-01), following the `audition_note` precedent.

| Tauri command (`src-tauri/src`) | sonara-app service (module) | Returns or does |
|---|---|---|
| `library_tree` (dialogs.rs) | `browser::library_tree` (catalog, profiles, `orchestration`, S8 chain) | the Library tree of section 3 |
| `instrument_detail(id)` | `instrument_view::detail` | the detail pane and Inspector read-only rows |
| `track_instrument_contexts` (commands.rs) | `instrument_view::contexts` | per instrument track: identity and provenance, family and panel kind, solo/section/players, sounding ranges, transposition, clefs, key labels, articulation choices with kind, key switch and the compiled substitute of each unmapped choice (layer b 5.2), dynamics mode, control availability with reasons; cached per (track, preset, params) |
| `orchestra_findings` | `quality::check_project` + O4's orchestration findings, grouped per track | header badges, Inspector list, piano-roll markers; recomputed off the main thread, debounced after note edits |
| `ensembles_view` | `instrument_view::ensembles` (data from `orchestration::ensembles`, S1; plan from `ensembles`, PF-20) | ensembles, parts, preview with substitutions |
| `edit` (existing) | `edit::Edit` new variants below | one undo step each |
| `sample_libraries_view` / `set_sample_libraries` (dialogs.rs) | `samples` (owns `LIBRARIES_KEY`, `samples.rs:19`) | the Sample Libraries sheet; writes only that key, never through `settings_dialog` (GLD-01) |
| `reveal_preset(preset_id)` | `instruments` (the catalog's `patch_path`) | Show in Finder for a catalog preset only (TYP-06) |
| `open_link` (existing) | `about::is_known_link` + a library link table | download pages |
| `audition_note` (existing) | unchanged | pad and key audition |

New `Edit` variants (`crates/sonara-app/src/edit.rs`, mirrored in `src/lib/types.ts`):

Sketch (not implemented):

```rust
SetAutomation { track_id: String, device_id: Option<String>, param: String,
                points: Vec<PointInput>, start: Option<f64>, end: Option<f64> }, // core SetAutomation
SetControlEvents { clip_id: String, kind: ControlKind, number: i64,
                   events: Vec<ControlInput>, start: f64, end: f64 },
CreateTrack { /* existing fields */ params: Option<ParamValues>, index: Option<i64>,
              parent_id: Option<String> },
SetSource { track_id: String, preset_id: String },
SplitDivisi { track_id: String, note_ids: Vec<String>, parts: u8 },
ArrangeScoreOrder { track_ids: Option<Vec<String>> },
GroupByFamily { track_ids: Option<Vec<String>> },
ApplyEnsemble { ensemble_id: String, parts: Option<Vec<String>>, folders: bool },
```

Each validates at the boundary (TYP-06): a preset id parses into `InstrumentId` through a new
in-crate parser that looks it up in the catalog (today only `InstrumentId::new(library, path)`
exists, `instruments/model.rs:89-95`), an orchestration id into layer a's `InstrumentType`
(`from_id`; TYP-07 keeps the two apart). An unknown ensemble or preset is `NOT_FOUND`; the core
answers a non-automatable parameter with `UNSUPPORTED`, a value out of range with
`INVALID_ARGUMENT` and too many points with `LIMIT_EXCEEDED` ("Today"). The view adds
`TrackView.automation` (device and mixer lanes with points) and `MidiClipView.controls`
(`view.rs`, `types.ts`).

### 11. Accessibility, theme and shortcuts

**Accessibility** follows ADR 0009's rule that drawn views get an ARIA layer of their own
([0009-gui-toolkit.md](../../decisions/0009-gui-toolkit.md) line 122) and the existing patterns:

- Library: `role="tree"` like the Devices tree (`Browser.svelte:262-299`); a source's state is in
  its accessible name ("VSCO 2 CE violin section, ready" / "not installed, VSCO 2 CE"), never
  colour or glyph alone.
- Family panels are DOM controls with names and values (combo "Articulation, pizzicato", knob
  "Dynamics, 0.80, minus 3.9 decibels", or "…, mezzo-forte" in `cc` mode); unavailable controls are
  text, not disabled widgets.
- Lanes: a list box of transparent options over the lane, as `noteList.ts` does for notes, capped
  at 256: "pizzicato from bar 3 beat 1", "dynamics 0.80 at bar 5"; ←/→ move between points,
  ↑/↓ change the value (the next articulation choice), Delete removes a point.
- Notes: `noteOptionLabel` (`a11y/labels.ts:68-75`) adds "outside playable range" and, in written
  mode, the written name. Pads: a grid of buttons named "Snare drum rim, key 37".
- Headers: the extended `trackOptionLabel` (section 4).

**Theme.** New panels, the Library and dialogs use the CSS tokens and follow dark and light. The
new canvas elements (shading, lanes, labels) draw with `THEME` like every canvas today, so they
stay dark in both themes (`theme.ts:1-8`); range and kind colours are chosen against the dark lane
and backed by text or outline, never colour alone.

**Shortcuts.** Menu accelerators (new `MenuAction`s in `menus.rs`, so they appear in the menus;
the `menus-v1.json` fixture is regenerated): Track › Add Ensemble… ⌥⌘N; View › Library ⌥⌘L (shows
the Browser on the Library tab). The menu model's `Shortcut` has only ⌘ and ⇧
(`crates/sonara-app/src/menus.rs:349-358`, helpers `cmd`, `cmd_shift`, `plain` at `500-510`), so ⌥
needs an `option` field in `accelerator()` and `glyphs()` (`385-411`) and a helper (part of UI-29),
or the items fall back to ⇧⌘ combinations. Piano-roll page keys (in `gestures.ts`'s key handler,
not menu items), while the grid or lane has focus: ⌥⌘1-⌥⌘5 choose the lane, ⌥⌘↑/↓ set the next or
previous articulation on the selected notes, ⌥⌘W toggles Written; they match `KeyboardEvent.code`,
because ⌥ changes `key` on macOS. Single letters are avoided because Space, Home, R, L and C are
page-wide (`shortcuts.ts:1-4`). Each proposed key is checked against `menus-v1.json` for conflicts
before it lands.

### 12. Performance

Every element below is new work on a measured path; estimates are reasoned, not measured
(PERF-01). Numbers are taken locally with `pnpm run perf:signoff` and recorded in
`docs/audio-engine.md`; they are never CI gates (HW-01). The suites cannot assume an installed
library, so `scripts/bench-timeline.mjs` generates a fixture library in its sandbox: tiny generated
SFZ patches (sine samples) and a generated profile with key switches, a key map and dynamics,
loaded as a configured library (TST-04, TST-08).

| Element | Expected cost | ADR 0009 budget it must keep | Measured by |
|---|---|---|---|
| Range shading (grid, keyboard) | ≤ 4 `fillRect` per surface per frame, O(1) | piano roll ≤ 2 display frames at 2,000 notes; 10,000 notes in one frame | `bench/pianoroll.ts`, new clip on a profiled sampler track (shading on) |
| Out-of-range note outline | two range comparisons per visible note; two more stroke batches | same | same |
| Key labels (key switch, percussion, written) | ≤ visible rows' text, widths cached (`paint.ts:181-190`) | same | same, Written on |
| Articulation lane | one `Path2D` per kind colour plus ≤ visible segment labels | same | new lane run: paint per frame; new gesture kind "lane-paint" pointer to pixel |
| Dynamics, Pedal, Expression lanes | one polyline or block path over visible points | same | same suite, lane = dynamics |
| Header chip, detail, badge | DOM text per row; changes on identity change or switch crossing only | timeline p95 ≤ 7 ms, no frame > 16.6 ms | `bench/timeline.ts` on an orchestral fixture (100 sampler tracks × 40 clips, automation on each) |
| Larger project view (automation, controls) | JSON grows with lanes and CC | timeline suite reports the view's cost | `bench/timeline.ts` "project view cost", before and after |
| Family panel (knob drag) | DOM, one edit per frame (gesture protocol) | meters ≥ 58 fps, < 1 % dropped, p99 ≤ 50 ms | `bench/meters.ts` knob segment on a family panel |
| Header articulation during playback | one comparison per row per live frame, DOM only on crossing | playback part: 0 underruns; timeline p95 while scrolling | perf-signoff playback part with the orchestral fixture project |
| Findings and contexts | Rust, off the main thread, debounced | must not show in `bench_main_lag` | `meters.ts` main-lag readout during note edits |

The Library tree is DOM outside ADR 0009's budgets. A page test with 5,000 generated presets (the
scan cap, C7; `samples.rs:23`) checks a count, not a time: search and expansion render at most 500
rows. Its timing is read locally in perf:signoff, never asserted in CI (HW-01).

## Contracts and rules

| Rule | How the proposal holds it |
|---|---|
| CTR-01 | O1-O5 add no agent tool, tool-schema field, `.sonara` schema change, IPC message or CLI command. New Tauri commands and `Edit` variants are the desktop's internal boundary, not the agent's JSON-RPC IPC (`docs/architecture.md`; precedent `audition_note`, `commands.rs:381-382`). The core `SetControlEvents` target list is an internal command change; no tool reaches it. Ensemble and CC tools, per-note articulation and `part` are O6 decisions (A1, A4, A7). |
| GLD-01 | The `orchestral` template, its golden workflow and the 12 built-in descriptors are untouched. The prototype oracles stay byte-for-byte: `dialogs-v1.json` (Devices tree unchanged; New Project templates unchanged, `ensemble` a separate defaulted field; Settings untouched, libraries written by their own command; Inspector Track rows unchanged, the Instrument group is separate) and `device-editors-v1.json` (built-in cards unchanged; pads only added above the Drum Kit's groups). `menus-v1.json` is a regenerable fixture, not golden data. |
| LIC-01 | No samples are shipped; the Library shows each source's licence; download links only for libraries the owner accepts (A17, U2). No new npm package or crate; sonara-app gains only a dev-only edge to the workspace's `proptest` (MIT/Apache-2.0, `docs/dependencies.md`). |
| QG-13 | Considered: a tree/virtual-list package for the Library (not needed: rows are capped as `noteList.ts` caps notes), a charting package for the lanes (not needed: the existing Canvas2D painter batches paths). The folder picker is the existing `prompt::pick_folder` over `tauri-plugin-dialog` (`src-tauri/src/prompt.rs:173-180`, `Cargo.toml:36`). The existing Svelte, Canvas2D and Tauri stack covers everything. |
| QG-01..QG-05, QG-09, QG-11 | Rust: fmt, clippy `-D warnings`, documented public items and errors; page: `svelte-check --fail-on-warnings`. Tests per section "Tests"; every performance claim measured with perf:signoff. |
| ARC-11 | Desktop → `sonara-app` only; `sonara-app` → `sonara-midi`, `sonara-core` (allowed by `architecture.rs:32-44,60`). No new edge between workspace crates. |
| MOD-09 | No new crate. New module `sonara_app::instrument_view`; functions in `sonara_app::browser`, `edit`, `view`, `samples`, `instruments`, `menus`; one field on core `SetControlEvents`. The harp-pedal and timpani derivations are O4's (performance layer), in sonara-midi. |
| RT-01..RT-05 | No UI code runs on a real-time thread; the sampler parameters the UI binds to are layer (b)'s and follow RT-02..RT-05 there. |
| RT-06 | Every GUI control posts an edit and returns; nothing waits on the audio thread. |
| RT-07 | The header's articulation is computed from the software playhead (`session.playhead`) and never presented as the device's position. |
| RT-10 | No `unsafe`. |
| DES-01, TYP-06, TYP-07, ERR-03/05 | One data source in sonara-app; ids parsed at the Tauri boundary into newtypes; errors keep the existing `ErrorNotice` codes with context. |
| PERF-10, HW-01 | ADR 0009 budgets per element (section 12); numbers recorded locally in `docs/audio-engine.md`. |
| DOC-01 | `CHANGELOG.md`, `docs/implementation-status.md` (Library, family panels, lanes, ensembles; the "CC automation and pitch bend in clips" row), `docs/sample-libraries.md` (Sample Libraries sheet, install steps matching the in-app sheet), ADR 0009 accessibility checkpoints. |
| VER-01 | No version in `tauri.conf.json` or `package.json`. |

## Alternatives considered

- **Rejected.** A separate window per family (ADR 0009 keeps separate windows for hosted plugins,
  M5; inline panels keep one Devices tab and its benchmarks). The Library as a modal sheet after
  New Track (a Browser tab stays searchable while editing and reuses the tree's accessibility).
  Key-switch notes as the authoring path (A3 option a: they clutter clips, and they broke across
  seeks until layer b's SS-18, C11). Stacked lanes (more canvases and height; a selector keeps one lane surface). Instrument
  data shipped to the page as JSON with range logic in TypeScript (two sources of truth, DES-01).
  A Written mode that moves notes (every gesture would convert pitches; TST-07). One Tauri command
  per family (one cached context per track feeds header, Inspector, panel and roll). Family panels
  in the Inspector (its rows are a GLD-01 oracle; the card already hosts parameter controls).
- **Deferred.** Notes tinted by the articulation in effect (block granularity and clip-independent
  automation could mislead; per-note articulation in O6). Preset preview in the Library (a sampler
  preview decodes the whole patch to RAM, up to 3 GiB, ADR 0010 consequences). User-mappable
  controls like Logic's Smart Controls Learn (needs stored per-preset metadata). Live library
  rescan (the registry is built at startup; a rescan changes the sampler descriptor while projects
  are open).

## Implementation plan

| # | Work item | Milestone | Crate / module | Size | Depends on |
|---|---|---|---|---|---|
| UI-1 | Sampler `instrument` combo shows preset names grouped by library, not paths | O2 | desktop `devices/`; sonara-app `instruments` | S | SS-10 (A5) |
| UI-2 | `Edit::SetAutomation`; `TrackView.automation` in the view and `types.ts` | O5 | sonara-app `edit`, `view`; desktop `types.ts` | M | core `SetAutomation` |
| UI-3 | Core `SetControlEvents` gains an explicit (kind, number) target list; `Edit::SetControlEvents`; `MidiClipView.controls` | O5 | sonara-core `commands::clip`; sonara-app `edit`, `view` | M | none |
| UI-4 | `Edit::CreateTrack` with `params`, `index` and `parent_id`; `Edit::SetSource`; each one `Transaction`; the preset-id parser | O5 | sonara-app `edit`, `instruments` | M | SS-10 (A5) |
| UI-5 | Instrument context service and `track_instrument_contexts` command | O5 | sonara-app `instrument_view`; src-tauri `commands.rs` | L | IM-1..IM-14, SS-13, SS-14 |
| UI-6 | `library_tree` and `instrument_detail` | O5 | sonara-app `browser`, `instrument_view`; src-tauri `dialogs.rs` | M | IM-9, SS-12, SS-26 |
| UI-7 | Library tab: tree, alias search, filter, activation, context menu, row cap | O5 | desktop `browser/` (new `library.ts`, `Library.svelte`) | M | UI-4, UI-6 |
| UI-8 | Library detail pane | O5 | desktop `browser/` | S | UI-6 |
| UI-9 | Sample Libraries sheet (own commands, restart notice), outside the Settings dialog | O5 | sonara-app `samples`; src-tauri `dialogs.rs`; desktop `dialogs/` | S | none |
| UI-10 | In-app install sheet from the profile; download links in the `open_link` allow-list; `reveal_preset` | O5 | sonara-app `about`, `instruments`; src-tauri `dialogs.rs` | S | S7 profiles; U2, A17 for non-CC0 libraries |
| UI-11 | Track header chip, identity detail, badges, menu items, ARIA label | O5 | desktop `timeline/headers.ts`, `Headers.svelte`, `a11y/labels.ts` | M | UI-5, UI-12 |
| UI-12 | `orchestra_findings` service and command | O5 | sonara-app `quality`, `instrument_view` | M | PF-10 |
| UI-13 | Inspector Instrument group (Track group rows unchanged) | O5 | desktop `inspector/` | S | UI-5, UI-12 |
| UI-14 | Family panel framework: binding model, gating, "Not available" list, duplicate-parameter hiding | O5 | desktop `devices/family/` (new), `DeviceCard.svelte` | M | UI-5, SS-14 |
| UI-15 | Strings panel and `Edit::SplitDivisi` | O5 | desktop `devices/family/`; sonara-app `edit` | M | UI-14 |
| UI-16 | Keyboards panel (type comparison, pedal gating, lane shortcuts) | O5 | desktop `devices/family/` | M | UI-14, UI-23 |
| UI-17 | Percussion pads and kit map, audition | O5 | desktop `devices/family/` | M | UI-14, SS-12 (`keymap[].name`) |
| UI-18 | Woodwind and brass panel | O5 | desktop `devices/family/` | S | UI-14, UI-12 |
| UI-19 | Voices panel | O5 | desktop `devices/family/` | S | UI-14, A16 |
| UI-20 | Harp pedal diagram and timpani tuning views; harp panel | O5 | sonara-app `instrument_view`; desktop `devices/family/` | S | UI-14, PF-6 (derivations) |
| UI-21 | Organ panel (registration, swell) | O5 | desktop `devices/family/` | S | UI-14, UI-23 |
| UI-22 | Lane selector and Articulation lane | O5 | desktop `pianoroll/paint.ts`, `gestures.ts`, `model.ts`, `PianoRoll.svelte` | L | UI-2, UI-5, SS-14 |
| UI-23 | Dynamics, Pedal and Expression lanes | O5 | desktop `pianoroll/` | M | UI-2, UI-3, UI-22 |
| UI-24 | Range shading and out-of-range outlines | O5 | desktop `pianoroll/paint.ts` | S | UI-5 |
| UI-25 | Written/sounding toggle (labels, readout, ARIA) | O5 | desktop `pianoroll/`, `a11y/labels.ts` | S | UI-5 |
| UI-26 | `keyLabels` (key switches, percussion), hatched key-switch notes | O5 | desktop `pianoroll/` | S | UI-5 |
| UI-27 | Arrange in Score Order, Group by Family, folder collapse | O5 | sonara-app `edit`; desktop `timeline/` | M | UI-5 |
| UI-28 | New Project `ensemble` field (defaulted; oracle unchanged) and preview | O5 | sonara-app `projects`; desktop `dialogs/NewProject.svelte` | M | PF-20 (S9 service) |
| UI-29 | Add Ensemble… dialog, `Edit::ApplyEnsemble`, Track menu item, fixture regenerated | O5 | sonara-app `edit`, `menus`; desktop `dialogs/` | M | UI-28 |
| UI-30 | ARIA layers for lanes and pads; a11y e2e checkpoints | O5 | desktop `pianoroll/`, `bench/e2e-a11y.ts` | M | UI-17, UI-22 |
| UI-31 | Bench fixture library and orchestral scenarios in all perf:signoff parts | O5 | desktop `scripts/bench-timeline.mjs`, `src/lib/bench/` | M | UI-22-UI-24, UI-11 |
| UI-32 | Docs (DOC-01) | O5 | `CHANGELOG.md`, `docs/` | S | all |
| UI-33 | Phase 2: per-note articulation in the roll, note inspector, part editing in the Inspector | O6 | desktop `pianoroll/`, `inspector/`; sonara-app `view`, `edit` | L | IM-19, PF-18 (A1, A3 b) |
| UI-34 | GM sources in the Library | O6 | sonara-app `browser` | S | A10 |

Placement rules: desktop code (`apps/sonara-desktop`) is the GUI of ADR 0009 and reaches the core
only through new Tauri commands into `sonara-app` (ARC-11); services, view DTOs and `Edit` variants
live in `sonara-app`, which may use `sonara-core` and `sonara-midi` (ARC-11, architecture test);
the only core change is UI-3's field on `SetControlEvents` (sonara-core stays pure); the harp and
timpani derivations UI-20 renders are O4's, in sonara-midi (no I/O, not a real-time crate; ARC-11,
RT-01 unaffected); no work item adds a crate (MOD-09), touches a real-time module (RT-01..RT-05) or
adds `unsafe` (RT-10).

## Tests

Rust tests go in `crates/sonara-app/tests/gui_orchestra.rs` and `crates/sonara-core/tests/commands.rs`;
page tests in `tests/unit/*.test.ts` and `tests/page/*.spec.ts` with `tests/page/tauri-mock.ts`.

- **TST-01 (unit).** `library_tree_orders_families_by_score_order` (expected: the order of section
  3); `library_states_follow_catalog_and_verdict` (fixture presets Available+as is, partly, listed
  only by a profile, Invalid → ready, partly, not_installed, invalid); `alias_search_finds_violin_from_housle`;
  `context_provenance_is_named_declared_profile_inferred_or_none` (five fixture tracks, expected provenance
  each); `preset_id_parser_round_trips_catalog_ids`. Page: `detailText` keeps every existing
  expectation of `headers.test.ts` and gives "Violin · section · VSCO 2 CE · sustain" for a
  profiled fixture; `keyLabel` in written mode on `horn_f` labels sounding F3 (53) as written "C4";
  range bands for a known viewport are the expected ≤ 4 rectangles; articulation segments from
  known points; lane gating gives Pedal to sampler and Poly Synth, Expression to the sampler only,
  none to the Drum Kit.
- **TST-02 (integration).** `create_track_from_source_is_one_undo_step` (undo gives back the equal
  project); `apply_ensemble_is_one_undo_step`; `split_divisi_moves_notes_and_copies_lanes`;
  `articulation_switch_lands_at_the_next_block_start` (null backend, generated fixture library: the
  sampler's articulation changes at the first block start at or after the switch beat, computed
  from tempo and block size); `set_control_events_clears_a_target_with_no_events` (core). Page specs: Library activation,
  family panels per family, lanes, New Project and Add Ensemble.
- **TST-03 (failure paths).** `automating_the_instrument_param_is_unsupported` (`UNSUPPORTED`);
  `unknown_preset_is_not_found`; `unknown_ensemble_is_not_found`; `an_unlisted_library_page_is_refused`
  (`PERMISSION_DENIED`); `reveal_refuses_a_path_outside_the_catalog`; page: a track with no identity
  gets the generic card, and an invalid preset shows its reason.
- **TST-04, TST-08.** Generated SFZ fixtures and profiles only; no user library, no network; the
  page uses the Tauri mock; generated inputs use fixed seeds.
- **TST-05 (contracts and oracles).** Unchanged and green: `golden/contracts/tools.json` contract
  tests, the golden `cinematic_orchestral` workflow, `the_new_project_dialog_matches_the_prototype`
  and `the_settings_dialog_writes_the_prototypes_bytes` (`gui_dialogs.rs`), the Inspector oracle
  (`inspector.test.ts`) and the device-editor oracle (`device-editors-v1.json`).
- **TST-06 (regressions).** `include_fragments_are_not_library_rows` (C8);
  `imported_keyswitch_note_is_labelled_and_visible` (C11).
- **TST-07 (invariants).** `written_toggle_never_changes_a_pitch` (every gesture's edit payload is
  equal with Written on and off); `score_order_is_a_permutation`; `apply_ensemble_never_edits_an_existing_track`;
  `lanes_never_offer_a_non_automatable_param`; `source_change_keeps_articulation_ids`.
- **TST-09 (property).** `arrange_in_score_order_is_idempotent_and_stable` with `proptest` (a new
  dev-dependency edge of sonara-app; the crate is already a workspace dev dependency, its
  `docs/dependencies.md` row gains the user, QG-10); page: points → segments → points is the identity
  over a seeded generator loop (no new npm package, QG-13, LIC-01).
- **TST-10.** Every test compares labels, edit payloads, ARIA strings, undo results or drawn
  rectangles against known values; none only checks that a component renders.
- **Accessibility.** New checkpoints in `src/lib/bench/e2e-a11y.ts` read by `tests/e2e/a11y-tree.mjs`:
  `library` (a source treeitem with its state), `family-panel` (Articulation combo name and
  value), `lane` (an articulation lane option), `pad` (a pad button), each with its expected label.
- **Performance (PERF-10).** Section 12's scenarios in perf:signoff, recorded in
  `docs/audio-engine.md`; never CI gates (HW-01).

## Risks

- **Automation is per track, not per clip**: moving, duplicating or looping a clip does not carry
  its articulations. Mitigation: the lane shows it; per-note articulation (O6) is the fix.
- **Block granularity** makes a switch land up to one block late; the performance layer's lead
  time is drawn, not hidden.
- **GUI-only CC lanes** give the GUI a capability agents lack until a CC tool (U1, A4).
- **Few profiled presets at first**, so long "Not available" lists. Mitigation: `instrument_type`
  gives unprofiled presets ranges and names; VSCO 2 CE and VCSL profiles ship first (S7).
- **Dependence on layers (b) and (c)**: `articulation`, `dynamics`, `instrument_type` (SS-14),
  `keymap[].name` (SS-12), the findings (PF-10) and derivations (PF-6). Without them the related
  controls show as unavailable, never broken.
- **Enum automation stores indices.** A reordered or per-preset `articulation` list would switch
  existing lanes to other articulations silently. Mitigation: the global, append-only list this
  layer requests, and `source_change_keeps_articulation_ids` (TST-07).
- **Inferred identities can be wrong**; they are marked and never used for blocking errors.
- **Header density** at 34 px rows: only the chip and the badge show there.
- **Dark canvases in the light theme** look partial; a light canvas palette is outside this layer.
- **Bench fixtures differ from real libraries** (sizes, key maps); real-library runs stay human
  checks (`docs/implementation-status.md`, "the sampler with a real library";
  [B1](../open-questions.md#b-human-checks) for the renders).

## Owner decisions

- [A1](../open-questions.md#a1-where-an-instruments-identity-and-part-live-in-a-project): players and solo/section
  become editable only with a stored part (O6).
- [A3](../open-questions.md#a3-how-articulations-reach-a-library), [A4](../open-questions.md#a4-dynamics-an-automatable-sampler-parameter-or-a-cc-writing-tool):
  the lanes bind to the S4 parameters in phase 1; per-note articulation and a CC tool are O6.
- [A5](../open-questions.md#a5-the-samplers-instrument-parameter-absolute-paths-or-preset-ids): the Source picker,
  UI-1 and UI-4 assume preset ids.
- [A6](../open-questions.md#a6-library-mapping-data-where-it-lives-what-it-holds-whether-sonara-ships-it): what the
  Library and panels show depends on the profile fields, including `keymap[].name`.
- [A7](../open-questions.md#a7-ensemble-templates-gui-only-or-the-create_project-enum): ensembles in the GUI now, a
  tool only by a versioned decision. [A10](../open-questions.md#a10-a-general-midi-fallback-engine): GM rows (UI-34).
- [A16](../open-questions.md#a16-articulation-vocabulary-flat-list-or-separate-axes-mutes-beaters-vowels-shapes):
  the articulation choices, the mute and vibrato carriers and the vowel control follow its shape.
- [A17](../open-questions.md#a17-licences-recommending-and-supporting-sonatina--iowa--or-philharmonia-derived-libraries):
  which libraries get install links (Sonatina and others).
- [A18](../open-questions.md#a18-what-playable-and-comfortable-mean), [A19](../open-questions.md#a19-range-model-extensions-extensions-gaps-variants-and-a-third-tier):
  what the shading's bands mean and whether it shows extensions and gaps.
- [A20](../open-questions.md#a20-notation-one-transposition-per-instrument-and-clefs): a per-part override for the
  Written toggle. [A22](../open-questions.md#a22-instrument-controls-and-multi-zone-keyboards): pedals beyond CC64,
  drawbars, Leslie, harp pedals as stored controls.
- **U1 (new, beside A4):** may the GUI edit CC64 and CC11 lanes before an agent tool writes CC?
  Recommended: yes (the data is already in the format and played), with the asymmetry documented.
- **U2 (new, beside A17):** which third-party download pages Sonara may open (today
  `is_known_link` allows only About's links).

## Deviation from the spine

No decision of S1-S12 is reversed. These refinements are listed so that they are decided, not
assumed:

1. **`mute` and `vibrato` carriers** (S2, S4). S4 names only `articulation` and `dynamics`; layer b
   carries the states as `articulation` choices in phase 1 (its 5.7), so the Mute and Vibrato rows
   are filtered views of that combo, and offers two more automatable enums as optional SS-34 after
   A16. The sampler is not golden (ADR 0010 decision 4), so GLD-01 and CTR-01 are unaffected.
2. **One global, append-only `articulation` choice list** (layer b's deviation 1; the spine's earlier
   wording of S4 said "the preset's mapped articulation ids", now folded in). Choices belong to the plugin descriptor and enum automation
   stores indices ("Today"), so the per-preset set is a filtered view in the GUI, not the
   descriptor's list; `auto` is its default (layer b's deviation 2).
3. **Pad and key labels from the S7 profile**: layer b's `keymap[].name` (its 2.2, deviation 4).
4. **GUI editing of CC64 and CC11 lanes in phase 1** through `SetControlEvents` (with UI-3's target
   list). S4 phase 2 plans a CC-writing *agent tool*; a GUI edit of controller data the format
   already holds is not a tool (CTR-01), so it extends S11 without contradicting S4. Owner U1.
5. **Where S11's "per-family inspector panels" live.** The family controls sit on the sampler's
   device card (Devices tab), and the Inspector gets a read-mostly Instrument group, because the
   Inspector's existing rows are a GLD-01 oracle and the card already hosts parameter controls.
   S11's "section size" is shown read-only until O6 stores `part.players` (S3, A1).
6. **S9's buses and pan** are applied by the S9 service (layer c's PF-20, shared with the planner)
   and shown in the preview; the Add Ensemble dialog exposes only folders and score order in phase 1.

## Review

Adversarial review of 2026-10-30 against the code at 9431759, sfz-conformance.md, the spine and
layer (a). Changes, one per line:

- Title: "(layer d)" added, as layer (a) names itself.
- Today: checkout corrected to 9431759 (code identical to main @ a212544); several line ranges corrected (browser.rs, model.rs, edit.rs, types.ts, projects.rs, templates.rs, track.rs, ADR 0009 lines).
- Today: `docs/implementation-status.md` line numbers replaced by row names (they differ between the commit and the working tree).
- Today: added the prototype-recorded GLD-01 GUI oracles (`dialogs-v1.json`, `device-editors-v1.json`) that the layer had missed.
- Today: track-header citation fixed (rows at `Headers.svelte:301-321`, not 109-150); context-menu item list corrected.
- Today: added that `InstrumentId` has no text parser, that the app bundle has no docs, and that `fillKey`'s `playable` bit is about the clip's played content.
- Today: added `Edit::CreateTrack`'s missing index, `Edit::SetInstrument`'s missing params and the `Transaction` precedent in `edit.rs` (templates.rs uses none).
- Today: added `SetAutomation`'s real error codes (`UNSUPPORTED`, `LIMIT_EXCEEDED`, `INVALID_ARGUMENT`) and that enum ranges come from the plugin descriptor.
- Today: added that `SetControlEvents` cannot erase a range without events (`clip.rs:966-973`).
- Today: added the Poly Synth's and Drum Kit's controller handling (CC64/CC1 only; none).
- Today: added the menu `Shortcut`'s ⌘/⇧-only model and the piano roll's page-key `Mods`.
- Principles: Transaction precedent re-cited; per-frame playback work stated honestly; principle 7 (prototype oracles) added.
- Mockups and Inspector: the unverified "14 players (recorded)" removed (no source records VSCO player counts).
- Library: families aligned with the real `Family` enum; catalog `Missing` state added; "partly" tied to the S7 verdict, not S8 step 1.
- Library: Create Track and Use on Selected Track spelled out as Transactions (`Edit::SetSource` added).
- Library: "How to install…" changed from opening a repo doc (not bundled) to an in-app sheet; libraries edited in their own sheet so the Settings byte oracle stays (GLD-01).
- Track header: detail-line replacement limited to source/named sampler identities so `headers.test.ts` holds; menu additions keep the existing order; ARIA label unchanged without identity.
- Inspector: Track group rows kept (oracle); new separate Instrument group; `instrument_type` described as layer a's non-automatable enum.
- Device editor: GLD-01 note on the built-in cards; Drum Kit pads only added; per-plugin choices and index-based enum automation addressed with a global append-only list; layer a's combination alternative for mute and vibrato added.
- Family table: divisi copies parameters and lanes; Salamander resonance wording aligned with N1 and Q7; swell cites the sampler gain (C15).
- Harp and timpani derivations moved from this layer to O4's PLY-07, PLY-08 and PLY-11 checks; UI-20 now depends on O4.
- Piano roll: Pedal and Expression lanes gated by what the device reads (principle 3 was contradicted).
- Piano roll: out-of-range outlines are stroke batches, not a fill-key bit; `SetControlEvents` gains a target list (UI-3, sonara-core); coverage.md citation corrected to gap 3 and section 4.
- Arrangement: Group by Family folders follow the `Family` enum.
- Ensembles: New Project `ensemble` field defaulted with the oracle unchanged; S9's buses and pan applied by the service.
- Backend: settings row replaced by sample-library commands; `reveal_preset` added; Edit sketch gains `index`, `SetSource` and the control target; id validation and error codes corrected.
- Shortcuts: menu accelerators separated from piano-roll page keys; ⌥ needs a `Shortcut` field; keys matched by `KeyboardEvent.code`.
- Performance: the Library's CI timing test replaced by a count test (HW-01); outline and header costs restated.
- Contracts table: CTR-01, GLD-01, LIC-01, QG-13, ARC-11, MOD-09 and DOC-01 rows updated to the above.
- Plan: UI-1, UI-3, UI-4, UI-9, UI-10, UI-13, UI-20 and UI-28 rewritten (crate, size, dependencies).
- Tests: named tests with oracles; TST-03 code is `UNSUPPORTED`; TST-05 cites the existing oracle tests; TST-09's proptest edge and a seeded TypeScript loop (no new npm package).
- Risks: enum-index risk added; human-check references corrected (B1 anchor).
- Owner decisions: A17 anchor fixed; U1 and U2 tied to A4 and A17.
- Deviation: added the global articulation list, the placement of S11's family panels and S9's buses and pan.
- Alternatives: compressed; "family panels in the Inspector" added.

Not resolved here:

- The accelerators ⌥⌘N, ⌥⌘L and ⌥⌘W were not checked against macOS and Tauri's default menu items, only planned against `menus-v1.json`.
- Section 12's costs are reasoned estimates until perf:signoff runs (PERF-01).
- The body is about 795 lines (844 with this review), above the 750 aimed for; the additions are verification and oracle constraints.

Cross-layer review of 2026-10-30 against layers a, b and c (one line per change):

- Work items renamed `UI-n` (layer c also used `W1`…); dependencies now name `IM-`, `SS-` and `PF-` items instead of milestones.
- Principle 4, section 4, TST-01: provenance words were *source/named/inferred/none*; now *part*, *named*, *declared*, *profile*, *inferred*, *none* in layer b 3.3's precedence, as layers a and c use them.
- Section 6: the Articulation combo omitted `auto` and said "ordinario only for unprofiled presets", the reverse of layer b 5.1 (`ordinario` = the profile default; without a profile it acts as `auto`); now `auto`, `ordinario`, mapped ids, `legato` marked monophonic, and unprofiled presets list the combo as unavailable.
- Section 6, mockup, family table, Deviation 1, 7.8: Mute and Vibrato were bound to `mute`/`vibrato` parameters no layer provides in phase 1; they are filtered views of the `articulation` choices (layer b 5.7), with SS-34 optional after A16.
- Section 6, 7.3, accessibility, mockup: `dynamics` was labelled pp-ff, but layer c defines it as a ratio in `velocity` and `fixed` mode; pp-ff only in `cc` mode, otherwise "× written velocity" and `40·log10(d)` dB.
- Section 6, UI-17, Deviation 3, Risks, owner A6: `key_names` replaced by layer b's `keymap[].name`.
- Section 6 and UI-20: harp and timpani views relied on PLY-07, PLY-08 and PLY-11 findings that layer c runs only in wave 2; they now render layer c's derivations (PF-6, O4), and the findings join with wave 2; PLY-04 is wave 1 in layer c.
- 7.2 and TST-02: "lands at the start of the block that holds its beat" was the spine's wording that layer b corrects; now the first block start at or after the beat, with layer c's two-block lead rule as the marker.
- Keyboards rows, 7.7, TST-06, Alternatives: the N1 notice and the C11 tooltip were stale by O5 (layer b's SS-07 and SS-18 land in O2 and O3); removed or reworded; the N1 test dropped.
- Section 3: `section_emulated` added to the source sketch (layer b 2.2); C8 cites SS-08; family order cites layer a's `Family::score_rank()`.
- Section 9, section 10, Deviation 6: New Project and Add Ensemble… apply layer c's PF-20 plan (one source with the planner); the Orchestrate… sheet (PF-15) given its place.
- "Not resolved here": the note that layers b and c had no design is answered by this review and removed.
- Section 3: Ready and Partly now use layer b 7's one definition of *available* (the chain's), so the Library and the planner cannot disagree about a preset with a missing sample.
