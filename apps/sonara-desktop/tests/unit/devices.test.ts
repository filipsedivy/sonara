import { expect, test } from "@playwright/test";
import oracle from "../fixtures/device-editors-v1.json" with { type: "json" };
import { CurveLoader, DB_RANGE, F_MAX, F_MIN, responseAt, xOf, yOf } from "../../src/lib/devices/eq";
import { devicesTrackAfter } from "../../src/lib/devices/focus";
import {
  canAddEffect,
  chainOf,
  comboChoice,
  controlOf,
  deviceIndex,
  emptyText,
  findTrack,
  gainReductionOf,
  gainReductionText,
  groupParams,
  groupRows,
  headerText,
  instrumentCombo,
  knobValue,
  missingText,
  ownerOf,
  pluginLabel,
  presetIndex,
  presetItems,
  shortLabel,
  sidechainOptions,
  toggleText,
  tooltipOf,
} from "../../src/lib/devices/model";
import type { LiveFrame } from "../../src/lib/live/frame";
import { keyValue, normOf, pageOf, userValue } from "../../src/lib/mixer/controls";
import { EMPTY_SELECTION, selectTrack, setCurrentDevice } from "../../src/lib/state/selection";
import type { DeviceView, EqResponse, LibraryChoices, ParamView } from "../../src/lib/types";
import { GOLDEN, PLUGINS, defaults, plugin } from "../support/plugins";
import { project, track } from "../support/project";

// The device editors' model against two oracles:
//
// - golden/plugins/catalog.json (tests/fixtures/plugin-catalog.json, its checked copy): every
//   built-in plugin's descriptor, so every parameter of every plugin gets its control, range,
//   default and unit;
// - tests/fixtures/device-editors-v1.json: what the prototype's editor makes of each of them
//   (`group_params`, `short_label` and `ParamWidget` of src/sonara/ui/devices/panel.py at the
// descriptor's defaults), recorded on the host from the frozen prototype (GLD-01, : the
//   prototype is only read). The recorder, run from the repository root as
//   `.venv/bin/python -I record.py > apps/sonara-desktop/tests/fixtures/device-editors-v1.json`:
//
//     import json, sys
//     from PySide6.QtWidgets import QApplication, QLabel
//     qt = QApplication(["oracle", "-platform", "offscreen"])
//     from sonara.plugins.builtin import create_registry
//     from sonara.ui.devices.panel import ParamWidget, group_params, short_label
//     out = []
//     for d in sorted(create_registry().all(), key=lambda d: d.id):
//         groups = []
//         for key, title, specs in group_params(d.params):
//             items = []
//             for s in specs:
//                 label = short_label(s, title)
//                 w = ParamWidget(s, label)
//                 w.set_value(s.default)
//                 if w.knob is not None:
//                     k = w.knob
//                     c = {"control": "knob", "min": k.minimum, "max": k.maximum, "default": k.default,
//                          "bipolar": k.bipolar, "step": k.STEP, "page": k.PAGE, "norm": k.norm(),
//                          "text": k.text(), "tooltip": k.toolTip()}
//                 elif w.toggle is not None:
//                     c = {"control": "toggle", "text": w.toggle.text(), "checked": w.toggle.isChecked(),
//                          "tooltip": w.findChild(QLabel).toolTip()}
//                 else:
//                     c = {"control": "combo", "items": [w.combo.itemText(i) for i in range(w.combo.count())],
//                          "current": w.combo.currentText(), "tooltip": w.combo.toolTip()}
//                 items.append({"id": s.id, "label": label, **c})
//             groups.append({"key": key, "title": title, "params": items})
//         out.append({"id": d.id, "groups": groups})
//     SOURCE = ("The prototype's device editor widgets for every built-in plugin at its defaults: group_params, short_label "
//               "and ParamWidget (src/sonara/ui/devices/panel.py) with ParamWidget.set_value(default), recorded on the host "
//               "from the frozen prototype; the recorder is in tests/unit/devices.test.ts (GLD-01: never edited by hand).")
//     json.dump({"format": 1, "source": SOURCE, "plugins": out}, sys.stdout, indent=1, ensure_ascii=False)
//     sys.stdout.write("\n")
//
// Inventory §7 maps `grouping_helpers` and the unit halves of `every_builtin_plugin_has_a_complete_editor`
// and `knob_drag_and_int_quantize` here.

