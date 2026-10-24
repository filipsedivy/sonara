import { expect, test } from "@playwright/test";
import { LiveGesture } from "../../src/lib/gesture";
import {
  createdNotes,
  GridGestures,
  gridWheel,
  hitNote,
  keyboardWheel,
  NO_MODS,
  noteSelection,
  patched,
  type RollActions,
  type RollState,
  rulerWheel,
  VelocityGestures,
} from "../../src/lib/pianoroll/gestures";
import {
  beatX,
  centerOnContent,
  clampView,
  contentExtent,
  duplicateEdit,
  movedNotes,
  NoteIndex,
  type NoteLengthName,
  noteAtX,
  noteName,
  pitchY,
  playableRange,
  isPlayable,
  type RollView,
  scrollBy,
  snapContent,
  timelineToContent,
  TRANSPOSE_REFUSED,
  velocityAt,
  zoomPitch,
  zoomTime,
} from "../../src/lib/pianoroll/model";
import { keyLabel, keyboardWidth, velocityColor } from "../../src/lib/pianoroll/paint";
import { DEFAULT_SNAP, type GridName, type Snap } from "../../src/lib/state/snap";
import type { Edit, EditOutcome, MidiClipView, NoteView } from "../../src/lib/types";

// The piano roll's logic against the prototype's tests (tests/gui/test_piano_roll.py): the same
// clip ("Chords" at beat 4, three notes), the same view (100 px per beat, 14 px rows, pitch 72 at
// the top), the same pointer paths. The edits go to an in-memory clip that applies them as
// sonara-app's commands do, so a gesture's next step sees the previous one's result. The page
// tests (tests/page/pianoroll.spec.ts) drive the same through the canvas's events.

const PPB = 100;
const ROW = 14;
const VIEW: RollView = { ppb: PPB, rowH: ROW, x0: 0, y0: (127 - 72) * ROW };
const SIGNATURES = [{ bar: 0, beat: 0, numerator: 4, denominator: 4, beats_per_bar: 4 }];

function note(id: string, pitch: number, start: number, length: number, velocity: number): NoteView {
  return { id, pitch, start, length, velocity, channel: 0 };
}

function chords(extra: Partial<MidiClipView> = {}): MidiClipView {
  return {
    type: "midi",
    id: "clp",
    name: "Chords",
    start: 4,
    length: 8,
    muted: false,
    color: null,
    content_offset: 0,
    loop_length: null,
    notes: [note("n0", 60, 0, 1, 100), note("n1", 64, 1, 1, 90), note("n2", 67, 2, 0.5, 80)],
    ...extra,
  };
}

/** A piano roll over an in-memory clip: what the panel and sonara-app do for the gestures. */
class Roll {
  clip: MidiClipView;
  index: NoteIndex;
  view: RollView = VIEW;
  selected = new Set<string>();
  snap: Snap = DEFAULT_SNAP;
  lengthName: NoteLengthName = "Grid";
  velocity = 100;
  strength = 1;
  readonly edits: Edit[] = [];
  readonly calls: string[] = [];
  readonly messages: string[] = [];
  readonly seeks: number[] = [];
  private next = 0;
  readonly grid: GridGestures;
  readonly lane: VelocityGestures;

  constructor(clip: MidiClipView = chords()) {
    this.clip = clip;
    this.index = new NoteIndex(clip.notes);
    const state = (): RollState => ({
      ctx: { clip: this.clip, signatures: SIGNATURES, snap: this.snap },
      index: this.index,
      view: this.view,
      selected: this.selected,
      lengthName: this.lengthName,
      velocity: this.velocity,
      strength: this.strength,
    });
    const actions: RollActions = {
      select: (ids) => (this.selected = new Set(ids)),
      run: async (edit) => this.apply(edit),
      live: () =>
        new LiveGesture({
          send: async (edit) => this.apply(edit),
          seal: async () => void this.calls.push("seal"),
          nextFrame: (callback) => callback(),
          onError: () => {},
        }),
      commit: async (edit) => {
        this.apply(edit);
        this.calls.push("seal");
        return 1;
      },
      message: (text) => this.messages.push(text),
      changed: () => {},
      seek: (beat) => this.seeks.push(beat),
      setView: (view) => (this.view = view),
    };
    this.grid = new GridGestures(state, actions);
    this.lane = new VelocityGestures(state, actions);
  }

