// A strip's meter: its box, of role `meter`, holds the canvas, which is registered
// with the window's `MeterBank`, which draws it at every animation frame and sets the box's value
// for assistive technology a few times a second; the canvas is resized with its box and
// with the display's scale (a move to a display with another scale factor); a press
// clears its clip indicator.

import { watchDevicePixelRatio } from "../hidpi";
import type { MeterBank } from "./meter";

export interface MeterCanvasParams {
  bank: MeterBank;
  trackId: string;
}

/** The Svelte action of a meter's box, around its canvas. */
export function meterCanvas(box: HTMLElement, params: MeterCanvasParams) {
  const canvas = box.querySelector("canvas");
  if (!canvas) return {};
  let current = params;
  const register = () => current.bank.register(canvas, () => current.trackId, performance.now() / 1000, box);
  let unregister = register();
  const resize = () =>
    current.bank.resize(canvas, canvas.clientWidth, canvas.clientHeight, window.devicePixelRatio || 1);
  const observer = new ResizeObserver(resize);
  observer.observe(canvas);
  const stopWatching = watchDevicePixelRatio(resize);
  const press = (event: PointerEvent) => {
    event.stopPropagation();
    current.bank.clearClip(canvas);
  };
  box.addEventListener("pointerdown", press);
  return {
    update(next: MeterCanvasParams) {
      if (next.bank !== current.bank) {
        unregister();
        current = next;
        unregister = register();
        resize();
      } else {
        current = next;
      }
    },
    destroy() {
      unregister();
      observer.disconnect();
      stopWatching();
      box.removeEventListener("pointerdown", press);
    },
  };
}
