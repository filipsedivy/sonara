// The end-to-end check of the window, run in the real application on a real
// session: the page drives its own canvas and commands, and checks what the backend did through
// the project view and the live frames. It stands in for a WebdriverIO test, whose npm tree
// would need a licence decision; the drop of files from the Finder itself stays a manual check,
// its command is exercised here.
//
// Checks: the tracks show; a drag of a clip is one "Move Clip" step in the session, at the snapped
// beat, and the macOS close button's dot shows the unsaved changes; undo takes it back; the mixer
// shows a strip per track and the master, a header's M mutes the track and its strip shows it, a
// fader's key step is one "Track Volume" step, and the mixer opens in a window of its own whose
// page takes its live frames; the piano roll opens the clip, a note drag moves the note in
// time and pitch as one "Edit Notes" step that refreshes only its clip, undo takes it back, and the
// drum map has its GM names; the device editors show the synthesizer, and a key step of
// its Cutoff knob is one "Set Parameter" step refreshing only its device, which undo takes back;
// an import job (a drop's command) puts a clip on the audio
// track and its waveform is drawn; the transport plays and stops on the simulated device; an
// autosave shows "Autosaved" for 1.5 s; and a hidden window, which gets no animation
// frames, still follows the transport's state and position (stalled play).

import { invoke } from "@tauri-apps/api/core";
import * as backend from "../backend";
import type { Session } from "../session.svelte";
import { beatX, pitchY } from "../pianoroll/model";
import { benchHooks } from "./hooks";
import { sleep } from "./stats";
import type { ClipView, SessionNotice, TrackView } from "../types";

export interface EndToEndConfig {
  /** An audio file to import. */
  media: string | null;
}

interface Check {
  name: string;
  ok: boolean;
  detail: string;
}

const PPB = 24;

async function until<T>(
  what: string,
  probe: () => T | null | undefined | false,
  timeoutMs = 15_000,
  detail: () => string = () => "",
): Promise<T> {
  const deadline = performance.now() + timeoutMs;
  for (;;) {
    const value = probe();
    if (value) return value;
    if (performance.now() > deadline) {
      const more = detail();
      throw new Error(`timed out waiting for ${what}${more ? ` (${more})` : ""}`);
    }
    await sleep(50);
  }
}

/** The session events of `label` from now on, in order, until `stop` is called. */
async function noticesOf(label: string): Promise<{ seen: SessionNotice[]; stop: () => void }> {
  const seen: SessionNotice[] = [];
  const stop = await backend.onSessionChanged((notice) => {
    if (notice.label === label) seen.push(notice);
  });
  return { seen, stop };
}

/** What a step that waits on the page saw, for a timed-out wait's message. */
function pageState(session: Session, control: Element | null, seen: readonly SessionNotice[]): string {
  return [
    `page ${document.visibilityState}${document.hasFocus() ? ", focused" : ", not focused"}`,
    `view v${session.view?.version ?? "-"}`,
    `undo ${session.view?.project?.history.undo ?? "-"}`,
    `events v${seen.map((n) => n.version).join(",") || "none"}`,
    `requests ${JSON.stringify(session.viewRequests)}`,
    `control ${control?.getAttribute("aria-valuenow") ?? "-"}${control?.isConnected === false ? " (detached)" : ""}`,
  ].join("; ");
}

function trackNamed(session: Session, name: string): TrackView | undefined {
  return session.view?.project?.tracks.find((track) => track.name === name);
}

function pointer(target: Element, type: string, x: number, y: number, buttons: number) {
  target.dispatchEvent(
    new PointerEvent(type, {
      clientX: x,
      clientY: y,
      pointerId: 7,
      pointerType: "mouse",
      isPrimary: true,
      button: 0,
      buttons,
      bubbles: true,
      cancelable: true,
    }),
  );
}

/** The status bar's messages (not its errors) with the moment each was shown or cleared. */
class StatusLog {
  private readonly entries: { text: string; at: number }[] = [];

  constructor() {
    const record = (element: Element) =>
      this.entries.push({ text: element.textContent?.trim() ?? "", at: performance.now() });
    // The status bar exists once the window has started (App.svelte).
    const attach = () => {
      const element = document.querySelector('.status-bar [role="status"]');
      if (!element) return false;
      new MutationObserver(() => record(element)).observe(element, { childList: true, characterData: true, subtree: true });
      return true;
    };
    if (!attach()) {
      const wait = new MutationObserver(() => {
        if (attach()) wait.disconnect();
      });
      wait.observe(document.body, { childList: true, subtree: true });
    }
  }

