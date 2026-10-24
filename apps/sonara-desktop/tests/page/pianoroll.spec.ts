import { expect, type Page, test } from "@playwright/test";
import { centerOnContent, DEFAULT_ROLL_VIEW, NoteIndex, beatX, pitchY, type RollView } from "../../src/lib/pianoroll/model";
import { velocityColor } from "../../src/lib/pianoroll/paint";
import type { DeviceView, MidiClipView, NoteView, OpenProject, TrackView } from "../../src/lib/types";
import { audioClip, project, track, view } from "../support/project";
import { AUDIO_SUFFIXES, DATA_LAYER, answer, calls, emit, mockTauri } from "./tauri-mock";
import { trackList } from "../support/tracks";

// The piano roll against mocked commands, driven through its canvases'
// pointer, wheel and key events in WebKit: the prototype's tests/gui/test_piano_roll.py, whose
// edits the page sends to sonara-app (P). The clip is the prototype's ("Chords" at beat 4 with
// three notes), opened by a double-click in the timeline and fitted; the points are computed with
// the page's own `centerOnContent` for the grid's size.

const OUTCOME = {
  committed: true,
  version: 2,
  label: "Edit",
  changes: { created: [], updated: [], deleted: [], flags: ["notes"], structure_changed: false },
  result: {},
};

const DRUM_MAP = {
  plugin_id: "sonara.drums",
  names: [
    { pitch: 36, label: "Kick" },
    { pitch: 38, label: "Snare" },
    { pitch: 42, label: "Hat Closed" },
  ],
};

function device(id: string, pluginId: string, name: string): DeviceView {
  return { id, plugin_id: pluginId, name, enabled: true, preset: null, sidechain_id: null, available: true, params: {} };
}

function note(id: string, pitch: number, start: number, length: number, velocity: number): NoteView {
  return { id, pitch, start, length, velocity, channel: 0 };
}

const NOTES = [note("n0", 60, 0, 1, 100), note("n1", 64, 1, 1, 90), note("n2", 67, 2, 0.5, 80)];

function chords(notes: NoteView[] = NOTES, extra: Partial<MidiClipView> = {}): MidiClipView {
  return { type: "midi", id: "clp", name: "Chords", start: 4, length: 8, muted: false, color: null, content_offset: 0, loop_length: null, notes, ...extra };
}

function keys(clip: MidiClipView = chords(), plugin = "sonara.synth.poly"): TrackView {
  return { ...track("k", "Keys", "instrument", [clip]), instrument: device("dev_i", plugin, "Instrument") };
}

/** The grid as the page fits the clip: its box on the page and the view it shows. */
interface Grid {
  x: number;
  y: number;
  view: RollView;
}

/** Opens the window on `open` with the timeline and the Editors, then the clip "clp" by a double-click. */
async function openRoll(page: Page, open: OpenProject, extra: Record<string, unknown> = {}): Promise<Grid> {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: {
      agent: { on: false, address: null },
      messages: [],
      panes: {
        browser: { size: 240, open: false },
        inspector: { size: 280, open: false },
        editors: { size: 300, open: true },
      },
      audio_suffixes: AUDIO_SUFFIXES,
    },
    recoverable_sessions: [],
    project_view: view(1, open),
    edit: OUTCOME,
    seal_undo: null,
    drum_map: DRUM_MAP,
    ...extra,
  });
  await page.goto("/");
  await expect(trackList(page).getByRole("option")).toHaveCount(open.tracks.length);
  const arrangement = await page.getByRole("application", { name: "Arrangement" }).boundingBox();
  if (!arrangement) throw new Error("no arrangement");
  await page.mouse.dblclick(arrangement.x + 5 * 24, arrangement.y + 28);
  await expect(page.getByRole("tab", { name: "Piano Roll" })).toHaveAttribute("aria-selected", "true");
  return fitted(page, open);
}

