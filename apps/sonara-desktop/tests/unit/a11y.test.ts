import { expect, test } from "@playwright/test";
import { cycleTarget, wrapTarget } from "../../src/lib/a11y/focus";
import {
  beatsText,
  clipOptionLabel,
  meterText,
  noteLengthText,
  noteOptionLabel,
  positionText,
  trackOptionLabel,
} from "../../src/lib/a11y/labels";
import { backingSize, watchDevicePixelRatio, type PixelRatioSource } from "../../src/lib/hidpi";
import { ARIA_INTERVAL_S, FLOOR_DB, MeterBank } from "../../src/lib/mixer/meter";
import { MAX_NOTE_OPTIONS, noteOptions, noteSpokenName, revealNote } from "../../src/lib/pianoroll/noteList";
import { NoteIndex, type RollView } from "../../src/lib/pianoroll/model";
import { fieldUndo, installFieldUndo } from "../../src/lib/shell/shortcuts";
import { DEFAULT_TIMELINE } from "../../src/lib/state/timeline";
import { adjacentClip, clipGroups, revealClip } from "../../src/lib/timeline/clipList";
import { TrackLayout } from "../../src/lib/timeline/layout";
import { TimelineModel } from "../../src/lib/timeline/model";
import type { LiveFrame } from "../../src/lib/live/frame";
import type { NoteView, TimeSignatureView } from "../../src/lib/types";
import { audioClip, midiClip, project, track } from "../support/project";

// The ARIA layer of (U): the texts assistive technology reads, the clip and
// note lists over the canvases, the meters' value rate, the dialogs' focus wrap, the text fields'
// own ⌘Z and the display-scale watcher. The prototype had no accessible representation of these,
// so the oracles are ADR 0009's check ("Drums 1, 0.0 dB"), the inventory's
// examples ("C4, bar 2 beat 1, 1/8, velocity 100") and the prototype's line edits, probed on
// 2026-10-23 (⌘Z took back the typing, the project's "Undo Rename Track" stayed).

const FOUR_FOUR: TimeSignatureView[] = [{ bar: 0, beat: 0, numerator: 4, denominator: 4, beats_per_bar: 4 }];
// 4/4 for two bars, then 6/8 (bars of 3 beats, beats of half a beat).
const CHANGING: TimeSignatureView[] = [...FOUR_FOUR, { bar: 2, beat: 8, numerator: 6, denominator: 8, beats_per_bar: 3 }];

test("positions read as bar and beat, with the ticks off the beat, in the signature in force", () => {
  expect(positionText(FOUR_FOUR, 0)).toBe("bar 1 beat 1");
  expect(positionText(FOUR_FOUR, 4)).toBe("bar 2 beat 1");
  expect(positionText(FOUR_FOUR, 5.5)).toBe("bar 2 beat 2 tick 480");
  expect(positionText(CHANGING, 8)).toBe("bar 3 beat 1");
  // 6/8 counts eighths: half a beat each.
  expect(positionText(CHANGING, 9)).toBe("bar 3 beat 3");
  expect(positionText(null, 2)).toBe("bar 1 beat 3");
  expect(positionText(FOUR_FOUR, -1)).toBe("bar 1 beat 1");
});

test("lengths read in beats, and notes as note values", () => {
  expect(beatsText(1)).toBe("1 beat");
  expect(beatsText(4)).toBe("4 beats");
  expect(beatsText(0.5)).toBe("0.5 beats");
  expect(noteLengthText(0.5)).toBe("1/8");
  expect(noteLengthText(0.25)).toBe("1/16");
  expect(noteLengthText(1)).toBe("1/4");
  expect(noteLengthText(2)).toBe("1/2");
  expect(noteLengthText(4)).toBe("1/1");
  expect(noteLengthText(8)).toBe("2/1");
  expect(noteLengthText(0.75)).toBe("3/16");
  expect(noteLengthText(1.5)).toBe("3/8");
  // A triplet or a free length is no note value.
  expect(noteLengthText(1 / 3)).toBe("0.333333 beats");
  expect(noteLengthText(0)).toBe("0 beats");
});

