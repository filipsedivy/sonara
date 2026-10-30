import { expect, test } from "@playwright/test";
import { chatError, type ChatEvent, type ChatState, type ProviderStatus, type TaskView } from "../../src/lib/chat/api";
import {
  MAX_PROMPT_CHARS,
  applyEvent,
  availabilityText,
  canClear,
  canMove,
  clearNeedsConfirm,
  composerEnabled,
  emptyView,
  logOf,
  moveTarget,
  phase,
  promptLength,
  promptProblem,
  providerSelectable,
  queuedIds,
  resync,
  runningTask,
  selectedProvider,
  withProviders,
} from "../../src/lib/chat/model";

// The AI Chat's view of the backend (crates/sonara-app/src/chat): the events, the resync, the
// generation rule and the rules of its buttons. The store (chat.svelte.ts) is covered by the page
// tests, which need the Svelte compiler.

function task(id: number, state: TaskView["state"] = "queued", extra: Partial<TaskView> = {}): TaskView {
  return { id, prompt: `prompt ${id}`, provider: "claude", state, error: null, result: null, ...extra };
}

function chatState(tasks: TaskView[], extra: Partial<ChatState> = {}): ChatState {
  return { generation: 1, mode: "running", provider: "claude", provider_locked: tasks.length > 0, tasks, ...extra };
}

const changed = (state: ChatState): ChatEvent => ({ kind: "changed", state });
const run = (task_id: number, generation: number, event: Extract<ChatEvent, { kind: "run" }>["event"]): ChatEvent => ({
  kind: "run",
  task_id,
  generation,
  event,
});

function provider(id: ProviderStatus["id"], availability: ProviderStatus["availability"]): ProviderStatus {
  return {
    id,
    name: id === "claude" ? "Claude Code" : "Codex CLI",
    availability,
    version: availability === "not_found" ? null : "1.0.0",
    path: null,
    hint: "",
    beta: true,
    min_version: "2.1.0",
  };
}

test("a changed event replaces the state and keeps the logs of the same thread", () => {
  let view = applyEvent(emptyView(), changed(chatState([task(1, "running")])));
  view = applyEvent(view, run(1, 1, { kind: "text", text: "Hello" }));
  view = applyEvent(view, changed(chatState([task(1, "done"), task(2)])));
  expect(view.state?.tasks.map((t) => t.state)).toEqual(["done", "queued"]);
  expect(logOf(view, 1).entries).toEqual([{ type: "text", text: "Hello" }]);
});

test("run events build one log per task, in order, with the composer's reducer", () => {
  let view = applyEvent(emptyView(), changed(chatState([task(1, "running"), task(2)])));
  for (const event of [
    run(1, 1, { kind: "text", text: "Add " }),
    run(1, 1, { kind: "text", text: "a bass." }),
    run(1, 1, { kind: "tool_use", id: "t1", tool: "create_track", summary: "{}", nested: false }),
    run(1, 1, { kind: "tool_result", id: "t1", ok: true, preview: "ok" }),
    run(2, 1, { kind: "text", text: "second" }),
    run(1, 1, { kind: "exited", code: 0, stopped: false }),
  ]) {
    view = applyEvent(view, event);
  }
  expect(logOf(view, 1).entries).toEqual([
    { type: "text", text: "Add a bass." },
    { type: "step", id: "t1", tool: "create_track", summary: "{}", nested: false, status: "ok", preview: "ok" },
  ]);
  expect(logOf(view, 1).phase).toBe("done");
  expect(logOf(view, 2).entries).toEqual([{ type: "text", text: "second" }]);
  // A task that reported nothing has an empty log.
  expect(logOf(view, 99).entries).toEqual([]);
});

test("a run event of another generation is ignored", () => {
  const view = applyEvent(emptyView(), changed(chatState([task(1, "running")], { generation: 4 })));
  const stale = applyEvent(view, run(1, 3, { kind: "text", text: "from the wiped thread" }));
  expect(stale).toBe(view);
  const future = applyEvent(view, run(1, 5, { kind: "text", text: "not known yet" }));
  expect(future).toBe(view);
  const current = applyEvent(view, run(1, 4, { kind: "text", text: "now" }));
  expect(logOf(current, 1).entries).toEqual([{ type: "text", text: "now" }]);
});

test("a run event before the first state is ignored", () => {
  const view = emptyView();
  expect(applyEvent(view, run(1, 1, { kind: "text", text: "early" }))).toBe(view);
});

