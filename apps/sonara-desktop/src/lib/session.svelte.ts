// The page's state (Svelte 5 runes, no state library): the project view, kept current by the
// session's events; the live frame; the jobs; and the prototype's GUI-only state: the selection,
// the playhead, the snap grid and the timeline's zoom, scroll and Follow. The logic is in the
// plain modules of state/ and live/; this class only holds their current values.
//
// The mixer can detach into a window of its own. That window's page has a session
// of its own (role "mixer": its view, its live frames, its meters), and the windows share the
// selection while both are open: a change made in one is sent to the other (`gui-selection`), and
// a mixer window that starts asks for the main window's (`sync`). What only the main window can
// do, the Devices tab and the transport's keys, the mixer window asks it for (`gui-intent`).

import { flushSync } from "svelte";
import * as backend from "./backend";
import { gridBeats, snapBeat, type GridName, type Snap, DEFAULT_SNAP, type SnapMode } from "./state/snap";
import { decodeLiveFrame, type LiveFrame } from "./live/frame";
import { FrameQueue, waitsForDisplay } from "./live/queue";
import { MidiFocus } from "./state/focus";
import { PeaksStore } from "./state/peaks";
import { ViewRefresher, type ViewPatch } from "./state/project";
import * as selection from "./state/selection";
import {
  DEFAULT_TIMELINE,
  START_PLAYHEAD,
  followPlayhead,
  playheadFromFrame,
  type Playhead,
  type TimelineView,
} from "./state/timeline";
import type { EngineState, LiveState } from "./live/frame";
import { benchHooks } from "./bench/hooks";
import { MeterBank } from "./mixer/meter";
import { EditTargets } from "./shell/editing";
import { DEFAULT_PANES } from "./shell/panes";
import type { TransportShortcut } from "./shell/shortcuts";
import { DEFAULT_ROLL_VIEW, type NoteLengthName, type RollView } from "./pianoroll/model";
import { devicesTrackAfter, type SelectionCause } from "./devices/focus";
import type { JobNotice, PaneLayout, PluginGroup, ProjectView, SessionNotice, StatusMessage } from "./types";

/** The tabs of the Editors pane. */
export type EditorTab = "mixer" | "piano-roll" | "devices";

/**
 * The piano roll's GUI-only state (`PianoRollEditor`'s view and the toolbar's values), kept while
 * its tab is hidden; none of it is persisted.
 */
export interface RollSettings {
  readonly view: RollView;
  /** The clip the view was centred on; another current clip is centred again. */
  readonly centredClip: string | null;
  /** The length of new notes. */
  readonly lengthName: NoteLengthName;
  /** The velocity of new notes, 1–127. */
  readonly velocity: number;
  /** The quantize strength in percent, 0–100. */
  readonly strength: number;
}

export const DEFAULT_ROLL: RollSettings = {
  view: DEFAULT_ROLL_VIEW,
  centredClip: null,
  lengthName: "Grid",
  velocity: 100,
  strength: 100,
};

/** The drum map as the piano roll reads it. */
export interface DrumNames {
  readonly pluginId: string;
  readonly labels: ReadonlyMap<number, string>;
}

/** Which window a page is: the main window, or the mixer's own. */
export type WindowRole = "main" | "mixer";

/** A selection one window's page sends the others (`gui-selection`). */
export interface SharedSelection {
  from: string;
  selection: selection.Selection;
}

/** What the mixer window asks the main window for (`gui-intent`). */
export type WindowIntent =
  | { from: string; kind: "show-devices"; trackId: string | null }
  | { from: string; kind: "shortcut"; action: TransportShortcut }
  /** The mixer window's page started: the main window sends it the selection. */
  | { from: string; kind: "sync" };

