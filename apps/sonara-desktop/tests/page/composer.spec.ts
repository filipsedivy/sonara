import { expect, type Page, test } from "@playwright/test";
import { project, track, view } from "../support/project";
import { AUDIO_SUFFIXES, DATA_LAYER, PANES, calls, emit, mockTauri, sendChannelMessage } from "./tauri-mock";

// The AI Composer's panel against mocked commands in WebKit: it opens from the AI menu without
// blocking the window, checks that Claude Code can run, sends the prompt, shows the run's
// progress as it streams, stops it, and ends with everything the run returned.

const READY = {
  ready: true,
  running: false,
  claude_path: "/opt/homebrew/bin/claude",
  claude_version: "2.1.312",
  problem: null,
};

async function openWindow(page: Page, status: Record<string, unknown>) {
  await mockTauri(page, {
    ...DATA_LAYER,
    shell_status: { agent: { on: true, address: "/tmp/s.sock" }, messages: [], panes: PANES, audio_suffixes: AUDIO_SUFFIXES },
    recoverable_sessions: [],
    project_view: view(1, project([track("t", "Synth")], { name: "Song" })),
    composer_check: status,
    composer_start: null,
    composer_stop: null,
  });
  await page.goto("/");
  await expect(page.getByTestId("timeline-info")).not.toHaveText("");
  await emit(page, "dialog-command", { dialog: "ai-composer" });
}

test("it is a panel, not a modal dialog, and checks Claude Code first", async ({ page }) => {
  await openWindow(page, READY);
  const panel = page.getByRole("region", { name: "AI Composer" });
  await expect(panel.getByText("Claude Code 2.1.312")).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  // The window stays usable: no modal state is reported to the menu.
  const contexts = (await calls(page)).filter((c) => c.cmd === "set_menu_context");
  expect(contexts.every((c) => (c.args as { context: { modal: boolean } }).context.modal === false)).toBe(true);
  await expect(panel.getByRole("button", { name: "Compose", exact: true })).toBeDisabled();
});

test("a missing Claude Code is explained and can be checked again", async ({ page }) => {
  await openWindow(page, { ...READY, ready: false, claude_path: null, claude_version: null, problem: "Claude Code is not installed" });
  const panel = page.getByRole("region", { name: "AI Composer" });
  await expect(panel.getByRole("alert")).toContainText("Claude Code is not installed");
  await panel.getByRole("textbox", { name: "Describe the music you want" }).fill("a beat");
  await expect(panel.getByRole("button", { name: "Compose", exact: true })).toBeDisabled();
  await panel.getByRole("button", { name: "Check again" }).click();
  await expect.poll(async () => (await calls(page)).filter((c) => c.cmd === "composer_check").length).toBe(2);
});