test("a new generation wipes the logs, and the old thread's late events stay out", () => {
  let view = applyEvent(emptyView(), changed(chatState([task(1, "running")], { generation: 1 })));
  view = applyEvent(view, run(1, 1, { kind: "text", text: "old" }));
  view = applyEvent(view, withNewGeneration());
  expect(view.state?.tasks).toEqual([]);
  expect(view.state?.generation).toBe(2);
  expect(view.logs).toEqual({});
  // The old run reports once more after the wipe (its generation is 1): nothing comes back.
  const late = applyEvent(view, run(1, 1, { kind: "exited", code: 130, stopped: true }));
  expect(late).toBe(view);
  // Task ids are never reused for the old thread: the new thread's task 1 logs from empty.
  view = applyEvent(view, changed(chatState([task(1, "running")], { generation: 2 })));
  view = applyEvent(view, run(1, 2, { kind: "text", text: "new" }));
  expect(logOf(view, 1).entries).toEqual([{ type: "text", text: "new" }]);

  function withNewGeneration(): ChatEvent {
    return changed(chatState([], { generation: 2 }));
  }
});

test("the providers survive a wipe, and the first state does not drop them", () => {
  let view = withProviders(emptyView(), [provider("claude", "ready"), provider("codex", "not_found")]);
  view = applyEvent(view, changed(chatState([task(1)], { generation: 1 })));
  view = applyEvent(view, changed(chatState([], { generation: 2 })));
  expect(view.providers?.map((p) => p.id)).toEqual(["claude", "codex"]);
});

test("a snapshot rebuilds the state and every log from its events", () => {
  const before = applyEvent(emptyView(), changed(chatState([task(7, "running")], { generation: 1 })));
  const view = resync(before, {
    state: chatState([task(1, "done"), task(2, "running")], { generation: 3 }),
    logs: [
      {
        task_id: 1,
        events: [
          { kind: "text", text: "Done " },
          { kind: "text", text: "already." },
          { kind: "exited", code: 0, stopped: false },
        ],
      },
      { task_id: 2, events: [{ kind: "tool_use", id: "a", tool: "set_tempo", summary: "{}", nested: false }] },
    ],
  });
  expect(view.state?.generation).toBe(3);
  expect(Object.keys(view.logs).sort()).toEqual(["1", "2"]);
  expect(logOf(view, 1).entries).toEqual([{ type: "text", text: "Done already." }]);
  expect(logOf(view, 1).phase).toBe("done");
  expect(logOf(view, 2).entries[0]).toMatchObject({ type: "step", status: "running" });
  // The old view's log of task 7 is gone, and run events of the snapshot's generation apply.
  expect(view.logs[7]).toBeUndefined();
  const next = applyEvent(view, run(2, 3, { kind: "tool_result", id: "a", ok: true, preview: "" }));
  expect(logOf(next, 2).entries[0]).toMatchObject({ status: "ok" });
});

test("a snapshot keeps the providers already detected", () => {
  const detected = withProviders(emptyView(), [provider("claude", "ready")]);
  const view = resync(detected, { state: chatState([]), logs: [] });
  expect(view.providers).toHaveLength(1);
});

test("the queue is the queued tasks in order, whatever else is in the thread", () => {
  const state = chatState([task(1, "done"), task(2, "running"), task(3), task(4), task(5)]);
  expect(queuedIds(state)).toEqual([3, 4, 5]);
  expect(runningTask(state)?.id).toBe(2);
  expect(runningTask(chatState([task(1, "done")]))).toBeNull();
});

test("a task moves within the queued tasks only, and not past either end", () => {
  const state = chatState([task(1, "done"), task(2, "running"), task(3), task(4), task(5)]);
  // Indices are among the queued tasks: 3 is 0, 4 is 1, 5 is 2.
  expect(moveTarget(state, 3, "up")).toBeNull();
  expect(moveTarget(state, 3, "down")).toBe(1);
  expect(moveTarget(state, 4, "up")).toBe(0);
  expect(moveTarget(state, 4, "down")).toBe(2);
  expect(moveTarget(state, 5, "down")).toBeNull();
  expect(canMove(state, 5, "up")).toBe(true);
  expect(canMove(state, 3, "up")).toBe(false);
  // A task that is not queued (running, done, unknown) never moves.
  for (const id of [1, 2, 42]) {
    expect(moveTarget(state, id, "up")).toBeNull();
    expect(moveTarget(state, id, "down")).toBeNull();
  }
  // A lone queued task has nowhere to go.
  const lone = chatState([task(9)]);
  expect(canMove(lone, 9, "up") || canMove(lone, 9, "down")).toBe(false);
});