  /** When `text` was first shown, or null. */
  shown(text: string): number | null {
    return this.entries.find((e) => e.text === text)?.at ?? null;
  }

  /** When the message shown at `since` was replaced or cleared, or null. */
  cleared(text: string, since: number): number | null {
    return this.entries.find((e) => e.at > since && e.text !== text)?.at ?? null;
  }

  texts(): string {
    return this.entries.map((e) => JSON.stringify(e.text)).join(", ") || "none";
  }
}

/** The page's visibility changes since the check started, for a timed-out wait's message. */
class VisibilityLog {
  private readonly t0 = performance.now();
  readonly changes: string[] = [`${document.visibilityState}@0`];

  constructor() {
    document.addEventListener("visibilitychange", () =>
      this.changes.push(`${document.visibilityState}@${Math.round(performance.now() - this.t0)} ms`),
    );
  }
}

/**
 * What the page and the engine saw from now on: the animation frames, the live frames received
 * and shown, and the engine's own state (polled), for a timed-out wait's message.
 */
class LiveWatch {
  animationFrames = 0;
  private maxGapMs = 0;
  private lastFrameAt = 0;
  private running = true;
  private readonly started = performance.now();
  private readonly receivedBefore: number;
  private engine = "-";
  private readonly poll: ReturnType<typeof setInterval>;

