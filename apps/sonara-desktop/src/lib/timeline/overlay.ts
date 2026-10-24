// The arrangement's text, on a transparent 2D canvas over the WebGL one: the clip names in their
// headers ("(muted)" when muted, elided to the clip's width), and "media missing" in red on audio
// clips whose media is gone (`ClipPainter.paint` and `_missing`). Only the visible clips are
// drawn, found by bisection as the prototype does.

import { xToBeat, type TimelineView } from "../state/timeline";
import type { ClipView } from "../types";
import { css, luminance, parseHex, THEME } from "./colors";
import { clipRect, type Rect, trackOf } from "./hit";
import type { TrackLayout } from "./layout";
import type { TimelineModel, TrackClips } from "./model";
import { clipColors, type Placement } from "./scene";

const HEADER_H = 14;
const FONT = '10px -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif';
const DARK_TEXT = "#111317";
const LIGHT_TEXT = "#f2f4f7";
const MISSING = parseHex(THEME.meterRed);

/** What the overlay draws in one frame. */
export interface OverlayFrame {
  readonly model: TimelineModel;
  readonly layout: TrackLayout;
  readonly view: TimelineView;
  readonly scale: number;
  readonly width: number;
  readonly height: number;
  readonly selected: ReadonlySet<string>;
  readonly ghosts: ReadonlySet<string>;
  readonly previews: readonly Placement[];
  readonly missing: ReadonlySet<string>;
}

export class TextOverlay {
  private readonly widths = new Map<string, number>();
  private readonly elided = new Map<string, string>();

  constructor(private readonly ctx: CanvasRenderingContext2D) {}

  private measure(text: string): number {
    let width = this.widths.get(text);
    if (width === undefined) {
      if (this.widths.size > 20000) this.widths.clear();
      width = this.ctx.measureText(text).width;
      this.widths.set(text, width);
    }
    return width;
  }

  /** `elidedText(…, ElideRight, width)`. */
  private elide(text: string, width: number): string {
    if (this.measure(text) <= width) return text;
    const key = `${text}\u0000${Math.floor(width)}`;
    const cached = this.elided.get(key);
    if (cached !== undefined) return cached;
    let lo = 0;
    let hi = text.length;
    while (lo < hi) {
      const mid = (lo + hi + 1) >>> 1;
      if (this.measure(`${text.slice(0, mid)}…`) <= width) lo = mid;
      else hi = mid - 1;
    }
    const out = lo > 0 ? `${text.slice(0, lo)}…` : "";
    if (this.elided.size > 20000) this.elided.clear();
    this.elided.set(key, out);
    return out;
  }

  draw(frame: OverlayFrame): void {
    const { ctx } = this;
    const canvas = ctx.canvas;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.setTransform(frame.scale, 0, 0, frame.scale, 0, 0);
    ctx.font = FONT;
    ctx.textBaseline = "middle";
    const b0 = xToBeat(frame.view, -2);
    const b1 = xToBeat(frame.view, frame.width + 2);
    for (const row of frame.layout.visibleRows(frame.view.scrollY, frame.height)) {
      if (row.kind === "folder") continue;
      const track = trackOf(frame.model, row);
      if (!track) continue;
      for (const clip of frame.model.visibleClips(track, b0, b1)) {
        const rect = clipRect(frame.view, row, clip);
        this.clip(frame, track, clip, rect, frame.selected.has(clip.id), frame.ghosts.has(clip.id), track.track.mixer.mute);
      }
    }
    for (const preview of frame.previews) {
      const rect = clipRect(frame.view, preview.row, preview.placed.clip, preview.start, preview.end);
      this.clip(frame, preview.placed.track, preview.placed.clip, rect, true, false, false);
    }
    ctx.globalAlpha = 1;
  }

  private clip(
    frame: OverlayFrame,
    track: TrackClips,
    clip: ClipView,
    rect: Rect,
    selected: boolean,
    ghost: boolean,
    dimmed: boolean,
  ): void {
    const { ctx } = this;
    const colors = clipColors(clip, track.color, selected, ghost, dimmed);
    ctx.globalAlpha = colors.opacity;
    const showHeader = rect.height >= 26;
    if (showHeader && rect.width > 18) {
      const left = Math.max(rect.x, 0) + 5;
      const width = rect.x + rect.width - left - 4;
      if (width > 12) {
        let label = clip.name || (clip.type === "midi" ? "MIDI" : "Audio");
        if (clip.muted) label = `${label} (muted)`;
        const text = this.elide(label, width);
        if (text) {
          ctx.fillStyle = luminance(colors.header) > 0.55 ? DARK_TEXT : LIGHT_TEXT;
          ctx.fillText(text, left, rect.y + HEADER_H / 2);
        }
      }
    }
    if (clip.type === "audio" && frame.missing.has(clip.id)) {
      const top = showHeader ? rect.y + HEADER_H : rect.y;
      const height = showHeader ? rect.height - HEADER_H : rect.height;
      if (height >= 4) {
        ctx.save();
        ctx.beginPath();
        ctx.rect(rect.x, top, rect.width, height);
        ctx.clip();
        ctx.fillStyle = css(MISSING);
        ctx.textAlign = "center";
        ctx.fillText("media missing", rect.x + rect.width / 2, top + height / 2);
        ctx.restore();
      }
    }
  }
}

