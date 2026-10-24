<script lang="ts">
  import { onMount } from "svelte";
  import SpinField from "../inspector/SpinField.svelte";
  import {
    errorMessage,
    exportAudio,
    exportBrowse,
    exportPreset,
    exportView,
    type ExportForm,
    type ExportTail,
    type ExportView,
  } from "./api";
  import Dialog from "./Dialog.svelte";
  import type { SpinSpec } from "./fields";

  // Export Audio (the prototype's `ExportDialog`): the file without its extension
  // and Browse…, a preset, the formats this build writes, the PCM depth, the rate, the channels,
  // the lossy bitrate, dither; the range and the tail; the normalization; the stems. sonara-app
  // builds the outputs (FLAC at most 24 bits, float only in PCM containers, a `-container` suffix
  // for WAV-family files that would collide) and asks before replacing files, the stems' too.
  // The podcast preset sets 192 kbps.
  let { onclose }: { onclose: () => void } = $props();

  let view = $state<ExportView | null>(null);
  let form = $state<ExportForm | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  const DEPTHS: [ExportForm["bit_depth"], string][] = [
    ["16", "16-bit"],
    ["24", "24-bit"],
    ["32f", "32-bit float"],
  ];
  const RANGES: [ExportForm["range"], string][] = [
    ["song", "Entire song"],
    ["loop", "Loop region"],
  ];
  const TAILS: [ExportTail, string][] = [
    ["auto", "Auto (until silence)"],
    ["none", "None"],
    [2, "2 seconds"],
    [5, "5 seconds"],
  ];
  const NORMALIZE: [ExportForm["normalize"], string][] = [
    ["none", "None"],
    ["peak", "Peak"],
    ["true_peak", "True peak"],
    ["loudness", "Loudness (LUFS)"],
  ];

  const target = $derived<SpinSpec>({
    min: view?.target_db[0] ?? -60,
    max: view?.target_db[1] ?? 0,
    step: 1,
    decimals: 2,
    suffix: " dB / LUFS",
  });
  const ceiling = $derived<SpinSpec>({
    min: view?.ceiling_db[0] ?? -20,
    max: view?.ceiling_db[1] ?? 0,
    step: 1,
    decimals: 2,
    suffix: " dBTP",
  });

  onMount(() => {
    exportView()
      .then((loaded) => {
        view = loaded;
        form = { ...loaded.form, formats: [...loaded.form.formats], stem_tracks: [...loaded.form.stem_tracks] };
      })
      .catch((failure: unknown) => (error = errorMessage(failure)));
  });

  async function preset(name: string) {
    if (!form) return;
    try {
      form = await exportPreset(form, name);
    } catch (failure) {
      error = errorMessage(failure);
    }
  }

  async function browse() {
    if (!form) return;
    try {
      const path = await exportBrowse(form.path);
      if (path && form) form.path = path;
    } catch (failure) {
      error = errorMessage(failure);
    }
  }

  function toggle(list: string[], value: string, on: boolean, order: readonly string[]): string[] {
    const next = on ? [...list.filter((v) => v !== value), value] : list.filter((v) => v !== value);
    return order.filter((v) => next.includes(v));
  }

  async function submit() {
    if (!form || busy) return;
    busy = true;
    error = null;
    try {
      const done = await exportAudio(form);
      // Kept when the user would not replace the existing files: the path can change.
      if (done.started) onclose();
    } catch (failure) {
      error = errorMessage(failure);
    } finally {
      busy = false;
    }
  }
</script>

