import { expect, test } from "@playwright/test";
import fixture from "../fixtures/live-frame-v1.json" with { type: "json" };
import { FRAME_VERSION, decodeLiveFrame, isRolling, metersFor } from "../../src/lib/live/frame";
import { FrameQueue, QUEUE_DEPTH, UNDRAWN_MS, waitsForDisplay } from "../../src/lib/live/queue";

// The live frame's decoder against the byte fixture that sonara-app's Rust test writes and checks
// (crates/sonara-app/tests/gui_live.rs): both sides read and write the same bytes.

function bytes(hex: string): ArrayBuffer {
  const pairs = hex.match(/../g) ?? [];
  return new Uint8Array(pairs.map((pair) => parseInt(pair, 16))).buffer;
}

/** The fixture's decoded form, with its null gain reductions as NaN. */
function expected(decoded: (typeof fixture.frames)[number]["decoded"]) {
  return {
    ...decoded,
    gain_reduction: decoded.gain_reduction.map((value) => (value === null ? NaN : value)),
  };
}

test("every fixture frame decodes to the Rust side's values", () => {
  expect(fixture.format).toBe(FRAME_VERSION);
  expect(fixture.frames.map((frame) => frame.name)).toEqual(["playing", "idle", "absent"]);
  for (const frame of fixture.frames) {
    expect(decodeLiveFrame(bytes(frame.hex)), frame.name).toEqual(expected(frame.decoded));
  }
});

test("the playing frame keeps position and latency apart, and its meters follow the layout", () => {
  const playing = decodeLiveFrame(bytes(fixture.frames[0].hex));
  expect(isRolling(playing)).toBe(true);
  expect(playing.position).toEqual({ frame: 96000, beat: 4, seconds: 2 });
  expect(playing.output_latency_ms).toBe(10.67);
  expect(playing.engine).toBe("running");
  expect(metersFor(playing, 0xdeadbeef)).toHaveLength(3);
  expect(metersFor(playing, 1)).toBeNull();
  expect(Number.isNaN(playing.gain_reduction[0])).toBe(true);
  const idle = decodeLiveFrame(bytes(fixture.frames[1].hex));
  expect(isRolling(idle)).toBe(false);
  expect(idle.engine).toBe("unavailable");
  expect(idle.position.beat).toBeNull();
  expect(idle.sequence).toBe(0xffffffff);
});

test("damaged frames are refused", () => {
  const good = new Uint8Array(bytes(fixture.frames[0].hex));
  expect(() => decodeLiveFrame(good.slice(0, 100).buffer)).toThrow("needs 120 bytes");
  expect(() => decodeLiveFrame(good.slice(0, good.length - 1).buffer)).toThrow("is not");
  const version = good.slice();
  version[0] = 9;
  expect(() => decodeLiveFrame(version.buffer)).toThrow("version 9");
  const state = good.slice();
  state[4] = 7;
  expect(() => decodeLiveFrame(state.buffer)).toThrow("state 7");
});

test("the two-frame queue shows each frame once and drops the oldest when full", () => {
  const queue = new FrameQueue<{ sequence: number }>();
  expect(queue.take()).toBeNull();
  for (const sequence of [1, 2, 3]) queue.push({ sequence });
  expect(QUEUE_DEPTH).toBe(2);
  expect(queue.superseded).toBe(1);
  expect(queue.take()).toEqual({ sequence: 2 });
  expect(queue.take()).toEqual({ sequence: 3 });
  expect(queue.take()).toBeNull();
  // A skipped sequence number is a gap; numbers wrap at 2^32.
  queue.push({ sequence: 6 });
  expect(queue.gaps).toBe(2);
  queue.push({ sequence: 0xffffffff });
  queue.push({ sequence: 0 });
  expect(queue.received).toBe(6);
  expect(queue.gaps).toBe(2 + (0xffffffff - 6 - 1));
});

test("a frame waits for the display only while the page is drawn", () => {
  // Visible, with an animation frame a moment ago: the next animation frame shows it.
  expect(waitsForDisplay(false, 0)).toBe(true);
  expect(waitsForDisplay(false, 16.7)).toBe(true);
  expect(waitsForDisplay(false, UNDRAWN_MS - 1)).toBe(true);
  // Hidden, or no animation frame for 250 ms (minimized, another Space, a sleeping display): it
  // is shown when it arrives, so the transport's state follows (stalled play).
  expect(UNDRAWN_MS).toBe(250);
  expect(waitsForDisplay(true, 0)).toBe(false);
  expect(waitsForDisplay(false, UNDRAWN_MS)).toBe(false);
  expect(waitsForDisplay(false, 15_000)).toBe(false);
});