type OracleParam = (typeof oracle.plugins)[number]["groups"][number]["params"][number];

test("the catalog has every built-in plugin: 10 effects and 2 instruments", () => {
  expect(PLUGINS.filter((p) => p.kind === "effect")).toHaveLength(10);
  expect(PLUGINS.filter((p) => p.kind === "instrument").map((p) => p.id).sort()).toEqual(["sonara.drums", "sonara.synth.poly"]);
  expect([...PLUGINS.map((p) => p.id)].sort()).toEqual(oracle.plugins.map((p) => p.id));
});

test("every built-in plugin's editor has one control per parameter, with its range, default and unit", () => {
  for (const d of PLUGINS) {
    const groups = groupParams(d.params);
    const shown = groups.flatMap((g) => g.params.map((p) => p.id));
    expect([...shown].sort(), d.id).toEqual(d.params.map((p) => p.id).sort());
    expect(new Set(shown).size, d.id).toBe(shown.length);
    for (const param of d.params) {
      const golden = GOLDEN.find((g) => g.id === d.id)?.params.find((p) => p.id === param.id);
      const control = controlOf(param);
      const where = `${d.id} ${param.id}`;
      if (param.kind === "float" || param.kind === "int") {
        if (control.kind !== "knob") throw new Error(`${where}: not a knob`);
        expect(control.spec.min, where).toBe(golden?.minimum);
        expect(control.spec.max, where).toBe(golden?.maximum);
        expect(control.spec.defaultValue, where).toBe(Number(golden?.default));
        expect(control.integer, where).toBe(param.kind === "int");
        // Log or linear: the mapping's midpoint is the geometric or the arithmetic mean.
        const [lo, hi] = [control.spec.min, control.spec.max];
        const mid = golden?.scale === "log" ? Math.sqrt(lo * hi) : (lo + hi) / 2;
        expect(normOf(control.spec, mid), where).toBeCloseTo(0.5, 12);
        if (param.unit) expect(tooltipOf(param), where).toContain(`(${param.unit})`);
      } else if (param.kind === "bool") {
        expect(control.kind, where).toBe("toggle");
      } else {
        if (control.kind !== "combo") throw new Error(`${where}: not a combo`);
        expect(control.choices, where).toEqual(golden?.choices);
        expect(control.choices, where).toContain(String(golden?.default));
      }
    }
  }
});

test("the editors are the prototype's: groups, labels, controls, ranges, steps, positions, texts and tooltips", () => {
  for (const recorded of oracle.plugins) {
    const d = plugin(recorded.id);
    const groups = groupParams(d.params);
    expect(groups.map((g) => [g.key, g.title]), d.id).toEqual(recorded.groups.map((g) => [g.key, g.title]));
    groups.forEach((group, gi) => {
      const expectedParams: OracleParam[] = recorded.groups[gi].params;
      expect(group.params.map((p) => p.id), `${d.id} ${group.key}`).toEqual(expectedParams.map((p) => p.id));
      group.params.forEach((param: ParamView, pi) => {
        const want = expectedParams[pi];
        const where = `${d.id} ${param.id}`;
        expect(shortLabel(param, group.title), where).toBe(want.label);
        const control = controlOf(param);
        expect(control.kind, where).toBe(want.control);
        if (control.kind === "knob" && want.control === "knob") {
          expect(control.spec.min, where).toBe(want.min);
          expect(control.spec.max, where).toBe(want.max);
          expect(control.spec.defaultValue, where).toBe(want.default);
          expect(control.bipolar, where).toBe(want.bipolar);
          expect(control.spec.step, where).toBeCloseTo(want.step as number, 15);
          expect(pageOf(control.spec.step), where).toBeCloseTo(want.page as number, 15);
          expect(normOf(control.spec, control.spec.defaultValue), where).toBeCloseTo(want.norm as number, 12);
          expect(control.format(control.spec.defaultValue), where).toBe(want.text);
          expect(tooltipOf(param), where).toBe(want.tooltip);
        } else if (control.kind === "toggle") {
          expect(toggleText(Boolean(param.default)), where).toBe(want.text);
          expect(Boolean(param.default), where).toBe(want.checked);
          expect(tooltipOf(param), where).toBe(want.tooltip);
        } else if (control.kind === "combo") {
          expect(control.choices, where).toEqual(want.items);
          expect(comboChoice(control.choices, param.default), where).toBe(want.current);
          expect(tooltipOf(param), where).toBe(want.tooltip);
        }
      });
    });
  }
});

