#!/usr/bin/env bash
# Builds Sonara.app and its DMG with one command (LIC-01):
#
#   1. the codec libraries: scripts/native/build-codecs.sh (skipped when up to date);
#   2. the frontend's dependencies: pnpm install --frozen-lockfile;
#   3. the licences folder target/licenses: apps/sonara-desktop/scripts/stage-licenses.mjs, with
#      cargo-about (installed into target/tools on first use), including sources/, the source
#      code of libsoxr and LAME;
#   4. the Tauri bundle, `tauri build --config src-tauri/tauri.bundle.conf.json`:
#      - its beforeBuildCommand builds the frontend and stages the sonara CLI (stage-cli.mjs);
#      - the release build of sonara-desktop;
#      - Contents/MacOS/sonara (externalBin), Contents/Frameworks/*.dylib
#        (bundle.macOS.frameworks) and Contents/Resources/licenses (bundle.resources);
#      - signing, with the hardened runtime and Entitlements.plist, and the DMG;
#   5. the checks: codesign --verify --deep --strict on the app and on the app inside the DMG,
#      the ad-hoc signature, the layout, the @rpath install names, the LGPL source archives
#      against the SHA-256 pins of build-codecs.sh, the minos of every Mach-O (at most 26.0),
# and the sizes against the budget of
#
# Signing is ad hoc (`signingIdentity "-"` in tauri.conf.json), and nothing is notarized: the
# owner's decision of 2026-10-23, for local builds and CI alike. The script clears the Tauri
# bundler's signing and notarization variables, so an identity in the environment cannot change
# that, and fails a bundle that is not signed ad hoc.
#
# Needs the Xcode command line tools, cmake, Node and pnpm (the versions of
# apps/sonara-desktop/package.json), and the Rust toolchain of rust-toolchain.toml. Runs on Apple
# Silicon macOS only.
#
#   scripts/bundle-macos.sh
set -euo pipefail

# The minimum macOS (tauri.conf.json's minimumSystemVersion): no Mach-O may need a newer one.
MINIMUM_MACOS=26.0
# The size budget, in MiB: the DMG and the installed app. Measured on 0.1.0
# (2026-10-23): app 37 MiB, DMG 14.4 MiB; the budget leaves about a third for growth.
DMG_BUDGET_MIB=20
APP_BUDGET_MIB=50
CODECS=(libsoxr.0.dylib libmp3lame.0.dylib libFLAC.12.dylib)

root="$(cd "$(dirname "$0")/.." && pwd)"
desktop="$root/apps/sonara-desktop"
codecs_script="$root/scripts/native/build-codecs.sh"

die() {
    echo "bundle-macos: $*" >&2
    exit 1
}

# A pin of build-codecs.sh, the one source of the codec pins (DES-01), with its ${NAME}
# references expanded. The file is read, never sourced.
codec_pin() {
    local value ref
    value="$(sed -n "s/^$1=//p" "$codecs_script" | tr -d '"')"
    [ -n "$value" ] || die "build-codecs.sh defines no $1"
    while [[ $value =~ \$\{([A-Z0-9_]+)\} ]]; do
        ref="${BASH_REMATCH[1]}"
        value="${value/\$\{$ref\}/$(codec_pin "$ref")}"
    done
    printf '%s\n' "$value"
}

# The source code of the LGPL libraries in the app at $1 (LIC-01): the archives
# with the SHA-256 that build-codecs.sh pins, and the LAME patch and build-codecs.sh as they are
# in the source tree.
check_sources() {
    local sources="$1/Contents/Resources/licenses/sources" name archive actual patch_name
    for name in SOXR LAME; do
        archive="$sources/$(codec_pin "${name}_ARCHIVE")"
        [ -f "$archive" ] || die "${archive#"$1/"} is missing"
        actual="$(shasum -a 256 "$archive" | cut -d ' ' -f 1)"
        [ "$actual" = "$(codec_pin "${name}_SHA256")" ] ||
            die "${archive#"$1/"}: SHA-256 is $actual, build-codecs.sh pins $(codec_pin "${name}_SHA256")"
    done
    patch_name="$(codec_pin LAME_PATCH)"
    cmp -s "$sources/$patch_name" "$root/scripts/native/$patch_name" ||
        die "${sources#"$1/"}/$patch_name is missing or differs from scripts/native/$patch_name"
    cmp -s "$sources/build-codecs.sh" "$codecs_script" ||
        die "${sources#"$1/"}/build-codecs.sh is missing or differs from scripts/native/build-codecs.sh"
}

