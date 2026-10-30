<script lang="ts">
  // The Beta badge: the word "Beta" (never only a colour) on a button that explains it. The
  // explanation shows on hover, on keyboard focus and on a click (a click keeps it open until the
  // next click, Escape, a click elsewhere or the focus leaving), is hoverable and dismissible with
  // Escape (WCAG 1.4.13), and is the badge's accessible description. WebKit does not focus a
  // button on a click, so a pinned explanation is closed by listeners on the document, not by the
  // button's own key and blur events.
  import { BETA_EXPLANATION } from "./model";

  let hovered = $state(false);
  let focused = $state(false);
  let pinned = $state(false);
  let dismissed = $state(false);
  let wrap: HTMLSpanElement | undefined = $state();
  const shown = $derived(!dismissed && (hovered || focused || pinned));

  function dismiss() {
    dismissed = true;
    pinned = false;
  }

  function keydown(event: KeyboardEvent) {
    if (event.key !== "Escape" || !shown) return;
    event.stopPropagation();
    dismiss();
  }

  // While it is open: Escape from anywhere, and a press outside the badge, close it. (Capture, so
  // that a field which handles Escape itself does not hide the key from here.)
  $effect(() => {
    if (!shown) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") dismiss();
    };
    const onPress = (event: PointerEvent) => {
      if (!(event.target instanceof Node && wrap?.contains(event.target))) dismiss();
    };
    document.addEventListener("keydown", onKey, true);
    document.addEventListener("pointerdown", onPress, true);
    return () => {
      document.removeEventListener("keydown", onKey, true);
      document.removeEventListener("pointerdown", onPress, true);
    };
  });

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

<span class="wrap" role="presentation" bind:this={wrap} onmouseenter={enter} onmouseleave={() => (hovered = false)}>
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