test("ADR 0009: a track reads as its name and volume, then its lit buttons", () => {
  expect(trackOptionLabel(track("d", "Drums 1"))).toBe("Drums 1, 0.0 dB");
  const loud = { ...track("v", "Vox", "audio"), mixer: { volume_db: -6.04, pan: 0, mute: true, solo: true, solo_safe: false } };
  expect(trackOptionLabel({ ...loud, input: { ...loud.input, armed: true } })).toBe("Vox, -6.0 dB, muted, solo, armed");
  expect(trackOptionLabel({ ...loud, mixer: { ...loud.mixer, volume_db: -144, mute: false, solo: false } })).toBe("Vox, -inf dB");
  // A folder has no volume of its own in the headers.
  expect(trackOptionLabel(track("f", "Group", "folder"))).toBe("Group");
});

test("a clip reads as its name, start and length; a note as its name, start, length and velocity", () => {
  expect(clipOptionLabel({ ...midiClip("c", 4, 4), name: "Riff" }, FOUR_FOUR)).toBe("Riff, bar 2 beat 1, 4 beats");
  expect(clipOptionLabel({ ...audioClip("a", "asset", 9.5, 1), muted: true }, CHANGING)).toBe("Take, bar 3 beat 4, 1 beat, muted");
  expect(clipOptionLabel({ ...midiClip("c", 0, 1), name: "" }, FOUR_FOUR)).toBe("Clip, bar 1 beat 1, 1 beat");
  const note: NoteView = { id: "n", pitch: 60, start: 0, length: 0.5, velocity: 100, channel: 0 };
  expect(noteOptionLabel(note, "C4", 4, FOUR_FOUR)).toBe("C4, bar 2 beat 1, 1/8, velocity 100");
  expect(noteSpokenName(61, { drums: false, drumLabels: new Map() })).toBe("C#4");
  expect(noteSpokenName(38, { drums: true, drumLabels: new Map([[38, "Acoustic Snare"]]) })).toBe("Acoustic Snare");
  // A drum track's note without a GM name keeps its pitch name.
  expect(noteSpokenName(20, { drums: true, drumLabels: new Map([[38, "Acoustic Snare"]]) })).toBe("G#0");
});

test("the note list reads the notes the grid shows, at the timeline's bars, capped, and keeps the focused one", () => {
  const clip = { ...midiClip("c", 4, 8), content_offset: 0 };
  const index = new NoteIndex(clip.notes);
  // 80 px a beat, 14 px rows, pitch 127 at the top: scrolled so that C4 (60) shows.
  const view: RollView = { ppb: 80, rowH: 14, x0: 0, y0: (127 - 66) * 14 };
  const naming = { drums: false, drumLabels: new Map<number, string>() };
  const all = noteOptions(index, clip, FOUR_FOUR, view, { width: 800, height: 200 }, new Set(["c_n1"]), naming, null);
  expect(all.map((o) => o.label)).toEqual([
    "C4, bar 2 beat 1, 1/8, velocity 100",
    "C#4, bar 2 beat 1 tick 480, 1/8, velocity 100",
    "D4, bar 2 beat 2, 1/8, velocity 100",
    "D#4, bar 2 beat 2 tick 480, 1/8, velocity 100",
  ]);
  expect(all.map((o) => [o.position, o.count, o.selected])).toEqual([
    [1, 4, false],
    [2, 4, true],
    [3, 4, false],
    [4, 4, false],
  ]);
  expect(all[1].rect).toEqual({ x: 40, y: (127 - 61) * 14 - view.y0, width: 40, height: 14 });
  // Only what shows: a grid one beat wide shows the first two notes; the focused note stays.
  const narrow = noteOptions(index, clip, FOUR_FOUR, view, { width: 80, height: 200 }, new Set(), naming, "c_n3");
  expect(narrow.map((o) => o.id)).toEqual(["c_n0", "c_n1", "c_n3"]);
  // A zoomed-out clip of many notes gives at most MAX_NOTE_OPTIONS options.
  const many = Array.from({ length: 1000 }, (_, i) => ({ id: `m${i}`, pitch: 60, start: i / 8, length: 0.1, velocity: 90, channel: 0 }));
  const wide = noteOptions(new NoteIndex(many), { ...clip, notes: many }, FOUR_FOUR, { ...view, ppb: 6 }, { width: 2000, height: 200 }, new Set(), naming, "m999");
  expect(wide).toHaveLength(MAX_NOTE_OPTIONS + 1);
  expect(wide.at(-1)?.id).toBe("m999");
});

