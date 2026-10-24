// The owner's performance sign-off of the GUI, in one command (docs/audio-engine.md,
// "The GUI's performance sign-off"). On the built-in display at 2×, with the release `bench` build:
//
// 1. the timeline benchmark (draw p95 ≤ 7 ms and no frame over 16.6 ms), 3 runs;
// 2. the meter benchmark (at least 58 fps, less than 1 % dropped, p99 ≤ 50 ms), with its fader and
//    knob segments, 3 runs;
// 3. the piano-roll benchmark (within 2 display frames; 10,000 notes within one frame), 3 runs;
// 4. Night Drive and Ascension playing on the system's audio device with the master at −144 dB
//    (inaudible) while the timeline scrolls and the meters run: 0 underruns.
//
// Each part is scripts/bench-timeline.mjs with its own sandbox under --out; this script builds once,
// places the window on the built-in display, waits for an unlocked screen and for no full-screen app
// there, gives every part a hard timeout, and writes one summary.md with every number against its
// threshold, and the command that reruns what did not pass. The numbers belong in
// docs/audio-engine.md; they are never CI gates (HW-01).
//
// Before you run it: the Mac idle and on mains power, the screen unlocked, no full-screen app on the
// built-in display, about 30 minutes. Nothing is audible, and nothing touches your settings or
// projects (the projects are copied into the sandbox).
//
// Usage (from apps/sonara-desktop):
//   pnpm run perf:signoff [-- --quick] [--skip-build] [--runs 3] [--out <dir>] [--only <parts>]
//                          [--display builtin|main|external] [--no-wait] [--wait-s 900]
//
// --quick: one short run of each part (seconds instead of minutes), to check the harness; its
// numbers do not count. --only: a comma-separated subset of timeline, meters, pianoroll,
// playback-night-drive, playback-ascension. --display main or external: the main display, or the
// first display that is not built in, instead of the built-in one (a check of the script; not a
// sign-off).

import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { builtinDisplay, centredOn, displays, fullScreenApps, idleSeconds, screenLocked } from "./macos.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const app = path.resolve(here, "..");
const root = path.resolve(app, "../..");
const args = process.argv.slice(2).filter((a) => a !== "--");
const option = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : fallback;
};
const quick = args.includes("--quick");
const skipBuild = args.includes("--skip-build");
const noWait = args.includes("--no-wait");
const runs = quick ? 1 : Number(option("runs", "3"));
const stamp = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
const out = path.resolve(option("out", path.join(root, "target/perf-signoff", `${quick ? "quick-" : ""}${stamp}`)));
const which = option("display", "builtin");
// How long a locked screen or a full-screen app on the display is waited for, before and during
// the parts.
const waitS = Number(option("wait-s", "900"));

const PARTS = [
  { name: "timeline", title: "Timeline (100 tracks × 40 clips)", suite: "timeline", runs, minutes: 30 },
  { name: "meters", title: "Meters (404 values a frame, with the fader and knob drags)", suite: "meters", runs, minutes: 20 },
  { name: "pianoroll", title: "Piano roll (2,000 and 10,000 notes)", suite: "pianoroll", runs, minutes: 25 },
  {
    name: "playback-night-drive",
    title: "Night Drive on the audio device while the timeline scrolls",
    suite: "playback",
    runs: 1,
    minutes: 8,
    project: path.join(root, "examples/output/electronic_124bpm/Night Drive.sonara"),
  },
  {
    name: "playback-ascension",
    title: "Ascension on the audio device while the timeline scrolls",
    suite: "playback",
    runs: 1,
    minutes: 8,
    project: path.join(root, "examples/output/cinematic_orchestral/Ascension.sonara"),
  },
];
const only = option("only", null)?.split(",");
const parts = only ? PARTS.filter((p) => only.includes(p.name)) : PARTS;
if (only && parts.length !== only.length) {
  throw new Error(`unknown part in --only ${only.join(",")}: the parts are ${PARTS.map((p) => p.name).join(", ")}`);
}

const sh = (cmd, cmdArgs) => (spawnSync(cmd, cmdArgs, { encoding: "utf8" }).stdout || "").trim();
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const rerunCommand = (names) =>
  `cd ${app} && pnpm run perf:signoff -- --skip-build${quick ? " --quick" : ""}${which !== "builtin" ? ` --display ${which}` : ""} --only ${names.join(",")}`;

// -- 0. where the window goes ------------------------------------------------------------------------

