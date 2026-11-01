"""Prints the GitHub release notes for a tag: its section of CHANGELOG.md plus the install note.
usage: python3 tools/release_notes.py v0.18.0"""
import os, re, sys

tag = sys.argv[1]
text = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "CHANGELOG.md")).read()
# A section starts at "## v0.18.0 (date)" and runs to the next "## ".
m = re.search(r"^## " + re.escape(tag) + r"\b[^\n]*\n(.*?)(?=^## |\Z)", text, re.S | re.M)
body = m.group(1).strip() if m else "See CHANGELOG.md."
print(body)
print()
print("Prebuilt brevi compiler for Linux, macOS and Windows. brevi still needs rustc on the PATH to build programs; "
      "`sh install.sh` fetches it and picks the right download.")
