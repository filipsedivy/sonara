import { expect, type Page, test } from "@playwright/test";
import fixture from "../fixtures/dialogs-v1.json" with { type: "json" };
import { project, track, view } from "../support/project";
import { AUDIO_SUFFIXES, DATA_LAYER, PANES, answer, calls, emit, mockTauri } from "./tauri-mock";
import { trackRow } from "../support/tracks";

// The dialogs (–DL-06) against mocked commands in WebKit: New Project
// (and G3: a wrong field is named and the dialog stays open), Settings (and G11: a stored value
// outside the choices is offered), Export Audio (the presets and the form it sends), Agent
// Connection and About (`test_about_brand.py::about_dialog_shows_author_and_contact`), Import
// Audio…, and the window's shortcuts waiting while a dialog is open.

const NEW_PROJECT_VIEW = {
  form: { name: "Untitled", template: "empty", bpm: 120, time_signature: "4/4", sample_rate: 48000 },
  templates: fixture.new_project.fields.templates.map(([id, name, description]) => ({ id, name, description })),
  signatures: fixture.new_project.fields.signatures,
  rates: fixture.new_project.fields.rates,
  bpm: [10, 999],
};

const SETTINGS_VIEW = {
  form: {
    backend: "auto",
    output_device: null,
    input_device: null,
    buffer_frames: 480,
    lookahead_ms: 80,
    autosave_seconds: 60,
    backup_retention: 10,
    undo_limit: 500,
    theme: "dark",
    agent_require_confirmation: true,
    agent_allowed_paths: "",
  },
  backends: fixture.settings.choices.backends.map(([label, value]) => ({ label, value })),
  outputs: [
    { label: "System default", value: null },
    { label: "Speakers (default)", value: "out-1" },
  ],
  inputs: [{ label: "System default", value: null }],
  devices_error: null,
  buffers: [64, 128, 256, 480, 512, 1024, 2048].map((value) => ({ label: `${value} frames`, value })),
  themes: fixture.settings.choices.themes.map(([label, value]) => ({ label, value })),
  lookahead_ms: [20, 1000],
  autosave_seconds: [0, 3600],
  backup_retention: [0, 100],
  undo_limit: [10, 10000],
};

const EXPORT_FORM = {
  path: "/Users/me/Music/Song",
  preset: "Custom",
  formats: ["wav"],
  bit_depth: "24",
  sample_rate: null,
  channels: 2,
  bitrate_kbps: 320,
  dither: true,
  range: "song",
  tail: "auto",
  normalize: "none",
  target: -14,
  ceiling: -1,
  stems: false,
  through_master: false,
  stem_tracks: ["t", "b"],
};

const EXPORT_VIEW = {
  form: EXPORT_FORM,
  formats: fixture.export.format_labels.map(([value, label]) => ({ label, value })),
  presets: ["Custom", ...Object.keys(fixture.export.presets)],
  rates: [{ label: "Project rate", value: null }, ...[44100, 48000, 88200, 96000].map((r) => ({ label: `${r / 1000} kHz`, value: r }))],
  bitrates: [320, 256, 192, 160, 128, 96].map((kbps) => ({ label: `${kbps} kbps`, value: kbps })),
  stem_tracks: [
    { label: "Synth", value: "t" },
    { label: "Bass", value: "b" },
  ],
  target_db: [-60, 0],
  ceiling_db: [-20, 0],
};

async function openWindow(page: Page, extra: Record<string, unknown> = {}) {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: { agent: { on: true, address: "/tmp/s.sock" }, messages: [], panes: PANES, audio_suffixes: AUDIO_SUFFIXES },
    recoverable_sessions: [],
    project_view: view(1, project([track("t", "Synth"), track("b", "Bass"), track("au", "Vox", "audio")], { name: "Song" })),
    transport_play: { state: "playing", position_beat: 0 },
    ...extra,
  });
  await page.goto("/");
  await expect(page.getByTestId("timeline-info")).not.toHaveText("");
}

async function argsOf(page: Page, cmd: string): Promise<unknown[]> {
  return (await calls(page)).filter((c) => c.cmd === cmd).map((c) => c.args);
}

