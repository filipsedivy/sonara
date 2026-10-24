<script lang="ts">
  // A line edit as the prototype's `QLineEdit` with `editingFinished`: the text is sent on Enter
  // or focus loss, when it differs from `value`; Escape reverts. While the field has focus or
  // holds typed text, a new `value` from the project does not replace it.
  let {
    id,
    label,
    value,
    onchange,
  }: {
    id: string;
    label: string;
    value: string;
    onchange: (text: string) => void;
  } = $props();

  let draft: string | null = $state(null);

  function commit() {
    if (draft === null) return;
    const text = draft;
    draft = null;
    if (text !== value) onchange(text);
  }
</script>

<label for={id}>{label}</label>
<input
  {id}
  type="text"
  autocomplete="off"
  spellcheck="false"
  value={draft ?? value}
  oninput={(event) => (draft = event.currentTarget.value)}
  onblur={commit}
  onkeydown={(event) => {
    if (event.key === "Enter") {
      // In a dialog, Enter also submits its form, after the text is in.
      commit();
    } else if (event.key === "Escape" && draft !== null) {
      event.preventDefault();
      event.stopPropagation();
      draft = null;
    }
  }}
/>

<style>
  input {
    min-width: 0;
    padding: 2px 4px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--bar);
    color: var(--text);
    font: inherit;
    -webkit-user-select: text;
    user-select: text;
  }
</style>
