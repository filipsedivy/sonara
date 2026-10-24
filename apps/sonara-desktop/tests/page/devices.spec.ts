import { expect, type Locator, type Page, test } from "@playwright/test";
import oracle from "../fixtures/device-editors-v1.json" with { type: "json" };
import { frameHex } from "../support/frame";
import { GOLDEN, PLUGINS, defaults, plugin, pluginMap } from "../support/plugins";
import { project, track, view } from "../support/project";
import { CURVE_SIZE, xOf, yOf } from "../../src/lib/devices/eq";
import type { DeviceView, EqResponse, OpenProject, PluginGroup, TrackView } from "../../src/lib/types";
import { AUDIO_SUFFIXES, DATA_LAYER, answer, calls, emit, mockTauri, sendChannel } from "./tauri-mock";
import { trackList } from "../support/tracks";

// The device editors (–DV-08) against mocked commands in WebKit: the
// prototype's tests/gui/test_mixer_devices.py device tests, whose edits the page sends to
// sonara-app (P). The descriptors are golden/plugins/catalog.json (its checked copy,
// tests/fixtures/plugin-catalog.json), and what each control shows is the prototype's
// (tests/fixtures/device-editors-v1.json, recorded from it; see tests/unit/devices.test.ts). The
// Rust side of each edit is tested in crates/sonara-app/tests/gui_devices.rs.

// An action that cannot happen fails with its call log instead of the test's timeout.
test.use({ actionTimeout: 10_000 });

const OUTCOME = {
  committed: true,
  version: 2,
  label: "Edit",
  changes: { created: [], updated: [], deleted: [], flags: [], structure_changed: false },
  result: {},
};

const EFFECTS: PluginGroup[] = [
  {
    category: "delay",
    title: "Delay",
    plugins: [{ id: "sonara.delay", name: "Delay", description: "Echoes" }],
  },
  {
    category: "dynamics",
    title: "Dynamics",
    plugins: [{ id: "sonara.dynamics.compressor", name: "Compressor", description: "Compresses" }],
  },
];

const INSTRUMENTS: PluginGroup[] = [
  { category: "drums", title: "Drums", plugins: [{ id: "sonara.drums", name: "Drum Kit", description: "Drums" }] },
  { category: "synth", title: "Synthesizers", plugins: [{ id: "sonara.synth.poly", name: "Poly Synth", description: "Synth" }] },
];

function device(id: string, pluginId: string, extra: Partial<DeviceView> = {}): DeviceView {
  const known = PLUGINS.some((p) => p.id === pluginId);
  return {
    id,
    plugin_id: pluginId,
    name: known ? plugin(pluginId).name : pluginId,
    enabled: true,
    preset: null,
    sidechain_id: null,
    available: known,
    params: known ? defaults(pluginId) : {},
    ...extra,
  };
}

const EFFECT_IDS = PLUGINS.filter((p) => p.kind === "effect").map((p) => p.id);

/**
 * FX (audio) with every built-in effect, Synth and Drums (instruments), Bass (the synth and a
 * compressor), a folder, Vox (audio, no devices), and the master; every built-in plugin's
 * descriptor in the view.
 */
function devicesProject(edit: (tracks: TrackView[]) => TrackView[] = (t) => t): OpenProject {
  const tracks = edit([
    { ...track("fx", "FX", "audio"), inserts: EFFECT_IDS.map((id, i) => device(`dev_fx${i}`, id)) },
    { ...track("s", "Synth", "instrument"), instrument: device("dev_synth", "sonara.synth.poly") },
    { ...track("d", "Drums", "instrument"), instrument: device("dev_drums", "sonara.drums") },
    {
      ...track("b", "Bass", "instrument"),
      instrument: device("dev_bass", "sonara.synth.poly"),
      inserts: [device("dev_comp", "sonara.dynamics.compressor")],
    },
    track("f", "Folder", "folder"),
    track("a", "Vox", "audio"),
  ]);
  return project(tracks, { plugins: pluginMap(), meter_layout: 9 });
}

