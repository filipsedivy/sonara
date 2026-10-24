import { expect, type Page, test } from "@playwright/test";
import { frameHex } from "../support/frame";
import { midiClip, project, track, view } from "../support/project";
import { trackControls, trackList, trackRow } from "../support/tracks";
import type { OpenProject } from "../../src/lib/types";
import { AUDIO_SUFFIXES, DATA_LAYER, calls, emit, mockTauri, sendChannel } from "./tauri-mock";

// The accessibility layer of against mocked commands in WebKit (P): ADR 0009's
// VoiceOver checks as the accessibility tree states them (Play a toggle button "Play", the master's
// fader a slider with its dB value that the arrow keys change, the track list a list box of
// "Drums 1, 0.0 dB, selected, 1 of N"), and the inventory's accessibility notes: the clips and the
// notes as options over their canvases, the meters, the live regions, the dialogs' focus, and the
// text fields' own ⌘Z (the prototype's line edits, probed on 2026-10-23). The names, roles and
// states are Playwright's reading of the ARIA; the AppKit tree of the running application is read
// by tests/e2e/a11y-tree.mjs.

test.use({ actionTimeout: 10_000 });

const OUTCOME = {
  committed: true,
  version: 2,
  label: "Edit",
  changes: { created: [], updated: [], deleted: [], flags: [], structure_changed: false },
  result: {},
};

/** Drums 1 (Beat, Fill), Bass (Riff, four notes from C4 in bar 2) and Vox, at the default height. */
function band(): OpenProject {
  return project(
    [
      track("d", "Drums 1", "instrument", [midiClip("d1", 0, 4, { name: "Beat" }), midiClip("d2", 4, 4, { name: "Fill" })]),
      track("b", "Bass", "instrument", [midiClip("b1", 4, 4, { name: "Riff" })]),
      track("v", "Vox", "audio"),
    ],
    { meter_layout: 5 },
  );
}

async function openWindow(page: Page, open: OpenProject, extra: Record<string, unknown> = {}) {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: {
      agent: { on: false, address: null },
      messages: [],
      panes: {
        browser: { size: 240, open: false },
        inspector: { size: 280, open: true },
        editors: { size: 380, open: true },
      },
      audio_suffixes: AUDIO_SUFFIXES,
    },
    recoverable_sessions: [],
    project_view: view(1, open),
    edit: OUTCOME,
    seal_undo: null,
    transport_play: { state: "playing", position_beat: 0 },
    transport_stop: { was_playing: true, takes: null, message: null, status: { stopped: true, was_playing: true } },
    transport_seek: { position_beat: 0 },
    ...extra,
  });
  await page.goto("/");
  await expect(trackList(page).getByRole("option")).toHaveCount(open.tracks.length);
}

async function edits(page: Page): Promise<unknown[]> {
  return (await calls(page)).filter((c) => c.cmd === "edit").map((c) => (c.args as { edit: unknown }).edit);
}

const arrangement = (page: Page) => page.getByRole("application", { name: "Arrangement" });
const clips = (page: Page) => page.getByRole("listbox", { name: "Clips" });
const notes = (page: Page) => page.getByRole("listbox", { name: "Notes" });

test("ADR 0009: Play is a toggle button named Play, pressed while the transport rolls", async ({ page }) => {
  await openWindow(page, band());
  const play = page.getByRole("button", { name: "Play", exact: true });
  await expect(play).toHaveAttribute("aria-pressed", "false");
  await expect(page.getByRole("toolbar", { name: "Transport" })).toMatchAriaSnapshot(`
    - toolbar "Transport":
      - button "Return to Start"
      - button "Play" [pressed=false]
      - button "Stop"
      - button "Record" [pressed=false]
      - timer "Position 1.1.000, time 00:00.000"
      - button "Loop" [pressed=false]
      - button "Metronome" [pressed=false]
  `);
  await play.click();
  await expect(play).toHaveAttribute("aria-pressed", "true");
  // The same button, the same name: only its state changed.
  await expect(page.getByRole("button", { name: "Play", exact: true, pressed: true })).toHaveCount(1);
  await page.keyboard.press("Space");
  await expect(play).toHaveAttribute("aria-pressed", "false");
});

