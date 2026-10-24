// The playback check: a real project plays on the system's audio device
// (the `native` backend) with its master at −144 dB, so nothing is audible, while the timeline
// scrolls and zooms as in the timeline benchmark ("working", timeline.ts) and the mixer's meters
// run; the requirement is 0 underruns (Night Drive and Ascension). It is the GUI's share of
// the prototype's `--smoke-play`: the headless numbers are (docs/audio-engine.md).
//
// - the harness (scripts/bench-timeline.mjs --suite playback) copies the project into its sandbox
//   and sets the copy's master to −144 dB through `set_track_volume`, the `SetTrackMixer` of
//   `smoke_play`, so the whole graph still renders; the page refuses to play a louder master;
// - the view moves through the real timeline (the session's view, Follow off) at every animation
//   frame, the Editors pane shows the Mixer tab, and the window's meters draw the live frames;
// - the engine's status is read every second: its state, position, underruns and device errors;
// - the segment is invalid when the page was hidden or no animation frame came for 250 ms
//   (segments.ts), and it is played again, up to five times.
//
// The window runs it when the desktop was built with the `bench` feature and started with
// SONARA_BENCH_OUT and SONARA_BENCH_SUITE=playback (src-tauri/src/bench.rs).

import { invoke } from "@tauri-apps/api/core";
import * as backend from "../backend";
import type { BenchOptions } from "../backend";
import type { Session } from "../session.svelte";
import { TRACK_HEIGHTS } from "../state/timeline";
import { TrackLayout } from "../timeline/layout";
import type { EngineStatus } from "../types";
import { benchHooks } from "./hooks";
import { Disturbances, retrying, waitForFrames } from "./segments";
import { rounded, sleep, summarize } from "./stats";
import { SCENARIOS, waitForPeaks } from "./timeline";

/** The loudest master the check plays: the fader's bottom, −∞ (`MIN_DB`). */
const SILENT_MASTER_DB = -143.9;

/** One reading of the engine's status. */
interface Reading {
  atS: number;
  state: string;
  positionS: number | null;
  underruns: number;
  deviceErrors: number;
  renderLoad: number | null;
}

const number = (value: unknown): number | null => (typeof value === "number" && Number.isFinite(value) ? value : null);

function reading(status: EngineStatus | null, atS: number): Reading {
  return {
    atS: Math.round(atS * 100) / 100,
    state: typeof status?.state === "string" ? status.state : "-",
    positionS: number(status?.position_seconds),
    underruns: number(status?.underruns) ?? 0,
    deviceErrors: number(status?.device_errors) ?? 0,
    renderLoad: number(status?.render_load),
  };
}