if (process.platform !== "darwin") throw new Error("the sign-off runs on the macOS host");
const display = { builtin: () => builtinDisplay(), main: () => displays()[0], external: () => displays().find((d) => !d.builtin) }[which]?.();
if (!display) {
  console.log(`No ${which} display (is the lid closed?). Open the lid, or pass --display main for a check only.`);
  process.exit(2);
}
const place = centredOn(display);
fs.mkdirSync(out, { recursive: true });
console.log(`Sonara's performance sign-off${quick ? ", quick check: the numbers do not count" : ""}`);
console.log(`  display: ${display.name}, ${display.scale}×, up to ${display.maxFps} Hz; the window at ${place.x},${place.y}`);
console.log(`  parts: ${parts.map((p) => p.name).join(", ")}; reports in ${out}`);
if (display.scale !== 2) console.log(`  note: the display is at ${display.scale}×; the sign-off measures at 2×`);
console.log("  Before you run it: the Mac idle and on mains power, the screen unlocked, no full-screen app on");
console.log("  that display, about 30 minutes (a few with --quick). Nothing is audible. Ctrl-C stops it.");

const caffeinate = spawn("caffeinate", ["-d", "-i", "-u"], { stdio: "ignore" });
let child = null;
function cleanUp() {
  caffeinate.kill();
  if (child) child.kill("SIGTERM");
  spawnSync("pkill", ["-f", path.join(root, "target/release/sonara-desktop")]);
}
for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
  process.on(signal, () => {
    cleanUp();
    console.log(`\nstopped. To go on: ${rerunCommand(parts.map((p) => p.name))}`);
    process.exit(130);
  });
}

// -- 1. the machine ------------------------------------------------------------------------------------

async function waitUntilDrawable(maxWaitMs) {
  const deadline = Date.now() + maxWaitMs;
  for (;;) {
    const locked = screenLocked();
    const full = fullScreenApps(display);
    if (!locked && full.length === 0) return true;
    if (Date.now() > deadline) return false;
    console.log(locked ? "the screen is locked: waiting for it to be unlocked" : `${full.join(", ")} is full screen on ${display.name}: waiting`);
    await sleep(10_000);
  }
}

