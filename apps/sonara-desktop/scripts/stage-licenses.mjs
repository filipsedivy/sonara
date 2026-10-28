// Stages the bundle's licences folder (LIC-01, QG-10).
//
// Writes target/licenses/ at the repository root, which tauri.bundle.conf.json maps to
// Sonara.app/Contents/Resources/licenses/:
//
//   THIRD-PARTY-LICENSES.txt     every text file below, joined; About Sonara shows it
//   NOTICE-LGPL.txt              libsoxr and LAME: separate, replaceable libraries; their source in sources/
//   NOTICE-MPL.txt               the MPL-2.0 components, unmodified, and where their source is
//   rust-crates.txt              cargo-about: the crates of sonara-desktop and the sonara CLI
//   frontend.txt                 the frontend's production npm packages (scripts/check-licenses.mjs)
//   miniaudio-LICENSE.txt        third_party/miniaudio
//   libsoxr-*, lame-*, libFLAC-* the codec libraries' texts (scripts/native/build-codecs.sh)
//   sources/                     the complete corresponding source of libsoxr and LAME (LGPL 2.1
//                                section 6, owner decision of 2026-10-23): their source archives, checked
//                                against the SHA-256 pins of build-codecs.sh, LAME's export-list
//                                patch, and build-codecs.sh itself
//
// Needs the codec libraries, their licences and source archives (run
// scripts/native/build-codecs.sh first), the installed frontend (pnpm install) and the crates in
// Cargo's cache (cargo fetch). cargo-about, a build-time tool, is installed into target/tools on
// first use (docs/dependencies.md).
//
// Usage (from apps/sonara-desktop): node scripts/stage-licenses.mjs

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const CARGO_ABOUT_VERSION = "0.9.2";
const TARGET = "aarch64-apple-darwin";
// The executables of the bundle: the app and the CLI beside it.
const PACKAGES = [
  { name: "sonara-desktop", manifest: "apps/sonara-desktop/src-tauri/Cargo.toml" },
  { name: "sonara", manifest: "apps/sonara/Cargo.toml" },
];

const app = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const root = resolve(app, "../..");
const out = join(root, "target", "licenses");
const tools = join(root, "target", "tools");
const native = join(root, "target", "native", "licenses");
const nativeScripts = join(root, "scripts", "native");
const rule = "=".repeat(78);
const thin = "-".repeat(78);

function run(command, args, options = {}) {
  return execFileSync(command, args, { cwd: root, encoding: "utf8", maxBuffer: 1 << 30, ...options });
}

// --- cargo-about ---------------------------------------------------------------------------

const cargoAbout = join(tools, "bin", "cargo-about");
function installedVersion() {
  if (!existsSync(cargoAbout)) return null;
  return /cargo-about (\S+)/.exec(run(cargoAbout, ["--version"]))?.[1] ?? null;
}
if (installedVersion() !== CARGO_ABOUT_VERSION) {
  console.log(`stage-licenses: installing cargo-about ${CARGO_ABOUT_VERSION} into ${tools}`);
  run("cargo", ["install", "--locked", "--root", tools, "--features", "cli", `cargo-about@${CARGO_ABOUT_VERSION}`], {
    stdio: "inherit",
  });
}

// Every crate of Cargo.lock in the local registry, for all targets: cargo-about and cargo tree
// read the whole lock file's manifests, and then run offline.
run("cargo", ["fetch", "--locked"], { stdio: "inherit" });
const config = join(app, "scripts", "licenses", "about.toml");
const reports = PACKAGES.map((pkg) =>
  JSON.parse(
    run(cargoAbout, [
      "about",
      "generate",
      "--format",
      "json",
      "--frozen",
      "--fail",
      "--config",
      config,
      "--manifest-path",
      join(root, pkg.manifest),
    ]),
  ),
);

// One entry per licence text, with every crate that uses it in either executable.
const texts = new Map();
for (const report of reports) {
  for (const licence of report.licenses) {
    const key = `${licence.id}\0${licence.text}`;
    const entry = texts.get(key) ?? { id: licence.id, name: licence.name, text: licence.text, users: new Set() };
    for (const user of licence.used_by) entry.users.add(`${user.crate.name} ${user.crate.version}`);
    texts.set(key, entry);
  }
}
const crates = new Map();
for (const report of reports) {
  for (const { package: pkg, license } of report.crates) {
    crates.set(`${pkg.name} ${pkg.version}`, { name: pkg.name, version: pkg.version, license, repository: pkg.repository });
  }
}

