// What drives a value control (the knob, the fader, a send's level): the gesture protocol of
// gesture.ts on the control's changes. A change sends its edit on the
// gesture's next display frame; the end of the gesture (release, a key step, 350 ms after the
// last wheel step: the prototype's `editingFinished`) seals the undo step.
//
// While a gesture drives the control, and after it until the project view has its value, the
// control shows its own value: the prototype's "a control the user is dragging is not
// overwritten". A gesture that starts while the previous one still ends sends nothing before that
// one's seal, so two key steps never merge into one undo step.

import { LiveGesture, type GestureDeps } from "../gesture";
import type { Edit } from "../types";
import { WHEEL_END_MS } from "./controls";

/** How long a finished gesture's value waits for the project view before it gives way. */
export const RELEASE_MS = 600;

export class ValueDriver {
  private value: number | null = null;
  private gesture: LiveGesture | null = null;
  private ending: Promise<void> = Promise.resolve();
  private wheelTimer: ReturnType<typeof setTimeout> | undefined;
  private releaseTimer: ReturnType<typeof setTimeout> | undefined;

  /**
   * `show` hears the value the control shows instead of the model's (null: the model's), for the
   * component's reactive state.
   */
  constructor(
    private readonly deps: () => GestureDeps,
    private readonly edit: (value: number) => Edit,
    private readonly show: (local: number | null) => void = () => {},
  ) {}

  /** The value the control shows instead of the model's, while it has one. */
  get local(): number | null {
    return this.value;
  }

  private set local(value: number | null) {
    if (value !== this.value) {
      this.value = value;
      this.show(value);
    }
  }

  /** Whether a gesture drives the control now. */
  get driving(): boolean {
    return this.gesture !== null;
  }

  /** The value to show over the model's `model`. */
  shown(model: number): number {
    return this.local ?? model;
  }

  /** The user changed the control to `value` (clamped already); nothing when it stays. */
  change(value: number, model: number): void {
    if (value === this.shown(model)) return;
    clearTimeout(this.releaseTimer);
    this.local = value;
    if (this.gesture === null) {
      const deps = this.deps();
      const after = this.ending;
      this.gesture = new LiveGesture({ ...deps, send: (edit) => after.then(() => deps.send(edit)) });
    }
    this.gesture.move(this.edit(value));
  }

  /** The gesture ends: its last edit is sent, then the undo step is sealed. */
  finish(): Promise<void> {
    clearTimeout(this.wheelTimer);
    const gesture = this.gesture;
    this.gesture = null;
    if (gesture === null) return this.ending;
    this.ending = gesture.end().then(() => {
      if (this.gesture === null) {
        clearTimeout(this.releaseTimer);
        this.releaseTimer = setTimeout(() => {
          if (this.gesture === null) this.local = null;
        }, RELEASE_MS);
      }
    });
    return this.ending;
  }

  /** A wheel step: the gesture ends 350 ms after the last one. */
  wheeled(): void {
    clearTimeout(this.wheelTimer);
    this.wheelTimer = setTimeout(() => void this.finish(), WHEEL_END_MS);
  }

  /** The project view's value arrived: a finished gesture's value gives way once it matches. */
  modelChanged(model: number): void {
    if (this.gesture === null && this.local !== null && Math.abs(this.local - model) < 1e-9) {
      clearTimeout(this.releaseTimer);
      this.local = null;
    }
  }
}
