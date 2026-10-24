<script lang="ts">
  // `ToggleButton` (inventory CT): a small pill (M, S, R, Pre) in its colour when pressed,
  // focusable with Tab, with its pressed state for assistive technology (`aria-pressed`).
  let {
    label,
    pressed,
    color = "accent",
    title,
    width = 22,
    onclick,
  }: {
    label: string;
    pressed: boolean;
    color?: "accent" | "mute" | "solo" | "arm";
    title: string;
    width?: number;
    onclick: () => void;
  } = $props();
</script>

<button
  type="button"
  class="toggle {color}"
  aria-pressed={pressed}
  aria-label={title}
  {title}
  style:min-width="{width}px"
  onclick={(event) => {
    event.stopPropagation();
    onclick();
  }}>{label}</button
>

<style>
  .toggle {
    height: 18px;
    padding: 0 4px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--panel-alt);
    color: var(--text-dim);
    font: inherit;
    font-size: 10px;
    font-weight: 700;
    line-height: 16px;
    cursor: pointer;
  }
  .toggle:hover {
    filter: brightness(1.12);
  }
  .toggle[aria-pressed="true"] {
    color: #16171a;
  }
  .accent[aria-pressed="true"] {
    background: var(--accent);
    color: #fff;
  }
  .mute[aria-pressed="true"] {
    background: var(--meter-yellow);
  }
  .solo[aria-pressed="true"] {
    background: var(--meter-green);
  }
  .arm[aria-pressed="true"] {
    background: var(--meter-red);
  }
</style>
