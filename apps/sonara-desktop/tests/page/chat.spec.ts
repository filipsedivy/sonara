import { expect, type Page, test } from "@playwright/test";
import type { ChatState, ProviderStatus, TaskLog, TaskView } from "../../src/lib/chat/api";
import { MIN_TEXT_CONTRAST, contrastRatio } from "../../src/lib/chat/contrast";
import { BETA_EXPLANATION, MAX_PROMPT_CHARS } from "../../src/lib/chat/model";
import { PALETTES, type ThemeName } from "../../src/lib/theme";
import { calls, emit, mockTauri, type Responses } from "./tauri-mock";

// The AI Chat window (webview label "chat") against mocked commands in WebKit. The chat lives in
// Rust (crates/sonara-app/src/chat): the window follows its `chat-event` stream, so every test
// that changes the thread emits the `changed` event the backend would send after the command.

test.use({ actionTimeout: 10_000 });

const task = (id: number, state: TaskView["state"] = "queued", extra: Partial<TaskView> = {}): TaskView => ({
  id,
  prompt: `prompt ${id}`,
  provider: "claude",
  state,
  error: null,
  result: null,
  ...extra,
});

const chatState = (tasks: TaskView[], extra: Partial<ChatState> = {}): ChatState => ({
  generation: 1,
  mode: "running",
  provider: "claude",
  provider_locked: tasks.length > 0,
  tasks,
  ...extra,
});

const changed = (state: ChatState) => emit(pageOf, "chat-event", { kind: "changed", state });

const READY_CLAUDE: ProviderStatus = {
  id: "claude",
  name: "Claude Code",
  availability: "ready",
  version: "2.1.312",
  path: "/opt/homebrew/bin/claude",
  hint: "Signed in.",
  beta: true,
  min_version: "2.1.0",
};
const SIGNED_OUT_CODEX: ProviderStatus = {
  id: "codex",
  name: "Codex CLI",
  availability: "not_signed_in",
  version: "0.166.0",
  path: "/opt/homebrew/bin/codex",
  hint: "Run `codex login` in a terminal.",
  beta: true,
  min_version: "0.160.0",
};

interface Options {
  state?: ChatState;
  logs?: TaskLog[];
  providers?: ProviderStatus[];
  theme?: ThemeName;
  extra?: Responses;
}

// The page of the current test, for the `changed` helper.
let pageOf: Page;

async function openChat(page: Page, options: Options = {}) {
  pageOf = page;
  const state = options.state ?? chatState([]);
  await mockTauri(
    page,
    {
      gui_theme: options.theme ?? "dark",
      chat_snapshot: { state, logs: options.logs ?? [] },
      chat_providers: options.providers ?? [READY_CLAUDE, SIGNED_OUT_CODEX],
      chat_set_provider: state,
      chat_enqueue: 1,
      chat_edit: state,
      chat_remove: state,
      chat_move: state,
      chat_stop: state,
      chat_resume: state,
      chat_clear: chatState([], { generation: state.generation + 1 }),
      ...options.extra,
    },
    "chat",
  );
  await page.goto("/");
  // The status shows once the first snapshot is in, which is after the listener is registered.
  await expect(statusLine(page)).not.toHaveText("Loading…");
}

const statusLine = (page: Page) => page.locator("main > header").getByRole("status");
const chatCalls = async (page: Page) => (await calls(page)).filter((c) => c.cmd.startsWith("chat_"));
const input = (page: Page) => page.getByRole("textbox", { name: "Message" });
const thread = (page: Page) => page.getByRole("log", { name: "Conversation" });

/** A running task 1 with the events it reported, then the queue behind it. */
const RUNNING = chatState([task(1, "running", { prompt: "Make a lo-fi beat" }), task(2), task(3), task(4)]);

// -- the window ------------------------------------------------------------------------------

test("the window has a title, landmarks and a status line, and follows the contract's startup", async ({ page }) => {
  await openChat(page);
  await expect(page).toHaveTitle("AI Chat (Beta)");
  await expect(page.getByRole("main")).toBeVisible();
  await expect(page.getByRole("heading", { level: 1, name: "AI Chat" })).toBeVisible();
  await expect(statusLine(page)).toHaveText("Ready");
  await expect(thread(page)).toBeVisible();
  // The listener is registered before the snapshot is read, so no change is missed in between.
  const all = await page.evaluate(() => window.__sonaraMock.calls.map((c) => `${c.cmd}:${(c.args as { event?: string }).event ?? ""}`));
  const listen = all.indexOf("plugin:event|listen:chat-event");
  expect(listen).toBeGreaterThanOrEqual(0);
  expect(listen).toBeLessThan(all.findIndex((c) => c.startsWith("chat_snapshot")));
  expect(all.some((c) => c.startsWith("chat_providers"))).toBe(true);
  // Nothing is sent before the user acts.
  expect((await chatCalls(page)).map((c) => c.cmd).sort()).toEqual(["chat_providers", "chat_snapshot"]);
});