/** Fit, and the view it gives on the grid's size. */
async function fitted(page: Page, open: OpenProject): Promise<Grid> {
  const roll = page.getByRole("region", { name: "Piano Roll" });
  await roll.getByRole("button", { name: "Fit" }).click();
  const grid = page.getByRole("application", { name: "Note grid" });
  const [width, height] = await grid.evaluate((el) => [el.clientWidth, el.clientHeight]);
  const box = await grid.boundingBox();
  if (!box) throw new Error("no grid");
  const track = open.tracks.find((t) => t.clips.some((c) => c.id === "clp"));
  const clip = track?.clips.find((c) => c.id === "clp");
  if (!clip || clip.type !== "midi") throw new Error("no clip");
  const drums = track?.instrument?.plugin_id === "sonara.drums";
  return { x: box.x, y: box.y, view: centerOnContent(DEFAULT_ROLL_VIEW, clip, new NoteIndex(clip.notes), drums, { width, height }) };
}

/** `Env.point`: the page pixel at the centre of `pitch`'s row at content `beat`. */
function point(grid: Grid, beat: number, pitch: number): [number, number] {
  return [grid.x + Math.round(beatX(grid.view, beat)), grid.y + Math.round(pitchY(grid.view, pitch) + grid.view.rowH / 2)];
}

async function drag(page: Page, from: [number, number], to: [number, number]) {
  await page.mouse.move(...from);
  await page.mouse.down();
  for (let i = 1; i <= 6; i++) {
    await page.mouse.move(from[0] + ((to[0] - from[0]) * i) / 6, from[1] + ((to[1] - from[1]) * i) / 6);
  }
  await page.mouse.up();
}

async function edits(page: Page): Promise<unknown[]> {
  return (await calls(page)).filter((c) => c.cmd === "edit").map((c) => (c.args as { edit: unknown }).edit);
}

async function commands(page: Page): Promise<string[]> {
  return (await calls(page)).map((c) => c.cmd).filter((cmd) => ["edit", "seal_undo"].includes(cmd));
}

/** The lane's box on the page. */
async function lane(page: Page) {
  const box = await page.locator('canvas[aria-label="Velocity lane"]').boundingBox();
  if (!box) throw new Error("no lane");
  return box;
}

test("with no MIDI clip the piano roll says how to open one; an audio clip keeps it", async ({ page }) => {
  const vox = { ...track("v", "Vox", "audio", [audioClip("take", "a1", 0, 4)]) };
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: { agent: { on: false, address: null }, messages: [], panes: { browser: { size: 240, open: false }, inspector: { size: 280, open: false }, editors: { size: 300, open: true } }, audio_suffixes: AUDIO_SUFFIXES },
    recoverable_sessions: [],
    project_view: view(1, project([vox])),
    peaks: { state: "failed", sample_rate: 48000, frames: 0, missing: true, level_bins: [], bin_frames: 256, tile_bins: 4096 },
    drum_map: DRUM_MAP,
  });
  await page.goto("/");
  await page.getByRole("tab", { name: "Piano Roll" }).click();
  await expect(page.getByTestId("piano-roll-empty")).toHaveText(/No MIDI clip selected\.\s+Double-click a MIDI clip in the timeline to edit its notes here\./);
  await expect(page.getByTestId("piano-roll-title")).toHaveText("Piano Roll");
  const arrangement = await page.getByRole("application", { name: "Arrangement" }).boundingBox();
  if (!arrangement) throw new Error("no arrangement");
  await page.mouse.click(arrangement.x + 2 * 24, arrangement.y + 28);
  await expect(page.getByTestId("piano-roll-empty")).toBeVisible();
  await expect(page.getByRole("button", { name: "Quantize" })).toBeDisabled();
});

/** The grid canvas's colour at a page pixel. */
async function pixel(page: Page, [x, y]: [number, number]): Promise<number[] | null> {
  return page.getByTestId("note-grid-canvas").evaluate(
    (canvas, [px, py]) => {
      const c = canvas as HTMLCanvasElement;
      const box = c.getBoundingClientRect();
      const scale = c.width / box.width;
      const data = c.getContext("2d")?.getImageData(Math.round((px - box.left) * scale), Math.round((py - box.top) * scale), 1, 1).data;
      return data ? [data[0], data[1], data[2]] : null;
    },
    [x, y],
  );
}

