import { expect, type Page, test } from "@playwright/test";
import fixture from "../fixtures/live-frame-v1.json" with { type: "json" };
import { project, track, view } from "../support/project";
import type { OpenProject } from "../../src/lib/types";
import { AUDIO_SUFFIXES, DATA_LAYER, calls, mockTauri, sendChannel } from "./tauri-mock";

// The transport bar (–TB-09) against mocked commands. The prototype
// has no tests of its own for it; these follow the rows.

const STOPPED = {
  was_playing: true,
  takes: null,
  message: null,
  status: { stopped: true, was_playing: true },
};

async function openBar(page: Page, open: OpenProject | null, extra: Record<string, unknown> = {}) {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: { agent: { on: false, address: null }, messages: [], audio_suffixes: AUDIO_SUFFIXES },
    recoverable_sessions: [],
    project_view: view(1, open),
    edit: { committed: true, version: 2, label: "Edit", changes: { created: [], updated: [], deleted: [], flags: [], structure_changed: false }, result: {} },
    transport_play: { state: "playing", position_beat: 0 },
    transport_stop: STOPPED,
    transport_seek: { position_beat: 0 },
    transport_record: { recording: true },
    ...extra,
  });
  await page.goto("/");
  if (open) await expect(page.getByTestId("timeline-info")).not.toHaveText("");
}

async function transportCalls(page: Page): Promise<[string, unknown][]> {
  return (await calls(page))
    .filter((c) => c.cmd.startsWith("transport_") && c.cmd !== "transport_status")
    .map((c) => [c.cmd, c.args]);
}

async function edits(page: Page): Promise<unknown[]> {
  return (await calls(page)).filter((c) => c.cmd === "edit").map((c) => (c.args as { edit: unknown }).edit);
}

const frame = (name: string) => {
  const found = fixture.frames.find((f) => f.name === name);
  if (!found) throw new Error(`no frame ${name}`);
  return found.hex;
};

test("Play and Space toggle the transport; a second Stop returns to the last start", async ({ page }) => {
  await openBar(page, project([track("t", "Synth")]));
  const play = page.getByTestId("play");
  await play.click();
  await expect(play).toHaveAttribute("aria-pressed", "true");
  await page.keyboard.press("Space");
  await expect(play).toHaveAttribute("aria-pressed", "false");
  await page.getByRole("button", { name: "Stop" }).click();
  await expect.poll(() => transportCalls(page)).toEqual([
    ["transport_play", { startBeat: 0, looped: false }],
    ["transport_stop", {}],
    ["transport_seek", { beat: 0 }],
  ]);
});

test("Home moves the playhead to the start, and seeks once a transport exists", async ({ page }) => {
  await openBar(page, project([track("t", "Synth")]));
  const ruler = await page.locator("canvas.ruler").boundingBox();
  if (!ruler) throw new Error("no ruler");
  await page.mouse.click(ruler.x + 8 * 24, ruler.y + 36);
  await expect(page.getByTestId("position")).toHaveText("3.1.000");
  await expect(page.getByTestId("time")).toHaveText("00:04.000");
  await page.keyboard.press("Home");
  await expect(page.getByTestId("position")).toHaveText("1.1.000");
  expect(await transportCalls(page)).toEqual([]);
  // A transport (the frame's engine is no longer absent): Home seeks.
  await sendChannel(page, frame("idle"));
  await page.getByRole("button", { name: "Return to Start" }).click();
  await expect.poll(() => transportCalls(page)).toEqual([["transport_seek", { beat: 0 }]]);
});

test("Record records from the playhead and a refusal unchecks it", async ({ page }) => {
  await openBar(page, project([track("t", "Synth")]), {
    transport_record: { reject: { code: "INVALID_ARGUMENT", message: "no armed audio or instrument tracks to record" } },
  });
  const record = page.getByRole("button", { name: "Record", exact: true });
  await page.keyboard.press("r");
  await expect(page.getByRole("alert")).toHaveText("no armed audio or instrument tracks to record");
  await expect(record).toHaveAttribute("aria-pressed", "false");
  expect(await transportCalls(page)).toEqual([["transport_record", { startBeat: 0 }]]);
});

test("Loop, Metronome, the count-in, the tempo and the signature edit the project", async ({ page }) => {
  await openBar(page, project([track("t", "Synth")]));
  await page.keyboard.press("l");
  await page.getByRole("button", { name: "Metronome" }).click();
  await page.getByRole("combobox", { name: "Count-in" }).selectOption("2");
  const tempo = page.getByRole("spinbutton", { name: "Tempo" });
  await tempo.fill("97.456");
  await tempo.press("Enter");
  const signature = page.getByRole("combobox", { name: "Time signature" });
  await signature.fill("7/8");
  await signature.press("Enter");
  await expect.poll(() => edits(page)).toEqual([
    { op: "set_transport", loop_enabled: true },
    { op: "set_transport", metronome: true },
    { op: "set_transport", count_in_bars: 2 },
    { op: "set_tempo", bpm: 97.46 },
    { op: "set_time_signature", numerator: 7, denominator: 8 },
  ]);
  // Text that is not "n/d" is refused and the field shows the project's again.
  await signature.fill("waltz");
  await signature.press("Enter");
  await expect(page.getByRole("alert")).toHaveText("invalid time signature 'waltz'");
  await expect(signature).toHaveValue("4/4");
  expect(await edits(page)).toHaveLength(5);
});

