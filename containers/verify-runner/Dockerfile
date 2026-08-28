# syntax=docker/dockerfile:1
#
# Build with the REPO ROOT as context (Docker cannot COPY paths outside the build context, and
# `dry-core = { path = "../../crates/core" }` in this crate's Cargo.toml reaches outside
# `containers/verify-runner/`; `crates/core`'s own manifest also inherits `edition`/`rust-version`/
# etc. from the root `Cargo.toml`'s `[workspace.package]`, which requires the *whole* root workspace
# — including its other member manifests — to be present and parseable, not just crates/core):
#
#   docker build -f containers/verify-runner/Dockerfile -t dry-verify-runner .
#
# The repo root `.dockerignore` trims the context to a reasonable size (drops target/, .git,
# node_modules, docs/site, py/.venv).

# --- build stage -------------------------------------------------------------------------------
FROM rust:1.88 AS build
WORKDIR /src

COPY . .

WORKDIR /src/containers/verify-runner
RUN cargo build --release --locked

# --- runtime stage -------------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# reqwest is built with rustls-tls (no openssl), so no ca-certificates/libssl runtime package is
# required for the registry HTTPS fetch. `/tmp` ships writable (world-writable + sticky bit) in the
# base image for ALL users, not just root, so the non-root user below can still stream request
# bodies there — no extra chmod needed.
#
# Install curl for container healthcheck probe.
RUN apt-get update && apt-get install -y --no-install-recommends curl && rm -rf /var/lib/apt/lists/*

# Run as a fixed-uid, non-root, non-login system user (defense in depth: a dry-core panic or an
# engine bug in the request path shouldn't hand an attacker root inside the container).
RUN useradd --system --uid 10001 --no-create-home --shell /usr/sbin/nologin runner

COPY --from=build /src/containers/verify-runner/target/release/dry-verify-runner /usr/local/bin/dry-verify-runner

USER runner
EXPOSE 8080
HEALTHCHECK --interval=15s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://127.0.0.1:8080/healthz || exit 1

ENTRYPOINT ["/usr/local/bin/dry-verify-runner"]
