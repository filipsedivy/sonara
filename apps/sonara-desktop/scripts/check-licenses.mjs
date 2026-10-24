// The frontend licence gate (LIC-01, QG-10): the counterpart of
// `cargo deny check licenses` for the npm packages.
//
// Runs `pnpm licenses list --json` for every installed package and for the production set (the
// packages whose code can ship in the bundle), and fails on any licence outside the allow-list.
// The allow-list is deny.toml's. MPL-2.0 is allowed only for the packages in REVIEWED, which the
// owner accepted unmodified (2026-10-22), and only at build time: they must stay out
// of the production set. GPL, AGPL and LGPL always fail: a JavaScript bundle links statically.
//
// pnpm lists the packages installed for this platform, so the gate checks the native packages
// of the machine it runs on (macOS on the host, Linux in the development container and CI).
//
// With `--notices FILE`, a passing check also writes the licence texts of the production set to
// FILE, for the app bundle's licences folder (scripts/stage-licenses.mjs).
//
// Usage (from apps/sonara-desktop): pnpm run check:licenses [-- --notices FILE]

import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

// deny.toml's `[licenses] allow`, verbatim.
const ALLOWED = new Set([
  "0BSD",
  "Apache-2.0",
  "Apache-2.0 WITH LLVM-exception",
  "BSD-1-Clause",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "BSL-1.0",
  "CC0-1.0",
  "ISC",
  "MIT",
  "MIT-0",
  "Unicode-3.0",
  "Unlicense",
  "Zlib",
]);

// Accepted by the owner (2026-10-22), build time only: Vite 8 depends on lightningcss
// (MPL-2.0), and pnpm installs its native package for the platform (lightningcss-<platform>).
const REVIEWED = [{ name: /^lightningcss(-[a-z0-9-]+)?$/, licence: "MPL-2.0" }];

const FORBIDDEN = /\b(A?GPL|LGPL)/i;

function list(prod) {
  const args = ["licenses", "list", "--json", ...(prod ? ["--prod"] : [])];
  const out = execFileSync("pnpm", args, { encoding: "utf8" });
  const byLicence = JSON.parse(out);
  return Object.entries(byLicence).flatMap(([licence, packages]) =>
    packages.map((pkg) => ({
      name: pkg.name,
      versions: pkg.versions,
      paths: pkg.paths ?? [],
      author: pkg.author,
      homepage: pkg.homepage,
      licence,
    })),
  );
}

// Accepts "A", "(A OR B)", "A OR B", "A AND B"; OR needs one allowed term, AND needs all.
// A term with "WITH" (an exception) is matched as a whole.
function allowed(expression) {
  const clean = expression.replace(/[()]/g, " ").replace(/\s+/g, " ").trim();
  if (ALLOWED.has(clean)) return true;
  if (/\sAND\s/.test(clean)) return clean.split(/\s+AND\s+/).every((t) => ALLOWED.has(t.trim()));
  return clean.split(/\s+OR\s+/).some((t) => ALLOWED.has(t.trim()));
}

function reviewed(pkg) {
  return REVIEWED.some((entry) => entry.name.test(pkg.name) && entry.licence === pkg.licence);
}

const noticesAt = process.argv.indexOf("--notices");
const noticesFile = noticesAt === -1 ? null : process.argv[noticesAt + 1];
if (noticesAt !== -1 && !noticesFile) throw new Error("--notices needs a file");

const all = list(false);
const prodPackages = list(true);
const prod = new Set(prodPackages.map((p) => p.name));
const failures = [];
const notes = [];

for (const pkg of all) {
  const id = `${pkg.name}@${pkg.versions.join(",")} (${pkg.licence})`;
  if (FORBIDDEN.test(pkg.licence)) {
    failures.push(`forbidden licence: ${id}`);
  } else if (reviewed(pkg)) {
    if (prod.has(pkg.name)) failures.push(`an MPL-2.0 build-time package ships in the bundle: ${id}`);
    else notes.push(`${id}, build time only`);
  } else if (!allowed(pkg.licence)) {
    failures.push(`licence not on the allow-list: ${id}`);
  }
}

const counts = {};
for (const pkg of all) counts[pkg.licence] = (counts[pkg.licence] ?? 0) + 1;
console.log(`packages: ${all.length} in total, ${prod.size} in the production set`);
for (const [licence, n] of Object.entries(counts).sort()) console.log(`  ${licence}: ${n}`);
console.log(`production set: ${[...prod].sort().join(", ")}`);
for (const line of notes) console.log(`accepted MPL-2.0 exception: ${line}`);
if (failures.length > 0) {
  for (const line of failures) console.error(`FAIL ${line}`);
  process.exit(1);
}
console.log("licence check: OK");

// The licence files a package ships (LICENSE, LICENCE, COPYING, NOTICE and their variants; not
// LICENSE.spdx, which only names the licence).
function licenceFiles(dir) {
  return readdirSync(dir)
    .filter((file) => /^(licen[cs]e|copying|notice)([.-].*)?$/i.test(file) && !/\.spdx$/i.test(file))
    .sort()
    .map((file) => join(dir, file));
}

// The MIT licence text, for a package whose package.json declares MIT but which ships no file.
const MIT_TEXT = `Permission is hereby granted, free of charge, to any person obtaining a copy of this
software and associated documentation files (the "Software"), to deal in the Software without
restriction, including without limitation the rights to use, copy, modify, merge, publish,
distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or
substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
`;

// The production set's licence texts: one section per package and version, sorted by name.
function notices(packages) {
  const rule = "=".repeat(78);
  const entries = packages
    .flatMap((pkg) => pkg.versions.map((version, i) => ({ ...pkg, version, path: pkg.paths[i] })))
    .sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));
  const sections = entries.map((pkg) => {
    const head = [`${pkg.name} ${pkg.version} (${pkg.licence})`];
    if (pkg.homepage) head.push(pkg.homepage);
    const files = pkg.path ? licenceFiles(pkg.path) : [];
    let body;
    if (files.length > 0) {
      body = files.map((file) => readFileSync(file, "utf8").trimEnd()).join("\n\n");
    } else if (pkg.licence === "MIT") {
      const author = pkg.author ? `Copyright (c) ${pkg.author}\n\n` : "";
      body = `The package ships no licence file; its package.json declares MIT.\n\n${author}${MIT_TEXT}`;
    } else {
      throw new Error(`${pkg.name}@${pkg.version} (${pkg.licence}) ships no licence file`);
    }
    return `${rule}\n${head.join("\n")}\n${rule}\n\n${body}\n`;
  });
  return (
    "The desktop frontend: the npm packages of its production set (`pnpm licenses list --prod`),\n" +
    "whose code can be part of the bundled web assets, with their licence texts.\n\n" +
    sections.join("\n")
  );
}

if (noticesFile) {
  writeFileSync(noticesFile, notices(prodPackages));
  console.log(`licence texts of the production set: ${noticesFile}`);
}
