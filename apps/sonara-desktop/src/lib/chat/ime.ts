// Whether a key press belongs to an input method (Japanese, Chinese, Korean). The Enter that
// accepts a candidate must not send or save: WebKit reports `compositionend` before that keydown,
// and the keydown has `isComposing` false and `keyCode` 229, so none of the three signals is
// enough alone.

/** The keyCode a keydown carries while an input method handles the key. */
const IME_KEY_CODE = 229;

export class Composition {
  private active = false;
  private justEnded = false;
  private timer: ReturnType<typeof setTimeout> | undefined;

  /** The `compositionstart` handler. */
  start = (): void => {
    this.clearTimer();
    this.active = true;
    this.justEnded = false;
  };

  /**
   * The `compositionend` handler. The committing keydown can follow it, so the composition counts
   * until the next task of the event loop; a later key is the user's own.
   */
  end = (): void => {
    this.active = false;
    this.justEnded = true;
    this.clearTimer();
    this.timer = setTimeout(() => {
      this.justEnded = false;
      this.timer = undefined;
    }, 0);
  };

  /** Forgets everything (the field was replaced, so no `compositionend` may come). */
  reset(): void {
    this.clearTimer();
    this.active = false;
    this.justEnded = false;
  }

  /** Whether `event` is part of a composition and must not act as a plain key. */
  handles(event: Pick<KeyboardEvent, "isComposing" | "keyCode">): boolean {
    return this.active || this.justEnded || event.isComposing || event.keyCode === IME_KEY_CODE;
  }

  private clearTimer(): void {
    if (this.timer !== undefined) clearTimeout(this.timer);
    this.timer = undefined;
  }
}
