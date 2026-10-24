// The end-to-end check of the browser, the inspector and the dialogs on
// macOS: the real application on a real project, driven by the page itself
// (src/lib/bench/e2e-dialogs.ts) through the `bench` build's hook, Sonara's own harness.
//
// 1. builds a debug desktop with the `bench` feature and its frontend embedded, and the debug
//    `sonara` CLI, unless --skip-build;
// 2. makes a sandbox (--out): its own SONARA_HOME with a settings file, its own HOME whose Music
//    folder holds "e2e tone.wav" (the Files tab's root), and "E2E.sonara" built by `sonara batch`
//    (a "Keys" instrument track with a clip, an empty "Vox" audio track);
// 3. runs the desktop on the simulated audio device with SONARA_BENCH_SUITE=e2e-dialogs, prints
//    the page's checks, then checks what the run left on disk: the settings file the Settings
//    dialog wrote (its values, the prototype's layout and mode 0600), and the Export dialog's WAV
//    file. The exit code is 0 when all passed.
//
// The window must stay visible (the page reads its own panes): keep the screen unlocked. A run
// has a hard timeout.
//
// Usage (from apps/sonara-desktop): node scripts/e2e-dialogs.mjs [--out <dir>] [--skip-build]

import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const app = path.resolve(here, "..");
const root = path.resolve(app, "../..");
const args = process.argv.slice(2);
const option = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : fallback;
};
const out = path.resolve(option("out", path.join(os.tmpdir(), `sonara-e2e-dialogs-${Date.now()}`)));
const desktop = path.join(root, "target/debug/sonara-desktop");
const cli = path.join(root, "target/debug/sonara");
const timeoutMs = 180_000;

const sonaraHome = path.join(out, "home");
const userHome = path.join(out, "user");
fs.mkdirSync(path.join(sonaraHome, "config"), { recursive: true });
fs.mkdirSync(path.join(userHome, "Music"), { recursive: true });
fs.mkdirSync(path.join(out, "export"), { recursive: true });
const settingsFile = path.join(sonaraHome, "config/settings.json");
fs.writeFileSync(settingsFile, JSON.stringify({ audio: { backend: "native-null" }, autosave_seconds: 0, window: { kept: true } }));
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

// A 2-second stereo tone, 16-bit PCM.
function writeTone(file) {
  const rate = 48000;
  const frames = rate * 2;
  const data = Buffer.alloc(frames * 4);
  for (let n = 0; n < frames; n++) {
    const sample = Math.round(32767 * 0.3 * Math.sin((2 * Math.PI * 330 * n) / rate));
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

writeTone(path.join(userHome, "Music", "e2e tone.wav"));
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

const report = path.join(out, "e2e-dialogs.json");
fs.rmSync(report, { force: true });
const outcome = await new Promise((resolve) => {
  const child = spawn(desktop, ["--no-ipc", "--audio", "native-null", project], {
    stdio: ["ignore", "ignore", "pipe"],
    env: { ...env, SONARA_BENCH_OUT: report, SONARA_BENCH_SUITE: "e2e-dialogs" },
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

// What the run left on disk.
const disk = [];
const check = (name, ok, detail = "") => disk.push({ name, ok, detail });
const text = fs.readFileSync(settingsFile, "utf8");
const saved = JSON.parse(text);
check(
  "the Settings dialog saved its choices",
  saved.theme === "light" &&
    saved.undo_limit === 2000 &&
    JSON.stringify(saved.agent_allowed_paths) === JSON.stringify(["/Volumes/Audio", "~/Samples"]) &&
    saved.audio.backend === "native-null",
  `theme ${saved.theme}, undo ${saved.undo_limit}, paths ${JSON.stringify(saved.agent_allowed_paths)}`,
);
check(
  "the settings file has the prototype's layout: unknown keys first, two-space indent, a final newline",
  Object.keys(saved)[0] === "window" &&
    Object.keys(saved)[1] === "version" &&
    text.startsWith('{\n  "window": {\n    "kept": true\n  },\n  "version": 1,\n  "audio": {\n    "backend": "native-null",') &&
    text.endsWith("}\n") &&
    text.includes('"lookahead_ms": 80.0'),
  text.split("\n").slice(0, 3).join(" "),
);
check("the settings file is readable by its owner only", (fs.statSync(settingsFile).mode & 0o777) === 0o600);
const wav = path.join(out, "export", "E2E Mix.wav");
const exported = fs.existsSync(wav) ? fs.readFileSync(wav) : null;
check(
  "the Export dialog wrote the WAV file",
  !!exported && exported.toString("ascii", 0, 4) === "RIFF" && exported.toString("ascii", 8, 12) === "WAVE",
  exported ? `${exported.length} bytes` : "missing",
);

for (const c of [...result.checks, ...disk]) console.log(`${c.ok ? "ok  " : "FAIL"} ${c.name}${c.detail ? ` (${c.detail})` : ""}`);
if (result.error) console.log(`error: ${result.error}`);
const valid = result.valid && disk.every((c) => c.ok);
console.log(valid ? "end to end: passed" : "end to end: failed");
process.exit(valid ? 0 : 1);
