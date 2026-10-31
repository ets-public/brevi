#!/bin/bash
# usage: guide.sh RUN WORD   Prints `brevi guide WORD` for a test subject and logs what it read,
# so guide lookups count as tokens read (task "_guide", no code) in that run's totals.
B=$(dirname "$(readlink -f "$0")")
BREVI=${BREVI:-$B/../target/release/brevi}
run=$1; shift
out=$($BREVI guide "$@" 2>&1)
echo "$out"
ot=$(printf '%s' "$out" | python3 $B/tokcount.py)
echo "{\"run\":\"$run\",\"lang\":\"brevi\",\"task\":\"_guide\",\"word\":\"$*\",\"code_tok\":0,\"patch_tok\":0,\"out_tok\":$ot,\"compiled\":true,\"pass\":0,\"total\":0,\"tok\":\"exact\"}" >> $B/logs/$run.jsonl
