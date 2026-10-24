// The piano roll's painting (`paintEvent` of `NoteGrid`, `Keyboard`, `Ruler` and `VelocityLane`
// in sonara/ui/piano_roll/widgets.py) on four Canvas2D surfaces. As in the prototype the painting
// is virtualised: only the rows, grid lines and notes inside the visible rectangle are drawn, the
// notes found through `NoteIndex`; the notes are batched into one path per colour, so 10,000
// visible notes cost a few hundred fills. A frame is drawn at the next animation frame after
// anything changed; the playheads are DOM lines (PianoRoll.svelte), so playback repaints nothing.
//
// Canvas2D was measured first (PERF-01; the step's status in the spec has the numbers), as the
// spike's piano roll was; the timeline needs WebGL2 for its waveforms, the piano roll does not.

import { HiDpiCanvas } from "../hidpi";
import { barLength, barToBeat, beatToBar, signatureAtBar } from "../time/signatures";
import { css, fromHsvF, lighter, parseHex, type Rgb, THEME } from "../timeline/colors";
import type { MidiClipView } from "../types";
import {
  beatX,
  contentToTimeline,
  gridAt,
  isBlackKey,
  isPlayable,
  type NoteIndex,
  type NotePatch,
  noteName,
  pitchY,
  playableRange,
  type RollContext,
  type RollView,
  xBeat,
  yPitch,
} from "./model";

export const KEYBOARD_W = 64;
export const DRUM_KEYBOARD_W = 132;
export const RULER_H = 22;
export const VELOCITY_H = 72;

const FONT_FAMILY = '-apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif';
const WHITE_KEY = "#d9dbe0";
const BLACK_KEY = "#2a2c31";
const KEY_TEXT = "#30333a";
const NOTE_TEXT = "rgb(20, 20, 24)";

/** `velocity_color`: soft blue for quiet notes through to warm orange for loud ones. */
export function velocityColor(velocity: number, selected = false): Rgb {
  const v = Math.min(Math.max(velocity, 1), 127) / 127;
  const hue = (210 - 190 * v) / 360;
  const c = fromHsvF(hue % 1, 0.45 + 0.35 * v, 0.62 + 0.33 * v);
  return selected ? lighter(c, 135) : c;
}

/** The fill of every (velocity, selected, playable) as a CSS colour, built once. */
const NOTE_FILLS: string[] = (() => {
  const fills: string[] = [];
  for (let velocity = 0; velocity < 128; velocity++) {
    for (const selected of [false, true]) {
      const c = velocityColor(velocity, selected);
      fills.push(css(c), css(c, 80 / 255));
    }
  }
  return fills;
})();

function fillKey(velocity: number, selected: boolean, playable: boolean): number {
  return (Math.min(Math.max(velocity, 0), 127) * 2 + (selected ? 1 : 0)) * 2 + (playable ? 0 : 1);
}

/** `drum_label`: the GM name of a drum note, or null. */
export type DrumLabels = ReadonlyMap<number, string>;

/** What one frame of the piano roll shows. */
export interface RollFrame {
  readonly ctx: RollContext | null;
  readonly index: NoteIndex | null;
  readonly view: RollView;
  readonly drums: boolean;
  readonly drumLabels: DrumLabels;
  readonly selected: ReadonlySet<string>;
  /** Notes a gesture moved or resized, drawn there instead of where the index has them. */
  readonly preview: ReadonlyMap<string, NotePatch>;
  /** Velocities the lane shows instead of the notes' (its drag). */
  readonly velocities: ReadonlyMap<string, number>;
  /** The rubber band in grid pixels. */
  readonly rubber: { x: number; y: number; width: number; height: number } | null;
}

/** One frame's cost (the benchmark reads it). */
export interface RollDrawStats {
  readonly drawMs: number;
  /** The same plus a read-back of each canvas, when `syncAfterDraw`. */
  readonly syncMs: number;
  readonly notes: number;
}

