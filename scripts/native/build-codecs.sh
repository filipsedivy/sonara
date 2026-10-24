#!/usr/bin/env bash
# Builds the codec libraries that sonara-media loads at run time (codec_ffi.rs;
# decided points R1 and R2, and the FLAC decision of 2026-10-22; LIC-01) from pinned sources, as
# dynamic libraries for aarch64-apple-darwin:
#
#   target/native/lib/libsoxr.0.dylib      libsoxr at commit a66f3ee, as python-soxr 1.1 builds it
#   target/native/lib/libmp3lame.0.dylib   LAME 3.100, as soundfile 0.14's libsndfile builds it
#   target/native/lib/libFLAC.12.dylib     libFLAC 1.4.3, as soundfile 0.14's libsndfile builds it
#   target/native/licenses/                their licence texts, for the app bundle:
#                                          libsoxr's notice, its LGPL-2.1 text and the PFFFT
#                                          licence (cr32s uses PFFFT), LAME's LGPL-2 text and its
#                                          LICENSE note, libFLAC's COPYING.Xiph
#   target/native/downloads/               the verified source archives (SOXR_ARCHIVE,
#                                          LAME_ARCHIVE, FLAC_ARCHIVE), kept between builds. The
#                                          app bundle ships the two LGPL archives, this script and
# LAME_PATCH as the libraries' source
#                                          (apps/sonara-desktop/scripts/stage-licenses.mjs)
#
# All three are the builds the golden data was recorded with (GLD-01), so the media and export
# tests compare against the same arithmetic. Each install name is @rpath/<file>, ready for the app
# bundle's Contents/Frameworks. Cargo points SONARA_NATIVE_LIB_DIR at target/native/lib
# (.cargo/config.toml), so tests and `cargo run` load these first.
#
# Flags (RT-11): Apple Clang, -mcpu=apple-m1, -O3, never fast-math. Floating-point
# contraction stays at Apple Clang's default, as in the recording's builds. The deployment
# target 11.0 is the one the recorded builds use (python-soxr's macosx_11_0_arm64 wheel,
# libsndfile-binaries' arm64 build). libFLAC's own reassociation flags are removed too (see its
# section).
#
# The script is idempotent: it does nothing when target/native was built by this exact script
# and LAME_PATCH and still holds the libraries and the source archives (CI caches target/native
# under the hash of both files). Needs cmake, curl and the Xcode command line tools; runs on
# Apple Silicon only.
#
#   scripts/native/build-codecs.sh            build unless up to date
#   scripts/native/build-codecs.sh --force    rebuild
set -euo pipefail

# libsoxr: python-soxr 1.1.0's `libsoxr` submodule, the dofuuz/soxr fork at a66f3ee
# (`0.1.3-14-ga66f3ee`, the version golden/media/index.json records). The fork has no release
# tarball, so the pin is GitHub's archive of the commit. GitHub does not promise byte-stable
# archives; if the hash ever drifts, the commit hash itself stays the authority.
SOXR_COMMIT=a66f3eeeeb62a32403ff143b756eed92b1ec6b62
SOXR_URL="https://github.com/dofuuz/soxr/archive/${SOXR_COMMIT}.tar.gz"
SOXR_SHA256=d72cf79f82b4bc56d853c8e166bef994f0bbf9a1ca90ae6f81a3847081f48b96
SOXR_ARCHIVE="soxr-${SOXR_COMMIT}.tar.gz"

# LAME: the official 3.100 release tarball.
LAME_VERSION=3.100
LAME_URL="https://downloads.sourceforge.net/project/lame/lame/${LAME_VERSION}/lame-${LAME_VERSION}.tar.gz"
LAME_SHA256=ddfe36cab873794038ae2c1210557ad34857a4b6bdc515785d1da9e175b1da1e
LAME_ARCHIVE="lame-${LAME_VERSION}.tar.gz"
# The one change to LAME's sources, a file beside this script (see the LAME section).
LAME_PATCH="lame-${LAME_VERSION}-export-list.patch"

