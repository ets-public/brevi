#!/bin/sh
# Installs the brevi compiler. brevi turns programs into Rust, so it needs rustc: the Rust toolchain is
# fetched first if it is missing. Then a prebuilt brevi is downloaded from the latest GitHub release when
# one exists for this machine and checked against the release's SHA256SUMS, or brevi is built from source.
# It also links brevi-python, the brevi binary acting as python3 (native when Brevi can build the program).
# usage: sh install.sh            (BREVI_FROM_SOURCE=1 always builds from source;
#                                  BREVI_VERSION=0.27.0 installs that release instead of the latest)
set -e
# Stops with nothing installed: a download that cannot be verified is never unpacked.
die() {
  printf 'brevi was not installed: %s\nBREVI_FROM_SOURCE=1 builds from source instead.\n' "$1" >&2
  rm -rf "$tmp"; exit 1
}
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
archive="brevi-$target.tar.gz"
release="https://github.com/ets-public/brevi/releases/latest/download"
[ -n "$BREVI_VERSION" ] && release="https://github.com/ets-public/brevi/releases/download/v${BREVI_VERSION#v}"
tmp=$(mktemp -d)
if [ -z "$BREVI_FROM_SOURCE" ] && [ -n "$target" ] && curl -fsSL "$release/$archive" -o "$tmp/b.tgz" 2>/dev/null; then
  curl -fsSL "$release/SHA256SUMS" -o "$tmp/SHA256SUMS" 2>/dev/null ||
    die "could not download SHA256SUMS to verify $archive"
  if command -v sha256sum >/dev/null 2>&1; then actual=$(sha256sum <"$tmp/b.tgz")
  elif command -v shasum >/dev/null 2>&1; then actual=$(shasum -a 256 <"$tmp/b.tgz")
  else die "need sha256sum or shasum to verify $archive"
  fi
  actual=${actual%% *}
  expected=$(awk -v f="$archive" '$2 == f || $2 == "*" f { print $1; exit }' "$tmp/SHA256SUMS")
  [ -n "$expected" ] || die "SHA256SUMS has no entry for $archive"
  [ "$actual" = "$expected" ] || die "checksum mismatch for $archive (expected $expected, got $actual)"
  tar xzf "$tmp/b.tgz" -C "$tmp"
  cp "$tmp/brevi-$target/brevi" "$dest/brevi"
  echo "installed prebuilt brevi to $dest"
else
  [ -n "$BREVI_VERSION" ] && tag="--tag v${BREVI_VERSION#v}"
  cargo install --git https://github.com/ets-public/brevi $tag brevi
fi
rm -rf "$tmp"
ln -sf brevi "$dest/brevi-python"
"$dest/brevi" --version