[ "$(uname -s)" = Darwin ] && [ "$(uname -m)" = arm64 ] ||
    die "bundles for aarch64-apple-darwin and runs on Apple Silicon macOS only (found $(uname -s) $(uname -m))"
for tool in cargo node pnpm codesign vtool hdiutil; do
    command -v "$tool" >/dev/null || die "$tool is missing"
done

# --- 1-3: codecs, frontend dependencies, licences ----------------------------------------

"$root/scripts/native/build-codecs.sh"
(cd "$desktop" && CI=true pnpm install --frozen-lockfile)
(cd "$desktop" && node scripts/stage-licenses.mjs)

# --- 4: the Tauri bundle -------------------------------------------------------------------

# The bundler signs everything it places (the app, the CLI, the frameworks) and builds the DMG.
# CI=true skips the Finder layout script of the DMG, which needs a logged-in Finder.
target_dir="$(cd "$root" && cargo metadata --format-version 1 --no-deps --locked |
    node -e 'let s = ""; process.stdin.on("data", (d) => (s += d)).on("end", () => console.log(JSON.parse(s).target_directory))')"
bundle_dir="$target_dir/release/bundle"
rm -rf "$bundle_dir/macos" "$bundle_dir/dmg"
# No Developer ID identity and no notarization credentials reach the bundler.
(cd "$desktop" &&
    env -u APPLE_SIGNING_IDENTITY -u APPLE_CERTIFICATE -u APPLE_CERTIFICATE_PASSWORD \
        -u APPLE_API_KEY -u APPLE_API_ISSUER -u APPLE_API_KEY_PATH \
        -u APPLE_ID -u APPLE_PASSWORD -u APPLE_TEAM_ID \
        CI=true pnpm tauri build --config src-tauri/tauri.bundle.conf.json -- --locked)

app="$bundle_dir/macos/Sonara.app"
version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Contents/Info.plist")"
dmg="$bundle_dir/dmg/Sonara_${version}_aarch64.dmg"
[ -d "$app" ] || die "$app was not built"
[ -f "$dmg" ] || die "$dmg was not built"

# --- 5: checks -----------------------------------------------------------------------------

# The layout: both executables, the codec libraries with @rpath install names, the licences.
[ -x "$app/Contents/MacOS/sonara-desktop" ] || die "Contents/MacOS/sonara-desktop is missing"
[ -x "$app/Contents/MacOS/sonara" ] || die "Contents/MacOS/sonara (the CLI) is missing"
for lib in "${CODECS[@]}"; do
    file="$app/Contents/Frameworks/$lib"
    [ -f "$file" ] || die "Contents/Frameworks/$lib is missing"
    [ "$(otool -D "$file" | tail -n 1)" = "@rpath/$lib" ] || die "$lib: the install name is not @rpath/$lib"
done
[ -s "$app/Contents/Resources/licenses/THIRD-PARTY-LICENSES.txt" ] ||
    die "Contents/Resources/licenses/THIRD-PARTY-LICENSES.txt is missing"
check_sources "$app"
/usr/libexec/PlistBuddy -c 'Print :NSMicrophoneUsageDescription' "$app/Contents/Info.plist" >/dev/null ||
    die "Info.plist has no NSMicrophoneUsageDescription"
[ "$(/usr/libexec/PlistBuddy -c 'Print :LSMinimumSystemVersion' "$app/Contents/Info.plist")" = "$MINIMUM_MACOS" ] ||
    die "Info.plist's LSMinimumSystemVersion is not $MINIMUM_MACOS"

