// The end-to-end accessibility check on macOS: the real application on a
// real project, its page driven by itself (src/lib/bench/e2e-a11y.ts) through the `bench` build's
// hook, and its AppKit accessibility tree read from outside through System Events, as VoiceOver
// reads it. At each of the page's checkpoints this harness reads the focused element (and, for the
// meters, the window's level indicators) with their role, name, value and state, checks ADR 0009's
// VoiceOver checks and the inventory's notes against them, and lets the page go on.
//
// 1. builds a debug desktop with the `bench` feature and its frontend embedded, and the debug
//    `sonara` CLI, unless --skip-build (scripts/e2e-dialogs.mjs builds the same);
// 2. makes a sandbox (--out): its own SONARA_HOME and HOME, and "A11y.sonara" built by
//    `sonara batch` ("Drums 1" and "Bass" with a clip of four notes each, "Vox");
// 3. runs the desktop on the simulated audio device with SONARA_BENCH_SUITE=e2e-a11y and answers
//    its checkpoints; prints every check and, with --dump, every element read. The exit code is 0
//    when all passed.
//
// --at <x>,<y> opens the window there (in the sandbox's saved layout, global points from the main
// display's top left), for example on another display; --shots <dir> saves a capture of the real
// window at the page's `shot-*` checkpoints (the main window, the piano roll, a device card), at
// the scale of the display it is on.
//
// It needs System Events' accessibility access (granted to the terminal that runs it), and the
// window must stay visible on the display's current Space: keep the screen unlocked, and no
// full-screen app in front of it. A run has a hard timeout.
//
// Usage (from apps/sonara-desktop):
//   node tests/e2e/a11y-tree.mjs [--out <dir>] [--skip-build] [--dump] [--at <x>,<y>] [--shots <dir>]

import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const app = path.resolve(here, "../..");
const root = path.resolve(app, "../..");
const args = process.argv.slice(2);
const option = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : fallback;
};
const dump = args.includes("--dump");
const out = path.resolve(option("out", path.join(os.tmpdir(), `sonara-e2e-a11y-${Date.now()}`)));
const desktop = path.join(root, "target/debug/sonara-desktop");
const cli = path.join(root, "target/debug/sonara");
const timeoutMs = 240_000;

const sonaraHome = path.join(out, "home");
const userHome = path.join(out, "user");
fs.mkdirSync(path.join(sonaraHome, "config"), { recursive: true });
fs.mkdirSync(path.join(userHome, "Music"), { recursive: true });
fs.writeFileSync(path.join(sonaraHome, "config/settings.json"), JSON.stringify({ audio: { backend: "native-null" }, autosave_seconds: 0 }));
const at = option("at", null);
if (at) {
  const [x, y] = at.split(",").map(Number);
  const panes = { browser: { size: 240, open: true }, inspector: { size: 280, open: true }, editors: { size: 470, open: true } };
  fs.writeFileSync(path.join(sonaraHome, "config/window.json"), JSON.stringify({ x, y, width: 1440, height: 900, maximized: false, panes }));
}
const shots = option("shots", null);
if (shots) fs.mkdirSync(shots, { recursive: true });
const env = { ...process.env, SONARA_HOME: sonaraHome, SONARA_PROJECTS_DIR: out, HOME: userHome };
delete env.SONARA_AGENT_ALLOWED_PATHS;

function run(cmd, cmdArgs, options = {}) {
  console.log(`$ ${cmd} ${cmdArgs.join(" ")}`);
  const result = spawnSync(cmd, cmdArgs, { stdio: "inherit", env: { ...env, HOME: process.env.HOME }, ...options });
  if (result.status !== 0) throw new Error(`${cmd} failed with ${result.status}`);
}

if (!args.includes("--skip-build")) {
  run("pnpm", ["tauri", "build", "--no-bundle", "--debug", "--features", "bench"], { cwd: app });
  run("cargo", ["build", "--locked", "-p", "sonara"], { cwd: root });
}

