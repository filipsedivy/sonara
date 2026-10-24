// The measured segments of the benchmarks (timeline.ts, meters.ts; the spike's `Disturbances`
// and `retrying` in ): a segment during which the page
// was hidden or no animation frame came for 250 ms is invalid and is measured again, up to five
// times; focus changes are only recorded, since the window stays on top.

import { sleep } from "./stats";

export interface Validity {
  valid: boolean;
  maxRafGapMs: number;
  frames: number;
  expectedFrames: number;
  events: string[];
}

/** Resolves at the start of the next frame, or after `timeoutMs` without one. */
export function nextFrame(timeoutMs = 1000): Promise<number> {
  return new Promise((resolve) => {
    let done = false;
    const finish = (time: number) => {
      if (!done) {
        done = true;
        resolve(time);
      }
    };
    requestAnimationFrame(finish);
    setTimeout(() => finish(performance.now()), timeoutMs);
  });
}

/** Watches what makes a measured segment unrepresentative (the spike's `Disturbances`). */
export class Disturbances {
  private readonly events: string[] = [];
  readonly segments: Validity[] = [];

  constructor() {
    document.addEventListener("visibilitychange", () => this.events.push(`visibility:${document.visibilityState}`));
    window.addEventListener("blur", () => this.events.push("blur"));
    window.addEventListener("focus", () => this.events.push("focus"));
  }

  /** Starts a segment of `frames`; the returned function ends it. */
  begin(frameTimes: number[]): () => Validity {
    const firstEvent = this.events.length;
    const firstFrame = frameTimes.length;
    const started = performance.now();
    return () => {
      const ended = performance.now();
      const times = frameTimes.slice(firstFrame);
      let maxGap = times.length ? Math.max(times[0] - started, ended - times[times.length - 1]) : ended - started;
      for (let i = 1; i < times.length; i++) maxGap = Math.max(maxGap, times[i] - times[i - 1]);
      const events = this.events.slice(firstEvent);
      const hidden = events.includes("visibility:hidden");
      const validity = {
        valid: maxGap < 250 && !hidden,
        maxRafGapMs: maxGap,
        frames: times.length,
        expectedFrames: Math.floor((ended - started) / (1000 / 60)),
        events,
      };
      this.segments.push(validity);
      return validity;
    };
  }
}

/** Waits for 30 frames in a row less than 100 ms apart on a visible page; fails after a minute. */
export async function waitForFrames(maxWaitMs = 60_000): Promise<void> {
  const deadline = performance.now() + maxWaitMs;
  let good = 0;
  let last = performance.now();
  while (performance.now() < deadline) {
    const now = await nextFrame(1000);
    good = now - last < 100 && document.visibilityState === "visible" ? good + 1 : 0;
    last = now;
    if (good >= 30) return;
  }
  throw new Error(`no animation frames for ${maxWaitMs / 1000} s: the window is not visible`);
}

/** Measures a segment until it is valid, at most five times; the last attempt is kept either way. */
export async function retrying<T extends { validity: Validity }>(
  disturbances: Disturbances,
  measure: () => Promise<T>,
): Promise<T & { attempts: number }> {
  let attempts = 0;
  for (;;) {
    attempts += 1;
    await waitForFrames();
    const result = await measure();
    if (result.validity.valid || attempts >= 5) return { ...result, attempts };
    disturbances.segments.pop();
    await sleep(1000);
  }
}
