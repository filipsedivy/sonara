<script lang="ts">
  // A pane's splitter: dragging it resizes the pane; the arrow keys move it 16 px.
  // `direction` is the sign of the pane's growth per pixel the pointer moves (a left pane grows
  // to the right, a right pane to the left, the Editors upwards).
  let {
    orientation,
    label,
    size,
    direction,
    onresize,
  }: {
    orientation: "vertical" | "horizontal";
    label: string;
    size: number;
    direction: 1 | -1;
    onresize: (size: number) => void;
  } = $props();

  let start: { pointer: number; size: number } | null = null;

  function down(event: PointerEvent) {
    event.preventDefault();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    start = { pointer: orientation === "vertical" ? event.clientX : event.clientY, size };
  }

  function move(event: PointerEvent) {
    if (!start) return;
    const pointer = orientation === "vertical" ? event.clientX : event.clientY;
    onresize(start.size + direction * (pointer - start.pointer));
  }

  function up() {
    start = null;
  }

  function key(event: KeyboardEvent) {
    const grow = orientation === "vertical" ? ["ArrowRight", "ArrowLeft"] : ["ArrowDown", "ArrowUp"];
    const step = event.key === grow[0] ? 16 : event.key === grow[1] ? -16 : 0;
    if (step === 0) return;
    event.preventDefault();
    onresize(size + direction * step);
  }
</script>

<!-- A focusable separator is the WAI-ARIA window splitter; Svelte's rule counts every separator as
     static. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="splitter {orientation}"
  role="separator"
  aria-orientation={orientation}
  aria-label={label}
  aria-valuenow={Math.round(size)}
  tabindex="0"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  onkeydown={key}
></div>

<style>
  .splitter {
    flex: none;
    background: var(--border);
    touch-action: none;
  }
  .splitter.vertical {
    width: 4px;
    cursor: col-resize;
  }
  .splitter.horizontal {
    height: 4px;
    cursor: row-resize;
  }
  .splitter:hover,
  .splitter:focus-visible {
    background: var(--accent);
    outline: none;
  }
</style>
