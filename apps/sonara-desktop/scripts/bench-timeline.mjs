// The timeline, meter and piano roll benchmarks on the product with 100 tracks × 40 clips. It is
// the ADR 0009 spike's runner adapted to the product. `--suite` chooses:
//
// - `timeline` (the default; ADR 0009's threshold: draw p95 ≤ 7 ms and no frame over 16.6 ms in
//   10 s of scrolling and zooming), on the `null` backend; the page's src/lib/bench/timeline.ts;
// - `meters` (ADR 0009's threshold: at least 58 fps, less than 1 % dropped frames and a p99
//   latency from the Rust sample to the pixel of at most 50 ms), the project playing on the
//   simulated device (`native-null`, the real engine); the page's src/lib/bench/meters.ts, with a
//   fader drag and a knob drag in the Devices tab, with the device refresh and with whole
//   views to compare;
// - `pianoroll` (the owner's restated threshold: drag, resize and lasso on a 2,000-note clip respond
//   within 2 display frames, pointer to pixel, and 10,000 notes paint within one display frame), on
//   the `null` backend, the project with two more instrument tracks: "Piano 2k" (the spike's
//   2,000-note clip: 256 beats, pitches 36–95) and "Piano 10k" (the prototype's
//   `test_paint_10k_notes_is_fast`: 10,000 notes over 200 beats, pitches 30–96); the page's
//   src/lib/bench/pianoroll.ts. Every move sends its edit to the real session;
// - `playback` (0 underruns while the timeline scrolls and the meters run), a copy of
//   `--project` (Night Drive or Ascension) with its master at −144 dB, so nothing is audible,
//   playing on the system's audio device (`native`) for 60 s while the view scrolls and zooms and
//   the Mixer tab's meters draw; the page's src/lib/bench/playback.ts.
//
// 1. builds the release desktop with the `bench` feature (`pnpm tauri build --no-bundle
//    --features bench`) and the release `sonara` CLI, unless --skip-build;
// 2. writes four test tones and builds "Timeline Bench.sonara" through `sonara batch`: 50 audio
//    tracks (linked tones, each clip duplicated to 40) and 50 instrument tracks (a 6-note clip
//    duplicated to 40), interleaved, 160 beats long;
// 3. runs the desktop on it `--runs` times with SONARA_BENCH_OUT; the page measures and writes a
//    report, then quits (src/lib/bench/timeline.ts). A run whose report is invalid (the window
//    was hidden, a frame gap over 250 ms, an error) is repeated, up to four attempts;
// 4. writes summary.json and summary.md with the median over runs and the worst run.
//
// The window must stay visible: keep the screen unlocked and the machine untouched while it runs
// (about 3 minutes per run, 1 for the meters). A locked screen, or another app's full-screen Space
// on the window's display (which hides the window: no animation frames), is waited for at most
// --unlock-wait-s seconds (then the script says so and exits with 2), and each run has a hard
// timeout. Everything lives in --out (a sandboxed SONARA_HOME included); nothing touches the
// owner's settings or projects. --at <x>,<y> opens the window there (the sandbox's saved layout,
// global points from the main display's top left), for example on the built-in display.
//
// Usage (from apps/sonara-desktop):
//   node scripts/bench-timeline.mjs [--suite timeline|meters|pianoroll|playback] [--runs 3] [--out <dir>]
//                                   [--quick] [--skip-build] [--timeout-s 600] [--unlock-wait-s 600]
//                                   [--at <x>,<y>] [--project <path.sonara>]
//
// --debug builds and runs debug binaries instead (to check the harness; its numbers do not count).

