import { expect, test } from "@playwright/test";
import {
  FADER_SPEC,
  MIN_DB,
  PAN_SPEC,
  type ControlSpec,
  dbText,
  faderFromNorm,
  faderToNorm,
  formatDb,
  formatPan,
  formatValue,
  keyValue,
  linearMapping,
  parseDb,
  parsePyFloat,
  valueAtNorm,
  wheelValue,
} from "../../src/lib/mixer/controls";
import { ValueDriver, RELEASE_MS } from "../../src/lib/mixer/drive";
import { FLOOR_DB, MeterBallistics, MeterBank, meterIndex, toDb } from "../../src/lib/mixer/meter";
import { dropIndex, effectMenu, mixedTracks, moveIndex, sendMenu, subtitle } from "../../src/lib/mixer/strip";
import type { GestureDeps } from "../../src/lib/gesture";
import type { Edit, PluginGroup } from "../../src/lib/types";
import type { LiveFrame } from "../../src/lib/live/frame";
import { project, track } from "../support/project";

// The mixer's pure parts (inventory MX and CT, U): the prototype's tests/gui/test_mixer_devices.py
// `fader_taper_is_monotonic_and_invertible`, `formatting`, `meter_clip_latch` (the ballistics),
// the key steps of `knob_keyboard_and_reset`, `knob_drag_and_int_quantize` and `fader_keyboard`,
// and the reorder arithmetic of `add_effect_from_menu_and_bypass_and_reorder`.

const approx = (value: number, expected: number, digits = 6) => expect(value).toBeCloseTo(expected, digits);

test("the fader taper is monotonic and invertible", () => {
  const dbs = [-60, -40, -20, -6, 0, 6, 12];
  const norms = dbs.map(faderToNorm);
  expect(norms).toEqual([...norms].sort((a, b) => a - b));
  expect(faderToNorm(MIN_DB)).toBe(0);
  expect(faderToNorm(0)).toBeGreaterThan(0.65);
  expect(faderToNorm(0)).toBeLessThan(0.75);
  for (const db of dbs) approx(faderFromNorm(faderToNorm(db)), db);
  expect(faderFromNorm(0)).toBe(MIN_DB);
  // Past the top it stays at +12; the bottom 0.2 % is −∞.
  expect(faderToNorm(20)).toBe(1);
  expect(faderFromNorm(0.002)).toBe(MIN_DB);
});

test("formatting follows the prototype's", () => {
  expect(formatDb(MIN_DB)).toBe("-inf");
  expect(formatDb(-6.04)).toBe("-6.0");
  expect(formatDb(0.04)).toBe("0.0");
  expect(formatDb(3)).toBe("+3.0");
  expect(dbText(-12)).toBe("-12.0 dB");
  expect(formatPan(0)).toBe("C");
  expect(formatPan(-0.3)).toBe("L30");
  expect(formatPan(1)).toBe("R100");
  expect(formatPan(0.125)).toBe("R12");
  expect(formatValue(1500, "Hz")).toBe("1.50 kHz");
  expect(formatValue(120, "Hz")).toBe("120 Hz");
  expect(formatValue(0.05, "s")).toBe("50 ms");
  expect(formatValue(4, ":1")).toBe("4.0:1");
  expect(formatValue(-6, "dB")).toBe("-6.0 dB");
  expect(formatValue(12.5, "semitones")).toBe("+12 st");
  expect(formatValue(250)).toBe("250");
});

test("the typed dB accepts the prototype's forms and clamps", () => {
  expect(parseDb("-12.5")).toBe(-12.5);
  expect(parseDb(" -6 dB ")).toBe(-6);
  expect(parseDb("-6DB")).toBe(-6);
  expect(parseDb("-inf")).toBe(MIN_DB);
  expect(parseDb("inf")).toBe(MIN_DB);
  expect(parseDb("-∞")).toBe(MIN_DB);
  expect(parseDb("40")).toBe(12);
  expect(parseDb("-500")).toBe(MIN_DB);
  expect(parseDb("1e1")).toBe(10);
  expect(parseDb("+3")).toBe(3);
  expect(parseDb(".5")).toBe(0.5);
  // float("nan") passes, and Python's min(12.0, nan) is 12.0.
  expect(parseDb("nan")).toBe(12);
  expect(parseDb("-infinity")).toBe(MIN_DB);
  expect(parseDb("loud")).toBeNull();
  expect(parseDb("")).toBeNull();
  expect(parseDb("1,5")).toBeNull();
  expect(parsePyFloat("1_000.5")).toBe(1000.5);
  expect(parsePyFloat("1__0")).toBeNull();
});