  /** sonara-app's note commands on the clip. */
  apply(edit: Edit): EditOutcome {
    this.edits.push(edit);
    this.calls.push(edit.op);
    let notes = [...this.clip.notes];
    const created: { kind: string; id: string }[] = [];
    if (edit.op === "add_notes") {
      for (const n of edit.notes) {
        const id = `new${this.next++}`;
        created.push({ kind: "note", id });
        notes.push({ id, channel: n.channel ?? 0, pitch: n.pitch, start: n.start, length: n.length, velocity: n.velocity });
      }
    } else if (edit.op === "edit_notes") {
      notes = notes.map((n) => {
        const change = edit.edits.find((e) => e.note_id === n.id);
        if (!change) return n;
        const { note_id: _, ...values } = change;
        return { ...n, ...Object.fromEntries(Object.entries(values).filter(([, v]) => v !== undefined)) };
      });
    } else if (edit.op === "delete_notes") {
      notes = notes.filter((n) => !edit.note_ids.includes(n.id));
    }
    this.clip = { ...this.clip, notes };
    this.index = new NoteIndex(notes);
    return {
      committed: true,
      version: this.edits.length,
      label: "Edit",
      changes: { created, updated: [], deleted: [], flags: ["notes"], structure_changed: false },
      result: {},
    };
  }

  setGrid(gridName: GridName) {
    this.snap = { ...this.snap, gridName };
  }

  /** `Env.point`: the pixel at the centre of `pitch`'s row at content `beat`. */
  point(beat: number, pitch: number): [number, number] {
    return [Math.round(beatX(this.view, beat)), Math.round(pitchY(this.view, pitch) + this.view.rowH / 2)];
  }

  note(id: string): NoteView | undefined {
    return this.clip.notes.find((n) => n.id === id);
  }

  async click(x: number, y: number, mods = NO_MODS) {
    this.grid.press(x, y, mods);
    await this.grid.release(mods);
  }

  /** `drag`: press, six moves, release. */
  async drag(from: [number, number], to: [number, number], mods = NO_MODS) {
    this.grid.press(from[0], from[1], mods);
    for (let i = 1; i <= 6; i++) {
      this.grid.move(from[0] + ((to[0] - from[0]) * i) / 6, from[1] + ((to[1] - from[1]) * i) / 6, mods);
    }
    await this.grid.release(mods);
  }

  async laneDrag(x: number, y0: number, y1: number, height = 72) {
    this.lane.press(x, y0, height);
    for (let i = 1; i <= 6; i++) this.lane.move(x, y0 + ((y1 - y0) * i) / 6, height);
    return this.lane.release();
  }
}

test("a click on empty space adds a floor-snapped note with the toolbar's length and velocity", async () => {
  const roll = new Roll();
  roll.setGrid("1/16");
  roll.lengthName = "1/8";
  roll.velocity = 77;
  await roll.click(...roll.point(3.13, 65));
  const added = roll.clip.notes.filter((n) => n.pitch === 65);
  expect(added).toHaveLength(1);
  expect(added[0].start).toBeCloseTo(3.0, 9);
  expect(added[0].length).toBeCloseTo(0.5, 9);
  expect(added[0].velocity).toBe(77);
  expect([...roll.selected]).toEqual([added[0].id]);
  // ⇧-click on empty space clears the selection instead.
  await roll.click(...roll.point(5.5, 50), { ...NO_MODS, shift: true });
  expect(roll.selected.size).toBe(0);
  expect(roll.clip.notes).toHaveLength(4);
});

