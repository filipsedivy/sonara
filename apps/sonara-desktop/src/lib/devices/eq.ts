// The parametric EQ's response curve on its card (the prototype's `EqCurve`, sonara/ui/devices/
// panel.py): 20 Hz–20 kHz on a log axis, ±24 dB, the band markers. The curve's points come from
// sonara-app's `eq_response` (the processor's own biquad design plus the output gain, DES-01);
// this module only places and paints them, and keeps one request in flight per card.

import { THEME, TRACK_COLORS } from "../timeline/colors";
import type { EqResponse } from "../types";

/** The lowest frequency drawn (`F_MIN`). */
export const F_MIN = 20;
/** The highest frequency drawn (`F_MAX`). */
export const F_MAX = 20000;
/** The dB range above and below 0 (`DB_RANGE`). */
export const DB_RANGE = 24;
/** The curve's size in CSS pixels (`sizeHint`, 280 × 120; at least 240 × 110). */
export const CURVE_SIZE = { width: 280, height: 120 } as const;

/** The frequency lines and their labels (an empty label draws the line only). */
const FREQ_GRID: readonly [number, string][] = [
  [50, "50"],
  [100, "100"],
  [200, ""],
  [500, "500"],
  [1000, "1k"],
  [2000, ""],
  [5000, "5k"],
  [10000, "10k"],
];
/** The dB lines; ±12 are labelled. */
const DB_GRID = [-18, -12, -6, 0, 6, 12, 18];

/** `_x`: the x of `freq` on a curve `width` pixels wide. */
export function xOf(freq: number, width: number): number {
  return (Math.log(freq / F_MIN) / Math.log(F_MAX / F_MIN)) * (width - 2) + 1;
}

/** `_y`: the y of `db`, clamped to ±24 dB, on a curve `height` pixels high. */
export function yOf(db: number, height: number): number {
  const h = height - 2;
  return 1 + (1 - (Math.min(Math.max(db, -DB_RANGE), DB_RANGE) + DB_RANGE) / (2 * DB_RANGE)) * h;
}

/** `response_at`: the curve at `freq`, interpolated linearly in log frequency (`np.interp`). */
export function responseAt(curve: EqResponse, freq: number): number {
  const { freqs, db } = curve;
  if (freqs.length === 0) return 0;
  const x = Math.log(freq);
  if (x <= Math.log(freqs[0])) return db[0];
  const last = freqs.length - 1;
  if (x >= Math.log(freqs[last])) return db[last];
  let i = 1;
  while (i < last && Math.log(freqs[i]) < x) i += 1;
  const x0 = Math.log(freqs[i - 1]);
  const x1 = Math.log(freqs[i]);
  return db[i - 1] + ((x - x0) / (x1 - x0)) * (db[i] - db[i - 1]);
}

/** `#rrggbb` with an alpha of `alpha`/255, as Qt's `color(name, alpha)`. */
function withAlpha(hex: string, alpha: number): string {
  const n = Number.parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha / 255})`;
}

/**
 * `paintEvent` on a `width` × `height` CSS-pixel context (already scaled to the device pixels):
 * the lane, the frequency and dB grids with their labels, the curve filled down to 0 dB, the
 * markers of the bands that are on, and the frame. Without a curve, a flat line at 0 dB.
 */
export function paintCurve(ctx: CanvasRenderingContext2D, width: number, height: number, curve: EqResponse | null): void {
  ctx.clearRect(0, 0, width, height);
  ctx.fillStyle = THEME.lane;
  ctx.fillRect(0, 0, width, height);
  ctx.font = "9px -apple-system, BlinkMacSystemFont, sans-serif";
  ctx.lineWidth = 1;
  for (const [freq, label] of FREQ_GRID) {
    const x = xOf(freq, width);
    ctx.strokeStyle = THEME.gridBeat;
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, height);
    ctx.stroke();
    if (label) {
      ctx.fillStyle = THEME.textDim;
      ctx.fillText(label, x + 2, height - 3);
    }
  }
  for (const db of DB_GRID) {
    const y = yOf(db, height);
    ctx.strokeStyle = db === 0 ? THEME.gridBar : THEME.gridSub;
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(width, y);
    ctx.stroke();
    if (db !== 0 && db % 12 === 0) {
      ctx.fillStyle = THEME.textDim;
      ctx.fillText(db > 0 ? `+${db}` : `${db}`, 2, y - 2);
    }
  }
  const freqs = curve?.freqs ?? [F_MIN, F_MAX];
  const db = curve?.db ?? [0, 0];
  const path = new Path2D();
  freqs.forEach((freq, i) => {
    const x = xOf(freq, width);
    const y = yOf(db[i] ?? 0, height);
    if (i === 0) path.moveTo(x, y);
    else path.lineTo(x, y);
  });
  const fill = new Path2D(path);
  fill.lineTo(xOf(F_MAX, width), yOf(0, height));
  fill.lineTo(xOf(F_MIN, width), yOf(0, height));
  fill.closePath();
  ctx.fillStyle = withAlpha(THEME.accent, 40);
  ctx.fill(fill);
  ctx.strokeStyle = THEME.accent;
  ctx.lineWidth = 1.8;
  ctx.stroke(path);
  ctx.lineWidth = 1;
  (curve?.bands ?? []).forEach((band, i) => {
    if (!band.on) return;
    ctx.beginPath();
    ctx.arc(xOf(band.freq, width), yOf(band.gain_db, height), 4, 0, 2 * Math.PI);
    ctx.fillStyle = TRACK_COLORS[(i * 3) % TRACK_COLORS.length];
    ctx.fill();
    ctx.strokeStyle = THEME.border;
    ctx.stroke();
  });
  ctx.strokeStyle = THEME.border;
  ctx.strokeRect(0.5, 0.5, width - 1, height - 1);
}

/**
 * The curve of one EQ device, asked for again whenever its parameters change: at most one request
 * in flight, and the latest parameters win (a knob drag changes them at every display frame).
 */
export class CurveLoader {
  private inFlight = false;
  private due = false;
  /** Requests sent so far. */
  requests = 0;

  constructor(
    private readonly fetch: () => Promise<EqResponse>,
    private readonly show: (curve: EqResponse) => void,
    private readonly onError: (error: unknown) => void = () => {},
  ) {}

  /** The parameters changed (or the card appeared): the curve is asked for. */
  refresh(): void {
    this.due = true;
    if (!this.inFlight) void this.run();
  }

  private async run(): Promise<void> {
    this.inFlight = true;
    try {
      while (this.due) {
        this.due = false;
        this.requests += 1;
        this.show(await this.fetch());
      }
    } catch (error) {
      this.onError(error);
    } finally {
      this.inFlight = false;
    }
  }
}
