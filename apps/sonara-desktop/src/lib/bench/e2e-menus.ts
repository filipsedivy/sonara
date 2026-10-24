// The end-to-end check of the menus and shortcuts, run in the real
// application on a real session after the dialogs' checks (e2e-dialogs.ts, which
// scripts/e2e-dialogs.mjs runs): the page drives the native menu bar through the `bench` build's
// hooks (src-tauri/src/bench.rs) and checks what the application did.
//
// - The native menu bar, read from AppKit, is sonara-app's model (tests/fixtures/menus-v1.json):
//   the items, their order and key equivalents, Open Recent, and the Edit menu's texts and
//   enabled states for the focused editor.
// - Native items are chosen as a click chooses them (`performActionForItemAtIndex`): the Edit
//   menu's items on the focused editor, Undo and Redo, the Track, Transport and View menus, the
//   dialogs, and the File menu's flows, whose native prompts and panels the hook answers and
//   records (their wording is checked).
// - Real key events, posted into the application's event queue, reach the focused editor before
//   the menu: a hardware ⌘A, ⌘D and ⌘E go to the timeline and the piano roll, a typed R stays in
//   its text field, Space plays, and a key while a dialog is open runs nothing. Step 11: ⌘Z and
//   ⇧⌘Z in a text field undo and redo its typing, and ⌘Z elsewhere is the menu's Undo.

import { invoke } from "@tauri-apps/api/core";
import * as backend from "../backend";
import type { Session } from "../session.svelte";
import { sleep } from "./stats";
import { benchHooks } from "./hooks";
import type { MidiClipView, StatusMessage, TrackView } from "../types";
import menusFixture from "../../../tests/fixtures/menus-v1.json" with { type: "json" };

type Check = (name: string, ok: boolean, detail?: string) => void;

interface NativeItem {
  title: string;
  separator: boolean;
  key: string;
  modifiers: string[];
  enabled: boolean;
  checked: boolean;
  items: NativeItem[];
}

type BenchAnswer = { choice: "Ok" | "Yes" | "No" | "Save" | "Discard" | "Cancel" } | { path: string } | { paths: string[] };

interface Asked {
  kind: string;
  title: string;
  text: string;
}

interface MenuHandled {
  id: string;
  declined: boolean;
}

interface KeyTarget {
  key_window: boolean;
  first_responder: string;
}

interface FixtureEntry {
  type: "item" | "check" | "separator" | "system" | "open-recent";
  id?: string;
  text?: string;
  shortcut?: string | null;
}

const nativeMenu = () => invoke<NativeItem[]>("bench_native_menu");
const choose = (...path: string[]) => invoke<void>("bench_activate_menu", { path });
const answer = (...answers: BenchAnswer[]) => invoke<void>("bench_answer_prompts", { answers });
const asked = () => invoke<[Asked[], BenchAnswer[]]>("bench_prompts_asked");
const menuLog = () => invoke<MenuHandled[]>("bench_menu_log");
const postKeys = (...keys: { key: string; command?: boolean; shift?: boolean }[]) =>
  invoke<KeyTarget>("bench_post_keys", { keys });

