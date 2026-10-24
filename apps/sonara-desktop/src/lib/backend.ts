// The typed client of the desktop's commands and events (src-tauri/src/commands.rs). Every
// computation behind them is sonara-app's; this module only calls them.

import { Channel, invoke } from "@tauri-apps/api/core";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type {
  ClipsView,
  DevicesView,
  DrumMap,
  Edit,
  EditOutcome,
  EngineStatus,
  EqResponse,
  JobNotice,
  PaneLayout,
  PeaksInfo,
  PeaksReady,
  PluginGroup,
  ProjectView,
  RecoverableSession,
  SessionNotice,
  ShellStatus,
  StatusMessage,
  Stopped,
  TracksView,
  ViewCommand,
} from "./types";
import type { MenuContext } from "./shell/editing";
import type { MenuCommand } from "./shell/menuCommands";

/** The agent label and the startup's messages (handed out once). */
export function shellStatus(): Promise<ShellStatus> {
  return invoke<ShellStatus>("shell_status");
}

/** The stored `theme` setting (A1), as the settings file has it. */
export function guiTheme(): Promise<string> {
  return invoke<string>("gui_theme");
}

/** The stored `theme` setting changed (the Settings dialog's OK), for every window. */
export function onTheme(handler: (theme: string) => void): Promise<UnlistenFn> {
  return listen<string>("theme", (event) => handler(event.payload));
}

/** The unsaved scratch sessions to offer for recovery. */
export function recoverableSessions(): Promise<RecoverableSession[]> {
  return invoke<RecoverableSession[]>("recoverable_sessions");
}

/** Restores the session in `workspace`, one of {@link recoverableSessions}. */
export function recoverSession(workspace: string): Promise<void> {
  return invoke<void>("recover_session", { workspace });
}

/** The project view the panels draw. */
export function projectView(): Promise<ProjectView> {
  return invoke<ProjectView>("project_view");
}

/** The views of some tracks, for the refresh of an edit that changed only their mixer settings. */
export function projectTracks(trackIds: string[]): Promise<TracksView> {
  return invoke<TracksView>("project_tracks", { trackIds });
}

/** The views of some MIDI clips, for the refresh of an edit that changed only their notes. */
export function projectClips(clipIds: string[]): Promise<ClipsView> {
  return invoke<ClipsView>("project_clips", { clipIds });
}

/** The views of some devices, for the refresh of an edit that changed only their state. */
export function projectDevices(deviceIds: string[]): Promise<DevicesView> {
  return invoke<DevicesView>("project_devices", { deviceIds });
}

/** Runs one edit with the source "gui"; rejects with an `ErrorNotice` (`{code, message}`). */
export function edit(change: Edit): Promise<EditOutcome> {
  return invoke<EditOutcome>("edit", { edit: change });
}

/** Undoes the last step; the status bar says which. */
export function undo(): Promise<EditOutcome> {
  return invoke<EditOutcome>("undo");
}

/** Redoes the last undone step; the status bar says which. */
export function redo(): Promise<EditOutcome> {
  return invoke<EditOutcome>("redo");
}

/** Ends a gesture: the next edit starts a new undo step. */
export function sealUndo(): Promise<void> {
  return invoke<void>("seal_undo");
}

/** Gives the live MIDI focus to the selection's current track (an instrument track), or none. */
export function setMidiFocus(trackId: string | null): Promise<void> {
  return invoke<void>("set_midi_focus", { trackId });
}

/** GUI-only: plays (`on: true`) or releases one preview note of `trackId`'s instrument, for a
 * key clicked in the piano roll's keyboard. */
export function auditionNote(trackId: string, pitch: number, velocity: number, on: boolean): Promise<void> {
  return invoke<void>("audition_note", { trackId, pitch, velocity, on });
}

/** The parametric EQ's response curve. */
export function eqResponse(deviceId: string): Promise<EqResponse> {
  return invoke<EqResponse>("eq_response", { deviceId });
}

