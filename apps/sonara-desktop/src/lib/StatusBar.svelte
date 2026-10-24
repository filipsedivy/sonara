<script lang="ts">
  import { agentLabel, jobLabel } from "./status";
  import type { AgentStatus, JobNotice, StatusMessage } from "./types";

  // The window's status bar ("Layout"): the message area, the job label and its
  // progress bar while a job is queued or running ("label — message"), and "agent: on/off".
  let {
    agent,
    message,
    jobs = [],
  }: { agent: AgentStatus | null; message: StatusMessage | null; jobs?: JobNotice[] } = $props();

  const label = $derived(agentLabel(agent));
  const job = $derived(jobs.at(-1) ?? null);
</script>

<footer class="status-bar">
  <span class="message" role="status" aria-live="polite">
    {#if message && !message.error}{message.text}{/if}
  </span>
  <span class="message error" role="alert">
    {#if message?.error}{message.text}{/if}
  </span>
  {#if job}
    <span class="job" data-testid="job-label">{jobLabel(job)}</span>
    <progress class="job-progress" max="100" value={Math.round(job.progress * 100)} aria-label={job.label}></progress>
  {/if}
  <span class="agent" title={label.title} data-testid="agent-label">{label.text}</span>
</footer>

<style>
  .status-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 24px;
    padding: 0 10px;
    border-top: 1px solid var(--border);
    background: var(--bar);
    color: var(--text);
    font-size: 12px;
  }
  .message {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .message:first-child {
    flex: 1;
  }
  .message.error {
    color: var(--error);
  }
  .message.error:empty {
    display: none;
  }
  .job {
    color: var(--text-dim);
    white-space: nowrap;
  }
  .job-progress {
    width: 180px;
    height: 8px;
  }
  .agent {
    color: var(--text-dim);
  }
</style>