test("the keys step, page, jump and reset as ValueControl's", () => {
  // `Knob(0, 1, 0.5, default=0.25)`.
  const mix: ControlSpec = { min: 0, max: 1, defaultValue: 0.25, mapping: linearMapping(0, 1), step: 0.01 };
  let value = 0.5;
  value = keyValue(mix, "ArrowUp", false, value) ?? value;
  approx(value, 0.51);
  value = keyValue(mix, "ArrowDown", true, value) ?? value;
  approx(value, 0.509);
  approx(keyValue(mix, "PageUp", false, value) ?? Number.NaN, 0.609);
  value = keyValue(mix, "End", false, value) ?? value;
  expect(value).toBe(1);
  value = keyValue(mix, "Delete", false, value) ?? value;
  expect(value).toBe(0.25);
  expect(keyValue(mix, "Backspace", false, 0.9)).toBe(0.25);
  expect(keyValue(mix, "Home", false, 0.9)).toBe(0);
  expect(keyValue(mix, "a", false, 0.9)).toBeNull();
  // `fader_keyboard`: Home is −∞, the reset 0 dB, ↑ above it.
  expect(keyValue(FADER_SPEC, "Home", false, 0)).toBe(MIN_DB);
  expect(keyValue(FADER_SPEC, "Delete", false, -20)).toBe(0);
  expect(keyValue(FADER_SPEC, "ArrowUp", false, 0)).toBeGreaterThan(0);
  // The pan knob's → moves right.
  expect(keyValue(PAN_SPEC, "ArrowRight", false, 0)).toBeGreaterThan(0);
});

test("an integer control quantizes its drags and steps", () => {
  // `Knob(1, 32, 16, quantize=round, step=1/31)`.
  const voices: ControlSpec = {
    min: 1,
    max: 32,
    defaultValue: 16,
    mapping: linearMapping(1, 32),
    step: 1 / 31,
    quantize: (v) => Math.round(v),
  };
  const dragged = valueAtNorm(voices, (16 - 1) / 31 + 70 / 220, 16);
  expect(dragged).toBeGreaterThan(16);
  expect(dragged).toBe(Math.round(dragged));
  expect(keyValue(voices, "ArrowDown", false, dragged)).toBe(dragged - 1);
  // A wheel notch is one step, ⇧ a tenth of one.
  approx(wheelValue(PAN_SPEC, -60, false, 0), 0.02);
  approx(wheelValue(PAN_SPEC, -60, true, 0), 0.002);
});

test("the meter's ballistics: the levels in dB, the falls, the hold and the clip latch", () => {
  // `meter_clip_latch`.
  const meter = new MeterBallistics(2, FLOOR_DB, 0);
  meter.setLevels([0.5, 0.25], [0.3, 0.1], 0);
  expect(meter.clipped).toBe(false);
  approx(meter.peakDb(0), -6.02, 1);
  approx(meter.rms[1], toDb(0.1));
  meter.setLevels([1.1, 0.2], null, 0);
  expect(meter.clipped).toBe(true);
  expect(meter.clearClip()).toBe(true);
  expect(meter.clipped).toBe(false);
  expect(meter.clearClip()).toBe(false);
  // The peak and the RMS fall at 24 dB/s; the hold waits 1.5 s, then falls at 36 dB/s.
  const fresh = new MeterBallistics(1, FLOOR_DB, 0);
  fresh.setLevels([1], [0.5], 0);
  fresh.decay(1);
  approx(fresh.peak[0], -24);
  approx(fresh.rms[0], toDb(0.5) - 24);
  approx(fresh.hold[0], 0);
  fresh.decay(1.5);
  approx(fresh.hold[0], 0);
  // Past the hold, each decay drops it by 1.5 times the fall of its interval.
  fresh.decay(2);
  approx(fresh.hold[0], -18);
  approx(fresh.peak[0], -48);
  fresh.decay(10);
  expect(fresh.peak[0]).toBe(FLOOR_DB);
  expect(fresh.hold[0]).toBe(FLOOR_DB);
  expect(fresh.active).toBe(false);
  // A missing RMS is the peak × 0.707; a mono level feeds both channels.
  const mono = new MeterBallistics(2, FLOOR_DB, 0);
  mono.setLevels([0.5], null, 0);
  approx(mono.rms[1], toDb(0.5 * 0.707));
  approx(mono.norm(0), 60 / 66);
  expect(mono.norm(-100)).toBe(0);
});