test("a focused note scrolls into the grid only as far as it must", () => {
  const view: RollView = { ppb: 80, rowH: 14, x0: 0, y0: (127 - 66) * 14 };
  const viewport = { width: 800, height: 200 };
  // C4 at beat 1 shows whole: the view stays.
  expect(revealNote(view, { start: 1, length: 0.5, pitch: 60 }, viewport)).toBe(view);
  // Beat 12 is right of the grid: it scrolls until the note's end shows, with a margin.
  const right = revealNote(view, { start: 12, length: 1, pitch: 60 }, viewport);
  expect(right.x0).toBeCloseTo(13 - (800 - 8) / 80);
  expect(right.y0).toBe(view.y0);
  // A note left of the grid scrolls to its start; a pitch above it scrolls up to its row.
  const left = revealNote({ ...view, x0: 8 }, { start: 2, length: 1, pitch: 60 }, viewport);
  expect(left.x0).toBeCloseTo(2 - 8 / 80);
  const above = revealNote(view, { start: 1, length: 1, pitch: 100 }, viewport);
  expect(above.y0).toBe((127 - 100) * 14);
  const below = revealNote(view, { start: 1, length: 1, pitch: 30 }, viewport);
  expect(below.y0).toBe((127 - 30) * 14 + 14 - 200);
});

test("the clip list groups the clips the canvas shows by track; ↑ and ↓ walk every clip in order", () => {
  const open = project([
    track("a", "Keys", "instrument", [midiClip("a1", 0, 4), midiClip("a2", 8, 4), midiClip("a3", 200, 4)]),
    track("b", "Vox", "audio", [audioClip("b1", "x", 2, 2)]),
  ]);
  const model = new TimelineModel(open);
  const layout = new TrackLayout(open.tracks, 56);
  const view = DEFAULT_TIMELINE; // 24 px a beat
  const groups = clipGroups(model, layout, view, 600, 300, new Set(["a2"]), null);
  expect(groups.map((g) => [g.name, g.options.map((o) => o.label)])).toEqual([
    ["Keys", ["Clip, bar 1 beat 1, 4 beats", "Clip, bar 3 beat 1, 4 beats"]],
    ["Vox", ["Take, bar 1 beat 3, 2 beats"]],
  ]);
  expect(groups[0].options.map((o) => [o.position, o.count, o.selected])).toEqual([
    [1, 3, false],
    [2, 3, true],
  ]);
  // The rect is the canvas's clip_rect.
  expect(groups[0].options[1].rect).toEqual({ x: 192.5, y: 1.5, width: 95, height: 53 });
  // The focused clip stays an option off screen.
  const kept = clipGroups(model, layout, view, 600, 300, new Set(), "a3");
  expect(kept[0].options.map((o) => o.id)).toEqual(["a1", "a2", "a3"]);
  // The tracks' order, then the start; the ends stay.
  expect(adjacentClip(model, null, 1)?.clip.id).toBe("a1");
  expect(adjacentClip(model, null, -1)?.clip.id).toBe("b1");
  expect(adjacentClip(model, "a3", 1)?.clip.id).toBe("b1");
  expect(adjacentClip(model, "b1", 1)?.clip.id).toBe("b1");
  expect(adjacentClip(model, "a1", -1)?.clip.id).toBe("a1");
  expect(adjacentClip(model, "gone", 1)?.clip.id).toBe("a1");
  expect(adjacentClip(new TimelineModel(project([track("e", "Empty")])), null, 1)).toBeNull();
  // Revealing a clip scrolls only as far as needed.
  const row = layout.row("a");
  if (!row) throw new Error("no row");
  expect(revealClip(view, row, model.clip("a1")!.clip, 600, 300)).toEqual({ scrollX: 0, scrollY: 0 });
  expect(revealClip(view, row, model.clip("a3")!.clip, 600, 300)).toEqual({ scrollX: 200 * 24 + 4 * 24 - 600 + 8, scrollY: 0 });
  const below = layout.row("b")!;
  expect(revealClip({ ...view, scrollX: 4000 }, below, model.clip("b1")!.clip, 600, 80)).toEqual({ scrollX: 2 * 24 - 8, scrollY: 112 - 80 });
});

