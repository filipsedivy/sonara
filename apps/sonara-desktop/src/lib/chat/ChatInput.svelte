<script lang="ts">
  import { Composition } from "./ime";
  import { MAX_PROMPT_CHARS, MAX_PROMPT_UNITS, promptLength, promptProblem } from "./model";

  // The message box. Enter (or Send) queues the message, also while a task runs; Shift+Enter
  // starts a new line. The text is kept when the backend refuses the message.
  let {
    enabled,
    hint,
    onsend,
  }: {
    enabled: boolean;
    /** Why the box is disabled, when it is. */
    hint: string | null;
    onsend: (prompt: string) => Promise<boolean>;
  } = $props();

  let text = $state("");
  let sending = $state(false);
  let box: HTMLTextAreaElement | undefined = $state();

  const composition = new Composition();

  // The count is the one the limit uses: without the outer blanks, as Send judges it. Each is
  // worked out once per change of the text, not once per use.
  const count = $derived(promptLength(text.trim()));
  const over = $derived(count > MAX_PROMPT_CHARS);
  const problem = $derived(promptProblem(text));
  const canSend = $derived(enabled && !sending && problem === null);

  /** Moves the focus into the box (after a Clear, for instance); false when it cannot take it. */
  export function focus(): boolean {
    box?.focus();
    return box !== undefined && document.activeElement === box;
  }

  async function send() {
    if (!canSend) return;
    sending = true;
    const sent = text.trim();
    const ok = await onsend(sent);
    sending = false;
    if (ok && text.trim() === sent) text = "";
  }

  function keydown(event: KeyboardEvent) {
    // The Enter that commits an input method's candidate is not a send.
    if (event.key !== "Enter" || event.shiftKey || event.altKey || composition.handles(event)) return;
    event.preventDefault();
    void send();
  }
</script>

<form
  onsubmit={(event) => {
    event.preventDefault();
    void send();
  }}
>
  <label for="chat-input" class="sr">Message</label>
  <textarea
    id="chat-input"
    rows="3"
    bind:this={box}
    bind:value={text}
    onkeydown={keydown}
    oncompositionstart={composition.start}
    oncompositionend={composition.end}
    maxlength={MAX_PROMPT_UNITS}
    disabled={!enabled}
    aria-describedby="chat-input-count"
    placeholder="Describe the music or the change you want (Enter sends, Shift+Enter adds a line)"
  ></textarea>
  <div class="row">
    <span id="chat-input-count" class="note" class:over>
      {#if hint && !enabled}{hint}{:else}{count} / {MAX_PROMPT_CHARS}{over ? ": too long" : ""}{/if}
    </span>
    <button type="submit" class="primary" disabled={!canSend}>Send</button>
  </div>
</form>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  textarea {
    resize: none;
    min-height: 54px;
    padding: 4px 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bar);
    color: var(--text);
    font: inherit;
  }
  textarea::placeholder {
    color: var(--text-dim);
  }
  textarea:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .note {
    color: var(--text-dim);
    font-size: 12px;
  }
  .note.over {
    color: var(--text);
    font-weight: 700;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
  button {
    padding: 4px 16px;
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
</style>
