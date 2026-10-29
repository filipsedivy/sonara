<script lang="ts">
  import { tick } from "svelte";
  import { composer } from "./composer.svelte";
  import { duration, failureReason, outcome, stepCounts, type Entry } from "./model";

  // The AI composer: describe the music, and a headless Claude Code composes it into the open
  // project while the timeline shows it happen. Not a modal dialog: the window and the menu stay
  // usable (and Undo works) while the agent runs, and the panel can be hidden and brought back.

  const log = $derived(composer.log);
  const result = $derived(log.result);
  const how = $derived(outcome(log));
  const counts = $derived(stepCounts(log));
  const canStart = $derived(!composer.active && composer.status?.ready === true && composer.prompt.trim().length > 0);

  let scroller: HTMLDivElement | undefined = $state();
  let following = true;

  function onScroll() {
    if (!scroller) return;
    following = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 24;
  }

  // Keep the newest entry in view while the user has not scrolled away from the end.
  $effect(() => {
    void log.entries;
    if (!following) return;
    void tick().then(() => scroller?.scrollTo({ top: scroller.scrollHeight }));
  });

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (canStart) void composer.start();
  }

  function keydown(event: KeyboardEvent) {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      if (canStart) void composer.start();
    }
  }

  /** Claude Code loads its deferred tools through `ToolSearch`; to a musician that is noise. */
  const label = (tool: string) => (tool === "ToolSearch" ? "loading tools" : tool);

  const mark: Record<Extract<Entry, { type: "step" }>["status"], string> = { running: "…", ok: "✓", failed: "✕" };

  function money(usd: number): string {
    return `$${usd.toFixed(usd < 1 ? 4 : 2)}`;
  }

  function tokens(usage: Record<string, unknown> | null): string {
    if (!usage) return "—";
    const part = (key: string) => (typeof usage[key] === "number" ? (usage[key] as number) : 0);
    return `${part("input_tokens")} in, ${part("output_tokens")} out, ${part("cache_read_input_tokens")} cached`;
  }
</script>