test("a snapshot rebuilds a reloaded window: its tasks, steps and results", async ({ page }) => {
  const state = chatState([
    task(1, "done", { prompt: "First", result: { text: "All set.", duration_ms: 3000, num_turns: 2, cost_usd: 0.01, input_tokens: 1, output_tokens: 2, session_id: "s1", denied_tools: [] } }),
    task(2, "running", { prompt: "Second" }),
  ]);
  await openChat(page, {
    state,
    logs: [
      { task_id: 1, events: [{ kind: "text", text: "All " }, { kind: "text", text: "set." }, { kind: "exited", code: 0, stopped: false }] },
      { task_id: 2, events: [{ kind: "tool_use", id: "a", tool: "set_tempo", summary: '{"bpm":90}', nested: false }] },
    ],
  });
  await expect(thread(page).getByText("First", { exact: true })).toBeVisible();
  await expect(thread(page).getByText("All set.", { exact: true })).toBeVisible();
  await expect(thread(page).locator('[data-task="2"]').getByText("set_tempo")).toBeVisible();
  await expect(statusLine(page)).toHaveText("Working");
});

// -- the providers -------------------------------------------------------------------------------

test("each provider shows its availability, and one that is not ready cannot be chosen", async ({ page }) => {
  const providers: ProviderStatus[] = [
    READY_CLAUDE,
    { ...SIGNED_OUT_CODEX, availability: "too_old", version: "0.100.0", hint: "Update Codex CLI." },
  ];
  await openChat(page, { providers });
  const group = page.getByRole("group", { name: "Provider" });
  const claude = group.getByRole("radio", { name: /Claude Code/ });
  const codex = group.getByRole("radio", { name: /Codex CLI/ });
  await expect(claude).toBeChecked();
  await expect(claude).toBeEnabled();
  await expect(group.getByText("2.1.312")).toBeVisible();
  await expect(group.getByText("Ready")).toBeVisible();
  await expect(group.getByText("Signed in.")).toBeVisible();
  await expect(codex).toBeDisabled();
  await expect(group.getByText("Too old (needs 0.160.0 or newer)")).toBeVisible();
  await expect(group.getByText("Update Codex CLI.")).toBeVisible();
  // The hint is the radio's description.
  await expect(codex).toHaveAccessibleDescription("Update Codex CLI.");
  expect((await chatCalls(page)).some((c) => c.cmd === "chat_set_provider")).toBe(false);
});

test("every availability state has its own words", async ({ page }) => {
  const providers: ProviderStatus[] = [
    { ...READY_CLAUDE, availability: "not_found", version: null, hint: "Install Claude Code." },
    { ...SIGNED_OUT_CODEX },
  ];
  await openChat(page, { providers });
  const group = page.getByRole("group", { name: "Provider" });
  await expect(group.getByText("Not installed")).toBeVisible();
  await expect(group.getByText("Not signed in")).toBeVisible();
  await expect(group.getByRole("radio", { name: /Claude Code/ })).toBeDisabled();
  await expect(group.getByRole("radio", { name: /Codex CLI/ })).toBeDisabled();
  // Neither is ready: nothing can be sent, and the box says why.
  await expect(input(page)).toBeDisabled();
  await expect(page.getByText(/is not available: choose a provider that is ready/)).toBeVisible();
  await expect(page.getByRole("button", { name: "Send" })).toBeDisabled();
});

test("a ready provider is chosen with the command, and the thread's event shows the choice", async ({ page }) => {
  await openChat(page, { providers: [READY_CLAUDE, { ...SIGNED_OUT_CODEX, availability: "ready", hint: "Signed in." }] });
  const group = page.getByRole("group", { name: "Provider" });
  await group.getByRole("radio", { name: /Codex CLI/ }).check();
  expect((await chatCalls(page)).filter((c) => c.cmd === "chat_set_provider").map((c) => c.args)).toEqual([{ provider: "codex" }]);
  await changed(chatState([], { provider: "codex" }));
  await expect(group.getByRole("radio", { name: /Codex CLI/ })).toBeChecked();
  await expect(group.getByRole("radio", { name: /Claude Code/ })).not.toBeChecked();
});

test("a refused choice goes back to the thread's provider and says why", async ({ page }) => {
  await openChat(page, {
    providers: [READY_CLAUDE, { ...SIGNED_OUT_CODEX, availability: "ready" }],
    extra: { chat_set_provider: { reject: { code: "PROVIDER_LOCKED", message: "The provider cannot change after the first message." } } },
  });
  const group = page.getByRole("group", { name: "Provider" });
  await group.getByRole("radio", { name: /Codex CLI/ }).click();
  await expect(page.getByRole("alert")).toContainText("The provider cannot change after the first message.");
  await expect(group.getByRole("radio", { name: /Claude Code/ })).toBeChecked();
  await expect(group.getByRole("radio", { name: /Codex CLI/ })).not.toBeChecked();
});

test("once the thread has a task the provider is locked", async ({ page }) => {
  await openChat(page, { state: chatState([task(1, "done")], { provider_locked: true }), providers: [READY_CLAUDE, { ...SIGNED_OUT_CODEX, availability: "ready" }] });
  const group = page.getByRole("group", { name: "Provider" });
  await expect(group.getByRole("radio", { name: /Claude Code/ })).toBeDisabled();
  await expect(group.getByRole("radio", { name: /Codex CLI/ })).toBeDisabled();
  await expect(group.getByRole("radio", { name: /Claude Code/ })).toBeChecked();
  await expect(group.getByText(/Clear the history to choose another/)).toBeVisible();
});

test("Check again detects the providers again and shows what changed", async ({ page }) => {
  await openChat(page);
  const group = page.getByRole("group", { name: "Provider" });
  await expect(group.getByRole("radio", { name: /Codex CLI/ })).toBeDisabled();
  await page.evaluate((codex) => {
    window.__sonaraMock.answers.chat_providers = codex;
  }, [READY_CLAUDE, { ...SIGNED_OUT_CODEX, availability: "ready", hint: "Signed in." }]);
  await group.getByRole("button", { name: "Check again" }).click();
  await expect.poll(async () => (await chatCalls(page)).filter((c) => c.cmd === "chat_providers").length).toBe(2);
  await expect(group.getByRole("radio", { name: /Codex CLI/ })).toBeEnabled();
});

