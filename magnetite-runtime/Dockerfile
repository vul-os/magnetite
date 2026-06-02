# =============================================================================
# magnetite-runtime/Dockerfile
#
# Builds the `magnetite-serve` binary and produces a minimal runtime image.
#
# The binary accepts:
#   --wasm <path>    path to a compiled wasm32-wasip1 game module
#   --host <addr>    bind address   [default: 0.0.0.0 via RUNTIME_HOST]
#   --port <port>    WebSocket port [default: 9000 via RUNTIME_PORT]
#   --workers <n>    Tokio workers  [default: 0 (auto)]
#
# Run (with a game.wasm bind-mounted):
#   docker build -f magnetite-runtime/Dockerfile -t magnetite-runtime .
#   docker run --rm -p 9000:9000 \
#       -v $(pwd)/game.wasm:/game.wasm \
#       magnetite-runtime --wasm /game.wasm --host 0.0.0.0
#
# Run (NopGame — no .wasm needed, for smoke-testing):
#   docker run --rm -p 9000:9000 magnetite-runtime --host 0.0.0.0
#
# Environment variables (all optional):
#   RUNTIME_HOST     bind address override (default: 0.0.0.0)
#   RUNTIME_PORT     port override (default: 9000)
#   RUNTIME_WORKERS  Tokio worker threads (default: 0 = auto)
#   RUST_LOG         tracing filter (default: info)
# =============================================================================

# ---------------------------------------------------------------------------
# Stage 1 — build
# ---------------------------------------------------------------------------
FROM rust:1.82-slim-bookworm AS builder

# Build dependencies (Wasmtime needs a C toolchain for linking).
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    clang \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy the full workspace so inter-crate path dependencies resolve.
# We copy selectively to avoid invalidating the layer on frontend changes.
COPY backend/magnetite-sdk/ ./backend/magnetite-sdk/
COPY magnetite-anticheat/   ./magnetite-anticheat/
COPY magnetite-sandbox/     ./magnetite-sandbox/
COPY magnetite-runtime/     ./magnetite-runtime/

# The workspace Cargo.toml and lock file are needed for the resolver.
COPY Cargo.toml              ./Cargo.toml
COPY Cargo.lock              ./Cargo.lock

# Build only the serve binary in release mode.
# --package scopes the build; the workspace resolver handles dependencies.
RUN cargo build --release --package magnetite-runtime --bin serve

# ---------------------------------------------------------------------------
# Stage 2 — runtime image
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Runtime C libraries (Wasmtime links against libgcc / glibc).
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary.
COPY --from=builder \
    /build/target/release/serve \
    /usr/local/bin/magnetite-serve

# Default environment.
ENV RUNTIME_HOST=0.0.0.0 \
    RUNTIME_PORT=9000 \
    RUNTIME_WORKERS=0 \
    RUST_LOG=info

EXPOSE 9000

# The entrypoint honours RUNTIME_HOST / RUNTIME_PORT via the binary's
# env-var defaults; CLI flags can override them at `docker run` time.
ENTRYPOINT ["/usr/local/bin/magnetite-serve"]
CMD ["--host", "0.0.0.0", "--port", "9000"]
