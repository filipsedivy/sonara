import { expect, type Page, test } from "@playwright/test";
import { audioClip, midiClip, project, track, view } from "../support/project";
import type { OpenProject, TrackView } from "../../src/lib/types";
import { AUDIO_SUFFIXES, DATA_LAYER, answer, calls, emit, mockTauri, sendChannel } from "./tauri-mock";
import { frameHex } from "../support/frame";
import { trackList } from "../support/tracks";

// The timeline against mocked commands, driven through the canvas's
// pointer, wheel and key events in WebKit: the prototype's tests/gui/test_timeline.py, whose edits
// the page sends to sonara-app (P). The default zoom is 24 px per beat and a row 56 px high.

const PPB = 24;
const ROW = 56;

const OUTCOME = {
  committed: true,
  version: 2,
  label: "Edit",
  changes: { created: [], updated: [], deleted: [], flags: [], structure_changed: false },
  result: {},
};

/** Opens the window on `open`, with the side panes and the Editors hidden. */
async function openTimeline(page: Page, open: OpenProject, extra: Record<string, unknown> = {}) {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: {
      agent: { on: false, address: null },
      messages: [],
      panes: {
        browser: { size: 240, open: false },
        inspector: { size: 280, open: false },
        editors: { size: 300, open: false },
      },
      audio_suffixes: AUDIO_SUFFIXES,
    },
    recoverable_sessions: [],
    project_view: view(1, open),
    edit: OUTCOME,
    seal_undo: null,
    ...extra,
  });
  await page.goto("/");
  await expect(trackList(page).getByRole("option")).toHaveCount(open.tracks.length);
}

async function canvas(page: Page) {
  const box = await page.getByRole("application", { name: "Arrangement" }).boundingBox();
  if (!box) throw new Error("no arrangement");
  return box;
}

/** `drag`: press, six moves, release, in canvas coordinates. */
async function drag(page: Page, x0: number, y0: number, x1: number, y1: number) {
  const box = await canvas(page);
  await page.mouse.move(box.x + x0, box.y + y0);
  await page.mouse.down();
  for (let i = 1; i <= 6; i++) {
    await page.mouse.move(box.x + x0 + ((x1 - x0) * i) / 6, box.y + y0 + ((y1 - y0) * i) / 6);
  }
  await page.mouse.up();
}

async function edits(page: Page): Promise<unknown[]> {
  return (await calls(page)).filter((c) => c.cmd === "edit").map((c) => (c.args as { edit: unknown }).edit);
}

async function commands(page: Page): Promise<string[]> {
  return (await calls(page))
    .map((c) => c.cmd)
    .filter((cmd) => ["edit", "seal_undo", "import_audio_files", "open_project_file", "transport_play"].includes(cmd));
}

function synth(clips: TrackView["clips"] = []): TrackView {
  return track("t", "Synth", "instrument", clips);
}

test("the timeline shows the tracks and their clips", async ({ page }) => {
  const tracks = [track("a", "Lead"), track("b", "Bass", "instrument", [midiClip("clp", 4, 8)])];
  await openTimeline(page, project(tracks));
  await expect(trackList(page).locator(".name")).toHaveText(["Lead", "Bass"]);
  await expect(page.getByTestId("timeline-info")).toHaveText("120 BPM  4/4  ·  2 tracks");
  // The renderer started: no error in the status bar.
  await expect(page.getByRole("alert")).toBeHidden();
});

test("folder children are indented", async ({ page }) => {
  const folder = track("f", "Group", "folder");
  const child = { ...track("c", "Child", "audio"), parent_id: "f", depth: 1 };
  await openTimeline(page, project([folder, child]));
  const names = trackList(page).locator(".name");
  await expect(names.nth(0)).toHaveAttribute("data-depth", "0");
  await expect(names.nth(1)).toHaveAttribute("data-depth", "1");
});

test("a drag moves the clip with the snap as one undo step", async ({ page }) => {
  await openTimeline(page, project([synth([midiClip("clp", 0, 4)])]));
  await drag(page, 2 * PPB, ROW / 2, 4.6 * PPB, ROW / 2);
  await expect.poll(() => commands(page)).toEqual(["edit", "seal_undo"]);
  expect(await edits(page)).toEqual([{ op: "move_clip", clip_id: "clp", start: 2.5 }]);
});

