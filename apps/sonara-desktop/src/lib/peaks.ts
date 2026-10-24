// Waveform peaks on the page: the tiles of the `peaks:` scheme (crates/sonara-app/src/peaks.rs,
// `encode_tile`) assembled into levels, and the prototype's reduction of a level to one
// (min, max) pair per pixel column (`PeakPyramid.level_for` and `envelope` in
// sonara/ui/timeline/waveforms.py). The pyramid itself is computed in Rust; this only reads it.

export const TILE_VERSION = 1;
export const TILE_HEADER = 32;

/** One tile of one level. */
export interface PeakTile {
  level: number;
  index: number;
  /** Bins in this tile. */
  bins: number;
  /** Bins in the whole level. */
  levelBins: number;
  /** Frames per bin of the level. */
  binFrames: number;
  sampleRate: number;
  frames: number;
  /** The bins' minimum and maximum, interleaved. */
  data: Float32Array;
}

/** A whole level: its frames per bin, and min/max pairs interleaved. */
export interface PeakLevel {
  binFrames: number;
  data: Float32Array;
}

/** Decodes a tile; throws on another version or a bad length. */
export function decodeTile(buffer: ArrayBuffer): PeakTile {
  if (buffer.byteLength < TILE_HEADER) throw new Error("a peaks tile needs its header");
  const view = new DataView(buffer);
  const version = view.getUint16(0, true);
  if (version !== TILE_VERSION) throw new Error(`unknown peaks tile version ${version}`);
  const bins = view.getUint32(8, true);
  if (buffer.byteLength !== TILE_HEADER + bins * 8) throw new Error("a truncated peaks tile");
  const data = new Float32Array(bins * 2);
  for (let i = 0; i < bins * 2; i++) data[i] = view.getFloat32(TILE_HEADER + i * 4, true);
  return {
    level: view.getUint16(2, true),
    index: view.getUint32(4, true),
    bins,
    levelBins: view.getUint32(12, true),
    binFrames: view.getUint32(16, true),
    sampleRate: view.getUint32(20, true),
    frames: Number(view.getBigUint64(24, true)),
    data,
  };
}

/** Places `tile` into its level, made with `levelBins` bins when it does not exist yet. */
export function addTile(levels: PeakLevel[], tile: PeakTile, tileBins: number): void {
  let level = levels[tile.level];
  if (!level) {
    level = { binFrames: tile.binFrames, data: new Float32Array(tile.levelBins * 2) };
    levels[tile.level] = level;
  }
  level.data.set(tile.data, tile.index * tileBins * 2);
}

/** The coarsest level that still has at least one bin per pixel (`level_for`). */
export function levelFor(levels: PeakLevel[], framesPerPixel: number): number {
  let level = 0;
  while (level + 1 < levels.length && levels[level + 1].binFrames <= framesPerPixel) level += 1;
  return level;
}

/**
 * The (min, max) of each pixel column that starts at `frameStarts[i]` (source frames) and spans
 * `framesPerPixel` frames, written into `mins` and `maxs` (`envelope`): the bins from
 * `floor(start / size)` to `max(lo + 1, floor((start + fpp) / size))` of the level of
 * {@link levelFor}; 0 for a column outside the media.
 */
export function envelope(
  levels: PeakLevel[],
  frameStarts: ArrayLike<number>,
  framesPerPixel: number,
  mins: Float32Array,
  maxs: Float32Array,
): void {
  mins.fill(0);
  maxs.fill(0);
  const level = levels[levelFor(levels, framesPerPixel)];
  if (!level) return;
  const { data, binFrames: size } = level;
  const n = data.length / 2;
  if (n === 0) return;
  for (let column = 0; column < frameStarts.length; column++) {
    const start = frameStarts[column];
    const lo = Math.floor(start / size);
    const hi = Math.max(lo + 1, Math.floor((start + framesPerPixel) / size));
    if (hi <= 0 || lo >= n) continue;
    const from = Math.min(Math.max(lo, 0), n - 1);
    const to = Math.min(Math.max(hi, 1), n);
    let low = Infinity;
    let high = -Infinity;
    for (let bin = from; bin < to; bin++) {
      low = Math.min(low, data[2 * bin]);
      high = Math.max(high, data[2 * bin + 1]);
    }
    mins[column] = low;
    maxs[column] = high;
  }
}