/** A flat curve at `db` over the prototype's 240 points, its bands off. */
function flatCurve(db: number): EqResponse {
  const freqs = Array.from({ length: 240 }, (_, i) => 20 * 1000 ** (i / 239));
  return { freqs, db: freqs.map(() => db), bands: [] };
}

async function openDevices(page: Page, open: OpenProject | null, extra: Record<string, unknown> = {}) {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: {
      agent: { on: false, address: null },
      messages: [],
      panes: {
        browser: { size: 240, open: false },
        inspector: { size: 280, open: false },
        editors: { size: 470, open: true },
      },
      audio_suffixes: AUDIO_SUFFIXES,
    },
    recoverable_sessions: [],
    project_view: view(1, open),
    plugin_menu: EFFECTS,
    edit: OUTCOME,
    seal_undo: null,
    eq_response: flatCurve(0),
    ...extra,
  });
  await page.goto("/");
  if (open) await expect(trackList(page).getByRole("option")).toHaveCount(open.tracks.length);
  // The page asked for the effect picker as it started; the panel asks for the instruments.
  await expect.poll(async () => (await calls(page)).some((c) => c.cmd === "plugin_menu")).toBe(true);
  await answer(page, "plugin_menu", INSTRUMENTS);
}

const devicesTab = (page: Page) => page.getByRole("tab", { name: "Devices" });
const panel = (page: Page) => page.getByRole("region", { name: "Devices" });
const card = (page: Page, name: string) => panel(page).getByRole("group", { name: `Device ${name}`, exact: true });

/** Selects the track `trackId` by its header, then shows the Devices tab. */
async function showTrack(page: Page, trackId: string) {
  await page.locator(`.header[data-track="${trackId}"] .name`).click();
  await devicesTab(page).click();
}

async function edits(page: Page): Promise<unknown[]> {
  return (await calls(page)).filter((c) => c.cmd === "edit").map((c) => (c.args as { edit: unknown }).edit);
}

/** The edits and seals, in order (`seal` for `seal_undo`). */
async function steps(page: Page): Promise<unknown[]> {
  return (await calls(page))
    .filter((c) => c.cmd === "edit" || c.cmd === "seal_undo")
    .map((c) => (c.cmd === "edit" ? (c.args as { edit: unknown }).edit : "seal"));
}

/**
 * A new project view after an edit that changed the chain or the routing (a device added or
 * removed, a sidechain, a track deleted): the session event, then the whole view it asks for.
 */
async function refresh(page: Page, open: OpenProject, version = 2, source = "gui") {
  await answer(page, "project_view", view(version, open));
  await emit(page, "session-changed", {
    version,
    kind: "command",
    label: "Edit",
    source,
    changes: {
      created: [],
      updated: [{ kind: "track", id: "x" }],
      deleted: [],
      flags: ["devices", "routing", "tracks"],
      structure_changed: true,
    },
  });
}

/**
 * The refresh of an edit that changed only the state of the devices `ids` (a knob, a preset,
 * On/Off; `changedDevices`): the session event, then the devices view it asks for, built from
 * `open` as sonara-app's `devices_view` builds it.
 */
async function refreshDevices(page: Page, open: OpenProject, ids: string[], version = 2, source = "gui") {
  const devices = [...open.tracks, open.master].flatMap((t) =>
    [...(t.instrument ? [t.instrument] : []), ...t.inserts].filter((d) => ids.includes(d.id)).map((device) => ({ track_id: t.id, device })),
  );
  await answer(page, "project_devices", {
    version,
    project: { id: open.id, dirty: true, history: { undo: "Set Parameter", redo: null }, meter_layout: open.meter_layout, devices },
  });
  await emit(page, "session-changed", {
    version,
    kind: "command",
    label: "Set Parameter",
    source,
    changes: {
      created: [],
      updated: ids.map((id) => ({ kind: "device", id })),
      deleted: [],
      flags: ["devices"],
      structure_changed: true,
    },
  });
}