function smallFont(size: number): string {
  return `${size}px ${FONT_FAMILY}`;
}

/** The surfaces of the piano roll: the grid, the keyboard, the ruler and the velocity lane. */
export interface RollCanvases {
  grid: HTMLCanvasElement;
  keys: HTMLCanvasElement;
  ruler: HTMLCanvasElement;
  lane: HTMLCanvasElement;
}

type Part = "grid" | "keys" | "ruler" | "lane";

export class RollCanvas {
  private readonly surfaces: Record<Part, HiDpiCanvas>;
  private readonly contexts: Record<Part, CanvasRenderingContext2D>;
  private frame: RollFrame | null = null;
  private frameRequested = false;
  private destroyed = false;
  private readonly widths = new Map<string, number>();
  /** Draws wait for each canvas's pixels (the benchmark's synced pass). */
  syncAfterDraw = false;
  /** No frame is drawn by itself: the benchmark draws them. */
  paused = false;
  /** Called after each drawn frame (the benchmark). */
  onDraw: ((stats: RollDrawStats) => void) | null = null;

  constructor(canvases: RollCanvases) {
    const surface = (canvas: HTMLCanvasElement) => new HiDpiCanvas(canvas, () => this.requestDraw());
    this.surfaces = {
      grid: surface(canvases.grid),
      keys: surface(canvases.keys),
      ruler: surface(canvases.ruler),
      lane: surface(canvases.lane),
    };
    const context = (part: Part) => {
      const ctx = this.surfaces[part].canvas.getContext("2d", { alpha: false });
      if (!ctx) throw new Error("no 2D context for the piano roll");
      return ctx;
    };
    this.contexts = { grid: context("grid"), keys: context("keys"), ruler: context("ruler"), lane: context("lane") };
  }

  /** The grid's size in CSS pixels. */
  get gridSize(): { width: number; height: number } {
    return { width: this.surfaces.grid.cssWidth, height: this.surfaces.grid.cssHeight };
  }

  update(frame: RollFrame): void {
    this.frame = frame;
    this.requestDraw();
  }

  requestDraw(): void {
    if (this.frameRequested || this.destroyed || this.paused) return;
    this.frameRequested = true;
    requestAnimationFrame(() => {
      this.frameRequested = false;
      if (!this.destroyed && !this.paused) this.draw();
    });
  }

  /** Draws the frame now; its cost. */
  draw(): RollDrawStats | null {
    const frame = this.frame;
    if (!frame || this.destroyed) return null;
    for (const surface of Object.values(this.surfaces)) surface.update();
    const t0 = performance.now();
    const notes = this.paintGrid(frame);
    this.paintKeys(frame);
    this.paintRuler(frame);
    this.paintLane(frame);
    const t1 = performance.now();
    if (this.syncAfterDraw) {
      for (const ctx of Object.values(this.contexts)) ctx.getImageData(0, 0, 1, 1);
    }
    const stats = { drawMs: t1 - t0, syncMs: performance.now() - t0, notes };
    this.onDraw?.(stats);
    return stats;
  }

  destroy(): void {
    this.destroyed = true;
    for (const surface of Object.values(this.surfaces)) surface.destroy();
  }

  private measure(ctx: CanvasRenderingContext2D, font: string, text: string): number {
    const key = `${font}\u0000${text}`;
    let width = this.widths.get(key);
    if (width === undefined) {
      if (this.widths.size > 5000) this.widths.clear();
      width = ctx.measureText(text).width;
      this.widths.set(key, width);
    }
    return width;
  }

  // -- the grid -----------------------------------------------------------------------------------

