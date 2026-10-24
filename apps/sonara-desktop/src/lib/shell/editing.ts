// The Edit menu acts on the focused editor (sonara-app's `menus`). The prototype's
// Edit menu took ⌘D and ⌘E window-wide, so in the timeline ⌘D duplicated each clip after itself as
// separate steps and ⌘E split only at the playhead, and in the piano roll ⌘D duplicated the clips
// instead of the notes. Here the web view gives the page every key before the menu sees it: the
// focused editor takes its own ⌘D, ⌘E, ⌘A and Delete, and the menu's Split, Duplicate and Delete
// run the focused editor's own operation: the timeline's (also with no editor focused, as the
// prototype's items acted on the selected clips from anywhere), the piano roll's notes (it splits
// nothing), and nothing while a device card has focus.
//
// The editors register their operations here, and the page tells the desktop which editor has the
// focus and whether a dialog is open (`set_menu_context`): the menu words and enables its items
// for that editor, and does nothing while a dialog is open, as the prototype's modal dialogs let
// no menu item run.

/** The editor with the keyboard focus, as the desktop reads it (sonara-app's `EditFocus`). */
export type EditFocus = "none" | "timeline" | "piano-roll" | "devices";

/** The Edit menu's clip items (their `menu-command`). */
export type EditCommand = "split-clips" | "duplicate-clips" | "delete-clips";

/** What the page reports for the menu. */
export interface MenuContext {
  readonly focus: EditFocus;
  readonly modal: boolean;
}

/** An editor's operations behind the Edit menu: its own ⌘E, ⌘D and Delete. */
export interface EditorOperations {
  /** Absent for an editor that splits nothing. */
  readonly split?: () => unknown;
  readonly duplicate: () => unknown;
  readonly remove: () => unknown;
}

/** The editors that act on the Edit menu's items. */
export type EditorName = "timeline" | "piano-roll";

/** The editor whose region (`data-editor`) holds `element`, or "none". */
export function editorOf(element: Element | null): EditFocus {
  const region = element && typeof element.closest === "function" ? element.closest("[data-editor]") : null;
  const name = region?.getAttribute("data-editor");
  return name === "timeline" || name === "piano-roll" || name === "devices" ? name : "none";
}

/** The editors' operations, registered while they are shown. */
export class EditTargets {
  private readonly editors = new Map<EditorName, EditorOperations>();

  /** `operations` are `name`'s until the returned function is called. */
  register(name: EditorName, operations: EditorOperations): () => void {
    this.editors.set(name, operations);
    return () => {
      if (this.editors.get(name) === operations) this.editors.delete(name);
    };
  }

  /**
   * Runs the Edit menu's `command` on the editor that `focus` names: the piano roll's notes, the
   * timeline's clips (also with no editor focused), nothing for a device card. Whether it ran.
   */
  run(command: EditCommand, focus: EditFocus): boolean {
    if (focus === "devices") return false;
    const operations = this.editors.get(focus === "piano-roll" ? "piano-roll" : "timeline");
    if (!operations) return false;
    if (command === "duplicate-clips") void operations.duplicate();
    else if (command === "delete-clips") void operations.remove();
    else if (operations.split) void operations.split();
    else return false;
    return true;
  }
}

/** Sends the menu context to the desktop whenever it changes. */
export class MenuContextReporter {
  private sent: MenuContext | null = null;

  constructor(
    private readonly send: (context: MenuContext) => Promise<void>,
    private readonly onError: (error: unknown) => void = () => {},
  ) {}

  update(context: MenuContext): void {
    if (this.sent && this.sent.focus === context.focus && this.sent.modal === context.modal) return;
    this.sent = context;
    this.send(context).catch((error: unknown) => {
      // Sent again at the next change.
      this.sent = null;
      this.onError(error);
    });
  }
}

/** The dialogs open now (each `Dialog.svelte` while it shows). */
let openDialogs = 0;
const modalListeners = new Set<() => void>();

/** A dialog opened; the returned function closes it. */
export function dialogOpened(): () => void {
  openDialogs += 1;
  for (const listener of modalListeners) listener();
  let closed = false;
  return () => {
    if (closed) return;
    closed = true;
    openDialogs -= 1;
    for (const listener of modalListeners) listener();
  };
}

/** Whether a dialog is open. */
export function isModal(): boolean {
  return openDialogs > 0;
}

/** Calls `listener` whenever a dialog opens or closes; the returned function stops it. */
export function onModalChange(listener: () => void): () => void {
  modalListeners.add(listener);
  return () => modalListeners.delete(listener);
}