test("the single-key shortcuts never fire while a text field has focus", async ({ page }) => {
  await openBar(page, project([track("t", "Synth")]));
  await page.getByRole("spinbutton", { name: "Tempo" }).focus();
  await page.keyboard.press("Space");
  await page.keyboard.press("l");
  expect(await transportCalls(page)).toEqual([]);
  expect(await edits(page)).toEqual([]);
});

test("the engine label shows the device, the load and the dropouts, or why there is none", async ({ page }) => {
  await openBar(page, project([track("t", "Synth")]), {
    transport_status: { available: false, backend: "null", reason: "audio backend disabled (null backend)" },
  });
  const engine = page.getByTestId("engine");
  await expect(engine).toHaveText("");
  await sendChannel(page, frame("playing"));
  await expect(engine).toHaveText("48 kHz · 256 · render 12% · 3 dropouts");
  await expect(engine).toHaveAttribute("title", "Output latency estimate 10.67 ms, lookahead 80.0 ms");
  await expect(page.getByTestId("position")).toHaveText("2.1.000");
  await expect(page.getByTestId("time")).toHaveText("00:02.000");
  await sendChannel(page, frame("idle"));
  await expect(engine).toHaveText("Audio engine unavailable");
  await expect(engine).toHaveAttribute("title", "audio backend disabled (null backend)");
});

test("a window that gets no animation frames still follows the transport (a stalled play)", async ({ page }) => {
  // WebKit gives a hidden window (minimized, on another Space, behind a full-screen app) no
  // animation frames; the live frames must still reach the transport's state and the playhead.
  await page.addInitScript(() => {
    window.requestAnimationFrame = () => 0;
  });
  await openBar(page, project([track("t", "Synth")]));
  const play = page.getByTestId("play");
  await expect(play).toHaveAttribute("aria-pressed", "false");
  // Longer than UNDRAWN_MS (live/queue.ts) without an animation frame.
  await page.waitForTimeout(300);
  await sendChannel(page, frame("playing"));
  await expect(play).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("position")).toHaveText("2.1.000");
  // Space now stops, as it must while the transport rolls.
  await page.keyboard.press("Space");
  await expect.poll(() => transportCalls(page)).toEqual([["transport_stop", {}]]);
  await sendChannel(page, frame("idle"));
  await expect(play).toHaveAttribute("aria-pressed", "false");
});

test("a drawn window shows a live frame at its next animation frame, not when it arrives", async ({ page }) => {
  // The animation frames run when the test says (the two-frame queue of live/queue.ts).
  await page.addInitScript(() => {
    const waiting: FrameRequestCallback[] = [];
    window.requestAnimationFrame = (callback) => waiting.push(callback);
    (window as unknown as { __tick: () => void }).__tick = () => {
      const now = performance.now();
      for (const callback of waiting.splice(0)) callback(now);
    };
  });
  await openBar(page, project([track("t", "Synth")]));
  const play = page.getByTestId("play");
  const bytes = [...frame("playing").matchAll(/../g)].map((m) => Number.parseInt(m[0], 16));
  // An animation frame, then a frame from the channel right after it: it waits for the next one.
  const before = await page.evaluate(async (data) => {
    (window as unknown as { __tick: () => void }).__tick();
    window.__sonaraMock.sendChannel(data);
    await new Promise((resolve) => setTimeout(resolve, 100));
    return document.querySelector('[data-testid="play"]')?.getAttribute("aria-pressed");
  }, bytes);
  expect(before).toBe("false");
  await page.evaluate(() => (window as unknown as { __tick: () => void }).__tick());
  await expect(play).toHaveAttribute("aria-pressed", "true");
});

test("without a project only Home and the count-in are enabled", async ({ page }) => {
  await openBar(page, null);
  for (const name of ["Play", "Stop", "Record", "Loop", "Metronome"]) {
    await expect(page.getByRole("button", { name, exact: true })).toBeDisabled();
  }
  await expect(page.getByRole("button", { name: "Return to Start" })).toBeEnabled();
  await expect(page.getByRole("combobox", { name: "Count-in" })).toBeEnabled();
  await expect(page.getByRole("spinbutton", { name: "Tempo" })).toBeDisabled();
});
