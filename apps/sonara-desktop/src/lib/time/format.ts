// Number and time formatting with the prototype's Python wording: the transport bar's
// "mm:ss.mmm", `{x:g}`, `{x:.Nf}` (ties to even, as Python rounds the exact binary value) and
// the `repr` of a float ("16.0", not "16").

/**
 * `f"{value:.{digits}f}"`: fixed decimals. JavaScript's `toFixed` rounds an exact tie up;
 * Python rounds it to even, which this follows.
 */
export function pyFixed(value: number, digits: number): string {
  // `toFixed` prints the exact binary value; a tie shows as a 5 followed only by zeros.
  const exact = Math.abs(value).toFixed(Math.min(100, digits + 70));
  const point = exact.indexOf(".");
  const rest = exact.slice(point + 1 + digits);
  if (/^50*$/.test(rest)) {
    const kept = exact.slice(0, point + 1 + digits).replace(".", "");
    const last = Number(kept.at(-1) ?? "0");
    const down = (value < 0 ? "-" : "") + exact.slice(0, digits > 0 ? point + 1 + digits : point);
    // Ties to even: an even last digit stays; `toFixed` rounds an odd one away from zero,
    // which makes it even.
    if (last % 2 === 0) return down;
  }
  return value.toFixed(digits);
}

/** `f"{value:g}"` for the values the window shows (tempi, kHz): six significant digits. */
export function pyG(value: number): string {
  if (!Number.isFinite(value)) return String(value);
  return String(Number(value.toPrecision(6)));
}

/** `repr(float)`: "16.0" for a whole number, the shortest round trip otherwise. */
export function pyRepr(value: number): string {
  if (Number.isInteger(value) && Math.abs(value) < 1e16) return value.toFixed(1);
  return String(value);
}

/** The transport bar's time, `f"{int(m):02d}:{s:06.3f}"` of non-negative `seconds`. */
export function formatTime(seconds: number): string {
  const total = Math.max(0, seconds);
  const minutes = Math.floor(total / 60);
  const rest = total - minutes * 60;
  return `${String(minutes).padStart(2, "0")}:${pyFixed(rest, 3).padStart(6, "0")}`;
}