test("a drag moves a note in time (snapped) and pitch as one undo step", async () => {
  const roll = new Roll();
  await roll.drag(roll.point(0.5, 60), roll.point(1.52, 62));
  const moved = roll.note("n0");
  expect(moved?.start).toBeCloseTo(1.0, 9);
  expect(moved?.pitch).toBe(62);
  // Every move is an edit of the same notes (one merge key), and the release seals once.
  expect(roll.calls.filter((c) => c === "seal")).toHaveLength(1);
  expect(roll.calls.at(-1)).toBe("seal");
  expect(new Set(roll.edits.map((e) => e.op))).toEqual(new Set(["edit_notes"]));
});

test("⌥ bypasses the snap of a drag", async () => {
  const roll = new Roll();
  await roll.drag(roll.point(0.5, 60), roll.point(0.83, 60), { ...NO_MODS, alt: true });
  expect(roll.note("n0")?.start).toBeCloseTo(0.33, 1);
  expect(Math.abs((roll.note("n0")?.start ?? 0) - 0.33)).toBeLessThan(0.02);
});

test("a drag of the right edge resizes", async () => {
  const roll = new Roll();
  const [x, y] = roll.point(1.0, 60);
  await roll.drag([x - 2, y], [x - 2 + 50, y]);
  expect(roll.note("n0")?.start).toBe(0);
  expect(roll.note("n0")?.length).toBeCloseTo(1.5, 9);
});

test("a resize to before the start leaves one grid step; ⌥ leaves 1/64", async () => {
  const roll = new Roll();
  const [x, y] = roll.point(1.0, 60);
  await roll.drag([x - 2, y], [x - 2 - 150, y]);
  expect(roll.note("n0")?.length).toBeCloseTo(0.25, 9);
  const other = new Roll();
  await other.drag([x - 2, y], [x - 2 - 150, y], { ...NO_MODS, alt: true });
  expect(other.note("n0")?.length).toBeCloseTo(1 / 64, 9);
});

test("a rubber band selects, and Delete deletes the selection", async () => {
  const roll = new Roll();
  await roll.drag(roll.point(-0.05, 69), roll.point(1.9, 59));
  expect(roll.selected).toEqual(new Set(["n0", "n1"]));
  expect(roll.grid.key("Delete", NO_MODS)).toBe(true);
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(roll.clip.notes.map((n) => n.pitch)).toEqual([67]);
  expect(roll.selected.size).toBe(0);
});

test("⇧-click toggles a note in the selection", async () => {
  const roll = new Roll();
  await roll.click(...roll.point(0.5, 60));
  await roll.click(...roll.point(1.5, 64), { ...NO_MODS, shift: true });
  expect(roll.selected).toEqual(new Set(["n0", "n1"]));
  await roll.click(...roll.point(0.5, 60), { ...NO_MODS, shift: true });
  expect(roll.selected).toEqual(new Set(["n1"]));
  // A click that only selects sends nothing.
  expect(roll.edits).toHaveLength(0);
});

test("a double-click deletes the note under the pointer", async () => {
  const roll = new Roll();
  await roll.grid.doubleClick(...roll.point(2.2, 67));
  expect(roll.clip.notes.map((n) => n.pitch)).not.toContain(67);
  // On empty space the first click's note goes again, as in the prototype.
  const empty = new Roll();
  const point = empty.point(6.1, 50);
  await empty.click(...point);
  await empty.grid.doubleClick(...point);
  expect(empty.edits.map((e) => e.op)).toEqual(["add_notes", "delete_notes"]);
  expect(empty.clip.notes).toHaveLength(3);
});

