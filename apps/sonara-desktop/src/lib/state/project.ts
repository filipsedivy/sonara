// Keeping the page's project view current: every `session-changed`
// event asks for a new view, at most one request is in flight, and events that arrive meanwhile
// cost one more request, not one each. A view older than the one shown (by session version) is
// dropped.
//
// An event that changed only some tracks' mixer settings (a fader, a knob, a send, M or S: the
// flag `mixer` alone, tracks updated, nothing created or deleted; `changedTracks`) asks for those
// tracks only (`project_tracks`) and puts them into the view shown (`patchTracks`): a fader drag
// on 100 tracks × 40 clips then moves a few kilobytes per edit instead of the 1.9 MB view: in the
// meter benchmark's drag (PERF-01) the page updates in 0.4 ms instead of 2.2 ms per edit,
// and the meters keep 0 % dropped frames and a p99 of 44 ms, where whole views dropped 1.1 % with
// a p99 of 52 ms.
//
// An event that changed only some MIDI clips' notes (the flag `notes` alone, notes created or
// deleted, clips and notes updated; `changedClips`: a piano-roll drag, a key, the velocity lane,
// quantize, an agent's notes) asks for those clips only (`project_clips`) and puts them into their
// tracks (`patchClips`): the other tracks keep their objects, so the timeline rebuilds the edited
// track's clips and the mixer nothing. A note drag commits an edit per display frame; whole views of
// the piano roll benchmark's project cost the drag up to four long frames per gesture
// (PERF-01).
//
// An event that changed only some devices' state (the flag `devices` alone, devices updated,
// nothing created or deleted; `changedDevices`: a knob, a toggle, a choice, a preset, On/Off, an
// agent's `set_effect_parameter`) asks for those devices only (`project_devices`) and puts them into
// their tracks (`patchDevices`), although the prototype counts such an edit as structural. A knob
// drag commits an edit per display frame; in the meter benchmark's knob drag (PERF-01) the
// page applies a device refresh in 1 ms instead of 2 ms for a whole view, and whole views dropped up
// to 3 % of the frames. Every other event, and a patch that does not fit the view shown, asks for
// the whole view.

import type { ClipsView, DeviceView, DevicesView, ProjectView, SessionNotice, TrackView, TracksView } from "../types";

/** How a view came: whole, or patched by a refresh of some tracks or some clips. */
export type ViewPatch = "tracks" | "clips" | "devices" | null;

type PlacedClip = NonNullable<ClipsView["project"]>["clips"][number];

/**
 * The tracks an event changed when it changed only their mixer settings; null when it needs the
 * whole view.
 */
export function changedTracks(notice: SessionNotice): string[] | null {
  const changes = notice.changes;
  if (notice.kind !== "command" || changes.structure_changed) return null;
  if (changes.created.length > 0 || changes.deleted.length > 0) return null;
  if (changes.flags.length === 0 || !changes.flags.every((flag) => flag === "mixer")) return null;
  const ids = changes.updated.filter((object) => object.kind === "track").map((object) => object.id);
  return ids.length > 0 ? ids : null;
}

/**
 * The MIDI clips an event changed when it changed only their notes; null when it needs the whole
 * view.
 */
export function changedClips(notice: SessionNotice): string[] | null {
  const changes = notice.changes;
  if (notice.kind !== "command" || changes.structure_changed) return null;
  if (changes.flags.length === 0 || !changes.flags.every((flag) => flag === "notes")) return null;
  if (![...changes.created, ...changes.deleted].every((object) => object.kind === "note")) return null;
  if (!changes.updated.every((object) => object.kind === "clip" || object.kind === "note")) return null;
  const ids = changes.updated.filter((object) => object.kind === "clip").map((object) => object.id);
  return ids.length > 0 ? ids : null;
}

/**
 * `view` with the clips of `clips` in their tracks, or null when they do not fit it (another
 * project, another meter layout, a clip that is not where the view has it). The other tracks keep
 * their objects.
 */
