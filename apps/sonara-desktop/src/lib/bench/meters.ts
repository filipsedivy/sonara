// The meter benchmark (ADR 0009's meter threshold: at least 58 fps, less than
// 1 % dropped frames and a p99 latency from the Rust sample to the pixel of at most 50 ms), run in
// the product's page on the real engine. It follows the ADR 0009 spike's meter suite
// (`recordMeters` and `analyzeMeters`) adapted to the product:
//
// - the project plays, looped, on the simulated device (`native-null`), so the live frames carry
//   the engine's own meters at 60 Hz (sonara-app `LiveFeed`), four values per meter;
// - the page shows the Mixer tab, and every strip's meter is drawn at each animation frame from
//   the frame shown there (the session's two-frame queue and `MeterBank`);
// - the Rust side stamps each frame when the live channel sends it (src-tauri/src/bench.rs); a
//   round-trip handshake maps that clock onto performance.now(). The latency is the start of the
//   animation frame after the draw that showed a frame, minus its stamp. The cost of building the
//   frame before the stamp is measured on its own (`bench_frame_cost`);
// - "dropped" counts the frames the channel sent in the segment that were never drawn;
// - four segments: the meters alone; the meters while a fader drag on the first strip sends an
//   edit per display frame (the gesture protocol), each followed by a refresh of the project view;
//   and a knob drag on the first synthesizer's card in the Devices tab, likewise, beside a
//   compressor's card whose gain reduction the frames update (the meters then only count the frames
//   applied), first with the refresh of its device alone, then with whole views to compare. The
// fader drag is the measurement behind remark that a per-track delta of the
//   view may be needed (PERF-01): it led to the track refresh (state/project.ts) and to the strips'
//   rendering containment (ChannelStrip.svelte); the knob drags led to the device refresh;
// - each segment also reports the script time of the session's animation frame, the cost of
//   applying a view and the layout it forces, and the lag of the application's main thread, which
//   carries the IPC and the channel (src-tauri/src/bench.rs `bench_main_lag`).
//
// A segment during which the page was hidden or no frame came for 250 ms is measured again
// (segments.ts). The window runs it when the desktop was built with the `bench` feature and
// started with SONARA_BENCH_OUT and SONARA_BENCH_SUITE=meters; scripts/bench-timeline.mjs
// (`--suite meters`) drives the runs.

import { invoke } from "@tauri-apps/api/core";
import * as backend from "../backend";
import type { BenchOptions } from "../backend";
import type { Session } from "../session.svelte";
import { benchHooks } from "./hooks";
import { Disturbances, nextFrame, retrying, waitForFrames, type Validity } from "./segments";
import { type Summary, rounded, sleep, summarize } from "./stats";

const r = (s: Summary) => rounded(s);

/** The spike's handshake (`syncClock`): the Rust clock minus the page's, from the best round. */
interface ClockSync {
  offsetMs: number;
  rttMs: number;
  errorBoundMs: number;
  rttMedianMs: number;
}

async function syncClock(rounds = 80): Promise<ClockSync> {
  for (let i = 0; i < 5; i++) await invoke<number>("bench_clock");
  let best = { rtt: Number.POSITIVE_INFINITY, offset: 0 };
  const rtts: number[] = [];
  for (let i = 0; i < rounds; i++) {
    const t0 = performance.now();
    const rust = await invoke<number>("bench_clock");
    const t1 = performance.now();
    const rtt = t1 - t0;
    rtts.push(rtt);
    if (rtt < best.rtt) best = { rtt, offset: rust - (t0 + t1) / 2 };
  }
  rtts.sort((a, b) => a - b);
  return { offsetMs: best.offset, rttMs: best.rtt, errorBoundMs: best.rtt / 2, rttMedianMs: rtts[Math.floor(rtts.length / 2)] };
}

interface Sample {
  sequence: number;
  receivedMs: number;
  drawnMs: number;
  /** The start of the animation frame after the draw ("pixel" time). */
  presentMs: number;
}

/** A fader drag driven by pointer events at every animation frame, `t` seconds into it. */
type Drive = (t: number) => void;

interface Segment {
  validity: Validity;
  [key: string]: unknown;
}

