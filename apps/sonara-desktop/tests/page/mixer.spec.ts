import { expect, type Locator, type Page, test } from "@playwright/test";
import { frameHex } from "../support/frame";
import { project, track, view } from "../support/project";
import type { DeviceView, OpenProject, PluginGroup, TrackView } from "../../src/lib/types";
import { AUDIO_SUFFIXES, DATA_LAYER, answer, calls, emit, emitted, mockTauri, sendChannel } from "./tauri-mock";

// The mixer (–MX-13) against mocked commands in WebKit: the
// prototype's tests/gui/test_mixer_devices.py mixer and control tests, whose edits the page sends
// to sonara-app (P). The Rust side of each edit is tested in crates/sonara-app/tests/gui_mixer.rs.

const OUTCOME = {
  committed: true,
  version: 2,
  label: "Edit",
  changes: { created: [], updated: [], deleted: [], flags: [], structure_changed: false },
  result: {},
};

const EFFECTS: PluginGroup[] = [
  {
    category: "dynamics",
    title: "Dynamics",
    plugins: [{ id: "sonara.dynamics.compressor", name: "Compressor", description: "Compresses" }],
  },
  { category: "eq", title: "EQ", plugins: [{ id: "sonara.eq.parametric", name: "Parametric EQ", description: "Bands" }] },
];

function device(id: string, pluginId: string, name: string, enabled = true): DeviceView {
  return { id, plugin_id: pluginId, name, enabled, preset: null, sidechain_id: null, available: true, params: {} };
}

const SYNTH = device("dev_synth", "sonara.synth.poly", "Sonara Poly Synth");

function routed(t: TrackView, buses: TrackView[]): TrackView {
  return {
    ...t,
    output_targets: [{ id: "master", name: "Master" }, ...buses.filter((b) => b.id !== t.id).map((b) => ({ id: b.id, name: b.name }))],
    send_targets: buses.filter((b) => b.id !== t.id && !t.sends.some((s) => s.target_id === b.id)).map((b) => ({ id: b.id, name: b.name })),
  };
}

/** Vox (audio), Keys (instrument), Verb (bus) and a folder, routed as sonara-app routes them. */
function mixProject(edit: (tracks: TrackView[]) => TrackView[] = (t) => t): OpenProject {
  const verb = track("v", "Verb", "bus");
  const tracks = edit([
    track("a", "Vox", "audio"),
    { ...track("k", "Keys", "instrument"), instrument: SYNTH },
    verb,
    track("f", "Folder", "folder"),
  ]);
  const buses = tracks.filter((t) => t.kind === "bus");
  return project(tracks.map((t) => (t.kind === "folder" ? t : routed(t, buses))), { meter_layout: 42 });
}

async function openMixer(page: Page, open: OpenProject | null, extra: Record<string, unknown> = {}) {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: {
      agent: { on: false, address: null },
      messages: [],
      panes: {
        browser: { size: 240, open: false },
        inspector: { size: 280, open: false },
        editors: { size: 440, open: true },
      },
      audio_suffixes: AUDIO_SUFFIXES,
    },
    recoverable_sessions: [],
    project_view: view(1, open),
    plugin_menu: EFFECTS,
    edit: OUTCOME,
    seal_undo: null,
    open_mixer_window: null,
    ...extra,
  });
  await page.goto("/");
  if (open) await expect(page.getByRole("group", { name: "Channel strip Master" })).toBeVisible();
}

const strip = (page: Page, name: string) => page.getByRole("group", { name: `Channel strip ${name}` });

async function edits(page: Page): Promise<unknown[]> {
  return (await calls(page)).filter((c) => c.cmd === "edit").map((c) => (c.args as { edit: unknown }).edit);
}

/** The edits and seals, in order (`seal` for `seal_undo`). */
async function steps(page: Page): Promise<unknown[]> {
  return (await calls(page))
    .filter((c) => c.cmd === "edit" || c.cmd === "seal_undo")
    .map((c) => (c.cmd === "edit" ? (c.args as { edit: unknown }).edit : "seal"));
}

