// The timeline's ruler (`Ruler` in sonara/ui/timeline/ruler.py), 46 px high: a 16 px band of
// sections and markers, an 8 px loop band with its handles, and the bar numbers, beat ticks and
// time signatures. A click or drag seeks without snapping; dragging a loop handle moves
// that end, ⇧-dragging draws a new loop and turns it on, both snapped (⌥ bypasses) and committed
// once on release; a double-click on the loop band toggles the loop.

import { beatToX, type TimelineView, xToBeat } from "../state/timeline";
import type { SnapMode } from "../state/snap";
import type { Edit, OpenProject } from "../types";
import { blend, css, parseHex, type Rgb, THEME, trackColor } from "./colors";
import type { Mods } from "./gestures";
import { rulerMarks } from "./grid";

export const RULER_HEIGHT = 46;
/** The sections and markers band. */
export const BAND = 16;
export const LOOP_BAND = 8;
/** How close to a loop end a press grabs it. */
export const HANDLE_PX = 6;

/** What the ruler's gestures read. */
export interface RulerState {
  readonly project: OpenProject;
  readonly view: TimelineView;
  snap(beat: number, mode: SnapMode, bypass: boolean): number;
}

/** What they do. */
export interface RulerActions {
  /** MW-27's seek intent. */
  seek(beat: number): void;
  /** A loop drag's edit: committed once, then the undo step is sealed. */
  commit(edit: Edit): Promise<unknown>;
  /** The loop toggle of a double-click. */
  run(edit: Edit): Promise<unknown>;
}

type Mode = "seek" | "loop_start" | "loop_end" | "loop_new";

function inLoopBand(y: number): boolean {
  return y >= BAND && y <= BAND + LOOP_BAND + 4;
}

export class RulerGestures {
  mode: Mode | null = null;
  /** The loop being dragged. */
  loopPreview: [number, number] | null = null;
  private pressBeat = 0;

  constructor(
    private readonly state: () => RulerState | null,
    private readonly actions: RulerActions,
  ) {}

  /** The loop as drawn: the preview while dragging, else the project's. */
  loopRange(): [number, number] {
    if (this.loopPreview) return this.loopPreview;
    const transport = this.state()?.project.transport;
    return transport ? [transport.loop_start, transport.loop_end] : [0, 0];
  }

  /** `_beat`: never before 0; snapped unless `snap` is false (⌥ bypasses). */
  private beat(state: RulerState, x: number, mods: Mods, snap = true): number {
    const beat = Math.max(0, xToBeat(state.view, x));
    return snap ? state.snap(beat, "nearest", mods.alt) : beat;
  }

  press(x: number, y: number, mods: Mods): void {
    const state = this.state();
    if (!state) return;
    const [start, end] = this.loopRange();
    const xs = beatToX(state.view, start);
    const xe = beatToX(state.view, end);
    if (mods.shift) {
      this.mode = "loop_new";
      this.pressBeat = this.beat(state, x, mods);
      this.loopPreview = [this.pressBeat, this.pressBeat];
    } else if (inLoopBand(y) && Math.abs(x - xs) <= HANDLE_PX) {
      this.mode = "loop_start";
      this.loopPreview = [start, end];
    } else if (inLoopBand(y) && Math.abs(x - xe) <= HANDLE_PX) {
      this.mode = "loop_end";
      this.loopPreview = [start, end];
    } else {
      this.mode = "seek";
      this.actions.seek(this.beat(state, x, mods, false));
    }
  }

  /** The drag's step, or the cursor over the handles while nothing is pressed. */
  move(x: number, y: number, mods: Mods): "default" | "ew-resize" {
    const state = this.state();
    if (!state) return "default";
    if (this.mode === null) {
      const [start, end] = this.loopRange();
      const near =
        inLoopBand(y) &&
        (Math.abs(x - beatToX(state.view, start)) <= HANDLE_PX ||
          Math.abs(x - beatToX(state.view, end)) <= HANDLE_PX);
      return near ? "ew-resize" : "default";
    }
    if (this.mode === "seek") {
      this.actions.seek(this.beat(state, x, mods, false));
      return "default";
    }
    const beat = this.beat(state, x, mods);
    const [start, end] = this.loopPreview ?? [0, 0];
    if (this.mode === "loop_start") this.loopPreview = [Math.min(beat, end - 1e-3), end];
    else if (this.mode === "loop_end") this.loopPreview = [start, Math.max(beat, start + 1e-3)];
    else this.loopPreview = this.pressBeat <= beat ? [this.pressBeat, beat] : [beat, this.pressBeat];
    return "ew-resize";
  }

  /** The release: a loop drag commits one edit and seals it. */
  async release(): Promise<void> {
    const mode = this.mode;
    const preview = this.loopPreview;
    this.mode = null;
    this.loopPreview = null;
    if (!mode || mode === "seek" || !preview) return;
    const [start, end] = preview;
    if (end - start <= 1e-3) return;
    const edit: Edit =
      mode === "loop_new"
        ? { op: "set_transport", loop_start: start, loop_end: end, loop_enabled: true }
        : { op: "set_transport", loop_start: start, loop_end: end };
    await this.actions.commit(edit);
  }

  /** A double-click on the loop band toggles the loop. */
  async doubleClick(y: number): Promise<void> {
    const state = this.state();
    if (!state || !inLoopBand(y)) return;
    await this.actions.run({ op: "set_transport", loop_enabled: !state.project.transport.loop_enabled });
  }
}