test("the prompt is sent, the progress streams in, and the result ends the run", async ({ page }) => {
  await openWindow(page, READY);
  const panel = page.getByRole("region", { name: "AI Composer" });
  await panel.getByRole("textbox", { name: "Describe the music you want" }).fill("  A chill lo-fi beat  ");
  await panel.getByRole("button", { name: "Compose", exact: true }).click();
  const started = (await calls(page)).filter((c) => c.cmd === "composer_start");
  expect(started).toHaveLength(1);
  expect((started[0].args as { prompt: string }).prompt).toBe("  A chill lo-fi beat  ");
  await expect(panel.getByRole("status")).toHaveText("Starting…");

  const send = (message: unknown) => sendChannelMessage(page, message);
  await send({ kind: "init", session_id: "s1", model: "claude-x", tool_count: 40, mcp_servers: [{ name: "sonara", status: "connected" }], mcp_errors: [] });
  await send({ kind: "text", text: "Setting the tempo." });
  await send({ kind: "tool_use", id: "t1", tool: "set_tempo", summary: '{"bpm":80}', nested: false });
  const log = panel.getByRole("log", { name: "Progress" });
  await expect(log.getByText("set_tempo")).toBeVisible();
  await expect(panel.getByRole("status")).toContainText("Composing…");
  await send({ kind: "tool_result", id: "t1", ok: true, preview: "{}" });
  await send({ kind: "tool_use", id: "t2", tool: "create_track", summary: "{}", nested: false });
  await send({ kind: "tool_result", id: "t2", ok: false, preview: '{"ok":false,"error":{"code":"INVALID_ARGUMENT"}}' });
  await expect(log.getByText("INVALID_ARGUMENT")).toBeVisible();

  await send({
    kind: "result",
    success: true,
    subtype: "success",
    text: "A slow lo-fi loop at 80 BPM.",
    duration_ms: 65_000,
    num_turns: 7,
    cost_usd: 0.12,
    session_id: "s1",
    usage: { input_tokens: 10, output_tokens: 20 },
    permission_denials: ["save_project"],
    raw: '{\n  "type": "result"\n}',
  });
  await send({ kind: "exited", code: 0, stopped: false });
  await expect(panel.getByRole("heading", { name: "Finished" })).toBeVisible();
  await expect(panel.getByText("A slow lo-fi loop at 80 BPM.")).toBeVisible();
  for (const text of ["1 min 05 s", "$0.1200", "10 in, 20 out", "save_project", "2, 1 failed"]) {
    await expect(panel.getByText(text)).toBeVisible();
  }
  await panel.getByText("Full output of Claude Code").click();
  await expect(panel.locator("pre").first()).toContainText('"type": "result"');
  // Ready for the next one.
  await expect(panel.getByRole("button", { name: "Compose", exact: true })).toBeEnabled();
});

test("Stop interrupts the run and the panel reports it as stopped", async ({ page }) => {
  await openWindow(page, READY);
  const panel = page.getByRole("region", { name: "AI Composer" });
  await panel.getByRole("textbox", { name: "Describe the music you want" }).fill("a beat");
  await panel.getByRole("button", { name: "Compose", exact: true }).click();
  await sendChannelMessage(page, { kind: "init", session_id: "s", model: "m", tool_count: 1, mcp_servers: [{ name: "sonara", status: "connected" }], mcp_errors: [] });
  await panel.getByRole("button", { name: "Stop" }).click();
  expect((await calls(page)).filter((c) => c.cmd === "composer_stop")).toHaveLength(1);
  await expect(panel.getByRole("status")).toHaveText("Stopping…");
  await sendChannelMessage(page, { kind: "exited", code: 130, stopped: true });
  await expect(panel.getByRole("heading", { name: "Stopped by you" })).toBeVisible();
});

test("a start that the desktop refuses is shown, and nothing runs", async ({ page }) => {
  await openWindow(page, READY);
  await page.evaluate(() => {
    window.__sonaraMock.answers.composer_start = { reject: { code: "ENGINE_UNAVAILABLE", message: "the agent server did not start" } };
  });
  const panel = page.getByRole("region", { name: "AI Composer" });
  await panel.getByRole("textbox", { name: "Describe the music you want" }).fill("a beat");
  await panel.getByRole("button", { name: "Compose", exact: true }).click();
  await expect(panel.getByRole("heading", { name: "Did not finish" })).toBeVisible();
  await expect(panel.getByText("the agent server did not start")).toBeVisible();
});

test("a Sonara connection that failed stops the run at once", async ({ page }) => {
  await openWindow(page, READY);
  const panel = page.getByRole("region", { name: "AI Composer" });
  await panel.getByRole("textbox", { name: "Describe the music you want" }).fill("a beat");
  await panel.getByRole("button", { name: "Compose", exact: true }).click();
  await sendChannelMessage(page, { kind: "init", session_id: "s", model: "m", tool_count: 1, mcp_servers: [{ name: "sonara", status: "failed" }], mcp_errors: [] });
  await expect.poll(async () => (await calls(page)).filter((c) => c.cmd === "composer_stop").length).toBe(1);
  await expect(panel.getByRole("alert")).toContainText("not connected");
});
