<script lang="ts">
  import { onDestroy, onMount, type Snippet } from "svelte";
  import { rememberFocus, trapTab } from "../a11y/focus";
  import { dialogOpened } from "../shell/editing";

  // A modal dialog ("HTML with labelled fields and focus trapping"): a `<dialog>`
  // shown with `showModal()`, so the rest of the window is inert while it is open; Escape
  // closes it (`onclose`). The window's single-key shortcuts wait while it is open (App.svelte),
  // and so does the native menu (the page reports it, shell/editing.ts). Tab cycles
  // inside it, and the focus returns where it was when it closes (a11y/focus.ts).
  let {
    id,
    title,
    width = 460,
    onclose,
    onsubmit,
    children,
    buttons,
  }: {
    id: string;
    title: string;
    width?: number;
    onclose: () => void;
    onsubmit?: () => void;
    children: Snippet;
    buttons: Snippet;
  } = $props();

  let dialog: HTMLDialogElement | undefined = $state();

  $effect(() => {
    if (dialog && !dialog.open) dialog.showModal();
  });

  onMount(() => dialogOpened());
  // Taken before the dialog takes the focus.
  onDestroy(rememberFocus());
</script>

<dialog
  bind:this={dialog}
  use:trapTab
  aria-labelledby="{id}-title"
  style:width="{width}px"
  oncancel={(event) => {
    event.preventDefault();
    onclose();
  }}
>
  <form
    method="dialog"
    onsubmit={(event) => {
      event.preventDefault();
      onsubmit?.();
    }}
  >
    <h2 id="{id}-title">{title}</h2>
    <div class="body">{@render children()}</div>
    <div class="buttons">{@render buttons()}</div>
  </form>
</dialog>

<style>
  dialog {
    max-width: calc(100vw - 32px);
    max-height: calc(100vh - 32px);
    padding: 14px 16px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
    color: var(--text);
    -webkit-user-select: text;
    user-select: text;
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 40%);
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  h2 {
    margin: 0;
    font-size: 15px;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 10px;
    overflow: auto;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .body :global(fieldset) {
    margin: 0;
    padding: 6px 10px 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
  }
  .body :global(legend) {
    padding: 0 4px;
    color: var(--text-dim);
  }
  .body :global(.form) {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 6px 10px;
    align-items: center;
  }
  .body :global(.wide) {
    grid-column: 1 / -1;
  }
  .body :global(.note) {
    margin: 0;
    color: var(--text-dim);
    font-size: 12px;
  }
  .body :global(.error) {
    margin: 0;
    color: var(--error);
  }
  .body :global(input[type="text"]),
  .body :global(input[type="number"]),
  .body :global(textarea),
  .body :global(select) {
    min-width: 0;
    font: inherit;
  }
  .body :global(textarea) {
    resize: vertical;
  }
  .buttons :global(button) {
    min-width: 76px;
    padding: 4px 12px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--panel-alt);
    color: var(--text);
    font: inherit;
  }
  .buttons :global(button[type="submit"]) {
    background: var(--accent);
    color: #fff;
  }
</style>