/** What a card's parameter editors show, read from the page in one go. */
interface Shown {
  id: string;
  label: string;
  role: string | null;
  name: string | null;
  min: string | null;
  max: string | null;
  now: string | null;
  text: string | null;
  pressed: string | null;
  options: string[];
  value: string | null;
}

async function shownParams(c: Locator): Promise<{ legends: string[]; params: Shown[] }> {
  return c.evaluate((element) => {
    const legends = [...element.querySelectorAll("legend")].map((l) => l.textContent?.trim() ?? "");
    const params = [...element.querySelectorAll<HTMLElement>(".param[data-param]")].map((param) => {
      const slider = param.querySelector<HTMLElement>("[role=slider]");
      const button = param.querySelector<HTMLButtonElement>("button[aria-pressed]");
      const select = param.querySelector<HTMLSelectElement>("select");
      const control = slider ?? button ?? select;
      return {
        id: param.dataset.param ?? "",
        label: (slider?.querySelector(".title") ?? param.querySelector(".label"))?.textContent?.trim() ?? "",
        role: slider ? "slider" : button ? "toggle" : select ? "combo" : null,
        name: control?.getAttribute("aria-label") ?? null,
        min: slider?.getAttribute("aria-valuemin") ?? null,
        max: slider?.getAttribute("aria-valuemax") ?? null,
        now: slider?.getAttribute("aria-valuenow") ?? null,
        text: slider?.getAttribute("aria-valuetext") ?? button?.textContent?.trim() ?? null,
        pressed: button?.getAttribute("aria-pressed") ?? null,
        options: select ? [...select.options].map((o) => o.text) : [],
        value: select?.value ?? null,
      };
    });
    return { legends, params };
  });
}

/** The card of `pluginId` shows every parameter as the prototype's editor does, at its defaults. */
async function expectCompleteEditor(c: Locator, pluginId: string) {
  const recorded = oracle.plugins.find((p) => p.id === pluginId);
  const golden = GOLDEN.find((d) => d.id === pluginId);
  if (!recorded || !golden) throw new Error(`no fixture for ${pluginId}`);
  const shown = await shownParams(c);
  expect(shown.legends, pluginId).toEqual(recorded.groups.map((g) => g.title));
  const expected = recorded.groups.flatMap((g) => g.params);
  expect(shown.params.map((p) => p.id), pluginId).toEqual(expected.map((p) => p.id));
  expect([...shown.params.map((p) => p.id)].sort(), pluginId).toEqual(golden.params.map((p) => p.id).sort());
  for (const want of expected) {
    const spec = golden.params.find((p) => p.id === want.id);
    const got = shown.params.find((p) => p.id === want.id);
    const where = `${pluginId} ${want.id}`;
    if (!spec || !got) throw new Error(`${where}: missing`);
    expect(got.label, where).toBe(want.label);
    expect(got.name, where).toBe(spec.name);
    if (want.control === "knob") {
      expect(got.role, where).toBe("slider");
      expect(Number(got.min), where).toBe(spec.minimum);
      expect(Number(got.max), where).toBe(spec.maximum);
      expect(Number(got.now), where).toBe(Number(spec.default));
      // The value text carries the unit as the prototype formats it.
      expect(got.text, where).toBe(want.text);
    } else if (want.control === "toggle") {
      expect(got.role, where).toBe("toggle");
      expect(got.text, where).toBe(want.text);
      expect(got.pressed, where).toBe(String(want.checked));
    } else {
      expect(got.role, where).toBe("combo");
      expect(got.options, where).toEqual(want.items);
      expect(got.value, where).toBe(want.current);
    }
  }
}

