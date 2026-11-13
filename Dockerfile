# Brevi compiler with the Rust toolchain it needs. Build: docker build -t brevi .
# Use: docker run --rm -i -v "$PWD:/w" -w /w brevi run main.bv < input.txt
# MCP server: docker run --rm -i brevi mcp
# The Rust version CI tests with (.github/rust-version; toolchains.yml checks the two agree). A newer rustc's
# Unicode tables can change what Brevi accepts (re.IGNORECASE is refused on tables not checked against CPython).
ARG RUST_VERSION=1.99.1
FROM rust:${RUST_VERSION}-slim AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY BREVI.md REFERENCE.md LIBRARIES.md ./
RUN cargo build --release

FROM rust:${RUST_VERSION}-slim
COPY --from=build /src/target/release/brevi /usr/local/bin/brevi
ENTRYPOINT ["brevi"]
