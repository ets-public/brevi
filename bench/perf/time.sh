#!/bin/bash
# Speed benchmark: builds each program and prints the best of 3 wall times per language.
# usage: time.sh [AXON_BINARY]   (run gen.py once first to make the inputs)
cd "$(dirname "$0")"
A=${1:-../../target/release/axon}
for t in words graph sieve records; do
  $A build $t.ax -o bin_$t --release || exit 1
  rustc -O -o rs_$t $t.rs || exit 1
done
t(){ best=999; for k in 1 2 3; do s=$(date +%s.%N); "$@" < $in > out.$lang.$task 2>&1; e=$(date +%s.%N); x=$(echo "$e - $s" | bc); best=$(echo "if ($x < $best) $x else $best" | bc); done; echo $best; }
for task in words graph sieve records; do
  in=$task.in
  lang=py; p=$(t python3 $task.py); lang=rs; r=$(t ./rs_$task); lang=ax; a=$(t ./bin_$task)
  cmp -s out.py.$task out.ax.$task && cmp -s out.py.$task out.rs.$task && ok=same || ok=DIFF
  printf "%-8s python %6.3f  rust %6.3f  axon %6.3f  %s\n" $task $p $r $a $ok
done
