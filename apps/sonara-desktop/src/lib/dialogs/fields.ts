// The number fields of the dialogs and the inspector, as the prototype's Qt spin boxes behave:
// a value is clamped to the range and held with the box's decimals, rounded on the exact binary
// value with a tie away from zero (Qt's `QString::number(v, 'f', n)`); the text may carry the
// suffix ("80 ms") or a decimal comma. Plain functions, tested without a page.

/** A spin box's range and display. */
export interface SpinSpec {
  readonly min: number;
  readonly max: number;
  readonly step: number;
  readonly decimals: number;
  /** Shown after the number (" dB"). */
  readonly suffix?: string;
  /** Shown instead of the minimum ("off", "auto (end of content)"). */
  readonly special?: string;
}

/**
 * `value` with `decimals` decimals as a `QDoubleSpinBox` holds it: on the exact binary value, a
 * tie away from zero. JavaScript's `toFixed` rounds the exact value too and takes the larger of
 * two equally near results, which is away from zero for a positive value.
 */
export function qtRound(value: number, decimals: number): number {
  if (!Number.isFinite(value)) return value;
  const magnitude = Number(Math.abs(value).toFixed(decimals));
  return value < 0 ? -magnitude : magnitude;
}

/** `value` clamped to the spin box's range and rounded to its decimals. */
export function spinValue(value: number, spec: SpinSpec): number {
  return qtRound(Math.min(spec.max, Math.max(spec.min, value)), spec.decimals);
}

/** The spin box's text: the special text at the minimum, else the number with its decimals and suffix. */
export function spinText(value: number, spec: SpinSpec): string {
  if (spec.special !== undefined && value <= spec.min) return spec.special;
  const rounded = qtRound(value, spec.decimals);
  // No "-0.0".
  return `${(rounded === 0 ? 0 : rounded).toFixed(spec.decimals)}${spec.suffix ?? ""}`;
}

/**
 * A typed spin box text as a number: the suffix and spaces dropped, a decimal comma taken, the
 * special text as the minimum. `null` when it is not a number.
 */
export function parseSpin(text: string, spec: SpinSpec): number | null {
  let body = text.trim();
  if (spec.special !== undefined && body === spec.special) return spec.min;
  const suffix = spec.suffix?.trim();
  if (suffix && body.toLowerCase().endsWith(suffix.toLowerCase())) body = body.slice(0, -suffix.length).trim();
  body = body.replace(",", ".");
  if (!/^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/.test(body)) return null;
  const value = Number(body);
  return Number.isFinite(value) ? value : null;
}

/** A step of the arrow keys (`steps` = ±1) or Page Up and Page Down (±10) from `value`. */
export function stepSpin(value: number, steps: number, spec: SpinSpec): number {
  return spinValue(value + steps * spec.step, spec);
}

/** Whether a modal dialog is open: the window's single-key shortcuts wait while one is. */
export function dialogIsOpen(root: Pick<Document, "querySelector"> = document): boolean {
  return root.querySelector("dialog[open]") !== null;
}
