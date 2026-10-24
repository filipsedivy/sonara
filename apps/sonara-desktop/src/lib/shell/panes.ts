// The window's panes: the Browser on the left, the Inspector on the right and the Editors
// (Mixer, Piano Roll, Devices) below the timeline. Each can be resized with its splitter and
// shown or hidden from the View menu; their sizes are saved with the window (sonara-app's
// `window_layout`). The defaults are the prototype's dock sizes.

import type { Pane, PaneLayout } from "../types";

export type PaneName = keyof PaneLayout;

export const DEFAULT_PANES: PaneLayout = {
  browser: { size: 240, open: true },
  inspector: { size: 280, open: true },
  editors: { size: 300, open: true },
};

/** The smallest and largest size of each pane, in pixels. */
export const PANE_LIMITS: Record<PaneName, readonly [number, number]> = {
  browser: [160, 640],
  inspector: [200, 640],
  editors: [160, 1200],
};

/** The timeline keeps at least this much room beside and above the panes. */
export const MIN_CENTER = 320;

/** `size` within the pane's limits, and leaving the timeline `MIN_CENTER` of `available`. */
export function clampSize(name: PaneName, size: number, available = Infinity): number {
  const [min, max] = PANE_LIMITS[name];
  const room = Math.max(min, available - MIN_CENTER);
  return Math.round(Math.min(Math.max(size, min), max, room));
}

/** The layout with `name` resized to `size`. */
export function resize(layout: PaneLayout, name: PaneName, size: number, available = Infinity): PaneLayout {
  const pane: Pane = { ...layout[name], size: clampSize(name, size, available) };
  return { ...layout, [name]: pane };
}

/** The layout with `name` shown or hidden. */
export function toggle(layout: PaneLayout, name: PaneName, open = !layout[name].open): PaneLayout {
  return { ...layout, [name]: { ...layout[name], open } };
}

/** The layout as saved, with each size within its limits. */
export function restore(saved: PaneLayout | null | undefined): PaneLayout {
  if (!saved) return DEFAULT_PANES;
  const pane = (name: PaneName): Pane => ({
    size: clampSize(name, Number.isFinite(saved[name]?.size) ? saved[name].size : DEFAULT_PANES[name].size),
    open: saved[name]?.open ?? true,
  });
  return { browser: pane("browser"), inspector: pane("inspector"), editors: pane("editors") };
}
