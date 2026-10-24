import { expect, test } from "@playwright/test";
import type { AssetPeaks } from "../../src/lib/state/peaks";
import type { AssetView } from "../../src/lib/types";
import { PeaksAtlas } from "../../src/lib/timeline/atlas";
import { blend, lighter, parseHex, THEME, trackColor } from "../../src/lib/timeline/colors";
import { TrackLayout } from "../../src/lib/timeline/layout";
import { TimelineModel } from "../../src/lib/timeline/model";
import {
  buildScene,
  CLIP_FLOATS,
  clipColors,
  NO_FADE,
  NOTE_FLOATS,
  SceneLayer,
  WAVE_FLOATS,
  WAVE_INTS,
} from "../../src/lib/timeline/scene";
import { audioClip, midiClip, project, track } from "../support/project";

// The arrangement's instances (scene.ts): the clips with the prototype's colours, the mini notes
// fitted to each clip's pitch range, the loop lines, and the waveform pieces through the tempo map.

const ASSET: AssetView = {
  id: "ast_1",
  kind: "audio",
  name: "tone.wav",
  location: "embedded",
  sample_rate: 48000,
  channels: 2,
  frames: 480000,
  duration_seconds: 10,
};

function ready(levels: number[]): AssetPeaks {
  let binFrames = 256;
  return {
    state: "ready",
    missing: false,
    frames: 480000,
    sampleRate: 48000,
    levels: levels.map((bins) => {
      const level = { binFrames, data: new Float32Array(bins * 2) };
      binFrames *= 4;
      return level;
    }),
  };
}

function scene(open: ReturnType<typeof project>, peaks: Record<string, AssetPeaks | undefined> = {}, selected: string[] = []) {
  const model = new TimelineModel(open);
  const layout = new TrackLayout(open.tracks, 56);
  const atlas = new PeaksAtlas({ get: (id) => peaks[id] });
  atlas.rebuild(open.assets.map((asset) => asset.id));
  const layer = new SceneLayer();
  buildScene(layer, model, layout, atlas, new Set(selected), new Set());
  return { layer, atlas };
}

test("clips carry the prototype's colours, the selection outline and the dimming", () => {
  const base = trackColor(0);
  const lane = parseHex(THEME.lane);
  const plain = clipColors(midiClip("c", 0, 4), base, false, false, false);
  expect(plain.body).toEqual(blend(lane, base, 0.3));
  expect(plain.header).toEqual(blend(lane, base, 0.85));
  expect(plain.outline).toEqual(blend(base, [0, 0, 0], 0.45));
  expect(plain.opacity).toBe(1);
  const selected = clipColors(midiClip("c", 0, 4), base, true, false, false);
  expect(selected.outline).toEqual(parseHex(THEME.selection));
  expect(clipColors(midiClip("c", 0, 4, { muted: true }), base, false, false, true).opacity).toBeCloseTo(0.7 * 0.55, 9);
  expect(clipColors(midiClip("c", 0, 4, { muted: true }), base, false, true, false).opacity).toBe(0.35);
  const { layer } = scene(project([track("t", "Synth", "instrument", [midiClip("a", 0, 4), midiClip("b", 8, 4)])]), {}, ["b"]);
  expect(layer.clips.count).toBe(2);
  const second = layer.clips.floats.subarray(CLIP_FLOATS, 2 * CLIP_FLOATS);
  expect([...second.subarray(0, 4)]).toEqual([8, 12, 0, 56]);
  expect(second[14]).toBe(2);
  // Folders draw no clips.
  const folder = scene(project([track("f", "Group", "folder", [midiClip("x", 0, 4)])]));
  expect(folder.layer.clips.count).toBe(0);
});

