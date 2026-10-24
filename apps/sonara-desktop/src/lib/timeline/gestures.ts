// The arrangement canvas's interactions (`ArrangementCanvas` in sonara/ui/timeline/canvas.py):
// clicks select, drags move, retarget or trim a clip or draw a rubber band and
// commit one edit on release, a double-click opens or creates a MIDI clip,
// the wheel scrolls and zooms, the keys delete, duplicate, split, nudge and select,
// the context menu offers the clip and lane actions, and a file drop picks its
// track and beat. The prototype's arithmetic is kept line by line; the page draws the
// preview from `drag`.
//
// Edits go through `CanvasActions`: a drag's edit is committed on release and sealed (one
// gesture, one undo step, `commitOnRelease`); a key's edit runs alone, as the prototype's
// `bridge.execute` did.

import { type TimelineView, xToBeat } from "../state/timeline";
import type { Selection } from "../state/selection";
import type { SnapMode } from "../state/snap";
import { barLength, barToBeat, beatToBar, EPSILON } from "../time/signatures";
import type { MenuItem } from "../shell/menu";
import type { ClipView, Edit, EditOutcome } from "../types";
import { DRAG_THRESHOLD, hitTest, type Rect, trackOf } from "./hit";
import type { Row, TrackLayout } from "./layout";
import { clipEnd, isCompatible, type PlacedClip, type TimelineModel } from "./model";

/** The modifier keys of a pointer or key event, as the prototype reads them. */
export interface Mods {
  /** ⇧ */
  readonly shift: boolean;
  /** ⌘ (Qt's Control on macOS) */
  readonly command: boolean;
  /** ⌥: bypasses snapping */
  readonly alt: boolean;
}

export const NO_MODS: Mods = { shift: false, command: false, alt: false };

/** What the gestures read: the timeline as it is drawn now. */
export interface CanvasState {
  readonly model: TimelineModel;
  readonly layout: TrackLayout;
  readonly view: TimelineView;
  readonly selection: Selection;
  /** The canvas's height in pixels (the rubber band looks at the visible rows). */
  readonly height: number;
  /** The canvas's width in pixels (the keys zoom around its middle). */
  readonly width: number;
  readonly playheadBeat: number;
  /** `SnapModel.snap` with the project's signatures. */
  snap(beat: number, mode: SnapMode, bypass: boolean): number;
  /** `SnapModel.grid_beats` at `beat`. */
  gridBeats(beat: number): number;
}

/** What the gestures do. */
export interface CanvasActions {
  selectClips(clipIds: string[], additive?: boolean): void;
  selectTrack(trackId: string | null, additive?: boolean): void;
  setCurrentClip(clipId: string | null): void;
  /** A drag's edit: committed once, then the undo step is sealed. */
  commit(edit: Edit): Promise<unknown>;
  /** A key's or a menu's edit; the outcome, or null when it was refused (and reported). */
  run(edit: Edit): Promise<EditOutcome | null>;
  /** MW-27: the piano roll shows the clip. */
  openClipEditor(clipId: string): void;
  /** MW-27: the Devices tab shows the track. */
  showDevices(trackId: string): void;
  /** TL-08 "Import Audio Here…". */
  importAudio(trackId: string, beat: number): void;
  /** The view changed (zoom from the keys). */
  zoom(factor: number, anchorX: number): void;
}

/** A drag in progress (`Drag`). */
export interface Drag {
  kind: "pending" | "move" | "trim_left" | "trim_right" | "rubber";
  readonly pressX: number;
  readonly pressY: number;
  readonly pressBeat: number;
  readonly anchor: PlacedClip | null;
  readonly clips: ReadonlyMap<string, PlacedClip>;
  delta: number;
  targetTrack: string | null;
  trimBeat: number;
  readonly baseSelection: readonly string[];
  rect: Rect | null;
  readonly pendingKind: "move" | "trim_left" | "trim_right";
}

/** A context menu entry, or a separator. */
export type MenuEntry = MenuItem;

/** The cursor over the canvas. */
export type Cursor = "default" | "ew-resize";

function additive(mods: Mods): boolean {
  return mods.shift || mods.command;
}

