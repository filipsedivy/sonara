// The spike's two-frame queue (the "fifo" policy that
// passed ADR 0009's meter thresholds): frames from the channel wait for the next animation
// frame, each animation frame shows the oldest waiting one, and at most QUEUE_DEPTH wait; beyond
// that the oldest is dropped (superseded). A 60 Hz producer read by an unsynchronized 60 Hz
// display then shows every frame once instead of beating against it.
//
// A page that is not drawn gets no animation frames: WebKit gives a hidden web view none (the
// window minimized, hidden, on another Space, behind a full-screen app or tucked away by Stage
// Manager), and none come while the display sleeps. Such a page shows each frame when it arrives
// ({@link waitsForDisplay}), so that its transport state, engine and playhead follow the
// transport, which a hidden main window still acts on (the mixer window's keys, the menu, a seek).

/** Most frames waiting for a draw. */
export const QUEUE_DEPTH = 2;

/**
 * Without an animation frame for this long the page is taken as not drawn (the benchmarks' gap of
 * an invalid segment, bench/segments.ts, so a valid measurement never meets it).
 */
export const UNDRAWN_MS = 250;

/**
 * Whether a frame that arrives now waits for the next animation frame: only while the page is
 * drawn, that is visible and with an animation frame in the last {@link UNDRAWN_MS}.
 */
export function waitsForDisplay(hidden: boolean, sinceAnimationFrameMs: number): boolean {
  return !hidden && sinceAnimationFrameMs < UNDRAWN_MS;
}

/** A frame with its sequence number. */
export interface Sequenced {
  sequence: number;
}

export class FrameQueue<T extends Sequenced> {
  /** Frames received. */
  received = 0;
  /** Frames dropped unseen because the queue was full. */
  superseded = 0;
  /** Frames the producer numbered but that never arrived. */
  gaps = 0;
  private lastSequence: number | null = null;
  private readonly waiting: T[] = [];

  /** Queues a frame from the channel. */
  push(frame: T): void {
    if (this.lastSequence !== null) {
      // Sequence numbers wrap at 2^32.
      const step = (frame.sequence - this.lastSequence) >>> 0;
      if (step > 1) this.gaps += step - 1;
    }
    this.lastSequence = frame.sequence;
    this.received += 1;
    if (this.waiting.length >= QUEUE_DEPTH) {
      this.waiting.shift();
      this.superseded += 1;
    }
    this.waiting.push(frame);
  }

  /** The frame to show at this animation frame, or null when none is waiting. */
  take(): T | null {
    return this.waiting.shift() ?? null;
  }

  /** How many frames wait. */
  get length(): number {
    return this.waiting.length;
  }
}