function near(rgb: number[] | null, expected: readonly number[]): boolean {
  return rgb !== null && expected.every((v, i) => Math.abs(rgb[i] - v) <= 2);
}

test("a double-click on a MIDI clip opens it: the title and the notes, coloured by velocity", async ({ page }) => {
  const grid = await openRoll(page, project([keys()]));
  await expect(page.getByTestId("piano-roll-title")).toHaveText("Keys — Chords");
  await expect(page.getByTestId("piano-roll-empty")).toHaveCount(0);
  // The middle of each note shows its velocity's colour.
  for (const n of NOTES) {
    expect(near(await pixel(page, point(grid, n.start + n.length / 2, n.pitch)), velocityColor(n.velocity))).toBe(true);
  }
  // Zoomed in, the first note leaves its place; opening the clip again centres it once more.
  const first = point(grid, 0.5, 60);
  const roll = page.getByRole("region", { name: "Piano Roll" });
  for (let i = 0; i < 3; i++) await roll.getByRole("button", { name: "Zoom in" }).click();
  await expect.poll(async () => near(await pixel(page, first), velocityColor(100))).toBe(false);
  const arrangement = await page.getByRole("application", { name: "Arrangement" }).boundingBox();
  if (!arrangement) throw new Error("no arrangement");
  await page.mouse.dblclick(arrangement.x + 5 * 24, arrangement.y + 28);
  await expect.poll(async () => near(await pixel(page, first), velocityColor(100))).toBe(true);
});

test("a looped clip says so in the title", async ({ page }) => {
  await openRoll(page, project([keys(chords(NOTES, { loop_length: 2 }))]));
  await expect(page.getByTestId("piano-roll-title")).toHaveText("Keys — Chords  ·  loop 2 beats");
});

test("a click on empty space adds a floor-snapped note with the toolbar's length and velocity", async ({ page }) => {
  const grid = await openRoll(page, project([keys()]));
  const roll = page.getByRole("region", { name: "Piano Roll" });
  await roll.getByRole("combobox", { name: "Length" }).selectOption("1/8");
  await roll.getByRole("spinbutton", { name: "Vel" }).fill("77");
  await roll.getByRole("spinbutton", { name: "Vel" }).press("Enter");
  await page.mouse.click(...point(grid, 3.13, 65));
  await expect.poll(() => edits(page)).toEqual([
    { op: "add_notes", clip_id: "clp", notes: [{ pitch: 65, start: 3, length: 0.5, velocity: 77 }] },
  ]);
  expect(await commands(page)).toEqual(["edit"]);
});

test("a drag moves a note in time (snapped) and pitch: an edit per move, sealed once", async ({ page }) => {
  const grid = await openRoll(page, project([keys()]));
  await drag(page, point(grid, 0.5, 60), point(grid, 1.52, 62));
  await expect.poll(async () => (await commands(page)).at(-1)).toBe("seal_undo");
  const sent = await commands(page);
  expect(sent.filter((c) => c === "seal_undo")).toHaveLength(1);
  expect(sent.length).toBeGreaterThanOrEqual(2);
  const all = (await edits(page)) as { op: string; edits: { note_id: string }[] }[];
  expect(all.every((e) => e.op === "edit_notes" && e.edits.length === 1 && e.edits[0].note_id === "n0")).toBe(true);
  expect(all.at(-1)).toEqual({ op: "edit_notes", clip_id: "clp", edits: [{ note_id: "n0", pitch: 62, start: 1 }] });
});

test("⌥ bypasses the snap of a drag", async ({ page }) => {
  const grid = await openRoll(page, project([keys()]));
  await page.keyboard.down("Alt");
  await drag(page, point(grid, 0.5, 60), point(grid, 0.83, 60));
  await page.keyboard.up("Alt");
  await expect.poll(async () => (await commands(page)).at(-1)).toBe("seal_undo");
  const last = (await edits(page)).at(-1) as { edits: { start: number }[] };
  expect(Math.abs(last.edits[0].start - 0.33)).toBeLessThan(0.03);
});

