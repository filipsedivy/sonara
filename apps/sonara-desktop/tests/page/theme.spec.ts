import { expect, type Page, test } from "@playwright/test";
import fixture from "../fixtures/theme-v1.json" with { type: "json" };
import { midiClip, project, track, view } from "../support/project";
import { trackList } from "../support/tracks";
import { AUDIO_SUFFIXES, DATA_LAYER, emit, mockTauri } from "./tauri-mock";

// The two themes (owner, 2026-10-24): the stored `theme` setting is applied to the
// panels at startup and live when the Settings dialog changes it, with the prototype's palettes
// (tests/fixtures/theme-v1.json); the canvases stay dark in both.

test.use({ viewport: { width: 1440, height: 900 } });

const rgb = (hex: string) => {
  const n = Number.parseInt(hex.slice(1), 16);
  return `rgb(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255})`;
};

async function openWindow(page: Page, stored: string | null) {
  await mockTauri(page, {
    ...DATA_LAYER,
    ...(stored === null ? {} : { gui_theme: stored }),
    shell_status: { agent: { on: false, address: null }, messages: [], audio_suffixes: AUDIO_SUFFIXES },
    recoverable_sessions: [],
    project_view: view(1, project([track("d", "Drums 1", "instrument", [midiClip("d1", 0, 4)]), track("v", "Vox", "audio")])),
  });
  await page.goto("/");
  await expect(trackList(page).getByRole("option")).toHaveCount(2);
}

/** The colours the page shows: the window, the status bar, the first header row, the transport bar's text. */
function shown(page: Page) {
  return page.evaluate(() => {
    const style = (selector: string) => getComputedStyle(document.querySelector(selector) as Element);
    return {
      theme: document.documentElement.dataset.theme,
      window: getComputedStyle(document.body).backgroundColor,
      statusBar: style(".status-bar").backgroundColor,
      text: getComputedStyle(document.body).color,
      scheme: document.documentElement.style.colorScheme,
    };
  });
}

/** The ruler canvas's pixel in the middle of its empty right end, which stays dark in both themes. */
function rulerPixel(page: Page) {
  return page.evaluate(() => {
    const canvas = document.querySelector<HTMLCanvasElement>("canvas.ruler");
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return null;
    return [...ctx.getImageData(canvas.width - 4, Math.floor(canvas.height / 4), 1, 1).data.slice(0, 3)];
  });
}

test("a stored light theme lights the panels at startup, and the canvases stay dark", async ({ page }) => {
  await openWindow(page, "light");
  const light = fixture.palettes.light;
  await expect.poll(() => shown(page)).toEqual({
    theme: "light",
    window: rgb(light.window),
    statusBar: rgb(light.panel),
    text: rgb(light.text),
    scheme: "light",
  });
  const pixel = await rulerPixel(page);
  expect(pixel).not.toBeNull();
  expect(Math.max(...(pixel ?? [255]))).toBeLessThan(90);
});

test("the dark theme is the default, and a change of the setting applies at once", async ({ page }) => {
  await openWindow(page, null);
  const dark = fixture.palettes.dark;
  await expect.poll(() => shown(page)).toEqual({
    theme: "dark",
    window: rgb(dark.window),
    statusBar: "rgb(32, 34, 38)",
    text: rgb(dark.text),
    scheme: "dark",
  });
  // The Settings dialog's OK: the desktop tells every window (`theme`).
  await emit(page, "theme", "light");
  await expect.poll(async () => (await shown(page)).window).toBe(rgb(fixture.palettes.light.window));
  await emit(page, "theme", "solarized");
  await expect.poll(async () => (await shown(page)).theme).toBe("dark");
  const pixel = await rulerPixel(page);
  expect(Math.max(...(pixel ?? [255]))).toBeLessThan(90);
});