test("⌥ bypasses the snap", async ({ page }) => {
  await openTimeline(page, project([synth([midiClip("clp", 0, 4)])]));
  await page.keyboard.down("Alt");
  await drag(page, 2 * PPB, ROW / 2, 4.6 * PPB, ROW / 2);
  await page.keyboard.up("Alt");
  await expect.poll(async () => (await edits(page)).length).toBe(1);
  const [edit] = (await edits(page)) as { start: number }[];
  // Unsnapped: 2.6 beats within the pointer's whole pixel (the prototype's test moved by 62.4 px).
  expect(Math.abs(edit.start - 2.6)).toBeLessThan(1 / PPB);
  expect(edit.start % 0.25).not.toBe(0);
});

test("a vertical drag moves a clip to a compatible track only", async ({ page }) => {
  const tracks = [track("a", "A", "instrument", [midiClip("clp", 0, 4)]), track("au", "Audio", "audio"), track("b", "B")];
  await openTimeline(page, project(tracks));
  await drag(page, 2 * PPB, ROW / 2, 2 * PPB, ROW * 1.5);
  await drag(page, 2 * PPB, ROW / 2, 2 * PPB, ROW * 2.5);
  await expect.poll(() => edits(page)).toEqual([{ op: "move_clip", clip_id: "clp", start: 0, track_id: "b" }]);
});

test("the clip edges trim, the left one keeping the content in place", async ({ page }) => {
  await openTimeline(page, project([synth([midiClip("clp", 0, 4)])]));
  await drag(page, 4 * PPB - 3, ROW / 2, 5 * PPB - 3, ROW / 2);
  await drag(page, 3, ROW / 2, 3 + PPB, ROW / 2);
  await expect.poll(() => edits(page)).toEqual([
    { op: "trim_clip", clip_id: "clp", end: 5 },
    { op: "trim_clip", clip_id: "clp", start: 1 },
  ]);
});

test("the keys duplicate, split at the playhead and delete", async ({ page }) => {
  await openTimeline(page, project([synth([midiClip("clp", 0, 4)])]));
  const box = await canvas(page);
  await page.mouse.click(box.x + 2 * PPB, box.y + ROW / 2);
  await page.keyboard.press("Meta+d");
  // The playhead to beat 2 through the ruler, then S splits there.
  const ruler = await page.locator("canvas.ruler").boundingBox();
  if (!ruler) throw new Error("no ruler");
  await page.mouse.click(ruler.x + 2 * PPB, ruler.y + 36);
  await expect(page.getByTestId("position")).toHaveText("1.3.000");
  await page.mouse.click(box.x + 3 * PPB, box.y + ROW / 2);
  await page.keyboard.press("s");
  await page.keyboard.press("Delete");
  await expect.poll(() => edits(page)).toEqual([
    { op: "duplicate_clips", clip_ids: ["clp"] },
    { op: "split_clips", clip_ids: ["clp"], at: 2 },
    { op: "delete_clips", clip_ids: ["clp"] },
  ]);
});

test("the arrow keys nudge by the grid", async ({ page }) => {
  await openTimeline(page, project([synth([midiClip("clp", 1, 4)])]));
  const box = await canvas(page);
  await page.mouse.click(box.x + 2 * PPB, box.y + ROW / 2);
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowLeft");
  await expect.poll(() => edits(page)).toEqual([
    { op: "move_clips", clip_ids: ["clp"], delta_beats: 0.25 },
    { op: "move_clips", clip_ids: ["clp"], delta_beats: -0.25 },
  ]);
});

test("a rubber band selects clips across tracks", async ({ page }) => {
  const tracks = [
    track("a", "A", "instrument", [midiClip("c1", 2, 2)]),
    track("b", "B", "instrument", [midiClip("c2", 3, 2), midiClip("c3", 20, 2)]),
  ];
  await openTimeline(page, project(tracks));
  await drag(page, 1 * PPB, 5, 6 * PPB, ROW * 2 - 5);
  await page.keyboard.press("Delete");
  await expect.poll(async () => (await edits(page)) as { clip_ids: string[] }[]).toHaveLength(1);
  const [deleted] = (await edits(page)) as { op: string; clip_ids: string[] }[];
  expect(deleted.op).toBe("delete_clips");
  expect(new Set(deleted.clip_ids)).toEqual(new Set(["c1", "c2"]));
});

test("a double-click opens a MIDI clip in the piano roll", async ({ page }) => {
  await openTimeline(page, project([synth([midiClip("clp", 0, 4)])]));
  const box = await canvas(page);
  await page.mouse.dblclick(box.x + 2 * PPB, box.y + ROW / 2);
  await expect(page.getByRole("tab", { name: "Piano Roll" })).toHaveAttribute("aria-selected", "true");
  expect(await edits(page)).toEqual([]);
});

