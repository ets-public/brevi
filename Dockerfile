# Brevi compiler with the Rust toolchain it needs. Build: docker build -t brevi .
# Use: docker run --rm -i -v "$PWD:/w" -w /w brevi run main.bv < input.txt
# MCP server: docker run --rm -i brevi mcp
FROM rust:1-slim AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY BREVI.md ./
RUN cargo build --release

FROM rust:1-slim
COPY --from=build /src/target/release/brevi /usr/local/bin/brevi
ENTRYPOINT ["brevi"]