// The project: two instrument tracks with a clip of four notes each, and an audio track.
const project = path.join(out, "A11y.sonara");
fs.rmSync(project, { recursive: true, force: true });
const notes = [0, 1, 2, 3].map((i) => ({ pitch: 60 + i, start: i * 0.5, length: 0.5, velocity: 100 }));
const steps = [
  { tool: "create_project", args: { name: "A11y", bpm: 120 } },
  { id: "drums", tool: "create_track", args: { name: "Drums 1", kind: "instrument", instrument: "sonara.synth.poly" } },
  { tool: "create_midi_clip", args: { track_id: "{{drums.result.track_id}}", start: 0, length: 4, name: "Beat", notes } },
  { id: "bass", tool: "create_track", args: { name: "Bass", kind: "instrument", instrument: "sonara.synth.poly" } },
  { tool: "create_midi_clip", args: { track_id: "{{bass.result.track_id}}", start: 4, length: 4, name: "Riff", notes } },
  { tool: "create_track", args: { name: "Vox", kind: "audio" } },
  { tool: "save_project", confirm: true, args: { path: project } },
];
fs.writeFileSync(path.join(out, "steps.json"), JSON.stringify({ steps }, null, 1));
run(cli, ["batch", path.join(out, "steps.json"), "--connect", "never", "--allow", out], { cwd: out });

// -- reading the AppKit tree ----------------------------------------------------------------------

/** The JXA that reads the focused element and its ancestors, or every level indicator. */
const READER = `
function run(argv) {
  const pid = Number(argv[0]);
  const mode = argv[1];
  const se = Application("System Events");
  const proc = se.processes.whose({ unixId: pid })[0];
  const NAMES = ["AXRole", "AXSubrole", "AXRoleDescription", "AXTitle", "AXDescription", "AXValue", "AXValueDescription",
    "AXSelected", "AXOrientation", "AXMinValue", "AXMaxValue", "AXFocused", "AXHelp"];
  const read = (el) => {
    const out = {};
    for (const name of NAMES) {
      try {
        const value = el.attributes.byName(name).value();
        if (value !== null && value !== undefined && value !== "") out[name] = value;
      } catch (e) {}
    }
    return out;
  };
  if (mode === "focused") {
    const focused = proc.attributes.byName("AXFocusedUIElement").value();
    if (!focused) return JSON.stringify({ focused: null });
    const ancestors = [];
    let el = focused;
    for (let i = 0; i < 6; i++) {
      try {
        el = el.attributes.byName("AXParent").value();
      } catch (e) {
        break;
      }
      if (!el) break;
      ancestors.push(read(el));
    }
    let siblings = null;
    try {
      const parent = focused.attributes.byName("AXParent").value();
      siblings = parent.uiElements().length;
    } catch (e) {}
    return JSON.stringify({ focused: read(focused), ancestors, siblings });
  }
  // Every level indicator of the front window, breadth first.
  const found = [];
  const queue = [proc.windows[0]];
  let seen = 0;
  while (queue.length > 0 && seen < 4000) {
    const el = queue.shift();
    seen += 1;
    let role = "";
    try {
      role = el.role();
    } catch (e) {}
    if (role === "AXLevelIndicator") found.push(read(el));
    try {
      for (const child of el.uiElements()) queue.push(child);
    } catch (e) {}
  }
  return JSON.stringify({ meters: found, seen });
}
`;
const readerFile = path.join(out, "ax-reader.js");
fs.writeFileSync(readerFile, READER);

/** Captures the front window of `pid` (its frame from System Events) into `file`. */
function capture(pid, file) {
  const frame = spawnSync(
    "osascript",
    ["-l", "JavaScript", "-e", `function run(){const p=Application("System Events").processes.whose({unixId:${pid}})[0];const w=p.windows[0];return JSON.stringify([w.position(),w.size()]);}`],
    { encoding: "utf8", timeout: 30_000 },
  );
  if (frame.status !== 0) throw new Error(`no window frame: ${frame.stderr.trim()}`);
  const [[x, y], [width, height]] = JSON.parse(frame.stdout);
  const shot = spawnSync("screencapture", ["-x", "-o", `-R${x},${y},${width},${height}`, file], { encoding: "utf8", timeout: 30_000 });
  if (shot.status !== 0) throw new Error(`screencapture failed: ${shot.stderr.trim()}`);
  return `${file} (${width} × ${height} points at ${x},${y})`;
}

function readOnce(pid, mode) {
  const result = spawnSync("osascript", ["-l", "JavaScript", readerFile, String(pid), mode], { encoding: "utf8", timeout: 90_000 });
  if (result.status !== 0) throw new Error(`osascript failed: ${result.stderr.trim()}`);
  return JSON.parse(result.stdout);
}