/** The bar that starts at or before `beat`, its start and its length. */
export function barAt(state: CanvasState, beat: number): { start: number; length: number } {
  const signatures = state.model.project.time_signatures;
  const bar = Math.trunc(beatToBar(signatures, Math.max(0, beat)) + 1e-9);
  return { start: barToBeat(signatures, bar), length: barLength(signatures, bar) };
}

export class CanvasGestures {
  /** The drag in progress, which the page draws. */
  drag: Drag | null = null;
  /** The beat under the pointer, for a split outside the clips (`_mouse_beat`). */
  mouseBeat = 0;

  constructor(
    private readonly state: () => CanvasState | null,
    private readonly actions: CanvasActions,
  ) {}

  // -- the pointer -------------------------------------------------------------------------------

  /** `mousePressEvent` with the left button. */
  press(x: number, y: number, mods: Mods): void {
    const state = this.state();
    if (!state) return;
    const beat = xToBeat(state.view, x);
    const hit = hitTest(state.model, state.layout, state.view, x, y);
    if (hit) {
      const { selection } = state;
      if (additive(mods)) this.actions.selectClips([hit.clip.id], true);
      else if (!selection.clips.includes(hit.clip.id)) this.actions.selectClips([hit.clip.id]);
      if (hit.track.track.id !== this.state()?.selection.currentTrack) {
        this.actions.selectTrack(hit.track.track.id);
      }
      if (hit.clip.type === "midi") this.actions.setCurrentClip(hit.clip.id);
      const pendingKind =
        hit.region === "left" ? "trim_left" : hit.region === "right" ? "trim_right" : "move";
      const clips = new Map<string, PlacedClip>();
      if (pendingKind === "move") {
        const now = this.state();
        for (const found of state.model.selected(now?.selection.clips ?? [])) {
          clips.set(found.clip.id, found);
        }
      } else {
        clips.set(hit.clip.id, { track: hit.track, clip: hit.clip });
      }
      this.drag = {
        kind: "pending",
        pressX: x,
        pressY: y,
        pressBeat: beat,
        anchor: { track: hit.track, clip: hit.clip },
        clips,
        delta: 0,
        targetTrack: null,
        trimBeat: 0,
        baseSelection: [],
        rect: null,
        pendingKind,
      };
      return;
    }
    const row = state.layout.rowAt(y, state.view.scrollY);
    if (!additive(mods)) this.actions.selectClips([]);
    if (row) this.actions.selectTrack(row.trackId, mods.command);
    this.drag = {
      kind: "rubber",
      pressX: x,
      pressY: y,
      pressBeat: beat,
      anchor: null,
      clips: new Map(),
      delta: 0,
      targetTrack: null,
      trimBeat: 0,
      baseSelection: [...(this.state()?.selection.clips ?? [])],
      rect: null,
      pendingKind: "move",
    };
  }

  /** `mouseMoveEvent`: the drag's preview, or the cursor over the clips' edges. */
  move(x: number, y: number, mods: Mods): Cursor {
    const state = this.state();
    if (!state) return "default";
    this.mouseBeat = xToBeat(state.view, x);
    const drag = this.drag;
    if (!drag) {
      const hit = hitTest(state.model, state.layout, state.view, x, y);
      return hit && hit.region !== "body" ? "ew-resize" : "default";
    }
    if (drag.kind === "pending") {
      if (Math.abs(x - drag.pressX) + Math.abs(y - drag.pressY) < DRAG_THRESHOLD) return "default";
      drag.kind = drag.pendingKind;
    }
    const beat = xToBeat(state.view, x);
    const anchor = drag.anchor;
    if (drag.kind === "move" && anchor) {
      const raw = anchor.clip.start + (beat - drag.pressBeat);
      const newStart = state.snap(raw, "nearest", mods.alt);
      let earliest = Infinity;
      for (const placed of drag.clips.values()) earliest = Math.min(earliest, placed.clip.start);
      if (!Number.isFinite(earliest)) earliest = 0;
      drag.delta = Math.max(newStart - anchor.clip.start, -earliest);
      drag.targetTrack = null;
      const row = state.layout.rowAt(y, state.view.scrollY);
      if (row && drag.clips.size === 1 && row.trackId !== anchor.track.track.id) {
        const target = trackOf(state.model, row);
        if (target && isCompatible(target.track, anchor.clip)) drag.targetTrack = target.track.id;
      }
    } else if ((drag.kind === "trim_left" || drag.kind === "trim_right") && anchor) {
      // The grabbed edge moves by the pointer's delta (keeping the grab offset), then snaps.
      const left = drag.kind === "trim_left";
      const edge = left ? anchor.clip.start : clipEnd(anchor.clip);
      const b = state.snap(edge + (beat - drag.pressBeat), "nearest", mods.alt);
      const minLength = Math.max(1 / 64, 3 / state.view.pixelsPerBeat);
      drag.trimBeat = left
        ? Math.min(Math.max(0, b), clipEnd(anchor.clip) - minLength)
        : Math.max(b, anchor.clip.start + minLength);
    } else if (drag.kind === "rubber") {
      drag.rect = {
        x: Math.min(drag.pressX, x),
        y: Math.min(drag.pressY, y),
        width: Math.abs(x - drag.pressX),
        height: Math.abs(y - drag.pressY),
      };
      this.rubberSelect(state, drag);
    }
    return drag.kind === "trim_left" || drag.kind === "trim_right" ? "ew-resize" : "default";
  }