test("grouping helpers: prefixes make groups, a bare ID joins its group, lone prefixes stay in Main (grouping_helpers)", () => {
  const synth = plugin("sonara.synth.poly");
  const groups = Object.fromEntries(groupParams(synth.params).map((g) => [g.key, g.params.map((p) => p.id)]));
  expect(groups.osc1).toContain("osc1_wave");
  expect(groups.unison).toContain("unison");
  expect(groups.main).toContain("polyphony");
  expect(Object.values(groups).flat()).toHaveLength(synth.params.length);
  const wave = synth.params.find((p) => p.id === "osc1_wave");
  if (!wave) throw new Error("no osc1_wave");
  expect(shortLabel(wave, "Osc 1")).toBe("Wave");
  // A group of more than two parameters fills two rows, column by column.
  const all = groupParams(synth.params);
  expect(all.map((g) => groupRows(g))).toEqual(all.map((g) => (g.params.length > 2 ? 2 : 1)));
});

test("an int knob quantizes its changes as Python's round and sends whole numbers (knob_drag_and_int_quantize)", () => {
  const polyphony = plugin("sonara.synth.poly").params.find((p) => p.id === "polyphony");
  if (!polyphony) throw new Error("no polyphony");
  const control = controlOf(polyphony);
  if (control.kind !== "knob") throw new Error("not a knob");
  expect(control.spec.step).toBeCloseTo(1 / 31, 15);
  expect(keyValue(control.spec, "ArrowUp", false, 16)).toBe(17);
  expect(keyValue(control.spec, "PageDown", false, 16)).toBe(6);
  // Ties go to the even number, as Python's round does.
  expect(userValue(control.spec, 2.5, 16)).toBe(2);
  expect(userValue(control.spec, 3.5, 16)).toBe(4);
  expect(knobValue(control, 7.4999)).toBe(7);
  expect(control.format(16)).toBe("16 voices");
  const octave = plugin("sonara.synth.poly").params.find((p) => p.id === "osc1_octave");
  if (!octave) throw new Error("no osc1_octave");
  const octaves = controlOf(octave);
  expect(octaves.kind === "knob" && octaves.format(-1)).toBe("-1");
  // A float is sent as it is.
  const cutoff = plugin("sonara.synth.poly").params.find((p) => p.id === "filter_cutoff");
  if (!cutoff) throw new Error("no filter_cutoff");
  expect(knobValue(controlOf(cutoff), 2512.5)).toBe(2512.5);
});

function device(id: string, pluginId: string, name: string, extra: Partial<DeviceView> = {}): DeviceView {
  return {
    id,
    plugin_id: pluginId,
    name,
    enabled: true,
    preset: null,
    sidechain_id: null,
    available: PLUGINS.some((p) => p.id === pluginId),
    params: PLUGINS.some((p) => p.id === pluginId) ? defaults(pluginId) : {},
    ...extra,
  };
}

const SYNTH = device("dev_synth", "sonara.synth.poly", "Poly Synth");
const COMP = device("dev_comp", "sonara.dynamics.compressor", "Compressor");
const EQ = device("dev_eq", "sonara.eq.parametric", "Parametric EQ");
const LIMIT = device("dev_limit", "sonara.dynamics.limiter", "Limiter");

function chainProject() {
  const bass = { ...track("b", "Bass", "instrument"), instrument: SYNTH, inserts: [COMP, EQ] };
  const kick = { ...track("k", "Kick", "instrument"), instrument: device("dev_drums", "sonara.drums", "Drums") };
  const tracks = [track("f", "Folder", "folder"), bass, kick, track("v", "Verb", "bus"), track("a", "Vox", "audio")];
  const open = project(tracks, { meter_layout: 7 });
  return { ...open, master: { ...open.master, inserts: [LIMIT] } };
}