/**
 * The tree's answer; WebKit builds its accessibility tree on the first request, so a focused
 * element that is not there yet is asked for again, for up to 3 s, as an assistive client keeps
 * asking.
 */
async function readTree(pid, mode) {
  let read = readOnce(pid, mode);
  for (let attempt = 0; mode === "focused" && read.focused === null && attempt < 10; attempt++) {
    await new Promise((resolve) => setTimeout(resolve, 300));
    read = readOnce(pid, mode);
  }
  return read;
}

const nameOf = (el) => el?.AXDescription ?? el?.AXTitle ?? "";
const checks = [];
const check = (name, ok, detail = "") => checks.push({ name, ok, detail });
const brief = (el) =>
  el ? `${el.AXRole}${el.AXSubrole ? `/${el.AXSubrole}` : ""} “${nameOf(el)}” (${el.AXRoleDescription ?? "?"})${el.AXValue !== undefined ? ` value ${JSON.stringify(el.AXValue)}` : ""}${el.AXValueDescription ? ` “${el.AXValueDescription}”` : ""}${el.AXSelected !== undefined ? ` selected ${el.AXSelected}` : ""}` : "none";
const ancestorNamed = (read, role, name) => read.ancestors.some((a) => a.AXRole === role && nameOf(a) === name);

/** What each checkpoint expects of the tree. */
const EXPECT = {
  play(read) {
    const el = read.focused;
    check("ADR 0009: Play reads as a toggle button “Play”, not pressed", nameOf(el) === "Play" && /toggle/i.test(el?.AXRoleDescription ?? "") && Number(el?.AXValue) === 0, brief(el));
  },
  "play-pressed"(read) {
    const el = read.focused;
    check("ADR 0009: Play reads as a toggle button “Play”, pressed while playing", nameOf(el) === "Play" && /toggle/i.test(el?.AXRoleDescription ?? "") && Number(el?.AXValue) === 1, brief(el));
  },
  master(read) {
    const el = read.focused;
    check(
      "ADR 0009: the master's fader reads “Master volume”, slider, “0.0 dB”, inside the strip “Channel strip Master”",
      el?.AXRole === "AXSlider" && nameOf(el) === "Master volume" && el?.AXValueDescription === "0.0 dB" && ancestorNamed(read, "AXGroup", "Channel strip Master"),
      brief(el),
    );
    state.masterValue = el?.AXValue;
  },
  "master-down"(read) {
    const el = read.focused;
    check(
      "ADR 0009: a hardware ↓ changes the master's value, read as dB",
      el?.AXRole === "AXSlider" && nameOf(el) === "Master volume" && el?.AXValue !== state.masterValue && /^-\d+\.\d dB$/.test(el?.AXValueDescription ?? ""),
      brief(el),
    );
  },
  tracks(read) {
    const el = read.focused;
    check(
      "ADR 0009: the track list is a list box “Tracks”; its option reads “Drums 1, 0.0 dB”, selected",
      nameOf(el) === "Drums 1, 0.0 dB" && el?.AXSelected === true && read.ancestors.some((a) => nameOf(a) === "Tracks" && /list/i.test(a.AXRoleDescription ?? "")),
      `${brief(el)} in ${brief(read.ancestors[0])}, ${read.siblings} options`,
    );
  },
  "tracks-down"(read) {
    const el = read.focused;
    check("ADR 0009: a hardware ↓ in the track list reads “Bass, 0.0 dB”, selected", nameOf(el) === "Bass, 0.0 dB" && el?.AXSelected === true, brief(el));
  },
  clip(read) {
    const el = read.focused;
    check(
      "a clip reads as its name, start and length, selected, in its track's group of the list box “Clips”",
      nameOf(el) === "Beat, bar 1 beat 1, 4 beats" && el?.AXSelected === true && ancestorNamed(read, "AXGroup", "Drums 1") && read.ancestors.some((a) => nameOf(a) === "Clips"),
      `${brief(el)} in ${read.ancestors.slice(0, 2).map(brief).join(" in ")}`,
    );
  },
  note(read) {
    const el = read.focused;
    check(
      "a note reads “C4, bar 2 beat 1, 1/8, velocity 100”, selected, in the list box “Notes”",
      nameOf(el) === "C4, bar 2 beat 1, 1/8, velocity 100" && el?.AXSelected === true && read.ancestors.some((a) => nameOf(a) === "Notes"),
      `${brief(el)} in ${brief(read.ancestors[0])}`,
    );
  },
  meter(read) {
    const master = read.meters.find((m) => nameOf(m) === "Master level");
    check(
      "the meters are level indicators named after their track; the master's has a level in dB while playing",
      read.meters.length >= 4 && !!master && /dB/.test(master.AXValueDescription ?? "") && Number(master.AXValue) > -60,
      `${read.meters.length} meters (${read.seen} elements read); ${brief(master)}`,
    );
  },
  dialog(read) {
    const el = read.focused;
    check(
      "a dialog takes the focus: the focused element is inside “About Sonara”",
      read.ancestors.some((a) => nameOf(a) === "About Sonara"),
      `${brief(el)} in ${read.ancestors.slice(0, 4).map(brief).join(" in ")}`,
    );
  },
  "dialog-closed"(read) {
    const el = read.focused;
    check("Escape closes the dialog and gives the focus back to the arrangement", nameOf(el) === "Arrangement", brief(el));
  },
};
const state = {};

