#!/bin/sh
# Checks a built sandbox image (templates/image) before it is pushed or a release is published (release.yml):
#   sh tools/image_test.sh IMAGE [PYTHON3_IMAGE]
# A program runs sandboxed, the SDK answers, and a crate program builds and runs as the image's own user with
# cargo's files in that user's ~/.cargo. Then, as root in a fresh container: after the brevi user has built a
# crate program, a second user can write to none of the brevi user's cargo home or build cache, the runtime seed
# or the toolchain (Brevi runs what those hold, so no other user may change them).
set -e
img=$1; py3=$2
sum=ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
here=$(dirname "$0")/..
docker run --rm "$img" sh -c 'echo "print(sum(range(10)))" > m.py && brevi run --sandbox m.py' | grep -qx 45
docker run --rm "$img" python3 -c 'import brevi; print(brevi.run("print(6 * 7)").stdout, end="")' | grep -qx 42
[ -z "$py3" ] || docker run --rm "$py3" python3 -c 'print(sum(range(10)))' | grep -qx 45
[ "$(docker run --rm "$img" id -un)" = brevi ]
docker run --rm -i "$img" sh -c 'cat > c.bv && brevi run c.bv' < "$here/tests/run/crate_use.bv" | grep -qx $sum
docker run --rm -i -u root -e sum=$sum "$img" sh -ec '
  cat > /workspace/c.bv; chown brevi /workspace/c.bv
  su brevi -c "PATH=$PATH; cd /workspace && brevi run c.bv" | grep -qx "$sum"
  useradd -m other
  for d in /home/brevi/.cargo /home/brevi/.cache/brevi /opt/brevi-seed /opt/rust; do
    test -d "$d" || { echo "missing $d" >&2; exit 1; }
    if su other -c "touch $d/x" 2>/dev/null; then echo "$d is writable by another user" >&2; exit 1; fi
  done
  w=$(find /home/brevi/.cargo /home/brevi/.cache/brevi /opt/brevi-seed /opt/rust ! -type l -perm -o+w | head -5)
  [ -z "$w" ] || { echo "writable by other users: $w" >&2; exit 1; }
  echo ok' < "$here/tests/run/crate_use.bv" | grep -qx ok
echo "image ok: $img"