/** A new project view, as after an edit: the session event, then the refreshed view. */
async function refresh(page: Page, open: OpenProject, version = 2) {
  await answer(page, "project_view", view(version, open));
  await emit(page, "session-changed", {
    version,
    kind: "command",
    label: "Edit",
    source: "agent",
    changes: { created: [], updated: [], deleted: [], flags: ["mixer"], structure_changed: false },
  });
}

test("strips for the mixed tracks and the master; none for folders; R only where it applies", async ({ page }) => {
  await openMixer(page, mixProject());
  const strips = page.getByRole("region", { name: "Mixer" }).getByRole("group", { name: /^Channel strip / });
  await expect(strips).toHaveCount(4);
  await expect(strip(page, "Folder")).toHaveCount(0);
  await expect(strip(page, "Keys").getByRole("listitem")).toHaveText(["♪ Sonara Poly Synth"]);
  await expect(strip(page, "Keys").getByText("Sonara Poly Synth", { exact: true })).toBeVisible();
  await expect(strip(page, "Vox").getByRole("button", { name: "Arm for recording" })).toBeVisible();
  await expect(strip(page, "Verb").getByRole("button", { name: "Arm for recording" })).toHaveCount(0);
  const master = strip(page, "Master");
  await expect(master.getByRole("button", { name: "Solo" })).toHaveCount(0);
  await expect(master.getByRole("combobox", { name: "Output routing" })).toHaveCount(0);
  await expect(master.getByText("Main output")).toBeVisible();
  await expect(strip(page, "Vox").getByRole("combobox", { name: "Output routing" }).locator("option")).toHaveText([
    "→ Master",
    "→ Verb",
  ]);
  // A new track appears without the panel being made again.
  await refresh(page, mixProject((t) => [...t, track("d", "Drums", "instrument")]));
  await expect(strip(page, "Drums")).toBeVisible();
});

test("with no tracks the mixer says how to start", async ({ page }) => {
  await openMixer(page, project([]));
  await expect(page.getByText("No tracks yet — add a track to start mixing.")).toBeVisible();
});

async function capOf(fader: Locator) {
  // A short pane scrolls the strips (MX layout): the cap must be in view for the pointer.
  await fader.scrollIntoViewIfNeeded();
  const cap = await fader.locator(".cap").boundingBox();
  if (!cap) throw new Error("no fader cap");
  return { x: cap.x + cap.width / 2, y: cap.y + cap.height / 2 };
}

test("a fader drag sends its moves and seals once, as one undo step; undo restores the fader", async ({ page }) => {
  await openMixer(page, mixProject());
  const fader = strip(page, "Vox").getByRole("slider", { name: "Vox volume" });
  await expect(fader).toHaveAttribute("aria-valuetext", "0.0 dB");
  const cap = await capOf(fader);
  await page.mouse.move(cap.x, cap.y);
  await page.mouse.down();
  for (let i = 1; i <= 8; i++) await page.mouse.move(cap.x, cap.y + (60 * i) / 8);
  // The pointer's value shows at once, before any view; the typed dB follows it.
  await expect(fader).not.toHaveAttribute("aria-valuetext", "0.0 dB");
  const dragged = await fader.getAttribute("aria-valuetext");
  await expect(strip(page, "Vox").getByRole("textbox", { name: "Volume dB" })).toHaveValue(dragged?.replace(" dB", "") ?? "");
  await page.mouse.up();
  await expect.poll(async () => (await steps(page)).at(-1)).toBe("seal");
  const sent = await steps(page);
  expect(sent.filter((s) => s === "seal")).toHaveLength(1);
  const moves = sent.slice(0, -1) as { op: string; track_id: string; volume_db: number }[];
  expect(moves.length).toBeGreaterThan(0);
  for (const move of moves) expect(move).toMatchObject({ op: "set_track_mixer", track_id: "a" });
  const last = moves.at(-1)?.volume_db ?? 0;
  expect(last).toBeLessThan(-3);
  expect(dragged).toBe(`${last.toFixed(1)} dB`);
  // The view with the drag's value, then the undo's view, which puts the fader back.
  await refresh(page, mixProject((t) => t.map((x) => (x.id === "a" ? { ...x, mixer: { ...x.mixer, volume_db: last } } : x))), 2);
  await expect(fader).toHaveAttribute("aria-valuetext", `${last.toFixed(1)} dB`);
  await refresh(page, mixProject(), 3);
  await expect(fader).toHaveAttribute("aria-valuetext", "0.0 dB");
});