import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { activate, displays, frontmost, fullScreenApps, screenLocked } from "./macos.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const app = path.resolve(here, "..");
const root = path.resolve(app, "../..");
const args = process.argv.slice(2);
const option = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : fallback;
};
const suite = option("suite", "timeline");
if (!["timeline", "meters", "pianoroll", "playback"].includes(suite)) throw new Error(`unknown suite ${suite}`);
const sourceProject = option("project", null);
if (suite === "playback" && !sourceProject) throw new Error("the playback suite needs --project <path.sonara>");
const runs = Number(option("runs", "3"));
const out = path.resolve(option("out", path.join(os.tmpdir(), `sonara-${suite}-bench-${Date.now()}`)));
const unlockWaitMs = Number(option("unlock-wait-s", "600")) * 1000;
// The meters need the real engine: the simulated device plays the project; the playback check
// plays on the system's audio device.
const backendName = { meters: "native-null", playback: "native" }[suite] ?? "null";
const quick = args.includes("--quick");
const skipBuild = args.includes("--skip-build");
const timeoutMs = Number(option("timeout-s", "600")) * 1000;
const attemptsPerRun = 4;
const debug = args.includes("--debug");
const profile = debug ? "debug" : "release";
const desktop = path.join(root, `target/${profile}/sonara-desktop`);
const cli = path.join(root, `target/${profile}/sonara`);

fs.mkdirSync(out, { recursive: true });
const home = path.join(out, "home");
fs.mkdirSync(path.join(home, "config"), { recursive: true });
// No sound card, no autosave: the timeline only draws; the meters play on the simulated device.
fs.writeFileSync(
  path.join(home, "config/settings.json"),
  JSON.stringify({ audio: { backend: backendName }, autosave_seconds: 0 }),
);
const at = option("at", null);
const windowAt = at ? at.split(",").map(Number) : null;
if (windowAt) {
  const panes = { browser: { size: 240, open: true }, inspector: { size: 280, open: true }, editors: { size: 470, open: true } };
  fs.writeFileSync(
    path.join(home, "config/window.json"),
    JSON.stringify({ x: windowAt[0], y: windowAt[1], width: 1440, height: 900, maximized: false, panes }),
  );
}
const env = { ...process.env, SONARA_HOME: home, SONARA_PROJECTS_DIR: out };
delete env.SONARA_AGENT_ALLOWED_PATHS;

function run(cmd, cmdArgs, options = {}) {
  console.log(`$ ${cmd} ${cmdArgs.join(" ")}`);
  const result = spawnSync(cmd, cmdArgs, { stdio: "inherit", env, ...options });
  if (result.status !== 0) throw new Error(`${cmd} failed with ${result.status}`);
}

function sh(cmd, cmdArgs) {
  return (spawnSync(cmd, cmdArgs, { encoding: "utf8" }).stdout || "").trim();
}

// -- 1. the binaries -------------------------------------------------------------------------------

if (!skipBuild) {
  run("pnpm", ["tauri", "build", "--no-bundle", "--features", "bench", ...(debug ? ["--debug"] : [])], { cwd: app });
  run("cargo", ["build", "--locked", "-p", "sonara", ...(debug ? [] : ["--release"])], { cwd: root });
}
for (const binary of [desktop, cli]) {
  if (!fs.existsSync(binary)) throw new Error(`${binary} is missing: run without --skip-build`);
}

// -- 2. the project ---------------------------------------------------------------------------------

/** A 2-second stereo 16-bit tone at 48 kHz with an envelope of its own. */
function writeTone(file, seed) {
  const rate = 48000;
  const frames = rate * 2;
  const data = Buffer.alloc(frames * 4);
  for (let n = 0; n < frames; n++) {
    const t = n / rate;
    const envelope = Math.abs(Math.sin(Math.PI * (t * (seed + 1)) / 2)) * (0.3 + 0.15 * seed);
    const sample = Math.round(32767 * envelope * Math.sin(2 * Math.PI * (110 * (seed + 1)) * t));
    data.writeInt16LE(sample, n * 4);
    data.writeInt16LE(-sample, n * 4 + 2);
  }
  const header = Buffer.alloc(44);
  header.write("RIFF", 0);
  header.writeUInt32LE(36 + data.length, 4);
  header.write("WAVEfmt ", 8);
  header.writeUInt32LE(16, 16);
  header.writeUInt16LE(1, 20);
  header.writeUInt16LE(2, 22);
  header.writeUInt32LE(rate, 24);
  header.writeUInt32LE(rate * 4, 28);
  header.writeUInt16LE(4, 32);
  header.writeUInt16LE(16, 34);
  header.write("data", 36);
  header.writeUInt32LE(data.length, 40);
  fs.writeFileSync(file, Buffer.concat([header, data]));
}