test("ADR 0009: the master's fader reads Master volume, a slider with its dB value, and the arrow keys change it", async ({ page }) => {
  await openWindow(page, band());
  const master = page.getByRole("group", { name: "Channel strip Master" });
  const fader = master.getByRole("slider", { name: "Master volume" });
  await expect(fader).toHaveAttribute("aria-valuetext", "0.0 dB");
  await expect(fader).toHaveAttribute("aria-orientation", "vertical");
  await expect(fader).toHaveAttribute("aria-valuemin", "-144");
  await expect(fader).toHaveAttribute("aria-valuemax", "12");
  await fader.focus();
  await page.keyboard.press("ArrowDown");
  await expect(fader).not.toHaveAttribute("aria-valuetext", "0.0 dB");
  const text = await fader.getAttribute("aria-valuetext");
  expect(text).toMatch(/^-\d+\.\d dB$/);
  await expect
    .poll(async () => (await edits(page)).map((e) => (e as { op: string; track_id: string }).op + ":" + (e as { track_id: string }).track_id))
    .toEqual(["set_track_mixer:master"]);
  // The pan and the sends are sliders too, with their units; the toggles carry their state.
  const bass = page.getByRole("group", { name: "Channel strip Bass" });
  await expect(bass.getByRole("slider", { name: "Pan" })).toHaveAttribute("aria-valuetext", "C");
  await expect(bass.getByRole("slider", { name: "Bass volume" })).toHaveAttribute("aria-valuetext", "0.0 dB");
  await expect(bass.getByRole("button", { name: "Mute" })).toHaveAttribute("aria-pressed", "false");
  await expect(bass.getByRole("textbox", { name: "Volume dB" })).toHaveValue("0.0");
});

test("ADR 0009: the track list is a list box; ↑ and ↓ read Drums 1, 0.0 dB, selected, 1 of 3", async ({ page }) => {
  await openWindow(page, band());
  await expect(trackList(page)).toMatchAriaSnapshot(`
    - listbox "Tracks":
      - option "Drums 1, 0.0 dB"
      - option "Bass, 0.0 dB"
      - option "Vox, 0.0 dB"
  `);
  const drums = trackRow(page, "Drums 1");
  // One tab stop: the first row while no track is current.
  await expect(drums).toHaveAttribute("tabindex", "0");
  await expect(trackRow(page, "Bass")).toHaveAttribute("tabindex", "-1");
  await drums.focus();
  await page.keyboard.press("ArrowDown");
  const bass = trackRow(page, "Bass");
  await expect(bass).toBeFocused();
  await expect(bass).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("ArrowUp");
  await expect(drums).toBeFocused();
  await expect(drums).toHaveAttribute("aria-selected", "true");
  await expect(bass).toHaveAttribute("aria-selected", "false");
  await expect(drums).toHaveAttribute("aria-posinset", "1");
  await expect(drums).toHaveAttribute("aria-setsize", "3");
  await expect(trackList(page)).toHaveAttribute("aria-multiselectable", "true");
  // The selection is the window's: the mixer strip of Drums 1 is selected too, and the row is
  // the list's tab stop now.
  await expect(page.locator('.strip[data-track="d"]')).toHaveClass(/selected/);
  await expect(drums).toHaveAttribute("tabindex", "0");
  // ↑ at the first row stays there; VoiceOver's press (a click without a pointer) selects a row.
  await page.keyboard.press("ArrowUp");
  await expect(drums).toBeFocused();
  await page.getByRole("option", { name: "Vox, 0.0 dB" }).dispatchEvent("click");
  await expect(trackRow(page, "Vox")).toHaveAttribute("aria-selected", "true");
  // M, S and R are real buttons beside the row, with their pressed state.
  await expect(trackControls(page, "Drums 1")).toMatchAriaSnapshot(`
    - group "Drums 1 controls":
      - button "Mute" [pressed=false]
      - button "Solo" [pressed=false]
      - button "Arm for recording" [pressed=false]
  `);
});

