import { expect, type Page, test } from "@playwright/test";
import { project, track, view } from "../support/project";
import type { OpenProject, TrackView } from "../../src/lib/types";
import { AUDIO_SUFFIXES, DATA_LAYER, answer, calls, emit, mockTauri } from "./tauri-mock";
import { trackControls, trackList, trackRow } from "../support/tracks";

// The track headers (–TH-06) against mocked commands in WebKit: the
// prototype's tests/gui/test_timeline.py `header_buttons_rename_and_reorder`, and the rest of the
// TH rows (P). Rows are 56 px high at the default height.

const ROW = 56;

const OUTCOME = {
  committed: true,
  version: 2,
  label: "Edit",
  changes: { created: [], updated: [], deleted: [], flags: [], structure_changed: false },
  result: {},
};

async function openHeaders(page: Page, open: OpenProject, extra: Record<string, unknown> = {}) {
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
    import_audio_dialog: null,
    ...extra,
  });
  await page.goto("/");
  await expect(rows(page)).toHaveCount(open.tracks.length);
}

const rows = (page: Page) => trackList(page).getByRole("option");
const row = trackRow;

async function edits(page: Page): Promise<unknown[]> {
  return (await calls(page)).filter((c) => c.cmd === "edit").map((c) => (c.args as { edit: unknown }).edit);
}

async function box(page: Page) {
  const found = await trackList(page).boundingBox();
  if (!found) throw new Error("no headers");
  return found;
}

function abc(): TrackView[] {
  return [track("a", "A"), track("b", "B"), track("c", "C")];
}

test("M, S and R edit the track; folders have no buttons, buses no R", async ({ page }) => {
  await openHeaders(page, project([...abc(), track("v", "Verb", "bus"), track("f", "Group", "folder")]));
  const a = row(page, "A");
  const aButtons = trackControls(page, "A");
  await aButtons.getByRole("button", { name: "Mute" }).click();
  await aButtons.getByRole("button", { name: "Solo" }).click();
  await aButtons.getByRole("button", { name: "Arm for recording" }).click();
  await expect.poll(() => edits(page)).toEqual([
    { op: "set_track_mixer", track_id: "a", mute: true },
    { op: "set_track_mixer", track_id: "a", solo: true },
    { op: "set_track_input", track_id: "a", armed: true },
  ]);
  await expect(trackControls(page, "Verb").getByRole("button")).toHaveCount(2);
  await expect(trackControls(page, "Group").getByRole("button")).toHaveCount(0);
  // The buttons and the name follow the model: pressed, and the name dimmed when muted.
  const muted = abc().map((t) => (t.id === "a" ? { ...t, mixer: { ...t.mixer, mute: true }, input: { ...t.input, armed: true } } : t));
  await answer(page, "project_view", view(2, project(muted)));
  await emit(page, "session-changed", {
    version: 2,
    kind: "command",
    label: "Mute",
    source: "gui",
    changes: { created: [], updated: [], deleted: [], flags: ["mixer"], structure_changed: false },
  });
  await expect(aButtons.getByRole("button", { name: "Mute" })).toHaveAttribute("aria-pressed", "true");
  await expect(aButtons.getByRole("button", { name: "Arm for recording" })).toHaveAttribute("aria-pressed", "true");
  await expect(aButtons.getByRole("button", { name: "Solo" })).toHaveAttribute("aria-pressed", "false");
  await expect(a).toHaveClass(/muted/);
});

test("tall rows show the detail line; compact rows do not", async ({ page }) => {
  const synth = { ...track("k", "Keys"), mixer: { volume_db: -6, pan: -0.3, mute: false, solo: false, solo_safe: false } };
  await openHeaders(page, project([synth]));
  await expect(row(page, "Keys").locator(".detail")).toHaveText("MIDI  ·  -6.0 dB  ·  L30");
  await page.getByRole("combobox", { name: /Height|Track height/ }).selectOption("S");
  await expect(row(page, "Keys").locator(".detail")).toHaveCount(0);
});

