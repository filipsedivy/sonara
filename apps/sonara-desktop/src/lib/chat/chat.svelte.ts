// The AI Chat window's store. The chat lives in Rust: this keeps a view of it (model.ts) that
// follows the `chat-event` stream, and every action only calls a command. Nothing is predicted
// here (no optimistic update): the `changed` event that follows a command shows its effect, and a
// rejection shows as `error`.

import {
  chatClear,
  chatEdit,
  chatEnqueue,
  chatError,
  chatMove,
  chatProviders,
  chatRemove,
  chatResume,
  chatSetProvider,
  chatSnapshot,
  chatStop,
  onChatEvent,
  type ChatError,
  type ChatEvent,
  type ProviderId,
  type TaskId,
} from "./api";
import { applyEvent, emptyView, moveTarget, resync, withProviders, type MoveDirection } from "./model";
import { RESYNC_DELAY_MS, readFollowUp, readUntilQuiet, type EventFlag } from "./snapshot";

class Chat {
  /** Replaced as a whole on every change, so the deep proxy of `$state` is not needed. */
  view = $state.raw(emptyView());
  error = $state.raw<ChatError | null>(null);
  /** Detection is running (the first one, or Check again). */
  checking = $state(false);

  /** Set by an event while a snapshot is on its way: that snapshot may be older. */
  private flag: EventFlag = { arrived: false };
  private loading = true;
  private resyncTimer: ReturnType<typeof setTimeout> | undefined;
  private stopped = false;

  /**
   * Starts following the chat: listens first, then reads the snapshot, so no change is missed.
   * Events that arrive while the first snapshot is on its way are not applied (it may be older or
   * newer than them); the snapshot is read again instead, until it is quiet. If it never is, the
   * last one is applied and read once more after a pause, so that a dropped final event does not
   * leave the view stale. Returns the stop.
   */
  async init(): Promise<() => void> {
    this.loading = true;
    this.stopped = false;
    let unlisten: () => void = () => {};
    try {
      unlisten = await onChatEvent((event) => this.apply(event));
      void this.refreshProviders();
      const { snapshot, quiet } = await readUntilQuiet(chatSnapshot, this.flag);
      this.view = resync(this.view, snapshot);
      if (!quiet) this.resyncTimer = setTimeout(() => void this.followUp(), RESYNC_DELAY_MS);
    } catch (error) {
      // No listener (the window lacks the `listen` permission) or no snapshot: say so, and stay
      // usable for what still works rather than wait for a state that never comes.
      this.error = chatError(error);
    } finally {
      this.loading = false;
    }
    return () => {
      this.stopped = true;
      clearTimeout(this.resyncTimer);
      unlisten();
    };
  }

  /** The second look at a first snapshot that events disturbed; events apply live meanwhile. */
  private async followUp() {
    this.resyncTimer = undefined;
    try {
      const snapshot = await readFollowUp(chatSnapshot, this.flag);
      if (snapshot !== null && !this.stopped) this.view = resync(this.view, snapshot);
    } catch {
      // The live events are all there is; the view stays as it is.
    }
  }

  private apply(event: ChatEvent) {
    this.flag.arrived = true;
    if (this.loading) return;
    this.view = applyEvent(this.view, event);
  }

  /** Detects the providers again (they may have been installed or signed in since). */
  async refreshProviders() {
    this.checking = true;
    try {
      this.view = withProviders(this.view, await chatProviders());
    } catch (error) {
      this.error = chatError(error);
    } finally {
      this.checking = false;
    }
  }

  /** Runs a command; a rejection becomes `error`, a success clears it. */
  private async run(command: () => Promise<unknown>): Promise<boolean> {
    try {
      await command();
      this.error = null;
      return true;
    } catch (error) {
      this.error = chatError(error);
      return false;
    }
  }

  dismissError() {
    this.error = null;
  }

  /** Queues `prompt`; true when the backend took it. */
  send(prompt: string): Promise<boolean> {
    return this.run(() => chatEnqueue(prompt));
  }

  edit(id: TaskId, prompt: string): Promise<boolean> {
    return this.run(() => chatEdit(id, prompt));
  }

  remove(id: TaskId): Promise<boolean> {
    return this.run(() => chatRemove(id));
  }

  /** Moves a queued task one step; nothing when it cannot move that way. */
  async move(id: TaskId, direction: MoveDirection): Promise<boolean> {
    const state = this.view.state;
    const to = state ? moveTarget(state, id, direction) : null;
    if (to === null) return false;
    return this.run(() => chatMove(id, to));
  }

  stop(): Promise<boolean> {
    return this.run(chatStop);
  }

  resume(): Promise<boolean> {
    return this.run(chatResume);
  }

  clear(): Promise<boolean> {
    return this.run(chatClear);
  }

  setProvider(provider: ProviderId): Promise<boolean> {
    return this.run(() => chatSetProvider(provider));
  }
}

export const chat = new Chat();