test("M, S and R toggle, and the typed dB edits with its forms", async ({ page }) => {
  await openMixer(page, mixProject());
  const vox = strip(page, "Vox");
  await vox.getByRole("button", { name: "Mute" }).click();
  await vox.getByRole("button", { name: "Solo" }).click();
  await vox.getByRole("button", { name: "Arm for recording" }).click();
  await expect.poll(() => edits(page)).toEqual([
    { op: "set_track_mixer", track_id: "a", mute: true },
    { op: "set_track_mixer", track_id: "a", solo: true },
    { op: "set_track_input", track_id: "a", armed: true },
  ]);
  const db = vox.getByRole("textbox", { name: "Volume dB" });
  await db.fill("-12.5");
  await db.press("Enter");
  await expect.poll(() => steps(page)).toContainEqual({ op: "set_track_mixer", track_id: "a", volume_db: -12.5 });
  await expect.poll(async () => (await steps(page)).at(-1)).toBe("seal");
  await db.fill("-inf");
  await db.press("Enter");
  await expect.poll(async () => (await steps(page)).at(-2)).toEqual({ op: "set_track_mixer", track_id: "a", volume_db: -144 });
  const before = (await steps(page)).length;
  await db.fill("loud");
  await db.press("Enter");
  await expect(page.getByRole("status")).toHaveText("'loud' is not a dB value");
  await expect(db).toHaveValue("-inf");
  expect(await steps(page)).toHaveLength(before);
  // The model's toggles show as pressed.
  await refresh(
    page,
    mixProject((t) => t.map((x) => (x.id === "a" ? { ...x, mixer: { ...x.mixer, mute: true } } : x))),
  );
  await expect(vox.getByRole("button", { name: "Mute" })).toHaveAttribute("aria-pressed", "true");
});

test("the pan knob and the fader take the keys of ValueControl", async ({ page }) => {
  await openMixer(page, mixProject());
  const vox = strip(page, "Vox");
  const pan = vox.getByRole("slider", { name: "Pan" });
  await pan.focus();
  await page.keyboard.press("ArrowRight");
  // One step is a hundredth of the travel: 0.02 of the −1…+1 range, as the prototype's.
  await expect.poll(async () => (await steps(page)).length).toBe(2);
  const [step, seal] = await steps(page);
  expect(step).toMatchObject({ op: "set_track_mixer", track_id: "a" });
  expect((step as { pan: number }).pan).toBeCloseTo(0.02, 12);
  expect(seal).toBe("seal");
  await expect(pan).toHaveAttribute("aria-valuetext", "R2");
  const fader = vox.getByRole("slider", { name: "Vox volume" });
  await fader.focus();
  await page.keyboard.press("Home");
  await expect.poll(async () => (await steps(page)).slice(2)).toEqual([
    { op: "set_track_mixer", track_id: "a", volume_db: -144 },
    "seal",
  ]);
  await expect(fader).toHaveAttribute("aria-valuetext", "-inf dB");
  await page.keyboard.press("Delete");
  await expect.poll(async () => (await steps(page)).slice(4)).toEqual([
    { op: "set_track_mixer", track_id: "a", volume_db: 0 },
    "seal",
  ]);
  await page.keyboard.press("ArrowUp");
  await expect.poll(async () => (await steps(page)).length).toBe(8);
  const up = (await steps(page))[6] as { volume_db: number };
  expect(up.volume_db).toBeGreaterThan(0);
});