test("a meter's value for assistive technology changes at most four times a second", () => {
  const open = project([track("a", "A")], { meter_layout: 3 });
  const writes: [string, string][] = [];
  const box = { setAttribute: (name: string, value: string) => writes.push([name, value]) } as unknown as Element;
  const canvas = {} as unknown as HTMLCanvasElement;
  const bank = new MeterBank();
  bank.register(canvas, () => "a", 0, box);
  expect(writes).toEqual([
    ["aria-valuenow", String(FLOOR_DB)],
    ["aria-valuetext", "silent"],
  ]);
  writes.length = 0;
  const frame = (peak: number) => ({ meter_layout: 3, meters: [{ peak: [peak, peak / 2], rms: [peak, peak / 2] }] }) as unknown as LiveFrame;
  // A second of 60 Hz frames with a level that changes at every frame.
  for (let i = 1; i <= 60; i++) bank.update(i / 60, frame(0.1 + i / 200), open);
  const values = writes.filter(([name]) => name === "aria-valuetext");
  expect(values.length).toBeLessThanOrEqual(Math.ceil(1 / ARIA_INTERVAL_S));
  expect(values.length).toBeGreaterThanOrEqual(2);
  expect(values.at(-1)?.[1]).toMatch(/^-?\d+ dB$/);
  // An unchanged text is not written again; a clip says so.
  writes.length = 0;
  bank.update(1.3, frame(0.4), open);
  bank.update(1.6, frame(1.5), open);
  expect(writes.filter(([name]) => name === "aria-valuetext").at(-1)?.[1]).toBe("4 dB, clipped");
  expect(meterText(-75, FLOOR_DB, false)).toBe("silent");
  expect(meterText(-0.3, FLOOR_DB, false)).toBe("0 dB");
});

test("Tab wraps inside a dialog, both ways", () => {
  const items = ["name", "template", "cancel", "create"];
  expect(wrapTarget(items, "create", false)).toBe("name");
  expect(wrapTarget(items, "name", true)).toBe("create");
  expect(wrapTarget(items, "template", false)).toBeNull();
  expect(wrapTarget(items, "cancel", true)).toBeNull();
  // From outside (the dialog itself): the first, or the last going back.
  expect(wrapTarget(items, null, false)).toBe("name");
  expect(wrapTarget(items, null, true)).toBe("create");
  expect(wrapTarget([], null, false)).toBeNull();
});

