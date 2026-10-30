<script lang="ts">
  import { tick } from "svelte";
  import type { ChatState, TaskId, TaskView } from "./api";
  import { MAX_PROMPT_CHARS, canMove, phase as phaseOf, promptLength, promptProblem, queuedTasks, runningTask, type MoveDirection } from "./model";

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
  }: {
    thread: ChatState;
    onedit: (id: TaskId, prompt: string) => Promise<boolean>;
    onremove: (id: TaskId) => Promise<boolean>;
    onmove: (id: TaskId, direction: MoveDirection) => Promise<boolean>;
    onstop: () => void;
    onresume: () => void;
  } = $props();

  const queued = $derived(queuedTasks(thread));
  const running = $derived(runningTask(thread));
  const phase = $derived(phaseOf(thread));

  let editing: { id: TaskId; text: string } | null = $state(null);
  let saving = $state(false);
  /** The element to focus once the list shows the next order: a moved task keeps the focus. */
  let focusAfter: { id: TaskId; part: "item" | MoveDirection } | null = null;
  let list: HTMLUListElement | undefined = $state();

  // A task that left the queue (it started, or an agent removed it) ends its editing.
  $effect(() => {
    if (editing && !queued.some((task) => task.id === editing?.id)) editing = null;
  });

  // After a move the keyed list reorders its elements, which drops the focus: put it back.
  $effect(() => {
    void queued.map((task) => task.id).join();
    if (!focusAfter) return;
    const { id, part } = focusAfter;
    focusAfter = null;
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

  function itemKey(event: KeyboardEvent, task: TaskView) {
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
    editing = { id: task.id, text: task.prompt };
    void tick().then(() => list?.querySelector<HTMLTextAreaElement>(`[data-task="${task.id}"] textarea`)?.focus());
  }

  function closeEdit(id: TaskId) {
    editing = null;
    void tick().then(() => list?.querySelector<HTMLElement>(`[data-task="${id}"]`)?.focus({ preventScroll: true }));
  }

  async function save() {
    if (!editing || saving || promptProblem(editing.text) !== null) return;
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
    } else if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      void save();
    }
  }

  /** The first line of a prompt, short enough for a label. */
  function excerpt(text: string): string {
    const line = text.trim().split("\n")[0];
    return line.length > 80 ? `${line.slice(0, 79)}…` : line;
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
                ></textarea>
                <div class="row">
                  <span id="edit-count" class="note" class:over={promptLength(editing.text) > MAX_PROMPT_CHARS}>
                    {promptLength(editing.text)} / {MAX_PROMPT_CHARS}
                  </span>
                  <button type="button" onclick={() => closeEdit(task.id)}>Cancel</button>
                  <button type="button" class="primary" onclick={() => void save()} disabled={saving || promptProblem(editing.text) !== null}>
                    Save
                  </button>
                </div>
              </div>
            {:else}
              <span class="text" title={task.prompt}>{excerpt(task.prompt)}</span>
              <span class="actions">
                <button type="button" onclick={() => startEdit(task)}>Edit</button>
                <button type="button" onclick={() => void onremove(task.id)}>Remove</button>
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
  .note {
    margin-right: auto;
    color: var(--text-dim);
    font-size: 12px;
  }
  .note.over {
    color: var(--text);
    font-weight: 700;
  }
  textarea {
    resize: vertical;
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
