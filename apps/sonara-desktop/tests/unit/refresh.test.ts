import { expect, test } from "@playwright/test";
import { ViewRefresher, changedClips, changedTracks, patchClips, patchTracks, type ViewPatch } from "../../src/lib/state/project";
import * as selection from "../../src/lib/state/selection";
import { sameRows } from "../../src/lib/timeline/layout";
import { sameTimeline } from "../../src/lib/timeline/model";
import type { ClipsView, MidiClipView, ProjectView, SessionNotice, TracksView, TrackView } from "../../src/lib/types";
import { midiClip, project, track } from "../support/project";

// The refresh of a mixer edit (PERF-01): a fader drag asks for the tracks it
// changed instead of the whole view, and the timeline keeps its model through it. The refresh of a
// note edit: a piano-roll drag asks for the clips it changed.

function notice(kind: SessionNotice["kind"], flags: string[], updated: [string, string][], extra = {}): SessionNotice {
  return {
    version: 2,
    kind,
    label: "Track Volume",
    source: "gui",
    changes: {
      created: [],
      updated: updated.map(([k, id]) => ({ kind: k, id })),
      deleted: [],
      flags,
      structure_changed: false,
      ...extra,
    },
  };
}

test("only an edit of the mixer alone refreshes just its tracks", () => {
  expect(changedTracks(notice("command", ["mixer"], [["track", "a"]]))).toEqual(["a"]);
  // A send's level names its send and its track.
  expect(changedTracks(notice("command", ["mixer"], [["send", "snd_1"], ["track", "a"]]))).toEqual(["a"]);
  expect(changedTracks(notice("command", ["mixer"], [["track", "master"]]))).toEqual(["master"]);
  expect(changedTracks(notice("undo", ["mixer"], [["track", "a"]]))).toBeNull();
  expect(changedTracks(notice("command", ["mixer", "routing"], [["track", "a"]]))).toBeNull();
  expect(changedTracks(notice("command", ["tracks"], [["track", "a"]]))).toBeNull();
  expect(changedTracks(notice("command", [], [["track", "a"]]))).toBeNull();
  expect(changedTracks(notice("command", ["mixer"], []))).toBeNull();
  expect(changedTracks(notice("command", ["mixer"], [["track", "a"]], { structure_changed: true }))).toBeNull();
  expect(changedTracks(notice("command", ["mixer"], [["track", "a"]], { created: [{ kind: "send", id: "s" }] }))).toBeNull();
});

function mixView(version: number, volume = 0): ProjectView {
  const vox = { ...track("a", "Vox", "audio"), mixer: { volume_db: volume, pan: 0, mute: false, solo: false, solo_safe: false } };
  return { version, project: project([vox, track("k", "Keys", "instrument", [midiClip("c", 0, 4)])], { meter_layout: 9 }) };
}

function tracksView(version: number, tracks: TrackView[], extra = {}): TracksView {
  return {
    version,
    project: { id: "prj_1", dirty: true, history: { undo: "Track Volume", redo: null }, meter_layout: 9, tracks, ...extra },
  };
}

test("a track refresh lands in the view and keeps every other object", () => {
  const before = mixView(1);
  const open = before.project;
  if (!open) throw new Error("no project");
  const vox = { ...open.tracks[0], mixer: { ...open.tracks[0].mixer, volume_db: -6 }, clips: [] };
  const master = { ...open.master, mixer: { ...open.master.mixer, mute: true } };
  const after = patchTracks(before, tracksView(3, [vox, master]));
  const next = after?.project;
  if (!after || !next) throw new Error("not patched");
  expect(after.version).toBe(3);
  expect(next.tracks[0].mixer.volume_db).toBe(-6);
  // The patched track keeps its clips; the others are the same objects.
  expect(next.tracks[0].clips).toBe(open.tracks[0].clips);
  expect(next.tracks[1]).toBe(open.tracks[1]);
  expect(next.master.mixer.mute).toBe(true);
  expect(next.tempo).toBe(open.tempo);
  expect([next.dirty, next.history.undo]).toEqual([true, "Track Volume"]);
  // The timeline draws the same; its rows are the same.
  expect(sameTimeline(open, next)).toBe(true);
  expect(sameRows(open.tracks, next.tracks)).toBe(true);
  // Another layout or project, or no project: the whole view is needed.
  expect(patchTracks(before, tracksView(3, [vox], { meter_layout: 10 }))).toBeNull();
  expect(patchTracks(before, tracksView(3, [vox], { id: "prj_2" }))).toBeNull();
  expect(patchTracks(before, { version: 3, project: null })).toBeNull();
  expect(patchTracks({ version: 1, project: null }, tracksView(3, [vox]))).toBeNull();
});

