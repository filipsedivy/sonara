<script lang="ts">
  import { onMount } from "svelte";
  import * as backend from "../backend";
  import type { Session } from "../session.svelte";
  import ContextMenu from "../shell/ContextMenu.svelte";
  import type { MenuItem } from "../shell/menu";
  import { browserDevices, browserRoot, listDirectory } from "./api";
  import { DRAG_THRESHOLD, fileDropZone } from "./drag";
  import {
    FIRST_OPEN,
    activation,
    activationLabel,
    clickSelection,
    draggedFiles,
    fileRows,
    searchTree,
    treeRows,
    type BrowserItem,
    type FileEntry,
    type TreeRow,
  } from "./tree";

  // The Browser pane (the prototype's `BrowserPanel`): the Devices tab
  // with the instruments, the effects and their presets, searchable; a double-click or the
  // context menu creates an instrument track or adds an effect to the current track.
  // The Files tab lists `~/Music` (or `~`) through sonara-app, folders and the audio
  // files the import takes; a double-click imports a file into a new track at beat 0 on an
  // import job, and files drag onto the timeline or the window. There is no
  // preview playback.
  let { session }: { session: Session } = $props();

  let tab: "devices" | "files" = $state("devices");

  // -- Devices --------------------------------------------------------------------------------------

  let devices: BrowserItem[] = $state.raw([]);
  let search = $state("");
  let open: Set<string> = $state(new Set(FIRST_OPEN));
  let focusedKey = $state<string | null>(null);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  const found = $derived(searchTree(devices, search));
  const rows = $derived(treeRows(devices, found.shown, new Set([...open, ...found.opened])));

  function toggle(key: string) {
    const next = new Set(open);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    open = next;
  }

  async function activate(item: BrowserItem) {
    const action = activation(item, session.selection.currentTrack);
    if (!action) return;
    if (action.kind === "error") {
      session.fail(action.message);
      return;
    }
    try {
      const outcome = await backend.edit(action.edit);
      if (action.kind === "track") {
        const trackId = outcome.result.track_id;
        if (typeof trackId === "string") session.selectTrack(trackId);
      } else {
        const deviceId = outcome.result.device_id;
        if (typeof deviceId === "string") session.setCurrentDevice(deviceId);
        session.showDevices(action.trackId);
      }
    } catch (error) {
      session.fail(error);
    }
  }

  function deviceMenu(event: MouseEvent, item: BrowserItem) {
    const label = activationLabel(item);
    if (!label) return;
    event.preventDefault();
    menu = { x: event.clientX, y: event.clientY, items: [{ label, run: () => void activate(item) }] };
  }

  function deviceKey(event: KeyboardEvent, row: TreeRow<BrowserItem>) {
    const index = rows.findIndex((r) => r.key === row.key);
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const next = rows[index + (event.key === "ArrowDown" ? 1 : -1)];
      if (next) focusRow("device", next.key);
    } else if (event.key === "ArrowRight" && row.hasChildren && !open.has(row.key)) {
      event.preventDefault();
      toggle(row.key);
    } else if (event.key === "ArrowLeft" && row.hasChildren && open.has(row.key)) {
      event.preventDefault();
      toggle(row.key);
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (row.item.plugin_id) void activate(row.item);
      else toggle(row.key);
    }
  }

  // -- Files ----------------------------------------------------------------------------------------

  let root = $state<string | null>(null);
  let listings: Map<string, FileEntry[]> = $state.raw(new Map());
  let openFolders: Set<string> = $state(new Set());
  let selected: string[] = $state([]);
  let anchor = $state<string | null>(null);
  const files = $derived(root ? fileRows(root, listings, openFolders) : []);

  async function load(folder: string) {
    try {
      const entries = await listDirectory(folder);
      listings = new Map(listings).set(folder, entries);
    } catch (error) {
      session.fail(error);
    }
  }

  function toggleFolder(entry: FileEntry) {
    const next = new Set(openFolders);
    if (next.has(entry.path)) next.delete(entry.path);
    else {
      next.add(entry.path);
      if (!listings.has(entry.path)) void load(entry.path);
    }
    openFolders = next;
  }

  function showFiles() {
    tab = "files";
    if (root !== null) return;
    browserRoot()
      .then((folder) => {
        root = folder;
        return load(folder);
      })
      .catch((error: unknown) => session.fail(error));
  }

  /** BR-04: a file into a new track at beat 0, on an import job (MW-07's path). */
  function importFile(entry: FileEntry) {
    if (entry.folder) {
      toggleFolder(entry);
      return;
    }
    backend.importAudioFiles([entry.path], null, 0).catch((error: unknown) => session.fail(error));
  }

  function fileKey(event: KeyboardEvent, row: TreeRow<FileEntry>) {
    const index = files.findIndex((r) => r.key === row.key);
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const next = files[index + (event.key === "ArrowDown" ? 1 : -1)];
      if (!next) return;
      selected = clickSelection(files, selected, anchor, next.key, { command: false, shift: event.shiftKey });
      if (!event.shiftKey) anchor = next.key;
      focusRow("file", next.key);
    } else if ((event.key === "ArrowRight" || event.key === "ArrowLeft") && row.item.folder) {
      const isOpen = openFolders.has(row.key);
      if ((event.key === "ArrowRight") !== isOpen) {
        event.preventDefault();
        toggleFolder(row.item);
      }
    } else if (event.key === "Enter") {
      event.preventDefault();
      importFile(row.item);
    }
  }

  // A press on a file that moves becomes a drag of the files it carries.
  let press: { x: number; y: number; key: string; pointer: number } | null = null;
  let drag = $state<{ paths: string[]; x: number; y: number } | null>(null);
  let pane: HTMLElement | undefined = $state();

  /** Set when a drag ended, so that the click that follows its release selects nothing. */
  let dragEnded = false;

  function filePointerDown(event: PointerEvent, row: TreeRow<FileEntry>) {
    if (event.button !== 0) return;
    dragEnded = false;
    press = { x: event.clientX, y: event.clientY, key: row.key, pointer: event.pointerId };
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  /** A click without a drag selects (⌘ toggles, ⇧ extends from the anchor). */
  function fileClick(event: MouseEvent, row: TreeRow<FileEntry>) {
    const command = event.metaKey || event.ctrlKey;
    selected = clickSelection(files, selected, anchor, row.key, { command, shift: event.shiftKey });
    if (!event.shiftKey) anchor = row.key;
  }

  function filePointerMove(event: PointerEvent) {
    if (!press || event.pointerId !== press.pointer) return;
    if (!drag) {
      if (Math.hypot(event.clientX - press.x, event.clientY - press.y) < DRAG_THRESHOLD) return;
      const paths = draggedFiles(files, selected, press.key);
      if (paths.length === 0) {
        press = null;
        return;
      }
      drag = { paths, x: event.clientX, y: event.clientY };
    }
    drag = { ...drag, x: event.clientX, y: event.clientY };
    const zone = fileDropZone();
    if (overPane(event)) zone?.leave();
    else zone?.over(drag.paths, event.clientX, event.clientY);
  }

  function overPane(event: PointerEvent): boolean {
    const box = pane?.getBoundingClientRect();
    return !!box && event.clientX >= box.left && event.clientX <= box.right && event.clientY >= box.top && event.clientY <= box.bottom;
  }

  function filePointerUp(event: PointerEvent) {
    if (!press || event.pointerId !== press.pointer) return;
    press = null;
    const ended = drag;
    drag = null;
    if (!ended) return;
    dragEnded = true;
    const zone = fileDropZone();
    if (overPane(event)) zone?.leave();
    else zone?.drop(ended.paths, event.clientX, event.clientY);
  }

  function filePointerCancel() {
    press = null;
    if (drag) fileDropZone()?.leave();
    drag = null;
  }

  // -- shared ---------------------------------------------------------------------------------------

  function focusRow(kind: "device" | "file", key: string) {
    focusedKey = key;
    requestAnimationFrame(() =>
      document.querySelector<HTMLElement>(`[data-${kind}-key="${CSS.escape(key)}"]`)?.focus(),
    );
  }

  onMount(() => {
    browserDevices()
      .then((tree) => (devices = tree))
      .catch((error: unknown) => session.fail(error));
  });
</script>

<div class="browser" bind:this={pane}>
  <!-- Toggle buttons rather than tabs: the Editors pane's tabs are the window's "Devices" tab. -->
  <div class="tabs" role="group" aria-label="Browser view">
    <button type="button" aria-pressed={tab === "devices"} onclick={() => (tab = "devices")}>Devices</button>
    <button type="button" aria-pressed={tab === "files"} onclick={showFiles}>Files</button>
  </div>
  {#if tab === "devices"}
    <div class="tab">
      <input
        class="search"
        type="search"
        aria-label="Search"
        placeholder="Search instruments, effects, presets"
        bind:value={search}
      />
      <ul class="tree" role="tree" aria-label="Instruments and effects">
        {#each rows as row (row.key)}
          <li
            role="treeitem"
            aria-level={row.depth + 1}
            aria-expanded={row.hasChildren ? open.has(row.key) || found.opened.has(row.key) : undefined}
            aria-selected={focusedKey === row.key}
            tabindex={focusedKey === row.key || (focusedKey === null && row.key === "0") ? 0 : -1}
            title={row.item.tooltip || undefined}
            data-device-key={row.key}
            style:padding-left="{4 + row.depth * 14}px"
            onclick={() => (focusedKey = row.key)}
            ondblclick={() => {
              // Qt's tree opens or closes an item on a double-click, and the browser activates it.
              if (row.hasChildren) toggle(row.key);
              void activate(row.item);
            }}
            oncontextmenu={(event) => deviceMenu(event, row.item)}
            onkeydown={(event) => deviceKey(event, row)}
          >
            {#if row.hasChildren}
              <button
                type="button"
                class="twisty"
                tabindex="-1"
                aria-hidden="true"
                onclick={(event) => {
                  event.stopPropagation();
                  toggle(row.key);
                }}>{open.has(row.key) || found.opened.has(row.key) ? "▾" : "▸"}</button
              >
            {:else}
              <span class="twisty"></span>
            {/if}
            <span class="text">{row.item.text}</span>
          </li>
        {/each}
      </ul>
    </div>
  {:else}
    <div class="tab">
      <ul class="tree" role="tree" aria-label="Audio files" aria-multiselectable="true">
        {#each files as row (row.key)}
          <li
            role="treeitem"
            aria-level={row.depth + 1}
            aria-expanded={row.item.folder ? openFolders.has(row.key) : undefined}
            aria-selected={selected.includes(row.key)}
            tabindex={anchor === row.key || (anchor === null && row === files[0]) ? 0 : -1}
            title={row.item.path}
            data-file-key={row.key}
            class:folder={row.item.folder}
            style:padding-left="{4 + row.depth * 14}px"
            onpointerdown={(event) => filePointerDown(event, row)}
            onpointermove={filePointerMove}
            onpointerup={filePointerUp}
            onclick={(event) => {
              if (dragEnded) dragEnded = false;
              else fileClick(event, row);
            }}
            onpointercancel={filePointerCancel}
            ondblclick={() => importFile(row.item)}
            onkeydown={(event) => fileKey(event, row)}
          >
            {#if row.item.folder}
              <button
                type="button"
                class="twisty"
                tabindex="-1"
                aria-hidden="true"
                onpointerdown={(event) => event.stopPropagation()}
                onclick={(event) => {
                  event.stopPropagation();
                  toggleFolder(row.item);
                }}>{openFolders.has(row.key) ? "▾" : "▸"}</button
              >
            {:else}
              <span class="twisty"></span>
            {/if}
            <span class="text">{row.item.name}</span>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</div>
{#if drag}
  <div class="ghost" style:left="{drag.x + 12}px" style:top="{drag.y + 8}px" aria-hidden="true">
    {drag.paths.length === 1 ? drag.paths[0].split("/").at(-1) : `${drag.paths.length} files`}
  </div>
{/if}
{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} label="Browser item" onclose={() => (menu = null)} />
{/if}

<style>
  .browser {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .tabs {
    display: flex;
    gap: 2px;
    padding: 2px 4px 0;
  }
  .tabs button {
    padding: 4px 10px;
    border: 0;
    border-radius: 4px 4px 0 0;
    background: transparent;
    color: var(--text-dim);
    font: inherit;
  }
  .tabs button[aria-pressed="true"] {
    background: var(--bar);
    color: var(--text);
  }
  .tab {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 4px;
    gap: 4px;
    background: var(--bar);
  }
  .search {
    padding: 3px 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--panel);
    color: var(--text);
    font: inherit;
    -webkit-user-select: text;
    user-select: text;
  }
  .tree {
    flex: 1;
    min-height: 0;
    overflow: auto;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 22px;
    white-space: nowrap;
    cursor: default;
  }
  li[aria-selected="true"] {
    background: color-mix(in srgb, var(--accent) 30%, transparent);
  }
  li:focus-visible {
    outline: 1px solid var(--accent);
    outline-offset: -1px;
  }
  li.folder .text {
    color: var(--text);
  }
  .twisty {
    display: inline-block;
    width: 14px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text-dim);
    font-size: 10px;
    text-align: center;
  }
  .text {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ghost {
    position: fixed;
    z-index: 30;
    padding: 2px 8px;
    border-radius: 4px;
    background: var(--panel-alt);
    color: var(--text);
    pointer-events: none;
    box-shadow: 0 2px 6px rgb(0 0 0 / 40%);
  }
</style>