# libFLAC: the official 1.4.3 release tarball (Xiph.Org).
FLAC_VERSION=1.4.3
FLAC_URL="https://downloads.xiph.org/releases/flac/flac-${FLAC_VERSION}.tar.xz"
FLAC_SHA256=6c58e69cd22348f441b861092b825e591d0b822e106de6eb0ee4d05d27205b70
FLAC_ARCHIVE="flac-${FLAC_VERSION}.tar.xz"
# The vendor string libFLAC writes into every file; the golden files carry it.
FLAC_VENDOR="reference libFLAC ${FLAC_VERSION} 20230623"

DEPLOYMENT_TARGET=11.0
CPU=apple-m1

root="$(cd "$(dirname "$0")/../.." && pwd)"
script="$root/scripts/native/build-codecs.sh"
out="$root/target/native"
lib="$out/lib"
downloads="$out/downloads"
work="$out/build"
stamp="$out/.stamp"

die() {
    echo "build-codecs: $*" >&2
    exit 1
}

[ "$(uname -s)" = Darwin ] && [ "$(uname -m)" = arm64 ] ||
    die "builds for aarch64-apple-darwin and runs on Apple Silicon macOS only (found $(uname -s) $(uname -m))"
command -v cmake >/dev/null || die "cmake is missing"

# The build's inputs: this script and the LAME patch.
inputs_hash="$(cat "$script" "$root/scripts/native/$LAME_PATCH" | shasum -a 256 | cut -d ' ' -f 1)"
if [ "${1:-}" != --force ] && [ -f "$stamp" ] && [ "$(cat "$stamp")" = "$inputs_hash" ] &&
    [ -f "$lib/libsoxr.0.dylib" ] && [ -f "$lib/libmp3lame.0.dylib" ] &&
    [ -f "$lib/libFLAC.12.dylib" ] && [ -f "$downloads/$SOXR_ARCHIVE" ] &&
    [ -f "$downloads/$LAME_ARCHIVE" ]; then
    echo "build-codecs: up to date ($lib)"
    exit 0
fi

# Only the flags below reach the compilers.
unset CFLAGS CXXFLAGS CPPFLAGS LDFLAGS CC CXX
export MACOSX_DEPLOYMENT_TARGET="$DEPLOYMENT_TARGET"
# Clang called by its full path finds the SDK only through SDKROOT.
SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
export SDKROOT
cc="$(xcrun --find clang)"
"$cc" --version | head -n 1 | grep -q '^Apple clang' || die "$cc is not Apple Clang"
jobs="$(sysctl -n hw.ncpu)"

rm -f "$stamp"
rm -rf "$work"
mkdir -p "$downloads" "$work" "$lib" "$out/licenses"

# Downloads `url` to `file` unless it is already there, and checks its SHA-256.
fetch() {
    local url="$1" sha256="$2" file="$3" actual
    if [ ! -f "$file" ]; then
        echo "build-codecs: downloading $url"
        curl -fsSL --retry 3 -o "$file.part" "$url"
        mv "$file.part" "$file"
    fi
    actual="$(shasum -a 256 "$file" | cut -d ' ' -f 1)"
    if [ "$actual" != "$sha256" ]; then
        rm -f "$file"
        die "$url: SHA-256 is $actual, expected $sha256"
    fi
}

# Fails when a build would use fast-math (RT-11).
check_flags() {
    local what="$1" flags="$2"
    case "$flags" in
    *fast-math* | *-Ofast* | *-ffinite-math* | *-funsafe-math*)
        die "$what would build with fast-math: $flags"
        ;;
    esac
    echo "build-codecs: $what flags: $flags"
}

# Sets the install name to @rpath/<file> and signs the result again: editing a Mach-O file
# invalidates the linker's ad-hoc signature, and Apple Silicon refuses to load it unsigned.
finish_dylib() {
    local file="$1"
    install_name_tool -id "@rpath/$(basename "$file")" "$file"
    codesign --force --sign - "$file"
    [ "$(otool -D "$file" | tail -n 1)" = "@rpath/$(basename "$file")" ] ||
        die "$file: install name not set"
}

# --- libsoxr --------------------------------------------------------------------------------

fetch "$SOXR_URL" "$SOXR_SHA256" "$downloads/$SOXR_ARCHIVE"
mkdir -p "$work/soxr-src"
tar -xzf "$downloads/$SOXR_ARCHIVE" -C "$work/soxr-src" --strip-components 1

