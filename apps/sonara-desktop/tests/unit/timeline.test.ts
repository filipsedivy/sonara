import { expect, test } from "@playwright/test";
import * as selection from "../../src/lib/state/selection";
import { DEFAULT_SNAP, gridBeats, snapBeat, type Snap } from "../../src/lib/state/snap";
import {
  DEFAULT_TIMELINE,
  fitView,
  scrollRange,
  setZoom,
  stepHeight,
  type TimelineView,
  xToBeat,
} from "../../src/lib/state/timeline";
import type { Edit, EditOutcome, OpenProject, TrackView } from "../../src/lib/types";
import { CanvasGestures, type CanvasState, type Mods, NO_MODS, wheelAction } from "../../src/lib/timeline/gestures";
import { gridLines, gridSteps, rulerMarks } from "../../src/lib/timeline/grid";
import { hitTest } from "../../src/lib/timeline/hit";
import { TrackLayout } from "../../src/lib/timeline/layout";
import { TimelineModel } from "../../src/lib/timeline/model";
import { BAND, RulerGestures } from "../../src/lib/timeline/ruler";
import { midiClip, project, track } from "../support/project";

// The timeline's logic against the prototype's tests (tests/gui/test_timeline.py): rows, hit
// testing, the canvas's gestures and keys, the ruler, zoom and Fit, at the default zoom (24 px
// per beat) and track height (56 px). The page tests (tests/page/timeline.spec.ts) drive the same
// through the canvas's pointer and key events.

const PPB = 24;
const ROW = 56;

/** A timeline over `open`, with recorded edits and a live selection. */
class Harness {
  model: TimelineModel;
  layout: TrackLayout;
  view: TimelineView = DEFAULT_TIMELINE;
  selection: selection.Selection = selection.EMPTY_SELECTION;
  snap: Snap = DEFAULT_SNAP;
  playhead = 0;
  committed: Edit[] = [];
  ran: Edit[] = [];
  opened: string[] = [];
  outcome: Partial<EditOutcome> | null = null;
  readonly gestures: CanvasGestures;

  constructor(readonly open: OpenProject) {
    this.model = new TimelineModel(open);
    this.layout = new TrackLayout(open.tracks, ROW);
    this.gestures = new CanvasGestures(() => this.state(), {
      selectClips: (ids, additive) => (this.selection = selection.selectClips(this.selection, ids, additive)),
      selectTrack: (id, additive) => (this.selection = selection.selectTrack(this.selection, id, additive)),
      setCurrentClip: (id) => (this.selection = selection.setCurrentClip(this.selection, id)),
      commit: async (edit) => {
        this.committed.push(edit);
      },
      run: async (edit) => {
        this.ran.push(edit);
        return (this.outcome as EditOutcome) ?? null;
      },
      openClipEditor: (id) => this.opened.push(id),
      showDevices: () => {},
      importAudio: () => {},
      zoom: () => {},
    });
  }

  state(): CanvasState {
    return {
      model: this.model,
      layout: this.layout,
      view: this.view,
      selection: this.selection,
      width: 1200,
      height: 520,
      playheadBeat: this.playhead,
      snap: (beat, mode, bypass) => snapBeat(this.snap, beat, this.open.time_signatures, mode, bypass),
      gridBeats: (beat) => gridBeats(this.snap, this.open.time_signatures, beat),
    };
  }

  /** `drag`: press, six moves, release. */
  async drag(x0: number, y0: number, x1: number, y1: number, mods: Mods = NO_MODS) {
    this.gestures.press(x0, y0, mods);
    for (let i = 1; i <= 6; i++) {
      this.gestures.move(x0 + ((x1 - x0) * i) / 6, y0 + ((y1 - y0) * i) / 6, mods);
    }
    await this.gestures.release();
  }
}

const command: Mods = { shift: false, command: true, alt: false };

function instruments(...names: string[]): TrackView[] {
  return names.map((name, i) => track(`trk_${i}`, name));
}

