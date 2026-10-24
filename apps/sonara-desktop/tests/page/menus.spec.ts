import { expect, type Page, test } from "@playwright/test";
import { defaults, pluginMap } from "../support/plugins";
import { project, track, view } from "../support/project";
import type { DeviceView, MidiClipView, NoteView, OpenProject, TrackView } from "../../src/lib/types";
import { AUDIO_SUFFIXES, DATA_LAYER, answer, calls, emit, mockTauri } from "./tauri-mock";
import { trackList } from "../support/tracks";

// The menus and shortcuts against mocked commands, in WebKit: the routing of the
// Edit, Track and Transport menus' commands (`menu-command`) to the focused editor, the selection
// and the transport bar; G9: the focused editor takes its own ⌘D, ⌘E, ⌘A and Delete,
// so the native menu never sees them, and the Edit menu's items act on it; the page's report of
// the focused editor and its dialogs (`set_menu_context`); and the single-key shortcuts, which
// never fire in a text field or while a dialog is open. The native menu itself, chosen and keyed
// for real, is the end-to-end harness's (src/lib/bench/e2e-menus.ts).

const OUTCOME = {
  committed: true,
  version: 2,
  label: "Edit",
  changes: { created: [], updated: [], deleted: [], flags: [], structure_changed: false },
  result: {},
};

function note(id: string, pitch: number, start: number): NoteView {
  return { id, pitch, start, length: 1, velocity: 100, channel: 0 };
}

const CHORDS: MidiClipView = {
  type: "midi",
  id: "clp",
  name: "Chords",
  start: 4,
  length: 8,
  muted: false,
  color: null,
  content_offset: 0,
  loop_length: null,
  notes: [note("n0", 60, 0), note("n1", 64, 1), note("n2", 67, 2)],
};

const SYNTH: DeviceView = {
  id: "dev_synth",
  plugin_id: "sonara.synth.poly",
  name: "Sonara Poly Synth",
  enabled: true,
  preset: null,
  sidechain_id: null,
  available: true,
  params: defaults("sonara.synth.poly"),
};

function songProject(): OpenProject {
  const keys: TrackView = { ...track("k", "Keys", "instrument", [CHORDS]), instrument: SYNTH };
  return project([keys, track("v", "Vox", "audio")], { plugins: pluginMap(["sonara.synth.poly"]) });
}

async function openWindow(page: Page, extra: Record<string, unknown> = {}) {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: {
      agent: { on: false, address: null },
      messages: [],
      panes: {
        browser: { size: 240, open: false },
        inspector: { size: 280, open: true },
        editors: { size: 470, open: true },
      },
      audio_suffixes: AUDIO_SUFFIXES,
    },
    recoverable_sessions: [],
    project_view: view(1, songProject()),
    edit: OUTCOME,
    seal_undo: null,
    delete_track: true,
    transport_play: { state: "playing", position_beat: 0 },
    transport_record: { recording: true },
    ...extra,
  });
  await page.goto("/");
  await expect(trackList(page).getByRole("option")).toHaveCount(2);
}

const arrangement = (page: Page) => page.getByRole("application", { name: "Arrangement" });
const noteGrid = (page: Page) => page.getByRole("application", { name: "Note grid" });

/** Clicks the clip "clp" (beat 5 of the first row): it is selected and the timeline has the focus. */
async function selectClip(page: Page) {
  const box = await arrangement(page).boundingBox();
  if (!box) throw new Error("no arrangement");
  await page.mouse.click(box.x + 5 * 24, box.y + 28);
}

/** A double-click on the clip opens it in the piano roll. */
async function openRoll(page: Page) {
  const box = await arrangement(page).boundingBox();
  if (!box) throw new Error("no arrangement");
  await page.mouse.dblclick(box.x + 5 * 24, box.y + 28);
  await expect(page.getByRole("tab", { name: "Piano Roll" })).toHaveAttribute("aria-selected", "true");
}

async function edits(page: Page): Promise<unknown[]> {
  return (await calls(page)).filter((c) => c.cmd === "edit").map((c) => (c.args as { edit: unknown }).edit);
}

async function contexts(page: Page): Promise<unknown[]> {
  return (await calls(page)).filter((c) => c.cmd === "set_menu_context").map((c) => (c.args as { context: unknown }).context);
}

const command = (page: Page, name: string, extra: Record<string, unknown> = {}) => emit(page, "menu-command", { command: name, ...extra });

/**
 * Records every key the page gets and, once its dispatch is over, whether the page consumed it
 * (`preventDefault`): the web view then gives it to no menu.
 */
async function watchKeys(page: Page) {
  await page.evaluate(() => {
    const seen: KeyboardEvent[] = [];
    (window as unknown as { __keys: KeyboardEvent[] }).__keys = seen;
    window.addEventListener("keydown", (event) => seen.push(event), { capture: true });
  });
}