  /** `NoteGrid.paintEvent`; the number of notes drawn. */
  private paintGrid(frame: RollFrame): number {
    const surface = this.surfaces.grid;
    const ctx = this.contexts.grid;
    const s = surface.scale;
    const w = surface.cssWidth;
    const h = surface.cssHeight;
    const { view, drums } = frame;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.fillStyle = THEME.lane;
    ctx.fillRect(0, 0, surface.canvas.width, surface.canvas.height);
    ctx.setTransform(s, 0, 0, s, 0, 0);
    const top = Math.min(127, yPitch(view, 0));
    const bottom = Math.max(0, yPitch(view, h));
    // Rows: black keys shaded, a line under every C (every row on drum tracks).
    const rowLines = new Path2D();
    ctx.fillStyle = THEME.laneAlt;
    for (let pitch = bottom; pitch <= top; pitch++) {
      const y = pitchY(view, pitch);
      if (!drums && isBlackKey(pitch)) ctx.fillRect(0, y, w, view.rowH);
      if (pitch % 12 === 0 || drums) hLine(rowLines, 0, w, y + view.rowH, s);
    }
    stroke(ctx, rowLines, THEME.gridBeat, 1 / s);
    const clipCtx = frame.ctx;
    if (!clipCtx) return 0;
    const clip = clipCtx.clip;
    const b0 = xBeat(view, 0);
    const b1 = xBeat(view, w);
    this.paintGridLines(ctx, clipCtx, view, b0, b1, h, s);
    // The content outside what plays.
    const [a, z] = playableRange(clip);
    const xa = beatX(view, a);
    const xz = beatX(view, z);
    ctx.fillStyle = "rgba(0, 0, 0, 0.2745)";
    if (xa > 0) ctx.fillRect(0, 0, xa, h);
    if (xz < w) {
      ctx.fillRect(xz, 0, w - xz, h);
      ctx.save();
      ctx.strokeStyle = clip.loop_length !== null ? THEME.accent : THEME.textDim;
      ctx.lineWidth = 1.5;
      if (clip.loop_length !== null) ctx.setLineDash([4.5, 3]);
      ctx.beginPath();
      ctx.moveTo(xz, 0);
      ctx.lineTo(xz, h);
      ctx.stroke();
      ctx.restore();
    }
    const drawn = this.paintNotes(ctx, frame, clip, b0, b1, bottom, top, s);
    if (frame.rubber) {
      const r = frame.rubber;
      ctx.fillStyle = css(parseHex(THEME.accent), 40 / 255);
      ctx.fillRect(r.x, r.y, r.width, r.height);
      ctx.save();
      ctx.strokeStyle = THEME.accent;
      ctx.lineWidth = 1 / s;
      ctx.setLineDash([3, 3]);
      ctx.strokeRect(r.x, r.y, r.width, r.height);
      ctx.restore();
    }
    return drawn;
  }

  /** `_paint_grid`: the snap grid when dense enough to read, the beats and the bars. */
  private paintGridLines(
    ctx: CanvasRenderingContext2D,
    roll: RollContext,
    view: RollView,
    b0: number,
    b1: number,
    h: number,
    s: number,
  ): void {
    const signatures = roll.signatures;
    const offset = contentToTimeline(roll.clip, 0);
    const grid = gridAt(roll, Math.max(0, b0));
    if (grid > 0 && grid < 1 && grid * view.ppb >= 7) {
      const sub = new Path2D();
      for (let k = Math.floor((b0 + offset) / grid); ; k++) {
        const c = k * grid - offset;
        if (c > b1) break;
        vLine(sub, beatX(view, c), 0, h, s);
      }
      stroke(ctx, sub, THEME.gridSub, 1 / s);
    }
    const beats = new Path2D();
    const bars = new Path2D();
    const tStart = Math.max(0, b0 + offset);
    let bar = Math.max(0, Math.floor(beatToBar(signatures, tStart)));
    const beatsReadable = view.ppb >= 9;
    for (let guard = 0; guard < 100_000; guard++, bar++) {
      const c = barToBeat(signatures, bar) - offset;
      if (c > b1) break;
      const sig = signatureAtBar(signatures, bar);
      const unit = 4 / sig.denominator;
      if (beatsReadable) {
        for (let k = 1; k < sig.numerator; k++) {
          const cb = c + k * unit;
          if (b0 <= cb && cb <= b1) vLine(beats, beatX(view, cb), 0, h, s);
        }
      }
      if (c >= b0 - 1e-9) vLine(bars, beatX(view, c), 0, h, s);
    }
    stroke(ctx, beats, THEME.gridBeat, 1 / s);
    stroke(ctx, bars, THEME.gridBar, 1 / s);
  }

