// The time signatures for display (sonara-core's `TimeSignatureMap`): the bar at a beat, the
// beat at a bar, a bar's length and the "Bar.Beat.Tick" position of the transport bar and
// the ruler. The project view gives each signature's start beat and bar length. Checked
// against tests/fixtures/time-v1.json, which sonara-core writes (tests/unit/time.test.ts).

import type { TimeSignatureView } from "../types";

/** The beat tolerance of the prototype (`EPSILON`). */
export const EPSILON = 1e-9;

/** Ticks per quarter-note beat in a displayed position. */
export const TICKS_PER_BEAT = 960;

const FALLBACK: TimeSignatureView = { bar: 0, beat: 0, numerator: 4, denominator: 4, beats_per_bar: 4 };

/** A position for display: 1-based bar and beat (in denominator units), the rest in beats. */
export interface BarPosition {
  bar: number;
  beat: number;
  fraction: number;
}

function points(signatures: readonly TimeSignatureView[] | null): readonly TimeSignatureView[] {
  return signatures && signatures.length > 0 ? signatures : [FALLBACK];
}

/** The signature in force at `beat` (`index_for_beat`). */
export function indexForBeat(signatures: readonly TimeSignatureView[] | null, beat: number): number {
  const sigs = points(signatures);
  let i = 0;
  while (i + 1 < sigs.length && sigs[i + 1].beat <= beat + EPSILON) i += 1;
  return i;
}

/** The signature in force at the 0-based, possibly fractional, `bar` (`index_for_bar`). */
export function indexForBar(signatures: readonly TimeSignatureView[] | null, bar: number): number {
  const sigs = points(signatures);
  let i = 0;
  while (i + 1 < sigs.length && sigs[i + 1].bar <= bar) i += 1;
  return i;
}

/** `at_bar`. */
export function signatureAtBar(
  signatures: readonly TimeSignatureView[] | null,
  bar: number,
): TimeSignatureView {
  return points(signatures)[indexForBar(signatures, bar)];
}

/** `at_beat`. */
export function signatureAtBeat(
  signatures: readonly TimeSignatureView[] | null,
  beat: number,
): TimeSignatureView {
  return points(signatures)[indexForBeat(signatures, beat)];
}

/** `beat_to_bar`: the fractional 0-based bar at `beat`. */
export function beatToBar(signatures: readonly TimeSignatureView[] | null, beat: number): number {
  const sig = signatureAtBeat(signatures, beat);
  return sig.bar + (beat - sig.beat) / sig.beats_per_bar;
}

/** `bar_to_beat`: the beat at the start of the 0-based, possibly fractional, `bar`. */
export function barToBeat(signatures: readonly TimeSignatureView[] | null, bar: number): number {
  const sig = signatureAtBar(signatures, bar);
  return sig.beat + (bar - sig.bar) * sig.beats_per_bar;
}

/** `bar_length`: the length in beats of the 0-based `bar`. */
export function barLength(signatures: readonly TimeSignatureView[] | null, bar: number): number {
  return signatureAtBar(signatures, bar).beats_per_bar;
}

/** `position`: the musical position of `beat`. */
export function position(signatures: readonly TimeSignatureView[] | null, beat: number): BarPosition {
  const sig = signatureAtBeat(signatures, beat);
  const rel = beat - sig.beat;
  const barOffset = Math.floor(rel / sig.beats_per_bar + EPSILON);
  const inBar = rel - barOffset * sig.beats_per_bar;
  const unit = 4 / sig.denominator;
  const beatIndex = Math.floor(inBar / unit + EPSILON);
  const fraction = Math.max(0, inBar - beatIndex * unit);
  return { bar: sig.bar + barOffset + 1, beat: beatIndex + 1, fraction };
}

/** Python's `round` (ties to even). */
export function pyRound(value: number): number {
  const floor = Math.floor(value);
  const diff = value - floor;
  if (diff > 0.5) return floor + 1;
  if (diff < 0.5) return floor;
  return floor % 2 === 0 ? floor : floor + 1;
}

/** "Bar.Beat.Tick" with 960 ticks per beat (`BarPosition.__str__`): "1.1.000". */
export function formatPosition(signatures: readonly TimeSignatureView[] | null, beat: number): string {
  const pos = position(signatures, beat);
  const ticks = pyRound(pos.fraction * TICKS_PER_BEAT);
  return `${pos.bar}.${pos.beat}.${String(ticks).padStart(3, "0")}`;
}

/** "n/d" of a signature (`TimeSignaturePoint.__str__`). */
export function signatureText(sig: TimeSignatureView): string {
  return `${sig.numerator}/${sig.denominator}`;
}
