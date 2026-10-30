# SFZ conformance of Sonara's sampler

This record checks, against Sonara's source code, every statement that
[sound-mapping.md](sound-mapping.md) makes about how Sonara loads and plays SFZ instruments: the SFZ
parser, the sampler, catalog discovery, the sampler's descriptor and parameters, the engine's note
schedule as the mapping uses it, and the Drum Kit's notes. It then lists SFZ behaviours that
sound-mapping.md did not cover when it was checked (its section 2.0 has since added N1-N6 as
rows), and proposes a fix and a pinning test for each defect. It is research: no code was changed.

## Method

- **Code.** Read on branch `research/orchestra-instruments`. `git diff a212544 HEAD -- crates apps`
  is empty, so the code is that of `main` at `a212544` (Release 0.1.11), and every line number
  below refers to that commit. Nothing was built, run, rendered or listened to.
- **Files.** Short names used below:

  | Short name | Repository path |
  |---|---|
  | `sfz.rs` | `crates/sonara-media/src/sfz.rs` |
  | `tests/sfz.rs` | `crates/sonara-media/tests/sfz.rs` |
  | `sampler.rs` | `crates/sonara-dsp/src/processors/sampler.rs` |
  | `tests/sampler.rs` | `crates/sonara-dsp/tests/sampler.rs` |
  | `bank.rs` | `crates/sonara-dsp/src/bank.rs` |
  | `drums.rs` | `crates/sonara-dsp/src/processors/drums.rs` |
  | `builtin.rs` | `crates/sonara-core/src/plugins/builtin.rs` |
  | `midi.rs` | `crates/sonara-core/src/midi.rs` |
  | `clip.rs` | `crates/sonara-core/src/model/clip.rs` |
  | `graph/mod.rs` | `crates/sonara-engine/src/graph/mod.rs` |
  | `graph/live.rs` | `crates/sonara-engine/src/graph/live.rs` |
  | `schedule.rs` | `crates/sonara-engine/src/schedule.rs` |
  | `control.rs` | `crates/sonara-engine/src/player/control.rs` |
  | `renderer.rs` | `crates/sonara-engine/src/renderer.rs` |
  | `samples.rs` | `crates/sonara-app/src/samples.rs` |
  | `discovery.rs` | `crates/sonara-app/src/instruments/discovery.rs` |
  | `model.rs` | `crates/sonara-app/src/instruments/model.rs` |
  | `args.rs` | `crates/sonara-agent/src/contract/args.rs` |
  | `gm.rs` | `crates/sonara-midi/src/gm.rs` |

