import { expect, test } from "@playwright/test";
import type { ChatState, TaskView } from "../../src/lib/chat/api";
import {
  MAX_PROMPT_CHARS,
  MAX_PROMPT_UNITS,
  clearSentence,
  clip,
  endAnnouncement,
  excerpt,
  promptLength,
  promptProblem,
} from "../../src/lib/chat/model";

// The text rules of the AI Chat window: the code-point counter and its limit, the labels, the
// sentence of the Clear dialog and what a screen reader hears when a task ends.

const NOTE = "\u{1D11E}"; // a musical symbol: two UTF-16 units, one code point

/** Whether `text` has no lone surrogate (String.prototype.isWellFormed is newer than the test target). */
const isWellFormed = (text: string) => !/[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/.test(text);

const task = (id: number, state: TaskView["state"], extra: Partial<TaskView> = {}): TaskView => ({
  id,
  prompt: `prompt ${id}`,
  provider: "claude",
  state,
  error: null,
  result: null,
  ...extra,
});
const state = (tasks: TaskView[], extra: Partial<ChatState> = {}): ChatState => ({
  generation: 1,
  mode: "running",
  provider: "claude",
  provider_locked: true,
  tasks,
  ...extra,
});
const result = (text: string): TaskView["result"] => ({
  text,
  duration_ms: null,
  num_turns: null,
  cost_usd: null,
  input_tokens: null,
  output_tokens: null,
  session_id: null,
  denied_tools: [],
});

// -- the counter ----------------------------------------------------------------------------

test("the counter counts code points like the spread does, lone surrogates included", () => {
  const pieces = ["a", "é", NOTE, "\u{1F3B5}", "\ud834", "\udd1e", "\n", " ", "日本語"];
  // A fixed pseudo-random sequence (a linear congruential generator), so that the test is the same every run.
  let seed = 12345;
  const next = () => (seed = (seed * 1103515245 + 12345) % 2147483648);
  for (let round = 0; round < 300; round += 1) {
    let text = "";
    for (let n = next() % 12; n > 0; n -= 1) text += pieces[next() % pieces.length];
    expect(promptLength(text), JSON.stringify(text)).toBe([...text].length);
  }
});

test("astral text counts one per code point", () => {
  expect(promptLength(NOTE.repeat(MAX_PROMPT_CHARS))).toBe(MAX_PROMPT_CHARS);
  expect(promptLength("")).toBe(0);
});

test("the limit is in code points: the astral text at the limit is sent, one more is not", () => {
  expect(promptProblem(NOTE.repeat(MAX_PROMPT_CHARS))).toBeNull();
  expect(promptProblem(NOTE.repeat(MAX_PROMPT_CHARS + 1))).toBe("too_long");
  expect(promptProblem("a".repeat(MAX_PROMPT_CHARS))).toBeNull();
  expect(promptProblem("a".repeat(MAX_PROMPT_CHARS + 1))).toBe("too_long");
});

test("blank text is empty, and the outer blanks do not count", () => {
  expect(promptProblem(" \n\t ")).toBe("empty");
  expect(promptProblem(`  ${"a".repeat(MAX_PROMPT_CHARS)}  `)).toBeNull();
});

test("a huge text is refused without being counted by code point", () => {
  // More than twice the limit in units is over the limit whatever it holds.
  expect(promptProblem(NOTE.repeat(MAX_PROMPT_UNITS))).toBe("too_long");
  expect(promptProblem("a".repeat(5_000_000))).toBe("too_long");
});

// -- the labels -------------------------------------------------------------------------------

test("a short first line is the label as it is, and only the first line counts", () => {
  expect(excerpt("  Add a bass\nthen drums")).toBe("Add a bass");
  expect(clip("12345", 5)).toBe("12345");
});

test("a long first line is cut to 80 characters with an ellipsis", () => {
  const label = excerpt("a".repeat(100));
  expect(label).toBe(`${"a".repeat(79)}…`);
  expect([...label]).toHaveLength(80);
});

test("the cut is by code points and never splits a surrogate pair", () => {
  const label = excerpt(NOTE.repeat(80));
  // 80 code points are at the limit: kept whole.
  expect(label).toBe(NOTE.repeat(80));
  const cut = excerpt(NOTE.repeat(81));
  expect(cut).toBe(`${NOTE.repeat(79)}…`);
  expect(isWellFormed(cut)).toBe(true);
  // Mixed units: the pair that would straddle the cut stays whole.
  const mixed = excerpt(`${"a".repeat(78)}${NOTE}${NOTE}b`);
  expect(mixed).toBe(`${"a".repeat(78)}${NOTE}…`);
  expect(isWellFormed(mixed)).toBe(true);
});

// -- the Clear dialog -----------------------------------------------------------------------

test("the Clear sentence names what runs or waits, and stays whole when nothing does", () => {
  expect(clearSentence(true, 0)).toBe("This will stop the running task, and erase the conversation.");
  expect(clearSentence(false, 1)).toBe("This will drop 1 queued task, and erase the conversation.");
  expect(clearSentence(false, 3)).toBe("This will drop 3 queued tasks, and erase the conversation.");
  expect(clearSentence(true, 3)).toBe("This will stop the running task and drop 3 queued tasks, and erase the conversation.");
  expect(clearSentence(false, 0)).toBe("This erases the conversation.");
});

// -- what a screen reader hears ---------------------------------------------------------------

test("a task that finishes is announced with the start of its answer", () => {
  const before = state([task(1, "running")]);
  const after = state([task(1, "done", { result: result("All set.\n\nI added a bass.") })]);
  expect(endAnnouncement(before, after)).toBe("Task done: All set. I added a bass.");
});

test("a long answer is cut at 80 characters", () => {
  const after = state([task(1, "done", { result: result("x".repeat(200)) })]);
  expect(endAnnouncement(state([task(1, "running")]), after)).toBe(`Task done: ${"x".repeat(79)}…`);
  const astral = state([task(1, "done", { result: result(NOTE.repeat(200)) })]);
  expect(isWellFormed(endAnnouncement(state([task(1, "running")]), astral) ?? "")).toBe(true);
});

test("a task that fails or is stopped is announced with the reason", () => {
  const running = state([task(1, "running")]);
  expect(endAnnouncement(running, state([task(1, "failed", { error: "Not logged in" })]))).toBe("Task failed: Not logged in");
  expect(endAnnouncement(running, state([task(1, "failed")]))).toBe("Task failed");
  expect(endAnnouncement(running, state([task(1, "cancelled")]))).toBe("Task cancelled");
  expect(endAnnouncement(running, state([task(1, "done", { result: result("  ") })]))).toBe("Task done");
});

test("nothing is announced for a task that did not end in this change", () => {
  const running = state([task(1, "running"), task(2, "queued")]);
  // Another task joins or starts, text streams (the state is the same): no ending.
  expect(endAnnouncement(running, state([task(1, "running"), task(2, "queued"), task(3, "queued")]))).toBeNull();
  expect(endAnnouncement(running, state([task(1, "running"), task(2, "running")]))).toBeNull();
  expect(endAnnouncement(running, running)).toBeNull();
  // A task that was already over stays quiet.
  const over = state([task(1, "done"), task(2, "running")]);
  expect(endAnnouncement(over, state([task(1, "done"), task(2, "running")]))).toBeNull();
});

test("nothing is announced for the first state, nor across a new generation", () => {
  const ended = state([task(1, "done", { result: result("Old.") })]);
  expect(endAnnouncement(null, ended)).toBeNull();
  expect(endAnnouncement(state([task(1, "running")]), { ...ended, generation: 2 })).toBeNull();
});

test("two tasks that end together are both announced", () => {
  const before = state([task(1, "running"), task(2, "queued")]);
  const after = state([task(1, "done", { result: result("One.") }), task(2, "cancelled")]);
  expect(endAnnouncement(before, after)).toBe("Task done: One. Task cancelled");
});
