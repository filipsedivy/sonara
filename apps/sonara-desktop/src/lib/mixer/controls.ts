// The value controls of the mixer and the device editors (CT; the prototype's
// sonara/ui/widgets/controls.py): the mappings between a value in real units and a 0–1
// position (linear, logarithmic and the fader's gain taper), the value texts, the typed dB entry
// and the keyboard, wheel and drag steps of `ValueControl`. Plain functions; the components
// (Fader.svelte, Knob.svelte) only call them.

import { pyRound } from "../time/signatures";
import { pyFixed } from "../time/format";

/** The bottom of the fader's travel: −∞ (`MIN_DB`). */
export const MIN_DB = -144;
/** The top of the fader's travel (`MAX_DB`). */
export const MAX_DB = 12;
const FADER_MAX_GAIN = 10 ** (MAX_DB / 20);

/** A mapping between a value and its 0–1 position. */
export interface Mapping {
  toNorm(value: number): number;
  fromNorm(x: number): number;
}

/** `linear_mapping(lo, hi)`. */
export function linearMapping(lo: number, hi: number): Mapping {
  const span = hi - lo;
  return { toNorm: (v) => (v - lo) / span, fromNorm: (x) => lo + x * span };
}

/** `log_mapping(lo, hi)`. */
export function logMapping(lo: number, hi: number): Mapping {
  const ratio = Math.log(hi / lo);
  return { toNorm: (v) => Math.log(Math.max(v, lo) / lo) / ratio, fromNorm: (x) => lo * Math.exp(x * ratio) };
}

/**
 * `fader_to_norm`: dB to the fader's position, a fourth-root gain taper. Unity sits at about 71 %
 * of the travel, −20 dB at about 40 %, −60 dB at about 13 %; the bottom is −∞.
 */
export function faderToNorm(db: number): number {
  if (db <= MIN_DB + 1e-9) return 0;
  const gain = 10 ** (Math.min(db, MAX_DB) / 20);
  return Math.min(1, (gain / FADER_MAX_GAIN) ** 0.25);
}

/** `fader_from_norm`: the fader's position to dB. */
export function faderFromNorm(x: number): number {
  if (x <= 0.002) return MIN_DB;
  const gain = FADER_MAX_GAIN * Math.min(x, 1) ** 4;
  return Math.max(MIN_DB, Math.min(MAX_DB, 20 * Math.log10(gain)));
}

/** `FADER_MAPPING`. */
export const FADER_MAPPING: Mapping = { toNorm: faderToNorm, fromNorm: faderFromNorm };

/** `f"{value:+.{digits}f}"`: the sign always shown, ties to even. */
export function pySigned(value: number, digits: number): string {
  const negative = value < 0 || Object.is(value, -0);
  return (negative ? "-" : "+") + pyFixed(Math.abs(value), digits);
}

/** `format_db`: "-inf" at the bottom, "0.0" near unity, else signed ("+3.0", "-6.0"). */
export function formatDb(db: number, digits = 1): string {
  if (db <= MIN_DB + 1e-6) return "-inf";
  return Math.abs(db) >= 0.05 ? pySigned(db, digits) : "0.0";
}

/** `format_pan`: "C", "L30" or "R100". */
export function formatPan(pan: number): string {
  if (Math.abs(pan) < 0.005) return "C";
  return `${pan < 0 ? "L" : "R"}${pyRound(Math.abs(pan) * 100)}`;
}

/** `format_value`: a compact, unit-aware value text ("1.20 kHz", "12.0 ms", "-6.0 dB", "4.0:1"). */
export function formatValue(value: number, unit = ""): string {
  const v = Number(value);
  switch (unit) {
    case "Hz":
      return v >= 1000 ? `${pyFixed(v / 1000, 2)} kHz` : v >= 100 ? `${pyFixed(v, 0)} Hz` : `${pyFixed(v, 1)} Hz`;
    case "ms":
      return v >= 100 ? `${pyFixed(v, 0)} ms` : v >= 1 ? `${pyFixed(v, 1)} ms` : `${pyFixed(v, 2)} ms`;
    case "s":
      return v >= 10 ? `${pyFixed(v, 1)} s` : v >= 0.1 ? `${pyFixed(v, 2)} s` : `${pyFixed(v * 1000, 0)} ms`;
    case "dB":
    case "dBFS":
      return `${formatDb(v)} ${unit}`;
    case ":1":
      return `${pyFixed(v, 1)}:1`;
    case "cents":
      return `${pySigned(v, 0)} ct`;
    case "oct":
      return `${pySigned(v, 2)} oct`;
    case "x":
      return `${pyFixed(v, 2)}x`;
    case "semitones":
      return `${pySigned(v, 0)} st`;
    default:
      if (unit) return `${pyFixed(v, 2)} ${unit}`;
      return Math.abs(v) >= 100 ? pyFixed(v, 0) : pyFixed(v, 2);
  }
}

/** The fader's and the send knob's text: "-6.0 dB", "-inf dB". */
export function dbText(db: number): string {
  return `${formatDb(db)} dB`;
}

