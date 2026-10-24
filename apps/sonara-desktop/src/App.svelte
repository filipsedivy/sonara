<script lang="ts">
  import { onMount } from "svelte";
  import * as backend from "./lib/backend";
  import Browser from "./lib/browser/Browser.svelte";
  import DialogHost from "./lib/dialogs/DialogHost.svelte";
  import { deleteTrack } from "./lib/dialogs/api";
  import { dialogIsOpen } from "./lib/dialogs/fields";
  import Inspector from "./lib/inspector/Inspector.svelte";
  import DevicePanel from "./lib/devices/DevicePanel.svelte";
  import Mixer from "./lib/mixer/Mixer.svelte";
  import PianoRoll from "./lib/pianoroll/PianoRoll.svelte";
  import Recovery from "./lib/Recovery.svelte";
  import { Session, type EditorTab } from "./lib/session.svelte";
  import { editorOf, isModal, MenuContextReporter, onModalChange } from "./lib/shell/editing";
  import { runMenuCommand, type MenuCommandHandlers } from "./lib/shell/menuCommands";
  import { clampSize, restore, toggle, type PaneName } from "./lib/shell/panes";
  import { installFieldUndo, transportShortcut, type TransportShortcut } from "./lib/shell/shortcuts";
  import Splitter from "./lib/shell/Splitter.svelte";
  import StatusBar from "./lib/StatusBar.svelte";
  import { errorText } from "./lib/status";
  import Timeline from "./lib/timeline/Timeline.svelte";
  import TransportBar from "./lib/transport/TransportBar.svelte";
  import { TransportControl } from "./lib/transport/transport";
  import type { AgentStatus, Edit, RecoverableSession, StatusMessage } from "./lib/types";

  // The main window: the transport bar on top; the Browser, the
  // timeline and the Inspector side by side, with the Editors (Mixer, Piano Roll, Devices) below
  // the timeline; the status bar. The panes resize with their splitters and show or hide from the
  // View menu; their sizes are saved with the window. The Mixer tab holds the mixer, which
  // can move to a window of its own, the Piano Roll tab the piano roll; the other
  // panes' contents come in steps 8 and 9: the Browser and the Inspector, and the dialogs the menu
  // opens (DialogHost; their single-key shortcuts wait while a dialog is open).
  let agent: AgentStatus | null = $state(null);
  let message: StatusMessage | null = $state(null);
  let recoverable: RecoverableSession[] = $state([]);
  let audioSuffixes: string[] = $state([]);
  let clearTimer: ReturnType<typeof setTimeout> | undefined;
  let body: HTMLDivElement | undefined = $state();

  function show(next: StatusMessage) {
    message = next;
    clearTimeout(clearTimer);
    clearTimer = setTimeout(() => (message = null), next.duration_ms);
  }

  function fail(error: unknown) {
    show({ text: errorText(error), duration_ms: 8000, error: true });
  }

  const session = new Session(show);

  const control = new TransportControl({
    play: backend.transportPlay,
    stop: backend.transportStop,
    record: backend.transportRecord,
    seek: backend.transportSeek,
    edit: backend.edit,
    transport: () => session.transport(),
    project: () => session.view?.project ?? null,
    playhead: () => session.playhead.beat,
    setPlayhead: (beat, playing) => session.setPlayhead(beat, playing),
    fail: (error) => session.fail(error),
    answered: (state) => session.answered(state),
    setRecording: (checked) => (session.recording = checked),
  });

  const panes = $derived(session.panes);
  const TABS: [EditorTab, string][] = [
    ["mixer", "Mixer"],
    ["piano-roll", "Piano Roll"],
    ["devices", "Devices"],
  ];

  function resizePane(name: PaneName, size: number) {
    const available = name === "editors" ? (body?.clientHeight ?? Infinity) : (body?.clientWidth ?? Infinity);
    session.setPanes({ ...session.panes, [name]: { ...session.panes[name], size: clampSize(name, size, available) } });
  }

  async function recover(workspace: string) {
    recoverable = [];
    try {
      await backend.recoverSession(workspace);
    } catch (error) {
      fail(error);
    }
  }

  function shortcut(event: KeyboardEvent) {
    // A modal dialog takes the keys: Space on its button must not also play.
    if (dialogIsOpen()) return;
    const action = transportShortcut(event);
    if (!action) return;
    event.preventDefault();
    runShortcut(action);
  }

  /** A transport key, from this window or from the mixer's. */
  function runShortcut(action: TransportShortcut) {
    if (action === "play") void control.togglePlay();
    else if (action === "home") void control.returnToStart();
    else if (action === "record") void control.toggleRecord(!session.recording);
    else if (action === "loop") {
      void control.setTransport("loop_enabled", !(session.view?.project?.transport.loop_enabled ?? false));
    } else {
      void control.setTransport("metronome", !(session.view?.project?.transport.metronome ?? false));
    }
  }

  // The Edit, Track and Transport menus: run as the page's own keys and gestures, and
  // never while a dialog is open.
  const menuHandlers: MenuCommandHandlers = {
    modal: () => dialogIsOpen() || isModal(),
    edit: (command) => void session.editing.run(command, editorOf(document.activeElement)),
    newTrack: (edit) => void newTrack(edit),
    deleteTrack: () => {
      const trackId = session.selection.currentTrack;
      if (trackId) deleteTrack(trackId).catch(fail);
    },
    transport: runShortcut,
  };

  /** MW-20: the Track menu's new track, selected. */
  async function newTrack(edit: Edit) {
    try {
      const outcome = await backend.edit(edit);
      const trackId = outcome.result.track_id;
      if (typeof trackId === "string") session.selectTrack(trackId);
    } catch (error) {
      session.fail(error);
    }
  }

  /** The benchmarks, in a build with the `bench` feature started for them. */
  function startBenchmark() {
    backend
      .benchConfig()
      .then(async (config) => {
        if (!config) return;
        const status = (text: string) => show({ text, duration_ms: 600_000, error: false });
        if (config.suite === "e2e") {
          const { runEndToEnd } = await import("./lib/bench/e2e");
          await runEndToEnd(session, config, status);
        } else if (config.suite === "meters") {
          const { runMeterBench } = await import("./lib/bench/meters");
          await runMeterBench(session, config, status);
        } else if (config.suite === "playback") {
          const { runPlaybackBench } = await import("./lib/bench/playback");
          await runPlaybackBench(session, config, status);
        } else if (config.suite === "pianoroll") {
          const { runPianoRollBench } = await import("./lib/bench/pianoroll");
          await runPianoRollBench(session, config, status);
        } else if (config.suite === "e2e-dialogs") {
          const { runDialogsEndToEnd } = await import("./lib/bench/e2e-dialogs");
          await runDialogsEndToEnd(session, config, status);
        } else if (config.suite === "e2e-a11y") {
          const { runA11yEndToEnd } = await import("./lib/bench/e2e-a11y");
          await runA11yEndToEnd(session, config, status);
        } else {
          const { runTimelineBench } = await import("./lib/bench/timeline");
          await runTimelineBench(session, config, status);
        }
      })
      .catch(() => {
        // Not a benchmark build: the command does not exist.
      });
  }

  session.onShortcut = runShortcut;

  onMount(() => {
    const unlisten = [
      backend.onMessage(show),
      backend.onMenuCommand((command) => void runMenuCommand(command, menuHandlers)),
      backend.onViewCommand(({ command }) => {
        if (command === "toggle-browser") session.setPanes(toggle(session.panes, "browser"));
        else if (command === "toggle-inspector") session.setPanes(toggle(session.panes, "inspector"));
        else if (command === "toggle-editors") session.setPanes(toggle(session.panes, "editors"));
        else session.raiseTab(command);
      }),
    ];
    const stop = session.start();
    backend
      .shellStatus()
      .then((status) => {
        agent = status.agent;
        audioSuffixes = status.audio_suffixes ?? [];
        session.panes = restore(status.panes);
        for (const startup of status.messages) show(startup);
        // A10: the mixer's window opens again where it was, as it was open when Sonara quit.
        if (status.reopen_mixer) session.detachMixer();
      })
      .catch(fail);
    backend
      .recoverableSessions()
      .then((sessions) => (recoverable = sessions))
      .catch(fail);
    window.addEventListener("keydown", shortcut);
    // Step 11: a text field's ⌘Z and ⇧⌘Z undo its typing, not the project (the prototype's).
    const stopFieldUndo = installFieldUndo(window, (command) => document.execCommand(command));
    // The desktop words and enables the Edit menu for the focused editor, and declines the
    // menu while a dialog is open.
    const reporter = new MenuContextReporter(backend.setMenuContext, fail);
    const report = () => reporter.update({ focus: editorOf(document.activeElement), modal: isModal() });
    const reportLater = () => setTimeout(report, 0);
    document.addEventListener("focusin", report);
    document.addEventListener("focusout", reportLater);
    const stopModal = onModalChange(report);
    report();
    startBenchmark();
    return () => {
      clearTimeout(clearTimer);
      stop();
      window.removeEventListener("keydown", shortcut);
      stopFieldUndo();
      document.removeEventListener("focusin", report);
      document.removeEventListener("focusout", reportLater);
      stopModal();
      for (const listener of unlisten) void listener.then((stopListening) => stopListening());
    };
  });
