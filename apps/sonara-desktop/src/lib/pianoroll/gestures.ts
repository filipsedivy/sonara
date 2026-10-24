// The piano roll's input (`NoteGrid`, `Keyboard`, `Ruler` and `VelocityLane` in
// sonara/ui/piano_roll/widgets.py): a click on empty space adds a note, a drag there draws a rubber
// band; a press on a note selects it, a drag of its body moves the selection and a
// drag of its right edge resizes it; a double-click deletes; the keys, the
// context menu, the wheel, a key of the keyboard and the ruler; the
// velocity lane paints velocities or shifts the selection's. The prototype's arithmetic is
// kept line by line.
//
// Undo steps, as in the prototype: a move or resize sends its edit on every move (`LiveGesture`,
// at most one per display frame) and seals the step on release; the velocity lane previews
// locally and commits one edit on release, then seals it; a click, a key or a menu item runs its
// edit alone (`bridge.execute`). The page draws a gesture's notes at once (`preview`), and keeps
// drawing them where the gesture left them until the project view has the gesture's last edit
// (`held`), so a release never shows the notes jump back.

import { LiveGesture, type GestureDeps } from "../gesture";
import type { MenuItem } from "../shell/menu";
import type { Edit, EditOutcome, NoteView } from "../types";
import {
  addNoteEdit,
  beatX,
  deleteEdit,
  duplicateEdit,
  gridAt,
  MIN_LENGTH,
  type NoteIndex,
  type NoteLengthName,
  type NotePatch,
  movedNotes,
  moveEdit,
  noteAtX,
  notesInRect,
  notesOfPitch,
  nudgeStep,
  quantizeEdit,
  relativeVelocities,
  resizedNotes,
  resizeEdit,
  type RollContext,
  type RollView,
  selectedNotes,
  snapContent,
  contentToTimeline,
  targets,
  TRANSPOSE_REFUSED,
  transposeEdit,
  velocityAt,
  velocityEdit,
  xBeat,
  yPitch,
} from "./model";

/** The right-edge zone of a note that resizes it (`EDGE_PX`). */
export const EDGE_PX = 5;
/** How far the pointer moves before a press becomes a drag (`DRAG_THRESHOLD`). */
export const DRAG_THRESHOLD = 4;

/** The modifier keys of a pointer or key event, as the prototype reads them. */
export interface Mods {
  /** ⇧ */
  readonly shift: boolean;
  /** ⌘ (Qt's Control on macOS) */
  readonly command: boolean;
  /** ⌥: bypasses snapping */
  readonly alt: boolean;
}

export const NO_MODS: Mods = { shift: false, command: false, alt: false };

/** What the gestures read: the clip as it is drawn now. */
export interface RollState {
  readonly ctx: RollContext;
  readonly index: NoteIndex;
  readonly view: RollView;
  readonly selected: ReadonlySet<string>;
  /** The toolbar's length and velocity of new notes, and the quantize strength (0–1). */
  readonly lengthName: NoteLengthName;
  readonly velocity: number;
  readonly strength: number;
}

/** What the gestures do. */
export interface RollActions {
  /** The note selection becomes `ids` (the session's `selectNotes`). */
  select(ids: string[]): void;
  /** A click's, key's or menu's edit, run alone; the outcome, or null when it was refused. */
  run(edit: Edit): Promise<EditOutcome | null>;
  /** A move's or resize's edits, one undo step until its `end`. */
  live(): LiveGesture;
  /** The velocity lane's edit: committed once, then sealed; the outcome's version, or null. */
  commit(edit: Edit): Promise<number | null>;
  /** A status message (4 s). */
  message(text: string): void;
  /** Something the canvas draws changed. */
  changed(): void;
  /** MW-27's seek, in timeline beats. */
  seek(beat: number): void;
  /** Zoom or scroll: the new view. */
  setView(view: RollView): void;
}

/** A drag in progress on the grid (`_Drag`). */
export interface GridDrag {
  mode: "pending" | "rubber" | "move" | "resize";
  readonly pressX: number;
  readonly pressY: number;
  readonly pressBeat: number;
  readonly pressPitch: number;
  readonly anchor: NoteView | null;
  readonly originals: readonly NoteView[];
  readonly additive: boolean;
  last: [number, number] | null;
  moved: boolean;
  /** The rubber band in grid pixels. */
  rect: { x: number; y: number; width: number; height: number } | null;
  gesture: LiveGesture | null;
}

/** The grid's cursor. */
export type Cursor = "crosshair" | "grab" | "ew-resize";

