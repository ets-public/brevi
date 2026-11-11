# Upgrading Brevi

Brevi is 0.x and a new minor release ships most Mondays. Only the latest minor gets every fix, so keep current. See [SUPPORT.md](SUPPORT.md). The full list of changes is [CHANGELOG.md](../CHANGELOG.md).

## See your version

```
brevi --version
```

## Upgrade, by install channel

| Channel | Upgrade | Pin a version |
| --- | --- | --- |
| `install.sh` | run the install command again (see the README) | `BREVI_VERSION=0.27.0 sh install.sh` |
| pip | `pip install --upgrade brevi` | `pip install brevi==0.27.0` |
| npm | `npm install -g brevi@latest` | `npm install -g brevi@0.27.0` |
| cargo | `cargo install brevi --force` | `cargo install brevi --version 0.27.0 --force` |
| Docker image | pull `ghcr.io/ets-public/brevi-sandbox:latest` | use a tag such as `ghcr.io/ets-public/brevi-sandbox:0.27.0` (from v0.27.0) |
| GitHub Action | `ets-public/brevi/setup` at a newer tag | `uses: ets-public/brevi/setup@v0.27.0`, or the `version` input |
| Dev container feature | set `version` to `latest` (the default) | `"version": "0.27.0"` |
| Python and Node SDKs | upgrade the pip or npm package | pin the package version |
| Windows | unzip `brevi-x86_64-pc-windows-msvc.zip` from the release over the old `brevi.exe` | download that release's zip |

Notes:

- `install.sh` puts `brevi` and `brevi-python` in `~/.cargo/bin` and checks the download against the release's `SHA256SUMS`. `BREVI_FROM_SOURCE=1` builds from source instead.
- The npm wrapper downloads the matching release binary on first run into `~/.cache/brevi/VERSION`, so a new package version fetches a new binary.
- The Action, the dev container feature and the sandbox image exist from v0.27.0.
- The pip, npm, cargo and Docker pin forms are those tools' usual syntax. The tag names are the release versions.
- Brevi's build cache is keyed on the compiler, so a new release does not reuse binaries built by an old one.
- After upgrading, run `brevi verify x.py --input tests/` on your own programs. It prints a certificate when the native build and CPython agree.

## Behaviour changes by version

Newest first. Only changes that can affect programs or setups that already work.

### v0.27.0

- **MCP sandbox is locked by default.** A plain `brevi mcp` refuses a call with `sandbox: false`. A call can grant only what you passed to `brevi mcp` (`--allow`, `--allow-py`, `--allow-net`, `--allow-read DIR`, `--allow-write DIR`); the default is nothing. `--trust-calls` restores the old behaviour, and the `verify` tool needs it. The Python and Node SDKs start it that way, so they work as before.
- **Sandboxed programs start with a short environment.** Only `PATH`, locale, `TZ`, terminal size, temp folders and Brevi's own `BREVI_*` settings. With `--allow-py` or `--allow native`, also the home folder and proxy and certificate settings. If a sandboxed program read a key from the environment, pass it another way.
- **Sandboxed runs are checked against Brevi's dynamic build.** The MCP `run` tool and `brevi run --verify --sandbox` compare the typed answer with the dynamic build. When they differ you get the dynamic answer and a `{"verify":"differs",...}` line.
- **Limits.** `--limit written` now holds to the byte and needs `--sandbox` without `--allow-py` or `--allow native`. `--limit threads` is held by a cgroup. Under `written`, SQLite, DuckDB and ONNX Runtime open database files read-only and write no files.
- **Arguments.** `brevi run file.py a b` passes `a b` to the program. A dashed word after the file that is not a Brevi option is refused with exit 2; put it after `--` to pass it on.
- **Big ints.** On the typed build an int that passes 64 bits no longer wraps. `brevi run` re-runs such a program from the start on the dynamic build.
- **Windows is beta.** Programs that import libraries now build there. Text output uses CRLF as CPython does. See [KNOWN_ISSUES.md](KNOWN_ISSUES.md) for what is refused.
- **MCP argument checks.** An `allow_read` or `allow_write` item with a comma is refused. `code`, `patch` and `text` must be strings.

### v0.26.0

- **New limits.** `--limit` also takes `written`, `threads`, `fds`, `files` and `net`. A limit the platform cannot enforce is refused, not dropped.
- **Folder-level grants.** `--allow-read DIR` and `--allow-write DIR` hold a program to folders. `--allow read` and `--allow write` still mean every file the user can reach.
- **CPython is held to file grants on Linux** under `--sandbox` (Landlock, Linux 5.13 or later). Where Landlock is missing, named folders are refused.
- **numpy needs no grant under `--sandbox`.** Hugging Face `Tokenizer.from_file` now needs a read grant, and `from_pretrained` a network grant.
- **`brevi mcp --http`** needs a bearer token (at least 16 characters) on any address other than 127.0.0.1.
- **Decorators and `json` options** that typed builds used to ignore now work or are refused. Programs that relied on the old wrong output change.

### v0.25.0

- **Uncaught exceptions in `.py` files exit 1**, as in CPython. Native typed builds used to exit 101. `.bv` programs still exit 101.
- **A `.py` file with Brevi-only syntax is refused** (`&&`, `=>`, `fn`, `true`, `.push`), so `brevi run` falls back to CPython.
- **`install.sh` stops on a checksum mismatch** or when it cannot verify the download.
- Many silent wrong answers on the typed path were fixed. A program that printed the old answer now prints CPython's.

### v0.24.6 (breaking)

- **Under `--sandbox`, a program's own `allow` lines grant nothing.** Only the caller grants: `--allow`, `--allow-py`, or the MCP `allow` and `allow_py`. A program that asks for more than the caller granted is refused. Move the grants from the file to the command.