{#if composer.visible}
  <section class="panel" aria-labelledby="ai-composer-title">
    <header>
      <h2 id="ai-composer-title">AI Composer</h2>
      <span class="state" data-state={how} role="status">
        {#if log.phase === "idle"}
          Ready
        {:else if log.phase === "starting"}
          Starting…
        {:else if log.phase === "running"}
          Composing… {counts.total} steps
        {:else if log.phase === "stopping"}
          Stopping…
        {:else if how === "done"}
          Done
        {:else if how === "stopped"}
          Stopped
        {:else}
          Failed
        {/if}
      </span>
      <button type="button" class="close" aria-label="Hide the AI Composer" onclick={() => composer.hide()}>×</button>
    </header>

    {#if composer.status && !composer.status.ready && !composer.active}
      <p class="problem" role="alert">
        {composer.status.running ? "A composition is already running." : composer.status.problem}
        <button type="button" onclick={() => void composer.check()} disabled={composer.checking}>Check again</button>
      </p>
    {:else if composer.status?.claude_version}
      <p class="note">Claude Code {composer.status.claude_version}{log.model ? ` · ${log.model}` : ""}</p>
    {/if}

    <form onsubmit={submit}>
      <label for="ai-composer-prompt" class="note">Describe the music you want</label>
      <textarea
        id="ai-composer-prompt"
        rows="3"
        bind:value={composer.prompt}
        onkeydown={keydown}
        disabled={composer.active}
        placeholder="e.g. A 32-bar dark techno loop at 128 BPM in F minor: kick, rumbling bass, an arpeggio lead and a pad"
      ></textarea>
      <div class="buttons">
        <span class="note">It edits the open project. Save and export it yourself; Undo undoes its steps.</span>
        {#if composer.active}
          <button type="button" onclick={() => void composer.stop()} disabled={log.phase === "stopping"}>
            {log.phase === "stopping" ? "Stopping…" : "Stop"}
          </button>
        {:else}
          <button type="submit" class="primary" disabled={!canStart}>Compose</button>
        {/if}
      </div>
    </form>

    {#if log.mcpErrors.length > 0 || log.mcpServers.some((s) => s.name === "sonara" && s.status !== "connected")}
      <p class="problem" role="alert">
        The Sonara tools are not connected ({log.mcpServers.map((s) => `${s.name}: ${s.status}`).join(", ")}).
        {log.mcpErrors.join("; ")}
      </p>
    {/if}

    {#if log.entries.length > 0 || composer.active}
      <div class="log" role="log" aria-label="Progress" bind:this={scroller} onscroll={onScroll}>
        {#each log.entries as entry, index (index)}
          {#if entry.type === "text"}
            <p class="said">{entry.text}</p>
          {:else}
            <div class="step" data-status={entry.status} class:nested={entry.nested} title={entry.preview}>
              <span class="mark" aria-hidden="true">{mark[entry.status]}</span>
              <span class="tool">{label(entry.tool)}</span>
              <span class="args">{entry.tool === "ToolSearch" ? "" : entry.summary}</span>
              {#if entry.status === "failed" && entry.preview}
                <span class="why">{entry.preview}</span>
              {/if}
            </div>
          {/if}
        {/each}
        {#if log.retry}
          <p class="note">{log.retry}</p>
        {/if}
      </div>
    {/if}

    {#if log.phase === "done"}
      <div class="summary" data-state={how}>
        <h3>
          {#if how === "done"}Finished{:else if how === "stopped"}Stopped by you{:else}Did not finish{/if}
        </h3>
        {#if how === "failed"}
          <p class="problem">{failureReason(log)}</p>
        {/if}
        {#if how === "done" && result?.text}
          <p class="final">{result.text}</p>
        {/if}
        <dl>
          <dt>Steps</dt>
          <dd>{counts.total}{counts.failed > 0 ? `, ${counts.failed} failed` : ""}</dd>
          {#if result?.duration_ms != null}
            <dt>Time</dt>
            <dd>{duration(result.duration_ms)}</dd>
          {/if}
          {#if result?.num_turns != null}
            <dt>Turns</dt>
            <dd>{result.num_turns}</dd>
          {/if}
          {#if result?.cost_usd != null}
            <dt>Cost (estimate)</dt>
            <dd>{money(result.cost_usd)}</dd>
          {/if}
          {#if result}
            <dt>Tokens</dt>
            <dd>{tokens(result.usage)}</dd>
          {/if}
          {#if log.sessionId}
            <dt>Session</dt>
            <dd class="mono">{log.sessionId}</dd>
          {/if}
          {#if log.exit}
            <dt>Exit code</dt>
            <dd>{log.exit.code ?? "ended by a signal"}</dd>
          {/if}
          {#if result && result.permission_denials.length > 0}
            <dt>Denied</dt>
            <dd class="mono">{result.permission_denials.join(", ")}</dd>
          {/if}
        </dl>
        {#if result || log.stderr.length > 0 || log.output.length > 0}
          <details>
            <summary>Full output of Claude Code</summary>
            {#if result}
              <pre>{result.raw}</pre>
            {/if}
            {#if log.stderr.length > 0}
              <h4>Standard error</h4>
              <pre>{log.stderr.join("\n")}</pre>
            {/if}
            {#if log.output.length > 0}
              <h4>Other output</h4>
              <pre>{log.output.join("\n")}</pre>
            {/if}
          </details>
        {/if}
      </div>
    {/if}
  </section>
{/if}

<style>
  .panel {
    position: fixed;
    right: 12px;
    bottom: 36px;
    z-index: 20;
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: min(460px, calc(100vw - 24px));
    max-height: min(72vh, 680px);
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
    color: var(--text);
    box-shadow: 0 8px 28px rgb(0 0 0 / 35%);
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  h2 {
    margin: 0;
    font-size: 14px;
  }
  h3 {
    margin: 0;
    font-size: 13px;
  }
  h4 {
    margin: 6px 0 2px;
    font-size: 12px;
    color: var(--text-dim);
  }
  .state {
    flex: 1;
    color: var(--text-dim);
    font-size: 12px;
  }
  .state[data-state="done"] {
    color: var(--meter-green);
  }
  .state[data-state="failed"] {
    color: var(--error);
  }
  .close {
    border: none;
    background: none;
    color: var(--text-dim);
    font-size: 18px;
    line-height: 1;
    cursor: pointer;
  }
  .note {
    margin: 0;
    color: var(--text-dim);
    font-size: 12px;
  }
  .problem {
    margin: 0;
    color: var(--error);
    font-size: 12px;
    -webkit-user-select: text;
    user-select: text;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  textarea {
    resize: vertical;
    min-height: 54px;
    font: inherit;
  }
  .buttons {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  button {
    padding: 4px 12px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--panel-alt);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  button.primary:not(:disabled) {
    border-color: var(--accent);
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .close:disabled {
    opacity: 1;
  }
  .log {
    flex: 1 1 auto;
    min-height: 80px;
    max-height: 300px;
    overflow: auto;
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--panel-alt);
    font-size: 12px;
    -webkit-user-select: text;
    user-select: text;
  }
  .said {
    margin: 4px 0;
    white-space: pre-wrap;
  }
  .step {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 6px;
    align-items: baseline;
    padding: 1px 0;
  }
  .step.nested {
    padding-left: 14px;
  }
  .mark {
    width: 1.2em;
    text-align: center;
    color: var(--text-dim);
  }
  .step[data-status="ok"] .mark {
    color: var(--meter-green);
  }
  .step[data-status="failed"] .mark,
  .why {
    color: var(--error);
  }
  .tool {
    font-weight: 600;
  }
  .args {
    flex: 1 1 0;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-dim);
    font-family: Menlo, monospace;
    font-size: 11px;
  }
  .why {
    flex-basis: 100%;
    padding-left: 1.6em;
    font-size: 11px;
  }
  .summary {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
    overflow: auto;
    -webkit-user-select: text;
    user-select: text;
  }
  .final {
    margin: 0;
    white-space: pre-wrap;
    font-size: 12px;
  }
  dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 2px 10px;
    margin: 0;
    font-size: 12px;
  }
  dt {
    color: var(--text-dim);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .mono,
  pre {
    font-family: Menlo, monospace;
    font-size: 11px;
  }
  pre {
    max-height: 200px;
    margin: 0;
    overflow: auto;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--panel-alt);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  summary {
    cursor: pointer;
    font-size: 12px;
  }
</style>
