// Reading the chat's snapshot while its events stream in. The window listens first and reads the
// snapshot after, so no change is missed; but an event that arrives while a read is on its way is
// not applied (the snapshot may be older or newer than it). That is settled here: the read is
// repeated until it is quiet, and a read that never was is checked once more later. Plain
// functions, so the unit tests can drive them with a fake read.

import type { ChatSnapshot } from "./api";

/** How often the first snapshot is read again when events arrive while it is on its way. */
export const SNAPSHOT_TRIES = 5;

/** How long after the first snapshot a doubtful one is read again, in milliseconds. */
export const RESYNC_DELAY_MS = 400;

/** Whether an event arrived since it was last cleared: the store sets it, a read clears it. */
export interface EventFlag {
  arrived: boolean;
}

export interface FirstSnapshot {
  snapshot: ChatSnapshot;
  /** False when an event arrived during the last read too: the snapshot may be stale. */
  quiet: boolean;
}

/**
 * Reads the snapshot until no event arrives during a read, at most `tries` times. When every try
 * was disturbed the last snapshot is returned all the same, marked as not quiet, so that the
 * caller can read once more after a pause. Rejects with the read's own rejection.
 */
export async function readUntilQuiet(read: () => Promise<ChatSnapshot>, flag: EventFlag, tries: number = SNAPSHOT_TRIES): Promise<FirstSnapshot> {
  let snapshot: ChatSnapshot;
  let quiet = false;
  let attempt = 0;
  do {
    flag.arrived = false;
    snapshot = await read();
    quiet = !flag.arrived;
    attempt += 1;
  } while (!quiet && attempt < tries);
  return { snapshot, quiet };
}

/**
 * The follow-up read of a doubtful first snapshot, made while events apply live: `snapshot` when
 * no event arrived during the read (so it is the state now), otherwise null, because the live
 * events (a `changed` carries the whole state) are newer than it.
 */
export async function readFollowUp(read: () => Promise<ChatSnapshot>, flag: EventFlag): Promise<ChatSnapshot | null> {
  flag.arrived = false;
  const snapshot = await read();
  return flag.arrived ? null : snapshot;
}
