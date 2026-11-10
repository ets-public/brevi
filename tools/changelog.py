"""Changelog fragments: each PR adds its entry as its own file in changelog.d/ instead of editing CHANGELOG.md,
so PRs never conflict on the changelog. A fragment is one or more "- " lines, written as a CHANGELOG entry.
The Unreleased section is CHANGELOG.md's "## Unreleased" lines followed by every fragment, in file-name order.

usage:
  python3 tools/changelog.py            print the Unreleased entries (CHANGELOG.md's plus the fragments)
  python3 tools/changelog.py --check    exit 1 if a fragment is empty or not a "- " list
  python3 tools/changelog.py --release vX.Y.Z [DATE]   (DATE like "16 Nov 2026", default today)
                                        for the release PR: move every fragment into CHANGELOG.md, rename
                                        "## Unreleased" to "## vX.Y.Z (DATE)" under a new empty Unreleased,
                                        and delete the fragment files
"""
import datetime, os, re, sys

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
DIR = os.path.join(ROOT, "changelog.d")
UNRELEASED = re.compile(r"^## Unreleased\n(.*?)(?=^## |\Z)", re.S | re.M)


def fragment_files():
    if not os.path.isdir(DIR):
        return []
    return sorted(os.path.join(DIR, f) for f in os.listdir(DIR) if f.endswith(".md") and f != "README.md")


def fragments():
    """The fragments' entries, one string per file, in file-name order."""
    return [open(p).read().strip() for p in fragment_files()]


def with_fragments(text):
    """CHANGELOG.md's text with the fragments appended to its Unreleased section."""
    frags = [f for f in fragments() if f]
    if not frags:
        return text
    m = UNRELEASED.search(text)
    if not m:
        return text
    body = m.group(1).rstrip("\n")
    body = (body + "\n" if body.strip() else "\n") + "\n".join(frags) + "\n\n"
    return text[:m.start(1)] + body + text[m.end(1):]


def changelog():
    """CHANGELOG.md as it reads with the fragments in: what the site and the release tools use."""
    return with_fragments(open(os.path.join(ROOT, "CHANGELOG.md")).read())


def unreleased():
    m = UNRELEASED.search(changelog())
    return m.group(1) if m else ""


def main(argv):
    if not argv:
        print(unreleased().strip())
        return 0
    if argv[0] == "--check":
        bad = [p for p, f in zip(fragment_files(), fragments()) if not f.startswith("- ")]
        for p in bad:
            print(f"{os.path.relpath(p, ROOT)}: a fragment is one or more CHANGELOG lines starting with '- '")
        return 1 if bad else 0
    if argv[0] == "--release" and len(argv) >= 2:
        tag = argv[1]
        d = datetime.date.today()
        date = argv[2] if len(argv) > 2 else f"{d.day} {d.strftime('%b %Y')}"
        text = changelog().replace("## Unreleased\n", f"## Unreleased\n\n## {tag} ({date})\n", 1)
        open(os.path.join(ROOT, "CHANGELOG.md"), "w").write(re.sub(r"\n{3,}", "\n\n", text))
        for p in fragment_files():
            os.remove(p)
        print(f"CHANGELOG.md: Unreleased is now {tag} ({date}); fragments removed")
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
