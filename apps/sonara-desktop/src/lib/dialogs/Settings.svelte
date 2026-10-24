<script lang="ts">
  import { onMount } from "svelte";
  import SpinField from "../inspector/SpinField.svelte";
  import { applySettings, errorMessage, settingsView, type SettingsForm, type SettingsView } from "./api";
  import Dialog from "./Dialog.svelte";
  import type { SpinSpec } from "./fields";

  // Settings (the prototype's `SettingsDialog`): the engine, the
  // devices, the buffer and the lookahead; the autosave interval, the backups, the undo steps and
  // the theme; the agents' confirmation and allowed folders. OK saves the file through sonara-app,
  // whose bytes equal the prototype's for the same choices; the autosave interval applies at once,
  // the rest at the next start. G11, fixed: a stored value that is none of a
  // combo's choices is offered as it is, and kept.
  let { onclose }: { onclose: () => void } = $props();

  let view = $state<SettingsView | null>(null);
  let form = $state<SettingsForm | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  const spec = (range: [number, number] | undefined, decimals: number, suffix: string, special?: string): SpinSpec => ({
    min: range?.[0] ?? 0,
    max: range?.[1] ?? 0,
    step: 1,
    decimals,
    suffix,
    special,
  });

  onMount(() => {
    settingsView()
      .then((loaded) => {
        view = loaded;
        form = { ...loaded.form };
      })
      .catch((failure: unknown) => (error = errorMessage(failure)));
  });

  async function submit() {
    if (!form || busy) return;
    busy = true;
    error = null;
    try {
      await applySettings(form);
      onclose();
    } catch (failure) {
      error = errorMessage(failure);
    } finally {
      busy = false;
    }
  }

  /** The `<option>` value of a device id (`null` is the system default). */
  const deviceValue = (id: string | null) => id ?? "";
</script>

<Dialog id="settings" title="Settings" width={520} onclose={onclose} onsubmit={submit}>
  {#if form && view}
    <fieldset>
      <legend>Audio</legend>
      <div class="form">
        <label for="settings-backend">Engine</label>
        <select id="settings-backend" bind:value={form.backend}>
          {#each view.backends as choice (choice.value)}<option value={choice.value}>{choice.label}</option>{/each}
        </select>
        {#if view.devices_error}
          <p class="note wide" role="status">{view.devices_error}</p>
        {/if}
        <label for="settings-output">Output device</label>
        <select
          id="settings-output"
          value={deviceValue(form.output_device)}
          onchange={(event) => form && (form.output_device = event.currentTarget.value || null)}
        >
          {#each view.outputs as choice (deviceValue(choice.value))}
            <option value={deviceValue(choice.value)}>{choice.label}</option>
          {/each}
        </select>
        <label for="settings-input">Input device</label>
        <select
          id="settings-input"
          value={deviceValue(form.input_device)}
          onchange={(event) => form && (form.input_device = event.currentTarget.value || null)}
        >
          {#each view.inputs as choice (deviceValue(choice.value))}
            <option value={deviceValue(choice.value)}>{choice.label}</option>
          {/each}
        </select>
        <label for="settings-buffer">Device buffer</label>
        <select id="settings-buffer" bind:value={form.buffer_frames}>
          {#each view.buffers as choice (choice.value)}<option value={choice.value}>{choice.label}</option>{/each}
        </select>
        <SpinField
          id="settings-lookahead"
          label="Render lookahead"
          value={form.lookahead_ms}
          spec={spec(view.lookahead_ms, 2, " ms")}
          onchange={(value) => form && (form.lookahead_ms = value)}
        />
        <p class="note wide">Changes to the audio engine apply after restarting Sonara.</p>
      </div>
    </fieldset>
    <fieldset>
      <legend>Projects</legend>
      <div class="form">
        <SpinField
          id="settings-autosave"
          label="Autosave interval"
          value={form.autosave_seconds}
          spec={spec(view.autosave_seconds, 0, " s", "off")}
          onchange={(value) => form && (form.autosave_seconds = value)}
        />
        <SpinField
          id="settings-backups"
          label="Backups kept per project"
          value={form.backup_retention}
          spec={spec(view.backup_retention, 0, "")}
          onchange={(value) => form && (form.backup_retention = value)}
        />
        <SpinField
          id="settings-undo"
          label="Undo steps"
          value={form.undo_limit}
          spec={spec(view.undo_limit, 0, "")}
          onchange={(value) => form && (form.undo_limit = value)}
        />
        <label for="settings-theme">Theme</label>
        <select id="settings-theme" bind:value={form.theme}>
          {#each view.themes as choice (choice.value)}<option value={choice.value}>{choice.label}</option>{/each}
        </select>
      </div>
    </fieldset>
    <fieldset>
      <legend>Agent access</legend>
      <div class="form">
        <label class="wide check">
          <input type="checkbox" bind:checked={form.agent_require_confirmation} />
          Require confirmation for destructive agent actions
        </label>
        <label for="settings-paths">Allowed folders</label>
        <textarea
          id="settings-paths"
          rows="4"
          placeholder="Additional folders agents may read and write (one per line)"
          bind:value={form.agent_allowed_paths}
        ></textarea>
      </div>
    </fieldset>
  {/if}
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
  {#snippet buttons()}
    <button type="button" onclick={onclose}>Cancel</button>
    <button type="submit" disabled={!form || busy}>OK</button>
  {/snippet}
</Dialog>

<style>
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
  }
</style>
