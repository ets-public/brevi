# Brevi compiler with the Rust toolchain it needs. Build: docker build -t brevi .
# Use: docker run --rm -i -v "$PWD:/w" -w /w brevi run main.bv < input.txt
# MCP server: docker run --rm -i brevi mcp
# The Rust version CI tests with (.github/rust-version), pinned by digest: toolchains.yml checks that both FROM
# lines name that version and that the digest is that image (its RUST_VERSION), and Dependabot's docker updates
# move tag and digest together.
FROM rust:1.100.0-slim@sha256:6f14b2b4b278ec0d50aa1d69541c304ce0f95e0f6268487aa9924af1c2e9aebe AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY BREVI.md REFERENCE.md LIBRARIES.md ./
# Also built into brevi (include_str!): the GUI runtime sources and the pinned Rust version.
COPY runtime/brevi_tk runtime/brevi_tk
COPY .github/rust-version .github/rust-version
RUN cargo build --release --locked

FROM rust:1.100.0-slim@sha256:6f14b2b4b278ec0d50aa1d69541c304ce0f95e0f6268487aa9924af1c2e9aebe
COPY --from=build /src/target/release/brevi /usr/local/bin/brevi
# Runs as an unprivileged user, never root. cargo's registry and lock files go in that user's home; the
# toolchain in /usr/local stays read-only.
RUN useradd --system --create-home --shell /bin/sh brevi
USER brevi
ENV CARGO_HOME=/home/brevi/.cargo
ENTRYPOINT ["brevi"]