test("an effect from the menu, bypass, reorder and the Devices tab", async ({ page }) => {
  const withInserts = mixProject((t) =>
    t.map((x) =>
      x.id === "k"
        ? { ...x, inserts: [device("dev_eq", "sonara.eq.parametric", "Parametric EQ"), device("dev_comp", "sonara.dynamics.compressor", "Compressor")] }
        : x,
    ),
  );
  await openMixer(page, withInserts, { edit: { ...OUTCOME, result: { device_id: "dev_new" } } });
  const keys = strip(page, "Keys");
  await keys.getByRole("button", { name: "Add effect" }).click();
  const menu = page.getByRole("menu", { name: "Add effect" });
  await expect(menu.getByRole("menuitem")).toHaveText(["Dynamics▸", "EQ▸"]);
  await menu.getByRole("menuitem", { name: "Dynamics" }).click();
  await page.getByRole("menu", { name: "Dynamics" }).getByRole("menuitem", { name: "Compressor" }).click();
  await expect.poll(() => edits(page)).toEqual([{ op: "add_device", track_id: "k", plugin_id: "sonara.dynamics.compressor" }]);
  // The new device is the current one, and the Devices tab shows it.
  await expect(page.getByRole("tab", { name: "Devices" })).toHaveAttribute("aria-selected", "true");
  await page.getByRole("tab", { name: "Mixer" }).click();
  // The checkbox bypasses, then seals.
  await keys.getByRole("checkbox", { name: "Bypass Parametric EQ" }).uncheck();
  await expect.poll(async () => (await steps(page)).slice(1)).toEqual([
    { op: "set_device_enabled", device_id: "dev_eq", enabled: false },
    "seal",
  ]);
  // The compressor dragged onto the EQ's row moves before it.
  const rows = keys.getByRole("listitem");
  const compressor = await rows.nth(2).boundingBox();
  const eq = await rows.nth(1).boundingBox();
  if (!compressor || !eq) throw new Error("no insert rows");
  await page.mouse.move(compressor.x + 50, compressor.y + compressor.height / 2);
  await page.mouse.down();
  await page.mouse.move(eq.x + 50, eq.y + eq.height / 2, { steps: 4 });
  await page.mouse.up();
  await expect.poll(async () => (await edits(page)).at(-1)).toEqual({ op: "move_device", device_id: "dev_comp", index: 0 });
  // A click on an insert selects it and shows the Devices tab; the context menu has the rest.
  await rows.nth(1).click();
  await expect(page.getByRole("tab", { name: "Devices" })).toHaveAttribute("aria-selected", "true");
  await page.getByRole("tab", { name: "Mixer" }).click();
  await expect(rows.nth(1)).toHaveClass(/current/);
  await rows.nth(1).click({ button: "right" });
  await expect(page.getByRole("menu", { name: "Inserts" }).getByRole("menuitem")).toHaveText([
    "Edit…",
    "Bypass",
    "Remove",
    "Add Effect▸",
  ]);
  await page.getByRole("menuitem", { name: "Remove" }).click();
  await expect.poll(async () => (await edits(page)).at(-1)).toEqual({ op: "remove_device", device_id: "dev_eq" });
  // The instrument cannot be removed.
  await rows.nth(0).click({ button: "right" });
  await expect(page.getByRole("menu", { name: "Inserts" }).getByRole("menuitem")).toHaveText([
    "Edit…",
    "Bypass",
    "Add Effect▸",
  ]);
});