test("the timeline rebuilds for a mute, a clip or another track, not for a fader", () => {
  const open = mixView(1).project;
  if (!open) throw new Error("no project");
  const withTrack = (change: Partial<TrackView>) => ({ ...open, tracks: [{ ...open.tracks[0], ...change }, open.tracks[1]] });
  expect(sameTimeline(open, withTrack({ mixer: { ...open.tracks[0].mixer, volume_db: -20, pan: 0.5 } }))).toBe(true);
  expect(sameTimeline(open, withTrack({ mixer: { ...open.tracks[0].mixer, mute: true } }))).toBe(false);
  expect(sameTimeline(open, withTrack({ clips: [] }))).toBe(false);
  expect(sameTimeline(open, withTrack({ color: "#123456" }))).toBe(false);
  expect(sameTimeline(open, withTrack({ name: "Other" }))).toBe(false);
  expect(sameTimeline(open, { ...open, tracks: open.tracks.slice(1) })).toBe(false);
  expect(sameTimeline(open, { ...open, tempo: [...open.tempo] })).toBe(false);
  expect(sameRows(open.tracks, [{ ...open.tracks[0], depth: 1 }, open.tracks[1]])).toBe(false);
  expect(sameRows(open.tracks, [open.tracks[1], open.tracks[0]])).toBe(false);
});

test("a fader drag's events ask for their tracks, one request in flight; anything else for the view", async () => {
  const views: ((view: ProjectView) => void)[] = [];
  const asked: string[][] = [];
  const trackAnswers: ((tracks: TracksView) => void)[] = [];
  const shown: [number, ViewPatch][] = [];
  const refresher = new ViewRefresher(
    () => new Promise<ProjectView>((resolve) => views.push(resolve)),
    (view, patched) => shown.push([view.version, patched]),
    () => {},
    (ids) => {
      asked.push(ids);
      return new Promise<TracksView>((resolve) => trackAnswers.push(resolve));
    },
  );
  // The first view is whole, whatever the event.
  refresher.notice(notice("command", ["mixer"], [["track", "a"]]));
  expect(refresher.requests).toBe(1);
  views.shift()?.(mixView(1));
  await refresher.settled();
  const open = mixView(1).project;
  if (!open) throw new Error("no project");
  // A drag: the first move's tracks, then the moves meanwhile together.
  refresher.notice(notice("command", ["mixer"], [["track", "a"]]));
  refresher.notice(notice("command", ["mixer"], [["track", "a"]]));
  refresher.notice(notice("command", ["mixer"], [["send", "snd_1"], ["track", "k"]]));
  expect(asked).toEqual([["a"]]);
  trackAnswers.shift()?.(tracksView(2, [{ ...open.tracks[0], mixer: { ...open.tracks[0].mixer, volume_db: -1 } }]));
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(asked).toEqual([["a"], ["a", "k"]]);
  trackAnswers.shift()?.(tracksView(4, [open.tracks[0], open.tracks[1]]));
  await refresher.settled();
  expect(shown).toEqual([
    [1, null],
    [2, "tracks"],
    [4, "tracks"],
  ]);
  // A structural event asks for the whole view; a refresh that no longer fits does too.
  refresher.notice(notice("command", ["tracks"], [["track", "a"]]));
  views.shift()?.(mixView(5));
  await refresher.settled();
  refresher.notice(notice("command", ["mixer"], [["track", "a"]]));
  trackAnswers.shift()?.(tracksView(6, [open.tracks[0]], { meter_layout: 77 }));
  await new Promise((resolve) => setTimeout(resolve, 0));
  views.shift()?.(mixView(6));
  await refresher.settled();
  expect(shown.slice(3)).toEqual([
    [5, null],
    [6, null],
  ]);
  expect([refresher.requests, refresher.trackRequests]).toEqual([3, 3]);
});

function clipsView(version: number, clips: { track_id: string; clip: MidiClipView }[], extra = {}): ClipsView {
  return {
    version,
    project: { id: "prj_1", dirty: true, history: { undo: "Edit Notes", redo: null }, meter_layout: 9, clips, ...extra },
  };
}