async function recordMeters(
  session: Session,
  disturbances: Disturbances,
  durationMs: number,
  drive?: Drive,
  editLabel = "Track Volume",
  watched: HTMLElement | null = null,
): Promise<Segment> {
  const clock = await syncClock();
  await invoke("bench_live_stamps");
  const samples = new Map<number, Sample>();
  const frameTimes: number[] = [];
  let edits = 0;
  const counting = await backend.onSessionChanged((notice) => {
    if (notice.label === editLabel) edits += 1;
  });
  const updates: number[] = [];
  const layouts: number[] = [];
  const work: number[] = [];
  benchHooks.frameWork = (ms) => work.push(ms);
  benchHooks.viewShown = (updateMs, layoutMs) => {
    updates.push(updateMs);
    layouts.push(layoutMs);
  };
  benchHooks.liveReceived = (sequence, receivedMs) =>
    samples.set(sequence, { sequence, receivedMs, drawnMs: Number.NaN, presentMs: Number.NaN });
  benchHooks.meterShown = (sequence, drawnMs) => {
    const sample = samples.get(sequence);
    if (!sample || !Number.isNaN(sample.drawnMs)) return;
    sample.drawnMs = drawnMs;
    requestAnimationFrame((now) => (sample.presentMs = now));
  };
  let stop = false;
  let origin = 0;
  const dbField = document.querySelector<HTMLInputElement>('.mixer .strips input[aria-label="Volume dB"]');
  const capElement = document.querySelector<HTMLElement>('.mixer .strips [role="slider"][aria-label$=" volume"] .cap');
  let lastDb = dbField?.value;
  let lastCap = capElement?.style.transform;
  let dbChanges = 0;
  let capChanges = 0;
  let lastWatched = watched?.getAttribute("aria-valuenow");
  let watchedChanges = 0;
  const loop = (now: number) => {
    if (stop) return;
    if (watched && watched.getAttribute("aria-valuenow") !== lastWatched) {
      watchedChanges += 1;
      lastWatched = watched.getAttribute("aria-valuenow");
    }
    if (dbField && dbField.value !== lastDb) {
      dbChanges += 1;
      lastDb = dbField.value;
    }
    if (capElement && capElement.style.transform !== lastCap) {
      capChanges += 1;
      lastCap = capElement.style.transform;
    }
    frameTimes.push(now);
    if (!origin) origin = now;
    drive?.((now - origin) / 1000);
    requestAnimationFrame(loop);
  };
  requestAnimationFrame(loop);
  const views = session.viewRequests;
  const superseded = session.queue.superseded;
  const underruns = session.frame?.underruns ?? 0;
  const end = disturbances.begin(frameTimes);
  const started = performance.now();
  const lag = invoke<number[]>("bench_main_lag", { durationMs: durationMs - 200 });
  await sleep(durationMs);
  const validity = end();
  const ended = performance.now();
  stop = true;
  benchHooks.liveReceived = null;
  benchHooks.viewShown = null;
  benchHooks.frameWork = null;
  await nextFrame();
  await nextFrame();
  benchHooks.meterShown = null;
  counting();
  const mainLagMs = r(summarize(await lag));
  const stamps = await invoke<[number, number][]>("bench_live_stamps");
  const clockAfter = await syncClock();
  const offset = (clock.offsetMs + clockAfter.offsetMs) / 2;
  // The frames the channel sent within the segment; the last 50 ms may still wait for a draw.
  const sent = stamps.filter(([, rust]) => rust - offset >= started && rust - offset <= ended - 50);
  if (sent.length < 2) return { validity, error: "no live frames sent in the segment" };
  const page = (rust: number) => rust - offset;
  const received = sent.map(([sequence]) => samples.get(sequence)).filter((s): s is Sample => s !== undefined);
  const shown = sent.flatMap(([sequence, rust]) => {
    const sample = samples.get(sequence);
    return sample && !Number.isNaN(sample.presentMs) ? [{ sample, rust }] : [];
  });
  const seconds = (page(sent[sent.length - 1][1]) - page(sent[0][1])) / 1000;
  const intervals = frameTimes.slice(1).map((t, i) => t - frameTimes[i]);
  const rustIntervals = sent.slice(1).map(([, rust], i) => rust - sent[i][1]);
  const lastFrame = session.frame;
  const transport = await backend.transportStatus();
  return {
    // The transport at the segment's end: a feed at 4 Hz means it stopped rolling.
    end: {
      state: lastFrame?.state,
      beat: lastFrame?.position.beat,
      deviceStopped: lastFrame?.device_stopped,
      status: transport ? { state: transport.state, position_beat: transport.position_beat } : null,
    },
    validity,
    seconds: Math.round(seconds * 100) / 100,
    sent: sent.length,
    received: received.length,
    shown: shown.length,
    meterFps: Math.round((shown.length / seconds) * 100) / 100,
    droppedPercent: Math.round(((sent.length - shown.length) / sent.length) * 10000) / 100,
    transportGaps: sent.length - received.length,
    superseded: session.queue.superseded - superseded,
    latencyToPixelMs: r(summarize(shown.map(({ sample, rust }) => sample.presentMs - page(rust)))),
    rustToReceiveMs: r(summarize(received.map((s) => s.receivedMs - page(stamps.find(([q]) => q === s.sequence)?.[1] ?? 0)))),
    receiveToPixelMs: r(summarize(shown.map(({ sample }) => sample.presentMs - sample.receivedMs))),
    sendIntervalMs: r(summarize(rustIntervals)),
    rafIntervalMs: r(summarize(intervals)),
    longFrames: intervals.filter((d) => d > 25).length,
    meterValues: (lastFrame?.meters.length ?? 0) * 4,
    underruns: (lastFrame?.underruns ?? 0) - underruns,
    edits,
    frameWorkMs: r(summarize(work)),
    dbChanges,
    capChanges,
    mainLagMs,
    viewUpdateMs: r(summarize(updates)),
    viewLayoutMs: r(summarize(layouts)),
    viewRequests: session.viewRequests.full - views.full,
    trackRequests: session.viewRequests.tracks - views.tracks,
    clipRequests: session.viewRequests.clips - views.clips,
    deviceRequests: session.viewRequests.devices - views.devices,
    watchedChanges,
    clock: {
      offsetDriftMs: Math.round((clockAfter.offsetMs - clock.offsetMs) * 1000) / 1000,
      // performance.now() has 1 ms steps, so the offset is known to within 1 ms plus half the best
      // round trip.
      errorBoundMs: Math.round((1 + Math.max(clock.errorBoundMs, clockAfter.errorBoundMs)) * 1000) / 1000,
      bestRttMs: Math.max(clock.rttMs, clockAfter.rttMs),
      rttMedianMs: clock.rttMedianMs,
    },
  };
}

