// The editing grid shared by the timeline and the piano roll (the prototype's `SnapModel` in
// sonara/ui/panels.py): on by default at 1/16; "Bar" follows the time signature at the beat; ⌥
// bypasses snapping during a gesture. The time signatures come from the project view, which
// gives each one's start beat and bar length.

import { barToBeat, beatToBar, EPSILON, pyRound, signatureAtBeat } from "../time/signatures";
import type { TimeSignatureView } from "../types";

export { barToBeat, beatToBar };

/** The grid choices; 0 means one bar, resolved against the time signature. */
export const GRID_CHOICES = [
  ["Bar", 0],
  ["1/2", 2],
  ["1/4", 1],
  ["1/8", 0.5],
  ["1/8T", 1 / 3],
  ["1/16", 0.25],
  ["1/16T", 1 / 6],
  ["1/32", 0.125],
] as const;

export type GridName = (typeof GRID_CHOICES)[number][0];

export interface Snap {
  readonly enabled: boolean;
  readonly gridName: GridName;
}

export const DEFAULT_SNAP: Snap = { enabled: true, gridName: "1/16" };

export type SnapMode = "nearest" | "floor" | "ceil";

/** The grid in beats of `name`; 0 for "Bar". */
export function gridValue(name: GridName): number {
  return GRID_CHOICES.find(([choice]) => choice === name)?.[1] ?? 0.25;
}

/** `grid_beats`: the grid at `atBeat`, a bar's length for "Bar" (4 without a project). */
export function gridBeats(
  snap: Snap,
  signatures: readonly TimeSignatureView[] | null,
  atBeat = 0,
): number {
  const grid = gridValue(snap.gridName);
  if (grid > 0) return grid;
  if (!signatures || signatures.length === 0) return 4;
  return signatureAtBeat(signatures, atBeat).beats_per_bar;
}

/** `snap` of sonara.domain.time: multiples of `grid`; a grid of 0 or less does nothing. */
export function snapToGrid(beat: number, grid: number, mode: SnapMode = "nearest"): number {
  if (grid <= 0) return beat;
  const q = beat / grid;
  const n =
    mode === "floor"
      ? Math.floor(q + EPSILON)
      : mode === "ceil"
        ? Math.ceil(q - EPSILON)
        : Math.floor(q + 0.5);
  return n * grid;
}

/** `SnapModel.snap`: `beat` on the grid, never before 0; off or bypassed, only clamped. */
export function snapBeat(
  snap: Snap,
  beat: number,
  signatures: readonly TimeSignatureView[] | null,
  mode: SnapMode = "nearest",
  bypass = false,
): number {
  if (!snap.enabled || bypass) return Math.max(0, beat);
  if (gridValue(snap.gridName) === 0 && signatures && signatures.length > 0) {
    const bar = beatToBar(signatures, beat);
    const n = mode === "floor" ? Math.floor(bar) : mode === "ceil" ? Math.ceil(bar) : pyRound(bar);
    return Math.max(0, barToBeat(signatures, n));
  }
  return Math.max(0, snapToGrid(beat, gridBeats(snap, signatures, beat), mode));
}