test("a send to a bus, its level, Pre and Remove; a bus offers no send to itself", async ({ page }) => {
  await openMixer(page, mixProject());
  const vox = strip(page, "Vox");
  await vox.getByRole("button", { name: "Add send to a bus" }).click();
  await page.getByRole("menu", { name: "Add send" }).getByRole("menuitem", { name: "Verb" }).click();
  await expect.poll(() => edits(page)).toEqual([{ op: "add_send", track_id: "a", target_id: "v", level_db: -12 }]);
  const withSend = mixProject((t) =>
    t.map((x) => (x.id === "a" ? { ...x, sends: [{ id: "snd_1", target_id: "v", level_db: -12, pre_fader: false, enabled: true }] } : x)),
  );
  await refresh(page, withSend);
  const send = vox.getByRole("group", { name: "Send to Verb" });
  await send.getByRole("slider", { name: "Send to Verb" }).focus();
  await page.keyboard.press("ArrowUp");
  await expect.poll(async () => (await steps(page)).length).toBe(3);
  const level = (await steps(page))[1] as { op: string; send_id: string; level_db: number };
  expect(level).toMatchObject({ op: "update_send", track_id: "a", send_id: "snd_1" });
  expect(level.level_db).toBeGreaterThan(-12);
  expect((await steps(page))[2]).toBe("seal");
  await send.getByRole("button", { name: "Pre-fader send" }).click();
  await expect.poll(async () => (await steps(page)).slice(3)).toEqual([
    { op: "update_send", track_id: "a", send_id: "snd_1", pre_fader: true },
    "seal",
  ]);
  await send.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Remove Send" }).click();
  await expect.poll(async () => (await edits(page)).at(-1)).toEqual({ op: "remove_send", track_id: "a", send_id: "snd_1" });
  // The bus has no other bus to send to.
  await strip(page, "Verb").getByRole("button", { name: "Add send to a bus" }).click();
  const none = page.getByRole("menu", { name: "Add send" }).getByRole("menuitem");
  await expect(none).toHaveText(["No buses available"]);
  await expect(none).toBeDisabled();
});

test("a refused output route reports its loop and shows the model's output again", async ({ page }) => {
  await openMixer(page, mixProject());
  const output = strip(page, "Vox").getByRole("combobox", { name: "Output routing" });
  await output.selectOption("v");
  await expect.poll(() => edits(page)).toEqual([{ op: "set_track_output", track_id: "a", output_id: "v" }]);
  await answer(page, "edit", { reject: { code: "ROUTING_CYCLE", message: "routing Verb to Vox would create a loop" } });
  const verb = strip(page, "Verb").getByRole("combobox", { name: "Output routing" });
  await verb.selectOption({ label: "→ Master" });
  await output.selectOption("master");
  await expect(page.getByRole("alert")).toContainText("loop");
  await expect(output).toHaveValue("master");
});

test("a click on a strip selects its track, ⇧ adds; a double-click shows its devices", async ({ page }) => {
  await openMixer(page, mixProject());
  await strip(page, "Vox").getByText("Vox", { exact: true }).click();
  await expect(strip(page, "Vox")).toHaveClass(/selected/);
  await expect(page.locator('.header[data-track="a"]')).toHaveClass(/selected/);
  await strip(page, "Keys").locator(".name").click({ modifiers: ["Shift"] });
  await expect(strip(page, "Vox")).toHaveClass(/selected/);
  await expect(strip(page, "Keys")).toHaveClass(/selected/);
  // Without a mixer window the selection goes nowhere.
  expect((await emitted(page)).filter(([event]) => event === "gui-selection")).toEqual([]);
  await strip(page, "Verb").locator(".subtitle").dblclick();
  await expect(page.getByRole("tab", { name: "Devices" })).toHaveAttribute("aria-selected", "true");
  // A mixer window that starts is sent the selection, and then every change of it.
  await emit(page, "gui-intent", { from: "mixer", kind: "sync" });
  const shared = async () =>
    (await emitted(page))
      .filter(([event]) => event === "gui-selection")
      .map(([, payload]) => (payload as { selection: { tracks: string[] } }).selection.tracks);
  await expect.poll(shared).toEqual([["v"]]);
  await page.locator('.header[data-track="a"] .name').click();
  await expect.poll(shared).toEqual([["v"], ["a"]]);
});