test("a double-click on an empty instrument lane creates a one-bar clip", async ({ page }) => {
  await openTimeline(page, project([synth()]));
  const box = await canvas(page);
  await page.mouse.dblclick(box.x + 5.5 * PPB, box.y + ROW / 2);
  await expect.poll(() => edits(page)).toEqual([
    { op: "create_midi_clip", track_id: "t", start: 4, length: 4, name: "Synth" },
  ]);
});

test("the ruler seeks, draws a loop with ⇧ and drags its end", async ({ page }) => {
  await openTimeline(page, project([synth()]));
  const ruler = await page.locator("canvas.ruler").boundingBox();
  if (!ruler) throw new Error("no ruler");
  await page.mouse.click(ruler.x + 10 * PPB, ruler.y + 36);
  await expect(page.getByTestId("position")).toHaveText("3.3.000");
  await page.keyboard.down("Shift");
  await page.mouse.move(ruler.x + 8 * PPB + 2, ruler.y + 36);
  await page.mouse.down();
  for (let i = 1; i <= 6; i++) await page.mouse.move(ruler.x + 8 * PPB + 2 + (8 * PPB * i) / 6, ruler.y + 36);
  await page.mouse.up();
  await page.keyboard.up("Shift");
  await expect.poll(() => commands(page)).toEqual(["edit", "seal_undo"]);
  expect(await edits(page)).toEqual([{ op: "set_transport", loop_start: 8, loop_end: 16, loop_enabled: true }]);
  // The project's loop is 0–16: its end handle in the loop band drags to beat 20.
  await page.mouse.move(ruler.x + 16 * PPB, ruler.y + 19);
  await page.mouse.down();
  for (let i = 1; i <= 6; i++) await page.mouse.move(ruler.x + 16 * PPB + (4 * PPB * i) / 6, ruler.y + 19);
  await page.mouse.up();
  await expect.poll(() => commands(page)).toEqual(["edit", "seal_undo", "edit", "seal_undo"]);
  expect((await edits(page))[1]).toEqual({ op: "set_transport", loop_start: 0, loop_end: 20 });
});

test("⌘-wheel zooms around the pointer; Fit and the zoom buttons change the view", async ({ page }) => {
  await openTimeline(page, project([synth([midiClip("clp", 0, 64)])]));
  const box = await canvas(page);
  const ruler = await page.locator("canvas.ruler").boundingBox();
  if (!ruler) throw new Error("no ruler");
  const seekAt = async (x: number) => {
    await page.mouse.click(ruler.x + x, ruler.y + 36);
    return page.getByTestId("position").textContent();
  };
  const before = await seekAt(300);
  await page.mouse.move(box.x + 300, box.y + 20);
  await page.keyboard.down("Meta");
  await page.mouse.wheel(0, -240);
  await page.keyboard.up("Meta");
  expect(await seekAt(300)).toBe(before);
  await page.getByRole("button", { name: "Fit" }).click();
  // The whole song (64 beats) fits in the width less 24 px.
  const fitted = await seekAt(box.width - 24);
  expect(fitted).toBe("17.1.000");
});

test("Follow scrolls the view with the playhead during playback; off, the view stays", async ({ page }) => {
  // The prototype's test_follow_playhead_scrolls_during_playback, through the live frames: a
  // playhead at beat 200 brings the view along, so that it is drawn in the left half (at 10 %).
  await openTimeline(page, project([synth([midiClip("clp", 0, 480)])]));
  const box = await canvas(page);
  const playhead = page.getByTestId("playhead");
  const playheadX = () => playhead.evaluate((element) => Number.parseFloat(/translateX\(([-\d.]+)px\)/.exec(element.style.transform)?.[1] ?? "NaN"));
  const follow = page.getByRole("button", { name: "Follow" });
  await expect(follow).toHaveAttribute("aria-pressed", "true");
  await sendChannel(page, frameHex({ sequence: 1, meterLayout: 0, meters: [], beat: 200 }));
  // Without Follow, beat 200 would be 4,800 px to the right, past the view.
  await expect.poll(async () => Math.abs((await playheadX()) - box.width * 0.1)).toBeLessThanOrEqual(2);
  const followed = await playheadX();
  expect(followed).toBeLessThan(box.width * 0.5);
  // Follow off: ten beats later the playhead moves on, and the view does not.
  await follow.click();
  await expect(follow).toHaveAttribute("aria-pressed", "false");
  await sendChannel(page, frameHex({ sequence: 2, meterLayout: 0, meters: [], beat: 210 }));
  await expect.poll(playheadX).toBeCloseTo(followed + 10 * PPB, 0);
  await sendChannel(page, frameHex({ sequence: 3, meterLayout: 0, meters: [], beat: 400 }));
  await expect(playhead).toHaveCount(0);
});

