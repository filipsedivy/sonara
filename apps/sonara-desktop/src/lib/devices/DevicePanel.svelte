<script lang="ts">
  import { onMount, tick } from "svelte";
  import * as backend from "../backend";
  import { effectMenu } from "../mixer/strip";
  import type { Session } from "../session.svelte";
  import ContextMenu from "../shell/ContextMenu.svelte";
  import type { MenuItem } from "../shell/menu";
  import type { PluginGroup } from "../types";
  import DeviceCard from "./DeviceCard.svelte";
  import { instrumentGroups } from "./catalog";
  import { canAddEffect, chainOf, emptyText, findTrack, headerText } from "./model";

  // The device editors (the prototype's `DevicePanel`): the chain of the track the
  // session's Devices focus names (the selected track, or the track of the current device)
  // as cards in a row, the instrument first, then the effects; the header "track — devices" with the
  // "+ Effect" menu grouped by category (DV-07: the new device becomes the current one); one of three
  // hints without cards. The cards are keyed by device, so they are made again only when the chain
  // changes, and every other change updates them in place. The current device's card is outlined
  // and scrolled into view.

  let { session }: { session: Session } = $props();

  const project = $derived(session.view?.project ?? null);
  const track = $derived(findTrack(project, session.devicesTrack));
  const devices = $derived(track && track.kind !== "folder" ? chainOf(track) : []);
  const hint = $derived(emptyText(track));

  let instruments: PluginGroup[] = $state([]);
  let menu: { x: number; y: number; items: MenuItem[] } | null = $state(null);
  let row: HTMLDivElement | undefined = $state();

  onMount(() => {
    instrumentGroups()
      .then((groups) => (instruments = groups))
      .catch((error: unknown) => session.fail(error));
  });

  // The card of a device that became the current one comes into view, once
  // (`currentDeviceChanged`): when the chain changes it is looked for again until its card appears
  // (a device just added), but a refresh never scrolls the row back to it.
  let scrolledTo: string | null = null;
  $effect(() => {
    const current = session.selection.currentDevice;
    void devices;
    if (!current) scrolledTo = null;
    if (!current || !row || scrolledTo === current) return;
    void tick().then(() => {
      const card = row?.querySelector(`[data-device="${CSS.escape(current)}"]`);
      if (!card || session.selection.currentDevice !== current) return;
      scrolledTo = current;
      card.scrollIntoView({ block: "nearest", inline: "nearest" });
    });
  });

  async function addEffect(pluginId: string) {
    const trackId = track?.id;
    if (!trackId) return;
    try {
      const outcome = await backend.edit({ op: "add_device", track_id: trackId, plugin_id: pluginId });
      const deviceId = outcome.result.device_id;
      if (outcome.committed && typeof deviceId === "string" && deviceId) session.setCurrentDevice(deviceId);
    } catch (error) {
      session.fail(error);
    }
  }

  function showEffects(event: MouseEvent) {
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    menu = { x: box.left, y: box.bottom, items: effectMenu(session.effectGroups, (id) => void addEffect(id)) };
  }
</script>

<section class="devices" aria-label="Devices" data-editor="devices">
  <header>
    <span class="title">{headerText(track)}</span>
    <button
      type="button"
      class="add"
      title="Add an effect at the end of the chain"
      disabled={!canAddEffect(track)}
      onclick={showEffects}>+ Effect</button
    >
  </header>
  <div class="cards" bind:this={row}>
    {#if hint}
      <p class="hint">{hint}</p>
    {/if}
    {#if project && track}
      {#each devices as device (`${device.id}:${device.plugin_id}`)}
        <DeviceCard
          {session}
          {project}
          {track}
          {device}
          plugin={project.plugins[device.plugin_id] ?? null}
          instrument={track.instrument?.id === device.id}
          {instruments}
          selected={session.selection.currentDevice === device.id}
        />
      {/each}
    {/if}
  </div>
</section>
{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} label="Add effect" onclose={() => (menu = null)} />
{/if}

<style>
  .devices {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    min-height: 0;
    padding: 4px 6px;
    background: var(--window);
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .title {
    flex: 1;
    font-weight: 700;
  }
  .add {
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--panel-alt);
    color: var(--text);
    font: inherit;
  }
  .add:disabled {
    color: var(--text-dim);
  }
  .cards {
    flex: 1;
    display: flex;
    align-items: stretch;
    gap: 6px;
    min-height: 0;
    overflow: auto;
  }
  .hint {
    margin: 4px 2px;
    color: var(--text-dim);
  }
</style>