/** Python's `float(text)`: decimals, exponents, `_` between digits, "inf", "infinity", "nan". */
export function parsePyFloat(text: string): number | null {
  const t = text.trim();
  const special = /^([+-]?)(inf|infinity|nan)$/i.exec(t);
  if (special) {
    if (special[2].toLowerCase() === "nan") return Number.NaN;
    return special[1] === "-" ? -Infinity : Infinity;
  }
  const digits = "\\d(?:_?\\d)*";
  const number = new RegExp(`^[+-]?(?:${digits}(?:\\.(?:${digits})?)?|\\.${digits})(?:[eE][+-]?${digits})?$`);
  if (!number.test(t)) return null;
  return Number(t.replaceAll("_", ""));
}

/**
 * The strip's typed dB (`_typed_db`): "-inf", "inf" and "-∞" are the bottom; an optional "dB";
 * clamped to −144…+12 as Python's `max(MIN_DB, min(12.0, db))` (NaN gives +12). `null` for a
 * text that is not a number.
 */
export function parseDb(text: string): number | null {
  const t = text.trim().toLowerCase().replaceAll("db", "").trim();
  const db = t === "-inf" || t === "inf" || t === "-∞" ? MIN_DB : parsePyFloat(t);
  if (db === null) return null;
  // Python's min and max keep their first argument unless the second compares below (above)
  // it, so a NaN gives +12.
  const upper = db < MAX_DB ? db : MAX_DB;
  return upper > MIN_DB ? upper : MIN_DB;
}

/** A value control's range and steps (`ValueControl`). */
export interface ControlSpec {
  readonly min: number;
  readonly max: number;
  readonly defaultValue: number;
  readonly mapping: Mapping;
  /** One step of the position (arrow keys, a wheel notch); 0.01 unless given. */
  readonly step: number;
  /** Rounds a value before it is set (integer parameters); none by default. */
  readonly quantize?: (value: number) => number;
}

/** `STEP` and `PAGE` of `ValueControl` for `step` (0.01 by default). */
export function pageOf(step: number): number {
  return Math.min(1, step * 10);
}

/** The pixels of vertical drag for the whole range (`DRAG_PIXELS`; ⇧ divides the speed by 10). */
export const DRAG_PIXELS = 220;

/** The pixels of a wheel notch (the timeline's ⌘-wheel uses the same). */
export const WHEEL_NOTCH_PX = 60;

/** The time after the last wheel step that ends the gesture (the knob's 350 ms timer). */
export const WHEEL_END_MS = 350;

/** `_clamp`: within the range; NaN keeps `current`. */
export function clampValue(spec: ControlSpec, value: number, current: number): number {
  if (Number.isNaN(value)) return current;
  return Math.min(spec.max, Math.max(spec.min, value));
}

/** The control's position, 0–1 (`norm`). */
export function normOf(spec: ControlSpec, value: number): number {
  return Math.min(1, Math.max(0, spec.mapping.toNorm(value)));
}

/** `_set_from_user`: the value a user's change gives, clamped and quantized. */
export function userValue(spec: ControlSpec, value: number, current: number): number {
  let v = clampValue(spec, value, current);
  if (spec.quantize) v = clampValue(spec, spec.quantize(v), current);
  return v;
}

/** `_set_norm_from_user`: the value at position `x`. */
export function valueAtNorm(spec: ControlSpec, x: number, current: number): number {
  return userValue(spec, spec.mapping.fromNorm(Math.min(1, Math.max(0, x))), current);
}

/**
 * `keyPressEvent`: the value after `key` (with ⇧ for a tenth of a step), or null for a key the
 * control does not take. ↑ and → step up, ↓ and ← down; Page Up and Page Down move ten steps;
 * Home and End go to the limits; Delete and Backspace reset to the default. Every key ends its
 * own gesture.
 */
export function keyValue(spec: ControlSpec, key: string, shift: boolean, current: number): number | null {
  const fine = shift ? 0.1 : 1;
  const page = pageOf(spec.step);
  const deltas: Record<string, number> = {
    ArrowUp: spec.step,
    ArrowRight: spec.step,
    ArrowDown: -spec.step,
    ArrowLeft: -spec.step,
    PageUp: page,
    PageDown: -page,
  };
  const delta = deltas[key];
  if (delta !== undefined) return valueAtNorm(spec, normOf(spec, current) + delta * fine, current);
  if (key === "Home") return userValue(spec, spec.min, current);
  if (key === "End") return userValue(spec, spec.max, current);
  if (key === "Delete" || key === "Backspace") return userValue(spec, spec.defaultValue, current);
  return null;
}

/** `wheelEvent`: the value after a wheel of `deltaY` pixels (⇧ for a tenth of a step). */
export function wheelValue(spec: ControlSpec, deltaY: number, shift: boolean, current: number): number {
  const steps = -deltaY / WHEEL_NOTCH_PX;
  return valueAtNorm(spec, normOf(spec, current) + steps * spec.step * (shift ? 0.1 : 1), current);
}

/** The pan knob: −1…+1, centre by default, bipolar. */
export const PAN_SPEC: ControlSpec = { min: -1, max: 1, defaultValue: 0, mapping: linearMapping(-1, 1), step: 0.01 };

/** The fader and the send knob: −∞…+12 dB on the fader taper, 0 dB by default. */
export const FADER_SPEC: ControlSpec = { min: MIN_DB, max: MAX_DB, defaultValue: 0, mapping: FADER_MAPPING, step: 0.01 };
