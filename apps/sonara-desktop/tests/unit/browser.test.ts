import { expect, test } from "@playwright/test";
import fixture from "../fixtures/dialogs-v1.json" with { type: "json" };
import {
  FIRST_OPEN,
  activation,
  activationLabel,
  clickSelection,
  draggedFiles,
  fileRows,
  searchTree,
  treeRows,
  type BrowserItem,
  type FileEntry,
} from "../../src/lib/browser/tree";
import { dialogIsOpen } from "../../src/lib/dialogs/fields";

// The Browser's logic (–BR-05) on the prototype's Devices tree
// (tests/fixtures/dialogs-v1.json, `BrowserPanel._populate`).

interface OracleItem {
  text: string;
  plugin: string | null;
  preset: string | null;
  tooltip: string;
  children: OracleItem[];
}

/** The oracle's tree as sonara-app sends it (the kind of each plugin and preset added). */
function tree(): BrowserItem[] {
  const convert = (item: OracleItem, kind: BrowserItem["kind"]): BrowserItem => ({
    text: item.text,
    plugin_id: item.plugin,
    preset: item.preset,
    kind: item.plugin ? kind : null,
    tooltip: item.tooltip,
    children: item.children.map((child) => convert(child, kind)),
  });
  const [instruments, effects] = fixture.browser as OracleItem[];
  return [convert(instruments, "instrument"), convert(effects, "effect")];
}

const items = tree();
const texts = (rows: { item: BrowserItem }[]) => rows.map((row) => row.item.text);

test("the tree opens with Instruments and Effects, their children closed", () => {
  const { shown } = searchTree(items, "");
  const rows = treeRows(items, shown, FIRST_OPEN);
  expect(texts(rows)).toEqual([
    "Instruments",
    "Sonara Drum Kit",
    "Sonara Poly Synth",
    "Effects",
    "Delay",
    "Distortion",
    "Dynamics",
    "EQ",
    "Filter",
    "Reverb",
    "Utility",
  ]);
  expect(rows.map((row) => row.depth)).toEqual([0, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1]);
});

test("BR-01: the search keeps the matches and their parents, and opens the parents", () => {
  const pads = searchTree(items, "PAD");
  const rows = treeRows(items, pads.shown, new Set([...FIRST_OPEN, ...pads.opened]));
  expect(texts(rows)).toEqual(["Instruments", "Sonara Poly Synth", "Warm Pad", "Choir Pad"]);
  // A matching parent shows, its children that do not match do not.
  const reverb = searchTree(items, "reverb");
  const shown = treeRows(items, reverb.shown, new Set([...FIRST_OPEN, ...reverb.opened]));
  expect(texts(shown)).toEqual(["Effects", "Reverb", "Convolution Reverb", "Reverb"]);
  expect(texts(treeRows(items, searchTree(items, "zzz").shown, FIRST_OPEN))).toEqual([]);
  // Spaces around the text do not count, as `text.strip()`.
  expect(searchTree(items, "  808 ").shown).toEqual(searchTree(items, "808").shown);
});

test("BR-02 and BR-03: an instrument makes a track, an effect goes on the current track", () => {
  const synth = items[0].children[1];
  const pad = synth.children.find((p) => p.text === "Warm Pad")!;
  expect(activation(synth, null)).toEqual({
    kind: "track",
    edit: { op: "create_track", name: "Sonara Poly Synth", kind: "instrument", instrument: "sonara.synth.poly", preset: null },
  });
  expect(activation(pad, "t1")).toEqual({
    kind: "track",
    edit: { op: "create_track", name: "Warm Pad", kind: "instrument", instrument: "sonara.synth.poly", preset: "Warm Pad" },
  });
  const delay = items[1].children[0].children[0];
  const dotted = delay.children[0];
  expect(activation(delay, null)).toEqual({ kind: "error", message: "select a track to add the effect to" });
  expect(activation(delay, "t1")).toEqual({
    kind: "device",
    trackId: "t1",
    edit: { op: "add_device", track_id: "t1", plugin_id: "sonara.delay" },
  });
  expect(activation(dotted, "master")).toEqual({
    kind: "device",
    trackId: "master",
    edit: { op: "add_device", track_id: "master", plugin_id: "sonara.delay", preset: "Dotted Eighth" },
  });
  expect(activation(items[1], "t1")).toBeNull();
  expect(activationLabel(pad)).toBe("Create Instrument Track");
  expect(activationLabel(dotted)).toBe("Add to Selected Track");
  expect(activationLabel(items[1].children[0])).toBeNull();
});

const entry = (path: string, folder = false): FileEntry => ({ name: path.split("/").at(-1)!, path, folder });

test("BR-04 and BR-05: the Files tab's rows, its extended selection and what a drag carries", () => {
  const listings = new Map<string, FileEntry[]>([
    ["/m", [entry("/m/a.wav"), entry("/m/Loops", true), entry("/m/b.flac")]],
    ["/m/Loops", [entry("/m/Loops/c.wav")]],
  ]);
  const closed = fileRows("/m", listings, new Set());
  expect(closed.map((r) => r.key)).toEqual(["/m/a.wav", "/m/Loops", "/m/b.flac"]);
  const rows = fileRows("/m", listings, new Set(["/m/Loops"]));
  expect(rows.map((r) => [r.key, r.depth])).toEqual([
    ["/m/a.wav", 0],
    ["/m/Loops", 0],
    ["/m/Loops/c.wav", 1],
    ["/m/b.flac", 0],
  ]);
  const none = { command: false, shift: false };
  expect(clickSelection(rows, ["/m/a.wav"], "/m/a.wav", "/m/b.flac", none)).toEqual(["/m/b.flac"]);
  expect(clickSelection(rows, ["/m/a.wav"], "/m/a.wav", "/m/b.flac", { command: true, shift: false })).toEqual([
    "/m/a.wav",
    "/m/b.flac",
  ]);
  expect(clickSelection(rows, ["/m/a.wav", "/m/b.flac"], "/m/a.wav", "/m/a.wav", { command: true, shift: false })).toEqual([
    "/m/b.flac",
  ]);
  const range = clickSelection(rows, ["/m/a.wav"], "/m/a.wav", "/m/Loops/c.wav", { command: false, shift: true });
  expect(range).toEqual(["/m/a.wav", "/m/Loops", "/m/Loops/c.wav"]);
  // A drag from a selected row carries the selected files, folders left out; from another row, that one.
  expect(draggedFiles(rows, range, "/m/Loops/c.wav")).toEqual(["/m/a.wav", "/m/Loops/c.wav"]);
  expect(draggedFiles(rows, range, "/m/b.flac")).toEqual(["/m/b.flac"]);
  expect(draggedFiles(rows, [], "/m/Loops")).toEqual([]);
});

test("the window's shortcuts wait while a modal dialog is open", () => {
  expect(dialogIsOpen({ querySelector: (selector: string) => (selector === "dialog[open]" ? ({} as Element) : null) })).toBe(true);
  expect(dialogIsOpen({ querySelector: () => null })).toBe(false);
});
