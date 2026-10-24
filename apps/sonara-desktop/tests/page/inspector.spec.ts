import { expect, type Page, test } from "@playwright/test";
import { audioClip, midiClip, project, track, view } from "../support/project";
import type { AudioClipView, OpenProject } from "../../src/lib/types";
import { AUDIO_SUFFIXES, DATA_LAYER, PANES, answer, calls, emit, mockTauri } from "./tauri-mock";

// The Inspector (–IN-15) against mocked commands in WebKit: its groups,
// the edits each field sends (the prototype's commands, never a seal), and the fixed defects G4 (a
// fade keeps its curve), G5 (the inspector shows its own edits) and G6 (an edit from elsewhere
// never replaces what is being typed).

const OUTCOME = {
  committed: true,
  version: 2,
  label: "Edit",
  changes: { created: [], updated: [], deleted: [], flags: [], structure_changed: false },
  result: {},
};

function song(): OpenProject {
  const tone = {
    ...audioClip("a1", "asset1", 4, 8),
    fade_in: { seconds: 0.1, curve: "linear" as const },
    fade_out: { seconds: 0.2, curve: "s_curve" as const },
  };
  const vox = {
    ...track("vox", "Vox", "audio", [tone]),
    input: { channels: [0, 1], armed: false, monitor: "auto" as const },
    output_targets: [
      { id: "master", name: "Master" },
      { id: "bus", name: "Verb" },
    ],
  };
  const keys = {
    ...track("keys", "Keys", "instrument", [midiClip("m1", 0, 4, { name: "Riff", loop_length: null })]),
    output_targets: [{ id: "master", name: "Master" }],
  };
  return project([vox, keys, track("bus", "Verb", "bus")], {
    name: "Song",
    assets: [
      {
        id: "asset1",
        kind: "audio",
        name: "tone.wav",
        location: "embedded",
        sample_rate: 44100,
        channels: 2,
        frames: 88200,
        duration_seconds: 2,
      },
    ],
  });
}

async function openInspector(page: Page, open: OpenProject = song()) {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: { agent: { on: false, address: null }, messages: [], panes: PANES, audio_suffixes: AUDIO_SUFFIXES },
    recoverable_sessions: [],
    project_view: view(1, open),
    edit: OUTCOME,
    seal_undo: null,
  });
  await page.goto("/");
  await expect(inspector(page).getByRole("textbox", { name: "Name" })).toHaveValue(open.name);
}

const inspector = (page: Page) => page.getByRole("complementary", { name: "Inspector" });

async function select(page: Page, clips: string[], currentTrack: string | null) {
  await emit(page, "gui-selection", {
    from: "mixer",
    selection: { tracks: currentTrack ? [currentTrack] : [], clips, notes: [], currentTrack, currentClip: null, currentDevice: null },
  });
}

async function edits(page: Page): Promise<unknown[]> {
  return (await calls(page)).filter((c) => c.cmd === "edit").map((c) => (c.args as { edit: unknown }).edit);
}

async function refresh(page: Page, version: number, open: OpenProject, source = "gui") {
  await answer(page, "project_view", view(version, open));
  await emit(page, "session-changed", {
    version,
    kind: "command",
    label: "Edit",
    source,
    changes: { created: [], updated: [], deleted: [], flags: ["clips"], structure_changed: false },
  });
}

test("the Project group edits the name, the song end and the metadata", async ({ page }) => {
  await openInspector(page);
  const group = inspector(page).getByRole("region", { name: "Project" });
  await expect(group.getByText("48000 Hz")).toBeVisible();
  const end = group.getByRole("spinbutton", { name: "Song end" });
  await expect(end).toHaveValue("auto (end of content)");
  await end.fill("64");
  await end.press("Enter");
  await group.getByRole("textbox", { name: "Name" }).fill("Night Drive");
  await group.getByRole("textbox", { name: "Name" }).press("Enter");
  const artist = group.getByRole("textbox", { name: "Artist" });
  // Unchanged metadata is not sent; a change is, once.
  await artist.focus();
  await artist.press("Enter");
  await artist.fill("Filip");
  await artist.press("Tab");
  // Escape gives the typed text up.
  await group.getByRole("textbox", { name: "Genre" }).fill("Synth");
  await group.getByRole("textbox", { name: "Genre" }).press("Escape");
  await expect(group.getByRole("textbox", { name: "Genre" })).toHaveValue("");
  await group.getByRole("textbox", { name: "Genre" }).blur();
  await expect.poll(() => edits(page)).toEqual([
    { op: "set_project_length", length_beats: 64 },
    { op: "rename_project", name: "Night Drive" },
    { op: "set_metadata", values: { artist: "Filip" } },
  ]);
  expect((await calls(page)).map((c) => c.cmd)).not.toContain("seal_undo");
});

