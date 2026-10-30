<script lang="ts">
  import type { ProviderId, ProviderStatus } from "./api";
  import { availabilityText, providerSelectable } from "./model";

  // Which agent answers: the providers detection found, one radio each. A provider that is not
  // ready cannot be chosen, and once the thread has a task the choice is locked (the thread
  // continues with one provider). `providers` is null until detection has answered.
  let {
    providers,
    selected,
    locked,
    checking,
    onselect,
    oncheck,
  }: {
    providers: readonly ProviderStatus[] | null;
    selected: ProviderId | null;
    locked: boolean;
    checking: boolean;
    onselect: (id: ProviderId) => Promise<boolean>;
    oncheck: () => void;
  } = $props();

  async function change(event: Event, id: ProviderId) {
    const input = event.currentTarget as HTMLInputElement;
    // The backend decides: when it refuses, the radio goes back to the thread's provider.
    if (!(await onselect(id))) input.checked = selected === id;
  }
</script>

<fieldset class="picker">
  <legend>Provider</legend>
  {#if providers === null}
    <p class="note" role="status">Looking for providers…</p>
  {:else}
    <ul>
      {#each providers as provider (provider.id)}
        {@const selectable = providerSelectable(provider, locked)}
        <li data-availability={provider.availability}>
          <label class:off={!selectable}>
            <input
              type="radio"
              name="chat-provider"
              value={provider.id}
              checked={selected === provider.id}
              disabled={!selectable}
              aria-describedby="provider-{provider.id}-note"
              onchange={(event) => change(event, provider.id)}
            />
            <span class="name">{provider.name}</span>
            {#if provider.version}<span class="version">{provider.version}</span>{/if}
            <span class="status" class:bad={provider.availability !== "ready"}>{availabilityText(provider)}</span>
          </label>
          <p class="note" id="provider-{provider.id}-note">{provider.hint}</p>
        </li>
      {/each}
    </ul>
  {/if}
  <div class="foot">
    {#if locked}
      <span class="note">The provider stays the same for a conversation. Clear the history to choose another.</span>
    {/if}
    <button type="button" onclick={oncheck} disabled={checking}>{checking ? "Checking…" : "Check again"}</button>
  </div>
</fieldset>

<style>
  .picker {
    margin: 0;
    padding: 4px 10px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    min-width: 0;
  }
  legend {
    padding: 0 4px;
    color: var(--text-dim);
    font-size: 12px;
  }
  ul {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  label {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  label.off {
    color: var(--text-dim);
  }
  input {
    accent-color: var(--accent);
  }
  .name {
    font-weight: 600;
  }
  .version,
  .note {
    color: var(--text-dim);
    font-size: 12px;
  }
  .status {
    margin-left: auto;
    font-size: 12px;
  }
  .status.bad {
    font-weight: 600;
  }
  .note {
    margin: 0 0 0 24px;
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-top: 6px;
  }
  .foot .note {
    margin: 0;
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
    opacity: 0.6;
    cursor: default;
  }
</style>
