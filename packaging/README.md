# Packaging (prepared, not published)

Everything here is ready for launch and nothing has been published yet.

| Channel | Files | How it installs |
| --- | --- | --- |
| Claude Code plugin | `.claude-plugin/marketplace.json`, `plugins/brevi/` | `/plugin marketplace add ets-public/brevi`, then `/plugin install brevi@brevi`. Adds the Brevi skill and the `brevi mcp` server. Needs `brevi` on PATH. |
| MCP registry | `server.json` | Name `io.github.ets-public/brevi`, runs `brevi mcp` from the PyPI package. Publish with `mcp-publisher publish` after the PyPI upload. |
| PyPI | `packaging/pypi/` | `pip install brevi` or `uvx brevi mcp`. Wheels are built by the `wheels` job in `release.yml` (maturin, binary only). |
| npm | `packaging/npm/` | `npx brevi-lang mcp`. The wrapper downloads the matching release binary on first run into `~/.cache/brevi/VERSION`. |
| Homebrew | `packaging/homebrew/make_formula.py` | `brew install ets-public/brevi/brevi` once the formula is in a tap repo (`ets-public/homebrew-brevi`). The script writes the formula from the release's SHA256SUMS. It installs the prebuilt binary and depends on `rust`. The repo must be public first, since Homebrew downloads without a login. |
| Dataset | `tools/export_dataset.py` | Writes tasks, verified solutions and a Hugging Face README to `dist/dataset`. |

Versions in `plugins/brevi/.claude-plugin/plugin.json`, `server.json` and `packaging/npm/package.json` must match `Cargo.toml` (CI checks this with `tools/version_sync.py`). The PyPI version comes from Cargo.toml.
`skills/gen_rules.py` keeps the plugin's copy of the skill in step with `skills/brevi/SKILL.md`.
