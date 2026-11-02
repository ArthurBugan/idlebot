FROM rust:1.97.1 AS builder

# Build essentials for any C-based build scripts (zstd/lz4/openssl, etc.).
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# wasm target + the wasm-bindgen CLI pinned to the version in Cargo.lock
# (the CLI version must exactly match the wasm-bindgen crate version).
RUN rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version 0.2.130

WORKDIR /app

# Copy the whole workspace (only the client bin is built, so the other
# crates' heavy deps are never compiled).
COPY . .

# Docker VMs ship ~8 GB of RAM and the fat-LTO finisher over the Bevy app
# needs more than that — the final rustc gets SIGKILLed ("cannot allocate
# memory") right after its 360 s dependency pass. Thin LTO keeps the
# cross-crate dead-code elimination the wasm size depends on while
# streaming per-module bitcode across worker threads, so peak RSS stays
# within the VM. Env override so the local `cargo build --release`
# profile in Cargo.toml stays untouched.
ENV CARGO_PROFILE_RELEASE_LTO=thin

# Build the release wasm and generate the web bundle into /app/dist.
RUN cargo build -p idlecore-client --bin idlecore-client \
        --target wasm32-unknown-unknown --release \
    && mkdir -p dist \
    && wasm-bindgen target/wasm32-unknown-unknown/release/idlecore-client.wasm \
        --out-dir dist --target web --out-name idlecore_client \
    && cp -R crates/idlecore-client/assets/* dist/ \
    && cp docker/index.html dist/index.html

# ---------------------------------------------------------------------------
# Stage 2 — serve the static bundle with Bun
# ---------------------------------------------------------------------------
FROM oven/bun:latest

WORKDIR /app
COPY --from=builder /app/dist ./public
COPY docker/server.ts ./server.ts
COPY docker/precompress.ts ./precompress.ts

# Serve the wasm/assets compressed (Accept-Encoding: br/gz) — shrinks the
# download ~60-70% without touching the build. Bun.serve serves the
# precompressed siblings; server.ts picks them up.
RUN bun run precompress.ts public

EXPOSE 3012
CMD ["bun", "run", "server.ts"]
