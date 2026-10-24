// From the ADR 0009 spike: keeps a canvas's backing
// store at its CSS size times devicePixelRatio, and follows changes of both: a resize, or a move
// to a display with another scale factor (the owner's setup mixes 2x and 1x displays).
//
// Step 11 moved the scale watcher out of the class (`watchDevicePixelRatio`), so that the canvases
// that size themselves (the timeline's ruler, the mixer's meters) follow a display change too, and
// so that a test can drive it with a window of its own.

/** What the scale watcher reads of the window. */
export interface PixelRatioSource {
  readonly devicePixelRatio: number;
  matchMedia(query: string): Pick<MediaQueryList, "addEventListener" | "removeEventListener">;
}

/**
 * Calls `onChange` with the new device pixel ratio whenever it changes: a `resolution` media query
 * for the current ratio stops matching when the window moves to a display with another scale (or
 * the page zooms), and the watcher then listens for the next one. Returns the stop.
 */
export function watchDevicePixelRatio(
  onChange: (dpr: number) => void,
  source: PixelRatioSource = window,
): () => void {
  let media: Pick<MediaQueryList, "addEventListener" | "removeEventListener"> | null = null;
  const changed = () => {
    arm();
    onChange(source.devicePixelRatio || 1);
  };
  const arm = () => {
    media?.removeEventListener("change", changed);
    media = source.matchMedia(`(resolution: ${source.devicePixelRatio || 1}dppx)`);
    media.addEventListener("change", changed);
  };
  arm();
  return () => {
    media?.removeEventListener("change", changed);
    media = null;
  };
}

/** The backing store's size of a canvas `width` × `height` CSS pixels at `dpr`, at least 1 × 1. */
export function backingSize(width: number, height: number, dpr: number): { width: number; height: number } {
  return { width: Math.max(1, Math.round(width * dpr)), height: Math.max(1, Math.round(height * dpr)) };
}

export class HiDpiCanvas {
  cssWidth = 0;
  cssHeight = 0;
  dpr = 1;
  private readonly observer: ResizeObserver;
  private readonly stopWatching: () => void;

  constructor(
    readonly canvas: HTMLCanvasElement,
    private readonly onResize: () => void = () => {},
  ) {
    this.observer = new ResizeObserver(() => this.update());
    this.observer.observe(canvas);
    this.stopWatching = watchDevicePixelRatio(() => this.update());
    this.update();
  }

  /** Device pixels per CSS pixel actually used by the backing store. */
  get scale(): number {
    return this.cssWidth > 0 ? this.canvas.width / this.cssWidth : this.dpr;
  }

  update() {
    const rect = this.canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const { width, height } = backingSize(rect.width, rect.height, dpr);
    this.cssWidth = rect.width;
    this.cssHeight = rect.height;
    const scaleChanged = dpr !== this.dpr;
    this.dpr = dpr;
    if (this.canvas.width !== width || this.canvas.height !== height) {
      this.canvas.width = width;
      this.canvas.height = height;
      this.onResize();
    } else if (scaleChanged) {
      // The same backing size at another scale (a canvas of a fractional CSS size) still draws
      // with the new transform.
      this.onResize();
    }
  }

  destroy() {
    this.observer.disconnect();
    this.stopWatching();
  }
}
