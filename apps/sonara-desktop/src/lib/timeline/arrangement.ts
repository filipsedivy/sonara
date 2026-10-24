// The arrangement canvas: the WebGL2 renderer, the text overlay and the scene they draw,
// kept in step with the project view, the selection, the view, a drag and a file drop. A frame is
// drawn at the next animation frame after anything changed; scrolling and zooming redraw with new
// uniforms only, while the scene's instances are rebuilt only when the project, the selection,
// the track height, a drag's ghosts or the waveforms change.
//
// Without WebGL2 the canvas stays empty and `error` says why; hit testing and the gestures do
// not depend on drawing.

import { HiDpiCanvas } from "../hidpi";
import type { Selection } from "../state/selection";
import type { TimelineView } from "../state/timeline";
import { PeaksAtlas, type PeaksSource } from "./atlas";
import type { Drag } from "./gestures";
import type { TrackLayout } from "./layout";
import { clipEnd, type TimelineModel } from "./model";
import { TextOverlay } from "./overlay";
import { type DropMark, overRects, RectList, underRects } from "./rects";
import { addPlacement, buildScene, type Parts, type Placement, SceneLayer } from "./scene";
import { WebGLTimeline } from "./webgl";

/** What the arrangement shows. */
export interface ArrangementInputs {
  readonly model: TimelineModel | null;
  readonly layout: TrackLayout | null;
  readonly view: TimelineView;
  readonly selection: Selection;
  /** Grows whenever a waveform pyramid changed. */
  readonly peaksVersion: number;
}

/** One drawn frame's cost (the benchmark reads it). */
export interface DrawStats {
  /** performance.now() around the draw call. */
  readonly drawMs: number;
  /** The same plus a read-back that waits for the GPU, when `syncAfterDraw`. */
  readonly syncMs: number;
  /** performance.now() at the start of the animation frame. */
  readonly frameStart: number;
}

/** The scene without a project. */
const EMPTY = new SceneLayer();

function sameIds(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((id, i) => id === b[i]);
}

export class Arrangement {
  readonly error: string | null = null;
  readonly renderer: WebGLTimeline | null = null;
  private readonly surface: HiDpiCanvas;
  private readonly overlaySurface: HiDpiCanvas;
  private readonly text: TextOverlay | null;
  private readonly scene = new SceneLayer();
  private readonly preview = new SceneLayer();
  private readonly atlas: PeaksAtlas;
  private readonly under = new RectList();
  private readonly over = new RectList();
  private inputs: ArrangementInputs | null = null;
  private ghosts = new Set<string>();
  private previews: Placement[] = [];
  private drop: DropMark | null = null;
  private dirtyParts: Parts | null = "all";
  private frameRequested = false;
  private destroyed = false;
  /** Draws block on a GPU read-back (the benchmark's synced pass). */
  syncAfterDraw = false;
  /** No frame is drawn by itself: the benchmark draws them. */
  paused = false;
  /** Called after each drawn frame (the benchmark). */
  onDraw: ((stats: DrawStats) => void) | null = null;

  constructor(
    canvas: HTMLCanvasElement,
    overlay: HTMLCanvasElement,
    peaks: PeaksSource,
    private readonly drag: () => Drag | null,
  ) {
    this.atlas = new PeaksAtlas(peaks);
    this.surface = new HiDpiCanvas(canvas, () => this.requestDraw());
    this.overlaySurface = new HiDpiCanvas(overlay, () => this.requestDraw());
    const context = overlay.getContext("2d");
    this.text = context ? new TextOverlay(context) : null;
    try {
      this.renderer = new WebGLTimeline(canvas);
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
    }
  }

  /** The canvas's size in CSS pixels. */
  get width(): number {
    return this.surface.cssWidth;
  }

  get height(): number {
    return this.surface.cssHeight;
  }

  /** The project view, the selection or the view changed. */
  update(inputs: ArrangementInputs): void {
    const before = this.inputs;
    this.inputs = inputs;
    if (!before || before.model !== inputs.model || before.layout !== inputs.layout) {
      this.mark("all");
    } else if (before.peaksVersion !== inputs.peaksVersion) {
      this.mark("content");
    } else if (!sameIds(before.selection.clips, inputs.selection.clips)) {
      this.mark("clips");
    }
    if (before?.peaksVersion !== inputs.peaksVersion || before?.model !== inputs.model) {
      this.atlas.rebuild(inputs.model?.project.assets.map((asset) => asset.id) ?? []);
    }
    this.requestDraw();
  }

