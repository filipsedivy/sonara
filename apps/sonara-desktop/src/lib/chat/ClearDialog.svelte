<script lang="ts">
  import { onDestroy } from "svelte";
  import { rememberFocus, trapTab } from "../a11y/focus";

  // The confirmation of Clear history, in the window (a `<dialog>`, so the rest is inert while it
  // is open). It is asked only when clearing would stop a running task or drop queued ones.
  // Cancel has the focus; Escape cancels; Tab stays inside; the focus returns afterwards.
  let {
    running,
    queued,
    onconfirm,
    oncancel,
  }: { running: boolean; queued: number; onconfirm: () => void; oncancel: () => void } = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  let cancelButton: HTMLButtonElement | undefined = $state();

  $effect(() => {
    if (dialog && !dialog.open) {
      dialog.showModal();
      cancelButton?.focus();
    }
  });

  // Taken before the dialog takes the focus.
  onDestroy(rememberFocus());

  const what = $derived(
    [running ? "stop the running task" : null, queued > 0 ? `drop ${queued === 1 ? "1 queued task" : `${queued} queued tasks`}` : null]
      .filter((part) => part !== null)
      .join(" and "),
  );
</script>

<dialog
  bind:this={dialog}
  use:trapTab
  aria-labelledby="clear-title"
  aria-describedby="clear-body"
  oncancel={(event) => {
    event.preventDefault();
    oncancel();
  }}
>
  <h2 id="clear-title">Clear the history?</h2>
  <p id="clear-body">
    This will {what}, and erase the conversation. Edits already made to the project stay; Undo steps back through them.
  </p>
  <div class="buttons">
    <button type="button" bind:this={cancelButton} onclick={oncancel}>Cancel</button>
    <button type="button" class="danger" onclick={onconfirm}>Clear history</button>
  </div>
</dialog>

<style>
  dialog {
    width: min(380px, calc(100vw - 32px));
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
  h2 {
    margin: 0 0 8px;
    font-size: 15px;
  }
  p {
    margin: 0 0 12px;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  button {
    min-width: 76px;
    padding: 4px 12px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--panel-alt);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .danger {
    border-color: var(--error);
  }
</style>
