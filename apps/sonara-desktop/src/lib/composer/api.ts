// The typed client of the AI composer's commands (src-tauri/src/composer.rs) and the events of a
// run, as sonara-app serializes them (crates/sonara-app/src/composer/stream.rs).

import { Channel, invoke } from "@tauri-apps/api/core";

/** One MCP server of the run, as Claude Code reports it. */
export interface McpServer {
  name: string;
  status: string;
}

/** What a run reports, in order (`composer::Event`). `exited` is always the last. */
export type ComposerEvent =
  | {
      kind: "init";
      session_id: string | null;
      model: string | null;
      tool_count: number;
      mcp_servers: McpServer[];
      mcp_errors: string[];
    }
  | { kind: "text"; text: string }
  | { kind: "tool_use"; id: string; tool: string; summary: string; nested: boolean }
  | { kind: "tool_result"; id: string; ok: boolean; preview: string }
  | { kind: "retry"; attempt: number; max_retries: number; delay_ms: number; error: string }
  | {
      kind: "result";
      success: boolean;
      subtype: string;
      text: string;
      duration_ms: number | null;
      num_turns: number | null;
      cost_usd: number | null;
      session_id: string | null;
      usage: Record<string, unknown> | null;
      permission_denials: string[];
      raw: string;
    }
  | { kind: "stderr"; line: string }
  | { kind: "output"; line: string }
  | { kind: "exited"; code: number | null; stopped: boolean };

/** `composer_check`: whether a run can start, and why not. */
export interface ComposerStatus {
  ready: boolean;
  running: boolean;
  claude_path: string | null;
  claude_version: string | null;
  problem: string | null;
}

export function composerCheck(): Promise<ComposerStatus> {
  return invoke<ComposerStatus>("composer_check");
}

/** Starts a run; `onEvent` gets its events until `exited`. Rejects with an `ErrorNotice`. */
export function composerStart(prompt: string, onEvent: (event: ComposerEvent) => void): Promise<void> {
  const channel = new Channel<ComposerEvent>();
  channel.onmessage = onEvent;
  return invoke<void>("composer_start", { prompt, channel });
}

/** Interrupts the run; Claude Code still reports its result. */
export function composerStop(): Promise<void> {
  return invoke<void>("composer_stop");
}