test("every built-in plugin has a complete editor: each parameter's control, range, default and unit", async ({ page }) => {
  await openDevices(page, devicesProject());
  await showTrack(page, "fx");
  await expect(panel(page).getByText("FX — devices")).toBeVisible();
  const cards = panel(page).getByRole("group", { name: /^Device / });
  await expect(cards).toHaveCount(EFFECT_IDS.length);
  for (const pluginId of EFFECT_IDS) {
    const descriptor = plugin(pluginId);
    const c = card(page, descriptor.name);
    await expectCompleteEditor(c, pluginId);
    // The preset combo only with presets; the sidechain source only where the plugin takes one;
    // the gain reduction for dynamics; the curve for the parametric EQ.
    await expect(c.getByRole("combobox", { name: "Preset" })).toHaveCount(descriptor.presets.length > 0 ? 1 : 0);
    if (descriptor.presets.length > 0) {
      await expect(c.getByRole("combobox", { name: "Preset" }).locator("option")).toHaveText(["Custom", ...descriptor.presets]);
    }
    await expect(c.getByRole("combobox", { name: "Sidechain source" })).toHaveCount(descriptor.sidechain ? 1 : 0);
    await expect(c.getByTitle("Gain reduction (live)")).toHaveCount(descriptor.category === "dynamics" ? 1 : 0);
    await expect(c.locator("canvas.curve")).toHaveCount(pluginId === "sonara.eq.parametric" ? 1 : 0);
  }
  // The instruments, on their own tracks.
  await page.getByRole("tab", { name: "Mixer" }).click();
  await showTrack(page, "s");
  await expect(cards).toHaveCount(1);
  await expectCompleteEditor(card(page, plugin("sonara.synth.poly").name), "sonara.synth.poly");
  await page.getByRole("tab", { name: "Mixer" }).click();
  await showTrack(page, "d");
  await expectCompleteEditor(card(page, plugin("sonara.drums").name), "sonara.drums");
});

test("with nothing to edit the panel says why; + Effect waits for a track that takes effects", async ({ page }) => {
  await openDevices(page, devicesProject());
  await devicesTab(page).click();
  await expect(panel(page).getByText("Select a track to edit its instrument and effects.")).toBeVisible();
  await expect(panel(page).getByText("Devices", { exact: true })).toBeVisible();
  await expect(panel(page).getByRole("button", { name: "+ Effect" })).toBeDisabled();
  await page.getByRole("tab", { name: "Mixer" }).click();
  await showTrack(page, "f");
  await expect(panel(page).getByText("Folder tracks have no devices.")).toBeVisible();
  await expect(panel(page).getByRole("button", { name: "+ Effect" })).toBeDisabled();
  await page.getByRole("tab", { name: "Mixer" }).click();
  await showTrack(page, "a");
  await expect(panel(page).getByText("No devices — use “+ Effect” to add one.")).toBeVisible();
  await expect(panel(page).getByText("Vox — devices")).toBeVisible();
  await expect(panel(page).getByRole("button", { name: "+ Effect" })).toBeEnabled();
});

