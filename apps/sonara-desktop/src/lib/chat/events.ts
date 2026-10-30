// The events of one run of an agent, as sonara-app serializes them
// (crates/sonara-app/src/composer/stream.rs). The AI Chat carries them in its `run` events
// (api.ts) and replays them in its snapshot.

/** One MCP server of the run, as the agent reports it. */
export interface McpServer {
  name: string;
  status: string;
}

/** What a run reports, in order (`composer::Event`, normalised across the agents). `exited` is always the last. */
export type RunEvent =
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