</script>

<TransportBar {session} {control} />
<div class="body" bind:this={body}>
  {#if panes.browser.open}
    <aside class="pane side" aria-label="Browser" style:width="{panes.browser.size}px">
      <header>Browser</header>
      <Browser {session} />
    </aside>
    <Splitter
      orientation="vertical"
      label="Browser width"
      size={panes.browser.size}
      direction={1}
      onresize={(size) => resizePane("browser", size)}
    />
  {/if}
  <div class="center">
    <Timeline {session} {audioSuffixes} />
    {#if panes.editors.open}
      <Splitter
        orientation="horizontal"
        label="Editors height"
        size={panes.editors.size}
        direction={-1}
        onresize={(size) => resizePane("editors", size)}
      />
      <section class="pane editors" aria-label="Editors" style:height="{panes.editors.size}px">
        <div class="tabs" role="tablist" aria-label="Editors">
          {#each TABS as [tab, title] (tab)}
            <button
              type="button"
              role="tab"
              aria-selected={session.editorTab === tab}
              onclick={() => (session.editorTab = tab)}>{title}</button
            >
          {/each}
          <span class="spacer"></span>
          {#if session.editorTab === "mixer" && !session.mixerDetached}
            <button
              type="button"
              class="detach"
              title="Open the mixer in a window of its own"
              onclick={() => session.detachMixer()}>Open in Window</button
            >
          {/if}
        </div>
        <div class="tab-body" role="tabpanel" aria-label={TABS.find(([tab]) => tab === session.editorTab)?.[1]}>
          {#if session.editorTab === "mixer"}
            {#if session.mixerDetached}
              <div class="detached">
                <p>The mixer is in its own window.</p>
                <button type="button" onclick={() => session.detachMixer()}>Show the Mixer Window</button>
              </div>
            {:else}
              <Mixer {session} />
            {/if}
          {:else if session.editorTab === "piano-roll"}
            <PianoRoll {session} />
          {:else if session.editorTab === "devices"}
            <DevicePanel {session} />
          {/if}
        </div>
      </section>
    {/if}
  </div>
  {#if panes.inspector.open}
    <Splitter
      orientation="vertical"
      label="Inspector width"
      size={panes.inspector.size}
      direction={-1}
      onresize={(size) => resizePane("inspector", size)}
    />
    <aside class="pane side" aria-label="Inspector" style:width="{panes.inspector.size}px">
      <header>Inspector</header>
      <Inspector {session} />
    </aside>
  {/if}
</div>
<StatusBar {agent} {message} jobs={session.jobs} />
{#if recoverable.length > 0}
  <Recovery sessions={recoverable} onrecover={recover} ondismiss={() => (recoverable = [])} />
{/if}
<DialogHost {session} />

<style>
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .center {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .pane {
    flex: none;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    min-width: 0;
    min-height: 0;
  }
  .pane header {
    padding: 4px 8px;
    background: var(--panel-alt);
    color: var(--text-dim);
    font-size: 12px;
  }
  .tabs {
    display: flex;
    gap: 2px;
    padding: 2px 4px 0;
    background: var(--panel-alt);
  }
  .tabs button {
    padding: 5px 12px;
    border: 0;
    border-radius: 4px 4px 0 0;
    background: transparent;
    color: var(--text-dim);
    font: inherit;
  }
  .tabs button[aria-selected="true"] {
    background: var(--panel);
    color: var(--text);
  }
  .tabs .spacer {
    flex: 1;
  }
  .tabs .detach {
    margin-bottom: 2px;
    border-radius: 4px;
  }
  .tabs .detach:hover {
    background: var(--panel);
    color: var(--text);
  }
  .tab-body {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .detached {
    margin: auto;
    color: var(--text-dim);
    text-align: center;
  }
  .detached button {
    padding: 4px 12px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--panel-alt);
    color: var(--text);
    font: inherit;
  }
</style>
