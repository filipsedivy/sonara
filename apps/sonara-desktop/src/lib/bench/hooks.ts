// What the benchmarks (timeline.ts, meters.ts) reach inside the page: the arrangement canvas the
// timeline created, and the moments a live frame arrived and its meters were drawn. Set by
// Timeline.svelte and the benchmarks; empty outside a benchmark.

import type { NoteIndex, RollView } from "../pianoroll/model";
import type { RollCanvas } from "../pianoroll/paint";
import type { Arrangement } from "../timeline/arrangement";

/** What the piano roll benchmark reaches (PianoRoll.svelte sets it while the roll is shown). */
export interface PianoRollHook {
  readonly canvas: RollCanvas;
  /** The note grid's element, which takes the pointer events. */
  readonly grid: HTMLElement;
  /** The clip shown and its notes as drawn, or null. */
  readonly clipId: () => string | null;
  readonly index: () => NoteIndex | null;
  readonly view: () => RollView;
  setView(view: RollView): void;
  /** Grows whenever a gesture changed what the grid draws (a preview, the selection, a band). */
  readonly changes: () => number;
  /** Whether a finished gesture's notes are still held until the view has its last edit. */
  readonly held: () => boolean;
}

export const benchHooks: {
  arrangement: Arrangement | null;
  pianoRoll: PianoRollHook | null;
  /** Called when the live frame `sequence` arrived from the channel, at `receivedMs`. */
  liveReceived: ((sequence: number, receivedMs: number) => void) | null;
  /** Called after the meters drew the live frame `sequence`, at `drawnMs` (performance.now()). */
  meterShown: ((sequence: number, drawnMs: number) => void) | null;
  /**
   * When set, a new project view is applied at once and measured: the page's update (`updateMs`)
   * and the style and layout it causes (`layoutMs`).
   */
  viewShown: ((updateMs: number, layoutMs: number, patched: boolean) => void) | null;
  /** The script time of the session's animation frame: the frame shown and the meters drawn. */
  frameWork: ((ms: number) => void) | null;
} = { arrangement: null, pianoRoll: null, liveReceived: null, meterShown: null, viewShown: null, frameWork: null };