<Dialog id="export" title="Export Audio" width={560} onclose={onclose} onsubmit={submit}>
  {#if form && view}
    {@const formatOrder = view.formats.map((f) => f.value)}
    {@const trackOrder = view.stem_tracks.map((t) => t.value)}
    <fieldset>
      <legend>Output</legend>
      <div class="form">
        <label for="export-path">File (without extension)</label>
        <span class="row">
          <input id="export-path" type="text" bind:value={form.path} />
          <button type="button" onclick={browse}>Browse…</button>
        </span>
        <label for="export-preset">Preset</label>
        <select id="export-preset" value={form.preset} onchange={(event) => void preset(event.currentTarget.value)}>
          {#each view.presets as name (name)}<option value={name}>{name}</option>{/each}
        </select>
        <span class="label" id="export-formats-label">Formats</span>
        <div class="checks" role="group" aria-labelledby="export-formats-label">
          {#each view.formats as format (format.value)}
            <label>
              <input
                type="checkbox"
                checked={form.formats.includes(format.value)}
                onchange={(event) =>
                  form && (form.formats = toggle(form.formats, format.value, event.currentTarget.checked, formatOrder))}
              />
              {format.label}
            </label>
          {/each}
        </div>
        <label for="export-depth">PCM bit depth</label>
        <select id="export-depth" bind:value={form.bit_depth}>
          {#each DEPTHS as [value, label] (value)}<option {value}>{label}</option>{/each}
        </select>
        <label for="export-rate">Sample rate</label>
        <select id="export-rate" bind:value={form.sample_rate}>
          {#each view.rates as rate (rate.label)}<option value={rate.value}>{rate.label}</option>{/each}
        </select>
        <label for="export-channels">Channels</label>
        <select id="export-channels" bind:value={form.channels}>
          <option value={2}>Stereo</option>
          <option value={1}>Mono</option>
        </select>
        <label for="export-bitrate">Lossy bitrate</label>
        <select id="export-bitrate" bind:value={form.bitrate_kbps}>
          {#each view.bitrates as rate (rate.value)}<option value={rate.value}>{rate.label}</option>{/each}
        </select>
        <label class="wide check">
          <input type="checkbox" bind:checked={form.dither} />
          TPDF dither when reducing bit depth
        </label>
      </div>
    </fieldset>
    <fieldset>
      <legend>Range</legend>
      <div class="form">
        <label for="export-range">Range</label>
        <select id="export-range" bind:value={form.range}>
          {#each RANGES as [value, label] (value)}<option {value}>{label}</option>{/each}
        </select>
        <label for="export-tail">Tail</label>
        <select id="export-tail" bind:value={form.tail}>
          {#each TAILS as [value, label] (label)}<option {value}>{label}</option>{/each}
        </select>
      </div>
    </fieldset>
    <fieldset>
      <legend>Loudness</legend>
      <div class="form">
        <label for="export-normalize">Normalize</label>
        <select id="export-normalize" bind:value={form.normalize}>
          {#each NORMALIZE as [value, label] (value)}<option {value}>{label}</option>{/each}
        </select>
        <SpinField
          id="export-target"
          label="Target"
          value={form.target}
          spec={target}
          onchange={(value) => form && (form.target = value)}
        />
        <SpinField
          id="export-ceiling"
          label="True-peak ceiling"
          value={form.ceiling}
          spec={ceiling}
          onchange={(value) => form && (form.ceiling = value)}
        />
        <p class="note wide">Normalization changes gain only; it never adds limiting.</p>
      </div>
    </fieldset>
    <fieldset>
      <legend>Stems</legend>
      <div class="form">
        <label class="wide check">
          <input type="checkbox" bind:checked={form.stems} />
          Also export each track as a stem
        </label>
        <label class="wide check">
          <input type="checkbox" bind:checked={form.through_master} />
          Include master processing in stems
        </label>
        <div class="checks wide tracks" role="group" aria-label="Stem tracks">
          {#each view.stem_tracks as track (track.value)}
            <label>
              <input
                type="checkbox"
                checked={form.stem_tracks.includes(track.value)}
                onchange={(event) =>
                  form &&
                  (form.stem_tracks = toggle(form.stem_tracks, track.value, event.currentTarget.checked, trackOrder))}
              />
              {track.label}
            </label>
          {/each}
        </div>
      </div>
    </fieldset>
  {/if}
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
  {#snippet buttons()}
    <button type="button" onclick={onclose}>Cancel</button>
    <button type="submit" disabled={!form || busy}>Export</button>
  {/snippet}
</Dialog>

<style>
  .row {
    display: flex;
    gap: 6px;
    min-width: 0;
  }
  .row input {
    flex: 1;
  }
  .row button {
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--panel-alt);
    color: var(--text);
    font: inherit;
  }
  .label {
    color: var(--text-dim);
  }
  .checks {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 2px 10px;
    max-height: 130px;
    overflow: auto;
  }
  .checks label,
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .tracks {
    max-height: 110px;
  }
</style>
