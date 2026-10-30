<script lang="ts">
  // The Beta badge: the word "Beta" (never only a colour) on a button that explains it. The
  // explanation shows on hover, on keyboard focus and on a click (a click keeps it open until the
  // next click, Escape or the focus leaving), is hoverable and dismissible with Escape (WCAG
  // 1.4.13), and is the badge's accessible description.
  import { BETA_EXPLANATION } from "./model";

  let hovered = $state(false);
  let focused = $state(false);
  let pinned = $state(false);
  let dismissed = $state(false);
  const shown = $derived(!dismissed && (hovered || focused || pinned));

  function keydown(event: KeyboardEvent) {
    if (event.key !== "Escape" || !shown) return;
    event.stopPropagation();
    dismissed = true;
    pinned = false;
  }

  /** A click pins the explanation open; a click on a pinned one closes it. */
  function click() {
    if (shown && pinned) {
      pinned = false;
      dismissed = true;
    } else {
      pinned = true;
      dismissed = false;
    }
  }

  function enter() {
    hovered = true;
    dismissed = false;
  }
</script>

<span class="wrap" role="presentation" onmouseenter={enter} onmouseleave={() => (hovered = false)}>
  <button
    type="button"
    class="beta"
    aria-label="Beta, experimental feature"
    aria-describedby="beta-explanation"
    onclick={click}
    onfocus={() => {
      focused = true;
      dismissed = false;
    }}
    onblur={() => {
      focused = false;
      pinned = false;
    }}
    onkeydown={keydown}
  >
    Beta
  </button>
  <span id="beta-explanation" class="tip" role="tooltip" hidden={!shown}>{BETA_EXPLANATION}</span>
</span>

<style>
  .wrap {
    position: relative;
    display: inline-flex;
  }
  .beta {
    padding: 1px 8px;
    border: 1px solid var(--accent);
    border-radius: 999px;
    background: var(--panel-alt);
    color: var(--text);
    font: inherit;
    font-size: 11px;
    font-weight: 600;
    cursor: help;
  }
  .tip {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 30;
    width: 260px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel-alt);
    color: var(--text);
    font-size: 12px;
    line-height: 1.4;
    box-shadow: 0 6px 20px rgb(0 0 0 / 35%);
  }
  .tip[hidden] {
    display: none;
  }
</style>
