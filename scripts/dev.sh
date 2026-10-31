#!/usr/bin/env bash
# Runs a development task natively on the macOS host (the Docker development container is gone;
# owner decision, 2026-10-24). Needs the toolchain from CONTRIBUTING.md: rustup, cargo-deny, the
# Xcode command line tools, cmake, Node 26 and pnpm 12, with Playwright's WebKit installed
# (`pnpm --dir apps/sonara-desktop exec playwright install webkit`).
#
#   scripts/dev.sh rust            all Rust gates: rust-check, rust-deny, rust-loom, rust-rtsan
#   scripts/dev.sh frontend        the desktop frontend's gates
#   scripts/dev.sh help            all tasks
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

rust_check() {
    cargo fmt --all -- --check
    cargo clippy --locked --workspace --all-targets -- -D warnings
    cargo test --locked --workspace
    RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps
}

rust_deny() { cargo deny --locked check; }

rust_loom() { RUSTFLAGS="--cfg loom" cargo test --locked -p sonara-rt --release; }

rust_rtsan() {
    RTSAN_ENABLE=1 cargo test --locked -p sonara-rt -p sonara-io --features sonara-rt/rtsan,sonara-io/rtsan
    # The player's render step and the graph's real-time entry points (7 and 11):
    # `player` drives every kind of step, with media, MIDI and clicks, also on the null device,
    # whose callback is checked too (sonara-io/rtsan); `live` drives the graph's adoption, swaps
    # and installs directly; `sampler` drives a sampler's chase after seeks and loop wraps. The
    # engine's golden renders are offline, not real-time code.
    RTSAN_ENABLE=1 cargo test --locked -p sonara-engine --features sonara-engine/rtsan,sonara-io/rtsan --test player --test live --test sampler
    # The transport service: the render step and the device callback under the
    # service's calls, recordings and live MIDI, its RT-06 test included.
    RTSAN_ENABLE=1 cargo test --locked -p sonara-app --features sonara-engine/rtsan,sonara-io/rtsan --test transport --test transport_state
}

# The desktop frontend's gates, run in place against the host's checkout.
frontend() {
    cd "$root/apps/sonara-desktop"
    pnpm install --frozen-lockfile
    pnpm run check:licenses
    pnpm run check
    pnpm run build
    pnpm run test
}

usage() {
    cat <<'USAGE'
Usage: scripts/dev.sh <task> [args]

  rust-check      cargo fmt --check, clippy -D warnings, test, doc -D warnings
  rust-fmt        cargo fmt (rewrites files)
  rust-deny       cargo deny check (licences, advisories, bans, sources)
  rust-loom       loom model checks of sonara-rt
  rust-rtsan      RealtimeSanitizer tests of sonara-rt, sonara-io, the player and the transport
  rust            rust-check, rust-deny, rust-loom, rust-rtsan
  frontend        the desktop frontend: frozen install, licences, svelte-check, build, tests
USAGE
}

task="${1:-}"
[ $# -gt 0 ] && shift
case "$task" in
    rust-check) rust_check ;;
    rust-fmt) cargo fmt --all ;;
    rust-deny) rust_deny ;;
    rust-loom) rust_loom ;;
    rust-rtsan) rust_rtsan ;;
    rust) rust_check && rust_deny && rust_loom && rust_rtsan ;;
    frontend) frontend ;;
    *) usage; [ -z "$task" ] || [ "$task" = help ] || exit 2 ;;
esac
