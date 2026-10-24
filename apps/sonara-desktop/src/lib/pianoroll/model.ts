// The piano roll's editing model (`PianoRollEditor`, `View` and `NoteIndex` in
// sonara/ui/piano_roll/editor.py), as plain values and functions: the edited clip's notes in a
// columnar index for virtualised painting and hit testing, the view's zoom and scroll, the mapping
// between the clip's content beats and the timeline, the snap in content beats, and the edits of
// every gesture. The prototype's arithmetic is kept line by line; the edits go to sonara-app
// (`edit::Edit`), which runs the commands, so the page holds no editing logic of its own.
//
// Coordinates: x is the clip's content beat (a note's `start`), y the pitch with 127 at the top.
// Timeline beats map to content beats through the clip's `start` and `content_offset`, and its
// `loop_length` when it loops.

import { gridBeats, type Snap, snapBeat, type SnapMode } from "../state/snap";
import { EPSILON, pyRound, signatureAtBeat } from "../time/signatures";
import type { Edit, MidiClipView, NoteView, TimeSignatureView } from "../types";

export const MIN_PPB = 6;
export const MAX_PPB = 1200;
export const MIN_ROW = 5;
export const MAX_ROW = 40;

/** The lengths of new notes: the grid, fixed values in beats, or a bar (0). */
export const NOTE_LENGTHS = [
  ["Grid", null],
  ["1/16", 0.25],
  ["1/8", 0.5],
  ["1/4", 1],
  ["1/2", 2],
  ["Bar", 0],
] as const;

export type NoteLengthName = (typeof NOTE_LENGTHS)[number][0];

/** The shortest note a resize leaves (1/64). */
export const MIN_LENGTH = 1 / 64;

/** `NOTE_NAMES` of sonara.domain.midi. */
const NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/** `note_name`: scientific pitch notation with C4 = 60. */
export function noteName(pitch: number): string {
  return `${NOTE_NAMES[((pitch % 12) + 12) % 12]}${Math.floor(pitch / 12) - 1}`;
}

/** The pitch classes of the black keys. */
export const BLACK_KEYS: ReadonlySet<number> = new Set([1, 3, 6, 8, 10]);

export function isBlackKey(pitch: number): boolean {
  return BLACK_KEYS.has(((pitch % 12) + 12) % 12);
}

// -- the view ----------------------------------------------------------------------------------

/**
 * Zoom and scroll (`View`): `x0` is the content beat at the grid's left edge, `y0` the vertical
 * scroll in pixels (pitch 127's row starts at `-y0`).
 */
export interface RollView {
  readonly ppb: number;
  readonly rowH: number;
  readonly x0: number;
  readonly y0: number;
}

export const DEFAULT_ROLL_VIEW: RollView = { ppb: 80, rowH: 14, x0: 0, y0: 0 };

/** The grid's size in CSS pixels (`viewport_w`, `viewport_h`). */
export interface Viewport {
  readonly width: number;
  readonly height: number;
}

export function beatX(view: RollView, beat: number): number {
  return (beat - view.x0) * view.ppb;
}

export function xBeat(view: RollView, x: number): number {
  return view.x0 + x / view.ppb;
}

/** The top of `pitch`'s row. */
export function pitchY(view: RollView, pitch: number): number {
  return (127 - pitch) * view.rowH - view.y0;
}

export function yPitch(view: RollView, y: number): number {
  return 127 - Math.floor((y + view.y0) / view.rowH);
}

export function contentHeight(view: RollView): number {
  return 128 * view.rowH;
}

// -- the note index ----------------------------------------------------------------------------

/** The order of sonara-core's notes: start, pitch, ID. */
function noteOrder(a: NoteView, b: NoteView): number {
  return a.start - b.start || a.pitch - b.pitch || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);
}