function frame(layout: number, levels: number[][]): LiveFrame {
  return {
    sequence: 1,
    engine: "running",
    state: "playing",
    position: { frame: 0, beat: 0, seconds: 0 },
    device_stopped: false,
    sample_rate: 48000,
    buffer_frames: 256,
    render_load: 0,
    callback_load: 0,
    output_latency_ms: 0,
    lookahead_ms: 0,
    queued_ms: 0,
    underruns: 0,
    device_errors: 0,
    version: 1,
    meter_layout: layout,
    meters: levels.map(([l, r]) => ({ peak: [l, r], rms: [l / 2, r / 2] })),
    gain_reduction: [],
  };
}

test("the bank feeds each strip its track's meter when the layout matches", () => {
  const open = project([track("a", "A"), track("f", "F", "folder"), track("b", "B")], { meter_layout: 7 });
  expect(meterIndex(open, "b")).toBe(2);
  expect(meterIndex(open, "master")).toBe(3);
  const bank = new MeterBank();
  const fakeCanvas = (id: string) => ({ id, setAttribute: () => {} }) as unknown as HTMLCanvasElement;
  const forB = fakeCanvas("b");
  const forMaster = fakeCanvas("master");
  bank.register(forB, () => "b", 0);
  bank.register(forMaster, () => "master", 0);
  bank.update(0, frame(7, [[0.1, 0.1], [0, 0], [0.5, 0.25], [1.2, 0.9]]), open);
  approx(bank.meterOf("b")?.peakDb(0) ?? 0, toDb(0.5));
  expect(bank.meterOf("master")?.clipped).toBe(true);
  // A frame of another layout (the tracks changed) reaches no meter; the levels only fall.
  bank.update(0.5, frame(8, [[1, 1], [1, 1], [1, 1], [1, 1]]), open);
  approx(bank.meterOf("b")?.peakDb(0) ?? 0, toDb(0.5) - 12);
  expect(bank.clearClip(forMaster)).toBe(true);
  expect(bank.meterOf("master")?.clipped).toBe(false);
});

test("strips: the mixed tracks, the subtitles and the reorder arithmetic", () => {
  const folder = track("f", "Group", "folder");
  const child = { ...track("c", "Child", "audio"), parent_id: "f", depth: 1 };
  const keys = {
    ...track("k", "Keys", "instrument"),
    instrument: {
      id: "dev_synth",
      plugin_id: "sonara.synth.poly",
      name: "Sonara Poly Synth",
      enabled: true,
      preset: null,
      sidechain_id: null,
      available: true,
      params: {},
    },
  };
  const bus = track("v", "Verb", "bus");
  const open = project([folder, child, keys, bus]);
  expect(mixedTracks(open).map((t) => t.id)).toEqual(["c", "k", "v"]);
  expect(subtitle(child, open)).toBe("▸ Group");
  expect(subtitle(keys, open)).toBe("Sonara Poly Synth");
  expect(subtitle(bus, open)).toBe("Bus");
  expect(subtitle(open.master, open)).toBe("Master");
  // A drop on a row: the instrument's row is not an insert; below the rows is the end.
  expect(dropIndex(0, 3, true)).toBe(0);
  expect(dropIndex(2, 3, true)).toBe(1);
  expect(dropIndex(-1, 3, true)).toBe(2);
  expect(dropIndex(-1, 2, false)).toBe(2);
  // `_move_device`: the index counts the places before the device is taken out.
  expect(moveIndex(["eq", "comp"], "comp", 0)).toBe(0);
  expect(moveIndex(["eq", "comp", "delay"], "eq", 2)).toBe(1);
  expect(moveIndex(["eq", "comp", "delay"], "eq", 3)).toBe(2);
  expect(moveIndex(["eq", "comp"], "eq", 1)).toBeNull();
  expect(moveIndex(["eq", "comp"], "eq", 0)).toBeNull();
  expect(moveIndex(["eq"], "synth", 0)).toBeNull();
});