/** A waveform pyramid's state; a pending one is followed by `peaks-ready`. */
export function peaks(assetId: string): Promise<PeaksInfo> {
  return invoke<PeaksInfo>("peaks", { assetId });
}

/** The URL of a pyramid tile on the `peaks:` scheme (macOS: `peaks://localhost/...`). */
export function peaksTileUrl(
  assetId: string,
  sampleRate: number,
  level: number,
  index: number,
): string {
  const path = `tile/${encodeURIComponent(assetId)}/${sampleRate}/${level}/${index}`;
  return `peaks://localhost/${path}`;
}

/** One tile of a ready pyramid, as bytes (`decodeTile` in peaks.ts reads them). */
export async function peaksTile(
  assetId: string,
  sampleRate: number,
  level: number,
  index: number,
): Promise<ArrayBuffer> {
  const response = await fetch(peaksTileUrl(assetId, sampleRate, level, index));
  if (!response.ok) throw new Error(`peaks tile ${level}/${index} of ${assetId}: ${response.status}`);
  return response.arrayBuffer();
}

/** Subscribes `handler` to the live frames; the bytes are decoded by frame.ts. */
export async function subscribeLive(handler: (frame: ArrayBuffer) => void): Promise<void> {
  const channel = new Channel<ArrayBuffer>();
  channel.onmessage = handler;
  await invoke("subscribe_live", { channel });
}

// -- the transport bar -------------------------------------------------------------------

/** TB-01: plays from `startBeat`, looping the loop range when `looped`. */
export function transportPlay(startBeat: number, looped: boolean): Promise<Record<string, unknown>> {
  return invoke("transport_play", { startBeat, looped });
}

/** TB-03: stops; never creates the transport. */
export function transportStop(): Promise<Stopped> {
  return invoke<Stopped>("transport_stop");
}

/** TB-04: records the armed tracks from `startBeat`. */
export function transportRecord(startBeat: number): Promise<Record<string, unknown>> {
  return invoke("transport_record", { startBeat });
}

/** TB-02, TL-12, MW-27: moves the transport to `beat` (creates the transport). */
export function transportSeek(beat: number): Promise<Record<string, unknown>> {
  return invoke("transport_seek", { beat });
}

/** The engine label's status; null before the transport exists. */
export function transportStatus(): Promise<EngineStatus | null> {
  return invoke<EngineStatus | null>("transport_status");
}

// -- imports, drops and the window ---------------------------------------------------------

/** TL-09, MW-24: imports `paths` on a job into `trackId` (or new tracks) at `startBeat`. */
export function importAudioFiles(
  paths: string[],
  trackId: string | null,
  startBeat: number,
): Promise<string> {
  return invoke<string>("import_audio_files", { paths, trackId, startBeat });
}

/** TL-08 "Import Audio Here…": the open panel, then an import job. */
export function importAudioDialog(trackId: string | null, startBeat: number): Promise<void> {
  return invoke<void>("import_audio_dialog", { trackId, startBeat });
}

/** MW-24: opens a dropped `.sonara`, after the unsaved-changes prompt. */
export function openProjectFile(path: string): Promise<void> {
  return invoke<void>("open_project_file", { path });
}

/** The panes the page shows now: saved with the window, checked in the View menu. */
export function setPaneLayout(panes: PaneLayout): Promise<void> {
  return invoke<void>("set_pane_layout", { panes });
}

/** A position of a drag over the window, in physical pixels. */
export interface DropPosition {
  x: number;
  y: number;
}

/** What a drag of files over the window does at each stage. */
export interface DragDropHandlers {
  enter(paths: string[], position: DropPosition): void;
  over(position: DropPosition): void;
  drop(paths: string[], position: DropPosition): void;
  leave(): void;
}