test("a drag of the right edge resizes", async ({ page }) => {
  const grid = await openRoll(page, project([keys()]));
  const [x, y] = point(grid, 1.0, 60);
  const to = Math.round(beatX(grid.view, 1.5)) + grid.x - 2;
  await drag(page, [x - 2, y], [to, y]);
  await expect.poll(async () => (await commands(page)).at(-1)).toBe("seal_undo");
  expect((await edits(page)).at(-1)).toEqual({ op: "edit_notes", clip_id: "clp", edits: [{ note_id: "n0", length: 1.5 }] });
});

test("a rubber band selects notes, and Delete deletes them", async ({ page }) => {
  const grid = await openRoll(page, project([keys()]));
  await drag(page, point(grid, -0.05, 69), point(grid, 1.9, 59));
  await page.keyboard.press("Delete");
  await expect.poll(() => edits(page)).toEqual([{ op: "delete_notes", clip_id: "clp", note_ids: ["n0", "n1"] }]);
});

test("⇧-click toggles a note in the selection", async ({ page }) => {
  const grid = await openRoll(page, project([keys()]));
  await page.mouse.click(...point(grid, 0.5, 60));
  await page.keyboard.down("Shift");
  await page.mouse.click(...point(grid, 1.5, 64));
  await page.mouse.click(...point(grid, 0.5, 60));
  await page.keyboard.up("Shift");
  expect(await edits(page)).toEqual([]);
  await page.keyboard.press("Backspace");
  await expect.poll(() => edits(page)).toEqual([{ op: "delete_notes", clip_id: "clp", note_ids: ["n1"] }]);
});

test("a double-click deletes the note under the pointer", async ({ page }) => {
  const grid = await openRoll(page, project([keys()]));
  await page.mouse.dblclick(...point(grid, 2.2, 67));
  await expect.poll(() => edits(page)).toEqual([{ op: "delete_notes", clip_id: "clp", note_ids: ["n2"] }]);
});

test("the keys nudge by the grid and a semitone or an octave, duplicate after the span and select all", async ({ page }) => {
  const created = { ...OUTCOME, changes: { ...OUTCOME.changes, created: [{ kind: "note", id: "n9" }] } };
  const grid = await openRoll(page, project([keys()]));
  await page.mouse.click(...point(grid, 0.5, 60));
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("Shift+ArrowUp");
  await answer(page, "edit", created);
  await page.keyboard.press("Meta+d");
  await expect.poll(async () => (await edits(page)).length).toBe(4);
  expect(await edits(page)).toEqual([
    { op: "edit_notes", clip_id: "clp", edits: [{ note_id: "n0", pitch: 61, start: 0 }] },
    { op: "edit_notes", clip_id: "clp", edits: [{ note_id: "n0", pitch: 60, start: 0.25 }] },
    { op: "edit_notes", clip_id: "clp", edits: [{ note_id: "n0", pitch: 72, start: 0 }] },
    { op: "add_notes", clip_id: "clp", notes: [{ pitch: 60, start: 1, length: 1, velocity: 100, channel: 0 }] },
  ]);
  await answer(page, "edit", OUTCOME);
  await page.keyboard.press("Meta+a");
  await page.keyboard.press("Delete");
  await expect.poll(async () => (await edits(page)).at(-1)).toEqual({ op: "delete_notes", clip_id: "clp", note_ids: ["n0", "n1", "n2"] });
  // Each key is an edit of its own, never sealed (the prototype's `bridge.execute`).
  expect(await commands(page)).not.toContain("seal_undo");
});

test("the velocity lane paints a velocity: one edit on release, then the seal", async ({ page }) => {
  const grid = await openRoll(page, project([keys()]));
  const box = await lane(page);
  const x = box.x + Math.round(beatX(grid.view, 1.0)) + 1;
  await drag(page, [x, box.y + box.height - 10], [x, box.y + 2]);
  await expect.poll(() => commands(page)).toEqual(["edit", "seal_undo"]);
  expect(await edits(page)).toEqual([{ op: "edit_notes", clip_id: "clp", edits: [{ note_id: "n1", velocity: 127 }] }]);
});

