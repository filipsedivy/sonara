// The arrangement's instances for the WebGL2 renderer (webgl.ts), built on the CPU when the
// project, the selection, the track height, a drag or the waveforms change; scrolling and
// zooming only change the renderer's uniforms. It keeps the prototype's `ClipPainter` look:
//
// - a clip: its body and 14 px header in the track's (or clip's) colour against the lane, a
//   1 px darker outline or the 2 px selection outline; muted clips greyed and at 70 %, the
//   clips of muted tracks at 55 %, the ghosts of dragged clips at 35 % (`paint`);
// - a MIDI clip's notes, fitted to its pitch range, brighter from velocity 64, and dotted lines
//   at its loop boundaries (`_paint_notes`);
// - an audio clip's waveform from the peak pyramid, through the tempo map, with the clip gain
//   (at most × 4) and its fades; a dashed centre line while the peaks are computed
//   (`_paint_waveform`, `_paint_fades`). "media missing" is text, drawn by overlay.ts.
//
// Positions stay in beats and content pixels (a row's top before the scroll), so the shaders
// place them for any view.

import { playbackNotes } from "../time/playback";
import type { AssetPeaks } from "../state/peaks";
import type { ClipView, OpenProject } from "../types";
import { blend, lighter, parseHex, type Rgb, THEME, unit } from "./colors";
import type { Row, TrackLayout } from "./layout";
import { clipEnd, type PlacedClip, type TimelineModel } from "./model";

/**
 * Floats per clip instance: start and end beat, row top and height; body, header and outline
 * colour (0–1); opacity, outline width; one unused.
 */
export const CLIP_FLOATS = 16;
/**
 * Floats per note instance: start and end beat, row top and height, the clip's drawn start
 * and end beat, the pitch slot from the top (−1 for a loop line) and the slots; colour,
 * opacity, kind (0 a note, 1 a loop line); three unused.
 */
export const NOTE_FLOATS = 16;
/**
 * Floats per waveform piece instance: the piece's start and end beat, the clip's drawn start
 * and end beat, row top and height, the source frame's fraction at the piece's start, frames
 * per beat, gain, the fade-in's end and the fade-out's start beat, the clip's start and end
 * beat (for the fades); colour, the waveform's alpha, opacity; two unused.
 */
export const WAVE_FLOATS = 20;
/**
 * Integers per waveform piece instance: the atlas offset, level 0's bins, the levels, level 0's
 * frames per bin, and the whole source frame at the piece's start (32 bits, which a float
 * cannot hold).
 */
export const WAVE_INTS = 5;

/** No fade (a beat far before any clip). */
export const NO_FADE = -1e9;

const GREY = parseHex("#7a7d84");
const BLACK: Rgb = [0, 0, 0];
const LANE = parseHex(THEME.lane);
const SELECTION = parseHex(THEME.selection);

/** How one clip is placed: its row, its drawn span, and how far its content moved. */
export interface Placement {
  readonly placed: PlacedClip;
  readonly row: Row;
  readonly start: number;
  readonly end: number;
  /** Beats the content is moved by (a move preview); 0 keeps it in place (a trim). */
  readonly shift: number;
  readonly selected: boolean;
  readonly ghost: boolean;
  /** The track is muted (55 %). */
  readonly dimmed: boolean;
}

/** Where each asset's pyramid lies in the peaks atlas. */
export interface AtlasEntry {
  /** The texel of level 0's first bin. */
  readonly offset: number;
  readonly level0Bins: number;
  readonly levels: number;
  readonly level0Frames: number;
}

/** What a clip's waveform needs: the asset's pyramid state and its place in the atlas. */
export interface WaveSource {
  peaks(assetId: string): AssetPeaks | undefined;
  atlas(assetId: string): AtlasEntry | undefined;
}

/** A clip that shows "media missing" (drawn as text). */
export interface MissingMedia {
  readonly clipId: string;
}

/** Growable typed arrays for one kind of instance. */
class Instances {
  floats: Float32Array;
  ints: Uint32Array;
  count = 0;

  constructor(
    readonly floatStride: number,
    readonly intStride = 0,
    capacity = 64,
  ) {
    this.floats = new Float32Array(capacity * floatStride);
    this.ints = new Uint32Array(capacity * Math.max(1, intStride));
  }

  /** Room for one more instance; returns its float offset. */
  next(): number {
    if ((this.count + 1) * this.floatStride > this.floats.length) {
      const grown = new Float32Array(this.floats.length * 2);
      grown.set(this.floats);
      this.floats = grown;
      if (this.intStride > 0) {
        const ints = new Uint32Array(this.ints.length * 2);
        ints.set(this.ints);
        this.ints = ints;
      }
    }
    const at = this.count * this.floatStride;
    this.count += 1;
    return at;
  }

