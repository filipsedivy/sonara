// The track headers beside the timeline (inventory TH; the prototype's sonara/ui/timeline/
// headers.py): a row's detail line, its geometry, and the arithmetic of a reorder drag. Plain
// functions; Headers.svelte draws the rows as DOM and calls them.

import type { TrackView } from "../types";
import type { Row } from "./layout";
import { pySigned } from "../mixer/controls";
import { pyFixed } from "../time/format";

/** The headers' width (`HEADER_WIDTH`). */
export const HEADER_WIDTH = 236;
/** The indent per folder level (`INDENT`). */
export const INDENT = 12;
/** The M, S and R buttons' size (`BUTTON`). */
export const BUTTON = 18;
/** Rows at least this high show the detail line. */
export const DETAIL_MIN_HEIGHT = 44;
/** The vertical drag that starts a reorder. */
export const REORDER_THRESHOLD = 6;

const KIND_LABELS: Record<string, string> = {
  audio: "AUDIO",
  instrument: "MIDI",
  bus: "BUS",
  folder: "FOLDER",
  master: "MASTER",
};

/**
 * `_detail`: "AUDIO  ·  +0.0 dB", "MIDI  ·  Analog Bass  ·  -6.0 dB  ·  L30  ·  2 fx",
 * "FOLDER".
 */
export function detailText(track: TrackView): string {
  const parts = [KIND_LABELS[track.kind] ?? track.kind.toUpperCase()];
  if (track.instrument) parts.push(track.instrument.preset || track.instrument.name);
  if (track.kind !== "folder") {
    const vol = track.mixer.volume_db;
    parts.push(vol <= -144 ? "-inf dB" : `${pySigned(vol, 1)} dB`);
    const pan = track.mixer.pan;
    if (Math.abs(pan) > 1e-3) parts.push(`${pan < 0 ? "L" : "R"}${pyFixed(Math.abs(pan) * 100, 0)}`);
  }
  if (track.inserts.length > 0) parts.push(`${track.inserts.length} fx`);
  return parts.join("  ·  ");
}

/** Whether a row has the R button (audio and instrument tracks). */
export function hasArm(track: TrackView): boolean {
  return track.kind === "audio" || track.kind === "instrument";
}

/** Whether a row has buttons at all: folders have none. */
export function hasButtons(track: TrackView): boolean {
  return track.kind !== "folder";
}

/** Where a row's parts sit, in pixels from its top-left corner (`geometry_for`). */
export interface HeaderGeometry {
  /** The swatch's left edge; the folder guide sits half an indent before it. */
  readonly left: number;
  /** The name's top. */
  readonly nameTop: number;
  /** The buttons' top. */
  readonly buttonTop: number;
  /** Whether the detail line shows. */
  readonly detail: boolean;
}

/** `geometry_for`: a compact row centres the name and the buttons; a tall one tops them. */
export function geometry(row: Row): HeaderGeometry {
  const compact = row.height < DETAIL_MIN_HEIGHT;
  return {
    left: 8 + row.depth * INDENT,
    nameTop: compact ? (row.height - 18) / 2 : 6,
    buttonTop: compact ? (row.height - BUTTON) / 2 : 7,
    detail: !compact,
  };
}

/** `insertion_index`: the track index a dragged header would go before, 0…rows. */
export function insertionIndex(rows: readonly Row[], y: number, scrollY: number): number {
  const cy = y + scrollY;
  for (const row of rows) {
    if (cy < row.top + row.height / 2) return row.index;
  }
  return rows.length;
}

/** `mouseReleaseEvent`: the new index of the track at `current` dropped before `target`; null
 * when it stays. */
export function reorderIndex(current: number, target: number): number | null {
  const index = target > current ? target - 1 : target;
  return index === current ? null : index;
}
