#!/usr/bin/env bash
# Bumps the workspace version (VER-01), commits it, pushes to main, then tags the release and
# pushes the tag. The owner runs this by hand; it is never run on their behalf (see CLAUDE.md,
# "the owner versions the repository").
#
#   scripts/push-version.sh patch|minor|major|X.Y.Z
#
# Order: bump Cargo.toml -> refresh Cargo.lock -> commit -> push main -> tag -> push tag. Each
# step only runs once the previous one has landed, so a failure midway leaves nothing dangling
# except a commit already on main (the tag step can then be retried on its own).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

usage() {
    echo "usage: scripts/push-version.sh patch|minor|major|X.Y.Z" >&2
    exit 2
}

[ $# -eq 1 ] || usage
bump="$1"

current="$(awk '
    /^\[/ { in_table = ($0 == "[workspace.package]"); next }
    in_table && /^version[[:space:]]*=/ {
        value = $0
        sub(/^[^=]*=[[:space:]]*/, "", value)
        if (match(value, /^"[^"]*"/)) { print substr(value, 2, RLENGTH - 2); exit }
    }
' Cargo.toml)"
[ -n "$current" ] || { echo "error: no version in [workspace.package] of Cargo.toml" >&2; exit 1; }

old_major="${current%%.*}"
rest="${current#*.}"
old_minor="${rest%%.*}"
old_patch="${rest#*.}"

case "$bump" in
    patch) new="$old_major.$old_minor.$((old_patch + 1))" ;;
    minor) new="$old_major.$((old_minor + 1)).0" ;;
    major) new="$((old_major + 1)).0.0" ;;
    [0-9]*.[0-9]*.[0-9]*) new="$bump" ;;
    *) usage ;;
esac

case "$new" in
    [0-9]*.[0-9]*.[0-9]*) ;;
    *) echo "error: '$new' is not a semver X.Y.Z" >&2; exit 1 ;;
esac
[ "$new" != "$current" ] || { echo "error: $new is already the current version" >&2; exit 1; }

tag="v$new"

# Preconditions: on main, not behind/ahead of it, nothing staged or modified (an untracked file
# is fine), and the tag does not already exist locally or on the remote.
branch="$(git rev-parse --abbrev-ref HEAD)"
[ "$branch" = main ] || { echo "error: on branch '$branch', not main" >&2; exit 1; }

git diff --quiet || { echo "error: unstaged changes present; commit or stash them first" >&2; exit 1; }
git diff --cached --quiet || { echo "error: staged changes present; commit or stash them first" >&2; exit 1; }

git fetch origin main --quiet
local_head="$(git rev-parse HEAD)"
remote_head="$(git rev-parse origin/main)"
[ "$local_head" = "$remote_head" ] || {
    echo "error: HEAD ($local_head) differs from origin/main ($remote_head); pull or push first" >&2
    exit 1
}

if git rev-parse -q --verify "refs/tags/$tag" >/dev/null || git ls-remote --exit-code --tags origin "$tag" >/dev/null 2>&1; then
    echo "error: tag $tag already exists locally or on origin" >&2
    exit 1
fi

echo "Sonara $current -> $new"
read -r -p "Bump, commit, push to main, tag $tag and push the tag? [y/N] " reply
case "$reply" in
    y | Y) ;;
    *) echo "aborted" >&2; exit 1 ;;
esac

awk -v new="$new" '
    /^\[/ { in_table = ($0 == "[workspace.package]") }
    in_table && /^version[[:space:]]*=/ { print "version = \"" new "\""; next }
    { print }
' Cargo.toml > Cargo.toml.new
mv Cargo.toml.new Cargo.toml

export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
cargo update --workspace --locked 2>/dev/null || cargo update --workspace

changed="$(git diff --name-only; git status --porcelain --untracked-files=no | awk '{print $2}')"
unexpected="$(printf '%s\n' "$changed" | sort -u | grep -v -E '^(Cargo\.toml|Cargo\.lock)$' || true)"
if [ -n "$unexpected" ]; then
    echo "error: unexpected files changed, aborting:" >&2
    printf '%s\n' "$unexpected" >&2
    git checkout -- Cargo.toml Cargo.lock
    exit 1
fi

scripts/check-versions.sh --tag "$tag"

git add Cargo.toml Cargo.lock
git commit -m "Release $new"
git push origin main

git tag -a "$tag" -m "Sonara $new"
git push origin "$tag"

echo "Released $tag."