test("rows follow the tracks, folders shorter and indented; hit testing finds clips", () => {
  const tracks = [track("a", "Lead"), track("b", "Bass", "instrument", [midiClip("clp_b", 4, 8)])];
  const h = new Harness(project(tracks));
  expect(h.layout.rows.map((r) => r.trackId)).toEqual(["a", "b"]);
  const hit = hitTest(h.model, h.layout, h.view, 8 * PPB, ROW + ROW / 2);
  expect([hit?.clip.id, hit?.region]).toEqual(["clp_b", "body"]);
  expect(hitTest(h.model, h.layout, h.view, PPB, ROW / 2)).toBeNull();
  // `test_folder_children_are_indented`
  const folder = { ...track("f", "Group", "folder"), depth: 0 };
  const child = { ...track("c", "Child", "audio"), parent_id: "f", depth: 1 };
  const layout = new TrackLayout([folder, child], ROW);
  expect(layout.rows.map((r) => [r.depth, r.height])).toEqual([
    [0, ROW - 14],
    [1, ROW],
  ]);
  expect(layout.rowAt(ROW - 15, 0)?.trackId).toBe("f");
  expect(layout.rowAt(ROW - 14, 0)?.trackId).toBe("c");
  expect(layout.rowAt(2 * ROW, 0)).toBeNull();
  expect(layout.visibleRows(10, 20).map((r) => r.trackId)).toEqual(["f"]);
});

test("a drag moves the clip by the snapped delta in one committed edit", async () => {
  const h = new Harness(project([track("t", "Synth", "instrument", [midiClip("clp", 0, 4)])]));
  await h.drag(2 * PPB, ROW / 2, 2 * PPB + 2.6 * PPB, ROW / 2);
  // 0 + 2.6 beats snaps to the 1/16 grid: 2.5
  expect(h.committed).toEqual([{ op: "move_clip", clip_id: "clp", start: 2.5 }]);
  expect(h.selection.clips).toEqual(["clp"]);
  expect(h.selection.currentClip).toBe("clp");
  expect(h.selection.currentTrack).toBe("t");
});

test("⌥ bypasses the snap while dragging", async () => {
  const h = new Harness(project([track("t", "Synth", "instrument", [midiClip("clp", 0, 4)])]));
  await h.drag(2 * PPB, ROW / 2, 2 * PPB + 2.6 * PPB, ROW / 2, { shift: false, command: false, alt: true });
  const edit = h.committed[0] as Extract<Edit, { op: "move_clip" }>;
  expect(edit.start).toBeCloseTo(2.6, 6);
});

test("a vertical drag moves a clip only to a compatible track", async () => {
  const tracks = [track("a", "A", "instrument", [midiClip("clp", 0, 4)]), track("au", "Audio", "audio"), track("b", "B")];
  const h = new Harness(project(tracks));
  await h.drag(2 * PPB, ROW / 2, 2 * PPB, ROW * 1.5);
  expect(h.committed).toEqual([]);
  await h.drag(2 * PPB, ROW / 2, 2 * PPB, ROW * 2.5);
  expect(h.committed).toEqual([{ op: "move_clip", clip_id: "clp", start: 0, track_id: "b" }]);
});

test("the edges trim: the right end, and the left start with the content in place", async () => {
  const h = new Harness(project([track("t", "Synth", "instrument", [midiClip("clp", 0, 4)])]));
  const right = hitTest(h.model, h.layout, h.view, 4 * PPB - 3, ROW / 2);
  expect(right?.region).toBe("right");
  await h.drag(4 * PPB - 3, ROW / 2, 5 * PPB - 3, ROW / 2);
  await h.drag(3, ROW / 2, 3 + PPB, ROW / 2);
  expect(h.committed).toEqual([
    { op: "trim_clip", clip_id: "clp", end: 5 },
    { op: "trim_clip", clip_id: "clp", start: 1 },
  ]);
  // A clip narrower than 18 px has no trim zones.
  const narrow = new Harness(project([track("t", "Synth", "instrument", [midiClip("n", 0, 0.5)])]));
  expect(hitTest(narrow.model, narrow.layout, narrow.view, 2, ROW / 2)?.region).toBe("body");
});

