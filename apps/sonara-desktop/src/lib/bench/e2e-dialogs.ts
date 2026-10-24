// The end-to-end check of the browser, the inspector and the dialogs, run in
// the real application on a real session by scripts/e2e-dialogs.mjs (the `bench` build's hook,
// Sonara's own harness): the page drives its own panes and dialogs and checks what the backend did
// through the project view; the harness checks the files on disk after the run.
//
// Checks: the Devices tree comes from the real registry; the Files tab lists the sandbox's
// ~/Music and a double-click imports a file into a new audio track at beat 0 on an import job;
// the inspector's volume field is one "Set Track Mixer"-like step and a fade edit
// keeps the other fade's curve; typed text survives an edit from elsewhere; the New
// Project dialog names a wrong time signature and stays open; the Settings dialog saves the
// choices (the harness reads settings.json); the Export dialog exports a file (the harness checks
// it); About and Agent Connection show the version and the bundled CLI. Then the menus' checks of
// step 10 run on the same session (e2e-menus.ts).

import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import * as backend from "../backend";
import type { Session } from "../session.svelte";
import { runMenusEndToEnd } from "./e2e-menus";
import { sleep } from "./stats";
import type { AudioClipView, JobNotice, TrackView } from "../types";

interface Check {
  name: string;
  ok: boolean;
  detail: string;
}