test("mini notes fill a 12-semitone range centred on the clip's pitches", () => {
  const { layer } = scene(project([track("t", "Synth", "instrument", [midiClip("a", 2, 4)])]));
  // Pitches 60–63: the span is 12 rows from 56, so pitch 60 sits in row 56 + 11 − 60 = 7.
  expect(layer.notes.count).toBe(4);
  const slots = [0, 1, 2, 3].map((i) => layer.notes.floats[i * NOTE_FLOATS + 6]);
  const spans = [0, 1, 2, 3].map((i) => layer.notes.floats[i * NOTE_FLOATS + 7]);
  expect(slots).toEqual([7, 6, 5, 4]);
  expect(spans).toEqual([12, 12, 12, 12]);
  expect([...layer.notes.floats.subarray(0, 2)]).toEqual([2, 2.5]);
  const loud = lighter(trackColor(0), 165).map((v) => v / 255);
  [0, 1, 2].forEach((c) => expect(layer.notes.floats[8 + c]).toBeCloseTo(loud[c], 6));
});

test("a looped clip repeats its notes and marks its loop boundaries", () => {
  const looped = midiClip("a", 0, 5, { loop_length: 2 });
  const { layer } = scene(project([track("t", "Synth", "instrument", [looped])]));
  const kinds = Array.from({ length: layer.notes.count }, (_, i) => layer.notes.floats[i * NOTE_FLOATS + 12]);
  const lines = kinds.filter((kind) => kind === 1).length;
  expect(lines).toBe(2);
  expect(kinds.length - lines).toBe(10);
});

test("an audio clip's waveform follows the tempo map in linear pieces", () => {
  const take = audioClip("au", "ast_1", 2, 8);
  const open = project([track("v", "Vox", "audio", [take])], {
    assets: [ASSET],
    tempo: [
      { beat: 0, bpm: 120, ramp: false },
      { beat: 6, bpm: 60, ramp: false },
    ],
  });
  const { layer, atlas } = scene(open, { ast_1: ready([1875, 469, 118, 30]) });
  expect(atlas.atlas("ast_1")).toEqual({ offset: 0, level0Bins: 1875, levels: 4, level0Frames: 256 });
  expect(layer.waves.count).toBe(2);
  const piece = (i: number) => layer.waves.floats.subarray(i * WAVE_FLOATS, (i + 1) * WAVE_FLOATS);
  const ints = (i: number) => [...layer.waves.ints.subarray(i * WAVE_INTS, (i + 1) * WAVE_INTS)];
  // Beats 2–6 at 120 BPM: 24,000 frames per beat from frame 0; beats 6–10 at 60 BPM: 48,000
  // from frame 96,000.
  expect([...piece(0).subarray(0, 2), piece(0)[6], piece(0)[7]]).toEqual([2, 6, 0, 24000]);
  expect(ints(0)).toEqual([0, 1875, 4, 256, 0]);
  expect([...piece(1).subarray(0, 2), piece(1)[7]]).toEqual([6, 10, 48000]);
  expect(ints(1)[4]).toBe(96000);
  expect([piece(0)[9], piece(0)[10]]).toEqual([NO_FADE, NO_FADE]);
  // Pending peaks draw one dashed piece; missing media draws text instead.
  const pending = scene(open, { ast_1: { ...ready([]), state: "pending", levels: [] } });
  expect(pending.layer.waves.count).toBe(1);
  expect(pending.layer.waves.ints[2]).toBe(0);
  const missing = scene(open, { ast_1: { ...ready([]), missing: true } });
  expect(missing.layer.waves.count).toBe(0);
  expect(missing.layer.missing).toEqual([{ clipId: "au" }]);
  const unknown = scene(project([track("v", "Vox", "audio", [take])]));
  expect(unknown.layer.missing).toEqual([{ clipId: "au" }]);
});

test("fades become beats through the tempo map, and the gain is capped at × 4", () => {
  const take = {
    ...audioClip("au", "ast_1", 4, 8),
    gain_db: 18,
    fade_in: { seconds: 0.5, curve: "linear" as const },
    fade_out: { seconds: 1, curve: "linear" as const },
  };
  const open = project([track("v", "Vox", "audio", [take])], { assets: [ASSET] });
  const { layer } = scene(open, { ast_1: ready([1875, 469, 118, 30]) });
  const piece = layer.waves.floats.subarray(0, WAVE_FLOATS);
  expect(piece[8]).toBe(4);
  expect(piece[9]).toBeCloseTo(5, 9);
  expect(piece[10]).toBeCloseTo(10, 9);
});
