import { expect, type Page, test } from "@playwright/test";
import fixture from "../fixtures/dialogs-v1.json" with { type: "json" };
import { frameHex } from "../support/frame";
import { pluginMap, defaults, plugin } from "../support/plugins";
import { midiClip, project, track, view } from "../support/project";
import { trackList, trackRow } from "../support/tracks";
import type { DeviceView, OpenProject } from "../../src/lib/types";
import { AUDIO_SUFFIXES, DATA_LAYER, mockTauri, sendChannel } from "./tauri-mock";

// HiDPI of (P): every canvas of the window draws at the display's device
// pixel ratio, on a 1× and a 2× display, and again at the new ratio when the window moves to a
// display with another scale (src/lib/hidpi.ts). The prototype's `logo_pixmap_respects_device_
// pixel_ratio` maps here. With SONARA_SHOTS_DIR set, the test also saves the
// screenshots of the owner's review: the main window, the mixer, the piano roll and a device card,
// at 1× and 2×.

declare const process: { env: Record<string, string | undefined> };
const SHOTS = process.env.SONARA_SHOTS_DIR;

test.use({ actionTimeout: 10_000, viewport: { width: 1440, height: 900 } });

function eq(id: string): DeviceView {
  return {
    id,
    plugin_id: "sonara.eq.parametric",
    name: plugin("sonara.eq.parametric").name,
    enabled: true,
    preset: null,
    sidechain_id: null,
    available: true,
    params: defaults("sonara.eq.parametric"),
  };
}

/** Drums 1 and Bass with MIDI clips, Vox with an EQ, and every built-in plugin's descriptor. */
function band(): OpenProject {
  const synth: DeviceView = {
    id: "dev_synth",
    plugin_id: "sonara.synth.poly",
    name: plugin("sonara.synth.poly").name,
    enabled: true,
    preset: null,
    sidechain_id: null,
    available: true,
    params: defaults("sonara.synth.poly"),
  };
  return project(
    [
      { ...track("d", "Drums 1", "instrument", [midiClip("d1", 0, 4, { name: "Beat" }), midiClip("d2", 4, 4, { name: "Fill" })]), instrument: synth },
      { ...track("b", "Bass", "instrument", [midiClip("b1", 4, 8, { name: "Riff" })]), instrument: { ...synth, id: "dev_bass" } },
      { ...track("v", "Vox", "audio"), inserts: [eq("dev_eq")] },
    ],
    { meter_layout: 5, plugins: pluginMap() },
  );
}

interface OracleItem {
  text: string;
  plugin: string | null;
  preset: string | null;
  tooltip: string;
  children: OracleItem[];
}

/** The Browser's tree of the prototype (tests/fixtures/dialogs-v1.json), as sonara-app sends it. */
function browserTree() {
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

/** A flat EQ curve over the prototype's 240 points. */
function flatCurve() {
  const freqs = Array.from({ length: 240 }, (_, i) => 20 * 1000 ** (i / 239));
  return { freqs, db: freqs.map(() => 0), bands: [] };
}

async function openWindow(page: Page) {
  const open = band();
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: {
      agent: { on: false, address: null },
      messages: [],
      panes: {
        browser: { size: 240, open: true },
        inspector: { size: 280, open: true },
        editors: { size: 470, open: true },
      },
      audio_suffixes: AUDIO_SUFFIXES,
    },
    recoverable_sessions: [],
    project_view: view(1, open),
    edit: { committed: true, version: 2, label: "Edit", changes: { created: [], updated: [], deleted: [], flags: [], structure_changed: false }, result: {} },
    seal_undo: null,
    eq_response: flatCurve(),
    browser_devices: browserTree(),
  });
  await page.goto("/");
  await expect(trackList(page).getByRole("option")).toHaveCount(open.tracks.length);
  // The meters show a level, so that their drawing is in the screenshots (the transport stopped).
  await sendChannel(page, frameHex({ state: 0, meterLayout: 5, meters: [[0.5, 0.45, 0.3, 0.28], [0.25, 0.3, 0.15, 0.18], [0, 0, 0, 0], [0.6, 0.55, 0.35, 0.3]] }));
}

interface CanvasSize {
  name: string;
  width: number;
  height: number;
  cssWidth: number;
  cssHeight: number;
}

