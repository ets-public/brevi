#!/bin/sh
# Installs the brevi compiler: fetches the Rust toolchain (needed for rustc) if it is missing, then builds brevi.
# usage: sh install.sh
set -e
if ! command -v cargo >/dev/null 2>&1; then
  echo "installing the Rust toolchain (rustup)"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
fi
cargo install --git https://github.com/ets-public/brevi brevi
brevi --version
