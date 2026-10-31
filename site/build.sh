#!/bin/bash
# Builds brevi-lang.org into site/out: the home page plus the files it serves for agents
# (llms.txt, llms-full.txt, .md pages), and the brevi.run holding page into site/out-run.
set -e
S=$(dirname "$(readlink -f "$0")"); R=$S/..; O=$S/out
mkdir -p $O $S/out-run
cp $S/index.html $O/index.html
cp $S/run/index.html $S/out-run/index.html
# Logo and favicons (favicon.svg is the source; node site/icons/make.js redraws the PNG and ICO sizes).
for d in $O $S/out-run; do cp $S/icons/*.{svg,ico,png,webmanifest} $d/; done
# The repo is private until the public release, so leave its link out of the served llms.txt.
grep -v 'github.com/ets-public/brevi' $S/llms.txt | sed '/^## Tools$/{N;/\n$/d}' | cat -s > $O/llms.txt
cp $R/BREVI.md $O/guide.md
cp $R/LIBRARIES.md $O/libraries.md
cp $R/REFERENCE.md $O/reference.md
sed '1,/^---$/{/^---$/!d};1,/^---$/d' $R/skills/brevi/SKILL.md > $O/skill.md
sed -n '/^## Status/,$p' $R/README.md > $O/bench.md
{ cat $O/llms.txt; echo; cat $O/guide.md; echo; cat $O/reference.md; echo; cat $O/libraries.md; echo; cat $O/skill.md; } > $O/llms-full.txt
echo "built $(ls $O | wc -l) files in $O"
