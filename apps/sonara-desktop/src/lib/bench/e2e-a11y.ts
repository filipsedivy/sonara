// The end-to-end accessibility check, run in the real application on a real
// session by tests/e2e/a11y-tree.mjs (the `bench` build's hook): the page puts the window into
// each state of ADR 0009's VoiceOver checks and of the inventory's accessibility notes, moving the
// focus with real key events where a user would (`bench_post_keys`), and holds it at a checkpoint
// (`bench_checkpoint`) while the harness reads the application's AppKit accessibility tree from
// outside, as VoiceOver reads it. The page checks its own side (the selection, the edits).
//
// Checkpoints, in order: `play` and `play-pressed` (Play, a toggle button); `master` and
// `master-down` (the master's fader, a slider whose value ↓ changes); `tracks` and `tracks-down`
// (the track list; ↓ selects the next track); `clip` (a clip's option, reached by ↓ on the
// arrangement); `note` (a note's option on the piano roll's grid); `meter` (the meters while
// playing); `dialog` and `dialog-closed` (About Sonara takes the focus, Escape gives it back).
// The `shot-*` checkpoints hold the window for the harness's captures of the real window: the
// main window with the mixer, the piano roll and the Devices tab.

import { invoke } from "@tauri-apps/api/core";
import * as backend from "../backend";
import type { Session } from "../session.svelte";
import { sleep } from "./stats";
import type { TrackView } from "../types";

interface Check {
  name: string;
  ok: boolean;
  detail: string;
}

interface KeyTarget {
  key_window: boolean;
  first_responder: string;
}

const postKeys = (...keys: { key: string; command?: boolean; shift?: boolean }[]) =>
  invoke<KeyTarget>("bench_post_keys", { keys });
const checkpoint = (name: string) => invoke<void>("bench_checkpoint", { name });
const choose = (...path: string[]) => invoke<void>("bench_activate_menu", { path });

