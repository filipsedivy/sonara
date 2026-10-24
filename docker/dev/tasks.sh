#!/usr/bin/env bash
# Quality gates inside the development container.
# Run them through scripts/dev.sh on the host; CI's Linux job calls the same tasks.
set -euo pipefail

cd /work

# The desktop crate needs WebKitGTK to compile on Linux, so the container's
# gates leave it out. The macOS CI job and the host gates build it, and `cargo deny` still checks
# its whole dependency tree.
exclude_desktop=(--exclude sonara-desktop)

rust_check() {
    cargo fmt --all -- --check
    cargo clippy --locked --workspace "${exclude_desktop[@]}" --all-targets -- -D warnings
    cargo test --locked --workspace "${exclude_desktop[@]}"
    RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace "${exclude_desktop[@]}" --no-deps
}

rust_deny() { cargo deny --locked check; }

rust_loom() { RUSTFLAGS="--cfg loom" cargo test --locked -p sonara-rt --release; }

rust_rtsan() {
    RTSAN_ENABLE=1 cargo test --locked -p sonara-rt -p sonara-io --features sonara-rt/rtsan,sonara-io/rtsan
    # The player's render step and the graph's real-time entry points (7 and 11):
    # `player` drives every kind of step, with media, MIDI and clicks, also on the null device,
    # whose callback is checked too (sonara-io/rtsan); `live` drives the graph's adoption, swaps
    # and installs directly. The engine's golden renders are offline, not real-time code.
    RTSAN_ENABLE=1 cargo test --locked -p sonara-engine --features sonara-engine/rtsan,sonara-io/rtsan --test player --test live
    # The transport service: the render step and the device callback under the
    # service's calls, recordings and live MIDI, its RT-06 test included.
    RTSAN_ENABLE=1 cargo test --locked -p sonara-app --features sonara-engine/rtsan,sonara-io/rtsan --test transport --test transport_state
}

# The desktop frontend's gates: the frozen install, the licence gate,
# svelte-check, the Vite build and the Playwright tests in WebKit. They run on a copy of
# apps/sonara-desktop in /cache, so that the Linux node_modules and dist/ never land in the
# source tree, where the host keeps its own.
frontend() {
    local source=/work/apps/sonara-desktop work=/cache/frontend
    mkdir -p "$work"
    find "$work" -mindepth 1 -maxdepth 1 ! -name node_modules -exec rm -rf {} +
    tar -C "$source" --exclude=./node_modules --exclude=./dist --exclude=./src-tauri \
        --exclude=./test-results --exclude=./playwright-report -cf - . | tar -C "$work" -xf -
    cd "$work"
    CI=true pnpm install --frozen-lockfile
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
                  (rust-check leaves out the desktop crate)
  frontend        the desktop frontend: frozen install, licences, svelte-check, build, tests
  shell           interactive shell
  run <cmd...>    run a command in the container
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
    shell) exec bash ;;
    run) exec "$@" ;;
    *) usage; [ -z "$task" ] || [ "$task" = help ] || exit 2 ;;
esac
