import { expect, test } from "@playwright/test";
import { detailText, geometry, hasArm, hasButtons, insertionIndex, reorderIndex } from "../../src/lib/timeline/headers";
import { TrackLayout } from "../../src/lib/timeline/layout";
import { track } from "../support/project";

// The track headers' pure parts (U): the detail line of `_detail`, the geometry of
// `geometry_for`, and the reorder of `insertion_index` and `mouseReleaseEvent`
// (tests/gui/test_timeline.py `header_buttons_rename_and_reorder`).

test("the detail line names the kind, the instrument, the volume, the pan and the effects", () => {
  expect(detailText(track("a", "Vox", "audio"))).toBe("AUDIO  ·  +0.0 dB");
  const synth = {
    ...track("k", "Keys", "instrument"),
    mixer: { volume_db: -6.04, pan: -0.3, mute: false, solo: false, solo_safe: false },
    instrument: {
      id: "dev",
      plugin_id: "sonara.synth.poly",
      name: "Sonara Poly Synth",
      enabled: true,
      preset: "Analog Bass",
      sidechain_id: null,
      available: true,
      params: {},
    },
  };
  expect(detailText(synth)).toBe("MIDI  ·  Analog Bass  ·  -6.0 dB  ·  L30");
  expect(detailText({ ...synth, instrument: { ...synth.instrument, preset: null } })).toBe(
    "MIDI  ·  Sonara Poly Synth  ·  -6.0 dB  ·  L30",
  );
  const effect = synth.instrument;
  const bus = {
    ...track("b", "Verb", "bus"),
    mixer: { volume_db: -144, pan: 0.0005, mute: false, solo: false, solo_safe: false },
    inserts: [effect, effect],
  };
  expect(detailText(bus)).toBe("BUS  ·  -inf dB  ·  2 fx");
  expect(detailText({ ...bus, mixer: { ...bus.mixer, volume_db: 3, pan: 1 } })).toBe("BUS  ·  +3.0 dB  ·  R100  ·  2 fx");
  expect(detailText(track("f", "Group", "folder"))).toBe("FOLDER");
  // Python's `.0f` rounds a tie to even.
  expect(detailText({ ...bus, inserts: [], mixer: { ...bus.mixer, volume_db: 0, pan: 0.125 } })).toBe(
    "BUS  ·  +0.0 dB  ·  R12",
  );
});

test("the buttons: M and S except on folders, R on audio and instrument tracks", () => {
  expect([hasButtons(track("a", "A", "audio")), hasArm(track("a", "A", "audio"))]).toEqual([true, true]);
  expect([hasButtons(track("i", "I", "instrument")), hasArm(track("i", "I", "instrument"))]).toEqual([true, true]);
  expect([hasButtons(track("b", "B", "bus")), hasArm(track("b", "B", "bus"))]).toEqual([true, false]);
  expect([hasButtons(track("f", "F", "folder")), hasArm(track("f", "F", "folder"))]).toEqual([false, false]);
});

test("compact rows centre the name and the buttons; tall rows show the detail", () => {
  const tracks = [track("f", "Group", "folder"), { ...track("c", "Child", "audio"), parent_id: "f", depth: 1 }];
  const small = new TrackLayout(tracks, 34);
  // The folder is 14 px shorter, never below 24 px.
  expect(geometry(small.rows[0])).toEqual({ left: 8, nameTop: 3, buttonTop: 3, detail: false });
  expect(geometry(small.rows[1])).toEqual({ left: 20, nameTop: 8, buttonTop: 8, detail: false });
  const tall = new TrackLayout(tracks, 56);
  // A folder at 56 px is 42 px high, still compact.
  expect(geometry(tall.rows[0])).toEqual({ left: 8, nameTop: 12, buttonTop: 12, detail: false });
  expect(geometry(tall.rows[1])).toEqual({ left: 20, nameTop: 6, buttonTop: 7, detail: true });
});

test("a dragged header goes before the row whose middle it passed", () => {
  const layout = new TrackLayout([track("a", "A"), track("b", "B"), track("c", "C")], 56);
  expect(insertionIndex(layout.rows, 4, 0)).toBe(0);
  expect(insertionIndex(layout.rows, 27, 0)).toBe(0);
  expect(insertionIndex(layout.rows, 28, 0)).toBe(1);
  expect(insertionIndex(layout.rows, 140, 0)).toBe(3);
  expect(insertionIndex(layout.rows, 4, 56)).toBe(1);
  // C (index 2) before A: index 0; A after the last row: index 2; a drop onto itself: none.
  expect(reorderIndex(2, 0)).toBe(0);
  expect(reorderIndex(0, 3)).toBe(2);
  expect(reorderIndex(1, 1)).toBeNull();
  expect(reorderIndex(1, 2)).toBeNull();
});
