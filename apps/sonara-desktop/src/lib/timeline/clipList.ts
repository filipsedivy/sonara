// The arrangement's clips for assistive technology: the canvas draws them, so a list box of transparent options over it names each
// one ("Riff, bar 2 beat 1, 4 beats, selected, 1 of 40" in its track's group) and takes the focus.
// Only the clips the canvas shows are options, plus the focused one wherever it is, so the list
// costs what the visible rows cost (the prototype's `_visible_clips`). ↑ and ↓ move to the previous
// or next clip in the tracks' order and select it as a click does; the canvas's own keys then act
// on it. Plain functions; Timeline.svelte renders them.

import { clipOptionLabel } from "../a11y/labels";
import { xToBeat, type TimelineView } from "../state/timeline";
import { clipRect, type Rect, trackOf } from "./hit";
import type { Row, TrackLayout } from "./layout";
import { clipEnd, type PlacedClip, type TimelineModel } from "./model";

/** One clip's option. */
export interface ClipOption {
  readonly id: string;
  readonly label: string;
  readonly selected: boolean;
  /** Its place among its track's clips, from 1. */
  readonly position: number;
  /** Its track's clips. */
  readonly count: number;
  /** Where the canvas draws it, in CSS pixels from the canvas's top left. */
  readonly rect: Rect;
}

/** A track's clip options, in start order. */
export interface ClipGroup {
  readonly trackId: string;
  readonly name: string;
  readonly options: readonly ClipOption[];
}

/** The element id of a clip's option. */
export function clipOptionId(clipId: string): string {
  return `clip-option-${clipId}`;
}

/**
 * The options of the clips a `width` × `height` canvas shows at `view`, grouped by track in the
 * tracks' order, and of `keep` (the focused clip) wherever it is.
 */
export function clipGroups(
  model: TimelineModel,
  layout: TrackLayout,
  view: TimelineView,
  width: number,
  height: number,
  selected: ReadonlySet<string>,
  keep: string | null,
): ClipGroup[] {
  const signatures = model.project.time_signatures;
  const b0 = xToBeat(view, 0);
  const b1 = xToBeat(view, Math.max(0, width));
  const shown = new Map<string, Set<string>>();
  for (const row of layout.visibleRows(view.scrollY, Math.max(0, height))) {
    const track = trackOf(model, row);
    if (!track) continue;
    shown.set(track.track.id, new Set(model.visibleClips(track, b0, b1).map((clip) => clip.id)));
  }
  const kept = keep ? model.clip(keep) : undefined;
  if (kept) {
    const ids = shown.get(kept.track.track.id) ?? new Set<string>();
    ids.add(kept.clip.id);
    shown.set(kept.track.track.id, ids);
  }
  const groups: ClipGroup[] = [];
  for (const track of model.tracks) {
    const ids = shown.get(track.track.id);
    const row = layout.row(track.track.id);
    if (!ids || ids.size === 0 || !row) continue;
    const options: ClipOption[] = [];
    track.clips.forEach((clip, i) => {
      if (!ids.has(clip.id)) return;
      options.push({
        id: clip.id,
        label: clipOptionLabel(clip, signatures),
        selected: selected.has(clip.id),
        position: i + 1,
        count: track.clips.length,
        rect: clipRect(view, row, clip),
      });
    });
    groups.push({ trackId: track.track.id, name: track.track.name, options });
  }
  return groups;
}

/**
 * The clip after (`step` 1) or before (−1) `clipId` in the tracks' order, then by start; without
 * `clipId` (or when it is gone) the first clip, or the last one going back. Null without clips, and
 * at either end the clip itself.
 */
export function adjacentClip(model: TimelineModel, clipId: string | null, step: 1 | -1): PlacedClip | null {
  const order: PlacedClip[] = model.tracks.flatMap((track) => track.clips.map((clip) => ({ track, clip })));
  if (order.length === 0) return null;
  const at = clipId === null ? -1 : order.findIndex((placed) => placed.clip.id === clipId);
  if (at < 0) return step > 0 ? order[0] : order[order.length - 1];
  return order[Math.min(order.length - 1, Math.max(0, at + step))];
}

/**
 * The scroll that shows `clip` on `row` in a `width` × `height` canvas: unchanged when it shows
 * whole, else the least move that shows it (its start first when it is wider than the canvas).
 */
export function revealClip(
  view: TimelineView,
  row: Row,
  clip: PlacedClip["clip"],
  width: number,
  height: number,
): { scrollX: number; scrollY: number } {
  const margin = 8;
  const x0 = clip.start * view.pixelsPerBeat;
  const x1 = clipEnd(clip) * view.pixelsPerBeat;
  let scrollX = view.scrollX;
  if (x0 < scrollX) scrollX = Math.max(0, x0 - margin);
  else if (x1 > scrollX + width) scrollX = Math.max(0, Math.min(x0 - margin, x1 - width + margin));
  let scrollY = view.scrollY;
  if (row.top < scrollY) scrollY = row.top;
  else if (row.top + row.height > scrollY + height) scrollY = Math.max(0, row.top + row.height - height);
  return { scrollX, scrollY };
}