test("a knob, a choice, a preset, On/Off, Remove, Replace Instrument and Reset each send their step", async ({ page }) => {
  await openDevices(page, devicesProject());
  await showTrack(page, "b");
  const synth = card(page, plugin("sonara.synth.poly").name);
  // A key step is one edit, then the seal.
  await synth.getByRole("slider", { name: "Cutoff", exact: true }).focus();
  await page.keyboard.press("ArrowUp");
  await expect.poll(() => steps(page)).toHaveLength(2);
  const [cutoff, seal] = await steps(page);
  expect(seal).toBe("seal");
  const values = (cutoff as { values: { filter_cutoff: number } }).values;
  expect(cutoff).toMatchObject({ op: "set_device_params", device_id: "dev_bass" });
  expect(values.filter_cutoff).toBeGreaterThan(2500);
  // A choice and a toggle each send one edit and seal it.
  await synth.getByRole("combobox", { name: "Voice Mode", exact: true }).selectOption("mono");
  await expect.poll(async () => (await steps(page)).slice(2)).toEqual([
    { op: "set_device_params", device_id: "dev_bass", values: { voice_mode: "mono" } },
    "seal",
  ]);
  const comp = card(page, "Compressor");
  await comp.getByRole("button", { name: "Auto Makeup", exact: true }).click();
  await expect.poll(async () => (await steps(page)).slice(4)).toEqual([
    { op: "set_device_params", device_id: "dev_comp", values: { auto_makeup: true } },
    "seal",
  ]);
  // A preset; "Custom" sends nothing.
  await synth.getByRole("combobox", { name: "Preset" }).selectOption("Analog Bass");
  await expect.poll(async () => (await steps(page)).slice(6)).toEqual([
    { op: "apply_preset", device_id: "dev_bass", preset: "Analog Bass" },
    "seal",
  ]);
  await synth.getByRole("combobox", { name: "Preset" }).selectOption("Custom");
  // On/Off bypasses, then seals.
  await synth.getByRole("button", { name: "Enable / bypass" }).click();
  await expect.poll(async () => (await steps(page)).slice(8)).toEqual([
    { op: "set_device_enabled", device_id: "dev_bass", enabled: false },
    "seal",
  ]);
  // An effect's menu removes it; the instrument's replaces it; Reset to Defaults seals.
  await comp.getByRole("button", { name: "Device actions" }).click();
  await expect(page.getByRole("menu", { name: "Device actions" }).getByRole("menuitem")).toHaveText(["Remove", "Reset to Defaults"]);
  await page.getByRole("menuitem", { name: "Remove" }).click();
  await expect.poll(async () => (await steps(page)).slice(10)).toEqual([{ op: "remove_device", device_id: "dev_comp" }]);
  await synth.getByRole("button", { name: "Device actions" }).click();
  const actions = page.getByRole("menu", { name: "Device actions" });
  await expect(actions.getByRole("menuitem")).toHaveText(["Replace Instrument▸", "Reset to Defaults"]);
  await actions.getByRole("menuitem", { name: "Replace Instrument" }).click();
  await page.getByRole("menu", { name: "Replace Instrument" }).getByRole("menuitem", { name: "Drums" }).click();
  await page.getByRole("menu", { name: "Drums" }).getByRole("menuitem", { name: "Drum Kit" }).click();
  await expect.poll(async () => (await steps(page)).slice(11)).toEqual([
    { op: "set_instrument", track_id: "b", plugin_id: "sonara.drums" },
  ]);
  await synth.getByRole("button", { name: "Device actions" }).click();
  await page.getByRole("menuitem", { name: "Reset to Defaults" }).click();
  await expect.poll(async () => (await steps(page)).slice(12)).toEqual([{ op: "reset_device", device_id: "dev_bass" }, "seal"]);
});

test("a knob drag sends its moves and seals once, as one undo step (knob_drag_and_int_quantize)", async ({ page }) => {
  await openDevices(page, devicesProject());
  await showTrack(page, "b");
  const synth = card(page, plugin("sonara.synth.poly").name);
  const knob = synth.getByRole("slider", { name: "Polyphony", exact: true });
  await knob.scrollIntoViewIfNeeded();
  const box = await knob.boundingBox();
  if (!box) throw new Error("no knob");
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2 - 60, { steps: 6 });
  await page.mouse.up();
  await expect.poll(async () => (await steps(page)).at(-1)).toBe("seal");
  const sent = await steps(page);
  expect(sent.filter((s) => s === "seal")).toHaveLength(1);
  const moves = sent.slice(0, -1) as { op: string; values: { polyphony: number } }[];
  expect(moves.length).toBeGreaterThanOrEqual(1);
  // An int parameter sends whole numbers, rising with the drag.
  for (const move of moves) {
    expect(move.op).toBe("set_device_params");
    expect(Number.isInteger(move.values.polyphony)).toBe(true);
  }
  expect(moves.at(-1)?.values.polyphony).toBeGreaterThan(16);
});