test("the arrangement's clips are options with their name, start and length; ↑ and ↓ select them, and the canvas's keys act on them", async ({ page }) => {
  await openWindow(page, band());
  await expect(clips(page)).toMatchAriaSnapshot(`
    - listbox "Clips":
      - group "Drums 1":
        - option "Beat, bar 1 beat 1, 4 beats"
        - option "Fill, bar 2 beat 1, 4 beats"
      - group "Bass":
        - option "Riff, bar 2 beat 1, 4 beats"
  `);
  await arrangement(page).focus();
  await page.keyboard.press("ArrowDown");
  const beat = clips(page).getByRole("option", { name: /^Beat,/ });
  await expect(beat).toBeFocused();
  await expect(beat).toHaveAttribute("aria-selected", "true");
  await expect(beat).toHaveAttribute("aria-posinset", "1");
  await expect(beat).toHaveAttribute("aria-setsize", "2");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  const riff = clips(page).getByRole("option", { name: /^Riff,/ });
  await expect(riff).toBeFocused();
  await expect(riff).toHaveAttribute("aria-selected", "true");
  await expect(beat).toHaveAttribute("aria-selected", "false");
  // The track follows, as a click on the clip selects it.
  await expect(trackRow(page, "Bass")).toHaveAttribute("aria-selected", "true");
  // The canvas's own keys act on the focused clip: → nudges, Delete deletes.
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("Delete");
  await expect.poll(() => edits(page)).toEqual([
    expect.objectContaining({ op: "move_clips", clip_ids: ["b1"] }),
    { op: "delete_clips", clip_ids: ["b1"] },
  ]);
  // VoiceOver's press on an option selects its clip.
  await clips(page).getByRole("option", { name: /^Fill,/ }).dispatchEvent("click");
  await expect(clips(page).getByRole("option", { name: /^Fill,/ })).toHaveAttribute("aria-selected", "true");
  // The canvases are drawings for assistive technology: hidden.
  await expect(page.getByTestId("arrangement-canvas")).toHaveAttribute("aria-hidden", "true");
});