/** A drag of the first strip's fader: pointer events on its element, 30 px up and down every 2 s. */
function faderDrag(): { drive: Drive; release: () => void; name: string } {
  const fader = document.querySelector<HTMLElement>('.mixer .strips [role="slider"][aria-label$=" volume"]');
  if (!fader) throw new Error("no fader to drag");
  fader.scrollIntoView({ block: "center", inline: "nearest" });
  const pointer = (type: string, y: number) => {
    const box = fader.getBoundingClientRect();
    const cap = fader.querySelector(".cap")?.getBoundingClientRect() ?? box;
    fader.dispatchEvent(
      new PointerEvent(type, {
        bubbles: true,
        cancelable: true,
        pointerId: 7,
        pointerType: "mouse",
        button: 0,
        buttons: type === "pointerup" ? 0 : 1,
        clientX: cap.left + cap.width / 2,
        clientY: y,
      }),
    );
  };
  let pressed: number | null = null;
  return {
    name: fader.getAttribute("aria-label") ?? "",
    drive: (t) => {
      if (pressed === null) {
        const cap = fader.querySelector(".cap")?.getBoundingClientRect();
        if (!cap) return;
        pressed = cap.top + cap.height / 2;
        pointer("pointerdown", pressed);
        return;
      }
      pointer("pointermove", pressed + 30 * Math.sin((2 * Math.PI * t) / 2));
    },
    release: () => {
      if (pressed !== null) pointer("pointerup", pressed);
    },
  };
}

/**
 * A drag of the Cutoff knob of the first synthesizer's card in the Devices tab (PERF-01):
 * beside a compressor's card, whose gain reduction the live frames update; pointer events on the
 * knob, 30 px up and down every 2 s, so every display frame sends a "Set Parameter" edit.
 */
async function knobDrag(session: Session): Promise<{ drive: Drive; release: () => void; name: string; knob: HTMLElement }> {
  const synth = session.view?.project?.tracks.find((t) => t.instrument?.plugin_id === "sonara.synth.poly");
  const instrument = synth?.instrument;
  if (!synth || !instrument) throw new Error("no synthesizer track");
  if (!synth.inserts.some((d) => d.plugin_id === "sonara.dynamics.compressor")) {
    await backend.edit({ op: "add_device", track_id: synth.id, plugin_id: "sonara.dynamics.compressor" });
    await backend.sealUndo();
  }
  session.showDevices(synth.id);
  const selector = `.devices [data-device="${instrument.id}"] [role="slider"][aria-label="Cutoff"]`;
  const deadline = performance.now() + 15_000;
  let found = document.querySelector<HTMLElement>(selector);
  while (!found || !document.querySelector('.devices [aria-label="Device Compressor"]')) {
    if (performance.now() > deadline) throw new Error("the Devices tab shows no Cutoff knob and compressor");
    await sleep(100);
    found = document.querySelector<HTMLElement>(selector);
  }
  const knob = found;
  knob.scrollIntoView({ block: "nearest", inline: "nearest" });
  await sleep(1500);
  const centre = () => {
    const box = knob.getBoundingClientRect();
    return { x: box.left + box.width / 2, y: box.top + box.height / 2 };
  };
  const pointer = (type: string, y: number) =>
    knob.dispatchEvent(
      new PointerEvent(type, {
        bubbles: true,
        cancelable: true,
        pointerId: 7,
        pointerType: "mouse",
        button: 0,
        buttons: type === "pointerup" ? 0 : 1,
        clientX: centre().x,
        clientY: y,
      }),
    );
  let pressed: number | null = null;
  return {
    name: `${synth.name} Cutoff`,
    knob,
    drive: (t) => {
      if (pressed === null) {
        pressed = centre().y;
        pointer("pointerdown", pressed);
        return;
      }
      pointer("pointermove", pressed + 30 * Math.sin((2 * Math.PI * t) / 2));
    },
    release: () => {
      if (pressed !== null) pointer("pointerup", pressed);
    },
  };
}

