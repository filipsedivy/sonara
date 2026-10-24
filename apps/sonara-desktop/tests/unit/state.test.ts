import { expect, test } from "@playwright/test";
import type { LiveFrame } from "../../src/lib/live/frame";
import { MidiFocus } from "../../src/lib/state/focus";
import { ViewRefresher } from "../../src/lib/state/project";
import * as selection from "../../src/lib/state/selection";
import {
  DEFAULT_SNAP,
  barToBeat,
  beatToBar,
  gridBeats,
  snapBeat,
  snapToGrid,
} from "../../src/lib/state/snap";
import {
  DEFAULT_TIMELINE,
  START_PLAYHEAD,
  beatToX,
  followPlayhead,
  playheadFromFrame,
  setZoom,
  zoomBy,
} from "../../src/lib/state/timeline";
import type { OpenProject, ProjectView, TimeSignatureView, TrackView } from "../../src/lib/types";

// The GUI-only state of the prototype's panels (selection, snap, playhead, zoom, Follow), the
// project view's refresh on events, and the MIDI focus.

function track(id: string, clips: TrackView["clips"] = []): TrackView {
  return {
    id,
    name: id,
    kind: "instrument",
    color: null,
    parent_id: null,
    depth: 0,
    mixer: { volume_db: 0, pan: 0, mute: false, solo: false, solo_safe: false },
    output_id: "master",
    output_targets: [],
    sends: [],
    send_targets: [],
    input: { channels: [], armed: false, monitor: "auto" },
    instrument: null,
    inserts: [],
    clips,
  };
}

function project(tracks: TrackView[]): OpenProject {
  return {
    id: "prj_1",
    name: "Song",
    path: null,
    dirty: false,
    sample_rate: 48000,
    length_beats: null,
    end_beat: 16,
    tempo: [{ beat: 0, bpm: 120, ramp: false }],
    time_signatures: [{ bar: 0, beat: 0, numerator: 4, denominator: 4, beats_per_bar: 4 }],
    transport: {
      loop_enabled: false,
      loop_start: 0,
      loop_end: 16,
      punch_enabled: false,
      punch_in: 0,
      punch_out: 16,
      metronome: false,
      count_in_bars: 0,
    },
    metadata: { title: "", artist: "", album: "", genre: "", composer: "", comment: "" },
    tracks,
    master: { ...track("master"), kind: "master" },
    markers: [],
    sections: [],
    assets: [],
    plugins: {},
    history: { undo: null, redo: null },
    meter_layout: 1,
  };
}

test("tracks select alone, or toggle additively, in order", () => {
  let s = selection.selectTrack(selection.EMPTY_SELECTION, "a");
  s = selection.selectTrack(s, "b", true);
  s = selection.selectTrack(s, "c", true);
  expect(s.tracks).toEqual(["a", "b", "c"]);
  expect(s.currentTrack).toBe("c");
  s = selection.selectTrack(s, "b", true);
  expect(s.tracks).toEqual(["a", "c"]);
  s = selection.selectTrack(s, "d");
  expect(s.tracks).toEqual(["d"]);
  s = selection.selectTrack(s, null);
  expect([s.tracks, s.currentTrack]).toEqual([[], null]);
  s = selection.selectClips(s, ["x", "y", "x"]);
  expect(s.clips).toEqual(["x", "y"]);
  s = selection.selectClips(s, ["y", "z"], true);
  expect(s.clips).toEqual(["x", "z"]);
});

test("a new current clip clears the notes; pruning drops what is gone", () => {
  const midi = {
    type: "midi" as const,
    id: "clp_1",
    name: "c",
    start: 0,
    length: 4,
    muted: false,
    color: null,
    content_offset: 0,
    loop_length: null,
    notes: [{ id: "n_1", pitch: 60, start: 0, length: 1, velocity: 100, channel: 0 }],
  };
  let s = selection.setCurrentClip(selection.EMPTY_SELECTION, "clp_1");
  s = selection.selectNotes(s, ["n_1", "n_2"]);
  s = selection.selectTrack(s, "trk_1");
  s = selection.selectTrack(s, "trk_gone", true);
  s = selection.selectClips(s, ["clp_1", "clp_gone"]);
  s = selection.setCurrentDevice(s, "dev_gone");
  const pruned = selection.prune(s, project([track("trk_1", [midi])]));
  expect(pruned.tracks).toEqual(["trk_1"]);
  expect(pruned.clips).toEqual(["clp_1"]);
  expect(pruned.notes).toEqual(["n_1"]);
  expect(pruned.currentTrack).toBeNull();
  expect(pruned.currentClip).toBe("clp_1");
  expect(pruned.currentDevice).toBeNull();
  expect(selection.setCurrentClip(pruned, "clp_2").notes).toEqual([]);
  expect(selection.prune(s, null)).toEqual(selection.EMPTY_SELECTION);
  expect(selection.sameSelection(pruned, { ...pruned })).toBe(true);
});

const waltz: TimeSignatureView[] = [
  { bar: 0, beat: 0, numerator: 4, denominator: 4, beats_per_bar: 4 },
  { bar: 2, beat: 8, numerator: 3, denominator: 4, beats_per_bar: 3 },
];

