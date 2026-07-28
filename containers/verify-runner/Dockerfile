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
# required for the registry HTTPS fetch. `/tmp` ships writable in the base image; the runner streams
# request bodies there.
COPY --from=build /src/containers/verify-runner/target/release/dry-verify-runner /usr/local/bin/dry-verify-runner

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/dry-verify-runner"]