test("the keys duplicate, split at the playhead and delete the selection", async () => {
  const h = new Harness(project([track("t", "Synth", "instrument", [midiClip("clp", 0, 4)])]));
  h.selection = selection.selectClips(h.selection, ["clp"]);
  h.outcome = {
    changes: {
      created: [{ kind: "clip", id: "clp_copy" }],
      updated: [],
      deleted: [],
      flags: ["clips"],
      structure_changed: false,
    },
  };
  expect(h.gestures.key("d", command)).toBe(true);
  await Promise.resolve();
  await Promise.resolve();
  expect(h.ran).toEqual([{ op: "duplicate_clips", clip_ids: ["clp"] }]);
  expect(h.selection.clips).toEqual(["clp_copy"]);
  h.selection = selection.selectClips(h.selection, ["clp"]);
  h.playhead = 2;
  h.gestures.key("s", NO_MODS);
  h.gestures.key("Delete", NO_MODS);
  await Promise.resolve();
  expect(h.ran.slice(1)).toEqual([
    { op: "split_clips", clip_ids: ["clp"], at: 2 },
    { op: "delete_clips", clip_ids: ["clp"] },
  ]);
  // Outside every selected clip, S splits at the snapped pointer instead.
  h.ran = [];
  h.selection = selection.selectClips(h.selection, ["clp"]);
  h.playhead = 9;
  h.gestures.move(1.1 * PPB, ROW / 2, NO_MODS);
  h.gestures.key("e", command);
  await Promise.resolve();
  expect(h.ran).toEqual([{ op: "split_clips", clip_ids: ["clp"], at: 1 }]);
});

test("arrow keys nudge by one grid step, never before 0", async () => {
  const h = new Harness(project([track("t", "Synth", "instrument", [midiClip("clp", 1, 4)])]));
  h.selection = selection.selectClips(h.selection, ["clp"]);
  h.gestures.key("ArrowRight", NO_MODS);
  h.gestures.key("ArrowLeft", NO_MODS);
  await Promise.resolve();
  expect(h.ran).toEqual([
    { op: "move_clips", clip_ids: ["clp"], delta_beats: 0.25 },
    { op: "move_clips", clip_ids: ["clp"], delta_beats: -0.25 },
  ]);
  const start = new Harness(project([track("t", "Synth", "instrument", [midiClip("z", 0.1, 4)])]));
  start.selection = selection.selectClips(start.selection, ["z"]);
  start.gestures.key("ArrowLeft", NO_MODS);
  await Promise.resolve();
  expect(start.ran).toEqual([{ op: "move_clips", clip_ids: ["z"], delta_beats: -0.1 }]);
});

test("⌘A selects every clip and Esc clears the selection", () => {
  const tracks = [track("a", "A", "instrument", [midiClip("c1", 0, 2)]), track("b", "B", "instrument", [midiClip("c2", 4, 2)])];
  const h = new Harness(project(tracks));
  h.gestures.key("a", command);
  expect(h.selection.clips).toEqual(["c1", "c2"]);
  h.gestures.key("Escape", NO_MODS);
  expect(h.selection.clips).toEqual([]);
});

test("a rubber band selects the clips it touches across tracks", async () => {
  const tracks = [
    track("a", "A", "instrument", [midiClip("c1", 2, 2)]),
    track("b", "B", "instrument", [midiClip("c2", 3, 2), midiClip("c3", 20, 2)]),
  ];
  const h = new Harness(project(tracks));
  await h.drag(1 * PPB, 5, 6 * PPB, ROW * 2 - 5);
  expect(new Set(h.selection.clips)).toEqual(new Set(["c1", "c2"]));
  expect(h.committed).toEqual([]);
  // A click on empty space clears the clips and selects the row's track.
  h.gestures.press(30 * PPB, ROW / 2, NO_MODS);
  await h.gestures.release();
  expect([h.selection.clips, h.selection.tracks]).toEqual([[], ["a"]]);
});

