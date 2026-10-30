<script lang="ts">
  import { tick } from "svelte";
  import type { ChatState, TaskId, TaskView } from "./api";
  import { Composition } from "./ime";
  import {
    MAX_PROMPT_CHARS,
    MAX_PROMPT_UNITS,
    canMove,
    excerpt,
    phase as phaseOf,
    promptLength,
    promptProblem,
    queuedTasks,
    runningTask,
    type MoveDirection,
  } from "./model";

  // What is running and what waits. The running task has Stop. A paused queue (a stop or a failure
  // paused it) says so and has Run queue. Each queued task can be edited in place, removed and
  // moved: with the buttons, or with Alt+ArrowUp and Alt+ArrowDown on the task. The backend does
  // the work; the list shows what its next state says.
  let {
    thread,
    onedit,
    onremove,
    onmove,
    onstop,
    onresume,
    onfocuslost,
  }: {
    thread: ChatState;
    onedit: (id: TaskId, prompt: string) => Promise<boolean>;
    onremove: (id: TaskId) => Promise<boolean>;
    onmove: (id: TaskId, direction: MoveDirection) => Promise<boolean>;
    onstop: () => void;
    onresume: () => void;
    /** Called when the focused task left the list with no neighbour to take the focus. */
    onfocuslost: () => void;
  } = $props();

  const queued = $derived(queuedTasks(thread));
  const running = $derived(runningTask(thread));
  const phase = $derived(phaseOf(thread));

  let editing = $state<{ id: TaskId; text: string } | null>(null);
  const composition = new Composition();
  // Worked out once per change of the text: the counter, its class and Save all use them.
  const editCount = $derived(editing ? promptLength(editing.text.trim()) : 0);
  const editProblem = $derived(editing ? promptProblem(editing.text) : null);
  let saving = $state(false);
  /** The element to focus once the list shows the next order: a moved task keeps the focus. */
  let focusAfter: { id: TaskId; part: "item" | MoveDirection } | "lost" | null = null;
  let list: HTMLUListElement | undefined = $state();

  // A task that left the queue (it started, or an agent removed it) ends its editing.
  $effect(() => {
    if (editing && !queued.some((task) => task.id === editing?.id)) editing = null;
  });

  // After a move the keyed list reorders its elements, which drops the focus: put it back.
  $effect(() => {
    void queued.map((task) => task.id).join();
    if (!focusAfter) return;
    const after = focusAfter;
    focusAfter = null;
    if (after === "lost") {
      // The last waiting task is gone, and its list with it (or the running task is all that is
      // left): the focus would fall to the body. Give it to the caller's stable target, unless
      // the user has moved it somewhere meanwhile.
      void tick().then(() => {
        const active = document.activeElement;
        if (active === null || active === document.body || !active.isConnected) onfocuslost();
      });
      return;
    }
    const { id, part } = after;
    void tick().then(() => {
      const item = list?.querySelector<HTMLElement>(`[data-task="${id}"]`);
      const target = part === "item" ? item : item?.querySelector<HTMLButtonElement>(`[data-move="${part}"]:not(:disabled)`);
      (target ?? item)?.focus({ preventScroll: true });
    });
  });

  async function move(id: TaskId, direction: MoveDirection, part: "item" | MoveDirection) {
    focusAfter = { id, part };
    if (!(await onmove(id, direction))) focusAfter = null;
  }

  /** Removes `task` and gives the focus to the task beside it, which the list shows next. */
  async function remove(task: TaskView) {
    const at = queued.findIndex((candidate) => candidate.id === task.id);
    const neighbour = queued[at + 1] ?? queued[at - 1];
    focusAfter = neighbour ? { id: neighbour.id, part: "item" } : "lost";
    if (!(await onremove(task.id))) focusAfter = null;
  }

  function itemKey(event: KeyboardEvent, task: TaskView) {
    // Without full keyboard access WebKit's Tab reaches only the tasks, not their buttons, so the
    // focused task answers to the keys too: Enter edits it, Delete removes it.
    if (event.target === event.currentTarget && !event.altKey && !event.metaKey && !event.ctrlKey && !event.shiftKey) {
      if (event.key === "Enter") {
        event.preventDefault();
        startEdit(task);
      } else if (event.key === "Delete" || event.key === "Backspace") {
        event.preventDefault();
        void remove(task);
      }
      return;
    }
    if (!event.altKey || event.metaKey || event.ctrlKey || event.shiftKey) return;
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
    // Not while the task's own text is being edited: the textarea has its own arrows.
    if (event.target instanceof HTMLTextAreaElement) return;
    event.preventDefault();
    const direction = event.key === "ArrowUp" ? "up" : "down";
    // On one of the task's move buttons the focus stays on it; elsewhere it stays on the task.
    const onButton = event.target instanceof HTMLElement && event.target.dataset.move !== undefined;
    void move(task.id, direction, onButton ? direction : "item");
  }

  function startEdit(task: TaskView) {
    composition.reset();
    editing = { id: task.id, text: task.prompt };
    void tick().then(() => list?.querySelector<HTMLTextAreaElement>(`[data-task="${task.id}"] textarea`)?.focus());
  }

  function closeEdit(id: TaskId) {
    editing = null;
    void tick().then(() => list?.querySelector<HTMLElement>(`[data-task="${id}"]`)?.focus({ preventScroll: true }));
  }

  async function save() {
    if (!editing || saving || editProblem !== null) return;
    const { id, text } = editing;
    saving = true;
    const ok = await onedit(id, text.trim());
    saving = false;
    if (ok) closeEdit(id);
  }

  function editKey(event: KeyboardEvent, id: TaskId) {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      closeEdit(id);
    } else if (event.key === "Enter" && !event.shiftKey && !composition.handles(event)) {
      // (The Enter that commits an input method's candidate is not a save.)
      event.preventDefault();
      void save();
    }
  }
