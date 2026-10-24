import { expect, test } from "@playwright/test";
import fixture from "../fixtures/menus-v1.json" with { type: "json" };
import {
  EditTargets,
  MenuContextReporter,
  dialogOpened,
  editorOf,
  isModal,
  onModalChange,
  type EditCommand,
  type EditFocus,
  type MenuContext,
} from "../../src/lib/shell/editing";
import { runMenuCommand, type MenuCommand, type MenuCommandHandlers } from "../../src/lib/shell/menuCommands";
import type { TransportShortcut } from "../../src/lib/shell/shortcuts";
import type { Edit } from "../../src/lib/types";

// The page's side of the menus: its command table and focus names against the
// model of sonara-app (tests/fixtures/menus-v1.json, written by crates/sonara-app/tests/gui_menus.rs),
// the Edit menu's routing to the focused editor, the modal guard and the report
// of the menu's context.

/** Records what the handlers were asked to do. */
function recorder(modal = false) {
  const done: string[] = [];
  const handlers: MenuCommandHandlers = {
    modal: () => modal,
    edit: (command) => done.push(`edit ${command}`),
    newTrack: (edit) => done.push(`new ${JSON.stringify(edit)}`),
    deleteTrack: () => done.push("delete track"),
    transport: (action) => done.push(`transport ${action}`),
  };
  return { done, handlers };
}

const TRANSPORT: Record<string, TransportShortcut> = {
  "play-stop": "play",
  "return-to-start": "home",
  record: "record",
  loop: "loop",
  metronome: "metronome",
};

test("every page command of sonara-app's menu runs its own handler", () => {
  const commands = fixture.page_commands as unknown as MenuCommand[];
  expect(commands.map((c) => c.command)).toEqual([
    "split-clips",
    "duplicate-clips",
    "delete-clips",
    "new-instrument-track",
    "new-drum-track",
    "new-audio-track",
    "new-bus",
    "new-folder",
    "delete-track",
    "play-stop",
    "return-to-start",
    "record",
    "loop",
    "metronome",
  ]);
  for (const command of commands) {
    const { done, handlers } = recorder();
    expect(runMenuCommand(command, handlers)).toBe(true);
    let expected: string;
    if ("edit" in command) expected = `new ${JSON.stringify(command.edit)}`;
    else if (command.command === "delete-track") expected = "delete track";
    else if (command.command in TRANSPORT) expected = `transport ${TRANSPORT[command.command]}`;
    else expected = `edit ${command.command}`;
    expect(done, command.command).toEqual([expected]);
  }
  // MW-20's names and instruments travel in the command; the page sends the edit as it is.
  const created = commands.filter((c): c is Extract<MenuCommand, { edit: Edit }> => "edit" in c).map((c) => c.edit);
  expect(created).toEqual([
    { op: "create_track", name: "Instrument", kind: "instrument", instrument: "sonara.synth.poly" },
    { op: "create_track", name: "Drums", kind: "instrument", instrument: "sonara.drums" },
    { op: "create_track", name: "Audio", kind: "audio", instrument: null },
    { op: "create_track", name: "Bus", kind: "bus", instrument: null },
    { op: "create_track", name: "Folder", kind: "folder", instrument: null },
  ]);
});

test("while a dialog is open no menu command runs; an unknown one is refused", () => {
  for (const command of fixture.page_commands as unknown as MenuCommand[]) {
    const { done, handlers } = recorder(true);
    expect(runMenuCommand(command, handlers)).toBe(true);
    expect(done).toEqual([]);
  }
  const { done, handlers } = recorder();
  expect(runMenuCommand({ command: "quit" } as unknown as MenuCommand, handlers)).toBe(false);
  expect(done).toEqual([]);
});

/** An element in the region `name` (`data-editor`), or outside every region. */
function element(name: string | null): Element {
  const region = { getAttribute: (attribute: string) => (attribute === "data-editor" ? name : null) };
  return { closest: (selector: string) => (selector === "[data-editor]" && name !== null ? region : null) } as unknown as Element;
}