  /** `_rubber_select`: the clips the band touches, added to the selection it started with. */
  private rubberSelect(state: CanvasState, drag: Drag): void {
    const rect = drag.rect;
    if (!rect) return;
    const b0 = xToBeat(state.view, rect.x);
    const b1 = xToBeat(state.view, rect.x + rect.width);
    const hits: string[] = [];
    for (const row of state.layout.visibleRows(state.view.scrollY, state.height)) {
      const top = row.top - state.view.scrollY;
      if (top + row.height < rect.y || top > rect.y + rect.height) continue;
      const track = trackOf(state.model, row);
      if (!track) continue;
      for (const clip of state.model.visibleClips(track, b0, b1)) hits.push(clip.id);
    }
    const merged = [...new Set([...drag.baseSelection, ...hits])];
    const current = state.selection.clips;
    const same = merged.length === current.length && merged.every((id, i) => id === current[i]);
    if (!same) this.actions.selectClips(merged);
  }

  /** `mouseReleaseEvent`: the drag's one edit, committed and sealed. */
  async release(): Promise<void> {
    const drag = this.drag;
    this.drag = null;
    const edit = drag ? this.releaseEdit(drag) : null;
    if (edit) await this.actions.commit(edit);
  }

  /** The edit a finished drag commits, or null. */
  releaseEdit(drag: Drag): Edit | null {
    const anchor = drag.anchor;
    if (!anchor) return null;
    const clip = anchor.clip;
    if (drag.kind === "move") {
      if (drag.clips.size === 1) {
        if (Math.abs(drag.delta) > EPSILON || drag.targetTrack !== null) {
          return drag.targetTrack === null
            ? { op: "move_clip", clip_id: clip.id, start: clip.start + drag.delta }
            : {
                op: "move_clip",
                clip_id: clip.id,
                start: clip.start + drag.delta,
                track_id: drag.targetTrack,
              };
        }
      } else if (Math.abs(drag.delta) > EPSILON) {
        return { op: "move_clips", clip_ids: [...drag.clips.keys()], delta_beats: drag.delta };
      }
    } else if (drag.kind === "trim_left" && Math.abs(drag.trimBeat - clip.start) > EPSILON) {
      return { op: "trim_clip", clip_id: clip.id, start: drag.trimBeat };
    } else if (drag.kind === "trim_right" && Math.abs(drag.trimBeat - clipEnd(clip)) > EPSILON) {
      return { op: "trim_clip", clip_id: clip.id, end: drag.trimBeat };
    }
    return null;
  }

  /** Esc during a drag: the drag ends without an edit. */
  cancel(): void {
    this.drag = null;
  }

