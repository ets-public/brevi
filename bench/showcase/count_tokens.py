# Count the tokens a folder of source would cost a model, as ttok and files-to-prompt do: walk the paths given,
# skip hidden and build folders, read each text file, and count it with tiktoken. Prints the largest files and
# the total.
#
#   python3 bench/showcase/count_tokens.py src site/docs --ext .rs .py .md --top 10
import argparse
import os

import tiktoken

SKIP = {"__pycache__", "node_modules", "target", "venv", ".venv", "dist", "build"}


def files_under(paths, exts):
    for p in paths:
        if os.path.isfile(p):
            yield p
            continue
        for top, dirs, names in os.walk(p):
            dirs[:] = sorted(d for d in dirs if not d.startswith(".") and d not in SKIP)
            for n in sorted(names):
                if n.startswith("."):
                    continue
                if exts and os.path.splitext(n)[1] not in exts:
                    continue
                yield os.path.join(top, n)


def main():
    ap = argparse.ArgumentParser(description="Count the tokens in files, as a model would read them.")
    ap.add_argument("paths", nargs="+", help="files or folders")
    ap.add_argument("--ext", nargs="*", default=[], help="only files with these extensions (.py .md ...)")
    ap.add_argument("--encoding", default="o200k_base", help="tiktoken encoding (default o200k_base)")
    ap.add_argument("--top", type=int, default=10, help="how many of the largest files to list")
    args = ap.parse_args()
    enc = tiktoken.get_encoding(args.encoding)
    counts = []
    skipped = 0
    for path in files_under(args.paths, set(args.ext)):
        try:
            with open(path, encoding="utf-8") as f:
                text = f.read()
        except UnicodeDecodeError:
            skipped += 1
            continue
        counts.append((len(enc.encode(text, disallowed_special=())), len(text), path))
    counts.sort(key=lambda c: (-c[0], c[2]))
    for tokens, chars, path in counts[: args.top]:
        print("%9d tokens %9d chars  %s" % (tokens, chars, path))
    total = sum(c[0] for c in counts)
    chars = sum(c[1] for c in counts)
    print("%d files, %d tokens, %d chars (%.2f chars a token), %d binary files skipped" % (len(counts), total, chars, chars / max(total, 1), skipped))


main()
