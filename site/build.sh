#!/bin/bash
# Builds the files brevi-lang.org serves for agents (llms.txt, llms-full.txt, .md pages) into site/out.
set -e
S=$(dirname "$(readlink -f "$0")"); R=$S/..; O=$S/out
mkdir -p $O
cp $S/llms.txt $O/llms.txt
cp $R/BREVI.md $O/guide.md
sed '1,/^---$/{/^---$/!d};1,/^---$/d' $R/skills/brevi/SKILL.md > $O/skill.md
sed -n '/^## Status/,$p' $R/README.md > $O/bench.md
{ cat $S/llms.txt; echo; cat $O/guide.md; echo; cat $O/skill.md; } > $O/llms-full.txt
echo "built $(ls $O | wc -l) files in $O"
