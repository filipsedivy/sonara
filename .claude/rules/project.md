# Project rules

These rules apply to every change in this repository: Rust, the TypeScript frontend, CI and
docs. They are MUST rules, like QG and RT (`rust/quality-gate.md`).

Sonara was ported from a Python prototype. The prototype and its tooling are gone. Its recorded
behaviour stays in `golden/` as regression data, and these rules keep that behaviour stable.

- **CTR-01 Contracts stay put.** These are external contracts and change only by a deliberate,
  versioned decision:
  - the `.sonara` format: `project-v1.schema.json` and `manifest-v1.schema.json`;
  - the 57 agent tool schemas, their semantics and their error codes (`docs/agent-api.md`);
  - the IPC protocol;
  - the `sonara` command tree and its JSON output (`golden/contracts/cli.json`).
- **GLD-01 Golden data is the regression oracle.**
  - `golden/` holds the behaviour recorded from the prototype. The Rust tests in
    `sonara-testkit` and the crates compare against it.
  - Never re-record it, and never loosen a tolerance to make a test pass.
  - A new or changed tolerance needs a measured value and a written justification next to it,
    and in the change description.
  - A deliberate behaviour change that the golden data does not allow (for example, a fixed
    defect) is written next to the test that pins the new behaviour.
- **LIC-01 Licences.**
  - No GPL or AGPL anywhere.
  - No LGPL Rust crates, because they are linked statically.
  - LGPL C libraries only as separate, replaceable dynamic libraries. The app bundle ships their
    complete corresponding source (`Contents/Resources/licenses/sources/`).
  - MPL-2.0 crates and npm packages only unmodified, each listed as an exception in `deny.toml`
    or the frontend licence gate.
  - A licence outside `deny.toml`'s allow-list needs the owner's decision first.
  - Every dependency, including build-time and frontend ones, is recorded in
    `docs/dependencies.md`.
  - `LICENSE` stays "all rights reserved".
- **HW-01 Hardware numbers stay local.** Underrun, render-load and GUI performance figures are
  measured locally and recorded in `docs/audio-engine.md`. They are never CI gates.
- **VER-01 One version.** The workspace `Cargo.toml` holds the only version. The desktop's
  `tauri.conf.json` and `package.json` carry none, and release tags equal it.
  `scripts/check-versions.sh` checks this.
- **DOC-01 Docs move with the code.** Every user-visible change updates `CHANGELOG.md` and
  `docs/implementation-status.md`. Dependency changes update `docs/dependencies.md`.
