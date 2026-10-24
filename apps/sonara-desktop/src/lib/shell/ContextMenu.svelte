<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { rememberFocus } from "../a11y/focus";
  import ContextMenu from "./ContextMenu.svelte";
  import type { MenuItem } from "./menu";

  // A popup menu at the pointer (the timeline's context menu, TL-08; the "+ Track" menu, TL-11;
  // the track headers', TH-05; the mixer's insert, send and effect menus): items run
  // and close it; Escape, a click elsewhere or the window losing focus close it. The arrow keys
  // move between the items; → or a hover opens a submenu beside its item, ← closes it again.
  // `ondone` closes the whole menu after a submenu's item ran (its parent's `onclose`).

  let {
    x,
    y,
    items,
    label,
    onclose,
    ondone,
    flip,
  }: {
    x: number;
    y: number;
    items: readonly MenuItem[];
    label: string;
    onclose: () => void;
    ondone?: () => void;
    /** A submenu's right edge when it does not fit right of `x`: its item's left. */
    flip?: number;
  } = $props();

  let menu: HTMLDivElement | undefined = $state();
  // Step 11: a menu that closes gives the focus back to where it was, unless the focus moved on
  // (a click elsewhere, a field the item opened); a submenu leaves that to its menu.
  const restoreFocus = rememberFocus((active) => active === null || active === document.body);
  onDestroy(() => {
    if (!ondone) restoreFocus();
  });
  let open: { index: number; x: number; y: number; flip: number } | null = $state(null);

  function buttons(): HTMLButtonElement[] {
    return menu ? [...menu.querySelectorAll<HTMLButtonElement>(":scope > button")] : [];
  }

  function done() {
    if (ondone) ondone();
    else onclose();
  }

  function openSubmenu(index: number, button: HTMLElement) {
    const box = button.getBoundingClientRect();
    open = { index, x: box.right - 2, y: box.top - 5, flip: box.left + 2 };
  }

  function pick(item: MenuItem, index: number, button: HTMLElement) {
    if (item === "separator" || "disabled" in item) return;
    if ("items" in item) {
      openSubmenu(index, button);
      return;
    }
    done();
    item.run();
  }

  function keydown(event: KeyboardEvent) {
    if (event.target instanceof Node && !buttons().some((b) => b === event.target)) return;
    const all = buttons();
    const at = all.indexOf(document.activeElement as HTMLButtonElement);
    if (event.key === "Escape" || (event.key === "ArrowLeft" && ondone)) {
      event.preventDefault();
      event.stopPropagation();
      onclose();
    } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const step = event.key === "ArrowDown" ? 1 : -1;
      all[(at + step + all.length) % all.length]?.focus();
    } else if (event.key === "ArrowRight" && at >= 0) {
      const index = Number(all[at].dataset.index);
      const item = items[index];
      if (item !== "separator" && "items" in item) {
        event.preventDefault();
        openSubmenu(index, all[at]);
      }
    }
  }

  onMount(() => {
    buttons()[0]?.focus();
    // Kept inside the window.
    if (menu) {
      const box = menu.getBoundingClientRect();
      // A submenu that does not fit right of its item opens on its left, not over it.
      const left = flip !== undefined && x + box.width + 4 > window.innerWidth ? flip - box.width : x;
      menu.style.left = `${Math.max(0, Math.min(left, window.innerWidth - box.width - 4))}px`;
      menu.style.top = `${Math.max(0, Math.min(y, window.innerHeight - box.height - 4))}px`;
    }
    const away = (event: PointerEvent) => {
      if (menu && !menu.contains(event.target as Node)) onclose();
    };
    const blur = () => onclose();
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("blur", blur);
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("blur", blur);
    };
  });
</script>

<div
  bind:this={menu}
  class="menu"
  role="menu"
  aria-label={label}
  tabindex="-1"
  style:left="{x}px"
  style:top="{y}px"
  onkeydown={keydown}
>
  {#each items as item, i (i)}
    {#if item === "separator"}
      <div class="separator" role="separator"></div>
    {:else if "disabled" in item}
      <button type="button" role="menuitem" aria-disabled="true" disabled data-index={i}>{item.label}</button>
    {:else if "items" in item}
      <button
        type="button"
        role="menuitem"
        aria-haspopup="menu"
        aria-expanded={open?.index === i}
        data-index={i}
        class="submenu"
        onpointerenter={(event) => openSubmenu(i, event.currentTarget)}
        onclick={(event) => pick(item, i, event.currentTarget)}>{item.label}<span aria-hidden="true">▸</span></button
      >
    {:else}
      <button
        type="button"
        role="menuitem"
        title={item.title}
        data-index={i}
        onpointerenter={() => (open = null)}
        onclick={(event) => pick(item, i, event.currentTarget)}>{item.label}</button
      >
    {/if}
  {/each}
  {#if open}
    {@const sub = items[open.index]}
    {#if sub !== "separator" && "items" in sub}
      <ContextMenu
        x={open.x}
        y={open.y}
        items={sub.items}
        label={sub.label}
        onclose={() => {
          const index = open?.index;
          open = null;
          if (index !== undefined) (buttons().find((b) => Number(b.dataset.index) === index) ?? null)?.focus();
        }}
        ondone={done}
        flip={open.flip}
      />
    {/if}
  {/if}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 100;
    min-width: 180px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel-alt);
    box-shadow: 0 6px 20px rgb(0 0 0 / 45%);
  }
  button {
    display: flex;
    justify-content: space-between;
    width: 100%;
    padding: 4px 10px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
  }
  button:disabled {
    color: var(--text-dim);
  }
  button:not(:disabled):hover,
  button:not(:disabled):focus {
    background: var(--accent);
    color: #fff;
    outline: none;
  }
  .separator {
    height: 1px;
    margin: 4px 2px;
    background: var(--border);
  }
</style>
