#!/bin/sh
# The Brevi dev container feature: the release binary (checked against its SHA256SUMS) in /usr/local/bin,
# brevi-python beside it, and the minimal Rust toolchain in /usr/local/cargo when rustc is missing.
# Uses the same install.sh as everyone else.
set -e
VERSION="${VERSION:-latest}"
need() { command -v "$1" >/dev/null 2>&1; }
if ! need curl || ! need cc; then
  if need apt-get; then
    apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates curl gcc libc6-dev
    rm -rf /var/lib/apt/lists/*
  elif need dnf; then dnf install -y ca-certificates curl gcc glibc-devel
  else echo "brevi feature: install curl and a C compiler first" >&2; exit 1
  fi
fi
export RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo PATH="/usr/local/cargo/bin:$PATH"
if ! need rustc; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path
  chmod -R a+rwX /usr/local/rustup /usr/local/cargo
fi
[ "$VERSION" = latest ] && VERSION=
curl -fsSL https://raw.githubusercontent.com/ets-public/brevi/main/install.sh -o /tmp/brevi-install.sh
HOME=/tmp/brevi-home BREVI_VERSION="$VERSION" sh /tmp/brevi-install.sh
install -m 755 /tmp/brevi-home/.cargo/bin/brevi /usr/local/bin/brevi
ln -sf brevi /usr/local/bin/brevi-python
rm -rf /tmp/brevi-home /tmp/brevi-install.sh
brevi --version