// The crates linked into the executables: normal dependencies without procedural macros, whose
// code runs only in the compiler.
const linked = new Set(
  run("cargo", [
    "tree",
    "--locked",
    "--offline",
    "--target",
    TARGET,
    "--edges",
    "normal,no-proc-macro",
    "--prefix",
    "none",
    "--format",
    "{p}",
    ...PACKAGES.flatMap((pkg) => ["--package", pkg.name]),
  ])
    .split("\n")
    .map((line) => /^(\S+) v(\S+)/.exec(line))
    .filter(Boolean)
    .map((match) => `${match[1]} ${match[2]}`),
);

const byName = (a, b) => a.localeCompare(b, "en");
const sortedTexts = [...texts.values()].sort((a, b) => byName(a.name, b.name) || b.users.size - a.users.size);
const overview = new Map();
for (const entry of sortedTexts) {
  for (const user of entry.users) {
    const users = overview.get(entry.id) ?? new Set();
    users.add(user);
    overview.set(entry.id, users);
  }
}
const toolchain = /channel\s*=\s*"([^"]+)"/.exec(readFileSync(join(root, "rust-toolchain.toml"), "utf8"))?.[1];
const rustCrates = [
  "Sonara's Rust crates: the third-party crates compiled into sonara-desktop and the sonara CLI",
  `for ${TARGET}, as cargo-about ${CARGO_ABOUT_VERSION} reports them, grouped by licence text. Where a`,
  "crate offers a choice of licences, the first one Sonara accepts is reproduced.",
  "",
  `The Rust standard library (Rust ${toolchain}) is linked into both executables under MIT OR`,
  "Apache-2.0; its licence texts are those of the Rust project (https://www.rust-lang.org/policies/licenses).",
  "",
  "Crates per licence:",
  ...[...overview.entries()].sort(([a], [b]) => byName(a, b)).map(([id, users]) => `  ${id}: ${users.size}`),
  "",
  ...sortedTexts.map((entry) =>
    [
      rule,
      `${entry.name} (${entry.id})`,
      `Used by: ${[...entry.users].sort(byName).join(", ")}`,
      thin,
      "",
      entry.text.trim(),
      "",
    ].join("\n"),
  ),
].join("\n");

// --- The frontend ----------------------------------------------------------------------------

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
writeFileSync(join(out, "rust-crates.txt"), `${rustCrates}\n`);
run("node", [join(app, "scripts", "check-licenses.mjs"), "--notices", join(out, "frontend.txt")], {
  cwd: app,
  stdio: "inherit",
});

// --- The codec libraries and miniaudio ------------------------------------------------------

const CODEC_FILES = [
  "libsoxr-LICENCE.txt",
  "libsoxr-COPYING.LGPL.txt",
  "libsoxr-pffft-LICENSE.txt",
  "lame-COPYING.txt",
  "lame-LICENSE.txt",
  "libFLAC-COPYING.Xiph.txt",
];
for (const file of CODEC_FILES) {
  const source = join(native, file);
  if (!existsSync(source)) throw new Error(`${source} is missing: run scripts/native/build-codecs.sh first`);
  copyFileSync(source, join(out, file));
}
copyFileSync(join(root, "third_party", "miniaudio", "LICENSE"), join(out, "miniaudio-LICENSE.txt"));

// The codec pins: build-codecs.sh is their one source (DES-01).
const codecs = readFileSync(join(root, "scripts", "native", "build-codecs.sh"), "utf8");
function pin(name) {
  const value = new RegExp(`^${name}=(.+)$`, "m").exec(codecs)?.[1];
  if (!value) throw new Error(`build-codecs.sh defines no ${name}`);
  return value.replace(/^"|"$/g, "").replace(/\$\{(\w+)\}/g, (_, inner) => pin(inner));
}

// The source code of the LGPL libraries. The archives are the ones
// build-codecs.sh downloaded and built from; the stamp proves that this build-codecs.sh and patch
// built the libraries in target/native/lib.
const sha256 = (data) => createHash("sha256").update(data).digest("hex");
const sources = join(out, "sources");
mkdirSync(sources, { recursive: true });
const codecsScript = readFileSync(join(nativeScripts, "build-codecs.sh"));
const lamePatch = readFileSync(join(nativeScripts, pin("LAME_PATCH")));
const stamp = join(root, "target", "native", ".stamp");
if (!existsSync(stamp) || readFileSync(stamp, "utf8").trim() !== sha256(Buffer.concat([codecsScript, lamePatch]))) {
  throw new Error("target/native was not built by this build-codecs.sh: run scripts/native/build-codecs.sh first");
}
for (const name of ["SOXR", "LAME"]) {
  const archive = pin(`${name}_ARCHIVE`);
  const source = join(root, "target", "native", "downloads", archive);
  if (!existsSync(source)) throw new Error(`${source} is missing: run scripts/native/build-codecs.sh first`);
  const actual = sha256(readFileSync(source));
  if (actual !== pin(`${name}_SHA256`)) {
    throw new Error(`${source}: SHA-256 is ${actual}, build-codecs.sh pins ${pin(`${name}_SHA256`)}`);
  }
  copyFileSync(source, join(sources, archive));
}
writeFileSync(join(sources, pin("LAME_PATCH")), lamePatch);
writeFileSync(join(sources, "build-codecs.sh"), codecsScript);