export async function runMeterBench(session: Session, config: BenchOptions, status: (text: string) => void): Promise<void> {
  const report: Record<string, unknown> = { started: new Date().toISOString(), quick: config.quick };
  const disturbances = new Disturbances();
  const watchdog = new Promise<never>((_, reject) =>
    setTimeout(() => reject(new Error("watchdog: the benchmark ran for 9 minutes")), 9 * 60 * 1000),
  );
  try {
    await Promise.race([watchdog, suites()]);
  } catch (error) {
    report.error = error instanceof Error ? `${error.message}\n${error.stack}` : String(error);
  }
  try {
    await backend.transportStop();
  } catch {
    // The report says what happened.
  }
  report.valid = disturbances.segments.every((s) => s.valid) && !report.error;
  report.invalidSegments = disturbances.segments.filter((s) => !s.valid);
  report.finished = new Date().toISOString();
  status("Benchmark: writing the report");
  await invoke("bench_report", { report });

  async function suites() {
    status("Benchmark: waiting for the project");
    while (!session.view?.project) await sleep(100);
    // The mixer takes the window's width: the side panes close, the Editors pane grows (the
    // panes are not saved).
    session.panes = {
      browser: { ...session.panes.browser, open: false },
      inspector: { ...session.panes.inspector, open: false },
      editors: { size: Math.max(320, window.innerHeight - 260), open: true },
    };
    session.raiseTab("mixer");
    await sleep(1500);
    await waitForFrames();
    const project = session.view?.project;
    report.env = {
      userAgent: navigator.userAgent,
      devicePixelRatio: window.devicePixelRatio,
      window: { width: window.innerWidth, height: window.innerHeight },
      tracks: project?.tracks.length ?? 0,
      strips: document.querySelectorAll('.mixer [role="group"][aria-label^="Channel strip"]').length,
      meters: session.meters.size,
    };
    status("Benchmark: starting playback on the simulated device");
    await backend.transportPlay(0, true);
    const deadline = performance.now() + 20_000;
    while (session.frame?.state !== "playing" || session.frame.meters.length === 0) {
      if (performance.now() > deadline) throw new Error(`the transport did not play: ${JSON.stringify(session.frame)}`);
      await sleep(100);
    }
    await sleep(3000);
    report.frameCostMs = Math.round((await invoke<number>("bench_frame_cost")) * 1000) / 1000;
    report.engine = {
      sampleRate: session.frame.sample_rate,
      bufferFrames: session.frame.buffer_frames,
      renderLoad: session.frame.render_load,
      outputLatencyMs: session.frame.output_latency_ms,
    };
    status("Benchmark: meters");
    report.meters = await retrying(disturbances, () => recordMeters(session, disturbances, config.quick ? 4000 : 20_000));
    status("Benchmark: meters during a fader drag");
    report.drag = await retrying(disturbances, async () => {
      const drag = faderDrag();
      try {
        return { fader: drag.name, ...(await recordMeters(session, disturbances, config.quick ? 3000 : 10_000, drag.drive)) };
      } finally {
        drag.release();
        await sleep(500);
      }
    });
    // The knob drag twice, back to back: with the refresh of its device alone, then with whole
    // views for its edits (PERF-01: the comparison behind `project_devices`).
    const refresher = (session as unknown as { refresher: { devicePatches: boolean } }).refresher;
    for (const [key, patches] of [
      ["knob", true],
      ["knobWhole", false],
    ] as const) {
      status(`Benchmark: meters during a knob drag in the Devices tab (${patches ? "device refresh" : "whole views"})`);
      refresher.devicePatches = patches;
      report[key] = await retrying(disturbances, async () => {
        const drag = await knobDrag(session);
        try {
          const segment = await recordMeters(session, disturbances, config.quick ? 3000 : 10_000, drag.drive, "Set Parameter", drag.knob);
          return { knob: drag.name, devicePatches: patches, ...segment };
        } finally {
          drag.release();
          await sleep(500);
        }
      });
    }
    refresher.devicePatches = true;
  }
}
