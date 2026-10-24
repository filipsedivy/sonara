<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import type { GestureDeps } from "../gesture";
  import type { Edit } from "../types";
  import {
    FADER_SPEC,
    dbText,
    faderToNorm,
    keyValue,
    normOf,
    valueAtNorm,
    userValue,
    wheelValue,
  } from "./controls";
  import { ValueDriver } from "./drive";

  // The volume fader (CT `Fader`): −∞ to +12 dB on the fourth-root gain taper,
  // with the scale +12 … −60 and −∞. A drag moves the cap (⇧ ten times finer), a press on the
  // groove away from the cap jumps it there, the wheel steps it, and the keys of `ValueControl`
  // step, page, jump to the limits or reset it; a double-click resets it to 0 dB. Every change
  // sends its edit through the gesture protocol (`ValueDriver`), and the gesture's end seals the
  // undo step. A slider for assistive technology, with the value as its text.

  let {
    value,
    name,
    deps,
    edit,
    shown = $bindable(0),
  }: {
    /** The model's value in dB. */
    value: number;
    /** The accessible name ("Vox volume"). */
    name: string;
    deps: () => GestureDeps;
    edit: (db: number) => Edit;
    /** The value the fader shows (its own while a gesture drives it). */
    shown?: number;
  } = $props();

  const TICKS = [12, 6, 0, -6, -12, -24, -36, -60];
  /** The groove's inset from the top and the bottom (`_groove`). */
  const INSET = 10;

  let local: number | null = $state(null);
  const driver = new ValueDriver(
    () => deps(),
    (db) => edit(db),
    (v) => (local = v),
  );
  const current = $derived(local ?? value);
  let height = $state(180);
  let origin: { y: number; norm: number } | null = null;

  $effect(() => {
    shown = current;
  });
  $effect(() => {
    const model = value;
    untrack(() => driver.modelChanged(model));
  });
  onDestroy(() => void driver.finish());

  const grooveHeight = $derived(Math.max(1, height - 2 * INSET));
  const yFor = (norm: number) => INSET + (1 - norm) * grooveHeight;
  const capY = $derived(yFor(normOf(FADER_SPEC, current)));

  /** MX-03: a typed value, as a gesture of one step (its edit, then the seal). */
  export function commit(db: number): Promise<void> {
    set(db);
    return driver.finish();
  }

  function set(db: number) {
    driver.change(userValue(FADER_SPEC, db, current), value);
  }

  function setNorm(norm: number) {
    driver.change(valueAtNorm(FADER_SPEC, norm, current), value);
  }

  function pointerDown(event: PointerEvent) {
    if (event.button !== 0 || event.detail >= 2) return;
    event.stopPropagation();
    const element = event.currentTarget as HTMLElement;
    element.focus();
    try {
      element.setPointerCapture(event.pointerId);
    } catch {
      // A pointer the page made itself (the benchmark) cannot be captured; it is followed anyway.
    }
    const y = event.clientY - element.getBoundingClientRect().top;
    origin = { y, norm: normOf(FADER_SPEC, current) };
    if (Math.abs(y - capY) > 10) {
      origin.norm = (INSET + grooveHeight - y) / grooveHeight;
      setNorm(origin.norm);
    }
  }

  function pointerMove(event: PointerEvent) {
    if (!origin) return;
    const y = event.clientY - (event.currentTarget as HTMLElement).getBoundingClientRect().top;
    const fine = event.shiftKey ? 10 : 1;
    origin.norm += (origin.y - y) / (grooveHeight * fine);
    origin.y = y;
    setNorm(origin.norm);
  }

  function pointerUp() {
    if (!origin) return;
    origin = null;
    void driver.finish();
  }

  function doubleClick(event: MouseEvent) {
    event.stopPropagation();
    origin = null;
    set(FADER_SPEC.defaultValue);
    void driver.finish();
  }

  function wheel(event: WheelEvent) {
    if (event.deltaY === 0) return;
    event.preventDefault();
    event.stopPropagation();
    driver.change(wheelValue(FADER_SPEC, event.deltaY, event.shiftKey, current), value);
    driver.wheeled();
  }

  function keyDown(event: KeyboardEvent) {
    const next = keyValue(FADER_SPEC, event.key, event.shiftKey, current);
    if (next === null) return;
    event.preventDefault();
    event.stopPropagation();
    driver.change(next, value);
    void driver.finish();
  }

  const tick = (db: number) => (db === 0 ? "0" : `${db > 0 ? "+" : ""}${db}`);
</script>

<div
  class="fader"
  role="slider"
  tabindex="0"
  aria-label={name}
  aria-orientation="vertical"
  aria-valuemin={FADER_SPEC.min}
  aria-valuemax={FADER_SPEC.max}
  aria-valuenow={current}
  aria-valuetext={dbText(current)}
  title="{name}: {dbText(current)}&#10;Drag or scroll to change (Shift: fine) · double-click resets"
  bind:clientHeight={height}
  onpointerdown={pointerDown}
  onpointermove={pointerMove}
  onpointerup={pointerUp}
  onpointercancel={pointerUp}
  ondblclick={doubleClick}
  onwheel={wheel}
  onkeydown={keyDown}
>
  {#each TICKS as db (db)}
    {@const y = yFor(faderToNorm(db))}
    <span class="label" class:unity={db === 0} style:top="{y - 7}px">{tick(db)}</span>
    <span class="tick" class:unity={db === 0} style:top="{y}px"></span>
  {/each}
  <span class="label" style:top="{yFor(0) - 7}px">-∞</span>
  <!-- The cap and the fill move by transforms, which a drag's every move can change without a
       layout. -->
  <div class="groove" style:top="{INSET}px" style:height="{grooveHeight}px">
    <div class="fill" style:transform="scaleY({(INSET + grooveHeight - capY) / grooveHeight})"></div>
  </div>
  <div class="cap" style:transform="translateY({capY - 6}px)"></div>
</div>

<style>
  .fader {
    position: relative;
    flex: none;
    width: 40px;
    min-height: 120px;
    height: 100%;
    outline: none;
    touch-action: none;
  }
  .label {
    position: absolute;
    left: 0;
    width: 18px;
    height: 14px;
    font-size: 8px;
    line-height: 14px;
    text-align: right;
    color: var(--text-dim);
    pointer-events: none;
  }
  .label.unity {
    color: var(--text);
  }
  .tick {
    position: absolute;
    left: 20px;
    width: 4px;
    height: 1px;
    background: var(--grid-beat);
    pointer-events: none;
  }
  .tick.unity {
    background: var(--grid-bar);
  }
  .groove {
    position: absolute;
    left: 26px;
    width: 4px;
    border-radius: 2px;
    background: var(--border);
  }
  .fill {
    position: absolute;
    inset: 0;
    border-radius: 2px;
    background: color-mix(in srgb, var(--accent) 67%, transparent);
    transform-origin: bottom;
  }
  .cap {
    position: absolute;
    top: 0;
    left: 19px;
    width: 18px;
    height: 12px;
    border: 1.2px solid var(--border);
    border-radius: 2.5px;
    background: var(--panel-alt);
  }
  .cap::after {
    content: "";
    position: absolute;
    left: 3px;
    right: 3px;
    top: 4.5px;
    height: 1.2px;
    background: var(--text);
  }
  .fader:focus-visible .cap,
  .fader:focus .cap {
    border-color: var(--accent);
  }
</style>