test("the velocity lane shifts a selection of two or more by the same offset", async ({ page }) => {
  const grid = await openRoll(page, project([keys()]));
  await page.getByRole("application", { name: "Note grid" }).focus();
  await page.keyboard.press("Meta+a");
  const box = await lane(page);
  const x = box.x + Math.round(beatX(grid.view, 0.0)) + 1;
  await drag(page, [x, box.y + 36], [x, box.y + 56]);
  await expect.poll(() => commands(page)).toEqual(["edit", "seal_undo"]);
  const [edit] = (await edits(page)) as { edits: { note_id: string; velocity: number }[] }[];
  const deltas = new Set(edit.edits.map((e) => e.velocity - (NOTES.find((n) => n.id === e.note_id)?.velocity ?? 0)));
  expect(edit.edits).toHaveLength(3);
  expect(deltas.size).toBe(1);
  expect([...deltas][0]).toBeLessThan(0);
});

test("Quantize sends the grid at content beat 0 and the strength", async ({ page }) => {
  await openRoll(page, project([keys()]));
  const roll = page.getByRole("region", { name: "Piano Roll" });
  await roll.getByRole("combobox", { name: "Grid" }).selectOption("1/4");
  await roll.getByRole("spinbutton", { name: "Quantize strength" }).fill("50");
  await roll.getByRole("spinbutton", { name: "Quantize strength" }).press("Enter");
  await roll.getByRole("button", { name: "Quantize" }).click();
  await expect.poll(() => edits(page)).toEqual([{ op: "quantize_notes", clip_id: "clp", note_ids: [], grid: 1, strength: 0.5 }]);
});

test("the transpose buttons move every note, and refuse to leave the MIDI range", async ({ page }) => {
  const high = [...NOTES, note("n3", 120, 3, 1, 100)];
  await openRoll(page, project([keys(chords(high))]));
  const roll = page.getByRole("region", { name: "Piano Roll" });
  await roll.getByRole("button", { name: "+1", exact: true }).click();
  await expect.poll(() => edits(page)).toEqual([
    { op: "edit_notes", clip_id: "clp", edits: high.map((n) => ({ note_id: n.id, pitch: n.pitch + 1 })) },
  ]);
  await roll.getByRole("button", { name: "+12", exact: true }).click();
  await expect(page.getByRole("status")).toHaveText("transposing would leave the MIDI range");
  expect(await edits(page)).toHaveLength(1);
});

test("the snap controls are the timeline's", async ({ page }) => {
  await openRoll(page, project([keys()]));
  const roll = page.getByRole("region", { name: "Piano Roll" });
  const timeline = page.getByRole("region", { name: "Timeline" });
  await roll.getByRole("combobox", { name: "Grid" }).selectOption("1/8");
  await expect(timeline.getByRole("combobox", { name: "Grid" })).toHaveValue("1/8");
  await timeline.getByRole("combobox", { name: "Grid" }).selectOption("1/4");
  await expect(roll.getByRole("combobox", { name: "Grid" })).toHaveValue("1/4");
  await roll.getByRole("button", { name: "Snap" }).click();
  await expect(timeline.getByRole("button", { name: "Snap" })).toHaveAttribute("aria-pressed", "false");
});

test("a drum track's keyboard shows the GM names, 132 px wide", async ({ page }) => {
  const drums = keys(chords([note("d0", 36, 0, 0.25, 110), note("d1", 42, 0.5, 0.25, 90)]), "sonara.drums");
  await openRoll(page, project([{ ...drums, name: "Drums" }]));
  const keyboard = page.locator('canvas[aria-label="Drum notes"]');
  await expect(keyboard).toHaveCount(1);
  await expect.poll(async () => (await keyboard.boundingBox())?.width).toBe(132);
  expect((await calls(page)).some((c) => c.cmd === "drum_map")).toBe(true);
});

