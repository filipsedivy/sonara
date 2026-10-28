# Orchestration metadata: research record (SON-P06)

Verified on 2026-10-28. The conventions and the dataset are in
[orchestration-metadata.md](orchestration-metadata.md).

## Repository state

- Pitches are `i64` MIDI notes with middle C = 60 (`sonara_midi::theory::bass_note`). No P05
  primitives existed, so no interval or spelling model was added.
- The instrument catalog (SON-P02, not merged when this was written) identifies presets as
  `<library>/<patch path>` and has the same family slugs; its articulations come only from a
  library manifest. The metadata here uses its own instrument-type ids and the same slugs.

## Sources read (English Wikipedia, facts only)

| Instrument | What the page states |
|---|---|
| Violin | lowest G3; E7 the limit for orchestral parts; up to C8 by stopping |
| Cello | lowest C2 (open string); no upper limit in the text |
| Double bass | orchestral parts rarely exceed E1–G3 (sounding); C extension to C1; no upper limit |
| Flute | three octaves from middle C, a semitone lower with a B foot |
| Piccolo | tessitura D5–C8, an octave above written; some reach C5 |
| Oboe | Bb3 to about G6 |
| English horn | in F, a fifth below the oboe; sounding E3–C6 |
| Clarinet | Bb clarinet written E3–C7, sounding D3–Bb6 |
| Bassoon | infobox Bb1–E5, Ab5 in parentheses |
| Contrabassoon | sounding Bb0 (A0) to D4, notated an octave higher |
| Trumpet | written F#3–C6, sounds a whole step lower |
| Trombone | lowest E2 without pedal notes; top F5, conservatively D5 |
| Horn in F | sounds a fifth lower; the range appears only in an image |
| Glockenspiel, xylophone | +24 and +12; ranges only in octaves or images |

## Not from a source that was read (marked `Estimate`)

Cello and double bass upper limits; horn in F, glockenspiel and xylophone ranges; every
articulation list (general orchestration practice).

## Alternatives rejected

- Taking ranges from a sample library or its manifest: the contract wants instrument facts kept
  apart from library facts.
- JSON data with a loader: needs a parser and validation at run time; a Rust `static` is checked
  at compile time.

## Licence and risks

- Wikipedia text is CC BY-SA; only numbers are used. No new dependency except `thiserror`, which
  the workspace already uses.
- Wikipedia is a tertiary source. Check the values against orchestration handbooks before
  relying on them, and replace the estimates.
