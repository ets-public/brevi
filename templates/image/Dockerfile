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
# Commands may run as any user (E2B runs them as `user`, Daytona as `daytona`), so the toolchain lives in /opt
# (read-only). Each user keeps a build cache of their own (~/.cache/brevi): Brevi runs and links what a cache
# holds, so it never uses one another user could change. The runtime libraries are built once here, as root,
# into the read-only seed /opt/brevi-seed (BREVI_CACHE_SEED), which every user's cache links from. cargo's own
# files go in /opt/brevi-cache/cargo (writable by everyone). Run as is, the image runs as the unprivileged
# `brevi` user, never root.
#
# The base image is pinned by digest (Dependabot's docker updates keep it current), and Brevi, this file's
# install.sh and the SDK all come from one release tag.
FROM debian:trixie-slim@sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2

ARG BREVI_VERSION=
ARG PYTHON3_SHIM=0
LABEL org.opencontainers.image.source=https://github.com/ets-public/brevi \
      org.opencontainers.image.description="Brevi: AI-written Python built natively and sandboxed" \
      org.opencontainers.image.licenses=Apache-2.0

ENV RUSTUP_HOME=/opt/rust CARGO_HOME=/opt/rust BREVI_CACHE_SEED=/opt/brevi-seed BREVI_PYTHON=/usr/bin/python3 \
    PATH=/opt/rust/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl gcc libc6-dev libssl3t64 python3 \
 && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup.sh \
 && sh /tmp/rustup.sh -y --profile minimal --no-modify-path && rm /tmp/rustup.sh \
 && rustc --version

# Brevi, brevi-python beside it, and the SDK, all from one release: BREVI_VERSION, or the latest release
# (releases/latest redirects to releases/tag/vX.Y.Z), never main.
RUN if [ -n "$BREVI_VERSION" ]; then ref="v${BREVI_VERSION#v}"; \
    else ref=$(curl -fsSLI -o /dev/null -w '%{url_effective}' https://github.com/ets-public/brevi/releases/latest); ref=${ref##*/}; fi \
 && case "$ref" in v[0-9]*.[0-9]*) ;; *) echo "no Brevi release found (got '$ref')" >&2; exit 1 ;; esac \
 && curl -fsSL "https://raw.githubusercontent.com/ets-public/brevi/$ref/install.sh" -o /tmp/install.sh \
 && HOME=/tmp/home BREVI_VERSION="$ref" sh /tmp/install.sh && install -m 755 /tmp/home/.cargo/bin/brevi /usr/local/bin/brevi \
 && ln -sf brevi /usr/local/bin/brevi-python && rm -rf /tmp/home /tmp/install.sh && brevi --version \
 && sdk="$(python3 -c 'import sysconfig; print(sysconfig.get_path("purelib"))')/brevi" && mkdir -p "$sdk" \
 && curl -fsSL "https://raw.githubusercontent.com/ets-public/brevi/$ref/packaging/pypi/brevi/__init__.py" -o "$sdk/__init__.py" \
 && python3 -c 'import brevi'

# Build the runtime libraries once into the seed, so the first program an agent runs does not pay for building
# them: the typed one, and the dynamic one (about 50 s cold), which `brevi mcp` and the SDK also build when they
# start. Only the libraries stay; the seed is root's and read-only to everyone else.
RUN mkdir -p /tmp/warm \
 && printf 'import sys\nprint(sum(int(x) for x in sys.stdin.read().split()))\n' > /tmp/warm/main.py \
 && echo 1 2 3 | BREVI_CACHE=/opt/brevi-seed brevi run --sandbox /tmp/warm/main.py \
 && BREVI_DYN=1 BREVI_CACHE=/opt/brevi-seed brevi build /tmp/warm/main.py -o /tmp/warm/dyn \
 && rm -rf /tmp/warm /root/.cache/brevi && find /opt/brevi-seed -mindepth 1 -maxdepth 1 ! -name rt ! -name dynrt -exec rm -rf {} + \
 && chmod -R go-w /opt/brevi-seed && ls /opt/brevi-seed/rt /opt/brevi-seed/dynrt \
 && mkdir -p /workspace && chmod 1777 /workspace

# The python3 variant: brevi-python first on PATH as python3. Scripts that name /usr/bin/python3 still get CPython.
RUN if [ "$PYTHON3_SHIM" = 1 ]; then \
      ln -s brevi /usr/local/bin/python3 && ln -s /usr/bin/python3 /usr/local/bin/python3.real \
      && [ "$(echo 'print(6 * 7)' | python3)" = 42 ]; fi

# cargo keeps its registry and lock files in CARGO_HOME; the toolchain stays in /opt/rust, read-only.
ENV CARGO_HOME=/opt/brevi-cache/cargo
RUN useradd --system --create-home --shell /bin/sh brevi && mkdir -p "$CARGO_HOME" && chmod a+rwX "$CARGO_HOME"
USER brevi
WORKDIR /workspace
