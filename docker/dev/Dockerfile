# syntax=docker/dockerfile:1
#
# Sonara development container: the Rust toolchain pinned in
# rust-toolchain.toml, a C toolchain, and Node, pnpm and Playwright's WebKit for the desktop
# frontend. Use it through scripts/dev.sh.
FROM debian:trixie-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad

ARG DEBIAN_FRONTEND=noninteractive
# libsoxr0 is the resampler that sonara-media loads at run time; trixie ships
# 0.1.3-4+b2. libmp3lame0 is LAME 3.100, the MP3 encoder sonara-media loads at run time.
# libflac14 is the FLAC encoder sonara-media loads at run time (libFLAC.so.14): trixie ships
# libFLAC 1.5.0 (1.5.0+ds-2), not the recording's 1.4.3, so FLAC exports are compared with the
# golden files byte for byte on macOS only (crates/sonara-media/tests/golden_export.rs).
# Only the shared libraries are installed: the Rust code never links against them. cmake builds
# the bundled libopus of the opus crate (opusic-sys) for the Opus export.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates curl git \
        build-essential pkg-config cmake \
        libasound2-dev libpulse-dev \
        libsoxr0 libmp3lame0 libflac14 \
    && rm -rf /var/lib/apt/lists/*

# Rust: rustup only; the toolchain itself comes from rust-toolchain.toml (the single pin).
ENV RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo \
    PATH=/opt/cargo/bin:$PATH
ARG RUSTUP_VERSION=1.29.1
ARG TARGETARCH
RUN set -eu; \
    case "$TARGETARCH" in \
        arm64) target=aarch64-unknown-linux-gnu; sha=15f6e4ce9f583b929c996c91562bad6d4454f3281de858b02cdfdef615fac433 ;; \
        amd64) target=x86_64-unknown-linux-gnu; sha=dda7234360b7f578ca8b0ddcb80145646fa61a67c1720a5abc7051b35c9fcb71 ;; \
        *) echo "unsupported architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    curl -sSfL -o /tmp/rustup-init "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/${target}/rustup-init"; \
    echo "${sha}  /tmp/rustup-init" | sha256sum -c -; \
    chmod +x /tmp/rustup-init; \
    /tmp/rustup-init -y --no-modify-path --profile minimal --default-toolchain none; \
    rm /tmp/rustup-init
COPY rust-toolchain.toml /opt/toolchain/rust-toolchain.toml
RUN cd /opt/toolchain && rustup toolchain install && rustc --version && cargo clippy --version

# Cargo tools used by the gates.
ARG CARGO_DENY_VERSION=0.20.2
RUN cd /opt/toolchain && cargo install --locked "cargo-deny@${CARGO_DENY_VERSION}" \
    && rm -rf /opt/cargo/registry /opt/cargo/git

# The desktop frontend's toolchain (`scripts/dev.sh frontend`): Node from the
# official tarball, its SHA-256 checked, and pnpm, both the versions the host uses (pnpm equals
# `packageManager` in apps/sonara-desktop/package.json).
ARG NODE_VERSION=26.12.1
ARG PNPM_VERSION=12.14.0
ENV PATH=/opt/node/bin:$PATH
RUN set -eu; \
    case "$TARGETARCH" in \
        arm64) arch=arm64; sha=42aa04922591ee5d4dbc7740d452621acf28508239daa512f5ee8c47ee224fdb ;; \
        amd64) arch=x64; sha=6170b2d474e1c626f43cff3adc118df9fe3b573258a4fc3d850c8dbc6b079bbd ;; \
        *) echo "unsupported architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    curl -sSfL -o /tmp/node.tar.xz "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-${arch}.tar.xz"; \
    echo "${sha}  /tmp/node.tar.xz" | sha256sum -c -; \
    mkdir -p /opt/node; \
    tar -xJf /tmp/node.tar.xz -C /opt/node --strip-components=1; \
    rm /tmp/node.tar.xz; \
    npm install --global --no-fund --no-audit "pnpm@${PNPM_VERSION}"; \
    rm -rf /root/.npm; \
    node --version; pnpm --version

# Playwright's WebKit and its system libraries, for the frontend's page tests. The version
# equals `@playwright/test` in apps/sonara-desktop/package.json: each Playwright release pins
# its browser builds.
ARG PLAYWRIGHT_VERSION=1.64.1
ENV PLAYWRIGHT_BROWSERS_PATH=/opt/ms-playwright
RUN npx --yes "playwright@${PLAYWRIGHT_VERSION}" install --with-deps webkit \
    && rm -rf /root/.npm /var/lib/apt/lists/*


WORKDIR /work