</script>

{#if running || queued.length > 0 || phase === "paused"}
  <section class="queue" aria-label="Queue">
    {#if running}
      <div class="running">
        <span class="label">Running</span>
        <span class="text" title={running.prompt}>{excerpt(running.prompt)}</span>
        <button type="button" onclick={onstop}>Stop</button>
      </div>
    {/if}
    {#if phase === "paused"}
      <div class="paused">
        <span>The queue is paused. {queued.length === 1 ? "1 task is" : `${queued.length} tasks are`} waiting.</span>
        <button type="button" class="primary" onclick={onresume}>Run queue</button>
      </div>
    {/if}
    {#if queued.length > 0}
      <ul bind:this={list} aria-label="Queued tasks">
        {#each queued as task, index (task.id)}
          <!-- A focusable list item: Alt+Arrow moves the task, the buttons do the same for a pointer. -->
          <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
          <li
            data-task={task.id}
            tabindex="0"
            aria-label="{index + 1} of {queued.length}: {excerpt(task.prompt)}"
            onkeydown={(event) => itemKey(event, task)}
          >
            {#if editing?.id === task.id}
              <div class="edit">
                <textarea
                  rows="3"
                  aria-label="Edit the queued task"
                  aria-describedby="edit-count"
                  bind:value={editing.text}
                  onkeydown={(event) => editKey(event, task.id)}
                  oncompositionstart={composition.start}
                  oncompositionend={composition.end}
                  maxlength={MAX_PROMPT_UNITS}
                ></textarea>
                <div class="row">
                  <span id="edit-count" class="note" class:over={editCount > MAX_PROMPT_CHARS}>
                    {editCount} / {MAX_PROMPT_CHARS}
                  </span>
                  <button type="button" onclick={() => closeEdit(task.id)}>Cancel</button>
                  <button type="button" class="primary" onclick={() => void save()} disabled={saving || editProblem !== null}>
                    Save
                  </button>
                </div>
              </div>
            {:else}
              <span class="text" title={task.prompt}>{excerpt(task.prompt)}</span>
              <span class="actions">
                <button type="button" onclick={() => startEdit(task)}>Edit</button>
                <button type="button" onclick={() => void remove(task)}>Remove</button>
                <button
                  type="button"
                  data-move="up"
                  aria-label="Move up"
                  title="Move up (Alt+Up)"
                  disabled={!canMove(thread, task.id, "up")}
                  onclick={() => void move(task.id, "up", "up")}>↑</button
                >
                <button
                  type="button"
                  data-move="down"
                  aria-label="Move down"
                  title="Move down (Alt+Down)"
                  disabled={!canMove(thread, task.id, "down")}
                  onclick={() => void move(task.id, "down", "down")}>↓</button
                >
              </span>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </section>
{/if}

<style>
  .queue {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel);
  }
  .running,
  .paused,
  li {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .paused {
    justify-content: space-between;
    padding: 4px 8px;
    border-left: 3px solid var(--selection);
    background: var(--panel-alt);
  }
  .label {
    color: var(--text-dim);
    font-size: 12px;
  }
  .text {
    flex: 1 1 0;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  ul {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 160px;
    margin: 0;
    padding: 0;
    overflow: auto;
    list-style: none;
  }
  li {
    padding: 3px 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--panel-alt);
  }
  li:focus-visible,
  textarea:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
  .actions {
    display: flex;
    gap: 4px;
  }
  .edit {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: 4px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 6px;
  }
  /* On the task's own panel, whose contrast with the dim text is under 4.5:1 in the dark theme. */
  .note {
    margin-right: auto;
    font-size: 12px;
  }
  .note.over {
    font-weight: 700;
  }
  textarea {
    resize: vertical;
    padding: 4px 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bar);
    color: var(--text);
    font: inherit;
  }
  button {
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--panel);
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
</style>