  reset(): void {
    this.count = 0;
  }
}

/** Which instances a rebuild replaces: the clips' bodies, their content, or both. */
export type Parts = "clips" | "content" | "all";

/** The instances of a set of placements. */
export class SceneLayer {
  readonly clips = new Instances(CLIP_FLOATS);
  readonly notes = new Instances(NOTE_FLOATS);
  readonly waves = new Instances(WAVE_FLOATS, WAVE_INTS);
  readonly missing: MissingMedia[] = [];
  /** Grows with every rebuild of the clips, so the renderer uploads them again. */
  clipsVersion = 0;
  /** Grows with every rebuild of the notes and waveforms. */
  contentVersion = 0;

  clear(parts: Parts = "all"): void {
    if (parts !== "content") {
      this.clips.reset();
      this.clipsVersion += 1;
    }
    if (parts !== "clips") {
      this.notes.reset();
      this.waves.reset();
      this.missing.length = 0;
      this.contentVersion += 1;
    }
  }
}

/** The colours `ClipPainter.paint` derives from a clip's base colour. */
export interface ClipColors {
  readonly base: Rgb;
  readonly body: Rgb;
  readonly header: Rgb;
  readonly outline: Rgb;
  readonly opacity: number;
}

export function clipColors(
  clip: ClipView,
  trackColor: Rgb,
  selected: boolean,
  ghost: boolean,
  dimmed: boolean,
): ClipColors {
  let base = clip.color ? parseHex(clip.color) : trackColor;
  if (clip.muted) base = blend(base, GREY, 0.7);
  let opacity = ghost ? 0.35 : clip.muted ? 0.7 : 1;
  if (dimmed) opacity *= 0.55;
  return {
    base,
    body: blend(LANE, base, 0.3),
    header: blend(LANE, base, 0.85),
    outline: selected ? SELECTION : blend(base, BLACK, 0.45),
    opacity,
  };
}

function put(target: Float32Array, at: number, values: readonly number[]): void {
  for (let i = 0; i < values.length; i++) target[at + i] = values[i];
}

/** Python's `//` for the pitch padding. */
function floorDiv(a: number, b: number): number {
  return Math.floor(a / b);
}

/** Adds one placed clip, with its notes or its waveform, to `layer`. */
export function addPlacement(
  layer: SceneLayer,
  project: OpenProject,
  model: TimelineModel,
  waves: WaveSource,
  placement: Placement,
  parts: Parts = "all",
): void {
  const { placed, row, start, end } = placement;
  const clip = placed.clip;
  const colors = clipColors(clip, placed.track.color, placement.selected, placement.ghost, placement.dimmed);
  if (parts !== "clips") {
    if (clip.type === "midi") addNotes(layer, clip, placement, colors);
    else addWaveform(layer, project, model, waves, clip, placement, colors);
  }
  if (parts === "content") return;
  const at = layer.clips.next();
  put(layer.clips.floats, at, [
    start,
    end,
    row.top,
    row.height,
    ...unit(colors.body),
    ...unit(colors.header),
    ...unit(colors.outline),
    colors.opacity,
    placement.selected ? 2 : 1,
    0,
  ]);
}

function addNotes(
  layer: SceneLayer,
  clip: Extract<ClipView, { type: "midi" }>,
  placement: Placement,
  colors: ClipColors,
): void {
  const { row, start, end, shift } = placement;
  const opacity = colors.opacity;
  if (clip.loop_length !== null && clip.loop_length > 0) {
    const loop = clip.loop_length;
    const line = unit(blend(colors.base, BLACK, 0.35));
    const offset = ((clip.content_offset % loop) + loop) % loop;
    let b = clip.start + (loop - offset);
    for (let k = 1; b < clipEnd(clip) - 1e-9 && k < 4096; k++, b += loop) {
      const at = layer.notes.next();
      put(layer.notes.floats, at, [b + shift, b + shift, row.top, row.height, start, end, -1, 1, ...line, opacity, 1, 0, 0]);
    }
  }
  const played = playbackNotes(clip);
  if (played.length === 0) return;
  let lo = 127;
  let hi = 0;
  for (const { note } of played) {
    lo = Math.min(lo, note.pitch);
    hi = Math.max(hi, note.pitch);
  }
  const span = Math.max(12, hi - lo + 1);
  const padLo = lo - floorDiv(span - (hi - lo + 1), 2);
  const loud = unit(lighter(colors.base, 165));
  const dim = unit(lighter(colors.base, 120));
  const clipEndBeat = clipEnd(clip);
  for (const { start: noteStart, note } of played) {
    const noteEnd = Math.min(noteStart + note.length, clipEndBeat);
    const slot = padLo + span - 1 - note.pitch;
    const at = layer.notes.next();
    put(layer.notes.floats, at, [
      noteStart + shift,
      noteEnd + shift,
      row.top,
      row.height,
      start,
      end,
      slot,
      span,
      ...(note.velocity >= 64 ? loud : dim),
      opacity,
      0,
      0,
      0,
    ]);
  }
}