async function until<T>(what: string, probe: () => T | null | undefined | false, timeoutMs = 10_000): Promise<T> {
  const deadline = performance.now() + timeoutMs;
  for (;;) {
    const value = probe();
    if (value) return value;
    if (performance.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await sleep(40);
  }
}

function trackNamed(session: Session, name: string): TrackView | undefined {
  return session.view?.project?.tracks.find((track) => track.name === name);
}

const query = <T extends HTMLElement>(selector: string) => document.querySelector<T>(selector);

/** Runs the accessibility check and hands the report to the desktop, which writes it and exits. */
export async function runA11yEndToEnd(session: Session, _config: unknown, status: (text: string) => void): Promise<void> {
  const checks: Check[] = [];
  const check = (name: string, ok: boolean, detail = "") => checks.push({ name, ok, detail });
  let error: string | null = null;
  try {
    status("Accessibility: waiting for the project");
    const drums = await until("the Drums 1 track", () => trackNamed(session, "Drums 1"), 20_000);
    const bass = await until("the Bass track", () => trackNamed(session, "Bass"));
    session.editorTab = "mixer";
    await until("the master's strip", () => query('[aria-label="Channel strip Master"]'));
    // The window becomes the key window of the active application, as for a user: only then has
    // it a focused element for assistive technology.
    await postKeys();
    // The main window with the mixer, its meters showing a level.
    await backend.transportPlay(0, true);
    await until("a level on the meters", () => query('[aria-label="Channel strip Master"] [role="meter"]')?.getAttribute("aria-valuetext") !== "silent", 10_000).catch(() => null);
    await sleep(300);
    await checkpoint("shot-main");
    await backend.transportStop();
    await until("stopped", () => session.transport().state === "stopped", 8000).catch(() => false);

    // -- ADR 0009: Play, a toggle button "Play" ----------------------------------------------------
    status("Accessibility: Play");
    const play = await until("Play", () => query<HTMLButtonElement>('[data-testid="play"]'));
    play.focus();
    await sleep(300);
    await checkpoint("play");
    play.click();
    const rolling = await until("playing", () => session.transport().state === "playing", 8000).catch(() => false);
    play.focus();
    await sleep(400);
    await checkpoint("play-pressed");
    check("Play plays on the simulated device, its pressed state follows", rolling && play.getAttribute("aria-pressed") === "true");
    play.click();
    await until("stopped", () => session.transport().state === "stopped", 8000).catch(() => false);

    // -- ADR 0009: the master's fader, a slider the arrow keys change -------------------------------
    status("Accessibility: the master's fader");
    const fader = await until("the master's fader", () => query('[aria-label="Channel strip Master"] [role="slider"][aria-label="Master volume"]'));
    fader.focus();
    await sleep(300);
    await checkpoint("master");
    const before = fader.getAttribute("aria-valuetext");
    await postKeys({ key: "ArrowDown" });
    const lowered = await until("the fader's new value", () => fader.getAttribute("aria-valuetext") !== before && session.view?.project?.master.mixer.volume_db !== 0, 5000).catch(() => false);
    await sleep(400);
    await checkpoint("master-down");
    check("a hardware ↓ on the master's fader lowers the master", lowered, `${before} → ${fader.getAttribute("aria-valuetext")}, ${session.view?.project?.master.mixer.volume_db}`);
    if (lowered) {
      await backend.undo();
      await until("the undone fader", () => session.view?.project?.master.mixer.volume_db === 0, 5000).catch(() => false);
    }

    // -- ADR 0009: the track list, a list box whose ↓ selects the next track -------------------------
    status("Accessibility: the track list");
    const first = await until("the first track's row", () => query(`[role="listbox"][aria-label="Tracks"] [role="option"][data-track="${drums.id}"]`));
    session.selectTrack(drums.id);
    await sleep(100);
    first.focus();
    await sleep(300);
    await checkpoint("tracks");
    await postKeys({ key: "ArrowDown" });
    const moved = await until("the Bass row focused", () => document.activeElement?.getAttribute("data-track") === bass.id, 5000).catch(() => false);
    await sleep(300);
    await checkpoint("tracks-down");
    check("a hardware ↓ in the track list selects and focuses the next track", moved && session.selection.currentTrack === bass.id);

    // -- the arrangement's clips -------------------------------------------------------------------------
    status("Accessibility: the clips");
    const arrangement = await until("the arrangement", () => query('[role="application"][aria-label="Arrangement"]'));
    session.selectClips([]);
    arrangement.focus();
    await sleep(200);
    await postKeys({ key: "ArrowDown" });
    const clipFocused = await until("a clip's option focused", () => document.activeElement?.getAttribute("role") === "option" && document.activeElement.closest('[aria-label="Clips"]'), 5000).catch(() => null);
    await sleep(300);
    await checkpoint("clip");
    check("a hardware ↓ on the arrangement focuses and selects the first clip", !!clipFocused && session.selection.clips.length === 1, document.activeElement?.getAttribute("aria-label") ?? "");

    // -- the piano roll's notes ----------------------------------------------------------------------------
    status("Accessibility: the notes");
    const riff = bass.clips[0];
    session.openClipEditor(riff.id);
    const noteOption = await until("a note's option", () => query('[role="listbox"][aria-label="Notes"] [role="option"]'), 5000);
    noteOption.click();
    noteOption.focus();
    await sleep(300);
    await checkpoint("note");
    check("a note's option takes the focus", document.activeElement === noteOption, noteOption.getAttribute("aria-label") ?? "");
    await checkpoint("shot-piano-roll");
    session.showDevices(drums.id);
    await until("the Devices tab's card", () => query('[aria-label="Devices"] [role="group"][aria-label^="Device "]'), 5000).catch(() => null);
    await sleep(400);
    await checkpoint("shot-devices");

    // -- the meters ------------------------------------------------------------------------------------------
    status("Accessibility: the meters");
    session.editorTab = "mixer";
    await backend.transportPlay(0, true);
    const meter = await until("a level on the master's meter", () => {
      const element = query('[aria-label="Channel strip Master"] [role="meter"]');
      return element && element.getAttribute("aria-valuetext") !== "silent" ? element : null;
    }, 10_000).catch(() => null);
    const level = meter?.getAttribute("aria-valuetext") ?? "silent";
    await checkpoint("meter");
    check("the master's meter has a level while playing", !!meter, level);
    await backend.transportStop();

    // -- a dialog takes the focus and gives it back ------------------------------------------------------------
    status("Accessibility: a dialog");
    arrangement.focus();
    await sleep(200);
    await choose("Sonara", "About Sonara");
    const dialog = await until("About Sonara", () => query("dialog[open]"), 5000);
    await sleep(400);
    await checkpoint("dialog");
    await postKeys({ key: "Tab" }, { key: "Tab" }, { key: "Tab" }, { key: "Tab" }, { key: "Tab" });
    await sleep(300);
    const kept = dialog.contains(document.activeElement);
    await postKeys({ key: "Escape" });
    const closed = await until("the dialog closed", () => !query("dialog[open]"), 5000).catch(() => false);
    await sleep(300);
    await checkpoint("dialog-closed");
    check("hardware Tabs stay in the dialog, and Escape gives the focus back to the arrangement", kept && closed && document.activeElement === arrangement, `${kept} ${closed} ${document.activeElement?.getAttribute("aria-label")}`);
  } catch (caught) {
    error = caught instanceof Error ? caught.message : String(caught);
  }
  const report = { suite: "e2e-a11y", valid: error === null && checks.every((c) => c.ok), error, checks };
  await invoke("bench_report", { report });
}
