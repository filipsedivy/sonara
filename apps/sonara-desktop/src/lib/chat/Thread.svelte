<script lang="ts">
  import { tick } from "svelte";
  import { duration, sonaraMissing, stepCounts, type Entry } from "./runlog";
  import type { TaskView } from "./api";
  import { logOf, money, taskOutcomeText, tokensText, type ChatView } from "./model";

  // The conversation: for each task that has started, the user's prompt, then what the agent said
  // and did as it streamed in (its tool calls as collapsible steps), then how it ended. Queued
  // tasks are not here yet: the queue shows them.
  let { view }: { view: ChatView } = $props();

  const tasks = $derived(view.state?.tasks.filter((task) => task.state !== "queued") ?? []);

  let scroller: HTMLDivElement | undefined = $state();
  let following = true;

  /** Moves the focus to the conversation (where it goes when nothing else can take it). */
  export function focus() {
    scroller?.focus({ preventScroll: true });
  }

  function onScroll() {
    if (!scroller) return;
    following = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 24;
  }

  // Keep the newest entry in view while the user has not scrolled away from the end.
  $effect(() => {
    void view.logs;
    void tasks.length;
    if (!following) return;
    void tick().then(() => scroller?.scrollTo({ top: scroller.scrollHeight }));
  });

  /** Claude Code loads its deferred tools through `ToolSearch`; to a musician that is noise. */
  const label = (tool: string) => (tool === "ToolSearch" ? "loading tools" : tool);

  const mark: Record<Extract<Entry, { type: "step" }>["status"], string> = { running: "…", ok: "✓", failed: "✕" };
  const markText: Record<Extract<Entry, { type: "step" }>["status"], string> = {
    running: "running",
    ok: "succeeded",
    failed: "failed",
  };

  function sayings(task: TaskView): number {
    return logOf(view, task.id).entries.filter((entry) => entry.type === "text").length;
  }
</script>

<div class="thread" role="log" aria-label="Conversation" aria-live="off" tabindex="-1" bind:this={scroller} onscroll={onScroll}>
  {#if tasks.length === 0}
    <p class="empty">Describe what you want. The agent edits the open project while you watch; Undo steps back.</p>
  {/if}
  {#each tasks as task (task.id)}
    {@const log = logOf(view, task.id)}
    {@const counts = stepCounts(log)}
    <article class="task" data-task={task.id} data-state={task.state}>
      <p class="user">{task.prompt}</p>
      <div class="agent">
        {#each log.entries as entry, index (index)}
          {#if entry.type === "text"}
            <p class="said">{entry.text}</p>
          {:else}
            <details class="step" data-status={entry.status} class:nested={entry.nested}>
              <summary>
                <span class="mark" aria-hidden="true">{mark[entry.status]}</span>
                <span class="tool">{label(entry.tool)}</span>
                <span class="sr">, {markText[entry.status]}</span>
                <span class="args">{entry.tool === "ToolSearch" ? "" : entry.summary}</span>
              </summary>
              <pre>{entry.preview || (entry.status === "running" ? "Running…" : "No output")}</pre>
            </details>
          {/if}
        {/each}
        {#if log.retry}
          <p class="note">{log.retry}</p>
        {/if}
        {#if sonaraMissing(log) || log.mcpErrors.length > 0}
          <p class="problem">The Sonara tools are not connected. {log.mcpErrors.join("; ")}</p>
        {/if}
        {#if task.state === "running" && log.entries.length === 0}
          <p class="note">Starting…</p>
        {/if}
      </div>
      {#if task.state !== "running"}
        <footer data-state={task.state}>
          <h3>{taskOutcomeText(task)}</h3>
          {#if task.state === "done" && task.result?.text && sayings(task) === 0}
            <p class="said">{task.result.text}</p>
          {/if}
          {#if task.result}
            {@const result = task.result}
            <dl>
              {#if counts.total > 0}
                <dt>Steps</dt>
                <dd>{counts.total}{counts.failed > 0 ? `, ${counts.failed} failed` : ""}</dd>
              {/if}
              {#if result.duration_ms !== null}
                <dt>Time</dt>
                <dd>{duration(result.duration_ms)}</dd>
              {/if}
              {#if result.num_turns !== null}
                <dt>Turns</dt>
                <dd>{result.num_turns}</dd>
              {/if}
              {#if result.cost_usd !== null}
                <dt>Cost (estimate)</dt>
                <dd>{money(result.cost_usd)}</dd>
              {/if}
              <dt>Tokens</dt>
              <dd>{tokensText(result)}</dd>
              {#if result.session_id}
                <dt>Session</dt>
                <dd class="mono">{result.session_id}</dd>
              {/if}
              {#if result.denied_tools.length > 0}
                <dt>Denied</dt>
                <dd class="mono">{result.denied_tools.join(", ")}</dd>
              {/if}
            </dl>
          {/if}
        </footer>
      {/if}
    </article>
  {/each}
</div>

<style>
  .thread {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel);
    -webkit-user-select: text;
    user-select: text;
  }
  .thread:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
  .empty,
  .note {
    margin: 0;
    color: var(--text-dim);
    font-size: 12px;
  }
  .task {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-bottom: 12px;
  }
  .task + .task {
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }
  .user {
    align-self: flex-end;
    max-width: 85%;
    margin: 0;
    padding: 6px 10px;
    border: 1px solid var(--accent);
    border-radius: 10px 10px 2px 10px;
    background: var(--panel-alt);
    color: var(--text);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .agent {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .said {
    margin: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .step.nested {
    margin-left: 14px;
  }
  summary {
    display: flex;
    gap: 6px;
    align-items: baseline;
    font-size: 12px;
    cursor: pointer;
  }
  summary:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .mark {
    width: 1.2em;
    text-align: center;
    color: var(--text-dim);
  }
  .step[data-status="failed"] .mark {
    color: var(--text);
    font-weight: 700;
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
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
  pre {
    max-height: 160px;
    margin: 2px 0 0 1.8em;
    overflow: auto;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--panel-alt);
    font-family: Menlo, monospace;
    font-size: 11px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  /* The light theme's error colour is under 4.5:1 on its panels, so a problem is written in the
     text colour and marked by its border and its wording. */
  .problem {
    margin: 0;
    padding-left: 8px;
    border-left: 3px solid var(--error);
    font-size: 12px;
  }
  footer {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  h3 {
    margin: 0;
    font-size: 12px;
    color: var(--text-dim);
    overflow-wrap: anywhere;
  }
  footer[data-state="failed"] h3 {
    padding-left: 8px;
    border-left: 3px solid var(--error);
    color: var(--text);
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
  .mono {
    font-family: Menlo, monospace;
    font-size: 11px;
  }
</style>