if (!noWait) {
  const idle = idleSeconds();
  if (idle < 60) {
    console.log(`The Mac was used ${Math.round(idle)} s ago; starting in 30 s. Leave it alone from now on.`);
    await sleep(30_000);
  }
}
if (!(await waitUntilDrawable(waitS * 1000))) {
  console.log(`The screen stayed locked or a full-screen app stayed on ${display.name}. Rerun: ${rerunCommand(parts.map((p) => p.name))}`);
  cleanUp();
  process.exit(2);
}
const machine = {
  date: new Date().toISOString(),
  macOS: `${sh("sw_vers", ["-productVersion"])} (${sh("sw_vers", ["-buildVersion"])})`,
  cpu: sh("sysctl", ["-n", "machdep.cpu.brand_string"]),
  memoryGiB: Math.round(os.totalmem() / 2 ** 30),
  power: sh("pmset", ["-g", "batt"]).split("\n")[0]?.replace(/^Now drawing from /, "").replace(/'/g, ""),
  loadBefore: sh("sysctl", ["-n", "vm.loadavg"]),
  display: `${display.name}, ${display.scale}×, up to ${display.maxFps} Hz`,
};

// -- 2. the build -----------------------------------------------------------------------------------------

function build(cmd, cmdArgs, cwd) {
  console.log(`$ ${cmd} ${cmdArgs.join(" ")}`);
  const result = spawnSync(cmd, cmdArgs, { cwd, stdio: "inherit", timeout: 40 * 60_000 });
  if (result.status !== 0) {
    console.log(`the build failed (${result.status ?? result.signal}); nothing was measured`);
    cleanUp();
    process.exit(1);
  }
}
if (!skipBuild) {
  build("pnpm", ["tauri", "build", "--no-bundle", "--features", "bench"], app);
  build("cargo", ["build", "--locked", "-p", "sonara", "--release"], root);
}

// -- 3. the parts -----------------------------------------------------------------------------------------

/** Runs `part` through bench-timeline.mjs with a hard timeout; its summary.json, or why there is none. */
function runPart(part) {
  const dir = path.join(out, part.name);
  const partArgs = [
    path.join(here, "bench-timeline.mjs"),
    "--suite",
    part.suite,
    "--runs",
    String(part.runs),
    "--skip-build",
    "--at",
    `${place.x},${place.y}`,
    "--out",
    dir,
    "--unlock-wait-s",
    String(waitS),
    ...(quick ? ["--quick"] : []),
    ...(part.project ? ["--project", part.project] : []),
  ];
  console.log(`\n== ${part.title} (${part.runs} run${part.runs > 1 ? "s" : ""}) ==`);
  return new Promise((resolve) => {
    child = spawn(process.execPath, partArgs, { cwd: app, stdio: "inherit" });
    const timer = setTimeout(() => {
      console.log(`${part.name}: no result within ${part.minutes} minutes; stopped`);
      child.kill("SIGTERM");
      spawnSync("pkill", ["-f", path.join(root, "target/release/sonara-desktop")]);
    }, part.minutes * 60_000);
    child.on("exit", (code, signal) => {
      clearTimeout(timer);
      child = null;
      const file = path.join(dir, "summary.json");
      const summary = fs.existsSync(file) ? JSON.parse(fs.readFileSync(file, "utf8")) : null;
      resolve({ part, code: code ?? signal, summary });
    });
  });
}

const results = [];
for (const part of parts) {
  const result = await runPart(part);
  results.push(result);
  if (result.code === 2) {
    console.log("the screen stayed locked or the window stayed hidden: the remaining parts are skipped");
    break;
  }
}
cleanUp();

// -- 4. the summary -----------------------------------------------------------------------------------------

/** Whether `row` decides the sign-off (the bench marks the others "informational"). */
const gating = (row) => !String(row.note ?? "").includes("informational") && row.verdict !== "informational";

const lines = [
  `# Sonara GUI performance sign-off${quick ? " (quick check: the numbers do not count)" : ""}`,
  "",
  "The thresholds of ADR 0009 and docs/audio-engine.md. Measured locally, never a CI gate (HW-01).",
  "",
  `- Date: ${machine.date}`,
  `- Machine: ${machine.cpu}, ${machine.memoryGiB} GiB, macOS ${machine.macOS}`,
  `- Power: ${machine.power}; load average before: ${machine.loadBefore}`,
  `- Display: ${machine.display}; the window at ${place.x},${place.y} (1440 × 900)`,
  "- Build: release, `bench` feature",
  "",
];
const failed = [];
let allPass = true;
for (const { part, code, summary } of results) {
  lines.push(`## ${part.title}`, "");
  if (!summary) {
    lines.push(`No result (${code === 2 ? "the screen stayed locked or the window hidden" : `exit ${code}`}).`, "");
    failed.push(part.name);
    allPass = false;
    continue;
  }
  const env = summary.environment ?? {};
  lines.push(
    `Runs accepted: ${summary.runsAccepted} of ${part.runs}; attempts rejected: ${summary.rejected.length}` +
      (summary.rejected.length ? ` (${summary.rejected.map((r) => r.reasons.join("; ")).join(" | ")})` : "") +
      `. Device pixel ratio ${env.devicePixelRatio ?? "?"}, display frame ${env.frameMs ?? "?"} ms.`,
    "",
    "| Test | Metric | Median of runs | Worst run | Threshold | Verdict |",
    "|---|---|---|---|---|---|",
  );
  let partPass = summary.runsAccepted === part.runs;
  for (const row of summary.rows) {
    lines.push(`| ${row.test} | ${row.metric} | ${row.value} | ${row.worstRun ?? ""} | ${row.limit} | ${row.verdict}${row.note ? ` (${row.note})` : ""} |`);
    if (gating(row) && row.verdict !== "PASS") partPass = false;
  }
  lines.push("", `Reports: \`${path.join(out, part.name)}\``, "");
  if (!partPass) {
    failed.push(part.name);
    allPass = false;
  }
}
const skipped = parts.filter((p) => !results.some((r) => r.part === p)).map((p) => p.name);
failed.push(...skipped);
lines.splice(
  10,
  0,
  `**Verdict: ${allPass && skipped.length === 0 ? "PASS" : "NOT PASSED"}**${quick ? " (quick check)" : ""}` +
    (failed.length ? `: ${failed.join(", ")} did not pass or did not run.` : ": every threshold holds."),
  "",
);
if (failed.length) lines.push("## Rerun", "", "```text", rerunCommand(failed), "```", "");
lines.push(
  "## Where the numbers go",
  "",
  "Record the medians and the verdict in `docs/audio-engine.md`, section \"The GUI's performance sign-off\".",
  "",
);
const summaryFile = path.join(out, "summary.md");
fs.writeFileSync(summaryFile, lines.join("\n"));
console.log(`\n${lines.slice(0, 12).join("\n")}`);
console.log(`\nThe summary: ${summaryFile}`);
if (failed.length) console.log(`Rerun what did not pass: ${rerunCommand(failed)}`);
process.exit(failed.length ? 1 : 0);
