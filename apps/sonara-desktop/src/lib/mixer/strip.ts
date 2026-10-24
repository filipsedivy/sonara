// What a channel strip shows and offers (inventory MX; the prototype's `ChannelStrip` and
// `InsertList` in sonara/ui/mixer/panel.py): the mixed tracks, a strip's subtitle, the insert
// list's drop arithmetic, and the menus of effects and send targets. Plain functions; the
// strips (ChannelStrip.svelte) call them.

import type { MenuItem } from "../shell/menu";
import type { OpenProject, PluginGroup, TrackView } from "../types";

/** A strip's width (`STRIP_WIDTH`); the master's is 16 px wider. */
export const STRIP_WIDTH = 104;
/** The send rows visible before the list scrolls (`SEND_ROWS_VISIBLE`). */
export const SEND_ROWS_VISIBLE = 3;
/** The send level of a new send. */
export const NEW_SEND_DB = -12;

const KIND_LABELS: Record<string, string> = {
  audio: "Audio",
  instrument: "Instrument",
  bus: "Bus",
  master: "Master",
};

/** The tracks with a strip, in track order: every track but the folders (`_mixed_tracks`). */
export function mixedTracks(project: OpenProject): TrackView[] {
  return project.tracks.filter((track) => track.kind !== "folder");
}

/** The subtitle: "▸ folder" for a folder's child, else the instrument's name, else the kind. */
export function subtitle(track: TrackView, project: OpenProject): string {
  if (track.parent_id) {
    const parent = project.tracks.find((t) => t.id === track.parent_id);
    if (parent) return `▸ ${parent.name}`;
  }
  if (track.instrument) return track.instrument.name;
  return KIND_LABELS[track.kind] ?? track.kind;
}

/** Whether a strip has the R button (audio and instrument tracks). */
export function canArm(track: TrackView): boolean {
  return track.kind === "audio" || track.kind === "instrument";
}

/** A strip's index for its colour: the track's place in the project, 0 for the master. */
export function colorIndex(project: OpenProject, track: TrackView): number {
  const at = project.tracks.findIndex((t) => t.id === track.id);
  return at < 0 ? 0 : at;
}

/**
 * `InsertList.dropEvent`: the insert index of a drop on list row `target` (−1 below the rows,
 * which is the end); the instrument's row, first in the list, is not an insert.
 */
export function dropIndex(target: number, rows: number, hasInstrument: boolean): number {
  const row = target < 0 ? rows : target;
  return Math.max(0, row - (hasInstrument ? 1 : 0));
}

/**
 * `_move_device`: the new index of `deviceId` among `inserts` for a drop at insert `index`,
 * which counts the positions before the device is taken out; null when nothing moves.
 */
export function moveIndex(inserts: readonly string[], deviceId: string, index: number): number | null {
  const old = inserts.indexOf(deviceId);
  if (old < 0) return null;
  let next = index > old ? index - 1 : index;
  next = Math.max(0, Math.min(next, inserts.length - 1));
  return next === old ? null : next;
}

/**
 * The effect picker (`plugin_menu`): a submenu per category, or the plugins directly when there
 * is only one category.
 */
export function effectMenu(groups: readonly PluginGroup[], pick: (pluginId: string) => void): MenuItem[] {
  const entries = (group: PluginGroup): MenuItem[] =>
    group.plugins.map((plugin) => ({ label: plugin.name, title: plugin.description, run: () => pick(plugin.id) }));
  if (groups.length === 1) return entries(groups[0]);
  return groups.map((group) => ({ label: group.title, items: entries(group) }));
}

/** The "+" menu of sends (`_fill_send_menu`): the buses the track can still send to. */
export function sendMenu(track: TrackView, add: (busId: string) => void): MenuItem[] {
  if (track.send_targets.length === 0) return [{ label: "No buses available", disabled: true }];
  return track.send_targets.map((bus) => ({ label: bus.name, run: () => add(bus.id) }));
}
