<script lang="ts">
  import { onMount } from "svelte";
  import SpinField from "../inspector/SpinField.svelte";
  import { createProject, newProjectView, type FieldError, type NewProjectForm, type NewProjectView } from "./api";
  import Dialog from "./Dialog.svelte";
  import type { SpinSpec } from "./fields";

  // New Project (the prototype's `NewProjectDialog`): name, template (its
  // description as the tooltip), tempo, an editable time signature and the sample rate. OK sends
  // the choices; sonara-app checks them, then the unsaved-changes prompt runs and the project
  // replaces the open one. G3, fixed: a wrong field is named and the dialog stays
  // open, where the prototype failed silently.
  let { onclose }: { onclose: () => void } = $props();

  let view = $state<NewProjectView | null>(null);
  let form = $state<NewProjectForm | null>(null);
  let error = $state<FieldError | null>(null);
  let busy = $state(false);

  const tempo = $derived<SpinSpec>({ min: view?.bpm[0] ?? 10, max: view?.bpm[1] ?? 999, step: 1, decimals: 2 });

  onMount(() => {
    newProjectView()
      .then((loaded) => {
        view = loaded;
        form = { ...loaded.form };
      })
      .catch((failure: unknown) => (error = { field: "form", code: "INTERNAL", message: String(failure) }));
  });

  async function submit() {
    if (!form || busy) return;
    busy = true;
    error = null;
    try {
      await createProject(form);
      onclose();
    } catch (failure) {
      error = failure as FieldError;
      document.getElementById(`new-project-${error.field.replace("_", "-")}`)?.focus();
    } finally {
      busy = false;
    }
  }

  const invalid = (field: FieldError["field"]) => error?.field === field;
</script>

<Dialog id="new-project" title="New Project" onclose={onclose} onsubmit={submit}>
  {#if form && view}
    <div class="form">
      <label for="new-project-name">Name</label>
      <input id="new-project-name" type="text" bind:value={form.name} aria-invalid={invalid("name")} />
      <label for="new-project-template">Template</label>
      <select id="new-project-template" bind:value={form.template} aria-invalid={invalid("template")}>
        {#each view.templates as template (template.id)}
          <option value={template.id} title={template.description}>{template.name}</option>
        {/each}
      </select>
      <SpinField
        id="new-project-bpm"
        label="Tempo (BPM)"
        value={form.bpm}
        spec={tempo}
        onchange={(bpm) => form && (form.bpm = bpm)}
      />
      <label for="new-project-time-signature">Time signature</label>
      <input
        id="new-project-time-signature"
        type="text"
        list="new-project-signatures"
        bind:value={form.time_signature}
        aria-invalid={invalid("time_signature")}
        aria-describedby={error ? "new-project-error" : undefined}
      />
      <datalist id="new-project-signatures">
        {#each view.signatures as signature (signature)}<option value={signature}></option>{/each}
      </datalist>
      <label for="new-project-sample-rate">Sample rate</label>
      <select id="new-project-sample-rate" bind:value={form.sample_rate} aria-invalid={invalid("sample_rate")}>
        {#each view.rates as [label, rate] (rate)}<option value={rate}>{label}</option>{/each}
      </select>
    </div>
  {/if}
  {#if error}
    <p class="error" id="new-project-error" role="alert">{error.message}</p>
  {/if}
  {#snippet buttons()}
    <button type="button" onclick={onclose}>Cancel</button>
    <button type="submit" disabled={!form || busy}>OK</button>
  {/snippet}
</Dialog>