async function until<T>(what: string, probe: () => T | null | undefined | false, timeoutMs = 10_000): Promise<T> {
  const deadline = performance.now() + timeoutMs;
  for (;;) {
    const value = probe();
    if (value) return value;
    if (performance.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await sleep(40);
  }
}

/** Waits up to `timeoutMs` for `probe`; whether it held (for checks that may stay false). */
async function within(probe: () => boolean, timeoutMs: number): Promise<boolean> {
  const deadline = performance.now() + timeoutMs;
  while (performance.now() < deadline) {
    if (probe()) return true;
    await sleep(40);
  }
  return probe();
}

/** A native key equivalent as writes it: "⇧⌘S", "⌘,", "Space", "Home". */
function glyphs(item: NativeItem): string | null {
  if (!item.key) return null;
  const key = item.key === " " ? "Space" : item.key === "" ? "Home" : item.key.toUpperCase();
  return `${item.modifiers.includes("shift") ? "⇧" : ""}${item.modifiers.includes("command") ? "⌘" : ""}${key}`;
}

const DYNAMIC = new Set(["undo", "redo", "split-clips", "duplicate-clips", "delete-clips"]);

/**
 * Whether `native`'s key equivalent is the model's `shortcut`. AppKit localizes a key equivalent
 * the keyboard layout cannot type unshifted (macOS 12+): on a Czech layout ⌘1 becomes ⌘+, still on
 * the physical 1 key; so a digit's modifiers must match and its character may differ.
 */
function sameShortcut(shortcut: string | null, native: NativeItem): boolean {
  const shown = glyphs(native);
  if (shortcut === shown) return true;
  const digit = shortcut?.match(/^([⇧⌘]*)(\d)$/);
  return !!digit && !!shown && shown.startsWith(digit[1]) && shown.length === digit[1].length + 1;
}

/** Where the native menu bar differs from the model (dynamic texts aside). */
function compareMenuBar(native: NativeItem[]): string[] {
  const problems: string[] = [];
  const menus = menusFixture.menus as { title: string; entries: FixtureEntry[] }[];
  menus.forEach((menu, index) => {
    const bar = native[index];
    if (!bar || bar.title !== menu.title) {
      problems.push(`menu ${index}: ${bar?.title} instead of ${menu.title}`);
      return;
    }
    // AppKit may append its own items (the window list, Enter Full Screen): the model is a prefix.
    menu.entries.forEach((entry, at) => {
      const item = bar.items[at];
      const where = `${menu.title} ${at}`;
      if (!item) problems.push(`${where}: missing`);
      else if (entry.type === "separator") {
        if (!item.separator) problems.push(`${where}: ${item.title} instead of a separator`);
      } else if (item.separator) problems.push(`${where}: a separator instead of ${entry.text ?? entry.type}`);
      else if (entry.type === "system") {
        if (!item.title) problems.push(`${where}: the system's ${entry.text} has no title`);
      } else if (entry.type === "open-recent") {
        if (item.title !== "Open Recent" || item.items.length === 0) problems.push(`${where}: Open Recent is ${item.title}`);
      } else {
        const text = entry.text ?? "";
        const titled = entry.id && DYNAMIC.has(entry.id) ? item.title.length > 0 : item.title === text;
        if (!titled) problems.push(`${where}: “${item.title}” instead of “${text}”`);
        if (!sameShortcut(entry.shortcut ?? null, item)) problems.push(`${where} ${text}: ${glyphs(item)} instead of ${entry.shortcut}`);
        if (entry.type === "check" && !item.checked) problems.push(`${where} ${text}: not checked`);
      }
    });
  });
  return problems;
}

function submenu(native: NativeItem[], ...path: string[]): NativeItem | undefined {
  let items = native;
  let found: NativeItem | undefined;
  for (const title of path) {
    found = items.find((item) => item.title === title);
    items = found?.items ?? [];
  }
  return found;
}

function edit(native: NativeItem[]): { title: string; enabled: boolean }[] {
  const items = submenu(native, "Edit")?.items ?? [];
  return items.slice(3, 6).map((item) => ({ title: item.title, enabled: item.enabled }));
}

const trackNamed = (session: Session, name: string): TrackView | undefined =>
  session.view?.project?.tracks.find((track) => track.name === name);

const history = (session: Session) => session.view?.project?.history;

const waitIdle = () => sleep(150);

export async function runMenusEndToEnd(
  session: Session,
  sandbox: string,
  check: Check,
  status: (text: string) => void,
): Promise<void> {
  const messages: string[] = [];
  const stopMessages = await backend.onMessage((message: StatusMessage) => messages.push(message.text));
  const trusted: { key: string; meta: boolean; target: string }[] = [];
  const recordKeys = (event: KeyboardEvent) => {
    if (event.isTrusted) trusted.push({ key: event.key, meta: event.metaKey, target: (event.target as Element | null)?.tagName ?? "" });
  };
  window.addEventListener("keydown", recordKeys, { capture: true });
  const blur = () => (document.activeElement as HTMLElement | null)?.blur?.();
  const message = (prefix: string) => until(`the message “${prefix}…”`, () => messages.find((m) => m.startsWith(prefix)));
  try {
    await until("no dialog", () => !document.querySelector("dialog[open]"));
    await menuLog();
    await asked();
    blur();
    await waitIdle();

    // -- the native menu bar is the model ------------------------------------------------------------
    status("End to end: the native menu bar");
    let native = await nativeMenu();
    const problems = compareMenuBar(native);
    check("the native menu bar is sonara-app's model (items, order, key equivalents)", problems.length === 0, problems.slice(0, 6).join("; "));
    const recent = submenu(native, "File", "Open Recent")?.items ?? [];
    check("Open Recent lists the opened project", recent.some((item) => item.title === "E2E" && item.enabled), recent.map((i) => i.title).join(", "));

    // -- G9: the Edit menu follows the focused editor ---------------------------------------------------
    status("End to end: the Edit menu and the focused editor");
    const keys = await until("the Keys 2 track", () => trackNamed(session, "Keys 2"));
    const riff = keys.clips[0] as MidiClipView;
    session.openClipEditor(riff.id);
    const grid = await until("the piano roll's grid", () => benchHooks.pianoRoll?.grid);
    grid.focus();
    const notesItems = await editItemsWhen((items) => items[1]?.title === "Duplicate Notes");
    check(
      "G9: with the piano roll focused, the Edit menu reads Duplicate Notes and Delete Notes, and Split is off",
      notesItems[0]?.enabled === false && notesItems[1]?.title === "Duplicate Notes" && notesItems[2]?.title === "Delete Notes",
      JSON.stringify(notesItems),
    );
    session.raiseTab("devices");
    session.showDevices(keys.id);
    const knob = await until("a device knob", () => document.querySelector<HTMLElement>('[data-editor="devices"] [role="slider"]'));
    knob.focus();
    const deviceItems = await editItemsWhen((items) => items.every((i) => !i.enabled));
    check("G9: with a device card focused, the Edit menu's clip items are off", deviceItems.every((i) => !i.enabled), JSON.stringify(deviceItems));
    const arrangement = await until("the arrangement", () => document.querySelector<HTMLElement>('[aria-label="Arrangement"]'));
    arrangement.focus();
    const clipItems = await editItemsWhen((items) => items[1]?.title === "Duplicate Clips");
    check(
      "G9: with the timeline focused, the Edit menu has the clip items",
      clipItems.map((i) => i.title).join("|") === "Split Clips at Playhead|Duplicate Clips|Delete Clips" && clipItems.every((i) => i.enabled),
      JSON.stringify(clipItems),
    );

    // -- the Edit menu's items, chosen -----------------------------------------------------------------
    session.selectClips([riff.id]);
    arrangement.focus();
    await sleep(200);
    const clipsBefore = trackNamed(session, "Keys 2")?.clips.length ?? 0;
    await choose("Edit", "Duplicate Clips");
    const duplicated = await until("the duplicate", () => (trackNamed(session, "Keys 2")?.clips.length ?? 0) > clipsBefore && trackNamed(session, "Keys 2"));
    check(
      "G9: Edit ▸ Duplicate Clips on the timeline is the canvas's one step after the span, the copy selected",
      history(session)?.undo === "Duplicate Clips" &&
        duplicated.clips.some((c) => c.start === riff.start + riff.length) &&
        session.selection.clips.length === 1 &&
        session.selection.clips[0] !== riff.id,
      `${history(session)?.undo}; starts ${duplicated.clips.map((c) => c.start).join(",")}`,
    );
    native = await nativeMenu();
    const undoTitle = submenu(native, "Edit")?.items[0]?.title ?? "";
    check("Edit ▸ Undo is named for the last step", undoTitle === "Undo Duplicate Clips", undoTitle);
    await choose("Edit", "Undo Duplicate Clips");
    await until("the undone duplicate", () => trackNamed(session, "Keys 2")?.clips.length === clipsBefore);
    native = await nativeMenu();
    const redoItem = submenu(native, "Edit")?.items[1];
    check("Edit ▸ Undo takes the step back, and Redo is named for it", redoItem?.title === "Redo Duplicate Clips" && redoItem.enabled, redoItem?.title);
    await choose("Edit", "Redo Duplicate Clips");
    await until("the redone duplicate", () => (trackNamed(session, "Keys 2")?.clips.length ?? 0) > clipsBefore);
    await backend.undo();
    await until("the undone redo", () => trackNamed(session, "Keys 2")?.clips.length === clipsBefore);

    session.selectClips([riff.id]);
    session.setPlayhead(2);
    arrangement.focus();
    await sleep(200);
    await choose("Edit", "Split Clips at Playhead");
    const split = await until("the split", () => (trackNamed(session, "Keys 2")?.clips.length ?? 0) === clipsBefore + 1 && trackNamed(session, "Keys 2"));
    check("Edit ▸ Split Clips at Playhead is one Split Clips step", history(session)?.undo === "Split Clips", `${history(session)?.undo}; ${split.clips.length} clips`);
    await backend.undo();
    await until("the undone split", () => trackNamed(session, "Keys 2")?.clips.length === clipsBefore);

    session.openClipEditor(riff.id);
    const rollGrid = await until("the piano roll's grid", () => benchHooks.pianoRoll?.grid);
    const notesBefore = riff.notes.length;
    session.selectNotes([riff.notes[0].id]);
    rollGrid.focus();
    await editItemsWhen((items) => items[1]?.title === "Duplicate Notes");
    await choose("Edit", "Duplicate Notes");
    const withCopy = await until("the duplicated note", () => {
      const clip = trackNamed(session, "Keys 2")?.clips.find((c) => c.id === riff.id) as MidiClipView | undefined;
      return clip && clip.notes.length === notesBefore + 1 ? clip : null;
    });
    check(
      "G9: Edit ▸ Duplicate Notes with the piano roll focused duplicates the notes, not the clip",
      withCopy.notes.length === notesBefore + 1 && trackNamed(session, "Keys 2")?.clips.length === clipsBefore,
      `${withCopy.notes.length} notes, ${history(session)?.undo}`,
    );
    await backend.undo();
    await until("the undone note", () => (trackNamed(session, "Keys 2")?.clips.find((c) => c.id === riff.id) as MidiClipView | undefined)?.notes.length === notesBefore);

    // -- real keys: the focused editor gets them before the menu ------------------------------------
    status("End to end: hardware keys");
    await menuLog();
    session.selectClips([riff.id]);
    arrangement.focus();
    await sleep(200);
    trusted.length = 0;
    const target = await postKeys({ key: "d", command: true });
    const keyDuplicated = await within(() => (trackNamed(session, "Keys 2")?.clips.length ?? 0) === clipsBefore + 1, 3000);
    let log = await menuLog();
    check(
      "a hardware ⌘D reaches the focused timeline before the menu: one Duplicate Clips step, no menu item",
      keyDuplicated && history(session)?.undo === "Duplicate Clips" && trusted.some((k) => k.key === "d" && k.meta) && !log.some((e) => e.id === "duplicate-clips"),
      `key window ${target.key_window}, first responder ${target.first_responder}; keys ${JSON.stringify(trusted)}; menu ${JSON.stringify(log)}; ${history(session)?.undo}`,
    );
    if (keyDuplicated) {
      // Step 11: outside a text field ⌘Z is the menu's Undo, the project's last step.
      arrangement.focus();
      await sleep(150);
      await menuLog();
      await postKeys({ key: "z", command: true });
      const keyUndone = await within(() => trackNamed(session, "Keys 2")?.clips.length === clipsBefore, 3000);
      log = await menuLog();
      check(
        "a hardware ⌘Z with the timeline focused is the menu's Undo: the project's last step goes",
        keyUndone && log.some((e) => e.id === "undo" && !e.declined),
        `menu ${JSON.stringify(log)}; ${history(session)?.undo}`,
      );
      if (!keyUndone) {
        await backend.undo();
        await until("the undone key duplicate", () => trackNamed(session, "Keys 2")?.clips.length === clipsBefore);
      }
    }

    session.selectClips([riff.id]);
    session.setPlayhead(2);
    arrangement.focus();
    await sleep(200);
    await postKeys({ key: "e", command: true });
    const keySplit = await within(() => (trackNamed(session, "Keys 2")?.clips.length ?? 0) === clipsBefore + 1, 3000);
    log = await menuLog();
    check(
      "a hardware ⌘E reaches the focused timeline: one Split Clips step, no menu item",
      keySplit && history(session)?.undo === "Split Clips" && !log.some((e) => e.id === "split-clips"),
      `${history(session)?.undo}; menu ${JSON.stringify(log)}`,
    );
    if (keySplit) {
      await backend.undo();
      await until("the undone key split", () => trackNamed(session, "Keys 2")?.clips.length === clipsBefore);
    }

    session.openClipEditor(riff.id);
    const keyGrid = await until("the piano roll's grid", () => benchHooks.pianoRoll?.grid);
    session.selectNotes([]);
    keyGrid.focus();
    await sleep(200);
    trusted.length = 0;
    await postKeys({ key: "a", command: true });
    const all = await within(() => session.selection.notes.length === notesBefore, 3000);
    check(
      "a hardware ⌘A reaches the focused piano roll grid and selects every note (not the menu's Select All)",
      all && trusted.some((k) => k.key === "a" && k.meta && k.target !== "INPUT"),
      `${session.selection.notes.length} of ${notesBefore} selected; keys ${JSON.stringify(trusted)}`,
    );
    session.selectNotes([riff.notes[0].id]);
    keyGrid.focus();
    await sleep(150);
    await postKeys({ key: "d", command: true });
    const keyNotes = await within(
      () => (trackNamed(session, "Keys 2")?.clips.find((c) => c.id === riff.id) as MidiClipView | undefined)?.notes.length === notesBefore + 1,
      3000,
    );
    log = await menuLog();
    check(
      "a hardware ⌘D reaches the focused piano roll: the note is duplicated, not the clip",
      keyNotes && trackNamed(session, "Keys 2")?.clips.length === clipsBefore && !log.some((e) => e.id === "duplicate-clips"),
      `menu ${JSON.stringify(log)}`,
    );
    if (keyNotes) {
      await backend.undo();
      await until("the undone key note", () => (trackNamed(session, "Keys 2")?.clips.find((c) => c.id === riff.id) as MidiClipView | undefined)?.notes.length === notesBefore);
    }

    const name = await until("the project name field", () => document.querySelector<HTMLInputElement>("#project-name"));
    const typedBefore = name.value;
    name.focus();
    await sleep(150);
    const recordingBefore = session.recording;
    await postKeys({ key: "r" });
    const typed = await within(() => name.value.length === typedBefore.length + 1 && name.value.toLowerCase().includes("r"), 2000);
    await sleep(300);
    log = await menuLog();
    check(
      "a typed R stays in the focused text field: no Record",
      typed && session.recording === recordingBefore && !log.some((e) => e.id === "record" && !e.declined),
      `field “${name.value}”; menu ${JSON.stringify(log)}`,
    );
    // Step 11: ⌘Z and ⇧⌘Z in a text field are the field's own, as the prototype's line edits:
    // the typing goes back and forth, the project's history stays, and the menu gets neither.
    const undoBefore = history(session)?.undo ?? null;
    const versionBefore = session.view?.version ?? -1;
    const withR = name.value;
    await menuLog();
    await postKeys({ key: "z", command: true });
    const fieldUndone = await within(() => name.value === typedBefore, 2000);
    await postKeys({ key: "z", command: true, shift: true });
    const fieldRedone = await within(() => name.value === withR, 2000);
    await sleep(300);
    log = await menuLog();
    check(
      "a hardware ⌘Z in a text field undoes its typing, not the project, and ⇧⌘Z redoes it",
      fieldUndone &&
        fieldRedone &&
        history(session)?.undo === undoBefore &&
        (session.view?.version ?? -1) === versionBefore &&
        !log.some((e) => e.id === "undo" || e.id === "redo"),
      `undone ${fieldUndone}, redone ${fieldRedone}, field “${name.value}”; undo ${history(session)?.undo}; menu ${JSON.stringify(log)}`,
    );
    name.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
    name.blur();
    blur();
    await sleep(150);

    await postKeys({ key: "Space" });
    const played = await within(() => session.transport().state === "playing", 4000);
    await postKeys({ key: "Space" });
    const stopped = await within(() => session.transport().state === "stopped", 4000);
    log = await menuLog();
    check(
      "a hardware Space plays and stops through the page, not the menu",
      played && stopped && !log.some((e) => e.id === "play-stop"),
      `played ${played}, stopped ${stopped}; menu ${JSON.stringify(log)}`,
    );

    // -- the Track menu -----------------------------------------------------------------------------------
    status("End to end: the Track menu");
    const count = () => session.view?.project?.tracks.length ?? 0;
    const before = count();
    await choose("Track", "New Audio Track");
    const audio = await until("the new audio track", () => count() === before + 1 && trackNamed(session, "Audio"));
    check(
      "Track ▸ New Audio Track adds “Audio” and selects it",
      audio.kind === "audio" && session.selection.currentTrack === audio.id,
      `${audio.name} ${audio.kind}`,
    );
    await choose("Track", "New Drum Track");
    const drums = await until("the new drum track", () => trackNamed(session, "Drums"));
    check(
      "Track ▸ New Drum Track adds “Drums” with the drum kit",
      drums.kind === "instrument" && drums.instrument?.plugin_id === "sonara.drums" && session.selection.currentTrack === drums.id,
      `${drums.instrument?.plugin_id}`,
    );
    await choose("Track", "Delete Selected Track");
    await until("the deleted drum track", () => !trackNamed(session, "Drums"));
    const [noPrompt] = await asked();
    check("Track ▸ Delete Selected Track deletes a track without clips at once", noPrompt.length === 0, JSON.stringify(noPrompt));
    session.selectTrack(keys.id);
    await answer({ choice: "No" });
    await choose("Track", "Delete Selected Track");
    await sleep(600);
    const [kept] = await asked();
    check(
      "MW-21: a track with clips is deleted only after its prompt; No keeps it",
      kept[0]?.text === "Delete “Keys 2” and its 1 clip(s)? (Undo is available.)" && !!trackNamed(session, "Keys 2"),
      kept[0]?.text,
    );
    await answer({ choice: "Yes" });
    await choose("Track", "Delete Selected Track");
    await until("the deleted Keys 2", () => !trackNamed(session, "Keys 2"));
    await asked();
    check("MW-21: Yes deletes it", true);
    await backend.undo();
    await until("Keys 2 back", () => trackNamed(session, "Keys 2"));
    session.selectTrack(audio.id);
    await choose("Track", "Delete Selected Track");
    await until("the deleted audio track", () => !trackNamed(session, "Audio"));

    // -- Transport and View --------------------------------------------------------------------------------
    status("End to end: Transport and View");
    const loop = () => session.view?.project?.transport.loop_enabled;
    const loopBefore = loop();
    await choose("Transport", "Loop");
    const looped = await within(() => loop() === !loopBefore, 3000);
    await choose("Transport", "Loop");
    const unlooped = await within(() => loop() === loopBefore, 3000);
    check("Transport ▸ Loop toggles the loop", looped && unlooped);
    const metronome = () => session.view?.project?.transport.metronome;
    const clickBefore = metronome();
    await choose("Transport", "Metronome");
    const clicked = await within(() => metronome() === !clickBefore, 3000);
    await choose("Transport", "Metronome");
    check("Transport ▸ Metronome toggles the metronome", clicked && (await within(() => metronome() === clickBefore, 3000)));
    await choose("Transport", "Play / Stop");
    const menuPlayed = await within(() => session.transport().state === "playing", 4000);
    await choose("Transport", "Play / Stop");
    const menuStopped = await within(() => session.transport().state === "stopped", 4000);
    session.setPlayhead(3);
    await choose("Transport", "Return to Start");
    const home = await within(() => session.playhead.beat === 0, 3000);
    check("Transport ▸ Play / Stop plays and stops; Return to Start goes to beat 0", menuPlayed && menuStopped && home, `${menuPlayed} ${menuStopped} ${home}`);
    await choose("View", "Piano Roll");
    const rollTab = await within(() => session.editorTab === "piano-roll", 2000);
    await choose("View", "Devices");
    const devicesTab = await within(() => session.editorTab === "devices", 2000);
    await choose("View", "Mixer");
    const mixerTab = await within(() => session.editorTab === "mixer", 2000);
    await choose("View", "Browser");
    const hidden = await within(() => !session.panes.browser.open, 2000);
    await sleep(400);
    native = await nativeMenu();
    const unchecked = submenu(native, "View", "Browser")?.checked === false;
    await choose("View", "Browser");
    const shown = await within(() => session.panes.browser.open, 2000);
    check(
      "View ▸ Piano Roll, Devices and Mixer raise their tabs; Browser hides and shows its pane, checked as shown",
      rollTab && devicesTab && mixerTab && hidden && unchecked && shown,
      `${rollTab} ${devicesTab} ${mixerTab} ${hidden} ${unchecked} ${shown}`,
    );

    // -- the dialogs, and the menu while one is open --------------------------------------------------------
    status("End to end: the dialogs from the menu");
    const dialogTitled = (title: string) =>
      until(`the ${title} dialog`, () =>
        [...document.querySelectorAll<HTMLDialogElement>("dialog[open]")].find((d) => d.querySelector("h2")?.textContent === title),
      );
    const closeDialog = async (dialog: HTMLDialogElement, label: string) => {
      [...dialog.querySelectorAll("button")].find((b) => b.textContent?.trim() === label)?.click();
      await until("the closed dialog", () => !document.querySelector("dialog[open]"));
      await sleep(200);
    };
    await choose("Sonara", "About Sonara");
    const about = await dialogTitled("About Sonara");
    await sleep(300);
    await menuLog();
    const loopModal = loop();
    await choose("Transport", "Loop");
    await postKeys({ key: "l" });
    await sleep(600);
    log = await menuLog();
    check(
      "while a dialog is open, a menu item and a single key run nothing",
      loop() === loopModal && log.some((e) => e.id === "loop" && e.declined) && !log.some((e) => !e.declined),
      `menu ${JSON.stringify(log)}`,
    );
    await closeDialog(about, "Close");
    await choose("Help", "Agent Connection…");
    await closeDialog(await dialogTitled("Agent Connection"), "Close");
    await choose("Sonara", "Settings…");
    await closeDialog(await dialogTitled("Settings"), "Cancel");
    check("Sonara ▸ About Sonara, Settings… and Help ▸ Agent Connection… open their dialogs", true);

    await choose("File", "New Project…");
    const newProject = await dialogTitled("New Project");
    await answer({ choice: "Cancel" });
    [...newProject.querySelectorAll("button")].find((b) => b.textContent?.trim() === "OK")?.click();
    await until("the New Project dialog closed", () => !document.querySelector("dialog[open]"));
    await sleep(300);
    const [unsaved] = await asked();
    check(
      "File ▸ New Project… asks about unsaved changes first; Cancel keeps the project",
      unsaved[0]?.title === "Unsaved Changes" && unsaved[0]?.text === "Save changes to “E2E”?" && session.view?.project?.name === "E2E",
      JSON.stringify(unsaved),
    );

    await choose("File", "Export Audio…");
    const exporting = await dialogTitled("Export Audio");
    const path = await until("the export path", () => exporting.querySelector<HTMLInputElement>("#export-path"));
    path.value = `${sandbox}/export/E2E Mix`;
    path.dispatchEvent(new Event("input", { bubbles: true }));
    await answer({ choice: "No" });
    [...exporting.querySelectorAll("button")].find((b) => b.textContent?.trim() === "Export")?.click();
    const [overwrite] = await pollAsked();
    check(
      "G2: exporting over an existing file asks first, naming it; No exports nothing",
      overwrite[0]?.title === "Overwrite?" && overwrite[0]?.text.includes("E2E Mix.wav") && !!exporting.open,
      overwrite[0]?.text.split("\n").slice(0, 2).join(" "),
    );
    await closeDialog(exporting, "Cancel");

    await answer({ paths: [`${sandbox}/user/Music/e2e tone.wav`] });
    const tones = () => session.view?.project?.tracks.filter((t) => t.name === "e2e tone").length ?? 0;
    const tonesBefore = tones();
    await choose("File", "Import Audio…");
    const imported = await within(() => tones() === tonesBefore + 1, 30_000);
    const [importPanel] = await asked();
    check("File ▸ Import Audio… imports the chosen files on a job", imported && importPanel[0]?.title === "Import Audio", JSON.stringify(importPanel));

    // -- the File menu's flows through their prompts and panels ------------------------------------------
    status("End to end: the File menu's flows");
    await answer({ choice: "Ok" });
    await choose("File", "Verify Project");
    const [verify] = await pollAsked();
    check(
      "File ▸ Verify Project reports in a box",
      verify[0]?.title === "Verify Project" && /^(Project is consistent\.|Problems found\.)\n\nMissing media: \d+/.test(verify[0]?.text ?? ""),
      verify[0]?.text.split("\n")[0],
    );
    await choose("File", "Embed Linked Media");
    const embedded = await message("Embedded ");
    check("File ▸ Embed Linked Media reports the files", /^Embedded \d+ linked file\(s\)$/.test(embedded), embedded);
    await answer({ choice: "Ok" }, { choice: "No" });
    await choose("File", "Remove Unused Media…");
    const [unused] = await pollAsked();
    check(
      "File ▸ Remove Unused Media… says there is nothing, or asks first",
      unused[0]?.title === "Remove Unused Media" && (unused[0]?.text === "No unused media files." || unused[0]?.text.endsWith("This cannot be undone.")),
      unused[0]?.text.split("\n")[0],
    );
    await asked();
    const midi = `${sandbox}/export/E2E.mid`;
    await answer({ path: midi });
    await choose("File", "Export MIDI…");
    const exportedMidi = await message("Exported ");
    const [midiPanel] = await asked();
    check(
      "File ▸ Export MIDI… writes the chosen file",
      exportedMidi.endsWith(`notes to ${midi}`) && midiPanel[0]?.kind === "save" && midiPanel[0]?.text === "E2E.mid",
      `${exportedMidi}; ${JSON.stringify(midiPanel)}`,
    );
    const tracksBeforeMidi = count();
    await answer({ path: midi });
    await choose("File", "Import MIDI…");
    const importedMidi = await until("the MIDI import's message", () => messages.find((m) => /^Imported \d+ MIDI track\(s\)$/.test(m)));
    check("File ▸ Import MIDI… imports it", /^Imported \d+ MIDI track\(s\)$/.test(importedMidi) && (await within(() => count() > tracksBeforeMidi, 3000)), importedMidi);
    await asked();
    const packed = `${sandbox}/export/Packed.sonara`;
    await answer({ path: packed });
    await choose("File", "Save Packaged Copy (.sonara archive)…");
    const packaged = await message("Packaged copy written to ");
    await asked();
    check("File ▸ Save Packaged Copy… writes the archive", packaged.includes("Packed.sonara"), packaged);

    await choose("File", "Save");
    const saved = await message("Saved ");
    check("File ▸ Save saves in place", saved.endsWith("E2E.sonara") && (await within(() => session.view?.project?.dirty === false, 3000)), saved);
    const copy = `${sandbox}/E2E Saved.sonara`;
    await answer({ path: copy });
    await choose("File", "Save As…");
    const savedAs = await until("the Save As message", () => messages.find((m) => m.startsWith("Saved ") && m.includes("E2E Saved")));
    await asked();
    check("File ▸ Save As… saves where the panel says", savedAs.length > 0, savedAs);
    native = await nativeMenu();
    const recentNow = (submenu(native, "File", "Open Recent")?.items ?? []).map((i) => i.title);
    check("Open Recent follows the saved projects", recentNow.includes("E2E Saved") && recentNow.includes("E2E"), recentNow.join(", "));
    const loaded = session.notice?.version ?? 0;
    await choose("File", "Open Recent", "E2E");
    await until("the reopened project", () => (session.notice?.version ?? 0) > loaded && session.notice?.kind === "loaded");
    check("File ▸ Open Recent ▸ E2E opens it", session.view?.project?.name === "E2E");
    await answer({ path: copy });
    await choose("File", "Open…");
    await until("the opened copy", () => session.notice?.kind === "loaded" && (session.view?.project?.path ?? "").includes("E2E Saved"));
    await asked();
    check("File ▸ Open… opens the chosen project", true, session.view?.project?.path ?? "");

    await backend.edit({ op: "rename_project", name: "E2E Quit" });
    await until("the dirty project", () => session.view?.project?.dirty === true);
    await answer({ choice: "Cancel" });
    await choose("Sonara", "Quit Sonara");
    const [quit] = await pollAsked();
    await sleep(500);
    check(
      "Sonara ▸ Quit Sonara asks about unsaved changes; Cancel keeps Sonara open",
      quit[0]?.text === "Save changes to “E2E Quit”?" && session.view?.project?.name === "E2E Quit",
      JSON.stringify(quit),
    );
  } finally {
    window.removeEventListener("keydown", recordKeys, { capture: true });
    stopMessages();
  }
}

/** The Edit menu's clip items once `ready` holds for them (the page's focus report is asynchronous), else after 3 s. */
async function editItemsWhen(ready: (items: { title: string; enabled: boolean }[]) => boolean) {
  const deadline = performance.now() + 3000;
  for (;;) {
    const items = edit(await nativeMenu());
    if (ready(items) || performance.now() > deadline) return items;
    await sleep(60);
  }
}

/** The prompts answered so far, waiting up to 5 s for at least one. */
async function pollAsked(): Promise<[Asked[], BenchAnswer[]]> {
  for (let i = 0; i < 50; i++) {
    const got = await asked();
    if (got[0].length > 0) return got;
    await sleep(100);
  }
  return [[], []];
}