test("a parameter edit, an agent's included, updates its card in place (card_updates_in_place_for_param_edits)", async ({ page }) => {
  await openDevices(page, devicesProject());
  await showTrack(page, "b");
  const comp = card(page, "Compressor");
  await comp.evaluate((element) => ((element as HTMLElement & { tagged?: boolean }).tagged = true));
  const threshold = comp.getByRole("slider", { name: "Threshold", exact: true });
  await expect(threshold).toHaveAttribute("aria-valuenow", "-18");
  const edited = devicesProject((t) =>
    t.map((x) => (x.id === "b" ? { ...x, inserts: [device("dev_comp", "sonara.dynamics.compressor", { params: { ...defaults("sonara.dynamics.compressor"), threshold_db: -30, mix: 0.8 } })] } : x)),
  );
  await refreshDevices(page, edited, ["dev_comp"], 2, "agent");
  await expect(threshold).toHaveAttribute("aria-valuenow", "-30");
  await expect(threshold).toHaveAttribute("aria-valuetext", "-30.0 dB");
  await expect(comp.getByRole("slider", { name: "Mix", exact: true })).toHaveAttribute("aria-valuenow", "0.8");
  // Not made again: the element is the one tagged before the edit. The page asked for that device
  // alone, not for the whole view (the device refresh).
  expect(await comp.evaluate((element) => (element as HTMLElement & { tagged?: boolean }).tagged)).toBe(true);
  const asked = (await calls(page)).filter((c) => c.cmd === "project_devices" || c.cmd === "project_view");
  expect(asked.map((c) => [c.cmd, c.cmd === "project_view" ? null : c.args])).toEqual([
    ["project_view", null],
    ["project_devices", { deviceIds: ["dev_comp"] }],
  ]);
  // A new chain makes the cards again.
  const replaced = devicesProject((t) => t.map((x) => (x.id === "b" ? { ...x, inserts: [device("dev_comp2", "sonara.dynamics.compressor")] } : x)));
  await refresh(page, replaced, 3);
  await expect(comp.getByRole("slider", { name: "Threshold", exact: true })).toHaveAttribute("aria-valuenow", "-18");
  expect(await comp.evaluate((element) => (element as HTMLElement & { tagged?: boolean }).tagged ?? false)).toBe(false);
});

test("the sidechain source is one step that keys the device from another track", async ({ page }) => {
  await openDevices(page, devicesProject());
  await showTrack(page, "b");
  const source = card(page, "Compressor").getByRole("combobox", { name: "Sidechain source" });
  // None, then the audio, instrument and bus tracks but the device's own.
  await expect(source.locator("option")).toHaveText(["None", "FX", "Synth", "Drums", "Vox"]);
  await expect(source).toHaveValue("");
  await source.selectOption({ label: "Drums" });
  await expect.poll(() => steps(page)).toEqual([{ op: "set_sidechain", device_id: "dev_comp", source_track_id: "d" }]);
  const keyed = devicesProject((t) =>
    t.map((x) =>
      x.id === "b"
        ? { ...x, inserts: [device("dev_comp", "sonara.dynamics.compressor", { sidechain_id: "d", params: { ...defaults("sonara.dynamics.compressor"), sidechain: true } })] }
        : x,
    ),
  );
  await refresh(page, keyed);
  await expect(source).toHaveValue("d");
  await expect(card(page, "Compressor").getByRole("button", { name: "External Sidechain", exact: true })).toHaveAttribute("aria-pressed", "true");
  await source.selectOption({ label: "None" });
  await expect.poll(async () => (await steps(page)).at(-1)).toEqual({ op: "set_sidechain", device_id: "dev_comp", source_track_id: null });
});

/** The rows where the canvas's column at `freq` has the accent colour, in CSS pixels (their mean). */
async function curveY(c: Locator, freq: number): Promise<number | null> {
  return c.locator("canvas.curve").evaluate((canvas, args) => {
    const element = canvas as HTMLCanvasElement;
    const ctx = element.getContext("2d");
    if (!ctx) return null;
    const scale = element.width / args.width;
    const x = Math.round(args.x * scale);
    const column = ctx.getImageData(x, 0, 1, element.height).data;
    const rows: number[] = [];
    for (let y = 0; y < element.height; y++) {
      const [r, g, b] = [column[y * 4], column[y * 4 + 1], column[y * 4 + 2]];
      if (Math.abs(r - 76) < 24 && Math.abs(g - 155) < 24 && Math.abs(b - 240) < 24) rows.push(y);
    }
    return rows.length > 0 ? rows.reduce((a, b) => a + b, 0) / rows.length / scale : null;
  }, { x: xOf(freq, CURVE_SIZE.width), width: CURVE_SIZE.width });
}

