// The piano roll benchmark (ADR 0009's threshold restated by the owner: drag,
// resize and lasso on a 2,000-note clip respond within 2 display frames, pointer to pixel, and
// 10,000 notes paint within one display frame), run in the product's page on the timeline
// benchmark's 100 tracks × 40 clips plus a 2,000-note and a 10,000-note clip. It follows the
// spike's piano roll suite:
//
// - pointer to pixel: synthetic PointerEvents on the note grid, each dispatched at a random phase
//   of a frame; a sample is an event that changed what the grid draws (a preview, the selection,
//   the band), from the event's timestamp to the start of the frame after the draw that showed it,
//   in milliseconds and in display frames (counted from the start of the frame the event fell in).
//   Move and resize hold ⌥, so every event moves the notes and every frame sends an edit to the
//   real session, whose events refresh the page's view (the step's refresh cost);
// - paint: the 10,000-note clip fitted as the prototype's `test_paint_10k_notes_is_fast` (6 px a
//   beat, 6 px rows), the whole piano roll drawn every frame: performance.now() around the draw
//   ("cpu"), and with a one-pixel read-back of each canvas, which waits for its pixels ("synced");
// - a segment during which the page was hidden or no frame came for 250 ms is invalid and is
//   measured again, up to five times (segments.ts).
//
// scripts/bench-timeline.mjs --suite pianoroll builds the project and drives the runs.

import { invoke } from "@tauri-apps/api/core";
import { hitNote } from "../pianoroll/gestures";
import { beatX, clampView, contentExtent, pitchY, type RollView } from "../pianoroll/model";
import type { RollDrawStats } from "../pianoroll/paint";
import type { Session } from "../session.svelte";
import type { MidiClipView, TrackView } from "../types";
import { benchHooks, type PianoRollHook } from "./hooks";
import { Disturbances, nextFrame, retrying, waitForFrames } from "./segments";
import { type Summary, rounded, sleep, summarize } from "./stats";

export interface BenchConfig {
  out: string;
  quick: boolean;
}

const r = (s: Summary) => rounded(s);

type Kind = "move" | "resize" | "lasso";

interface Sample {
  eventMs: number;
  /** The start of the frame the event fell in. */
  eventFrameMs: number;
  drawnMs: number;
  presentMs: number;
}

function clipOf(session: Session, name: string): { track: TrackView; clip: MidiClipView } | null {
  const track = session.view?.project?.tracks.find((t) => t.name === name);
  const clip = track?.clips.find((c) => c.type === "midi");
  return track && clip && clip.type === "midi" ? { track, clip } : null;
}

