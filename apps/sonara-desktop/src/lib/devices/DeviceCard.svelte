<script lang="ts">
  import * as backend from "../backend";
  import type { GestureDeps } from "../gesture";
  import { effectMenu } from "../mixer/strip";
  import Toggle from "../mixer/Toggle.svelte";
  import type { Session } from "../session.svelte";
  import ContextMenu from "../shell/ContextMenu.svelte";
  import type { MenuItem } from "../shell/menu";
  import type {
    DeviceView,
    Edit,
    EditOutcome,
    LibraryChoices,
    OpenProject,
    ParamValue,
    PluginGroup,
    PluginView,
    TrackView,
  } from "../types";
  import { samplerInstruments, samplerPresetOf } from "./catalog";
  import EqCurve from "./EqCurve.svelte";
  import {
    DYNAMICS,
    EQ_PLUGIN,
    SAMPLER_INSTRUMENT,
    SAMPLER_PLUGIN,
    gainReductionOf,
    gainReductionText,
    groupParams,
    groupRows,
    instrumentCombo,
    missingText,
    paramValue,
    pluginLabel,
    presetIndex,
    presetItems,
    shortLabel,
    sidechainOptions,
  } from "./model";
  import ParamControl from "./ParamControl.svelte";

  // One device's card (the prototype's `DeviceCard`): the On/Off
  // toggle, the name and the plugin's name when the device was renamed, the live gain reduction of
  // a dynamics plugin, the preset combo, the ⋯ menu (Replace Instrument or Remove, and Reset to
  // Defaults); the warning of a plugin this build lacks; the sidechain source of a plugin that takes
  // one; the parametric EQ's curve; and the parameters, grouped, each with its control. On a
  // sampler's card, and only there, the instrument combo asks sonara-app for the presets' names by
  // library and shows the preset the track plays. The card is keyed by its device, so a parameter
  // edit, from here, an agent or undo, updates it in place; a press on it (outside its controls)
  // makes its device the current one, outlined.

  let {
    session,
    project,
    track,
    device,
    plugin,
    instrument,
    instruments,
    selected,
  }: {
    session: Session;
    project: OpenProject;
    track: TrackView;
    device: DeviceView;
    /** The plugin's descriptor; null when this build lacks the plugin. */
    plugin: PluginView | null;
    /** The device is the track's instrument. */
    instrument: boolean;
    /** The instrument picker of Replace Instrument. */
    instruments: readonly PluginGroup[];
    /** The device is the current one. */
    selected: boolean;
  } = $props();

  const deps = (): GestureDeps => ({
    send: backend.edit,
    seal: backend.sealUndo,
    nextFrame: (callback) => requestAnimationFrame(() => callback()),
    onError: (error) => session.fail(error),
  });

  const groups = $derived(plugin ? groupParams(plugin.params) : []);
  const dynamics = $derived(plugin?.category === DYNAMICS);
  const options = $derived(plugin?.sidechain ? sidechainOptions(project, track) : []);
  const sidechain = $derived(options.some((o) => o.id === device.sidechain_id) ? (device.sidechain_id ?? "") : "");

  /** The sampler's `instrument` parameter; undefined on any other card. */
  const instrumentParam = $derived(
    plugin?.id === SAMPLER_PLUGIN ? plugin.params.find((p) => p.id === SAMPLER_INSTRUMENT && p.kind === "enum") : undefined,
  );
  const storedInstrument = $derived(instrumentParam ? String(paramValue(device, instrumentParam)) : null);
  /** The sampler's choices by library, once sonara-app has answered. */
  let libraries = $state<readonly LibraryChoices[] | null>(null);
  /** The preset sonara-app reads a stored value that is not a choice as (an older project's path). */
  let readAs = $state<{ value: string; preset: string | null } | null>(null);
  const instrumentChoices = $derived(
    instrumentParam && libraries && storedInstrument !== null
      ? instrumentCombo(
          instrumentParam.choices,
          libraries,
          storedInstrument,
          readAs?.value === storedInstrument ? readAs.preset : undefined,
        )
      : null,
  );

  $effect(() => {
    if (!instrumentParam) return;
    samplerInstruments()
      .then((choices) => (libraries = choices))
      .catch((error: unknown) => session.fail(error));
  });

  $effect(() => {
    const value = storedInstrument;
    if (value === null || !instrumentParam || instrumentParam.choices.includes(value)) return;
    samplerPresetOf(value)
      .then((preset) => (readAs = { value, preset }))
      .catch((error: unknown) => session.fail(error));
  });

  let menu: { x: number; y: number; items: MenuItem[] } | null = $state(null);
  /** The readout's text; a frame without a value for the device keeps the last one. */
  let gainReduction = $state(gainReductionText(null));

  $effect(() => {
    if (!dynamics) return;
    const gr = gainReductionOf(session.frame, project, device.id);
    if (gr !== null) gainReduction = gainReductionText(gr);
  });

  async function run(edit: Edit): Promise<EditOutcome | null> {
    try {
      return await backend.edit(edit);
    } catch (error) {
      session.fail(error);
      return null;
    }
  }

  async function seal() {
    try {
      await backend.sealUndo();
    } catch (error) {
      session.fail(error);
    }
  }

  /** One step: the edit, then the seal. */
  async function step(edit: Edit): Promise<boolean> {
    const outcome = await run(edit);
    await seal();
    return outcome !== null;
  }

  function setParam(id: string, value: ParamValue): Promise<boolean> {
    return step({ op: "set_device_params", device_id: device.id, values: { [id]: value } });
  }

  async function choosePreset(event: Event) {
    const select = event.currentTarget as HTMLSelectElement;
    const index = select.selectedIndex;
    if (index <= 0 || !plugin) return;
    if (!(await step({ op: "apply_preset", device_id: device.id, preset: presetItems(plugin)[index] }))) {
      select.selectedIndex = presetIndex(plugin, device.preset);
    }
  }

  async function chooseSidechain(event: Event) {
    const select = event.currentTarget as HTMLSelectElement;
    // One step "Set Sidechain", which also turns the plugin's `sidechain` switch on.
    const outcome = await run({ op: "set_sidechain", device_id: device.id, source_track_id: select.value || null });
    if (!outcome) select.value = sidechain;
  }

  function showMenu(event: MouseEvent) {
    event.stopPropagation();
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const items: MenuItem[] = [];
    if (instrument) {
      items.push(
        instruments.length > 0
          ? {
              label: "Replace Instrument",
              items: effectMenu(instruments, (pluginId) => void run({ op: "set_instrument", track_id: track.id, plugin_id: pluginId })),
            }
          : { label: "Replace Instrument", disabled: true },
      );
    } else {
      items.push({ label: "Remove", run: () => void run({ op: "remove_device", device_id: device.id }) });
    }
    if (plugin) items.push({ label: "Reset to Defaults", run: () => void step({ op: "reset_device", device_id: device.id }) });
    menu = { x: box.left, y: box.bottom, items };
  }

  /** Whether a press on `target` belongs to one of the card's controls rather than the card. */
  function onControl(target: EventTarget | null): boolean {
    return target instanceof Element && target.closest("button, select, input, [role=slider]") !== null;
  }

  function cardDown(event: PointerEvent) {
    if (event.button !== 0 || onControl(event.target)) return;
    session.setCurrentDevice(device.id);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="card"
  class:selected
  role="group"
  aria-label="Device {device.name}"
  data-device={device.id}
  onpointerdown={cardDown}
>
  <div class="head">
    <Toggle
      label={device.enabled ? "On" : "Off"}
      pressed={device.enabled}
      color="solo"
      title="Enable / bypass"
      width={28}
      onclick={() => void step({ op: "set_device_enabled", device_id: device.id, enabled: !device.enabled })}
    />
    <span class="name">{device.name}</span>
    <span class="plugin">{pluginLabel(device, plugin)}</span>
    <span class="spacer"></span>
    {#if dynamics}
      <span class="gr" title="Gain reduction (live)">{gainReduction}</span>
    {/if}
    {#if plugin && plugin.presets.length > 0}
      <select class="preset" aria-label="Preset" title="Load a preset" value={presetItems(plugin)[presetIndex(plugin, device.preset)]} onchange={choosePreset}>
        {#each presetItems(plugin) as item, i (i)}
          <option value={item}>{item}</option>
        {/each}
      </select>
    {/if}
    <button type="button" class="actions" title="Device actions" aria-label="Device actions" onclick={showMenu}>⋯</button>
  </div>

  {#if !plugin}
    <p class="missing">{missingText(device)}</p>
  {:else}
    {#if plugin.sidechain}
      <div class="sidechain">
        <span>Sidechain</span>
        <select aria-label="Sidechain source" title="Track keying this device's detector" value={sidechain} onchange={chooseSidechain}>
          {#each options as option (option.id)}
            <option value={option.id}>{option.name}</option>
          {/each}
        </select>
      </div>
    {/if}
    <div class="body">
      {#if plugin.id === EQ_PLUGIN}
        <EqCurve deviceId={device.id} params={device.params} onerror={(error) => session.fail(error)} />
      {/if}
      {#each groups as group (group.key)}
        <fieldset class="group">
          <legend>{group.title}</legend>
          <div class="grid" style:grid-template-rows="repeat({groupRows(group)}, auto)">
            {#each group.params as param (param.id)}
              <ParamControl
                {param}
                value={paramValue(device, param)}
                label={shortLabel(param, group.title)}
                deviceId={device.id}
                {deps}
                set={(value) => setParam(param.id, value)}
                grouped={param === instrumentParam ? instrumentChoices : null}
              />
            {/each}
          </div>
        </fieldset>
      {/each}
    </div>
  {/if}
</div>
{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} label="Device actions" onclose={() => (menu = null)} />
{/if}

<style>
  .card {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    align-self: stretch;
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--panel);
  }
  .card.selected {
    border: 1.5px solid var(--accent);
    background: var(--panel-alt);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
  }
  .name {
    font-weight: 700;
  }
  .plugin,
  .sidechain span {
    color: var(--text-dim);
    font-size: 11px;
  }
  .spacer {
    flex: 1;
    min-width: 8px;
  }
  .gr {
    font-family: Menlo, monospace;
    font-size: 11px;
    white-space: pre;
  }
  select {
    padding: 1px 2px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--panel-alt);
    color: var(--text);
    font: inherit;
    font-size: 11px;
  }
  .preset {
    min-width: 120px;
  }
  .actions {
    padding: 0 6px;
    border: 0;
    border-radius: 3px;
    background: transparent;
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .actions:hover {
    background: var(--panel-alt);
  }
  .missing {
    max-width: 320px;
    margin: 0;
    color: var(--meter-red);
    white-space: normal;
  }
  .sidechain {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .sidechain select {
    flex: 1;
  }
  .body {
    display: flex;
    align-items: flex-start;
    gap: 6px;
  }
  .group {
    flex: none;
    margin: 0;
    padding: 8px 4px 2px;
    border: 1px solid var(--border);
    border-radius: 4px;
  }
  .group legend {
    padding: 0 3px;
    color: var(--text-dim);
    font-size: 11px;
  }
  .grid {
    display: grid;
    grid-auto-flow: column;
    gap: 2px;
  }
</style>