  /** The visible notes, one path per colour, the outlines and the labels where they fit. */
  private paintNotes(
    ctx: CanvasRenderingContext2D,
    frame: RollFrame,
    clip: MidiClipView,
    b0: number,
    b1: number,
    bottom: number,
    top: number,
    s: number,
  ): number {
    const { view, index, selected, preview, drums } = frame;
    const fills = new Map<number, Path2D>();
    const plain = new Path2D();
    const chosen = new Path2D();
    const outline = view.rowH >= 6;
    const labelled = view.rowH >= 9.5;
    const labels: { x: number; y: number; width: number; text: string }[] = [];
    let drawn = 0;
    const add = (start: number, end: number, pitch: number, velocity: number, isSelected: boolean) => {
      const x0 = beatX(view, start);
      const x1 = beatX(view, end);
      const y = pitchY(view, pitch) + 1;
      const width = Math.max(2, x1 - x0 - 1);
      const height = view.rowH - 2;
      const key = fillKey(velocity, isSelected, isPlayable(clip, start));
      let path = fills.get(key);
      if (!path) {
        path = new Path2D();
        fills.set(key, path);
      }
      path.rect(x0, y, width, height);
      if (outline && (isSelected || width >= 4)) {
        // Inset half a device pixel, so the outline lies on the note's pixels.
        const inset = 0.5 / s;
        (isSelected ? chosen : plain).rect(x0 + inset, y + inset, width - 2 * inset, height - 2 * inset);
      }
      if (labelled && width > 20) {
        const text = drums ? (frame.drumLabels.get(pitch) ?? null) : noteName(pitch);
        if (text) labels.push({ x: x0, y, width, text });
      }
      drawn += 1;
    };
    if (index) {
      for (const i of index.visible(b0, b1, bottom, top)) {
        const id = index.ids[i];
        if (preview.has(id)) continue;
        add(index.starts[i], index.ends[i], index.pitches[i], index.velocities[i], selected.has(id));
      }
    }
    for (const note of preview.values()) {
      const end = note.start + note.length;
      if (end <= b0 || note.start >= b1 || note.pitch < bottom || note.pitch > top) continue;
      add(note.start, end, note.pitch, note.velocity, selected.has(note.id));
    }
    for (const [key, path] of fills) {
      ctx.fillStyle = NOTE_FILLS[key];
      ctx.fill(path);
    }
    if (outline) {
      stroke(ctx, plain, THEME.border, 1 / s);
      stroke(ctx, chosen, THEME.selection, view.rowH >= 10 ? 1.5 : 1);
    }
    if (labels.length > 0) {
      const font = smallFont(Math.min(8, view.rowH * 0.62));
      ctx.font = font;
      ctx.textBaseline = "middle";
      ctx.fillStyle = NOTE_TEXT;
      const middle = (view.rowH - 2) / 2;
      for (const label of labels) {
        // Only full labels: elided fragments are noise.
        if (this.measure(ctx, font, label.text) + 6 <= label.width) ctx.fillText(label.text, label.x + 3, label.y + middle);
      }
    }
    return drawn;
  }

  // -- the keyboard --------------------------------------------------------------------------------