# python-soxr 1.1's CMakeLists.txt (`add_subdirectory(libsoxr)`): no tests, no OpenMP, no
# libsamplerate bindings, built by scikit-build-core as Release (-O3 -DNDEBUG). Everything else
# keeps libsoxr's defaults, which on arm64 give the engines of python-soxr's macOS wheel: cr32,
# cr32s with PFFFT (NEON), cr64 (the VHQ engine), vr32; no cr64s (FindSIMD64 finds nothing on
# arm) and no libavcodec FFT. Only the library type differs: shared here, static in the wheel.
# libsoxr's extra GCC/Clang flags (warnings, -std=gnu99, hidden visibility) test for the compiler
# ID "Clang", which Apple Clang ("AppleClang") is not, so they apply to neither build; that is
# why the internal _soxr_* functions are exported, exactly as in the wheel.
cmake -S "$work/soxr-src" -B "$work/soxr-build" -G "Unix Makefiles" \
    -DCMAKE_C_COMPILER="$cc" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_C_FLAGS="-mcpu=$CPU" \
    -DCMAKE_OSX_ARCHITECTURES=arm64 \
    -DCMAKE_OSX_DEPLOYMENT_TARGET="$DEPLOYMENT_TARGET" \
    -DBUILD_SHARED_LIBS=ON \
    -DBUILD_TESTS=OFF \
    -DWITH_OPENMP=OFF \
    -DWITH_LSR_BINDINGS=OFF
check_flags libsoxr "$(grep -h -E '^C_(DEFINES|FLAGS) ' "$work/soxr-build/src/CMakeFiles/soxr.dir/flags.make" | tr '\n' ' ')"
cmake --build "$work/soxr-build" --target soxr --parallel "$jobs"

# The engine set must be the recorded build's (see above).
objects="$work/soxr-build/src/CMakeFiles/soxr.dir"
for engine in cr32 cr32s cr64 vr32 pffft32s; do
    [ -f "$objects/$engine.c.o" ] || die "libsoxr was configured without $engine"
done
for engine in cr64s avfft32s; do
    [ ! -f "$objects/$engine.c.o" ] || die "libsoxr was configured with $engine"
done

cp -L "$work/soxr-build/src/libsoxr.0.dylib" "$work/libsoxr.0.dylib"
finish_dylib "$work/libsoxr.0.dylib"
cp "$work/soxr-src/LICENCE" "$out/licenses/libsoxr-LICENCE.txt"
cp "$work/soxr-src/COPYING.LGPL" "$out/licenses/libsoxr-COPYING.LGPL.txt"
# LICENCE says "If building with pffft.c, see the licence embedded in that file": the cr32s engine
# is built with it, so its licence, the file's first two comments (its origin, then the FFTPACK
# licence), ships too.
awk '{ print } /\*\// { if (++comments == 2) exit }' "$work/soxr-src/src/pffft.c" \
    >"$out/licenses/libsoxr-pffft-LICENSE.txt"
grep -q 'FFTPACK license' "$out/licenses/libsoxr-pffft-LICENSE.txt" &&
    grep -q 'OTHER DEALINGS WITH THE' "$out/licenses/libsoxr-pffft-LICENSE.txt" ||
    die "src/pffft.c: its licence comment was not found"

# --- LAME -----------------------------------------------------------------------------------

fetch "$LAME_URL" "$LAME_SHA256" "$downloads/$LAME_ARCHIVE"
mkdir -p "$work/lame-src"
tar -xzf "$downloads/$LAME_ARCHIVE" -C "$work/lame-src" --strip-components 1

# 3.100 exports `lame_init_old` but no longer defines it, which the macOS linker rejects for a
# shared library. Homebrew and MacPorts drop the line the same way; the code is unchanged. The
# patch file is the change itself, so the copy the app bundle ships is exactly what was applied
# (LIC-01); it must apply without fuzz.
grep -q -x 'lame_init_old' "$work/lame-src/include/libmp3lame.sym" ||
    die "include/libmp3lame.sym: lame_init_old is not there to remove"
patch -p1 -N -s -F 0 -d "$work/lame-src" -i "$root/scripts/native/$LAME_PATCH" ||
    die "$LAME_PATCH does not apply to LAME $LAME_VERSION"