// -- sending -----------------------------------------------------------------------------------

test("Enter queues the message and clears the box; Shift+Enter adds a line", async ({ page }) => {
  await openChat(page);
  const box = input(page);
  await expect(page.getByRole("button", { name: "Send" })).toBeDisabled();
  await box.fill("A chill lo-fi beat");
  await box.press("Shift+Enter");
  await box.pressSequentially("at 80 BPM");
  expect((await chatCalls(page)).some((c) => c.cmd === "chat_enqueue")).toBe(false);
  await expect(box).toHaveValue("A chill lo-fi beat\nat 80 BPM");
  await box.press("Enter");
  await expect.poll(async () => (await chatCalls(page)).filter((c) => c.cmd === "chat_enqueue").map((c) => c.args)).toEqual([
    { prompt: "A chill lo-fi beat\nat 80 BPM" },
  ]);
  await expect(box).toHaveValue("");
  // The backend's event shows the task: nothing is predicted by the window.
  await expect(thread(page).getByText("A chill lo-fi beat")).toHaveCount(0);
  await changed(chatState([task(1, "running", { prompt: "A chill lo-fi beat\nat 80 BPM" })]));
  await expect(thread(page).locator('[data-task="1"]')).toContainText("at 80 BPM");
  await expect(statusLine(page)).toHaveText("Working");
});

test("the Send button sends, and sending works while a task runs", async ({ page }) => {
  await openChat(page, { state: RUNNING });
  await expect(input(page)).toBeEnabled();
  await input(page).fill("  then add a pad  ");
  await page.getByRole("button", { name: "Send" }).click();
  await expect.poll(async () => (await chatCalls(page)).filter((c) => c.cmd === "chat_enqueue").map((c) => c.args)).toEqual([
    { prompt: "then add a pad" },
  ]);
});

test("an empty or blank message cannot be sent", async ({ page }) => {
  await openChat(page);
  await input(page).fill("   \n ");
  await expect(page.getByRole("button", { name: "Send" })).toBeDisabled();
  await input(page).press("Enter");
  expect((await chatCalls(page)).some((c) => c.cmd === "chat_enqueue")).toBe(false);
});

test("the limit is shown, and a message over it cannot be sent", async ({ page }) => {
  await openChat(page);
  await expect(page.getByText(`0 / ${MAX_PROMPT_CHARS}`)).toBeVisible();
  await input(page).fill("x".repeat(MAX_PROMPT_CHARS));
  await expect(page.getByText(`${MAX_PROMPT_CHARS} / ${MAX_PROMPT_CHARS}`)).toBeVisible();
  await expect(page.getByRole("button", { name: "Send" })).toBeEnabled();
  await input(page).fill("x".repeat(MAX_PROMPT_CHARS + 1));
  await expect(page.getByText(`${MAX_PROMPT_CHARS + 1} / ${MAX_PROMPT_CHARS}: too long`)).toBeVisible();
  await expect(page.getByRole("button", { name: "Send" })).toBeDisabled();
  await input(page).press("Enter");
  expect((await chatCalls(page)).some((c) => c.cmd === "chat_enqueue")).toBe(false);
});

test("a message the backend refuses is explained and kept", async ({ page }) => {
  await openChat(page, { extra: { chat_enqueue: { reject: { code: "QUEUE_FULL", message: "The queue is full (50 tasks)." } } } });
  await input(page).fill("one more");
  await input(page).press("Enter");
  await expect(page.getByRole("alert")).toContainText("The queue is full (50 tasks).");
  await expect(input(page)).toHaveValue("one more");
  // The next successful command clears the notice.
  await page.evaluate(() => {
    window.__sonaraMock.answers.chat_enqueue = 5;
  });
  await input(page).press("Enter");
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(input(page)).toHaveValue("");
});

// -- the thread --------------------------------------------------------------------------------

