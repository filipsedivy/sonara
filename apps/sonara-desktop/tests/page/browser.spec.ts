import { expect, type Page, test } from "@playwright/test";
import fixture from "../fixtures/dialogs-v1.json" with { type: "json" };
import { audioClip, project, track, view } from "../support/project";
import type { OpenProject } from "../../src/lib/types";
import { AUDIO_SUFFIXES, DATA_LAYER, PANES, answer, calls, mockTauri } from "./tauri-mock";
import { trackRow } from "../support/tracks";

// The Browser (–BR-05) against mocked commands in WebKit: the Devices
// tab on the prototype's tree (tests/fixtures/dialogs-v1.json), its search and its actions, and the
// Files tab's listing, double-click import and drags onto the timeline.

const PPB = 24;
const ROW = 56;

interface OracleItem {
  text: string;
  plugin: string | null;
  preset: string | null;
  tooltip: string;
  children: OracleItem[];
}

function devices() {
  const convert = (item: OracleItem, kind: string): unknown => ({
    text: item.text,
    plugin_id: item.plugin,
    preset: item.preset,
    kind: item.plugin ? kind : null,
    tooltip: item.tooltip,
    children: item.children.map((child) => convert(child, kind)),
  });
  const [instruments, effects] = fixture.browser as OracleItem[];
  return [convert(instruments, "instrument"), convert(effects, "effect")];
}

const LISTINGS: Record<string, unknown> = {
  "/Users/me/Music": [
    { name: "drop.wav", path: "/Users/me/Music/drop.wav", folder: false },
    { name: "Loops", path: "/Users/me/Music/Loops", folder: true },
    { name: "take.flac", path: "/Users/me/Music/take.flac", folder: false },
  ],
  "/Users/me/Music/Loops": [{ name: "beat.wav", path: "/Users/me/Music/Loops/beat.wav", folder: false }],
};

function outcome(result: Record<string, unknown>) {
  return {
    committed: true,
    version: 2,
    label: "Edit",
    changes: { created: [], updated: [], deleted: [], flags: [], structure_changed: false },
    result,
  };
}

async function openBrowser(page: Page, open: OpenProject, extra: Record<string, unknown> = {}) {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: { agent: { on: false, address: null }, messages: [], panes: PANES, audio_suffixes: AUDIO_SUFFIXES },
    recoverable_sessions: [],
    project_view: view(1, open),
    browser_devices: devices(),
    browser_root: "/Users/me/Music",
    list_directory: LISTINGS["/Users/me/Music"],
    import_audio_files: "op_1",
    edit: outcome({ track_id: "new_track" }),
    seal_undo: null,
    ...extra,
  });
  await page.goto("/");
  await expect(browser(page).getByRole("tree")).toBeVisible();
}

const browser = (page: Page) => page.getByRole("complementary", { name: "Browser" });
const item = (page: Page, name: string) => browser(page).getByRole("treeitem", { name, exact: true });

async function edits(page: Page): Promise<unknown[]> {
  return (await calls(page)).filter((c) => c.cmd === "edit").map((c) => (c.args as { edit: unknown }).edit);
}

async function imports(page: Page): Promise<unknown[]> {
  return (await calls(page)).filter((c) => c.cmd === "import_audio_files").map((c) => c.args);
}

test("the Devices tab lists instruments and effects; the search keeps matches and their parents", async ({ page }) => {
  await openBrowser(page, project([track("t", "Synth")]));
  const tree = browser(page).getByRole("tree", { name: "Instruments and effects" });
  await expect(tree.getByRole("treeitem").locator(".text")).toHaveText([
    "Instruments",
    "Sonara Drum Kit",
    "Sonara Poly Synth",
    "Effects",
    "Delay",
    "Distortion",
    "Dynamics",
    "EQ",
    "Filter",
    "Reverb",
    "Utility",
  ]);
  await expect(item(page, "Sonara Poly Synth")).toHaveAttribute("title", /Polyphonic subtractive synthesizer/);
  await browser(page).getByRole("searchbox", { name: "Search" }).fill("pad");
  await expect(tree.getByRole("treeitem").locator(".text")).toHaveText(["Instruments", "Sonara Poly Synth", "Warm Pad", "Choir Pad"]);
  await browser(page).getByRole("searchbox", { name: "Search" }).fill("");
  await expect(item(page, "Warm Pad")).toHaveCount(0);
});

test("an instrument or its preset makes a track and selects it", async ({ page }) => {
  await openBrowser(page, project([track("t", "Synth")]));
  await item(page, "Sonara Poly Synth").locator(".twisty").click();
  await item(page, "Warm Pad").dblclick();
  await item(page, "Sonara Drum Kit").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Create Instrument Track" }).click();
  await expect.poll(() => edits(page)).toEqual([
    { op: "create_track", name: "Warm Pad", kind: "instrument", instrument: "sonara.synth.poly", preset: "Warm Pad" },
    { op: "create_track", name: "Sonara Drum Kit", kind: "instrument", instrument: "sonara.drums", preset: null },
  ]);
});

