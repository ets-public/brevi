#!/bin/bash
# Rebuild every Axon solution in work/ with the given compiler and rerun its tests (only those that passed before).
A=${1:-/home/claude/axon/target/release/axon}; B=/home/claude/bench; n=0; bad=0
for f in $B/work/*-axon/*.ax; do t=$(basename $f .ax); [ -d $B/tasks/$t ] || continue
  $B/axon build $f -o /tmp/claude-0/t/old >/dev/null 2>&1 || continue
  n=$((n+1))
  if ! $A build $f -o /tmp/claude-0/t/new >/tmp/claude-0/t/err 2>&1; then echo "BUILD $f"; head -c 400 /tmp/claude-0/t/err; echo; bad=$((bad+1)); continue; fi
  for i in $B/tasks/$t/tests/*.in; do a=$(timeout 10 /tmp/claude-0/t/old < $i 2>&1); b=$(timeout 10 /tmp/claude-0/t/new < $i 2>&1); [ "$a" == "$b" ] || { echo "DIFF $f $i"; bad=$((bad+1)); break; }; done
done
echo "checked $n, problems $bad"
