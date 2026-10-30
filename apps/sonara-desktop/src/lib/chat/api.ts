// The typed client of the AI Chat's commands (src-tauri/src/chat.rs) and its event, with the
// payloads as sonara-app serializes them (crates/sonara-app/src/chat). The chat's state lives in
// Rust: every command answers with the new state, but the window follows the `chat-event` stream,
// which carries every change, whoever caused it.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { RunEvent } from "./events";

/** A task's id: a private counter of the chat, never a project id. */
export type TaskId = number;

export type ProviderId = "claude" | "codex";

export type TaskState = "queued" | "running" | "done" | "failed" | "cancelled";

/** `running` also when idle with an empty queue. */
export type QueueMode = "running" | "paused";

export type Availability = "ready" | "not_found" | "not_signed_in" | "too_old" | "unsupported";

/** One provider as detection found it (`chat_providers`). */
export interface ProviderStatus {
  id: ProviderId;
  name: string;
  availability: Availability;
  version: string | null;
  path: string | null;
  /** How to fix it, or a short note when it is ready. */
  hint: string;
  beta: boolean;
  min_version: string;
}

/** What a run reported at its end. */
export interface TaskResult {
  text: string;
  duration_ms: number | null;
  num_turns: number | null;
  cost_usd: number | null;
  input_tokens: number | null;
  output_tokens: number | null;
  session_id: string | null;
  denied_tools: string[];
}

export interface TaskView {
  id: TaskId;
  prompt: string;
  provider: ProviderId;
  state: TaskState;
  /** A failed task's reason. */
  error: string | null;
  result: TaskResult | null;
}

/** The whole thread: in thread order, the queued tasks in queue order. */
export interface ChatState {
  generation: number;
  mode: QueueMode;
  provider: ProviderId;
  provider_locked: boolean;
  tasks: TaskView[];
}

/** The events of one task so far (bounded, text coalesced). */
export interface TaskLog {
  task_id: TaskId;
  events: RunEvent[];
}

export interface ChatSnapshot {
  state: ChatState;
  logs: TaskLog[];
}

/** `chat-event`: a change of the state, or one normalised event of a task's run. */
export type ChatEvent =
  | { kind: "changed"; state: ChatState }
  | { kind: "run"; task_id: TaskId; generation: number; event: RunEvent };

/** The codes a chat command rejects with. */
export type ChatErrorCode =
  | "PROMPT_EMPTY"
  | "PROMPT_TOO_LONG"
  | "TASK_NOT_FOUND"
  | "TASK_NOT_QUEUED"
  | "PROVIDER_LOCKED"
  | "PROVIDER_UNAVAILABLE"
  | "QUEUE_FULL"
  | "CONFLICT";

/** An error of a chat command. The code is open: anything else (a failed call) is `UNKNOWN`. */
export interface ChatError {
  code: ChatErrorCode | "UNKNOWN";
  message: string;
}

/** `error` as a {@link ChatError}: a rejection `{ code, message }` as it is, anything else as `UNKNOWN`. */
export function chatError(error: unknown): ChatError {
  if (typeof error === "object" && error !== null && "message" in error) {
    const { code, message } = error as { code?: unknown; message: unknown };
    return { code: typeof code === "string" ? (code as ChatError["code"]) : "UNKNOWN", message: String(message) };
  }
  return { code: "UNKNOWN", message: String(error) };
}

export function chatSnapshot(): Promise<ChatSnapshot> {
  return invoke<ChatSnapshot>("chat_snapshot");
}

/** Detects the providers again; may take a second. */
export function chatProviders(): Promise<ProviderStatus[]> {
  return invoke<ProviderStatus[]>("chat_providers");
}

/** Rejects with PROVIDER_LOCKED once the thread has tasks. */
export function chatSetProvider(provider: ProviderId): Promise<ChatState> {
  return invoke<ChatState>("chat_set_provider", { provider });
}

/** Queues a task; it starts at once when nothing runs and the queue is not paused. */
export function chatEnqueue(prompt: string): Promise<TaskId> {
  return invoke<TaskId>("chat_enqueue", { prompt });
}

/** Changes the prompt of a queued task. */
export function chatEdit(id: TaskId, prompt: string): Promise<ChatState> {
  return invoke<ChatState>("chat_edit", { id, prompt });
}

/** Removes a queued task. */
export function chatRemove(id: TaskId): Promise<ChatState> {
  return invoke<ChatState>("chat_remove", { id });
}

/** Moves a queued task to `toIndex` among the queued tasks. */
export function chatMove(id: TaskId, toIndex: number): Promise<ChatState> {
  return invoke<ChatState>("chat_move", { id, toIndex });
}

/** Stops the running task (it becomes `cancelled`) and pauses the queue. */
export function chatStop(): Promise<ChatState> {
  return invoke<ChatState>("chat_stop");
}

/** Resumes a paused queue: the next queued task starts. */
export function chatResume(): Promise<ChatState> {
  return invoke<ChatState>("chat_resume");
}

/** Stops everything, drops the thread and starts a new generation. */
export function chatClear(): Promise<ChatState> {
  return invoke<ChatState>("chat_clear");
}

/** Every change of the chat, to this window only. */
export function onChatEvent(handler: (event: ChatEvent) => void): Promise<UnlistenFn> {
  return listen<ChatEvent>("chat-event", (message) => handler(message.payload));
}