test("the EQ curve draws sonara-app's response and follows the gain and the output (eq_curve_reflects_gain)", async ({ page }) => {
  await openDevices(page, devicesProject((t) => [...t, { ...track("e", "Mix", "bus"), inserts: [device("dev_eq", "sonara.eq.parametric")] }]), {
    eq_response: flatCurve(6),
  });
  await showTrack(page, "e");
  const eq = card(page, plugin("sonara.eq.parametric").name);
  await expect.poll(async () => (await calls(page)).filter((c) => c.cmd === "eq_response").length).toBeGreaterThan(0);
  expect((await calls(page)).find((c) => c.cmd === "eq_response")?.args).toEqual({ deviceId: "dev_eq" });
  await expect.poll(() => curveY(eq, 300)).toBeCloseTo(yOf(6, CURVE_SIZE.height), 0);
  // An edit of its parameters asks for the curve again: the output gain moves it down.
  await answer(page, "eq_response", flatCurve(3));
  const before = (await calls(page)).filter((c) => c.cmd === "eq_response").length;
  const edited = devicesProject((t) => [
    ...t,
    { ...track("e", "Mix", "bus"), inserts: [device("dev_eq", "sonara.eq.parametric", { params: { ...defaults("sonara.eq.parametric"), output_db: -3 } })] },
  ]);
  await refreshDevices(page, edited, ["dev_eq"]);
  await expect.poll(async () => (await calls(page)).filter((c) => c.cmd === "eq_response").length).toBeGreaterThan(before);
  await expect.poll(() => curveY(eq, 300)).toBeCloseTo(yOf(3, CURVE_SIZE.height), 0);
});

/** A live frame of the meter layout `layout` with these gain reductions (NaN for none). */
function gainFrame(layout: number, gains: number[]): string {
  const head = frameHex({ meterLayout: layout, meters: [] });
  const bytes = new Uint8Array(head.match(/../g)?.map((pair) => parseInt(pair, 16)) ?? []);
  const out = new Uint8Array(bytes.length + gains.length * 4);
  out.set(bytes);
  const data = new DataView(out.buffer);
  data.setUint32(116, gains.length, true);
  gains.forEach((gain, i) => data.setFloat32(bytes.length + i * 4, gain, true));
  return [...out].map((b) => b.toString(16).padStart(2, "0")).join("");
}

test("a dynamics card shows the live gain reduction of its device (live data)", async ({ page }) => {
  await openDevices(page, devicesProject());
  await showTrack(page, "b");
  const readout = card(page, "Compressor").getByTitle("Gain reduction (live)");
  await expect(readout).toHaveText("GR  –");
  // FX's ten effects, Synth, Drums, then Bass's synth and compressor.
  const gains = Array.from({ length: 14 }, () => Number.NaN);
  gains[13] = 4.5;
  await sendChannel(page, gainFrame(9, gains));
  await expect(readout).toHaveText("GR  4.5 dB");
  // A frame of another layout lands nowhere; a device without a value keeps the last one.
  gains[13] = 9;
  await sendChannel(page, gainFrame(8, gains));
  gains[13] = Number.NaN;
  await sendChannel(page, gainFrame(9, gains));
  await page.waitForTimeout(100);
  await expect(readout).toHaveText("GR  4.5 dB");
});