test("an agent's edit refreshes a strip in place (agent_edit_from_other_thread_updates_strip)", async ({ page }) => {
  await openMixer(page, mixProject());
  const fader = strip(page, "Vox").getByRole("slider", { name: "Vox volume" });
  await fader.evaluate((element) => element.setAttribute("data-kept", "yes"));
  await refresh(
    page,
    mixProject((t) => t.map((x) => (x.id === "a" ? { ...x, mixer: { ...x.mixer, volume_db: -10, mute: true } } : x))),
  );
  await expect(fader).toHaveAttribute("aria-valuetext", "-10.0 dB");
  await expect(strip(page, "Vox").getByRole("button", { name: "Mute" })).toHaveAttribute("aria-pressed", "true");
  // The same element: refreshed, not rebuilt.
  await expect(fader).toHaveAttribute("data-kept", "yes");
});

test("a mixer edit refreshes only its tracks; any other event the whole view", async ({ page }) => {
  await openMixer(page, mixProject());
  const open = mixProject();
  const vox = open.tracks[0];
  await answer(page, "project_tracks", {
    version: 2,
    project: {
      id: open.id,
      dirty: true,
      history: { undo: "Track Volume", redo: null },
      meter_layout: open.meter_layout,
      tracks: [{ ...vox, mixer: { ...vox.mixer, volume_db: -9 } }],
    },
  });
  const viewsBefore = (await calls(page)).filter((c) => c.cmd === "project_view").length;
  await emit(page, "session-changed", {
    version: 2,
    kind: "command",
    label: "Track Volume",
    source: "agent",
    changes: { created: [], updated: [{ kind: "track", id: "a" }], deleted: [], flags: ["mixer"], structure_changed: false },
  });
  const fader = strip(page, "Vox").getByRole("slider", { name: "Vox volume" });
  await expect(fader).toHaveAttribute("aria-valuetext", "-9.0 dB");
  const asked = (await calls(page)).filter((c) => c.cmd === "project_tracks").map((c) => c.args);
  expect(asked).toEqual([{ trackIds: ["a"] }]);
  expect((await calls(page)).filter((c) => c.cmd === "project_view")).toHaveLength(viewsBefore);
  // An event that names no track asks for the whole view.
  await refresh(page, mixProject((t) => t.map((x) => (x.id === "a" ? { ...x, name: "Lead" } : x))), 3);
  await expect(strip(page, "Lead")).toBeVisible();
});

/** The colour of a meter canvas's clip indicator (its top-left pixel). */
async function clipColor(page: Page, trackId: string): Promise<number[]> {
  return page.locator(`canvas[data-meter="${trackId}"]`).evaluate((canvas: HTMLCanvasElement) => {
    const ctx = canvas.getContext("2d");
    return ctx ? [...ctx.getImageData(1, 1, 1, 1).data.slice(0, 3)] : [];
  });
}

test("the meters show the live frame's levels; a clip latches until a click clears it", async ({ page }) => {
  await openMixer(page, mixProject());
  // The tracks in order (Vox, Keys, Verb, Folder), then the master.
  await sendChannel(page, frameHex({ meterLayout: 42, meters: [[1.2, 0.5, 0.4, 0.3], [0.1, 0.1, 0.05, 0.05], [0, 0, 0, 0], [0, 0, 0, 0], [0.5, 0.5, 0.3, 0.3]] }));
  await expect.poll(() => clipColor(page, "a")).toEqual([0xee, 0x50, 0x50]);
  expect(await clipColor(page, "k")).toEqual([0x14, 0x15, 0x17]);
  // A frame of another layout lights nothing.
  await sendChannel(page, frameHex({ sequence: 2, meterLayout: 7, meters: [[0, 0, 0, 0], [2, 2, 1, 1], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]] }));
  await page.waitForTimeout(100);
  expect(await clipColor(page, "k")).toEqual([0x14, 0x15, 0x17]);
  await page.locator('canvas[data-meter="a"]').click();
  await expect.poll(() => clipColor(page, "a")).toEqual([0x14, 0x15, 0x17]);
});

