#!/bin/bash -eu
# Builds every target in fuzz/ (docs/SECURITY_TESTING.md) and copies it where OSS-Fuzz runs it.
cd $SRC/brevi
cargo fuzz build -O --debug-assertions
for t in $(cargo fuzz list); do
  cp fuzz/target/x86_64-unknown-linux-gnu/release/$t $OUT/
done
