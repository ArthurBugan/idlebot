FROM rust:1.97.1 AS builder

# Build essentials for any C-based build scripts (zstd/lz4/openssl, etc.).
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# wasm target + the wasm-bindgen CLI pinned to the version in Cargo.lock.
RUN rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version 0.2.127 --locked

WORKDIR /app

# Copy the whole workspace (only the client bin is built, so the other
# crates' heavy deps are never compiled).
COPY . .

# Build the release wasm and generate the web bundle into /app/dist.
RUN cargo build -p idlecore-client --bin idlecore-client \
        --target wasm32-unknown-unknown --release \
    && mkdir -p dist \
    && wasm-bindgen target/wasm32-unknown-unknown/release/idlecore-client.wasm \
        --out-dir dist --target web --out-name idlecore_client \
    && cp -R crates/idlecore-client/assets/* dist/ \
    && cp docker/index.html dist/index.html

# ---------------------------------------------------------------------------
# Stage 2 — serve the static bundle
# ---------------------------------------------------------------------------
FROM nginx:alpine

COPY docker/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist /usr/share/nginx/html

EXPOSE 80