test("the mixer opens in a window of its own and comes back when it closes", async ({ page }) => {
  await openMixer(page, mixProject());
  await page.getByRole("button", { name: "Open in Window" }).click();
  await expect.poll(async () => (await calls(page)).some((c) => c.cmd === "open_mixer_window")).toBe(true);
  await expect(page.getByText("The mixer is in its own window.")).toBeVisible();
  await expect(strip(page, "Vox")).toHaveCount(0);
  // The mixer window asks for the Devices tab; then it closes.
  await emit(page, "gui-intent", { from: "mixer", kind: "show-devices", trackId: "k" });
  await expect(page.getByRole("tab", { name: "Devices" })).toHaveAttribute("aria-selected", "true");
  await expect(page.locator('.header[data-track="k"]')).toHaveClass(/selected/);
  await emit(page, "mixer-window", { open: false });
  await page.getByRole("tab", { name: "Mixer" }).click();
  await expect(strip(page, "Vox")).toBeVisible();
});

test("a mixer window that was open when Sonara quit opens again at startup (A10)", async ({ page }) => {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: { agent: { on: false, address: null }, messages: [], audio_suffixes: AUDIO_SUFFIXES, reopen_mixer: true },
    recoverable_sessions: [],
    project_view: view(1, mixProject()),
    plugin_menu: EFFECTS,
    open_mixer_window: null,
  });
  await page.goto("/");
  await expect.poll(async () => (await calls(page)).filter((c) => c.cmd === "open_mixer_window").length).toBe(1);
  await expect(page.getByText("The mixer is in its own window.")).toBeVisible();
});

test("a mixer window that was closed stays closed at startup (A10)", async ({ page }) => {
  await openMixer(page, mixProject(), {
    shell_status: { agent: { on: false, address: null }, messages: [], audio_suffixes: AUDIO_SUFFIXES, reopen_mixer: false },
  });
  await page.waitForTimeout(300);
  expect((await calls(page)).some((c) => c.cmd === "open_mixer_window")).toBe(false);
  await expect(strip(page, "Vox")).toBeVisible();
});

test("the mixer's own window shows only the mixer and asks the main window for the rest", async ({ page }) => {
  await mockTauri(
    page,
    { ...DATA_LAYER, project_view: view(1, mixProject()), plugin_menu: EFFECTS, edit: OUTCOME, seal_undo: null },
    "mixer",
  );
  await page.goto("/");
  await expect(strip(page, "Vox")).toBeVisible();
  await expect(page.getByRole("region", { name: "Timeline" })).toHaveCount(0);
  // It asks the main window for its selection first.
  expect((await emitted(page))[0]).toEqual(["gui-intent", { from: "mixer", kind: "sync" }]);
  // The main window's selection shows here; a click here goes there.
  await emit(page, "gui-selection", {
    from: "main",
    selection: { tracks: ["k"], clips: [], notes: [], currentTrack: "k", currentClip: null, currentDevice: null },
  });
  await expect(strip(page, "Keys")).toHaveClass(/selected/);
  await strip(page, "Vox").locator(".name").click();
  await expect.poll(async () => (await emitted(page)).at(-1)).toEqual([
    "gui-selection",
    { from: "mixer", selection: { tracks: ["a"], clips: [], notes: [], currentTrack: "a", currentClip: null, currentDevice: null } },
  ]);
  await strip(page, "Vox").locator(".subtitle").dblclick();
  await expect.poll(async () => (await emitted(page)).at(-1)).toEqual(["gui-intent", { from: "mixer", kind: "show-devices", trackId: "a" }]);
  await page.keyboard.press("Space");
  await expect.poll(async () => (await emitted(page)).at(-1)).toEqual(["gui-intent", { from: "mixer", kind: "shortcut", action: "play" }]);
  // It never takes the MIDI focus, which is the main window's.
  expect((await calls(page)).some((c) => c.cmd === "set_midi_focus")).toBe(false);
});
