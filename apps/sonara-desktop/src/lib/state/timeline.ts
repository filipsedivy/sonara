// The GUI-only view state of the prototype's timeline and transport (sonara/ui/panels.py
// `PlayheadModel`, sonara/ui/timeline/scale.py `TimeScale`, the panel's Follow): the playhead,
// the zoom, the horizontal scroll, the track height and Follow. None of it is persisted.

import type { LiveFrame } from "../live/frame";

/** The zoom range in pixels per beat, and its default. */
export const MIN_PPB = 1.5;
export const MAX_PPB = 1200;
export const DEFAULT_PPB = 24;

/** The track heights S, M, L and XL in pixels. */
export const TRACK_HEIGHTS = { S: 34, M: 56, L: 88, XL: 132 } as const;
export type TrackHeight = keyof typeof TRACK_HEIGHTS;

/** The playhead as the GUI shows it (`PlayheadModel`). */
export interface Playhead {
  readonly beat: number;
  readonly playing: boolean;
}

export const START_PLAYHEAD: Playhead = { beat: 0, playing: false };

/**
 * The transport bar's `_on_tick` for one live frame: while playing, recording or paused the
 * playhead takes the frame's beat; otherwise it stays where the user put it and stops playing.
 */
export function playheadFromFrame(playhead: Playhead, frame: LiveFrame): Playhead {
  const playing = frame.state === "playing" || frame.state === "recording";
  const beat = frame.position.beat;
  if (beat !== null && (playing || frame.state === "paused")) {
    const moved = Math.abs(beat - playhead.beat) > 1e-6;
    if (!moved && playhead.playing === playing) return playhead;
    return { beat: moved ? beat : playhead.beat, playing };
  }
  return playhead.playing ? { ...playhead, playing: false } : playhead;
}

/** The order of the track heights (⌘⌥ wheel steps through it). */
export const HEIGHT_ORDER: readonly TrackHeight[] = ["S", "M", "L", "XL"];

/** Beats of room past the song end (`EXTRA_BEATS`). */
export const EXTRA_BEATS = 64;

/**
 * The timeline's view (`x = beat · pixelsPerBeat − scrollX`, `y = top − scrollY`) and its
 * options.
 */
export interface TimelineView {
  readonly pixelsPerBeat: number;
  readonly scrollX: number;
  /** The vertical scroll of the rows, in pixels. */
  readonly scrollY: number;
  readonly trackHeight: TrackHeight;
  /** Follow: scroll with the playhead during playback (on by default). */
  readonly follow: boolean;
}

export const DEFAULT_TIMELINE: TimelineView = {
  pixelsPerBeat: DEFAULT_PPB,
  scrollX: 0,
  scrollY: 0,
  trackHeight: "M",
  follow: true,
};

export function beatToX(view: TimelineView, beat: number): number {
  return beat * view.pixelsPerBeat - view.scrollX;
}

export function xToBeat(view: TimelineView, x: number): number {
  return (x + view.scrollX) / view.pixelsPerBeat;
}

/** `set_zoom`: clamped to the range, keeping the beat under `anchorX` in place. */
export function setZoom(view: TimelineView, pixelsPerBeat: number, anchorX = 0): TimelineView {
  const ppb = Math.min(Math.max(pixelsPerBeat, MIN_PPB), MAX_PPB);
  if (Math.abs(ppb - view.pixelsPerBeat) <= 1e-9 * Math.max(ppb, view.pixelsPerBeat)) return view;
  const beat = xToBeat(view, anchorX);
  return { ...view, pixelsPerBeat: ppb, scrollX: Math.max(0, beat * ppb - anchorX) };
}

/** `zoom_by`: the zoom times `factor`, anchored at `anchorX`. */
export function zoomBy(view: TimelineView, factor: number, anchorX = 0): TimelineView {
  return setZoom(view, view.pixelsPerBeat * factor, anchorX);
}

/** `set_scroll`: never before the start. */
export function setScroll(view: TimelineView, scrollX: number): TimelineView {
  const x = Math.max(0, scrollX);
  return x === view.scrollX ? view : { ...view, scrollX: x };
}

/** `set_scroll` of the rows: never above the first. */
export function setScrollY(view: TimelineView, scrollY: number): TimelineView {
  const y = Math.max(0, scrollY);
  return y === view.scrollY ? view : { ...view, scrollY: y };
}

/** `set_view`: the zoom (clamped) and the scroll at once. */
export function setView(view: TimelineView, pixelsPerBeat: number, scrollX: number): TimelineView {
  const ppb = Math.min(Math.max(pixelsPerBeat, MIN_PPB), MAX_PPB);
  return { ...view, pixelsPerBeat: ppb, scrollX: Math.max(0, scrollX) };
}

/** The scroll limits of `_update_ranges`. */
export interface ScrollRange {
  /** The largest horizontal scroll. */
  readonly maxX: number;
  /** The largest vertical scroll. */
  readonly maxY: number;
}

/**
 * `_update_ranges`: the content reaches 64 beats past the song end or the playhead (or past
 * one screen), and 40 pixels below the last row.
 */
export function scrollRange(
  view: TimelineView,
  endBeat: number,
  playheadBeat: number,
  width: number,
  height: number,
  contentHeight: number,
): ScrollRange {
  const w = Math.max(1, width);
  const end = Math.max(endBeat, playheadBeat);
  const visibleBeats = w / view.pixelsPerBeat;
  const contentPx = (Math.max(end, visibleBeats) + EXTRA_BEATS) * view.pixelsPerBeat;
  return {
    maxX: Math.max(0, Math.round(contentPx - w)),
    maxY: Math.max(0, contentHeight + 40 - Math.max(1, height)),
  };
}

/** The view with its scroll inside `range`. */
export function clampScroll(view: TimelineView, range: ScrollRange): TimelineView {
  const x = Math.min(view.scrollX, range.maxX);
  const y = Math.min(view.scrollY, range.maxY);
  return x === view.scrollX && y === view.scrollY ? view : { ...view, scrollX: x, scrollY: y };
}

/** `zoom_fit`: the whole song (at least 16 beats) in `width` less 24 pixels, from the start. */
export function fitView(view: TimelineView, endBeat: number, width: number): TimelineView {
  const end = Math.max(16, endBeat);
  return setView(view, Math.max(100, width - 24) / end, 0);
}

/** One step of the track height, up (+1) or down (−1), within S–XL. */
export function stepHeight(view: TimelineView, direction: number): TimelineView {
  const i = HEIGHT_ORDER.indexOf(view.trackHeight);
  const next = HEIGHT_ORDER[Math.max(0, Math.min(HEIGHT_ORDER.length - 1, i + direction))];
  return next === view.trackHeight ? view : { ...view, trackHeight: next };
}

/**
 * Follow: while playing, once the playhead passes 90 % of `width` or leaves the view,
 * the view scrolls to put it at 10 %.
 */
export function followPlayhead(view: TimelineView, playhead: Playhead, width: number): TimelineView {
  if (!view.follow || !playhead.playing) return view;
  const x = beatToX(view, playhead.beat);
  if (x <= width * 0.9 && x >= 0) return view;
  return setScroll(view, Math.round(playhead.beat * view.pixelsPerBeat - width * 0.1));
}