test("a card's texts and choices: presets, the renamed plugin, a missing plugin, the sidechain sources", () => {
  const synth = plugin("sonara.synth.poly");
  expect(presetItems(synth).slice(0, 3)).toEqual(["Custom", "Init", "Analog Bass"]);
  expect(presetIndex(synth, null)).toBe(0);
  expect(presetIndex(synth, "Analog Bass")).toBe(2);
  expect(presetIndex(synth, "Gone")).toBe(0);
  expect(pluginLabel(device("d", "sonara.synth.poly", "Sonara Poly Synth"), synth)).toBe("");
  expect(pluginLabel(device("d", "sonara.synth.poly", "Lead"), synth)).toBe(synth.name);
  expect(pluginLabel(device("d", "vendor.gone", "Gone"), null)).toBe("");
  expect(missingText(device("d", "vendor.gone", "Gone"))).toBe(
    "Plugin 'vendor.gone' is not available in this build; the device is bypassed.",
  );
  const open = chainProject();
  const bass = findTrack(open, "b");
  if (!bass) throw new Error("no Bass");
  // None, then the audio, instrument and bus tracks but the device's own; no folder, no master.
  expect(sidechainOptions(open, bass)).toEqual([
    { id: "", name: "None" },
    { id: "k", name: "Kick" },
    { id: "v", name: "Verb" },
    { id: "a", name: "Vox" },
  ]);
});

// The sampler's instrument combo. Its choices and libraries are what sonara-app answers for two
// libraries (crates/sonara-app/tests/instrument_catalog.rs pins that answer).
const SAMPLER_CHOICES = ["bare/Tuba", "bare/a/Horn", "bare/b/Horn", "zeta/Ens/Violins", "zeta/Solo/Cello", "zeta/Solo/Violin"];
const SAMPLER_LIBRARIES: LibraryChoices[] = [
  {
    id: "zeta",
    name: "Alpha Strings",
    presets: [
      { id: "zeta/Solo/Cello", label: "Cello" },
      { id: "zeta/Ens/Violins", label: "violin (Ens/Violins)" },
      { id: "zeta/Solo/Violin", label: "Violin (Solo/Violin)" },
    ],
  },
  {
    id: "bare",
    name: "bare",
    presets: [
      { id: "bare/a/Horn", label: "Horn (a/Horn)" },
      { id: "bare/b/Horn", label: "Horn (b/Horn)" },
      { id: "bare/Tuba", label: "Tuba" },
    ],
  },
];

/** A combo's groups as `[title, [label, value, disabled]…]`. */
function comboRows(groups: ReturnType<typeof instrumentCombo>["groups"]): [string | null, [string, string, boolean][]][] {
  return groups.map((g) => [g.title, g.options.map((o) => [o.label, o.value, o.disabled])]);
}

test("the sampler's instrument combo shows preset names under their library, in sonara-app's order", () => {
  const combo = instrumentCombo(SAMPLER_CHOICES, SAMPLER_LIBRARIES, "zeta/Solo/Violin", undefined);
  expect(combo.selected).toBe("zeta/Solo/Violin");
  expect(comboRows(combo.groups)).toEqual([
    [
      "Alpha Strings",
      [
        ["Cello", "zeta/Solo/Cello", false],
        ["violin (Ens/Violins)", "zeta/Ens/Violins", false],
        ["Violin (Solo/Violin)", "zeta/Solo/Violin", false],
      ],
    ],
    [
      "bare",
      [
        ["Horn (a/Horn)", "bare/a/Horn", false],
        ["Horn (b/Horn)", "bare/b/Horn", false],
        ["Tuba", "bare/Tuba", false],
      ],
    ],
  ]);
  // The tooltip names the preset ID.
  expect(combo.groups[0]?.options.map((o) => o.title)).toEqual(["zeta/Solo/Cello", "zeta/Ens/Violins", "zeta/Solo/Violin"]);
  // Every choice once: the options are exactly the descriptor's choices (TST-07).
  const values = combo.groups.flatMap((g) => g.options.map((o) => o.value));
  expect([...values].sort()).toEqual(SAMPLER_CHOICES);
});