! grep -q -x 'lame_init_old' "$work/lame-src/include/libmp3lame.sym" ||
    die "$LAME_PATCH did not remove lame_init_old"

# libsndfile-binaries' mac_build.sh for soundfile 0.14 (`--host=aarch64-apple-darwin`, the
# default library options), as a shared library and without the frontend. With Clang, LAME's
# configure uses -O3 and never adds fast-math; -O3 is passed explicitly all the same.
(
    cd "$work/lame-src"
    CC="$cc" CFLAGS="-O3 -arch arm64 -mcpu=$CPU -mmacosx-version-min=$DEPLOYMENT_TARGET" \
        ./configure --host=aarch64-apple-darwin --prefix="$work/lame-prefix" \
        --enable-shared --disable-static --disable-frontend --disable-gtktest \
        --disable-dependency-tracking
)
check_flags LAME "$(grep -h '^CFLAGS = ' "$work/lame-src/libmp3lame/Makefile")"
make -C "$work/lame-src" -j "$jobs"

cp -L "$work/lame-src/libmp3lame/.libs/libmp3lame.0.dylib" "$work/libmp3lame.0.dylib"
finish_dylib "$work/libmp3lame.0.dylib"
cp "$work/lame-src/COPYING" "$out/licenses/lame-COPYING.txt"
cp "$work/lame-src/LICENSE" "$out/licenses/lame-LICENSE.txt"

# --- libFLAC --------------------------------------------------------------------------------

fetch "$FLAC_URL" "$FLAC_SHA256" "$downloads/$FLAC_ARCHIVE"
mkdir -p "$work/flac-src"
tar -xJf "$downloads/$FLAC_ARCHIVE" -C "$work/flac-src" --strip-components 1

# libsndfile-binaries' mac_build.sh for soundfile 0.14 builds libFLAC 1.4.3 with autotools
# (`--host=aarch64-apple-darwin --target=arm64-apple-macos11`, CFLAGS `-arch arm64 -target
# arm64-apple-macos11`, static, linked into libsndfile). Its configure gives a release build:
# `-O3 -funroll-loops`, NDEBUG, the A64 NEON intrinsics (arm_neon.h is found), 64-bit bit-writer
# words, and libFLAC's own `-fassociative-math -fno-signed-zeros -fno-trapping-math
# -freciprocal-math` (src/libFLAC/Makefile.am, for every release build whose compiler accepts
# them). This cmake build reproduces that configuration as a shared library, without the C++
# library, the programs, examples, tests, docs and Ogg: CMAKE_BUILD_TYPE=Release adds -O3 and
# NDEBUG, and -funroll-loops is passed explicitly. It omits the four reassociation flags, which
# are fast-math components (RT-11): the line of src/libFLAC/CMakeLists.txt that adds them is
# removed before configuring, and the build fails if any of them reaches the compiler. Measured
# on 2026-10-22: the golden FLAC files (10 export cases, 9 stems, 2 workflow renders) are
# byte-identical with and without them, so the recording's arithmetic is unchanged on that data.
# -mcpu=apple-m1 is Apple Clang's default CPU for the recording's arm64-apple-macos11 target, so
# it selects the same code. Iconv (for the programs' UTF-8 code) is not searched, so nothing from
# Homebrew is linked.
assoc_line='target_compile_options(FLAC BEFORE PRIVATE -fassociative-math -fno-signed-zeros -fno-trapping-math -freciprocal-math)'
flac_cmakelists="$work/flac-src/src/libFLAC/CMakeLists.txt"
[ "$(grep -c -F "$assoc_line" "$flac_cmakelists")" = 1 ] ||
    die "$flac_cmakelists: the reassociation line to remove is not there exactly once"
sed -i '' "s/^\([[:space:]]*\)target_compile_options(FLAC BEFORE PRIVATE -fassociative-math -fno-signed-zeros -fno-trapping-math -freciprocal-math)\$/\1# reassociation flags removed by build-codecs.sh (RT-11)/" "$flac_cmakelists"
# The `check_c_compiler_flag` probe above that line stays: it only tests the compiler.
[ "$(grep -c -F "$assoc_line" "$flac_cmakelists")" = 0 ] &&
    grep -q -F '# reassociation flags removed by build-codecs.sh (RT-11)' "$flac_cmakelists" ||
    die "$flac_cmakelists: the reassociation flags were not removed"
