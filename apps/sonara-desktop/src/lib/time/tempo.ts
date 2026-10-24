// The tempo map for display (sonara-core's `TempoMap`, crates/sonara-core/src/time.rs): beats to
// seconds and back, for the transport bar's time, the waveforms' frames and the fades. The
// arithmetic is sonara-core's line by line; tests/unit/time.test.ts checks it against the values
// the Rust test of crates/sonara-app/tests/gui_window.rs writes (tests/fixtures/time-v1.json).
//
// DES-01: the page reads positions it shows at 60 frames a second and maps every waveform pixel
// through this map, so asking the backend for each would not do; the shared fixture keeps the
// two implementations equal.

import type { TempoView } from "../types";

const SLOPE_EPSILON = 1e-12;

/** A piece of the map over which seconds grow linearly with beats. */
export interface LinearPiece {
  /** The first beat of the piece. */
  beat0: number;
  /** The beat after its end. */
  beat1: number;
  /** Seconds at `beat0`. */
  seconds0: number;
  /** Seconds per beat over the piece. */
  secondsPerBeat: number;
}

export class TempoMap {
  readonly points: readonly TempoView[];
  private readonly seconds: number[];

  constructor(points: readonly TempoView[]) {
    this.points = points.length > 0 ? points : [{ beat: 0, bpm: 120, ramp: false }];
    this.seconds = [0];
    for (let i = 1; i < this.points.length; i++) {
      this.seconds.push(this.seconds[i - 1] + this.segmentSeconds(i - 1, this.points[i].beat));
    }
  }

  /** BPM change per beat within segment `i` (0 for constant segments and the last one). */
  private slope(i: number): number {
    const p = this.points[i];
    const next = this.points[i + 1];
    return next && p.ramp ? (next.bpm - p.bpm) / (next.beat - p.beat) : 0;
  }

  /** Seconds from the start of segment `i` to `beat`. */
  private segmentSeconds(i: number, beat: number): number {
    const p = this.points[i];
    const d = beat - p.beat;
    const k = this.slope(i);
    if (Math.abs(k) < SLOPE_EPSILON) return (60 * d) / p.bpm;
    return (60 / k) * Math.log((p.bpm + k * d) / p.bpm);
  }

  private segmentBeats(i: number, seconds: number): number {
    const p = this.points[i];
    const k = this.slope(i);
    if (Math.abs(k) < SLOPE_EPSILON) return (seconds * p.bpm) / 60;
    return (p.bpm * Math.exp((k * seconds) / 60) - p.bpm) / k;
  }

  /** The last point at or before `beat`. */
  private segmentIndex(beat: number): number {
    let lo = 0;
    let hi = this.points.length - 1;
    while (lo < hi) {
      const mid = Math.ceil((lo + hi) / 2);
      if (this.points[mid].beat <= beat) lo = mid;
      else hi = mid - 1;
    }
    return lo;
  }

  /** The base tempo (the transport bar's BPM). */
  get baseBpm(): number {
    return this.points[0].bpm;
  }

  /** Whether the map changes tempo anywhere (the timeline's "BPM*"). */
  get hasChanges(): boolean {
    return this.points.length > 1;
  }

  /** Tempo at `beat` (negative beats use the first point). */
  bpmAt(beat: number): number {
    const b = Math.max(beat, 0);
    const i = this.segmentIndex(b);
    return this.points[i].bpm + this.slope(i) * (b - this.points[i].beat);
  }

  /** Seconds at `beat`; negative beats extrapolate the first tempo. */
  beatToSeconds(beat: number): number {
    if (beat < 0) return (60 * beat) / this.points[0].bpm;
    const i = this.segmentIndex(beat);
    return this.seconds[i] + this.segmentSeconds(i, beat);
  }

  /** Beat at `seconds`; negative times extrapolate the first tempo. */
  secondsToBeat(seconds: number): number {
    if (seconds < 0) return (seconds * this.points[0].bpm) / 60;
    let lo = 0;
    let hi = this.seconds.length - 1;
    while (lo < hi) {
      const mid = Math.ceil((lo + hi) / 2);
      if (this.seconds[mid] <= seconds) lo = mid;
      else hi = mid - 1;
    }
    return this.points[lo].beat + this.segmentBeats(lo, seconds - this.seconds[lo]);
  }

  /**
   * The pieces of `[beat0, beat1)` over which seconds are linear in beats: a constant segment
   * is one piece, a ramp is cut into pieces of at most `rampStep` beats whose ends lie on the
   * map (a waveform drawn through them is off by less than the ramp's curvature over one piece).
   */
  linearPieces(beat0: number, beat1: number, rampStep = 0.25): LinearPiece[] {
    const pieces: LinearPiece[] = [];
    if (!(beat1 > beat0)) return pieces;
    let b = beat0;
    while (b < beat1) {
      const i = b < 0 ? 0 : this.segmentIndex(b);
      const next = this.points[i + 1];
      const segmentEnd = b < 0 ? Math.min(0, beat1) : next ? Math.min(next.beat, beat1) : beat1;
      const ramp = b >= 0 && Math.abs(this.slope(i)) >= SLOPE_EPSILON;
      const end = ramp ? Math.min(segmentEnd, b + rampStep) : segmentEnd;
      const s0 = this.beatToSeconds(b);
      const s1 = this.beatToSeconds(end);
      pieces.push({ beat0: b, beat1: end, seconds0: s0, secondsPerBeat: (s1 - s0) / (end - b) });
      if (pieces.length > 100_000 || end <= b) break;
      b = end;
    }
    return pieces;
  }
}
