import { expect, test } from "@playwright/test";
import {
  TILE_HEADER,
  addTile,
  decodeTile,
  envelope,
  levelFor,
  type PeakLevel,
} from "../../src/lib/peaks";
import { PeaksStore } from "../../src/lib/state/peaks";
import type { PeaksInfo } from "../../src/lib/types";

const BIN = 256;

/** A seeded PRNG (mulberry32), so the test is deterministic (TST-04). */
function random(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Test data: level 0, and levels of four bins each until 64 bins (as sonara-app builds them). */
function pyramid(bins: number, seed: number): PeakLevel[] {
  const next = random(seed);
  const level0 = new Float32Array(bins * 2);
  for (let i = 0; i < bins; i++) {
    const a = next() * 2 - 1;
    const b = next() * 2 - 1;
    level0[2 * i] = Math.min(a, b);
    level0[2 * i + 1] = Math.max(a, b);
  }
  const levels: PeakLevel[] = [{ binFrames: BIN, data: level0 }];
  while (levels[levels.length - 1].data.length / 2 > 64) {
    const below = levels[levels.length - 1];
    const count = below.data.length / 2;
    const data = new Float32Array(Math.ceil(count / 4) * 2);
    for (let i = 0; i < data.length / 2; i++) {
      let low = Infinity;
      let high = -Infinity;
      for (let j = 4 * i; j < Math.min(4 * i + 4, count); j++) {
        low = Math.min(low, below.data[2 * j]);
        high = Math.max(high, below.data[2 * j + 1]);
      }
      data[2 * i] = low;
      data[2 * i + 1] = high;
    }
    levels.push({ binFrames: below.binFrames * 4, data });
  }
  return levels;
}

// `test_waveform_envelope_matches_bins_at_any_zoom`: each column's (min, max) equals the bins it
// covers at the level chosen for the zoom.
test("the envelope matches the bins at any zoom", () => {
  const levels = pyramid(4096, 3);
  for (const fpp of [37, 300, 1100, 5000]) {
    const starts: number[] = [];
    for (let start = 0; start < 4096 * BIN - fpp; start += fpp) starts.push(start);
    const mins = new Float32Array(starts.length);
    const maxs = new Float32Array(starts.length);
    envelope(levels, starts, fpp, mins, maxs);
    const level = levels[levelFor(levels, fpp)];
    const size = level.binFrames;
    for (const k of [0, Math.floor(starts.length / 2), starts.length - 2]) {
      const lo = Math.floor(starts[k] / size);
      const hi = Math.max(lo + 1, Math.floor((starts[k] + fpp) / size));
      let low = Infinity;
      let high = -Infinity;
      for (let bin = lo; bin < hi; bin++) {
        low = Math.min(low, level.data[2 * bin]);
        high = Math.max(high, level.data[2 * bin + 1]);
      }
      expect(maxs[k], `fpp ${fpp}, column ${k}`).toBe(high);
      expect(mins[k], `fpp ${fpp}, column ${k}`).toBe(low);
    }
  }
});

test("the level is the coarsest with a bin per pixel, and columns outside the media are 0", () => {
  const levels = pyramid(4096, 7);
  expect(levels.map((l) => l.binFrames)).toEqual([256, 1024, 4096, 16384]);
  expect(levelFor(levels, 100)).toBe(0);
  expect(levelFor(levels, 1024)).toBe(1);
  expect(levelFor(levels, 5000)).toBe(2);
  expect(levelFor(levels, 1e9)).toBe(3);
  const mins = new Float32Array(2);
  const maxs = new Float32Array(2);
  envelope(levels, [-10_000, 4096 * BIN + 10], 300, mins, maxs);
  expect([...mins, ...maxs]).toEqual([0, 0, 0, 0]);
  envelope([], [0], 300, mins, maxs);
  expect(maxs[0]).toBe(0);
});

/** A tile as sonara-app's `encode_tile` writes it. */
function tile(level: number, index: number, values: number[], levelBins: number, binFrames: number) {
  const bins = values.length / 2;
  const buffer = new ArrayBuffer(TILE_HEADER + bins * 8);
  const view = new DataView(buffer);
  view.setUint16(0, 1, true);
  view.setUint16(2, level, true);
  view.setUint32(4, index, true);
  view.setUint32(8, bins, true);
  view.setUint32(12, levelBins, true);
  view.setUint32(16, binFrames, true);
  view.setUint32(20, 48000, true);
  view.setBigUint64(24, 1000n, true);
  values.forEach((value, i) => view.setFloat32(TILE_HEADER + 4 * i, value, true));
  return buffer;
}

test("tiles decode and assemble into levels", () => {
  const first = decodeTile(tile(0, 0, [-0.5, 0.5, -0.25, 0.25], 3, 256));
  expect(first).toMatchObject({ level: 0, index: 0, bins: 2, levelBins: 3, binFrames: 256 });
  expect(first.frames).toBe(1000);
  const levels: PeakLevel[] = [];
  addTile(levels, first, 2);
  addTile(levels, decodeTile(tile(0, 1, [-1, 1], 3, 256)), 2);
  expect([...levels[0].data]).toEqual([-0.5, 0.5, -0.25, 0.25, -1, 1]);
  expect(() => decodeTile(new ArrayBuffer(8))).toThrow();
  const bad = tile(0, 0, [0, 0], 1, 256);
  new DataView(bad).setUint16(0, 2, true);
  expect(() => decodeTile(bad)).toThrow("version 2");
});

test("the peaks store fetches every tile once ready, and again after peaks-ready", async () => {
  let state: PeaksInfo["state"] = "pending";
  const info = (): PeaksInfo => ({
    state,
    sample_rate: 48000,
    frames: 1000,
    missing: false,
    level_bins: state === "ready" ? [3] : [],
    bin_frames: 256,
    tile_bins: 2,
  });
  const fetched: string[] = [];
  const changed: string[] = [];
  const store = new PeaksStore(
    {
      info: async () => info(),
      tile: async (asset, rate, level, index) => {
        fetched.push(`${asset}/${rate}/${level}/${index}`);
        return index === 0
          ? tile(0, 0, [-0.5, 0.5, -0.25, 0.25], 3, 256)
          : tile(0, 1, [-1, 1], 3, 256);
      },
    },
    (asset) => changed.push(asset),
  );
  const settle = () => new Promise((resolve) => setTimeout(resolve, 10));
  expect(store.get("med_1")).toBeUndefined();
  await settle();
  expect(store.get("med_1")?.state).toBe("pending");
  expect(fetched).toEqual([]);
  state = "ready";
  store.ready({ asset_id: "med_1", sample_rate: 48000, state: "ready" });
  await settle();
  const peaks = store.get("med_1");
  expect(peaks?.state).toBe("ready");
  expect([...(peaks?.levels[0].data ?? [])]).toEqual([-0.5, 0.5, -0.25, 0.25, -1, 1]);
  expect(fetched.slice(0, 2)).toEqual(["med_1/48000/0/0", "med_1/48000/0/1"]);
  expect(changed[0]).toBe("med_1");
  // An asset nobody asked for is not fetched on its notice.
  store.ready({ asset_id: "med_2", sample_rate: 48000, state: "ready" });
  expect(fetched.every((path) => path.startsWith("med_1"))).toBe(true);
});
