<script lang="ts">
  import { onMount } from "svelte";
  import BetaBadge from "./BetaBadge.svelte";
  import { chat } from "./chat.svelte";
  import ChatInput from "./ChatInput.svelte";
  import ClearDialog from "./ClearDialog.svelte";
  import { canClear, clearNeedsConfirm, composerEnabled, phase, queuedTasks, runningTask, selectedProvider } from "./model";
  import ProviderPicker from "./ProviderPicker.svelte";
  import QueueList from "./QueueList.svelte";
  import Thread from "./Thread.svelte";

  // The AI Chat's own window (webview label "chat"). The chat lives in Rust: this window follows
  // its events, so a reloaded window shows the same thread, and closing it wipes the thread.
  const view = $derived(chat.view);
  const thread = $derived(view.state);
  const provider = $derived(selectedProvider(view));

  let confirming = $state(false);
  let input: ChatInput | undefined = $state();

  const status = $derived.by(() => {
    if (thread === null) return "Loading…";
    const waiting = queuedTasks(thread).length;
    const queue = waiting > 0 ? `, ${waiting} queued` : "";
    switch (phase(thread)) {
      case "running":
        return `Working${queue}`;
      case "paused":
        return `Queue paused${queue}`;
      case "idle":
        return thread.tasks.length === 0 ? "Ready" : "Idle";
    }
  });

  const hint = $derived.by(() => {
    if (thread === null) return "Loading…";
    if (view.providers === null) return "Looking for providers…";
    if (provider?.availability !== "ready") {
      return `${provider?.name ?? "The provider"} is not available: choose a provider that is ready.`;
    }
    return null;
  });

  function clearClicked() {
    if (thread === null) return;
    if (clearNeedsConfirm(thread)) confirming = true;
    else void clear();
  }

  async function clear() {
    confirming = false;
    await chat.clear();
    input?.focus();
  }

  onMount(() => {
    document.title = "AI Chat (Beta)";
    let stop: (() => void) | undefined;
    let unmounted = false;
    void chat.init().then((unlisten) => {
      if (unmounted) unlisten();
      else stop = unlisten;
    });
    return () => {
      unmounted = true;
      stop?.();
    };
  });
</script>

<main class="chat">
  <header>
    <h1>AI Chat</h1>
    <BetaBadge />
    <span class="status" role="status" aria-live="polite" aria-atomic="true">{status}</span>
    <button type="button" onclick={clearClicked} disabled={thread === null || !canClear(thread)}>Clear history</button>
  </header>

  {#if chat.error}
    <p class="problem" role="alert">
      <span>{chat.error.message}</span>
      <button type="button" aria-label="Dismiss the error" onclick={() => chat.dismissError()}>×</button>
    </p>
  {/if}

  <ProviderPicker
    providers={view.providers}
    selected={thread?.provider ?? null}
    locked={thread?.provider_locked ?? false}
    checking={chat.checking}
    onselect={(id) => chat.setProvider(id)}
    oncheck={() => void chat.refreshProviders()}
  />

  <Thread {view} />

  {#if thread}
    <QueueList
      {thread}
      onedit={(id, prompt) => chat.edit(id, prompt)}
      onremove={(id) => chat.remove(id)}
      onmove={(id, direction) => chat.move(id, direction)}
      onstop={() => void chat.stop()}
      onresume={() => void chat.resume()}
    />
  {/if}

  <ChatInput bind:this={input} enabled={composerEnabled(view)} {hint} onsend={(prompt) => chat.send(prompt)} />
</main>

{#if confirming && thread}
  <ClearDialog
    running={runningTask(thread) !== null}
    queued={queuedTasks(thread).length}
    onconfirm={() => void clear()}
    oncancel={() => (confirming = false)}
  />
{/if}

<style>
  .chat {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
    padding: 10px 12px;
    background: var(--window);
    color: var(--text);
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  h1 {
    margin: 0;
    font-size: 15px;
  }
  .status {
    flex: 1;
    color: var(--text-dim);
    font-size: 12px;
    text-align: right;
  }
  .problem {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 0;
    padding: 4px 8px;
    border-left: 3px solid var(--error);
    background: var(--panel);
    font-size: 12px;
    -webkit-user-select: text;
    user-select: text;
  }
  button {
    padding: 3px 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--panel-alt);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