# Every nested Mach-O is signed, and the whole bundle verifies. The signature is ad hoc.
codesign --verify --deep --strict --verbose=2 "$app"
codesign -dv "$app" 2>&1 | grep -x 'Signature=adhoc' >/dev/null || die "Sonara.app is not signed ad hoc"
entitlements="$(codesign -d --entitlements - --xml "$app/Contents/MacOS/sonara-desktop" 2>/dev/null)"
case "$entitlements" in
*com.apple.security.device.audio-input*) ;;
*) die "sonara-desktop is not signed with the audio-input entitlement" ;;
esac
case "$entitlements" in
*com.apple.security.app-sandbox*) die "sonara-desktop is sandboxed (the bundle must not use the App Sandbox)";
esac

# The codec libraries load from Contents/Frameworks: without SONARA_NATIVE_LIB_DIR, the bundled
# CLI offers MP3 and FLAC export (both are missing from export_formats when libmp3lame and
# libFLAC do not load, for example when library validation refuses them).
home="$(mktemp -d "${TMPDIR:-/tmp}/sonara-home.XXXXXX")"
capabilities="$(env -u SONARA_NATIVE_LIB_DIR SONARA_HOME="$home" \
    "$app/Contents/MacOS/sonara" tool get_capabilities --connect never)"
rm -rf "$home"
for format in mp3 flac; do
    case "$capabilities" in
    *"\"$format\""*) ;;
    *) die "the bundled CLI cannot export $format: a codec library in Contents/Frameworks did not load" ;;
    esac
done

# The minimum macOS of every Mach-O, at most MINIMUM_MACOS.
version_le() {
    [ "$(printf '%s\n%s\n' "$1" "$2" | sort -t . -k 1,1n -k 2,2n -k 3,3n | head -n 1)" = "$1" ]
}
echo "bundle-macos: Mach-O files (minos, signature flags):"
while IFS= read -r -d '' file; do
    file -b "$file" | grep -q '^Mach-O' || continue
    minos="$(vtool -show-build "$file" | awk '$1 == "minos" { print $2; exit }')"
    [ -n "$minos" ] || die "${file#"$app/"}: no minimum macOS in its build version"
    version_le "$minos" "$MINIMUM_MACOS" || die "${file#"$app/"}: minos $minos is newer than $MINIMUM_MACOS"
    flags="$(codesign -dv "$file" 2>&1 | sed -n 's/^CodeDirectory .*flags=\([^ ]*\).*/\1/p')"
    printf '  %-44s minos %-5s flags %s\n' "${file#"$app/"}" "$minos" "$flags"
done < <(find "$app" -type f -print0)

# The app inside the DMG verifies too, with the same source archives. The DMG file itself is not
# signed.
mount="$(mktemp -d "${TMPDIR:-/tmp}/sonara-dmg.XXXXXX")"
hdiutil attach -nobrowse -readonly -mountpoint "$mount" "$dmg" >/dev/null
trap 'hdiutil detach "$mount" >/dev/null 2>&1 || true; rmdir "$mount" 2>/dev/null || true' EXIT
codesign --verify --deep --strict "$mount/Sonara.app" || die "the app inside the DMG does not verify"
check_sources "$mount/Sonara.app"
[ -L "$mount/Applications" ] || die "the DMG has no Applications link"
hdiutil detach "$mount" >/dev/null
rmdir "$mount"
trap - EXIT

# The sizes.
app_kib="$(du -sk "$app" | cut -f 1)"
dmg_bytes="$(stat -f %z "$dmg")"
app_mib=$((app_kib / 1024))
dmg_mib=$((dmg_bytes / 1048576))
echo "bundle-macos: Sonara.app $app_mib MiB (budget $APP_BUDGET_MIB MiB), DMG $dmg_mib MiB (budget $DMG_BUDGET_MIB MiB)"
[ "$app_mib" -le "$APP_BUDGET_MIB" ] || die "Sonara.app is over its size budget"
[ "$dmg_mib" -le "$DMG_BUDGET_MIB" ] || die "the DMG is over its size budget"

echo "bundle-macos: built $app"
echo "bundle-macos: built $dmg"