export function patchClips(view: ProjectView, clips: ClipsView): ProjectView | null {
  const open = view.project;
  const delta = clips.project;
  if (!open || !delta || delta.id !== open.id || delta.meter_layout !== open.meter_layout) return null;
  const byTrack = new Map<string, Map<string, PlacedClip>>();
  for (const placed of delta.clips) {
    const clips = byTrack.get(placed.track_id) ?? new Map();
    clips.set(placed.clip.id, placed);
    byTrack.set(placed.track_id, clips);
  }
  let placedAll = 0;
  const tracks = open.tracks.map((track) => {
    const changed = byTrack.get(track.id);
    if (!changed) return track;
    let found = 0;
    const next = track.clips.map((clip) => {
      const placed = changed.get(clip.id);
      if (!placed) return clip;
      found += 1;
      return placed.clip;
    });
    placedAll += found;
    return found > 0 ? { ...track, clips: next } : track;
  });
  if (placedAll !== delta.clips.length) return null;
  return {
    version: clips.version,
    project: { ...open, dirty: delta.dirty, history: delta.history, tracks },
  };
}

/**
 * `view` with the tracks of `tracks` in place, or null when they do not fit it (another project,
 * another meter layout: the structure changed). The other tracks keep their objects, and a
 * patched track keeps its clips, which a mixer edit does not change, so the timeline sees the
 * same clips and the strips of the other tracks the same tracks.
 */
export function patchTracks(view: ProjectView, tracks: TracksView): ProjectView | null {
  const open = view.project;
  const delta = tracks.project;
  if (!open || !delta || delta.id !== open.id || delta.meter_layout !== open.meter_layout) return null;
  const byId = new Map(delta.tracks.map((track) => [track.id, track]));
  const patch = <T extends { id: string; clips: unknown }>(track: T): T => {
    const next = byId.get(track.id) as T | undefined;
    return next ? { ...next, clips: track.clips } : track;
  };
  return {
    version: tracks.version,
    project: {
      ...open,
      dirty: delta.dirty,
      history: delta.history,
      tracks: open.tracks.map(patch),
      master: patch(open.master),
    },
  };
}

/**
 * The devices an event changed when it changed only their state (a knob, a toggle, a choice, a
 * preset, On/Off, a rename: the flag `devices` alone, only devices updated, nothing created or
 * deleted; `changedDevices`); null when it needs the whole view. Such an event is
 * structural for the prototype's panels (`structure_changed`: `devices` is one of its flags), but
 * no object appears, vanishes or moves: an added, removed, moved or replaced device also updates
 * its track or creates or deletes a device, and a sidechain also carries `routing`.
 */
export function changedDevices(notice: SessionNotice): string[] | null {
  const changes = notice.changes;
  if (notice.kind !== "command") return null;
  if (changes.created.length > 0 || changes.deleted.length > 0) return null;
  if (changes.flags.length !== 1 || changes.flags[0] !== "devices") return null;
  if (changes.updated.length === 0 || !changes.updated.every((object) => object.kind === "device")) return null;
  return changes.updated.map((object) => object.id);
}

/**
 * `view` with the devices of `devices` in their tracks, or null when they do not fit it (another
 * project, another meter layout, a device that is not where the view has it). A patched track keeps
 * its clips and every other track its object, so the timeline keeps its model and the mixer redraws
 * one strip.
 */
export function patchDevices(view: ProjectView, devices: DevicesView): ProjectView | null {
  const open = view.project;
  const delta = devices.project;
  if (!open || !delta || delta.id !== open.id || delta.meter_layout !== open.meter_layout) return null;
  const byTrack = new Map<string, Map<string, DeviceView>>();
  for (const placed of delta.devices) {
    const changed = byTrack.get(placed.track_id) ?? new Map<string, DeviceView>();
    changed.set(placed.device.id, placed.device);
    byTrack.set(placed.track_id, changed);
  }
  let placedAll = 0;
  const patch = (track: TrackView): TrackView => {
    const changed = byTrack.get(track.id);
    if (!changed) return track;
    let found = 0;
    const take = (device: DeviceView): DeviceView => {
      const next = changed.get(device.id);
      if (!next) return device;
      found += 1;
      return next;
    };
    const instrument = track.instrument ? take(track.instrument) : null;
    const inserts = track.inserts.map(take);
    placedAll += found;
    return found > 0 ? { ...track, instrument, inserts } : track;
  };
  const tracks = open.tracks.map(patch);
  const master = patch(open.master);
  if (placedAll !== delta.devices.length) return null;
  return {
    version: devices.version,
    project: { ...open, dirty: delta.dirty, history: delta.history, tracks, master },
  };
}