test("the piano roll's notes are options: C4, bar 2 beat 1, 1/8, velocity 100; the grid's keys act on them", async ({ page }) => {
  await openWindow(page, band());
  await arrangement(page).focus();
  for (let i = 0; i < 3; i++) await page.keyboard.press("ArrowDown");
  await expect(clips(page).getByRole("option", { name: /^Riff,/ })).toBeFocused();
  await page.getByRole("tab", { name: "Piano Roll" }).click();
  await expect(notes(page)).toMatchAriaSnapshot(`
    - listbox "Notes":
      - option "C4, bar 2 beat 1, 1/8, velocity 100"
      - option "C#4, bar 2 beat 1 tick 480, 1/8, velocity 100"
      - option "D4, bar 2 beat 2, 1/8, velocity 100"
      - option "D#4, bar 2 beat 2 tick 480, 1/8, velocity 100"
  `);
  const second = notes(page).getByRole("option", { name: /^C#4,/ });
  await expect(second).toHaveAttribute("aria-posinset", "2");
  await expect(second).toHaveAttribute("aria-setsize", "4");
  // VoiceOver's press selects the note; ↑ then transposes it, the grid's own key.
  await second.dispatchEvent("click");
  await expect(second).toHaveAttribute("aria-selected", "true");
  await second.focus();
  await page.keyboard.press("ArrowUp");
  await expect.poll(async () => (await edits(page)).at(-1)).toEqual({
    op: "edit_notes",
    clip_id: "b1",
    edits: [{ note_id: "b1_n1", pitch: 62, start: 0.5 }],
  });
  await expect(page.getByTestId("note-grid-canvas")).toHaveAttribute("aria-hidden", "true");
});

test("the meters are meters named after their track, with a level a few times a second", async ({ page }) => {
  await openWindow(page, band());
  const meter = page.getByRole("group", { name: "Channel strip Bass" }).getByRole("meter", { name: "Bass level" });
  await expect(meter).toHaveAttribute("aria-valuetext", "silent");
  await expect(meter).toHaveAttribute("aria-valuemin", "-60");
  await expect(meter).toHaveAttribute("aria-valuemax", "6");
  // Every value the bank sets until the meter is silent again (54 dB at 24 dB/s: 2.25 s).
  await meter.evaluate((element) => {
    const values: [number, string][] = [];
    (window as unknown as { __meterValues: [number, string][] }).__meterValues = values;
    new MutationObserver(() => values.push([performance.now(), element.getAttribute("aria-valuetext") ?? ""])).observe(element, {
      attributes: true,
      attributeFilter: ["aria-valuetext"],
    });
  });
  // The tracks in order (Drums 1, Bass, Vox), then the master: Bass peaks at 0.5 (−6 dB), then
  // falls at 24 dB/s.
  await sendChannel(page, frameHex({ meterLayout: 5, meters: [[0, 0, 0, 0], [0.5, 0.25, 0.3, 0.2], [0, 0, 0, 0], [0.5, 0.5, 0.3, 0.3]] }));
  await page.waitForTimeout(2800);
  const values = await page.evaluate(() => (window as unknown as { __meterValues: [number, string][] }).__meterValues);
  const levels = values.map(([, text]) => (text === "silent" ? -Infinity : Number.parseInt(text, 10)));
  // The first value is the peak, at most a quarter of a second of fall below −6 dB.
  expect(levels[0]).toBeLessThanOrEqual(-6);
  expect(levels[0]).toBeGreaterThanOrEqual(-13);
  // It only falls, to silence, and it changes at most four times a second (plus the first).
  expect(levels.every((level, i) => i === 0 || level < levels[i - 1])).toBe(true);
  expect(values.at(-1)?.[1]).toBe("silent");
  const span = (values.at(-1)?.[0] ?? 0) - (values[0]?.[0] ?? 0);
  expect(values.length - 1).toBeLessThanOrEqual(Math.ceil(span / 250) + 1);
  for (let i = 1; i < values.length; i++) expect(values[i][0] - values[i - 1][0]).toBeGreaterThan(200);
});

test("status messages go to a polite live region, errors to an assertive one", async ({ page }) => {
  await openWindow(page, band());
  const status = page.getByRole("status").filter({ hasText: "Autosaved" });
  await expect(page.locator('.status-bar [role="status"]')).toHaveAttribute("aria-live", "polite");
  await emit(page, "message", { text: "Autosaved", duration_ms: 5000, error: false });
  await expect(status).toHaveCount(1);
  await emit(page, "message", { text: "the file is gone", duration_ms: 5000, error: true });
  await expect(page.getByRole("alert")).toHaveText("the file is gone");
  // The transport's position is no live region: it would chatter at 60 Hz.
  await expect(page.getByRole("timer")).not.toHaveAttribute("aria-live", /polite|assertive/);
});

test("a dialog is labelled, Tab and ⇧Tab stay inside it, and the focus returns when it closes", async ({ page }) => {
  await openWindow(page, band(), {
    agent_connection: {
      status: "This window accepts agent connections at /tmp/s.sock.",
      address: "/tmp/s.sock",
      cli: "/Applications/Sonara.app/Contents/MacOS/sonara",
      command: "claude mcp add sonara -- /Applications/Sonara.app/Contents/MacOS/sonara mcp",
      text: "claude mcp add sonara -- /Applications/Sonara.app/Contents/MacOS/sonara mcp",
      discovery: "/tmp/run/gui.json",
    },
  });
  await arrangement(page).focus();
  await emit(page, "dialog-command", { dialog: "agent-connection" });
  const dialog = page.getByRole("dialog", { name: "Agent Connection" });
  await expect(dialog).toBeVisible();
  const inside = () => dialog.evaluate((element) => element.contains(document.activeElement));
  await expect.poll(inside).toBe(true);
  for (let i = 0; i < 6; i++) {
    await page.keyboard.press("Tab");
    expect(await inside()).toBe(true);
  }
  for (let i = 0; i < 6; i++) {
    await page.keyboard.press("Shift+Tab");
    expect(await inside()).toBe(true);
  }
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(arrangement(page)).toBeFocused();
});

/** Records, after the page's own handlers, whether the window's next ⌘Z was taken. */
async function watchUndoKeys(page: Page) {
  await page.evaluate(() => {
    const seen: boolean[] = [];
    (window as unknown as { __undoKeys: boolean[] }).__undoKeys = seen;
    window.addEventListener("keydown", (event) => {
      if (event.metaKey && event.key.toLowerCase() === "z") seen.push(event.defaultPrevented);
    }, true);
  });
}

const undoKeys = (page: Page) => page.evaluate(() => (window as unknown as { __undoKeys: boolean[] }).__undoKeys);

test("⌘Z and ⇧⌘Z in a text field undo and redo its typing, as the prototype's line edits; elsewhere the menu's undo gets them", async ({ page }) => {
  await openWindow(page, band());
  await watchUndoKeys(page);
  const name = page.locator("#project-name");
  await name.click();
  await page.keyboard.press("End");
  await page.keyboard.type("xyz");
  await expect(name).toHaveValue("Timelinexyz");
  await page.keyboard.press("Meta+z");
  await expect(name).toHaveValue("Timeline");
  await page.keyboard.press("Meta+Shift+z");
  await expect(name).toHaveValue("Timelinexyz");
  await page.keyboard.press("Meta+z");
  await expect(name).toHaveValue("Timeline");
  // A number field too: the tempo.
  const tempo = page.getByRole("spinbutton", { name: "Tempo" });
  await tempo.click();
  await page.keyboard.press("Meta+a");
  await page.keyboard.type("140");
  await page.keyboard.press("Meta+z");
  await expect(tempo).toHaveValue("120.00");
  // Outside a text field the key is left to the native menu's Undo (the project's).
  await arrangement(page).focus();
  await page.keyboard.press("Meta+z");
  expect(await undoKeys(page)).toEqual([true, true, true, true, false]);
  // None of them sent a project edit: the typing was taken back before the fields lost the focus.
  expect(await edits(page)).toEqual([]);
});

test("a row reached by ↓, or scrolled into view by assistive technology, shows where the timeline draws it", async ({ page }) => {
  const many = project(Array.from({ length: 30 }, (_, i) => track(`t${i}`, `T${i}`)));
  await openWindow(page, many);
  const list = await trackList(page).boundingBox();
  if (!list) throw new Error("no track list");
  const inside = async (name: string) => {
    const row = await trackRow(page, name).boundingBox();
    return !!row && row.y >= list.y - 1 && row.y + row.height <= list.y + list.height + 1;
  };
  await trackRow(page, "T0").focus();
  for (let i = 0; i < 20; i++) await page.keyboard.press("ArrowDown");
  await expect(trackRow(page, "T20")).toBeFocused();
  expect(await inside("T20")).toBe(true);
  // A row above the view that takes the focus without the pointer (VoiceOver's cursor moves the
  // keyboard focus) scrolls into view.
  await trackRow(page, "T2").focus();
  await expect.poll(() => inside("T2")).toBe(true);
  // A scroll into view of a hidden row below scrolls the view, not the list's box.
  await trackRow(page, "T29").evaluate((element) => element.scrollIntoView());
  await expect.poll(() => inside("T29")).toBe(true);
  expect(await trackList(page).evaluate((element) => element.scrollTop)).toBe(0);
});