/** The note under a point, and whether the point is on its right (resize) edge. */
export interface NoteHit {
  readonly note: NoteView;
  readonly edge: boolean;
}

/** `hit`: the note under `(x, y)`; the right-edge zone wins, even slightly outside a note. */
export function hitNote(index: NoteIndex, view: RollView, x: number, y: number): NoteHit | null {
  const beat = xBeat(view, x);
  const pitch = yPitch(view, y);
  const edgeBeats = EDGE_PX / view.ppb;
  const near = index.visible(beat - edgeBeats, beat + edgeBeats, pitch, pitch);
  for (let k = near.length - 1; k >= 0; k--) {
    const i = near[k];
    const xs = beatX(view, index.starts[i]);
    const xe = beatX(view, index.ends[i]);
    // Tiny notes: only the very end (or just past it) resizes; the body moves.
    const nearEnd = Math.abs(xe - x) <= EDGE_PX && (xe - xs >= 3 * EDGE_PX || x >= xe - 1.5);
    if (nearEnd) return { note: index.notes[i], edge: true };
  }
  const i = index.at(beat, pitch);
  return i === null ? null : { note: index.notes[i], edge: false };
}

/** `set_selection`: `ids` replace the selection, join it (`additive`) or toggle in it. */
export function noteSelection(
  current: ReadonlySet<string>,
  ids: readonly string[],
  index: NoteIndex,
  mode: "replace" | "additive" | "toggle" = "replace",
): string[] {
  const next = new Set(mode === "replace" ? [] : current);
  for (const id of ids) {
    if (mode === "toggle" && next.has(id)) next.delete(id);
    else next.add(id);
  }
  return [...next].filter((id) => index.has(id));
}

/** The notes of `index` with `patches` applied, as the page draws them. */
export function patched(index: NoteIndex, patches: ReadonlyMap<string, NotePatch>): NoteView[] {
  return index.notes.map((note) => {
    const patch = patches.get(note.id);
    return patch ? { ...note, ...patch } : note;
  });
}

export class GridGestures {
  drag: GridDrag | null = null;
  /** The notes a move or resize draws where it put them (by note ID). */
  readonly preview = new Map<string, NotePatch>();
  /** The note the last click added, for a double-click on empty space. */
  private added: Promise<string | null> | null = null;

  constructor(
    private readonly state: () => RollState | null,
    private readonly actions: RollActions,
  ) {}

  /** `mousePressEvent` with the left button. */
  press(x: number, y: number, mods: Mods): void {
    const state = this.state();
    this.added = null;
    if (!state) return;
    const { view, index } = state;
    const beat = xBeat(view, x);
    const pitch = yPitch(view, y);
    const additive = mods.shift;
    const hit = hitNote(index, view, x, y);
    const base = { pressX: x, pressY: y, pressBeat: beat, pressPitch: pitch, additive, last: null, moved: false, rect: null, gesture: null };
    if (!hit) {
      this.drag = { ...base, mode: "pending", anchor: null, originals: [] };
      return;
    }
    let selected = state.selected;
    if (additive) {
      const next = noteSelection(selected, [hit.note.id], index, "toggle");
      this.actions.select(next);
      if (!next.includes(hit.note.id)) {
        this.drag = null;
        return;
      }
      selected = new Set(next);
    } else if (!selected.has(hit.note.id)) {
      const next = noteSelection(selected, [hit.note.id], index);
      this.actions.select(next);
      selected = new Set(next);
    }
    const chosen = selectedNotes(index, selected);
    this.drag = {
      ...base,
      mode: hit.edge ? "resize" : "move",
      anchor: hit.note,
      originals: chosen.length > 0 ? chosen : [hit.note],
    };
  }

