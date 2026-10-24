// What the status bar and the recovery dialog show, as plain functions (tested without a page).

import type { AgentStatus, JobNotice, RecoverableSession } from "./types";

/** "agent: on" with the IPC address as tooltip, or "agent: off" (the prototype's label). */
export function agentLabel(agent: AgentStatus | null): { text: string; title: string } {
  if (agent?.on) return { text: "agent: on", title: agent.address ?? "" };
  return { text: "agent: off", title: "agent connections disabled" };
}

/** A recovery entry: "name — autosaved time", as the prototype's Recovery dialog lists it. */
export function sessionLabel(session: RecoverableSession): string {
  return `${session.name ?? "Untitled"} — autosaved ${session.saved_at ?? "?"}`;
}

/** The text of a failed command: commands reject with their message. */
export function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

/** The job label while a job is queued or running: "label — message", or the label alone. */
export function jobLabel(job: JobNotice): string {
  return job.message ? `${job.label} — ${job.message}` : job.label;
}
