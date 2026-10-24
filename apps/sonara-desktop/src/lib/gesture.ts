// The gesture protocol: one gesture is one undo step, as in the prototype.
//
// - Timeline drags, the ruler's loop and the velocity lane preview locally and commit one edit
//   on release, then seal the undo step ({@link commitOnRelease}).
// - Faders, knobs, sends and piano-roll drags send their edit on every move ({@link LiveGesture}):
//   at most one edit in flight and at most one sent per display frame, always the latest value;
//   the commands' merge keys make the moves one step, and the release seals it once the last
//   edit has landed (a seal before it would split the gesture in two).
//
// The page draws the pointer's position at once, as the spike's edit protocol does; a control
// that a gesture drives ignores the project view's value for it until the gesture ends.

import type { Edit } from "./types";

/** What a gesture talks to: the backend's `edit` and `seal_undo`, and the display's frames. */
export interface GestureDeps {
  send: (edit: Edit) => Promise<unknown>;
  seal: () => Promise<void>;
  /** Runs `callback` at the next display frame (`requestAnimationFrame`). */
  nextFrame: (callback: () => void) => void;
  /** Reports a refused edit (the status bar); the gesture goes on. */
  onError: (error: unknown) => void;
}

/** Commits `edit` once, then seals the undo step. */
export async function commitOnRelease(deps: GestureDeps, edit: Edit): Promise<boolean> {
  let committed = true;
  try {
    await deps.send(edit);
  } catch (error) {
    committed = false;
    deps.onError(error);
  }
  await deps.seal();
  return committed;
}

/** A gesture that edits on every move. */
export class LiveGesture {
  /** The latest value that is not sent yet. */
  private pending: Edit | null = null;
  private inFlight: Promise<void> | null = null;
  private frameRequested = false;
  private ending = false;
  /** Edits sent so far. */
  sent = 0;

  constructor(private readonly deps: GestureDeps) {}

  /** Whether the gesture still runs: a control it drives keeps its own value. */
  get active(): boolean {
    return !this.ending;
  }

  /** The control moved: `edit` is its new value. Values between two frames collapse. */
  move(edit: Edit): void {
    if (this.ending) return;
    this.pending = edit;
    this.requestFrame();
  }

  /** The release: waits for the edit in flight, sends the last value, then seals the step. */
  async end(): Promise<void> {
    this.ending = true;
    while (this.inFlight !== null || this.pending !== null) {
      if (this.inFlight !== null) {
        await this.inFlight;
      } else {
        this.dispatch();
      }
    }
    await this.deps.seal();
  }

  private requestFrame(): void {
    if (this.frameRequested || this.inFlight !== null) return;
    this.frameRequested = true;
    this.deps.nextFrame(() => {
      this.frameRequested = false;
      if (!this.ending) this.dispatch();
    });
  }

  private dispatch(): void {
    if (this.inFlight !== null || this.pending === null) return;
    const edit = this.pending;
    this.pending = null;
    this.sent += 1;
    this.inFlight = this.deps
      .send(edit)
      .then(
        () => undefined,
        (error: unknown) => this.deps.onError(error),
      )
      .finally(() => {
        this.inFlight = null;
        if (this.pending !== null && !this.ending) this.requestFrame();
      });
  }
}
