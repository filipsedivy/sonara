<script lang="ts">
  import { onMount, untrack } from "svelte";
  import * as backend from "../backend";
  import { benchHooks } from "../bench/hooks";
  import { commitOnRelease, LiveGesture, type GestureDeps } from "../gesture";
  import type { Session } from "../session.svelte";
  import ContextMenu from "../shell/ContextMenu.svelte";
  import type { MenuItem } from "../shell/menu";
  import { GRID_CHOICES, type GridName } from "../state/snap";
  import { pyG } from "../time/format";
  import type { Edit, EditOutcome, MidiClipView, NoteView, OpenProject, TrackView } from "../types";
  import {
    chained,
    GridGestures,
    gridWheel,
    keyboardWheel,
    type Mods,
    patched,
    type RollState,
    type RollWheel,
    rulerWheel,
    VelocityGestures,
  } from "./gestures";
  import {
    beatX,
    centerOnContent,
    clampView,
    contentExtent,
    NOTE_LENGTHS,
    NoteIndex,
    type NoteLengthName,
    type RollContext,
    type RollView,
    scrollBy,
    timelineToContent,
    yPitch,
    zoomPitch,
    zoomTime,
  } from "./model";
  import { MAX_NOTE_OPTIONS, noteOptionId, noteOptions, revealNote } from "./noteList";
  import { keyboardWidth, RollCanvas, RULER_H, VELOCITY_H } from "./paint";

  // The piano roll (`PianoRollPanel` in sonara/ui/piano_roll/panel.py): the toolbar,
  // the keyboard (GM names on drum tracks), the ruler, the note grid, the velocity lane and the
  // scrollbars, editing the current clip of the selection. The model, the gestures and the
  // painting are in model.ts, gestures.ts and paint.ts. Over the grid, a list box of its notes for
  // assistive technology (noteList.ts).
  let { session }: { session: Session } = $props();

  const EMPTY_HINT = "No MIDI clip selected.\nDouble-click a MIDI clip in the timeline to edit its notes here.";

  /** The current clip of the selection when it is a MIDI clip of the project, with its track. */
  function findClip(project: OpenProject | null, clipId: string | null): { track: TrackView; clip: MidiClipView } | null {
    if (!project || !clipId) return null;
    for (const track of project.tracks) {
      for (const clip of track.clips) if (clip.id === clipId) return clip.type === "midi" ? { track, clip } : null;
    }
    return null;
  }

  const project = $derived(session.view?.project ?? null);
  const found = $derived(findClip(project, session.selection.currentClip));
  const clip = $derived(found?.clip ?? null);
  const drums = $derived(
    found !== null && session.drumNames !== null && found.track.instrument?.plugin_id === session.drumNames.pluginId,
  );
  const drumLabels = $derived(session.drumNames?.labels ?? new Map<number, string>());

  // The clip's notes as indexed; rebuilt only when the view brings other notes.
  let indexKept: { notes: readonly NoteView[]; index: NoteIndex } | null = null;
  const viewIndex = $derived.by(() => {
    if (!clip) return (indexKept = null);
    if (!indexKept || indexKept.notes !== clip.notes) indexKept = { notes: clip.notes, index: new NoteIndex(clip.notes) };
    return indexKept.index;
  });

  // A finished gesture's notes, held until the project view has its last edit (`until`, the
  // session version; Infinity while its edits are still landing).
  let held: { clipId: string; index: NoteIndex; until: number } | null = $state.raw(null);
  const index = $derived.by((): NoteIndex | null => {
    const holding = held;
    return holding && clip && holding.clipId === clip.id ? holding.index : viewIndex;
  });

  const rollContext = $derived<RollContext | null>(
    clip ? { clip, signatures: project?.time_signatures ?? null, snap: session.snap } : null,
  );

  let selectedKept: { notes: readonly string[]; set: Set<string> } = { notes: [], set: new Set() };
  const selected = $derived.by(() => {
    const notes = session.selection.notes;
    if (selectedKept.notes !== notes) selectedKept = { notes, set: new Set(notes) };
    return selectedKept.set;
  });

  let gridBox: HTMLDivElement | undefined = $state();
  let gridCanvas: HTMLCanvasElement | undefined = $state();
  let keysCanvas: HTMLCanvasElement | undefined = $state();
  let rulerCanvas: HTMLCanvasElement | undefined = $state();
  let laneCanvas: HTMLCanvasElement | undefined = $state();
  let roll: RollCanvas | null = null;
  let viewport = $state({ width: 0, height: 0 });
  let cursor = $state("crosshair");
  let menu: { x: number; y: number; items: MenuItem[] } | null = $state(null);
  /** Grows whenever a gesture changed what the grid draws (the benchmark's sample filter). */
  let changes = 0;

  const view = $derived(session.roll.view);
  const extent = $derived(contentExtent(clip, index));

  function setView(next: RollView) {
    const clamped = clampView(next, viewport, extent);
    if (clamped !== session.roll.view) session.setRoll({ view: clamped });
  }

  function applyWheel(action: RollWheel, x: number, y: number) {
    if (action.kind === "time") setView(zoomTime(view, action.factor, viewport, extent, x));
    else if (action.kind === "pitch") setView(zoomPitch(view, action.factor, viewport, extent, y));
    else setView(scrollBy(view, action.dx, action.dy, viewport, extent));
  }

  function fit() {
    if (clip && index) setView(centerOnContent(view, clip, index, drums, viewport));
  }

  // The view stays inside the content when the clip or the grid changes (`set_viewport`).
  $effect(() => {
    const limits = { viewport, extent };
    untrack(() => {
      if (limits.viewport.width > 0) setView(session.roll.view);
    });
  });

  // A new current clip, or one opened again (`openClipEditor`), is centred on its notes once the
  // grid has its size.
  $effect(() => {
    const shown = clip;
    const notes = index;
    const size = viewport;
    const centred = session.roll.centredClip;
    untrack(() => {
      if (!shown || !notes || size.width <= 0 || size.height <= 0 || centred === shown.id) return;
      session.setRoll({ centredClip: shown.id, view: centerOnContent(session.roll.view, shown, notes, drums, size) });
    });
  });

  // A held gesture ends once the view has its last edit, or when the clip changes.
  $effect(() => {
    const version = session.view?.version ?? -1;
    const shown = clip?.id ?? null;
    untrack(() => {
      if (held && (held.clipId !== shown || version >= held.until)) held = null;
    });
  });

  // -- the gestures --------------------------------------------------------------------------------

  /** The previous gesture's end (its last edit and seal): later edits wait for it. */
  let ending: Promise<void> = Promise.resolve();
  /** The highest session version a gesture's edits reached. */
  let gestureVersion = 0;

  function send(edit: Edit): Promise<EditOutcome> {
    return backend.edit(edit).then((outcome) => {
      gestureVersion = Math.max(gestureVersion, outcome.version);
      return outcome;
    });
  }

  const deps: GestureDeps = {
    send,
    seal: backend.sealUndo,
    nextFrame: (callback) => requestAnimationFrame(() => callback()),
    onError: (error) => session.fail(error),
  };

  function rollState(): RollState | null {
    if (!rollContext || !index) return null;
    const settings = session.roll;
    return {
      ctx: rollContext,
      index,
      view: settings.view,
      selected,
      lengthName: settings.lengthName,
      velocity: settings.velocity,
      strength: settings.strength / 100,
    };
  }

  const actions = {
    select: (ids: string[]) => session.selectNotes(ids),
    run: async (edit: Edit): Promise<EditOutcome | null> => {
      await ending;
      try {
        return await backend.edit(edit);
      } catch (error) {
        session.fail(error);
        return null;
      }
    },
    live: () => {
      gestureVersion = 0;
      const after = ending;
      return new LiveGesture(chained(deps, () => after));
    },
    commit: async (edit: Edit): Promise<number | null> => {
      await ending;
      gestureVersion = 0;
      const committed = await commitOnRelease(deps, edit);
      return committed ? gestureVersion : null;
    },
    message: (text: string) => session.message(text),
    changed: () => {
      changes += 1;
      pushFrame();
    },
    seek: (beat: number) => session.seek(beat),
    setView,
  };

  const grid = new GridGestures(rollState, actions);
  const lane = new VelocityGestures(rollState, actions);

  /** Holds `notes` (the index with a gesture's result) until the view has `done`'s version. */
  function hold(notes: NoteView[], done: Promise<number | null>) {
    if (!clip) return;
    const entry = { clipId: clip.id, index: new NoteIndex(notes), until: Infinity };
    held = entry;
    void done.then((version) => {
      if (held !== entry) return;
      if (version === null || (session.view?.version ?? -1) >= version) held = null;
      else held = { ...entry, until: version };
    });
  }

  function pushFrame() {
    roll?.update({
      ctx: rollContext,
      index,
      view,
      drums,
      drumLabels,
      selected,
      preview: grid.preview,
      velocities: lane.preview,
      rubber: grid.drag?.rect ?? null,
    });
  }

  $effect(() => {
    void rollContext;
    void index;
    void view;
    void drums;
    void drumLabels;
    void selected;
    pushFrame();
  });

  const mods = (event: MouseEvent | KeyboardEvent): Mods => ({
    shift: event.shiftKey,
    command: event.metaKey || event.ctrlKey,
    alt: event.altKey,
  });

  function local(event: MouseEvent, element: Element | undefined): { x: number; y: number } {
    const box = element?.getBoundingClientRect();
    return { x: event.clientX - (box?.left ?? 0), y: event.clientY - (box?.top ?? 0) };
  }

  /** The pointer stays with the element until it is released (a synthetic one cannot be held). */
  function capture(event: PointerEvent) {
    try {
      (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    } catch {
      // Not an active pointer.
    }
  }

  function gridDown(event: PointerEvent) {
    gridBox?.focus();
    // A second press of a double-click is the double-click (Qt sends no press for it), and
    // ⌃-click opens the context menu on macOS.
    if (event.button !== 0 || event.ctrlKey || event.detail >= 2 || !clip) return;
    capture(event);
    const { x, y } = local(event, gridBox);
    grid.press(x, y, mods(event));
  }

  function gridMove(event: PointerEvent) {
    if (!clip) return;
    const { x, y } = local(event, gridBox);
    cursor = grid.move(x, y, mods(event));
  }

  /** The release, or a cancel (which has no button): the drag ends either way. */
  async function gridUp(event: PointerEvent) {
    const drag = grid.drag;
    if ((event.type !== "pointercancel" && event.button !== 0) || drag === null) return;
    const live = (drag.mode === "move" || drag.mode === "resize") && drag.moved && drag.gesture !== null;
    if (live && index) {
      // The notes stay where the drag left them until the view has the last edit.
      const notes = patched(index, grid.preview);
      grid.preview.clear();
      const done = grid.release(mods(event));
      ending = done.catch(() => undefined);
      hold(
        notes,
        done.then(
          () => gestureVersion || null,
          () => null,
        ),
      );
      pushFrame();
      await done;
      return;
    }
    await grid.release(mods(event));
  }

  function gridDoubleClick(event: MouseEvent) {
    if (!clip) return;
    const { x, y } = local(event, gridBox);
    void grid.doubleClick(x, y);
  }

  function gridMenu(event: MouseEvent) {
    event.preventDefault();
    if (!clip) return;
    const { x, y } = local(event, gridBox);
    const items = grid.contextMenu(x, y);
    if (items.length > 0) menu = { x: event.clientX, y: event.clientY, items };
  }

  function gridKey(event: KeyboardEvent) {
    if (grid.key(event.key, mods(event))) {
      event.preventDefault();
      event.stopPropagation();
    }
  }

  function gridWheelEvent(event: WheelEvent) {
    event.preventDefault();
    const { x, y } = local(event, gridBox);
    applyWheel(gridWheel(event), x, y);
  }

  function keysDown(event: PointerEvent) {
    if (event.button !== 0 || !clip) return;
    capture(event);
    const y = local(event, keysCanvas).y;
    grid.selectPitch(y);
    const pitch = yPitch(view, y);
    if (found && pitch >= 0 && pitch <= 127) auditionOn(found.track.id, pitch);
  }

  function keysMove(event: PointerEvent) {
    if ((event.buttons & 1) === 0 || !found) return;
    const pitch = yPitch(view, local(event, keysCanvas).y);
    if (pitch >= 0 && pitch <= 127) auditionOn(found.track.id, pitch);
  }

  function keysUp() {
    auditionOff();
  }

  function keysWheel(event: WheelEvent) {
    event.preventDefault();
    const { x, y } = local(event, keysCanvas);
    applyWheel(keyboardWheel(event), x, y);
  }

  // -- the keyboard's audition (the user hears which pitch a key is) -------------------------

  /** Serializes the keyboard's NoteOn before the NoteOff that ends it: a fast click must not
   * let the backend's async command queue reorder them into a stuck note. */
  let auditionChain: Promise<void> = Promise.resolve();
  let auditioning: { trackId: string; pitch: number } | null = null;

  function auditionOn(trackId: string, pitch: number) {
    if (auditioning?.trackId === trackId && auditioning.pitch === pitch) return;
    auditionOff();
    auditioning = { trackId, pitch };
    auditionChain = auditionChain
      .then(() => backend.auditionNote(trackId, pitch, session.roll.velocity, true))
      .catch(() => {});
  }

  function auditionOff() {
    const held = auditioning;
    if (!held) return;
    auditioning = null;
    auditionChain = auditionChain
      .then(() => backend.auditionNote(held.trackId, held.pitch, 0, false))
      .catch(() => {});
  }

  function rulerDown(event: PointerEvent) {
    if (event.button !== 0 || !clip) return;
    grid.seekAt(local(event, rulerCanvas).x);
  }

  function rulerWheelEvent(event: WheelEvent) {
    event.preventDefault();
    const { x, y } = local(event, rulerCanvas);
    applyWheel(rulerWheel(event), x, y);
  }

  function laneDown(event: PointerEvent) {
    if (event.button !== 0 || !clip) return;
    capture(event);
    const { x, y } = local(event, laneCanvas);
    lane.press(x, y, VELOCITY_H);
  }

  function laneMove(event: PointerEvent) {
    if (!lane.active || (event.buttons & 1) === 0) return;
    const { x, y } = local(event, laneCanvas);
    lane.move(x, y, VELOCITY_H);
  }

  async function laneUp(event: PointerEvent) {
    if ((event.type !== "pointercancel" && event.button !== 0) || !lane.active || !index) return;
    const before = index;
    const values = new Map(lane.preview);
    const done = lane.release();
    ending = done.then(
      () => undefined,
      () => undefined,
    );
    const changed = new Map([...values].filter(([id, v]) => before.note(id)?.velocity !== v));
    if (changed.size > 0) {
      hold(
        before.notes.map((note) => (changed.has(note.id) ? { ...note, velocity: changed.get(note.id) ?? note.velocity } : note)),
        done.then((result) => result?.version ?? null),
      );
    }
    await done;
  }

  // -- the notes for assistive technology -----------------------------------------------

  /** How long the grid rests before the note list follows it: a scroll, a zoom or a drag's edits
   * change no DOM. */
  const LIST_DELAY_MS = 120;
  let listSource: { clip: MidiClipView | null; index: NoteIndex | null; view: RollView } = $state.raw({
    clip: null,
    index: null,
    view: untrack(() => session.roll.view),
  });
  $effect(() => {
    const next = { clip, index, view };
    const timer = setTimeout(() => (listSource = next), LIST_DELAY_MS);
    return () => clearTimeout(timer);
  });
  /** The note whose option has (or last had) the focus; it stays an option wherever it is. */
  let focusedNote: string | null = $state(null);
  const noteList = $derived.by(() => {
    const source = listSource;
    if (!source.clip || !source.index || source.clip.id !== clip?.id) return [];
    const naming = { drums, drumLabels };
    const signatures = project?.time_signatures ?? null;
    return noteOptions(source.index, source.clip, signatures, source.view, viewport, selected, naming, focusedNote);
  });

  // A focused note that went (deleted, or another clip shown) gives the focus back to the grid.
  $effect(() => {
    const current = index;
    untrack(() => {
      if (focusedNote === null || current?.has(focusedNote)) return;
      focusedNote = null;
      const active = document.activeElement;
      if (!active || active === document.body) gridBox?.focus({ preventScroll: true });
    });
  });

  /** A focused option (VoiceOver's cursor; never the pointer) shows its note whole. */
  function noteFocused(noteId: string) {
    focusedNote = noteId;
    const note = index?.note(noteId);
    if (note) setView(revealNote(session.roll.view, note, viewport));
  }

  /** An option's activation without a pointer (VoiceOver's press) selects its note. */
  function noteOptionClick(event: MouseEvent, noteId: string) {
    if (event.detail === 0) session.selectNotes([noteId]);
  }

  /**
   * The grid's layers are placed by the view, so its box never scrolls itself: a scroll into view
   * (a focused note, VoiceOver's cursor) scrolls the view by as much instead.
   */
  function scrolled(event: Event) {
    const element = event.currentTarget as HTMLElement;
    const dx = element.scrollLeft;
    const dy = element.scrollTop;
    element.scrollTop = 0;
    element.scrollLeft = 0;
    if (dx !== 0 || dy !== 0) setView(scrollBy(session.roll.view, dx, dy, viewport, extent));
  }

  // -- the toolbar -------------------------------------------------------------------------------

  const title = $derived.by(() => {
    if (!found) return "Piano Roll";
    const loop = found.clip.loop_length !== null ? `  ·  loop ${pyG(found.clip.loop_length)} beats` : "";
    return `${found.track.name} — ${found.clip.name}${loop}`;
  });

  function setNumber(key: "velocity" | "strength", input: HTMLInputElement, min: number, max: number) {
    const value = Math.round(Number(input.value));
    if (!Number.isFinite(value)) return;
    session.setRoll({ [key]: Math.min(max, Math.max(min, value)) });
  }

  const TRANSPOSE: [string, number, string][] = [
    ["−12", -12, "Octave down"],
    ["−1", -1, "Semitone down"],
    ["+1", 1, "Semitone up"],
    ["+12", 12, "Octave up"],
  ];

  // -- the playheads and scrollbars ----------------------------------------------------------------

  const playheadX = $derived.by(() => {
    if (!clip) return null;
    const content = timelineToContent(clip, session.playhead.beat);
    if (content === null) return null;
    const x = beatX(view, content);
    return x >= 0 && x <= viewport.width ? x : null;
  });

  const hbar = $derived.by(() => {
    const min = -view.ppb;
    const max = Math.max(0, extent * view.ppb - viewport.width * 0.25);
    const page = Math.max(1, viewport.width);
    const span = max - min + page;
    return { min, max, thumb: Math.max(24, (page * page) / span), at: max > min ? (view.x0 * view.ppb - min) / (max - min) : 0 };
  });
  const vbar = $derived.by(() => {
    const max = Math.max(0, 128 * view.rowH - viewport.height);
    const page = Math.max(1, viewport.height);
    return { max, thumb: Math.max(24, (page * page) / (max + page)), at: max > 0 ? view.y0 / max : 0 };
  });

  function scrollbar(axis: "x" | "y", event: PointerEvent) {
    const track = event.currentTarget as HTMLElement;
    capture(event);
    const box = track.getBoundingClientRect();
    const length = axis === "x" ? box.width : box.height;
    const thumb = axis === "x" ? hbar.thumb : vbar.thumb;
    const scrollTo = (pointer: number) => {
      const fraction = Math.min(1, Math.max(0, (pointer - thumb / 2) / Math.max(1, length - thumb)));
      const current = session.roll.view;
      if (axis === "x") setView({ ...current, x0: (hbar.min + fraction * (hbar.max - hbar.min)) / current.ppb });
      else setView({ ...current, y0: fraction * vbar.max });
    };
    const move = (e: PointerEvent) => scrollTo(axis === "x" ? e.clientX - box.left : e.clientY - box.top);
    const up = () => {
      track.removeEventListener("pointermove", move);
      track.removeEventListener("pointerup", up);
    };
    track.addEventListener("pointermove", move);
    track.addEventListener("pointerup", up);
    move(event);
  }

  onMount(() => {
    session.loadDrumNames();
    if (!gridCanvas || !keysCanvas || !rulerCanvas || !laneCanvas || !gridBox) return;
    roll = new RollCanvas({ grid: gridCanvas, keys: keysCanvas, ruler: rulerCanvas, lane: laneCanvas });
    const box = gridBox;
    const observer = new ResizeObserver(() => {
      viewport = { width: box.clientWidth, height: box.clientHeight };
      roll?.requestDraw();
    });
    observer.observe(box);
    viewport = { width: box.clientWidth, height: box.clientHeight };
    pushFrame();
    benchHooks.pianoRoll = {
      canvas: roll,
      grid: box,
      clipId: () => clip?.id ?? null,
      index: () => index,
      view: () => session.roll.view,
      setView: (next) => session.setRoll({ view: next }),
      changes: () => changes,
      held: () => held !== null,
    };
    // The Edit menu's Duplicate and Delete are the grid's own ⌘D and Delete.
    const stopEditing = session.editing.register("piano-roll", {
      duplicate: () => grid.duplicate(),
      remove: () => grid.deleteSelected(),
    });
    return () => {
      observer.disconnect();
      roll?.destroy();
      roll = null;
      benchHooks.pianoRoll = null;
      stopEditing();
      auditionOff();
    };
  });
</script>

<section class="piano-roll" aria-label="Piano Roll" data-editor="piano-roll">
  <div class="toolbar" role="toolbar" aria-label="Piano roll tools">
    <span class="title" data-testid="piano-roll-title">{title}</span>
    <span class="gap"></span>
    <button
      type="button"
      title="Snap edits to the grid (hold Alt/Option to bypass)"
      aria-pressed={session.snap.enabled}
      onclick={() => session.setSnapEnabled(!session.snap.enabled)}>Snap</button
    >
    <select
      title="Editing grid"
      aria-label="Grid"
      value={session.snap.gridName}
      onchange={(event) => session.setGrid(event.currentTarget.value as GridName)}
    >
      {#each GRID_CHOICES as [name] (name)}<option value={name}>{name}</option>{/each}
    </select>
    <label>
      Length
      <select
        title="Length of new notes"
        disabled={!clip}
        value={session.roll.lengthName}
        onchange={(event) => session.setRoll({ lengthName: event.currentTarget.value as NoteLengthName })}
      >
        {#each NOTE_LENGTHS as [name] (name)}<option value={name}>{name}</option>{/each}
      </select>
    </label>
    <label>
      Vel
      <input
        type="number"
        min="1"
        max="127"
        title="Velocity of new notes"
        disabled={!clip}
        value={session.roll.velocity}
        onchange={(event) => setNumber("velocity", event.currentTarget, 1, 127)}
      />
    </label>
    <span class="gap"></span>
    <button
      type="button"
      title="Quantize selected notes (all if none selected) to the grid"
      disabled={!clip}
      onclick={() => void grid.quantize()}>Quantize</button
    >
    <label class="strength">
      <input
        type="number"
        min="0"
        max="100"
        title="Quantize strength"
        aria-label="Quantize strength"
        value={session.roll.strength}
        onchange={(event) => setNumber("strength", event.currentTarget, 0, 100)}
      />
      %
    </label>
    <span class="gap"></span>
    {#each TRANSPOSE as [text, semitones, tip] (semitones)}
      <button type="button" title="Transpose: {tip}" onclick={() => void grid.transpose(semitones)}>{text}</button>
    {/each}
    <span class="gap"></span>
    <button type="button" title="Select all notes (Cmd/Ctrl+A)" disabled={!clip} onclick={() => grid.selectAll()}>Select All</button>
    <span class="spacer"></span>
    <button
      type="button"
      title="Zoom out (Cmd/Ctrl+wheel)"
      aria-label="Zoom out"
      onclick={() => setView(zoomTime(view, 1 / 1.4, viewport, extent))}>−</button
    >
    <button
      type="button"
      title="Zoom in (Cmd/Ctrl+wheel)"
      aria-label="Zoom in"
      onclick={() => setView(zoomTime(view, 1.4, viewport, extent))}>+</button
    >
    <button type="button" title="Fit the clip's notes into view" onclick={fit}>Fit</button>
  </div>
  <div class="body">
    <div
      class="editor"
      class:hidden={!clip}
      style:grid-template-columns="{keyboardWidth(drums)}px minmax(0, 1fr) 12px"
      style:grid-template-rows="{RULER_H}px minmax(0, 1fr) {VELOCITY_H}px 12px"
    >
      <div class="corner"></div>
      <div class="ruler-box">
        <canvas
          bind:this={rulerCanvas}
          class="fill"
          aria-label="Piano roll ruler"
          onpointerdown={rulerDown}
          onwheel={rulerWheelEvent}
        ></canvas>
        {#if playheadX !== null}
          <div class="playhead wide" style:transform="translateX({playheadX}px)"></div>
        {/if}
      </div>
      <div class="cell"></div>
      <canvas
        bind:this={keysCanvas}
        class="fill keys"
        aria-label={drums ? "Drum notes" : "Keyboard"}
        onpointerdown={keysDown}
        onpointermove={keysMove}
        onpointerup={keysUp}
        onpointercancel={keysUp}
        onwheel={keysWheel}
      ></canvas>
      <!-- The grid is one focusable widget with its own keys; its notes are a list box of
           options over it for assistive technology, whose keys reach the grid's. -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <div
        bind:this={gridBox}
        class="grid-box"
        role="application"
        aria-label="Note grid"
        tabindex="0"
        style:cursor={cursor}
        onpointerdown={gridDown}
        onpointermove={gridMove}
        onpointerup={gridUp}
        onpointercancel={gridUp}
        ondblclick={gridDoubleClick}
        oncontextmenu={gridMenu}
        onkeydown={gridKey}
        onwheel={gridWheelEvent}
        onscroll={scrolled}
      >
        <canvas bind:this={gridCanvas} class="fill" data-testid="note-grid-canvas" aria-hidden="true"></canvas>
        <!-- svelte-ignore a11y_interactive_supports_focus -->
        <div
          class="note-list"
          role="listbox"
          aria-label="Notes"
          aria-multiselectable="true"
          title={noteList.length >= MAX_NOTE_OPTIONS ? `The first ${MAX_NOTE_OPTIONS} notes in view` : undefined}
        >
          {#each noteList as option (option.id)}
            <!-- The grid takes the option's keys; the click is VoiceOver's press. -->
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div
              id={noteOptionId(option.id)}
              class="note-option"
              role="option"
              tabindex="-1"
              data-note={option.id}
              aria-selected={option.selected}
              aria-label={option.label}
              aria-posinset={option.position}
              aria-setsize={option.count}
              style:transform="translate({option.rect.x}px, {option.rect.y}px)"
              style:width="{option.rect.width}px"
              style:height="{option.rect.height}px"
              onfocus={() => noteFocused(option.id)}
              onclick={(event) => noteOptionClick(event, option.id)}
            ></div>
          {/each}
        </div>
        {#if playheadX !== null}
          <div class="playhead" style:transform="translateX({playheadX}px)" data-testid="roll-playhead"></div>
        {/if}
      </div>
      <div class="vbar" onpointerdown={(event) => scrollbar("y", event)} role="presentation">
        <div class="thumb" style:height="{vbar.thumb}px" style:top="{vbar.at * (viewport.height - vbar.thumb)}px"></div>
      </div>
      <div class="vel-label">Vel</div>
      <canvas
        bind:this={laneCanvas}
        class="fill"
        aria-label="Velocity lane"
        onpointerdown={laneDown}
        onpointermove={laneMove}
        onpointerup={laneUp}
        onpointercancel={laneUp}
      ></canvas>
      <div class="cell"></div>
      <div class="cell"></div>
      <div class="hbar" onpointerdown={(event) => scrollbar("x", event)} role="presentation">
        <div class="thumb" style:width="{hbar.thumb}px" style:left="{hbar.at * (viewport.width - hbar.thumb)}px"></div>
      </div>
      <div class="cell"></div>
    </div>
    {#if !clip}
      <p class="empty" data-testid="piano-roll-empty">{EMPTY_HINT}</p>
    {/if}
  </div>
</section>
{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} label="Notes" onclose={() => (menu = null)} />
{/if}

<style>
  .piano-roll {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--panel);
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    min-height: 30px;
    padding: 3px 8px;
    border-bottom: 1px solid var(--border);
    background: var(--panel-alt);
    overflow: hidden;
    white-space: nowrap;
  }
  .title {
    font-weight: 600;
  }
  .toolbar button {
    height: 22px;
    padding: 0 7px;
    border: 1px solid transparent;
    border-radius: 4px;
    background: transparent;
    color: var(--text);
    font: inherit;
  }
  .toolbar button:hover:not(:disabled) {
    background: var(--panel);
  }
  .toolbar button:disabled {
    color: var(--text-dim);
  }
  .toolbar button[aria-pressed="true"] {
    background: var(--accent);
    color: #fff;
  }
  select,
  input {
    height: 22px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
  input {
    width: 52px;
    padding: 0 4px;
  }
  label {
    display: flex;
    align-items: center;
    gap: 4px;
    color: var(--text);
  }
  .gap {
    width: 8px;
    flex: none;
  }
  .spacer {
    flex: 1;
  }
  .body {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .editor {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .editor.hidden {
    visibility: hidden;
  }
  .empty {
    position: absolute;
    inset: 0;
    margin: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-dim);
    font-size: 13px;
    text-align: center;
    white-space: pre-line;
  }
  .fill {
    display: block;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
  }
  .corner,
  .cell {
    background: var(--panel);
  }
  .ruler-box,
  .grid-box {
    position: relative;
    overflow: hidden;
    min-width: 0;
    min-height: 0;
  }
  .grid-box {
    outline: none;
  }
  .grid-box:focus {
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 47%, transparent);
  }
  .grid-box canvas {
    position: absolute;
    inset: 0;
  }
  /* The notes' options are transparent: the canvas draws the notes, and the pointer goes to it. */
  .note-list {
    position: absolute;
    inset: 0;
    pointer-events: none;
    contain: strict;
  }
  .note-option {
    position: absolute;
    top: 0;
    left: 0;
    outline: none;
  }
  .note-option:focus-visible {
    box-shadow: 0 0 0 1px var(--text);
  }
  .playhead {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 1.5px;
    background: var(--playhead);
    pointer-events: none;
  }
  .playhead.wide {
    width: 2px;
  }
  .vel-label {
    padding: 3px 6px;
    color: var(--text-dim);
    text-align: right;
    background: var(--panel);
  }
  .vbar,
  .hbar {
    position: relative;
    background: var(--panel);
  }
  .thumb {
    position: absolute;
    border-radius: 6px;
    background: var(--panel-alt);
  }
  .vbar .thumb {
    left: 2px;
    right: 2px;
  }
  .hbar .thumb {
    top: 2px;
    bottom: 2px;
  }
  .thumb:hover {
    background: var(--text-dim);
  }
</style>
