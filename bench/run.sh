#!/bin/bash
# usage: run.sh LANG TASK FILE   (LANG = axon | python | rust)
B=$(cd "$(dirname "$0")" && pwd)
lang=$1; task=$2; file=$(readlink -f "$3")
[ -d "$B/tasks/$task" ] || { echo "unknown task $task"; exit 2; }
dir=$(dirname "$file"); run=$(basename "$dir")
bin=$(mktemp -d)/prog
out=$(
  case $lang in
    axon) ${AXON:-axon} build "$file" -o "$bin" 2>&1 ;;
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
echo "{\"run\":\"$run\",\"lang\":\"$lang\",\"task\":\"$task\",\"code_tok\":$ct,\"out_tok\":$ot,\"compiled\":$([ $ok -eq 0 ] && echo true || echo false),\"pass\":$pass,\"total\":$total}" >> $B/logs/$run.jsonl