  /** `mouseMoveEvent`: the drag's step, or the cursor while nothing is pressed. */
  move(x: number, y: number, mods: Mods): Cursor {
    const state = this.state();
    const drag = this.drag;
    if (!state) return "crosshair";
    if (!drag) {
      const hit = hitNote(state.index, state.view, x, y);
      return hit?.edge ? "ew-resize" : hit ? "grab" : "crosshair";
    }
    if (!drag.moved && Math.abs(x - drag.pressX) + Math.abs(y - drag.pressY) < DRAG_THRESHOLD) {
      return drag.mode === "resize" ? "ew-resize" : "crosshair";
    }
    drag.moved = true;
    const { view, index, ctx } = state;
    const beat = xBeat(view, x);
    if (drag.mode === "pending") drag.mode = "rubber";
    if (drag.mode === "rubber") {
      drag.rect = {
        x: Math.min(drag.pressX, x),
        y: Math.min(drag.pressY, y),
        width: Math.abs(x - drag.pressX),
        height: Math.abs(y - drag.pressY),
      };
      const ids = notesInRect(index, drag.pressBeat, beat, drag.pressPitch, yPitch(view, y));
      this.actions.select(noteSelection(state.selected, ids, index, drag.additive ? "additive" : "replace"));
      this.actions.changed();
      return "crosshair";
    }
    const anchor = drag.anchor;
    if (!anchor) return "crosshair";
    if (drag.mode === "move") {
      const raw = anchor.start + (beat - drag.pressBeat);
      const dBeats = snapContent(ctx, raw, mods.alt) - anchor.start;
      const dPitch = yPitch(view, y) - drag.pressPitch;
      if (!drag.last || drag.last[0] !== dBeats || drag.last[1] !== dPitch) {
        drag.last = [dBeats, dPitch];
        this.show(drag, movedNotes(drag.originals, dBeats, dPitch), (moved) => moveEdit(ctx.clip.id, moved));
      }
      return "grab";
    }
    const anchorEnd = anchor.start + anchor.length;
    const rawEnd = anchorEnd + (beat - drag.pressBeat);
    let newEnd = snapContent(ctx, rawEnd, mods.alt);
    if (newEnd <= anchor.start + 1e-9) {
      newEnd = anchor.start + (mods.alt ? MIN_LENGTH : gridAt(ctx, anchor.start));
    }
    const dBeats = newEnd - anchorEnd;
    if (!drag.last || drag.last[0] !== dBeats || drag.last[1] !== 0) {
      drag.last = [dBeats, 0];
      this.show(drag, resizedNotes(drag.originals, dBeats), (resized) => resizeEdit(ctx.clip.id, resized));
    }
    return "ew-resize";
  }

  /** The preview of a move or resize, drawn at once, and its edit sent on the gesture. */
  private show(drag: GridDrag, notes: NotePatch[], edit: (notes: NotePatch[]) => Edit): void {
    this.preview.clear();
    for (const note of notes) this.preview.set(note.id, note);
    drag.gesture ??= this.actions.live();
    drag.gesture.move(edit(notes));
    this.actions.changed();
  }

  /**
   * `mouseReleaseEvent`: a click on empty space adds a note (⇧ clears the selection instead);
   * a move or resize seals its step once its last edit has landed. Resolves with the gesture's
   * end; the preview stays for the caller to hold until the view has it.
   */
  async release(mods: Mods): Promise<void> {
    const drag = this.drag;
    this.drag = null;
    if (!drag) return;
    if (drag.rect) this.actions.changed();
    const state = this.state();
    if (drag.mode === "pending" && !drag.moved && state) {
      if (drag.additive) {
        this.actions.select([]);
        return;
      }
      const edit = addNoteEdit(state.ctx, drag.pressBeat, drag.pressPitch, state.lengthName, state.velocity, mods.alt);
      if (!edit) return;
      this.added = this.actions.run(edit).then((outcome) => {
        const ids = outcome ? createdNotes(outcome) : [];
        if (ids.length > 0) this.actions.select(ids);
        return ids[0] ?? null;
      });
      await this.added;
      return;
    }
    if ((drag.mode === "move" || drag.mode === "resize") && drag.moved && drag.gesture) {
      await drag.gesture.end();
    }
  }

  /** `mouseDoubleClickEvent`: deletes the note under the pointer (the one the click added). */
  async doubleClick(x: number, y: number): Promise<void> {
    this.drag = null;
    const state = this.state();
    if (!state) return;
    const hit = hitNote(state.index, state.view, x, y);
    // The first click of a double-click on empty space added a note under the pointer, which
    // the prototype's double-click then deleted; the view may not show it yet.
    const id = hit ? hit.note.id : await (this.added ?? Promise.resolve(null));
    this.added = null;
    if (!id) return;
    await this.actions.run({ op: "delete_notes", clip_id: state.ctx.clip.id, note_ids: [id] });
  }

  // -- the context menu and the keys --------------------------------------------------------------

