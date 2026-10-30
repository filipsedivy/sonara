// What the AI Chat window shows: the state the backend sends (the queue and the thread), a log
// per task built from the run events with the composer's reducer, and the providers. Pure
// functions over an immutable view (the store keeps it), so that the generation rule, the resync
// and the rules of the buttons are testable. The backend is the truth: nothing here predicts a
// change, the next `changed` event shows it.

import { emptyLog, reduce, type RunLog } from "../composer/model";
import type { ChatEvent, ChatSnapshot, ChatState, ProviderStatus, TaskId, TaskResult, TaskView } from "./api";

/** The longest prompt, in characters (the backend refuses more with PROMPT_TOO_LONG). */
export const MAX_PROMPT_CHARS = 16_000;

/** What the Beta badge says (the contract's wording). */
export const BETA_EXPLANATION =
  "Experimental feature: behaviour may change. The agent edits your project. Undo works step by step.";

export interface ChatView {
  /** Null until the first snapshot. */
  state: ChatState | null;
  /** The run log of each task that has reported anything, by task id. */
  logs: Readonly<Record<TaskId, RunLog>>;
  /** Null until detection has answered. */
  providers: readonly ProviderStatus[] | null;
}

export function emptyView(): ChatView {
  return { state: null, logs: {}, providers: null };
}

/** The log of `id`, empty when the task has reported nothing. */
export function logOf(view: ChatView, id: TaskId): RunLog {
  return view.logs[id] ?? emptyLog();
}

/**
 * `view` after `event`. A `changed` event replaces the state; a new generation (the thread was
 * wiped: Clear, a closed or replaced project) also drops every log. A `run` event of another
 * generation than the state's is stale, from a task of a wiped thread, and changes nothing; so
 * does any `run` event before the first state.
 */
export function applyEvent(view: ChatView, event: ChatEvent): ChatView {
  if (event.kind === "changed") {
    const wiped = view.state !== null && view.state.generation !== event.state.generation;
    return { ...view, state: event.state, logs: wiped ? {} : view.logs };
  }
  if (view.state === null || event.generation !== view.state.generation) return view;
  const log = reduce(logOf(view, event.task_id), event.event);
  return { ...view, logs: { ...view.logs, [event.task_id]: log } };
}

/** `view` rebuilt from a snapshot: its state, and each task's log from the events it holds. */
export function resync(view: ChatView, snapshot: ChatSnapshot): ChatView {
  const logs: Record<TaskId, RunLog> = {};
  for (const { task_id, events } of snapshot.logs) logs[task_id] = events.reduce(reduce, emptyLog());
  return { ...view, state: snapshot.state, logs };
}

/** `view` with the providers detection found. */
export function withProviders(view: ChatView, providers: readonly ProviderStatus[]): ChatView {
  return { ...view, providers };
}

/** The provider the thread uses (or will use): the state's choice, as detection reports it. */
export function selectedProvider(view: ChatView): ProviderStatus | null {
  const id = view.state?.provider;
  return view.providers?.find((provider) => provider.id === id) ?? null;
}

// -- the queue ------------------------------------------------------------------------------

export function queuedTasks(state: ChatState): TaskView[] {
  return state.tasks.filter((task) => task.state === "queued");
}

/** The ids of the queued tasks, in queue order. */
export function queuedIds(state: ChatState): TaskId[] {
  return queuedTasks(state).map((task) => task.id);
}

export type MoveDirection = "up" | "down";

/**
 * Where `chat_move` should put `id` to move it one step: its index among the queued tasks
 * plus or minus one, or null when it is not queued or already at that end.
 */
export function moveTarget(state: ChatState, id: TaskId, direction: MoveDirection): number | null {
  const ids = queuedIds(state);
  const at = ids.indexOf(id);
  if (at < 0) return null;
  const to = direction === "up" ? at - 1 : at + 1;
  return to >= 0 && to < ids.length ? to : null;
}

export function canMove(state: ChatState, id: TaskId, direction: MoveDirection): boolean {
  return moveTarget(state, id, direction) !== null;
}

// -- the window's phase and its buttons -----------------------------------------------------------

/**
 * `running` while a task runs; `paused` when the queue is paused with tasks waiting (a stop or a
 * failure paused it: Run queue starts the next); otherwise `idle`.
 */
export type Phase = "idle" | "running" | "paused";

export function phase(state: ChatState): Phase {
  if (state.tasks.some((task) => task.state === "running")) return "running";
  if (state.mode === "paused" && state.tasks.some((task) => task.state === "queued")) return "paused";
  return "idle";
}

/** The running task, if any. */
export function runningTask(state: ChatState): TaskView | null {
  return state.tasks.find((task) => task.state === "running") ?? null;
}

/** Clear history asks first when it would stop a running task or drop queued ones. */
export function clearNeedsConfirm(state: ChatState): boolean {
  return state.tasks.some((task) => task.state === "running" || task.state === "queued");
}

/** Whether there is anything to clear. */
export function canClear(state: ChatState): boolean {
  return state.tasks.length > 0;
}

/** Whether a provider can be chosen: it is ready, and the thread has not locked the choice. */
export function providerSelectable(status: ProviderStatus, locked: boolean): boolean {
  return status.availability === "ready" && !locked;
}

/** Whether a prompt can be sent: the thread is known and its provider is ready. */
export function composerEnabled(view: ChatView): boolean {
  return view.state !== null && selectedProvider(view)?.availability === "ready";
}

/** The characters of `text` as the limit counts them (a code point is one). */
export function promptLength(text: string): number {
  return [...text].length;
}

export type PromptProblem = "empty" | "too_long";

/** Why `text` cannot be sent, or null. */
export function promptProblem(text: string): PromptProblem | null {
  if (text.trim() === "") return "empty";
  return promptLength(text.trim()) > MAX_PROMPT_CHARS ? "too_long" : null;
}

/** Why a provider cannot run, in words. */
export function availabilityText(status: ProviderStatus): string {
  switch (status.availability) {
    case "ready":
      return "Ready";
    case "not_found":
      return "Not installed";
    case "not_signed_in":
      return "Not signed in";
    case "too_old":
      return `Too old (needs ${status.min_version} or newer)`;
    case "unsupported":
      return "Not supported";
  }
}

// -- the footer of a task ---------------------------------------------------------------------

/** `usd` as dollars, to four decimals below one dollar. */
export function money(usd: number): string {
  return `$${usd.toFixed(usd < 1 ? 4 : 2)}`;
}

/** The tokens a run used, `—` when it reported none. */
export function tokensText(result: Pick<TaskResult, "input_tokens" | "output_tokens">): string {
  if (result.input_tokens === null && result.output_tokens === null) return "—";
  return `${result.input_tokens ?? 0} in, ${result.output_tokens ?? 0} out`;
}

/** What a finished task says of itself: the words of its state, with the reason of a failure. */
export function taskOutcomeText(task: TaskView): string {
  switch (task.state) {
    case "done":
      return "Finished";
    case "failed":
      return task.error ? `Did not finish: ${task.error}` : "Did not finish";
    case "cancelled":
      return "Stopped by you";
    case "running":
      return "Working…";
    case "queued":
      return "Waiting";
  }
}
