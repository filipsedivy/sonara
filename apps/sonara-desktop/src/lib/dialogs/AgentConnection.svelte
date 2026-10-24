<script lang="ts">
  import { onMount } from "svelte";
  import { agentConnection, errorMessage, type AgentConnection } from "./api";
  import Dialog from "./Dialog.svelte";

  // Agent Connection (the prototype's `AgentInfoDialog`): the IPC address or that the
  // server is off, how to register the bundled `sonara mcp` with Claude Code
  // (`Sonara.app/Contents/MacOS/sonara`), and the discovery file.
  let { onclose }: { onclose: () => void } = $props();

  let shown = $state<AgentConnection | null>(null);
  let error = $state<string | null>(null);

  onMount(() => {
    agentConnection()
      .then((loaded) => (shown = loaded))
      .catch((failure: unknown) => (error = errorMessage(failure)));
  });
</script>

<Dialog id="agent-connection" title="Agent Connection" width={640} onclose={onclose} onsubmit={onclose}>
  {#if shown}
    <p class="status">
      {#if shown.address}
        This window accepts agent connections at <b>{shown.address}</b>.
      {:else}
        {shown.status}
      {/if}
    </p>
    <label for="agent-connection-text" class="note">How to connect</label>
    <textarea id="agent-connection-text" readonly rows="10" value={shown.text}></textarea>
  {/if}
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
  {#snippet buttons()}
    <button type="submit">Close</button>
  {/snippet}
</Dialog>

<style>
  .status {
    margin: 0;
  }
  textarea {
    font-family: Menlo, monospace;
    font-size: 12px;
  }
</style>