test("the effect and send menus", () => {
  const picked: string[] = [];
  const groups: PluginGroup[] = [
    { category: "delay", title: "Delay", plugins: [{ id: "sonara.delay", name: "Delay", description: "Echoes" }] },
    { category: "eq", title: "EQ", plugins: [{ id: "sonara.eq.parametric", name: "Parametric EQ", description: "" }] },
  ];
  const menu = effectMenu(groups, (id) => picked.push(id));
  expect(menu.map((item) => (item === "separator" ? "—" : item.label))).toEqual(["Delay", "EQ"]);
  const eq = menu[1];
  if (eq !== "separator" && "items" in eq) {
    const item = eq.items[0];
    if (item !== "separator" && "run" in item) item.run();
  }
  expect(picked).toEqual(["sonara.eq.parametric"]);
  // One category: its plugins directly, with the description as the tooltip.
  const flat = effectMenu(groups.slice(0, 1), (id) => picked.push(id));
  expect(flat).toHaveLength(1);
  expect(flat[0] !== "separator" && "title" in flat[0] ? flat[0].title : null).toBe("Echoes");
  const vox = { ...track("a", "Vox", "audio"), send_targets: [{ id: "v", name: "Verb" }] };
  const sends = sendMenu(vox, (id) => picked.push(id));
  expect(sends.map((item) => (item === "separator" ? "—" : item.label))).toEqual(["Verb"]);
  expect(sendMenu(track("b", "Bus", "bus"), () => {})).toEqual([{ label: "No buses available", disabled: true }]);
});

function driverHarness() {
  const log: string[] = [];
  const frames: (() => void)[] = [];
  let pending: (() => void)[] = [];
  const deps: GestureDeps = {
    send: (edit) =>
      new Promise<void>((resolve) => {
        log.push(`send ${(edit as { volume_db: number }).volume_db}`);
        pending.push(resolve);
      }),
    seal: async () => {
      log.push("seal");
    },
    nextFrame: (callback) => frames.push(callback),
    onError: () => {},
  };
  const frame = () => {
    for (const callback of frames.splice(0)) callback();
  };
  const land = () => {
    const open = pending;
    pending = [];
    for (const resolve of open) resolve();
  };
  const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
  return { deps, log, frame, land, settle };
}

const volume = (db: number): Edit => ({ op: "set_track_mixer", track_id: "t", volume_db: db });

test("a control's gesture sends per frame, seals at its end and keeps its value until the view has it", async () => {
  const h = driverHarness();
  const shown: (number | null)[] = [];
  const driver = new ValueDriver(() => h.deps, volume, (local) => shown.push(local));
  driver.change(-1, 0);
  driver.change(-2, 0);
  expect(driver.shown(0)).toBe(-2);
  h.frame();
  await h.settle();
  expect(h.log).toEqual(["send -2"]);
  // An unchanged value sends nothing.
  driver.change(-2, 0);
  const ended = driver.finish();
  h.land();
  await h.settle();
  await ended;
  expect(h.log).toEqual(["send -2", "seal"]);
  // The model's value arrives: the control shows it again.
  expect(driver.shown(0)).toBe(-2);
  driver.modelChanged(-2);
  expect(driver.local).toBeNull();
  expect(shown).toEqual([-1, -2, null]);
});

test("a key step right after another waits for its seal", async () => {
  const h = driverHarness();
  const driver = new ValueDriver(() => h.deps, volume);
  driver.change(-1, 0);
  const first = driver.finish();
  driver.change(-2, 0);
  const second = driver.finish();
  // The first step's edit goes out at once (the gesture ends); the second waits for its seal.
  await h.settle();
  expect(h.log).toEqual(["send -1"]);
  h.land();
  await h.settle();
  await first;
  await h.settle();
  expect(h.log).toEqual(["send -1", "seal", "send -2"]);
  h.land();
  await second;
  expect(h.log).toEqual(["send -1", "seal", "send -2", "seal"]);
});

test("a finished gesture's value gives way after a while when the view never has it", async () => {
  const h = driverHarness();
  const driver = new ValueDriver(() => h.deps, volume);
  driver.change(-3, 0);
  const ended = driver.finish();
  await h.settle();
  h.land();
  await ended;
  expect(driver.local).toBe(-3);
  await new Promise((resolve) => setTimeout(resolve, RELEASE_MS + 50));
  expect(driver.local).toBeNull();
});