test("a double-click opens a MIDI clip, or creates a one-bar clip on an empty lane", async () => {
  const h = new Harness(project([track("t", "Synth", "instrument", [midiClip("clp", 0, 4)])]));
  await h.gestures.doubleClick(2 * PPB, ROW / 2);
  expect(h.opened).toEqual(["clp"]);
  expect(h.selection.currentClip).toBe("clp");
  const empty = new Harness(project(instruments("Keys")));
  empty.outcome = {
    changes: { created: [{ kind: "clip", id: "new" }], updated: [], deleted: [], flags: [], structure_changed: false },
  };
  await empty.gestures.doubleClick(5.5 * PPB, ROW / 2);
  expect(empty.ran).toEqual([{ op: "create_midi_clip", track_id: "trk_0", start: 4, length: 4, name: "Keys" }]);
  expect(empty.opened).toEqual(["new"]);
  expect(empty.selection.clips).toEqual(["new"]);
  // Not on an audio lane.
  const audio = new Harness(project([track("au", "Audio", "audio")]));
  await audio.gestures.doubleClick(5.5 * PPB, ROW / 2);
  expect(audio.ran).toEqual([]);
});

test("the context menu offers the clip's and the lane's actions", async () => {
  const tracks = [
    track("t", "Synth", "instrument", [midiClip("clp", 0, 4, { muted: true })]),
    track("au", "Audio", "audio"),
  ];
  const h = new Harness(project(tracks));
  const labels = (entries: ReturnType<CanvasGestures["contextMenu"]>) =>
    entries.map((entry) => (entry === "separator" ? "—" : entry.label));
  const onClip = h.gestures.contextMenu(2 * PPB, ROW / 2);
  expect(labels(onClip)).toEqual(["Open in Piano Roll", "Split Here", "Duplicate", "Unmute", "—", "Delete"]);
  expect(h.selection.clips).toEqual(["clp"]);
  const unmute = onClip[3];
  if (unmute !== "separator" && "run" in unmute) unmute.run();
  await Promise.resolve();
  expect(h.ran).toEqual([{ op: "set_clip_properties", clip_id: "clp", muted: false }]);
  expect(labels(h.gestures.contextMenu(10 * PPB, ROW / 2))).toEqual(["Add MIDI Clip", "Show Devices"]);
  expect(labels(h.gestures.contextMenu(10 * PPB, ROW * 1.5))).toEqual(["Import Audio Here…", "Show Devices"]);
  expect(h.gestures.contextMenu(10 * PPB, ROW * 3)).toEqual([]);
});

test("a file drop targets the audio track under the pointer at the floor-snapped beat", () => {
  const h = new Harness(project([track("au", "Audio", "audio"), track("t", "Synth")]));
  const onAudio = h.gestures.dropTarget(4.2 * PPB, ROW / 2, NO_MODS);
  expect([onAudio?.row?.trackId, onAudio?.beat]).toEqual(["au", 4]);
  const onSynth = h.gestures.dropTarget(4.2 * PPB, ROW * 1.5, NO_MODS);
  expect([onSynth?.row, onSynth?.beat]).toEqual([null, 4]);
});

test("the ruler seeks unsnapped, draws a new loop with ⇧ and drags its handles", async () => {
  const open = project([]);
  const seeks: number[] = [];
  const committed: Edit[] = [];
  let current = open;
  const ruler = new RulerGestures(
    () => ({
      project: current,
      view: DEFAULT_TIMELINE,
      snap: (beat, mode, bypass) => snapBeat(DEFAULT_SNAP, beat, current.time_signatures, mode, bypass),
    }),
    {
      seek: (beat) => seeks.push(beat),
      commit: async (edit) => {
        committed.push(edit);
      },
      run: async () => {},
    },
  );
  ruler.press(10 * PPB, 36, NO_MODS);
  await ruler.release();
  expect(seeks[0]).toBeCloseTo(10, 6);
  const shift = { shift: true, command: false, alt: false };
  ruler.press(8 * PPB + 2, 36, shift);
  for (let i = 1; i <= 6; i++) ruler.move(8 * PPB + 2 + (8 * PPB * i) / 6, 36, shift);
  await ruler.release();
  expect(committed).toEqual([{ op: "set_transport", loop_start: 8, loop_end: 16, loop_enabled: true }]);
  current = project([], { transport: { ...open.transport, loop_enabled: true, loop_start: 8, loop_end: 16 } });
  ruler.press(16 * PPB, BAND + 3, NO_MODS);
  for (let i = 1; i <= 6; i++) ruler.move(16 * PPB + (4 * PPB * i) / 6, BAND + 3, NO_MODS);
  await ruler.release();
  expect(committed[1]).toEqual({ op: "set_transport", loop_start: 8, loop_end: 20 });
  expect(seeks).toHaveLength(1);
});