const lgplTitle = "Sonara: the LGPL libraries and their source code";
const lgpl = `${lgplTitle}
${"=".repeat(lgplTitle.length)}

Sonara uses two libraries under the GNU Lesser General Public License. Neither is linked into
Sonara's executables. Each is a separate dynamic library in Sonara.app/Contents/Frameworks that
Sonara loads at run time, and each can be replaced by the user (see "Replacing a library").
Their complete source code ships with Sonara (see "The source code").

libsoxr, the SoX Resampler library: Contents/Frameworks/libsoxr.0.dylib
  Licence:  GNU Lesser General Public License 2.1 or later. Notice: libsoxr-LICENCE.txt;
            licence text: libsoxr-COPYING.LGPL.txt; the licence of its PFFFT code:
            libsoxr-pffft-LICENSE.txt.
  Version:  commit ${pin("SOXR_COMMIT")} of https://github.com/dofuuz/soxr
            (0.1.3-14-ga66f3ee), unmodified.
  Source:   sources/${pin("SOXR_ARCHIVE")}
            SHA-256 ${pin("SOXR_SHA256")}
            GitHub's archive of that commit, from
            ${pin("SOXR_URL")}

LAME, the MP3 encoder library: Contents/Frameworks/libmp3lame.0.dylib
  Licence:  GNU Library General Public License 2 or later. Licence text: lame-COPYING.txt;
            LAME's note: lame-LICENSE.txt. Home page: https://lame.sourceforge.io
  Version:  ${pin("LAME_VERSION")}
  Source:   sources/${pin("LAME_ARCHIVE")}
            SHA-256 ${pin("LAME_SHA256")}
            The official release, from
            ${pin("LAME_URL")}
  Change:   sources/${pin("LAME_PATCH")} removes one line from
            include/libmp3lame.sym, the library's export list: the symbol lame_init_old, which
            LAME ${pin("LAME_VERSION")} exports but no longer defines, and which the macOS linker rejects
            for a shared library. No source code is changed.

The source code
  Sonara.app/Contents/Resources/licenses/sources holds the complete corresponding source code
  of both libraries as shipped: the two source archives above, the patch to LAME's export list,
  and build-codecs.sh, the script of Sonara's source tree that built both libraries with the
  configuration and compiler flags it records. In the Finder, Control-click Sonara.app and
  choose Show Package Contents to reach the folder.

  To build both libraries again as Sonara did, on a Mac with Apple Silicon, cmake and the Xcode
  command line tools: in an empty folder, put build-codecs.sh and the patch into
  scripts/native/ and the two source archives into target/native/downloads/, then run
  bash scripts/native/build-codecs.sh. It checks the archives' SHA-256, applies the patch and
  writes the libraries to target/native/lib/. It also downloads and builds libFLAC (see below).

Replacing a library
  1. Build a compatible version of the library (the API of soxr.h 0.1.3 or lame.h 3.100) as an
     arm64 dynamic library with the install name @rpath/libsoxr.0.dylib or
     @rpath/libmp3lame.0.dylib.
  2. The library must be signed, because a Mac with Apple Silicon loads no unsigned code. The
     linker signs it ad hoc; if you change the file afterwards, for example with
     install_name_tool, sign it again: codesign --force --sign - libmp3lame.0.dylib
  3. Put it in place of Sonara.app/Contents/Frameworks/libsoxr.0.dylib or libmp3lame.0.dylib.
  Sonara then loads your build. Sonara.app does not have to be signed again: it is signed
  without library validation, so it loads a library signed ad hoc or by anyone else.
  You may modify Sonara for your own use with a modified library, and reverse engineer it to
  debug such modifications, as these licences require.

libFLAC ${pin("FLAC_VERSION")} (Contents/Frameworks/libFLAC.12.dylib) is not under the LGPL: its licence is
Xiph.Org's BSD licence in libFLAC-COPYING.Xiph.txt. build-codecs.sh builds it from
${pin("FLAC_URL")}.
`;
writeFileSync(join(out, "NOTICE-LGPL.txt"), lgpl);

