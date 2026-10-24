// The timeline's grid and the ruler's marks (`grid_steps` and `paint_grid` in sonara/ui/timeline/
// scale.py and painting.py, `Ruler._paint_bars`): bar lines thin out to keep at least 36 px per
// step, beat lines show from 14 px per beat and sub-beat lines from 10 px per step, across every
// time signature.

import { pyRound } from "../time/signatures";
import { barToBeat, beatToBar, signatureAtBar, signatureText } from "../time/signatures";
import { beatToX, type TimelineView, xToBeat } from "../state/timeline";
import type { TimeSignatureView } from "../types";

export interface GridSteps {
  /** The bar line step in bars. */
  readonly barStep: number;
  /** The beat line step, or null when beats are too close. */
  readonly beatStep: number | null;
  /** The sub-beat step in beats, or null. */
  readonly sub: number | null;
}

/** `grid_steps`: steps that keep the lines a few pixels apart at `pixelsPerBeat`. */
export function gridSteps(pixelsPerBeat: number, beatsPerBar: number): GridSteps {
  const barPx = beatsPerBar * pixelsPerBeat;
  let barStep = 1;
  while (barPx * barStep < 36) barStep *= 2;
  const beatStep = pixelsPerBeat >= 14 ? 1 : null;
  let sub: number | null = null;
  for (const s of [0.25, 0.5]) {
    if (pixelsPerBeat * s >= 10) {
      sub = s;
      break;
    }
  }
  return { barStep, beatStep, sub };
}

export type LineKind = "bar" | "beat" | "sub";

export interface GridLine {
  /** The line's pixel column (the line covers x to x + 1). */
  readonly x: number;
  readonly kind: LineKind;
}

/** The first bar to draw for a view starting at `beat` (never before bar 0). */
function firstBar(signatures: readonly TimeSignatureView[], beat: number): number {
  return Math.max(0, Math.floor(beatToBar(signatures, Math.max(0, beat)) + 1e-9));
}

/** `paint_grid`: the grid lines between the canvas columns `left` and `right`. */
export function gridLines(
  view: TimelineView,
  signatures: readonly TimeSignatureView[],
  left: number,
  right: number,
): GridLine[] {
  const lines: GridLine[] = [];
  const b1 = xToBeat(view, right + 1);
  const at = (beat: number) => Math.round(beatToX(view, beat));
  let bar = firstBar(signatures, xToBeat(view, left - 1));
  for (let guard = 0; guard < 20000; guard++, bar++) {
    const start = barToBeat(signatures, bar);
    if (start > b1) break;
    const sig = signatureAtBar(signatures, bar);
    const { barStep, beatStep, sub } = gridSteps(view.pixelsPerBeat, sig.beats_per_bar);
    const length = sig.beats_per_bar;
    if (bar % Math.trunc(barStep) === 0) lines.push({ x: at(start), kind: "bar" });
    const unit = 4 / sig.denominator;
    if (beatStep !== null && barStep === 1) {
      const step = sub !== null && sub < unit ? sub : null;
      for (let k = 1; unit * k < length - 1e-9; k++) lines.push({ x: at(start + unit * k), kind: "beat" });
      if (step !== null) {
        const n = pyRound(length / step);
        for (let j = 1; j < n; j++) {
          const off = j * step;
          if (Math.abs(off / unit - pyRound(off / unit)) < 1e-6) continue;
          lines.push({ x: at(start + off), kind: "sub" });
        }
      }
    }
  }
  return lines;
}

/** One mark of the ruler. */
export type RulerMark =
  | { readonly kind: "bar"; readonly x: number; readonly label: string }
  | { readonly kind: "signature"; readonly x: number; readonly label: string }
  | { readonly kind: "tick"; readonly x: number; readonly label: string | null };

/**
 * `Ruler._paint_bars`: numbered bar lines (thinned like the grid), the time signatures where
 * they change (and at bar 1), and beat ticks, labelled "bar.beat" from 44 px per beat, when
 * whole bars are drawn. `width` is the ruler's width.
 */
export function rulerMarks(
  view: TimelineView,
  signatures: readonly TimeSignatureView[],
  width: number,
): RulerMark[] {
  const marks: RulerMark[] = [];
  const b1 = xToBeat(view, width + 40);
  const sigBars = new Set(signatures.map((sig) => sig.bar));
  const at = (beat: number) => Math.round(beatToX(view, beat));
  let bar = firstBar(signatures, xToBeat(view, -40));
  for (let guard = 0; guard < 20000; guard++, bar++) {
    const start = barToBeat(signatures, bar);
    if (start > b1) break;
    const sig = signatureAtBar(signatures, bar);
    const { barStep, beatStep } = gridSteps(view.pixelsPerBeat, sig.beats_per_bar);
    const x = at(start);
    if (bar % Math.trunc(barStep) === 0) marks.push({ kind: "bar", x, label: String(bar + 1) });
    if (sigBars.has(bar) && (signatures.length > 1 || bar === 0)) {
      marks.push({ kind: "signature", x, label: signatureText(sig) });
    }
    if (beatStep !== null && barStep === 1) {
      const unit = 4 / sig.denominator;
      const labelled = unit * view.pixelsPerBeat >= 44;
      for (let k = 1; unit * k < sig.beats_per_bar - 1e-9; k++) {
        marks.push({ kind: "tick", x: at(start + unit * k), label: labelled ? `${bar + 1}.${k + 1}` : null });
      }
    }
  }
  return marks;
}