test("the snap grid follows the prototype's SnapModel", () => {
  expect(DEFAULT_SNAP).toEqual({ enabled: true, gridName: "1/16" });
  expect(gridBeats(DEFAULT_SNAP, null)).toBe(0.25);
  const bar = { enabled: true, gridName: "Bar" as const };
  expect(gridBeats(bar, waltz, 1)).toBe(4);
  expect(gridBeats(bar, waltz, 9)).toBe(3);
  expect(gridBeats(bar, null)).toBe(4);
  expect(beatToBar(waltz, 11)).toBe(3);
  expect(barToBeat(waltz, 2.5)).toBe(9.5);
  // Nearest, floor and ceil on a 1/4 grid; never before 0; ⌥ only clamps.
  const quarter = { enabled: true, gridName: "1/4" as const };
  expect(snapBeat(quarter, 3.4, waltz)).toBe(3);
  expect(snapBeat(quarter, 3.6, waltz, "floor")).toBe(3);
  expect(snapBeat(quarter, 3.1, waltz, "ceil")).toBe(4);
  expect(snapBeat(quarter, -0.4, waltz)).toBe(0);
  expect(snapBeat(quarter, 3.4, waltz, "nearest", true)).toBe(3.4);
  expect(snapBeat({ ...quarter, enabled: false }, 3.4, waltz)).toBe(3.4);
  // "Bar" snaps through the signatures: bar 2 starts at beat 8, bar 3 at beat 11.
  expect(snapBeat(bar, 9.4, waltz)).toBe(8);
  expect(snapBeat(bar, 9.6, waltz)).toBe(11);
  expect(snapBeat(bar, 8.1, waltz, "ceil")).toBe(11);
  expect(snapToGrid(5, 0)).toBe(5);
  expect(snapToGrid(0.375, 0.25)).toBe(0.5);
});

function frame(state: LiveFrame["state"], beat: number | null): LiveFrame {
  return {
    sequence: 0,
    engine: "running",
    state,
    position: { frame: 0, beat, seconds: beat === null ? null : beat / 2 },
    device_stopped: false,
    sample_rate: 48000,
    buffer_frames: 256,
    render_load: 0,
    callback_load: 0,
    output_latency_ms: 0,
    lookahead_ms: 80,
    queued_ms: 0,
    underruns: 0,
    device_errors: 0,
    version: 1,
    meter_layout: 1,
    meters: [],
    gain_reduction: [],
  };
}

test("the playhead follows the transport while it rolls or pauses", () => {
  let playhead = { beat: 3, playing: false };
  expect(playheadFromFrame(playhead, frame("stopped", 0))).toBe(playhead);
  playhead = playheadFromFrame(playhead, frame("playing", 5));
  expect(playhead).toEqual({ beat: 5, playing: true });
  playhead = playheadFromFrame(playhead, frame("paused", 6));
  expect(playhead).toEqual({ beat: 6, playing: false });
  playhead = playheadFromFrame({ beat: 6, playing: true }, frame("stopped", 0));
  expect(playhead).toEqual({ beat: 6, playing: false });
  expect(playheadFromFrame(START_PLAYHEAD, frame("recording", null))).toEqual({
    beat: 0,
    playing: false,
  });
});

// `test_follow_playhead_scrolls_during_playback` and `ctrl_wheel_zoom_is_anchored`.
test("Follow scrolls during playback, and zoom keeps the anchor in place", () => {
  const width = 800;
  let view = followPlayhead(DEFAULT_TIMELINE, { beat: 200, playing: true }, width);
  expect(beatToX(view, 200)).toBeLessThan(width * 0.5);
  expect(beatToX(view, 200)).toBeCloseTo(width * 0.1);
  const off = { ...view, follow: false };
  expect(followPlayhead(off, { beat: 400, playing: true }, width)).toBe(off);
  expect(followPlayhead(view, { beat: 400, playing: false }, width)).toBe(view);
  const anchorBeat = (view.scrollX + 300) / view.pixelsPerBeat;
  view = zoomBy(view, 1.15, 300);
  expect(beatToX(view, anchorBeat)).toBeCloseTo(300);
  expect(setZoom(view, 1e6).pixelsPerBeat).toBe(1200);
  expect(setZoom(view, 0.1).pixelsPerBeat).toBe(1.5);
  expect(setZoom(DEFAULT_TIMELINE, 0.1, 0).scrollX).toBe(0);
});

function view(version: number, name = "Song"): ProjectView {
  return { version, project: { ...project([]), name } };
}

test("events refresh the view with one request in flight and one more at most", async () => {
  const pending: ((view: ProjectView) => void)[] = [];
  const shown: number[] = [];
  const refresher = new ViewRefresher(
    () => new Promise<ProjectView>((resolve) => pending.push(resolve)),
    (v) => shown.push(v.version),
  );
  refresher.notice();
  for (let i = 0; i < 10; i++) refresher.notice();
  expect(refresher.requests).toBe(1);
  pending.shift()?.(view(5));
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(refresher.requests).toBe(2);
  // An older view than the one shown is dropped.
  pending.shift()?.(view(4));
  await refresher.settled();
  expect(shown).toEqual([5]);
  refresher.notice();
  pending.shift()?.(view(9));
  await refresher.settled();
  expect(shown).toEqual([5, 9]);
});

test("the MIDI focus follows the current track and is sent again for a new transport", async () => {
  const sent: (string | null)[] = [];
  const focus = new MidiFocus(async (trackId) => {
    sent.push(trackId);
  });
  focus.follow(null);
  focus.follow(null);
  focus.follow("trk_1");
  focus.follow("trk_1");
  focus.follow("trk_2");
  focus.reset();
  focus.follow("trk_2");
  expect(sent).toEqual([null, "trk_1", "trk_2", "trk_2"]);
});
