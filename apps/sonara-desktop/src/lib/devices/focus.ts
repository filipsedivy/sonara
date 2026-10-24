// Which track the Devices tab shows (the prototype's `DevicePanel` following the shared
// `Selection`): the track of the device that became current (`currentDeviceChanged`), else the
// selection's current track whenever the selection changes (`changed`: every track or clip
// selection, and a prune that dropped something), and the track a "Show Devices" names. It is the
// last of these events, not a function of the selection alone: a device made current on another
// track shows that track although the selected track stays, until the selection changes again.

import type { OpenProject } from "../types";
import type { Selection } from "../state/selection";
import { ownerOf } from "./model";

/**
 * What changed the selection: `select` a track or clip selection (the prototype's `changed`,
 * emitted even when nothing differs), `device` a new current device, `update` anything else (a
 * prune after a project change, the other window's selection), which counts only what differs.
 */
export type SelectionCause = "select" | "device" | "update";

function sameItems(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((id, i) => id === b[i]);
}

/** The track the Devices tab shows after the selection went from `before` to `after`. */
export function devicesTrackAfter(
  shown: string | null,
  before: Selection,
  after: Selection,
  project: OpenProject | null,
  cause: SelectionCause,
): string | null {
  if (cause !== "select" && after.currentDevice !== before.currentDevice && after.currentDevice !== null) {
    const owner = ownerOf(project, after.currentDevice);
    if (owner) return owner.id;
  }
  if (cause === "device") return shown;
  const changed =
    cause === "select" ||
    !sameItems(before.tracks, after.tracks) ||
    !sameItems(before.clips, after.clips) ||
    before.currentTrack !== after.currentTrack;
  return changed && after.currentTrack !== null ? after.currentTrack : shown;
}