async function until<T>(what: string, probe: () => T | null | undefined | false, timeoutMs = 15_000): Promise<T> {
  const deadline = performance.now() + timeoutMs;
  for (;;) {
    const value = probe();
    if (value) return value;
    if (performance.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await sleep(50);
  }
}

function trackNamed(session: Session, name: string): TrackView | undefined {
  return session.view?.project?.tracks.find((track) => track.name === name);
}

const query = <T extends Element>(selector: string, root: ParentNode = document) => root.querySelector<T>(selector);

/** Types `text` into `input` as a user would, then presses `key` (Enter, Escape) or blurs. */
function type(input: HTMLInputElement | HTMLTextAreaElement, text: string, key: "Enter" | "Escape" | "blur" = "Enter") {
  input.focus();
  input.value = text;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  if (key === "blur") input.blur();
  else input.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
}

function choose(select: HTMLSelectElement, value: string) {
  select.value = value;
  select.dispatchEvent(new Event("change", { bubbles: true }));
}

function rowNamed(tree: Element, text: string): HTMLElement | null {
  return (
    [...tree.querySelectorAll<HTMLElement>('[role="treeitem"]')].find((row) => row.querySelector(".text")?.textContent === text) ??
    null
  );
}

async function openDialog(name: string, title: string): Promise<HTMLDialogElement> {
  await emit("dialog-command", { dialog: name });
  return until(`the ${title} dialog`, () =>
    [...document.querySelectorAll<HTMLDialogElement>("dialog[open]")].find((d) => d.querySelector("h2")?.textContent === title),
  );
}

function button(root: ParentNode, text: string): HTMLButtonElement | null {
  return [...root.querySelectorAll<HTMLButtonElement>("button")].find((b) => b.textContent?.trim() === text) ?? null;
}

export async function runDialogsEndToEnd(
  session: Session,
  config: { media: string | null; out: string },
  status: (text: string) => void,
): Promise<void> {
  const checks: Check[] = [];
  const report: Record<string, unknown> = { started: new Date().toISOString(), checks };
  const check = (name: string, ok: boolean, detail = "") => checks.push({ name, ok, detail });
  // The report lies in the sandbox the harness made.
  const sandbox = config.out.replace(/\/[^/]*$/, "");
  try {
    status("End to end: waiting for the project");
    const keys = await until("the Keys track", () => trackNamed(session, "Keys"));

    // -- the browser --------------------------------------------------------------------------------
    status("End to end: the browser");
    const browser = await until("the browser", () => query('[aria-label="Browser"]'));
    const devices = await until("the devices tree", () => {
      const tree = query('[role="tree"]', browser);
      return tree && rowNamed(tree, "Sonara Poly Synth") ? tree : null;
    });
    check("the Devices tab lists the registry's instruments and effects", !!rowNamed(devices, "Effects"));
    button(browser, "Files")?.click();
    const files = await until("the media file", () => {
      const tree = query('[aria-label="Audio files"]', browser);
      return tree && rowNamed(tree, "e2e tone.wav") ? tree : null;
    });
    const tracksBefore = session.view?.project?.tracks.length ?? 0;
    rowNamed(files, "e2e tone.wav")?.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    const imported = await until(
      "the imported track",
      () => (session.view?.project?.tracks.length ?? 0) > tracksBefore && trackNamed(session, "e2e tone"),
      30_000,
    );
    const clip = imported.clips[0];
    check(
      "a double-click in the Files tab imports into a new audio track at beat 0",
      imported.kind === "audio" && clip?.start === 0,
      `${imported.kind}, clip at ${clip?.start}`,
    );

    // -- the inspector ----------------------------------------------------------------------------------
    status("End to end: the inspector");
    const inspector = await until("the inspector", () => query('[aria-label="Inspector"]'));
    session.selectTrack(keys.id);
    const volume = await until("the volume field", () => query<HTMLInputElement>("#track-volume", inspector));
    type(volume, "-6");
    const quieter = await until("the quieter Keys", () => (trackNamed(session, "Keys")?.mixer.volume_db === -6 ? true : null));
    check("the inspector's volume field edits the track", quieter, `undo ${session.view?.project?.history.undo}`);

    session.selectClips([clip.id]);
    const outCurve = await until("the fade-out curve", () => query<HTMLSelectElement>("#clip-fade-out-curve", inspector));
    choose(outCurve, "s_curve");
    await until("the fade-out curve in the view", () => {
      const c = trackNamed(session, "e2e tone")?.clips[0] as AudioClipView | undefined;
      return c?.fade_out.curve === "s_curve";
    });
    type(query<HTMLInputElement>("#clip-fade-in", inspector)!, "0.25");
    const faded = await until("the fade-in", () => {
      const c = trackNamed(session, "e2e tone")?.clips[0] as AudioClipView | undefined;
      return c && Math.abs(c.fade_in.seconds - 0.25) < 1e-9 ? c : null;
    });
    check("G4: a fade-in edit keeps the fade-out curve", faded.fade_out.curve === "s_curve", faded.fade_out.curve);

    const projectName = await until("the project name field", () => query<HTMLInputElement>("#project-name", inspector));
    projectName.focus();
    projectName.value = "typed, not sent";
    projectName.dispatchEvent(new Event("input", { bubbles: true }));
    const version = session.view?.version ?? 0;
    await backend.edit({ op: "rename_track", track_id: keys.id, name: "Keys 2" });
    await until("the rename's view", () => (session.view?.version ?? 0) > version && trackNamed(session, "Keys 2"));
    await sleep(100);
    check(
      "G6: typed text survives an edit from elsewhere",
      projectName.value === "typed, not sent" && document.activeElement === projectName,
      projectName.value,
    );
    projectName.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
    projectName.blur();

    // -- New Project: G3 -----------------------------------------------------------------------------------
    status("End to end: New Project");
    const newProject = await openDialog("new-project", "New Project");
    const signature = await until("the time signature field", () => query<HTMLInputElement>("#new-project-time-signature", newProject));
    signature.value = "x/4";
    signature.dispatchEvent(new Event("input", { bubbles: true }));
    button(newProject, "OK")?.click();
    const shown = await until("the field's error", () => query('[role="alert"]', newProject)?.textContent);
    check(
      "G3: New Project names a wrong time signature and stays open",
      shown === "invalid time signature 'x/4'" && newProject.open,
      shown,
    );
    button(newProject, "Cancel")?.click();
    await until("the closed dialog", () => !document.querySelector("dialog[open]"));

    // -- Settings -----------------------------------------------------------------------------------
    status("End to end: Settings");
    const settings = await openDialog("settings", "Settings");
    const theme = await until("the theme", () => query<HTMLSelectElement>("#settings-theme", settings));
    choose(theme, "light");
    type(query<HTMLInputElement>("#settings-undo", settings)!, "2000", "blur");
    type(query<HTMLTextAreaElement>("#settings-paths", settings)!, "/Volumes/Audio\n\n  ~/Samples  ", "blur");
    button(settings, "OK")?.click();
    await until("the saved settings", () => !document.querySelector("dialog[open]"));
    check("the Settings dialog saves and closes", true);

    // -- Export -----------------------------------------------------------------------------------
    status("End to end: Export Audio");
    const exporting = await openDialog("export-audio", "Export Audio");
    const path = await until("the export path", () => query<HTMLInputElement>("#export-path", exporting));
    let exported: JobNotice | null = null;
    const stopJobs = await backend.onJobChanged((job) => {
      if (job.kind === "export_audio" && job.finished) exported = job;
    });
    path.value = `${sandbox}/export/E2E Mix`;
    path.dispatchEvent(new Event("input", { bubbles: true }));
    button(exporting, "Export")?.click();
    await until("the closed export dialog", () => !document.querySelector("dialog[open]"));
    const job = await until("the export job", () => exported, 60_000);
    stopJobs();
    check("the Export dialog's export job succeeds", job.status === "succeeded", job.error?.message ?? job.status);

    // -- Help ---------------------------------------------------------------------------------------
    status("End to end: About and Agent Connection");
    const about = await openDialog("about", "About Sonara");
    const version2 = await until("the version", () =>
      [...about.querySelectorAll("p")].map((p) => p.textContent ?? "").find((t) => t.startsWith("Version ")),
    );
    check("About shows the version and stage", /^Version \d+\.\d+\.\d+ \(alpha\)$/.test(version2), version2);
    button(about, "Close")?.click();
    await until("About closed", () => !document.querySelector("dialog[open]"));
    const agent = await openDialog("agent-connection", "Agent Connection");
    const text = await until("the registration", () => query<HTMLTextAreaElement>("textarea", agent)?.value);
    check(
      "Agent Connection names the bundled sonara mcp",
      /claude mcp add sonara -- \S*\/sonara mcp/.test(text) && text.includes("Discovery file:"),
      text.split("\n")[2] ?? "",
    );
    button(agent, "Close")?.click();

    // -- the menus and shortcuts ----------------------------------------------------------
    await runMenusEndToEnd(session, sandbox, check, status);
  } catch (error) {
    report.error = error instanceof Error ? `${error.message}\n${error.stack}` : String(error);
  }
  report.valid = !report.error && checks.every((c) => c.ok);
  report.finished = new Date().toISOString();
  status("End to end: writing the report");
  await invoke("bench_report", { report });
}