test("a choice no library lists still shows, as its ID; a listed preset that is no choice and an empty library do not", () => {
  const libraries: LibraryChoices[] = [
    { id: "zeta", name: "Alpha Strings", presets: [{ id: "zeta/Solo/Cello", label: "Cello" }, { id: "zeta/Gone", label: "Gone" }] },
    { id: "empty", name: "Empty", presets: [{ id: "empty/Nothing", label: "Nothing" }] },
  ];
  const combo = instrumentCombo(["other/Pad", "zeta/Solo/Cello"], libraries, "other/Pad", undefined);
  expect(combo.selected).toBe("other/Pad");
  expect(comboRows(combo.groups)).toEqual([
    ["Alpha Strings", [["Cello", "zeta/Solo/Cello", false]]],
    [null, [["other/Pad", "other/Pad", false]]],
  ]);
});

test("an older project's patch path shows the preset it plays; a value not installed here says so first", () => {
  const path = "/Users/me/Libraries/Zeta/Solo/Cello.sfz";
  // sonara-app reads the path as the preset of that patch: the combo shows that preset.
  const read = instrumentCombo(SAMPLER_CHOICES, SAMPLER_LIBRARIES, path, "zeta/Solo/Cello");
  expect(read.selected).toBe("zeta/Solo/Cello");
  expect(comboRows(read.groups).map(([title]) => title)).toEqual(["Alpha Strings", "bare"]);

  // Not installed here (TST-03): a disabled first option with the instrument's name, never the
  // first preset, which the track does not play.
  const missing = instrumentCombo(SAMPLER_CHOICES, SAMPLER_LIBRARIES, "vsco2ce/Tuba-KS", null);
  expect(missing.selected).toBe("vsco2ce/Tuba-KS");
  expect(comboRows(missing.groups)[0]).toEqual([null, [["Not installed: Tuba-KS", "vsco2ce/Tuba-KS", true]]]);
  expect(missing.groups[0]?.options[0]?.title).toBe("vsco2ce/Tuba-KS");
  expect(missing.groups).toHaveLength(3);
  const gonePath = instrumentCombo(SAMPLER_CHOICES, SAMPLER_LIBRARIES, "C:\\Libs\\Keys\\Tone.SFZ", null);
  expect(gonePath.groups[0]?.options[0]?.label).toBe("Not installed: Tone");
  // A preset that is no choice does not count as read.
  expect(instrumentCombo(SAMPLER_CHOICES, SAMPLER_LIBRARIES, path, "zeta/Gone").selected).toBe(path);

  // While sonara-app has not answered, the name alone, with no claim.
  const asking = instrumentCombo(SAMPLER_CHOICES, SAMPLER_LIBRARIES, path, undefined);
  expect(asking.selected).toBe(path);
  expect(comboRows(asking.groups)[0]).toEqual([null, [["Cello", path, true]]]);
});

test("the gain-reduction readout follows the live frame's devices when its layout is the project's", () => {
  expect(gainReductionText(null)).toBe("GR  –");
  expect(gainReductionText(3.25)).toBe("GR  3.2 dB");
  expect(gainReductionText(12.5)).toBe("GR 12.5 dB");
  expect(gainReductionText(-0.04)).toBe("GR -0.0 dB");
  const open = chainProject();
  // Every track's instrument and effects in order, then the master's.
  expect(["dev_synth", "dev_comp", "dev_eq", "dev_drums", "dev_limit", "nope"].map((id) => deviceIndex(open, id))).toEqual([
    0, 1, 2, 3, 4, -1,
  ]);
  const frame = { meter_layout: 7, gain_reduction: [Number.NaN, 4.5, Number.NaN, Number.NaN, 1] } as unknown as LiveFrame;
  expect(gainReductionOf(frame, open, "dev_comp")).toBe(4.5);
  expect(gainReductionOf(frame, open, "dev_limit")).toBe(1);
  expect(gainReductionOf(frame, open, "dev_synth")).toBeNull();
  expect(gainReductionOf({ ...frame, meter_layout: 8 }, open, "dev_comp")).toBeNull();
  expect(gainReductionOf(null, open, "dev_comp")).toBeNull();
});

