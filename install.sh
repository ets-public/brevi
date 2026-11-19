#!/bin/sh
# Installs the brevi compiler. A prebuilt brevi is downloaded from the latest GitHub release when one exists
# for this machine and checked against the release's SHA256SUMS, or brevi is built from source. brevi turns
# programs into Rust, so it needs rustc: with none on PATH, `brevi toolchain` fetches Brevi's pinned Rust (and
# zig to link, if there is no C linker) into Brevi's cache, with no Rust install. Where brevi cannot do that
# (a release without `brevi toolchain`, or a machine the pinned toolchain is not verified on yet), rustup installs Rust.
# It also links brevi-python, the brevi binary acting as python3 (native when Brevi can build the program).
# Everything comes from one release tag: the latest release (or BREVI_VERSION), found once at the start, so the
# archive, its SHA256SUMS and a source build all name the same release. When `gh` is installed and signed in,
# the archive's build-provenance attestation is checked too (releases from v0.28.0 have one).
# usage: sh install.sh            (BREVI_FROM_SOURCE=1 always builds that release from source;
#                                  BREVI_VERSION=0.28.3 installs that release instead of the latest)
set -e
# Stops with nothing installed: a download that cannot be verified is never unpacked.
die() {
  printf 'brevi was not installed: %s\n' "$1" >&2
  if [ -n "$tag" ]; then echo "BREVI_FROM_SOURCE=1 builds $tag from source instead." >&2
  else echo "BREVI_VERSION=X.Y.Z installs a named release instead." >&2; fi
  rm -rf "$tmp"; exit 1
}
rustup() {
  echo "installing the Rust toolchain (rustup)"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
}
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=x86_64-unknown-linux-gnu ;;
  Linux-aarch64|Linux-arm64) target=aarch64-unknown-linux-gnu ;;
  Darwin-arm64) target=aarch64-apple-darwin ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  *) target= ;;
esac
dest="$HOME/.cargo/bin"; mkdir -p "$dest"
archive="brevi-$target.tar.gz"
repo=https://github.com/ets-public/brevi
tag=; tmp=$(mktemp -d)
if [ -n "$BREVI_VERSION" ]; then tag="v${BREVI_VERSION#v}"
else
  # releases/latest redirects to releases/tag/vX.Y.Z.
  tag=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$repo/releases/latest" 2>/dev/null) ||
    die "could not find the latest release at $repo/releases/latest"
  tag=${tag##*/}
fi
case "$tag" in v[0-9]*.[0-9]*) ;; *) die "could not tell the latest release from $repo/releases/latest (got '$tag')" ;; esac
release="$repo/releases/download/$tag"
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
  # SHA256SUMS comes from the same place as the archive; the attestation, signed by this repository's release
  # workflow, does not. Checked whenever gh can check it, and a failed check installs nothing.
  v=${tag#v}; minor=${v#*.}; minor=${minor%%.*}
  if command -v gh >/dev/null 2>&1 && gh attestation --help >/dev/null 2>&1 && gh auth status >/dev/null 2>&1 && { [ "${v%%.*}" != 0 ] || [ "$minor" -ge 28 ]; }; then
    gh attestation verify "$tmp/b.tgz" -R ets-public/brevi \
      --signer-workflow ets-public/brevi/.github/workflows/release.yml >/dev/null 2>&1 ||
      die "$archive has no valid build-provenance attestation from ets-public/brevi's release workflow"
    echo "checked the build-provenance attestation of $archive"
  fi
  tar xzf "$tmp/b.tgz" -C "$tmp"
  cp "$tmp/brevi-$target/brevi" "$dest/brevi"
  echo "installed prebuilt brevi to $dest"
else
  # The release tag, never main, and with its Cargo.lock (--locked), so a source build is the release's code
  # with the dependency versions it was tested with.
  command -v cargo >/dev/null 2>&1 || rustup
  cargo install --locked --git "$repo" --tag "$tag" brevi
fi
rm -rf "$tmp"
ln -sf brevi "$dest/brevi-python"
if ! command -v rustc >/dev/null 2>&1 && ! "$dest/brevi" toolchain; then
  rustup
fi
"$dest/brevi" --version
