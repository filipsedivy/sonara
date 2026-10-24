<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import * as backend from "../backend";
  import { HiDpiCanvas } from "../hidpi";
  import type { EqResponse, ParamValue } from "../types";
  import { CURVE_SIZE, CurveLoader, paintCurve } from "./eq";

  // The parametric EQ's response curve on its card (the prototype's `EqCurve`): sonara-app's
  // `eq_response` of the device, asked for again whenever the device's parameters change (here, by
  // an agent or by undo), painted at the device pixel ratio. Decorative for assistive technology:
  // the knobs carry its values (accessibility).

  let {
    deviceId,
    params,
    onerror,
  }: {
    deviceId: string;
    /** The device's parameters; a change asks for the curve again. */
    params: Record<string, ParamValue>;
    onerror: (error: unknown) => void;
  } = $props();

  let canvas: HTMLCanvasElement | undefined = $state();
  let curve: EqResponse | null = null;
  let hidpi: HiDpiCanvas | null = null;

  const loader = new CurveLoader(
    () => backend.eqResponse(deviceId),
    (next) => {
      curve = next;
      draw();
    },
    (error) => onerror(error),
  );

  function draw() {
    if (!canvas || !hidpi) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const scale = hidpi.scale;
    ctx.setTransform(scale, 0, 0, scale, 0, 0);
    paintCurve(ctx, hidpi.cssWidth || CURVE_SIZE.width, hidpi.cssHeight || CURVE_SIZE.height, curve);
  }

  $effect(() => {
    // Every new parameters object (a whole or a patched view) asks for the curve again.
    void params;
    loader.refresh();
  });

  onMount(() => {
    if (canvas) hidpi = new HiDpiCanvas(canvas, draw);
    draw();
  });
  onDestroy(() => hidpi?.destroy());
</script>

<canvas
  bind:this={canvas}
  class="curve"
  aria-hidden="true"
  title="Equalizer frequency response"
  data-device={deviceId}
  style:width="{CURVE_SIZE.width}px"
  style:height="{CURVE_SIZE.height}px"
></canvas>

<style>
  .curve {
    display: block;
    flex: none;
  }
</style>
