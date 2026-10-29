// The AI composer's state, kept for the whole window: the panel can be hidden while a run goes on
// and shows the same log when it comes back.

import { errorMessage } from "../dialogs/api";
import { composerCheck, composerStart, composerStop, type ComposerEvent, type ComposerStatus } from "./api";
import { emptyLog, failedToStart, reduce, sonaraMissing, starting, stopping, type RunLog } from "./model";

class Composer {
  visible = $state(false);
  prompt = $state("");
  status = $state<ComposerStatus | null>(null);
  checking = $state(false);
  log = $state<RunLog>(emptyLog());

  get active(): boolean {
    return this.log.phase === "starting" || this.log.phase === "running" || this.log.phase === "stopping";
  }

  show() {
    this.visible = true;
    if (!this.active) void this.check();
  }

  hide() {
    this.visible = false;
  }

  /** Looks for Claude Code again (it may have been installed or signed in since). */
  async check() {
    this.checking = true;
    try {
      this.status = await composerCheck();
    } catch (error) {
      this.status = {
        ready: false,
        running: false,
        claude_path: null,
        claude_version: null,
        problem: errorMessage(error),
      };
    } finally {
      this.checking = false;
    }
  }

  async start() {
    if (this.active) return;
    this.log = starting();
    try {
      await composerStart(this.prompt, (event) => this.apply(event));
    } catch (error) {
      this.log = failedToStart(errorMessage(error));
    }
  }

  async stop() {
    if (!this.active) return;
    this.log = stopping(this.log);
    try {
      await composerStop();
    } catch (error) {
      this.log = { ...this.log, stderr: [...this.log.stderr, errorMessage(error)] };
    }
  }

  private apply(event: ComposerEvent) {
    this.log = reduce(this.log, event);
    // Without the Sonara tools nothing can be composed: do not let the run burn turns.
    if (event.kind === "init" && sonaraMissing(this.log)) void this.stop();
  }
}

export const composer = new Composer();