const media = path.join(out, "media");
fs.mkdirSync(media, { recursive: true });
const tones = [0, 1, 2, 3].map((seed) => {
  const file = path.join(media, `tone-${seed + 1}.wav`);
  writeTone(file, seed);
  return file;
});
/** A seeded generator of floats in [0, 1) (mulberry32), so the clips are the same every run. */
function random(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** The piano roll's clips: the spike's 2,000 notes and the prototype's 10,000. */
function pianoSteps() {
  const steps = [];
  const two = random(2000);
  const lengths = [0.25, 0.5, 1, 2];
  const spike = Array.from({ length: 2000 }, () => {
    const start = Math.floor(two() * 1024) * 0.25;
    return {
      pitch: 36 + Math.floor(two() * 60),
      start,
      length: Math.min(lengths[Math.floor(two() * 4)], 256 - start),
      velocity: 40 + Math.floor(two() * 88),
    };
  });
  const ten = random(10000);
  const prototype = Array.from({ length: 10000 }, () => ({
    pitch: 30 + Math.floor(ten() * 67),
    start: Math.round(ten() * 200 * 1e6) / 1e6,
    length: 0.25,
    velocity: 1 + Math.floor(ten() * 127),
  }));
  for (const [name, length, notes] of [
    ["Piano 2k", 256, spike],
    ["Piano 10k", 204, prototype],
  ]) {
    const id = name.replace(/\W/g, "");
    steps.push({ id, tool: "create_track", args: { name, kind: "instrument" } });
    steps.push({ tool: "create_midi_clip", args: { track_id: `{{${id}.result.track_id}}`, start: 0, length, name, notes } });
  }
  return steps;
}

/** Copies the project bundle `from` into the sandbox, without its backups, with its master at −144 dB. */
function silentCopy(from) {
  const copy = path.join(out, path.basename(from));
  fs.rmSync(copy, { recursive: true, force: true });
  fs.cpSync(from, copy, { recursive: true, filter: (source) => path.basename(source) !== "backups" });
  const steps = [
    { tool: "open_project", confirm: true, args: { path: copy } },
    // smoke_play's SetTrackMixer: the graph still renders, nothing is audible.
    { tool: "set_track_volume", args: { track_id: "master", volume_db: -144 } },
    { tool: "save_project", confirm: true, args: {} },
  ];
  const batch = path.join(out, "silence.json");
  fs.writeFileSync(batch, JSON.stringify({ description: "The playback check's silent copy", steps }, null, 1));
  run(cli, ["batch", batch, "--connect", "never", "--allow", out], { cwd: out });
  const saved = JSON.parse(fs.readFileSync(path.join(copy, "project.json"), "utf8"));
  if (!(saved.master?.mixer?.volume_db <= -143.9)) throw new Error(`the copy's master is at ${saved.master?.mixer?.volume_db} dB`);
  return copy;
}

const projectPath =
  suite === "playback"
    ? silentCopy(path.resolve(sourceProject))
    : path.join(out, suite === "pianoroll" ? "Piano Roll Bench.sonara" : "Timeline Bench.sonara");
if (!fs.existsSync(projectPath)) {
  const steps = [{ tool: "create_project", args: { name: "Timeline Bench", bpm: 120 } }];
  for (let i = 0; i < 100; i++) {
    const id = `t${i}`;
    if (i % 2 === 0) {
      steps.push({ id, tool: "import_audio", args: { path: tones[(i / 2) % 4], mode: "link", track_name: `Audio ${i / 2 + 1}` } });
      steps.push({ tool: "duplicate_clip", args: { clip_id: `{{${id}.clip_id}}`, times: 39 } });
    } else {
      steps.push({ id, tool: "create_track", args: { name: `Synth ${(i + 1) / 2}`, kind: "instrument" } });
      const notes = Array.from({ length: 6 }, (_, j) => ({
        pitch: 48 + ((j * 5 + i) % 24),
        start: j * 0.5,
        length: 0.4,
        velocity: 64 + j * 10,
      }));
      steps.push({
        id: `${id}c`,
        tool: "create_midi_clip",
        args: { track_id: `{{${id}.result.track_id}}`, start: 0, length: 4, name: `c${i}`, notes },
      });
      steps.push({ tool: "duplicate_clip", args: { clip_id: `{{${id}c.result.clip_id}}`, times: 39 } });
    }
  }
  if (suite === "pianoroll") steps.push(...pianoSteps());
  steps.push({ tool: "save_project", confirm: true, args: { path: projectPath } });
  const batch = path.join(out, "steps.json");
  fs.writeFileSync(batch, JSON.stringify({ description: "The timeline benchmark's project", steps }, null, 1));
  run(cli, ["batch", batch, "--connect", "never", "--allow", out], { cwd: out });
}

// -- 3. the runs --------------------------------------------------------------------------------------

/** The display the window opens on: the one under --at, else the main display. */
function windowDisplay() {
  const all = displays();
  if (!windowAt) return all[0];
  const [x, y] = windowAt;
  return all.find((d) => x >= d.x && x < d.x + d.width && y >= d.y && y < d.y + d.height) ?? all[0];
}

/** Why the window would not be drawn now: a locked screen, or a full-screen app on its display. */
function hidingCause() {
  if (screenLocked()) return "the screen is locked";
  const display = windowDisplay();
  const apps = display ? fullScreenApps(display) : [];
  return apps.length ? `${apps.join(", ")} is full screen on ${display.name}` : null;
}

const rerun = () =>
  `cd ${app} && node scripts/bench-timeline.mjs ${args.filter((a) => a !== "--skip-build").join(" ")} --skip-build`;

async function waitForUnlockedScreen() {
  let cause = hidingCause();
  if (!cause) return;
  console.log(`${cause}: waiting up to ${unlockWaitMs / 1000} s (WebKit draws nothing in a hidden window)`);
  const deadline = Date.now() + unlockWaitMs;
  while ((cause = hidingCause())) {
    if (Date.now() > deadline) {
      console.log(`${cause}, still. Unlock the screen, leave full screen on that display, and run:`);
      console.log(`  ${rerun()}`);
      caffeinate.kill();
      process.exit(2);
    }
    await new Promise((resolve) => setTimeout(resolve, 5000));
  }
  await new Promise((resolve) => setTimeout(resolve, 20000));
}

function launch(report) {
  return new Promise((resolve) => {
    const child = spawn(desktop, ["--no-ipc", "--audio", backendName, projectPath], {
      stdio: ["ignore", "ignore", "pipe"],
      env: {
        ...env,
        SONARA_BENCH_OUT: report,
        SONARA_BENCH_SUITE: suite,
        ...(quick ? { SONARA_BENCH_QUICK: "1" } : {}),
      },
    });
    let log = "";
    child.stderr.on("data", (chunk) => (log += chunk));
    // The window opens on top and takes the focus; when another application kept the front, it
    // is asked once more (through System Events, which may lack the accessibility access).
    const front = setTimeout(() => {
      try {
        if (frontmost().pid !== child.pid && !activate(child.pid)) log += "\nthe window could not be brought to the front\n";
      } catch (error) {
        log += `\n${error}\n`;
      }
    }, 4000);
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      resolve({ how: "timeout", log });
    }, timeoutMs);
    child.on("exit", (code) => {
      clearTimeout(timer);
      clearTimeout(front);
      resolve({ how: `exited ${code}`, log });
    });
  });
}