test("a click on a key selects its pitch's notes; the ruler seeks to the snapped beat", async ({ page }) => {
  const twice = [...NOTES, note("n3", 64, 2.5, 0.5, 70)];
  const grid = await openRoll(page, project([keys(chords(twice))]));
  const keyboard = await page.locator('canvas[aria-label="Keyboard"]').boundingBox();
  if (!keyboard) throw new Error("no keyboard");
  await page.mouse.click(keyboard.x + 20, point(grid, 0, 64)[1]);
  await page.getByRole("application", { name: "Note grid" }).focus();
  await page.keyboard.press("Delete");
  await expect.poll(() => edits(page)).toEqual([{ op: "delete_notes", clip_id: "clp", note_ids: ["n1", "n3"] }]);
  const ruler = await page.locator('canvas[aria-label="Piano roll ruler"]').boundingBox();
  if (!ruler) throw new Error("no ruler");
  // WebKit gives the pointer whole pixels: content beat 1.2 is well clear of a snap boundary.
  await page.mouse.click(grid.x + beatX(grid.view, 1.2), ruler.y + 10);
  // Content beat 1.25 of a clip at beat 4: bar 2, beat 2, 240 ticks.
  await expect(page.getByTestId("position")).toHaveText("2.2.240");
});

test("the context menu deletes, duplicates, quantizes, transposes and selects all", async ({ page }) => {
  const grid = await openRoll(page, project([keys()]));
  await page.mouse.click(...point(grid, 1.5, 64), { button: "right" });
  const menu = page.getByRole("menu", { name: "Notes" });
  await expect(menu.getByRole("menuitem")).toHaveText(["Delete", "Duplicate", "Quantize", "Transpose +1", "Transpose -1", "Select All"]);
  await menu.getByRole("menuitem", { name: "Delete" }).click();
  await expect.poll(() => edits(page)).toEqual([{ op: "delete_notes", clip_id: "clp", note_ids: ["n1"] }]);
});

test("a clip deleted elsewhere returns the piano roll to its empty state", async ({ page }) => {
  await openRoll(page, project([keys()]));
  await answer(page, "project_view", view(2, project([track("k", "Keys", "instrument")])));
  await emit(page, "session-changed", {
    version: 2,
    kind: "command",
    label: "Delete Clips",
    source: "agent",
    changes: { created: [], updated: [], deleted: [{ kind: "clip", id: "clp" }], flags: ["clips"], structure_changed: false },
  });
  await expect(page.getByTestId("piano-roll-empty")).toBeVisible();
  await expect(page.getByTestId("piano-roll-title")).toHaveText("Piano Roll");
});

test("an agent's notes appear in the piano roll where they are drawn", async ({ page }) => {
  const grid = await openRoll(page, project([keys()]));
  const added = [...NOTES, note("agent", 62, 3, 0.5, 64)];
  await answer(page, "project_view", view(2, project([keys(chords(added))])));
  // A note edit refreshes its clip only (`project_clips`).
  await answer(page, "project_clips", {
    version: 2,
    project: { id: "prj_1", dirty: true, history: { undo: "Add Notes", redo: null }, meter_layout: 1, clips: [{ track_id: "k", clip: chords(added) }] },
  });
  await emit(page, "session-changed", {
    version: 2,
    kind: "command",
    label: "Add Notes",
    source: "agent",
    changes: { created: [{ kind: "note", id: "agent" }], updated: [{ kind: "clip", id: "clp" }], deleted: [], flags: ["notes"], structure_changed: false },
  });
  await expect.poll(async () => (await calls(page)).some((c) => c.cmd === "project_clips")).toBe(true);
  // Hit where it is drawn: a double-click deletes it.
  await expect
    .poll(async () => {
      await page.mouse.dblclick(...point(grid, 3.25, 62));
      return (await edits(page)).at(-1);
    })
    .toEqual({ op: "delete_notes", clip_id: "clp", note_ids: ["agent"] });
});