export class Session {
  /**
   * The project view; `null` until the first one arrives. Raw state: a view is replaced whole
   * and never changed in place, and a deep proxy over 4,000 clips would slow every reader.
   */
  view: ProjectView | null = $state.raw(null);
  /** The latest session event. */
  notice: SessionNotice | null = $state(null);
  /** Grows when the structure changed: panels rebuild instead of refreshing in place. */
  structure = $state(0);
  /** The live frame shown at this display frame. */
  frame: LiveFrame | null = $state(null);
  /** The unfinished jobs, for the status bar's label and progress. */
  jobs: JobNotice[] = $state([]);
  /** Grows whenever a waveform pyramid changed. */
  peaksVersion = $state(0);

  selection: selection.Selection = $state.raw(selection.EMPTY_SELECTION);
  snap: Snap = $state.raw(DEFAULT_SNAP);
  playhead: Playhead = $state.raw(START_PLAYHEAD);
  timeline: TimelineView = $state.raw(DEFAULT_TIMELINE);
  /** The window's panes. */
  panes: PaneLayout = $state.raw(DEFAULT_PANES);
  /** The Editors pane's tab. */
  editorTab: EditorTab = $state("mixer");
  /** The transport bar's Record button. */
  recording = $state(false);
  /** The main window's mixer is in a window of its own. */
  mixerDetached = $state(false);
  /** The effect picker, once loaded. */
  effectGroups: PluginGroup[] = $state.raw([]);
  /** The piano roll's view and toolbar values. */
  roll: RollSettings = $state.raw(DEFAULT_ROLL);
  /** The GM names of drum notes, once the piano roll asked for them. */
  drumNames: DrumNames | null = $state.raw(null);
  private drumNamesRequested = false;
  /** The track the Devices tab shows (devices/focus.ts), shown or not. */
  devicesTrack: string | null = $state(null);
  /**
   * The transport's state as a command answered it, until the next live frame: a second Space
   * right after Play must stop, even before a frame says the transport plays.
   */
  private answeredState: LiveState | null = $state(null);

  readonly queue = new FrameQueue<LiveFrame>();
  readonly peaks: PeaksStore;
  /** The window's meters, fed at every animation frame. */
  readonly meters = new MeterBank();
  /** The editors the Edit menu's items act on (shell/editing.ts). */
  readonly editing = new EditTargets();
  private readonly refresher: ViewRefresher;
  private readonly focus: MidiFocus;
  private stopped = false;
  /** When the last animation frame started (`performance.now()`); a page that is not drawn has none. */
  private lastAnimationFrame = performance.now();
  /** The latest frame shown when it arrived, whose meters no animation frame has drawn yet. */
  private undrawn: LiveFrame | null = null;
  /** This page's web view label, set by {@link start}. */
  private label = "main";
  /** The main window's handler of the mixer window's transport keys. */
  onShortcut: ((action: TransportShortcut) => void) | null = null;

  constructor(
    private readonly report: (message: StatusMessage) => void,
    readonly role: WindowRole = "main",
  ) {
    const fail = (error: unknown) => this.fail(error);
    this.refresher = new ViewRefresher(
      backend.projectView,
      (view, patched) => this.showView(view, patched),
      fail,
      backend.projectTracks,
      backend.projectClips,
      backend.projectDevices,
    );
    this.focus = new MidiFocus(backend.setMidiFocus, fail);
    this.peaks = new PeaksStore(
      { info: backend.peaks, tile: backend.peaksTile },
      () => (this.peaksVersion += 1),
      fail,
    );
  }

