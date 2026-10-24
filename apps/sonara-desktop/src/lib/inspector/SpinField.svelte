<script lang="ts">
  import { parseSpin, spinText, spinValue, stepSpin, type SpinSpec } from "../dialogs/fields";

  // A spin box as the prototype's `QDoubleSpinBox` with keyboard tracking off: typing changes
  // nothing until Enter or focus loss; ↑ and ↓ step (Page Up and Page Down by ten steps) and
  // send at once; Escape reverts. `onchange` runs only when the value differs from `value`, as
  // Qt's `valueChanged` does. While the field has focus or holds typed text, a new `value` from
  // the project does not replace it; otherwise the field shows it at once.
  let {
    id,
    label,
    value,
    spec,
    disabled = false,
    onchange,
  }: {
    id: string;
    label: string;
    value: number;
    spec: SpinSpec;
    disabled?: boolean;
    onchange: (value: number) => void;
  } = $props();

  let draft: string | null = $state(null);
  let focused = $state(false);
  const shown = $derived(spinText(value, spec));
  const text = $derived(draft ?? shown);

  function commit(next: number) {
    draft = null;
    const clamped = spinValue(next, spec);
    if (clamped !== spinValue(value, spec)) onchange(clamped);
  }

  function commitText() {
    if (draft === null) return;
    const parsed = parseSpin(draft, spec);
    if (parsed === null) {
      // Qt keeps the last valid value.
      draft = null;
      return;
    }
    commit(parsed);
  }

  function current(): number {
    if (draft === null) return value;
    return parseSpin(draft, spec) ?? value;
  }

  function keydown(event: KeyboardEvent) {
    const steps: Record<string, number> = { ArrowUp: 1, ArrowDown: -1, PageUp: 10, PageDown: -10 };
    if (event.key in steps) {
      event.preventDefault();
      commit(stepSpin(current(), steps[event.key], spec));
    } else if (event.key === "Enter") {
      // In a dialog, Enter also submits its form, after the value is in.
      commitText();
    } else if (event.key === "Escape" && draft !== null) {
      event.preventDefault();
      event.stopPropagation();
      draft = null;
    }
  }
</script>

<label for={id}>{label}</label>
<span class="spin" class:focused>
  <input
    {id}
    type="text"
    inputmode="decimal"
    role="spinbutton"
    aria-valuemin={spec.min}
    aria-valuemax={spec.max}
    aria-valuenow={value}
    aria-valuetext={shown}
    autocomplete="off"
    spellcheck="false"
    {disabled}
    value={text}
    oninput={(event) => (draft = event.currentTarget.value)}
    onfocus={() => (focused = true)}
    onblur={() => {
      focused = false;
      commitText();
    }}
    onkeydown={keydown}
  />
  <span class="steps" aria-hidden="true">
    <button type="button" tabindex="-1" {disabled} onclick={() => commit(stepSpin(current(), 1, spec))}>▴</button>
    <button type="button" tabindex="-1" {disabled} onclick={() => commit(stepSpin(current(), -1, spec))}>▾</button>
  </span>
</span>

<style>
  .spin {
    display: flex;
    min-width: 0;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--bar);
  }
  .spin.focused {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
  input {
    flex: 1;
    min-width: 0;
    padding: 2px 4px;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-variant-numeric: tabular-nums;
    -webkit-user-select: text;
    user-select: text;
  }
  input:focus-visible {
    outline: none;
  }
  input:disabled {
    color: var(--text-dim);
  }
  .steps {
    display: flex;
    flex-direction: column;
  }
  .steps button {
    flex: 1;
    width: 16px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text-dim);
    font-size: 8px;
    line-height: 1;
  }
  .steps button:hover:not(:disabled) {
    color: var(--text);
  }
</style>
