// Live frames for the page tests, packed as sonara-app's `live.rs` packs format 1 (its module
// docs give the layout; the shared fixture tests/fixtures/live-frame-v1.json pins the decoder).
// Only the fields the tests vary are parameters.

export interface FrameSpec {
  sequence?: number;
  /** 0 stopped, 1 playing. */
  state?: number;
  meterLayout: number;
  /** Per meter: peak left, peak right, RMS left, RMS right (linear). */
  meters: [number, number, number, number][];
  /** The position in beats (0 by default). */
  beat?: number;
}

/** The frame's bytes as hex, for `sendChannel`. */
export function frameHex(spec: FrameSpec): string {
  const bytes = new ArrayBuffer(120 + spec.meters.length * 16);
  const view = new DataView(bytes);
  view.setUint16(0, 1, true);
  // The transport exists, a backend is usable, a player runs, a musical position.
  view.setUint16(2, 1 | 2 | 4 | 8, true);
  view.setUint8(4, spec.state ?? 1);
  view.setUint32(8, spec.sequence ?? 1, true);
  view.setUint32(12, spec.meterLayout, true);
  view.setBigUint64(16, 1n, true);
  view.setFloat64(32, spec.beat ?? 0, true);
  view.setFloat64(40, 0, true);
  view.setUint32(48, 48000, true);
  view.setUint32(52, 256, true);
  view.setUint32(112, spec.meters.length, true);
  view.setUint32(116, 0, true);
  spec.meters.forEach((meter, i) => {
    meter.forEach((value, j) => view.setFloat32(120 + i * 16 + j * 4, value, true));
  });
  return [...new Uint8Array(bytes)].map((b) => b.toString(16).padStart(2, "0")).join("");
}
