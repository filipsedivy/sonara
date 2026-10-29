import { expect, test } from "@playwright/test";
import type { ComposerEvent } from "../../src/lib/composer/api";
import {
  MAX_ENTRIES,
  duration,
  emptyLog,
  failedToStart,
  failureReason,
  outcome,
  reduce,
  sonaraMissing,
  starting,
  stepCounts,
  stopping,
  type RunLog,
} from "../../src/lib/composer/model";

// The AI composer's log: what the panel shows of a run, from the events of
// crates/sonara-app/src/composer/stream.rs.

const init: ComposerEvent = {
  kind: "init",
  session_id: "s1",
  model: "claude-x",
  tool_count: 40,
  mcp_servers: [{ name: "sonara", status: "connected" }],
  mcp_errors: [],
};

const result = (extra: Partial<Extract<ComposerEvent, { kind: "result" }>> = {}): ComposerEvent => ({
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

function play(events: ComposerEvent[], from: RunLog = starting()): RunLog {
  return events.reduce(reduce, from);
}

test("a run goes from starting to running to done, and keeps its steps in order", () => {
  let log = starting();
  expect(log.phase).toBe("starting");
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
  expect(outcome(log)).toBe("running");
  log = play([result(), { kind: "exited", code: 0, stopped: false }], log);
  expect(log.phase).toBe("done");
  expect(outcome(log)).toBe("done");
  expect(log.exit).toEqual({ code: 0, stopped: false });
});

test("a stop ends as stopped even though Claude Code still reports a result", () => {
  let log = play([init, { kind: "tool_use", id: "t1", tool: "create_track", summary: "{}", nested: false }]);
  log = stopping(log);
  expect(log.phase).toBe("stopping");
  // An init that arrives late must not undo the stop.
  expect(reduce(log, init).phase).toBe("stopping");
  log = play([result({ success: false, subtype: "error_during_execution" }), { kind: "exited", code: 130, stopped: true }], log);
  expect(outcome(log)).toBe("stopped");
  expect(log.entries[0]).toMatchObject({ type: "step", status: "failed" });
  // Stopping a finished run changes nothing.
  expect(stopping(log)).toBe(log);
});

test("a failed run says why, from the result, else the last line it printed", () => {
  const notLoggedIn = play([result({ success: false, subtype: "success", text: "Not logged in · Please run /login" }), { kind: "exited", code: 1, stopped: false }]);
  expect(outcome(notLoggedIn)).toBe("failed");
  expect(failureReason(notLoggedIn)).toBe("Not logged in · Please run /login");

  const crashed = play([{ kind: "stderr", line: "boom" }, { kind: "exited", code: 2, stopped: false }]);
  expect(failureReason(crashed)).toBe("boom");

  const silent = play([{ kind: "exited", code: 9, stopped: false }]);
  expect(failureReason(silent)).toContain("code 9");

  const refused = failedToStart("Claude Code is not installed");
  expect(outcome(refused)).toBe("failed");
  expect(failureReason(refused)).toBe("Claude Code is not installed");
});

test("a failed Sonara connection is detected from the init", () => {
  const failed = reduce(starting(), {
    ...init,
    mcp_servers: [{ name: "sonara", status: "failed" }],
  } as ComposerEvent);
  expect(sonaraMissing(failed)).toBe(true);
  expect(sonaraMissing(reduce(starting(), init))).toBe(false);
  expect(sonaraMissing(emptyLog())).toBe(false);
});

test("a retry shows until the next sign of life", () => {
  let log = reduce(starting(), { kind: "retry", attempt: 2, max_retries: 10, delay_ms: 2000, error: "rate_limit" });
  expect(log.retry).toContain("rate_limit");
  expect(log.retry).toContain("2 of 10");
  log = reduce(log, { kind: "text", text: "x" });
  expect(log.retry).toBeNull();
});

test("the log is bounded", () => {
  let log = starting();
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