export class ViewRefresher {
  private inFlight = false;
  /** A whole view is due. */
  private full = false;
  /** The tracks due, when only tracks are. */
  private readonly tracks = new Set<string>();
  /** The clips due, when only clips' notes are. */
  private readonly clips = new Set<string>();
  /** The devices due, when only devices' state is. */
  private readonly devices = new Set<string>();
  private current: ProjectView | null = null;
  private shownVersion = -1;
  /** Whole views requested so far. */
  requests = 0;
  /** Track refreshes requested so far. */
  trackRequests = 0;
  /** Clip refreshes requested so far. */
  clipRequests = 0;
  /** Device refreshes requested so far. */
  deviceRequests = 0;
  /** Whether a device edit asks for its devices only; the meter benchmark turns it off to compare. */
  devicePatches = true;

  /**
   * `show` gets each view, with how it was patched: `"tracks"` from a track refresh (no object
   * appeared or vanished), `"clips"` from a clip refresh (notes may have), `"devices"` from a device
   * refresh (nothing appeared or vanished), null for a whole view. Without `fetchTracks`,
   * `fetchClips` and `fetchDevices` every refresh is a whole view.
   */
  constructor(
    private readonly fetchView: () => Promise<ProjectView>,
    private readonly show: (view: ProjectView, patched: ViewPatch) => void,
    private readonly onError: (error: unknown) => void = () => {},
    private readonly fetchTracks: ((trackIds: string[]) => Promise<TracksView>) | null = null,
    private readonly fetchClips: ((clipIds: string[]) => Promise<ClipsView>) | null = null,
    private readonly fetchDevices: ((deviceIds: string[]) => Promise<DevicesView>) | null = null,
  ) {}

  /** A session event arrived (or the page starts): the view is refreshed. */
  notice(notice?: SessionNotice): void {
    const open = this.current?.project;
    const tracks = notice && this.fetchTracks && open ? changedTracks(notice) : null;
    const clips = !tracks && notice && this.fetchClips && open ? changedClips(notice) : null;
    const devices = !tracks && !clips && notice && this.fetchDevices && this.devicePatches && open ? changedDevices(notice) : null;
    if (tracks) for (const id of tracks) this.tracks.add(id);
    else if (clips) for (const id of clips) this.clips.add(id);
    else if (devices) for (const id of devices) this.devices.add(id);
    else this.full = true;
    if (!this.inFlight) void this.run();
  }

  /** Resolves once no request is in flight or pending (tests). */
  async settled(): Promise<void> {
    while (this.inFlight) await new Promise((resolve) => setTimeout(resolve, 1));
  }

  private async run(): Promise<void> {
    this.inFlight = true;
    try {
      while (this.full || this.tracks.size > 0 || this.clips.size > 0 || this.devices.size > 0) {
        if (this.full || (!this.fetchTracks && !this.fetchClips && !this.fetchDevices)) {
          this.full = false;
          this.tracks.clear();
          this.clips.clear();
          this.devices.clear();
          this.requests += 1;
          this.accept(await this.fetchView(), null);
          continue;
        }
        if (this.tracks.size > 0 && this.fetchTracks) {
          const ids = [...this.tracks];
          this.tracks.clear();
          this.trackRequests += 1;
          const tracks = await this.fetchTracks(ids);
          if (tracks.version < this.shownVersion) continue;
          const patched = this.current ? patchTracks(this.current, tracks) : null;
          if (patched) this.accept(patched, "tracks");
          else this.full = true;
          continue;
        }
        if (this.clips.size > 0 && this.fetchClips) {
          const ids = [...this.clips];
          this.clips.clear();
          this.clipRequests += 1;
          const clips = await this.fetchClips(ids);
          if (clips.version < this.shownVersion) continue;
          const patched = this.current ? patchClips(this.current, clips) : null;
          if (patched) this.accept(patched, "clips");
          else this.full = true;
          continue;
        }
        if (this.devices.size > 0 && this.fetchDevices) {
          const ids = [...this.devices];
          this.devices.clear();
          this.deviceRequests += 1;
          const devices = await this.fetchDevices(ids);
          if (devices.version < this.shownVersion) continue;
          const patched = this.current ? patchDevices(this.current, devices) : null;
          if (patched) this.accept(patched, "devices");
          else this.full = true;
          continue;
        }
        this.full = true;
      }
    } catch (error) {
      // What is still due waits for the next event.
      this.onError(error);
    } finally {
      this.inFlight = false;
    }
  }

  private accept(view: ProjectView, patched: ViewPatch): void {
    if (view.version < this.shownVersion) return;
    this.shownVersion = view.version;
    this.current = view;
    this.show(view, patched);
  }
}
