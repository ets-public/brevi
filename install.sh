#!/bin/sh
# Installs the brevi compiler. brevi turns programs into Rust, so it needs rustc: the Rust toolchain is
# fetched first if it is missing. Then a prebuilt brevi is downloaded from the latest GitHub release when
# one exists for this machine, or brevi is built from source.
# usage: sh install.sh            (BREVI_FROM_SOURCE=1 always builds from source)
set -e
if ! command -v rustc >/dev/null 2>&1; then
  echo "installing the Rust toolchain (rustup)"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
fi
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=x86_64-unknown-linux-gnu ;;
  Linux-aarch64|Linux-arm64) target=aarch64-unknown-linux-gnu ;;
  Darwin-arm64) target=aarch64-apple-darwin ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  *) target= ;;
esac
dest="$HOME/.cargo/bin"; mkdir -p "$dest"
url="https://github.com/ets-public/brevi/releases/latest/download/brevi-$target.tar.gz"
tmp=$(mktemp -d)
if [ -z "$BREVI_FROM_SOURCE" ] && [ -n "$target" ] && curl -fsSL "$url" -o "$tmp/b.tgz" 2>/dev/null; then
  tar xzf "$tmp/b.tgz" -C "$tmp"
  cp "$tmp/brevi-$target/brevi" "$dest/brevi"
  echo "installed prebuilt brevi to $dest"
else
  cargo install --git https://github.com/ets-public/brevi brevi
fi
rm -rf "$tmp"
"$dest/brevi" --version