test("a run's text and tool steps stream in, and the result ends the task", async ({ page }) => {
  await openChat(page, { state: chatState([task(1, "running", { prompt: "Make a beat" })]) });
  const log = thread(page).locator('[data-task="1"]');
  await expect(log.getByText("Starting…")).toBeVisible();
  const run = (event: unknown, generation = 1) => emit(page, "chat-event", { kind: "run", task_id: 1, generation, event });
  await run({ kind: "init", session_id: "s1", model: "m", tool_count: 3, mcp_servers: [{ name: "sonara", status: "connected" }], mcp_errors: [] });
  await run({ kind: "text", text: "Setting " });
  await run({ kind: "text", text: "the tempo." });
  await run({ kind: "tool_use", id: "t1", tool: "set_tempo", summary: '{"bpm":80}', nested: false });
  await expect(log.getByText("Setting the tempo.")).toBeVisible();
  const step = log.locator("details.step").first();
  await expect(step).toContainText("set_tempo");
  await expect(step).toHaveAttribute("data-status", "running");
  await expect(step.locator("summary")).toContainText(", running");
  // A step is collapsible: its output shows when it is opened.
  await expect(step.locator("pre")).toBeHidden();
  await run({ kind: "tool_result", id: "t1", ok: true, preview: '{"ok":true}' });
  await expect(step).toHaveAttribute("data-status", "ok");
  await step.locator("summary").click();
  await expect(step.locator("pre")).toHaveText('{"ok":true}');
  await step.locator("summary").click();
  await expect(step.locator("pre")).toBeHidden();
  await run({ kind: "tool_use", id: "t2", tool: "create_track", summary: "{}", nested: false });
  await run({ kind: "tool_result", id: "t2", ok: false, preview: '{"error":{"code":"INVALID_ARGUMENT"}}' });
  await expect(log.locator("details.step").nth(1)).toHaveAttribute("data-status", "failed");
  await expect(log.locator("details.step").nth(1).locator("summary")).toContainText(", failed");
  await run({ kind: "retry", attempt: 1, max_retries: 3, delay_ms: 2000, error: "overloaded" });
  await expect(log.getByText(/attempt 1 of 3/)).toBeVisible();

  await changed(
    chatState([
      task(1, "done", {
        prompt: "Make a beat",
        result: { text: "A slow loop.", duration_ms: 65_000, num_turns: 7, cost_usd: 0.12, input_tokens: 10, output_tokens: 20, session_id: "s1", denied_tools: ["save_project"] },
      }),
    ]),
  );
  await expect(log.getByRole("heading", { name: "Finished" })).toBeVisible();
  for (const text of ["1 min 05 s", "$0.1200", "10 in, 20 out", "s1", "save_project", "2, 1 failed"]) {
    await expect(log.getByText(text, { exact: true })).toBeVisible();
  }
  // The text was streamed, so the final text is not repeated under it.
  await expect(log.getByText("A slow loop.")).toHaveCount(0);
  await expect(statusLine(page)).toHaveText("Idle");
});

test("a task that reported no text shows the result's text", async ({ page }) => {
  await openChat(page, {
    state: chatState([task(1, "done", { result: { text: "Just the answer.", duration_ms: null, num_turns: null, cost_usd: null, input_tokens: null, output_tokens: null, session_id: null, denied_tools: [] } })]),
  });
  await expect(thread(page).getByText("Just the answer.", { exact: true })).toBeVisible();
  await expect(thread(page).getByText("—", { exact: true })).toBeVisible();
});

test("a failed or stopped task says so, with the reason", async ({ page }) => {
  await openChat(page, {
    state: chatState([task(1, "failed", { error: "Not logged in" }), task(2, "cancelled"), task(3)], { mode: "paused" }),
  });
  await expect(thread(page).locator('[data-task="1"]').getByRole("heading")).toHaveText("Did not finish: Not logged in");
  await expect(thread(page).locator('[data-task="2"]').getByRole("heading")).toHaveText("Stopped by you");
  // A queued task is in the queue, not in the thread yet.
  await expect(thread(page).locator('[data-task="3"]')).toHaveCount(0);
});

test("missing Sonara tools are reported in the task's thread", async ({ page }) => {
  await openChat(page, {
    state: chatState([task(1, "running")]),
    logs: [{ task_id: 1, events: [{ kind: "init", session_id: null, model: null, tool_count: 0, mcp_servers: [{ name: "sonara", status: "failed" }], mcp_errors: ["spawn failed"] }] }],
  });
  await expect(thread(page).getByText(/The Sonara tools are not connected\. spawn failed/)).toBeVisible();
});

// -- the queue ---------------------------------------------------------------------------------

test("the running task has Stop, which calls chat_stop", async ({ page }) => {
  await openChat(page, { state: RUNNING });
  const queue = page.getByRole("region", { name: "Queue" });
  await expect(queue.getByText("Make a lo-fi beat")).toBeVisible();
  await expect(statusLine(page)).toHaveText("Working, 3 queued");
  await queue.getByRole("button", { name: "Stop" }).click();
  expect((await chatCalls(page)).filter((c) => c.cmd === "chat_stop")).toHaveLength(1);
});

test("a stop pauses the queue: the banner says so and Run queue resumes it", async ({ page }) => {
  await openChat(page, { state: RUNNING });
  await changed(chatState([task(1, "cancelled"), task(2), task(3), task(4)], { mode: "paused" }));
  const queue = page.getByRole("region", { name: "Queue" });
  await expect(queue.getByText("The queue is paused. 3 tasks are waiting.")).toBeVisible();
  await expect(statusLine(page)).toHaveText("Queue paused, 3 queued");
  await expect(queue.getByRole("button", { name: "Stop" })).toHaveCount(0);
  await queue.getByRole("button", { name: "Run queue" }).click();
  expect((await chatCalls(page)).filter((c) => c.cmd === "chat_resume")).toHaveLength(1);
  // The backend started the next task: the banner goes.
  await changed(chatState([task(1, "cancelled"), task(2, "running"), task(3), task(4)]));
  await expect(queue.getByText(/The queue is paused/)).toHaveCount(0);
  await expect(queue.getByRole("button", { name: "Stop" })).toBeVisible();
});

test("a paused queue with nothing waiting shows no banner, and no queue at all", async ({ page }) => {
  await openChat(page, { state: chatState([task(1, "failed", { error: "x" })], { mode: "paused" }) });
  await expect(page.getByRole("region", { name: "Queue" })).toHaveCount(0);
  await expect(statusLine(page)).toHaveText("Idle");
});

test("the queued tasks are listed in order, each named by its place", async ({ page }) => {
  await openChat(page, { state: RUNNING });
  const items = page.getByRole("list", { name: "Queued tasks" }).getByRole("listitem");
  await expect(items).toHaveCount(3);
  await expect(items.nth(0)).toHaveAccessibleName("1 of 3: prompt 2");
  await expect(items.nth(2)).toHaveAccessibleName("3 of 3: prompt 4");
});