test("New Project shows the prototype's fields and sends the choices", async ({ page }) => {
  await openWindow(page, { new_project_view: NEW_PROJECT_VIEW, create_project: { created: true } });
  await emit(page, "dialog-command", { dialog: "new-project" });
  const dialog = page.getByRole("dialog", { name: "New Project" });
  await expect(dialog.getByRole("textbox", { name: "Name" })).toHaveValue("Untitled");
  await expect(dialog.getByRole("combobox", { name: "Template" }).locator("option")).toHaveText([
    "Empty",
    "Electronic",
    "Band",
    "Orchestral",
  ]);
  await expect(dialog.getByRole("combobox", { name: "Template" }).locator("option").nth(1)).toHaveAttribute(
    "title",
    /Drums, bass/,
  );
  await expect(dialog.getByRole("combobox", { name: "Sample rate" })).toHaveValue("48000");
  await dialog.getByRole("textbox", { name: "Name" }).fill("Night Drive");
  await dialog.getByRole("combobox", { name: "Template" }).selectOption("band");
  const bpm = dialog.getByRole("spinbutton", { name: "Tempo (BPM)" });
  await bpm.fill("97.255");
  await bpm.press("Tab");
  await dialog.getByRole("combobox", { name: "Time signature" }).fill("7/8");
  await dialog.getByRole("combobox", { name: "Sample rate" }).selectOption({ label: "44.1 kHz" });
  await dialog.getByRole("button", { name: "OK" }).click();
  await expect(dialog).toHaveCount(0);
  expect(await argsOf(page, "create_project")).toEqual([
    { form: { name: "Night Drive", template: "band", bpm: 97.25, time_signature: "7/8", sample_rate: 44100 } },
  ]);
});

test("G3: a wrong field is named and the dialog stays open", async ({ page }) => {
  await openWindow(page, {
    new_project_view: NEW_PROJECT_VIEW,
    create_project: {
      reject: { field: "time_signature", code: "INVALID_ARGUMENT", message: "invalid time signature 'x/4'" },
    },
  });
  await emit(page, "dialog-command", { dialog: "new-project" });
  const dialog = page.getByRole("dialog", { name: "New Project" });
  const signature = dialog.getByRole("combobox", { name: "Time signature" });
  await signature.fill("x/4");
  await signature.press("Enter");
  await expect(dialog.getByRole("alert")).toHaveText("invalid time signature 'x/4'");
  await expect(signature).toHaveAttribute("aria-invalid", "true");
  await expect(signature).toBeFocused();
  await answer(page, "create_project", { created: false });
  await signature.fill("4/4");
  await dialog.getByRole("button", { name: "OK" }).click();
  // Cancelled at the unsaved-changes prompt: the dialog closes, as the prototype's did.
  await expect(dialog).toHaveCount(0);
});

test("Settings offers the stored values and sends the form", async ({ page }) => {
  await openWindow(page, { settings_view: SETTINGS_VIEW, apply_settings: { autosave_seconds: 0 } });
  await emit(page, "dialog-command", { dialog: "settings" });
  const dialog = page.getByRole("dialog", { name: "Settings" });
  await expect(dialog.getByRole("combobox", { name: "Device buffer" })).toHaveValue("480");
  await expect(dialog.getByRole("combobox", { name: "Engine" }).locator("option")).toHaveText([
    "Automatic",
    "System audio (native)",
    "Simulated device (testing)",
    "Off (offline only)",
  ]);
  await expect(dialog.getByText("Changes to the audio engine apply after restarting Sonara.")).toBeVisible();
  await dialog.getByRole("combobox", { name: "Engine" }).selectOption("native-null");
  await dialog.getByRole("combobox", { name: "Output device" }).selectOption({ label: "Speakers (default)" });
  const lookahead = dialog.getByRole("spinbutton", { name: "Render lookahead" });
  await lookahead.fill("37.555");
  await lookahead.press("Tab");
  const autosave = dialog.getByRole("spinbutton", { name: "Autosave interval" });
  await autosave.fill("0");
  await autosave.press("Tab");
  await expect(autosave).toHaveValue("off");
  await dialog.getByRole("combobox", { name: "Theme" }).selectOption("light");
  await dialog.getByRole("checkbox", { name: /Require confirmation/ }).uncheck();
  await dialog.getByRole("textbox", { name: "Allowed folders" }).fill("/Volumes/Audio\n~/Samples");
  await dialog.getByRole("button", { name: "OK" }).click();
  await expect(dialog).toHaveCount(0);
  expect(await argsOf(page, "apply_settings")).toEqual([
    {
      form: {
        ...SETTINGS_VIEW.form,
        backend: "native-null",
        output_device: "out-1",
        lookahead_ms: 37.55,
        autosave_seconds: 0,
        theme: "light",
        agent_require_confirmation: false,
        agent_allowed_paths: "/Volumes/Audio\n~/Samples",
      },
    },
  ]);
});

test("a settings file that cannot be saved is reported in the dialog", async ({ page }) => {
  await openWindow(page, {
    settings_view: SETTINGS_VIEW,
    apply_settings: { reject: { code: "IO_ERROR", message: "could not save settings: read-only" } },
  });
  await emit(page, "dialog-command", { dialog: "settings" });
  const dialog = page.getByRole("dialog", { name: "Settings" });
  await dialog.getByRole("button", { name: "OK" }).click();
  await expect(dialog.getByRole("alert")).toHaveText("could not save settings: read-only");
  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(dialog).toHaveCount(0);
});

