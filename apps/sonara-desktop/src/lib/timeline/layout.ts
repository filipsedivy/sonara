// The timeline's rows (the prototype's `TrackLayout` in sonara/ui/timeline/scale.py): one row
// per track in order, the master excluded; folders 14 px shorter, never below 24 px; the depth
// indents a header (at most 6 levels, from the project view). Positions are content pixels: a
// row's top before the vertical scroll.

import type { TrackKind, TrackView } from "../types";

/** The smallest row. */
export const MIN_ROW_HEIGHT = 24;
/** How much shorter a folder's row is. */
export const FOLDER_SHRINK = 14;

export interface Row {
  readonly trackId: string;
  /** The index in the project's tracks. */
  readonly index: number;
  /** Content y (before the vertical scroll). */
  readonly top: number;
  readonly height: number;
  readonly depth: number;
  readonly kind: TrackKind;
}

/** Whether `next` lays out as `prev` at the same height: the same tracks, kinds and depths. */
export function sameRows(prev: readonly TrackView[], next: readonly TrackView[]): boolean {
  return (
    prev === next ||
    (prev.length === next.length &&
      prev.every((a, i) => {
        const b = next[i];
        return a.id === b.id && a.kind === b.kind && a.depth === b.depth;
      }))
  );
}

export class TrackLayout {
  readonly rows: readonly Row[];
  private readonly tops: number[];
  private readonly byId: Map<string, Row>;

  constructor(tracks: readonly TrackView[], readonly trackHeight: number) {
    const rows: Row[] = [];
    let y = 0;
    tracks.forEach((track, index) => {
      const height = Math.max(MIN_ROW_HEIGHT, trackHeight - (track.kind === "folder" ? FOLDER_SHRINK : 0));
      rows.push({ trackId: track.id, index, top: y, height, depth: Math.min(track.depth, 6), kind: track.kind });
      y += height;
    });
    this.rows = rows;
    this.tops = rows.map((row) => row.top);
    this.byId = new Map(rows.map((row) => [row.trackId, row]));
  }

  /** The height of every row. */
  get contentHeight(): number {
    const last = this.rows.at(-1);
    return last ? last.top + last.height : 0;
  }

  row(trackId: string): Row | undefined {
    return this.byId.get(trackId);
  }

  /** The last index whose top is at or before `y` (`bisect_right − 1`). */
  private indexAt(y: number): number {
    let lo = 0;
    let hi = this.tops.length;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (this.tops[mid] <= y) lo = mid + 1;
      else hi = mid;
    }
    return lo - 1;
  }

  /** The row under the canvas `y`, scrolled by `scrollY` (`row_at`). */
  rowAt(y: number, scrollY: number): Row | null {
    const cy = y + scrollY;
    const i = this.indexAt(cy);
    const row = this.rows[i];
    return i >= 0 && row && cy < row.top + row.height ? row : null;
  }

  /** The rows that intersect a canvas `height` pixels high (`visible_rows`). */
  visibleRows(scrollY: number, height: number): Row[] {
    if (this.rows.length === 0) return [];
    const lo = Math.max(0, this.indexAt(scrollY));
    const hi = this.indexAt(scrollY + height) + 1;
    return this.rows.slice(lo, hi);
  }
}
