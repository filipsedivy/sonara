import { expect, test } from "@playwright/test";
import fixture from "../fixtures/live-frame-v1.json" with { type: "json" };
import { decodeLiveFrame, type EngineState, type LiveState } from "../../src/lib/live/frame";
import { clampSize, DEFAULT_PANES, resize, restore, toggle } from "../../src/lib/shell/panes";
import { isTextField, transportShortcut } from "../../src/lib/shell/shortcuts";
import {
  engineLabel,
  parseSignature,
  pyStrRepr,
  TransportControl,
  type TransportDeps,
} from "../../src/lib/transport/transport";
import type { Edit, OpenProject } from "../../src/lib/types";
import { project } from "../support/project";

// The transport bar's behaviour (the prototype has no tests of it) and the window's
// shortcuts and panes, against fakes of the backend.

class Fake {
  calls: unknown[][] = [];
  state: LiveState = "stopped";
  engine: EngineState = "absent";
  open: OpenProject | null = project([]);
  playhead = 0;
  recording = false;
  failures: unknown[] = [];
  refuse: string | null = null;

  deps(): TransportDeps {
    const call =
      (name: string) =>
      async (...args: unknown[]) => {
        this.calls.push([name, ...args]);
        if (this.refuse === name) throw { code: "ENGINE_UNAVAILABLE", message: "audio backend disabled (null backend)" };
        return name === "stop"
          ? { was_playing: true, takes: null, message: null, status: {} }
          : { state: name === "play" ? "playing" : "stopped" };
      };
    return {
      play: call("play") as TransportDeps["play"],
      stop: call("stop") as TransportDeps["stop"],
      record: call("record") as TransportDeps["record"],
      seek: call("seek") as TransportDeps["seek"],
      edit: call("edit") as TransportDeps["edit"],
      transport: () => ({ state: this.state, engine: this.engine }),
      project: () => this.open,
      playhead: () => this.playhead,
      setPlayhead: (beat) => (this.playhead = beat),
      fail: (error) => this.failures.push(error),
      answered: (state) => (this.state = state),
      setRecording: (checked) => (this.recording = checked),
    };
  }
}

test("Play starts from the playhead with the loop; a second press stops; Stop returns to the start", async () => {
  const fake = new Fake();
  const control = new TransportControl(fake.deps());
  fake.playhead = 6;
  await control.togglePlay();
  expect(fake.calls).toEqual([["play", 6, false]]);
  expect(fake.state).toBe("playing");
  fake.playhead = 9;
  await control.togglePlay();
  expect(fake.calls[1]).toEqual(["stop"]);
  expect(fake.state).toBe("stopped");
  // A second stop puts the playhead back at the last start and seeks there.
  await control.stop();
  expect(fake.playhead).toBe(6);
  expect(fake.calls[2]).toEqual(["seek", 6]);
  // The loop follows the project's Loop.
  fake.open = project([], { transport: { ...project([]).transport, loop_enabled: true } });
  await control.togglePlay();
  expect(fake.calls[3]).toEqual(["play", 6, true]);
});

test("Home seeks only once the transport exists; refusals reach the status bar", async () => {
  const fake = new Fake();
  const control = new TransportControl(fake.deps());
  fake.playhead = 12;
  await control.returnToStart();
  expect(fake.playhead).toBe(0);
  expect(fake.calls).toEqual([]);
  fake.engine = "unavailable";
  fake.refuse = "seek";
  await control.returnToStart();
  expect(fake.calls).toEqual([["seek", 0]]);
  expect(fake.failures).toEqual([{ code: "ENGINE_UNAVAILABLE", message: "audio backend disabled (null backend)" }]);
});

test("Record checks, records from the playhead, unchecks on failure and stops when unchecked", async () => {
  const fake = new Fake();
  const control = new TransportControl(fake.deps());
  fake.playhead = 4;
  await control.toggleRecord(true);
  expect(fake.calls).toEqual([["record", 4]]);
  expect([fake.recording, fake.state]).toEqual([true, "recording"]);
  await control.toggleRecord(false);
  expect(fake.calls[1]).toEqual(["stop"]);
  expect(fake.recording).toBe(false);
  fake.refuse = "record";
  await control.toggleRecord(true);
  expect(fake.recording).toBe(false);
  expect(fake.failures).toHaveLength(1);
});