test("the keys nudge, duplicate after the span and select all", async () => {
  const roll = new Roll();
  roll.selected = new Set(["n0"]);
  const key = async (name: string, mods = NO_MODS) => {
    expect(roll.grid.key(name, mods)).toBe(true);
    await new Promise((resolve) => setTimeout(resolve, 0));
  };
  await key("ArrowUp");
  await key("ArrowRight");
  expect(roll.note("n0")?.pitch).toBe(61);
  expect(roll.note("n0")?.start).toBeCloseTo(0.25, 9);
  await key("ArrowUp", { ...NO_MODS, shift: true });
  expect(roll.note("n0")?.pitch).toBe(73);
  const count = roll.clip.notes.length;
  await key("d", { ...NO_MODS, command: true });
  expect(roll.clip.notes).toHaveLength(count + 1);
  const copies = roll.clip.notes.filter((n) => roll.selected.has(n.id));
  expect(copies).toHaveLength(1);
  expect(copies[0].pitch).toBe(73);
  expect(copies[0].start).toBeCloseTo(1.25, 9);
  await key("a", { ...NO_MODS, command: true });
  expect(roll.selected).toEqual(new Set(roll.clip.notes.map((n) => n.id)));
  await key("Escape");
  expect(roll.selected.size).toBe(0);
  // Without a selection the arrows are not the grid's.
  expect(roll.grid.key("ArrowLeft", NO_MODS)).toBe(false);
  // Each key is one edit of its own, never sealed (the prototype's `bridge.execute`).
  expect(roll.calls).not.toContain("seal");
});

test("nudging by ← uses the grid, or 1/16 with snapping off", async () => {
  const roll = new Roll();
  roll.setGrid("1/4");
  roll.selected = new Set(["n1"]);
  roll.grid.key("ArrowLeft", NO_MODS);
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(roll.note("n1")?.start).toBeCloseTo(0, 9);
  roll.snap = { ...roll.snap, enabled: false };
  roll.grid.key("ArrowRight", NO_MODS);
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(roll.note("n1")?.start).toBeCloseTo(1 / 16, 9);
});

test("the velocity lane paints a velocity in one step", async () => {
  const roll = new Roll();
  const x = Math.round(beatX(roll.view, 1.0)) + 1;
  const result = await roll.laneDrag(x, 72 - 10, 2);
  expect(roll.note("n1")?.velocity).toBe(127);
  expect(roll.edits).toHaveLength(1);
  expect(roll.calls).toEqual(["edit_notes", "seal"]);
  expect(result?.values.get("n1")).toBe(127);
});

test("the velocity lane shifts a selection of two or more by the same offset", async () => {
  const roll = new Roll();
  const before = new Map(roll.clip.notes.map((n) => [n.id, n.velocity]));
  roll.selected = new Set(["n0", "n1", "n2"]);
  const x = Math.round(beatX(roll.view, 0.0)) + 1;
  await roll.laneDrag(x, 36, 56);
  const deltas = new Set(roll.clip.notes.map((n) => n.velocity - (before.get(n.id) ?? 0)));
  expect(deltas.size).toBe(1);
  expect([...deltas][0]).toBeLessThan(0);
  // Upwards the shift stops where the loudest note reaches 127.
  const up = new Roll();
  up.selected = new Set(["n0", "n1", "n2"]);
  await up.laneDrag(x, 36, -200);
  expect(up.clip.notes.map((n) => n.velocity)).toEqual([127, 117, 107]);
});

test("velocity_at and the lane's note search follow the prototype", () => {
  expect(velocityAt(2, 72)).toBe(127);
  expect(velocityAt(69, 72)).toBe(1);
  // 63.5 rounds to even, as Python's `round`.
  expect(velocityAt(36, 72)).toBe(64);
  const twins = new NoteIndex([note("a", 60, 1, 1, 100), note("b", 62, 1, 1, 100), note("c", 64, 3, 1, 100)]);
  expect(noteAtX(twins, VIEW, 101, new Set())).toBe("a");
  // A selected note wins a tie; a start more than 4 px away is no hit.
  expect(noteAtX(twins, VIEW, 101, new Set(["b"]))).toBe("b");
  expect(noteAtX(twins, VIEW, 105, new Set())).toBeNull();
  expect(noteAtX(twins, VIEW, 302, new Set())).toBe("c");
});