  /** `mouseDoubleClickEvent`. */
  async doubleClick(x: number, y: number): Promise<void> {
    this.drag = null;
    const state = this.state();
    if (!state) return;
    const hit = hitTest(state.model, state.layout, state.view, x, y);
    if (hit) {
      if (hit.clip.type === "midi") {
        this.actions.setCurrentClip(hit.clip.id);
        this.actions.openClipEditor(hit.clip.id);
      }
      return;
    }
    const row = state.layout.rowAt(y, state.view.scrollY);
    const track = row ? trackOf(state.model, row) : undefined;
    if (!track || track.track.kind !== "instrument") return;
    const bar = barAt(state, xToBeat(state.view, x));
    const outcome = await this.actions.run({
      op: "create_midi_clip",
      track_id: track.track.id,
      start: bar.start,
      length: bar.length,
      name: track.track.name,
    });
    const created = outcome ? createdClips(outcome) : [];
    if (created.length > 0) {
      this.actions.selectClips([created[0]]);
      this.actions.setCurrentClip(created[0]);
      this.actions.openClipEditor(created[0]);
    }
  }

  // -- the keys --------------------------------------------------------------------------

  /** `keyPressEvent`; whether the key was the canvas's. */
  key(key: string, mods: Mods): boolean {
    const state = this.state();
    const lower = key.length === 1 ? key.toLowerCase() : key;
    if (key === "Delete" || key === "Backspace") void this.deleteSelected();
    else if (lower === "d" && mods.command) void this.duplicateSelected();
    else if ((lower === "s" && !mods.command) || (lower === "e" && mods.command)) {
      void this.splitSelected();
    } else if (key === "ArrowLeft") void this.nudgeSelected(-1);
    else if (key === "ArrowRight") void this.nudgeSelected(1);
    else if (lower === "a" && mods.command) {
      if (state) this.actions.selectClips(state.model.allClipIds());
    } else if (key === "Escape") {
      this.drag = null;
      this.actions.selectClips([]);
    } else if (key === "+" || key === "=") this.actions.zoom(1.25, (state?.width ?? 0) / 2);
    else if (key === "-") this.actions.zoom(0.8, (state?.width ?? 0) / 2);
    else return false;
    return true;
  }

  private selectedClips(): PlacedClip[] {
    const state = this.state();
    return state ? state.model.selected(state.selection.clips) : [];
  }

  /** `delete_selected`. */
  async deleteSelected(): Promise<void> {
    const ids = this.selectedClips().map((placed) => placed.clip.id);
    if (ids.length === 0) return;
    await this.actions.run({ op: "delete_clips", clip_ids: ids });
    this.actions.selectClips([]);
  }

  /** `duplicate_selected`: one "Duplicate Clips" step after the span; the copies are selected. */
  async duplicateSelected(): Promise<void> {
    const ids = this.selectedClips().map((placed) => placed.clip.id);
    if (ids.length === 0) return;
    const outcome = await this.actions.run({ op: "duplicate_clips", clip_ids: ids });
    if (outcome) this.actions.selectClips(createdClips(outcome));
  }

  /**
   * `split_selected`: at `at`, else at the playhead, else (when the playhead is outside them)
   * at the snapped pointer; one "Split Clips" step.
   */
  async splitSelected(at?: number): Promise<void> {
    const state = this.state();
    if (!state) return;
    const clips = this.selectedClips();
    const inside = (beat: number) =>
      clips
        .filter((p) => p.clip.start + EPSILON < beat && beat < clipEnd(p.clip) - EPSILON)
        .map((p) => p.clip.id);
    let beat = at ?? state.playheadBeat;
    let targets = inside(beat);
    if (targets.length === 0 && at === undefined) {
      beat = state.snap(this.mouseBeat, "nearest", false);
      targets = inside(beat);
    }
    if (targets.length > 0) await this.actions.run({ op: "split_clips", clip_ids: targets, at: beat });
  }

  /** `nudge_selected`: one grid step at the earliest clip, never before 0. */
  async nudgeSelected(direction: number): Promise<void> {
    const state = this.state();
    const clips = this.selectedClips();
    if (!state || clips.length === 0) return;
    const earliest = Math.min(...clips.map((p) => p.clip.start));
    const delta = Math.max(direction * state.gridBeats(earliest), -earliest);
    if (Math.abs(delta) > EPSILON) {
      await this.actions.run({
        op: "move_clips",
        clip_ids: clips.map((p) => p.clip.id),
        delta_beats: delta,
      });
    }
  }