  /** Subscribes to the backend's events and live frames; returns the unsubscribe. */
  start(): () => void {
    this.label = backend.webviewLabel();
    const listeners = [
      backend.onSessionChanged((notice) => this.sessionChanged(notice)),
      backend.onJobChanged((job) => this.jobChanged(job)),
      backend.onPeaksReady((ready) => this.peaks.ready(ready)),
      backend.onWindows<SharedSelection>("gui-selection", (shared) => {
        if (shared.from !== this.label) this.setSelection(selection.prune(shared.selection, this.view?.project ?? null));
      }),
    ];
    if (this.role === "main") {
      listeners.push(
        backend.onWindows<WindowIntent>("gui-intent", (intent) => this.intent(intent)),
        backend.onMixerWindow(({ open }) => (this.mixerDetached = open)),
      );
    }
    if (this.role === "mixer") {
      const hello: WindowIntent = { from: this.label, kind: "sync" };
      backend.tellWindows("gui-intent", hello).catch((error: unknown) => this.fail(error));
    }
    backend
      .pluginMenu("effect")
      .then((groups) => (this.effectGroups = groups))
      .catch((error: unknown) => this.fail(error));
    backend
      .subscribeLive((bytes) => {
        try {
          const frame = decodeLiveFrame(bytes);
          const now = performance.now();
          benchHooks.liveReceived?.(frame.sequence, now);
          this.queue.push(frame);
          // A page that is not drawn gets no animation frame to show it at (live/queue.ts); its
          // meters take it at the next one, if one comes.
          if (!waitsForDisplay(document.visibilityState === "hidden", now - this.lastAnimationFrame)) {
            this.undrawn = this.showFrame();
          }
        } catch (error) {
          this.fail(error);
        }
      })
      .catch((error: unknown) => this.fail(error));
    this.refresher.notice();
    const tick = (now: number) => {
      if (this.stopped) return;
      this.lastAnimationFrame = now;
      const t0 = benchHooks.frameWork ? performance.now() : 0;
      const frame = this.showFrame() ?? this.takeUndrawn();
      const update = this.meters.update(now / 1000, frame, this.view?.project ?? null);
      if (frame && update.applied) benchHooks.meterShown?.(frame.sequence, performance.now());
      benchHooks.frameWork?.(performance.now() - t0);
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
    return () => {
      this.stopped = true;
      for (const listener of listeners) void listener.then((stop) => stop());
    };
  }

  // -- events -------------------------------------------------------------------------------------

  sessionChanged(notice: SessionNotice): void {
    this.notice = notice;
    if (notice.changes.structure_changed) this.structure += 1;
    if (notice.kind === "loaded" || notice.kind === "closed") {
      this.peaks.clear();
      this.playhead = START_PLAYHEAD;
      // `projectReplaced` → `show_track(None)`.
      this.devicesTrack = null;
    }
    this.refresher.notice(notice);
  }

  jobChanged(job: JobNotice): void {
    const others = this.jobs.filter((j) => j.id !== job.id);
    this.jobs = job.finished ? others : [...others, job];
  }

  /**
   * A new view: one patched by a track or a device refresh lost no object, so the selection stays;
   * one patched by a clip refresh may have lost notes, so only the notes are pruned.
   */
  private showView(view: ProjectView, patched: ViewPatch = null): void {
    const measure = benchHooks.viewShown;
    const t0 = measure ? performance.now() : 0;
    this.view = view;
    if (patched === null) this.setSelection(selection.prune(this.selection, view.project));
    else if (patched === "clips") this.setSelection(selection.pruneNotes(this.selection, view.project));
    if (measure) {
      flushSync();
      const t1 = performance.now();
      // Reading a box forces the style and layout the update caused.
      void document.body.getBoundingClientRect();
      measure(t1 - t0, performance.now() - t1, patched !== null);
    }
  }

  /** The frame shown on arrival that the meters have not drawn, once. */
  private takeUndrawn(): LiveFrame | null {
    const frame = this.undrawn;
    this.undrawn = null;
    return frame;
  }

  /** The next queued live frame, at a display frame; null when none waits. */
  showFrame(): LiveFrame | null {
    const frame = this.queue.take();
    if (!frame) return null;
    if (this.role === "main" && this.frame?.engine === "absent" && frame.engine !== "absent") {
      // The transport was created: the focus is sent again.
      this.focus.reset();
      this.focus.follow(this.selection.currentTrack);
    }
    this.frame = frame;
    this.answeredState = null;
    this.playhead = playheadFromFrame(this.playhead, frame);
    return frame;
  }

  /** The transport's state and engine: a command's answer, else the latest frame's. */
  transport(): { state: LiveState; engine: EngineState } {
    const engine = this.frame?.engine ?? "absent";
    return { state: this.answeredState ?? this.frame?.state ?? "stopped", engine };
  }

  /** A transport command answered with `state`. */
  answered(state: LiveState): void {
    this.answeredState = state;
  }

  /** The project views and track refreshes requested so far (the meter benchmark's drag). */
  get viewRequests(): { full: number; tracks: number; clips: number; devices: number } {
    return {
      full: this.refresher.requests,
      tracks: this.refresher.trackRequests,
      clips: this.refresher.clipRequests,
      devices: this.refresher.deviceRequests,
    };
  }

  /** Follow during playback, for a timeline `width` pixels wide. */
  follow(width: number): void {
    this.timeline = followPlayhead(this.timeline, this.playhead, width);
  }

  /** A message in the status bar for 4 s (the bridge's `message`). */
  message(text: string): void {
    this.report({ text, duration_ms: 4000, error: false });
  }

  /** A refused command or edit, in the status bar for 8 s. */
  fail(error: unknown): void {
    const text =
      typeof error === "object" && error !== null && "message" in error
        ? String((error as { message: unknown }).message)
        : String(error);
    this.report({ text, duration_ms: 8000, error: true });
  }

  // -- the GUI-only state -------------------------------------------------------------------------

  private setSelection(next: selection.Selection, cause: SelectionCause = "update"): void {
    const before = this.selection;
    if (!selection.sameSelection(next, this.selection)) this.selection = next;
    // The Devices tab follows the selection.
    const shown = devicesTrackAfter(this.devicesTrack, before, next, this.view?.project ?? null, cause);
    if (shown !== this.devicesTrack) this.devicesTrack = shown;
    // The main window holds the MIDI focus.
    if (this.role === "main") this.focus.follow(next.currentTrack);
  }

  /** A selection the user made here: shown, and sent to the other window while there is one. */
  private choose(next: selection.Selection, cause: SelectionCause = "update"): void {
    const changed = !selection.sameSelection(next, this.selection);
    this.setSelection(next, cause);
    if (changed) this.shareSelection();
  }

  private shareSelection(): void {
    if (this.role === "main" && !this.mixerDetached) return;
    const shared: SharedSelection = { from: this.label, selection: this.selection };
    backend.tellWindows("gui-selection", shared).catch((error: unknown) => this.fail(error));
  }

  selectTrack(trackId: string | null, additive = false): void {
    this.choose(selection.selectTrack(this.selection, trackId, additive), "select");
  }

  selectClips(clipIds: string[], additive = false): void {
    this.choose(selection.selectClips(this.selection, clipIds, additive), "select");
  }

  setCurrentClip(clipId: string | null): void {
    this.choose(selection.setCurrentClip(this.selection, clipId));
  }

  setCurrentDevice(deviceId: string | null): void {
    this.choose(selection.setCurrentDevice(this.selection, deviceId), "device");
  }

  selectNotes(noteIds: string[], additive = false): void {
    this.choose(selection.selectNotes(this.selection, noteIds, additive));
  }

  setGrid(gridName: GridName): void {
    this.snap = { ...this.snap, gridName };
  }

  setSnapEnabled(enabled: boolean): void {
    this.snap = { ...this.snap, enabled };
  }

  /** The grid in beats at `beat` ("Bar" through the time signature there). */
  gridBeats(beat = 0): number {
    return gridBeats(this.snap, this.view?.project?.time_signatures ?? null, beat);
  }

  /** `beat` snapped to the grid; ⌥ (`bypass`) only clamps it. */
  snapBeat(beat: number, mode: SnapMode = "nearest", bypass = false): number {
    return snapBeat(this.snap, beat, this.view?.project?.time_signatures ?? null, mode, bypass);
  }

  /** The user put the playhead at `beat` (`PlayheadModel.set_beat`); `playing` when given. */
  setPlayhead(beat: number, playing?: boolean): void {
    const next = { beat: Math.max(0, beat), playing: playing ?? this.playhead.playing };
    if (next.beat !== this.playhead.beat || next.playing !== this.playhead.playing) this.playhead = next;
  }

  /**
   * Seek: the playhead moves, and the transport follows once it exists or while it
   * plays (`MainWindow._seek`).
   */
  seek(beat: number): void {
    this.setPlayhead(beat);
    const { state, engine } = this.transport();
    if (engine !== "absent" || state !== "stopped") {
      backend.transportSeek(beat).catch((error: unknown) => this.fail(error));
    }
  }

  /** MW-27: the piano roll shows `clipId` (its tab is raised), centred on its notes. */
  openClipEditor(clipId: string): void {
    this.setCurrentClip(clipId);
    this.roll = { ...this.roll, centredClip: null };
    this.raiseTab("piano-roll");
  }

  /** The piano roll's view or toolbar values changed. */
  setRoll(patch: Partial<RollSettings>): void {
    this.roll = { ...this.roll, ...patch };
  }

  /** Loads the drum map for the piano roll, once. */
  loadDrumNames(): void {
    if (this.drumNamesRequested) return;
    this.drumNamesRequested = true;
    backend
      .drumMap()
      .then((map) => {
        this.drumNames = { pluginId: map.plugin_id, labels: new Map(map.names.map((n) => [n.pitch, n.label])) };
      })
      .catch((error: unknown) => {
        this.drumNamesRequested = false;
        this.fail(error);
      });
  }

  /** MW-27: the Devices tab shows `trackId` (the main window's; the mixer window asks it). */
  showDevices(trackId: string | null): void {
    if (trackId) this.selectTrack(trackId);
    if (this.role === "mixer") {
      const intent: WindowIntent = { from: this.label, kind: "show-devices", trackId };
      backend.tellWindows("gui-intent", intent).catch((error: unknown) => this.fail(error));
      return;
    }
    // The intent names the panel's track (`showDevices` → `show_track`).
    this.devicesTrack = trackId;
    this.raiseTab("devices");
  }

  /** The mixer window's single-key transport shortcuts run in the main window. */
  shortcut(action: TransportShortcut): void {
    if (this.role === "main") {
      this.onShortcut?.(action);
      return;
    }
    const intent: WindowIntent = { from: this.label, kind: "shortcut", action };
    backend.tellWindows("gui-intent", intent).catch((error: unknown) => this.fail(error));
  }

  private intent(intent: WindowIntent): void {
    if (intent.from === this.label) return;
    if (intent.kind === "show-devices") this.showDevices(intent.trackId);
    else if (intent.kind === "sync") {
      this.mixerDetached = true;
      this.shareSelection();
    } else this.onShortcut?.(intent.action);
  }

  /** `_raise_tab`: the Editors pane shows, on `tab`; the detached mixer's window comes forward. */
  raiseTab(tab: EditorTab): void {
    if (this.role === "mixer") return;
    if (tab === "mixer" && this.mixerDetached) {
      this.detachMixer();
      return;
    }
    if (!this.panes.editors.open) this.setPanes({ ...this.panes, editors: { ...this.panes.editors, open: true } });
    this.editorTab = tab;
  }

  /** The mixer moves to a window of its own (or that window comes forward). */
  detachMixer(): void {
    backend
      .openMixerWindow()
      .then(() => (this.mixerDetached = true))
      .catch((error: unknown) => this.fail(error));
  }

  /** The panes changed: the backend saves them and checks the View menu. */
  setPanes(panes: PaneLayout): void {
    this.panes = panes;
    clearTimeout(this.panesTimer);
    this.panesTimer = setTimeout(() => {
      backend.setPaneLayout(panes).catch((error: unknown) => this.fail(error));
    }, 150);
  }

  private panesTimer: ReturnType<typeof setTimeout> | undefined;


  setTimeline(view: TimelineView): void {
    this.timeline = view;
  }
}