test("a text field takes ⌘Z and ⇧⌘Z for its own typing; elsewhere they are the project's", () => {
  const field = { tagName: "INPUT", type: "text" };
  const number = { tagName: "INPUT", type: "number" };
  const area = { tagName: "TEXTAREA" };
  const button = { tagName: "BUTTON" };
  const checkbox = { tagName: "INPUT", type: "checkbox" };
  const press = (target: object, key = "z", extra: Partial<Record<"metaKey" | "ctrlKey" | "altKey" | "shiftKey", boolean>> = {}) =>
    fieldUndo({ key, metaKey: true, ctrlKey: false, altKey: false, shiftKey: false, ...extra, target: target as EventTarget });
  expect(press(field)).toBe("undo");
  expect(press(number)).toBe("undo");
  expect(press(area, "Z", { shiftKey: true })).toBe("redo");
  expect(press(button)).toBeNull();
  expect(press(checkbox)).toBeNull();
  expect(press(field, "z", { metaKey: false })).toBeNull();
  expect(press(field, "z", { ctrlKey: true })).toBeNull();
  expect(press(field, "z", { altKey: true })).toBeNull();
  expect(press(field, "y")).toBeNull();
  // Installed: taken in the capture phase, before the field's own handlers and the menu.
  const listeners: { type: string; listener: (event: KeyboardEvent) => void; capture: unknown }[] = [];
  const fakeWindow = {
    addEventListener: (type: string, listener: (event: KeyboardEvent) => void, capture: unknown) => listeners.push({ type, listener, capture }),
    removeEventListener: (type: string, listener: unknown) => {
      const at = listeners.findIndex((l) => l.type === type && l.listener === listener);
      if (at >= 0) listeners.splice(at, 1);
    },
  } as unknown as Window;
  const ran: string[] = [];
  const stop = installFieldUndo(fakeWindow, (command) => ran.push(command));
  expect(listeners.map((l) => [l.type, l.capture])).toEqual([["keydown", true]]);
  let prevented = 0;
  const event = (target: object, shiftKey = false) =>
    ({
      key: "z",
      metaKey: true,
      ctrlKey: false,
      altKey: false,
      shiftKey,
      target,
      preventDefault: () => (prevented += 1),
      stopPropagation: () => {},
    }) as unknown as KeyboardEvent;
  listeners[0].listener(event(field));
  listeners[0].listener(event(field, true));
  listeners[0].listener(event(button));
  expect(ran).toEqual(["undo", "redo"]);
  expect(prevented).toBe(2);
  stop();
  expect(listeners).toHaveLength(0);
});

test("the scale watcher follows a move to a display with another scale, and stops", () => {
  const queries: { query: string; listeners: (() => void)[] }[] = [];
  const source: PixelRatioSource & { devicePixelRatio: number } = {
    devicePixelRatio: 2,
    matchMedia(query: string) {
      const entry = { query, listeners: [] as (() => void)[] };
      queries.push(entry);
      return {
        addEventListener: (_type: string, listener: () => void) => entry.listeners.push(listener),
        removeEventListener: (_type: string, listener: () => void) => {
          entry.listeners = entry.listeners.filter((l) => l !== listener);
        },
      } as unknown as MediaQueryList;
    },
  };
  const seen: number[] = [];
  const stop = watchDevicePixelRatio((dpr) => seen.push(dpr), source);
  expect(queries.map((q) => q.query)).toEqual(["(resolution: 2dppx)"]);
  // The window moves to a 1x display: the 2x query stops matching.
  source.devicePixelRatio = 1;
  for (const listener of queries[0].listeners) listener();
  expect(seen).toEqual([1]);
  expect(queries.map((q) => q.query)).toEqual(["(resolution: 2dppx)", "(resolution: 1dppx)"]);
  expect(queries[0].listeners).toHaveLength(0);
  // And back.
  source.devicePixelRatio = 2;
  for (const listener of queries[1].listeners) listener();
  expect(seen).toEqual([1, 2]);
  stop();
  expect(queries.every((q) => q.listeners.length === 0)).toBe(true);
  expect(backingSize(300.4, 0, 2)).toEqual({ width: 601, height: 1 });
  expect(backingSize(16, 120, 1)).toEqual({ width: 16, height: 120 });
});

test("Tab never leaves the move to the browser: every step names the next element", () => {
  const items = ["name", "template", "cancel", "create"];
  expect(cycleTarget(items, "name", false)).toBe("template");
  expect(cycleTarget(items, "template", false)).toBe("cancel");
  expect(cycleTarget(items, "template", true)).toBe("name");
  expect(cycleTarget(items, "create", false)).toBe("name");
  expect(cycleTarget(items, "name", true)).toBe("create");
  expect(cycleTarget(items, null, false)).toBe("name");
  expect(cycleTarget(items, null, true)).toBe("create");
  expect(cycleTarget([], null, false)).toBeNull();
  expect(cycleTarget(["only"], "only", false)).toBe("only");
});
