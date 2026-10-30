// What the AI Chat window shows: the state the backend sends (the queue and the thread), a log
// per task built from the run events with the run log's reducer, and the providers. Pure
// functions over an immutable view (the store keeps it), so that the generation rule, the resync
// and the rules of the buttons are testable. The backend is the truth: nothing here predicts a
// change, the next `changed` event shows it.

import { emptyLog, reduce, type RunLog } from "./runlog";
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

/**
 * The most UTF-16 units a message box takes: twice the limit, because a code point is at most two
 * units. A hard guard against a huge paste; the counter and the limit still count code points.
 */
export const MAX_PROMPT_UNITS = 2 * MAX_PROMPT_CHARS;

/**
 * The characters of `text` as the limit counts them (a code point is one). The same number as
 * `[...text].length` (a lone surrogate counts as one), without building the array.
 */
export function promptLength(text: string): number {
  let count = 0;
  for (let at = 0; at < text.length; at += 1) {
    count += 1;
    const unit = text.charCodeAt(at);
    // A high surrogate followed by a low one is one code point: skip the low one.
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = text.charCodeAt(at + 1);
      if (next >= 0xdc00 && next <= 0xdfff) at += 1;
    }
  }
  return count;
}

export type PromptProblem = "empty" | "too_long";

/**
 * Why `text` cannot be sent, or null. A code point is one or two UTF-16 units, so a text of at
 * most the limit in units is surely within it and one of more than twice the limit surely is not:
 * only the texts in between are counted.
 */
export function promptProblem(text: string): PromptProblem | null {
  const trimmed = text.trim();
  if (trimmed === "") return "empty";
  if (trimmed.length <= MAX_PROMPT_CHARS) return null;
  if (trimmed.length > MAX_PROMPT_UNITS) return "too_long";
  return promptLength(trimmed) > MAX_PROMPT_CHARS ? "too_long" : null;
}

/**
 * `text` cut to at most `max` characters (code points, never inside a surrogate pair), with an
 * ellipsis in place of the rest. Short text is returned as it is.
 */
export function clip(text: string, max: number): string {
  if (text.length <= max) return text;
  let units = 0;
  let points = 0;
  let cutAt = 0;
  for (const point of text) {
    if (points === max - 1) cutAt = units;
    points += 1;
    units += point.length;
    if (points > max) return `${text.slice(0, cutAt)}…`;
  }
  return text;
}

/** The longest label of a task, in characters. */
export const EXCERPT_CHARS = 80;

/** The first line of a prompt, short enough for a label. */
export function excerpt(text: string): string {
  return clip(text.trim().split("\n")[0], EXCERPT_CHARS);
}

/** What Clear history says it will do, before "and erase the conversation". */
export function clearSentence(running: boolean, queued: number): string {
  const what = [
    running ? "stop the running task" : null,
    queued > 0 ? `drop ${queued === 1 ? "1 queued task" : `${queued} queued tasks`}` : null,
  ].filter((part) => part !== null);
  // Nothing runs or waits (the state changed under the open dialog): no empty fragment.
  return what.length === 0 ? "This erases the conversation." : `This will ${what.join(" and ")}, and erase the conversation.`;
}

// -- what a screen reader is told ----------------------------------------------------------------

/** The characters of the answer a task's announcement carries. */
export const ANNOUNCE_CHARS = 80;

/** How one task's end is announced. */
function endText(task: TaskView): string {
  switch (task.state) {
    case "done": {
      const answer = clip((task.result?.text ?? "").trim().replace(/\s+/g, " "), ANNOUNCE_CHARS);
      return answer === "" ? "Task done" : `Task done: ${answer}`;
    }
    case "failed":
      return task.error ? `Task failed: ${task.error}` : "Task failed";
    default:
      return "Task cancelled";
  }
}

/**
 * What to announce when the state moves from `before` to `after`: one sentence for each task that
 * was queued or running and has ended since. Nothing before the first state or across a new
 * generation (a reloaded window must not announce the old tasks), and not for streamed text.
 */
export function endAnnouncement(before: ChatState | null, after: ChatState): string | null {
  if (before === null || before.generation !== after.generation) return null;
  const open = new Set(before.tasks.filter((task) => task.state === "queued" || task.state === "running").map((task) => task.id));
  const ended = after.tasks.filter((task) => open.has(task.id) && task.state !== "queued" && task.state !== "running");
  return ended.length === 0 ? null : ended.map(endText).join(" ");
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