test("the panel: the header, the three hints, the chain instrument first, the master's chain", () => {
  const open = chainProject();
  expect(headerText(null)).toBe("Devices");
  expect(emptyText(null)).toBe("Select a track to edit its instrument and effects.");
  const folder = findTrack(open, "f");
  expect(headerText(folder)).toBe("Devices");
  expect(emptyText(folder)).toBe("Folder tracks have no devices.");
  expect(canAddEffect(folder)).toBe(false);
  const vox = findTrack(open, "a");
  expect(headerText(vox)).toBe("Vox — devices");
  expect(emptyText(vox)).toBe("No devices — use “+ Effect” to add one.");
  expect(canAddEffect(vox)).toBe(true);
  const bass = findTrack(open, "b");
  if (!bass) throw new Error("no Bass");
  expect(emptyText(bass)).toBeNull();
  expect(chainOf(bass).map((d) => d.id)).toEqual(["dev_synth", "dev_comp", "dev_eq"]);
  expect(findTrack(open, "master")?.inserts.map((d) => d.id)).toEqual(["dev_limit"]);
  expect(ownerOf(open, "dev_limit")?.id).toBe("master");
  expect(ownerOf(open, "dev_eq")?.id).toBe("b");
  expect(ownerOf(open, "nope")).toBeNull();
});

test("the Devices tab follows the selection and the current device (device_panel_follows_selection)", () => {
  const open = chainProject();
  const a = selectTrack(EMPTY_SELECTION, "b");
  let shown = devicesTrackAfter(null, EMPTY_SELECTION, a, open, "select");
  expect(shown).toBe("b");
  const b = selectTrack(a, "k");
  shown = devicesTrackAfter(shown, a, b, open, "select");
  expect(shown).toBe("k");
  // The current device of another track shows its track, although Kick stays selected.
  const withDevice = setCurrentDevice(b, "dev_comp");
  shown = devicesTrackAfter(shown, b, withDevice, open, "device");
  expect(shown).toBe("b");
  // Selecting the selected track again shows it again (`changed` fires for every selection).
  shown = devicesTrackAfter(shown, withDevice, selectTrack(withDevice, "k"), open, "select");
  expect(shown).toBe("k");
  // A prune that drops nothing keeps the panel; one that drops the current track keeps its last.
  expect(devicesTrackAfter("b", b, b, open, "update")).toBe("b");
  expect(devicesTrackAfter("b", b, { ...b, tracks: [], currentTrack: null }, open, "update")).toBe("b");
  // Clearing the selection keeps the panel where it was.
  expect(devicesTrackAfter("b", b, selectTrack(b, null), open, "select")).toBe("b");
});

const CURVE: EqResponse = {
  freqs: [20, 200, 2000, 20000],
  db: [0, 6, -6, 0],
  bands: [{ freq: 1000, gain_db: 6, on: true }],
};

test("the EQ curve: the log frequency axis, the clamped dB axis and the response between points", () => {
  expect(xOf(F_MIN, 282)).toBe(1);
  expect(xOf(F_MAX, 282)).toBeCloseTo(281, 12);
  expect(xOf(Math.sqrt(F_MIN * F_MAX), 282)).toBeCloseTo(141, 12);
  expect(yOf(0, 122)).toBe(61);
  expect(yOf(DB_RANGE, 122)).toBe(1);
  expect(yOf(40, 122)).toBe(1);
  expect(yOf(-40, 122)).toBe(121);
  expect(responseAt(CURVE, 200)).toBe(6);
  expect(responseAt(CURVE, Math.sqrt(200 * 2000))).toBeCloseTo(0, 12);
  expect(responseAt(CURVE, 10)).toBe(0);
  expect(responseAt(CURVE, 40000)).toBe(0);
});

test("the curve loader keeps one request in flight and asks again with the latest parameters", async () => {
  let resolve: ((curve: EqResponse) => void) | null = null;
  let fetches = 0;
  const shown: EqResponse[] = [];
  const loader = new CurveLoader(
    () =>
      new Promise<EqResponse>((r) => {
        fetches += 1;
        resolve = r;
      }),
    (curve) => shown.push(curve),
  );
  loader.refresh();
  loader.refresh();
  loader.refresh();
  expect(fetches).toBe(1);
  (resolve as unknown as (curve: EqResponse) => void)(CURVE);
  await new Promise((r) => setTimeout(r, 0));
  // The changes during the request cost one more request, not one each.
  expect(fetches).toBe(2);
  (resolve as unknown as (curve: EqResponse) => void)({ ...CURVE, db: [1, 1, 1, 1] });
  await new Promise((r) => setTimeout(r, 0));
  expect(fetches).toBe(2);
  expect(shown.map((c) => c.db[0])).toEqual([0, 1]);
});
