// The Files tab's drags. An HTML drag cannot hand the system's file URLs to the window,
// so the Files tab drags inside the page, with pointer events, and drops onto the zone the
// timeline registers: over the canvas, the audio track under the pointer at the snapped beat;
// elsewhere in the window, new tracks at the playhead. Drops from the Finder
// arrive through Tauri's drag-and-drop event instead (Timeline.svelte).

/** Where the Files tab's drags land; the positions are the page's CSS pixels. */
export interface FileDropZone {
  /** The drag with `paths` is at `x`, `y` (the drop indicator follows). */
  over(paths: readonly string[], x: number, y: number): void;
  /** The files `paths` were dropped at `x`, `y`. */
  drop(paths: readonly string[], x: number, y: number): void;
  /** The drag left the window or was cancelled. */
  leave(): void;
}

let zone: FileDropZone | null = null;

/** Makes `target` the drop zone until the returned function removes it. */
export function registerFileDropZone(target: FileDropZone): () => void {
  zone = target;
  return () => {
    if (zone === target) zone = null;
  };
}

/** The registered drop zone, or null. */
export function fileDropZone(): FileDropZone | null {
  return zone;
}

/** The distance in pixels a pointer moves before a press becomes a drag (the prototype's 4 px). */
export const DRAG_THRESHOLD = 4;