function addWaveform(
  layer: SceneLayer,
  project: OpenProject,
  model: TimelineModel,
  waves: WaveSource,
  clip: Extract<ClipView, { type: "audio" }>,
  placement: Placement,
  colors: ClipColors,
): void {
  const { row, start, end, shift } = placement;
  const asset = project.assets.find((a) => a.id === clip.asset_id);
  const peaks = asset ? waves.peaks(asset.id) : undefined;
  if (!asset || peaks?.missing) {
    layer.missing.push({ clipId: clip.id });
    return;
  }
  if (peaks?.state === "failed") return;
  const atlas = peaks?.state === "ready" ? waves.atlas(asset.id) : undefined;
  const tempo = model.tempo;
  const rate = project.sample_rate;
  const gain = Math.min(4, 10 ** (clip.gain_db / 20));
  const fadeIn =
    clip.fade_in.seconds > 0
      ? tempo.secondsToBeat(tempo.beatToSeconds(clip.start) + clip.fade_in.seconds) + shift
      : NO_FADE;
  const fadeOut =
    clip.fade_out.seconds > 0
      ? tempo.secondsToBeat(tempo.beatToSeconds(clipEnd(clip)) - clip.fade_out.seconds) + shift
      : NO_FADE;
  const color = unit(lighter(colors.base, atlas ? 150 : 140));
  const alpha = atlas ? 230 / 255 : 1;
  const piece = (beat0: number, beat1: number, frame: number, framesPerBeat: number) => {
    const whole = Math.max(0, Math.floor(frame));
    const at = layer.waves.next();
    put(layer.waves.floats, at, [
      beat0,
      beat1,
      start,
      end,
      row.top,
      row.height,
      frame - whole,
      framesPerBeat,
      gain,
      fadeIn,
      fadeOut,
      clip.start + shift,
      clipEnd(clip) + shift,
      ...color,
      alpha,
      colors.opacity,
      0,
      0,
    ]);
    const i = layer.waves.count - 1;
    put32(layer.waves.ints, i * WAVE_INTS, [
      atlas?.offset ?? 0,
      atlas?.level0Bins ?? 0,
      atlas?.levels ?? 0,
      atlas?.level0Frames ?? 256,
      Math.min(whole, 0xffffffff),
    ]);
  };
  if (!atlas) {
    // Pending: one piece over the clip, which draws the dashed centre line (no levels).
    piece(start, end, 0, 0);
    return;
  }
  // The source frame at a beat: the clip's offset (at the project rate) plus the seconds since
  // the clip's start, through the tempo map; linear over each piece of the map.
  const offset = (clip.source_offset * rate) / Math.max(1, asset.sample_rate);
  const t0 = tempo.beatToSeconds(clip.start);
  const from = Math.max(start - shift, clip.start);
  const to = Math.min(end - shift, clipEnd(clip));
  for (const p of tempo.linearPieces(from, to)) {
    piece(p.beat0 + shift, p.beat1 + shift, offset + (p.seconds0 - t0) * rate, p.secondsPerBeat * rate);
  }
}

function put32(target: Uint32Array, at: number, values: readonly number[]): void {
  for (let i = 0; i < values.length; i++) target[at + i] = values[i] >>> 0;
}

/** The static scene: every clip of every track (folders excluded), as drawn when nothing moves. */
export function buildScene(
  layer: SceneLayer,
  model: TimelineModel,
  layout: TrackLayout,
  waves: WaveSource,
  selected: ReadonlySet<string>,
  ghosts: ReadonlySet<string>,
  parts: Parts = "all",
): void {
  layer.clear(parts);
  for (const row of layout.rows) {
    if (row.kind === "folder") continue;
    const track = model.tracks[row.index];
    if (!track || track.track.id !== row.trackId) continue;
    const dimmed = track.track.mixer.mute;
    for (const clip of track.clips) {
      addPlacement(layer, model.project, model, waves, {
        placed: { track, clip },
        row,
        start: clip.start,
        end: clipEnd(clip),
        shift: 0,
        selected: selected.has(clip.id),
        ghost: ghosts.has(clip.id),
        dimmed,
      }, parts);
    }
  }
}