function problems(report) {
  const found = [];
  if (report.error) found.push(`error: ${report.error.split("\n")[0]}`);
  const cause = hidingCause();
  if (cause) found.push(cause);
  if (report.valid === false) {
    for (const s of report.invalidSegments ?? []) {
      found.push(`invalid segment: gap ${Math.round(s.maxRafGapMs)} ms, events ${s.events.join(",")}`);
    }
  }
  return found;
}

const caffeinate = spawn("caffeinate", ["-d", "-i", "-u"], { stdio: "ignore" });
for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
  process.on(signal, () => {
    caffeinate.kill();
    spawnSync("pkill", ["-f", desktop]);
    process.exit(130);
  });
}
const accepted = [];
const rejected = [];
try {
  for (let runNumber = 1; runNumber <= runs; runNumber++) {
    for (let attempt = 1, counted = 0; counted < attemptsPerRun; attempt++) {
      const file = path.join(out, `run-${runNumber}-attempt-${attempt}.json`);
      fs.rmSync(file, { force: true });
      await waitForUnlockedScreen();
      const started = Date.now();
      let lockedDuring = false;
      const watch = setInterval(() => {
        if (screenLocked()) lockedDuring = true;
      }, 2000);
      console.log(`run ${runNumber} attempt ${attempt}: started`);
      const { how, log } = await launch(file);
      clearInterval(watch);
      if (!lockedDuring) counted += 1;
      const seconds = Math.round((Date.now() - started) / 1000);
      if (!fs.existsSync(file)) {
        console.log(`run ${runNumber} attempt ${attempt}: no report (${how}, ${seconds} s)`);
        fs.writeFileSync(`${file}.log`, log);
        rejected.push({ run: runNumber, attempt, reasons: [`no report (${how})`] });
        continue;
      }
      const report = JSON.parse(fs.readFileSync(file, "utf8"));
      const found = problems(report);
      if (lockedDuring) found.push("the screen was locked during the run");
      if (found.length === 0) {
        console.log(`run ${runNumber} attempt ${attempt}: valid (${seconds} s)`);
        accepted.push(report);
        break;
      }
      console.log(`run ${runNumber} attempt ${attempt}: rejected (${seconds} s): ${found.join("; ")}`);
      rejected.push({ run: runNumber, attempt, reasons: found });
    }
  }
} finally {
  caffeinate.kill();
}