test("Export Audio applies a preset and sends the form; a refusal stays in the dialog", async ({ page }) => {
  const podcast = { ...EXPORT_FORM, preset: "Podcast/Web (MP3 192 + Opus)", formats: ["mp3", "opus"], bitrate_kbps: 192 };
  await openWindow(page, {
    export_view: EXPORT_VIEW,
    export_preset: podcast,
    export_audio: { reject: { code: "INVALID_ARGUMENT", message: "choose at least one format" } },
    export_browse: "/Users/me/Exports/Night",
  });
  await emit(page, "dialog-command", { dialog: "export-audio" });
  const dialog = page.getByRole("dialog", { name: "Export Audio" });
  await expect(dialog.getByRole("textbox", { name: "File (without extension)" })).toHaveValue("/Users/me/Music/Song");
  await expect(dialog.getByRole("group", { name: "Formats" }).getByRole("checkbox")).toHaveCount(10);
  await expect(dialog.getByRole("checkbox", { name: "WAV", exact: true })).toBeChecked();
  await dialog.getByRole("checkbox", { name: "WAV", exact: true }).uncheck();
  await dialog.getByRole("button", { name: "Export" }).click();
  await expect(dialog.getByRole("alert")).toHaveText("choose at least one format");
  await dialog.getByRole("combobox", { name: "Preset" }).selectOption("Podcast/Web (MP3 192 + Opus)");
  await expect(dialog.getByRole("combobox", { name: "Lossy bitrate" })).toHaveValue("192");
  await expect(dialog.getByRole("checkbox", { name: "MP3" })).toBeChecked();
  await dialog.getByRole("button", { name: "Browse…" }).click();
  await expect(dialog.getByRole("textbox", { name: "File (without extension)" })).toHaveValue("/Users/me/Exports/Night");
  await dialog.getByRole("combobox", { name: "Tail" }).selectOption({ label: "2 seconds" });
  await dialog.getByRole("combobox", { name: "Normalize" }).selectOption("loudness");
  await dialog.getByRole("checkbox", { name: "Also export each track as a stem" }).check();
  await dialog.getByRole("group", { name: "Stem tracks" }).getByRole("checkbox", { name: "Synth" }).uncheck();
  await answer(page, "export_audio", { started: true, jobs: ["op_1", "op_2"] });
  await dialog.getByRole("button", { name: "Export" }).click();
  await expect(dialog).toHaveCount(0);
  const sent = await argsOf(page, "export_audio");
  expect(sent.at(-1)).toEqual({
    form: { ...podcast, path: "/Users/me/Exports/Night", tail: 2, normalize: "loudness", stems: true, stem_tracks: ["b"] },
  });
  expect(await argsOf(page, "export_preset")).toEqual([
    { form: { ...EXPORT_FORM, formats: [] }, preset: "Podcast/Web (MP3 192 + Opus)" },
  ]);
});

test("G13: the archive preset shows and sends 32-bit float, its FLAC checked beside the WAV", async ({ page }) => {
  const archive = { ...EXPORT_FORM, preset: "Archive (FLAC 24-bit + 32-bit float WAV)", formats: ["wav", "flac"], bit_depth: "32f" };
  await openWindow(page, {
    export_view: EXPORT_VIEW,
    export_preset: archive,
    export_audio: { started: true, jobs: ["op_1"] },
  });
  await emit(page, "dialog-command", { dialog: "export-audio" });
  const dialog = page.getByRole("dialog", { name: "Export Audio" });
  await expect(dialog.getByRole("combobox", { name: "PCM bit depth" })).toHaveValue("24");
  await dialog.getByRole("combobox", { name: "Preset" }).selectOption("Archive (FLAC 24-bit + 32-bit float WAV)");
  const depth = dialog.getByRole("combobox", { name: "PCM bit depth" });
  await expect(depth).toHaveValue("32f");
  await expect(depth.locator("option:checked")).toHaveText("32-bit float");
  await expect(dialog.getByRole("checkbox", { name: "WAV", exact: true })).toBeChecked();
  await expect(dialog.getByRole("checkbox", { name: "FLAC" })).toBeChecked();
  await dialog.getByRole("button", { name: "Export" }).click();
  await expect(dialog).toHaveCount(0);
  expect(await argsOf(page, "export_audio")).toEqual([{ form: archive }]);
});