  /** `contextMenuEvent`: a note under the pointer is selected first. */
  contextMenu(x: number, y: number): MenuItem[] {
    const state = this.state();
    if (!state) return [];
    const hit = hitNote(state.index, state.view, x, y);
    let selected = state.selected;
    if (hit && !selected.has(hit.note.id)) {
      const next = noteSelection(selected, [hit.note.id], state.index);
      this.actions.select(next);
      selected = new Set(next);
    }
    const some = selected.size > 0;
    return [
      some ? { label: "Delete", run: () => void this.deleteSelected() } : { label: "Delete", disabled: true },
      some ? { label: "Duplicate", run: () => void this.duplicate() } : { label: "Duplicate", disabled: true },
      "separator",
      { label: "Quantize", run: () => void this.quantize() },
      { label: "Transpose +1", run: () => void this.transpose(1) },
      { label: "Transpose -1", run: () => void this.transpose(-1) },
      "separator",
      { label: "Select All", run: () => this.selectAll() },
    ];
  }

  /** `keyPressEvent`; whether the key was handled. */
  key(key: string, mods: Mods): boolean {
    const state = this.state();
    if (!state) return false;
    const lower = key.toLowerCase();
    if (key === "Delete" || key === "Backspace") void this.deleteSelected();
    else if (mods.command && lower === "d") void this.duplicate();
    else if (mods.command && lower === "a") this.selectAll();
    else if (key === "Escape") this.actions.select([]);
    else if ((key === "ArrowLeft" || key === "ArrowRight") && state.selected.size > 0) {
      const step = nudgeStep(state.ctx);
      void this.nudge(key === "ArrowLeft" ? -step : step, 0);
    } else if ((key === "ArrowUp" || key === "ArrowDown") && state.selected.size > 0) {
      const step = mods.shift ? 12 : 1;
      void this.nudge(0, key === "ArrowUp" ? step : -step);
    } else return false;
    return true;
  }

  // -- the editor's operations (also the toolbar's) -----------------------------------------------

  /** `delete_notes` of the selection; the deleted notes leave the selection. */
  async deleteSelected(): Promise<void> {
    const state = this.state();
    if (!state) return;
    const edit = deleteEdit(state.ctx.clip.id, state.index, state.selected);
    if (!edit || edit.op !== "delete_notes") return;
    if (await this.actions.run(edit)) {
      const doomed = new Set(edit.note_ids);
      this.actions.select([...(this.state()?.selected ?? [])].filter((id) => !doomed.has(id)));
    }
  }

  /** `duplicate_selection`: the copies become the selection. */
  async duplicate(): Promise<void> {
    const state = this.state();
    if (!state) return;
    const edit = duplicateEdit(state.ctx, selectedNotes(state.index, state.selected));
    if (!edit) return;
    const outcome = await this.actions.run(edit);
    if (outcome) this.actions.select(createdNotes(outcome));
  }

  /** `select_all`. */
  selectAll(): void {
    const state = this.state();
    if (state) this.actions.select([...state.index.ids]);
  }

  /** `nudge`: the selection moved by a delta, one command. */
  async nudge(dBeats: number, dPitch: number): Promise<void> {
    const state = this.state();
    if (!state) return;
    const moved = movedNotes(selectedNotes(state.index, state.selected), dBeats, dPitch);
    if (moved.length > 0) await this.actions.run(moveEdit(state.ctx.clip.id, moved));
  }

  /** `transpose`: the selection, or every note; refused past the MIDI range. */
  async transpose(semitones: number): Promise<void> {
    const state = this.state();
    if (!state) return;
    const edit = transposeEdit(state.ctx.clip.id, targets(state.index, state.selected), semitones);
    if (edit === "refused") this.actions.message(TRANSPOSE_REFUSED);
    else if (edit) await this.actions.run(edit);
  }

  /** `quantize` at the toolbar's strength. */
  async quantize(): Promise<void> {
    const state = this.state();
    if (!state) return;
    const edit = quantizeEdit(state.ctx, state.index, state.selected, state.strength);
    if (edit) await this.actions.run(edit);
  }

  /** PR-11: a key selects every note of its pitch. */
  selectPitch(y: number): void {
    const state = this.state();
    if (!state) return;
    const pitch = yPitch(state.view, y);
    if (pitch >= 0 && pitch <= 127) this.actions.select(notesOfPitch(state.index, pitch));
  }

  /** PR-12: a click on the ruler seeks to the snapped beat, in timeline beats. */
  seekAt(x: number): void {
    const state = this.state();
    if (!state) return;
    const content = snapContent(state.ctx, xBeat(state.view, x));
    this.actions.seek(contentToTimeline(state.ctx.clip, content));
  }
}

/** The notes an outcome created, in order. */
export function createdNotes(outcome: EditOutcome): string[] {
  return outcome.changes.created.filter((object) => object.kind === "note").map((object) => object.id);
}

