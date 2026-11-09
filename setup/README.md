# Set up Brevi (GitHub Action)

Installs Brevi on a GitHub Actions runner, for CI jobs and agents that run there: the release binary for the runner (Linux x64 and arm64, macOS, Windows), checked against the release's SHA256SUMS, `brevi-python` beside it, and the minimal Rust toolchain if the runner has none (GitHub's own images already have it). Brevi's build cache is kept between runs, keyed on the Brevi version.

```yaml
steps:
  - uses: actions/checkout@v7
  - uses: ets-public/brevi/setup@v0.27.0
  - run: brevi run main.py
  - run: brevi verify main.py --input tests/
```

| Input | Default | |
| --- | --- | --- |
| `version` | the tag the action is used at (`setup@v0.27.0` installs 0.27.0), else the latest release | a version such as `0.27.0` |
| `cache` | `true` | keep `BREVI_CACHE` between runs with `actions/cache` |

Outputs: `version` (the version installed) and `path` (the `brevi` executable). Available from v0.27.0.
