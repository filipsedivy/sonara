// The device editors' model (the prototype's sonara/ui/devices/panel.py): the cards'
// parameters grouped by their ID prefix with short labels (`group_params`, `short_label`), each
// parameter's control generated from its descriptor (`ParamWidget`: a knob for a float or an int,
// logarithmic or linear, bipolar for a symmetric range, ints quantized; an On/Off toggle for a
// bool; a combo for a choice), and what a card and the panel show around them: the texts, the
// preset and sidechain choices, the gain-reduction readout and the chain of a track. Plain
// functions; the components (DevicePanel.svelte, DeviceCard.svelte, ParamControl.svelte) only call
// them. The EQ curve's numbers come from sonara-app (`eq_response`), never from here (DES-01).

import type { LiveFrame } from "../live/frame";
import { type ControlSpec, formatValue, linearMapping, logMapping } from "../mixer/controls";
import { pyFixed } from "../time/format";
import { pyRound } from "../time/signatures";
import type { DeviceView, OpenProject, ParamValue, ParamView, PluginView, TrackView } from "../types";

/** The titles of the groups the prototype names (`GROUP_TITLES`); others are title-cased. */
export const GROUP_TITLES: Readonly<Record<string, string>> = {
  osc1: "Osc 1",
  osc2: "Osc 2",
  amp: "Amp",
  filter: "Filter",
  lfo: "LFO",
  unison: "Unison",
  hp: "High-Pass",
  lp: "Low-Pass",
  low: "Low",
  mid1: "Mid 1",
  mid2: "Mid 2",
  high: "High",
  kick: "Kick",
  snare: "Snare",
  hat: "Hi-Hat",
  cymbal: "Cymbal",
  velocity: "Velocity",
  sidechain: "Sidechain",
  invert: "Polarity",
  voice: "Voice",
  pulse: "Pulse",
  lfo_rate: "LFO",
};

/** The group of the parameters without a group of their own (`MAIN_GROUP`), shown first. */
export const MAIN_GROUP = "main";
/** The rows of a group's grid when it has more than two parameters (`GROUP_ROWS`). */
export const GROUP_ROWS = 2;
/** The category whose cards show the live gain reduction (`DYNAMICS`). */
export const DYNAMICS = "dynamics";
/** The plugin whose card shows the response curve. */
export const EQ_PLUGIN = "sonara.eq.parametric";

/** The knob of a parameter card (`ParamWidget`'s `Knob(diameter=26, min_width=66)`). */
export const PARAM_KNOB = { diameter: 26, minWidth: 66 } as const;

/** One group of a card's parameters. */
export interface ParamGroup {
  readonly key: string;
  readonly title: string;
  readonly params: readonly ParamView[];
}

/** Python's `str.title()`: a letter after a non-letter is upper case, every other lower case. */
function titleCase(text: string): string {
  let out = "";
  let afterLetter = false;
  for (const c of text) {
    const letter = c.toLowerCase() !== c.toUpperCase();
    out += letter ? (afterLetter ? c.toLowerCase() : c.toUpperCase()) : c;
    afterLetter = letter;
  }
  return out;
}

/** The ID's prefix before its first `_`, or null without one (`id.split("_", 1)[0]`). */
function prefixOf(id: string): string | null {
  const at = id.indexOf("_");
  return at < 0 ? null : id.slice(0, at);
}

/**
 * `group_params`: the parameters grouped by their ID prefix (`osc1_wave` → `osc1`), a bare ID that
 * names a group (`unison` beside `unison_detune`) in that group, and every prefix that only one
 * parameter uses in the main group. The main group comes first, then the others in the order they
 * were first seen; each keeps its parameters' order.
 */
export function groupParams(params: readonly ParamView[]): ParamGroup[] {
  const groupKeys = new Set(params.map((p) => prefixOf(p.id)).filter((key): key is string => key !== null));
  const prefixes = params.map((p) => prefixOf(p.id) ?? (groupKeys.has(p.id) ? p.id : MAIN_GROUP));
  const counts = new Map<string, number>();
  for (const prefix of prefixes) counts.set(prefix, (counts.get(prefix) ?? 0) + 1);
  const groups = new Map<string, ParamView[]>();
  params.forEach((param, i) => {
    const prefix = prefixes[i];
    const key = (counts.get(prefix) ?? 0) >= 2 ? prefix : MAIN_GROUP;
    groups.set(key, [...(groups.get(key) ?? []), param]);
  });
  const out: ParamGroup[] = [];
  const main = groups.get(MAIN_GROUP);
  if (main) out.push({ key: MAIN_GROUP, title: "Main", params: main });
  for (const [key, items] of groups) {
    if (key === MAIN_GROUP) continue;
    out.push({ key, title: GROUP_TITLES[key] ?? titleCase(key.replaceAll("_", " ")), params: items });
  }
  return out;
}

