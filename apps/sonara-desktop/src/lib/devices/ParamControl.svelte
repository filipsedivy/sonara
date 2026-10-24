<script lang="ts">
  import type { GestureDeps } from "../gesture";
  import Knob from "../mixer/Knob.svelte";
  import Toggle from "../mixer/Toggle.svelte";
  import type { Edit, ParamValue, ParamView } from "../types";
  import { PARAM_KNOB, comboChoice, controlOf, knobValue, toggleText, tooltipOf } from "./model";

  // The editor of one parameter (the prototype's `ParamWidget`): a knob for
  // a float or an int, with the short label above and the value below; an On/Off toggle or a combo
  // under the label for a bool or a choice. A knob sends its edit at every move of its gesture and
  // seals the undo step at its end (`ValueDriver`); a toggle or a choice sends one edit and
  // seals it. The model's value is shown again when it changes, from here, an agent or undo,
  // and a knob the user drives keeps its own value until the view has it.

  let {
    param,
    value,
    label,
    deviceId,
    deps,
    set,
  }: {
    param: ParamView;
    /** The model's value (resolved against the defaults). */
    value: ParamValue;
    /** The short label (`short_label`). */
    label: string;
    deviceId: string;
    deps: () => GestureDeps;
    /** Sends one edit of this parameter, then seals the undo step (a toggle, a choice). */
    set: (value: ParamValue) => Promise<boolean>;
  } = $props();

  const control = $derived(controlOf(param));
  const tooltip = $derived(tooltipOf(param));

  function knobEdit(v: number): Edit {
    return { op: "set_device_params", device_id: deviceId, values: { [param.id]: knobValue(control, v) } };
  }

  async function choose(event: Event) {
    const select = event.currentTarget as HTMLSelectElement;
    const choice = select.value;
    // A refused choice leaves the model's value, which the combo shows again.
    if (!(await set(choice)) && control.kind === "combo") select.value = comboChoice(control.choices, value);
  }
</script>

<div class="param" data-param={param.id}>
  {#if control.kind === "knob"}
    <div class="knob" title={tooltip}>
      <Knob
        value={Number(value)}
        spec={control.spec}
        name={param.name}
        format={control.format}
        title={label}
        bipolar={control.bipolar}
        diameter={PARAM_KNOB.diameter}
        minWidth={PARAM_KNOB.minWidth}
        {deps}
        edit={knobEdit}
      />
    </div>
  {:else}
    <span class="label" title={tooltip}>{label}</span>
    {#if control.kind === "toggle"}
      <Toggle
        label={toggleText(Boolean(value))}
        pressed={Boolean(value)}
        title={param.name}
        width={40}
        onclick={() => void set(!value)}
      />
    {:else}
      <select aria-label={param.name} title={tooltip} value={comboChoice(control.choices, value)} onchange={choose}>
        {#each control.choices as choice (choice)}
          <option value={choice}>{choice}</option>
        {/each}
      </select>
    {/if}
  {/if}
</div>

<style>
  .param {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1px;
    min-width: 66px;
  }
  .label {
    width: 100%;
    overflow: hidden;
    color: var(--text-dim);
    font-size: 10px;
    line-height: 14px;
    text-align: center;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  select {
    max-width: 96px;
    padding: 1px 2px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--panel-alt);
    color: var(--text);
    font: inherit;
    font-size: 10px;
  }
</style>