test("quantize sends the selection (or every note), the grid at content beat 0 and the strength", async () => {
  const roll = new Roll();
  roll.setGrid("1/4");
  roll.strength = 0.5;
  await roll.grid.quantize();
  expect(roll.edits.at(-1)).toEqual({ op: "quantize_notes", clip_id: "clp", note_ids: [], grid: 1, strength: 0.5 });
  roll.selected = new Set(["n1"]);
  roll.setGrid("Bar");
  await roll.grid.quantize();
  expect(roll.edits.at(-1)).toEqual({ op: "quantize_notes", clip_id: "clp", note_ids: ["n1"], grid: 4, strength: 0.5 });
  // A clip without notes sends nothing.
  const empty = new Roll(chords({ notes: [] }));
  await empty.grid.quantize();
  expect(empty.edits).toHaveLength(0);
});

test("transpose moves the selection or every note, and refuses to leave the MIDI range", async () => {
  const roll = new Roll();
  const pitches = roll.clip.notes.map((n) => n.pitch);
  await roll.grid.transpose(1);
  expect(roll.clip.notes.map((n) => n.pitch)).toEqual(pitches.map((p) => p + 1));
  roll.selected = new Set(["n0"]);
  await roll.grid.transpose(-12);
  expect(roll.note("n0")?.pitch).toBe(49);
  roll.selected = new Set();
  for (let i = 0; i < 6; i++) await roll.grid.transpose(12);
  expect(Math.max(...roll.clip.notes.map((n) => n.pitch))).toBeLessThanOrEqual(127);
  expect(roll.messages).toContain(TRANSPOSE_REFUSED);
});

test("a key selects every note of its pitch; the ruler seeks to the snapped beat on the timeline", () => {
  const roll = new Roll(chords({ notes: [...chords().notes, note("n3", 60, 4, 1, 100)] }));
  roll.grid.selectPitch(pitchY(roll.view, 60) + 3);
  expect(roll.selected).toEqual(new Set(["n0", "n3"]));
  roll.grid.seekAt(beatX(roll.view, 1.13));
  expect(roll.seeks).toEqual([5.25]);
});

test("the context menu selects the note under the pointer and offers the prototype's items", () => {
  const roll = new Roll();
  const items = roll.grid.contextMenu(...roll.point(1.5, 64));
  expect(roll.selected).toEqual(new Set(["n1"]));
  expect(items.map((item) => (item === "separator" ? "-" : item.label))).toEqual([
    "Delete",
    "Duplicate",
    "-",
    "Quantize",
    "Transpose +1",
    "Transpose -1",
    "-",
    "Select All",
  ]);
  roll.selected = new Set();
  const none = roll.grid.contextMenu(...roll.point(6, 40));
  expect(none.slice(0, 2).every((item) => item !== "separator" && "disabled" in item)).toBe(true);
});

test("hit testing prefers the right edge, which tiny notes have only at their very end", () => {
  const index = new NoteIndex([note("long", 60, 0, 1, 100), note("tiny", 62, 2, 0.08, 100)]);
  const view = VIEW;
  const y60 = pitchY(view, 60) + 7;
  expect(hitNote(index, view, 98, y60)).toMatchObject({ note: { id: "long" }, edge: true });
  expect(hitNote(index, view, 102, y60)).toMatchObject({ note: { id: "long" }, edge: true });
  expect(hitNote(index, view, 50, y60)).toMatchObject({ note: { id: "long" }, edge: false });
  expect(hitNote(index, view, 120, y60)).toBeNull();
  const y62 = pitchY(view, 62) + 7;
  // 8 px wide: its body moves, only the last 1.5 px (or just past) resizes.
  expect(hitNote(index, view, 203, y62)).toMatchObject({ note: { id: "tiny" }, edge: false });
  expect(hitNote(index, view, 207, y62)).toMatchObject({ note: { id: "tiny" }, edge: true });
});

