# Packaging

`release.yml` publishes every release to PyPI, npm, crates.io and the MCP registry once the
binaries are built and attached to the GitHub release. It uses trusted publishing (OIDC), so no
registry tokens are stored anywhere: each registry trusts `ets-public/brevi`'s workflow
`release.yml`. Renaming that file means updating the trusted publisher on each registry.
A version that is already on a registry is skipped, so re-running a release is safe. Start a
release by hand with `publish: false` to attach binaries only, or with `from_release: true` to publish
an existing release's binaries without rebuilding (crates.io is skipped then, as it publishes source).

One-time setup (registry owner, in a browser):

| Registry | Where | Values |
| --- | --- | --- |
| PyPI | https://pypi.org/manage/project/brevi/settings/publishing/ | Owner `ets-public`, repository `brevi`, workflow `release.yml`, environment blank |
| npm | https://www.npmjs.com/package/brevi/access, Trusted Publisher, GitHub Actions | Organization or user `ets-public`, repository `brevi`, workflow `release.yml`, environment blank |
| crates.io | https://crates.io/crates/brevi/settings, Trusted Publishing, Add | Repository owner `ets-public`, repository `brevi`, workflow `release.yml`, environment blank |
| MCP registry | nothing to set up | Logs in with the workflow's GitHub OIDC token, which owns the `io.github.ets-public/*` names |

| Channel | Files | How it installs |
| --- | --- | --- |
| Claude Code plugin | `.claude-plugin/marketplace.json`, `plugins/brevi/` | `/plugin marketplace add ets-public/brevi`, then `/plugin install brevi@brevi`. Adds the Brevi skill and the `brevi mcp` server. Needs `brevi` on PATH. |
| MCP registry | `server.json` | Name `io.github.ets-public/brevi`, runs `brevi mcp` from the PyPI or npm package. Published by the `mcp-registry` job after PyPI and npm. |
| PyPI | `packaging/pypi/`, `tools/make_wheels.py` | `pip install brevi` or `uvx brevi mcp`. `make_wheels.py` repacks the five release binaries as wheels (no second build, so no extra macOS minutes). `pyproject.toml` (maturin) is kept for building a wheel from source. |
| crates.io | `Cargo.toml` (`include`) | `cargo install brevi`. Builds from source, so it needs Rust, which Brevi needs anyway. |
| npm | `packaging/npm/` | `npx brevi mcp`. The wrapper downloads the matching release binary on first run into `~/.cache/brevi/VERSION`. |
| Homebrew | `packaging/homebrew/make_formula.py` | `brew install ets-public/brevi/brevi` once the formula is in a tap repo (`ets-public/homebrew-brevi`). The script writes the formula from the release's SHA256SUMS. It installs the prebuilt binary and depends on `rust`. The repo must be public first, since Homebrew downloads without a login. |
| Dataset | `tools/export_dataset.py` | Writes tasks, verified solutions and a Hugging Face README to `dist/dataset`. |

Versions in `plugins/brevi/.claude-plugin/plugin.json`, `server.json` and `packaging/npm/package.json` must match `Cargo.toml` (CI checks this with `tools/version_sync.py`). The PyPI version comes from Cargo.toml.
`skills/gen_rules.py` keeps the plugin's copy of the skill in step with `skills/brevi/SKILL.md`.
