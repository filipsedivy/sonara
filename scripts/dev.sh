#!/usr/bin/env bash
# Runs a development task in the Sonara development container.
# Needs Docker. The image is rebuilt automatically when its Dockerfile or the toolchain pin change.
#
#   scripts/dev.sh rust            all Rust gates (without the desktop crate)
#   scripts/dev.sh frontend        the desktop frontend's gates
#   scripts/dev.sh help            all tasks
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
tty_flag=()
[ -t 0 ] && [ -t 1 ] || tty_flag=(-T)

compose=(docker compose -f "$root/docker/compose.yaml")
# CI builds the image itself (with its layer cache) and sets SONARA_DEV_NO_BUILD=1.
[ "${SONARA_DEV_NO_BUILD:-}" = 1 ] || "${compose[@]}" build --quiet dev
exec "${compose[@]}" run --rm ${tty_flag[@]+"${tty_flag[@]}"} dev docker/dev/tasks.sh "$@"