function bisectLeft(values: Float64Array, value: number): number {
  let lo = 0;
  let hi = values.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (values[mid] < value) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

function bisectRight(values: Float64Array, value: number): number {
  let lo = 0;
  let hi = values.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (values[mid] <= value) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** A columnar copy of a clip's notes for virtualised painting and hit testing (`NoteIndex`). */
export class NoteIndex {
  readonly notes: readonly NoteView[];
  readonly ids: readonly string[];
  readonly starts: Float64Array;
  readonly ends: Float64Array;
  readonly pitches: Int32Array;
  readonly velocities: Int32Array;
  readonly maxLength: number;
  private readonly byId = new Map<string, number>();

  constructor(notes: readonly NoteView[]) {
    // sonara-core keeps a clip's notes sorted; a view built elsewhere is sorted here.
    let sorted = true;
    for (let i = 1; i < notes.length && sorted; i++) sorted = noteOrder(notes[i - 1], notes[i]) <= 0;
    this.notes = sorted ? notes : [...notes].sort(noteOrder);
    const n = this.notes.length;
    this.starts = new Float64Array(n);
    this.ends = new Float64Array(n);
    this.pitches = new Int32Array(n);
    this.velocities = new Int32Array(n);
    const ids: string[] = new Array(n);
    let longest = 0;
    this.notes.forEach((note, i) => {
      ids[i] = note.id;
      this.starts[i] = note.start;
      this.ends[i] = note.start + note.length;
      this.pitches[i] = note.pitch;
      this.velocities[i] = note.velocity;
      longest = Math.max(longest, note.length);
      this.byId.set(note.id, i);
    });
    this.ids = ids;
    this.maxLength = longest;
  }

  get size(): number {
    return this.ids.length;
  }

  has(id: string): boolean {
    return this.byId.has(id);
  }

  note(id: string): NoteView | undefined {
    const i = this.byId.get(id);
    return i === undefined ? undefined : this.notes[i];
  }

  /** The indices of the notes that intersect `[b0, b1)` and the pitch range, ascending. */
  visible(b0: number, b1: number, pLo = 0, pHi = 127): number[] {
    if (this.size === 0) return [];
    const lo = bisectLeft(this.starts, b0 - this.maxLength);
    const hi = bisectLeft(this.starts, b1);
    const out: number[] = [];
    for (let i = lo; i < hi; i++) {
      const pitch = this.pitches[i];
      if (this.ends[i] > b0 && pitch >= pLo && pitch <= pHi) out.push(i);
    }
    return out;
  }

  /** The index of the topmost (latest-starting) note under `(beat, pitch)`. */
  at(beat: number, pitch: number): number | null {
    let found: number | null = null;
    for (const i of this.visible(beat, beat + 1e-9, pitch, pitch)) {
      if (this.starts[i] <= beat && beat < this.ends[i]) found = i;
    }
    return found;
  }

  /** The notes whose start lies within `tolerance` of `beat` (the velocity lane's search). */
  startsNear(beat: number, tolerance: number): [number, number] {
    return [bisectLeft(this.starts, beat - tolerance), bisectRight(this.starts, beat + tolerance)];
  }

  /** The highest note end, 0 without notes. */
  get lastEnd(): number {
    let end = 0;
    for (const e of this.ends) end = Math.max(end, e);
    return end;
  }
}

// -- the clip and the timeline -----------------------------------------------------------------

/** What the editor maps and snaps with: the clip, the project's signatures and the snap grid. */
export interface RollContext {
  readonly clip: MidiClipView;
  readonly signatures: readonly TimeSignatureView[] | null;
  readonly snap: Snap;
}

/** `content_to_timeline`. */
export function contentToTimeline(clip: MidiClipView, content: number): number {
  return clip.start + (content - clip.content_offset);
}

/** Python's `%` for floats: the result takes the divisor's sign. */
function pyMod(value: number, divisor: number): number {
  const rem = value % divisor;
  return rem !== 0 && rem < 0 !== divisor < 0 ? rem + divisor : rem;
}

/** `timeline_to_content`: the content beat played at timeline `beat`; null outside the clip. */
export function timelineToContent(clip: MidiClipView, beat: number): number | null {
  if (!(clip.start - EPSILON <= beat && beat < clip.start + clip.length)) return null;
  let c = clip.content_offset + (beat - clip.start);
  if (clip.loop_length !== null) c = pyMod(c, clip.loop_length);
  return c;
}

/** `playable_range`: the content heard when the clip plays. */
export function playableRange(clip: MidiClipView): [number, number] {
  if (clip.loop_length !== null) return [0, clip.loop_length];
  return [clip.content_offset, clip.content_offset + clip.length];
}

/** `is_playable`. */
export function isPlayable(clip: MidiClipView, start: number): boolean {
  const [a, b] = playableRange(clip);
  return a - EPSILON <= start && start < b - EPSILON;
}

/**
 * `snap`: a content beat on the project grid, in timeline coordinates (clips that do not start on
 * the grid and time-signature changes are honoured); off or bypassed, only clamped at 0.
 */
export function snapContent(ctx: RollContext, content: number, bypass = false, mode: SnapMode = "nearest"): number {
  const offset = contentToTimeline(ctx.clip, 0);
  let snapped = snapBeat(ctx.snap, content + offset, ctx.signatures, mode, bypass);
  if (bypass || !ctx.snap.enabled) snapped = content + offset;
  return Math.max(0, snapped - offset);
}

/** `grid_beats`: the grid at content beat `atContent`. */
export function gridAt(ctx: RollContext, atContent = 0): number {
  return gridBeats(ctx.snap, ctx.signatures, contentToTimeline(ctx.clip, atContent));
}

/** `bar_length`: a bar at the clip's content beat 0. */
export function barLengthAt(ctx: RollContext): number {
  if (!ctx.signatures || ctx.signatures.length === 0) return 4;
  return signatureAtBeat(ctx.signatures, contentToTimeline(ctx.clip, 0)).beats_per_bar;
}

/** `new_note_length`. */
export function newNoteLength(ctx: RollContext, name: NoteLengthName, atContent = 0): number {
  const entry = NOTE_LENGTHS.find(([choice]) => choice === name);
  if (!entry) return 0.25;
  const value = entry[1];
  if (value === null) return Math.max(gridAt(ctx, atContent), MIN_LENGTH);
  if (value === 0) return barLengthAt(ctx);
  return value;
}

// -- zoom and scroll ---------------------------------------------------------------------------

/** `content_extent`: the rightmost beat worth scrolling to. */
export function contentExtent(clip: MidiClipView | null, index: NoteIndex | null): number {
  let end = 16;
  if (clip) end = Math.max(end, playableRange(clip)[1], clip.content_offset + clip.length);
  if (index && index.size > 0) end = Math.max(end, index.lastEnd);
  return end + 4;
}

/** `clamp_view`. */
export function clampView(view: RollView, viewport: Viewport, extent: number): RollView {
  const width = Math.max(1, viewport.width);
  const height = Math.max(1, viewport.height);
  const ppb = Math.min(Math.max(view.ppb, MIN_PPB), MAX_PPB);
  const rowH = Math.min(Math.max(view.rowH, MIN_ROW), MAX_ROW);
  const y0 = Math.min(Math.max(view.y0, 0), Math.max(0, 128 * rowH - height));
  const x0 = Math.min(Math.max(view.x0, -1), Math.max(0, extent - (width / ppb) * 0.25));
  if (ppb === view.ppb && rowH === view.rowH && x0 === view.x0 && y0 === view.y0) return view;
  return { ppb, rowH, x0, y0 };
}

/** `zoom_time`: around `anchorX` (the middle when null). */
export function zoomTime(view: RollView, factor: number, viewport: Viewport, extent: number, anchorX: number | null = null): RollView {
  const ax = anchorX ?? viewport.width / 2;
  const beat = xBeat(view, ax);
  const ppb = Math.min(Math.max(view.ppb * factor, MIN_PPB), MAX_PPB);
  return clampView({ ...view, ppb, x0: beat - ax / ppb }, viewport, extent);
}

/** `zoom_pitch`: around `anchorY` (the middle when null). */
export function zoomPitch(view: RollView, factor: number, viewport: Viewport, extent: number, anchorY: number | null = null): RollView {
  const ay = anchorY ?? viewport.height / 2;
  const rows = (ay + view.y0) / view.rowH;
  const rowH = Math.min(Math.max(view.rowH * factor, MIN_ROW), MAX_ROW);
  return clampView({ ...view, rowH, y0: rows * rowH - ay }, viewport, extent);
}

/** `scroll` by pixels. */
export function scrollBy(view: RollView, dx: number, dy: number, viewport: Viewport, extent: number): RollView {
  return clampView({ ...view, x0: view.x0 + dx / view.ppb, y0: view.y0 + dy }, viewport, extent);
}

/**
 * `center_on_content` (Fit, and a new clip): the notes (or the playable range) fill about
 * 92 % of the width, rows of 8–16 px fit the pitch range plus a margin, centred on it.
 */
export function centerOnContent(
  view: RollView,
  clip: MidiClipView,
  index: NoteIndex,
  drums: boolean,
  viewport: Viewport,
): RollView {
  const [a, b] = playableRange(clip);
  let lo: number;
  let hi: number;
  let pLo: number;
  let pHi: number;
  if (index.size > 0) {
    lo = Infinity;
    hi = -Infinity;
    pLo = 127;
    pHi = 0;
    for (let i = 0; i < index.size; i++) {
      lo = Math.min(lo, index.starts[i]);
      hi = Math.max(hi, index.ends[i]);
      pLo = Math.min(pLo, index.pitches[i]);
      pHi = Math.max(pHi, index.pitches[i]);
    }
  } else {
    lo = a;
    hi = Math.max(b, a + 4);
    pLo = pHi = drums ? 38 : 60;
  }
  const span = Math.max(hi - lo, b - a, 1);
  const width = Math.max(1, viewport.width);
  const height = Math.max(1, viewport.height);
  const ppb = Math.min(Math.max((width * 0.92) / span, 12), 240);
  const x0 = Math.min(lo, a) - 0.04 * span;
  const rows = pHi - pLo + 5;
  const rowH = Math.min(Math.max(height / rows, MIN_ROW + 3), 16);
  const centre = (pLo + pHi) / 2;
  const y0 = (127 - centre + 0.5) * rowH - height / 2;
  return clampView({ ppb, rowH, x0, y0 }, viewport, contentExtent(clip, index));
}

// -- selection ---------------------------------------------------------------------------------

/** `notes_in_rect`: the notes in the beat and pitch rectangle (either corner first). */
export function notesInRect(index: NoteIndex, b0: number, b1: number, p0: number, p1: number): string[] {
  const out: string[] = [];
  for (const i of index.visible(Math.min(b0, b1), Math.max(b0, b1), Math.min(p0, p1), Math.max(p0, p1))) {
    out.push(index.ids[i]);
  }
  return out;
}

/** `selected_notes`: the selected notes, in the clip's order. */
export function selectedNotes(index: NoteIndex, selected: ReadonlySet<string>): NoteView[] {
  return index.notes.filter((note) => selected.has(note.id));
}

/** `targets`: the selected notes, or every note when none is selected (for transforms). */
export function targets(index: NoteIndex, selected: ReadonlySet<string>): NoteView[] {
  const chosen = selectedNotes(index, selected);
  return chosen.length > 0 ? chosen : [...index.notes];
}

/** The notes of `pitch` (a click on a key). */
export function notesOfPitch(index: NoteIndex, pitch: number): string[] {
  return index.ids.filter((_, i) => index.pitches[i] === pitch);
}

// -- edits -------------------------------------------------------------------------------------

/** `add_note`: a note at the floor-snapped beat; null outside the MIDI range. */
export function addNoteEdit(
  ctx: RollContext,
  content: number,
  pitch: number,
  lengthName: NoteLengthName,
  velocity: number,
  bypass = false,
): Edit | null {
  if (pitch < 0 || pitch > 127) return null;
  const start = snapContent(ctx, content, bypass, "floor");
  return {
    op: "add_notes",
    clip_id: ctx.clip.id,
    notes: [{ pitch, start, length: newNoteLength(ctx, lengthName, start), velocity }],
  };
}

/** A note moved or resized by a gesture: what the page draws until the view has it. */
export type NotePatch = Pick<NoteView, "id" | "pitch" | "start" | "length" | "velocity">;

/** `move_notes`: `originals` (captured at the press) by a delta, clamped to 0–127 and beat 0. */
export function movedNotes(originals: readonly NoteView[], dBeats: number, dPitch: number): NotePatch[] {
  if (originals.length === 0) return [];
  let earliest = Infinity;
  let low = 127;
  let high = 0;
  for (const note of originals) {
    earliest = Math.min(earliest, note.start);
    low = Math.min(low, note.pitch);
    high = Math.max(high, note.pitch);
  }
  const beats = Math.max(dBeats, -earliest);
  const semitones = Math.min(Math.max(dPitch, -low), 127 - high);
  return originals.map((note) => ({
    id: note.id,
    pitch: note.pitch + semitones,
    start: Math.max(0, note.start + beats),
    length: note.length,
    velocity: note.velocity,
  }));
}

/** `resize_notes`: `originals` lengthened by `dBeats`, at least `minLength`. */
export function resizedNotes(originals: readonly NoteView[], dBeats: number, minLength = MIN_LENGTH): NotePatch[] {
  return originals.map((note) => ({
    id: note.id,
    pitch: note.pitch,
    start: note.start,
    length: Math.max(minLength, note.length + dBeats),
    velocity: note.velocity,
  }));
}

/** The `EditNotes` of a move. */
export function moveEdit(clipId: string, moved: readonly NotePatch[]): Edit {
  return { op: "edit_notes", clip_id: clipId, edits: moved.map((n) => ({ note_id: n.id, pitch: n.pitch, start: n.start })) };
}

/** The `EditNotes` of a resize. */
export function resizeEdit(clipId: string, resized: readonly NotePatch[]): Edit {
  return { op: "edit_notes", clip_id: clipId, edits: resized.map((n) => ({ note_id: n.id, length: n.length })) };
}

/** `delete_notes`: the notes of `ids` that exist; null when none does. */
export function deleteEdit(clipId: string, index: NoteIndex, ids: Iterable<string>): Edit | null {
  const doomed = [...ids].filter((id) => index.has(id));
  return doomed.length > 0 ? { op: "delete_notes", clip_id: clipId, note_ids: doomed } : null;
}

/**
 * `duplicate_selection` (PR-06 ⌘D): copies of the selection after its span, rounded up to the
 * grid when snapping is on.
 */
export function duplicateEdit(ctx: RollContext, chosen: readonly NoteView[]): Edit | null {
  if (chosen.length === 0) return null;
  let lo = Infinity;
  let hi = -Infinity;
  for (const note of chosen) {
    lo = Math.min(lo, note.start);
    hi = Math.max(hi, note.start + note.length);
  }
  const grid = ctx.snap.enabled ? gridAt(ctx, lo) : 0;
  let span = hi - lo;
  if (grid > 0) span = Math.ceil(span / grid - 1e-9) * grid;
  return {
    op: "add_notes",
    clip_id: ctx.clip.id,
    notes: chosen.map((n) => ({ pitch: n.pitch, start: n.start + span, length: n.length, velocity: n.velocity, channel: n.channel })),
  };
}

/** The nudge step of ← and →: the grid, or 1/16 with snapping off. */
export function nudgeStep(ctx: RollContext): number {
  return ctx.snap.enabled ? gridAt(ctx, 0) : 1 / 16;
}

/** The refusal of a transpose past the MIDI range. */
export const TRANSPOSE_REFUSED = "transposing would leave the MIDI range";

/** `transpose`: the `EditNotes`, `"refused"` past the MIDI range, or null without notes. */
export function transposeEdit(clipId: string, notes: readonly NoteView[], semitones: number): Edit | "refused" | null {
  if (notes.length === 0) return null;
  let low = 127;
  let high = 0;
  for (const note of notes) {
    low = Math.min(low, note.pitch);
    high = Math.max(high, note.pitch);
  }
  if (low + semitones < 0 || high + semitones > 127) return "refused";
  return { op: "edit_notes", clip_id: clipId, edits: notes.map((n) => ({ note_id: n.id, pitch: n.pitch + semitones })) };
}

/**
 * `quantize`: the selection (every note when none is selected) toward the grid at content
 * beat 0, at `strength`; sonara-app runs `quantize` and "Quantize Notes". Null without notes.
 */
export function quantizeEdit(ctx: RollContext, index: NoteIndex, selected: ReadonlySet<string>, strength: number): Edit | null {
  if (index.size === 0) return null;
  const chosen = selectedNotes(index, selected).map((note) => note.id);
  return { op: "quantize_notes", clip_id: ctx.clip.id, note_ids: chosen, grid: gridAt(ctx, 0), strength };
}

/** `set_velocities`: the `EditNotes` of the velocities, clamped to 1–127. */
export function velocityEdit(clipId: string, values: ReadonlyMap<string, number>): Edit | null {
  if (values.size === 0) return null;
  return {
    op: "edit_notes",
    clip_id: clipId,
    edits: [...values].map(([id, velocity]) => ({ note_id: id, velocity: Math.max(1, Math.min(127, velocity)) })),
  };
}

// -- the velocity lane -------------------------------------------------------------------------

/** `velocity_at`: the velocity at `y` in a lane `height` pixels high. */
export function velocityAt(y: number, height: number): number {
  const usable = Math.max(1, height - 6);
  return Math.max(1, Math.min(127, pyRound(((height - 3 - y) / usable) * 127)));
}

/** `note_at_x`: the note starting nearest `x` within 4 px, a selected one on a tie. */
export function noteAtX(index: NoteIndex, view: RollView, x: number, selected: ReadonlySet<string>): string | null {
  if (index.size === 0) return null;
  const beat = xBeat(view, x);
  const [lo, hi] = index.startsNear(beat, 4 / view.ppb);
  if (hi <= lo) return null;
  let best = lo;
  const key = (i: number): [number, number] => [Math.abs(index.starts[i] - beat), selected.has(index.ids[i]) ? 0 : 1];
  let bestKey = key(lo);
  for (let i = lo + 1; i < hi; i++) {
    const k = key(i);
    if (k[0] < bestKey[0] || (k[0] === bestKey[0] && k[1] < bestKey[1])) {
      best = i;
      bestKey = k;
    }
  }
  return index.ids[best];
}

/** `_apply_relative`: the selection's velocities shifted by the drag from `y0` to `y`. */
export function relativeVelocities(base: ReadonlyMap<string, number>, y0: number, y: number, height: number): Map<string, number> {
  let delta = pyRound(((y0 - y) / Math.max(1, height - 6)) * 127);
  let anchor = 0;
  for (const v of base.values()) anchor = Math.max(anchor, v);
  delta = Math.min(delta, 127 - anchor);
  const out = new Map<string, number>();
  for (const [id, v] of base) out.set(id, Math.max(1, Math.min(127, v + delta)));
  return out;
}
