// A popup menu's entries (ContextMenu.svelte): an item that runs (with an optional tooltip), a
// submenu, a disabled item, or a separator.

export type MenuItem =
  | { readonly label: string; readonly run: () => void; readonly title?: string }
  | { readonly label: string; readonly items: readonly MenuItem[] }
  | { readonly label: string; readonly disabled: true }
  | "separator";
