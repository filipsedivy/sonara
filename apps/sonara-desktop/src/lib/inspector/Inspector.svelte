<script lang="ts">
  import * as backend from "../backend";
  import type { Session } from "../session.svelte";
  import type { ClipView, Edit, FadeView, TrackView } from "../types";
  import {
    CLIP_GAIN,
    CLIP_LENGTH,
    FADE,
    FADE_CURVES,
    INPUTS,
    LOOP_LENGTH,
    METADATA,
    SONG_END,
    TRACK_PAN,
    TRACK_VOLUME,
    clipAsset,
    effectsText,
    fadeEdit,
    inputIndex,
    inspected,
    instrumentText,
    lengthEdit,
    lengthText,
    loopEdit,
    loopLengthValue,
    mediaText,
    metadataEdit,
    outputChoices,
    projectLengthText,
    songEndEdit,
    startText,
  } from "./inspector";
  import SpinField from "./SpinField.svelte";
  import TextField from "./TextField.svelte";

  // The Inspector pane (the prototype's `InspectorPanel`): the Clip group
  // when exactly one clip is selected, else the Track group of the current track, and always the
  // Project group. Every field is updated in place from the project view, so an edit from
  // anywhere (an agent, the mixer, the inspector itself) shows at once and
  // never replaces the text of a field being edited. Each fade has its own curve (G4,
  // D22). Fields are `<label for>`-labelled.
  let { session }: { session: Session } = $props();

  const project = $derived(session.view?.project ?? null);
  const shows = $derived(project ? inspected(project, session.selection) : null);

  async function run(edit: Edit | null): Promise<boolean> {
    if (!edit) return true;
    try {
      await backend.edit(edit);
      return true;
    } catch (error) {
      session.fail(error);
      return false;
    }
  }

  /** A combo's edit; a refused one shows the model's value again (as MX-05's combo). */
  function choose(select: HTMLSelectElement, model: string, edit: Edit) {
    void run(edit).then((ok) => {
      if (!ok) select.value = model;
    });
  }

  function fade(clip: ClipView, which: "fade_in" | "fade_out", patch: Partial<FadeView>) {
    if (clip.type !== "audio") return;
    void run(fadeEdit(clip, which, { ...clip[which], ...patch }));
  }

  function trackOf(track: TrackView) {
    return outputChoices(track);
  }
</script>

