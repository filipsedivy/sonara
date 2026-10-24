// The page's waveform pyramids: asked for by asset, their tiles fetched from the `peaks:` scheme
// once sonara-app says they are ready, and asked for again on `peaks-ready`. The pyramids are
// computed, cached and persisted in Rust (crates/sonara-app/src/peaks.rs); this only gathers them.

import { addTile, decodeTile, type PeakLevel } from "../peaks";
import type { PeaksInfo, PeaksReady } from "../types";

/** What the timeline draws for an audio asset. */
export interface AssetPeaks {
  /** `pending` draws the dashed centre line; `failed` draws nothing. */
  state: PeaksInfo["state"];
  /** "media missing" in red. */
  missing: boolean;
  frames: number;
  sampleRate: number;
  levels: PeakLevel[];
}

export interface PeaksDeps {
  info: (assetId: string) => Promise<PeaksInfo>;
  tile: (assetId: string, sampleRate: number, level: number, index: number) => Promise<ArrayBuffer>;
}

export class PeaksStore {
  private readonly assets = new Map<string, AssetPeaks>();
  private readonly loading = new Set<string>();
  /** Assets whose `peaks-ready` came while they were loading: they load once more. */
  private readonly again = new Set<string>();

  constructor(
    private readonly deps: PeaksDeps,
    /** Called when an asset's peaks changed. */
    private readonly changed: (assetId: string) => void,
    private readonly onError: (error: unknown) => void = () => {},
  ) {}

  /** The asset's peaks as far as they are known; the first call asks for them. */
  get(assetId: string): AssetPeaks | undefined {
    if (!this.assets.has(assetId)) void this.load(assetId);
    return this.assets.get(assetId);
  }

  /** A pyramid became ready or failed: its tiles are fetched again. */
  ready(notice: PeaksReady): void {
    if (this.loading.has(notice.asset_id)) {
      this.again.add(notice.asset_id);
    } else if (this.assets.has(notice.asset_id)) {
      void this.load(notice.asset_id);
    }
  }

  /** Forgets every pyramid (a project was replaced). */
  clear(): void {
    this.assets.clear();
  }

  /** Fetches the info and, once ready, every tile of every level. */
  async load(assetId: string): Promise<void> {
    if (this.loading.has(assetId)) return;
    this.loading.add(assetId);
    try {
      const info = await this.deps.info(assetId);
      const peaks: AssetPeaks = {
        state: info.state,
        missing: info.missing,
        frames: info.frames,
        sampleRate: info.sample_rate,
        levels: [],
      };
      if (info.state === "ready") {
        for (const [level, bins] of info.level_bins.entries()) {
          const tiles = Math.max(1, Math.ceil(bins / info.tile_bins));
          for (let index = 0; index < tiles; index++) {
            const bytes = await this.deps.tile(assetId, info.sample_rate, level, index);
            addTile(peaks.levels, decodeTile(bytes), info.tile_bins);
          }
        }
      }
      this.assets.set(assetId, peaks);
      this.changed(assetId);
    } catch (error) {
      this.onError(error);
    } finally {
      this.loading.delete(assetId);
    }
    if (this.again.delete(assetId)) await this.load(assetId);
  }
}