/** `short_label`: the parameter's name without its group's leading words ("Osc 1 Wave" → "Wave"). */
export function shortLabel(param: ParamView, groupTitle: string): string {
  const name = param.name;
  const first = groupTitle.split(" ")[0] ?? "";
  for (const prefix of [groupTitle, first, groupTitle.replaceAll("-", " ")]) {
    if (prefix && name.toLowerCase().startsWith(`${prefix.toLowerCase()} `) && name.length > prefix.length + 1) {
      return name.slice(prefix.length + 1);
    }
  }
  return name;
}

/** The rows of a group's grid; its parameters fill the columns top to bottom (`GROUP_ROWS`). */
export function groupRows(group: ParamGroup): number {
  return group.params.length > 2 ? GROUP_ROWS : 1;
}

/** A parameter's tooltip: its name, its unit in parentheses and its description on a line of its own. */
export function tooltipOf(param: ParamView): string {
  return `${param.name}${param.unit ? ` (${param.unit})` : ""}${param.description ? `\n${param.description}` : ""}`;
}

/** The control of one parameter. */
export type ParamControl =
  | {
      readonly kind: "knob";
      readonly spec: ControlSpec;
      /** The value arc starts at the centre (a range symmetric around 0). */
      readonly bipolar: boolean;
      /** An int parameter: quantized, and sent as an integer. */
      readonly integer: boolean;
      /** The value's text below the knob. */
      readonly format: (value: number) => string;
    }
  | { readonly kind: "toggle" }
  | { readonly kind: "combo"; readonly choices: readonly string[] };

/** `lo < 0 < hi and abs(lo + hi) < 1e-9`. */
export function isBipolar(lo: number, hi: number): boolean {
  return lo < 0 && 0 < hi && Math.abs(lo + hi) < 1e-9;
}

/**
 * `ParamWidget`'s control of `param`: a float or an int is a knob over its range (logarithmic on a
 * log scale), its default the descriptor's; an int is quantized to whole numbers (Python's `round`,
 * ties to even), steps by one unit (at least 1 % of the travel) and shows "16 voices"; a float shows
 * its value by its unit (`format_value`). A bool is an On/Off toggle; a choice a combo.
 */
export function controlOf(param: ParamView): ParamControl {
  if (param.kind === "bool") return { kind: "toggle" };
  if (param.kind === "enum") return { kind: "combo", choices: param.choices };
  const lo = param.min ?? 0;
  const hi = param.max ?? 1;
  const integer = param.kind === "int";
  const unit = param.unit;
  const spec: ControlSpec = {
    min: lo,
    max: hi,
    defaultValue: Number(param.default),
    mapping: param.log_scale ? logMapping(lo, hi) : linearMapping(lo, hi),
    step: integer ? Math.max(0.01, 1 / (hi - lo)) : 0.01,
    ...(integer ? { quantize: (v: number) => pyRound(v) } : {}),
  };
  const format = integer ? (v: number) => `${pyRound(v)} ${unit}`.trim() : (v: number) => formatValue(v, unit);
  return { kind: "knob", spec, bipolar: isBipolar(lo, hi), integer, format };
}

/** The value a knob's change sends: an int parameter's as a whole number (`round(v)`). */
export function knobValue(control: ParamControl, value: number): number {
  return control.kind === "knob" && control.integer ? pyRound(value) : value;
}

/** The parameter's value on `device`: its resolved value, else the descriptor's default. */
export function paramValue(device: DeviceView, param: ParamView): ParamValue {
  return device.params[param.id] ?? param.default;
}

/** A toggle's text (`"On" if checked else "Off"`). */
export function toggleText(on: boolean): "On" | "Off" {
  return on ? "On" : "Off";
}

/** The choice a combo shows for `value`: its own when it is one of them, else the first. */
export function comboChoice(choices: readonly string[], value: ParamValue): string {
  const text = String(value);
  return choices.includes(text) ? text : (choices[0] ?? "");
}

// -- a card -------------------------------------------------------------------------------------