test("loop, metronome and count-in edit only on a change; tempo and signature are validated", async () => {
  const fake = new Fake();
  const control = new TransportControl(fake.deps());
  await control.setTransport("loop_enabled", false);
  await control.setTransport("loop_enabled", true);
  await control.setTransport("metronome", true);
  await control.setTransport("count_in_bars", 2);
  await control.setTempo(120);
  await control.setTempo(1500);
  await control.setTempo(97.456);
  expect(await control.setSignature(" 3/4 ")).toBe(true);
  expect(await control.setSignature("4/4")).toBe(true);
  expect(await control.setSignature("4")).toBe(false);
  const edits = fake.calls.map((call) => call[1] as Edit);
  expect(edits).toEqual([
    { op: "set_transport", loop_enabled: true },
    { op: "set_transport", metronome: true },
    { op: "set_transport", count_in_bars: 2 },
    { op: "set_tempo", bpm: 999 },
    { op: "set_tempo", bpm: 97.46 },
    { op: "set_time_signature", numerator: 3, denominator: 4 },
  ]);
  expect(fake.failures).toEqual(["invalid time signature '4'"]);
  expect(parseSignature("x/4")).toBeNull();
  expect(parseSignature("7/8")).toEqual([7, 8]);
  expect(pyStrRepr("it's")).toBe(`"it's"`);
});

test("without a project the bar does nothing but Home", async () => {
  const fake = new Fake();
  fake.open = null;
  const control = new TransportControl(fake.deps());
  await control.togglePlay();
  await control.toggleRecord(true);
  await control.setTempo(100);
  expect(fake.calls).toEqual([]);
  expect(fake.recording).toBe(false);
});

test("the engine label words the frame, and the reason when unavailable", () => {
  const frames = Object.fromEntries(
    fixture.frames.map((f) => {
      const hex = f.hex.match(/../g)?.map((pair) => parseInt(pair, 16)) ?? [];
      return [f.name, decodeLiveFrame(new Uint8Array(hex).buffer)];
    }),
  );
  const playing = Object.values(frames).find((f) => f.state === "playing");
  expect(playing).toBeDefined();
  if (!playing) return;
  const label = engineLabel(playing, null);
  expect(label?.text).toMatch(/^\d+(\.\d+)? kHz · \d+ · render \d+%( · \d+ dropouts)?$/);
  expect(label?.title).toMatch(/^Output latency estimate [\d.]+ ms, lookahead [\d.]+ ms$/);
  expect(engineLabel({ ...playing, sample_rate: 44100, buffer_frames: 512, render_load: 0.125, underruns: 0 }, null)?.text).toBe(
    "44.1 kHz · 512 · render 12%",
  );
  expect(engineLabel({ ...playing, sample_rate: 48000, underruns: 3, render_load: 0.135 }, null)?.text).toBe(
    `48 kHz · ${playing.buffer_frames} · render 14% · 3 dropouts`,
  );
  expect(engineLabel({ ...playing, output_latency_ms: 16, lookahead_ms: 80 }, null)?.title).toBe(
    "Output latency estimate 16.0 ms, lookahead 80.0 ms",
  );
  const unavailable = { ...playing, engine: "unavailable" as const };
  expect(engineLabel(unavailable, { available: false, reason: "no device" })).toEqual({
    text: "Audio engine unavailable",
    title: "no device",
  });
  expect(engineLabel({ ...playing, engine: "absent" }, null)).toBeNull();
  expect(engineLabel(null, null)).toBeNull();
});

test("single-key shortcuts never fire in a text field or with a modifier", () => {
  const key = (k: string, extra: Partial<{ metaKey: boolean; ctrlKey: boolean; altKey: boolean }> = {}) =>
    transportShortcut({ key: k, metaKey: false, ctrlKey: false, altKey: false, target: null, ...extra });
  expect([key(" "), key("Home"), key("r"), key("L"), key("c")]).toEqual(["play", "home", "record", "loop", "metronome"]);
  expect(key("r", { metaKey: true })).toBeNull();
  expect(key("x")).toBeNull();
  const field = { tagName: "INPUT", type: "text" } as unknown as EventTarget;
  const checkbox = { tagName: "INPUT", type: "checkbox" } as unknown as EventTarget;
  const button = { tagName: "BUTTON" } as unknown as EventTarget;
  expect(transportShortcut({ key: " ", metaKey: false, ctrlKey: false, altKey: false, target: field })).toBeNull();
  expect(transportShortcut({ key: " ", metaKey: false, ctrlKey: false, altKey: false, target: button })).toBe("play");
  expect([isTextField(null), isTextField(checkbox), isTextField({ tagName: "TEXTAREA" } as unknown as EventTarget)]).toEqual([
    false,
    false,
    true,
  ]);
});

test("panes keep their limits and leave the timeline room", () => {
  expect(clampSize("browser", 50)).toBe(160);
  expect(clampSize("browser", 2000)).toBe(640);
  expect(clampSize("inspector", 500, 700)).toBe(380);
  expect(resize(DEFAULT_PANES, "editors", 400).editors).toEqual({ size: 400, open: true });
  expect(toggle(DEFAULT_PANES, "browser").browser.open).toBe(false);
  expect(restore(null)).toEqual(DEFAULT_PANES);
  expect(restore({ ...DEFAULT_PANES, editors: { size: 10, open: false } }).editors).toEqual({ size: 160, open: false });
});