// -- the run --------------------------------------------------------------------------------------

const report = path.join(out, "e2e-a11y.json");
const checkpointFile = `${report}.checkpoint`;
const resumeFile = `${report}.resume`;
for (const file of [report, checkpointFile, resumeFile]) fs.rmSync(file, { force: true });
const child = spawn(desktop, ["--no-ipc", "--audio", "native-null", project], {
  stdio: ["ignore", "ignore", "pipe"],
  env: { ...env, SONARA_BENCH_OUT: report, SONARA_BENCH_SUITE: "e2e-a11y" },
});
let log = "";
child.stderr.on("data", (chunk) => (log += chunk));
let exited = null;
child.on("exit", (code) => (exited = `exited ${code}`));
const deadline = Date.now() + timeoutMs;
let answered = "";
const reads = [];
while (exited === null && Date.now() < deadline) {
  const name = fs.existsSync(checkpointFile) ? fs.readFileSync(checkpointFile, "utf8").trim() : "";
  if (name && name !== answered && name.startsWith("shot-")) {
    if (shots) {
      try {
        console.log(`captured ${capture(child.pid, path.join(shots, `${name.slice(5)}-real.png`))}`);
      } catch (error) {
        check(`the window could be captured at ${name}`, false, String(error));
      }
    }
    answered = name;
    fs.writeFileSync(resumeFile, name);
  } else if (name && name !== answered) {
    try {
      const read = await readTree(child.pid, name === "meter" ? "meters" : "focused");
      reads.push({ name, read });
      if (dump) console.log(`-- ${name}: ${JSON.stringify(read)}`);
      if (EXPECT[name]) EXPECT[name](read);
      else check(`an unknown checkpoint ${name}`, false);
    } catch (error) {
      check(`the tree at ${name} could be read`, false, String(error));
    }
    answered = name;
    fs.writeFileSync(resumeFile, name);
  }
  await new Promise((resolve) => setTimeout(resolve, 100));
}
if (exited === null) {
  child.kill("SIGKILL");
  exited = "timeout";
}
fs.writeFileSync(path.join(out, "ax-reads.json"), JSON.stringify(reads, null, 1));
if (!fs.existsSync(report)) {
  console.log(log);
  console.log(`no report (${exited})`);
  process.exit(1);
}
const result = JSON.parse(fs.readFileSync(report, "utf8"));
const missing = Object.keys(EXPECT).filter((name) => !reads.some((r) => r.name === name));
if (missing.length > 0) check("every checkpoint was reached", false, missing.join(", "));
for (const c of [...result.checks, ...checks]) console.log(`${c.ok ? "ok  " : "FAIL"} ${c.name}${c.detail ? ` (${c.detail})` : ""}`);
if (result.error) console.log(`error: ${result.error}`);
const valid = result.valid && checks.every((c) => c.ok);
console.log(`the AppKit tree read: ${path.join(out, "ax-reads.json")}`);
console.log(valid ? "end to end: passed" : "end to end: failed");
process.exit(valid ? 0 : 1);
