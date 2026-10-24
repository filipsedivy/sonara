# ADR 0003: Canonical time base

- **Status:** Accepted (2026-10-20)

## Context

Arrangement and MIDI data are musical: bars, beats, grids and tempo changes. Audio data is
absolute: sample frames at a source sample rate. Rendering needs sample positions at the
project rate. The agent API should accept musically meaningful coordinates.

## Options considered

1. **Integer ticks at 960 PPQ.** This is exact for common grids and maps to Standard MIDI
   Files. However, one tick at 124 BPM is about 24 samples at 48 kHz, so audio clips cannot
   be placed sample-accurately.
2. **Integer ticks at a very high PPQ** (for example 960 000). This is exact and
   sample-accurate, but the numbers are unreadable for humans and agents, and it is
   arbitrary.
3. **float64 quarter-note beats.** This is readable (`16.0` is bar 5 in 4/4) and
   sub-sample precise: float64 resolution at 10⁴ beats is about 10⁻¹² beats. Triplet grid
   values are not exactly representable.
4. **Dual time base per object** (musical or absolute). This is the most flexible, but every
   operation has to handle both cases.

## Decision

Option 3, with the following rules:

- Timeline positions and lengths (clips, notes, automation points, markers, sections, loop
  and punch ranges) are **float64 beats**, where one beat is one quarter note.
- The tempo map converts beats to seconds and seconds to beats exactly, with closed-form
  integration over tempo segments. Seconds convert to samples at the project rate. Sample
  positions are computed once per block and never accumulated incrementally, so they do not
  drift.
- Audio source offsets are **integer frames at the source sample rate**, which is exact.
- Fades and crossfades are in **seconds**, because they are perceived in absolute time.
- Grid snapping rounds to `k * grid`. Comparisons that decide overlap or containment use a
  tolerance of 1e-9 beats (`domain.time.EPSILON`).
- Standard MIDI File export quantizes to 960 PPQ and reports the maximum rounding error.
- The agent API accepts beats, and additionally `bar:beat` strings and seconds where the
  schema says so. Results always include beats and seconds.

## Consequences

Audio clips do not stretch when the tempo changes: they keep their start beat and play at
their native rate (like an unwarped audio clip). Time-stretching is an explicit clip
property, not an implicit result of tempo edits.
