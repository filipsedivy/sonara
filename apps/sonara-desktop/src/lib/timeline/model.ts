// What the timeline draws and hit-tests, built once per project view: each track's clips sorted
// by start, with their starts and the longest clip for the prototype's bisection over visible
// clips (`ArrangementCanvas._visible_clips`), the colours, and the clip and track lookups.

import { TempoMap } from "../time/tempo";
import type { ClipView, OpenProject, TrackView } from "../types";
import { type Rgb, trackColor } from "./colors";

/** One track's clips, sorted by start. */
export interface TrackClips {
  readonly track: TrackView;
  readonly index: number;
  readonly clips: readonly ClipView[];
  readonly starts: Float64Array;
  /** The longest clip's length: a clip starting this far before a range may reach into it. */
  readonly longest: number;
  readonly color: Rgb;
}

/**
 * Whether `next` draws as `prev` in the timeline: the same project-level parts and, per track,
 * the same clips and the fields the arrangement reads. A mixer refresh (`patchTracks`) keeps all
 * of them but a mute, so a fader drag never rebuilds the scene.
 */
export function sameTimeline(prev: OpenProject, next: OpenProject): boolean {
  if (prev === next) return true;
  if (
    prev.id !== next.id ||
    prev.tempo !== next.tempo ||
    prev.time_signatures !== next.time_signatures ||
    prev.assets !== next.assets ||
    prev.sample_rate !== next.sample_rate ||
    prev.tracks.length !== next.tracks.length
  ) {
    return false;
  }
  return prev.tracks.every((a, i) => {
    const b = next.tracks[i];
    return (
      a === b ||
      (a.id === b.id &&
        a.clips === b.clips &&
        a.kind === b.kind &&
        a.name === b.name &&
        a.color === b.color &&
        a.mixer.mute === b.mixer.mute)
    );
  });
}

/** A clip with its track. */
export interface PlacedClip {
  readonly track: TrackClips;
  readonly clip: ClipView;
}

export function clipEnd(clip: ClipView): number {
  return clip.start + clip.length;
}

/** `is_compatible`: audio clips go to audio tracks, MIDI clips to instrument tracks. */
export function isCompatible(track: TrackView, clip: ClipView): boolean {
  return clip.type === "audio" ? track.kind === "audio" : track.kind === "instrument";
}

/** The first index whose start is at least `beat` (`bisect_left`). */
function bisectLeft(starts: Float64Array, beat: number): number {
  let lo = 0;
  let hi = starts.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (starts[mid] < beat) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** The first index whose start is past `beat` (`bisect_right`). */
function bisectRight(starts: Float64Array, beat: number): number {
  let lo = 0;
  let hi = starts.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (starts[mid] <= beat) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

export class TimelineModel {
  readonly tracks: readonly TrackClips[];
  readonly tempo: TempoMap;
  private readonly byTrack = new Map<string, TrackClips>();
  private readonly byClip = new Map<string, PlacedClip>();
  /** Clips in the order of their tracks, then by start. */
  readonly clipCount: number;

  constructor(readonly project: OpenProject) {
    this.tempo = new TempoMap(project.tempo);
    let count = 0;
    this.tracks = project.tracks.map((track, index) => {
      const clips = [...track.clips].sort((a, b) => a.start - b.start);
      const entry: TrackClips = {
        track,
        index,
        clips,
        starts: Float64Array.from(clips, (clip) => clip.start),
        longest: clips.reduce((longest, clip) => Math.max(longest, clip.length), 0),
        color: trackColor(index, track.color),
      };
      this.byTrack.set(track.id, entry);
      for (const clip of clips) this.byClip.set(clip.id, { track: entry, clip });
      count += clips.length;
      return entry;
    });
    this.clipCount = count;
  }

  track(trackId: string): TrackClips | undefined {
    return this.byTrack.get(trackId);
  }

  clip(clipId: string): PlacedClip | undefined {
    return this.byClip.get(clipId);
  }

  /** The clips of `track` that overlap `[b0, b1)` (`_visible_clips`), in start order. */
  visibleClips(track: TrackClips, b0: number, b1: number): ClipView[] {
    const lo = bisectLeft(track.starts, b0 - track.longest);
    const hi = bisectRight(track.starts, b1);
    const out: ClipView[] = [];
    for (let i = lo; i < hi; i++) {
      const clip = track.clips[i];
      if (clipEnd(clip) > b0 && clip.start < b1) out.push(clip);
    }
    return out;
  }

  /** The selected clips that still exist, with their tracks, in selection order. */
  selected(clipIds: readonly string[]): PlacedClip[] {
    const out: PlacedClip[] = [];
    for (const id of clipIds) {
      const found = this.byClip.get(id);
      if (found) out.push(found);
    }
    return out;
  }

  /** Every clip's ID, track by track (⌘A). */
  allClipIds(): string[] {
    return this.tracks.flatMap((track) => track.clips.map((clip) => clip.id));
  }
}