test("the panel follows the selection and the current device; a deleted track clears it", async ({ page }) => {
  await openDevices(page, devicesProject(), { edit: { ...OUTCOME, result: { device_id: "dev_new" } } });
  await showTrack(page, "s");
  await expect(panel(page).getByText("Synth — devices")).toBeVisible();
  // A header click elsewhere shows that track.
  await page.locator('.header[data-track="d"] .name').click();
  await expect(panel(page).getByText("Drums — devices")).toBeVisible();
  // A click on a card makes its device the current one, outlined.
  const drums = card(page, plugin("sonara.drums").name);
  await drums.locator(".name").click();
  await expect(drums).toHaveClass(/selected/);
  // The mixer's insert of another track shows that track, its device outlined.
  await page.getByRole("tab", { name: "Mixer" }).click();
  await page.getByRole("group", { name: "Channel strip Bass" }).getByRole("listitem").nth(1).click();
  await expect(devicesTab(page)).toHaveAttribute("aria-selected", "true");
  await expect(panel(page).getByText("Bass — devices")).toBeVisible();
  await expect(card(page, "Compressor")).toHaveClass(/selected/);
  await expect(card(page, plugin("sonara.synth.poly").name)).not.toHaveClass(/selected/);
  // "+ Effect" adds at the end of the chain and makes the new device the current one.
  await panel(page).getByRole("button", { name: "+ Effect" }).click();
  await page.getByRole("menu", { name: "Add effect" }).getByRole("menuitem", { name: "Delay" }).click();
  await page.getByRole("menu", { name: "Delay" }).getByRole("menuitem", { name: "Delay" }).click();
  await expect.poll(async () => (await edits(page)).at(-1)).toEqual({ op: "add_device", track_id: "b", plugin_id: "sonara.delay" });
  const added = devicesProject((t) =>
    t.map((x) => (x.id === "b" ? { ...x, inserts: [...x.inserts, device("dev_new", "sonara.delay")] } : x)),
  );
  await refresh(page, added);
  await expect(card(page, "Delay")).toHaveClass(/selected/);
  await expect(card(page, "Compressor")).not.toHaveClass(/selected/);
  // The row scrolled to the new current card, once: a refresh of another device does not scroll it
  // back there.
  const cardsRow = panel(page).locator(".cards");
  await expect.poll(() => cardsRow.evaluate((e) => e.scrollLeft)).toBeGreaterThan(0);
  await cardsRow.evaluate((e) => (e.scrollLeft = 0));
  const tweaked = devicesProject((t) =>
    t.map((x) =>
      x.id === "b"
        ? {
            ...x,
            instrument: device("dev_bass", "sonara.synth.poly", { params: { ...defaults("sonara.synth.poly"), polyphony: 8 } }),
            inserts: [...x.inserts, device("dev_new", "sonara.delay")],
          }
        : x,
    ),
  );
  await refreshDevices(page, tweaked, ["dev_bass"], 3);
  await expect(card(page, plugin("sonara.synth.poly").name).getByRole("slider", { name: "Polyphony", exact: true })).toHaveAttribute(
    "aria-valuenow",
    "8",
  );
  await page.waitForTimeout(100);
  expect(await cardsRow.evaluate((e) => e.scrollLeft)).toBe(0);
  // The track deleted elsewhere: no cards, and the first hint.
  await refresh(page, devicesProject((t) => t.filter((x) => x.id !== "b")), 4);
  await expect(panel(page).getByRole("group", { name: /^Device / })).toHaveCount(0);
  await expect(panel(page).getByText("Select a track to edit its instrument and effects.")).toBeVisible();
});

test("a device whose plugin this build lacks says so, and can still be removed", async ({ page }) => {
  await openDevices(
    page,
    devicesProject((t) => t.map((x) => (x.id === "a" ? { ...x, inserts: [device("dev_gone", "vendor.gone")] } : x))),
  );
  await showTrack(page, "a");
  const gone = card(page, "vendor.gone");
  await expect(gone.getByText("Plugin 'vendor.gone' is not available in this build; the device is bypassed.")).toBeVisible();
  await expect(gone.locator(".param")).toHaveCount(0);
  await gone.getByRole("button", { name: "Device actions" }).click();
  await expect(page.getByRole("menu", { name: "Device actions" }).getByRole("menuitem")).toHaveText(["Remove"]);
});