// --- MPL-2.0 ---------------------------------------------------------------------------------

const mpl = [...crates.values()]
  .filter((crate) => /MPL-2\.0/.test(crate.license))
  .sort((a, b) => byName(a.name, b.name));
const mplLine = (crate) =>
  `  ${crate.name} ${crate.version}: https://crates.io/crates/${crate.name}/${crate.version}` +
  (crate.repository ? `\n      ${crate.repository}` : "");
const npmMpl = Object.entries(JSON.parse(run("pnpm", ["licenses", "list", "--json"], { cwd: app })))
  .filter(([licence]) => /MPL-2\.0/.test(licence))
  .flatMap(([, packages]) => packages)
  .flatMap((pkg) => pkg.versions.map((version) => `  ${pkg.name} ${version} (npm): https://www.npmjs.com/package/${pkg.name}/v/${version}`))
  .sort(byName);
const mplNotice = [
  "Sonara: components under the Mozilla Public License 2.0",
  "=".repeat(55),
  "",
  "Sonara uses these components unmodified. Their source code is available from the registries",
  "and repositories below, at the versions given. The MPL-2.0 text is in rust-crates.txt.",
  "",
  "Compiled into Sonara's executables:",
  ...mpl.filter((crate) => linked.has(`${crate.name} ${crate.version}`)).map(mplLine),
  "",
  "Used only while Sonara is built (procedural macros and the frontend's CSS build); none of",
  "their code is part of the application:",
  ...mpl.filter((crate) => !linked.has(`${crate.name} ${crate.version}`)).map(mplLine),
  ...npmMpl,
  "",
].join("\n");
writeFileSync(join(out, "NOTICE-MPL.txt"), mplNotice);

// --- The joined file -------------------------------------------------------------------------

const version = /^\[workspace\.package\][^[]*?^version\s*=\s*"([^"]+)"/ms.exec(
  readFileSync(join(root, "Cargo.toml"), "utf8"),
)?.[1];
const PARTS = [
  ["NOTICE-LGPL.txt", "libsoxr and LAME: separate, replaceable libraries; their source code"],
  ["NOTICE-MPL.txt", "the MPL-2.0 components and where their source code is"],
  ["libsoxr-LICENCE.txt", "libsoxr's licence notice"],
  ["libsoxr-COPYING.LGPL.txt", "the GNU Lesser General Public License 2.1 (libsoxr)"],
  ["libsoxr-pffft-LICENSE.txt", "the licence of libsoxr's PFFFT code"],
  ["lame-COPYING.txt", "the GNU Library General Public License 2 (LAME)"],
  ["lame-LICENSE.txt", "LAME's note on its licence"],
  ["libFLAC-COPYING.Xiph.txt", "libFLAC's licence (BSD-3-Clause)"],
  ["miniaudio-LICENSE.txt", "miniaudio's licence (public domain or MIT-0)"],
  ["rust-crates.txt", "the Rust crates of both executables"],
  ["frontend.txt", "the frontend's npm packages"],
];
const joined = [
  `Sonara ${version}: third-party licences`,
  "=".repeat(`Sonara ${version}: third-party licences`.length),
  "",
  "Sonara is © 2026 Filip Sedivy, free software under the 0BSD licence. It contains the third-party software",
  "listed below, under the licences given. This text joins the files of",
  "Sonara.app/Contents/Resources/licenses:",
  "",
  ...PARTS.map(([file, what]) => `  ${file.padEnd(28)} ${what}`),
  "",
  "The folder sources/ beside them holds the source code of libsoxr and LAME (see NOTICE-LGPL.txt).",
  "",
  ...PARTS.map(([file]) => [rule, rule, file, rule, rule, "", readFileSync(join(out, file), "utf8").trimEnd(), ""].join("\n")),
].join("\n");
writeFileSync(join(out, "THIRD-PARTY-LICENSES.txt"), `${joined}\n`);

const total = [...overview.values()].reduce((set, users) => new Set([...set, ...users]), new Set()).size;
console.log(
  `stage-licenses: ${out}: ${total} crates in ${texts.size} licence texts, ` +
    `${mpl.length} MPL-2.0 crates (${mpl.filter((c) => linked.has(`${c.name} ${c.version}`)).length} compiled in)`,
);
