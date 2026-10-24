// The timeline benchmark (ADR 0009's threshold: draw p95 ≤ 7 ms and no frame
// over 16.6 ms in 10 s of scrolling and zooming), run in the product's page on a real project of
// 100 tracks × 40 clips. It follows the ADR 0009 spike's timeline suite,
// adapted to the product:
//
// - the product's renderer (Arrangement: the WebGL2 scene, the grid, the text overlay) draws
//   every animation frame while the view moves;
// - "working": the M track height (56 px), zooming between the whole song and about six bars,
//   scrolling both ways; "overview": the S height (34 px, the product's smallest), zooming
//   between the whole song and a sixth of it, scrolling through all the tracks;
// - each scenario is measured twice: performance.now() around the draw ("cpu"), and around the
//   draw plus a one-pixel read-back that waits for the GPU ("synced");
// - a segment during which the page was hidden or no frame came for 250 ms is invalid and is
//   measured again, up to five times (the spike's `Disturbances` and `retrying`);
// - the project view's cost: its request (IPC and JSON), the model and a full scene rebuild.
//
// The window runs it when the desktop was built with the `bench` feature and started with
// SONARA_BENCH_OUT (src-tauri/src/bench.rs); scripts/bench-timeline.mjs drives the runs.

import { invoke } from "@tauri-apps/api/core";
import * as backend from "../backend";
import type { Session } from "../session.svelte";
import { TRACK_HEIGHTS, type TimelineView } from "../state/timeline";
import { TrackLayout } from "../timeline/layout";
import { TimelineModel } from "../timeline/model";
import type { DrawStats } from "../timeline/arrangement";
import { benchHooks } from "./hooks";
import { Disturbances, nextFrame, retrying, waitForFrames } from "./segments";
import { type Summary, rounded, sleep, summarize } from "./stats";

export interface BenchConfig {
  out: string;
  quick: boolean;
}

const r = (s: Summary) => rounded(s);

export interface Scenario {
  name: string;
  trackHeight: "M" | "S";
  view: (t: number, width: number, height: number, length: number, content: number) => TimelineView;
}

const wave = (t: number, period: number) => 0.5 - 0.5 * Math.cos((2 * Math.PI * t) / period);
const logLerp = (a: number, b: number, f: number) => Math.exp(Math.log(a) + (Math.log(b) - Math.log(a)) * f);

function at(ppb: number, beat0: number, scrollY: number, trackHeight: "M" | "S"): TimelineView {
  return { pixelsPerBeat: ppb, scrollX: beat0 * ppb, scrollY, trackHeight, follow: false };
}

/** The scroll and zoom of the benchmark; the playback check (playback.ts) moves the view the same way. */
export const SCENARIOS: Scenario[] = [
  {
    // M tracks (about 10 visible); zoom between the whole song and ~6 bars; scroll both ways.
    name: "working",
    trackHeight: "M",
    view: (t, width, height, length, content) => {
      const ppb = logLerp(width / length, 48, wave(t, 5));
      const span = width / ppb;
      const center = length * wave(t, 10);
      const beat0 = Math.max(0, Math.min(length - span, center - span / 2));
      return at(ppb, beat0, Math.max(0, content - height) * wave(t, 7), "M");
    },
  },
  {
    // S tracks, the product's smallest; zoom between the whole song and a sixth of it.
    name: "overview",
    trackHeight: "S",
    view: (t, width, height, length, content) => {
      const ppb = logLerp(width / length, (6 * width) / length, wave(t, 5));
      const span = width / ppb;
      const center = length * wave(t, 10);
      const beat0 = Math.max(0, Math.min(length - span, center - span / 2));
      return at(ppb, beat0, Math.max(0, content - height) * wave(t, 7), "S");
    },
  },
];

/** The project's assets have their pyramids (or failed), within `maxWaitMs`. */
export async function waitForPeaks(session: Session, maxWaitMs = 120_000): Promise<{ assets: number; seconds: number }> {
  const t0 = performance.now();
  const assets = session.view?.project?.assets.filter((a) => a.kind === "audio") ?? [];
  for (;;) {
    const pending = assets.filter((asset) => {
      const peaks = session.peaks.get(asset.id);
      return !peaks || peaks.state === "pending";
    });
    if (pending.length === 0) break;
    if (performance.now() - t0 > maxWaitMs) throw new Error(`${pending.length} waveforms still pending`);
    await sleep(250);
  }
  return { assets: assets.length, seconds: Math.round(performance.now() - t0) / 1000 };
}

export async function runTimelineBench(session: Session, config: BenchConfig, status: (text: string) => void): Promise<void> {
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
  report.valid = disturbances.segments.every((s) => s.valid) && !report.error;
  report.invalidSegments = disturbances.segments.filter((s) => !s.valid);
  report.finished = new Date().toISOString();
  status("Benchmark: writing the report");
  await invoke("bench_report", { report });

  async function suites() {
    status("Benchmark: waiting for the project and its waveforms");
    while (!session.view?.project || !benchHooks.arrangement) await sleep(100);
    // The timeline takes the whole window, as the spike's did (the panes are not saved).
    session.panes = {
      browser: { ...session.panes.browser, open: false },
      inspector: { ...session.panes.inspector, open: false },
      editors: { ...session.panes.editors, open: false },
    };
    report.peaks = await waitForPeaks(session);
    await sleep(1000);
    await waitForFrames();
    const arrangement = benchHooks.arrangement;
    if (!arrangement) throw new Error("no arrangement");
    if (arrangement.error) throw new Error(`the timeline cannot draw: ${arrangement.error}`);
    report.env = {
      userAgent: navigator.userAgent,
      devicePixelRatio: window.devicePixelRatio,
      window: { width: window.innerWidth, height: window.innerHeight },
      canvas: { width: arrangement.width, height: arrangement.height },
      renderer: arrangement.renderer?.renderer ?? null,
    };
    status("Benchmark: the project view");
    report.view = await measureView(session);
    report.timeline = await measureTimeline(session, disturbances, config.quick ? 2000 : 10_000, status);
  }
}