test("Remove calls chat_remove with the task's id", async ({ page }) => {
  await openChat(page, { state: RUNNING });
  await page.locator('[data-task="3"]').getByRole("button", { name: "Remove" }).click();
  expect((await chatCalls(page)).filter((c) => c.cmd === "chat_remove").map((c) => c.args)).toEqual([{ id: 3 }]);
  await changed(chatState([task(1, "running"), task(2), task(4)]));
  await expect(page.locator('[data-task="3"]')).toHaveCount(0);
});

test("Edit changes a queued task in place: Enter saves, Escape cancels", async ({ page }) => {
  await openChat(page, { state: RUNNING });
  const item = page.locator('[data-task="3"]');
  await item.getByRole("button", { name: "Edit" }).click();
  const box = item.getByRole("textbox", { name: "Edit the queued task" });
  await expect(box).toBeFocused();
  await expect(box).toHaveValue("prompt 3");
  // Escape leaves it as it was, and the focus returns to the task.
  await box.fill("changed my mind");
  await box.press("Escape");
  await expect(box).toHaveCount(0);
  await expect(item).toBeFocused();
  expect((await chatCalls(page)).some((c) => c.cmd === "chat_edit")).toBe(false);
  // Edit again and save.
  await item.getByRole("button", { name: "Edit" }).click();
  await box.fill("  a longer pad  ");
  await box.press("Enter");
  await expect.poll(async () => (await chatCalls(page)).filter((c) => c.cmd === "chat_edit").map((c) => c.args)).toEqual([
    { id: 3, prompt: "a longer pad" },
  ]);
  await expect(box).toHaveCount(0);
  await changed(chatState([task(1, "running"), task(2), task(3, "queued", { prompt: "a longer pad" }), task(4)]));
  await expect(item).toContainText("a longer pad");
});

test("Edit has Save and Cancel, a counter, and refuses an empty prompt", async ({ page }) => {
  await openChat(page, { state: RUNNING });
  const item = page.locator('[data-task="2"]');
  await item.getByRole("button", { name: "Edit" }).click();
  await expect(item.getByText(`8 / ${MAX_PROMPT_CHARS}`)).toBeVisible();
  await item.getByRole("textbox").fill("  ");
  await expect(item.getByRole("button", { name: "Save" })).toBeDisabled();
  await item.getByRole("textbox").press("Enter");
  expect((await chatCalls(page)).some((c) => c.cmd === "chat_edit")).toBe(false);
  await item.getByRole("button", { name: "Cancel" }).click();
  await expect(item.getByRole("textbox")).toHaveCount(0);
});

test("an edit the backend refuses keeps the editor open and says why", async ({ page }) => {
  await openChat(page, {
    state: RUNNING,
    extra: { chat_edit: { reject: { code: "TASK_NOT_QUEUED", message: "Task 3 already started." } } },
  });
  const item = page.locator('[data-task="3"]');
  await item.getByRole("button", { name: "Edit" }).click();
  await item.getByRole("textbox").fill("late edit");
  await item.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("alert")).toContainText("Task 3 already started.");
  await expect(item.getByRole("textbox")).toHaveValue("late edit");
  // The task started meanwhile (the backend's event): the editor closes with it.
  await changed(chatState([task(1, "done"), task(2, "done"), task(3, "running"), task(4)]));
  await expect(page.locator('[data-task="3"]').getByRole("textbox")).toHaveCount(0);
});

test("Move up and Move down use the index among the queued tasks, and stop at the ends", async ({ page }) => {
  await openChat(page, { state: RUNNING });
  const move = (id: number, direction: "up" | "down") => page.locator(`[data-task="${id}"]`).getByRole("button", { name: `Move ${direction}` });
  await expect(move(2, "up")).toBeDisabled();
  await expect(move(2, "down")).toBeEnabled();
  await expect(move(4, "down")).toBeDisabled();
  await expect(move(4, "up")).toBeEnabled();
  await move(4, "up").click();
  await move(2, "down").click();
  await move(3, "up").click();
  expect((await chatCalls(page)).filter((c) => c.cmd === "chat_move").map((c) => c.args)).toEqual([
    { id: 4, toIndex: 1 },
    { id: 2, toIndex: 1 },
    { id: 3, toIndex: 0 },
  ]);
});

test("Alt+ArrowUp and Alt+ArrowDown move the focused task, which keeps the focus", async ({ page }) => {
  await openChat(page, { state: RUNNING });
  const second = page.locator('[data-task="3"]');
  await second.focus();
  await page.keyboard.press("Alt+ArrowDown");
  await expect.poll(async () => (await chatCalls(page)).filter((c) => c.cmd === "chat_move").map((c) => c.args)).toEqual([{ id: 3, toIndex: 2 }]);
  // The backend's event reorders the list; the moved task still has the focus.
  await changed(chatState([task(1, "running"), task(2), task(4), task(3)]));
  await expect(page.locator('[data-task="3"]')).toBeFocused();
  await expect(page.getByRole("list", { name: "Queued tasks" }).getByRole("listitem").nth(2)).toHaveAttribute("data-task", "3");
  await page.keyboard.press("Alt+ArrowUp");
  await expect.poll(async () => (await chatCalls(page)).filter((c) => c.cmd === "chat_move").map((c) => c.args)).toEqual([
    { id: 3, toIndex: 2 },
    { id: 3, toIndex: 1 },
  ]);
  // At the ends nothing is sent.
  await changed(chatState([task(1, "running"), task(3), task(2), task(4)]));
  await expect(page.locator('[data-task="3"]')).toBeFocused();
  await page.keyboard.press("Alt+ArrowUp");
  await page.locator('[data-task="4"]').focus();
  await page.keyboard.press("Alt+ArrowDown");
  expect(await chatCalls(page).then((all) => all.filter((c) => c.cmd === "chat_move"))).toHaveLength(2);
  // Plain arrows do not move anything.
  await page.keyboard.press("ArrowUp");
  expect(await chatCalls(page).then((all) => all.filter((c) => c.cmd === "chat_move"))).toHaveLength(2);
});

