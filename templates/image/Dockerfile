# The image every agent platform template runs: Brevi, the Rust toolchain it builds with, and python3
# (for `brevi verify` and for libraries CPython runs when the caller grants `native`).
#
#   docker build -t brevi-sandbox templates/image
#
# It installs the latest Brevi release, checked against the release's SHA256SUMS. The file has no COPY
# lines, so the E2B, Daytona and Modal SDKs can build it from the text alone.
#
# Commands may run as any user (E2B runs them as `user`, Daytona as `daytona`), so the toolchain and the
# build cache live in /opt and are writable by everyone.
FROM debian:trixie-slim

ENV RUSTUP_HOME=/opt/rust CARGO_HOME=/opt/rust BREVI_CACHE=/opt/brevi-cache \
    PATH=/opt/rust/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl gcc libc6-dev libssl3t64 python3 \
 && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup.sh \
 && sh /tmp/rustup.sh -y --profile minimal --no-modify-path && rm /tmp/rustup.sh \
 && rustc --version

RUN curl -fsSL https://raw.githubusercontent.com/ets-public/brevi/main/install.sh -o /tmp/install.sh \
 && HOME=/tmp/home sh /tmp/install.sh && install -m 755 /tmp/home/.cargo/bin/brevi /usr/local/bin/brevi \
 && rm -rf /tmp/home /tmp/install.sh && brevi --version

# Warm the build cache once, so the first program an agent runs does not pay for the toolchain lookups.
RUN mkdir -p /tmp/warm "$BREVI_CACHE" \
 && printf 'import sys\nprint(sum(int(x) for x in sys.stdin.read().split()))\n' > /tmp/warm/main.py \
 && echo 1 2 3 | brevi run --sandbox /tmp/warm/main.py \
 && rm -rf /tmp/warm && chmod -R a+rwX "$BREVI_CACHE" \
 && mkdir -p /workspace && chmod 1777 /workspace

WORKDIR /workspace