test("the playable range, the loop and the mapping to the timeline", () => {
  const clip = chords();
  expect(timelineToContent(clip, 5.5)).toBeCloseTo(1.5, 9);
  expect(timelineToContent(clip, 3.0)).toBeNull();
  const looped = chords({ loop_length: 2 });
  expect(playableRange(looped)).toEqual([0, 2]);
  expect(isPlayable(looped, 1)).toBe(true);
  expect(isPlayable(looped, 2)).toBe(false);
  expect(timelineToContent(looped, 6.5)).toBeCloseTo(0.5, 9);
  const offset = chords({ content_offset: 1 });
  expect(playableRange(offset)).toEqual([1, 9]);
  // A clip that does not start on the grid snaps on the project grid.
  const ctx = { clip: chords({ start: 4.1 }), signatures: SIGNATURES, snap: { enabled: true, gridName: "1/4" as GridName } };
  expect(snapContent(ctx, 0.5)).toBeCloseTo(0.9, 9);
  expect(snapContent(ctx, 0.5, true)).toBeCloseTo(0.5, 9);
});

test("zoom and scroll stay clamped, and Fit shows every note", () => {
  const clip = chords();
  const index = new NoteIndex(clip.notes);
  const viewport = { width: 1020, height: 460 };
  const extent = contentExtent(clip, index);
  let view = VIEW;
  for (let i = 0; i < 50; i++) view = zoomTime(view, 2, viewport, extent);
  expect(view.ppb).toBeLessThanOrEqual(1200);
  for (let i = 0; i < 50; i++) view = zoomPitch(view, 0.5, viewport, extent);
  expect(view.rowH).toBeGreaterThanOrEqual(5);
  view = scrollBy(view, 0, -1e9, viewport, extent);
  expect(view.y0).toBe(0);
  view = centerOnContent(view, clip, index, false, viewport);
  for (const n of clip.notes) {
    expect(pitchY(view, n.pitch)).toBeGreaterThanOrEqual(0);
    expect(pitchY(view, n.pitch)).toBeLessThanOrEqual(viewport.height);
    expect(beatX(view, n.start)).toBeGreaterThanOrEqual(0);
  }
  expect(view.rowH).toBeGreaterThanOrEqual(8);
  expect(view.rowH).toBeLessThanOrEqual(16);
  // The scroll stops a quarter of the view before the content's end.
  const far = clampView({ ...VIEW, x0: 1e6 }, viewport, extent);
  expect(far.x0).toBeCloseTo(extent - (viewport.width / PPB) * 0.25, 9);
});

test("moves clamp to the MIDI range and beat 0; duplicates round up to the grid", () => {
  const notes = chords().notes;
  const moved = movedNotes(notes, -5, 100);
  expect(moved.map((n) => n.start)).toEqual([0, 1, 2]);
  expect(Math.max(...moved.map((n) => n.pitch))).toBe(127);
  const ctx = { clip: chords(), signatures: SIGNATURES, snap: DEFAULT_SNAP };
  const edit = duplicateEdit(ctx, [note("x", 60, 0, 0.3, 100)]);
  expect(edit && edit.op === "add_notes" ? edit.notes[0].start : null).toBeCloseTo(0.5, 9);
  const off = duplicateEdit({ ...ctx, snap: { ...DEFAULT_SNAP, enabled: false } }, [note("x", 60, 0, 0.3, 100)]);
  expect(off && off.op === "add_notes" ? off.notes[0].start : null).toBeCloseTo(0.3, 9);
});