export async function runPlaybackBench(session: Session, config: BenchOptions, status: (text: string) => void): Promise<void> {
  const report: Record<string, unknown> = { started: new Date().toISOString(), quick: config.quick };
  const disturbances = new Disturbances();
  const watchdog = new Promise<never>((_, reject) =>
    setTimeout(() => reject(new Error("watchdog: the playback check ran for 9 minutes")), 9 * 60 * 1000),
  );
  try {
    await Promise.race([watchdog, check()]);
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

  async function check() {
    status("Benchmark: waiting for the project");
    while (!session.view?.project || !benchHooks.arrangement) await sleep(100);
    const project = session.view.project;
    const masterDb = project.master.mixer.volume_db;
    if (!(masterDb <= SILENT_MASTER_DB)) {
      throw new Error(`the master is at ${masterDb} dB, not −144 dB: the check does not play aloud`);
    }
    // The timeline beside the Mixer tab, as a user works: the Editors pane open, the side panes
    // closed (the panes are not saved).
    session.panes = {
      browser: { ...session.panes.browser, open: false },
      inspector: { ...session.panes.inspector, open: false },
      editors: { size: Math.max(300, Math.round(window.innerHeight * 0.4)), open: true },
    };
    session.raiseTab("mixer");
    report.peaks = await waitForPeaks(session);
    await sleep(1500);
    await waitForFrames();
    report.env = {
      userAgent: navigator.userAgent,
      devicePixelRatio: window.devicePixelRatio,
      window: { width: window.innerWidth, height: window.innerHeight },
      project: project.name,
      tracks: project.tracks.length,
      strips: document.querySelectorAll('.mixer [role="group"][aria-label^="Channel strip"]').length,
      meters: session.meters.size,
      masterDb,
    };
    const seconds = config.quick ? 8 : 60;
    status(`Benchmark: playing ${project.name} on the audio device for ${seconds} s (master at −144 dB)`);
    report.playback = await retrying(disturbances, () => play(seconds));
  }

  async function play(seconds: number) {
    const arrangement = benchHooks.arrangement;
    const project = session.view?.project;
    if (!arrangement || !project) throw new Error("no timeline");
    const layout = new TrackLayout(project.tracks, TRACK_HEIGHTS.M);
    const length = Math.max(16, project.end_beat);
    const scenario = SCENARIOS[0];
    const frameTimes: number[] = [];
    let received = 0;
    let shown = 0;
    benchHooks.liveReceived = () => (received += 1);
    benchHooks.meterShown = () => (shown += 1);
    const readings: Reading[] = [];
    const before = reading(await backend.transportStatus(), 0);
    await backend.transportPlay(0, false);
    // The first play opens the device: the segment starts once the engine says it plays.
    const deadline = performance.now() + 10_000;
    for (;;) {
      const now = await backend.transportStatus();
      if (now?.state === "playing") break;
      if (performance.now() > deadline) throw new Error(`the transport did not start: ${JSON.stringify(now)}`);
      await sleep(50);
    }
    const started = performance.now();
    let stop = false;
    let origin = 0;
    const loop = (now: number) => {
      if (stop) return;
      if (!origin) origin = now;
      session.timeline = scenario.view((now - origin) / 1000, arrangement.width, arrangement.height, length, layout.contentHeight);
      frameTimes.push(now);
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
    const end = disturbances.begin(frameTimes);
    const poll = setInterval(() => {
      const at = (performance.now() - started) / 1000;
      backend
        .transportStatus()
        .then((s) => readings.push(reading(s, at)))
        .catch(() => readings.push(reading(null, at)));
    }, 1000);
    await sleep(seconds * 1000);
    const validity = end();
    stop = true;
    clearInterval(poll);
    const raw = await backend.transportStatus();
    const last = reading(raw, (performance.now() - started) / 1000);
    await backend.transportStop();
    benchHooks.liveReceived = null;
    benchHooks.meterShown = null;
    const intervals = frameTimes.slice(1).map((t, i) => t - frameTimes[i]);
    const nominal = summarize(intervals).p50;
    const playing = [...readings, last].filter((r) => r.state === "playing").length;
    return {
      validity,
      seconds,
      // The engine's counters over the play (the player starts at the first play).
      underruns: last.underruns - before.underruns,
      deviceErrors: last.deviceErrors - before.deviceErrors,
      renderLoad: last.renderLoad,
      callbackLoad: number(raw?.callback_load),
      outputLatencyMs: number(raw?.output_latency_estimate_ms),
      sampleRate: number(raw?.device_sample_rate),
      bufferFrames: number(raw?.buffer_frames),
      lastError: typeof raw?.last_error === "string" ? raw.last_error : null,
      deviceStatus: typeof raw?.device_status === "string" ? raw.device_status : null,
      positionSeconds: last.positionS,
      // Every reading said playing: the transport rolled the whole time.
      rolledPercent: Math.round((playing / (readings.length + 1)) * 10000) / 100,
      readings,
      liveFramesReceived: received,
      meterFramesShown: shown,
      meterFps: Math.round((shown / seconds) * 100) / 100,
      rafIntervalMs: rounded(summarize(intervals)),
      longFrames: intervals.filter((d) => d > nominal * 1.5).length,
    };
  }
}
