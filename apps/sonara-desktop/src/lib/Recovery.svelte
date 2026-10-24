<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { rememberFocus, trapTab } from "./a11y/focus";
  import { dialogOpened } from "./shell/editing";
  import { sessionLabel } from "./status";
  import type { RecoverableSession } from "./types";

  // The recovery offer of unsaved scratch sessions at startup (the prototype's
  // RecoveryDialog). Step 9 makes it the full DL-02 dialog.
  let {
    sessions,
    onrecover,
    ondismiss,
  }: {
    sessions: RecoverableSession[];
    onrecover: (workspace: string) => void;
    ondismiss: () => void;
  } = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  // The first session is selected until the user picks another.
  let chosen: string | null = $state(null);
  const selected = $derived(chosen ?? sessions[0]?.workspace ?? "");

  $effect(() => {
    if (dialog && !dialog.open) dialog.showModal();
  });

  // The menu waits while it is open, as for every dialog.
  onMount(() => dialogOpened());
  // Tab cycles inside it, and the focus returns where it was.
  onDestroy(rememberFocus());

  function recover(event: SubmitEvent) {
    event.preventDefault();
    if (selected) onrecover(selected);
  }
</script>

<dialog bind:this={dialog} use:trapTab aria-labelledby="recovery-title" oncancel={ondismiss}>
  <form onsubmit={recover}>
    <h2 id="recovery-title">Recover Unsaved Work</h2>
    <label for="recovery-list">Sonara found unsaved work from a previous session:</label>
    <select
      id="recovery-list"
      size={Math.min(Math.max(sessions.length, 2), 8)}
      onchange={(event) => (chosen = event.currentTarget.value)}
    >
      {#each sessions as session (session.workspace)}
        <option value={session.workspace} selected={session.workspace === selected}>
          {sessionLabel(session)}
        </option>
      {/each}
    </select>
    <div class="buttons">
      <button type="button" onclick={ondismiss}>Not Now</button>
      <button type="submit" disabled={!selected}>Recover</button>
    </div>
  </form>
</dialog>

<style>
  dialog {
    min-width: 420px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
    color: var(--text);
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
  select {
    width: 100%;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