test("only an edit of some clips' notes refreshes just those clips", () => {
  const notes = (updated: [string, string][], extra = {}) => notice("command", ["notes"], updated, extra);
  expect(changedClips(notes([["clip", "c"], ["note", "n1"]]))).toEqual(["c"]);
  // Notes created or deleted (add, duplicate, delete) are the clip's notes too.
  expect(changedClips(notes([["clip", "c"]], { created: [{ kind: "note", id: "n9" }] }))).toEqual(["c"]);
  expect(changedClips(notes([["clip", "c"]], { deleted: [{ kind: "note", id: "n1" }] }))).toEqual(["c"]);
  expect(changedClips(notes([["clip", "c"]], { deleted: [{ kind: "clip", id: "c" }] }))).toBeNull();
  expect(changedClips(notice("command", ["notes", "clips"], [["clip", "c"]]))).toBeNull();
  expect(changedClips(notice("undo", ["notes"], [["clip", "c"]]))).toBeNull();
  expect(changedClips(notes([["note", "n1"]]))).toBeNull();
  expect(changedClips(notes([["track", "k"], ["clip", "c"]]))).toBeNull();
  expect(changedClips(notes([["clip", "c"]], { structure_changed: true }))).toBeNull();
  expect(changedClips(notice("command", ["mixer"], [["track", "a"]]))).toBeNull();
});

test("a clip refresh lands in its track, keeps every other object and prunes the notes", () => {
  const before = mixView(1);
  const open = before.project;
  if (!open) throw new Error("no project");
  const old = open.tracks[1].clips[0] as MidiClipView;
  const clip: MidiClipView = { ...old, notes: old.notes.slice(1).map((n) => ({ ...n, pitch: n.pitch + 1 })) };
  const after = patchClips(before, clipsView(3, [{ track_id: "k", clip }]));
  expect(after?.version).toBe(3);
  expect(after?.project?.tracks[0]).toBe(open.tracks[0]);
  expect(after?.project?.master).toBe(open.master);
  expect(after?.project?.tracks[1].clips[0]).toBe(clip);
  expect(after?.project?.tracks[1].mixer).toBe(open.tracks[1].mixer);
  expect(after?.project?.history.undo).toBe("Edit Notes");
  // The timeline sees the edited track's clips change, and only them.
  expect(sameTimeline(open, after?.project ?? open)).toBe(false);
  // Another project, another meter layout, or a clip the view has elsewhere: a whole view.
  expect(patchClips(before, clipsView(3, [{ track_id: "k", clip }], { id: "prj_2" }))).toBeNull();
  expect(patchClips(before, clipsView(3, [{ track_id: "k", clip }], { meter_layout: 1 }))).toBeNull();
  expect(patchClips(before, clipsView(3, [{ track_id: "a", clip }]))).toBeNull();
  // The note selection keeps the notes that are left.
  let chosen = selection.setCurrentClip(selection.EMPTY_SELECTION, old.id);
  chosen = selection.selectNotes(chosen, [old.notes[0].id, old.notes[1].id]);
  expect(selection.pruneNotes(chosen, after?.project ?? null).notes).toEqual([old.notes[1].id]);
});

test("a note drag's events ask for their clips, one request in flight", async () => {
  const views: ((view: ProjectView) => void)[] = [];
  const asked: string[][] = [];
  const answers: ((clips: ClipsView) => void)[] = [];
  const shown: [number, ViewPatch][] = [];
  const refresher = new ViewRefresher(
    () => new Promise<ProjectView>((resolve) => views.push(resolve)),
    (view, patched) => shown.push([view.version, patched]),
    () => {},
    async () => tracksView(0, []),
    (ids) => {
      asked.push(ids);
      return new Promise<ClipsView>((resolve) => answers.push(resolve));
    },
  );
  refresher.notice();
  views.shift()?.(mixView(1));
  await refresher.settled();
  const clip = mixView(1).project?.tracks[1].clips[0] as MidiClipView;
  const edit = notice("command", ["notes"], [["clip", "c"], ["note", "c_n0"]]);
  refresher.notice(edit);
  refresher.notice(edit);
  refresher.notice(edit);
  expect(asked).toEqual([["c"]]);
  answers.shift()?.(clipsView(2, [{ track_id: "k", clip }]));
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(asked).toEqual([["c"], ["c"]]);
  answers.shift()?.(clipsView(4, [{ track_id: "k", clip }]));
  await refresher.settled();
  expect(shown).toEqual([
    [1, null],
    [2, "clips"],
    [4, "clips"],
  ]);
  expect([refresher.requests, refresher.clipRequests]).toEqual([1, 2]);
});