test("Alt+Arrow on a task's move button keeps the focus on that button", async ({ page }) => {
  await openChat(page, { state: RUNNING });
  const down = page.locator('[data-task="2"]').getByRole("button", { name: "Move down" });
  await down.focus();
  await page.keyboard.press("Alt+ArrowDown");
  await expect.poll(async () => (await chatCalls(page)).filter((c) => c.cmd === "chat_move").map((c) => c.args)).toEqual([{ id: 2, toIndex: 1 }]);
  await changed(chatState([task(1, "running"), task(3), task(2), task(4)]));
  await expect(page.locator('[data-task="2"]').getByRole("button", { name: "Move down" })).toBeFocused();
});

test("a refused move reports the error", async ({ page }) => {
  await openChat(page, { state: RUNNING, extra: { chat_move: { reject: { code: "CONFLICT", message: "The queue changed." } } } });
  await page.locator('[data-task="3"]').getByRole("button", { name: "Move down" }).click();
  await expect(page.getByRole("alert")).toContainText("The queue changed.");
  await page.getByRole("button", { name: "Dismiss the error" }).click();
  await expect(page.getByRole("alert")).toHaveCount(0);
});

// -- Clear history -------------------------------------------------------------------------------

test("Clear history is disabled for an empty thread", async ({ page }) => {
  await openChat(page);
  await expect(page.getByRole("button", { name: "Clear history" })).toBeDisabled();
});

