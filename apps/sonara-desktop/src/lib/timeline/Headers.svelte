<script lang="ts">
  import { tick } from "svelte";
  import { trackOptionLabel } from "../a11y/labels";
  import * as backend from "../backend";
  import type { Session } from "../session.svelte";
  import ContextMenu from "../shell/ContextMenu.svelte";
  import type { MenuItem } from "../shell/menu";
  import { barLength, barToBeat, beatToBar } from "../time/signatures";
  import type { Edit, OpenProject, TrackView } from "../types";
  import { blend, parseHex, toHex, trackColor } from "./colors";
  import { palette } from "../theme.svelte";
  import {
    BUTTON,
    HEADER_WIDTH,
    INDENT,
    REORDER_THRESHOLD,
    detailText,
    geometry,
    hasArm,
    hasButtons,
    insertionIndex,
    reorderIndex,
  } from "./headers";
  import type { Row, TrackLayout } from "./layout";

  // The track headers (the prototype's `TrackHeaders`): DOM rows beside the
  // arrangement, scrolled with it. Each row has the colour swatch, the folder indent, the name
  // (bold, dimmed when muted), on tall rows the detail line, and M, S and R as real buttons.
  // A click selects the track, ⌘ or ⇧ adds it, a click below the rows clears the
  // selection; a vertical drag reorders; a double-click on the name renames it
  // in place; the context menu has the prototype's items; the wheel scrolls the
  // rows with the canvas.
  //
  // For assistive technology (ADR 0009's "track list" check) the rows are a list box
  // "Tracks" whose options read "Drums 1, 0.0 dB, selected, 1 of 100": one tab stop (the current
  // track's row), ↑ and ↓ select the previous or next track. An option's children are
  // presentational, so M, S, R and the rename field sit in a layer of their own over the rows, a
  // group per track, where they stay real buttons with their pressed state.
  let {
    session,
    project,
    layout,
    scrollY,
    onscroll,
  }: {
    session: Session;
    project: OpenProject;
    layout: TrackLayout;
    scrollY: number;
    onscroll: (dy: number) => void;
  } = $props();

  let box: HTMLDivElement | undefined = $state();
  let picker: HTMLInputElement | undefined = $state();
  let pickerTrack: string | null = null;
  let renaming: string | null = $state(null);
  let renameInput: HTMLInputElement | undefined = $state();
  let reorderTarget: number | null = $state(null);
  let press: { trackId: string; y: number } | null = null;
  let menu: { x: number; y: number; items: MenuItem[]; label: string } | null = $state(null);

  async function run(edit: Edit) {
    try {
      return await backend.edit(edit);
    } catch (error) {
      session.fail(error);
      return null;
    }
  }

  function localY(event: MouseEvent): number {
    return event.clientY - (box?.getBoundingClientRect().top ?? 0);
  }

  /**
   * A row's background: alternating, tinted towards the accent when selected (`paintEvent`), in
   * the theme shown (the headers are a panel; A1).
   */
  function rowBackground(index: number, selected: boolean): string {
    const colors = palette();
    const base = parseHex(index % 2 ? colors.panelAlt : colors.panel);
    return toHex(selected ? blend(base, parseHex(colors.accent), 0.18) : base);
  }

  /** The name's right edge from the row's: left of M, as `geometry_for` places it. */
  function nameRight(track: TrackView): number {
    return 8 + BUTTON + (BUTTON + 3) + (hasArm(track) ? BUTTON + 3 : 0) + 6;
  }

  // -- selection and reordering --------------------------------------------------

  function rowDown(event: PointerEvent, row: Row) {
    pointerAt = performance.now();
    if (event.button !== 0 || event.ctrlKey || event.detail >= 2) return;
    session.selectTrack(row.trackId, event.metaKey || event.shiftKey);
    press = { trackId: row.trackId, y: localY(event) };
    window.addEventListener("pointermove", dragMove);
    window.addEventListener("pointerup", dragEnd, { once: true });
  }

  function dragMove(event: PointerEvent) {
    if (!press) return;
    const y = localY(event);
    if (Math.abs(y - press.y) < REORDER_THRESHOLD && reorderTarget === null) return;
    reorderTarget = insertionIndex(layout.rows, y, scrollY);
  }

  function dragEnd() {
    window.removeEventListener("pointermove", dragMove);
    const target = reorderTarget;
    const pressed = press;
    press = null;
    reorderTarget = null;
    if (!pressed || target === null) return;
    const current = project.tracks.findIndex((t) => t.id === pressed.trackId);
    const index = current < 0 ? null : reorderIndex(current, target);
    if (index !== null) void run({ op: "move_track", track_id: pressed.trackId, index });
  }

  function emptyDown(event: PointerEvent) {
    if (event.button !== 0 || (event.target !== box && event.target !== rowsBox)) return;
    session.selectTrack(null);
  }

  // -- the track list for assistive technology ----------------------------------------

  let rowsBox: HTMLDivElement | undefined = $state();

  /** The element id of a track's option. */
  const optionId = (trackId: string) => `track-option-${trackId}`;

  /** The list's one tab stop: the current track's row, else the first. */
  const tabStop = $derived(
    layout.rows.find((row) => row.trackId === session.selection.currentTrack)?.trackId ?? layout.rows[0]?.trackId ?? null,
  );

  /** ↑ and ↓ select the previous or next track and move the focus to its row, scrolled into view. */
  function listKey(event: KeyboardEvent) {
    if ((event.key !== "ArrowDown" && event.key !== "ArrowUp") || event.metaKey || event.ctrlKey || event.altKey) return;
    const rows = layout.rows;
    if (rows.length === 0) return;
    const focused = (event.target as HTMLElement).closest<HTMLElement>("[role=option]")?.dataset.track ?? tabStop;
    const at = rows.findIndex((row) => row.trackId === focused);
    const step = event.key === "ArrowDown" ? 1 : -1;
    const next = rows[at < 0 ? 0 : Math.min(rows.length - 1, Math.max(0, at + step))];
    event.preventDefault();
    event.stopPropagation();
    session.selectTrack(next.trackId);
    reveal(next);
    document.getElementById(optionId(next.trackId))?.focus({ preventScroll: true });
  }

  /** Scrolls the rows so that `row` shows whole. */
  function reveal(row: Row) {
    const height = box?.clientHeight ?? 0;
    if (row.top < scrollY) onscroll(row.top - scrollY);
    else if (row.top + row.height > scrollY + height) onscroll(row.top + row.height - height - scrollY);
  }

  /** When the pointer last pressed a row: its focus is the click's, which scrolls nothing. */
  let pointerAt = -Infinity;

  /** A row focused without the pointer (VoiceOver's cursor, the keys) scrolls into view. */
  function optionFocus(row: Row) {
    if (performance.now() - pointerAt > 500) reveal(row);
  }

  /** An activation without a pointer (VoiceOver's press): selects the track, as a click does. */
  function optionClick(event: MouseEvent, trackId: string) {
    if (event.detail === 0) session.selectTrack(trackId);
  }

  /**
   * The rows are placed by the view's scroll, so their boxes never scroll themselves: a scroll
   * into view (a focus, VoiceOver's cursor) scrolls the view by as much instead.
   */
  function scrolled(event: Event) {
    const element = event.currentTarget as HTMLElement;
    const dy = element.scrollTop;
    element.scrollTop = 0;
    element.scrollLeft = 0;
    if (dy !== 0) onscroll(dy);
  }

  const insertionY = $derived.by(() => {
    if (reorderTarget === null) return 0;
    const row = layout.rows[reorderTarget];
    return (row ? row.top : layout.contentHeight) - scrollY;
  });

  // -- renaming ---------------------------------------------------------------------------

  async function startRename(trackId: string) {
    renaming = trackId;
    await tick();
    renameInput?.focus();
    renameInput?.select();
  }

  function nameDoubleClick(event: MouseEvent, trackId: string) {
    event.stopPropagation();
    void startRename(trackId);
  }

  function finishRename(track: TrackView, commit: boolean) {
    if (renaming !== track.id) return;
    const text = renameInput?.value.trim() ?? "";
    renaming = null;
    if (commit && text && text !== track.name) void run({ op: "rename_track", track_id: track.id, name: text });
  }

  function renameKey(event: KeyboardEvent, track: TrackView) {
    event.stopPropagation();
    if (event.key === "Enter") finishRename(track, true);
    else if (event.key === "Escape") finishRename(track, false);
  }

  // -- the context menu -------------------------------------------------------------------

  function pickColor(track: TrackView, index: number) {
    if (!picker) return;
    pickerTrack = track.id;
    picker.value = toHex(trackColor(index, track.color));
    picker.click();
  }

  function colorChosen(event: Event) {
    const trackId = pickerTrack;
    pickerTrack = null;
    const color = (event.currentTarget as HTMLInputElement).value.toLowerCase();
    if (trackId && /^#[0-9a-f]{6}$/.test(color)) void run({ op: "set_track_color", track_id: trackId, color });
  }

  /** "Add MIDI Clip at Playhead": the playhead's bar, named after the track (`_add_clip`). */
  function addClip(track: TrackView) {
    const signatures = project.time_signatures;
    const bar = Math.trunc(beatToBar(signatures, session.playhead.beat) + 1e-9);
    void run({
      op: "create_midi_clip",
      track_id: track.id,
      start: barToBeat(signatures, bar),
      length: barLength(signatures, bar),
      name: track.name,
    });
  }

  function rowMenu(event: MouseEvent, track: TrackView, index: number) {
    event.preventDefault();
    const items: MenuItem[] = [
      { label: "Rename…", run: () => void startRename(track.id) },
      { label: "Colour…", run: () => pickColor(track, index) },
    ];
    if (track.color) {
      items.push({ label: "Automatic Colour", run: () => void run({ op: "set_track_color", track_id: track.id, color: null }) });
    }
    items.push({ label: "Show Devices", run: () => session.showDevices(track.id) });
    if (track.kind === "instrument") items.push({ label: "Add MIDI Clip at Playhead", run: () => addClip(track) });
    if (track.kind === "audio") {
      items.push({
        label: "Import Audio…",
        run: () => {
          backend.importAudioDialog(track.id, session.playhead.beat).catch((error: unknown) => session.fail(error));
        },
      });
    }
    items.push("separator", { label: "Delete Track", run: () => void run({ op: "delete_track", track_id: track.id }) });
    menu = { x: event.clientX, y: event.clientY, items, label: track.name };
  }

  // -- the wheel ----------------------------------------------------------------------------

  function wheel(event: WheelEvent) {
    event.preventDefault();
    onscroll(event.deltaY);
  }
</script>

<!-- The box clears the selection on a click below the rows and scrolls them with the wheel; the
     list box takes the keys. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:this={box}
  class="headers"
  style:width="{HEADER_WIDTH}px"
  class:reordering={reorderTarget !== null}
  onpointerdown={emptyDown}
  onwheel={wheel}
  onscroll={scrolled}
>
  <!-- The options are the list's tab stops (a roving tab index), not the list itself. -->
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div
    bind:this={rowsBox}
    class="rows"
    role="listbox"
    aria-label="Tracks"
    aria-multiselectable="true"
    onkeydown={listKey}
    onscroll={scrolled}
  >
    {#each layout.rows as row (row.trackId)}
      {@const track = project.tracks[row.index]}
      {@const g = geometry(row)}
      {#if track}
        {@const selected = session.selection.tracks.includes(row.trackId)}
        <!-- The list box takes the option's keys (listKey); the click is VoiceOver's press. -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          id={optionId(row.trackId)}
          class="header"
          class:selected
          class:muted={track.mixer.mute}
          style:background={rowBackground(row.index, selected)}
          role="option"
          tabindex={row.trackId === tabStop ? 0 : -1}
          aria-selected={selected}
          aria-label={trackOptionLabel(track)}
          aria-posinset={row.index + 1}
          aria-setsize={layout.rows.length}
          data-track={row.trackId}
          style:transform="translateY({row.top - scrollY}px)"
          style:height="{row.height}px"
          onpointerdown={(event) => rowDown(event, row)}
          oncontextmenu={(event) => rowMenu(event, track, row.index)}
          onclick={(event) => optionClick(event, row.trackId)}
          onfocus={() => optionFocus(row)}
        >
          <span
            class="swatch"
            style:left="{g.left}px"
            style:height="{row.height - 8}px"
            style:background={toHex(trackColor(row.index, track.color))}
          ></span>
          {#if row.depth > 0}<span class="guide" style:left="{g.left - INDENT / 2}px"></span>{/if}
          {#if renaming !== track.id}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <span
              class="name"
              data-depth={row.depth}
              style:left="{g.left + 10}px"
              style:top="{g.nameTop}px"
              style:right="{nameRight(track)}px"
              ondblclick={(event) => nameDoubleClick(event, track.id)}>{track.name}</span
            >
          {/if}
          {#if g.detail}
            <span class="detail" style:left="{g.left + 10}px">{detailText(track)}</span>
          {/if}
        </div>
      {/if}
    {/each}
  </div>
  <div class="controls-layer" onscroll={scrolled}>
    {#each layout.rows as row (row.trackId)}
      {@const track = project.tracks[row.index]}
      {@const g = geometry(row)}
      {#if track && (hasButtons(track) || renaming === track.id)}
        <!-- A right-click on M, S or R opens the row's menu, as anywhere on the row. -->
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <div
          class="controls"
          role="group"
          aria-label="{track.name} controls"
          data-track={row.trackId}
          style:transform="translateY({row.top - scrollY}px)"
          style:height="{row.height}px"
          oncontextmenu={(event) => rowMenu(event, track, row.index)}
        >
          {#if renaming === track.id}
            <input
              bind:this={renameInput}
              class="rename"
              aria-label="Track name"
              value={track.name}
              style:left="{g.left + 7}px"
              style:top="{g.nameTop - 1}px"
              style:right="{nameRight(track) - 3}px"
              onpointerdown={(event) => event.stopPropagation()}
              onkeydown={(event) => renameKey(event, track)}
              onblur={() => finishRename(track, true)}
            />
          {/if}
          {#if hasButtons(track)}
            <div class="buttons" style:top="{g.buttonTop}px">
              <button
                type="button"
                class="mute"
                aria-label="Mute"
                title="Mute"
                aria-pressed={track.mixer.mute}
                onpointerdown={(event) => event.stopPropagation()}
                onclick={() => void run({ op: "set_track_mixer", track_id: track.id, mute: !track.mixer.mute })}>M</button
              >
              <button
                type="button"
                class="solo"
                aria-label="Solo"
                title="Solo"
                aria-pressed={track.mixer.solo}
                onpointerdown={(event) => event.stopPropagation()}
                onclick={() => void run({ op: "set_track_mixer", track_id: track.id, solo: !track.mixer.solo })}>S</button
              >
              {#if hasArm(track)}
                <button
                  type="button"
                  class="arm"
                  aria-label="Arm for recording"
                  title="Arm for recording"
                  aria-pressed={track.input.armed}
                  onpointerdown={(event) => event.stopPropagation()}
                  onclick={() => void run({ op: "set_track_input", track_id: track.id, armed: !track.input.armed })}
                  >R</button
                >
              {/if}
            </div>
          {/if}
        </div>
      {/if}
    {/each}
  </div>
  {#if reorderTarget !== null}
    <div class="insertion" data-testid="insertion" style:transform="translateY({insertionY}px)"></div>
  {/if}
  <input bind:this={picker} class="picker" type="color" tabindex="-1" aria-hidden="true" onchange={colorChosen} />
</div>
{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} label={menu.label} onclose={() => (menu = null)} />
{/if}

<style>
  .headers {
    position: relative;
    overflow: hidden;
    border-right: 1px solid var(--border);
    background: var(--panel);
  }
  .headers.reordering {
    cursor: grabbing;
  }
  .rows,
  .controls-layer {
    position: absolute;
    inset: 0;
    overflow: hidden;
    outline: none;
  }
  /* The buttons' layer lets the pointer through to the rows, except on its own controls. */
  .controls-layer,
  .controls {
    pointer-events: none;
  }
  .controls :global(button),
  .controls :global(input) {
    pointer-events: auto;
  }
  .header,
  .controls {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
  }
  .header {
    border-bottom: 1px solid var(--border);
    overflow: hidden;
    outline: none;
  }
  .header:focus-visible {
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .swatch {
    position: absolute;
    top: 4px;
    width: 4px;
    border-radius: 2px;
  }
  .guide {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1px;
    background: var(--grid-beat);
  }
  .name,
  .rename {
    position: absolute;
    height: 18px;
    font-size: 13px;
    font-weight: 600;
    line-height: 18px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .header.muted .name {
    color: var(--text-dim);
  }
  .rename {
    height: 20px;
    padding: 0 2px;
    border: 1px solid var(--accent);
    border-radius: 3px;
    background: var(--bg);
    color: var(--text);
    font-family: inherit;
    -webkit-user-select: text;
    user-select: text;
  }
  .detail {
    position: absolute;
    top: 26px;
    right: 8px;
    height: 14px;
    color: var(--text-dim);
    font-size: 10px;
    line-height: 14px;
    white-space: pre;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .buttons {
    position: absolute;
    right: 8px;
    display: flex;
    gap: 3px;
  }
  .buttons button {
    width: 18px;
    height: 16px;
    padding: 0;
    border: 0;
    border-radius: 3px;
    background: var(--window);
    color: var(--text-dim);
    font: inherit;
    font-size: 10px;
    line-height: 16px;
    text-align: center;
  }
  .buttons button[aria-pressed="true"] {
    color: #15171a;
  }
  .mute[aria-pressed="true"] {
    background: var(--meter-yellow);
  }
  .solo[aria-pressed="true"] {
    background: var(--meter-green);
  }
  .arm[aria-pressed="true"] {
    background: var(--meter-red);
  }
  .insertion {
    position: absolute;
    top: -1px;
    left: 4px;
    right: 4px;
    height: 2px;
    background: var(--accent);
    pointer-events: none;
  }
  .picker {
    position: absolute;
    width: 1px;
    height: 1px;
    left: 0;
    bottom: 0;
    opacity: 0;
    pointer-events: none;
  }
</style>
