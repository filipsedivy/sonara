<script lang="ts">
  import * as backend from "../backend";
  import type { Session } from "../session.svelte";
  import { formatTime } from "../time/format";
  import { formatPosition, signatureText } from "../time/signatures";
  import { TempoMap } from "../time/tempo";
  import type { EngineStatus } from "../types";
  import { COUNT_IN, engineLabel, MAX_BPM, MIN_BPM, SIGNATURES, type TransportControl } from "./transport";

  // The transport bar: Home, Play/Pause, Stop and Record; the position as
  // Bar.Beat.Tick and the time; Loop and Metronome; the base tempo, the time signature at bar 1
  // and the count-in; and the engine label. Without a project only Home and the count-in work.
  let { session, control }: { session: Session; control: TransportControl } = $props();

  const project = $derived(session.view?.project ?? null);
  const disabled = $derived(project === null);
  const tempo = $derived(project ? new TempoMap(project.tempo) : null);
  const position = $derived(formatPosition(project?.time_signatures ?? null, session.playhead.beat));
  const time = $derived(formatTime(tempo ? tempo.beatToSeconds(session.playhead.beat) : 0));
  const rolling = $derived(session.transport().state === "playing" || session.transport().state === "recording");
  const signature = $derived(project?.time_signatures[0] ? signatureText(project.time_signatures[0]) : "4/4");
  const bpm = $derived(project ? project.tempo[0].bpm.toFixed(2) : "120.00");
  const countIn = $derived(Math.min(2, project?.transport.count_in_bars ?? 0));

  // The engine's reason is asked for once the frames say it is unavailable.
  let status: EngineStatus | null = $state(null);
  const unavailable = $derived(session.frame?.engine === "unavailable");
  $effect(() => {
    if (unavailable && status === null) {
      backend
        .transportStatus()
        .then((answer) => (status = answer))
        .catch((error: unknown) => session.fail(error));
    }
  });
  const engine = $derived(engineLabel(session.frame, status));

  let signatureField: HTMLInputElement | undefined = $state();
  let tempoField: HTMLInputElement | undefined = $state();

  async function applySignature() {
    if (!signatureField) return;
    const ok = await control.setSignature(signatureField.value);
    // The field shows the project's signature again after a refusal (`refresh`).
    if (!ok && signatureField) signatureField.value = signature;
  }

  async function applyTempo() {
    if (!tempoField) return;
    const value = Number.parseFloat(tempoField.value);
    if (Number.isFinite(value)) await control.setTempo(value);
    if (tempoField) tempoField.value = bpm;
  }

  // Enter commits a field through its change event, as focus loss does; Escape reverts it.
  function enter(event: KeyboardEvent, shown: string) {
    const field = event.currentTarget as HTMLInputElement;
    if (event.key === "Enter") {
      event.preventDefault();
      field.blur();
    } else if (event.key === "Escape") {
      field.value = shown;
      field.blur();
    }
  }
</script>