test("a click selects, ⌘ adds, a click below the rows clears", async ({ page }) => {
  await openHeaders(page, project(abc()));
  await row(page, "A").locator(".name").click();
  await expect(row(page, "A")).toHaveClass(/selected/);
  await row(page, "B").locator(".name").click({ modifiers: ["Meta"] });
  await expect(row(page, "A")).toHaveClass(/selected/);
  await expect(row(page, "B")).toHaveClass(/selected/);
  const headers = await box(page);
  await page.mouse.click(headers.x + 60, headers.y + ROW * 3 + 20);
  await expect(page.locator(".header.selected")).toHaveCount(0);
});

test("a vertical drag reorders with an insertion line", async ({ page }) => {
  await openHeaders(page, project(abc()));
  const headers = await box(page);
  // Drag C above A.
  await page.mouse.move(headers.x + 60, headers.y + ROW * 2.5);
  await page.mouse.down();
  await page.mouse.move(headers.x + 60, headers.y + ROW * 2.5 - 3);
  await expect(page.getByTestId("insertion")).toHaveCount(0);
  await page.mouse.move(headers.x + 60, headers.y + ROW, { steps: 3 });
  await page.mouse.move(headers.x + 60, headers.y + 4, { steps: 3 });
  await expect(page.getByTestId("insertion")).toBeVisible();
  await page.mouse.up();
  await expect.poll(() => edits(page)).toEqual([{ op: "move_track", track_id: "c", index: 0 }]);
  await expect(page.getByTestId("insertion")).toHaveCount(0);
  // A drag that ends where it started moves nothing; the press selected the track.
  await page.mouse.move(headers.x + 60, headers.y + ROW * 0.5);
  await page.mouse.down();
  await page.mouse.move(headers.x + 60, headers.y + ROW * 0.9, { steps: 3 });
  await page.mouse.up();
  await expect(row(page, "A")).toHaveClass(/selected/);
  expect(await edits(page)).toHaveLength(1);
});

test("a double-click on the name renames in place; Enter commits, Esc cancels", async ({ page }) => {
  await openHeaders(page, project(abc()));
  await row(page, "B").locator(".name").dblclick();
  const editor = page.getByRole("textbox", { name: "Track name" });
  await expect(editor).toBeFocused();
  await expect(editor).toHaveValue("B");
  await editor.fill("Bassline");
  await editor.press("Enter");
  await expect.poll(() => edits(page)).toEqual([{ op: "rename_track", track_id: "b", name: "Bassline" }]);
  await expect(editor).toHaveCount(0);
  // Esc cancels; an unchanged or empty name commits nothing; focus loss commits.
  await row(page, "A").locator(".name").dblclick();
  await editor.fill("Other");
  await editor.press("Escape");
  await row(page, "A").locator(".name").dblclick();
  await editor.fill("   ");
  await editor.press("Enter");
  await row(page, "C").locator(".name").dblclick();
  await editor.fill("Chords");
  await page.getByRole("application", { name: "Arrangement" }).click();
  await expect.poll(() => edits(page)).toEqual([
    { op: "rename_track", track_id: "b", name: "Bassline" },
    { op: "rename_track", track_id: "c", name: "Chords" },
  ]);
});

