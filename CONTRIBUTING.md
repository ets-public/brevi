# Contributing to Brevi

Brevi is in public preview. The most useful thing you can send is a program that Brevi gets wrong.

## Reporting a bug

Open an issue with the "Bug report" template and include:

- the smallest Python (or `.bv`) file that shows the problem, and its input if it reads stdin;
- what `python3 file.py` prints and what `brevi run file.py` prints (or the build error);
- `brevi --version`, `rustc --version` and your OS.

A program where Brevi prints a different answer from CPython is the most serious kind of bug: the aim is the same output as CPython or a clear refusal, never a quietly different answer.

## Pull requests

1. Build with `cargo build --release` (needs `rustc` on the PATH).
2. Add a test: a program and its expected output under `tests/run/`, or an error case under `tests/errors/`.
3. Run `sh tools/ci_local.sh` before you open the PR. It runs the same checks as CI. CI minutes are limited and every push to an open PR runs CI again, so batch your fixes into one push where you can, or keep the PR as a draft (drafts skip CI) until it's ready.
4. Keep each PR to one change, and add your changelog entry as a new file in [changelog.d/](changelog.d/README.md) (for example `changelog.d/my-change.md`, one `- ` line in plain words) rather than editing CHANGELOG.md, so PRs don't conflict on it. Merged changes ship in the next weekly release (Mondays). Your entry ends with your PR and `thanks @your-handle`, and each release's notes list everyone who contributed to it.

[AGENTS.md](AGENTS.md) explains how the compiler is laid out and the rules for working on it, and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) shows how a file becomes a native program. Never remove a line from `compat/baseline.txt` to make a check pass.

By sending a contribution you agree that it is licensed under the [Apache License 2.0](LICENSE), the same as the rest of Brevi.

## Keeping up with upstream

Brevi follows Rust, Python, the MCP specification and the Python libraries it mirrors natively (numpy, requests,
httpx, pydantic, tiktoken, tokenizers, the anthropic and openai SDKs). Bots open the work as issues and PRs:

- Dependabot (`.github/dependabot.yml`): Cargo, GitHub Actions, CI's Python tools and the Docker base image, one
  grouped PR per ecosystem each Saturday.
- `toolchains.yml` (daily): Rust stable and the newest Python; Rust beta and nightly on Mondays.
- `upstream.yml` (Monday, Thursday, Saturday): new Rust, Python, crate and MCP specification releases
  (`tools/upstream_watch.py`); new releases of the mirrored libraries, with a CPython run of their tests on the
  pinned and the new version (`tools/parity_drift.py`, pins in `tools/upstream_pins.json`); OSV security
  advisories for all of it (`tools/advisories.py`). Each finding becomes an `upstream` or `security` issue.

When you update a mirrored library, bump its pin in `tools/upstream_pins.json` together with its `.expected` files.

## Security

Please do not open a public issue for a security problem. See [SECURITY.md](SECURITY.md).
