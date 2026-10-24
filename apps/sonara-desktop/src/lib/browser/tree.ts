// The Browser's logic (the prototype's `BrowserPanel`), as plain
// functions: the Devices tree's search, what a double-click or the context menu does,
// and the Files tab's rows and selection. The tree and the folder
// listings come from sonara-app (`browser.rs`).

import type { Edit } from "../types";

/** One item of the Devices tree (sonara-app `browser::BrowserItem`). */
export interface BrowserItem {
  text: string;
  plugin_id: string | null;
  preset: string | null;
  kind: "instrument" | "effect" | null;
  tooltip: string;
  children: BrowserItem[];
}

/** One entry of a folder (sonara-app `browser::FileEntry`). */
export interface FileEntry {
  name: string;
  path: string;
  folder: boolean;
}

// -- the Devices tree ---------------------------------------------------------------------------------

/** A shown row of a tree: its key (the indices from the root, "0/3/1"), item and depth. */
export interface TreeRow<T> {
  readonly key: string;
  readonly item: T;
  readonly depth: number;
  readonly hasChildren: boolean;
}

/** The keys of the branches open at first: "Instruments" and "Effects". */
export const FIRST_OPEN: ReadonlySet<string> = new Set(["0", "1"]);

/**
 * BR-01, `_apply_filter`: a case-insensitive substring match on each item's text. An item shows
 * when it matches or one of its children shows; a branch with a shown child opens. Every item
 * shows without a search.
 */
export function searchTree(items: readonly BrowserItem[], text: string): { shown: Set<string>; opened: Set<string> } {
  const needle = text.trim().toLowerCase();
  const shown = new Set<string>();
  const opened = new Set<string>();
  const visit = (item: BrowserItem, key: string): boolean => {
    const match = !needle || item.text.toLowerCase().includes(needle);
    let childMatch = false;
    item.children.forEach((child, index) => {
      if (visit(child, `${key}/${index}`)) childMatch = true;
    });
    if (match || childMatch) shown.add(key);
    if (needle && childMatch) opened.add(key);
    return match || childMatch;
  };
  items.forEach((item, index) => visit(item, String(index)));
  return { shown, opened };
}

/** The rows a tree shows: the `shown` items whose parents are all `open`. */
export function treeRows(
  items: readonly BrowserItem[],
  shown: ReadonlySet<string>,
  open: ReadonlySet<string>,
): TreeRow<BrowserItem>[] {
  const rows: TreeRow<BrowserItem>[] = [];
  const walk = (list: readonly BrowserItem[], prefix: string, depth: number) => {
    list.forEach((item, index) => {
      const key = prefix ? `${prefix}/${index}` : String(index);
      if (!shown.has(key)) return;
      rows.push({ key, item, depth, hasChildren: item.children.length > 0 });
      if (open.has(key)) walk(item.children, key, depth + 1);
    });
  };
  walk(items, "", 0);
  return rows;
}

/** What activating an item does. */
export type Activation =
  | { readonly kind: "track"; readonly edit: Edit }
  | { readonly kind: "device"; readonly trackId: string; readonly edit: Edit }
  | { readonly kind: "error"; readonly message: string }
  | null;

/**
 * `_activate`: an instrument or its preset creates an instrument track named after the preset
 * (else the instrument) playing it; an effect or its preset goes on the current track, or says
 * "select a track to add the effect to". A branch does nothing.
 */
export function activation(item: BrowserItem, currentTrack: string | null): Activation {
  if (!item.plugin_id) return null;
  if (item.kind === "instrument") {
    return {
      kind: "track",
      edit: {
        op: "create_track",
        name: item.preset ?? item.text,
        kind: "instrument",
        instrument: item.plugin_id,
        preset: item.preset,
      },
    };
  }
  if (!currentTrack) return { kind: "error", message: "select a track to add the effect to" };
  const edit: Edit = { op: "add_device", track_id: currentTrack, plugin_id: item.plugin_id };
  if (item.preset) edit.preset = item.preset;
  return { kind: "device", trackId: currentTrack, edit };
}

/** The context menu's item for a plugin or preset (`_menu`). */
export function activationLabel(item: BrowserItem): string | null {
  if (!item.plugin_id) return null;
  return item.kind === "instrument" ? "Create Instrument Track" : "Add to Selected Track";
}

// -- the Files tab -----------------------------------------------------------------------------------

/** The rows of the Files tab: the root's entries, and those of each open folder under its own. */
export function fileRows(
  root: string,
  listings: ReadonlyMap<string, readonly FileEntry[]>,
  open: ReadonlySet<string>,
): TreeRow<FileEntry>[] {
  const rows: TreeRow<FileEntry>[] = [];
  const walk = (folder: string, depth: number) => {
    for (const entry of listings.get(folder) ?? []) {
      rows.push({ key: entry.path, item: entry, depth, hasChildren: entry.folder });
      if (entry.folder && open.has(entry.path)) walk(entry.path, depth + 1);
    }
  };
  walk(root, 0);
  return rows;
}

/**
 * A click on the row `key` (the Files tab's extended selection): alone, it selects only that row;
 * with ⌘ it toggles the row; with ⇧ it selects the rows from `anchor` to it.
 */
export function clickSelection(
  rows: readonly { readonly key: string }[],
  selected: readonly string[],
  anchor: string | null,
  key: string,
  mods: { readonly command: boolean; readonly shift: boolean },
): string[] {
  if (mods.shift && anchor !== null) {
    const from = rows.findIndex((row) => row.key === anchor);
    const to = rows.findIndex((row) => row.key === key);
    if (from >= 0 && to >= 0) {
      const [a, b] = from <= to ? [from, to] : [to, from];
      return rows.slice(a, b + 1).map((row) => row.key);
    }
  }
  if (mods.command) return selected.includes(key) ? selected.filter((k) => k !== key) : [...selected, key];
  return [key];
}

/**
 * The audio files a drag that starts on `key` carries: the selected files when the row is
 * selected, else that row alone; folders are not dragged.
 */
export function draggedFiles(
  rows: readonly TreeRow<FileEntry>[],
  selected: readonly string[],
  key: string,
): string[] {
  const files = (keys: readonly string[]) =>
    rows.filter((row) => keys.includes(row.key) && !row.item.folder).map((row) => row.item.path);
  return selected.includes(key) ? files(selected) : files([key]);
}
