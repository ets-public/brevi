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

### Next release

These are on `main` now and ship with the next release.

- **Sandboxed CPython has no network without a grant (Linux).** Under `--sandbox`, a program that loads CPython (`--allow-py`, `--allow native`) and has no network grant gets `PermissionError` from `socket()`, in CPython, its C extensions and every process they start. A sandboxed program that starts CPython with hosts named by `--allow-net` is refused on every OS, because CPython cannot be held to those hosts. Grant `--allow net`, or leave CPython out.
- **DuckDB and ONNX Runtime libraries are pinned to the build.** A program loads the library only if its bytes still match the ones it was built against. After upgrading duckdb or onnxruntime, `brevi run` rebuilds by itself, but a binary from `brevi build` must be built again.
- **Pickling and copying iterators.** itertools iterators, `map`, `filter`, `zip`, `enumerate`, `reversed` and `functools.partial` pickle, copy and deep-copy as in CPython 3.13, with CPython's DeprecationWarnings for itertools. Before, some were saved as a plain list iterator, and `copy.copy` of a list, tuple, str, range, dict or set iterator returned the same iterator. Copying a generator raises `TypeError`, as in CPython.
- **Abstract methods are enforced.** In dynamic builds, instantiating an `abc.ABC` (or `metaclass=ABCMeta`) class that still has an abstract method raises CPython's `TypeError`. Before, the object was made.
- **Annotations are evaluated.** Dynamic builds evaluate module, class and def annotations as CPython 3.13 does, so `x: undefined_name` raises `NameError` and a bad subscript such as `int[str]` raises `TypeError`. The typed build sends a file whose annotations could raise to the dynamic build.
- **Builtin types answer only CPython's attributes.** `hasattr(int, "zzz")` is False and `int.zzz` raises `AttributeError` (a method stub came back before).
- **Refused at build time now:** `typing.ByteString`, and a class built on `zip`, `map`, `filter`, `enumerate`, `reversed` or an itertools type (these stopped at run time before). `collections.abc.ByteString` issues CPython 3.13's DeprecationWarning.
- **PEP 479 for generators.** In dynamic builds a `StopIteration` that escapes a generator or a generator expression raises `RuntimeError("generator raised StopIteration")`, as in CPython. Before, the generator ended quietly.
- **`is` and `id()`.** The typed build no longer answers `is` by comparing values (`[1] is [1]` was True): a file that uses `is` with anything but None, True, False or Ellipsis goes to the dynamic build, where `id()` and `is` now agree for every value (`id()` of a float, set, function or iterator was often 1).
- **OrderedDict and deque.** Pickling an OrderedDict gives back an OrderedDict (a plain dict before); calling dict's own methods on one (`dict.pop(od, k)`) is refused. Changing a deque or OrderedDict while iterating it raises, and `insert` into a full deque raises `IndexError`, as in CPython. `list.insert` past either end clamps in typed builds too.
- **`match` statements build natively** in the dynamic build, so `brevi build` and `--native` no longer refuse them; a pattern CPython's compiler rejects refuses the build.
- **New refusals at build time** (dynamic builds): `import dis`; `locals()` in the body of a class that may be an Enum; a class built on `datetime.date`, `datetime`, `time`, `timedelta` or `timezone`; reading `__bases__`, `__mro__`, `__dict__`, `dir()` or `vars()` of a class on `Generic` or `Protocol`; `issubclass()` with a Protocol; a module's `__spec__`, `__loader__` and `__dict__`. `brevi run` runs these with python3; `--strict` refuses them.
- **Module reprs and `__file__`.** A program module's repr is CPython's (`<module 'helper' from '/abs/helper.py'>`) and its `__file__` and `__doc__` now read; a library module's repr is the one the build machine's CPython prints. On Windows `__file__` no longer starts with `\\?\`.
- **Threads take turns.** In dynamic builds each `threading.Thread` runs on its own stack, and `Event`, `Lock`, `RLock`, `join()` and `time.sleep()` wait as in CPython. Before, `start()` ran the whole thread at once. A thread that never waits prints what it printed before.
- **Windows paths are Windows paths.** In dynamic builds on Windows, `os.path` is `ntpath` (drive letters, both slashes, UNC shares), `os.sep` and `os.linesep` are Windows', and `sys.platform` is `win32` (`darwin` on macOS; it said `linux` everywhere before). A program on Windows that relied on POSIX path rules now gets CPython's answer. `pathlib` on Windows runs in CPython (refused with `--strict`).
- **More CPython warnings.** Dynamic builds issue CPython 3.13's DeprecationWarning and FutureWarning where CPython does (`int()` of an `__int__` returning a bool, `functools.partial` read from an instance), naming the program's own line. `warnings` filters in the program hide them, as in CPython.
- **Releases are held to the conformance gates.** Nothing is published unless core conformance is at least 90%, stdlib at least 80%, and neither has a known-wrong result.
- **New `--strict`** (or `BREVI_STRICT=1`) refuses every fallback. It changes nothing unless you pass it ([Strict builds](https://brevi-lang.org/docs/usage#strict-builds)).

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