  constructor(
    private readonly session: Session,
    private readonly visibility: VisibilityLog,
  ) {
    this.receivedBefore = session.queue.received;
    const tick = (now: number) => {
      if (!this.running) return;
      if (this.lastFrameAt > 0) this.maxGapMs = Math.max(this.maxGapMs, now - this.lastFrameAt);
      this.lastFrameAt = now;
      this.animationFrames += 1;
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
    this.poll = setInterval(() => {
      backend
        .transportStatus()
        .then((s) => (this.engine = `${s?.state ?? "-"} at ${String(s?.position_seconds ?? "-")} s`))
        .catch((error: unknown) => (this.engine = String(error)));
    }, 500);
  }

  readonly detail = (): string =>
    [
      `page ${document.visibilityState} (${this.visibility.changes.join(", ")})`,
      `${this.animationFrames} animation frames in ${Math.round(performance.now() - this.started)} ms, longest gap ${Math.round(this.maxGapMs)} ms`,
      `${this.session.queue.received - this.receivedBefore} live frames received, ${this.session.queue.length} waiting`,
      `shown #${this.session.frame?.sequence ?? "-"} ${this.session.frame?.state ?? "-"}`,
      `the page's transport ${this.session.transport().state}`,
      `the engine ${this.engine}`,
    ].join("; ");

  stop(): void {
    this.running = false;
    clearInterval(this.poll);
  }
}

export async function runEndToEnd(
  session: Session,
  config: EndToEndConfig,
  status: (text: string) => void,
): Promise<void> {
  const checks: Check[] = [];
  const report: Record<string, unknown> = { started: new Date().toISOString(), checks };
  const check = (name: string, ok: boolean, detail = "") => checks.push({ name, ok, detail });
  const visibility = new VisibilityLog();
  const statusLog = new StatusLog();
  try {
    status("End to end: waiting for the project");
    const keys = await until("the Keys track", () => trackNamed(session, "Keys"));
    const vox = await until("the Vox track", () => trackNamed(session, "Vox"));
    await until("the arrangement", () => benchHooks.arrangement);
    const names = [...document.querySelectorAll('[aria-label="Tracks"] [role="option"] .name')].map((e) => e.textContent?.trim());
    check("the window shows the project's tracks", names.join(",") === "Keys,Vox", names.join(","));

    // The macOS close button's dot follows the unsaved changes.
    const edited = () => invoke<boolean | null>("bench_document_edited");
    check("a saved project shows no unsaved-changes dot", (await edited()) === false, `${await edited()}`);

    // A drag of the Keys clip by 2.6 beats: one "Move Clip" at beat 2.5.
    status("End to end: dragging a clip");
    const riff = keys.clips[0];
    const canvas = await until("the canvas", () => document.querySelector('[aria-label="Arrangement"]'));
    const box = canvas.getBoundingClientRect();
    const y = box.top + 28;
    const x0 = box.left + (riff.start + 2) * PPB;
    pointer(canvas, "pointerdown", x0, y, 1);
    for (let i = 1; i <= 6; i++) pointer(canvas, "pointermove", x0 + (2.6 * PPB * i) / 6, y, 1);
    pointer(canvas, "pointerup", x0 + 2.6 * PPB, y, 0);
    const moved = await until("the moved clip", () => {
      const clip = trackNamed(session, "Keys")?.clips.find((c) => c.id === riff.id);
      return clip && clip.start !== riff.start ? clip : null;
    });
    check("a drag moves the clip to the snapped beat", moved.start === 2.5, `start ${moved.start}`);
    check(
      "the drag is one undo step",
      session.view?.project?.history.undo === "Move Clip",
      `undo ${session.view?.project?.history.undo}`,
    );
    let dot = await edited();
    for (let i = 0; i < 50 && dot !== true; i++) {
      await sleep(100);
      dot = await edited();
    }
    check("an edit shows the unsaved-changes dot", dot === true, `${dot}`);

    // The autosave of the unsaved changes (every 2 s in this sandbox) shows
    // "Autosaved" in the status bar for 1.5 s.
    status("End to end: autosave");
    const autosaved = await until("the Autosaved message", () => statusLog.shown("Autosaved"), 10_000, () => statusLog.texts());
    const cleared = await until("the cleared Autosaved message", () => statusLog.cleared("Autosaved", autosaved), 10_000, () => statusLog.texts());
    check(
      "an autosave shows Autosaved in the status bar for 1.5 s",
      cleared - autosaved > 1200 && cleared - autosaved < 2500,
      `${Math.round(cleared - autosaved)} ms`,
    );

    status("End to end: undo");
    await backend.undo();
    const back = await until("the undone clip", () => {
      const clip = trackNamed(session, "Keys")?.clips.find((c) => c.id === riff.id);
      return clip && clip.start === riff.start ? clip : null;
    });
    check("undo puts the clip back", back.start === riff.start, `start ${back.start}`);

    // The headers and the mixer on the session.
    status("End to end: the headers and the mixer");
    session.raiseTab("mixer");
    const strips = await until("the mixer's strips", () => {
      const found = document.querySelectorAll('.mixer [role="group"][aria-label^="Channel strip"]');
      return found.length > 0 ? found : null;
    });
    check("the mixer shows a strip per track and the master", strips.length === 3, `${strips.length} strips`);
    const mute = await until("the Keys header's M", () =>
      document.querySelector<HTMLButtonElement>(`.controls[data-track="${keys.id}"] button[aria-label="Mute"]`),
    );
    mute.click();
    await until("the muted Keys", () => trackNamed(session, "Keys")?.mixer.mute === true);
    const stripMute = await until("the Keys strip's pressed M", () =>
      document.querySelector('.mixer [aria-label="Channel strip Keys"] button[aria-label="Mute"][aria-pressed="true"]'),
    );
    check("a header's M mutes the track, and its strip shows it", stripMute !== null);
    await backend.undo();
    await until("the unmuted Keys", () => trackNamed(session, "Keys")?.mixer.mute === false);
    const fader = await until("the Keys fader", () =>
      document.querySelector<HTMLElement>('.mixer [aria-label="Channel strip Keys"] [role="slider"][aria-label="Keys volume"]'),
    );
    // The fader shows the undone model before its key step, and the check waits on what it
    // asserts: the step's session event, then the view of that event (in 1 of 4 runs of
    // step 7 this wait on the volume alone timed out; a timed-out wait now says what it saw).
    const volumes = await noticesOf("Track Volume");
    const faderState = () => pageState(session, fader, volumes.seen);
    await until("the fader at 0 dB", () => fader.isConnected && fader.getAttribute("aria-valuenow") === "0", 15_000, faderState);
    fader.focus();
    fader.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true, cancelable: true }));
    const stepNotice = await until("the Track Volume event", () => volumes.seen[0], 15_000, faderState);
    await until("the view of the Track Volume event", () => (session.view?.version ?? -1) >= stepNotice.version, 15_000, faderState);
    const quieter = trackNamed(session, "Keys")?.mixer.volume_db ?? 0;
    await until("the sealed step", () => session.view?.project?.history.undo === "Track Volume", 15_000, faderState);
    volumes.stop();
    check(
      "a fader's key step is one Track Volume step",
      quieter < 0 && volumes.seen.length === 1,
      `${quieter.toFixed(2)} dB, ${volumes.seen.length} event(s)`,
    );
    await backend.undo();
    await until("the restored Keys", () => trackNamed(session, "Keys")?.mixer.volume_db === 0);

    // The mixer moves to a window of its own, whose page takes its own live frames.
    session.detachMixer();
    await until("the detached mixer", () => session.mixerDetached, 10_000);
    const subscribers = await (async () => {
      for (let i = 0; i < 100; i++) {
        const labels = await invoke<string[]>("bench_live_subscribers");
        if (labels.includes("mixer")) return labels;
        await sleep(100);
      }
      return invoke<string[]>("bench_live_subscribers");
    })();
    check("the mixer opens in its own window, whose page takes the live frames", subscribers.includes("mixer"), subscribers.join(","));

    // The piano roll on the session.
    status("End to end: the piano roll");
    session.openClipEditor(riff.id);
    const roll = await until("the piano roll", () => {
      const hook = benchHooks.pianoRoll;
      return hook && hook.clipId() === riff.id && (hook.index()?.size ?? 0) === 4 ? hook : null;
    });
    await sleep(300);
    const rollView = roll.view();
    const first = roll.index()?.notes[0];
    if (!first) throw new Error("no notes in the piano roll");
    const grid = roll.grid.getBoundingClientRect();
    const nx = grid.left + beatX(rollView, first.start + first.length / 2);
    const ny = grid.top + pitchY(rollView, first.pitch) + rollView.rowH / 2;
    const clipRefreshes = session.viewRequests.clips;
    pointer(roll.grid, "pointerdown", nx, ny, 1);
    for (let i = 1; i <= 6; i++) {
      pointer(roll.grid, "pointermove", nx + (2 * rollView.ppb * i) / 6, ny - (2 * rollView.rowH * i) / 6, 1);
    }
    pointer(roll.grid, "pointerup", nx + 2 * rollView.ppb, ny - 2 * rollView.rowH, 0);
    const noteOf = () => {
      const clip = trackNamed(session, "Keys")?.clips.find((c) => c.id === riff.id);
      return clip?.type === "midi" ? clip.notes.find((n) => n.id === first.id) : undefined;
    };
    const movedNote = await until("the moved note", () => {
      const n = noteOf();
      return n && n.start !== first.start ? n : null;
    });
    await until("the sealed note drag", () => session.view?.project?.history.undo === "Edit Notes");
    check(
      "a note drag moves it in time and pitch, snapped",
      movedNote.start === first.start + 2 && movedNote.pitch === first.pitch + 2,
      `start ${movedNote.start}, pitch ${movedNote.pitch}`,
    );
    check(
      "the note edits refresh only their clip",
      session.viewRequests.clips > clipRefreshes,
      `${session.viewRequests.clips - clipRefreshes} clip refreshes`,
    );
    await backend.undo();
    const restored = await until("the undone note drag", () => {
      const n = noteOf();
      return n && n.start === first.start ? n : null;
    });
    check("one undo takes the whole note drag back", restored.pitch === first.pitch, `pitch ${restored.pitch}`);
    const kick = await until("the drum map", () => session.drumNames?.labels.get(36));
    check("the drum map names the GM notes", kick === "Kick" && session.drumNames?.labels.get(42) === "Hat Closed", kick);

    // The device editors on the session: the Keys track's synthesizer; a key
    // step of its Cutoff knob is one "Set Parameter" step whose refresh is of its device alone, and
    // one undo takes it back.
    status("End to end: the device editors");
    const synth = trackNamed(session, "Keys")?.instrument;
    if (!synth) throw new Error("the Keys track has no instrument");
    session.showDevices(keys.id);
    const cutoff = await until("the Cutoff knob", () =>
      document.querySelector<HTMLElement>(`.devices [data-device="${synth.id}"] [role="slider"][aria-label="Cutoff"]`),
    );
    const cutoffOf = () => Number(trackNamed(session, "Keys")?.instrument?.params.filter_cutoff ?? Number.NaN);
    const before = cutoffOf();
    const params = await noticesOf("Set Parameter");
    const knobState = () => pageState(session, cutoff, params.seen);
    const requests = session.viewRequests;
    cutoff.focus();
    cutoff.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true, cancelable: true }));
    const paramNotice = await until("the Set Parameter event", () => params.seen[0], 15_000, knobState);
    await until("the view of the Set Parameter event", () => (session.view?.version ?? -1) >= paramNotice.version, 15_000, knobState);
    const raised = cutoffOf();
    await until("the sealed parameter step", () => session.view?.project?.history.undo === "Set Parameter", 15_000, knobState);
    params.stop();
    check(
      "a knob's key step is one Set Parameter step",
      raised > before && params.seen.length === 1,
      `${before} → ${raised.toFixed(1)} Hz, ${params.seen.length} event(s)`,
    );
    const after = session.viewRequests;
    check(
      "the knob's edit refreshes only its device",
      after.devices > requests.devices && after.full === requests.full,
      `${after.devices - requests.devices} device refreshes, ${after.full - requests.full} whole views`,
    );
    await backend.undo();
    const undone = await until("the undone Cutoff", () => (cutoffOf() === before ? cutoffOf() : null), 15_000, knobState);
    check("one undo takes the knob's step back", undone === before, `${undone} Hz`);

    // A drop's command: an import job onto the Vox track at beat 4.
    if (config.media) {
      status("End to end: importing audio");
      await backend.importAudioFiles([config.media], vox.id, 4);
      const imported = await until(
        "the imported clip",
        () => trackNamed(session, "Vox")?.clips.find((c: ClipView) => c.type === "audio"),
        30_000,
      );
      check("an import job puts the clip at the drop's beat", imported.start === 4, `start ${imported.start}`);
      if (imported.type === "audio") {
        const assetId = imported.asset_id;
        await until("the waveform", () => session.peaks.get(assetId)?.state === "ready", 30_000);
        const arrangement = benchHooks.arrangement;
        if (arrangement?.renderer) {
          // The waveform's colour inside the clip, below its header.
          arrangement.draw();
          const pixels = arrangement.renderer.readPixels();
          const scale = arrangement.renderer.canvas.width / arrangement.width;
          const rowTop = 56;
          const width = arrangement.renderer.canvas.width;
          const height = arrangement.renderer.canvas.height;
          const colors = new Set<string>();
          for (let cx = (4 * PPB + 4) * scale; cx < (4 * PPB + 4 + 60) * scale; cx += 2) {
            for (let cy = (rowTop + 18) * scale; cy < (rowTop + 52) * scale; cy += 2) {
              const i = ((height - 1 - Math.floor(cy)) * width + Math.floor(cx)) * 4;
              colors.add(`${pixels[i]},${pixels[i + 1]},${pixels[i + 2]}`);
            }
          }
          check("the waveform is drawn", colors.size >= 2, `${colors.size} colours in the clip's content`);
        } else {
          check("the waveform is drawn", false, arrangement?.error ?? "no renderer");
        }
      }
    }

    // The transport on the simulated device. A timed-out wait says what the page
    // and the engine saw: a page that is not drawn gets no animation frames (live/queue.ts).
    status("End to end: playing");
    const watch = new LiveWatch(session, visibility);
    try {
      await backend.transportPlay(0, false);
      const playing = await until("a playing frame", () => (session.frame?.state === "playing" ? session.frame : null), 15_000, watch.detail);
      const startBeat = playing.position.beat ?? 0;
      await sleep(500);
      const later = session.frame?.position.beat ?? 0;
      check("the transport plays and its position advances", later > startBeat, `${startBeat} → ${later}; ${watch.detail()}`);
      await backend.transportStop();
      await until("a stopped frame", () => session.frame?.state === "stopped", 15_000, watch.detail);
      check("the transport stops", true);
    } finally {
      watch.stop();
    }

    // A window that is not drawn (hidden here; minimized, on another Space or behind a full-screen
    // app alike) gets no animation frames, and still follows the transport: the mixer window's keys,
    // the menu and a seek act on its state. Step 14's runs on the built-in display stopped at the
    // play above because the window was not drawn there.
    status("End to end: playing in a hidden window");
    await invoke("bench_show_main", { visible: false });
    const hidden = new LiveWatch(session, visibility);
    try {
      await until("the hidden page", () => document.visibilityState === "hidden", 5_000, hidden.detail);
      await backend.transportPlay(0, false);
      const playing = await until(
        "a playing frame in the hidden window",
        () => (session.frame?.state === "playing" && session.transport().state === "playing" ? session.frame : null),
        5_000,
        hidden.detail,
      );
      const startBeat = playing.position.beat ?? 0;
      await sleep(500);
      const later = session.frame?.position.beat ?? 0;
      check(
        "a hidden window follows the transport and its position",
        later > startBeat && session.playhead.beat > 0 && hidden.animationFrames === 0,
        `${startBeat} → ${later}, playhead ${session.playhead.beat.toFixed(2)}; ${hidden.detail()}`,
      );
      await backend.transportStop();
      await until("a stopped frame in the hidden window", () => session.transport().state === "stopped", 5_000, hidden.detail);
      check("a hidden window sees the stop", session.frame?.state === "stopped");
    } finally {
      hidden.stop();
      await invoke("bench_show_main", { visible: true });
    }
  } catch (error) {
    report.error = error instanceof Error ? `${error.message}\n${error.stack}` : String(error);
  }
  report.valid = !report.error && checks.every((c) => c.ok);
  report.finished = new Date().toISOString();
  status("End to end: writing the report");
  await invoke("bench_report", { report });
}