- **Spec.** These pages of the SFZ format site (https://sfzformat.com/) were opened:
  `opcodes/include`, `opcodes/define`, `headers/control`, `opcodes/trigger`, `opcodes/sw_default`,
  `opcodes/sw_lokey`, `opcodes/sw_last`, `opcodes/lokey`, `opcodes/loccN`, `opcodes/on_loccN`,
  `opcodes/xfin_lovel`, `opcodes/amp_veltrack`, `opcodes/loop_mode`, `opcodes/off_mode` and
  `opcodes/pitch_keytrack`. `opcodes/loprog` was not opened.
- **Reference implementation.** The format site says nothing on some points: a mid-line
  `#include`, `$define` in an include path, and what `<control>` clears. For those, the reference
  player sfizz was read instead: `src/sfizz/parser/Parser.cpp` and `src/sfizz/Synth.cpp` on its
  `develop` branch (https://github.com/sfztools/sfizz). sfizz is a reference implementation, not the
  specification, and is labelled as such below. Its code was read through a fetch that summarizes
  pages, so treat the excerpts as close paraphrases of the code.
- **Library files.** The library files below were downloaded and read as plain text, to test the
  "Hits" columns of sound-mapping.md and to try to refute them:
  - VSCO 2 CE: `ViolinEns-KS`, `SViolin-KS`, `Flute-KS`, `Clarinet-KS`, `GM-StylePerc`,
    `ViolinEnsSusVib`, and one sustain WAV;
  - Sonatina: `1st Violins KS`, `Flutes KS`, `1st Violins Legato`, `Timpani`, `Grand Piano`, plus
    seven first-level includes and two `includes/` fragments;
  - Salamander V3: the main file plus `Data/notes.txt`, `pedal.txt`, `region.txt`, `vel_01.txt` and
    `tune_nat.txt`;
  - MTG Solo Sax: `MTG Baritone Sax`, `Data/group_bar.txt` and the repository tree.

  Any hit not in this list is marked "hit not re-read".
- **Libraries and licences.** The libraries named in this record, with their licences as the
  fact-checked card states them:

  | Name used here | Library | Licence | Card |
  |---|---|---|---|
  | VSCO 2 CE | VS Chamber Orchestra 2, Community Edition (branch `SFZ`) | CC0 1.0 | [Card cards/free_libraries.md#vsco_2_ce] |
  | VCSL | Versilian Community Sample Library, SFZ build at tag `v1.2.2-RC` | CC0 1.0 (some recordings carry third-party notes) | [Card cards/free_libraries.md#vcsl] |
  | Sonatina | Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 | [Card cards/free_libraries.md#sonatina_symphonic_orchestra] |
  | Salamander V3 | Salamander Grand Piano V3, sfzinstruments remap | CC BY 3.0 | [Card cards/free_libraries.md#salamander_grand_piano] |
  | Splendid | Splendid Grand Piano (AKAI) | public domain, per AKAI's statement as relayed by others | [Card cards/free_libraries.md#splendid_grand_piano] |
  | MTG Solo Sax | MTG Solo Saxophones (sfzinstruments `MTG.SoloSax`) | CC BY 4.0 | [Card cards/free_libraries.md#other_sources] |
  | Ixox Flute | Ixox Flute | CC BY 4.0 (catalogue) | [Card cards/free_libraries.md#other_sources] |
  | Bigcat cello | Karoryfer and Bigcat cello | CC0 1.0 (catalogue) | [Card cards/free_libraries.md#karoryfer_samples] |
  | jRhodes | jRhodes3d | samples CC BY-NC 4.0 | [Card cards/free_libraries.md#electric_pianos] |
  | Discord GM bank | Discord SFZ GM Bank | not on the card; sound-mapping.md: per instrument file, no repository licence | sound-mapping.md section 2.5 |

  The test fixtures proposed in section D are written by hand to reproduce a library's header
  order. They copy no file text and no samples, so none of these licences reaches the repository
  (LIC-01).
- **Verdicts.**
  - CONFIRMED: the code does what the claim says.
  - PARTLY: the core is right, but a stated detail or consequence is wrong or incomplete.
  - REFUTED: the code contradicts the claim.
  - "Render-only" marks the part of a claim that only listening to a render could settle.
  - The quotes show sound-mapping.md's current text, which has since adopted the corrections
    recorded here. A PARTLY verdict records what the check found in the earlier wording.

## Summary

- **Q1-Q11:** every code behaviour in the Q rows of sound-mapping.md is **CONFIRMED** by the
  source. None is refuted.
  - For the hits re-read, the effects follow from the code exactly as stated: VSCO
    `ViolinEns-KS`/`Flute-KS`/`Clarinet-KS`, Sonatina `1st Violins KS`/`Flutes KS`/Timpani/Legato/Grand
    Piano, Salamander, and MTG.
  - Two refinements, both now stated in sound-mapping.md's rows:
    - Q1 is triggered by any later header at the same scope level or above, `<group>` included, not
      only by `<control>` or `<global>`.
    - Q5's remapping is silent: `trigger` is a supported opcode, so `first`, `legato` and
      `release_key` never appear in the loader's ignored-opcode warning.
- **Other claims:** four are **PARTLY**; sound-mapping.md has since adopted all four corrections.
  - Seek and `sw_default`: a seek while playing does not reset the sampler, so it does not restore
    `sw_default`.
  - Catalog ids survive moving the library only with a manifest id or an unchanged folder name.
  - Drum Kit toms for the timpani fallback: in 41-50 the toms are on 41, 43, 45, 47, 48 and 50 only.
  - Verdicts built from the loader's ignored-opcode list plus the Q1-Q7 checks: the list
    undercounts, and Q11 is not among the checks.

  Everything else checked is CONFIRMED.
- **Behaviours sound-mapping.md did not list** (its section 2.0 now has N1-N6): one has hits, the
  rest are latent.
  - **N1 (has hits): release samples never play for a key released while the sustain pedal is
    down.** The spec and sfizz start them when the pedal lifts. This affects pianos with release
    samples whenever CC64 comes from imported MIDI or live input.
  - **N2-N6 (latent):**
    - N2: the ignored list undercounts.
    - N3: one missing or `*generator` sample fails the whole patch.
    - N4: `sw_*` written at region level are never read.
    - N5: a header glued to its first opcode is not recognised.
    - N6: loop points embedded in a sample file are ignored.

    For N5 and N6 the re-read files were checked. N5 has one harmless hit (Sonatina `Grand Piano`
    writes `<curve>curve_index=11`). N6 has none: VSCO's sustain WAV has no `smpl` chunk, and
    Sonatina's looped includes write explicit `loop_start` and `loop_end`.
- **Research cards:** these findings settle open question 14 of
  [cards/acoustic_pianos.md](cards/acoustic_pianos.md) by code reading, and make several "Sonara
  plays as is?" rows of [cards/free_libraries.md](cards/free_libraries.md) too optimistic (section E).

---

## A. The Q rows (sound-mapping.md section 2.0)

### Q1: key-switch settings are read once, at the end of the file

> "`sw_lokey`/`sw_hikey`/`sw_default` are read once, from the header scopes still open at the end of the file. Any later header at the same scope level or an outer one clears them: `<control>` clears all four scopes, `<global>` the inner three, `<master>` the inner two, and a later `<group>` without `sw_default` drops a group-level one. Values written at `<region>` level are never read (N4). The reference player sfizz instead takes the last `sw_default` parsed in any scope."

**Verdict: CONFIRMED.**

**Evidence.**

- `sfz.rs:269-280` builds the bank's only switch range from the four header scopes as they stand
  after the last token:
  ```rust
  let merged_scopes: Opcodes = scopes.iter().flat_map(Clone::clone).collect();
  let keyswitches = match (merged_scopes.get("sw_lokey").and_then(|v| note(v)),
                           merged_scopes.get("sw_hikey").and_then(|v| note(v))) {
      (Some(lo), Some(hi)) => Some(Keyswitches { lo, hi, default: merged_scopes.get("sw_default")... }),
      _ => None,
  };
  ```
- Each header clears its own level and every inner level, so any later header at the same level or
  above drops the values (`sfz.rs:218-233`):
  - `<control>` clears all four scopes;
  - `<global>` clears scopes 1-3 (`skip(1)`);
  - `<master>` clears scopes 2-3;
  - `<group>` clears scope 3.

  A later `<group>` without `sw_default` therefore loses a group-level `sw_default`.
- `sw_*` written at `<region>` level never count, because only `scopes` is read (N4).
- `sampler.rs:162-166`: `rest()` sets `last_switch = keyswitches.and_then(|k| k.default)`.
- `sampler.rs:487` plays a region only when `region.sw_last.is_none_or(|sw| Some(sw) == switch)`.
  - With `default: None`, every `sw_last` region is silent until a key in `lo..=hi` is pressed
    (`sampler.rs:233-237`).
  - With `keyswitches: None`, `last_switch` never changes, so every `sw_last` region stays silent
    for good.

**Hits re-read.**

- **VSCO `ViolinEns-KS`.**
  - Line 6 `sw_default=c2` sits only in the first `<group>`. Lines 238 and 241 then start
    `<control>` and `<group>`.
  - The later groups repeat `sw_lokey=c2 sw_hikey=d#2` (lines 243-244 and 1181-1182) but not
    `sw_default`.
  - At the end of the file the bank gets `lo=36, hi=39, default=None`, so the patch is silent until
    a switch key is played. CONFIRMED.
- **VSCO `Flute-KS` and `Clarinet-KS`:** same layout (last groups at lines 768-771 and 687-691).
  CONFIRMED.
- **VSCO `SViolin-KS`** repeats `sw_default=c2` in every group (lines 324, 615, 933, ...).
  sound-mapping.md's "as is" is CONFIRMED.
- **Salamander V3.**
  - `sw_lokey=$NATURAL sw_hikey=$RETUNED sw_default=$NATURAL` sit in the first `<global>`.
  - Two later `<global>` headers, for the release and pedal sections, clear them. The bank gets
    `keyswitches: None`, so the `sw_last=$NATURAL`/`$RETUNED` note regions could never play.
  - This is moot while Q2 holds, because those regions are never loaded at all. CONFIRMED.
  - `$NATURAL` is `C0`, which `note()` reads as 12 (`sfz.rs:321`), so sound-mapping.md's
    "KS 12 natural / 13 retuned" is right.

**Spec.**

- `opcodes/sw_last`: under SFZ 1 or 2 such an instrument "will play no sound until one of the
  keyswitches is pressed".
- `opcodes/sw_default`: "Define keyswitch 'power on default' so that you hear something when a patch
  loads".
- `opcodes/sw_lokey`: the range is "normally going to be the same for the entire instrument", usually
  under `<global>`.
- The spec does not say that a later header cancels an earlier `sw_default`.
- sfizz (reference) reads `sw_default` in `<global>`, `<master>` and `<group>` and in regions,
  "the last one parsed wins". It collects switch keys per region as a union (`lastKeyswitchLists_`,
  `swLastSlots_`). So Sonara diverges from the reference player.

---

### Q2: `#include` is read only at the start of a line

> "`#include` is honoured only at the start of a line."

**Verdict: CONFIRMED.**

**Evidence.** `sfz.rs:104-108`:
```rust
let line = line.split("//").next().unwrap_or("").trim();
if let Some(rest) = line.strip_prefix("#include") {
    let name = rest.trim().trim_matches('"').replace('\\', "/");
    out.push_str(&read_preprocessed(&dir.join(name), defines, depth + 1)?);
```

**Trace of a Salamander line** (`Data/notes.txt`):
```
<group> #include "Data/vel_01.txt" lovel=1 hivel=26 #include "Data/region.txt"
```
1. The line does not start with `#include`, so it reaches the tokenizer as text.
2. `<group>` resets `current` (`sfz.rs:214`). The first `#include` and its path have no `=` and no
   open opcode, so they are dropped.
3. `lovel=1` and `hivel=26` go to the group scope.
4. The second `#include` and its path are then **appended to `hivel`'s value** as continuation tokens
   (`sfz.rs:256-265`). `hivel` becomes `26 #include "Data/region.txt"`, which no longer parses as a
   number, so it falls back to 127.
5. `region.txt` holds every note `<region>`, and it is never read.

The patch still loads, because `pedal.txt` (read) and the other include lines start at column 0.
So it plays noises but no piano notes. sound-mapping.md's "note regions never loaded" is right.

**Hits.** Salamander re-read: CONFIRMED. Discord master and Ixox Flute: hit not re-read.

**Spec.** `opcodes/include` is silent on where `#include` may appear in a line. Its only placement
rule is that, once every include has been expanded, the `<control>` header must come before the
regions. sfizz (reference) parses a character stream and dispatches on `#` wherever a token starts
(`switch (reader.peekChar()) { case '#': processDirective(); ...`), so `<group> #include "x"` works
there. sound-mapping.md's spec column now says the same: the page is silent on placement, and sfizz
reads an include wherever a token starts.

---

### Q3: `$NAME` defines are not expanded in `#include` paths

> "`$NAME` defines are substituted in opcode lines, not in `#include` paths."

**Verdict: CONFIRMED.**

**Evidence.**

- The `#include` branch (`sfz.rs:106-108`) never expands defines.
- Only ordinary lines are expanded (`sfz.rs:114-123`):
  `for (name, value) in names { if line.contains(name.as_str()) { line = line.replace(...) } }`.
- What happens to a path that holds a define:
  1. `#include "$DIR/Res.txt"` opens the literal path `<patch dir>/$DIR/Res.txt`, which does not exist.
  2. The read fails with `MediaError::Io` (`sfz.rs:101`), and `load_sfz` fails.
  3. The graph turns that into the warning "instrument … cannot be loaded" and a silent device
     (`graph/mod.rs:1131-1134`).

  So sound-mapping.md's "load error, silence plus warning" is CONFIRMED.

**Hits.** Sonatina `Grand Piano` re-read: line 19 `#define $DIR Data`, line 84
`#include "$DIR/Res.txt"`, lines 91-125 `#define $DYN …` followed by `#include "$DIR/$DYN.txt"`
(the includes on lines 93, 101, 109, 117 and 125). CONFIRMED. Its header names it a conversion of
the Splendid Grand Piano, which agrees with sound-mapping.md's "as Splendid". Splendid itself: hit
not re-read.

**Spec.** `opcodes/define` shows variables only in opcode values and never in an `#include` path. It
warns that redefining a variable between includes "does not work well in ARIA/Sforzando". sfizz
(reference) expands them in include paths: `path = expandDollarVars({ valueStart, valueEnd }, path);`.

---

### Q4: a nested `#include` resolves against the including file's directory

> "A nested `#include` resolves against the including file's directory."

**Verdict: CONFIRMED.**

**Evidence.** `sfz.rs:102`, `let dir = path.parent()` of the file being read, then `sfz.rs:108`,
`dir.join(name)`. Samples, by contrast, resolve against the main file (`sfz.rs:50,352-358`), which is
correct.

**Hit.** MTG `MTG Baritone Sax.sfz`, re-read:

- line 84 includes `Data/group_bar.txt`;
- `group_bar.txt` line 4 includes `Data/bar_p_rr1.txt`;
- Sonara therefore opens `Data/Data/bar_p_rr1.txt` and gets an `Io` error. The whole patch fails:
  silence plus a warning. CONFIRMED.
- The repository tree holds the same layout for the other saxophones: eight patches (soprano, alto,
  tenor and baritone, each also `(NL)`) and four `Data/group_*.txt` files. Only the baritone files
  were re-read.

**Spec.** `opcodes/include`: when included files sit in another folder, the SFZ is still read as if
it were in the main file's folder, not in the folder of the included file. sfizz (reference) uses
`_originalDirectory / path`.

---

### Q5: every `trigger` other than `release` becomes an attack

> "Every `trigger` other than `release` becomes an attack. The remap is silent: `trigger` is a supported opcode, so `first`, `legato` and `release_key` never appear in the loader's ignored-opcode warning (likewise an unknown `loop_mode` value becomes no loop)."

**Verdict: CONFIRMED.**

**Evidence.**

- `sfz.rs:389-392`:
  ```rust
  region.trigger = match opcodes.get("trigger").map(String::as_str) {
      Some("release") => Trigger::Release,
      _ => Trigger::Attack,
  };
  ```
- `bank.rs:12-18` has only `Attack` and `Release`.
- `trigger` is in `SUPPORTED` (`sfz.rs:171`). So `trigger=first`, `trigger=legato` and
  `trigger=release_key` are **never listed** in `ignored_opcodes`, and they never reach the graph's
  "uses opcodes the sampler does not play" warning (`graph/mod.rs:1118-1123`).
- The same holds for an unknown `loop_mode` value, which becomes `NoLoop` (`sfz.rs:397-403`).

**Hits re-read.**

- **Sonatina `Timpani`.** Lines 229-231 put `loop_mode=one_shot trigger=release_key` on the roll
  endings (for example `offset=87538`). In Sonara they start at note-on, together with the looped roll
  (lines 179-182 and following). CONFIRMED.
- **Sonatina `1st Violins Legato`.**
  - The file has two `<group>`s, both `group=1 off_by=1 offset=20000 off_mode=time off_time=1`.
  - The first is `trigger=first ampeg_attack=0.30`; the second is `trigger=legato ampeg_attack=0.2`.
  - Both `#include` the same `includes/1st-violins-sustain.sfz`.
- **Code trace of one note-on** (`sampler.rs:247-258`):
  1. The first group's region matches, and `choke(1)` fades the previous note's voices over
     `CHOKE_RELEASE` = 10 ms (`sampler.rs:50,315-325`). The region's voice starts with `level: 0.0`
     (`sampler.rs:357`).
  2. The second group's region matches the same key, and `choke(1)` releases that just-started voice
     at level 0. `start_release` sets a step of `EPSILON/…` (`sampler.rs:505-506`), and the next
     render frame drops the voice before it makes any sound.
  3. The legato group's voice starts.

  The net result is one voice per note, with the legato group's 0.2 s attack, monophonic, and the
  previous note faded in 10 ms. sound-mapping.md's outcome is CONFIRMED, including that the first
  group's voice "is dropped before it makes any sound".

  sound-mapping.md gives both attack times. Both groups start at `offset=20000`; only
  `ampeg_attack` differs (0.2 s against 0.30 s).
- Bigcat cello, MTG legato, Ixox Flute, Sonatina All Unpitched Percussion and Organ Combinations:
  hits not re-read.

**Spec.** `opcodes/trigger`:

- `first`: "Region will play on note-on, but if there's no other note going on".
- `legato`: "only if there's a note going on".
- `release_key`: "Region will play on note-off. Ignores sustain pedal."

**Render-only:** how bad the retriggered legato sounds.

---

### Q6: velocity crossfades are ignored

> "`xfin_*`/`xfout_*` velocity crossfades are ignored."

The effect column says "both layers sound on every note, each at its normal velocity gain without
the crossfade attenuation".

**Verdict: CONFIRMED.**

**Evidence.**

- `xfin_lovel`, `xfin_hivel`, `xfout_lovel`, `xfout_hivel` and `xf_cccurve` are not in `SUPPORTED`
  (`sfz.rs:154-187`). They are recorded as ignored (`sfz.rs:244-246`) and have no effect.
- Without `lovel`/`hivel` a region's velocity range is 0-127 (`sfz.rs:376-377`).
- Each layer's gain is `db_to_amp(volume) × (v/127)²` (`sampler.rs:344-345`).
- So each layer keeps its normal velocity gain and loses only the crossfade attenuation, as the
  effect column says. In the 70-90 crossfade zone and outside it, both layers sound.

**Hit.** Sonatina `Timpani`, re-read:

- the f group has `xfin_lovel=70 xfin_hivel=90` (lines 13-14);
- the p group has `xfout_lovel=70 xfout_hivel=90` (lines 55-56);
- neither sets `lovel`/`hivel`, so both play on every hit. CONFIRMED.

`xfin_locc1` in the Sonatina Chorus - Performance patches is reported as `xfin_locc*`
(`sfz.rs:289-295`) and is equally inert. Hits not re-read: those chorus patches, Sonatina Celeste,
VCSL Concert Harp, Glockenspiel and Marimba, and jRhodes.

**Spec.** `opcodes/xfin_lovel`: the region's volume is zero for velocities at or below
`xfin_lovel`.

**Render-only:** how audible the doubled layer is.

---

### Q7: CC gates are ignored, and a key of `-1` falls back to the full range

> "`locc`/`hicc` gates are ignored, so gated regions always play. A key value of `-1` fails to parse and falls back to the full 0-127 range."

**Verdict: CONFIRMED** (both sentences).

**Evidence.**

- `locc*`/`hicc*`/`on_locc*` are not supported (`sfz.rs:154-187`), and the existing test pins
  `locc*` as ignored (`tests/sfz.rs:115`).
- `note("-1")`: `"-1".parse::<i32>()` succeeds, then `u8::try_from(-1)` fails, so the result is
  `None` (`sfz.rs:300-301`).
- `region.key_lo = key_of("lokey").unwrap_or(region.key_lo)` (`sfz.rs:373-374`) then keeps the
  defaults `key_lo: 0, key_hi: 127` (`bank.rs:108-109`).

**Hit.** Salamander `Data/pedal.txt`, re-read:

- Lines 1-6 set `lokey=-1 hikey=-1 group=1`. No header precedes them in the file, and the main
  file's third `<global>` (line 105) is the last header before `#include "Data/pedal.txt"`
  (line 112), so they land in that `<global>`'s scope and every pedal region inherits them.
- The groups use `on_locc64=127 … locc64=127` and `on_locc64=0 … locc64=0`.
- Each group has two regions split by `lorand`/`hirand`.
- In Sonara every note-on therefore starts **one pedal-down and one pedal-up noise**. They are also
  transposed by `key − 60`, because `pitch_keycenter` defaults to 60 (`bank.rs:112`) and
  `pitch_keytrack` is not supported. CONFIRMED; the effect column states both noises and the
  transposition.

Hits not re-read: Bigcat cello CC107, Sonatina and Splendid piano pedal layers.

**Spec.**

- `opcodes/lokey`: SFZ 2 also allows `lokey` and `hikey` of -1, so that no key triggers the region.
- `opcodes/on_loccN` "effectively replaces the default trigger=attack".
- `opcodes/loccN`: "Defines the range of the last MIDI controller N required for the region to play."

---

### Q8: gain is `volume × velocity²` for every region

> "Gain is `volume × velocity²` for every region; `amp_veltrack`, `amp_velcurve_*` and the other `*_veltrack` opcodes are ignored. This fixed curve equals the spec's default (`amp_veltrack=100`)."

**Verdict: CONFIRMED.**

**Evidence.** `sampler.rs:344-345`:
```rust
let velocity = f64::from(event.velocity) / 127.0;
let gain = db_to_amp(region.volume_db) * velocity * velocity;
```
None of these opcodes is supported.

**Spec.** `opcodes/amp_veltrack` gives the default (100 %) as `Gain(v) = 20·log10[(v/127)²]` dB. So
Sonara's fixed curve **equals the spec default**; only regions that set a non-default
`amp_veltrack` or `amp_velcurve_N` diverge. That narrows Q8 to organ, noise and harpsichord regions
that set `amp_veltrack=0`.

**Hits:** Sonatina Organ Combinations, Harpsichord KS and Performance strings, VCSL Pipe Organ,
Knight upright and Flemish harpsichord: not re-read.

---

### Q9: envelope, filter, LFO, pan and overlap opcodes are ignored

> "`ampeg_decay`/`ampeg_sustain`/`ampeg_hold`, filters …, LFOs, `pan`/`width`, `off_mode`/`off_time`, `note_polyphony`, `pitch_keytrack`, `rt_decay` and `offset_random` are ignored. Every choke uses a fixed 10 ms release."

The effect column adds: "Sonatina legato's 1 s overlap (`off_mode=time off_time=1`) becomes a 10 ms
choke".

**Verdict: CONFIRMED.**

**Evidence.**

- None of the listed opcodes is in `SUPPORTED` (`sfz.rs:154-187`).
- `choke()` always uses `CHOKE_RELEASE = 0.01` (`sampler.rs:50,322`), whatever `off_mode` says.
- The sampler's only envelope is a linear attack and release (`sampler.rs:346,358,420-428`).

**Hit.** `1st Violins Legato` sets `off_mode=time off_time=1` in both groups (re-read). In ARIA terms
that is a 1 s overlap; Sonara gives 10 ms. CONFIRMED.

**Spec.** `opcodes/off_mode`: "the default is fast", fast = "6 ms"; `normal` = "the region will be
set into release stage". Sonara's 10 ms is close to the default fast mode, but it ignores `normal` and
`time`. `opcodes/pitch_keytrack`: default 100; 0 makes "all notes in the region … play the same
pitch".

---

### Q10: a region with an empty key range never matches

> "A region with an empty key range (`lokey` above `hikey`) never matches."

**Verdict: CONFIRMED.**

**Evidence.** `sampler.rs:485`: `(region.key_lo..=region.key_hi).contains(&key)` is empty when
`lo > hi`. That is harmless for key triggering. The CC-triggered sound those regions carry is lost
anyway (Q7, `on_locc`).

**Hit:** VCSL Knight upright, not re-read.

---

### Q11: a `<control>` header clears all four scopes

> "A `<control>` header clears all four scopes: `<control>`, `<global>`, `<master>` and `<group>`."

**Verdict: CONFIRMED.** Sonara diverges here from ARIA and from sfizz. The spec itself does not
commit either way.

**Evidence.** `sfz.rs:218-221`:
```rust
"<control>" => {
    scopes = Default::default();
    open_scope = 0;
}
```

**Hits re-read.**

- **Sonatina `1st Violins KS`.**
  - It has eight `<global>` blocks, each holding `sw_lokey=c2 sw_hikey=a2 sw_default=c2` and one
    `sw_last`, followed by a line-start `#include`.
  - First-level includes that contain `<control>`: `Sustain`, `Marcato`, `Harmonics` and `Legato`
    (re-read). `Staccato` and `Pizzicato` contain none (re-read). `Col Legno` and `Tremolo` were not
    re-read.
  - The last include, `Harmonics`, has a `<control>`, so the scopes at the end of the file hold no
    `sw_lokey`. The bank gets `keyswitches: None`.
  - The articulations from files with `<control>` lost their `sw_last`, so they play on every note.
  - `Staccato` and `Pizzicato` kept `sw_last` (a `<group>` clears only scope 3). With no switch range
    they can never be selected.

  sound-mapping.md's row is CONFIRMED for the files re-read.
- **Sonatina `Flutes KS`.** Six `<global>` blocks. `Flutes Staccato` and `Flutes Sustain (looped)`
  contain `<control>` (re-read); the other four includes were not re-read. If all six have it, as
  sound-mapping.md says, every articulation sounds on every note.
- The other seven key-switch files that sound-mapping.md section 2.2 lists were not re-read.

**Spec and reference.**

- `headers/control`: the page says it is not entirely clear whether a new control header should
  reset settings other than `default_path`, and that ARIA resets only `default_path`, not the other
  control settings. Nothing there says that `<global>` is cleared.
- sfizz (reference) does only `defaultPath_ = ""; … handleControlOpcodes(members);` on `<control>`.
  It does not touch `globalOpcodes_`, `masterOpcodes_` or `groupOpcodes_`.

---

## B. Other parser, sampler, catalog and Drum Kit claims

The claims come from sound-mapping.md section 1, its sections 2.0, 2.1, 2.5, 3.1, 3.2 and 4.1, and
the timpani row and closing note of section 4.3.

| Id | Claim (short, verbatim) | Verdict | Evidence |
|---|---|---|---|
| C1 | "a `sonara.sampler` device whose enum parameter `instrument` lists the installed patches' `.sfz` paths; the project stores that path"; "not canonicalized, so it is absolute only when the configured root is" | CONFIRMED | `builtin.rs:767-798`: `ep("instrument", "Instrument", first, &choices)`, where `choices` comes from `samples.rs:45-54` `scan_patches`. Paths are the configured root joined with the entry, **not canonicalized**, so they are absolute only if the setting is. |
| C2 | "The `instrument` parameter is built with `automatable: false`" | CONFIRMED | `builtin.rs:84-91`, `fn ep … automatable: false`. |
| C3 | "A patch that is missing or unreadable renders as silence with a warning; it does not fail the render." | CONFIRMED | Three paths. No library configured: the device is bypassed, with the warning "plugin … is unavailable; bypassed" (`graph/mod.rs:978-986`). Path not installed: "… is not installed; it plays silence" (`graph/mod.rs:1074-1084`). Load error: "… cannot be loaded" (`graph/mod.rs:1131-1134`). Not checked: whether project loading in `sonara-app`/`sonara-project` accepts a device whose `plugin_id` is not registered. This verdict covers the render only. |
| C4 | catalog id `<library id>/<patch path below the library root, without the extension>`; "It survives moving the library to another parent folder, but renaming the library folder changes every id unless a manifest declares the `id`." | **PARTLY** | `model.rs:90-95` builds `format!("{library}/{stem}")`. The library id is the manifest `id`, or else a slug of the **directory name** (`discovery.rs:148-157`, `model.rs:49-67`). Without a manifest, renaming the folder changes every id; moving it under another parent does not. |
| C5 | library id slug: "only ASCII letters and digits are kept, lower-cased, other runs become `-`"; `VSCO-2-CE` gives `vsco-2-ce` | CONFIRMED | `model.rs:49-67`: only ASCII letters and digits are kept, a trailing `-` is trimmed, at most 64 characters, `library` when empty. |
| C6 | "No agent tool, CLI command or GUI uses the catalog yet." | CONFIRMED | `InstrumentCatalog` is used outside its module only in `crates/sonara-app/tests/instrument_catalog.rs` and `crates/sonara-agent/tests/contracts.rs:19,227-241`. There are no uses in `apps/`. |
| C7 | "The scan lists every `.sfz` file below a root (8 levels; hidden entries and symbolic links skipped; extension matched without regard to case). The sampler's cap of 5,000 patches covers all roots together." | CONFIRMED | `samples.rs:21,23,56-82`. Also: hidden entries and symbolic links are skipped, and the extension check ignores case. The sampler's 5,000 cap covers all roots together (`samples.rs:45-54`). |
| C8 | include fragments: "The scan lists them as presets … plain discovery shows it as `available`"; "A deep `verify` marks it `invalid`" | CONFIRMED | `discovery.rs:266-292` `check_file` checks only that the file is regular, non-empty, inside the library and openable. `verify` (`discovery.rs:99-118`) runs the patch loader, which loads the SFZ. Two traces: `includes/1st-violins-sustain.sfz` names `sample=../Samples/1st Violins/…`, which resolves against `includes/` to the missing `Strings - Notation/Samples/…`, so the load fails with `Io` and the preset is Invalid. `includes/vibrato.sfz` has no `<region>`, so `Malformed` and Invalid. The count of 190 is confirmed from the GitHub tree. A consequence, which sound-mapping.md now also states: the fragments are **also choices of the sampler's `instrument` enum**, because both use `scan_patches`. |
| C9 | "The sampler ignores the MIDI channel at note-on and uses it only to match note-offs" | CONFIRMED | `note_on` (`sampler.rs:228-262`) never reads the channel. `note_off` filters on `voice.channel != event.channel` (`sampler.rs:277`). `last_switch`, `key_down` and the release-region start are also blind to the channel. |
| C10 | "The engine sorts notes stably by start frame … note-offs, then note-ons, then controller events. Two notes that start on the same frame keep their clip order." | CONFIRMED | `schedule.rs:143-151` (`sort_by_key`, which is stable), `schedule.rs:233-234` (the merge order), and `clip.rs:261-297`: `playback_notes` keeps the clip's stored note order. |
| C11 | "After a seek, the engine sends an all-notes-off, then a note-on for every note that started before and ends after the new position (the chase). A short key-switch note that ended earlier is not re-sent. A seek while playing, and a loop wrap, do not reset the sampler, and all-notes-off leaves the switch alone, so the switch last pressed before the jump stays active. The sampler's `rest()`, which restores `sw_default`, runs only when the graph is prepared or reset: on play from stop and in an offline render." | **PARTLY** | The chase is confirmed: `schedule.rs:205-209` (`n.on < frame && n.off > frame`). But **a seek while playing does not reset the graph**: `control.rs:795-796` says "without a reset (the next block chases)", and `graph/live.rs:19-20` says "Start, seek and loop wraps still chase". `AllNotesOff` (`sampler.rs:192-200`) does not touch `last_switch`. So after a seek, or at a loop wrap, the **switch last pressed before the jump stays active**, not `sw_default`. `rest()` runs only from `prepare`/`reset`/`set_state` (`sampler.rs:537-552,579-581`), that is, on play-from-stop and offline render (`control.rs:1387,1393`; `renderer.rs:211`). sound-mapping.md section 3.3 advises holding the switch note for the whole passage, so that the chase re-sends it; that is the workaround that makes this safe. |
| C12 | the supported-opcode list "is" the 29 names given; "Every other opcode 'has no effect'" | CONFIRMED | `sfz.rs:10-15,154-190`. sound-mapping.md also names the aliases `loopmode`, `loopstart` and `loopend`. Opcodes under an unknown header (`<curve>`, `<effect>`, `<midi>`, `<sample>`) are skipped **without** being reported (`sfz.rs:235,241-243`); see N2. |
| C13 | key switches use "the SFZ convention c4 = 60, so `c2` = 36 and `c6` = 84" | CONFIRMED | `sfz.rs:321`: `(octave + 1) * 12 + semitone + accidental`. The VSCO `sw_last` values c2, c#2, d2 and d#2 give 36-39. `opcodes/sw_lokey` allows names from "C-1 to G9". |
| C14 | "`sw_down` etc. not in the supported list" | CONFIRMED | `sfz.rs:154-187`. |
| C15 | "CC7 and CC11 act as linear volume (value/127); CC1/CC21 do nothing" | CONFIRMED | `sampler.rs:204-226` handles only 7, 11 and 64, each as a linear gain of `value/127` (7 and 11). |
| C16 | "`loprog` ignored; clips cannot hold a program change" | CONFIRMED | `loprog` is not supported. `midi.rs:126-133` defines `ControlKind { Cc, ChannelPressure, PitchBend }`. The sampler also ignores channel pressure (`sampler.rs:191`). |
| C17 | "An agent can write switch notes with the existing note tools, since a switch is an ordinary note" | CONFIRMED | `args.rs:691-703`: `NoteInput.pitch` is a "MIDI number 0-127" or a note name. `sampler.rs:233-237` sets the switch, and then the same key **also** triggers any region mapped to it (`sampler.rs:247-258`; the test comment at `tests/sampler.rs:211`). sfizz appears to do the same; its layer gating (`Layer::registerNoteOn`) was not read. |
| C18 | velocity gain and release scale "come from the sampler" | CONFIRMED | `sampler.rs:345`. `release_scale` is clamped to 0.1-4 (`sampler.rs:68`) and applied at `sampler.rs:217,290`. |
| C19 | "Sonara bends ±2 st" (`bend_up/down` ignored) | CONFIRMED | `sampler.rs:52,189`. |
| C20 | "Drum Kit plays the 34 notes of `GM_MAP`, which include 36, 38, 39, 49, 51, 54, 70, 75, 76 and 77 but not 80/81 … or any note above 77" | CONFIRMED | `gm.rs:5-40` and `drums.rs:244-277` (`from_note` matches the same 34 notes; 69 and 70 are both `Shaker`, 76 and 77 are `BlockHigh`/`BlockLow`). |
| C21 | "Cinematic Toms" kit "holds 'big low toms and long cymbals'" | CONFIRMED | `builtin.rs:732-734`: `tom_tune = 0.7`, `cymbal_decay = 3.5` against the default 1.6 (`builtin.rs:707`). |
| C22 | `timpani` fallback: "Drum Kit 'Cinematic Toms', toms on notes 41, 43, 45, 47, 48 and 50 only (pitch lost)" | **PARTLY** | The toms are on 41, 43, 45, 47, 48 and 50 only. Notes 42, 44 and 46 are hi-hats and 49 is a crash (`drums.rs:250-259`). The kit's own description says "41-50 toms" (`builtin.rs:748`), which has the same gap. The other fallback notes are right: 36 kick, 38 snare, 39 clap, 49 crash, 51 ride, 54 tambourine, 70 shaker, 75 claves, 76/77 blocks. |
| C23 | percussion key map "yes"; `GM-StylePerc` plays "as is" | CONFIRMED | All 220 regions are single-key, with `pitch_keycenter` equal to the key (re-read). The only opcodes outside the supported list are `ampeg_dynamic` (with the harmless `group_label`, `sw_label`). Whether losing `ampeg_dynamic` is inaudible is a **render-only** question. |
| C24 | sampler verdicts built "from a deep load plus structural checks for Q1-Q7, Q11 and N1-N6; the loader's ignored-opcode list is one input, not the verdict" (section 3.2, `sonara_verdict`); "List the losses from the verdict: the loader's ignored-opcode list plus the structural checks of section 2.0, because the list alone undercounts (N2)" (section 4.1) | **PARTLY** | The list exists and is surfaced as a render warning (`graph/mod.rs:1118-1123`). It **undercounts** (N2). The earlier wording's Q1-Q7 checks covered the structural Q1-Q4 and the silent Q5 remap, but not Q11, which stacks or silences the articulations of Sonatina key-switch files, nor N1 or N3; the current wording adds them. A message that lists only the losses the loader reported still understates. |

---

## C. Behaviours sound-mapping.md does not list

sound-mapping.md section 2.0 has since added N1-N6 as rows; N7 is not listed there.

**N1: release samples are lost under the sustain pedal (hits).**

- `note_off` returns before the release-region loop when the pedal is down (`sampler.rs:287-295`):
  ```rust
  if pedal { voice.held_by_pedal = true; } else { start_release(...) }
  …
  if pedal { return; }
  ```
- On pedal-up, `control_change(64)` only releases the held voices (`sampler.rs:209-221`). It never
  starts the `Trigger::Release` regions.
- Spec, `opcodes/trigger`: release = "Region will play on note-off or sustain pedal off".
- sfizz (reference) queues these as delayed sustain releases, which start on pedal-up
  (`startDelayedSustainReleases`).
- Affected: pianos with `trigger=release` regions, played with CC64.
  - Salamander V3 (re-read): the string-resonance and hammer-noise sections sit under a `<global>`
    with `trigger=release` and are included at the start of a line, so they load, and they lose
    their tails under the pedal.
  - VCSL `Grand Piano, Kawai` and `Upright Piano, Knight`: release samples per sound-mapping.md
    section 2.3; not re-read.
  - VCSL `Grand Piano, Steinway B`: its SFZ was not opened. [cards/acoustic_pianos.md](cards/acoustic_pianos.md)
    lists a `Rel` sample folder.
  - Sonatina `Grand Piano`: its main file has no `trigger=` opcode at all (re-read); the included
    `Data/*.txt` files were not read. It fails to load anyway while Q3 holds.
- CC64 reaches a project only through `import_midi` or live input, because no tool writes CC.

**N2: the ignored-opcode list undercounts (latent).** Two gaps:

- `trigger` and `loop_mode` values that Sonara does not support are remapped silently (Q5).
- Opcodes under `<curve>`, `<effect>`, `<midi>` and `<sample>` are dropped without being reported
  (`sfz.rs:235,241-243`).

Salamander (main file line 116, re-read) uses a stand-alone `<curve>`, and it is skipped today. MTG
`MTG Baritone Sax` (re-read) does too, which matters only once Q4 is fixed. jRhodes and Sonatina
Vibraphone and Celeste use `<curve>` as well, according to sound-mapping.md (not re-read).
Sonatina `Grand Piano` writes its `<curve>` glued to the first opcode, so its curve is reported
instead of skipped (N5).

**N3: one unresolved sample fails the whole patch (latent).**

- `Loader::region` propagates any sample error (`sfz.rs:359`, `AudioStream::open(file)?` at
  `sfz.rs:418`).
- This includes the spec's generator samples. `opcodes/on_loccN` uses `sample=*silence`.
- So a Discord GM bank stub (`sample=*sine`) fails to load: it gives silence plus a warning, not a
  sine.
- One missing file in a 1,000-region patch silences the whole instrument.
- Also: the `load_sfz` doc says a region without a sample is `Malformed` (`sfz.rs:44-45`), but the
  code skips it (`sfz.rs:343-345`). This is a doc defect (QG-05).

**N4: `sw_lokey`, `sw_hikey` and `sw_default` written at `<region>` level are never read (latent).**
Only header scopes feed the range (`sfz.rs:269`). This is a variant of Q1. No hit in the files read.

**N5: a header glued to its first opcode is not recognised (latent, one harmless hit).**

- Headers must stand alone as whitespace-separated tokens (`sfz.rs:212-213`, `token.starts_with('<') && token.ends_with('>')`).
- `<region>sample=a.wav` becomes an opcode named `<region>sample`, so the region is lost.
- The format site's own `#define` example is written this way: `<region>key=$KICKKEY sample=kick.wav`.
- One re-read file does this: Sonatina `Grand Piano` line 68, `<curve>curve_index=11`. Sonara
  reads the token as an unknown opcode `<curve>curve_index` in the open `<global>` scope
  (`sfz.rs:244-254`), and the curve's `v000`-`v127` points land there too. All are unknown, so they
  are reported as ignored (`<curve>curve_index`, `v*`) and have no effect. Harmless, and moot while
  Q3 fails the patch. No other re-read file glues a header to an opcode.

**N6: loop points embedded in a sample file are ignored (latent, no hit).**

- `loop_mode` defaults to `NoLoop`, and `loop_continuous` without `loop_start`/`loop_end` loops the
  whole region from frame 0 (`sfz.rs:397-409`).
- Spec, `opcodes/loop_mode`: the default is "loop_continuous for samples with defined loop(s)", taken
  from the file's loop metadata.
- No hits found:
  - The VSCO sustain WAV re-read (`VlnEns_susVib_A2_v1.wav`) has only `fmt ` and `data` chunks,
    and no `smpl` chunk.
  - Sonatina's looped includes give explicit points, for example `flutes-sustain-looped.sfz`
    `loop_start=108289 loop_end=203511`.

**N7, minor:** `#define` keeps only the first word of a value that contains spaces (`sfz.rs:110-112`).

---

## D. Verdicts and follow-ups

The test files are:

- `crates/sonara-media/tests/sfz.rs` for loader tests (named "sfz:" below);
- `crates/sonara-dsp/tests/sampler.rs` for sampler tests (named "sampler:" below);
- `crates/sonara-engine/src/schedule.rs` for the engine test in the C11 row.

Each sfz test uses a hand-written fixture that reproduces the real library's header layout
(TST-06). A test marked "pin current" documents today's behaviour until a fix lands. Every sampler
fix runs on the audio thread and must stay allocation-free and panic-free (RT-02, RT-05).

| Claim id | Verdict | Consequence for which libraries | Suggested fix (one line) | Test that would pin it |
|---|---|---|---|---|
| Q1 | CONFIRMED | VSCO `ViolinEns-KS`, `Flute-KS`, `Clarinet-KS`: silent until a switch key. Salamander: switch range lost, which matters once Q2 is fixed. | Take `sw_lokey`/`sw_hikey` as the union over all regions' merged opcodes, and use the last `sw_default` seen in any scope, as sfizz does. | sfz: `sw_default_in_the_first_group_survives_later_groups` (VSCO layout: `<control>`, then `<group>` with `sw_default`, then `<control>` and `<group>` without it; expect default 36). |
| Q2 | CONFIRMED | Salamander V3 (no note regions); Discord master and Ixox Flute (not re-read). | Run `#include`/`#define` as tokens anywhere in the stream, not only at the start of a line. | sfz: `an_include_after_a_header_on_the_same_line_is_read` (`<group> #include "v.txt" lovel=1 hivel=26 #include "r.txt"`; expect the regions, with `hivel` 26). |
| Q3 | CONFIRMED | Sonatina Grand Piano; Splendid (not re-read). The patch fails: silence plus a warning. | Expand `$defines` in the `#include` path before resolving it. | sfz: `defines_expand_in_include_paths` (`#define $DIR Data` + `#include "$DIR/x.txt"`). |
| Q4 | CONFIRMED | MTG Solo Sax: the baritone patch fails (re-read); the other seven share the layout. | Pass the main file's directory through `read_preprocessed` and join every include to it. | sfz: `nested_includes_resolve_against_the_main_file` (`Data/a.txt` includes `Data/b.txt`). |
| Q5 | CONFIRMED (the remap is silent) | Sonatina Timpani roll endings sound at note-on. Every Sonatina `* Legato` becomes a monophonic retrigger. Bigcat cello, MTG, Ixox Flute, All Unpitched Percussion and Organ Combinations (not re-read). | Add `First`/`Legato`/`ReleaseKey` to `Trigger`, gate them on `key_down` and ignore the pedal for `release_key`; until then, report unsupported `trigger=` values. | sampler: `release_key_plays_on_note_off_even_with_the_pedal_down`, `legato_region_plays_only_while_another_key_is_held`; sfz: `unsupported_trigger_values_are_reported`. |
| Q6 | CONFIRMED | Sonatina Timpani (both layers on every hit), Sonatina Chorus - Performance (CC1). Sonatina Celeste, VCSL Concert Harp, Glockenspiel, Marimba and jRhodes (not re-read). | Implement `xfin`/`xfout` on velocity as a per-voice gain set at note-on (allocation-free, RT-02). | sampler: `velocity_crossfade_scales_each_layer` (two regions with `xfin`/`xfout` 70-90; at v=80 each layer at its crossfade gain). |
| Q7 | CONFIRMED | Salamander: pedal-down and pedal-up noise on every key, transposed. Bigcat cello CC107 layers stack (not re-read). | Treat `lokey=-1`/`hikey=-1`/`key=-1` as "no key"; drop or skip regions gated by `on_locc` until CC triggers exist. | sfz: `minus_one_keys_disable_key_triggering` (expect the region to match no key). |
| Q8 | CONFIRMED (the default curve matches the spec) | Organs and noise regions with `amp_veltrack=0` (Sonatina, VCSL; not re-read). | Support `amp_veltrack` as `gain = 1 − t + t·(v/127)²`. | sampler: `amp_veltrack_zero_ignores_velocity`. |
| Q9 | CONFIRMED | Sonatina legato overlap (1 s becomes 10 ms), "(looped, decay)" variants, filters and LFOs across Sonatina and Karoryfer. | Honour `off_mode=normal` (use the region's release on choke) first; the envelope stages later. | sampler: `off_mode_normal_chokes_over_the_region_release`. |
| Q10 | CONFIRMED | VCSL Knight upright pedal regions stay silent (harmless). | None needed. | sampler: `an_empty_key_range_never_plays` (pin current). |
| Q11 | CONFIRMED (diverges from ARIA/sfizz; the spec is non-committal) | Sonatina `1st Violins KS` (4 articulations stack, 4 never sound), `Flutes KS`, and per sound-mapping.md most other Sonatina key-switch files. | A new `<control>` resets only the control scope (`default_path`), never `<global>`, `<master>` or `<group>`. | sfz: `control_header_keeps_the_enclosing_global_switch` (Sonatina layout: `<global> sw_* sw_last=c2`, then an include that starts with `<control>`; expect `sw_last` 36 and the range kept). |
| C4 | PARTLY | Any library without a manifest whose folder is renamed: the preset ids change. | Document that ids survive a move only with a manifest `id` or an unchanged folder name. | catalog test: `renaming_a_library_without_manifest_changes_its_ids` (pin current). |
| C8 | CONFIRMED (+ fragments are sampler choices) | Sonatina: 190 fragments listed as available and offered in the sampler's `instrument` list. | Skip `.sfz` files below a folder named `includes`, or files with no `<region>` after preprocessing. | `samples.rs` unit test: `include_fragments_are_not_patches`. |
| C11 | PARTLY | Every key-switch patch, after a seek or loop wrap while playing: the pre-jump articulation persists. | Document the "hold the switch note" rule (sound-mapping.md section 3.3); a real fix needs the chase to re-send the last switch note before the position. | engine: `a_seek_while_playing_keeps_the_last_switch` (pin current); sampler: `all_notes_off_keeps_the_switch` (pin current). |
| C22 | PARTLY | Built-in fallback for `timpani`: only 6 of the notes 41-50 are toms. | Change the kit description at `builtin.rs:748` to "41, 43, 45, 47, 48, 50", as sound-mapping.md now reads. | Already pinned by `gm.rs` `GM_MAP`; no new test. |
| C24 | PARTLY | Any `sonara_verdict` built from `ignored_opcodes` plus Q1-Q7 misses Q11, N1 and N3, and the skipped headers of N2. | Derive verdicts from loader checks plus structural checks (Q1-Q5, Q7, Q11), not the ignored list alone. | See the N2 row. |
| N1 | new (hits) | Salamander V3 resonance and hammer releases (re-read); VCSL Kawai and Knight (not re-read); VCSL Steinway B and Sonatina Grand Piano unconfirmed (see section C). Only when CC64 is used (imported MIDI, live input). | Remember the keys released under the pedal (a fixed `[bool; 128]`) and start their release regions on CC64 up (RT-02-safe). | sampler: `release_samples_play_when_the_pedal_lifts` (note-on, CC64=127, note-off, CC64=0; expect the tail at the note's velocity). |
| N2 | new (latent) | Salamander, jRhodes and Sonatina Vibraphone and Celeste (`<curve>`; MTG once Q4 is fixed); every Q5 file. The warnings understate. | Report unsupported `trigger=`/`loop_mode=` values and skipped headers (`<curve>`, `<effect>`) in `ignored_opcodes`. | sfz: `unsupported_values_and_headers_are_reported`. |
| N3 | new (latent) | The Discord GM bank (`*sine`); any library using `*silence`; any patch with one missing file. | Treat `*silence` as a silent region, skip other `*` generators with a warning, and report a missing sample per region instead of failing the patch. | sfz: `a_generator_sample_does_not_fail_the_patch`. |
| N4 | new (latent) | None found. | Part of the Q1 fix: read `sw_*` from merged region opcodes. | sfz: `region_level_switch_range_is_read`. |
| N5 | new (latent, one harmless hit) | Sonatina Grand Piano's `<curve>curve_index=11` (reported as ignored, no effect). Any file written like the format site's examples (`<region>key=…`) would lose regions. | Split a token at `>` so that `<region>sample=a.wav` is a header plus an opcode. | sfz: `a_header_glued_to_its_first_opcode_is_parsed`. |
| N6 | new (latent, no hit) | None found (the VSCO WAV re-read has no `smpl` chunk; Sonatina gives explicit points). | Read WAV `smpl` loop points as the default `loop_mode`/`loop_start`/`loop_end`. | sfz: `embedded_loop_points_are_the_default` (a WAV fixture with a `smpl` chunk). |

The other claims are CONFIRMED (C1-C3, C5-C7, C9, C10, C12-C21, C23) and need no follow-up.

---

## E. What this means for the research cards

The cards judged the libraries from their opcodes and from Sonara's documentation. The code
reading above adds structural failures that the opcode lists do not show:

- **[cards/acoustic_pianos.md](cards/acoustic_pianos.md), open question 14 (Salamander in the Sonara
  sampler).** Settled by code reading, not by a render. The sfzinstruments V3 file loads but plays no
  piano notes (Q2); its pedal noises fire on every key, transposed, rather than never (Q7); its
  natural/retuned switch is lost (Q1). Verdict: no, until Q2 is fixed.
- **[cards/free_libraries.md](cards/free_libraries.md), "Sonara plays as is?" rows.**
  - VSCO 2 CE, "Mostly": right about the opcodes, but `ViolinEns-KS`, `Flute-KS` and `Clarinet-KS`
    are silent until a switch key is pressed (Q1).
  - Sonatina, "Partly": the key-switch files that include `<control>`-headed files stack some
    articulations and never select others (Q11); Timpani roll endings sound at note-on (Q5) and both
    dynamic layers sound on every hit (Q6); `Grand Piano` fails to load (Q3). The card's legato
    description matches Q5, except that the first group's voice is dropped before it sounds rather
    than released over 10 ms.
  - Salamander (sfzinstruments remap), "partly": no, as above.
  - Splendid, "Partly": the patch fails to load (Q3), per sound-mapping.md; Splendid itself was not
    re-read here, but its Sonatina copy was.
- **Cards that say the sampler plays release triggers and CC64**
  ([cards/double_reeds.md](cards/double_reeds.md) and [cards/clarinets.md](cards/clarinets.md);
  [cards/electric_keys.md](cards/electric_keys.md) names CC64 sustain only): true, except that a
  release sample never plays for a key released under the pedal (N1).
- **VCSL `Grand Piano, Steinway B`.** [cards/acoustic_pianos.md](cards/acoustic_pianos.md) calls it
  raw WAVs that need a generated SFZ, from a listing of the `master` branch;
  [cards/free_libraries.md](cards/free_libraries.md) says the SFZ patches ship only at tag
  `v1.2.2-RC`, and sound-mapping.md lists a `Grand Piano, Steinway B` preset from that tag. Not
  settled here; the patch was not opened.

---

## Review notes

- Retitled the record "SFZ conformance of Sonara's sampler" and made it stand-alone: an
  introduction, a short-name-to-path table for every source file, and the commit it was checked at
  (branch `research/orchestra-instruments`, code identical to `main` at `a212544`). Removed the
  earlier branch-local commit hashes and the reference to a source number of another file.
- Replaced the local download folder with "the library files were downloaded and read as plain
  text", kept the list of files read, and added the MTG repository tree to it.
- Replaced the internal working name of the verified file with a link to
  [sound-mapping.md](sound-mapping.md) and its section numbers; converted first-person wording to
  neutral voice.
- Re-checked every cited line reference against `a212544`; all point at the code they describe.
  Five examples: `sfz.rs:269-280` (switch range from the merged scopes), `sampler.rs:287-295`
  (`note_off` returns while the pedal is down), `graph/mod.rs:1118-1123` (ignored-opcode warning),
  `drums.rs:250-259` (toms and hi-hats between 41 and 50), `control.rs:795-796` (seek "without a
  reset"). Corrected two references: the Sonatina `Grand Piano` `$DYN` includes run to line 125, not
  109; the Salamander note-name parse is `sfz.rs:321` only.
- Paraphrased five spec quotes longer than 15 words (`opcodes/include` twice, `opcodes/xfin_lovel`,
  `opcodes/lokey`, `headers/control`); the shorter quotes stay.
- N1: the affected list now separates the re-read hit (Salamander releases) from the unread ones
  (VCSL Kawai, Knight), and marks VCSL Steinway B and Sonatina Grand Piano unconfirmed: the Steinway
  SFZ was not opened, and the Sonatina main file has no `trigger=` opcode and fails under Q3 anyway.
- Q4: "all 8 patches" now says that only the baritone was re-read and the other seven share the
  layout in the repository tree. Q7: confirmed that the pedal opcodes land in the third `<global>`.
- N5 corrected: the earlier "no downloaded file does this" was wrong. Sonatina `Grand Piano` glues
  `<curve>curve_index=11`; the effect is harmless (reported as ignored opcodes). N2 adjusted to
  match: that file's curve is reported, not skipped; MTG's stand-alone `<curve>` added as a
  consequence that applies only once Q4 is fixed.
- C24 now quotes the current sound-mapping.md wording ("plus Q1-Q7 checks") and says what those
  checks still miss (Q11, N1, N3).
- Added the libraries-and-licences table (from cards/free_libraries.md), the note that test fixtures
  copy no library text (LIC-01), the RT-02/RT-05 constraint on sampler fixes, and section E, which
  lists where these findings settle or contradict the research cards.
- The only instrument id used is `timpani`, which exists in [model.md](model.md).
- Re-check against the corrected sound-mapping.md, Method: the quotes show its current text, which
  has adopted this record's corrections; a PARTLY verdict records what the earlier wording got wrong.
- Introduction, Summary and section C: sound-mapping.md section 2.0 now lists N1-N6, so they are
  behaviours it "did not list"; the section C heading is kept, with a note under it.
- Summary: the Q1 and Q5 refinements are now stated in sound-mapping.md's rows.
- Summary: the four PARTLY corrections are adopted there; the C4 and C22 bullets no longer quote the
  replaced wording.
- Q1: quote is now the full row (scope levels, N4, sfizz); "the second sentence is incomplete"
  dropped; the Salamander quote is now "KS 12 natural / 13 retuned".
- Q2: the spec column no longer carries an Estimate; it now gives the page's silence and sfizz.
- Q5: quote now includes the silent-remap sentence, so "does not say that the remapping is silent"
  dropped.
- Q5 legato: "slightly loose" and "shorter attack" replaced; sound-mapping.md now says the first
  group's voice "is dropped before it makes any sound" and gives both attack times.
- Q5, Q6 and Q8 hits (and the Q5 and Q6 rows of section D): added the hits sound-mapping.md names
  that were not re-read: All Unpitched Percussion, Sonatina Celeste, Sonatina Performance strings.
- Q6: effect quote is now "each at its normal velocity gain without the crossfade attenuation"; the
  "full level" gloss removed.
- Q7: "slightly worse than stated" replaced; the effect column now states both noises and the
  transposition.
- Q8: quote now names "the other `*_veltrack` opcodes" and the spec-default sentence; the remark
  that `veltrack` is not an opcode name removed.
- Q9: quote adds "Every choke uses a fixed 10 ms release."; the effect quote adds
  `(off_mode=time off_time=1)`.
- Section B introduction: the claims also come from sections 2.1 (C13, C23), 2.5 (C19), 3.1
  (C14-C17, C23) and the note that closes 4.3 (C20, C21).
- C1, C5, C7 and C15: quotes updated to the current wording (no "absolute"; the ASCII slug rule;
  hidden entries, case and the all-roots cap; "linear volume (value/127)").
- C4, C11 and C22: quotes updated to the corrected wording; verdicts stay PARTLY (see Method); the
  C22 fix in section D now names only `builtin.rs:748`.
- C8: "would mark" is now "marks"; the sampler-choice consequence is no longer called unstated.
- C12: sound-mapping.md now names the aliases `loopmode`, `loopstart` and `loopend`.
- C24: both quotes updated (deep load plus structural checks; losses from the verdict); the evidence
  now ties the missing Q11, N1 and N3 to the earlier wording. Supersedes the C24 note above.
- Checked, no change: the Q2-Q4, Q10 and Q11 quotes; C2, C3, C6, C9, C10, C13, C14, C16-C21 and
  C23; "as Splendid", "note regions never loaded"; sections 2.2, 2.3 and 2.5 as cited.