test("Clear history on an idle thread clears at once, without asking", async ({ page }) => {
  await openChat(page, { state: chatState([task(1, "done"), task(2, "failed", { error: "x" })]) });
  await page.getByRole("button", { name: "Clear history" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect.poll(async () => (await chatCalls(page)).filter((c) => c.cmd === "chat_clear")).toHaveLength(1);
  await changed(chatState([], { generation: 2 }));
  await expect(thread(page).locator("[data-task]")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Clear history" })).toBeDisabled();
  // The focus went to the message box, not to the button that is now disabled.
  await expect(input(page)).toBeFocused();
});

test("Clear history while a task runs asks first, and Cancel keeps everything", async ({ page }) => {
  await openChat(page, { state: RUNNING });
  const clear = page.getByRole("button", { name: "Clear history" });
  // WebKit does not focus a button on a click, so the focus to restore is set by the keyboard.
  await clear.focus();
  await page.keyboard.press("Enter");
  const dialog = page.getByRole("dialog", { name: "Clear the history?" });
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText("stop the running task and drop 3 queued tasks");
  await expect(dialog.getByRole("button", { name: "Cancel" })).toBeFocused();
  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(dialog).toHaveCount(0);
  await expect(clear).toBeFocused();
  expect((await chatCalls(page)).some((c) => c.cmd === "chat_clear")).toBe(false);
  await expect(thread(page).locator('[data-task="1"]')).toBeVisible();
  // Escape cancels too.
  await page.keyboard.press("Enter");
  await expect(dialog).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  expect((await chatCalls(page)).some((c) => c.cmd === "chat_clear")).toBe(false);
});

test("Tab stays inside the confirmation", async ({ page }) => {
  await openChat(page, { state: RUNNING });
  await page.getByRole("button", { name: "Clear history" }).click();
  const dialog = page.getByRole("dialog");
  const cancel = dialog.getByRole("button", { name: "Cancel" });
  const confirm = dialog.getByRole("button", { name: "Clear history" });
  await expect(cancel).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(confirm).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(cancel).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(confirm).toBeFocused();
});

test("confirming calls chat_clear, and the new generation empties the thread", async ({ page }) => {
  await openChat(page, { state: RUNNING });
  await page.getByRole("button", { name: "Clear history" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Clear history" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect.poll(async () => (await chatCalls(page)).filter((c) => c.cmd === "chat_clear")).toHaveLength(1);
  await changed(chatState([], { generation: 2 }));
  await expect(thread(page).locator("[data-task]")).toHaveCount(0);
  await expect(page.getByRole("region", { name: "Queue" })).toHaveCount(0);
  await expect(statusLine(page)).toHaveText("Ready");
});

test("a paused queue with waiting tasks asks before clearing, naming only what it drops", async ({ page }) => {
  await openChat(page, { state: chatState([task(1, "cancelled"), task(2)], { mode: "paused" }) });
  await page.getByRole("button", { name: "Clear history" }).click();
  const dialog = page.getByRole("dialog", { name: "Clear the history?" });
  await expect(dialog).toContainText("drop 1 queued task, and erase");
  await expect(dialog).not.toContainText("stop the running task");
});

// -- a new generation --------------------------------------------------------------------------

test("a project replaced under the chat empties the view and ignores the old run's events", async ({ page }) => {
  await openChat(page, { state: RUNNING });
  const run = (generation: number, text: string, task_id = 1) => emit(page, "chat-event", { kind: "run", task_id, generation, event: { kind: "text", text } });
  await run(1, "old words");
  await expect(thread(page).getByText("old words")).toBeVisible();
  // The backend wiped the thread (a new project was opened).
  await changed(chatState([], { generation: 2 }));
  await expect(thread(page).locator("[data-task]")).toHaveCount(0);
  await expect(page.getByRole("region", { name: "Queue" })).toHaveCount(0);
  // The old run reports once more before it ends: it must not come back.
  await run(1, "late words");
  await expect(thread(page).getByText("late words")).toHaveCount(0);
  await expect(thread(page).getByText("old words")).toHaveCount(0);
  // A task of the new thread (its id counter may repeat) starts clean.
  await changed(chatState([task(1, "running", { prompt: "new thread" })], { generation: 2 }));
  await run(2, "new words");
  await expect(thread(page).locator('[data-task="1"]')).toContainText("new words");
  await expect(thread(page).locator('[data-task="1"]')).not.toContainText("old words");
  await run(1, "stale again");
  await expect(thread(page).getByText("stale again")).toHaveCount(0);
});

// -- the Beta badge and accessibility ------------------------------------------------------------

test("the Beta badge says Beta in words, and explains itself on hover, focus and click", async ({ page }) => {
  await openChat(page);
  const badge = page.getByRole("button", { name: "Beta, experimental feature" });
  await expect(badge).toBeVisible();
  await expect(badge).toHaveText("Beta");
  await expect(badge).toHaveAccessibleDescription(BETA_EXPLANATION);
  const tip = page.getByRole("tooltip");
  await expect(tip).toBeHidden();
  // Hover (and leaving hides it again).
  await badge.hover();
  await expect(tip).toBeVisible();
  await expect(tip).toHaveText(BETA_EXPLANATION);
  await page.mouse.move(400, 400);
  await expect(tip).toBeHidden();
  // Keyboard focus; Escape dismisses it without moving the focus.
  await badge.focus();
  await expect(tip).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(tip).toBeHidden();
  await expect(badge).toBeFocused();
  // Focus leaving hides it.
  await badge.blur();
  await expect(tip).toBeHidden();
  // A click opens it (and keeps it while the pointer is away); another click closes it.
  await badge.click();
  await page.mouse.move(400, 400);
  await expect(tip).toBeVisible();
  await badge.click();
  await expect(tip).toBeHidden();
  // Enter on the focused badge opens it too.
  await badge.press("Enter");
  await expect(tip).toBeVisible();
});

test("the popover can be reached with the pointer without closing", async ({ page }) => {
  await openChat(page);
  const badge = page.getByRole("button", { name: "Beta, experimental feature" });
  await badge.hover();
  const tip = page.getByRole("tooltip");
  await tip.hover();
  await expect(tip).toBeVisible();
});

test("the controls have names and roles a screen reader reads", async ({ page }) => {
  await openChat(page, { state: RUNNING });
  await expect(page.getByRole("button", { name: "Clear history" })).toBeVisible();
  await expect(page.getByRole("group", { name: "Provider" })).toBeVisible();
  await expect(page.getByRole("radio")).toHaveCount(2);
  await expect(page.getByRole("log", { name: "Conversation" })).toBeVisible();
  await expect(page.getByRole("region", { name: "Queue" })).toBeVisible();
  await expect(page.getByRole("textbox", { name: "Message" })).toBeVisible();
  // Icon-only buttons are named; the status is a polite live region.
  await expect(page.getByRole("button", { name: "Move up" })).toHaveCount(3);
  await expect(page.getByRole("button", { name: "Move down" })).toHaveCount(3);
  const live = statusLine(page);
  await expect(live).toHaveAttribute("aria-live", "polite");
  // The conversation does not read out every streamed word (the status line speaks for it).
  await expect(thread(page)).toHaveAttribute("aria-live", "off");
  // An error is announced.
  await page.evaluate(() => {
    window.__sonaraMock.answers.chat_stop = { reject: { code: "CONFLICT", message: "Nothing to stop." } };
  });
  await page.getByRole("button", { name: "Stop" }).click();
  await expect(page.getByRole("alert")).toHaveText(/Nothing to stop\./);
});

test("a focused task answers to Enter (edit) and Delete (remove), for a WebKit Tab that skips buttons", async ({ page }) => {
  await openChat(page, { state: RUNNING });
  const item = page.locator('[data-task="3"]');
  await item.focus();
  await page.keyboard.press("Enter");
  const box = item.getByRole("textbox", { name: "Edit the queued task" });
  await expect(box).toBeFocused();
  // Enter in the editor saves; it does not open a second editor or remove anything.
  await box.press("Enter");
  await expect.poll(async () => (await chatCalls(page)).filter((c) => c.cmd === "chat_edit").map((c) => c.args)).toEqual([{ id: 3, prompt: "prompt 3" }]);
  await expect(item).toBeFocused();
  await page.keyboard.press("Delete");
  await expect.poll(async () => (await chatCalls(page)).filter((c) => c.cmd === "chat_remove").map((c) => c.args)).toEqual([{ id: 3 }]);
  // The task beside the removed one gets the focus once the backend's event shows the new list.
  await changed(chatState([task(1, "running"), task(2), task(4)]));
  await expect(page.locator('[data-task="4"]')).toBeFocused();
});

// -- colours, in both themes -------------------------------------------------------------------

/** Every visible piece of text with its colour and the first opaque background behind it. */
async function textColours(page: Page) {
  return page.evaluate(() => {
    const found: { text: string; fg: string; bg: string }[] = [];
    const background = (start: Element | null) => {
      for (let el = start; el; el = el.parentElement) {
        const colour = getComputedStyle(el).backgroundColor;
        if (colour !== "rgba(0, 0, 0, 0)" && colour !== "transparent") return colour;
      }
      return "rgb(255, 255, 255)";
    };
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const text = node.textContent?.trim();
      const el = node.parentElement;
      if (!text || !el || el.getClientRects().length === 0) continue;
      const rect = el.getBoundingClientRect();
      if (rect.width <= 1 || rect.height <= 1) continue; // visually hidden text
      if (el.closest(":disabled, [hidden], script, style")) continue; // a disabled control is exempt
      found.push({ text, fg: getComputedStyle(el).color, bg: background(el) });
    }
    for (const field of document.querySelectorAll<HTMLTextAreaElement>("textarea[placeholder]")) {
      if (field.disabled || field.value !== "") continue;
      found.push({ text: `placeholder of ${field.id || "a field"}`, fg: getComputedStyle(field, "::placeholder").color, bg: background(field) });
    }
    return found;
  });
}

/** A thread with every kind of text: bubbles, steps, results, failures, the queue, a banner, an error. */
const BUSY = chatState(
  [
    task(1, "done", { result: { text: "Done.", duration_ms: 5000, num_turns: 2, cost_usd: 0.5, input_tokens: 1, output_tokens: 2, session_id: "abc", denied_tools: ["save_project"] } }),
    task(2, "failed", { error: "Not logged in" }),
    task(3, "cancelled"),
    task(4),
    task(5),
  ],
  { mode: "paused" },
);

for (const theme of ["dark", "light"] as const) {
  test(`all text of the window passes 4.5:1 in the ${theme} theme`, async ({ page }) => {
    await openChat(page, {
      theme,
      state: BUSY,
      logs: [
        {
          task_id: 1,
          events: [
            { kind: "text", text: "Adding a bass." },
            { kind: "tool_use", id: "a", tool: "create_track", summary: "{}", nested: false },
            { kind: "tool_result", id: "a", ok: false, preview: "NOT_FOUND" },
            { kind: "retry", attempt: 1, max_retries: 2, delay_ms: 1000, error: "overloaded" },
          ],
        },
      ],
      providers: [READY_CLAUDE, { ...SIGNED_OUT_CODEX, availability: "too_old" }],
      extra: { chat_edit: { reject: { code: "TASK_NOT_QUEUED", message: "Task 4 already started." } } },
    });
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    // An error, the Beta explanation and an open editor are text too.
    await page.locator('[data-task="4"]').getByRole("button", { name: "Edit" }).click();
    await page.locator('[data-task="4"]').getByRole("button", { name: "Save" }).click();
    await expect(page.getByRole("alert")).toBeVisible();
    await page.getByRole("button", { name: "Beta, experimental feature" }).hover();
    await expect(page.getByRole("tooltip")).toBeVisible();
    const found = await textColours(page);
    expect(found.length).toBeGreaterThan(30);
    const failing = found
      .map((entry) => ({ ...entry, ratio: contrastRatio(entry.fg, entry.bg) }))
      .filter((entry) => entry.ratio < MIN_TEXT_CONTRAST)
      .map((entry) => `${entry.text} ${entry.fg} on ${entry.bg} = ${entry.ratio.toFixed(2)}`);
    expect(failing).toEqual([]);
  });

  test(`the Beta badge draws the palette's text on its panel in the ${theme} theme`, async ({ page }) => {
    await openChat(page, { theme });
    const palette = PALETTES[theme];
    const badge = page.getByRole("button", { name: "Beta, experimental feature" });
    const colours = await badge.evaluate((el) => ({ fg: getComputedStyle(el).color, bg: getComputedStyle(el).backgroundColor }));
    expect(contrastRatio(colours.fg, palette.text)).toBeCloseTo(1, 5);
    expect(contrastRatio(colours.bg, palette.panelAlt)).toBeCloseTo(1, 5);
    expect(contrastRatio(colours.fg, colours.bg)).toBeGreaterThanOrEqual(MIN_TEXT_CONTRAST);
  });
}

test("a theme change reaches the open window", async ({ page }) => {
  await openChat(page, { theme: "dark" });
  const badge = page.getByRole("button", { name: "Beta, experimental feature" });
  const background = () => badge.evaluate((el) => getComputedStyle(el).backgroundColor);
  const dark = await background();
  await emit(page, "theme", "light");
  await expect.poll(background).not.toBe(dark);
  expect(contrastRatio(await background(), PALETTES.light.panelAlt)).toBeCloseTo(1, 5);
});

test("the confirmation's text passes 4.5:1 in both themes", async ({ page }) => {
  await openChat(page, { state: RUNNING, theme: "light" });
  await page.getByRole("button", { name: "Clear history" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  const found = await textColours(page);
  const dialogText = found.filter((entry) => ["Clear the history?", "Cancel"].includes(entry.text) || entry.text.startsWith("This will"));
  expect(dialogText).toHaveLength(3);
  for (const entry of dialogText) expect(contrastRatio(entry.fg, entry.bg), entry.text).toBeGreaterThanOrEqual(MIN_TEXT_CONTRAST);
});