const keysSeen = (page: Page) =>
  page.evaluate(() =>
    (window as unknown as { __keys: KeyboardEvent[] }).__keys
      .splice(0)
      .map((event) => ({ key: event.key, meta: event.metaKey, prevented: event.defaultPrevented })),
  );

test("G9: the Edit menu's Split, Duplicate and Delete act on the focused timeline, also with no editor focused", async ({ page }) => {
  await openWindow(page);
  // The playhead at beat 6, inside the clip (an unsnapped ruler click).
  const ruler = await page.locator("canvas.ruler").boundingBox();
  if (!ruler) throw new Error("no ruler");
  await page.mouse.click(ruler.x + 6 * 24, ruler.y + 36);
  await selectClip(page);
  await expect.poll(() => contexts(page)).toContainEqual({ focus: "timeline", modal: false });
  await command(page, "duplicate-clips");
  await expect.poll(() => edits(page)).toEqual([{ op: "duplicate_clips", clip_ids: ["clp"] }]);
  // The copies become the selection (none in the mocked outcome): the clip is selected again.
  await selectClip(page);
  await command(page, "split-clips");
  await expect.poll(async () => (await edits(page)).slice(1)).toEqual([{ op: "split_clips", clip_ids: ["clp"], at: 6 }]);
  // The focus elsewhere (no editor): the timeline's clips, as the prototype's items did.
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await expect.poll(() => contexts(page)).toContainEqual({ focus: "none", modal: false });
  await command(page, "delete-clips");
  await expect.poll(async () => (await edits(page)).slice(2)).toEqual([{ op: "delete_clips", clip_ids: ["clp"] }]);
});

test("G9: with the piano roll focused they act on its notes, and Split does nothing; a device card has none", async ({ page }) => {
  await openWindow(page);
  await openRoll(page);
  await noteGrid(page).focus();
  await expect.poll(() => contexts(page)).toContainEqual({ focus: "piano-roll", modal: false });
  await page.keyboard.press("Meta+a");
  await command(page, "split-clips");
  await command(page, "duplicate-clips");
  await expect.poll(() => edits(page)).toHaveLength(1);
  const [duplicate] = (await edits(page)) as { op: string; clip_id: string; notes: unknown[] }[];
  expect(duplicate.op).toBe("add_notes");
  expect(duplicate.clip_id).toBe("clp");
  expect(duplicate.notes).toHaveLength(3);
  // The copies become the selection (none in the mocked outcome): every note is selected again.
  await page.keyboard.press("Meta+a");
  await command(page, "delete-clips");
  await expect.poll(async () => (await edits(page)).slice(1)).toEqual([{ op: "delete_notes", clip_id: "clp", note_ids: ["n0", "n1", "n2"] }]);
  // A device card's knob: none of the three runs.
  await page.getByRole("tab", { name: "Devices" }).click();
  const knob = page.getByRole("region", { name: "Devices" }).getByRole("slider").first();
  await knob.focus();
  await expect.poll(() => contexts(page)).toContainEqual({ focus: "devices", modal: false });
  for (const name of ["split-clips", "duplicate-clips", "delete-clips"]) await command(page, name);
  await page.waitForTimeout(150);
  expect(await edits(page)).toHaveLength(2);
});

test("G9: the focused editor consumes its own ⌘D, ⌘E, ⌘A and Delete, so the native menu never sees them", async ({ page }) => {
  await openWindow(page);
  await watchKeys(page);
  await selectClip(page);
  for (const key of ["Meta+d", "Meta+e", "Meta+a", "Delete"]) await page.keyboard.press(key);
  expect((await keysSeen(page)).filter((k) => k.key !== "Meta")).toEqual([
    { key: "d", meta: true, prevented: true },
    { key: "e", meta: true, prevented: true },
    { key: "a", meta: true, prevented: true },
    { key: "Delete", meta: false, prevented: true },
  ]);
  await openRoll(page);
  await noteGrid(page).focus();
  await keysSeen(page);
  for (const key of ["Meta+a", "Meta+d", "Delete"]) await page.keyboard.press(key);
  expect((await keysSeen(page)).filter((k) => k.key !== "Meta")).toEqual([
    { key: "a", meta: true, prevented: true },
    { key: "d", meta: true, prevented: true },
    { key: "Delete", meta: false, prevented: true },
  ]);
  // A knob takes Delete (it resets the value) and leaves ⌘D to the menu, whose items are off then.
  await page.getByRole("tab", { name: "Devices" }).click();
  await page.getByRole("region", { name: "Devices" }).getByRole("slider").first().focus();
  await keysSeen(page);
  for (const key of ["Delete", "Meta+d"]) await page.keyboard.press(key);
  expect((await keysSeen(page)).filter((k) => k.key !== "Meta")).toEqual([
    { key: "Delete", meta: false, prevented: true },
    { key: "d", meta: true, prevented: false },
  ]);
  // A text field leaves ⌘A to the system's Select All, which selects its text.
  await page.locator("#project-name").focus();
  await keysSeen(page);
  await page.keyboard.press("Meta+a");
  expect((await keysSeen(page)).filter((k) => k.key !== "Meta")).toEqual([{ key: "a", meta: true, prevented: false }]);
});