test("Agent Connection names the bundled sonara mcp", async ({ page }) => {
  const text = "Register Sonara with Claude Code (once):\n\n    claude mcp add sonara -- /Applications/Sonara.app/Contents/MacOS/sonara mcp\n";
  await openWindow(page, {
    agent_connection: {
      status: "This window accepts agent connections at /tmp/s.sock.",
      address: "/tmp/s.sock",
      cli: "/Applications/Sonara.app/Contents/MacOS/sonara",
      command: "claude mcp add sonara -- /Applications/Sonara.app/Contents/MacOS/sonara mcp",
      text,
      discovery: "/tmp/run/gui.json",
    },
  });
  await emit(page, "dialog-command", { dialog: "agent-connection" });
  const dialog = page.getByRole("dialog", { name: "Agent Connection" });
  await expect(dialog.getByText("/tmp/s.sock", { exact: true })).toBeVisible();
  await expect(dialog.getByRole("textbox")).toHaveValue(text);
  await dialog.getByRole("button", { name: "Close" }).click();
  await expect(dialog).toHaveCount(0);
});

test("About shows the author, the contact, the version and the system info", async ({ page }) => {
  await openWindow(page, {
    about_info: {
      product: "Sonara",
      version: "Version 0.1.0 (alpha)",
      tagline: "A digital audio workstation with a first-class agent interface.",
      author: "Filip Sedivy",
      email: "mail@filipsedivy.cz",
      copyright: "© 2026 Filip Sedivy. All rights reserved.",
      system: [
        ["Sonara", "0.1.0 (alpha)"],
        ["Audio", "test"],
      ],
      credits: [{ name: "Tauri", license: "Apache-2.0 or MIT", url: "https://tauri.app" }],
      lgpl_note: "LGPL components (LAME, libsoxr) are separate dynamic libraries.",
      licenses: null,
    },
    open_link: null,
  });
  await emit(page, "dialog-command", { dialog: "about" });
  const dialog = page.getByRole("dialog", { name: "About Sonara" });
  await expect(dialog.getByRole("img", { name: "Sonara logo" })).toBeVisible();
  await expect(dialog.getByText("Created by Filip Sedivy")).toBeVisible();
  await expect(dialog.getByRole("link", { name: "mail@filipsedivy.cz" })).toHaveAttribute("href", "mailto:mail@filipsedivy.cz");
  await expect(dialog.getByRole("textbox", { name: "System" })).toHaveValue("Sonara: 0.1.0 (alpha)\nAudio: test");
  await dialog.getByRole("link", { name: "mail@filipsedivy.cz" }).click();
  await dialog.getByRole("tab", { name: "Credits" }).click();
  await dialog.getByRole("link", { name: "Tauri" }).click();
  expect(await argsOf(page, "open_link")).toEqual([{ url: "mailto:mail@filipsedivy.cz" }, { url: "https://tauri.app" }]);
  await dialog.getByRole("button", { name: "Copy System Info" }).click();
  await expect(dialog.getByRole("button", { name: "Copied" })).toBeVisible();
});

test("Import Audio… opens the panel for the selected audio track at the playhead", async ({ page }) => {
  await openWindow(page, { import_audio_dialog: null });
  await emit(page, "dialog-command", { dialog: "import-audio" });
  await trackRow(page, "Vox").locator(".name").click();
  await emit(page, "dialog-command", { dialog: "import-audio" });
  await trackRow(page, "Synth").locator(".name").click();
  await emit(page, "dialog-command", { dialog: "import-audio" });
  await expect.poll(() => argsOf(page, "import_audio_dialog")).toEqual([
    { trackId: null, startBeat: 0 },
    { trackId: "au", startBeat: 0 },
    { trackId: null, startBeat: 0 },
  ]);
});

test("the single-key shortcuts wait while a dialog is open", async ({ page }) => {
  await openWindow(page, { new_project_view: NEW_PROJECT_VIEW });
  await emit(page, "dialog-command", { dialog: "new-project" });
  const dialog = page.getByRole("dialog", { name: "New Project" });
  await expect(dialog.getByRole("textbox", { name: "Name" })).toHaveValue("Untitled");
  await dialog.getByRole("button", { name: "Cancel" }).focus();
  await page.keyboard.press("l");
  await page.keyboard.press("c");
  // Space presses the focused button, and only that.
  await page.keyboard.press("Space");
  await expect(dialog).toHaveCount(0);
  expect((await calls(page)).map((c) => c.cmd)).not.toContain("transport_play");
  expect((await calls(page)).filter((c) => c.cmd === "edit")).toEqual([]);
  // Without a dialog, Space plays again.
  await page.getByTestId("timeline-info").click();
  await page.keyboard.press("Space");
  await expect.poll(async () => (await calls(page)).map((c) => c.cmd)).toContain("transport_play");
});