test("the phase is running while a task runs, paused only with tasks waiting, else idle", () => {
  expect(phase(chatState([]))).toBe("idle");
  expect(phase(chatState([task(1, "done")]))).toBe("idle");
  expect(phase(chatState([task(1, "running"), task(2)]))).toBe("running");
  // Paused with a running task cannot happen, but a running task wins.
  expect(phase(chatState([task(1, "running")], { mode: "paused" }))).toBe("running");
  expect(phase(chatState([task(1, "cancelled"), task(2)], { mode: "paused" }))).toBe("paused");
  // A paused queue with nothing waiting has nothing to resume.
  expect(phase(chatState([task(1, "failed")], { mode: "paused" }))).toBe("idle");
  expect(phase(chatState([task(1, "done"), task(2)], { mode: "running" }))).toBe("idle");
});

test("Clear asks first only when a task runs or waits", () => {
  expect(clearNeedsConfirm(chatState([]))).toBe(false);
  expect(clearNeedsConfirm(chatState([task(1, "done"), task(2, "failed"), task(3, "cancelled")]))).toBe(false);
  expect(clearNeedsConfirm(chatState([task(1, "done"), task(2, "running")]))).toBe(true);
  expect(clearNeedsConfirm(chatState([task(1, "cancelled"), task(2, "queued")], { mode: "paused" }))).toBe(true);
  expect(canClear(chatState([]))).toBe(false);
  expect(canClear(chatState([task(1, "done")]))).toBe(true);
});

test("a provider is selectable only when it is ready and the choice is not locked", () => {
  expect(providerSelectable(provider("claude", "ready"), false)).toBe(true);
  expect(providerSelectable(provider("claude", "ready"), true)).toBe(false);
  for (const availability of ["not_found", "not_signed_in", "too_old", "unsupported"] as const) {
    expect(providerSelectable(provider("codex", availability), false)).toBe(false);
  }
});

test("each availability has its own words", () => {
  const words = (["ready", "not_found", "not_signed_in", "too_old", "unsupported"] as const).map((a) => availabilityText(provider("claude", a)));
  expect(words).toEqual(["Ready", "Not installed", "Not signed in", "Too old (needs 2.1.0 or newer)", "Not supported"]);
});

test("the composer needs a known thread and a ready provider", () => {
  const ready = withProviders(emptyView(), [provider("claude", "ready"), provider("codex", "not_signed_in")]);
  // No state yet.
  expect(composerEnabled(ready)).toBe(false);
  const withState = applyEvent(ready, changed(chatState([])));
  expect(composerEnabled(withState)).toBe(true);
  expect(selectedProvider(withState)?.id).toBe("claude");
  // The thread's provider is not signed in.
  const codex = applyEvent(ready, changed(chatState([], { provider: "codex" })));
  expect(composerEnabled(codex)).toBe(false);
  // Detection has not answered.
  expect(composerEnabled(applyEvent(emptyView(), changed(chatState([]))))).toBe(false);
  // It stays enabled while a task runs: tasks queue.
  expect(composerEnabled(applyEvent(ready, changed(chatState([task(1, "running")]))))).toBe(true);
});

test("the prompt limit counts characters, not UTF-16 units, and ignores the outer blanks", () => {
  expect(promptProblem("")).toBe("empty");
  expect(promptProblem("  \n\t ")).toBe("empty");
  expect(promptProblem("a beat")).toBeNull();
  expect(promptLength("a🎵b")).toBe(3);
  expect(promptProblem("x".repeat(MAX_PROMPT_CHARS))).toBeNull();
  expect(promptProblem(`  ${"x".repeat(MAX_PROMPT_CHARS)}\n`)).toBeNull();
  expect(promptProblem("x".repeat(MAX_PROMPT_CHARS + 1))).toBe("too_long");
  // 16000 astral characters are 32000 UTF-16 units but within the limit.
  expect(promptProblem("🎵".repeat(MAX_PROMPT_CHARS))).toBeNull();
});

test("a command's rejection keeps its code, anything else is unknown", () => {
  expect(chatError({ code: "QUEUE_FULL", message: "the queue is full" })).toEqual({ code: "QUEUE_FULL", message: "the queue is full" });
  expect(chatError(new Error("boom"))).toEqual({ code: "UNKNOWN", message: "boom" });
  expect(chatError("plain")).toEqual({ code: "UNKNOWN", message: "plain" });
  expect(chatError({ message: "no code" })).toEqual({ code: "UNKNOWN", message: "no code" });
});