test("an effect goes on the current track and shows its device; without one the browser says so", async ({ page }) => {
  await openBrowser(page, project([track("t", "Synth")]), { edit: outcome({ device_id: "dev_new" }) });
  await item(page, "Delay").first().locator(".twisty").click();
  await item(page, "Delay").nth(1).dblclick();
  await expect(page.getByRole("alert")).toHaveText("select a track to add the effect to");
  expect(await edits(page)).toEqual([]);
  await trackRow(page, "Synth").locator(".name").click();
  await item(page, "Delay").nth(1).dblclick();
  await item(page, "Delay").nth(1).locator(".twisty").click();
  await item(page, "Dub Echo").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Add to Selected Track" }).click();
  await expect.poll(() => edits(page)).toEqual([
    { op: "add_device", track_id: "t", plugin_id: "sonara.delay" },
    { op: "add_device", track_id: "t", plugin_id: "sonara.delay", preset: "Dub Echo" },
  ]);
  await expect(page.getByRole("tab", { name: "Devices" })).toHaveAttribute("aria-selected", "true");
});

test("the Files tab lists the audio files and folders; a double-click imports into a new track at beat 0", async ({ page }) => {
  await openBrowser(page, project([track("t", "Synth")]));
  await browser(page).getByRole("button", { name: "Files" }).click();
  const files = browser(page).getByRole("tree", { name: "Audio files" });
  await expect(files.getByRole("treeitem").locator(".text")).toHaveText(["drop.wav", "Loops", "take.flac"]);
  await answer(page, "list_directory", LISTINGS["/Users/me/Music/Loops"]);
  await item(page, "Loops").locator(".twisty").click();
  await expect(files.getByRole("treeitem").locator(".text")).toHaveText(["drop.wav", "Loops", "beat.wav", "take.flac"]);
  await item(page, "beat.wav").dblclick();
  await expect.poll(() => imports(page)).toEqual([{ paths: ["/Users/me/Music/Loops/beat.wav"], trackId: null, startBeat: 0 }]);
  expect(
    (await calls(page)).filter((c) => c.cmd === "list_directory").map((c) => c.args),
  ).toEqual([{ path: "/Users/me/Music" }, { path: "/Users/me/Music/Loops" }]);
  // ⌘ adds to the selection, ⇧ selects a range.
  await item(page, "drop.wav").click();
  await item(page, "take.flac").click({ modifiers: ["Meta"] });
  await expect(files.locator('[aria-selected="true"] .text')).toHaveText(["drop.wav", "take.flac"]);
  await item(page, "drop.wav").click();
  await item(page, "beat.wav").click({ modifiers: ["Shift"] });
  await expect(files.locator('[aria-selected="true"] .text')).toHaveText(["drop.wav", "Loops", "beat.wav"]);
});

test("files dragged from the Files tab import where they are dropped", async ({ page }) => {
  const tracks = [track("au", "Audio", "audio", [audioClip("a1", "ast_1", 40, 4)]), track("t", "Synth")];
  await openBrowser(page, project(tracks));
  await browser(page).getByRole("button", { name: "Files" }).click();
  await item(page, "drop.wav").click();
  await item(page, "take.flac").click({ modifiers: ["Meta"] });
  const canvas = await page.getByRole("application", { name: "Arrangement" }).boundingBox();
  if (!canvas) throw new Error("no canvas");
  const from = await item(page, "drop.wav").boundingBox();
  if (!from) throw new Error("no file");
  const drag = async (x: number, y: number) => {
    await page.mouse.move(from.x + 20, from.y + 10);
    await page.mouse.down();
    await page.mouse.move(from.x + 40, from.y + 30, { steps: 3 });
    await page.mouse.move(x, y, { steps: 5 });
    await page.mouse.up();
  };
  // Onto the audio track at the snapped beat; onto the instrument track: new tracks there.
  await drag(canvas.x + 4.2 * PPB, canvas.y + ROW / 2);
  await drag(canvas.x + 6.6 * PPB, canvas.y + ROW * 1.5);
  // Back onto the browser: nothing.
  await drag(from.x + 30, from.y + 60);
  await expect.poll(() => imports(page)).toEqual([
    { paths: ["/Users/me/Music/drop.wav", "/Users/me/Music/take.flac"], trackId: "au", startBeat: 4 },
    { paths: ["/Users/me/Music/drop.wav", "/Users/me/Music/take.flac"], trackId: null, startBeat: 6.5 },
  ]);
});