  /** The drag changed: the previews follow it, and the ghosts when it starts or ends. */
  dragChanged(): void {
    const drag = this.drag();
    const moving = drag && drag.kind !== "pending" && drag.kind !== "rubber" ? drag : null;
    const ghosts = new Set(moving ? moving.clips.keys() : []);
    if (ghosts.size !== this.ghosts.size || [...ghosts].some((id) => !this.ghosts.has(id))) {
      this.ghosts = ghosts;
      this.mark("all");
    }
    this.previews = moving ? this.placements(moving) : [];
    this.buildPreview();
    this.requestDraw();
  }

  /** The file drop's target, or none. */
  setDrop(drop: DropMark | null): void {
    this.drop = drop;
    this.requestDraw();
  }

  private mark(parts: Parts): void {
    this.dirtyParts =
      this.dirtyParts === null || this.dirtyParts === parts ? parts : "all";
  }

  /** `_paint_drag`: where the dragged clips would land. */
  private placements(drag: Drag): Placement[] {
    const inputs = this.inputs;
    if (!inputs?.layout) return [];
    const out: Placement[] = [];
    for (const placed of drag.clips.values()) {
      const clip = placed.clip;
      if (drag.kind === "move") {
        const target =
          drag.targetTrack !== null && drag.clips.size === 1 ? drag.targetTrack : placed.track.track.id;
        const row = inputs.layout.row(target);
        if (!row) continue;
        out.push({
          placed,
          row,
          start: clip.start + drag.delta,
          end: clipEnd(clip) + drag.delta,
          shift: drag.delta,
          selected: true,
          ghost: false,
          dimmed: false,
        });
      } else {
        const row = inputs.layout.row(placed.track.track.id);
        if (!row) continue;
        const left = drag.kind === "trim_left";
        out.push({
          placed,
          row,
          start: left ? drag.trimBeat : clip.start,
          end: left ? clipEnd(clip) : drag.trimBeat,
          shift: 0,
          selected: true,
          ghost: false,
          dimmed: false,
        });
      }
    }
    return out;
  }

  private buildPreview(): void {
    const model = this.inputs?.model;
    this.preview.clear();
    if (!model) return;
    for (const placement of this.previews) addPlacement(this.preview, model.project, model, this.atlas, placement);
  }

  requestDraw(): void {
    if (this.frameRequested || this.destroyed || this.paused) return;
    this.frameRequested = true;
    requestAnimationFrame((frameStart) => {
      this.frameRequested = false;
      if (!this.destroyed) this.draw(frameStart);
    });
  }

  /** Draws now (the benchmark drives frames itself). */
  draw(frameStart = performance.now()): DrawStats | null {
    const inputs = this.inputs;
    if (!inputs) return null;
    this.surface.update();
    this.overlaySurface.update();
    const t0 = performance.now();
    const { model, layout, view } = inputs;
    const width = this.surface.cssWidth;
    const height = this.surface.cssHeight;
    const scale = this.surface.scale;
    if (model && layout && this.dirtyParts !== null) {
      const selected = new Set(inputs.selection.clips);
      buildScene(this.scene, model, layout, this.atlas, selected, this.ghosts, this.dirtyParts);
      this.dirtyParts = null;
      if (this.previews.length > 0) this.buildPreview();
    }
    if (this.renderer) {
      if (model && layout) {
        underRects(this.under, model.project, layout, view, new Set(inputs.selection.tracks), width, height);
      } else {
        this.under.clear();
      }
      const drag = this.drag();
      overRects(this.over, view, drag?.kind === "rubber" ? drag.rect : null, this.drop, width, height);
      const empty = !model || !layout;
      this.renderer.draw({
        view: { scrollX: view.scrollX, pixelsPerBeat: view.pixelsPerBeat, scrollY: view.scrollY, scale },
        under: this.under.data,
        underCount: this.under.count,
        scene: empty ? EMPTY : this.scene,
        preview: this.previews.length > 0 ? this.preview : null,
        over: this.over.data,
        overCount: this.over.count,
        atlas: this.atlas,
      });
    }
    if (this.text && model && layout) {
      this.text.draw({
        model,
        layout,
        view,
        scale: this.overlaySurface.scale,
        width,
        height,
        selected: new Set(inputs.selection.clips),
        ghosts: this.ghosts,
        previews: this.previews,
        missing: new Set(this.scene.missing.map((m) => m.clipId)),
      });
    }
    const drawMs = performance.now() - t0;
    let syncMs = drawMs;
    if (this.syncAfterDraw && this.renderer) {
      this.renderer.sync();
      syncMs = performance.now() - t0;
    }
    const stats = { drawMs, syncMs, frameStart };
    this.onDraw?.(stats);
    return stats;
  }

  destroy(): void {
    this.destroyed = true;
    this.surface.destroy();
    this.overlaySurface.destroy();
  }
}
