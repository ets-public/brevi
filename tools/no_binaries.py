#!/usr/bin/env python3
"""Fail when git tracks a built program or any other large file.

A `brevi build` without -o writes the program into the current folder, so a build run at the root of the
checkout followed by `git add -A` commits it (that happened once, in #347). CI runs this script on every
PR: a tracked file that is an executable (ELF, Mach-O or Windows PE) or is over 2 MB fails the check.

  python3 tools/no_binaries.py
"""
import subprocess, sys

LIMIT = 2 * 1024 * 1024
MAGIC = (b"\x7fELF", b"MZ", b"\xcf\xfa\xed\xfe", b"\xce\xfa\xed\xfe", b"\xca\xfe\xba\xbe")

bad = []
for f in subprocess.run(["git", "ls-files", "-z"], capture_output=True, check=True).stdout.split(b"\0"):
    if not f:
        continue
    try:
        with open(f, "rb") as h:
            head = h.read(4)
            h.seek(0, 2)
            size = h.tell()
    except OSError:
        continue  # a symlink to a folder, or deleted in the work tree
    name = f.decode(errors="replace")
    if head.startswith(MAGIC):
        bad.append(f"{name}: a built program ({size // 1024} KB)")
    elif size > LIMIT:
        bad.append(f"{name}: {size // 1024} KB, over the {LIMIT // 1024 // 1024} MB limit")
for b in bad:
    print(b)
if bad:
    print(f"{len(bad)} file(s) should not be in git: remove them with `git rm --cached` (built programs are ignored by .gitignore)")
    sys.exit(1)
print("no built programs or large files tracked")