<div class="inspector" aria-label="Inspector fields">
  {#if !project}
    <p class="empty">No project</p>
  {:else}
    {#if shows?.clip}
      {@const clip = shows.clip.clip}
      {@const track = shows.clip.track}
      {#key clip.id}
        <section class="group" aria-labelledby="inspector-clip">
          <h3 id="inspector-clip">Clip</h3>
          <div class="form">
            <TextField
              id="clip-name"
              label="Name"
              value={clip.name}
              onchange={(name) => run({ op: "set_clip_properties", clip_id: clip.id, name })}
            />
            <span class="label">Track</span><span class="value">{track.name}</span>
            <span class="label">Start</span><span class="value" data-testid="clip-start">{startText(project, clip)}</span>
            <span class="label">Length</span><span class="value" data-testid="clip-length">{lengthText(project, clip)}</span>
            <SpinField
              id="clip-set-length"
              label="Set length"
              value={clip.length}
              spec={CLIP_LENGTH}
              onchange={(length) => run(lengthEdit(clip, length))}
            />
            <label for="clip-muted">Muted</label>
            <input
              id="clip-muted"
              type="checkbox"
              checked={clip.muted}
              onchange={(event) =>
                run({ op: "set_clip_properties", clip_id: clip.id, muted: event.currentTarget.checked })}
            />
            {#if clip.type === "audio"}
              {@const asset = clipAsset(project, clip)}
              <SpinField
                id="clip-gain"
                label="Gain"
                value={clip.gain_db}
                spec={CLIP_GAIN}
                onchange={(gain_db) => run({ op: "set_clip_properties", clip_id: clip.id, gain_db })}
              />
              <SpinField
                id="clip-fade-in"
                label="Fade in"
                value={clip.fade_in.seconds}
                spec={FADE}
                onchange={(seconds) => fade(clip, "fade_in", { seconds })}
              />
              <label for="clip-fade-in-curve">Fade-in curve</label>
              <select
                id="clip-fade-in-curve"
                value={clip.fade_in.curve}
                onchange={(event) => fade(clip, "fade_in", { curve: event.currentTarget.value as FadeView["curve"] })}
              >
                {#each FADE_CURVES as curve (curve)}<option value={curve}>{curve}</option>{/each}
              </select>
              <SpinField
                id="clip-fade-out"
                label="Fade out"
                value={clip.fade_out.seconds}
                spec={FADE}
                onchange={(seconds) => fade(clip, "fade_out", { seconds })}
              />
              <label for="clip-fade-out-curve">Fade-out curve</label>
              <select
                id="clip-fade-out-curve"
                value={clip.fade_out.curve}
                onchange={(event) =>
                  fade(clip, "fade_out", { curve: event.currentTarget.value as FadeView["curve"] })}
              >
                {#each FADE_CURVES as curve (curve)}<option value={curve}>{curve}</option>{/each}
              </select>
              {#if asset}
                <span class="label">Media</span><span class="value media" data-testid="clip-media">{mediaText(asset)}</span>
              {/if}
            {:else}
              {@const loopOn = clip.loop_length !== null}
              {@const loopValue = loopLengthValue(clip)}
              <span class="label">Notes</span><span class="value">{clip.notes.length}</span>
              <span class="label"></span>
              <label class="check">
                <input
                  id="clip-loop"
                  type="checkbox"
                  checked={loopOn}
                  onchange={(event) => run(loopEdit(clip, event.currentTarget.checked, loopValue))}
                />
                Loop content
              </label>
              <SpinField
                id="clip-loop-length"
                label="Loop length"
                value={loopValue}
                spec={LOOP_LENGTH}
                disabled={!loopOn}
                onchange={(length) => run(loopEdit(clip, true, length))}
              />
            {/if}
          </div>
        </section>
      {/key}
    {:else if shows?.track}
      {@const track = shows.track}
      {#key track.id}
        <section class="group" aria-labelledby="inspector-track">
          <h3 id="inspector-track">Track · {track.kind}</h3>
          <div class="form">
            <TextField
              id="track-name"
              label="Name"
              value={track.name}
              onchange={(name) => run({ op: "rename_track", track_id: track.id, name })}
            />
            <SpinField
              id="track-volume"
              label="Volume"
              value={track.mixer.volume_db}
              spec={TRACK_VOLUME}
              onchange={(volume_db) => run({ op: "set_track_mixer", track_id: track.id, volume_db })}
            />
            <SpinField
              id="track-pan"
              label="Pan"
              value={track.mixer.pan}
              spec={TRACK_PAN}
              onchange={(pan) => run({ op: "set_track_mixer", track_id: track.id, pan })}
            />
            {#if track.kind !== "master"}
              {@const outputs = trackOf(track)}
              {@const current = outputs.choices[outputs.index]?.id ?? "master"}
              <label for="track-output">Output</label>
              <select
                id="track-output"
                value={current}
                onchange={(event) =>
                  choose(event.currentTarget, current, {
                    op: "set_track_output",
                    track_id: track.id,
                    output_id: event.currentTarget.value,
                  })}
              >
                {#each outputs.choices as choice (choice.id)}<option value={choice.id}>{choice.name}</option>{/each}
              </select>
            {/if}
            {#if instrumentText(track) !== null}
              <span class="label">Instrument</span><span class="value">{instrumentText(track)}</span>
            {/if}
            {#if track.kind === "audio"}
              <label for="track-monitoring">Monitoring</label>
              <select
                id="track-monitoring"
                value={track.input.monitor}
                onchange={(event) =>
                  run({
                    op: "set_track_input",
                    track_id: track.id,
                    monitor: event.currentTarget.value as TrackView["input"]["monitor"],
                  })}
              >
                {#each ["off", "auto", "on"] as mode (mode)}<option value={mode}>{mode}</option>{/each}
              </select>
              <label for="track-input">Input</label>
              <select
                id="track-input"
                value={String(inputIndex(track.input.channels))}
                onchange={(event) =>
                  run({
                    op: "set_track_input",
                    track_id: track.id,
                    channels: [...INPUTS[Number(event.currentTarget.value)][1]],
                  })}
              >
                {#each INPUTS as [name], index (name)}<option value={String(index)}>{name}</option>{/each}
              </select>
            {/if}
            <span class="label">Clips</span><span class="value">{track.clips.length}</span>
            <span class="label">Effects</span><span class="value" data-testid="track-effects">{effectsText(track)}</span>
          </div>
        </section>
      {/key}
    {/if}
    <section class="group" aria-labelledby="inspector-project">
      <h3 id="inspector-project">Project</h3>
      <div class="form">
        <TextField
          id="project-name"
          label="Name"
          value={project.name}
          onchange={(name) => run({ op: "rename_project", name })}
        />
        <span class="label">Sample rate</span><span class="value">{project.sample_rate} Hz</span>
        <span class="label">Length</span><span class="value" data-testid="project-length">{projectLengthText(project)}</span>
        <SpinField
          id="project-song-end"
          label="Song end"
          value={project.length_beats ?? 0}
          spec={SONG_END}
          onchange={(beats) => run(songEndEdit(beats))}
        />
        {#each METADATA as [key, label] (key)}
          <TextField
            id={`project-${key}`}
            {label}
            value={project.metadata[key]}
            onchange={(text) => run(metadataEdit(project, key, text))}
          />
        {/each}
      </div>
    </section>
  {/if}
</div>

<style>
  .inspector {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 8px;
  }
  .empty {
    color: var(--text-dim);
  }
  .group {
    margin-bottom: 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 6px 8px 8px;
  }
  h3 {
    margin: 0 0 6px;
    font-size: 12px;
    font-weight: 600;
  }
  .form {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 4px 8px;
    align-items: center;
  }
  .form :global(label),
  .label {
    color: var(--text-dim);
    font-size: 12px;
  }
  .form :global(label.check) {
    display: flex;
    align-items: center;
    gap: 4px;
    color: var(--text);
  }
  .value {
    min-width: 0;
    overflow-wrap: anywhere;
    font-size: 12px;
  }
  .value.media {
    white-space: pre-line;
    color: var(--text-dim);
  }
  select {
    min-width: 0;
    font: inherit;
  }
  input[type="checkbox"] {
    justify-self: start;
    margin: 0;
  }
</style>
