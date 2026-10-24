// Focus for the page's dialogs and menus ("HTML with labelled fields and focus trapping"). A `<dialog>` shown with `showModal()` makes the rest of
// the window inert, but Tab can still leave it for the web view's edge, and the dialogs leave the
// page by being unmounted rather than closed, so WebKit never restores the focus itself. Here Tab
// and ⇧Tab cycle inside the dialog, and the focus returns to where it was when the dialog opened.

const TABBABLE = [
  "a[href]",
  "button:not([disabled])",
  'input:not([disabled]):not([type="hidden"])',
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(", ");

/** The elements Tab reaches inside `root`, in document order: enabled, shown and not inert. */
export function tabbables(root: ParentNode): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(TABBABLE)].filter(
    (element) => element.tabIndex >= 0 && element.getClientRects().length > 0 && element.closest("[inert]") === null,
  );
}

/**
 * The element Tab or ⇧Tab moves to so that the focus stays inside `items`: the first after the
 * last, the last before the first, or null when the browser's own move stays inside.
 */
export function wrapTarget<T>(items: readonly T[], active: T | null, backwards: boolean): T | null {
  if (items.length === 0) return null;
  const at = active === null ? -1 : items.indexOf(active);
  if (at < 0) return backwards ? items[items.length - 1] : items[0];
  if (backwards && at === 0) return items[items.length - 1];
  if (!backwards && at === items.length - 1) return items[0];
  return null;
}

/** A Svelte action: Tab and ⇧Tab cycle through the focusable elements of `node`. */
export function trapTab(node: HTMLElement) {
  const keydown = (event: KeyboardEvent) => {
    if (event.key !== "Tab" || event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) return;
    const items = tabbables(node);
    const active = document.activeElement instanceof HTMLElement && node.contains(document.activeElement) ? document.activeElement : null;
    const target = wrapTarget(items, active, event.shiftKey);
    if (items.length === 0 || target) event.preventDefault();
    target?.focus();
  };
  node.addEventListener("keydown", keydown);
  return {
    destroy() {
      node.removeEventListener("keydown", keydown);
    },
  };
}

/**
 * Remembers the focused element now (call it while the dialog or menu is created, before it takes
 * the focus) and returns the restore: the focus goes back to that element if it is still in the
 * page and `when` agrees (by default always).
 */
export function rememberFocus(when: (active: Element | null) => boolean = () => true): () => void {
  const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const restore = () => {
    if (previous?.isConnected && document.activeElement !== previous && when(document.activeElement)) {
      previous.focus({ preventScroll: true });
    }
  };
  return () => {
    restore();
    // A modal dialog that is still in the top layer keeps the rest inert until it leaves the page.
    if (document.activeElement !== previous) setTimeout(restore, 0);
  };
}