test("the context menu mutes a clip and adds a MIDI clip on an empty lane", async ({ page }) => {
  await openTimeline(page, project([synth([midiClip("clp", 0, 4)])]));
  const box = await canvas(page);
  await page.mouse.click(box.x + 2 * PPB, box.y + ROW / 2, { button: "right" });
  const menu = page.getByRole("menu");
  await expect(menu.getByRole("menuitem")).toHaveText(["Open in Piano Roll", "Split Here", "Duplicate", "Mute", "Delete"]);
  await menu.getByRole("menuitem", { name: "Mute" }).click();
  await page.mouse.click(box.x + 9 * PPB, box.y + ROW / 2, { button: "right" });
  await page.getByRole("menuitem", { name: "Add MIDI Clip" }).click();
  await expect.poll(() => edits(page)).toEqual([
    { op: "set_clip_properties", clip_id: "clp", muted: true },
    { op: "create_midi_clip", track_id: "t", start: 8, length: 4, name: "Synth" },
  ]);
});

test("+ Track adds a track named after its kind and selects it", async ({ page }) => {
  await openTimeline(page, project([synth()]), { edit: { ...OUTCOME, result: { track_id: "trk_new" } } });
  await page.getByRole("button", { name: "+ Track" }).click();
  await page.getByRole("menuitem", { name: "Instrument Track" }).click();
  await expect.poll(() => edits(page)).toEqual([
    { op: "create_track", name: "Instrument 2", kind: "instrument", instrument: "sonara.synth.poly" },
  ]);
});

test("audio files dropped on the canvas import into the audio track under the pointer", async ({ page }) => {
  const tracks = [track("au", "Audio", "audio", [audioClip("a1", "ast_1", 40, 4)]), synth()];
  await openTimeline(page, project(tracks), { import_audio_files: "op_1", open_project_file: null });
  const box = await canvas(page);
  const dpr = await page.evaluate(() => window.devicePixelRatio);
  const at = (x: number, y: number) => ({ x: (box.x + x) * dpr, y: (box.y + y) * dpr });
  const paths = ["/Users/me/Music/drop.wav", "/Users/me/notes.txt"];
  await emit(page, "tauri://drag-enter", { paths, position: at(4.2 * PPB, ROW / 2) });
  await emit(page, "tauri://drag-over", { position: at(4.2 * PPB, ROW / 2) });
  await emit(page, "tauri://drag-drop", { paths, position: at(4.2 * PPB, ROW / 2) });
  // Over the instrument track: new tracks; outside the canvas: new tracks at the playhead.
  await emit(page, "tauri://drag-drop", { paths, position: at(6.6 * PPB, ROW * 1.5) });
  await emit(page, "tauri://drag-drop", { paths, position: { x: 2 * dpr, y: 2 * dpr } });
  // A project opens instead.
  await emit(page, "tauri://drag-drop", { paths: ["/Users/me/Song.sonara/"], position: at(10, 10) });
  await expect.poll(async () => (await calls(page)).filter((c) => c.cmd !== "edit").map((c) => [c.cmd, c.args]).filter(([cmd]) => cmd === "import_audio_files" || cmd === "open_project_file")).toEqual([
    ["import_audio_files", { paths: ["/Users/me/Music/drop.wav"], trackId: "au", startBeat: 4 }],
    ["import_audio_files", { paths: ["/Users/me/Music/drop.wav"], trackId: null, startBeat: 6.5 }],
    ["import_audio_files", { paths: ["/Users/me/Music/drop.wav"], trackId: null, startBeat: 0 }],
    ["open_project_file", { path: "/Users/me/Song.sonara/" }],
  ]);
});

test("an agent's edit from elsewhere appears in the timeline", async ({ page }) => {
  await openTimeline(page, project([synth()]));
  const agentTrack = track("agent", "Agent Track", "instrument", [midiClip("agent_clip", 8, 4)]);
  await answer(page, "project_view", view(2, project([synth(), agentTrack])));
  await emit(page, "session-changed", {
    version: 2,
    kind: "command",
    label: "Create Track",
    source: "agent",
    changes: { created: [], updated: [], deleted: [], flags: ["tracks"], structure_changed: true },
  });
  await expect(trackList(page).locator(".name")).toHaveText(["Synth", "Agent Track"]);
  // Its clip is hit where it is drawn: a double-click opens it.
  const box = await canvas(page);
  await page.mouse.dblclick(box.x + 10 * PPB, box.y + ROW * 1.5);
  await expect(page.getByRole("tab", { name: "Piano Roll" })).toHaveAttribute("aria-selected", "true");
});
