#!/bin/bash
# Builds brevi-lang.org into site/out (home page, docs section, and the files it serves for agents:
# llms.txt, llms-full.txt, .md pages) and brevi.run into site/out-run. Pages are made by site/gen.py.
set -e
S=$(dirname "$(readlink -f "$0")"); R=$S/..; O=$S/out
rm -rf $O $S/out-run; mkdir -p $O $S/out-run
# The version shown in the header comes from Cargo.toml (major.minor), so it cannot go stale.
V=$(sed -n 's/^version = "\([0-9]*\.[0-9]*\).*/\1/p' $R/Cargo.toml | head -1)
FV=$(sed -n 's/^version = "\([0-9.]*\)".*/\1/p' $R/Cargo.toml | head -1)
[ -n "$V" ] && [ -n "$FV" ] || { echo "no version in Cargo.toml"; exit 1; }
# The guide's size is counted from BREVI.md (exact count, cached in bench/tokcache.json; a new text
# needs BENCH_ANTHROPIC_KEY once).
G=$(python3 -c 'import sys; sys.path.insert(0, sys.argv[1]); import tokcount; print(f"{tokcount.count(open(sys.argv[2]).read()):,}")' $R/bench $R/BREVI.md) \
  || { echo "could not count BREVI.md tokens (set BENCH_ANTHROPIC_KEY)"; exit 1; }
# The repo is public, so the site is always built with its GitHub links, the install one-liner and the
# open-source status line. There is no private mode any more (Adam, 2026-11-09): a deploy built that way
# once shipped the live site with every GitHub link removed.
python3 $S/gen.py $O $S/out-run "$V" "$G" 1 "$FV"
# Logo and favicons (favicon.svg is the source; node site/icons/make.js redraws the PNG and ICO sizes).
for d in $O $S/out-run; do cp $S/icons/*.{svg,ico,png,webmanifest} $d/; done
# Fonts are served from the site itself (IBM Plex, latin subset; the licence goes with them).
for d in $O $S/out-run; do cp $S/fonts/*.woff2 $S/fonts/OFL.txt $d/; done
cp $S/llms.txt $O/llms.txt
cp $R/BREVI.md $O/guide.md
cp $R/LIBRARIES.md $O/libraries.md
cp $R/REFERENCE.md $O/reference.md
sed '1,/^---$/{/^---$/!d};1,/^---$/d' $R/skills/brevi/SKILL.md > $O/skill.md
sed -n '/^## Status/,$p' $R/README.md > $O/bench.md
{ cat $O/llms.txt; echo; cat $O/guide.md; echo; cat $O/reference.md; echo; cat $O/libraries.md; echo; cat $O/skill.md; } > $O/llms-full.txt
# sitemap.xml and robots.txt for search engines (Search Console reads the sitemap): one clean address
# per page, as deploy.sh serves them (docs/usage.html is /docs/usage, an index.html is its folder).
for pair in "$O https://brevi-lang.org" "$S/out-run https://brevi.run"; do set -- $pair
  { echo '<?xml version="1.0" encoding="UTF-8"?>'; echo '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">'
    (cd $1 && find . -name '*.html' ! -name 404.html | sed 's|^\.||; s|index\.html$||; s|\.html$||' | sort) | sed "s|.*|  <url><loc>$2&</loc></url>|"
    echo '</urlset>'; } > $1/sitemap.xml
  printf 'User-agent: *\nAllow: /\n\nSitemap: %s/sitemap.xml\n' "$2" > $1/robots.txt
done
echo "built $(find $O -type f | wc -l) files in $O, $(find $S/out-run -type f | wc -l) in $S/out-run"