/** Files dragged over the window from the Finder (Tauri's drag-and-drop event). */
export function onDragDrop(handlers: DragDropHandlers): Promise<UnlistenFn> {
  return getCurrentWebview().onDragDropEvent((event) => {
    const payload = event.payload;
    if (payload.type === "enter") handlers.enter(payload.paths, payload.position);
    else if (payload.type === "over") handlers.over(payload.position);
    else if (payload.type === "drop") handlers.drop(payload.paths, payload.position);
    else handlers.leave();
  });
}

/** View menu items (panes and editor tabs). */
export function onViewCommand(handler: (command: ViewCommand) => void): Promise<UnlistenFn> {
  return listen<ViewCommand>("view-command", (event) => handler(event.payload));
}

/** The Edit, Track and Transport menus' commands, to the main window only. */
export function onMenuCommand(handler: (command: MenuCommand) => void): Promise<UnlistenFn> {
  return listen<MenuCommand>("menu-command", (event) => handler(event.payload));
}

/** Tells the desktop the focused editor and whether a dialog is open (the Edit menu). */
export function setMenuContext(context: MenuContext): Promise<void> {
  return invoke<void>("set_menu_context", { context });
}

/** Status-bar messages, for example an agent's edit. */
export function onMessage(handler: (message: StatusMessage) => void): Promise<UnlistenFn> {
  return listen<StatusMessage>("message", (event) => handler(event.payload));
}

/** Every session event, whoever caused it. */
export function onSessionChanged(handler: (notice: SessionNotice) => void): Promise<UnlistenFn> {
  return listen<SessionNotice>("session-changed", (event) => handler(event.payload));
}

/** Every change of a background job. */
export function onJobChanged(handler: (job: JobNotice) => void): Promise<UnlistenFn> {
  return listen<JobNotice>("job-changed", (event) => handler(event.payload));
}

/** Every waveform pyramid that became ready or failed. */
export function onPeaksReady(handler: (ready: PeaksReady) => void): Promise<UnlistenFn> {
  return listen<PeaksReady>("peaks-ready", (event) => handler(event.payload));
}

// -- the mixer and its window ---------------------------------------------------------------

/** MX-06: the picker of every registered plugin of `kind`, grouped by category. */
export function pluginMenu(kind: "effect" | "instrument"): Promise<PluginGroup[]> {
  return invoke<PluginGroup[]>("plugin_menu", { kind });
}

/** Opens the mixer in a window of its own, or brings that window to the front. */
export function openMixerWindow(): Promise<void> {
  return invoke<void>("open_mixer_window");
}

/** The mixer's own window opened or closed (`mixer-window`). */
export function onMixerWindow(handler: (state: { open: boolean }) => void): Promise<UnlistenFn> {
  return listen<{ open: boolean }>("mixer-window", (event) => handler(event.payload));
}

// -- the piano roll ---------------------------------------------------------------------------

/** The drum kit's plugin and the GM names of its notes. */
export function drumMap(): Promise<DrumMap> {
  return invoke<DrumMap>("drum_map");
}

/** The label of this page's web view: "main", or "mixer" in the mixer's own window. */
export function webviewLabel(): string {
  return getCurrentWebview().label;
}

/** Sends `payload` to the other windows' pages (`event` is `gui-selection` or `gui-intent`). */
export function tellWindows<T>(event: "gui-selection" | "gui-intent", payload: T): Promise<void> {
  return emit(event, payload);
}

/** What another window's page sent with {@link tellWindows}. */
export function onWindows<T>(event: "gui-selection" | "gui-intent", handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (message) => handler(message.payload));
}

/** The benchmark's options (src-tauri/src/bench.rs); the command exists only in a `bench` build. */
export interface BenchOptions {
  out: string;
  quick: boolean;
  suite: "timeline" | "e2e" | "e2e-dialogs" | "e2e-a11y" | "meters" | "pianoroll" | "playback";
  media: string | null;
}

/** The benchmark's options, or null outside benchmark mode; rejects in a normal build. */
export function benchConfig(): Promise<BenchOptions | null> {
  return invoke<BenchOptions | null>("bench_config");
}