// -- 4. the summary -------------------------------------------------------------------------------------

function leaves(value, prefix = "", into = new Map()) {
  if (typeof value === "number") into.set(prefix, value);
  else if (value && typeof value === "object" && !Array.isArray(value)) {
    for (const [key, child] of Object.entries(value)) leaves(child, prefix ? `${prefix}.${key}` : key, into);
  }
  return into;
}

const median = (values) => {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
};

const perRun = accepted.map((report) => leaves(report));
const medians = {};
const worst = {};
if (perRun.length > 0) {
  for (const key of perRun[0].keys()) {
    const values = perRun.map((m) => m.get(key)).filter((v) => typeof v === "number");
    if (values.length !== perRun.length) continue;
    medians[key] = Math.round(median(values) * 1000) / 1000;
    worst[key] = Math.max(...values);
  }
}

const rows = [];
const check = (test, metric, key, limit, compare, note = "") => {
  const value = medians[key];
  if (value === undefined) {
    rows.push({ test, metric, value: "n/a", limit, verdict: "not measured", note });
    return;
  }
  const pass = compare === "<=" ? value <= limit : value < limit;
  rows.push({ test, metric, value, worstRun: worst[key], limit: `${compare} ${limit}`, verdict: pass ? "PASS" : "FAIL", note });
};
const atLeast = (test, metric, key, limit, note = "") => {
  const value = medians[key];
  if (value === undefined) {
    rows.push({ test, metric, value: "n/a", limit, verdict: "not measured", note });
    return;
  }
  // The worst run of a floor is the lowest.
  const values = perRun.map((m) => m.get(key)).filter((v) => typeof v === "number");
  rows.push({ test, metric, value, worstRun: Math.min(...values), limit: `>= ${limit}`, verdict: value >= limit ? "PASS" : "FAIL", note });
};
if (suite === "timeline") {
  for (const scenario of ["working", "overview"]) {
    const base = `timeline.${scenario}`;
    check(`Timeline ${scenario}`, "draw+sync p95 ms", `${base}.synced.drawMs.p95`, 7, "<=");
    check(`Timeline ${scenario}`, "draw+sync max ms", `${base}.synced.drawMs.max`, 16.6, "<=");
    check(`Timeline ${scenario}`, "draw (CPU) p95 ms", `${base}.cpu.drawMs.p95`, 7, "<=");
    check(`Timeline ${scenario}`, "draw (CPU) max ms", `${base}.cpu.drawMs.max`, 16.6, "<=");
    check(`Timeline ${scenario}`, "frame interval p95 ms (CPU pass)", `${base}.cpu.rafIntervalMs.p95`, 16.6 * 1.5, "<=", "informational");
  }
  check("Project view", "request p95 ms", "view.requestMs.p95", 50, "<=", "informational");
  check("Project view", "JSON parse p95 ms", "view.parseMs.p95", 16.6, "<=", "informational");
  check("Project view", "empty request p95 ms", "view.emptyRequestMs.p95", 5, "<=", "informational");
  check("Project view", "model p95 ms", "view.modelMs.p95", 16.6, "<=", "informational");
  check("Project view", "scene rebuild and draw p95 ms", "view.rebuildAndDrawMs.p95", 50, "<=", "informational");
} else if (suite === "pianoroll") {
  for (const kind of ["move", "resize", "lasso"]) {
    const base = `pointer.${kind}`;
    const title = `Piano roll ${kind} (2,000 notes)`;
    check(title, "pointer to pixel p95, display frames", `${base}.pointerToPixelFrames.p95`, 2, "<=");
    check(title, "pointer to pixel max, display frames", `${base}.pointerToPixelFrames.max`, 2, "<=", "informational");
    atLeast(title, "events within 2 frames %", `${base}.withinTwoFramesPercent`, 95, "informational");
    check(title, "pointer to pixel p50 ms", `${base}.pointerToPixelMs.p50`, 33.4, "<=", "informational");
    check(title, "pointer to pixel p95 ms", `${base}.pointerToPixelMs.p95`, 33.4, "<=", "informational; 2 frames at 60 Hz");
    check(title, "pointer to drawn p95 ms", `${base}.pointerToDrawnMs.p95`, 16.7, "<=", "informational");
    check(title, "long frames (> 1.5 frames)", `${base}.longFrames`, 0, "<=", "informational");
    atLeast(title, "events shown", `${base}.shown`, 1, "informational");
  }
  check("Piano roll 10,000 notes", "draw (CPU) p95 ms", "paint.cpu.drawMs.p95", 16.6, "<=");
  check("Piano roll 10,000 notes", "draw (CPU) max ms", "paint.cpu.drawMs.max", 16.6, "<=", "informational");
  check("Piano roll 10,000 notes", "draw+sync p95 ms", "paint.synced.drawMs.p95", 16.6, "<=");
  check("Piano roll 10,000 notes", "draw+sync max ms", "paint.synced.drawMs.max", 16.6, "<=", "informational");
  atLeast("Piano roll 10,000 notes", "notes drawn", "paint.cpu.notesDrawn", 9000, "informational");
} else if (suite === "playback") {
  const title = `Playback of ${path.basename(projectPath, ".sonara")} (native, master −144 dB)`;
  check(title, "underruns", "playback.underruns", 0, "<=");
  check(title, "device errors", "playback.deviceErrors", 0, "<=");
  atLeast(title, "transport rolled %", "playback.rolledPercent", 100);
  atLeast(title, "position reached s", "playback.positionSeconds", quick ? 7 : 55, "informational");
  check(title, "render load", "playback.renderLoad", 0.5, "<=", "informational");
  check(title, "frame interval p95 ms", "playback.rafIntervalMs.p95", 16.6 * 1.5, "<=", "informational");
  check(title, "long frames (> 1.5 frames)", "playback.longFrames", 0, "<=", "informational");
  atLeast(title, "meter fps while scrolling", "playback.meterFps", 58, "informational");
} else {
  for (const [segment, title, note] of [
    ["meters", "Meters", ""],
    ["drag", "Meters during a fader drag", "informational"],
    ["knob", "Meters during a knob drag (Devices tab)", "informational"],
    ["knobWhole", "Meters during a knob drag (Devices tab), whole views to compare", "informational"],
  ]) {
    atLeast(title, "meter fps", `${segment}.meterFps`, 58, note);
    check(title, "dropped %", `${segment}.droppedPercent`, 1, "<", note);
    check(title, "p99 Rust sample to pixel ms", `${segment}.latencyToPixelMs.p99`, 50, "<=", note);
    check(title, "p50 Rust sample to pixel ms", `${segment}.latencyToPixelMs.p50`, 50, "<=", "informational");
    check(title, "frame interval p95 ms", `${segment}.rafIntervalMs.p95`, 16.6 * 1.5, "<=", "informational");
    check(title, "long frames (> 25 ms)", `${segment}.longFrames`, 0, "<=", "informational");
    check(title, "underruns", `${segment}.underruns`, 0, "<=", "informational");
  }
  atLeast("Meters", "meter values per frame", "meters.meterValues", 200);
  check("Meters", "frame build ms", "frameCostMs", 16.6, "<=", "informational; before the stamp");
  atLeast("Meters during a fader drag", "edits sent", "drag.edits", 1, "informational");
  atLeast("Meters during a knob drag (Devices tab)", "edits sent", "knob.edits", 1, "informational");
  check("Meters during a knob drag (Devices tab)", "whole views", "knob.viewRequests", 0, "<=", "informational");
  check("Meters during a knob drag (Devices tab)", "view update p95 ms", "knob.viewUpdateMs.p95", 16.6, "<=", "informational");
}

