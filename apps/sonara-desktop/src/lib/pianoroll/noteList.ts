// The note grid's notes for assistive technology ("at least readable notes ... and the existing
// keys"): the canvas draws them, so a
// list box of transparent options over it reads each one ("C4, bar 2 beat 1, 1/8, velocity 100,
// selected, 3 of 24"). Only the notes the grid shows are options, at most `MAX_NOTE_OPTIONS` of
// them, plus the focused one wherever it is: a zoomed-out clip of 10,000 notes must not become
// 10,000 elements. The grid's own keys act on the selection, so ↑ and ↓ keep transposing;
// VoiceOver's press on an option selects its note. Plain functions; PianoRoll.svelte renders them.

import { noteOptionLabel } from "../a11y/labels";
import type { MidiClipView, TimeSignatureView } from "../types";
import { beatX, contentToTimeline, noteName, type NoteIndex, pitchY, type RollView, type Viewport, xBeat, yPitch } from "./model";

/** The most notes the list shows at once. */
export const MAX_NOTE_OPTIONS = 256;

/** One note's option. */
export interface NoteOption {
  readonly id: string;
  readonly label: string;
  readonly selected: boolean;
  /** Its place among the clip's notes (start, pitch, ID order), from 1. */
  readonly position: number;
  /** The clip's notes. */
  readonly count: number;
  /** Where the grid draws it, in CSS pixels from the grid's top left. */
  readonly rect: { readonly x: number; readonly y: number; readonly width: number; readonly height: number };
}

/** The element id of a note's option. */
export function noteOptionId(noteId: string): string {
  return `note-option-${noteId}`;
}

/** What a note's name is read from: the pitch, or the GM name on a drum track. */
export interface NoteNaming {
  readonly drums: boolean;
  readonly drumLabels: ReadonlyMap<number, string>;
}

/** A note's name: "C4", or the GM name on a drum track ("Acoustic Snare"). */
export function noteSpokenName(pitch: number, naming: NoteNaming): string {
  return (naming.drums ? naming.drumLabels.get(pitch) : undefined) ?? noteName(pitch);
}

/**
 * The options of the notes the grid shows at `view` in `viewport`, in the index's order, at most
 * `MAX_NOTE_OPTIONS` of them, and of `keep` (the focused note) wherever it is.
 */
export function noteOptions(
  index: NoteIndex,
  clip: MidiClipView,
  signatures: readonly TimeSignatureView[] | null,
  view: RollView,
  viewport: Viewport,
  selected: ReadonlySet<string>,
  naming: NoteNaming,
  keep: string | null,
): NoteOption[] {
  const b0 = xBeat(view, 0);
  const b1 = xBeat(view, Math.max(0, viewport.width));
  const pHi = yPitch(view, 0);
  const pLo = yPitch(view, Math.max(0, viewport.height));
  const shown = index.visible(b0, b1, Math.max(0, pLo), Math.min(127, pHi)).slice(0, MAX_NOTE_OPTIONS);
  const kept = keep === null ? -1 : index.ids.indexOf(keep);
  if (kept >= 0 && !shown.includes(kept)) {
    shown.push(kept);
    shown.sort((a, b) => a - b);
  }
  return shown.map((i) => {
    const note = index.notes[i];
    return {
      id: note.id,
      label: noteOptionLabel(note, noteSpokenName(note.pitch, naming), contentToTimeline(clip, note.start), signatures),
      selected: selected.has(note.id),
      position: i + 1,
      count: index.size,
      rect: {
        x: beatX(view, note.start),
        y: pitchY(view, note.pitch),
        width: Math.max(1, note.length * view.ppb),
        height: view.rowH,
      },
    };
  });
}

/**
 * The view that shows `note` whole in `viewport`: unchanged when it does, else the least scroll
 * that shows it (its start first when it is wider than the grid). `clampView` keeps it in range.
 */
export function revealNote(view: RollView, note: { start: number; length: number; pitch: number }, viewport: Viewport): RollView {
  const margin = 8;
  const x0 = beatX(view, note.start);
  const x1 = beatX(view, note.start + note.length);
  let start = view.x0;
  if (x0 < 0) start = xBeat(view, x0 - margin);
  else if (x1 > viewport.width) start = xBeat(view, Math.min(x0 - margin, x1 - viewport.width + margin));
  const top = pitchY(view, note.pitch);
  let y0 = view.y0;
  if (top < 0) y0 = view.y0 + top;
  else if (top + view.rowH > viewport.height) y0 = view.y0 + top + view.rowH - viewport.height;
  return start === view.x0 && y0 === view.y0 ? view : { ...view, x0: start, y0 };
}
