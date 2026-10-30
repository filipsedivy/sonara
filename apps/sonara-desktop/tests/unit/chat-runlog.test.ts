import { expect, test } from "@playwright/test";
import type { RunEvent } from "../../src/lib/chat/events";
import { MAX_ENTRIES, duration, emptyLog, reduce, sonaraMissing, stepCounts, type RunLog } from "../../src/lib/chat/runlog";

// The run log of one chat task: what the chat shows of a run, from the events of
// crates/sonara-app/src/composer/stream.rs.

const init: RunEvent = {
  kind: "init",
  session_id: "s1",
  model: "claude-x",
  tool_count: 40,
  mcp_servers: [{ name: "sonara", status: "connected" }],
  mcp_errors: [],
};

const result = (extra: Partial<Extract<RunEvent, { kind: "result" }>> = {}): RunEvent => ({
  kind: "result",
  success: true,
  subtype: "success",
  text: "Done.",
  duration_ms: 65_000,
  num_turns: 7,
  cost_usd: 0.12,
  session_id: "s1",
  usage: { input_tokens: 1, output_tokens: 2 },
  permission_denials: [],
  raw: "{}",
  ...extra,
});

function play(events: RunEvent[], from: RunLog = emptyLog()): RunLog {
  return events.reduce(reduce, from);
}

test("a run goes from idle to running to done, and keeps its steps in order", () => {
  let log = emptyLog();
  expect(log.phase).toBe("idle");
  log = reduce(log, init);
  expect(log).toMatchObject({ phase: "running", model: "claude-x", sessionId: "s1", toolCount: 40 });
  log = play(
    [
      { kind: "text", text: "Adding " },
      { kind: "text", text: "a bass." },
      { kind: "tool_use", id: "t1", tool: "create_track", summary: "{}", nested: false },
      { kind: "tool_use", id: "t2", tool: "set_tempo", summary: "{}", nested: false },
      { kind: "tool_result", id: "t2", ok: true, preview: "ok" },
      { kind: "tool_result", id: "t1", ok: false, preview: "NOT_FOUND" },
      { kind: "text", text: "Next." },
    ],
    log,
  );
  expect(log.entries.map((e) => (e.type === "text" ? e.text : `${e.tool}:${e.status}`))).toEqual([
    "Adding a bass.",
    "create_track:failed",
    "set_tempo:ok",
    "Next.",
  ]);
  expect(stepCounts(log)).toEqual({ total: 2, failed: 1 });
  expect(log.phase).toBe("running");
  expect(log.exit).toBeNull();
  log = play([result(), { kind: "exited", code: 0, stopped: false }], log);
  expect(log.phase).toBe("done");
  expect(log.result).toMatchObject({ success: true, text: "Done." });
  expect(log.exit).toEqual({ code: 0, stopped: false });
});

test("a stopped run keeps its result and marks the step it interrupted as failed", () => {
  let log = play([init, { kind: "tool_use", id: "t1", tool: "create_track", summary: "{}", nested: false }]);
  log = play([result({ success: false, subtype: "error_during_execution" }), { kind: "exited", code: 130, stopped: true }], log);
  expect(log.phase).toBe("done");
  expect(log.exit).toEqual({ code: 130, stopped: true });
  expect(log.result).toMatchObject({ success: false, subtype: "error_during_execution" });
  expect(log.entries[0]).toMatchObject({ type: "step", status: "failed" });
});

test("the last lines a failed run printed are kept, and a run without a result stays without one", () => {
  const crashed = play([{ kind: "stderr", line: "boom" }, { kind: "output", line: "partial" }, { kind: "exited", code: 2, stopped: false }]);
  expect(crashed).toMatchObject({ phase: "done", stderr: ["boom"], output: ["partial"], result: null, exit: { code: 2, stopped: false } });
});

test("a failed Sonara connection is detected from the init", () => {
  const failed = reduce(emptyLog(), {
    ...init,
    mcp_servers: [{ name: "sonara", status: "failed" }],
  } as RunEvent);
  expect(sonaraMissing(failed)).toBe(true);
  expect(sonaraMissing(reduce(emptyLog(), init))).toBe(false);
  expect(sonaraMissing(emptyLog())).toBe(false);
});

test("a retry shows until the next sign of life", () => {
  let log = reduce(emptyLog(), { kind: "retry", attempt: 2, max_retries: 10, delay_ms: 2000, error: "rate_limit" });
  expect(log.retry).toContain("rate_limit");
  expect(log.retry).toContain("2 of 10");
  log = reduce(log, { kind: "text", text: "x" });
  expect(log.retry).toBeNull();
});

test("the log is bounded", () => {
  let log = emptyLog();
  for (let i = 0; i < MAX_ENTRIES + 50; i++) {
    log = reduce(log, { kind: "tool_use", id: `t${i}`, tool: "x", summary: "", nested: false });
  }
  expect(log.entries).toHaveLength(MAX_ENTRIES);
  expect(log.entries[0]).toMatchObject({ id: "t50" });
  for (let i = 0; i < 300; i++) log = reduce(log, { kind: "stderr", line: `l${i}` });
  expect(log.stderr).toHaveLength(200);
  expect(log.stderr[199]).toBe("l299");
});

test("durations read as minutes and seconds", () => {
  expect(duration(4200)).toBe("4 s");
  expect(duration(65_000)).toBe("1 min 05 s");
});
