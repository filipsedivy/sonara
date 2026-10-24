// The window's single-key shortcuts ("Application"): Space plays or stops, Home
// returns to the start, R records, L loops, C toggles the metronome. They are page
// shortcuts, not menu accelerators, so that they never fire while a text field has focus; a
// key with ⌘, ⌃ or ⌥ is not one of them.

export type TransportShortcut = "play" | "home" | "record" | "loop" | "metronome";

/** Input types that take no typed text. */
const NOT_TEXT = ["button", "checkbox", "radio", "range", "color", "submit", "reset", "file", "image"];

/** Whether `target` takes typed text (a text field, an editable combo, a text area). */
export function isTextField(target: EventTarget | null): boolean {
  const element = target as { tagName?: unknown; type?: unknown; isContentEditable?: unknown } | null;
  if (!element || typeof element.tagName !== "string") return false;
  if (element.isContentEditable === true) return true;
  const tag = element.tagName.toUpperCase();
  if (tag === "TEXTAREA") return true;
  if (tag !== "INPUT") return false;
  const type = typeof element.type === "string" ? element.type.toLowerCase() : "text";
  return !NOT_TEXT.includes(type);
}

/** The transport shortcut of a key press, or null. */
export function transportShortcut(event: {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  target: EventTarget | null;
}): TransportShortcut | null {
  if (event.metaKey || event.ctrlKey || event.altKey || isTextField(event.target)) return null;
  switch (event.key) {
    case " ":
      return "play";
    case "Home":
      return "home";
    case "r":
    case "R":
      return "record";
    case "l":
    case "L":
      return "loop";
    case "c":
    case "C":
      return "metronome";
    default:
      return null;
  }
}

/** A text field's own undo step: ⌘Z undoes its typing, ⇧⌘Z redoes it. */
export type FieldUndo = "undo" | "redo";

/**
 * The field-local undo of a key press, or null. As the prototype's Qt line edits and spin
 * boxes, a focused text field takes ⌘Z and ⇧⌘Z for its own typing, even with nothing to undo, so
 * the project's undo applies only outside text fields (confirmed on the prototype on 2026-10-23,
 * offscreen with the main window: ⌘Z in the inspector's name field took back the typed text and
 * left the project's "Undo Rename Track" as it was). The Edit menu's Undo, chosen with the pointer,
 * still undoes the project, as the prototype's did.
 */
export function fieldUndo(event: {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  target: EventTarget | null;
}): FieldUndo | null {
  if (!event.metaKey || event.ctrlKey || event.altKey || event.key.toLowerCase() !== "z") return null;
  if (!isTextField(event.target)) return null;
  return event.shiftKey ? "redo" : "undo";
}

/**
 * Installs the field-local undo on `target` (a window): its keys are taken before any handler of
 * the field's own and before the native menu, which gets only a key the page leaves alone (step
 * 10); `run` performs the field's undo or redo (WebKit's editing command). Returns the removal.
 */
export function installFieldUndo(
  target: Pick<Window, "addEventListener" | "removeEventListener">,
  run: (command: FieldUndo) => void,
): () => void {
  const keydown = (event: KeyboardEvent) => {
    const command = fieldUndo(event);
    if (!command) return;
    event.preventDefault();
    event.stopPropagation();
    run(command);
  };
  target.addEventListener("keydown", keydown, true);
  return () => target.removeEventListener("keydown", keydown, true);
}
