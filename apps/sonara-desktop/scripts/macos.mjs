// What the benchmark harnesses ask macOS before and during a run (scripts/bench-timeline.mjs,
// scripts/perf-signoff.mjs): the displays, a full-screen app on one, a locked screen, the idle
// time, the front application, and bringing a process to the front. A web view that is not drawn
// gets no animation frames, so a run in a window that a locked screen, another Space or another
// app's full-screen Space hides measures nothing (stalled play).
//
// Everything is read through `osascript -l JavaScript` (AppKit and CoreGraphics) and `ioreg`; none
// of it needs the accessibility permission except `activate`, which only tries.

import { spawnSync } from "node:child_process";

function jxa(source) {
  const result = spawnSync("osascript", ["-l", "JavaScript", "-e", source], { encoding: "utf8", timeout: 20_000 });
  if (result.status !== 0) throw new Error(`osascript failed: ${(result.stderr || "").trim()}`);
  return result.stdout.trim();
}

/**
 * The displays, in global points from the main display's top left (the coordinates of Tauri's
 * window positions and of `window.json`): `{name, id, builtin, x, y, width, height, visibleTop,
 * visibleHeight, scale, maxFps}`.
 */
export function displays() {
  return JSON.parse(
    jxa(`
ObjC.import("AppKit");
ObjC.import("CoreGraphics");
const screens = $.NSScreen.screens;
const mainHeight = screens.objectAtIndex(0).frame.size.height;
const out = [];
for (let i = 0; i < screens.count; i++) {
  const s = screens.objectAtIndex(i);
  const id = ObjC.unwrap(s.deviceDescription.objectForKey("NSScreenNumber"));
  const f = s.frame;
  const v = s.visibleFrame;
  const builtin = $.CGDisplayIsBuiltin(id);
  out.push({
    name: ObjC.unwrap(s.localizedName), id, builtin: builtin === true || builtin === 1,
    x: f.origin.x, y: mainHeight - (f.origin.y + f.size.height), width: f.size.width, height: f.size.height,
    visibleTop: mainHeight - (v.origin.y + v.size.height), visibleHeight: v.size.height,
    scale: s.backingScaleFactor, maxFps: Number(s.maximumFramesPerSecond),
  });
}
JSON.stringify(out);`),
  );
}

/** The built-in display, or null (a closed lid, a desktop Mac). */
export function builtinDisplay() {
  return displays().find((d) => d.builtin) ?? null;
}

/** The point at which a `width` × `height` window is centred in `display`'s visible area. */
export function centredOn(display, width = 1440, height = 900) {
  const x = Math.round(display.x + Math.max(0, (display.width - width) / 2));
  const y = Math.round(display.visibleTop + Math.max(0, (display.visibleHeight - height) / 2));
  return { x, y };
}

/**
 * The applications whose window fills `display` exactly (a full-screen app: its Space hides every
 * other window on that display, a benchmark's included), among the windows on screen now.
 */
export function fullScreenApps(display) {
  const windows = JSON.parse(
    jxa(`
ObjC.import("CoreGraphics");
const list = ObjC.castRefToObject($.CGWindowListCopyWindowInfo($.kCGWindowListOptionOnScreenOnly | $.kCGWindowListExcludeDesktopElements, 0));
const out = [];
for (let i = 0; i < list.count; i++) {
  const w = list.objectAtIndex(i);
  if (ObjC.unwrap(w.objectForKey("kCGWindowLayer")) !== 0) continue;
  const b = w.objectForKey("kCGWindowBounds");
  out.push({ owner: ObjC.unwrap(w.objectForKey("kCGWindowOwnerName")), x: ObjC.unwrap(b.objectForKey("X")), y: ObjC.unwrap(b.objectForKey("Y")),
    width: ObjC.unwrap(b.objectForKey("Width")), height: ObjC.unwrap(b.objectForKey("Height")) });
}
JSON.stringify(out);`),
  );
  const fills = (w) =>
    Math.abs(w.x - display.x) < 1 && Math.abs(w.y - display.y) < 1 && Math.abs(w.width - display.width) < 1 && Math.abs(w.height - display.height) < 1;
  return [...new Set(windows.filter(fills).map((w) => w.owner))];
}

/** Whether the screen is locked (WebKit draws nothing then). */
export function screenLocked() {
  const result = spawnSync("ioreg", ["-n", "Root", "-d1"], { encoding: "utf8" });
  return /"CGSSessionScreenIsLocked"=Yes/.test(result.stdout || "");
}

/** Seconds since the last keyboard or mouse input. */
export function idleSeconds() {
  const result = spawnSync("ioreg", ["-c", "IOHIDSystem"], { encoding: "utf8" });
  const match = /"HIDIdleTime" = (\d+)/.exec(result.stdout || "");
  return match ? Number(match[1]) / 1e9 : 0;
}

/** The front application's name and process id. */
export function frontmost() {
  return JSON.parse(
    jxa(`
ObjC.import("AppKit");
const app = $.NSWorkspace.sharedWorkspace.frontmostApplication;
JSON.stringify({ name: ObjC.unwrap(app.localizedName), pid: app.processIdentifier });`),
  );
}

/** Brings the process `pid` to the front; false when macOS refused (no accessibility access). */
export function activate(pid) {
  const result = spawnSync(
    "osascript",
    ["-e", `tell application "System Events" to set frontmost of (first process whose unix id is ${pid}) to true`],
    { encoding: "utf8", timeout: 10_000 },
  );
  return result.status === 0;
}
