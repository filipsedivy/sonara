<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { aboutInfo, errorMessage, type About } from "./api";
  import logo from "./brand/logo-dark.svg";
  import Dialog from "./Dialog.svelte";

  // About Sonara (the prototype's `AboutDialog`): the logo, the version and stage, the
  // tagline, the author with a mail link, the copyright; the System tab with the versions (Copy
  // System Info copies them) and the Credits tab with the main components, the LGPL note and,
  // once the bundle carries them, the generated licence texts. Links open
  // in the system's applications (`open_link`, only the author's address and the credits' pages).
  let { onclose }: { onclose: () => void } = $props();

  let shown = $state<About | null>(null);
  let error = $state<string | null>(null);
  let tab: "system" | "credits" = $state("system");
  let copied = $state(false);

  const systemText = $derived(shown ? shown.system.map(([key, value]) => `${key}: ${value}`).join("\n") : "");

  onMount(() => {
    aboutInfo()
      .then((loaded) => (shown = loaded))
      .catch((failure: unknown) => (error = errorMessage(failure)));
  });

  function open(event: MouseEvent, url: string) {
    event.preventDefault();
    invoke("open_link", { url }).catch((failure: unknown) => (error = errorMessage(failure)));
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText(systemText);
    } catch {
      // Without the clipboard API: a selected text area and the copy command.
      const area = document.createElement("textarea");
      area.value = systemText;
      document.body.append(area);
      area.select();
      document.execCommand("copy");
      area.remove();
    }
    copied = true;
  }
</script>

<Dialog id="about" title="About Sonara" width={560} onclose={onclose} onsubmit={onclose}>
  {#if shown}
    <div class="identity">
      <img src={logo} alt="{shown.product} logo" width="320" />
      <p class="dim">{shown.version}</p>
      <p>{shown.tagline}</p>
      <hr />
      <p>Created by <b>{shown.author}</b></p>
      <p>
        <a href="mailto:{shown.email}" title="Write to {shown.author}" onclick={(e) => open(e, `mailto:${shown?.email}`)}
          >{shown.email}</a
        >
      </p>
      <p class="dim">{shown.copyright}</p>
    </div>
    <div class="tabs" role="tablist" aria-label="About">
      <button type="button" role="tab" aria-selected={tab === "system"} onclick={() => (tab = "system")}>System</button>
      <button type="button" role="tab" aria-selected={tab === "credits"} onclick={() => (tab = "credits")}>Credits</button>
    </div>
    {#if tab === "system"}
      <textarea class="system" readonly aria-label="System" rows={shown.system.length + 1} value={systemText}></textarea>
    {:else}
      <div class="credits" role="tabpanel" aria-label="Credits">
        <p>{shown.product} is built on these open-source components:</p>
        <table>
          <tbody>
            {#each shown.credits as credit (credit.name)}
              <tr>
                <td><a href={credit.url} onclick={(e) => open(e, credit.url)}>{credit.name}</a></td>
                <td class="dim">{credit.license}</td>
              </tr>
            {/each}
          </tbody>
        </table>
        <p class="dim">{shown.lgpl_note}</p>
        {#if shown.licenses}
          <pre class="licenses">{shown.licenses}</pre>
        {:else}
          <p class="dim">The full licence texts of every component are generated into the application bundle.</p>
        {/if}
      </div>
    {/if}
  {/if}
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
  {#snippet buttons()}
    <button type="button" onclick={copy} disabled={!shown}>{copied ? "Copied" : "Copy System Info"}</button>
    <button type="submit">Close</button>
  {/snippet}
</Dialog>

<style>
  .identity {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
  }
  .identity img {
    max-width: 100%;
    height: auto;
  }
  .identity p {
    margin: 2px 0;
  }
  hr {
    width: 100%;
    border: 0;
    border-top: 1px solid var(--border);
  }
  .dim {
    color: var(--text-dim);
  }
  a {
    color: var(--accent);
  }
  .tabs {
    display: flex;
    gap: 2px;
  }
  .tabs button {
    padding: 4px 10px;
    border: 0;
    border-radius: 4px 4px 0 0;
    background: transparent;
    color: var(--text-dim);
    font: inherit;
  }
  .tabs button[aria-selected="true"] {
    background: var(--bar);
    color: var(--text);
  }
  .system {
    font-family: Menlo, monospace;
    font-size: 11px;
  }
  .credits {
    max-height: 220px;
    overflow: auto;
  }
  .credits p {
    margin: 4px 0;
  }
  td {
    padding: 1px 12px 1px 0;
  }
  .licenses {
    white-space: pre-wrap;
    font-size: 11px;
  }
</style>
