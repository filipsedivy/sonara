// The window's end-to-end check on macOS: the real application on a real
// project, driven by the page itself (src/lib/bench/e2e.ts) through the `bench` build's hook.
// It replaces WebdriverIO, whose npm tree carries licences outside the allow-list.
//
// 1. builds a debug desktop with the `bench` feature and its frontend embedded (`pnpm tauri build
//    --no-bundle --debug --features bench`) and the debug `sonara` CLI, unless --skip-build;
// 2. builds "E2E.sonara" through `sonara batch` (a "Keys" instrument track with a 4-beat clip, an
//    empty "Vox" audio track) and a test tone, in a sandbox (--out, with its own SONARA_HOME);
// 3. runs the desktop on the simulated audio device with SONARA_BENCH_SUITE=e2e and prints the
//    page's checks; the exit code is 0 when all passed.
//
// The window must stay visible (the page draws and reads its own pixels): keep the screen
// unlocked. A run has a hard timeout. A wait that times out says what the page saw: its
// visibility, its animation frames, the live frames received and shown, and the engine's state.
//
// Usage (from apps/sonara-desktop):
//   node scripts/e2e-timeline.mjs [--out <dir>] [--skip-build] [--at <x>,<y>]

import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { displays, fullScreenApps } from "./macos.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const app = path.resolve(here, "..");
const root = path.resolve(app, "../..");
const args = process.argv.slice(2);
const option = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : fallback;
};
const out = path.resolve(option("out", path.join(os.tmpdir(), `sonara-e2e-${Date.now()}`)));
const desktop = path.join(root, "target/debug/sonara-desktop");
const cli = path.join(root, "target/debug/sonara");
const timeoutMs = 180_000;

fs.mkdirSync(path.join(out, "home/config"), { recursive: true });
fs.writeFileSync(
  path.join(out, "home/config/settings.json"),
  // Autosaves every 2 s, longer than the 1.5 s of "Autosaved", which the window shows.
  JSON.stringify({ audio: { backend: "native-null" }, autosave_seconds: 2 }),
);
// --at <x>,<y> opens the window there (the sandbox's saved layout, global points from the main
// display's top left), as tests/e2e/a11y-tree.mjs does: for example on the built-in display.
const at = option("at", null);
if (at) {
  const [x, y] = at.split(",").map(Number);
  const panes = { browser: { size: 240, open: true }, inspector: { size: 280, open: true }, editors: { size: 470, open: true } };
  fs.writeFileSync(
    path.join(out, "home/config/window.json"),
    JSON.stringify({ x, y, width: 1440, height: 900, maximized: false, panes }),
  );
}
const env = { ...process.env, SONARA_HOME: path.join(out, "home"), SONARA_PROJECTS_DIR: out };
delete env.SONARA_AGENT_ALLOWED_PATHS;

function run(cmd, cmdArgs, options = {}) {
  console.log(`$ ${cmd} ${cmdArgs.join(" ")}`);
  const result = spawnSync(cmd, cmdArgs, { stdio: "inherit", env, ...options });
  if (result.status !== 0) throw new Error(`${cmd} failed with ${result.status}`);
}

if (!args.includes("--skip-build")) {
  run("pnpm", ["tauri", "build", "--no-bundle", "--debug", "--features", "bench"], { cwd: app });
  run("cargo", ["build", "--locked", "-p", "sonara"], { cwd: root });
}

// A 2-second stereo tone, loud then quiet.
function writeTone(file) {
  const rate = 48000;
  const frames = rate * 2;
  const data = Buffer.alloc(frames * 4);
  for (let n = 0; n < frames; n++) {
    const amplitude = n < frames / 2 ? 0.5 : 0.1;
    const sample = Math.round(32767 * amplitude * Math.sin((2 * Math.PI * 220 * n) / rate));
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

const tone = path.join(out, "drop.wav");
writeTone(tone);
const project = path.join(out, "E2E.sonara");
fs.rmSync(project, { recursive: true, force: true });
const steps = [
  { tool: "create_project", args: { name: "E2E", bpm: 120 } },
  { id: "keys", tool: "create_track", args: { name: "Keys", kind: "instrument" } },
  {
    tool: "create_midi_clip",
    args: {
      track_id: "{{keys.result.track_id}}",
      start: 0,
      length: 4,
      name: "Riff",
      notes: [0, 1, 2, 3].map((i) => ({ pitch: 60 + i, start: i * 0.5, length: 0.5, velocity: 100 })),
    },
  },
  { tool: "create_track", args: { name: "Vox", kind: "audio" } },
  { tool: "save_project", confirm: true, args: { path: project } },
];
fs.writeFileSync(path.join(out, "steps.json"), JSON.stringify({ steps }, null, 1));
run(cli, ["batch", path.join(out, "steps.json"), "--connect", "never", "--allow", out], { cwd: out });

const report = path.join(out, "e2e.json");
fs.rmSync(report, { force: true });
const outcome = await new Promise((resolve) => {
  const child = spawn(desktop, ["--no-ipc", "--audio", "native-null", project], {
    stdio: ["ignore", "ignore", "pipe"],
    env: { ...env, SONARA_BENCH_OUT: report, SONARA_BENCH_SUITE: "e2e", SONARA_BENCH_MEDIA: tone },
  });
  let log = "";
  child.stderr.on("data", (chunk) => (log += chunk));
  const timer = setTimeout(() => {
    child.kill("SIGKILL");
    resolve({ how: "timeout", log });
  }, timeoutMs);
  child.on("exit", (code) => {
    clearTimeout(timer);
    resolve({ how: `exited ${code}`, log });
  });
});
if (!fs.existsSync(report)) {
  console.log(outcome.log);
  console.log(`no report (${outcome.how})`);
  process.exit(1);
}
const result = JSON.parse(fs.readFileSync(report, "utf8"));
for (const c of result.checks) console.log(`${c.ok ? "ok  " : "FAIL"} ${c.name}${c.detail ? ` (${c.detail})` : ""}`);
// The checks pass in a window that is not drawn too (a hidden page gets no animation frames and
// applies the live frames as they arrive); say so, since nothing was drawn then.
const played = result.checks.find((c) => c.name === "the transport plays and its position advances");
if (played?.detail?.includes("page hidden")) {
  const [x, y] = (at ?? "").split(",").map(Number);
  const display = displays().find((d) => x >= d.x && x < d.x + d.width && y >= d.y && y < d.y + d.height) ?? displays()[0];
  const full = display ? fullScreenApps(display) : [];
  console.log(
    `note: the window was not drawn${full.length ? ` (${full.join(", ")} is full screen on ${display.name})` : " (hidden, minimized or on another Space)"}`,
  );
}
if (result.error) console.log(`error: ${result.error}`);
console.log(result.valid ? "end to end: passed" : "end to end: failed");
process.exit(result.valid ? 0 : 1);
