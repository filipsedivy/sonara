// The waveform pyramids of the open project packed for the GPU: every ready asset's levels, one
// after another, as (min, max) pairs; each asset's place is an `AtlasEntry`. Level k + 1 follows
// level k and holds ceil(bins / 4) bins, as sonara-app's `build_levels` makes them, so the
// shader finds every level from level 0's offset and bins. Rebuilt when a pyramid changes.

import type { AssetPeaks } from "../state/peaks";
import type { AtlasEntry, WaveSource } from "./scene";

/** The pyramids the atlas reads (the session's `PeaksStore`). */
export interface PeaksSource {
  get(assetId: string): AssetPeaks | undefined;
}

export class PeaksAtlas implements WaveSource {
  /** The (min, max) pairs of every packed level. */
  data = new Float32Array(0);
  /** The pairs used. */
  texels = 0;
  /** Grows with every rebuild, so the renderer uploads again. */
  version = 0;
  private entries = new Map<string, AtlasEntry>();

  constructor(private readonly source: PeaksSource) {}

  peaks(assetId: string): AssetPeaks | undefined {
    return this.source.get(assetId);
  }

  atlas(assetId: string): AtlasEntry | undefined {
    return this.entries.get(assetId);
  }

  /** Packs the ready pyramids of `assetIds` (asking for the others). */
  rebuild(assetIds: readonly string[]): void {
    const ready: { id: string; peaks: AssetPeaks }[] = [];
    let total = 0;
    for (const id of assetIds) {
      const peaks = this.source.get(id);
      if (!peaks || peaks.state !== "ready" || peaks.levels.length === 0) continue;
      if (!levelsChain(peaks)) continue;
      ready.push({ id, peaks });
      for (const level of peaks.levels) total += level.data.length / 2;
    }
    const data = new Float32Array(Math.max(1, total) * 2);
    const entries = new Map<string, AtlasEntry>();
    let at = 0;
    for (const { id, peaks } of ready) {
      entries.set(id, {
        offset: at,
        level0Bins: peaks.levels[0].data.length / 2,
        levels: peaks.levels.length,
        level0Frames: peaks.levels[0].binFrames,
      });
      for (const level of peaks.levels) {
        data.set(level.data, at * 2);
        at += level.data.length / 2;
      }
    }
    this.data = data;
    this.texels = at;
    this.entries = entries;
    this.version += 1;
  }
}

/** Whether each level holds ceil(bins / 4) of the one before, as the shader assumes. */
function levelsChain(peaks: AssetPeaks): boolean {
  for (let i = 1; i < peaks.levels.length; i++) {
    const below = peaks.levels[i - 1].data.length / 2;
    if (peaks.levels[i].data.length / 2 !== Math.ceil(below / 4)) return false;
    if (peaks.levels[i].binFrames !== peaks.levels[i - 1].binFrames * 4) return false;
  }
  return true;
}
