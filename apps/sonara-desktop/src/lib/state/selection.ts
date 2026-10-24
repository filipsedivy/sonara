// The prototype's GUI-only selection (`Selection` in sonara/ui/bridge.py), shared by the timeline,
// the piano roll, the mixer and the inspector: ordered tracks with additive toggling, clips,
// notes, and the current track, clip and device. Plain values and functions; the reactive store
// (gui.svelte.ts) holds the current value.

import type { OpenProject, TrackView } from "../types";

export interface Selection {
  /** Selected tracks, in the order they were selected. */
  readonly tracks: readonly string[];
  /** Selected clips, in order. */
  readonly clips: readonly string[];
  /** Selected notes of the current clip. */
  readonly notes: readonly string[];
  readonly currentTrack: string | null;
  readonly currentClip: string | null;
  readonly currentDevice: string | null;
}

export const EMPTY_SELECTION: Selection = {
  tracks: [],
  clips: [],
  notes: [],
  currentTrack: null,
  currentClip: null,
  currentDevice: null,
};

/** Toggles `id` in `items`, or adds it at the end. */
function toggle(items: readonly string[], id: string): string[] {
  return items.includes(id) ? items.filter((item) => item !== id) : [...items, id];
}

/** `select_track`: `null` clears the tracks; `additive` toggles; otherwise only `trackId`. */
export function selectTrack(
  selection: Selection,
  trackId: string | null,
  additive = false,
): Selection {
  const tracks = trackId === null ? [] : additive ? toggle(selection.tracks, trackId) : [trackId];
  return { ...selection, tracks, currentTrack: trackId };
}

/** `select_clips`: `additive` toggles each; otherwise the clips, without duplicates. */
export function selectClips(
  selection: Selection,
  clipIds: readonly string[],
  additive = false,
): Selection {
  const clips = additive
    ? clipIds.reduce<string[]>((items, id) => toggle(items, id), [...selection.clips])
    : [...new Set(clipIds)];
  return { ...selection, clips };
}

/** `set_current_clip`: a new current clip clears the note selection. */
export function setCurrentClip(selection: Selection, clipId: string | null): Selection {
  if (clipId === selection.currentClip) return selection;
  return { ...selection, currentClip: clipId, notes: [] };
}

/** `set_current_device`. */
export function setCurrentDevice(selection: Selection, deviceId: string | null): Selection {
  if (deviceId === selection.currentDevice) return selection;
  return { ...selection, currentDevice: deviceId };
}

/** The piano roll's note selection: `additive` toggles each; otherwise only `noteIds`. */
export function selectNotes(
  selection: Selection,
  noteIds: readonly string[],
  additive = false,
): Selection {
  const notes = additive
    ? noteIds.reduce<string[]>((items, id) => toggle(items, id), [...selection.notes])
    : [...new Set(noteIds)];
  return { ...selection, notes };
}

function allTracks(project: OpenProject): TrackView[] {
  return [...project.tracks, project.master];
}

/** `prune`: drops what no longer exists, after an undo, a delete or an agent's edit. */
export function prune(selection: Selection, project: OpenProject | null): Selection {
  if (project === null) return EMPTY_SELECTION;
  const tracks = new Set(allTracks(project).map((t) => t.id));
  const clips = new Map<string, Set<string>>();
  const devices = new Set<string>();
  for (const track of allTracks(project)) {
    for (const clip of track.clips) {
      clips.set(clip.id, new Set(clip.type === "midi" ? clip.notes.map((n) => n.id) : []));
    }
    if (track.instrument) devices.add(track.instrument.id);
    for (const device of track.inserts) devices.add(device.id);
  }
  const currentClip =
    selection.currentClip !== null && clips.has(selection.currentClip)
      ? selection.currentClip
      : null;
  const notesOfClip = currentClip === null ? null : clips.get(currentClip);
  return {
    tracks: selection.tracks.filter((id) => tracks.has(id)),
    clips: selection.clips.filter((id) => clips.has(id)),
    notes: notesOfClip ? selection.notes.filter((id) => notesOfClip.has(id)) : [],
    currentTrack:
      selection.currentTrack !== null && tracks.has(selection.currentTrack)
        ? selection.currentTrack
        : null,
    currentClip,
    currentDevice:
      selection.currentDevice !== null && devices.has(selection.currentDevice)
        ? selection.currentDevice
        : null,
  };
}

/**
 * The note selection pruned against the current clip's notes alone (a refresh of some clips'
 * notes, which leaves the tracks, clips and devices as they were).
 */
export function pruneNotes(selection: Selection, project: OpenProject | null): Selection {
  if (selection.notes.length === 0) return selection;
  if (project === null || selection.currentClip === null) return { ...selection, notes: [] };
  for (const track of project.tracks) {
    for (const clip of track.clips) {
      if (clip.id !== selection.currentClip) continue;
      const ids = new Set(clip.type === "midi" ? clip.notes.map((n) => n.id) : []);
      const notes = selection.notes.filter((id) => ids.has(id));
      return notes.length === selection.notes.length ? selection : { ...selection, notes };
    }
  }
  return { ...selection, notes: [] };
}

/** Whether two selections hold the same IDs. */
export function sameSelection(a: Selection, b: Selection): boolean {
  const same = (x: readonly string[], y: readonly string[]) =>
    x.length === y.length && x.every((id, i) => id === y[i]);
  return (
    same(a.tracks, b.tracks) &&
    same(a.clips, b.clips) &&
    same(a.notes, b.notes) &&
    a.currentTrack === b.currentTrack &&
    a.currentClip === b.currentClip &&
    a.currentDevice === b.currentDevice
  );
}