test("the focused editor is the region of the focused element, named as the desktop reads it", () => {
  const names = (fixture.edit_items as { focus: string }[]).map((entry) => entry.focus);
  expect(names).toEqual(["none", "timeline", "piano-roll", "devices"]);
  for (const name of ["timeline", "piano-roll", "devices"]) expect(editorOf(element(name))).toBe(name);
  expect(editorOf(element(null))).toBe("none");
  expect(editorOf(element("mixer"))).toBe("none");
  expect(editorOf(null)).toBe("none");
});

test("G9: the Edit menu's items run the focused editor's own operation", () => {
  const done: string[] = [];
  const targets = new EditTargets();
  const stopTimeline = targets.register("timeline", {
    split: () => done.push("timeline split"),
    duplicate: () => done.push("timeline duplicate"),
    remove: () => done.push("timeline delete"),
  });
  const run = (command: EditCommand, focus: EditFocus) => targets.run(command, focus);
  // The piano roll is not shown: its focus finds no editor.
  expect(run("duplicate-clips", "piano-roll")).toBe(false);
  targets.register("piano-roll", { duplicate: () => done.push("notes duplicate"), remove: () => done.push("notes delete") });
  expect(run("duplicate-clips", "timeline")).toBe(true);
  expect(run("split-clips", "none")).toBe(true);
  expect(run("delete-clips", "none")).toBe(true);
  expect(run("duplicate-clips", "piano-roll")).toBe(true);
  expect(run("delete-clips", "piano-roll")).toBe(true);
  // The piano roll splits nothing; a device card has none of them.
  expect(run("split-clips", "piano-roll")).toBe(false);
  for (const command of ["split-clips", "duplicate-clips", "delete-clips"] as EditCommand[]) {
    expect(run(command, "devices")).toBe(false);
  }
  expect(done).toEqual(["timeline duplicate", "timeline split", "timeline delete", "notes duplicate", "notes delete"]);
  // The same as sonara-app's enabled states (the fixture's), item by item.
  for (const entry of fixture.edit_items as { focus: EditFocus; items: { action: EditCommand; enabled: boolean }[] }[]) {
    for (const item of entry.items) {
      const probe = new EditTargets();
      probe.register("timeline", { split: () => {}, duplicate: () => {}, remove: () => {} });
      probe.register("piano-roll", { duplicate: () => {}, remove: () => {} });
      expect(probe.run(item.action, entry.focus), `${entry.focus} ${item.action}`).toBe(item.enabled);
    }
  }
  stopTimeline();
  expect(run("duplicate-clips", "timeline")).toBe(false);
});

test("the menu's context is sent once per change, and again after a failure", async () => {
  const sent: MenuContext[] = [];
  let fail = false;
  const errors: unknown[] = [];
  const reporter = new MenuContextReporter(
    async (context) => {
      sent.push(context);
      if (fail) throw new Error("refused");
    },
    (error) => errors.push(error),
  );
  reporter.update({ focus: "none", modal: false });
  reporter.update({ focus: "none", modal: false });
  reporter.update({ focus: "timeline", modal: false });
  reporter.update({ focus: "timeline", modal: true });
  expect(sent).toEqual([
    { focus: "none", modal: false },
    { focus: "timeline", modal: false },
    { focus: "timeline", modal: true },
  ]);
  fail = true;
  reporter.update({ focus: "devices", modal: false });
  await Promise.resolve();
  await Promise.resolve();
  fail = false;
  reporter.update({ focus: "devices", modal: false });
  expect(sent.slice(3)).toEqual([
    { focus: "devices", modal: false },
    { focus: "devices", modal: false },
  ]);
  expect(errors).toHaveLength(1);
});

test("a dialog is modal from its opening to its closing, which counts once", () => {
  let changes = 0;
  const stop = onModalChange(() => (changes += 1));
  expect(isModal()).toBe(false);
  const close = dialogOpened();
  const other = dialogOpened();
  expect(isModal()).toBe(true);
  close();
  close();
  expect(isModal()).toBe(true);
  other();
  expect(isModal()).toBe(false);
  expect(changes).toBe(4);
  stop();
});
