#!/bin/bash
# usage: verify.sh OLD_BREVI [NEW_BREVI]
# Rebuilds every saved Brevi (formerly Axon) solution in solutions/ with both compilers and checks the new one
# prints the same as the old one on every test (only solutions the old compiler builds are checked). Runs in parallel.
B=$(dirname "$(readlink -f "$0")"); export B O=$(readlink -f $1) A=$(readlink -f ${2:-$B/../target/release/brevi})
one() {
  f=$1; t=$(basename ${f%.*}); [ -d $B/tasks/$t ] || exit 0
  T=$(mktemp -d); trap "rm -rf $T" EXIT
  $O build $f -o $T/old >/dev/null 2>&1 || exit 0
  if ! $A build $f -o $T/new >$T/err 2>&1; then echo "BUILD $f $(head -c 300 $T/err)"; exit 0; fi
  for i in $B/tasks/$t/tests/*.in; do a=$(timeout 10 $T/old < $i 2>&1); b=$(timeout 10 $T/new < $i 2>&1); [ "$a" == "$b" ] || { echo "DIFF $f $i"; exit 0; }; done
  echo "OK $f"
}
export -f one
ls $B/solutions/*/*.ax $B/solutions/*/*.bv 2>/dev/null | xargs -P ${JOBS:-8} -I{} bash -c 'one {}' > ${OUT:-/dev/stdout}