const PANEL = parseHex(THEME.panel);
const LIGHT_TEXT = "#f2f4f7";
const DARK_TEXT = "#111317";
const FONT = '10.5px -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif';
const SMALL = '9.5px -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif';

/** HSL lightness (`QColor.lightnessF`). */
function lightness(c: Rgb): number {
  return (Math.max(...c) + Math.min(...c)) / 2 / 255;
}

function elide(ctx: CanvasRenderingContext2D, text: string, width: number): string {
  if (ctx.measureText(text).width <= width) return text;
  for (let n = text.length - 1; n > 0; n--) {
    const candidate = `${text.slice(0, n)}…`;
    if (ctx.measureText(candidate).width <= width) return candidate;
  }
  return "";
}

/** `Ruler.paintEvent` without the playhead (the page draws it). */
export function drawRuler(
  ctx: CanvasRenderingContext2D,
  project: OpenProject | null,
  view: TimelineView,
  loop: [number, number],
  previewing: boolean,
  width: number,
  scale: number,
): void {
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
  ctx.setTransform(scale, 0, 0, scale, 0, 0);
  ctx.fillStyle = THEME.panel;
  ctx.fillRect(0, 0, width, RULER_HEIGHT);
  if (project) {
    sections(ctx, project, view, width);
    loopBand(ctx, project, view, loop, previewing);
    bars(ctx, project, view, width);
  }
  ctx.fillStyle = THEME.border;
  ctx.fillRect(0, RULER_HEIGHT - 1, width, 1);
  ctx.fillRect(0, BAND - 1, width, 1);
}

function sections(ctx: CanvasRenderingContext2D, project: OpenProject, view: TimelineView, width: number): void {
  ctx.font = SMALL;
  ctx.textBaseline = "middle";
  project.sections.forEach((section, i) => {
    const x0 = beatToX(view, section.start);
    const x1 = beatToX(view, section.start + section.length);
    if (x1 < 0 || x0 > width) return;
    const color = section.color ? parseHex(section.color) : trackColor(i + 6);
    const rect = { x: x0 + 1, y: 2, width: Math.max(2, x1 - x0 - 2), height: BAND - 4 };
    ctx.fillStyle = css(blend(PANEL, color, 0.55));
    ctx.beginPath();
    ctx.roundRect(rect.x, rect.y, rect.width, rect.height, 3);
    ctx.fill();
    const left = Math.max(rect.x, 0) + 4;
    const textWidth = rect.x + rect.width - 3 - left;
    if (textWidth > 10) {
      ctx.fillStyle = lightness(color) < 0.75 ? LIGHT_TEXT : DARK_TEXT;
      const text = elide(ctx, section.name, textWidth);
      if (text) ctx.fillText(text, left, rect.y + rect.height / 2);
    }
  });
  for (const marker of project.markers) {
    const x = beatToX(view, marker.beat);
    if (x < -60 || x > width) continue;
    ctx.fillStyle = marker.color ?? THEME.selection;
    ctx.beginPath();
    ctx.moveTo(x, 2);
    ctx.lineTo(x + 7, 2);
    ctx.lineTo(x, 11);
    ctx.closePath();
    ctx.fill();
    ctx.fillStyle = THEME.text;
    ctx.textBaseline = "alphabetic";
    ctx.fillText(marker.name, x + 9, 11);
    ctx.textBaseline = "middle";
  }
}

function loopBand(
  ctx: CanvasRenderingContext2D,
  project: OpenProject,
  view: TimelineView,
  [start, end]: [number, number],
  previewing: boolean,
): void {
  const enabled = project.transport.loop_enabled || previewing;
  const x0 = beatToX(view, start);
  const x1 = beatToX(view, end);
  const loop = parseHex(THEME.loop);
  ctx.fillStyle = css(loop, (enabled ? 170 : 55) / 255);
  ctx.fillRect(x0, BAND, x1 - x0, LOOP_BAND);
  if (enabled) {
    ctx.fillStyle = css(loop, 28 / 255);
    ctx.fillRect(x0, BAND + LOOP_BAND, x1 - x0, RULER_HEIGHT - BAND - LOOP_BAND);
  }
}

function bars(ctx: CanvasRenderingContext2D, project: OpenProject, view: TimelineView, width: number): void {
  const tickTop = BAND + LOOP_BAND;
  ctx.textBaseline = "alphabetic";
  for (const mark of rulerMarks(view, project.time_signatures, width)) {
    if (mark.kind === "bar") {
      ctx.fillStyle = THEME.textDim;
      ctx.fillRect(mark.x, tickTop, 1, RULER_HEIGHT - tickTop);
      ctx.font = FONT;
      ctx.fillStyle = THEME.text;
      ctx.fillText(mark.label, mark.x + 4, tickTop + 12);
    } else if (mark.kind === "signature") {
      ctx.font = SMALL;
      ctx.fillStyle = THEME.textDim;
      ctx.fillText(mark.label, mark.x + 4, RULER_HEIGHT - 3);
    } else {
      ctx.fillStyle = THEME.gridBar;
      ctx.fillRect(mark.x, RULER_HEIGHT - 7, 1, 7);
      if (mark.label) {
        ctx.font = SMALL;
        ctx.fillStyle = THEME.textDim;
        ctx.fillText(mark.label, mark.x + 3, tickTop + 12);
      }
    }
  }
}
