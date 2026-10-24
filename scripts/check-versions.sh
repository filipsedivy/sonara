#!/bin/sh
# Fails unless every version of Sonara is the same (VER-01).
#
#   scripts/check-versions.sh [--tag vX.Y.Z]
#
# - The Rust workspace's version: `[workspace.package]` in Cargo.toml.
# - With --tag, the release tag without its leading "v".
# - The desktop's tauri.conf.json and package.json carry no version at all, so that the bundle
#   takes the workspace's.
#
# POSIX sh, awk and grep only, so it runs on any CI runner without a toolchain.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"

usage() {
    echo "usage: scripts/check-versions.sh [--tag vX.Y.Z]" >&2
    exit 2
}

tag=
while [ $# -gt 0 ]; do
    case "$1" in
        --tag) [ $# -ge 2 ] || usage; tag="$2"; shift 2 ;;
        --tag=*) tag="${1#--tag=}"; shift ;;
        -h | --help) usage ;;
        *) echo "error: unknown argument $1" >&2; usage ;;
    esac
done

# Prints the value of `version` in the TOML table `[$2]` of file $1. Fails when the table has
# no `version`, or when its value is not a one-line basic string ("0.1.0").
toml_version() {
    awk -v table="[$2]" -v file="$1" '
        # A table header: `[name]` or `[[name]]`, with optional white space and comment.
        /^[[:space:]]*\[/ {
            header = $0
            sub(/#.*$/, "", header)
            gsub(/[[:space:]]/, "", header)
            in_table = (header == table)
            next
        }
        in_table && /^[[:space:]]*version[[:space:]]*=/ {
            value = $0
            sub(/^[^=]*=[[:space:]]*/, "", value)
            if (match(value, /^"[^"]*"/)) {
                print substr(value, 2, RLENGTH - 2)
                found = 1
                exit
            }
            printf "error: %s: the version in %s is not a basic string\n", file, table > "/dev/stderr"
            failed = 1
            exit 2
        }
        END {
            if (!found && !failed) {
                printf "error: %s: no version in %s\n", file, table > "/dev/stderr"
                exit 2
            }
        }
    ' "$root/$1"
}

first=
differ=
# Prints one row of the table and remembers whether it differs from the first version.
row() {
    printf '%12s  %s\n' "$1" "$2"
    if [ -z "$first" ]; then
        first="$1"
    elif [ "$1" != "$first" ]; then
        differ=1
    fi
}

# Each version goes through a variable first: `set -e` stops on a failed command substitution
# only in an assignment, not in an argument.
version="$(toml_version Cargo.toml workspace.package)"
row "$version" Cargo.toml
[ -z "$tag" ] || row "${tag#v}" tag

status=0
for file in apps/sonara-desktop/src-tauri/tauri.conf.json apps/sonara-desktop/package.json; do
    if [ ! -f "$root/$file" ]; then
        echo "error: $file is missing" >&2
        status=1
    elif grep -Eq '"version"[[:space:]]*:' "$root/$file"; then
        echo "error: $file has a version; the workspace's Cargo.toml is the only one" >&2
        status=1
    fi
done
if [ -n "$differ" ]; then
    echo "error: versions differ" >&2
    status=1
fi
exit "$status"
