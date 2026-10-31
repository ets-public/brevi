#!/bin/bash
# usage: run.sh LANG TASK FILE   (LANG = brevi | python | rust; axon is the old name)
# Token counts are exact (tokcount.py calls the count_tokens endpoint; needs BENCH_ANTHROPIC_KEY).
B=$(dirname "$(readlink -f "$0")")
BREVI=${BREVI:-$B/../target/release/brevi}
lang=$1; task=$2; file=$(readlink -f "$3")
[ -d "$B/tasks/$task" ] || { echo "unknown task $task"; exit 2; }
dir=$(dirname "$file"); run=$(basename "$dir")
bin=$(mktemp -d)/prog
out=$(
  case $lang in
    brevi|axon) $BREVI build "$file" -o "$bin" 2>&1 ;;
    rust) rustc --edition 2021 -A warnings -o "$bin" "$file" 2>&1 ;;
    python) python3 -m py_compile "$file" 2>&1 ;;
  esac
)
ok=$?
pass=0; total=0
if [ $ok -eq 0 ]; then
  out=""
  for t in $B/tasks/$task/tests/*.in; do
    total=$((total+1)); exp="${t%.in}.out"
    if [ $lang = python ]; then got=$(timeout 10 python3 "$file" < "$t" 2>&1); else got=$(timeout 10 "$bin" < "$t" 2>&1); fi
    if [ "$got" == "$(cat "$exp")" ]; then pass=$((pass+1)); elif [ -z "$fail_shown" ]; then
      fail_shown=1
      out="test $(basename "$t" .in) failed
input:
$(cat "$t")
expected:
$(cat "$exp")
got:
$got"
    fi
  done
  [ $pass -eq $total ] && out="PASS $pass/$total" || out="$out
passed $pass/$total"
fi
echo "$out"
ct=$(python3 $B/tokcount.py < "$file"); ot=$(printf '%s' "$out" | python3 $B/tokcount.py)
# A retry's patch_tok counts only the lines changed since the last check; the first check counts the whole file.
n=$(grep -c "\"task\":\"$task\"" $B/logs/$run.jsonl 2>/dev/null); prev=$B/logs/attempts/$run/$task.$n.src
pt=$([ -f "$prev" ] && python3 $B/patchtok.py "$prev" "$file" || echo $ct)
echo "{\"run\":\"$run\",\"lang\":\"$lang\",\"task\":\"$task\",\"code_tok\":$ct,\"patch_tok\":$pt,\"out_tok\":$ot,\"compiled\":$([ $ok -eq 0 ] && echo true || echo false),\"pass\":$pass,\"total\":$total,\"tok\":\"exact\"}" >> $B/logs/$run.jsonl
# Keep each attempt's code and output for later study.
n=$(grep -c "\"task\":\"$task\"" $B/logs/$run.jsonl); a=$B/logs/attempts/$run/$task.$n; mkdir -p $(dirname $a)
cp "$file" $a.src; printf '%s\n' "$out" > $a.out