test("the context menu: rename, colour, devices, a clip at the playhead, import, delete", async ({ page }) => {
  const coloured = { ...track("k", "Keys"), color: "#112233" };
  await openHeaders(page, project([coloured, track("a", "Vox", "audio")]));
  await row(page, "Keys").click({ button: "right" });
  const menu = page.getByRole("menu", { name: "Keys" });
  await expect(menu.getByRole("menuitem")).toHaveText([
    "Rename…",
    "Colour…",
    "Automatic Colour",
    "Show Devices",
    "Add MIDI Clip at Playhead",
    "Delete Track",
  ]);
  await menu.getByRole("menuitem", { name: "Automatic Colour" }).click();
  // The buttons sit in a layer of their own; a right-click on one is still the row's.
  await trackControls(page, "Keys").getByRole("button", { name: "Mute" }).click({ button: "right" });
  await expect(page.getByRole("menu", { name: "Keys" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu", { name: "Keys" })).toHaveCount(0);
  await row(page, "Vox").click({ button: "right" });
  await expect(page.getByRole("menu", { name: "Vox" }).getByRole("menuitem")).toHaveText([
    "Rename…",
    "Colour…",
    "Show Devices",
    "Import Audio…",
    "Delete Track",
  ]);
  await page.getByRole("menuitem", { name: "Import Audio…" }).click();
  // The playhead at beat 5: the clip fills its bar, 4–8.
  await page.getByRole("application", { name: "Arrangement" }).focus();
  const ruler = await page.getByLabel("Ruler").boundingBox();
  if (!ruler) throw new Error("no ruler");
  await page.mouse.click(ruler.x + 5 * 24, ruler.y + 36);
  await row(page, "Keys").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Add MIDI Clip at Playhead" }).click();
  // Colour… opens the colour panel; its choice sets the colour.
  await row(page, "Vox").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Colour…" }).click();
  await page.locator("input.picker").evaluate((input: HTMLInputElement) => {
    input.value = "#a1b2c3";
    input.dispatchEvent(new Event("change", { bubbles: true }));
  });
  await row(page, "Vox").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Delete Track" }).click();
  await expect.poll(() => edits(page)).toEqual([
    { op: "set_track_color", track_id: "k", color: null },
    { op: "create_midi_clip", track_id: "k", start: 4, length: 4, name: "Keys" },
    { op: "set_track_color", track_id: "a", color: "#a1b2c3" },
    { op: "delete_track", track_id: "a" },
  ]);
  const imports = (await calls(page)).filter((c) => c.cmd === "import_audio_dialog");
  expect(imports.map((c) => c.args)).toEqual([{ trackId: "a", startBeat: 0 }]);
  // Rename… starts the inline editor; Show Devices raises the Devices tab.
  await row(page, "Keys").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Rename…" }).click();
  await expect(page.getByRole("textbox", { name: "Track name" })).toBeFocused();
  await page.keyboard.press("Escape");
  await row(page, "Keys").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Show Devices" }).click();
  await expect(page.getByRole("tab", { name: "Devices" })).toHaveAttribute("aria-selected", "true");
  await expect(row(page, "Keys")).toHaveClass(/selected/);
});

test("the wheel scrolls the headers with the canvas", async ({ page }) => {
  const many = Array.from({ length: 30 }, (_, i) => track(`t${i}`, `T${i}`));
  await openHeaders(page, project(many));
  const headers = await box(page);
  const first = () => row(page, "T0").evaluate((element) => element.getBoundingClientRect().top);
  const before = await first();
  await page.mouse.move(headers.x + 60, headers.y + 100);
  await page.mouse.wheel(0, 120);
  await expect.poll(first).toBeLessThan(before - 100);
});

test("an empty project shows an empty, inert track list", async ({ page }) => {
  await openHeaders(page, project([]));
  await expect(trackList(page)).toBeVisible();
  const headers = await box(page);
  // A click on the empty list clears the selection, which is already empty: no edit goes out.
  await page.mouse.click(headers.x + 60, headers.y + 40);
  await page.keyboard.press("ArrowDown");
  expect(await edits(page)).toEqual([]);
  await expect(rows(page)).toHaveCount(0);
});

test("a large project keeps every header, in order, one selectable row each", async ({ page }) => {
  const many = Array.from({ length: 200 }, (_, i) => track(`t${i}`, `T${i}`));
  await openHeaders(page, project(many));
  await expect(rows(page)).toHaveCount(200);
  await expect(rows(page).first()).toContainText("T0");
  await expect(rows(page).last()).toContainText("T199");
  await row(page, "T0").click();
  await expect(row(page, "T0")).toHaveClass(/selected/);
});
