import { expect, test } from "@playwright/test";
import fixture from "../fixtures/live-frame-v1.json" with { type: "json" };
import { DATA_LAYER, answer, calls, emit, mockTauri, sendChannel } from "./tauri-mock";
import { trackList } from "../support/tracks";

// The data layer against mocked commands: the project view is fetched at startup
// and again after session events, live frames arrive on the channel and reach the page at the
// next display frame, and the MIDI focus is sent with the selection.

const SOCKET = "/Users/me/Library/Application Support/Sonara/run/sonara-4242.sock";

function track(id: string, name: string) {
  return {
    id,
    name,
    kind: "audio",
    color: null,
    parent_id: null,
    depth: 0,
    mixer: { volume_db: 0, pan: 0, mute: false, solo: false, solo_safe: false },
    output_id: "master",
    output_targets: [],
    sends: [],
    send_targets: [],
    input: { channels: [], armed: false, monitor: "auto" },
    instrument: null,
    inserts: [],
    clips: [],
  };
}

function view(version: number, names: string[]) {
  return {
    version,
    project: {
      id: "prj_1",
      name: "Night Drive",
      path: null,
      dirty: true,
      sample_rate: 48000,
      length_beats: null,
      end_beat: 0,
      tempo: [{ beat: 0, bpm: 120, ramp: false }],
      time_signatures: [{ bar: 0, beat: 0, numerator: 4, denominator: 4, beats_per_bar: 4 }],
      transport: {
        loop_enabled: false,
        loop_start: 0,
        loop_end: 16,
        punch_enabled: false,
        punch_in: 0,
        punch_out: 16,
        metronome: false,
        count_in_bars: 0,
      },
      metadata: { title: "", artist: "", album: "", genre: "", composer: "", comment: "" },
      tracks: names.map((name, i) => track(`trk_${i}`, name)),
      master: { ...track("master", "Master"), kind: "master" },
      markers: [],
      sections: [],
      assets: [],
      plugins: {},
      history: { undo: null, redo: null },
      meter_layout: 7,
    },
  };
}

function notice(version: number, flags: string[], structure: boolean) {
  return {
    version,
    kind: "command",
    label: "Create Track",
    source: "agent",
    changes: { created: [], updated: [], deleted: [], flags, structure_changed: structure },
  };
}

test("the project view is fetched at startup and after every session event", async ({ page }) => {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: { agent: { on: true, address: SOCKET }, messages: [] },
    recoverable_sessions: [],
    project_view: view(3, ["Drums", "Bass"]),
  });
  await page.goto("/");
  const tracks = trackList(page).locator(".name");
  await expect(tracks).toHaveText(["Drums", "Bass"]);
  await expect(page.getByTestId("timeline-info")).toHaveText("120 BPM  4/4  ·  2 tracks");
  await answer(page, "project_view", view(4, ["Drums", "Bass", "Agent Track"]));
  await emit(page, "session-changed", notice(4, ["routing", "tracks"], true));
  await expect(page.getByTestId("timeline-info")).toHaveText("120 BPM  4/4  ·  3 tracks");
  await expect(tracks).toHaveText(["Drums", "Bass", "Agent Track"]);
  const views = (await calls(page)).filter((c) => c.cmd === "project_view");
  expect(views).toHaveLength(2);
  // A stale answer (an older version) does not replace the view.
  await answer(page, "project_view", view(2, ["Old"]));
  await emit(page, "session-changed", notice(5, ["mixer"], false));
  await expect
    .poll(async () => (await calls(page)).filter((c) => c.cmd === "project_view").length)
    .toBe(3);
  await expect(page.getByTestId("timeline-info")).toHaveText("120 BPM  4/4  ·  3 tracks");
});

test("live frames reach the page, and the focus is sent when the transport appears", async ({
  page,
}) => {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: { agent: { on: false, address: null }, messages: [] },
    recoverable_sessions: [],
    project_view: view(1, ["Drums"]),
  });
  await page.goto("/");
  const play = page.getByTestId("play");
  await expect(play).toHaveAttribute("aria-pressed", "false");
  expect((await calls(page)).map((c) => c.cmd)).toContain("subscribe_live");
  const focus = async () =>
    (await calls(page)).filter((c) => c.cmd === "set_midi_focus").map((c) => c.args);
  await expect.poll(focus).toEqual([{ trackId: null }]);
  const [playing, , absent] = fixture.frames;
  // No transport yet, then a running one: the focus goes again to the new transport.
  await sendChannel(page, absent.hex);
  await expect(play).toHaveAttribute("aria-pressed", "false");
  await sendChannel(page, playing.hex);
  await expect(play).toHaveAttribute("aria-pressed", "true");
  await expect.poll(focus).toEqual([{ trackId: null }, { trackId: null }]);
});

test("a failing view request is reported in the status bar", async ({ page }) => {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: { agent: { on: false, address: null }, messages: [] },
    recoverable_sessions: [],
    project_view: { reject: { code: "INTERNAL", message: "the session is gone" } },
  });
  await page.goto("/");
  await expect(page.getByRole("alert")).toHaveText("the session is gone");
});