const display = windowDisplay();
const environment = {
  display: display ? `${display.name} (${display.scale}×, up to ${display.maxFps} Hz)` : null,
  macOS: sh("sw_vers", ["-productVersion"]),
  build: sh("sw_vers", ["-buildVersion"]),
  cpu: sh("sysctl", ["-n", "machdep.cpu.brand_string"]),
  devicePixelRatio: medians["env.devicePixelRatio"],
  viewBytes: medians["view.bytes"],
  strips: medians["env.strips"],
  frameMs: medians["env.frameMs"],
};
if (environment.devicePixelRatio !== undefined && environment.devicePixelRatio !== 2) {
  rows.push({ test: "Environment", metric: "device pixel ratio", value: environment.devicePixelRatio, limit: "= 2", verdict: quick ? "informational" : "FAIL", note: "step 12 measures at 2×" });
}
const summary = { suite, environment, profile, quick, runsAccepted: accepted.length, rejected, rows, medians, worst, rerun: rerun() };
fs.writeFileSync(path.join(out, "summary.json"), JSON.stringify(summary, null, 2));
const table = [
  "| Test | Metric | Median of runs | Worst run | Threshold | Verdict |",
  "|---|---|---|---|---|---|",
  ...rows.map((r) => `| ${r.test} | ${r.metric} | ${r.value} | ${r.worstRun ?? ""} | ${r.limit} | ${r.verdict}${r.note ? ` (${r.note})` : ""} |`),
];
fs.writeFileSync(path.join(out, "summary.md"), `${table.join("\n")}\n`);
console.log(JSON.stringify(environment));
console.log(`accepted runs: ${accepted.length}, rejected attempts: ${rejected.length}; reports in ${out}`);
if (accepted.length !== runs) console.log(`rerun: ${rerun()}`);
console.log(table.join("\n"));
process.exit(accepted.length === runs ? 0 : 1);
