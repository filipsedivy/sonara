// The mixer's level meters (inventory MX, "Live data"; the prototype's `Meter` widget in
// sonara/ui/widgets/controls.py): the ballistics and the drawing of one stereo meter, and the
// bank that feeds every meter of a window from the live frames.
//
// Ballistics: the displayed peak and RMS fall at 24 dB/s; the peak hold stays 1.5 s, then falls
// at 36 dB/s; the clip indicator latches at 1.0 or above until a click clears it. The scale runs
// from −60 to +6 dBFS: green below −6, yellow up to 0, red above; the peak is the dim bar, the
// RMS the bright one inside it.
//
// The prototype animated at 30 fps from a 33 ms timer and took new levels about 30 times a
// second. Here the bank runs on every animation frame (60 Hz): it decays every meter
// from the frame's time, applies the live frame shown at that frame when its meter layout equals
// the view's, and redraws the meters that changed. None of it goes through Svelte's reactivity:
// 60 frames a second of 100 meters would be 6,000 updates.
//
// For assistive technology ("Accessibility") each meter has an element of
// role `meter` (the canvas's box) whose value the bank sets at most four times a second
// (`ARIA_INTERVAL_S`), and only when its text changes: the louder channel's displayed peak in
// whole dB, "silent" at the floor, and ", clipped" while the clip indicator is lit.

import { meterText, wholeDb } from "../a11y/labels";
import type { LiveFrame } from "../live/frame";
import type { OpenProject } from "../types";
import { THEME, css, parseHex } from "../timeline/colors";

export const FALL_DB_PER_S = 24;
export const HOLD_S = 1.5;
export const TOP_DB = 6;
export const FLOOR_DB = -60;
/** The shortest time between two changes of a meter's value for assistive technology. */
export const ARIA_INTERVAL_S = 0.25;

/** `_to_db`: a linear level in dB, −200 for silence. */
export function toDb(x: number): number {
  return x > 1e-12 ? 20 * Math.log10(x) : -200;
}

/** The ballistics of one meter (`Meter`), with time in seconds. */
export class MeterBallistics {
  readonly peak: number[];
  readonly rms: number[];
  readonly hold: number[];
  private readonly holdTime: number[];
  readonly clip: boolean[];
  private last: number;

  constructor(
    readonly channels = 2,
    readonly floorDb = FLOOR_DB,
    now = 0,
  ) {
    this.peak = new Array<number>(channels).fill(floorDb);
    this.rms = new Array<number>(channels).fill(floorDb);
    this.hold = new Array<number>(channels).fill(floorDb);
    this.holdTime = new Array<number>(channels).fill(0);
    this.clip = new Array<boolean>(channels).fill(false);
    this.last = now;
  }

  /** Whether any channel clipped. */
  get clipped(): boolean {
    return this.clip.some(Boolean);
  }

  /** Whether anything is above the floor (the prototype animated only then). */
  get active(): boolean {
    return [...this.peak, ...this.hold].some((v) => v > this.floorDb);
  }

  /** The displayed peak of `channel` in dB. */
  peakDb(channel = 0): number {
    return this.peak[channel];
  }

  /** `set_levels`: linear peak (and RMS) levels at `now`; a missing RMS is the peak × 0.707. */
  setLevels(peak: readonly number[], rms: readonly number[] | null, now: number): void {
    this.decay(now);
    for (let c = 0; c < this.channels; c++) {
      const pk = Number(c < peak.length ? peak[c] : (peak.at(-1) ?? 0));
      const rm = Number(rms !== null && c < rms.length ? rms[c] : rms && rms.length > 0 ? rms[rms.length - 1] : pk * 0.707);
      if (pk >= 1) this.clip[c] = true;
      const pdb = toDb(pk);
      const rdb = toDb(rm);
      this.peak[c] = Math.max(this.peak[c], pdb);
      this.rms[c] = Math.max(this.rms[c], rdb);
      if (pdb >= this.hold[c]) {
        this.hold[c] = pdb;
        this.holdTime[c] = now;
      }
    }
  }

  /** `_decay`: everything falls by the time since the last call; the hold only after 1.5 s. */
  decay(now: number): void {
    const dt = Math.max(0, now - this.last);
    this.last = now;
    const fall = FALL_DB_PER_S * dt;
    for (let c = 0; c < this.channels; c++) {
      this.peak[c] = Math.max(this.floorDb, this.peak[c] - fall);
      this.rms[c] = Math.max(this.floorDb, this.rms[c] - fall);
      if (now - this.holdTime[c] > HOLD_S) this.hold[c] = Math.max(this.floorDb, this.hold[c] - fall * 1.5);
    }
  }

