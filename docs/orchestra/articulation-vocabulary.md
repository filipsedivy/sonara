# Articulation vocabulary: harmonization across the instrument cards

_Synthesized 2026-10-30 from the instrument cards in [cards/](cards/); the counts were re-read from the card
json blocks after their harmonization. Status: research input for the orchestra design; nothing here is
implemented. Five of the six merges proposed here have been applied to the card json blocks. The sixth
(`col_legno_battuto` into `col_legno`) and every other proposal in this file wait for the vocabulary owner's
decision._

## Sources

| # | Source | Path | Kind | Opened |
|---|---|---|---|---|
| S1 | The instrument cards: the json block of each of the 85 instruments, section 5 (articulations), the "Open questions" and the review notes of each file; the Apple card's §2.2-2.3 (Studio Strings and Studio Horns key switches) and §3 (Articulation Sets, articulation IDs). Cited as `[Card cards/<file>.md#<id or section>]` or `[Card cards/<file>.md OQ<n>]` | [cards/](cards/) | research cards | yes |
| S2 | The 15-id `Articulation` enum, its doc comments, `from_id` and the test `unknown_ids_are_not_guessed`; `Instrument::articulation_availability` | `crates/sonara-midi/src/orchestration/articulation.rs`, `crates/sonara-midi/src/orchestration/instrument.rs` | repository | yes |
| S3 | The `sonara-library.json` manifest: each instrument's `articulations`, kept as trimmed, de-duplicated strings | `crates/sonara-app/src/instruments/manifest.rs` | repository | yes |
| S4 | Project format v1: the MIDI note `{id, pitch, start, length, velocity, channel}`, the clip `controls` kinds `cc`, `pitch_bend` and `channel_pressure`, and the objects that keep unknown keys | `docs/project-format.md`, `crates/sonara-project/schemas/project-v1.schema.json` | repository | yes |
| S5 | The agent tools: the note tools take notes only; `create_automation` targets mixer and device parameters | `docs/agent-api.md`, `crates/sonara-agent/src/contract/args.rs` | repository | yes |
| S6 | Device-parameter automation and the sampler descriptor: device parameters are sampled once per block and an enum is automated by choice index; the enum helper `ep` builds non-automatable parameters; the sampler is registered beside the 12 golden-pinned built-in descriptors, only when a configured library holds patches | `crates/sonara-engine/src/automation.rs` (module header), `crates/sonara-core/src/plugins/builtin.rs`, `crates/sonara-app/src/samples.rs`, `golden/plugins/catalog.json` | repository | yes |
| S7 | Sonara's SFZ loader and sampler, verified claim by claim (rows Q1-Q11, C1-C24, N1-N7) | `crates/sonara-media/src/sfz.rs`, `crates/sonara-dsp/src/processors/sampler.rs`; [sfz-conformance.md](sfz-conformance.md) | repository and its verification record | yes |
| S8 | Sound mapping: §3.1 (how libraries switch articulations), §3.3 (sending articulations under today's contracts) and §4.2 (articulation substitution) | [sound-mapping.md](sound-mapping.md) | research synthesis | yes |
| S9 | The piano roll: keyboard, note grid and velocity lane; no file mentions articulations | `apps/sonara-desktop/src/lib/pianoroll/` | repository | yes |

Musical facts come from the cards, which carry their own citations. This file adds the harmonization
judgements, tagged [Estimate: ...] where they are judgements. The status of each id (existing, preferred,
invented) is defined in "Canonical vocabulary".

## Summary

- Before harmonization, the 85 instrument json blocks used **101 distinct ids**: the 15 existing ones and 86
  proposed ones. Six proposed ids are spelling variants or synonyms of another id ("Variant mapping").
- Five of those merges **have been applied** to the card json blocks: `multitongue`, `double_tonguing` and
  `triple_tonguing` into `multi_tongue`; `muted_string` into `palm_mute`; `pitch_bend` into `portamento`.
- The json blocks now use **96 distinct ids**: 15 existing and 81 proposed (47 from the preferred list, 34
  invented by the cards). "Canonical vocabulary" lists all 96.
- The sixth merge, `col_legno_battuto` into the existing `col_legno`, is **not applied**. It is an open
  owner decision (1.4). If the owner accepts it, the vocabulary has 95 ids, 80 of them proposed.
- Two named collision groups are **not** merges: `mute_*` against `con_sordino`, and `trill_half` /
  `trill_whole` against `trill`. They are refinements of a general id, not synonyms. Section 1.4 lists
  them with the other near-misses that were checked and kept apart.
- Four ids mean different things in different cards (`damped`, `tremolo`, `staccato`, `sustain`). That is
  a card-usage problem, not a spelling one; section 1.5 proposes fixes for the card owners.
- The card prose (section 5 tables, open questions, fact-check logs) still uses the old spellings in the
  five cards that changed, and the clarinets card names them. The json blocks are authoritative (1.6).
- "Modelling notes" lists options for modelling mutes, vibrato, contact point, trill interval, dynamic shapes and
  eight more dimensions as separate axes, against Logic's flat per-note list. It decides nothing.

## Variant mapping

### 1.1 Rules applied

1. **An existing id is never a variant.** The 15 ids of `articulation.rs` are the spellings that
   `sonara-library.json` manifests declare: the module doc says the slugs match the manifest spelling "so a
   later mapping is a string match" [Cited S2], and the manifest keeps each declared articulation as a
   plain string [Cited S3]. Renaming an id would break installed libraries' declarations. When a proposed
   id means the same as an existing id, the proposed id is mapped onto the existing one.
2. **Preferred spellings win over invented ones** when two ids mean the same thing.
3. **Among invented spellings**, the one that follows the existing pattern wins: the vocabulary writes
   tonguing techniques as `<kind>_tongue` (`flutter_tongue`, and the preferred `slap_tongue`).
4. **Only true synonyms are mapped.** A more specific technique (`trill_half`, `mute_cup`,
   `harmonics_natural`) is a refinement of a general id, not a synonym; it stays a canonical id of its own.
5. **The mapping normalizes card data before data entry.** It is not a proposal for runtime aliases:
   `Articulation::from_id` is deliberately strict, and the test `unknown_ids_are_not_guessed` rejects
   `pizz` and `Pizzicato` [Cited S2].

### 1.2 Mapping table

| Variant id | Canonical id | In the card json | Cards that used the variant | Why they are the same |
|---|---|---|---|---|
| `multitongue` | `multi_tongue` | applied | 7: piccolo, flute, alto_flute, bass_flute, horn_f, wagner_tuba_tenor, wagner_tuba_bass | The cards define both as fast repeated or running notes by double (t-k) or triple (t-k-t) tonguing [Card cards/flutes.md#piccolo; Card cards/clarinets.md#clarinet_bb]. The clarinets card asked for one spelling across groups [Card cards/clarinets.md OQ5]. `multi_tongue` follows `flutter_tongue` and `slap_tongue` (rule 3); BBCSO writes "Multi-tongue", Spitfire "Multitongue" [Card cards/clarinets.md#clarinet_bb; Card cards/flutes.md#piccolo]. |
| `double_tonguing` | `multi_tongue` | applied | 5: trumpet_bb, trumpet_c, trumpet_piccolo, cornet, flugelhorn | Defined as fast repeated or detached notes articulated T-K, which libraries realize as repetition patches [Card cards/high_brass.md#trumpet_bb]. Double or triple is chosen for the rhythm (duplets or triplets); it is one technique, and the clarinets card already folds Orchestral Tools' separate "Double Tongue" and "Triple Tongue" patches into one id [Card cards/clarinets.md#bass_clarinet]. BBCSO and Spitfire record one multi-tongue patch, VSL "Performance repetitions" [Card cards/clarinets.md#clarinet_bb; Card cards/high_brass.md#trumpet_bb]. The pattern can be a parameter or a library variant (3.3) [Estimate: judgement]. |
| `triple_tonguing` | `multi_tongue` | applied | 4: trumpet_bb, trumpet_c, trumpet_piccolo, cornet | As `double_tonguing` (T-K-T). |
| `muted_string` | `palm_mute` | applied | 1: piano_prepared | Both sound a string while a hand rests on it, giving a dry, short, still pitched note [Card cards/acoustic_pianos.md#piano_prepared; Card cards/plucked.md#guitar_classical]. The pianos card's own fact-check recommends `palm_mute` "to keep the vocabulary small" [Card cards/acoustic_pianos.md, fact-check log, part 7 "Remaining concerns"]. Rule 2. |
| `pitch_bend` | `portamento` | applied | 1: vibraphone | The vibraphone card defines it as a struck bar's pitch lowered by sliding a mallet toward its centre, at most a semitone to a whole tone [Card cards/tuned_percussion.md#vibraphone]. The harp card already maps the same gesture on a harp, a sounding string bent by its pedal, to `portamento` [Card cards/plucked.md#harp; Card cards/plucked.md OQ11]. The spelling `pitch_bend` also clashes with the project format, whose MIDI clip controls have the kinds `cc`, `pitch_bend` and `channel_pressure` [Cited S4] [Estimate: naming judgement]. The vibraphone keeps `glissando` for its sweep across the bars. |
| `col_legno_battuto` | `col_legno` | **not applied**: open owner decision (1.4) | 4: violin, viola, cello, double_bass, which still list it next to `col_legno` | The existing doc comment defines `col_legno` as "Strings struck with the wood of the bow", which is col legno battuto [Cited S2]. The strings card files `col_legno` and `col_legno_battuto` in one row for the struck stroke, beside `col_legno_tratto` for the drawn one [Card cards/strings.md#violin]. Spitfire files its plain "Col Legno" among the Shorts [Card cards/strings.md#cello]. Rule 1 keeps the existing spelling; `col_legno_tratto` stays a separate id. |

### 1.3 The applied mapping as json

This is the only json block in this file, so a script can find it unambiguously. Keys are variant ids,
values are canonical ids. It has already been applied to `articulations_existing_vocab` and
`articulations_proposed_new` in every card json block; each of the five cards that changed (flutes, horns,
high_brass, acoustic_pianos, tuned_percussion) records it in its review notes. It stays here to normalize
any other text or data that still uses the old spellings, such as the card prose (1.6): apply it to both
lists, then de-duplicate each list.

```json
{
  "multitongue": "multi_tongue",
  "double_tonguing": "multi_tongue",
  "triple_tonguing": "multi_tongue",
  "muted_string": "palm_mute",
  "pitch_bend": "portamento"
}
```

`col_legno_battuto` is deliberately left out of the block, because the merge is not decided. If the owner
accepts it, add `"col_legno_battuto": "col_legno"`. The four string cards then drop `col_legno_battuto`
from their proposed lists; they already list `col_legno` among the existing ids, so nothing moves between
the lists. The applied merges removed one entry each from `trumpet_bb`, `trumpet_c`, `trumpet_piccolo` and
`cornet`, which had listed both `double_tonguing` and `triple_tonguing` and now list `multi_tongue` once.

### 1.4 Considered and kept distinct

Each pair or group below was checked for a merge. None is a synonym; the "relation" column says what the
orchestra design can do with it instead.

| Ids | Why they stay distinct | Relation |
|---|---|---|
| `con_sordino` vs `mute_cup`, `mute_harmon`, `mute_plunger`, `mute_bucket` | `con_sordino` is the general mute, and for brass the straight mute when no type is named; cup is darker, harmon penetrating (stem in) or subdued (stem out), bucket covered and horn-like [Card cards/high_brass.md#trumpet_bb] | refinements; mute axis option (3.3) |
| `stopped` vs `con_sordino` | hand-stopping raises the horn's pitch by a semitone, so the written note changes; a non-transposing mute keeps the pitch [Card cards/horns.md#horn_f] | fallback `stopped` to `con_sordino`; mute axis |
| `trill` vs `trill_half`, `trill_whole` | the interval is information that `trill` does not carry; libraries record each interval as its own patch [Card cards/clarinets.md#bass_clarinet; Card cards/strings.md#violin] | refinements; trill-interval axis option |
| `trill` vs `shake` | the jazz shake is a wide oscillation between adjacent harmonics; Logic shares one key ("Shake (Horns) / Trill (Saxes)"), and the saxophone cards map that key to `trill` [Card cards/high_brass.md#trumpet_bb; Card cards/saxophones.md#saxophone_alto] | fallback `shake` to `trill` |
| `shake` vs `shake_stroke` | unrelated: brass lip shake against one shake gesture of a rattle [Card cards/untuned_percussion.md#tambourine] | none; the names look alike only |
| `tremolo` vs `tremolo_measured` | measured repetition follows a notated subdivision and must be synchronized [Card cards/strings.md#violin] | refinement; metering option |
| `tremolo` vs `fingered_tremolo` | `tremolo` is one note repeated [Cited S2]; the wind and string fingered tremolo alternates two notes wider than a second. Four card groups asked for this split [Card cards/clarinets.md OQ4; Card cards/double_reeds.md OQ9; Card cards/flutes.md OQ11; Card cards/saxophones.md OQ10] | distinct |
| `tremolo` vs `roll` | both are existing ids. `tremolo`'s doc comment says "bowed or mallet" [Cited S2], while every library read calls the mallet tremolo a "roll"; the cards use `roll` for timpani and drums and `tremolo` for mallets [Card cards/tuned_percussion.md OQ12] | definitional overlap; metering option (3.3) |
| `roll` vs `thumb_roll` | a tambourine thumb roll is one way of making a roll, by friction [Card cards/untuned_percussion.md#tambourine] | fallback `thumb_roll` to `roll` |
| `multi_tongue` vs `flutter_tongue` vs `tremolo` | Widor calls double-tongued repetition on the flute "a true tremolo"; the flutes card leaves open whether the flute should list `tremolo` [Card cards/flutes.md#flute; Card cards/flutes.md OQ11] | distinct; open card question |
| `harmonics` vs `harmonics_natural`, `harmonics_artificial` | the existing id covers "natural or artificial harmonics" [Cited S2] | refinements |
| `harmonics` vs the preferred `overblown` | the flute and saxophone cards record their overblown partials under `harmonics` [Card cards/flutes.md#flute; Card cards/saxophones.md#saxophone_soprano]; no card uses `overblown` | `overblown` gets no row ("Canonical vocabulary") |
| `col_legno` vs `col_legno_tratto` | struck with the wood against drawn with the wood [Card cards/strings.md#violin] | distinct |
| `pizzicato` vs `bartok_pizzicato`, `left_hand_pizzicato`, `slap_pizzicato` | snap against the fingerboard, left-hand pluck, jazz slap on the beat [Card cards/strings.md#violin; Card cards/strings.md#double_bass] | refinements; fallback to `pizzicato` |
| `slap_pizzicato` vs `bartok_pizzicato` | both snap the string against the fingerboard; the jazz slap is a rhythmic bass style [Card cards/strings.md#double_bass] | fallback `slap_pizzicato` to `bartok_pizzicato` |
| `portato` vs `loure` | the strings card defines them apart, after VSL: portato "slightly accented notes on one stroke", loure "pulsed notes in one bow, legato impression" [Card cards/strings.md#violin]. Common usage often treats louré as another name for portato [Estimate: general knowledge, no source opened] | kept apart because one library vocabulary separates them; fallback `loure` to `portato`; candidate merge for the owner |
| `spiccato` vs `sautille` vs `ricochet`; `marcato` vs `martele`; `detache` vs `staccato` | different bow strokes with different libraries' patches [Card cards/strings.md#violin] | distinct; fallbacks `sautille` and `ricochet` to `spiccato` |
| `sul_tasto` vs `flautando` | Spitfire records both separately (flautando "flute-like", tasto a soft normale) [Card cards/strings.md#violin] | fallback `flautando` to `sul_tasto` |
| `sul_ponticello` vs `pres_de_la_table` | the same idea, exciting the string near its fixed end for a thin metallic tone, on bowed strings and guitar against the harp, where près de la table is plucking near the soundboard [Card cards/plucked.md#harp]; the preferred list keeps the harp's own term | distinct names; contact-point axis option |
| `palm_mute` vs `xylophonic` | the same gesture, a hand damping the string while the other plucks, on guitar against harp [Card cards/plucked.md#harp]; the preferred list keeps the harp's own term. `muted_string` (invented) was merged instead (1.2) | fallback `xylophonic` to `palm_mute`; mute axis option |
| `glissando` vs `portamento` | `glissando` is a deliberate slide through a range, continuous or stepwise; `portamento` is an expressive slide between two notes, or a bend of one sounding note, without a new attack. The timpani's pedal slide stays `glissando`: it is a notated "gliss." over an interval [Card cards/tuned_percussion.md#timpani] | distinct |
| `glissando` vs `half_valve` | half-valve is a tone colour and the valved brass's substitute for a glissando; the trumpet card lists both [Card cards/high_brass.md#trumpet_bb; Card cards/low_brass.md#tuba] | distinct |
| `scoop` vs `rip` vs `doit` vs `fall` | direction and place: scoop bends up into the note from just below; rip sweeps up fast into a loud note; doit slides up off the end; fall slides down off the end. The trombone card cites Spitfire for `rip` ("fast sweep up to a note"), the trumpet card only estimates it; `scoop` is invented [Card cards/low_brass.md#trombone_tenor; Card cards/high_brass.md#trumpet_bb; Card cards/saxophones.md#saxophone_soprano] | distinct; note-boundary option (3.3) |
| `accent` vs `marcato` vs `sforzando` vs `fortepiano` | graded accents, and a loud attack that falls away at once; the trumpet card groups "accent / sforzando" in one row but lists two ids [Card cards/high_brass.md#trumpet_bb] | distinct; dynamic-shape axis option |
| `let_ring` vs `sustain` | `sustain` is the note type (the library's long or default patch); `let_ring` is how it ends (pedal held, laissez vibrer); the plucked cards use them together [Card cards/plucked.md#harp] | distinct; release option |
| `damped` vs `dead_stroke` vs `choke` | `damped` stops the ring at the written end; `dead_stroke` damps from the attack; `choke` grabs a ringing cymbal at once. Choke is close to a minimal-length `damped`, but the preferred list has both [Card cards/untuned_percussion.md#clash_cymbals; Card cards/tuned_percussion.md#vibraphone] | fallback `choke` to `damped`; candidate merge for the owner |
| `snares_off` vs `con_sordino` | loosened snares against a covered head; Widor leaves the choice to the player for "sans timbre" [Card cards/untuned_percussion.md OQ18] | distinct; owner may prefer one id |
| `slap_tongue` vs `slap_pizzicato` | wind tongue slap against bass string slap | none |
| `key_click` (key noise) vs key slap | the double-reeds card maps VSL's "key slaps" onto `key_click` [Card cards/double_reeds.md#oboe] | one id, as the card chose |
| `bisbigliando` (harp) vs `bisbigliando` (English horn) | a soft same-pitch shimmer in both: the harp alternates strings [Card cards/plucked.md#harp]; on a woodwind it is a timbral trill between fingerings of one pitch [Estimate: the English-horn card lists the id without defining it, Card cards/double_reeds.md#english_horn] | one id |
| `air_tone` vs `subtone` vs `jet_whistle` vs `whistle_tone` vs `whisper` | breath noise; a soft breathy pitched sax tone; an air burst; faint high partials; unvoiced voice | distinct |
| `humming` vs `vowel` | Voices of Rapture records "Mm" as one of its vowels [Card cards/voices.md#voice_soprano] | distinct ids; vowel-parameter option |
| `cuivre` vs `stopped` | the horn's cuivré is an overblown stopped note; on trumpet and trombone it is a forced brassy tone without stopping [Card cards/horns.md#horn_f; Card cards/low_brass.md#trombone_tenor] | distinct |
| `wah_wah` vs `mute_plunger` | the plunger moving against the plunger held still [Card cards/high_brass.md#trumpet_bb] | distinct |
| `strum` vs `rasgueado`; `rim` vs `rimshot` | rasgueado is a flamenco kind of strum; rim alone against head and rim together [Card cards/plucked.md#guitar_classical; Card cards/untuned_percussion.md#snare_drum] | fallback `rasgueado` to `strum` |

**Open decision (the owner's): `col_legno_battuto` into `col_legno`. Not applied in the cards.** Until it
is decided, the json blocks of violin, viola, cello and double_bass list both `col_legno` (existing) and
`col_legno_battuto` (proposed) for the same struck stroke [Card cards/strings.md#violin], and the count of
96 ids includes both. `col_legno_battuto` breaks the pattern of the other preferred refinements. `trill`,
`harmonics` and `tremolo` keep the existing id as the unspecified parent, because their doc comments are
general. `col_legno`'s doc comment is specific ("struck"), so the merge keeps that meaning and drops a
preferred spelling. The alternative is to widen `col_legno`'s definition to "struck or drawn" and keep
`col_legno_battuto` as a refinement; that changes the meaning of an id that manifests may already declare
[Estimate: judgement].

### 1.5 Same id, different meanings (card usage to review)

These ids are spelled consistently, but some cards use them for a different technique. The harmonization
of the json blocks did not touch these usages, and they are still in the cards as described here; the
right-hand column is a proposal for the card owners.

| Id | Card usage that differs from the canonical definition | Proposed card change |
|---|---|---|
| `damped` | clavinet: the mute slider down, a dull, dry tone from the attack [Card cards/electric_keys.md#clavinet] | `palm_mute` (felt or hand on the strings while they sound) |
| `damped` | triangle: "muted stroke (hand on the triangle)", the GM "Mute Triangle" [Card cards/untuned_percussion.md#triangle] | `dead_stroke` (struck while damped), or `con_sordino` |
| `damped` | triangle: "choke of an open note" [Card cards/untuned_percussion.md#triangle] | `choke` |
| `damped` | guitar: "damping with cloth or mutes" [Card cards/plucked.md#guitar_classical] | if a mute on the strings throughout: `palm_mute` or `con_sordino`; if stopping the ring: keep `damped` |
| `tremolo` | acoustic pianos: "alternation of notes, octaves or chords"; harp: "two hands alternating" [Card cards/acoustic_pianos.md#piano_concert_grand; Card cards/plucked.md#harp] | two-note alternations become `fingered_tremolo`; repeated single notes or chords stay `tremolo`; the harp's two strings on one pitch stay `tremolo` |
| `staccato` | xylophone and marimba: the ordinary single stroke ("short by nature"); vibraphone and glockenspiel list it in their json too, and the glockenspiel card names `staccato` an alias of `damped` [Card cards/tuned_percussion.md#xylophone; Card cards/tuned_percussion.md#marimba; Card cards/tuned_percussion.md#glockenspiel] | the ordinary stroke carries no articulation (default question, 3.3); untuned percussion already writes "plain note" with no id |
| `sustain` | acoustic pianos, harpsichord, celesta, harp, guitar, lute, mandolin: the ordinary, freely decaying note, chosen "so that default patches match" [Card cards/plucked.md OQ12; Card cards/acoustic_pianos.md OQ15; Card cards/historical_keys_organs.md#harpsichord]; tuned percussion writes `let_ring` "/ default" instead | same default question; until it is decided the usage is consistent with the "ordinary note" half of the definition |

Usages that differ in mechanism but fit the canonical definition, so no change is proposed:
`con_sordino` on percussion is a cloth or cover (timpani coperto, bass drum, snare) and in winds a pad or
cloth in the bell [Card cards/tuned_percussion.md#timpani; Card cards/double_reeds.md#oboe]; `vibrato_on` is the
vibraphone's motor and the clavichord's Bebung [Card cards/tuned_percussion.md#vibraphone; Card
cards/historical_keys_organs.md#clavichord]; `harmonics` on flute and saxophones is overblown partials;
`portamento` on the harp is the pedal bend.

### 1.6 Where the counts come from, and the card prose

The counts in "Canonical vocabulary" were read with a script from the `articulations_existing_vocab` and
`articulations_proposed_new` lists of the json blocks of all 85 instruments in [cards/](cards/), after the
five applied merges. [model.md](model.md) is generated from the same blocks, so its per-instrument
articulation lists agree with these counts.

The cards' prose was not rewritten. Each card that changed says in its review notes that its json blocks
were harmonized and that the prose keeps the original wording. The old spellings therefore remain in these
places; where prose and json differ, the json is authoritative:

| Card | Old spelling in the prose | Where |
|---|---|---|
| [cards/flutes.md](cards/flutes.md) | `multitongue` | the piccolo, flute, alto_flute and bass_flute sections; open questions 10 and 11; the fact-check log; the consistency review |
| [cards/horns.md](cards/horns.md) | `multitongue` | the horn_f and wagner_tuba_tenor sections; open question 9; the fact-check log |
| [cards/high_brass.md](cards/high_brass.md) | `double_tonguing`, `triple_tonguing` | the trumpet_bb, trumpet_piccolo, cornet and flugelhorn sections; open question 10 |
| [cards/acoustic_pianos.md](cards/acoustic_pianos.md) | `muted_string` | the piano_prepared section; open question 15; the fact-check log |
| [cards/tuned_percussion.md](cards/tuned_percussion.md) | `pitch_bend` | the vibraphone section; open question 12; the fact-check log |
| [cards/clarinets.md](cards/clarinets.md) | `multitongue`, `double_tonguing`, named as the other groups' spellings | open question 5; the consistency review |

`col_legno_battuto` is not in this table: it was not merged, so prose and json agree.

## Canonical vocabulary

**KIND** sorts each id into one of the ten proposed kinds. Ids that straddle two kinds were placed by their
main use; the definition notes the other.

| KIND | Meaning used here |
|---|---|
| long note | how a held sound is produced or kept going, including repetition-based sustains (tremolo, roll) |
| short note | length and separation of detached notes, and short plucked or struck sound productions |
| ornament | extra notes or strokes around the main note |
| transition | pitch movement into, between or out of notes, including legato |
| effect | a special sound production, usually its own sample patch |
| timbre modifier | a colour change that can combine with other articulations (contact point, vibrato, embouchure, bell) |
| mute | a device or a hand damping the sound throughout the note |
| dynamic shape | an attack accent or a loudness contour within the note |
| keyboard control | how the note ends or how keys are struck; on keyboards the damper pedal and hand, with the hand or laissez-vibrer equivalent on harp, guitar and percussion |
| vocal | a voice-only sound production |

Status:

- **existing** = one of the 15 ids in `articulation.rs` [Cited S2];
- **preferred** = on the preferred-spelling list that this research started from. The list has 48 ids:
  `accent`, `air_tone`, `bartok_pizzicato`, `bisbigliando`, `choke`, `col_legno_battuto`,
  `col_legno_tratto`, `crescendo_swell`, `cuivre`, `damped`, `dead_stroke`, `doit`, `drag`, `fall`,
  `falsetto`, `flam`, `flautando`, `fortepiano`, `glissando`, `growl`, `half_valve`,
  `harmonics_artificial`, `harmonics_natural`, `humming`, `key_click`, `let_ring`, `multiphonic`,
  `non_vibrato`, `overblown`, `palm_mute`, `portamento`, `portato`, `pres_de_la_table`, `rim`, `rimshot`,
  `rip`, `sforzando`, `shake`, `slap_tongue`, `staccatissimo`, `stopped`, `sul_tasto`, `tremolo_measured`,
  `trill_half`, `trill_whole`, `vibrato_on`, `vowel`, `xylophonic`. The cards use 47 of them;
- **invented** = coined by a card. The cards' open questions say their invented ids "need the vocabulary
  owner's approval" [Card cards/acoustic_pianos.md OQ15; Card cards/plucked.md OQ11]; the high-brass card
  calls its own (`scoop`, the tonguing ids and the mute ids) "newly invented" [Card cards/high_brass.md OQ10].

The instruments column counts each instrument once per id and lists the instrument ids when there are four
or fewer. Families and counts reflect the card json blocks as they stand: after the five applied merges of
1.3, with `col_legno_battuto` still separate, and without the card-usage proposals of 1.5 (for example, the
clavinet still counts under `damped`).

| KIND | ids |
|---|---|
| long note | 8: `bisbigliando`, `fingered_tremolo`, `flutter_tongue`, `roll`, `sustain`, `thumb_roll`, `tremolo`, `tremolo_measured` |
| short note | 18: `bartok_pizzicato`, `choke`, `dead_stroke`, `detache`, `left_hand_pizzicato`, `loure`, `martele`, `multi_tongue`, `pizzicato`, `portato`, `ricochet`, `sautille`, `shake_stroke`, `slap_pizzicato`, `spiccato`, `staccatissimo`, `staccato`, `tenuto` |
| ornament | 6: `drag`, `flam`, `shake`, `trill`, `trill_half`, `trill_whole` |
| transition | 8: `doit`, `fall`, `glissando`, `half_valve`, `legato`, `portamento`, `rip`, `scoop` |
| effect | 24: `air_tone`, `behind_the_bridge`, `bowed`, `col_legno`, `col_legno_battuto`, `col_legno_tratto`, `golpe`, `growl`, `harmonics`, `harmonics_artificial`, `harmonics_natural`, `jet_whistle`, `key_click`, `multiphonic`, `rasgueado`, `rim`, `rimshot`, `rute`, `scrape`, `slap_tongue`, `strum`, `tongue_ram`, `wah_wah`, `whistle_tone` |
| timbre modifier | 11: `bells_up`, `cuivre`, `flautando`, `non_vibrato`, `pres_de_la_table`, `snares_off`, `su_una_corda`, `subtone`, `sul_ponticello`, `sul_tasto`, `vibrato_on` |
| mute | 8: `con_sordino`, `mute_bucket`, `mute_cup`, `mute_harmon`, `mute_plunger`, `palm_mute`, `stopped`, `xylophonic` |
| dynamic shape | 5: `accent`, `crescendo_swell`, `fortepiano`, `marcato`, `sforzando` |
| keyboard control | 3: `cluster`, `damped`, `let_ring` |
| vocal | 5: `falsetto`, `humming`, `text`, `vowel`, `whisper` |

Totals: 96 ids = 15 existing + 47 from the preferred list + 34 invented by the cards, read from the json
blocks of 85 instruments. If the owner merges `col_legno_battuto` into `col_legno` (1.4), the vocabulary
has 95 ids: 15 existing + 46 preferred + 34 invented.

Preferred-list ids with no row of their own: `overblown` (no card uses it).

| id | status | KIND | definition | families | instruments |
|---|---|---|---|---|---|
| `flutter_tongue` | existing | long note | Wind or brass tone with a rolled or guttural R (frullato). | brass, woodwinds | 30 |
| `roll` | existing | long note | A sustained roll by rapid strokes on a drum, timpani, cymbal or other percussion; on rattles a continuous shake. | percussion | 14 |
| `sustain` | existing | long note | The ordinary held note with no special attack; on plucked and struck instruments the cards use it for the ordinary, freely decaying note. | brass, choir, keyboards, plucked, strings, woodwinds | 63 |
| `tremolo` | existing | long note | Rapid, unmeasured repetition of one note or chord (bow, mallet, plectrum, bellows). | keyboards, percussion, plucked, strings | 21 |
| `bisbigliando` | preferred | long note | Soft, whispering same-pitch shimmer: harp alternates strings on one chord or pitch; woodwinds alternate fingerings of one pitch. | plucked, woodwinds | 2 (english_horn, harp) |
| `tremolo_measured` | preferred | long note | Repetition at a notated, metered subdivision that the players synchronize (strings; timpani measured roll). | percussion, strings | 5 |
| `fingered_tremolo` | invented | long note | Rapid alternation of two notes wider than a second, made by the fingers (wind fingering; string left hand under one bow). | strings, woodwinds | 11 |
| `thumb_roll` | invented | long note | Tambourine roll made by rubbing a moistened or rosined thumb along the head. | percussion | 1 (tambourine) |
| `pizzicato` | existing | short note | Bowed string plucked with a finger; the prepared-piano card uses it for a string plucked inside the piano. | keyboards, strings | 5 |
| `spiccato` | existing | short note | Controlled bounced bow stroke, one stroke per note (strings). | strings | 4 (cello, double_bass, viola, violin) |
| `staccato` | existing | short note | Short, detached notes. | brass, choir, keyboards, percussion, plucked, strings, woodwinds | 64 |
| `tenuto` | existing | short note | Note held for its full value with a slight weight; libraries file it among the shorts. | brass, choir, keyboards, strings, woodwinds | 49 |
| `bartok_pizzicato` | preferred | short note | String pulled so it snaps back against the fingerboard; percussive (guitar: snap). | plucked, strings | 5 |
| `choke` | preferred | short note | A ringing cymbal grabbed immediately after the stroke: an abrupt cut (also a way of ending the note). | percussion | 2 (clash_cymbals, suspended_cymbal) |
| `dead_stroke` | preferred | short note | The instrument is struck while damped, so the note has almost no ring (mallet left pressed on the bar). | percussion | 3 (marimba, vibraphone, xylophone) |
| `portato` | preferred | short note | Slightly separated, gently weighted notes (strings: several in one bow; winds: soft tonguing; clavichord: added key pressure). | brass, keyboards, strings, woodwinds | 28 |
| `staccatissimo` | preferred | short note | Shorter and sharper than staccato. | brass, keyboards, woodwinds | 21 |
| `detache` | invented | short note | Separate bow strokes on the string, one note per stroke, full value. | strings | 4 (cello, double_bass, viola, violin) |
| `left_hand_pizzicato` | invented | short note | String plucked by a finger of the left hand (marked +). | strings | 4 (cello, double_bass, viola, violin) |
| `loure` | invented | short note | Several gently pulsed notes in one bow with a legato impression (strings). | strings | 4 (cello, double_bass, viola, violin) |
| `martele` | invented | short note | Hammered stroke, abruptly stopped with the bow left on the string. | strings | 4 (cello, double_bass, viola, violin) |
| `multi_tongue` | invented | short note | Fast repeated or running notes articulated by double (t-k) or triple (t-k-t) tonguing. | brass, woodwinds | 15 |
| `ricochet` | invented | short note | Thrown bow that bounces several notes in one stroke (jeté). | strings | 3 (cello, viola, violin) |
| `sautille` | invented | short note | Fast, light, uncontrolled bounce in the middle of the bow (p to mp). | strings | 3 (cello, viola, violin) |
| `shake_stroke` | invented | short note | One short shake gesture of a jingle or rattle instrument, as opposed to a sustained roll; unrelated to the brass shake. | percussion | 4 (maracas, shaker, sleigh_bells, tambourine) |
| `slap_pizzicato` | invented | short note | Jazz double-bass slap: the string is pulled so it slaps the fingerboard on the beat. | strings | 1 (double_bass) |
| `trill` | existing | ornament | Rapid alternation with the note above; interval unspecified. | brass, choir, keyboards, percussion, plucked, strings, woodwinds | 54 |
| `drag` | preferred | ornament | A stroke preceded by several grace strokes (two to six before an accent). | percussion | 1 (snare_drum) |
| `flam` | preferred | ornament | A stroke preceded by one grace stroke (open or closed). | percussion | 1 (snare_drum) |
| `shake` | preferred | ornament | Jazz lip shake: wide, fast oscillation between adjacent harmonics (brass). | brass | 3 (cornet, flugelhorn, trumpet_bb) |
| `trill_half` | preferred | ornament | Trill to the semitone above. | brass, strings, woodwinds | 17 |
| `trill_whole` | preferred | ornament | Trill to the whole tone above. | brass, strings, woodwinds | 17 |
| `legato` | existing | transition | Notes joined without a new attack; libraries trigger it by overlapping notes, monophonically. | brass, choir, keyboards, plucked, strings, woodwinds | 58 |
| `doit` | preferred | transition | Upward slide off the end of a note (Logic writes it as a following same-pitch note). | brass, woodwinds | 6 |
| `fall` | preferred | transition | Descending slide off the end of a note (Logic writes it as a following same-pitch note). | brass, woodwinds | 16 |
| `glissando` | preferred | transition | A deliberate slide through a range of pitches, continuous (strings, trombone, timpani pedal) or stepwise (harp, piano, mallets, temple blocks). | brass, keyboards, percussion, plucked, strings, woodwinds | 41 |
| `half_valve` | preferred | transition | Valves part-depressed: a choked, squeezed tone, and valved brass's approximation of a glissando. | brass | 7 |
| `portamento` | preferred | transition | Continuous pitch slide without a new attack: an expressive carry between two notes, or a bend of a sounding note (harp pedal, vibraphone mallet). | brass, choir, percussion, plucked, strings | 17 |
| `rip` | preferred | transition | Fast upward sweep into a loud note. | brass, woodwinds | 13 |
| `scoop` | invented | transition | The note starts below pitch and bends up into it. | brass, woodwinds | 5 |
| `col_legno` | existing | effect | String struck with the wood of the bow (col legno battuto, the meaning in articulation.rs). | strings | 4 (cello, double_bass, viola, violin) |
| `harmonics` | existing | effect | Natural or artificial harmonics; on flute and saxophones the overblown partials over a low fingering. | keyboards, plucked, strings, woodwinds | 12 |
| `air_tone` | preferred | effect | Breath noise with little or no pitch (flute 'air'; accordion air button). | keyboards, woodwinds | 4 (accordion, alto_flute, flute, piccolo) |
| `col_legno_battuto` | preferred | effect | String struck with the wood of the bow: the same stroke as the existing `col_legno`. The merge into `col_legno` is an open owner decision, not applied (1.4). | strings | 4 (cello, double_bass, viola, violin) |
| `col_legno_tratto` | preferred | effect | The wood of the bow drawn across the string. | strings | 4 (cello, double_bass, viola, violin) |
| `growl` | preferred | effect | Humming, singing or a uvular flutter into the instrument while playing. | brass, woodwinds | 9 |
| `harmonics_artificial` | preferred | effect | A stopped note plus a lightly touched node (a fourth above sounds two octaves up). | plucked, strings | 5 |
| `harmonics_natural` | preferred | effect | Partials of an open string sounded by lightly touching a node. | plucked, strings | 5 |
| `key_click` | preferred | effect | Key noise or key slaps of a woodwind, with or without blowing. | woodwinds | 9 |
| `multiphonic` | preferred | effect | Two or more pitches at once, by special fingering or by humming while playing. | brass, woodwinds | 8 |
| `rim` | preferred | effect | Stick on the rim only (rim click, cross-stick). | percussion | 1 (snare_drum) |
| `rimshot` | preferred | effect | Head and rim struck together. | percussion | 1 (snare_drum) |
| `slap_tongue` | preferred | effect | Tongue slapped off the reed or embouchure for a percussive attack (flute: tongue pizzicato). | woodwinds | 7 |
| `behind_the_bridge` | invented | effect | Bowing the short string segment between bridge and tailpiece; indeterminate pitch. | strings | 3 (cello, viola, violin) |
| `bowed` | invented | effect | A bar, plate, disc, cymbal or tam-tam sounded with a bass bow. | percussion | 7 |
| `golpe` | invented | effect | Knocking on the guitar's soundboard, or striking the strings near the bridge. | plucked | 1 (guitar_classical) |
| `jet_whistle` | invented | effect | A strong rushing air burst blown with the flute's embouchure hole fully covered. | woodwinds | 1 (flute) |
| `rasgueado` | invented | effect | Flamenco strum in which the fingers flick out one after another across the strings. | plucked | 1 (guitar_classical) |
| `rute` | invented | effect | Drum or shell struck with a bundle of twigs (Rute). | percussion | 1 (bass_drum) |
| `scrape` | invented | effect | A beater drawn across the surface of a cymbal or tam-tam. | percussion | 2 (suspended_cymbal, tam_tam) |
| `strum` | invented | effect | A chord swept by thumb, fingers or plectrum. | plucked | 2 (guitar_classical, mandolin) |
| `tongue_ram` | invented | effect | The tongue closes the flute's embouchure hole sharply: a percussive pitch about a seventh lower. | woodwinds | 1 (flute) |
| `wah_wah` | invented | effect | A plunger, or a hand over a harmon mute, opened and closed while the note sounds. | brass | 2 (cornet, trumpet_bb) |
| `whistle_tone` | invented | effect | Faint high partials blown with very little air (flute whisper tones). | woodwinds | 1 (flute) |
| `sul_ponticello` | existing | timbre modifier | Bowing (guitar: plucking) near the bridge: thin, glassy tone. | plucked, strings | 5 |
| `cuivre` | preferred | timbre modifier | Forced, brassy, cutting tone (horn: overblown stopped notes). | brass | 6 |
| `flautando` | preferred | timbre modifier | Light, fast bow over the fingerboard: flute-like tone (recorded apart from sul_tasto). | strings | 4 (cello, double_bass, viola, violin) |
| `non_vibrato` | preferred | timbre modifier | Straight tone without vibrato (vibraphone: motor off). | brass, choir, percussion, strings, woodwinds | 27 |
| `pres_de_la_table` | preferred | timbre modifier | Harp plucked near the soundboard: metallic, guitar-like tone. | plucked | 1 (harp) |
| `sul_tasto` | preferred | timbre modifier | Bowing (guitar: plucking) over the fingerboard: soft, veiled tone. | plucked, strings | 5 |
| `vibrato_on` | preferred | timbre modifier | With vibrato: the player's, or a mechanism (vibraphone motor, clavichord Bebung). | brass, choir, keyboards, percussion, plucked, strings, woodwinds | 40 |
| `bells_up` | invented | timbre modifier | Horn played with the bell raised: more direct, brassier sound. | brass | 1 (horn_f) |
| `snares_off` | invented | timbre modifier | Snare drum played with the snares disengaged (sans timbre). | percussion | 1 (snare_drum) |
| `su_una_corda` | invented | timbre modifier | A passage kept on one named string for its colour (sul G, sul C). | strings | 3 (cello, viola, violin) |
| `subtone` | invented | timbre modifier | Soft, breathy saxophone tone. | woodwinds | 4 (saxophone_alto, saxophone_baritone, saxophone_soprano, saxophone_tenor) |
| `con_sordino` | existing | mute | Played with a mute; for brass the straight mute when no type is named; for percussion a covered head (coperto). | brass, percussion, strings, woodwinds | 22 |
| `palm_mute` | preferred | mute | The string is sounded while a hand rests on it: dry, short, still pitched (guitar palm mute; prepared piano: finger on the string). | keyboards, plucked | 2 (guitar_classical, piano_prepared) |
| `stopped` | preferred | mute | Horn hand-stopped (+): muted, nasal, pitch raised a semitone. | brass | 1 (horn_f) |
| `xylophonic` | preferred | mute | Harp: one hand damps at the soundboard while the other plucks: dry, xylophone-like. | plucked | 1 (harp) |
| `mute_bucket` | invented | mute | Bucket mute clipped to the bell: covered, horn-like sound. | brass | 2 (trumpet_bb, trumpet_c) |
| `mute_cup` | invented | mute | Cup mute: darker and more subdued than straight. | brass | 3 (cornet, trumpet_bb, trumpet_c) |
| `mute_harmon` | invented | mute | Harmon mute, stem in (penetrating) or out (subdued). | brass | 3 (cornet, trumpet_bb, trumpet_c) |
| `mute_plunger` | invented | mute | Plunger held in front of the bell (static; see wah_wah for the moving plunger). | brass | 2 (cornet, trumpet_bb) |
| `marcato` | existing | dynamic shape | Strongly accented, heavy attack. | brass, choir, keyboards, percussion, strings, woodwinds | 73 |
| `accent` | preferred | dynamic shape | An accented attack, lighter than marcato or sforzando. | brass, keyboards | 7 |
| `crescendo_swell` | preferred | dynamic shape | A swell on a held note: crescendo, or crescendo and diminuendo. | brass, choir, keyboards, percussion, strings, woodwinds | 40 |
| `fortepiano` | preferred | dynamic shape | Loud attack, immediately soft (fp). | brass, percussion, strings, woodwinds | 22 |
| `sforzando` | preferred | dynamic shape | A sudden strong accent (sfz). | brass, choir, keyboards, woodwinds | 36 |
| `damped` | preferred | keyboard control | The ring is stopped at the note's written end by damper, pedal or hand (étouffé, secco); the opposite of let_ring. | keyboards, percussion, plucked | 14 |
| `let_ring` | preferred | keyboard control | The note rings on past its written end: sustain pedal held (harp, guitar, percussion: laissez vibrer). | keyboards, percussion, plucked | 21 |
| `cluster` | invented | keyboard control | Adjacent keys struck together with palm, fist or forearm. | keyboards | 4 (piano_concert_grand, piano_prepared, piano_studio_grand, piano_upright) |
| `falsetto` | preferred | vocal | Male voice in falsetto or head voice. | choir | 5 |
| `humming` | preferred | vocal | Sung with the mouth closed (mm). | choir | 10 |
| `vowel` | preferred | vocal | A sustained sung vowel (ah, oh, oo, ee, mm). | choir | 10 |
| `text` | invented | vocal | Sung words or syllables. | choir | 10 |
| `whisper` | invented | vocal | Unvoiced, whispered voice. | choir | 10 |

## Modelling notes

_Options, not decisions._ Everything in this section is input for the orchestra design: options with their evidence and costs. Nothing
here is decided, and nothing here changes the cards.

### 3.1 Constraints that shape any choice

- **No per-note articulation exists today.** A MIDI note in project format v1 stores id, pitch, start,
  length, velocity and channel. Notes do not keep unknown keys, so adding an articulation per note, flat or
  factored, is a `.sonara` schema change under CTR-01 [Cited S4].
- **No agent tool writes MIDI CC into a clip** [Cited S5]. The note tools take notes only; CC reaches a
  clip only through `import_midi` or live input [Cited S5; Cited S7: row N1]. Any option that renders vibrato, swells or the damper
  pedal as CC needs a tool-schema change, also under CTR-01.
- **The sampler flattens everything.** It plays key switches (`sw_*`), velocity layers, round robin,
  `trigger=release` regions, `group`/`off_by`, CC7 and CC11 (both as linear gain), CC64 and pitch bend of
  ±2 semitones. It does not play CC-driven layers or crossfades, CC1 or CC21, velocity crossfades
  (`xfin`/`xfout`), or `trigger=first` and `trigger=legato`, which silently become ordinary attacks. It
  ignores the MIDI channel at note-on, and a clip cannot hold a program change [Cited S7: rows C9, C15,
  C16, C19, Q5, Q6]. Whatever the musical model, the output to a library ends as key switches, separate
  patches, velocity and CC7/CC11 volume [Cited S8 §3.1, §3.3].
- **Key switches have caveats in today's sampler** [Cited S7]. Some files' header layout loses the switch
  range or the default switch, so a key-switch patch can be silent until a switch key is played, or play
  several articulations at once (rows Q1, Q11). After a seek or a loop wrap during playback, the switch
  pressed last before the jump stays active (C11). A switch note also triggers any region mapped to its key
  (C17). Release samples do not start for a key released while the sustain pedal is down (N1), which
  matters for `let_ring` and `damped` on pianos.
- **A path without a project-format change exists, at track granularity.** An automatable enum parameter
  on the sampler (for example `articulation`, `mute`) would reach it once per audio block through
  `create_automation` [Cited S5]: the engine samples device parameters once per block and automates an
  enum by choice index [Cited S6]. No enum parameter is automatable today: the descriptor helper `ep`
  builds every enum with `automatable: false`, the sampler's `instrument` parameter included [Cited S6;
  Cited S7: row C2]. So this path needs a new, automatable enum on the sampler descriptor. The sampler is
  registered beside the 12 golden-pinned built-in descriptors, not among them, so its parameters can change
  without touching golden data [Cited S6] (GLD-01). The cost: one state per track, so divisi on one track
  cannot differ, and a switch lands at the start of the block that contains its beat.
- **Availability today is per id.** `articulation_availability(id, library_declared)` answers Supported,
  IdiomaticUnverified, NotIdiomatic or UnknownArticulation for one id; any id outside the 15 is
  UnknownArticulation [Cited S2].

### 3.2 What Logic's flat list does

From the Apple card [Card cards/apple.md#3]:

- Each note carries one articulation ID from 1 to 255, or none. Since Logic 10.4.2, "no ID" is no longer
  treated as ID 1.
- One Articulation Set per track names the IDs, gives each a score symbol, and defines input switches.
  Switch types include note, CC, program and velocity; the modes are Permanent, Momentary and Toggle, each
  also with Retrigger or Trigger.
- On output, an ID sends up to three MIDI messages (note, controller, program change, aftertouch or pitch
  bend, each on a chosen channel), and an articulation can also be selected by MIDI channel. Outputs are
  independent of the input switches, so one input layout can drive different libraries. Of these outputs,
  Sonara's sampler acts on key-switch notes and CC7/CC11 volume only (3.1).
- An ID with no articulation in the set plays silence.
- A fall or doit is a second note of the same pitch with the Fall or Doit articulation, placed directly
  after the main note; live, a Trigger-mode switch ends the held note with the effect [Card
  cards/apple.md#2.2; Card cards/apple.md#2.3].
- Libraries ship combined patches, which a flat list names one by one: Spitfire's "Long Flutter Stopped",
  "Short Marcato Stopped", "Rip Stopped", "Bells up Staccatissimo"; VSL's "Portato long marcato" and
  "Sforzato with and without vibrato"; Spitfire's trombone "Muted" long, short, fall, rip and swell
  [Card cards/horns.md#horn_f; Card cards/high_brass.md#cornet; Card cards/double_reeds.md#oboe; Card
  cards/low_brass.md#trombone_tenor].

### 3.3 Candidate axes

The KIND column already sorts the vocabulary into candidate slots: `mute`, `timbre modifier`, `dynamic
shape` and the ending half of `keyboard control` are the dimensions below; long note, short note, ornament,
transition, effect and vocal would remain the note's primary articulation [Estimate: design reading of
"Canonical vocabulary"].

| Option axis | Ids it would absorb | For | Against or cost |
|---|---|---|---|
| **Mute**, a state over a passage | `con_sordino`, `mute_cup`, `mute_harmon`, `mute_plunger`, `mute_bucket`, `stopped`; the percussion cover (`con_sordino` on timpani and drums); possibly `snares_off` | Samplemodeling selects the mute on CC100, combinable with every articulation, and applies a change only on the next detached note; players need rests to fit or remove a mute, and *senza sordino* cancels it [Card cards/high_brass.md#trumpet_bb]. Spitfire records muted and stopped variants of long, short, rip, fall, swell and flutter (3.2). The high-brass card proposes a separate mute field (none, straight, cup, harmon, plunger, bucket and more) as an owner decision [Card cards/high_brass.md OQ9]; the low-brass card treats the mute types as a parameter of `con_sordino`, not as new ids [Card cards/low_brass.md OQ12] | `con_sordino` becomes a value of the axis instead of an id: a stable-id change for the vocabulary and for manifests that already declare it [Cited S2; Cited S3]. Libraries record only some mute and articulation pairs, so the output map needs pair entries and a fallback when a pair is missing |
| **Vibrato** | `vibrato_on`, `non_vibrato` | Vibrato is a continuous controller in most libraries: Spitfire and BBCSO CC21, VSL CC20, Samplemodeling CC1 for depth and CC19 for rate, Sonatina Symphonic Orchestra CC21 (on its vibraphone CC21 drives an LFO labelled vibrato) [Card cards/strings.md#violin; Card cards/clarinets.md#clarinet_bb; Card cards/flutes.md#piccolo; Card cards/high_brass.md#trumpet_bb; Card cards/tuned_percussion.md#vibraphone]. A tri-state (default, on, off) replaces two ids; VSL records sforzato and portato with and without vibrato (3.2) | Sonara's sampler has no CC crossfade and ignores CC21 [Cited S7: row C15], so in Sonara vibrato means choosing a patch or key switch (VSCO `OboeSusVib` against `OboeSusNV`; VSCO flute switches C2 non-vibrato, C#2 vibrato) [Card cards/double_reeds.md#oboe; Card cards/flutes.md#flute]. EW Hollywood Choirs ties vibrato to the CC1 dynamics, so it cannot be set apart there [Card cards/voices.md#choir_soprano] |
| **Contact point** (bow or pluck position) | `sul_ponticello`, `sul_tasto`, `flautando`, `pres_de_la_table`, `su_una_corda` (string choice), `behind_the_bridge` (the extreme) | Position combines with other strokes: the cello card cites a sul ponticello tremolo in Wagner's *Meistersinger* [Card cards/strings.md#cello]; on the guitar it is a hand position [Card cards/plucked.md#guitar_classical]. The harp term is the same idea as `sul_ponticello` (1.4) | Libraries ship these as separate long patches (Spitfire "Sul Pont", "Super Sul Tasto", "Flautando", "Sul C") and few combinations [Card cards/strings.md#viola; Card cards/strings.md#cello]; Logic's Studio Strings has none of them [Card cards/apple.md#2.2] |
| **Trill interval** | `trill_half`, `trill_whole`, and Spitfire's minor- and major-third trills | The interval is a number of semitones; it could also be derived from the key signature and the written accidental [Estimate] | The libraries in the cards give the intervals separate patches or key switches: Logic key switches 32 "Trill Semi" and 33 "Trill Whole", BBCSO minor and major 2nd patches, Orchestral Tools HT and WT [Card cards/strings.md#violin; Card cards/apple.md#2.2; Card cards/clarinets.md#bass_clarinet], so the output side stays flat. The brass shake is a different idiom (1.4) |
| **Dynamic shape** | `accent`, `marcato`, `sforzando`, `fortepiano`, `crescendo_swell` | Shapes combine with other articulations: the timpani card maps its crescendo roll to `crescendo_swell` and its sfp in a roll to `fortepiano`, and the suspended-cymbal card maps its crescendo roll to `crescendo_swell`, so in a flat list the roll itself is lost [Card cards/tuned_percussion.md#timpani; Card cards/untuned_percussion.md#suspended_cymbal]; Orchestral Tools records "Trills Sforzando" [Card cards/clarinets.md#clarinet_bb]. Shapes have a duration: Logic "Crescendo Slow/Fast" and "Fortepiano Slow/Fast", VSL crescendos of 2, 3 and 4 seconds, NI "FortePianoCrescendo" over 2 and 4 beats [Card cards/apple.md#2.2; Card cards/double_reeds.md#oboe; Card cards/saxophones.md#saxophone_alto]. A swell recorded at one length fits one tempo only; a CC-driven swell fits any [Card cards/untuned_percussion.md#suspended_cymbal] | A sampled fp or sfz attack cannot be rebuilt from CC11 alone. CC11 swells need a CC-writing tool (3.1). Accents are also score symbols that Logic attaches to a note |
| **Repetition metering** | `tremolo` and `tremolo_measured`; `roll` (drums, timpani) and `tremolo` (mallets) | Measured or unmeasured is a property of the same gesture. The timpani card already writes the measured roll as `tremolo_measured` and the unmeasured one as `roll` [Card cards/tuned_percussion.md#timpani]. Libraries call the mallet tremolo a roll [Card cards/tuned_percussion.md OQ12] | `roll` and `tremolo` are both existing ids; merging them is a stable-id change. A measured tremolo is notated rhythm that an agent can also write as notes |
| **Release** (how the note ends) | `let_ring`, `damped`, `choke`; `dead_stroke` is its attack-side twin | This is already MIDI behaviour: CC64 on pianos and the vibraphone; on Spitfire's harp, pedal down is laissez vibrer and a key released with the pedal up is *sons étouffés*; the triangle choke is an exclusive group (`group`/`off_by`), which the sampler plays [Card cards/plucked.md#harp; Card cards/untuned_percussion.md#triangle] | Percussion samples have fixed tails, so "damped" needs a release cut or its own sample. CC64 cannot be written by an agent today, and the sampler does not start release samples for a key released under the pedal (3.1) |
| **Beater or implement** | `rute`, `bowed`; mallet hardness, brushes, hand | The untuned-percussion card proposes a `beater` field (stick, soft_mallet, hard_mallet, brush, rute, hand, bow) and says `rute` and `bowed` are ids only because no such field exists [Card cards/untuned_percussion.md OQ17]; the tuned-percussion cards treat the mallet type as a control, and VSL ships one patch per beater [Card cards/tuned_percussion.md#xylophone; Card cards/tuned_percussion.md#glockenspiel] | One more field, per note or per track |
| **Vowel and text** | `vowel`, `humming`, `text` | `vowel` needs a parameter (ah, oh, oo, ee, mm) to be useful to an agent [Card cards/voices.md OQ14]; libraries offer vowels as a choice and treat "Mm" as one of them [Card cards/voices.md#voice_soprano]. `text` carries a payload (syllables, words), which is closer to a lyric on a note than to an articulation [Estimate] | A per-note lyric or vowel is a project-format change (3.1) |
| **Tonguing pattern** | `multi_tongue` (double, triple, quadruple) | Spitfire's one "Multitongue" patch covers double, triple and quadruple tonguing, selected by a "Variation" slider; the low-brass card records it as a patch, not an id, so no low-brass json lists `multi_tongue`, while the trumpet, horn, flute and clarinet cards do [Card cards/low_brass.md#trombone_tenor; Card cards/low_brass.md OQ12] | Orchestral Tools records "Double Tongue" and "Triple Tongue" as separate patches [Card cards/clarinets.md#bass_clarinet], so the output map needs the pattern as a library variant; one id is still enough [Estimate] |
| **Pitch movement at note boundaries** | `scoop` and `rip` (start), `fall` and `doit` (end) | Logic writes falls and doits as a following same-pitch note (3.2); lengths are parameters: "Fall Long/Medium/Short", "Scoop Slow/Fast" [Card cards/apple.md#2.2; Card cards/apple.md#2.3] | Either the boundary note carries the id (Logic) or the main note carries an end attribute; both need a note-level field |
| **Instrument controls, not ids** | none of the 96; listed so they do not become ids | harp pedal state, organ registration, Hammond drawbars and Leslie speed, vibraphone motor speed, clavichord key pressure, accordion bellows on CC11 [Card cards/plucked.md#harp; Card cards/historical_keys_organs.md#organ_pipe; Card cards/historical_keys_organs.md#organ_hammond; Card cards/tuned_percussion.md OQ12; Card cards/historical_keys_organs.md#accordion] | They are track or device controls |
| **Default (ordinario)** | the cards' usage of `sustain` and `staccato` for the ordinary note (1.5) | A note without an articulation could mean "the instrument's ordinary sound", as Logic separates "no ID" from ID 1 (3.2); then `sustain` keeps its meaning as the long patch [Card cards/plucked.md OQ12] | Manifests that declare `sustain` for a default patch would need a rule for "no articulation" |

### 3.4 Flat list against separate axes

| Aspect | Flat list, one id per note (Logic) | Separate axes |
|---|---|---|
| Per-note data | one value | several fields, or one composite value |
| Vocabulary size | grows with every combination a library records, or leaves combinations out | small per axis; combinations come free |
| Library mapping | matches how libraries ship combined patches; a manifest id is a string match [Cited S2; Cited S3] | needs a resolver from a tuple of values to a library's sparse set of combined patches; the output is still flat (3.1) |
| Fallback | needs an explicit table [Cited S8 §4.2] | dropping an axis value is a natural fallback (muted staccato becomes staccato, and is reported) |
| Passage states | a mute or vibrato change is repeated on every note, and the rest a mute change needs is invisible | a state spans a passage and has a change time that can be checked against rests |
| Agent use | one string per note; `from_id` validates it | several fields; validation must also check which combinations are idiomatic for the instrument |
| GUI | one articulation lane (Logic: pop-up, colour by articulation) | several lanes or a composite editor; the piano roll has no articulation lane today [Cited S9] |
| Availability | `articulation_availability` works unchanged | availability per axis value and per combination |
| Contracts | a per-note id is a `.sonara` schema change (CTR-01) | the same, with more fields; or track-level enum parameters automated per block, without a schema change (3.1) |

Shapes the orchestra design could choose between, with no recommendation here:

- **A. Flat list, extended.** Add the 81 proposed ids to the enum (80 if `col_legno_battuto` is merged),
  keep combinations out, and add a refinement fallback table (3.5) in front of the substitution of
  [sound-mapping.md](sound-mapping.md) §4.2 [Cited S8]. Smallest change; the list keeps growing.
- **B. Primary id plus a few states.** One primary id per note (long note, short note, ornament,
  transition, effect, vocal), with mute, vibrato and contact point (and possibly dynamic shape) as states
  that change at a time point, per note or per track. The output map resolves the pair, or falls back by
  dropping a state.
- **C. Full axes per note.** Every dimension of 3.3 as its own note field.
- **Realization in any shape:** device-level enum parameters on the sampler, automated through
  `create_automation`, give per-track states without a schema change (3.1); per-note data still needs the
  CTR-01 decision.

### 3.5 Refinement fallbacks (option)

If the vocabulary stays flat (shape A), a refinement can fall back to its general id when a library lacks
it; the substitution of [sound-mapping.md](sound-mapping.md) §4.2 [Cited S8] then continues from the
general id. This is an option for the availability logic, not a rename: it must not be applied with the
mapping of 1.3.

| Refinement | Falls back to |
|---|---|
| `trill_half`, `trill_whole`, `shake` | `trill` |
| `harmonics_natural`, `harmonics_artificial` | `harmonics` |
| `tremolo_measured` | `tremolo` |
| `thumb_roll` | `roll` |
| `bartok_pizzicato`, `left_hand_pizzicato` | `pizzicato` |
| `slap_pizzicato` | `bartok_pizzicato`, then `pizzicato` |
| `mute_cup`, `mute_harmon`, `mute_plunger`, `mute_bucket`, `stopped` | `con_sordino` |
| `xylophonic` | `palm_mute` |
| `staccatissimo` | `staccato` |
| `sautille`, `ricochet` | `spiccato` |
| `loure` | `portato` |
| `flautando` | `sul_tasto` |
| `rasgueado` | `strum` |
| `choke` | `damped` |
| `accent` | `marcato` |
| `col_legno_battuto`, while it stays a separate id (1.4) | `col_legno` |

### 3.6 Questions for the owner

1. Confirm the five merges already applied to the card json blocks (1.2), in particular the spelling
   `multi_tongue`, and decide the sixth, `col_legno_battuto` into `col_legno`, which is not applied (1.4).
2. Approve or reject the 34 invented ids ("Canonical vocabulary", status "invented").
3. Decide the two candidate merges kept apart here: `loure` into `portato`, and `choke` into `damped`.
4. Let the card owners apply the usage fixes of 1.5 (`damped`, `tremolo`, `staccato`), and decide the
   default question (`sustain` and `staccato` used for the ordinary note).
5. Decide whether mallet rolls stay `tremolo` or may use `roll` (1.4; the doc comments of `articulation.rs`).
6. Choose a shape from 3.4 (A, B or C) before any per-note field is proposed under CTR-01.

## Review notes

Review of 2026-10-30, against the reviewed cards, [model.md](model.md), [sfz-conformance.md](sfz-conformance.md)
and the repository code. What changed:

1. **Counts regenerated** with a script from the json blocks of all 85 instruments in [cards/](cards/), after
   the card json was harmonized: 96 distinct ids (15 existing + 47 preferred + 34 invented), no duplicate
   within an instrument, no existing id in a proposed list and no proposed id in an existing list. Every row
   of "Canonical vocabulary" (count, families, instrument list) and every KIND total agrees with the cards.
   Every instrument id used in this file exists in [model.md](model.md).
2. **Merge status.** Five merges are marked as applied (1.2); `col_legno_battuto` into `col_legno` is marked
   as not applied, an open owner decision (Summary, 1.2, 1.4, 3.6). The json block of 1.3 holds only the
   five applied merges. `col_legno_battuto` got its own row in "Canonical vocabulary" and a fallback row in
   3.5. Totals changed from 95 to 96 (95 if the merge is accepted), and shape A in 3.4 from 80 to 81 new ids.
3. **Section 1.6 rewritten.** The comparison with a separate extract file was removed, because that file
   is not part of this record and the counts come from the cards. In its place, a table lists where the
   card prose still uses the old spellings.
4. **Sources made repository-ready.** Local paths were replaced by repository paths and relative links;
   the facts formerly taken from a research index were re-checked in the code and are now cited to the
   files that hold them (S2-S7, S9). Source references use `[Cited Sn]`; card citations name
   `cards/<file>.md`. The preferred-spelling list is now written out in full, instead of being referred to.
5. **Code facts corrected or completed** against [sfz-conformance.md](sfz-conformance.md) and the code (3.1,
   3.2): CC7 and CC11 act as gain only and CC1/CC21 do nothing; `trigger=first`/`legato` become attacks; the
   sampler ignores the channel at note-on and clips cannot hold program changes; the key-switch caveats
   (Q1, Q11, C11, C17) and the lost release samples under the pedal (N1); no enum parameter is automatable
   today, so the track-level path needs a new automatable enum on the sampler descriptor.
6. **Card citations fixed.** The pianos card's `palm_mute` recommendation is in its fact-check log, part 7,
   not an "item 7"; the "owner's approval" quote is in the pianos and plucked open questions only, so the
   high-brass open question 10 is cited for "newly invented" instead; Logic's falls are in the Apple card
   §2.2 and its doits and "Fall Medium" in §2.3; the Logic trill key switches also cite the Apple card.
7. **Contradictions removed.** "Sonatina CC21" and "SSO's vibraphone CC21" named the same library twice;
   the mute-field row now separates the high-brass proposal (a mute field) from the low-brass view (mute
   types as a parameter of `con_sordino`); the tonguing row notes that the low-brass cards list no
   `multi_tongue` while the other wind and brass cards do.
8. **Wording.** Internal work labels were replaced ("the orchestra design"), and the statement that no card
   was edited now says what the harmonization changed and what it left alone (header, 1.5).
