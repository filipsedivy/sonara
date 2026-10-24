<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import type { GestureDeps } from "../gesture";
  import type { Edit } from "../types";
  import { DRAG_PIXELS, type ControlSpec, keyValue, normOf, valueAtNorm, wheelValue } from "./controls";
  import { ValueDriver } from "./drive";

  // A rotary control (inventory CT `Knob`; MX-02 pan, MX-11 send level): a 270° sweep with the
  // value arc from the start, or from the centre when `bipolar`; an optional title above and the
  // value below. A vertical drag covers the range in 220 px (⇧ ten times finer), the wheel steps
  // it, the keys of `ValueControl` step, page, jump or reset it, and a double-click resets it.
  // Every change sends its edit through the gesture protocol (`ValueDriver`); the gesture's end
  // seals the undo step. A slider for assistive technology, with the value as its text.

  let {
    value,
    spec,
    name,
    format,
    deps,
    edit,
    title = "",
    bipolar = false,
    diameter = 30,
    showValue = true,
    minWidth = 58,
  }: {
    value: number;
    spec: ControlSpec;
    /** The accessible name ("Pan", "Send to Verb"). */
    name: string;
    format: (value: number) => string;
    deps: () => GestureDeps;
    edit: (value: number) => Edit;
    title?: string;
    bipolar?: boolean;
    diameter?: number;
    showValue?: boolean;
    minWidth?: number;
  } = $props();

  const START = 225;
  const SPAN = 270;

  let local: number | null = $state(null);
  const driver = new ValueDriver(
    () => deps(),
    (v) => edit(v),
    (v) => (local = v),
  );
  const current = $derived(local ?? value);
  const norm = $derived(normOf(spec, current));
  let origin: { y: number; norm: number } | null = null;

  $effect(() => {
    const model = value;
    untrack(() => driver.modelChanged(model));
  });
  onDestroy(() => void driver.finish());

  const width = $derived(Math.max(diameter + (showValue || title ? 16 : 4), minWidth));
  const cx = $derived(width / 2);
  const cy = $derived(3 + diameter / 2);
  const r = $derived(diameter / 2);

  /** The point at Qt angle `deg` (counter-clockwise from 3 o'clock) on a circle of `radius`. */
  const at = (deg: number, radius: number) => {
    const a = (deg * Math.PI) / 180;
    return `${cx + Math.cos(a) * radius} ${cy - Math.sin(a) * radius}`;
  };
  /** An arc from Qt angle `start` over `span` degrees (positive: counter-clockwise). */
  const arc = (start: number, span: number, radius: number) =>
    `M ${at(start, radius)} A ${radius} ${radius} 0 ${Math.abs(span) > 180 ? 1 : 0} ${span > 0 ? 0 : 1} ${at(start + span, radius)}`;

  const valueArc = $derived(bipolar ? { start: 90, span: -(norm - 0.5) * SPAN } : { start: START, span: -norm * SPAN });
  const needle = $derived(START - norm * SPAN);

  function pointerDown(event: PointerEvent) {
    if (event.button !== 0 || event.detail >= 2) return;
    event.stopPropagation();
    const element = event.currentTarget as HTMLElement;
    element.focus();
    try {
      element.setPointerCapture(event.pointerId);
    } catch {
      // A pointer the page made itself cannot be captured; it is followed anyway.
    }
    origin = { y: event.clientY, norm };
  }

  function pointerMove(event: PointerEvent) {
    if (!origin) return;
    const speed = DRAG_PIXELS * (event.shiftKey ? 10 : 1);
    origin.norm += (origin.y - event.clientY) / speed;
    origin.y = event.clientY;
    driver.change(valueAtNorm(spec, origin.norm, current), value);
  }

  function pointerUp() {
    if (!origin) return;
    origin = null;
    void driver.finish();
  }

  function doubleClick(event: MouseEvent) {
    event.stopPropagation();
    origin = null;
    const next = keyValue(spec, "Delete", false, current);
    if (next !== null) driver.change(next, value);
    void driver.finish();
  }

  function wheel(event: WheelEvent) {
    if (event.deltaY === 0) return;
    event.preventDefault();
    event.stopPropagation();
    driver.change(wheelValue(spec, event.deltaY, event.shiftKey, current), value);
    driver.wheeled();
  }

  function keyDown(event: KeyboardEvent) {
    const next = keyValue(spec, event.key, event.shiftKey, current);
    if (next === null) return;
    event.preventDefault();
    event.stopPropagation();
    driver.change(next, value);
    void driver.finish();
  }
</script>

<div
  class="knob"
  role="slider"
  tabindex="0"
  aria-label={name}
  aria-valuemin={spec.min}
  aria-valuemax={spec.max}
  aria-valuenow={current}
  aria-valuetext={format(current)}
  title="{name ? `${name}: ` : ''}{format(current)}&#10;Drag or scroll to change (Shift: fine) · double-click resets"
  style:width="{width}px"
  onpointerdown={pointerDown}
  onpointermove={pointerMove}
  onpointerup={pointerUp}
  onpointercancel={pointerUp}
  ondblclick={doubleClick}
  onwheel={wheel}
  onkeydown={keyDown}
>
  {#if title}<span class="title">{title}</span>{/if}
  <svg width={width} height={diameter + 6} aria-hidden="true">
    <g>
      <path class="track" d={arc(-45, SPAN, r)} />
      {#if Math.abs(valueArc.span) > 0.5}
        <path class="value" d={arc(valueArc.start, valueArc.span, r)} />
      {/if}
      <circle class="cap" cx={cx} cy={cy} r={Math.max(1, r - 5)} />
      <line
        class="needle"
        x1={cx + Math.cos((needle * Math.PI) / 180) * (r - 6.5) * 0.25}
        y1={cy - Math.sin((needle * Math.PI) / 180) * (r - 6.5) * 0.25}
        x2={cx + Math.cos((needle * Math.PI) / 180) * (r - 6.5)}
        y2={cy - Math.sin((needle * Math.PI) / 180) * (r - 6.5)}
      />
    </g>
  </svg>
  {#if showValue}<span class="value-text">{format(current)}</span>{/if}
</div>

<style>
  .knob {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    flex: none;
    outline: none;
    touch-action: none;
  }
  .title,
  .value-text {
    width: 100%;
    height: 14px;
    overflow: hidden;
    font-size: 10px;
    line-height: 14px;
    text-align: center;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .title {
    color: var(--text-dim);
  }
  svg {
    display: block;
    overflow: visible;
  }
  .track,
  .value {
    fill: none;
    stroke-width: 3.2;
    stroke-linecap: round;
  }
  .track {
    stroke: var(--border);
  }
  .value {
    stroke: var(--accent);
  }
  .cap {
    fill: var(--panel-alt);
    stroke: var(--border);
    stroke-width: 1;
  }
  .needle {
    stroke: var(--text);
    stroke-width: 2;
    stroke-linecap: round;
  }
  .knob:focus-visible svg,
  .knob:focus svg {
    filter: drop-shadow(0 0 1.5px var(--accent));
  }
</style>
