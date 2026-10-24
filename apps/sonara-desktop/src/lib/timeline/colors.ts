// The prototype's dark theme (sonara/ui/theme.py) and Qt's colour arithmetic, for the canvases.
// The page's CSS custom properties carry the same values (app.css); the canvases read these.

/** A colour as 0–255 channels. */
export type Rgb = readonly [number, number, number];

/** The dark palette of `theme.py`. */
export const THEME = {
  window: "#1e1f22",
  panel: "#26282c",
  panelAlt: "#2c2f34",
  lane: "#232529",
  laneAlt: "#27292e",
  gridBar: "#4a4e57",
  gridBeat: "#363940",
  gridSub: "#2d3036",
  text: "#d8dbe0",
  textDim: "#8b9099",
  accent: "#4c9bf0",
  selection: "#f0c24c",
  playhead: "#ff5f56",
  loop: "#4c9bf0",
  meterGreen: "#46c46b",
  meterYellow: "#e6c84a",
  meterRed: "#ee5050",
  border: "#141517",
} as const;

/** The 12 track colours, picked by track index (`TRACK_COLORS`). */
export const TRACK_COLORS = [
  "#d9644a",
  "#d9a24a",
  "#c9c44a",
  "#7fbf4f",
  "#4fbf8a",
  "#4fb3bf",
  "#5b8fd9",
  "#7d6bd9",
  "#b35bd9",
  "#d95b9f",
  "#a07850",
  "#8a8a8a",
] as const;

/** `#rrggbb` (or `#rgb`) as channels; black for anything else. */
export function parseHex(hex: string): Rgb {
  let digits = hex.startsWith("#") ? hex.slice(1) : hex;
  if (digits.length === 3) digits = [...digits].map((d) => d + d).join("");
  const n = Number.parseInt(digits, 16);
  if (digits.length !== 6 || Number.isNaN(n)) return [0, 0, 0];
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** `#rrggbb` of `c`. */
export function toHex(c: Rgb): string {
  return `#${c.map((v) => Math.round(v).toString(16).padStart(2, "0")).join("")}`;
}

/** `theme.track_color(index, explicit)`. */
export function trackColor(index: number, explicit: string | null = null): Rgb {
  return parseHex(explicit ?? TRACK_COLORS[((index % 12) + 12) % 12]);
}

/** `painting.blend`: `a` moved towards `b` by `t`, rounded per channel. */
export function blend(a: Rgb, b: Rgb, t: number): Rgb {
  return [
    Math.round(a[0] + (b[0] - a[0]) * t),
    Math.round(a[1] + (b[1] - a[1]) * t),
    Math.round(a[2] + (b[2] - a[2]) * t),
  ];
}

/** `painting.luminance` of 0–1 channels. */
export function luminance(c: Rgb): number {
  return (0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]) / 255;
}

function toHsv([r, g, b]: Rgb): [number, number, number] {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const d = max - min;
  let h = 0;
  if (d > 0) {
    if (max === r) h = ((g - b) / d) % 6;
    else if (max === g) h = (b - r) / d + 2;
    else h = (r - g) / d + 4;
    h = (h * 60 + 360) % 360;
  }
  return [h, max === 0 ? 0 : d / max, max / 255];
}

function fromHsv(h: number, s: number, v: number): Rgb {
  const c = v * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = v - c;
  const [r, g, b] =
    h < 60 ? [c, x, 0] : h < 120 ? [x, c, 0] : h < 180 ? [0, c, x] : h < 240 ? [0, x, c] : h < 300 ? [x, 0, c] : [c, 0, x];
  return [Math.round((r + m) * 255), Math.round((g + m) * 255), Math.round((b + m) * 255)];
}

/** `QColor.fromHsvF`: hue, saturation and value from 0 to 1. */
export function fromHsvF(h: number, s: number, v: number): Rgb {
  return fromHsv(h * 360, s, v);
}

/** `QColor.lighter(factor)`: the value times factor/100; past full value the saturation drops. */
export function lighter(c: Rgb, factor: number): Rgb {
  if (factor < 100) return darker(c, 10000 / factor);
  const [h, s, v] = toHsv(c);
  let value = (v * factor) / 100;
  let saturation = s;
  if (value > 1) {
    saturation = Math.max(0, s - (value - 1));
    value = 1;
  }
  return fromHsv(h, saturation, value);
}

/** `QColor.darker(factor)`: the value divided by factor/100. */
export function darker(c: Rgb, factor: number): Rgb {
  if (factor < 100) return lighter(c, 10000 / factor);
  const [h, s, v] = toHsv(c);
  return fromHsv(h, s, (v * 100) / factor);
}

/** 0–1 channels for a shader. */
export function unit(c: Rgb): [number, number, number] {
  return [c[0] / 255, c[1] / 255, c[2] / 255];
}

/** `rgba(…)` for a 2D canvas. */
export function css(c: Rgb, alpha = 1): string {
  return `rgba(${c[0]}, ${c[1]}, ${c[2]}, ${alpha})`;
}