test("the selection modes of set_selection keep only the clip's notes", () => {
  const index = new NoteIndex(chords().notes);
  expect(noteSelection(new Set(["n0"]), ["n1", "gone"], index)).toEqual(["n1"]);
  expect(noteSelection(new Set(["n0"]), ["n1"], index, "additive")).toEqual(["n0", "n1"]);
  expect(noteSelection(new Set(["n0", "n1"]), ["n1"], index, "toggle")).toEqual(["n0"]);
  const preview = new Map([["n1", { id: "n1", pitch: 70, start: 3, length: 1, velocity: 90 }]]);
  expect(patched(index, preview).find((n) => n.id === "n1")).toMatchObject({ pitch: 70, start: 3 });
});

test("names, the keyboard's labels and widths, and the velocity colours follow the prototype", () => {
  expect(noteName(60)).toBe("C4");
  expect(noteName(61)).toBe("C#4");
  expect(noteName(0)).toBe("C-1");
  const labels = new Map([
    [36, "Kick"],
    [42, "Hat Closed"],
  ]);
  expect(keyLabel(36, true, labels)).toBe("36 Kick");
  expect(keyLabel(42, true, labels)).toContain("Hat Closed");
  expect(keyLabel(37, true, labels)).toBe("");
  expect(keyLabel(60, false, labels)).toBe("C4");
  expect(keyLabel(62, false, labels)).toBe("");
  expect(keyboardWidth(true)).toBeGreaterThan(100);
  expect(keyboardWidth(false)).toBe(64);
  // The prototype's `velocity_color(v, selected).getRgb()` (Qt rounds through 16 bits).
  const oracle: [number, boolean, [number, number, number]][] = [
    [1, false, [87, 125, 159]],
    [1, true, [117, 168, 214]],
    [40, false, [81, 185, 133]],
    [40, true, [110, 249, 180]],
    [64, false, [87, 201, 75]],
    [64, true, [125, 255, 111]],
    [100, false, [223, 224, 62]],
    [100, true, [254, 255, 118]],
    [127, false, [242, 113, 48]],
    [127, true, [255, 167, 123]],
  ];
  for (const [velocity, selected, rgb] of oracle) {
    const c = velocityColor(velocity, selected);
    for (let i = 0; i < 3; i++) expect(Math.abs(c[i] - rgb[i])).toBeLessThanOrEqual(1);
  }
});

test("the wheels: ⌘ zooms time, ⌥ zooms pitch, ⇧ scrolls sideways, the keyboard and the ruler zoom", () => {
  const wheel = (deltaX: number, deltaY: number, mods: Partial<Record<"metaKey" | "ctrlKey" | "altKey" | "shiftKey", boolean>> = {}) => ({
    deltaX,
    deltaY,
    metaKey: false,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    ...mods,
  });
  expect(gridWheel(wheel(0, -100, { metaKey: true }))).toEqual({ kind: "time", factor: 1.0015 ** 100 });
  expect(gridWheel(wheel(0, 50, { altKey: true }))).toEqual({ kind: "pitch", factor: 1.0015 ** -50 });
  expect(gridWheel(wheel(0, 30, { shiftKey: true }))).toEqual({ kind: "scroll", dx: 30, dy: 0 });
  expect(gridWheel(wheel(5, 30))).toEqual({ kind: "scroll", dx: 5, dy: 30 });
  expect(keyboardWheel(wheel(0, 30))).toEqual({ kind: "scroll", dx: 0, dy: 30 });
  expect(keyboardWheel(wheel(0, -30, { metaKey: true }))).toEqual({ kind: "pitch", factor: 1.0015 ** 30 });
  expect(rulerWheel(wheel(0, -30))).toEqual({ kind: "time", factor: 1.0015 ** 30 });
});

test("an outcome's created notes are its new selection", () => {
  const outcome: EditOutcome = {
    committed: true,
    version: 3,
    label: "Add Notes",
    changes: { created: [{ kind: "note", id: "n9" }], updated: [{ kind: "clip", id: "clp" }], deleted: [], flags: ["notes"], structure_changed: false },
    result: {},
  };
  expect(createdNotes(outcome)).toEqual(["n9"]);
});
