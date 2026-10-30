import { expect, test } from "@playwright/test";
import type { ChatSnapshot } from "../../src/lib/chat/api";
import { SNAPSHOT_TRIES, readFollowUp, readUntilQuiet, type EventFlag } from "../../src/lib/chat/snapshot";

// The first snapshot while events stream in: events during a read are not applied, so the read is
// repeated until it is quiet, and one that never was is read once more (the store applies the
// follow-up only when it was quiet).

const snapshot = (generation: number): ChatSnapshot => ({
  state: { generation, mode: "running", provider: "claude", provider_locked: false, tasks: [] },
  logs: [],
});

/** A read that returns the next snapshot and lets `disturb(call)` raise the flag during the call. */
function fakeRead(flag: EventFlag, disturb: (call: number) => boolean) {
  let calls = 0;
  const read = async () => {
    calls += 1;
    if (disturb(calls)) flag.arrived = true;
    return snapshot(calls);
  };
  return { read, calls: () => calls };
}

test("a quiet first read is read once", async () => {
  const flag: EventFlag = { arrived: true };
  const fake = fakeRead(flag, () => false);
  const first = await readUntilQuiet(fake.read, flag);
  expect(fake.calls()).toBe(1);
  expect(first.quiet).toBe(true);
  expect(first.snapshot.state.generation).toBe(1);
});

test("a read an event disturbed is repeated until it is quiet", async () => {
  const flag: EventFlag = { arrived: false };
  const fake = fakeRead(flag, (call) => call < 3);
  const first = await readUntilQuiet(fake.read, flag);
  expect(fake.calls()).toBe(3);
  expect(first.quiet).toBe(true);
  expect(first.snapshot.state.generation).toBe(3);
});

test("when every try is disturbed, the last snapshot comes back marked as doubtful", async () => {
  const flag: EventFlag = { arrived: false };
  const fake = fakeRead(flag, () => true);
  const first = await readUntilQuiet(fake.read, flag);
  expect(fake.calls()).toBe(SNAPSHOT_TRIES);
  expect(first.quiet).toBe(false);
  expect(first.snapshot.state.generation).toBe(SNAPSHOT_TRIES);
});

test("a rejected read rejects", async () => {
  const flag: EventFlag = { arrived: false };
  await expect(readUntilQuiet(() => Promise.reject(new Error("no snapshot")), flag)).rejects.toThrow("no snapshot");
});

test("the follow-up read is used when nothing arrived during it, and dropped when an event did", async () => {
  const quiet: EventFlag = { arrived: true };
  const quietRead = fakeRead(quiet, () => false);
  // The flag from the earlier, disturbed reads does not count: the follow-up clears it first.
  expect((await readFollowUp(quietRead.read, quiet))?.state.generation).toBe(1);

  const busy: EventFlag = { arrived: false };
  const busyRead = fakeRead(busy, () => true);
  expect(await readFollowUp(busyRead.read, busy)).toBeNull();
});
