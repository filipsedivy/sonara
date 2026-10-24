<script lang="ts">
  import type { Session } from "../session.svelte";
  import ChannelStrip from "./ChannelStrip.svelte";
  import { mixedTracks } from "./strip";

  // The mixer (the prototype's `MixerPanel`): one channel strip per track that is
  // not a folder, in track order, scrolling horizontally, and the master pinned on the right,
  // which scrolls vertically with the others so that a short pane squashes no strip. The strips
  // are keyed by track: they are rebuilt only when the mixed tracks change, and every other change,
  // an agent's included, refreshes them in place. It shows in the Editors pane's Mixer tab and in
  // the mixer's own window.
  let { session }: { session: Session } = $props();

  const project = $derived(session.view?.project ?? null);
  const tracks = $derived(project ? mixedTracks(project) : []);

  let strips: HTMLDivElement | undefined = $state();
  let masterBox: HTMLDivElement | undefined = $state();

  function scrolled() {
    if (strips && masterBox) masterBox.scrollTop = strips.scrollTop;
  }

  function wheelMaster(event: WheelEvent) {
    if (!strips) return;
    strips.scrollTop += event.deltaY;
  }
</script>

<section class="mixer" aria-label="Mixer">
  {#if project}
    <div class="strips" bind:this={strips} onscroll={scrolled}>
      {#if tracks.length === 0}
        <p class="empty">No tracks yet — add a track to start mixing.</p>
      {/if}
      {#each tracks as track (track.id)}
        <ChannelStrip {session} {track} {project} />
      {/each}
    </div>
    <div class="separator"></div>
    <div class="master" bind:this={masterBox} onwheel={wheelMaster}>
      {#key project.master.id}
        <ChannelStrip {session} track={project.master} {project} master />
      {/key}
    </div>
  {/if}
</section>

<style>
  .mixer {
    flex: 1;
    display: flex;
    min-width: 0;
    min-height: 0;
    height: 100%;
    background: var(--window);
  }
  .strips {
    flex: 1;
    display: flex;
    gap: 3px;
    min-width: 0;
    padding: 4px;
    overflow: auto;
  }
  .strips > :global(.strip),
  .master > :global(.strip) {
    min-height: max(100%, 520px);
  }
  .empty {
    margin: 8px;
    color: var(--text-dim);
  }
  .separator {
    flex: none;
    width: 1px;
    background: var(--border);
  }
  .master {
    flex: none;
    display: flex;
    width: 128px;
    padding: 4px;
    overflow: hidden;
  }
</style>