test("the Track menu's new tracks are created as sonara-app names them and selected; Delete Selected Track asks the desktop", async ({ page }) => {
  await openWindow(page);
  await answer(page, "edit", { ...OUTCOME, label: "Create Track", result: { track_id: "trk_new" } });
  const edit = { op: "create_track", name: "Audio", kind: "audio", instrument: null };
  await command(page, "new-audio-track", { edit });
  await expect.poll(() => edits(page)).toEqual([edit]);
  // The new track is the selection's current track: it gets the MIDI focus the page sends.
  await expect.poll(async () => (await calls(page)).filter((c) => c.cmd === "set_midi_focus").at(-1)?.args).toEqual({ trackId: "trk_new" });
  await page.locator('.header[data-track="k"] .name').click();
  await command(page, "delete-track");
  await expect.poll(async () => (await calls(page)).filter((c) => c.cmd === "delete_track")).toEqual([{ cmd: "delete_track", args: { trackId: "k" } }]);
});

test("the Transport menu's commands run the transport bar's actions", async ({ page }) => {
  await openWindow(page);
  await command(page, "loop");
  await command(page, "metronome");
  await expect.poll(() => edits(page)).toEqual([
    { op: "set_transport", loop_enabled: true },
    { op: "set_transport", metronome: true },
  ]);
  await command(page, "play-stop");
  await command(page, "record");
  await command(page, "return-to-start");
  await expect
    .poll(async () => (await calls(page)).filter((c) => ["transport_play", "transport_record"].includes(c.cmd)).map((c) => c.cmd))
    .toEqual(["transport_play", "transport_record"]);
  await expect(page.getByTestId("position")).toHaveText("1.1.000");
});

test("while a dialog is open, neither the menu's commands nor the single-key shortcuts run, and the page says so", async ({ page }) => {
  await openWindow(page, {
    recoverable_sessions: [{ workspace: "/w/new-a", name: "Song", saved_at: "2026-10-22T10:00:00Z" }],
  });
  const dialog = page.getByRole("dialog", { name: "Recover Unsaved Work" });
  await expect(dialog).toBeVisible();
  await expect.poll(() => contexts(page)).toContainEqual({ focus: "none", modal: true });
  // The keys go to the dialog's list of sessions, which takes them (Space selects, a letter finds).
  await dialog.getByRole("listbox").focus();
  for (const key of ["Space", "Home", "r", "l", "c"]) await page.keyboard.press(key);
  for (const name of ["play-stop", "loop", "metronome", "record", "duplicate-clips", "delete-track"]) await command(page, name);
  await command(page, "new-bus", { edit: { op: "create_track", name: "Bus", kind: "bus", instrument: null } });
  await page.waitForTimeout(200);
  const transportOrEdits = (await calls(page)).filter((c) => c.cmd === "edit" || c.cmd === "delete_track" || c.cmd.startsWith("transport_"));
  expect(transportOrEdits.filter((c) => c.cmd !== "transport_status")).toEqual([]);
  await dialog.getByRole("button", { name: "Not Now" }).click();
  await expect.poll(async () => (await contexts(page)).at(-1)).toEqual({ focus: "none", modal: false });
  await command(page, "loop");
  await expect.poll(() => edits(page)).toEqual([{ op: "set_transport", loop_enabled: true }]);
});

test("the single-key shortcuts never fire while a text field has focus, each of them", async ({ page }) => {
  await openWindow(page);
  const name = page.locator("#project-name");
  const before = await name.inputValue();
  await name.focus();
  for (const key of ["Space", "Home", "r", "l", "c"]) await page.keyboard.press(key);
  // The field took them as text: none became the global shortcut. Home is a no-op here on
  // macOS WebKit (unlike Linux's WebKitGTK, it does not move the caret to the field's start),
  // so every key lands where typed, at the end.
  await expect(name).toHaveValue(`${before} rlc`);
  await page.keyboard.press("Escape");
  await page.getByRole("spinbutton", { name: "Tempo" }).focus();
  for (const key of ["Space", "Home", "r", "l", "c"]) await page.keyboard.press(key);
  await page.keyboard.press("Escape");
  await page.waitForTimeout(150);
  expect((await calls(page)).filter((c) => c.cmd.startsWith("transport_") && c.cmd !== "transport_status")).toEqual([]);
  const transport = async () => (await edits(page)).filter((e) => (e as { op: string }).op === "set_transport");
  expect(await transport()).toEqual([]);
  // Elsewhere they run.
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await page.keyboard.press("l");
  await expect.poll(transport).toEqual([{ op: "set_transport", loop_enabled: true }]);
});