test("⌘-wheel zooms around the pointer and Fit shows the whole song", () => {
  const action = wheelAction({ deltaX: 0, deltaY: -120, metaKey: true, ctrlKey: false, altKey: false, shiftKey: false });
  expect(action.kind).toBe("zoom");
  const factor = action.kind === "zoom" ? action.factor : 1;
  expect(factor).toBeCloseTo(1.15 ** 2, 9);
  const before = xToBeat(DEFAULT_TIMELINE, 300);
  const zoomed = setZoom(DEFAULT_TIMELINE, DEFAULT_TIMELINE.pixelsPerBeat * factor, 300);
  expect(zoomed.pixelsPerBeat).toBeGreaterThan(PPB);
  expect(xToBeat(zoomed, 300)).toBeCloseTo(before, 6);
  const fitted = fitView({ ...zoomed, scrollX: 500 }, 64, 1224);
  expect(fitted.scrollX).toBe(0);
  expect(fitted.pixelsPerBeat).toBeCloseTo(1200 / 64, 9);
  expect(fitView(zoomed, 4, 1224).pixelsPerBeat).toBeCloseTo(1200 / 16, 9);
  expect(wheelAction({ deltaX: 0, deltaY: -10, metaKey: true, ctrlKey: false, altKey: true, shiftKey: false })).toEqual({
    kind: "height",
    direction: 1,
  });
  expect(wheelAction({ deltaX: 0, deltaY: 30, metaKey: false, ctrlKey: false, altKey: false, shiftKey: true })).toEqual({
    kind: "scroll",
    dx: 30,
    dy: 0,
  });
  expect(stepHeight(DEFAULT_TIMELINE, 1).trackHeight).toBe("L");
  expect(stepHeight({ ...DEFAULT_TIMELINE, trackHeight: "XL" }, 1).trackHeight).toBe("XL");
});

test("the scroll reaches 64 beats past the song end and 40 px below the rows", () => {
  const range = scrollRange(DEFAULT_TIMELINE, 100, 0, 1000, 400, 1000);
  expect(range.maxX).toBe(Math.round((100 + 64) * PPB - 1000));
  expect(range.maxY).toBe(640);
  expect(scrollRange(DEFAULT_TIMELINE, 0, 0, 1000, 400, 100).maxY).toBe(0);
});

test("the grid thins out with the zoom and follows the time signatures", () => {
  expect(gridSteps(24, 4)).toEqual({ barStep: 1, beatStep: 1, sub: 0.5 });
  expect(gridSteps(5, 4)).toEqual({ barStep: 2, beatStep: null, sub: null });
  const lines = gridLines(DEFAULT_TIMELINE, project([]).time_signatures, 0, 4 * PPB);
  expect(lines.filter((l) => l.kind === "bar").map((l) => l.x)).toEqual([0, 96]);
  expect(lines.filter((l) => l.kind === "beat").map((l) => l.x)).toEqual([24, 48, 72, 120, 144, 168]);
  expect(lines.filter((l) => l.kind === "sub").map((l) => l.x).slice(0, 2)).toEqual([12, 36]);
  const sigs = [
    { bar: 0, beat: 0, numerator: 4, denominator: 4, beats_per_bar: 4 },
    { bar: 1, beat: 4, numerator: 6, denominator: 8, beats_per_bar: 3 },
  ];
  const marks = rulerMarks(DEFAULT_TIMELINE, sigs, 300);
  expect(marks.filter((m) => m.kind === "bar").map((m) => [m.x, m.label]).slice(0, 3)).toEqual([
    [0, "1"],
    [96, "2"],
    [168, "3"],
  ]);
  expect(marks.filter((m) => m.kind === "signature").map((m) => m.label)).toEqual(["4/4", "6/8"]);
});