test("the Clip group of an audio clip: name, set length, mute, gain and media", async ({ page }) => {
  await openInspector(page);
  await select(page, ["a1"], "vox");
  const clip = inspector(page).getByRole("region", { name: "Clip" });
  await expect(clip).toBeVisible();
  await expect(clip.getByTestId("clip-start")).toHaveText("2.1.000  (2.000 s)");
  await expect(clip.getByTestId("clip-length")).toHaveText("8 beats  (4.000 s)");
  await expect(clip.getByTestId("clip-media")).toHaveText("tone.wav\n44100 Hz · 2 ch · 2.00 s · embedded");
  await clip.getByRole("textbox", { name: "Name" }).fill("Lead vocal");
  await clip.getByRole("textbox", { name: "Name" }).press("Enter");
  const length = clip.getByRole("spinbutton", { name: "Set length" });
  await expect(length).toHaveValue("8.000 beats");
  await length.press("ArrowUp");
  await clip.getByRole("checkbox", { name: "Muted" }).check();
  const gain = clip.getByRole("spinbutton", { name: "Gain" });
  await gain.fill("-200");
  await gain.press("Enter");
  await expect.poll(() => edits(page)).toEqual([
    { op: "set_clip_properties", clip_id: "a1", name: "Lead vocal" },
    { op: "trim_clip", clip_id: "a1", end: 13 },
    { op: "set_clip_properties", clip_id: "a1", muted: true },
    // Clamped as the spin box clamps it.
    { op: "set_clip_properties", clip_id: "a1", gain_db: -96 },
  ]);
  // Two clips selected: the Track group of the current track instead.
  await select(page, ["a1", "m1"], "vox");
  await expect(inspector(page).getByRole("region", { name: "Clip" })).toHaveCount(0);
  await expect(inspector(page).getByRole("region", { name: "Track · audio" })).toBeVisible();
});

test("G4: each fade keeps its own curve; an edit sends only its fade", async ({ page }) => {
  await openInspector(page);
  await select(page, ["a1"], "vox");
  const clip = inspector(page).getByRole("region", { name: "Clip" });
  await expect(clip.getByRole("combobox", { name: "Fade-in curve" })).toHaveValue("linear");
  await expect(clip.getByRole("combobox", { name: "Fade-out curve" })).toHaveValue("s_curve");
  const fadeIn = clip.getByRole("spinbutton", { name: "Fade in" });
  await fadeIn.fill("0.25");
  await fadeIn.press("Enter");
  await clip.getByRole("combobox", { name: "Fade-out curve" }).selectOption("exponential");
  await expect.poll(() => edits(page)).toEqual([
    { op: "set_clip_properties", clip_id: "a1", fade_in: { seconds: 0.25, curve: "linear" } },
    { op: "set_clip_properties", clip_id: "a1", fade_out: { seconds: 0.2, curve: "exponential" } },
  ]);
});

test("G5: the inspector's own edit shows at once, the derived rows included", async ({ page }) => {
  await openInspector(page);
  await select(page, ["a1"], "vox");
  const clip = inspector(page).getByRole("region", { name: "Clip" });
  const length = clip.getByRole("spinbutton", { name: "Set length" });
  await length.fill("4");
  await length.press("Enter");
  const shorter = song();
  shorter.tracks[0].clips[0] = { ...shorter.tracks[0].clips[0], length: 4 };
  await refresh(page, 2, shorter);
  await expect(clip.getByTestId("clip-length")).toHaveText("4 beats  (2.000 s)");
  await expect(length).toHaveValue("4.000 beats");
});

