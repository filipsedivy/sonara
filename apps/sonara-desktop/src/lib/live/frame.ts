// The live frame's decoder (format 1 of crates/sonara-app/src/live.rs, whose module docs give the
// byte layout). The Rust test writes tests/fixtures/live-frame-v1.json, which the unit test of
// this decoder reads, so both sides share one byte fixture.

export const FRAME_VERSION = 1;
export const HEADER_BYTES = 120;
export const METER_BYTES = 16;
export const DEVICE_BYTES = 4;

const FLAG_TRANSPORT = 1;
const FLAG_AVAILABLE = 2;
const FLAG_RUNNING = 4;
const FLAG_MUSICAL = 8;
const FLAG_DEVICE_STOPPED = 16;

const STATES = ["stopped", "playing", "paused", "recording"] as const;

/** How far the audio engine is. */
export type EngineState = "absent" | "unavailable" | "ready" | "running";
/** The transport state. */
export type LiveState = (typeof STATES)[number];

/** One meter: post-fader peak and RMS, left and right, linear. */
export interface MeterLevels {
  peak: [number, number];
  rms: [number, number];
}

/** One live frame. */
export interface LiveFrame {
  sequence: number;
  engine: EngineState;
  state: LiveState;
  /** The software position (never the hardware's); beat and seconds need a project. */
  position: { frame: number; beat: number | null; seconds: number | null };
  device_stopped: boolean;
  sample_rate: number;
  buffer_frames: number;
  render_load: number;
  callback_load: number;
  /** The backend's estimate, a field of its own (RT-07). */
  output_latency_ms: number;
  lookahead_ms: number;
  queued_ms: number;
  underruns: number;
  device_errors: number;
  version: number;
  meter_layout: number;
  /** The tracks in order, then the master. */
  meters: MeterLevels[];
  /** dB per device, in the tracks' order; NaN for a device that reports none. */
  gain_reduction: number[];
}

/** Whether the transport rolls (playing or recording). */
export function isRolling(frame: LiveFrame): boolean {
  return frame.state === "playing" || frame.state === "recording";
}

function engineOf(flags: number): EngineState {
  if (flags & FLAG_RUNNING) return "running";
  if (flags & FLAG_AVAILABLE) return "ready";
  if (flags & FLAG_TRANSPORT) return "unavailable";
  return "absent";
}

/** Decodes a frame; throws on another format version, an unknown state or a bad length. */
export function decodeLiveFrame(buffer: ArrayBuffer): LiveFrame {
  if (buffer.byteLength < HEADER_BYTES) {
    throw new Error(`a live frame needs ${HEADER_BYTES} bytes, got ${buffer.byteLength}`);
  }
  const view = new DataView(buffer);
  const version = view.getUint16(0, true);
  if (version !== FRAME_VERSION) throw new Error(`unknown live frame version ${version}`);
  const flags = view.getUint16(2, true);
  const stateCode = view.getUint8(4);
  const state = STATES[stateCode];
  if (state === undefined) throw new Error(`unknown transport state ${stateCode}`);
  const meterCount = view.getUint32(112, true);
  const deviceCount = view.getUint32(116, true);
  const expected = HEADER_BYTES + meterCount * METER_BYTES + deviceCount * DEVICE_BYTES;
  if (buffer.byteLength !== expected) {
    throw new Error(
      `a live frame of ${meterCount} meters and ${deviceCount} devices is not ${buffer.byteLength} bytes`,
    );
  }
  const musical = (flags & FLAG_MUSICAL) !== 0;
  const meters: MeterLevels[] = [];
  let at = HEADER_BYTES;
  for (let i = 0; i < meterCount; i++, at += METER_BYTES) {
    meters.push({
      peak: [view.getFloat32(at, true), view.getFloat32(at + 4, true)],
      rms: [view.getFloat32(at + 8, true), view.getFloat32(at + 12, true)],
    });
  }
  const gainReduction: number[] = [];
  for (let i = 0; i < deviceCount; i++, at += DEVICE_BYTES) {
    gainReduction.push(view.getFloat32(at, true));
  }
  return {
    sequence: view.getUint32(8, true),
    engine: engineOf(flags),
    state,
    position: {
      frame: Number(view.getBigInt64(24, true)),
      beat: musical ? view.getFloat64(32, true) : null,
      seconds: musical ? view.getFloat64(40, true) : null,
    },
    device_stopped: (flags & FLAG_DEVICE_STOPPED) !== 0,
    sample_rate: view.getUint32(48, true),
    buffer_frames: view.getUint32(52, true),
    render_load: view.getFloat64(56, true),
    callback_load: view.getFloat64(64, true),
    output_latency_ms: view.getFloat64(72, true),
    lookahead_ms: view.getFloat64(80, true),
    queued_ms: view.getFloat64(88, true),
    underruns: Number(view.getBigUint64(96, true)),
    device_errors: Number(view.getBigUint64(104, true)),
    version: Number(view.getBigUint64(16, true)),
    meter_layout: view.getUint32(12, true),
    meters,
    gain_reduction: gainReduction,
  };
}

/** The frame's meters when they line up with a view of `meterLayout`, else null. */
export function metersFor(frame: LiveFrame, meterLayout: number): MeterLevels[] | null {
  return frame.meter_layout === meterLayout ? frame.meters : null;
}
