<script lang="ts">
  import { onMount } from "svelte";
  import Mixer from "./lib/mixer/Mixer.svelte";
  import { Session } from "./lib/session.svelte";
  import { installFieldUndo, transportShortcut } from "./lib/shell/shortcuts";
  import type { StatusMessage } from "./lib/types";

  // The mixer's own window: the mixer of the Editors pane on a page of its own, with a
  // session of its own (its view, its live frames, its meters) that shares the selection with the
  // main window. The Devices tab and the transport's single keys are the main window's: this page
  // asks it for them. Closing the window puts the mixer back into its tab.
  let message: StatusMessage | null = $state(null);
  let clearTimer: ReturnType<typeof setTimeout> | undefined;

  function show(next: StatusMessage) {
    message = next;
    clearTimeout(clearTimer);
    clearTimer = setTimeout(() => (message = null), next.duration_ms);
  }

  const session = new Session(show, "mixer");

  function shortcut(event: KeyboardEvent) {
    const action = transportShortcut(event);
    if (!action) return;
    event.preventDefault();
    session.shortcut(action);
  }

  onMount(() => {
    const stop = session.start();
    window.addEventListener("keydown", shortcut);
    // Step 11: the typed dB's ⌘Z and ⇧⌘Z undo its typing, as in the main window.
    const stopFieldUndo = installFieldUndo(window, (command) => document.execCommand(command));
    return () => {
      clearTimeout(clearTimer);
      stop();
      window.removeEventListener("keydown", shortcut);
      stopFieldUndo();
    };
  });
</script>

<Mixer {session} />
<footer class="status" class:error={message?.error}>
  <span role="status" aria-live="polite">{message && !message.error ? message.text : ""}</span>
  <span role="alert">{message?.error ? message.text : ""}</span>
</footer>

<style>
  .status {
    flex: none;
    height: 22px;
    padding: 3px 8px;
    border-top: 1px solid var(--border);
    background: var(--bar);
    color: var(--text-dim);
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .status.error {
    color: var(--error);
  }
</style>