  // -- the context menu ------------------------------------------------------------------

  /** `contextMenuEvent`: the entries at (`x`, `y`); a clip under the pointer is selected. */
  contextMenu(x: number, y: number): MenuEntry[] {
    const state = this.state();
    if (!state) return [];
    const hit = hitTest(state.model, state.layout, state.view, x, y);
    const beat = state.snap(xToBeat(state.view, x), "nearest", false);
    const entries: MenuEntry[] = [];
    if (hit) {
      if (!state.selection.clips.includes(hit.clip.id)) this.actions.selectClips([hit.clip.id]);
      const clip: ClipView = hit.clip;
      if (clip.type === "midi") {
        entries.push({ label: "Open in Piano Roll", run: () => this.actions.openClipEditor(clip.id) });
      }
      entries.push({ label: "Split Here", run: () => void this.splitSelected(beat) });
      entries.push({ label: "Duplicate", run: () => void this.duplicateSelected() });
      entries.push({
        label: clip.muted ? "Unmute" : "Mute",
        run: () => void this.actions.run({ op: "set_clip_properties", clip_id: clip.id, muted: !clip.muted }),
      });
      entries.push("separator");
      entries.push({ label: "Delete", run: () => void this.deleteSelected() });
      return entries;
    }
    const row = state.layout.rowAt(y, state.view.scrollY);
    const track = row ? trackOf(state.model, row)?.track : undefined;
    if (!track) return entries;
    if (track.kind === "instrument") {
      const bar = barAt(state, beat);
      entries.push({
        label: "Add MIDI Clip",
        run: () =>
          void this.actions.run({
            op: "create_midi_clip",
            track_id: track.id,
            start: bar.start,
            length: bar.length,
            name: track.name,
          }),
      });
    }
    if (track.kind === "audio") {
      entries.push({ label: "Import Audio Here…", run: () => this.actions.importAudio(track.id, beat) });
    }
    entries.push({ label: "Show Devices", run: () => this.actions.showDevices(track.id) });
    return entries;
  }

  // -- file drops ------------------------------------------------------------------------

  /**
   * `dragMoveEvent` and `dropEvent`: the audio track under the pointer (none over another
   * kind of track or below the rows) and the floor-snapped beat (⌥ bypasses).
   */
  dropTarget(x: number, y: number, mods: Mods): { row: Row | null; beat: number } | null {
    const state = this.state();
    if (!state) return null;
    const under = state.layout.rowAt(y, state.view.scrollY);
    const row = under && under.kind === "audio" ? under : null;
    return { row, beat: state.snap(xToBeat(state.view, x), "floor", mods.alt) };
  }
}

/** The clips an outcome created, in order. */
export function createdClips(outcome: EditOutcome): string[] {
  return outcome.changes.created.filter((object) => object.kind === "clip").map((object) => object.id);
}

/** The wheel's meaning: a track height step, a zoom around the pointer, or a scroll. */
export type WheelAction =
  | { readonly kind: "height"; readonly direction: number }
  | { readonly kind: "zoom"; readonly factor: number }
  | { readonly kind: "scroll"; readonly dx: number; readonly dy: number };

/**
 * `wheelEvent` for a browser wheel event (pixel deltas, positive down and right): ⌘⌥ steps the
 * track height, ⌘ (or a pinch) zooms by 1.15 per 60 pixels (Qt's pixel path), ⇧ scrolls
 * sideways, and otherwise the deltas scroll.
 */
export function wheelAction(event: {
  deltaX: number;
  deltaY: number;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}): WheelAction {
  const command = event.metaKey || event.ctrlKey;
  if (command && event.altKey) return { kind: "height", direction: event.deltaY < 0 ? 1 : -1 };
  if (command) return { kind: "zoom", factor: 1.15 ** (-event.deltaY / 60) };
  if (event.shiftKey) return { kind: "scroll", dx: event.deltaX || event.deltaY, dy: 0 };
  return { kind: "scroll", dx: event.deltaX, dy: event.deltaY };
}
