// Clip rectangles and hit testing on the arrangement canvas (`ArrangementCanvas.clip_rect` and
// `hit`): a clip covers its rounded beats from half a pixel in, one and a half pixels below its
// row's top, three pixels short of the row; its left and right 6 px are trim zones when it is
// at least 18 px wide; a hit allows one pixel of slack on either side. Canvas coordinates are
// CSS pixels from the canvas's top left.

import { beatToX, type TimelineView, xToBeat } from "../state/timeline";
import type { ClipView } from "../types";
import type { Row, TrackLayout } from "./layout";
import { clipEnd, type TimelineModel, type TrackClips } from "./model";

/** The width of a trim zone (`EDGE_PX`). */
export const EDGE_PX = 6;
/** How far the pointer moves before a press becomes a drag (`DRAG_THRESHOLD`). */
export const DRAG_THRESHOLD = 4;

export interface Rect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export type Region = "body" | "left" | "right";

export interface Hit {
  readonly track: TrackClips;
  readonly clip: ClipView;
  readonly region: Region;
  readonly rect: Rect;
}

/** `clip_rect` of `clip` on `row`, between `start` and `end` (the clip's own by default). */
export function clipRect(
  view: TimelineView,
  row: Row,
  clip: ClipView,
  start: number = clip.start,
  end: number = clipEnd(clip),
): Rect {
  const x0 = Math.round(beatToX(view, start));
  const x1 = Math.round(beatToX(view, end));
  const y = row.top - view.scrollY;
  return { x: x0 + 0.5, y: y + 1.5, width: Math.max(2, x1 - x0 - 1), height: row.height - 3 };
}

export function contains(rect: Rect, x: number, y: number): boolean {
  return x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height;
}

/** The track of `row` in `model`. */
export function trackOf(model: TimelineModel, row: Row): TrackClips | undefined {
  const byIndex = model.tracks[row.index];
  return byIndex && byIndex.track.id === row.trackId ? byIndex : model.track(row.trackId);
}

/** `hit`: the topmost clip under (`x`, `y`) and the region the pointer is in. */
export function hitTest(
  model: TimelineModel,
  layout: TrackLayout,
  view: TimelineView,
  x: number,
  y: number,
): Hit | null {
  const row = layout.rowAt(y, view.scrollY);
  if (!row) return null;
  const track = trackOf(model, row);
  if (!track) return null;
  const beat = xToBeat(view, x);
  const tolerance = EDGE_PX / view.pixelsPerBeat;
  const clips = model.visibleClips(track, beat - tolerance, beat + tolerance);
  for (let i = clips.length - 1; i >= 0; i--) {
    const clip = clips[i];
    const rect = clipRect(view, row, clip);
    const slack = { ...rect, x: rect.x - 1, width: rect.width + 2 };
    if (!contains(slack, x, y)) continue;
    let region: Region = "body";
    if (rect.width >= 3 * EDGE_PX) {
      if (x - rect.x <= EDGE_PX) region = "left";
      else if (rect.x + rect.width - x <= EDGE_PX) region = "right";
    }
    return { track, clip, region, rect };
  }
  return null;
}
