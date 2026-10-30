// What the AI Chat shows of one task's run: a log of the assistant's text and of its tool calls
// in order, the session's facts, and how the process ended. A pure reducer over the run's events
// (the chat keeps one log per task), so that the progress and the end of a run are testable.

import type { McpServer, RunEvent } from "./events";

/** The most entries kept: a long song is hundreds of calls, never thousands. */
export const MAX_ENTRIES = 2000;
const MAX_LINES = 200;

export type StepStatus = "running" | "ok" | "failed";

export type Entry =
  | { type: "text"; text: string }
  | {
      type: "step";
      id: string;
      tool: string;
      summary: string;
      nested: boolean;
      status: StepStatus;
      preview: string;
    };

export type ResultInfo = Extract<RunEvent, { kind: "result" }>;

export type Phase = "idle" | "running" | "done";

export interface RunLog {
  phase: Phase;
  entries: Entry[];
  model: string | null;
  sessionId: string | null;
  toolCount: number;
  mcpServers: McpServer[];
  mcpErrors: string[];
  /** The latest API retry, until the next sign of life. */
  retry: string | null;
  result: ResultInfo | null;
  stderr: string[];
  output: string[];
  exit: { code: number | null; stopped: boolean } | null;
}

export function emptyLog(): RunLog {
  return {
    phase: "idle",
    entries: [],
    model: null,
    sessionId: null,
    toolCount: 0,
    mcpServers: [],
    mcpErrors: [],
    retry: null,
    result: null,
    stderr: [],
    output: [],
    exit: null,
  };
}

/** Whether the Sonara tools failed to connect: the run can compose nothing. */
export function sonaraMissing(log: RunLog): boolean {
  return log.mcpServers.some((server) => server.name === "sonara" && server.status === "failed");
}

function tail<T>(items: T[], item: T, max: number): T[] {
  const next = [...items, item];
  return next.length > max ? next.slice(next.length - max) : next;
}

function pushEntry(entries: Entry[], entry: Entry): Entry[] {
  const next = [...entries, entry];
  return next.length > MAX_ENTRIES ? next.slice(next.length - MAX_ENTRIES) : next;
}

/** `log` after `event`. */
export function reduce(log: RunLog, event: RunEvent): RunLog {
  switch (event.kind) {
    case "init":
      return {
        ...log,
        phase: "running",
        model: event.model,
        sessionId: event.session_id,
        toolCount: event.tool_count,
        mcpServers: event.mcp_servers,
        mcpErrors: event.mcp_errors,
      };
    case "text": {
      const last = log.entries[log.entries.length - 1];
      const entries =
        last?.type === "text"
          ? [...log.entries.slice(0, -1), { type: "text" as const, text: last.text + event.text }]
          : pushEntry(log.entries, { type: "text", text: event.text });
      return { ...log, entries, retry: null };
    }
    case "tool_use":
      return {
        ...log,
        retry: null,
        entries: pushEntry(log.entries, {
          type: "step",
          id: event.id,
          tool: event.tool,
          summary: event.summary,
          nested: event.nested,
          status: "running",
          preview: "",
        }),
      };
    case "tool_result":
      return {
        ...log,
        entries: log.entries.map((entry) =>
          entry.type === "step" && entry.id === event.id
            ? { ...entry, status: event.ok ? "ok" : "failed", preview: event.preview }
            : entry,
        ),
      };
    case "retry":
      return {
        ...log,
        retry: `API error (${event.error}); attempt ${event.attempt} of ${event.max_retries}, next in ${Math.round(event.delay_ms / 1000)} s`,
      };
    case "result":
      return { ...log, result: event, retry: null };
    case "stderr":
      return { ...log, stderr: tail(log.stderr, event.line, MAX_LINES) };
    case "output":
      return { ...log, output: tail(log.output, event.line, MAX_LINES) };
    case "exited":
      return {
        ...log,
        phase: "done",
        retry: null,
        exit: { code: event.code, stopped: event.stopped },
        // A step still running when the process ended did not finish.
        entries: log.entries.map((entry) =>
          entry.type === "step" && entry.status === "running" ? { ...entry, status: "failed" } : entry,
        ),
      };
  }
}

/** The steps of a log, for the counter. */
export function stepCounts(log: RunLog): { total: number; failed: number } {
  let total = 0;
  let failed = 0;
  for (const entry of log.entries) {
    if (entry.type !== "step") continue;
    total += 1;
    if (entry.status === "failed") failed += 1;
  }
  return { total, failed };
}

/** `ms` as `1 min 05 s`. */
export function duration(ms: number): string {
  const seconds = Math.round(ms / 1000);
  if (seconds < 60) return `${seconds} s`;
  return `${Math.floor(seconds / 60)} min ${String(seconds % 60).padStart(2, "0")} s`;
}