test("G6: an edit from elsewhere never replaces what is being typed, nor takes the focus", async ({ page }) => {
  await openInspector(page);
  await select(page, ["a1"], "vox");
  const clip = inspector(page).getByRole("region", { name: "Clip" });
  const name = clip.getByRole("textbox", { name: "Name" });
  await name.fill("typed, not sent");
  const fromAgent = song();
  fromAgent.tracks[0] = { ...fromAgent.tracks[0], name: "Renamed by agent" };
  fromAgent.tracks[0].clips[0] = { ...(fromAgent.tracks[0].clips[0] as AudioClipView), gain_db: -6 };
  await refresh(page, 2, fromAgent, "agent");
  // The other fields follow the agent's edit in place.
  await expect(clip.getByText("Renamed by agent")).toBeVisible();
  await expect(clip.getByRole("spinbutton", { name: "Gain" })).toHaveValue("-6.0 dB");
  await expect(name).toHaveValue("typed, not sent");
  await expect(name).toBeFocused();
  await name.press("Enter");
  await expect.poll(() => edits(page)).toEqual([{ op: "set_clip_properties", clip_id: "a1", name: "typed, not sent" }]);
});

test("the Track group: name, volume, pan, output, monitoring and input", async ({ page }) => {
  await openInspector(page);
  await select(page, [], "vox");
  const group = inspector(page).getByRole("region", { name: "Track · audio" });
  const volume = group.getByRole("spinbutton", { name: "Volume" });
  await expect(volume).toHaveValue("0.0 dB");
  await volume.fill("-6");
  await volume.press("Enter");
  await group.getByRole("spinbutton", { name: "Pan" }).press("ArrowDown");
  await group.getByRole("combobox", { name: "Output" }).selectOption({ label: "Verb" });
  await group.getByRole("combobox", { name: "Monitoring" }).selectOption("on");
  await expect(group.getByRole("combobox", { name: "Input" })).toHaveValue("2");
  await group.getByRole("combobox", { name: "Input" }).selectOption({ label: "In 2" });
  await expect.poll(() => edits(page)).toEqual([
    { op: "set_track_mixer", track_id: "vox", volume_db: -6 },
    { op: "set_track_mixer", track_id: "vox", pan: -0.05 },
    { op: "set_track_output", track_id: "vox", output_id: "bus" },
    { op: "set_track_input", track_id: "vox", monitor: "on" },
    { op: "set_track_input", track_id: "vox", channels: [1] },
  ]);
  await expect(group.getByText("1", { exact: true })).toBeVisible();
  await expect(group.getByTestId("track-effects")).toHaveText("—");
});

test("a refused output route shows the model's output again", async ({ page }) => {
  await openInspector(page);
  await answer(page, "edit", { reject: { code: "ROUTING_CYCLE", message: "routing would create a loop" } });
  await select(page, [], "vox");
  const output = inspector(page).getByRole("region", { name: "Track · audio" }).getByRole("combobox", { name: "Output" });
  await output.selectOption({ label: "Verb" });
  await expect(page.getByRole("alert")).toHaveText("routing would create a loop");
  await expect(output).toHaveValue("master");
});

test("a MIDI clip's loop: the switch and its length, disabled while off", async ({ page }) => {
  await openInspector(page);
  await select(page, ["m1"], "keys");
  const clip = inspector(page).getByRole("region", { name: "Clip" });
  await expect(clip.getByText("4", { exact: true })).toBeVisible();
  const length = clip.getByRole("spinbutton", { name: "Loop length" });
  await expect(length).toBeDisabled();
  await expect(length).toHaveValue("2.00 beats");
  await clip.getByRole("checkbox", { name: "Loop content" }).check();
  await expect.poll(() => edits(page)).toEqual([{ op: "set_clip_properties", clip_id: "m1", loop_length: 2 }]);
  // The master's group has no Output; a project without tracks shows only the Project group.
  await select(page, [], "master");
  await expect(inspector(page).getByRole("region", { name: "Track · master" })).toBeVisible();
  await expect(inspector(page).getByRole("combobox", { name: "Output" })).toHaveCount(0);
});