async function until<T>(what: string, probe: () => T | null | undefined | false, timeoutMs = 30_000): Promise<T> {
  const deadline = performance.now() + timeoutMs;
  for (;;) {
    const value = probe();
    if (value) return value;
    if (performance.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await sleep(50);
  }
}

function pointer(type: string, x: number, y: number, buttons: number, alt: boolean): PointerEvent {
  return new PointerEvent(type, {
    clientX: x,
    clientY: y,
    pointerId: 7,
    pointerType: "mouse",
    isPrimary: true,
    button: 0,
    buttons,
    altKey: alt,
    bubbles: true,
    cancelable: true,
  });
}

/** The start time of every animation frame, from a loop of its own. */
class FrameClock {
  last = 0;
  readonly starts: number[] = [];
  private running = true;

  constructor() {
    const tick = (t: number) => {
      if (!this.running) return;
      this.last = t;
      this.starts.push(t);
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  }

  stop(): void {
    this.running = false;
  }
}

/** Opens `clipId` in the piano roll and waits for its notes. */
async function openClip(session: Session, clipId: string, notes: number): Promise<PianoRollHook> {
  session.openClipEditor(clipId);
  const hook = await until("the piano roll", () => {
    const roll = benchHooks.pianoRoll;
    return roll && roll.clipId() === clipId && (roll.index()?.size ?? 0) === notes ? roll : null;
  });
  await nextFrame();
  await nextFrame();
  return hook;
}

export async function runPianoRollBench(session: Session, config: BenchConfig, status: (text: string) => void): Promise<void> {
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
    status("Benchmark: waiting for the project");
    const two = await until("the 2,000-note clip", () => clipOf(session, "Piano 2k"), 120_000);
    const ten = await until("the 10,000-note clip", () => clipOf(session, "Piano 10k"), 120_000);
    // The piano roll gets most of the window, below a short timeline.
    session.panes = {
      browser: { ...session.panes.browser, open: false },
      inspector: { ...session.panes.inspector, open: false },
      editors: { size: 560, open: true },
    };
    session.raiseTab("piano-roll");
    await sleep(1000);
    await waitForFrames();
    const frameMs = summarize(await frameIntervals(60)).p50;
    report.env = {
      userAgent: navigator.userAgent,
      devicePixelRatio: window.devicePixelRatio,
      window: { width: window.innerWidth, height: window.innerHeight },
      frameMs: rounded(summarize([frameMs])).p50,
      viewNotes: session.view?.project?.tracks.reduce((n, t) => n + t.clips.reduce((m, c) => m + (c.type === "midi" ? c.notes.length : 0), 0), 0),
      tracks: session.view?.project?.tracks.length,
    };
    status("Benchmark: 10,000 notes");
    report.paint = await measurePaint(session, ten.clip.id, ten.clip.notes.length, config.quick ? 1000 : 3000);
    status("Benchmark: pointer to pixel");
    const roll = await openClip(session, two.clip.id, two.clip.notes.length);
    report.env = { ...(report.env as object), grid: roll.canvas.gridSize };
    const events = config.quick ? 60 : 240;
    const pointerReport: Record<string, unknown> = { events, clipNotes: two.clip.notes.length };
    for (const kind of ["move", "resize", "lasso"] as const) {
      status(`Benchmark: piano roll ${kind}`);
      pointerReport[kind] = await retrying(disturbances, () => gesture(session, disturbances, kind, events, frameMs));
    }
    report.pointer = pointerReport;
  }
}

async function frameIntervals(count: number): Promise<number[]> {
  const times: number[] = [];
  for (let i = 0; i <= count; i++) times.push(await nextFrame());
  return times.slice(1).map((t, i) => t - times[i]);
}

/** The 10,000-note clip as the prototype's test views it, drawn every frame. */
async function measurePaint(session: Session, clipId: string, notes: number, durationMs: number) {
  const roll = await openClip(session, clipId, notes);
  const index = roll.index();
  const clip = session.view?.project?.tracks.flatMap((t) => t.clips).find((c) => c.id === clipId);
  if (!index || !clip || clip.type !== "midi") throw new Error("no 10,000-note clip");
  // 6 px a beat and 6 px rows, beat 0 at the left, the notes' pitches (30–96) in the middle.
  const size = roll.canvas.gridSize;
  const view: RollView = { ppb: 6, rowH: 6, x0: 0, y0: (127 - 63 + 0.5) * 6 - size.height / 2 };
  roll.setView(clampView(view, size, contentExtent(clip, index)));
  await nextFrame();
  await nextFrame();
  const canvas = roll.canvas;
  const run = async (synced: boolean) => {
    const draws: RollDrawStats[] = [];
    canvas.paused = true;
    canvas.syncAfterDraw = synced;
    let stop = false;
    const loop = () => {
      if (stop) return;
      const stats = canvas.draw();
      if (stats) draws.push(stats);
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
    await sleep(500);
    draws.length = 0;
    await sleep(durationMs);
    stop = true;
    canvas.syncAfterDraw = false;
    canvas.paused = false;
    const times = draws.map((d) => (synced ? d.syncMs : d.drawMs));
    return {
      frames: draws.length,
      notesDrawn: draws.length ? Math.max(...draws.map((d) => d.notes)) : 0,
      drawMs: r(summarize(times)),
      over16_6ms: times.filter((t) => t > 16.6).length,
    };
  };
  return { cpu: await run(false), synced: await run(true) };
}

/**
 * One gesture on the 2,000-note clip, `events` pointer moves at random phases of the frames:
 * a block of notes selected and one of them dragged by its body (move) or its right edge
 * (resize), or a rubber band from the top left (lasso).
 */
async function gesture(session: Session, disturbances: Disturbances, kind: Kind, events: number, frameMs: number) {
  const roll = benchHooks.pianoRoll;
  if (!roll) throw new Error("no piano roll");
  const clipId = roll.clipId();
  const clip = session.view?.project?.tracks.flatMap((t) => t.clips).find((c) => c.id === clipId);
  if (!clip || clip.type !== "midi") throw new Error("no clip");
  // The first 128 beats and 62 rows from pitch 97 down, as the spike's fit.
  const size = roll.canvas.gridSize;
  const start0 = roll.index();
  if (!start0) throw new Error("no notes");
  roll.setView(
    clampView({ ppb: size.width / 128, rowH: size.height / 62, x0: 0, y0: (127 - 97) * (size.height / 62) }, size, contentExtent(clip, start0)),
  );
  session.selectNotes([]);
  await nextFrame();
  await nextFrame();
  const index = roll.index();
  if (!index) throw new Error("no notes");
  const view = roll.view();
  const box = roll.grid.getBoundingClientRect();
  let start = { x: 20, y: view.rowH * 0.5 };
  let selected = 0;
  if (kind !== "lasso") {
    // A block of notes (the spike's 20–45 % of the width, 25–60 % of the height).
    const ids: string[] = [];
    const b0 = view.x0 + (size.width * 0.2) / view.ppb;
    const b1 = view.x0 + (size.width * 0.45) / view.ppb;
    const pHi = 127 - Math.floor((size.height * 0.25 + view.y0) / view.rowH);
    const pLo = 127 - Math.floor((size.height * 0.6 + view.y0) / view.rowH);
    for (let i = 0; i < index.size; i++) {
      if (index.starts[i] >= b0 && index.starts[i] < b1 && index.pitches[i] >= pLo && index.pitches[i] <= pHi) ids.push(index.ids[i]);
    }
    session.selectNotes(ids);
    selected = ids.length;
    await nextFrame();
    // A note of the block whose body (or right edge) hit-tests to itself.
    const middle = Math.floor(ids.length / 2);
    const found = [...ids.slice(middle), ...ids.slice(0, middle)].find((id) => {
      const note = index.note(id);
      if (!note) return false;
      const x = kind === "resize" ? beatX(view, note.start + note.length) - 2 : beatX(view, note.start + note.length / 2);
      const y = pitchY(view, note.pitch) + view.rowH / 2;
      const hit = hitNote(index, view, x, y);
      if (hit?.note.id === id && hit.edge === (kind === "resize")) {
        start = { x, y };
        return true;
      }
      return false;
    });
    if (!found) throw new Error(`no note to ${kind}`);
  }
  const alt = kind !== "lasso";
  const clock = new FrameClock();
  const samples: Sample[] = [];
  const pending: Sample[] = [];
  const canvas = roll.canvas;
  canvas.onDraw = () => {
    if (pending.length === 0) return;
    const drawn = pending.splice(0);
    const now = performance.now();
    for (const sample of drawn) sample.drawnMs = now;
    requestAnimationFrame((present) => {
      for (const sample of drawn) {
        sample.presentMs = present;
        samples.push(sample);
      }
    });
  };
  const requests = session.viewRequests;
  const versionBefore = session.view?.version ?? 0;
  await nextFrame();
  const end = disturbances.begin(clock.starts);
  const target = roll.grid;
  target.dispatchEvent(pointer("pointerdown", box.left + start.x, box.top + start.y, 1, alt));
  let x = start.x;
  let y = start.y;
  let maxSelected = 0;
  const wave = (e: number, period: number) => 0.5 - 0.5 * Math.cos((2 * Math.PI * e) / period);
  for (let e = 1; e <= events; e++) {
    await nextFrame();
    await sleep(Math.random() * frameMs * 0.9);
    if (kind === "lasso") {
      x = start.x + (size.width - start.x - 20) * wave(e, 160);
      y = start.y + (size.height - start.y - 20) * wave(e, 100);
    } else if (kind === "move") {
      x = start.x + 160 * Math.sin((2 * Math.PI * e) / 120);
      y = start.y + 80 * Math.sin((2 * Math.PI * e) / 90);
    } else {
      x = start.x + 120 * Math.sin((2 * Math.PI * e) / 120);
    }
    const before = roll.changes();
    const event = pointer("pointermove", box.left + x, box.top + y, 1, alt);
    const eventFrameMs = clock.last;
    target.dispatchEvent(event);
    if (roll.changes() !== before) {
      pending.push({ eventMs: event.timeStamp, eventFrameMs, drawnMs: Number.NaN, presentMs: Number.NaN });
    }
    if (kind === "lasso") maxSelected = Math.max(maxSelected, session.selection.notes.length);
  }
  for (let i = 0; i < 6; i++) await nextFrame();
  const releasedAt = performance.now();
  target.dispatchEvent(pointer("pointerup", box.left + x, box.top + y, 0, alt));
  // The release ends when the view has the gesture's last edit (the held notes give way).
  let caughtUpMs = Number.NaN;
  if (kind !== "lasso") {
    for (let i = 0; i < 500; i++) {
      await sleep(5);
      if (!roll.held()) {
        caughtUpMs = performance.now() - releasedAt;
        break;
      }
    }
  }
  await sleep(200);
  const validity = end();
  clock.stop();
  canvas.onDraw = null;
  const after = session.viewRequests;
  const intervals = clock.starts.slice(1).map((t, i) => t - clock.starts[i]);
  const shown = samples.filter((s) => !Number.isNaN(s.presentMs));
  const frames = shown.map((s) => Math.round((s.presentMs - s.eventFrameMs) / frameMs));
  const histogram: Record<string, number> = {};
  for (const f of frames) histogram[String(f)] = (histogram[String(f)] ?? 0) + 1;
  const undo = session.view?.project?.history.undo ?? null;
  // Take the gesture back so the next starts from the same notes.
  if (kind !== "lasso" && undo === "Edit Notes") {
    const version = session.view?.version ?? 0;
    await invoke("undo");
    await until("the undone gesture", () => (session.view?.version ?? 0) > version && !roll.held());
    await sleep(300);
  }
  session.selectNotes([]);
  return {
    validity,
    events,
    shown: shown.length,
    selectedNotes: kind === "lasso" ? maxSelected : selected,
    pointerToPixelMs: r(summarize(shown.map((s) => s.presentMs - s.eventMs))),
    pointerToDrawnMs: r(summarize(shown.map((s) => s.drawnMs - s.eventMs))),
    drawnToPixelMs: r(summarize(shown.map((s) => s.presentMs - s.drawnMs))),
    pointerToPixelFrames: r(summarize(frames)),
    framesHistogram: histogram,
    withinTwoFramesPercent: shown.length ? Math.round((frames.filter((f) => f <= 2).length / shown.length) * 10000) / 100 : 0,
    rafIntervalMs: r(summarize(intervals)),
    longFrames: intervals.filter((d) => d > frameMs * 1.5).length,
    viewRequests: Object.fromEntries(Object.entries(after).map(([key, n]) => [key, n - (requests as Record<string, number>)[key]])),
    sessionVersions: (session.view?.version ?? 0) - versionBefore,
    undo,
    releaseToViewMs: Math.round(caughtUpMs * 10) / 10,
  };
}