cmake -S "$work/flac-src" -B "$work/flac-build" -G "Unix Makefiles" \
    -DCMAKE_C_COMPILER="$cc" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_C_FLAGS="-mcpu=$CPU -funroll-loops" \
    -DCMAKE_OSX_ARCHITECTURES=arm64 \
    -DCMAKE_OSX_DEPLOYMENT_TARGET="$DEPLOYMENT_TARGET" \
    -DCMAKE_DISABLE_FIND_PACKAGE_Iconv=ON \
    -DBUILD_SHARED_LIBS=ON \
    -DBUILD_CXXLIBS=OFF \
    -DBUILD_PROGRAMS=OFF \
    -DBUILD_EXAMPLES=OFF \
    -DBUILD_TESTING=OFF \
    -DBUILD_DOCS=OFF \
    -DWITH_OGG=OFF \
    -DINSTALL_MANPAGES=OFF \
    -DINSTALL_PKGCONFIG_MODULES=OFF \
    -DINSTALL_CMAKE_CONFIG_MODULE=OFF
# Every compile-flag line, including the per-architecture `C_FLAGSarm64`.
flac_flags="$(grep -h -E '^C_(DEFINES|FLAGS)[[:alnum:]_]* ' "$work/flac-build/src/libFLAC/CMakeFiles/FLAC.dir/flags.make" | tr '\n' ' ')"
check_flags libFLAC "$flac_flags"
# No reassociation (RT-11, see above).
for flag in -fassociative-math -fno-signed-zeros -fno-trapping-math -freciprocal-math; do
    case " $flac_flags " in
    *" $flag "*) die "libFLAC would build with $flag" ;;
    esac
done
# The recording's configuration (see above): the unrolled -O3 release build and the A64 NEON
# intrinsics.
for flag in -O3 -funroll-loops -DNDEBUG; do
    case " $flac_flags " in
    *" $flag "*) ;;
    *) die "libFLAC was configured without $flag" ;;
    esac
done
for define in FLAC__HAS_NEONINTRIN FLAC__HAS_A64NEONINTRIN ENABLE_64_BIT_WORDS; do
    grep -q -E "^#define $define 1\$" "$work/flac-build/config.h" ||
        die "libFLAC was configured without $define"
done
grep -q -E '^#define FLAC__HAS_OGG OGG_FOUND$' "$work/flac-build/config.h" &&
    grep -q -E '^#define OGG_FOUND 0$' "$work/flac-build/config.h" ||
    die "libFLAC was configured with Ogg"
cmake --build "$work/flac-build" --target FLAC --parallel "$jobs"

cp -L "$work/flac-build/src/libFLAC/libFLAC.12.dylib" "$work/libFLAC.12.dylib"
finish_dylib "$work/libFLAC.12.dylib"
# No git metadata reaches the build, so the vendor string is the release's.
# grep reads all of its input (no -q), so pipefail sees no SIGPIPE from strings.
strings "$work/libFLAC.12.dylib" | grep -x "$FLAC_VENDOR" >/dev/null ||
    die "libFLAC.12.dylib does not carry the vendor string \"$FLAC_VENDOR\""
# libFLAC needs only the system library.
[ "$(otool -L "$work/libFLAC.12.dylib" | tail -n +2 | grep -v -c -E '@rpath/libFLAC\.12\.dylib|/usr/lib/libSystem\.B\.dylib')" = 0 ] ||
    die "libFLAC.12.dylib links more than libSystem: $(otool -L "$work/libFLAC.12.dylib")"
cp "$work/flac-src/COPYING.Xiph" "$out/licenses/libFLAC-COPYING.Xiph.txt"

# --- Install --------------------------------------------------------------------------------

mv -f "$work/libsoxr.0.dylib" "$work/libmp3lame.0.dylib" "$work/libFLAC.12.dylib" "$lib/"
rm -rf "$work"
echo "$inputs_hash" >"$stamp"
echo "build-codecs: built $lib/libsoxr.0.dylib (libsoxr $SOXR_COMMIT)," \
    "$lib/libmp3lame.0.dylib (LAME $LAME_VERSION) and $lib/libFLAC.12.dylib (libFLAC $FLAC_VERSION)"