/** The project view's request, its model and a full scene rebuild with a draw. */
async function measureView(session: Session) {
  const arrangement = benchHooks.arrangement;
  const invokes: number[] = [];
  let bytes = 0;
  let view = session.view;
  for (let i = 0; i < 20; i++) {
    const t0 = performance.now();
    view = await backend.projectView();
    invokes.push(performance.now() - t0);
  }
  const parses: number[] = [];
  if (view) {
    const text = JSON.stringify(view);
    bytes = new TextEncoder().encode(text).length;
    // The page's share of a request: parsing the JSON it carries.
    for (let i = 0; i < 10; i++) {
      const t0 = performance.now();
      JSON.parse(text);
      parses.push(performance.now() - t0);
    }
  }
  // The round trip of a command without a payload, for the IPC's own share.
  const empties: number[] = [];
  for (let i = 0; i < 20; i++) {
    const t0 = performance.now();
    await backend.transportStatus();
    empties.push(performance.now() - t0);
  }
  const project = view?.project;
  const models: number[] = [];
  const rebuilds: number[] = [];
  let counts = {};
  if (project && arrangement) {
    arrangement.paused = true;
    for (let i = 0; i < 10; i++) {
      const t0 = performance.now();
      const model = new TimelineModel(project);
      const layout = new TrackLayout(project.tracks, TRACK_HEIGHTS.M);
      models.push(performance.now() - t0);
      await nextFrame();
      const t1 = performance.now();
      arrangement.update({ model, layout, view: session.timeline, selection: session.selection, peaksVersion: session.peaksVersion });
      arrangement.draw();
      rebuilds.push(performance.now() - t1);
    }
    arrangement.paused = false;
    counts = {
      tracks: project.tracks.length,
      clips: project.tracks.reduce((n, t) => n + t.clips.length, 0),
      notes: project.tracks.reduce((n, t) => n + t.clips.reduce((m, c) => m + (c.type === "midi" ? c.notes.length : 0), 0), 0),
      assets: project.assets.length,
    };
  }
  return {
    bytes,
    ...counts,
    requestMs: r(summarize(invokes)),
    parseMs: r(summarize(parses)),
    emptyRequestMs: r(summarize(empties)),
    modelMs: r(summarize(models)),
    rebuildAndDrawMs: r(summarize(rebuilds)),
  };
}

async function measureTimeline(
  session: Session,
  disturbances: Disturbances,
  durationMs: number,
  status: (text: string) => void,
) {
  const arrangement = benchHooks.arrangement;
  const project = session.view?.project;
  if (!arrangement || !project) throw new Error("no timeline");
  const model = new TimelineModel(project);
  const layouts = {
    M: new TrackLayout(project.tracks, TRACK_HEIGHTS.M),
    S: new TrackLayout(project.tracks, TRACK_HEIGHTS.S),
  };
  const length = Math.max(16, project.end_beat);
  const result: Record<string, unknown> = {};
  arrangement.paused = true;
  try {
    for (const scenario of SCENARIOS) {
      const layout = layouts[scenario.trackHeight];
      const run = (synced: boolean) =>
        retrying(disturbances, async () => {
          const draws: number[] = [];
          const frameTimes: number[] = [];
          arrangement.syncAfterDraw = synced;
          let origin = 0;
          let recording = false;
          let stop = false;
          const loop = (now: number) => {
            if (stop) return;
            if (!origin) origin = now;
            const view = scenario.view((now - origin) / 1000, arrangement.width, arrangement.height, length, layout.contentHeight);
            arrangement.update({ model, layout, view, selection: session.selection, peaksVersion: session.peaksVersion });
            const stats: DrawStats | null = arrangement.draw(now);
            if (recording && stats) {
              draws.push(synced ? stats.syncMs : stats.drawMs);
              frameTimes.push(now);
            }
            requestAnimationFrame(loop);
          };
          requestAnimationFrame(loop);
          // Warm-up: JIT, shader compilation, buffers.
          await sleep(1000);
          recording = true;
          origin = 0;
          const end = disturbances.begin(frameTimes);
          await sleep(durationMs);
          const validity = end();
          stop = true;
          recording = false;
          arrangement.syncAfterDraw = false;
          const intervals = frameTimes.slice(1).map((t, i) => t - frameTimes[i]);
          const nominal = summarize(intervals).p50;
          return {
            validity,
            frames: draws.length,
            drawMs: r(summarize(draws)),
            over16_6ms: draws.filter((d) => d > 16.6).length,
            over7ms: draws.filter((d) => d > 7).length,
            rafIntervalMs: r(summarize(intervals)),
            longFrames: intervals.filter((d) => d > nominal * 1.5).length,
          };
        });
      status(`Benchmark: timeline ${scenario.name}`);
      const cpu = await run(false);
      status(`Benchmark: timeline ${scenario.name}, synced`);
      const synced = await run(true);
      result[scenario.name] = { trackHeight: scenario.trackHeight, cpu, synced };
    }
  } finally {
    arrangement.paused = false;
    arrangement.update({
      model,
      layout: layouts.M,
      view: session.timeline,
      selection: session.selection,
      peaksVersion: session.peaksVersion,
    });
  }
  return result;
}