/** Every shown canvas of the page: its backing store and its CSS size. */
function canvases(page: Page): Promise<CanvasSize[]> {
  return page.evaluate(() =>
    [...document.querySelectorAll("canvas")]
      .map((canvas) => {
        const box = canvas.getBoundingClientRect();
        const name = canvas.dataset.testid
          ? canvas.dataset.testid
          : canvas.dataset.meter
            ? `meter:${canvas.dataset.meter}`
            : canvas.classList.contains("curve")
              ? "eq-curve"
              : (canvas.getAttribute("aria-label") ?? canvas.className);
        return { name, width: canvas.width, height: canvas.height, cssWidth: box.width, cssHeight: box.height };
      })
      .filter((c) => c.cssWidth > 0 && c.cssHeight > 0),
  );
}

/** Each canvas's backing store is its CSS size times `dpr`, within the rounding of a pixel. */
async function expectScale(page: Page, dpr: number, names: string[]) {
  await expect
    .poll(async () => {
      const found = await canvases(page);
      const wrong = found.filter((c) => Math.abs(c.width - c.cssWidth * dpr) > 1 || Math.abs(c.height - c.cssHeight * dpr) > 1);
      const missing = names.filter((name) => !found.some((c) => c.name === name));
      return { wrong, missing };
    })
    .toEqual({ wrong: [], missing: [] });
}

async function shot(page: Page, name: string, dpr: number, element?: string) {
  if (!SHOTS) return;
  await page.waitForTimeout(300);
  const path = `${SHOTS}/${name}-${dpr}x.png`;
  if (element) await page.locator(element).first().screenshot({ path });
  else await page.screenshot({ path });
}

for (const dpr of [1, 2]) {
  test.describe(`at ${dpr}x`, () => {
    test.use({ deviceScaleFactor: dpr });

    test(`every canvas draws at ${dpr} device pixels a CSS pixel: the timeline, the meters, the piano roll and the EQ curve`, async ({ page }) => {
      await openWindow(page);
      await expectScale(page, dpr, ["arrangement-canvas", "Ruler", "meter:d", "meter:master"]);
      await shot(page, "main", dpr);
      await shot(page, "mixer", dpr, 'section[aria-label="Editors"]');
      // The piano roll of Riff.
      await page.getByRole("application", { name: "Arrangement" }).focus();
      for (let i = 0; i < 3; i++) await page.keyboard.press("ArrowDown");
      await page.getByRole("tab", { name: "Piano Roll" }).click();
      await expect(page.getByRole("listbox", { name: "Notes" }).getByRole("option")).toHaveCount(4);
      await expectScale(page, dpr, ["note-grid-canvas", "Keyboard", "Piano roll ruler", "Velocity lane"]);
      await shot(page, "piano-roll", dpr, 'section[aria-label="Editors"]');
      // The EQ's card on Vox.
      await trackRow(page, "Vox").locator(".name").click();
      await page.getByRole("tab", { name: "Devices" }).click();
      const card = page.getByRole("group", { name: "Device Parametric EQ", exact: true });
      await expect(card).toBeVisible();
      await expectScale(page, dpr, ["eq-curve"]);
      await shot(page, "device-card", dpr, '[aria-label="Device Parametric EQ"]');
    });
  });
}

test("a move to a display with another scale redraws every canvas at the new ratio", async ({ page }) => {
  // The window starts on a 1× display; the page's scale queries are kept so that the test can
  // move it, as the system does, by changing the ratio and telling the queries.
  await page.addInitScript(() => {
    const queries: MediaQueryList[] = [];
    let ratio = 1;
    Object.defineProperty(window, "devicePixelRatio", { configurable: true, get: () => ratio });
    const original = window.matchMedia.bind(window);
    window.matchMedia = (query: string) => {
      const list = original(query);
      if (query.includes("resolution")) queries.push(list);
      return list;
    };
    (window as unknown as { __moveToScale: (next: number) => void }).__moveToScale = (next: number) => {
      ratio = next;
      for (const list of queries.splice(0)) list.dispatchEvent(new Event("change"));
    };
  });
  await openWindow(page);
  await expectScale(page, 1, ["arrangement-canvas", "Ruler", "meter:d", "meter:master"]);
  await page.evaluate(() => (window as unknown as { __moveToScale: (next: number) => void }).__moveToScale(2));
  await expectScale(page, 2, ["arrangement-canvas", "Ruler", "meter:d", "meter:master"]);
  await page.evaluate(() => (window as unknown as { __moveToScale: (next: number) => void }).__moveToScale(1));
  await expectScale(page, 1, ["arrangement-canvas", "Ruler", "meter:d", "meter:master"]);
});