  /** A click on the meter: clears the clip indicator; whether one was lit. */
  clearClip(): boolean {
    if (!this.clipped) return false;
    this.clip.fill(false);
    return true;
  }

  /** `_norm`: the position of `db` on the −60…+6 scale, 0–1. */
  norm(db: number): number {
    return Math.min(1, Math.max(0, (db - this.floorDb) / (TOP_DB - this.floorDb)));
  }
}

const COLORS = {
  border: parseHex(THEME.border),
  red: parseHex(THEME.meterRed),
  yellow: parseHex(THEME.meterYellow),
  green: parseHex(THEME.meterGreen),
  text: parseHex(THEME.text),
  textDim: parseHex(THEME.textDim),
};

/** `Meter.paintEvent` of `meter` on a `width` × `height` CSS-pixel canvas context. */
export function drawMeter(ctx: CanvasRenderingContext2D, meter: MeterBallistics, width: number, height: number): void {
  ctx.clearRect(0, 0, width, height);
  const top = 6;
  const barH = height - top - 2;
  const bw = (width - (meter.channels - 1) * 2) / meter.channels;
  const zeroY = top + (1 - meter.norm(0)) * barH;
  const warnY = top + (1 - meter.norm(-6)) * barH;
  const zones: [number, number, (typeof COLORS)["red"]][] = [
    [top, zeroY, COLORS.red],
    [zeroY, warnY, COLORS.yellow],
    [warnY, top + barH, COLORS.green],
  ];
  for (let c = 0; c < meter.channels; c++) {
    const x = c * (bw + 2);
    ctx.fillStyle = css(meter.clip[c] ? COLORS.red : COLORS.border);
    ctx.fillRect(x, 0, bw, 4);
    ctx.fillStyle = css(COLORS.border);
    ctx.fillRect(x, top, bw, barH);
    const peakY = top + (1 - meter.norm(meter.peak[c])) * barH;
    const rmsY = top + (1 - meter.norm(meter.rms[c])) * barH;
    for (const [y0, alpha] of [
      [peakY, 110 / 255],
      [rmsY, 1],
    ] as const) {
      if (y0 >= top + barH) continue;
      for (const [zoneTop, zoneBottom, color] of zones) {
        const a = Math.max(y0, zoneTop);
        if (zoneBottom > a) {
          ctx.fillStyle = css(color, alpha);
          ctx.fillRect(x, a, bw, zoneBottom - a);
        }
      }
    }
    if (meter.hold[c] > meter.floorDb) {
      const holdY = top + (1 - meter.norm(meter.hold[c])) * barH;
      ctx.fillStyle = css(meter.hold[c] >= 0 ? COLORS.red : COLORS.text);
      ctx.fillRect(x, holdY - 1, bw, 1.5);
    }
  }
  ctx.fillStyle = css(COLORS.textDim, 150 / 255);
  ctx.fillRect(0, zeroY - 0.5, width, 1);
}

/** The meter index of `trackId` in a live frame of `project`: the tracks in order, then the master. */
export function meterIndex(project: OpenProject, trackId: string): number {
  if (trackId === project.master.id) return project.tracks.length;
  return project.tracks.findIndex((track) => track.id === trackId);
}

interface Entry {
  readonly canvas: HTMLCanvasElement;
  /** The element of role `meter` whose value the bank sets. */
  readonly aria: Element;
  readonly trackId: () => string;
  readonly meter: MeterBallistics;
  ctx: CanvasRenderingContext2D | null;
  width: number;
  height: number;
  /** Drawn since the last change. */
  drawn: boolean;
  /** When the meter's value for assistive technology was last set, in seconds. */
  ariaAt: number;
  /** That value's text. */
  ariaText: string;
}

/** The value a meter shows to assistive technology: its louder displayed peak, in whole dB. */
export function meterValue(meter: MeterBallistics): { now: number; text: string } {
  const peak = Math.max(...meter.peak);
  return {
    now: wholeDb(Math.min(TOP_DB, Math.max(meter.floorDb, peak))),
    text: meterText(peak, meter.floorDb, meter.clipped),
  };
}

