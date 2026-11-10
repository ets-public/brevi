# Changelog fragments

Each PR adds its changelog entry here as a new file instead of editing CHANGELOG.md, so PRs never
conflict on the changelog and don't have to rebase and rerun CI for it.

- Name the file after the change, e.g. `changelog.d/faster-dict-iteration.md` (any name ending in `.md`;
  entries are listed in file-name order).
- Write it exactly as a CHANGELOG line: one or more lines starting with `- `, in plain words. An outside
  contributor's entry ends with the PR and `thanks @handle`.
- Edit CHANGELOG.md itself only to fix a released section.

The Unreleased section on the site, the release notes and `tools/credits.py` read CHANGELOG.md plus these
files (`python3 tools/changelog.py` prints the result). The release PR runs
`python3 tools/changelog.py --release vX.Y.Z`, which moves every fragment into CHANGELOG.md under the new
version's heading and deletes the files.
