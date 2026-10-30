<script lang="ts">
  import { onMount } from "svelte";
  import * as backend from "../backend";
  import type { Session } from "../session.svelte";
  import About from "./About.svelte";
  import AgentConnection from "./AgentConnection.svelte";
  import { onDialogCommand, type DialogName } from "./api";
  import Export from "./Export.svelte";
  import NewProject from "./NewProject.svelte";
  import Settings from "./Settings.svelte";

  // The dialogs the menu opens (`dialog-command`): one at a time. Import
  // Audio… opens the native panel at once, for the selected audio track (else new tracks)
  // at the playhead, and imports on a job.
  let { session }: { session: Session } = $props();

  let open: DialogName | null = $state(null);

  /** MW-07: the open panel for the selected audio track, else new tracks, at the playhead. */
  function importAudio() {
    const project = session.view?.project ?? null;
    const current = session.selection.currentTrack;
    const track = project?.tracks.find((t) => t.id === current && t.kind === "audio") ?? null;
    backend.importAudioDialog(track?.id ?? null, session.playhead.beat).catch((error: unknown) => session.fail(error));
  }

  export function show(dialog: DialogName) {
    if (dialog === "import-audio") importAudio();
    else open = dialog;
  }

  onMount(() => {
    const stop = onDialogCommand((dialog) => show(dialog));
    return () => void stop.then((unlisten) => unlisten());
  });

  const close = () => (open = null);
</script>

{#if open === "new-project"}
  <NewProject onclose={close} />
{:else if open === "settings"}
  <Settings onclose={close} />
{:else if open === "export-audio"}
  <Export onclose={close} />
{:else if open === "agent-connection"}
  <AgentConnection onclose={close} />
{:else if open === "about"}
  <About onclose={close} />
{/if}
