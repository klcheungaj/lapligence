# syntax=docker/dockerfile:1
#
# Dev / dependency image – contains the full build toolchain and all pre-fetched
# Cargo crates, but does NOT build the project itself.
#
# Build the image (UID/GID default to 1000; override to match your host user):
#   docker build -t llg-dev .
#   docker build --build-arg UID=$(id -u) --build-arg GID=$(id -g) -t llg-dev .
#
# Build the project (debug):
#   docker run --rm \
#     -v "$(pwd)":/workspace \
#     -v llg-target:/workspace/target \
#     llg-dev
#
# Build release:
#   docker run --rm \
#     -v "$(pwd)":/workspace \
#     -v llg-target:/workspace/target \
#     llg-dev \
#     cargo build --locked --release --target x86_64-unknown-linux-musl
#
# The named volume `llg-target` persists incremental compilation artefacts
# across runs.  Drop it with `docker volume rm llg-target` for a clean build.

# ── Stage 1: Toolchain ───────────────────────────────────────────────────────
FROM alpine:3.20 AS toolchain

# Build toolchain and Slang's non-Rust dependencies:
#   build-base   – gcc, g++, make, binutils (all target musl natively on Alpine)
#   cmake        – CMake ≥ 3.20 is provided by Alpine 3.20
#   python3      – required by Slang's syntax and diagnostic generators
#   zlib-dev / zlib-static – generated FST waveform models link zlib statically
#   curl         – used by the Rust installer
#   patch        – applies vendored fixes even when submodule Git metadata is
#                  unavailable through a bind mount
RUN apk update
RUN apk add --no-cache \
        build-base \
        cmake \
        python3 \
        zlib-dev \
        zlib-static \
        libstdc++-dev \
        curl \
        linux-headers \
        git \
        patch

# Install Rust into /opt so any user can access the toolchain.
# RUSTUP_HOME  – toolchain binaries (rustc, cargo, …)
# CARGO_HOME   – registry index and downloaded crate sources
ENV RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y \
        --no-modify-path \
        --default-toolchain 1.98.0 \
        --profile minimal

ENV PATH="/opt/cargo/bin:${PATH}"

# Add the musl target so Cargo produces a fully static binary
RUN rustup target add x86_64-unknown-linux-musl

# build.rs and the cc crate both look for the triple-prefixed compiler
# (x86_64-linux-musl-gcc).  On Alpine, gcc/g++ already target musl natively,
# so we just symlink them under the expected names.
RUN ln -sf /usr/bin/gcc  /usr/local/bin/x86_64-linux-musl-gcc  && \
    ln -sf /usr/bin/g++  /usr/local/bin/x86_64-linux-musl-g++

# Create a non-root user whose UID/GID can be matched to the host user so
# that bind-mounted source files are owned correctly on both sides.
ARG UID=1000
ARG GID=1000
RUN addgroup -g ${GID} builder && \
    adduser -u ${UID} -G builder -s /bin/sh -D builder && \
    chown -R builder:builder /opt/rustup /opt/cargo

# ── Stage 2: Pre-fetch Cargo dependencies ────────────────────────────────────
# Crates are downloaded into /opt/cargo and baked into the image so they are
# available without network access when the container is run later.
# Source code is intentionally NOT copied – it will be bind-mounted at runtime.
FROM toolchain AS dev

USER builder
WORKDIR /workspace

# Copy only the manifests so this layer is invalidated only when dependencies change.
COPY --chown=builder:builder Cargo.toml Cargo.lock ./

# Stub out the binary entry-points so `cargo fetch` can resolve the workspace
# graph without requiring the real sources.
RUN mkdir -p src/bin && \
    echo 'fn main(){}' > src/main.rs && \
    echo 'fn main(){}' > src/bin/helloslang.rs

# Fetch all crates into the image (no cache mount – crates must live in the layer).
RUN cargo fetch --locked --target x86_64-unknown-linux-musl

# Remove stubs; real sources are bind-mounted from the host at runtime.
RUN rm -f src/main.rs src/bin/helloslang.rs

# Default command builds the project in debug mode against the musl target.
# Override by passing a different `cargo` invocation to `docker run`.
CMD ["cargo", "build", "--locked", "--target", "x86_64-unknown-linux-musl"]
