// The flat rectangles of one frame of the arrangement (`ArrangementCanvas.paintEvent`): under
// the clips the lanes (alternating, lighter when selected, darker for folders), the grid, the
// loop band, the dashed section lines and the row lines; over the clips the rubber band and the
// drop indicator. Built every frame for the visible part only, so they cost a few hundred
// rectangles at most.

import { beatToX, type TimelineView } from "../state/timeline";
import type { OpenProject } from "../types";
import { darker, lighter, parseHex, type Rgb, THEME, unit } from "./colors";
import { gridLines } from "./grid";
import type { Rect } from "./hit";
import type { Row, TrackLayout } from "./layout";
import { RECT_FLOATS } from "./webgl";

const LANE = parseHex(THEME.lane);
const LANE_ALT = parseHex(THEME.laneAlt);
const GRID = { bar: parseHex(THEME.gridBar), beat: parseHex(THEME.gridBeat), sub: parseHex(THEME.gridSub) };
const LOOP = parseHex(THEME.loop);
const TEXT_DIM = parseHex(THEME.textDim);
const BORDER = parseHex(THEME.border);
const ACCENT = parseHex(THEME.accent);

/** A growable list of rectangles for the renderer's rect pass. */
export class RectList {
  data = new Float32Array(256 * RECT_FLOATS);
  count = 0;

  clear(): void {
    this.count = 0;
  }

  add(x: number, y: number, width: number, height: number, color: Rgb, alpha = 1): void {
    if ((this.count + 1) * RECT_FLOATS > this.data.length) {
      const grown = new Float32Array(this.data.length * 2);
      grown.set(this.data);
      this.data = grown;
    }
    const c = unit(color);
    this.data.set([x, y, width, height, c[0], c[1], c[2], alpha], this.count * RECT_FLOATS);
    this.count += 1;
  }
}

/** What lies under the clips, for a canvas `width` × `height` pixels. */
export function underRects(
  out: RectList,
  project: OpenProject,
  layout: TrackLayout,
  view: TimelineView,
  selectedTracks: ReadonlySet<string>,
  width: number,
  height: number,
): void {
  out.clear();
  const rows = layout.visibleRows(view.scrollY, height);
  for (const row of rows) {
    let color = row.index % 2 ? LANE_ALT : LANE;
    if (selectedTracks.has(row.trackId)) color = lighter(color, 112);
    if (row.kind === "folder") color = darker(color, 108);
    out.add(0, row.top - view.scrollY, width, row.height, color);
  }
  const bottom = Math.min(height, layout.contentHeight - view.scrollY);
  if (bottom > 0) {
    for (const line of gridLines(view, project.time_signatures, 0, width)) {
      out.add(line.x, 0, 1, bottom, GRID[line.kind]);
    }
    const transport = project.transport;
    if (transport.loop_enabled) {
      const x0 = beatToX(view, transport.loop_start);
      const x1 = beatToX(view, transport.loop_end);
      out.add(x0, 0, x1 - x0, bottom, LOOP, 18 / 255);
    }
    for (const section of project.sections) {
      const x = Math.round(beatToX(view, section.start));
      if (x < -1 || x > width + 1) continue;
      for (let y = 0; y < bottom; y += 6) out.add(x, y, 1, Math.min(4, bottom - y), TEXT_DIM);
    }
  }
  for (const row of rows) out.add(0, row.top - view.scrollY + row.height - 1, width, 1, BORDER);
}

/** A file drop's target: the audio row under the pointer (or none) and the beat. */
export interface DropMark {
  readonly row: Row | null;
  readonly beat: number;
}

/** What lies over the clips: the rubber band and the drop indicator. */
export function overRects(
  out: RectList,
  view: TimelineView,
  rubber: Rect | null,
  drop: DropMark | null,
  width: number,
  height: number,
): void {
  out.clear();
  if (rubber) {
    out.add(rubber.x, rubber.y, rubber.width, rubber.height, ACCENT, 40 / 255);
    out.add(rubber.x, rubber.y, rubber.width, 1, ACCENT);
    out.add(rubber.x, rubber.y + rubber.height, rubber.width + 1, 1, ACCENT);
    out.add(rubber.x, rubber.y, 1, rubber.height, ACCENT);
    out.add(rubber.x + rubber.width, rubber.y, 1, rubber.height, ACCENT);
  }
  if (drop) {
    const x = Math.round(beatToX(view, drop.beat));
    if (drop.row) {
      const y = drop.row.top - view.scrollY;
      out.add(0, y, width, drop.row.height, ACCENT, 30 / 255);
      out.add(x - 0.5, y, 2, drop.row.height, ACCENT);
    } else {
      out.add(x - 0.5, 0, 2, height, ACCENT);
    }
  }
}
