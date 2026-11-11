# Support and version policy

This page says which Brevi versions get fixes, how fast we answer bug reports, and where to look first. For vulnerabilities, use [SECURITY.md](../SECURITY.md) instead.

## Versions

Brevi is 0.x. The CLI and Brevi's own syntax can change between releases. A new minor release ships most Mondays, after the release test passes. Until then, changes collect under "Unreleased" in [CHANGELOG.md](../CHANGELOG.md).

| Release | What it gets |
| --- | --- |
| Latest minor | Every fix. |
| Previous minor | Fixes for Critical and High security issues only, for 4 weeks after its successor ships. |
| Older | Nothing. |

The upgrade is one command. See [UPGRADING.md](UPGRADING.md).

Security fixes for Critical and High issues ship as a patch release as soon as they are ready, outside the Monday cadence if needed. Medium and Low fixes ship in the next weekly release. The fix targets are in [SECURITY.md](../SECURITY.md).

Bug fixes that are not security fixes go into the latest minor only. If a bug is in a release older than that, the first answer will be to upgrade and try again.

## Reporting a bug

Open an issue with the "Bug report" template and include what [CONTRIBUTING.md](../CONTRIBUTING.md) asks for:

- the smallest Python (or `.bv`) file that shows the problem, and its input if it reads stdin;
- what `python3 file.py` prints and what `brevi run file.py` prints (or the build error);
- `brevi --version`, `rustc --version` and your OS.

Check [KNOWN_ISSUES.md](KNOWN_ISSUES.md) first. A limit listed there is already tracked.

## Triage and response targets

| Kind of report | First reply |
| --- | --- |
| A wrong answer: Brevi prints something different from CPython and does not refuse | within 3 days |
| A crash | within 3 days |
| A sandbox or limit issue (if it lets a program escape, use [SECURITY.md](../SECURITY.md)) | within 3 days |
| Any other bug | within 7 days |

A first reply is not a fix date. It tells you the report was read and what happens next.

Brevi's aim is that a build either prints what CPython prints or is refused. A different answer is therefore the most serious kind of bug, and these are fixed first. A refusal (a clear error at build time, or `brevi run` falling back to `python3`) is a gap, not a wrong answer, and is handled as an ordinary bug. Issues may get labels for the kind of problem; the labels are for sorting and do not change these targets.

## Platforms

- Linux and macOS are the main platforms. Release binaries come for Linux x86-64 and arm64, and macOS arm64 and x86-64.
- **Windows is beta, x86-64 only** (`x86_64-pc-windows-msvc`). Bug reports are welcome and follow the same targets. What it refuses is in [KNOWN_ISSUES.md](KNOWN_ISSUES.md).

## Where to ask

- Bugs and questions about a program: a GitHub issue on [ets-public/brevi](https://github.com/ets-public/brevi/issues).
- Anything private, including security: support@brevi-lang.org with "security" in the subject for vulnerabilities.
- Contributing a fix: [CONTRIBUTING.md](../CONTRIBUTING.md). Merged changes ship in the next weekly release.