/** The preset combo's items: "Custom", then the plugin's presets. */
export function presetItems(plugin: PluginView): string[] {
  return ["Custom", ...plugin.presets];
}

/** The preset combo's index for `preset` (`findText`, else "Custom"). */
export function presetIndex(plugin: PluginView, preset: string | null): number {
  if (!preset) return 0;
  return Math.max(0, presetItems(plugin).indexOf(preset));
}

/** The plugin's name beside the device's, when the device was renamed; else empty. */
export function pluginLabel(device: DeviceView, plugin: PluginView | null): string {
  return plugin && plugin.name !== device.name ? plugin.name : "";
}

/** The warning of a device whose plugin this build does not have. */
export function missingText(device: DeviceView): string {
  return `Plugin '${device.plugin_id}' is not available in this build; the device is bypassed.`;
}

/** One choice of the sidechain combo: the source track's ID ("" for none) and its text. */
export interface SidechainOption {
  readonly id: string;
  readonly name: string;
}

/**
 * The sidechain combo's choices: "None", then every audio, instrument and bus track but the
 * device's own, in track order.
 */
export function sidechainOptions(project: OpenProject, track: TrackView): SidechainOption[] {
  const sources = project.tracks.filter(
    (t) => t.id !== track.id && (t.kind === "audio" || t.kind === "instrument" || t.kind === "bus"),
  );
  return [{ id: "", name: "None" }, ...sources.map((t) => ({ id: t.id, name: t.name }))];
}

/** The text of the gain-reduction readout (`f"GR {gr:4.1f} dB"`); "GR  –" before a value. */
export function gainReductionText(gr: number | null): string {
  if (gr === null) return "GR  –";
  const digits = (gr < 0 || Object.is(gr, -0) ? "-" : "") + pyFixed(Math.abs(gr), 1);
  return `GR ${digits.padStart(4)} dB`;
}

/**
 * The device's place among a live frame's gain reductions: every track's instrument and effects in
 * order, then the master's (sonara-app `live`); −1 for a device the project does not have.
 */
export function deviceIndex(project: OpenProject, deviceId: string): number {
  let index = 0;
  for (const track of [...project.tracks, project.master]) {
    for (const device of chainOf(track)) {
      if (device.id === deviceId) return index;
      index += 1;
    }
  }
  return -1;
}

/**
 * The gain reduction the frame reports for the device in dB, or null: when the frame's layout is
 * not the project's (its values would land on other devices), or the device reports none (NaN).
 */
export function gainReductionOf(frame: LiveFrame | null, project: OpenProject, deviceId: string): number | null {
  if (!frame || frame.meter_layout !== project.meter_layout) return null;
  const value = frame.gain_reduction[deviceIndex(project, deviceId)];
  return value === undefined || Number.isNaN(value) ? null : value;
}

// -- the panel ------------------------------------------------------------------------------------

/** A track's devices: the instrument first, then the effects (`Track.devices()`). */
export function chainOf(track: TrackView): DeviceView[] {
  return track.instrument ? [track.instrument, ...track.inserts] : [...track.inserts];
}

/** The track `trackId` of the project, the master included; null for none. */
export function findTrack(project: OpenProject | null, trackId: string | null): TrackView | null {
  if (!project || trackId === null) return null;
  if (project.master.id === trackId) return project.master;
  return project.tracks.find((t) => t.id === trackId) ?? null;
}

/** The track that holds the device `deviceId`, the master included; null for none. */
export function ownerOf(project: OpenProject | null, deviceId: string | null): TrackView | null {
  if (!project || deviceId === null) return null;
  return [...project.tracks, project.master].find((t) => chainOf(t).some((d) => d.id === deviceId)) ?? null;
}

/** The panel's header: "track — devices", or "Devices" without a track to edit. */
export function headerText(track: TrackView | null): string {
  return track && track.kind !== "folder" ? `${track.name} — devices` : "Devices";
}

/** The panel's hint instead of cards, or null when it shows cards. */
export function emptyText(track: TrackView | null): string | null {
  if (!track) return "Select a track to edit its instrument and effects.";
  if (track.kind === "folder") return "Folder tracks have no devices.";
  return chainOf(track).length === 0 ? "No devices — use “+ Effect” to add one." : null;
}

/** Whether the panel can add an effect: it shows a track that is not a folder. */
export function canAddEffect(track: TrackView | null): boolean {
  return track !== null && track.kind !== "folder";
}
