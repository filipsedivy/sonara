<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import * as backend from "../backend";
  import { commitOnRelease } from "../gesture";
  import type { Session } from "../session.svelte";
  import ContextMenu from "../shell/ContextMenu.svelte";
  import type { MenuItem } from "../shell/menu";
  import { GRID_CHOICES, type GridName } from "../state/snap";
  import {
    beatToX,
    clampScroll,
    fitView,
    followPlayhead,
    HEIGHT_ORDER,
    scrollRange,
    setScroll,
    setScrollY,
    stepHeight,
    TRACK_HEIGHTS,
    type TimelineView,
    type TrackHeight,
    zoomBy,
  } from "../state/timeline";
  import { pyG } from "../time/format";
  import { signatureText } from "../time/signatures";
  import type { Edit, EditOutcome, TrackView } from "../types";
  import { benchHooks } from "../bench/hooks";
  import { registerFileDropZone } from "../browser/drag";
  import { watchDevicePixelRatio } from "../hidpi";
  import { Arrangement } from "./arrangement";
  import { adjacentClip, clipGroups, clipOptionId, revealClip } from "./clipList";
  import { CanvasGestures, type CanvasState, type Mods, wheelAction } from "./gestures";
  import { TrackLayout, sameRows } from "./layout";
  import { TimelineModel, sameTimeline } from "./model";
  import { drawRuler, RULER_HEIGHT, RulerGestures } from "./ruler";
  import Headers from "./Headers.svelte";

  // The arrangement panel: the toolbar, the "+ Track" menu, the
  // ruler, the track headers (Headers.svelte), the canvas with its gestures
  // and the scrollbars. Over the canvas, a list box of its clips for assistive
  // technology (clipList.ts).
  let {
    session,
    audioSuffixes,
  }: { session: Session; audioSuffixes: readonly string[] } = $props();

  const project = $derived(session.view?.project ?? null);
  // A view that draws as the last one (a mixer refresh) keeps the model and the layout, so the
  // scene is not rebuilt during a fader drag.
  let modelKept: TimelineModel | null = null;
  const model = $derived.by(() => {
    if (!project) return (modelKept = null);
    if (!modelKept || !sameTimeline(modelKept.project, project)) modelKept = new TimelineModel(project);
    return modelKept;
  });
  const trackHeight = $derived(TRACK_HEIGHTS[session.timeline.trackHeight]);
  let layoutKept: { tracks: readonly TrackView[]; layout: TrackLayout } | null = null;
  const layout = $derived.by(() => {
    if (!project) {
      layoutKept = null;
      return null;
    }
    if (!layoutKept || layoutKept.layout.trackHeight !== trackHeight || !sameRows(layoutKept.tracks, project.tracks)) {
      layoutKept = { tracks: project.tracks, layout: new TrackLayout(project.tracks, trackHeight) };
    }
    return layoutKept.layout;
  });

  let arrangementBox: HTMLDivElement | undefined = $state();
  let glCanvas: HTMLCanvasElement | undefined = $state();
  let overlayCanvas: HTMLCanvasElement | undefined = $state();
  let rulerCanvas: HTMLCanvasElement | undefined = $state();
  let arrangement: Arrangement | null = null;
  let size = $state({ width: 0, height: 0 });
  let rulerWidth = $state(0);
  let cursor = $state("default");
  let menu: { x: number; y: number; items: MenuItem[]; label: string } | null = $state(null);
  let renderError: string | null = $state(null);

  // -- the view ------------------------------------------------------------------------------------

  const range = $derived(
    scrollRange(
      session.timeline,
      project?.end_beat ?? 0,
      session.playhead.beat,
      size.width,
      size.height,
      layout?.contentHeight ?? 0,
    ),
  );

  /** Sets the view, its scroll inside the content (`_update_ranges`). */
  function setView(next: TimelineView) {
    const limits = scrollRange(
      next,
      project?.end_beat ?? 0,
      session.playhead.beat,
      size.width,
      size.height,
      layout && next.trackHeight === session.timeline.trackHeight
        ? layout.contentHeight
        : (project?.tracks.length ?? 0) * TRACK_HEIGHTS[next.trackHeight],
    );
    session.setTimeline(clampScroll(next, limits));
  }

  function zoom(factor: number, anchorX = size.width / 2) {
    setView(zoomBy(session.timeline, factor, anchorX));
  }

  function fit() {
    setView(fitView(session.timeline, project?.end_beat ?? 0, size.width));
  }

  // The scroll stays inside the content when the project or the canvas shrinks (`_update_ranges`).
  $effect(() => {
    const limits = range;
    untrack(() => {
      const clamped = clampScroll(session.timeline, limits);
      if (clamped !== session.timeline) session.setTimeline(clamped);
    });
  });

  // Follow: while playing, the view keeps the playhead in sight.
  $effect(() => {
    const playhead = session.playhead;
    untrack(() => {
      const next = followPlayhead(session.timeline, playhead, size.width);
      if (next !== session.timeline) setView(next);
    });
  });

  // A project replaced resets the scroll (`_on_project_replaced`).
  $effect(() => {
    if (session.notice?.kind === "loaded" || session.notice?.kind === "closed") {
      untrack(() => session.setTimeline({ ...session.timeline, scrollX: 0, scrollY: 0 }));
    }
  });

  // -- the canvas ----------------------------------------------------------------------------------

  const mods = (event: MouseEvent | KeyboardEvent): Mods => ({
    shift: event.shiftKey,
    command: event.metaKey,
    alt: event.altKey,
  });

  function canvasState(): CanvasState | null {
    if (!model || !layout) return null;
    return {
      model,
      layout,
      view: session.timeline,
      selection: session.selection,
      width: size.width,
      height: size.height,
      playheadBeat: session.playhead.beat,
      snap: (beat, mode, bypass) => session.snapBeat(beat, mode, bypass),
      gridBeats: (beat) => session.gridBeats(beat),
    };
  }

  async function run(edit: Edit): Promise<EditOutcome | null> {
    try {
      return await backend.edit(edit);
    } catch (error) {
      session.fail(error);
      return null;
    }
  }

  const gestureDeps = {
    send: backend.edit,
    seal: backend.sealUndo,
    nextFrame: (callback: () => void) => requestAnimationFrame(() => callback()),
    onError: (error: unknown) => session.fail(error),
  };

  const gestures = new CanvasGestures(canvasState, {
    selectClips: (ids, additive) => session.selectClips(ids, additive),
    selectTrack: (id, additive) => session.selectTrack(id, additive),
    setCurrentClip: (id) => session.setCurrentClip(id),
    commit: (edit) => commitOnRelease(gestureDeps, edit),
    run,
    openClipEditor: (id) => session.openClipEditor(id),
    showDevices: (id) => session.showDevices(id),
    importAudio: (trackId, beat) => {
      backend.importAudioDialog(trackId, beat).catch((error: unknown) => session.fail(error));
    },
    zoom,
  });

  const ruler = new RulerGestures(
    () => (project ? { project, view: session.timeline, snap: (b, m, by) => session.snapBeat(b, m, by) } : null),
    {
      seek: (beat) => session.seek(beat),
      commit: (edit) => commitOnRelease(gestureDeps, edit),
      run,
    },
  );

  // The arrangement follows the project, the selection, the view and the waveforms.
  $effect(() => {
    const inputs = {
      model,
      layout,
      view: session.timeline,
      selection: session.selection,
      peaksVersion: session.peaksVersion,
    };
    arrangement?.update(inputs);
  });

  /** The pointer stays with the element until it is released; a pointer it cannot capture
   * (an event the page made itself) is followed without capture. */
  function capture(event: PointerEvent) {
    try {
      (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    } catch {
      // Not an active pointer.
    }
  }

  function local(event: MouseEvent): { x: number; y: number } {
    const box = arrangementBox?.getBoundingClientRect();
    return { x: event.clientX - (box?.left ?? 0), y: event.clientY - (box?.top ?? 0) };
  }

  function pointerDown(event: PointerEvent) {
    arrangementBox?.focus();
    // A second press of a double-click is the double-click (Qt sends no press for it), and
    // ⌃-click opens the context menu on macOS.
    if (event.button !== 0 || event.ctrlKey || event.detail >= 2) return;
    capture(event);
    const { x, y } = local(event);
    gestures.press(x, y, mods(event));
    arrangement?.dragChanged();
  }

  function pointerMove(event: PointerEvent) {
    const { x, y } = local(event);
    const dragging = gestures.drag !== null;
    cursor = gestures.move(x, y, mods(event));
    if (dragging) arrangement?.dragChanged();
  }

  async function pointerUp(event: PointerEvent) {
    if (event.button !== 0 || gestures.drag === null) return;
    const done = gestures.release();
    arrangement?.dragChanged();
    await done;
  }

  function doubleClick(event: MouseEvent) {
    const { x, y } = local(event);
    void gestures.doubleClick(x, y);
    arrangement?.dragChanged();
  }

  function contextMenu(event: MouseEvent) {
    event.preventDefault();
    const { x, y } = local(event);
    const items = gestures.contextMenu(x, y);
    if (items.length > 0) menu = { x: event.clientX, y: event.clientY, items, label: "Clip" };
  }

  function wheel(event: WheelEvent) {
    event.preventDefault();
    const action = wheelAction(event);
    const view = session.timeline;
    if (action.kind === "height") setView(stepHeight(view, action.direction));
    else if (action.kind === "zoom") zoom(action.factor, local(event).x);
    else setView(setScrollY(setScroll(view, view.scrollX + action.dx), view.scrollY + action.dy));
  }

  function keyDown(event: KeyboardEvent) {
    // Step 11: ↑ and ↓ go to the previous or next clip (the canvas has no keys of its own there).
    if ((event.key === "ArrowDown" || event.key === "ArrowUp") && !event.metaKey && !event.ctrlKey && !event.altKey) {
      if (model && model.clipCount > 0) {
        event.preventDefault();
        event.stopPropagation();
        void moveToClip(event.key === "ArrowDown" ? 1 : -1);
      }
      return;
    }
    if (gestures.key(event.key, mods(event))) {
      event.preventDefault();
      event.stopPropagation();
      arrangement?.dragChanged();
    }
  }

  // -- the clips for assistive technology -----------------------------------------------

  /** How long the view rests before the clip list follows it: a scroll or a zoom changes no DOM. */
  const LIST_DELAY_MS = 120;
  let listView: TimelineView = $state.raw(untrack(() => session.timeline));
  $effect(() => {
    const view = session.timeline;
    const timer = setTimeout(() => (listView = view), LIST_DELAY_MS);
    return () => clearTimeout(timer);
  });
  /** The clip whose option has (or last had) the focus; it stays an option wherever it is. */
  let focusedClip: string | null = $state(null);
  const selectedClips = $derived(new Set(session.selection.clips));
  const clipList = $derived(
    model && layout ? clipGroups(model, layout, listView, size.width, size.height, selectedClips, focusedClip) : [],
  );

  // A focused clip that went (deleted, or undone) gives the focus back to the canvas.
  $effect(() => {
    const current = model;
    untrack(() => {
      if (focusedClip === null || current?.clip(focusedClip)) return;
      focusedClip = null;
      const active = document.activeElement;
      if (!active || active === document.body) arrangementBox?.focus({ preventScroll: true });
    });
  });

  /** Selects `clipId` as a click on it does: the clip, its track, a MIDI clip's editor. */
  function chooseClip(clipId: string) {
    const placed = model?.clip(clipId);
    if (!placed) return;
    session.selectClips([clipId]);
    if (placed.track.track.id !== session.selection.currentTrack) session.selectTrack(placed.track.track.id);
    if (placed.clip.type === "midi") session.setCurrentClip(clipId);
  }

  /** ↑ and ↓: the previous or next clip, selected, scrolled into view and focused. */
  async function moveToClip(step: 1 | -1) {
    if (!model || !layout) return;
    const from = (document.activeElement as HTMLElement | null)?.dataset.clip ?? session.selection.clips.at(-1) ?? null;
    const target = adjacentClip(model, from, step);
    const row = target ? layout.row(target.track.track.id) : undefined;
    if (!target || !row) return;
    chooseClip(target.clip.id);
    const scroll = revealClip(session.timeline, row, target.clip, size.width, size.height);
    setView(setScrollY(setScroll(session.timeline, scroll.scrollX), scroll.scrollY));
    listView = session.timeline;
    focusedClip = target.clip.id;
    await tick();
    document.getElementById(clipOptionId(target.clip.id))?.focus({ preventScroll: true });
  }

  /** A focused option (the keys, VoiceOver's cursor; never the pointer) shows its clip whole. */
  function clipFocused(clipId: string) {
    focusedClip = clipId;
    const placed = model?.clip(clipId);
    const row = placed ? layout?.row(placed.track.track.id) : undefined;
    if (!placed || !row) return;
    const view = session.timeline;
    const scroll = revealClip(view, row, placed.clip, size.width, size.height);
    if (scroll.scrollX !== view.scrollX || scroll.scrollY !== view.scrollY) {
      setView(setScrollY(setScroll(view, scroll.scrollX), scroll.scrollY));
    }
  }

  /** An option's activation without a pointer (VoiceOver's press) selects its clip. */
  function optionClick(event: MouseEvent, clipId: string) {
    if (event.detail === 0) chooseClip(clipId);
  }

  /**
   * The layers are placed by the view, so their box never scrolls itself: a scroll into view (a
   * focused clip, VoiceOver's cursor) scrolls the view by as much instead.
   */
  function scrolled(event: Event) {
    const element = event.currentTarget as HTMLElement;
    const dx = element.scrollLeft;
    const dy = element.scrollTop;
    element.scrollTop = 0;
    element.scrollLeft = 0;
    if (dx === 0 && dy === 0) return;
    const view = session.timeline;
    setView(setScrollY(setScroll(view, view.scrollX + dx), view.scrollY + dy));
  }

  // -- the ruler ------------------------------------------------------------------------------------

  let rulerFrame = 0;
  function redrawRuler() {
    if (rulerFrame) return;
    rulerFrame = requestAnimationFrame(() => {
      rulerFrame = 0;
      if (!rulerCanvas) return;
      const dpr = window.devicePixelRatio || 1;
      const width = rulerCanvas.clientWidth;
      const w = Math.max(1, Math.round(width * dpr));
      const h = Math.round(RULER_HEIGHT * dpr);
      if (rulerCanvas.width !== w || rulerCanvas.height !== h) {
        rulerCanvas.width = w;
        rulerCanvas.height = h;
      }
      const ctx = rulerCanvas.getContext("2d");
      if (ctx) {
        drawRuler(ctx, project, session.timeline, ruler.loopRange(), ruler.loopPreview !== null, width, dpr);
      }
    });
  }

  $effect(() => {
    void project;
    void session.timeline;
    void rulerWidth;
    redrawRuler();
  });

  function rulerPoint(event: MouseEvent): { x: number; y: number } {
    const box = rulerCanvas?.getBoundingClientRect();
    return { x: event.clientX - (box?.left ?? 0), y: event.clientY - (box?.top ?? 0) };
  }

  function rulerDown(event: PointerEvent) {
    if (event.button !== 0 || event.detail >= 2) return;
    capture(event);
    const { x, y } = rulerPoint(event);
    ruler.press(x, y, mods(event));
    redrawRuler();
  }

  function rulerMove(event: PointerEvent) {
    const { x, y } = rulerPoint(event);
    cursor = ruler.move(x, y, mods(event));
    if (ruler.mode !== null) redrawRuler();
  }

  async function rulerUp() {
    const done = ruler.release();
    redrawRuler();
    await done;
  }

  function rulerDoubleClick(event: MouseEvent) {
    void ruler.doubleClick(rulerPoint(event).y);
  }

  // -- the toolbar and the "+ Track" menu ---------------------------------------------------------

  const info = $derived.by(() => {
    if (!project) return "";
    const bpm = project.tempo[0]?.bpm ?? 120;
    const changes = project.tempo.length > 1;
    const sig = project.time_signatures[0] ? signatureText(project.time_signatures[0]) : "4/4";
    return `${pyG(bpm)} BPM${changes ? "*" : ""}  ${sig}  ·  ${project.tracks.length} tracks`;
  });

  /** `add_track`: "Kind n", an instrument with the poly synth; the new track is selected. */
  async function addTrack(kind: "instrument" | "audio" | "bus" | "folder", base: string) {
    if (!project) return;
    const n = project.tracks.filter((track) => track.kind === kind).length + 1;
    const outcome = await run({
      op: "create_track",
      name: `${base} ${n}`,
      kind,
      instrument: kind === "instrument" ? "sonara.synth.poly" : null,
    });
    const trackId = outcome?.result.track_id;
    if (typeof trackId === "string") session.selectTrack(trackId);
  }

  function trackMenu(event: MouseEvent) {
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    menu = {
      x: box.left,
      y: box.bottom,
      label: "Add a track",
      items: [
        { label: "Instrument Track", run: () => void addTrack("instrument", "Instrument") },
        { label: "Audio Track", run: () => void addTrack("audio", "Audio") },
        { label: "Bus", run: () => void addTrack("bus", "Bus") },
        { label: "Folder", run: () => void addTrack("folder", "Folder") },
      ],
    };
  }

  // -- file drops ---------------------------------------------------------------

  const isAudio = (path: string) => {
    const dot = path.lastIndexOf(".");
    const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    return dot > slash + 1 && audioSuffixes.includes(path.slice(dot).toLowerCase());
  };
  const isProject = (path: string) => path.replace(/\/+$/, "").endsWith(".sonara");

  let dropPaths: string[] = [];

  /** The drop point in the arrangement's coordinates, or null outside it. */
  function dropPoint(position: { x: number; y: number }): { x: number; y: number } | null {
    const box = arrangementBox?.getBoundingClientRect();
    if (!box) return null;
    const dpr = window.devicePixelRatio || 1;
    const x = position.x / dpr - box.left;
    const y = position.y / dpr - box.top;
    return x >= 0 && y >= 0 && x <= box.width && y <= box.height ? { x, y } : null;
  }

  function dragOver(position: { x: number; y: number }) {
    const point = dropPoint(position);
    const audio = dropPaths.some(isAudio) && !dropPaths.some(isProject);
    const target = audio && point ? gestures.dropTarget(point.x, point.y, { shift: false, command: false, alt: false }) : null;
    arrangement?.setDrop(target);
  }

  function dropped(paths: string[], position: { x: number; y: number }) {
    arrangement?.setDrop(null);
    const projectPath = paths.find(isProject);
    if (projectPath) {
      backend.openProjectFile(projectPath).catch((error: unknown) => session.fail(error));
      return;
    }
    const audio = paths.filter(isAudio);
    if (audio.length === 0) return;
    const point = dropPoint(position);
    const target = point ? gestures.dropTarget(point.x, point.y, { shift: false, command: false, alt: false }) : null;
    // Over the canvas: the audio track under the pointer, at the snapped beat; elsewhere in the
    // window: new tracks at the playhead.
    const trackId = target?.row?.trackId ?? null;
    const beat = target ? target.beat : session.playhead.beat;
    backend.importAudioFiles(audio, trackId, beat).catch((error: unknown) => session.fail(error));
  }

  onMount(() => {
    if (!glCanvas || !overlayCanvas || !arrangementBox) return;
    arrangement = new Arrangement(glCanvas, overlayCanvas, session.peaks, () => gestures.drag);
    benchHooks.arrangement = arrangement;
    renderError = arrangement.error;
    if (renderError) session.fail(`the timeline cannot draw: ${renderError}`);
    const observer = new ResizeObserver(() => {
      if (arrangementBox) size = { width: arrangementBox.clientWidth, height: arrangementBox.clientHeight };
      if (rulerCanvas) rulerWidth = rulerCanvas.clientWidth;
      arrangement?.requestDraw();
    });
    observer.observe(arrangementBox);
    if (rulerCanvas) observer.observe(rulerCanvas);
    // Step 11: a move to a display with another scale redraws the ruler at that scale (the
    // arrangement's canvases follow it themselves, HiDpiCanvas).
    const stopScale = watchDevicePixelRatio(() => redrawRuler());
    arrangement.update({
      model,
      layout,
      view: session.timeline,
      selection: session.selection,
      peaksVersion: session.peaksVersion,
    });
    const stopDrops = backend.onDragDrop({
      enter: (paths, position) => {
        dropPaths = paths;
        dragOver(position);
      },
      over: (position) => dragOver(position),
      drop: (paths, position) => dropped(paths, position),
      leave: () => {
        dropPaths = [];
        arrangement?.setDrop(null);
      },
    });
    // The Edit menu's Split, Duplicate and Delete are the canvas's own ⌘E, ⌘D and
    // Delete: one "Split Clips" or "Duplicate Clips" step, the copies selected.
    const stopEditing = session.editing.register("timeline", {
      split: () => gestures.splitSelected(),
      duplicate: () => gestures.duplicateSelected(),
      remove: () => gestures.deleteSelected(),
    });
    // The Browser's Files tab drags inside the page, in CSS pixels.
    const physical = (x: number, y: number) => ({ x: x * (window.devicePixelRatio || 1), y: y * (window.devicePixelRatio || 1) });
    const stopFileDrags = registerFileDropZone({
      over: (paths, x, y) => {
        dropPaths = [...paths];
        dragOver(physical(x, y));
      },
      drop: (paths, x, y) => {
        dropPaths = [];
        dropped([...paths], physical(x, y));
      },
      leave: () => {
        dropPaths = [];
        arrangement?.setDrop(null);
      },
    });
    return () => {
      observer.disconnect();
      stopScale();
      arrangement?.destroy();
      arrangement = null;
      benchHooks.arrangement = null;
      void stopDrops.then((stop) => stop());
      stopFileDrags();
      stopEditing();
    };
  });

  const playheadX = $derived(beatToX(session.timeline, session.playhead.beat));
  const hbar = $derived({
    thumb: size.width > 0 ? Math.max(24, (size.width * size.width) / (range.maxX + size.width)) : 0,
    at: range.maxX > 0 ? session.timeline.scrollX / range.maxX : 0,
  });
  const vbar = $derived({
    thumb: size.height > 0 ? Math.max(24, (size.height * size.height) / (range.maxY + size.height)) : 0,
    at: range.maxY > 0 ? session.timeline.scrollY / range.maxY : 0,
  });

  function scrollbar(axis: "x" | "y", event: PointerEvent) {
    const track = event.currentTarget as HTMLElement;
    capture(event);
    const box = track.getBoundingClientRect();
    const length = axis === "x" ? box.width : box.height;
    const thumb = axis === "x" ? hbar.thumb : vbar.thumb;
    const max = axis === "x" ? range.maxX : range.maxY;
    const scrollTo = (pointer: number) => {
      const fraction = Math.min(1, Math.max(0, (pointer - thumb / 2) / Math.max(1, length - thumb)));
      const view = session.timeline;
      setView(axis === "x" ? setScroll(view, fraction * max) : setScrollY(view, fraction * max));
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
</script>

<section class="timeline" aria-label="Timeline" data-editor="timeline">
  <div class="toolbar" role="toolbar" aria-label="Timeline tools">
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
    <span class="gap"></span>
    <button type="button" title="Zoom out (−, Cmd/Ctrl+wheel)" aria-label="Zoom out" onclick={() => zoom(0.8)}>−</button>
    <button type="button" title="Zoom in (+, Cmd/Ctrl+wheel)" aria-label="Zoom in" onclick={() => zoom(1.25)}>+</button>
    <button type="button" title="Fit the whole song in the view" onclick={fit}>Fit</button>
    <span class="gap"></span>
    <label>
      Height
      <select
        title="Track height (Cmd/Ctrl+Alt+wheel)"
        value={session.timeline.trackHeight}
        onchange={(event) => setView({ ...session.timeline, trackHeight: event.currentTarget.value as TrackHeight })}
      >
        {#each HEIGHT_ORDER as name (name)}<option value={name}>{name}</option>{/each}
      </select>
    </label>
    <span class="gap"></span>
    <button
      type="button"
      title="Scroll with the playhead during playback"
      aria-pressed={session.timeline.follow}
      onclick={() => session.setTimeline({ ...session.timeline, follow: !session.timeline.follow })}>Follow</button
    >
    <span class="spacer"></span>
    <span class="info" data-testid="timeline-info">{info}</span>
  </div>
  <div class="grid">
    <div class="corner">
      <button type="button" title="Add a track" disabled={!project} onclick={trackMenu}>+ Track</button>
    </div>
    <div class="ruler-box" style:cursor={cursor}>
      <canvas
        bind:this={rulerCanvas}
        class="ruler"
        aria-label="Ruler"
        onpointerdown={rulerDown}
        onpointermove={rulerMove}
        onpointerup={rulerUp}
        ondblclick={rulerDoubleClick}
      ></canvas>
      {#if playheadX >= 0 && playheadX <= rulerWidth}
        <div class="ruler-playhead" style:transform="translateX({playheadX}px)"></div>
      {/if}
    </div>
    {#if layout && project}
      <Headers
        {session}
        {project}
        {layout}
        scrollY={session.timeline.scrollY}
        onscroll={(dy) => setView(setScrollY(session.timeline, session.timeline.scrollY + dy))}
      />
    {:else}
      <div class="no-headers"></div>
    {/if}
    <!-- The canvas is one focusable widget with its own keys; its clips are a list box of
         options over it for assistive technology, whose keys reach the canvas's. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      bind:this={arrangementBox}
      class="arrangement"
      role="application"
      aria-label="Arrangement"
      tabindex="0"
      style:cursor={cursor}
      onpointerdown={pointerDown}
      onpointermove={pointerMove}
      onpointerup={pointerUp}
      ondblclick={doubleClick}
      oncontextmenu={contextMenu}
      onwheel={wheel}
      onkeydown={keyDown}
      onscroll={scrolled}
    >
      <canvas bind:this={glCanvas} class="layer" data-testid="arrangement-canvas" aria-hidden="true"></canvas>
      <canvas bind:this={overlayCanvas} class="layer" aria-hidden="true"></canvas>
      <!-- svelte-ignore a11y_interactive_supports_focus -->
      <div class="clip-list" role="listbox" aria-label="Clips" aria-multiselectable="true">
        {#each clipList as group (group.trackId)}
          <div role="group" aria-label={group.name}>
            {#each group.options as option (option.id)}
              <!-- The canvas takes the option's keys; the click is VoiceOver's press. -->
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <div
                id={clipOptionId(option.id)}
                class="clip-option"
                role="option"
                tabindex="-1"
                data-clip={option.id}
                aria-selected={option.selected}
                aria-label={option.label}
                aria-posinset={option.position}
                aria-setsize={option.count}
                style:transform="translate({option.rect.x}px, {option.rect.y}px)"
                style:width="{option.rect.width}px"
                style:height="{option.rect.height}px"
                onfocus={() => clipFocused(option.id)}
                onclick={(event) => optionClick(event, option.id)}
              ></div>
            {/each}
          </div>
        {/each}
      </div>
      {#if playheadX >= 0 && playheadX <= size.width}
        <div class="playhead" style:transform="translateX({Math.round(playheadX)}px)" data-testid="playhead"></div>
      {/if}
    </div>
    <div class="vbar" onpointerdown={(event) => scrollbar("y", event)} role="presentation">
      <div class="thumb" style:height="{vbar.thumb}px" style:top="{vbar.at * (size.height - vbar.thumb)}px"></div>
    </div>
    <div class="hbar" onpointerdown={(event) => scrollbar("x", event)} role="presentation">
      <div class="thumb" style:width="{hbar.thumb}px" style:left="{hbar.at * (size.width - hbar.thumb)}px"></div>
    </div>
  </div>
</section>
{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} label={menu.label} onclose={() => (menu = null)} />
{/if}

<style>
  .timeline {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    min-height: 0;
    background: var(--window);
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 32px;
    padding: 3px 6px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }
  .toolbar button,
  .corner button {
    height: 24px;
    padding: 0 8px;
    border: 1px solid transparent;
    border-radius: 4px;
    background: transparent;
    color: var(--text);
    font: inherit;
  }
  .toolbar button:hover,
  .corner button:hover:not(:disabled) {
    background: var(--panel-alt);
  }
  .toolbar button[aria-pressed="true"] {
    background: var(--accent);
    color: #fff;
  }
  select {
    height: 24px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
  label {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .gap {
    width: 12px;
  }
  .spacer {
    flex: 1;
  }
  .info {
    color: var(--text-dim);
    white-space: pre;
  }
  .grid {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 236px minmax(0, 1fr) 12px;
    grid-template-rows: 46px minmax(0, 1fr) 12px;
  }
  .corner {
    display: flex;
    align-items: center;
    padding: 0 6px;
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }
  .ruler-box {
    position: relative;
    overflow: hidden;
    grid-column: 2 / 4;
  }
  .ruler {
    display: block;
    width: 100%;
    height: 46px;
  }
  .ruler-playhead {
    position: absolute;
    top: 16px;
    left: -5px;
    width: 0;
    height: 0;
    border-left: 5px solid transparent;
    border-right: 5px solid transparent;
    border-top: 7px solid var(--playhead);
    pointer-events: none;
  }
  .ruler-playhead::after {
    content: "";
    position: absolute;
    top: -1px;
    left: -0.5px;
    width: 1px;
    height: 24px;
    background: var(--playhead);
  }
  .no-headers {
    border-right: 1px solid var(--border);
    background: var(--panel);
  }
  .arrangement {
    position: relative;
    overflow: hidden;
    outline: none;
  }
  .arrangement:focus-visible {
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .layer {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  /* The clips' options are transparent: the canvas draws the clips, and the pointer goes to it. */
  .clip-list {
    position: absolute;
    inset: 0;
    pointer-events: none;
    contain: strict;
  }
  .clip-option {
    position: absolute;
    top: 0;
    left: 0;
    border-radius: 3px;
    outline: none;
  }
  .clip-option:focus-visible {
    box-shadow: 0 0 0 1px var(--text);
  }
  .playhead {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 1px;
    background: var(--playhead);
    pointer-events: none;
  }
  .vbar,
  .hbar {
    position: relative;
    background: var(--panel);
  }
  .hbar {
    grid-column: 2;
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