/** What the bank did at one animation frame. */
export interface BankUpdate {
  /** Whether the frame's levels reached the meters (its layout matched the view's). */
  applied: boolean;
  /** Meters redrawn. */
  drawn: number;
}

/** Every meter of one window, fed from the live frames at each animation frame. */
export class MeterBank {
  private readonly entries: Entry[] = [];
  private indexProject: OpenProject | null = null;
  private readonly indices = new Map<string, number>();

  /** The meters registered. */
  get size(): number {
    return this.entries.length;
  }

  /**
   * Registers the meter of `trackId` drawn on `canvas`, its value for assistive technology on
   * `aria` (the canvas itself by default); returns the unregister.
   */
  register(
    canvas: HTMLCanvasElement,
    trackId: () => string,
    now = performance.now() / 1000,
    aria: Element = canvas,
  ): () => void {
    const entry: Entry = {
      canvas,
      aria,
      trackId,
      meter: new MeterBallistics(2, FLOOR_DB, now),
      ctx: null,
      width: 0,
      height: 0,
      drawn: false,
      ariaAt: now,
      ariaText: "",
    };
    this.publish(entry, now);
    this.entries.push(entry);
    return () => {
      const at = this.entries.indexOf(entry);
      if (at >= 0) this.entries.splice(at, 1);
    };
  }

  /** The canvas's CSS size changed: the next frame draws it at `dpr`. */
  resize(canvas: HTMLCanvasElement, width: number, height: number, dpr: number): void {
    const entry = this.entries.find((e) => e.canvas === canvas);
    if (!entry) return;
    entry.width = width;
    entry.height = height;
    const w = Math.max(1, Math.round(width * dpr));
    const h = Math.max(1, Math.round(height * dpr));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
    entry.ctx = canvas.getContext("2d");
    entry.ctx?.setTransform(dpr, 0, 0, dpr, 0, 0);
    entry.drawn = false;
  }

  /** Sets the meter's value for assistive technology at `now`, when its text changed. */
  private publish(entry: Entry, now: number): void {
    entry.ariaAt = now;
    const value = meterValue(entry.meter);
    if (value.text === entry.ariaText) return;
    entry.ariaText = value.text;
    entry.aria.setAttribute("aria-valuenow", String(value.now));
    entry.aria.setAttribute("aria-valuetext", value.text);
  }

  /** MX-13: a click on the meter of `canvas` clears its clip indicator. */
  clearClip(canvas: HTMLCanvasElement): boolean {
    const entry = this.entries.find((e) => e.canvas === canvas);
    if (!entry?.meter.clearClip()) return false;
    entry.drawn = false;
    return true;
  }

  /** The ballistics of `trackId`'s meter (tests and the bench). */
  meterOf(trackId: string): MeterBallistics | null {
    return this.entries.find((e) => e.trackId() === trackId)?.meter ?? null;
  }

  private indexOf(project: OpenProject, trackId: string): number {
    if (this.indexProject !== project) {
      this.indexProject = project;
      this.indices.clear();
      project.tracks.forEach((track, i) => this.indices.set(track.id, i));
      this.indices.set(project.master.id, project.tracks.length);
    }
    return this.indices.get(trackId) ?? -1;
  }

  /**
   * One animation frame at `now` (seconds): every meter decays; `frame`, the live frame shown at
   * this display frame, sets the levels when its layout equals `project`'s; the meters that
   * changed are redrawn.
   */
  update(now: number, frame: LiveFrame | null, project: OpenProject | null): BankUpdate {
    const applied = frame !== null && project !== null && frame.meter_layout === project.meter_layout;
    let drawn = 0;
    for (const entry of this.entries) {
      const before = entry.meter.active;
      entry.meter.decay(now);
      if (applied && frame && project) {
        const levels = frame.meters[this.indexOf(project, entry.trackId())];
        if (levels) entry.meter.setLevels(levels.peak, levels.rms, now);
      }
      if (before || entry.meter.active || !entry.drawn) {
        if (entry.ctx && entry.width > 0) {
          drawMeter(entry.ctx, entry.meter, entry.width, entry.height);
          drawn += 1;
        }
        entry.drawn = true;
      }
      if (now - entry.ariaAt >= ARIA_INTERVAL_S) this.publish(entry, now);
    }
    return { applied, drawn };
  }
}