<div class="transport-bar" role="toolbar" aria-label="Transport">
  <button type="button" class="tool" title="Return to start (Home)" aria-label="Return to Start" onclick={() => control.returnToStart()}>⏮</button>
  <!-- ADR 0009's check: a toggle button "Play", pressed while the transport rolls; the glyph
       shows ⏸ then. -->
  <button
    type="button"
    class="tool play"
    title="Play / Stop (Space)"
    aria-label="Play"
    aria-pressed={rolling}
    {disabled}
    onclick={() => control.togglePlay()}
    data-testid="play">{rolling ? "⏸" : "▶"}</button
  >
  <button type="button" class="tool" title="Stop (Return to last start)" aria-label="Stop" {disabled} onclick={() => control.stop()}>■</button>
  <button
    type="button"
    class="tool record"
    title="Record armed tracks (R)"
    aria-label="Record"
    aria-pressed={session.recording}
    {disabled}
    onclick={() => control.toggleRecord(!session.recording)}>●</button
  >
  <span class="gap"></span>
  <!-- The position is readable on focus (a timer, whose live region is off: it would chatter at
       60 Hz while playing). -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <span class="clock" role="timer" tabindex="0" aria-label="Position {position}, time {time}">
    <span class="position" title="Bar.Beat.Tick (960 ticks per beat)" data-testid="position">{position}</span>
    <span class="time" data-testid="time">{time}</span>
  </span>
  <span class="gap"></span>
  <button
    type="button"
    class="tool"
    title="Loop (L)"
    aria-label="Loop"
    aria-pressed={project?.transport.loop_enabled ?? false}
    {disabled}
    onclick={() => control.setTransport("loop_enabled", !(project?.transport.loop_enabled ?? false))}>⟳</button
  >
  <button
    type="button"
    class="tool"
    title="Metronome (C)"
    aria-label="Metronome"
    aria-pressed={project?.transport.metronome ?? false}
    {disabled}
    onclick={() => control.setTransport("metronome", !(project?.transport.metronome ?? false))}>♩</button
  >
  <span class="gap"></span>
  <label class="field">
    <span>BPM</span>
    <input
      bind:this={tempoField}
      type="number"
      min={MIN_BPM}
      max={MAX_BPM}
      step="1"
      value={bpm}
      title="Song tempo (tempo changes are edited in the timeline)"
      aria-label="Tempo"
      {disabled}
      onchange={applyTempo}
      onkeydown={(event) => enter(event, bpm)}
    />
  </label>
  <input
    bind:this={signatureField}
    class="signature"
    list="signature-choices"
    value={signature}
    title="Time signature at bar 1"
    aria-label="Time signature"
    {disabled}
    onchange={applySignature}
    onkeydown={(event) => enter(event, signature)}
  />
  <datalist id="signature-choices">
    {#each SIGNATURES as choice (choice)}<option value={choice}></option>{/each}
  </datalist>
  <select
    title="Count-in before recording"
    aria-label="Count-in"
    value={countIn}
    onchange={(event) => control.setTransport("count_in_bars", Number(event.currentTarget.value))}
  >
    {#each COUNT_IN as label, bars (label)}<option value={bars}>{label}</option>{/each}
  </select>
  <span class="spacer"></span>
  <span class="engine" title={engine?.title ?? ""} data-testid="engine">{engine?.text ?? ""}</span>
</div>

<style>
  .transport-bar {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 38px;
    padding: 2px 8px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }
  .tool {
    min-width: 30px;
    height: 26px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text);
    font-size: 15px;
  }
  .tool:hover:not(:disabled) {
    background: var(--panel-alt);
  }
  .tool[aria-pressed="true"] {
    background: var(--accent);
    color: #fff;
  }
  .tool.record[aria-pressed="true"] {
    background: var(--meter-red);
  }
  /* Play's pressed state is for assistive technology; its glyph shows it on screen. */
  .tool.play[aria-pressed="true"] {
    background: transparent;
    color: var(--text);
  }
  .tool.play[aria-pressed="true"]:hover:not(:disabled) {
    background: var(--panel-alt);
  }
  .tool:disabled {
    color: var(--text-dim);
    opacity: 0.5;
  }
  .gap {
    width: 10px;
  }
  .spacer {
    flex: 1;
  }
  .clock {
    display: flex;
    align-items: center;
    gap: 4px;
    border-radius: 4px;
  }
  .position {
    min-width: 110px;
    font: 15px Menlo, monospace;
  }
  .time {
    min-width: 90px;
    font: 12px Menlo, monospace;
    color: var(--text-dim);
  }
  .field {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  input,
  select {
    height: 24px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
  input[type="number"] {
    width: 72px;
  }
  .signature {
    width: 70px;
  }
  .engine {
    color: var(--text-dim);
    white-space: nowrap;
  }
</style>