  /** `Keyboard.paintEvent`: piano keys, or "pitch GM name" rows on drum tracks. */
  private paintKeys(frame: RollFrame): void {
    const surface = this.surfaces.keys;
    const ctx = this.contexts.keys;
    const s = surface.scale;
    const w = surface.cssWidth;
    const h = surface.cssHeight;
    const { view, drums } = frame;
    ctx.setTransform(s, 0, 0, s, 0, 0);
    ctx.fillStyle = THEME.panel;
    ctx.fillRect(0, 0, w, h);
    const top = Math.min(127, yPitch(view, 0));
    const bottom = Math.max(0, yPitch(view, h));
    const font = smallFont(view.rowH >= 10 ? 8 : 7);
    ctx.font = font;
    ctx.textBaseline = "middle";
    const used = new Set<number>();
    if (drums && frame.index) for (const pitch of frame.index.pitches) used.add(pitch);
    const lines = new Path2D();
    for (let pitch = bottom; pitch <= top; pitch++) {
      const y = pitchY(view, pitch);
      if (drums) {
        ctx.fillStyle = frame.drumLabels.has(pitch) ? THEME.panelAlt : THEME.panel;
        ctx.fillRect(0, y, w, view.rowH);
        if (used.has(pitch)) {
          ctx.fillStyle = THEME.accent;
          ctx.fillRect(0, y, 3, view.rowH);
        }
      } else {
        const black = isBlackKey(pitch);
        ctx.fillStyle = black ? BLACK_KEY : WHITE_KEY;
        ctx.fillRect(0, y, w, view.rowH);
        if (black) {
          ctx.fillStyle = WHITE_KEY;
          ctx.fillRect(w * 0.62, y, w * 0.38, view.rowH);
        }
      }
      hLine(lines, 0, w, y + view.rowH, s);
      const text = keyLabel(pitch, drums, frame.drumLabels);
      if (text && view.rowH >= 7) {
        ctx.fillStyle = drums ? THEME.text : KEY_TEXT;
        ctx.fillText(text, 4, y + view.rowH / 2, w - 7);
      }
    }
    vLine(lines, w - 0.5, 0, h, s);
    stroke(ctx, lines, THEME.border, 1 / s);
  }

  // -- the ruler ----------------------------------------------------------------------------------

  /** `Ruler.paintEvent`: the playable range, the bars in the timeline's numbering. */
  private paintRuler(frame: RollFrame): void {
    const surface = this.surfaces.ruler;
    const ctx = this.contexts.ruler;
    const s = surface.scale;
    const w = surface.cssWidth;
    const h = surface.cssHeight;
    const { view } = frame;
    ctx.setTransform(s, 0, 0, s, 0, 0);
    ctx.fillStyle = THEME.panelAlt;
    ctx.fillRect(0, 0, w, h);
    const roll = frame.ctx;
    if (roll) {
      const signatures = roll.signatures;
      const [a, z] = playableRange(roll.clip);
      ctx.fillStyle = css(parseHex(THEME.accent), 150 / 255);
      ctx.fillRect(beatX(view, a), h - 4, beatX(view, z) - beatX(view, a), 4);
      const offset = contentToTimeline(roll.clip, 0);
      const b0 = xBeat(view, 0);
      const b1 = xBeat(view, w);
      const font = smallFont(8);
      ctx.font = font;
      ctx.textBaseline = "alphabetic";
      const minPx = this.measure(ctx, font, "0000") + 6;
      let bar = Math.max(0, Math.floor(beatToBar(signatures, Math.max(0, b0 + offset))));
      let step = 1;
      while (barLength(signatures, bar) * step * view.ppb < minPx && step < 4096) step *= 2;
      bar -= bar % step;
      const ticks = new Path2D();
      ctx.fillStyle = THEME.textDim;
      for (let guard = 0; guard < 100_000; guard++, bar += step) {
        const c = barToBeat(signatures, bar) - offset;
        if (c > b1) break;
        const x = beatX(view, c);
        if (x >= -40) {
          vLine(ticks, x, h * 0.35, h, s);
          ctx.fillText(String(bar + 1), x + 3, h * 0.62);
        }
      }
      stroke(ctx, ticks, THEME.gridBar, 1 / s);
    }
    const border = new Path2D();
    hLine(border, 0, w, h - 0.5, s);
    stroke(ctx, border, THEME.border, 1 / s);
  }