/** The velocity lane's drag (`VelocityLane`): local preview, one edit on release. */
export class VelocityGestures {
  /** The velocities shown instead of the notes' while the lane is dragged. */
  readonly preview = new Map<string, number>();
  private relative: { y0: number; base: Map<string, number> } | null = null;
  private pressed = false;

  constructor(
    private readonly state: () => RollState | null,
    private readonly actions: RollActions,
  ) {}

  get active(): boolean {
    return this.pressed;
  }

  /** `mousePressEvent`: over a selected note of a selection of two or more, a relative drag. */
  press(x: number, y: number, height: number): void {
    const state = this.state();
    if (!state) return;
    this.pressed = true;
    this.preview.clear();
    const id = noteAtX(state.index, state.view, x, state.selected);
    if (id !== null && state.selected.has(id) && state.selected.size > 1) {
      const base = new Map(selectedNotes(state.index, state.selected).map((note) => [note.id, note.velocity]));
      this.relative = { y0: y, base };
      this.applyRelative(y, height);
    } else {
      this.relative = null;
      if (id !== null) this.preview.set(id, velocityAt(y, height));
    }
    this.actions.changed();
  }

  private applyRelative(y: number, height: number): void {
    if (!this.relative) return;
    this.preview.clear();
    for (const [id, v] of relativeVelocities(this.relative.base, this.relative.y0, y, height)) this.preview.set(id, v);
  }

  /** `mouseMoveEvent` with the button down. */
  move(x: number, y: number, height: number): void {
    const state = this.state();
    if (!state || !this.pressed) return;
    if (this.relative) this.applyRelative(y, height);
    else {
      const id = noteAtX(state.index, state.view, x, state.selected);
      if (id !== null) this.preview.set(id, velocityAt(y, height));
    }
    this.actions.changed();
  }

  /**
   * `mouseReleaseEvent`: the changed velocities in one edit, then the seal. Returns them with the
   * outcome's version, for the caller to hold until the view has it.
   */
  async release(): Promise<{ values: Map<string, number>; version: number | null } | null> {
    const state = this.state();
    this.pressed = false;
    this.relative = null;
    const values = new Map<string, number>();
    if (state) {
      for (const [id, v] of this.preview) {
        const note = state.index.note(id);
        if (note && note.velocity !== v) values.set(id, v);
      }
    }
    this.preview.clear();
    this.actions.changed();
    const edit = state ? velocityEdit(state.ctx.clip.id, values) : null;
    if (!edit) return null;
    return { values, version: await this.actions.commit(edit) };
  }
}

/**
 * The wheel's meaning on the grid, from a browser wheel event (pixel deltas, positive down
 * and right; Qt's are the opposite): ⌘ (or a pinch) zooms time around the pointer by 1.0015 per
 * pixel, ⌥ zooms pitch, ⇧ scrolls sideways, and otherwise the deltas scroll.
 */
export type RollWheel =
  | { readonly kind: "time"; readonly factor: number }
  | { readonly kind: "pitch"; readonly factor: number }
  | { readonly kind: "scroll"; readonly dx: number; readonly dy: number };

export interface WheelInput {
  deltaX: number;
  deltaY: number;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

/** `NoteGrid.wheelEvent`. */
export function gridWheel(event: WheelInput): RollWheel {
  const delta = -(event.deltaY || event.deltaX);
  if (event.metaKey || event.ctrlKey) return { kind: "time", factor: 1.0015 ** delta };
  if (event.altKey) return { kind: "pitch", factor: 1.0015 ** delta };
  if (event.shiftKey) return { kind: "scroll", dx: event.deltaY || event.deltaX, dy: 0 };
  return { kind: "scroll", dx: event.deltaX, dy: event.deltaY };
}

/** `Keyboard.wheelEvent`: ⌥ or ⌘ zooms pitch, otherwise it scrolls. */
export function keyboardWheel(event: WheelInput): RollWheel {
  if (event.altKey || event.metaKey || event.ctrlKey) return { kind: "pitch", factor: 1.0015 ** -event.deltaY };
  return { kind: "scroll", dx: 0, dy: event.deltaY };
}

/** `Ruler.wheelEvent`: zooms time. */
export function rulerWheel(event: WheelInput): RollWheel {
  return { kind: "time", factor: 1.0015 ** -(event.deltaY || event.deltaX) };
}

/** The gesture deps of the piano roll: edits wait for the previous gesture's seal. */
export function chained(deps: GestureDeps, after: () => Promise<void>): GestureDeps {
  return { ...deps, send: (edit) => after().then(() => deps.send(edit)) };
}
