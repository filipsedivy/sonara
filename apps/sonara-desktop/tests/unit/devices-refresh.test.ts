import { expect, test } from "@playwright/test";
import { ViewRefresher, changedClips, changedDevices, changedTracks, patchDevices, type ViewPatch } from "../../src/lib/state/project";
import { sameRows } from "../../src/lib/timeline/layout";
import { sameTimeline } from "../../src/lib/timeline/model";
import type { DeviceView, DevicesView, ProjectView, SessionNotice } from "../../src/lib/types";
import { defaults } from "../support/plugins";
import { midiClip, project, track } from "../support/project";

// The refresh of a device edit (PERF-01): a knob drag asks for the devices it
// changed instead of the whole view, although the prototype's panels count a device edit as
// structural, and the timeline keeps its model through it.

function notice(kind: SessionNotice["kind"], flags: string[], updated: [string, string][], extra = {}): SessionNotice {
  return {
    version: 2,
    kind,
    label: "Set Parameter",
    source: "gui",
    changes: {
      created: [],
      updated: updated.map(([k, id]) => ({ kind: k, id })),
      deleted: [],
      flags,
      structure_changed: flags.includes("devices"),
      ...extra,
    },
  };
}

test("only an edit of devices' state alone refreshes just those devices", () => {
  // A parameter, On/Off, a preset (SetDeviceParams, SetDeviceEnabled, ApplyPreset).
  expect(changedDevices(notice("command", ["devices"], [["device", "dev_1"]]))).toEqual(["dev_1"]);
  expect(changedDevices(notice("command", ["devices"], [["device", "dev_1"], ["device", "dev_2"]]))).toEqual(["dev_1", "dev_2"]);
  // Moving a device updates its track; adding or removing one creates or deletes it; a sidechain
  // also changes the routing; undo and redo may do anything.
  expect(changedDevices(notice("command", ["devices"], [["device", "dev_1"], ["track", "a"]]))).toBeNull();
  expect(changedDevices(notice("command", ["devices"], [["track", "a"]], { created: [{ kind: "device", id: "d" }] }))).toBeNull();
  expect(changedDevices(notice("command", ["devices"], [["track", "a"]], { deleted: [{ kind: "device", id: "d" }] }))).toBeNull();
  expect(changedDevices(notice("command", ["devices", "routing"], [["device", "dev_1"]]))).toBeNull();
  expect(changedDevices(notice("undo", ["devices"], [["device", "dev_1"]]))).toBeNull();
  expect(changedDevices(notice("command", ["devices"], []))).toBeNull();
  expect(changedDevices(notice("command", ["mixer"], [["device", "dev_1"]]))).toBeNull();
  // The other refreshes leave such an event alone.
  const knob = notice("command", ["devices"], [["device", "dev_1"]]);
  expect(changedTracks(knob)).toBeNull();
  expect(changedClips(knob)).toBeNull();
});

function device(id: string, extra: Partial<DeviceView> = {}): DeviceView {
  return {
    id,
    plugin_id: "sonara.dynamics.compressor",
    name: "Compressor",
    enabled: true,
    preset: null,
    sidechain_id: null,
    available: true,
    params: defaults("sonara.dynamics.compressor"),
    ...extra,
  };
}

function chainView(version: number): ProjectView {
  const keys = { ...track("k", "Keys", "instrument", [midiClip("c", 0, 4)]), instrument: device("dev_synth"), inserts: [device("dev_1")] };
  const open = project([keys, track("a", "Vox", "audio")], { meter_layout: 9 });
  return { version, project: { ...open, master: { ...open.master, inserts: [device("dev_m")] } } };
}

function devicesView(version: number, devices: { track_id: string; device: DeviceView }[], extra = {}): DevicesView {
  return {
    version,
    project: { id: "prj_1", dirty: true, history: { undo: "Set Parameter", redo: null }, meter_layout: 9, devices, ...extra },
  };
}

test("a device refresh lands in its track and keeps every other object", () => {
  const before = chainView(1);
  const open = before.project;
  if (!open) throw new Error("no project");
  const edited = device("dev_1", { params: { ...defaults("sonara.dynamics.compressor"), threshold_db: -30 } });
  const synth = device("dev_synth", { enabled: false });
  const master = device("dev_m", { preset: "Gentle Bus Glue" });
  const after = patchDevices(
    before,
    devicesView(3, [
      { track_id: "k", device: synth },
      { track_id: "k", device: edited },
      { track_id: "master", device: master },
    ]),
  );
  const next = after?.project;
  if (!after || !next) throw new Error("not patched");
  expect(after.version).toBe(3);
  expect(next.tracks[0].instrument).toBe(synth);
  expect(next.tracks[0].inserts).toEqual([edited]);
  expect(next.master.inserts).toEqual([master]);
  // The patched track keeps its clips and mixer; the others are the same objects.
  expect(next.tracks[0].clips).toBe(open.tracks[0].clips);
  expect(next.tracks[0].mixer).toBe(open.tracks[0].mixer);
  expect(next.tracks[1]).toBe(open.tracks[1]);
  expect(next.plugins).toBe(open.plugins);
  expect([next.dirty, next.history.undo]).toEqual([true, "Set Parameter"]);
  expect(sameTimeline(open, next)).toBe(true);
  expect(sameRows(open.tracks, next.tracks)).toBe(true);
  // A device that is not where the view has it, another layout or project: the whole view.
  expect(patchDevices(before, devicesView(3, [{ track_id: "a", device: edited }]))).toBeNull();
  expect(patchDevices(before, devicesView(3, [{ track_id: "k", device: device("dev_new") }]))).toBeNull();
  expect(patchDevices(before, devicesView(3, [{ track_id: "k", device: edited }], { meter_layout: 10 }))).toBeNull();
  expect(patchDevices(before, devicesView(3, [{ track_id: "k", device: edited }], { id: "prj_2" }))).toBeNull();
  expect(patchDevices(before, { version: 3, project: null })).toBeNull();
});

test("a knob drag's edits cost device refreshes, one in flight; a refresh that does not fit asks for the view", async () => {
  const shown: [number, ViewPatch][] = [];
  let full = 0;
  const asked: string[][] = [];
  let answer: (ids: string[]) => DevicesView = (ids) =>
    devicesView(5, ids.map((id) => ({ track_id: "k", device: device(id, { params: { ...defaults("sonara.dynamics.compressor"), mix: 0.5 } }) })));
  const refresher = new ViewRefresher(
    async () => {
      full += 1;
      return chainView(full === 1 ? 1 : 9);
    },
    (view, patched) => shown.push([view.version, patched]),
    () => {},
    null,
    null,
    async (ids) => {
      asked.push(ids);
      return answer(ids);
    },
  );
  refresher.notice();
  await refresher.settled();
  for (let i = 0; i < 3; i++) refresher.notice(notice("command", ["devices"], [["device", "dev_1"]]));
  await refresher.settled();
  // The first edit's refresh, then one for the two that came while it was in flight.
  expect(asked).toEqual([["dev_1"], ["dev_1"]]);
  expect(shown).toEqual([
    [1, null],
    [5, "devices"],
    [5, "devices"],
  ]);
  expect(refresher.deviceRequests).toBe(2);
  // A device the view does not have: the whole view instead.
  answer = () => devicesView(6, [{ track_id: "k", device: device("dev_gone") }]);
  refresher.notice(notice("command", ["devices"], [["device", "dev_gone"]]));
  await refresher.settled();
  expect(shown.at(-1)).toEqual([9, null]);
  expect(full).toBe(2);
});
