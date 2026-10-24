import { expect, type Page, test } from "@playwright/test";
import { project, track, view } from "../support/project";
import { AUDIO_SUFFIXES, DATA_LAYER, PANES, calls, emit, mockTauri } from "./tauri-mock";

// The window shell (inventory MW "Layout") against mocked commands: the panes
// with their saved sizes and the View menu's toggles, the Editors' tabs, and the status bar's job
// label and progress.

async function openWindow(page: Page, panes = PANES) {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: { agent: { on: true, address: "/tmp/s.sock" }, messages: [], panes, audio_suffixes: AUDIO_SUFFIXES },
    recoverable_sessions: [],
    project_view: view(1, project([track("t", "Synth")])),
  });
  await page.goto("/");
  await expect(page.getByTestId("timeline-info")).not.toHaveText("");
}

async function savedPanes(page: Page): Promise<unknown[]> {
  return (await calls(page)).filter((c) => c.cmd === "set_pane_layout").map((c) => (c.args as { panes: unknown }).panes);
}

test("the panes open at their saved sizes and the View menu shows and hides them", async ({ page }) => {
  await openWindow(page, { ...PANES, browser: { size: 300, open: true }, inspector: { size: 280, open: false } });
  const browser = page.getByRole("complementary", { name: "Browser" });
  await expect(browser).toBeVisible();
  expect((await browser.boundingBox())?.width).toBe(300);
  await expect(page.getByRole("complementary", { name: "Inspector" })).toHaveCount(0);
  await emit(page, "view-command", { command: "toggle-browser" });
  await emit(page, "view-command", { command: "toggle-inspector" });
  await expect(browser).toHaveCount(0);
  await expect(page.getByRole("complementary", { name: "Inspector" })).toBeVisible();
  await expect.poll(() => savedPanes(page)).toEqual([
    {
      browser: { size: 300, open: false },
      inspector: { size: 280, open: true },
      editors: { size: 300, open: true },
    },
  ]);
});

test("a splitter resizes its pane, and the View menu raises an editor tab", async ({ page }) => {
  await openWindow(page);
  const splitter = page.getByRole("separator", { name: "Editors height" });
  await splitter.focus();
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("ArrowUp");
  await expect.poll(async () => (await savedPanes(page)).at(-1)).toEqual({ ...PANES, editors: { size: 332, open: true } });
  await emit(page, "view-command", { command: "devices" });
  await expect(page.getByRole("tab", { name: "Devices" })).toHaveAttribute("aria-selected", "true");
  await emit(page, "view-command", { command: "toggle-editors" });
  await expect(page.getByRole("tablist", { name: "Editors" })).toHaveCount(0);
  await emit(page, "view-command", { command: "piano-roll" });
  await expect(page.getByRole("tab", { name: "Piano Roll" })).toHaveAttribute("aria-selected", "true");
});

test("the status bar shows a running job and its progress, until it finishes", async ({ page }) => {
  await openWindow(page);
  const job = {
    id: "op_1",
    kind: "import_audio",
    label: "Import 2 files",
    status: "running",
    finished: false,
    progress: 0.5,
    message: "drop.wav",
    result: null,
    error: null,
  };
  await emit(page, "job-changed", job);
  await expect(page.getByTestId("job-label")).toHaveText("Import 2 files — drop.wav");
  await expect(page.getByRole("progressbar", { name: "Import 2 files" })).toHaveAttribute("value", "50");
  await emit(page, "job-changed", { ...job, status: "succeeded", finished: true, progress: 1 });
  await expect(page.getByTestId("job-label")).toHaveCount(0);
  await emit(page, "message", { text: "Imported drop.wav", duration_ms: 4000, error: false });
  await expect(page.getByRole("status")).toHaveText("Imported drop.wav");
  await expect(page.getByTestId("agent-label")).toHaveText("agent: on");
});
