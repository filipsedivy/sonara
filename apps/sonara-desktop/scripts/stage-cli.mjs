// Stages the `sonara` CLI as the bundle's external binary.
//
// `tauri build --config src-tauri/tauri.bundle.conf.json` runs this after the frontend build
// (`beforeBuildCommand`). It builds the `sonara` package of the workspace with the bundle's
// profile and copies it to src-tauri/binaries/sonara-<target triple>, the name `externalBin`
// expects. The bundler then places it next to the app's executable as `sonara` in
// `Contents/MacOS` and signs it with the app.
//
// The external binary is configured only for bundling: tauri-build checks that it exists on
// every build, so listing it in tauri.conf.json would make every `cargo build` of the
// workspace depend on a staged CLI.
//
// Tauri sets TAURI_ENV_DEBUG ("true" for `tauri build --debug`) and TAURI_ENV_TARGET_TRIPLE.

import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const app = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const workspace = resolve(app, "../..");

const host = /^host: (.+)$/m.exec(execFileSync("rustc", ["-vV"], { encoding: "utf8" }))?.[1];
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || host;
if (!triple) throw new Error("cannot tell the target triple");
const debug = process.env.TAURI_ENV_DEBUG === "true";
const profile = debug ? "debug" : "release";

const metadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version", "1", "--no-deps", "--locked"], {
    cwd: workspace,
    encoding: "utf8",
  }),
);
const cross = triple !== host;
const args = ["build", "--locked", "-p", "sonara", ...(debug ? [] : ["--release"])];
if (cross) args.push("--target", triple);
execFileSync("cargo", args, { cwd: workspace, stdio: "inherit" });

const exe = triple.includes("windows") ? ".exe" : "";
const built = join(metadata.target_directory, ...(cross ? [triple] : []), profile, `sonara${exe}`);
const staged = join(app, "src-tauri", "binaries", `sonara-${triple}${exe}`);
mkdirSync(dirname(staged), { recursive: true });
copyFileSync(built, staged);
chmodSync(staged, 0o755);
console.log(`staged ${built} as ${staged}`);