  // -- the velocity lane ----------------------------------------------------------------------------

  /** `VelocityLane.paintEvent`: a bar per visible note, the dragged velocities instead. */
  private paintLane(frame: RollFrame): void {
    const surface = this.surfaces.lane;
    const ctx = this.contexts.lane;
    const s = surface.scale;
    const w = surface.cssWidth;
    const h = surface.cssHeight;
    const { view, index, selected, preview, velocities } = frame;
    ctx.setTransform(s, 0, 0, s, 0, 0);
    ctx.fillStyle = THEME.panel;
    ctx.fillRect(0, 0, w, h);
    const guides = new Path2D();
    for (const fraction of [0.25, 0.5, 0.75]) hLine(guides, 0, w, h - 3 - fraction * (h - 6), s);
    stroke(ctx, guides, THEME.gridSub, 1 / s);
    if (index && frame.ctx) {
      const b0 = xBeat(view, 0);
      const b1 = xBeat(view, w);
      const bars = new Map<number, Path2D>();
      const ticks = new Map<number, Path2D>();
      const add = (id: string, start: number, velocity: number) => {
        const vel = velocities.get(id) ?? velocity;
        const x = beatX(view, start);
        const top = h - 3 - (vel / 127) * (h - 6);
        const isSelected = selected.has(id);
        const key = fillKey(vel, isSelected, true);
        let bar = bars.get(key);
        if (!bar) {
          bar = new Path2D();
          bars.set(key, bar);
        }
        bar.rect(x, top, 3, h - 3 - top);
        const tickKey = isSelected ? -1 : key;
        let tick = ticks.get(tickKey);
        if (!tick) {
          tick = new Path2D();
          ticks.set(tickKey, tick);
        }
        hLine(tick, x, x + 7, top, s);
      };
      for (const i of index.visible(b0, b1)) {
        const id = index.ids[i];
        if (!preview.has(id)) add(id, index.starts[i], index.velocities[i]);
      }
      for (const note of preview.values()) {
        if (note.start + note.length > b0 && note.start < b1) add(note.id, note.start, note.velocity);
      }
      for (const [key, path] of bars) {
        ctx.fillStyle = NOTE_FILLS[key];
        ctx.fill(path);
      }
      for (const [key, path] of ticks) stroke(ctx, path, key < 0 ? THEME.selection : NOTE_FILLS[key], 1 / s);
    }
    const border = new Path2D();
    hLine(border, 0, w, 0.5, s);
    stroke(ctx, border, THEME.border, 1 / s);
  }
}

/** `Keyboard.label_for`: "pitch GM name" on drum tracks, the C names otherwise. */
export function keyLabel(pitch: number, drums: boolean, labels: DrumLabels): string {
  if (drums) {
    const label = labels.get(pitch);
    return label ? `${pitch} ${label}` : "";
  }
  return pitch % 12 === 0 ? noteName(pitch) : "";
}

/** The keyboard column's width: 132 px on drum tracks, 64 px otherwise. */
export function keyboardWidth(drums: boolean): number {
  return drums ? DRUM_KEYBOARD_W : KEYBOARD_W;
}

/** A crisp one-device-pixel horizontal line at `y` (CSS pixels). */
function hLine(path: Path2D, x0: number, x1: number, y: number, s: number): void {
  const yy = (Math.round(y * s - 0.5) + 0.5) / s;
  path.moveTo(x0, yy);
  path.lineTo(x1, yy);
}

/** A crisp one-device-pixel vertical line at `x` (CSS pixels). */
function vLine(path: Path2D, x: number, y0: number, y1: number, s: number): void {
  const xx = (Math.round(x * s - 0.5) + 0.5) / s;
  path.moveTo(xx, y0);
  path.lineTo(xx, y1);
}

function stroke(ctx: CanvasRenderingContext2D, path: Path2D, color: string, width: number): void {
  ctx.strokeStyle = color;
  ctx.lineWidth = width;
  ctx.stroke(path);
}
