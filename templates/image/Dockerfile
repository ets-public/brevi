# The image every agent platform template runs: Brevi, the Rust toolchain it builds with, python3 (for
# `brevi verify` and for libraries CPython runs when the caller grants `native`) and the Python SDK
# (`import brevi`).
#
#   docker build -t brevi-sandbox templates/image
#   docker build -t brevi-sandbox:python3 --build-arg PYTHON3_SHIM=1 templates/image
#
# Each release publishes it as ghcr.io/ets-public/brevi-sandbox:VERSION and :latest (release.yml), and the
# PYTHON3_SHIM=1 variant as :VERSION-python3 and :python3, where `python3` is brevi-python (native when Brevi
# can build the program, CPython otherwise) and CPython is /usr/bin/python3 (also python3.real).
#
# It installs the latest Brevi release (or BREVI_VERSION), checked against the release's SHA256SUMS. The file
# has no COPY lines, so the E2B, Daytona and Modal SDKs can build it from the text alone.
#
# Commands may run as any user (E2B runs them as `user`, Daytona as `daytona`), so the toolchain and the
# build cache live in /opt and are writable by everyone.
FROM debian:trixie-slim

ARG BREVI_VERSION=
ARG PYTHON3_SHIM=0
LABEL org.opencontainers.image.source=https://github.com/ets-public/brevi \
      org.opencontainers.image.description="Brevi: AI-written Python built natively and sandboxed" \
      org.opencontainers.image.licenses=Apache-2.0

ENV RUSTUP_HOME=/opt/rust CARGO_HOME=/opt/rust BREVI_CACHE=/opt/brevi-cache BREVI_PYTHON=/usr/bin/python3 \
    PATH=/opt/rust/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl gcc libc6-dev libssl3t64 python3 \
 && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup.sh \
 && sh /tmp/rustup.sh -y --profile minimal --no-modify-path && rm /tmp/rustup.sh \
 && rustc --version

# Brevi, brevi-python beside it, and the SDK from the same release (main for the latest).
RUN if [ -n "$BREVI_VERSION" ]; then ref="v${BREVI_VERSION#v}"; else ref=main; fi \
 && curl -fsSL https://raw.githubusercontent.com/ets-public/brevi/main/install.sh -o /tmp/install.sh \
 && HOME=/tmp/home sh /tmp/install.sh && install -m 755 /tmp/home/.cargo/bin/brevi /usr/local/bin/brevi \
 && ln -sf brevi /usr/local/bin/brevi-python && rm -rf /tmp/home /tmp/install.sh && brevi --version \
 && sdk="$(python3 -c 'import sysconfig; print(sysconfig.get_path("purelib"))')/brevi" && mkdir -p "$sdk" \
 && curl -fsSL "https://raw.githubusercontent.com/ets-public/brevi/$ref/packaging/pypi/brevi/__init__.py" -o "$sdk/__init__.py" \
 && python3 -c 'import brevi'

# Warm the build cache once, so the first program an agent runs does not pay for the toolchain lookups or
# for building Brevi's runtime libraries: the typed one, and the dynamic one (about 50 s cold), which
# `brevi mcp` and the SDK also build when they start.
RUN mkdir -p /tmp/warm "$BREVI_CACHE" \
 && printf 'import sys\nprint(sum(int(x) for x in sys.stdin.read().split()))\n' > /tmp/warm/main.py \
 && echo 1 2 3 | brevi run --sandbox /tmp/warm/main.py \
 && BREVI_DYN=1 brevi build /tmp/warm/main.py -o /tmp/warm/dyn \
 && rm -rf /tmp/warm && chmod -R a+rwX "$BREVI_CACHE" \
 && mkdir -p /workspace && chmod 1777 /workspace

# The python3 variant: brevi-python first on PATH as python3. Scripts that name /usr/bin/python3 still get CPython.
RUN if [ "$PYTHON3_SHIM" = 1 ]; then \
      ln -s brevi /usr/local/bin/python3 && ln -s /usr/bin/python3 /usr/local/bin/python3.real \
      && [ "$(echo 'print(6 * 7)' | python3)" = 42 ]; fi

WORKDIR /workspace
